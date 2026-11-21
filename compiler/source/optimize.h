// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
// `final` mode's own optimizations on Unio IR (D124), each keeping D46's rule: the program does exactly what it does
// in `working` mode, and a check goes only where it's proved unable to fail.

#pragma once

#include "ir.h"

namespace unio {

void optimize(ir::Module& module);

// The passes one by one, for the unit tests.
namespace passes {
void simplify(ir::Function& function, const ir::Module& module);
void remove_checks(ir::Function& function);
void find_side_effects(ir::Module& module);
void inline_small(ir::Module& module);
void inline_into_itself(ir::Module& module);
void merge_repeated(ir::Function& function, const ir::Module& module);
void constants_across(ir::Module& module);
} // namespace passes

} // namespace unio
