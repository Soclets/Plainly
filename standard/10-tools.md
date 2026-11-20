# 10. Tools

## The compiler

Plainly's compiler, PlainCompiler (D51), is written in Rust. It compiles a whole program ahead of time to one native executable, through
Cranelift, with no bytecode and no JIT. Running a program compiles it to a temporary file and starts it (D45).

The web playground runs the oracle compiled to WebAssembly (D45).

**Not decided:** the oracle's language.

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

## Memory

PlainGC (D51), a garbage collector in the style of Go's, frees memory (D48):
- **it marks what's in use while the program keeps running,** so pauses stay well under a millisecond;
- **it never moves objects,** so an `extern.rust` procedure can hold a plain pointer during a call;
- **it needs only a write barrier,** a check when a reference is stored;
- **it groups objects by size;**
- **it's precise,** finding references through Cranelift's stack maps.

It may first be built as a simple mark-and-sweep with the same never-moving promise (D48).

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
