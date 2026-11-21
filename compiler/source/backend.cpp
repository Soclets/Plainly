// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception

#include "backend.h"

#include "codegen.h"

#include <llvm/IR/LLVMContext.h>
#include <llvm/IR/LegacyPassManager.h>
#include <llvm/IR/Module.h>
#include <llvm/IR/Verifier.h>
#include <llvm/MC/TargetRegistry.h>
#include <llvm/Passes/PassBuilder.h>
#include <llvm/Support/FileSystem.h>
#include <llvm/Support/Path.h>
#include <llvm/Support/TargetSelect.h>
#include <llvm/Support/raw_ostream.h>
#include <llvm/Support/xxhash.h>
#include <llvm/Target/TargetMachine.h>
#include <llvm/TargetParser/Host.h>
#include <llvm/Transforms/IPO/HotColdSplitting.h>

#include <array>
#include <atomic>
#include <cerrno>
#include <cstdlib>
#include <cstring>
#include <mutex>
#include <thread>

#include <mach-o/dyld.h>
#include <spawn.h>
#include <sys/stat.h>
#include <sys/wait.h>
#include <unistd.h>

extern char** environ; // NOLINT(cppcoreguidelines-avoid-non-const-global-variables): POSIX defines it

namespace unio {

namespace {

// NOLINTNEXTLINE(cppcoreguidelines-avoid-non-const-global-variables): LLVM sets up its targets once per process
std::once_flag targets_ready;

// The oldest macOS a compiled program runs on.
constexpr const char* minimum_macos = "14.0";

void initialize_targets() {
    std::call_once(targets_ready, [] {
        llvm::InitializeNativeTarget();
        llvm::InitializeNativeTargetAsmPrinter();
    });
}

std::string executable_path() {
    std::array<char, 4096> path{};
    auto size = static_cast<std::uint32_t>(path.size());
    if (_NSGetExecutablePath(path.data(), &size) != 0) {
        return {};
    }
    std::array<char, PATH_MAX> resolved{};
    if (realpath(path.data(), resolved.data()) == nullptr) {
        return {path.data()};
    }
    return {resolved.data()};
}

// What changes whenever the compiler does: its own file's size and time, so a rebuilt compiler never reuses objects
// an older one cached (D65).
std::string compiler_identity() {
    const std::string path = executable_path();
    struct stat info{};
    if (stat(path.c_str(), &info) != 0) {
        return "unknown";
    }
    return std::to_string(info.st_size) + ":" + std::to_string(info.st_mtimespec.tv_sec) + "." +
           std::to_string(info.st_mtimespec.tv_nsec);
}

// The system's cache folder, shared by every project (D65).
std::string cache_folder() {
    const char* home = std::getenv("HOME");
    llvm::SmallString<256> folder(home != nullptr ? home : "/tmp");
    llvm::sys::path::append(folder, "Library", "Caches", "unio", "objects");
    if (llvm::sys::fs::create_directories(folder)) {
        llvm::SmallString<256> fallback;
        llvm::sys::path::system_temp_directory(true, fallback);
        llvm::sys::path::append(fallback, "unio-objects");
        // If even this fails, writing an object says so with its own error.
        [[maybe_unused]] const std::error_code ignored = llvm::sys::fs::create_directories(fallback);
        return std::string(fallback);
    }
    return std::string(folder);
}

std::string hex(llvm::XXH128_hash_t hash) {
    static constexpr std::array<char, 16> digits{'0', '1', '2', '3', '4', '5', '6', '7',
                                                 '8', '9', 'a', 'b', 'c', 'd', 'e', 'f'};
    std::string out;
    for (const std::uint64_t part : {hash.high64, hash.low64}) {
        for (int shift = 60; shift >= 0; shift -= 4) {
            out.push_back(digits.at((part >> static_cast<unsigned>(shift)) & 0xFU));
        }
    }
    return out;
}

std::unique_ptr<llvm::TargetMachine> target_machine(Mode mode, std::string& error) {
    llvm::Triple triple(llvm::sys::getDefaultTargetTriple());
    if (triple.isOSDarwin()) {
        triple.setOSName(std::string("macosx") + minimum_macos);
    }
    const llvm::Target* target = llvm::TargetRegistry::lookupTarget(triple, error);
    if (target == nullptr) {
        return nullptr;
    }
    const llvm::TargetOptions options;
    const std::string cpu = triple.isAArch64() && triple.isOSDarwin() ? "apple-m1" : "generic";
    const auto level = mode == Mode::final ? llvm::CodeGenOptLevel::Aggressive : llvm::CodeGenOptLevel::None;
    return std::unique_ptr<llvm::TargetMachine>(
        target->createTargetMachine(triple, cpu, "", options, llvm::Reloc::PIC_, std::nullopt, level));
}

// trace: D124
// LLVM's full optimizing (#1), with its inliner (#3), tail recursion into loops (#7), what an `if` proved (#8),
// constants across procedures (#9), and cold check-failure code split out of the way (#13).
void optimize(llvm::Module& module, llvm::TargetMachine& machine) {
    llvm::LoopAnalysisManager loops;
    llvm::FunctionAnalysisManager functions;
    llvm::CGSCCAnalysisManager components;
    llvm::ModuleAnalysisManager modules;
    llvm::PassBuilder builder(&machine);
    builder.registerModuleAnalyses(modules);
    builder.registerCGSCCAnalyses(components);
    builder.registerFunctionAnalyses(functions);
    builder.registerLoopAnalyses(loops);
    builder.crossRegisterProxies(loops, functions, components, modules);
    llvm::ModulePassManager pipeline = builder.buildPerModuleDefaultPipeline(llvm::OptimizationLevel::O3);
    pipeline.addPass(llvm::HotColdSplittingPass());
    pipeline.run(module, modules);
}

struct Compiled {
    std::string object_path;
    std::string error;
};

Compiled compile_function(const ir::Module& module, std::uint32_t index, Mode mode, const std::string& identity,
                          const std::string& folder) {
    Compiled result;
    std::string error;
    auto machine = target_machine(mode, error);
    if (!machine) {
        result.error = "no LLVM target for this computer: " + error;
        return result;
    }
    llvm::LLVMContext context;
    std::unique_ptr<llvm::Module> generated = generate(module, index, context);
    generated->setTargetTriple(machine->getTargetTriple());
    generated->setDataLayout(machine->createDataLayout());
    std::string verify_errors;
    llvm::raw_string_ostream verify_stream(verify_errors);
    if (llvm::verifyModule(*generated, &verify_stream)) {
        result.error = "LLVM IR for `" + module.functions.at(index).name + "` is broken: " + verify_errors;
        return result;
    }
    std::string printed;
    llvm::raw_string_ostream printed_stream(printed);
    printed_stream << (mode == Mode::final ? "final\n" : "working\n") << identity << "\n";
    generated->print(printed_stream, nullptr);
    const std::string key = hex(llvm::xxh3_128bits(llvm::arrayRefFromStringRef(printed)));
    llvm::SmallString<256> path(folder);
    llvm::sys::path::append(path, key + ".o");
    result.object_path = std::string(path);
    if (llvm::sys::fs::exists(path)) {
        return result;
    }
    if (mode == Mode::final) {
        optimize(*generated, *machine);
    }
    llvm::SmallVector<char, 0> buffer;
    llvm::raw_svector_ostream stream(buffer);
    llvm::legacy::PassManager emitter;
    if (machine->addPassesToEmitFile(emitter, stream, nullptr, llvm::CodeGenFileType::ObjectFile)) {
        result.error = "LLVM can't write object files for this computer";
        return result;
    }
    emitter.run(*generated);
    // Written under a temporary name, then renamed, so another build never reads half a file.
    llvm::SmallString<256> temporary(folder);
    llvm::sys::path::append(temporary, key + "." + std::to_string(getpid()) + "." + std::to_string(index) + ".tmp");
    {
        std::error_code code;
        llvm::raw_fd_ostream file(temporary, code);
        if (code) {
            result.error = "can't write " + std::string(temporary) + ": " + code.message();
            return result;
        }
        file.write(buffer.data(), buffer.size());
    }
    if (const std::error_code code = llvm::sys::fs::rename(temporary, path)) {
        result.error = "can't write " + std::string(path) + ": " + code.message();
    }
    return result;
}

// Links with the platform's own linker, which always reads its own SDK; lld 23 can't read macOS 27's library lists.
std::expected<void, std::string> link(const std::vector<std::string>& objects, const std::string& output) {
    const std::string runtime = executable_folder() + "/libunio_runtime.a";
    std::vector<std::string> arguments = {"/usr/bin/ld", "-arch",       "arm64",       "-platform_version",
                                          "macos",       minimum_macos, minimum_macos, "-syslibroot",
                                          UNIO_SDK_PATH, "-o",          output,        "-dead_strip"};
    arguments.insert(arguments.end(), objects.begin(), objects.end());
    arguments.push_back(runtime);
    // A sanitized build's runtime needs the sanitizers' own library (Building rule 3).
    if (const std::string sanitizers = UNIO_SANITIZER_LIBRARY_DIR; !sanitizers.empty()) {
        arguments.insert(arguments.end(), {"-L" + sanitizers, "-lclang_rt.asan_osx_dynamic", "-rpath", sanitizers});
    }
    arguments.emplace_back("-lSystem");
    std::vector<char*> argv;
    argv.reserve(arguments.size() + 1);
    for (std::string& argument : arguments) {
        argv.push_back(argument.data());
    }
    argv.push_back(nullptr);
    std::array<int, 2> errors{-1, -1};
    if (pipe(errors.data()) != 0) {
        return std::unexpected(std::string("can't start the linker: no pipe"));
    }
    posix_spawn_file_actions_t actions{};
    posix_spawn_file_actions_init(&actions);
    posix_spawn_file_actions_adddup2(&actions, errors[1], 1);
    posix_spawn_file_actions_adddup2(&actions, errors[1], 2);
    posix_spawn_file_actions_addclose(&actions, errors[0]);
    pid_t child = 0;
    const int spawned = posix_spawn(&child, argv.front(), &actions, nullptr, argv.data(), environ);
    posix_spawn_file_actions_destroy(&actions);
    close(errors[1]);
    std::string said;
    std::array<char, 4096> buffer{};
    for (ssize_t count = 0; (count = read(errors[0], buffer.data(), buffer.size())) > 0;) {
        said.append(buffer.data(), static_cast<std::size_t>(count));
    }
    close(errors[0]);
    if (spawned != 0) {
        return std::unexpected("can't start the linker: " + std::string(std::strerror(spawned)));
    }
    int status = 0;
    while (waitpid(child, &status, 0) < 0 && errno == EINTR) {
    }
    if (!WIFEXITED(status) || WEXITSTATUS(status) != 0) {
        return std::unexpected("linking failed: " + said);
    }
    return {};
}

} // namespace

std::string executable_folder() {
    const std::string path = executable_path();
    return std::string(llvm::sys::path::parent_path(path));
}

std::expected<void, std::string> build(const ir::Module& module, const BuildSettings& settings) {
    initialize_targets();
    const std::string identity = compiler_identity();
    const std::string folder = cache_folder();
    const auto count = static_cast<std::uint32_t>(module.functions.size());
    std::vector<Compiled> compiled(count);
    unsigned threads = settings.threads != 0 ? settings.threads : std::max(1U, std::thread::hardware_concurrency());
    threads = std::min<unsigned>(threads, std::max<std::uint32_t>(count, 1));
    // trace: D65
    std::atomic<std::uint32_t> next{0};
    {
        std::vector<std::jthread> workers;
        workers.reserve(threads);
        for (unsigned worker = 0; worker < threads; ++worker) {
            workers.emplace_back([&] {
                for (std::uint32_t index = next++; index < count; index = next++) {
                    compiled.at(index) = compile_function(module, index, settings.mode, identity, folder);
                }
            });
        }
    }
    std::vector<std::string> objects;
    for (const Compiled& piece : compiled) {
        if (!piece.error.empty()) {
            return std::unexpected(piece.error);
        }
        objects.push_back(piece.object_path);
    }
    return link(objects, settings.output);
}

} // namespace unio
