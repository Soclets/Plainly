//! Linking: the object file and the runtime become one native executable (D45).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::internal_error::InternalError;

/// The runtime library, `plain_runtime` with PlainGC inside, built beside the `plainly` command.
fn runtime_library() -> Result<PathBuf, InternalError> {
    let command = std::env::current_exe().map_err(|error| {
        InternalError::new(format!(
            "finding the `plainly` command's own folder: {error}"
        ))
    })?;
    let folder = command
        .parent()
        .ok_or_else(|| InternalError::new("the `plainly` command has no folder"))?;
    let library = folder.join("libplain_runtime.a");
    if !library.exists() {
        return Err(InternalError::new(format!(
            "the runtime library isn't at {}; build the whole workspace with `cargo build --workspace`",
            library.display()
        )));
    }
    Ok(library)
}

pub fn link(object: &[u8], output: &Path) -> Result<(), InternalError> {
    let runtime = runtime_library()?;
    let object_path = output.with_extension("o");
    fs::write(&object_path, object).map_err(|error| {
        InternalError::new(format!("writing {}: {error}", object_path.display()))
    })?;
    let mut command = Command::new("cc");
    command
        .arg(&object_path)
        .arg(&runtime)
        .arg("-o")
        .arg(output);
    if cfg!(target_os = "linux") {
        command.args(["-lpthread", "-ldl", "-lm"]);
    }
    let result = command.output();
    let _ = fs::remove_file(&object_path);
    let result = result.map_err(|error| {
        InternalError::new(format!(
            "running the C linker, `cc`, which compiling needs: {error}"
        ))
    })?;
    if !result.status.success() {
        return Err(InternalError::new(format!(
            "the C linker, `cc`, failed: {}",
            String::from_utf8_lossy(&result.stderr).trim()
        )));
    }
    Ok(())
}
