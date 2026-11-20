# extern.rust tests

Programs with `extern.rust` procedures (D36, D105 to D112), each in its own folder with a real `rust` folder, run by
`compiler/run_tests.py` in the same way as the tests in `tests/`, normally and in collect-often mode. Building a
`rust` folder needs Cargo; the runner builds offline, so every crate a test uses must already be in Cargo's cache.

- `d107_rust_folder`: Text, Whole and Bool crossing both ways, a crate from crates.io (`heck`), failures caught by
  `??`, and a kept handle surviving collections between calls.
- `d107_no_rust_folder`: an `extern.rust` procedure with no `rust` folder is refused.
- `d110_header_mismatch`: Rust functions whose shape or failures don't match their headers, and one that's missing.
- `d111_rust_panic`: a panic in Rust stops the program as an internal error, saying where in the Rust (D112).
