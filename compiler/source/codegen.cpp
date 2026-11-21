// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception

#include "codegen.h"

#include <llvm/IR/Constants.h>
#include <llvm/IR/IRBuilder.h>
#include <llvm/IR/Intrinsics.h>
#include <llvm/IR/LLVMContext.h>
#include <llvm/IR/MDBuilder.h>
#include <llvm/IR/Module.h>

#include <map>

namespace unio {

namespace {

using ir::Kind;
using ir::Op;

class Generator {
public:
    Generator(const ir::Module& module, std::uint32_t index, llvm::LLVMContext& context)
        : unio_(module), function_(module.functions.at(index)), context_(context),
          module_(std::make_unique<llvm::Module>(function_.symbol, context)), builder_(context) {}

    std::unique_ptr<llvm::Module> run() {
        whole_ = llvm::Type::getInt64Ty(context_);
        boolean_ = llvm::Type::getInt1Ty(context_);
        pointer_ = llvm::PointerType::getUnqual(context_);
        int32_ = llvm::Type::getInt32Ty(context_);
        void_ = llvm::Type::getVoidTy(context_);
        site_type_ = llvm::StructType::get(context_, {pointer_, pointer_, pointer_, int32_, int32_, int32_, int32_});
        llvm::Function* function = declare(function_);
        function->setLinkage(llvm::GlobalValue::ExternalLinkage);
        generate(*function);
        return std::move(module_);
    }

private:
    llvm::Type* type_of(Kind kind) {
        switch (kind) {
        case Kind::whole:
            return whole_;
        case Kind::boolean:
            return boolean_;
        case Kind::text:
            return pointer_;
        case Kind::whole_or_failure:
            return llvm::StructType::get(context_, {boolean_, whole_});
        case Kind::none:
            break;
        }
        return void_;
    }

    llvm::Function* declare(const ir::Function& function) {
        if (llvm::Function* existing = module_->getFunction(function.symbol)) {
            return existing;
        }
        std::vector<llvm::Type*> parameters;
        parameters.reserve(function.parameter_kinds.size());
        for (const Kind kind : function.parameter_kinds) {
            parameters.push_back(type_of(kind));
        }
        auto* type = llvm::FunctionType::get(type_of(function.returns), parameters, false);
        llvm::Function* declared =
            llvm::Function::Create(type, llvm::GlobalValue::ExternalLinkage, function.symbol, *module_);
        declared->addFnAttr(llvm::Attribute::NoUnwind);
        return declared;
    }

    llvm::FunctionCallee runtime(const char* name, llvm::Type* returns, const std::vector<llvm::Type*>& parameters,
                                 bool cold_stop = false) {
        auto* type = llvm::FunctionType::get(returns, parameters, false);
        llvm::FunctionCallee callee = module_->getOrInsertFunction(name, type);
        if (auto* declared = llvm::dyn_cast<llvm::Function>(callee.getCallee())) {
            declared->addFnAttr(llvm::Attribute::NoUnwind);
            if (cold_stop) {
                declared->addFnAttr(llvm::Attribute::NoReturn);
                declared->addFnAttr(llvm::Attribute::Cold);
            }
        }
        return callee;
    }

    llvm::Constant* string_constant(const std::string& text) {
        return builder_.CreateGlobalString(text, "", 0, module_.get());
    }

    // A check's place, for its message (D54), as a constant the runtime reads only when the check fails.
    llvm::Constant* site(std::uint32_t index) {
        if (const auto found = sites_.find(index); found != sites_.end()) {
            return found->second;
        }
        const ir::Site& site = unio_.sites.at(index);
        auto* value = llvm::ConstantStruct::get(
            site_type_, {string_constant(site.file), string_constant(site.source_line), string_constant(site.detail),
                         llvm::ConstantInt::get(int32_, site.line), llvm::ConstantInt::get(int32_, site.column),
                         llvm::ConstantInt::get(int32_, site.length),
                         llvm::ConstantInt::get(int32_, static_cast<std::uint64_t>(site.kind))});
        auto* global =
            new llvm::GlobalVariable(*module_, site_type_, true, llvm::GlobalValue::PrivateLinkage, value, "site");
        sites_.emplace(index, global);
        return global;
    }

    // A text constant, laid out as the runtime's texts are: an owner count that marks it as never freed, its length,
    // and its bytes (D130).
    llvm::Constant* text(std::int64_t index) {
        if (const auto found = texts_.find(index); found != texts_.end()) {
            return found->second;
        }
        const std::string& contents = unio_.texts.at(static_cast<std::size_t>(index));
        auto* bytes = llvm::ConstantDataArray::getString(context_, contents, false);
        auto* type = llvm::StructType::get(context_, {whole_, whole_, bytes->getType()});
        auto* value = llvm::ConstantStruct::get(
            type, {llvm::ConstantInt::getSigned(whole_, -1),
                   llvm::ConstantInt::get(whole_, static_cast<std::uint64_t>(contents.size())), bytes});
        auto* global = new llvm::GlobalVariable(*module_, type, true, llvm::GlobalValue::PrivateLinkage, value, "text");
        global->setAlignment(llvm::Align(8));
        texts_.emplace(index, global);
        return global;
    }

    llvm::Value* get(ir::ValueId id) { return values_.at(id); }

    void generate(llvm::Function& function) {
        std::vector<llvm::BasicBlock*> blocks(function_.blocks.size(), nullptr);
        for (std::size_t index = 0; index < function_.blocks.size(); ++index) {
            if (!function_.blocks.at(index).dead) {
                blocks.at(index) = llvm::BasicBlock::Create(context_, "b" + std::to_string(index), &function);
            }
        }
        values_.assign(function_.values.size(), nullptr);
        // Constants the optimizer made belong to no block; LLVM's constants don't need one either.
        for (std::size_t index = 0; index < function_.values.size(); ++index) {
            const ir::Value& value = function_.values.at(index);
            if (value.block == ir::no_block &&
                (value.op == Op::constant_whole || value.op == Op::constant_bool || value.op == Op::constant_text)) {
                values_.at(index) = instruction(value, function);
            }
        }
        // Block parameters become phi nodes; the entry block's are the function's own parameters.
        for (std::size_t index = 0; index < function_.blocks.size(); ++index) {
            const ir::Block& block = function_.blocks.at(index);
            if (block.dead) {
                continue;
            }
            builder_.SetInsertPoint(blocks.at(index));
            for (std::size_t parameter = 0; parameter < block.parameters.size(); ++parameter) {
                const ir::ValueId id = block.parameters.at(parameter);
                if (index == 0) {
                    values_.at(id) = function.getArg(static_cast<unsigned>(parameter));
                } else {
                    values_.at(id) = builder_.CreatePHI(type_of(function_.values.at(id).kind), 2);
                }
            }
        }
        builder_.SetInsertPoint(blocks.front(), blocks.front()->begin());
        for (std::size_t index = 0; index < function_.blocks.size(); ++index) {
            const ir::Block& block = function_.blocks.at(index);
            if (block.dead) {
                continue;
            }
            current_ = blocks.at(index);
            builder_.SetInsertPoint(current_);
            for (const ir::ValueId id : block.instructions) {
                values_.at(id) = instruction(function_.values.at(id), function);
            }
            terminator(block.end, blocks, index);
        }
        // Each phi's incoming values, now that every value exists.
        for (std::size_t index = 0; index < function_.blocks.size(); ++index) {
            const ir::Block& block = function_.blocks.at(index);
            if (block.dead) {
                continue;
            }
            for (const auto& [target, from] : outgoing(block, index)) {
                const ir::Block& destination = function_.blocks.at(target->block);
                for (std::size_t parameter = 0; parameter < destination.parameters.size(); ++parameter) {
                    auto* phi = llvm::cast<llvm::PHINode>(values_.at(destination.parameters.at(parameter)));
                    phi->addIncoming(get(target->arguments.at(parameter)), exits_.at(from));
                }
            }
        }
    }

    std::vector<std::pair<const ir::Target*, std::size_t>> outgoing(const ir::Block& block, std::size_t index) {
        std::vector<std::pair<const ir::Target*, std::size_t>> targets;
        if (block.end.kind == ir::TerminatorKind::jump) {
            targets.emplace_back(&block.end.then_target, index);
        } else if (block.end.kind == ir::TerminatorKind::branch) {
            targets.emplace_back(&block.end.then_target, index);
            targets.emplace_back(&block.end.else_target, index);
        }
        return targets;
    }

    // The LLVM block a Unio block ends in, after its checks split it, is where its phi arguments come from.
    void terminator(const ir::Terminator& end, const std::vector<llvm::BasicBlock*>& blocks, std::size_t index) {
        exits_[index] = builder_.GetInsertBlock();
        switch (end.kind) {
        case ir::TerminatorKind::jump:
            builder_.CreateBr(blocks.at(end.then_target.block));
            break;
        case ir::TerminatorKind::branch:
            builder_.CreateCondBr(get(end.condition), blocks.at(end.then_target.block),
                                  blocks.at(end.else_target.block));
            break;
        case ir::TerminatorKind::give_back:
            if (end.value == ir::no_value) {
                builder_.CreateRetVoid();
            } else {
                builder_.CreateRet(get(end.value));
            }
            break;
        case ir::TerminatorKind::unset:
            builder_.CreateUnreachable();
            break;
        }
    }

    // Stops the program through the runtime when `condition` holds, on a cold, unlikely path (D124 #13).
    void stop_if(llvm::Value* condition, const char* stop_name, const std::vector<llvm::Value*>& arguments,
                 const std::vector<llvm::Type*>& types, llvm::Function& function) {
        auto* fail = llvm::BasicBlock::Create(context_, "stop", &function);
        auto* fine = llvm::BasicBlock::Create(context_, "", &function);
        llvm::MDBuilder weights(context_);
        builder_.CreateCondBr(condition, fail, fine, weights.createUnlikelyBranchWeights());
        builder_.SetInsertPoint(fail);
        llvm::FunctionCallee stop = runtime(stop_name, void_, types, true);
        auto* call = builder_.CreateCall(stop, arguments);
        call->setDoesNotReturn();
        builder_.CreateUnreachable();
        builder_.SetInsertPoint(fine);
    }

    // trace: D46, D92, D124
    // A checked operation: LLVM's overflow intrinsic, and a branch to a cold block that stops the program with the
    // check's message, marked unlikely so it's moved out of the way (D124 #13).
    llvm::Value* checked(llvm::Intrinsic::ID intrinsic, llvm::Value* left, llvm::Value* right, std::uint32_t site_index,
                         llvm::Function& function) {
        llvm::Value* pair = builder_.CreateBinaryIntrinsic(intrinsic, left, right);
        llvm::Value* result = builder_.CreateExtractValue(pair, 0);
        llvm::Value* overflowed = builder_.CreateExtractValue(pair, 1);
        auto* fail = llvm::BasicBlock::Create(context_, "check_failed", &function);
        auto* fine = llvm::BasicBlock::Create(context_, "", &function);
        llvm::MDBuilder weights(context_);
        builder_.CreateCondBr(overflowed, fail, fine, weights.createUnlikelyBranchWeights());
        builder_.SetInsertPoint(fail);
        llvm::FunctionCallee stop = runtime("unio_overflow", void_, {pointer_, whole_, whole_}, true);
        auto* call = builder_.CreateCall(stop, {site(site_index), left, right});
        call->setDoesNotReturn();
        builder_.CreateUnreachable();
        builder_.SetInsertPoint(fine);
        return result;
    }

    llvm::Value* instruction(const ir::Value& value, llvm::Function& function) {
        auto operand = [&](std::size_t index) { return get(value.operands.at(index)); };
        switch (value.op) {
        case Op::parameter:
            return nullptr;
        case Op::constant_whole:
            return llvm::ConstantInt::getSigned(whole_, value.immediate);
        case Op::constant_bool:
            return llvm::ConstantInt::get(boolean_, value.immediate != 0 ? 1 : 0);
        case Op::constant_text:
            return text(value.immediate);
        case Op::add:
            return builder_.CreateNSWAdd(operand(0), operand(1));
        case Op::subtract:
            return builder_.CreateNSWSub(operand(0), operand(1));
        case Op::multiply:
            return builder_.CreateNSWMul(operand(0), operand(1));
        case Op::negate:
            return builder_.CreateNSWNeg(operand(0));
        case Op::checked_add:
            return checked(llvm::Intrinsic::sadd_with_overflow, operand(0), operand(1), value.site, function);
        case Op::checked_subtract:
            return checked(llvm::Intrinsic::ssub_with_overflow, operand(0), operand(1), value.site, function);
        case Op::checked_multiply:
            return checked(llvm::Intrinsic::smul_with_overflow, operand(0), operand(1), value.site, function);
        case Op::checked_negate:
            return checked(llvm::Intrinsic::ssub_with_overflow, llvm::ConstantInt::get(whole_, 0), operand(0),
                           value.site, function);
        case Op::equal:
            return builder_.CreateICmpEQ(operand(0), operand(1));
        case Op::not_equal:
            return builder_.CreateICmpNE(operand(0), operand(1));
        case Op::less:
            return builder_.CreateICmpSLT(operand(0), operand(1));
        case Op::greater:
            return builder_.CreateICmpSGT(operand(0), operand(1));
        case Op::at_least:
            return builder_.CreateICmpSGE(operand(0), operand(1));
        case Op::at_most:
            return builder_.CreateICmpSLE(operand(0), operand(1));
        case Op::logical_not:
            return builder_.CreateNot(operand(0));
        case Op::choose:
            return builder_.CreateSelect(operand(0), operand(1), operand(2));
        case Op::value_or_crash: {
            // trace: D152
            llvm::Value* failed = builder_.CreateExtractValue(operand(0), 0);
            stop_if(failed, "unio_crash", {site(value.site)}, {pointer_}, function);
            return builder_.CreateExtractValue(operand(0), 1);
        }
        case Op::deeper: {
            // trace: D154
            llvm::Value* deeper = builder_.CreateAdd(operand(0), llvm::ConstantInt::get(whole_, 1));
            llvm::Value* too_deep = builder_.CreateICmpSGT(deeper, llvm::ConstantInt::get(whole_, ir::deepest_calls));
            stop_if(too_deep, "unio_too_deep", {site(value.site), operand(0)}, {pointer_, whole_}, function);
            return deeper;
        }
        case Op::call: {
            llvm::Function* callee = declare(unio_.functions.at(value.callee));
            std::vector<llvm::Value*> arguments;
            arguments.reserve(value.operands.size());
            for (std::size_t index = 0; index < value.operands.size(); ++index) {
                arguments.push_back(operand(index));
            }
            auto* call = builder_.CreateCall(callee, arguments);
            return value.kind == Kind::none ? nullptr : call;
        }
        case Op::show:
            builder_.CreateCall(
                runtime("unio_show", void_, {pointer_, whole_}),
                {operand(0), llvm::ConstantInt::get(whole_, static_cast<std::uint64_t>(value.immediate))});
            return nullptr;
        case Op::ask:
            return builder_.CreateCall(runtime("unio_ask", pointer_, {pointer_, pointer_}),
                                       {operand(0), site(value.site)});
        case Op::random:
            return builder_.CreateCall(runtime("unio_random", whole_, {whole_, whole_, pointer_}),
                                       {operand(0), operand(1), site(value.site)});
        case Op::whole_to_text:
            return builder_.CreateCall(runtime("unio_whole_to_text", pointer_, {whole_, pointer_}),
                                       {operand(0), site(value.site)});
        case Op::join:
            return builder_.CreateCall(runtime("unio_join", pointer_, {pointer_, pointer_, pointer_}),
                                       {operand(0), operand(1), site(value.site)});
        case Op::parse_whole: {
            llvm::IRBuilder<> entry(&function.getEntryBlock(), function.getEntryBlock().begin());
            llvm::AllocaInst* slot = entry.CreateAlloca(whole_);
            llvm::Value* fine =
                builder_.CreateCall(runtime("unio_parse_whole", boolean_, {pointer_, pointer_}), {operand(0), slot});
            llvm::Value* parsed = builder_.CreateLoad(whole_, slot);
            llvm::Value* pair = llvm::PoisonValue::get(type_of(Kind::whole_or_failure));
            pair = builder_.CreateInsertValue(pair, builder_.CreateNot(fine), 0);
            return builder_.CreateInsertValue(pair, parsed, 1);
        }
        case Op::failed:
            return builder_.CreateExtractValue(operand(0), 0);
        case Op::value_of:
            return builder_.CreateExtractValue(operand(0), 1);
        case Op::retain:
            builder_.CreateCall(runtime("unio_retain", void_, {pointer_}), {operand(0)});
            return nullptr;
        case Op::release:
            builder_.CreateCall(runtime("unio_release", void_, {pointer_}), {operand(0)});
            return nullptr;
        }
        return nullptr;
    }

    const ir::Module& unio_;
    const ir::Function& function_;
    llvm::LLVMContext& context_;
    std::unique_ptr<llvm::Module> module_;
    llvm::IRBuilder<> builder_;
    llvm::Type* whole_ = nullptr;
    llvm::Type* boolean_ = nullptr;
    llvm::PointerType* pointer_ = nullptr;
    llvm::Type* int32_ = nullptr;
    llvm::Type* void_ = nullptr;
    llvm::StructType* site_type_ = nullptr;
    std::vector<llvm::Value*> values_;
    std::map<std::uint32_t, llvm::Constant*> sites_;
    std::map<std::int64_t, llvm::Constant*> texts_;
    llvm::BasicBlock* current_ = nullptr;
    std::map<std::size_t, llvm::BasicBlock*> exits_;
};

} // namespace

std::unique_ptr<llvm::Module> generate(const ir::Module& module, std::uint32_t function, llvm::LLVMContext& context) {
    return Generator(module, function, context).run();
}

} // namespace unio
