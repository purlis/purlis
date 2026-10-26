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

/// SK-50 (#3173): what a piece was declared before is read from a log any host's worker
/// writes. `sanitize` let an SGR in it take effect; it is escaped now, as the piece name is.
#[test]
fn an_earlier_declaration_read_from_the_log_is_shown_escaped() {
    let line = already_declared("pie\u{200b}ce", "done \x1b[8mhid\u{202e}", Event::Abandoned);
    assert_eq!(
        line,
        "'pie⟦U+200B⟧ce' was already declared done ⟦U+001B⟧[8mhid⟦U+202E⟧ — recording \
         abandoned over it (the earlier one stays in the log)."
    );
}

/// SK-54 (#3225): the `Say::Done` line of `worktree done` and `abandon` splices the repo, the
/// piece and the worker's reason escaped. The reason used to keep its SGR.
#[test]
fn a_declared_piece_s_line_shows_its_repo_piece_and_reason_escaped() {
    let hostile = "a\x1b[8mb\u{202e}c\u{200b}d";
    let shown = "a⟦U+001B⟧[8mb⟦U+202E⟧c⟦U+200B⟧d";
    let done = declared_line(hostile, hostile, Event::Done, Declaration::Done);
    assert_eq!(done.as_str(), format!("{shown} · {shown} — done"));

    let abandoned = declared_line(
        "repo",
        "piece",
        Event::Abandoned,
        Declaration::Abandoned {
            reason: " blocked \x1b[31mred\u{2066}\u{feff} ",
        },
    );
    assert_eq!(
        abandoned.as_str(),
        "repo · piece — abandoned: blocked ⟦U+001B⟧[31mred⟦U+2066⟧⟦U+FEFF⟧"
    );
}
