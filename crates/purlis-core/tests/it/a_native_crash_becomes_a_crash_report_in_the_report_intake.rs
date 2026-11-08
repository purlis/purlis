// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A native crash becomes a Crash report in the Report intake, sent on the next launch, and
//! only there** (#5691, spec #5683, ruling O9).
//!
//! The crash handler (`purlis-crash-handler`) keeps a crash's minidump in the crashes folder. On
//! the next launch the app hands each one waiting there to Reports through
//! [`native::report_waiting`]. Each case drives that interface with a minidump in a folder and
//! reads the bytes a recording of Sentry's envelope endpoint was sent, as the panic's cases do.

use std::path::Path;
use std::sync::Arc;

use purlis_core::forge::Caller;
use purlis_core::report::intake::{Dsn, Recorded, Sentry};
use purlis_core::report::{
    Consent, Evidence, Known, NativeCrash, Outcome, Refusal, Reports, SentList, Stream, native,
};

const DSN: &str = concat!("https://f00dfeed1234", "@", "o1.ingest.de.sentry.io/4508");

/// What a minidump begins with, and some bytes that are not text.
const MINIDUMP: &[u8] = b"MDMP\x93\xa7\x00\x00\xff\xfe binary";

fn recorded(answering: u16) -> Arc<Recorded> {
    let text = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/intake_contract/sentry/envelope.json"),
    )
    .expect("the recording")
    .replace("\"status\": 200", &format!("\"status\": {answering}"));
    Arc::new(Recorded::parse(&text).expect("a recording"))
}

fn reports(recorded: &Arc<Recorded>, root: &Path, crash_reports: Option<bool>) -> Reports {
    Reports::new(Known::default())
        .with_consent(Consent {
            crash_reports,
            agent_drafts: None,
            asked: true,
        })
        .sending_to(
            Arc::new(Sentry::new(Dsn::parse(DSN).unwrap(), recorded.clone())),
            root,
        )
}

fn a_crash() -> Evidence {
    Evidence::NativeCrash(NativeCrash {
        at: Some(1_762_570_000),
        minidump: MINIDUMP.to_vec(),
    })
}

/// The envelope's lines, up to the minidump: its header, the event's header, the event and the
/// attachment's header, as JSON, and the bytes after them.
fn split(envelope: &[u8]) -> (Vec<serde_json::Value>, Vec<u8>) {
    let mut rest = envelope;
    let mut lines = Vec::new();
    for _ in 0..4 {
        let end = rest.iter().position(|b| *b == b'\n').expect("a line");
        lines.push(serde_json::from_slice(&rest[..end]).expect("a JSON line"));
        rest = &rest[end + 1..];
    }
    (lines, rest.to_vec())
}

#[test]
fn a_native_crash_with_a_yes_is_sent_with_only_the_fixed_fields_and_its_minidump() {
    let home = tempfile::tempdir().unwrap();
    let recorded = recorded(200);

    let said = reports(&recorded, home.path(), Some(true)).take(a_crash(), &Caller::trigger());

    assert!(matches!(said, Outcome::Sent { .. }), "{said:?}");
    let sent = recorded.received();
    assert_eq!(sent.len(), 1, "sent once");
    let (lines, after) = split(&sent[0]);
    let [header, event_item, event, attachment] = &lines[..] else {
        unreachable!()
    };
    assert_eq!(header["dsn"], DSN);
    assert_eq!(event_item["type"], "event");
    let mut fields: Vec<&str> = event
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    fields.sort_unstable();
    assert_eq!(
        fields,
        [
            "contexts",
            "environment",
            "event_id",
            "level",
            "platform",
            "release",
            "tags",
            "timestamp"
        ],
        "only the fixed fields: {event}"
    );
    assert_eq!(event["level"], "fatal");
    assert_eq!(event["timestamp"], "2025-11-08T02:46:40Z");
    assert_eq!(event["contexts"]["os"]["name"], std::env::consts::OS);
    assert_eq!(attachment["type"], "attachment");
    assert_eq!(attachment["attachment_type"], "event.minidump");
    assert_eq!(attachment["length"], MINIDUMP.len());
    assert_eq!(
        after,
        [MINIDUMP, b"\n"].concat(),
        "the minidump, as it was kept"
    );
}

#[test]
fn without_a_yes_to_crash_reports_a_native_crash_is_not_sent() {
    for unanswered in [None, Some(false)] {
        let home = tempfile::tempdir().unwrap();
        let recorded = recorded(200);

        let said = reports(&recorded, home.path(), unanswered).take(a_crash(), &Caller::trigger());

        assert_eq!(
            said,
            Outcome::Refused(Refusal::NoConsent(Stream::CrashReports))
        );
        assert!(recorded.received().is_empty());
        assert!(SentList::read(home.path()).is_empty());
    }
}

#[test]
fn a_native_crash_is_never_filed_on_a_tracker_even_by_the_person_at_the_window() {
    let home = tempfile::tempdir().unwrap();
    let recorded = recorded(200);
    let reports = reports(&recorded, home.path(), Some(true));

    let said = reports.file(a_crash(), &Caller::window(), "any digest");

    assert!(
        matches!(said, Outcome::Refused(Refusal::Unusable(_))),
        "{said:?}"
    );
    assert!(recorded.received().is_empty());
}

#[test]
fn the_sent_list_shows_the_envelope_with_the_minidump_named_by_its_size_and_digest() {
    let home = tempfile::tempdir().unwrap();
    let recorded = recorded(200);
    reports(&recorded, home.path(), Some(true)).take(a_crash(), &Caller::trigger());

    let listed = SentList::read(home.path());

    assert_eq!(listed.len(), 1, "{listed:?}");
    assert_eq!(listed[0].title, "purlis crashed");
    let sent = &recorded.received()[0];
    let text_part = &sent[..sent.len() - MINIDUMP.len() - 1];
    assert!(
        listed[0].bytes.as_bytes().starts_with(text_part),
        "every line before the minidump, exactly: {}",
        listed[0].bytes
    );
    assert!(
        listed[0].bytes.ends_with(&format!(
            "[a minidump of {} bytes, sha256 {}]\n",
            MINIDUMP.len(),
            "e2b034809d40e63bf6defd2519042344e3f3becc376dbc6b1236ca7f3a851d00"
        )),
        "{}",
        listed[0].bytes
    );
}

/// Leaves a minidump in `crashes`, as the crash handler names one: `<seconds since 1970>-<id>.dmp`.
fn left_by_the_handler(crashes: &Path, at: u64) -> std::path::PathBuf {
    std::fs::create_dir_all(crashes).unwrap();
    let path = crashes.join(format!("{at}-0f3c9a.dmp"));
    std::fs::write(&path, MINIDUMP).unwrap();
    path
}

fn now() -> u64 {
    chrono::Utc::now().timestamp().unsigned_abs()
}

#[test]
fn a_minidump_waiting_on_the_next_launch_is_sent_and_then_removed() {
    let home = tempfile::tempdir().unwrap();
    let crashes = native::crashes_dir(home.path());
    let at = now() - 60;
    let left = left_by_the_handler(&crashes, at);
    let recorded = recorded(200);

    let said = native::report_waiting(&reports(&recorded, home.path(), Some(true)), &crashes);

    assert!(matches!(said[..], [Outcome::Sent { .. }]), "{said:?}");
    let (lines, after) = split(&recorded.received()[0]);
    let when = chrono::DateTime::from_timestamp(i64::try_from(at).unwrap(), 0).unwrap();
    assert_eq!(
        lines[2]["timestamp"],
        when.format("%Y-%m-%dT%H:%M:%SZ").to_string()
    );
    assert_eq!(after, [MINIDUMP, b"\n"].concat());
    assert!(!left.exists(), "a minidump sent is not kept");
}

#[test]
fn a_minidump_the_intake_did_not_take_waits_for_the_next_launch() {
    let home = tempfile::tempdir().unwrap();
    let crashes = native::crashes_dir(home.path());
    let left = left_by_the_handler(&crashes, now() - 60);
    let recorded = recorded(503);

    let said = native::report_waiting(&reports(&recorded, home.path(), Some(true)), &crashes);

    assert!(
        matches!(said[..], [Outcome::Refused(Refusal::NotSent(_))]),
        "{said:?}"
    );
    assert!(left.exists(), "kept, to be sent on a later launch");
}

#[test]
fn a_minidump_older_than_fourteen_days_is_dropped_unsent() {
    let home = tempfile::tempdir().unwrap();
    let crashes = native::crashes_dir(home.path());
    let left = left_by_the_handler(&crashes, now() - 15 * 24 * 60 * 60);
    let recorded = recorded(200);

    let said = native::report_waiting(&reports(&recorded, home.path(), Some(true)), &crashes);

    assert_eq!(said, []);
    assert!(recorded.received().is_empty());
    assert!(!left.exists());
}

#[test]
fn without_a_yes_a_waiting_minidump_is_removed_unsent() {
    let home = tempfile::tempdir().unwrap();
    let crashes = native::crashes_dir(home.path());
    let left = left_by_the_handler(&crashes, now() - 60);
    let recorded = recorded(200);

    let said = native::report_waiting(&reports(&recorded, home.path(), Some(false)), &crashes);

    assert_eq!(
        said,
        [Outcome::Refused(Refusal::NoConsent(Stream::CrashReports))]
    );
    assert!(recorded.received().is_empty());
    assert!(
        !left.exists(),
        "a minidump that may not go is not kept either"
    );
}

#[test]
fn a_minidump_still_being_written_is_left_alone() {
    let home = tempfile::tempdir().unwrap();
    let crashes = native::crashes_dir(home.path());
    std::fs::create_dir_all(crashes.join("writing")).unwrap();
    std::fs::write(crashes.join("writing/a.dmp"), MINIDUMP).unwrap();
    std::fs::write(crashes.join(format!("{}-0f3c9a.part", now())), MINIDUMP).unwrap();
    let recorded = recorded(200);

    let said = native::report_waiting(&reports(&recorded, home.path(), Some(true)), &crashes);

    assert_eq!(said, []);
    assert!(recorded.received().is_empty());
}

#[test]
fn no_crashes_folder_is_no_crash() {
    let home = tempfile::tempdir().unwrap();
    let recorded = recorded(200);

    let said = native::report_waiting(
        &reports(&recorded, home.path(), Some(true)),
        &native::crashes_dir(home.path()),
    );

    assert_eq!(said, []);
}
