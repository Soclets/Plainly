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
