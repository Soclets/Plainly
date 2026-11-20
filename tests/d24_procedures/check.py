def heal(health, by, times):
    return health + by * times
lines = [heal(50, 10, 2), max(3, 9), 5]
assert open("expected.out").read() == "".join(f"{n}\n" for n in lines)
