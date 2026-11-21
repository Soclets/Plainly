// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception

#include "optimize.h"

#include <algorithm>
#include <limits>
#include <map>
#include <optional>
#include <set>
#include <tuple>
#include <unordered_map>

namespace unio {

namespace {

using ir::Block;
using ir::BlockId;
using ir::Function;
using ir::Kind;
using ir::Module;
using ir::Op;
using ir::Target;
using ir::TerminatorKind;
using ir::Value;
using ir::ValueId;

constexpr std::int64_t lowest = std::numeric_limits<std::int64_t>::min();
constexpr std::int64_t highest = std::numeric_limits<std::int64_t>::max();

// --- Ranges ---------------------------------------------------------------------------------------------------------

// The values a `Whole` can have at some point: every number from `low` to `high`.
struct Range {
    std::int64_t low = lowest;
    std::int64_t high = highest;

    [[nodiscard]] bool exact() const { return low == high; }
};

Range intersect(Range left, Range right) {
    return Range{std::max(left.low, right.low), std::min(left.high, right.high)};
}
Range hull(Range left, Range right) {
    return Range{std::min(left.low, right.low), std::max(left.high, right.high)};
}

// The range of a calculation's exact result, or nothing when it might not fit in a `Whole`.
std::optional<Range> add_ranges(Range left, Range right) {
    std::int64_t low = 0;
    std::int64_t high = 0;
    if (__builtin_add_overflow(left.low, right.low, &low) || __builtin_add_overflow(left.high, right.high, &high)) {
        return std::nullopt;
    }
    return Range{low, high};
}

std::optional<Range> subtract_ranges(Range left, Range right) {
    std::int64_t low = 0;
    std::int64_t high = 0;
    if (__builtin_sub_overflow(left.low, right.high, &low) || __builtin_sub_overflow(left.high, right.low, &high)) {
        return std::nullopt;
    }
    return Range{low, high};
}

std::optional<Range> multiply_ranges(Range left, Range right) {
    std::int64_t low = highest;
    std::int64_t high = lowest;
    for (const std::int64_t first : {left.low, left.high}) {
        for (const std::int64_t second : {right.low, right.high}) {
            std::int64_t product = 0;
            if (__builtin_mul_overflow(first, second, &product)) {
                return std::nullopt;
            }
            low = std::min(low, product);
            high = std::max(high, product);
        }
    }
    return Range{low, high};
}

std::optional<Range> negate_range(Range range) {
    if (range.low == lowest) {
        return std::nullopt;
    }
    return Range{-range.high, -range.low};
}

// --- The control-flow graph and dominators --------------------------------------------------------------------------

std::vector<BlockId> successors(const Block& block) {
    switch (block.end.kind) {
    case TerminatorKind::jump:
        return {block.end.then_target.block};
    case TerminatorKind::branch:
        return {block.end.then_target.block, block.end.else_target.block};
    default:
        return {};
    }
}

struct Graph {
    std::vector<std::vector<BlockId>> predecessors;
    std::vector<BlockId> order; // reverse postorder of the blocks the entry reaches
    std::vector<std::uint32_t> position;
    std::vector<BlockId> dominator; // each block's immediate dominator; the entry's is itself
    std::vector<std::vector<BlockId>> children;
};

constexpr std::uint32_t unreached = std::numeric_limits<std::uint32_t>::max();

Graph analyse(const Function& function) {
    Graph graph;
    const std::size_t count = function.blocks.size();
    graph.predecessors.assign(count, {});
    graph.position.assign(count, unreached);
    graph.dominator.assign(count, ir::no_block);
    graph.children.assign(count, {});
    // Depth-first from the entry, for the postorder.
    std::vector<BlockId> postorder;
    std::vector<bool> seen(count, false);
    std::vector<std::pair<BlockId, std::size_t>> stack{{0, 0}};
    seen.at(0) = true;
    while (!stack.empty()) {
        auto& [block, next] = stack.back();
        const std::vector<BlockId> following = successors(function.blocks.at(block));
        if (next < following.size()) {
            const BlockId successor = following.at(next++);
            if (!seen.at(successor)) {
                seen.at(successor) = true;
                stack.emplace_back(successor, 0);
            }
            continue;
        }
        postorder.push_back(block);
        stack.pop_back();
    }
    graph.order.assign(postorder.rbegin(), postorder.rend());
    for (std::uint32_t index = 0; index < graph.order.size(); ++index) {
        graph.position.at(graph.order.at(index)) = index;
    }
    for (const BlockId block : graph.order) {
        for (const BlockId successor : successors(function.blocks.at(block))) {
            graph.predecessors.at(successor).push_back(block);
        }
    }
    // Cooper, Harvey and Kennedy's "A Simple, Fast Dominance Algorithm".
    graph.dominator.at(0) = 0;
    bool changed = true;
    while (changed) {
        changed = false;
        for (const BlockId block : graph.order) {
            if (block == 0) {
                continue;
            }
            BlockId chosen = ir::no_block;
            for (const BlockId predecessor : graph.predecessors.at(block)) {
                if (graph.dominator.at(predecessor) == ir::no_block) {
                    continue;
                }
                if (chosen == ir::no_block) {
                    chosen = predecessor;
                    continue;
                }
                BlockId first = chosen;
                BlockId second = predecessor;
                while (first != second) {
                    while (graph.position.at(first) > graph.position.at(second)) {
                        first = graph.dominator.at(first);
                    }
                    while (graph.position.at(second) > graph.position.at(first)) {
                        second = graph.dominator.at(second);
                    }
                }
                chosen = first;
            }
            if (chosen != ir::no_block && graph.dominator.at(block) != chosen) {
                graph.dominator.at(block) = chosen;
                changed = true;
            }
        }
    }
    for (const BlockId block : graph.order) {
        if (block != 0) {
            graph.children.at(graph.dominator.at(block)).push_back(block);
        }
    }
    return graph;
}

// --- Rewriting -------------------------------------------------------------------------------------------------------

// Values replaced by others, followed to the end of the chain.
class Replacements {
public:
    explicit Replacements(std::size_t size) : to_(size) {
        for (std::size_t index = 0; index < size; ++index) {
            to_.at(index) = static_cast<ValueId>(index);
        }
    }
    void grow(std::size_t size) {
        while (to_.size() < size) {
            to_.push_back(static_cast<ValueId>(to_.size()));
        }
    }
    void replace(ValueId from, ValueId to) {
        grow(std::max<std::size_t>(from, to) + 1);
        to_.at(from) = to;
        any_ = true;
    }
    ValueId find(ValueId value) {
        if (value == ir::no_value || value >= to_.size()) {
            return value;
        }
        ValueId root = value;
        while (to_.at(root) != root) {
            root = to_.at(root);
        }
        while (to_.at(value) != root) {
            const ValueId next = to_.at(value);
            to_.at(value) = root;
            value = next;
        }
        return root;
    }
    [[nodiscard]] bool any() const { return any_; }

private:
    std::vector<ValueId> to_;
    bool any_ = false;
};

void apply(Function& function, Replacements& replacements) {
    if (!replacements.any()) {
        return;
    }
    for (Block& block : function.blocks) {
        if (block.dead) {
            continue;
        }
        for (const ValueId id : block.instructions) {
            for (ValueId& operand : function.values.at(id).operands) {
                operand = replacements.find(operand);
            }
        }
        block.end.condition = replacements.find(block.end.condition);
        block.end.value = replacements.find(block.end.value);
        for (ValueId& argument : block.end.then_target.arguments) {
            argument = replacements.find(argument);
        }
        for (ValueId& argument : block.end.else_target.arguments) {
            argument = replacements.find(argument);
        }
    }
}

ValueId free_constant(Function& function, Op op, Kind kind, std::int64_t immediate) {
    Value value;
    value.op = op;
    value.kind = kind;
    value.immediate = immediate;
    return function.add_value(std::move(value));
}

// Whether a value can simply be dropped when nothing uses it: it does nothing else, and can't stop the program.
bool removable(Op op) {
    switch (op) {
    case Op::constant_whole:
    case Op::constant_bool:
    case Op::constant_text:
    case Op::add:
    case Op::subtract:
    case Op::multiply:
    case Op::negate:
    case Op::equal:
    case Op::not_equal:
    case Op::less:
    case Op::greater:
    case Op::at_least:
    case Op::at_most:
    case Op::logical_not:
    case Op::parse_whole:
    case Op::failed:
    case Op::value_of:
        return true;
    default:
        return false;
    }
}

std::size_t size_of(const Function& function) {
    std::size_t total = 0;
    for (const Block& block : function.blocks) {
        if (!block.dead) {
            total += block.instructions.size() + 1;
        }
    }
    return total;
}

// --- Simplifying
// ------------------------------------------------------------------------------------------------------

class Simplifier {
public:
    explicit Simplifier(Function& function) : function_(function) {}

    void run() {
        for (int round = 0; round < 8; ++round) {
            bool changed = fold();
            changed = fold_branches() || changed;
            changed = sweep_unreachable() || changed;
            changed = remove_trivial_parameters() || changed;
            changed = merge_blocks() || changed;
            changed = remove_unused() || changed;
            if (!changed) {
                break;
            }
        }
    }

private:
    Value& value(ValueId id) { return function_.values.at(id); }

    std::optional<std::int64_t> constant_of(ValueId id) {
        const Value& found = value(id);
        if (found.op == Op::constant_whole || found.op == Op::constant_bool) {
            return found.immediate;
        }
        return std::nullopt;
    }

    // Folds one instruction, giving what replaces it, if anything.
    std::optional<ValueId> fold_one(ValueId id) {
        Value& current = value(id);
        const auto operand = [&](std::size_t index) { return current.operands.at(index); };
        const auto constant = [&](std::size_t index) { return constant_of(current.operands.at(index)); };
        const auto whole = [&](std::int64_t number) {
            return free_constant(function_, Op::constant_whole, Kind::whole, number);
        };
        const auto boolean = [&](bool truth) {
            return free_constant(function_, Op::constant_bool, Kind::boolean, truth ? 1 : 0);
        };
        switch (current.op) {
        case Op::add:
        case Op::checked_add: {
            const auto left = constant(0);
            const auto right = constant(1);
            std::int64_t result = 0;
            if (left && right && !__builtin_add_overflow(*left, *right, &result)) {
                return whole(result);
            }
            if (right && *right == 0) {
                return operand(0);
            }
            if (left && *left == 0) {
                return operand(1);
            }
            if (current.op == Op::add && left && !right) {
                std::swap(current.operands.at(0), current.operands.at(1));
                return std::nullopt;
            }
            // (x + a) + b becomes x + (a + b) when both are plain, so proved to fit: the same number either way.
            if (current.op == Op::add && right) {
                const Value& inner = value(operand(0));
                if (inner.op == Op::add) {
                    if (const auto inner_right = constant_of(inner.operands.at(1))) {
                        if (!__builtin_add_overflow(*inner_right, *right, &result)) {
                            // Making the constant can move the value list, so `current` is looked up again.
                            const ValueId base = inner.operands.at(0);
                            const ValueId sum = whole(result);
                            value(id).operands = {base, sum};
                            return std::nullopt;
                        }
                    }
                }
            }
            return std::nullopt;
        }
        case Op::subtract:
        case Op::checked_subtract: {
            const auto left = constant(0);
            const auto right = constant(1);
            std::int64_t result = 0;
            if (left && right && !__builtin_sub_overflow(*left, *right, &result)) {
                return whole(result);
            }
            if (right && *right == 0) {
                return operand(0);
            }
            // x - c becomes x + (-c) when it's plain, so additions of constants can gather.
            if (current.op == Op::subtract && right && *right != lowest) {
                const ValueId negated = whole(-*right);
                Value& again = value(id);
                again.op = Op::add;
                again.operands.at(1) = negated;
                return std::nullopt;
            }
            if (operand(0) == operand(1)) {
                return whole(0);
            }
            return std::nullopt;
        }
        case Op::multiply:
        case Op::checked_multiply: {
            const auto left = constant(0);
            const auto right = constant(1);
            std::int64_t result = 0;
            if (left && right && !__builtin_mul_overflow(*left, *right, &result)) {
                return whole(result);
            }
            if (right && *right == 1) {
                return operand(0);
            }
            if (left && *left == 1) {
                return operand(1);
            }
            if ((right && *right == 0) || (left && *left == 0)) {
                return whole(0);
            }
            return std::nullopt;
        }
        case Op::negate:
        case Op::checked_negate: {
            const auto inner = constant(0);
            if (inner && *inner != lowest) {
                return whole(-*inner);
            }
            return std::nullopt;
        }
        case Op::equal:
        case Op::not_equal:
        case Op::less:
        case Op::greater:
        case Op::at_least:
        case Op::at_most: {
            const auto left = constant(0);
            const auto right = constant(1);
            if (left && right) {
                bool truth = false;
                switch (current.op) {
                case Op::equal:
                    truth = *left == *right;
                    break;
                case Op::not_equal:
                    truth = *left != *right;
                    break;
                case Op::less:
                    truth = *left < *right;
                    break;
                case Op::greater:
                    truth = *left > *right;
                    break;
                case Op::at_least:
                    truth = *left >= *right;
                    break;
                default:
                    truth = *left <= *right;
                    break;
                }
                return boolean(truth);
            }
            if (operand(0) == operand(1)) {
                return boolean(current.op == Op::equal || current.op == Op::at_least || current.op == Op::at_most);
            }
            return std::nullopt;
        }
        case Op::logical_not: {
            if (const auto inner = constant(0)) {
                return boolean(*inner == 0);
            }
            const Value& inner_value = value(operand(0));
            if (inner_value.op == Op::logical_not) {
                return inner_value.operands.at(0);
            }
            return std::nullopt;
        }
        default:
            return std::nullopt;
        }
    }

    bool fold() {
        Replacements replacements(function_.values.size());
        bool changed = false;
        for (std::size_t block_index = 0; block_index < function_.blocks.size(); ++block_index) {
            if (function_.blocks.at(block_index).dead) {
                continue;
            }
            std::vector<ValueId> kept;
            const std::vector<ValueId> instructions = function_.blocks.at(block_index).instructions;
            for (const ValueId id : instructions) {
                for (ValueId& operand : value(id).operands) {
                    operand = replacements.find(operand);
                }
                if (const auto replacement = fold_one(id)) {
                    replacements.replace(id, *replacement);
                    changed = true;
                    continue;
                }
                kept.push_back(id);
            }
            function_.blocks.at(block_index).instructions = std::move(kept);
        }
        apply(function_, replacements);
        return changed;
    }

    bool fold_branches() {
        bool changed = false;
        for (Block& block : function_.blocks) {
            if (block.dead || block.end.kind != TerminatorKind::branch) {
                continue;
            }
            const auto condition = constant_of(block.end.condition);
            const bool same = block.end.then_target.block == block.end.else_target.block &&
                              block.end.then_target.arguments == block.end.else_target.arguments;
            if (!condition && !same) {
                continue;
            }
            const Target taken = (condition && *condition == 0) ? block.end.else_target : block.end.then_target;
            block.end.kind = TerminatorKind::jump;
            block.end.condition = ir::no_value;
            block.end.then_target = taken;
            block.end.else_target = Target{};
            changed = true;
        }
        return changed;
    }

    bool sweep_unreachable() {
        const Graph graph = analyse(function_);
        bool changed = false;
        for (std::size_t index = 0; index < function_.blocks.size(); ++index) {
            Block& block = function_.blocks.at(index);
            if (!block.dead && graph.position.at(index) == unreached) {
                block.dead = true;
                block.instructions.clear();
                changed = true;
            }
        }
        return changed;
    }

    // The targets in the program that go to `block`.
    std::vector<Target*> incoming(BlockId block) {
        std::vector<Target*> targets;
        for (Block& other : function_.blocks) {
            if (other.dead) {
                continue;
            }
            if ((other.end.kind == TerminatorKind::jump || other.end.kind == TerminatorKind::branch) &&
                other.end.then_target.block == block) {
                targets.push_back(&other.end.then_target);
            }
            if (other.end.kind == TerminatorKind::branch && other.end.else_target.block == block) {
                targets.push_back(&other.end.else_target);
            }
        }
        return targets;
    }

    // A block parameter given the same value from everywhere, or only itself, is that value.
    bool remove_trivial_parameters() {
        bool changed = false;
        for (std::size_t block_index = 1; block_index < function_.blocks.size(); ++block_index) {
            if (function_.blocks.at(block_index).dead) {
                continue;
            }
            const auto block = static_cast<BlockId>(block_index);
            std::size_t parameter_index = 0;
            while (parameter_index < function_.blocks.at(block).parameters.size()) {
                const ValueId parameter = function_.blocks.at(block).parameters.at(parameter_index);
                std::vector<Target*> targets = incoming(block);
                ValueId only = ir::no_value;
                bool trivial = true;
                for (const Target* target : targets) {
                    const ValueId argument = target->arguments.at(parameter_index);
                    if (argument == parameter || argument == only) {
                        continue;
                    }
                    if (only != ir::no_value) {
                        trivial = false;
                        break;
                    }
                    only = argument;
                }
                if (!trivial || only == ir::no_value) {
                    ++parameter_index;
                    continue;
                }
                for (Target* target : targets) {
                    target->arguments.erase(target->arguments.begin() + static_cast<std::ptrdiff_t>(parameter_index));
                }
                auto& parameters = function_.blocks.at(block).parameters;
                parameters.erase(parameters.begin() + static_cast<std::ptrdiff_t>(parameter_index));
                Replacements replacements(function_.values.size());
                replacements.replace(parameter, only);
                apply(function_, replacements);
                changed = true;
            }
        }
        return changed;
    }

    // A block that only one block jumps to, and only by a plain jump, joins that block.
    bool merge_blocks() {
        bool changed = false;
        const Graph graph = analyse(function_);
        for (const BlockId first : graph.order) {
            Block& block = function_.blocks.at(first);
            if (block.dead) {
                continue;
            }
            while (block.end.kind == TerminatorKind::jump) {
                const BlockId second = block.end.then_target.block;
                if (second == first || second == 0 || incoming(second).size() != 1) {
                    break;
                }
                Block& next = function_.blocks.at(second);
                Replacements replacements(function_.values.size());
                for (std::size_t index = 0; index < next.parameters.size(); ++index) {
                    replacements.replace(next.parameters.at(index), block.end.then_target.arguments.at(index));
                }
                for (const ValueId id : next.instructions) {
                    function_.values.at(id).block = first;
                    block.instructions.push_back(id);
                }
                block.end = next.end;
                next.dead = true;
                next.instructions.clear();
                next.parameters.clear();
                apply(function_, replacements);
                changed = true;
            }
        }
        return changed;
    }

    bool remove_unused() {
        bool changed = false;
        while (true) {
            const std::vector<std::uint32_t> uses = ir::use_counts(function_);
            bool removed = false;
            for (Block& block : function_.blocks) {
                if (block.dead) {
                    continue;
                }
                const auto end = std::remove_if(block.instructions.begin(), block.instructions.end(), [&](ValueId id) {
                    return uses.at(id) == 0 && removable(function_.values.at(id).op);
                });
                if (end != block.instructions.end()) {
                    block.instructions.erase(end, block.instructions.end());
                    removed = true;
                }
            }
            if (!removed) {
                return changed;
            }
            changed = true;
        }
    }

    Function& function_;
};

// --- Checks that can't fail
// -------------------------------------------------------------------------------------------

class CheckRemover {
public:
    explicit CheckRemover(Function& function) : function_(function), graph_(analyse(function)) {}

    void run() {
        base_.assign(function_.values.size(), Range{});
        facts_.assign(function_.blocks.size(), {});
        for (std::size_t index = 0; index < function_.values.size(); ++index) {
            const Value& value = function_.values.at(index);
            if (value.block == ir::no_block && value.op == Op::constant_whole) {
                base_.at(index) = Range{value.immediate, value.immediate};
            }
        }
        for (const BlockId block : graph_.order) {
            gather_facts(block);
            Block& current = function_.blocks.at(block);
            for (const ValueId parameter : current.parameters) {
                base_.at(parameter) = parameter_range(block, parameter);
            }
            for (const ValueId id : current.instructions) {
                base_.at(id) = instruction_range(block, id);
            }
        }
    }

private:
    Range range_in(ValueId id, BlockId block) {
        Range range = id < base_.size() ? base_.at(id) : Range{};
        const auto& facts = facts_.at(block);
        if (const auto found = facts.find(id); found != facts.end()) {
            range = intersect(range, found->second);
        }
        return range;
    }

    void narrow(std::map<ValueId, Range>& facts, ValueId id, Range range, BlockId block) {
        const Range now = intersect(range_in(id, block), range);
        facts[id] = now;
    }

    // What a branch's condition says on one of its two ways, as ranges for the values it compares.
    void learn(std::map<ValueId, Range>& facts, ValueId condition, bool holds, BlockId from) {
        const Value& value = function_.values.at(condition);
        if (value.op == Op::logical_not) {
            learn(facts, value.operands.at(0), !holds, from);
            return;
        }
        if (value.kind != Kind::boolean || value.operands.size() != 2 ||
            function_.values.at(value.operands.at(0)).kind != Kind::whole) {
            return;
        }
        ValueId left = value.operands.at(0);
        ValueId right = value.operands.at(1);
        Op op = value.op;
        // Everything becomes `left < right`, `left <= right`, `left == right` or `left != right`.
        if (op == Op::greater || op == Op::at_least) {
            std::swap(left, right);
            op = op == Op::greater ? Op::less : Op::at_most;
        }
        if (!holds) {
            switch (op) {
            case Op::less: // not (l < r): r <= l
                std::swap(left, right);
                op = Op::at_most;
                break;
            case Op::at_most: // not (l <= r): r < l
                std::swap(left, right);
                op = Op::less;
                break;
            case Op::equal:
                op = Op::not_equal;
                break;
            case Op::not_equal:
                op = Op::equal;
                break;
            default:
                return;
            }
        }
        const Range left_range = range_in(left, from);
        const Range right_range = range_in(right, from);
        switch (op) {
        case Op::less:
            if (right_range.high != lowest) {
                narrow(facts, left, Range{lowest, right_range.high - 1}, from);
            }
            if (left_range.low != highest) {
                narrow(facts, right, Range{left_range.low + 1, highest}, from);
            }
            break;
        case Op::at_most:
            narrow(facts, left, Range{lowest, right_range.high}, from);
            narrow(facts, right, Range{left_range.low, highest}, from);
            break;
        case Op::equal:
            narrow(facts, left, right_range, from);
            narrow(facts, right, left_range, from);
            break;
        default:
            break;
        }
    }

    void gather_facts(BlockId block) {
        if (block == 0) {
            return;
        }
        auto facts = facts_.at(graph_.dominator.at(block));
        const auto& predecessors = graph_.predecessors.at(block);
        if (predecessors.size() == 1) {
            const BlockId from = predecessors.front();
            const ir::Terminator& end = function_.blocks.at(from).end;
            if (end.kind == TerminatorKind::branch && end.then_target.block != end.else_target.block) {
                learn(facts, end.condition, end.then_target.block == block, from);
            }
        }
        facts_.at(block) = std::move(facts);
    }

    Range parameter_range(BlockId block, ValueId parameter) {
        if (block == 0) {
            return Range{};
        }
        const auto& parameters = function_.blocks.at(block).parameters;
        const auto index = static_cast<std::size_t>(std::ranges::find(parameters, parameter) - parameters.begin());
        std::optional<Range> range;
        for (const BlockId predecessor : graph_.predecessors.at(block)) {
            // A loop's way back isn't known yet, so a loop's parameters can be anything.
            if (graph_.position.at(predecessor) >= graph_.position.at(block)) {
                return Range{};
            }
            const ir::Terminator& end = function_.blocks.at(predecessor).end;
            for (const Target* target : {&end.then_target, &end.else_target}) {
                if (target->block != block || target->arguments.size() <= index) {
                    continue;
                }
                const Range incoming = range_in(target->arguments.at(index), predecessor);
                range = range ? hull(*range, incoming) : incoming;
            }
        }
        return range.value_or(Range{});
    }

    // trace: D46, D124
    Range instruction_range(BlockId block, ValueId id) {
        Value& value = function_.values.at(id);
        const auto operand = [&](std::size_t index) { return range_in(value.operands.at(index), block); };
        std::optional<Range> result;
        switch (value.op) {
        case Op::constant_whole:
            return Range{value.immediate, value.immediate};
        case Op::add:
        case Op::checked_add:
            result = add_ranges(operand(0), operand(1));
            break;
        case Op::subtract:
        case Op::checked_subtract:
            result = subtract_ranges(operand(0), operand(1));
            break;
        case Op::multiply:
        case Op::checked_multiply:
            result = multiply_ranges(operand(0), operand(1));
            break;
        case Op::negate:
        case Op::checked_negate:
            result = negate_range(operand(0));
            break;
        case Op::random:
            // `random` gives a number from its first value to its second, or stops the program (D26).
            return Range{operand(0).low, operand(1).high};
        default:
            return Range{};
        }
        if (!result) {
            return Range{};
        }
        // Proved to fit: the check can't fail, so it goes (D46).
        switch (value.op) {
        case Op::checked_add:
            value.op = Op::add;
            break;
        case Op::checked_subtract:
            value.op = Op::subtract;
            break;
        case Op::checked_multiply:
            value.op = Op::multiply;
            break;
        case Op::checked_negate:
            value.op = Op::negate;
            break;
        default:
            break;
        }
        return *result;
    }

    Function& function_;
    Graph graph_;
    std::vector<Range> base_;
    std::vector<std::map<ValueId, Range>> facts_;
};

// --- Inlining
// -----------------------------------------------------------------------------------------------------------

// Copies `callee`'s body in place of the call `call`, which is at `position` in `block` of `caller`.
void inline_call(Function& caller, BlockId block, std::size_t position, const Function& callee) {
    const ValueId call = caller.blocks.at(block).instructions.at(position);
    const std::vector<ValueId> arguments = caller.values.at(call).operands;
    // The rest of the block moves to a new block, which takes the result as its parameter.
    const BlockId rest = caller.add_block();
    ValueId result = ir::no_value;
    if (callee.returns != Kind::none) {
        Value parameter;
        parameter.op = Op::parameter;
        parameter.kind = callee.returns;
        parameter.block = rest;
        result = caller.add_value(std::move(parameter));
        caller.blocks.at(rest).parameters.push_back(result);
    }
    {
        Block& from = caller.blocks.at(block);
        std::vector<ValueId> moved(from.instructions.begin() + static_cast<std::ptrdiff_t>(position) + 1,
                                   from.instructions.end());
        from.instructions.resize(position);
        caller.blocks.at(rest).instructions = moved;
        for (const ValueId id : moved) {
            caller.values.at(id).block = rest;
        }
        caller.blocks.at(rest).end = caller.blocks.at(block).end;
    }
    // New blocks and values for the callee's.
    std::vector<BlockId> blocks(callee.blocks.size(), ir::no_block);
    for (std::size_t index = 0; index < callee.blocks.size(); ++index) {
        if (!callee.blocks.at(index).dead) {
            blocks.at(index) = caller.add_block();
        }
    }
    std::vector<ValueId> values(callee.values.size(), ir::no_value);
    const auto& entry_parameters = callee.blocks.front().parameters;
    for (std::size_t index = 0; index < entry_parameters.size(); ++index) {
        values.at(entry_parameters.at(index)) = arguments.at(index);
    }
    for (std::size_t index = 0; index < callee.values.size(); ++index) {
        if (values.at(index) != ir::no_value) {
            continue;
        }
        const Value& original = callee.values.at(index);
        const bool live = original.block == ir::no_block || !callee.blocks.at(original.block).dead;
        if (!live) {
            continue;
        }
        Value copy = original;
        if (copy.block != ir::no_block) {
            copy.block = blocks.at(copy.block);
        }
        values.at(index) = caller.add_value(std::move(copy));
    }
    const auto map = [&](ValueId id) { return id == ir::no_value ? id : values.at(id); };
    for (std::size_t index = 0; index < callee.blocks.size(); ++index) {
        const Block& original = callee.blocks.at(index);
        if (original.dead) {
            continue;
        }
        Block& copy = caller.blocks.at(blocks.at(index));
        if (index != 0) {
            for (const ValueId parameter : original.parameters) {
                copy.parameters.push_back(map(parameter));
            }
        }
        for (const ValueId id : original.instructions) {
            const ValueId mapped = map(id);
            copy.instructions.push_back(mapped);
            for (ValueId& operand : caller.values.at(mapped).operands) {
                operand = map(operand);
            }
        }
        const ir::Terminator& end = original.end;
        if (end.kind == TerminatorKind::give_back) {
            copy.end.kind = TerminatorKind::jump;
            copy.end.then_target.block = rest;
            if (end.value != ir::no_value) {
                copy.end.then_target.arguments = {map(end.value)};
            }
            continue;
        }
        copy.end = end;
        copy.end.condition = map(end.condition);
        for (Target* target : {&copy.end.then_target, &copy.end.else_target}) {
            if (target->block != ir::no_block) {
                target->block = blocks.at(target->block);
            }
            for (ValueId& argument : target->arguments) {
                argument = map(argument);
            }
        }
    }
    Block& from = caller.blocks.at(block);
    from.end = ir::Terminator{};
    from.end.kind = TerminatorKind::jump;
    from.end.then_target.block = blocks.front();
    if (result != ir::no_value) {
        Replacements replacements(caller.values.size());
        replacements.replace(call, result);
        apply(caller, replacements);
    }
}

// Which procedures each procedure can reach through calls, itself included only through a cycle.
std::vector<std::set<std::uint32_t>> reachable_calls(const Module& module) {
    const std::size_t count = module.functions.size();
    std::vector<std::set<std::uint32_t>> direct(count);
    for (std::size_t index = 0; index < count; ++index) {
        for (const Block& block : module.functions.at(index).blocks) {
            if (block.dead) {
                continue;
            }
            for (const ValueId id : block.instructions) {
                const Value& value = module.functions.at(index).values.at(id);
                if (value.op == Op::call) {
                    direct.at(index).insert(value.callee);
                }
            }
        }
    }
    std::vector<std::set<std::uint32_t>> reach = direct;
    bool changed = true;
    while (changed) {
        changed = false;
        for (std::size_t index = 0; index < count; ++index) {
            std::set<std::uint32_t> more = reach.at(index);
            for (const std::uint32_t callee : reach.at(index)) {
                more.insert(reach.at(callee).begin(), reach.at(callee).end());
            }
            if (more.size() != reach.at(index).size()) {
                reach.at(index) = std::move(more);
                changed = true;
            }
        }
    }
    return reach;
}

// A call's argument as a parameter plus a constant, or a constant alone, when it's that simple.
struct Affine {
    std::optional<std::size_t> parameter;
    std::int64_t offset = 0;
    auto operator<=>(const Affine&) const = default;
};

std::optional<Affine> affine(const Function& function, ValueId id) {
    const Value& value = function.values.at(id);
    if (value.op == Op::parameter && value.block == 0) {
        const auto& parameters = function.blocks.front().parameters;
        return Affine{static_cast<std::size_t>(std::ranges::find(parameters, id) - parameters.begin()), 0};
    }
    if (value.op == Op::constant_whole) {
        return Affine{std::nullopt, value.immediate};
    }
    if (value.op == Op::add) {
        const auto inner = affine(function, value.operands.at(0));
        const Value& right = function.values.at(value.operands.at(1));
        std::int64_t sum = 0;
        if (inner && right.op == Op::constant_whole && !__builtin_add_overflow(inner->offset, right.immediate, &sum)) {
            return Affine{inner->parameter, sum};
        }
    }
    return std::nullopt;
}

std::vector<std::pair<BlockId, ValueId>> self_calls(const Function& function, std::uint32_t self) {
    std::vector<std::pair<BlockId, ValueId>> calls;
    for (std::size_t index = 0; index < function.blocks.size(); ++index) {
        const Block& block = function.blocks.at(index);
        if (block.dead) {
            continue;
        }
        for (const ValueId id : block.instructions) {
            const Value& value = function.values.at(id);
            if (value.op == Op::call && value.callee == self) {
                calls.emplace_back(static_cast<BlockId>(index), id);
            }
        }
    }
    return calls;
}

void tidy(Function& function, const Module& module) {
    passes::simplify(function);
    passes::remove_checks(function);
    passes::simplify(function);
    passes::merge_repeated(function, module);
    passes::simplify(function);
}

} // namespace

namespace passes {

void simplify(Function& function) {
    Simplifier(function).run();
}

void remove_checks(Function& function) {
    CheckRemover(function).run();
}

// trace: D124
void find_side_effects(Module& module) {
    for (Function& function : module.functions) {
        bool direct = function.is_main;
        for (const Block& block : function.blocks) {
            if (block.dead) {
                continue;
            }
            for (const ValueId id : block.instructions) {
                const Op op = function.values.at(id).op;
                direct = direct || (op != Op::call && ir::has_side_effects(function, module, function.values.at(id)));
            }
        }
        function.no_side_effects = !direct;
    }
    bool changed = true;
    while (changed) {
        changed = false;
        for (Function& function : module.functions) {
            if (!function.no_side_effects) {
                continue;
            }
            for (const Block& block : function.blocks) {
                if (block.dead) {
                    continue;
                }
                for (const ValueId id : block.instructions) {
                    const Value& value = function.values.at(id);
                    if (value.op == Op::call && !module.functions.at(value.callee).no_side_effects) {
                        function.no_side_effects = false;
                        changed = true;
                    }
                }
            }
        }
    }
}

// trace: D124
void inline_small(Module& module) {
    constexpr std::size_t small = 40;
    for (int round = 0; round < 2; ++round) {
        const auto reach = reachable_calls(module);
        for (std::size_t index = 0; index < module.functions.size(); ++index) {
            bool inlined = true;
            while (inlined) {
                inlined = false;
                Function& caller = module.functions.at(index);
                for (std::size_t block = 0; block < caller.blocks.size() && !inlined; ++block) {
                    if (caller.blocks.at(block).dead) {
                        continue;
                    }
                    const auto& instructions = caller.blocks.at(block).instructions;
                    for (std::size_t position = 0; position < instructions.size(); ++position) {
                        const Value& value = caller.values.at(instructions.at(position));
                        if (value.op != Op::call) {
                            continue;
                        }
                        const std::uint32_t callee = value.callee;
                        const Function& target = module.functions.at(callee);
                        if (callee == index || reach.at(callee).contains(callee) || target.is_main ||
                            size_of(target) > small) {
                            continue;
                        }
                        const Function copy = target;
                        inline_call(caller, static_cast<BlockId>(block), position, copy);
                        inlined = true;
                        break;
                    }
                }
            }
        }
    }
}

// trace: D124
// gfortran's trick, from the general method: a procedure with no side effects is inlined into itself at each of its
// own calls where the copy's inner calls would repeat a call that's already there, so merging them saves work.
void inline_into_itself(Module& module) {
    constexpr std::size_t small = 60;
    for (std::size_t index = 0; index < module.functions.size(); ++index) {
        Function& function = module.functions.at(index);
        const auto self = static_cast<std::uint32_t>(index);
        if (!function.no_side_effects || size_of(function) > small) {
            continue;
        }
        const auto calls = self_calls(function, self);
        if (calls.size() < 2) {
            continue;
        }
        const Function original = function;
        // Each call's arguments as parameters plus constants.
        std::vector<std::optional<std::vector<Affine>>> shapes;
        for (const auto& [block, call] : calls) {
            (void)block;
            std::vector<Affine> shape;
            bool simple = true;
            for (const ValueId argument : original.values.at(call).operands) {
                const auto form = affine(original, argument);
                if (!form) {
                    simple = false;
                    break;
                }
                shape.push_back(*form);
            }
            shapes.push_back(simple ? std::optional(shape) : std::nullopt);
        }
        std::vector<ValueId> chosen;
        for (std::size_t candidate = 0; candidate < calls.size(); ++candidate) {
            if (!shapes.at(candidate)) {
                continue;
            }
            const auto& outer = *shapes.at(candidate);
            bool repeats = false;
            // The copy's own calls, with the candidate's arguments put in for the parameters.
            for (std::size_t inner = 0; inner < calls.size() && !repeats; ++inner) {
                if (!shapes.at(inner)) {
                    continue;
                }
                std::vector<Affine> substituted;
                bool fits = true;
                for (const Affine& part : *shapes.at(inner)) {
                    if (!part.parameter) {
                        substituted.push_back(part);
                        continue;
                    }
                    const Affine& given = outer.at(*part.parameter);
                    std::int64_t sum = 0;
                    if (__builtin_add_overflow(given.offset, part.offset, &sum)) {
                        fits = false;
                        break;
                    }
                    substituted.push_back(Affine{given.parameter, sum});
                }
                if (!fits) {
                    continue;
                }
                for (std::size_t other = 0; other < calls.size(); ++other) {
                    if (other != candidate && shapes.at(other) && *shapes.at(other) == substituted) {
                        repeats = true;
                        break;
                    }
                }
            }
            if (repeats) {
                chosen.push_back(calls.at(candidate).second);
            }
        }
        if (chosen.empty()) {
            continue;
        }
        for (const ValueId call : chosen) {
            for (std::size_t block = 0; block < function.blocks.size(); ++block) {
                auto& instructions = function.blocks.at(block).instructions;
                const auto found = std::ranges::find(instructions, call);
                if (!function.blocks.at(block).dead && found != instructions.end()) {
                    inline_call(function, static_cast<BlockId>(block),
                                static_cast<std::size_t>(found - instructions.begin()), original);
                    break;
                }
            }
        }
        simplify(function);
        remove_checks(function);
        simplify(function);
        duplicate_returns(function, module);
        merge_repeated(function, module);
        simplify(function);
    }
}

// trace: D124
void duplicate_returns(Function& function, const Module& module) {
    constexpr std::size_t small = 12;
    const Graph graph = analyse(function);
    for (const BlockId join : graph.order) {
        const Block& block = function.blocks.at(join);
        if (join == 0 || block.end.kind != TerminatorKind::give_back || block.instructions.size() > small ||
            graph.predecessors.at(join).size() < 2) {
            continue;
        }
        const bool worth = std::ranges::any_of(block.instructions, [&](ValueId id) {
            const Value& value = function.values.at(id);
            return value.op == Op::call && module.functions.at(value.callee).no_side_effects;
        });
        if (!worth) {
            continue;
        }
        for (const BlockId predecessor : graph.predecessors.at(join)) {
            Block& from = function.blocks.at(predecessor);
            if (from.end.kind != TerminatorKind::jump || from.end.then_target.block != join) {
                continue;
            }
            const Block& template_block = function.blocks.at(join);
            std::unordered_map<ValueId, ValueId> mapped;
            for (std::size_t index = 0; index < template_block.parameters.size(); ++index) {
                mapped.emplace(template_block.parameters.at(index), from.end.then_target.arguments.at(index));
            }
            const auto map = [&](ValueId id) {
                const auto found = mapped.find(id);
                return found == mapped.end() ? id : found->second;
            };
            const std::vector<ValueId> instructions = template_block.instructions;
            const ir::Terminator end = template_block.end;
            for (const ValueId id : instructions) {
                Value copy = function.values.at(id);
                copy.block = predecessor;
                for (ValueId& operand : copy.operands) {
                    operand = map(operand);
                }
                const ValueId copied = function.add_value(std::move(copy));
                mapped.emplace(id, copied);
                function.blocks.at(predecessor).instructions.push_back(copied);
            }
            Block& updated = function.blocks.at(predecessor);
            updated.end = ir::Terminator{};
            updated.end.kind = TerminatorKind::give_back;
            updated.end.value = map(end.value);
        }
    }
}

// trace: D124
void merge_repeated(Function& function, const Module& module) {
    const Graph graph = analyse(function);
    // Operands are keyed by value, except constants, keyed by what they are, so two copies of `2` match.
    using Operand = std::pair<int, std::int64_t>;
    using Key = std::tuple<Op, Kind, std::vector<Operand>, std::int64_t, std::uint32_t>;
    const auto operand_key = [&](ValueId id) -> Operand {
        const Value& value = function.values.at(id);
        if (value.op == Op::constant_whole) {
            return {1, value.immediate};
        }
        if (value.op == Op::constant_bool) {
            return {2, value.immediate};
        }
        return {0, static_cast<std::int64_t>(id)};
    };
    std::map<Key, ValueId> available;
    Replacements replacements(function.values.size());
    const auto mergeable = [&](const Value& value) {
        switch (value.op) {
        case Op::add:
        case Op::subtract:
        case Op::multiply:
        case Op::negate:
        case Op::checked_add:
        case Op::checked_subtract:
        case Op::checked_multiply:
        case Op::checked_negate:
        case Op::equal:
        case Op::not_equal:
        case Op::less:
        case Op::greater:
        case Op::at_least:
        case Op::at_most:
        case Op::logical_not:
        case Op::parse_whole:
        case Op::failed:
        case Op::value_of:
            return true;
        case Op::call:
            // The same values give the same result, and if the first call came back, so would this one.
            return module.functions.at(value.callee).no_side_effects;
        default:
            return false;
        }
    };
    // Walks the dominator tree, so a value is reused only where the first one is sure to have run.
    std::vector<std::pair<BlockId, bool>> stack{{0, false}};
    std::vector<std::vector<Key>> added(function.blocks.size());
    while (!stack.empty()) {
        const auto [block, leaving] = stack.back();
        stack.pop_back();
        if (leaving) {
            for (const Key& key : added.at(block)) {
                available.erase(key);
            }
            continue;
        }
        stack.emplace_back(block, true);
        std::vector<ValueId> kept;
        for (const ValueId id : function.blocks.at(block).instructions) {
            Value& value = function.values.at(id);
            for (ValueId& operand : value.operands) {
                operand = replacements.find(operand);
            }
            if (!mergeable(value)) {
                kept.push_back(id);
                continue;
            }
            std::vector<Operand> operands;
            for (const ValueId operand : value.operands) {
                operands.push_back(operand_key(operand));
            }
            if (value.op == Op::add || value.op == Op::multiply || value.op == Op::checked_add ||
                value.op == Op::checked_multiply || value.op == Op::equal || value.op == Op::not_equal) {
                std::ranges::sort(operands);
            }
            Key key{value.op == Op::checked_add        ? Op::add
                    : value.op == Op::checked_subtract ? Op::subtract
                    : value.op == Op::checked_multiply ? Op::multiply
                    : value.op == Op::checked_negate   ? Op::negate
                                                       : value.op,
                    value.kind, operands, value.immediate, value.op == Op::call ? value.callee : 0};
            if (const auto found = available.find(key); found != available.end()) {
                // A plain calculation proved to fit is the same number as a checked one that passed.
                replacements.replace(id, found->second);
                continue;
            }
            available.emplace(key, id);
            added.at(block).push_back(key);
            kept.push_back(id);
        }
        function.blocks.at(block).instructions = std::move(kept);
        for (const BlockId child : graph.children.at(block)) {
            stack.emplace_back(child, false);
        }
    }
    apply(function, replacements);
}

// trace: D124
void constants_across(Module& module) {
    const std::size_t count = module.functions.size();
    // Every call's arguments, by the procedure called.
    std::vector<std::vector<std::vector<std::optional<std::pair<Op, std::int64_t>>>>> given(count);
    for (const Function& caller : module.functions) {
        for (const Block& block : caller.blocks) {
            if (block.dead) {
                continue;
            }
            for (const ValueId id : block.instructions) {
                const Value& value = caller.values.at(id);
                if (value.op != Op::call) {
                    continue;
                }
                std::vector<std::optional<std::pair<Op, std::int64_t>>> arguments;
                for (const ValueId argument : value.operands) {
                    const Value& passed = caller.values.at(argument);
                    if (passed.op == Op::constant_whole || passed.op == Op::constant_bool) {
                        arguments.emplace_back(std::pair(passed.op, passed.immediate));
                    } else {
                        arguments.emplace_back(std::nullopt);
                    }
                }
                given.at(value.callee).push_back(std::move(arguments));
            }
        }
    }
    for (std::size_t index = 0; index < count; ++index) {
        Function& function = module.functions.at(index);
        if (function.is_main || given.at(index).empty()) {
            continue;
        }
        Replacements replacements(function.values.size());
        const auto& parameters = function.blocks.front().parameters;
        for (std::size_t parameter = 0; parameter < parameters.size(); ++parameter) {
            const auto& first = given.at(index).front().at(parameter);
            if (!first) {
                continue;
            }
            const bool same = std::ranges::all_of(
                given.at(index), [&](const auto& arguments) { return arguments.at(parameter) == first; });
            if (!same) {
                continue;
            }
            const Kind kind = first->first == Op::constant_whole ? Kind::whole : Kind::boolean;
            replacements.replace(parameters.at(parameter), free_constant(function, first->first, kind, first->second));
        }
        apply(function, replacements);
    }
}

// trace: D124
void recursion_into_loops(Function& function, std::uint32_t self) {
    std::vector<BlockId> tails;
    for (std::size_t index = 0; index < function.blocks.size(); ++index) {
        const Block& block = function.blocks.at(index);
        if (block.dead || block.end.kind != TerminatorKind::give_back || block.instructions.empty()) {
            continue;
        }
        const ValueId last = block.instructions.back();
        const Value& value = function.values.at(last);
        if (value.op == Op::call && value.callee == self &&
            (block.end.value == last || (block.end.value == ir::no_value && value.kind == Kind::none))) {
            tails.push_back(static_cast<BlockId>(index));
        }
    }
    if (tails.empty()) {
        return;
    }
    // The body moves to a new loop block, whose parameters stand for the procedure's own.
    const BlockId loop = function.add_block();
    Block moved = std::move(function.blocks.front());
    function.blocks.front() = Block{};
    Block& body = function.blocks.at(loop);
    body.instructions = std::move(moved.instructions);
    body.end = moved.end;
    for (const ValueId id : body.instructions) {
        function.values.at(id).block = loop;
    }
    Replacements replacements(function.values.size());
    std::vector<ValueId> arguments;
    for (const ValueId parameter : moved.parameters) {
        Value copy = function.values.at(parameter);
        copy.block = loop;
        const ValueId renamed = function.add_value(std::move(copy));
        function.blocks.at(loop).parameters.push_back(renamed);
        replacements.replace(parameter, renamed);
        arguments.push_back(parameter);
    }
    apply(function, replacements);
    Block& entry = function.blocks.front();
    entry.parameters = moved.parameters;
    entry.end.kind = TerminatorKind::jump;
    entry.end.then_target = Target{loop, arguments};
    for (BlockId tail : tails) {
        if (tail == 0) {
            tail = loop;
        }
        Block& block = function.blocks.at(tail);
        const ValueId call = block.instructions.back();
        block.instructions.pop_back();
        block.end = ir::Terminator{};
        block.end.kind = TerminatorKind::jump;
        block.end.then_target = Target{loop, function.values.at(call).operands};
    }
}

} // namespace passes

// trace: D46, D124
void optimize(Module& module) {
    for (Function& function : module.functions) {
        passes::simplify(function);
        passes::remove_checks(function);
        passes::simplify(function);
    }
    passes::find_side_effects(module);
    passes::constants_across(module);
    for (Function& function : module.functions) {
        tidy(function, module);
    }
    passes::inline_into_itself(module);
    passes::inline_small(module);
    passes::find_side_effects(module);
    for (Function& function : module.functions) {
        tidy(function, module);
    }
    for (std::size_t index = 0; index < module.functions.size(); ++index) {
        passes::recursion_into_loops(module.functions.at(index), static_cast<std::uint32_t>(index));
        tidy(module.functions.at(index), module);
    }
}

} // namespace unio
