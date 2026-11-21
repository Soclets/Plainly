import math, sys
from decimal import Decimal, ROUND_HALF_UP, ROUND_HALF_EVEN

def ways(x):
    near = int(Decimal(x).quantize(Decimal(1), rounding=ROUND_HALF_UP))
    even = int(Decimal(x).quantize(Decimal(1), rounding=ROUND_HALF_EVEN))
    return f"{math.floor(x)} {math.trunc(x)} {math.ceil(x)} {near} {even}"

want = [ways(x) for x in (2.7, 0.0 - 2.7, 2.5, 0.0 - 2.5, 3.5)]
sys.exit(0 if open("expected.out").read().splitlines() == want else 1)
