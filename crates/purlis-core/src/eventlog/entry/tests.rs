// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

use super::super::{ArgsKey, Attributes, Began, Event, Followed, Log, Recorder, RunOf, read};
use super::Entry;

const DEVICE: &str = "01J9ZZDEVICE0000000000000";
const CHAT: &str = "01K6E8ZK6V4Q9T0N3M2B1C5D7E";
const RUN: &str = "01K6E8ZK6V4Q9T0N3M2B1C5D7F";
const CLEARED: &str = "01K6E8ZK6V4Q9T0N3M2B1C5D7K";

fn recorder(dir: &Path) -> Recorder {
    Recorder::new(Log::open(dir, DEVICE).unwrap(), ArgsKey::open(dir).unwrap())
}

fn report(chat: u32, event: crate::state::Event) -> crate::hookwire::Report {
    crate::hookwire::Report {
        chat,
        event,
        conversation: crate::hookwire::Conversation::default(),
        pid: None,
        agent: Some("agent-1".to_owned()),
        detail: crate::state::Detail::default(),
    }
}

fn tool_call(chat: u32) -> crate::hookwire::ToolCall {
    crate::hookwire::ToolCall {
        chat,
        tool_hook: "pretooluse".to_owned(),
        tool: Some("Bash".to_owned()),
        call: Some("toolu_01".to_owned()),
        args: None,
        decision: crate::hookwire::Decision::Deny,
        rule: Some("no-force-push".to_owned()),
        hook_ms: 3,
        agent: None,
        at_ms: 0,
    }
}

fn attributes() -> Attributes {
    Attributes {
        persona: Some("devops".to_owned()),
        harness: Some("claude".to_owned()),
        ..Attributes::default()
    }
}

/// What an event says, but for what each write makes fresh (its time, and an id the log mints
/// for a child run): the same where the same was written.
fn said(event: &Event) -> (u64, String, Option<String>, bool, String, serde_json::Value) {
    (
        event.seq,
        event.kind.clone(),
        event.chat.clone(),
        event.parent_run.is_some(),
        event
            .run
            .clone()
            .filter(|run| event.parent_run.is_none() || run == RUN)
            .unwrap_or_default(),
        event.body.clone(),
    )
}

/// What the window asks, as it travels: written as JSON and read back.
fn sent(entry: Entry) -> Entry {
    serde_json::from_value(serde_json::to_value(&entry).expect("it is written")).expect("it reads")
}

#[test]
fn an_entry_sent_as_json_writes_what_the_windows_own_call_writes() {
    use crate::state::Event::{SessionStart, Stop};
    let plane = Path::new("/planes/one");
    let audited = || serde_json::json!({ "actor_kind": "human", "target": "devops" });

    let here = tempfile::tempdir().unwrap();
    {
        let mut window = recorder(here.path());
        let run = RunOf {
            chat: CHAT,
            run: RUN,
        };
        window
            .begin(plane, 3, run, Began::Start, &attributes())
            .unwrap();
        window
            .trust_said(
                run,
                "trust.sandbox.off",
                serde_json::json!({ "reason": null }),
            )
            .unwrap();
        window
            .report_with(plane, &report(3, Stop), Followed::No, None)
            .unwrap();
        window
            .report_with(
                plane,
                &report(3, SessionStart),
                Followed::Moved,
                Some(CLEARED),
            )
            .unwrap();
        window
            .tool(plane, &tool_call(3), std::time::Instant::now())
            .unwrap();
        window.refused(plane, 3).unwrap();
        window
            .audited(plane, Some(3), "trust.dispatch.grant", audited())
            .unwrap();
        window
            .audited(plane, None, "trust.sandbox.revoke", audited())
            .unwrap();
    }

    let there = tempfile::tempdir().unwrap();
    {
        let mut host = recorder(there.path());
        for entry in [
            Entry::Begin {
                plane: plane.into(),
                number: 3,
                chat: CHAT.into(),
                run: RUN.into(),
                cause: Began::Start,
                attributes: attributes(),
                held: false,
            },
            Entry::Trust {
                chat: CHAT.into(),
                run: RUN.into(),
                kind: "trust.sandbox.off".into(),
                body: serde_json::json!({ "reason": null }),
            },
            Entry::Report {
                plane: plane.into(),
                report: report(3, Stop),
                followed: Followed::No,
                begun: None,
            },
            Entry::Report {
                plane: plane.into(),
                report: report(3, SessionStart),
                followed: Followed::Moved,
                begun: Some(CLEARED.into()),
            },
            Entry::Tool {
                plane: plane.into(),
                call: tool_call(3),
            },
            Entry::Refused {
                plane: plane.into(),
                chat: 3,
            },
            Entry::Audited {
                plane: plane.into(),
                number: Some(3),
                kind: "trust.dispatch.grant".into(),
                body: audited(),
            },
            Entry::Audited {
                plane: plane.into(),
                number: None,
                kind: "trust.sandbox.revoke".into(),
                body: audited(),
            },
        ] {
            host.apply(sent(entry)).expect("the host writes it");
        }
    }

    let window: Vec<_> = read(here.path()).unwrap().iter().map(said).collect();
    let host: Vec<_> = read(there.path()).unwrap().iter().map(said).collect();
    assert!(
        window.len() >= 10,
        "every write and its child runs: {window:?}"
    );
    assert_eq!(host, window);
}

#[test]
fn a_chat_a_begun_run_names_is_known_by_its_id_and_one_never_named_is_not() {
    let dir = tempfile::tempdir().unwrap();
    let plane = Path::new("/planes/one");
    let mut host = recorder(dir.path());

    host.apply(Entry::Begin {
        plane: plane.into(),
        number: 4,
        chat: CHAT.into(),
        run: RUN.into(),
        cause: Began::Reopen,
        attributes: Attributes::default(),
        held: false,
    })
    .unwrap();

    assert_eq!(host.chat_of(plane, 4).as_deref(), Some(CHAT));
    assert_eq!(host.chat_of(plane, 5), None);
    assert_eq!(host.chat_of(Path::new("/planes/two"), 4), None);
}

#[test]
fn a_kill_switch_entry_is_refused_by_a_log_opened_without_the_machine_store() {
    let dir = tempfile::tempdir().unwrap();
    let mut host = recorder(dir.path());

    let refused = host
        .apply(Entry::KillSwitchRead)
        .expect_err("no journal to read");

    assert_eq!(refused.kind(), std::io::ErrorKind::NotFound);
    assert!(read(dir.path()).unwrap().is_empty(), "nothing is written");
}

#[test]
fn only_a_hook_line_names_the_chat_whose_spool_is_looked_at_next() {
    let plane = Path::new("/planes/one");
    let lines = [
        Entry::Report {
            plane: plane.into(),
            report: report(7, crate::state::Event::Stop),
            followed: Followed::No,
            begun: None,
        },
        Entry::Tool {
            plane: plane.into(),
            call: tool_call(7),
        },
        Entry::Refused {
            plane: plane.into(),
            chat: 7,
        },
    ];
    for line in lines {
        assert_eq!(line.hook_line_of(), Some(7), "{line:?}");
    }
    assert_eq!(Entry::KillSwitchRead.hook_line_of(), None);
    assert_eq!(
        Entry::Audited {
            plane: plane.into(),
            number: Some(7),
            kind: "trust.dispatch.grant".into(),
            body: serde_json::json!({}),
        }
        .hook_line_of(),
        None,
        "a person's act is no hook line"
    );
}

#[test]
fn a_trust_or_audited_entry_of_a_kind_that_is_no_trust_event_is_refused_and_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let plane = Path::new("/planes/one");
    let mut host = recorder(dir.path());

    for kind in [
        "run.started",
        "killswitch.stop",
        "trust.",
        "trust.Sandbox",
        "hook.stop",
    ] {
        let audited = host.apply(Entry::Audited {
            plane: plane.into(),
            number: None,
            kind: kind.into(),
            body: serde_json::json!({}),
        });
        let trust = host.apply(Entry::Trust {
            chat: CHAT.into(),
            run: RUN.into(),
            kind: kind.into(),
            body: serde_json::json!({}),
        });
        assert!(audited.is_err() && trust.is_err(), "{kind}");
    }
    assert!(read(dir.path()).unwrap().is_empty(), "nothing is written");
    host.apply(Entry::Audited {
        plane: plane.into(),
        number: None,
        kind: "trust.dispatch.give_back".into(),
        body: serde_json::json!({}),
    })
    .expect("a trust kind is written");
}

#[test]
fn a_held_run_sent_as_entries_is_written_as_the_windows_own_held_run_is() {
    use crate::state::Event::Stop;
    let plane = Path::new("/planes/one");
    let run = RunOf {
        chat: CHAT,
        run: RUN,
    };

    let here = tempfile::tempdir().unwrap();
    {
        let mut window = recorder(here.path());
        window.begin_held(plane, 3, run, Began::Start, &attributes());
        window.begun(plane, 3).unwrap();
        window.begin_held(plane, 4, run, Began::Start, &attributes());
        window
            .report_with(plane, &report(4, Stop), Followed::No, None)
            .unwrap();
        window.begin_held(plane, 5, run, Began::Start, &attributes());
    }

    let there = tempfile::tempdir().unwrap();
    {
        let mut host = recorder(there.path());
        let held = |number| Entry::Begin {
            plane: plane.into(),
            number,
            chat: CHAT.into(),
            run: RUN.into(),
            cause: Began::Start,
            attributes: attributes(),
            held: true,
        };
        assert!(
            host.apply(sent(held(3))).unwrap().is_empty(),
            "held, not written"
        );
        host.apply(sent(Entry::Begun {
            plane: plane.into(),
            number: 3,
        }))
        .unwrap();
        host.apply(sent(held(4))).unwrap();
        host.apply(sent(Entry::Report {
            plane: plane.into(),
            report: report(4, Stop),
            followed: Followed::No,
            begun: None,
        }))
        .unwrap();
        host.apply(sent(held(5))).unwrap();
    }

    let window: Vec<_> = read(here.path()).unwrap().iter().map(said).collect();
    let host: Vec<_> = read(there.path()).unwrap().iter().map(said).collect();
    let kinds: Vec<&str> = window.iter().map(|one| one.1.as_str()).collect();
    assert_eq!(
        kinds,
        // Chat 3's run, chat 4's run before its line, and the sub-agent's run that line begins.
        ["run.started", "run.started", "run.started", "hook.stop"],
        "one never begun writes nothing"
    );
    assert_eq!(host, window);
}
