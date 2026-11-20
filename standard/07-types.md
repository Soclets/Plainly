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

**Not decided:** a quote inside text, and other escapes; empty text; joining and comparing text; lists.

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
