// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
// Lowers the typed tree to Unio IR (D62), writing out every check, every allocation and who owns each text (D130).

#pragma once

#include "ir.h"
#include "syntax.h"

namespace unio {

ir::Module lower(const Sources& sources, const Program& program);

} // namespace unio
