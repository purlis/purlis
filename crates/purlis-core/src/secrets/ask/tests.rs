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

/// The project's vault context. Its state directory is named by `$PURLIS_HOME`, as a person may
/// name it: the same reads and writes, under a folder no sandbox of the run holds back (the
/// links index takes the vaults' folder lock).
fn ctx(root: &Path) -> Ctx {
    let home = root.join("home");
    Ctx::new(
        root,
        crate::secrets::Env::of(&[("PURLIS_HOME", &home.to_string_lossy())]),
    )
}

fn need(chat: u32, key: &str) -> Need {
    Need {
        chat: Some(chat),
        persona: "devops".into(),
        vault: "team".into(),
        key: key.into(),
        tool: Some("kubectl".into()),
        purpose: None,
        declared: None,
        names_no_vault: false,
        reason: None,
        no_allow: None,
    }
}

/// Filled as asked: vault `team`, under `key`.
fn filled(key: &str) -> Held {
    Held::Filled {
        vault: "team".into(),
        key: key.into(),
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
            line: format!(
                "waiting on you for team/MISSING; {}; retry once answered",
                crate::askwait::still_answerable_said(project.path())
            )
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
            if waiting.is_finished() {
                panic!("it ended without asking: {:?}", waiting.join());
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
        assert_eq!(waiting.join().unwrap(), filled("MISSING"));
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
            if waiting.is_finished() {
                panic!("it ended without asking: {:?}", waiting.join());
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

/// [`Asks::hold_while`] for ten seconds, its caller gone from its first look.
fn held_with_its_caller_gone(asks: &Asks, need: Need) -> (Held, Duration) {
    let started = std::time::Instant::now();
    let held = asks.hold_while(need, Duration::from_secs(10), &|| true, &mut |_| {});
    (held, started.elapsed())
}

#[test]
fn a_hold_whose_caller_hangs_up_stops_waiting_and_its_ask_goes_from_the_window() {
    let project = project();
    let asks = Asks::new(project.path());

    let (held, took) = held_with_its_caller_gone(&asks, need(7, "MISSING"));

    assert!(
        matches!(&held, Held::CallerGone { line } if line.contains("stopped waiting")),
        "{held:?}"
    );
    assert!(
        took < Duration::from_secs(5),
        "it looked, and stopped: {took:?}"
    );
    assert!(asks.open().is_empty(), "nobody else needs it");
}

#[test]
fn a_hold_whose_caller_hangs_up_leaves_the_ask_to_the_chats_still_needing_it() {
    let project = project();
    let asks = Asks::new(project.path());
    held_for_nothing(&asks, need(8, "MISSING"));

    let (held, _) = held_with_its_caller_gone(&asks, need(7, "MISSING"));

    assert!(matches!(held, Held::CallerGone { .. }), "{held:?}");
    let open = asks.open();
    assert_eq!(open.len(), 1);
    assert_eq!(
        open[0].chats,
        vec![8],
        "the chat that hung up is not nudged"
    );
    let answered = asks
        .answer(
            &ctx(project.path()),
            open[0].id,
            Answer::Fill(VALUE.to_owned().into()),
            window(),
        )
        .unwrap();
    assert_eq!(answered.nudge, vec![8]);
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
        assert_eq!(retry.join().unwrap(), filled("MISSING"));
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
            if let Some(ended) = waiting
                .iter()
                .position(std::thread::ScopedJoinHandle::is_finished)
            {
                panic!(
                    "chat {} ended before all three held: {open:?}",
                    [7, 8, 9][ended]
                );
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
            assert_eq!(one.join().unwrap(), filled("MISSING"));
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

/// The first ask open, once one is: failing at [`SPIN_AT_MOST`], where no run asked.
fn first_open(asks: &Asks) -> Open {
    let deadline = std::time::Instant::now() + SPIN_AT_MOST;
    loop {
        if let Some(one) = asks.open().into_iter().next() {
            break one;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "no ask was opened in {SPIN_AT_MOST:?}"
        );
        std::thread::yield_now();
    }
}

/// How long a test spins for what another thread does where it holds no handle to that thread,
/// before it fails: never for ever in a sandboxed chat that refused the run (R-355).
const SPIN_AT_MOST: Duration = Duration::from_secs(60);

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
        format!(
            "waiting on you for team/MISSING; {}; retry once answered",
            crate::askwait::still_answerable_said(project.path())
        )
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

/// #5078 with #5074: an ask only a chat's start waits on is still recorded, under no chat,
/// and its decline too, counting the start as one waiting.
#[test]
fn a_starts_ask_is_recorded_under_no_chat() {
    use crate::eventlog::secretasks::Step;
    let project = project();
    let asks = Asks::new(project.path());
    let told = steps_told(&asks);
    std::thread::scope(|scope| {
        let start =
            scope.spawn(|| asks.hold(a_start("MISSING"), Duration::from_secs(60), &mut |_| {}));
        let id = first_open(&asks).id;
        // The ask is listed a moment before its raise is told: answered only once it is, so
        // the record's order is the one this test is about.
        while told.lock().unwrap().is_empty() {
            if start.is_finished() {
                panic!("it ended before its raise was told: {:?}", start.join());
            }
            std::thread::yield_now();
        }
        asks.answer(&ctx(project.path()), id, Answer::Decline, window())
            .expect("declined");
        assert!(matches!(start.join().unwrap(), Held::Declined { .. }));
        assert_eq!(
            told.lock().unwrap().as_slice(),
            [
                (
                    None,
                    Step::Raised {
                        ask: id,
                        declared: false
                    }
                ),
                (
                    None,
                    Step::Declined {
                        ask: id,
                        waiting: 1
                    }
                ),
            ]
        );
    });
}

/// What chat `chat`'s trace says of the ask for `key`, in the order it was written.
fn traced_of(root: &Path, chat: u32, key: &str) -> Vec<String> {
    let file = crate::trace::file(root, &chat.to_string());
    std::fs::read_to_string(&file)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|line| line["event"] == TRACED_AS && line["key"] == key)
        .map(|line| line["what"].as_str().unwrap_or_default().to_owned())
        .collect()
}

/// #5220, ADR 0109: an ask is told raised (or joined) before any answer to it, in the event log
/// and in each chat's trace alike, however soon the person answers. One thread raises and
/// another joins while a third answers the moment the ask is listed, many times over.
#[test]
fn an_answer_the_moment_an_ask_is_listed_is_never_recorded_before_its_raise_or_join() {
    use crate::eventlog::secretasks::Step;
    let project = project();
    let root = project.path();
    let asks = Asks::new(root);
    let told = steps_told(&asks);
    for round in 0..200 {
        let key = format!("MISSING_{round}");
        told.lock().unwrap().clear();
        std::thread::scope(|scope| {
            let chats = [7, 8].map(|chat| {
                let (asks, key) = (&asks, &key);
                scope
                    .spawn(move || asks.hold(need(chat, key), Duration::from_secs(60), &mut |_| {}))
            });
            // Whichever chat comes first raises and the other joins, unless it comes once the
            // ask is answered and raises one of its own: each is answered the moment it is
            // listed, until both chats have heard.
            while !chats.iter().all(std::thread::ScopedJoinHandle::is_finished) {
                if let Some(open) = asks.open().into_iter().find(|open| open.key == key) {
                    // Refused only where it was answered already.
                    let _ = asks.answer(&ctx(root), open.id, Answer::Decline, window());
                }
                std::thread::yield_now();
            }
        });
        // Each ask's first step is its raise, and nothing joins it once it is answered.
        let steps = told.lock().unwrap().clone();
        let ask_of = |step: &Step| match *step {
            Step::Raised { ask, .. }
            | Step::Joined { ask, .. }
            | Step::Filled { ask, .. }
            | Step::Declined { ask, .. } => Some(ask),
            Step::Refused { .. } => None,
        };
        let mut asked = std::collections::BTreeMap::<u32, Vec<Step>>::new();
        for (_, step) in &steps {
            if let Some(ask) = ask_of(step) {
                asked.entry(ask).or_default().push(*step);
            }
        }
        assert!(
            asked
                .values()
                .flatten()
                .any(|step| matches!(step, Step::Declined { .. })),
            "round {round}: the decline is told: {steps:?}"
        );
        for (ask, its) in &asked {
            assert!(
                matches!(its.first(), Some(Step::Raised { .. })),
                "round {round}: ask {ask} is told before its raise: {steps:?}"
            );
            if let Some(answered) = its
                .iter()
                .position(|step| matches!(step, Step::Declined { .. }))
            {
                assert_eq!(
                    answered,
                    its.len() - 1,
                    "round {round}: ask {ask} is told after its answer: {steps:?}"
                );
            }
        }
        for chat in [7, 8] {
            let trace = traced_of(root, chat, &key);
            let declined = trace
                .iter()
                .position(|what| what == "declined")
                .unwrap_or_else(|| {
                    panic!("round {round}: chat {chat} heard its decline: {trace:?}")
                });
            {
                assert!(
                    trace[..declined]
                        .iter()
                        .any(|what| what == "raised" || what == "joined"),
                    "round {round}: chat {chat}'s trace has the answer first: {trace:?}"
                );
            }
        }
    }
}

/// #5220: a chat that cannot receive a value raises its ask, as any ask is raised (#5077), the
/// person declines it at once, and the chat raises it again the moment it is gone: its trace,
/// which names no ask, reads raise, decline, raise, so the second ask is never read as
/// declined; and the event log tells the first ask's answer before the second's raise.
#[test]
fn a_decline_is_recorded_before_an_ask_raised_again_the_moment_it_is_answered() {
    use crate::eventlog::secretasks::Step;
    use std::sync::atomic::{AtomicBool, Ordering};
    let project = project();
    let root = project.path();
    let asks = Asks::new(root);
    let told = steps_told(&asks);
    for round in 0..200 {
        let key = format!("MISSING_{round}");
        told.lock().unwrap().clear();
        let listed = AtomicBool::new(false);
        std::thread::scope(|scope| {
            let raised = scope.spawn(|| asks.cannot_receive(need(9, &key)));
            let again = scope.spawn(|| {
                let deadline = std::time::Instant::now() + SPIN_AT_MOST;
                while !listed.load(Ordering::SeqCst) {
                    assert!(
                        std::time::Instant::now() < deadline,
                        "the ask was never listed"
                    );
                    std::thread::yield_now();
                }
                while asks.open().iter().any(|open| open.key == key) {
                    assert!(
                        std::time::Instant::now() < deadline,
                        "the ask was never answered: {:?}",
                        asks.open()
                    );
                    std::thread::yield_now();
                }
                asks.cannot_receive(need(9, &key))
            });
            let id = loop {
                if let Some(open) = asks.open().into_iter().find(|open| open.key == key) {
                    break open.id;
                }
                if raised.is_finished() && again.is_finished() {
                    panic!("round {round}: neither raise was listed: {:?}", asks.open());
                }
                std::thread::yield_now();
            };
            listed.store(true, Ordering::SeqCst);
            asks.answer(&ctx(root), id, Answer::Decline, window())
                .expect("declined");
            again.join().unwrap();
        });
        assert_eq!(
            traced_of(root, 9, &key),
            [
                "raised, cannot receive",
                "declined",
                "raised, cannot receive"
            ],
            "round {round}"
        );
        let steps: Vec<Step> = told.lock().unwrap().iter().map(|(_, step)| *step).collect();
        assert!(
            matches!(
                steps.as_slice(),
                [Step::Raised { ask: first, .. }, Step::Declined { ask: answered, .. }, Step::Raised { ask: second, .. }]
                    if first == answered && second != first
            ),
            "round {round}: {steps:?}"
        );
        asks.forget(9);
    }
}

// ----------------------------------------------------------------------------------------
// A chat asks: the `secret_ask` line (#5072)

/// [`project`], committed, with persona `devops` declaring `DECLARED` as a Secret need, a
/// second vault `mine` tagged for it, a third `others` tagged for nobody, and `charter.toml`.
fn project_with_needs() -> tempfile::TempDir {
    let tmp = project();
    let root = tmp.path();
    std::fs::write(root.join("charter.toml"), "").unwrap();
    std::fs::create_dir_all(root.join("personas/devops")).unwrap();
    std::fs::write(
        root.join("personas/devops/persona.md"),
        "---\nrole: Ops\nvault: team\nsecret-need.DECLARED: deploy to staging\n---\n",
    )
    .unwrap();
    std::fs::write(
        root.join("vaults.json"),
        serde_json::json!({ "vaults": {
            "team": {"provider": "plain-file", "config": {"file": "team.json"}, "persona": "devops"},
            "mine": {"provider": "plain-file", "config": {"file": "mine.json"}, "persona": "devops"},
            "others": {"provider": "plain-file", "config": {"file": "others.json"}},
        }})
        .to_string(),
    )
    .unwrap();
    std::fs::write(root.join("mine.json"), "{}").unwrap();
    std::fs::write(root.join("others.json"), "{}").unwrap();
    crate::secrets::needs::tests::committed(root);
    tmp
}

fn devops(chat: u32) -> Chat<'static> {
    Chat {
        chat,
        persona: Some("devops"),
        harness: None,
        asker: None,
    }
}

fn line(chat: u32, key: &str, vault: Option<&str>) -> SecretAsk {
    SecretAsk {
        chat,
        key: key.into(),
        purpose: "  charge test cards\nin the checkout e2e ".into(),
        vault: vault.map(str::to_owned),
        replace: None,
        program: None,
    }
}

/// [`raised_within`] for nothing at all.
fn raised_now(root: &Path, asks: &Asks, line: &SecretAsk) -> Result<Said, String> {
    raised_within(
        &ctx(root),
        asks,
        devops(line.chat),
        line,
        Duration::ZERO,
        &|| {},
    )
}

/// [`raised_within`] for a minute, telling `waits` once the ask is listed.
fn raised_telling(
    root: &Path,
    asks: &Asks,
    line: &SecretAsk,
    waits: &std::sync::mpsc::Sender<()>,
) -> Result<Said, String> {
    raised_within(
        &ctx(root),
        asks,
        devops(line.chat),
        line,
        Duration::from_secs(60),
        &|| {
            let _ = waits.send(());
        },
    )
}

#[test]
fn a_chats_ask_goes_to_its_personas_vault_with_its_purpose_as_one_line_and_waits_on_the_person() {
    let project = project_with_needs();
    let asks = Asks::new(project.path());
    let said = raised_now(project.path(), &asks, &line(7, "STRIPE_KEY", None));
    assert_eq!(
        said,
        Ok(Said::Pending {
            vault: "team".into(),
            key: "STRIPE_KEY".into()
        }),
        "the vault defaults to the persona's"
    );
    let open = asks.open();
    assert_eq!(open.len(), 1, "{open:?}");
    assert_eq!(
        (
            open[0].vault.as_str(),
            open[0].key.as_str(),
            open[0].chats.as_slice()
        ),
        ("team", "STRIPE_KEY", &[7][..])
    );
    assert_eq!(
        open[0].purpose.as_deref(),
        Some("charge test cards⟦U+000A⟧in the checkout e2e"),
        "the chat's words, on one line"
    );
    assert_eq!(open[0].tool, None);
    let (told, code) = said.unwrap().line(project.path());
    assert_eq!(code, super::super::brokered::WAITING_ON_THE_PERSON);
    assert_eq!(
        told,
        format!(
            "waiting on you for team/STRIPE_KEY; {}; retry once answered",
            crate::askwait::still_answerable_said(project.path())
        )
    );
}

#[test]
fn a_key_the_vault_holds_already_answers_filled_at_once_and_asks_nothing() {
    let project = project_with_needs();
    let asks = Asks::new(project.path());
    assert_eq!(
        raised_now(project.path(), &asks, &line(7, "TOKEN", None)),
        Ok(Said::Filled {
            vault: "team".into(),
            key: "TOKEN".into()
        })
    );
    assert!(asks.open().is_empty(), "nothing was asked");
    assert!(
        !traced(project.path()).contains("raised"),
        "and nothing raised in the trace"
    );
}

#[test]
fn a_key_the_persona_does_not_declare_is_tagged_and_a_declared_one_is_not() {
    let project = project_with_needs();
    let asks = Asks::new(project.path());
    raised_now(project.path(), &asks, &line(7, "UNDECLARED", None)).unwrap();
    raised_now(project.path(), &asks, &line(7, "DECLARED", None)).unwrap();
    let declared: Vec<(String, bool)> = asks
        .open()
        .into_iter()
        .map(|one| (one.key, one.declared.is_some()))
        .collect();
    assert_eq!(
        declared,
        [("UNDECLARED".into(), false), ("DECLARED".into(), true)]
    );
}

#[test]
fn a_fill_the_person_renamed_or_put_in_another_vault_is_heard_under_its_final_name() {
    let project = project_with_needs();
    let root = project.path();
    let asks = Asks::new(root);
    let (waits, waiting) = std::sync::mpsc::channel();
    std::thread::scope(|scope| {
        let asked = &asks;
        let chat =
            scope.spawn(move || raised_telling(root, asked, &line(7, "STRIPE", None), &waits));
        waiting.recv().unwrap();
        let id = asks.open()[0].id;
        asks.answer_as(
            &ctx(root),
            id,
            Answer::Fill(VALUE.to_owned().into()),
            Named {
                vault: Some("mine".into()),
                key: Some("STRIPE_TEST_KEY".into()),
            },
            window(),
        )
        .unwrap();
        assert_eq!(
            chat.join().unwrap(),
            Ok(Said::Filled {
                vault: "mine".into(),
                key: "STRIPE_TEST_KEY".into()
            })
        );
    });
    let ctx = ctx(root);
    let mine = crate::secrets::cmd::provider(&ctx, "mine").unwrap();
    assert_eq!(
        crate::secrets::cmd::get_value(&ctx, &mine, "STRIPE_TEST_KEY").ok(),
        Some(VALUE.to_owned())
    );
    assert_eq!(stored(root, "STRIPE"), None, "nothing under the asked name");
    assert!(asks.open().is_empty());
}

#[test]
fn a_rename_no_key_takes_is_refused_and_leaves_the_ask_open() {
    let project = project_with_needs();
    let asks = Asks::new(project.path());
    raised_now(project.path(), &asks, &line(7, "STRIPE", None)).unwrap();
    let id = asks.open()[0].id;
    let refused = asks
        .answer_as(
            &ctx(project.path()),
            id,
            Answer::Fill(VALUE.to_owned().into()),
            Named {
                vault: None,
                key: Some("not a key".into()),
            },
            window(),
        )
        .expect_err("no such key name");
    assert!(refused.contains("is not a key name"), "{refused}");
    assert_eq!(asks.open().len(), 1, "still open");
}

#[test]
fn a_declined_ask_is_said_as_declined_and_the_chat_is_not_asked_again() {
    let project = project_with_needs();
    let root = project.path();
    let asks = Asks::new(root);
    let (waits, waiting) = std::sync::mpsc::channel();
    std::thread::scope(|scope| {
        let asked = &asks;
        let chat =
            scope.spawn(move || raised_telling(root, asked, &line(7, "STRIPE", None), &waits));
        waiting.recv().unwrap();
        let id = asks.open()[0].id;
        asks.answer(&ctx(root), id, Answer::Decline, window())
            .unwrap();
        let said = chat.join().unwrap().unwrap();
        assert_eq!(
            said,
            Said::Declined {
                vault: "team".into(),
                key: "STRIPE".into(),
                reason: DECLINED_BY_THE_PERSON.into()
            }
        );
        assert_eq!(said.line(Path::new("/p")).1, 1);
    });
    assert!(matches!(
        raised_now(root, &asks, &line(7, "STRIPE", None)),
        Ok(Said::Declined { .. })
    ));
    assert!(asks.open().is_empty(), "the same chat is not asked again");
    assert!(matches!(
        raised_now(root, &asks, &line(8, "STRIPE", None)),
        Ok(Said::Pending { .. })
    ));
}

#[test]
fn a_line_that_is_not_one_is_refused_before_anything_is_asked() {
    let project = project_with_needs();
    let root = project.path();
    let asks = Asks::new(root);
    let no_key = raised_now(root, &asks, &line(7, "A B", None)).unwrap_err();
    assert!(no_key.contains("is not a key name"), "{no_key}");
    let mut blank = line(7, "KEY", None);
    blank.purpose = " \n ".into();
    assert!(raised_now(root, &asks, &blank).is_err());
    let nobody = raised_within(
        &ctx(root),
        &asks,
        Chat {
            chat: 7,
            persona: None,
            harness: None,
            asker: None,
        },
        &line(7, "KEY", None),
        Duration::ZERO,
        &|| {},
    )
    .unwrap_err();
    assert!(nobody.contains("no persona"), "{nobody}");
    // A line naming another chat than the app's record of the asking one.
    assert!(
        raised_within(
            &ctx(root),
            &asks,
            devops(8),
            &line(7, "KEY", None),
            Duration::ZERO,
            &|| {}
        )
        .is_err()
    );
    assert!(asks.open().is_empty());
}

#[test]
fn a_vault_the_persona_may_not_use_is_still_asked_and_never_answered_filled_at_once() {
    // `others` holds the key, and the persona may not use it: a chat learns nothing of what a
    // vault it could not list holds.
    let project = project_with_needs();
    let root = project.path();
    std::fs::write(root.join("others.json"), r#"{"HELD": "x"}"#).unwrap();
    let asks = Asks::new(root);
    assert_eq!(
        raised_now(root, &asks, &line(7, "HELD", Some("others"))),
        Ok(Said::Pending {
            vault: "others".into(),
            key: "HELD".into()
        })
    );
    // And the person cannot fill into it for this persona, nor over the value it holds.
    let refused = asks
        .answer(
            &ctx(root),
            asks.open()[0].id,
            Answer::Fill(VALUE.to_owned().into()),
            window(),
        )
        .expect_err("not the persona's to use");
    assert!(refused.contains("Allow it for devops first"), "{refused}");
    assert_eq!(asks.open().len(), 1, "still open");
}

/// #5246's review: a picked entry is held to the rule a typed value is. A chat that names a vault
/// its persona may not use gets no link planted in it, which that vault's own persona's tools
/// would read.
#[test]
fn a_link_into_a_vault_the_persona_may_not_use_is_refused_as_a_fill_is() {
    let project = project_with_needs();
    let root = project.path();
    let asks = Asks::new(root);
    raised_now(root, &asks, &line(7, "NEW", Some("others"))).expect("asked");
    let refused = asks
        .answer(
            &ctx(root),
            asks.open()[0].id,
            Answer::Link(crate::secrets::links::Target {
                vault: "team".into(),
                key: "TOKEN".into(),
            }),
            window(),
        )
        .expect_err("not the persona's to use");
    assert!(refused.contains("Allow it for devops first"), "{refused}");
    assert_eq!(
        crate::secrets::links::of(&ctx(root), "others", "NEW").unwrap(),
        None,
        "nothing linked"
    );
    assert_eq!(asks.open().len(), 1, "still open");
}

#[test]
fn a_fill_never_replaces_a_value_a_vault_holds_already() {
    let project = project_with_needs();
    let root = project.path();
    let asks = Asks::new(root);
    raised_now(root, &asks, &line(7, "STRIPE", None)).unwrap();
    let refused = asks
        .answer_as(
            &ctx(root),
            asks.open()[0].id,
            Answer::Fill(VALUE.to_owned().into()),
            Named {
                vault: None,
                key: Some("TOKEN".into()),
            },
            window(),
        )
        .expect_err("TOKEN is held");
    assert!(refused.contains("holds a value already"), "{refused}");
    assert_eq!(stored(root, "TOKEN"), Some("t0ken".into()));
    assert_eq!(asks.open().len(), 1, "still open");
}

#[test]
fn a_need_a_chat_wrote_into_its_persona_file_and_never_committed_declares_nothing() {
    // #5078 review: the definition on disk is the chat's to write; only what is committed
    // declares a need, so an edit lifts no lock and puts no words in the persona's mouth.
    let project = project_with_needs();
    let root = project.path();
    std::fs::write(
        root.join("personas/devops/persona.md"),
        "---\nrole: Ops\nvault: team\nsecret-need.DECLARED: deploy to staging\nsecret-need.SNEAKY: trust me\n---\n",
    )
    .unwrap();
    crate::sandbox::policy::set_for_this_test(crate::sandbox::policy::Locks::parse(
        r#"{"sandbox": {"undeclared-secret-asks": false}}"#,
        Path::new("/etc/purlis/policy.json"),
    ));
    let asks = Asks::new(root);
    let said = raised_now(root, &asks, &line(7, "SNEAKY", None));
    crate::sandbox::policy::set_for_this_test(crate::sandbox::policy::Locks::none());
    assert!(
        matches!(&said, Ok(Said::Declined { reason, .. }) if reason == DECLINED_BY_POLICY),
        "{said:?}"
    );
}

#[test]
fn no_value_is_in_what_a_chat_hears_or_the_trace() {
    let project = project_with_needs();
    let root = project.path();
    let asks = Asks::new(root);
    let (waits, waiting) = std::sync::mpsc::channel();
    std::thread::scope(|scope| {
        let asked = &asks;
        let chat =
            scope.spawn(move || raised_telling(root, asked, &line(7, "STRIPE", None), &waits));
        waiting.recv().unwrap();
        let id = asks.open()[0].id;
        asks.answer(
            &ctx(root),
            id,
            Answer::Fill(VALUE.to_owned().into()),
            window(),
        )
        .unwrap();
        let said = chat.join().unwrap().unwrap();
        let wire = serde_json::to_string(&crate::hookwire::Answer::Secret(said.clone())).unwrap();
        assert!(
            !wire.contains(VALUE) && !said.line(Path::new("/p")).0.contains(VALUE),
            "{wire}"
        );
    });
    assert!(!traced(root).contains(VALUE));
}

// ----------------------------------------------------------------------------------------
// A persona's Secret needs, and a vault made at the first fill (#5076)

#[test]
fn a_declared_keys_ask_carries_the_purpose_its_persona_declares() {
    let project = project_with_needs();
    let asks = Asks::new(project.path());
    raised_now(project.path(), &asks, &line(7, "DECLARED", None)).unwrap();
    assert_eq!(
        asks.open()[0].declared.as_deref(),
        Some("deploy to staging")
    );
}

#[test]
fn a_persona_that_names_no_vault_is_asked_into_one_named_after_it_made_at_the_first_fill() {
    let project = project_with_needs();
    let root = project.path();
    std::fs::create_dir_all(root.join("personas/shop")).unwrap();
    std::fs::write(
        root.join("personas/shop/persona.md"),
        "---\nrole: Shop\nvault: none\n---\n",
    )
    .unwrap();
    let asks = Asks::new(root);
    let shop = |chat| Chat {
        chat,
        persona: Some("shop"),
        harness: None,
        asker: None,
    };
    let (waits, waiting) = std::sync::mpsc::channel();
    let answered = std::thread::scope(|scope| {
        let asked = &asks;
        let chat = scope.spawn(move || {
            raised_within(
                &ctx(root),
                asked,
                shop(7),
                &line(7, "STRIPE_KEY", None),
                Duration::from_secs(60),
                &|| {
                    let _ = waits.send(());
                },
            )
        });
        waiting.recv().unwrap();
        let open = asks.open();
        assert_eq!(
            (open[0].vault.as_str(), open[0].names_no_vault),
            ("shop", true)
        );
        let answered = asks
            .answer(
                &ctx(root),
                open[0].id,
                Answer::Fill(VALUE.to_owned().into()),
                window(),
            )
            .expect("filled, the vault made");
        assert_eq!(
            chat.join().unwrap(),
            Ok(Said::Filled {
                vault: "shop".into(),
                key: "STRIPE_KEY".into()
            })
        );
        answered
    });
    assert!(answered.made_vault && answered.names_no_vault && !answered.declared);
    assert_eq!(answered.persona, "shop");
    let ctx = ctx(root);
    let made = crate::secrets::cmd::provider(&ctx, "shop").expect("registered");
    assert_eq!(made.provider, MADE_ON);
    assert_eq!(
        crate::secrets::cmd::get_value(&ctx, &made, "STRIPE_KEY").ok(),
        Some(VALUE.to_owned())
    );
    // Tagged for the persona, so its chats may use it.
    assert!(crate::secrets::brokered::authorise(&ctx, Some("shop"), "shop").is_ok());
    // Nothing about the secret in the definition: that is the person's proposed change.
    assert_eq!(
        std::fs::read_to_string(root.join("personas/shop/persona.md")).unwrap(),
        "---\nrole: Shop\nvault: none\n---\n"
    );
}

// ----------------------------------------------------------------------------------------
// Policy's lock on undeclared keys, and a metadata-only audit (#5078)

/// A token-shaped value, built where it is used so no source file holds one.
fn token_shaped() -> String {
    ["ghp", "_0123456789abcdefABCDEFghij0123456789"].concat()
}

/// Each step told, with the chat it is of (none for a start's).
type Told = std::sync::Arc<Mutex<Vec<(Option<u32>, crate::eventlog::secretasks::Step)>>>;

/// The steps `asks` tells, by chat, as the app writes them to the event log.
fn steps_told(asks: &Asks) -> Told {
    let told = std::sync::Arc::new(Mutex::new(Vec::new()));
    let kept = std::sync::Arc::clone(&told);
    asks.telling(std::sync::Arc::new(move |chat, step| {
        kept.lock().unwrap().push((chat, step));
    }));
    told
}

#[test]
fn policy_refuses_an_undeclared_key_as_declined_policy_and_never_asks_the_person() {
    let project = project_with_needs();
    let root = project.path();
    crate::sandbox::policy::set_for_this_test(crate::sandbox::policy::Locks::parse(
        r#"{"sandbox": {"undeclared-secret-asks": false}}"#,
        Path::new("/etc/purlis/policy.json"),
    ));
    let asks = Asks::new(root);
    let told = steps_told(&asks);
    let said = raised_now(root, &asks, &line(7, "UNDECLARED", None)).unwrap();
    assert_eq!(
        said,
        Said::Declined {
            vault: "team".into(),
            key: "UNDECLARED".into(),
            reason: DECLINED_BY_POLICY.into()
        }
    );
    assert!(asks.open().is_empty(), "the person is never asked");
    assert_eq!(
        told.lock().unwrap().as_slice(),
        [(
            Some(7),
            crate::eventlog::secretasks::Step::Refused {
                by: crate::eventlog::secretasks::By::Policy,
                declared: false
            }
        )]
    );
    // A declared key is asked as before, and a key the vault holds still answers filled.
    assert!(matches!(
        raised_now(root, &asks, &line(7, "DECLARED", None)),
        Ok(Said::Pending { .. })
    ));
    assert!(matches!(
        raised_now(root, &asks, &line(7, "TOKEN", None)),
        Ok(Said::Filled { .. })
    ));
    // And a broker miss of an undeclared key is refused the same way.
    let (held, _) = held_for_nothing(&asks, need(8, "MISSING"));
    assert!(
        matches!(&held, Held::Refused { line } if line.contains("policy")),
        "{held:?}"
    );
    crate::sandbox::policy::set_for_this_test(crate::sandbox::policy::Locks::none());
}

#[test]
fn each_step_of_an_ask_is_one_record_with_no_value_name_or_purpose() {
    use crate::eventlog::secretasks::{How, Step};
    let project = project_with_needs();
    let root = project.path();
    let asks = Asks::new(root);
    let told = steps_told(&asks);
    let (waits, waiting) = std::sync::mpsc::channel();
    std::thread::scope(|scope| {
        let asked = &asks;
        let first = scope.spawn({
            let waits = waits.clone();
            move || raised_telling(root, asked, &line(7, "STRIPE", None), &waits)
        });
        waiting.recv().unwrap();
        let second =
            scope.spawn(move || raised_telling(root, asked, &line(8, "STRIPE", None), &waits));
        waiting.recv().unwrap();
        let id = asks.open()[0].id;
        asks.answer_as(
            &ctx(root),
            id,
            Answer::Fill(VALUE.to_owned().into()),
            Named {
                vault: None,
                key: Some("STRIPE_KEY".into()),
            },
            window(),
        )
        .unwrap();
        first.join().unwrap().unwrap();
        second.join().unwrap().unwrap();
    });
    let steps = told.lock().unwrap().clone();
    let id = match steps[0].1 {
        Step::Raised { ask, .. } => ask,
        other => panic!("raised first: {other:?}"),
    };
    assert_eq!(
        steps,
        [
            (
                Some(7),
                Step::Raised {
                    ask: id,
                    declared: false
                }
            ),
            (
                Some(8),
                Step::Joined {
                    ask: id,
                    waiting: 2
                }
            ),
            (
                Some(7),
                Step::Filled {
                    ask: id,
                    how: How::Value,
                    waiting: 2,
                    renamed: true,
                    made_vault: false
                }
            ),
        ],
        "one record a step, the answer's once for every chat"
    );
    for (_, step) in steps {
        let body = step.body().to_string();
        for never in [VALUE, "STRIPE", "team", "charge test cards"] {
            assert!(
                !body.contains(never),
                "{} holds {never}: {body}",
                step.kind()
            );
        }
    }
}

#[test]
fn a_purpose_that_carries_a_secret_is_masked_before_anyone_reads_it() {
    let project = project_with_needs();
    let asks = Asks::new(project.path());
    let mut asked = line(7, "STRIPE", None);
    asked.purpose = format!("the old one was {} and stopped working", token_shaped());
    raised_now(project.path(), &asks, &asked).unwrap();
    let purpose = asks.open()[0].purpose.clone().unwrap_or_default();
    assert!(!purpose.contains(&token_shaped()), "{purpose}");
    assert!(!traced(project.path()).contains(&token_shaped()));
}

// An open ask across a restart of the app (#5079, #5054's story 59).

/// Where the project at `root` keeps its open Secret asks.
fn kept_file(root: &Path) -> PathBuf {
    crate::names::state(root).join("app").join(KEPT_IN)
}

/// Chat 7's ask for `MISSING`, which chat 9 joined, both commands having stopped waiting: what
/// the app holds as it quits.
fn two_chats_wait_on_one_ask(asks: &Asks) -> Vec<Open> {
    held_for_nothing(asks, need(7, "MISSING"));
    held_for_nothing(asks, need(9, "MISSING"));
    asks.open()
}

#[test]
fn an_open_ask_is_restored_after_a_restart_with_the_same_payload() {
    let project = project();
    let before = two_chats_wait_on_one_ask(&Asks::kept(project.path()));
    assert_eq!(before[0].chats, vec![7, 9]);
    let restarted = Asks::kept(project.path());
    assert_eq!(restarted.open(), before);
}

#[test]
fn answering_an_ask_after_a_restart_nudges_every_chat_that_waited() {
    let project = project();
    let before = two_chats_wait_on_one_ask(&Asks::kept(project.path()));
    let restarted = Asks::kept(project.path());
    let answered = restarted
        .answer(
            &ctx(project.path()),
            before[0].id,
            Answer::Fill(VALUE.to_owned().into()),
            window(),
        )
        .unwrap();
    assert_eq!(answered.nudge, vec![7, 9]);
    assert_eq!(stored(project.path(), "MISSING").as_deref(), Some(VALUE));
    // Answered, it is kept no more.
    assert!(Asks::kept(project.path()).open().is_empty());
}

#[test]
fn a_decline_after_a_restart_nudges_the_chats_and_none_is_asked_again() {
    let project = project();
    let before = two_chats_wait_on_one_ask(&Asks::kept(project.path()));
    let restarted = Asks::kept(project.path());
    let answered = restarted
        .answer(
            &ctx(project.path()),
            before[0].id,
            Answer::Decline,
            window(),
        )
        .unwrap();
    assert_eq!(answered.nudge, vec![7, 9]);
    let (held, _) = held_for_nothing(&restarted, need(7, "MISSING"));
    assert!(matches!(held, Held::Declined { .. }), "{held:?}");
}

#[test]
fn a_chat_that_retries_after_a_restart_joins_its_kept_ask_and_hears_the_fill() {
    let project = project();
    let before = two_chats_wait_on_one_ask(&Asks::kept(project.path()));
    let restarted = Asks::kept(project.path());
    let (waits, waiting) = std::sync::mpsc::channel();
    std::thread::scope(|scope| {
        let asked = &restarted;
        let retry = scope.spawn(move || asks_hold_telling(asked, need(7, "MISSING"), &waits));
        waiting.recv().unwrap();
        assert_eq!(restarted.open().len(), 1, "joined, not raised again");
        let answered = restarted
            .answer(
                &ctx(project.path()),
                before[0].id,
                Answer::Fill(VALUE.to_owned().into()),
                window(),
            )
            .unwrap();
        assert_eq!(answered.nudge, vec![9], "chat 7 heard it on its retry");
        assert!(matches!(retry.join().unwrap(), Held::Filled { .. }));
    });
}

#[test]
fn only_the_chats_a_restart_brought_back_still_wait() {
    let project = project();
    two_chats_wait_on_one_ask(&Asks::kept(project.path()));
    let restarted = Asks::kept(project.path());
    restarted.only_these_came_back(&[9]);
    assert_eq!(restarted.open()[0].chats, vec![9]);
    restarted.only_these_came_back(&[]);
    assert!(restarted.open().is_empty(), "nobody waits on it any more");
    assert!(Asks::kept(project.path()).open().is_empty());
}

#[test]
fn an_ask_raised_after_a_restart_never_takes_a_kept_asks_number() {
    let project = project();
    let before = two_chats_wait_on_one_ask(&Asks::kept(project.path()));
    let restarted = Asks::kept(project.path());
    held_for_nothing(&restarted, need(7, "OTHER"));
    let ids: Vec<u32> = restarted.open().iter().map(|one| one.id).collect();
    assert_eq!(ids.len(), 2);
    assert!(ids[1] > before[0].id, "{ids:?}");
}

#[test]
fn what_is_kept_is_masked_as_every_open_ask_is_and_never_holds_a_value() {
    let project = project();
    let asks = Asks::kept(project.path());
    held_for_nothing(&asks, need(7, "MISSING"));
    // A key whose name reads as a credential is masked out of what is kept, so the ask is not
    // kept at all: kept masked, its answer would be written under another key.
    // Built here, so the source holds nothing a secret scan reads as a token.
    let shaped = &format!("{}_{}", "ghp", "0123456789abcdefghijklmnopqrstuvwxyz");
    held_for_nothing(&asks, need(8, shaped));
    let id = asks.open()[0].id;
    asks.answer(
        &ctx(project.path()),
        id,
        Answer::Fill(VALUE.to_owned().into()),
        window(),
    )
    .unwrap();
    let kept = std::fs::read_to_string(kept_file(project.path())).unwrap();
    assert!(!kept.contains(VALUE), "{kept}");
    assert!(!kept.contains(shaped), "{kept}");
    assert!(
        !kept.contains("MISSING"),
        "answered, so no longer kept: {kept}"
    );
    let restarted = Asks::kept(project.path());
    assert!(restarted.open().is_empty(), "{:?}", restarted.open());
}

#[test]
fn a_kept_file_that_does_not_read_starts_with_no_open_ask() {
    let project = project();
    let file = kept_file(project.path());
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, "not json").unwrap();
    assert!(Asks::kept(project.path()).open().is_empty());
    // An entry whose names are not one line is left out, the rest read.
    std::fs::write(
        &file,
        serde_json::json!({"v": 1, "asked": 4, "asks": [
            {"id": 3, "persona": "devops", "vault": "team", "key": "A\nB", "since": 1, "chats": [7]},
            {"id": 4, "persona": "devops", "vault": "team", "key": "GOOD", "since": 2, "chats": [8]},
        ]})
        .to_string(),
    )
    .unwrap();
    let open = Asks::kept(project.path()).open();
    assert_eq!(open.len(), 1, "{open:?}");
    assert_eq!(
        (open[0].key.as_str(), open[0].chats.as_slice()),
        ("GOOD", &[8][..])
    );
}

// A runner chat that cannot receive a value (#5077).

#[test]
fn a_chat_whose_runner_cannot_receive_is_declined_at_once_and_the_ask_stays_for_the_person() {
    let project = project();
    let asks = Asks::new(project.path());
    let line = asks.cannot_receive(need(8, "MISSING"));
    assert_eq!(line, "declined team/MISSING: runner can't receive secrets");
    let open = asks.open();
    assert_eq!(open.len(), 1, "{open:?}");
    assert!(open[0].chats.is_empty(), "nothing waits on it: {open:?}");
    assert_eq!(open[0].cannot_receive, vec![8]);
    let answered = asks
        .answer(
            &ctx(project.path()),
            open[0].id,
            Answer::Fill(VALUE.to_owned().into()),
            window(),
        )
        .unwrap();
    assert!(answered.nudge.is_empty(), "{answered:?}");
    assert_eq!(stored(project.path(), "MISSING").as_deref(), Some(VALUE));
}

#[test]
fn a_runner_chat_that_cannot_receive_joins_a_local_chats_ask_and_only_the_local_one_is_nudged() {
    let project = project();
    let asks = Asks::new(project.path());
    held_for_nothing(&asks, need(7, "MISSING"));
    asks.cannot_receive(need(8, "MISSING"));
    let open = asks.open();
    assert_eq!(open.len(), 1, "one ask: {open:?}");
    assert_eq!(
        (open[0].chats.clone(), open[0].cannot_receive.clone()),
        (vec![7], vec![8])
    );
    let answered = asks
        .answer(
            &ctx(project.path()),
            open[0].id,
            Answer::Fill(VALUE.to_owned().into()),
            window(),
        )
        .unwrap();
    assert_eq!(answered.nudge, vec![7]);
}

#[test]
fn an_ask_only_a_closed_runner_chat_raised_goes_with_it() {
    let project = project();
    let asks = Asks::new(project.path());
    asks.cannot_receive(need(8, "MISSING"));
    asks.forget(8);
    assert!(asks.open().is_empty());
}

#[test]
fn an_ask_a_runner_that_cannot_receive_raised_is_kept_across_a_restart() {
    let project = project();
    let before = {
        let asks = Asks::kept(project.path());
        asks.cannot_receive(need(8, "MISSING"));
        asks.open()
    };
    assert_eq!(Asks::kept(project.path()).open(), before);
}

#[test]
fn a_chat_whose_runner_was_unreachable_waits_on_the_ask_is_nudged_on_its_fill_and_nothing_is_declined()
 {
    let project = project();
    let asks = Asks::new(project.path());
    assert_eq!(
        asks.unconfirmed(need(8, "MISSING")).as_deref(),
        Ok(
            "waiting on you for team/MISSING, and the runner was unreachable just now; retry once \
            answered"
        )
    );
    let open = asks.open();
    assert_eq!(open.len(), 1, "{open:?}");
    assert_eq!(open[0].chats, vec![8], "it waits: {open:?}");
    assert!(
        open[0].cannot_receive.is_empty(),
        "never declined: {open:?}"
    );
    let answered = asks
        .answer(
            &ctx(project.path()),
            open[0].id,
            Answer::Fill(VALUE.to_owned().into()),
            window(),
        )
        .unwrap();
    assert_eq!(answered.nudge, vec![8]);
}

#[test]
fn an_ask_only_a_chats_start_raised_is_kept_across_a_restart_and_no_launch_drops_it() {
    let project = project();
    let before = {
        let asks = Asks::kept(project.path());
        held_for_nothing(&asks, a_start("MISSING"));
        asks.open()
    };
    assert_eq!(before[0].starts, 1, "{before:?}");
    let restarted = Asks::kept(project.path());
    assert_eq!(restarted.open(), before);
    restarted.only_these_came_back(&[]);
    assert_eq!(
        restarted.open(),
        before,
        "a start is no chat a launch brings back"
    );
}

#[test]
fn a_kept_file_with_two_asks_of_one_number_opens_only_the_first_so_no_fill_goes_to_the_other_key() {
    let project = project();
    let file = kept_file(project.path());
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(
        &file,
        serde_json::json!({"v": 1, "asked": 3, "asks": [
            {"id": 3, "persona": "devops", "vault": "team", "key": "FIRST", "since": 1, "chats": [7]},
            {"id": 3, "persona": "devops", "vault": "team", "key": "SECOND", "since": 2, "chats": [8]},
        ]})
        .to_string(),
    )
    .unwrap();
    let open = Asks::kept(project.path()).open();
    assert_eq!(open.len(), 1, "{open:?}");
    assert_eq!(open[0].key, "FIRST");
}

#[test]
fn with_no_record_of_chats_read_no_kept_chat_number_waits_but_a_start_does() {
    let project = project();
    {
        let asks = Asks::kept(project.path());
        held_for_nothing(&asks, need(7, "MISSING"));
        asks.cannot_receive(need(8, "RUNNER"));
        held_for_nothing(&asks, a_start("STARTED"));
    }
    let restarted = Asks::kept(project.path());
    restarted.only_these_came_back(&[]);
    let open = restarted.open();
    assert_eq!(open.len(), 1, "{open:?}");
    assert_eq!((open[0].key.as_str(), open[0].starts), ("STARTED", 1));
}

#[test]
fn policy_refuses_a_runner_chats_undeclared_key_as_any_chats_and_asks_nobody() {
    let project = project();
    crate::sandbox::policy::set_for_this_test(crate::sandbox::policy::Locks::parse(
        r#"{"sandbox": {"undeclared-secret-asks": false}}"#,
        Path::new("/etc/purlis/policy.json"),
    ));
    let asks = Asks::new(project.path());
    let cannot = asks.cannot_receive(need(8, "UNDECLARED"));
    let waits = asks.unconfirmed(need(9, "UNDECLARED"));
    crate::sandbox::policy::set_for_this_test(crate::sandbox::policy::Locks::none());
    assert!(cannot.contains("policy"), "{cannot}");
    assert!(waits.is_err_and(|line| line.contains("policy")));
    assert!(asks.open().is_empty(), "the person is never asked");
}

#[test]
fn an_asks_purpose_and_what_its_persona_declares_it_for_are_kept_across_a_restart() {
    let project = project();
    let before = {
        let asks = Asks::kept(project.path());
        held_for_nothing(
            &asks,
            Need {
                purpose: Some("charge test cards".into()),
                declared: Some("the shop's payments".into()),
                ..need(7, "STRIPE_KEY")
            },
        );
        asks.open()
    };
    assert_eq!(before[0].purpose.as_deref(), Some("charge test cards"));
    assert_eq!(Asks::kept(project.path()).open(), before);
}

/// **A chat purlis started waits past the app's hold** (#5192): its start names the harness by
/// the registry's name (`claude-code`), and the asker read only the kind's word (`claude`), so
/// it took the chat for an unknown harness, waited 25 seconds, and gave up on a hold of 90.
#[test]
fn a_started_chat_waits_longer_than_the_app_holds_its_ask() {
    for (named, harness) in [
        ("claude-code", Harness::ClaudeCode),
        ("opencode", Harness::Opencode),
        ("codex", Harness::Codex),
    ] {
        let env = |name: &str| (name == crate::hookwire::HARNESS_ENV).then(|| named.to_owned());
        assert!(
            asker_waits(&env) > holds_for(Some(harness)),
            "a chat started as {named} waits {:?}, the app holds {:?}",
            asker_waits(&env),
            holds_for(Some(harness)),
        );
    }
}

// ----------------------------------------------------------------------------------------
// filled by picking an entry the person keeps: a Linked entry (#5073)

const PICKED: &str = "str1pe-v4lue";

/// [`project`], with a second vault `own`, which `devops` may use too, holding `STRIPE`: the
/// entry the person picks; and `elsewhere`, tagged for nobody, holding `PROD_PW`.
fn project_with_an_entry() -> tempfile::TempDir {
    let tmp = project();
    let root = tmp.path();
    std::fs::write(
        root.join("vaults.json"),
        serde_json::json!({ "vaults": {
            "team": {"provider": "plain-file", "config": {"file": "team.json"}, "persona": "devops"},
            "own": {"provider": "plain-file", "config": {"file": "own.json"}, "persona": "devops"},
            "elsewhere": {"provider": "plain-file", "config": {"file": "elsewhere.json"}},
        }})
        .to_string(),
    )
    .unwrap();
    std::fs::write(
        root.join("own.json"),
        serde_json::json!({"STRIPE": PICKED}).to_string(),
    )
    .unwrap();
    std::fs::write(
        root.join("elsewhere.json"),
        serde_json::json!({"PROD_PW": "pr0d-v4lue"}).to_string(),
    )
    .unwrap();
    tmp
}

/// **D-5246-1** (the person's ruling, amending D-5080-11): a press that picks an entry is judged
/// as the run's picker is, before anything is written: none of a vault the asking persona may not
/// use, and none a browser login fills, whatever the window listed.
#[test]
fn a_picked_entry_the_persona_may_not_use_or_a_browser_fills_is_refused_and_nothing_linked() {
    let project = project_with_an_entry();
    let root = project.path();
    std::fs::write(
        root.join("charter.toml"),
        "[sandbox.personas.qa.tools.shop]\nvault = \"own\"\nbrowser = \
         \"https://shop.example.test/login\"\nkey = \"STRIPE\"\n",
    )
    .unwrap();
    let asks = Asks::new(root);
    held_for_nothing(&asks, need(7, "MISSING"));
    let id = asks.open()[0].id;
    for (vault, key, says) in [
        ("elsewhere", "PROD_PW", "can't use vault elsewhere"),
        ("own", "STRIPE", "browser"),
    ] {
        let refused = asks
            .answer(
                &ctx(root),
                id,
                Answer::Link(crate::secrets::links::Target {
                    vault: vault.into(),
                    key: key.into(),
                }),
                window(),
            )
            .expect_err("refused");
        assert!(refused.contains(says), "{vault}/{key}: {refused}");
    }
    assert_eq!(
        crate::secrets::links::of(&ctx(root), "team", "MISSING").unwrap(),
        None,
        "nothing linked"
    );
    assert_eq!(asks.open().len(), 1, "still open");
}

/// **#5745** (the person's ruling, 2026-11-08: refuse such links): a link is read by every
/// persona that may use the vault it is in, through a declared tool too. So a press is refused
/// where some persona of the link's vault may not use the entry's, though the asking one may, and
/// the refusal says which persona and why.
#[test]
fn a_link_another_persona_of_its_vault_may_not_read_through_is_refused_and_says_why() {
    let project = project_with_an_entry();
    let root = project.path();
    // `qa` may use `team` on this machine, and not `own`; `devops` may use both.
    crate::sandbox::local::grant_vault(root, "team", "qa").unwrap();
    let asks = Asks::new(root);
    held_for_nothing(&asks, need(7, "MISSING"));
    let refused = asks
        .answer(&ctx(root), asks.open()[0].id, stripe_in_own(), window())
        .expect_err("qa would read own's entry through the link");
    assert!(
        refused.contains("qa may use vault team") && refused.contains("can't use vault own"),
        "{refused}"
    );
    assert_eq!(
        crate::secrets::links::of(&ctx(root), "team", "MISSING").unwrap(),
        None,
        "nothing linked"
    );
    assert_eq!(asks.open().len(), 1, "still open");
}

/// **#5745**, the issue's own case: the persona the link's vault is tagged for, not the asking one
/// (which may use it here), may not use the entry's vault. The press is refused, naming it.
#[test]
fn a_link_the_persona_its_vault_is_tagged_for_may_not_read_through_is_refused() {
    let project = project_with_an_entry();
    let root = project.path();
    std::fs::write(
        root.join("vaults.json"),
        serde_json::json!({ "vaults": {
            "team": {"provider": "plain-file", "config": {"file": "team.json"}, "persona": "qa"},
            "own": {"provider": "plain-file", "config": {"file": "own.json"}, "persona": "devops"},
        }})
        .to_string(),
    )
    .unwrap();
    // `devops` may use `team` on this machine, and `own` by its tag; `qa` may use only `team`.
    crate::sandbox::local::grant_vault(root, "team", "devops").unwrap();
    let asks = Asks::new(root);
    held_for_nothing(&asks, need(7, "MISSING"));
    let refused = asks
        .answer(&ctx(root), asks.open()[0].id, stripe_in_own(), window())
        .expect_err("qa would read own's entry through the link");
    assert!(
        refused.contains("qa may use vault team") && refused.contains("can't use vault own"),
        "{refused}"
    );
    assert_eq!(asks.open().len(), 1, "still open");
}

/// #5745's review: where the link's vault's own entry can't be read, which personas may use it
/// is not known, so the press is refused, failing closed, never taken as "none".
#[test]
fn a_link_whose_vaults_personas_cant_be_read_is_refused() {
    let project = project_with_an_entry();
    let root = project.path();
    std::fs::write(
        root.join("vaults.json"),
        serde_json::json!({ "vaults": {
            "team": {"provider": "no-such-provider", "persona": "devops"},
            "own": {"provider": "plain-file", "config": {"file": "own.json"}, "persona": "devops"},
        }})
        .to_string(),
    )
    .unwrap();
    let asks = Asks::new(root);
    held_for_nothing(&asks, need(7, "MISSING"));
    let refused = asks
        .answer(&ctx(root), asks.open()[0].id, stripe_in_own(), window())
        .expect_err("who may use team is not known");
    assert!(
        refused.contains("could not read which personas may use vault team"),
        "{refused}"
    );
    assert_eq!(asks.open().len(), 1, "still open");
}

fn stripe_in_own() -> Answer {
    Answer::Link(crate::secrets::links::Target {
        vault: "own".into(),
        key: "STRIPE".into(),
    })
}

#[test]
fn picking_an_entry_stores_a_linked_entry_and_never_a_copy() {
    let project = project_with_an_entry();
    let asks = Asks::new(project.path());
    held_for_nothing(&asks, need(7, "MISSING"));
    let id = asks.open()[0].id;
    let answered = asks
        .answer(&ctx(project.path()), id, stripe_in_own(), window())
        .unwrap();
    assert!(answered.filled, "{answered:?}");
    assert_eq!(
        crate::secrets::links::of(&ctx(project.path()), "team", "MISSING").unwrap(),
        Some(crate::secrets::links::Target {
            vault: "own".into(),
            key: "STRIPE".into()
        }),
        "the asked key is a link to the picked entry"
    );
    let team = std::fs::read_to_string(project.path().join("team.json")).unwrap();
    assert!(
        !team.contains("MISSING") && !team.contains(PICKED),
        "nothing was copied into the vault: {team}"
    );
    assert_eq!(
        stored(project.path(), "MISSING").as_deref(),
        Some(PICKED),
        "the key reads the picked entry's value"
    );
    assert!(asks.open().is_empty());
}

#[test]
fn a_waiting_command_hears_a_picked_entry_as_filled_and_reads_the_entrys_value() {
    let project = project_with_an_entry();
    let asks = Asks::new(project.path());
    std::thread::scope(|scope| {
        let waiting =
            scope.spawn(|| asks.hold(need(7, "MISSING"), Duration::from_secs(60), &mut |_| {}));
        let id = loop {
            if let Some(one) = asks.open().first() {
                break one.id;
            }
            if waiting.is_finished() {
                panic!("it ended without asking: {:?}", waiting.join());
            }
            std::thread::yield_now();
        };
        asks.answer(&ctx(project.path()), id, stripe_in_own(), window())
            .unwrap();
        assert_eq!(waiting.join().unwrap(), filled("MISSING"));
    });
    // A rotation of the entry reaches the key that reads it.
    std::fs::write(
        project.path().join("own.json"),
        serde_json::json!({"STRIPE": "r0tated"}).to_string(),
    )
    .unwrap();
    assert_eq!(
        stored(project.path(), "MISSING").as_deref(),
        Some("r0tated")
    );
}

#[test]
fn the_chat_hears_filled_with_the_keys_name_and_never_the_entry_a_link_reads() {
    let project = project_with_an_entry();
    let asks = Asks::new(project.path());
    let (_, said_while_waiting) = held_for_nothing(&asks, need(7, "MISSING"));
    let id = asks.open()[0].id;
    let answered = asks
        .answer(&ctx(project.path()), id, stripe_in_own(), window())
        .unwrap();
    assert_eq!(answered.nudge, vec![7], "the chat stopped waiting");
    let told = answered.told.line();
    assert!(told.contains("team/MISSING"), "{told}");
    let trace = traced(project.path());
    for what in [told, said_while_waiting.join("\n"), trace.clone()] {
        for target in ["own", "STRIPE", PICKED] {
            assert!(!what.contains(target), "{target} in: {what}");
        }
    }
    assert!(
        trace
            .lines()
            .any(|line| line.contains("\"secret.ask\"") && line.contains("linked")),
        "a link is traced as one, by the asked names alone: {trace}"
    );
}

#[test]
fn a_pick_the_vault_refuses_leaves_the_ask_open_and_links_nothing() {
    let project = project_with_an_entry();
    let asks = Asks::new(project.path());
    held_for_nothing(&asks, need(7, "MISSING"));
    let id = asks.open()[0].id;
    let gone = Answer::Link(crate::secrets::links::Target {
        vault: "own".into(),
        key: "NOT_THERE".into(),
    });
    asks.answer(&ctx(project.path()), id, gone, window())
        .unwrap_err();
    assert_eq!(asks.open().len(), 1, "the ask stays open");
    assert_eq!(
        crate::secrets::links::of(&ctx(project.path()), "team", "MISSING").unwrap(),
        None
    );
}

#[test]
fn only_the_window_links_an_entry() {
    let project = project_with_an_entry();
    let asks = Asks::new(project.path());
    held_for_nothing(&asks, need(7, "MISSING"));
    let id = asks.open()[0].id;
    let inbox = Answerer::admitted(Admitted::Approval).unwrap();
    asks.answer(&ctx(project.path()), id, stripe_in_own(), inbox)
        .unwrap_err();
    assert_eq!(
        crate::secrets::links::of(&ctx(project.path()), "team", "MISSING").unwrap(),
        None
    );
}

// ----------------------------------------------------------------------------------------
// replace a value that doesn't work, and a link whose entry is gone (#5075)

const SAID: &str = "the API answers 401 Unauthorized";

/// A replace ask for `key` of `team`, with the chat's reason, as the app raises it.
fn replacing(project: &Path, key: &str) -> Need {
    Need {
        reason: Some(Reason::replace(&ctx(project), "team", key, SAID)),
        no_allow: None,
        ..need(7, key)
    }
}

fn the_link(project: &Path, key: &str) -> Option<crate::secrets::links::Target> {
    crate::secrets::links::of(&ctx(project), "team", key).unwrap()
}

/// `team`'s `KEY` made a link to `own`'s `STRIPE`.
fn linked_to_stripe(project: &Path) {
    let ctx = ctx(project);
    let team = crate::secrets::cmd::provider(&ctx, "team").unwrap();
    crate::secrets::links::link(&ctx, &team, "KEY", "own", "STRIPE").unwrap();
}

#[test]
fn a_replace_ask_carries_the_chats_reason_as_its_own_words() {
    let project = project_with_an_entry();
    let asks = Asks::new(project.path());
    held_for_nothing(&asks, replacing(project.path(), "TOKEN"));
    let open = asks.open();
    assert_eq!(
        open[0].reason,
        Some(Reason::Replace {
            said: SAID.into(),
            reads: None
        })
    );
    assert_eq!(
        Reason::Replace {
            said: SAID.into(),
            reads: None
        }
        .said("TOKEN"),
        format!("the chat says the current value doesn't work: {SAID}")
    );
}

#[test]
fn a_replace_ask_filled_with_a_value_overwrites_the_value() {
    let project = project_with_an_entry();
    let asks = Asks::new(project.path());
    held_for_nothing(&asks, replacing(project.path(), "TOKEN"));
    let id = asks.open()[0].id;
    asks.answer(
        &ctx(project.path()),
        id,
        Answer::Fill(VALUE.to_owned().into()),
        window(),
    )
    .unwrap();
    assert_eq!(stored(project.path(), "TOKEN").as_deref(), Some(VALUE));
}

#[test]
fn a_replace_ask_filled_with_an_entry_links_over_the_value_it_replaces() {
    let project = project_with_an_entry();
    let asks = Asks::new(project.path());
    held_for_nothing(&asks, replacing(project.path(), "TOKEN"));
    let id = asks.open()[0].id;
    asks.answer(&ctx(project.path()), id, stripe_in_own(), window())
        .unwrap();
    assert_eq!(
        the_link(project.path(), "TOKEN"),
        Some(crate::secrets::links::Target {
            vault: "own".into(),
            key: "STRIPE".into()
        })
    );
    assert_eq!(stored(project.path(), "TOKEN").as_deref(), Some(PICKED));
    let team = std::fs::read_to_string(project.path().join("team.json")).unwrap();
    assert!(
        !team.contains("TOKEN"),
        "the value it replaced is gone: {team}"
    );
}

#[test]
fn an_ask_that_is_no_replace_never_links_over_a_value() {
    let project = project_with_an_entry();
    let asks = Asks::new(project.path());
    held_for_nothing(&asks, need(7, "TOKEN"));
    let id = asks.open()[0].id;
    asks.answer(&ctx(project.path()), id, stripe_in_own(), window())
        .unwrap_err();
    assert_eq!(the_link(project.path(), "TOKEN"), None);
    assert_eq!(stored(project.path(), "TOKEN").as_deref(), Some("t0ken"));
}

#[test]
fn a_replace_ask_on_a_link_names_the_entry_it_reads() {
    let project = project_with_an_entry();
    linked_to_stripe(project.path());
    let asks = Asks::new(project.path());
    held_for_nothing(&asks, replacing(project.path(), "KEY"));
    let reason = asks.open()[0].reason.clone().unwrap();
    let reads = crate::secrets::links::Target {
        vault: "own".into(),
        key: "STRIPE".into(),
    };
    assert_eq!(
        reason,
        Reason::Replace {
            said: SAID.into(),
            reads: Some(reads)
        }
    );
}

#[test]
fn a_replace_on_a_link_filled_with_a_value_writes_it_into_the_entry_it_reads() {
    let project = project_with_an_entry();
    linked_to_stripe(project.path());
    let asks = Asks::new(project.path());
    held_for_nothing(&asks, replacing(project.path(), "KEY"));
    let id = asks.open()[0].id;
    asks.answer(
        &ctx(project.path()),
        id,
        Answer::Fill(VALUE.to_owned().into()),
        window(),
    )
    .unwrap();
    assert!(the_link(project.path(), "KEY").is_some(), "still a link");
    let own = std::fs::read_to_string(project.path().join("own.json")).unwrap();
    assert!(own.contains(VALUE), "rotated where it lives: {own}");
    assert_eq!(stored(project.path(), "KEY").as_deref(), Some(VALUE));
}

#[test]
fn a_replace_on_a_link_filled_with_an_entry_re_points_the_link() {
    let project = project_with_an_entry();
    linked_to_stripe(project.path());
    let asks = Asks::new(project.path());
    held_for_nothing(&asks, replacing(project.path(), "KEY"));
    let id = asks.open()[0].id;
    let token = Answer::Link(crate::secrets::links::Target {
        vault: "team".into(),
        key: "TOKEN".into(),
    });
    asks.answer(&ctx(project.path()), id, token, window())
        .unwrap();
    assert_eq!(stored(project.path(), "KEY").as_deref(), Some("t0ken"));
    let own = std::fs::read_to_string(project.path().join("own.json")).unwrap();
    assert!(
        own.contains(PICKED),
        "the old entry is left as it was: {own}"
    );
}

#[test]
fn a_broken_link_ask_names_the_entry_that_is_gone_and_a_fill_gives_the_key_a_value() {
    let project = project_with_an_entry();
    linked_to_stripe(project.path());
    std::fs::write(project.path().join("own.json"), "{}").unwrap();
    let ctx = ctx(project.path());
    let reason = Reason::broken(&ctx, "team", "KEY").expect("KEY is a link");
    assert_eq!(
        reason,
        Reason::BrokenLink {
            was: crate::secrets::links::Target {
                vault: "own".into(),
                key: "STRIPE".into()
            }
        }
    );
    assert_eq!(
        reason.said("KEY"),
        "KEY was linked to STRIPE in vault own, which is gone"
    );
    let asks = Asks::new(project.path());
    let (_, said) = held_for_nothing(
        &asks,
        Need {
            reason: Some(reason),
            no_allow: None,
            ..need(7, "KEY")
        },
    );
    for line in &said {
        assert!(!line.contains("own") && !line.contains("STRIPE"), "{line}");
    }
    let id = asks.open()[0].id;
    asks.answer(&ctx, id, Answer::Fill(VALUE.to_owned().into()), window())
        .unwrap();
    assert_eq!(the_link(project.path(), "KEY"), None, "no longer a link");
    assert_eq!(stored(project.path(), "KEY").as_deref(), Some(VALUE));
}

// ----------------------------------------------------------------------------------------
// what a replace never does (#5075, the adversarial review)

/// `team`'s `TOKEN` marked as a GitHub App's key by a persona's `mint` line.
fn token_is_an_app_key(project: &Path) {
    std::fs::write(
        crate::names::manifest(project),
        r#"
[sandbox.personas.devops]
tools = { gh = { vault = "team", mint = "github-app", app = "Iv1.abc", key = "TOKEN" } }
"#,
    )
    .unwrap();
}

#[test]
fn a_replace_never_overwrites_or_links_over_a_github_apps_key() {
    let project = project_with_an_entry();
    token_is_an_app_key(project.path());
    let asks = Asks::new(project.path());
    held_for_nothing(&asks, replacing(project.path(), "TOKEN"));
    let id = asks.open()[0].id;
    asks.answer(
        &ctx(project.path()),
        id,
        Answer::Fill(VALUE.to_owned().into()),
        window(),
    )
    .unwrap_err();
    asks.answer(&ctx(project.path()), id, stripe_in_own(), window())
        .unwrap_err();
    let team = std::fs::read_to_string(project.path().join("team.json")).unwrap();
    assert!(team.contains("t0ken"), "the App's key is as it was: {team}");
    assert_eq!(the_link(project.path(), "TOKEN"), None);
}

#[test]
fn a_replace_of_a_link_re_pointed_since_the_ask_writes_nowhere() {
    let project = project_with_an_entry();
    linked_to_stripe(project.path());
    let asks = Asks::new(project.path());
    held_for_nothing(&asks, replacing(project.path(), "KEY"));
    let id = asks.open()[0].id;
    // Re-pointed before the person answers: the window showed STRIPE in own.
    let ctx = ctx(project.path());
    let team = crate::secrets::cmd::provider(&ctx, "team").unwrap();
    crate::secrets::links::link(&ctx, &team, "KEY", "team", "TOKEN").unwrap();
    asks.answer(&ctx, id, Answer::Fill(VALUE.to_owned().into()), window())
        .unwrap_err();
    let written = std::fs::read_to_string(project.path().join("team.json")).unwrap()
        + &std::fs::read_to_string(project.path().join("own.json")).unwrap();
    assert!(!written.contains(VALUE), "{written}");
    assert_eq!(asks.open().len(), 1, "the ask stays open");
}

#[test]
fn picking_an_entry_mends_a_broken_link() {
    let project = project_with_an_entry();
    linked_to_stripe(project.path());
    std::fs::write(project.path().join("own.json"), "{}").unwrap();
    let ctx = ctx(project.path());
    let asks = Asks::new(project.path());
    held_for_nothing(
        &asks,
        Need {
            reason: Reason::broken(&ctx, "team", "KEY"),
            no_allow: None,
            ..need(7, "KEY")
        },
    );
    let id = asks.open()[0].id;
    let token = Answer::Link(crate::secrets::links::Target {
        vault: "team".into(),
        key: "TOKEN".into(),
    });
    asks.answer(&ctx, id, token, window()).unwrap();
    assert_eq!(stored(project.path(), "KEY").as_deref(), Some("t0ken"));
}

#[test]
fn a_chat_joining_an_ask_never_changes_why_it_was_asked() {
    let project = project_with_an_entry();
    let asks = Asks::new(project.path());
    held_for_nothing(&asks, need(7, "MISSING"));
    held_for_nothing(
        &asks,
        Need {
            reason: Some(Reason::replace(
                &ctx(project.path()),
                "team",
                "MISSING",
                SAID,
            )),
            ..need(8, "MISSING")
        },
    );
    assert_eq!(asks.open()[0].reason, None);
}

// ----------------------------------------------------------------------------------------
// a chat asks for a replacement: `secret_ask` with `replace` (#5075)

/// [`line`] asking to replace `key`, saying why.
fn replacing_line(chat: u32, key: &str, vault: Option<&str>, why: &str) -> SecretAsk {
    SecretAsk {
        replace: Some(why.into()),
        ..line(chat, key, vault)
    }
}

#[test]
fn a_replace_raises_an_ask_even_for_a_key_the_vault_holds_with_the_chats_reason() {
    let project = project_with_needs();
    let asks = Asks::new(project.path());
    let said = raised_now(
        project.path(),
        &asks,
        &replacing_line(7, "TOKEN", None, "  the API answers\n401 Unauthorized "),
    )
    .unwrap();
    assert_eq!(
        said,
        Said::Pending {
            vault: "team".into(),
            key: "TOKEN".into()
        },
        "never filled at once: the person is asked"
    );
    let open = asks.open();
    assert_eq!(
        open[0].reason,
        Some(Reason::Replace {
            said: "the API answers 401 Unauthorized".into(),
            reads: None
        })
    );
}

#[test]
fn a_replace_filled_by_the_person_overwrites_the_value_and_the_chat_hears_filled() {
    let project = project_with_needs();
    let asks = Asks::new(project.path());
    let said = std::thread::scope(|scope| {
        let asking = scope.spawn(|| {
            raised_within(
                &ctx(project.path()),
                &asks,
                devops(7),
                &replacing_line(7, "TOKEN", None, "401"),
                Duration::from_secs(60),
                &|| {},
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
        asking.join().unwrap().unwrap()
    });
    assert_eq!(
        said,
        Said::Filled {
            vault: "team".into(),
            key: "TOKEN".into()
        }
    );
    assert_eq!(stored(project.path(), "TOKEN").as_deref(), Some(VALUE));
}

#[test]
fn a_replace_in_a_vault_the_persona_may_not_use_is_refused_and_asks_nothing() {
    let project = project_with_needs();
    let asks = Asks::new(project.path());
    let refused = raised_now(
        project.path(),
        &asks,
        &replacing_line(7, "TOKEN", Some("others"), "401"),
    )
    .unwrap_err();
    assert!(refused.contains("others"), "{refused}");
    assert!(asks.open().is_empty());
}

#[test]
fn a_replace_that_says_no_reason_is_refused() {
    let project = project_with_needs();
    let asks = Asks::new(project.path());
    raised_now(
        project.path(),
        &asks,
        &replacing_line(7, "TOKEN", None, "   "),
    )
    .unwrap_err();
    assert!(asks.open().is_empty());
}

#[test]
fn a_secret_ask_line_without_replace_still_reads() {
    let old: SecretAsk =
        serde_json::from_value(serde_json::json!({"chat": 7, "key": "K", "purpose": "p"})).unwrap();
    assert_eq!(old.replace, None);
    let sent = serde_json::to_value(line(7, "K", None)).unwrap();
    assert!(sent.get("replace").is_none(), "{sent}");
}

#[test]
fn a_replace_ask_keeps_its_reason_across_a_restart() {
    let project = project_with_an_entry();
    linked_to_stripe(project.path());
    let before = {
        let asks = Asks::kept(project.path());
        held_for_nothing(&asks, replacing(project.path(), "KEY"));
        asks.open()
    };
    assert!(before[0].reason.is_some());
    assert_eq!(Asks::kept(project.path()).open(), before);
}

#[test]
fn a_secret_ask_that_waits_says_where_the_person_answers_and_that_it_stays_answerable() {
    let project = project();
    let name = project
        .path()
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    let asks = Asks::new(project.path());
    let (held, said) = held_for_nothing(&asks, need(7, "MISSING"));
    assert_eq!(
        said,
        [format!(
            "purlis asked the person for team/MISSING, which the vault doesn't hold; waiting for \
             the person: Asks (✋) in the purlis window, project {name}, for up to 0 seconds"
        )]
    );
    assert_eq!(
        held,
        Held::StillOpen {
            line: format!(
                "waiting on you for team/MISSING; the ask is still answerable in Asks (✋) in \
                 the purlis window, project {name}; retry once answered"
            )
        }
    );
}
