# Problems C caused in PlainOracle

PlainOracle is written in C as an experiment, to see how many problems C causes (D52). Every problem goes here as it
happens, newest last, including ones caught before they did any harm. Nothing is left out for being small or
embarrassing: the count is the point.

Each entry:

```
## N. Short title (date)

- **What went wrong:** what happened, or would have.
- **The part of C behind it:** for example manual memory, a union read as the wrong field, undefined behaviour,
  text as bytes, missing bounds checks, the preprocessor.
- **How it was caught:** a test, a sanitizer, a compiler warning, reading the code, or a user.
- **What it cost:** time spent, and anything shipped wrong.
- **Would OCaml, C++ or SPARK have prevented it?** Which, and how (D52 weighs them).
```

No problems yet: PlainOracle isn't built (D12).
