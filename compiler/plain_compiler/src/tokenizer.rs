//! Splits a source file into tokens: words, whole numbers, text, and symbols.

use crate::message::{Edit, Message};
use crate::source::{Place, SourceFile};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Symbol {
    Semicolon,
    Colon,
    DoubleColon,
    Comma,
    OpenBracket,
    CloseBracket,
    OpenSquare,
    CloseSquare,
    Dot,
    DotStar,
    Equals,
    EqualsEquals,
    NotEquals,
    Less,
    Greater,
    Plus,
    Minus,
    Star,
    Slash,
    Ampersand,
    Slide,
    Question,
    QuestionQuestion,
}

impl Symbol {
    pub fn text(self) -> &'static str {
        match self {
            Symbol::Semicolon => ";",
            Symbol::Colon => ":",
            Symbol::DoubleColon => "::",
            Symbol::Comma => ",",
            Symbol::OpenBracket => "(",
            Symbol::CloseBracket => ")",
            Symbol::OpenSquare => "[",
            Symbol::CloseSquare => "]",
            Symbol::Dot => ".",
            Symbol::DotStar => ".*",
            Symbol::Equals => "=",
            Symbol::EqualsEquals => "==",
            Symbol::NotEquals => "!=",
            Symbol::Less => "<",
            Symbol::Greater => ">",
            Symbol::Plus => "+",
            Symbol::Minus => "-",
            Symbol::Star => "*",
            Symbol::Slash => "/",
            Symbol::Ampersand => "&",
            Symbol::Slide => "|>",
            Symbol::Question => "?",
            Symbol::QuestionQuestion => "??",
        }
    }
}

/// One character of a text literal, after `\` escapes (D30).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextCharacter {
    pub character: char,
    /// Whether it was written after a `\`, so a `{` stays a brace in `.fmt` text (D30).
    pub escaped: bool,
    /// Its column in the source.
    pub column: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TokenKind {
    /// A name, a keyword, or a type's name.
    Word(String),
    /// A whole number, as its digits.
    Whole(String),
    /// Text in double quotes (D21).
    Text(Vec<TextCharacter>),
    Symbol(Symbol),
    EndOfFile,
}

#[derive(Clone, Debug)]
pub struct Token {
    pub kind: TokenKind,
    pub place: Place,
    /// How many spaces come before it on its line; for the first token on a line, its indentation.
    pub spaces_before: usize,
    pub first_on_line: bool,
}

impl Token {
    pub fn is_symbol(&self, symbol: Symbol) -> bool {
        self.kind == TokenKind::Symbol(symbol)
    }

    pub fn is_word(&self, word: &str) -> bool {
        matches!(&self.kind, TokenKind::Word(text) if text == word)
    }

    /// How the token is written, for messages.
    pub fn written(&self) -> String {
        match &self.kind {
            TokenKind::Word(text) | TokenKind::Whole(text) => text.clone(),
            TokenKind::Text(_) => "text".to_string(),
            TokenKind::Symbol(symbol) => symbol.text().to_string(),
            TokenKind::EndOfFile => "the end of the file".to_string(),
        }
    }
}

/// What the tokenizer learns about each line, for the parser's indentation checks (D10).
#[derive(Clone, Debug, Default)]
pub struct LineFacts {
    /// Whether the line holds nothing but spaces.
    pub blank: bool,
    /// Whether its indentation holds a tab.
    pub tab_in_indentation: bool,
}

pub struct Tokens {
    pub tokens: Vec<Token>,
    /// One entry per line, line 1 first.
    pub lines: Vec<LineFacts>,
}

pub fn tokenize(file_index: usize, source: &SourceFile, messages: &mut Vec<Message>) -> Tokens {
    let mut tokens = Vec::new();
    let mut lines = Vec::new();
    for line_number in 1..=source.line_count() {
        let text = source.line_text(line_number);
        lines.push(tokenize_line(
            file_index,
            line_number,
            text,
            &mut tokens,
            messages,
        ));
    }
    let last_line = source.line_count().max(1);
    tokens.push(Token {
        kind: TokenKind::EndOfFile,
        place: Place {
            file: file_index,
            line: last_line,
            column: source.line_text(last_line).chars().count() + 1,
            length: 1,
        },
        spaces_before: 0,
        first_on_line: true,
    });
    Tokens { tokens, lines }
}

fn tokenize_line(
    file: usize,
    line: usize,
    text: &str,
    tokens: &mut Vec<Token>,
    messages: &mut Vec<Message>,
) -> LineFacts {
    let characters: Vec<char> = text.chars().collect();
    let place = |column: usize, length: usize| Place {
        file,
        line,
        column,
        length,
    };
    let mut facts = LineFacts::default();
    if characters.iter().all(|&character| character == ' ') {
        facts.blank = true;
        if !characters.is_empty() {
            // trace: D98
            messages.push(
                Message::refused_with_fix(
                    place(1, characters.len()),
                    Edit {
                        place: place(1, characters.len()),
                        replacement: String::new(),
                    },
                )
                .why("a blank line has no spaces on it."),
            );
        }
        return facts;
    }

    // Indentation: spaces and tabs before the first character. Tabs are refused by the parser's check (D10).
    let mut index = 0;
    while index < characters.len() && matches!(characters[index], ' ' | '\t') {
        if characters[index] == '\t' {
            facts.tab_in_indentation = true;
        }
        index += 1;
    }
    let mut spaces_before = index;
    let mut first_on_line = true;

    while index < characters.len() {
        let character = characters[index];
        let column = index + 1;
        if character == ' ' {
            let start = index;
            while index < characters.len() && characters[index] == ' ' {
                index += 1;
            }
            if index == characters.len() {
                // trace: D98
                messages.push(
                    Message::refused_with_fix(
                        place(start + 1, index - start),
                        Edit {
                            place: place(start + 1, index - start),
                            replacement: String::new(),
                        },
                    )
                    .why("a line never ends with spaces."),
                );
            }
            spaces_before = index - start;
            continue;
        }
        if character == '\t' {
            messages.push(Message::not_designed_yet(
                place(column, 1),
                "a tab between the parts of a line isn't decided",
            ));
            index += 1;
            spaces_before = 1;
            continue;
        }

        if character == '/' && characters.get(index + 1) == Some(&'/') {
            messages.push(Message::not_built_yet(
                place(column, characters.len() - index),
                "comments (D74) aren't built yet",
            ));
            break;
        }
        let start = index;
        // Letters beyond English stay in the word, so the name check can say they aren't decided (D99).
        let kind = if character.is_alphabetic() || character == '_' {
            while index < characters.len()
                && (characters[index].is_alphanumeric() || characters[index] == '_')
            {
                index += 1;
            }
            Some(TokenKind::Word(characters[start..index].iter().collect()))
        } else if character.is_ascii_digit() {
            while index < characters.len()
                && (characters[index].is_ascii_alphanumeric() || characters[index] == '_')
            {
                index += 1;
            }
            let written: String = characters[start..index].iter().collect();
            if written.chars().any(|letter| letter.is_ascii_alphabetic()) {
                // A word starting with a digit, like `2player`: the parser refuses it as a name (D99).
                tokens.push(Token {
                    kind: TokenKind::Word(written),
                    place: place(column, index - start),
                    spaces_before,
                    first_on_line,
                });
                first_on_line = false;
                spaces_before = 0;
                continue;
            }
            if !written.chars().all(|digit| digit.is_ascii_digit())
                || (written.len() > 1 && written.starts_with('0'))
            {
                messages.push(Message::not_designed_yet(
                    place(column, index - start),
                    format!("how whole numbers are written isn't decided, so `{written}` can't be read yet"),
                ));
            }
            Some(TokenKind::Whole(written))
        } else if character == '"' {
            match read_text(&characters, &mut index, file, line, messages) {
                Some(text) => Some(TokenKind::Text(text)),
                None => break,
            }
        } else if character == '\'' {
            // Single quotes aren't used for text (D21).
            let closing = characters[index + 1..]
                .iter()
                .position(|&other| other == '\'');
            if let Some(offset) = closing {
                let inside: String = characters[index + 1..index + 1 + offset].iter().collect();
                let replacement = format!("\"{inside}\"");
                messages.push(
                    Message::refused_with_fix(
                        place(column, offset + 2),
                        Edit {
                            place: place(column, offset + 2),
                            replacement,
                        },
                    )
                    // trace: D85
                    .why("text is written in double quotes, never single ones like `'hi'`."),
                );
                index += offset + 2;
                // Carry on as if it were written with double quotes.
                let text = inside
                    .chars()
                    .enumerate()
                    .map(|(offset, character)| TextCharacter {
                        character,
                        escaped: false,
                        column: column + 1 + offset,
                    })
                    .collect();
                Some(TokenKind::Text(text))
            } else {
                messages.push(Message::not_designed_yet(
                    place(column, 1),
                    "`'` has no meaning in Plainly yet",
                ));
                index += 1;
                None
            }
        } else if let Some((symbol, length)) = read_symbol(&characters, index) {
            index += length;
            Some(TokenKind::Symbol(symbol))
        } else if character == '\r' {
            messages.push(Message::not_designed_yet(
                place(column, 1),
                "line breaks written as `\\r\\n`, as Windows writes them, aren't decided",
            ));
            index += 1;
            None
        } else {
            messages.push(Message::not_designed_yet(
                place(column, 1),
                format!("`{character}` has no meaning in Plainly yet"),
            ));
            index += 1;
            None
        };

        if let Some(kind) = kind {
            tokens.push(Token {
                kind,
                place: place(column, index - start),
                spaces_before,
                first_on_line,
            });
            first_on_line = false;
        }
        spaces_before = 0;
    }
    facts
}

fn read_symbol(characters: &[char], index: usize) -> Option<(Symbol, usize)> {
    let next = characters.get(index + 1).copied();
    let two = |symbol| Some((symbol, 2));
    let one = |symbol| Some((symbol, 1));
    match (characters[index], next) {
        (':', Some(':')) => two(Symbol::DoubleColon),
        ('.', Some('*')) => two(Symbol::DotStar),
        ('=', Some('=')) => two(Symbol::EqualsEquals),
        ('!', Some('=')) => two(Symbol::NotEquals),
        ('|', Some('>')) => two(Symbol::Slide),
        ('?', Some('?')) => two(Symbol::QuestionQuestion),
        (';', _) => one(Symbol::Semicolon),
        (':', _) => one(Symbol::Colon),
        (',', _) => one(Symbol::Comma),
        ('(', _) => one(Symbol::OpenBracket),
        (')', _) => one(Symbol::CloseBracket),
        ('[', _) => one(Symbol::OpenSquare),
        (']', _) => one(Symbol::CloseSquare),
        ('.', _) => one(Symbol::Dot),
        ('=', _) => one(Symbol::Equals),
        ('<', _) => one(Symbol::Less),
        ('>', _) => one(Symbol::Greater),
        ('+', _) => one(Symbol::Plus),
        ('-', _) => one(Symbol::Minus),
        ('*', _) => one(Symbol::Star),
        ('/', _) => one(Symbol::Slash),
        ('&', _) => one(Symbol::Ampersand),
        ('?', _) => one(Symbol::Question),
        _ => None,
    }
}

/// Reads text in double quotes, starting at the opening quote (D21, D30). Gives `None` when the rest of the line
/// can't be read.
fn read_text(
    characters: &[char],
    index: &mut usize,
    file: usize,
    line: usize,
    messages: &mut Vec<Message>,
) -> Option<Vec<TextCharacter>> {
    let start = *index;
    let place = |column: usize, length: usize| Place {
        file,
        line,
        column,
        length,
    };
    let mut text = Vec::new();
    *index += 1;
    loop {
        let Some(&character) = characters.get(*index) else {
            messages.push(Message::not_designed_yet(
                place(start + 1, characters.len() - start),
                "text running onto another line isn't decided, so every text needs its closing `\"` on the same line",
            ));
            *index = characters.len();
            return None;
        };
        let column = *index + 1;
        match character {
            '"' => {
                *index += 1;
                return Some(text);
            }
            // trace: D30
            '\\' => {
                let Some(&next) = characters.get(*index + 1) else {
                    *index += 1;
                    continue;
                };
                if next.is_alphabetic() {
                    messages.push(Message::not_designed_yet(
                        place(column, 2),
                        format!(
                            "`\\` before a letter isn't decided, so `\\{next}` can't be read yet"
                        ),
                    ));
                }
                text.push(TextCharacter {
                    character: next,
                    escaped: true,
                    column: column + 1,
                });
                *index += 2;
            }
            '\t' => {
                messages.push(Message::not_designed_yet(
                    place(column, 1),
                    "a tab inside text isn't decided",
                ));
                *index += 1;
            }
            _ => {
                text.push(TextCharacter {
                    character,
                    escaped: false,
                    column,
                });
                *index += 1;
            }
        }
    }
}
