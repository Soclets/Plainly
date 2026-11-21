// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
// The `unio` command: `run`, `compile`, `check` and `oracle` (D50, D91, D136).

#pragma once

#include <span>
#include <string>
#include <vector>

namespace unio {

int drive(std::span<char*> arguments);

} // namespace unio
