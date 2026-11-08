// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// **A sender kept from the queue is pointed to the tools only where they queue** (#3914):
/// the command, in a chat whose harness runs purlis's tools outside its sandbox (Claude
/// Code). The tools themselves, and the command under Codex or opencode, whose wrap holds
/// the tools too, are told to send it again once the app answers, and nothing else.
#[test]
fn a_sender_kept_from_the_queue_is_pointed_to_the_tools_only_where_they_queue() {
    use super::Via;
    use purlis_core::dispatchqueue::{ToolPath, Unqueued};
    let kept = || Unqueued::Unreachable(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
    let tool = "with purlis's dispatch tool for it, which queues it";
    let command_in_claude = super::not_queued_for(&kept(), Via::Command, ToolPath::Queues);
    assert!(command_in_claude.contains(tool), "{command_in_claude}");
    for (via, tools) in [
        (Via::Tool, ToolPath::Queues),
        (Via::Command, ToolPath::KeptOut),
        (Via::Tool, ToolPath::KeptOut),
    ] {
        let said = super::not_queued_for(&kept(), via, tools);
        assert!(!said.contains("tool"), "{said}");
        assert!(
            said.contains("Send it again once the app answers."),
            "{said}"
        );
        assert!(said.contains("Nothing was sent."), "{said}");
    }
}

/// **A `--long`-first line reaches no subcommand** (#3905): `Bash(purlis dispatch --long *)`
/// is pre-allowed for Claude Code, and it allows the task and nothing else only because the
/// command's arguments and its subcommands exclude each other
/// (`args_conflicts_with_subcommands`). So a subcommand after `--long` is no command at all,
/// and `--long` is a task's flag, never a subcommand's.
#[test]
fn a_long_first_line_reaches_no_subcommand() {
    use clap::Parser as _;
    for words in [
        vec!["purlis", "dispatch", "--long", "cancel", "9"],
        vec!["purlis", "dispatch", "--long", "close", "9"],
        vec![
            "purlis",
            "dispatch",
            "--long",
            "report",
            "--outcome",
            "done",
            "x",
        ],
        vec!["purlis", "dispatch", "--long", "tell", "9", "x"],
        vec!["purlis", "dispatch", "cancel", "9", "--long"],
    ] {
        assert!(crate::Cli::try_parse_from(&words).is_err(), "{words:?}");
    }
    for words in [
        vec!["purlis", "dispatch", "--long", "--name", "x"],
        vec!["purlis", "dispatch", "--name", "x", "--long"],
        vec![
            "purlis", "dispatch", "--long", "--to", "devops", "--name", "x",
        ],
    ] {
        assert!(crate::Cli::try_parse_from(&words).is_ok(), "{words:?}");
    }
}

use super::*;

#[test]
fn a_refused_message_is_said_whole_however_long_the_app_s_sentence_is() {
    // What the app answers `tell`, `answer`, `note`, `ask`, `wait` and `cancel` with when it
    // refuses. Each ends with what to do instead, and none is cut.
    let refusals = [
        purlis_core::dispatchtalk::no_question("check the queue", 9),
        purlis_core::dispatched::not_yours(9),
        purlis_core::dispatched::asked_by_the_person(9),
        purlis_core::dispatchtalk::NO_ASKING_CHAT.to_owned(),
        purlis_core::dispatchtalk::ALREADY_REPORTED.to_owned(),
        purlis_core::dispatchtalk::ASKED_BY_THE_PERSON.to_owned(),
        purlis_core::dispatched::being_stopped("check the queue", 9),
    ];
    let longest = refusals
        .iter()
        .map(|why| why.chars().count())
        .max()
        .expect("some");
    assert!(longest > purlis_core::shown::DISPLAY_LIMIT, "{longest}");
    for why in refusals {
        let said = task_refused(&why);
        assert_eq!(said, format!("{SAYS} {why}"));
        assert!(!said.contains('…') && !said.contains('\n'), "{said}");
    }
}

#[test]
fn a_refusal_longer_than_a_name_s_budget_is_said_whole_and_on_one_line() {
    // The longest refusal a dispatch has: what an unattended chat with no standing grant
    // is told. It ends with what to do, which is the part a cut would lose.
    let why = purlis_core::dispatchunattended::Missing {
        asking: Some("steward".to_owned()),
        target: "devops".to_owned(),
        unreviewed: false,
    }
    .say();
    assert!(why.chars().count() > purlis_core::shown::DISPLAY_LIMIT);

    let said = app_refused(&why);

    assert_eq!(said, format!("{SAYS} {why} {NOTHING}"));
    assert!(!said.contains('…'), "{said}");
    // Still one line, whatever the app's sentence held.
    assert!(!app_refused("one\ntwo").contains('\n'));
    // And a refusal that already says nothing was started is not told so twice.
    let none = purlis_core::personaprofile::Refused::NoProfile.say();
    assert_eq!(app_refused(&none), format!("{SAYS} {none}"));
}

#[test]
fn an_app_that_does_not_answer_in_time_is_never_said_to_have_started_nothing() {
    // #1453 review, fold-in 2: the app may still be cutting the worktree.
    assert_eq!(
        no_answer("check the queue", true),
        "purlis dispatch: the purlis app did not answer in time, so purlis cannot say \
         whether the chat started. Cutting a worktree of a large repo can take longer than \
         this command waits, and the chat may still start. Look for 'check the queue' under \
         this chat in the explorer before you dispatch it again."
    );
    for cuts in [true, false] {
        assert!(
            !no_answer("x", cuts)
                .to_lowercase()
                .contains("nothing was started")
        );
    }
    assert!(A_WORKTREE_TAKES_AT_MOST > crate::handoff::AN_OPEN_TAKES_AT_MOST);
}

#[test]
fn a_persona_that_isolates_by_default_is_given_the_worktree_s_wait() {
    // #3903: its default cut may ask the remote for its default branch, so it is waited
    // on as `--in worktree` is.
    let dir = tempfile::tempdir().expect("a directory");
    let root = dir.path();
    std::fs::create_dir_all(root.join("personas/web")).expect("a persona folder");
    std::fs::write(
        root.join("personas/web/persona.md"),
        "---\nname: web\ndescription: the site\ndispatch-isolation: worktree\n---\n# web\n",
    )
    .expect("its definition");
    std::fs::create_dir_all(root.join("personas/ops")).expect("a persona folder");
    std::fs::write(
        root.join("personas/ops/persona.md"),
        "---\nname: ops\ndescription: the servers\n---\n# ops\n",
    )
    .expect("its definition");

    assert!(may_cut_a_worktree(root, None, Some("web")));
    assert!(!may_cut_a_worktree(root, None, Some("ops")));
    assert!(!may_cut_a_worktree(root, None, None));
    assert!(may_cut_a_worktree(root, Some("worktree"), None));
    assert!(may_cut_a_worktree(root, Some("worktree:api"), Some("ops")));
    // A place that names another workspace is never a worktree, whatever the persona says.
    assert!(!may_cut_a_worktree(
        root,
        Some("workspace:beta"),
        Some("web")
    ));
}

#[test]
fn a_ticket_the_app_will_not_mint_is_said_in_a_dispatch_s_words() {
    // A ticket is one run of a command's own (#1441): a chat that dispatches several tasks
    // in one step mints one for each, up to as many as it may have under way at once. The
    // refusal past that is the app's sentence, said as a dispatch's and whole. It names no
    // other command, so there is nothing in it to reword.
    use purlis_core::hookwire::{MOST_LIVE_TICKETS_A_CHAT, Tickets};
    let tickets = Tickets::default();
    let now = std::time::Instant::now();
    for connection in 0..u64::try_from(MOST_LIVE_TICKETS_A_CHAT).expect("a small number") {
        tickets.mint(3, connection, now).expect("under the cap");
    }
    let why = tickets.mint(3, 99, now).expect_err("the cap");

    let said = app_refused(&why);

    assert_eq!(
        said,
        format!(
            "{SAYS} chat 3 has {MOST_LIVE_TICKETS_A_CHAT} requests to the app under way at \
             once; try again in a few seconds {NOTHING}"
        )
    );
    assert!(!said.contains("handoff"), "{said}");
    // A second ticket on one connection is refused too, and said the same way.
    let again = Tickets::default();
    again.mint(3, 1, now).expect("the first");
    let why = again
        .mint(3, 1, now)
        .expect_err("one live ticket a connection");
    assert_eq!(
        app_refused(&why),
        format!(
            "{SAYS} this connection already holds a ticket for chat 3; spend it first \
             {NOTHING}"
        )
    );
}

/// A chat whose dispatch waits because it moves work is told, before the person answers, that a
/// refusal holds for that place while it runs (#4862).
#[test]
fn a_chat_held_to_move_is_told_a_refusal_holds_for_that_place() {
    let said = held_to_move("pool:builds", "lint");
    assert!(said.contains("It asks to run on pool 'builds'"), "{said}");
    assert!(
        said.contains(
            "its dispatches to run on pool 'builds' are refused from then on without asking them"
        ),
        "{said}"
    );
}

/// **A silent app is told apart from a closed connection and an unreadable reply** (#5772):
/// each used to read "the purlis app did not answer", so a chat could not tell an app that
/// is running but busy from one that is gone. A timeout names how long this command waited.
#[test]
fn an_ask_that_runs_out_of_time_says_the_app_is_busy_not_gone() {
    use std::io;
    let within = std::time::Duration::from_secs(2);
    for kind in [io::ErrorKind::WouldBlock, io::ErrorKind::TimedOut] {
        let failed = failed_of(Err(io::Error::from(kind)), within);
        assert!(matches!(failed, Err(Failed::NoAnswer(Silence::OutOfTime(t))) if t == within));
    }
    let said = no_answer_said(&Silence::OutOfTime(within));
    assert_eq!(
        said,
        "purlis dispatch: the purlis app did not answer within 2 seconds. It is running but \
         busy, so try again in a moment. `purlis dispatch list` shows where this chat's tasks \
         stand."
    );

    let closed = failed_of(
        Err(io::Error::new(io::ErrorKind::UnexpectedEof, "closed")),
        within,
    );
    assert!(matches!(closed, Err(Failed::NoAnswer(Silence::Closed))));
    assert!(no_answer_said(&Silence::Closed).contains("closed the connection"));

    let unreadable = failed_of(Err(io::Error::other("not json")), within);
    assert!(matches!(
        unreadable,
        Err(Failed::NoAnswer(Silence::Unreadable))
    ));
    assert!(no_answer_said(&Silence::Unreadable).contains("could not read"));

    // An answer that is not a task's is not one this command can use either.
    let wrong = failed_of(Ok(Answer::Alive {}), within);
    assert!(matches!(wrong, Err(Failed::NoAnswer(Silence::Unreadable))));
    // And no sentence says the app is gone: `the_app` says that where nothing connected.
    for silence in [
        Silence::OutOfTime(within),
        Silence::Closed,
        Silence::Unreadable,
    ] {
        assert!(!no_answer_said(&silence).contains("no purlis app"));
    }
}
