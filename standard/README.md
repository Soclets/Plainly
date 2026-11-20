# The Plainly standard

What Plainly is now, by topic. [`DECISIONS.md`](../DECISIONS.md) records how each rule was decided, in the order it
happened; this standard says what the rules are today (D12).

## How it's written

- **`DECISIONS.md` is the source.** Every rule here names its decision, as in "(D7)". Where the two disagree, the
  decision wins and the standard is wrong.
- **It changes with the decisions.** A decision that changes a rule updates the standard in the same commit (D12).
- **Undecided means "Not decided".** Anything the decisions leave open is marked **Not decided** where it would go,
  never filled in here.
- **Terms come first.** Every word the standard relies on is defined in [Terms](01-terms.md) before a chapter uses it
  (D12).
- **Placeholders are explained.** A general form says right after it what each placeholder stands for. A real example,
  with no placeholders, needs no such line (D12).

## Chapters

1. [Terms](01-terms.md)
2. [Lines and blocks](02-lines-and-blocks.md): `;`, `:` and `end`, and indentation
3. [Words](03-words.md): keywords, names and brackets
4. [Procedures](04-procedures.md): definitions, calls, `proc.main`, slides and `show_msg`
5. [Control flow](05-control-flow.md): what may follow an `if`
6. [Variables](06-variables.md): `let` and `var`
7. [Types](07-types.md): `Whole`
