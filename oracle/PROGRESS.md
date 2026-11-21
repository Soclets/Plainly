# Ralph, the oracle: progress

Ralph is Unio's reference implementation (D129, D139, D140, D141). This file is where the oracle session keeps its
plan, where things stand, and its open questions, so a fresh context can carry on from it, `test-status.json` and
`git log`.

## The one command

```
cd oracle && opam exec --switch=ralph -- dune build @check -j 3
```

It builds the Rocq core (checking every proof), extracts it to OCaml, builds the shell and the test runner, checks
`Print Assumptions` for every main theorem, runs every test in `../tests`, and writes `test-status.json`.

To run a program by hand: `opam exec --switch=ralph -- dune exec --root oracle -- ralph PROGRAM.unio`, or the built
`oracle/_build/default/shell/ralph.exe PROGRAM.unio`.

**Placeholder:** Ralph's own program is called `ralph` and takes one file, `ralph program.unio`. It stands in for
`unio run program.unio --working` in the tests, and later for what `unio oracle` forwards to (D50).

## Pinned versions (Building rule 14)

- opam switch `ralph`
- OCaml 5.5.2
- The Rocq Prover 9.3.0, with rocq-stdlib 9.2.0
- dune 3.25.0, menhir 20260209, ocamlformat 0.29.0

## Layout

| Folder | What | Language |
|---|---|---|
| `core/` | Unio's meaning: syntax tree, values, `Whole` arithmetic, checker, evaluator, and the proofs | Rocq |
| `extraction/` | turns `core/` into OCaml (`Extract.v`), and the list of trusted extraction mappings | Rocq |
| `assumptions/` | `Print Assumptions` for every main theorem, captured to a file the test runner checks | Rocq |
| `shell/` | reading the file, lexer, parser, message wording and layout, exit statuses, printing, input, random | OCaml |
| `tests/` | the test runner | OCaml |

### Where each refusal lives

- **Shell (OCaml), how text is written:** `;` and `:`, closers, indentation, blank lines, spacing, quotes, a name's
  spelling (D99), `=` in a condition, chains of comparisons. These are found while reading the text, before there's
  a syntax tree.
- **Core (Rocq), what the program means:** names and scopes, types, `let` and `var`, labels, calls, `return`,
  `break`, slides, `.fmt` braces, failures, and the program's shape. The core gives back each refusal as data; the
  shell only words it (D54).

## Plan for the first milestone

Each step ends with something running. Proofs grow with the core rather than at the end.

1. [x] Test runner, in OCaml, and every test failing.
2. [ ] Hello world end to end: lexer and parser for `proc.main` and `show_msg`, the core's syntax tree, checker and
   evaluator, extraction, the shell printing. `d04_hello_world` passes.
3. [ ] Values and `Whole` arithmetic in Rocq: `Whole` as Rocq's exact integers (`Z`), kept in range by D46's
   overflow check; `be.Text` and `be.Whole` (D95). **Prove** that `+`, `-`, `*` and negation are exact when the result
   fits and stop with the overflow check otherwise. `d13_variables`, `d104_open_line`.
4. [ ] Variables, `if`, `while`, `break`, conditions, `and`/`or`/`not` stopping early. `d16_if`, `d19_while`,
   `d34_logic`, `d06_same_line`.
5. [ ] Text: `&`, escapes, `.fmt`. `d28_join_text`, `d29_fmt`, `d30_escapes`.
6. [ ] Procedures, `returns`, `return`, labels, slides; `ask`, `??`, `random`. `d05_slides`, `d24_procedures`,
   `d20_ask`, and the guessing game played by hand.
7. [ ] **Prove type safety** for the subset: a program the checker accepts never gets stuck.
8. [ ] The refusals the tests check, in the shell and the core, with D54's layout and D86's column.
9. [ ] Every other decided refusal in the subset, or `NotBuiltYet` where it isn't built, so Ralph never accepts a
   program the standard refuses.
10. [ ] Review the theorems' statements with the design session; report to Tankun.

### Order of proofs

1. `Whole` arithmetic is exact (step 3): small and self-contained.
2. Type safety (step 7), by the "definitional interpreter" method: the evaluator runs with fuel (a counter that
   shrinks each step, since Rocq only accepts functions that finish), and the proof goes by induction on the fuel,
   showing that a well-typed statement or expression always gives a value of its type, a runtime stop, or runs out of
   fuel, never "stuck".

## How the core runs input and output

The evaluator is pure: instead of printing or reading, it gives back a tree of requests ("show this", "need a line
of input", "need a random number from 1 to 100"), each carrying what to do next with the answer. The shell walks the
tree, doing the real printing and reading. Type safety is stated over every path through that tree, for every
possible input.

## Status

Not started yet: see the plan above.

## Questions for the design session

None yet.

## Readings taken where the standard is quiet

None yet.
