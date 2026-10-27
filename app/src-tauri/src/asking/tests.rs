// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The asks registry's window side (#1690): one adapter per source, what each derives, the
//! path each is answered by, and the list as a whole.

use std::path::Path;
use std::time::Instant;

use purlis_core::harness::asks::{Admitted, Answerer, Asks};
use purlis_core::harness::model::{Ask, Channel, Choice, ChoiceKind, ChoiceScope, Prompt};
use purlis_core::sandbox::policy::Locks;

use super::*;
use crate::dispatchgrants::DispatchPending;
use crate::sandboxing::{GrantLevel, GrantWhat};
use crate::taskblocks::{BlockShown, HeldBlock};

fn choice(id: &str, kind: ChoiceKind, scope: ChoiceScope) -> Choice {
    Choice {
        id: id.to_owned(),
        label: id.to_owned(),
        kind,
        scope,
    }
}

/// A Claude Code hook's ask, with allow and deny for this call.
fn a_hook_ask(summary: &str) -> Ask {
    Ask {
        options: vec![
            choice("allow", ChoiceKind::Allow, ChoiceScope::Once),
            choice("deny", ChoiceKind::Reject, ChoiceScope::Once),
        ],
        summary: purlis_core::harness::model::Summary::of(summary),
        channel: Channel::Hook,
        ..Ask::default()
    }
}

/// Chat `session`'s dispatch `id` to `target`, held for the person, offering `levels`.
fn a_dispatch(id: u32, session: u32, target: &str, levels: Vec<GrantLevel>) -> DispatchPending {
    DispatchPending {
        plane: PlaneId::for_tests(Path::new("/p/one")),
        id,
        session,
        chat: format!("chat {session}"),
        asking: Some("steward".to_owned()),
        target: target.to_owned(),
        brief: "Look at the logs".to_owned(),
        brief_cut: false,
        brief_lines: 1,
        levels,
        locked: None,
        never_unread: None,
        works_in: None,
        works_in_missing: false,
        allowed_in: Vec::new(),
        works_with: String::new(),
        also: Vec::new(),
        shown: "digest-1".to_owned(),
        task: None,
        task_cut: false,
        profile: None,
        mode: "task".to_owned(),
    }
}

fn a_host_block(target: &str) -> HeldBlock {
    HeldBlock {
        operation: "connect".to_owned(),
        kind: "host".to_owned(),
        what: GrantWhat::Host,
        target: target.to_owned(),
    }
}

fn ids(shown: &Shown) -> Vec<&str> {
    shown.options.iter().map(|one| one.id.as_str()).collect()
}

/// What one project has waiting, with nothing yet: each test fills in its sources.
struct World {
    permissions: Vec<purlis_core::harness::asks::Raised>,
    queue: Vec<u32>,
    at_prompt: Vec<u32>,
    /// Chats stopped on a prompt whose kind their nudge named (#1691).
    prompts: Vec<(u32, Prompt)>,
    dispatches: Vec<DispatchPending>,
    blocks: Vec<(u32, HeldBlock)>,
    /// The workspaces a chat asked the person to remove, with what each holds (qw356).
    removals: Vec<(crate::removalasks::Waits, purlis_core::removalask::Holds)>,
    locks: Locks,
    /// Whether the project's sandbox is in force, so a browser site for everyone counts (#2443).
    everyone: bool,
    names: Vec<(u32, &'static str)>,
    askers: Vec<(u32, u32)>,
    /// Chats in the queue only for what is an update (#1693, #1694): tasks of theirs that came
    /// to nothing, a report with nowhere to go, a commit refused.
    only_updates: Vec<u32>,
    /// The hosts a chat's proxy holds a connection to now, and when that hold ends (#1709).
    held: Vec<(u32, &'static str, u32)>,
    /// When each ask began, by its key (#1700).
    since: Vec<(&'static str, u32)>,
    /// The profiles waiting to be signed in (#2035).
    sign_ins: Vec<purlis_core::profilestore::sign_in::Wait>,
    pool_sign_ins: Vec<purlis_core::profilestore::onrunner::PoolWait>,
    sandbox_only: Option<purlis_core::profilestore::sandbox_only::Wait>,
    /// The chats whose first browser call asks what their browser may reach (#2013).
    browsers: Vec<crate::chatbrowsers::Waits>,
    /// The browsers whose chat asked the person to take them over (#2015).
    needs_you: Vec<crate::browserneedsyou::Waiting>,
    /// The chats whose browser's sites wait on the person (#2018).
    browser_sites: Vec<crate::chatbrowsers::SitesWait>,
    pool_logins: Vec<crate::chatbrowsers::PoolLoginWaits>,
    pool_tools: Vec<crate::pooltools::PoolToolWaits>,
    /// What the project's committed file declares it expects (ADR 0093).
    declared: purlis_core::sandbox::services::Declared,
}

impl World {
    fn new() -> Self {
        Self {
            permissions: Vec::new(),
            queue: Vec::new(),
            at_prompt: Vec::new(),
            prompts: Vec::new(),
            dispatches: Vec::new(),
            blocks: Vec::new(),
            removals: Vec::new(),
            locks: Locks::none(),
            everyone: true,
            names: Vec::new(),
            askers: Vec::new(),
            only_updates: Vec::new(),
            held: Vec::new(),
            since: Vec::new(),
            sign_ins: Vec::new(),
            pool_sign_ins: Vec::new(),
            sandbox_only: None,
            browsers: Vec::new(),
            needs_you: Vec::new(),
            browser_sites: Vec::new(),
            pool_logins: Vec::new(),
            pool_tools: Vec::new(),
            declared: purlis_core::sandbox::services::Declared::default(),
        }
    }

    fn asks(&self) -> Vec<Shown> {
        let at_prompt = |session: u32| {
            self.prompts
                .iter()
                .find(|(one, _)| *one == session)
                .map(|(_, prompt)| *prompt)
                .or_else(|| self.at_prompt.contains(&session).then_some(Prompt::Unsaid))
        };
        let name_of = |session: u32| {
            self.names
                .iter()
                .find(|(one, _)| *one == session)
                .map(|(_, name)| (*name).to_owned())
        };
        let asker_of = |session: u32| {
            self.askers
                .iter()
                .find(|(task, _)| *task == session)
                .map(|(_, asker)| *asker)
        };
        derive(&Waiting {
            sign_ins: self.sign_ins.clone(),
            pool_sign_ins: self.pool_sign_ins.clone(),
            sandbox_only: self.sandbox_only.clone(),
            proposals: Vec::new(),
            runner_writes: Vec::new(),
            browsers: self.browsers.clone(),
            needs_you: self.needs_you.clone(),
            browser_sites: self.browser_sites.clone(),
            pool_logins: self.pool_logins.clone(),
            pool_tools: self.pool_tools.clone(),
            permissions: self.permissions.clone(),
            queue: self.queue.clone(),
            at_its_prompt: &at_prompt,
            dispatches: self.dispatches.clone(),
            blocks: self.blocks.clone(),
            removals: self.removals.clone(),
            locks: &self.locks,
            everyone: self.everyone,
            declared: &self.declared,
            name_of: &name_of,
            asker_of: &asker_of,
            only_updates: &|session| self.only_updates.contains(&session),
            held: &|session, target, _| {
                self.held
                    .iter()
                    .find(|(one, host, _)| *one == session && *host == target)
                    .map(|(_, _, until)| *until)
            },
            since: &|ask| {
                self.since
                    .iter()
                    .find(|(key, _)| *key == ask)
                    .map(|(_, at)| *at)
            },
            runner_of: &|_| None,
        })
    }
}

// The permission adapter.

#[test]
fn a_held_permission_prompt_is_an_ask_answered_on_its_own_hook() {
    let asks = Asks::new();
    let raised = asks
        .raise("3", a_hook_ask("Run cargo test"), Instant::now())
        .raised;

    let shown = permission(raised.clone()).expect("a chat of this project's");

    assert_eq!(shown.source, AskSource::Permission);
    assert_eq!(shown.session, 3);
    assert_eq!(shown.ask, raised.id.to_string());
    assert_eq!(shown.says, "Run cargo test");
    assert_eq!(ids(&shown), ["allow", "deny"]);
    assert_eq!(shown.answer, AnswerPath::Hook);
}

#[test]
fn a_permission_holding_a_line_break_its_command_is_not_laid_out_with_shows_its_mark() {
    // #3470: a next line, a vertical tab or a line separator in a command reaches the Inbox as
    // its mark, so the window can say why only Deny is offered, as for a no-break space.
    for (c, mark) in [
        ('\u{85}', "⟦U+0085⟧"),
        ('\u{0B}', "⟦U+000B⟧"),
        ('\u{2028}', "⟦U+2028⟧"),
    ] {
        let asks = Asks::new();
        let raised = asks
            .raise("3", a_hook_ask(&format!("Run ls{c}rm")), Instant::now())
            .raised;

        let shown = permission(raised).expect("a chat of this project's");

        assert_eq!(shown.says, format!("Run ls{mark}rm"));
    }
}

#[test]
fn a_permission_prompt_s_answer_through_the_registry_closes_it_once() {
    // The path a permission ask names is the registry's own answer: the first one applies, and
    // the ask leaves the list the moment it does.
    let asks = Asks::new();
    let now = Instant::now();
    let raised = asks.raise("3", a_hook_ask("Run cargo test"), now).raised;
    let window = Answerer::admitted(Admitted::LocalUi).expect("the window answers");

    asks.answer("3", &raised.id, "allow", window, now)
        .expect("the window's answer applies");

    assert!(asks.pending(now).is_empty());
    assert!(asks.answer("3", &raised.id, "allow", window, now).is_err());
}

#[test]
fn an_edit_on_the_permission_hook_carries_its_change_to_its_row_beside_its_allow() {
    // #1146: the row draws the change before its Allow; any other ask carries none.
    let payload = serde_json::json!({
        "tool_name": "Edit",
        "tool_input": {"file_path": "/w/a.rs", "old_string": "one", "new_string": "two"}
    });
    let ask = purlis_core::harness::hooked::ask(
        purlis_core::harness::hooked::Source::ClaudeCode,
        &payload,
    );
    let asks = Asks::new();
    let raised = asks.raise("5", ask, Instant::now()).raised;

    let shown = permission(raised).expect("a chat of this project's");

    assert_eq!(shown.says, "Change /w/a.rs");
    assert_eq!(ids(&shown), ["allow", "deny"]);
    assert!(matches!(
        shown.writes,
        Some(crate::askwrites::WritesShown::Whole { ref path, ref changes })
            if path == "/w/a.rs" && changes.len() == 1
    ));
}

#[test]
fn an_edit_raised_on_the_hook_channel_is_drawn_in_its_row_and_its_allow_goes_back_on_its_hook() {
    // #1146: the whole path but the socket. A Claude Code `PermissionRequest` line for an Edit
    // of a CRLF file is heard by the project's own hearer ([`permitting`]), held, and told to
    // the window with its change drawn; the window's Allow reaches the hook that waits, and
    // the hook prints the harness's allow for the tool's own input.
    use purlis_core::harness::hooked::{Source, decision};
    use purlis_core::hookwire::permission::PermissionAsked;

    let payload = serde_json::json!({
        "session_id": "abc",
        "hook_event_name": "PermissionRequest",
        "tool_name": "Edit",
        "tool_input": {"file_path": "/w/run.bat", "old_string": "echo one\r\n",
            "new_string": "echo two\r\n"}
    });
    let plane = PlaneId::for_tests(Path::new("/p/one"));
    let hooks = Arc::new(HookAsks::new(Arc::new(Asks::new())));
    let told: Arc<Mutex<Vec<Asking>>> = Arc::default();
    let teller: Teller = Arc::new({
        let told = Arc::clone(&told);
        move |asking| told.lock().unwrap().push(asking)
    });
    let telling: Telling = Arc::new(Mutex::new(Some(teller)));
    let hear = permitting(plane.clone(), Arc::clone(&hooks), telling);

    let waiting = hear(PermissionAsked {
        chat: 5,
        permission_request: Source::ClaudeCode,
        payload: payload.clone(),
        ran_for_ms: None,
    })
    .expect("the ask is held");

    // The window was told the project's asks, this one among them, with its change drawn.
    let said = told.lock().unwrap().last().cloned().expect("told");
    assert_eq!(said, asking(&plane, &hooks));
    let [row] = said.asks.as_slice() else {
        panic!("one ask: {said:?}");
    };
    assert_eq!(row.session, 5);
    assert_eq!(row.says, "Change /w/run.bat");
    assert_eq!(ids(row), ["allow", "deny"]);
    let Some(crate::askwrites::WritesShown::Whole { path, changes }) = &row.writes else {
        panic!("shown whole: {:?}", row.writes);
    };
    assert_eq!(path, "/w/run.bat");
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].before, "echo one\u{240D}\n");
    assert_eq!(changes[0].after, "echo two\u{240D}\n");

    // The window's Allow, on the ask its row names, goes back on the hook that waits.
    let id = row.ask.parse().expect("the ask's id");
    hooks
        .answer("5", &id, "allow", the_window(), Instant::now())
        .expect("the window's answer applies");
    let chosen = waiting.answered.try_recv().expect("the hook is told");
    assert_eq!(chosen.id, "allow");
    // And what the hook prints for it lets the tool run on its own input, changed in nothing.
    let printed: serde_json::Value =
        serde_json::from_str(&decision(Source::ClaudeCode, &payload, &chosen.id).expect("one"))
            .expect("json");
    assert_eq!(
        printed["hookSpecificOutput"]["decision"],
        serde_json::json!({"behavior": "allow"})
    );
    assert!(asking(&plane, &hooks).asks.is_empty());
}

#[test]
fn a_question_on_the_permission_hook_lists_its_answers_as_the_ways_to_answer_it() {
    // #1703: Claude Code's AskUserQuestion, one question and one choice, answered in the Inbox
    // with one of its own answers, back on its hook, and never in place of its terminal row.
    let payload = serde_json::json!({
        "tool_name": "AskUserQuestion",
        "tool_input": {"questions": [{
            "question": "Which database?", "header": "DB", "multiSelect": false,
            "options": [{"label": "SQLite"}, {"label": "Postgres"}]
        }]}
    });
    let ask = purlis_core::harness::hooked::ask(
        purlis_core::harness::hooked::Source::ClaudeCode,
        &payload,
    );
    let asks = Asks::new();
    let raised = asks.raise("5", ask, Instant::now()).raised;

    let shown = permission(raised).expect("a chat of this project's");

    assert_eq!(shown.says, "Asks you: Which database?");
    assert_eq!(ids(&shown), ["answer:0", "answer:1"]);
    let labels: Vec<&str> = shown.options.iter().map(|one| one.label.as_str()).collect();
    assert_eq!(labels, ["SQLite", "Postgres"]);
    assert_eq!(shown.answer, AnswerPath::Hook);
}

// The dispatch grant adapter.

#[test]
fn a_held_dispatch_offers_its_notice_s_answers_and_names_the_dispatch_and_what_it_showed() {
    let told = a_dispatch(7, 4, "devops", vec![GrantLevel::Chat, GrantLevel::You]);

    let shown = dispatch(&told);

    assert_eq!(shown.source, AskSource::Dispatch);
    assert_eq!(shown.session, 4);
    assert_eq!(shown.says, "Wants to hand a task to devops");
    assert_eq!(ids(&shown), ["chat", "you", KEEP, NEVER]);
    assert_eq!(
        shown
            .options
            .iter()
            .map(|one| (one.label.as_str(), one.allows))
            .collect::<Vec<_>>(),
        [
            ("Allow for this chat", true),
            ("Allow for me on this machine", true),
            ("Keep blocked", false),
            ("Never for this pair", false),
        ]
    );
    assert_eq!(
        shown.answer,
        AnswerPath::Dispatch {
            id: 7,
            shown: "digest-1".to_owned()
        }
    );
}

#[test]
fn a_held_handoff_says_it_hands_the_work_off() {
    // #1471: the registry's line for a held handoff names what it is, as its Notice does.
    let mut told = a_dispatch(7, 4, "devops", vec![GrantLevel::Chat]);
    told.mode = "handoff".to_owned();

    assert_eq!(dispatch(&told).says, "Wants to hand its work off to devops");
}

#[test]
fn a_dispatch_from_a_chat_on_no_persona_offers_no_never_and_a_locked_one_offers_nothing() {
    let mut none = a_dispatch(7, 4, "devops", vec![GrantLevel::Chat]);
    none.asking = None;
    assert_eq!(ids(&dispatch(&none)), ["chat", KEEP]);

    let mut locked = a_dispatch(8, 4, "devops", Vec::new());
    locked.locked = Some("Policy forbids it.".to_owned());
    let shown = dispatch(&locked);
    assert!(shown.options.is_empty());
    assert_eq!(shown.answer, AnswerPath::InItsPane);
}

// The sandbox host adapter.

#[test]
fn a_refused_host_offers_allow_at_every_level_and_keep_blocked_bound_to_the_block_shown() {
    let shown = sandbox_block(
        5,
        &a_host_block("api.example.com"),
        &Locks::none(),
        &Default::default(),
    )
    .expect("a host is an ask");

    assert_eq!(shown.source, AskSource::SandboxHost);
    assert_eq!(shown.says, "The sandbox refused api.example.com");
    // In its Notice's order and words (#1666 N-3, #1700): this project on this machine first.
    assert_eq!(
        shown
            .options
            .iter()
            .map(|one| (one.id.as_str(), one.label.as_str(), one.allows))
            .collect::<Vec<_>>(),
        [
            ("you", "Allow for me on this machine", true),
            ("chat", "Allow only for this chat", true),
            ("project", "Allow for everyone in this project", true),
            (KEEP, "Keep blocked", false),
        ]
    );
    assert_eq!(
        shown.answer,
        AnswerPath::SandboxBlock {
            shown: BlockShown {
                operation: "connect".to_owned(),
                kind: "host".to_owned(),
                what: GrantWhat::Host,
                target: "api.example.com".to_owned(),
            }
        }
    );
}

#[test]
fn a_refused_host_is_offered_only_at_the_levels_policy_leaves_open() {
    let locks = Locks::parse(
        r#"{"sandbox": {"personal-hosts": false}}"#,
        Path::new("/etc/purlis/policy.json"),
    );

    let shown = sandbox_block(
        5,
        &a_host_block("api.example.com"),
        &locks,
        &Default::default(),
    )
    .expect("an ask");

    assert_eq!(ids(&shown), ["project", KEEP]);
}

#[test]
fn a_host_the_block_did_not_name_is_answered_in_its_chat() {
    let unnamed = sandbox_block(
        5,
        &HeldBlock::unnamed_host("connect", "host"),
        &Locks::none(),
        &Default::default(),
    )
    .expect("still an ask");
    // Answered in its chat, where the host is typed; its Notice draws the registry's words for
    // that Allow all the same (#1700), so its options are the words and nothing sends them.
    assert_eq!(unnamed.answer, AnswerPath::InItsPane);
    assert_eq!(
        unnamed
            .options
            .iter()
            .map(|one| one.label.as_str())
            .collect::<Vec<_>>(),
        [
            "Allow for me on this machine",
            "Allow only for this chat",
            "Allow for everyone in this project",
            "Keep blocked",
        ]
    );
}

fn a_write_block(folder: &str) -> HeldBlock {
    HeldBlock {
        operation: "write".to_owned(),
        kind: "project-files".to_owned(),
        what: GrantWhat::Write,
        target: folder.to_owned(),
    }
}

#[test]
fn a_refused_folder_write_offers_its_notice_s_allow_and_keep_blocked_bound_to_the_block_shown() {
    // D-1700-1: a folder's block offers Allow and Keep blocked on its Notice, so it is an ask,
    // answered by the same commands; a folder is never offered for everyone in the project.
    let shown = sandbox_block(
        5,
        &a_write_block("/w/out"),
        &Locks::none(),
        &Default::default(),
    )
    .expect("an ask");

    // A source word of its own (#1700): a folder is no host.
    assert_eq!(shown.source, AskSource::SandboxWrite);
    assert_eq!(shown.says, "The sandbox refused a write in /w/out");
    assert_eq!(
        shown
            .options
            .iter()
            .map(|one| (one.id.as_str(), one.label.as_str()))
            .collect::<Vec<_>>(),
        [
            ("chat", "Allow only for this chat"),
            ("you", "Allow for me on this machine"),
            (KEEP, "Keep blocked"),
        ]
    );
    assert_eq!(shown.ask, "block:5:write:project-files:/w/out");
    assert_eq!(
        shown.answer,
        AnswerPath::SandboxBlock {
            shown: BlockShown {
                operation: "write".to_owned(),
                kind: "project-files".to_owned(),
                what: GrantWhat::Write,
                target: "/w/out".to_owned(),
            }
        }
    );
}

fn a_local_block(what: GrantWhat, kind: &str, target: &str) -> HeldBlock {
    HeldBlock {
        operation: if what == GrantWhat::Socket {
            "connect"
        } else {
            "lookup"
        }
        .to_owned(),
        kind: kind.to_owned(),
        what,
        target: target.to_owned(),
    }
}

/// A container engine's socket, and a service purlis cannot tell, are asked about with the
/// narrowest answer first, then every project on this machine (R2, 2026-10-13), never for
/// everyone in the project, and policy's scopes have the last word.
#[test]
fn a_socket_or_a_service_is_asked_with_once_first_and_never_for_the_project() {
    let ids =
        |shown: &Shown| -> Vec<String> { shown.options.iter().map(|one| one.id.clone()).collect() };
    for other in [
        a_local_block(GrantWhat::Socket, "local-socket", "/var/run/docker.sock"),
        a_local_block(GrantWhat::Service, "system-service", "com.example.helper"),
    ] {
        let shown = sandbox_block(5, &other, &Locks::none(), &Default::default()).expect("an ask");
        assert_eq!(shown.source, AskSource::SandboxLocal);
        assert_eq!(
            ids(&shown),
            ["once", "machine", "chat", "you", KEEP],
            "{other:?}"
        );
        assert_eq!(
            shown.options[1].label,
            "Allow on this machine, for every project"
        );
    }
    let locks = Locks::parse(
        r#"{"sandbox": {"allow-scopes": ["chat"]}}"#,
        Path::new("/etc/purlis/policy.json"),
    );
    let narrowed = sandbox_block(
        5,
        &a_local_block(GrantWhat::Service, "system-service", "com.example.helper"),
        &locks,
        &Default::default(),
    )
    .expect("an ask");
    assert_eq!(ids(&narrowed), ["chat", KEEP]);
}

/// ADR 0093 (D-sock-4): the Inbox's ask about a socket or a service the project declares says
/// "This project expects it", as the block's own Notice does (#1888); one it does not declare
/// says only what allowing it means.
#[test]
fn the_inbox_says_this_project_expects_a_socket_or_a_service_it_declares() {
    let mut world = World::new();
    world.declared = purlis_core::sandbox::services::Declared {
        sockets: vec!["/var/run/docker.sock".into()],
        services: vec!["com.example.helper".to_owned()],
    };
    world.blocks = vec![
        (
            5,
            a_local_block(GrantWhat::Socket, "local-socket", "/var/run/docker.sock"),
        ),
        (
            6,
            a_local_block(GrantWhat::Service, "system-service", "com.example.helper"),
        ),
        (
            7,
            a_local_block(GrantWhat::Service, "system-service", "com.example.other"),
        ),
    ];
    let says: Vec<String> = world.asks().into_iter().map(|one| one.says).collect();
    assert_eq!(
        says,
        [
            "The sandbox refused the local socket /var/run/docker.sock. This project expects \
             it. Allowing it lets this chat control containers, which can reach your whole \
             machine: about the same as no sandbox.",
            "The sandbox refused the system service com.example.helper. This project expects \
             it. It is a service on this machine; purlis can't tell what it does.",
            "The sandbox refused the system service com.example.other. It is a service on this \
             machine; purlis can't tell what it does.",
        ]
    );
}

#[test]
fn what_an_ask_names_is_said_with_every_character_that_draws_as_nothing_written_out() {
    // A folder's name, or a chat's, is drawn in the Inbox and in a notification's group: one
    // that turns the text around or hides part of it reads as something else, so each such
    // character is written out as its escape and the rest is said as it is.
    let sly = a_write_block("/w/gpj.\u{202e}exe\u{200b}");
    let shown = sandbox_block(5, &sly, &Locks::none(), &Default::default()).expect("an ask");
    assert_eq!(
        shown.says,
        "The sandbox refused a write in /w/gpj.⟦U+202E⟧exe⟦U+200B⟧"
    );
    let mut world = World::new();
    world.blocks = vec![(5, sly)];
    world.names = vec![(5, "pay\u{202e}lanigiro")];
    assert_eq!(world.asks()[0].chain, ["pay⟦U+202E⟧lanigiro"]);
}

#[test]
fn a_folder_write_is_offered_only_at_the_levels_policy_leaves_open() {
    let locks = Locks::parse(
        r#"{"sandbox": {"allow-scopes": ["you"]}}"#,
        Path::new("/etc/purlis/policy.json"),
    );

    let shown =
        sandbox_block(5, &a_write_block("/w/out"), &locks, &Default::default()).expect("an ask");

    assert_eq!(ids(&shown), ["you", KEEP]);
}

// The harness-terminal and question adapters, and the list as a whole.

#[test]
fn a_chat_in_the_queue_is_a_terminal_prompt_where_it_stopped_on_one_and_a_question_otherwise() {
    let mut world = World::new();
    world.queue = vec![2, 3];
    world.at_prompt = vec![2];

    let asks = world.asks();

    assert_eq!(
        asks.iter()
            .map(|ask| (ask.session, ask.source, ask.options.len()))
            .collect::<Vec<_>>(),
        [(2, AskSource::Terminal, 0), (3, AskSource::Question, 0)]
    );
    assert!(asks.iter().all(|ask| ask.answer == AnswerPath::InItsPane));
    assert_eq!(asks[0].says, "Waiting in its terminal");
}

#[test]
fn a_chat_in_the_queue_only_for_tasks_that_came_to_nothing_asks_nothing() {
    // #1693 (D-1690-6): a failed task is an update in the Inbox, not an ask, so a chat whose
    // only reason to be in the queue is a failure below it is not counted. One that also waits
    // on the person's reply still is.
    let mut world = World::new();
    world.queue = vec![2, 3, 4];
    world.at_prompt = vec![4];
    world.only_updates = vec![2];

    let asks = world.asks();

    assert_eq!(
        asks.iter()
            .map(|ask| (ask.session, ask.source))
            .collect::<Vec<_>>(),
        [(3, AskSource::Question), (4, AskSource::Terminal)]
    );
}

#[test]
fn a_terminal_prompt_says_which_prompt_waits_there_and_is_answered_only_in_its_chat() {
    // #1691: what the harness's nudge named, so the person knows what they go to the chat
    // for; nothing the window could send answers it, so no chat can answer it either (V16a).
    let mut world = World::new();
    world.queue = vec![2, 3];
    world.prompts = vec![(2, Prompt::Permission), (3, Prompt::Form)];

    let asks = world.asks();

    let said: Vec<(u32, &str)> = asks
        .iter()
        .map(|ask| (ask.session, ask.says.as_str()))
        .collect();
    assert_eq!(
        said,
        [
            (2, "Waiting in its terminal: a permission prompt"),
            (3, "Waiting in its terminal: a form to fill"),
        ]
    );
    for ask in &asks {
        assert_eq!(ask.source, AskSource::Terminal);
        assert!(ask.options.is_empty());
        assert_eq!(ask.answer, AnswerPath::InItsPane);
    }
}

#[test]
fn a_chat_waiting_on_a_structured_ask_is_one_ask_not_two() {
    // A chat stopped on a permission prompt purlis holds is in the queue too, and a chat whose
    // dispatch is held waits on that: each is counted once, as the ask that says why.
    let asks_held = Asks::new();
    let mut world = World::new();
    world.permissions = vec![
        asks_held
            .raise("2", a_hook_ask("Run ls"), Instant::now())
            .raised,
    ];
    world.dispatches = vec![a_dispatch(1, 3, "devops", vec![GrantLevel::Chat])];
    world.queue = vec![2, 3, 4];
    world.at_prompt = vec![2];

    let asks = world.asks();

    assert_eq!(
        asks.iter()
            .map(|ask| (ask.session, ask.source))
            .collect::<Vec<_>>(),
        [
            (2, AskSource::Permission),
            (3, AskSource::Dispatch),
            (4, AskSource::Question)
        ]
    );
}

#[test]
fn a_sandbox_host_ask_counts_beside_whatever_else_its_chat_waits_on() {
    // A refused host does not stop the chat's turn, so it is an ask of its own.
    let mut world = World::new();
    world.queue = vec![5];
    world.blocks = vec![
        (5, a_host_block("api.example.com")),
        (
            6,
            HeldBlock {
                operation: "write".to_owned(),
                kind: "project-files".to_owned(),
                what: GrantWhat::Write,
                target: "/w".to_owned(),
            },
        ),
    ];

    let sources: Vec<_> = world
        .asks()
        .iter()
        .map(|ask| (ask.session, ask.source))
        .collect();

    assert_eq!(
        sources,
        [
            (5, AskSource::Question),
            (5, AskSource::SandboxHost),
            (6, AskSource::SandboxWrite)
        ]
    );
}

#[test]
fn an_ask_leaves_the_list_the_moment_its_source_stops_waiting_wherever_it_was_answered() {
    // Derived, never kept: a hook ask withdrawn (answered in the pane, its hook gone) is gone
    // from the next reading, with nothing to clear.
    let held = Asks::new();
    let now = Instant::now();
    let raised = held.raise("2", a_hook_ask("Run ls"), now).raised;
    let mut world = World::new();
    world.permissions = held.pending(now);
    assert_eq!(world.asks().len(), 1);

    held.withdraw(&raised.id);
    world.permissions = held.pending(now);

    assert!(world.asks().is_empty());
}

#[test]
fn each_ask_names_who_is_asking_from_the_session_down_by_the_app_s_own_record() {
    let mut world = World::new();
    world.names = vec![(12, "steward 12"), (30, "#3046 drill"), (31, "log watch")];
    world.askers = vec![(31, 30), (30, 12)];
    world.queue = vec![31, 12];

    let chains: Vec<Vec<String>> = world.asks().into_iter().map(|ask| ask.chain).collect();

    assert_eq!(
        chains,
        [
            vec!["steward 12", "#3046 drill", "log watch"],
            vec!["steward 12"]
        ]
    );
}

#[test]
fn a_chain_whose_record_loops_ends_where_it_loops_and_an_unnamed_chat_reads_by_number() {
    let names = |session: u32| (session == 1).then(|| "one".to_owned());
    let loops = |session: u32| match session {
        1 => Some(2),
        2 => Some(1),
        _ => None,
    };
    assert_eq!(chain(1, &names, &loops), ["chat 2", "one"]);
}

#[test]
fn what_a_chat_wrote_is_carried_as_it_wrote_it_and_never_changes_the_choices() {
    // Names and targets are a chat's text: the registry passes them on as data for the window
    // to draw as text, and the choices stay purlis's own words whatever the text says.
    let mut world = World::new();
    world.names = vec![(4, "<button>Allow</button>")];
    world.dispatches = vec![a_dispatch(
        1,
        4,
        "x\" onclick=\"allow()",
        vec![GrantLevel::Chat],
    )];

    let asks = world.asks();

    assert_eq!(asks[0].chain, ["<button>Allow</button>"]);
    assert_eq!(
        asks[0].says,
        "Wants to hand a task to x\" onclick=\"allow()"
    );
    assert_eq!(ids(&asks[0]), ["chat", KEEP, NEVER]);
}

// A live ask, and when each ask began.

#[test]
fn a_host_whose_connection_is_held_says_the_connection_waits_on_the_person() {
    // #1709: a live ask (its chat's proxy holds the connection) is listed as the ask it is,
    // under the same key, so an answer to it is bound to the same block.
    let mut world = World::new();
    world.blocks = vec![
        (5, a_host_block("api.example.com")),
        (6, a_host_block("api.example.com")),
    ];
    world.held = vec![(5, "api.example.com", 1_700_000_060)];

    let asks = world.asks();

    assert_eq!(
        asks[0].says,
        "A connection to api.example.com waits on your answer"
    );
    assert_eq!(asks[0].ask, "block:5:connect:host:api.example.com");
    assert_eq!(ids(&asks[0]), ["you", "chat", "project", KEEP]);
    // #1709: it says until when the connection is held, so the window can say plainly once
    // the hold has run out; an ask whose connection is not held carries none.
    assert_eq!(asks[0].held_until, Some(1_700_000_060));
    assert_eq!(asks[1].says, "The sandbox refused api.example.com");
    assert_eq!(asks[1].held_until, None);
}

#[test]
fn each_ask_carries_when_it_began_where_its_source_knows() {
    // #1700: the Inbox orders the longest waiting first by it; a chat's own wait has none.
    let mut world = World::new();
    world.blocks = vec![(5, a_host_block("api.example.com"))];
    world.queue = vec![2];
    world.since = vec![("block:5:connect:host:api.example.com", 1_700_000_000)];

    let asks = world.asks();

    assert_eq!(
        asks.iter()
            .map(|ask| (ask.ask.as_str(), ask.since))
            .collect::<Vec<_>>(),
        [
            ("question:2", None),
            ("block:5:connect:host:api.example.com", Some(1_700_000_000)),
        ]
    );
}

// What is an update, and when a hold ends.

#[test]
fn a_chat_in_the_queue_for_the_app_s_reasons_alone_is_no_ask() {
    // #1694, #1700 (I-1): a report with nowhere to go and a commit refused are updates in the
    // Inbox; a chat in the queue for those alone, and not itself waiting on the person, asks
    // nothing. One whose own turn ended on the person, or that stopped on a prompt in its
    // terminal, still asks, whatever else it is in the queue for.
    let quiet = InQueue {
        failed: false,
        for_itself: true,
        reasons: 1,
        waiting: false,
        at_its_prompt: false,
    };
    assert!(only_updates(quiet));
    assert!(!only_updates(InQueue {
        waiting: true,
        ..quiet
    }));
    assert!(!only_updates(InQueue {
        at_its_prompt: true,
        ..quiet
    }));
    assert!(!only_updates(InQueue {
        reasons: 0,
        ..quiet
    }));
    // A task of its that came to nothing, and nothing of its own (#1693).
    assert!(only_updates(InQueue {
        failed: true,
        for_itself: false,
        reasons: 0,
        waiting: true,
        at_its_prompt: false,
    }));
}

#[test]
fn a_hold_ends_a_minute_after_it_began_and_one_past_that_is_looked_at_again_soon() {
    // #1709: the hold is the proxy's minute from when the block was first heard. A block heard
    // before its connection was held is still held past that: the window looks again shortly.
    let hold = purlis_core::sandbox::asks::HOLD.as_secs();
    assert_eq!(hold_ends(Some(1_000), 1_010), 1_000 + hold);
    assert_eq!(
        hold_ends(Some(1_000), 1_000 + hold),
        1_000 + hold + LOOK_AGAIN
    );
    assert_eq!(hold_ends(None, 2_000), 2_000 + hold);
}

// A workspace a chat asked the person to remove (qw356).

fn a_removal(session: u32, workspace: &str) -> crate::removalasks::Waits {
    crate::removalasks::Waits {
        session,
        workspace: workspace.to_owned(),
        since: 40,
    }
}

#[test]
fn a_workspace_a_chat_asked_to_remove_is_an_ask_with_remove_and_keep() {
    let holds = purlis_core::removalask::Holds {
        memories: 12,
        todos: 3,
    };

    let shown = workspace_removal(&a_removal(3, "boot-migration-safety"), holds);

    assert_eq!(shown.source, AskSource::WorkspaceRemoval);
    assert_eq!(shown.session, 3);
    assert_eq!(shown.ask, "removal:3:boot-migration-safety");
    assert_eq!(
        shown.says,
        "Removing workspace boot-migration-safety deletes its memory, its todos and its clones"
    );
    // Remove… is the one that leads to something happening, so it waits to settle as an Allow
    // does; it opens a dialog, and is spelled so.
    assert_eq!(ids(&shown), ["remove", "keep"]);
    assert_eq!(
        shown
            .options
            .iter()
            .map(|one| (one.label.as_str(), one.allows))
            .collect::<Vec<_>>(),
        [("Remove…", true), ("Keep", false)]
    );
    assert_eq!(
        shown.answer,
        AnswerPath::WorkspaceRemoval {
            workspace: "boot-migration-safety".to_owned(),
            memories: 12,
            todos: 3,
        }
    );
}

#[test]
fn a_chat_waiting_on_a_removal_s_answer_is_that_ask_and_no_question_beside_it() {
    // Safe remove's chat ends its turn once it has asked: it is in the queue, and what it
    // waits on is the removal, listed once, since when it was asked.
    let mut world = World::new();
    world.queue = vec![3, 4];
    world.names = vec![(3, "Safe remove · alpha")];
    world.removals = vec![(a_removal(3, "alpha"), Default::default())];
    world.since = vec![("removal:3:alpha", 40)];

    let asks = world.asks();

    let sources: Vec<(u32, AskSource)> = asks.iter().map(|ask| (ask.session, ask.source)).collect();
    assert_eq!(
        sources,
        [(3, AskSource::WorkspaceRemoval), (4, AskSource::Question)]
    );
    assert_eq!(asks[0].chain, ["Safe remove · alpha"]);
    assert_eq!(asks[0].since, Some(40));
}

// Who may answer (V16).

#[test]
fn every_command_an_ask_is_answered_by_is_the_window_s_alone() {
    // No chat, and no client but the window, reaches any of them: an ask is never answered by
    // the chat that raised it, through the registry or around it.
    for command in [
        "answer_ask",
        "allow_dispatch",
        "keep_dispatch_blocked",
        "never_dispatch",
        "allow_sandbox_block",
        "keep_sandbox_block",
        "forget_sandbox_block",
        "keep_asked_workspace",
        "asks_waiting",
    ] {
        assert!(
            purlis_session_protocol::ui::WINDOW_ONLY.contains(&command),
            "{command} is the window's alone"
        );
    }
    assert_eq!(Answerer::admitted(Admitted::Chat), None);
}

#[test]
fn a_refused_port_on_this_mac_offers_this_chat_and_this_project_here_bound_to_the_block_shown() {
    // qw384: one port, never once or everyone's; one the block did not name is typed in its
    // Notice and answered there.
    let block = |target: &str| HeldBlock {
        operation: "connect".to_owned(),
        kind: "local-service".to_owned(),
        what: GrantWhat::Port,
        target: target.to_owned(),
    };
    let shown =
        sandbox_block(5, &block("50795"), &Locks::none(), &Default::default()).expect("an ask");
    assert_eq!(shown.source, AskSource::SandboxLocal);
    assert!(
        shown
            .says
            .starts_with("The sandbox refused the service on this Mac at port 50795.")
    );
    assert_eq!(
        shown
            .options
            .iter()
            .map(|one| (one.id.as_str(), one.label.as_str()))
            .collect::<Vec<_>>(),
        [
            ("chat", "Allow only for this chat"),
            ("you", "Allow for me on this machine"),
            (KEEP, "Keep blocked"),
        ]
    );
    assert_eq!(
        shown.answer,
        AnswerPath::SandboxBlock {
            shown: BlockShown {
                operation: "connect".to_owned(),
                kind: "local-service".to_owned(),
                what: GrantWhat::Port,
                target: "50795".to_owned(),
            }
        }
    );
    let typed = sandbox_block(5, &block(""), &Locks::none(), &Default::default()).expect("an ask");
    assert_eq!(typed.answer, AnswerPath::InItsPane);
}

// The sign-in adapter (#2035).

/// A profile of `kind` named `work`, a start was refused on in `/p`.
fn waits_to_sign_in(kind: &str, harness: &str) -> purlis_core::profilestore::sign_in::Wait {
    purlis_core::profilestore::sign_in::Wait {
        root: "/p".into(),
        profile: purlis_core::profiles::Profile {
            name: "work".to_owned(),
            kind: kind.to_owned(),
            harness: kind.to_owned(),
            command: vec![kind.to_owned()],
            env: Vec::new(),
            source: purlis_core::profiles::Source::Machine,
        },
        harness: harness.to_owned(),
        since: 7,
    }
}

#[test]
fn a_profile_not_signed_in_is_one_ask_about_no_chat_that_signs_it_in() {
    let mut world = World::new();
    world.queue = vec![3];
    world.names = vec![(3, "steward 3")];
    world.sign_ins = vec![waits_to_sign_in("claude", "Claude Code")];
    world.since = vec![("sign-in:work", 7)];

    let asks = world.asks();
    let ask = asks
        .iter()
        .find(|ask| ask.source == AskSource::SignIn)
        .expect("the profile asks");

    assert_eq!(ask.session, 0);
    assert_eq!(ask.chain, ["Profile work"]);
    assert_eq!(ids(ask), ["sign-in", "dismiss"]);
    assert_eq!(
        ask.answer,
        AnswerPath::SignIn {
            profile: "work".to_owned(),
            pool: None,
        }
    );
    assert!(
        ask.says
            .contains("Claude Code's own sign-in in a shell tab"),
        "{}",
        ask.says
    );
    assert_eq!(ask.since, Some(7));
    assert!(
        asks.iter()
            .any(|one| one.session == 3 && one.chain == ["steward 3"]),
        "the chats keep their own chains"
    );
}

/// **A profile not signed in on a pool is one Ask to sign it in there** (#2049), about no chat,
/// beside the same profile's Ask for this machine, each answered on its own device.
#[test]
fn a_profile_not_signed_in_on_a_pool_is_one_ask_to_sign_it_in_on_that_pool() {
    let mut world = World::new();
    world.sign_ins = vec![waits_to_sign_in("claude", "Claude Code")];
    world.pool_sign_ins = vec![purlis_core::profilestore::onrunner::PoolWait {
        root: "/p".into(),
        pool: "builds".to_owned(),
        profile: "work".to_owned(),
        kind: "claude".to_owned(),
        harness: "Claude Code".to_owned(),
        since: 9,
    }];
    world.since = vec![("sign-in:work:on:builds", 9)];

    let asks = world.asks();
    let on_pool: Vec<_> = asks
        .iter()
        .filter(|ask| {
            ask.answer
                != AnswerPath::SignIn {
                    profile: "work".to_owned(),
                    pool: None,
                }
        })
        .filter(|ask| ask.source == AskSource::SignIn)
        .collect();

    assert_eq!(on_pool.len(), 1);
    let ask = on_pool[0];
    assert_eq!(ask.session, 0);
    assert_eq!(ask.chain, ["Profile work on pool builds"]);
    assert_eq!(ids(ask), ["sign-in", "dismiss"]);
    assert_eq!(
        ask.answer,
        AnswerPath::SignIn {
            profile: "work".to_owned(),
            pool: Some("builds".to_owned()),
        }
    );
    assert!(
        ask.says.contains("not signed in on pool builds"),
        "{}",
        ask.says
    );
    assert!(
        ask.says
            .contains("nothing of this machine's sign-in goes there"),
        "{}",
        ask.says
    );
    assert_eq!(ask.since, Some(9));
}

#[test]
fn an_opencode_profile_is_signed_in_to_a_plan_named_and_never_to_claude() {
    let ask = sign_in(&waits_to_sign_in("opencode", "opencode"));

    assert_eq!(
        ids(&ask),
        ["sign-in:openai", "sign-in:github-copilot", "dismiss"]
    );
    assert!(ask.options.iter().all(|one| !one.label.contains("Claude")));
}

#[test]
fn a_sandboxed_codex_chat_without_a_sandbox_only_profile_is_one_ask_that_makes_one() {
    // #2037, the operator's ruling of 2026-10-15.
    let mut world = World::new();
    world.queue = vec![3];
    world.names = vec![(3, "steward 3")];
    world.sandbox_only = Some(purlis_core::profilestore::sandbox_only::Wait {
        root: std::path::PathBuf::from("/p"),
        since: 9,
    });
    world.since = vec![("sandbox-only:codex", 9)];

    let asks = world.asks();
    let ask = asks
        .iter()
        .find(|ask| ask.source == AskSource::SandboxOnly)
        .expect("the project asks");

    assert_eq!(ask.session, 0);
    assert_eq!(ask.chain, ["Codex"]);
    assert_eq!(ids(ask), ["make", "dismiss"]);
    assert_eq!(ask.answer, AnswerPath::SandboxOnly);
    assert!(
        ask.says
            .starts_with("Codex needs a sandbox-only profile here"),
        "{}",
        ask.says
    );
    assert_eq!(ask.since, Some(9));
    assert!(
        asks.iter()
            .any(|one| one.session == 3 && one.chain == ["steward 3"]),
        "the chats keep their own chains"
    );
}

#[test]
fn a_chats_first_browser_call_is_one_ask_of_what_its_browser_may_reach() {
    // #2013: the first browser in a chat raises one Ask that sets its posture.
    let mut world = World::new();
    world.queue = vec![8];
    world.names = vec![(8, "steward 8")];
    world.browsers = vec![crate::chatbrowsers::Waits {
        session: 8,
        since: 40,
        as_persona: crate::chatbrowsers::AsPersona::default(),
        unsandboxed: false,
    }];
    world.since = vec![("browser:8", 40)];

    let asks = world.asks();

    assert_eq!(
        asks.len(),
        1,
        "its wait is that Ask, and no question beside it"
    );
    let ask = &asks[0];
    assert_eq!(ask.source, AskSource::Browser);
    assert_eq!(ask.answer, AnswerPath::Browser);
    assert_eq!(ask.chain, ["steward 8"]);
    assert_eq!(ids(ask), ["project", "open-web", "keep"]);
    assert_eq!(
        ask.options
            .iter()
            .map(|one| (one.label.as_str(), one.allows))
            .collect::<Vec<_>>(),
        [
            ("This project's sites", true),
            ("Also the open web", true),
            ("Keep off", false),
        ]
    );
    assert_eq!(ask.since, Some(40));
}

#[test]
fn a_chat_without_the_sandbox_is_told_what_each_answer_gives_its_browser() {
    // #2242: its browser is held to what a sandboxed chat of its persona lists, and the Ask
    // says so; the answers are the same, in the same place.
    let mut world = World::new();
    world.queue = vec![8];
    world.names = vec![(8, "qa 8")];
    world.browsers = vec![crate::chatbrowsers::Waits {
        session: 8,
        since: 40,
        as_persona: crate::chatbrowsers::AsPersona {
            persona: Some("qa".to_owned()),
            ..crate::chatbrowsers::AsPersona::default()
        },
        unsandboxed: true,
    }];
    world.since = vec![("browser:8", 40)];

    let ask = world.asks().remove(0);

    assert_eq!(
        ask.says,
        "Wants a browser. This chat has no sandbox, but its browser does: this project's sites \
         are the hosts a sandboxed qa chat lists and the services on this machine you allowed"
    );
    assert_eq!(&ids(&ask)[..2], ["project", "open-web"]);
}

/// A chat as `persona`, declaring `declared`, waiting on its browser Ask (#2241).
fn browser_waits(
    world: &mut World,
    persona: &str,
    declared: Option<purlis_core::sandbox::persona::Browser>,
) {
    world.queue = vec![8];
    world.names = vec![(8, "qa 8")];
    world.browsers = vec![crate::chatbrowsers::Waits {
        session: 8,
        since: 40,
        as_persona: crate::chatbrowsers::AsPersona {
            persona: Some(persona.to_owned()),
            declared,
            always: None,
        },
        unsandboxed: false,
    }];
    world.since = vec![("browser:8", 40)];
}

#[test]
fn a_persona_chats_browser_ask_offers_always_for_this_persona_beside_each_reach() {
    // #2241: Allow for this chat, Always for this persona, Keep off.
    let mut world = World::new();
    browser_waits(&mut world, "qa", None);

    let ask = world.asks().remove(0);

    assert_eq!(
        ids(&ask),
        [
            "project",
            "open-web",
            "project-always",
            "open-web-always",
            "keep"
        ]
    );
    assert_eq!(ask.options[2].label, "This project's sites, always for qa");
    assert_eq!(ask.options[3].label, "Also the open web, always for qa");
    assert!(ask.options.iter().all(|one| one.preselected.is_none()));
}

#[test]
fn a_persona_s_declared_browser_is_marked_in_place_and_still_asked() {
    // #2241, ADR 0073: the declaration marks its answer for this chat, by its label, and the
    // person answers; its Always beside it is not marked.
    use purlis_core::sandbox::persona::Browser;
    let mut world = World::new();
    browser_waits(&mut world, "qa", Some(Browser::OpenWeb));

    let ask = world.asks().remove(0);

    let marked: Vec<(&str, &str)> = ask
        .options
        .iter()
        .filter(|one| one.preselected == Some(true))
        .map(|one| (one.id.as_str(), one.label.as_str()))
        .collect();
    assert_eq!(marked, [("open-web", "Also the open web (qa uses this)")]);
    assert_eq!(ask.options[3].label, "Also the open web, always for qa");
    assert!(
        ask.says
            .ends_with("Its persona qa declares the open web too"),
        "{}",
        ask.says
    );
}

#[test]
fn the_browser_asks_answers_keep_their_places_whatever_a_persona_declares() {
    // #2241: the same key presses the same answer: the order, and the first answer (where the
    // keyboard lands), never depend on what is declared.
    use purlis_core::sandbox::persona::Browser;
    let order = |declared: Option<Browser>| {
        let mut world = World::new();
        browser_waits(&mut world, "qa", declared);
        ids(&world.asks().remove(0))
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    let undeclared = order(None);
    assert_eq!(
        undeclared,
        [
            "project",
            "open-web",
            "project-always",
            "open-web-always",
            "keep"
        ]
    );
    assert_eq!(order(Some(Browser::Project)), undeclared);
    assert_eq!(order(Some(Browser::OpenWeb)), undeclared);
}

#[test]
fn an_administrators_policy_forbids_always_for_this_persona_on_the_browser_ask() {
    // #2241: `allow-scopes` without `you` removes Always; a declaration preselects the Allow
    // for this chat instead.
    use purlis_core::sandbox::persona::Browser;
    let mut world = World::new();
    browser_waits(&mut world, "qa", Some(Browser::Project));
    world.locks = Locks::parse(
        r#"{"sandbox": {"allow-scopes": ["chat", "project"]}}"#,
        std::path::Path::new("/etc/purlis/policy.json"),
    );

    let ask = world.asks().remove(0);

    assert_eq!(ids(&ask), ["project", "open-web", "keep"]);
    assert_eq!(ask.options[0].preselected, Some(true));
    assert_eq!(ask.options[0].label, "This project's sites (qa uses this)");
}

#[test]
fn a_browser_that_needs_the_person_is_an_ask_whose_one_answer_takes_it_over() {
    // #2015: its source is the browser, its words the agent's, marked as the chat's, and its
    // one answer opens the browser taken over.
    let mut world = World::new();
    world.queue = vec![8];
    world.names = vec![(8, "qa 8")];
    world.needs_you = vec![crate::browserneedsyou::Waiting {
        session: 8,
        browser: "b1".into(),
        program: "01J9BRWSR000000000000000P1".into(),
        question: "The page asks for the letters in the picture".into(),
        since: 50,
        taken: false,
    }];
    world.since = vec![("browser-needs-you:01J9BRWSR000000000000000P1", 50)];

    let asks = world.asks();

    assert_eq!(
        asks.len(),
        1,
        "its wait is that Ask, and no question beside it"
    );
    let ask = &asks[0];
    assert_eq!(ask.source, AskSource::BrowserNeedsYou);
    assert_eq!(
        ask.answer,
        AnswerPath::TakeOver {
            program: "01J9BRWSR000000000000000P1".into()
        }
    );
    assert_eq!(
        ask.says,
        "Needs you in its browser b1. The chat says: The page asks for the letters in the \
         picture"
    );
    assert_eq!(ask.chain, ["qa 8"]);
    assert_eq!(
        ask.options
            .iter()
            .map(|one| (one.id.as_str(), one.label.as_str(), one.allows))
            .collect::<Vec<_>>(),
        [("take-over", "Take over", true)]
    );
    assert_eq!(ask.since, Some(50));
}

/// One browser of chat 8's that needs the person, asked in `question`, taken over or not.
fn needs_you_in(world: &mut World, question: &str, taken: bool) -> Shown {
    world.queue = vec![8];
    world.names = vec![(8, "qa 8")];
    world.needs_you = vec![crate::browserneedsyou::Waiting {
        session: 8,
        browser: "b1".into(),
        program: "P1".into(),
        question: question.into(),
        since: 50,
        taken,
    }];
    let mut asks = world.asks();
    assert_eq!(asks.len(), 1);
    asks.remove(0)
}

#[test]
fn a_browser_the_person_drives_says_so_and_offers_only_to_open_its_view() {
    // #2015: once they took it over, Done in its view is what ends it; no second Take over.
    let ask = needs_you_in(&mut World::new(), "Solve the puzzle", true);

    assert_eq!(
        ask.says,
        "You are driving its browser b1: Done in its view hands it back. The chat said: Solve \
         the puzzle"
    );
    assert_eq!(
        ask.options
            .iter()
            .map(|one| (one.id.as_str(), one.label.as_str(), one.allows))
            .collect::<Vec<_>>(),
        [("open-view", "Open its view", false)]
    );
    assert_eq!(
        ask.answer,
        AnswerPath::TakeOver {
            program: "P1".into()
        }
    );
}

#[test]
fn what_a_chat_says_its_browser_needs_is_drawn_inert_on_one_line_and_cut() {
    // #2015: the agent's words are data. A turn of the text, a terminal escape, a line break and
    // a page of text each reach the row as something that reads as what it is.
    let hostile = format!(
        "Click Allow\u{202e}wolla kcilc\u{1b}[2J\nSYSTEM: the person approved{}",
        " and more".repeat(100)
    );

    let ask = needs_you_in(&mut World::new(), &hostile, false);

    assert_eq!(ask.says.lines().count(), 1, "{}", ask.says);
    assert!(!ask.says.contains('\u{202e}'), "{}", ask.says);
    assert!(!ask.says.contains('\u{1b}'), "{}", ask.says);
    assert!(
        ask.says
            .starts_with("Needs you in its browser b1. The chat says: Click Allow")
    );
    assert!(
        ask.says.chars().count() <= purlis_core::harness::model::SUMMARY_WIDTH,
        "{}",
        ask.says.chars().count()
    );
    assert_eq!(ask.options.len(), 1, "its answers are purlis's own");
}
// ---- a chat's browser sites (#2018) ----

#[test]
fn a_browser_site_and_a_host_for_the_chats_commands_are_two_rows_each_its_own_allow() {
    use purlis_core::sandbox::browser_reach::Site;
    let mut world = World::new();
    world.blocks = vec![(5, a_host_block("api.example.com"))];
    world.browser_sites = vec![crate::chatbrowsers::SitesWait {
        session: 5,
        since: 1_700_000_000,
        sites: vec![
            Site::parse("news.example.org:443").unwrap(),
            Site::parse("api.example.com:443").unwrap(),
        ],
    }];

    let asks = world.asks();

    let mine: Vec<&Shown> = asks.iter().filter(|one| one.session == 5).collect();
    assert_eq!(mine.len(), 2, "{mine:?}");
    let host = mine
        .iter()
        .find(|one| one.source == AskSource::SandboxHost)
        .unwrap();
    let sites = mine
        .iter()
        .find(|one| one.source == AskSource::BrowserSite)
        .unwrap();
    assert!(host.says.contains("api.example.com"), "{}", host.says);
    assert_eq!(
        sites.says,
        "Its browser wants to reach news.example.org:443 and api.example.com:443. An Allow is \
         its browser's alone, from its next act; nothing restarts."
    );
    assert_eq!(ids(sites), ["chat", "you", "project", KEEP]);
    assert_eq!(
        sites.answer,
        AnswerPath::BrowserSites {
            sites: vec![
                "news.example.org:443".to_owned(),
                "api.example.com:443".to_owned()
            ]
        }
    );
    assert_ne!(
        host.answer, sites.answer,
        "each is answered by its own Allow"
    );
}

#[test]
fn one_browser_site_is_named_as_one_site_its_browser_reaches_from_its_next_step() {
    use purlis_core::sandbox::browser_reach::Site;
    let mut world = World::new();
    world.browser_sites = vec![crate::chatbrowsers::SitesWait {
        session: 5,
        since: 1_700_000_000,
        sites: vec![Site::parse("news.example.org:443").unwrap()],
    }];

    let asks = world.asks();

    let sites = asks
        .iter()
        .find(|one| one.source == AskSource::BrowserSite)
        .unwrap();
    assert_eq!(
        sites.says,
        "Its browser wants to reach news.example.org:443. An Allow is its browser's alone, from \
         its next act; nothing restarts."
    );
}

/// #2364: an ask names whole every site it answers. Where they do not all fit its line, it
/// names the oldest that do and answers those; the others wait for the next ask.
#[test]
fn a_browser_sites_ask_answers_only_the_sites_its_line_names_whole() {
    use purlis_core::sandbox::browser_reach::Site;
    let written: Vec<String> = (1..=8)
        .map(|n| format!("site-number-{n}.example.org:443"))
        .collect();
    let mut world = World::new();
    world.browser_sites = vec![crate::chatbrowsers::SitesWait {
        session: 5,
        since: 1_700_000_000,
        sites: written
            .iter()
            .map(|one| Site::parse(one).unwrap())
            .collect(),
    }];

    let asks = world.asks();

    let sites = asks
        .iter()
        .find(|one| one.source == AskSource::BrowserSite)
        .unwrap();
    let AnswerPath::BrowserSites { sites: answered } = &sites.answer else {
        panic!("{:?}", sites.answer);
    };
    assert!(
        !answered.is_empty() && answered.len() < written.len(),
        "{answered:?}"
    );
    assert_eq!(answered[..], written[..answered.len()], "the oldest first");
    for site in answered {
        assert!(sites.says.contains(site.as_str()), "{}", sites.says);
    }
    assert!(!sites.says.contains('…'), "{}", sites.says);
    assert!(
        sites
            .says
            .contains(&format!("and {} more.", written.len() - answered.len())),
        "{}",
        sites.says
    );
    assert_eq!(ids(sites), ["chat", "you", "project", KEEP]);
}

/// #2364: a site too long for the line to name whole is never offered an Allow.
#[test]
fn a_browser_site_too_long_to_name_whole_is_only_kept_blocked() {
    use purlis_core::sandbox::browser_reach::Site;
    let long = format!(
        "{}.example.org:443",
        ["a".repeat(60), "b".repeat(60)].join(".")
    );
    let mut world = World::new();
    world.browser_sites = vec![crate::chatbrowsers::SitesWait {
        session: 5,
        since: 1_700_000_000,
        sites: vec![Site::parse(&long).unwrap()],
    }];

    let asks = world.asks();

    let sites = asks
        .iter()
        .find(|one| one.source == AskSource::BrowserSite)
        .unwrap();
    assert_eq!(ids(sites), [KEEP]);
}

#[test]
fn a_runners_service_is_asked_by_its_runner_and_offers_once_and_this_chat_alone() {
    // #2510, ruled 2026-10-18: a service the runner left out is never a service on this Mac,
    // and is allowed for no more than this chat.
    let block = HeldBlock {
        operation: "connect".to_owned(),
        kind: "runner-service".to_owned(),
        what: GrantWhat::Port,
        target: "5432".to_owned(),
    };
    let named = sandbox_runner(5, &block, Some("runner-7")).expect("an ask");
    assert_eq!(
        named.says,
        "The runner runner-7 refused its service at port 5432. Allowing it lets this chat alone \
         reach it there, and nothing on this Mac."
    );
    assert_eq!(
        named
            .options
            .iter()
            .map(|one| one.id.as_str())
            .collect::<Vec<_>>(),
        ["once", "chat", KEEP]
    );
    let unnamed = sandbox_block(5, &block, &Locks::none(), &Default::default()).expect("an ask");
    assert!(
        unnamed
            .says
            .starts_with("The runner this chat runs on refused its service at port 5432."),
        "{}",
        unnamed.says
    );
    assert!(
        !unnamed.says.contains("on this Mac at port"),
        "{}",
        unnamed.says
    );
}

#[test]
fn a_chats_browser_sites_ask_offers_everyone_only_where_the_projects_sandbox_is_on() {
    // #2443: a committed browser site reaches a browser only where the project's sandbox is on,
    // so elsewhere the ask does not offer it.
    use purlis_core::sandbox::browser_reach::Site;
    let ids = |everyone: bool| {
        let mut world = World::new();
        world.everyone = everyone;
        world.browser_sites = vec![crate::chatbrowsers::SitesWait {
            session: 5,
            since: 1_700_000_000,
            sites: vec![Site::parse("news.example.org:443").unwrap()],
        }];
        world
            .asks()
            .into_iter()
            .find(|one| one.source == AskSource::BrowserSite)
            .unwrap()
            .options
            .into_iter()
            .map(|one| one.id)
            .collect::<Vec<_>>()
    };
    assert_eq!(ids(true), ["chat", "you", "project", KEEP]);
    assert_eq!(ids(false), ["chat", "you", KEEP]);
}

#[test]
fn a_runner_chats_browser_login_asks_whether_its_persona_may_log_in_on_the_pool() {
    let mut world = World::new();
    world.pool_logins = vec![crate::chatbrowsers::PoolLoginWaits {
        session: 6,
        persona: "steward".into(),
        bound: purlis_core::poollogins::Bound {
            pool: "builds".into(),
            definition: "d".repeat(64),
            image: "a".repeat(64),
            runner: "01JB0RUNNER0000000000000000".into(),
            runner_key: "ab".repeat(32),
            runner_image: "a".repeat(64),
            engine: "this machine's Docker".into(),
        },
        login: "shop-login".into(),
        identity: "id-shop-login".into(),
        since: 1_700_000_000,
        changed: false,
    }];

    let asks = world.asks();

    let ask = asks
        .iter()
        .find(|one| one.source == AskSource::PoolLogin)
        .expect("its Ask is listed");
    assert_eq!(ask.session, 6);
    assert_eq!(ask.answer, AnswerPath::PoolLogin);
    assert_eq!(ids(ask), [super::POOL_LOGIN_ALLOW, KEEP]);
    assert_eq!(
        ask.options[0].label, "Allow on pool builds",
        "the line names its answer"
    );
    assert!(ask.says.contains("shop-login"), "{}", ask.says);
    assert_eq!(
        ask.says,
        "Wants its browser logged in with shop-login on pool builds, which runs on this \
         machine's Docker. Allow on pool builds fills steward's shop-login there from your \
         vault; the runner keeps none"
    );
}

#[test]
fn each_run_of_a_runner_chats_tool_asks_whether_its_values_may_be_sent_for_that_run() {
    let mut world = World::new();
    let waits = |run: u32, waited: bool| crate::pooltools::PoolToolWaits {
        session: 6,
        run,
        persona: "devops".into(),
        bound: purlis_core::poollogins::Bound {
            pool: "builds".into(),
            definition: "d".repeat(64),
            image: "a".repeat(64),
            runner: "01JB0RUNNER0000000000000000".into(),
            runner_key: "ab".repeat(32),
            runner_image: "a".repeat(64),
            engine: "this machine's Docker".into(),
        },
        tool: "kubectl".into(),
        vault: "team".into(),
        since: 1_700_000_000,
        waited,
    };
    world.pool_tools = vec![waits(1, false), waits(2, true)];

    let asks = world.asks();

    let tools: Vec<&Shown> = asks
        .iter()
        .filter(|one| one.source == AskSource::PoolTool)
        .collect();
    assert_eq!(tools.len(), 2, "one Ask per run");
    let ask = tools[0];
    assert_eq!(ask.session, 6);
    assert_eq!(ask.answer, AnswerPath::PoolTool { run: 1 });
    assert_eq!(ids(ask), [super::POOL_TOOL_ALLOW, KEEP]);
    assert_eq!(ask.options[0].label, "Allow this run");
    assert_eq!(
        ask.says,
        "Wants to run kubectl on pool builds, which runs on this machine's Docker, with \
         devops's values from vault team. Allow this run sends them from your vault for this \
         run alone; the runner keeps none"
    );
    assert!(
        tools[1].says.starts_with("Asked while you were away. "),
        "{}",
        tools[1].says
    );
}
