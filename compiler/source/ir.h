// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
// Unio IR (D62): the one intermediate form of Unio's own, where every check, allocation and label is written out, and
// where `final` mode's own optimizations run (D124). It's in SSA form (static single assignment: each value is made
// once), with block parameters in place of phi nodes, as Swift's SIL has.

#pragma once

#include <cstdint>
#include <limits>
#include <string>
#include <vector>

namespace unio::ir {

using ValueId = std::uint32_t;
using BlockId = std::uint32_t;
constexpr ValueId no_value = std::numeric_limits<ValueId>::max();
constexpr BlockId no_block = std::numeric_limits<BlockId>::max();

enum class Kind : std::uint8_t { none, whole, boolean, text, whole_or_failure };

enum class Op : std::uint8_t {
    parameter, // a block parameter
    constant_whole,
    constant_bool,
    constant_text, // immediate: index into Module::texts
    // Arithmetic. The checked forms stop the program at their site when the result doesn't fit (D46); `final` turns
    // one into its plain form only where it's proved unable to fail.
    add,
    subtract,
    multiply,
    negate,
    checked_add,
    checked_subtract,
    checked_multiply,
    checked_negate,
    equal,
    not_equal,
    less,
    greater,
    at_least,
    at_most,
    logical_not,
    call,          // callee: index into Module::functions
    show,          // text; immediate 1 leaves the line open (D6)
    ask,           // text -> text (D20)
    random,        // whole, whole -> whole (D26)
    whole_to_text, // be.Text (D22)
    join,          // & (D28)
    parse_whole,   // be.Whole (D95): -> whole_or_failure
    failed,        // whole_or_failure -> bool
    value_of,      // whole_or_failure -> whole
    retain,        // a second owner for a text (D130)
    release,       // an owner of a text ends (D130)
    choose,        // bool, a, b -> a when true, b when false
};

// What a check reports when it fails at runtime: its place, shown as D54 lays out.
enum class SiteKind : std::uint8_t { add, subtract, multiply, negate, memory, random_order };

struct Site {
    SiteKind kind = SiteKind::add;
    std::string file;
    std::uint32_t line = 0;
    std::uint32_t column = 0;
    std::uint32_t length = 0;
    std::string source_line;
};

struct Value {
    Op op = Op::parameter;
    Kind kind = Kind::none;
    std::vector<ValueId> operands;
    std::int64_t immediate = 0;
    std::uint32_t site = 0;   // for checked arithmetic and allocations
    std::uint32_t callee = 0; // for calls
    BlockId block = no_block;
};

enum class TerminatorKind : std::uint8_t { jump, branch, give_back, unset };

struct Target {
    BlockId block = no_block;
    std::vector<ValueId> arguments;
};

struct Terminator {
    TerminatorKind kind = TerminatorKind::unset;
    ValueId condition = no_value; // branch
    Target then_target;           // jump, or branch when true
    Target else_target;           // branch when false
    ValueId value = no_value;     // give_back, when the function returns a value
};

struct Block {
    std::vector<ValueId> parameters;
    std::vector<ValueId> instructions;
    Terminator end;
    bool dead = false;
};

struct Function {
    std::string name; // the Unio name, as `fib`, or `main` for proc.main
    std::string symbol;
    bool is_main = false;
    std::vector<Kind> parameter_kinds;
    Kind returns = Kind::none;
    std::vector<Value> values;
    std::vector<Block> blocks; // blocks[0] is the entry; its parameters are the function's
    // Facts `final`'s passes work out (D124 #6, #19).
    bool no_side_effects = false;

    ValueId add_value(Value value);
    BlockId add_block();
    ValueId append(BlockId block, Value value);
};

struct Module {
    std::vector<Function> functions;
    std::vector<Site> sites;
    std::vector<std::string> texts;
};

bool is_checked(Op op);
bool has_side_effects(const Function& function, const Module& module, const Value& value);
const char* op_name(Op op);
std::string print(const Function& function, const Module& module);
std::string print(const Module& module);

// Blocks that jump to each block, in order.
std::vector<std::vector<BlockId>> predecessors(const Function& function);
// Every value's uses, counted.
std::vector<std::uint32_t> use_counts(const Function& function);

} // namespace unio::ir
