// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception

#include "lower.h"

#include <map>
#include <unordered_map>

namespace unio {

namespace {

using ir::BlockId;
using ir::Kind;
using ir::Op;
using ir::ValueId;

Kind kind_of(Type type) {
    switch (type) {
    case Type::whole:
        return Kind::whole;
    case Type::boolean:
        return Kind::boolean;
    case Type::text:
        return Kind::text;
    default:
        return Kind::none;
    }
}

// A lowered expression's value, and whether this expression made a text that someone must release (D130).
struct Lowered {
    ValueId value = ir::no_value;
    bool owned = false;
};

class Lowerer {
public:
    Lowerer(const Sources& sources, const Program& program) : sources_(sources), program_(program) {}

    ir::Module run() {
        for (const auto& procedure : program_.procedures) {
            ir::Function function;
            function.name = procedure->is_main ? "main" : procedure->name;
            function.symbol = procedure->is_main ? "unio_main" : "unio_" + procedure->name;
            function.is_main = procedure->is_main;
            for (const Parameter& parameter : procedure->parameters) {
                function.parameter_kinds.push_back(kind_of(parameter.type.type));
            }
            function.returns = procedure->returns.transform([](const TypeName& type) { return kind_of(type.type); })
                                   .value_or(Kind::none);
            indices_.emplace(procedure.get(), static_cast<std::uint32_t>(module_.functions.size()));
            module_.functions.push_back(std::move(function));
        }
        for (const auto& procedure : program_.procedures) {
            lower_procedure(*procedure);
        }
        return std::move(module_);
    }

private:
    // --- SSA construction (Braun et al., "Simple and Efficient Construction of SSA Form") ------------------------

    BlockId new_block() {
        const BlockId block = function_->add_block();
        sealed_.push_back(false);
        predecessors_.emplace_back();
        return block;
    }

    void write_variable(std::uint32_t variable, BlockId block, ValueId value) {
        definitions_.at(variable)[block] = value;
    }

    ValueId add_parameter(BlockId block, Kind kind) {
        ir::Value value;
        value.op = Op::parameter;
        value.kind = kind;
        value.block = block;
        const ValueId id = function_->add_value(value);
        function_->blocks.at(block).parameters.push_back(id);
        return id;
    }

    ir::Target& target_of(BlockId from, BlockId to) {
        ir::Terminator& end = function_->blocks.at(from).end;
        if (end.then_target.block == to) {
            return end.then_target;
        }
        return end.else_target;
    }

    ValueId read_variable(std::uint32_t variable, BlockId block) {
        auto& definitions = definitions_.at(variable);
        if (const auto found = definitions.find(block); found != definitions.end()) {
            return found->second;
        }
        ValueId value = ir::no_value;
        const Kind kind = kinds_.at(variable);
        if (!sealed_.at(block)) {
            value = add_parameter(block, kind);
            incomplete_[block].emplace_back(variable, value);
        } else if (predecessors_.at(block).size() == 1) {
            value = read_variable(variable, predecessors_.at(block).front());
        } else {
            value = add_parameter(block, kind);
            write_variable(variable, block, value);
            fill_parameter(variable, block);
        }
        write_variable(variable, block, value);
        return value;
    }

    // Gives a new block parameter its argument from every predecessor. Parameters are added in order, so the
    // argument lists stay aligned with them.
    void fill_parameter(std::uint32_t variable, BlockId block) {
        for (const BlockId predecessor : predecessors_.at(block)) {
            const ValueId argument = read_variable(variable, predecessor);
            target_of(predecessor, block).arguments.push_back(argument);
        }
    }

    void seal(BlockId block) {
        if (sealed_.at(block)) {
            return;
        }
        auto pending = std::move(incomplete_[block]);
        incomplete_.erase(block);
        sealed_.at(block) = true;
        for (const auto& [variable, parameter] : pending) {
            (void)parameter;
            fill_parameter(variable, block);
        }
    }

    // --- Blocks and terminators -----------------------------------------------------------------------------------

    void jump(BlockId to, std::vector<ValueId> arguments = {}) {
        ir::Terminator& end = function_->blocks.at(current_).end;
        end.kind = ir::TerminatorKind::jump;
        end.then_target = ir::Target{.block = to, .arguments = std::move(arguments)};
        predecessors_.at(to).push_back(current_);
        reachable_ = false;
    }

    void branch(ValueId condition, BlockId when_true, BlockId when_false) {
        ir::Terminator& end = function_->blocks.at(current_).end;
        end.kind = ir::TerminatorKind::branch;
        end.condition = condition;
        end.then_target = ir::Target{.block = when_true, .arguments = {}};
        end.else_target = ir::Target{.block = when_false, .arguments = {}};
        predecessors_.at(when_true).push_back(current_);
        predecessors_.at(when_false).push_back(current_);
        reachable_ = false;
    }

    void start(BlockId block) {
        current_ = block;
        reachable_ = true;
    }

    ValueId emit(Op op, Kind kind, std::vector<ValueId> operands = {}, std::int64_t immediate = 0,
                 std::uint32_t site = 0) {
        ir::Value value;
        value.op = op;
        value.kind = kind;
        value.operands = std::move(operands);
        value.immediate = immediate;
        value.site = site;
        return function_->append(current_, std::move(value));
    }

    std::uint32_t site(ir::SiteKind kind, Place place) {
        const LineColumn where = sources_.line_column(place);
        ir::Site site;
        site.kind = kind;
        site.file = sources_.file(place.file).name;
        site.line = where.line;
        site.column = where.column;
        site.length = place.length;
        site.source_line = std::string(sources_.line_text(place.file, where.line));
        module_.sites.push_back(std::move(site));
        return static_cast<std::uint32_t>(module_.sites.size() - 1);
    }

    ValueId text_constant(const std::string& text) {
        const auto [entry, added] = text_indices_.try_emplace(text, module_.texts.size());
        if (added) {
            module_.texts.push_back(text);
        }
        return emit(Op::constant_text, Kind::text, {}, static_cast<std::int64_t>(entry->second));
    }

    // --- Ownership of texts (D130) --------------------------------------------------------------------------------

    void release_if_owned(const Lowered& lowered) {
        if (lowered.owned && reachable_) {
            emit(Op::release, Kind::none, {lowered.value});
        }
    }

    // A value a variable will own: a borrowed text gets a second owner.
    ValueId owned_copy(const Lowered& lowered, Kind kind) {
        if (kind == Kind::text && !lowered.owned) {
            emit(Op::retain, Kind::none, {lowered.value});
        }
        return lowered.value;
    }

    void release_scopes_from(std::size_t first_scope) {
        for (std::size_t scope = scopes_.size(); scope > first_scope; --scope) {
            for (const std::uint32_t variable : scopes_.at(scope - 1)) {
                emit(Op::release, Kind::none, {read_variable(variable, current_)});
            }
        }
    }

    // --- Procedures -----------------------------------------------------------------------------------------------

    void lower_procedure(const Procedure& procedure) {
        function_ = &module_.functions.at(indices_.at(&procedure));
        sealed_.clear();
        predecessors_.clear();
        incomplete_.clear();
        definitions_.assign(procedure.variables.size(), {});
        kinds_.clear();
        for (const auto& variable : procedure.variables) {
            kinds_.push_back(kind_of(variable->type));
        }
        scopes_.clear();
        loops_.clear();
        const BlockId entry = new_block();
        seal(entry);
        start(entry);
        for (std::size_t index = 0; index < procedure.parameters.size(); ++index) {
            const ValueId parameter = add_parameter(entry, function_->parameter_kinds.at(index));
            write_variable(static_cast<std::uint32_t>(index), entry, parameter);
        }
        scopes_.emplace_back();
        lower_body(procedure.body);
        if (reachable_) {
            release_scopes_from(0);
            function_->blocks.at(current_).end.kind = ir::TerminatorKind::give_back;
            reachable_ = false;
        }
        scopes_.clear();
    }

    void lower_body(const Body& body) {
        scopes_.emplace_back();
        for (const auto& statement : body) {
            if (!reachable_) {
                break;
            }
            lower_statement(*statement);
        }
        if (reachable_) {
            for (const std::uint32_t variable : scopes_.back()) {
                emit(Op::release, Kind::none, {read_variable(variable, current_)});
            }
        }
        scopes_.pop_back();
    }

    void lower_statement(const Statement& statement) {
        switch (statement.kind) {
        case StatementKind::declaration: {
            const Lowered value = lower_expression(*statement.value);
            const Kind kind = kind_of(statement.variable->type);
            const ValueId owned = owned_copy(value, kind);
            write_variable(statement.variable->index, current_, owned);
            if (kind == Kind::text) {
                scopes_.back().push_back(statement.variable->index);
            }
            break;
        }
        case StatementKind::assignment: {
            const Lowered value = lower_expression(*statement.value);
            const Kind kind = kind_of(statement.variable->type);
            const ValueId owned = owned_copy(value, kind);
            if (kind == Kind::text) {
                emit(Op::release, Kind::none, {read_variable(statement.variable->index, current_)});
            }
            write_variable(statement.variable->index, current_, owned);
            break;
        }
        case StatementKind::expression:
            release_if_owned(lower_expression(*statement.value));
            break;
        case StatementKind::if_chain:
            lower_if(statement);
            break;
        case StatementKind::loop:
            lower_while(statement);
            break;
        case StatementKind::leave_loop: {
            const Loop& loop = loops_.back();
            release_scopes_from(loop.scope_depth);
            jump(loop.exit);
            break;
        }
        case StatementKind::give_back: {
            ValueId value = ir::no_value;
            if (statement.value) {
                const Lowered lowered = lower_expression(*statement.value);
                value = owned_copy(lowered, kind_of(statement.value->type));
            }
            release_scopes_from(0);
            ir::Terminator& end = function_->blocks.at(current_).end;
            end.kind = ir::TerminatorKind::give_back;
            end.value = value;
            reachable_ = false;
            break;
        }
        }
    }

    // trace: D16
    void lower_if(const Statement& statement) {
        const BlockId join = new_block();
        bool join_reached = false;
        for (const Branch& arm : statement.branches) {
            if (!arm.condition) {
                lower_body(arm.body);
                if (reachable_) {
                    jump(join);
                    join_reached = true;
                }
                break;
            }
            const Lowered condition = lower_expression(*arm.condition);
            const BlockId body = new_block();
            const BlockId next = new_block();
            branch(condition.value, body, next);
            seal(body);
            seal(next);
            start(body);
            lower_body(arm.body);
            if (reachable_) {
                jump(join);
                join_reached = true;
            }
            start(next);
        }
        // Falling out of the last condition with no `else:` reaches the join too.
        if (statement.branches.back().condition) {
            jump(join);
            join_reached = true;
        }
        seal(join);
        if (join_reached) {
            start(join);
        } else {
            function_->blocks.at(join).dead = true;
            reachable_ = false;
        }
    }

    // trace: D19, D32
    void lower_while(const Statement& statement) {
        const BlockId header = new_block();
        const BlockId body = new_block();
        const BlockId exit = new_block();
        jump(header);
        start(header);
        const Lowered condition = lower_expression(*statement.condition);
        branch(condition.value, body, exit);
        seal(body);
        start(body);
        loops_.push_back(Loop{.exit = exit, .scope_depth = scopes_.size()});
        lower_body(statement.body);
        loops_.pop_back();
        if (reachable_) {
            jump(header);
        }
        seal(header);
        seal(exit);
        start(exit);
    }

    // --- Expressions ----------------------------------------------------------------------------------------------

    Lowered lower_expression(const Expression& expression) {
        switch (expression.kind) {
        case ExpressionKind::whole:
            return {.value = emit(Op::constant_whole, Kind::whole, {}, expression.whole), .owned = false};
        case ExpressionKind::boolean:
            return {.value = emit(Op::constant_bool, Kind::boolean, {}, expression.boolean ? 1 : 0), .owned = false};
        case ExpressionKind::text:
            return {.value = text_constant(expression.text), .owned = false};
        case ExpressionKind::variable:
            return {.value = read_variable(expression.variable->index, current_), .owned = false};
        case ExpressionKind::group:
            return lower_expression(*expression.left);
        case ExpressionKind::negate: {
            // trace: D27
            if (expression.left->kind == ExpressionKind::whole) {
                return {.value = emit(Op::constant_whole, Kind::whole, {}, -expression.left->whole), .owned = false};
            }
            const Lowered operand = lower_expression(*expression.left);
            return {.value = emit(Op::checked_negate, Kind::whole, {operand.value}, 0,
                                  site(ir::SiteKind::negate, expression.place)),
                    .owned = false};
        }
        case ExpressionKind::logical_not: {
            const Lowered operand = lower_expression(*expression.left);
            return {.value = emit(Op::logical_not, Kind::boolean, {operand.value}), .owned = false};
        }
        case ExpressionKind::binary:
            return lower_binary(expression);
        case ExpressionKind::call:
            return lower_call(expression);
        case ExpressionKind::slide:
            return lower_slide(expression);
        case ExpressionKind::handled:
            return lower_handled(expression);
        case ExpressionKind::error:
            break;
        }
        return {.value = emit(Op::constant_whole, Kind::whole, {}, 0), .owned = false};
    }

    // trace: D34
    Lowered lower_short_circuit(const Expression& expression) {
        const bool is_and = expression.op == BinaryOperator::logical_and;
        const Lowered left = lower_expression(*expression.left);
        const BlockId right_block = new_block();
        const BlockId join = new_block();
        const ValueId result = add_parameter(join, Kind::boolean);
        const BlockId left_end = current_;
        if (is_and) {
            branch(left.value, right_block, join);
        } else {
            branch(left.value, join, right_block);
        }
        target_of(left_end, join).arguments.push_back(left.value);
        seal(right_block);
        start(right_block);
        const Lowered right = lower_expression(*expression.right);
        jump(join, {right.value});
        seal(join);
        start(join);
        return {.value = result, .owned = false};
    }

    Lowered lower_binary(const Expression& expression) {
        if (expression.op == BinaryOperator::logical_and || expression.op == BinaryOperator::logical_or) {
            return lower_short_circuit(expression);
        }
        const Lowered left = lower_expression(*expression.left);
        const Lowered right = lower_expression(*expression.right);
        switch (expression.op) {
        case BinaryOperator::add:
            return {.value = emit(Op::checked_add, Kind::whole, {left.value, right.value}, 0,
                                  site(ir::SiteKind::add, expression.place)),
                    .owned = false};
        case BinaryOperator::subtract:
            return {.value = emit(Op::checked_subtract, Kind::whole, {left.value, right.value}, 0,
                                  site(ir::SiteKind::subtract, expression.place)),
                    .owned = false};
        case BinaryOperator::multiply:
            return {.value = emit(Op::checked_multiply, Kind::whole, {left.value, right.value}, 0,
                                  site(ir::SiteKind::multiply, expression.place)),
                    .owned = false};
        case BinaryOperator::join: {
            const ValueId joined =
                emit(Op::join, Kind::text, {left.value, right.value}, 0, site(ir::SiteKind::memory, expression.place));
            release_if_owned(left);
            release_if_owned(right);
            return {.value = joined, .owned = true};
        }
        default:
            break;
        }
        Op op = Op::equal;
        switch (expression.op) {
        case BinaryOperator::equal:
            op = Op::equal;
            break;
        case BinaryOperator::not_equal:
            op = Op::not_equal;
            break;
        case BinaryOperator::less:
            op = Op::less;
            break;
        case BinaryOperator::greater:
            op = Op::greater;
            break;
        case BinaryOperator::at_least:
            op = Op::at_least;
            break;
        default:
            op = Op::at_most;
            break;
        }
        return {.value = emit(op, Kind::boolean, {left.value, right.value}), .owned = false};
    }

    // `.fmt` text: its plain pieces and its variables, each number made text, joined in order (D29).
    Lowered lower_format(const Expression& expression) {
        const Place place = expression.place;
        Lowered result;
        for (const Expression::Piece& piece : expression.pieces) {
            Lowered part;
            if (piece.variable == nullptr) {
                part = {.value = text_constant(piece.text), .owned = false};
            } else if (piece.variable->type == Type::whole) {
                part = {.value = emit(Op::whole_to_text, Kind::text, {read_variable(piece.variable->index, current_)},
                                      0, site(ir::SiteKind::memory, place)),
                        .owned = true};
            } else {
                part = {.value = read_variable(piece.variable->index, current_), .owned = false};
            }
            if (result.value == ir::no_value) {
                result = part;
                continue;
            }
            const ValueId joined =
                emit(Op::join, Kind::text, {result.value, part.value}, 0, site(ir::SiteKind::memory, place));
            release_if_owned(result);
            release_if_owned(part);
            result = {.value = joined, .owned = true};
        }
        return result;
    }

    Lowered call_builtin(const std::string& name, const std::vector<Lowered>& arguments, const Expression* format,
                         Place place) {
        if (name.starts_with("show_msg")) {
            // The caller releases its own arguments; a `.fmt` text is made here, so it's released here.
            const Lowered text = format != nullptr ? lower_format(*format) : arguments.front();
            const bool same_line = name.ends_with(".same_line");
            emit(Op::show, Kind::none, {text.value}, same_line ? 1 : 0);
            if (format != nullptr) {
                release_if_owned(text);
            }
            return {};
        }
        if (name == "be.Text.fmt") {
            return lower_format(*format);
        }
        if (name == "be.Text") {
            return {.value = emit(Op::whole_to_text, Kind::text, {arguments.front().value}, 0,
                                  site(ir::SiteKind::memory, place)),
                    .owned = true};
        }
        if (name == "ask") {
            const ValueId answer =
                emit(Op::ask, Kind::text, {arguments.front().value}, 0, site(ir::SiteKind::memory, place));
            return {.value = answer, .owned = true};
        }
        if (name == "random") {
            return {.value = emit(Op::random, Kind::whole, {arguments.at(0).value, arguments.at(1).value}, 0,
                                  site(ir::SiteKind::random_order, place)),
                    .owned = false};
        }
        if (name == "be.Whole") {
            return {.value = emit(Op::parse_whole, Kind::whole_or_failure, {arguments.front().value}), .owned = false};
        }
        return {};
    }

    Lowered finish_call(const Procedure* procedure, const std::string& builtin, const std::vector<Lowered>& arguments,
                        const Expression* format, Place place) {
        Lowered result;
        if (procedure != nullptr) {
            std::vector<ValueId> operands;
            operands.reserve(arguments.size());
            for (const Lowered& argument : arguments) {
                operands.push_back(argument.value);
            }
            const std::uint32_t callee = indices_.at(procedure);
            const Kind returns = module_.functions.at(callee).returns;
            ir::Value value;
            value.op = Op::call;
            value.kind = returns;
            value.operands = std::move(operands);
            value.callee = callee;
            result = {.value = function_->append(current_, std::move(value)), .owned = returns == Kind::text};
        } else {
            result = call_builtin(builtin, arguments, format, place);
        }
        for (const Lowered& argument : arguments) {
            release_if_owned(argument);
        }
        return result;
    }

    Lowered lower_call(const Expression& expression) {
        std::vector<Lowered> arguments;
        if (expression.pieces.empty()) {
            for (const Argument& argument : expression.arguments) {
                arguments.push_back(lower_expression(*argument.value));
            }
        }
        return finish_call(expression.procedure, expression.builtin, arguments,
                           expression.pieces.empty() ? nullptr : &expression, expression.place);
    }

    // trace: D5
    Lowered lower_slide(const Expression& expression) {
        Lowered slid = lower_expression(*expression.left);
        for (const Step& step : expression.steps) {
            std::vector<Lowered> arguments{slid};
            for (const Argument& argument : step.arguments) {
                arguments.push_back(lower_expression(*argument.value));
            }
            slid = finish_call(step.procedure, step.builtin, arguments, nullptr, step.place);
        }
        return slid;
    }

    // trace: D23, D95
    Lowered lower_handled(const Expression& expression) {
        const Lowered attempt = lower_expression(*expression.left);
        const ValueId failed = emit(Op::failed, Kind::boolean, {attempt.value});
        const BlockId fine = new_block();
        const BlockId fallback = new_block();
        const BlockId join = new_block();
        const ValueId result = add_parameter(join, Kind::whole);
        branch(failed, fallback, fine);
        seal(fine);
        seal(fallback);
        start(fine);
        const ValueId value = emit(Op::value_of, Kind::whole, {attempt.value});
        jump(join, {value});
        start(fallback);
        const Lowered instead = lower_expression(*expression.right);
        jump(join, {instead.value});
        seal(join);
        start(join);
        return {.value = result, .owned = false};
    }

    struct Loop {
        BlockId exit;
        std::size_t scope_depth;
    };

    const Sources& sources_;
    const Program& program_;
    ir::Module module_;
    std::map<const Procedure*, std::uint32_t> indices_;
    std::map<std::string, std::size_t> text_indices_;
    ir::Function* function_ = nullptr;
    BlockId current_ = 0;
    bool reachable_ = false;
    std::vector<bool> sealed_;
    std::vector<std::vector<BlockId>> predecessors_;
    std::unordered_map<BlockId, std::vector<std::pair<std::uint32_t, ValueId>>> incomplete_;
    std::vector<std::unordered_map<BlockId, ValueId>> definitions_;
    std::vector<Kind> kinds_;
    std::vector<std::vector<std::uint32_t>> scopes_;
    std::vector<Loop> loops_;
};

} // namespace

ir::Module lower(const Sources& sources, const Program& program) {
    return Lowerer(sources, program).run();
}

} // namespace unio
