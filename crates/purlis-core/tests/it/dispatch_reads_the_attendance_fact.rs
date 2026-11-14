// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Dispatch reads the attendance fact** (T1-19, #6410; R-110, R-113): whether a chat that
//! asks to dispatch is attended is [`purlis_core::attendance::Fact`], read from the app's
//! record of the chat ([`dispatchunattended::fact_of`]) and lowered, never raised, by what its
//! hook reports said ([`dispatchunattended::attendance_of`]), until the fact's holder hears
//! those reports itself (T1-18, #6409).
//!
//! **Each pin names the mutation that shows it red** (Q208), in its doc comment.

use purlis_core::active::Place;
use purlis_core::dispatchgrant::{self, ChatPair, Covers, InForce};
use purlis_core::dispatchunattended::{self, Answer, Attendance, Mark, Refusal};
use purlis_core::reopen::{Chat, HandedFrom, Mode, Owed};
use purlis_core::sandbox::policy::Locks;

/// A Claude Code chat as steward, as the app records it: a person started it from the window.
fn a_persons_chat() -> Chat {
    Chat {
        program: "claude".to_owned(),
        persona: Some("steward".to_owned()),
        number: Some(3),
        ..Chat::default()
    }
}

/// The same chat, but another chat dispatched it as a task.
fn a_task() -> Chat {
    Chat {
        from: Some(HandedFrom {
            chat: 1,
            id: None,
            name: "steward 1".to_owned(),
            workspace: Place::PlaneRoot,
            report: Owed::Due,
            mode: Mode::Task,
            depth: 1,
            root: None,
            above: Some(vec![Some("steward".to_owned())]),
            by_person: false,
            long: false,
        }),
        ..a_persons_chat()
    }
}

/// A mark that heard its harness report its prompts off.
fn heard_prompts_off() -> Mark {
    let mut mark = Mark::default();
    mark.heard(Some(purlis_core::floorguard::UNATTENDED_MODE));
    mark
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

    // The handoff path's judgement: the fact, lowered by the mark.
    let attended = dispatchunattended::attendance_of(
        dispatchunattended::fact_of(&a_persons_chat(), None),
        Mark::default(),
    );
    let a_task = dispatchunattended::attendance_of(
        dispatchunattended::fact_of(&a_task(), None),
        Mark::default(),
    );
    let prompts_off = dispatchunattended::attendance_of(
        dispatchunattended::fact_of(&a_persons_chat(), None),
        heard_prompts_off(),
    );
    assert_eq!(attended, Attendance::Attended);
    assert_eq!(a_task, Attendance::Unattended, "a task has nobody at it");
    assert_eq!(prompts_off, Attendance::Unattended, "the mark only lowers");

    // Through the dispatch grants: covered for the chat a person is at, refused, never asked,
    // for the same chat nobody is at.
    assert_eq!(grants_answer(attended, &grants), Ok(Covers::Covered));
    for unattended in [a_task, prompts_off] {
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
    let asks = ["claude".to_owned()];
    let prompts_off = [
        "claude".to_owned(),
        "--dangerously-skip-permissions".to_owned(),
    ];
    assert_eq!(
        dispatchunattended::attendance_of(
            dispatchunattended::fact_of(&a_persons_chat(), Some(&asks)),
            Mark::default(),
        ),
        Attendance::Attended
    );
    assert_eq!(
        dispatchunattended::attendance_of(
            dispatchunattended::fact_of(&a_persons_chat(), Some(&prompts_off)),
            Mark::default(),
        ),
        Attendance::Unattended
    );
    // And the chat's own words, where its record has them.
    let own = Chat {
        args: vec![
            "--permission-mode".to_owned(),
            "bypassPermissions".to_owned(),
        ],
        ..a_persons_chat()
    };
    assert_eq!(
        dispatchunattended::attendance_of(dispatchunattended::fact_of(&own, None), Mark::default()),
        Attendance::Unattended
    );
}

/// **R-113, a pin** (Q208): at No sandbox, an unattended task can't dispatch to another
/// persona, whatever the grants say, as shipped. Its own persona is still its own.
///
/// Shipped behaviour, so it is shown red by a mutation, not first: delete the
/// `Covers::Covered | Covers::NeedsGrant if !sandboxed` arm of
/// `purlis_core::dispatchunattended::answer_of`, and the standing grant answers
/// `Answer::Covered`. A second mutation, `Begun::Task => Begun::Person` in
/// `dispatchunattended::fact_of`, makes the task attended and the first assertion fails.
#[test]
fn an_unattended_no_sandbox_task_cannot_dispatch_to_another_persona() {
    purlis_core::unsteered!();
    let attendance = dispatchunattended::attendance_of(
        dispatchunattended::fact_of(&a_task(), None),
        Mark::default(),
    );
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
