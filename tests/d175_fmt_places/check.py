import sys
x = 0.1 + 0.2
want = [f"Speed: {x:.2f}", f"{x:.0f}", f"{0.5:.3f}", f"{0.125:.2f}", f"{x:.4f}!", "~" + repr(x)]
got = open("expected.out").read().splitlines()
sys.exit(0 if got == want else 1)
