//! The parser: tokens to the syntax tree (D62), with the layout rules that need no types: indentation (D10),
//! spacing (D17, D18, D27, D59), `;` (D7) and closers (D8).

use crate::message::{Edit, Message, RemovedLines};
use crate::source::{Place, SourceFile};
use crate::syntax_tree::{
    Argument, BinaryOperator, Callee, Expression, ExpressionKind, LabelPair, Name, Parameter,
    Procedure, Statement, StatementKind, Step, Type, TypeName,
};
use crate::tokenizer::{LineFacts, Symbol, Token, TokenKind, Tokens};

/// Every keyword decided so far (standard chapter 3). A name can't be one (D58).
const KEYWORDS: &[&str] = &[
    "proc",
    "end",
    "let",
    "var",
    "if",
    "else",
    "at_least",
    "at_most",
    "while",
    "break",
    "true",
    "false",
    "and",
    "or",
    "not",
    "be",
    "ignore",
    "crash",
    "otherwise",
    "case",
    "instead",
    "labels",
    "as",
    "extern",
    "returns",
    "return",
    "fam",
    "handler",
    "include",
    "copy",
    "share",
    "fails",
    "div",
    "rem",
    "comment",
];

/// One level of indentation is 4 spaces (D10).
const INDENTATION_LEVEL: usize = 4;

pub fn is_keyword(word: &str) -> bool {
    KEYWORDS.contains(&word)
}

/// What a token does where it's used, for the spacing rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Role {
    Plain,
    /// Between two values: `+`, `-`, `*`, `&`, a comparison, `and`, `or`, `|>`, `??`, `=`.
    BinaryOperator,
    /// A `-` right before a value, negating it (D27).
    PrefixMinus,
    /// The `(` of a call, a definition or a step, touching the name before it.
    CallOpen,
}

/// The statement or line was refused and a message given; the caller skips it (D55).
struct Skip;

pub struct ParsedFile {
    pub procedures: Vec<Procedure>,
    /// Whether a broken opening line stopped checking (D55).
    pub stopped: bool,
}

pub fn parse_file(
    file: usize,
    source: &SourceFile,
    tokens: Tokens,
    messages: &mut Vec<Message>,
) -> ParsedFile {
    let roles = vec![Role::Plain; tokens.tokens.len()];
    let mut parser = Parser {
        tokens: &tokens.tokens,
        lines: &tokens.lines,
        source,
        file,
        position: 0,
        messages,
        roles,
        stopped: false,
    };
    let procedures = parser.parse_top_level();
    parser.check_spacing();
    parser.check_blank_lines();
    ParsedFile {
        procedures,
        stopped: parser.stopped,
    }
}

struct Parser<'a> {
    tokens: &'a [Token],
    lines: &'a [LineFacts],
    source: &'a SourceFile,
    file: usize,
    position: usize,
    messages: &'a mut Vec<Message>,
    roles: Vec<Role>,
    /// Set when a broken opening line leaves it unclear what was meant, so checking stops (D55).
    stopped: bool,
}

impl<'a> Parser<'a> {
    // ----- Tokens -----

    fn current(&self) -> &'a Token {
        let tokens: &'a [Token] = self.tokens;
        &tokens[self.position.min(tokens.len() - 1)]
    }

    fn peek(&self, offset: usize) -> &'a Token {
        let tokens: &'a [Token] = self.tokens;
        &tokens[(self.position + offset).min(tokens.len() - 1)]
    }

    fn previous(&self) -> &'a Token {
        let tokens: &'a [Token] = self.tokens;
        &tokens[self.position.saturating_sub(1)]
    }

    fn at_end(&self) -> bool {
        self.current().kind == TokenKind::EndOfFile
    }

    fn advance(&mut self) -> &'a Token {
        let token = self.current();
        if !self.at_end() {
            self.position += 1;
        }
        token
    }

    fn set_role(&mut self, role: Role) {
        if let Some(slot) = self.roles.get_mut(self.position) {
            *slot = role;
        }
    }

    fn at_symbol(&self, symbol: Symbol) -> bool {
        self.current().is_symbol(symbol)
    }

    fn at_word(&self, word: &str) -> bool {
        self.current().is_word(word)
    }

    /// Whether the current token starts a new line, or the file has ended.
    fn at_line_start(&self) -> bool {
        self.current().first_on_line
    }

    fn skip_rest_of_line(&mut self) {
        self.advance();
        while !self.at_line_start() {
            self.advance();
        }
    }

    /// Whether the line the current token is on ends with `:`.
    fn line_ends_with_colon(&self) -> bool {
        let mut index = self.position;
        let tokens = self.tokens;
        while index + 1 < tokens.len() && !tokens[index + 1].first_on_line {
            index += 1;
        }
        tokens[index].is_symbol(Symbol::Colon)
    }

    fn place_after(&self, token: &Token) -> Place {
        Place {
            column: token.place.column + token.place.length,
            length: 1,
            ..token.place
        }
    }

    /// The source text at `place`, when it's on one line.
    fn written_at(&self, place: Place) -> Option<String> {
        let line: Vec<char> = self.source.line_text(place.line).chars().collect();
        let start = place.column.checked_sub(1)?;
        let end = start + place.length;
        (end <= line.len()).then(|| line[start..end].iter().collect())
    }

    fn refuse_with_edit(&mut self, place: Place, edit: Edit, why: &str) {
        self.messages
            .push(Message::refused_with_fix(place, edit).why(why));
    }

    fn refuse_unexpected(&mut self, expected: &str) -> Skip {
        let token = self.current();
        let message = if token.kind == TokenKind::EndOfFile {
            Message::refused(
                token.place,
                // trace: D92
                format!("the file ends where {expected} should be"),
            )
        } else if token.first_on_line {
            Message::refused(
                self.place_after(self.previous()),
                // trace: D92
                format!("the line ends where {expected} should be"),
            )
        } else {
            Message::refused(
                token.place,
                // trace: D92
                format!("{expected} should be here, not `{}`", token.written()),
            )
        };
        self.messages.push(message);
        Skip
    }

    // ----- Top level -----

    fn parse_top_level(&mut self) -> Vec<Procedure> {
        let mut procedures = Vec::new();
        while !self.at_end() && !self.stopped {
            let token = self.current();
            self.check_indentation(0);
            if token.is_word("proc") {
                if let Some(procedure) = self.parse_procedure() {
                    procedures.push(procedure);
                }
            } else {
                let message = match &token.kind {
                    TokenKind::Word(word) if word == "fam" => {
                        Message::not_built_yet(token.place, "families (D41) aren't built yet")
                    }
                    TokenKind::Word(word) if word == "comment" => {
                        Message::not_built_yet(token.place, "comment blocks (D74) aren't built yet")
                    }
                    TokenKind::Word(word) if word == "extern" => Message::not_built_yet(
                        token.place,
                        "procedures written in Rust (D36) aren't built yet",
                    ),
                    _ => Message::not_designed_yet(
                        token.place,
                        "anything outside a procedure, other than a procedure, isn't decided",
                    ),
                };
                self.messages.push(message);
                self.skip_to_next_top_level();
            }
        }
        procedures
    }

    /// Skips to the next line starting at the left edge with `proc`, `fam` or `extern`.
    fn skip_to_next_top_level(&mut self) {
        self.advance();
        loop {
            let token = self.current();
            if self.at_end()
                || (token.first_on_line
                    && token.place.column == 1
                    && (token.is_word("proc")
                        || token.is_word("fam")
                        || token.is_word("extern")
                        || token.is_word("comment")))
            {
                return;
            }
            self.advance();
        }
    }

    fn parse_procedure(&mut self) -> Option<Procedure> {
        let proc_token = self.advance();
        let mut is_main = false;
        let name = if self.at_symbol(Symbol::Dot) {
            self.advance();
            let word = self.current();
            if word.is_word("main") {
                self.advance();
                is_main = true;
                Name {
                    text: "proc.main".to_string(),
                    place: proc_token.place.to(word.place),
                }
            } else {
                self.messages.push(Message::not_designed_yet(
                    word.place,
                    format!(
                        "`proc.{}` isn't decided; `proc.main` is the only narrowing of `proc`",
                        word.written()
                    ),
                ));
                self.stop_unless_line_ends_with_colon();
                Name {
                    text: word.written(),
                    place: word.place,
                }
            }
        } else if let TokenKind::Word(_) = &self.current().kind {
            let token = self.advance();
            self.name_from(token)
        } else {
            let _ = self.refuse_unexpected("the procedure's name");
            self.stop_unless_line_ends_with_colon();
            Name {
                text: String::new(),
                place: proc_token.place,
            }
        };
        let header_place = proc_token.place.to(self.previous().place);

        let mut parameters = Vec::new();
        let mut returns = None;
        let mut header_broken = false;
        if !self.stopped && self.at_symbol(Symbol::OpenBracket) {
            match self.parse_parameters() {
                Ok(list) => parameters = list,
                Err(Skip) => header_broken = true,
            }
        }
        if !self.stopped && !header_broken && self.at_word("returns") {
            self.advance();
            match self.parse_type() {
                Ok(type_name) => returns = Some(type_name),
                Err(Skip) => header_broken = true,
            }
        }
        if !self.stopped && !header_broken && (self.at_word("or") || self.at_word("fails")) {
            self.messages.push(Message::not_built_yet(
                self.current().place,
                "failures in a procedure's header (D39) aren't built yet",
            ));
            header_broken = true;
        }
        if self.stopped {
            return None;
        }
        if header_broken {
            // A header broken only after its name: carry on into the body if the line still ends with `:` (D55).
            if !self.line_ends_with_colon() {
                self.stopped = true;
                return None;
            }
            while !self.at_line_start() {
                self.advance();
            }
        } else {
            self.expect_opening_colon();
            if self.stopped {
                return None;
            }
        }

        // The body, starting with an optional `labels` line (D25).
        let mut labels = Vec::new();
        if self.at_word("labels") && !self.at_end() {
            self.check_indentation(1);
            labels = self.parse_labels_line();
        }
        let body = self.parse_body(1);
        if self.stopped {
            return None;
        }
        let closer = if is_main {
            "end proc.main".to_string()
        } else {
            format!("end proc {}", name.text)
        };
        self.expect_closer(0, &closer);
        Some(Procedure {
            name,
            is_main,
            header_place,
            parameters,
            returns,
            labels,
            body,
        })
    }

    fn stop_unless_line_ends_with_colon(&mut self) {
        if !self.line_ends_with_colon() {
            self.stopped = true;
        }
    }

    /// `(` parameters `)`, in the form D61 requires: `(Whole a, b)` when two or more share one type, and
    /// `(name: Type, ...)` otherwise.
    fn parse_parameters(&mut self) -> Result<Vec<Parameter>, Skip> {
        self.set_role(Role::CallOpen);
        let open = self.advance();
        if self.at_symbol(Symbol::CloseBracket) {
            // trace: D4
            let close = self.advance();
            let place = open.place.to(close.place);
            self.refuse_with_edit(
                place,
                Edit {
                    place,
                    replacement: String::new(),
                },
                // trace: D88
                "a definition writes `( )` only when it has parameters.",
            );
            return Ok(Vec::new());
        }
        let shared_type = matches!(&self.current().kind, TokenKind::Word(word)
            if word.starts_with(|first: char| first.is_ascii_uppercase()));
        let mut parameters = Vec::new();
        if shared_type {
            let type_name = self.parse_type()?;
            loop {
                let token = self.expect_word("a parameter's name")?;
                let name = self.name_from(token);
                if self.at_symbol(Symbol::Colon) {
                    self.messages.push(Message::refused(
                        self.current().place,
                        "a header writes one type in front for every parameter, or a type after each, never both",
                    // trace: D89
                    ).why("parameters sharing one type write it once, in front; otherwise each writes its own."));
                    return Err(Skip);
                }
                parameters.push(Parameter { name, type_name });
                if self.at_symbol(Symbol::Comma) {
                    self.advance();
                } else {
                    break;
                }
            }
        } else {
            loop {
                let token = self.expect_word("a parameter's name")?;
                let name = self.name_from(token);
                if !self.at_symbol(Symbol::Colon) {
                    return Err(self.refuse_unexpected("`:` and the parameter's type"));
                }
                self.advance();
                let type_name = self.parse_type()?;
                parameters.push(Parameter { name, type_name });
                if self.at_symbol(Symbol::Comma) {
                    self.advance();
                } else {
                    break;
                }
            }
        }
        if !self.at_symbol(Symbol::CloseBracket) {
            return Err(self.refuse_unexpected("`)`"));
        }
        let close = self.advance();
        let place = open.place.to(close.place);

        // trace: D61
        let names = |parameters: &[Parameter]| {
            parameters
                .iter()
                .map(|parameter| parameter.name.text.clone())
                .collect::<Vec<_>>()
        };
        let first_type = parameters.first().and_then(|first| first.type_name.known);
        let all_same = first_type.is_some()
            && parameters
                .iter()
                .all(|parameter| parameter.type_name.known == first_type);
        // trace: D89
        let why = "parameters sharing one type write it once, in front, when there are two or more; \
                   otherwise each writes its own.";
        if shared_type && parameters.len() == 1 {
            if let (Some(parameter), Some(type_name)) = (parameters.first(), first_type) {
                let replacement = format!("({}: {})", parameter.name.text, type_name.name());
                self.refuse_with_edit(place, Edit { place, replacement }, why);
            }
        } else if !shared_type
            && parameters.len() >= 2
            && all_same
            && let Some(type_name) = first_type
        {
            let replacement = format!("({} {})", type_name.name(), names(&parameters).join(", "));
            self.refuse_with_edit(place, Edit { place, replacement }, why);
        }
        Ok(parameters)
    }

    fn parse_type(&mut self) -> Result<TypeName, Skip> {
        let token = self.current();
        let TokenKind::Word(word) = &token.kind else {
            return Err(self.refuse_unexpected("a type"));
        };
        self.advance();
        let known = match word.as_str() {
            "Whole" => Some(Type::Whole),
            "Bool" => Some(Type::Bool),
            "Text" => Some(Type::Text),
            _ => None,
        };
        if word == "List" {
            self.messages.push(Message::not_built_yet(
                token.place,
                "lists (D75) aren't built yet",
            ));
            while !self.at_line_start()
                && !self.at_symbol(Symbol::Equals)
                && !self.at_symbol(Symbol::CloseBracket)
            {
                self.advance();
            }
            return Ok(TypeName {
                known: None,
                place: token.place,
            });
        }
        if known.is_none() {
            self.messages.push(Message::not_designed_yet(
                token.place,
                format!("types other than `Whole`, `Bool` and `Text` aren't decided, so `{word}` can't be used yet"),
            ));
        }
        if self.at_symbol(Symbol::Less) && self.current().spaces_before == 0 {
            self.messages.push(Message::not_designed_yet(
                self.current().place,
                "filling in a type with `< >` isn't decided beyond the bracket itself (D3)",
            ));
            while !self.at_line_start() && !self.at_symbol(Symbol::Greater) {
                self.advance();
            }
            if self.at_symbol(Symbol::Greater) {
                self.advance();
            }
            return Ok(TypeName {
                known: None,
                place: token.place,
            });
        }
        Ok(TypeName {
            known,
            place: token.place,
        })
    }

    fn expect_word(&mut self, expected: &str) -> Result<&'a Token, Skip> {
        if let TokenKind::Word(_) = &self.current().kind {
            Ok(self.advance())
        } else {
            Err(self.refuse_unexpected(expected))
        }
    }

    /// A name a program chooses: refused if it's a keyword (D58), and not designed yet if it holds anything but
    /// lowercase words joined by `_`, the only names the standard shows.
    fn name_from(&mut self, token: &Token) -> Name {
        let text = token.written();
        if is_keyword(&text) {
            // trace: D58
            self.messages.push(
                Message::refused(
                    token.place,
                    format!("`{text}` is a keyword, so it can't be a name"),
                )
                // trace: D87
                .why("a name can't be one of Plainly's own words."),
            );
        } else if !text.is_ascii() {
            self.messages.push(Message::not_designed_yet(
                token.place,
                format!("letters beyond English in a name, as in `{text}`, aren't decided"),
            ));
        } else if let Some(problem) = name_problem(&text) {
            // trace: D99
            self.messages.push(
                Message::refused(token.place, format!("`{text}` {problem}"))
                    .why("a name is lowercase letters and digits, starting with a letter, with `_` only between words."),
            );
        }
        Name {
            text,
            place: token.place,
        }
    }

    /// `:` at the end of an opening line (D8). A missing `:` is refused and checking carries on into the body
    /// (D55); anything else after the opening line stops checking.
    fn expect_opening_colon(&mut self) {
        if self.at_symbol(Symbol::Colon) {
            self.advance();
            if !self.at_line_start() {
                self.messages.push(Message::not_designed_yet(
                    self.current().place,
                    "a body on the same line as its opening line isn't decided",
                ));
                self.stopped = true;
            }
        } else if self.at_line_start() {
            let previous = self.previous();
            let place = self.place_after(previous);
            self.refuse_with_edit(
                place,
                Edit {
                    place: Place { length: 0, ..place },
                    replacement: ":".to_string(),
                },
                // trace: D85
                "a block's opening line ends with `:`.",
            );
        } else {
            let _ = self.refuse_unexpected("`:`");
            self.stopped = true;
        }
    }

    // ----- Indentation and closers -----

    /// The current token starts a line at `depth` levels in; any other indentation, a tab included, is refused
    /// with the whole corrected line as the fix (D10).
    fn check_indentation(&mut self, depth: usize) {
        let token = self.current();
        if !token.first_on_line || token.kind == TokenKind::EndOfFile {
            return;
        }
        let line = token.place.line;
        let has_tab = self
            .lines
            .get(line - 1)
            .is_some_and(|facts| facts.tab_in_indentation);
        let expected = depth * INDENTATION_LEVEL;
        if has_tab || token.place.column - 1 != expected {
            // trace: D10
            let edit = Edit {
                place: Place {
                    file: self.file,
                    line,
                    column: 1,
                    length: token.place.column - 1,
                },
                replacement: " ".repeat(expected),
            };
            let place = Place {
                file: self.file,
                line,
                column: 1,
                length: (token.place.column - 1).max(1),
            };
            self.messages.push(
                Message::refused_with_fix(place, edit)
                    // trace: D85
                    .why("each level of indentation is 4 spaces, and tabs aren't used."),
            );
        }
    }

    /// A closer like `end if` or `end proc larger`, alone on its line with no `;` (D8).
    fn expect_closer(&mut self, depth: usize, expected: &str) {
        let token = self.current();
        if !token.is_word("end") {
            let (place, what) = if token.kind == TokenKind::EndOfFile {
                (
                    self.place_after(self.previous()),
                    // trace: D87
                    format!("the file ends before `{expected}`"),
                )
            } else {
                (
                    token.place,
                    format!("`{expected}` is missing before this line"),
                )
            };
            self.messages.push(
                Message::refused(place, what)
                    // trace: D85
                    .why("every block ends with `end` and what it closes."),
            );
            self.stopped = true;
            return;
        }
        self.check_indentation(depth);
        let start = self.advance().place;
        let mut written = "end".to_string();
        let mut end_place = start;
        while !self.at_line_start() {
            let part = self.advance();
            if part.spaces_before > 0 {
                written.push(' ');
            }
            written.push_str(&part.written());
            end_place = part.place;
        }
        // trace: D8
        if written != expected {
            let place = start.to(end_place);
            let why = if written == format!("{expected};") {
                // trace: D85
                "a closer stands alone, with no `;`."
            } else {
                // trace: D85
                "a closer is `end` and what it closes."
            };
            self.refuse_with_edit(
                place,
                Edit {
                    place,
                    replacement: expected.to_string(),
                },
                why,
            );
        }
    }

    // ----- Blocks and statements -----

    /// Statements at `depth`, up to an `end` or `else`, or a `proc` at the left edge.
    fn parse_body(&mut self, depth: usize) -> Vec<Statement> {
        let mut statements = Vec::new();
        loop {
            if self.stopped || self.at_end() {
                break;
            }
            let token = self.current();
            if token.is_word("end") || token.is_word("else") {
                break;
            }
            if token.is_word("proc") && token.place.column == 1 {
                break;
            }
            self.check_indentation(depth);
            let start = self.position;
            let statement = self.parse_statement(depth);
            statements.push(statement);
            if self.position == start {
                // Nothing was read: skip the line so the parser always moves on.
                self.skip_rest_of_line();
            }
        }
        statements
    }

    fn parse_statement(&mut self, depth: usize) -> Statement {
        let first = self.current();
        let place = first.place;
        let refused = |place| Statement {
            kind: StatementKind::Refused,
            place,
        };
        let kind = match &first.kind {
            TokenKind::Word(word) => match word.as_str() {
                "let" | "var" => self.parse_declaration(),
                "if" => return self.parse_if(depth),
                "while" => return self.parse_while(depth),
                "break" => {
                    self.advance();
                    self.expect_statement_end(place)
                        .map(|()| StatementKind::Break)
                }
                "return" => self.parse_return(place),
                "labels" => {
                    // trace: D25
                    self.messages.push(
                        Message::refused(
                            place,
                            "a `labels` line can only be the first line of a procedure's body",
                        )
                        // trace: D89
                        .why("a procedure declares its labels on the first line of its body."),
                    );
                    Err(Skip)
                }
                "comment" => {
                    self.messages.push(Message::not_built_yet(
                        place,
                        "comment blocks (D74) aren't built yet",
                    ));
                    Err(Skip)
                }
                "case" | "otherwise" | "instead" | "include" => {
                    self.messages.push(Message::not_built_yet(
                        place,
                        format!("`{word}` (D40, D43) isn't built yet"),
                    ));
                    Err(Skip)
                }
                _ if self.peek(1).is_symbol(Symbol::Equals) => self.parse_change(),
                _ => self.parse_expression_statement(place),
            },
            _ => self.parse_expression_statement(place),
        };
        match kind {
            Ok(kind) => Statement { kind, place },
            Err(Skip) => {
                if !self.at_line_start() {
                    self.skip_rest_of_line();
                }
                refused(place)
            }
        }
    }

    /// `let name: Type = value;` or `var ...` (D13).
    fn parse_declaration(&mut self) -> Result<StatementKind, Skip> {
        let keyword = self.advance();
        let changeable = keyword.is_word("var");
        let name_token = self.expect_word("the variable's name")?;
        let name = self.name_from(name_token);
        if !self.at_symbol(Symbol::Colon) {
            if self.at_symbol(Symbol::Equals) {
                // trace: D13
                self.messages.push(
                    Message::refused(
                        name.place,
                        format!("`{}` needs its type, written after a `:`", name.text),
                    )
                    // trace: D85
                    .why("a declaration always writes the variable's type."),
                );
                return Err(Skip);
            }
            if let TokenKind::Word(word) = &self.current().kind
                && word.starts_with(|first: char| first.is_ascii_uppercase())
            {
                let place = self.place_after(name_token);
                self.refuse_with_edit(
                    place,
                    Edit {
                        place: Place { length: 0, ..place },
                        replacement: ":".to_string(),
                    },
                    // trace: D87
                    "a declaration writes `:` between the variable's name and its type.",
                );
            } else {
                return Err(self.refuse_unexpected("`:` and the variable's type"));
            }
        } else {
            self.advance();
        }
        let type_name = self.parse_type()?;
        if self.at_symbol(Symbol::Equals) {
            self.set_role(Role::BinaryOperator);
            self.advance();
        } else if self.at_symbol(Symbol::Semicolon) {
            self.messages.push(Message::not_designed_yet(
                self.current().place,
                "declaring a variable without a first value isn't decided",
            ));
            return Err(Skip);
        } else if self.starts_value() {
            // trace: D13
            let place = self.current().place;
            self.refuse_with_edit(
                place,
                Edit {
                    place: Place { length: 0, ..place },
                    replacement: "= ".to_string(),
                },
                // trace: D87
                "a declaration gives its variable a first value after `=`.",
            );
        } else {
            return Err(self.refuse_unexpected("`=` and the first value"));
        }
        let value_start = self.current().place;
        let value = self
            .parse_full_expression()
            .and_then(|value| self.expect_statement_end(keyword.place).map(|()| value));
        // trace: D55
        let value = value.unwrap_or_else(|Skip| {
            if !self.at_line_start() {
                self.skip_rest_of_line();
            }
            Expression {
                kind: ExpressionKind::Refused,
                place: value_start,
            }
        });
        Ok(StatementKind::Declare {
            changeable,
            name,
            type_name,
            value,
        })
    }

    fn starts_value(&self) -> bool {
        match &self.current().kind {
            TokenKind::Whole(_) | TokenKind::Text(_) => true,
            TokenKind::Word(word) => {
                !is_keyword(word) || matches!(word.as_str(), "true" | "false" | "not" | "be")
            }
            TokenKind::Symbol(symbol) => matches!(symbol, Symbol::Minus | Symbol::OpenBracket),
            TokenKind::EndOfFile => false,
        }
    }

    /// `name = value;` (D14).
    fn parse_change(&mut self) -> Result<StatementKind, Skip> {
        let name_token = self.advance();
        let name = Name {
            text: name_token.written(),
            place: name_token.place,
        };
        self.set_role(Role::BinaryOperator);
        self.advance();
        let value = self.parse_full_expression()?;
        self.expect_statement_end(name_token.place)?;
        Ok(StatementKind::Change { name, value })
    }

    fn parse_expression_statement(&mut self, place: Place) -> Result<StatementKind, Skip> {
        let expression = self.parse_full_expression()?;
        self.expect_statement_end(place)?;
        Ok(StatementKind::Expression(expression))
    }

    fn parse_return(&mut self, place: Place) -> Result<StatementKind, Skip> {
        self.advance();
        if self.at_symbol(Symbol::Semicolon) || self.at_line_start() {
            self.expect_statement_end(place)?;
            return Ok(StatementKind::Return(None));
        }
        let value = self.parse_full_expression()?;
        self.expect_statement_end(place)?;
        Ok(StatementKind::Return(Some(value)))
    }

    /// `;` ends the statement (D7). A missing one is refused with its place as the fix, and checking carries on as
    /// if it were there.
    fn expect_statement_end(&mut self, start: Place) -> Result<(), Skip> {
        if self.at_symbol(Symbol::Semicolon) {
            let semicolon = self.advance();
            if semicolon.place.line != start.line {
                // A statement running over several lines is allowed (D7); how its later lines are indented isn't.
                let later = self.tokens[..self.position]
                    .iter()
                    .rev()
                    .take_while(|token| token.place.line != start.line)
                    .last()
                    .map(|token| token.place)
                    .unwrap_or(semicolon.place);
                self.messages.push(Message::not_designed_yet(
                    later,
                    "how a statement's later lines are indented isn't decided",
                ));
            }
            if !self.at_line_start() {
                self.messages.push(Message::not_designed_yet(
                    self.current().place,
                    "two statements on one line isn't decided",
                ));
            }
            return Ok(());
        }
        if self.at_line_start() {
            // trace: D7
            let previous = self.previous();
            let place = self.place_after(previous);
            self.refuse_with_edit(
                place,
                Edit {
                    place: Place { length: 0, ..place },
                    replacement: ";".to_string(),
                },
                // trace: D85
                "every statement ends with `;`.",
            );
            return Ok(());
        }
        Err(self.refuse_unexpected("`;`"))
    }

    fn parse_if(&mut self, depth: usize) -> Statement {
        let place = self.current().place;
        self.advance();
        let mut branches = Vec::new();
        let mut else_body = None;
        let condition = self.parse_condition_line();
        let body = self.parse_body(depth + 1);
        branches.push((condition, body));
        loop {
            if self.stopped {
                break;
            }
            if self.at_word("else") {
                self.check_indentation(depth);
                self.advance();
                if self.at_word("if") {
                    self.advance();
                    let condition = self.parse_condition_line();
                    let body = self.parse_body(depth + 1);
                    branches.push((condition, body));
                    continue;
                }
                self.expect_opening_colon();
                let body = self.parse_body(depth + 1);
                else_body = Some(body);
                if self.at_word("else") {
                    self.messages.push(
                        Message::refused(
                            self.current().place,
                            "nothing can follow an `if`'s `else:` part but `end if`",
                        )
                        .why("an `if` has at most one `else:`, and it comes last."),
                    );
                    self.stopped = true;
                    break;
                }
            }
            if !self.stopped {
                self.expect_closer(depth, "end if");
            }
            break;
        }
        Statement {
            kind: StatementKind::If {
                branches,
                else_body,
            },
            place,
        }
    }

    fn parse_while(&mut self, depth: usize) -> Statement {
        let place = self.current().place;
        self.advance();
        let condition = self.parse_condition_line();
        let body = self.parse_body(depth + 1);
        if !self.stopped {
            self.expect_closer(depth, "end while");
        }
        Statement {
            kind: StatementKind::While { condition, body },
            place,
        }
    }

    /// A condition, then `:` at the end of the line. When the condition is broken but the line still ends with
    /// `:`, it's clear what was meant, so checking carries on into the body (D55).
    fn parse_condition_line(&mut self) -> Expression {
        let start = self.current().place;
        match self.parse_full_expression() {
            Ok(condition) => {
                self.expect_opening_colon();
                condition
            }
            Err(Skip) => {
                if self.line_ends_with_colon() {
                    while !self.at_line_start() {
                        self.advance();
                    }
                } else {
                    self.stopped = true;
                }
                Expression {
                    kind: ExpressionKind::Refused,
                    place: start,
                }
            }
        }
    }

    /// `labels parameter as label, ...;` (D25).
    fn parse_labels_line(&mut self) -> Vec<LabelPair> {
        let labels_token = self.advance();
        let mut pairs = Vec::new();
        let result = (|| -> Result<(), Skip> {
            loop {
                let parameter_token = self.expect_word("a parameter's name")?;
                let parameter = Name {
                    text: parameter_token.written(),
                    place: parameter_token.place,
                };
                if !self.at_word("as") {
                    return Err(self.refuse_unexpected("`as` and the label"));
                }
                self.advance();
                let label_token = self.expect_word("the label")?;
                let label = self.name_from(label_token);
                pairs.push(LabelPair { parameter, label });
                if self.at_symbol(Symbol::Comma) {
                    self.advance();
                } else {
                    break;
                }
            }
            self.expect_statement_end(labels_token.place)
        })();
        if result.is_err() && !self.at_line_start() {
            self.skip_rest_of_line();
        }
        pairs
    }

    // ----- Expressions -----

    /// A whole expression: a slide (D5), `call ?? value` (D23), or anything less.
    fn parse_full_expression(&mut self) -> Result<Expression, Skip> {
        let first = self.parse_or()?;
        if self.at_symbol(Symbol::QuestionQuestion) {
            // A call takes its own `??` (D81), so this one follows something else.
            self.messages.push(
                Message::refused(
                    self.current().place,
                    "`??` here doesn't follow a call that can fail",
                )
                .why("`??` deals with the failure of the call right before it."),
            );
            return Err(Skip);
        }
        if !self.at_symbol(Symbol::Slide) {
            return Ok(first);
        }
        // trace: D73
        let mut steps = Vec::new();
        while self.at_symbol(Symbol::Slide) {
            self.set_role(Role::BinaryOperator);
            self.advance();
            steps.push(self.parse_step()?);
        }
        let place = first.place.to(self.previous().place);
        if self.at_symbol(Symbol::QuestionQuestion) {
            return Err(self.not_designed_here(
                self.current().place,
                "`??` after a slide's step isn't decided, since a step isn't written as a call",
            ));
        }
        if self.at_binary_operator() {
            // trace: D81
            let why = "a slide's step is a procedure, so an operator after it goes outside brackets around the slide.";
            let message = match self.written_at(place) {
                Some(text) if place.line == self.current().place.line => Message::refused_with_fix(
                    place,
                    Edit {
                        place,
                        replacement: format!("({text})"),
                    },
                ),
                _ => Message::refused(
                    self.current().place,
                    "an operator can't follow a slide's step",
                ),
            };
            self.messages.push(message.why(why));
            return Err(Skip);
        }
        Ok(Expression {
            kind: ExpressionKind::Slide {
                first: Box::new(first),
                steps,
            },
            place,
        })
    }

    /// `call ?? value`: `??` belongs to the call right before it, and takes a single value (D23, D81).
    fn parse_fallback(&mut self, call: Expression) -> Result<Expression, Skip> {
        // trace: D81
        self.set_role(Role::BinaryOperator);
        self.advance();
        if self.at_word("ignore") || self.at_word("crash") {
            return Err(self.not_built_here(
                self.current().place,
                format!("`?? {}` (D23) isn't built yet", self.current().written()),
            ));
        }
        let fallback = self.parse_unary()?;
        let place = call.place.to(fallback.place);
        Ok(Expression {
            kind: ExpressionKind::Fallback {
                call: Box::new(call),
                fallback: Box::new(fallback),
            },
            place,
        })
    }

    fn not_designed_here(&mut self, place: Place, what: &str) -> Skip {
        self.messages.push(Message::not_designed_yet(place, what));
        Skip
    }

    fn not_built_here(&mut self, place: Place, what: String) -> Skip {
        self.messages.push(Message::not_built_yet(place, what));
        Skip
    }

    fn at_binary_operator(&self) -> bool {
        self.binary_operator().is_some() || self.at_symbol(Symbol::Equals)
    }

    fn binary_operator(&self) -> Option<BinaryOperator> {
        let token = self.current();
        Some(match &token.kind {
            TokenKind::Symbol(Symbol::Plus) => BinaryOperator::Add,
            TokenKind::Symbol(Symbol::Minus) => BinaryOperator::Subtract,
            TokenKind::Symbol(Symbol::Star) => BinaryOperator::Multiply,
            TokenKind::Symbol(Symbol::Ampersand) => BinaryOperator::Join,
            TokenKind::Symbol(Symbol::EqualsEquals) => BinaryOperator::Equal,
            TokenKind::Symbol(Symbol::NotEquals) => BinaryOperator::NotEqual,
            TokenKind::Symbol(Symbol::Less) => BinaryOperator::Less,
            TokenKind::Symbol(Symbol::Greater) => BinaryOperator::Greater,
            TokenKind::Word(word) => match word.as_str() {
                "at_least" => BinaryOperator::AtLeast,
                "at_most" => BinaryOperator::AtMost,
                "and" => BinaryOperator::And,
                "or" => BinaryOperator::Or,
                _ => return None,
            },
            _ => return None,
        })
    }

    fn binary(
        operator: BinaryOperator,
        operator_place: Place,
        left: Expression,
        right: Expression,
    ) -> Expression {
        let place = left.place.to(right.place);
        Expression {
            kind: ExpressionKind::Binary {
                operator,
                operator_place,
                left: Box::new(left),
                right: Box::new(right),
            },
            place,
        }
    }

    /// `or`, worked out last (D34).
    fn parse_or(&mut self) -> Result<Expression, Skip> {
        let mut left = self.parse_and()?;
        while self.at_word("or") {
            let operator_place = self.current().place;
            self.set_role(Role::BinaryOperator);
            self.advance();
            let right = self.parse_and()?;
            // trace: D34
            left = Self::binary(BinaryOperator::Or, operator_place, left, right);
        }
        Ok(left)
    }

    /// `and`, worked out before `or` (D34).
    fn parse_and(&mut self) -> Result<Expression, Skip> {
        let mut left = self.parse_not()?;
        while self.at_word("and") {
            let operator_place = self.current().place;
            self.set_role(Role::BinaryOperator);
            self.advance();
            let right = self.parse_not()?;
            // trace: D34
            left = Self::binary(BinaryOperator::And, operator_place, left, right);
        }
        Ok(left)
    }

    /// `not`, worked out before `and` and `or`, and after comparisons (D34).
    fn parse_not(&mut self) -> Result<Expression, Skip> {
        if self.at_word("not") {
            let start = self.advance().place;
            let value = self.parse_not()?;
            let place = start.to(value.place);
            return Ok(Expression {
                kind: ExpressionKind::Not(Box::new(value)),
                place,
            });
        }
        self.parse_comparison()
    }

    /// One comparison at most (D17); chains of them aren't decided. Maths comes first (D72).
    fn parse_comparison(&mut self) -> Result<Expression, Skip> {
        let left = self.parse_sum()?;
        if self.at_symbol(Symbol::Equals) {
            // trace: D14
            let place = self.current().place;
            self.set_role(Role::BinaryOperator);
            self.refuse_with_edit(
                place,
                Edit {
                    place,
                    replacement: "==".to_string(),
                },
                // trace: D85
                "`=` gives a variable a value, and is never part of a condition; `==` compares.",
            );
        }
        let operator = match self.binary_operator() {
            Some(operator) if operator.is_comparison() => operator,
            _ if self.at_symbol(Symbol::Equals) => BinaryOperator::Equal,
            _ => return Ok(left),
        };
        let operator_place = self.current().place;
        self.set_role(Role::BinaryOperator);
        self.advance();
        let right = self.parse_sum()?;
        if self
            .binary_operator()
            .is_some_and(BinaryOperator::is_comparison)
        {
            // trace: D81
            let why = "two comparisons in a row are refused; `and` joins them, naming the middle value twice.";
            let message = match self.written_at(right.place) {
                Some(middle) if right.place.line == operator_place.line => {
                    let end = Place {
                        column: right.place.column + right.place.length,
                        length: 0,
                        ..right.place
                    };
                    Message::refused_with_fix(
                        left.place.to(self.current().place),
                        Edit {
                            place: end,
                            replacement: format!(" and {middle}"),
                        },
                    )
                }
                _ => Message::refused(self.current().place, "two comparisons are written in a row"),
            };
            self.messages.push(message.why(why));
            return Err(Skip);
        }
        // trace: D17
        Ok(Self::binary(operator, operator_place, left, right))
    }

    /// `+`, `-` and `&`, left to right, after `*` (D72). `&` next to an unbracketed `+`, `-` or `*` is refused,
    /// since maths doesn't say which goes first (D72).
    fn parse_sum(&mut self) -> Result<Expression, Skip> {
        let mut left = self.parse_product()?;
        let mut join_place: Option<Place> = None;
        let mut maths_place: Option<Place> = arithmetic_place(&left);
        while let Some(operator) = self.binary_operator() {
            if !matches!(
                operator,
                BinaryOperator::Add | BinaryOperator::Subtract | BinaryOperator::Join
            ) {
                break;
            }
            let operator_place = self.current().place;
            if operator == BinaryOperator::Join {
                join_place = join_place.or(Some(operator_place));
            } else {
                maths_place = maths_place.or(Some(operator_place));
            }
            self.set_role(Role::BinaryOperator);
            self.advance();
            let right = self.parse_product()?;
            maths_place = maths_place.or(arithmetic_place(&right));
            // trace: D72
            left = Self::binary(operator, operator_place, left, right);
        }
        if let (Some(join_place), Some(maths_place)) = (join_place, maths_place) {
            // trace: D72
            self.messages.push(
                Message::refused(
                    join_place,
                    "`&` is next to maths without brackets, so it isn't clear which goes first",
                )
                .because("the maths is here", maths_place)
                // trace: D89
                .why("where maths doesn't say which goes first, `( )` shows it."),
            );
            return Err(Skip);
        }
        Ok(left)
    }

    /// `*`, left to right, after a negating `-` and before `+` and `-` (D72).
    fn parse_product(&mut self) -> Result<Expression, Skip> {
        let mut left = self.parse_unary()?;
        loop {
            if self.at_symbol(Symbol::Slash) || self.at_word("div") || self.at_word("rem") {
                return Err(self.not_built_here(
                    self.current().place,
                    "dividing and remainders, `/`, `div` and `rem` (D70, D71), aren't built yet"
                        .to_string(),
                ));
            }
            if !self.at_symbol(Symbol::Star) {
                return Ok(left);
            }
            let operator_place = self.current().place;
            self.set_role(Role::BinaryOperator);
            self.advance();
            let right = self.parse_unary()?;
            // trace: D59, D72
            left = Self::binary(BinaryOperator::Multiply, operator_place, left, right);
        }
    }

    /// `-` right before a value negates it (D27).
    fn parse_unary(&mut self) -> Result<Expression, Skip> {
        if self.at_symbol(Symbol::Minus) {
            self.set_role(Role::PrefixMinus);
            let minus = self.advance();
            if let TokenKind::Whole(digits) = &self.current().kind
                && self.current().spaces_before == 0
            {
                // A negative number is read whole, so the most negative `Whole` can be written.
                let number = self.advance();
                let value =
                    self.whole_value(&format!("-{digits}"), minus.place.to(number.place))?;
                return Ok(Expression {
                    kind: ExpressionKind::Whole(value),
                    place: minus.place.to(number.place),
                });
            }
            let value = self.parse_unary()?;
            let place = minus.place.to(value.place);
            // trace: D27
            return Ok(Expression {
                kind: ExpressionKind::Negate(Box::new(value)),
                place,
            });
        }
        self.parse_primary()
    }

    fn whole_value(&mut self, written: &str, place: Place) -> Result<i64, Skip> {
        match written.parse::<i64>() {
            Ok(value) => Ok(value),
            Err(_) => {
                // Unreadable digits were already reported by the tokenizer.
                if written
                    .trim_start_matches('-')
                    .chars()
                    .all(|digit| digit.is_ascii_digit())
                {
                    self.messages.push(Message::not_designed_yet(
                        place,
                        format!("going past `Whole`'s range isn't decided, and `{written}` is outside it"),
                    ));
                }
                Err(Skip)
            }
        }
    }

    fn parse_primary(&mut self) -> Result<Expression, Skip> {
        let token = self.current();
        let expression = match &token.kind {
            TokenKind::Whole(digits) => {
                self.advance();
                let value = self.whole_value(digits, token.place)?;
                Expression {
                    kind: ExpressionKind::Whole(value),
                    place: token.place,
                }
            }
            TokenKind::Text(characters) => {
                self.advance();
                Expression {
                    kind: ExpressionKind::Text(characters.clone()),
                    place: token.place,
                }
            }
            TokenKind::Word(word) if word == "true" || word == "false" => {
                self.advance();
                Expression {
                    kind: ExpressionKind::Bool(word == "true"),
                    place: token.place,
                }
            }
            TokenKind::Word(word) if !is_keyword(word) || word == "be" => {
                let callee = self.parse_callee()?;
                if self.at_symbol(Symbol::OpenBracket) && !self.at_line_start() {
                    self.set_role(Role::CallOpen);
                    let arguments = self.parse_arguments()?;
                    let place = callee.place.to(self.previous().place);
                    let call = Expression {
                        kind: ExpressionKind::Call { callee, arguments },
                        place,
                    };
                    if self.at_symbol(Symbol::QuestionQuestion) {
                        self.parse_fallback(call)?
                    } else {
                        call
                    }
                } else if callee.words.len() == 1 {
                    let place = callee.place;
                    let name = callee.words.into_iter().next().map(|word| word.text);
                    Expression {
                        kind: ExpressionKind::Name(name.unwrap_or_default()),
                        place,
                    }
                } else {
                    let place = callee.place;
                    Expression {
                        kind: ExpressionKind::BareNarrowed(callee),
                        place,
                    }
                }
            }
            TokenKind::Symbol(Symbol::OpenBracket) => {
                let open = self.advance();
                let inner = self.parse_full_expression()?;
                if !self.at_symbol(Symbol::CloseBracket) {
                    return Err(self.refuse_unexpected("`)`"));
                }
                let close = self.advance();
                Expression {
                    kind: ExpressionKind::Grouped(Box::new(inner)),
                    place: open.place.to(close.place),
                }
            }
            TokenKind::Symbol(Symbol::OpenSquare) => {
                return Err(
                    self.not_built_here(token.place, "lists (D75) aren't built yet".to_string())
                );
            }
            TokenKind::Word(word) if word == "copy" || word == "share" => {
                return Err(self.not_built_here(
                    token.place,
                    format!("`{word}` (D49) isn't built yet, since lists aren't"),
                ));
            }
            _ => return Err(self.refuse_unexpected("a value")),
        };
        if self.at_symbol(Symbol::Question) {
            return Err(self.not_built_here(
                self.current().place,
                "`?`, passing a failure up (D23), isn't built yet".to_string(),
            ));
        }
        if self.at_symbol(Symbol::OpenSquare) && self.current().spaces_before == 0 {
            self.set_role(Role::CallOpen);
            return Err(self.not_built_here(
                self.current().place,
                "lists and their items, as in `list[pos: 1]` (D75, D80), aren't built yet"
                    .to_string(),
            ));
        }
        Ok(expression)
    }

    /// A word and its narrowings, like `show_msg.fmt.same_line` (D41).
    fn parse_callee(&mut self) -> Result<Callee, Skip> {
        let first = self.advance();
        let mut words = vec![Name {
            text: first.written(),
            place: first.place,
        }];
        loop {
            if self.at_symbol(Symbol::DotStar) {
                return Err(self.not_built_here(
                    self.current().place,
                    "handlers and their `.*` mark (D42) aren't built yet".to_string(),
                ));
            }
            if !self.at_symbol(Symbol::Dot) {
                break;
            }
            if !matches!(self.peek(1).kind, TokenKind::Word(_)) {
                self.advance();
                return Err(self.refuse_unexpected("a narrowing after `.`"));
            }
            self.advance();
            let word = self.advance();
            words.push(Name {
                text: word.written(),
                place: word.place,
            });
        }
        let place = first.place.to(self.previous().place);
        Ok(Callee { words, place })
    }

    /// `( arguments )`, each with its label if it has one (D24).
    fn parse_arguments(&mut self) -> Result<Vec<Argument>, Skip> {
        self.advance();
        let mut arguments = Vec::new();
        if self.at_symbol(Symbol::CloseBracket) {
            self.advance();
            return Ok(arguments);
        }
        loop {
            let label = if matches!(self.current().kind, TokenKind::Word(_))
                && self.peek(1).is_symbol(Symbol::Colon)
            {
                let token = self.advance();
                self.advance();
                Some(Name {
                    text: token.written(),
                    place: token.place,
                })
            } else {
                None
            };
            let value = self.parse_full_expression()?;
            arguments.push(Argument { label, value });
            if self.at_symbol(Symbol::Comma) {
                self.advance();
                continue;
            }
            if self.at_symbol(Symbol::CloseBracket) {
                self.advance();
                return Ok(arguments);
            }
            return Err(self.refuse_unexpected("`,` or `)`"));
        }
    }

    /// One step of a slide: bare, or with `( )` for the arguments it adds (D5).
    fn parse_step(&mut self) -> Result<Step, Skip> {
        let token = self.current();
        if !matches!(&token.kind, TokenKind::Word(word) if !is_keyword(word) || word == "be") {
            return Err(self.refuse_unexpected("a procedure's name"));
        }
        let callee = self.parse_callee()?;
        let mut arguments = Vec::new();
        if self.at_symbol(Symbol::OpenBracket) && !self.at_line_start() {
            self.set_role(Role::CallOpen);
            let open = self.current().place;
            arguments = self.parse_arguments()?;
            if arguments.is_empty() {
                // trace: D5
                let place = open.to(self.previous().place);
                self.refuse_with_edit(
                    place,
                    Edit {
                        place,
                        replacement: String::new(),
                    },
                    // trace: D88
                    "a step with nothing else to pass is written bare, since the slide gives it its value.",
                );
            }
        }
        let place = callee.place.to(self.previous().place);
        Ok(Step {
            callee,
            arguments,
            place,
        })
    }

    // ----- Checks over the whole file -----

    /// Spacing between the parts of each line. Decided spacing is refused when it's wrong, with one message per
    /// symbol: `.` touches both sides (D18), `<` and `>` have a space on each side (D17), so do `+`, `-` and `*`
    /// between two values (D27, D59), and a negating `-` has none after it (D27). The spacing every example shows
    /// elsewhere isn't decided, so anything else is `NotDesignedYet`.
    fn check_spacing(&mut self) {
        let count = self.tokens.len();
        // `checked[index]` is set once the gap before token `index` has been looked at.
        let mut checked = vec![false; count];
        for index in 0..count {
            let token = &self.tokens[index];
            let role = self.roles[index];
            let Some(why) = decided_spacing(token, role) else {
                continue;
            };
            let why = why.as_str();
            let gap_before =
                (index > 0 && !token.first_on_line && role != Role::PrefixMinus).then_some(index);
            let gap_after = (index + 1 < count
                && !self.tokens[index + 1].first_on_line
                && self.tokens[index + 1].kind != TokenKind::EndOfFile)
                .then_some(index + 1);
            let mut wrong = false;
            let mut replacement = String::new();
            let mut start = token.place;
            let mut end = token.place;
            if let Some(gap) = gap_before {
                checked[gap] = true;
                let before = &self.tokens[gap - 1];
                let expected = expected_gap(before, token, (self.roles[gap - 1], role));
                wrong |= token.spaces_before != expected;
                replacement.push_str(&" ".repeat(expected));
                start = Place {
                    column: before.place.column + before.place.length,
                    length: 0,
                    ..before.place
                };
            }
            replacement.push_str(&token.written());
            if let Some(gap) = gap_after {
                checked[gap] = true;
                let after = &self.tokens[gap];
                let expected = expected_gap(token, after, (role, self.roles[gap]));
                wrong |= after.spaces_before != expected;
                replacement.push_str(&" ".repeat(expected));
                end = Place {
                    length: 0,
                    ..after.place
                };
            } else {
                end = Place {
                    column: end.column + end.length,
                    length: 0,
                    ..end
                };
            }
            if !wrong {
                continue;
            }
            let edit = Edit {
                place: Place {
                    length: end.column - start.column,
                    ..start
                },
                replacement,
            };
            let first = gap_before.map_or(token.place, |gap| self.tokens[gap - 1].place);
            let last = gap_after.map_or(token.place, |gap| self.tokens[gap].place);
            self.refuse_with_edit(first.to(last), edit, why);
        }

        for (index, &already_checked) in checked.iter().enumerate().skip(1) {
            let before = &self.tokens[index - 1];
            let after = &self.tokens[index];
            if already_checked || after.first_on_line || after.kind == TokenKind::EndOfFile {
                continue;
            }
            let expected = expected_gap(before, after, (self.roles[index - 1], self.roles[index]));
            let gap = after.spaces_before;
            if gap == expected {
                continue;
            }
            let place = if gap == 0 {
                before.place.to(after.place)
            } else {
                Place {
                    column: before.place.column + before.place.length,
                    length: gap,
                    ..after.place
                }
            };
            // trace: D98
            let as_written = |token: &Token| {
                self.written_at(token.place)
                    .unwrap_or_else(|| token.written())
            };
            let (first, second) = (as_written(before), as_written(after));
            let why = if expected == 0 {
                format!("no space goes between `{first}` and `{second}`.")
            } else if gap == 0 {
                format!("`{first}` and `{second}` have one space between them.")
            } else {
                "spaces never come two in a row, outside indentation.".to_string()
            };
            let edit = Edit {
                place: Place {
                    column: before.place.column + before.place.length,
                    length: gap,
                    ..after.place
                },
                replacement: " ".repeat(expected),
            };
            self.refuse_with_edit(place, edit, &why);
        }
    }

    /// Blank lines are allowed anywhere, at most one in a row (D97).
    fn check_blank_lines(&mut self) {
        // trace: D97
        let mut line = 1;
        while line <= self.lines.len() {
            let is_blank = |line: usize| self.lines.get(line - 1).is_some_and(|facts| facts.blank);
            if !is_blank(line) {
                line += 1;
                continue;
            }
            let mut run = 1;
            while is_blank(line + run) {
                run += 1;
            }
            if run > 1 {
                let mut message = Message::refused(
                    Place {
                        file: self.file,
                        line: line + 1,
                        column: 1,
                        length: 1,
                    },
                    "",
                )
                .why("one blank line is enough to separate the parts of a program.");
                message.what = None;
                message.removed_lines = Some(RemovedLines {
                    first_line: line + 1,
                    count: run - 1,
                });
                self.messages.push(message);
            }
            line += run;
        }
    }
}

/// The rule, as its `why:`, for a symbol whose spacing is decided.
fn decided_spacing(token: &Token, role: Role) -> Option<String> {
    if token.is_symbol(Symbol::Dot) || token.is_symbol(Symbol::DotStar) {
        // trace: D18
        // trace: D85
        return Some("a narrowing's `.` touches the words on both sides.".to_string());
    }
    if role == Role::PrefixMinus {
        // trace: D27
        // trace: D89
        return Some("a `-` that makes a value negative has no space after it.".to_string());
    }
    let spaced = [
        Symbol::Less,
        Symbol::Greater,
        Symbol::Plus,
        Symbol::Minus,
        Symbol::Star,
    ];
    if role == Role::BinaryOperator && spaced.iter().any(|&symbol| token.is_symbol(symbol)) {
        // trace: D17, D27, D59
        // trace: D85
        return Some(
            "`<`, `>`, `+`, `-` and `*` between two values have a space on each side.".to_string(),
        );
    }
    if role == Role::BinaryOperator {
        // trace: D98
        return Some(format!("`{}` has one space on each side.", token.written()));
    }
    None
}

/// The spacing between two tokens on one line that the standard's examples show.
fn expected_gap(before: &Token, after: &Token, roles: (Role, Role)) -> usize {
    let touching_after = [
        Symbol::Semicolon,
        Symbol::Comma,
        Symbol::CloseBracket,
        Symbol::CloseSquare,
        Symbol::Colon,
        Symbol::Question,
        Symbol::Dot,
        Symbol::DotStar,
        Symbol::DoubleColon,
    ];
    let touching_before = [
        Symbol::OpenBracket,
        Symbol::OpenSquare,
        Symbol::Dot,
        Symbol::DotStar,
        Symbol::DoubleColon,
    ];
    if touching_after.iter().any(|&symbol| after.is_symbol(symbol))
        || touching_before
            .iter()
            .any(|&symbol| before.is_symbol(symbol))
        || roles.1 == Role::CallOpen
        || roles.0 == Role::PrefixMinus
    {
        0
    } else {
        1
    }
}

/// Where an expression's unbracketed `+`, `-` or `*` is, if it has one.
fn arithmetic_place(expression: &Expression) -> Option<Place> {
    match &expression.kind {
        ExpressionKind::Binary {
            operator: BinaryOperator::Add | BinaryOperator::Subtract | BinaryOperator::Multiply,
            operator_place,
            ..
        } => Some(*operator_place),
        _ => None,
    }
}

/// What's wrong with a name, if anything (D99): it's lowercase letters `a` to `z` and digits, starting with a letter,
/// with `_` only between words, as in `player2` or `hp_max_2`.
fn name_problem(text: &str) -> Option<&'static str> {
    if text.chars().any(|letter| letter.is_ascii_uppercase()) {
        Some("has a capital letter, and names have none")
    } else if text.starts_with(|first: char| first.is_ascii_digit()) {
        Some("starts with a digit")
    } else if text.starts_with('_') {
        Some("starts with `_`")
    } else if text.ends_with('_') {
        Some("ends with `_`")
    } else if text.contains("__") {
        Some("has two `_` in a row")
    } else {
        None
    }
}
