//! PlainGC (D51), Plainly's garbage collector, in the style of Java's ZGC (D69).
//!
//! This is PlainGC's interface, the only part of it the runtime and the compiler rely on:
//!
//! - [`start`], once, before the program runs, with the stack map table PlainCompiler wrote (its format is in
//!   `stack_roots.rs`) and the frame pointer of the program's `main`;
//! - [`allocate`], for every object a program makes, which may collect first, and gives a colored reference to it;
//! - the load barrier (D69): wherever compiled code reads a reference, it checks it against the 64-bit word
//!   `plainly_gc_bad_mask`; if they share a bit, it calls the slow path, `plainly_gc_heal` in the runtime, which calls
//!   [`heal`], and stores the healed reference back where it read it;
//! - colored references (`color.rs`): [`address`] gives the object a reference points to;
//! - the object layout in `object.rs`: an object's address is its header word, and its payload starts one word after.
//!
//! Steps 1 to 3 are built: a collection pauses the program to take its stack's roots; PlainGC's marker thread marks
//! while the program runs; then a pause compacts by evacuating sparse pages, and the load barrier finds moved objects
//! through forwarding. The steps still to come are in `compiler/PROGRESS.md`.

mod color;
mod heap;
mod object;
mod stack_roots;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Condvar, Mutex, MutexGuard};

use heap::{Heap, HeapError};
pub use object::{ObjectKind, WORD};
use stack_roots::SafepointTable;

/// Where an object's payload starts, from its address.
pub const PAYLOAD_OFFSET: usize = WORD;

/// A collection starts once this many bytes have been allocated since the last one, or as many as were live after
/// it, whichever is more. A placeholder until the design session decides the pacer (`compiler/PROGRESS.md`).
const LEAST_BYTES_BETWEEN_COLLECTIONS: usize = 4 * 1024 * 1024;

/// Why an allocation failed.
#[derive(Debug, PartialEq, Eq)]
pub enum AllocationError {
    /// The system has no more memory to give.
    OutOfMemory,
    /// A bug in PlainGC or PlainCompiler, described for the report.
    Internal(String),
}

impl From<HeapError> for AllocationError {
    fn from(error: HeapError) -> AllocationError {
        match error {
            HeapError::OutOfMemory => AllocationError::OutOfMemory,
            HeapError::Internal(what) => AllocationError::Internal(what),
        }
    }
}

struct Collector {
    heap: Heap,
    table: SafepointTable,
    /// The frame pointer of the program's `main`, where the walk for stack roots stops; 0 before [`start`].
    main_frame: usize,
    /// Collect at every allocation and evacuate every page, so tests reach every path quickly. Set by
    /// `PLAINGC_STRESS=1` in the environment, which no Plainly program can read.
    stress: bool,
    /// Where the collection under way is.
    phase: Phase,
    /// Whether PlainGC's marker thread has been started.
    marker_started: bool,
    /// A bug the marker thread found, which the program's next allocation reports.
    marker_failure: Option<String>,
    /// Counts for `PLAINGC_REPORT=1`: collections run, objects moved, and batches the marker thread marked.
    collections: usize,
    moved_objects: usize,
    marker_batches: usize,
}

/// Where a collection is (D69).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    /// No collection under way.
    Idle,
    /// The marker thread is marking while the program runs.
    Marking,
    /// Marking's work ran out; the program's next allocation finishes the collection.
    MarkingDone,
}

/// How many objects the marker thread marks each time it holds the lock, so the program never waits long for it.
const MARKING_BATCH: usize = 256;

/// What PlainGC has done so far, for its own tests.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub collections: usize,
    pub moved_objects: usize,
    pub healed_references: usize,
    pub marker_batches: usize,
}

/// The colors a reference mustn't have to pass the load barrier's fast path: every color but the good one. Compiled
/// code reads it by this name.
#[unsafe(no_mangle)]
#[allow(non_upper_case_globals)]
pub static plainly_gc_bad_mask: AtomicU64 = AtomicU64::new(color::COLOR_BITS & !color::FIRST_COLOR);

/// The one heap, shared by every thread of the program and PlainGC's own (D69), behind one lock.
static COLLECTOR: Mutex<Option<Collector>> = Mutex::new(None);

/// Wakes the marker thread when a marking starts, and the program when marking's work runs out.
static PHASE_CHANGED: Condvar = Condvar::new();

fn broken_lock<T>(_: T) -> AllocationError {
    AllocationError::Internal("PlainGC's lock was left broken".to_string())
}

fn new_collector() -> Collector {
    Collector {
        heap: Heap::new(),
        table: SafepointTable::default(),
        main_frame: 0,
        stress: false,
        phase: Phase::Idle,
        marker_started: false,
        marker_failure: None,
        collections: 0,
        moved_objects: 0,
        marker_batches: 0,
    }
}

fn lock() -> Result<MutexGuard<'static, Option<Collector>>, AllocationError> {
    COLLECTOR.lock().map_err(broken_lock)
}

fn with_collector<T>(
    work: impl FnOnce(&mut Collector) -> Result<T, AllocationError>,
) -> Result<T, AllocationError> {
    let mut guard = lock()?;
    work(guard.get_or_insert_with(new_collector))
}

/// Starts PlainGC for a compiled program.
///
/// # Safety
///
/// `stack_maps` must point to the table PlainCompiler wrote into the running program, and `main_frame` must be the
/// frame pointer of its `main`, which stays on the stack while the program runs.
pub unsafe fn start(stack_maps: *const u64, main_frame: usize) -> Result<(), AllocationError> {
    // SAFETY: passed on from this function's own promise.
    let table = unsafe { SafepointTable::read(stack_maps) }.map_err(AllocationError::Internal)?;
    let stress = std::env::var_os("PLAINGC_STRESS").is_some_and(|value| value == "1");
    with_collector(|collector| {
        collector.table = table;
        collector.main_frame = main_frame;
        collector.stress = stress;
        Ok(())
    })
}

/// Allocates an object of `kind` with a payload of `length` bytes, all zero, collecting first when it's time. Gives a
/// good-colored reference to it.
///
/// Any reference the caller holds outside the stack maps' slots may be out of date after this returns, since a
/// collection moves objects; the runtime reads what it needs from its arguments before allocating.
pub fn allocate(kind: ObjectKind, length: usize) -> Result<u64, AllocationError> {
    let mut guard = lock()?;
    if guard
        .as_ref()
        .is_some_and(|collector| collector.main_frame != 0)
    {
        guard = run_collection(guard)?;
    }
    let collector = guard.get_or_insert_with(new_collector);
    let object = collector.heap.allocate(kind, length)?;
    if object == 0 {
        return Err(AllocationError::Internal(
            "PlainGC allocated at address 0".to_string(),
        ));
    }
    Ok(collector.heap.reference_to(object))
}

/// Moves the collection along at an allocation, on the program's thread: finishes one whose marking is done, and
/// starts one when it's due. In collect-often mode, it waits for the marker thread first, so allocations take turns
/// finishing one collection and starting the next.
fn run_collection(
    guard: MutexGuard<'static, Option<Collector>>,
) -> Result<MutexGuard<'static, Option<Collector>>, AllocationError> {
    let stress = guard.as_ref().is_some_and(|collector| collector.stress);
    let mut guard = if stress {
        PHASE_CHANGED
            .wait_while(guard, |collector| {
                collector
                    .as_ref()
                    .is_some_and(|collector| collector.phase == Phase::Marking)
            })
            .map_err(broken_lock)?
    } else {
        guard
    };
    let collector = guard.get_or_insert_with(new_collector);
    if let Some(failure) = collector.marker_failure.take() {
        return Err(AllocationError::Internal(failure));
    }
    let finishing = collector.phase == Phase::MarkingDone;
    if finishing {
        // trace: D69
        let report = collector.heap.finish(collector.stress)?;
        collector.phase = Phase::Idle;
        collector.collections += 1;
        collector.moved_objects += report.moved_objects;
        plainly_gc_bad_mask.store(
            color::bad_mask(collector.heap.good_color()),
            Ordering::SeqCst,
        );
    }
    let due = collector.heap.allocated_since_collection
        >= LEAST_BYTES_BETWEEN_COLLECTIONS.max(collector.heap.live_after_collection);
    // An allocation that finishes a collection doesn't start the next, so the program always runs a while between
    // them: with references the barrier has to heal, then with marking under way.
    if collector.phase == Phase::Idle && !finishing && (collector.stress || due) {
        begin_marking(collector)?;
        if !collector.marker_started {
            std::thread::Builder::new()
                .name("PlainGC marker".to_string())
                .spawn(marker)
                .map_err(|error| {
                    AllocationError::Internal(format!(
                        "PlainGC's marker thread couldn't start: {error}"
                    ))
                })?;
            collector.marker_started = true;
        }
        PHASE_CHANGED.notify_all();
    }
    Ok(guard)
}

/// Starts a marking, with the program paused for its stack's roots only.
fn begin_marking(collector: &mut Collector) -> Result<(), AllocationError> {
    let Collector {
        heap,
        table,
        main_frame,
        phase,
        ..
    } = collector;
    let mut roots = |visitor: &mut dyn FnMut(&mut usize)| {
        // SAFETY: the program's frames and the runtime's keep frame pointers, and `table` describes the compiled
        // code on this stack, as `start` was promised. This runs on the program's own thread.
        unsafe { stack_roots::visit(table, *main_frame, visitor) }.map_err(HeapError::Internal)
    };
    heap.begin_marking(&mut roots)?;
    *phase = Phase::Marking;
    plainly_gc_bad_mask.store(color::bad_mask(heap.good_color()), Ordering::SeqCst);
    Ok(())
}

/// PlainGC's marker thread: marks while the program runs (D69), a batch at a time, and says when marking's work
/// has run out.
fn marker() {
    loop {
        let Ok(guard) = COLLECTOR.lock() else {
            return;
        };
        let Ok(mut guard) = PHASE_CHANGED.wait_while(guard, |collector| {
            !collector
                .as_ref()
                .is_some_and(|collector| collector.phase == Phase::Marking)
        }) else {
            return;
        };
        let Some(collector) = guard.as_mut() else {
            continue;
        };
        collector.marker_batches += 1;
        let failure = match collector.heap.mark_some(MARKING_BATCH) {
            Ok(false) => None,
            Ok(true) => Some(None),
            Err(HeapError::Internal(what)) => Some(Some(what)),
            Err(HeapError::OutOfMemory) => {
                Some(Some("PlainGC's marker ran out of memory".to_string()))
            }
        };
        if let Some(failure) = failure {
            collector.marker_failure = failure;
            collector.phase = Phase::MarkingDone;
            PHASE_CHANGED.notify_all();
        }
        drop(guard);
        std::thread::yield_now();
    }
}

/// The load barrier's slow path (D69): brings a reference of a bad color up to date, finding a moved object through
/// the forwarding table.
pub fn heal(reference: u64) -> Result<u64, AllocationError> {
    with_collector(|collector| Ok(collector.heap.heal(reference)))
}

/// The address of the object a reference points to, whatever its color. Only an up-to-date reference, one that has
/// passed the load barrier, points where its object is now.
pub fn address(reference: u64) -> *mut u8 {
    color::address(reference) as *mut u8
}

/// What PlainGC has done so far, for its own tests: the runtime shows it when `PLAINGC_REPORT=1` is in the
/// environment, which no Plainly program can read.
pub fn report() -> Option<Report> {
    with_collector(|collector| {
        Ok(Report {
            collections: collector.collections,
            moved_objects: collector.moved_objects,
            healed_references: collector.heap.healed,
            marker_batches: collector.marker_batches,
        })
    })
    .ok()
}

/// The length of an object's payload, in bytes.
///
/// # Safety
///
/// `object` must be an object PlainGC allocated, still live.
pub unsafe fn payload_length(object: *const u8) -> usize {
    // SAFETY: an object starts with its header word, aligned, as the caller promises this is one.
    let header = unsafe { object.cast::<u64>().read() };
    object::read_header(header).map_or(0, |(_, length)| length)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocation_before_start_never_collects() {
        let first = allocate(ObjectKind::Bytes, 3);
        let second = allocate(ObjectKind::Bytes, 16);
        assert!(
            first
                .as_ref()
                .is_ok_and(|&reference| (address(reference) as usize).is_multiple_of(WORD))
        );
        assert_ne!(first, second);
        // SAFETY: `second` was just allocated with a 16-byte payload.
        let length = second.map(|reference| unsafe { payload_length(address(reference)) });
        assert_eq!(length, Ok(16));
    }

    #[test]
    fn a_fresh_reference_passes_the_barrier() {
        let reference = allocate(ObjectKind::Bytes, 1).unwrap_or(0);
        assert_ne!(reference, 0);
        assert_eq!(reference & plainly_gc_bad_mask.load(Ordering::SeqCst), 0);
        assert_eq!(heal(reference), Ok(reference));
    }
}
