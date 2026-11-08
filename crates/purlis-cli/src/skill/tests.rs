// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! SK-50 (#3173): a skill's name, a name its text says and a commit's author and subject are
//! not purlis's, so each is spliced into its line escaped: a conceal (`\x1b[8m`), a bidi
//! override or a zero-width character is shown as its mark and never obeyed.

use purlis_core::forge::Fork;
use purlis_core::forge::backend::Visibility;
use purlis_core::skills::graph::{Use, UsesOf};
use purlis_core::skills::history::{History, Version};
use purlis_core::skills::source::publish::Route;
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

#[test]
fn a_publish_asks_only_at_a_terminal_outside_any_chat_and_a_doubt_counts_as_a_chat() {
    use purlis_core::secrets::setup::Where;
    let unsure = Where::Unsure("the record could not be read".to_owned());

    assert!(may_publish(false, &Where::Outside, true));
    assert!(!may_publish(false, &Where::Inside, true));
    assert!(!may_publish(false, &unsure, true));
    assert!(!may_publish(true, &Where::Outside, true));
    assert!(!may_publish(false, &Where::Outside, false));
}

#[test]
fn a_publish_to_the_source_itself_says_nothing_of_a_fork() {
    assert_eq!(route_said(&Route::Source, "GitHub"), None);
}

#[test]
fn a_publish_that_makes_a_fork_names_it_who_reads_it_and_that_it_is_made_on_a_yes() {
    let said = route_said(
        &Route::NewFork {
            path: "ada/skills".to_owned(),
            visibility: Visibility::Public,
        },
        "GitHub",
    )
    .unwrap();
    assert!(
        said.starts_with("GitHub says your account cannot push"),
        "{said}"
    );
    assert!(said.contains("makes your fork ada/skills"), "{said}");
    assert!(said.contains("PUBLIC: everyone can read"), "{said}");
    assert!(said.contains("Nothing is made until you say yes"), "{said}");
    assert!(said.contains("`purlis skill export`"), "{said}");
}

#[test]
fn a_publish_through_a_fork_the_person_has_names_it_and_makes_nothing() {
    let fork = Fork::for_a_test("9", "ada/skills", "https://github.com/ada/skills.git", None);
    let said = route_said(&Route::Fork(fork), "GitHub").unwrap();
    assert!(said.contains("https://github.com/ada/skills"), "{said}");
    assert!(!said.contains("makes"), "{said}");
}

#[test]
fn an_extensions_browse_lists_each_skill_escaped_with_what_its_files_do_and_where_it_is() {
    use purlis_core::skills::source::extension::Held;
    let plane = tempfile::tempdir().unwrap();
    let skill_md = format!("---\nname: {HOSTILE}\ndescription: Ships it.\n---\n\nGo.\n");
    let browse = Held::of(
        "acme",
        "r7",
        &[
            (
                HOSTILE,
                "Ships it.",
                &[
                    ("SKILL.md", skill_md.as_str()),
                    ("scripts/ship.sh", "#!/bin/sh\n"),
                ],
            ),
            ("notify", "Tells the team.", &[]),
        ],
    )
    .browse(plane.path());
    assert_eq!(
        browse_text(&browse),
        format!(
            "the extension acme at revision r7:\n  {SHOWN}  Ships it. · runs a script\n  notify  \
             Tells the team. · its files are read when you review it\n"
        )
    );
}
