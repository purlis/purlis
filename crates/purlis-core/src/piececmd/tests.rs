// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn said(rows: &[[String; 5]]) -> Vec<String> {
    let mut lines = Vec::new();
    table(rows, &mut |say| {
        if let Say::Out(line) = say {
            lines.push(line);
        }
    });
    lines
}

/// SK-47: the last column of `worktree list` and `history` is text from a log, as the others
/// are. A conceal in it used to reach the terminal whole and hide every row after it.
#[test]
fn every_cell_of_a_row_is_shown_escaped_the_last_one_too() {
    let rows = [
        [
            "one\u{202e}".into(),
            "b".into(),
            "c".into(),
            "d".into(),
            "log \x1b[8mhidden".into(),
        ],
        [
            "two".into(),
            "b".into(),
            "c".into(),
            "d".into(),
            "next\tline".into(),
        ],
    ];

    let lines = said(&rows);

    assert!(
        lines
            .iter()
            .all(|line| !line.contains(['\x1b', '\u{202e}', '\t'])),
        "{lines:#?}"
    );
    assert!(lines[0].ends_with("log ⟦U+001B⟧[8mhidden"), "{lines:#?}");
    assert!(lines[1].ends_with("next⟦U+0009⟧line"), "{lines:#?}");
    // The first column is sized on what is shown, so the second starts in one place.
    let at = |line: &str| line[..line.find(" b ").unwrap()].chars().count();
    assert_eq!(at(&lines[0]), at(&lines[1]), "{lines:#?}");
}
