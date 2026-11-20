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
