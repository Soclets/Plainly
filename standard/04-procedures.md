# 4. Procedures

## Definitions

```
proc name(parameters) returns Type:
    body
end proc name
```

where `name` is the procedure's name, `parameters` its parameters separated by commas, `Type` the type it gives back,
and `body` its statements. `proc`, `returns`, the brackets, the `:` and `end proc` are written as they are (D3, D8).

A definition writes `( )` only when it has parameters. One that takes nothing has no brackets (D4):

```
proc roll_dice returns Whole:
    ...
end proc roll_dice
```

where `...` stands for the procedure's body.

A procedure that gives back a value says so at the end of its header, with `returns` and the type (D37).

A procedure that gives nothing back writes no `returns` (D38):

```
proc greet($name: Text):
```

Using what such a procedure gives as a value is refused (D38).

### `return`

- **`return value;`** gives back a value of the header's type, and every way through a procedure with `returns` must
  end in one (D57).
- **A bare `return;`** leaves early, in a procedure with no `returns` (D57).
- **Nothing may follow `return` in the same block** (D57).
- **A bare `return;` as a procedure's last line is refused** (D57).

### Parameters

Each parameter is written as its name, with its `$`, then `:` and its type, as a declaration is, even when several
share a type (D125, D126). Writing a shared type once in front, as in `proc add(Whole a, b)`, is refused (D126):

```
proc add($a: Whole, $b: Whole) returns Whole:
proc draw($x: Whole, $name: Text):
proc double($n: Whole) returns Whole:
```

Failures in a header: chapter 8.

## Procedures written in Rust

`extern.rust` in front of `proc` says the procedure's body is written in Rust, outside the Plainly file. Its body is
empty, or holds only a `labels` line, and its closer doesn't repeat `extern.rust` (D36):

```
extern.rust proc beep returns Bool:
end proc beep
```

Callers can't tell it apart from any other procedure. `extern` is short for external (D2, D36).

Its Rust code lives in a folder named `rust` in the program's folder, holding an ordinary Cargo project, which
PlainCompiler builds and links into the program (D107), only when an `extern.rust` procedure uses it (D118). Each procedure's Rust function is marked with a macro naming
it exactly, which writes the glue that links it, and holds Plainly values through handles (D105, D108):

```rust
#[plainly::procedure("show_msg.same_line")]
fn show_msg_same_line(call: &mut Call, text: Handle<Text>) { ... }
```

A `Whole` crosses into Rust as `i64` and a `Bool` as `bool`, passed and returned directly; a `Text` crosses as a handle
(D105, D109).

An `extern.rust` procedure that can fail lists its failures in its header as any procedure does; its Rust function
returns Rust's `Result`, with the failures named in the macro, as in `fails = "not_found, unreadable"`, and PlainCompiler
refuses a program whose macro and header disagree (D110). If its Rust code panics, going wrong rather than failing on purpose, the program stops with an
`internal error:` naming the procedure, with status 7, and a `why:` naming the Rust file and line, to fix it there or
report it to whoever wrote it (D111, D112).

## Families

A family declares a word and its kinds together, in a block saying what kind of thing it holds, and holding only that
(D41):

```
fam.kind word:
    members
end fam.kind word
```

where `kind` is what the family holds (`proc` for a procedure and its narrowings, `handler` for a failure's handlers),
`word` the word, and `members` its declarations.

```
fam.proc show_msg:
    proc show_msg($text: Text):
        ...
    end proc show_msg

    proc show_msg.same_line($text: Text):
        ...
    end proc show_msg.same_line
end fam.proc show_msg
```

where each `...` stands for that procedure's body.

- **Every member is the word itself, or reads as "a Y kind of X" for its name `X.Y`.** Anything else is refused:
  `geometry.area` isn't a kind of geometry (D41).
- **A narrowed procedure is declared inside its word's family** (D41).
- **Each member keeps its full name** (D41).

`fam` is short for family (D2).

Handler families: chapter 8.

## Calls

A call writes the procedure's name and `( )`, with its arguments inside, even when there are none (D3, D4):

```
let $total: Whole = add(1, 2);
show_score();
```

A call that gives back a value has its value used: kept in a variable, passed on, or compared. One standing alone,
its value thrown away, is refused; a call that gives nothing stands alone (D120).

### Labels

A procedure can give its arguments labels, and every call writes them, with `:` after each (D24):

```
let $secret: Whole = random(from: 1, to: 100);
```

- **The procedure's writer chooses each label.** A call can't leave one off, or write one the procedure doesn't have
  (D24).
- **By convention, the first argument has no label,** as in `show_msg("Hi")` (D24).

A procedure declares its labels on a `labels` line, the first line of its body, with no brackets (D25):

```
labels parameter as label, parameter as label;
```

where each `parameter` is one of the procedure's parameters and `label` the word callers write for it. `labels`, `as`,
the commas and the `;` are written as they are.

```
proc random($low: Whole, $high: Whole) returns Whole:
    labels $low as from, $high as to;
    ...
end proc random
```

where `...` stands for the rest of the body. Inside, the procedure uses `$low` and `$high`, and callers write `from:` and
`to:` (D25).

**Not decided:** defaults; label order.

## `proc.main`

`proc.main` is the procedure the program starts with. It takes nothing, so it has no brackets, and it's never called,
since only the program's start runs it (D4):

```
proc.main:
    show_msg("Hello, World!");
end proc.main
```

## Slides

A slide passes the value on the left of `|>` into the next step as its first argument, so a chain reads left to right
in the order it runs (D5):

```
$points |> be.Text |> show_msg;
$score |> clamp(min: 0, max: 100) |> be.Text |> show_msg;
```

- **A step with nothing else to pass is bare,** with no `( )`: `|> be.Text`.
- **A step with more arguments takes `( )` for those only:** `|> clamp(min: 0, max: 100)`, and the slid value goes in
  first.

A step isn't a whole call, so it doesn't write `()` as a call does: the step names the procedure, and the slide calls
it (D4, D5).

`|>` comes last in the order of operators, so a slide takes everything on its left, as in `a + 1 |> be.Text` (D73).

**Not decided:** `clamp`, which the example uses.

## Showing text

`show_msg` shows text and ends the line. `show_msg.same_line` shows text and leaves the line open, for prompts (D6):

```
show_msg("Hello, World!");
show_msg.same_line("Your guess: ");
```

## Asking

`ask` shows a question, leaves the cursor on its line, waits until the player presses Enter, and gives back what they
typed, without the Enter (D20):

```
ask(question)
```

where `question` is the text shown.

```
let $typed: Text = ask("Your guess: ");
```

What's typed always comes back as text, even digits: typing `42` gives `"42"`, not the number 42 (D20).

When there's no more input (the end-of-input key, or a file of answers running out), `ask` ends the program right
there, quietly, with exit status 0. A last line with no Enter after it still counts as an answer (D96).

**Not decided:** asking with no question; reading single key presses; what Ctrl+C does.
