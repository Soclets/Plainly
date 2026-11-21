def sum_to(n):
    return 0 if n < 1 else n + sum_to(n - 1)
assert open("expected.out").read() == f"{sum_to(10)}\n{sum_to(0)}\n"
