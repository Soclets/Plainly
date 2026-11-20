# 5. Control flow

## `if`

```
if condition:
    body
else if condition:
    body
else:
    body
end if
```

where each `condition` is what's checked and each `body` the statements run when it holds. There can be any number of
`else if` parts and at most one `else:`, and both are optional. One `end if` closes the whole chain (D8, D16).

```
if $guess < $secret:
    show_msg("Too low");
else if $guess > $secret:
    show_msg("Too high");
else:
    show_msg("Correct!");
end if
```

## Comparisons

```
$lives == 0
$guess != $secret
$guess < $secret
$score > $best
$hp at_least 1
$hp at_most 100
```

- **`==` and `!=`:** equal and not equal (D17).
- **`<` and `>`:** less and greater, with a space on each side, so they're never mistaken for a type's `< >`, which has
  none (D3, D17).
- **`at_least` and `at_most`:** instead of `>=` and `<=` (D17).
- **A lone `=` in a condition is refused** (D14).

**Not decided:** comparing two pieces of text; combining conditions; what else a condition may be.

## After an `if` that leaves

When any way through an `if` leaves, by `return`, `break` or a failure, nothing may follow its `end if` in the same
block, and code that should run afterwards goes in the ways that don't leave (D9). That covers:

- **every way leaving, with no `else`,** the guard clause;
- **only some ways leaving,** where code after `end if` would run only after the others;
- **every way leaving, `else` included,** where code after `end if` could never run.

Each is refused while building (D9).

```
proc larger($a: Whole, $b: Whole) returns Whole:
    if $a > $b:
        return $a;
    else:
        return $b;
    end if
end proc larger
```

**Not decided:** `return`, `break`, failures, loops, and every other part of control flow.
