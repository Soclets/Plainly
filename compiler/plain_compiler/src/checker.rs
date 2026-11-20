//! The checker: the syntax tree to the typed tree, resolving names and types. Refusals happen here (D62), and
//! checking carries on after each one, so one compile shows every real error (D55).

use std::collections::HashMap;

use crate::message::{Edit, Message};
use crate::source::{Place, SourceFile};
use crate::syntax_tree::{
    Argument, BinaryOperator, Callee, Expression, ExpressionKind, Procedure, Statement,
    StatementKind, Type,
};
use crate::tokenizer::TextCharacter;
use crate::typed_tree::{
    Arithmetic, Comparison, FormatPiece, LibraryProcedure, ProcedureId, TypedExpression,
    TypedExpressionKind, TypedProcedure, TypedProgram, TypedStatement, VariableId,
};

/// The library's procedures a program can't declare a procedure of its own over (standard chapter 9).
const LIBRARY_NAMES: &[&str] = &["show_msg", "ask", "random"];

/// What a procedure gives back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Gives {
    /// No `returns` (D38).
    Nothing,
    /// `returns Type`; `None` when the type was refused.
    Value(Option<Type>),
}

struct Signature {
    /// Each parameter's label, if it has one (D24, D25), and type.
    parameters: Vec<(Option<String>, Option<Type>)>,
    gives: Gives,
    place: Place,
}

/// What an expression gives, while checking.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shape {
    Value(Type),
    /// A call to a procedure that gives nothing back (D38).
    Nothing,
    /// Refused, or built from something refused: nothing is refused again for using it (D55).
    Unknown,
}

struct Checked {
    expression: TypedExpression,
    shape: Shape,
    /// Whether it's a call that can fail, which must be dealt with (D23).
    can_fail: bool,
}

impl Checked {
    fn new(kind: TypedExpressionKind, shape: Shape, place: Place) -> Checked {
        let value_type = match shape {
            Shape::Value(value_type) => Some(value_type),
            Shape::Nothing | Shape::Unknown => None,
        };
        Checked {
            expression: TypedExpression {
                kind,
                value_type,
                place,
            },
            shape,
            can_fail: false,
        }
    }

    fn unknown(place: Place) -> Checked {
        Checked::new(TypedExpressionKind::Whole(0), Shape::Unknown, place)
    }

    fn boxed(self) -> Box<TypedExpression> {
        Box::new(self.expression)
    }
}

pub fn check_program(
    procedures: &[Procedure],
    sources: &[SourceFile],
    messages: &mut Vec<Message>,
) -> Option<TypedProgram> {
    let messages_before = messages.len();
    let mut signatures = Vec::new();
    let mut by_name: HashMap<String, ProcedureId> = HashMap::new();
    let mut main = None;

    for (id, procedure) in procedures.iter().enumerate() {
        if procedure.is_main {
            if let Some(first) = main {
                let first: &Procedure = &procedures[first];
                // trace: D64
                messages.push(
                    Message::refused(procedure.header_place, "this program has two `proc.main`s")
                        .because("the first is here", first.header_place)
                        // trace: D92
                        .why("a program has exactly one `proc.main`, where it starts."),
                );
            } else {
                main = Some(id);
            }
            if !procedure.parameters.is_empty() {
                // trace: D4
                messages.push(
                    Message::refused(procedure.header_place, "`proc.main` takes nothing").why(
                        // trace: D88
                        "only the program's start runs `proc.main`, so it's never given anything.",
                    ),
                );
            }
            if let Some(returns) = &procedure.returns {
                messages.push(Message::not_designed_yet(
                    returns.place,
                    "`proc.main` giving something back isn't decided",
                ));
            }
        } else if LIBRARY_NAMES.contains(&procedure.name.text.as_str()) {
            messages.push(Message::not_designed_yet(
                procedure.name.place,
                format!(
                    "a program declaring its own `{}`, which the library has, isn't decided",
                    procedure.name.text
                ),
            ));
        } else if let Some(&first) = by_name.get(&procedure.name.text) {
            // trace: D64
            messages.push(
                Message::refused(
                    procedure.name.place,
                    format!("there's already a procedure named `{}`", procedure.name.text),
                )
                .because("it's declared here", procedures[first].name.place)
                // trace: D92
                .why("every procedure in a program needs its own name, since a call has to know which one it means."),
            );
        } else {
            by_name.insert(procedure.name.text.clone(), id);
        }
        signatures.push(signature_of(procedure, messages));
    }

    let Some(main) = main else {
        messages.push(
            Message::refused(
                Place {
                    file: 0,
                    line: 1,
                    column: 1,
                    length: 1,
                },
                "this program has no `proc.main`",
            )
            // trace: D92
            .why("a program starts with `proc.main`."),
        );
        return None;
    };

    let program = ProgramFacts {
        signatures: &signatures,
        by_name: &by_name,
        sources,
    };
    let mut typed_procedures = Vec::new();
    for procedure in procedures {
        typed_procedures.push(ProcedureChecker::check(&program, procedure, messages));
    }
    if messages.len() > messages_before {
        return None;
    }
    Some(TypedProgram {
        procedures: typed_procedures,
        main,
    })
}

/// A procedure's parameters, labels and what it gives back, checking its `labels` line (D25) and that its
/// parameters' names differ.
fn signature_of(procedure: &Procedure, messages: &mut Vec<Message>) -> Signature {
    let mut labels: Vec<Option<String>> = vec![None; procedure.parameters.len()];
    for (index, parameter) in procedure.parameters.iter().enumerate() {
        if let Some(earlier) = procedure.parameters[..index]
            .iter()
            .find(|earlier| earlier.name.text == parameter.name.text)
        {
            messages.push(
                Message::refused(
                    parameter.name.place,
                    format!("two parameters are named `{}`", parameter.name.text),
                )
                .because("the first is here", earlier.name.place)
                .why("each parameter needs its own name."),
            );
        }
    }
    let mut labelled: Vec<(String, Place)> = Vec::new();
    for pair in &procedure.labels {
        // trace: D25
        let Some(index) = procedure
            .parameters
            .iter()
            .position(|parameter| parameter.name.text == pair.parameter.text)
        else {
            messages.push(
                Message::refused(
                    pair.parameter.place,
                    format!(
                        "`{}` has no parameter named `{}`",
                        procedure.name.text, pair.parameter.text
                    ),
                )
                // trace: D89
                .why("a `labels` line gives labels to the procedure's own parameters."),
            );
            continue;
        };
        if labels[index].is_some() {
            messages.push(
                Message::refused(
                    pair.parameter.place,
                    format!("`{}` already has a label", pair.parameter.text),
                )
                // trace: D89
                .why("each parameter has at most one label."),
            );
            continue;
        }
        if let Some((_, place)) = labelled.iter().find(|(label, _)| *label == pair.label.text) {
            messages.push(
                Message::refused(
                    pair.label.place,
                    format!("two parameters have the label `{}`", pair.label.text),
                )
                .because("the first is here", *place)
                // trace: D89
                .why("each label names one parameter, so a call can say which value is which."),
            );
            continue;
        }
        labelled.push((pair.label.text.clone(), pair.label.place));
        labels[index] = Some(pair.label.text.clone());
    }
    Signature {
        parameters: labels
            .into_iter()
            .zip(&procedure.parameters)
            .map(|(label, parameter)| (label, parameter.type_name.known))
            .collect(),
        gives: match &procedure.returns {
            Some(type_name) => Gives::Value(type_name.known),
            None => Gives::Nothing,
        },
        place: procedure.name.place,
    }
}

struct ProgramFacts<'a> {
    signatures: &'a [Signature],
    by_name: &'a HashMap<String, ProcedureId>,
    sources: &'a [SourceFile],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VariableKind {
    Let,
    Var,
    Parameter,
}

struct VariableFacts {
    name: String,
    value_type: Option<Type>,
    kind: VariableKind,
    /// Where it's declared: the `let` or `var`, or the parameter's name.
    place: Place,
}

/// How a statement can leave, for D9 and D57.
#[derive(Clone, Copy, Debug, Default)]
struct Flow {
    /// Some way through it runs a `return`.
    returns_somewhere: bool,
    /// Some way through it runs a `break` that leaves a loop around it.
    breaks_out: bool,
    /// Every way through it ends in a `return`.
    always_returns: bool,
}

/// Why nothing may follow in a block.
enum Blocked {
    /// trace: D57
    Return(Place),
    Break(Place),
    /// trace: D9
    IfLeaves(Place),
}

struct ProcedureChecker<'a, 'b> {
    program: &'a ProgramFacts<'a>,
    messages: &'b mut Vec<Message>,
    variables: Vec<VariableFacts>,
    scopes: Vec<Vec<VariableId>>,
    /// Variables whose block has ended, by name, with where each was declared (D93).
    ended: HashMap<String, Place>,
    loop_depth: usize,
    gives: Gives,
    name: String,
}

impl<'a, 'b> ProcedureChecker<'a, 'b> {
    fn check(
        program: &'a ProgramFacts<'a>,
        procedure: &Procedure,
        messages: &'b mut Vec<Message>,
    ) -> TypedProcedure {
        let gives = match &procedure.returns {
            Some(type_name) => Gives::Value(type_name.known),
            None => Gives::Nothing,
        };
        let mut checker = ProcedureChecker {
            program,
            messages,
            variables: Vec::new(),
            scopes: vec![Vec::new()],
            ended: HashMap::new(),
            loop_depth: 0,
            gives,
            name: procedure.name.text.clone(),
        };
        let mut parameters = Vec::new();
        for parameter in &procedure.parameters {
            checker.refuse_procedure_name(&parameter.name.text, parameter.name.place);
            parameters.push(checker.declare(
                &parameter.name.text,
                parameter.type_name.known,
                VariableKind::Parameter,
                parameter.name.place,
            ));
        }
        let (body, flow) = checker.check_block(&procedure.body);

        // trace: D57
        if let Some(last) = procedure.body.last()
            && matches!(last.kind, StatementKind::Return(None))
        {
            checker.messages.push(
                Message::refused(
                    last.place,
                    "a bare `return;` as a procedure's last line does nothing",
                )
                // trace: D88
                .why("a procedure ends there anyway; `return;` is for leaving early."),
            );
        }
        if let Gives::Value(value_type) = gives
            && !flow.always_returns
        {
            let type_text = value_type.map(Type::name).unwrap_or("value");
            checker.messages.push(
                Message::refused(
                    procedure.header_place,
                    format!(
                        "some way through `{}` reaches its end without `return` giving back a `{type_text}`",
                        procedure.name.text
                    ),
                )
                // trace: D88
                .why("every way through a procedure with `returns` ends in `return` and a value."),
            );
        }

        TypedProcedure {
            name: procedure.name.text.clone(),
            parameters,
            returns: match gives {
                Gives::Value(value_type) => value_type,
                Gives::Nothing => None,
            },
            variables: checker
                .variables
                .iter()
                .map(|variable| variable.value_type.unwrap_or(Type::Whole))
                .collect(),
            body,
        }
    }

    // ----- Variables -----

    fn declare(
        &mut self,
        name: &str,
        value_type: Option<Type>,
        kind: VariableKind,
        place: Place,
    ) -> VariableId {
        let id = self.variables.len();
        self.variables.push(VariableFacts {
            name: name.to_string(),
            value_type,
            kind,
            place,
        });
        if let Some(scope) = self.scopes.last_mut() {
            scope.push(id);
        }
        id
    }

    fn lookup(&self, name: &str) -> Option<VariableId> {
        self.scopes
            .iter()
            .rev()
            .flat_map(|scope| scope.iter().rev())
            .copied()
            .find(|&id| self.variables[id].name == name)
    }

    /// Ends a block: whatever it declared ends with it (D93).
    fn close_scope(&mut self) {
        // trace: D93
        if let Some(scope) = self.scopes.pop() {
            for id in scope {
                let variable = &self.variables[id];
                self.ended.insert(variable.name.clone(), variable.place);
            }
        }
    }

    /// A variable or parameter can't take a procedure's name (D94).
    fn refuse_procedure_name(&mut self, name: &str, place: Place) {
        if let Some(&id) = self.program.by_name.get(name) {
            // trace: D94
            let declared = self.program.signatures[id].place;
            self.messages.push(
                Message::refused(place, format!("`{name}` is already a procedure's name"))
                    .because(format!("the procedure `{name}` is declared here"), declared)
                    .why(format!(
                        "a variable can't take a procedure's name, since `{name}` and `{name}()` would look alike."
                    )),
            );
        }
    }

    /// A name that isn't a variable here, including one whose block has ended (D93).
    fn unknown_name(&mut self, name: &str, place: Place) {
        if let Some(&declared) = self.ended.get(name) {
            // trace: D93
            self.messages.push(
                Message::refused(place, format!("there's no variable named `{name}` here"))
                    .because(
                        format!("`{name}` is declared inside a block that has ended"),
                        declared,
                    )
                    .why("a variable is declared with `let` or `var` before it's used."),
            );
        } else if let Some(&id) = self.program.by_name.get(name) {
            let signature = &self.program.signatures[id];
            let message = if signature.parameters.is_empty() {
                let replacement = format!("{name}()");
                Message::refused_with_fix(place, Edit { place, replacement })
            } else {
                Message::refused(
                    place,
                    format!("`{name}` is a procedure, so using it writes `( )` with its arguments"),
                )
            };
            self.messages
                // trace: D92
                .push(message.why("a call writes the procedure's name and `( )`."));
        } else {
            self.messages.push(
                Message::refused(place, format!("there's no variable named `{name}` here"))
                    // trace: D92
                    .why("a variable is declared with `let` or `var` before it's used."),
            );
        }
    }

    // ----- Blocks and statements -----

    fn check_block(&mut self, statements: &[Statement]) -> (Vec<TypedStatement>, Flow) {
        self.scopes.push(Vec::new());
        let mut typed = Vec::new();
        let mut flow = Flow::default();
        let mut blocked: Option<Blocked> = None;
        let mut reported = false;
        for statement in statements {
            if let Some(reason) = &blocked {
                if !reported {
                    reported = true;
                    let message = match reason {
                        Blocked::Return(place) => Message::refused(
                            statement.place,
                            "this line comes after a `return` in the same block, so it can never run",
                        )
                        .because("the `return` is here", *place)
                        // trace: D88
                        .why("nothing may follow `return` in the same block."),
                        Blocked::Break(place) => Message::not_designed_yet(
                            statement.place,
                            "code after `break` in the same block isn't decided",
                        )
                        .because("the `break` is here", *place),
                        Blocked::IfLeaves(place) => Message::refused(
                            statement.place,
                            "this line follows an `if` that can leave",
                        )
                        .because(format!("the `if` on line {} can leave", place.line), *place)
                        // trace: D88
                        .why("when any way through an `if` leaves, nothing may follow its `end if`; code that should run afterwards goes in the ways that don't leave."),
                    };
                    self.messages.push(message);
                }
                continue;
            }
            let (statement_typed, statement_flow) = self.check_statement(statement);
            if let Some(statement_typed) = statement_typed {
                typed.push(statement_typed);
            }
            flow.returns_somewhere |= statement_flow.returns_somewhere;
            flow.breaks_out |= statement_flow.breaks_out;
            flow.always_returns |= statement_flow.always_returns;
            blocked = match &statement.kind {
                StatementKind::Return(_) => Some(Blocked::Return(statement.place)),
                StatementKind::Break => Some(Blocked::Break(statement.place)),
                StatementKind::If { .. }
                    if statement_flow.returns_somewhere || statement_flow.breaks_out =>
                {
                    Some(Blocked::IfLeaves(statement.place))
                }
                _ => None,
            };
        }
        self.close_scope();
        (typed, flow)
    }

    fn check_statement(&mut self, statement: &Statement) -> (Option<TypedStatement>, Flow) {
        let mut flow = Flow::default();
        let typed = match &statement.kind {
            StatementKind::Declare {
                changeable,
                name,
                type_name,
                value,
            } => {
                let checked = self.value(value);
                if let Some(id) = self.lookup(&name.text) {
                    // trace: D94
                    let earlier = self.variables[id].place;
                    self.messages.push(
                        Message::refused(
                            name.place,
                            format!("`{}` is already in use here", name.text),
                        )
                        .because(format!("`{}` is declared here", name.text), earlier)
                        .why("a name in use can't be declared again; two things get two names."),
                    );
                } else {
                    self.refuse_procedure_name(&name.text, name.place);
                }
                self.expect_type(&checked, type_name.known, |expected, found| {
                    format!(
                        "`{}` is a `{expected}`, but this value is a `{found}`",
                        name.text
                    )
                });
                let kind = if *changeable {
                    VariableKind::Var
                } else {
                    VariableKind::Let
                };
                // A refused declaration still creates its variable, with its written type (D55).
                let variable = self.declare(&name.text, type_name.known, kind, statement.place);
                Some(TypedStatement::Declare {
                    variable,
                    value: checked.expression,
                })
            }
            StatementKind::Change { name, value } => {
                let checked = self.value(value);
                match self.lookup(&name.text) {
                    None => {
                        self.unknown_name(&name.text, name.place);
                        None
                    }
                    Some(id) => {
                        let variable = &self.variables[id];
                        match variable.kind {
                            VariableKind::Let => {
                                // trace: D14
                                let declared = variable.place;
                                self.messages.push(
                                    Message::refused(
                                        name.place,
                                        format!("`{}` can't be given a new value", name.text),
                                    )
                                    .because(
                                        // trace: D87
                                        format!("line {} declares it with `let`", declared.line),
                                        Place {
                                            length: 3,
                                            ..declared
                                        },
                                    )
                                    // trace: D85
                                    .why("only a `var` can be given a new value."),
                                );
                                None
                            }
                            VariableKind::Parameter => {
                                self.messages.push(Message::not_designed_yet(
                                    name.place,
                                    "whether a parameter can be given a new value isn't decided",
                                ));
                                None
                            }
                            VariableKind::Var => {
                                let expected = variable.value_type;
                                self.expect_type(&checked, expected, |expected, found| {
                                    format!(
                                        "`{}` is a `{expected}`, but this value is a `{found}`",
                                        name.text
                                    )
                                });
                                Some(TypedStatement::Change {
                                    variable: id,
                                    value: checked.expression,
                                })
                            }
                        }
                    }
                }
            }
            StatementKind::Expression(expression) => {
                let checked = self.check_expression(expression);
                let checked = self.require_handled(checked);
                match checked.shape {
                    Shape::Nothing => Some(TypedStatement::Run(checked.expression)),
                    Shape::Unknown => None,
                    Shape::Value(_) => {
                        let what = if matches!(
                            expression.kind,
                            ExpressionKind::Call { .. } | ExpressionKind::Slide { .. }
                        ) {
                            "a call whose value isn't used isn't decided"
                        } else {
                            "a statement that's only a value isn't decided"
                        };
                        self.messages
                            .push(Message::not_designed_yet(expression.place, what));
                        None
                    }
                }
            }
            StatementKind::If {
                branches,
                else_body,
            } => {
                let mut typed_branches = Vec::new();
                let mut all_return = true;
                for (condition, body) in branches {
                    let condition = self.condition(condition);
                    let (body, body_flow) = self.check_block(body);
                    flow.returns_somewhere |= body_flow.returns_somewhere;
                    flow.breaks_out |= body_flow.breaks_out;
                    all_return &= body_flow.always_returns;
                    typed_branches.push((condition, body));
                }
                let else_typed = match else_body {
                    Some(body) => {
                        let (body, body_flow) = self.check_block(body);
                        flow.returns_somewhere |= body_flow.returns_somewhere;
                        flow.breaks_out |= body_flow.breaks_out;
                        all_return &= body_flow.always_returns;
                        body
                    }
                    None => {
                        all_return = false;
                        Vec::new()
                    }
                };
                flow.always_returns = all_return;
                Some(TypedStatement::If {
                    branches: typed_branches,
                    else_body: else_typed,
                })
            }
            StatementKind::While { condition, body } => {
                let condition = self.condition(condition);
                self.loop_depth += 1;
                let (body, body_flow) = self.check_block(body);
                self.loop_depth -= 1;
                flow.returns_somewhere = body_flow.returns_somewhere;
                Some(TypedStatement::While { condition, body })
            }
            StatementKind::Break => {
                if self.loop_depth == 0 {
                    // trace: D32
                    self.messages.push(
                        Message::refused(statement.place, "this `break` isn't inside a loop")
                            // trace: D88
                            .why("`break` leaves the innermost loop it's in."),
                    );
                }
                flow.breaks_out = true;
                Some(TypedStatement::Break)
            }
            StatementKind::Return(value) => {
                flow.returns_somewhere = true;
                flow.always_returns = true;
                self.check_return(statement.place, value.as_ref())
            }
            StatementKind::Refused => {
                // Nothing is known about a refused statement, so no error follows from how it might leave (D55).
                flow.always_returns = true;
                None
            }
        };
        (typed, flow)
    }

    /// `return value;` in a procedure with `returns`, and `return;` in one without (D57).
    fn check_return(&mut self, place: Place, value: Option<&Expression>) -> Option<TypedStatement> {
        // trace: D57
        match (self.gives, value) {
            (Gives::Nothing, None) => Some(TypedStatement::Return(None)),
            (Gives::Value(value_type), Some(value)) => {
                let checked = self.value(value);
                let name = self.name.clone();
                self.expect_type(&checked, value_type, |expected, found| {
                    format!("`{name}` gives back a `{expected}`, but this value is a `{found}`")
                });
                Some(TypedStatement::Return(Some(checked.expression)))
            }
            (Gives::Value(value_type), None) => {
                let type_text = value_type.map(Type::name).unwrap_or("value");
                self.messages.push(
                    Message::refused(
                        place,
                        format!(
                            "`{}` gives back a `{type_text}`, so `return` needs one",
                            self.name
                        ),
                    )
                    .why(
                        // trace: D88
                        "in a procedure with `returns`, `return` gives back a value of that type.",
                    ),
                );
                None
            }
            (Gives::Nothing, Some(value)) => {
                let _ = self.value(value);
                self.messages.push(
                    Message::refused(
                        value.place,
                        format!("`{}` has no `returns`, so it gives nothing back", self.name),
                    )
                    // trace: D88
                    .why("only a procedure with `returns` gives back a value."),
                );
                None
            }
        }
    }

    /// A condition for `if` or `while`. Whether it must be a `Bool` isn't decided (standard chapter 7).
    fn condition(&mut self, condition: &Expression) -> TypedExpression {
        let checked = self.value(condition);
        match checked.shape {
            Shape::Value(Type::Bool) | Shape::Unknown => {}
            Shape::Value(other) => {
                self.messages.push(Message::not_designed_yet(
                    condition.place,
                    format!(
                        "whether a condition must be a `Bool` isn't decided, but this is a `{}`",
                        other.name()
                    ),
                ));
            }
            Shape::Nothing => {}
        }
        checked.expression
    }

    /// Refuses a value of the wrong type.
    fn expect_type(
        &mut self,
        checked: &Checked,
        expected: Option<Type>,
        what: impl FnOnce(&str, &str) -> String,
    ) {
        if let (Shape::Value(found), Some(expected)) = (checked.shape, expected)
            && found != expected
        {
            let place = checked.expression.place;
            let what = what(expected.name(), found.name());
            let message = match self.wrap_in_be_text(place, found, expected) {
                Some(edit) => Message::refused_with_fix(place, edit)
                    .why("a number becomes text only where `be.Text` says so."),
                None => Message::refused(place, what)
                    // trace: D92
                    .why("a value's type has to match where it's used."),
            };
            self.messages.push(message);
        }
    }

    /// The fix `be.Text(...)` around a `Whole` where `Text` is needed (D22, D28), when the value is on one line.
    fn wrap_in_be_text(&self, place: Place, found: Type, expected: Type) -> Option<Edit> {
        if found != Type::Whole || expected != Type::Text {
            return None;
        }
        let written = self.written_at(place)?;
        Some(Edit {
            place,
            replacement: format!("be.Text({written})"),
        })
    }

    fn written_at(&self, place: Place) -> Option<String> {
        let source = self.program.sources.get(place.file)?;
        let line: Vec<char> = source.line_text(place.line).chars().collect();
        let start = place.column.checked_sub(1)?;
        let end = start + place.length;
        (end <= line.len()).then(|| line[start..end].iter().collect())
    }

    // ----- Expressions -----

    /// An expression used as a value: a failure must be dealt with (D23), and a call that gives nothing back
    /// can't be used (D38).
    fn value(&mut self, expression: &Expression) -> Checked {
        let checked = self.check_expression(expression);
        let checked = self.require_handled(checked);
        self.require_value(checked)
    }

    fn require_handled(&mut self, mut checked: Checked) -> Checked {
        if checked.can_fail {
            // trace: D23
            self.messages.push(
                Message::refused(
                    checked.expression.place,
                    "this call can fail, and nothing here deals with the failure",
                )
                // trace: D87
                .why("every call that can fail deals with its failure, with `?`, `??` or `otherwise:`."),
            );
            checked.can_fail = false;
            checked.shape = Shape::Unknown;
        }
        checked
    }

    fn require_value(&mut self, checked: Checked) -> Checked {
        if checked.shape == Shape::Nothing {
            // trace: D38
            self.messages.push(
                Message::refused(
                    checked.expression.place,
                    "this call gives nothing back, so it can't be used as a value",
                )
                // trace: D88
                .why("a procedure with no `returns` gives nothing back."),
            );
            return Checked::unknown(checked.expression.place);
        }
        checked
    }

    fn check_expression(&mut self, expression: &Expression) -> Checked {
        let place = expression.place;
        match &expression.kind {
            ExpressionKind::Whole(value) => Checked::new(
                TypedExpressionKind::Whole(*value),
                Shape::Value(Type::Whole),
                place,
            ),
            ExpressionKind::Text(characters) => {
                // trace: D21, D76
                let text = characters
                    .iter()
                    .map(|character| character.character)
                    .collect();
                Checked::new(
                    TypedExpressionKind::Text(text),
                    Shape::Value(Type::Text),
                    place,
                )
            }
            ExpressionKind::Bool(value) => Checked::new(
                TypedExpressionKind::Bool(*value),
                Shape::Value(Type::Bool),
                place,
            ),
            ExpressionKind::Name(name) => match self.lookup(name) {
                Some(id) => match self.variables[id].value_type {
                    Some(value_type) => Checked::new(
                        TypedExpressionKind::Variable(id),
                        Shape::Value(value_type),
                        place,
                    ),
                    None => Checked::unknown(place),
                },
                None => {
                    self.unknown_name(name, place);
                    Checked::unknown(place)
                }
            },
            ExpressionKind::BareNarrowed(callee)
                if self.lookup(&callee.words[0].text).is_some() =>
            {
                self.messages.push(Message::not_built_yet(
                    place,
                    "facts about a value, like a list's `.len` (D79), aren't built yet",
                ));
                Checked::unknown(place)
            }
            ExpressionKind::BareNarrowed(callee) => {
                self.messages.push(
                    Message::refused(
                        place,
                        format!("`{}` is a procedure, so using it writes `( )` with its arguments", callee.written()),
                    )
                    .why("a call writes the procedure's name and `( )`; only a slide's step is written bare."),
                );
                Checked::unknown(place)
            }
            ExpressionKind::Call { callee, arguments } => {
                self.check_call(callee, None, arguments, place)
            }
            ExpressionKind::Binary {
                operator,
                operator_place,
                left,
                right,
            } => self.check_binary(*operator, *operator_place, left, right, place),
            ExpressionKind::Negate(value) => {
                let checked = self.value(value);
                match checked.shape {
                    Shape::Value(Type::Whole) => Checked::new(
                        TypedExpressionKind::Negate(checked.boxed()),
                        Shape::Value(Type::Whole),
                        place,
                    ),
                    Shape::Value(other) => {
                        // trace: D27
                        self.messages.push(
                            Message::refused(
                                place,
                                format!(
                                    "`-` makes a number negative, but this is a `{}`",
                                    other.name()
                                ),
                            )
                            // trace: D89
                            .why("a `-` with no space after it makes a number negative."),
                        );
                        Checked::unknown(place)
                    }
                    _ => Checked::unknown(place),
                }
            }
            ExpressionKind::Not(value) => {
                let checked = self.value(value);
                match checked.shape {
                    Shape::Value(Type::Bool) => Checked::new(
                        TypedExpressionKind::Not(checked.boxed()),
                        Shape::Value(Type::Bool),
                        place,
                    ),
                    Shape::Value(other) => {
                        self.messages.push(Message::not_designed_yet(
                            value.place,
                            format!(
                                "whether a condition must be a `Bool` isn't decided, but this is a `{}`",
                                other.name()
                            ),
                        ));
                        Checked::unknown(place)
                    }
                    _ => Checked::unknown(place),
                }
            }
            ExpressionKind::Fallback { call, fallback } => {
                let checked = self.check_expression(call);
                let checked_fallback = self.value(fallback);
                if checked.shape == Shape::Unknown {
                    return Checked::unknown(place);
                }
                if !checked.can_fail {
                    self.messages.push(Message::not_designed_yet(
                        call.place,
                        "`??` after a call that can't fail isn't decided",
                    ));
                    return Checked::unknown(place);
                }
                let expected = match checked.shape {
                    Shape::Value(value_type) => Some(value_type),
                    _ => None,
                };
                // trace: D23
                self.expect_type(&checked_fallback, expected, |expected, found| {
                    // trace: D89
                    format!("the call gives a `{expected}`, so the value after `??` must be one too, not a `{found}`")
                });
                let shape = checked.shape;
                Checked::new(
                    TypedExpressionKind::Fallback {
                        call: checked.boxed(),
                        fallback: checked_fallback.boxed(),
                    },
                    shape,
                    place,
                )
            }
            ExpressionKind::Slide { first, steps } => {
                // trace: D5
                let mut value = self.value(first);
                let count = steps.len();
                for (index, step) in steps.iter().enumerate() {
                    value = self.check_call(&step.callee, Some(value), &step.arguments, step.place);
                    if index + 1 < count {
                        value = self.require_handled(value);
                        value = self.require_value(value);
                    }
                }
                Checked {
                    expression: TypedExpression {
                        place,
                        ..value.expression
                    },
                    ..value
                }
            }
            ExpressionKind::Grouped(inner) => self.check_expression(inner),
            ExpressionKind::Refused => Checked::unknown(place),
        }
    }

    fn check_binary(
        &mut self,
        operator: BinaryOperator,
        operator_place: Place,
        left: &Expression,
        right: &Expression,
        place: Place,
    ) -> Checked {
        let left_checked = self.value(left);
        let right_checked = self.value(right);
        let (left_shape, right_shape) = (left_checked.shape, right_checked.shape);
        if left_shape == Shape::Unknown || right_shape == Shape::Unknown {
            return Checked::unknown(place);
        }
        let types = match (left_shape, right_shape) {
            (Shape::Value(left_type), Shape::Value(right_type)) => (left_type, right_type),
            _ => return Checked::unknown(place),
        };
        let written = operator.written();
        match operator {
            BinaryOperator::Add | BinaryOperator::Subtract | BinaryOperator::Multiply => {
                if types == (Type::Whole, Type::Whole) {
                    let arithmetic = match operator {
                        BinaryOperator::Add => Arithmetic::Add,
                        BinaryOperator::Subtract => Arithmetic::Subtract,
                        _ => Arithmetic::Multiply,
                    };
                    // trace: D27, D59
                    return Checked::new(
                        TypedExpressionKind::Arithmetic {
                            operator: arithmetic,
                            left: left_checked.boxed(),
                            right: right_checked.boxed(),
                        },
                        Shape::Value(Type::Whole),
                        place,
                    );
                }
                let message =
                    if operator == BinaryOperator::Add && types == (Type::Text, Type::Text) {
                        // trace: D27, D28
                        let edit = Edit {
                            place: operator_place,
                            replacement: "&".to_string(),
                        };
                        Message::refused_with_fix(operator_place, edit)
                            // trace: D87
                            .why("`+` is only for numbers, and `&` joins text.")
                    } else {
                        Message::refused(
                            operator_place,
                            format!(
                                "`{written}` works on two `Whole`s, but this is a `{}` and a `{}`",
                                types.0.name(),
                                types.1.name()
                            ),
                        )
                        // trace: D89
                        .why("`+`, `-` and `*` are only for numbers.")
                    };
                self.messages.push(message);
                Checked::unknown(place)
            }
            BinaryOperator::Join => {
                // trace: D28
                if types == (Type::Text, Type::Text) {
                    return Checked::new(
                        TypedExpressionKind::Join(left_checked.boxed(), right_checked.boxed()),
                        Shape::Value(Type::Text),
                        place,
                    );
                }
                for (side, side_type) in [(&left_checked, types.0), (&right_checked, types.1)] {
                    if side_type == Type::Text {
                        continue;
                    }
                    let side_place = side.expression.place;
                    let message = match self.wrap_in_be_text(side_place, side_type, Type::Text) {
                        // trace: D87
                        Some(edit) => Message::refused_with_fix(side_place, edit)
                            .why("`&` joins two pieces of text, and a number becomes text only where `be.Text` says so."),
                        // trace: D89
                        None => Message::refused(
                            side_place,
                            format!("`&` joins text, but this is a `{}`", side_type.name()),
                        )
                        .why("`&` joins two pieces of text, and anything else becomes text only where `be.Text` says so."),
                    };
                    self.messages.push(message);
                }
                Checked::unknown(place)
            }
            BinaryOperator::And | BinaryOperator::Or => {
                if types == (Type::Bool, Type::Bool) {
                    // trace: D34
                    let kind = if operator == BinaryOperator::And {
                        TypedExpressionKind::And(left_checked.boxed(), right_checked.boxed())
                    } else {
                        TypedExpressionKind::Or(left_checked.boxed(), right_checked.boxed())
                    };
                    return Checked::new(kind, Shape::Value(Type::Bool), place);
                }
                self.messages.push(Message::not_designed_yet(
                    operator_place,
                    format!(
                        "whether `{written}` works on anything but `Bool`s isn't decided, but this is a `{}` and a `{}`",
                        types.0.name(),
                        types.1.name()
                    ),
                ));
                Checked::unknown(place)
            }
            _ => {
                // A comparison (D17).
                let comparison = match operator {
                    BinaryOperator::Equal => Comparison::Equal,
                    BinaryOperator::NotEqual => Comparison::NotEqual,
                    BinaryOperator::Less => Comparison::Less,
                    BinaryOperator::Greater => Comparison::Greater,
                    BinaryOperator::AtLeast => Comparison::AtLeast,
                    _ => Comparison::AtMost,
                };
                let allowed = match types {
                    (Type::Whole, Type::Whole) => true,
                    (Type::Bool, Type::Bool) => {
                        matches!(comparison, Comparison::Equal | Comparison::NotEqual)
                    }
                    _ => false,
                };
                if allowed {
                    // trace: D17
                    return Checked::new(
                        TypedExpressionKind::Compare {
                            operator: comparison,
                            left: left_checked.boxed(),
                            right: right_checked.boxed(),
                        },
                        Shape::Value(Type::Bool),
                        place,
                    );
                }
                let message = if types.0 != types.1 {
                    Message::refused(
                        operator_place,
                        format!(
                            // trace: D89
                            "`{written}` compares two values of one type, but this is a `{}` and a `{}`",
                            types.0.name(),
                            types.1.name()
                        ),
                    )
                    // trace: D87
                    .why("a comparison is between two values of the same type.")
                } else if types.0 == Type::Text {
                    Message::not_designed_yet(
                        operator_place,
                        "comparing two pieces of text isn't decided",
                    )
                } else {
                    Message::not_designed_yet(
                        operator_place,
                        format!(
                            "comparing `{}`s with `{written}` isn't decided",
                            types.0.name()
                        ),
                    )
                };
                self.messages.push(message);
                Checked::unknown(place)
            }
        }
    }

    // ----- Calls -----

    /// A call, or a slide's step when `slid` holds the value passed in as its first argument (D5).
    fn check_call(
        &mut self,
        callee: &Callee,
        slid: Option<Checked>,
        arguments: &[Argument],
        place: Place,
    ) -> Checked {
        let words: Vec<&str> = callee.words.iter().map(|word| word.text.as_str()).collect();
        match words.as_slice() {
            ["show_msg", narrowings @ ..] => {
                self.check_show_msg(callee, narrowings, slid, arguments, place)
            }
            ["ask"] => {
                if slid.is_none() && arguments.is_empty() {
                    self.messages.push(Message::not_designed_yet(
                        place,
                        "asking with no question isn't decided",
                    ));
                    return Checked::unknown(place);
                }
                // trace: D20
                self.library_call(
                    LibraryProcedure::Ask,
                    "ask",
                    &[(None, Some(Type::Text))],
                    slid,
                    arguments,
                    place,
                    Shape::Value(Type::Text),
                )
            }
            ["random"] => {
                // trace: D26
                self.library_call(
                    LibraryProcedure::Random,
                    "random",
                    &[
                        (Some("from"), Some(Type::Whole)),
                        (Some("to"), Some(Type::Whole)),
                    ],
                    slid,
                    arguments,
                    place,
                    Shape::Value(Type::Whole),
                )
            }
            ["be", target, rest @ ..] => {
                self.check_be(callee, target, rest, slid, arguments, place)
            }
            ["be"] => {
                // trace: D22
                self.messages.push(
                    Message::refused(
                        place,
                        "`be` needs the type the value becomes, as in `be.Text`",
                    )
                    // trace: D87
                    .why("a value is converted with `be` narrowed by the type it becomes."),
                );
                Checked::unknown(place)
            }
            [name] => self.check_user_call(name, callee, slid, arguments, place),
            [name, _, ..] if self.lookup(name).is_some() => {
                self.messages.push(Message::not_built_yet(
                    callee.place,
                    "methods, called on a value with `.` (D77), aren't built yet",
                ));
                self.check_arguments_alone(slid, arguments);
                Checked::unknown(place)
            }
            [name, ..] if self.program.by_name.contains_key(*name) => {
                self.messages.push(Message::not_built_yet(
                    callee.place,
                    "narrowed procedures, declared in families (D41), aren't built yet",
                ));
                self.check_arguments_alone(slid, arguments);
                Checked::unknown(place)
            }
            _ => {
                self.messages.push(
                    Message::refused(
                        callee.place,
                        format!("there's no procedure named `{}`", callee.written()),
                    )
                    // trace: D92
                    .why("a call names a procedure the program or the library has."),
                );
                self.check_arguments_alone(slid, arguments);
                Checked::unknown(place)
            }
        }
    }

    /// Checks arguments for their own errors, for a call that was refused.
    fn check_arguments_alone(&mut self, _slid: Option<Checked>, arguments: &[Argument]) {
        for argument in arguments {
            let _ = self.value(&argument.value);
        }
    }

    fn check_user_call(
        &mut self,
        name: &str,
        callee: &Callee,
        slid: Option<Checked>,
        arguments: &[Argument],
        place: Place,
    ) -> Checked {
        let Some(&id) = self.program.by_name.get(name) else {
            let message = if self.lookup(name).is_some() {
                Message::refused(
                    callee.place,
                    format!("`{name}` is a variable, not a procedure"),
                )
                // trace: D92
                .why("only a procedure can be called.")
            } else {
                Message::refused(callee.place, format!("there's no procedure named `{name}`"))
                    // trace: D92
                    .why("a call names a procedure the program or the library has.")
            };
            self.messages.push(message);
            self.check_arguments_alone(slid, arguments);
            return Checked::unknown(place);
        };
        let signature = &self.program.signatures[id];
        let parameters: Vec<(Option<String>, Option<Type>)> = signature.parameters.clone();
        let shape = match signature.gives {
            Gives::Nothing => Shape::Nothing,
            Gives::Value(Some(value_type)) => Shape::Value(value_type),
            Gives::Value(None) => Shape::Unknown,
        };
        let declared = signature.place;
        let parameters_borrowed: Vec<(Option<&str>, Option<Type>)> = parameters
            .iter()
            .map(|(label, value_type)| (label.as_deref(), *value_type))
            .collect();
        match self.match_arguments(
            name,
            &parameters_borrowed,
            slid,
            arguments,
            place,
            Some(declared),
        ) {
            Some(checked) => Checked::new(
                TypedExpressionKind::Call {
                    procedure: id,
                    arguments: checked
                        .into_iter()
                        .map(|argument| argument.expression)
                        .collect(),
                },
                shape,
                place,
            ),
            None => Checked::unknown(place),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn library_call(
        &mut self,
        procedure: LibraryProcedure,
        name: &str,
        parameters: &[(Option<&str>, Option<Type>)],
        slid: Option<Checked>,
        arguments: &[Argument],
        place: Place,
        shape: Shape,
    ) -> Checked {
        match self.match_arguments(name, parameters, slid, arguments, place, None) {
            Some(checked) => Checked::new(
                TypedExpressionKind::Library {
                    procedure,
                    arguments: checked
                        .into_iter()
                        .map(|argument| argument.expression)
                        .collect(),
                },
                shape,
                place,
            ),
            None => Checked::unknown(place),
        }
    }

    /// Matches a call's arguments to the procedure's parameters, with every label written (D24). A parameter
    /// whose type is `None` takes any type; the caller checks it.
    fn match_arguments(
        &mut self,
        name: &str,
        parameters: &[(Option<&str>, Option<Type>)],
        slid: Option<Checked>,
        arguments: &[Argument],
        place: Place,
        declared: Option<Place>,
    ) -> Option<Vec<Checked>> {
        let mut checked = Vec::new();
        let offset = usize::from(slid.is_some());
        if let Some(slid) = slid {
            if let Some((Some(label), _)) = parameters.first() {
                self.messages.push(Message::not_designed_yet(
                    place,
                    format!("sliding a value into a first parameter with a label, `{label}:`, isn't decided"),
                ));
                self.check_arguments_alone(None, arguments);
                return None;
            }
            checked.push(slid);
        }

        // Every label is there but in another order: label order isn't decided (standard chapter 4).
        let written: Vec<&str> = arguments
            .iter()
            .filter_map(|argument| argument.label.as_ref().map(|label| label.text.as_str()))
            .collect();
        let expected: Vec<&str> = parameters
            .iter()
            .skip(offset)
            .filter_map(|(label, _)| *label)
            .collect();
        let mut written_sorted = written.clone();
        let mut expected_sorted = expected.clone();
        written_sorted.sort_unstable();
        expected_sorted.sort_unstable();
        if written != expected && written_sorted == expected_sorted && !written.is_empty() {
            self.messages.push(Message::not_designed_yet(
                place,
                "writing labels in a different order from the procedure's isn't decided",
            ));
            self.check_arguments_alone(None, arguments);
            return None;
        }

        let given = offset + arguments.len();
        if given != parameters.len() {
            let what = format!(
                // trace: D89
                "`{name}` takes {}, but this call gives {}",
                count(parameters.len(), "value"),
                count(given, "value"),
            );
            let mut message = Message::refused(place, what)
                // trace: D88
                .why("a call gives every parameter a value, with its label if it has one.");
            if let Some(declared) = declared {
                message = message.because(format!("`{name}` is declared here"), declared);
            }
            self.messages.push(message);
            self.check_arguments_alone(None, arguments);
            return None;
        }

        let mut refused = false;
        for (argument, (label, parameter_type)) in
            arguments.iter().zip(parameters.iter().skip(offset))
        {
            // trace: D24
            match (label, &argument.label) {
                (None, None) => {}
                (Some(label), Some(written)) if *label == written.text => {}
                (Some(label), None) => {
                    let value_place = argument.value.place;
                    let edit = Edit {
                        place: Place {
                            length: 0,
                            ..value_place
                        },
                        replacement: format!("{label}: "),
                    };
                    self.messages.push(
                        Message::refused_with_fix(value_place, edit)
                            // trace: D88
                            .why("every call writes the labels its procedure chose."),
                    );
                    refused = true;
                }
                (None, Some(written)) => {
                    let edit = Edit {
                        place: Place {
                            length: argument.value.place.column - written.place.column,
                            ..written.place
                        },
                        replacement: String::new(),
                    };
                    self.messages.push(
                        Message::refused_with_fix(written.place, edit)
                            // trace: D88
                            .why("a call writes only the labels its procedure chose."),
                    );
                    refused = true;
                }
                (Some(label), Some(written)) => {
                    self.messages.push(
                        Message::refused(
                            written.place,
                            format!("this argument's label is `{label}`, not `{}`", written.text),
                        )
                        // trace: D88
                        .why("every call writes the labels its procedure chose."),
                    );
                    refused = true;
                }
            }
            let value = self.value(&argument.value);
            self.expect_type(&value, *parameter_type, |expected, found| {
                format!("`{name}` takes a `{expected}` here, but this is a `{found}`")
            });
            if let (Shape::Value(found), Some(expected)) = (value.shape, parameter_type)
                && found != *expected
            {
                refused = true;
            }
            checked.push(value);
        }
        if let (Some(first), Some((_, Some(expected)))) = (checked.first(), parameters.first())
            && offset == 1
            && let Shape::Value(found) = first.shape
            && found != *expected
        {
            self.messages.push(
                Message::refused(
                    first.expression.place,
                    format!(
                        "`{name}` takes a `{}` first, but the slid value is a `{}`",
                        expected.name(),
                        found.name()
                    ),
                )
                // trace: D89
                .why("a slide passes the value on its left in as the step's first argument."),
            );
            refused = true;
        }
        (!refused).then_some(checked)
    }

    /// `show_msg` and its narrowings `.fmt` and `.same_line`, strongest first (D6, D29, D31).
    fn check_show_msg(
        &mut self,
        callee: &Callee,
        narrowings: &[&str],
        slid: Option<Checked>,
        arguments: &[Argument],
        place: Place,
    ) -> Checked {
        let mut format = false;
        let mut same_line = false;
        for (index, narrowing) in narrowings.iter().enumerate() {
            let word = &callee.words[index + 1];
            let seen = match *narrowing {
                "fmt" => &mut format,
                "same_line" => &mut same_line,
                _ => {
                    self.messages.push(
                        Message::refused(
                            word.place,
                            format!("`show_msg` has no narrowing `.{narrowing}`"),
                        )
                        .why("`show_msg` is narrowed by `.fmt` and `.same_line`."),
                    );
                    self.check_arguments_alone(slid, arguments);
                    return Checked::unknown(place);
                }
            };
            if *seen {
                self.messages.push(
                    Message::refused(word.place, format!("`.{narrowing}` is written twice"))
                        .why("each narrowing is written once."),
                );
                self.check_arguments_alone(slid, arguments);
                return Checked::unknown(place);
            }
            *seen = true;
        }
        if narrowings == ["same_line", "fmt"] {
            // trace: D31
            let replacement = "show_msg.fmt.same_line".to_string();
            self.messages.push(
                Message::refused_with_fix(
                    callee.place,
                    Edit {
                        place: callee.place,
                        replacement,
                    },
                )
                // trace: D89
                .why("narrowings go strongest first: `.fmt` changes what's shown, `.same_line` only where it ends."),
            );
        }
        // trace: D6
        let procedure = LibraryProcedure::ShowMessage { same_line };
        if format {
            return match self.check_format(slid, arguments, place) {
                Some(pieces) => {
                    let text = Checked::new(
                        TypedExpressionKind::Format(pieces),
                        Shape::Value(Type::Text),
                        place,
                    );
                    Checked::new(
                        TypedExpressionKind::Library {
                            procedure,
                            arguments: vec![text.expression],
                        },
                        Shape::Nothing,
                        place,
                    )
                }
                None => Checked::unknown(place),
            };
        }
        self.library_call(
            procedure,
            &callee.written(),
            &[(None, Some(Type::Text))],
            slid,
            arguments,
            place,
            Shape::Nothing,
        )
    }

    /// `be.Text`, `be.Text.fmt` and `be.Whole` (D22, D29).
    fn check_be(
        &mut self,
        callee: &Callee,
        target: &str,
        rest: &[&str],
        slid: Option<Checked>,
        arguments: &[Argument],
        place: Place,
    ) -> Checked {
        let target_type = match target {
            "Text" => Type::Text,
            "Whole" => Type::Whole,
            "Bool" => {
                self.messages.push(Message::not_designed_yet(
                    callee.place,
                    "converting to `Bool` isn't decided",
                ));
                self.check_arguments_alone(slid, arguments);
                return Checked::unknown(place);
            }
            _ => {
                self.messages.push(Message::not_designed_yet(
                    callee.place,
                    format!("types other than `Whole`, `Bool` and `Text` aren't decided, so `be.{target}` can't be used yet"),
                ));
                self.check_arguments_alone(slid, arguments);
                return Checked::unknown(place);
            }
        };
        match rest {
            [] => {}
            ["fmt"] if target_type == Type::Text => {
                // trace: D29
                return match self.check_format(slid, arguments, place) {
                    Some(pieces) => Checked::new(
                        TypedExpressionKind::Format(pieces),
                        Shape::Value(Type::Text),
                        place,
                    ),
                    None => Checked::unknown(place),
                };
            }
            _ => {
                self.messages.push(
                    Message::refused(
                        callee.place,
                        format!("there's no procedure named `{}`", callee.written()),
                    )
                    .why("`be.Text` is narrowed only by `.fmt`."),
                );
                self.check_arguments_alone(slid, arguments);
                return Checked::unknown(place);
            }
        }
        let written = callee.written();
        let Some(mut checked) =
            self.match_arguments(&written, &[(None, None)], slid, arguments, place, None)
        else {
            return Checked::unknown(place);
        };
        let Some(value) = checked.pop() else {
            return Checked::unknown(place);
        };
        let value_type = match value.shape {
            Shape::Value(value_type) => value_type,
            _ => return Checked::unknown(place),
        };
        // trace: D22
        let procedure = match (value_type, target_type) {
            (Type::Whole, Type::Text) => LibraryProcedure::WholeToText,
            (Type::Text, Type::Whole) => LibraryProcedure::TextToWhole,
            (from, to) if from == to => {
                self.messages.push(
                    Message::refused(
                        place,
                        format!("this value is already a `{}`, so `{written}` does nothing", to.name()),
                    )
                    // trace: D87
                    .why("converting a value to the type it already is does nothing, so it's refused."),
                );
                return Checked::unknown(place);
            }
            (from, to) => {
                self.messages.push(Message::not_designed_yet(
                    place,
                    format!(
                        "converting a `{}` to `{}` isn't decided",
                        from.name(),
                        to.name()
                    ),
                ));
                return Checked::unknown(place);
            }
        };
        let mut result = Checked::new(
            TypedExpressionKind::Library {
                procedure,
                arguments: vec![value.expression],
            },
            Shape::Value(target_type),
            place,
        );
        result.can_fail = procedure == LibraryProcedure::TextToWhole;
        result
    }

    /// The text given to `.fmt`, with its braces read (D29, D30). It must be written in the call, so it can be
    /// checked at compile time.
    fn check_format(
        &mut self,
        slid: Option<Checked>,
        arguments: &[Argument],
        place: Place,
    ) -> Option<Vec<FormatPiece>> {
        if slid.is_some() {
            self.messages.push(Message::not_designed_yet(
                place,
                "`.fmt` as a slide's step isn't decided",
            ));
            return None;
        }
        let [argument] = arguments else {
            self.messages.push(
                Message::refused(place, "`.fmt` takes one piece of text")
                    // trace: D92
                    .why("`.fmt` fills in the braces of the text it's given."),
            );
            self.check_arguments_alone(None, arguments);
            return None;
        };
        if let Some(label) = &argument.label {
            self.messages.push(
                Message::refused(label.place, "`.fmt`'s text has no label")
                    // trace: D88
                    .why("a call writes only the labels its procedure chose."),
            );
            return None;
        }
        let ExpressionKind::Text(characters) = &argument.value.kind else {
            self.messages.push(Message::not_designed_yet(
                argument.value.place,
                "`.fmt` with text that isn't written in the call isn't decided, since its braces are checked at compile time",
            ));
            let _ = self.value(&argument.value);
            return None;
        };
        self.read_braces(characters, argument.value.place)
    }

    fn read_braces(
        &mut self,
        characters: &[TextCharacter],
        text_place: Place,
    ) -> Option<Vec<FormatPiece>> {
        let mut pieces = Vec::new();
        let mut plain = String::new();
        let mut index = 0;
        let mut refused = false;
        while index < characters.len() {
            let character = characters[index];
            if character.character != '{' || character.escaped {
                plain.push(character.character);
                index += 1;
                continue;
            }
            let Some(length) = characters[index + 1..]
                .iter()
                .position(|other| other.character == '}' && !other.escaped)
            else {
                self.messages.push(Message::not_designed_yet(
                    Place {
                        column: character.column,
                        length: 1,
                        ..text_place
                    },
                    "a `{` with no `}` after it in `.fmt` text isn't decided; `\\{` writes a brace",
                ));
                return None;
            };
            let inside: String = characters[index + 1..index + 1 + length]
                .iter()
                .map(|other| other.character)
                .collect();
            let closing = characters[index + 1 + length];
            let brace_place = Place {
                column: character.column,
                length: closing.column + 1 - character.column,
                ..text_place
            };
            index += length + 2;
            if !plain.is_empty() {
                pieces.push(FormatPiece::Text(std::mem::take(&mut plain)));
            }
            // trace: D29
            if inside
                .chars()
                .all(|letter| letter.is_ascii_lowercase() || letter == '_')
                && !inside.is_empty()
            {
                match self.lookup(&inside) {
                    Some(id) => match self.variables[id].value_type {
                        Some(Type::Whole) => pieces.push(FormatPiece::Variable(id, Type::Whole)),
                        Some(Type::Text) => pieces.push(FormatPiece::Variable(id, Type::Text)),
                        Some(Type::Bool) => {
                            self.messages.push(Message::not_designed_yet(
                                brace_place,
                                "a `Bool` in `.fmt` braces isn't decided",
                            ));
                            refused = true;
                        }
                        None => refused = true,
                    },
                    None => {
                        let message = Message::refused(
                            brace_place,
                            format!("there's no variable named `{inside}` here"),
                        )
                        // trace: D92
                        .why("braces in `.fmt` text hold a variable's name, checked at compile time.");
                        self.messages.push(message);
                        refused = true;
                    }
                }
            } else if inside.contains(['+', '-', '*', '&', '(', ')'])
                || (!inside.is_empty() && inside.chars().all(|digit| digit.is_ascii_digit()))
            {
                self.messages.push(
                    Message::refused(brace_place, "braces in `.fmt` text hold only a variable's name")
                        // trace: D92
                        .why("text never hides a calculation or a call; work it out into a variable first."),
                );
                refused = true;
            } else {
                self.messages.push(Message::not_designed_yet(
                    brace_place,
                    format!(
                        "`{{{inside}}}` in `.fmt` text isn't decided; braces hold a variable's name"
                    ),
                ));
                refused = true;
            }
        }
        if !plain.is_empty() {
            pieces.push(FormatPiece::Text(plain));
        }
        (!refused).then_some(pieces)
    }
}

fn count(number: usize, noun: &str) -> String {
    if number == 1 {
        format!("1 {noun}")
    } else {
        format!("{number} {noun}s")
    }
}
