# Prompt for PlainGC

> **Stale:** this prompt was for the first build, which ended on 2026-11-20. PlainCompiler and PlainGC are rewritten
> from a clean start on 2026-11-21 under CLAUDE.md's Building rules, with a new prompt (CLAUDE.md, "Working with
> Tankun"; Building rule 1 builds optimizations in from the start). Kept for reference.

Copy the block below and paste it into the compiler session (Plainly Compiler Claude), which builds PlainGC too.

````text
<context>
Your next milestone is PlainGC, Plainly's garbage collector, in the style of Java's ZGC (D69). Tankun has wanted to
see this built since Plainly v1, which was abandoned before its compiler and its ZGC ever became real. This is the
fun part of the project for Tankun, so build it properly, and report each step in plain words as it starts working.

Milestone 1 is done: the compiler puts every allocation behind PlainGC's interface, with the simplest version behind
it. This milestone replaces that with the real collector, without the compiler depending on anything but the
interface.
</context>

<sources>
Read these first: D69 (what PlainGC is), D48 (the Go-style design it replaced, for what was weighed), D36 and D45 (the
`extern.rust` procedures and Cranelift's stack maps it relies on), D46 (build modes), D66 to D68 (tasks and workers,
which come later and which the design has to leave room for), and standard chapter 10's Memory section. CLAUDE.md's
rules apply as before, including the Code section and the shared-folder rules in your first prompt.
</sources>

<what_d69_decides>
- One heap for every worker's thread, collected while they all keep running, with pauses that stay tiny at any heap
  size.
- It moves objects as it collects, compacting memory so it never fragments.
- Every read of a reference goes through a load barrier, emitted by PlainCompiler, so a moved object is always found.
- It's precise: it finds every reference through the stack maps Cranelift records, and never mistakes a number for a
  pointer.
- `extern.rust` code holds handles to Plainly values, never plain pointers, since objects can move.
</what_d69_decides>

<the_central_rule>
Build what D69 says, and bring anything it leaves open to the design session instead of choosing it yourself. Open
so far: how handles work on the Rust side, when a collection starts (the pacer), how much memory the heap may use,
and what happens when memory runs out, including its message and exit status. Until each is decided, use a clearly
labelled placeholder, written in your PROGRESS file, and never something a program could come to rely on.
</the_central_rule>

<steps>
Build it in steps that each end with something working and tested, and commit and push after each. Go straight on
to the next step after each one, without waiting for a reply: report it to Tankun in a sentence or two and keep
building. Stop only at step 5, which waits on the design session, at bedtime, or when Tankun says so.
1. Allocation, with objects laid out so the collector can find every reference inside them, and a collection that
   runs with the program paused (stop-the-world mark and compact). Roots come from Cranelift's stack maps.
2. Moving objects, with the load barrier emitted by PlainCompiler on every reference read, and forwarding so a moved
   object is found from an old reference.
3. Marking while the program keeps running.
4. Moving while the program keeps running.
5. Handles for `extern.rust` code, once the design session has the answer for how they work.

Workers (D68) aren't built yet, so the program has one thread for now. Design every step so more threads can join
later without redoing it: D69's whole point is many threads sharing one heap.
</steps>

<tests>
- Unit tests in Rust for each piece: allocation, finding roots, marking, moving, forwarding, the barrier.
- Stress programs in Plainly that allocate heavily, so collections really happen while they run, checking that every
  value is still right afterwards. Write them in the compiler's own test folder, not `tests/`, since `tests/` belongs
  to the design session.
- A mode or setting that collects as often as possible, so the tests reach every collector path quickly. Keep it out
  of anything a program can see.
- Every existing test in `tests/` keeps passing at every step.
</tests>

<rules>
- `unsafe` is allowed in PlainGC, where it can't be avoided, and every `unsafe` block has a comment saying why it's
  safe.
- No `unwrap`, `expect` or `panic` on paths a real program takes. A bug in PlainGC itself ends with exit status 7
  (D82) and a message saying how to report it.
- Make only what each step needs. Don't build generations, thread support or tuning before a step calls for them.
- Keep `compiler/PROGRESS.md` up to date with PlainGC's steps, what works, and its open questions.
</rules>

<first_steps>
1. Read the sources above.
2. Add PlainGC's steps to `compiler/PROGRESS.md`, with your plan for step 1 in detail.
3. Write step 1's unit tests, then step 1.
4. Carry on through steps 2, 3 and 4 the same way, one after another.
</first_steps>
````
