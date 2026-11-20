# 3. Words

## Keywords

Each keyword has exactly one spelling, the short one: `proc`, never `procedure` (D2). The docs, and error messages,
teach the full word where a keyword first appears, as in "`proc` is short for procedure" and "`msg` is short for
message".

| Keyword | Short for | Decided in |
|---|---|---|
| `proc` | procedure | D2, D4 |
| `end` | | D8 |
| `.main` | | D4 |
| `.same_line` | | D6 |

**Not decided:** every other keyword.

## Names

A variable's name starts with `$`: `$name`, `$secret` (D11). So a variable is never mistaken for a keyword or a call,
and new keywords never break existing names.

A procedure's name is a plain word, like `larger` or `roll_dice`, with no mark.

**Not decided:** which characters a name may hold; how words in a name are joined.

## Brackets

Each kind of bracket has one job (D3):

| Bracket | Job | Example |
|---|---|---|
| `( )` | a procedure's parameters and a call's arguments; grouping in maths | `add(1, 2)` |
| `[ ]` | lists | `[1, 2, 3]` |
| `< >` | filling in a type | `List<Whole>` |

**Not decided:** lists themselves, types themselves, maths.

## Symbols

| Symbol | Meaning | Decided in |
|---|---|---|
| `;` | ends a statement | D7 |
| `:` | ends an opening line | D8 |
| `$` | starts a variable's name | D11 |
| `.` | narrows | D4, D6 |
| `\|>` | passes a value along a slide | D5 |
