def is_even(n): return True if n == 0 else is_odd(n - 1)
def is_odd(n): return False if n == 0 else is_even(n - 1)
w = lambda n: f"{n} is {'even' if is_even(n) else 'odd'}"
assert open("expected.out").read() == f"{w(10)}\n{w(7)}\n"
