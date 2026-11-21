// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
// Resolves names and types, turning the syntax tree into the typed tree, and refuses what the standard refuses (D62:
// refusals happen at the typed tree, so `unio check` stops after this).

#pragma once

#include "messages.h"
#include "syntax.h"

namespace unio {

void check(const Sources& sources, Program& program, Messages& messages);

} // namespace unio
