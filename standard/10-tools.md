# 10. Tools

## The compiler

Plainly's compiler, PlainCompiler (D51), is written in Rust. It compiles a whole program ahead of time to one native executable, through
Cranelift, with no bytecode and no JIT. Running a program compiles it to a temporary file and starts it (D45).

The web playground runs the oracle compiled to WebAssembly (D45).

PlainCompiler works in stages (D62):

```
source → syntax tree → typed tree → Plainly IR → Cranelift IR → native code
```

Refusals happen at the typed tree, so `plainly check` stops there. Plainly IR is the one intermediate form of Plainly's
own, where every check, allocation, label, default and handler case is written out, and where `final` mode's own
optimizations run. Both modes take the same path (D62).

PlainOracle, the reference implementation, is written in C (D52).

It's C23, compiled with Clang with every warning on, and every test run is under AddressSanitizer and
UndefinedBehaviorSanitizer (D53).

## The command

```
plainly run game.plainly --working
plainly build game.plainly --final
plainly check game.plainly
plainly oracle game.plainly
```

- **`run`** builds to a temporary file and runs it; **`build`** makes the file to ship. Both need a mode (D46, D50).
- **`check`** reports what's refused without building, and needs no mode (D50).
- **`oracle`** runs the file on PlainOracle (D51), the separate reference implementation, printing exactly what the compiled
  program would. `plainly` forwards to the oracle's own program (D50).

## Compiling

- **While you edit,** PlainCompiler compiles after every save on one background thread, inside the editor support,
  filling the cache (D65).
- **`plainly run` and `plainly build`** compile whatever isn't cached on every CPU core (D65).
- **The cache** keeps each compiled piece under a fingerprint of everything that affects it, in the system's cache
  folder, shared by every project, so only what changed is recompiled, and a build never differs because of it (D65).

## Memory

PlainGC (D51), a garbage collector in the style of Java's ZGC, frees memory (D69):
- **one heap for every worker's thread,** collected while they all keep running, with pauses that stay tiny at any
  heap size;
- **it moves objects as it collects,** so memory never fragments;
- **every read of a reference gets a small check** (a load barrier), so a moved object is found;
- **it's precise,** finding references through Cranelift's stack maps;
- **`extern.rust` code holds handles** to Plainly values, not plain pointers.

**Not decided:** how handles work on the Rust side; the order of building it.

## Build modes

Every build names one of two modes, `--working` or `--final`. There's no default (D46).

| Mode | Optimizations | Checks |
|---|---|---|
| working | none, so the fastest compile | every check kept, each stopping at the exact line |
| final | full, from Plainly's front end and Cranelift | none dropped; expensive ones made cheaper |

In final mode, checks the optimizer proves can never fail are removed, and the NaN check after every float operation
becomes the CPU's own flags, read where a value leaves a procedure, so its message names the procedure (D46).

The modes are written lowercase everywhere, docs and messages included: `working` and `final` (D47).

**Not decided:** each check's message.
