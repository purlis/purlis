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
        purpose: None,
        declared: None,
        names_no_vault: false,
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

// ----------------------------------------------------------------------------------------
// A chat asks: the `secret_ask` line (#5072)

/// [`project`], with persona `devops` declaring `DECLARED` as a Secret need, a second vault
/// `mine` (a plain file of its own, tagged for nobody) and `charter.toml`.
fn project_with_needs() -> tempfile::TempDir {
    let tmp = project();
    let root = tmp.path();
    std::fs::write(root.join("charter.toml"), "").unwrap();
    std::fs::create_dir_all(root.join("personas/devops")).unwrap();
    std::fs::write(
        root.join("personas/devops/persona.md"),
        "---\nrole: Ops\nsecret-need.DECLARED: deploy to staging\n---\n",
    )
    .unwrap();
    std::fs::write(
        root.join("vaults.json"),
        serde_json::json!({ "vaults": {
            "team": {"provider": "plain-file", "config": {"file": "team.json"}, "persona": "devops"},
            "mine": {"provider": "plain-file", "config": {"file": "mine.json"}},
        }})
        .to_string(),
    )
    .unwrap();
    std::fs::write(root.join("mine.json"), "{}").unwrap();
    tmp
}

fn devops(chat: u32) -> Chat<'static> {
    Chat {
        chat,
        persona: Some("devops"),
        harness: None,
    }
}

fn line(chat: u32, key: &str, vault: Option<&str>) -> SecretAsk {
    SecretAsk {
        chat,
        key: key.into(),
        purpose: "  charge test cards\nin the checkout e2e ".into(),
        vault: vault.map(str::to_owned),
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
    let (told, code) = said.unwrap().line();
    assert_eq!(code, super::super::brokered::WAITING_ON_THE_PERSON);
    assert_eq!(
        told,
        "waiting on you for team/STRIPE_KEY; retry once answered"
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
        assert_eq!(said.line().1, 1);
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
    // `mine` holds the key, and the persona may not use it: a chat learns nothing of what a
    // vault it could not list holds. The person decides what to put where.
    let project = project_with_needs();
    let root = project.path();
    std::fs::write(root.join("mine.json"), r#"{"HELD": "x"}"#).unwrap();
    let asks = Asks::new(root);
    assert_eq!(
        raised_now(root, &asks, &line(7, "HELD", Some("mine"))),
        Ok(Said::Pending {
            vault: "mine".into(),
            key: "HELD".into()
        })
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
            !wire.contains(VALUE) && !said.line().0.contains(VALUE),
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
