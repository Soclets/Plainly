# 9. Library

Procedures that come with C+. The library targets the terminal: showing text, asking, single key presses,
colours, drawing at a position and clearing the screen, on Windows, macOS and Linux. A GUI, if ever, is a
separate package (D63). What the terminal part is built on isn't decided since D131 moved the runtime to C++.

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

**Not decided:** seeding; randomness for secrets; `from:` above `to:`; random numbers of other types.
