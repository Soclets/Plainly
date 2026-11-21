import sys
from decimal import Decimal, ROUND_HALF_UP

def places(x, n):
    # ROUND_HALF_UP in Python's decimal sends a tie away from zero, as D178 says
    return str(Decimal(x).quantize(Decimal(1).scaleb(-n), rounding=ROUND_HALF_UP))

x = 0.1 + 0.2
want = ["Speed: " + places(x, 2), places(x, 0), places(0.5, 3), places(0.125, 2), places(x, 4) + "!", "~" + repr(x)]
sys.exit(0 if open("expected.out").read().splitlines() == want else 1)
