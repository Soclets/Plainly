//! Building a program's `rust` folder (D107): the Cargo project holding its `extern.rust` procedures' Rust code.
//!
//! PlainCompiler writes a small Cargo project of its own beside the program's, in `rust/target/plainly_program`. It
//! builds one static library holding the runtime, PlainGC and the program's Rust code, which is linked with the
//! compiled Plainly code. The program's crate depends on `plainly` (D108), which is patched to PlainCompiler's own
//! copy, so the handles and the runtime are the same ones. Before linking, every `extern.rust` procedure's symbol is
//! looked for in the library: the symbol spells out the procedure's shape, so a Rust function that doesn't match its
//! header is found here, with a message, rather than as a linker error (D110).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::internal_error::InternalError;
use crate::message::Message;
use crate::source::Place;

/// An `extern.rust` procedure, as linking needs it.
pub struct ExternalProcedure {
    pub name: String,
    /// The symbol its Rust function's glue has (D108).
    pub symbol: String,
    pub header_place: Place,
}

/// What building the `rust` folder came to.
pub enum Built {
    /// The static library to link instead of the runtime's own.
    Library(PathBuf),
    /// The program is refused, with these messages.
    Refused(Vec<Message>),
}

/// Where PlainCompiler's own crates are: `plain_runtime`, `plain_gc` and `plainly`.
// How Plainly is installed isn't decided, so for now the crates are found where PlainCompiler was built from.
fn compiler_folder() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// The `name` in a Cargo.toml's `[package]` table.
fn package_name(manifest: &str) -> Option<String> {
    let mut in_package = false;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_package = line == "[package]";
        } else if in_package
            && let Some((key, value)) = line.split_once('=')
            && key.trim() == "name"
        {
            return Some(value.trim().trim_matches('"').to_string());
        }
    }
    None
}

/// A path as a TOML string.
fn toml_path(path: &Path) -> String {
    format!("{:?}", path.display().to_string())
}

/// Builds the `rust` folder beside `program_file` for the procedures in `externals`.
pub fn build(program_file: &Path, externals: &[ExternalProcedure]) -> Result<Built, InternalError> {
    let Some(first) = externals.first() else {
        return Err(InternalError::new(
            "building a `rust` folder with no `extern.rust` procedure",
        ));
    };
    let program_folder = program_file
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default();
    let program_folder = if program_folder.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        program_folder
    };
    let program_folder = program_folder
        .canonicalize()
        .map_err(|error| InternalError::new(format!("finding the program's folder: {error}")))?;
    let rust_folder = program_folder.join("rust");
    let manifest_path = rust_folder.join("Cargo.toml");

    // trace: D107
    let Ok(manifest) = fs::read_to_string(&manifest_path) else {
        let what = if rust_folder.is_dir() {
            format!(
                "`{}` is `extern.rust`, but the `rust` folder has no Cargo.toml",
                first.name
            )
        } else {
            format!(
                "`{}` is `extern.rust`, but there's no `rust` folder beside the program",
                first.name
            )
        };
        return Ok(Built::Refused(vec![Message::refused(first.header_place, what).why(
            "an `extern.rust` procedure's body is Rust code, in a Cargo project in a folder named `rust` beside the \
             program's .plainly files.",
        )]));
    };
    let Some(package) = package_name(&manifest) else {
        return Ok(Built::Refused(vec![Message::refused(
            first.header_place,
            "the `rust` folder's Cargo.toml has no package name",
        )
        .why("PlainCompiler links the package the `rust` folder's Cargo.toml names, under `[package]`.")]));
    };

    let target = rust_folder.join("target");
    let glue_folder = target.join("plainly_program");
    fs::create_dir_all(&glue_folder).map_err(|error| {
        InternalError::new(format!("making {}: {error}", glue_folder.display()))
    })?;
    let compiler = compiler_folder();
    let glue_manifest = format!(
        "# Written by PlainCompiler to link the program's Rust code with its runtime. Changes are overwritten.\n\
         [package]\n\
         name = \"plainly_program\"\n\
         version = \"0.0.0\"\n\
         edition = \"2024\"\n\
         \n\
         [lib]\n\
         crate-type = [\"staticlib\"]\n\
         path = \"lib.rs\"\n\
         \n\
         [dependencies]\n\
         plain_runtime = {{ path = {runtime} }}\n\
         program_rust = {{ path = {rust}, package = {package:?} }}\n\
         \n\
         [patch.crates-io]\n\
         plainly = {{ path = {plainly} }}\n\
         plain_gc = {{ path = {plain_gc} }}\n\
         \n\
         [workspace]\n",
        runtime = toml_path(&compiler.join("plain_runtime")),
        rust = toml_path(&rust_folder),
        plainly = toml_path(&compiler.join("plainly")),
        plain_gc = toml_path(&compiler.join("plain_gc")),
    );
    let glue_source = "// Written by PlainCompiler: links the runtime and the program's Rust code into one library.\n\
                       extern crate plain_runtime;\n\
                       extern crate program_rust;\n";
    for (name, text) in [
        ("Cargo.toml", glue_manifest.as_str()),
        ("lib.rs", glue_source),
    ] {
        let path = glue_folder.join(name);
        if fs::read_to_string(&path).ok().as_deref() != Some(text) {
            fs::write(&path, text).map_err(|error| {
                InternalError::new(format!("writing {}: {error}", path.display()))
            })?;
        }
    }

    // Frame pointers let PlainGC walk the stack (D69); the remapping makes a panic's place read `rust/src/lib.rs`
    // (D112). Cargo's encoded form keeps a folder name with spaces in it whole.
    let flags = [
        "-C".to_string(),
        "force-frame-pointers=yes".to_string(),
        format!("--remap-path-prefix={}/=", program_folder.display()),
    ]
    .join("\u{1f}");
    let result = Command::new("cargo")
        .arg("build")
        .arg("--quiet")
        .arg("--manifest-path")
        .arg(glue_folder.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", &target)
        .env("CARGO_ENCODED_RUSTFLAGS", flags)
        .env_remove("RUSTFLAGS")
        .output();
    let result = match result {
        Ok(result) => result,
        Err(_) => {
            return Ok(Built::Refused(vec![Message::refused(
                first.header_place,
                "building the `rust` folder needs Cargo, Rust's build tool, which isn't installed",
            )
            .why("Rust is needed for a program with a `rust` folder; it's installed from https://rustup.rs.")]));
        }
    };
    if !result.status.success() {
        let mut standard_error = std::io::stderr().lock();
        let _ = std::io::Write::write_all(&mut standard_error, &result.stderr);
        let _ = std::io::Write::write_all(&mut standard_error, b"\n");
        return Ok(Built::Refused(vec![
            Message::refused(
                first.header_place,
                "the Rust code in the `rust` folder doesn't build",
            )
            .why("Cargo's messages above say what's wrong in the Rust code."),
        ]));
    }

    let library = target.join("debug").join("libplainly_program.a");
    let contents = fs::read(&library)
        .map_err(|error| InternalError::new(format!("reading {}: {error}", library.display())))?;
    let messages: Vec<Message> = externals
        .iter()
        .filter_map(|procedure| mismatch(procedure, &contents))
        .collect();
    if messages.is_empty() {
        Ok(Built::Library(library))
    } else {
        Ok(Built::Refused(messages))
    }
}

/// Whether `bytes` holds `needle`.
fn find(bytes: &[u8], needle: &[u8]) -> Option<usize> {
    bytes
        .windows(needle.len())
        .position(|window| window == needle)
}

/// A message when no Rust function in the library is the body of `procedure`, or the one there has another shape.
fn mismatch(procedure: &ExternalProcedure, library: &[u8]) -> Option<Message> {
    // trace: D108, D110
    let exact = format!("{}\0", procedure.symbol);
    if find(library, exact.as_bytes()).is_some() {
        return None;
    }
    let prefix = format!(
        "plainly_procedure_{}_takes_",
        procedure.name.replace('.', "__")
    );
    let Some(start) = find(library, prefix.as_bytes()) else {
        return Some(
            Message::refused(
                procedure.header_place,
                format!(
                    "no Rust function in the `rust` folder is marked `#[plainly::procedure(\"{}\")]`",
                    procedure.name
                ),
            )
            .why("an `extern.rust` procedure's body is the Rust function marked with its name."),
        );
    };
    let found = &library[start..];
    let end = found
        .iter()
        .position(|&byte| byte == 0)
        .unwrap_or(found.len());
    let found = String::from_utf8_lossy(&found[prefix.len()..end]).to_string();
    let wanted = procedure
        .symbol
        .get(prefix.len()..)
        .unwrap_or_default()
        .to_string();
    Some(
        Message::refused(
            procedure.header_place,
            format!(
                "the Rust function for `{}` {}, but this header {}",
                procedure.name,
                shape_words(&found),
                shape_words(&wanted)
            ),
        )
        .why(
            "a Rust function takes and gives what its header says, with `Handle<Text>` for `Text`, `i64` for \
             `Whole` and `bool` for `Bool`, and names the same failures.",
        ),
    )
}

/// A symbol's shape in words: `text_whole_gives_bool_fails_a__b` as "takes a `Text` and a `Whole`, gives back a
/// `Bool`, and can fail with `a` or `b`".
fn shape_words(shape: &str) -> String {
    let (takes, rest) = shape.split_once("_gives_").unwrap_or((shape, "nothing"));
    let (gives, fails) = match rest.split_once("_fails_") {
        Some((gives, fails)) => (gives, Some(fails)),
        None => (rest, None),
    };
    let kind = |word: &str| match word {
        "text" => "a `Text`",
        "whole" => "a `Whole`",
        "bool" => "a `Bool`",
        _ => "nothing",
    };
    let takes = if takes == "nothing" {
        "takes nothing".to_string()
    } else {
        let kinds: Vec<&str> = takes.split('_').map(kind).collect();
        format!("takes {}", join_words(&kinds, "and"))
    };
    let gives = format!("gives back {}", kind(gives));
    match fails {
        None => format!("{takes} and {gives}"),
        Some(fails) => {
            let names: Vec<String> = fails.split("__").map(|name| format!("`{name}`")).collect();
            let names: Vec<&str> = names.iter().map(String::as_str).collect();
            format!(
                "{takes}, {gives}, and can fail with {}",
                join_words(&names, "or")
            )
        }
    }
}

/// `a`, `a and b`, `a, b and c`.
fn join_words(words: &[&str], last: &str) -> String {
    match words {
        [] => String::new(),
        [only] => only.to_string(),
        [rest @ .., end] => format!("{} {last} {end}", rest.join(", ")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn d108_a_package_name_is_read_from_its_table() {
        let manifest =
            "[workspace]\nname = \"not this\"\n[package]\nname = \"dice\"\nversion = \"0.1.0\"\n";
        assert_eq!(package_name(manifest), Some("dice".to_string()));
    }

    #[test]
    fn d110_a_shape_is_put_in_words() {
        assert_eq!(
            shape_words("text_whole_gives_bool_fails_empty__too_long"),
            "takes a `Text` and a `Whole`, gives back a `Bool`, and can fail with `empty` or `too_long`"
        );
        assert_eq!(
            shape_words("nothing_gives_nothing"),
            "takes nothing and gives back nothing"
        );
    }
}
