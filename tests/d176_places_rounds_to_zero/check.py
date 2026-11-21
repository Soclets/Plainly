import sys
x = 0.25 - 0.251953125
want = [f"{x:z.2f}", f"{x:z.3f}"]
sys.exit(0 if open("expected.out").read().splitlines() == want else 1)
