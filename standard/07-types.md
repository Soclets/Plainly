# 7. Types

## `Whole`

A whole number from -9223372036854775808 to 9223372036854775807, negatives included: a 64-bit signed integer, Rust's
`i64` (D15).

```
let $secret: Whole = 42;
```

`Whole` has no second name. Other sizes, if they come, are different types with names of their own (D15).

**Not decided:** going past the range; how whole numbers are written; other number types.

## `Text`

Text, written in double quotes (D21):

```
let $name: Text = "Mia";
show_msg("Don't give up!");
```

Single quotes aren't used for text (D21).

**Not decided:** a quote inside text, and other escapes; empty text; joining and comparing text; lists.
