// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **An error the window did not catch becomes a Crash report, the panic's way** (#5690, spec
//! #5683, ADR 0118).
//!
//! The window hands the core what it saw (an uncaught error or an unhandled rejection) and the
//! core alone says whether it may go, what of it goes and where: the same consent, scrub, Report
//! intake and sent list a panic has. The intake is the real Sentry adapter over the recording of
//! Sentry's envelope endpoint (`tests/intake_contract/sentry/`), which keeps every body it was
//! sent, so a case reads exactly the bytes that left.

use std::path::Path;
use std::sync::Arc;

use purlis_core::forge::Caller;
use purlis_core::report::intake::{Dsn, Recorded, Sentry};
use purlis_core::report::{
    Consent, Evidence, Known, Outcome, Refusal, Reports, SentList, Stream, WindowError,
};

/// Put together from its parts, as the panic's case does, so the commit scan does not read it.
const DSN: &str = concat!("https://f00dfeed1234", "@", "o1.ingest.de.sentry.io/4508");

fn recorded() -> Arc<Recorded> {
    let text = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/intake_contract/sentry/envelope.json"),
    )
    .expect("the recording");
    Arc::new(Recorded::parse(&text).expect("a recording"))
}

fn reports(recorded: &Arc<Recorded>, consent: Consent, home: &Path) -> Reports {
    Reports::new(Known {
        plane: Some("/Users/someone/Work/northwind-plane".into()),
        home: Some("/Users/someone".into()),
        env: Vec::new(),
        names: vec![
            ("acme-migration".into(), "[workspace]"),
            ("widget-service".into(), "[repo]"),
            ("northwind-plane".into(), "[project]"),
        ],
    })
    .with_consent(consent)
    .sending_to(
        Arc::new(Sentry::new(
            Dsn::parse(DSN).expect("a DSN"),
            recorded.clone(),
        )),
        home,
    )
}

fn crash_reports(answer: Option<bool>) -> Consent {
    Consent {
        crash_reports: answer,
        agent_drafts: Some(true),
        asked: true,
    }
}

/// A `TypeError` thrown in the window's bundle, its message naming the person's paths and names.
fn thrown() -> Evidence {
    Evidence::WindowError(WindowError {
        name: "TypeError".into(),
        message: "cannot read workspace of undefined in \
                  /Users/someone/Work/northwind-plane/workspaces/acme-migration/repos/\
                  widget-service for acme-migration"
            .into(),
        source: Some("tauri://localhost/assets/index-3f2a9c.js".into()),
        line: Some(1),
        column: Some(4242),
        rejection: false,
    })
}

/// The envelope's three lines, as JSON.
fn lines(bytes: &[u8]) -> Vec<serde_json::Value> {
    std::str::from_utf8(bytes)
        .expect("an envelope is text")
        .lines()
        .map(|line| serde_json::from_str(line).expect("each line is JSON"))
        .collect()
}

#[test]
fn an_error_thrown_in_the_window_reaches_the_intake_once_as_a_scrubbed_window_error() {
    let home = tempfile::tempdir().unwrap();
    let recorded = recorded();

    let said = reports(&recorded, crash_reports(Some(true)), home.path())
        .take(thrown(), &Caller::window());

    assert!(matches!(said, Outcome::Sent { .. }), "{said:?}");
    let sent = recorded.received();
    assert_eq!(sent.len(), 1, "sent once");
    let [header, _, event] = &lines(&sent[0])[..] else {
        panic!("an envelope of three lines");
    };
    assert_eq!(header["dsn"], DSN);
    assert_eq!(event["platform"], "javascript");
    assert_eq!(event["culprit"], "index-3f2a9c.js:1:4242");
    let exception = &event["exception"]["values"][0];
    assert_eq!(exception["type"], "TypeError");
    assert_eq!(exception["mechanism"]["type"], "onerror");
    assert_eq!(
        exception["value"],
        "cannot read workspace of undefined in [path] for [workspace]"
    );
    let text = String::from_utf8(sent[0].clone()).unwrap();
    for never in [
        "someone",
        "/Users",
        "acme-migration",
        "widget-service",
        "northwind",
    ] {
        assert!(!text.contains(never), "{never:?} left in: {text}");
    }
    let listed = SentList::read(home.path());
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert_eq!(listed[0].bytes.as_bytes(), sent[0].as_slice());
    assert_eq!(
        listed[0].title,
        "purlis's window failed at index-3f2a9c.js:1:4242"
    );
}

#[test]
fn without_a_yes_to_crash_reports_a_window_error_is_neither_sent_nor_listed() {
    for unanswered in [None, Some(false)] {
        let home = tempfile::tempdir().unwrap();
        let recorded = recorded();

        let said = reports(&recorded, crash_reports(unanswered), home.path())
            .take(thrown(), &Caller::window());

        assert_eq!(
            said,
            Outcome::Refused(Refusal::NoConsent(Stream::CrashReports))
        );
        assert!(recorded.received().is_empty(), "nothing was sent");
        assert!(SentList::read(home.path()).is_empty(), "nothing was listed");
    }
}

#[test]
fn a_script_the_window_loaded_from_the_persons_folders_is_named_by_its_file_alone() {
    let home = tempfile::tempdir().unwrap();
    let recorded = recorded();
    let Evidence::WindowError(mut error) = thrown() else {
        unreachable!()
    };
    error.source = Some("file:///Users/someone/Work/acme-migration/extension.js".into());

    reports(&recorded, crash_reports(Some(true)), home.path())
        .take(Evidence::WindowError(error), &Caller::window());

    let event = &lines(&recorded.received()[0])[2];
    assert_eq!(event["culprit"], "extension.js:1:4242");
}

#[test]
fn an_unhandled_rejection_goes_as_one_with_its_scrubbed_reason() {
    let home = tempfile::tempdir().unwrap();
    let recorded = recorded();

    let said = reports(&recorded, crash_reports(Some(true)), home.path()).take(
        Evidence::WindowError(WindowError {
            name: "Error".into(),
            message: "no answer for acme-migration".into(),
            source: None,
            line: None,
            column: None,
            rejection: true,
        }),
        &Caller::window(),
    );

    assert!(matches!(said, Outcome::Sent { .. }), "{said:?}");
    let event = &lines(&recorded.received()[0])[2];
    let exception = &event["exception"]["values"][0];
    assert_eq!(exception["mechanism"]["type"], "onunhandledrejection");
    assert_eq!(exception["value"], "no answer for [workspace]");
    assert_eq!(
        SentList::read(home.path())[0].title,
        "purlis's window failed: Error"
    );
}
