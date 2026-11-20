//! Messages, in D54's layout.
//!
//! Each message's wording is a decision of its own (D54), and none is decided yet: every wording PlainCompiler shows
//! is a draft, listed in `compiler/PROGRESS.md` for the design session.

use crate::source::{Place, SourceFile};

/// What kind of message it is, the first word of its first line (D54, D56).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Rejected while building (D54).
    Refused,
    /// Decided, but PlainCompiler can't do it yet (D56).
    NotBuiltYet,
    /// No decision covers it yet (D56).
    NotDesignedYet,
}

impl Kind {
    fn word(self) -> &'static str {
        match self {
            Kind::Refused => "refused",
            Kind::NotBuiltYet => "NotBuiltYet",
            Kind::NotDesignedYet => "NotDesignedYet",
        }
    }
}

/// A change to the source that fixes a refusal: the part at `place` becomes `replacement`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edit {
    pub place: Place,
    pub replacement: String,
}

/// `because:`: the earlier place the error comes from, as a sentence, then that place (D54).
#[derive(Clone, Debug)]
pub struct Because {
    pub sentence: String,
    pub place: Place,
}

#[derive(Clone, Debug)]
pub struct Message {
    pub kind: Kind,
    pub place: Place,
    /// The fix, when there's exactly one (D54), as a change to the source. `fix:` shows the whole corrected
    /// line, and the change is also used to sort out knock-on errors (D55).
    pub edit: Option<Edit>,
    /// `what:`, shown when there's no single fix (D54).
    pub what: Option<String>,
    pub because: Option<Because>,
    pub why: Option<String>,
}

impl Message {
    fn new(kind: Kind, place: Place) -> Message {
        Message {
            kind,
            place,
            edit: None,
            what: None,
            because: None,
            why: None,
        }
    }

    /// A refusal with `what:`, saying what's wrong.
    pub fn refused(place: Place, what: impl Into<String>) -> Message {
        Message {
            what: Some(what.into()),
            ..Message::new(Kind::Refused, place)
        }
    }

    /// A refusal with exactly one fix: the part at `edit.place` becomes `edit.replacement`.
    pub fn refused_with_fix(place: Place, edit: Edit) -> Message {
        Message {
            edit: Some(edit),
            ..Message::new(Kind::Refused, place)
        }
    }

    /// Something no decision covers yet (D56), with `what:` saying what isn't decided.
    pub fn not_designed_yet(place: Place, what: impl Into<String>) -> Message {
        Message {
            what: Some(what.into()),
            ..Message::new(Kind::NotDesignedYet, place)
        }
    }

    /// Something decided that PlainCompiler can't do yet (D56).
    pub fn not_built_yet(place: Place, what: impl Into<String>) -> Message {
        Message {
            what: Some(what.into()),
            ..Message::new(Kind::NotBuiltYet, place)
        }
    }

    pub fn because(mut self, sentence: impl Into<String>, place: Place) -> Message {
        self.because = Some(Because {
            sentence: sentence.into(),
            place,
        });
        self
    }

    pub fn why(mut self, why: impl Into<String>) -> Message {
        self.why = Some(why.into());
        self
    }

    /// The message as shown, ending with a line break.
    pub fn render(&self, sources: &[SourceFile]) -> String {
        let mut places = vec![self.place];
        if let Some(because) = &self.because {
            places.push(because.place);
        }
        let number_width = places
            .iter()
            .map(|place| place.line.to_string().len())
            .max()
            .unwrap_or(1);

        let mut text = format!(
            "{}: {}\n",
            self.kind.word(),
            place_heading(sources, self.place)
        );
        text.push_str(&source_snippet(sources, self.place, number_width));
        if let Some(edit) = &self.edit {
            text.push_str(&labelled_line("fix", &fixed_line(sources, edit)));
        } else if let Some(what) = &self.what {
            text.push_str(&labelled_line("what", what));
        }
        if let Some(because) = &self.because {
            let sentence = if because.place.file == self.place.file {
                because.sentence.clone()
            } else {
                let path = sources
                    .get(because.place.file)
                    .map(|source| source.shown_path.as_str())
                    .unwrap_or("");
                format!("{} (in {path})", because.sentence)
            };
            text.push_str(&labelled_line("because", &sentence));
            text.push_str(&source_snippet(sources, because.place, number_width));
        }
        if let Some(why) = &self.why {
            text.push_str(&labelled_line("why", why));
        }
        text
    }
}

/// The whole line `edit` is on, with the edit made, for `fix:` (D10).
pub fn fixed_line(sources: &[SourceFile], edit: &Edit) -> String {
    let text = sources
        .get(edit.place.file)
        .map(|source| source.line_text(edit.place.line))
        .unwrap_or("");
    let characters: Vec<char> = text.chars().collect();
    let start = edit.place.column.saturating_sub(1).min(characters.len());
    let end = (start + edit.place.length).min(characters.len());
    let mut fixed: String = characters[..start].iter().collect();
    fixed.push_str(&edit.replacement);
    fixed.extend(&characters[end..]);
    fixed
}

/// `file:line:column, line N column M`: the place written twice, for tools and for people (D54).
pub fn place_heading(sources: &[SourceFile], place: Place) -> String {
    let path = sources
        .get(place.file)
        .map(|source| source.shown_path.as_str())
        .unwrap_or("");
    format!(
        "{path}:{line}:{column}, line {line} column {column}",
        line = place.line,
        column = place.column
    )
}

/// The source line, then `^` under exactly the part in question (D54).
pub fn source_snippet(sources: &[SourceFile], place: Place, number_width: usize) -> String {
    let line_text = sources
        .get(place.file)
        .map(|source| source.line_text(place.line))
        .unwrap_or("");
    let line_length = line_text.chars().count();
    let start = place.column.saturating_sub(1);
    // A place just past the end of the line, like a missing `;`, still gets one `^`.
    let length = place.length.min(line_length.saturating_sub(start)).max(1);
    format!(
        " {number:>number_width$} | {line_text}\n {blank:number_width$} | {spaces}{carets}\n",
        number = place.line,
        blank = "",
        spaces = " ".repeat(start),
        carets = "^".repeat(length),
    )
}

/// A labelled line, with every label's text starting in the same column (D54).
pub fn labelled_line(label: &str, text: &str) -> String {
    format!(" {:<9}{text}\n", format!("{label}:"))
}
