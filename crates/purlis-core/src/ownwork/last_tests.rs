// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

const CONVERSATION: &str = "0b7c1e2a-9d3f-4c5b-8e6a-1f2d3c4b5a69";

/// A folder laid out as Claude Code lays a conversation's out: `<project>/<conversation>/`.
fn a_conversation() -> (tempfile::TempDir, PathBuf) {
    let root = tempfile::tempdir().expect("a folder");
    let at = root.path().join("-Users-me-proj").join(CONVERSATION);
    std::fs::create_dir_all(at.join("tasks")).expect("made");
    std::fs::create_dir_all(at.join("subagents")).expect("made");
    (root, at)
}

fn piece(kind: Kind, id: &str, output: &Path) -> Piece {
    Piece {
        output: Some(output.to_str().expect("utf-8").to_owned()),
        ..Piece::new(kind, id, "work").expect("a piece")
    }
}

#[test]
fn a_shell_s_last_twenty_lines_are_read_with_where_the_rest_is() {
    let (_root, at) = a_conversation();
    let file = at.join("tasks").join("b1.output");
    let text: String = (1..=30).map(|n| format!("line {n}\n")).collect();
    std::fs::write(&file, text).expect("written");

    match of(&piece(Kind::Shell, "b1", &file), CONVERSATION) {
        Last::Lines { lines, path } => {
            assert_eq!(lines.len(), LINES);
            assert_eq!(lines.first().map(String::as_str), Some("line 11"));
            assert_eq!(lines.last().map(String::as_str), Some("line 30"));
            assert_eq!(Path::new(&path), file);
        }
        other => panic!("its lines, not {other:?}"),
    }
}

#[test]
fn a_line_s_control_characters_are_spaces_and_a_long_line_is_cut() {
    let (_root, at) = a_conversation();
    let file = at.join("tasks").join("m1.output");
    std::fs::write(
        &file,
        format!("\u{1b}[31mred\u{1b}[0m\n{}\n", "y".repeat(500)),
    )
    .expect("written");
    let Last::Lines { lines, .. } = of(&piece(Kind::Monitor, "m1", &file), CONVERSATION) else {
        panic!("its lines");
    };
    assert_eq!(lines[0], "[31mred [0m");
    assert_eq!(lines[1].len(), MOST_LINE_BYTES);
}

#[test]
fn an_output_file_that_is_gone_or_was_never_known_reads_as_gone() {
    let (_root, at) = a_conversation();
    let gone = at.join("tasks").join("b1.output");
    assert_eq!(
        of(&piece(Kind::Shell, "b1", &gone), CONVERSATION),
        Last::Gone
    );
    let unknown = Piece::new(Kind::Shell, "b1", "npm test").expect("a piece");
    assert_eq!(of(&unknown, CONVERSATION), Last::Gone);
}

#[test]
fn a_file_that_is_not_the_piece_s_own_is_never_read() {
    let (root, at) = a_conversation();
    let secret = root.path().join("secret.txt");
    std::fs::write(&secret, "the key\n").expect("written");
    // Named outright, under another piece's name, linked to, and linked hard.
    assert_eq!(
        of(&piece(Kind::Shell, "b1", &secret), CONVERSATION),
        Last::Gone
    );
    let other = at.join("tasks").join("b2.output");
    std::fs::write(&other, "x\n").expect("written");
    assert_eq!(
        of(&piece(Kind::Shell, "b1", &other), CONVERSATION),
        Last::Gone
    );
    let linked = at.join("tasks").join("b3.output");
    std::os::unix::fs::symlink(&secret, &linked).expect("linked");
    assert_eq!(
        of(&piece(Kind::Shell, "b3", &linked), CONVERSATION),
        Last::Gone
    );
    let hard = at.join("tasks").join("b4.output");
    std::fs::hard_link(&secret, &hard).expect("linked");
    assert_eq!(
        of(&piece(Kind::Shell, "b4", &hard), CONVERSATION),
        Last::Gone
    );
    // And of another conversation.
    let file = at.join("tasks").join("b5.output");
    std::fs::write(&file, "x\n").expect("written");
    assert_eq!(of(&piece(Kind::Shell, "b5", &file), "another"), Last::Gone);
}

#[test]
fn a_subagent_s_last_message_is_read_from_its_transcript_through_its_output_link() {
    let (_root, at) = a_conversation();
    let transcript = at.join("subagents").join("agent-a7.jsonl");
    let records = [
        serde_json::json!({"type": "user", "message": {"content": "find the callers"}}),
        serde_json::json!({"type": "assistant", "message": {"content": [
            {"type": "text", "text": "Found three callers"}]}}),
        serde_json::json!({"type": "assistant", "message": {"content": [
            {"type": "tool_use", "name": "Grep"}]}}),
        serde_json::json!({"type": "user", "message": {"content": []}}),
    ];
    let text: String = records.iter().map(|one| format!("{one}\n")).collect();
    std::fs::write(&transcript, text).expect("written");
    let output = at.join("tasks").join("a7.output");
    std::os::unix::fs::symlink(&transcript, &output).expect("linked");

    assert_eq!(
        of(&piece(Kind::Subagent, "a7", &output), CONVERSATION),
        Last::Message("Found three callers".to_owned())
    );
}

#[test]
fn a_cron_and_a_wake_up_put_nothing_out() {
    let cron = Piece::new(Kind::Cron, "c1", "").expect("a piece");
    assert_eq!(of(&cron, CONVERSATION), Last::Nothing);
}
