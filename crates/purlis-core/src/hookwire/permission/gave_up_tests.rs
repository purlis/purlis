// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Each hook give-up raises one `Terminal` ask** (R-204, T1-35, B2): a permission hook the host
//! will not hold (its ask too big, not its harness's, too late, or the chat at the 32-open cap)
//! leaves the harness's own prompt in the pane, and the board says the chat waits in its terminal,
//! which the app lists as one `Terminal` ask (`asking::terminal`). The pane's answer, which the
//! chat's tool lines say, withdraws it. Nothing here reads what the terminal shows (ADR 0076).

use super::*;
use crate::harness::Harness;
use crate::harness::asks::Asks;
use crate::harness::hooked::{Asker, HookAsks, MOST_ASK_BYTES, MOST_OPEN_ASKS, NotRaised};
use crate::harness::model::Prompt;
use crate::hookwire::{Conversation, Report, Speaker};
use crate::state::{Board, Detail, Event};
use std::sync::{Arc, Mutex};

/// The conversation each chat is in.
const A: &str = "11111111-2222-4333-8444-555555555555";

/// The conversation of a harness nested in the chat, which holds its token.
const B: &str = "99999999-8888-4777-8666-555555555555";

/// The chat's number.
const CHAT: u32 = 7;

/// A harness whose permission hook the host holds asks for: its source, as the hook names it,
/// how the app starts it, and the pid it reports, where it reports one.
#[derive(Clone, Copy)]
struct On {
    source: Source,
    harness: Harness,
    pid: Option<u32>,
}

/// Every harness with a permission hook this measures: Claude Code names its process, Codex none.
const HARNESSES: [On; 2] = [
    On {
        source: Source::ClaudeCode,
        harness: Harness::ClaudeCode,
        pid: Some(4242),
    },
    On {
        source: Source::Codex,
        harness: Harness::Codex,
        pid: None,
    },
];

/// The four ways a hook gives up, by [`NotRaised`]'s name in the ticket.
const CASES: [&str; 4] = ["TooBig", "NotItsHarness", "TooLate", "the 32-open cap"];

fn report(on: On, event: Event) -> Report {
    Report {
        agent: None,
        chat: CHAT,
        event,
        conversation: Conversation::Named(A.to_owned()),
        pid: on.pid,
        detail: Detail::default(),
    }
}

fn speaker(on: On) -> Speaker {
    Speaker {
        pid: on.pid,
        conversation: Conversation::Named(A.to_owned()),
    }
}

/// A Bash call's ask, on `on`'s hook.
fn asked(on: On, payload: serde_json::Value) -> PermissionAsked {
    PermissionAsked {
        chat: CHAT,
        permission_request: on.source,
        payload,
        ran_for_ms: None,
        speaker: speaker(on),
    }
}

fn bash(command: &str) -> serde_json::Value {
    serde_json::json!({"tool_name": "Bash", "tool_input": {"command": command}})
}

/// A host's hook asks and board, chat 7 opened on `on` and mid-turn, and its permission hearer:
/// each give-up told to the board, as the app's is.
struct Host {
    hooks: Arc<HookAsks>,
    board: Arc<Mutex<Board>>,
    hear: Permitting,
    gave_up: Arc<Mutex<Vec<NotRaised>>>,
}

fn host(on: On) -> Host {
    let hooks = Arc::new(HookAsks::new(Arc::new(Asks::new())));
    let board = Arc::new(Mutex::new(Board::new()));
    {
        let mut board = board.lock().expect("the board");
        board.opened(CHAT, Some(on.harness), None);
        board.reported(&report(on, Event::UserPromptSubmit));
    }
    let gave_up = Arc::new(Mutex::new(Vec::new()));
    let hear = held_or_given_up(Arc::clone(&hooks), Arc::new(|| {}), {
        let board = Arc::clone(&board);
        let gave_up = Arc::clone(&gave_up);
        Arc::new(move |left: GaveUp| {
            assert_eq!(left.chat, CHAT);
            board
                .lock()
                .expect("the board")
                .hook_gave_up(left.chat, &left.speaker, &left.asker);
            gave_up.lock().expect("the list").push(left.why);
        })
    });
    Host {
        hooks,
        board,
        hear,
        gave_up,
    }
}

impl Host {
    fn waits_in_its_terminal(&self) -> bool {
        self.board
            .lock()
            .expect("the board")
            .waits_in_its_terminal(CHAT)
    }

    fn prompt(&self) -> Option<Prompt> {
        self.board.lock().expect("the board").its_prompt(CHAT)
    }

    /// The person answered in the pane: a tool of its own began past the prompt and came back.
    fn answered_in_the_pane(&self, on: On) {
        use crate::doing::{Kind, Said as Tool};
        let mut board = self.board.lock().expect("the board");
        board.tool_said_by(
            CHAT,
            &speaker(on),
            &Tool::Began {
                kind: Kind::Command,
                name: None,
            },
        );
        board.tool_said_by(
            CHAT,
            &speaker(on),
            &Tool::Ended {
                kind: Some(Kind::Command),
            },
        );
    }
}

/// Makes the hook named by `case` give up on `host`, and answers what its hearer said.
fn give_up(host: &Host, on: On, case: &str) -> Option<Waiting> {
    match case {
        "TooBig" => (host.hear)(asked(on, bash(&"x".repeat(MOST_ASK_BYTES + 1)))),
        "NotItsHarness" => {
            // The app started this chat as another harness than the hook names (#1703).
            let other = if on.source == Source::Codex {
                Source::ClaudeCode
            } else {
                Source::Codex
            };
            host.hooks.reading_by(Box::new(move |_| Some(other)));
            (host.hear)(asked(on, bash("npm test")))
        }
        "TooLate" => (host.hear)(PermissionAsked {
            ran_for_ms: Some(59_000),
            ..asked(on, bash("npm test"))
        }),
        "the 32-open cap" => {
            for n in 0..MOST_OPEN_ASKS {
                assert!(
                    (host.hear)(asked(on, bash(&format!("step {n}")))).is_some(),
                    "ask {n} is held"
                );
            }
            (host.hear)(asked(on, bash("one more")))
        }
        other => panic!("no case {other}"),
    }
}

#[test]
fn each_hook_give_up_raises_one_terminal_ask() {
    for on in HARNESSES {
        for case in CASES {
            let host = host(on);
            let why = format!("{case} on {:?}", on.source);
            assert!(!host.waits_in_its_terminal(), "{why}: nothing yet");

            assert!(give_up(&host, on, case).is_none(), "{why}: not held");

            // One give-up told, and the chat waits on its harness's permission prompt.
            assert_eq!(host.gave_up.lock().expect("the list").len(), 1, "{why}");
            assert!(host.waits_in_its_terminal(), "{why}: in its terminal");
            assert_eq!(host.prompt(), Some(Prompt::Permission), "{why}");
            assert!(
                host.board
                    .lock()
                    .expect("the board")
                    .needs_you()
                    .contains(&CHAT),
                "{why}: in the needs-you queue, which lists its one ask"
            );

            // The pane answers it, and the ask is withdrawn.
            host.answered_in_the_pane(on);
            assert!(!host.waits_in_its_terminal(), "{why}: answered in the pane");
            assert_eq!(host.prompt(), None, "{why}");
        }
    }
}

#[test]
fn each_give_up_says_which_case_it_was() {
    for on in HARNESSES {
        let told: Vec<NotRaised> = CASES
            .iter()
            .map(|case| {
                let host = host(on);
                give_up(&host, on, case);
                host.gave_up.lock().expect("the list").remove(0)
            })
            .collect();
        assert_eq!(
            told,
            [
                NotRaised::TooBig,
                NotRaised::NotItsHarness,
                NotRaised::TooLate,
                NotRaised::TooMany,
            ]
        );
    }
}

#[test]
fn a_second_give_up_in_one_wait_is_still_the_one_ask() {
    // Two hooks that gave up while the chat waits on its prompt: one wait, one ask.
    for on in HARNESSES {
        let host = host(on);
        give_up(&host, on, "TooLate");
        let first = host.board.lock().expect("the board").moved_at(CHAT);
        give_up(&host, on, "TooLate");
        assert!(host.waits_in_its_terminal());
        assert_eq!(host.board.lock().expect("the board").moved_at(CHAT), first);
        assert_eq!(
            host.board
                .lock()
                .expect("the board")
                .needs_you()
                .iter()
                .filter(|chat| **chat == CHAT)
                .count(),
            1
        );
    }
}

#[test]
fn a_held_ask_is_no_give_up() {
    // A hook whose ask is held is answered from the window: nothing waits in the terminal.
    for on in HARNESSES {
        let host = host(on);
        assert!((host.hear)(asked(on, bash("npm test"))).is_some());
        assert!(host.gave_up.lock().expect("the list").is_empty());
        assert!(!host.waits_in_its_terminal());
    }
}

/// A report of helper `agent`'s, on `on`'s hooks.
fn from_helper(on: On, event: Event, agent: &str) -> Report {
    Report {
        agent: Some(agent.to_owned()),
        ..report(on, event)
    }
}

/// A helper `a1`'s ask, too big to hold: its hook gives up.
fn a_helper_gives_up(host: &Host, on: On) {
    let mut payload = bash(&"x".repeat(MOST_ASK_BYTES + 1));
    payload["agent_id"] = serde_json::Value::from("a1");
    assert!((host.hear)(asked(on, payload)).is_none());
    assert!(host.waits_in_its_terminal());
}

#[test]
fn a_helper_s_give_up_stands_until_that_helper_moves_on() {
    // A helper's hook gave up (#1644, Q266): the prompt in the pane is that helper's. A tool of
    // the chat's own coming back says nothing of it, nor does an ask of that helper's answered in
    // the window; the helper's end does.
    let on = HARNESSES[0];
    let host = host(on);
    a_helper_gives_up(&host, on);

    host.answered_in_the_pane(on);
    assert!(
        host.waits_in_its_terminal(),
        "the chat's own tools are not the helper's"
    );
    host.board
        .lock()
        .expect("the board")
        .answered_by(CHAT, &Asker::Helper("a1".to_owned()));
    assert!(
        host.waits_in_its_terminal(),
        "an answer in the window is not the pane's"
    );

    host.board
        .lock()
        .expect("the board")
        .reported(&from_helper(on, Event::SubagentStop, "a1"));
    assert!(!host.waits_in_its_terminal(), "the helper ended");
}

#[test]
fn a_helper_s_give_up_goes_with_its_own_tool_line() {
    // The person answered the helper's prompt in the pane: a tool of that helper's own began past
    // it and came back (#1644).
    use crate::doing::{Kind, Said as Tool};
    let on = HARNESSES[0];
    let host = host(on);
    a_helper_gives_up(&host, on);

    let mut board = host.board.lock().expect("the board");
    board.child_tool_said_by(
        CHAT,
        "a1",
        &speaker(on),
        &Tool::Began {
            kind: Kind::Command,
            name: None,
        },
    );
    board.child_tool_said_by(
        CHAT,
        "a1",
        &speaker(on),
        &Tool::Ended {
            kind: Some(Kind::Command),
        },
    );
    assert!(!board.waits_in_its_terminal(CHAT));
}

#[test]
fn at_the_32_open_cap_answering_another_ask_in_the_window_leaves_the_terminal_row() {
    // Q266: a give-up is its asker's. The 32 asks the window holds are the chat's too, and
    // answering one of them there says nothing of the prompt the 33rd left in the pane, whoever
    // the window read as asking it.
    for on in HARNESSES {
        for answered in [Asker::Chat, Asker::Unread] {
            let host = host(on);
            assert!(give_up(&host, on, "the 32-open cap").is_none());
            assert!(host.waits_in_its_terminal());

            host.board
                .lock()
                .expect("the board")
                .answered_by(CHAT, &answered);
            let why = format!("{:?}, answered as {answered:?}", on.source);
            assert!(host.waits_in_its_terminal(), "{why}: the pane's prompt stands");
            assert_eq!(host.prompt(), Some(Prompt::Permission), "{why}");

            host.answered_in_the_pane(on);
            assert!(!host.waits_in_its_terminal(), "{why}: answered in the pane");
        }
    }
}

#[test]
fn the_chat_s_own_give_up_answered_in_its_pane_while_a_helper_still_asks_clears_the_chat_s_part()
{
    // Q266: the chat's own prompt, answered in its pane, was the only one in the pane. Its helper
    // still asks, in the window, so the chat still waits, but no longer in its terminal.
    let on = HARNESSES[0];
    let host = host(on);
    assert!(give_up(&host, on, "TooBig").is_none());
    host.board
        .lock()
        .expect("the board")
        .reported(&from_helper(on, Event::Notification, "a1"));
    assert!(host.waits_in_its_terminal());

    host.answered_in_the_pane(on);
    assert!(!host.waits_in_its_terminal(), "its own prompt was answered");
    assert!(
        host.board
            .lock()
            .expect("the board")
            .waits_on_its_prompt(CHAT),
        "its helper still asks"
    );
}

#[test]
fn the_turn_s_end_clears_every_give_up() {
    for on in HARNESSES {
        let host = host(on);
        assert!(give_up(&host, on, "TooBig").is_none());
        host.board
            .lock()
            .expect("the board")
            .reported(&report(on, Event::Stop));
        assert!(!host.waits_in_its_terminal(), "{:?}", on.source);
        assert_eq!(host.prompt(), None, "{:?}", on.source);
    }
}

#[test]
fn a_harness_nested_in_the_chat_never_marks_it_waiting_in_its_terminal() {
    // Q266: a give-up counts only from the run the chat adopted, as `Board::tool_said_by` holds a
    // tool line. A harness nested in the chat holds its token and sits in its process tree.
    for on in HARNESSES {
        let host = host(on);
        let nested = Speaker {
            pid: on.pid.map(|pid| pid + 1),
            conversation: Conversation::Named(B.to_owned()),
        };
        assert!(
            (host.hear)(PermissionAsked {
                speaker: nested,
                ..asked(on, bash(&"x".repeat(MOST_ASK_BYTES + 1)))
            })
            .is_none()
        );
        assert_eq!(
            host.gave_up.lock().expect("the list").len(),
            1,
            "{:?}: its hook is still told no option",
            on.source
        );
        assert!(!host.waits_in_its_terminal(), "{:?}", on.source);
        assert_eq!(host.prompt(), None, "{:?}", on.source);
    }
}

#[test]
fn a_chat_started_at_its_folder_trust_prompt_with_no_hook_yet_is_can_t_tell() {
    // R-351: the harness asks whether to trust its folder before any hook of it loads, so
    // nothing reports the chat. The real board lists no Terminal ask, and its state reads Can't
    // tell, by the same reading as the window's All chats.
    use crate::catalogue::chats::{Facts, Mark, mark};
    for on in HARNESSES {
        let mut board = Board::new();
        board.opened(CHAT, Some(on.harness), None);

        assert!(!board.waits_in_its_terminal(CHAT), "{:?}", on.source);
        assert_eq!(board.its_prompt(CHAT), None, "{:?}", on.source);
        let needs_you = board.needs_you().contains(&CHAT);
        assert!(!needs_you, "{:?}", on.source);
        let facts = Facts {
            board: Some(board.state(CHAT)),
            needs_you,
            ..Facts::default()
        };
        assert_eq!(
            mark(&facts),
            (Mark::CantTell, "no-hook-heard"),
            "{:?}",
            on.source
        );
    }
}
