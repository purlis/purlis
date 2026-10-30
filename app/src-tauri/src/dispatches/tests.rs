// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use purlis_core::dispatchrecord::{Asker, Changed, Mode, Place, Report, Usage, Worker, Worktree};

use super::*;

fn at(time: &str) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339(time)
        .unwrap()
        .with_timezone(&chrono::Utc)
}

fn a_record() -> Record {
    Record {
        v: dispatchrecord::VERSION,
        id: "01K6DISPATCH00000000000000".to_owned(),
        mode: Mode::Handoff,
        asker: Asker {
            chat: ChatRef {
                chat: 3,
                id: Some("asker".to_owned()),
                name: "steward 3".to_owned(),
                persona: Some("steward".to_owned()),
            },
            workspace: Some("alpha".to_owned()),
            by_person: false,
            session_record: None,
        },
        persona: Some("devops".to_owned()),
        worker: Worker {
            chat: ChatRef {
                chat: 7,
                id: Some("worker".to_owned()),
                name: "devops 7".to_owned(),
                persona: Some("devops".to_owned()),
            },
            harness: Some("claude".to_owned()),
            profile: Some("work".to_owned()),
            session_record: None,
        },
        task: Some("check prod".to_owned()),
        place: Place {
            workspace: Some("beta".to_owned()),
            folder: Some("workspaces/beta".to_owned()),
            worktree: None,
        },
        brief: "Is the rollout healthy?".to_owned(),
        report_owed: true,
        started: "2026-10-07T12:00:00+00:00".to_owned(),
        ended: None,
        report: None,
        needed_you: 2,
        messages: 0,
        talk: Vec::new(),
        last_note: None,
        usage: None,
        conversation: None,
        cleared: false,
        ended_by: None,
        ended_way: None,
        kept_open: false,
        did_not_start: false,
        attempts: 0,
        worked: 0,
        limit: None,
        undelivered: None,
        earlier: Vec::new(),
        asker_last_record: None,
    }
}

#[test]
fn a_running_dispatch_s_row_opens_its_chat_and_counts_its_time_so_far() {
    // The chat was started again since: another session, the same chat.
    let open = [OpenChat {
        session: 12,
        id: Some("worker".to_owned()),
    }];

    let row = row(&a_record(), &open, at("2026-10-07T12:04:30Z"));

    assert_eq!(row.outcome, "running");
    assert_eq!(row.duration, "4m 30s");
    assert_eq!(row.open_session, Some(12));
    assert_eq!(row.asker_key, "asker");
    assert_eq!(row.mode, "handoff");
    assert_eq!(row.ended, None);
    assert_eq!(row.persona.as_deref(), Some("devops"));
    assert_eq!(row.task, "check prod");
    assert_eq!(row.asker, "steward 3");
    assert_eq!(row.place, "beta");
    assert_eq!(row.needed_you, 2);
    assert_eq!(row.cost, None);
    assert_eq!(row.tokens, None);
}

#[test]
fn a_finished_dispatch_s_row_says_its_outcome_its_cost_and_its_session_record() {
    let record = Record {
        ended: Some("2026-10-07T14:05:00+00:00".to_owned()),
        report: Some(Report {
            outcome: Outcome::Blocked,
            text: "No access to the cluster.".to_owned(),
            changed: Changed::default(),
        }),
        usage: Some(Usage {
            input_tokens: Some(15_234),
            output_tokens: Some(4_521),
            cost_usd: Some(0.416),
        }),
        worker: Worker {
            session_record: Some("workspaces/beta/sessions/x.md".to_owned()),
            ..a_record().worker
        },
        place: Place {
            worktree: Some(Worktree {
                repo: "svc".to_owned(),
                piece: "fix".to_owned(),
                branch: Some("fix/rollout".to_owned()),
                removed: None,
            }),
            ..a_record().place
        },
        ..a_record()
    };

    // Its chat is closed: nothing open is it, whatever number another chat has.
    let row = row(
        &record,
        &[OpenChat {
            session: 7,
            id: Some("someone else".to_owned()),
        }],
        at("2026-10-09T00:00:00Z"),
    );

    assert_eq!(row.outcome, "blocked");
    assert_eq!(row.duration, "2h 5m");
    assert_eq!(row.cost.as_deref(), Some("$0.42"));
    assert_eq!(row.tokens.as_deref(), Some("15k in, 4k out"));
    assert_eq!(row.report.as_deref(), Some("No access to the cluster."));
    assert_eq!(row.open_session, None);
    assert_eq!(
        row.session_record.as_deref(),
        Some("workspaces/beta/sessions/x.md")
    );
    assert_eq!(row.place, "beta · fix/rollout");
}

/// A number is dealt again in another launch: a row never opens a chat because it has
/// the number the dispatch's chat once had.
#[test]
fn a_row_never_opens_another_chat_that_was_dealt_its_chat_s_number() {
    let namesakes = [
        OpenChat {
            session: 7,
            id: Some("someone else".to_owned()),
        },
        OpenChat {
            session: 7,
            id: None,
        },
    ];

    let row = row(&a_record(), &namesakes, at("2026-10-07T12:04:30Z"));

    assert_eq!(row.open_session, None);
    // Only a record that names its chat by nothing but a number is matched by one.
    let numbered = Record {
        worker: Worker {
            chat: ChatRef {
                id: None,
                ..a_record().worker.chat
            },
            ..a_record().worker
        },
        asker: Asker {
            chat: ChatRef {
                id: None,
                ..a_record().asker.chat
            },
            ..a_record().asker
        },
        ..a_record()
    };
    let row = super::row(&numbered, &namesakes, at("2026-10-07T12:04:30Z"));
    assert_eq!(row.open_session, Some(7));
    assert_eq!(row.asker_key, "#3");
}

/// #1457: a record whose chat the reopen record lists but which was not brought back
/// showed "running", with a duration that grew, until the project was next opened.
#[test]
fn a_dispatch_that_has_not_ended_and_whose_chat_is_not_open_is_not_said_to_be_running() {
    let row = row(&a_record(), &[], at("2026-10-07T12:04:30Z"));

    assert_eq!(row.outcome, NOT_OPEN);
    assert_eq!(row.outcome, "not open");
    assert_eq!(
        row.duration, "",
        "no clock runs on a chat that is not there"
    );
    assert_eq!(row.ended, None);
    assert_eq!(row.open_session, None);
    // One that ended says how, open or not.
    let ended = Record {
        ended: Some("2026-10-07T12:01:00+00:00".to_owned()),
        report: Some(Report {
            outcome: Outcome::Stopped,
            text: "stopped by the operator".to_owned(),
            changed: Changed::default(),
        }),
        ..a_record()
    };
    let row = super::row(&ended, &[], at("2026-10-09T00:00:00Z"));
    assert_eq!(
        (row.outcome.as_str(), row.duration.as_str()),
        ("stopped", "1m 0s")
    );
    assert_eq!(row.changed, None);
}

#[test]
fn a_row_says_what_the_report_says_changed_in_the_chat_s_words() {
    let record = Record {
        ended: Some("2026-10-07T12:01:00+00:00".to_owned()),
        report: Some(Report {
            outcome: Outcome::Cancelled,
            text: "Stopped half way.".to_owned(),
            changed: Changed {
                said: Some("svc: 2 files".to_owned()),
                ..Changed::default()
            },
        }),
        ..a_record()
    };

    let row = row(&record, &[], at("2026-10-09T00:00:00Z"));

    assert_eq!(row.outcome, "cancelled");
    assert_eq!(row.changed.as_deref(), Some("svc: 2 files"));
}

#[test]
fn a_handoff_that_ended_owing_no_report_is_handed_off_and_has_no_cost_to_say() {
    let record = Record {
        report_owed: false,
        ended: Some("2026-10-07T12:00:45+00:00".to_owned()),
        task: None,
        place: Place::default(),
        ..a_record()
    };

    let row = row(&record, &[], at("2026-10-09T00:00:00Z"));

    assert_eq!(row.outcome, "handed off");
    assert_eq!(row.duration, "45s");
    // Named for its chat where the dispatch gave no task name.
    assert_eq!(row.task, "devops 7");
    assert_eq!(row.place, "project root");
    assert_eq!(row.cost, None);
}

#[test]
fn a_cost_is_said_as_the_harness_said_it_and_a_zero_only_when_it_said_zero() {
    assert_eq!(dollars(0.0), "$0.00");
    assert_eq!(dollars(0.001), "under $0.01");
    assert_eq!(dollars(12.345), "$12.35");
    assert_eq!(counted(None, None), None);
    assert_eq!(counted(Some(900), None).as_deref(), Some("900 in"));
}

#[test]
fn a_wait_is_counted_when_it_begins_and_not_at_every_report_made_during_it() {
    let waiting = Waiting::default();

    assert!(waiting.began(7, true));
    assert!(!waiting.began(7, true));
    assert!(!waiting.began(7, false));
    assert!(waiting.began(7, true));
    // A chat closed while waiting starts from nothing.
    waiting.forget(7);
    assert!(waiting.began(7, true));
}

#[test]
fn a_row_lists_the_worktree_its_dispatch_was_given_and_offers_discard_only_while_it_is_kept() {
    use purlis_core::dispatchrecord::Removed;

    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let given = |piece: &str, removed: Option<Removed>| Record {
        ended: Some("2026-10-07T14:05:00+00:00".to_owned()),
        place: Place {
            workspace: Some("alpha".to_owned()),
            folder: Some(format!("workspaces/alpha/.worktrees/svc/{piece}")),
            worktree: Some(Worktree {
                repo: "svc".to_owned(),
                piece: piece.to_owned(),
                branch: Some(piece.to_owned()),
                removed,
            }),
        },
        ..a_record()
    };
    let listed = |record: &Record| {
        worktree_row(root, record).map(|listed| (listed.standing, listed.discard))
    };
    std::fs::create_dir_all(root.join("workspaces/alpha/.worktrees/svc/fix-00000000")).unwrap();

    // Its folder is there and nothing has merged it: listed, with Discard.
    let kept = given("fix-00000000", None);
    assert_eq!(listed(&kept), Some(("kept".to_owned(), true)));
    let row = worktree_row(root, &kept).unwrap();
    assert_eq!(
        (row.repo.as_str(), row.branch.as_deref()),
        ("svc", Some("fix-00000000"))
    );
    // What purlis did to it is the record's word, and neither is discarded twice.
    assert_eq!(
        listed(&given("fix-00000000", Some(Removed::Merged))),
        Some(("merged".to_owned(), false))
    );
    assert_eq!(
        listed(&given("fix-00000000", Some(Removed::MergedBranchKept))),
        Some(("merged-branch-kept".to_owned(), false))
    );
    assert_eq!(
        listed(&given("fix-00000000", Some(Removed::Discarded))),
        Some(("discarded".to_owned(), false))
    );
    // A record that names a branch folder purlis did not cut for it offers no Discard: its
    // name does not end with the end of the record's own id.
    std::fs::create_dir_all(root.join("workspaces/alpha/.worktrees/svc/chat-1")).unwrap();
    assert_eq!(
        listed(&given("chat-1", None)),
        Some(("gone".to_owned(), false))
    );
    // Its folder gone by other hands, and a record whose names climb out of the project.
    assert_eq!(
        listed(&given("other-00000000", None)),
        Some(("gone".to_owned(), false))
    );
    assert_eq!(
        listed(&given("../../../etc", None)),
        Some(("gone".to_owned(), false))
    );
    // A dispatch that worked in a folder that was already there lists none.
    assert_eq!(listed(&a_record()), None);
    // And the row's place names the branch, as it did.
    assert_eq!(
        row_of(&kept).place,
        "alpha · fix-00000000",
        "the workspace, then its own branch"
    );
}

/// `record`'s row with nothing open, at a fixed time.
fn row_of(record: &Record) -> DispatchRow {
    row(record, &[], at("2026-10-09T00:00:00Z"))
}

#[test]
fn a_folder_is_said_from_the_project_and_one_outside_it_whole() {
    let root = Path::new("/p/project");
    assert_eq!(folder(root, Path::new("/p/project")), ".");
    assert_eq!(
        folder(root, Path::new("/p/project/workspaces/beta")),
        "workspaces/beta"
    );
    assert_eq!(folder(root, Path::new("/elsewhere/x")), "/elsewhere/x");
}

fn a_loss(changes: &[&str], seal: &str) -> WorktreeLoss {
    WorktreeLoss {
        task: "fix login".to_owned(),
        repo: "api".to_owned(),
        branch: Some("fix-login-0a1b2c3d".to_owned()),
        on: Some("fix-login-0a1b2c3d".to_owned()),
        changes: changes.iter().map(|line| (*line).to_owned()).collect(),
        ignored: vec!["target/".to_owned()],
        nested: Vec::new(),
        seal: seal.to_owned(),
        unmerged: 0,
        lost: Vec::new(),
        own: Some(Vec::new()),
    }
}

/// #1472: git state only the folder held, gained or let go of since the question, refuses
/// the Discard and says so.
#[test]
fn a_change_in_what_git_keeps_for_the_folder_alone_is_named() {
    let seen = a_loss(&[], "one");
    let now = WorktreeLoss {
        own: Some(vec![
            "refs/bisect/bad, a ref only this folder has".to_owned(),
        ]),
        ..a_loss(&[], "one")
    };
    let said = what_changed(&seen, &now);
    assert!(
        said.contains("what git keeps for it alone has changed"),
        "{said}"
    );
    assert!(said.contains("Nothing was removed"), "{said}");
    // Unread now, where it was read as none: not the same answer either.
    let unread = WorktreeLoss {
        own: None,
        ..a_loss(&[], "one")
    };
    assert!(what_changed(&seen, &unread).contains("what git keeps for it alone"));
}

/// The question a window that knows nothing of `own` hands back still reads, as unread.
#[test]
fn a_loss_handed_back_without_its_own_git_state_reads_as_unread() {
    let mut sent = serde_json::to_value(a_loss(&[], "one")).expect("it serialises");
    sent.as_object_mut().expect("an object").remove("own");
    let read: WorktreeLoss = serde_json::from_value(sent).expect("it reads");
    assert_eq!(read.own, None);
}

/// #1472: a Discard refused because the folder changed names what changed, so the person
/// is not sent round the same question with nothing to go on.
#[test]
fn a_discard_refused_for_a_change_names_the_paths_that_changed() {
    let seen = a_loss(&[" M src/app.rs"], "one");
    let now = a_loss(&[" M src/app.rs", "?? notes.txt", " D old.rs"], "two");
    let said = what_changed(&seen, &now);
    assert!(said.contains("notes.txt"), "{said}");
    assert!(said.contains("old.rs"), "{said}");
    assert!(!said.contains("src/app.rs"), "{said}");
    assert!(said.contains("Nothing was removed"), "{said}");
    assert!(said.contains("Press Discard again"), "{said}");
    // A path gone from the list since is named as well.
    let said = what_changed(&now, &seen);
    assert!(said.contains("notes.txt"), "{said}");
}

#[test]
fn a_long_list_of_changed_paths_is_cut_and_says_how_many_more() {
    let seen = a_loss(&[], "one");
    let lines: Vec<String> = (0..9).map(|n| format!("?? f{n}.txt")).collect();
    let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
    let said = what_changed(&seen, &a_loss(&lines, "two"));
    assert!(said.contains("f0.txt") && said.contains("f2.txt"), "{said}");
    assert!(!said.contains("f3.txt"), "{said}");
    assert!(said.contains("6 more"), "{said}");
}

/// #1472: the same list and another fingerprint is a file written again while the person
/// looked. Where that keeps happening something is still writing there: say so, and how
/// to stop it, rather than "changed" with no way out.
#[test]
fn a_discard_whose_list_is_the_same_says_something_is_still_writing_and_how_to_stop_it() {
    let seen = a_loss(&[" M src/app.rs"], "one");
    let said = what_changed(&seen, &a_loss(&[" M src/app.rs"], "two"));
    assert!(said.contains("still writing"), "{said}");
    assert!(said.contains("Stop it"), "{said}");
    assert!(said.contains("Nothing was removed"), "{said}");
}

#[test]
fn a_commit_made_since_is_named_as_a_commit() {
    let seen = a_loss(&[], "one");
    let now = WorktreeLoss {
        unmerged: 1,
        ..a_loss(&[], "two")
    };
    let said = what_changed(&seen, &now);
    assert!(said.contains("commit"), "{said}");
    let switched = WorktreeLoss {
        on: Some("main".to_owned()),
        ..a_loss(&[], "one")
    };
    assert!(what_changed(&seen, &switched).contains("main"));
}

/// An entry that vanished while purlis read the folder is a write in progress, not a
/// folder purlis cannot read.
#[test]
fn an_entry_that_vanished_mid_read_says_something_is_still_writing() {
    let said = not_sealed(NotSealed::Vanished);
    assert!(said.contains("still writing"), "{said}");
    assert_eq!(not_sealed(NotSealed::Unread), unread());
    assert_eq!(not_sealed(NotSealed::TooMany), TOO_MANY);
}

#[test]
fn a_secret_in_a_row_s_brief_report_or_changed_words_is_masked() {
    // #1872: a token built here from its parts, so no line of this file holds one.
    let token = format!("{}{}", "ghp_", "0123456789abcdefghijABCDEFGHIJ012345");
    let record = Record {
        brief: format!("Deploy with {token} today."),
        report: Some(Report {
            outcome: Outcome::Done,
            text: format!("Signed in with {token}."),
            changed: Changed {
                said: Some(format!("Rotated {token}.")),
                ..Changed::default()
            },
        }),
        ..a_record()
    };

    let row = row(&record, &[], at("2026-10-07T12:04:30Z"));

    assert_eq!(row.brief, "Deploy with *** today.");
    assert_eq!(row.report.as_deref(), Some("Signed in with ***."));
    assert_eq!(row.changed.as_deref(), Some("Rotated ***."));
}
