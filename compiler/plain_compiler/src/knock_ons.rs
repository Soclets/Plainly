//! Sorting out knock-on errors (D55 rule 4): the first error's single fix is applied in memory and the program
//! checked again. Anything that disappears was caused by it, and gets a `because:` pointing back at it (D54).

use crate::message::{Because, Edit, Kind, Message, RemovedLines, fixed_line};
use crate::source::{Place, SourceFile};

/// What identifies a message across the two checks: where it is, moved to match the fixed line, and what it says.
type Key = (
    u8,
    usize,
    usize,
    usize,
    Option<String>,
    Option<String>,
    Option<String>,
);

fn kind_number(kind: Kind) -> u8 {
    match kind {
        Kind::Refused => 0,
        Kind::NotBuiltYet => 1,
        Kind::NotDesignedYet => 2,
    }
}

/// The first error's fix: a change to one line, or blank lines removed (D97).
enum Change {
    Edit(Edit),
    RemoveLines(usize, RemovedLines),
}

/// A message's key, in the fixed sources' places: the change moved everything after it on its line, or every line
/// after the removed ones.
fn key(message: &Message, change: Option<&Change>) -> Key {
    let mut line = message.place.line;
    let mut column = message.place.column;
    match change {
        Some(Change::Edit(edit))
            if message.place.file == edit.place.file
                && message.place.line == edit.place.line
                && message.place.column >= edit.place.column + edit.place.length =>
        {
            let added = edit.replacement.chars().count() as isize - edit.place.length as isize;
            column = (column as isize + added).max(1) as usize;
        }
        Some(Change::RemoveLines(file, removed))
            if message.place.file == *file && line >= removed.first_line + removed.count =>
        {
            line -= removed.count;
        }
        _ => {}
    }
    (
        kind_number(message.kind),
        message.place.file,
        line,
        column,
        message.what.clone(),
        message.why.clone(),
        message.edit.as_ref().map(|edit| edit.replacement.clone()),
    )
}

/// The sources with the change made.
fn with_change(sources: &[SourceFile], change: &Change) -> Vec<SourceFile> {
    match change {
        Change::Edit(edit) => with_edit(sources, edit),
        Change::RemoveLines(file, removed) => sources
            .iter()
            .enumerate()
            .map(|(index, source)| {
                let text = if index == *file {
                    source
                        .text
                        .split_inclusive('\n')
                        .enumerate()
                        .filter(|(number, _)| {
                            let line = number + 1;
                            line < removed.first_line || line >= removed.first_line + removed.count
                        })
                        .map(|(_, line)| line)
                        .collect()
                } else {
                    source.text.clone()
                };
                SourceFile::new(source.shown_path.clone(), text)
            })
            .collect(),
    }
}

/// The sources with `edit` made.
fn with_edit(sources: &[SourceFile], edit: &Edit) -> Vec<SourceFile> {
    sources
        .iter()
        .enumerate()
        .map(|(index, source)| {
            if index != edit.place.file {
                return SourceFile::new(source.shown_path.clone(), source.text.clone());
            }
            let mut text = String::new();
            for (number, line) in source.text.split_inclusive('\n').enumerate() {
                if number + 1 == edit.place.line {
                    text.push_str(&fixed_line(sources, edit));
                    if line.ends_with('\n') {
                        text.push('\n');
                    }
                } else {
                    text.push_str(line);
                }
            }
            SourceFile::new(source.shown_path.clone(), text)
        })
        .collect()
}

/// Gives every knock-on of the first error a `because:` pointing at it. `messages` must be sorted by place, and
/// `check` gives the messages for a program.
pub fn sort_out(
    messages: &mut [Message],
    sources: &[SourceFile],
    check: impl Fn(&[SourceFile]) -> Vec<Message>,
) {
    // trace: D55, D83
    let Some(first) = messages
        .iter()
        .position(|message| message.kind == Kind::Refused)
    else {
        return;
    };
    let change = match (&messages[first].edit, messages[first].removed_lines) {
        (_, Some(removed)) => Change::RemoveLines(messages[first].place.file, removed),
        (Some(edit), None) => Change::Edit(edit.clone()),
        (None, None) => return,
    };
    let first_place: Place = messages[first].place;
    let fixed = with_change(sources, &change);
    let remaining: Vec<Key> = check(&fixed)
        .iter()
        .map(|message| key(message, None))
        .collect();
    for (index, message) in messages.iter_mut().enumerate() {
        if index == first || remaining.contains(&key(message, Some(&change))) {
            continue;
        }
        message.because = Some(Because {
            sentence: format!(
                "it follows from the error at line {} column {}",
                first_place.line, first_place.column
            ),
            place: first_place,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn place(line: usize, column: usize) -> Place {
        Place {
            file: 0,
            line,
            column,
            length: 1,
        }
    }

    #[test]
    fn d55_a_knock_on_points_back_at_the_first_error() {
        let sources = vec![SourceFile::new(
            "game.plainly".to_string(),
            "line one\nline two\nline three\n".to_string(),
        )];
        let first = Message::refused_with_fix(
            place(1, 5),
            Edit {
                place: Place {
                    length: 0,
                    ..place(1, 5)
                },
                replacement: "X".to_string(),
            },
        );
        let knock_on = Message::refused(place(2, 1), "goes away with the fix");
        let real = Message::refused(place(3, 1), "stays after the fix");
        let mut messages = vec![first, knock_on, real.clone()];
        let seen_fixed = std::cell::RefCell::new(String::new());
        sort_out(&mut messages, &sources, |fixed| {
            *seen_fixed.borrow_mut() = fixed[0].text.clone();
            vec![real.clone()]
        });
        assert_eq!(*seen_fixed.borrow(), "lineX one\nline two\nline three\n");
        assert!(messages[0].because.is_none());
        assert_eq!(
            messages[1].because.as_ref().map(|because| because.place),
            Some(place(1, 5))
        );
        assert!(messages[2].because.is_none());
    }

    #[test]
    fn d55_a_message_moved_by_the_fix_is_the_same_message() {
        let edit = Edit {
            place: Place {
                length: 0,
                ..place(1, 2)
            },
            replacement: "ab".to_string(),
        };
        let later = Message::refused(place(1, 5), "same");
        let moved = Message::refused(place(1, 7), "same");
        assert_eq!(key(&later, Some(&Change::Edit(edit))), key(&moved, None));
    }
}
