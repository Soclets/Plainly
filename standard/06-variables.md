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

**Not decided:** how a `var` is changed; constants known while building, like `const`; the types themselves; declaring
a variable without a first value.
