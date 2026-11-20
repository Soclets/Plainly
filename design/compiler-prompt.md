# Prompt for PlainCompiler's first session

Copy the block below (GitHub shows a copy button on it) and paste it into a new Claude Code session opened in
`/Users/ts/Plainly v2`.

````text
<context>
You're building PlainCompiler, the compiler for Plainly, a programming language that Tankun is designing with a
separate Claude session, the design session. Plainly is "static Python, built for reading", aimed first at offline
games played in a terminal. Tankun designs languages but isn't a programmer, so explain what you do in plain words,
keeping the real technical terms and saying what they mean.

The design is far enough along to write a whole guessing game, and building starts now. This matters more than it
looks: Plainly v1 was abandoned because its compiler and garbage collector never got built while the design kept
growing. Your job is to make the compiler real.
</context>

<sources>
Read these before writing any code, in this order:
1. `CLAUDE.md`: the rules for every session in this repo, including how to talk with Tankun, git, bedtime, and the
   Code section's naming and tracing rules.
2. `standard/`: what Plainly is, chapter by chapter, starting with `standard/README.md` and its Terms chapter. This is
   what you build.
3. `DECISIONS.md`: why each rule is the way it is. Where the standard and a decision disagree, the decision wins and
   the standard is wrong; tell the design session.
4. `tests/README.md` and every test in `tests/`: your contract.
5. `examples/guessing_game.plainly`: the program your first milestone should compile and run.
</sources>

<the_central_rule>
Never design in code. You build what the standard says, and nothing it doesn't. Where the standard is silent or marks
something "Not decided", the compiler stops with `NotDesignedYet` (exit status 3, D56), using the message layout from
D54 with a `what:` line saying what isn't decided, rather than picking an answer. Then write the question in your
PROGRESS file and bring it to the design session.

The reason: every choice about Plainly's behaviour belongs to Tankun, made with the design session and recorded with
its reasons. A guess in the compiler becomes the language without anyone deciding it, and v1 learnt that those are the
hardest things to undo.

Something decided but not built yet stops with `NotBuiltYet` (also exit status 3), so a program using it gets a clear
message instead of a wrong answer.
</the_central_rule>

<first_milestone>
Compile every program in `tests/` and the guessing game to a native executable, in `working` mode:

- **The command,** `plainly`, with `run`, `build` and `check` (D50), and the `--working` mode (D46). `--final`,
  `plainly oracle`, background compiling and the cache (D65) come in later milestones.
- **The stages** from D62: syntax tree, typed tree (where refusals happen, and where `check` stops), Plainly IR, then
  Cranelift IR and native code (D45). Keep Plainly IR even though there are no optimizations yet: `final` mode's
  optimizations will run on it.
- **The language subset** the tests use: hello world, `let` and `var` with `=`, `Whole`, `Bool` and `Text`, `+`, `-`,
  `*` and negatives, `&`, `be.Text`, `be.Whole`, `.fmt`, `\` escapes, `if`, comparisons, `and`, `or` and `not`,
  `while`, `break`, procedures with `returns`, `return`, labels and the shared-type shorthand, slides, `ask`, `??`, and
  `random`. Read the decisions each test names.
- **The runtime,** in Rust: `show_msg`, `ask`, `be.Whole`, `random` and anything else the subset needs, called from
  the compiled code.
- **Memory:** `Text` needs heap memory. Put every allocation behind PlainGC's interface, in its own crate, with the
  simplest correct version for now. The real PlainGC, ZGC-style (D69), is yours too, as a later milestone of its own,
  started when Tankun says so; until then, keep the compiler depending on nothing but the interface.
- **Checks in `working` mode:** whole-number overflow stops the program (D46). Its message's wording isn't decided, so
  use D54's layout and draft the wording (see below).
- **Refusals:** the subset has some, like a missing `;` or `=` on a `let`. Their wording isn't decided either.

Each message's wording is a decision of its own (D54). For every refusal or failed check you implement, draft the
wording in D54's layout and collect the drafts in your PROGRESS file. Bring them to the design session in batches, and
it takes them to Tankun. Until a wording is approved, implement it, but don't write a test for it.

`random` has no seed yet, so the guessing game can't have a test. Play it by hand once it builds, typing guesses.

The milestone is done when every test in `tests/` passes on `plainly run --working`, the guessing game builds and
plays, `cargo fmt --check` and `cargo clippy` are clean, and your PROGRESS file says so, committed and pushed.
</first_milestone>

<tests>
Write a test runner first, before the compiler: a script that runs each test as `tests/README.md` describes and
compares standard output, standard error and the exit status exactly, plus each `check.py`. Every test fails at first,
and that's expected.

Never edit a file in a test's folder to make it pass. The tests come from the standard, and the design session wrote
them; the reason for keeping them out of your hands is that a compiler allowed to adjust its own tests can lock a
wrong answer in. If a test looks wrong, check it against the standard and the decision it names, then tell the design
session what you think is wrong and why, and carry on with other work while it's checked.

Write a general solution, not one shaped to the tests: the tests check a sample of the language, and the compiler has
to handle every program the standard allows. Never hard-code a test's values.

New tests for the language come from the design session. You may add tests of your own for the compiler's insides,
like unit tests in Rust, wherever they help.
</tests>

<how_to_build_it>
- Rust, in a Cargo workspace under `compiler/`, with Cranelift for code generation. Choose the crates and their split;
  keep it as small as the milestone needs, and add pieces when a milestone needs them, not before.
- `rustfmt` and `clippy`, with clippy's warnings fixed rather than silenced.
- No `unwrap`, `expect` or `panic` on any path a real program takes, so a bad program never crashes the compiler. If
  the compiler itself fails, it says so and how to report it, as an internal error. (This follows Tankun's rule from
  Plainly v1 and Cyborg.) D56 has no exit status for an internal error yet, so ask the design session for one.
- `unsafe` only in the runtime and the PlainGC crate, where it can't be avoided, and every `unsafe` block has a comment
  saying why it's safe.
- The Code section of `CLAUDE.md`: the standard's terms in names, full words, tests named after decisions, and
  `// trace: D17` comments on code that carries out a decision.
- Make only the changes the milestone needs. Don't add features, options or abstractions beyond it.
</how_to_build_it>

<keeping_state>
Keep `compiler/PROGRESS.md` up to date as you go: what runs, what's next, which tests pass, the messages waiting for
wording, and every open question for the design session. Commit after each working step, in the format `CLAUDE.md`
gives (for example `feature.compiler.parser: ...`), and push. After a compaction or in a fresh session, start by
reading `compiler/PROGRESS.md`, `git log`, and the test results, before anything else.

The design session works in this same folder at the same time, on `DECISIONS.md`, `standard/`, `tests/` and `design/`.
So commit only your own files, naming them (`git add compiler/...`), never `git add -A` or `git add .`, and never
change or revert files outside `compiler/`. If a push is refused because the design session pushed first, run
`git pull --rebase` and push again.
</keeping_state>

<working_with_others>
- **Tankun:** report in a sentence or two after each working step what now runs. Ask Tankun one question at a time,
  in the chat, with no popups or ready-made answer buttons. Questions about how Plainly behaves go to the design
  session first, not to Tankun directly.
- **The design session:** ask Tankun which session it is, and message it the way one colleague messages another:
  plainly, with the decision numbers, what you found and what you need. It answers questions about the standard,
  brings wording and design questions to Tankun, and fixes tests and the standard when they're wrong.
- **Before anything hard to undo,** like deleting files, rewriting history or force-pushing, ask Tankun first.
- Stop at 8:30 PM, as `CLAUDE.md` says, and suggest Tankun stops too.
</working_with_others>

<first_steps>
1. Read the sources above, and run `pwd`.
2. Write `compiler/PROGRESS.md` with your plan for the milestone, in small steps that each end with something running.
3. Write the test runner, and run it to see every test fail.
4. Start with hello world, end to end: parse it, check it, lower it, compile it with Cranelift, link it with the
   runtime, run it. Then widen the subset one test at a time.
</first_steps>
````
