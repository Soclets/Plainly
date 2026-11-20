points = 20
assert open("expected.out").read() == f"{points * 2}\n{(points + 5) * 2}\n"
