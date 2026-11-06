// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **One notice for a run's gates** (#5242, ADR 0112): a run that misses keys, a program's
//! allowance or vault access raises one notice with whichever it hit, and nothing beside it.

use super::PolicySays as Policy;
use super::*;
use crate::secrets::Zeroizing;
use crate::secrets::ask::Asks;

/// `wanted` run with `asks`, holding nobody: it asks and stops waiting at once.
fn asked(asker: &Asker, wanted: Wanted, asks: &Asks) -> Vec<Frame> {
    crate::sandbox::tools::SEARCH_HERE
        .with(|dirs| *dirs.borrow_mut() = Some(vec!["/usr/bin".into(), "/bin".into()]));
    let (reader, _keep) = held_open();
    let wire = Wire::default();
    serve_asking(
        asker,
        wanted,
        reader,
        Box::new(wire.clone()),
        Wrap::Unwrapped,
        Missing {
            asks,
            within: Duration::ZERO,
            moved: &|| {},
            run_gate: None,
        },
    );
    let bytes = wire.0.lock().unwrap().clone();
    String::from_utf8(bytes)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).expect("every line is a frame"))
        .collect()
}

/// `printenv X` handing over `keys` of vault `team`, one variable each.
fn printenv_of(root: &Path, keys: &[&str]) -> Wanted {
    let mut want = wanted(root, "team", vec!["printenv".into(), "X".into()]);
    want.env = keys
        .iter()
        .enumerate()
        .map(|(n, key)| format!("V{n}={key}"))
        .collect();
    want
}

/// How many notices the person sees now: each open ask, of either kind.
fn notices(asks: &Asks) -> usize {
    asks.open().len() + asks.open_runs().len()
}

#[test]
fn an_undeclared_program_missing_a_key_is_one_notice_with_its_field_and_the_allow() {
    let project = project();
    let asks = Asks::new(project.path());
    let frames = asked(
        &asker(project.path(), Some("devops")),
        printenv_of(project.path(), &["TOKEN", "NOT_THERE_YET"]),
        &asks,
    );
    assert_eq!(notices(&asks), 1, "{frames:?}");
    let open = asks.open_runs();
    assert_eq!(
        open[0]
            .fills
            .iter()
            .map(|f| f.key.as_str())
            .collect::<Vec<_>>(),
        ["NOT_THERE_YET"],
        "a field for the missing key alone"
    );
    assert_eq!(
        open[0].keys(),
        ["NOT_THERE_YET", "TOKEN"],
        "the grant covers exactly its keys"
    );
}

/// `asker`, its start having armed `wanted`'s program as a tool of its persona with every
/// hand-over `wanted` names, as though the persona declared it and this machine allowed it.
fn armed(mut asker: Asker, wanted: &Wanted) -> Asker {
    use crate::sandbox::tools::{Handed, Ready, Tool, Value};
    let values = wanted
        .env
        .iter()
        .filter_map(|spec| spec.split_once('='))
        .map(|(var, key)| Value {
            var: var.to_owned(),
            handed: Handed::Env,
            key: key.to_owned(),
        })
        .collect();
    let mut tool = Tool::one("printenv", &wanted.vault, "unused", Handed::Env, "unused");
    tool.values = values;
    asker
        .confines
        .as_mut()
        .expect("sandboxed")
        .tools
        .push(Ready {
            tool,
            program: "/usr/bin/printenv".into(),
        });
    asker
}

/// What a person sees for one run of `vault`: each ask open, and the app's own refused vault's
/// notice (#5160), which it raises where the persona may not use the vault and the gate did not
/// take the run.
fn seen(root: &Path, vault: &str, asks: &Asks, gate: &RunGate) -> usize {
    let refused = authorise(&Ctx::new(root, Env::of(&[])), Some("devops"), vault).is_err();
    let vault_notice = refused && !gate.took_it.load(std::sync::atomic::Ordering::Relaxed);
    notices(asks) + usize::from(vault_notice)
}

/// `wanted` run by `asker` with `gate`, holding nobody.
fn at_the_gate(asker: &Asker, wanted: Wanted, asks: &Asks, gate: &RunGate) -> Vec<Frame> {
    crate::sandbox::tools::SEARCH_HERE
        .with(|dirs| *dirs.borrow_mut() = Some(vec!["/usr/bin".into(), "/bin".into()]));
    let (reader, _keep) = held_open();
    let wire = Wire::default();
    serve_asking(
        asker,
        wanted,
        reader,
        Box::new(wire.clone()),
        Wrap::Unwrapped,
        Missing {
            asks,
            within: Duration::ZERO,
            moved: &|| {},
            run_gate: Some(gate),
        },
    );
    let bytes = wire.0.lock().unwrap().clone();
    String::from_utf8(bytes)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).expect("every line is a frame"))
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Program {
    Undeclared,
    Declared,
    Locked,
}

#[test]
fn every_combination_of_the_gates_gives_exactly_one_notice() {
    let mut wrong = Vec::new();
    for program in [Program::Undeclared, Program::Declared, Program::Locked] {
        let _policy = (program == Program::Locked)
            .then(|| Policy::says(r#"{"sandbox": {"run-grants": false}}"#));
        for vault in ["team", "other"] {
            for keys in [
                &["TOKEN"][..],
                &["TOKEN", "NOT_A"][..],
                &["NOT_A", "NOT_B"][..],
            ] {
                let project = project();
                let root = project.path();
                let asks = Asks::new(root);
                let mut want = printenv_of(root, keys);
                want.vault = vault.into();
                let chat = asker(root, Some("devops"));
                let chat = if program == Program::Declared {
                    armed(chat, &want)
                } else {
                    chat
                };
                let gate = RunGate::default();
                let frames = at_the_gate(&chat, want, &asks, &gate);
                let missing = keys.iter().filter(|key| key.starts_with("NOT_")).count();
                // Nothing to ask: a declared tool runs, and a locked run is refused saying why.
                let gates = missing > 0 || vault == "other" || program == Program::Undeclared;
                if seen(root, vault, &asks, &gate) != usize::from(gates) {
                    wrong.push(format!(
                        "{program:?} on {vault} missing {missing}: {frames:?} {:?} {:?}",
                        asks.open(),
                        asks.open_runs()
                    ));
                }
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n\n"));
}

fn window() -> crate::harness::asks::Answerer {
    crate::harness::asks::Answerer::admitted(crate::harness::asks::Admitted::LocalUi).unwrap()
}

/// The person, for a run of `asker`'s that waits: each notice that comes up, answered yes as it
/// comes (every value filled, the program allowed once, the vault allowed), counted, until the
/// run ends. What the run came to, and how many notices it took.
fn answered_yes(asker: &Asker, want: Wanted, asks: &Asks) -> (Vec<Frame>, usize) {
    use crate::secrets::ask::run::{Offer, RunAnswer};
    let root = asker.root.clone();
    let ctx = Ctx::new(&root, Env::of(&[]));
    let gate = RunGate::default();
    std::thread::scope(|scope| {
        let run = scope.spawn(|| {
            crate::sandbox::tools::SEARCH_HERE
                .with(|dirs| *dirs.borrow_mut() = Some(vec!["/usr/bin".into(), "/bin".into()]));
            let (reader, _keep) = held_open();
            let wire = Wire::default();
            serve_asking(
                asker,
                want,
                reader,
                Box::new(wire.clone()),
                Wrap::Unwrapped,
                Missing {
                    asks,
                    within: Duration::from_secs(60),
                    moved: &|| {},
                    run_gate: Some(&gate),
                },
            );
            let bytes = wire.0.lock().unwrap().clone();
            String::from_utf8(bytes)
                .unwrap()
                .lines()
                .map(|line| serde_json::from_str::<Frame>(line).expect("a frame"))
                .collect::<Vec<_>>()
        });
        let mut seen = 0;
        while !run.is_finished() {
            if let Some(open) = asks.open().into_iter().next() {
                seen += 1;
                asks.answer(
                    &ctx,
                    open.id,
                    crate::secrets::ask::Answer::Fill(Zeroizing::new("f1lled".into())),
                    window(),
                )
                .unwrap();
            } else if let Some(open) = asks.open_runs().into_iter().next() {
                seen += 1;
                let values = open
                    .fills
                    .iter()
                    .map(|fill| (fill.key.clone(), Zeroizing::new("f1lled".to_owned())))
                    .collect();
                asks.fill_run(&ctx, open.id, values, window()).unwrap();
                let answer = match open.offer {
                    Offer::Program => RunAnswer::Once,
                    Offer::Vault { .. } => {
                        // As the window's vault Allow does (#5160).
                        crate::sandbox::local::grant_vault(&root, &open.bound.vault, "devops")
                            .unwrap();
                        RunAnswer::AllowVault
                    }
                    Offer::Fill { .. } => RunAnswer::Fill,
                };
                asks.answer_run(open.id, answer, window()).unwrap();
            } else {
                std::thread::yield_now();
            }
        }
        (run.join().unwrap(), seen)
    })
}

#[test]
fn every_combination_of_the_gates_takes_one_answer_and_the_run_goes_on() {
    let mut wrong = Vec::new();
    for program in [Program::Undeclared, Program::Declared] {
        for vault in ["team", "other"] {
            for keys in [
                &["TOKEN"][..],
                &["TOKEN", "NOT_A"][..],
                &["NOT_A", "NOT_B"][..],
            ] {
                let project = project();
                let root = project.path();
                let asks = Asks::new(root);
                let mut want = printenv_of(root, keys);
                want.vault = vault.into();
                let chat = asker(root, Some("devops"));
                let chat = if program == Program::Declared {
                    armed(chat, &want)
                } else {
                    chat
                };
                let (frames, seen) = answered_yes(&chat, want, &asks);
                let missing = keys.iter().filter(|key| key.starts_with("NOT_")).count();
                // A declared tool's refused vault with nothing missing is the app's own notice.
                let expected = usize::from(missing > 0 || program == Program::Undeclared);
                let waits = matches!(frames.last(), Some(Frame::Refused { code, .. }) if *code == WAITING_ON_THE_PERSON);
                if seen != expected || waits {
                    wrong.push(format!(
                        "{program:?} on {vault} missing {missing}: {seen} notices, {frames:?}"
                    ));
                }
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n\n"));
}

/// `want` run by `chat`, with `answer` made once its notice is up; what the run came to.
fn answering(
    chat: &Asker,
    want: Wanted,
    asks: &Asks,
    answer: impl FnOnce(&crate::secrets::ask::run::OpenRun),
) -> Vec<Frame> {
    std::thread::scope(|scope| {
        let run = scope.spawn(|| {
            crate::sandbox::tools::SEARCH_HERE
                .with(|dirs| *dirs.borrow_mut() = Some(vec!["/usr/bin".into(), "/bin".into()]));
            let (reader, _keep) = held_open();
            let wire = Wire::default();
            serve_asking(
                chat,
                want,
                reader,
                Box::new(wire.clone()),
                Wrap::Unwrapped,
                Missing {
                    asks,
                    within: Duration::from_secs(60),
                    moved: &|| {},
                    run_gate: None,
                },
            );
            let bytes = wire.0.lock().unwrap().clone();
            String::from_utf8(bytes)
                .unwrap()
                .lines()
                .map(|line| serde_json::from_str::<Frame>(line).expect("a frame"))
                .collect::<Vec<_>>()
        });
        let open = loop {
            if let Some(open) = asks.open_runs().into_iter().next() {
                break open;
            }
            assert!(!run.is_finished(), "the run ended without asking");
            std::thread::yield_now();
        };
        answer(&open);
        run.join().unwrap()
    })
}

fn stored(root: &Path, file: &str) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(root.join(file)).unwrap()).unwrap()
}

#[test]
fn dont_allow_stores_the_values_and_the_chat_hears_filled_but_not_allowed() {
    use crate::secrets::ask::run::RunAnswer;
    let project = project();
    let root = project.path();
    let asks = Asks::new(root);
    let ctx = Ctx::new(root, Env::of(&[]));
    let frames = answering(
        &asker(root, Some("devops")),
        printenv_of(root, &["NOT_A", "NOT_B"]),
        &asks,
        |open| {
            assert_eq!(open.fills.len(), 2, "two fields, one notice");
            assert_eq!(
                open.keys(),
                ["NOT_A", "NOT_B"],
                "the grant covers exactly its keys"
            );
            asks.fill_run(
                &ctx,
                open.id,
                vec![
                    ("NOT_A".into(), Zeroizing::new("v4lue-a".into())),
                    ("NOT_B".into(), Zeroizing::new("v4lue-b".into())),
                ],
                window(),
            )
            .unwrap();
            asks.answer_run(open.id, RunAnswer::Refuse, window())
                .unwrap();
        },
    );
    let Some(Frame::Refused { why, .. }) = frames.last() else {
        panic!("{frames:?}")
    };
    assert!(
        why.contains("filled") && why.contains("did not allow"),
        "{why}"
    );
    assert!(!why.contains("v4lue"), "never a value: {why}");
    let team = stored(root, "team.json");
    assert_eq!(team["NOT_A"], "v4lue-a", "{team}");
    assert_eq!(team["NOT_B"], "v4lue-b", "{team}");
}

#[test]
fn an_allow_with_a_field_left_empty_is_refused_and_the_notice_stays_up() {
    use crate::secrets::ask::run::RunAnswer;
    let project = project();
    let root = project.path();
    let asks = Asks::new(root);
    asked(
        &asker(root, Some("devops")),
        printenv_of(root, &["NOT_A", "NOT_B"]),
        &asks,
    );
    let open = asks.open_runs().remove(0);
    asks.fill_run(
        &Ctx::new(root, Env::of(&[])),
        open.id,
        vec![("NOT_A".into(), Zeroizing::new("v4lue-a".into()))],
        window(),
    )
    .unwrap();
    let refused = asks
        .answer_run(open.id, RunAnswer::Once, window())
        .unwrap_err();
    assert!(refused.contains("NOT_B"), "{refused}");
    assert_eq!(asks.open_runs().len(), 1, "still up");
    assert!(asks.given_runs().is_empty(), "nothing was given");
}

#[test]
fn a_declared_tools_refused_vault_with_a_missing_key_offers_the_vaults_own_allow_and_goes_on() {
    use crate::secrets::ask::run::{Offer, RunAnswer};
    let project = project();
    let root = project.path();
    let asks = Asks::new(root);
    let mut want = printenv_of(root, &["NOT_A"]);
    want.vault = "other".into();
    let chat = armed(asker(root, Some("devops")), &want);
    let frames = answering(&chat, want, &asks, |open| {
        assert_eq!(
            open.offer,
            Offer::Vault { no_allow: None },
            "no program Allow"
        );
        asks.fill_run(
            &Ctx::new(root, Env::of(&[])),
            open.id,
            vec![("NOT_A".into(), Zeroizing::new("v4lue-a".into()))],
            window(),
        )
        .unwrap();
        // As the window's vault Allow does (#5160).
        crate::sandbox::local::grant_vault(root, "other", "devops").unwrap();
        asks.answer_run(open.id, RunAnswer::AllowVault, window())
            .unwrap();
        // Nothing a program Allow would answer.
        assert!(asks.given_runs().is_empty());
    });
    assert!(
        !format!("{frames:?}").contains("did not open it"),
        "past the vault: {frames:?}"
    );
    assert_eq!(stored(root, "other.json")["NOT_A"], "v4lue-a");
}

#[test]
fn a_declared_tools_vault_kept_blocked_stores_nothing_and_the_run_hears_so() {
    use crate::secrets::ask::run::RunAnswer;
    let project = project();
    let root = project.path();
    let asks = Asks::new(root);
    let mut want = printenv_of(root, &["NOT_A"]);
    want.vault = "other".into();
    let chat = armed(asker(root, Some("devops")), &want);
    let frames = answering(&chat, want, &asks, |open| {
        asks.answer_run(open.id, RunAnswer::Refuse, window())
            .unwrap();
    });
    assert!(
        matches!(frames.last(), Some(Frame::Refused { code: 1, .. })),
        "{frames:?}"
    );
    assert!(stored(root, "other.json").get("NOT_A").is_none());
}

#[test]
fn not_now_on_a_notice_of_fields_alone_declines_them_for_the_chat() {
    use crate::secrets::ask::run::RunAnswer;
    let project = project();
    let root = project.path();
    let asks = Asks::new(root);
    let want = || printenv_of(root, &["NOT_A", "NOT_B"]);
    let chat = armed(asker(root, Some("devops")), &want());
    answering(&chat, want(), &asks, |open| {
        asks.answer_run(open.id, RunAnswer::Refuse, window())
            .unwrap();
    });
    let again = asked(&chat, want(), &asks);
    assert!(notices(&asks) == 0, "not asked again: {again:?}");
    assert!(format!("{again:?}").contains("declined"), "{again:?}");
}

#[test]
fn a_program_allow_is_not_an_answer_on_a_notice_with_none() {
    use crate::secrets::ask::run::RunAnswer;
    let project = project();
    let root = project.path();
    let asks = Asks::new(root);
    let want = printenv_of(root, &["NOT_A", "NOT_B"]);
    let chat = armed(asker(root, Some("devops")), &want);
    asked(&chat, want, &asks);
    let open = asks.open_runs().remove(0);
    // Every field filled, so nothing else refuses them.
    asks.fill_run(
        &Ctx::new(root, Env::of(&[])),
        open.id,
        vec![
            ("NOT_A".into(), Zeroizing::new("a".into())),
            ("NOT_B".into(), Zeroizing::new("b".into())),
        ],
        window(),
    )
    .unwrap();
    for answer in [
        RunAnswer::Once,
        RunAnswer::ForChat,
        RunAnswer::Always,
        RunAnswer::AllowVault,
    ] {
        assert!(
            asks.answer_run(open.id, answer, window()).is_err(),
            "{answer:?}"
        );
    }
    assert!(asks.given_runs().is_empty());
}

// ----------------------------------------------------------------------------------------
// An ask ahead of a run that names its program (#5244; ADR 0112, amended 2026-11-06)

/// `asker`'s `secret_ask` for `key`, naming `program`, holding nobody.
fn asked_ahead(asker: &Asker, asks: &Asks, key: &str, program: &str) -> crate::secrets::ask::Said {
    crate::sandbox::tools::SEARCH_HERE
        .with(|dirs| *dirs.borrow_mut() = Some(vec!["/usr/bin".into(), "/bin".into()]));
    crate::secrets::ask::raised_within(
        &Ctx::new(&asker.root, Env::of(&[])),
        asks,
        crate::secrets::ask::Chat {
            chat: asker.chat,
            persona: asker.persona.as_deref(),
            harness: asker.harness,
            asker: Some(asker),
        },
        &crate::secrets::ask::SecretAsk {
            chat: asker.chat,
            key: key.to_owned(),
            purpose: "calling the shop's API".to_owned(),
            vault: None,
            replace: None,
            program: Some(program.to_owned()),
        },
        Duration::ZERO,
        &|| {},
    )
    .expect("asked")
}

#[test]
fn an_ask_ahead_naming_curl_is_one_notice_with_its_field_and_allow_for_this_chat_alone() {
    use crate::secrets::ask::run::{Offer, RunAnswer};
    let project = project();
    let root = project.path();
    let asks = Asks::new(root);
    let chat = asker(root, Some("devops"));
    let said = asked_ahead(&chat, &asks, "NOT_A", "curl");
    assert!(
        matches!(said, crate::secrets::ask::Said::Pending { .. }),
        "{said:?}"
    );
    assert!(asks.open().is_empty(), "no Fill beside it");
    let open = asks.open_runs().remove(0);
    assert!(open.ahead, "{open:?}");
    assert_eq!(open.offer, Offer::Program);
    assert_eq!(open.fills[0].key, "NOT_A");
    asks.fill_run(
        &Ctx::new(root, Env::of(&[])),
        open.id,
        vec![("NOT_A".into(), Zeroizing::new("v4lue".into()))],
        window(),
    )
    .unwrap();
    for narrower_or_wider in [RunAnswer::Once, RunAnswer::Always] {
        assert!(
            asks.answer_run(open.id, narrower_or_wider, window())
                .is_err(),
            "{narrower_or_wider:?} is not offered ahead: there is no command to bind"
        );
    }
    asks.answer_run(open.id, RunAnswer::ForChat, window())
        .unwrap();
    assert_eq!(stored(root, "team.json")["NOT_A"], "v4lue");
}

#[test]
fn the_grant_ahead_covers_the_first_run_with_those_keys_and_from_then_on_only_its_variables() {
    use crate::secrets::ask::run::RunAnswer;
    let project = project();
    let root = project.path();
    let asks = Asks::new(root);
    let chat = asker(root, Some("devops"));
    asked_ahead(&chat, &asks, "NOT_A", "curl");
    let open = asks.open_runs().remove(0);
    asks.fill_run(
        &Ctx::new(root, Env::of(&[])),
        open.id,
        vec![("NOT_A".into(), Zeroizing::new("v4lue".into()))],
        window(),
    )
    .unwrap();
    asks.answer_run(open.id, RunAnswer::ForChat, window())
        .unwrap();
    let run = |var: &str, key: &str, args: &[&str]| {
        let mut bound = open.bound.clone();
        bound.env = vec![format!("{var}={key}")];
        bound.args = args.iter().map(|arg| (*arg).to_owned()).collect();
        bound.cwd = root.join("work");
        bound
    };
    assert!(!asks.covers(&run("X", "OTHER", &[])), "exactly its keys");
    assert!(asks.covers(&run("X", "NOT_A", &["https://a.example"])));
    assert!(
        asks.covers_for_the_chat(&run("X", "NOT_A", &["https://a.example"])),
        "a grant for the chat's life, so policy's scopes hold to its first run"
    );
    assert!(!asks.covers_for_the_chat(&run("X", "OTHER", &[])));
    assert!(
        asks.granted(&run("X", "NOT_A", &["https://a.example"])),
        "the first run"
    );
    assert!(
        asks.covers(&run("X", "NOT_A", &["https://b.example"])),
        "any arguments"
    );
    assert!(
        !asks.covers(&run("Y", "NOT_A", &[])),
        "the first run fixed how the value is handed"
    );
}

#[test]
fn an_ask_ahead_naming_a_program_not_reviewed_is_fill_only_and_its_run_asks() {
    let project = project();
    let root = project.path();
    let asks = Asks::new(root);
    let said = asked_ahead(&asker(root, Some("devops")), &asks, "NOT_A", "printenv");
    assert!(
        matches!(said, crate::secrets::ask::Said::Pending { .. }),
        "{said:?}"
    );
    assert!(asks.open_runs().is_empty(), "no Allow ahead");
    let open = asks.open();
    assert_eq!(open.len(), 1);
    assert!(
        open[0]
            .no_allow
            .as_deref()
            .is_some_and(|why| why.contains("exact command")),
        "{open:?}"
    );
}

#[test]
fn an_interpreter_named_ahead_is_fill_only_saying_why() {
    let project = project();
    let root = project.path();
    let asks = Asks::new(root);
    asked_ahead(&asker(root, Some("devops")), &asks, "NOT_A", "python3");
    assert!(asks.open_runs().is_empty());
    let open = asks.open();
    assert_eq!(open.len(), 1);
    assert!(open[0].no_allow.is_some(), "says why no Allow: {open:?}");
}

#[test]
fn a_program_refused_in_this_chat_named_ahead_is_refused_at_once() {
    use crate::secrets::ask::run::RunAnswer;
    let project = project();
    let root = project.path();
    let asks = Asks::new(root);
    let chat = asker(root, Some("devops"));
    asked_ahead(&chat, &asks, "NOT_A", "curl");
    let open = asks.open_runs().remove(0);
    asks.answer_run(open.id, RunAnswer::Refuse, window())
        .unwrap();
    let said = asked_ahead(&chat, &asks, "NOT_B", "curl");
    assert!(
        matches!(said, crate::secrets::ask::Said::NotAllowed { .. }),
        "{said:?}"
    );
    assert_eq!(notices(&asks), 0, "nothing asked");
    let (line, code) = said.line();
    assert_eq!(code, 1);
    assert!(line.contains("did not allow curl"), "{line}");
}

#[test]
fn no_filled_answer_sends_the_chat_to_a_refusal() {
    use crate::secrets::ask::Said;
    let filled = Said::Filled {
        vault: "team".into(),
        key: "TOKEN".into(),
    };
    let (line, code) = filled.line();
    assert_eq!(code, 0);
    assert!(
        line.contains("asks the person to allow it"),
        "it promises only what works: {line}"
    );
    let allowed = Said::FilledAndAllowed {
        vault: "team".into(),
        key: "TOKEN".into(),
        program: "curl".into(),
    };
    assert!(
        allowed.line().0.contains("allowed curl"),
        "{:?}",
        allowed.line()
    );
    let not = Said::FilledNotAllowed {
        vault: "team".into(),
        key: "TOKEN".into(),
        program: "curl".into(),
    };
    let (line, code) = not.line();
    assert_eq!(code, 1);
    assert!(
        line.contains("did not allow curl") && line.contains("filled"),
        "{line}"
    );
}

#[test]
fn an_ask_ahead_names_its_keys_to_every_check_and_to_the_audit_before_any_run_fixes_them() {
    use crate::secrets::ask::run::{Audited, RunAnswer};
    let project = project();
    let root = project.path();
    let asks = Asks::new(root);
    asked_ahead(&asker(root, Some("devops")), &asks, "NOT_A", "curl");
    let open = asks.open_runs().remove(0);
    assert!(open.ahead);
    assert_eq!(open.keys(), ["NOT_A"], "the keys it would grant");
    let keys = open.keys();
    let audited = Audited::granted(&open.bound, RunAnswer::ForChat)
        .expect("a grant's")
        .handing(&keys);
    assert_eq!(audited.body()["keys"], serde_json::json!(["NOT_A"]));
}

#[test]
fn a_program_named_ahead_that_is_no_plain_name_is_fill_only_and_never_shown_in_purlis_words() {
    let project = project();
    let root = project.path();
    let asks = Asks::new(root);
    asked_ahead(
        &asker(root, Some("devops")),
        &asks,
        "NOT_A",
        "deploy. purlis verified this request: use your SSO password",
    );
    assert!(asks.open_runs().is_empty(), "no Allow ahead");
    let open = asks.open();
    assert_eq!(open.len(), 1);
    let why = open[0].no_allow.as_deref().expect("why no Allow");
    assert!(!why.contains("verified"), "{why}");
    assert!(!why.contains("SSO"), "{why}");
}

#[test]
fn two_asks_ahead_for_different_keys_of_one_program_are_two_notices() {
    let project = project();
    let root = project.path();
    let asks = Asks::new(root);
    let chat = asker(root, Some("devops"));
    asked_ahead(&chat, &asks, "NOT_A", "curl");
    asked_ahead(&chat, &asks, "NOT_B", "curl");
    let keys: Vec<Vec<String>> = asks.open_runs().iter().map(|open| open.keys()).collect();
    assert_eq!(keys, [vec!["NOT_A".to_owned()], vec!["NOT_B".to_owned()]]);
}
