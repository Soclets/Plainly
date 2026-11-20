# 8. Failures

A **failure** is something a procedure can give back instead of its result, like text that isn't a number. A procedure
that can fail says so, and every call to it must deal with the failure, or the program is refused while building
(D23).

```
let $guess: Whole = be.Whole($typed);
```

On its own, that line is refused, since `be.Whole` can fail and nothing handles it.

## `?`

`?` passes the failure up to whoever called this procedure, which must list that failure itself (D23):

```
let $level: Text = load_text("level1.txt")?;
```

## `??`

`??` handles the failure on one line (D23):

```
call ?? value
call ?? ignore
call ?? crash
```

where `call` is the call that can fail, and `value` the value to carry on with instead.

```
let $guess: Whole = be.Whole($typed) ?? 0;
save_text($line, "save.txt") ?? ignore;
load_level($name) ?? crash;
```

- **`?? value`** carries on with that value.
- **`?? ignore`** ignores the failure on purpose.
- **`?? crash`** stops the program on purpose.

## `otherwise:`

`otherwise:` handles each failure in its own `case`, each ending in a fallback value, `return`, `break`, or failing on
purpose (D23).

**Not decided:** the exact form of the `otherwise:` block and its cases; how a procedure lists its failures; failures'
names; how a procedure fails on purpose.
