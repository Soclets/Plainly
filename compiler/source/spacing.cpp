// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
// Spacing is fixed, and anything else is refused, with every mistake on a line in one message whose `fix:` corrects
// them all (D17, D18, D27, D59, D98, D100, D102, D132).

#include "spacing.h"

#include <algorithm>
#include <array>
#include <map>
#include <string_view>

namespace unio {

namespace {

constexpr std::array<std::string_view, 33> keywords = {
    "proc",  "end",    "let",     "var",    "if",  "else",   "at_least", "at_most",   "while",   "break",   "true",
    "false", "and",    "or",      "not",    "be",  "ignore", "crash",    "otherwise", "case",    "instead", "labels",
    "as",    "extern", "returns", "return", "fam", "div",    "rem",      "handler",   "include", "comment", "fails",
};

bool is_keyword(std::string_view word) {
    return std::ranges::find(keywords, word) != keywords.end();
}

// Whether a token ends a value, so a `-` after it subtracts rather than negates (D60).
bool ends_value(const Token& token) {
    switch (token.kind) {
    case TokenKind::number:
    case TokenKind::variable:
    case TokenKind::text:
    case TokenKind::single_quote:
    case TokenKind::close_round:
    case TokenKind::close_square:
        return true;
    case TokenKind::word:
        return !is_keyword(token.text) || token.text == "true" || token.text == "false";
    default:
        return false;
    }
}

bool is_word_operator(const Token& token) {
    return token.kind == TokenKind::word && (token.text == "and" || token.text == "or" || token.text == "at_least" ||
                                             token.text == "at_most" || token.text == "div" || token.text == "rem");
}

// A symbol with one space on each side, given what comes before it (D17, D27, D59, D98).
bool is_binary(const Token& token, const Token* before) {
    switch (token.kind) {
    case TokenKind::equals:
    case TokenKind::equals_equals:
    case TokenKind::not_equals:
    case TokenKind::plus:
    case TokenKind::star:
    case TokenKind::slash:
    case TokenKind::ampersand:
    case TokenKind::slide:
    case TokenKind::question_question:
        return true;
    case TokenKind::less:
    case TokenKind::greater:
        // `List<Whole>` fills in a type, with no spaces (D3).
        return before == nullptr || before->kind != TokenKind::word || before->text != "List";
    case TokenKind::minus:
        return before != nullptr && ends_value(*before);
    default:
        return is_word_operator(token);
    }
}

bool is_negation(const Token& token, const Token* before) {
    return token.kind == TokenKind::minus && (before == nullptr || !ends_value(*before));
}

bool is_type_bracket(const Token& token, const Token* before) {
    return (token.kind == TokenKind::less || token.kind == TokenKind::greater) && !is_binary(token, before);
}

struct Gap {
    const Token* left;
    const Token* right;
    const Token* before_left; // the token before `left`, for telling a negating `-` from a subtracting one
};

// How many spaces go between two tokens on one line: the spacing every example uses (D98).
std::uint32_t wanted(const Gap& gap) {
    const Token& left = *gap.left;
    const Token& right = *gap.right;
    switch (right.kind) {
    case TokenKind::semicolon:
    case TokenKind::comma:
    case TokenKind::close_round:
    case TokenKind::close_square:
    case TokenKind::question:
    case TokenKind::colon:
    case TokenKind::dot:
    case TokenKind::dot_star:
    case TokenKind::double_colon:
        return 0;
    default:
        break;
    }
    if (left.kind == TokenKind::open_round || left.kind == TokenKind::open_square || left.kind == TokenKind::dot ||
        left.kind == TokenKind::dot_star || left.kind == TokenKind::double_colon) {
        return 0;
    }
    if (is_negation(left, gap.before_left)) {
        return 0;
    }
    if (is_type_bracket(left, gap.before_left) || is_type_bracket(right, &left)) {
        return 0;
    }
    if (right.kind == TokenKind::open_round) {
        // A call's `(` touches its word; `be` is a keyword that's called too, as in `be.Text(...)` (D22).
        return left.kind == TokenKind::word && (!is_keyword(left.text) || left.text == "be") ? 0 : 1;
    }
    if (right.kind == TokenKind::open_square) {
        return ends_value(left) ? 0 : 1;
    }
    return 1;
}

struct Mistake {
    std::string why;
    std::vector<Place> carets;
};

class Spacing {
public:
    Spacing(const Sources& sources, std::uint32_t file, const Lexed& lexed, Messages& messages)
        : sources_(sources), file_(file), lexed_(lexed), messages_(messages) {}

    void run() {
        std::map<std::uint32_t, std::vector<const Token*>> lines;
        for (const Token& token : lexed_.tokens) {
            if (token.kind != TokenKind::end_of_file) {
                lines[token.line].push_back(&token);
            }
        }
        for (std::uint32_t line = 1; line <= lexed_.lines.size(); ++line) {
            const LineInfo& info = lexed_.lines.at(line - 1);
            const auto found = lines.find(line);
            if (found == lines.end()) {
                blank_or_comment(line, info);
                continue;
            }
            check_line(line, info, found->second);
        }
    }

private:
    [[nodiscard]] std::size_t line_start(std::uint32_t line) const {
        return sources_.file(file_).line_starts.at(line - 1);
    }

    [[nodiscard]] std::string_view written(const Token& token) const { return sources_.text(token.place); }

    // trace: D102
    void blank_or_comment(std::uint32_t line, const LineInfo& info) {
        const std::string_view text = sources_.line_text(file_, line);
        if (text.empty()) {
            return;
        }
        if (info.blank && !info.comment_only) {
            Message message;
            message.place = Place{.file = file_,
                                  .offset = static_cast<std::uint32_t>(line_start(line)),
                                  .length = static_cast<std::uint32_t>(text.size())};
            message.fix = "";
            message.why = "a blank line has no spaces on it.";
            messages_.add(std::move(message));
            return;
        }
        if (info.has_comment && info.trailing_spaces) {
            trailing_only(line, text);
        }
    }

    void trailing_only(std::uint32_t line, std::string_view text) {
        std::size_t end = text.size();
        while (end > 0 && (text.at(end - 1) == ' ' || text.at(end - 1) == '\t')) {
            --end;
        }
        Message message;
        message.place = Place{.file = file_,
                              .offset = static_cast<std::uint32_t>(line_start(line) + end),
                              .length = static_cast<std::uint32_t>(text.size() - end)};
        message.fix = std::string(text.substr(0, end));
        message.why = "a line never ends with spaces.";
        messages_.add(std::move(message));
    }

    void check_line(std::uint32_t line, const LineInfo& info, const std::vector<const Token*>& tokens) {
        if (std::ranges::any_of(
                tokens, [](const Token* token) { return token->kind == TokenKind::unknown || token->unterminated; })) {
            return;
        }
        const std::string_view text = sources_.line_text(file_, line);
        const std::size_t start = line_start(line);
        std::vector<Mistake> mistakes;
        std::map<const Token*, std::size_t> by_operator; // both sides of one operator are one mistake
        std::string fixed(text.substr(0, tokens.front()->place.offset - start));
        fixed += written(*tokens.front());
        for (std::size_t index = 1; index < tokens.size(); ++index) {
            const Gap gap{.left = tokens.at(index - 1),
                          .right = tokens.at(index),
                          .before_left = index >= 2 ? tokens.at(index - 2) : nullptr};
            const std::uint32_t should = wanted(gap);
            const std::uint32_t is = gap.right->place.offset - gap.left->place.end();
            fixed.append(should, ' ');
            fixed += written(*gap.right);
            if (is == should) {
                continue;
            }
            const Place caret = is == 0 ? Place{.file = file_, .offset = gap.right->place.offset, .length = 1}
                                        : Place{.file = file_, .offset = gap.left->place.end(), .length = is};
            add_gap_mistake(gap, should, is, caret, mistakes, by_operator);
        }
        // What follows the last token: a comment, and any spaces at the end.
        std::size_t rest = tokens.back()->place.end() - start;
        if (info.has_comment) {
            const std::size_t comment = info.comment_offset - start;
            const std::size_t gap = comment - rest;
            if (gap >= 2) {
                mistakes.push_back(Mistake{.why = "spaces never come two in a row, outside indentation.",
                                           .carets = {Place{.file = file_,
                                                            .offset = static_cast<std::uint32_t>(start + rest),
                                                            .length = static_cast<std::uint32_t>(gap)}}});
            }
            fixed.append(gap >= 2 ? 1 : gap, ' ');
            rest = comment;
        }
        std::size_t end = text.size();
        while (end > rest && (text.at(end - 1) == ' ' || text.at(end - 1) == '\t')) {
            --end;
        }
        fixed += text.substr(rest, end - rest);
        if (end < text.size()) {
            mistakes.push_back(Mistake{.why = "a line never ends with spaces.",
                                       .carets = {Place{.file = file_,
                                                        .offset = static_cast<std::uint32_t>(start + end),
                                                        .length = static_cast<std::uint32_t>(text.size() - end)}}});
        }
        if (mistakes.empty()) {
            return;
        }
        // trace: D100, D102
        std::vector<Place> carets;
        for (const Mistake& mistake : mistakes) {
            carets.insert(carets.end(), mistake.carets.begin(), mistake.carets.end());
        }
        std::ranges::sort(carets, {}, &Place::offset);
        Message message;
        message.place = carets.front();
        message.more_places.assign(carets.begin() + 1, carets.end());
        message.fix = fixed;
        message.why = mistakes.size() == 1
                          ? mistakes.front().why
                          : "spacing follows one rule: one space around operators and after `,` and `:`, none inside "
                            "brackets or before a call's `(`, never two in a row, and none at the end of a line.";
        messages_.add(std::move(message));
    }

    // trace: D18, D27, D98, D102, D132
    void add_gap_mistake(const Gap& gap, std::uint32_t should, std::uint32_t is, Place caret,
                         std::vector<Mistake>& mistakes, std::map<const Token*, std::size_t>& by_operator) {
        const Token& left = *gap.left;
        const Token& right = *gap.right;
        const auto quoted = [&](const Token& token) { return "`" + std::string(written(token)) + "`"; };
        // One mistake per operator or narrowing's `.`, whichever side its gaps are on.
        const Token* owner = nullptr;
        std::string why;
        if (should == 1 && is == 0 && is_binary(right, &left)) {
            owner = &right;
        } else if (should == 1 && is == 0 && is_binary(left, gap.before_left)) {
            owner = &left;
        } else if (should == 0 && (right.kind == TokenKind::dot || left.kind == TokenKind::dot)) {
            owner = right.kind == TokenKind::dot ? &right : &left;
            why = "a narrowing's `.` touches the words on both sides.";
        }
        if (owner != nullptr && why.empty()) {
            why = quoted(*owner) + " between two values has one space on each side.";
        }
        if (owner != nullptr) {
            if (const auto found = by_operator.find(owner); found != by_operator.end()) {
                mistakes.at(found->second).carets.push_back(caret);
                return;
            }
            by_operator.emplace(owner, mistakes.size());
            mistakes.push_back(Mistake{.why = why, .carets = {caret}});
            return;
        }
        if (should == 1 && is >= 2) {
            why = "spaces never come two in a row, outside indentation.";
        } else if (should == 1) {
            why = quoted(left) + " and " + quoted(right) + " have one space between them.";
        } else if (is_negation(left, gap.before_left)) {
            why = "a `-` that makes a value negative has no space after it.";
        } else {
            why = "no space goes between " + quoted(left) + " and " + quoted(right) + ".";
        }
        mistakes.push_back(Mistake{.why = why, .carets = {caret}});
    }

    const Sources& sources_;
    std::uint32_t file_;
    const Lexed& lexed_;
    Messages& messages_;
};

} // namespace

void check_spacing(const Sources& sources, std::uint32_t file, const Lexed& lexed, Messages& messages) {
    Spacing(sources, file, lexed, messages).run();
}

} // namespace unio
