//! When PlainCompiler itself goes wrong, it says so and how to report it, instead of crashing.

pub struct InternalError {
    pub what: String,
}

impl InternalError {
    pub fn new(what: impl Into<String>) -> InternalError {
        InternalError { what: what.into() }
    }

    /// The message, in D54's layout as far as it fits, worded as D92 approved.
    pub fn render(&self) -> String {
        format!(
            "internal error: PlainCompiler itself went wrong, not your program\n{}{}",
            crate::message::labelled_line("what", &self.what),
            crate::message::labelled_line(
                "why",
                // trace: D92
                "this is a bug in PlainCompiler. Please report it at https://github.com/Soclets/Plainly/issues, \
                 with the program that caused it."
            ),
        )
    }
}
