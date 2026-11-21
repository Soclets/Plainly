// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception

#include "ir.h"

namespace unio::ir {

ValueId Function::add_value(Value value) {
    values.push_back(std::move(value));
    return static_cast<ValueId>(values.size() - 1);
}

BlockId Function::add_block() {
    blocks.emplace_back();
    return static_cast<BlockId>(blocks.size() - 1);
}

ValueId Function::append(BlockId block, Value value) {
    value.block = block;
    const ValueId id = add_value(std::move(value));
    blocks.at(block).instructions.push_back(id);
    return id;
}

bool is_checked(Op op) {
    return op == Op::checked_add || op == Op::checked_subtract || op == Op::checked_multiply ||
           op == Op::checked_negate;
}

// Whether a value does anything beyond giving its result: output, input, randomness, owning memory, or calling a
// procedure that might. A failed check stops the program, but that's not a side effect for merging: the same check
// with the same values fails the same way (D124 #6).
bool has_side_effects(const Function& function, const Module& module, const Value& value) {
    (void)function;
    switch (value.op) {
    case Op::show:
    case Op::ask:
    case Op::random:
    case Op::retain:
    case Op::release:
    case Op::join:
    case Op::whole_to_text:
        return true;
    case Op::call:
        return !module.functions.at(value.callee).no_side_effects;
    default:
        return false;
    }
}

const char* op_name(Op op) {
    switch (op) {
    case Op::parameter:
        return "parameter";
    case Op::constant_whole:
        return "whole";
    case Op::constant_bool:
        return "bool";
    case Op::constant_text:
        return "text";
    case Op::add:
        return "add";
    case Op::subtract:
        return "subtract";
    case Op::multiply:
        return "multiply";
    case Op::negate:
        return "negate";
    case Op::checked_add:
        return "checked_add";
    case Op::checked_subtract:
        return "checked_subtract";
    case Op::checked_multiply:
        return "checked_multiply";
    case Op::checked_negate:
        return "checked_negate";
    case Op::equal:
        return "equal";
    case Op::not_equal:
        return "not_equal";
    case Op::less:
        return "less";
    case Op::greater:
        return "greater";
    case Op::at_least:
        return "at_least";
    case Op::at_most:
        return "at_most";
    case Op::logical_not:
        return "not";
    case Op::call:
        return "call";
    case Op::show:
        return "show";
    case Op::ask:
        return "ask";
    case Op::random:
        return "random";
    case Op::whole_to_text:
        return "whole_to_text";
    case Op::join:
        return "join";
    case Op::parse_whole:
        return "parse_whole";
    case Op::failed:
        return "failed";
    case Op::value_of:
        return "value_of";
    case Op::retain:
        return "retain";
    case Op::release:
        return "release";
    }
    return "?";
}

namespace {

void print_target(std::string& out, const Target& target) {
    out += "b" + std::to_string(target.block);
    if (!target.arguments.empty()) {
        out += "(";
        for (std::size_t index = 0; index < target.arguments.size(); ++index) {
            out += (index > 0 ? ", %" : "%") + std::to_string(target.arguments.at(index));
        }
        out += ")";
    }
}

} // namespace

std::string print(const Function& function, const Module& module) {
    std::string out = "procedure " + function.name;
    if (function.no_side_effects) {
        out += " [no side effects]";
    }
    out += "\n";
    for (std::size_t block_index = 0; block_index < function.blocks.size(); ++block_index) {
        const Block& block = function.blocks.at(block_index);
        if (block.dead) {
            continue;
        }
        out += "b" + std::to_string(block_index) + "(";
        for (std::size_t index = 0; index < block.parameters.size(); ++index) {
            out += (index > 0 ? ", %" : "%") + std::to_string(block.parameters.at(index));
        }
        out += "):\n";
        for (const ValueId id : block.instructions) {
            const Value& value = function.values.at(id);
            out += "  %" + std::to_string(id) + " = " + op_name(value.op);
            if (value.op == Op::constant_whole || value.op == Op::constant_bool || value.op == Op::show) {
                out += " " + std::to_string(value.immediate);
            } else if (value.op == Op::constant_text) {
                out += " \"" + module.texts.at(static_cast<std::size_t>(value.immediate)) + "\"";
            } else if (value.op == Op::call) {
                out += " " + module.functions.at(value.callee).name;
            }
            for (const ValueId operand : value.operands) {
                const Value& used = function.values.at(operand);
                if (used.block == no_block && used.op == Op::constant_whole) {
                    out += " " + std::to_string(used.immediate);
                } else if (used.block == no_block && used.op == Op::constant_bool) {
                    out += used.immediate != 0 ? " true" : " false";
                } else {
                    out += " %" + std::to_string(operand);
                }
            }
            out += "\n";
        }
        const Terminator& end = block.end;
        switch (end.kind) {
        case TerminatorKind::jump:
            out += "  jump ";
            print_target(out, end.then_target);
            break;
        case TerminatorKind::branch:
            out += "  branch %" + std::to_string(end.condition) + ", ";
            print_target(out, end.then_target);
            out += ", ";
            print_target(out, end.else_target);
            break;
        case TerminatorKind::give_back:
            out += "  return";
            if (end.value != no_value) {
                out += " %" + std::to_string(end.value);
            }
            break;
        case TerminatorKind::unset:
            out += "  (no terminator)";
            break;
        }
        out += "\n";
    }
    return out;
}

std::string print(const Module& module) {
    std::string out;
    for (const Function& function : module.functions) {
        out += print(function, module);
    }
    return out;
}

std::vector<std::vector<BlockId>> predecessors(const Function& function) {
    std::vector<std::vector<BlockId>> result(function.blocks.size());
    for (std::size_t index = 0; index < function.blocks.size(); ++index) {
        const Block& block = function.blocks.at(index);
        if (block.dead) {
            continue;
        }
        const auto id = static_cast<BlockId>(index);
        if (block.end.kind == TerminatorKind::jump) {
            result.at(block.end.then_target.block).push_back(id);
        } else if (block.end.kind == TerminatorKind::branch) {
            result.at(block.end.then_target.block).push_back(id);
            result.at(block.end.else_target.block).push_back(id);
        }
    }
    return result;
}

std::vector<std::uint32_t> use_counts(const Function& function) {
    std::vector<std::uint32_t> counts(function.values.size(), 0);
    for (const Block& block : function.blocks) {
        if (block.dead) {
            continue;
        }
        for (const ValueId id : block.instructions) {
            for (const ValueId operand : function.values.at(id).operands) {
                ++counts.at(operand);
            }
        }
        const Terminator& end = block.end;
        if (end.condition != no_value) {
            ++counts.at(end.condition);
        }
        if (end.value != no_value) {
            ++counts.at(end.value);
        }
        for (const ValueId argument : end.then_target.arguments) {
            ++counts.at(argument);
        }
        for (const ValueId argument : end.else_target.arguments) {
            ++counts.at(argument);
        }
    }
    return counts;
}

} // namespace unio::ir
