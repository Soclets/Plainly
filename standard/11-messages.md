# 11. Messages

```
refused: game.plainly:5:5, line 5 column 5
 5 |     $lives = $lives - 1;
   |     ^^^^^^
 what:    `$lives` can't be given a new value
 because: line 2 declares it with `let`
 2 |     let $lives: Whole = 3;
   |     ^^^
 why:     only a `var` can be given a new value.
```

- **The first word is the kind:** `refused:` while building, `check failed:` while running (D54).
- **The place is written twice:** `file:line:column` for tools, then `line N column M` for people (D54).
- **The source line is shown,** with `^` under exactly the part in question (D54).
- **Then labelled lines, each only when known, in this order** (D54):
  1. `fix:` when there's exactly one fix, otherwise `what:`;
  2. `when:`, the values that broke a check while running;
  3. `because:`, the earlier place the error comes from, as a sentence, then that line with `^` under the cause;
  4. `why:`, the rule, as a plain sentence.
- **No error codes, and no "did you mean" guesses** (D54).
- **Every error is shown** (D54).

## One mistake, one message

1. **After an error, only that statement is skipped,** and checking carries on (D55).
2. **A refused declaration still creates its variable** with its written type (D54, D55).
3. **A refused expression's type becomes unknown,** and nothing is refused for using an unknown value (D55).
4. **Knock-on errors are sorted out:** the first error's single fix is applied in memory and the program checked
   again, and anything that disappears is shown under a `because:` pointing back at it (D55).
5. **A broken block opener:** checking carries on into the body if it's clear what was meant, and stops there if it
   isn't (D55).

**Not decided:** each message's wording; how columns are counted in text beyond plain English letters; exit statuses.
