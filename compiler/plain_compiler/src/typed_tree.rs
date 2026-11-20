//! The typed tree: names resolved and types attached (D62). It's only built for a program that wasn't refused.

use crate::source::Place;
use crate::syntax_tree::Type;

pub type VariableId = usize;
pub type ProcedureId = usize;

pub struct TypedProgram {
    pub procedures: Vec<TypedProcedure>,
    /// The procedure the program starts with, `proc.main` (D4).
    pub main: ProcedureId,
}

pub struct TypedProcedure {
    pub name: String,
    /// For an `extern.rust` procedure (D36, D107), its failures, in the header's order (D110); its body is in Rust.
    pub external: Option<Vec<String>>,
    /// From `proc` to the end of the name, for messages.
    pub header_place: Place,
    pub parameters: Vec<VariableId>,
    pub returns: Option<Type>,
    /// Every variable and parameter, by its id.
    pub variables: Vec<Type>,
    pub body: Vec<TypedStatement>,
}

pub enum TypedStatement {
    Declare {
        variable: VariableId,
        value: TypedExpression,
    },
    Change {
        variable: VariableId,
        value: TypedExpression,
    },
    /// A call that gives nothing back, run for what it does.
    Run(TypedExpression),
    If {
        branches: Vec<(TypedExpression, Vec<TypedStatement>)>,
        else_body: Vec<TypedStatement>,
    },
    While {
        condition: TypedExpression,
        body: Vec<TypedStatement>,
    },
    Break,
    Return(Option<TypedExpression>),
}

#[derive(Clone, Debug)]
pub struct TypedExpression {
    pub kind: TypedExpressionKind,
    /// The type of its value, or `None` for a call that gives nothing back (D38).
    pub value_type: Option<Type>,
    pub place: Place,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arithmetic {
    Add,
    Subtract,
    Multiply,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Comparison {
    Equal,
    NotEqual,
    Less,
    Greater,
    AtLeast,
    AtMost,
}

/// The library's procedures that the subset uses (standard chapters 4, 7 and 9).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LibraryProcedure {
    /// `show_msg` and `show_msg.same_line` (D6).
    ShowMessage { same_line: bool },
    /// `ask` (D20).
    Ask,
    /// `be.Text` of a `Whole` (D22).
    WholeToText,
    /// `be.Whole` of a `Text`, which can fail (D22).
    TextToWhole,
    /// `random(from:, to:)` (D26).
    Random,
}

#[derive(Clone, Debug)]
pub enum FormatPiece {
    Text(String),
    Variable(VariableId, Type),
}

#[derive(Clone, Debug)]
pub enum TypedExpressionKind {
    Whole(i64),
    Text(String),
    Bool(bool),
    Variable(VariableId),
    /// `+`, `-` or `*` on two `Whole`s, stopping on overflow (D46).
    Arithmetic {
        operator: Arithmetic,
        left: Box<TypedExpression>,
        right: Box<TypedExpression>,
    },
    /// `-value`, stopping on overflow (D27, D46).
    Negate(Box<TypedExpression>),
    /// `&` (D28).
    Join(Box<TypedExpression>, Box<TypedExpression>),
    Compare {
        operator: Comparison,
        left: Box<TypedExpression>,
        right: Box<TypedExpression>,
    },
    Not(Box<TypedExpression>),
    /// `and`, stopping early (D34).
    And(Box<TypedExpression>, Box<TypedExpression>),
    /// `or`, stopping early (D34).
    Or(Box<TypedExpression>, Box<TypedExpression>),
    Call {
        procedure: ProcedureId,
        arguments: Vec<TypedExpression>,
    },
    Library {
        procedure: LibraryProcedure,
        arguments: Vec<TypedExpression>,
    },
    /// `.fmt` text with its braces filled in (D29).
    Format(Vec<FormatPiece>),
    /// `call ?? value` (D23).
    Fallback {
        call: Box<TypedExpression>,
        fallback: Box<TypedExpression>,
    },
}
