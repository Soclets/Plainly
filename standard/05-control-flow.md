# 5. Control flow

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

**Not decided:** `if` itself, `else`, comparisons, `return`, `break`, failures, and every other part of control flow.
