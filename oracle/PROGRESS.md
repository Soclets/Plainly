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
2. [x] Hello world end to end: lexer and parser for `proc.main` and `show_msg`, the core's syntax tree, checker and
   evaluator, extraction, the shell printing. `d04_hello_world` passes.
3. [x] Values and `Whole` arithmetic in Rocq: `Whole` as Rocq's exact integers (`Z`), kept in range by D46's
   overflow check; `be.Text` and `be.Whole` (D95). **Prove** that `+`, `-`, `*` and negation are exact when the result
   fits and stop with the overflow check otherwise. `d13_variables`, `d104_open_line`.
4. [x] Variables, `if`, `while`, `break`, conditions, `and`/`or`/`not` stopping early. `d16_if`, `d19_while`,
   `d34_logic`, `d06_same_line`.
5. [x] Text: `&`, escapes, `.fmt`. `d28_join_text`, `d29_fmt`, `d30_escapes`.
6. [x] Procedures, `returns`, `return`, labels, slides; `ask`, `??`, `random`. `d05_slides`, `d24_procedures`,
   `d20_ask`, and the guessing game played by hand.
7. [ ] **Prove type safety** for the subset: a program the checker accepts never gets stuck.
8. [x] The refusals the tests check, in the shell and the core, with D54's layout and D86's column.
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

2026-11-21: all 27 tests pass, and the guessing game runs (`ralph examples/guessing_game.unio`). The exactness
theorems for `Whole` (`whole_arithmetic_exact`, `whole_negate_exact` in `core/Whole.v`) are proven. Next: the
type-safety proof (step 7), then the spacing refusals (D98, D100) and the rest of step 9.

Known gaps, each to be built:
- Spacing (D98, D100, D102): not checked yet, so a wrongly spaced program is accepted. Step 9.
- Knock-on errors (D55 rule 4, D83): not sorted out yet; every error is shown as its own message without the
  `because:` pointing back.

## Theorem statements sent to the design session

- `whole_arithmetic_exact` (core/Whole.v): for any two whole numbers, `+`, `-` and `*` give the mathematically exact
  answer whenever it fits in a `Whole`; when the exact answer is bigger than the largest `Whole` they stop with D46's
  overflow check saying "too big", and when smaller than the smallest, "too small". Sent 2026-11-21.
- `whole_negate_exact` (core/Whole.v): `-` in front of a whole number gives exactly its negative when that fits; the
  one that doesn't, `-` of -9223372036854775808, stops with the overflow check, "too big". Sent 2026-11-21.

## Questions for the design session

Each stops with `NotDesignedYet` in Ralph until it's answered. Sent 2026-11-21 unless marked otherwise. Questions 1 to
3 are answered by D142, 4 by D143, 10 from D128, all built; 5 to 8 are with Tankun.

1. *(Answered, D142.)* **Negation overflowing:** D92's `when:` shows the values, as in `9223372036854775807 + 1`. What does it show for
   `-$x` when `$x` is -9223372036854775808? (Draft: `-(-9223372036854775808)`.)
2. *(Answered, D142.)* **D99's other bad names:** D102 gives "`` `Score` has a capital letter, but names have none ``" and says "likewise"
   for a name starting with a digit or `_`, ending with `_`, or with two `_` in a row. The exact wordings? (Drafts:
   "`` `$2x` starts with a digit, but names start with a letter ``", "`` `$_x` starts with `_`, but names start with a
   letter ``", "`` `$x_` ends with `_`, but `_` only goes between words ``", "`` `$a__b` has two `_` in a row, but `_`
   only goes between words ``".)
3. *(Answered, D142.)* **D128's three refusals** (a slide of one or two calls, three calls nested, a chain mixing both): their wording
   "comes with a later batch".
4. *(Answered, D143: tabs too.)* **`be.Whole` and spaces (D95):** only the space character, or tabs too?
5. **`ask` and Windows:** is a `\r` before the line break part of the answer? (Ralph keeps it for now; only `\n` ends
   a line.)
6. **A file with `\r\n` line endings:** Ralph stops with `NotDesignedYet`.
7. **A comment on a line of its own:** how is it indented? Ralph accepts the indentation of the line after it (or one
   level in, when that line is a closer or `else`), and stops with `NotDesignedYet` otherwise.
8. **Two statements on one line**, as in `a(); b();`: Ralph stops with `NotDesignedYet`.
9. **Giving a parameter a new value:** D130 leaves a parameter's words undecided, so Ralph stops with
   `NotDesignedYet`.

10. *(Answered by the design session from D128: the innermost call starts the slide whole when it takes more than
    one value.)* **Which argument starts a slide** in D142's fix for nested calls, when the innermost call takes more than one
    argument: `show_msg(be.Text(larger(3, 9)))` could become `larger(3, 9) |> be.Text |> show_msg` or
    `3 |> larger(9) |> be.Text |> show_msg`. Ralph stops with `NotDesignedYet` there. Sent 2026-11-21.

## Readings taken where the standard is quiet

Each is what Ralph does now. The design session confirmed all but 2 and 8 on 2026-11-21; those two are with
Tankun.

1. **Where refusals live:** the prompt puts "every refusal" in Rocq. Ralph's parser (OCaml) refuses what's about how
   text is written (`;`, `:`, closers, indentation, blank lines, quotes, names' spelling, `=` in a condition, chained
   comparisons), since those are found before there's a syntax tree; the Rocq checker refuses everything about
   meaning. The type-safety theorem covers what the checker accepts.
2. **Status with both kinds:** a program with refusals and a `NotBuiltYet` or `NotDesignedYet` exits with 3.
3. **The parser's messages,** generalised from D92's three: "`` `)` should be here, not `x` ``", "a value should be
   here, not `` `x` ``", "the line ends where `` `"` `` should be", with no `why:`.
4. **Carets** where the decisions don't say: a type mismatch in `==` or `+` points at the operator; a missing `return`
   (D88) at the procedure's name; no `proc.main` at line 1 column 1; the file ending inside a block just after the
   last line's text; a line after `return` or `break` at that whole line.
5. **One extra blank line** reads "1 extra blank line, after the one on line 3", singular.
6. **Bad names** (D99) are refused where they're declared; using an undeclared one gives D92's unknown variable.
7. **After `return` or `break`,** only the first line that follows is refused.
8. **`while`** always counts as a way through that can reach the end, for D57's "every way through ends in
   `return`", even `while true:`.
9. **`random`** is seeded differently on every run (seeding isn't decided).
10. **Names in messages** keep their `$`: "there's no variable named `$nme` here", also for `{nme}` with no `$`.
11. **The line-number column** in a message is as wide as its widest line number, and at least 2.
