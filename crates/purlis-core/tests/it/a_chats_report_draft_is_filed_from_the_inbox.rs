// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat's report is a draft in the person's Inbox, and only the person files it** (#5694,
//! spec #5683, Q2/Q3).
//!
//! A chat holds no sign-in (ADR 0070), so `purlis report bug` run in one only drafts. With the
//! person's yes to Agent drafts the Reports module holds the draft for the Inbox, under the
//! machine's config root; without it nothing is held. From the Inbox the person reads it, edits
//! it, dismisses it or files it: on purlis/purlis as themselves when signed in to GitHub, through
//! the duplicate search, and to the Report intake when not. Either way the sent list records
//! where it went.
//!
//! Every case drives the Reports module through its interface. The forge is the recording
//! `a_report_is_filed_only_with_the_purlis_sign_in` signs in to; the intake is the real Sentry
//! adapter over the recording `a_panic_becomes_a_crash_report_in_the_report_intake` uses.

use std::path::Path;
use std::sync::Arc;

use purlis_core::forge::Caller;
use purlis_core::report::intake::{Dsn, Recorded, Sentry};
use purlis_core::report::{
    AgentDraft, CameFrom, Consent, Evidence, Filing, Held, How, Kind, Known, Refusal, Reports,
    SentList, SentTo, Stream,
};
use serde_json::json;

use crate::a_crash_report_goes_once_per_version_and_waits_while_offline::Line;
use crate::a_report_is_filed_only_with_the_purlis_sign_in::{Machine, issue};

const DSN: &str = concat!("https://f00dfeed1234", "@", "o1.ingest.de.sentry.io/4508");

pub(crate) fn recorded_intake() -> Arc<Recorded> {
    let text = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/intake_contract/sentry/envelope.json"),
    )
    .expect("the recording");
    Arc::new(Recorded::parse(&text).expect("a recording"))
}

fn consent(agent_drafts: Option<bool>) -> Consent {
    Consent {
        crash_reports: None,
        agent_drafts,
        always_send: None,
        asked: true,
        crash_notice: None,
    }
}

/// Reports knowing nothing of this machine, kept under `root`, sending to `intake`.
pub(crate) fn reports(root: &Path, agent_drafts: Option<bool>, intake: &Arc<Recorded>) -> Reports {
    Reports::new(Known::default())
        .with_consent(consent(agent_drafts))
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

#[test]
fn without_a_yes_to_agent_drafts_a_chats_draft_is_refused_and_nothing_is_held() {
    purlis_core::unsteered!();
    let root = tempfile::tempdir().unwrap();
    let intake = recorded_intake();
    for unanswered in [None, Some(false)] {
        let reports = reports(root.path(), unanswered, &intake);
        assert_eq!(
            reports.hold(the_agents_draft(), &Caller::chat("4"), from()),
            Err(Refusal::NoConsent(Stream::AgentDrafts))
        );
        assert!(reports.held().is_empty(), "{:?}", reports.held());
    }
}

#[test]
fn with_the_yes_a_chats_draft_is_held_for_the_inbox_naming_its_chat_and_its_call() {
    purlis_core::unsteered!();
    let root = tempfile::tempdir().unwrap();
    let reports = reports(root.path(), Some(true), &recorded_intake());
    let one = held(&reports);
    assert_eq!(one.title, TITLE);
    assert_eq!(one.from, from());
    assert!(
        one.line
            .starts_with("purlis worktree update · exit 1 · purlis "),
        "{}",
        one.line
    );
    assert!(one.body.contains("It had none."), "{}", one.body);
    assert_eq!(reports.held(), [one]);
}

#[test]
fn a_draft_edited_in_the_inbox_is_kept_as_edited_and_its_digest_follows_it() {
    purlis_core::unsteered!();
    let root = tempfile::tempdir().unwrap();
    let reports = reports(root.path(), Some(true), &recorded_intake());
    let one = held(&reports);
    let edited = reports
        .edit_held(&one.id, "A shorter title", "What I saw.")
        .expect("edited");
    assert_eq!(
        (edited.title.as_str(), edited.body.as_str()),
        ("A shorter title", "What I saw.")
    );
    assert_ne!(edited.digest(), one.digest());
    assert_eq!(reports.held(), [edited]);
}

#[test]
fn a_dismissed_draft_is_gone_and_nothing_was_sent() {
    purlis_core::unsteered!();
    let root = tempfile::tempdir().unwrap();
    let intake = recorded_intake();
    let reports = reports(root.path(), Some(true), &intake);
    let one = held(&reports);
    reports.dismiss_held(&one.id);
    assert!(reports.held().is_empty());
    assert!(intake.received().is_empty());
    assert!(SentList::read(root.path()).is_empty());
}

#[test]
fn without_a_github_sign_in_a_filed_draft_goes_to_the_intake_and_is_listed() {
    purlis_core::unsteered!();
    let root = tempfile::tempdir().unwrap();
    let intake = recorded_intake();
    let reports = reports(root.path(), Some(true), &intake);
    let one = held(&reports);
    let filed = reports.file_held(&one.id, &Caller::window(), &one.digest(), How::Check);
    let Filing::Sent { to, address } = filed else {
        panic!("not sent: {filed:?}");
    };
    assert_eq!(to, SentTo::Intake);
    assert!(
        address.starts_with("purlis's report service, event "),
        "{address}"
    );

    let bytes = intake.received();
    assert_eq!(bytes.len(), 1);
    let text = String::from_utf8(bytes[0].clone()).unwrap();
    assert!(text.contains(TITLE), "{text}");
    assert!(text.contains("It had none."), "{text}");
    assert!(text.contains("\"written_by\":\"agent\""), "{text}");

    let sent = SentList::read(root.path());
    assert_eq!(sent.len(), 1);
    assert_eq!(
        (sent[0].to, sent[0].title.as_str()),
        (SentTo::Intake, TITLE)
    );
    assert_eq!(sent[0].bytes, text, "the list keeps exactly what left");
    assert!(reports.held().is_empty(), "a filed draft leaves the Inbox");
}

#[test]
fn a_draft_filed_while_the_intake_cannot_be_reached_waits_and_goes_when_it_answers() {
    purlis_core::unsteered!();
    let root = tempfile::tempdir().unwrap();
    let line = Line::down(1);
    // No yes to Crash reports: the person's File is the yes this draft needs.
    let reports = Reports::new(Known::default())
        .with_consent(consent(Some(true)))
        .sending_to(
            Arc::new(Sentry::new(Dsn::parse(DSN).expect("a DSN"), line.clone())),
            root.path(),
        );
    let one = held(&reports);

    let filed = reports.file_held(&one.id, &Caller::window(), &one.digest(), How::Check);
    assert_eq!(filed, Filing::Queued);
    assert!(reports.held().is_empty(), "a queued draft leaves the Inbox");
    let waiting = SentList::read(root.path());
    assert_eq!(waiting.len(), 1);
    assert!(waiting[0].waiting, "{waiting:?}");
    assert_eq!(
        waiting[0].fingerprint, None,
        "a filed draft is never counted away"
    );
    assert!(line.received().is_empty());

    line.set(true);
    assert_eq!(reports.send_waiting(), 1);
    let received = line.received();
    assert_eq!(received.len(), 1);
    assert!(received[0].contains(TITLE), "{}", received[0]);
    let sent = SentList::read(root.path());
    assert!(!sent[0].waiting, "{sent:?}");
    assert_eq!(
        sent[0].bytes, received[0],
        "the list keeps exactly what left"
    );
}

#[test]
fn a_digest_other_than_the_drafts_files_nothing() {
    purlis_core::unsteered!();
    let root = tempfile::tempdir().unwrap();
    let intake = recorded_intake();
    let reports = reports(root.path(), Some(true), &intake);
    let one = held(&reports);
    let filed = reports.file_held(&one.id, &Caller::window(), "000000000000", How::Check);
    assert_eq!(
        filed,
        Filing::Refused(Refusal::NotTheShownDraft {
            digest: one.digest()
        })
    );
    assert!(intake.received().is_empty());
    assert_eq!(reports.held(), [one]);
}

/// The search GitHub is sent for the draft's title, answered with `items`.
fn searched(items: serde_json::Value) -> serde_json::Value {
    json!({"call": {"endpoint": {"rest": {"method": null, "path":
              "search/issues?q=repo%3Apurlis%2Fpurlis%20is%3Aissue%20purlis%20worktree%20update%20refuses%20clean%20branch&per_page=5"}},
              "fields": []},
           "reply": {"code": 0, "err": "", "out": json!({"total_count": 0,
              "incomplete_results": false, "items": items}).to_string()}})
}

#[test]
fn signed_in_to_github_a_filed_draft_is_filed_as_the_person_and_listed_with_its_address() {
    purlis_core::unsteered!();
    if crate::alone() {
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let intake = recorded_intake();
    let reports = reports(root.path(), Some(true), &intake);
    let one = held(&reports);
    let machine = Machine::new(
        json!([
            searched(json!([])),
            {"call": {"endpoint": {"rest": {"method": "POST", "path": "repos/purlis/purlis/issues"}},
                      "fields": [{"text": ["title", TITLE]}, {"text": ["body", one.body]}]},
             "reply": {"code": 0, "err": "", "out": issue(42, TITLE, "open").to_string()}}
        ]),
        true,
    );
    let filed = reports.file_held(&one.id, &Caller::window(), &one.digest(), How::Check);
    assert_eq!(
        filed,
        Filing::Sent {
            to: SentTo::GitHub,
            address: "https://github.com/purlis/purlis/issues/42".into()
        }
    );
    assert!(intake.received().is_empty(), "nothing went to the intake");
    let sent = SentList::read(root.path());
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].to, SentTo::GitHub);
    assert_eq!(
        sent[0].address,
        "https://github.com/purlis/purlis/issues/42"
    );
    assert!(sent[0].bytes.contains("It had none."), "{}", sent[0].bytes);
    assert!(reports.held().is_empty());
    assert!(
        machine.recorded.unspent().is_empty(),
        "{:?}",
        machine.recorded.unspent()
    );
}

#[test]
fn a_possible_duplicate_is_shown_first_and_the_draft_is_added_to_it_instead_of_filed() {
    purlis_core::unsteered!();
    if crate::alone() {
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let intake = recorded_intake();
    let reports = reports(root.path(), Some(true), &intake);
    let one = held(&reports);
    let machine = Machine::new(
        json!([
            searched(json!([issue(41, "worktree update refuses clean branches", "open")])),
            {"call": {"endpoint": {"rest": {"method": "POST",
                                            "path": "repos/purlis/purlis/issues/41/comments"}},
                      "fields": [{"text": ["body", format!("**{TITLE}**\n\n{}", one.body)]}]},
             "reply": {"code": 0, "err": "",
                       "out": json!({"id": 9001, "html_url":
                           "https://github.com/purlis/purlis/issues/41#issuecomment-9001"})
                           .to_string()}}
        ]),
        true,
    );
    let checked = reports.file_held(&one.id, &Caller::window(), &one.digest(), How::Check);
    let Filing::Duplicates(hits) = checked else {
        panic!("no duplicates shown: {checked:?}");
    };
    assert_eq!(hits.iter().map(|hit| hit.number).collect::<Vec<_>>(), [41]);
    assert_eq!(
        reports.held(),
        std::slice::from_ref(&one),
        "nothing filed yet"
    );

    let added = reports.file_held(&one.id, &Caller::window(), &one.digest(), How::AddTo(41));
    assert_eq!(
        added,
        Filing::Sent {
            to: SentTo::GitHub,
            address: "https://github.com/purlis/purlis/issues/41#issuecomment-9001".into()
        }
    );
    let sent = SentList::read(root.path());
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].to, SentTo::GitHub);
    assert!(reports.held().is_empty());
    assert!(
        machine.recorded.unspent().is_empty(),
        "{:?}",
        machine.recorded.unspent()
    );
}
