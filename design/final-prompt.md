# Prompt for `final` mode

Copy the block below and paste it into the compiler session (Plainly Compiler Claude).

````text
<context>
Your next milestone is `final` mode: `plainly run` and `plainly compile` with `--final`, making the optimizations D124
lists. Tankun chose them one by one, including gfortran's fib trick and Graphite-style loop nests, because building
real compiler work like this is the part of Plainly they find fun. Plainly v1 was abandoned before its fun parts were
ever built, so build this properly, and tell Tankun in plain words what each optimization does as it starts working,
ideally with a before-and-after timing.
</context>

<sources>
Read these first: D46 and D47 (the modes, and the rule that `final` never drops a check), D62 (the stages, and Plainly
IR as where `final`'s own optimizations run), D124 (the twenty-one optimizations, with what each does), D45
(Cranelift), D69 and D105 (PlainGC and handles, for 4 and 5), and standard chapter 10's Build modes section. Your plan
for this milestone is already in `compiler/PROGRESS.md`, in five groups. CLAUDE.md's rules apply as before, including
the Code section and the shared-folder rules in your first prompt.
</sources>

<the_central_rule>
A program built with `--final` behaves exactly as it does with `--working`: the same standard output, the same
standard error, the same exit status, for every input. Only its speed may change. A check is removed only where the
optimizer proves it can never fail (D46), and a check that stays still stops at its exact line, with the same message,
even inside inlined or reordered code: keep each check's source place through every optimization.

If an optimization would change what any program does, it's wrong, however much faster it is. Where D124 or the
standard leaves something open, bring it to the design session with a labelled placeholder meanwhile, never a choice
of your own that a program could come to rely on.
</the_central_rule>

<order>
Build in the five groups in your plan, one optimization at a time, each ending working and tested, committed and
pushed:
1. Groundwork: 1, 2, 8 and 13, with the both-modes test below built first.
2. Procedures: 3, 19, 6, 7, 9, 14 and 20.
3. PlainGC's side: 4 and 5.
4. Loops: 18, 15, 11 and 17.
5. After lists: 10, 12, 16, and Graphite (21) last. Lists aren't built, so this group waits; when you reach it, stop
   and say so, unless lists have been built by then.
</order>

<keep_going>
This milestone is long, and Tankun wants it built straight through. A message with no tool call in it ends your turn,
and the work stops until Tankun comes back. Four ways of stopping aren't wanted:
1. a summary that announces the next optimization instead of starting it;
2. an offer to carry on unless told otherwise;
3. a list of questions when none of them blocks the next step;
4. stopping because an optimization or a group is done and it feels like a good place to report.
Status notes are welcome: put them in the same message as your next tool call and carry on. Questions go in
PROGRESS.md and to the design session, with a labelled placeholder meanwhile.

The stops that are wanted: group 5 while lists aren't built; bedtime (8:30 PM, CLAUDE.md); Tankun telling you to
stop; and anything hard to undo, which still needs Tankun's OK first.

Your context will be compacted automatically as it fills, so never stop or rush because of how much context is left.
Commit often, and keep PROGRESS.md current enough that after a compaction you can carry on from it and `git log`.
</keep_going>

<general_solutions>
Each optimization must work for every program it applies to, not for the tests or the benchmarks. Never recognise a
benchmark by its name or shape: the fib trick (6) has to come from detecting procedures with no side effects,
inlining and merging repeated calls in general, exactly as GCC does it, so that any pure recursive procedure gains,
not `fib` alone. If a test or the standard looks wrong, tell the design session rather than working around it.
</general_solutions>

<tests>
- **Both modes, every program:** run every test in `tests/` and every program in `compiler/gc_tests/` with `--working`
  and with `--final`, and check that both match the expected files. Also run them in collect-often mode. Build this
  before any optimization.
- **Each optimization:** a Rust unit test on Plainly IR showing it fires where it should and not where it shouldn't,
  including a case where it must not fire because the program's behaviour would change.
- **Speed:** small benchmark programs in `compiler/benchmarks/`, timed in both modes, with the numbers in
  PROGRESS.md. Include a recursive `fib`, so the fib trick (6) can be compared with gfortran's: the design session
  measured gfortran at about 3 ms for `fib(44)` at `-O3` against 2.62 s unoptimized.
- Every existing test keeps passing at every step, in both modes.
</tests>

<rules>
- Every optimization carries `// trace: D124` and its number from D124's list.
- No `unwrap`, `expect` or `panic` on paths a real program takes; `unsafe` only in the runtime and PlainGC, each with a
  comment saying why it's safe.
- Make only what each optimization needs. Don't add settings: D114 decided there are none for now.
- Keep `compiler/PROGRESS.md` up to date with what's built, the timings, and open questions.
</rules>

<first_steps>
1. Read the sources above, and your plan in PROGRESS.md.
2. Make `--final` run: the same path as `--working`, with Cranelift's optimizing switched on (1), so the both-modes
   test has something to compare.
3. Build the both-modes test, and see every program match.
4. Carry on through the groups in order.
</first_steps>
````
