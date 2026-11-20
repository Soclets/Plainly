//! The Rust code for program.plainly's `extern.rust` procedures (D107).

use std::cell::RefCell;

use heck::ToShoutySnakeCase;
use plainly::{Call, Handle, Kept, Text};

#[plainly::procedure("shout")]
fn shout(call: &mut Call, text: Handle<Text>) -> Handle<Text> {
    let loud = text.read(call).to_shouty_snake_case();
    call.new_text(&loud)
}

#[plainly::procedure("add_bonus")]
fn add_bonus(call: &mut Call, score: i64, doubled: bool) -> i64 {
    let _ = call;
    if doubled { score * 2 + 1 } else { score + 1 }
}

#[plainly::procedure("first_word", fails = "empty, too_long")]
fn first_word(call: &mut Call, text: Handle<Text>) -> Result<Handle<Text>, Failure> {
    let word = text.read(call).split_whitespace().next().ok_or(Failure::Empty)?.to_string();
    if word.len() > 20 {
        return Err(Failure::TooLong);
    }
    Ok(call.new_text(&word))
}

thread_local! {
    static REMEMBERED: RefCell<Option<Kept<Text>>> = const { RefCell::new(None) };
}

#[plainly::procedure("remember")]
fn remember(call: &mut Call, text: Handle<Text>) {
    let kept = text.keep(call);
    REMEMBERED.with(|remembered| *remembered.borrow_mut() = Some(kept));
}

#[plainly::procedure("remembered")]
fn remembered(call: &mut Call) -> Handle<Text> {
    let kept = REMEMBERED.with(|remembered| remembered.borrow_mut().take());
    match kept {
        Some(kept) => kept.handle(call),
        None => call.new_text("nothing remembered"),
    }
}
