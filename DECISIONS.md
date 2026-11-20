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

## D22. `be.Type(value)` converts a value to a type

2026-11-20.

**Chosen.** Converting is `be` narrowed by the type the value becomes:

```
let $typed: Text = ask("Your guess: ");
let $guess: Whole = be.Whole($typed);
show_msg(be.Text($guess));
```

Converting a value to the type it already is, like `be.Text` of a `Text`, is refused, since it does nothing.

`be.Whole("abc")` can't give a number, so converting can fail. (How a failure is handled, and what may be converted
to what, aren't decided yet.)

**Weighed.**
- The type called like a procedure, as Python's `int("42")`. It hides that converting is an action that can fail, and
  makes a type look like a procedure.
- A procedure per conversion, like `text_to_whole`. A new name for every pair of types.

**Why this won.** `be` ("become") narrowed by the type's own name is a real narrowing (D18), so nothing new is learnt
for each type. Refusing a conversion that does nothing catches a misunderstanding instead of hiding it. This is
Plainly v1's D6 and D90.

**Tankun's words.** Asked "`be.Type(value)` for converting?", the owner chose: "yea".

## D23. Failures must be handled: `?`, `??` and `otherwise:`

2026-11-20.

**Chosen.** A procedure that can fail says so, and every call to it must deal with the failure, or the program is
refused while building. There are three ways, taken from Cyborg:

```
let $level: Text = load_text("level1.txt")?;
let $guess: Whole = be.Whole($typed) ?? 0;
save_text($line, "save.txt") ?? ignore;
load_level($name) ?? crash;
```

- **`?` passes the failure up** to whoever called this procedure, which must list that failure itself.
- **`??` handles it on one line:** `?? value` carries on with that value, `?? ignore` ignores the failure on purpose,
  and `?? crash` stops the program on purpose.
- **`otherwise:` handles each failure in its own `case`,** each ending in a fallback value, `return`, `break`, or
  failing on purpose.

(How a procedure lists its failures, failures' names, the exact form of the `otherwise:` block and its cases, and how
a procedure fails on purpose aren't decided yet.)

**Weighed.**
- No checking, as with Python's exceptions, where any call can fail unnoticed until it happens; or special values like
  `-1`, which get ignored.
- `otherwise use 0`, `otherwise ignore` and `otherwise crash` in place of `??`, as in Plainly v1 (its D56), arguing
  that `??` and `otherwise` would be two spellings of "if this fails". Tankun: "That's longer for a common one, though":
  15 characters against 4 for a fallback value, which games need often. And since there's no `otherwise use`, `??` is
  the only one-line form and `otherwise:` the only block form, two shapes for two sizes of job, not two spellings.
- `?? void`, Cyborg's word for ignoring. "Ignore" says what's happening.

**Why this won.** An unhandled failure is a crash waiting for the one input nobody tested, like a player typing `abc`;
refusing it while building means it's dealt with before anyone plays, and a library procedure gaining a failure gives
its callers a build error, not a surprise. `?` and `??` read as a family, "on failure, pass it up" and "on failure, do
this instead", short where they're common, as D2's "learn the concept, then type it short" asks. Searching `?? ignore`
or `?? crash` finds every deliberate one. This is Cyborg's design, and Plainly v1's D42, D43 and D55.

**Tankun's words.** "Use Cyborg's? With `?`, `??`, and `otherwise:`?", "That's longer for a common one, though", then,
asked "Handling required, with `?`, `??` (a value, `ignore` or `crash`) and `otherwise:`?", the owner chose: "yea".

## D24. Labels: names for arguments, chosen by the procedure and written at every call

2026-11-20.

**Chosen.** A procedure can give its arguments labels, and every call writes them:

```
let $secret: Whole = random(from: 1, to: 100);
```

- The procedure's writer chooses each label. A call can't leave one off, or write one the procedure doesn't have.
- By convention, the first argument, the thing being worked on, has no label, as in `show_msg("Hi")` or a slide's
  value (D5), so simple calls stay simple.

(How a procedure declares its labels, defaults, label order, and `random` itself aren't decided yet. The first sketch
of a declaration, labels inside the parameter list, was the form Plainly v1's D9 turned down, and was dropped.)

**Weighed.**
- Arguments by position only, `random(1, 100)`. The reader has to remember which is which, and two can be swapped by
  mistake.
- Optional names, as Python's keyword arguments. Most code then skips them, so the reader still guesses.

**Why this won.** A call says what each value means, so it reads without opening the procedure, and arguments can't be
swapped by mistake. This is Cyborg's labels, and Plainly v1's D8.

**Tankun's words.** "What the fuck is that label decl syntax", "What does `random` do?", then, asked "Labels at the
call?", the owner chose: "yea".

## D25. Labels are declared on a `labels` line at the top of the body

2026-11-20.

**Chosen.** A procedure declares its labels on a `labels` line, the first line of its body, pairing each labelled
parameter with its label, with commas between and `;` at the end, and no brackets:

```
proc random($low: Whole, $high: Whole) returns Whole:
    labels $low as from, $high as to;
    ...
end proc random

let $secret: Whole = random(from: 1, to: 100);
```

where `...` stands for the rest of the body. Inside, the procedure uses `$low` and `$high`; callers write `from:` and
`to:` (D24).

(Defaults, label order, and `random` itself aren't decided yet.)

**Weighed.**
- The label as the parameter's own name, with a mark: `proc random(labeled $from: Whole, labeled $to: Whole)`. Each
  parameter packs a mark, a name and a type into one item, the crowding Plainly v1's D9 turned down ("That's bad"),
  and renaming a parameter breaks every call.
- `labels [$low as from, $high as to];`, as in Plainly v1. Since D3, `[ ]` means a list, and these pairings aren't a
  list the program works with. `( )` would look like a call.
- Labels inside the parameter list, `proc random(from $low: Whole, to $high: Whole)`, the first sketch for D24, which
  Tankun refused ("What the fuck is that label decl syntax").

**Why this won.** The header keeps one name and one type per parameter, and the inside name stays apart from the
outside label, so a procedure can rename its insides without breaking a call. The line runs nothing and gives back
nothing, so it's neither a call nor a list, and takes no brackets. `range(from: 1, to: 5)`, by contrast, gives back a
value, so it's a call (D3, D4).

**Tankun's words.** "Why no `[]`?", "Wait, then `range()` is legal or not", then, asked "A `labels` line, with no
brackets?", the owner chose: "yea".

## D26. `random(from: low, to: high)` gives a whole number with both ends included

2026-11-20.

**Chosen.** `random(from: low, to: high)` gives a whole number from `low` to `high`, both included, each equally
likely:

```
let $secret: Whole = random(from: 1, to: 100);
```

It's game randomness: fast, and fine for dice, loot and secret numbers in a game, not for anything that must be
unguessable.

(Seeding, a separate kind of randomness for real secrets, `from:` above `to:`, and random numbers of other types
aren't decided yet.)

**Weighed.**
- Leaving out the top, as Python's `randrange(1, 100)`: a classic off-by-one, where 100 never comes up.
- `randomize`, as in multifinity's first sketch. In Nim it means seeding, not picking a number.

**Why this won.** `from:` and `to:` read exactly as the range you get, both ends included, as people mean when they
say "a number from 1 to 100". Keeping the plain name for game randomness leaves room for a clearly different name for
secrets later, the split Plainly v1's D25 made.

**Tankun's words.** Asked "`random(from:, to:)`, both ends included?", the owner chose: "yea".

## D27. `+` adds and `-` subtracts, and a `-` with no space after it negates

2026-11-20.

**Chosen.**

```
$tries = $tries + 1;
let $left: Whole = 5 - $tries;
let $back: Whole = -$speed;
```

- `+` adds and `-` subtracts numbers, with a space on each side.
- A `-` right before a number or a name, with no space after it, makes it negative: `-7`, `-$speed`.
- `+` is only for numbers; text is joined some other way.

(Going past `Whole`'s range, multiplying, dividing, maths order, and joining text aren't decided yet.)

**Weighed.**
- `+` for joining text as well, as in Python. `"5" + 3` could then be misread as joining or as adding.

**Why this won.** Spacing tells subtracting (`$a - $b`) from negating (`-$b`), as D17's spacing tells comparisons from
types. Keeping `+` for numbers means a `+` always adds. This is Plainly v1's D1, D84 and D86.

**Tankun's words.** Asked "`+` and `-` like that?", the owner chose: "yea".

## D28. `&` joins text, and doesn't convert numbers by itself

2026-11-20.

**Chosen.** `&` joins two pieces of text. A number joined with `&` is refused, with `be.Text(...)` as the fix (D22),
so it's always visible where a number becomes text:

```
show_msg("Hello, " & $name & "!");
show_msg("You got it in " & be.Text($tries) & " tries.");
```

(Filling values into text, and `&` next to `+` or `-` without brackets, aren't decided yet.)

**Weighed.**
- `+`, as in Python. D27 keeps `+` for numbers, so `"5" + 3` can't be misread.
- Converting numbers by itself, as JavaScript does. `"5" & 3` would quietly mean something, and where a number becomes
  text would be hidden.

**Why this won.** `&` reads as "and", is what Ada, Visual Basic and Excel use, and has no other job in Plainly. This
is Plainly v1's D1 and D6, and multifinity's notes.

**Tankun's words.** Asked "`&` joins text, with no automatic converting?", the owner chose: "yea".

## D29. `.fmt` fills values into text, with braces holding only names

2026-11-20.

**Chosen.** `.fmt` fills in what's in braces. It narrows `show_msg` to show the result, and `be.Text` to give it back
as `Text`:

```
show_msg.fmt("You got it in {$tries} tries.");
let $line: Text = be.Text.fmt("{$name}: {$score}");
```

- Braces hold only a variable's name. `{$tries + 1}` and `{roll()}` are refused, so text never hides a calculation or a
  call that could fail.
- A number in braces becomes text by itself.
- The text is checked while building: a name in braces that isn't a variable is refused before the program runs.
- Without `.fmt`, text is plain: `show_msg("{$tries}")` shows exactly `{$tries}`.
- The docs teach "`fmt` is short for format" (D2).

(A literal `{` inside `.fmt` text, settings in braces like `places: 2`, and the order of `.fmt` and `.same_line`
together aren't decided yet.)

**Weighed.**
- `.format`, as in Plainly v1. D2's short form is `fmt`, known from Go and Rust.
- Python's f-strings, `f"You got it in {tries} tries."`. Shorter, but one letter before the quote changes what the
  text means, easy to miss when reading.
- Cyborg's `@fmt("...")`, a compiler builtin giving a "format value" that becomes text wherever it's used. The `@`
  marks Cyborg's builtins, and the format value exists because Cyborg makes every memory allocation visible; Cyborg's
  design claude advised that `.fmt` returning text is simpler for a language without that. Taken from it: names-only
  braces and checking while building.

**Why this won.** One narrowing covers both showing and building text, with no new keyword, and a fill-in can only
read a value, never do work. It replaces long `&` chains with `be.Text` in each (D28).

**Tankun's words.** "What about Cyborg's `@fmt`?", "How about `.fmt`?", then, asked "`.fmt` on `show_msg` and
`be.Text`, with braces holding only names, checked while building?", the owner chose: "yea".

## D30. `\` makes the next character literal, with no C-style codes

2026-11-20.

**Chosen.** Inside text, `\` makes the next character literal, and does nothing else:

```
show_msg.fmt("Type \{help} to see the commands, {$name}.");
show_msg("She said \"hi\".");
```

- `\{` is a brace in `.fmt` text, `\"` a quote, and `\\` a backslash.
- Only `{` needs escaping in `.fmt` text: a `}` with no open `{` is just a character.
- `\` doesn't start C-style codes: `\n` isn't a new line and `\t` isn't a tab.

(How a new line or a tab is written inside text, and whether `\` before a letter is refused, aren't decided yet.)

**Weighed.**
- C's codes, `\n`, `\t` and the rest, as Python has. Codes to memorise, which a plain word can replace.
- Python's doubled braces, `{{` and `}}`. Two rules where one does.

**Why this won.** One rule covers every case: the character after `\` is taken as it is. This is Plainly v1's D3 and
D5 ("`\` escapes, but it doesn't format like C", Tankun in v1).

**Tankun's words.** Asked "`\` makes the next character literal, with no C codes?", the owner chose: "yea".

## D31. Narrowings go strongest first, and any other order is refused

2026-11-20.

**Chosen.** When a word has two or more narrowings, the one that changes the action most comes first, and any other
order is refused, with the right spelling as the fix. `.fmt` changes what's shown, and `.same_line` only where it ends,
so:

```
show_msg.fmt.same_line("Lives: {$lives}, your move: ");
```

and `show_msg.same_line.fmt(...)` is refused, with `fix: show_msg.fmt.same_line`.

(A table of narrowing kinds, to place each new narrowing without judging it afresh, isn't decided yet.)

**Weighed.**
- Any order. Two spellings for one thing, against D2, and a reader may wonder whether the orders differ.

**Why this won.** One spelling means a search finds every use, and the reader learns the most important fact first.
This is Plainly v1's D14 and Cyborg's rule.

**Tankun's words.** Asked "Narrowings go strongest first, with any other order refused?", the owner chose: "yea".

## D32. `break` leaves the innermost loop, and there's no `continue`

2026-11-20.

**Chosen.** `break;` leaves the innermost loop it's in. An `if` that breaks has the rest of the pass in its other
ways (D9). There's no `continue`: ending one pass early is an `if` around the rest.

```
while $tries < 7:
    let $guess: Whole = be.Whole(ask("Your guess: ")) ?? 0;
    $tries = $tries + 1;
    if $guess < $secret:
        show_msg("Too low");
    else if $guess > $secret:
        show_msg("Too high");
    else:
        show_msg.fmt("Correct! You got it in {$tries} tries.");
        break;
    end if
end while
```

(Leaving a loop further out, by naming it, isn't decided yet.)

**Weighed.**
- `continue`, as in Python and C. In Radium's cold tests every word for it was misread: `continue` as "carry on",
  `skip` as skipping the whole loop, and `next` and others too (Radium's D77, Plainly v1's D53).

**Why this won.** `break` is the word every language uses, and with D9 the way out of a loop always sits in its own
branch. Without `continue`, the shape of the code shows which lines a pass runs.

**Tankun's words.** Asked "`break` leaves the innermost loop, and no `continue`?", the owner chose: "yea".

## D33. `Bool` is the type for true or false, with the values `true` and `false`

2026-11-20.

**Chosen.** A value that's either true or false has the type `Bool`, and its two values are written `true` and
`false`. A comparison gives a `Bool`, and that's what `if` and `while` check.

```
var $won: Bool = false;
$won = $guess == $secret;
```

The docs teach "`Bool` is short for Boolean, a value that's either true or false, named after George Boole" (D2).

(Combining conditions with words like `and`, `or` and `not`, and whether a condition must be a `Bool`, aren't decided
yet.)

**Weighed.**
- `Truth`. Reads oddly: `var $won: Truth`.
- `YesNo`. Made up, and found in no other language.
- `Flag`. Jargon of its own.
- `True` and `False`, capitalised as in Python. Plainly's other keyword values are lowercase.

**Why this won.** `Bool` follows D2: a short form taught once, and the word every language and tutorial uses, so a
learner keeps meeting it.

**Tankun's words.** Asked "`Bool`, with `true` and `false`?", the owner chose: "yea".

## D34. `and`, `or` and `not` combine conditions, in that order, and stop early

2026-11-20.

**Chosen.**

```
if not $won:
    show_msg.fmt("Out of tries! It was {$secret}.");
end if

if $hp at_least 1 and $lives > 0 or $cheat == true:
```

- `and`, `or` and `not` are words.
- `not` is worked out first, then `and`, then `or`, so the second example means
  `($hp at_least 1 and $lives > 0) or $cheat == true`, with no brackets needed. Comparisons come before all three.
- `and` and `or` stop early: the right side runs only if it can still change the answer, so a guard like
  `$i at_most $count and $items[pos: $i] == 0` never reaches past the end of a list.

(A form that always works out both sides, and lists and positions, aren't decided yet.)

**Weighed.**
- C's `&&`, `||` and `!`. Symbols to memorise where Python and plain English use words.
- Requiring brackets wherever `and` meets `or`. More to write, for an order nearly every language shares.
- Always working out both sides, as Ada's plain `and` does. Its users end up writing `and then` everywhere to stay
  safe.

**Why this won.** Plain words, Python's own, in the order most languages use, with the safe behaviour as the default.
This is Plainly v1's D36, D37 and D92.

**Tankun's words.** Asked "`and`, `or` and `not` like that?", the owner chose: "yea".

## D35. Plainly files end in `.plainly`

2026-11-20.

**Chosen.** A Plainly file's name ends in `.plainly`, as in `game.plainly`.

**Weighed.**
- A short extension like `.pl`, taken by Perl, or `.pn`, which says nothing.

**Why this won.** It names the language, so people and editors know what a file is; nothing else uses it, so GitHub
and editors can map it to Plainly without a clash; and it's easy to search for. It's long, but typed rarely. This is
Plainly v1's D78.

**Tankun's words.** Asked "`game.plainly`?", the owner chose: "yea. Where's families? And making a extern proc"

## D36. `extern.rust proc` declares a procedure whose body is written in Rust

2026-11-20.

**Chosen.** `extern.rust` in front of `proc` says the procedure's body is written in Rust, outside the Plainly file.
Its body is empty, or holds only a `labels` line (D25), never code, and its closer doesn't repeat `extern.rust`:

```
extern.rust proc beep returns Bool:
end proc beep
```

- `extern` is short for external (D2); `.rust` narrows it to where the body is, leaving room for `extern.c` later.
- Callers can't tell it apart from any other procedure.

(How Plainly's values cross into Rust, and what an extern procedure does when it fails, are left for when building
starts.)

**Weighed.**
- `proc.rust`, narrowing `proc` itself. `extern` is a modifier in front, like `let` against `var`, and says the body
  is elsewhere before saying where.

**Why this won.** A game's library needs procedures that talk to the terminal and the operating system, which are
written in Rust, and this lets them look like any other procedure to the people calling them. This is Plainly v1's
D40, with v2's short forms.

**Tankun's words.** Asked "`extern.rust proc` like that?", the owner chose: "yea. `returns` locked" (the second half
is D37).

## D37. A procedure's header says what it gives back with `returns`

2026-11-20.

**Chosen.** A procedure that gives back a value says so at the end of its header, with `returns` and the type:

```
proc larger($a: Whole, $b: Whole) returns Whole:
proc roll_dice returns Whole:
extern.rust proc beep returns Bool:
```

(How a procedure gives back its value in the body, a procedure that gives back nothing, and how failures are listed
in the header aren't decided yet.)

**Weighed.** Nothing else was put forward: the word had been in every example since D3, and Tankun locked it.

**Why this won.** It reads as what it says, "returns a whole number", in the header where a reader looks for what a
procedure takes and gives. It's Plainly v1's word.

**Tankun's words.** In answer to D36's question: "yea. `returns` locked".

## D38. A procedure that gives nothing back writes no `returns`

2026-11-20.

**Chosen.** A header writes `returns` only when the procedure gives back a value. One that gives nothing back writes
nothing there, and if it can fail, its failures follow on their own, with no `returns` and no `or`:

```
proc.main:
proc greet($name: Text):
proc save_game($state: Text) fails disk_full:
proc load_level($name: Text) returns Level or fails not_found:
```

(The word `fails` and how failures are listed were placeholders here, and aren't decided yet; nor is how a procedure
leaves early when it gives nothing back.)

**Weighed.**
- `returns nothing`, always, as Plainly v1's D100 chose, so every header reads "returns *this* or fails *that*" and a
  learner meets the difference between doing something and giving something back. Every header in every game pays
  for it, and v2's D4 writes only what's there.

**Why this won.** It follows D4: a definition writes only what's there, so `proc.main:` and the many procedures that
only do something stay short. The difference between doing and giving back can still be taught where someone trips on
it: using what such a procedure gives as a value is refused, with a message saying it gives nothing back. This is
Plainly v1's D46 before its D100.

**Tankun's words.** "`returns nothing or fail`? When there's void & fail?", then, asked "A procedure that gives
nothing back writes no `returns`, and `fails` alone when it can fail?", the owner chose: "yes".

## D39. A header lists its failures after `fails`, separated by commas, with no brackets

2026-11-20.

**Chosen.** A procedure that can fail lists its failures' names at the end of its header, after `fails`, separated by
commas. After `returns`, it's joined with `or`; with no `returns`, `fails` stands alone (D38):

```
proc load_level($name: Text) returns Level or fails not_found, broken:
proc save_game($state: Text) fails disk_full:
```

A failure's name is a plain word, like `not_found` or `disk_full`.

(How a procedure fails on purpose, the form of `otherwise:` and its cases, and handler families aren't decided yet.)

**Weighed.**
- `or fails [not_found, broken]`, as in Plainly v1. Since D3, `[ ]` means a list value, and a failure list is part of
  the definition. `( )` would look like a call.

**Why this won.** The header reads as English, "returns a Level, or fails: not found, broken", and the failures sit
next to what the procedure gives back, since a failure is what it can give back instead. The brackets follow the same
reasoning as D25's `labels` line.

**Tankun's words.** Asked "`fails`, then names separated by commas, no brackets?", the owner chose: "yea".

## D40. The `otherwise:` block: one `case` per failure, each ending in `instead`, `return`, `break` or failing

2026-11-20.

**Chosen.**

```
let $best: Whole = load_best_score() otherwise:
    case not_found:
        show_msg("No high score yet.");
        instead 0;
    case unreadable:
        instead 0;
end otherwise
```

- `otherwise:` after a call that can fail opens the block, and `end otherwise` closes it (D8).
- One `case` per failure, naming it (D39). The cases must cover every failure still possible at that call, so a
  procedure gaining a failure makes every incomplete block a build error.
- Each case ends in one of four ways: `instead` a value, carrying on with it; `return`; `break`; or failing on purpose.
  A case that could fall off its end is refused.
- `case` is indented one level in, and its body one level further (D10).

(How a procedure fails on purpose, and `return` itself, aren't decided yet.)

**Weighed.**
- `?? value` for every failure alike (D23). It can't treat failures differently, as when a missing file means "no score
  yet" but a broken one should warn.
- `match`, `catch` or `except` blocks, from other languages. `otherwise` reads as English after a call: "load the best
  score, otherwise...".

**Why this won.** Each failure gets its own handling where it's needed, the build checks none is forgotten, and every
case says how the line carries on. This is Cyborg's `otherwise:` and Plainly v1's D55, with v2's indentation.

**Tankun's words.** Asked "The `otherwise:` block like that?", the owner chose: "yea".

## D41. A family is a word and its kinds, declared together in a `fam.kind` block

2026-11-20.

**Chosen.** A family declares a word and its kinds together. `fam` narrowed by what it holds opens it, and it holds
only that kind of thing: `fam.proc` for a procedure and its narrowings, `fam.handler` for a failure's handlers.

```
fam.proc show_msg:
    proc show_msg($text: Text):
        ...
    end proc show_msg

    proc show_msg.same_line($text: Text):
        ...
    end proc show_msg.same_line
end fam.proc show_msg
```

where each `...` stands for that procedure's body.

- **Every member reads as "a Y kind of X"** for its name `X.Y`, or is the word `X` itself. Anything else is refused:
  `show_msg.same_line` is a same-line kind of showing, but `geometry.area` isn't a kind of geometry.
- **A narrowed procedure is declared inside its word's family,** so every variant is in one place.
- **Each member keeps its full name,** so one search finds its declaration and every use.
- **The test is what a narrowing is in general:** a kind. Everything v2 has passes it: `show_msg.same_line`,
  `show_msg.fmt`, `be.Whole`, `proc.main`, and handlers like `word_failed.use_placeholder`.
- `fam` is short for family (D2).

(How a handler family's handlers are written, handlers that take arguments, and handlers covering only some failures
aren't decided yet.)

**Weighed.**
- Cyborg's families before its D480: any named group, `fam.fn geometry` with `geometry.area()`, which grew into
  modules. Tankun's words: too big, when the purpose was `print` and `print.stored`.
- A family block with no kind, `fam show_msg:`, which Tankun questioned ("Woah, why no `.`?"): the opening line wouldn't
  say what's inside, and a family could mix procedures and handlers.
- No family block, with `proc show_msg.same_line` declarable anywhere. Shorter, but variants could scatter, and families
  stop being a visible thing.
- A `msg` family, `msg.show` and `msg.ask` (D20's alternative, which seemed worth reopening once families came in). It
  fails the test, "a show kind of message", so D20 stands.

**Why this won.** It's the original idea of families, a word and its variants, with Cyborg's test (its D480, refined
by Tankun on 2026-11-20) keeping them from growing into folders. The kind in `fam.kind` puts what's inside in the first
word, and lets the checker refuse anything else.

**Tankun's words.** "Wait what? That's not families", "Now, here's the thing. I think Cyborg's families are too big
grouping. You know what I mean? The purpose was originally `print`, and `print.stored` being different in a family.
Familiy now can create shit like different modules. And I think that is too big", "Woah, why no `.`?", "Actually,
wait. I think we should pause here, and let me go refine families in Cyborg. So, it would be better or more in my
vision. Since families are one of my biggest innovations.", then, after Cyborg's D480, asked "Families as a word and
its kinds, in a `fam.kind` block, with every member passing "a Y kind of X"?", the owner chose: "yea".

## D42. Handlers are declared with `handler` in a `fam.handler`, used with `otherwise`, and `*` marks one that can jump

2026-11-20.

**Chosen.**

```
fam.handler word_failed:
    handler word_failed.use_placeholder:
        case too_long:
            instead "...";
        case no_words:
            instead "(none)";
    end handler word_failed.use_placeholder

    handler word_failed.give_up:
        case too_long:
            return;
        case no_words:
            return;
    end handler word_failed.give_up
end fam.handler word_failed

let $title: Text = first_word($heading) otherwise word_failed.use_placeholder;
let $name: Text = first_word($typed) otherwise word_failed.*give_up;
```

- Each member of a `fam.handler` starts with `handler` and its full name, and is closed with `end handler` and that
  name, as `fam.proc`'s members are with `proc` (D41).
- A handler's body is `case`s, exactly as in an `otherwise:` block (D40). `otherwise family.handler` uses it at a call
  as if its cases were written there, so they must cover every failure still possible at that call.
- The family's word names what fails, so each handler reads as a kind of handling it (D41).
- A handler that can `return` or `break` is marked `*` where it's used, as in `word_failed.*give_up`: required on one
  that can jump, refused on one that can't.

(`return` itself, handlers that take arguments, handlers covering only some failures, and whether failing on purpose
inside a case counts as a jump aren't decided yet.)

**Weighed.**
- No mark, as an ordinary handler call. A `return` inside a handler leaves the caller's procedure, and nothing at the
  call would show that the lines after it might never run.
- The mark where the handler's declared. Then reading a call wouldn't show it.
- A handler header with no `handler` word, as Cyborg writes `word_failed.use_placeholder:`. `fam.proc`'s members start
  with `proc`, so v2's handlers start with `handler`.

**Why this won.** Handling that repeats across many calls is written once and named, and reads at the call as what it
does. The `*` at the use is the part of Cyborg's design that has most clearly earned its keep, Cyborg's design claude
reports: a hidden return out of the caller is the real surprise. This is Cyborg's handler families (its D46, D480) and
Plainly v1's D41.

**Tankun's words.** Asked "Handlers declared with `handler` inside a `fam.handler`, used as `otherwise
family.handler`, with `*` at the use for one that can jump?", the owner chose: "yea".

## D43. `include` brings a handler's cases into an `otherwise:` block, beside cases of its own

2026-11-20.

**Chosen.** Inside an `otherwise:` block, `include family.handler;` brings in that handler's cases as if written
there, and the block's own `case`s cover the failures it doesn't:

```
let $title: Text = load_word($file) otherwise:
    include word_failed.use_placeholder;
    case disk_full:
        instead "?";
end otherwise
```

- Together, the included and written cases must cover every failure still possible at the call, with no failure
  covered twice (D40).
- A handler that can jump is marked as at any use: `include word_failed.*give_up;` (D42).

(Handlers that take arguments aren't decided yet.)

**Weighed.**
- `use`, the word first proposed. Vaguer, since plain `otherwise family.handler` also uses a handler, and it's the
  word other languages use for importing names, which v2 may want later.
- No combining, so a call with one failure the handler doesn't cover needs a whole new handler. Cyborg's design claude
  warned to settle this early, since Cyborg never did.

**Why this won.** A handler covering some failures stays reusable on calls that can fail in more ways. `include` says
exactly what happens, its cases included as if written there, and leaves `use` free. Tankun found the word.

**Tankun's words.** "`include` instead?", then, asked "`include` inside `otherwise:` to combine a handler with extra
cases?", the owner chose: "yea".

## D44. Handlers take parameters as procedures do

2026-11-20.

**Chosen.** A handler can take parameters, written as a procedure's are: `( )` only when it has some (D4), a `labels`
line at the top of its body (D25), and arguments in `( )` with their labels where it's used (D24). An argument is
worked out at the call, before the cases run, and the cases read it like any parameter.

```
fam.handler word_failed:
    handler word_failed.use_default($fallback: Text):
        labels $fallback as with;
        case too_long:
            instead $fallback;
        case no_words:
            instead $fallback;
    end handler word_failed.use_default
end fam.handler word_failed

let $title: Text = first_word($heading) otherwise word_failed.use_default(with: "Untitled");
```

`include` takes arguments the same way: `include word_failed.use_default(with: "?");` (D43).

**Weighed.**
- No parameters, so every fallback value needs its own handler. Cyborg's design claude reports this as the gap its
  users hit first ("use_placeholder" wants to be "use_default(with: …)"), and Cyborg never settled it.

**Why this won.** A handler is reusable across calls that want different fallbacks, and it's written with the rules
procedures already use, so nothing new is learnt.

**Tankun's words.** Asked "Handlers take parameters like procedures do?", the owner chose: "yea. Let's decide arch,
like backend, memory, etc"

## D45. The compiler is written in Rust and compiles ahead of time to native code through Cranelift

2026-11-20.

**Chosen.** Plainly's compiler is written in Rust. It compiles a whole program ahead of time to one native executable,
through Cranelift, with no bytecode and no JIT. Running a program compiles it to a temporary file and starts it, as
`go run` does. The web playground runs the oracle compiled to WebAssembly, so the compiler needs no second backend for
the web.

(The command's name and its parts, build modes, and the oracle's language aren't decided yet.)

**Weighed.**
- LLVM. It optimizes more, but compiles far slower, so running a program wouldn't feel instant; a terminal game won't
  notice the difference in speed.
- Bytecode with an interpreter or a JIT, as Python and Java do. A runtime to install alongside every game, and a
  bytecode format to design and keep stable.
- Compiling to C, or to WebAssembly first. An extra toolchain on every player's machine, or a web-first design, which
  multifinity's notes warn killed Fale.

**Why this won.** A player gets one file that starts instantly, matching the "Download" installer in multifinity's
notes. Cranelift compiles fast enough that running feels immediate, covers x86-64 and ARM64, and is a Rust library, so
the compiler and `extern.rust` procedures (D36) share one world. This is Plainly v1's D16.

**Tankun's words.** Asked "A Rust compiler, ahead of time to native through Cranelift?", the owner chose: "yea, what
about build modes? `--Working` (no optimization, every runtime check kept) and `--Final` (max optimization from backend
and frontend, cheap checks kept)?"

## D46. Two build modes, working and final, with no default; final never drops a check

2026-11-20.

**Chosen.** Every build names one of two modes, with the flags `--working` and `--final`. There's no default.

- **Working:** no optimizations, so the fastest compile, and every check kept, each stopping at the exact line:
  whole-number overflow, a bad list position, division by zero, NaN in floats.
- **Final:** full optimization, from Plainly's own front end and from Cranelift. It never drops a check; it only makes
  expensive ones cheaper. Overflow, position and division-by-zero checks cost almost nothing and stay. Checks the
  optimizer proves can never fail are removed. The NaN check after every float operation becomes the CPU's own flags,
  read where a value leaves a procedure; its message names the procedure, and says to build in working mode to find
  the line.

(How the modes are written in docs and messages, the command's name, and what each check's message says aren't
decided yet. Overflow, positions, division and floats themselves aren't decided either; the list says what's checked
once they are.)

**Weighed.**
- Plainly v1's three modes, `Working`, `FinalSafe` and `FinalPerf` (its D19), where the third dropped safety checks for
  speed. A terminal game never needs that speed, and a dropped check turns a clear stop into a silent wrong answer.
- `--Working` and `--Final`, capitalised, as Tankun first wrote them. Command-line flags are lowercase almost
  everywhere.
- A default mode. Someone ships a slow working build by accident, or debugs in final without knowing it.

**Why this won.** Two modes cover the two jobs, writing a game and shipping it, and the rule that final never drops a
check means a shipped game is exactly as safe as the one that was tested. Tankun proposed the pair.

**Tankun's words.** In D45's answer (quoted there), Tankun proposed `--Working` and `--Final`. Asked "Two modes,
`Working` and `Final`, where `Final` never drops a check, with no default and lowercase flags?", the owner chose: "yea,
lowercase in docs too?"

## D47. Build modes are written lowercase everywhere

2026-11-20.

**Chosen.** The modes are `working` and `final` everywhere: on the command line, in the docs and in messages, as in
"build in `working` mode" and "build with `--working` to find the line".

**Weighed.**
- `Working` and `Final`, capitalised in docs and messages, as Plainly v1 wrote them. Each mode would have two spellings,
  the flag's and the name's.

**Why this won.** One spelling per thing (D2), and a reader sees exactly what they type. Tankun raised it.

**Tankun's words.** "yea, lowercase in docs too?" (in D46's answer), then, asked "Modes written lowercase everywhere,
docs and messages included?", the owner chose: "yea".

## D48. Memory is managed by a Go-style garbage collector that never moves objects

2026-11-20.

**Chosen.** Plainly frees memory with a tracing garbage collector in the style of Go's:
- it marks what's still in use while the program keeps running, so pauses stay well under a millisecond whatever the
  memory size;
- it never moves objects, so an `extern.rust` procedure (D36) can hold a plain pointer during a call;
- it needs only a write barrier, a check when a reference is stored;
- it groups objects by size, which keeps memory tidy without moving anything;
- it's precise, finding every reference through the stack maps Cranelift records (D45).

It may be built first as a simple mark-and-sweep with the same never-moving promise, growing into the full design
without changing any program.

**Weighed.**
- A simple mark-and-sweep only. Simplest to build, but its pauses grow with memory, which limits Plainly if it grows
  past terminal games.
- A moving, generational collector, as Java's and .NET's. Fast, but `extern.rust` code would need to pin objects.
- Reference counting, Swift-style, with copy-on-write. It frees memory the moment it's unused and appealed as fun to
  build ("But Swift-style seems fun, though"), but its count updates thread through the whole compiler, and with
  `share` (D49) it leaks cycles unless the programmer marks links `weak` or a backup cycle collector is added, as
  Python has. Tankun pointed out that for terminal games its main gains, no pauses and flat memory, are worth little
  ("Wait, look at our target, CLI games").
- Hylo's mutable value semantics: nothing is ever shared, so memory needs no runtime bookkeeping. The purest, but a
  research design, and it rules out `share` entirely.
- Ownership, as Rust's and Cyborg's. Borrowing rules are wrong for "static Python".

**Why this won.** Tankun asked for "the more flexible kind": Go's design scales from a small terminal game to anything
bigger, like a GUI package, with the same promises, handles every cycle `share` can make, and keeps Rust code simple
because nothing moves. This is Plainly v1's D38 (PlainGC). Its cost is size, roughly 5,000 to 12,000 lines of Rust by
v1's estimate, which the simple first version spreads out.

**Tankun's words.** "How about Swift-style?", "Wait, look at our target, CLI games", "But Swift-style seems fun,
though. What's share?", "What's Hylo's", "What is share for, though?", "What about RC with `share` and `copy` when
needed?", "So, what about `share` & `copy` with GC? What kind of GC, though. I would take the more flexible kind",
then, asked "Go-style GC (non-moving, collecting while the program runs), with `copy` and `share` required when
assigning a list or record?", the owner chose: "yea".

## D49. Assigning a list or record says `copy` or `share`

2026-11-20.

**Chosen.** When a list or a record is given to a new name, the line says whether that name gets its own copy or
shares the original, and there's no default:

```
var $b: List<Whole> = copy $a;
var $c: List<Whole> = share $a;
```

Changing `$b` leaves `$a` alone. Changing `$c` changes `$a`, since they're two names for one list, and the word
`share` on that line says so. A plain value like a `Whole` needs neither word, since copying and sharing it look the
same. `copy` really copies: the garbage collector (D48) keeps no counts to make it lazy.

(Lists, records, `List<…>`, whether arguments and changes with `=` need a word, and copying cost aren't decided yet.)

**Weighed.**
- Values only, with no `share`, as in Hylo. Nothing can be aliased, but things that really are one thing, like the
  player every monster targets, must be referred to by something like a number and looked up.
- Sharing by default, as in Python, where `b = a` makes two names for one list. Changing one variable then changes
  another with nothing on the page to say so, a classic beginner surprise.
- Copying by default. Hides when a large copy happens.

**Why this won.** Sharing is there when it's really meant, and both cases are visible on the line that makes them,
so "I changed `$b` and `$a` changed too" can only happen where `share` is written. This is Plainly v1's D32, which
Tankun brought back here ("What about RC with `share` and `copy` when needed?").

**Tankun's words.** Decided with D48, in the same answer (quoted there).

## D50. The command is `plainly`, with `run`, `build`, `check` and `oracle`

2026-11-20.

**Chosen.**

```
plainly run game.plainly --working
plainly build game.plainly --final
plainly check game.plainly
plainly oracle game.plainly
```

- `run` builds to a temporary file and runs it (D45); `build` makes the file to ship; both need a mode (D46).
- `check` only reports what's refused, without building, so it needs no mode.
- `oracle` runs the file on the oracle, the separate reference implementation, printing exactly what the compiled
  program would, with the same exit status. It needs no mode: it never builds, and keeps every check. `plainly`
  forwards to the oracle's own program, as `git something` runs a separate `git-something`, so the oracle stays
  separate code (D12); if it isn't installed, `plainly oracle` says how to get it. `plainly help` lists it as the
  reference implementation, for testing.

(A formatter isn't needed, since D10 refuses wrong indentation. Commands for tests or new projects, and the messages,
aren't decided yet.)

**Weighed.**
- `apy`, after the compiler, as Plainly v1 had. A second name to learn, and "APY" is a finance term crowding search
  results.
- The oracle as its own command, `plainoracle game.plainly`, as in Plainly v1, to keep it away from beginners. Tankun
  asked "Why not `plainly oracle`?": one name for all of Plainly's tools, with forwarding keeping the oracle separate
  and optional, and the help text saying what it's for.

**Why this won.** Everything is under the language's own name. The oracle's users are the test suite, compiler
developers, the web playground and bug reports, and `plainly oracle` serves all of them without making it part of the
compiler.

**Tankun's words.** "With `plainly oracle`?", "Why would they use it, then?", "So, how is the oracle gonna be tested?",
"Yeah, no shit, sherlock. But what the fuck do you say to run it", "Why not `plainly oracle`?", then, asked "`plainly`
with `run`, `build`, `check` and `oracle`, where `oracle` forwards to the separate oracle program?", the owner chose:
"yea. Names:
PlainCompiler
_Oracle
_GC" (the names are D51).

## D51. The parts are PlainCompiler, PlainOracle and PlainGC

2026-11-20.

**Chosen.** Plainly's compiler is PlainCompiler (D45), its reference implementation PlainOracle (D12, D50), and its
garbage collector PlainGC (D48). The command stays `plainly` (D50).

**Weighed.**
- Plainly v1's AroundPy (AroundPython, APy) for the compiler, a joke about its speed. A name that has to be explained.

**Why this won.** Each name says what the part is, with the language's name in front, so they read as one family of
tools. Tankun named them, writing `_` for "Plain".

**Tankun's words.** In their answer to D50, quoted whole there.

## D52. PlainOracle is written in C

2026-11-20.

**Chosen.** PlainOracle, the reference implementation (D12, D50, D51), is written in C.

(The C standard and compiler, how its memory, tagged values, overflow checks and Unicode are handled, and the guard
rails that keep it obviously correct aren't decided yet.)

**Weighed.** The design claude recommended OCaml throughout, and Tankun chose C.
- OCaml, as Plainly v1's oracle was (its D17): pattern matching that reads like the standard's rules, a garbage
  collector for free, and `wasm_of_ocaml` for the browser playground; a second toolchain, and `Int64` needed for every
  `Whole`.
- C++: `std::variant` and `std::visit` for checked tagged values, and `unique_ptr` and standard containers so memory
  frees itself; a large language needing a strict subset to stay readable.
- SPARK at Silver: checked arithmetic, checked variant records, and a proof that it never crashes, but not that its
  answers are right; pointers only under ownership rules with no garbage collector, and an experimental path to
  WebAssembly.
- The case against C: a bare `union` lets the wrong field be read, memory is freed by hand, text is bytes with no
  Unicode, and signed overflow is undefined behaviour unless every addition uses a checked form such as C23's
  `ckd_add`.

**Why this won.** It's an experiment: Tankun wants to see how many problems C causes. So every problem C causes
in PlainOracle is recorded, as it happens, in [`design/c-problems.md`](design/c-problems.md): what went wrong, which
part of C caused it, how it was caught, and what it cost. C also needs nothing beyond a C compiler, runs everywhere,
compiles to WebAssembly through Emscripten, and is a different language from PlainCompiler's Rust, so the two can't
share code or bugs (D12).

**Tankun's words.** "You know what's fucking funny? Clang C", "Can I see C syntax?", "What about SPARK? Not hardcore
SPARK, though", "C or C++ for PlainOracle, if there's two options only", "C for PlainOracle, and I'm not kidding.",
then, asked for the reason, "My reason for C is that I wanted to see how much problems C would cause, so make sure to
record them."

## D53. PlainOracle is careful modern C: C23, Clang, every warning, and sanitizers on every test run

2026-11-20.

**Chosen.** PlainOracle is written the way a serious C project is today:
- C23, so overflow is checked with the standard `ckd_add`, `ckd_sub` and `ckd_mul`;
- compiled with Clang, with every warning on and warnings treated as errors;
- every test run under AddressSanitizer (memory errors) and UndefinedBehaviorSanitizer (undefined behaviour).

Everything these guard rails catch is still logged in `design/c-problems.md` (D52).

**Weighed.**
- Plain C, with no guard rails. It would mostly measure how many bugs slip through unwatched, which says more about the
  setup than about C.

**Why this won.** D52's experiment measures what C costs as it's really written, and the log then shows both what C
gets wrong and what it takes to catch it.

**Tankun's words.** Asked "Careful modern C (C23, Clang, all warnings, sanitizers on every test run), with everything
the guard rails catch still logged?", the owner chose: "yea. We have naming rules already, right?"

## D54. The layout of a message

2026-11-20.

**Chosen.**

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

- **The first word is the kind:** `refused:` while building, `check failed:` while running.
- **The place is written twice:** `file:line:column` for editors and tools, then `line N column M` for people.
- **The source line is shown,** with `^` under exactly the part in question.
- **Then labelled lines, each only when known, in this order:**
  1. `fix:`, in place of `what:`, when there's exactly one fix; otherwise `what:`, what's wrong;
  2. `when:`, the values that broke a check while running;
  3. `because:`, the earlier place the error comes from, such as a declaration or an earlier error, as a sentence,
     followed by that line with `^` under the part that causes it; in another file, the sentence names the file;
  4. `why:`, the rule, as a plain sentence a beginner can read.
- **No error codes, and no "did you mean" guesses:** only what's certain.
- **Every error is shown,** and a broken line doesn't set off false errors after it: a refused declaration still
  creates its variable with its written type.

(Each message's wording, how columns are counted in text that isn't plain English letters, and the exit statuses
aren't decided yet.)

**Weighed.**
- `because:` as a sentence alone, as first proposed. Tankun asked for the code too: a beginner shouldn't have to
  scroll to find the line that causes the error.
- Showing every line between the two places. A long gap would make a long message.
- Error codes, as C compilers and Rust have. They're for looking up, where Plainly's messages say it all.
- "Did you mean" guesses. A wrong guess is worse than none.

**Why this won.** Each message says where, what, why, and where it came from, in words and in code, with nothing
uncertain. This is Plainly v1's D58 to D60 and Cyborg's format, with `because:` showing its line, as Rust's "declared
here" notes do.

**Tankun's words.** "Where's because? That msg seems like it should have because", "Should we have because show code
too?", then, asked "That message layout, with `because:` showing its line?", the owner chose: "yea".

## D55. Anti-cascade: one mistake gives one message

2026-11-20.

**Chosen.**
1. **After an error, only that statement is skipped,** and checking carries on, so one build shows every real error.
2. **A refused declaration still creates its variable** with its written type (D13), so later lines don't fail
   because it "doesn't exist".
3. **A refused expression's type becomes "unknown",** and nothing is refused for using an unknown value, so one
   mistake can't set off a chain of messages about everything using its result.
4. **Knock-on errors are sorted out:** the first error's fix, when it has exactly one, is applied in memory and the
   program checked again. Anything that disappears was caused by it, and is shown under a `because:` pointing back at
   it (D54), not as an error of its own.
5. **A broken block opener:** if it's clear what was meant, as with a header missing only its `:`, checking carries
   on into the body. If it isn't clear, checking stops there.

**Weighed.**
- Stopping at the first error, as some compilers do. One fix per build, which is slow to work through.
- Skipping a broken block to its `end` and carrying on. With a header too broken to read, the matching `end` can be
  misjudged, making every message after it wrong (Plainly v1's D62).
- Reporting every error as found, with no sorting. A beginner sees ten messages for one typo and starts at the wrong
  one.

**Why this won.** A wrong message is worse than a missing one (D54), and a cascade of false ones buries the real
mistake. This is Plainly v1's D58 and D62, Cyborg's rules, and what v1's PlainOracle did.

**Tankun's words.** "Where's Anti Cascade?", then, asked "Those five anti-cascade rules?", the owner chose: "yea".

## D56. Exit statuses

2026-11-20.

**Chosen.**

| Status | When |
|---|---|
| 0 | the program ran to its end |
| 1 | it was refused while building |
| 2 | the file couldn't be read |
| 3 | `NotBuiltYet` or `NotDesignedYet`: PlainCompiler or PlainOracle can't handle something yet |
| 4 | a check failed while running, like overflow (D46) |
| 5 | a failure left `proc.main`, or `?? crash` stopped the program on purpose (D23) |

`NotBuiltYet` is for something decided but not built yet, and `NotDesignedYet` for something no decision covers yet.
They're the implementations' own limits, not Plainly errors, and use D54's layout with their name as the kind.

(What a failure leaving `proc.main` shows, and how `NotBuiltYet` and `NotDesignedYet` are worded, aren't decided yet.)

**Weighed.**
- One status for every kind of failure, as many programs do. A test couldn't tell "refused" from "crashed" without
  reading the message.
- Status 4 for a deliberate crash too. A failed check is a bug; giving up on purpose isn't.

**Why this won.** Each kind of ending has its own number, so tests and scripts can tell them apart. 1 to 4 are Plainly
v1's D71 and D82; 5 is new, since v1 never decided it.

**Tankun's words.** Asked "Those six exit statuses?", the owner chose: "yea".

## D57. `return value;` gives back a value, and a bare `return;` leaves a procedure that gives nothing back

2026-11-20.

**Chosen.**

```
proc larger($a: Whole, $b: Whole) returns Whole:
    if $a > $b:
        return $a;
    else:
        return $b;
    end if
end proc larger

proc greet($name: Text):
    if $name == "":
        return;
    else:
        show_msg.fmt("Hi, {$name}!");
    end if
end proc greet
```

- **`return value;`** gives back a value, in a procedure with `returns` (D37). It must be the header's type, and every
  way through the procedure must end in one, or the procedure is refused.
- **A bare `return;`** leaves early, in a procedure with no `returns` (D38).
- **Nothing may follow `return` in the same block,** since it could never run, as with D9.
- **A bare `return;` as a procedure's last line is refused,** since the procedure ends there anyway.

(`$name == ""`, comparing text, isn't decided yet.)

**Weighed.**
- `return nothing;`, as in Plainly v1 (its D100), which matched v1's `returns nothing` headers. v2's headers write
  nothing when nothing comes back (D38), so neither does `return`.

**Why this won.** `return` is followed by exactly what the header says comes back: a value, or nothing. Code that can
never run is refused rather than left as a trap.

**Tankun's words.** Asked "`return` like that?", the owner chose: "yea.
What about adding `proc larger(Whole $a, $b) returns Whole:`?
Infront like that means every" (the second part is still being weighed).

## D58. Names have no `$`

2026-11-20.

**Chosen.** A variable's name is a plain word with no mark, written the same where it's made and where it's used:

```
let secret: Whole = random(from: 1, to: 100);
var tries: Whole = 0;
tries = tries + 1;
show_msg.fmt("You got it in {tries} tries.");
proc add(a: Whole, b: Whole) returns Whole:
```

A name can't be a keyword: a program using a keyword as a name is refused. If a later version of Plainly adds a
keyword, a program already using that word as a name stops building and has to rename it.

This replaces D11. Entries before this one write names with `$`.

**Weighed.**
- `$` on every name (D11). It keeps names apart from keywords in text with no colour, which matters most in a language
  that reads in words (`if hp at_least max and not won:`), means a new keyword never breaks a program, and tells
  autocomplete a variable comes next. Tankun: editors are more practical than that, the command line can colour code
  too, and Plainly is a language for learning, not one with Cyborg's guarantees: "If your shit breaks, sorry, but it
  broke".
- `$` only where a name is used, as shell scripts do (`name=Tankun`, `echo $name`). Two spellings for every variable,
  and shell's famous beginner trap.
- Highlighting in place of `$`, as asked. It helps where there's colour, but chats, plain terminals and editors without
  Plainly support show code bare.

**Why this won.** Code is mostly written and read in editors, the command line and the docs, all of which can colour
it, so the mark mostly costs a character per name. Without it, Plainly looks more like Python, as "static Python"
suggests. The costs are accepted: names and keywords look alike where there's no colour, a new keyword can break a
program, and words can't become operators over common names, so multiplying is `*`, not `x` (D59).

**Tankun's words.** Their answer to D57 (quoted whole there), "You can't do that, lol", "yea", has code, "Removing `$` would make it faster, but idk if
that's worth it", "What if we required it at use only?", "Wait, I think we forgot IDEs exist", "Wait, can't highlight
replace `$`?", "But IDEs are alot more practical, and CLI could do highlighting. Plus it's a learn language, rather
than Cyborg-style garuntees. If your shit breaks, sorry, but it broke", "We could replace `x` with `*`, and `*` with
`^`?", "Wouldn't that make it two meanings?", "Wait, then `=` is double meaning. So, maybe we should loosen the
rule?", "No, look at `!=`", "So, then we get `!` marking for free? Or just `*` again?", then, asked "Drop `$`, and
multiply with `*`?", the owner chose: "yea".

## D59. `*` multiplies

2026-11-20.

**Chosen.** `*` multiplies, with a space on each side, as in nearly every language:

```
let total: Whole = price * count;
```

`.*`, the mark on a handler that can jump (D42), is a different symbol: it's written stuck to a dot, so it's read as
one symbol of its own, and never comes where multiplying could. D42 doesn't change.

The rule behind that: **each symbol, read as a whole, has one job.** A longer symbol counts as its own, so pieces
inside it may echo other symbols: `!=` echoes `=`, `|>` echoes `>`, and `.*` echoes `*`. What the rule still refuses
is one whole symbol doing two jobs.

(Dividing, maths order, and going past `Whole`'s range aren't decided yet.)

**Weighed.**
- `x`, the sign taught at school, as in Plainly v1 (its D84). Without `$` (D58), `x` couldn't also be a name, and `x`
  is the most common name in a game, a position.
- Moving the jump mark to `^` (Tankun's idea), or to `!`. Both were allowed once the rule was stated as above, and
  neither adds anything over keeping `.*`, which is Cyborg's and Plainly v1's mark.
- A strict rule that no symbol appears in two meanings, even inside another. `=` already means "gets this value" alone
  and "equal" inside `!=`, as in nearly every language since C, and avoiding that would need `/=` or `:=`.

**Why this won.** `*` is what every other language and calculator app uses, and frees `x` to be a name. Stating the
symbol rule by whole symbols keeps it honest about `=` and `!=`.

**Tankun's words.** Decided with D58, in the same answer (quoted there).

## D60. A symbol has one job in any one place

2026-11-20.

**Chosen.** A symbol may do different jobs in different places, but only one job in any one place:
- `-` between two values subtracts, and right before one negates (D27);
- `<` and `>` between two values compare (D17), and right after a type's name fill it in (D3);
- `*` between two values multiplies (D59), and right after a dot marks a handler that can jump (D42).

Two jobs for one symbol in the same place would be refused as a design. A longer symbol, like `==`, `!=`, `|>` or
`??`, is a symbol of its own, so the pieces inside it don't count.

This replaces D59's wording of the rule ("each symbol, read as a whole, has one job"), which `-` and `< >` already
broke. D59's choice of `*` stands.

**Weighed.**
- "One job unless spacing or position tells the jobs apart at a glance", as the design claude first put it. It says
  the same thing more loosely; Tankun's version names the real line: the place.
- One job per symbol everywhere, as first assumed. `-` and `< >` already break it, and so would `.*`.

**Why this won.** What a symbol does is decided by where it's written, never guessed from the meaning around it, just
as a reader learns `-5` and `a - b` apart without thinking. Tankun put it this way.

**Tankun's words.** "So, what should the loosen rule be?", "No, I meant the symbol", "They aren't in the same place",
then, asked "Record it: a symbol has one job in any one place?", the owner chose: "yea".

## D61. Parameters sharing one type write it once, in front

2026-11-20.

**Chosen.** When a procedure has two or more parameters, all of one type, the type is written once, in front, and
covers them all. With mixed types, or a single parameter, each parameter writes its own type after its name:

```
proc add(Whole a, b) returns Whole:
proc draw(x: Whole, name: Text):
proc double(n: Whole) returns Whole:
```

The other form is refused, with the right one as the fix, so each header has exactly one spelling: the form follows
the situation, and the writer never chooses (D2's aim, as D60 does for symbols).

**Weighed.**
- Each parameter always writing its own type, `add(a: Whole, b: Whole)`, as the design claude recommended. One order
  everywhere, matching `let a: Whole` (D13), and nothing to rewrite when a parameter of another type is added. Tankun
  compared the two side by side and found the short one faster (has code).
- A type in front covering every name after it until the next type, as in C's `int a, b`. The design claude first
  misread Tankun's idea this way; to find a parameter's type you scan back past others, and C's `int* a, b` shows the
  trap.
- Go's order, `add(a, b: Whole)`, type after the names it covers. To find the first name's type you scan forward.
- Allowing both forms. Two spellings for one header.

**Why this won.** With every parameter one type, the short form reads faster: "all Whole" at a glance. Requiring it
exactly then, and the long form otherwise, keeps one spelling per header. The costs are accepted: the type comes first
only in short headers, and adding a parameter of another type means rewriting the header, which the refusal's fix
does.

**Tankun's words.** In their answer to D57 (quoted whole there), then the messages listed in D58; asked "The short form required when two or more parameters share
one type?", the owner chose: "yea".

## D62. PlainCompiler's stages, with one IR of Plainly's own

2026-11-20.

**Chosen.**

```
source → syntax tree → typed tree → Plainly IR → Cranelift IR → native code
```

- **Syntax tree:** the program as written.
- **Typed tree:** names resolved and types attached. Refusals happen here, so `plainly check` (D50) stops after it.
- **Plainly IR:** the one intermediate form of Plainly's own, a graph of simple steps where everything hidden is
  written out: each check (D46), each allocation for PlainGC (D48), labels and defaults filled in, and each handler's
  cases (D42). `final` mode's own optimizations run on it (D46).
- **Cranelift IR,** then native code (D45).

Both build modes take the same path and differ only in optimizations, so a translation bug shows in `working` mode too,
where tests and PlainOracle catch it.

(Which optimizations `final` mode makes isn't decided yet.)

**Weighed.**
- No IR of Plainly's own, going from the typed tree straight to Cranelift. Plainly's own optimizations, like removing
  checks it can prove, need a form that knows Plainly's types and checks, which Cranelift's doesn't.
- Two IRs of its own, as Go has. Go needs them because it writes its own backend; with Cranelift doing the low-level
  work, one is enough.

**Why this won.** It's Swift's shape (its SIL sits between the syntax tree and LLVM), and Plainly v1's D22: one layer of
Plainly's own for its checks and optimizations, with the backend library doing the rest.

**Tankun's words.** Asked "These stages for PlainCompiler?", the owner chose: "yea".

## D63. The standard library targets the terminal, on `crossterm`; a GUI only as a later package, if ever

2026-11-20.

**Chosen.** Plainly's standard library targets the terminal: showing text (D6), asking (D20), reading single key
presses, colours, drawing at a position and clearing the screen, on Windows, macOS and Linux alike. It's built on
`crossterm`, a Rust crate that handles every terminal's differences, through `extern.rust` procedures (D36). Examples
and the README show terminal games. A GUI, if it ever comes, is a separate package wrapping a library like raylib
behind `extern.rust`, and the language doesn't change for it.

(The library's procedures and names, beyond those decided, aren't decided yet.)

**Weighed.**
- Writing terminal escape codes directly. Every terminal's differences, Windows' especially, would land on Plainly.
- A GUI in the standard library. A far larger library, for games Plainly isn't aimed at first.
- The browser as the main target. multifinity's notes warn that Fale died drifting into a contained sandbox; the web
  playground stays a trailer (D45).

**Why this won.** Offline terminal games are Plainly's first target, and `crossterm` is the proven way to reach every
terminal from Rust. This is Plainly v1's D49.

**Tankun's words.** Asked "The standard library built on `crossterm`, terminal first, with a GUI only as a later
package?", the owner chose: "yea, if ever".

## D64. A program is every `.plainly` file in its folder; packages go in a manifest; `package::` settles a clash

2026-11-20.

**Chosen.**
- **A program is every `.plainly` file (D35) in its folder.** There are no imports between a program's own files: a
  procedure in one file calls one in another directly, in any order. There's exactly one `proc.main` across them all.
- **Packages are listed once, in a manifest file in the folder,** and their procedures are then usable in every file,
  with no imports.
- **Names are used bare,** as in `draw(...)`. When two packages both have a name, the bare name is refused, with the
  fix naming each package's version: `crossterm::draw(...)`. `::` names a package, one level deep, never a path inside
  one.

Every name in a program shares one space, so two of its files can't both declare the same procedure.

(The manifest's name and format, versions, and where packages come from aren't decided yet.)

**Weighed.**
- Import lines at the top of each file, as Python has. More to write in every file, and "which file is this from?"
  puzzles.
- Paths, like Rust's `std::io::stdin`. Tankun refused paths in Plainly v1 (its D39).
- A space of names per file. Two files could both have `helper`, at the cost of imports between them.

**Why this won.** It's Go's shape: a folder is a program, so a beginner never writes an import, and the rare clash is
settled on the spot with the package's name. This is Plainly v1's D39 with v2's files.

**Tankun's words.** Asked "One program per folder, a manifest for packages, bare names with `package::` only for a
clash?", the owner chose: "yea".

## D65. Compiling: one background thread while you edit, every core when you run or build, and a shared cache

2026-11-20.

**Chosen.**
- **Background:** while you edit, PlainCompiler compiles after every save on one thread of its own, inside the editor
  support (the language server), in `working` mode or the mode last run, filling the cache. It never blocks typing,
  and a newer save cancels an unfinished build.
- **Parallel:** `plainly run` and `plainly build` compile whatever the cache doesn't have on every CPU core, procedure
  by procedure, since Cranelift compiles each on its own.
- **Cached:** each compiled piece is saved under a fingerprint of everything that affects it (its code, the compiler's
  version, the mode) in the system's cache folder, shared by every project. Only what changed is recompiled, and a
  build can never differ because of the cache.

Without an editor, `plainly run` compiles on demand, in parallel, with the cache.

(The language server itself, the cache folder's place on each system, and clearing the cache aren't decided yet.)

**Weighed.**
- Background compiling in parallel too. It would take every core while you work, making the editor and a game being
  tested stutter.
- Background checking only, compiling at `run`. `run` would wait for the whole compile.
- A cache per project. The same package compiled again for every game.

**Why this won.** `plainly run` feels close to instant (D45) because most of the work was done while you were typing,
without slowing you down; when you're waiting, everything goes to the build. Keying the cache by fingerprint is how
Cyborg's `okay` keeps cached builds exactly equal to fresh ones.

**Tankun's words.** "no. What about compiling? Para, concur, cached, background?" (what the "no" answered, about threads in
the language, is still to be checked), "Let me answer compiling first. By background I meant that it compiles on
another thread while you work", "Background compiling is on another thread, but not para", then, asked "Background
compiling on one thread while you edit, parallel when you run or build, and a shared cache?", the owner chose: "yea".
