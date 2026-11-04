// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The Secret ask registry (#5068, #5071): what a chat whose brokered run missed a key hears,
//! and what the window is shown and may answer. Time is passed in: a slow answer is a hold of
//! nothing, never a sleep.

use super::*;
use crate::harness::asks::{Admitted, Answerer};

const VALUE: &str = "fr3sh-s3cret-value";

/// A project whose vault `team` is a plain file holding only `TOKEN`.
fn project() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    std::fs::write(
        root.join("vaults.json"),
        serde_json::json!({ "vaults": {
            "team": {"provider": "plain-file", "config": {"file": "team.json"}, "persona": "devops"},
        }})
        .to_string(),
    )
    .unwrap();
    std::fs::write(
        root.join("team.json"),
        serde_json::json!({"TOKEN": "t0ken"}).to_string(),
    )
    .unwrap();
    tmp
}

fn ctx(root: &Path) -> Ctx {
    Ctx::new(root, crate::secrets::Env::of(&[]))
}

fn need(chat: u32, key: &str) -> Need {
    Need {
        chat: Some(chat),
        persona: "devops".into(),
        vault: "team".into(),
        key: key.into(),
        tool: Some("kubectl".into()),
    }
}

fn window() -> Answerer {
    Answerer::admitted(Admitted::LocalUi).unwrap()
}

/// What a waiter holding for nothing at all hears, said lines and all.
fn held_for_nothing(asks: &Asks, need: Need) -> (Held, Vec<String>) {
    let mut said = Vec::new();
    let held = asks.hold(need, Duration::ZERO, &mut |line: &str| {
        said.push(line.to_owned())
    });
    (held, said)
}

/// [`Asks::hold`] for a minute, telling `waits` once it waits.
fn asks_hold_telling(asks: &Asks, need: Need, waits: &std::sync::mpsc::Sender<()>) -> Held {
    asks.hold(need, Duration::from_secs(60), &mut |_| {
        let _ = waits.send(());
    })
}

/// The value `key` of vault `team` holds now.
fn stored(root: &Path, key: &str) -> Option<String> {
    let ctx = ctx(root);
    let v = crate::secrets::cmd::provider(&ctx, "team").unwrap();
    crate::secrets::cmd::get_value(&ctx, &v, key).ok()
}

/// Every line of every trace in `root`.
fn traced(root: &Path) -> String {
    let dir = crate::names::state(root).join("persona-state/trace");
    let Ok(files) = std::fs::read_dir(dir) else {
        return String::new();
    };
    files
        .filter_map(Result::ok)
        .map(|file| std::fs::read_to_string(file.path()).unwrap())
        .collect()
}

#[test]
fn a_broker_miss_raises_one_secret_ask_naming_the_chat_persona_vault_and_key() {
    let project = project();
    let asks = Asks::new(project.path());
    let (held, said) = held_for_nothing(&asks, need(7, "MISSING"));
    assert_eq!(
        held,
        Held::StillOpen {
            line: "waiting on you for team/MISSING; retry once answered".into()
        }
    );
    assert!(
        said.iter().any(|line| line.contains("team/MISSING")),
        "the chat is told what it waits on: {said:?}"
    );
    let open = asks.open();
    assert_eq!(open.len(), 1, "{open:?}");
    let one = &open[0];
    assert_eq!(
        (
            one.chats.as_slice(),
            one.persona.as_str(),
            one.vault.as_str()
        ),
        (&[7][..], "devops", "team")
    );
    assert_eq!(one.key, "MISSING");
    assert_eq!(one.tool.as_deref(), Some("kubectl"));
    assert_eq!(one.waiting(), 1);
}

#[test]
fn a_quick_fill_resumes_the_waiting_command_and_is_written_through_the_vaults_provider() {
    let project = project();
    let asks = Asks::new(project.path());
    let id = std::thread::scope(|scope| {
        let waiting =
            scope.spawn(|| asks.hold(need(7, "MISSING"), Duration::from_secs(60), &mut |_| {}));
        let id = loop {
            if let Some(one) = asks.open().first() {
                break one.id;
            }
            std::thread::yield_now();
        };
        let answered = asks
            .answer(
                &ctx(project.path()),
                id,
                Answer::Fill(VALUE.to_owned().into()),
                window(),
            )
            .unwrap();
        assert!(
            answered.nudge.is_empty(),
            "nobody stopped waiting: {answered:?}"
        );
        assert_eq!(waiting.join().unwrap(), Held::Filled);
        id
    });
    assert_eq!(stored(project.path(), "MISSING").as_deref(), Some(VALUE));
    assert!(asks.open().is_empty(), "ask {id} is settled");
}

#[test]
fn a_decline_answers_the_chat_declined_with_the_vault_and_key() {
    let project = project();
    let asks = Asks::new(project.path());
    std::thread::scope(|scope| {
        let waiting =
            scope.spawn(|| asks.hold(need(7, "MISSING"), Duration::from_secs(60), &mut |_| {}));
        let id = loop {
            if let Some(one) = asks.open().first() {
                break one.id;
            }
            std::thread::yield_now();
        };
        asks.answer(&ctx(project.path()), id, Answer::Decline, window())
            .unwrap();
        assert_eq!(
            waiting.join().unwrap(),
            Held::Declined {
                line: "declined team/MISSING".into()
            }
        );
    });
    assert_eq!(stored(project.path(), "MISSING"), None);
}

#[test]
fn only_the_window_answers_a_secret_ask_and_an_inbox_answer_leaves_it_open() {
    let project = project();
    let asks = Asks::new(project.path());
    held_for_nothing(&asks, need(7, "MISSING"));
    let id = asks.open()[0].id;
    let inbox = Answerer::admitted(Admitted::Approval).unwrap();
    let refused = asks
        .answer(
            &ctx(project.path()),
            id,
            Answer::Fill(VALUE.to_owned().into()),
            inbox,
        )
        .unwrap_err();
    assert!(refused.contains("window"), "{refused}");
    assert_eq!(asks.open().len(), 1, "still open");
    assert_eq!(stored(project.path(), "MISSING"), None);
    // A chat, the fleet's MCP server, the editor and a terminal are no answerer at all.
    for scope in [
        Admitted::Chat,
        Admitted::FleetMcp,
        Admitted::Editor,
        Admitted::Terminal,
    ] {
        assert!(Answerer::admitted(scope).is_none(), "{scope:?}");
    }
}

#[test]
fn an_answer_to_an_ask_no_longer_open_is_refused() {
    let project = project();
    let asks = Asks::new(project.path());
    let refused = asks
        .answer(&ctx(project.path()), 41, Answer::Decline, window())
        .unwrap_err();
    assert!(refused.contains("no longer"), "{refused}");
}

#[test]
fn a_slow_answer_leaves_the_ask_open_and_nudges_the_chat_that_stopped_waiting() {
    let project = project();
    let asks = Asks::new(project.path());
    let (held, _) = held_for_nothing(&asks, need(7, "MISSING"));
    assert!(matches!(held, Held::StillOpen { .. }), "{held:?}");
    let id = asks.open()[0].id;
    let answered = asks
        .answer(
            &ctx(project.path()),
            id,
            Answer::Fill(VALUE.to_owned().into()),
            window(),
        )
        .unwrap();
    assert_eq!(answered.nudge, vec![7]);
    assert_eq!(
        answered.told.line(),
        "⬢ purlis: secret `team/MISSING` — Filled by the person — run the command that waited \
         on it again."
    );
    // The chat's next run finds the value: nothing is asked again.
    assert_eq!(stored(project.path(), "MISSING").as_deref(), Some(VALUE));
}

#[test]
fn a_chat_that_tries_again_while_its_ask_is_open_is_not_nudged_once_its_retry_hears_the_fill() {
    let project = project();
    let asks = Asks::new(project.path());
    held_for_nothing(&asks, need(7, "MISSING"));
    let id = asks.open()[0].id;
    let (waits, waiting) = std::sync::mpsc::channel();
    std::thread::scope(|scope| {
        let asked = &asks;
        let retry = scope.spawn(move || asks_hold_telling(asked, need(7, "MISSING"), &waits));
        waiting.recv().unwrap();
        let answered = asks
            .answer(
                &ctx(project.path()),
                id,
                Answer::Fill(VALUE.to_owned().into()),
                window(),
            )
            .unwrap();
        assert!(answered.nudge.is_empty(), "{answered:?}");
        assert_eq!(retry.join().unwrap(), Held::Filled);
    });
}

#[test]
fn three_chats_missing_one_key_join_one_ask_and_one_fill_resumes_them_all() {
    let project = project();
    let asks = Asks::new(project.path());
    std::thread::scope(|scope| {
        let waiting: Vec<_> = [7, 8, 9]
            .into_iter()
            .map(|chat| {
                let asks = &asks;
                scope.spawn(move || {
                    asks.hold(need(chat, "MISSING"), Duration::from_secs(60), &mut |_| {})
                })
            })
            .collect();
        let one = loop {
            let open = asks.open();
            if open.first().is_some_and(|one| one.waiting() == 3) {
                break open;
            }
            std::thread::yield_now();
        };
        assert_eq!(one.len(), 1, "one ask: {one:?}");
        let mut chats = one[0].chats.clone();
        chats.sort_unstable();
        assert_eq!(chats, vec![7, 8, 9]);
        asks.answer(
            &ctx(project.path()),
            one[0].id,
            Answer::Fill(VALUE.to_owned().into()),
            window(),
        )
        .unwrap();
        for one in waiting {
            assert_eq!(one.join().unwrap(), Held::Filled);
        }
    });
}

#[test]
fn a_decline_settles_only_the_chats_waiting_then_and_that_chat_is_not_asked_again() {
    let project = project();
    let asks = Asks::new(project.path());
    held_for_nothing(&asks, need(7, "MISSING"));
    held_for_nothing(&asks, need(8, "MISSING"));
    let id = asks.open()[0].id;
    let answered = asks
        .answer(&ctx(project.path()), id, Answer::Decline, window())
        .unwrap();
    let mut nudged = answered.nudge.clone();
    nudged.sort_unstable();
    assert_eq!(nudged, vec![7, 8]);
    assert_eq!(
        answered.told.line(),
        "⬢ purlis: secret `team/MISSING` — Declined by the person — don't ask for it again in \
         this chat; go on without it."
    );
    // Chat 7's later run hears declined at once, and nothing is asked.
    let (held, _) = held_for_nothing(&asks, need(7, "MISSING"));
    assert_eq!(
        held,
        Held::Declined {
            line: "declined team/MISSING".into()
        }
    );
    assert!(asks.open().is_empty(), "{:?}", asks.open());
    // A chat that was not waiting then gets an ask of its own.
    held_for_nothing(&asks, need(9, "MISSING"));
    let open = asks.open();
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].chats, vec![9]);
    assert_ne!(open[0].id, id);
}

#[test]
fn a_chat_that_closes_is_no_longer_waiting_and_forgets_what_it_declined() {
    let project = project();
    let asks = Asks::new(project.path());
    held_for_nothing(&asks, need(7, "MISSING"));
    held_for_nothing(&asks, need(8, "MISSING"));
    asks.forget(7);
    assert_eq!(asks.open()[0].chats, vec![8]);
    asks.forget(8);
    assert!(asks.open().is_empty(), "nobody waits on it");
}

#[test]
fn another_key_or_vault_is_another_ask() {
    let project = project();
    let asks = Asks::new(project.path());
    held_for_nothing(&asks, need(7, "MISSING"));
    held_for_nothing(&asks, need(7, "OTHER"));
    assert_eq!(asks.open().len(), 2);
}

#[test]
fn no_value_reaches_the_ask_the_chats_answer_or_the_trace() {
    let project = project();
    let asks = Asks::new(project.path());
    let (_, said_while_waiting) = held_for_nothing(&asks, need(7, "MISSING"));
    let shown = format!("{:?}", asks.open());
    let id = asks.open()[0].id;
    let answered = asks
        .answer(
            &ctx(project.path()),
            id,
            Answer::Fill(VALUE.to_owned().into()),
            window(),
        )
        .unwrap();
    let trace = traced(project.path());
    for what in [
        shown,
        format!("{answered:?}"),
        said_while_waiting.join("\n"),
        trace.clone(),
    ] {
        assert!(!what.contains(VALUE), "{what}");
    }
    // Metadata only: raised and filled, by vault and key.
    for event in ["raised", "filled"] {
        assert!(
            trace.lines().any(|line| line.contains("\"secret.ask\"")
                && line.contains(event)
                && line.contains("\"MISSING\"")),
            "{event}: {trace}"
        );
    }
}

#[test]
fn a_fill_that_the_provider_refuses_leaves_the_ask_open() {
    let project = project();
    let asks = Asks::new(project.path());
    held_for_nothing(&asks, need(7, "MISSING"));
    let id = asks.open()[0].id;
    std::fs::write(project.path().join("vaults.json"), "{}").unwrap();
    let refused = asks
        .answer(
            &ctx(project.path()),
            id,
            Answer::Fill(VALUE.to_owned().into()),
            window(),
        )
        .unwrap_err();
    assert!(!refused.contains(VALUE), "{refused}");
    assert_eq!(asks.open().len(), 1);
}

#[test]
fn an_empty_fill_is_refused_and_the_ask_stays_open() {
    let project = project();
    let asks = Asks::new(project.path());
    held_for_nothing(&asks, need(7, "MISSING"));
    let id = asks.open()[0].id;
    asks.answer(
        &ctx(project.path()),
        id,
        Answer::Fill(String::new().into()),
        window(),
    )
    .unwrap_err();
    assert_eq!(asks.open().len(), 1);
    assert_eq!(stored(project.path(), "MISSING"), None);
}

#[test]
fn a_harness_is_held_for_three_quarters_of_its_command_timeout() {
    use crate::harness::Harness;
    assert_eq!(
        holds_for(Some(Harness::ClaudeCode)),
        Duration::from_secs(90)
    );
    assert_eq!(holds_for(Some(Harness::Opencode)), Duration::from_secs(90));
    assert_eq!(
        holds_for(Some(Harness::Codex)),
        Duration::from_millis(7_500)
    );
    assert_eq!(holds_for(None), holds_for(Some(Harness::Codex)));
}

// ----------------------------------------------------------------------------------------
// every brokered read asks alike: a browser login, a profile's credential (#5074)

/// The first ask open, once one is.
fn first_open(asks: &Asks) -> Open {
    loop {
        if let Some(one) = asks.open().into_iter().next() {
            break one;
        }
        std::thread::yield_now();
    }
}

/// `key` of vault `team`, read as a brokered read reads it.
fn read_team(root: &Path, key: &str) -> Result<String, crate::secrets::VaultError> {
    let ctx = ctx(root);
    let v = crate::secrets::cmd::provider(&ctx, "team")?;
    crate::secrets::cmd::get_value(&ctx, &v, key)
}

#[test]
fn a_read_of_a_key_the_vault_holds_asks_nothing() {
    let project = project();
    let asks = Asks::new(project.path());
    let got = asks.read_asking(need(7, "TOKEN"), Duration::ZERO, &mut |_| {}, &mut || {
        read_team(project.path(), "TOKEN")
    });
    assert_eq!(got.ok().as_deref(), Some("t0ken"));
    assert!(asks.open().is_empty());
}

#[test]
fn a_read_of_a_missing_key_waits_on_its_secret_ask_and_reads_the_value_filled() {
    let project = project();
    let asks = Asks::new(project.path());
    let got = std::thread::scope(|scope| {
        let read = scope.spawn(|| {
            asks.read_asking(
                need(7, "MISSING"),
                Duration::from_secs(60),
                &mut |_| {},
                &mut || read_team(project.path(), "MISSING"),
            )
        });
        let id = first_open(&asks).id;
        asks.answer(
            &ctx(project.path()),
            id,
            Answer::Fill(VALUE.to_owned().into()),
            window(),
        )
        .unwrap();
        read.join().unwrap()
    });
    assert_eq!(got.ok().as_deref(), Some(VALUE));
}

#[test]
fn a_read_of_a_missing_key_that_is_declined_or_unanswered_says_so_and_reads_nothing() {
    let project = project();
    let asks = Asks::new(project.path());
    let unanswered = asks
        .read_asking(need(7, "MISSING"), Duration::ZERO, &mut |_| {}, &mut || {
            read_team(project.path(), "MISSING")
        })
        .unwrap_err();
    assert_eq!(
        unanswered.to_string(),
        "waiting on you for team/MISSING; retry once answered"
    );
    let id = asks.open()[0].id;
    asks.answer(&ctx(project.path()), id, Answer::Decline, window())
        .unwrap();
    let declined = asks
        .read_asking(need(7, "MISSING"), Duration::ZERO, &mut |_| {}, &mut || {
            read_team(project.path(), "MISSING")
        })
        .unwrap_err();
    assert_eq!(declined.to_string(), "declined team/MISSING");
}

#[test]
fn a_read_the_vault_refuses_for_anything_but_a_missing_key_asks_nothing() {
    let project = project();
    let asks = Asks::new(project.path());
    let why = asks
        .read_asking(need(7, "MISSING"), Duration::ZERO, &mut |_| {}, &mut || {
            Err::<String, _>(crate::secrets::VaultError::unavailable(
                "the vault is locked",
            ))
        })
        .unwrap_err();
    assert_eq!(why.to_string(), "the vault is locked");
    assert!(asks.open().is_empty(), "nothing asked");
}

/// A chat's start has no number yet: it waits on the ask as a chat would, and is counted.
fn a_start(key: &str) -> Need {
    Need {
        chat: None,
        tool: None,
        ..need(0, key)
    }
}

#[test]
fn a_start_waits_on_the_same_ask_as_a_chat_and_is_counted_but_never_named() {
    let project = project();
    let asks = Asks::new(project.path());
    std::thread::scope(|scope| {
        let start =
            scope.spawn(|| asks.hold(a_start("MISSING"), Duration::from_secs(60), &mut |_| {}));
        first_open(&asks);
        let (held, _) = held_for_nothing(&asks, need(7, "MISSING"));
        assert!(matches!(held, Held::StillOpen { .. }), "{held:?}");
        let open = asks.open();
        assert_eq!(open.len(), 1, "one ask per vault and key: {open:?}");
        assert_eq!(open[0].chats, [7]);
        assert_eq!(open[0].waiting(), 2, "the start and chat 7");
        let answered = asks
            .answer(&ctx(project.path()), open[0].id, Answer::Decline, window())
            .unwrap();
        assert_eq!(answered.nudge, [7], "a start is never nudged");
        assert!(matches!(start.join().unwrap(), Held::Declined { .. }));
    });
    // A decline is a chat's to keep: the next start is asked afresh.
    let (held, _) = held_for_nothing(&asks, a_start("MISSING"));
    assert!(matches!(held, Held::StillOpen { .. }), "{held:?}");
    let open = asks.open();
    assert_eq!(open.len(), 1);
    assert_eq!(
        open[0].waiting(),
        1,
        "a start that stopped waiting is still what the ask is for: {open:?}"
    );
    // One that asks again takes its place.
    held_for_nothing(&asks, a_start("MISSING"));
    assert_eq!(asks.open()[0].waiting(), 1);
}

#[test]
fn a_starts_ask_is_traced_under_no_session() {
    let project = project();
    let asks = Asks::new(project.path());
    held_for_nothing(&asks, a_start("MISSING"));
    let file = crate::trace::file(project.path(), crate::trace::NO_SESSION);
    let trace = std::fs::read_to_string(&file).unwrap_or_default();
    assert!(
        trace.contains("\"secret.ask\"") && trace.contains("raised"),
        "{file:?}: {trace}"
    );
}
