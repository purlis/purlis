// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Always send: a draft goes unread to the Report intake, held first with Undo, five a day**
//! (#5695, spec #5683, Q2b; ADR 0118).
//!
//! With the person's yes to Agent drafts and to Always send, a chat's draft skips the Inbox: it is
//! held for [`HOLD_SECS`] with Undo, then sent to the Report intake, never to GitHub, signed in
//! or not. Undo within the hold sends nothing and leaves the draft in the Inbox. Past five
//! unread drafts in a UTC day, a draft falls back to asking: it is held for the Inbox like any
//! other. Every case drives the Reports module through its interface; the forge is the recording
//! `a_report_is_filed_only_with_the_purlis_sign_in` signs in to, with nothing it may answer.

use std::path::Path;
use std::sync::Arc;

use purlis_core::forge::Caller;
use purlis_core::report::intake::{Dsn, Recorded, Sentry};
use purlis_core::report::{
    AgentDraft, CameFrom, Consent, Evidence, HOLD_SECS, Held, Kind, Known, Refusal, Reports,
    SentList, SentTo, Stream,
};
use serde_json::json;

use crate::a_report_is_filed_only_with_the_purlis_sign_in::Machine;

const DSN: &str = concat!("https://f00dfeed1234", "@", "o1.ingest.de.sentry.io/4508");

/// The intake's recorded answer, taken six times: a day's cap and one more.
fn recorded_intake() -> Arc<Recorded> {
    let text = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/intake_contract/sentry/envelope.json"),
    )
    .expect("the recording");
    let mut recording: serde_json::Value = serde_json::from_str(&text).expect("JSON");
    let one = recording["exchanges"][0].clone();
    recording["exchanges"] = json!(vec![one; 6]);
    Arc::new(Recorded::parse(&recording.to_string()).expect("a recording"))
}

fn consent(always_send: Option<bool>) -> Consent {
    Consent {
        crash_reports: None,
        agent_drafts: Some(true),
        always_send,
        asked: true,
        crash_notice: None,
    }
}

/// Reports knowing nothing of this machine, kept under `root`, sending to `intake`.
fn reports(root: &Path, consent: Consent, intake: &Arc<Recorded>) -> Reports {
    Reports::new(Known::default())
        .with_consent(consent)
        .sending_to(
            Arc::new(Sentry::new(Dsn::parse(DSN).expect("a DSN"), intake.clone())),
            root,
        )
}

const TITLE: &str = "purlis worktree update refuses a clean branch";

fn the_agents_draft() -> Evidence {
    Evidence::AgentDraft(AgentDraft {
        kind: Kind::Bug,
        text: format!("{TITLE}\n\nIt said the branch had uncommitted changes. It had none."),
        title: None,
        call: Some(("purlis worktree update".into(), 1)),
        ran_as: purlis_core::report::RanAs::default(),
    })
}

fn from() -> CameFrom {
    CameFrom {
        session: Some(4),
        chat: "steward 4".into(),
        project: "/plane".into(),
    }
}

fn held(reports: &Reports) -> Held {
    reports
        .hold(the_agents_draft(), &Caller::chat("4"), from())
        .expect("held")
}

fn after_the_hold() -> chrono::DateTime<chrono::Utc> {
    chrono::Utc::now() + chrono::Duration::seconds(i64::from(HOLD_SECS) + 1)
}

#[test]
fn an_always_send_draft_is_held_then_goes_to_the_intake_and_never_to_github() {
    purlis_core::unsteered!();
    if crate::alone() {
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let intake = recorded_intake();
    let reports = reports(root.path(), consent(Some(true)), &intake);
    // Signed in to GitHub, which may answer nothing: any call to it fails the test.
    let machine = Machine::new(json!([]), true);

    let one = held(&reports);
    assert!(one.sends_at.is_some(), "held with Undo: {one:?}");
    assert_eq!(
        reports.send_held_due(chrono::Utc::now()),
        0,
        "nothing leaves within the hold"
    );
    assert!(intake.received().is_empty());

    assert_eq!(reports.send_held_due(after_the_hold()), 1);
    let bytes = intake.received();
    assert_eq!(bytes.len(), 1);
    let text = String::from_utf8(bytes[0].clone()).unwrap();
    assert!(text.contains("It had none."), "{text}");
    assert!(text.contains("\"written_by\":\"agent\""), "{text}");
    assert!(
        text.contains("\"reviewed_by_the_person\":false"),
        "marked unreviewed: {text}"
    );

    let sent = SentList::read(root.path());
    assert_eq!(sent.len(), 1);
    assert_eq!((sent[0].to, sent[0].unread_draft), (SentTo::Intake, true));
    assert_eq!(sent[0].bytes, text, "the list keeps exactly what left");
    assert!(reports.held().is_empty(), "a sent draft leaves the Inbox");
    machine.nothing_sent();
}

#[test]
fn undo_within_the_hold_sends_nothing_and_the_draft_waits_in_the_inbox() {
    let root = tempfile::tempdir().unwrap();
    let intake = recorded_intake();
    let reports = reports(root.path(), consent(Some(true)), &intake);
    let one = held(&reports);

    let undone = reports.undo_held(&one.id).expect("undone within the hold");
    assert_eq!(undone.sends_at, None);

    assert_eq!(reports.send_held_due(after_the_hold()), 0);
    assert!(intake.received().is_empty());
    assert!(SentList::read(root.path()).is_empty());
    assert_eq!(reports.held(), [undone], "the draft waits for the person");
}

#[test]
fn undo_after_the_draft_left_is_refused() {
    let root = tempfile::tempdir().unwrap();
    let reports = reports(root.path(), consent(Some(true)), &recorded_intake());
    let one = held(&reports);
    assert_eq!(reports.send_held_due(after_the_hold()), 1);
    assert_eq!(reports.undo_held(&one.id), Err(Refusal::NotHeld));
}

#[test]
fn the_sixth_draft_of_the_day_falls_back_to_a_draft_in_the_inbox() {
    let root = tempfile::tempdir().unwrap();
    let intake = recorded_intake();
    let reports = reports(root.path(), consent(Some(true)), &intake);
    for _ in 0..5 {
        assert!(held(&reports).sends_at.is_some());
    }
    assert_eq!(reports.send_held_due(after_the_hold()), 5);

    let sixth = held(&reports);
    assert_eq!(sixth.sends_at, None, "past the cap a draft asks");
    assert_eq!(reports.send_held_due(after_the_hold()), 0);
    assert_eq!(intake.received().len(), 5);
    assert_eq!(reports.held(), [sixth]);
}

#[test]
fn drafts_still_held_count_against_the_cap() {
    let root = tempfile::tempdir().unwrap();
    let reports = reports(root.path(), consent(Some(true)), &recorded_intake());
    for _ in 0..5 {
        assert!(held(&reports).sends_at.is_some());
    }
    assert_eq!(held(&reports).sends_at, None);
}

#[test]
fn without_always_send_a_draft_waits_in_the_inbox_and_nothing_is_sent() {
    let root = tempfile::tempdir().unwrap();
    let intake = recorded_intake();
    for not_yes in [None, Some(false)] {
        let reports = reports(root.path(), consent(not_yes), &intake);
        let one = held(&reports);
        assert_eq!(one.sends_at, None);
        assert_eq!(reports.send_held_due(after_the_hold()), 0);
    }
    assert!(intake.received().is_empty());
}

#[test]
fn always_send_turned_off_during_the_hold_keeps_the_draft_in_the_inbox() {
    let root = tempfile::tempdir().unwrap();
    let intake = recorded_intake();
    let one = held(&reports(root.path(), consent(Some(true)), &intake));

    let now_off = reports(root.path(), consent(Some(false)), &intake);
    assert_eq!(now_off.send_held_due(after_the_hold()), 0);
    assert!(intake.received().is_empty());
    let back = now_off.held();
    assert_eq!(back.len(), 1);
    assert_eq!(
        (back[0].id.as_str(), back[0].sends_at.as_ref()),
        (one.id.as_str(), None)
    );
}

#[test]
fn always_send_on_a_card_holds_it_and_needs_the_standing_yes() {
    let root = tempfile::tempdir().unwrap();
    let intake = recorded_intake();
    let asking = reports(root.path(), consent(None), &intake);
    let one = held(&asking);
    assert_eq!(
        asking.always_send_held(&one.id),
        Err(Refusal::NoConsent(Stream::AgentDrafts)),
        "Always send asks first"
    );

    let said_yes = reports(root.path(), consent(Some(true)), &intake);
    let sending = said_yes.always_send_held(&one.id).expect("held to send");
    assert!(sending.sends_at.is_some());
    assert_eq!(said_yes.send_held_due(after_the_hold()), 1);
    assert_eq!(intake.received().len(), 1);
}

#[test]
fn always_send_on_a_card_past_the_cap_is_refused_and_the_draft_stays() {
    let root = tempfile::tempdir().unwrap();
    let reports = reports(root.path(), consent(Some(true)), &recorded_intake());
    for _ in 0..5 {
        held(&reports);
    }
    let sixth = held(&reports);
    assert_eq!(
        reports.always_send_held(&sixth.id),
        Err(Refusal::CapReached)
    );
    assert!(
        reports
            .held()
            .iter()
            .any(|one| one.id == sixth.id && one.sends_at.is_none())
    );
}

#[test]
fn an_agent_s_words_after_the_hold_follow_its_draft_to_the_intake_linked_to_it() {
    let root = tempfile::tempdir().unwrap();
    let intake = recorded_intake();
    let reports = reports(root.path(), consent(Some(true)), &intake);
    let one = held(&reports);
    assert_eq!(reports.send_held_due(after_the_hold()), 1);
    let first = String::from_utf8(intake.received()[0].clone()).unwrap();
    let first_id = serde_json::from_str::<serde_json::Value>(first.lines().next().unwrap())
        .unwrap()["event_id"]
        .as_str()
        .unwrap()
        .to_owned();

    assert_eq!(
        reports.add_to_held(&one.id, 5, "I was updating chat 5's branch"),
        Err(Refusal::NotHeld),
        "another chat's draft is not this chat's to add to, sent or not"
    );
    let added = reports
        .add_to_held(&one.id, 4, "I was updating the reporting branch.")
        .expect("followed");

    assert!(
        matches!(added, purlis_core::report::Added::SentAfter(Some(_))),
        "{added:?}"
    );
    let received = intake.received();
    assert_eq!(received.len(), 2, "the words left after the draft");
    let words = String::from_utf8(received[1].clone()).unwrap();
    assert!(
        words.contains("I was updating the reporting branch."),
        "{words}"
    );
    assert!(
        words.contains(&format!("\"follows\":\"{first_id}\"")),
        "linked to the draft's event: {words}"
    );
    assert!(words.contains("\"written_by\":\"agent\""), "{words}");
    let sent = SentList::read(root.path());
    assert_eq!(sent.len(), 2, "{sent:?}");
    assert_eq!(sent[1].bytes, words, "the list keeps exactly what left");
    assert!(
        !sent[1].unread_draft,
        "words about a draft already counted are not one more against the cap"
    );
}

#[test]
fn with_always_send_off_since_the_draft_left_the_agent_s_words_are_not_sent() {
    let root = tempfile::tempdir().unwrap();
    let intake = recorded_intake();
    let one = held(&reports(root.path(), consent(Some(true)), &intake));
    assert_eq!(
        reports(root.path(), consent(Some(true)), &intake).send_held_due(after_the_hold()),
        1
    );

    let now_off = reports(root.path(), consent(Some(false)), &intake);
    assert_eq!(
        now_off.add_to_held(&one.id, 4, "I was updating the branch."),
        Err(Refusal::NoConsent(Stream::AgentDrafts))
    );
    assert_eq!(intake.received().len(), 1);
}
