# 2. Lines and blocks

```
proc.main:
    show_msg("Hello, World!");
end proc.main
```

## Statements

Every statement ends with `;` (D7). A statement can run over several lines, and ends only at its `;`:

```
score
|> clamp(min: 0, max: 100)
|> be.Text
|> show_msg;
```

A missing `;` is refused, with the place it goes as the fix (D7).

## Blocks

A block's opening line ends with `:`, and its closer is `end` followed by what it closes, standing alone with no `;`
(D8):

```
opening line:
    body
end what
```

where `opening line` is the block's first line, like `proc.main` or `if a > b`; `body` is its statements; and `what`
is what the block is, like `proc.main`, `proc larger` or `if`. The `:`, the word `end` and the line breaks are written
as they are.

**Not decided:** what a closer says for every kind of block besides procedures and `if`.

## Indentation

A body is indented one level inside its opening line and closer, and a block inside a body one level further. One level
is 4 spaces. Any other indentation, a tab included, is refused while building, with the whole corrected line as the fix
(D10).

Since every block ends with its closer, indentation never decides where a block ends (D8, D10).

**Not decided:** comments; blank lines; how a statement's later lines are indented.

## Files

A Plainly file's name ends in `.plainly`, as in `game.plainly` (D35).
