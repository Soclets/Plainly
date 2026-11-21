# 7. Types

## `Whole`

A whole number from -9223372036854775808 to 9223372036854775807, negatives included: a 64-bit signed integer, C++'s
`std::int64_t` (D15, D131).

```
let $secret: Whole = 42;
```

`Whole` has no second name. Other sizes, if they come, are different types with names of their own (D15).

### Arithmetic

`+` adds and `-` subtracts, with a space on each side. A `-` with no space after it makes a number or a name negative
(D27):

```
$tries = $tries + 1;
let $left: Whole = 5 - $tries;
let $back: Whole = -$speed;
```

`+` is only for numbers (D27). `*` multiplies, with a space on each side (D59):

```
let $total: Whole = $price * $count;
```

### Dividing

```
let $half: Whole = 8 / 2;
let $each: Whole = 7 div.floor 2;
let $each: Whole = 7 div.trunc 2;
```

These give 4, 3 and 3. With -7 in place of 7, `div.floor` gives -4 and `div.trunc` gives -3.

- **`/` divides exactly.** A remainder is a failed check that stops the program in both modes (D46, D70).
- **`div.floor` rounds down to the nearest whole number below;** **`div.trunc` cuts the fraction off,** toward zero
  (D70).
- **A bare `div` is refused** (D70).

`div` is short for divide and `trunc` for truncate (D2).

### Remainders

`rem.floor` and `rem.trunc` give what's left after dividing, each matching its division (D71):

```
let $left: Whole = 7 rem.floor 2;
let $left: Whole = 7 rem.trunc 2;
```

Both give 1. With -7, `rem.floor` gives 1, with the sign of the number divided by, so `-1 rem.floor 8` is 7; and
`rem.trunc` gives -1, with the sign of the number divided. A bare `rem` is refused (D71). `rem` is short for remainder
(D2).

### Maths order

Without brackets, operators go in school maths' order (D72):
1. a `-` negating, as in `-7`;
2. `*`, `/`, `div` and `rem`, left to right;
3. `+` and `-`, left to right;
4. comparisons, then `not`, `and` and `or`;
5. `|>`, last, so a slide takes everything on its left (D73).

So `2 + 3 * 4` is 14. Where maths doesn't say, brackets are required: `&` next to an unbracketed `+`, `-`, `*`, `/`,
`div` or `rem` is refused (D72).

Dividing by zero is a failed check that stops the program (D46, D72).

`??` belongs to the call right before it and takes a single value after it, so `$score + be.Whole($t) ?? 0` means
`$score + (be.Whole($t) ?? 0)` (D81). Two comparisons in a row, as in `1 < $x < 10`, are refused, with `1 < $x and $x <
10` as the fix; and an operator after a slide's last step, as in `$x |> f |> g |> h + 1`, is refused, with `($x |> f |>
g |> h) + 1` as the fix (D81). A shorter one, like `$x |> f + 1`, is written as a call, `f($x) + 1` (D128).

**Not decided:** going past the range; how whole numbers are written; other
number types.

## `Bool`

`true` or `false` (D33):

```
var $won: Bool = false;
$won = $guess == $secret;
```

A comparison gives a `Bool`, and that's what `if` and `while` check (D33). `Bool` is short for Boolean (D2).

**Not decided:** whether a condition must be a `Bool`.

## `Text`

Text, written in double quotes (D21):

```
let $name: Text = "Mia";
show_msg("Don't give up!");
```

Single quotes aren't used for text (D21).

`&` joins two pieces of text. A number joined with `&` is refused, with `be.Text(...)` as the fix, so it's always
visible where a number becomes text (D22, D28):

```
show_msg("You got it in " & be.Text($tries) & " tries.");
```

### Filling in text

`.fmt` fills in what's in braces. It narrows `show_msg` to show the result, and `be.Text` to give it back as `Text`
(D29):

```
show_msg.fmt("You got it in {$tries} tries.");
let $line: Text = be.Text.fmt("{$name}: {$score}");
```

- **Braces hold only a variable's name, with its `$`;** a calculation or a call in them is refused (D29).
- **A name in braces without its `$` is refused,** with the `$` added as the fix when there's a variable of that
  name (D133).
- **A number in braces becomes text by itself** (D29).
- **The text is checked at compile time:** a name in braces that isn't a variable is refused (D29).
- **Without `.fmt`, text is plain,** so `show_msg("{$tries}")` shows exactly `{$tries}` (D29).

`fmt` is short for format (D2). With `.same_line` as well, it's `show_msg.fmt.same_line`, the stronger narrowing first
(D31).

### `\`

Inside text, `\` makes the next character literal, and does nothing else: `\{` is a brace in `.fmt` text, `\"` a quote,
and `\\` a backslash. Only `{` needs escaping in `.fmt` text, since a `}` with no open `{` is just a character. `\`
doesn't start C-style codes, so `\n` isn't a new line (D30).

```
show_msg.fmt("Type \{help} to see the commands, {$name}.");
show_msg("She said \"hi\".");
```

Empty text is `""` (D76).

**Not decided:** a new line or a tab inside text; `\` before a letter; settings in braces; comparing text.

## `Float`

`Float` is a binary floating-point number, stored as IEEE 754 binary64: about 15 to 17 significant digits, from about
10⁻³⁰⁸ to 10³⁰⁸ (D157). Like `Whole`, it has no second name. It's approximate, and honest about it (D155):
- **An inexact literal needs `~`:** `~0.1` is accepted, and `0.1` is refused with `~0.1` as the fix, since 0.1 can't
  be stored exactly in binary. A literal stored exactly, like `0.5`, takes no `~`.
- **An inexact value shows its `~` when printed:** the shortest text that reads back as the same number, with `~` in
  front when the stored value isn't exactly that, as in `~0.30000000000000004`.

```
let $speed: Float = ~0.1;
```

`Decimal` is kept for an exact decimal type, if Unio adds one; it would never need `~` (D155, D157).

**Not decided:** `Float`'s maths and conversions; NaN and infinities beyond D46's check; a rounded, `~`-free way to
show a `Float` to a player.

## Lists

`List<Type>` is a list of one type, written in `[ ]` with commas (D3, D75):

```
var $students: List<Text> = ["Fern", "Plern", "Churn"];
let $first: Text = $students[pos: 1];
let $also_first: Text = $students[offset: 0];
```

- **An item is reached with `pos:`, counting from 1, or `offset:`, counting from 0.** A bare number is refused (D75).
- **A bad position is a failed check** that stops the program, or is refused where the checker can see it (D46,
  D75).

`pos` is short for position (D2).

An empty list is `[]`, taking its type from where it goes (D76).

A value's methods are called with a plain `.` after it, and a method that changes its value can only be called on a
`var` (D77):

```
$students.add("Churn");
$students.add("Tankun", pos: 1);
$students.remove(pos: 2);
$students[pos: 1] = "Mia";
```

- **`.add(item)`** puts it at the end, and **`.add(item, pos: n)`** or `offset: n` at that place (D80).
- **`.remove(pos: n)`** or `offset: n` takes an item out (D80).
- **`list[pos: n] = value`** changes one item (D80).

After a value's `.`, a name with no `( )` is a fact the value already knows, read without running anything. A list's
number of items is `.len` (D79):

```
let $count: Whole = $students.len;
```

**Not decided:** which other methods and facts lists and text have; how a type's methods are declared;
going through a list.

## Converting

A value is converted with `be` narrowed by the type it becomes (D22):

```
be.Type(value)
```

where `Type` is the type it becomes and `value` the value converted.

```
let $guess: Whole = be.Whole($typed);
show_msg(be.Text($guess));
```

- **Converting to the type a value already is, is refused,** since it does nothing (D22).
- **A conversion can fail,** as `be.Whole("abc")` does (D22).
- **`be.Text` turns a `Bool` into `"true"` or `"false"`,** Unio's own spelling of them (D153).

`be.Whole` reads a whole number from text: spaces and tabs around it are ignored (D143), and a `+` or `-` in front and
leading zeros are fine, so ` 42`, `-7`, `+5` and `007` all work. Anything else fails, including `4.5`, `1,000` and a
number too big or too small for a `Whole` (D95), with the failure `not_a_whole` (D156).

**Not decided:** what may be converted to what, beyond D153 and D156, and what other conversions' failures are
called (chapter 8).
