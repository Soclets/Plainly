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

**Not decided:** how a parameter is written; `returns` and the types; how a procedure gives back its value; a
procedure that gives nothing back.

## Calls

A call writes the procedure's name and `( )`, with its arguments inside, even when there are none (D3, D4):

```
add(1, 2);
roll_dice();
```

### Labels

A procedure can give its arguments labels, and every call writes them, with `:` after each (D24):

```
let $secret: Whole = random(from: 1, to: 100);
```

- **The procedure's writer chooses each label.** A call can't leave one off, or write one the procedure doesn't have
  (D24).
- **By convention, the first argument has no label,** as in `show_msg("Hi")` (D24).

**Not decided:** how a procedure declares its labels; defaults; label order; `random` itself.

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

**Not decided:** where `|>` sits among other operators; `be.Text`, `clamp` and labels, which the examples use.

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

**Not decided:** text itself; filling values into text; turning text into a number; asking with no question; running
out of input; reading single key presses.
