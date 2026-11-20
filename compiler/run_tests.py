#!/usr/bin/env python3
"""Runs Plainly's tests (tests/README.md) against PlainCompiler.

Builds the compiler's workspace, then runs each test as

    plainly run tests/NAME/program.plainly --working < tests/NAME/input.txt

and compares standard output, standard error and the exit status exactly with the
expected files. A test with a check.py also runs `python3 -I check.py` in its folder,
which must exit with status 0.

Usage: python3 compiler/run_tests.py [NAME ...]
"""

import os
import subprocess
import sys

COMPILER_FOLDER = os.path.dirname(os.path.abspath(__file__))
REPOSITORY = os.path.dirname(COMPILER_FOLDER)
TESTS_FOLDER = os.path.join(REPOSITORY, "tests")
PLAINLY = os.path.join(COMPILER_FOLDER, "target", "debug", "plainly")
TIME_LIMIT_SECONDS = 60


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
    print(f"      expected: {expected!r}")
    print(f"      actual:   {actual!r}")


def run_test(name):
    """Returns a list of problems; an empty list means the test passed."""
    folder = os.path.join(TESTS_FOLDER, name)
    program = os.path.join("tests", name, "program.plainly")
    input_path = os.path.join(folder, "input.txt")
    standard_input = read_bytes(input_path) if os.path.exists(input_path) else b""
    problems = []
    try:
        result = subprocess.run(
            [PLAINLY, "run", program, "--working"],
            cwd=REPOSITORY,
            input=standard_input,
            capture_output=True,
            timeout=TIME_LIMIT_SECONDS,
        )
    except subprocess.TimeoutExpired:
        return [("time", f"under {TIME_LIMIT_SECONDS} seconds", "still running")]

    expected_output = read_bytes(os.path.join(folder, "expected.out"))
    if result.stdout != expected_output:
        problems.append(("standard output", expected_output, result.stdout))

    error_path = os.path.join(folder, "expected.err")
    expected_error = read_bytes(error_path) if os.path.exists(error_path) else b""
    if result.stderr != expected_error:
        problems.append(("standard error", expected_error, result.stderr))

    expected_status = int(read_bytes(os.path.join(folder, "expected.status")).decode().strip())
    if result.returncode != expected_status:
        problems.append(("exit status", expected_status, result.returncode))

    if os.path.exists(os.path.join(folder, "check.py")):
        check = subprocess.run(["python3", "-I", "check.py"], cwd=folder, capture_output=True)
        if check.returncode != 0:
            problems.append(("check.py status", 0, check.returncode))
    return problems


def main():
    names = sys.argv[1:] or sorted(
        entry for entry in os.listdir(TESTS_FOLDER) if os.path.isdir(os.path.join(TESTS_FOLDER, entry))
    )
    build_compiler()
    passed = 0
    for name in names:
        problems = run_test(name)
        if problems:
            print(f"FAIL {name}")
            for what, expected, actual in problems:
                show_difference(what, expected, actual)
        else:
            print(f"pass {name}")
            passed += 1
    print(f"\n{passed} of {len(names)} tests pass.")
    sys.exit(0 if passed == len(names) else 1)


if __name__ == "__main__":
    main()
