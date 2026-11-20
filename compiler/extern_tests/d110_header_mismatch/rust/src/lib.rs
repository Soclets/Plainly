use plainly::{Call, Handle, Text};

#[plainly::procedure("first_word", fails = "empty, too_long")]
fn first_word(call: &mut Call, text: Handle<Text>) -> Result<Handle<Text>, Failure> {
    let word = text.read(call).split_whitespace().next().ok_or(Failure::Empty)?.to_string();
    if word.len() > 20 {
        return Err(Failure::TooLong);
    }
    Ok(call.new_text(&word))
}

#[plainly::procedure("count_words")]
fn count_words(call: &mut Call, text: Handle<Text>, at_least: i64) -> i64 {
    (text.read(call).split_whitespace().count() as i64).max(at_least)
}
