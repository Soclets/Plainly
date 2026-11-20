//! The runtime: the Rust code compiled Plainly programs call, linked into every program (D45).
//!
//! `Text` is a pointer to memory from PlainGC (`plain_gc`): an 8-byte length, then that many bytes of UTF-8.
//!
//! Every message's wording here is a draft (D54), listed in `compiler/PROGRESS.md`.

use std::io::{BufRead, Write};
use std::ptr::NonNull;
use std::sync::atomic::{AtomicU64, Ordering};

// trace: D56
const STATUS_NOT_YET: i32 = 3;
const STATUS_CHECK_FAILED: i32 = 4;

/// The size of a text's length, written before its bytes.
const LENGTH_SIZE: usize = 8;

// ----- Messages -----

fn labelled_line(label: &str, text: &str) -> String {
    format!(" {:<9}{text}\n", format!("{label}:"))
}

/// Stops the program with a message in D54's layout. `site` is the start of the message, written out by the
/// compiler: the place twice, then the source line with `^` under the part in question.
fn stop(kind: &str, status: i32, site: &str, lines: &[(&str, String)]) -> ! {
    let _ = std::io::stdout().flush();
    let mut message = format!("{kind}: {site}");
    for (label, text) in lines {
        message.push_str(&labelled_line(label, text));
    }
    let _ = std::io::stderr().write_all(message.as_bytes());
    std::process::exit(status);
}

fn stop_not_designed_yet(site: &str, what: String, when: Option<String>) -> ! {
    let mut lines = vec![("what", what)];
    if let Some(when) = when {
        lines.push(("when", when));
    }
    stop("NotDesignedYet", STATUS_NOT_YET, site, &lines)
}

/// Stops when standard output can't be written; what should happen then isn't decided.
fn output_failed() -> ! {
    let message = format!(
        "NotDesignedYet: the program's output can't be written\n{}",
        labelled_line(
            "what",
            "what a program does when its output can't be written isn't decided"
        )
    );
    let _ = std::io::stderr().write_all(message.as_bytes());
    std::process::exit(STATUS_NOT_YET);
}

/// # Safety
///
/// `site` must point to `site_length` bytes of UTF-8 that stay valid while the program runs, as the compiler's
/// site data does.
unsafe fn site_text<'a>(site: *const u8, site_length: i64) -> &'a str {
    let length = usize::try_from(site_length).unwrap_or(0);
    if site.is_null() || length == 0 {
        return "";
    }
    // SAFETY: the caller promises `site` points to `site_length` bytes that live as long as the program.
    let bytes = unsafe { std::slice::from_raw_parts(site, length) };
    std::str::from_utf8(bytes).unwrap_or("")
}

// ----- Text -----

/// Makes new text through PlainGC, holding `bytes`.
fn new_text(bytes: &[u8]) -> *const u8 {
    let Some(memory) = plain_gc::allocate(LENGTH_SIZE + bytes.len()).map(NonNull::as_ptr) else {
        let message = format!(
            "NotDesignedYet: the program ran out of memory\n{}",
            labelled_line(
                "what",
                "what a program does when memory runs out isn't decided"
            )
        );
        let _ = std::io::stdout().flush();
        let _ = std::io::stderr().write_all(message.as_bytes());
        std::process::exit(STATUS_NOT_YET);
    };
    let length = bytes.len() as u64;
    // SAFETY: `memory` is a fresh allocation of `LENGTH_SIZE + bytes.len()` bytes, aligned to 8 (plain_gc's
    // ALIGNMENT), so the length fits at its start and the bytes right after it, and nothing else uses it yet.
    unsafe {
        memory.cast::<u64>().write(length);
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), memory.add(LENGTH_SIZE), bytes.len());
    }
    memory
}

/// # Safety
///
/// `text` must be a `Text` made by [`new_text`].
unsafe fn text_bytes<'a>(text: *const u8) -> &'a [u8] {
    if text.is_null() {
        return &[];
    }
    // SAFETY: the caller promises `text` came from `new_text`, which wrote its length at its start and that many
    // bytes after it. PlainGC never frees it, so it stays valid.
    unsafe {
        let length = text.cast::<u64>().read() as usize;
        std::slice::from_raw_parts(text.add(LENGTH_SIZE), length)
    }
}

/// Makes text from bytes in the compiled program's data, for text written in the program.
///
/// # Safety
///
/// `bytes` must point to `length` readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn plainly_text_from_bytes(bytes: *const u8, length: i64) -> *const u8 {
    let length = usize::try_from(length).unwrap_or(0);
    if bytes.is_null() || length == 0 {
        return new_text(&[]);
    }
    // SAFETY: the caller promises `bytes` points to `length` readable bytes.
    let slice = unsafe { std::slice::from_raw_parts(bytes, length) };
    new_text(slice)
}

/// `&`: new text holding `left`, then `right` (D28).
///
/// # Safety
///
/// Both must be `Text`s made by this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn plainly_text_join(left: *const u8, right: *const u8) -> *const u8 {
    // SAFETY: the caller promises both are texts from this runtime.
    let (left, right) = unsafe { (text_bytes(left), text_bytes(right)) };
    let mut joined = Vec::with_capacity(left.len() + right.len());
    joined.extend_from_slice(left);
    joined.extend_from_slice(right);
    new_text(&joined)
}

/// `be.Text` of a `Whole`: its digits, with `-` in front when it's negative (D22).
#[unsafe(no_mangle)]
pub extern "C" fn plainly_whole_to_text(value: i64) -> *const u8 {
    new_text(value.to_string().as_bytes())
}

/// `be.Whole` of text (D22, D95). Writes the number to `result` and gives 1, or gives 0 when the text isn't a
/// whole number that fits.
///
/// # Safety
///
/// `text` must be a `Text` made by this runtime, and `result` must point to 8 writable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn plainly_text_to_whole(text: *const u8, result: *mut i64) -> u8 {
    // SAFETY: the caller promises `text` is a text from this runtime.
    let value = read_whole(unsafe { text_bytes(text) });
    if !result.is_null() {
        // SAFETY: the caller promises `result` points to 8 writable bytes.
        unsafe { result.write(value.unwrap_or(0)) };
    }
    u8::from(value.is_some())
}

/// Reads a whole number as players type it (D95): spaces around it are ignored, and a `+` or `-` in front and
/// leading zeros are fine. Anything else, or a number too big or too small for a `Whole`, gives `None`, a failure.
fn read_whole(bytes: &[u8]) -> Option<i64> {
    // trace: D95
    let start = bytes.iter().position(|&byte| byte != b' ')?;
    let end = bytes.iter().rposition(|&byte| byte != b' ')? + 1;
    let number = &bytes[start..end];
    let digits = number
        .strip_prefix(b"-")
        .or_else(|| number.strip_prefix(b"+"))
        .unwrap_or(number);
    if digits.is_empty() || !digits.iter().all(u8::is_ascii_digit) {
        return None;
    }
    std::str::from_utf8(number).ok()?.parse().ok()
}

// ----- Showing and asking -----

fn write_output(bytes: &[u8], end_line: bool) {
    let mut output = std::io::stdout().lock();
    let written = output.write_all(bytes).and_then(|()| {
        if end_line {
            output.write_all(b"\n")
        } else {
            output.flush()
        }
    });
    if written.is_err() {
        output_failed();
    }
}

/// `show_msg`: shows text and ends the line (D6).
///
/// # Safety
///
/// `text` must be a `Text` made by this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn plainly_show_msg(text: *const u8) {
    // SAFETY: the caller promises `text` is a text from this runtime.
    write_output(unsafe { text_bytes(text) }, true);
}

/// `show_msg.same_line`: shows text and leaves the line open (D6).
///
/// # Safety
///
/// `text` must be a `Text` made by this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn plainly_show_msg_same_line(text: *const u8) {
    // SAFETY: the caller promises `text` is a text from this runtime.
    write_output(unsafe { text_bytes(text) }, false);
}

/// `ask`: shows the question, leaves the cursor on its line, waits for Enter, and gives back what was typed,
/// without the Enter (D20).
///
/// # Safety
///
/// `question` must be a `Text` made by this runtime, and `site` must point to `site_length` bytes of UTF-8 that
/// stay valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn plainly_ask(
    question: *const u8,
    site: *const u8,
    site_length: i64,
) -> *const u8 {
    // SAFETY: the caller promises `question` is a text from this runtime and `site` is valid.
    let (question, site) = unsafe { (text_bytes(question), site_text(site, site_length)) };
    write_output(question, false);
    let mut line = Vec::new();
    let read = std::io::stdin().lock().read_until(b'\n', &mut line);
    match read {
        Ok(0) => {
            stop_not_designed_yet(site, "running out of input isn't decided".to_string(), None)
        }
        Ok(_) if !line.ends_with(b"\n") => stop_not_designed_yet(
            site,
            "input ending without Enter after the last answer isn't decided".to_string(),
            None,
        ),
        Ok(_) => {}
        Err(_) => stop_not_designed_yet(
            site,
            "what happens when input can't be read isn't decided".to_string(),
            None,
        ),
    }
    line.pop();
    if line.ends_with(b"\r") {
        line.pop();
    }
    if std::str::from_utf8(&line).is_err() {
        stop_not_designed_yet(
            site,
            "typed input that isn't UTF-8 text isn't decided".to_string(),
            None,
        );
    }
    new_text(&line)
}

// ----- Randomness -----

/// The random number generator's state: wyrand, fast game randomness, not for secrets (D26).
static RANDOM_STATE: AtomicU64 = AtomicU64::new(0);

fn next_random() -> u64 {
    let state = RANDOM_STATE
        .fetch_add(0xa076_1d64_78bd_642f, Ordering::Relaxed)
        .wrapping_add(0xa076_1d64_78bd_642f);
    let product = u128::from(state) * u128::from(state ^ 0xe703_7ed1_a0b4_28db);
    ((product >> 64) as u64) ^ (product as u64)
}

/// `random(from:, to:)`: a whole number from `low` to `high`, both included, each equally likely (D26).
///
/// # Safety
///
/// `site` must point to `site_length` bytes of UTF-8 that stay valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn plainly_random(
    low: i64,
    high: i64,
    site: *const u8,
    site_length: i64,
) -> i64 {
    if low > high {
        // SAFETY: the caller promises `site` is valid.
        let site = unsafe { site_text(site, site_length) };
        stop_not_designed_yet(
            site,
            "`from:` above `to:` isn't decided".to_string(),
            Some(format!("from: {low}, to: {high}")),
        );
    }
    let span = (i128::from(high) - i128::from(low) + 1) as u128;
    if span > u128::from(u64::MAX) {
        return next_random() as i64;
    }
    let span = span as u64;
    // Lemire's method: multiply into 128 bits and reject the few results that would make some numbers likelier.
    let threshold = span.wrapping_neg() % span;
    loop {
        let product = u128::from(next_random()) * u128::from(span);
        if (product as u64) >= threshold {
            return (i128::from(low) + (product >> 64) as i128) as i64;
        }
    }
}

// ----- Checks -----

/// A `Whole` calculation overflowed: stops the program in `working` mode (D46). `operation` is `+`, `-`, `*`, or
/// `n` for negating `right`.
///
/// # Safety
///
/// `site` must point to `site_length` bytes of UTF-8 that stay valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn plainly_overflow(
    site: *const u8,
    site_length: i64,
    operation: u8,
    left: i64,
    right: i64,
) {
    // SAFETY: the caller promises `site` is valid.
    let site = unsafe { site_text(site, site_length) };
    let (left_wide, right_wide) = (i128::from(left), i128::from(right));
    let (exact, when) = match operation {
        b'+' => (left_wide + right_wide, format!("{left} + {right}")),
        b'-' => (left_wide - right_wide, format!("{left} - {right}")),
        b'*' => (left_wide * right_wide, format!("{left} * {right}")),
        _ => (-right_wide, format!("-({right})")),
    };
    let what = if exact > 0 {
        "the result is too big for a `Whole`"
    } else {
        "the result is too small for a `Whole`"
    };
    stop(
        "check failed",
        STATUS_CHECK_FAILED,
        site,
        &[
            ("what", what.to_string()),
            ("when", when),
            (
                "why",
                // trace: D92
                "a `Whole` holds whole numbers from -9223372036854775808 to 9223372036854775807."
                    .to_string(),
            ),
        ],
    );
}

// ----- Start and end -----

/// Runs before `proc.main`.
#[unsafe(no_mangle)]
pub extern "C" fn plainly_runtime_start() {
    // Seeding isn't decided (D26), so each run starts from the clock and the process.
    let nanoseconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(0);
    let seed = nanoseconds ^ (u64::from(std::process::id()) << 32);
    RANDOM_STATE.store(seed, Ordering::Relaxed);
}

/// Runs after `proc.main` ends.
#[unsafe(no_mangle)]
pub extern "C" fn plainly_runtime_end() {
    if std::io::stdout().flush().is_err() {
        output_failed();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(text: &str) -> Option<i64> {
        read_whole(text.as_bytes())
    }

    #[test]
    fn d95_reads_whole_numbers_as_players_type_them() {
        assert_eq!(read("41"), Some(41));
        assert_eq!(read(" 42"), Some(42));
        assert_eq!(read("42 "), Some(42));
        assert_eq!(read("-7"), Some(-7));
        assert_eq!(read("+5"), Some(5));
        assert_eq!(read("007"), Some(7));
        assert_eq!(read("-0"), Some(0));
        assert_eq!(read("9223372036854775807"), Some(i64::MAX));
        assert_eq!(read("-9223372036854775808"), Some(i64::MIN));
        for failing in [
            "abc",
            "",
            "   ",
            "4 2",
            "4.5",
            "1,000",
            "1_000",
            "+-5",
            "- 7",
            "\t42",
            "9223372036854775808",
            "-9223372036854775809",
        ] {
            assert_eq!(read(failing), None, "{failing:?}");
        }
    }

    #[test]
    fn d26_random_stays_in_range() {
        plainly_runtime_start();
        for _ in 0..1000 {
            // SAFETY: a null site is never read when `low` isn't above `high`.
            let number = unsafe { plainly_random(1, 6, std::ptr::null(), 0) };
            assert!((1..=6).contains(&number));
        }
        // SAFETY: as above.
        let anything = unsafe { plainly_random(i64::MIN, i64::MAX, std::ptr::null(), 0) };
        let _ = anything;
    }

    #[test]
    fn d28_joins_text() {
        let left = new_text(b"Hello, ");
        let right = new_text(b"Mia");
        // SAFETY: both texts come from `new_text`.
        let joined = unsafe { text_bytes(plainly_text_join(left, right)) };
        assert_eq!(joined, b"Hello, Mia");
    }
}
