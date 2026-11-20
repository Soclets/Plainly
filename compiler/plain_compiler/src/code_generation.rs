//! Code generation: Plainly IR to Cranelift IR, then native code in an object file (D45, D62).

use std::collections::HashMap;

use cranelift_codegen::ir::condcodes::IntCC;
use cranelift_codegen::ir::{
    AbiParam, Block, FuncRef, GlobalValue, InstBuilder, MemFlagsData, StackSlotData, StackSlotKind,
    TrapCode, Type as ClType, Value, types,
};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
use cranelift_module::{DataDescription, DataId, FuncId, Linkage, Module};
use cranelift_object::{ObjectBuilder, ObjectModule};

use crate::internal_error::InternalError;
use crate::plainly_ir::{BlockEnd, IrProcedure, IrProgram, IrStep, IrType, SiteId};
use crate::typed_tree::{Arithmetic, Comparison};

fn failed(what: &str, error: impl std::fmt::Display) -> InternalError {
    InternalError::new(format!("{what}: {error}"))
}

/// The runtime's procedures that compiled code calls (`plain_runtime`).
struct RuntimeFunctions {
    start: FuncId,
    end: FuncId,
    text_from_bytes: FuncId,
    text_join: FuncId,
    whole_to_text: FuncId,
    text_to_whole: FuncId,
    show_msg: FuncId,
    show_msg_same_line: FuncId,
    ask: FuncId,
    random: FuncId,
    overflow: FuncId,
    /// The load barrier's slow path (D69).
    heal: FuncId,
}

struct Generator {
    module: ObjectModule,
    pointer: ClType,
    runtime: RuntimeFunctions,
    procedures: Vec<FuncId>,
    texts: HashMap<Vec<u8>, DataId>,
    sites: Vec<DataId>,
    site_lengths: Vec<i64>,
    /// Each call where references are live, for PlainGC: the procedure, the call's return address as an offset into
    /// it, the frame's size, and the references' slots as offsets from the stack pointer (D69).
    safepoints: Vec<(FuncId, u64, u64, Vec<u64>)>,
    /// PlainGC's word of bad colors, which the load barrier checks every reference against (D69).
    bad_mask: DataId,
    /// For each `extern.rust` procedure, by id: whether it can fail, and what it gives back (D107, D110).
    externals: Vec<Option<(bool, Option<IrType>)>>,
}

/// The symbol of an `extern.rust` procedure's glue, which the `procedure` attribute writes (D108): its name, with
/// each `.` as `__`, then its shape, so a Rust function and a Plainly declaration that disagree can't be linked.
pub fn extern_symbol(procedure: &IrProcedure) -> String {
    let kind = |local_type: IrType| match local_type {
        IrType::Text => "text",
        IrType::Whole => "whole",
        IrType::Bool => "bool",
    };
    let takes: Vec<&str> = procedure
        .parameters
        .iter()
        .map(|&parameter| kind(procedure.locals.get(parameter).copied().unwrap_or(IrType::Whole)))
        .collect();
    let takes = if takes.is_empty() {
        "nothing".to_string()
    } else {
        takes.join("_")
    };
    let gives = procedure.returns.map_or("nothing", kind);
    let mut symbol = format!(
        "plainly_procedure_{}_takes_{takes}_gives_{gives}",
        procedure.name.replace('.', "__")
    );
    if let Some(failures) = &procedure.external
        && !failures.is_empty()
    {
        symbol.push_str("_fails_");
        symbol.push_str(&failures.join("__"));
    }
    symbol
}

/// The version of PlainGC's stack map table this writes (`plain_gc`'s `stack_roots.rs` defines the format).
const STACK_MAP_TABLE_VERSION: u64 = 1;

/// Compiles the program to an object file's bytes, in `working` mode: no optimizations (D46).
pub fn generate(program: &IrProgram) -> Result<Vec<u8>, InternalError> {
    let mut flags = settings::builder();
    flags
        .set("opt_level", "none")
        .map_err(|error| failed("setting Cranelift's optimization level", error))?;
    flags
        .set("is_pic", "true")
        .map_err(|error| failed("asking Cranelift for position-independent code", error))?;
    // PlainGC finds each frame's references by walking the chain of frame pointers (D69).
    flags
        .set("preserve_frame_pointers", "true")
        .map_err(|error| failed("asking Cranelift to keep frame pointers", error))?;
    let isa = cranelift_native::builder()
        .map_err(|error| failed("finding this computer's processor", error))?
        .finish(settings::Flags::new(flags))
        .map_err(|error| failed("setting up Cranelift", error))?;
    let builder = ObjectBuilder::new(
        isa,
        "plainly_program",
        cranelift_module::default_libcall_names(),
    )
    .map_err(|error| failed("starting the object file", error))?;
    let mut module = ObjectModule::new(builder);
    let pointer = module.target_config().pointer_type();
    let runtime = declare_runtime(&mut module, pointer)?;
    let bad_mask = module
        .declare_data("plainly_gc_bad_mask", Linkage::Import, true, false)
        .map_err(|error| failed("declaring PlainGC's bad colors", error))?;

    let mut generator = Generator {
        module,
        pointer,
        runtime,
        procedures: Vec::new(),
        texts: HashMap::new(),
        sites: Vec::new(),
        site_lengths: Vec::new(),
        safepoints: Vec::new(),
        bad_mask,
        externals: program
            .procedures
            .iter()
            .map(|procedure| {
                procedure
                    .external
                    .as_ref()
                    .map(|failures| (!failures.is_empty(), procedure.returns))
            })
            .collect(),
    };
    for (index, site) in program.sites.iter().enumerate() {
        let id = generator.define_bytes(&format!("plainly_site_{index}"), site.as_bytes())?;
        generator.sites.push(id);
        generator.site_lengths.push(site.len() as i64);
    }
    for (index, procedure) in program.procedures.iter().enumerate() {
        let signature = generator.signature_of(procedure);
        let (name, linkage) = if procedure.external.is_some() {
            // trace: D107, D108
            (extern_symbol(procedure), Linkage::Import)
        } else if index == program.main {
            ("plainly_proc_dot_main".to_string(), Linkage::Local)
        } else {
            (format!("plainly_proc_{}", procedure.name), Linkage::Local)
        };
        let id = generator
            .module
            .declare_function(&name, linkage, &signature)
            .map_err(|error| failed("declaring a procedure", error))?;
        generator.procedures.push(id);
    }
    for (index, procedure) in program.procedures.iter().enumerate() {
        if procedure.external.is_none() {
            generator.define_procedure(index, procedure)?;
        }
    }
    let table = generator.define_stack_map_table()?;
    generator.define_entry(program.main, table)?;
    let product = generator.module.finish();
    product
        .emit()
        .map_err(|error| failed("writing the object file", error))
}

fn declare_runtime(
    module: &mut ObjectModule,
    pointer: ClType,
) -> Result<RuntimeFunctions, InternalError> {
    let mut declare = |name: &str, parameters: &[ClType], returns: &[ClType]| {
        let mut signature = module.make_signature();
        signature
            .params
            .extend(parameters.iter().map(|&parameter| AbiParam::new(parameter)));
        signature
            .returns
            .extend(returns.iter().map(|&value| AbiParam::new(value)));
        module
            .declare_function(name, Linkage::Import, &signature)
            .map_err(|error| failed("declaring a runtime procedure", error))
    };
    let whole = types::I64;
    let bool_type = types::I8;
    Ok(RuntimeFunctions {
        start: declare("plainly_runtime_start", &[pointer, pointer], &[])?,
        end: declare("plainly_runtime_end", &[], &[])?,
        text_from_bytes: declare("plainly_text_from_bytes", &[pointer, whole], &[pointer])?,
        text_join: declare("plainly_text_join", &[pointer, pointer], &[pointer])?,
        whole_to_text: declare("plainly_whole_to_text", &[whole], &[pointer])?,
        text_to_whole: declare("plainly_text_to_whole", &[pointer, pointer], &[bool_type])?,
        show_msg: declare("plainly_show_msg", &[pointer], &[])?,
        show_msg_same_line: declare("plainly_show_msg_same_line", &[pointer], &[])?,
        ask: declare("plainly_ask", &[pointer, pointer, whole], &[pointer])?,
        random: declare("plainly_random", &[whole, whole, pointer, whole], &[whole])?,
        overflow: declare(
            "plainly_overflow",
            &[pointer, whole, bool_type, whole, whole],
            &[],
        )?,
        heal: declare("plainly_gc_heal", &[pointer], &[pointer])?,
    })
}

impl Generator {
    fn cranelift_type(&self, local_type: IrType) -> ClType {
        match local_type {
            IrType::Whole => types::I64,
            IrType::Bool => types::I8,
            IrType::Text => self.pointer,
        }
    }

    fn signature_of(&self, procedure: &IrProcedure) -> cranelift_codegen::ir::Signature {
        let mut signature = self.module.make_signature();
        for &parameter in &procedure.parameters {
            let local_type = procedure
                .locals
                .get(parameter)
                .copied()
                .unwrap_or(IrType::Whole);
            signature
                .params
                .push(AbiParam::new(self.cranelift_type(local_type)));
        }
        let can_fail = procedure
            .external
            .as_ref()
            .is_some_and(|failures| !failures.is_empty());
        if can_fail {
            // A procedure that can fail writes its value through a pointer and gives back which failure it had,
            // 0 for none (D110).
            if procedure.returns.is_some() {
                signature.params.push(AbiParam::new(self.pointer));
            }
            signature.returns.push(AbiParam::new(types::I32));
        } else if let Some(returns) = procedure.returns {
            signature
                .returns
                .push(AbiParam::new(self.cranelift_type(returns)));
        }
        if procedure.external.is_some() {
            // Where it's called, for a panic's message (D111).
            signature.params.push(AbiParam::new(self.pointer));
            signature.params.push(AbiParam::new(types::I64));
        }
        signature
    }

    fn define_bytes(&mut self, name: &str, bytes: &[u8]) -> Result<DataId, InternalError> {
        let id = self
            .module
            .declare_data(name, Linkage::Local, false, false)
            .map_err(|error| failed("declaring data", error))?;
        let mut description = DataDescription::new();
        description.define(bytes.to_vec().into_boxed_slice());
        self.module
            .define_data(id, &description)
            .map_err(|error| failed("defining data", error))?;
        Ok(id)
    }

    fn text_data(&mut self, bytes: &[u8]) -> Result<DataId, InternalError> {
        if let Some(&id) = self.texts.get(bytes) {
            return Ok(id);
        }
        let name = format!("plainly_text_{}", self.texts.len());
        let id = self.define_bytes(&name, bytes)?;
        self.texts.insert(bytes.to_vec(), id);
        Ok(id)
    }

    fn define_procedure(
        &mut self,
        index: usize,
        procedure: &IrProcedure,
    ) -> Result<(), InternalError> {
        let mut context = self.module.make_context();
        context.func.signature = self.signature_of(procedure);
        let mut builder_context = FunctionBuilderContext::new();
        {
            let builder = FunctionBuilder::new(&mut context.func, &mut builder_context);
            let mut emitter = Emitter {
                generator: self,
                builder,
                variables: Vec::new(),
                references: Vec::new(),
                blocks: Vec::new(),
                functions: HashMap::new(),
                data: HashMap::new(),
            };
            emitter.emit_procedure(procedure)?;
            let target = emitter.generator.module.target_config();
            emitter.builder.finalize(target);
        }
        let id = self.procedures[index];
        self.module
            .define_function(id, &mut context)
            .map_err(|error| failed(&format!("compiling `{}`", procedure.name), error))?;
        // trace: D69
        let compiled = context.compiled_code().ok_or_else(|| {
            InternalError::new("Cranelift kept no compiled code to read stack maps from")
        })?;
        for (return_offset, frame_size, stack_map) in compiled.buffer.user_stack_maps() {
            let slots = stack_map
                .entries()
                .map(|(_, offset)| u64::from(offset))
                .collect();
            self.safepoints
                .push((id, u64::from(*return_offset), u64::from(*frame_size), slots));
        }
        Ok(())
    }

    /// Writes the table of every safepoint into the program, for PlainGC to find its roots (D69).
    fn define_stack_map_table(&mut self) -> Result<DataId, InternalError> {
        let word = |value: u64| value.to_ne_bytes();
        let mut bytes = Vec::new();
        bytes.extend(word(STACK_MAP_TABLE_VERSION));
        bytes.extend(word(self.safepoints.len() as u64));
        let mut description = DataDescription::new();
        let mut addresses = Vec::new();
        for (procedure, return_offset, frame_size, slots) in &self.safepoints {
            // The procedure's address is filled in when the program is linked.
            addresses.push((bytes.len(), *procedure));
            bytes.extend(word(0));
            bytes.extend(word(*return_offset));
            bytes.extend(word(*frame_size));
            bytes.extend(word(slots.len() as u64));
            for slot in slots {
                bytes.extend(word(*slot));
            }
        }
        for (offset, procedure) in addresses {
            let reference = self
                .module
                .declare_func_in_data(procedure, &mut description);
            let offset = u32::try_from(offset)
                .map_err(|_| InternalError::new("the stack map table is too big"))?;
            description.write_function_addr(offset, reference);
        }
        description.set_align(8);
        description.define(bytes.into_boxed_slice());
        let id = self
            .module
            .declare_data("plainly_stack_maps", Linkage::Local, false, false)
            .map_err(|error| failed("declaring the stack map table", error))?;
        self.module
            .define_data(id, &description)
            .map_err(|error| failed("defining the stack map table", error))?;
        Ok(id)
    }

    /// The C `main` the operating system starts: it starts the runtime, with the stack map table and its own frame
    /// pointer, where PlainGC's walk for roots stops; then runs `proc.main` (D4), and ends.
    fn define_entry(&mut self, main: usize, table: DataId) -> Result<(), InternalError> {
        let mut signature = self.module.make_signature();
        signature.params.push(AbiParam::new(types::I32));
        signature.params.push(AbiParam::new(self.pointer));
        signature.returns.push(AbiParam::new(types::I32));
        let id = self
            .module
            .declare_function("main", Linkage::Export, &signature)
            .map_err(|error| failed("declaring `main`", error))?;
        let mut context = self.module.make_context();
        context.func.signature = signature;
        let mut builder_context = FunctionBuilderContext::new();
        {
            let mut builder = FunctionBuilder::new(&mut context.func, &mut builder_context);
            let block = builder.create_block();
            builder.append_block_params_for_function_params(block);
            builder.switch_to_block(block);
            let table = self.module.declare_data_in_func(table, builder.func);
            let table = builder.ins().symbol_value(self.pointer, table);
            let frame = builder.ins().get_frame_pointer(self.pointer);
            let start = self
                .module
                .declare_func_in_func(self.runtime.start, builder.func);
            builder.ins().call(start, &[table, frame]);
            for function in [self.procedures[main], self.runtime.end] {
                let reference = self.module.declare_func_in_func(function, builder.func);
                builder.ins().call(reference, &[]);
            }
            let zero = builder.ins().iconst(types::I32, 0);
            builder.ins().return_(&[zero]);
            builder.seal_all_blocks();
            builder.finalize(self.module.target_config());
        }
        self.module
            .define_function(id, &mut context)
            .map_err(|error| failed("compiling `main`", error))
    }
}

struct Emitter<'a, 'b> {
    generator: &'a mut Generator,
    builder: FunctionBuilder<'b>,
    variables: Vec<Variable>,
    /// Which locals hold references, which every read checks with the load barrier.
    references: Vec<bool>,
    blocks: Vec<Block>,
    functions: HashMap<FuncId, FuncRef>,
    data: HashMap<DataId, GlobalValue>,
}

impl Emitter<'_, '_> {
    fn emit_procedure(&mut self, procedure: &IrProcedure) -> Result<(), InternalError> {
        for &local_type in &procedure.locals {
            let cranelift_type = self.generator.cranelift_type(local_type);
            let variable = self.builder.declare_var(cranelift_type);
            if local_type == IrType::Text {
                // A reference: Cranelift keeps it in a stack slot across every call, for PlainGC (D69).
                // trace: D69
                self.builder.declare_var_needs_stack_map(variable);
            }
            self.variables.push(variable);
            self.references.push(local_type == IrType::Text);
        }
        for _ in &procedure.blocks {
            self.blocks.push(self.builder.create_block());
        }
        let entry = self.block(0)?;
        self.builder.append_block_params_for_function_params(entry);
        self.builder.switch_to_block(entry);
        let parameters = self.builder.block_params(entry).to_vec();
        // Every local starts at zero, so no way through reads one before it's set.
        for (local, &local_type) in procedure.locals.iter().enumerate() {
            let cranelift_type = self.generator.cranelift_type(local_type);
            let value = match procedure
                .parameters
                .iter()
                .position(|&parameter| parameter == local)
            {
                Some(position) => parameters
                    .get(position)
                    .copied()
                    .ok_or_else(|| InternalError::new("a parameter is missing from its block"))?,
                None => self.builder.ins().iconst(cranelift_type, 0),
            };
            self.builder.def_var(self.variables[local], value);
        }

        for (index, block) in procedure.blocks.iter().enumerate() {
            if index > 0 {
                let cranelift_block = self.block(index)?;
                self.builder.switch_to_block(cranelift_block);
            }
            for step in &block.steps {
                self.emit_step(step)?;
            }
            self.emit_end(&block.end)?;
        }
        self.builder.seal_all_blocks();
        Ok(())
    }

    fn block(&self, index: usize) -> Result<Block, InternalError> {
        self.blocks
            .get(index)
            .copied()
            .ok_or_else(|| InternalError::new("a jump to a block that doesn't exist"))
    }

    fn get(&mut self, local: usize) -> Result<Value, InternalError> {
        let variable = *self
            .variables
            .get(local)
            .ok_or_else(|| InternalError::new("a local that doesn't exist"))?;
        let value = self.builder.use_var(variable);
        if self.references.get(local).copied().unwrap_or(false) {
            return Ok(self.load_barrier(variable, value));
        }
        Ok(value)
    }

    /// The load barrier on a reference just read (D69): if it has any of PlainGC's bad colors, the slow path brings
    /// it up to date, and the healed reference goes back into the variable, so the next read takes the fast path.
    fn load_barrier(&mut self, variable: Variable, reference: Value) -> Value {
        // trace: D69
        let mask_address = self.address_of(self.generator.bad_mask);
        let mask = self
            .builder
            .ins()
            .load(types::I64, MemFlagsData::trusted(), mask_address, 0);
        let bad = self.builder.ins().band(reference, mask);
        let slow = self.builder.create_block();
        let done = self.builder.create_block();
        let healed = self
            .builder
            .append_block_param(done, self.generator.pointer);
        self.builder.set_cold_block(slow);
        self.builder
            .ins()
            .brif(bad, slow, &[], done, &[reference.into()]);
        self.builder.switch_to_block(slow);
        let heal = self.generator.runtime.heal;
        let fresh = self.call(heal, &[reference]).unwrap_or(reference);
        self.builder.def_var(variable, fresh);
        self.builder.ins().jump(done, &[fresh.into()]);
        self.builder.switch_to_block(done);
        healed
    }

    fn set(&mut self, local: usize, value: Value) -> Result<(), InternalError> {
        let variable = *self
            .variables
            .get(local)
            .ok_or_else(|| InternalError::new("a local that doesn't exist"))?;
        self.builder.def_var(variable, value);
        Ok(())
    }

    fn function(&mut self, id: FuncId) -> FuncRef {
        if let Some(&reference) = self.functions.get(&id) {
            return reference;
        }
        let reference = self
            .generator
            .module
            .declare_func_in_func(id, self.builder.func);
        self.functions.insert(id, reference);
        reference
    }

    fn address_of(&mut self, id: DataId) -> Value {
        let global = match self.data.get(&id) {
            Some(&global) => global,
            None => {
                let global = self
                    .generator
                    .module
                    .declare_data_in_func(id, self.builder.func);
                self.data.insert(id, global);
                global
            }
        };
        self.builder
            .ins()
            .symbol_value(self.generator.pointer, global)
    }

    fn call(&mut self, id: FuncId, arguments: &[Value]) -> Option<Value> {
        let reference = self.function(id);
        let call = self.builder.ins().call(reference, arguments);
        self.builder.inst_results(call).first().copied()
    }

    fn call_value(&mut self, id: FuncId, arguments: &[Value]) -> Result<Value, InternalError> {
        self.call(id, arguments)
            .ok_or_else(|| InternalError::new("a call that should give a value gave none"))
    }

    /// The site's message start, as a pointer and a length.
    fn site(&mut self, site: SiteId) -> Result<(Value, Value), InternalError> {
        let id = *self
            .generator
            .sites
            .get(site)
            .ok_or_else(|| InternalError::new("a site that doesn't exist"))?;
        let length = self.generator.site_lengths[site];
        let address = self.address_of(id);
        let length = self.builder.ins().iconst(types::I64, length);
        Ok((address, length))
    }

    /// Stops the program when `overflowed` is true, with the check's message (D46, D54).
    fn check_overflow(
        &mut self,
        overflowed: Value,
        site: SiteId,
        operation: u8,
        left: Value,
        right: Value,
    ) -> Result<(), InternalError> {
        let stop = self.builder.create_block();
        let carry_on = self.builder.create_block();
        self.builder.set_cold_block(stop);
        self.builder
            .ins()
            .brif(overflowed, stop, &[], carry_on, &[]);
        self.builder.switch_to_block(stop);
        let (address, length) = self.site(site)?;
        let operation = self.builder.ins().iconst(types::I8, i64::from(operation));
        self.call(
            self.generator.runtime.overflow,
            &[address, length, operation, left, right],
        );
        self.builder.ins().trap(TrapCode::INTEGER_OVERFLOW);
        self.builder.switch_to_block(carry_on);
        Ok(())
    }

    fn emit_step(&mut self, step: &IrStep) -> Result<(), InternalError> {
        let runtime = &self.generator.runtime;
        let (text_from_bytes, text_join, whole_to_text, text_to_whole) = (
            runtime.text_from_bytes,
            runtime.text_join,
            runtime.whole_to_text,
            runtime.text_to_whole,
        );
        let (show_msg, show_msg_same_line, ask, random) = (
            runtime.show_msg,
            runtime.show_msg_same_line,
            runtime.ask,
            runtime.random,
        );
        match step {
            IrStep::SetWhole { to, value } => {
                let value = self.builder.ins().iconst(types::I64, *value);
                self.set(*to, value)?;
            }
            IrStep::SetBool { to, value } => {
                let value = self.builder.ins().iconst(types::I8, i64::from(*value));
                self.set(*to, value)?;
            }
            IrStep::AllocateText { to, bytes } => {
                // Empty text has no bytes to point at (D76).
                let address = if bytes.is_empty() {
                    self.builder.ins().iconst(self.generator.pointer, 0)
                } else {
                    let id = self.generator.text_data(bytes)?;
                    self.address_of(id)
                };
                let length = self.builder.ins().iconst(types::I64, bytes.len() as i64);
                let text = self.call_value(text_from_bytes, &[address, length])?;
                self.set(*to, text)?;
            }
            IrStep::Copy { to, from } => {
                let value = self.get(*from)?;
                self.set(*to, value)?;
            }
            IrStep::CheckedArithmetic {
                to,
                operator,
                left,
                right,
                site,
            } => {
                // trace: D46
                let left = self.get(*left)?;
                let right = self.get(*right)?;
                let (result, overflowed, operation) = match operator {
                    Arithmetic::Add => {
                        // Overflow when both inputs' signs differ from the result's.
                        let result = self.builder.ins().iadd(left, right);
                        let left_change = self.builder.ins().bxor(left, result);
                        let right_change = self.builder.ins().bxor(right, result);
                        let both = self.builder.ins().band(left_change, right_change);
                        let overflowed =
                            self.builder
                                .ins()
                                .icmp_imm_s(IntCC::SignedLessThan, both, 0);
                        (result, overflowed, b'+')
                    }
                    Arithmetic::Subtract => {
                        // Overflow when the inputs' signs differ and the result's sign differs from the left's.
                        let result = self.builder.ins().isub(left, right);
                        let inputs_differ = self.builder.ins().bxor(left, right);
                        let left_change = self.builder.ins().bxor(left, result);
                        let both = self.builder.ins().band(inputs_differ, left_change);
                        let overflowed =
                            self.builder
                                .ins()
                                .icmp_imm_s(IntCC::SignedLessThan, both, 0);
                        (result, overflowed, b'-')
                    }
                    Arithmetic::Multiply => {
                        // Overflow when the high half isn't just the low half's sign.
                        let result = self.builder.ins().imul(left, right);
                        let high = self.builder.ins().smulhi(left, right);
                        let sign = self.builder.ins().sshr_imm_s(result, 63);
                        let overflowed = self.builder.ins().icmp(IntCC::NotEqual, high, sign);
                        (result, overflowed, b'*')
                    }
                };
                self.check_overflow(overflowed, *site, operation, left, right)?;
                self.set(*to, result)?;
            }
            IrStep::CheckedNegate { to, value, site } => {
                // trace: D27, D46
                let value = self.get(*value)?;
                let overflowed = self.builder.ins().icmp_imm_s(IntCC::Equal, value, i64::MIN);
                let zero = self.builder.ins().iconst(types::I64, 0);
                self.check_overflow(overflowed, *site, b'n', zero, value)?;
                let result = self.builder.ins().ineg(value);
                self.set(*to, result)?;
            }
            IrStep::Compare {
                to,
                operator,
                left,
                right,
            } => {
                // trace: D17
                let condition = match operator {
                    Comparison::Equal => IntCC::Equal,
                    Comparison::NotEqual => IntCC::NotEqual,
                    Comparison::Less => IntCC::SignedLessThan,
                    Comparison::Greater => IntCC::SignedGreaterThan,
                    Comparison::AtLeast => IntCC::SignedGreaterThanOrEqual,
                    Comparison::AtMost => IntCC::SignedLessThanOrEqual,
                };
                let left = self.get(*left)?;
                let right = self.get(*right)?;
                let result = self.builder.ins().icmp(condition, left, right);
                self.set(*to, result)?;
            }
            IrStep::Not { to, value } => {
                let value = self.get(*value)?;
                let result = self.builder.ins().bxor_imm_s(value, 1);
                self.set(*to, result)?;
            }
            IrStep::JoinText { to, left, right } => {
                let left = self.get(*left)?;
                let right = self.get(*right)?;
                let text = self.call_value(text_join, &[left, right])?;
                self.set(*to, text)?;
            }
            IrStep::WholeToText { to, value } => {
                let value = self.get(*value)?;
                let text = self.call_value(whole_to_text, &[value])?;
                self.set(*to, text)?;
            }
            IrStep::TextToWhole {
                to,
                succeeded,
                value,
            } => {
                let text = self.get(*value)?;
                let slot = self.builder.create_sized_stack_slot(StackSlotData::new(
                    StackSlotKind::ExplicitSlot,
                    8,
                    3,
                ));
                let address = self
                    .builder
                    .ins()
                    .stack_addr(self.generator.pointer, slot, 0);
                let result = self.call_value(text_to_whole, &[text, address])?;
                let number =
                    self.builder
                        .ins()
                        .load(types::I64, MemFlagsData::trusted(), address, 0);
                self.set(*succeeded, result)?;
                self.set(*to, number)?;
            }
            IrStep::Call {
                to,
                procedure,
                arguments,
                failed,
                site,
            } => {
                let id = *self.generator.procedures.get(*procedure).ok_or_else(|| {
                    InternalError::new("a call to a procedure that doesn't exist")
                })?;
                let mut values = arguments
                    .iter()
                    .map(|&argument| self.get(argument))
                    .collect::<Result<Vec<_>, _>>()?;
                let external = self.generator.externals.get(*procedure).copied().flatten();
                let Some((can_fail, gives)) = external else {
                    let result = self.call(id, &values);
                    if let Some(to) = to {
                        let result = result.ok_or_else(|| {
                            InternalError::new("a procedure that should give a value gave none")
                        })?;
                        self.set(*to, result)?;
                    }
                    return Ok(());
                };
                // trace: D107, D110, D111
                let result_slot = match (can_fail, gives) {
                    (true, Some(_)) => {
                        let slot = self.builder.create_sized_stack_slot(StackSlotData::new(
                            StackSlotKind::ExplicitSlot,
                            8,
                            3,
                        ));
                        let address = self.builder.ins().stack_addr(self.generator.pointer, slot, 0);
                        values.push(address);
                        Some(address)
                    }
                    _ => None,
                };
                let site = site.ok_or_else(|| InternalError::new("a call to Rust has no site"))?;
                let (site_address, site_length) = self.site(site)?;
                values.push(site_address);
                values.push(site_length);
                let result = self.call(id, &values);
                if can_fail {
                    let code = result.ok_or_else(|| InternalError::new("a call to Rust gave no failure number"))?;
                    let has_failed = self.builder.ins().icmp_imm_s(IntCC::NotEqual, code, 0);
                    if let Some(failed) = failed {
                        self.set(*failed, has_failed)?;
                    }
                    if let (Some(to), Some(address), Some(gives)) = (to, result_slot, gives) {
                        let value_type = self.generator.cranelift_type(gives);
                        let value = self.builder.ins().load(value_type, MemFlagsData::trusted(), address, 0);
                        self.set(*to, value)?;
                    }
                } else if let Some(to) = to {
                    let result = result.ok_or_else(|| InternalError::new("a call to Rust gave no value"))?;
                    self.set(*to, result)?;
                }
            }
            IrStep::ShowMessage { text, same_line } => {
                let text = self.get(*text)?;
                let function = if *same_line {
                    show_msg_same_line
                } else {
                    show_msg
                };
                self.call(function, &[text]);
            }
            IrStep::Ask { to, question, site } => {
                let question = self.get(*question)?;
                let (site_address, site_length) = self.site(*site)?;
                let answer = self.call_value(ask, &[question, site_address, site_length])?;
                self.set(*to, answer)?;
            }
            IrStep::Random {
                to,
                low,
                high,
                site,
            } => {
                let low = self.get(*low)?;
                let high = self.get(*high)?;
                let (site_address, site_length) = self.site(*site)?;
                let number = self.call_value(random, &[low, high, site_address, site_length])?;
                self.set(*to, number)?;
            }
        }
        Ok(())
    }

    fn emit_end(&mut self, end: &BlockEnd) -> Result<(), InternalError> {
        match end {
            BlockEnd::Jump(target) => {
                let target = self.block(*target)?;
                self.builder.ins().jump(target, &[]);
            }
            BlockEnd::Branch {
                condition,
                if_true,
                if_false,
            } => {
                let condition = self.get(*condition)?;
                let if_true = self.block(*if_true)?;
                let if_false = self.block(*if_false)?;
                self.builder
                    .ins()
                    .brif(condition, if_true, &[], if_false, &[]);
            }
            BlockEnd::Return(value) => {
                let values = match value {
                    Some(value) => vec![self.get(*value)?],
                    None => Vec::new(),
                };
                self.builder.ins().return_(&values);
            }
            BlockEnd::Unreachable => {
                let code = TrapCode::user(1).ok_or_else(|| {
                    InternalError::new("Cranelift has no trap code for unreachable code")
                })?;
                self.builder.ins().trap(code);
            }
        }
        Ok(())
    }
}
