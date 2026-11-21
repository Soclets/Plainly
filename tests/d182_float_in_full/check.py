import sys
from decimal import Decimal

def shown(x):
    short = Decimal(repr(x))
    text = format(short, "f")
    if "." not in text:
        text += ".0"
    return text if short == Decimal(x) else "~" + text

want = [shown(1e22), shown(2.0 ** 70), shown(0.00001)]
sys.exit(0 if open("expected.out").read().splitlines() == want else 1)
