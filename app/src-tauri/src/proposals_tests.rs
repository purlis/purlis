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
        apply_in(&root, &one, Some("devops"), &refused),
        Err(Refusal::Not("the event log is not open".to_owned()))
    );
    assert_eq!(manifest(&root), "[plane]\n", "nothing written unaudited");

    let audited = std::cell::RefCell::new(Vec::new());
    let audit = |applied: &proposed::Applied<'_>| {
        audited.borrow_mut().push(applied.body());
        Ok(())
    };
    let said = apply_in(&root, &one, Some("devops"), &audit).expect("applied");
    assert!(said.starts_with("Applied to purlis.toml"), "{said}");
    assert_eq!(manifest(&root), "[plane]\nmode = \"pr\"\n");
    let body = audited.borrow_mut().pop().expect("audited");
    assert_eq!(body["persona"], "devops");
    assert_eq!(body["file"], "purlis.toml");
    assert_eq!(
        body["written"],
        base_of(Some(b"[plane]\nmode = \"pr\"\n")).expect("a hash")
    );

    // The file is not what it was proposed against any more: stale, nothing written.
    assert_eq!(apply_in(&root, &one, None, &audit), Err(Refusal::Stale));
    assert_eq!(manifest(&root), "[plane]\nmode = \"pr\"\n");
    // One marked stale is never applied.
    let marked = Proposal {
        stale: true,
        base: base_of(Some(b"[plane]\nmode = \"pr\"\n")),
        ..one
    };
    assert_eq!(apply_in(&root, &marked, None, &audit), Err(Refusal::Stale));
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
        crate::asking::AnswerPath::ProposedChange { id: 7 }
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
    assert_eq!(ids(&too_long), ["dismiss"]);
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
