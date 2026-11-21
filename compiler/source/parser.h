// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
// Reads tokens into the syntax tree, refusing what can't be read (standard chapters 2 to 5, D55's recovery).

#pragma once

#include "lexer.h"
#include "messages.h"
#include "syntax.h"

namespace unio {

void parse(const Sources& sources, std::uint32_t file, const Lexed& lexed, Program& program, Messages& messages);

// Prints an expression the one way Unio spells it (D98), for `fix:` lines.
std::string print(const Expression& expression);

} // namespace unio
