// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
// From Unio IR to a native executable: each procedure is its own LLVM module, compiled on its own thread and cached
// under a fingerprint of everything that affects it (D65), then all are linked with the runtime by lld (D131).

#pragma once

#include "ir.h"

#include <expected>
#include <string>

namespace unio {

enum class Mode : std::uint8_t { working, final };

struct BuildSettings {
    Mode mode = Mode::working;
    std::string output;
    unsigned threads = 0; // 0: every core (D65)
};

// Gives the problem, for an internal error (D82), when something below the program went wrong.
std::expected<void, std::string> build(const ir::Module& module, const BuildSettings& settings);

// The folder the runtime library and the compiler live in.
std::string executable_folder();

} // namespace unio
