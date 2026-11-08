// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A project's rule keeps its drafts from going unread** (#5696, spec #5683, Q12).
//!
//! A team turns off `[reports].always_send` in the project's shared settings file, which the
//! team sees: "Members may let agents send reports without asking". Then no draft written in
//! that project goes to the Report intake unread, whatever a member's own Always send says: it
//! waits in the Inbox, and Always send on its card is refused. The member can still read and
//! File it by hand. A draft from another project is not held by the rule. Every case drives the
//! Reports module through its interface.

use std::path::Path;
use std::sync::Arc;

use purlis_core::forge::Caller;
use purlis_core::report::intake::{Dsn, Recorded, Sentry};
use purlis_core::report::{
    AgentDraft, CameFrom, Consent, Evidence, Filing, HOLD_SECS, Held, How, Kind, Known, Refusal,
    Reports, SentList, SentTo, members_may_always_send,
};
use serde_json::json;

const DSN: &str = concat!("https://f00dfeed1234", "@", "o1.ingest.de.sentry.io/4508");

fn recorded_intake() -> Arc<Recorded> {
    let text = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/intake_contract/sentry/envelope.json"),
    )
    .expect("the recording");
    let mut recording: serde_json::Value = serde_json::from_str(&text).expect("JSON");
    let one = recording["exchanges"][0].clone();
    recording["exchanges"] = json!(vec![one; 2]);
    Arc::new(Recorded::parse(&recording.to_string()).expect("a recording"))
}

/// A member who said yes to Agent drafts and to Always send.
fn always_sends(root: &Path, intake: &Arc<Recorded>) -> Reports {
    Reports::new(Known::default())
        .with_consent(Consent {
            crash_reports: None,
            agent_drafts: Some(true),
            always_send: Some(true),
            asked: true,
            crash_notice: None,
        })
        .sending_to(
            Arc::new(Sentry::new(Dsn::parse(DSN).expect("a DSN"), intake.clone())),
            root,
        )
}

/// A project whose shared settings file says `toml`.
fn project(toml: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("purlis.toml"), toml).unwrap();
    dir
}

const FORBIDS: &str = "[reports]\nalways_send = false\n";

fn held_from(reports: &Reports, project: &Path) -> Held {
    reports
        .hold(
            Evidence::AgentDraft(AgentDraft {
                kind: Kind::Bug,
                text: "The Inbox counts a dismissed Notice\n\nIt said 2; there was one.".into(),
                title: None,
                call: None,
                ran_as: purlis_core::report::RanAs::default(),
            }),
            &Caller::chat("4"),
            CameFrom {
                session: Some(4),
                chat: "steward 4".into(),
                project: project.display().to_string(),
            },
        )
        .expect("held")
}

fn after_the_hold() -> chrono::DateTime<chrono::Utc> {
    chrono::Utc::now() + chrono::Duration::seconds(i64::from(HOLD_SECS) + 1)
}

#[test]
fn members_may_always_send_unless_the_projects_shared_file_turns_it_off() {
    assert!(members_may_always_send(project("").path()), "on by default");
    assert!(members_may_always_send(
        project("[reports]\nalways_send = true\n").path()
    ));
    assert!(!members_may_always_send(project(FORBIDS).path()));
    let nowhere = tempfile::tempdir().unwrap();
    assert!(
        members_may_always_send(nowhere.path()),
        "no settings file: nothing turns it off"
    );
}

#[test]
fn this_machines_own_file_cannot_turn_the_teams_rule_back_on() {
    let forbids = project(FORBIDS);
    std::fs::write(
        forbids.path().join("purlis.local.toml"),
        "[reports]\nalways_send = true\n",
    )
    .unwrap();
    assert!(!members_may_always_send(forbids.path()));
}

#[test]
fn with_the_rule_off_a_draft_from_that_project_waits_in_the_inbox_and_nothing_is_sent() {
    let root = tempfile::tempdir().unwrap();
    let intake = recorded_intake();
    let reports = always_sends(root.path(), &intake);
    let forbids = project(FORBIDS);

    let one = held_from(&reports, forbids.path());
    assert_eq!(one.sends_at, None, "it asks: {one:?}");
    assert_eq!(reports.send_held_due(after_the_hold()), 0);
    assert!(intake.received().is_empty());
    assert_eq!(reports.held(), [one]);
}

#[test]
fn with_the_rule_off_always_send_on_its_card_is_refused() {
    let root = tempfile::tempdir().unwrap();
    let reports = always_sends(root.path(), &recorded_intake());
    let forbids = project(FORBIDS);
    let one = held_from(&reports, forbids.path());
    assert_eq!(
        reports.always_send_held(&one.id),
        Err(Refusal::ProjectForbids)
    );
    assert_eq!(reports.held()[0].sends_at, None);
}

#[test]
fn a_rule_turned_off_during_the_hold_keeps_the_draft_in_the_inbox() {
    let root = tempfile::tempdir().unwrap();
    let intake = recorded_intake();
    let reports = always_sends(root.path(), &intake);
    let allows = project("");
    let one = held_from(&reports, allows.path());
    assert!(one.sends_at.is_some());

    std::fs::write(allows.path().join("purlis.toml"), FORBIDS).unwrap();
    assert_eq!(reports.send_held_due(after_the_hold()), 0);
    assert!(intake.received().is_empty());
    assert_eq!(reports.held()[0].sends_at, None);
}

#[test]
fn a_draft_from_a_project_without_the_rule_still_goes_unread() {
    let root = tempfile::tempdir().unwrap();
    let intake = recorded_intake();
    let reports = always_sends(root.path(), &intake);
    let forbids = project(FORBIDS);
    let other = project("");
    held_from(&reports, forbids.path());
    assert!(held_from(&reports, other.path()).sends_at.is_some());
    assert_eq!(reports.send_held_due(after_the_hold()), 1);
    assert_eq!(intake.received().len(), 1);
}

#[test]
fn with_the_rule_off_a_member_still_files_a_draft_by_hand() {
    let root = tempfile::tempdir().unwrap();
    let intake = recorded_intake();
    let reports = always_sends(root.path(), &intake);
    let forbids = project(FORBIDS);
    let one = held_from(&reports, forbids.path());

    let filed = reports.file_held(&one.id, &Caller::window(), &one.digest(), How::Check);
    let Filing::Sent { to, .. } = filed else {
        panic!("not filed: {filed:?}");
    };
    assert_eq!(to, SentTo::Intake);
    let sent = SentList::read(root.path());
    assert_eq!(sent.len(), 1);
    assert!(!sent[0].unread_draft, "read and filed by the person");
}
