import sys
s = 0.1 + 0.2
lines = ["same" if s == 0.3 else "different"]
if s > 0.3:
    lines.append("above")
if 0.5 + 0.25 == 0.75:
    lines.append("exact")
if 0.0 * -1.0 == 0.0:
    lines.append("zero")
t, n = 0.0, 0
while t < 1.0:
    t, n = t + 0.1, n + 1
lines.append(f"{n} steps, ~{t!r}")
sys.exit(0 if open("expected.out").read().splitlines() == lines else 1)
