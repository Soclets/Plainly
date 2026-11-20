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
if guess < secret:
    show_msg("Too low");
else if guess > secret:
    show_msg("Too high");
else:
    show_msg("Correct!");
end if
```

## Comparisons

```
lives == 0
guess != secret
guess < secret
score > best
hp at_least 1
hp at_most 100
```

- **`==` and `!=`:** equal and not equal (D17).
- **`<` and `>`:** less and greater, with a space on each side, so they're never mistaken for a type's `< >`, which has
  none (D3, D17).
- **`at_least` and `at_most`:** instead of `>=` and `<=` (D17).
- **A lone `=` in a condition is refused** (D14).

**Not decided:** comparing two pieces of text; what else a condition may be.

## `and`, `or` and `not`

Conditions combine with `and`, `or` and `not`: `not` first, then `and`, then `or`, with comparisons before all three
(D34):

```
if not won:
    show_msg.fmt("Out of tries! It was {secret}.");
end if

if hp at_least 1 and lives > 0 or cheat == true:
```

The second means `(hp at_least 1 and lives > 0) or cheat == true`.

`and` and `or` stop early: the right side runs only if it can still change the answer (D34).

**Not decided:** a form that always works out both sides.

## `while`

```
while condition:
    body
end while
```

where `condition` is checked before each pass, and `body` is run again as long as it holds (D19).

```
var tries: Whole = 0;
while tries < 5:
    show_msg("Guess!");
    tries = tries + 1;
end while
```

## `break`

`break;` leaves the innermost loop it's in. There's no `continue`: ending one pass early is an `if` around the rest
(D32).

```
while tries < 7:
    let guess: Whole = be.Whole(ask("Your guess: ")) ?? 0;
    tries = tries + 1;
    if guess < secret:
        show_msg("Too low");
    else if guess > secret:
        show_msg("Too high");
    else:
        show_msg.fmt("Correct! You got it in {tries} tries.");
        break;
    end if
end while
```

## Running several things at once

A Plainly program can run several parts of itself at once (D66), in two ways (D67):

1. **Tasks on one thread, the everyday way:** tasks take turns, each handing over only at points it chooses, like
   waiting for a key or a timer, so nothing changes behind a task's back and the turns go the same way every run.
2. **Real threads on several cores, opt-in,** for speed.

Real threads are **workers** (D68):
- **Each worker has its own variables;** nothing changeable is shared between two workers.
- **Workers talk by sending values,** which arrive as copies.
- **`share` can't cross from one worker to another,** and trying is refused while building, so data races are
  impossible.
- **Values that can never change, like a `let`, may be read by every worker** without copying.

Messages from several workers can still arrive in different orders between runs, and two workers each waiting on the
other freeze (D68).

**Not decided:** the words for starting a task, handing over, starting a worker, sending and receiving.

## After an `if` that leaves

When any way through an `if` leaves, by `return`, `break` or a failure, nothing may follow its `end if` in the same
block, and code that should run afterwards goes in the ways that don't leave (D9). That covers:

- **every way leaving, with no `else`,** the guard clause;
- **only some ways leaving,** where code after `end if` would run only after the others;
- **every way leaving, `else` included,** where code after `end if` could never run.

Each is refused while building (D9).

```
proc larger(a: Whole, b: Whole) returns Whole:
    if a > b:
        return a;
    else:
        return b;
    end if
end proc larger
```

**Not decided:** failing on purpose, other loops, leaving an outer loop, and every other part of control flow. `return`:
chapter 4.
