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
        limit: None,
        skills: None,
        behind: None,
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
        limit: None,
        skills: None,
        behind: None,
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
        limit: None,
        skills: None,
        behind: None,
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
        &|_| None,
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
        &|_| None,
    );
    // PC-7: too long for a row, it opens its own tab, where Apply is pressed.
    assert_eq!(ids(&too_long), ["open", "dismiss"]);
    assert!(matches!(
        too_long.writes,
        Some(crate::askwrites::WritesShown::NotShown { .. })
    ));
    for now in [Now::Stale, Now::Unprepared] {
        assert_eq!(
            ids(&crate::asking::proposed_change(&one, &now, &|_| None)),
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
        limit: None,
        skills: None,
        behind: None,
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
        limit: None,
        skills: None,
        behind: None,
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
        limit: None,
        skills: None,
        behind: None,
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
        limit: None,
        skills: None,
        behind: None,
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
        limit: None,
        skills: None,
        behind: None,
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
        limit: None,
        skills: None,
        behind: None,
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
        limit: None,
        skills: None,
        behind: None,
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

/// **A value for a pool's limit** (#4284, the person's ruling of 2026-11-01 on PC-1).
mod limit {
    use super::*;
    use purlis_core::dockerpool::Image;
    use purlis_core::hookwire::ProposeLimitAsk;
    use purlis_core::poollimits::LimitId::SignInWait;
    use purlis_core::runners;

    const IMAGE: &str = "ghcr.io/purlis/purlis-runner@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn a_machine() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let pool = runners::Pool::new(
            "builds",
            Image::pinned(IMAGE).unwrap(),
            runners::Set::default(),
        )
        .unwrap();
        runners::record_pool(dir.path(), &pool).unwrap();
        dir
    }

    fn asks(value: Option<u64>) -> ProposeLimitAsk {
        ProposeLimitAsk {
            chat: 3,
            pool: "builds".to_owned(),
            key: "signInWait".to_owned(),
            value,
        }
    }

    fn set(config: &Path) -> Option<u64> {
        runners::chosen_limits_of(config, "builds")
            .unwrap()
            .get(SignInWait)
    }

    #[test]
    fn it_is_held_shown_old_to_new_writes_nothing_and_apply_saves_only_that_value() {
        let project = tempfile::tempdir().unwrap();
        let config = a_machine();
        let proposals = Proposals::default();
        let file =
            purlis_core::machine::dir(config.path()).join(purlis_core::machine::RUNNERS_FILE);
        let before = std::fs::read_to_string(&file).unwrap();

        limit_asked_in(
            project.path(),
            config.path(),
            &proposals,
            &asks(Some(30)),
            1,
        )
        .unwrap();
        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            before,
            "nothing yet"
        );

        let one = proposals.every().remove(0);
        let change = one.limit.clone().unwrap();
        let Now::Made { before: was, after } = limit_now(&change) else {
            panic!("it is shown");
        };
        assert!(was.contains("15 minutes (the default)"), "{was}");
        assert!(after.contains("30 minutes"), "{after}");
        assert!(one.path.contains("builds"), "{}", one.path);

        let outcomes = Mutex::new(Vec::new());
        let audit = |applied: &proposed::Applied<'_>| {
            outcomes.lock().unwrap().push(applied.outcome);
            Ok(())
        };
        let lock = Mutex::new(());
        apply_limit(&one, &change, None, &audit, &lock).unwrap();
        assert_eq!(set(config.path()), Some(30));
        assert_eq!(
            *outcomes.lock().unwrap(),
            vec![proposed::Outcome::Intent, proposed::Outcome::Applied],
            "audited before it is saved"
        );
    }

    #[test]
    fn a_value_the_check_refuses_or_no_such_pool_is_never_held() {
        let project = tempfile::tempdir().unwrap();
        let config = a_machine();
        let proposals = Proposals::default();
        let mut refused = vec![asks(Some(0)), asks(Some(121))];
        refused.push(ProposeLimitAsk {
            pool: "none".to_owned(),
            ..asks(Some(30))
        });
        refused.push(ProposeLimitAsk {
            key: "image".to_owned(),
            ..asks(Some(30))
        });
        refused.push(ProposeLimitAsk {
            key: "vouchWait".to_owned(),
            ..asks(Some(10))
        });
        refused.push(ProposeLimitAsk {
            chat: BY_PURLIS,
            ..asks(Some(30))
        });
        for ask in refused {
            assert!(
                limit_asked_in(project.path(), config.path(), &proposals, &ask, 1).is_err(),
                "{ask:?}"
            );
        }
        assert!(proposals.every().is_empty());
    }

    /// #4381: a chat can reset a value the file can't use, and is shown as replacing it.
    #[test]
    fn a_reset_of_a_value_the_file_cannot_use_is_held_and_says_what_it_replaces() {
        let project = tempfile::tempdir().unwrap();
        let config = a_machine();
        let file =
            purlis_core::machine::dir(config.path()).join(purlis_core::machine::RUNNERS_FILE);
        let edited = std::fs::read_to_string(&file).unwrap().replacen(
            "\"name\"",
            "\"signInWait\":\"lots\",\"name\"",
            1,
        );
        std::fs::write(&file, edited).unwrap();
        let proposals = Proposals::default();
        limit_asked_in(project.path(), config.path(), &proposals, &asks(None), 1).unwrap();
        let change = proposals.every().remove(0).limit.unwrap();
        let Now::Made { before, .. } = limit_now(&change) else {
            panic!("it is shown");
        };
        assert!(before.contains("can't be used"), "{before}");
        assert!(!before.contains("lots"), "{before}");
    }

    /// A chat can't learn the person's value by guessing it: the same value is held like another.
    #[test]
    fn a_value_already_set_is_held_and_says_nothing_to_the_chat() {
        let project = tempfile::tempdir().unwrap();
        let config = a_machine();
        let proposals = Proposals::default();
        runners::set_pool_limit(config.path(), "builds", SignInWait, Some(30)).unwrap();
        limit_asked_in(
            project.path(),
            config.path(),
            &proposals,
            &asks(Some(30)),
            1,
        )
        .unwrap();
        assert_eq!(proposals.every().len(), 1);
    }

    /// PC-8: another value since makes it stale, and Apply saves nothing.
    #[test]
    fn a_limit_changed_since_makes_it_stale() {
        let project = tempfile::tempdir().unwrap();
        let config = a_machine();
        let proposals = Proposals::default();
        limit_asked_in(
            project.path(),
            config.path(),
            &proposals,
            &asks(Some(30)),
            1,
        )
        .unwrap();
        runners::set_pool_limit(config.path(), "builds", SignInWait, Some(45)).unwrap();
        let one = proposals.every().remove(0);
        let change = one.limit.clone().unwrap();
        assert_eq!(limit_now(&change), Now::Stale);
        let audit = |_: &proposed::Applied<'_>| Ok(());
        let lock = Mutex::new(());
        assert_eq!(
            apply_limit(&one, &change, None, &audit, &lock),
            Err(Refusal::Stale)
        );
        assert_eq!(set(config.path()), Some(45));
    }

    /// PC-14: where proposed changes are off, none of these is held.
    #[test]
    fn policy_off_holds_no_limit_either() {
        let project = tempfile::tempdir().unwrap();
        let config = a_machine();
        let proposals = Proposals::default();
        purlis_core::sandbox::policy::set_for_this_test(
            purlis_core::sandbox::policy::Locks::parse(
                r#"{"sandbox": {"proposed-changes": "off"}}"#,
                Path::new("/etc/purlis/policy.json"),
            ),
        );
        let held = limit_asked_in(
            project.path(),
            config.path(),
            &proposals,
            &asks(Some(30)),
            1,
        );
        purlis_core::sandbox::policy::set_for_this_test(purlis_core::sandbox::policy::Locks::none());
        assert!(held.is_err());
        assert!(proposals.every().is_empty());
    }
}

/// What the person's fill of `key` came to, for persona `devops`, in vault `vault`.
fn filled(
    key: &str,
    vault: &str,
    declared: bool,
    names_no_vault: bool,
) -> purlis_core::secrets::ask::Answered {
    purlis_core::secrets::ask::Answered {
        vault: vault.to_owned(),
        key: key.to_owned(),
        filled: true,
        persona: "devops".to_owned(),
        declared,
        names_no_vault,
        made_vault: names_no_vault,
        asker: Some(7),
        nudge: Vec::new(),
        told: purlis_core::askoutcome::Outcome {
            about: purlis_core::askoutcome::About::Secret {
                vault: vault.to_owned(),
                key: key.to_owned(),
            },
            answer: purlis_core::askoutcome::Answer::Filled,
        },
    }
}

fn definition(root: &Path) -> String {
    std::fs::read_to_string(root.join("personas/devops/persona.md")).expect("the definition")
}

const NAMES_A_VAULT: &str = "---\nrole: Ops\nvault: team\n---\n\nThe charter.\n";
const NAMES_NO_VAULT: &str = "---\nrole: Ops\n---\n\nThe charter.\n";

/// What a Fill's Remember came to, with an audit that takes everything.
fn remember(
    root: &Path,
    held: &Proposals,
    answered: &purlis_core::secrets::ask::Answered,
    purpose: Option<&str>,
) -> Result<Remembered, String> {
    fill_remembered_in(root, held, answered, purpose, &|_| Ok(()), 1)
}

/// #5240, spec #5235: Remember unticked, a Fill writes nothing and holds nothing, for any key: a
/// first fill, and one into a vault the persona does not name.
#[test]
fn a_fill_with_remember_unticked_writes_and_offers_nothing() {
    let (_dir, root) = project();
    std::fs::write(root.join("personas/devops/persona.md"), NAMES_NO_VAULT).unwrap();
    let held = Proposals::default();

    let said = remember(
        &root,
        &held,
        &filled("STRIPE_KEY", "team", false, true),
        None,
    );

    assert_eq!(said, Ok(Remembered::Nothing));
    assert_eq!(definition(&root), NAMES_NO_VAULT);
    assert!(held.every().is_empty(), "no proposal after a Fill");
}

/// #5240, spec #5235: ticked, Fill writes the line the person read, in their words, there and
/// then: no proposal waits, and the chat's own words are not what goes in.
#[test]
fn a_fill_with_remember_ticked_writes_the_line_as_edited_and_holds_no_proposal() {
    let (_dir, root) = project();
    std::fs::write(root.join("personas/devops/persona.md"), NAMES_A_VAULT).unwrap();
    let held = Proposals::default();

    let said = remember(
        &root,
        &held,
        &filled("STRIPE_KEY", "team", false, false),
        Some("  charge test cards: checkout  "),
    );

    assert_eq!(said, Ok(Remembered::Written));
    assert_eq!(
        definition(&root),
        "---\nrole: Ops\nvault: team\nsecret-need.STRIPE_KEY: charge test cards: checkout\n---\n\nThe charter.\n"
    );
    assert!(held.every().is_empty(), "no proposal after a Fill");
}

/// #5240, spec #5235: a persona that names no vault gets a `vault:` line with its need.
#[test]
fn remember_adds_a_vault_line_where_the_persona_names_none() {
    let (_dir, root) = project();
    std::fs::write(root.join("personas/devops/persona.md"), NAMES_NO_VAULT).unwrap();

    remember(
        &root,
        &Proposals::default(),
        &filled("STRIPE_KEY", "team", false, true),
        Some("charge test cards"),
    )
    .expect("written");

    assert_eq!(
        definition(&root),
        "---\nrole: Ops\nsecret-need.STRIPE_KEY: charge test cards\nvault: team\n---\n\nThe charter.\n"
    );
}

/// A declined ask stored nothing, so nothing is remembered of it, ticked or not.
#[test]
fn a_decline_remembers_nothing_even_ticked() {
    let (_dir, root) = project();
    std::fs::write(root.join("personas/devops/persona.md"), NAMES_A_VAULT).unwrap();
    let declined = purlis_core::secrets::ask::Answered {
        filled: false,
        ..filled("STRIPE_KEY", "team", false, false)
    };

    let said = remember(&root, &Proposals::default(), &declined, Some("words"));

    assert_eq!(said, Ok(Remembered::Nothing));
    assert_eq!(definition(&root), NAMES_A_VAULT);
}

/// #5240: where the file can't be written, what the person confirmed is not lost: purlis holds
/// it, naming the chat and the ask behind it, in place of a row that went with the Fill.
#[cfg(unix)]
#[test]
fn a_remembered_line_that_cannot_be_written_is_held_naming_the_chat_and_ask_behind_it() {
    use std::os::unix::fs::PermissionsExt;
    let (_dir, root) = project();
    let folder = root.join("personas/devops");
    std::fs::write(folder.join("persona.md"), NAMES_A_VAULT).unwrap();
    std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o555)).unwrap();
    let held = Proposals::default();

    let said = remember(
        &root,
        &held,
        &filled("STRIPE_KEY", "team", false, false),
        Some("charge test cards"),
    );
    std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o755)).unwrap();

    let every = held.every();
    assert_eq!(every.len(), 1, "{every:?}");
    assert_eq!(said, Ok(Remembered::Held));
    assert_eq!(definition(&root), NAMES_A_VAULT, "not written");
    assert_eq!(every[0].session, BY_PURLIS);
    assert_eq!(
        every[0].behind,
        Some(Behind {
            chat: Some(7),
            key: "STRIPE_KEY".to_owned(),
        })
    );
    let Change::Whole { content } = &every[0].change else {
        panic!("a whole change: {:?}", every[0].change);
    };
    assert!(content.contains("secret-need.STRIPE_KEY: charge test cards\n"));
}

/// #5240, #5227's problem 2: a change purlis holds is drawn as purlis's, with the chat and the
/// ask behind it, and never as "chat 0".
#[test]
fn a_change_purlis_holds_names_purlis_and_the_chat_and_ask_behind_it() {
    let one = Proposal {
        id: 3,
        session: BY_PURLIS,
        path: "personas/devops/persona.md".to_owned(),
        base: None,
        change: Change::Whole {
            content: String::new(),
        },
        since: 1,
        stale: false,
        limit: None,
        skills: None,
        behind: Some(Behind {
            chat: Some(7),
            key: "STRIPE_KEY".to_owned(),
        }),
    };
    let now = Now::Made {
        before: NAMES_A_VAULT.to_owned(),
        after: format!("{NAMES_A_VAULT}more\n"),
    };
    let name_of = |chat: u32| (chat == 7).then(|| "billing work".to_owned());

    let shown = crate::asking::proposed_change(&one, &now, &name_of);

    assert_eq!(shown.chain, ["purlis", "billing work"]);
    assert!(shown.says.contains("STRIPE_KEY"), "{}", shown.says);
    assert!(!shown.says.contains("chat 0"), "{}", shown.says);

    // One with nothing behind it, such as removing an old table, is purlis's alone.
    let alone = crate::asking::proposed_change(
        &Proposal {
            behind: None,
            ..one
        },
        &now,
        &name_of,
    );
    assert_eq!(alone.chain, ["purlis"]);
}

/// #5240: Remember is refused before a Fill stores anything, while its row can still say why,
/// for a definition that has nowhere to hold the line, and is fine for one that does.
#[test]
fn a_definition_that_cannot_hold_the_line_is_refused_before_the_fill() {
    let (_dir, root) = project();
    std::fs::write(root.join("personas/devops/persona.md"), NAMES_A_VAULT).unwrap();
    assert_eq!(need_writable(&root, "devops"), Ok(()));

    std::fs::write(root.join("personas/devops/persona.md"), "No frontmatter.\n").unwrap();
    let why = need_writable(&root, "devops").expect_err("no frontmatter");
    assert!(why.contains("untick Remember"), "{why}");

    assert!(need_writable(&root, "nobody").is_err(), "no such persona");
    assert!(
        need_writable(&root, "../devops").is_err(),
        "no persona's name"
    );
}

/// #5240, a steered chat: a definition it swelled or swapped for a link is never read whole, and
/// what decides the tick on an ask reads no file at all, on the window's thread.
#[cfg(unix)]
#[test]
fn a_swollen_or_linked_definition_is_refused_unread_and_the_tick_reads_no_file() {
    let (_dir, root) = project();
    let file = root.join("personas/devops/persona.md");
    std::fs::write(&file, NAMES_A_VAULT).unwrap();
    assert!(need_offerable(&root, "devops"));

    // Past the limit, and otherwise a definition that would take the line.
    let padding = "x".repeat(purlis_core::proposed::MOST_FILE_BYTES);
    std::fs::write(&file, format!("{NAMES_A_VAULT}{padding}\n")).unwrap();
    assert!(need_writable(&root, "devops").is_err(), "past the limit");
    assert!(need_offerable(&root, "devops"), "its size isn't looked at");

    std::fs::remove_file(&file).unwrap();
    let elsewhere = root.join("elsewhere.md");
    std::fs::write(&elsewhere, NAMES_A_VAULT).unwrap();
    std::os::unix::fs::symlink(&elsewhere, &file).unwrap();
    assert!(need_writable(&root, "devops").is_err(), "a link");
    assert!(!need_offerable(&root, "devops"), "a link");
}

/// #5240, a steered chat: it took its `vault:` line out as it asked, then put it back. The ask
/// remembers no vault was named; the file says otherwise now, and the file wins.
#[test]
fn remember_never_replaces_a_vault_the_persona_names_by_then() {
    let (_dir, root) = project();
    std::fs::write(root.join("personas/devops/persona.md"), NAMES_A_VAULT).unwrap();

    remember(
        &root,
        &Proposals::default(),
        &filled("STRIPE_KEY", "devops", false, true),
        Some("charge test cards"),
    )
    .expect("written");

    assert_eq!(
        definition(&root),
        "---\nrole: Ops\nvault: team\nsecret-need.STRIPE_KEY: charge test cards\n---\n\nThe charter.\n"
    );
}

/// #5240: under policy that turns proposed changes off, Remember is not offered.
#[test]
fn policy_that_turns_proposed_changes_off_turns_remember_off() {
    let (_dir, root) = project();
    std::fs::write(root.join("personas/devops/persona.md"), NAMES_A_VAULT).unwrap();
    purlis_core::sandbox::policy::set_for_this_test(purlis_core::sandbox::policy::Locks::parse(
        r#"{"sandbox": {"proposed-changes": "off"}}"#,
        Path::new("/etc/purlis/policy.json"),
    ));
    assert!(need_writable(&root, "devops").is_err());
    assert!(!need_offerable(&root, "devops"));
    assert!(need_personas_in(&root, "team").is_empty());
}

/// A project with personas `devops`, naming vault `team`, `shop`, naming none, `quiet`, which
/// holds no credentials, and `other`, on another vault.
fn three_personas() -> (tempfile::TempDir, PathBuf) {
    let (dir, root) = project();
    for (name, vault) in [
        ("devops", "vault: team\nsecret-need.DECLARED: words\n"),
        ("shop", ""),
        ("quiet", "vault: none\n"),
        ("other", "vault: elsewhere\n"),
    ] {
        std::fs::create_dir_all(root.join("personas").join(name)).unwrap();
        std::fs::write(
            root.join("personas").join(name).join("persona.md"),
            format!("---\nrole: Ops\n{vault}---\n\nThe charter.\n"),
        )
        .unwrap();
    }
    git(&root, &["init", "-q", "-b", "main", "."]);
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-q", "-m", "a project"]);
    (dir, root)
}

/// #5240: the vault row offers the personas that use this vault, and those that name none, who
/// would come to name it. Another's vault, or `vault: none`, is not one.
#[test]
fn a_vault_row_offers_its_personas_and_those_that_name_no_vault() {
    let (_dir, root) = three_personas();

    let listed = need_personas_in(&root, "team");

    assert_eq!(
        listed,
        [
            NeedPersona {
                persona: "devops".to_owned(),
                file: "personas/devops/persona.md".to_owned(),
                declares: vec!["DECLARED".to_owned()],
                names_no_vault: false,
            },
            NeedPersona {
                persona: "shop".to_owned(),
                file: "personas/shop/persona.md".to_owned(),
                declares: Vec::new(),
                names_no_vault: true,
            },
        ]
    );
}

/// #5240: the vault row's action writes the line the person read, directly, with a `vault:`
/// line where the persona names none. Nothing waits in the Inbox.
#[test]
fn the_vault_rows_action_writes_the_line_directly_and_holds_no_proposal() {
    let (_dir, root) = three_personas();
    let held = Proposals::default();
    let audit = |_: &proposed::Applied<'_>| Ok(());

    need_remembered_in(
        &root,
        &held,
        "team",
        "shop",
        "STRIPE_KEY",
        "take payments",
        &audit,
    )
    .expect("written");

    assert_eq!(
        std::fs::read_to_string(root.join("personas/shop/persona.md")).unwrap(),
        "---\nrole: Ops\nsecret-need.STRIPE_KEY: take payments\nvault: team\n---\n\nThe charter.\n"
    );
    assert!(held.every().is_empty());
}

/// #5240: no persona takes a need by being named wrongly, and a line with no purpose is not
/// what the person read.
#[test]
fn the_vault_rows_action_refuses_what_was_never_offered() {
    let (_dir, root) = three_personas();
    let held = Proposals::default();
    let audit = |_: &proposed::Applied<'_>| Ok(());
    let before = std::fs::read_to_string(root.join("personas/other/persona.md")).unwrap();
    let said = |persona: &str, key: &str, purpose: &str| {
        need_remembered_in(&root, &held, "team", persona, key, purpose, &audit)
    };

    assert!(
        said("other", "STRIPE_KEY", "words").is_err(),
        "another vault's persona"
    );
    assert!(
        said("../devops", "STRIPE_KEY", "words").is_err(),
        "no persona's name"
    );
    assert!(said("devops", "9 bad", "words").is_err(), "no key");
    assert!(said("devops", "STRIPE_KEY", "   ").is_err(), "no purpose");

    assert_eq!(
        std::fs::read_to_string(root.join("personas/other/persona.md")).unwrap(),
        before
    );
    assert!(!definition(&root).contains("STRIPE_KEY"));
}

/// #5240: a file left by an earlier version does no harm: nothing reads it, and the need it
/// once quieted is remembered like any other.
#[test]
fn a_leftover_dismissal_file_does_no_harm() {
    let (_dir, root) = project();
    std::fs::write(root.join("personas/devops/persona.md"), NAMES_A_VAULT).unwrap();
    let state = purlis_core::plane::state_dir(&root).join("app");
    std::fs::create_dir_all(&state).unwrap();
    let leftover = state.join("secret-needs-dismissed.json");
    let kept = "{\"dismissed\": [{\"persona\": \"devops\", \"key\": \"STRIPE_KEY\"}]}\n";
    std::fs::write(&leftover, kept).unwrap();

    remember(
        &root,
        &Proposals::default(),
        &filled("STRIPE_KEY", "team", false, false),
        Some("charge test cards"),
    )
    .expect("written");

    assert!(definition(&root).contains("secret-need.STRIPE_KEY"));
    assert_eq!(std::fs::read_to_string(&leftover).unwrap(), kept);
}

/// **A change to several skills a chat proposed** (#5357, the operator's ruling of 2026-11-05):
/// held and shown from the plane, nothing changed until the person's Apply, and then only what
/// was shown.
mod skills {
    use super::*;
    use purlis_core::skills::verbs::Level;
    use purlis_core::skills::verbs::set::Member;

    fn a_plane() -> (tempfile::TempDir, PathBuf) {
        let (dir, root) = project();
        for name in ["grilling", "tdd", "review"] {
            let folder = root.join("skills").join(name);
            std::fs::create_dir_all(&folder).unwrap();
            std::fs::write(
                folder.join("SKILL.md"),
                format!("---\nname: {name}\ndescription: Does {name}.\n---\n\nbody\n"),
            )
            .unwrap();
        }
        (dir, root)
    }

    fn there(root: &Path, name: &str) -> bool {
        root.join("skills").join(name).join("SKILL.md").exists()
    }

    fn deletes(names: &[&str]) -> ProposeSkillSetAsk {
        ProposeSkillSetAsk {
            chat: 3,
            change: SetChange::Delete {
                members: names
                    .iter()
                    .map(|name| Member {
                        level: Level::project(),
                        name: (*name).to_owned(),
                    })
                    .collect(),
            },
        }
    }

    /// Records each outcome an audit is handed, in `outcomes`.
    fn record(
        outcomes: &Mutex<Vec<proposed::Outcome>>,
        applied: &proposed::Applied<'_>,
    ) -> Result<(), String> {
        outcomes.lock().unwrap().push(applied.outcome);
        Ok(())
    }

    #[test]
    fn a_chats_bulk_delete_is_held_for_the_person_and_deletes_nothing_until_apply() {
        let (_dir, root) = a_plane();
        let proposals = Proposals::default();
        skills_asked_in(&root, &proposals, &deletes(&["grilling", "tdd"]), 1).unwrap();
        assert!(
            there(&root, "grilling") && there(&root, "tdd"),
            "held, not made"
        );

        let one = proposals.every().remove(0);
        assert_eq!(one.path, "Delete 2 skills");
        let Now::Made { before, after } = now_of(&root, &one) else {
            panic!("it is shown");
        };
        assert!(before.contains("grilling · project: there"), "{before}");
        assert!(after.contains("grilling · project: deleted"), "{after}");
        assert!(after.contains("tdd · project: deleted"), "{after}");
        assert!(there(&root, "grilling"), "shown, still not made");

        let outcomes = Mutex::new(Vec::new());
        let audit = |applied: &proposed::Applied<'_>| record(&outcomes, applied);
        let lock = Mutex::new(());
        apply_in(&root, &one, None, &audit, &lock, From::Row).unwrap();
        assert!(!there(&root, "grilling") && !there(&root, "tdd"));
        assert!(there(&root, "review"), "only what was shown");
        assert_eq!(
            *outcomes.lock().unwrap(),
            vec![proposed::Outcome::Intent, proposed::Outcome::Applied],
            "audited before it is made"
        );
    }

    #[test]
    fn a_skill_changed_since_it_was_proposed_makes_it_stale_and_apply_changes_nothing() {
        let (_dir, root) = a_plane();
        let proposals = Proposals::default();
        skills_asked_in(&root, &proposals, &deletes(&["grilling", "tdd"]), 1).unwrap();
        let one = proposals.every().remove(0);
        std::fs::write(
            root.join("skills/tdd/SKILL.md"),
            "---\nname: tdd\ndescription: Edited by the person.\n---\n\nnew\n",
        )
        .unwrap();
        assert_eq!(now_of(&root, &one), Now::Stale);
        let outcomes = Mutex::new(Vec::new());
        let audit = |applied: &proposed::Applied<'_>| record(&outcomes, applied);
        let lock = Mutex::new(());
        assert_eq!(
            apply_in(&root, &one, None, &audit, &lock, From::Row),
            Err(Refusal::Stale)
        );
        assert!(there(&root, "grilling") && there(&root, "tdd"));
        assert!(outcomes.lock().unwrap().is_empty(), "nothing was tried");
    }

    #[test]
    fn a_proposal_of_no_skill_is_never_held() {
        let (_dir, root) = a_plane();
        let proposals = Proposals::default();
        assert!(skills_asked_in(&root, &proposals, &deletes(&[]), 1).is_err());
        assert!(proposals.every().is_empty());
    }

    #[test]
    fn policy_off_holds_no_change_to_skills_either() {
        let (_dir, root) = a_plane();
        purlis_core::sandbox::policy::set_for_this_test(
            purlis_core::sandbox::policy::Locks::parse(
                r#"{"sandbox": {"proposed-changes": "off"}}"#,
                Path::new("/etc/purlis/policy.json"),
            ),
        );
        let proposals = Proposals::default();
        let said = skills_asked_in(&root, &proposals, &deletes(&["tdd"]), 1);
        purlis_core::sandbox::policy::set_for_this_test(purlis_core::sandbox::policy::Locks::none());
        assert!(
            said.expect_err("refused")
                .starts_with("Policy turns proposed changes off")
        );
        assert!(proposals.every().is_empty());
    }

    #[test]
    fn no_chat_proposes_a_change_to_skills_as_purlis() {
        let (_dir, root) = a_plane();
        let proposals = Proposals::default();
        let mut ask = deletes(&["tdd"]);
        ask.chat = BY_PURLIS;
        assert!(skills_asked_in(&root, &proposals, &ask, 1).is_err());
    }
}

fn git(root: &Path, args: &[&str]) {
    let mut command = std::process::Command::new("git");
    command
        .arg("-C")
        .arg(root)
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.invalid")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.invalid");
    let out = purlis_core::forklock::output(&mut command).expect("git runs");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}
