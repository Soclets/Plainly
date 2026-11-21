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

#include <array>
#include <cstdio>
#include <functional>
#include <span>
#include <string>
#include <string_view>
#include <vector>

namespace {

int failures = 0;

void expect(bool holds, const std::string& what, const std::string& context = "") {
    if (!holds) {
        ++failures;
        std::fprintf(stderr, "FAIL: %s\n%s\n", what.c_str(), context.c_str());
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
        std::fprintf(stderr, "the test's program was refused:\n%s", messages.render(sources).c_str());
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
        expect(count(*add_one, unio::ir::Op::checked_add) == 1, "`+` is lowered as a checked add",
               unio::ir::print(*add_one, module));
    }
}

// `unit_tests --print FILE [--final]` prints a program's Unio IR, for working on the compiler.
int print_file(const char* path, bool final) {
    std::FILE* file = std::fopen(path, "rb");
    if (file == nullptr) {
        std::fprintf(stderr, "can't read %s\n", path);
        return 2;
    }
    std::string text;
    std::array<char, 4096> buffer{};
    for (std::size_t count = 0; (count = std::fread(buffer.data(), 1, buffer.size(), file)) > 0;) {
        text.append(buffer.data(), count);
    }
    std::fclose(file);
    auto module = compile(text);
    if (final) {
        unio::optimize(module);
    }
    std::printf("%s", unio::ir::print(module).c_str());
    return failures > 0 ? 1 : 0;
}

} // namespace

int main(int argc, char** argv) {
    const std::span<char*> arguments(argv, static_cast<std::size_t>(argc));
    if (arguments.size() >= 3 && std::string_view(arguments[1]) == "--print") {
        return print_file(arguments[2], arguments.size() >= 4 && std::string_view(arguments[3]) == "--final");
    }
    const std::vector<std::function<void()>> tests = {
        test_lowering_writes_out_checks,
    };
    for (const auto& test : tests) {
        test();
    }
    if (failures > 0) {
        std::fprintf(stderr, "%d unit test checks failed\n", failures);
        return 1;
    }
    std::printf("unit tests passed\n");
    return 0;
}
