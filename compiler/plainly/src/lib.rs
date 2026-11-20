//! The crate a Plainly program's `rust` project depends on (D107): the `procedure` attribute that marks a Rust
//! function as an `extern.rust` procedure's body (D108), and the handles it holds Plainly values with (D105).
//!
//! ```
//! use plainly::{Call, Handle, Text};
//!
//! #[plainly::procedure("shout")]
//! fn shout(call: &mut Call, text: Handle<Text>) -> Handle<Text> {
//!     let loud = text.read(call).to_uppercase();
//!     call.new_text(&loud)
//! }
//!
//! #[plainly::procedure("roll_over")]
//! fn roll_over(call: &mut Call, number: i64, up: bool) -> i64 {
//!     if up { number + 1 } else { number - 1 }
//! }
//! ```
//!
//! A procedure that can fail names its failures, as its Plainly header does, and gives back `Result<..., Failure>`
//! (D110):
//!
//! ```
//! use plainly::{Call, Handle, Text};
//!
//! #[plainly::procedure("first_word", fails = "empty, too_long")]
//! fn first_word(call: &mut Call, text: Handle<Text>) -> Result<Handle<Text>, Failure> {
//!     let word = text.read(call).split_whitespace().next().ok_or(Failure::Empty)?.to_string();
//!     if word.len() > 20 {
//!         return Err(Failure::TooLong);
//!     }
//!     Ok(call.new_text(&word))
//! }
//!
//! assert_eq!(plainly_failures_first_word::Failure::TooLong.number(), 2);
//! ```
//!
//! A parameter must be a `Handle<Text>`, an `i64` or a `bool` (D109), checked while building:
//!
//! ```compile_fail
//! #[plainly::procedure("halve")]
//! fn halve(call: &mut plainly::Call, number: f64) -> f64 {
//!     number / 2.0
//! }
//! ```

pub use plain_gc::handles::{Call, Handle, Kept, Text};
pub use plainly_macros::procedure;

/// What the `procedure` attribute's glue calls. Not for Rust writers to use directly.
#[doc(hidden)]
pub mod glue {
    pub use plain_gc::handles::with_call;

    use std::any::Any;
    use std::cell::{Cell, RefCell};
    use std::panic::AssertUnwindSafe;
    use std::sync::OnceLock;

    use crate::{Call, Handle, Text};

    /// A handle to a text Cranelift's code passed in.
    ///
    /// # Safety
    ///
    /// `raw` must be a live `Text`'s colored reference, as compiled code passes after its load barrier.
    pub unsafe fn adopt_text<'call>(call: &mut Call<'call>, raw: *const u8) -> Handle<'call, Text> {
        // SAFETY: passed on from this function's own promise.
        unsafe { call.adopt_text(raw as u64) }
    }

    /// A text's reference, to give back to Cranelift's code.
    pub fn give_back_text<'call>(call: &Call<'call>, text: Handle<'call, Text>) -> *const u8 {
        text.reference(call) as *const u8
    }

    /// What went wrong when an `extern.rust` procedure's Rust code panicked (D111, D112).
    pub struct RustWentWrong<'a> {
        /// The Plainly procedure's name.
        pub procedure: &'a str,
        /// Rust's own panic message.
        pub message: String,
        /// Where in the Rust the panic happened, as Rust records it: the file and the line.
        pub place: Option<(String, u32)>,
        /// The start of a message at the Plainly call, written out by PlainCompiler.
        pub site: *const u8,
        pub site_length: i64,
    }

    static HANDLER: OnceLock<fn(RustWentWrong) -> !> = OnceLock::new();

    thread_local! {
        /// Whether this thread is running an `extern.rust` procedure's Rust code, inside [`catch`].
        static IN_RUST: Cell<bool> = const { Cell::new(false) };
        /// Where the last panic inside [`catch`] happened.
        static PANIC_PLACE: RefCell<Option<(String, u32)>> = const { RefCell::new(None) };
    }

    /// Sets how the program stops when Rust code panics; the runtime sets it as the program starts. It also quiets
    /// Rust's own panic report inside an `extern.rust` procedure, keeping where the panic happened for the message.
    pub fn set_rust_went_wrong_handler(handler: fn(RustWentWrong) -> !) {
        if HANDLER.set(handler).is_err() {
            return;
        }
        let rust_report = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |information| {
            if IN_RUST.with(Cell::get) {
                let place = information
                    .location()
                    .map(|location| (location.file().to_string(), location.line()));
                PANIC_PLACE.with(|kept| *kept.borrow_mut() = place);
            } else {
                rust_report(information);
            }
        }));
    }

    /// Runs `work`, the Rust function, catching a panic at the edge, since unwinding into Cranelift's code would be
    /// undefined behaviour (D111).
    pub fn catch<R>(work: impl FnOnce() -> R) -> Result<R, Box<dyn Any + Send>> {
        let outer = IN_RUST.with(|inside| inside.replace(true));
        // A panic stops the program, so nothing broken by it is seen afterwards: asserting unwind safety is sound.
        let outcome = std::panic::catch_unwind(AssertUnwindSafe(work));
        IN_RUST.with(|inside| inside.set(outer));
        outcome
    }

    /// Stops the program because `procedure`'s Rust code panicked with `panic`, at the Plainly call `site` (D111).
    pub fn rust_went_wrong(
        procedure: &str,
        panic: Box<dyn Any + Send>,
        site: *const u8,
        site_length: i64,
    ) -> ! {
        let message = if let Some(text) = panic.downcast_ref::<&str>() {
            text.to_string()
        } else if let Some(text) = panic.downcast_ref::<String>() {
            text.clone()
        } else {
            "the Rust code panicked with a value that isn't text".to_string()
        };
        let place = PANIC_PLACE.with(|kept| kept.borrow_mut().take());
        let went_wrong = RustWentWrong {
            procedure,
            message,
            place,
            site,
            site_length,
        };
        match HANDLER.get() {
            Some(handler) => handler(went_wrong),
            None => {
                eprintln!(
                    "internal error: the Rust code of `{}` went wrong, not your Plainly code\n what:    {}",
                    went_wrong.procedure, went_wrong.message
                );
                // trace: D111
                std::process::exit(7)
            }
        }
    }
}
