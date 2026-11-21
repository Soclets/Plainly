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
// Folds constants, simplifies arithmetic and branches, and sweeps away what's unused (D124 #1's groundwork).
void simplify(ir::Function& function);
// Turns a check into plain arithmetic where ranges, and what each `if` proved, show it can't fail (D124 #2, #8).
void remove_checks(ir::Function& function);
// Works out which procedures have no side effects (D124 #6, GCC's -fipa-pure-const).
void find_side_effects(ir::Module& module);
// Inlines small procedures into the places that call them (D124 #3).
void inline_small(ir::Module& module);
// Inlines a procedure with no side effects into itself where that makes two of its calls the same (D124 #6).
void inline_into_itself(ir::Module& module);
// Copies a small block that returns into each block that jumps to it, so a repeated call becomes visible (D124 #6).
void duplicate_returns(ir::Function& function, const ir::Module& module);
// Merges repeated calculations and repeated calls with no side effects, keeping the first (D124 #6, #18).
void merge_repeated(ir::Function& function, const ir::Module& module);
// Builds in a parameter's value where every call gives the same constant (D124 #9).
void constants_across(ir::Module& module);
// Turns a procedure calling itself as its last act into a loop (D124 #7).
void recursion_into_loops(ir::Function& function, std::uint32_t self);
} // namespace passes

} // namespace unio
