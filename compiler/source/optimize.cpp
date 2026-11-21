// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception

#include "optimize.h"

namespace unio {

namespace passes {

void simplify(ir::Function& function, const ir::Module& module) {
    (void)function;
    (void)module;
}

void remove_checks(ir::Function& function) {
    (void)function;
}

void find_side_effects(ir::Module& module) {
    (void)module;
}

void inline_small(ir::Module& module) {
    (void)module;
}

void inline_into_itself(ir::Module& module) {
    (void)module;
}

void merge_repeated(ir::Function& function, const ir::Module& module) {
    (void)function;
    (void)module;
}

void constants_across(ir::Module& module) {
    (void)module;
}

} // namespace passes

void optimize(ir::Module& module) {
    (void)module;
}

} // namespace unio
