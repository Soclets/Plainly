// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception

#include "lexer.h"

#include <algorithm>
#include <array>
#include <optional>
#include <string_view>

namespace unio {

namespace {

bool is_word_start(char character) {
    return (character >= 'a' && character <= 'z') || (character >= 'A' && character <= 'Z') || character == '_';
}

bool is_word_part(char character) {
    return is_word_start(character) || (character >= '0' && character <= '9');
}

bool is_digit(char character) {
    return character >= '0' && character <= '9';
}

bool is_letter(char character) {
    return (character >= 'a' && character <= 'z') || (character >= 'A' && character <= 'Z');
}

class Lexer {
public:
    Lexer(const Sources& sources, std::uint32_t file, Messages& messages)
        : sources_(sources), file_(file), text_(sources.file(file).text), messages_(messages) {}

    Lexed run() {
        const std::uint32_t line_total = sources_.line_count(file_);
        result_.lines.resize(line_total + 1);
        std::uint32_t line = 1;
        while (line <= line_total) {
            line = lex_line(line);
        }
        Token end;
        end.kind = TokenKind::end_of_file;
        end.place = Place{.file = file_, .offset = static_cast<std::uint32_t>(text_.size()), .length = 0};
        end.line = line_total + 1;
        end.first_on_line = true;
        result_.tokens.push_back(end);
        result_.lines.resize(line_total);
        return std::move(result_);
    }

private:
    [[nodiscard]] Place place(std::size_t offset, std::size_t length) const {
        return Place{
            .file = file_, .offset = static_cast<std::uint32_t>(offset), .length = static_cast<std::uint32_t>(length)};
    }

    [[nodiscard]] std::size_t line_start(std::uint32_t line) const {
        return sources_.file(file_).line_starts.at(line - 1);
    }

    // trace: D74
    // A `comment name:` line starts a block comment, which runs to its own `end comment name`.
    std::optional<std::uint32_t> block_comment(std::uint32_t line, std::string_view content) {
        if (!content.starts_with("comment ") || !content.ends_with(':')) {
            return std::nullopt;
        }
        const std::string_view name = content.substr(8, content.size() - 9);
        if (name.empty() || !std::ranges::all_of(name, is_word_part)) {
            return std::nullopt;
        }
        const std::string closer = "end comment " + std::string(name);
        const std::uint32_t line_total = sources_.line_count(file_);
        for (std::uint32_t inner = line + 1; inner <= line_total; ++inner) {
            std::string_view inner_text = sources_.line_text(file_, inner);
            while (!inner_text.empty() && inner_text.front() == ' ') {
                inner_text.remove_prefix(1);
            }
            if (inner_text == closer) {
                for (std::uint32_t covered = line; covered <= inner; ++covered) {
                    LineInfo& info = result_.lines.at(covered - 1);
                    info.blank = false;
                    info.comment_only = true;
                }
                return inner + 1;
            }
        }
        const std::size_t start = line_start(line) + (sources_.line_text(file_, line).size() - content.size());
        messages_.refuse(place(start, content.size()), "the file ends before `" + closer + "`",
                         "every block ends with `end` and what it closes.");
        const std::uint32_t total = sources_.line_count(file_);
        for (std::uint32_t covered = line; covered <= total; ++covered) {
            result_.lines.at(covered - 1).comment_only = true;
            result_.lines.at(covered - 1).blank = false;
        }
        return total + 1;
    }

    std::uint32_t lex_line(std::uint32_t line) {
        const std::string_view line_text = sources_.line_text(file_, line);
        const std::size_t start = line_start(line);
        LineInfo& info = result_.lines.at(line - 1);
        std::size_t index = 0;
        while (index < line_text.size() && (line_text[index] == ' ' || line_text[index] == '\t')) {
            if (line_text[index] == '\t') {
                info.has_tab = true;
            } else if (!info.has_tab) {
                ++info.indent;
            }
            ++index;
        }
        if (index == line_text.size()) {
            info.blank = true;
            info.trailing_spaces = !line_text.empty();
            return line + 1;
        }
        info.blank = false;
        info.trailing_spaces = line_text.back() == ' ' || line_text.back() == '\t';
        if (const auto after = block_comment(line, line_text.substr(index))) {
            // Recompute this line's indentation facts, since block_comment overwrote only the flags.
            return *after;
        }
        bool first = true;
        while (index < line_text.size()) {
            const char character = line_text[index];
            if (character == ' ' || character == '\t') {
                ++index;
                continue;
            }
            if (line_text.substr(index).starts_with("//")) {
                info.has_comment = true;
                info.comment_offset = static_cast<std::uint32_t>(start + index);
                if (first) {
                    info.comment_only = true;
                }
                break;
            }
            index = lex_token(line, line_text, start, index, first);
            first = false;
        }
        return line + 1;
    }

    void push(TokenKind kind, std::uint32_t line, std::size_t offset, std::size_t length, bool first,
              std::string text) {
        Token token;
        token.kind = kind;
        token.place = place(offset, length);
        token.line = line;
        token.first_on_line = first;
        token.text = std::move(text);
        result_.tokens.push_back(std::move(token));
    }

    std::size_t lex_text(std::uint32_t line, std::string_view line_text, std::size_t start, std::size_t index,
                         bool first, char quote) {
        // trace: D21, D30
        std::string meaning;
        std::size_t at = index + 1;
        bool closed = false;
        while (at < line_text.size()) {
            const char character = line_text[at];
            if (character == '\\' && at + 1 < line_text.size()) {
                const char next = line_text[at + 1];
                if (is_letter(next)) {
                    messages_.not_designed_yet(place(start + at, 2), "`\\` before a letter, as in `\\" +
                                                                         std::string(1, next) + "`, isn't decided yet");
                }
                meaning.push_back(next);
                at += 2;
                continue;
            }
            if (character == quote) {
                closed = true;
                ++at;
                break;
            }
            meaning.push_back(character);
            ++at;
        }
        const TokenKind kind = quote == '"' ? TokenKind::text : TokenKind::single_quote;
        push(kind, line, start + index, at - index, first, std::move(meaning));
        if (!closed) {
            result_.tokens.back().unterminated = true;
        }
        return at;
    }

    std::size_t lex_token(std::uint32_t line, std::string_view line_text, std::size_t start, std::size_t index,
                          bool first) {
        const char character = line_text[index];
        const std::string_view rest = line_text.substr(index);
        if (is_word_start(character)) {
            std::size_t end = index;
            while (end < line_text.size() && is_word_part(line_text[end])) {
                ++end;
            }
            push(TokenKind::word, line, start + index, end - index, first,
                 std::string(line_text.substr(index, end - index)));
            return end;
        }
        if (character == '$') {
            std::size_t end = index + 1;
            while (end < line_text.size() && is_word_part(line_text[end])) {
                ++end;
            }
            push(TokenKind::variable, line, start + index, end - index, first,
                 std::string(line_text.substr(index, end - index)));
            return end;
        }
        if (is_digit(character)) {
            std::size_t end = index;
            while (end < line_text.size() && is_word_part(line_text[end])) {
                ++end;
            }
            push(TokenKind::number, line, start + index, end - index, first,
                 std::string(line_text.substr(index, end - index)));
            return end;
        }
        if (character == '"' || character == '\'') {
            return lex_text(line, line_text, start, index, first, character);
        }
        struct Symbol {
            std::string_view spelling;
            TokenKind kind;
        };
        static constexpr auto symbols = std::to_array<Symbol>({
            {.spelling = "|>", .kind = TokenKind::slide},
            {.spelling = "??", .kind = TokenKind::question_question},
            {.spelling = "==", .kind = TokenKind::equals_equals},
            {.spelling = "!=", .kind = TokenKind::not_equals},
            {.spelling = "::", .kind = TokenKind::double_colon},
            {.spelling = ".*", .kind = TokenKind::dot_star},
            {.spelling = "(", .kind = TokenKind::open_round},
            {.spelling = ")", .kind = TokenKind::close_round},
            {.spelling = "[", .kind = TokenKind::open_square},
            {.spelling = "]", .kind = TokenKind::close_square},
            {.spelling = ",", .kind = TokenKind::comma},
            {.spelling = ":", .kind = TokenKind::colon},
            {.spelling = ";", .kind = TokenKind::semicolon},
            {.spelling = ".", .kind = TokenKind::dot},
            {.spelling = "=", .kind = TokenKind::equals},
            {.spelling = "<", .kind = TokenKind::less},
            {.spelling = ">", .kind = TokenKind::greater},
            {.spelling = "+", .kind = TokenKind::plus},
            {.spelling = "-", .kind = TokenKind::minus},
            {.spelling = "*", .kind = TokenKind::star},
            {.spelling = "/", .kind = TokenKind::slash},
            {.spelling = "&", .kind = TokenKind::ampersand},
            {.spelling = "?", .kind = TokenKind::question},
        });
        for (const Symbol& symbol : symbols) {
            if (rest.starts_with(symbol.spelling)) {
                push(symbol.kind, line, start + index, symbol.spelling.size(), first, std::string(symbol.spelling));
                return index + symbol.spelling.size();
            }
        }
        // A character Unio doesn't use: one whole UTF-8 character, so the `^` covers it.
        std::size_t length = 1;
        const auto byte = static_cast<unsigned char>(character);
        if (byte >= 0xF0U) {
            length = 4;
        } else if (byte >= 0xE0U) {
            length = 3;
        } else if (byte >= 0xC0U) {
            length = 2;
        }
        length = std::min(length, line_text.size() - index);
        push(TokenKind::unknown, line, start + index, length, first, std::string(line_text.substr(index, length)));
        return index + length;
    }

    const Sources& sources_;
    std::uint32_t file_;
    std::string_view text_;
    Messages& messages_;
    Lexed result_;
};

} // namespace

Lexed lex(const Sources& sources, std::uint32_t file, Messages& messages) {
    return Lexer(sources, file, messages).run();
}

const char* token_spelling(TokenKind kind) {
    switch (kind) {
    case TokenKind::open_round:
        return "(";
    case TokenKind::close_round:
        return ")";
    case TokenKind::open_square:
        return "[";
    case TokenKind::close_square:
        return "]";
    case TokenKind::comma:
        return ",";
    case TokenKind::colon:
        return ":";
    case TokenKind::double_colon:
        return "::";
    case TokenKind::semicolon:
        return ";";
    case TokenKind::dot:
        return ".";
    case TokenKind::dot_star:
        return ".*";
    case TokenKind::equals:
        return "=";
    case TokenKind::equals_equals:
        return "==";
    case TokenKind::not_equals:
        return "!=";
    case TokenKind::less:
        return "<";
    case TokenKind::greater:
        return ">";
    case TokenKind::plus:
        return "+";
    case TokenKind::minus:
        return "-";
    case TokenKind::star:
        return "*";
    case TokenKind::slash:
        return "/";
    case TokenKind::ampersand:
        return "&";
    case TokenKind::slide:
        return "|>";
    case TokenKind::question:
        return "?";
    case TokenKind::question_question:
        return "??";
    case TokenKind::word:
    case TokenKind::variable:
    case TokenKind::number:
    case TokenKind::text:
    case TokenKind::single_quote:
    case TokenKind::unknown:
    case TokenKind::end_of_file:
        break;
    }
    return "";
}

} // namespace unio
