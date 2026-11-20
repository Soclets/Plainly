# 7. Types

## `Whole`

A whole number from -9223372036854775808 to 9223372036854775807, negatives included: a 64-bit signed integer, Rust's
`i64` (D15).

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

`+` is only for numbers (D27).

**Not decided:** going past the range; multiplying and dividing; maths order; how whole numbers are written; other
number types.

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

- **Braces hold only a variable's name;** a calculation or a call in them is refused (D29).
- **A number in braces becomes text by itself** (D29).
- **The text is checked while building:** a name in braces that isn't a variable is refused (D29).
- **Without `.fmt`, text is plain,** so `show_msg("{$tries}")` shows exactly `{$tries}` (D29).

`fmt` is short for format (D2).

**Not decided:** a literal `{` inside `.fmt` text; settings in braces; the order of `.fmt` and `.same_line` together; a
quote inside text, and other escapes; empty text; comparing text; lists.

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

**Not decided:** what may be converted to what, and what a failed conversion's failure is called (chapter 8).
