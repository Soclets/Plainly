# PlainCompiler: progress

The state of PlainCompiler, kept current as it's built. A fresh session starts by reading this file, `git log`, and
the test results (`python3 compiler/run_tests.py`).

## Milestone 1: every test and the guessing game, in `working` mode

### Plan

Each step ends with something running.

1. **Test runner** (`compiler/run_tests.py`): builds the workspace, runs every test in `tests/` as `tests/README.md`
   says, compares standard output, standard error and exit status exactly, and runs each `check.py`.
2. **Hello world end to end:** the command `plainly` (`run`, `build`, `check`, `--working`), tokenizer, parser to the
   syntax tree, checker to the typed tree, lowering to Plainly IR, Cranelift to an object file, linking with the
   runtime (`plain_runtime`, Rust) and PlainGC's interface (`plain_gc`). `d04_hello_world` passes.
3. **Text:** `show_msg.same_line`, `&`, `\` escapes, `.fmt` (`d06`, `d28`, `d29`, `d30`).
4. **Whole and variables:** `let`, `var`, `=`, `+`, `-`, `*`, negatives, `be.Text`, overflow checks (`d13`).
5. **Control flow:** `if`, comparisons, `while`, `break`, `Bool`, `and`, `or`, `not` (`d16`, `d19`, `d34`).
6. **Procedures:** parameters with the shared-type shorthand, `returns`, `return`, labels, slides (`d05`, `d24`).
7. **Asking:** `ask`, `be.Whole`, `??` with a value (`d20`), `random`; the guessing game builds and plays.
8. **Finish:** `cargo fmt --check` and `cargo clippy` clean, this file up to date, committed and pushed.

### Status

Step 1 is next.

## The pieces

| Crate | What it is |
|---|---|
| `plain_compiler` | the `plainly` command and every stage of D62 |
| `plain_runtime` | the Rust code compiled programs call: `show_msg`, `ask`, `be.Whole`, `random`, messages |
| `plain_gc` | PlainGC's interface, with the simplest correct version behind it |

## Messages waiting for wording (D54)

None yet.

## Open questions for the design session

None yet.
