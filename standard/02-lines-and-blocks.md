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

## Comments

- **`//` comments to the end of the line** (D74).
- **`///` is a doc comment** on the lines right before a declaration; one with no declaration after it is refused
  (D74).
- **`comment name:` to `end comment name` is a block comment.** Everything between is ignored, and it ends only at its
  own `end comment` with its name, so comment blocks can sit inside each other (D74).

```
/// Gives the larger of two whole numbers.
proc larger(Whole a, b) returns Whole:
    // the tie goes to b
    ...
end proc larger

comment old_scoring:
    let bonus: Whole = tries * 10;
end comment old_scoring
```

where `...` stands for the procedure's body.

**Not decided:** blank lines; how a statement's later lines are indented.

## Files

A Plainly file's name ends in `.plainly`, as in `game.plainly` (D35).

A program is every `.plainly` file in its folder, with exactly one `proc.main` across them. There are no imports: a
procedure in one file calls one in another directly, in any order (D64).

## Packages

Packages are listed once, in a manifest file in the program's folder, and their procedures are then usable in every
file (D64). Names are used bare. When two packages both have a name, the bare name is refused, and the package's name
goes in front with `::`, one level deep (D64):

```
crossterm::draw(...);
```

**Not decided:** the manifest's name and format; versions; where packages come from.
