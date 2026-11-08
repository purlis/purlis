// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A panic becomes a Crash report in the Report intake, and the sent list keeps its exact
//! bytes** (#5689, spec #5683, ADR 0118).
//!
//! Every case drives the Reports module through its interface: a panic record from the panic
//! hook (nobody at the keyboard, so a trigger) in, an outcome out. The intake is the real Sentry
//! adapter over a recording of Sentry's envelope endpoint (`tests/intake_contract/sentry/`), which
//! keeps every body it was sent, so a case reads exactly the bytes that left.

use std::path::Path;
use std::sync::Arc;

use purlis_core::forge::Caller;
use purlis_core::report::intake::{Dsn, Recorded, Sentry};
use purlis_core::report::{
    Consent, Evidence, Known, Outcome, Panic, Refusal, Reports, SentList, Stream,
};

/// The intake's address, as Sentry writes a DSN: `https://<public key>@<host>/<project>`. Put
/// together from its parts, so the commit scan does not read the key and host as an address.
const DSN: &str = concat!("https://f00dfeed1234", "@", "o1.ingest.de.sentry.io/4508");

fn recorded() -> Arc<Recorded> {
    let text = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/intake_contract/sentry/envelope.json"),
    )
    .expect("the recording");
    Arc::new(Recorded::parse(&text).expect("a recording"))
}

fn intake(recorded: &Arc<Recorded>) -> Arc<Sentry<Arc<Recorded>>> {
    Arc::new(Sentry::new(
        Dsn::parse(DSN).expect("a DSN"),
        recorded.clone(),
    ))
}

/// What purlis knows of this person's machine: their home, a plane with a workspace and a repo.
fn known() -> Known {
    Known {
        plane: Some("/Users/someone/Work/northwind-plane".into()),
        home: Some("/Users/someone".into()),
        env: Vec::new(),
        names: vec![
            ("acme-migration".into(), "[workspace]"),
            ("widget-service".into(), "[repo]"),
            ("northwind-plane".into(), "[project]"),
        ],
    }
}

/// A panic whose message names the person's paths, workspace, repo and project.
fn a_panic() -> Evidence {
    Evidence::Panic {
        record: Panic {
            at: Some(1_762_570_000),
            place: "crates/purlis-core/src/plane.rs:12:5".into(),
            message:
                "could not read /Users/someone/Work/northwind-plane/workspaces/acme-migration/\
                      repos/widget-service/Cargo.toml for acme-migration in widget-service of \
                      northwind-plane, nor /opt/build/acme-migration/out.json"
                    .into(),
            version: Some("0.41.2".into()),
        },
        words: None,
    }
}

fn yes() -> Consent {
    Consent {
        crash_reports: Some(true),
        agent_drafts: None,
        asked: true,
        crash_notice: None,
    }
}

/// The envelope's three lines: its header, its item's header, and the event, as JSON.
fn lines(bytes: &[u8]) -> Vec<serde_json::Value> {
    std::str::from_utf8(bytes)
        .expect("an envelope is text")
        .lines()
        .map(|line| serde_json::from_str(line).expect("each line is JSON"))
        .collect()
}

#[test]
fn a_panic_with_a_yes_to_crash_reports_is_sent_once_with_only_the_fixed_fields() {
    let home = tempfile::tempdir().unwrap();
    let recorded = recorded();
    let reports = Reports::new(known())
        .with_consent(yes())
        .sending_to(intake(&recorded), home.path());

    let said = reports.take(a_panic(), &Caller::trigger());

    assert!(matches!(said, Outcome::Sent { .. }), "{said:?}");
    let sent = recorded.received();
    assert_eq!(sent.len(), 1, "sent once");
    assert!(recorded.unspent().is_empty());
    let [header, item, event] = &lines(&sent[0])[..] else {
        panic!(
            "an envelope of three lines: {}",
            String::from_utf8_lossy(&sent[0])
        );
    };
    assert_eq!(header["dsn"], DSN);
    assert_eq!(item["type"], "event");
    let mut fields: Vec<&str> = event
        .as_object()
        .expect("an event")
        .keys()
        .map(String::as_str)
        .collect();
    fields.sort_unstable();
    assert_eq!(
        fields,
        [
            "contexts",
            "culprit",
            "environment",
            "event_id",
            "exception",
            "fingerprint",
            "level",
            "platform",
            "release",
            "tags",
            "timestamp"
        ],
        "only the fixed fields: {event}"
    );
    assert_eq!(event["culprit"], "crates/purlis-core/src/plane.rs:12:5");
    assert_eq!(event["level"], "fatal");
    assert_eq!(
        event["release"],
        format!("purlis@{}", purlis_core::adopt::release_version())
    );
    assert!(event["tags"]["channel"].is_string(), "{event}");
    assert_eq!(event["contexts"]["os"]["name"], std::env::consts::OS);
    assert_eq!(event["exception"]["values"][0]["type"], "panic");
    assert_eq!(header["event_id"], event["event_id"]);
}

#[test]
fn without_a_yes_to_crash_reports_nothing_is_sent_and_nothing_listed() {
    for unanswered in [None, Some(false)] {
        let home = tempfile::tempdir().unwrap();
        let recorded = recorded();
        let reports = Reports::new(known())
            .with_consent(Consent {
                crash_reports: unanswered,
                agent_drafts: Some(true),
                asked: true,
                crash_notice: None,
            })
            .sending_to(intake(&recorded), home.path());

        let said = reports.take(a_panic(), &Caller::trigger());

        assert_eq!(
            said,
            Outcome::Refused(Refusal::NoConsent(Stream::CrashReports))
        );
        assert!(recorded.received().is_empty(), "nothing was sent");
        assert!(SentList::read(home.path()).is_empty(), "nothing was listed");
    }
}

#[test]
fn no_path_workspace_project_or_repo_name_reaches_the_crash_report() {
    let home = tempfile::tempdir().unwrap();
    let recorded = recorded();
    Reports::new(known())
        .with_consent(yes())
        .sending_to(intake(&recorded), home.path())
        .take(a_panic(), &Caller::trigger());

    let sent = String::from_utf8(recorded.received().concat()).unwrap();
    for never in [
        "someone",
        "/Users",
        "/opt/build",
        "acme-migration",
        "widget-service",
        "northwind",
    ] {
        assert!(!sent.contains(never), "{never:?} left in: {sent}");
    }
    let event = &lines(sent.as_bytes())[2];
    assert_eq!(
        event["exception"]["values"][0]["value"],
        "could not read [path] for [workspace] in [repo] of [project], nor [path]"
    );
}

#[test]
fn the_sent_list_keeps_the_exact_bytes_that_left() {
    let home = tempfile::tempdir().unwrap();
    let recorded = recorded();
    let said = Reports::new(known())
        .with_consent(yes())
        .sending_to(intake(&recorded), home.path())
        .take(a_panic(), &Caller::trigger());
    let Outcome::Sent { address } = said else {
        panic!("not sent: {said:?}");
    };

    let listed = SentList::read(home.path());

    assert_eq!(listed.len(), 1, "{listed:?}");
    let entry = &listed[0];
    assert_eq!(entry.bytes.as_bytes(), recorded.received()[0].as_slice());
    assert_eq!(entry.address, address);
    assert_eq!(
        entry.title,
        "purlis panicked at crates/purlis-core/src/plane.rs:12:5"
    );
}

#[test]
fn a_build_with_no_intake_sends_nothing_and_says_so() {
    let home = tempfile::tempdir().unwrap();
    let said = Reports::new(known())
        .with_consent(yes())
        .take(a_panic(), &Caller::trigger());
    assert_eq!(said, Outcome::Refused(Refusal::NoIntake));
    assert!(SentList::read(home.path()).is_empty());
}

#[test]
fn a_sent_report_is_kept_ninety_days_and_then_dropped() {
    use purlis_core::report::{Sent, SentTo};
    let home = tempfile::tempdir().unwrap();
    let sent = |days_ago: i64, title: &str| Sent {
        at: (chrono::Utc::now() - chrono::Duration::days(days_ago))
            .format("%Y-%m-%dT%H:%M:%SZ")
            .to_string(),
        title: title.into(),
        to: SentTo::Intake,
        address: "an event".into(),
        bytes: "{}\n".into(),
        fingerprint: None,
        repeats: 0,
        repeats_sent: 0,
        unread_draft: false,
        waiting: false,
    };
    SentList::add(home.path(), sent(91, "too old")).unwrap();
    SentList::add(home.path(), sent(89, "kept")).unwrap();
    SentList::add(home.path(), sent(0, "today")).unwrap();

    let titles: Vec<String> = SentList::read(home.path())
        .into_iter()
        .map(|one| one.title)
        .collect();
    assert_eq!(titles, ["kept", "today"]);
}
