// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The scripted person, the window's queries and the global checks** (T1.1-07, #6854): the
//! tests of [`super::person`], [`super::queries`] and [`super::checks`].
//!
//! The planted-fault tests are pure: they run on any machine, with events written by hand. The
//! ones that start a scenario need the OS wrap, so their red and green are CI's (a scenario that
//! cannot apply its wrap panics there, and elsewhere says why it did not run).

use std::time::Duration;

use purlis_core::eventlog::Event;

use super::checks::{self, Canary, Check, Mark, Observed, Refusal, StepResult};
use super::host::{Dialect, Homes, Protection, Scenario};
use super::person::{Action, Press, chosen};
use crate::asking::{AnswerPath, AskSource, Offered, Shown};
use crate::sandboxing::{GrantLevel, GrantWhat};
use crate::taskblocks::BlockShown;

/// An event of chat `chat`, as the log reads one.
fn event(seq: u64, chat: Option<&str>, kind: &str, body: serde_json::Value) -> Event {
    Event {
        v: 1,
        device_id: "device".to_owned(),
        seq,
        ulid: "01J00000000000000000000000".to_owned(),
        chat: chat.map(str::to_owned),
        run: None,
        parent_run: None,
        kind: kind.to_owned(),
        body,
    }
}

fn a_block(seq: u64, chat: &str, host: &str) -> Event {
    event(
        seq,
        Some(chat),
        "stop.block",
        serde_json::json!({"stop": "block", "tool": "Bash", "source": "sandbox",
                           "target": {"host": host}}),
    )
}

fn raised(seq: u64, chat: &str) -> Event {
    event(
        seq,
        Some(chat),
        "ask.raised",
        serde_json::json!({"stop": "ask_raised", "ask": "sandbox"}),
    )
}

/// What a clean scenario observed: nothing to fire any check.
struct Clean {
    events: Vec<Event>,
    refusals: Vec<Refusal>,
    asks_left: Vec<String>,
    asked_for: Vec<(String, String, u64)>,
    grants_listed: Vec<(String, String, String)>,
    grants_made: Vec<(String, String, String)>,
    budget: Duration,
    /// Whether the chats ran a real harness binary ([`Observed::real_harness`]).
    real_harness: bool,
}

impl Clean {
    fn new() -> Clean {
        Clean {
            events: Vec::new(),
            refusals: Vec::new(),
            asks_left: Vec::new(),
            asked_for: Vec::new(),
            grants_listed: Vec::new(),
            grants_made: Vec::new(),
            budget: Duration::from_secs(40),
            real_harness: false,
        }
    }

    fn judged(&self) -> Vec<checks::Violation> {
        checks::judge(&Observed {
            events: &self.events,
            refusals: &self.refusals,
            asks_left: &self.asks_left,
            asked_for: &self.asked_for,
            grants_listed: &self.grants_listed,
            grants_made: &self.grants_made,
            hook_budget: self.budget,
            real_harness: self.real_harness,
        })
    }

    fn fired(&self) -> Vec<Check> {
        let mut fired: Vec<Check> = self.judged().into_iter().map(|one| one.check).collect();
        fired.sort();
        fired.dedup();
        fired
    }
}

/// **The planted fault of `check`**: what a scenario observes when exactly that check should fire.
fn planted(check: Check) -> Clean {
    let mut observed = Clean::new();
    match check {
        // A refusal of a host, and nothing written for the chat: #6675's urllib.
        Check::NoBlackHole => observed.refusals.push(Refusal {
            chat: "c1".to_owned(),
            step: "urllib".to_owned(),
            target: Some("urllib.example.test".to_owned()),
            after: 0,
        }),
        Check::NoUnansweredAsk => observed
            .asks_left
            .push("chat 1 `7` (\"asks for a host\")".to_owned()),
        // Settings lists a grant nobody made.
        Check::NoLeftoverGrant => observed.grants_listed.push((
            "Host".to_owned(),
            "stray.example.test".to_owned(),
            "you".to_owned(),
        )),
        Check::HookWithinBudget => observed.events.push(event(
            1,
            Some("c1"),
            "hook.pretool",
            serde_json::json!({"decision": "allow", "hook_ms": 600_000}),
        )),
        // A block that says nobody refused it.
        Check::EveryBlockHasASource => observed.events.push(event(
            1,
            Some("c1"),
            "stop.block",
            serde_json::json!({"stop": "block", "tool": "Bash"}),
        )),
        // #6674: two `ask.raised` for one ask, written together, with no new call between.
        Check::OneAskOneRaise => {
            observed.events.push(raised(1, "c1"));
            observed.events.push(raised(2, "c1"));
        }
    }
    observed
}

/// **Each global check fires on its planted fault** (T1.1-07), and on nothing else: one fault
/// per check, in a `match` with no wildcard, so a check added without a fault does not compile.
#[test]
fn each_global_check_fires_on_its_planted_fault() {
    assert!(
        Clean::new().judged().is_empty(),
        "a scenario that observed nothing wrong fires no check"
    );
    // Every check is tried before any is judged, so a red run names each one that stayed quiet.
    let quiet: Vec<String> = Check::ALL
        .into_iter()
        .filter_map(|check| {
            let observed = planted(check);
            (observed.fired() != vec![check]).then(|| {
                format!(
                    "{}: its planted fault must fire it, and only it, and fired {:?}",
                    check.word(),
                    observed.judged()
                )
            })
        })
        .collect();
    assert!(quiet.is_empty(), "{quiet:#?}");
    // And the same observations without the fault: each fault is the only difference.
    let mut healed = planted(Check::NoBlackHole);
    healed.events.push(a_block(1, "c1", "urllib.example.test"));
    assert!(healed.judged().is_empty(), "{:?}", healed.judged());

    let mut answered = planted(Check::OneAskOneRaise);
    answered.events.pop();
    assert!(answered.judged().is_empty(), "{:?}", answered.judged());

    let mut once = planted(Check::NoLeftoverGrant);
    once.grants_made.push((
        "Host".to_owned(),
        "stray.example.test".to_owned(),
        "once".to_owned(),
    ));
    assert_eq!(
        once.fired(),
        vec![Check::NoLeftoverGrant],
        "a once-grant listed in Settings outlived its use"
    );
    once.grants_made[0].2 = "you".to_owned();
    assert!(once.judged().is_empty(), "a kept grant is the person's");
}

/// **A refusal with nothing written is a black hole** (R-004, #6675): a planted urllib refusal
/// (a Python client that dials past what the proxy tells), or a hook's `deny`, with no ask, no
/// block and no hold written for its chat, is reported, naming the step and its target.
/// **A real harness is not failed for the asks its own start-up raises** (T1.1-53): two sandbox
/// asks in a row, written before any call, are two connections of a real harness's start-up whose
/// events name no target, and `one ask, one raise` cannot tell them from one ask raised twice
/// (#6674); the fake harness, which opens none, is still held to it, and every other check still
/// fires on a real harness.
#[test]
fn a_real_harness_is_not_failed_for_its_own_start_up_asks_but_the_fake_is() {
    let two_raises = planted(Check::OneAskOneRaise);
    assert_eq!(two_raises.fired(), vec![Check::OneAskOneRaise]);

    let mut real = planted(Check::OneAskOneRaise);
    real.real_harness = true;
    assert!(
        real.judged().is_empty(),
        "a real harness's start-up asks fail a check: {:?}",
        real.judged()
    );

    // Nothing else is relaxed: a refusal with nothing written is a black hole for it too.
    let mut hole = planted(Check::NoBlackHole);
    hole.real_harness = true;
    assert_eq!(hole.fired(), vec![Check::NoBlackHole]);
}

#[test]
fn a_refusal_with_nothing_written_is_reported_as_a_black_hole() {
    let observed = planted(Check::NoBlackHole);
    let found = observed.judged();
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].check, Check::NoBlackHole);
    assert!(
        found[0].what.contains("urllib") && found[0].what.contains("urllib.example.test"),
        "the report names the step and its target: {}",
        found[0].what
    );

    // A block for another target of the chat is no answer to this refusal; one for it is.
    let mut elsewhere = planted(Check::NoBlackHole);
    elsewhere
        .events
        .push(a_block(1, "c1", "another.example.test"));
    assert_eq!(elsewhere.fired(), vec![Check::NoBlackHole]);
    // Nor is another chat's.
    let mut another_chat = planted(Check::NoBlackHole);
    another_chat
        .events
        .push(a_block(1, "c2", "urllib.example.test"));
    assert_eq!(another_chat.fired(), vec![Check::NoBlackHole]);
    // A hold for the chat's target is one; so is an ask the window listed for that target.
    let mut asked = planted(Check::NoBlackHole);
    asked.events.push(raised(1, "c1"));
    asked
        .asked_for
        .push(("c1".to_owned(), "urllib.example.test".to_owned(), 1));
    assert!(asked.judged().is_empty(), "{:?}", asked.judged());

    // A hook's deny with nothing written is a black hole too, with no step declared.
    let mut denied = Clean::new();
    denied.events.push(event(
        1,
        Some("c1"),
        "hook.pretool",
        serde_json::json!({"decision": "deny", "hook_ms": 3}),
    ));
    assert_eq!(denied.fired(), vec![Check::NoBlackHole]);
    denied.events.push(event(
        2,
        Some("c1"),
        "stop.block",
        serde_json::json!({"stop": "block", "tool": "Bash", "source": "guard"}),
    ));
    assert!(denied.judged().is_empty(), "{:?}", denied.judged());
}

/// **An ask raised for one target is no answer to a refusal of another** (round 2, item 5): a
/// later `ask.raised` of the chat does not heal a refusal whatever its target, and one raise
/// answers one refusal, never two.
#[test]
fn an_ask_for_another_target_does_not_answer_a_refusal() {
    // The chat's ask was listed for `other`, not for the refused host.
    let mut elsewhere = planted(Check::NoBlackHole);
    elsewhere.events.push(raised(1, "c1"));
    elsewhere
        .asked_for
        .push(("c1".to_owned(), "other.example.test".to_owned(), 1));
    assert_eq!(elsewhere.fired(), vec![Check::NoBlackHole]);
    // And no ask listed at all for it.
    let mut unlisted = planted(Check::NoBlackHole);
    unlisted.events.push(raised(1, "c1"));
    assert_eq!(unlisted.fired(), vec![Check::NoBlackHole]);

    // One raise answers one of two refusals of the same target.
    let mut twice = planted(Check::NoBlackHole);
    twice.refusals.push(Refusal {
        chat: "c1".to_owned(),
        step: "urllib again".to_owned(),
        target: Some("urllib.example.test".to_owned()),
        after: 0,
    });
    twice.events.push(raised(1, "c1"));
    twice
        .asked_for
        .push(("c1".to_owned(), "urllib.example.test".to_owned(), 1));
    let found = twice.judged();
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].what.contains("urllib again"), "{found:?}");
}

/// **An ask listed before the refusal, or raised after the listing it would answer, heals
/// nothing** (round 3, item 3): the ask must be written after the refusal and listed, for the
/// target, at or after it was written.
#[test]
fn an_ask_listed_before_the_refusal_does_not_answer_it() {
    // The refusal is declared after event 3; the target was listed at 2, before.
    let mut earlier = planted(Check::NoBlackHole);
    earlier.refusals[0].after = 3;
    earlier.events.push(raised(4, "c1"));
    earlier
        .asked_for
        .push(("c1".to_owned(), "urllib.example.test".to_owned(), 2));
    assert_eq!(earlier.fired(), vec![Check::NoBlackHole]);
    // Seen at the raise's own number or later, it answers.
    let mut later = planted(Check::NoBlackHole);
    later.refusals[0].after = 3;
    later.events.push(raised(4, "c1"));
    later
        .asked_for
        .push(("c1".to_owned(), "urllib.example.test".to_owned(), 4));
    assert!(later.judged().is_empty(), "{:?}", later.judged());
}

/// **A bare 403 or "forbidden" is no proof the proxy refused** (round 3, item 2).
#[test]
fn a_traceback_that_says_forbidden_is_not_the_proxys_refusal() {
    let traceback = StepResult {
        ran: true,
        exit: Some(1),
        said: "traceback: file \"x.py\", line 3: http.client 403 forbidden".to_owned(),
        hook: None,
    };
    assert!(checks::urllib_refused_by_the_proxy(&traceback).is_err());
    let proxy_error = StepResult {
        ran: true,
        exit: Some(1),
        said: "requests.exceptions.proxyerror: cannot connect".to_owned(),
        hook: None,
    };
    assert!(checks::urllib_refused_by_the_proxy(&proxy_error).is_ok());
}

/// **A `hook_ms` that is not a whole number fails the budget check** (round 2, item 6): a string,
/// a float and a null measure nothing, and are not skipped.
#[test]
fn a_hook_ms_that_is_not_a_whole_number_fails_the_budget() {
    for (what, measured) in [
        ("a string", serde_json::json!("600000")),
        ("a float", serde_json::json!(1.5)),
        ("a null", serde_json::Value::Null),
        ("a negative", serde_json::json!(-3)),
    ] {
        let mut observed = Clean::new();
        observed.events.push(event(
            1,
            Some("c1"),
            "hook.pretool",
            serde_json::json!({"decision": "allow", "hook_ms": measured}),
        ));
        assert_eq!(
            observed.fired(),
            vec![Check::HookWithinBudget],
            "{what} was skipped"
        );
    }
    let mut fine = Clean::new();
    fine.events.push(event(
        1,
        Some("c1"),
        "hook.pretool",
        serde_json::json!({"decision": "allow", "hook_ms": 3}),
    ));
    assert!(fine.judged().is_empty());
}

/// **A log that was not read, or holds nothing once a chat started, is no quiet log** (round 2,
/// item 1): an unreadable log is an `Err`, an empty one with a chat started is one, and only a
/// scenario that started no chat may have an empty log.
#[test]
fn an_unreadable_or_empty_log_is_a_failure_and_never_a_pass() {
    let unreadable =
        checks::log_verdict(1, Err("permission denied".to_owned())).expect_err("an unreadable log");
    assert!(unreadable.contains("could not be read"), "{unreadable}");
    let empty = checks::log_verdict(2, Ok(Vec::new())).expect_err("an empty log after chats");
    assert!(empty.contains("no event"), "{empty}");
    assert!(checks::log_verdict(0, Ok(Vec::new())).is_ok());
    assert_eq!(
        checks::log_verdict(1, Ok(vec![raised(1, "c1")]))
            .expect("a written log")
            .len(),
        1
    );
    // A log in a folder that is a file cannot be read, and the scenario's read says so.
    let file = std::env::temp_dir().join(format!("log-is-a-file-{}", std::process::id()));
    std::fs::write(&file, "x").expect("a file");
    let read = purlis_core::eventlog::read(&file).map_err(|why| why.to_string());
    let _ = std::fs::remove_file(&file);
    if read.is_err() {
        assert!(checks::log_verdict(1, read).is_err());
    }
}

/// **No window at the end is a failure, not a skip** (round 2, item 2).
#[test]
fn a_scenario_with_no_window_at_its_end_is_not_checked_and_so_not_green() {
    assert!(checks::window_for_the_checks(true).is_ok());
    let why = checks::window_for_the_checks(false).expect_err("no window");
    assert!(
        why.contains("not been checked") || why.contains("not run"),
        "{why}"
    );
}

/// **The scripted person's `when_listed` matches exactly, per chat** (round 2, item 7).
#[test]
fn when_listed_matches_one_chats_ask_for_exactly_the_target() {
    assert!(super::person::is_the_ask_for(3, "a.test", 3, "a.test"));
    assert!(
        !super::person::is_the_ask_for(3, "a.test.evil", 3, "a.test"),
        "a longer name that contains the target is another ask"
    );
    assert!(
        !super::person::is_the_ask_for(4, "a.test", 3, "a.test"),
        "another chat's ask for the same target is not this chat's"
    );
    assert!(!super::person::is_the_ask_for(3, "", 3, "a.test"));
}

/// **A mark names its issue, and a mark whose check passes is itself a failure**: a check is
/// `expected_fail`ed per scenario, never switched off, and closing the issue removes the mark.
#[test]
fn a_global_check_is_only_ever_expected_to_fail_with_its_issue_and_never_for_the_suite() {
    let found = planted(Check::OneAskOneRaise).judged();
    let mark = Mark {
        check: Check::OneAskOneRaise,
        issue: 6674,
        why: "two writers".to_owned(),
    };
    let marked = checks::outcome(found.clone(), std::slice::from_ref(&mark));
    assert!(marked.failures.is_empty(), "{marked:?}");
    assert_eq!(marked.expected.len(), 1);
    assert_eq!(marked.expected[0].1, 6674);

    // Marked for one check, a failure of another is still a failure.
    let other = checks::outcome(
        planted(Check::NoBlackHole).judged(),
        std::slice::from_ref(&mark),
    );
    assert_eq!(other.failures.len(), 2, "{other:?}");
    assert!(
        other
            .failures
            .iter()
            .any(|one| one.contains("unexpected pass")),
        "the mark's check passed: {other:?}"
    );
    // Unmarked, the same failure fails.
    let unmarked = checks::outcome(found, &[]);
    assert_eq!(unmarked.failures.len(), 1, "{unmarked:?}");
    // Every check is judged by every scenario: none is a field a scenario can leave out.
    assert_eq!(Check::ALL.len(), 6);
}

/// A sandbox ask of chat `session`, as the window lists it: Allow at three levels and Keep
/// blocked, labelled as the window labels them.
fn a_sandbox_ask(session: u32, labels: &[(&str, &str)]) -> Shown {
    Shown {
        session,
        ask: "7".to_owned(),
        says: "wants to reach granted.example.test".to_owned(),
        options: labels
            .iter()
            .map(|(id, label)| Offered {
                id: (*id).to_owned(),
                label: (*label).to_owned(),
                allows: *id != crate::asking::KEEP,
                preselected: None,
                scope: None,
            })
            .collect(),
        source: AskSource::SandboxHost,
        chain: vec![format!("chat {session}")],
        answer: AnswerPath::SandboxBlock {
            shown: BlockShown {
                operation: "connect".to_owned(),
                kind: "host".to_owned(),
                what: GrantWhat::Host,
                target: "granted.example.test".to_owned(),
            },
        },
        since: None,
        held_until: None,
        expires: None,
        blocks: None,
        timed_out: None,
        risk: None,
        writes: None,
        lines: None,
        last_message: None,
        login: None,
        fill: None,
        earlier_messages: None,
        rule: None,
        secret: None,
        run: None,
    }
}

/// What a window offers a host ask, by id and label.
const OFFERED: [(&str, &str); 4] = [
    ("chat", "Allow only for this chat"),
    ("you", "Allow for me on this machine"),
    ("project", "Allow for everyone in this project"),
    ("keep", "Keep blocked"),
];

/// **A press on a label the ask does not offer fails the scenario** (T1.1-07): the person
/// presses only what a window draws. An ask with no such option, or one that says another label
/// than the window's `allow_said` and `KEEP_SAID`, is no press.
#[test]
fn a_press_on_a_label_the_ask_does_not_offer_fails_the_scenario() {
    let ask = a_sandbox_ask(1, &OFFERED);
    // What is offered is pressed, and the label is what the window says.
    for press in [
        Press::Allow(GrantLevel::Chat),
        Press::Allow(GrantLevel::You),
        Press::Allow(GrantLevel::Project),
        Press::Keep(None),
    ] {
        assert!(chosen(&ask, &press).is_ok(), "{press:?}");
    }
    // Not offered: Once and Machine are no Allow of this ask.
    for level in [GrantLevel::Once, GrantLevel::Machine] {
        let refused = chosen(&ask, &Press::Allow(level)).expect_err("not offered");
        assert!(refused.contains("offers no"), "{refused}");
    }
    assert!(chosen(&ask, &Press::Option("never".to_owned())).is_err());
    // Offered, but said in other words than the window's.
    let mut said_otherwise = OFFERED;
    said_otherwise[0].1 = "Allow for this chat";
    let wrong = chosen(
        &a_sandbox_ask(1, &said_otherwise),
        &Press::Allow(GrantLevel::Chat),
    )
    .expect_err("a label the window does not say");
    assert!(
        wrong.contains("Allow for this chat") && wrong.contains("Allow only for this chat"),
        "{wrong}"
    );
    let mut keeps = OFFERED;
    keeps[3].1 = "Keep it blocked";
    assert!(chosen(&a_sandbox_ask(1, &keeps), &Press::Keep(None)).is_err());

    // In a scenario, the person's press on such an ask fails the scenario.
    let Some(homes) = Homes::of_this_scenario() else {
        return;
    };
    let Some(scenario) = Scenario::new(&homes, Protection::Full, None, Dialect::Codex) else {
        return;
    };
    let pressed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        scenario.person().presses_on(
            &a_sandbox_ask(1, &OFFERED),
            &Press::Allow(GrantLevel::Machine),
        )
    }));
    assert!(
        pressed.is_err(),
        "the press on an unoffered label did not fail the scenario"
    );
    assert!(
        scenario.person().reached_commands().is_empty(),
        "no command was called for a press the ask does not offer"
    );
}

/// **Every action names a command the window registers** (T1.1-07): each of the person's
/// actions that is a window command is in `ipc_commands.rs`'s list, the one the handler, the
/// generated TypeScript and the capabilities are made from.
#[test]
fn every_person_action_names_a_command_the_window_registers() {
    let (value_free, vault_values) = app_commands!(command_names);
    let registered: Vec<&str> = value_free
        .iter()
        .chain(vault_values.iter())
        .copied()
        .collect();
    for action in Action::ALL {
        if action.is_a_window_command() {
            assert!(
                registered.contains(&action.command()),
                "{action:?} calls `{}`, which the window does not register",
                action.command()
            );
        }
    }
    assert!(!Action::KillHarness.is_a_window_command());
    // Every distinct command the matrix presses is named.
    let mut named: Vec<&str> = Action::ALL
        .iter()
        .filter(|action| action.is_a_window_command())
        .map(|action| action.command())
        .collect();
    named.sort_unstable();
    named.dedup();
    assert!(named.len() >= 20, "{named:?}");
}

/// **Each person action and its window command reach the same function** (T1.1-07): every
/// action is made on a live scenario's window, through the window's own `Planes`, and the
/// person's trace names the command each one called, the very name the IPC knows it by
/// (`through!` takes the name and the function from one path, so they cannot differ). A command
/// whose target is not there answers with its own refusal, which is its body running: nothing
/// is stopped, ended or restarted.
#[test]
fn each_person_action_and_its_window_command_reach_the_same_function() {
    let Some(homes) = Homes::of_this_scenario() else {
        return;
    };
    let Some(scenario) = Scenario::new(&homes, Protection::Full, None, Dialect::Codex) else {
        return;
    };
    let person = scenario.person();
    let nobody = 999;
    let shown = BlockShown {
        operation: "connect".to_owned(),
        kind: "host".to_owned(),
        what: GrantWhat::Host,
        target: "nobody.example.test".to_owned(),
    };
    let seen = vec![crate::taskblocks::SeenBlock {
        task: nobody,
        operation: "connect".to_owned(),
        kind: "host".to_owned(),
        what: GrantWhat::Host,
        target: "nobody.example.test".to_owned(),
    }];
    // The answers of a chat that is not there: each refused by the command's own body.
    assert!(
        person
            .allow_block(nobody, shown.clone(), GrantLevel::Chat)
            .is_err(),
        "Allow on a block nobody is held on is refused whole (`shown_one`)"
    );
    // A command that answers "answered already" with Ok is no less its body.
    let _ = person.keep_block(nobody, shown.clone(), None);
    let _ = person.forget_block(nobody, shown);
    assert!(
        person
            .allow_for_tasks(nobody, seen.clone(), GrantLevel::Chat)
            .is_err()
    );
    let _ = person.keep_for_tasks(nobody, seen, None);
    let _ = person.answer_ask(nobody, "0", "allow");
    assert!(
        person
            .answer_pause(nobody, "continue")
            .expect_err("a chat that is not paused")
            .contains("no longer paused")
    );
    let _ = person.trust_clone("nowhere", "https://example.test/x.git");
    let _ = person.show_this_rule();
    let _ = person.revokes("no-such-grant");
    let (repo, number, head) = ("example/none", 1, "0000000");
    let _ = person.merge_again(nobody, repo, number, head);
    let _ = person.merge(
        nobody,
        repo,
        number,
        head,
        crate::asking::RulesetShown::Unknown,
        "digest",
    );
    let _ = person.dont_merge(nobody, repo, number, head);
    let _ = person.view_changes(nobody, repo, number, head);
    let _ = person.stop_chat(nobody, false);
    let _ = person.ends(nobody);
    let _ = person.restarts(nobody, false);
    let _ = person.restarts_without_the_sandbox(nobody);
    person.quiet(true);
    person.quiet(false);
    let _ = person.ignores(nobody);
    let _ = person.snoozes(Vec::new(), None);
    let _ = person.dismisses(nobody, "no-such-failure");
    let _ = person.rejects_proposal(999_999);
    let _ = person.types_in_the_pane(nobody, "hello");
    let _ = person.esc(nobody);
    assert!(
        person.kills_the_harness(nobody).is_err(),
        "no process to kill"
    );

    let mut reached = person.reached_commands();
    reached.sort_unstable();
    reached.dedup();
    let mut wanted: Vec<&str> = Action::ALL
        .iter()
        .filter(|action| action.is_a_window_command())
        .map(|action| action.command())
        .collect();
    wanted.sort_unstable();
    wanted.dedup();
    assert_eq!(
        reached, wanted,
        "the commands the person called, and the ones its actions name"
    );
}

/// **A failing scenario's report** (T1.1-07): the step, what was expected, what was seen, the
/// events the log holds, what the window's queries show and the transcript's excerpt.
#[test]
fn a_failing_scenario_prints_the_step_the_expected_and_seen_events_the_queries_and_the_transcript()
{
    let events = vec![a_block(1, "c1", "seen.example.test"), raised(2, "c1")];
    let transcript = vec![
        serde_json::json!({"kind": "tool_call", "step": "reach"}),
        serde_json::json!({"kind": "tool_result", "step": "reach", "stdout": "status 000"}),
    ];
    let report = checks::failure_report(
        "reach the host",
        "stop.block for expected.example.test",
        "stop.block for seen.example.test",
        &events,
        "asks counted: [\"chat 1 `7`\"]",
        &transcript,
    );
    for wanted in [
        "step: reach the host",
        "expected: stop.block for expected.example.test",
        "seen: stop.block for seen.example.test",
        "stop.block",
        "ask.raised",
        "the window's queries:",
        "asks counted",
        "transcript",
        "status 000",
    ] {
        assert!(report.contains(wanted), "{wanted:?} is not in\n{report}");
    }
}

/// **The canary's judgment**: a wrap in force refuses both, and one that lets either through is
/// not in force.
#[test]
fn a_scenario_whose_wrap_is_not_in_force_fails_its_canary() {
    let refused_write = StepResult {
        ran: true,
        exit: Some(1),
        said: "sh: /x/y: operation not permitted".to_owned(),
        hook: None,
    };
    let refused_connect = StepResult {
        ran: true,
        exit: Some(7),
        said: "curl: (7) failed to connect".to_owned(),
        hook: None,
    };
    let seen = |wrote, dialled, write: &StepResult, connect: &StepResult| Canary {
        wrote,
        dialled,
        write: write.clone(),
        connect: connect.clone(),
    };
    assert!(checks::judge_canary(&seen(false, 0, &refused_write, &refused_connect)).is_ok());
    let wrote =
        checks::judge_canary(&seen(true, 0, &refused_write, &refused_connect)).expect_err("wrote");
    assert!(wrote.contains("write outside the project"), "{wrote}");
    let dialled = checks::judge_canary(&seen(false, 2, &refused_write, &refused_connect))
        .expect_err("dialled");
    assert!(dialled.contains("direct dials (2)"), "{dialled}");

    // No file and no dial is not enough (round 2, item 3): each step's own result must say it was
    // refused. A curl that is not installed, a call a hook denied and a step that never ran each
    // leave no file and no dial.
    let no_curl = StepResult {
        ran: true,
        exit: Some(127),
        said: "sh: curl: command not found".to_owned(),
        hook: None,
    };
    let denied = StepResult {
        ran: false,
        exit: None,
        said: "denied by a hook".to_owned(),
        hook: Some("pretooluse".to_owned()),
    };
    let never = StepResult::default();
    for (what, write, connect) in [
        ("curl missing", &refused_write, &no_curl),
        ("the connect denied by a hook", &refused_write, &denied),
        ("the connect never ran", &refused_write, &never),
        ("the write never ran", &never, &refused_connect),
        ("the write denied by a hook", &denied, &refused_connect),
    ] {
        assert!(
            checks::judge_canary(&seen(false, 0, write, connect)).is_err(),
            "{what} passed the canary"
        );
    }
    // A write that "succeeded" quietly (exit 0, nothing said) was not refused either.
    let silent = StepResult {
        ran: true,
        exit: Some(0),
        said: String::new(),
        hook: None,
    };
    assert!(checks::judge_canary(&seen(false, 0, &silent, &refused_connect)).is_err());

    // A transcript line is read as it is written.
    let line = serde_json::json!({"kind": "tool_result", "step": "canary-connect",
        "exit": 7, "stdout": "", "stderr": "curl: (7) Failed", "text": ""});
    let read = StepResult::of(Some(&line));
    assert!(read.ran && read.exit == Some(7), "{read:?}");
    assert!(!StepResult::of(None).ran);
    let hooked = serde_json::json!({"kind": "tool_result", "hook": "pretooluse",
        "text": "blocked", "is_error": true});
    assert!(!StepResult::of(Some(&hooked)).ran);

    // The canary's own commands, run with no wrap at all (this process is not wrapped): both get
    // through, so its judgment is red. This is what a scenario whose wrap did not apply sees.
    let outside = std::env::temp_dir().join(format!("canary-unwrapped-{}", std::process::id()));
    std::fs::create_dir_all(&outside).expect("a folder");
    // Where this machine lets no test listen (a sandboxed task), the unwrapped run is CI's.
    let listener = match std::net::TcpListener::bind("127.0.0.1:0") {
        Ok(listener) => listener,
        Err(why) if !super::host::on_ci() => {
            let _ = std::io::Write::write_fmt(
                &mut std::io::stderr(),
                format_args!("the unwrapped canary ran nowhere: no listener here ({why})\n"),
            );
            return;
        }
        Err(why) => panic!("the canary's listener: {why}"),
    };
    let port = listener.local_addr().expect("address").port();
    let file = outside.join("written");
    let script = checks::canary_script(&file, port);
    let accepting = std::thread::spawn(move || {
        let mut hits = 0;
        listener
            .set_nonblocking(true)
            .expect("polling for the dial");
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while hits == 0 && std::time::Instant::now() < deadline {
            if listener.accept().is_ok() {
                hits += 1;
            } else {
                std::thread::sleep(Duration::from_millis(20));
            }
        }
        hits
    });
    for call in script["turns"][0]["calls"].as_array().expect("calls") {
        let run = call["run"].as_str().expect("a command");
        let _ = purlis_core::forklock::status(std::process::Command::new("sh").args(["-c", run]));
    }
    let seen = Canary {
        wrote: file.exists(),
        dialled: accepting.join().expect("the listener's thread"),
        // An unwrapped run is read from the exit of its own commands: both ran and succeeded.
        write: StepResult {
            ran: true,
            exit: Some(0),
            said: String::new(),
            hook: None,
        },
        connect: StepResult {
            ran: true,
            exit: Some(0),
            said: String::new(),
            hook: None,
        },
    };
    let _ = std::fs::remove_dir_all(&outside);
    assert!(
        seen.wrote && seen.dialled > 0,
        "an unwrapped run lets both through: {seen:?}"
    );
    assert!(
        checks::judge_canary(&seen).is_err(),
        "a scenario whose wrap is not in force must fail its canary"
    );

    // And in a scenario whose wrap is in force, the canary passes: every scenario runs it as it
    // is made (`Scenario::under`), one refused write and one refused connect, the person keeping
    // what it raised blocked.
    let Some(homes) = Homes::of_this_scenario() else {
        return;
    };
    for dialect in Dialect::ALL {
        if dialect == Dialect::Claude && cfg!(target_os = "macos") {
            continue;
        }
        let Some(scenario) = Scenario::new(&homes, Protection::Full, None, dialect) else {
            return;
        };
        assert!(
            scenario.chats_started() > 0
                || (dialect == Dialect::Claude && cfg!(target_os = "macos")),
            "a scenario made under the wrap ran no canary"
        );
    }
}

/// **The #6675 mark holds only when the urllib step was refused by the proxy** (round 2, item 11).
#[test]
fn the_urllib_step_must_be_seen_refused_by_the_proxy() {
    let refused = StepResult {
        ran: true,
        exit: Some(1),
        said: "urllib.error.urlerror: <urlopen error tunnel connection failed: 403 forbidden>"
            .to_owned(),
        hook: None,
    };
    assert!(checks::urllib_refused_by_the_proxy(&refused).is_ok());
    let no_python = StepResult {
        ran: true,
        exit: Some(127),
        said: "sh: python3: command not found".to_owned(),
        hook: None,
    };
    let silent_success = StepResult {
        ran: true,
        exit: Some(0),
        said: String::new(),
        hook: None,
    };
    for (what, result) in [
        ("python3 missing", &no_python),
        ("the step never ran", &StepResult::default()),
        ("the request went through", &silent_success),
    ] {
        assert!(
            checks::urllib_refused_by_the_proxy(result).is_err(),
            "{what} was taken for a refusal"
        );
    }
}

/// A host of 134 characters: two labels of 60 and `example.test`, longer than the 120 an event
/// keeps of a target, and as long as the window may show whole.
fn a_long_host() -> String {
    format!("{}.{}.example.test", "a".repeat(60), "b".repeat(60))
}

/// **The ask shows exactly the decided target, and grants that string** (T1.1-07, item 5): the
/// window query's target is checked against the decided one (escaped, punycode shown, never cut
/// short to another host), and what is granted is the exact string shown.
#[test]
fn the_ask_shows_exactly_the_decided_target_and_grants_that_string() {
    let host = |target: &str| BlockShown {
        operation: "connect".to_owned(),
        kind: "host".to_owned(),
        what: GrantWhat::Host,
        target: target.to_owned(),
    };
    // What is shown, and decided, whole.
    for good in ["granted.example.test", "xn--bcher-kva.example.test"] {
        assert!(
            checks::shows_the_decided_target(&host(good), good).is_ok(),
            "{good}"
        );
    }
    let long = a_long_host();
    assert!(checks::shows_the_decided_target(&host(&long), &long).is_ok());
    // The planted faults: each is refused.
    let faults = [
        (
            "a bidi override shown raw",
            "evil\u{202e}moc.example.test",
            "evil\u{202e}moc.example.test",
        ),
        (
            "an ANSI escape shown raw",
            "red\u{1b}[31m.example.test",
            "red\u{1b}[31m.example.test",
        ),
        (
            "a control character shown raw",
            "tab\there.example.test",
            "tab\there.example.test",
        ),
        (
            "a host shown in Unicode, not punycode",
            "bücher.example.test",
            "bücher.example.test",
        ),
        ("a target cut short", "aaaaaaaa…", "aaaaaaaa.example.test"),
        (
            "another host than the decided one",
            "shown.example.test",
            "decided.example.test",
        ),
    ];
    for (what, shown, decided) in faults {
        assert!(
            checks::shows_the_decided_target(&host(shown), decided).is_err(),
            "{what} was not refused"
        );
    }

    let Some(homes) = Homes::of_this_scenario() else {
        return;
    };
    let Some(scenario) = Scenario::new(&homes, Protection::Full, Some("steward"), Dialect::Codex)
    else {
        return;
    };
    for (n, name) in [long, "xn--bcher-kva.example.test".to_owned()]
        .iter()
        .enumerate()
    {
        let step = format!("reaches-{n}");
        let chat = scenario.starts(&serde_json::json!({
            "turns": [{"prompt": "go", "calls": [
                {"step": step, "tool": "bash", "run": super::reaches(name)}
            ]}]
        }));
        // The person presses Allow for me on this machine, once the ask for it is listed.
        let (ask, pressed) = scenario.person().when_listed(
            chat,
            name,
            &Press::Allow(GrantLevel::You),
            Duration::from_millis(50),
        );
        pressed.unwrap_or_else(|why| panic!("{why}\n{}", scenario.said()));
        let shown = match &ask.answer {
            AnswerPath::SandboxBlock { shown } => shown.clone(),
            other => panic!("the ask for a host is answered by {other:?}"),
        };
        // What the window showed is what the proxy decides on: the name it looks up.
        super::until(&scenario, "the allowed host is looked up", || {
            scenario.names.lookups().iter().any(|one| &one.name == name)
        });
        checks::shows_the_decided_target(&shown, name)
            .unwrap_or_else(|why| scenario.fails_at("the ask's target", name, &why));
        scenario.until_played(chat, &[&step]);
        // And the grant written is that string, as Settings lists it.
        let listed = scenario.queries().settings();
        assert!(
            listed
                .iter()
                .any(|grant| grant.what == GrantWhat::Host && grant.target == shown.target),
            "Settings lists no grant for exactly {:?}: {:?}",
            shown.target,
            listed
                .iter()
                .map(|one| one.target.clone())
                .collect::<Vec<_>>()
        );
    }
}

/// **The window's queries read what the window reads** (T1.1-07): a held host is listed by the
/// ✋, its Notice is raised and, once the person keeps it blocked, retracted; the Chat log holds
/// the person's answer; the needs-you mark, Quiet, Settings, the Blocked log, the finishes and
/// the merge row are each read through the window's own function.
#[test]
fn the_window_queries_read_what_the_window_reads() {
    let Some(homes) = Homes::of_this_scenario() else {
        return;
    };
    let Some(scenario) = Scenario::new(&homes, Protection::Full, Some("steward"), Dialect::Codex)
    else {
        return;
    };
    let host = "queried.example.test";
    let chat = scenario.starts(&serde_json::json!({
        "turns": [{"prompt": "go", "calls": [
            {"step": "reaches", "tool": "bash", "run": super::reaches(host)}
        ]}]
    }));
    // "When an ask for the host is listed, press Keep blocked after 50 ms."
    let (ask, pressed) =
        scenario
            .person()
            .when_listed(chat, host, &Press::Keep(None), Duration::from_millis(50));
    pressed.unwrap_or_else(|why| panic!("{why}\n{}", scenario.said()));
    scenario.until_played(chat, &["reaches"]);

    let queries = scenario.queries();
    assert!(
        queries
            .counted()
            .asks
            .iter()
            .all(|one| one.ask != ask.ask || one.session != ask.session),
        "the ✋ still counts the ask the person kept blocked"
    );
    let notices = queries.notices();
    assert!(
        notices.was_raised(chat, host),
        "no Notice was raised for the host: told {:?}, held on {:?}",
        notices
            .told
            .iter()
            .map(|one| &one.target)
            .collect::<Vec<_>>(),
        notices.raised
    );
    assert_eq!(
        notices.retracted(host),
        vec![chat],
        "the Notice was raised and is retracted: {notices:?}"
    );
    let rows = queries.rows(chat);
    assert!(
        !rows.is_empty(),
        "the Chat log holds no row of the chat: {rows:?}"
    );
    // Whether the person's own answer is among them is today's behaviour, said and not pinned
    // here: the window writes it through its own recorder (R-229).
    let _ = std::io::Write::write_fmt(
        &mut std::io::stderr(),
        format_args!(
            "the Chat log holds the person's answer: {}\n",
            rows.iter()
                .any(|row| matches!(row, purlis_core::eventlog::chatlog::Row::AskAnswered { .. }))
        ),
    );
    let _ = queries.summary(chat);
    let _ = queries.blocks(chat, None);
    let _ = queries.finishes(chat);
    let _ = queries.blocked_log();
    assert!(queries.merge_row().is_empty());
    assert!(
        !queries.needs_you().moves.is_empty(),
        "the window was told no move"
    );

    // Quiet: on, then off, and what its bucket held is told as it ends.
    scenario.person().quiet(true);
    assert!(queries.quiet().on);
    scenario.person().quiet(false);
    let quiet = queries.quiet();
    assert!(!quiet.on);
    assert!(quiet.ended.is_some(), "Quiet ended and told what it held");
    // The text of a red run carries them all.
    let said = queries.said();
    for wanted in [
        "asks counted",
        "Notices raised",
        "needs you",
        "quiet",
        "settings",
        "merge row",
    ] {
        assert!(said.contains(wanted), "{wanted:?} is not in\n{said}");
    }
}

/// **A held refusal has its ask written by the end** (R-004), in a scenario: a held connection
/// nobody answers is refused at its hold, and by the time the scenario is quiet the log holds an
/// ask or a block for that chat, so the no-black-hole check passes on a real chat, with its
/// refusal declared. (Its block is written only at the chat's next prompt, #6673; the ask is
/// written by then.) The person then keeps it blocked, as a scenario that raised one must.
#[test]
fn a_held_refusal_has_its_ask_written_by_the_end_of_the_scenario() {
    let Some(homes) = Homes::of_this_scenario() else {
        return;
    };
    let Some(scenario) = Scenario::new(&homes, Protection::Full, Some("steward"), Dialect::Codex)
    else {
        return;
    };
    let host = "refused-held.example.test";
    let chat = scenario.starts(&serde_json::json!({
        "turns": [{"prompt": "go", "calls": [
            {"step": "reaches", "tool": "bash", "run": super::reaches(host)}
        ]}]
    }));
    scenario.until_played(chat, &["reaches"]);
    scenario.refused(chat, "reaches", Some(host));
    scenario.keeps_listed_asks_blocked();
}

/// **A Python client's refusal leaves nothing in the log** (R-004, #6675), in a scenario: a host
/// the person kept blocked is refused at once to `urllib` through the proxy, and nothing is
/// written for the second call: no ask, no block, no hold. The scenario declares the refusal
/// before the call, so the ask the first call (curl) raised answers nothing, and the no-black-hole
/// check fires, which the scenario pins to the issue. Linux alone: where it was seen on macOS,
/// the wrap writes its refusals another way.
#[test]
fn a_python_urllib_refusal_in_a_scenario_is_a_black_hole_until_6675_is_fixed() {
    let Some(homes) = Homes::of_this_scenario() else {
        return;
    };
    if !cfg!(target_os = "linux") {
        super::report::write(&[super::a_fixture_record(
            "a_python_urllib_refusal_in_a_scenario_is_a_black_hole_until_6675_is_fixed",
            Some(purlis_scenarios::model::Dialect::Codex),
            Some(purlis_scenarios::model::Protection::Full),
            purlis_scenarios::model::ReportVerdict::NotRun,
            "pinned on Linux: macOS's wrap writes its refusals another way (ticket 51)",
        )])
        .expect("the leg's report");
        homes.ran_no_scenario();
        return;
    }
    let Some(scenario) = Scenario::new(&homes, Protection::Full, Some("steward"), Dialect::Codex)
    else {
        return;
    };
    let host = "urllib-kept.example.test";
    let chat = scenario.starts(&serde_json::json!({
        "turns": [
            {"prompt": "first", "calls": [
                {"step": "curl", "tool": "bash", "run": super::reaches(host)}
            ]},
            {"prompt": "second", "calls": [
                {"step": "urllib", "tool": "bash", "run": format!(
                    "python3 -c \"import urllib.request; urllib.request.urlopen('https://{host}/', timeout=20)\""
                )}
            ]}
        ]
    }));
    // The person keeps the host blocked, as in the issue; curl's refusal is the ask's.
    let (_, kept) =
        scenario
            .person()
            .when_listed(chat, host, &Press::Keep(None), Duration::from_millis(50));
    kept.unwrap_or_else(|why| panic!("{why}\n{}", scenario.said()));
    scenario.until_played(chat, &["curl"]);
    // The next call is the one the issue is about.
    scenario.refused_after_now(chat, "urllib", Some(host));
    scenario.types(chat, "second");
    scenario.until_played(chat, &["urllib"]);
    // The mark is per check, not per violation: it holds for any black hole of this scenario. So
    // the step itself must be seen refused by the proxy, or the mark would stay green where python3
    // is missing and the refusal never happened.
    if let Err(why) = checks::urllib_refused_by_the_proxy(&checks::StepResult::of(
        scenario.result_of(chat, "urllib").as_ref(),
    )) {
        scenario.fails_at("urllib", "a request refused by the proxy", &why);
    }
    scenario.expected_fail(
        Check::NoBlackHole,
        6675,
        "a refusal seen by Python urllib writes nothing: no ask, no block",
    );
    scenario.keeps_listed_asks_blocked();
}
