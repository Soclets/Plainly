// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception

#include "driver.h"

#include "backend.h"
#include "checker.h"
#include "lexer.h"
#include "lower.h"
#include "messages.h"
#include "optimize.h"
#include "parser.h"
#include "spacing.h"

#include <llvm/Support/FileSystem.h>
#include <llvm/Support/Path.h>

#include <algorithm>
#include <cerrno>
#include <cstdio>
#include <cstdlib>
#include <optional>
#include <tuple>

#include <spawn.h>
#include <sys/wait.h>
#include <unistd.h>

extern char** environ; // NOLINT(cppcoreguidelines-avoid-non-const-global-variables): POSIX defines it

namespace unio {

namespace {

constexpr const char* usage = "usage:\n"
                              "  unio run game.unio --working\n"
                              "  unio compile game.unio --final\n"
                              "  unio check game.unio\n"
                              "  unio oracle game.unio\n";

void write_error(const std::string& text) {
    std::fwrite(text.data(), 1, text.size(), stderr);
    std::fflush(stderr);
}

// trace: D82, D103, D137
int internal_error(const std::string& what) {
    std::string text = "internal error: Unlo\\n itself went wrong, not your program\n";
    text += " what:    " + what + "\n";
    text += " why:     this is a bug in Unlo\\n. Please report it at https://github.com/Soclets/Unio-lang/issues, "
            "with the program that caused it.\n";
    write_error(text);
    return 7;
}

struct Command {
    std::string name;
    std::string file;
    std::optional<Mode> mode;
};

// trace: D46, D50, D91, D103, D116, D123
std::optional<Command> parse_command(std::span<char*> arguments, std::vector<std::string>& mistakes) {
    if (arguments.size() < 2) {
        mistakes.emplace_back("no command given");
        return std::nullopt;
    }
    Command command;
    command.name = arguments[1];
    if (command.name != "run" && command.name != "compile" && command.name != "check" && command.name != "oracle") {
        mistakes.push_back("`" + command.name + "` isn't a command");
        return std::nullopt;
    }
    const bool compiles = command.name == "run" || command.name == "compile";
    std::vector<std::string> files;
    bool working = false;
    bool final = false;
    bool unknown_mode = false;
    bool reported_files = false;
    std::size_t file_count = 0;
    for (std::size_t index = 2; index < arguments.size(); ++index) {
        if (!std::string(arguments[index]).starts_with("--")) {
            ++file_count;
        }
    }
    for (std::size_t index = 2; index < arguments.size(); ++index) {
        const std::string argument = arguments[index];
        if (argument == "--working" || argument == "--final") {
            if (!compiles) {
                mistakes.push_back("`" + command.name + "` takes no mode, since it doesn't compile");
                continue;
            }
            (argument == "--working" ? working : final) = true;
            if (working && final) {
                mistakes.push_back("`" + command.name +
                                   "` takes one mode, but this gives both `--working` and `--final`");
            }
            continue;
        }
        if (argument.starts_with("--")) {
            mistakes.push_back("`" + argument + "` isn't a mode; the modes are `--working` and `--final`");
            unknown_mode = true;
            continue;
        }
        files.push_back(argument);
        if (files.size() == 2 && !reported_files) {
            mistakes.push_back("`" + command.name + "` takes one file, but this gives " + std::to_string(file_count));
            reported_files = true;
        }
    }
    if (files.empty()) {
        mistakes.push_back("`" + command.name + "` needs a file");
    }
    if (compiles && !working && !final && !unknown_mode) {
        mistakes.emplace_back("`run` and `compile` need a mode: `--working` or `--final`");
    }
    if (!mistakes.empty()) {
        return std::nullopt;
    }
    command.file = files.front();
    if (compiles) {
        command.mode = final ? Mode::final : Mode::working;
    }
    return command;
}

// A program is every `.unio` file in its folder (D64): the one named first, then the rest in order.
std::vector<std::string> program_files(const std::string& named) {
    std::vector<std::string> files{named};
    const llvm::StringRef folder = llvm::sys::path::parent_path(named);
    std::error_code error;
    std::vector<std::string> siblings;
    for (llvm::sys::fs::directory_iterator entry(folder.empty() ? "." : folder, error), end; entry != end && !error;
         entry.increment(error)) {
        const std::string path = entry->path();
        const llvm::StringRef name = llvm::sys::path::filename(path);
        if (!name.ends_with(".unio") || name == llvm::sys::path::filename(named)) {
            continue;
        }
        llvm::SmallString<256> shown(folder);
        llvm::sys::path::append(shown, name);
        siblings.emplace_back(shown);
    }
    std::ranges::sort(siblings);
    files.insert(files.end(), siblings.begin(), siblings.end());
    return files;
}

// Reads, parses and checks every file, giving the typed tree and the messages in the order of their places.
std::pair<Program, Messages> check_program(const Sources& sources) {
    Messages messages;
    Program program;
    for (std::uint32_t file = 0; file < sources.size(); ++file) {
        const Lexed lexed = lex(sources, file, messages);
        parse(sources, file, lexed, program, messages);
        check_spacing(sources, file, lexed, messages);
    }
    check(sources, program, messages);
    std::ranges::stable_sort(messages.list(), [](const Message& left, const Message& right) {
        return std::pair(left.place.file, left.place.offset) < std::pair(right.place.file, right.place.offset);
    });
    return {std::move(program), std::move(messages)};
}

// trace: D55, D83
// The first error's one fix is tried in memory and the program checked again: every later message that disappears
// follows from it, and says so.
void sort_knock_on_errors(const Sources& sources, Messages& messages) {
    auto& list = messages.list();
    const auto first =
        std::ranges::find_if(list, [](const Message& message) { return message.kind == MessageKind::refused; });
    if (first == list.end()) {
        return;
    }
    const std::optional<std::string>& first_fix = first->fix;
    if (!first_fix) {
        return;
    }
    const LineColumn where = sources.line_column(first->place);
    Sources fixed = sources;
    fixed.replace_line(first->place.file, where.line, *first_fix);
    const auto [ignored, again] = check_program(fixed);
    const auto key = [](const Sources& in, const Message& message) {
        const LineColumn at = in.line_column(message.place);
        return std::tuple(message.place.file, at.line, at.column, message.kind, message.fix.value_or(""),
                          message.what.value_or(""));
    };
    const Place cause = first->place;
    const std::string sentence =
        "it follows from the error at line " + std::to_string(where.line) + " column " + std::to_string(where.column);
    for (auto later = std::next(first); later != list.end(); ++later) {
        const auto wanted = key(sources, *later);
        const bool still_there =
            std::ranges::any_of(again.list(), [&](const Message& other) { return key(fixed, other) == wanted; });
        if (!still_there) {
            later->because = Because{.sentence = sentence, .place = cause};
        }
    }
}

int run_program(const std::string& path) {
    const std::vector<std::string> owned{path};
    std::vector<char*> argv{const_cast<char*>(owned.front().c_str()), nullptr}; // NOLINT: posix_spawn's C signature
    pid_t child = 0;
    if (const int error = posix_spawn(&child, path.c_str(), nullptr, nullptr, argv.data(), environ); error != 0) {
        return internal_error("the compiled program couldn't start: " + std::string(std::strerror(error)));
    }
    int status = 0;
    while (waitpid(child, &status, 0) < 0 && errno == EINTR) {
    }
    if (WIFEXITED(status)) {
        return WEXITSTATUS(status);
    }
    return internal_error("the compiled program stopped with signal " + std::to_string(WTERMSIG(status)));
}

// trace: D50, D141
int run_oracle(const std::string& file) {
    llvm::SmallString<256> folder(executable_folder());
    while (!folder.empty()) {
        llvm::SmallString<256> candidate(folder);
        llvm::sys::path::append(candidate, "oracle", "_build", "default", "shell");
        llvm::sys::path::append(candidate, "ralph.exe");
        if (llvm::sys::fs::can_execute(candidate)) {
            const std::string path(candidate);
            std::vector<char*> argv{const_cast<char*>(path.c_str()),           // NOLINT: execv's C signature
                                    const_cast<char*>(file.c_str()), nullptr}; // NOLINT
            execv(path.c_str(), argv.data());
            return internal_error("the oracle couldn't start: " + std::string(std::strerror(errno)));
        }
        const llvm::StringRef parent = llvm::sys::path::parent_path(folder);
        if (parent == folder.str()) {
            break;
        }
        folder = parent;
    }
    write_error("unio: the oracle, Ralph, isn't built here; build it with "
                "`opam exec --switch=ralph -- dune build --root oracle`\n");
    return 6;
}

unsigned thread_count() {
    if (const char* threads = std::getenv("UNIO_THREADS")) {
        return static_cast<unsigned>(std::clamp(std::strtol(threads, nullptr, 10), 1L, 1024L));
    }
    return 0;
}

} // namespace

int drive(std::span<char*> arguments) {
    std::vector<std::string> mistakes;
    const auto command = parse_command(arguments, mistakes);
    if (!command) {
        std::string text;
        for (const std::string& mistake : mistakes) {
            text += "unio: " + mistake + "\n";
        }
        write_error(text + usage);
        return 6;
    }
    if (command->name == "oracle") {
        return run_oracle(command->file);
    }

    Sources sources;
    for (const std::string& path : program_files(command->file)) {
        const auto read = sources.read(path, path);
        if (!read) {
            // trace: D56, D92, D103
            write_error("can't read: " + path + "\n what:    " + read.error() + "\n");
            return 2;
        }
    }
    auto [program, messages] = check_program(sources);
    if (!messages.empty()) {
        sort_knock_on_errors(sources, messages);
        write_error(messages.render(sources));
        return messages.status();
    }
    if (command->name == "check") {
        return 0;
    }

    ir::Module module = lower(sources, program);
    const Mode mode = command->mode.value_or(Mode::working);
    if (mode == Mode::final) {
        optimize(module);
    }
    BuildSettings settings;
    settings.mode = mode;
    settings.threads = thread_count();
    if (command->name == "compile") {
        // trace: D150
        llvm::SmallString<256> output(command->file);
        llvm::sys::path::replace_extension(output, "");
        settings.output = std::string(output);
        if (const auto built = build(module, settings); !built) {
            return internal_error(built.error());
        }
        return 0;
    }
    const auto built = build(module, settings);
    if (!built) {
        return internal_error(built.error());
    }
    return run_program(*built);
}

} // namespace unio
