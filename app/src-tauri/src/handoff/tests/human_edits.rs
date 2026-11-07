// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A human edit is told at the chat's next turn** (#5409; R8, V34a, ADR 0084 §8): a real
//! discard in a real branch folder, read back as the line its chat's hook is handed.
//!
//! The whole chat is the repo's fake harness, whose hook is `purlis hook userpromptsubmit` as a
//! harness runs it: it binds a socket, which a sandboxed agent cannot, so it first runs on CI.

use purlis_core::eventlog::{self, Entry, Event as Logged};
use purlis_core::files::{self, Against, Branch, Discard, FileDiff, Target};
use purlis_core::hookwire::{Conversation, Report};
use purlis_core::state::Detail;
use std::time::{Duration, Instant};

use crate::humanedits::{DiscardAnswer, discard_in};

use super::*;

const FILE: &str = "src/lib.rs";
const GONE_LINE: &str = "the agent's line that is discarded";

fn git(at: &Path, args: &[&str]) {
    let mut git = std::process::Command::new("git");
    git.arg("-C").arg(at).args(args);
    git.args::<[&str; 0], &str>([])
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.com")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.com")
        .env_remove("GIT_DIR");
    let out = purlis_core::forklock::output(&mut git).expect("git runs");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Workspace `alpha`'s repo `svc`, and a branch folder of it whose file the agent has changed.
fn a_folder_with_the_agents_change(root: &Path) -> PathBuf {
    let clone = root.join("workspaces").join("alpha").join("svc");
    std::fs::create_dir_all(clone.join("src")).expect("the repo's folder");
    git(&clone, &["init", "-q", "-b", "main"]);
    std::fs::write(clone.join(FILE), "one\ntwo\nthree\n").expect("the file");
    git(&clone, &["add", "-A"]);
    git(&clone, &["commit", "-q", "-m", "files"]);
    let piece = purlis_core::worktree::add(root, "alpha", "svc", "piece", None)
        .expect("a piece is cut")
        .path;
    std::fs::write(piece.join(FILE), format!("one\n{GONE_LINE}\nthree\n")).expect("the change");
    piece
}

/// The operator discards the file's hunk from the window.
fn the_operator_discards_the_hunk(held: &Held) {
    let at = Branch::piece("alpha", "svc", "piece");
    let shown =
        files::what_changed(&crate::reader(), held.root(), at, FILE).expect("the file's diff");
    let FileDiff::Text { hunks, .. } = shown.diff else {
        panic!("a text file's diff");
    };
    let ask = Discard {
        path: FILE.to_owned(),
        target: Target::Hunk(hunks[0]),
        against: Against::Opened(shown.stamp.expect("a stamp")),
        turn: None,
    };
    let answer = discard_in(held, at, &ask, true).expect("the discard");
    assert!(
        matches!(
            answer,
            DiscardAnswer::Discarded {
                unrecorded: None,
                ..
            }
        ),
        "{answer:?}"
    );
}

/// The line chat `chat`'s hook is handed as its turn begins.
fn told(held: &Held, id: &PlaneId, chat: u32) -> Option<String> {
    match asks(held, id, chat, What::HumanEdits) {
        Answer::Task(answered) => match *answered {
            Answered::HumanEdits { line } => line,
            other => panic!("the line, not {other:?}"),
        },
        other => panic!("the line, not {other:?}"),
    }
}

/// Chat `chat`'s prompt reaches the log, as the hook reports it once it has printed.
fn its_prompt_is_recorded(events: &crate::hooks::Events, root: &Path, chat: u32) {
    events
        .record(Entry::Report {
            plane: root.to_path_buf(),
            report: Report {
                chat,
                event: Event::UserPromptSubmit,
                conversation: Conversation::Unknown,
                pid: None,
                agent: None,
                detail: Detail::default(),
            },
            followed: eventlog::Followed::No,
            begun: None,
        })
        .expect("the report is recorded");
}

/// A chat working in `cwd`, on the pretend host.
fn a_chat_in(held: &Held, cwd: &Path) -> u32 {
    let chat = Chat {
        program: "harness".to_owned(),
        name: "1".to_owned(),
        cwd: Some(cwd.to_path_buf()),
        ..Default::default()
    };
    held.chats().start(&chat, STARTING).expect("it runs")
}

#[test]
fn a_discarded_hunk_is_told_once_at_each_chat_in_the_folders_next_turn_by_its_path_alone() {
    use purlis_core::eventlog::{ArgsKey, Log, Recorder};
    let plane = Plane::new();
    let host = Pretend::default();
    let logs = plane.root.join("events");
    let events = crate::hooks::Events::here(Recorder::new(
        Log::open(&logs, "DEVICE").expect("a log"),
        ArgsKey::open(&logs).expect("a key"),
    ));
    let planes = planes_on(&host).recording_events(Some(events.clone()));
    let id = planes.open(&plane.root);
    let held = planes.held(&id).expect("held");
    let folder = a_folder_with_the_agents_change(&plane.root);
    let first = a_chat_in(&held, &folder);
    let second = a_chat_in(&held, &folder);
    let elsewhere = a_chat_in(&held, &plane.root);
    its_prompt_is_recorded(&events, held.root(), first);
    assert_eq!(told(&held, &id, first), None);

    the_operator_discards_the_hunk(&held);

    let line = told(&held, &id, first).expect("the first chat is told");
    assert_eq!(
        line,
        "The operator edited these files by hand since your last turn. Read them again before \
         you change them: `src/lib.rs` in `svc`."
    );
    assert!(!line.contains(GONE_LINE));
    // Each chat in the folder has a next turn of its own, and no other chat is told.
    assert_eq!(told(&held, &id, second).as_deref(), Some(line.as_str()));
    assert_eq!(told(&held, &id, elsewhere), None);

    its_prompt_is_recorded(&events, held.root(), first);

    assert_eq!(told(&held, &id, first), None);
    assert_eq!(told(&held, &id, second).as_deref(), Some(line.as_str()));
}

/// How many times the log says chat `chat`'s prompt was heard.
fn prompts_of(logs: &Path, chat: &str) -> usize {
    eventlog::read(logs)
        .expect("the log reads")
        .iter()
        .filter(|event: &&Logged| {
            event.kind == "hook.userpromptsubmit" && event.chat.as_deref() == Some(chat)
        })
        .count()
}

fn until(what: &str, done: impl Fn() -> bool) {
    until_or_say(what, done, String::new);
}

/// [`until`], whose failure says what `seen` read at the deadline.
fn until_or_say(what: &str, done: impl Fn() -> bool, seen: impl Fn() -> String) {
    let deadline = Instant::now() + Duration::from_secs(60);
    while !done() {
        assert!(Instant::now() < deadline, "{what} never happened{}", seen());
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// The ticket's whole chat: prompt, discard a hunk, next turn, and the turn after.
#[cfg(unix)]
#[test]
fn a_whole_chat_is_told_of_a_discarded_hunk_at_its_next_turn_and_not_at_the_one_after() {
    let plane = Plane::new();
    let logs = plane.root.join("events");
    let planes = planes_recording_events(&logs);
    let id = planes.open(&plane.root);
    let held = planes.held(&id).expect("held");
    let folder = a_folder_with_the_agents_change(&plane.root);
    let scratch = tempfile::tempdir().expect("a folder for what the harness is told");
    let told = scratch.path().join("told");
    let purlis = crate::behaviour::built_beside("purlis", "purlis-cli");
    let harness = crate::behaviour::built_beside("fake-harness", "fake-harness");
    let hook = format!(
        "printf '%s' '{{\"session_id\":\"s-1\",\"prompt\":\"carry on\"}}' \
         | '{}' hook userpromptsubmit >> '{}'",
        purlis.display(),
        told.display()
    );
    let chat = Chat {
        program: harness.display().to_string(),
        args: [
            "--hook",
            hook.as_str(),
            "--interactive",
            "--again",
            "next turn",
        ]
        .map(str::to_owned)
        .to_vec(),
        name: "fake".to_owned(),
        cwd: Some(folder),
        ..Default::default()
    };
    let session = held
        .chats()
        .start(&chat, STARTING)
        .expect("the chat starts");
    let chat_id = held.chats().id_of(session).expect("the chat's id");
    let line_times = || {
        std::fs::read_to_string(&told)
            .unwrap_or_default()
            .matches("`src/lib.rs` in `svc`")
            .count()
    };
    // The first prompt: its report is what the edit comes after.
    until("the first prompt's report", || {
        prompts_of(&logs, &chat_id) == 1
    });
    assert_eq!(line_times(), 0);

    the_operator_discards_the_hunk(&held);

    let next_turn = || {
        held.chats()
            .sessions()
            .input(session, b"next turn\r")
            .expect("typed");
    };
    next_turn();
    until("the next prompt's report", || {
        prompts_of(&logs, &chat_id) == 2
    });
    assert_eq!(line_times(), 1, "the next turn's briefing names the file");
    assert!(
        !std::fs::read_to_string(&told)
            .unwrap_or_default()
            .contains(GONE_LINE)
    );

    next_turn();
    until("the third prompt's report", || {
        prompts_of(&logs, &chat_id) == 3
    });
    assert_eq!(line_times(), 1, "the turn after is not told again");
}

/// **Undo a turn, from the window** (#5411): refused while the chat in the folder works, naming
/// it and its state, with nothing written; once it is idle, every file goes back as the turn
/// found it, and the chat's next turn is told that a turn was undone, by path alone.
#[test]
fn an_undo_is_refused_by_name_while_the_folders_chat_works_and_once_idle_is_told_as_undone() {
    use crate::humanedits::{UndoAnswer, undo_in};
    use purlis_core::eventlog::{ArgsKey, Log, Recorder};
    let plane = Plane::new();
    let host = Pretend::default();
    let logs = plane.root.join("events");
    let events = crate::hooks::Events::here(Recorder::new(
        Log::open(&logs, "DEVICE").expect("a log"),
        ArgsKey::open(&logs).expect("a key"),
    ));
    let planes = planes_on(&host).recording_events(Some(events.clone()));
    let id = planes.open(&plane.root);
    let held = planes.held(&id).expect("held");
    let folder = a_folder_with_the_agents_change(&plane.root);
    let at = Branch::piece("alpha", "svc", "piece");
    let chat = a_chat_in(&held, &folder);
    let chat_id = held.chats().id_of(chat).expect("the chat's id");
    let before = std::fs::read_to_string(folder.join(FILE)).expect("the file");
    its_prompt_is_recorded(&events, held.root(), chat);
    files::turns::began(held.root(), at, &chat_id).expect("the turn begins");
    std::fs::write(folder.join(FILE), "the turn's\n").expect("the agent's edit");
    files::turns::ended(held.root(), at, &chat_id).expect("the turn ends");
    let turn = files::TurnKey {
        chat: chat_id,
        number: 1,
    };

    // The chat on the board, as the host puts one there, and its own harness's hooks.
    held.hooks().board().opened(chat, None, None);
    let hears = |event: Event| {
        held.hooks().board().reported(&Report {
            chat,
            event,
            conversation: Conversation::Named("6f1c2d3e-4a5b-4c6d-8e7f-901a2b3c4d5e".to_owned()),
            pid: None,
            agent: None,
            detail: Detail::default(),
        });
    };
    hears(Event::UserPromptSubmit);
    let UndoAnswer::Busy { chats } = undo_in(&held, at, &turn).expect("an answer") else {
        panic!("a working chat refuses the undo");
    };
    let states: Vec<&str> = chats.iter().map(|busy| busy.state.as_str()).collect();
    assert_eq!(states, ["is running"]);
    assert!(chats.iter().all(|busy| !busy.name.is_empty()));
    assert_eq!(
        std::fs::read_to_string(folder.join(FILE)).expect("the file"),
        "the turn's\n"
    );
    assert_eq!(told(&held, &id, chat), None);

    hears(Event::Stop);
    let answer = undo_in(&held, at, &turn).expect("the undo");
    assert_eq!(
        answer,
        UndoAnswer::Undone {
            unrecorded: None,
            unmarked: None
        }
    );
    assert_eq!(
        std::fs::read_to_string(folder.join(FILE)).expect("the file"),
        before
    );
    assert_eq!(
        told(&held, &id, chat).as_deref(),
        Some(
            "The operator undid a turn since your last turn, so these files are as they were \
             before it. Read them again before you change them: `src/lib.rs` in `svc`."
        )
    );
}

/// #5427: **Undo a turn of a chat on a pool's runner is that runner's to carry out**, through the
/// link (session protocol 1.78), under its own idle check: what it waits on is said by name as a
/// chat here would be, and once it has put the files back the window keeps the edit, tells the
/// chat at its next turn and marks its copy of the turn undone. A refusal of the runner's is said
/// in its words. The runner's own side is `purlis_core::hookchannel::HostedTurns`.
#[test]
fn an_undo_of_a_pool_runners_turn_is_asked_of_its_runner_and_its_answer_kept_here() {
    use crate::humanedits::{UndoAnswer, undo_in};
    use purlis_core::eventlog::{ArgsKey, Log, Recorder};
    use purlis_core::humanedit::{EditKind, EditedFile, EditedLines, HumanEdit};
    use purlis_session_protocol::session::{self as wire, UndoWaitsOn};
    let plane = Plane::new();
    let host = Pretend::default();
    let logs = plane.root.join("events");
    let events = crate::hooks::Events::here(Recorder::new(
        Log::open(&logs, "DEVICE").expect("a log"),
        ArgsKey::open(&logs).expect("a key"),
    ));
    let planes = planes_on(&host).recording_events(Some(events.clone()));
    let id = planes.open(&plane.root);
    let held = planes.held(&id).expect("held");
    let folder = a_folder_with_the_agents_change(&plane.root);
    let at = Branch::piece("alpha", "svc", "piece");
    let chat = a_chat_in(&held, &folder);
    host.runs_on_pool(chat, "builds");
    host.records_turns_of_chat(chat);
    let chat_id = held.chats().id_of(chat).expect("the chat's id");
    its_prompt_is_recorded(&events, held.root(), chat);
    // The runner's turn, as its fetch keeps it here: under the pool, never as this machine's.
    files::turns::began(held.root(), at, &chat_id).expect("the turn begins");
    std::fs::write(folder.join(FILE), "the runner's\n").expect("its edit");
    files::turns::ended(held.root(), at, &chat_id).expect("the turn ends");
    for side in ["start", "end"] {
        let own = format!("refs/purlis/turns/{chat_id}/1/{side}");
        let copy = format!("refs/purlis/runners/builds/turns/{chat_id}/1/{side}");
        git(&folder, &["update-ref", &copy, &own]);
        git(&folder, &["update-ref", "-d", &own]);
    }
    let on_disk = std::fs::read_to_string(folder.join(FILE)).expect("the file");
    let turn = files::TurnKey {
        chat: chat_id.clone(),
        number: 1,
    };

    // The runner says its chat is running: nothing is written, and it is named as here.
    host.answers_undo_with(Some(wire::Answer::UndoWaits {
        chats: vec![UndoWaitsOn {
            number: chat,
            state: "is running".into(),
        }],
    }));
    let UndoAnswer::Busy { chats } = undo_in(&held, at, &turn).expect("an answer") else {
        panic!("the runner's working chat holds the undo back");
    };
    assert_eq!(chats.len(), 1);
    assert_eq!(chats[0].state, "is running");
    assert!(!chats[0].name.is_empty());
    assert_eq!(host.undone_on_runners(), [("builds".to_owned(), turn.clone())]);
    assert_eq!(
        std::fs::read_to_string(folder.join(FILE)).expect("the file"),
        on_disk,
        "nothing of this machine's folder is written"
    );

    // The runner refuses one, in its words.
    host.answers_undo_with(None);
    let refused = undo_in(&held, at, &turn).unwrap_err();
    assert!(refused.contains("the runner refused it"), "{refused}");

    // The runner put the files back: kept here as the edit, told, and the copy marked undone.
    let edit = HumanEdit {
        kind: EditKind::UndoneTurn,
        repo: "svc".into(),
        piece: Some("piece".into()),
        files: vec![EditedFile {
            path: FILE.into(),
            lines: vec![EditedLines { start: 1, count: 3 }],
        }],
        busy: false,
        chats: Vec::new(),
        chat_ids: Vec::new(),
    };
    host.answers_undo_with(Some(wire::Answer::TurnUndone {
        edit: edit.body(),
        unmarked: None,
    }));
    let answer = undo_in(&held, at, &turn).expect("the undo");
    assert_eq!(
        answer,
        UndoAnswer::Undone {
            unrecorded: None,
            unmarked: None
        }
    );
    let marked = std::process::Command::new("git")
        .arg("-C")
        .arg(&folder)
        .args([
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("refs/purlis/runners/builds/turns/{chat_id}/1/undone"),
        ])
        .output()
        .expect("git runs");
    assert!(marked.status.success(), "the copy is marked undone");
    assert_eq!(
        told(&held, &id, chat).as_deref(),
        Some(
            "The operator undid a turn since your last turn, so these files are as they were \
             before it. Read them again before you change them: `src/lib.rs` in `svc`."
        )
    );
}

/// **The ticket's whole path, through the fake harness** (#5411): its first turn edits the file
/// and its hooks record the turn; the operator discards that turn's hunk, as the turn's start had
/// it; its next turn edits again and is undone, refused while it works and done once it is idle;
/// and the turn after that is told a turn was undone.
#[cfg(unix)]
#[test]
fn a_whole_chat_has_its_turns_hunk_discarded_and_its_next_turn_undone_and_is_told_so() {
    use crate::humanedits::{UndoAnswer, undo_in};
    let plane = Plane::new();
    let logs = plane.root.join("events");
    let planes = planes_recording_events(&logs);
    let id = planes.open(&plane.root);
    let held = planes.held(&id).expect("held");
    let folder = a_folder_with_the_agents_change(&plane.root);
    let at = Branch::piece("alpha", "svc", "piece");
    let before = std::fs::read_to_string(folder.join(FILE)).expect("the file");
    let scratch = tempfile::tempdir().expect("a folder for what the harness is told");
    let told = scratch.path().join("told");
    // What the hooks printed to their stderr, for a failure to show.
    let said = scratch.path().join("said");
    let purlis = crate::behaviour::built_beside("purlis", "purlis-cli");
    let harness = crate::behaviour::built_beside("fake-harness", "fake-harness");
    let hook = format!(
        "printf '%s' '{{\"session_id\":\"s-1\",\"prompt\":\"carry on\"}}' \
         | '{}' hook userpromptsubmit >> '{}' 2>> '{said}'",
        purlis.display(),
        told.display(),
        said = said.display()
    );
    // Each turn's edit waits for its start to be recorded, as a harness's waits for its model,
    // and adds the turn's own line.
    let edit = format!(
        "cd '{dir}' && until [ \"$(git for-each-ref refs/purlis/turns/ | grep -c /start)\" -gt \
         \"$(git for-each-ref refs/purlis/turns/ | grep -c /end)\" ]; do sleep 0.02; done; \
         echo \"turn $(git for-each-ref refs/purlis/turns/ | grep -c /start)\" >> '{FILE}'",
        dir = folder.display()
    );
    // Its `Stop` names the conversation its prompt did: the board adopted `s-1` from the first
    // report, and refuses a later one that names none as another harness's (ADR 0024), which
    // left the chat running for ever.
    let stop = format!(
        "printf '%s' '{{\"session_id\":\"s-1\"}}' | '{}' hook stop 2>> '{said}'",
        purlis.display(),
        said = said.display()
    );
    let chat = Chat {
        program: harness.display().to_string(),
        args: [
            "--hook",
            hook.as_str(),
            "--hook",
            edit.as_str(),
            "--hook",
            stop.as_str(),
            "--interactive",
            "--again",
            "next turn",
        ]
        .map(str::to_owned)
        .to_vec(),
        name: "fake".to_owned(),
        cwd: Some(folder.clone()),
        ..Default::default()
    };
    let session = held
        .chats()
        .start(&chat, STARTING)
        .expect("the chat starts");
    let chat_id = held.chats().id_of(session).expect("the chat's id");
    let refs = || {
        let refs = purlis_core::forklock::output(
            std::process::Command::new("git")
                .arg("-C")
                .arg(&folder)
                .args(["for-each-ref", "--format=%(refname)", "refs/purlis/turns/"]),
        )
        .expect("git runs");
        String::from_utf8_lossy(&refs.stdout).into_owned()
    };
    // Its end is written, the board has it idle, and the recorder has let go of the chat, which
    // an undo is refused until it does (#5411).
    let ended = |number: u32| {
        refs().contains(&format!("{chat_id}/{number}/end"))
            && !crate::staleclones::mid_turn(&held.board().glance(session))
            && !held.turns().recording(&chat_id)
    };
    // What a wait that ran out saw: the hooks heard, where the chat's turns are recorded, the
    // refs, the board, and what the hooks said.
    let seen = || {
        let heard: Vec<String> = eventlog::read(&logs)
            .unwrap_or_default()
            .into_iter()
            .filter(|event| event.chat.as_deref() == Some(chat_id.as_str()))
            .map(|event| event.kind)
            .collect();
        let at = held.chats().chat_at_or_starting(session);
        let place = at
            .as_ref()
            .and_then(|at| at.cwd.as_deref())
            .and_then(|cwd| files::turns::place_of(held.root(), cwd));
        format!(
            "\n  events of the chat: {heard:?}\n  where its turns go: {at:?} -> {place:?}\n  \
             recording now: {}\n  refs in the clone: {:?}\n  board: {:?}\n  program pid: \
             {:?}\n  the file: {:?}\n  told: {:?}\n  the hooks said: {:?}",
            held.turns().recording(&chat_id),
            refs(),
            held.board().glance(session),
            held.chats().sessions().process_id(session),
            std::fs::read_to_string(folder.join(FILE)).unwrap_or_default(),
            std::fs::read_to_string(&told).unwrap_or_default(),
            std::fs::read_to_string(&said).unwrap_or_default(),
        )
    };
    let next_turn = || {
        held.chats()
            .sessions()
            .input(session, b"next turn\r")
            .expect("typed");
    };
    let turn = |number: u32| files::TurnKey {
        chat: chat_id.clone(),
        number,
    };

    until_or_say("the first turn's end", || ended(1), seen);
    let files::TurnChanged::Of {
        file: files::InTurn::Changed(shown),
        ..
    } = files::turn_changed(&crate::reader(), held.root(), at, FILE, Some(&turn(1)))
        .expect("the turn's diff")
    else {
        panic!("the first turn changed the file");
    };
    let FileDiff::Text { hunks, .. } = shown.diff else {
        panic!("a text file's diff");
    };
    let ask = Discard {
        path: FILE.to_owned(),
        target: Target::Hunk(hunks[0]),
        against: Against::Opened(shown.stamp.expect("a stamp")),
        turn: Some(turn(1)),
    };
    discard_in(&held, at, &ask, false).expect("the discard");
    assert_eq!(
        std::fs::read_to_string(folder.join(FILE)).expect("the file"),
        before
    );

    next_turn();
    until_or_say("the second turn's end", || ended(2), seen);
    let second = std::fs::read_to_string(folder.join(FILE)).expect("the file");
    assert_eq!(second, format!("{before}turn 2\n"));
    let answer = undo_in(&held, at, &turn(2)).expect("the undo");
    assert_eq!(
        answer,
        UndoAnswer::Undone {
            unrecorded: None,
            unmarked: None
        }
    );
    assert_eq!(
        std::fs::read_to_string(folder.join(FILE)).expect("the file"),
        before
    );

    next_turn();
    until_or_say(
        "the third turn's briefing",
        || {
            std::fs::read_to_string(&told)
                .unwrap_or_default()
                .contains("The operator undid a turn since your last turn")
        },
        seen,
    );
    let briefed = std::fs::read_to_string(&told).unwrap_or_default();
    assert!(briefed.contains("`src/lib.rs` in `svc`"), "{briefed}");
    assert!(!briefed.contains("turn 2"), "{briefed}");
}
