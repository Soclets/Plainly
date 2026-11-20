# 6. Variables

## Declaring

A new variable is declared with `let` or `var`, its name, its type and its first value (D13):

```
let $name: Type = value;
var $name: Type = value;
```

where `$name` is the variable's name (D11), `Type` its type, and `value` its first value. `let` or `var`, the `:`, the
`=` and the `;` are written as they are.

```
let $secret: Whole = 42;
var $tries: Whole = 0;
```

- **`let`:** the value never changes after this line (D13).
- **`var`:** the value can change. `var` is short for variable (D2, D13).
- **The type is always written,** never worked out from the value (D13).

## Changing

A `var` gets a new value with its name, `=` and the value, as a statement of its own (D14):

```
$name = value;
```

where `$name` is a `var`'s name and `value` its new value.

```
var $tries: Whole = 0;
$tries = $tries + 1;
```

- **`=` on a `let` is refused,** since a `let` never changes (D13, D14).
- **`=` never appears inside a condition,** so `if $tries = 3:` is refused (D14).

## `copy` and `share`

When a list or a record is given to a new name, the line says `copy` or `share`, with no default (D49):

```
var $b: List<Whole> = copy $a;
var $c: List<Whole> = share $a;
```

Changing `$b` leaves `$a` alone; changing `$c` changes `$a`. A plain value like a `Whole` needs neither word (D49).

**Not decided:** whether arguments and changes with `=` need a word; shorthand like `+=`; constants known while building, like `const`; the types themselves; declaring
a variable without a first value.
