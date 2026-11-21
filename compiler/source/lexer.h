// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
// Splits a file into tokens: words, `$` names, numbers, text and symbols (standard chapter 3).

#pragma once

#include "messages.h"
#include "source.h"

#include <cstdint>
#include <string>
#include <vector>

namespace unio {

enum class TokenKind : std::uint8_t {
    word,         // a keyword, a procedure's name, a type or a label
    variable,     // a `$` name (D125); its text is the whole `$name`
    number,       // digits
    text,         // "..." (D21)
    single_quote, // '...', refused (D21)
    open_round,
    close_round,
    open_square,
    close_square,
    comma,
    colon,
    double_colon,
    semicolon,
    dot,
    dot_star,
    equals,
    equals_equals,
    not_equals,
    less,
    greater,
    plus,
    minus,
    star,
    slash,
    ampersand,
    slide, // |>
    question,
    question_question,
    unknown,
    end_of_file,
};

struct Token {
    TokenKind kind = TokenKind::unknown;
    Place place;
    std::uint32_t line = 0;
    bool first_on_line = false;
    std::string text; // the token as written; for text, what it means, with D30's `\` applied
    bool unterminated = false;
};

// What the lexer learns about each line, for indentation (D10, D145) and blank lines (D97).
struct LineInfo {
    std::uint32_t indent = 0; // leading spaces
    bool has_tab = false;     // a tab in the indentation
    bool blank = true;        // nothing but spaces
    bool comment_only = false;
    bool trailing_spaces = false;
    std::uint32_t comment_offset = 0; // where a `//` starts, or 0 when there's none
    bool has_comment = false;
};

struct Lexed {
    std::vector<Token> tokens;
    std::vector<LineInfo> lines; // index 0 is line 1
};

Lexed lex(const Sources& sources, std::uint32_t file, Messages& messages);

const char* token_spelling(TokenKind kind);

} // namespace unio
