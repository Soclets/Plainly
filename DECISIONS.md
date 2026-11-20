# Plainly decisions

Every choice Tankun makes about Plainly, in the order it was made. Numbers
are never reused or renumbered; a changed decision is a new entry that says
which one it replaces.

Each entry: what was chosen, the alternatives weighed, why this one won, and
Tankun's own words.

## D1. Licences: Apache-2.0 WITH LLVM-exception for code, CC BY 4.0 for writing

2026-11-20.

**Chosen.** Plainly's code is under Apache-2.0 WITH LLVM-exception
(`LICENSE`). Its writing, meaning the Markdown files (README, this log, and
any design docs to come), is under CC BY 4.0 (`LICENSE-CC-BY-4.0`), credited
as `Plainly GitHub repo text: https://github.com/Soclets/Plainly`. This copies
Cyborg's D262, which copied CyborgPL; Lyftar uses the same pair.

**Weighed.**
- Plain Apache-2.0, the licence on all of Tankun's other language repos at
  first glance, and v1's. It has a patent promise (contributors can't sue
  users over patents on their code), but it clashes with GPLv2-only software,
  and arguably puts a notice requirement on every compiled program that has
  Plainly's runtime or standard library inside it.
- MIT. Short and plain, but no patent promise, and it too asks for its notice
  wherever its code goes, compiled programs included.
- MIT OR Apache-2.0, the user's choice, as Rust does. It fixes the GPLv2
  clash, but not the notice requirement in compiled programs. The design
  claude first argued against it as "two licences to explain, against one
  word for a thing"; Tankun rejected that argument ("That's like saying only
  C should exist"), rightly: one word for a thing is a rule about Plainly's
  code, not its licensing, and the argument was withdrawn.
- GPL. Forces work built with the tools to stay open source; wrong for a
  language Tankun wants people to adopt.

**Why this won.** Plainly is compiled, so pieces of Plainly's own code (the
code behind `show_msg`, say) end up inside every program people build. The
LLVM exception says that embedded code owes nothing, no notice and no credit,
and removes Apache-2.0's clash with GPLv2, while keeping the patent promise.
None of the other options does all three. Writing gets its own licence
because software licences fit prose badly; CC BY 4.0 lets anyone reuse the
docs and design reasoning with credit. Tankun asked the Cyborg design claude
("Mf, just ask Cyborg design claude"), which explained D262 and advised taking
the LLVM exception for any language whose runtime ends up in users' programs,
and adding the licence files before a repo is public (Cyborg sat public and
unlicensed for five days; Plainly for a few minutes).

**Tankun's words.** Asked "Go with that?", the owner chose: "yes".

## D2. Every keyword has one spelling, the short one, and the docs teach the full word

2026-11-20.

**Chosen.** Code has one spelling for each keyword, the short form: `proc`, `var`, `msg`, never `procedure`,
`variable` or `message`. The docs teach the full word once, where a keyword first appears ("`proc` is short for
procedure"), and so do error messages, so a beginner learns the concept and then types it short.

```
proc.main:
    show_msg("Hello, World!");
end proc.main
```

(Hello world's other parts, `.main`, `show_msg`, `;` and `end`, aren't decided yet.)

**Weighed.**
- Both forms, long and short, as in Plainly v1 (its D28: `procedure` or `proc`, `mutable` or `mut`). It let
  beginners start with full words, but anyone reading other people's code met the short ones anyway, so beginners
  learnt both, and teams argue over which to write.
- Only the long forms. Plainest to read, but long to type everywhere.

**Why this won.** One spelling per thing means every program looks the same, a search for `proc` finds every
procedure, and nobody learns two words for one thing. The short forms are clipped words, not symbols, so they stay
plain once taught. The cost is that a beginner sees `proc` before knowing what it stands for, which the docs and
messages make up for by teaching the full word at first sight.

**Tankun's words.** From multifinity's chat (Nov 19): "So, learn concepts, then type short" and "teach the full
words, and remember the shorten". Asked "Only the short form in code, with the docs teaching the full word once?",
the owner chose: "yea".

## D3. `( )` holds a procedure's parameters and a call's arguments, `[ ]` is for lists, `< >` for types

2026-11-20.

**Chosen.** Each kind of bracket has one job:
- `( )` for procedures, in their definition and in calls, and for grouping maths;
- `[ ]` for lists, as in `[1, 2, 3]` and reaching an item;
- `< >` for filling in a type, as in `List<Whole>`.

```
proc add($a: Whole, $b: Whole) returns Whole:
    return $a + $b;
end proc add

add(1, 2);
```

(`$`, `returns`, `return` and the types here aren't decided yet.)

**Weighed.**
- `[ ]` for a definition's parameters and `( )` for calls, as in Plainly v1 (its D51: `[ ]` holds values and runs
  nothing, `( )` runs something), so a definition can't be mistaken for a call.

**Why this won.** In v2, `proc` in front already marks a definition, so the brackets don't need to as well. Python,
and nearly every language a beginner meets next, writes `( )` in both places, which fits "static Python". It also
leaves `[ ]` to lists alone, so each bracket means one thing. What's lost is v1's signal that `[ ]` never runs
anything: `range(from: 1, to: 5)`, say, now looks like the call it is.

**Tankun's words.** From multifinity's chat (Nov 19): "If kids ask why, then I'll probably say there's maybe not a
real reason. So, maybe switch to `()`? `proc add()` and `add()` don't look the same". Asked "`( )` for parameters and
calls?", the owner chose: "yea".

## D4. A definition that takes nothing has no brackets, and every call writes `()`

2026-11-20.

**Chosen.** A procedure's definition writes brackets only when it has parameters. A call always writes `( )`, empty
or not. `proc.main` is never called, since only the runtime starts it, so it has no call form.

```
proc.main:
    show_msg("Hello, World!");
end proc.main

proc roll_dice returns Whole:
    ...
end proc roll_dice

$roll: Whole = roll_dice();
```

(`.main`'s meaning, `returns` and `$` aren't decided yet.)

**Weighed.**
- `()` everywhere, `proc.main():` and `proc roll_dice() returns Whole:`, as in Python. `proc.main()` reads like
  calling a method `main` on something called `proc`, as `$list.add()` would.
- `proc.main:` alone without brackets, and `()` on every other definition. It singles out `main` for no reason a
  learner could see.
- An empty pair in definitions, as Tankun first wrote `proc.main[]`. An empty bracket looks unfinished.

**Why this won.** In a definition, brackets hold parameters, so with none there's nothing to hold (multifinity's
rule). In a call, `( )` means "run it", so it's always there, which is what Python readers expect. Both readings
avoid the method-call look of `proc.main()`.

**Tankun's words.** Asked "No brackets on a definition that takes nothing, `()` on every call?", the owner chose:
"yea, what about calls inside slides?"

## D5. Slides: `|>` passes a value along as each step's first argument

2026-11-20.

**Chosen.** v2 has slides (a pipe): `|>` passes the value on its left into the next step as its first argument, so a
chain reads left to right in the order it runs. A step with nothing else to pass is written bare. A step with more
arguments takes `( )` for those only, and the slid value goes in first.

```
$points |> be.Text |> show_msg;
$score |> clamp(min: 0, max: 100) |> be.Text |> show_msg;
```

A step isn't a whole call, so D4's "every call writes `()`" doesn't apply to it: the step names the procedure, and the
slide does the calling. `|> show_msg()` would look like calling `show_msg` with nothing, when it actually gets the
slid value.

(`be.Text`, `clamp` and its labels, and where `|>` sits among other operators aren't decided yet.)

**Weighed.**
- `()` on every step, as D4 asks of calls. Empty `()` on each step of a long chain read "as a stroke" (multifinity's
  chat), and it hides that a value goes in.
- No slides, nesting calls inside out instead: `show_msg(be.Text(clamp($score, min: 0, max: 100)))`.

**Why this won.** A chain reads in the order it runs, where nesting reads from the inside out. Writing only what's
added in a step's `( )` matches Elixir and Plainly v1's pipe (its D29, D30), and keeps D4's rule true where it means
something: a full call writes all its arguments in `( )`.

**Tankun's words.** Asked "Slides like that?", the owner chose: "yea".

## D6. `show_msg` ends the line, and `show_msg.same_line` doesn't

2026-11-20.

**Chosen.** `show_msg` shows text and ends the line, like Python's `print`. `show_msg.same_line` shows text and leaves
the line open, for prompts and drawing.

```
show_msg("Hello, World!");
show_msg.same_line("Your guess: ");
```

The docs teach "`msg` is short for message" (D2).

**Weighed.**
- `append_msg` for the second one (multifinity's chat: "You're appending the message to output"). "Append" is
  programmer jargon for "add to the end", and a beginner may read it as adding to a message rather than to the
  screen; it also gives two unrelated-looking names to nearly one job.
- `show_msg.no_new_line`. It names what's missing rather than what you get.
- No newline by itself, writing it out every time. Most prints want one, and a beginner who forgets it gets
  everything on one line.

**Why this won.** Showing a message is one thing, so it's one procedure, and the narrowing names its one difference,
the way `.main` narrows `proc`. `.same_line` says exactly what appears on screen. This is Plainly v1's D12 with v2's
name.

**Tankun's words.** Asked "`show_msg` ends the line, and `show_msg.same_line` doesn't?", the owner chose: "yea".

## D7. `;` ends every statement

2026-11-20.

**Chosen.** Every statement ends with `;`, slides included, so a statement can run over several lines and ends only
at its `;`:

```
$score
|> clamp(min: 0, max: 100)
|> be.Text
|> show_msg;
```

A missing `;` is refused, with the place it goes as the fix.

(How a block's opening line and closer are written isn't decided yet.)

**Weighed.**
- Ending a statement at the end of its line, as Python does. One less symbol per line, and a forgotten `;` is the
  most common beginner mistake in C-like languages. But a statement running over several lines then needs a rule:
  Python's brackets or `\`, or JavaScript's guessing where statements end, which goes wrong in well-known ways.

**Why this won.** Every statement visibly stops, which was Plainly v1's reason (its D31), and long slides need no
special rule. With the exact fix in every message, a missing `;` costs a keystroke, not confusion.

**Tankun's words.** Asked "`;` ends every statement?", the owner chose: "yea".

## D8. A block opens with `:` and closes with `end` and what it closes

2026-11-20.

**Chosen.** A block's opening line ends with `:`. Its closer is `end` followed by what it closes, standing alone with
no `;`: `end proc add`, `end proc.main`, `end if`.

```
proc larger($a: Whole, $b: Whole) returns Whole:
    if $a > $b:
        return $a;
    else:
        return $b;
    end if
end proc larger
```

(Indentation, `if`, `return` and comparisons aren't decided yet. This example first had `return $b;` after `end if`,
the shape Plainly v1's D50 and Cyborg's D430 refuse, and called itself `add` though it doesn't add.)

**Weighed.**
- Python's way: `:` to open, and the block ends where the indentation does, with no closing line. Shorter, and
  closest to "static Python", but a wrongly indented line silently moves outside its block, pasted code can change
  meaning, and deep in nested code nothing says which block just ended.

**Why this won.** "Built for reading": `end if` and `end proc add` say which block ends, so a reader never counts
indentation. A missing closer gets a clear refusal instead of a silent change of meaning. Indentation can still be
required for looks, as a decision of its own. This is Plainly v1's shape (its D31, D35).

**Tankun's words.** Asked "Blocks open with `:` and close with `end` plus what they close?", the owner chose: "yea".

## D9. After an `if` where any way leaves, nothing follows `end if`

2026-11-20.

**Chosen.** When any way through an `if` leaves, by `return`, `break` or a failure, nothing may follow its `end if`
in the same block. Code that should run afterwards goes in the ways that don't leave. That covers:
- every way leaving with no `else`, the classic guard clause;
- only some ways leaving, where the code after `end if` would run only after the others;
- every way leaving, `else` included, where the code after `end if` could never run.

Each is refused while building.

```
proc greet($name: Text):
    if $name == "":
        return;
    else:
        show_msg("Hi, " & $name & "!");
    end if
end proc greet
```

(`==` on text, a bare `return;`, `&`, and the messages' wording aren't decided yet.)

**Weighed.**
- Guard clauses, as in Python and most languages: `if $name == "": return; end if` with the rest after. The habit
  most learners meet elsewhere.
- Plainly v1's D50 alone, refusing only the first case and leaving the other two open, as v1 did.

**Why this won.** A guard clause looks like a procedure doing two things in a row, when it does one or the other;
with `else`, the paths read as alternatives. Both shapes compile to the same test and jump, so it costs nothing at run
time. Covering all three cases at once leaves no shape where code after an `if` runs only sometimes, or never. This is
Plainly v1's D50 and Cyborg's D430, completed. Long guard chains become flat `else if` chains.

**Tankun's words.** Tankun spotted D8's first example breaking this: has code. Asked "After an `if` where any way leaves, nothing follows `end if`?",
the owner chose: "yea".

## D10. Bodies are indented 4 spaces per level, and anything else is refused

2026-11-20.

**Chosen.** A block's body is indented one level inside its opening line and its closer, and a block inside a body one
level further. One level is 4 spaces. Any other indentation, a tab included, is refused while building, with the whole
corrected line as the fix.

```
proc.main:
    show_msg("Hello, World!");
end proc.main
```

**Weighed.**
- A formatter, `plainly format`, rewriting files as Go's `gofmt` does, with the checker not caring. A beginner has
  to know to run it.
- Free indentation. A body's lines drift, so code can read wrong while running right.
- Tabs, or a width of the writer's choosing. Programs would look different from one another.

**Why this won.** Since D8 ends every block with `end`, indentation never decides where a block ends, so it's only
about looks, and "built for reading" wants every program to look the same. Python gets that by making indentation
meaningful; Plainly gets it by checking it. A refusal with the corrected line teaches it the first time. This is
Plainly v1's D70 and D74.

**Tankun's words.** Asked "4 spaces per level, required, tabs refused?", the owner chose: "yea".

## D11. Variable names start with `$`

2026-11-20.

**Chosen.** A variable's name starts with `$`: `$name`, `$secret`, `$roll`.

**Weighed.**
- `#`, as in Plainly v1. In Python and most other languages `#` starts a comment, so `#score` looks like one to
  anyone coming from them.
- No mark, as in Python. Variables could then clash with keywords and procedure names, and a new keyword could break
  existing programs.

**Why this won.** A mark means a variable is never mistaken for a keyword or a call (`$roll` is a value, `roll()` a
call), keywords can be added later without breaking anyone's names, and variables stand out when filled into text.
`$` already means "a variable" in shells and PHP. The cost is one character per name, and looking less like Python.

**Tankun's words.** Asked "Variable names start with `$`?", the owner chose: "yea. Compiler before oracle, right? But
with tests. And also a standard before compiler"

## D12. The order of work: standard, tests, compiler, then the oracle

2026-11-20.

**Chosen.**
1. **A standard grows with the decisions,** in `standard/`. Each decision that changes a rule updates the standard in
   the same commit, so the standard says what Plainly is now and DECISIONS.md how it got there.
   - It starts with a **Terms** chapter defining every word it relies on (statement, block, procedure, refused, and so
     on), each defined before any chapter uses it.
   - **Every general form says what its placeholders stand for,** right before or after it, as in: `proc name(parameters)
     returns Type:`, where `name` is the procedure's name, `parameters` its parameters separated by commas, and `Type`
     the type it gives back. A real example with no placeholders needs no such line.
2. **Tests are written by hand from the standard,** each a small program with its exact expected output or refusal,
   naming the decision and section it comes from. Where an expected value is computed (arithmetic, rounding, overflow,
   text, a whole program's output), it's cross-checked by the same calculation in Python, or in Rust where fixed-size
   numbers matter, with the check kept beside the test.
3. **The compiler is built to pass those tests.** Its side never edits an expected output: a test that looks wrong
   goes back to the design session, which checks it against the standard, and asks Tankun where the standard is
   unclear.
4. **The oracle comes later,** as a second implementation written from the standard, running the same tests, and as
   the web playground. Where the compiler, the oracle and an expected output disagree, the standard decides.

Nothing gets built until Tankun says so (CLAUDE.md); this sets the order.

**Weighed.**
- The oracle first, as in Plainly v1, where building it was what found the gaps in the design. Compiler first gets a
  usable language sooner; until the oracle exists, only the tests' named rules and their cross-checks stand against a
  wrong expected output, so the tests have to cover the awkward cases, not just programs that work.
- C as a cross-check: its signed overflow is undefined behaviour, so it's least trustworthy on exactly the cases that
  matter.
- SPARK at Platinum level, Ada with mathematical proof of correctness: the strongest opinion, but proving even a short
  function can take days, and the tests need an independent answer, not a proof. Lower SPARK levels prove only that
  code doesn't crash, not that its answer is right.

**Why this won.** A standard written alongside the decisions is always the current truth, and its Terms chapter and
placeholder lines make it readable by someone who isn't a programmer. Tests written from it are the compiler's
contract, and a compiler that may never edit them can't lock a wrong expectation in. An independent calculation in
another language catches a wrong expected value before any compiler exists, and the oracle later checks everything
twice.

**Tankun's words.** Tankun set the order in their answer to D11, quoted whole there, then added "Before that, I think there should be a definition inside the standard about
the terms it uses, and also if you write `let $placeholder: Placeholder = placeholder;`, you should say before or after
what `placeholder` in that context means", asked has code, suggested "What about a separate language's opinion on
result too? When possible, maybe C, Python, Rust, or SPARK Platinum", then "Holy shit, bro. Do you know how diabolical
Plat is?". Asked "Go with that?", the owner chose: "yea".

## D13. `let` makes a variable that can't change, `var` one that can, and the type is always written

2026-11-20.

**Chosen.** A new variable is declared with `let` or `var`, its name, its type and its first value:

```
let $secret: Whole = 42;
var $tries: Whole = 0;
```

- `let`: its value never changes after this line.
- `var`: its value can change. The docs teach "`var` is short for variable", a value that can vary (D2).
- The type is always written, never worked out from the value.

(`Whole`, numbers, and how a variable is changed aren't decided yet.)

**Weighed.**
- No word for a declaration, `$secret: Whole = 42;`, with a word only for one that can change, as in Plainly v1
  (`mutable`). Creating and changing then look more alike.
- `const` as well, for a value known while building that can sit outside any procedure, as in multifinity's notes
  (`const $MAXTRIES: Whole = 5;`). It means nearly what `let` does, and only matters once anything outside a procedure
  is decided, so it waits.
- Working the type out from the value, as Python and most modern languages do. multifinity's reason against it: if
  `let $name = "Tankun"` looked dynamic, a parameter refusing the wrong type later would surprise beginners.

**Why this won.** A word in front says "a new variable starts here", so a declaration is never mistaken for a change.
`let` and `var` are Nim's and Swift's words and close to JavaScript's, and both teach in one sentence. A written type
shows from the first line that Plainly is static, and says which kind of number a variable holds.

**Tankun's words.** Asked "`let` and `var`, with the type always written?", the owner chose: "yea".

## D14. A bare `=` changes a `var`, with no `change` in front

2026-11-20.

**Chosen.** A `var` gets a new value with its name, `=` and the value, as a statement of its own:

```
var $tries: Whole = 0;
$tries = $tries + 1;
```

- `=` on a `let` is refused, since a `let` never changes (D13).
- `=` only ever stands alone as a statement. It never appears inside a condition, so `if $tries = 3:` is refused, the
  classic typo for comparing. The fix offered was `==`, but how comparison is written isn't decided yet.

(`+`, `if` and shorthand like `+=` aren't decided yet.)

**Weighed.**
- `change` in front, `change $tries = $tries + 1;`, as in Plainly v1 (its D27), so a change can never be mistaken for
  a creation. In v2, `let` and `var` already mark every creation (D13), so the word adds nothing.

**Why this won.** A line starting with `let` or `var` makes a variable, and one starting with `$tries =` changes it,
so the two can't be confused. It's how Python and most languages write a change, and searching for `$tries =` still
finds every one. Keeping `=` out of conditions catches the `=`-for-`==` typo.

**Tankun's words.** Asked "A bare `=` changes a `var`, and `change` goes?", the owner chose: "yea".

## D15. `Whole` is a 64-bit signed whole number, with no other name

2026-11-20.

**Chosen.** `Whole` holds a whole number from -9223372036854775808 to 9223372036854775807, negatives included: a
64-bit signed integer, Rust's `i64`. It has no second name: there's no `i64` meaning the same thing.

```
let $secret: Whole = 42;
```

Other sizes, if they come, are different types with names of their own, not second names for `Whole`.

(Going past the range, and how whole numbers are written, aren't decided yet.)

**Weighed.**
- `Whole` and `i64` both, as in Plainly v1 (its D65). Two spellings for one type, against D2.
- 32 bits, as C's and Java's `int`. It runs out at about 2.1 billion, which a score or a timer in milliseconds can
  reach.
- Unlimited size, as Python's `int`. Every calculation gets slower and needs memory, for a range a game never uses.

**Why this won.** "Whole number" is the school-maths term, so the name teaches itself. 64 bits is far more than a game
needs, and matches Rust, so anything crossing over to Rust keeps its value. One name per type follows D2.

**Tankun's words.** Asked "`Whole`, a 64-bit signed whole number, with no other name?", the owner chose: "yea".

## D16. `if`, `else if`, `else` and `end if`

2026-11-20.

**Chosen.**

```
if $guess < $secret:
    show_msg("Too low");
else if $guess > $secret:
    show_msg("Too high");
else:
    show_msg("Correct!");
end if
```

- `if`, its condition and `:`, with no brackets and no `then`.
- Any number of `else if`, then at most one `else:`, each with its own body.
- One `end if` closes the whole chain (D8).

(Comparisons like `<`, and what a condition may be, aren't decided yet.)

**Weighed.**
- `elif`, as in Python. It's a made-up contraction, not short for any one word, so it breaks D2's "learn the concept,
  then type it short".
- Brackets around the condition, as in C, or `then`, as in Lua. Both are noise once `:` ends the line.

**Why this won.** It's Python's shape with plain words, ending each opening line with `:` like every block (D8). This
is Plainly v1's D35.

**Tankun's words.** Asked "`if`, `else if`, `else` and `end if` like that?", the owner chose: "yea".

## D17. Comparisons: `==`, `!=`, `<`, `>`, `at_least` and `at_most`

2026-11-20.

**Chosen.**

```
$lives == 0
$guess != $secret
$guess < $secret
$score > $best
$hp at_least 1
$hp at_most 100
```

- `==` and `!=` for equal and not equal; `<` and `>` for less and greater.
- `at_least` and `at_most` instead of `>=` and `<=`.
- `<` and `>` have a space on each side, since `< >` also fills in a type with no spaces, as in `List<Whole>` (D3).

(Comparing two pieces of text, and combining conditions, aren't decided yet.)

**Weighed.**
- `>=` and `<=`, as in nearly every language. They're where beginners trip: which way round, whether the number
  itself counts, `<=` or `=<`.
- `=` for equality, as in maths. `=` already gives a value (D13, D14), and `=` inside a condition is refused (D14).

**Why this won.** Symbols where they're read at a glance, words where they aren't: `==`, `!=`, `<` and `>` are
understood without reading them in your head, and `at_least` and `at_most` say exactly whether the number counts. The
spacing rule keeps comparisons and types apart. This is Plainly v1's D33 and D34.

**Tankun's words.** Asked "Those six comparisons?", the owner chose: "yea, `proc . main` also shouldn't be legal"
(the second half is D18).

## D18. A narrowing's `.` has no spaces around it

2026-11-20.

**Chosen.** The `.` of a narrowing touches the words on both sides: `proc.main`, `show_msg.same_line`. Writing
`proc . main`, or a space on either side, is refused, with the joined form as the fix.

**Weighed.**
- Allowing spaces, as many languages do around `.`. The same thing could then be written several ways, against one
  spelling per thing (D2), and `proc . main` reads like three separate words.

**Why this won.** A narrowing and its word are one name, `proc.main`, and should look like one, every time. Tankun
raised it.

**Tankun's words.** In answer to D17's question: "yea, `proc . main` also shouldn't be legal". Taken as the rule for
every narrowing, not `proc.main` alone.

## D19. `while` repeats while its condition holds

2026-11-20.

**Chosen.** `while`, its condition and `:` open a loop that runs its body again and again as long as the condition
holds, checked before each pass. `end while` closes it (D8).

```
var $tries: Whole = 0;
while $tries < 5:
    show_msg("Guess!");
    $tries = $tries + 1;
end while
```

(`+`, leaving a loop early, and other kinds of loop aren't decided yet.)

**Weighed.**
- `repeat while`, or `loop while`. Longer, with nothing gained over the word every language uses.

**Why this won.** It's Python's word, and the shape of every other block (D8). Plainly v1 had the same (its D53).

**Tankun's words.** Asked "`while` like that?", the owner chose: "yea".

## D20. `ask` shows a question and gives back what the player types

2026-11-20.

**Chosen.** `ask(question)` shows the question, leaves the cursor on its line, waits until the player presses Enter,
and gives back what they typed, without the Enter:

```
let $typed: Text = ask("Your guess: ");
```

What's typed always comes back as text, even digits: typing `42` gives the text `"42"`, since a keyboard only sends
characters. Making it a number is a separate step, which can fail.

(`Text`, turning text into a number, asking with no question, running out of input, and reading single key presses
aren't decided yet.)

**Weighed.**
- `read_msg()`, pairing with `show_msg`. What a player types isn't a message to anyone.
- `read_input()`. "Input" is vague: key presses, the mouse and files are input too.
- `read_line()`, with the question shown first by `show_msg.same_line` (D6). It names what the computer does rather
  than what the program means, and takes two lines for the common case.
- A `msg` family, `msg.show(...)` and `msg.ask(...)` (Tankun's idea). A narrowing makes a word more specific (D18's
  `proc.main`), and `msg.show` isn't a kind of message but an action, so the `.` would change meaning; it also reads
  noun-first and looks like calling a method on an object named `msg`, the look D4 removed. Grouping by topic belongs
  to packages, not narrowings.

**Why this won.** From the player's side the game is asking: it shows a question and waits for the answer, one act,
so one procedure. It reads like English, `let $typed: Text = ask("Your guess: ");`, and the common case needs no
`.same_line`. Tankun found the word.

**Tankun's words.** "What if they input a number? What about something like `read_input`? Idk. What does this do",
"Read? It seems more like `ask`?", "What if we made `msg` a family? `msg.ask`, `msg.show`? Lol", then, asked
"`ask(question)`?", the owner chose: "yea".

## D21. `Text` is the type for text, written in double quotes

2026-11-20.

**Chosen.** Text has the type `Text`, and a piece of text is written in double quotes:

```
let $name: Text = "Mia";
show_msg("Don't give up!");
```

Single quotes aren't used for text.

(A quote inside text, other escapes, empty text, joining and comparing text aren't decided yet.)

**Weighed.**
- `String` or `str`, as most languages say. Jargon that needs explaining ("a string of characters").
- Single and double quotes both, as in Python. Two ways to write the same text, against D2's one spelling per thing.

**Why this won.** `Text` is the plain word and needs no explaining. With only double quotes there's one way to write
text, and an apostrophe inside, as in `"Don't"`, needs nothing special. Plainly v1 and multifinity's notes both use
`Text`.

**Tankun's words.** Asked "`Text`, written in double quotes?", the owner chose: "yea".
