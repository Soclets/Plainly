# `Unlo\n`: progress

Where the compiler stands, kept current so a fresh context can carry on from here and `git log`. The live test results
are in `compiler/test-status.txt`, written by every test run.

## One command

```
cd compiler && cmake --workflow --preset check
```

configures, builds (3 cores, AddressSanitizer and UndefinedBehaviorSanitizer, libc++ hardened), checks formatting and
`clang-tidy`, runs the unit tests on Unio IR, then every test in `tests/` in both modes, and writes
`compiler/test-status.txt`. `cmake --workflow --preset fast` does the same without `clang-tidy`, for quick rounds.

## Tools, pinned

- **LLVM 23.1.2**, the self-contained static build at `~/llvm-self-contained/install` (LLVM and lld, built without
  RTTI or exceptions, targets AArch64, ARM and X86). It never changes under us, unlike Homebrew's, which `brew upgrade`
  can move. The compiler links LLVM and lld statically, and links programs with lld in its own process.
- **Homebrew's LLVM 23.1.2** (`/opt/homebrew/opt/llvm@23`) for `clang++`, `clang-format` and `clang-tidy`, the same
  version, and the compiler the static build was made with.
- CMake 4.4.3, Ninja. Nothing installed.
- Ralph, run as a black box: `oracle/_build/default/shell/ralph.exe PROGRAM.unio` after
  `opam exec --switch=ralph -- dune build --root oracle`.

## The plan for the first milestone

Each step ends with something running, in both modes.

1. **Test runner** (`tools/test_runner.cpp`): every test, both modes, stdout, stderr and status byte for byte, plus
   each `check.py`; writes `compiler/test-status.txt`. Every test fails at first.
2. **Hello world end to end:** source → syntax tree → typed tree → Unio IR → LLVM IR → object → linked with the runtime
   by lld in-process → run. Both modes. The parallel, cached back end arrives here, since it's the shape of the
   compiler, not an extra: each procedure is its own LLVM module, compiled on its own thread, cached under a
   fingerprint.
3. **Whole numbers, variables, `if`, `while`, `break`** (d13, d16, d19), with overflow checks written out in Unio IR,
   their failure path cold (D124 #13), and in `final`: check removal from what an `if` proved (#2, #8), and LLVM's full
   optimizing (#1). Runtime check messages (d104).
4. **Procedures, `return`, labels, slides** (d05, d24, d34), with `final`'s inlining of small procedures (#3), the
   fib trick for procedures with no side effects (#6), recursion into loops (#7) and constants across procedures (#9),
   each with unit tests on Unio IR where it applies and where it mustn't.
5. **Text:** `&`, `be.Text`, `.fmt`, escapes, `ask`, `be.Whole`, `??`, `random` (d20, d28, d29, d30), with Text as
   copy-on-write values (D130) and the 30% memory cap (D134).
6. **Refusals** the tests check, then the rest of the subset's decided refusals, with D55's recovery.
7. **Guessing game** in both modes; **fib** timed beside gfortran and C.

Where each of D124's optimizations for this subset arrives: #1 LLVM's full optimizing, step 2; #2 and #8 removing
checks from what an `if` proved, step 3; #13 check-failure code out of the way, step 3; #3 inlining, step 4; #6 the fib
trick, step 4; #7 recursion into loops, step 4; #9 constants across procedures, step 4.

## Status

- 2026-11-21: plan written; tools checked, LLVM pinned. Nothing built yet.

## Questions sent to the design session

(none yet)

## Notes for the design session

- The prompt's way to run Ralph, `dune exec --root oracle -- ralph`, says "Program 'ralph' not found"; the built
  program is `oracle/_build/default/shell/ralph.exe` (found from file names only, nothing in `oracle/` read).
