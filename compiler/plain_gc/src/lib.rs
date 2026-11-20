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
//! Steps 1 to 4 are built: a collection pauses the program briefly to take its stack's roots; PlainGC's own thread
//! marks while the program runs; a brief pause chooses the pages to evacuate; PlainGC's thread moves their live
//! objects while the program runs, and the load barrier finds moved objects through forwarding, moving any it reaches
//! first. The step still to come is in `compiler/PROGRESS.md`.

mod color;
pub mod handles;
mod heap;
mod object;
mod pacer;
mod stack_roots;

use std::cell::Cell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Condvar, Mutex, MutexGuard};
use std::time::Instant;

use heap::{Heap, HeapError};
use object::object_size;
pub use object::{ObjectKind, WORD};
use pacer::Pacer;
use stack_roots::SafepointTable;

/// Where an object's payload starts, from its address.
pub const PAYLOAD_OFFSET: usize = WORD;

/// Why an allocation failed.
#[derive(Debug, PartialEq, Eq)]
pub enum AllocationError {
    /// The heap can't grow past its hard limit, or the system has no more memory to give, and collecting freed too
    /// little (D115): `needed` more bytes were asked for, with `in_use` of the `limit` in use (D122).
    OutOfMemory {
        needed: usize,
        in_use: usize,
        limit: usize,
    },
    /// A bug in PlainGC or PlainCompiler, described for the report.
    Internal(String),
}

impl AllocationError {
    /// The error, with how many bytes the heap holds when it ran out, and its limit.
    fn in_collector(self, collector: &Collector) -> AllocationError {
        match self {
            AllocationError::OutOfMemory { needed, .. } => AllocationError::OutOfMemory {
                needed,
                in_use: collector.heap.used,
                limit: collector.hard_limit,
            },
            other => other,
        }
    }
}

impl From<HeapError> for AllocationError {
    fn from(error: HeapError) -> AllocationError {
        match error {
            HeapError::OutOfMemory { needed } => AllocationError::OutOfMemory {
                needed,
                in_use: 0,
                limit: 0,
            },
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
    /// Whether PlainGC's own thread has been started.
    worker_started: bool,
    /// A bug PlainGC's thread found, which the program's next allocation reports.
    worker_failure: Option<String>,
    /// Counts for `PLAINGC_REPORT=1`: collections run, objects moved, and batches PlainGC's thread did.
    collections: usize,
    moved_objects: usize,
    worker_batches: usize,
    /// The longest the program has been paused for a collection, in microseconds.
    longest_pause_microseconds: u128,
    /// Kept handles' references (D105), roots shared by every thread, by slot; `None` is a free slot.
    kept: Vec<Option<u64>>,
    /// When collections start (D113).
    pacer: Pacer,
    /// The most bytes the heap may hold (D114).
    hard_limit: usize,
}

/// Where a collection is (D69).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    /// No collection under way.
    Idle,
    /// PlainGC's thread is marking while the program runs.
    Marking,
    /// Marking's work ran out; the program's next allocation ends the marking.
    MarkingDone,
    /// PlainGC's thread is moving live objects out of the pages being evacuated while the program runs.
    Relocating,
    /// Every object has moved; the program's next allocation frees the evacuated pages.
    RelocationDone,
}

/// How many objects PlainGC's thread marks or moves each time it holds the lock, so the program never waits long
/// for it.
const BATCH: usize = 256;

/// What PlainGC has done so far, for its own tests.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub collections: usize,
    pub moved_objects: usize,
    pub healed_references: usize,
    pub relocated_by_barrier: usize,
    pub worker_batches: usize,
    pub longest_pause_microseconds: u128,
}

/// The colors a reference mustn't have to pass the load barrier's fast path: every color but the good one. Compiled
/// code reads it by this name.
#[unsafe(no_mangle)]
#[allow(non_upper_case_globals)]
pub static plainly_gc_bad_mask: AtomicU64 = AtomicU64::new(color::COLOR_BITS & !color::FIRST_COLOR);

/// The one heap, shared by every thread of the program and PlainGC's own (D69), behind one lock.
static COLLECTOR: Mutex<Option<Collector>> = Mutex::new(None);

/// Wakes PlainGC's thread when a phase it works in starts, and the program when that work runs out.
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
        worker_started: false,
        worker_failure: None,
        collections: 0,
        moved_objects: 0,
        worker_batches: 0,
        longest_pause_microseconds: 0,
        kept: Vec::new(),
        pacer: Pacer::new(Instant::now()),
        hard_limit: pacer::hard_limit(),
    }
}

fn lock() -> Result<MutexGuard<'static, Option<Collector>>, AllocationError> {
    COLLECTOR.lock().map_err(broken_lock)
}

fn with_collector<T>(
    work: impl FnOnce(&mut Collector) -> Result<T, AllocationError>,
) -> Result<T, AllocationError> {
    let mut guard = lock()?;
    let collector = guard.get_or_insert_with(new_collector);
    work(collector).map_err(|error| error.in_collector(collector))
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
    // A smaller hard limit, for tests of running out of memory, which no Plainly program can read.
    let test_limit = std::env::var("PLAINGC_TEST_HARD_LIMIT_MIB")
        .ok()
        .and_then(|mebibytes| mebibytes.parse::<usize>().ok())
        .map(|mebibytes| mebibytes.saturating_mul(1024 * 1024));
    with_collector(|collector| {
        collector.table = table;
        collector.main_frame = main_frame;
        collector.stress = stress;
        if let Some(limit) = test_limit {
            collector.hard_limit = limit;
        }
        Ok(())
    })
}

/// Allocates an object of `kind` with a payload of `length` bytes, all zero, collecting first when it's time. Gives a
/// good-colored reference to it.
///
/// Any reference the caller holds outside the stack maps' slots may be out of date after this returns, since a
/// collection moves objects; the runtime reads what it needs from its arguments before allocating.
pub fn allocate(kind: ObjectKind, length: usize) -> Result<u64, AllocationError> {
    allocate_with_roots(kind, length, &mut [])
}

/// [`allocate`], with `roots` as roots too: an `extern.rust` call's handles (D105), which a collection may heal.
pub(crate) fn allocate_with_roots(
    kind: ObjectKind,
    length: usize,
    roots: &mut [Cell<u64>],
) -> Result<u64, AllocationError> {
    let mut guard = lock()?;
    let started = guard
        .as_ref()
        .is_some_and(|collector| collector.main_frame != 0);
    if started {
        guard = run_collection(guard, roots)?;
    }
    let mut collected = false;
    loop {
        let collector = guard.get_or_insert_with(new_collector);
        if collector.heap.used.saturating_add(object_size(length)) <= collector.hard_limit {
            match collector.heap.allocate(kind, length) {
                Ok(0) => {
                    return Err(AllocationError::Internal(
                        "PlainGC allocated at address 0".to_string(),
                    ));
                }
                Ok(object) => return Ok(collector.heap.reference_to(object)),
                // The system refused memory before the hard limit: collect, as at the limit.
                Err(HeapError::OutOfMemory { .. }) => {}
                Err(HeapError::Internal(what)) => return Err(AllocationError::Internal(what)),
            }
        }
        // trace: D115
        if collected || !started {
            // trace: D122
            return Err(AllocationError::OutOfMemory {
                needed: object_size(length),
                in_use: collector.heap.used,
                limit: collector.hard_limit,
            });
        }
        // The program waits while PlainGC collects everything it can, as ZGC's allocation stall does.
        guard = collect_fully(guard, roots)?;
        collected = true;
    }
}

/// Runs collections until one that started after now has finished, so everything unreachable now is freed. The
/// program waits meanwhile; PlainGC's thread does the marking and moving.
fn collect_fully(
    mut guard: MutexGuard<'static, Option<Collector>>,
    roots: &mut [Cell<u64>],
) -> Result<MutexGuard<'static, Option<Collector>>, AllocationError> {
    let target = {
        let collector = guard.get_or_insert_with(new_collector);
        collector.collections + if collector.phase == Phase::Idle { 1 } else { 2 }
    };
    loop {
        guard = PHASE_CHANGED
            .wait_while(guard, |collector| {
                collector.as_ref().is_some_and(|collector| {
                    matches!(collector.phase, Phase::Marking | Phase::Relocating)
                })
            })
            .map_err(broken_lock)?;
        let collector = guard.get_or_insert_with(new_collector);
        if let Some(failure) = collector.worker_failure.take() {
            return Err(AllocationError::Internal(failure));
        }
        if collector.collections >= target {
            return Ok(guard);
        }
        match collector.phase {
            Phase::MarkingDone => end_marking(collector)?,
            Phase::RelocationDone => end_relocation(collector),
            Phase::Idle => start_collection(collector, roots)?,
            Phase::Marking | Phase::Relocating => {}
        }
    }
}

/// Ends a marking, with the program paused: chooses the pages to evacuate and changes the good color.
fn end_marking(collector: &mut Collector) -> Result<(), AllocationError> {
    collector.heap.end_marking(collector.stress)?;
    collector.phase = Phase::Relocating;
    plainly_gc_bad_mask.store(
        color::bad_mask(collector.heap.good_color()),
        Ordering::SeqCst,
    );
    PHASE_CHANGED.notify_all();
    Ok(())
}

/// Ends a relocation: frees the evacuated pages, and the collection is over.
fn end_relocation(collector: &mut Collector) {
    collector.moved_objects += collector.heap.end_relocation();
    collector.collections += 1;
    collector.phase = Phase::Idle;
    collector.pacer.collection_ended(Instant::now());
}

/// Starts a collection: takes the roots, and wakes PlainGC's thread to mark, starting it the first time.
fn start_collection(
    collector: &mut Collector,
    roots: &mut [Cell<u64>],
) -> Result<(), AllocationError> {
    collector.pacer.collection_started(Instant::now());
    begin_marking(collector, roots)?;
    if !collector.worker_started {
        std::thread::Builder::new()
            .name("PlainGC".to_string())
            .spawn(worker)
            .map_err(|error| {
                AllocationError::Internal(format!("PlainGC's thread couldn't start: {error}"))
            })?;
        collector.worker_started = true;
    }
    PHASE_CHANGED.notify_all();
    Ok(())
}

/// Moves the collection along at an allocation, on the program's thread, with the program paused only briefly: ends
/// a marking that's done, frees the pages a finished relocation emptied, and starts a collection when one is due. In
/// collect-often mode, it waits for PlainGC's thread first, so every allocation moves the collection one phase on.
fn run_collection(
    guard: MutexGuard<'static, Option<Collector>>,
    roots: &mut [Cell<u64>],
) -> Result<MutexGuard<'static, Option<Collector>>, AllocationError> {
    let stress = guard.as_ref().is_some_and(|collector| collector.stress);
    let mut guard = if stress {
        PHASE_CHANGED
            .wait_while(guard, |collector| {
                collector.as_ref().is_some_and(|collector| {
                    matches!(collector.phase, Phase::Marking | Phase::Relocating)
                })
            })
            .map_err(broken_lock)?
    } else {
        guard
    };
    let collector = guard.get_or_insert_with(new_collector);
    if let Some(failure) = collector.worker_failure.take() {
        return Err(AllocationError::Internal(failure));
    }
    let pause_start = std::time::Instant::now();
    // trace: D69
    let moved_on = match collector.phase {
        Phase::MarkingDone => {
            end_marking(collector)?;
            true
        }
        Phase::RelocationDone => {
            end_relocation(collector);
            true
        }
        _ => false,
    };
    // trace: D113
    let now = Instant::now();
    collector.pacer.sample(now, collector.heap.allocated_total);
    let due = || {
        let soft_target =
            pacer::soft_target(collector.heap.live_after_collection, collector.hard_limit);
        collector
            .pacer
            .should_start(
                now,
                collector.heap.used,
                collector.heap.allocated_since_collection,
                soft_target,
            )
            .is_some()
    };
    // An allocation that moves a collection on doesn't also start the next, so the program always runs a while in
    // every phase.
    if collector.phase == Phase::Idle && !moved_on && (collector.stress || due()) {
        start_collection(collector, roots)?;
    }
    let pause = pause_start.elapsed().as_micros();
    collector.longest_pause_microseconds = collector.longest_pause_microseconds.max(pause);
    Ok(guard)
}

/// Starts a marking, with the program paused for its stack's roots only.
fn begin_marking(
    collector: &mut Collector,
    handles: &mut [Cell<u64>],
) -> Result<(), AllocationError> {
    let Collector {
        heap,
        table,
        main_frame,
        phase,
        kept,
        ..
    } = collector;
    let mut roots = |visitor: &mut dyn FnMut(&mut usize)| {
        // Kept handles and the current call's handles (D105), then the stack.
        for reference in kept
            .iter_mut()
            .flatten()
            .chain(handles.iter_mut().map(Cell::get_mut))
        {
            let mut slot = *reference as usize;
            visitor(&mut slot);
            *reference = slot as u64;
        }
        if *main_frame == 0 {
            return Ok(());
        }
        // SAFETY: the program's frames and the runtime's keep frame pointers, and `table` describes the compiled
        // code on this stack, as `start` was promised. This runs on the program's own thread.
        unsafe { stack_roots::visit(table, *main_frame, visitor) }.map_err(HeapError::Internal)
    };
    heap.begin_marking(&mut roots)?;
    *phase = Phase::Marking;
    plainly_gc_bad_mask.store(color::bad_mask(heap.good_color()), Ordering::SeqCst);
    Ok(())
}

/// PlainGC's own thread: marks, then moves objects, while the program runs (D69), a batch at a time, and says when
/// each phase's work has run out.
fn worker() {
    loop {
        let Ok(guard) = COLLECTOR.lock() else {
            return;
        };
        let Ok(mut guard) = PHASE_CHANGED.wait_while(guard, |collector| {
            !collector.as_ref().is_some_and(|collector| {
                matches!(collector.phase, Phase::Marking | Phase::Relocating)
            })
        }) else {
            return;
        };
        let Some(collector) = guard.as_mut() else {
            continue;
        };
        collector.worker_batches += 1;
        let (result, next) = if collector.phase == Phase::Marking {
            (collector.heap.mark_some(BATCH), Phase::MarkingDone)
        } else {
            (collector.heap.relocate_some(BATCH), Phase::RelocationDone)
        };
        let failure = match result {
            Ok(false) => None,
            Ok(true) => Some(None),
            Err(HeapError::Internal(what)) => Some(Some(what)),
            Err(HeapError::OutOfMemory { .. }) => Some(Some(
                "PlainGC's thread ran out of memory while moving objects".to_string(),
            )),
        };
        if let Some(failure) = failure {
            collector.worker_failure = failure;
            collector.phase = next;
            PHASE_CHANGED.notify_all();
        }
        drop(guard);
        std::thread::yield_now();
    }
}

/// The load barrier's slow path (D69): brings a reference of a bad color up to date, finding a moved object through
/// the forwarding table.
pub fn heal(reference: u64) -> Result<u64, AllocationError> {
    with_collector(|collector| Ok(collector.heap.heal(reference)?))
}

thread_local! {
    /// Where the program is, as the site of the code about to allocate, for the message if memory runs out (D115).
    static SITE: Cell<(*const u8, i64)> = const { Cell::new((std::ptr::null(), 0)) };
}

/// Notes the site of the code about to allocate on this thread: the runtime does before making a value, and an
/// `extern.rust` procedure's glue before its Rust code runs.
pub fn note_site(site: *const u8, length: i64) {
    SITE.with(|noted| noted.set((site, length)));
}

/// The site noted last on this thread.
pub fn noted_site() -> (*const u8, i64) {
    SITE.with(Cell::get)
}

/// What stops the program when PlainGC can't go on inside an `extern.rust` call; the runtime installs its own.
static FAILURE_HANDLER: std::sync::OnceLock<fn(AllocationError) -> !> = std::sync::OnceLock::new();

/// Installs what stops the program when PlainGC fails inside a call that holds handles (D105), since how an
/// `extern.rust` procedure fails isn't decided: the runtime stops with its message.
pub fn set_failure_handler(handler: fn(AllocationError) -> !) {
    let _ = FAILURE_HANDLER.set(handler);
}

/// Stops the program because PlainGC can't go on.
pub(crate) fn fail(error: AllocationError) -> ! {
    if let Some(handler) = FAILURE_HANDLER.get() {
        handler(error);
    }
    // No runtime installed one, as in PlainGC's own tests: a bug in PlainGC, status 7 (D82).
    eprintln!("internal error: PlainGC itself went wrong, not your program: {error:?}");
    std::process::exit(7);
}

/// Registers a kept handle's reference as a root (D105), giving its slot.
pub(crate) fn keep_root(reference: u64) -> Result<usize, AllocationError> {
    with_collector(|collector| {
        if let Some(slot) = collector.kept.iter().position(Option::is_none) {
            collector.kept[slot] = Some(reference);
            return Ok(slot);
        }
        collector.kept.push(Some(reference));
        Ok(collector.kept.len() - 1)
    })
}

/// A kept handle's reference, through the load barrier, healed in its slot.
pub(crate) fn kept_root(slot: usize) -> Result<u64, AllocationError> {
    with_collector(|collector| {
        let Some(Some(reference)) = collector.kept.get(slot).copied() else {
            return Err(AllocationError::Internal(format!(
                "kept handle {slot} isn't registered"
            )));
        };
        let healed = if reference & plainly_gc_bad_mask.load(Ordering::SeqCst) != 0 {
            collector.heap.heal(reference)?
        } else {
            reference
        };
        collector.kept[slot] = Some(healed);
        Ok(healed)
    })
}

/// Releases a kept handle's slot, so its value is no longer a root.
pub(crate) fn release_root(slot: usize) {
    let _ = with_collector(|collector| {
        if let Some(reference) = collector.kept.get_mut(slot) {
            *reference = None;
        }
        Ok(())
    });
}

/// A whole collection, every phase in a row, with kept handles and `roots` as the only roots, moving every object it
/// can: for the handles' tests, which run without a program's stack.
#[cfg(test)]
pub(crate) fn collect_everything(roots: &mut [Cell<u64>]) -> Result<(), AllocationError> {
    with_collector(|collector| {
        begin_marking(collector, roots)?;
        while !collector.heap.mark_some(usize::MAX)? {}
        collector.heap.end_marking(true)?;
        while !collector.heap.relocate_some(usize::MAX)? {}
        collector.moved_objects += collector.heap.end_relocation();
        collector.collections += 1;
        collector.phase = Phase::Idle;
        plainly_gc_bad_mask.store(
            color::bad_mask(collector.heap.good_color()),
            Ordering::SeqCst,
        );
        Ok(())
    })
}

/// Tests that use the one global heap take this, so a collection in one never frees another's objects.
#[cfg(test)]
pub(crate) static TEST_LOCK: Mutex<()> = Mutex::new(());

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
            relocated_by_barrier: collector.heap.relocated_by_barrier,
            worker_batches: collector.worker_batches,
            longest_pause_microseconds: collector.longest_pause_microseconds,
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
        let _alone = TEST_LOCK.lock();
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
        let _alone = TEST_LOCK.lock();
        let reference = allocate(ObjectKind::Bytes, 1).unwrap_or(0);
        assert_ne!(reference, 0);
        assert_eq!(reference & plainly_gc_bad_mask.load(Ordering::SeqCst), 0);
        assert_eq!(heal(reference), Ok(reference));
    }
}
