// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
// The spacing check: one message per line with spacing mistakes (D98, D100).

#pragma once

#include "lexer.h"
#include "messages.h"

namespace unio {

void check_spacing(const Sources& sources, std::uint32_t file, const Lexed& lexed, Messages& messages);

} // namespace unio
