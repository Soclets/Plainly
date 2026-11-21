import sys
want = [repr(6.0 + 1.0), repr(0.0 - 3.0), repr(2.5), f"{6.0 + 1.0:.0f}"]
sys.exit(0 if open("expected.out").read().splitlines() == want else 1)
