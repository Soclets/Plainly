# PlainCompiler: progress

The state of PlainCompiler, kept current as it's built. A fresh session starts by reading this file, `git log`, and
the test results (`python3 compiler/run_tests.py`).

## Status (2026-11-20)

**Milestone 1 is done.**

- **All 21 tests pass** on `plainly run --working`, run from inside each test's folder (tests/README.md), including message tests for D85, D87 and D88.
- **The guessing game builds and plays** (`plainly compile examples/guessing_game.plainly --working`): played by hand
  with a binary search (won in 6 tries), with every guess too low (7 tries, then "Out of tries! It was 97."), and with
  `abc` as a guess (treated as 0, as `?? 0` says).
- `cargo fmt --check` and `cargo clippy --workspace --all-targets` are clean. 43 Rust unit tests pass, among them one
  per refusal, named after its decision (`plain_compiler/src/tests.rs`); they check what's refused and where, never
  the wording.
- Every stage of D62 exists: tokenizer and parser (syntax tree), checker (typed tree), lowering (Plainly IR), code
  generation (Cranelift IR and native code), linking with the runtime.
- D55 is built in full, rule 4 included: the first error's fix is applied in memory, the program checked again, and
  each error that disappears gets a `because:` pointing back at the first one (`knock_ons.rs`).
- Blank lines anywhere, one at a time (D97); all spacing fixed (D98); names with digits (D99).
- `be.Whole` reads numbers as players type them (D95), and `ask` ends the program quietly when input runs out (D96).
- A variable ends with its block (D93); a name in use can't be declared again, and a variable can't take a
  procedure's name (D94).
- `plainly compile` replaces `plainly build` (D91), and messages say "compile time" and "runtime" (D90).
- Follows the decisions up to D83: `??` sticks to its call, and comparison chains and operators after a step are
  refused with their fixes (D81); exit statuses 6 and 7 (D82); knock-ons as D83 confirms.
- Before that: maths order (D72) and `|>` last (D73) are built; `/`, `div`, `rem`, comments,
  lists, methods, `copy` and `share` (D49, D70 to D77) stop with `NotBuiltYet`; `""` is accepted (D76).

## Milestone 1: every test and the guessing game, in `working` mode

### Plan

1. ~~**Test runner** (`compiler/run_tests.py`)~~ done.
2. ~~**Hello world end to end**~~ done.
3. ~~**Text:** `show_msg.same_line`, `&`, `\` escapes, `.fmt`~~ done.
4. ~~**Whole and variables**, with overflow checks~~ done.
5. ~~**Control flow:** `if`, comparisons, `while`, `break`, `and`, `or`, `not`~~ done.
6. ~~**Procedures:** shorthand parameters, `returns`, `return`, labels, slides~~ done.
7. ~~**Asking:** `ask`, `be.Whole`, `??` with a value, `random`; the guessing game~~ done.
8. ~~**Finish:** fmt, clippy, this file~~ done. Q1 was answered by D72: maths order is school order.

### Next

Milestone 2 isn't set yet; that's Tankun's call. Decided pieces that would come next, smallest first:
- comments (D74), so games can carry notes;
- dividing and remainders (D70, D71), with their checks for a remainder in `/` and dividing by zero;
- `?`, `?? crash`, `?? ignore`, `otherwise:`, failures in headers, handlers (D23, D39 to D44);
- lists, methods, `copy` and `share` (D49, D75 to D78);
- `final` mode (D46), `plainly oracle` (D50), the cache and parallel compiling (D65);
- the real PlainGC, ZGC-style (D69), its own piece of work.

## How to run it

```
cd compiler && cargo build --workspace
compiler/target/debug/plainly run tests/d04_hello_world/program.plainly --working
python3 compiler/run_tests.py            # every test
python3 compiler/run_tests.py d16_if     # one test
```

`plainly` finds the runtime library, `libplain_runtime.a`, in its own folder, so build the whole workspace. Linking
uses the system's C compiler, `cc`.

## The pieces

| Crate | What it is |
|---|---|
| `plain_compiler` | the `plainly` command and every stage of D62 |
| `plain_runtime` | the Rust code compiled programs call: `show_msg`, `ask`, `be.Text`, `be.Whole`, `random`, text joining, the overflow check's message |
| `plain_gc` | PlainGC's interface, `allocate`, with the simplest correct version behind it: memory is never freed, like Java's Epsilon collector. The real ZGC-style PlainGC (D69) is a later milestone of its own |

Inside `plain_compiler`: `tokenizer.rs`, `parser.rs` (syntax tree, with indentation and spacing checks), `checker.rs`
(typed tree and refusals), `lowering.rs` (Plainly IR), `code_generation.rs` (Cranelift), `linking.rs`, `message.rs`
(D54's layout), `main.rs` (the command).

## What `NotDesignedYet` covers

The compiler stops with `NotDesignedYet` (exit status 3) wherever the standard is silent or says "Not decided". Each
place is an open question below. Briefly: `??` after a slide's step or a call that can't fail; letters beyond English in names; a statement over several lines (its later lines' indentation);
two statements on one line; comparing text; conditions that aren't `Bool`; a procedure named like a library one; a call whose value isn't used; `??` on a call that can't fail; label order;
sliding into a labelled first parameter; `.fmt` given anything but text written in the call; `random` with `from:` above `to:`; types other
than `Whole`, `Bool` and `Text`; lists; anything outside a procedure.

## What `NotBuiltYet` covers

Decided, but not in this milestone: `--final` mode, `plainly oracle`, families (`fam`), `extern.rust`, failures in a
header (`fails`), `?`, `?? ignore`, `?? crash`, `otherwise:`, `case`, `instead`, `include`, handlers and `.*`; `/`,
`div` and `rem` (D70, D71); comments (D74); lists (D75), their facts like `.len` (D79) and items (D80); methods (D77); `copy` and `share` (D49).

## Messages waiting for wording (D54)

Every wording below is a draft. All use D54's layout: the kind, the place twice, the source line with `^` under the
part in question, then `fix:` or `what:`, `when:`, `because:` and `why:` where known. Every `fix:` shows the whole
corrected line, in backticks (D84). A sample:

```
refused: game.plainly:7:5, line 7 column 5
 7 |     lives = lives - 1;
   |     ^^^^^
 what:    `lives` can't be given a new value
 because: line 6 declares it with `let`
 6 |     let lives: Whole = 3;
   |     ^^^
 why:     only a `var` can be given a new value.
```

**Approved in D85,** word for word as drafted: a missing `;` (D7); a wrong closer, `;` after a closer, a missing
closer before a line, and a missing `:` on an opening line (D8); indentation and tabs (D10); a declaration with no type
(D13); `=` on a `let`, without its `because:` line, and `=` in a condition (D14); missing spaces around `<`, `>`, `+`,
`-` and `*` (D17, D27, D59); spaces around a narrowing's `.` (D18); single quotes (D21). Each is marked
`// trace: D85` in the code.

**Approved in D87,** word for word as drafted: the `because:` for `=` on a `let`; the file ending before a closer
(D8); a missing `:` or `=` in a declaration (D13); comparing two types (D17); converting to the type a value already
is, and `be` with no type (D22); an unhandled call that can fail (D23); `+` on two texts (D27, D28); a number joined
with `&` (D28); a keyword as a name (D58). Each is marked `// trace: D87`.

**Approved in D88,** word for word as drafted: `( )` on a definition with no parameters and parameters on `proc.main`
(D4); `()` on a bare step (D5); the missing, unwanted and wrong label messages and the argument count (D24); using
what a no-`returns` procedure gives, and `return value;` without `returns` (D38); the four `return` messages (D57); a
line after an `if` that can leave (D9); `break` outside a loop (D32). Each is marked `// trace: D88`.

**Approved in D89,** with "but" wherever a `what:` sets what's expected against what was found, which also changed
D87's comparison and D88's argument count: a slid value of the wrong type (D5); the value after `??` of the wrong
type (D23); the four `labels` messages (D25); `-` on something that isn't a number, and a space after a negating `-`
(D27); maths on anything but two `Whole`s (D27, D59); a `Bool` joined with `&`, with its own `why:` (D28);
`show_msg.same_line.fmt` (D31); both parameter forms (D61); `&` next to maths (D72). Each is marked `// trace: D89`.
"But" is used the same way in the drafts still waiting.

**Approved in D92:** the `.fmt` messages (D29); two `proc.main`s, two procedures with one name, and no `proc.main`
(D64); an unknown variable or procedure, a procedure without `( )`, a variable called, a value or argument of the
wrong type; the parser's three messages; overflow at runtime (D46); `internal error:`, `can't read:` and the missing
mode. Each is marked `// trace: D92`.

**Still waiting for wording:**
- two parameters with one name; a narrowing written twice; an unknown narrowing of `show_msg` or `be.Text`; anything
  after an `if`'s `else:` part but `end if`;
- D81's three: two comparisons in a row, an operator after a slide's step, `??` not after a call;
- D98's spacing, one message per mistake, each `fix:` the line with that one gap corrected. `why:` by kind:
  `` `==` has one space on each side. `` (any operator but D85's five, which keep theirs);
  `` `,` and `2` have one space between them. ``; `` no space goes between `(` and `"three"`. ``;
  spaces never come two in a row, outside indentation.; a line never ends with spaces.; a blank line has no spaces
  on it. A word followed by a spaced `(`, as in `show_msg ("a")`, is read as a call, so its space is refused like
  any other.
- D99's names: what: `` `Score` has a capital letter, and names have none `` (or `` starts with a digit ``,
  `` starts with `_` ``, `` ends with `_` ``, `` has two `_` in a row ``); why: a name is lowercase letters and digits,
  starting with a letter, with `_` only between words.
- D97's blank lines in a row: no line to correct, so `fix:` names the lines, as in `remove blank lines 4 and 5,
  leaving one` (D84 has `fix:` show the whole corrected line); why: one blank line is enough to separate the parts of
  a program.
- D94's two: a name declared again while in use (what: `` `guess` is already in use here ``; because: `` `guess` is
  declared here ``; why: a name in use can't be declared again; two things get two names.), and a variable or
  parameter with a procedure's name (what: `` `roll` is already a procedure's name ``; because: `` the procedure
  `roll` is declared here ``; why: `` a variable can't take a procedure's name, since `roll` and `roll()` would look
  alike. ``);
- the `because:` on a variable used after its block (D93): `` `note` is declared inside a block that has ended ``,
  with the declaration shown;
- the `because:` on a knock-on error (D83): "it follows from the error at line N column M";
- `internal error:`'s first line, "PlainCompiler itself went wrong, not your program";
- the other command mistakes: no command, `x` isn't a command, no file, two files, two modes, `check` needs no mode,
  `x` isn't a mode; and `can't read:` with "a Plainly file's name ends in `.plainly`" or "it isn't UTF-8 text";
- every `NotBuiltYet` and `NotDesignedYet` message's `what:`, at compile time and at runtime (D56 leaves their wording
  open), and where a runtime message starts when the program's line is still open (Q23).

### Refusals

| Decision | Refused | `fix:` or `what:` | `why:` |
|---|---|---|---|
| D4 | `( )` on a definition with no parameters | fix: the line without `()` | a definition writes `( )` only when it has parameters. |
| D4 | parameters on `proc.main` | what: `proc.main` takes nothing | only the program's start runs `proc.main`, so it's never given anything. |
| D5 | `()` on a step with nothing else to pass | fix: the step bare | a step with nothing else to pass is written bare, since the slide gives it its value. |
| D5 | the slid value's type is wrong | what: `` `f` takes a `Whole` first, but the slid value is a `Text` `` | a slide passes the value on its left in as the step's first argument. |
| D7 | a missing `;` | fix: the line with `;` | every statement ends with `;`. |
| D8 | a wrong closer, or `;` after one | fix: the right closer | a closer is `end` and what it closes. / a closer stands alone, with no `;`. |
| D8 | a missing closer | what: `` `end if` is missing before this line `` or `` the file ends before `end proc.main` `` | every block ends with `end` and what it closes. |
| D8 | a missing `:` on an opening line | fix: the line with `:` | a block's opening line ends with `:`. |
| D9 | a line after an `if` that can leave | what: this line follows an `if` that can leave; because: the `if` on line N can leave | when any way through an `if` leaves, nothing may follow its `end if`; code that should run afterwards goes in the ways that don't leave. |
| D10 | wrong indentation, or a tab | fix: the whole corrected line | each level of indentation is 4 spaces, and tabs aren't used. |
| D13 | a declaration with no type | what: `` `x` needs its type, written after a `:` `` | a declaration always writes the variable's type. |
| D13 | a missing `:` before the type | fix: the line with `:` | a declaration writes `:` between the variable's name and its type. |
| D13 | a missing `=` before the first value | fix: the line with `= ` | a declaration gives its variable a first value after `=`. |
| D14 | `=` on a `let` | what: `` `lives` can't be given a new value ``; because: line N declares it with `let` | only a `var` can be given a new value. |
| D14 | `=` in a condition | fix: the line with `==` | `=` gives a variable a value, and is never part of a condition; `==` compares. |
| D17, D27, D59 | `<`, `>`, `+`, `-`, `*` without a space each side | fix: the spaced line | `<`, `>`, `+`, `-` and `*` between two values have a space on each side. |
| D17 | comparing two types | what: `` `==` compares two values of one type, but this is a `Whole` and a `Text` `` | a comparison is between two values of the same type. |
| D18 | spaces around a narrowing's `.` | fix: the joined line | a narrowing's `.` touches the words on both sides. |
| D21 | text in single quotes | fix: the line with double quotes | text is written in double quotes, never single ones like `'hi'`. |
| D22 | converting to the type it already is | what: `` this value is already a `Text`, so `be.Text` does nothing `` | converting a value to the type it already is does nothing, so it's refused. |
| D22 | `be` with no type | what: `` `be` needs the type the value becomes, as in `be.Text` `` | a value is converted with `be` narrowed by the type it becomes. |
| D23 | a call that can fail, unhandled | what: this call can fail, and nothing here deals with the failure | every call that can fail deals with its failure, with `?`, `??` or `otherwise:`. |
| D23 | the value after `??` has the wrong type | what: `` the call gives a `Whole`, so the value after `??` must be one too, not a `Text` `` | a value's type has to match where it's used. |
| D24 | a missing label | fix: the line with `from: ` | every call writes the labels its procedure chose. |
| D24 | a label where there's none | fix: the line without it | a call writes only the labels its procedure chose. |
| D24 | a wrong label | what: `` this argument's label is `to`, not `upto` `` | every call writes the labels its procedure chose. |
| D24 | too few or too many arguments | what: `` `pick` takes 2 values, but this call gives 1 value ``; because: `` `pick` is declared here `` | a call gives every parameter a value, with its label if it has one. |
| D25 | `labels` not on the first line | what: a `labels` line can only be the first line of a procedure's body | a procedure declares its labels on the first line of its body. |
| D25 | a label for a parameter that isn't there | what: `` `pick` has no parameter named `lo` `` | a `labels` line gives labels to the procedure's own parameters. |
| D25 | two labels on one parameter, or one label twice | what: `` `low` already has a label `` / `` two parameters have the label `from` `` | each parameter has at most one label. / each label names one parameter, so a call can say which value is which. |
| D27 | `-` on something that isn't a number | what: `` `-` makes a number negative, but this is a `Text` `` | a `-` with no space after it makes a number negative. |
| D27 | a space after a negating `-` | fix: the line without it | a `-` that makes a value negative has no space after it. |
| D27, D28 | `+` on two texts | fix: the line with `&` | `+` is only for numbers, and `&` joins text. |
| D27, D59 | `+`, `-`, `*` on anything but two `Whole`s | what: `` `+` works on two `Whole`s, but this is a `Bool` and a `Whole` `` | `+`, `-` and `*` are only for numbers. |
| D28 | a number joined with `&` | fix: the line with `be.Text(...)` around it | `&` joins two pieces of text, and a number becomes text only where `be.Text` says so. |
| D28 | a `Bool` joined with `&` | what: `` `&` joins text, but this is a `Bool` `` | (same) |
| D29 | a brace that isn't a variable | what: `` there's no variable named `nme` here `` | braces in `.fmt` text hold a variable's name, checked at compile time. |
| D29 | a calculation or a call in braces | what: braces in `.fmt` text hold only a variable's name | text never hides a calculation or a call; work it out into a variable first. |
| D29 | `.fmt` with no text, or with a label | what: `` `.fmt` takes one piece of text `` / `` `.fmt`'s text has no label `` | `.fmt` fills in the braces of the text it's given. |
| D31 | `show_msg.same_line.fmt` | fix: the line with `show_msg.fmt.same_line` | narrowings go strongest first: `.fmt` changes what's shown, `.same_line` only where it ends. |
| D32 | `break` outside a loop | what: this `break` isn't inside a loop | `break` leaves the innermost loop it's in. |
| D38 | using what a no-`returns` procedure gives | what: this call gives nothing back, so it can't be used as a value | a procedure with no `returns` gives nothing back. |
| D38, D57 | `return value;` with no `returns` | what: `` `greet` has no `returns`, so it gives nothing back `` | only a procedure with `returns` gives back a value. |
| D57 | `return;` with `returns` | what: `` `pick` gives back a `Whole`, so `return` needs one `` | in a procedure with `returns`, `return` gives back a value of that type. |
| D57 | a way through with no `return` | what: `` some way through `pick` reaches its end without `return` giving back a `Whole` `` | every way through a procedure with `returns` ends in `return` and a value. |
| D57 | a line after `return` | what: this line comes after a `return` in the same block, so it can never run; because: the `return` is here | nothing may follow `return` in the same block. |
| D57 | `return;` as the last line | what: a bare `return;` as a procedure's last line does nothing | a procedure ends there anyway; `return;` is for leaving early. |
| D58 | a keyword as a name | what: `` `if` is a keyword, so it can't be a name `` | a name can't be one of Plainly's own words. |
| D61 | the long form for parameters sharing one type, or the short form for one parameter | fix: the line in the other form | parameters sharing one type write it once, in front, when there are two or more; otherwise each writes its own. |
| D61 | both forms in one header | what: a header writes one type in front for every parameter, or a type after each, never both | parameters sharing one type write it once, in front; otherwise each writes its own. |
| D64 | two `proc.main`s, two procedures with one name | what: this program has two `proc.main`s / `` there's already a procedure named `pick` ``; because: the first is here | a program has exactly one `proc.main`, where it starts. / every name in a program shares one space, so two procedures can't have one name. |
| D64 | no `proc.main` | what: this program has no `proc.main` (shown at line 1 column 1 of the first file) | a program starts with `proc.main`. |
| D81 | two comparisons in a row | fix: the line with `and` and the middle value again, as `1 < x and x < 10` | two comparisons in a row are refused; `and` joins them, naming the middle value twice. |
| D81 | an operator after a slide's step | fix: the line with brackets around the slide, as `(x \|> f) + 1` | a slide's step is a procedure, so an operator after it goes outside brackets around the slide. |
| D81 | `??` after something that isn't a call | what: `` `??` here doesn't follow a call that can fail `` | `??` deals with the failure of the call right before it. |
| D72 | `&` next to an unbracketed `+`, `-` or `*` | what: `` `&` is next to maths without brackets, so it isn't clear which goes first ``; because: the maths is here | where maths doesn't say which goes first, `( )` shows it. |
| (D3, D4) | an unknown variable or procedure, a procedure used without `( )`, a variable called | what: `` there's no variable named `x` here `` / `` there's no procedure named `x` `` / `` `x` is a procedure, so using it writes `( )` with its arguments `` (fix: `x()` when it takes nothing) / `` `x` is a variable, not a procedure `` | a variable is declared with `let` or `var` before it's used. / a call names a procedure the program or the library has. / a call writes the procedure's name and `( )`. / only a procedure can be called. |
| (any) | something the parser can't read | what: `` `;` should be here, not `x` `` / the line ends where `` `)` `` should be / the file ends where a value should be | (none) |
| (any) | a value of the wrong type | what: `` `x` is a `Whole`, but this value is a `Text` `` (fix: `be.Text(...)` when a `Whole` is given for `Text`) | a value's type has to match where it's used. / a number becomes text only where `be.Text` says so. |

### Checks at runtime (`check failed:`, exit status 4)

| Decision | Check | `what:` | `when:` | `why:` |
|---|---|---|---|---|
| D46 | `Whole` overflow in `+`, `-`, `*` or a negating `-` | the result is too big for a `Whole` / the result is too small for a `Whole` | `9223372036854775807 + 1`, `-(-9223372036854775808)` | a `Whole` holds whole numbers from -9223372036854775808 to 9223372036854775807. |

The `^` is under the whole calculation, like `tries + 1`.

### The implementation's own messages

| Kind | When | Draft |
|---|---|---|
| `NotDesignedYet:`, `NotBuiltYet:` (D56) | as listed above | `what:` says what isn't decided or built, as in "maths order isn't decided, so more than one of `+`, `-` and `*` needs brackets for now" |
| `internal error:` (status 7, D82) | PlainCompiler itself goes wrong | `what:` the problem; `why:` this is a bug in PlainCompiler. Please report it at https://github.com/Soclets/Plainly/issues, with the program that caused it. |
| `can't read:` (status 2) | the file can't be read | `can't read: game.plainly` then `what:` the system's reason |
| `plainly:` (status 6, D82) | a mistake in the command | `plainly: run and compile need a mode: --working or --final`, then the usage lines |

## Open questions for the design session

Numbered so they can be referred to. None of these is answered by the compiler; each stops with `NotDesignedYet` or
uses a placeholder until it's decided.

- **Q1. Answered by D72:** maths order is school order, so `d24_procedures` stands and passes.
- **Q2. Answered by D82:** status 7 for a bug in PlainCompiler itself.
- **Q3. Answered by D82:** status 6 for a command written wrong.
- **Q4. The message for a file that can't be read** (status 2): its kind word, drafted as `can't read:`, and its
  layout, since there's no line to show.
- **Q5. The exit status when a program has both refusals and `NotDesignedYet`.** It's 1 for now.
- **Q6. Answered:** each session signs with its own trailer, `compiler claude` here (CLAUDE.md).
- **Q7. Answered by D97:** blank lines anywhere, at most one in a row; more are refused with a fix that removes the
  extra ones. Whether a blank line may hold spaces is part of Q8.
- **Q8. Answered by D98:** the spacing every example uses is the rule, and anything else is refused with the
  corrected line as the fix.
- **Q9. Answered by D99:** lowercase letters and digits, starting with a letter, with `_` only between words;
  letters beyond English stay `NotDesignedYet`.
- **Q10. Answered by D72, D73 and D81.** `??` sticks to the call before it; `a < b < c` and `x |> f + 1` are refused
  with their fixes.
- **Q11. Answered by D93:** a variable ends with its block; using it after is refused with "there's no variable
  named `x` here", and the name can be declared again.
- **Q12. Answered by D94:** a name in use can't be declared again, and a variable can't take a procedure's name;
  both refused. A program's procedure named like a library one (`ask`, `random`, `show_msg`) is still
  `NotDesignedYet`.
- **Q13. Answered by D95:** spaces around the number are ignored, and a `+` or `-` and leading zeros are fine;
  anything else fails, a number too big or too small for a `Whole` included. The failure's name is still open.
- **Q14. Answered by D96:** when input runs out, `ask` ends the program quietly with status 0, and a last line with
  no Enter still counts as an answer.
- **Q15. A call whose value isn't used,** like `random(from: 1, to: 6);` alone; and `?? ignore` on a call that gives
  a value.
- **Q16. Code after `break;` in the same block.** D57 refuses code after `return`, and D9 code after an `if` that can
  leave; a line straight after `break;` isn't covered. `NotDesignedYet`.
- **Q17. What the command's file means, and where `compile` puts its file.** D64 makes a program every `.plainly` file
  in its folder, so `plainly run games/guess.plainly` compiles every `.plainly` file in `games/`. `plainly compile`
  writes the executable into the current folder, named after the file (`guess`).
- **Q18. Answered by D84:** `fix:` shows the whole corrected line, in backticks so its indentation shows.
- **Q19. Answered by D83:** each knock-on stays a message of its own, with a `because:` pointing back to the first
  error. The `because:` line's wording comes with the wording batches.
- **Q20. Answered:** fixed in standard chapter 11.
- **Q21. Giving a parameter a new value** with `=`. `NotDesignedYet`.
- **Q22. `??` after a call that can't fail.** `NotDesignedYet`.
- **Q23. A message after an open line.** When the program stops while a line is still open, as after
  `Your guess: `, the message starts on that same line. Should it start on a new line?
- **Q24. Smaller text questions:** line breaks written as `\r\n` (Windows); `\` before a letter and a tab inside
  text (D30's notes). All `NotDesignedYet`. (Empty text `""` is decided by D76.)
- **Q25. Columns in messages** count characters, so `é` counts as one (D54's note says it isn't decided).
- **Q26. Conditions that aren't `Bool`, and comparing `Bool`s with `<` or `>`.** `NotDesignedYet`; `==` and `!=` on
  `Bool`s are accepted, as D34's `cheat == true` shows.
- **Q27. What the runtime does when standard output can't be written,** or memory runs out. `NotDesignedYet`.
- **Q28. Standard chapter 7** still lists maths order under "Not decided", though D72 decides it.
- **Q29. `??` after a slide's step,** as in `typed |> be.Whole ?? 0`. D81 gives `??` to the call right before it,
  and a step isn't written as a call. `NotDesignedYet`.
- **Q30. Answered by D86:** the text after every label starts one space after `because:`, in every message.
