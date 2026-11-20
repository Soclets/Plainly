# 10. Tools

## The compiler

Plainly's compiler is written in Rust. It compiles a whole program ahead of time to one native executable, through
Cranelift, with no bytecode and no JIT. Running a program compiles it to a temporary file and starts it (D45).

The web playground runs the oracle compiled to WebAssembly (D45).

**Not decided:** the command's name and its parts; build modes; the oracle's language.
