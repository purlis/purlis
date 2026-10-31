// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! SK-50 (#3173): a skill's name, a name its text says and a commit's author and subject are
//! not purlis's, so each is spliced into its line escaped: a conceal (`\x1b[8m`), a bidi
//! override or a zero-width character is shown as its mark and never obeyed.

use purlis_core::skills::graph::{Use, UsesOf};
use purlis_core::skills::history::{History, Version};
use purlis_core::skilluse::Totals;

use super::*;

/// A name carrying a control, a bidi override and a zero-width character.
const HOSTILE: &str = "de\x1b[8mploy\u{202e}x\u{200b}";
const SHOWN: &str = "de⟦U+001B⟧[8mploy⟦U+202E⟧x⟦U+200B⟧";

#[test]
fn skill_restore_lists_a_deleted_name_escaped() {
    let line = restorable(
        HOSTILE,
        &Level::workspace("w\u{2066}s"),
        "kept in the project's history",
    );
    assert_eq!(
        line.as_str(),
        format!("{SHOWN}  workspace w⟦U+2066⟧s · kept in the project's history")
    );
}

#[test]
fn skill_uses_shows_each_said_name_escaped() {
    let node = UsesOf {
        uses: vec![
            Use {
                said: format!("skills:{HOSTILE}"),
                name: HOSTILE.into(),
                project: true,
                broken: true,
            },
            Use {
                said: format!("purlis:{HOSTILE}"),
                name: HOSTILE.into(),
                project: false,
                broken: false,
            },
        ],
        used_by: vec![HOSTILE.into()],
        cut: false,
    };
    let lines: Vec<String> = uses_lines(HOSTILE, &node)
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(
        lines,
        [
            format!("uses skills:{SHOWN}  (no skill has this name)"),
            format!("uses purlis:{SHOWN}"),
            format!("used by {SHOWN}"),
        ]
    );
    let none = UsesOf {
        uses: vec![],
        used_by: vec![],
        cut: false,
    };
    let lines: Vec<String> = uses_lines(HOSTILE, &none)
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(
        lines,
        [
            format!("{SHOWN} names no other skill."),
            format!("No skill of the project names {SHOWN}."),
        ]
    );
}

#[test]
fn skill_versions_shows_a_commit_s_author_and_subject_escaped() {
    let history = History {
        versions: vec![Version {
            commit: "0123456789abcdef0123456789abcdef01234567".into(),
            at: 0,
            author: HOSTILE.into(),
            subject: format!("fix\t{HOSTILE}"),
            tree: String::new(),
            runs: Totals::default(),
            grade: None,
            folder: "skills/deploy".into(),
            name: "deploy".into(),
            before: None,
        }],
        uncommitted: Totals::default(),
    };
    let table = versions_table(HOSTILE, &history);
    assert!(
        !table.contains(['\x1b', '\u{202e}', '\u{200b}', '\t']),
        "{table}"
    );
    assert!(
        table.contains(&format!("  {SHOWN}  fix⟦U+0009⟧{SHOWN}  (no runs)\n")),
        "{table}"
    );
    let empty = versions_table(HOSTILE, &History::default());
    assert!(
        empty.starts_with(&format!("{SHOWN} has no history yet")),
        "{empty}"
    );
}

#[test]
fn skill_owners_shows_each_owner_escaped_one_a_line() {
    let lines: Vec<String> = owners_lines(HOSTILE, &[HOSTILE.into(), "ops\nroot".into()])
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(lines, [SHOWN.to_string(), "ops⟦U+000A⟧root".to_string()]);
    let none: Vec<String> = owners_lines(HOSTILE, &[])
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(none, [format!("{SHOWN} names no owners.")]);
}

/// SK-54 (#3225): the lines that say a skill was already so splice its name, and the
/// workspace's, escaped.
#[test]
fn a_skill_already_so_is_said_with_its_name_escaped() {
    assert_eq!(
        already_switched(HOSTILE, "on", "w\u{2066}s").as_str(),
        format!("{SHOWN} was already on in w⟦U+2066⟧s.")
    );
    assert_eq!(
        owners_unchanged(HOSTILE).as_str(),
        format!("{SHOWN}'s owners were already those.")
    );
    assert_eq!(
        already_in(HOSTILE, &Lifecycle::Active).as_str(),
        format!("{SHOWN} was already {}.", Lifecycle::Active.word())
    );
}
