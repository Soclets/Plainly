# 9. Library

Procedures that come with Unio. The library targets the terminal: showing text, asking, single key presses,
colours, drawing at a position and clearing the screen, on Windows, macOS and Linux. A GUI, if ever, is a
separate package (D63). What the terminal part is built on isn't decided since D131 moved the runtime to C++.

## `range`

A range describes a run of whole numbers, written in `[ ]` since nothing runs (D165, D170):

```
range[from: 1, to: 3]
range[from: 0, before: 10]
```

- **`from:`** is the first number.
- **Exactly one of `to:` or `before:`** says where it ends: `to: 3` includes 3, giving 1, 2, 3, and `before: 10` stops
  short of 10, giving 0 to 9, as for going through every `offset:` (D170).

**Not decided:** what a range is used in, a `for` loop being the likely one; a step, as `step_by:`; counting down.

## `random`

```
random(from: $low, to: $high)
```

where `$low` and `$high` are whole numbers. It gives a whole number from `$low` to `$high`, both included, each equally
likely (D26).

```
let $secret: Whole = random(from: 1, to: 100);
```

It's game randomness, fine for dice and loot, not for anything that must be unguessable (D26).

`from:` may equal `to:`, giving that number. `from:` above `to:` is refused at compile time when both are plain numbers,
with the two swapped as the fix, and otherwise stops at runtime with `check failed:` and status 4, showing both values
(D160).

**Not decided:** seeding; randomness for secrets; random numbers of other types.
