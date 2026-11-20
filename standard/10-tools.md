# 10. Tools

## The compiler

Plainly's compiler is written in Rust. It compiles a whole program ahead of time to one native executable, through
Cranelift, with no bytecode and no JIT. Running a program compiles it to a temporary file and starts it (D45).

The web playground runs the oracle compiled to WebAssembly (D45).

**Not decided:** the command's name and its parts; the oracle's language.

## Build modes

Every build names one of two modes, `--working` or `--final`. There's no default (D46).

| Mode | Optimizations | Checks |
|---|---|---|
| working | none, so the fastest compile | every check kept, each stopping at the exact line |
| final | full, from Plainly's front end and Cranelift | none dropped; expensive ones made cheaper |

In final mode, checks the optimizer proves can never fail are removed, and the NaN check after every float operation
becomes the CPU's own flags, read where a value leaves a procedure, so its message names the procedure (D46).

**Not decided:** how the modes are written in docs and messages; each check's message.
