//! Tests of what PlainCompiler refuses, named after the decision each one checks. They check the kind of message
//! and its line, never its wording, which isn't decided yet (D54).

use crate::front_end;
use crate::message::{Kind, Message};
use crate::source::SourceFile;

fn messages(program: &str) -> Vec<Message> {
    let sources = vec![SourceFile::new(
        "test.plainly".to_string(),
        program.to_string(),
    )];
    let mut messages = front_end(&sources).0;
    messages.sort_by_key(|message| message.place);
    messages
}

/// Each message's kind and line.
fn found(program: &str) -> Vec<(Kind, usize)> {
    messages(program)
        .iter()
        .map(|message| (message.kind, message.place.line))
        .collect()
}

/// A program whose `proc.main` body is `body`, each line indented one level.
fn main_with(body: &[&str]) -> String {
    let mut program = "proc.main:\n".to_string();
    for line in body {
        program.push_str("    ");
        program.push_str(line);
        program.push('\n');
    }
    program.push_str("end proc.main\n");
    program
}

fn refused_on(line: usize) -> Vec<(Kind, usize)> {
    vec![(Kind::Refused, line)]
}

#[test]
fn d04_hello_world_is_accepted() {
    assert_eq!(found(&main_with(&["show_msg(\"Hello, World!\");"])), vec![]);
}

#[test]
fn d04_empty_brackets_on_a_definition() {
    let program = "proc greet():\n    show_msg(\"hi\");\nend proc greet\n\nproc.main:\n    greet();\nend proc.main\n";
    assert_eq!(found(program), refused_on(1));
}

#[test]
fn d07_missing_semicolon() {
    let found = messages(&main_with(&["show_msg(\"a\")", "show_msg(\"b\");"]));
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].kind, Kind::Refused);
    assert_eq!(
        found[0].edit.as_ref().map(|edit| edit.replacement.as_str()),
        Some(";")
    );
}

#[test]
fn d08_wrong_closer() {
    let program = "proc.main:\n    show_msg(\"a\");\nend proc.mian\n";
    assert_eq!(found(program), refused_on(3));
}

#[test]
fn d09_nothing_after_an_if_that_can_leave() {
    let program = main_with(&[
        "var n: Whole = 0;",
        "while n < 3:",
        "    if n == 1:",
        "        break;",
        "    end if",
        "    n = n + 1;",
        "end while",
    ]);
    assert_eq!(found(&program), refused_on(7));
}

#[test]
fn d10_indentation_and_tabs() {
    let program = "proc.main:\n  show_msg(\"a\");\n\tshow_msg(\"b\");\nend proc.main\n";
    assert_eq!(found(program), vec![(Kind::Refused, 2), (Kind::Refused, 3)]);
}

#[test]
fn d14_a_let_never_changes() {
    assert_eq!(
        found(&main_with(&["let lives: Whole = 3;", "lives = 2;"])),
        refused_on(3)
    );
}

#[test]
fn d14_no_equals_in_a_condition() {
    let program = main_with(&[
        "let n: Whole = 3;",
        "if n = 3:",
        "    show_msg(\"three\");",
        "end if",
    ]);
    assert_eq!(found(&program), refused_on(3));
}

#[test]
fn d17_spaces_around_less_than() {
    let program = main_with(&[
        "let n: Whole = 3;",
        "if n<4:",
        "    show_msg(\"small\");",
        "end if",
    ]);
    let found = messages(&program);
    assert_eq!(found.len(), 1, "one message for both sides of `<`");
    assert_eq!(found[0].kind, Kind::Refused);
}

#[test]
fn d18_no_spaces_around_a_narrowing() {
    assert_eq!(
        found(&main_with(&["show_msg . same_line(\"a\");"])),
        refused_on(2)
    );
}

#[test]
fn d22_converting_to_the_same_type() {
    assert_eq!(
        found(&main_with(&["show_msg(be.Text(\"a\"));"])),
        refused_on(2)
    );
}

#[test]
fn d23_a_failure_must_be_handled() {
    assert_eq!(
        found(&main_with(&["let n: Whole = be.Whole(\"4\");"])),
        refused_on(2)
    );
    assert_eq!(
        found(&main_with(&["let n: Whole = be.Whole(\"4\") ?? 0;"])),
        vec![]
    );
}

#[test]
fn d24_every_label_is_written() {
    assert_eq!(
        found(&main_with(&["let n: Whole = random(1, to: 6);"])),
        refused_on(2)
    );
    assert_eq!(
        found(&main_with(&["let n: Whole = random(from: 1, upto: 6);"])),
        refused_on(2)
    );
}

#[test]
fn d24_label_order_is_not_designed_yet() {
    let found = found(&main_with(&["let n: Whole = random(to: 6, from: 1);"]));
    assert_eq!(found, vec![(Kind::NotDesignedYet, 2)]);
}

#[test]
fn d28_a_number_joined_with_text() {
    let program = main_with(&["let n: Whole = 3;", "show_msg(\"n: \" & n);"]);
    assert_eq!(found(&program), refused_on(3));
}

#[test]
fn d29_braces_hold_only_a_variable() {
    assert_eq!(
        found(&main_with(&["show_msg.fmt(\"{nobody}\");"])),
        refused_on(2)
    );
}

#[test]
fn d31_narrowings_go_strongest_first() {
    let program = main_with(&["let n: Whole = 3;", "show_msg.same_line.fmt(\"{n}\");"]);
    assert_eq!(found(&program), refused_on(3));
}

#[test]
fn d32_break_outside_a_loop() {
    assert_eq!(found(&main_with(&["break;"])), refused_on(2));
}

#[test]
fn d38_nothing_given_back_is_not_a_value() {
    let program = "proc greet:\n    show_msg(\"hi\");\nend proc greet\n\nproc.main:\n    let a: Text = greet();\nend proc.main\n";
    assert_eq!(found(program), refused_on(6));
}

#[test]
fn d55_a_refused_value_still_declares_its_variable() {
    let program = main_with(&["let n: Whole = 5 6;", "show_msg(be.Text(n));"]);
    assert_eq!(found(&program), refused_on(2));
}

#[test]
fn d57_every_way_returns_a_value() {
    let program = "proc pick(Whole a, b) returns Whole:\n    if a > b:\n        return a;\n    end if\nend proc pick\n\nproc.main:\n    show_msg(be.Text(pick(1, 2)));\nend proc.main\n";
    assert_eq!(found(program), refused_on(1));
}

#[test]
fn d58_a_keyword_is_not_a_name() {
    assert_eq!(found(&main_with(&["let while: Whole = 1;"])), refused_on(2));
}

#[test]
fn d61_parameters_sharing_a_type_write_it_once() {
    let program = "proc add(a: Whole, b: Whole) returns Whole:\n    return a + b;\nend proc add\n\nproc.main:\n    show_msg(be.Text(add(1, 2)));\nend proc.main\n";
    assert_eq!(found(program), refused_on(1));
}

#[test]
fn d64_exactly_one_proc_main() {
    let program = "proc helper:\n    show_msg(\"hi\");\nend proc helper\n";
    assert_eq!(found(program), refused_on(1));
}

#[test]
fn d72_join_next_to_maths_needs_brackets() {
    let program = main_with(&["let a: Whole = 1;", "show_msg(\"Total: \" & a + 2);"]);
    assert_eq!(found(&program), refused_on(3));
    let bracketed = main_with(&[
        "let a: Whole = 1;",
        "show_msg(\"Total: \" & be.Text(a + 2));",
    ]);
    assert_eq!(found(&bracketed), vec![]);
}

#[test]
fn d72_maths_order_is_accepted() {
    let program = main_with(&[
        "let a: Whole = 2 + 3 * 4;",
        "let b: Whole = 10 - 4 - 3;",
        "if a + 1 < 20 and b * 2 == 6:",
        "    show_msg(\"ok\");",
        "end if",
    ]);
    assert_eq!(found(&program), vec![]);
}

#[test]
fn d73_a_slide_takes_everything_on_its_left() {
    assert_eq!(
        found(&main_with(&["2 * 3 + 1 |> be.Text |> show_msg;"])),
        vec![]
    );
}

#[test]
fn d79_d80_list_facts_and_items_are_not_built_yet() {
    let program = main_with(&[
        "var n: Whole = 3;",
        "show_msg(be.Text(n.len));",
        "n[pos: 1] = 2;",
    ]);
    assert_eq!(
        found(&program),
        vec![(Kind::NotBuiltYet, 3), (Kind::NotBuiltYet, 4)]
    );
}

#[test]
fn d81_question_question_sticks_to_its_call() {
    let program = main_with(&[
        "let score: Whole = 1;",
        "let n: Whole = score + be.Whole(\"4\") ?? 0;",
    ]);
    assert_eq!(found(&program), vec![]);
    assert_eq!(
        found(&main_with(&["let n: Whole = 3 ?? 0;"])),
        refused_on(2)
    );
}

#[test]
fn d81_two_comparisons_in_a_row() {
    let program = main_with(&[
        "let x: Whole = 5;",
        "if 1 < x < 10:",
        "    show_msg(\"in\");",
        "end if",
    ]);
    let found = messages(&program);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].kind, Kind::Refused);
    assert_eq!(
        found[0].edit.as_ref().map(|edit| edit.replacement.as_str()),
        Some(" and x")
    );
}

#[test]
fn d81_an_operator_after_a_step() {
    let program = main_with(&["let x: Whole = 5;", "let n: Whole = x |> double + 1;"]);
    let found = messages(&program);
    assert_eq!(found.len(), 1);
    assert_eq!(
        found[0].edit.as_ref().map(|edit| edit.replacement.as_str()),
        Some("(x |> double)")
    );
}

#[test]
fn d93_a_variable_ends_with_its_block() {
    let program = main_with(&[
        "if true:",
        "    let message: Text = \"Still alive\";",
        "    show_msg(message);",
        "end if",
        "show_msg(message);",
        "let message: Text = \"again\";",
        "show_msg(message);",
    ]);
    assert_eq!(found(&program), refused_on(6));
}
