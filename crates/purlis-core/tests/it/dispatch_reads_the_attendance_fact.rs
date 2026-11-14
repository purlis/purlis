// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Dispatch reads the attendance fact** (T1-19, #6410; T1-22, #6413; R-110, R-113): whether a
//! chat that asks to dispatch is attended is the [`purlis_core::attendance::Fact`] the app holds
//! for it ([`attendance::of_held`]): set by how purlis started it and lowered, never raised, by
//! the reports its own harness's armed hook made. A chat with no fact held is unattended.
//!
//! **Each pin names the mutation that shows it red** (Q208), in its doc comment.

use purlis_core::attendance::{self, Begun, Fact};
use purlis_core::dispatchgrant::{self, ChatPair, Covers, InForce};
use purlis_core::dispatchunattended::{self, Answer, Attendance, Refusal};
use purlis_core::harness::Harness;
use purlis_core::hookwire::Report;
use purlis_core::sandbox::policy::Locks;

/// The words of a chat's start.
fn words(command: &[&str]) -> Vec<String> {
    command.iter().map(|word| (*word).to_owned()).collect()
}

/// The fact the app holds for a Claude Code chat a person started from the window.
fn a_persons_chat() -> Fact {
    Fact::at_start(
        Begun::Person,
        Some(Harness::ClaudeCode),
        &words(&["claude"]),
    )
}

/// The fact it holds for the same chat, had another chat dispatched it as a task.
fn a_task() -> Fact {
    Fact::at_start(Begun::Task, Some(Harness::ClaudeCode), &words(&["claude"]))
}

/// One report of a chat on the harness the registry names `registry`, whose payload names
/// `mode`, as the hook channel reads it for the app.
fn report(registry: &str, mode: &str) -> Report {
    let payload = format!(r#"{{"session_id":"s-1","permission_mode":"{mode}"}}"#);
    let registry = registry.to_owned();
    let env = move |name: &str| match name {
        "PURLIS_CHAT" | "CHARTER_CHAT" => Some("3".to_owned()),
        "PURLIS_HARNESS" | "CHARTER_HARNESS" => Some(registry.clone()),
        _ => None,
    };
    Report::read(purlis_core::state::Event::Stop, &payload, &env).expect("a report")
}

/// `fact`, once it heard its harness report its prompts off.
fn heard_prompts_off(mut fact: Fact, registry: &str) -> Fact {
    fact.heard(&report(registry, attendance::PROMPTS_OFF));
    fact
}

/// What the grants answer a dispatch from steward to devops, by the asking chat's attendance,
/// as the dispatch grants route it: a chat a person is at is asked of every grant in force,
/// its own chat's included, and one nobody is at of the standing grants alone
/// ([`dispatchunattended::covers`]), sandboxed.
fn grants_answer(attendance: Attendance, grants: &InForce) -> Result<Covers, Answer> {
    let none = Locks::none();
    match attendance {
        Attendance::Attended => Ok(dispatchgrant::covers(
            Some("steward"),
            "devops",
            grants,
            &none,
        )),
        Attendance::Unattended => Err(dispatchunattended::covers(
            Some("steward"),
            "devops",
            grants,
            &none,
            false,
            true,
        )),
    }
}

/// **R-110**: the same chat, flipped only in attendance, gets the other dispatch answer, both
/// through the handoff path (its own attendance judgement, which the dispatch-away path and
/// `chat_attended` read too) and through the dispatch grants. Flipped two ways: by the fact
/// itself (the chat is a task, which nobody is at, R-106) and by a report its harness made
/// (prompts off).
#[test]
fn dispatch_unattended_rules_key_on_attendance() {
    purlis_core::unsteered!();
    // A grant made for this one chat: it counts for a chat a person is at, never for one
    // nobody is at.
    let grants = InForce {
        chat: vec![ChatPair {
            asking: Some("steward".to_owned()),
            target: "devops".to_owned(),
        }],
        ..InForce::default()
    };

    // The handoff path's judgement: the fact the app holds.
    let attended = attendance::of_held(Some(a_persons_chat()));
    let a_task = attendance::of_held(Some(a_task()));
    let prompts_off = attendance::of_held(Some(heard_prompts_off(a_persons_chat(), "claude-code")));
    let none_held = attendance::of_held(None);
    assert_eq!(attended, Attendance::Attended);
    assert_eq!(a_task, Attendance::Unattended, "a task has nobody at it");
    assert_eq!(prompts_off, Attendance::Unattended, "a report only lowers");
    assert_eq!(
        none_held,
        Attendance::Unattended,
        "no fact held is nobody's"
    );

    // Through the dispatch grants: covered for the chat a person is at, refused, never asked,
    // for the same chat nobody is at.
    assert_eq!(grants_answer(attended, &grants), Ok(Covers::Covered));
    for unattended in [a_task, prompts_off, none_held] {
        assert!(
            matches!(
                grants_answer(unattended, &grants),
                Err(Answer::Refused(Refusal::Missing(_)))
            ),
            "a grant of one chat never counts for a chat nobody is at"
        );
    }
}

/// **A profile's own command that switches the prompts off lowers the fact**, as the old mark
/// did: the same chat on a profile that asks is attended, and on one that does not is not.
#[test]
fn a_profile_that_switches_the_prompts_off_makes_its_chat_unattended() {
    purlis_core::unsteered!();
    let on = |command: &[&str]| {
        attendance::of_held(Some(Fact::at_start(
            Begun::Person,
            Some(Harness::ClaudeCode),
            &words(command),
        )))
    };
    assert_eq!(on(&["claude"]), Attendance::Attended);
    assert_eq!(
        on(&["claude", "--dangerously-skip-permissions"]),
        Attendance::Unattended
    );
    assert_eq!(
        on(&["claude", "--permission-mode", attendance::PROMPTS_OFF]),
        Attendance::Unattended
    );
}

/// **R-110 at the core seam** (T1-22): a Codex chat a person started, whose start words name
/// `bypassPermissions` where no flag sets a mode, and whose every report says it, is attended to
/// dispatch: Codex names the bypass for every chat (R-289), so its fact never hears it.
///
/// Mutation that shows it red: let [`Fact::heard`] lower on the bypass bit on every harness.
#[test]
fn a_codex_chat_a_person_started_is_attended_to_dispatch_whatever_it_reports() {
    purlis_core::unsteered!();
    let mut fact = Fact::at_start(
        Begun::Person,
        Some(Harness::Codex),
        &words(&["codex", "--model", attendance::PROMPTS_OFF]),
    );
    for _ in 0..3 {
        fact = heard_prompts_off(fact, "codex");
    }
    assert_eq!(attendance::of_held(Some(fact)), Attendance::Attended);
}

/// **R-113, a pin** (Q208): at No sandbox, an unattended task can't dispatch to another
/// persona, whatever the grants say, as shipped. Its own persona is still its own.
///
/// Shipped behaviour, so it is shown red by a mutation, not first: delete the
/// `Covers::Covered | Covers::NeedsGrant if !sandboxed` arm of
/// `purlis_core::dispatchunattended::answer_of`, and the standing grant answers
/// `Answer::Covered`. A second mutation, `Begun::Task` read as attended in
/// `attendance::Begun::unattended`, makes the task attended and the first assertion fails.
#[test]
fn an_unattended_no_sandbox_task_cannot_dispatch_to_another_persona() {
    purlis_core::unsteered!();
    let attendance = attendance::of_held(Some(a_task()));
    assert_eq!(attendance, Attendance::Unattended);
    // A standing grant for the pair, made by the person on this machine.
    let grants = InForce {
        you: vec![dispatchgrant::Pair::new("steward", "devops").expect("a pair")],
        ..InForce::default()
    };
    let none = Locks::none();
    let unsandboxed = false;
    assert_eq!(
        dispatchunattended::covers(
            Some("steward"),
            "devops",
            &grants,
            &none,
            false,
            unsandboxed
        ),
        Answer::Refused(Refusal::Unsandboxed("devops".to_owned()))
    );
    assert_eq!(
        dispatchunattended::covers(
            Some("steward"),
            "steward",
            &grants,
            &none,
            false,
            unsandboxed
        ),
        Answer::Covered,
        "its own persona is still its own"
    );
}
