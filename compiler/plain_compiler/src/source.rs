//! Source files and places in them.

/// One `.plainly` file of the program (D35, D64).
pub struct SourceFile {
    /// The path as messages show it.
    pub shown_path: String,
    pub text: String,
    /// The byte where each line starts; line 1 is `line_starts[0]`.
    line_starts: Vec<usize>,
}

impl SourceFile {
    pub fn new(shown_path: String, text: String) -> SourceFile {
        let mut line_starts = vec![0];
        for (index, byte) in text.bytes().enumerate() {
            if byte == b'\n' && index + 1 < text.len() {
                line_starts.push(index + 1);
            }
        }
        SourceFile {
            shown_path,
            text,
            line_starts,
        }
    }

    pub fn line_count(&self) -> usize {
        if self.text.is_empty() {
            0
        } else {
            self.line_starts.len()
        }
    }

    /// The text of a line, without its line break. Lines count from 1.
    pub fn line_text(&self, line: usize) -> &str {
        let Some(&start) = self.line_starts.get(line.wrapping_sub(1)) else {
            return "";
        };
        let end = self
            .line_starts
            .get(line)
            .copied()
            .unwrap_or(self.text.len());
        let text = self.text.get(start..end).unwrap_or("");
        text.strip_suffix('\n').unwrap_or(text)
    }
}

/// A place in a source file: a line, a column, and how many characters the part in question covers. Lines and
/// columns count from 1, and columns count characters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Place {
    pub file: usize,
    pub line: usize,
    pub column: usize,
    pub length: usize,
}

impl Place {
    /// The place from the start of `self` to the end of `other`, when both are on one line.
    pub fn to(self, other: Place) -> Place {
        if other.file == self.file && other.line == self.line && other.column >= self.column {
            Place {
                length: other.column + other.length - self.column,
                ..self
            }
        } else {
            self
        }
    }
}
