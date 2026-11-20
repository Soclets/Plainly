//! PlainCompiler (D51): the `plainly` command (D50).
//!
//! ```text
//! plainly run game.plainly --working
//! plainly build game.plainly --working
//! plainly check game.plainly
//! ```
//!
//! The stages follow D62: source, syntax tree (`parser`), typed tree (`checker`), Plainly IR (`lowering`), then
//! Cranelift IR and native code (`code_generation`), linked with the runtime (`linking`).

mod checker;
mod code_generation;
mod internal_error;
mod knock_ons;
mod linking;
mod lowering;
mod message;
mod parser;
mod plainly_ir;
mod source;
mod syntax_tree;
#[cfg(test)]
mod tests;
mod tokenizer;
mod typed_tree;

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use internal_error::InternalError;
use message::{Kind, Message};
use plainly_ir::IrProgram;
use source::SourceFile;

// trace: D56
const STATUS_RAN: u8 = 0;
const STATUS_REFUSED: u8 = 1;
const STATUS_UNREADABLE: u8 = 2;
const STATUS_NOT_YET: u8 = 3;
/// D56 has no status for PlainCompiler going wrong itself yet; this placeholder waits for the design session.
const STATUS_INTERNAL_ERROR: u8 = 70;
/// D56 has no status for a mistake in the command itself yet; this placeholder waits for the design session.
const STATUS_COMMAND_MISTAKE: u8 = 64;

const USAGE: &str = "usage:
  plainly run FILE.plainly --working     build in a temporary file and run it
  plainly build FILE.plainly --working   build the file to ship
  plainly check FILE.plainly             say what's refused, without building
";

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    ExitCode::from(run_command(&arguments))
}

/// What the command was asked to do.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Action {
    Run,
    Build,
    Check,
}

fn command_mistake(what: &str) -> u8 {
    eprint!("plainly: {what}\n{USAGE}");
    STATUS_COMMAND_MISTAKE
}

fn run_command(arguments: &[String]) -> u8 {
    let Some(command) = arguments.first() else {
        return command_mistake("no command given");
    };
    let action = match command.as_str() {
        "run" => Action::Run,
        "build" => Action::Build,
        "check" => Action::Check,
        "oracle" => {
            eprintln!(
                "NotBuiltYet: `plainly oracle` (D50) isn't built yet, since PlainOracle doesn't exist yet"
            );
            return STATUS_NOT_YET;
        }
        other => return command_mistake(&format!("`{other}` isn't a command")),
    };
    let mut file = None;
    let mut mode = None;
    for argument in &arguments[1..] {
        if argument.starts_with("--") {
            if mode.is_some() {
                return command_mistake("only one mode can be given");
            }
            mode = Some(argument.as_str());
        } else if file.is_some() {
            return command_mistake("only one file can be given");
        } else {
            file = Some(argument.as_str());
        }
    }
    let Some(file) = file else {
        return command_mistake("no file given");
    };
    // trace: D46, D50
    match (action, mode) {
        (Action::Check, None) => {}
        (Action::Check, Some(_)) => return command_mistake("`check` needs no mode"),
        (_, None) => {
            return command_mistake("`run` and `build` need a mode: `--working` or `--final`");
        }
        (_, Some("--working")) => {}
        (_, Some("--final")) => {
            eprintln!(
                "NotBuiltYet: `final` mode (D46) isn't built yet; build in `working` mode with `--working`"
            );
            return STATUS_NOT_YET;
        }
        (_, Some(other)) => {
            return command_mistake(&format!(
                "`{other}` isn't a mode; the modes are `--working` and `--final`"
            ));
        }
    }
    match compile_and_go(action, Path::new(file)) {
        Ok(status) => status,
        Err(error) => {
            eprint!("{}", error.render());
            STATUS_INTERNAL_ERROR
        }
    }
}

/// Reads the program: every `.plainly` file in the named file's folder (D35, D64).
fn read_program(file: &Path) -> Result<Vec<SourceFile>, u8> {
    let unreadable = |path: &Path, what: String| {
        eprint!(
            "can't read: {}\n{}",
            path.display(),
            message::labelled_line("what", &what)
        );
        STATUS_UNREADABLE
    };
    if file.extension().and_then(|extension| extension.to_str()) != Some("plainly") {
        return Err(unreadable(
            file,
            "a Plainly file's name ends in `.plainly`".to_string(),
        ));
    }
    if let Err(error) = fs::metadata(file) {
        return Err(unreadable(file, error.to_string()));
    }
    let folder = file.parent().unwrap_or(Path::new(""));
    let listing_folder = if folder.as_os_str().is_empty() {
        Path::new(".")
    } else {
        folder
    };
    let entries = fs::read_dir(listing_folder)
        .map_err(|error| unreadable(listing_folder, error.to_string()))?;
    let mut paths: Vec<PathBuf> = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| unreadable(listing_folder, error.to_string()))?;
        let name = entry.file_name();
        if Path::new(&name)
            .extension()
            .and_then(|extension| extension.to_str())
            == Some("plainly")
        {
            paths.push(folder.join(name));
        }
    }
    paths.sort();
    let mut sources = Vec::new();
    for path in paths {
        let bytes = fs::read(&path).map_err(|error| unreadable(&path, error.to_string()))?;
        let text = String::from_utf8(bytes)
            .map_err(|_| unreadable(&path, "it isn't UTF-8 text".to_string()))?;
        sources.push(SourceFile::new(path.display().to_string(), text));
    }
    Ok(sources)
}

/// Shows every message, in order, and gives the exit status: refused if anything was, otherwise `NotBuiltYet` or
/// `NotDesignedYet`.
fn show_messages(messages: &mut [Message], sources: &[SourceFile]) -> u8 {
    messages.sort_by_key(|message| message.place);
    let mut shown: Vec<String> = Vec::new();
    for message in messages.iter() {
        let text = message.render(sources);
        if !shown.contains(&text) {
            shown.push(text);
        }
    }
    let mut standard_error = std::io::stderr().lock();
    let _ = standard_error.write_all(shown.join("\n").as_bytes());
    if messages.iter().any(|message| message.kind == Kind::Refused) {
        STATUS_REFUSED
    } else {
        STATUS_NOT_YET
    }
}

/// The front end: source to the typed tree, with every message on the way (D62).
fn front_end(sources: &[SourceFile]) -> (Vec<Message>, Option<typed_tree::TypedProgram>) {
    let mut messages = Vec::new();
    let mut procedures = Vec::new();
    let mut stopped = false;
    for (index, source) in sources.iter().enumerate() {
        let tokens = tokenizer::tokenize(index, source, &mut messages);
        let parsed = parser::parse_file(index, tokens, &mut messages);
        stopped |= parsed.stopped;
        procedures.extend(parsed.procedures);
    }
    // When a broken opening line stopped checking, the rest would only give false errors (D55).
    if stopped {
        return (messages, None);
    }
    let typed = checker::check_program(&procedures, sources, &mut messages);
    (messages, typed)
}

fn compile_and_go(action: Action, file: &Path) -> Result<u8, InternalError> {
    let sources = match read_program(file) {
        Ok(sources) => sources,
        Err(status) => return Ok(status),
    };
    let (mut messages, typed) = front_end(&sources);
    if !messages.is_empty() {
        messages.sort_by_key(|message| message.place);
        knock_ons::sort_out(&mut messages, &sources, |fixed| front_end(fixed).0);
        return Ok(show_messages(&mut messages, &sources));
    }
    // `check` stops at the typed tree (D50, D62).
    if action == Action::Check {
        return Ok(STATUS_RAN);
    }
    let typed =
        typed.ok_or_else(|| InternalError::new("checking gave no typed tree, and no message"))?;
    let program: IrProgram = lowering::lower_program(&typed, &sources)?;
    let object = code_generation::generate(&program)?;
    match action {
        Action::Build => {
            let name = file
                .file_stem()
                .ok_or_else(|| InternalError::new("the file has no name"))?;
            linking::link(&object, Path::new(name))?;
            Ok(STATUS_RAN)
        }
        _ => run_in_temporary_file(&object),
    }
}

/// `plainly run`: builds to a temporary file, runs it, and gives its exit status (D45, D50).
fn run_in_temporary_file(object: &[u8]) -> Result<u8, InternalError> {
    let nanoseconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let folder = std::env::temp_dir().join(format!("plainly-{}-{nanoseconds}", std::process::id()));
    fs::create_dir_all(&folder)
        .map_err(|error| InternalError::new(format!("making a temporary folder: {error}")))?;
    let program = folder.join("program");
    let result = linking::link(object, &program).and_then(|()| {
        Command::new(&program)
            .status()
            .map_err(|error| InternalError::new(format!("starting the built program: {error}")))
    });
    let _ = fs::remove_dir_all(&folder);
    let status = result?;
    match status.code() {
        Some(code) => Ok(u8::try_from(code).unwrap_or(STATUS_INTERNAL_ERROR)),
        None => Err(InternalError::new(format!(
            "the built program was stopped from outside ({status})"
        ))),
    }
}
