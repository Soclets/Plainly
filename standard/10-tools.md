# 10. Tools

## The compiler

Unio's compiler, `Clang\n` (D51, D135, D136), is written in C++. It compiles a whole program ahead of time to one native
executable, through LLVM in both modes, with no bytecode and no JIT. Running a program compiles it to a temporary file
and starts it (D45, D131).

The web playground runs the oracle compiled to WebAssembly (D45).

`Clang\n` works in stages (D62):

```
source → syntax tree → typed tree → Unio IR → LLVM IR → native code
```

Refusals happen at the typed tree, so `unio check` stops there. Unio IR is the one intermediate form of Unio's
own, where every check, allocation, label, default and handler case is written out, and where `final` mode's own
optimizations run. Both modes take the same path (D62).

PlainOracle, the reference implementation, is an OCaml shell around a checker and an evaluator written and proven in
Rocq, then extracted to OCaml, as CompCert is built (D129).

**Not decided:** what is proven, and in what order.

## The command

```
unio run game.unio --working
unio compile game.unio --final
unio check game.unio
unio oracle game.unio
```

- **`run`** compiles to a temporary file and runs it; **`compile`** makes the file to ship. Both need a mode (D46, D50,
  D91, D135, D136).
- **`check`** reports what's refused without building, and needs no mode (D50).
- **`oracle`** runs the file on PlainOracle (D51), the separate reference implementation, printing exactly what the
  compiled program would. `unio` forwards to the oracle's own program (D50).

## Compiling

- **While you edit,** `Clang\n` compiles after every save on one background thread, inside the editor support,
  filling the cache (D65).
- **`unio run` and `unio compile`** compile whatever isn't cached on every CPU core (D65).
- **The cache** keeps each compiled piece under a fingerprint of everything that affects it, in the system's cache
  folder, shared by every project, so only what changed is recompiled, and a build never differs because of it (D65).

## Memory

Every variable owns its own value, and a value is freed when its owner ends: there's no garbage collector, so no pauses
(D130). A program may hold at most 30% of the machine's memory, with no settings (D134). Running out stops the program
with `check failed:` and status 4, at the line that needed the memory, with what was needed, what was in use and the cap
in `when:` (D115, D122).

## Build modes

Every build names one of two modes, `--working` or `--final`. There's no default (D46).

| Mode | Optimizations | Checks |
|---|---|---|
| working | none, so the fastest compile | every check kept, each stopping at the exact line |
| final | full, from Unio's front end and LLVM | none dropped; expensive ones made cheaper |

In final mode, checks the optimizer proves can never fail are removed, and the NaN check after every float operation
becomes the CPU's own flags, read where a value leaves a procedure, so its message names the procedure (D46).

Final mode's optimizations (D124), none of which changes what a program does:
- LLVM's full optimizing;
- removing checks that can't fail, using what an `if` proved;
- inlining small procedures, and moving check-failure code out of the way so more inline;
- keeping short-lived values off the heap;
- for procedures with no side effects, inlining them into themselves and merging repeated calls;
- recursion into loops, constants across procedures, merging identical procedures, recording what each procedure
  reads and changes, and passing only the parts of a value a procedure uses;
- doing a calculation once where some paths did it twice;
- for loops: fast list fills and copies, unrolling, vectorization, moving an unchanging `if` out, swapping nested
  loops, reusing values from the last pass, and Graphite-style polyhedral reordering of loop nests.

The modes are written lowercase everywhere, docs and messages included: `working` and `final` (D47).

**Not decided:** each check's message.
