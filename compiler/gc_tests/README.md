# PlainGC's stress programs

Plainly programs that allocate heavily, so collections really happen while they run, and that check every value is
still right afterwards. `compiler/run_tests.py` runs each one twice, like every test in `tests/`: as it is, and with
`PLAINGC_STRESS=1`, which collects and moves every object at every allocation. Here it also asks for
`PLAINGC_REPORT=1` and checks that collections happened, objects moved, the load barrier healed references, and the marker thread marked while the program ran.

Each folder holds `program.plainly`, `expected.out`, and optionally `input.txt`, as in `tests/` (D12). These belong to
the compiler session; the language's own tests are in `tests/`, which belongs to the design session.
