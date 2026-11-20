//! Plainly IR: the one intermediate form of Plainly's own (D62). A graph of simple steps where everything hidden is
//! written out: each check (D46), each allocation for PlainGC, labels matched to their parameters, and the way a
//! failure is handled. `final` mode's optimizations will run on it.

use std::fmt;

use crate::typed_tree::{Arithmetic, Comparison};

pub type LocalId = usize;
pub type BlockId = usize;
pub type SiteId = usize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IrType {
    Whole,
    Bool,
    /// A reference to text allocated through PlainGC.
    Text,
}

pub struct IrProgram {
    pub procedures: Vec<IrProcedure>,
    pub main: usize,
    /// Each place a check or the runtime can stop the program, as the start of its message: the place written
    /// twice, then the source line with `^` under the part in question (D54).
    pub sites: Vec<String>,
}

pub struct IrProcedure {
    pub name: String,
    /// For an `extern.rust` procedure, its failures (D110); its body is Rust, linked in (D107), and it has no blocks.
    pub external: Option<Vec<String>>,
    pub parameters: Vec<LocalId>,
    pub returns: Option<IrType>,
    /// The type of every local, by its id: variables first, then values worked out along the way.
    pub locals: Vec<IrType>,
    /// Block 0 is where the procedure starts.
    pub blocks: Vec<IrBlock>,
}

pub struct IrBlock {
    pub steps: Vec<IrStep>,
    pub end: BlockEnd,
}

#[derive(Clone, Debug)]
pub enum IrStep {
    SetWhole {
        to: LocalId,
        value: i64,
    },
    SetBool {
        to: LocalId,
        value: bool,
    },
    /// Allocates text through PlainGC, holding these bytes.
    AllocateText {
        to: LocalId,
        bytes: Vec<u8>,
    },
    Copy {
        to: LocalId,
        from: LocalId,
    },
    /// `+`, `-` or `*`, with the overflow check that stops the program in `working` mode (D46).
    CheckedArithmetic {
        to: LocalId,
        operator: Arithmetic,
        left: LocalId,
        right: LocalId,
        site: SiteId,
    },
    /// A negating `-`, with its overflow check (D27, D46).
    CheckedNegate {
        to: LocalId,
        value: LocalId,
        site: SiteId,
    },
    Compare {
        to: LocalId,
        operator: Comparison,
        left: LocalId,
        right: LocalId,
    },
    Not {
        to: LocalId,
        value: LocalId,
    },
    /// Allocates new text holding `left`, then `right` (D28).
    JoinText {
        to: LocalId,
        left: LocalId,
        right: LocalId,
    },
    /// Allocates text writing a `Whole` in digits (D22).
    WholeToText {
        to: LocalId,
        value: LocalId,
    },
    /// Reads a `Whole` from text (D22, D95). `succeeded` says whether it could; when it couldn't, `to` holds
    /// nothing.
    TextToWhole {
        to: LocalId,
        succeeded: LocalId,
        value: LocalId,
    },
    Call {
        to: Option<LocalId>,
        procedure: usize,
        arguments: Vec<LocalId>,
        /// For a call to a procedure that can fail (D110): set to whether it failed, and `to` holds nothing then.
        failed: Option<LocalId>,
        /// For a call to an `extern.rust` procedure: where it's called, for a panic's message (D111).
        site: Option<SiteId>,
    },
    /// `show_msg` or `show_msg.same_line` (D6).
    ShowMessage {
        text: LocalId,
        same_line: bool,
    },
    /// `ask` (D20).
    Ask {
        to: LocalId,
        question: LocalId,
        site: SiteId,
    },
    /// `random(from:, to:)` (D26).
    Random {
        to: LocalId,
        low: LocalId,
        high: LocalId,
        site: SiteId,
    },
}

#[derive(Clone, Debug)]
pub enum BlockEnd {
    Jump(BlockId),
    Branch {
        condition: LocalId,
        if_true: BlockId,
        if_false: BlockId,
    },
    Return(Option<LocalId>),
    /// A block no way through the procedure reaches.
    Unreachable,
}

impl fmt::Display for IrProgram {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, procedure) in self.procedures.iter().enumerate() {
            if index > 0 {
                writeln!(formatter)?;
            }
            write!(formatter, "procedure {}(", procedure.name)?;
            for (position, parameter) in procedure.parameters.iter().enumerate() {
                if position > 0 {
                    write!(formatter, ", ")?;
                }
                write!(formatter, "l{parameter}")?;
            }
            write!(formatter, ")")?;
            if let Some(returns) = procedure.returns {
                write!(formatter, " returns {returns:?}")?;
            }
            writeln!(formatter)?;
            for (id, local_type) in procedure.locals.iter().enumerate() {
                writeln!(formatter, "  local l{id}: {local_type:?}")?;
            }
            for (id, block) in procedure.blocks.iter().enumerate() {
                writeln!(formatter, "  block b{id}:")?;
                for step in &block.steps {
                    writeln!(formatter, "    {step:?}")?;
                }
                writeln!(formatter, "    {:?}", block.end)?;
            }
        }
        Ok(())
    }
}
