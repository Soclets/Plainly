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
