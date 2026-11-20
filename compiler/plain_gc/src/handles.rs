//! Handles: how `extern.rust` code holds Plainly values that live in PlainGC's heap (D36, D69, D105).
//!
//! PlainGC moves objects while the program runs, so Rust code never holds a plain pointer to one. Instead:
//!
//! - **A [`Handle`] lasts for one call.** Each `extern.rust` call runs inside [`with_call`], which gives it a [`Call`];
//!   every handle is tied to that call, and Rust's compiler refuses code that keeps one past it.
//! - **A handle gives a direct look at the value** ([`Handle::read`]), with no copying. The look borrows the call;
//!   making a value ([`Call::new_text`]) can start a collection, which can move objects, so it borrows the call
//!   mutably, and Rust's borrow checker refuses code that makes a value while a look is held. A look never crosses a
//!   collection.
//! - **A [`Kept`] handle holds a value beyond one call.** PlainGC counts it as a root, shared by every thread, and
//!   releases it when the Rust code drops it.
//!
//! If PlainGC can't go on inside a call (out of memory, or a bug in PlainGC), the handler the runtime installs stops
//! the program with its message, so handles never hand Rust code an error to deal with.
//!
//! D105's example, as an `extern.rust` procedure's Rust function is written (D107):
//!
//! ```
//! use plain_gc::handles::{Call, Handle, Text, with_call};
//!
//! pub fn shout<'call>(call: &mut Call<'call>, text: Handle<'call, Text>) -> Handle<'call, Text> {
//!     let loud = text.read(call).to_uppercase();
//!     call.new_text(&loud)
//! }
//!
//! let shouted = with_call(|call| {
//!     let text = call.new_text("hello");
//!     let loud = shout(call, text);
//!     loud.read(call).to_string()
//! });
//! assert_eq!(shouted, "HELLO");
//! ```
//!
//! A handle can't outlive its call:
//!
//! ```compile_fail
//! use plain_gc::handles::with_call;
//!
//! let mut escaped = None;
//! with_call(|call| {
//!     escaped = Some(call.new_text("hello"));
//! });
//! ```
//!
//! And a look can't be held while a value is made, which could move the text it looks at:
//!
//! ```compile_fail
//! use plain_gc::handles::with_call;
//!
//! with_call(|call| {
//!     let text = call.new_text("hello");
//!     let look = text.read(call);
//!     call.new_text("something new");
//!     println!("{look}");
//! });
//! ```

use std::cell::Cell;
use std::marker::PhantomData;

use crate::{
    AllocationError, ObjectKind, PAYLOAD_OFFSET, address, allocate_with_roots, fail, heal,
    keep_root, kept_root, payload_length, plainly_gc_bad_mask, release_root,
};

/// The kind of value a handle holds: `Text` (D21).
pub enum Text {}

/// Ties a handle to one call, and makes `'call` exact, so no handle can be passed off as another call's.
type CallMarker<'call> = PhantomData<fn(&'call ()) -> &'call ()>;

/// One `extern.rust` call: the handles it holds, which are roots for any collection during the call. A call belongs
/// to the thread it runs on.
pub struct Call<'call> {
    /// Each handle's reference, healed by the load barrier when it's read, and by a collection's marking.
    roots: Vec<Cell<u64>>,
    _call: CallMarker<'call>,
    /// A call can't move to another thread: its handles are that thread's roots (D68).
    _thread: PhantomData<*const ()>,
}

/// A Plainly value, held for the rest of one call.
pub struct Handle<'call, T> {
    slot: usize,
    _call: CallMarker<'call>,
    _kind: PhantomData<fn() -> T>,
}

impl<T> Clone for Handle<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Handle<'_, T> {}

/// A Plainly value held beyond one call: a root until it's dropped.
pub struct Kept<T> {
    slot: usize,
    _kind: PhantomData<fn() -> T>,
}

/// Runs `work` as one `extern.rust` call, with a fresh [`Call`]. Nothing `work` gives back can hold one of its
/// handles.
pub fn with_call<R>(work: impl for<'call> FnOnce(&mut Call<'call>) -> R) -> R {
    let mut call = Call {
        roots: Vec::new(),
        _call: PhantomData,
        _thread: PhantomData,
    };
    work(&mut call)
}

impl<'call> Call<'call> {
    fn hold<T>(&mut self, reference: u64) -> Handle<'call, T> {
        self.roots.push(Cell::new(reference));
        Handle {
            slot: self.roots.len() - 1,
            _call: PhantomData,
            _kind: PhantomData,
        }
    }

    /// A handle's reference, through the load barrier (D69): healed in its slot if it has a bad color.
    fn current(&self, slot: usize) -> u64 {
        let Some(cell) = self.roots.get(slot) else {
            fail(AllocationError::Internal(format!(
                "handle {slot} doesn't belong to this call"
            )));
        };
        let reference = cell.get();
        if reference & plainly_gc_bad_mask.load(std::sync::atomic::Ordering::SeqCst) == 0 {
            return reference;
        }
        let healed = heal(reference).unwrap_or_else(|error| fail(error));
        cell.set(healed);
        healed
    }

    /// Makes new text holding `text`. It can start a collection, so it takes the call mutably: no look at a value
    /// can be held across it.
    pub fn new_text(&mut self, text: &str) -> Handle<'call, Text> {
        let reference = allocate_with_roots(ObjectKind::Bytes, text.len(), &mut self.roots)
            .unwrap_or_else(|error| fail(error));
        let object = address(reference);
        // SAFETY: `object` is a fresh object with a payload of `text.len()` bytes, starting `PAYLOAD_OFFSET` bytes
        // in, and nothing else uses it yet.
        unsafe {
            std::ptr::copy_nonoverlapping(text.as_ptr(), object.add(PAYLOAD_OFFSET), text.len())
        };
        self.hold(reference)
    }

    /// A handle to a text the compiled program passed in, as its colored reference.
    ///
    /// # Safety
    ///
    /// `reference` must be a reference to a live `Text`, as compiled code passes to an `extern.rust` procedure.
    pub unsafe fn adopt_text(&mut self, reference: u64) -> Handle<'call, Text> {
        self.hold(reference)
    }
}

impl<'call> Handle<'call, Text> {
    /// A direct look at the text, with no copying, for as long as the call is borrowed.
    pub fn read<'look>(self, call: &'look Call<'call>) -> &'look str {
        let object = address(call.current(self.slot));
        // SAFETY: the reference passed the load barrier, so `object` is the text's current place, and it stays
        // there while `call` is borrowed: only making a value, which needs `call` mutably, can start a collection
        // on this thread, and PlainGC's own thread only moves objects a collection's pause chose, whose references
        // all fail the barrier.
        let bytes = unsafe {
            std::slice::from_raw_parts(object.add(PAYLOAD_OFFSET), payload_length(object))
        };
        std::str::from_utf8(bytes).unwrap_or_else(|_| {
            fail(AllocationError::Internal(
                "a `Text` held bytes that aren't UTF-8".to_string(),
            ))
        })
    }

    /// The text's colored reference, up to date, to give back to the compiled program.
    pub fn reference(self, call: &Call<'call>) -> u64 {
        call.current(self.slot)
    }

    /// Keeps the text beyond this call.
    pub fn keep(self, call: &Call<'call>) -> Kept<Text> {
        let slot = keep_root(call.current(self.slot)).unwrap_or_else(|error| fail(error));
        Kept {
            slot,
            _kind: PhantomData,
        }
    }
}

impl Kept<Text> {
    /// A handle to the kept text, for the rest of `call`.
    pub fn handle<'call>(&self, call: &mut Call<'call>) -> Handle<'call, Text> {
        let reference = kept_root(self.slot).unwrap_or_else(|error| fail(error));
        call.hold(reference)
    }
}

impl<T> Drop for Kept<T> {
    fn drop(&mut self) {
        release_root(self.slot);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{TEST_LOCK, collect_everything};

    #[test]
    fn d105_a_handle_finds_its_text_after_a_collection_moves_it() {
        let _alone = TEST_LOCK.lock();
        let read = with_call(|call| {
            let text = call.new_text("before the collection");
            let before = text.reference(call);
            assert!(collect_everything(&mut call.roots).is_ok());
            let after = text.reference(call);
            assert_ne!(address(before), address(after));
            text.read(call).to_string()
        });
        assert_eq!(read, "before the collection");
    }

    #[test]
    fn d105_a_kept_handle_survives_collections_between_calls() {
        let _alone = TEST_LOCK.lock();
        let kept = with_call(|call| call.new_text("kept").keep(call));
        for _ in 0..3 {
            with_call(|call| {
                call.new_text("garbage");
                assert!(collect_everything(&mut call.roots).is_ok());
            });
        }
        let read = with_call(|call| kept.handle(call).read(call).to_string());
        assert_eq!(read, "kept");
    }

    #[test]
    fn d105_a_dropped_kept_handle_is_no_longer_a_root() {
        let _alone = TEST_LOCK.lock();
        let kept = with_call(|call| call.new_text("let go").keep(call));
        let slot = kept.slot;
        drop(kept);
        let registered =
            crate::with_collector(|collector| Ok(collector.kept.get(slot).copied().flatten()));
        assert_eq!(registered, Ok(None));
    }
}
