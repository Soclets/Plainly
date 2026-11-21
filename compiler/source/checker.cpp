// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception

#include "checker.h"

#include "parser.h"

#include <algorithm>
#include <map>
#include <ranges>
#include <set>
#include <string_view>

namespace unio {

namespace {

struct ParameterInfo {
    Type type = Type::unknown;
    std::optional<std::string> label;
};

// What a call needs to know about the procedure it calls, from the program or the library.
struct Signature {
    std::string name; // the full name, as `be.Text.fmt`
    std::vector<ParameterInfo> parameters;
    Type returns = Type::nothing;
    bool can_fail = false;
    const Procedure* procedure = nullptr;
    bool formats = false; // `.fmt`: one piece of text, with braces filled in
};

// trace: D2, D99, D125
std::optional<std::string> bad_name(std::string_view name, std::string_view shown) {
    const std::string quoted = "`" + std::string(shown) + "`";
    if (name.empty()) {
        return quoted + " has no name after its `$`, but names start with a letter";
    }
    if (std::ranges::any_of(name, [](char character) { return character >= 'A' && character <= 'Z'; })) {
        return quoted + " has a capital letter, but names have none";
    }
    if (name.front() >= '0' && name.front() <= '9') {
        return quoted + " starts with a digit, but names start with a letter";
    }
    if (name.front() == '_') {
        return quoted + " starts with `_`, but names start with a letter";
    }
    if (name.back() == '_') {
        return quoted + " ends with `_`, but `_` only goes between words";
    }
    if (name.find("__") != std::string_view::npos) {
        return quoted + " has two `_` in a row, but `_` only goes between words";
    }
    return std::nullopt;
}

constexpr const char* bad_name_why =
    "a name is lowercase letters and digits, starting with a letter, with `_` only between words.";

bool is_keyword_name(std::string_view word) {
    static const std::set<std::string_view> keywords = {
        "proc",      "end",  "let",     "var",     "if",      "else",   "at_least", "at_most", "while",
        "break",     "true", "false",   "and",     "or",      "not",    "be",       "ignore",  "crash",
        "otherwise", "case", "instead", "labels",  "as",      "extern", "returns",  "return",  "fam",
        "div",       "rem",  "handler", "include", "comment", "fails",
    };
    return keywords.contains(word);
}

std::string quoted_type(Type type) {
    return std::string("`") + type_name(type) + "`";
}

class Checker {
public:
    Checker(const Sources& sources, Program& program, Messages& messages)
        : sources_(sources), program_(program), messages_(messages) {}

    void run() {
        collect_procedures();
        for (auto& procedure : program_.procedures) {
            check_procedure(*procedure);
        }
    }

private:
    // --- Helpers --------------------------------------------------------------------------------------------------

    [[nodiscard]] std::uint32_t line_of(Place place) const { return sources_.line_column(place).line; }

    [[nodiscard]] std::string replaced_line(Place place, std::uint32_t length, std::string_view replacement) const {
        const std::uint32_t line = line_of(place);
        std::string text(sources_.line_text(place.file, line));
        const std::size_t column = place.offset - sources_.file(place.file).line_starts.at(line - 1);
        text.replace(column, std::min<std::size_t>(length, text.size() - column), replacement);
        return text;
    }

    [[nodiscard]] bool one_line(Place place) const {
        const auto start = sources_.line_column(place);
        const auto end = sources_.line_column(
            Place{.file = place.file, .offset = place.end() == 0 ? 0 : place.end() - 1, .length = 0});
        return start.line == end.line;
    }

    void refuse(Place place, std::string what, std::string why) {
        messages_.refuse(place, std::move(what), std::move(why));
    }

    void refuse_because(Place place, std::string what, std::string because, std::optional<Place> cause,
                        std::string why) {
        Message message;
        message.place = place;
        message.what = std::move(what);
        message.because = Because{.sentence = std::move(because), .place = cause};
        message.why = std::move(why);
        messages_.add(std::move(message));
    }

    void fix(Place place, std::uint32_t length, const std::string& replacement, std::string why) {
        if (!one_line(place)) {
            messages_.not_designed_yet(place, "how a statement's later lines are indented isn't decided yet");
            return;
        }
        messages_.fix(place, replaced_line(place, length, replacement), std::move(why));
    }

    // --- Procedures -----------------------------------------------------------------------------------------------

    // trace: D4, D64, D92, D94, D99, D102, D132
    void collect_procedures() {
        const Procedure* main = nullptr;
        std::uint32_t index = 0;
        for (auto& procedure : program_.procedures) {
            procedure->index = index++;
            if (procedure->is_main) {
                if (main != nullptr) {
                    refuse_because(procedure->place, "this program has two `proc.main`s", "the first is here",
                                   main->place, "a program has exactly one `proc.main`, where it starts.");
                } else {
                    main = procedure.get();
                }
                continue;
            }
            if (is_keyword_name(procedure->name)) {
                refuse(procedure->place, "`" + procedure->name + "` is a keyword, so it can't be a procedure's name",
                       "a procedure's name can't be one of Unio's own words.");
            } else if (const auto problem = bad_name(procedure->name, procedure->name)) {
                refuse(procedure->place, *problem, bad_name_why);
            }
            if (library(procedure->name)) {
                messages_.not_designed_yet(procedure->place, "a procedure with the name of one of the library's, `" +
                                                                 procedure->name + "`, isn't decided yet");
                continue;
            }
            const auto [existing, added] = procedures_.try_emplace(procedure->name, procedure.get());
            if (!added) {
                refuse_because(procedure->place, "there's already a procedure named `" + procedure->name + "`",
                               "the first is here", existing->second->place,
                               "every procedure in a program needs its own name, since a call has to know which "
                               "one it means.");
            }
        }
        if (main == nullptr && sources_.size() > 0) {
            Message message;
            message.place = Place{.file = 0, .offset = 0, .length = 1};
            message.what = "this program has no `proc.main`";
            message.why = "a program starts with `proc.main`.";
            messages_.add(std::move(message));
        }
    }

    static bool library(std::string_view word) {
        return word == "show_msg" || word == "ask" || word == "random" || word == "be";
    }

    struct Scope {
        std::map<std::string, Variable*> names;
    };

    void check_procedure(Procedure& procedure) {
        procedure_ = &procedure;
        scopes_.clear();
        ended_.clear();
        loop_depth_ = 0;
        scopes_.emplace_back();
        std::map<std::string, Place> seen;
        for (Parameter& parameter : procedure.parameters) {
            if (const auto problem = bad_name(parameter.name.substr(1), parameter.name)) {
                refuse(parameter.place, *problem, bad_name_why);
            }
            if (seen.contains(parameter.name)) {
                refuse(parameter.place, "two parameters are named `" + parameter.name + "`",
                       "each parameter needs its own name.");
                continue;
            }
            seen.emplace(parameter.name, parameter.place);
            auto variable = std::make_unique<Variable>();
            variable->name = parameter.name;
            variable->place = parameter.place;
            variable->type = parameter.type.type;
            variable->parameter = true;
            variable->index = static_cast<std::uint32_t>(procedure.variables.size());
            scopes_.back().names.emplace(parameter.name, variable.get());
            procedure.variables.push_back(std::move(variable));
        }
        for (Parameter& parameter : procedure.parameters) {
            if (parameter.label) {
                if (const auto problem = bad_name(*parameter.label, *parameter.label)) {
                    refuse(parameter.label_place, *problem, bad_name_why);
                }
            }
        }
        check_body(procedure.body);
        // trace: D57, D88, D149
        if (procedure.returns && procedure.returns->type != Type::unknown && !ends_in_return(procedure.body)) {
            refuse(procedure.place,
                   "some way through `" + procedure.name + "` reaches its end without `return` giving back a " +
                       quoted_type(procedure.returns->type),
                   "every way through a procedure with `returns` ends in `return` and a value.");
        }
        if (!procedure.body.empty()) {
            const Statement& last = *procedure.body.back();
            if (last.kind == StatementKind::give_back && !last.value) {
                refuse(last.whole_place, "a bare `return;` as a procedure's last line does nothing",
                       "a procedure ends there anyway; `return;` is for leaving early.");
            }
        }
        scopes_.clear();
    }

    static bool ends_in_return(const Body& body) {
        if (body.empty()) {
            return false;
        }
        const Statement& last = *body.back();
        if (last.kind == StatementKind::give_back) {
            return last.value != nullptr;
        }
        if (last.kind == StatementKind::if_chain) {
            const bool has_else = !last.branches.back().condition;
            return has_else &&
                   std::ranges::all_of(last.branches, [](const Branch& branch) { return ends_in_return(branch.body); });
        }
        return false;
    }

    // Whether running a statement can leave its block by `return` or `break` (D9). A `break` inside a loop that's
    // inside the statement only leaves that loop.
    static bool can_leave(const Statement& statement, bool inside_loop) {
        switch (statement.kind) {
        case StatementKind::give_back:
            return true;
        case StatementKind::leave_loop:
            return !inside_loop;
        case StatementKind::if_chain:
            return std::ranges::any_of(statement.branches, [&](const Branch& branch) {
                return std::ranges::any_of(branch.body,
                                           [&](const auto& inner) { return can_leave(*inner, inside_loop); });
            });
        case StatementKind::loop:
            return std::ranges::any_of(statement.body, [](const auto& inner) { return can_leave(*inner, true); });
        default:
            return false;
        }
    }

    // --- Statements -----------------------------------------------------------------------------------------------

    void check_body(Body& body) {
        scopes_.emplace_back();
        const Statement* left_by = nullptr;
        for (auto& statement : body) {
            if (left_by != nullptr) {
                refuse_after_leaving(*statement, *left_by);
                left_by = nullptr;
            }
            check_statement(*statement);
            if (statement->kind == StatementKind::give_back || statement->kind == StatementKind::leave_loop ||
                (statement->kind == StatementKind::if_chain && can_leave(*statement, false))) {
                left_by = statement.get();
            }
        }
        for (const auto& [name, variable] : scopes_.back().names) {
            ended_[name] = variable;
        }
        scopes_.pop_back();
    }

    // trace: D9, D57, D88, D117
    void refuse_after_leaving(const Statement& statement, const Statement& leaving) {
        const Place line = statement.whole_place.length > 0 ? statement.whole_place : statement.place;
        if (leaving.kind == StatementKind::if_chain) {
            refuse_because(line, "this line follows an `if` that can leave",
                           "the `if` on line " + std::to_string(line_of(leaving.place)) + " can leave",
                           leaving.branches.front().opener,
                           "when any way through an `if` leaves, nothing may follow its `end if`; code that should "
                           "run afterwards goes in the ways that don't leave.");
            return;
        }
        const std::string word = leaving.kind == StatementKind::give_back ? "return" : "break";
        refuse_because(line, "this line comes after a `" + word + "` in the same block, so it can never run",
                       "the `" + word + "` is here", leaving.whole_place,
                       "nothing may follow `" + word + "` in the same block.");
    }

    Variable* find(const std::string& name) {
        for (auto& scope : std::views::reverse(scopes_)) {
            if (const auto found = scope.names.find(name); found != scope.names.end()) {
                return found->second;
            }
        }
        return nullptr;
    }

    // trace: D92, D93, D102
    void unknown_variable(const std::string& name, Place place) {
        const std::string what = "there's no variable named `" + name + "` here";
        const std::string why = "a variable is declared with `let` or `var` before it's used.";
        if (const auto ended = ended_.find(name); ended != ended_.end()) {
            refuse_because(place, what, "`" + name + "` is declared inside a block that has ended",
                           ended->second->place, why);
            return;
        }
        refuse(place, what, why);
    }

    void check_statement(Statement& statement) {
        switch (statement.kind) {
        case StatementKind::declaration:
            check_declaration(statement);
            break;
        case StatementKind::assignment:
            check_assignment(statement);
            break;
        case StatementKind::expression:
            check_expression_statement(statement);
            break;
        case StatementKind::if_chain:
            for (Branch& branch : statement.branches) {
                if (branch.condition) {
                    check_condition(*branch.condition);
                }
                check_body(branch.body);
            }
            break;
        case StatementKind::loop:
            check_condition(*statement.condition);
            ++loop_depth_;
            check_body(statement.body);
            --loop_depth_;
            break;
        case StatementKind::leave_loop:
            // trace: D32, D88
            if (loop_depth_ == 0) {
                refuse(statement.whole_place, "this `break` isn't inside a loop",
                       "`break` leaves the innermost loop it's in.");
            }
            break;
        case StatementKind::give_back:
            check_return(statement);
            break;
        }
    }

    // trace: D13, D55, D94, D99, D102, D125
    void check_declaration(Statement& statement) {
        if (const auto problem = bad_name(statement.name.substr(1), statement.name)) {
            refuse(statement.name_place, *problem, bad_name_why);
        }
        if (statement.value) {
            check_value(*statement.value);
            expect_type(*statement.value, statement.type.type, [&] {
                return "`" + statement.name + "` is a " + quoted_type(statement.type.type) + ", but this value is a " +
                       quoted_type(statement.value->type);
            });
        }
        if (Variable* existing = find(statement.name)) {
            refuse_because(statement.name_place, "`" + statement.name + "` is already in use here",
                           "`" + statement.name + "` is declared here", existing->place,
                           "a name in use can't be declared again; two things get two names.");
        }
        auto variable = std::make_unique<Variable>();
        variable->name = statement.name;
        variable->place = statement.name_place;
        variable->keyword_place = statement.keyword_place;
        variable->type = statement.type.type;
        variable->changeable = statement.changeable;
        variable->index = static_cast<std::uint32_t>(procedure_->variables.size());
        statement.variable = variable.get();
        scopes_.back().names[statement.name] = variable.get();
        ended_.erase(statement.name);
        procedure_->variables.push_back(std::move(variable));
    }

    // trace: D14, D85, D87, D147
    void check_assignment(Statement& statement) {
        Variable* variable = find(statement.name);
        if (statement.value) {
            check_value(*statement.value);
        }
        if (variable == nullptr) {
            unknown_variable(statement.name, statement.name_place);
            return;
        }
        statement.variable = variable;
        if (variable->parameter) {
            refuse_because(statement.name_place, "`" + statement.name + "` can't be given a new value",
                           "`" + statement.name + "` is a parameter", variable->place,
                           "a parameter can't be given a new value; to change a copy, declare a `var` from it.");
            return;
        }
        if (!variable->changeable) {
            refuse_because(statement.name_place, "`" + statement.name + "` can't be given a new value",
                           "line " + std::to_string(line_of(variable->keyword_place)) + " declares it with `let`",
                           variable->keyword_place, "only a `var` can be given a new value.");
            return;
        }
        if (statement.value) {
            expect_type(*statement.value, variable->type, [&] {
                return "`" + statement.name + "` is a " + quoted_type(variable->type) + ", but this value is a " +
                       quoted_type(statement.value->type);
            });
        }
    }

    // trace: D38, D57, D88
    void check_return(Statement& statement) {
        const Procedure& procedure = *procedure_;
        if (statement.value) {
            check_value(*statement.value);
            if (!procedure.returns) {
                const std::string name = procedure.is_main ? "proc.main" : procedure.name;
                refuse(statement.whole_place, "`" + name + "` has no `returns`, so it gives nothing back",
                       "only a procedure with `returns` gives back a value.");
                return;
            }
            expect_type(*statement.value, procedure.returns->type, [&] {
                return "`" + procedure.name + "` gives back a " + quoted_type(procedure.returns->type) +
                       ", but this value is a " + quoted_type(statement.value->type);
            });
            return;
        }
        if (procedure.returns && procedure.returns->type != Type::unknown) {
            refuse(statement.whole_place,
                   "`" + procedure.name + "` gives back a " + quoted_type(procedure.returns->type) +
                       ", so `return` needs one",
                   "in a procedure with `returns`, `return` gives back a value of that type.");
        }
    }

    // trace: D120
    void check_expression_statement(Statement& statement) {
        Expression& value = *statement.value;
        check_expression(value);
        if (value.type == Type::unknown || value.type == Type::nothing) {
            return;
        }
        if (value.kind == ExpressionKind::call || value.kind == ExpressionKind::slide ||
            value.kind == ExpressionKind::handled) {
            const Expression* call = &value;
            if (value.kind == ExpressionKind::handled) {
                call = value.left.get();
            }
            std::string name;
            Place place = call->place;
            if (call->kind == ExpressionKind::slide && !call->steps.empty()) {
                name = call->steps.back().builtin.empty() ? call->steps.back().word : call->steps.back().builtin;
                place = call->steps.back().place;
            } else {
                name = call->builtin.empty() ? call->text : call->builtin;
            }
            refuse(place, "`" + name + "` gives back a " + quoted_type(value.type) + ", but nothing uses it",
                   "a call that gives back a value is used: kept in a variable, passed on, or compared.");
            return;
        }
        messages_.not_designed_yet(value.place, "a value standing alone as a statement isn't decided yet");
    }

    void check_condition(Expression& condition) {
        check_value(condition);
        if (condition.type != Type::boolean && condition.type != Type::unknown) {
            messages_.not_designed_yet(condition.place, "a condition that isn't a `Bool`, as this " +
                                                            quoted_type(condition.type) + " is, isn't decided yet");
        }
    }

    // --- Expressions ----------------------------------------------------------------------------------------------

    template <typename What> void expect_type(const Expression& value, Type wanted, What what) {
        if (value.type == Type::unknown || wanted == Type::unknown || value.type == wanted) {
            return;
        }
        if (value.type == Type::nothing) {
            return; // refused already by check_value
        }
        refuse(value.place, what(), "a value's type has to match where it's used.");
    }

    // An expression used as a value: a call that gives nothing can't be one (D38, D88).
    void check_value(Expression& expression) {
        check_expression(expression);
        if (expression.type == Type::nothing) {
            refuse(expression.place, "this call gives nothing back, so it can't be used as a value",
                   "a procedure with no `returns` gives nothing back.");
            expression.type = Type::unknown;
        }
        if (expression.can_fail) {
            // trace: D23, D87
            refuse(expression.place, "this call can fail, and nothing here deals with the failure",
                   "every call that can fail deals with its failure, with `?`, `??` or `otherwise:`.");
            expression.can_fail = false;
        }
    }

    void check_expression(Expression& expression) {
        switch (expression.kind) {
        case ExpressionKind::whole:
            expression.type = Type::whole;
            break;
        case ExpressionKind::boolean:
            expression.type = Type::boolean;
            break;
        case ExpressionKind::text:
            expression.type = Type::text;
            break;
        case ExpressionKind::variable:
            check_variable(expression);
            break;
        case ExpressionKind::call:
            check_call(expression);
            break;
        case ExpressionKind::negate:
            check_negate(expression);
            break;
        case ExpressionKind::logical_not:
            check_value(*expression.left);
            expression.type = Type::boolean;
            if (expression.left->type != Type::boolean && expression.left->type != Type::unknown) {
                refuse(expression.left->place,
                       "`not` works on a `Bool`, but this is a " + quoted_type(expression.left->type),
                       "a value's type has to match where it's used.");
            }
            break;
        case ExpressionKind::binary:
            check_binary(expression);
            break;
        case ExpressionKind::slide:
            check_slide(expression);
            break;
        case ExpressionKind::handled:
            check_handled(expression);
            break;
        case ExpressionKind::group:
            check_expression(*expression.left);
            expression.type = expression.left->type;
            expression.can_fail = expression.left->can_fail;
            break;
        case ExpressionKind::error:
            expression.type = Type::unknown;
            break;
        }
    }

    void check_variable(Expression& expression) {
        if (const auto problem = bad_name(expression.text.substr(1), expression.text)) {
            refuse(expression.place, *problem, bad_name_why);
            expression.type = Type::unknown;
            return;
        }
        Variable* variable = find(expression.text);
        if (variable == nullptr) {
            unknown_variable(expression.text, expression.place);
            expression.type = Type::unknown;
            return;
        }
        expression.variable = variable;
        expression.type = variable->type;
    }

    // trace: D27, D89, D142
    void check_negate(Expression& expression) {
        check_value(*expression.left);
        expression.type = Type::whole;
        const Type operand = expression.left->type;
        if (operand != Type::whole && operand != Type::unknown) {
            refuse(expression.place, "`-` makes a number negative, but this is a " + quoted_type(operand),
                   "a `-` with no space after it makes a number negative.");
            expression.type = Type::unknown;
        }
    }

    static bool is_unbracketed_maths(const Expression& expression) {
        return expression.kind == ExpressionKind::binary && is_arithmetic(expression.op);
    }
    static bool is_unbracketed_join(const Expression& expression) {
        return expression.kind == ExpressionKind::binary && expression.op == BinaryOperator::join;
    }

    void check_binary(Expression& expression) {
        Expression& left = *expression.left;
        Expression& right = *expression.right;
        // trace: D72, D89
        if (expression.op == BinaryOperator::join || is_arithmetic(expression.op)) {
            const Expression* maths = nullptr;
            if (expression.op == BinaryOperator::join) {
                maths = is_unbracketed_maths(left) ? &left : is_unbracketed_maths(right) ? &right : nullptr;
            } else if (is_unbracketed_join(left) || is_unbracketed_join(right)) {
                maths = &expression;
            }
            if (maths != nullptr) {
                const Place join_place = expression.op == BinaryOperator::join
                                             ? expression.op_place
                                             : (is_unbracketed_join(left) ? left.op_place : right.op_place);
                refuse_because(join_place, "`&` is next to maths without brackets, so it isn't clear which goes first",
                               "the maths is here", maths->op_place,
                               "where maths doesn't say which goes first, `( )` shows it.");
                check_expression(left);
                check_expression(right);
                expression.type = Type::unknown;
                return;
            }
        }
        if (expression.op == BinaryOperator::logical_and || expression.op == BinaryOperator::logical_or) {
            check_value(left);
            check_value(right);
            expression.type = Type::boolean;
            for (const Expression* side : {&left, &right}) {
                if (side->type != Type::boolean && side->type != Type::unknown) {
                    refuse(side->place,
                           std::string("`") + operator_spelling(expression.op) + "` joins two `Bool`s, but this is a " +
                               quoted_type(side->type),
                           "a value's type has to match where it's used.");
                }
            }
            return;
        }
        check_value(left);
        check_value(right);
        const Type left_type = left.type;
        const Type right_type = right.type;
        const bool known = left_type != Type::unknown && right_type != Type::unknown;
        if (is_arithmetic(expression.op)) {
            expression.type = Type::whole;
            if (!known || (left_type == Type::whole && right_type == Type::whole)) {
                return;
            }
            expression.type = Type::unknown;
            // trace: D27, D28, D87
            if (expression.op == BinaryOperator::add && left_type == Type::text && right_type == Type::text) {
                fix(expression.op_place, 1, "&", "`+` is only for numbers, and `&` joins text.");
                return;
            }
            refuse(expression.op_place,
                   std::string("`") + operator_spelling(expression.op) + "` works on two `Whole`s, but this is a " +
                       quoted_type(left_type) + " and a " + quoted_type(right_type),
                   "`+`, `-` and `*` are only for numbers.");
            return;
        }
        if (expression.op == BinaryOperator::join) {
            expression.type = Type::text;
            if (!known || (left_type == Type::text && right_type == Type::text)) {
                return;
            }
            expression.type = Type::unknown;
            // trace: D28, D87, D89
            for (const Expression* side : {&left, &right}) {
                if (side->type == Type::boolean) {
                    refuse(side->place, "`&` joins text, but this is a `Bool`",
                           "`&` joins two pieces of text, and anything else becomes text only where `be.Text` says "
                           "so.");
                    return;
                }
            }
            const Expression& number = left_type == Type::whole ? left : right;
            fix(number.place, number.place.length, "be.Text(" + print(number) + ")",
                "`&` joins two pieces of text, and a number becomes text only where `be.Text` says so.");
            return;
        }
        // A comparison (D17).
        expression.type = Type::boolean;
        if (!known) {
            return;
        }
        if (left_type != right_type) {
            // trace: D87, D89
            refuse(expression.op_place,
                   std::string("`") + operator_spelling(expression.op) +
                       "` compares two values of one type, but this is a " + quoted_type(left_type) + " and a " +
                       quoted_type(right_type),
                   "a comparison is between two values of the same type.");
            return;
        }
        if (left_type == Type::text) {
            messages_.not_designed_yet(expression.op_place, "comparing two pieces of text isn't decided yet");
            return;
        }
        if (left_type == Type::boolean && expression.op != BinaryOperator::equal &&
            expression.op != BinaryOperator::not_equal) {
            messages_.not_designed_yet(expression.op_place, std::string("comparing two `Bool`s with `") +
                                                                operator_spelling(expression.op) +
                                                                "` isn't decided yet");
        }
    }

    // trace: D23, D81, D89, D102, D121
    void check_handled(Expression& expression) {
        Expression& call = *expression.left;
        check_expression(call);
        if (!call.can_fail) {
            if (call.type != Type::unknown) {
                refuse(expression.op_place, "`??` here doesn't follow a call that can fail",
                       "`??` deals with the failure of the call right before it.");
            }
            if (expression.right) {
                check_value(*expression.right);
            }
            expression.type = Type::unknown;
            return;
        }
        expression.type = call.type;
        if (expression.fallback == Fallback::ignore) {
            if (call.type != Type::nothing) {
                refuse(expression.place,
                       "`" + call.builtin + "` gives back a " + quoted_type(call.type) +
                           ", so `?? ignore` leaves nothing to carry on with if it fails",
                       "`?? ignore` is for a call that gives nothing; after one that gives a value, `?? value` says "
                       "what to carry on with.");
                expression.type = Type::unknown;
            }
            return;
        }
        if (expression.fallback == Fallback::crash) {
            return; // trace: D152, D156
        }
        check_value(*expression.right);
        const Type fallback = expression.right->type;
        if (fallback != Type::unknown && call.type != Type::unknown && fallback != call.type) {
            refuse(expression.right->place,
                   "the call gives a " + quoted_type(call.type) + ", so the value after `??` must be one too, not a " +
                       quoted_type(fallback),
                   "a value's type has to match where it's used.");
        }
    }

    // --- Calls and slides -----------------------------------------------------------------------------------------

    // trace: D18, D22, D31, D87, D89, D102, D103
    std::optional<Signature> resolve(const std::string& word, Place word_place,
                                     const std::vector<Narrowing>& narrowings) {
        auto unknown_narrowing = [&](const Narrowing& narrowing, const std::string& owner, const std::string& why) {
            refuse(narrowing.place, "`" + owner + "` has no narrowing `." + narrowing.name + "`", why);
        };
        auto twice = [&](const std::vector<Narrowing>& list) -> bool {
            for (std::size_t index = 0; index < list.size(); ++index) {
                for (std::size_t later = index + 1; later < list.size(); ++later) {
                    if (list.at(index).name == list.at(later).name) {
                        refuse(list.at(later).place, "`." + list.at(later).name + "` is written twice",
                               "each narrowing is written once.");
                        return true;
                    }
                }
            }
            return false;
        };
        if (word == "show_msg") {
            bool formats = false;
            bool same_line = false;
            for (const Narrowing& narrowing : narrowings) {
                if (narrowing.name == "fmt") {
                    formats = true;
                } else if (narrowing.name == "same_line") {
                    same_line = true;
                } else {
                    unknown_narrowing(narrowing, "show_msg", "`show_msg`'s narrowings are `.fmt` and `.same_line`.");
                    return std::nullopt;
                }
            }
            if (twice(narrowings)) {
                return std::nullopt;
            }
            if (formats && same_line && narrowings.front().name == "same_line") {
                const Place place = span(word_place, narrowings.back().place);
                fix(place, place.length, "show_msg.fmt.same_line",
                    "narrowings go strongest first: `.fmt` changes what's shown, `.same_line` only where it ends.");
                return std::nullopt;
            }
            Signature signature;
            signature.name = std::string("show_msg") + (formats ? ".fmt" : "") + (same_line ? ".same_line" : "");
            signature.parameters = {ParameterInfo{.type = Type::text, .label = std::nullopt}};
            signature.formats = formats;
            return signature;
        }
        if (word == "ask" || word == "random") {
            if (!narrowings.empty()) {
                unknown_narrowing(narrowings.front(), word, "`" + word + "` has no narrowings.");
                return std::nullopt;
            }
            Signature signature;
            signature.name = word;
            if (word == "ask") {
                // trace: D20
                signature.parameters = {ParameterInfo{.type = Type::text, .label = std::nullopt}};
                signature.returns = Type::text;
            } else {
                // trace: D24, D26
                signature.parameters = {ParameterInfo{.type = Type::whole, .label = "from"},
                                        ParameterInfo{.type = Type::whole, .label = "to"}};
                signature.returns = Type::whole;
            }
            return signature;
        }
        if (word == "be") {
            if (narrowings.empty()) {
                refuse(word_place, "`be` needs the type the value becomes, as in `be.Text`",
                       "a value is converted with `be` narrowed by the type it becomes.");
                return std::nullopt;
            }
            const Narrowing& type = narrowings.front();
            if (type.name == "Bool") {
                messages_.not_designed_yet(type.place, "converting to a `Bool`, `be.Bool`, isn't decided yet");
                return std::nullopt;
            }
            if (type.name != "Text" && type.name != "Whole") {
                unknown_narrowing(type, "be", "`be`'s narrowings are `.Text` and `.Whole`.");
                return std::nullopt;
            }
            const std::string base = "be." + type.name;
            const std::vector<Narrowing> rest(narrowings.begin() + 1, narrowings.end());
            Signature signature;
            if (type.name == "Whole") {
                if (!rest.empty()) {
                    unknown_narrowing(rest.front(), base, "`be.Whole` can't be narrowed further.");
                    return std::nullopt;
                }
                // trace: D22, D95
                signature.name = base;
                signature.parameters = {ParameterInfo{.type = Type::text, .label = std::nullopt}};
                signature.returns = Type::whole;
                signature.can_fail = true;
                return signature;
            }
            for (const Narrowing& narrowing : rest) {
                if (narrowing.name != "fmt") {
                    unknown_narrowing(narrowing, base, "`be.Text`'s only narrowing is `.fmt`.");
                    return std::nullopt;
                }
            }
            if (twice(rest)) {
                return std::nullopt;
            }
            signature.name = rest.empty() ? base : base + ".fmt";
            signature.formats = !rest.empty();
            signature.parameters = {
                ParameterInfo{.type = rest.empty() ? Type::unknown : Type::text, .label = std::nullopt}};
            signature.returns = Type::text;
            return signature;
        }
        const auto found = procedures_.find(word);
        if (found == procedures_.end()) {
            refuse(word_place, "there's no procedure named `" + word + "`",
                   "a call names a procedure the program or the library has.");
            return std::nullopt;
        }
        if (!narrowings.empty()) {
            unknown_narrowing(narrowings.front(), word, "`" + word + "` has no narrowings.");
            return std::nullopt;
        }
        const Procedure& procedure = *found->second;
        Signature signature;
        signature.name = word;
        signature.procedure = &procedure;
        signature.returns = procedure.returns ? procedure.returns->type : Type::nothing;
        for (const Parameter& parameter : procedure.parameters) {
            signature.parameters.push_back(ParameterInfo{.type = parameter.type.type, .label = parameter.label});
        }
        return signature;
    }

    // Checks arguments against a signature. `first_slid` is the type of a slid first value, when there is one.
    // trace: D5, D24, D88, D89, D92
    bool check_arguments(const Signature& signature, std::vector<Argument>& arguments, Place call_place,
                         std::optional<Type> first_slid) {
        for (Argument& argument : arguments) {
            check_value(*argument.value);
        }
        const std::size_t offset = first_slid ? 1 : 0;
        const std::size_t given = arguments.size() + offset;
        if (signature.formats) {
            return true; // `.fmt`'s own checks follow
        }
        if (given != signature.parameters.size()) {
            const auto plural = [](std::size_t count) {
                return std::to_string(count) + (count == 1 ? " value" : " values");
            };
            const std::string what = "`" + signature.name + "` takes " + plural(signature.parameters.size()) +
                                     ", but this call gives " + plural(given);
            const std::string why = "a call gives every parameter a value, with its label if it has one.";
            if (signature.procedure != nullptr) {
                refuse_because(call_place, what, "`" + signature.name + "` is declared here",
                               signature.procedure->place, why);
            } else {
                refuse(call_place, what, why);
            }
            return false;
        }
        bool fine = true;
        if (first_slid && signature.parameters.front().type != Type::unknown && *first_slid != Type::unknown &&
            *first_slid != signature.parameters.front().type) {
            refuse(call_place,
                   "`" + signature.name + "` takes a " + quoted_type(signature.parameters.front().type) +
                       " first, but the slid value is a " + quoted_type(*first_slid),
                   "a slide passes the value on its left in as the step's first argument.");
            fine = false;
        }
        for (std::size_t index = 0; index < arguments.size(); ++index) {
            Argument& argument = arguments.at(index);
            const ParameterInfo& parameter = signature.parameters.at(index + offset);
            if (parameter.label && !argument.label) {
                fix(argument.place, 0, *parameter.label + ": ", "every call writes the labels its procedure chose.");
                fine = false;
            } else if (!parameter.label && argument.label) {
                const std::uint32_t written = argument.value->place.offset - argument.label_place.offset;
                Message message;
                message.place = argument.label_place;
                message.fix = replaced_line(argument.label_place, written, "");
                message.why = "a call writes only the labels its procedure chose.";
                messages_.add(std::move(message));
                fine = false;
            } else if (parameter.label && argument.label && *parameter.label != *argument.label) {
                refuse(argument.label_place,
                       "this argument's label is `" + *parameter.label + "`, not `" + *argument.label + "`",
                       "every call writes the labels its procedure chose.");
                fine = false;
            }
            const Type type = argument.value->type;
            if (parameter.type != Type::unknown && type != Type::unknown && type != Type::nothing &&
                type != parameter.type) {
                refuse(argument.value->place,
                       "`" + signature.name + "` takes a " + quoted_type(parameter.type) + " here, but this is a " +
                           quoted_type(type),
                       "a value's type has to match where it's used.");
                fine = false;
            }
        }
        return fine;
    }

    // trace: D22, D87, D153
    void check_conversion(const Signature& signature, const Expression& value, std::string& builtin) {
        if (signature.name == "be.Text" && value.type == Type::text) {
            refuse(value.place, "this value is already a `Text`, so `be.Text` does nothing",
                   "converting a value to the type it already is does nothing, so it's refused.");
        } else if (signature.name == "be.Whole" && value.type == Type::whole) {
            refuse(value.place, "this value is already a `Whole`, so `be.Whole` does nothing",
                   "converting a value to the type it already is does nothing, so it's refused.");
        }
        builtin = signature.name;
    }

    void check_call(Expression& expression) {
        if (!expression.has_brackets) {
            // trace: D4, D92
            const bool known = library(expression.text) || procedures_.contains(expression.text);
            if (known) {
                refuse(expression.place,
                       "`" + expression.text + "` is a procedure, so using it writes `( )` with its arguments",
                       "a call writes the procedure's name and `( )`.");
            } else {
                refuse(expression.place, "there's no procedure named `" + expression.text + "`",
                       "a call names a procedure the program or the library has.");
            }
            expression.type = Type::unknown;
            return;
        }
        check_chain(expression);
        const auto signature = resolve(expression.text, expression.word_place, expression.narrowings);
        if (!signature) {
            for (Argument& argument : expression.arguments) {
                check_expression(*argument.value);
            }
            expression.type = Type::unknown;
            return;
        }
        expression.procedure = signature->procedure;
        expression.builtin = signature->procedure == nullptr ? signature->name : "";
        expression.type = signature->returns;
        expression.can_fail = signature->can_fail;
        if (signature->formats) {
            check_format(expression, *signature);
            return;
        }
        const bool fine = check_arguments(*signature, expression.arguments, expression.place, std::nullopt);
        if (fine && signature->name.starts_with("be.") && !expression.arguments.empty()) {
            check_conversion(*signature, *expression.arguments.front().value, expression.builtin);
            if (signature->name == "be.Text" && expression.arguments.front().value->type == Type::unknown) {
                expression.type = Type::text;
            }
        }
    }

    // trace: D29, D30, D92, D133
    void check_format(Expression& expression, const Signature& signature) {
        expression.type = signature.returns;
        if (expression.arguments.size() != 1 || expression.arguments.front().value->kind != ExpressionKind::text) {
            for (Argument& argument : expression.arguments) {
                check_expression(*argument.value);
            }
            if (expression.arguments.size() == 1 && expression.arguments.front().label) {
                refuse(expression.arguments.front().label_place, "`.fmt`'s text has no label",
                       "`.fmt` fills in the braces of the text it's given.");
                return;
            }
            if (expression.arguments.size() == 1 && expression.arguments.front().value->type == Type::unknown) {
                return;
            }
            refuse(expression.place, "`.fmt` takes one piece of text",
                   "`.fmt` fills in the braces of the text it's given.");
            return;
        }
        const Argument& argument = expression.arguments.front();
        if (argument.label) {
            refuse(argument.label_place, "`.fmt`'s text has no label",
                   "`.fmt` fills in the braces of the text it's given.");
            return;
        }
        // Read the literal as written, so each brace's place is known.
        const Place literal = argument.value->place;
        const std::string_view raw = sources_.text(literal);
        std::string plain;
        std::size_t index = 1;
        while (index + 1 < raw.size()) {
            const char character = raw.at(index);
            if (character == '\\' && index + 2 < raw.size()) {
                plain.push_back(raw.at(index + 1));
                index += 2;
                continue;
            }
            if (character != '{') {
                plain.push_back(character);
                ++index;
                continue;
            }
            const std::size_t close = raw.find('}', index + 1);
            if (close == std::string_view::npos || close >= raw.size() - 1) {
                messages_.not_designed_yet(Place{.file = literal.file,
                                                 .offset = literal.offset + static_cast<std::uint32_t>(index),
                                                 .length = 1},
                                           "a `{` with no `}` after it in `.fmt` text isn't decided yet");
                expression.type = Type::unknown;
                return;
            }
            const std::string_view inside = raw.substr(index + 1, close - index - 1);
            const Place inside_place{.file = literal.file,
                                     .offset = literal.offset + static_cast<std::uint32_t>(index + 1),
                                     .length = static_cast<std::uint32_t>(inside.size())};
            const Variable* variable = format_variable(inside, inside_place, expression);
            if (variable == nullptr) {
                expression.type = Type::unknown;
                return;
            }
            if (!plain.empty()) {
                expression.pieces.push_back(Expression::Piece{.text = plain, .variable = nullptr});
                plain.clear();
            }
            expression.pieces.push_back(Expression::Piece{.text = "", .variable = variable});
            index = close + 1;
        }
        if (!plain.empty() || expression.pieces.empty()) {
            expression.pieces.push_back(Expression::Piece{.text = plain, .variable = nullptr});
        }
        expression.builtin = signature.name;
    }

    const Variable* format_variable(std::string_view inside, Place place, const Expression& call) {
        const bool plain_name = !inside.empty() && std::ranges::all_of(inside, [](char character) {
            return (character >= 'a' && character <= 'z') || (character >= 'A' && character <= 'Z') ||
                   (character >= '0' && character <= '9') || character == '_';
        });
        if (inside.starts_with('$') && std::ranges::all_of(inside.substr(1), [](char character) {
                return (character >= 'a' && character <= 'z') || (character >= 'A' && character <= 'Z') ||
                       (character >= '0' && character <= '9') || character == '_';
            })) {
            const std::string name(inside);
            Variable* variable = find(name);
            if (variable == nullptr) {
                refuse(place, "there's no variable named `" + name + "` here",
                       "braces in `.fmt` text hold a variable's name, checked at compile time.");
                return nullptr;
            }
            if (variable->type == Type::boolean) {
                messages_.not_designed_yet(place, "a `Bool` in `.fmt` braces isn't decided yet");
                return nullptr;
            }
            return variable;
        }
        if (plain_name) {
            const std::string with_dollar = "$" + std::string(inside);
            if (find(with_dollar) != nullptr) {
                fix(place, 0, "$", "braces in `.fmt` text hold a variable's name, with its `$`.");
            } else {
                refuse(place, "there's no variable named `" + with_dollar + "` here",
                       "braces in `.fmt` text hold a variable's name, checked at compile time.");
            }
            return nullptr;
        }
        (void)call;
        refuse(place, "braces in `.fmt` text hold only a variable's name",
               "text never hides a calculation or a call; work it out into a variable first.");
        return nullptr;
    }

    // trace: D5, D128, D142
    static std::size_t chain_length(const Expression& expression) {
        if (expression.kind != ExpressionKind::call || !expression.has_brackets) {
            return 0;
        }
        if (!expression.arguments.empty() && !expression.arguments.front().label) {
            return 1 + chain_length(*expression.arguments.front().value);
        }
        return 1;
    }

    // The innermost call of a chain, and the calls around it from the inside out.
    static void chain_calls(const Expression& expression, std::vector<const Expression*>& calls) {
        if (chain_length(expression) == 0) {
            return;
        }
        if (!expression.arguments.empty() && !expression.arguments.front().label &&
            chain_length(*expression.arguments.front().value) > 0) {
            chain_calls(*expression.arguments.front().value, calls);
        }
        calls.push_back(&expression);
    }

    static std::string step_text(const Expression& call, bool skip_first) {
        std::string out = call.text;
        for (const Narrowing& narrowing : call.narrowings) {
            out += "." + narrowing.name;
        }
        const std::size_t from = skip_first ? 1 : 0;
        if (call.arguments.size() > from) {
            out += '(';
            for (std::size_t index = from; index < call.arguments.size(); ++index) {
                if (index > from) {
                    out += ", ";
                }
                const Argument& argument = call.arguments.at(index);
                if (argument.label) {
                    out += *argument.label + ": ";
                }
                out += print(*argument.value);
            }
            out += ')';
        }
        return out;
    }

    // A chain of three or more calls is written as one slide (D128).
    std::string as_slide(const std::vector<const Expression*>& calls) {
        const Expression& innermost = *calls.front();
        std::string out;
        std::size_t first_step = 0;
        if (innermost.arguments.size() == 1 && !innermost.arguments.front().label) {
            out = print(*innermost.arguments.front().value);
        } else {
            out = print(innermost);
            first_step = 1;
        }
        for (std::size_t index = first_step; index < calls.size(); ++index) {
            out += " |> " + step_text(*calls.at(index), true);
        }
        return out;
    }

    void mark_inner(const Expression& expression) {
        const Expression* inner = &expression;
        while (chain_length(*inner) > 0 && !inner->arguments.empty() && !inner->arguments.front().label) {
            inner = inner->arguments.front().value.get();
            chain_inner_.insert(inner);
        }
    }

    void check_chain(const Expression& expression) {
        if (chain_inner_.contains(&expression)) {
            return;
        }
        mark_inner(expression);
        if (chain_length(expression) >= 3) {
            std::vector<const Expression*> calls;
            chain_calls(expression, calls);
            fix(expression.place, expression.place.length, as_slide(calls),
                "one or two calls are written as calls; a chain of three or more is a slide.");
        }
    }

    // trace: D5, D88
    void refuse_empty_steps(const Expression& slide) {
        for (const Step& step : slide.steps) {
            if (step.has_brackets && step.arguments.empty()) {
                fix(step.brackets_place, step.brackets_place.length, "",
                    "a step with nothing else to pass is written bare, since the slide gives it its value.");
            }
        }
    }

    void check_slide(Expression& expression) {
        Expression& start = *expression.left;
        const std::size_t start_calls = chain_length(start);
        const std::size_t total = start_calls + expression.steps.size();
        if (start_calls >= 2) {
            // A chain mixing calls and a slide is written as one slide (D128).
            std::vector<const Expression*> calls;
            chain_calls(start, calls);
            std::string out = as_slide(calls);
            for (const Step& step : expression.steps) {
                out += " |> " + step.word;
                for (const Narrowing& narrowing : step.narrowings) {
                    out += "." + narrowing.name;
                }
                if (!step.arguments.empty()) {
                    out += '(';
                    for (std::size_t index = 0; index < step.arguments.size(); ++index) {
                        if (index > 0) {
                            out += ", ";
                        }
                        if (const auto& label = step.arguments.at(index).label) {
                            out += *label + ": ";
                        }
                        out += print(*step.arguments.at(index).value);
                    }
                    out += ')';
                }
            }
            fix(expression.place, expression.place.length, out,
                "one or two calls are written as calls; a chain of three or more is a slide.");
            expression.type = Type::unknown;
            return;
        }
        if (total < 3) {
            // One or two calls are written as calls.
            std::string out = print(start);
            for (const Step& step : expression.steps) {
                std::string call = step.word;
                for (const Narrowing& narrowing : step.narrowings) {
                    call += "." + narrowing.name;
                }
                call += "(" + out;
                for (const Argument& argument : step.arguments) {
                    call += ", ";
                    if (argument.label) {
                        call += *argument.label + ": ";
                    }
                    call += print(*argument.value);
                }
                out = call + ")";
            }
            fix(expression.place, expression.place.length, out,
                "one or two calls are written as calls; a chain of three or more is a slide.");
            refuse_empty_steps(expression);
            expression.type = Type::unknown;
            return;
        }
        chain_inner_.insert(&start);
        mark_inner(start);
        check_value(start);
        Type slid = start.type;
        for (Step& step : expression.steps) {
            if (step.has_brackets && step.arguments.empty()) {
                // trace: D5, D88
                fix(step.brackets_place, step.brackets_place.length, "",
                    "a step with nothing else to pass is written bare, since the slide gives it its value.");
                expression.type = Type::unknown;
                return;
            }
            const auto signature = resolve(step.word, step.word_place, step.narrowings);
            if (!signature) {
                expression.type = Type::unknown;
                return;
            }
            if (slid == Type::nothing) {
                refuse(step.place, "this call gives nothing back, so it can't be used as a value",
                       "a procedure with no `returns` gives nothing back.");
                expression.type = Type::unknown;
                return;
            }
            if (signature->formats) {
                refuse(step.place, "`.fmt` takes one piece of text",
                       "`.fmt` fills in the braces of the text it's given.");
                expression.type = Type::unknown;
                return;
            }
            if (signature->can_fail) {
                refuse(step.place, "this call can fail, and nothing here deals with the failure",
                       "every call that can fail deals with its failure, with `?`, `??` or `otherwise:`.");
                expression.type = Type::unknown;
                return;
            }
            if (!check_arguments(*signature, step.arguments, step.place, slid)) {
                expression.type = Type::unknown;
                return;
            }
            if (signature->name == "be.Text" && slid == Type::text) {
                refuse(step.place, "this value is already a `Text`, so `be.Text` does nothing",
                       "converting a value to the type it already is does nothing, so it's refused.");
            }
            step.procedure = signature->procedure;
            step.builtin = signature->procedure == nullptr ? signature->name : "";
            step.type = signature->returns;
            slid = signature->returns;
        }
        expression.type = slid;
    }

    const Sources& sources_;
    Program& program_;
    Messages& messages_;
    std::map<std::string, Procedure*> procedures_;
    Procedure* procedure_ = nullptr;
    std::vector<Scope> scopes_;
    std::map<std::string, Variable*> ended_;
    int loop_depth_ = 0;
    std::set<const Expression*> chain_inner_;
};

} // namespace

void check(const Sources& sources, Program& program, Messages& messages) {
    Checker(sources, program, messages).run();
}

} // namespace unio
