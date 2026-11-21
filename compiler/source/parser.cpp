// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception

#include "parser.h"

#include <algorithm>
#include <array>
#include <charconv>
#include <limits>
#include <string_view>
#include <utility>

namespace unio {

const char* type_name(Type type) {
    switch (type) {
    case Type::whole:
        return "Whole";
    case Type::boolean:
        return "Bool";
    case Type::text:
        return "Text";
    case Type::nothing:
        return "nothing";
    case Type::unknown:
        break;
    }
    return "unknown";
}

const char* operator_spelling(BinaryOperator op) {
    switch (op) {
    case BinaryOperator::add:
        return "+";
    case BinaryOperator::subtract:
        return "-";
    case BinaryOperator::multiply:
        return "*";
    case BinaryOperator::join:
        return "&";
    case BinaryOperator::equal:
        return "==";
    case BinaryOperator::not_equal:
        return "!=";
    case BinaryOperator::less:
        return "<";
    case BinaryOperator::greater:
        return ">";
    case BinaryOperator::at_least:
        return "at_least";
    case BinaryOperator::at_most:
        return "at_most";
    case BinaryOperator::logical_and:
        return "and";
    case BinaryOperator::logical_or:
        return "or";
    }
    return "";
}

bool is_comparison(BinaryOperator op) {
    return op == BinaryOperator::equal || op == BinaryOperator::not_equal || op == BinaryOperator::less ||
           op == BinaryOperator::greater || op == BinaryOperator::at_least || op == BinaryOperator::at_most;
}

bool is_arithmetic(BinaryOperator op) {
    return op == BinaryOperator::add || op == BinaryOperator::subtract || op == BinaryOperator::multiply;
}

namespace {

void print_arguments(std::string& out, const std::vector<Argument>& arguments) {
    out += '(';
    bool first = true;
    for (const Argument& argument : arguments) {
        if (!first) {
            out += ", ";
        }
        first = false;
        if (argument.label) {
            out += *argument.label + ": ";
        }
        out += print(*argument.value);
    }
    out += ')';
}

void print_word(std::string& out, const std::string& word, const std::vector<Narrowing>& narrowings) {
    out += word;
    for (const Narrowing& narrowing : narrowings) {
        out += "." + narrowing.name;
    }
}

std::string quote_text(const std::string& text) {
    std::string out = "\"";
    for (const char character : text) {
        if (character == '"' || character == '\\') {
            out.push_back('\\');
        }
        out.push_back(character);
    }
    out += '"';
    return out;
}

} // namespace

std::string print(const Expression& expression) {
    std::string out;
    switch (expression.kind) {
    case ExpressionKind::whole:
        out = std::to_string(expression.whole);
        break;
    case ExpressionKind::boolean:
        out = expression.boolean ? "true" : "false";
        break;
    case ExpressionKind::text:
        out = quote_text(expression.text);
        break;
    case ExpressionKind::variable:
        out = expression.text;
        break;
    case ExpressionKind::call:
        print_word(out, expression.text, expression.narrowings);
        if (expression.has_brackets) {
            print_arguments(out, expression.arguments);
        }
        break;
    case ExpressionKind::negate:
        out = "-" + print(*expression.left);
        break;
    case ExpressionKind::logical_not:
        out = "not " + print(*expression.left);
        break;
    case ExpressionKind::binary:
        out = print(*expression.left) + " " + operator_spelling(expression.op) + " " + print(*expression.right);
        break;
    case ExpressionKind::slide:
        out = print(*expression.left);
        for (const Step& step : expression.steps) {
            out += " |> ";
            print_word(out, step.word, step.narrowings);
            if (!step.arguments.empty()) {
                print_arguments(out, step.arguments);
            }
        }
        break;
    case ExpressionKind::handled:
        out = print(*expression.left) + " ?? ";
        if (expression.fallback == Fallback::ignore) {
            out += "ignore";
        } else if (expression.fallback == Fallback::crash) {
            out += "crash";
        } else {
            out += print(*expression.right);
        }
        break;
    case ExpressionKind::group:
        out = "(" + print(*expression.left) + ")";
        break;
    case ExpressionKind::error:
        break;
    }
    return out;
}

namespace {

// trace: D2
constexpr std::array<std::string_view, 33> keywords = {
    "proc",  "end",    "let",     "var",    "if",  "else",   "at_least", "at_most",   "while",   "break",   "true",
    "false", "and",    "or",      "not",    "be",  "ignore", "crash",    "otherwise", "case",    "instead", "labels",
    "as",    "extern", "returns", "return", "fam", "div",    "rem",      "handler",   "include", "comment", "fails",
};

bool is_keyword(std::string_view word) {
    return std::ranges::find(keywords, word) != keywords.end();
}

int infix_power(const Token& token) {
    switch (token.kind) {
    case TokenKind::slide:
        return 1;
    case TokenKind::equals_equals:
    case TokenKind::not_equals:
    case TokenKind::less:
    case TokenKind::greater:
        return 5;
    case TokenKind::plus:
    case TokenKind::minus:
    case TokenKind::ampersand:
        return 6;
    case TokenKind::star:
    case TokenKind::slash:
        return 7;
    case TokenKind::word:
        if (token.text == "or") {
            return 2;
        }
        if (token.text == "and") {
            return 3;
        }
        if (token.text == "at_least" || token.text == "at_most") {
            return 5;
        }
        if (token.text == "div" || token.text == "rem") {
            return 7;
        }
        return 0;
    default:
        return 0;
    }
}

class Parser {
public:
    Parser(const Sources& sources, std::uint32_t file, const Lexed& lexed, Program& program, Messages& messages)
        : sources_(sources), file_(file), lexed_(lexed), tokens_(lexed.tokens), program_(program), messages_(messages),
          expected_indent_(lexed.lines.size() + 2, -1), before_closer_(lexed.lines.size() + 2, false) {}

    void run() {
        while (!at_end()) {
            parse_top_level();
        }
        check_layout();
    }

private:
    // --- Tokens -------------------------------------------------------------------------------------------------

    [[nodiscard]] const Token& peek(std::size_t ahead = 0) const {
        return tokens_.at(std::min(index_ + ahead, tokens_.size() - 1));
    }
    [[nodiscard]] const Token& previous() const { return tokens_.at(index_ == 0 ? 0 : index_ - 1); }
    [[nodiscard]] bool at_end() const { return peek().kind == TokenKind::end_of_file; }
    const Token& advance() {
        const Token& token = peek();
        if (index_ + 1 < tokens_.size()) {
            ++index_;
        }
        return token;
    }
    [[nodiscard]] bool is(TokenKind kind) const { return peek().kind == kind; }
    [[nodiscard]] bool is_word(std::string_view word, std::size_t ahead = 0) const {
        return peek(ahead).kind == TokenKind::word && peek(ahead).text == word;
    }
    // A token that continues the current line, rather than starting the next statement.
    [[nodiscard]] bool on_line() const { return !peek().first_on_line && !at_end(); }

    [[nodiscard]] std::string line_text(std::uint32_t line) const {
        return std::string(sources_.line_text(file_, line));
    }
    [[nodiscard]] std::uint32_t line_of(Place place) const { return sources_.line_column(place).line; }
    [[nodiscard]] std::size_t line_start(std::uint32_t line) const {
        return sources_.file(file_).line_starts.at(line - 1);
    }

    // The line holding `place`, with the `length` bytes at `place` replaced by `replacement`.
    [[nodiscard]] std::string replaced_line(Place place, std::uint32_t length, std::string_view replacement) const {
        const std::uint32_t line = line_of(place);
        std::string text = line_text(line);
        const std::size_t column = place.offset - line_start(line);
        text.replace(column, std::min<std::size_t>(length, text.size() - column), replacement);
        return text;
    }

    // Just past the end of the file's last line with anything on it, where a missing closer would go.
    [[nodiscard]] Place file_end() const {
        std::uint32_t line = sources_.line_count(file_);
        while (line > 1 && line_text(line).empty()) {
            --line;
        }
        return Place{.file = file_,
                     .offset = static_cast<std::uint32_t>(line_start(line) + line_text(line).size()),
                     .length = 1};
    }

    void skip_statement() {
        while (!at_end()) {
            if (is(TokenKind::semicolon)) {
                advance();
                return;
            }
            advance();
            if (peek().first_on_line) {
                return;
            }
        }
    }

    void skip_line() {
        advance();
        while (on_line()) {
            advance();
        }
    }

    // --- Layout: indentation, comments and blank lines ----------------------------------------------------------

    // trace: D10
    void check_indent(const Token& first, int depth) {
        const std::uint32_t line = first.line;
        expected_indent_.at(line) = depth * 4;
        const LineInfo& info = lexed_.lines.at(line - 1);
        const auto expected = static_cast<std::uint32_t>(depth * 4);
        if (!info.has_tab && info.indent == expected) {
            return;
        }
        const std::size_t start = line_start(line);
        const auto indentation = static_cast<std::uint32_t>(first.place.offset - start);
        const std::string text = line_text(line);
        Message message;
        message.place = Place{
            .file = file_, .offset = static_cast<std::uint32_t>(start), .length = indentation == 0 ? 1U : indentation};
        message.fix = std::string(expected, ' ') + text.substr(indentation);
        message.why = "each level of indentation is 4 spaces, and tabs aren't used.";
        messages_.add(std::move(message));
    }

    // trace: D97, D145
    void check_layout() {
        const auto& lines = lexed_.lines;
        const auto total = static_cast<std::uint32_t>(lines.size());
        std::uint32_t line = 1;
        while (line <= total) {
            if (!lines.at(line - 1).blank || lines.at(line - 1).trailing_spaces) {
                ++line;
                continue;
            }
            std::uint32_t run_end = line;
            while (run_end + 1 <= total && lines.at(run_end).blank) {
                ++run_end;
            }
            if (run_end > line) {
                const std::uint32_t extra = run_end - line;
                Message message;
                message.place =
                    Place{.file = file_, .offset = static_cast<std::uint32_t>(line_start(line + 1)), .length = 0};
                message.what = std::to_string(extra) + " extra blank line" + (extra == 1 ? "" : "s") +
                               ", after the one on line " + std::to_string(line);
                message.why = "one blank line is enough to separate the parts of a program.";
                messages_.add(std::move(message));
            }
            line = run_end + 1;
        }
        // A comment on its own line is indented like the next line of code, or one level in before a closer.
        for (std::uint32_t comment = 1; comment <= total; ++comment) {
            const LineInfo& info = lines.at(comment - 1);
            if (!info.comment_only || info.blank) {
                continue;
            }
            std::uint32_t next = comment + 1;
            while (next <= total && (lines.at(next - 1).blank || lines.at(next - 1).comment_only)) {
                ++next;
            }
            int expected = 0;
            if (next <= total && expected_indent_.at(next) >= 0) {
                expected = expected_indent_.at(next) + (before_closer_.at(next) ? 4 : 0);
            }
            if (!info.has_tab && std::cmp_equal(info.indent, expected)) {
                continue;
            }
            const std::string text = line_text(comment);
            const std::size_t content = text.find_first_not_of(" \t");
            Message message;
            message.place = Place{.file = file_,
                                  .offset = static_cast<std::uint32_t>(line_start(comment)),
                                  .length = content == 0 ? 1U : static_cast<std::uint32_t>(content)};
            message.fix = std::string(static_cast<std::size_t>(expected), ' ') + text.substr(content);
            message.why = "each level of indentation is 4 spaces, and tabs aren't used.";
            messages_.add(std::move(message));
        }
    }

    // --- Messages -------------------------------------------------------------------------------------------------

    // trace: D7, D85, D92
    void expected_semicolon() {
        const Token& last = previous();
        if (peek().first_on_line || at_end()) {
            const Place after{.file = file_, .offset = last.place.end(), .length = 1};
            messages_.fix(after, replaced_line(after, 0, ";"), "every statement ends with `;`.");
            return;
        }
        Message message;
        message.place = peek().place;
        message.what = "`;` should be here, not `" + std::string(sources_.text(peek().place)) + "`";
        messages_.add(std::move(message));
        skip_statement();
    }

    // trace: D92
    void expected_here(std::string_view wanted) {
        Message message;
        if (at_end()) {
            message.place = Place{.file = file_, .offset = previous().place.end(), .length = 1};
            message.what = "the file ends where " + std::string(wanted) + " should be";
        } else if (peek().first_on_line) {
            message.place = Place{.file = file_, .offset = previous().place.end(), .length = 1};
            message.what = "the line ends where " + std::string(wanted) + " should be";
        } else {
            message.place = peek().place;
            message.what =
                std::string(wanted) + " should be here, not `" + std::string(sources_.text(peek().place)) + "`";
        }
        messages_.add(std::move(message));
        failed_ = true;
    }

    void continuation_line() {
        if (!continued_) {
            messages_.not_designed_yet(peek().place, "how a statement's later lines are indented isn't decided yet");
            continued_ = true;
        }
    }

    // --- Top level ------------------------------------------------------------------------------------------------

    void parse_top_level() {
        const Token& first = peek();
        if (!first.first_on_line) {
            skip_line();
            return;
        }
        check_indent(first, 0);
        if (is_word("proc")) {
            parse_procedure();
            return;
        }
        if (is_word("fam")) {
            messages_.not_built_yet(first.place, "families, `fam` (D41), aren't built yet");
            skip_to_closer("fam");
            return;
        }
        if (is_word("extern")) {
            messages_.not_designed_yet(first.place,
                                       "how `extern.cpp` procedures are built and linked isn't decided yet");
            skip_to_closer("proc");
            return;
        }
        if (is_word("handler")) {
            messages_.not_built_yet(first.place, "handlers, `handler` (D42), aren't built yet");
            skip_to_closer("handler");
            return;
        }
        // trace: D4
        Message message;
        const Place start = first.place;
        skip_statement();
        message.place = span(start, previous().place);
        message.what = "this line isn't inside a procedure";
        message.why = "a program is made of procedures, and every statement goes inside one."; // trace: D161
        messages_.add(std::move(message));
        while (on_line()) {
            advance();
        }
    }

    void skip_to_closer(std::string_view word) {
        skip_line();
        while (!at_end()) {
            if (peek().first_on_line && is_word("end") && is_word(word, 1)) {
                skip_line();
                return;
            }
            skip_line();
        }
    }

    // trace: D4, D8, D37, D38, D125, D126
    void parse_procedure() {
        auto procedure = std::make_unique<Procedure>();
        const Token& proc_token = advance();
        procedure->header_place = proc_token.place;
        const std::uint32_t header_line = proc_token.line;
        if (is(TokenKind::dot) && on_line()) {
            advance();
            if (is_word("main")) {
                procedure->is_main = true;
                procedure->name = "main";
                procedure->place = advance().place;
            } else {
                messages_.refuse(peek().place, "`proc` has no narrowing `." + peek().text + "`",
                                 "`proc`'s only narrowing is `.main`.");
                skip_to_end_of_procedure();
                return;
            }
        } else if (is(TokenKind::word) && on_line()) {
            const Token& name = advance();
            procedure->name = name.text;
            procedure->place = name.place;
        } else {
            expected_here("the procedure's name");
            skip_to_end_of_procedure();
            return;
        }

        // trace: D4, D88, D165
        if ((is(TokenKind::open_square) || is(TokenKind::open_round)) && on_line()) {
            const Token& open = advance();
            const bool square = open.kind == TokenKind::open_square;
            const TokenKind closer = square ? TokenKind::close_square : TokenKind::close_round;
            if (is(closer)) {
                const Token& close = advance();
                const Place brackets = span(open.place, close.place);
                if (procedure->is_main) {
                    messages_.refuse(brackets, "`proc.main` takes nothing",
                                     "only the program's start runs `proc.main`, so it's never given anything.");
                } else {
                    messages_.fix(brackets, replaced_line(brackets, brackets.length, ""),
                                  "a definition writes `[ ]` only when it has parameters.");
                }
            } else {
                parse_parameters(*procedure, closer);
                const Place brackets = span(open.place, previous().place);
                if (procedure->is_main && !procedure->parameters.empty()) {
                    messages_.refuse(brackets, "`proc.main` takes nothing",
                                     "only the program's start runs `proc.main`, so it's never given anything.");
                    procedure->parameters.clear();
                } else if (!square && previous().kind == TokenKind::close_round) {
                    std::string fixed = replaced_line(previous().place, 1, "]");
                    const std::size_t at = open.place.offset - line_start(open.line);
                    fixed.replace(at, 1, "[");
                    messages_.fix(open.place, fixed,
                                  "a definition writes its parameters in `[ ]`; `( )` calls a procedure.");
                }
            }
        }
        if (is_word("returns") && on_line()) {
            advance();
            procedure->returns = parse_type();
            if (is_word("or") && on_line()) {
                messages_.not_built_yet(peek().place, "failures in a header, `fails` (D39), aren't built yet");
                while (on_line() && !is(TokenKind::colon)) {
                    advance();
                }
            }
        } else if (is_word("fails") && on_line()) {
            messages_.not_built_yet(peek().place, "failures in a header, `fails` (D39), aren't built yet");
            while (on_line() && !is(TokenKind::colon)) {
                advance();
            }
        }
        // trace: D8, D55
        if (is(TokenKind::colon) && on_line()) {
            advance();
        } else if (on_line()) {
            expected_here("`:`");
            failed_ = false;
            while (on_line()) {
                advance();
            }
        } else {
            const Place after{.file = file_, .offset = previous().place.end(), .length = 1};
            messages_.fix(after, replaced_line(after, 0, ":"), "a block's opening line ends with `:`.");
        }
        if (on_line()) {
            expected_here("the end of the line");
            failed_ = false;
            while (on_line()) {
                advance();
            }
        }
        (void)header_line;
        Procedure& target = *procedure;
        program_.procedures.push_back(std::move(procedure));
        target.body = parse_body(1, &target);
        parse_procedure_closer(target);
    }

    void skip_to_end_of_procedure() {
        while (!at_end()) {
            if (peek().first_on_line && is_word("end") && is_word("proc", 1)) {
                skip_line();
                return;
            }
            skip_line();
        }
    }

    // trace: D126
    void parse_parameters(Procedure& procedure, TokenKind closer) {
        while (true) {
            if (is(TokenKind::variable)) {
                const Token& name = advance();
                Parameter parameter;
                parameter.name = name.text;
                parameter.place = name.place;
                if (is(TokenKind::colon)) {
                    advance();
                    parameter.type = parse_type();
                } else {
                    expected_here("`:` and the parameter's type");
                    failed_ = false;
                }
                procedure.parameters.push_back(std::move(parameter));
            } else {
                expected_here("a parameter");
                failed_ = false;
                while (on_line() && !is(closer)) {
                    advance();
                }
            }
            if (is(TokenKind::comma) && on_line()) {
                advance();
                continue;
            }
            break;
        }
        if (is(closer) && on_line()) {
            advance();
        } else {
            expected_here(closer == TokenKind::close_square ? "`]`" : "`)`");
            failed_ = false;
            while (on_line() && !is(TokenKind::colon)) {
                advance();
            }
        }
    }

    // trace: D15, D21, D33, D75
    TypeName parse_type() {
        TypeName type;
        if (!is(TokenKind::word) || !on_line()) {
            expected_here("a type");
            failed_ = false;
            return type;
        }
        const Token& word = advance();
        type.written = word.text;
        type.place = word.place;
        if (word.text == "Whole") {
            type.type = Type::whole;
        } else if (word.text == "Bool") {
            type.type = Type::boolean;
        } else if (word.text == "Text") {
            type.type = Type::text;
        } else if (word.text == "List") {
            messages_.not_built_yet(word.place, "lists, `List` (D75), aren't built yet");
            while (on_line() && !is(TokenKind::equals) && !is(TokenKind::colon) && !is(TokenKind::comma) &&
                   !is(TokenKind::close_round) && !is(TokenKind::close_square)) {
                advance();
            }
        } else if (word.text == "Float") {
            messages_.not_built_yet(word.place, "`Float` (D157, D162, D163) isn't built yet");
        } else {
            // trace: D161
            messages_.refuse(word.place, "there's no type named `" + word.text + "`",
                             "a type is one Unio has, like `Whole`, `Float`, `Bool` or `Text`.");
        }
        return type;
    }

    // trace: D8, D85
    void parse_procedure_closer(Procedure& procedure) {
        const std::string closer = procedure.is_main ? std::string("end proc.main") : "end proc " + procedure.name;
        if (at_end()) {
            Message message;
            message.place = file_end();
            message.what = "the file ends before `" + closer + "`";
            message.why = "every block ends with `end` and what it closes.";
            messages_.add(std::move(message));
            return;
        }
        const Token& end_token = peek();
        check_indent(end_token, 0);
        before_closer_.at(end_token.line) = true;
        procedure.end_place = end_token.place;
        check_closer_line(end_token, closer);
    }

    // Reads a closer's line, checking it says what it closes and has no `;` (D8, D85).
    void check_closer_line(const Token& end_token, const std::string& closer) {
        const std::uint32_t line = end_token.line;
        const std::string text = line_text(line);
        const std::size_t start = line_start(line);
        const std::size_t from = end_token.place.offset - start;
        std::size_t content_end = text.size();
        const LineInfo& info = lexed_.lines.at(line - 1);
        if (info.has_comment) {
            content_end = info.comment_offset - start;
        }
        while (content_end > from && text.at(content_end - 1) == ' ') {
            --content_end;
        }
        std::string written = text.substr(from, content_end - from);
        skip_line();
        if (written == closer) {
            return;
        }
        if (written == closer + ";") {
            const Place semicolon{
                .file = file_, .offset = static_cast<std::uint32_t>(start + content_end - 1), .length = 1};
            messages_.fix(semicolon, replaced_line(semicolon, 1, ""), "a closer stands alone, with no `;`.");
            return;
        }
        const Place place{.file = file_,
                          .offset = static_cast<std::uint32_t>(start + from),
                          .length = static_cast<std::uint32_t>(written.size())};
        messages_.fix(place, replaced_line(place, place.length, closer), "a closer is `end` and what it closes.");
    }

    // --- Bodies and statements ------------------------------------------------------------------------------------

    // Reads statements until a line starting with `end` or `else`, or the file's end.
    Body parse_body(int depth, Procedure* procedure) {
        Body body;
        bool first_statement = true;
        while (!at_end()) {
            const Token& first = peek();
            if (is_word("end") || is_word("else")) {
                break;
            }
            if (!first.first_on_line) {
                // trace: D146
                Message message;
                const Place start = first.place;
                skip_statement();
                message.place = span(start, previous().place);
                message.what = "this line has two statements";
                message.why = "each statement goes on a line of its own.";
                messages_.add(std::move(message));
                continue;
            }
            check_indent(first, depth);
            if (is_word("labels")) {
                parse_labels(procedure, first_statement && procedure != nullptr);
                first_statement = false;
                continue;
            }
            first_statement = false;
            failed_ = false;
            continued_ = false;
            if (auto statement = parse_statement(depth)) {
                body.push_back(std::move(statement));
            }
        }
        return body;
    }

    // trace: D25, D89
    void parse_labels(Procedure* procedure, bool allowed) {
        const Token& labels_token = advance();
        if (!allowed) {
            Message message;
            const Place start = labels_token.place;
            while (on_line() && !is(TokenKind::semicolon)) {
                advance();
            }
            message.place = span(start, previous().place);
            message.what = "a `labels` line can only be the first line of a procedure's body";
            message.why = "a procedure declares its labels on the first line of its body.";
            messages_.add(std::move(message));
            skip_statement();
            return;
        }
        while (on_line()) {
            if (!is(TokenKind::variable)) {
                expected_here("a parameter's name");
                skip_statement();
                return;
            }
            const Token& name = advance();
            if (!is_word("as")) {
                expected_here("`as`");
                skip_statement();
                return;
            }
            advance();
            if (!is(TokenKind::word) || !on_line()) {
                expected_here("a label");
                skip_statement();
                return;
            }
            const Token& label = advance();
            labels_.push_back(LabelEntry{.procedure = procedure,
                                         .parameter = name.text,
                                         .parameter_place = name.place,
                                         .label = label.text,
                                         .label_place = label.place});
            if (is(TokenKind::comma) && on_line()) {
                advance();
                continue;
            }
            break;
        }
        if (is(TokenKind::semicolon) && on_line()) {
            advance();
        } else {
            expected_semicolon();
        }
    }

public:
    [[nodiscard]] const auto& labels() const { return labels_; }

    struct LabelEntry {
        Procedure* procedure;
        std::string parameter;
        Place parameter_place;
        std::string label;
        Place label_place;
    };

private:
    std::vector<LabelEntry> labels_;

    std::unique_ptr<Statement> parse_statement(int depth) {
        const Token& first = peek();
        statement_start_ = index_;
        auto statement = std::make_unique<Statement>();
        statement->place = first.place;
        statement->line = first.line;
        if (is_word("let") || is_word("var")) {
            parse_declaration(*statement);
        } else if (is_word("if")) {
            parse_if(*statement, depth);
            return statement;
        } else if (is_word("while")) {
            parse_while(*statement, depth);
            return statement;
        } else if (is_word("break")) {
            statement->kind = StatementKind::leave_loop;
            advance();
        } else if (is_word("return")) {
            statement->kind = StatementKind::give_back;
            advance();
            if (on_line() && !is(TokenKind::semicolon)) {
                statement->value = parse_expression(0);
            }
        } else if (is(TokenKind::variable) && peek(1).kind == TokenKind::equals) {
            statement->kind = StatementKind::assignment;
            const Token& name = advance();
            statement->name = name.text;
            statement->name_place = name.place;
            advance();
            statement->value = parse_expression(0);
        } else if (first.kind == TokenKind::word && (first.text == "otherwise" || first.text == "case" ||
                                                     first.text == "instead" || first.text == "include")) {
            messages_.not_built_yet(first.place, "`otherwise:` blocks (D40, D43) aren't built yet");
            skip_statement();
            return nullptr;
        } else {
            statement->kind = StatementKind::expression;
            statement->value = parse_expression(0);
            if (statement->value && is_word("otherwise") && on_line()) {
                messages_.not_built_yet(peek().place, "`otherwise:` blocks (D40) aren't built yet");
                skip_otherwise(depth);
                return nullptr;
            }
        }
        if (failed_) {
            skip_statement();
            statement->whole_place = span(first.place, previous().place);
            return nullptr;
        }
        if (statement->kind == StatementKind::declaration && statement->value && is_word("otherwise") && on_line()) {
            messages_.not_built_yet(peek().place, "`otherwise:` blocks (D40) aren't built yet");
            skip_otherwise(depth);
            return nullptr;
        }
        statement->whole_place = span(first.place, previous().place);
        if (is(TokenKind::semicolon) && on_line()) {
            advance();
        } else {
            expected_semicolon();
        }
        return statement;
    }

    void skip_otherwise(int depth) {
        (void)depth;
        skip_line();
        while (!at_end()) {
            if (peek().first_on_line && is_word("end") && is_word("otherwise", 1)) {
                skip_line();
                return;
            }
            skip_line();
        }
    }

    // trace: D13, D85, D87, D125
    void parse_declaration(Statement& statement) {
        statement.kind = StatementKind::declaration;
        const Token& keyword = advance();
        statement.changeable = keyword.text == "var";
        statement.keyword_place = keyword.place;
        if (!is(TokenKind::variable) || !on_line()) {
            if (is(TokenKind::word) && on_line() && !is_keyword(peek().text)) {
                const Token& name = peek();
                messages_.fix(name.place, replaced_line(name.place, 0, "$"), "a variable's name starts with `$`.");
            } else {
                expected_here("the variable's name, starting with `$`");
            }
            failed_ = true;
            return;
        }
        const Token& name = advance();
        statement.name = name.text;
        statement.name_place = name.place;
        if (is(TokenKind::colon) && on_line()) {
            advance();
            statement.type = parse_type();
        } else if (is(TokenKind::word) && on_line() && peek(1).kind == TokenKind::equals) {
            const Place gap{.file = file_, .offset = name.place.end(), .length = 1};
            messages_.fix(gap, replaced_line(gap, 0, ":"),
                          "a declaration writes `:` between the variable's name and its type.");
            statement.type = parse_type();
        } else {
            Message message;
            message.place = name.place;
            message.what = "`" + name.text + "` needs its type, written after a `:`";
            message.why = "a declaration always writes the variable's type.";
            messages_.add(std::move(message));
            if (is(TokenKind::equals) && on_line()) {
                advance();
                statement.value = parse_expression(0);
            } else {
                failed_ = true;
            }
            return;
        }
        if (is(TokenKind::equals) && on_line()) {
            advance();
            statement.value = parse_expression(0);
            return;
        }
        if (on_line() && !is(TokenKind::semicolon)) {
            const Place gap{.file = file_, .offset = previous().place.end(), .length = 1};
            messages_.fix(gap, replaced_line(gap, 0, " ="),
                          "a declaration gives its variable a first value after `=`.");
            statement.value = parse_expression(0);
            return;
        }
        messages_.not_designed_yet(span(statement.keyword_place, previous().place),
                                   "declaring a variable without a first value isn't decided yet");
        failed_ = true;
    }

    // trace: D14, D16
    ExpressionPointer parse_condition() {
        condition_ = true;
        auto condition = parse_expression(0);
        condition_ = false;
        return condition;
    }

    void finish_opener() {
        if (failed_) {
            while (on_line() && !is(TokenKind::colon)) {
                advance();
            }
            failed_ = false;
        }
        if (is(TokenKind::colon) && on_line()) {
            advance();
        } else if (on_line()) {
            expected_here("`:`");
            failed_ = false;
            while (on_line()) {
                advance();
            }
            return;
        } else {
            const Place after{.file = file_, .offset = previous().place.end(), .length = 1};
            messages_.fix(after, replaced_line(after, 0, ":"), "a block's opening line ends with `:`.");
        }
        if (on_line()) {
            expected_here("the end of the line");
            failed_ = false;
            while (on_line()) {
                advance();
            }
        }
    }

    void parse_if(Statement& statement, int depth) {
        statement.kind = StatementKind::if_chain;
        Branch first;
        first.place = advance().place;
        first.condition = parse_condition();
        finish_opener();
        first.opener = span(first.place, previous().place);
        first.body = parse_body(depth + 1, nullptr);
        statement.branches.push_back(std::move(first));
        bool seen_else = false;
        while (!at_end() && is_word("else")) {
            const Token& else_token = peek();
            check_indent(else_token, depth);
            before_closer_.at(else_token.line) = true;
            if (seen_else) {
                // trace: D102
                Message message;
                message.place = else_token.place;
                message.what = "nothing can follow an `if`'s `else:` part but `end if`";
                message.why = "an `if` has at most one `else:`, and it comes last.";
                messages_.add(std::move(message));
            }
            Branch branch;
            branch.place = advance().place;
            if (is_word("if") && on_line()) {
                advance();
                failed_ = false;
                branch.condition = parse_condition();
            } else {
                seen_else = true;
            }
            finish_opener();
            branch.body = parse_body(depth + 1, nullptr);
            statement.branches.push_back(std::move(branch));
        }
        parse_block_closer(depth, "end if");
    }

    void parse_while(Statement& statement, int depth) {
        statement.kind = StatementKind::loop;
        advance();
        statement.condition = parse_condition();
        finish_opener();
        statement.body = parse_body(depth + 1, nullptr);
        parse_block_closer(depth, "end while");
    }

    // trace: D8, D85, D87
    void parse_block_closer(int depth, const std::string& closer) {
        if (at_end()) {
            Message message;
            message.place = file_end();
            message.what = "the file ends before `" + closer + "`";
            message.why = "every block ends with `end` and what it closes.";
            messages_.add(std::move(message));
            return;
        }
        const Token& end_token = peek();
        // An `end` for an outer block: this block's closer is missing.
        const LineInfo& info = lexed_.lines.at(end_token.line - 1);
        const bool outer = !info.has_tab && std::cmp_less(info.indent, depth * 4) &&
                           (is_word("proc", 1) || is_word("if", 1) || is_word("while", 1));
        if (outer && sources_.text(peek(1).place) != closer.substr(4)) {
            Message message;
            message.place = end_token.place;
            message.what = "`" + closer + "` is missing before this line";
            message.why = "every block ends with `end` and what it closes.";
            messages_.add(std::move(message));
            return;
        }
        check_indent(end_token, depth);
        before_closer_.at(end_token.line) = true;
        check_closer_line(end_token, closer);
    }

    // --- Expressions ----------------------------------------------------------------------------------------------

    static ExpressionPointer make(ExpressionKind kind, Place place) {
        auto expression = std::make_unique<Expression>();
        expression->kind = kind;
        expression->place = place;
        return expression;
    }

    ExpressionPointer error_expression() {
        failed_ = true;
        return make(ExpressionKind::error, peek().place);
    }

    // Whether the next token continues the expression: on this line, or on a later line starting with an operator.
    bool continues(int power_needed) {
        if (at_end()) {
            return false;
        }
        const int power = infix_power(peek());
        if (power == 0 || power < power_needed) {
            return false;
        }
        if (peek().first_on_line) {
            if (bracket_depth_ == 0 && peek().kind != TokenKind::slide && peek().kind != TokenKind::ampersand) {
                return false;
            }
            continuation_line();
        }
        return true;
    }

    // trace: D34, D72, D73
    ExpressionPointer parse_expression(int minimum) {
        auto left = parse_prefix();
        if (failed_) {
            return left;
        }
        while (true) {
            if (condition_ && is(TokenKind::equals) && on_line()) {
                // trace: D14
                const Token& equals = advance();
                messages_.fix(equals.place, replaced_line(equals.place, 1, "=="),
                              "`=` gives a variable a value, and is never part of a condition; `==` compares.");
                auto right = parse_expression(6);
                auto binary = make(ExpressionKind::binary, span(left->place, right->place));
                binary->op = BinaryOperator::equal;
                binary->op_place = equals.place;
                binary->left = std::move(left);
                binary->right = std::move(right);
                left = std::move(binary);
                left->type = Type::unknown;
                continue;
            }
            if (!continues(minimum + 1)) {
                break;
            }
            const Token& op_token = peek();
            const int power = infix_power(op_token);
            if (op_token.kind == TokenKind::slide) {
                left = parse_slide(std::move(left));
                if (failed_) {
                    return left;
                }
                continue;
            }
            if (op_token.kind == TokenKind::slash || op_token.text == "div" || op_token.text == "rem") {
                messages_.not_built_yet(op_token.place,
                                        "dividing and remainders, `/`, `div` and `rem` (D70, D71), aren't built yet");
                advance();
                if (is(TokenKind::dot)) {
                    advance();
                    advance();
                }
                failed_ = true;
                return left;
            }
            advance();
            BinaryOperator op = BinaryOperator::add;
            switch (op_token.kind) {
            case TokenKind::plus:
                op = BinaryOperator::add;
                break;
            case TokenKind::minus:
                op = BinaryOperator::subtract;
                break;
            case TokenKind::star:
                op = BinaryOperator::multiply;
                break;
            case TokenKind::ampersand:
                op = BinaryOperator::join;
                break;
            case TokenKind::equals_equals:
                op = BinaryOperator::equal;
                break;
            case TokenKind::not_equals:
                op = BinaryOperator::not_equal;
                break;
            case TokenKind::less:
                op = BinaryOperator::less;
                break;
            case TokenKind::greater:
                op = BinaryOperator::greater;
                break;
            default:
                if (op_token.text == "and") {
                    op = BinaryOperator::logical_and;
                } else if (op_token.text == "or") {
                    op = BinaryOperator::logical_or;
                } else if (op_token.text == "at_least") {
                    op = BinaryOperator::at_least;
                } else {
                    op = BinaryOperator::at_most;
                }
                break;
            }
            auto right = parse_expression(power);
            auto binary = make(ExpressionKind::binary, span(left->place, right->place));
            binary->op = op;
            binary->op_place = op_token.place;
            binary->left = std::move(left);
            binary->right = std::move(right);
            if (failed_) {
                return binary;
            }
            left = std::move(binary);
            // trace: D81, D102
            if (is_comparison(op) && on_line() && infix_power(peek()) == 5) {
                refuse_comparison_chain(*left);
                return left;
            }
        }
        return left;
    }

    void refuse_comparison_chain(Expression& first) {
        const Token& second_op = advance();
        auto third = parse_expression(5);
        if (failed_) {
            return;
        }
        const Expression& middle = *first.right;
        const std::string replacement =
            print(first) + " and " + print(middle) + " " + second_op.text + " " + print(*third);
        const Place whole = span(first.place, third->place);
        Message message;
        message.place = second_op.place;
        message.fix = replaced_line(whole, whole.length, replacement);
        message.why = "a comparison sets two values against each other; `and` joins two comparisons, naming the "
                      "middle value twice.";
        messages_.add(std::move(message));
        failed_ = true;
    }

    // trace: D5, D73, D88
    ExpressionPointer parse_slide(ExpressionPointer start) {
        auto slide = make(ExpressionKind::slide, start->place);
        slide->left = std::move(start);
        while (is(TokenKind::slide) && (on_line() || continues(1))) {
            advance();
            if (!is(TokenKind::word)) {
                expected_here("a procedure's name");
                return slide;
            }
            Step step;
            const Token& word = advance();
            step.word = word.text;
            step.word_place = word.place;
            step.place = word.place;
            parse_narrowings(step.narrowings);
            if (is(TokenKind::open_round) && on_line()) {
                const Token& open = peek();
                step.arguments = parse_arguments();
                if (failed_) {
                    return slide;
                }
                step.has_brackets = true;
                step.brackets_place = span(open.place, previous().place);
            }
            step.place = span(word.place, previous().place);
            slide->steps.push_back(std::move(step));
            slide->place = span(slide->left->place, previous().place);
            if (on_line() && infix_power(peek()) > 1) {
                refuse_operator_after_step(*slide);
                return slide;
            }
        }
        return slide;
    }

    // trace: D81, D102, D128
    void refuse_operator_after_step(Expression& slide) {
        const Token& op_token = advance();
        auto right = parse_expression(infix_power(op_token));
        if (failed_) {
            return;
        }
        std::string head;
        if (slide.steps.size() >= 2) {
            head = "(" + print(slide) + ")";
        } else {
            // One step: written as a call (D128).
            Expression call;
            call.kind = ExpressionKind::call;
            call.text = slide.steps.front().word;
            call.narrowings = slide.steps.front().narrowings;
            call.has_brackets = true;
            Argument first_argument;
            first_argument.value = std::make_unique<Expression>(std::move(*slide.left));
            call.arguments.push_back(std::move(first_argument));
            for (Argument& argument : slide.steps.front().arguments) {
                call.arguments.push_back(std::move(argument));
            }
            head = print(call);
        }
        const std::string replacement = head + " " + op_token.text + " " + print(*right);
        const Place whole = span(slide.place, right->place);
        messages_.fix(whole, replaced_line(whole, whole.length, replacement),
                      "a slide's step is a procedure, so an operator after it goes outside brackets around the "
                      "slide.");
        failed_ = true;
    }

    void parse_narrowings(std::vector<Narrowing>& narrowings) {
        while (is(TokenKind::dot) && on_line() && peek(1).kind == TokenKind::word) {
            advance();
            const Token& name = advance();
            narrowings.push_back(Narrowing{.name = name.text, .place = name.place});
        }
        if (is(TokenKind::dot_star) && on_line()) {
            messages_.not_built_yet(peek().place, "handlers, `.*` (D42), aren't built yet");
            failed_ = true;
        }
    }

    // trace: D3, D24
    std::vector<Argument> parse_arguments() {
        std::vector<Argument> arguments;
        advance(); // (
        ++bracket_depth_;
        if (is(TokenKind::close_round)) {
            advance();
            --bracket_depth_;
            return arguments;
        }
        while (true) {
            if (peek().first_on_line) {
                continuation_line();
            }
            Argument argument;
            const Place start = peek().place;
            if (is(TokenKind::word) && peek(1).kind == TokenKind::colon) {
                const Token& label = advance();
                argument.label = label.text;
                argument.label_place = label.place;
                advance();
            }
            argument.value = parse_expression(0);
            argument.place = span(start, previous().place);
            arguments.push_back(std::move(argument));
            if (failed_) {
                --bracket_depth_;
                return arguments;
            }
            if (is(TokenKind::comma)) {
                advance();
                continue;
            }
            break;
        }
        --bracket_depth_;
        if (is(TokenKind::close_round)) {
            advance();
        } else {
            expected_here("`)`");
        }
        return arguments;
    }

    ExpressionPointer parse_prefix() {
        if (peek().first_on_line && bracket_depth_ == 0 && !at_end()) {
            // A value expected at the start of a new line: the statement ended early.
        }
        if (is(TokenKind::minus)) {
            const Token& minus = advance();
            // trace: D27
            if (is(TokenKind::number) && peek().text == "9223372036854775808") {
                const Token& number = advance();
                auto whole = make(ExpressionKind::whole, span(minus.place, number.place));
                whole->whole = std::numeric_limits<std::int64_t>::min();
                return parse_postfix(std::move(whole));
            }
            auto operand = parse_prefix();
            auto negate = make(ExpressionKind::negate, span(minus.place, operand->place));
            negate->op_place = minus.place;
            negate->left = std::move(operand);
            return negate;
        }
        if (is_word("not")) {
            const Token& not_token = advance();
            auto operand = parse_expression(4);
            auto negation = make(ExpressionKind::logical_not, span(not_token.place, operand->place));
            negation->op_place = not_token.place;
            negation->left = std::move(operand);
            return negation;
        }
        return parse_postfix(parse_primary());
    }

    // trace: D23, D81
    ExpressionPointer parse_postfix(ExpressionPointer value) {
        if (failed_) {
            return value;
        }
        if (is(TokenKind::question) && on_line()) {
            messages_.not_built_yet(peek().place, "passing a failure up with `?` (D23) isn't built yet");
            advance();
            failed_ = true;
            return value;
        }
        if (!is(TokenKind::question_question) || !(on_line() || continues(1))) {
            return value;
        }
        const Token& marks = advance();
        auto handled = make(ExpressionKind::handled, value->place);
        handled->op_place = marks.place;
        handled->left = std::move(value);
        if (is_word("ignore")) {
            handled->fallback = Fallback::ignore;
            handled->place = span(handled->place, advance().place);
            return handled;
        }
        if (is_word("crash")) {
            handled->fallback = Fallback::crash;
            handled->place = span(handled->place, advance().place);
            return handled;
        }
        handled->fallback = Fallback::value;
        // The value after `??` is a single value: a literal, a variable, a call, or brackets (D81).
        if (is(TokenKind::minus)) {
            const Token& minus = advance();
            auto operand = parse_postfix(parse_primary());
            auto negate = make(ExpressionKind::negate, span(minus.place, operand->place));
            negate->op_place = minus.place;
            negate->left = std::move(operand);
            handled->right = std::move(negate);
        } else {
            handled->right = parse_postfix(parse_primary());
        }
        handled->place = span(handled->place, handled->right->place);
        return handled;
    }

    ExpressionPointer parse_primary() {
        if (at_end() || (peek().first_on_line && bracket_depth_ == 0 && index_ != statement_start_)) {
            expected_here("a value");
            return error_expression();
        }
        const Token& token = peek();
        switch (token.kind) {
        case TokenKind::number:
            return parse_number();
        case TokenKind::text: {
            advance();
            if (token.unterminated) {
                Message message;
                message.place = Place{.file = file_, .offset = token.place.end(), .length = 1};
                message.what = "the line ends where `\"` should be";
                messages_.add(std::move(message));
                return error_expression();
            }
            auto text = make(ExpressionKind::text, token.place);
            text->text = token.text;
            return text;
        }
        case TokenKind::single_quote: {
            // trace: D21, D85
            advance();
            const std::string fixed = quote_text(token.text);
            messages_.fix(token.place, replaced_line(token.place, token.place.length, fixed),
                          "text is written in double quotes, never single ones like `'hi'`.");
            auto text = make(ExpressionKind::text, token.place);
            text->text = token.text;
            text->type = Type::unknown;
            text->kind = ExpressionKind::error;
            return text;
        }
        case TokenKind::variable: {
            advance();
            auto variable = make(ExpressionKind::variable, token.place);
            variable->text = token.text;
            return variable;
        }
        case TokenKind::open_round: {
            const Token& open = advance();
            ++bracket_depth_;
            auto inner = parse_expression(0);
            --bracket_depth_;
            if (failed_) {
                return inner;
            }
            if (!is(TokenKind::close_round)) {
                expected_here("`)`");
                return error_expression();
            }
            const Token& close = advance();
            auto group = make(ExpressionKind::group, span(open.place, close.place));
            group->left = std::move(inner);
            return group;
        }
        case TokenKind::open_square:
            messages_.not_built_yet(token.place, "lists (D75) aren't built yet");
            return error_expression();
        case TokenKind::word:
            return parse_word();
        default:
            expected_here("a value");
            return error_expression();
        }
    }

    // trace: D15
    ExpressionPointer parse_number() {
        const Token& token = advance();
        if (!std::ranges::all_of(token.text, [](char character) { return character >= '0' && character <= '9'; })) {
            messages_.not_designed_yet(token.place,
                                       "how whole numbers are written, as in `" + token.text + "`, isn't decided yet");
            return error_expression();
        }
        std::int64_t value = 0;
        const auto [end, error] = std::from_chars(token.text.data(), token.text.data() + token.text.size(), value);
        (void)end;
        if (error != std::errc{}) {
            messages_.not_designed_yet(token.place, "a whole number past a `Whole`'s range, as in `" + token.text +
                                                        "`, isn't decided yet");
            return error_expression();
        }
        auto whole = make(ExpressionKind::whole, token.place);
        whole->whole = value;
        return whole;
    }

    ExpressionPointer parse_word() {
        const Token& word = peek();
        if (word.text == "true" || word.text == "false") {
            advance();
            auto boolean = make(ExpressionKind::boolean, word.place);
            boolean->boolean = word.text == "true";
            return boolean;
        }
        if (is_keyword(word.text) && word.text != "be") {
            expected_here("a value");
            return error_expression();
        }
        advance();
        auto call = make(ExpressionKind::call, word.place);
        call->text = word.text;
        call->word_place = word.place;
        parse_narrowings(call->narrowings);
        if (failed_) {
            return call;
        }
        if (is(TokenKind::double_colon) && on_line()) {
            messages_.not_built_yet(peek().place, "packages, `::` (D64), aren't built yet");
            return error_expression();
        }
        if (is(TokenKind::open_round) && on_line()) {
            call->arguments = parse_arguments();
            call->has_brackets = true;
        }
        call->place = span(word.place, previous().place);
        return call;
    }

    const Sources& sources_;
    std::uint32_t file_;
    const Lexed& lexed_;
    const std::vector<Token>& tokens_;
    Program& program_;
    Messages& messages_;
    std::size_t index_ = 0;
    std::size_t statement_start_ = 0;
    bool failed_ = false;
    bool condition_ = false;
    bool continued_ = false;
    int bracket_depth_ = 0;
    std::vector<int> expected_indent_;
    std::vector<bool> before_closer_;

    friend void unio::parse(const Sources&, std::uint32_t, const Lexed&, Program&, Messages&);
};

} // namespace

void parse(const Sources& sources, std::uint32_t file, const Lexed& lexed, Program& program, Messages& messages) {
    Parser parser(sources, file, lexed, program, messages);
    parser.run();
    // Labels lines become their parameters' labels; the checker refuses any that don't fit (D25, D89).
    for (const auto& entry : parser.labels()) {
        Procedure& procedure = *entry.procedure;
        auto parameter = std::ranges::find_if(
            procedure.parameters, [&](const Parameter& candidate) { return candidate.name == entry.parameter; });
        if (parameter == procedure.parameters.end()) {
            Message message;
            message.place = entry.parameter_place;
            message.what = "`" + procedure.name + "` has no parameter named `" + entry.parameter + "`";
            message.why = "a `labels` line gives labels to the procedure's own parameters.";
            messages.add(std::move(message));
            continue;
        }
        if (parameter->label) {
            Message message;
            message.place = entry.parameter_place;
            message.what = "`" + entry.parameter + "` already has a label";
            message.why = "each parameter has at most one label.";
            messages.add(std::move(message));
            continue;
        }
        const bool label_taken = std::ranges::any_of(
            procedure.parameters, [&](const Parameter& other) { return other.label && *other.label == entry.label; });
        if (label_taken) {
            Message message;
            message.place = entry.label_place;
            message.what = "two parameters have the label `" + entry.label + "`";
            message.why = "each label names one parameter, so a call can say which value is which.";
            messages.add(std::move(message));
            continue;
        }
        parameter->label = entry.label;
        parameter->label_place = entry.label_place;
    }
}

} // namespace unio
