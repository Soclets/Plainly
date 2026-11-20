//! The syntax tree: the program as written (D62).

use crate::source::Place;
use crate::tokenizer::TextCharacter;

/// The types the standard has so far (D15, D21, D33).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Type {
    Whole,
    Bool,
    Text,
}

impl Type {
    pub fn name(self) -> &'static str {
        match self {
            Type::Whole => "Whole",
            Type::Bool => "Bool",
            Type::Text => "Text",
        }
    }
}

/// A type as written; `known` is `None` when it was refused, so nothing is refused again for using it (D55).
#[derive(Clone, Copy, Debug)]
pub struct TypeName {
    pub known: Option<Type>,
    pub place: Place,
}

#[derive(Clone, Debug)]
pub struct Name {
    pub text: String,
    pub place: Place,
}

pub struct Procedure {
    pub name: Name,
    /// Whether it's `proc.main` (D4).
    pub is_main: bool,
    /// From `proc` to the end of the name, for messages.
    pub header_place: Place,
    pub parameters: Vec<Parameter>,
    pub returns: Option<TypeName>,
    /// The `labels` line (D25).
    pub labels: Vec<LabelPair>,
    pub body: Vec<Statement>,
}

#[derive(Clone, Debug)]
pub struct Parameter {
    pub name: Name,
    pub type_name: TypeName,
}

pub struct LabelPair {
    pub parameter: Name,
    pub label: Name,
}

pub struct Statement {
    pub kind: StatementKind,
    pub place: Place,
}

pub enum StatementKind {
    /// `let` or `var` (D13).
    Declare {
        changeable: bool,
        name: Name,
        type_name: TypeName,
        value: Expression,
    },
    /// `name = value;` (D14).
    Change { name: Name, value: Expression },
    /// A call or a slide on its own.
    Expression(Expression),
    /// `if`, `else if`, `else` and `end if` (D16).
    If {
        branches: Vec<(Expression, Vec<Statement>)>,
        else_body: Option<Vec<Statement>>,
    },
    /// `while` (D19).
    While {
        condition: Expression,
        body: Vec<Statement>,
    },
    /// `break;` (D32).
    Break,
    /// `return value;` or `return;` (D57).
    Return(Option<Expression>),
    /// A statement that was refused, skipped so checking carries on (D55).
    Refused,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinaryOperator {
    Add,
    Subtract,
    Multiply,
    Join,
    Equal,
    NotEqual,
    Less,
    Greater,
    AtLeast,
    AtMost,
    And,
    Or,
}

impl BinaryOperator {
    pub fn written(self) -> &'static str {
        match self {
            BinaryOperator::Add => "+",
            BinaryOperator::Subtract => "-",
            BinaryOperator::Multiply => "*",
            BinaryOperator::Join => "&",
            BinaryOperator::Equal => "==",
            BinaryOperator::NotEqual => "!=",
            BinaryOperator::Less => "<",
            BinaryOperator::Greater => ">",
            BinaryOperator::AtLeast => "at_least",
            BinaryOperator::AtMost => "at_most",
            BinaryOperator::And => "and",
            BinaryOperator::Or => "or",
        }
    }

    pub fn is_comparison(self) -> bool {
        matches!(
            self,
            BinaryOperator::Equal
                | BinaryOperator::NotEqual
                | BinaryOperator::Less
                | BinaryOperator::Greater
                | BinaryOperator::AtLeast
                | BinaryOperator::AtMost
        )
    }
}

#[derive(Clone, Debug)]
pub struct Expression {
    pub kind: ExpressionKind,
    pub place: Place,
}

#[derive(Clone, Debug)]
pub enum ExpressionKind {
    Whole(i64),
    Text(Vec<TextCharacter>),
    Bool(bool),
    /// A single word: a variable, or a procedure written without `( )`.
    Name(String),
    /// A word with narrowings and no `( )`, like `be.Text` outside a slide.
    BareNarrowed(Callee),
    Call {
        callee: Callee,
        arguments: Vec<Argument>,
    },
    Binary {
        operator: BinaryOperator,
        operator_place: Place,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    /// `-` with no space after it (D27).
    Negate(Box<Expression>),
    /// `not` (D34).
    Not(Box<Expression>),
    /// `call ?? value` (D23).
    Fallback {
        call: Box<Expression>,
        fallback: Box<Expression>,
    },
    /// `value |> step |> step` (D5).
    Slide {
        first: Box<Expression>,
        steps: Vec<Step>,
    },
    /// `( )` around an expression, for grouping (D3).
    Grouped(Box<Expression>),
    /// An expression that was refused (D55).
    Refused,
}

/// A procedure's name as a call writes it: a word and its narrowings, like `show_msg.fmt.same_line` (D41).
#[derive(Clone, Debug)]
pub struct Callee {
    pub words: Vec<Name>,
    pub place: Place,
}

impl Callee {
    pub fn written(&self) -> String {
        self.words
            .iter()
            .map(|word| word.text.as_str())
            .collect::<Vec<_>>()
            .join(".")
    }
}

#[derive(Clone, Debug)]
pub struct Argument {
    pub label: Option<Name>,
    pub value: Expression,
}

/// One step of a slide (D5): a procedure, and the arguments added in `( )`, if any.
#[derive(Clone, Debug)]
pub struct Step {
    pub callee: Callee,
    pub arguments: Vec<Argument>,
    pub place: Place,
}
