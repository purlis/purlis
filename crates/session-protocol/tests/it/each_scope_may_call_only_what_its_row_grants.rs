// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What each client scope may call, checked once inside `session::serve` before any `Host` is
//! asked (FD-27, ADR 0068 §5 as amended by FD-27). A command the scope's row does not grant is
//! refused `not_allowed` and never reaches the host, and the link carries on.

use std::sync::{Arc, Mutex};

use purlis_session_protocol::auth::Scope;
use purlis_session_protocol::link;
use purlis_session_protocol::session::{
    self, Answer, Chat, Client, Command, Host, Peer, Refusal, Start,
};
use tokio::io::duplex;

use crate::common;
use common::HELD;

const CHAT: &str = "01J9ZQ3V7K8M2N4P6R8T0V2X4Z";
const ASK: &str = "ask-1";
const CONFINEMENT: &str = "01J9CNFNMNT0000000000000000";

/// A host that answers every command, keeps each it was sent, and says whether an ask elicits a
/// secret as it is told.
#[derive(Clone)]
struct Board {
    heard: Arc<Mutex<Vec<String>>>,
    secret: bool,
}

impl Board {
    fn new(secret: bool) -> Board {
        Board {
            heard: Arc::default(),
            secret,
        }
    }

    fn heard(&self) -> Vec<String> {
        self.heard.lock().unwrap().clone()
    }
}

impl Host for Board {
    async fn call(&self, command: Command, _peer: &Peer) -> Result<Answer, Refusal> {
        self.heard.lock().unwrap().push(command.word().to_owned());
        Ok(match command {
            Command::List => Answer::Chats(vec![Chat {
                chat: CHAT.into(),
                project: "01J9PRJCT00000000000000000".into(),
                project_path: None,
                title: None,
                state: "needs_you".into(),
                ..Chat::default()
            }]),
            Command::StartProgram(_) => Answer::ProgramStarted {
                program: CHAT.into(),
                number: None,
                process: None,
                confined: None,
                sandbox: None,
            },
            Command::AttachView { kind, .. } => Answer::Attached(session::Attached {
                view: 1,
                cols: 80,
                rows: 24,
                kind: Some(kind),
            }),
            Command::StopAll | Command::KillSwitch => Answer::KillSwitch(session::KillSwitch {
                stopped: true,
                why: None,
                ..Default::default()
            }),
            Command::Rearm => Answer::KillSwitch(session::KillSwitch {
                stopped: false,
                why: None,
                ..Default::default()
            }),
            _ => Answer::Done,
        })
    }

    async fn subscribe(
        &self,
        _since: session::Since,
        _peer: &Peer,
    ) -> Result<tokio::sync::mpsc::Receiver<session::Pushed>, Refusal> {
        self.heard.lock().unwrap().push("subscribe".into());
        let (_tx, rx) = tokio::sync::mpsc::channel(1);
        Ok(rx)
    }

    async fn elicits_a_secret(&self, _chat: &str, _ask: &str) -> bool {
        self.secret
    }
}

/// A link admitted as `scope`, with `host` behind it.
async fn linked(scope: Scope, host: Board) -> Client {
    let (a, b) = duplex(256 * 1024);
    let (client, served) = if scope == Scope::RemoteLink {
        tokio::join!(
            link::connect_as_a_device(a, session::speaks()),
            link::serve_as_a_device(b, session::speaks(), link::ProvenDevice::stand_in())
        )
    } else {
        tokio::join!(
            link::connect(a, session::speaks(), scope, HELD.of(scope)),
            link::serve_any(b, session::speaks(), &HELD)
        )
    };
    let served = served.unwrap();
    assert_eq!(served.scope(), scope);
    tokio::spawn(session::serve(served, host, None));
    let (client, _views, _events) = Client::new(client.unwrap());
    client
}

/// Every command, once, in the table's order.
fn every_command() -> Vec<Command> {
    vec![
        Command::List,
        Command::Attach { chat: CHAT.into() },
        Command::Detach {
            chat: CHAT.into(),
            view: 1,
        },
        Command::Write {
            chat: CHAT.into(),
            bytes: b"y".to_vec(),
        },
        Command::Resize {
            chat: CHAT.into(),
            cols: 100,
            rows: 30,
        },
        Command::Answer {
            chat: CHAT.into(),
            ask: ASK.into(),
            answer: "yes".into(),
        },
        Command::Stop { chat: CHAT.into() },
        Command::Start(Start {
            project: "01J9PRJCT00000000000000000".into(),
            workspace: None,
            harness: None,
        }),
        Command::StartProgram(Box::new(session::StartProgram {
            workspace: None,
            harness: None,
            ..session::StartProgram::of("01J9PRJCT00000000000000000", session::ProgramKind::Shell)
        })),
        Command::AttachView {
            program: CHAT.into(),
            kind: session::ViewKind::Terminal,
            role: None,
            rate: None,
        },
        Command::DetachView {
            program: CHAT.into(),
            view: 1,
        },
        Command::Subscribe {
            since: session::Since::new(),
        },
        Command::Unsubscribe,
        Command::HoldProject {
            project: "01J9PRJCT00000000000000000".into(),
            project_path: "/home/me/project".into(),
        },
        Command::LetGoOfProject {
            project: "01J9PRJCT00000000000000000".into(),
        },
        Command::Close {
            program: CHAT.into(),
        },
        Command::Readiness {
            program: CHAT.into(),
        },
        Command::StartSaving {
            project: "01J9PRJCT00000000000000000".into(),
        },
        Command::StopSaving {
            project: "01J9PRJCT00000000000000000".into(),
        },
        Command::FetchProject {
            project: "01J9PRJCT00000000000000000".into(),
        },
        Command::MidTurn {
            question: 1,
            chats: None,
        },
        // A start the window composed: the window's alone, or relayed by its host.
        Command::StartProgram(Box::new(session::StartProgram {
            launch: Some(session::Launch {
                program: "/bin/sh".into(),
                args: Vec::new(),
                cwd: None,
                set: Vec::new(),
                strip: Vec::new(),
                pass: Vec::new(),
                declared: Vec::new(),
                harness: None,
                git_hooks: None,
                reporting: None,
                confinement: None,
            }),
            ..session::StartProgram::of("01J9PRJCT00000000000000000", session::ProgramKind::Shell)
        })),
        Command::Deal {
            project: "01J9PRJCT00000000000000000".into(),
        },
        Command::DealtAlready {
            project: "01J9PRJCT00000000000000000".into(),
            dealt: 6,
        },
        Command::ReplyHeard {
            project: "01J9PRJCT00000000000000000".into(),
            heard: 12,
            reply: serde_json::json!("taken"),
        },
        // A chat's confinement and its sandbox's answers: the window's alone (1.4).
        Command::Confine(Box::new(session::Confine {
            confinement: CONFINEMENT.into(),
            project: "01J9PRJCT00000000000000000".into(),
            plan: session::ConfinePlan {
                listed: Vec::new(),
                never: Vec::new(),
                withdrawn: Vec::new(),
                anything: false,
                tunnel_ports: vec![443],
                plain_ports: vec![80],
                any_port: false,
                beside: session::Beside::Wrapped,
                given: Vec::new(),
            },
            asks: true,
        })),
        Command::ChangeReach {
            confinement: CONFINEMENT.into(),
            change: session::ReachChange::Allow {
                host: "pypi.example.com".into(),
                layer: session::Layer::Chat,
            },
        },
        Command::ReachOf {
            confinement: CONFINEMENT.into(),
        },
        Command::LetGoOfConfinement {
            confinement: CONFINEMENT.into(),
        },
        // A start whose composed confinement is named to its link: the window's alone, or
        // relayed by its host (1.16).
        Command::StartProgram(Box::new(session::StartProgram {
            confinement: Some(CONFINEMENT.into()),
            ..session::StartProgram::of("01J9PRJCT00000000000000000", session::ProgramKind::Harness)
        })),
        // The kill switch (1.5, ADR 0071): any scope that may stop stops every program, and
        // only the window re-arms.
        Command::StopAll,
        Command::KillSwitch,
        Command::Rearm,
        // A browser, started and watched: the window's alone (1.6).
        Command::StartProgram(Box::new(session::StartProgram::of(
            "01J9PRJCT00000000000000000",
            session::ProgramKind::Browser,
        ))),
        Command::AttachView {
            program: CHAT.into(),
            kind: session::ViewKind::Browser,
            role: Some(session::ViewRole::Focused),
            rate: None,
        },
        // A write in the event log the host holds: the window's alone (1.8).
        Command::Record {
            entry: serde_json::json!({ "entry": "kill_switch_read" }),
        },
        // A held project's board, read and moved: the window's alone (1.10).
        Command::BoardMoved {
            project: "01J9PRJCT00000000000000000".into(),
            chat: 7,
            moved: serde_json::json!({"move": "ignored"}),
        },
        Command::Board {
            project: "01J9PRJCT00000000000000000".into(),
        },
        // A browser driven for a chat: the window's alone (1.11).
        Command::Browse {
            program: CHAT.into(),
            call_id: "c-1".into(),
            act: purlis_session_protocol::browsing::Act::Pages,
            reach: None,
        },
        // A window that comes back takes a held project's programs again, and a quit ends the
        // host: the window's alone (1.13).
        Command::ProgramsOf {
            project: "01J9PRJCT00000000000000000".into(),
        },
        Command::Quit,
        // The window's note of a program, kept in the host for it (1.39, #2025).
        Command::NoteProgram {
            program: CHAT.into(),
            note: session::ProgramNote(serde_json::json!({"chat": 3})),
        },
        // The desktop taken out of a runner's peers (1.25, #2525).
        Command::Unpair,
        // A desktop's profile kept on its runner (1.43, #2049).
        Command::KeepProfile(session::ProfileToKeep {
            name: "work".into(),
            harness: "claude".into(),
            sandbox_only: false,
            pinned: None,
            settings: None,
            settings_staged: None,
        }),
        // A desktop's profile taken off its runner (1.52, #4047).
        Command::ForgetProfile {
            name: "work".into(),
        },
        // Its settings in batches, for that keep (1.56, #4081).
        Command::StageProfileSettings(session::SettingsBatch {
            profile: "work".into(),
            pieces: Vec::new(),
        }),
        // The profiles a runner keeps for its desktop (1.57, #4060).
        Command::KeptProfiles,
        // A browser the person takes over, drives and hands back: the window's alone (1.14).
        Command::TakeOver {
            program: CHAT.into(),
        },
        Command::BrowserInput {
            program: CHAT.into(),
            input: purlis_session_protocol::browsing::Input::Key {
                aim: purlis_session_protocol::browsing::Aim {
                    page: "p1".into(),
                    document: 1,
                },
                action: purlis_session_protocol::browsing::KeyAction::Down,
                key: "a".into(),
                physical: "KeyA".into(),
                key_code: 65,
                modifiers: 0,
            },
        },
        Command::SizeBrowser {
            program: CHAT.into(),
            width: 390,
            height: 844,
        },
        Command::HandBack {
            program: CHAT.into(),
        },
        // A login purlis performs carries a vault's values (1.18, #2016): the window's alone.
        Command::LogIn {
            program: CHAT.into(),
            login: purlis_session_protocol::browsing::LogIn {
                address: "https://shop.example.test/login".into(),
                user: None,
                password: purlis_session_protocol::browsing::Secret::new("not-a-real-one".into()),
                keep: None,
                identity: None,
            },
        },
        // A login that keeps its login on this device once the browser ends (1.27, #2021): the
        // window's alone, never relayed to a runner (1.38, #2050).
        Command::LogIn {
            program: CHAT.into(),
            login: purlis_session_protocol::browsing::LogIn {
                address: "https://shop.example.test/login".into(),
                user: None,
                password: purlis_session_protocol::browsing::Secret::new("not-a-real-one".into()),
                keep: Some(purlis_session_protocol::browsing::KeptLogin {
                    profile: "claude".into(),
                    persona: "steward".into(),
                    login: "shop-login".into(),
                    identity: "0123456789abcdef0123456789abcdef".into(),
                    address: "https://shop.example.test/login".into(),
                }),
                identity: None,
            },
        },
        // A browser opened from kept logins (1.27, #2021): the window's alone, as a login is.
        Command::StartProgram(Box::new(session::StartProgram {
            kept: Some(purlis_session_protocol::browsing::KeptLogin {
                profile: "claude".into(),
                persona: "steward".into(),
                login: "shop-login".into(),
                identity: "0123456789abcdef0123456789abcdef".into(),
                address: "https://shop.example.test/login".into(),
            }),
            ..session::StartProgram::of("01J9PRJCT00000000000000000", session::ProgramKind::Browser)
        })),
        // A start that keeps a note for the window (1.39, #2025): the window's alone, even of a
        // shell, which a terminal may start.
        Command::StartProgram(Box::new(session::StartProgram {
            note: Some(session::ProgramNote(serde_json::json!({"chat": 3}))),
            ..session::StartProgram::of("01J9PRJCT00000000000000000", session::ProgramKind::Shell)
        })),
        // A relaying host names the window a link is for, and is named again the confinements
        // of the chats that window started (1.22, #2511): over the link between hosts alone.
        Command::TakeBack {
            window: "01JA0WNDW0000000000000000".into(),
        },
        // The window naming itself on a link to its own host (1.31, #2623): the window's alone,
        // never a link between hosts.
        Command::NameWindow {
            name: purlis_session_protocol::browsing::Secret::new(
                "0123456789abcdef0123456789abcdef".into(),
            ),
        },
        // The window asks its own host to relay its link to a runner it keeps (1.33, #2679).
        Command::RelayTo {
            runner: "builds".into(),
        },
        // A repository's history moved over the link as a git bundle (1.34, #2046).
        Command::TakeCode {
            project: "01J9PRJCT00000000000000000".into(),
            repo: "workspaces/app/web".into(),
            transfer: 1,
            refs: vec![session::CodeRef {
                name: "refs/heads/fix-login".into(),
                oid: "0123456789abcdef0123456789abcdef01234567".into(),
            }],
            checkout: Some("refs/heads/fix-login".into()),
            moving: false,
        },
        Command::GiveCode {
            project: "01J9PRJCT00000000000000000".into(),
            repo: "workspaces/app/web".into(),
            transfer: 2,
            have: Vec::new(),
            with_work: false,
        },
        Command::TakeConversation {
            project: "01J9PRJCT00000000000000000".into(),
            transfer: 3,
            workspace: "app".into(),
            profile: "claude".into(),
            conversation: "0d6f3c1e".into(),
            file: "{config}/projects/{cwd_key}/0d6f3c1e.jsonl".into(),
        },
        Command::GiveConversation {
            project: "01J9PRJCT00000000000000000".into(),
            transfer: 4,
            workspace: "app".into(),
            profile: "claude".into(),
            conversation: "0d6f3c1e".into(),
        },
        // The clones a host holds in a workspace (1.51, #3811): the window's, and relayed.
        Command::ClonesOf {
            project: "01J9PRJCT00000000000000000".into(),
            workspace: "app".into(),
        },
        // Where the host's own event log ends (1.41, #3393): the window's, and relayed.
        Command::LogEnd,
        // The person's DevTools of a browser they drive (1.45, #2028): the window's alone, on its
        // own host, never relayed.
        Command::Inspect {
            program: CHAT.into(),
            width: 1000,
            height: 600,
        },
        Command::AttachView {
            program: CHAT.into(),
            kind: session::ViewKind::Inspector,
            role: None,
            rate: None,
        },
        Command::InspectorInput {
            program: CHAT.into(),
            input: purlis_session_protocol::browsing::Input::Key {
                aim: purlis_session_protocol::browsing::Aim {
                    page: "devtools".into(),
                    document: 1,
                },
                action: purlis_session_protocol::browsing::KeyAction::Down,
                key: "a".into(),
                physical: "KeyA".into(),
                key_code: 65,
                modifiers: 0,
            },
        },
    ]
}

/// Which of [`every_command`] `scope` is granted, by the table in ADR 0068 as amended by FD-27,
/// written out here by hand as the spec, not computed from the code.
fn granted(scope: Scope) -> &'static [&'static str] {
    match scope {
        Scope::LocalUi => &[
            "list",
            "attach",
            "detach",
            "write",
            "resize",
            "answer",
            "stop",
            "start",
            "start_program",
            "attach_view",
            "detach_view",
            "subscribe",
            "unsubscribe",
            "hold_project",
            "let_go_of_project",
            "close",
            "readiness",
            "start_saving",
            "stop_saving",
            "fetch_project",
            "mid_turn",
            "start_program", // the start the window composed, with its launch
            "deal",
            "dealt_already",
            "reply_heard",
            "confine",
            "change_reach",
            "reach_of",
            "let_go_of_confinement",
            "start_program", // the start whose composed confinement is named to its link
            "stop_all",
            "kill_switch",
            "rearm",
            "start_program",
            "attach_view",
            "record",
            "board_moved",
            "board",
            "browse",
            "programs_of",
            "quit",
            "note_program",
            "take_over",
            "browser_input",
            "size_browser",
            "hand_back",
            "log_in",
            "log_in",        // a login kept on this device
            "start_program", // a browser opened from kept logins
            "start_program", // a program noted for the window
            "name_window",
            "relay_to",
            "take_code",
            "give_code",
            "take_conversation",
            "give_conversation",
            "clones_of",
            "log_end",
            "inspect",
            "attach_view", // DevTools' frames
            "inspector_input",
        ],
        Scope::Terminal => &[
            "list",
            "attach",
            "detach",
            "write",
            "resize",
            "stop",
            "start",
            "start_program",
            "attach_view",
            "detach_view",
            "subscribe",
            "unsubscribe",
            "close",
            "readiness",
            "stop_all",
            "kill_switch",
        ],
        // The window's own commands, relayed by its host over the link between hosts (ruled
        // 2026-10-15, #2042): a hold, a start with its launch, the numbers dealt, its reply to
        // what a project's hooks wrote, and a chat's confinement (#2217); never an answer or a
        // re-arm. And the relaying host's own take-back of its window's runner chats (#2511).
        Scope::RemoteLink => &[
            "list",
            "attach",
            "detach",
            "write",
            "resize",
            "stop",
            "start",
            "start_program",
            "attach_view",
            "detach_view",
            "subscribe",
            "unsubscribe",
            "hold_project",
            "let_go_of_project",
            "close",
            "readiness",
            "start_program", // the start the window composed, with its launch
            "deal",
            "dealt_already",
            "reply_heard",
            "confine",
            "change_reach",
            "reach_of",
            "let_go_of_confinement",
            "start_program", // the start whose composed confinement is named to its link
            "stop_all",
            "kill_switch",
            // Its desktop re-arms it, as its window re-arms the desktop (#2051, V29b).
            "rearm",
            // A chat's browser on the runner, for the desktop's window (1.38, #2050): started,
            // watched, driven for its chat and by the person, and logged in by purlis; never a
            // kept login, which stays on the device it was made on.
            "start_program",
            "attach_view",
            "browse",
            // The desktop removing the runner (1.25, #2525).
            "unpair",
            "keep_profile",
            "forget_profile",
            "stage_profile_settings",
            "kept_profiles",
            "take_over",
            "browser_input",
            "size_browser",
            "hand_back",
            "log_in",
            "take_back",
            // Code moved for the desktop's window (1.34, #2046), and a chat's conversation as it
            // moves (1.44, #2048), with the clones it may bring back (1.51, #3811).
            "take_code",
            "give_code",
            "take_conversation",
            "give_conversation",
            "clones_of",
            // The runner's own event log, read by its desktop's window (1.41, #3393).
            "log_end",
        ],
        Scope::FleetMcp => &[
            "list",
            "stop",
            "subscribe",
            "unsubscribe",
            "close",
            "readiness",
            "stop_all",
            "kill_switch",
        ],
        Scope::Approval => &[
            "list",
            "answer",
            "stop",
            "subscribe",
            "unsubscribe",
            "close",
            "readiness",
            "stop_all",
            "kill_switch",
        ],
        Scope::Editor => &[],
    }
}

async fn check_the_row(scope: Scope) {
    let board = Board::new(false);
    let client = linked(scope, board.clone()).await;
    let mut reached = Vec::new();
    for command in every_command() {
        let word = command.word();
        match client.call(command).await {
            Ok(_) => reached.push(word),
            Err(refused) => assert_eq!(
                refused.refusal().map(|r| r.code.as_str()),
                Some(session::NOT_ALLOWED),
                "{scope} {word}: {refused}"
            ),
        }
    }
    assert_eq!(reached, granted(scope), "{scope}: what it was let call");
    // The refused ones never reached the host; `unsubscribe` is the link's own, and 1.0's words
    // reach it as the hosted-program commands.
    let to_the_host: Vec<&str> = granted(scope)
        .iter()
        .copied()
        .filter(|word| *word != "unsubscribe")
        .map(|word| match word {
            "start" => "start_program",
            "attach" => "attach_view",
            "detach" => "detach_view",
            word => word,
        })
        .collect();
    assert_eq!(board.heard(), to_the_host, "{scope}: what the host heard");
}

#[tokio::test]
async fn local_ui_may_call_every_command_but_a_relaying_hosts_take_back() {
    check_the_row(Scope::LocalUi).await;
}

#[tokio::test]
async fn terminal_may_drive_a_chat_but_never_answer_its_asks() {
    check_the_row(Scope::Terminal).await;
}

#[tokio::test]
async fn fleet_mcp_may_list_watch_and_stop_and_nothing_else() {
    check_the_row(Scope::FleetMcp).await;
}

#[tokio::test]
async fn approval_may_answer_list_watch_and_stop_and_never_drive_a_chat() {
    check_the_row(Scope::Approval).await;
}

#[tokio::test]
async fn editor_may_call_none_of_the_session_commands() {
    check_the_row(Scope::Editor).await;
}

#[tokio::test]
async fn remote_link_may_drive_a_chat_and_take_the_windows_relayed_start_but_never_answer_its_asks()
{
    check_the_row(Scope::RemoteLink).await;
}

#[tokio::test]
async fn only_the_window_or_its_hosts_relay_names_what_the_person_allowed_of_a_persona() {
    // A scope that may start nothing is refused that first.
    for scope in Scope::ALL
        .into_iter()
        .filter(|scope| scope.may(purlis_session_protocol::grants::Power::Start))
    {
        let client = linked(scope, Board::new(false)).await;
        let said = client
            .call(Command::StartProgram(Box::new(session::StartProgram {
                tools_allowed: Some("digest".into()),
                ..session::StartProgram::of(
                    "01J9PRJCT00000000000000000",
                    session::ProgramKind::Harness,
                )
            })))
            .await;
        let refused = said
            .err()
            .and_then(|why| why.refusal().map(|refusal| refusal.why.clone()))
            .is_some_and(|why| why.contains("persona's tools"));
        let windows = matches!(scope, Scope::LocalUi | Scope::RemoteLink);
        assert_eq!(refused, !windows, "{scope}");
    }
}

#[tokio::test]
async fn every_scope_that_may_stop_stops_all_and_is_told_in_words_that_only_the_window_re_arms() {
    // A runner's `remote-link` is re-armed by its desktop (#2051, V29b): it is not among them.
    for scope in [Scope::Terminal, Scope::FleetMcp, Scope::Approval] {
        let board = Board::new(false);
        let client = linked(scope, board.clone()).await;

        let stopped = client.stop_all().await.expect("a stop needs no human");
        let refused = client.rearm().await.unwrap_err();

        assert!(stopped.stopped, "{scope}");
        let refusal = refused.refusal().expect("refused by the host");
        assert_eq!(refusal.code, session::NOT_ALLOWED, "{scope}");
        assert!(
            refusal.why.contains("purlis's window"),
            "{scope}: the refusal does not say where a re-arm is made: {}",
            refusal.why
        );
        assert_eq!(
            board.heard(),
            ["stop_all"],
            "{scope}: the re-arm never reached the host"
        );
    }
}

#[tokio::test]
async fn an_ask_that_elicits_a_secret_is_answered_from_local_ui_only() {
    for (scope, answers) in [(Scope::LocalUi, true), (Scope::Approval, false)] {
        let board = Board::new(true);
        let client = linked(scope, board.clone()).await;
        let answered = client.answer(CHAT, ASK, "yes").await;
        assert_eq!(answered.is_ok(), answers, "{scope}: {answered:?}");
        if !answers {
            assert_eq!(
                answered.unwrap_err().refusal().map(|r| r.code.clone()),
                Some(session::NOT_ALLOWED.to_owned())
            );
            assert!(board.heard().is_empty(), "the host never heard it");
        }
    }
}

/// A host that says nothing about its asks is taken to hold secret ones: `approval` answers
/// only when the host says the ask elicits no secret.
#[derive(Clone, Default)]
struct Silent;

impl Host for Silent {
    async fn call(&self, _command: Command, _peer: &Peer) -> Result<Answer, Refusal> {
        Ok(Answer::Done)
    }
}

#[tokio::test]
async fn a_host_that_says_nothing_of_an_ask_has_it_answered_from_local_ui_only() {
    let (a, b) = duplex(64 * 1024);
    let (client, served) = tokio::join!(
        link::connect(
            a,
            session::speaks(),
            Scope::Approval,
            HELD.of(Scope::Approval)
        ),
        link::serve_any(b, session::speaks(), &HELD)
    );
    tokio::spawn(session::serve(served.unwrap(), Silent, None));
    let (client, _views, _events) = Client::new(client.unwrap());
    let refused = client.answer(CHAT, ASK, "yes").await.unwrap_err();
    assert_eq!(
        refused.refusal().map(|r| r.code.as_str()),
        Some(session::NOT_ALLOWED)
    );
}
