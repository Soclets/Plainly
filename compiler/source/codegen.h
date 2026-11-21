// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
// Turns one procedure's Unio IR into its own LLVM module (D62, D131), so each procedure compiles on its own thread
// and is cached on its own (D65).

#pragma once

#include "ir.h"

#include <memory>

namespace llvm {
class LLVMContext;
class Module;
} // namespace llvm

namespace unio {

std::unique_ptr<llvm::Module> generate(const ir::Module& module, std::uint32_t function, llvm::LLVMContext& context);

} // namespace unio
