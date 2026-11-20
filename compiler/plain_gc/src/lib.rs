//! PlainGC's interface (D51).
//!
//! Every allocation a Plainly program makes goes through [`allocate`], so the collector can change without
//! anything else changing. The real PlainGC is ZGC-style (D69): it collects while the program runs, moves objects,
//! and finds references precisely through Cranelift's stack maps. That's a later milestone, built as its own piece
//! of work.
//!
//! This is the simplest correct version for now: it allocates and never frees, like Java's Epsilon collector. A
//! program's memory only grows, which is fine for the short programs the first milestone runs, and nothing it
//! allocates is ever freed while still in use.

use std::alloc::{Layout, alloc};
use std::ptr::NonNull;

/// Every allocation is aligned to 8 bytes, so a `Whole` or a pointer can sit at its start.
pub const ALIGNMENT: usize = 8;

/// Allocates `size` bytes for a Plainly value, aligned to [`ALIGNMENT`]. Gives `None` when memory runs out.
pub fn allocate(size: usize) -> Option<NonNull<u8>> {
    let layout = Layout::from_size_align(size.max(1), ALIGNMENT).ok()?;
    // SAFETY: `layout` has a size of at least 1, which is all `alloc` asks. The memory is never freed, so it stays
    // valid for as long as the program runs.
    let pointer = unsafe { alloc(layout) };
    NonNull::new(pointer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocations_are_aligned_and_separate() {
        let first = allocate(3).map(NonNull::as_ptr);
        let second = allocate(16).map(NonNull::as_ptr);
        assert!(first.is_some_and(|pointer| (pointer as usize).is_multiple_of(ALIGNMENT)));
        assert!(second.is_some_and(|pointer| (pointer as usize).is_multiple_of(ALIGNMENT)));
        assert_ne!(first, second);
    }
}
