#!/usr/bin/env python3
"""Runs Plainly's tests (tests/README.md) and PlainGC's stress programs (compiler/gc_tests/README.md).

Builds the compiler's workspace, then runs each test from inside its own folder as

    cd tests/NAME
    plainly run program.plainly --working < input.txt

and compares standard output, standard error and the exit status exactly with the
expected files. A test with a check.py also runs `python3 -I check.py` in its folder,
which must exit with status 0.

Every test runs twice: as it is, and with PLAINGC_STRESS=1, which makes PlainGC collect
and move every object at every allocation. A stress program in compiler/gc_tests also
asks for PLAINGC_REPORT=1 there, and must show that collections happened, objects moved,
the load barrier healed references, and the marker thread marked while the program ran.

Usage: python3 compiler/run_tests.py [NAME ...]
"""

import os
import re
import subprocess
import sys

COMPILER_FOLDER = os.path.dirname(os.path.abspath(__file__))
REPOSITORY = os.path.dirname(COMPILER_FOLDER)
TESTS_FOLDER = os.path.join(REPOSITORY, "tests")
GC_TESTS_FOLDER = os.path.join(COMPILER_FOLDER, "gc_tests")
PLAINLY = os.path.join(COMPILER_FOLDER, "target", "debug", "plainly")
TIME_LIMIT_SECONDS = 120
REPORT = re.compile(
    rb"PlainGC: (\d+) collections, (\d+) objects moved, (\d+) references healed, "
    rb"(\d+) marking batches on its own thread\n$"
)


def read_bytes(path):
    with open(path, "rb") as file:
        return file.read()


def build_compiler():
    result = subprocess.run(["cargo", "build", "--workspace"], cwd=COMPILER_FOLDER)
    if result.returncode != 0:
        print("The compiler didn't build, so no test can run.")
        sys.exit(1)


def show_difference(what, expected, actual):
    print(f"    {what} differs:")
    print(f"      expected: {expected[:300]!r}")
    print(f"      actual:   {actual[:300]!r}")


def run_test(folder, stress, gc_test):
    """Returns a list of problems; an empty list means the test passed."""
    input_path = os.path.join(folder, "input.txt")
    standard_input = read_bytes(input_path) if os.path.exists(input_path) else b""
    environment = dict(os.environ)
    environment.pop("PLAINGC_STRESS", None)
    environment.pop("PLAINGC_REPORT", None)
    if stress:
        environment["PLAINGC_STRESS"] = "1"
        if gc_test:
            environment["PLAINGC_REPORT"] = "1"
    problems = []
    try:
        result = subprocess.run(
            [PLAINLY, "run", "program.plainly", "--working"],
            cwd=folder,
            input=standard_input,
            capture_output=True,
            timeout=TIME_LIMIT_SECONDS,
            env=environment,
        )
    except subprocess.TimeoutExpired:
        return [("time", f"under {TIME_LIMIT_SECONDS} seconds", "still running")]

    standard_error = result.stderr
    if stress and gc_test:
        report = REPORT.search(standard_error)
        if report is None:
            problems.append(("PlainGC's report", "a report", standard_error))
        else:
            standard_error = standard_error[: report.start()]
            counts = [int(report.group(number)) for number in (1, 2, 3, 4)]
            if 0 in counts:
                problems.append(
                    (
                        "PlainGC's work",
                        "collections, moved objects, healed references and marking on its own thread",
                        report.group(0),
                    )
                )

    expected_output = read_bytes(os.path.join(folder, "expected.out"))
    if result.stdout != expected_output:
        problems.append(("standard output", expected_output, result.stdout))

    error_path = os.path.join(folder, "expected.err")
    expected_error = read_bytes(error_path) if os.path.exists(error_path) else b""
    if standard_error != expected_error:
        problems.append(("standard error", expected_error, standard_error))

    status_path = os.path.join(folder, "expected.status")
    expected_status = int(read_bytes(status_path).decode().strip()) if os.path.exists(status_path) else 0
    if result.returncode != expected_status:
        problems.append(("exit status", expected_status, result.returncode))

    if not stress and os.path.exists(os.path.join(folder, "check.py")):
        check = subprocess.run(["python3", "-I", "check.py"], cwd=folder, capture_output=True)
        if check.returncode != 0:
            problems.append(("check.py status", 0, check.returncode))
    return problems


def test_folders(names):
    folders = []
    for base, gc_test in [(TESTS_FOLDER, False), (GC_TESTS_FOLDER, True)]:
        for entry in sorted(os.listdir(base)):
            folder = os.path.join(base, entry)
            if os.path.isdir(folder) and (not names or entry in names):
                folders.append((entry, folder, gc_test))
    return folders


def main():
    names = sys.argv[1:]
    build_compiler()
    runs = 0
    passed = 0
    for name, folder, gc_test in test_folders(names):
        for stress in [False, True]:
            label = f"{name} (collect-often)" if stress else name
            runs += 1
            problems = run_test(folder, stress, gc_test)
            if problems:
                print(f"FAIL {label}")
                for what, expected, actual in problems:
                    show_difference(what, expected, actual)
            else:
                print(f"pass {label}")
                passed += 1
    print(f"\n{passed} of {runs} runs pass.")
    sys.exit(0 if passed == runs else 1)


if __name__ == "__main__":
    main()
