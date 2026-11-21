# `Unlo\n`: progress

Where the compiler stands, kept current so a fresh context can carry on from here and `git log`. The live test results
are in `compiler/test-status.txt`, written by every test run.

## One command

```
cd compiler && cmake --workflow --preset check
```

configures, builds (3 cores, AddressSanitizer and UndefinedBehaviorSanitizer, libc++ hardened, `clang-tidy`'s checks
as errors), checks formatting, runs the unit tests on Unio IR, then every test in `tests/` in both modes, and writes
`compiler/test-status.txt`. `cmake --workflow --preset fast` does the same without `clang-tidy`, for quick rounds.
`compiler/build/dev/unit_tests --print FILE [--final]` prints a program's Unio IR, before or after `final`'s passes.

## Tools, pinned

- **LLVM 23.1.2**, the self-contained static build at `~/llvm-self-contained/install` (built without RTTI or
  exceptions, targets AArch64, ARM and X86). It never changes under us, unlike Homebrew's, which `brew upgrade` can
  move. The compiler links it statically.
- **Homebrew's LLVM 23.1.2** (`/opt/homebrew/opt/llvm@23`) for `clang++`, `clang-format`, `clang-tidy` and the
  sanitizers' runtime: the same version, and the compiler the static build was made with.
- **Apple's `ld`** links programs. lld 23 can't read the macOS 27 SDK's library lists (their `arm64e.x1` targets are
  newer than it), and Apple's linker always matches its own SDK. Programs target macOS 14 and up.
- CMake 4.4.3, Ninja. Nothing installed.
- Ralph, run as a black box: `oracle/_build/default/shell/ralph.exe PROGRAM.unio` after
  `opam exec --switch=ralph -- dune build --root oracle`. `unio oracle FILE` finds and runs it.

## How it's built

`source → syntax tree → typed tree → Unio IR → LLVM IR → native code` (D62), in `compiler/source/`:

- `source`, `lexer`, `parser`, `checker`: reading and refusing, with D54's layout (`messages`, `layout.h`, which the
  runtime shares).
- `lower`: the typed tree to Unio IR (`ir`), SSA with block parameters, built by Braun et al.'s algorithm. Every check
  is an instruction with its site; every text has its owners written out as `retain` and `release` (D130).
- `optimize`: `final`'s own passes on Unio IR (D124), below.
- `codegen`: one LLVM module per procedure. Checks are LLVM's overflow intrinsics with a cold, unlikely branch to the
  runtime's message.
- `backend`: compiles the procedures in parallel on every core (D65; the test runner sets `UNIO_THREADS=1`, since it
  runs 3 tests at once, rule 6), each cached in `~/Library/Caches/unio/objects` under a hash of its unoptimized LLVM
  IR, the mode and the compiler's own file, then links with Apple's `ld`.
- `runtime/runtime.cpp`: showing, asking, texts, the 30% memory cap, `be.Whole`, `random`, and runtime messages. It
  uses only the C library. In the sanitized test build it's sanitized too, so every test's program runs under ASan.

## `final`'s optimizations for this subset (D124)

| # | What | Where | Unit test |
|---|---|---|---|
| 1 | LLVM's full optimizing (O3) | `backend.cpp` | (LLVM's) |
| 1 | folding constants, simplifying, sweeping | `passes::simplify` | `test_folding` |
| 2, 8 | removing checks ranges and `if`s prove can't fail | `passes::remove_checks`, and LLVM's CVP | `test_removing_checks` |
| 3 | inlining small procedures | `passes::inline_small`, and LLVM's inliner | `test_inlining` |
| 6 | finding procedures with no side effects | `passes::find_side_effects` | `test_finding_side_effects` |
| 6 | the fib trick: inlining into itself where calls would repeat, then merging | `passes::inline_into_itself`, `duplicate_returns`, `merge_repeated` | `test_inlining_into_itself`, `test_merging_repeated` |
| 7 | recursion into loops | `passes::recursion_into_loops`, and LLVM's TailCallElim | `test_recursion_into_loops` |
| 9 | constants across procedures | `passes::constants_across` | `test_constants_across` |
| 13 | check-failure code out of the way | `codegen.cpp` (unlikely branches, `cold` `noreturn` stops), LLVM's HotColdSplitting | none yet |

The fib trick is general: a procedure with no side effects is inlined into itself at a call only where the copy's own
calls, with that call's arguments put in, repeat a call already there (arguments compared as a parameter plus a
constant). It applies to `fib`, to tribonacci-shaped procedures, and to nothing that repeats nothing.

Recursion into loops "even with a running total" (D124 #7) is done only for real tail calls: reassociating a checked
running total can change which check fails (`MAX + (1 + -1)` fits, `(MAX + 1) + -1` doesn't), which D46 forbids.

## Timings

`fib(44)` on Tankun's Mac, 2026-11-21, best of several runs, the number read from standard input as the C version reads
it from its arguments, so nothing's known at compile time:

| | time |
|---|---|
| Unio `final` | 2.1 to 2.3 ms |
| gfortran 16.2 `-O3` | 2.8 ms |
| clang `-O3` (C) | 965 to 999 ms |
| clang `-O0` (C) | 1757 ms |
| Unio `working` | 1946 to 2058 ms |
| gfortran `-O0` | 2711 ms |

Compiling hello world: 0.09 s in the sanitized debug build. Running an unchanged program again: 0.10 s, since the linked
program is cached (`~/Library/Caches/unio/programs`); a new program's first launch costs about 0.3 s more, macOS
checking it.

## Status

The first milestone is reached (2026-11-21):

1. `unio run`, `compile`, `check` and `oracle` work (D50, D91, D136, D150); `oracle` finds Ralph's built program.
2. Every test passes in both modes: 33 tests, 74 runs with the `check.py` files.
3. The guessing game plays in both modes, through a real terminal (a pty) and piped, matching Ralph when input runs
   out. Tankun hasn't played it by hand yet.
4. `final`'s optimizations for the subset are in, each from a general method, with unit tests (table above).
5. Compiling is parallel across procedures and cached, objects and linked programs both.
6. `fib` timed above.

Also checked: `clang-tidy` clean; 66 of 69 small refusal programs and all 21 spacing cases give Ralph's messages byte for
byte (the other 3 are refusals Ralph hasn't built, and Ralph's extra `NotDesignedYet` after `show_msg;`); 12 runtime
programs match Ralph in both modes; 2,500 randomly broken programs never crash `unio check`.

Not built, waiting on decisions: `?? crash` (D152 names the failure, and `be.Whole`'s failure has no name yet).

What comes next is Tankun's call.

## Questions sent to the design session

1. (Answered: Ralph's recursion crash is fixed; D150, D151, D152 and D153 decided.)
2. Still provisional: `random` with `from:` above `to:` stops at runtime with `NotDesignedYet`, status 3.
4. What `be.Whole`'s failure is called, for D152's `?? crash` message.
5. Standard chapter 10 says `run` compiles to a temporary file; `Unlo\n` runs the linked program from the cache.
3. Wordings drafted, not decided: "this line isn't inside a procedure"; "there's no type named `X`"; "`not` works on
   a `Bool`, but this is a `Whole`"; "`and` joins two `Bool`s, but this is a `Whole`"; "this value isn't used"; "a
   value should be here, not `x`"; the usage text; `unio oracle` without Ralph built (status 6); and D54's `because:`
   in another file, which should name the file.

## Notes for the design session

- The prompt's way to run Ralph, `dune exec --root oracle -- ralph`, says "Program 'ralph' not found"; the built
  program is `oracle/_build/default/shell/ralph.exe` (found from file names only, nothing in `oracle/` read).
- I wrote the test runner and the first compiler together, so the "every test fails" stage was never seen.
