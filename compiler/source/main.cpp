// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
// `unio`, the command for Unio's compiler, `Unlo\n` (D50, D136, D137).

#include "driver.h"

#include <cstddef>
#include <span>

int main(int argc, char** argv) {
    return unio::drive(std::span<char*>(argv, static_cast<std::size_t>(argc)));
}
