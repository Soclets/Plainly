//! The runtime: the Rust code compiled Plainly programs call, linked into every program (D45).
//!
//! `Text` is a pointer to memory from PlainGC (`plain_gc`): an 8-byte length, then that many bytes of UTF-8.
//!
//! Every message's wording here is a draft (D54), listed in `compiler/PROGRESS.md`.

use std::io::{BufRead, IsTerminal, Write};

use plain_gc::handles::{Call, Handle, Text};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

// trace: D56
const STATUS_NOT_YET: i32 = 3;
const STATUS_CHECK_FAILED: i32 = 4;

const STATUS_INTERNAL_ERROR: i32 = 7;

// ----- Messages -----

fn labelled_line(label: &str, text: &str) -> String {
    format!(" {:<9}{text}\n", format!("{label}:"))
}

/// Whether the program's output has a line still open, as after `show_msg.same_line` or `ask`'s question.
static LINE_OPEN: AtomicBool = AtomicBool::new(false);

/// Before a runtime message: ends an open line on standard output, so the message never shares a line with the
/// program's own output (D104).
fn before_message() {
    // trace: D104
    let mut output = std::io::stdout().lock();
    if LINE_OPEN.swap(false, Ordering::SeqCst) {
        let _ = output.write_all(b"\n");
    }
    let _ = output.flush();
}

/// Stops the program with a message in D54's layout. `site` is the start of the message, written out by the
/// compiler: the place twice, then the source line with `^` under the part in question.
fn stop(kind: &str, status: i32, site: &str, lines: &[(&str, String)]) -> ! {
    before_message();
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
            "what a program does when its output can't be written isn't decided yet"
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

/// Stops the program because PlainGC or the code PlainCompiler made went wrong (D82), worded as D103 approved.
fn internal_error(what: &str) -> ! {
    before_message();
    let message = format!(
        "internal error: PlainGC itself went wrong, not your program\n{}{}",
        labelled_line("what", what),
        labelled_line(
            "why",
            "this is a bug in PlainGC. Please report it at https://github.com/Soclets/Plainly/issues, with the \
             program that caused it."
        )
    );
    let _ = std::io::stderr().write_all(message.as_bytes());
    // trace: D82
    std::process::exit(STATUS_INTERNAL_ERROR);
}

/// Stops the program because an `extern.rust` procedure's Rust code panicked (D111), saying where in the Rust and
/// what to do (D112).
fn rust_went_wrong(went_wrong: plainly::glue::RustWentWrong) -> ! {
    // SAFETY: the glue passes the site PlainCompiler wrote for the call, which lives as long as the program.
    let site = unsafe { site_text(went_wrong.site, went_wrong.site_length) };
    // The site starts with the place; D111's message shows only the snippet under its own first line.
    let snippet = site.split_once('\n').map_or("", |(_, snippet)| snippet);
    let procedure = went_wrong.procedure;
    let place = match &went_wrong.place {
        Some((file, line)) => format!("at {file} line {line}"),
        None => "in the program's `rust` folder".to_string(),
    };
    before_message();
    let message = format!(
        "internal error: the Rust code of `{procedure}` went wrong, not your Plainly code\n{snippet}{}{}",
        labelled_line("what", &went_wrong.message),
        labelled_line(
            "why",
            &format!(
                "this is a bug in `{procedure}`'s Rust code, {place}; fix it there, or report it to whoever wrote it."
            )
        )
    );
    let _ = std::io::stderr().write_all(message.as_bytes());
    // trace: D111, D112
    std::process::exit(STATUS_INTERNAL_ERROR);
}

/// Stops the program when PlainGC can't go on: out of memory, or a bug in PlainGC itself.
fn gc_failed(error: plain_gc::AllocationError) -> ! {
    match error {
        plain_gc::AllocationError::OutOfMemory => {
            let message = format!(
                "NotDesignedYet: the program ran out of memory\n{}",
                labelled_line(
                    "what",
                    "what a program does when memory runs out isn't decided yet"
                )
            );
            before_message();
            let _ = std::io::stderr().write_all(message.as_bytes());
            std::process::exit(STATUS_NOT_YET);
        }
        plain_gc::AllocationError::Internal(what) => internal_error(&what),
    }
}

/// Runs `work` as one call holding PlainGC handles (D105), the way `extern.rust` code does: Rust's borrow checker
/// makes sure no look at a text is held while one is made, since making one can move every object. If PlainGC fails,
/// [`gc_failed`], installed at start, stops the program.
fn with_handles<R>(work: impl for<'call> FnOnce(&mut Call<'call>) -> R) -> R {
    // trace: D105
    plain_gc::handles::with_call(work)
}

/// A handle to a text compiled code passed in.
///
/// # Safety
///
/// `text` must be the colored reference to a live `Text`, as compiled code passes after its load barrier.
unsafe fn adopt<'call>(call: &mut Call<'call>, text: *const u8) -> Handle<'call, Text> {
    // SAFETY: passed on from this function's own promise.
    unsafe { call.adopt_text(text as u64) }
}

/// Makes new text, holding `text`, and gives the compiled program its colored reference (D69).
fn new_text(text: &str) -> *const u8 {
    with_handles(|call| call.new_text(text).reference(call)) as *const u8
}

/// The load barrier's slow path, which compiled code calls for a reference of a bad color (D69).
#[unsafe(no_mangle)]
pub extern "C" fn plainly_gc_heal(reference: *const u8) -> *const u8 {
    match plain_gc::heal(reference as u64) {
        Ok(healed) => healed as *const u8,
        Err(error) => gc_failed(error),
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
        return new_text("");
    }
    // SAFETY: the caller promises `bytes` points to `length` readable bytes.
    let slice = unsafe { std::slice::from_raw_parts(bytes, length) };
    match std::str::from_utf8(slice) {
        Ok(text) => new_text(text),
        Err(_) => internal_error("text written in the program isn't UTF-8"),
    }
}

/// `&`: new text holding `left`, then `right` (D28).
///
/// # Safety
///
/// Both must be `Text`s made by this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn plainly_text_join(left: *const u8, right: *const u8) -> *const u8 {
    with_handles(|call| {
        // SAFETY: the caller promises both are live texts, as compiled code passes them.
        let (left, right) = unsafe { (adopt(call, left), adopt(call, right)) };
        // Both looks end here, before making the joined text can move anything.
        let joined = format!("{}{}", left.read(call), right.read(call));
        call.new_text(&joined).reference(call)
    }) as *const u8
}

/// `be.Text` of a `Whole`: its digits, with `-` in front when it's negative (D22).
#[unsafe(no_mangle)]
pub extern "C" fn plainly_whole_to_text(value: i64) -> *const u8 {
    new_text(&value.to_string())
}

/// `be.Whole` of text (D22, D95). Writes the number to `result` and gives 1, or gives 0 when the text isn't a
/// whole number that fits.
///
/// # Safety
///
/// `text` must be a `Text` made by this runtime, and `result` must point to 8 writable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn plainly_text_to_whole(text: *const u8, result: *mut i64) -> u8 {
    // SAFETY: the caller promises `text` is a live text, as compiled code passes it.
    let value = with_handles(|call| read_whole(unsafe { adopt(call, text) }.read(call).as_bytes()));
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
    if end_line {
        LINE_OPEN.store(false, Ordering::SeqCst);
    } else if !bytes.is_empty() {
        LINE_OPEN.store(true, Ordering::SeqCst);
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
    with_handles(|call| {
        // SAFETY: the caller promises `text` is a live text, as compiled code passes it.
        write_output(unsafe { adopt(call, text) }.read(call).as_bytes(), true);
    });
}

/// `show_msg.same_line`: shows text and leaves the line open (D6).
///
/// # Safety
///
/// `text` must be a `Text` made by this runtime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn plainly_show_msg_same_line(text: *const u8) {
    // SAFETY: the caller promises `text` is a text from this runtime.
    with_handles(|call| {
        // SAFETY: the caller promises `text` is a live text, as compiled code passes it.
        write_output(unsafe { adopt(call, text) }.read(call).as_bytes(), false);
    });
}

/// `ask`: shows the question, leaves the cursor on its line, waits for Enter, and gives back what was typed,
/// without the Enter (D20). When input has run out, the program ends quietly (D96).
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
    // SAFETY: the caller promises `site` is valid.
    let site = unsafe { site_text(site, site_length) };
    with_handles(|call| {
        // SAFETY: the caller promises `question` is a live text, as compiled code passes it.
        write_output(
            unsafe { adopt(call, question) }.read(call).as_bytes(),
            false,
        );
    });
    let mut line = Vec::new();
    let read = std::io::stdin().lock().read_until(b'\n', &mut line);
    match read {
        // No more input: the program ends right here, quietly, with status 0 (D96).
        // trace: D96
        Ok(0) => {
            if std::io::stdout().flush().is_err() {
                output_failed();
            }
            std::process::exit(0);
        }
        Ok(_) => {}
        Err(_) => stop_not_designed_yet(
            site,
            "what happens when input can't be read isn't decided yet".to_string(),
            None,
        ),
    }
    // A last line with no Enter after it still counts as an answer (D96).
    if line.ends_with(b"\n") {
        // Typed at a terminal, the player's Enter has already ended the line on screen (D106).
        // trace: D106
        if std::io::stdin().is_terminal() {
            LINE_OPEN.store(false, Ordering::SeqCst);
        }
        line.pop();
        if line.ends_with(b"\r") {
            line.pop();
        }
    }
    match std::str::from_utf8(&line) {
        Ok(text) => new_text(text),
        Err(_) => stop_not_designed_yet(
            site,
            "typed input that isn't UTF-8 text isn't decided yet".to_string(),
            None,
        ),
    }
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
            "`from:` above `to:` isn't decided yet".to_string(),
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

/// Runs before `proc.main`: starts PlainGC with the program's stack map table and the frame pointer of its `main`.
///
/// # Safety
///
/// `stack_maps` must point to the stack map table PlainCompiler wrote into this program, and `main_frame` must be
/// the frame pointer of its `main`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn plainly_runtime_start(stack_maps: *const u64, main_frame: usize) {
    // How a program stops when PlainGC fails inside a call that holds handles (D105).
    plain_gc::set_failure_handler(gc_failed);
    // How a program stops when an `extern.rust` procedure's Rust code panics (D111).
    plainly::glue::set_rust_went_wrong_handler(rust_went_wrong);
    // SAFETY: passed on from this function's own promise, which the `main` PlainCompiler writes keeps.
    if let Err(error) = unsafe { plain_gc::start(stack_maps, main_frame) } {
        match error {
            plain_gc::AllocationError::Internal(what) => internal_error(&what),
            plain_gc::AllocationError::OutOfMemory => {
                internal_error("PlainGC ran out of memory while starting")
            }
        }
    }
    seed_random();
}

fn seed_random() {
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
    if std::env::var_os("PLAINGC_REPORT").is_some_and(|value| value == "1")
        && let Some(report) = plain_gc::report()
    {
        let _ = writeln!(
            std::io::stderr(),
            "PlainGC: {} collections, {} objects moved, {} references healed, {} objects moved by the barrier, \
             {} batches on its own thread, longest pause {} microseconds",
            report.collections,
            report.moved_objects,
            report.healed_references,
            report.relocated_by_barrier,
            report.worker_batches,
            report.longest_pause_microseconds
        );
    }
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
        seed_random();
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
        let left = new_text("Hello, ");
        let right = new_text("Mia");
        // SAFETY: both texts come from `new_text`.
        let joined = unsafe { plainly_text_join(left, right) };
        // SAFETY: as above.
        let read = with_handles(|call| unsafe { adopt(call, joined) }.read(call).to_string());
        assert_eq!(read, "Hello, Mia");
    }
}
