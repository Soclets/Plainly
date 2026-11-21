import math, sys
want = [repr(math.sqrt(9.0)), "~" + repr(math.sqrt(2.0)), "Distance: " + repr(math.sqrt(3.0 * 3.0 + 4.0 * 4.0))]
sys.exit(0 if open("expected.out").read().splitlines() == want else 1)
