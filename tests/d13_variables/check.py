price = 12
count = 3
total = price * count
total = total - 6
total = total + -4
assert open("expected.out").read() == f"{total}\n"
