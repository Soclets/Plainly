import math, sys
x = 0.0 * -1.0
# the stored value really is negative zero, which Unio shows without its minus
ok = math.copysign(1.0, x) < 0 and open("expected.out").read().splitlines() == [repr(abs(x)), f"{x:z.2f}"]
sys.exit(0 if ok else 1)
