// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Proposed changes the person applies (#1975): held, shown, applied, dismissed and stale.

use super::*;
use purlis_core::proposed::{Edit, base_of};

fn project() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("a folder");
    let root = dir.path().canonicalize().expect("real").join("plane");
    std::fs::create_dir_all(root.join("personas/devops")).expect("a persona");
    std::fs::write(root.join("purlis.toml"), "[plane]\n").expect("a manifest");
    (dir, root)
}

fn mode_pr() -> Change {
    Change::Edits {
        edits: vec![Edit {
            old: "[plane]".to_owned(),
            new: "[plane]\nmode = \"pr\"".to_owned(),
            every: false,
        }],
    }
}

fn ask(chat: u32, base: Option<String>, change: Change) -> ProposeAsk {
    ProposeAsk {
        chat,
        path: "purlis.toml".to_owned(),
        base,
        change,
    }
}

/// A token-shaped value, built where it is used so no source file holds one.
fn token() -> String {
    ["ghp", "_0123456789abcdefABCDEFghij0123456789"].concat()
}

fn manifest(root: &Path) -> String {
    std::fs::read_to_string(root.join("purlis.toml")).expect("the manifest")
}

/// PC-9: one a file a chat, the newer in the older's place; two chats' are both held; and a
/// chat's proposals end with it.
#[test]
fn a_chat_holds_one_proposal_a_file_and_two_chats_both_are_held() {
    let held = Proposals::default();
    let first = held
        .note(3, "purlis.toml", None, mode_pr(), 1)
        .expect("held");
    let newer = held
        .note(3, "purlis.toml", None, mode_pr(), 2)
        .expect("held");
    assert_ne!(first, newer);
    assert_eq!(held.every().len(), 1);
    assert!(held.get(first).is_none(), "the newer took its place");
    held.note(4, "purlis.toml", None, mode_pr(), 3)
        .expect("held");
    assert_eq!(held.every().len(), 2);
    assert_eq!(held.others_stale("purlis.toml"), vec![3, 4]);
    assert!(held.others_stale("purlis.toml").is_empty(), "told once");
    held.forget(3);
    assert_eq!(held.every().len(), 1);
    for at in 0..AT_MOST_PER_CHAT {
        held.note(5, &format!("personas/p{at}/persona.md"), None, mode_pr(), 4)
            .expect("held");
    }
    assert!(
        held.note(5, "personas/more/persona.md", None, mode_pr(), 5)
            .is_none()
    );
}

/// PC-3, PC-8: a proposal is held only where it applies to the file as the app reads it; one
/// worked out against another file is refused as changed since; nothing is written either way.
#[test]
fn a_proposal_is_held_only_where_it_applies_to_the_file_as_it_is() {
    let (_dir, root) = project();
    let held = Proposals::default();
    let base = base_of(Some(b"[plane]\n"));
    assert_eq!(
        asked_in(&root, &held, &ask(3, base.clone(), mode_pr()), 1),
        Ok(())
    );
    assert_eq!(held.every().len(), 1);
    assert_eq!(manifest(&root), "[plane]\n", "nothing written");

    let other_base = base_of(Some(b"[plane]\nname = \"x\"\n"));
    assert_eq!(
        asked_in(&root, &held, &ask(4, other_base, mode_pr()), 1),
        Err(CHANGED_SINCE.to_owned())
    );
    let nothing_to_replace = Change::Edits {
        edits: vec![Edit {
            old: "absent".to_owned(),
            new: "x".to_owned(),
            every: false,
        }],
    };
    assert_eq!(
        asked_in(&root, &held, &ask(4, base, nothing_to_replace), 1),
        Err(proposed::UNPREPARED.to_owned())
    );
    let mut not_ours = ask(4, None, mode_pr());
    not_ours.path = "notes.md".to_owned();
    assert!(asked_in(&root, &held, &not_ours, 1).is_err());
    assert_eq!(held.every().len(), 1);
}

/// PC-14: policy's "off" holds none.
#[test]
fn policy_off_holds_no_proposal() {
    let (_dir, root) = project();
    purlis_core::sandbox::policy::set_for_this_test(purlis_core::sandbox::policy::Locks::parse(
        r#"{"sandbox": {"proposed-changes": "off"}}"#,
        Path::new("/etc/purlis/policy.json"),
    ));
    let held = Proposals::default();
    let said = asked_in(
        &root,
        &held,
        &ask(3, base_of(Some(b"[plane]\n")), mode_pr()),
        1,
    );
    purlis_core::sandbox::policy::set_for_this_test(purlis_core::sandbox::policy::Locks::none());
    assert!(
        said.expect_err("refused")
            .starts_with("Policy turns proposed changes off")
    );
    assert!(held.every().is_empty());
}

/// PC-3, PC-4, PC-8: Apply writes exactly the change made to the file as it is, audited first;
/// an audit that fails writes nothing; a file that changed since is stale and nothing written.
#[test]
fn apply_writes_the_change_audited_first_and_only_while_the_file_is_what_it_was() {
    let (_dir, root) = project();
    let one = Proposal {
        id: 1,
        session: 3,
        path: "purlis.toml".to_owned(),
        base: base_of(Some(b"[plane]\n")),
        change: mode_pr(),
        since: 1,
        stale: false,
    };
    let refused = |_: &proposed::Applied<'_>| Err("the event log is not open".to_owned());
    assert_eq!(
        apply_in(
            &root,
            &one,
            Some("devops"),
            &refused,
            &Mutex::new(()),
            From::Row
        ),
        Err(Refusal::Not("the event log is not open".to_owned()))
    );
    assert_eq!(manifest(&root), "[plane]\n", "nothing written unaudited");

    let audited = std::cell::RefCell::new(Vec::new());
    let audit = |applied: &proposed::Applied<'_>| {
        audited.borrow_mut().push(applied.body());
        Ok(())
    };
    let said = apply_in(
        &root,
        &one,
        Some("devops"),
        &audit,
        &Mutex::new(()),
        From::Row,
    )
    .expect("applied");
    assert!(said.starts_with("Applied to purlis.toml"), "{said}");
    assert_eq!(manifest(&root), "[plane]\nmode = \"pr\"\n");
    let bodies: Vec<serde_json::Value> = audited.borrow_mut().drain(..).collect();
    // One record before the write, one after it, so none says a file holds what was never
    // written.
    let outcomes: Vec<&str> = bodies
        .iter()
        .map(|one| one["outcome"].as_str().unwrap())
        .collect();
    assert_eq!(outcomes, ["intent", "applied"]);
    let body = &bodies[1];
    assert_eq!(body["persona"], "devops");
    assert_eq!(body["file"], "purlis.toml");
    assert_eq!(
        body["written"],
        base_of(Some(b"[plane]\nmode = \"pr\"\n")).expect("a hash")
    );

    // The file is not what it was proposed against any more: stale, nothing written.
    assert_eq!(
        apply_in(&root, &one, None, &audit, &Mutex::new(()), From::Row),
        Err(Refusal::Stale)
    );
    assert_eq!(manifest(&root), "[plane]\nmode = \"pr\"\n");
    // One marked stale is never applied.
    let marked = Proposal {
        stale: true,
        base: base_of(Some(b"[plane]\nmode = \"pr\"\n")),
        ..one
    };
    assert_eq!(
        apply_in(&root, &marked, None, &audit, &Mutex::new(()), From::Row),
        Err(Refusal::Stale)
    );
}

/// PC-7, PC-8, PC-13: Apply is offered only beside a change shown whole that still applies;
/// Dismiss always; never "always apply".
#[test]
fn apply_is_offered_only_beside_a_change_shown_whole() {
    let one = Proposal {
        id: 7,
        session: 3,
        path: "purlis.toml".to_owned(),
        base: None,
        change: mode_pr(),
        since: 1,
        stale: false,
    };
    let ids = |shown: &crate::asking::Shown| -> Vec<String> {
        shown.options.iter().map(|one| one.id.clone()).collect()
    };
    let made = crate::asking::proposed_change(
        &one,
        &Now::Made {
            before: "[plane]\n".to_owned(),
            after: "[plane]\nmode = \"pr\"\n".to_owned(),
        },
    );
    assert_eq!(ids(&made), ["apply", "dismiss"]);
    assert_eq!(made.source, crate::asking::AskSource::ProposedChange);
    assert_eq!(
        made.answer,
        crate::asking::AnswerPath::ProposedChange {
            id: 7,
            path: "purlis.toml".to_owned()
        }
    );
    assert!(matches!(
        made.writes,
        Some(crate::askwrites::WritesShown::Whole { .. })
    ));

    let long = "line\n".repeat(purlis_core::harness::asked::edit::MOST_LINES + 1);
    let too_long = crate::asking::proposed_change(
        &one,
        &Now::Made {
            before: long.clone(),
            after: format!("{long}more\n"),
        },
    );
    // PC-7: too long for a row, it opens its own tab, where Apply is pressed.
    assert_eq!(ids(&too_long), ["open", "dismiss"]);
    assert!(matches!(
        too_long.writes,
        Some(crate::askwrites::WritesShown::NotShown { .. })
    ));
    for now in [Now::Stale, Now::Unprepared] {
        assert_eq!(
            ids(&crate::asking::proposed_change(&one, &now)),
            ["dismiss"]
        );
    }
}

/// PC-7, S3: Apply judges again that the change is shown whole; one that is not (a
/// credential's shape here) is never written, whatever asked for it.
#[test]
fn apply_never_writes_a_change_not_shown_whole() {
    let (_dir, root) = project();
    let one = Proposal {
        id: 1,
        session: 3,
        path: "purlis.toml".to_owned(),
        base: base_of(Some(b"[plane]\n")),
        change: Change::Whole {
            content: format!("token = \"{}\"\n", token()),
        },
        since: 1,
        stale: false,
    };
    let audit = |_: &proposed::Applied<'_>| -> Result<(), String> {
        panic!("nothing is recorded for a change never written")
    };
    assert_eq!(
        apply_in(&root, &one, None, &audit, &Mutex::new(()), From::Row),
        Err(Refusal::Not(NOT_SHOWN_SAID.to_owned()))
    );
    assert_eq!(manifest(&root), "[plane]\n");
}

/// PC-8: two Applies against the file as it was, at once: one writes, the other finds it
/// changed and is stale. Never both.
#[test]
fn two_applies_against_the_same_file_as_it_was_never_both_write() {
    let (_dir, root) = project();
    let lock = std::sync::Arc::new(Mutex::new(()));
    let proposal = |id: u32, line: &str| Proposal {
        id,
        session: id,
        path: "purlis.toml".to_owned(),
        base: base_of(Some(b"[plane]\n")),
        change: Change::Edits {
            edits: vec![Edit {
                old: "[plane]".to_owned(),
                new: format!("[plane]\n{line}"),
                every: false,
            }],
        },
        since: 1,
        stale: false,
    };
    let handles: Vec<_> = [
        proposal(1, "mode = \"pr\""),
        proposal(2, "mode = \"local\""),
    ]
    .into_iter()
    .map(|one| {
        let (root, lock) = (root.clone(), std::sync::Arc::clone(&lock));
        std::thread::spawn(move || apply_in(&root, &one, None, &|_| Ok(()), &lock, From::Row))
    })
    .collect();
    let results: Vec<_> = handles.into_iter().map(|one| one.join().unwrap()).collect();
    assert_eq!(
        results.iter().filter(|one| one.is_ok()).count(),
        1,
        "{results:?}"
    );
    assert_eq!(
        results
            .iter()
            .filter(|one| **one == Err(Refusal::Stale))
            .count(),
        1,
        "{results:?}"
    );
}

/// PC-4: a write that fails is recorded as failed, after the intent; nothing says applied.
#[cfg(unix)]
#[test]
fn a_write_that_fails_is_recorded_as_failed() {
    use std::os::unix::fs::PermissionsExt as _;
    let (_dir, root) = project();
    let one = Proposal {
        id: 1,
        session: 3,
        path: "purlis.toml".to_owned(),
        base: base_of(Some(b"[plane]\n")),
        change: mode_pr(),
        since: 1,
        stale: false,
    };
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o555)).expect("read-only");
    let audited = std::cell::RefCell::new(Vec::new());
    let audit = |applied: &proposed::Applied<'_>| {
        audited.borrow_mut().push(applied.body());
        Ok(())
    };
    let result = apply_in(&root, &one, None, &audit, &Mutex::new(()), From::Row);
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o755)).expect("writable");
    assert!(matches!(result, Err(Refusal::Not(_))), "{result:?}");
    let outcomes: Vec<String> = audited
        .borrow()
        .iter()
        .map(|one| one["outcome"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(outcomes, ["intent", "failed"]);
    assert_eq!(manifest(&root), "[plane]\n");
}

/// PC-7: a change too long for a row is shown whole on its own tab, with Apply there; one not
/// shown for anything but its length (a credential's shape) is shown nowhere, and offers no
/// Apply; a stale one says why.
#[test]
fn a_change_too_long_for_a_row_is_shown_whole_on_its_tab() {
    let one = Proposal {
        id: 7,
        session: 3,
        path: "personas/devops/persona.md".to_owned(),
        base: None,
        change: mode_pr(),
        since: 1,
        stale: false,
    };
    let long = "line\n".repeat(purlis_core::harness::asked::edit::MOST_LINES + 1);
    let whole = whole_of(
        &one,
        &Now::Made {
            before: long.clone(),
            after: format!("{long}more\n"),
        },
    );
    assert!(whole.applies);
    assert_eq!(whole.why, None);
    assert_eq!(whole.after.lines().count(), long.lines().count() + 1);

    let secret = whole_of(
        &one,
        &Now::Made {
            before: String::new(),
            after: format!("token = \"{}\"\n", token()),
        },
    );
    assert!(!secret.applies);
    assert_eq!(secret.after, "");
    assert!(secret.why.is_some());

    let stale = whole_of(&one, &Now::Stale);
    assert!(!stale.applies);
    assert_eq!(stale.why.as_deref(), Some(STALE_SAID));
}

/// #2121: a long change's tab marks the lines that changed, git's hunks between the two sides,
/// not the whole file as one hunk, so the merge view folds every unchanged stretch.
#[test]
fn a_long_change_on_its_tab_marks_only_the_lines_that_changed() {
    let one = Proposal {
        id: 7,
        session: 3,
        path: "personas/devops/persona.md".to_owned(),
        base: None,
        change: mode_pr(),
        since: 1,
        stale: false,
    };
    let lines = |changed: &str| {
        (1..=2000)
            .map(|at| {
                if at == 1000 {
                    format!("{changed}\n")
                } else {
                    format!("line {at}\n")
                }
            })
            .collect::<String>()
    };
    let whole = whole_of(
        &one,
        &Now::Made {
            before: lines("line 1000"),
            after: lines("the one line that changed"),
        },
    );
    assert!(whole.applies);
    assert_eq!(
        whole.hunks,
        [crate::piecefiles::GitHunk {
            old_start: 1000,
            old_lines: 1,
            new_start: 1000,
            new_lines: 1,
        }]
    );
}

/// The hunks the tab is sent for a change from `before` to `after`, as `(old_start, old_lines,
/// new_start, new_lines)`.
fn tab_hunks(before: &str, after: &str) -> Vec<(u32, u32, u32, u32)> {
    let one = Proposal {
        id: 7,
        session: 3,
        path: "personas/devops/persona.md".to_owned(),
        base: None,
        change: mode_pr(),
        since: 1,
        stale: false,
    };
    let now = Now::Made {
        before: before.to_owned(),
        after: after.to_owned(),
    };
    let whole = whole_of(&one, &now);
    assert!(whole.applies, "{:?}", whole.why);
    whole
        .hunks
        .iter()
        .map(|h| (h.old_start, h.old_lines, h.new_start, h.new_lines))
        .collect()
}

/// #2121: lines that only turn from CRLF to LF are changed lines on the tab, as git shows them.
#[test]
fn a_change_from_crlf_to_lf_marks_each_line_it_turns() {
    assert_eq!(tab_hunks("a\r\nb\r\nc\n", "a\nb\nc\n"), [(1, 2, 1, 2)]);
}

/// #2121: a last line that loses its line end is a changed line on the tab, as git shows it.
#[test]
fn a_last_line_that_loses_its_line_end_is_marked() {
    assert_eq!(tab_hunks("a\nb\n", "a\nb"), [(2, 1, 2, 1)]);
}

/// PC-7: a change too long for a row is applied only from its own tab, where it is shown whole;
/// one not shown for anything else (a credential's shape) is refused from anywhere.
#[test]
fn a_change_too_long_for_a_row_is_applied_only_from_its_tab() {
    let (_dir, root) = project();
    let long = "line\n".repeat(purlis_core::harness::asked::edit::MOST_LINES + 1);
    std::fs::write(root.join("purlis.toml"), &long).expect("a long manifest");
    let one = Proposal {
        id: 1,
        session: 3,
        path: "purlis.toml".to_owned(),
        base: base_of(Some(long.as_bytes())),
        change: Change::Whole {
            content: format!("{long}more\n"),
        },
        since: 1,
        stale: false,
    };
    // Too long a whole content to travel: made here, as the app would hold it.
    let ok = |_: &proposed::Applied<'_>| Ok(());
    assert_eq!(
        apply_in(&root, &one, None, &ok, &Mutex::new(()), From::Row),
        Err(Refusal::Not(NOT_SHOWN_SAID.to_owned()))
    );
    assert!(apply_in(&root, &one, None, &ok, &Mutex::new(()), From::Tab).is_ok());
    assert_eq!(manifest(&root), format!("{long}more\n"));

    std::fs::write(root.join("purlis.toml"), "[plane]\n").expect("a manifest");
    let secret = Proposal {
        base: base_of(Some(b"[plane]\n")),
        change: Change::Whole {
            content: format!("token = \"{}\"\n", token()),
        },
        ..one
    };
    assert_eq!(
        apply_in(&root, &secret, None, &ok, &Mutex::new(()), From::Tab),
        Err(Refusal::Not(NOT_SHOWN_SAID.to_owned()))
    );
}

/// #3467: no chat proposes as purlis itself, whose changes are judged by a wider rule.
#[test]
fn no_chat_proposes_as_purlis() {
    let (_dir, root) = project();
    let held = Proposals::default();
    let refused = asked_in(&root, &held, &ask(BY_PURLIS, None, mode_pr()), 1);
    assert!(refused.is_err(), "{refused:?}");
    assert!(held.every().is_empty());
}
