// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
// Unit tests on Unio IR: each optimization where it applies, and where it mustn't, since applying it there would
// change what a program does (D46, D124).

#include "checker.h"
#include "ir.h"
#include "lexer.h"
#include "lower.h"
#include "optimize.h"
#include "parser.h"

#include <algorithm>
#include <array>
#include <cstdio>
#include <fstream>
#include <functional>
#include <print>
#include <span>
#include <sstream>
#include <string>
#include <string_view>
#include <vector>

namespace {

using unio::ir::Op;
using unio::ir::print;

int failures = 0; // NOLINT(cppcoreguidelines-avoid-non-const-global-variables): counted across every test

void expect(bool holds, const std::string& what, const std::string& context = "") {
    if (!holds) {
        ++failures;
        std::println(stderr, "FAIL: {}\n{}", what, context);
    }
}

// Reads a program through the front end into Unio IR, or gives no functions if it's refused.
unio::ir::Module compile(const std::string& text) {
    unio::Sources sources;
    sources.add("test.unio", text);
    unio::Messages messages;
    unio::Program program;
    const unio::Lexed lexed = unio::lex(sources, 0, messages);
    unio::parse(sources, 0, lexed, program, messages);
    unio::check(sources, program, messages);
    if (!messages.empty()) {
        std::print(stderr, "the test's program was refused:\n{}", messages.render(sources));
        ++failures;
        return {};
    }
    return unio::lower(sources, program);
}

const unio::ir::Function* find(const unio::ir::Module& module, const std::string& name) {
    for (const auto& function : module.functions) {
        if (function.name == name) {
            return &function;
        }
    }
    return nullptr;
}

std::size_t count(const unio::ir::Function& function, unio::ir::Op op) {
    std::size_t total = 0;
    for (const auto& block : function.blocks) {
        if (block.dead) {
            continue;
        }
        for (const auto id : block.instructions) {
            total += function.values.at(id).op == op ? 1U : 0U;
        }
    }
    return total;
}

void test_lowering_writes_out_checks() {
    const auto module = compile("proc add_one($n: Whole) returns Whole:\n"
                                "    return $n + 1;\n"
                                "end proc add_one\n"
                                "\n"
                                "proc.main:\n"
                                "    1 |> add_one |> be.Text |> show_msg;\n"
                                "end proc.main\n");
    const auto* add_one = find(module, "add_one");
    expect(add_one != nullptr, "add_one is lowered");
    if (add_one != nullptr) {
        expect(count(*add_one, Op::checked_add) == 1, "`+` is lowered as a checked add", print(*add_one, module));
    }
}

// D124 #1: constants are worked out at compile time, but a calculation that doesn't fit keeps its check, so the
// program still stops there (D46).
void test_folding() {
    auto module = compile("proc.main:\n"
                          "    let $small: Whole = 2 + 3;\n"
                          "    show_msg(be.Text($small));\n"
                          "    let $big: Whole = 9223372036854775807 + 1;\n"
                          "    show_msg(be.Text($big));\n"
                          "end proc.main\n");
    auto& main = module.functions.front();
    unio::passes::simplify(main);
    expect(count(main, Op::checked_add) == 1, "2 + 3 is folded, and the overflowing addition keeps its check",
           print(main, module));
}

// D124 #2, #8: a check goes where ranges and what an `if` proved show it can't fail, and stays where it can.
void test_removing_checks() {
    auto module = compile("proc down($n: Whole) returns Whole:\n"
                          "    if $n < 2:\n"
                          "        return $n;\n"
                          "    else:\n"
                          "        return $n - 1;\n"
                          "    end if\n"
                          "end proc down\n"
                          "\n"
                          "proc up($n: Whole) returns Whole:\n"
                          "    return $n + 1;\n"
                          "end proc up\n"
                          "\n"
                          "proc.main:\n"
                          "    var $i: Whole = 0;\n"
                          "    while $i < 3:\n"
                          "        $i = $i + 1;\n"
                          "    end while\n"
                          "    var $j: Whole = 0;\n"
                          "    while $j at_least 0:\n"
                          "        $j = $j + 1;\n"
                          "    end while\n"
                          "    $i |> up |> down |> be.Text |> show_msg;\n"
                          "end proc.main\n");
    for (auto& function : module.functions) {
        unio::passes::simplify(function);
        unio::passes::remove_checks(function);
    }
    const auto* down = find(module, "down");
    const auto* up = find(module, "up");
    const auto* main = find(module, "main");
    expect(count(*down, Op::checked_subtract) == 0 && count(*down, Op::checked_add) == 0,
           "inside `else` of `$n < 2`, `$n - 1` can't fail", print(*down, module));
    expect(count(*up, Op::checked_add) == 1, "`$n + 1` with any `$n` keeps its check", print(*up, module));
    expect(count(*main, Op::checked_add) == 1,
           "`$i + 1` under `$i < 3` loses its check, and `$j + 1` under `$j at_least 0` keeps it",
           print(*main, module));
}

// D124 #6: which procedures have no side effects, through the calls they make.
void test_finding_side_effects() {
    auto module = compile("proc twice($n: Whole) returns Whole:\n"
                          "    return $n * 2;\n"
                          "end proc twice\n"
                          "\n"
                          "proc noisy($n: Whole) returns Whole:\n"
                          "    show_msg(\"ran\");\n"
                          "    return $n;\n"
                          "end proc noisy\n"
                          "\n"
                          "proc calls_noisy($n: Whole) returns Whole:\n"
                          "    return noisy($n) + 1;\n"
                          "end proc calls_noisy\n"
                          "\n"
                          "proc.main:\n"
                          "    1 |> calls_noisy |> twice |> be.Text |> show_msg;\n"
                          "end proc.main\n");
    unio::passes::find_side_effects(module);
    expect(find(module, "twice")->no_side_effects, "`twice` has no side effects");
    expect(!find(module, "noisy")->no_side_effects, "`noisy` shows text, a side effect");
    expect(!find(module, "calls_noisy")->no_side_effects, "`calls_noisy` calls `noisy`, so it has one too");
}

// D124 #3: small procedures are inlined, and what they do still happens, once and in order; a recursive one isn't.
void test_inlining() {
    auto module = compile("proc greet($name: Text):\n"
                          "    show_msg(\"Hi, \" & $name);\n"
                          "end proc greet\n"
                          "\n"
                          "proc count_down($n: Whole) returns Whole:\n"
                          "    if $n < 1:\n"
                          "        return 0;\n"
                          "    else:\n"
                          "        return count_down($n - 1);\n"
                          "    end if\n"
                          "end proc count_down\n"
                          "\n"
                          "proc.main:\n"
                          "    greet(\"Mia\");\n"
                          "    3 |> count_down |> be.Text |> show_msg;\n"
                          "end proc.main\n");
    unio::passes::find_side_effects(module);
    unio::passes::inline_small(module);
    auto& main = *const_cast<unio::ir::Function*>(find(module, "main")); // NOLINT: the test owns the module
    unio::passes::simplify(main);
    std::size_t calls_to_greet = 0;
    std::size_t calls_to_count_down = 0;
    for (const auto& block : main.blocks) {
        if (block.dead) {
            continue;
        }
        for (const auto id : block.instructions) {
            const auto& value = main.values.at(id);
            if (value.op == Op::call) {
                (module.functions.at(value.callee).name == "greet" ? calls_to_greet : calls_to_count_down) += 1;
            }
        }
    }
    expect(calls_to_greet == 0, "`greet` is inlined into `proc.main`", print(main, module));
    expect(count(main, Op::show) == 2, "both messages are still shown", print(main, module));
    expect(calls_to_count_down == 1, "the recursive `count_down` isn't inlined", print(main, module));
}

std::size_t self_calls(const unio::ir::Function& function, const unio::ir::Module& module) {
    std::size_t total = 0;
    for (const auto& block : function.blocks) {
        if (block.dead) {
            continue;
        }
        for (const auto id : block.instructions) {
            const auto& value = function.values.at(id);
            total += value.op == Op::call && module.functions.at(value.callee).name == function.name ? 1U : 0U;
        }
    }
    return total;
}

std::size_t most_self_calls_in_a_block(const unio::ir::Function& function, const unio::ir::Module& module) {
    std::size_t most = 0;
    for (const auto& block : function.blocks) {
        if (block.dead) {
            continue;
        }
        std::size_t here = 0;
        for (const auto id : block.instructions) {
            const auto& value = function.values.at(id);
            here += value.op == Op::call && module.functions.at(value.callee).name == function.name ? 1U : 0U;
        }
        most = std::max(most, here);
    }
    return most;
}

// D124 #6: gfortran's trick, for any procedure with no side effects whose calls repeat once it's inlined into
// itself; not where nothing would repeat, and never for one with side effects, whose calls must all happen.
void test_inlining_into_itself() {
    // Inlined at `three($n - 1)`, the copy calls `three` with `$n - 2`, `$n - 3` and `$n - 4`, and the first two
    // repeat calls already there, so the deepest way makes 3 calls rather than 5.
    auto module = compile("proc three($n: Whole) returns Whole:\n"
                          "    if $n < 3:\n"
                          "        return 1;\n"
                          "    else:\n"
                          "        return three($n - 1) + three($n - 2) + three($n - 3);\n"
                          "    end if\n"
                          "end proc three\n"
                          "\n"
                          "proc.main:\n"
                          "    30 |> three |> be.Text |> show_msg;\n"
                          "end proc.main\n");
    unio::optimize(module);
    const auto* three = find(module, "three");
    expect(three->no_side_effects, "`three` has no side effects");
    expect(most_self_calls_in_a_block(*three, module) == 3, "the copy's repeated calls are merged",
           print(*three, module));

    // Inlined at either call, `steps($n - 1) + steps($n - 3)` repeats nothing, so it's left alone.
    auto plain = compile("proc steps($n: Whole) returns Whole:\n"
                         "    if $n < 3:\n"
                         "        return 1;\n"
                         "    else:\n"
                         "        return steps($n - 1) + steps($n - 3);\n"
                         "    end if\n"
                         "end proc steps\n"
                         "\n"
                         "proc.main:\n"
                         "    30 |> steps |> be.Text |> show_msg;\n"
                         "end proc.main\n");
    unio::optimize(plain);
    const auto* steps = find(plain, "steps");
    expect(self_calls(*steps, plain) == 2, "with nothing to merge, `steps` isn't inlined into itself",
           print(*steps, plain));

    auto noisy = compile("proc three($n: Whole) returns Whole:\n"
                         "    show_msg(\"step\");\n"
                         "    if $n < 3:\n"
                         "        return 1;\n"
                         "    else:\n"
                         "        return three($n - 1) + three($n - 2) + three($n - 3);\n"
                         "    end if\n"
                         "end proc three\n"
                         "\n"
                         "proc.main:\n"
                         "    30 |> three |> be.Text |> show_msg;\n"
                         "end proc.main\n");
    unio::optimize(noisy);
    const auto* noisy_three = find(noisy, "three");
    expect(self_calls(*noisy_three, noisy) == 3, "a procedure that shows text keeps all its calls",
           print(*noisy_three, noisy));
}

// D124 #6, #18: a repeated call with no side effects is merged where the first is sure to have run; a call with side
// effects, or one that isn't sure to have run, is kept.
void test_merging_repeated() {
    auto module = compile("proc square($n: Whole) returns Whole:\n"
                          "    return $n * $n;\n"
                          "end proc square\n"
                          "\n"
                          "proc shout($n: Whole) returns Whole:\n"
                          "    show_msg(\"!\");\n"
                          "    return $n;\n"
                          "end proc shout\n"
                          "\n"
                          "proc.main:\n"
                          "    let $a: Whole = be.Whole(ask(\"\")) ?? 0;\n"
                          "    let $b: Whole = square($a) + square($a);\n"
                          "    let $c: Whole = shout($a) + shout($a);\n"
                          "    if $a > 0:\n"
                          "        $a + 1 |> square |> be.Text |> show_msg;\n"
                          "    else:\n"
                          "        $a + 1 |> square |> be.Text |> show_msg;\n"
                          "    end if\n"
                          "    show_msg(be.Text($b + $c));\n"
                          "end proc.main\n");
    unio::passes::find_side_effects(module);
    auto& main = *const_cast<unio::ir::Function*>(find(module, "main")); // NOLINT: the test owns the module
    unio::passes::merge_repeated(main, module);
    std::size_t squares = 0;
    std::size_t shouts = 0;
    for (const auto& block : main.blocks) {
        if (block.dead) {
            continue;
        }
        for (const auto id : block.instructions) {
            const auto& value = main.values.at(id);
            if (value.op == Op::call) {
                (module.functions.at(value.callee).name == "square" ? squares : shouts) += 1;
            }
        }
    }
    expect(squares == 3, "`square($a) + square($a)` calls once; each way of the `if` keeps its own call",
           print(main, module));
    expect(shouts == 2, "`shout` shows text, so both calls stay", print(main, module));
}

// D124 #9: a parameter every call gives the same constant has it built in; one given different values doesn't.
void test_constants_across() {
    auto module = compile("proc scale($n: Whole, $by: Whole) returns Whole:\n"
                          "    return $n * $by;\n"
                          "end proc scale\n"
                          "\n"
                          "proc shift($n: Whole, $by: Whole) returns Whole:\n"
                          "    return $n + $by;\n"
                          "end proc shift\n"
                          "\n"
                          "proc.main:\n"
                          "    let $a: Whole = be.Whole(ask(\"\")) ?? 0;\n"
                          "    scale($a, 10) |> be.Text |> show_msg;\n"
                          "    scale($a + 1, 10) |> be.Text |> show_msg;\n"
                          "    shift($a, 1) |> be.Text |> show_msg;\n"
                          "    shift($a, 2) |> be.Text |> show_msg;\n"
                          "end proc.main\n");
    unio::passes::constants_across(module);
    auto& scale = *const_cast<unio::ir::Function*>(find(module, "scale")); // NOLINT: the test owns the module
    auto& shift = *const_cast<unio::ir::Function*>(find(module, "shift")); // NOLINT: the test owns the module
    unio::passes::simplify(scale);
    unio::passes::simplify(shift);
    const std::string scaled = print(scale, module);
    const std::string shifted = print(shift, module);
    expect(scaled.find("checked_multiply %0 10") != std::string::npos,
           "`$by`, always 10, is built in, and `$n`, given different values, stays a parameter", scaled);
    expect(shifted.find("checked_add %0 %1") != std::string::npos, "`shift`'s `$by`, given 1 and 2, stays a parameter",
           shifted);
}

// D124 #7: a procedure that calls itself as its last act becomes a loop; one that does more after the call doesn't.
void test_recursion_into_loops() {
    auto module = compile("proc total($n: Whole, $sum: Whole) returns Whole:\n"
                          "    if $n < 1:\n"
                          "        return $sum;\n"
                          "    else:\n"
                          "        return total($n - 1, $sum + $n);\n"
                          "    end if\n"
                          "end proc total\n"
                          "\n"
                          "proc depth($n: Whole) returns Whole:\n"
                          "    if $n < 1:\n"
                          "        return 0;\n"
                          "    else:\n"
                          "        return depth($n - 1) + 1;\n"
                          "    end if\n"
                          "end proc depth\n"
                          "\n"
                          "proc.main:\n"
                          "    show_msg(be.Text(total(10, 0) + depth(10)));\n"
                          "end proc.main\n");
    for (std::uint32_t index = 0; index < module.functions.size(); ++index) {
        unio::passes::recursion_into_loops(module.functions.at(index), index);
        unio::passes::simplify(module.functions.at(index));
    }
    const auto* total = find(module, "total");
    const auto* depth = find(module, "depth");
    expect(self_calls(*total, module) == 0, "`total`'s call to itself becomes a loop", print(*total, module));
    expect(self_calls(*depth, module) == 1, "`depth` adds 1 after its call, so the call stays", print(*depth, module));
}

// `unit_tests --print FILE [--final]` prints a program's Unio IR, for working on the compiler.
int print_file(const char* path, bool final) {
    std::ifstream file(path, std::ios::binary);
    if (!file) {
        std::println(stderr, "can't read {}", path);
        return 2;
    }
    std::ostringstream contents;
    contents << file.rdbuf();
    const std::string text = contents.str();
    auto module = compile(text);
    if (final) {
        unio::optimize(module);
    }
    std::print("{}", unio::ir::print(module));
    return failures > 0 ? 1 : 0;
}

} // namespace

int main(int argc, char** argv) {
    const std::span<char*> arguments(argv, static_cast<std::size_t>(argc));
    if (arguments.size() >= 3 && std::string_view(arguments[1]) == "--print") {
        return print_file(arguments[2], arguments.size() >= 4 && std::string_view(arguments[3]) == "--final");
    }
    const std::vector<std::function<void()>> tests = {
        test_lowering_writes_out_checks, test_folding,          test_removing_checks,
        test_finding_side_effects,       test_inlining,         test_inlining_into_itself,
        test_merging_repeated,           test_constants_across, test_recursion_into_loops,
    };
    for (const auto& test : tests) {
        test();
    }
    if (failures > 0) {
        std::println(stderr, "{} unit test checks failed", failures);
        return 1;
    }
    std::println("unit tests passed");
    return 0;
}
