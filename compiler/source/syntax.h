// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
// The syntax tree: the program as written (D62). The checker fills in each expression's type and what each name
// means, which makes it the typed tree.

#pragma once

#include "source.h"

#include <cstdint>
#include <memory>
#include <optional>
#include <string>
#include <vector>

namespace unio {

// trace: D15, D21, D33
enum class Type : std::uint8_t { unknown, whole, boolean, text, nothing };

const char* type_name(Type type);

struct Expression;
using ExpressionPointer = std::unique_ptr<Expression>;

struct Narrowing {
    std::string name;
    Place place; // the name, without its dot
};

struct Argument {
    std::optional<std::string> label;
    Place label_place;
    ExpressionPointer value;
    Place place; // the whole argument, label included
};

enum class ExpressionKind : std::uint8_t {
    whole,
    boolean,
    text,
    variable,
    call,
    negate,
    logical_not,
    binary,
    slide,
    handled, // `call ?? value`, `?? ignore`, `?? crash`
    group,   // ( expression )
    error,   // what couldn't be read; its type is unknown (D55)
};

enum class BinaryOperator : std::uint8_t {
    add,
    subtract,
    multiply,
    join,
    equal,
    not_equal,
    less,
    greater,
    at_least,
    at_most,
    logical_and,
    logical_or,
};

const char* operator_spelling(BinaryOperator op);
bool is_comparison(BinaryOperator op);
bool is_arithmetic(BinaryOperator op);

enum class Fallback : std::uint8_t { value, ignore, crash };

struct Procedure;
struct Variable;

// One procedure in a slide (D5): a word, its narrowings, and the arguments after the slid one.
struct Step {
    std::string word;
    Place word_place;
    std::vector<Narrowing> narrowings;
    std::vector<Argument> arguments;
    bool has_brackets = false;
    Place brackets_place;
    Place place;
    // Filled in by the checker.
    const Procedure* procedure = nullptr;
    std::string builtin; // a library procedure's full name, as `be.Text`
    Type type = Type::unknown;
};

struct Expression {
    ExpressionKind kind = ExpressionKind::error;
    Place place;
    Type type = Type::unknown;

    std::int64_t whole = 0;
    bool boolean = false;
    std::string text; // a text literal's meaning; a variable's name with its `$`; a call's word

    // A call: its word, narrowings and arguments. A slide: `left` and `steps`.
    Place word_place;
    std::vector<Narrowing> narrowings;
    std::vector<Argument> arguments;
    bool has_brackets = false;
    std::vector<Step> steps;

    BinaryOperator op = BinaryOperator::add;
    Place op_place;
    ExpressionPointer left;
    ExpressionPointer right;
    Fallback fallback = Fallback::value;

    // Filled in by the checker.
    const Procedure* procedure = nullptr;
    std::string builtin;
    const Variable* variable = nullptr;
    bool can_fail = false;
    // `.fmt` text split into plain pieces and variables, in order.
    struct Piece {
        std::string text;
        const Variable* variable = nullptr;
    };
    std::vector<Piece> pieces;
};

struct Statement;
using Body = std::vector<std::unique_ptr<Statement>>;

enum class StatementKind : std::uint8_t {
    declaration,
    assignment,
    expression,
    if_chain,
    loop,
    leave_loop, // break
    give_back,  // return
};

struct Branch {
    ExpressionPointer condition; // none for `else:`
    Body body;
    Place place;  // the opening line's first word
    Place opener; // the whole opening line, from its first word to its `:`
};

struct TypeName {
    Type type = Type::unknown;
    std::string written;
    Place place;
};

// A variable: a declaration's or a parameter's (D125, D147).
struct Variable {
    std::string name; // with its `$`
    Place place;      // the name where it's declared
    Place keyword_place;
    Type type = Type::unknown;
    bool changeable = false; // `var`
    bool parameter = false;
    std::uint32_t index = 0; // within its procedure
};

struct Statement {
    StatementKind kind = StatementKind::expression;
    Place place; // the statement's first token
    std::uint32_t line = 0;
    Place whole_place; // the whole statement, for messages
    // A declaration: `let` or `var`.
    bool changeable = false;
    Place keyword_place;
    std::string name;
    Place name_place;
    TypeName type;
    ExpressionPointer value; // a declaration's, an assignment's, a return's or an expression statement's
    // `if`, `else if`, `else`.
    std::vector<Branch> branches;
    // `while`.
    ExpressionPointer condition;
    Body body;
    // Filled in by the checker.
    Variable* variable = nullptr;
    std::vector<const Variable*> scope_end; // variables whose block ends after this statement (for Text's frees)
};

struct Parameter {
    std::string name; // with its `$`
    Place place;
    TypeName type;
    std::optional<std::string> label;
    Place label_place;
};

struct Procedure {
    std::string name; // `main` for proc.main
    bool is_main = false;
    Place place; // the name in the header
    Place header_place;
    std::vector<Parameter> parameters;
    std::optional<TypeName> returns;
    Body body;
    Place end_place;
    // Filled in by the checker.
    std::vector<std::unique_ptr<Variable>> variables;
    std::uint32_t index = 0;
};

struct Program {
    std::vector<std::unique_ptr<Procedure>> procedures;
};

} // namespace unio
