//! PlainGC (D51), Plainly's garbage collector, in the style of Java's ZGC (D69).
//!
//! This is PlainGC's interface, the only part of it the runtime and the compiler rely on:
//!
//! - [`start`], once, before the program runs, with the stack map table PlainCompiler wrote (its format is in
//!   `stack_roots.rs`) and the frame pointer of the program's `main`;
//! - [`allocate`], for every object a program makes, which may collect first;
//! - the object layout in `object.rs`: an object's address is its header word, and its payload starts one word after.
//!
//! Step 1 is built: a collection stops the program, marks everything reachable from the roots, and compacts by
//! evacuating sparse pages. The steps still to come are in `compiler/PROGRESS.md`.

mod heap;
mod object;
mod stack_roots;

use std::ptr::NonNull;
use std::sync::Mutex;

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
    /// How many collections have run, and how many objects they moved, for `PLAINGC_REPORT=1`.
    collections: usize,
    moved_objects: usize,
}

/// The one heap, shared by every thread of the program (D69), behind one lock.
static COLLECTOR: Mutex<Option<Collector>> = Mutex::new(None);

fn with_collector<T>(
    work: impl FnOnce(&mut Collector) -> Result<T, AllocationError>,
) -> Result<T, AllocationError> {
    let mut guard = COLLECTOR
        .lock()
        .map_err(|_| AllocationError::Internal("PlainGC's lock was left broken".to_string()))?;
    let collector = guard.get_or_insert_with(|| Collector {
        heap: Heap::new(),
        table: SafepointTable::default(),
        main_frame: 0,
        stress: false,
        collections: 0,
        moved_objects: 0,
    });
    work(collector)
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

/// Allocates an object of `kind` with a payload of `length` bytes, all zero, collecting first when it's time.
///
/// Any reference the caller holds outside the stack maps' slots may be out of date after this returns, since a
/// collection moves objects; the runtime reads what it needs from its arguments before allocating.
pub fn allocate(kind: ObjectKind, length: usize) -> Result<NonNull<u8>, AllocationError> {
    with_collector(|collector| {
        let started = collector.main_frame != 0;
        let due = collector.heap.allocated_since_collection
            >= LEAST_BYTES_BETWEEN_COLLECTIONS.max(collector.heap.live_after_collection);
        if started && (collector.stress || due) {
            collect(collector)?;
        }
        let address = collector.heap.allocate(kind, length)?;
        NonNull::new(address as *mut u8)
            .ok_or_else(|| AllocationError::Internal("PlainGC allocated at address 0".to_string()))
    })
}

/// A collection with the program stopped, with the stack's references as its roots.
fn collect(collector: &mut Collector) -> Result<(), AllocationError> {
    let Collector {
        heap,
        table,
        main_frame,
        stress,
        collections,
        moved_objects,
    } = collector;
    let mut roots = |visitor: &mut dyn FnMut(&mut usize)| {
        // SAFETY: the program's frames and the runtime's keep frame pointers, and `table` describes the compiled
        // code on this stack, as `start` was promised.
        unsafe { stack_roots::visit(table, *main_frame, visitor) }.map_err(HeapError::Internal)
    };
    let report = heap.collect(&mut roots, *stress)?;
    *collections += 1;
    *moved_objects += report.moved_objects;
    Ok(())
}

/// How many collections have run and how many objects they moved, for PlainGC's own tests: the runtime shows it
/// when `PLAINGC_REPORT=1` is in the environment, which no Plainly program can read.
pub fn report() -> Option<(usize, usize)> {
    with_collector(|collector| Ok((collector.collections, collector.moved_objects))).ok()
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
        let first = allocate(ObjectKind::Bytes, 3).map(NonNull::as_ptr);
        let second = allocate(ObjectKind::Bytes, 16).map(NonNull::as_ptr);
        assert!(
            first
                .as_ref()
                .is_ok_and(|pointer| (*pointer as usize).is_multiple_of(WORD))
        );
        assert_ne!(first, second);
        // SAFETY: `second` was just allocated with a 16-byte payload.
        let length = second.map(|pointer| unsafe { payload_length(pointer) });
        assert_eq!(length, Ok(16));
    }
}
