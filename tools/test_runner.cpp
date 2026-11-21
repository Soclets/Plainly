// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
// Runs every test in tests/ as tests/README.md describes, in both build modes,
// and compares standard output, standard error and exit status byte for byte
// with the expected files, then runs each check.py (D12). With
// --oracle it also runs Ralph on each test, as a black box, with the same
// expected files (D50, Building rule 12).

#include <algorithm>
#include <array>
#include <atomic>
#include <cerrno>
#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <expected>
#include <filesystem>
#include <fstream>
#include <mutex>
#include <optional>
#include <print>
#include <span>
#include <sstream>
#include <string>
#include <string_view>
#include <thread>
#include <vector>

#include <csignal>
#include <fcntl.h>
#include <poll.h>
#include <spawn.h>
#include <sys/wait.h>
#include <unistd.h>

extern char** environ; // NOLINT(cppcoreguidelines-avoid-non-const-global-variables): POSIX defines it

namespace {

namespace fs = std::filesystem;

struct Outcome {
    std::string out;
    std::string err;
    int status = -1;
    bool timed_out = false;
};

struct Options {
    std::string unio;
    std::string oracle;
    std::string tests;
    std::string status_file;
    std::string filter;
    unsigned jobs = 3;
    std::vector<std::string> modes{"working", "final"};
};

std::optional<std::string> read_file(const fs::path& path) {
    std::ifstream file(path, std::ios::binary);
    if (!file) {
        return std::nullopt;
    }
    std::ostringstream text;
    text << file.rdbuf();
    return text.str();
}

void close_quietly(int descriptor) {
    if (descriptor >= 0) {
        close(descriptor);
    }
}

// Runs a program in a folder, with standard input from a file (or nothing), and
// collects what it writes. A test that runs past the time limit is stopped, so
// one hung program can't stop the whole run.
std::expected<Outcome, std::string> run(const std::vector<std::string>& arguments, const fs::path& folder,
                                        const std::optional<fs::path>& input) {
    std::array<int, 2> out_pipe{-1, -1};
    std::array<int, 2> err_pipe{-1, -1};
    if (pipe(out_pipe.data()) != 0 || pipe(err_pipe.data()) != 0) {
        return std::unexpected(std::string("pipe: ") + std::strerror(errno));
    }
    posix_spawn_file_actions_t actions{};
    posix_spawn_file_actions_init(&actions);
    const std::string input_path = input ? input->string() : std::string("/dev/null");
    posix_spawn_file_actions_addopen(&actions, 0, input_path.c_str(), O_RDONLY, 0);
    posix_spawn_file_actions_adddup2(&actions, out_pipe[1], 1);
    posix_spawn_file_actions_adddup2(&actions, err_pipe[1], 2);
    posix_spawn_file_actions_addclose(&actions, out_pipe[0]);
    posix_spawn_file_actions_addclose(&actions, err_pipe[0]);
    const std::string folder_text = folder.string();
    posix_spawn_file_actions_addchdir(&actions, folder_text.c_str());

    std::vector<std::string> owned = arguments;
    std::vector<char*> argv;
    argv.reserve(owned.size() + 1);
    for (std::string& argument : owned) {
        argv.push_back(argument.data());
    }
    argv.push_back(nullptr);

    pid_t child = 0;
    const int spawned = posix_spawn(&child, argv[0], &actions, nullptr, argv.data(), environ);
    posix_spawn_file_actions_destroy(&actions);
    close(out_pipe[1]);
    close(err_pipe[1]);
    if (spawned != 0) {
        close_quietly(out_pipe[0]);
        close_quietly(err_pipe[0]);
        return std::unexpected("can't start " + arguments[0] + ": " + std::strerror(spawned));
    }

    Outcome outcome;
    const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(60);
    std::array<pollfd, 2> watched{pollfd{.fd = out_pipe[0], .events = POLLIN, .revents = 0},
                                  pollfd{.fd = err_pipe[0], .events = POLLIN, .revents = 0}};
    std::array<char, 65536> buffer{};
    int open_count = 2;
    while (open_count > 0) {
        const auto left =
            std::chrono::duration_cast<std::chrono::milliseconds>(deadline - std::chrono::steady_clock::now());
        if (left.count() <= 0) {
            kill(child, SIGKILL);
            outcome.timed_out = true;
            break;
        }
        const int ready = poll(watched.data(), watched.size(), static_cast<int>(left.count()));
        if (ready < 0 && errno != EINTR) {
            break;
        }
        for (std::size_t index = 0; index < watched.size(); ++index) {
            pollfd& entry = watched.at(index);
            if (entry.fd < 0 ||
                (static_cast<unsigned>(entry.revents) & static_cast<unsigned>(POLLIN | POLLHUP | POLLERR)) == 0U) {
                continue;
            }
            const ssize_t count = read(entry.fd, buffer.data(), buffer.size());
            if (count <= 0) {
                close(entry.fd);
                entry.fd = -1;
                --open_count;
                continue;
            }
            std::string& target = index == 0 ? outcome.out : outcome.err;
            target.append(buffer.data(), static_cast<std::size_t>(count));
        }
    }
    for (pollfd& entry : watched) {
        close_quietly(entry.fd);
    }
    int wait_status = 0;
    while (waitpid(child, &wait_status, 0) < 0 && errno == EINTR) {
    }
    if (WIFEXITED(wait_status)) {
        outcome.status = WEXITSTATUS(wait_status);
    } else if (WIFSIGNALED(wait_status)) {
        outcome.status = 128 + WTERMSIG(wait_status);
    }
    return outcome;
}

// The first line where two texts differ, shown for a failing test.
std::string first_difference(std::string_view expected, std::string_view actual) {
    std::size_t line = 1;
    std::size_t start = 0;
    while (true) {
        const std::size_t expected_end = std::min(expected.find('\n', start), expected.size());
        const std::size_t actual_end = std::min(actual.find('\n', start), actual.size());
        const std::string_view expected_line =
            start <= expected.size() ? expected.substr(start, expected_end - start) : std::string_view{};
        const std::string_view actual_line =
            start <= actual.size() ? actual.substr(start, actual_end - start) : std::string_view{};
        if (expected_line != actual_line || expected_end != actual_end) {
            std::string text = "line " + std::to_string(line) + ": expected `" + std::string(expected_line) +
                               "`, got `" + std::string(actual_line) + "`";
            if (expected_end == expected.size() && actual_end == actual.size() && expected_line == actual_line) {
                text += " (they differ in the final line break)";
            }
            return text;
        }
        if (expected_end >= expected.size() || actual_end >= actual.size()) {
            return "they differ in length after line " + std::to_string(line);
        }
        start = expected_end + 1;
        ++line;
    }
}

struct Result {
    std::string test;
    std::string mode;
    bool passed = false;
    std::string reason;
};

std::vector<Result> run_test(const Options& options, const fs::path& folder) {
    std::vector<Result> results;
    const std::string name = folder.filename().string();
    const auto expected_out = read_file(folder / "expected.out");
    const auto expected_err = read_file(folder / "expected.err");
    const auto expected_status_text = read_file(folder / "expected.status");
    if (!expected_out || !expected_status_text) {
        results.push_back(
            {.test = name, .mode = "-", .passed = false, .reason = "the test has no expected.out or expected.status"});
        return results;
    }
    const auto expected_status = static_cast<int>(std::strtol(expected_status_text->c_str(), nullptr, 10));
    std::optional<fs::path> input;
    if (fs::exists(folder / "input.txt")) {
        input = folder / "input.txt";
    }

    std::vector<std::pair<std::string, std::vector<std::string>>> runs;
    runs.reserve(options.modes.size() + 1);
    for (const std::string& mode : options.modes) {
        runs.push_back({mode, {options.unio, "run", "program.unio", "--" + mode}});
    }
    if (!options.oracle.empty()) {
        runs.push_back({"oracle", {options.oracle, "program.unio"}});
    }
    for (const auto& [mode, arguments] : runs) {
        Result result{.test = name, .mode = mode, .passed = false, .reason = ""};
        const auto outcome = run(arguments, folder, input);
        if (!outcome) {
            result.reason = outcome.error();
        } else if (outcome->timed_out) {
            result.reason = "it ran past the time limit";
        } else if (outcome->status != expected_status) {
            result.reason =
                "exit status " + std::to_string(outcome->status) + ", expected " + std::to_string(expected_status);
            if (!outcome->err.empty()) {
                const std::string first_line = outcome->err.substr(0, outcome->err.find('\n'));
                result.reason += "; stderr starts `" + first_line + "`";
            }
        } else if (outcome->out != *expected_out) {
            result.reason = "standard output: " + first_difference(*expected_out, outcome->out);
        } else if (outcome->err != expected_err.value_or("")) {
            result.reason = "standard error: " + first_difference(expected_err.value_or(""), outcome->err);
        } else {
            result.passed = true;
        }
        results.push_back(result);
    }
    if (fs::exists(folder / "check.py")) {
        Result result{.test = name, .mode = "check.py", .passed = false, .reason = ""};
        const auto outcome = run({"/usr/bin/env", "python3", "-I", "check.py"}, folder, std::nullopt);
        if (!outcome) {
            result.reason = outcome.error();
        } else if (outcome->status != 0) {
            result.reason = "check.py exited with " + std::to_string(outcome->status) + ": " + outcome->err;
        } else {
            result.passed = true;
        }
        results.push_back(result);
    }
    return results;
}

std::expected<Options, std::string> parse_options(std::span<char*> arguments) {
    Options options;
    for (std::size_t index = 1; index < arguments.size(); ++index) {
        const std::string_view argument = arguments[index];
        const bool has_value = index + 1 < arguments.size();
        if (argument == "--unio" && has_value) {
            options.unio = arguments[++index];
        } else if (argument == "--oracle" && has_value) {
            options.oracle = arguments[++index];
        } else if (argument == "--tests" && has_value) {
            options.tests = arguments[++index];
        } else if (argument == "--status" && has_value) {
            options.status_file = arguments[++index];
        } else if (argument == "--filter" && has_value) {
            options.filter = arguments[++index];
        } else if (argument == "--mode" && has_value) {
            options.modes = {arguments[++index]};
        } else if (argument == "--jobs" && has_value) {
            options.jobs = static_cast<unsigned>(std::clamp(std::strtol(arguments[++index], nullptr, 10), 1L, 64L));
        } else {
            return std::unexpected("test_runner: unknown argument `" + std::string(argument) + "`");
        }
    }
    if (options.unio.empty() || options.tests.empty()) {
        return std::unexpected(std::string("usage: test_runner --unio PATH --tests DIR [--status FILE] "
                                           "[--oracle PATH] [--filter TEXT] [--mode working|final] [--jobs N]"));
    }
    return options;
}

} // namespace

int main(int argc, char** argv) {
    const auto options = parse_options(std::span<char*>(argv, static_cast<std::size_t>(argc)));
    if (!options) {
        std::println(stderr, "{}", options.error());
        return 2;
    }
    std::vector<fs::path> folders;
    std::error_code error;
    for (const auto& entry : fs::directory_iterator(options->tests, error)) {
        if (entry.is_directory() && fs::exists(entry.path() / "program.unio") &&
            entry.path().filename().string().find(options->filter) != std::string::npos) {
            folders.push_back(fs::absolute(entry.path()));
        }
    }
    if (error) {
        std::println(stderr, "test_runner: can't read {}: {}", options->tests, error.message());
        return 2;
    }
    std::ranges::sort(folders);

    std::vector<std::vector<Result>> results(folders.size());
    std::atomic<std::size_t> next{0};
    std::vector<std::jthread> workers;
    workers.reserve(options->jobs);
    for (unsigned worker = 0; worker < options->jobs; ++worker) {
        workers.emplace_back([&] {
            for (std::size_t index = next++; index < folders.size(); index = next++) {
                results.at(index) = run_test(*options, folders.at(index));
            }
        });
    }
    workers.clear();

    std::size_t passed = 0;
    std::size_t total = 0;
    std::ostringstream status;
    for (const auto& test_results : results) {
        for (const Result& result : test_results) {
            ++total;
            passed += result.passed ? 1U : 0U;
            status << (result.passed ? "pass " : "FAIL ") << result.test << " " << result.mode;
            if (!result.passed) {
                status << ": " << result.reason;
            }
            status << "\n";
        }
    }
    status << passed << " of " << total << " passed\n";
    std::fputs(status.str().c_str(), stdout);
    if (!options->status_file.empty()) {
        std::ofstream file(options->status_file, std::ios::binary);
        file << status.str();
    }
    return passed == total ? 0 : 1;
}
