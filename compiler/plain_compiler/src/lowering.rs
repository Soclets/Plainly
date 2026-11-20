//! Lowering: the typed tree to Plainly IR (D62), writing out every check, allocation and failure handling.

use crate::internal_error::InternalError;
use crate::message::{place_heading, source_snippet};
use crate::plainly_ir::{
    BlockEnd, BlockId, IrBlock, IrProcedure, IrProgram, IrStep, IrType, LocalId, SiteId,
};
use crate::source::{Place, SourceFile};
use crate::syntax_tree::Type;
use crate::typed_tree::{
    FormatPiece, LibraryProcedure, TypedExpression, TypedExpressionKind, TypedProcedure,
    TypedProgram, TypedStatement,
};

pub fn lower_program(
    program: &TypedProgram,
    sources: &[SourceFile],
) -> Result<IrProgram, InternalError> {
    let mut sites = Vec::new();
    let mut procedures = Vec::new();
    for procedure in &program.procedures {
        procedures.push(lower_procedure(procedure, sources, &mut sites)?);
    }
    Ok(IrProgram {
        procedures,
        main: program.main,
        sites,
    })
}

fn ir_type(value_type: Type) -> IrType {
    match value_type {
        Type::Whole => IrType::Whole,
        Type::Bool => IrType::Bool,
        Type::Text => IrType::Text,
    }
}

fn lower_procedure(
    procedure: &TypedProcedure,
    sources: &[SourceFile],
    sites: &mut Vec<String>,
) -> Result<IrProcedure, InternalError> {
    let mut lowering = Lowering {
        sources,
        sites,
        locals: procedure
            .variables
            .iter()
            .map(|&variable| ir_type(variable))
            .collect(),
        blocks: vec![BuildingBlock::default()],
        current: 0,
        loop_exits: Vec::new(),
    };
    lowering.lower_block(&procedure.body)?;
    if lowering.blocks[lowering.current].end.is_none() {
        // Falling off the end: only a procedure that gives nothing back can, as the checker made sure (D57).
        let end = if procedure.returns.is_none() {
            BlockEnd::Return(None)
        } else {
            BlockEnd::Unreachable
        };
        lowering.finish_block(end);
    }
    Ok(IrProcedure {
        name: procedure.name.clone(),
        parameters: procedure.parameters.clone(),
        returns: procedure.returns.map(ir_type),
        locals: lowering.locals,
        blocks: lowering
            .blocks
            .into_iter()
            .map(|block| IrBlock {
                steps: block.steps,
                end: block.end.unwrap_or(BlockEnd::Unreachable),
            })
            .collect(),
    })
}

#[derive(Default)]
struct BuildingBlock {
    steps: Vec<IrStep>,
    end: Option<BlockEnd>,
}

struct Lowering<'a> {
    sources: &'a [SourceFile],
    sites: &'a mut Vec<String>,
    locals: Vec<IrType>,
    blocks: Vec<BuildingBlock>,
    current: BlockId,
    /// Where each `break` goes, innermost loop last.
    loop_exits: Vec<BlockId>,
}

impl Lowering<'_> {
    fn new_local(&mut self, local_type: IrType) -> LocalId {
        self.locals.push(local_type);
        self.locals.len() - 1
    }

    fn new_block(&mut self) -> BlockId {
        self.blocks.push(BuildingBlock::default());
        self.blocks.len() - 1
    }

    fn push(&mut self, step: IrStep) {
        self.blocks[self.current].steps.push(step);
    }

    /// Ends the current block, if nothing has yet.
    fn finish_block(&mut self, end: BlockEnd) {
        let block = &mut self.blocks[self.current];
        if block.end.is_none() {
            block.end = Some(end);
        }
    }

    fn switch_to(&mut self, block: BlockId) {
        self.current = block;
    }

    /// A place the program can stop, with the start of its message written out now (D54).
    fn site(&mut self, place: Place) -> SiteId {
        let text = format!(
            "{}\n{}",
            place_heading(self.sources, place),
            source_snippet(self.sources, place, place.line.to_string().len())
        );
        self.sites.push(text);
        self.sites.len() - 1
    }

    fn lower_block(&mut self, statements: &[TypedStatement]) -> Result<(), InternalError> {
        for statement in statements {
            self.lower_statement(statement)?;
        }
        Ok(())
    }

    fn lower_statement(&mut self, statement: &TypedStatement) -> Result<(), InternalError> {
        match statement {
            TypedStatement::Declare { variable, value }
            | TypedStatement::Change { variable, value } => {
                let value = self.lower_value(value)?;
                self.push(IrStep::Copy {
                    to: *variable,
                    from: value,
                });
            }
            TypedStatement::Run(expression) => {
                self.lower_expression(expression)?;
            }
            TypedStatement::If {
                branches,
                else_body,
            } => {
                let after = self.new_block();
                for (condition, body) in branches {
                    let condition = self.lower_value(condition)?;
                    let if_true = self.new_block();
                    let if_false = self.new_block();
                    self.finish_block(BlockEnd::Branch {
                        condition,
                        if_true,
                        if_false,
                    });
                    self.switch_to(if_true);
                    self.lower_block(body)?;
                    self.finish_block(BlockEnd::Jump(after));
                    self.switch_to(if_false);
                }
                self.lower_block(else_body)?;
                self.finish_block(BlockEnd::Jump(after));
                self.switch_to(after);
            }
            TypedStatement::While { condition, body } => {
                // trace: D19
                let check = self.new_block();
                let pass = self.new_block();
                let after = self.new_block();
                self.finish_block(BlockEnd::Jump(check));
                self.switch_to(check);
                let condition = self.lower_value(condition)?;
                self.finish_block(BlockEnd::Branch {
                    condition,
                    if_true: pass,
                    if_false: after,
                });
                self.switch_to(pass);
                self.loop_exits.push(after);
                self.lower_block(body)?;
                self.loop_exits.pop();
                self.finish_block(BlockEnd::Jump(check));
                self.switch_to(after);
            }
            TypedStatement::Break => {
                // trace: D32
                let exit = *self.loop_exits.last().ok_or_else(|| {
                    InternalError::new("a `break` outside a loop reached lowering")
                })?;
                self.finish_block(BlockEnd::Jump(exit));
                let unreachable = self.new_block();
                self.switch_to(unreachable);
            }
            TypedStatement::Return(value) => {
                // trace: D57
                let value = match value {
                    Some(value) => Some(self.lower_value(value)?),
                    None => None,
                };
                self.finish_block(BlockEnd::Return(value));
                let unreachable = self.new_block();
                self.switch_to(unreachable);
            }
        }
        Ok(())
    }

    /// An expression that gives a value.
    fn lower_value(&mut self, expression: &TypedExpression) -> Result<LocalId, InternalError> {
        self.lower_expression(expression)?
            .ok_or_else(|| InternalError::new("a call that gives nothing back was used as a value"))
    }

    fn new_local_for(&mut self, expression: &TypedExpression) -> Result<LocalId, InternalError> {
        let value_type = expression
            .value_type
            .ok_or_else(|| InternalError::new("an expression with no type reached lowering"))?;
        Ok(self.new_local(ir_type(value_type)))
    }

    fn lower_expression(
        &mut self,
        expression: &TypedExpression,
    ) -> Result<Option<LocalId>, InternalError> {
        let local = match &expression.kind {
            TypedExpressionKind::Whole(value) => {
                let to = self.new_local(IrType::Whole);
                self.push(IrStep::SetWhole { to, value: *value });
                to
            }
            TypedExpressionKind::Bool(value) => {
                let to = self.new_local(IrType::Bool);
                self.push(IrStep::SetBool { to, value: *value });
                to
            }
            TypedExpressionKind::Text(text) => self.allocate_text(text),
            TypedExpressionKind::Variable(variable) => *variable,
            TypedExpressionKind::Arithmetic {
                operator,
                left,
                right,
            } => {
                let left = self.lower_value(left)?;
                let right = self.lower_value(right)?;
                let to = self.new_local(IrType::Whole);
                // trace: D46
                let site = self.site(expression.place);
                self.push(IrStep::CheckedArithmetic {
                    to,
                    operator: *operator,
                    left,
                    right,
                    site,
                });
                to
            }
            TypedExpressionKind::Negate(value) => {
                let value = self.lower_value(value)?;
                let to = self.new_local(IrType::Whole);
                // trace: D46
                let site = self.site(expression.place);
                self.push(IrStep::CheckedNegate { to, value, site });
                to
            }
            TypedExpressionKind::Join(left, right) => {
                let left = self.lower_value(left)?;
                let right = self.lower_value(right)?;
                let to = self.new_local(IrType::Text);
                self.push(IrStep::JoinText { to, left, right });
                to
            }
            TypedExpressionKind::Compare {
                operator,
                left,
                right,
            } => {
                let left = self.lower_value(left)?;
                let right = self.lower_value(right)?;
                let to = self.new_local(IrType::Bool);
                self.push(IrStep::Compare {
                    to,
                    operator: *operator,
                    left,
                    right,
                });
                to
            }
            TypedExpressionKind::Not(value) => {
                let value = self.lower_value(value)?;
                let to = self.new_local(IrType::Bool);
                self.push(IrStep::Not { to, value });
                to
            }
            TypedExpressionKind::And(left, right) | TypedExpressionKind::Or(left, right) => {
                // trace: D34
                let is_and = matches!(expression.kind, TypedExpressionKind::And(..));
                let to = self.new_local(IrType::Bool);
                let left = self.lower_value(left)?;
                self.push(IrStep::Copy { to, from: left });
                let right_block = self.new_block();
                let after = self.new_block();
                let (if_true, if_false) = if is_and {
                    (right_block, after)
                } else {
                    (after, right_block)
                };
                self.finish_block(BlockEnd::Branch {
                    condition: left,
                    if_true,
                    if_false,
                });
                self.switch_to(right_block);
                let right = self.lower_value(right)?;
                self.push(IrStep::Copy { to, from: right });
                self.finish_block(BlockEnd::Jump(after));
                self.switch_to(after);
                to
            }
            TypedExpressionKind::Call {
                procedure,
                arguments,
            } => {
                let arguments = self.lower_arguments(arguments)?;
                let to = expression
                    .value_type
                    .map(|value_type| self.new_local(ir_type(value_type)));
                self.push(IrStep::Call {
                    to,
                    procedure: *procedure,
                    arguments,
                });
                return Ok(to);
            }
            TypedExpressionKind::Library {
                procedure,
                arguments,
            } => return self.lower_library(*procedure, arguments, expression),
            TypedExpressionKind::Format(pieces) => self.lower_format(pieces)?,
            TypedExpressionKind::Fallback { call, fallback } => {
                // trace: D23
                let TypedExpressionKind::Library {
                    procedure: LibraryProcedure::TextToWhole,
                    arguments,
                } = &call.kind
                else {
                    return Err(InternalError::new(
                        "`??` after a call that can't fail reached lowering",
                    ));
                };
                let [text] = arguments.as_slice() else {
                    return Err(InternalError::new(
                        "`be.Whole` without one argument reached lowering",
                    ));
                };
                let value = self.lower_value(text)?;
                let to = self.new_local_for(call)?;
                let succeeded = self.new_local(IrType::Bool);
                self.push(IrStep::TextToWhole {
                    to,
                    succeeded,
                    value,
                });
                let use_fallback = self.new_block();
                let after = self.new_block();
                self.finish_block(BlockEnd::Branch {
                    condition: succeeded,
                    if_true: after,
                    if_false: use_fallback,
                });
                self.switch_to(use_fallback);
                let fallback = self.lower_value(fallback)?;
                self.push(IrStep::Copy { to, from: fallback });
                self.finish_block(BlockEnd::Jump(after));
                self.switch_to(after);
                to
            }
        };
        Ok(Some(local))
    }

    fn lower_arguments(
        &mut self,
        arguments: &[TypedExpression],
    ) -> Result<Vec<LocalId>, InternalError> {
        arguments
            .iter()
            .map(|argument| self.lower_value(argument))
            .collect()
    }

    fn allocate_text(&mut self, text: &str) -> LocalId {
        let to = self.new_local(IrType::Text);
        self.push(IrStep::AllocateText {
            to,
            bytes: text.as_bytes().to_vec(),
        });
        to
    }

    fn lower_library(
        &mut self,
        procedure: LibraryProcedure,
        arguments: &[TypedExpression],
        expression: &TypedExpression,
    ) -> Result<Option<LocalId>, InternalError> {
        let arguments = self.lower_arguments(arguments)?;
        let argument = |index: usize| {
            arguments.get(index).copied().ok_or_else(|| {
                InternalError::new("a library call with too few arguments reached lowering")
            })
        };
        Ok(Some(match procedure {
            LibraryProcedure::ShowMessage { same_line } => {
                self.push(IrStep::ShowMessage {
                    text: argument(0)?,
                    same_line,
                });
                return Ok(None);
            }
            LibraryProcedure::Ask => {
                let to = self.new_local(IrType::Text);
                let site = self.site(expression.place);
                self.push(IrStep::Ask {
                    to,
                    question: argument(0)?,
                    site,
                });
                to
            }
            LibraryProcedure::WholeToText => {
                let to = self.new_local(IrType::Text);
                self.push(IrStep::WholeToText {
                    to,
                    value: argument(0)?,
                });
                to
            }
            LibraryProcedure::Random => {
                let to = self.new_local(IrType::Whole);
                let site = self.site(expression.place);
                self.push(IrStep::Random {
                    to,
                    low: argument(0)?,
                    high: argument(1)?,
                    site,
                });
                to
            }
            LibraryProcedure::TextToWhole => {
                return Err(InternalError::new(
                    "`be.Whole` reached lowering without its failure handled",
                ));
            }
        }))
    }

    /// `.fmt` text: each piece becomes text, and the pieces are joined (D29).
    fn lower_format(&mut self, pieces: &[FormatPiece]) -> Result<LocalId, InternalError> {
        let mut joined: Option<LocalId> = None;
        for piece in pieces {
            let text = match piece {
                FormatPiece::Text(text) => self.allocate_text(text),
                FormatPiece::Variable(variable, Type::Text) => *variable,
                FormatPiece::Variable(variable, Type::Whole) => {
                    let to = self.new_local(IrType::Text);
                    self.push(IrStep::WholeToText {
                        to,
                        value: *variable,
                    });
                    to
                }
                FormatPiece::Variable(_, Type::Bool) => {
                    return Err(InternalError::new(
                        "a `Bool` in `.fmt` braces reached lowering",
                    ));
                }
            };
            joined = Some(match joined {
                None => text,
                Some(left) => {
                    let to = self.new_local(IrType::Text);
                    self.push(IrStep::JoinText {
                        to,
                        left,
                        right: text,
                    });
                    to
                }
            });
        }
        Ok(joined.unwrap_or_else(|| self.allocate_text("")))
    }
}
