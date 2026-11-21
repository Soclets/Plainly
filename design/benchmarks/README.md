# Benchmarks

Programs in other languages that `Clang\n`'s `final` mode is compared against (D124).

- `fib.f90`, `fib.c`: recursive Fibonacci. Measured on Tankun's Mac on 2026-11-20 for `fib(44)`:
  gfortran 16.2 `-O0` 2.62 s, clang `-O3` (C) 0.95 s, gfortran `-O3` about 0.003 s. gfortran's speed comes from GCC
  noticing `fib` has no side effects, inlining it into itself and merging the repeated `fib(n - 2)` call, so the work
  grows by about 1.32 per step instead of 1.62 (D124, optimization 6).

```
gfortran -O3 fib.f90 -o fib && ./fib 44
clang -O3 fib.c -o fib_c && ./fib_c 44
```
