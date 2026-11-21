import sys
from decimal import Decimal

def shown(x):
    text = repr(x)
    return text if Decimal(text) == Decimal(x) else "~" + text

want = [shown(10.0 / 3.0), shown(1.0 / 4.0), "3.5"]
sys.exit(0 if open("expected.out").read().splitlines() == want else 1)
