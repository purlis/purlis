// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A native crash becomes a Crash report of its crashing thread's stack in the Report intake,
//! sent on the next launch, and only there** (#5691, spec #5683, ruling O9 and the operator's
//! ruling (b) of 2026-11-10).
//!
//! The crash handler (`purlis-crash-handler`) keeps a crash's minidump in the crashes folder. On
//! the next launch the app hands each one waiting there to Reports through
//! [`native::report_waiting`], which takes the crashing thread's stack out of it on the device.
//! Each case drives that interface, or Reports with a stack, and reads the bytes a recording of
//! Sentry's envelope endpoint was sent, as the panic's cases do. That a real minidump gives its
//! stack and nothing else is `purlis-crash-handler`'s test, where real crashes are made.

use std::path::Path;
use std::sync::Arc;

use purlis_core::forge::Caller;
use purlis_core::report::intake::{Answer, Dsn, Post, Recorded, Request, Sentry};
use purlis_core::report::native::{CrashStack, Frame, Module};
use purlis_core::report::{
    Consent, Evidence, Known, NativeCrash, Outcome, Refusal, Reports, SentList, Stream, native,
};

const DSN: &str = concat!("https://f00dfeed1234", "@", "o1.ingest.de.sentry.io/4508");

/// What a minidump begins with, and some bytes that are not one.
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
            ..Consent::default()
        })
        .sending_to(
            Arc::new(Sentry::new(Dsn::parse(DSN).unwrap(), recorded.clone())),
            root,
        )
}

/// A stack as [`native::stack_of`] takes one out: a fault in AppKit called from purlis.
fn a_stack() -> CrashStack {
    CrashStack {
        reason: Some("EXC_BAD_ACCESS / KERN_INVALID_ADDRESS".into()),
        arch: Some("arm64".into()),
        frames: vec![
            Frame {
                module: 0,
                offset: 0x1a2b,
                trust: "context",
            },
            Frame {
                module: 1,
                offset: 0x40_0000,
                trust: "fp",
            },
            Frame {
                module: 1,
                offset: 0x40_1234,
                trust: "fp",
            },
        ],
        modules: vec![
            Module {
                name: "AppKit".into(),
                debug_id: Some("6d5d4a8e-2c3b-3f1a-9e7d-0a1b2c3d4e5f".into()),
                code_id: Some("6d5d4a8e2c3b3f1a9e7d0a1b2c3d4e5f".into()),
                size: 0x100_0000,
            },
            Module {
                name: "purlis".into(),
                debug_id: Some("0f1e2d3c-4b5a-6978-8796-a5b4c3d2e1f0".into()),
                code_id: None,
                size: 0x200_0000,
            },
        ],
    }
}

fn a_crash() -> Evidence {
    Evidence::NativeCrash(NativeCrash {
        at: Some(1_762_570_000),
        stack: a_stack(),
    })
}

/// The envelope's lines, as JSON.
fn lines(envelope: &[u8]) -> Vec<serde_json::Value> {
    std::str::from_utf8(envelope)
        .expect("an envelope of text: no minidump in it")
        .lines()
        .map(|line| serde_json::from_str(line).expect("a JSON line"))
        .collect()
}

#[test]
fn a_native_crash_with_a_yes_is_sent_as_its_crashing_threads_stack_and_nothing_else() {
    purlis_core::unsteered!();
    let home = tempfile::tempdir().unwrap();
    let recorded = recorded(200);

    let said = reports(&recorded, home.path(), Some(true)).take(a_crash(), &Caller::trigger());

    assert!(matches!(said, Outcome::Sent { .. }), "{said:?}");
    let sent = recorded.received();
    assert_eq!(sent.len(), 1, "sent once");
    let [header, event_item, event] = &lines(&sent[0])[..] else {
        panic!("a header and one event, and no attachment")
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
            "culprit",
            "debug_meta",
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
        "the fixed fields and the stack: {event}"
    );
    assert_eq!(event["level"], "fatal");
    assert_eq!(event["timestamp"], "2025-11-08T02:46:40Z");
    let exception = &event["exception"]["values"][0];
    assert_eq!(exception["type"], "EXC_BAD_ACCESS");
    assert_eq!(exception["value"], "EXC_BAD_ACCESS / KERN_INVALID_ADDRESS");
    assert_eq!(
        exception["stacktrace"]["frames"],
        serde_json::json!([
            {"instruction_addr": "0x401234", "addr_mode": "rel:1", "package": "purlis", "trust": "fp"},
            {"instruction_addr": "0x400000", "addr_mode": "rel:1", "package": "purlis", "trust": "fp"},
            {"instruction_addr": "0x1a2b", "addr_mode": "rel:0", "package": "AppKit", "trust": "context"},
        ]),
        "the outermost first, each an offset into its module"
    );
    let images = event["debug_meta"]["images"].as_array().unwrap();
    assert_eq!(images.len(), 2);
    assert_eq!(images[1]["code_file"], "purlis");
    assert_eq!(
        images[1]["debug_id"],
        "0f1e2d3c-4b5a-6978-8796-a5b4c3d2e1f0"
    );
    assert_eq!(images[1]["image_addr"], "0x0", "no address of the process");
    assert_eq!(event["culprit"], "AppKit+0x1a2b");
}

#[test]
fn a_modules_folder_never_leaves() {
    purlis_core::unsteered!();
    let home = tempfile::tempdir().unwrap();
    let recorded = recorded(200);
    let mut stack = a_stack();
    stack.modules[1].name = "/Users/someone/Projects/acme/target/release/purlis".into();

    reports(&recorded, home.path(), Some(true)).take(
        Evidence::NativeCrash(NativeCrash { at: None, stack }),
        &Caller::trigger(),
    );

    let sent = String::from_utf8(recorded.received()[0].clone()).unwrap();
    assert!(!sent.contains("someone"), "{sent}");
    assert!(!sent.contains("acme"), "{sent}");
}

#[test]
fn a_crash_reasons_address_code_or_number_never_leaves() {
    purlis_core::unsteered!();
    let home = tempfile::tempdir().unwrap();
    let recorded = recorded(200);
    let mut stack = a_stack();
    stack.reason = Some(
        "EXC_GUARD / GUARD_TYPE_VIRT_MEMORY / flavor: 0x0002 / offset: 0x00007ff7bfeff000".into(),
    );

    reports(&recorded, home.path(), Some(true)).take(
        Evidence::NativeCrash(NativeCrash { at: None, stack }),
        &Caller::trigger(),
    );

    let event = &lines(&recorded.received()[0])[2];
    assert_eq!(
        event["exception"]["values"][0]["value"],
        "EXC_GUARD / GUARD_TYPE_VIRT_MEMORY"
    );
    let sent = String::from_utf8(recorded.received()[0].clone()).unwrap();
    assert!(!sent.contains("7ff7bfeff000"), "{sent}");
}

#[test]
fn the_same_native_crash_again_in_the_same_version_is_counted_not_sent() {
    purlis_core::unsteered!();
    let home = tempfile::tempdir().unwrap();
    let recorded = recorded(200);
    let reports = reports(&recorded, home.path(), Some(true));

    reports.take(a_crash(), &Caller::trigger());
    let again = reports.take(a_crash(), &Caller::trigger());

    assert_eq!(again, Outcome::Counted { repeats: 1 });
    assert_eq!(recorded.received().len(), 1);
}

#[test]
fn a_native_crash_whose_stack_could_not_be_read_is_sent_with_no_frames_and_never_counted() {
    purlis_core::unsteered!();
    let home = tempfile::tempdir().unwrap();
    let recorded = recorded(200);
    let reports = reports(&recorded, home.path(), Some(true));
    let unread = || {
        Evidence::NativeCrash(NativeCrash {
            at: None,
            stack: CrashStack::default(),
        })
    };

    let first = reports.take(unread(), &Caller::trigger());
    let again = reports.take(unread(), &Caller::trigger());

    assert!(matches!(first, Outcome::Sent { .. }), "{first:?}");
    assert!(
        !matches!(again, Outcome::Counted { .. }),
        "nothing tells it from the first: {again:?}"
    );
    let event = &lines(&recorded.received()[0])[2];
    assert_eq!(event["exception"]["values"][0]["type"], "native crash");
    assert_eq!(
        event["exception"]["values"][0]["stacktrace"]["frames"],
        serde_json::json!([])
    );
    assert!(
        SentList::read(home.path())
            .iter()
            .all(|sent| sent.is_a_crash()),
        "still a crash to the sent list, so a later no to Crash reports holds it"
    );
}

#[test]
fn without_a_yes_to_crash_reports_a_native_crash_is_not_sent() {
    purlis_core::unsteered!();
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
    purlis_core::unsteered!();
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
fn the_sent_list_shows_exactly_the_envelope_that_was_sent() {
    purlis_core::unsteered!();
    let home = tempfile::tempdir().unwrap();
    let recorded = recorded(200);
    reports(&recorded, home.path(), Some(true)).take(a_crash(), &Caller::trigger());

    let listed = SentList::read(home.path());

    assert_eq!(listed.len(), 1, "{listed:?}");
    assert_eq!(listed[0].title, "purlis crashed at AppKit+0x1a2b");
    assert_eq!(listed[0].bytes.as_bytes(), &recorded.received()[0][..]);
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
fn a_minidump_waiting_on_the_next_launch_is_reported_once_and_kept_on_this_device() {
    purlis_core::unsteered!();
    let home = tempfile::tempdir().unwrap();
    let crashes = native::crashes_dir(home.path());
    let at = now() - 60;
    let left = left_by_the_handler(&crashes, at);
    let recorded = recorded(200);
    let reports = reports(&recorded, home.path(), Some(true));

    let said = native::report_waiting(&reports, &crashes);
    let next_launch = native::report_waiting(&reports, &crashes);

    assert!(matches!(said[..], [Outcome::Sent { .. }]), "{said:?}");
    assert_eq!(next_launch, [], "reported once");
    let event = &lines(&recorded.received()[0])[2];
    let when = chrono::DateTime::from_timestamp(i64::try_from(at).unwrap(), 0).unwrap();
    assert_eq!(
        event["timestamp"],
        when.format("%Y-%m-%dT%H:%M:%SZ").to_string()
    );
    assert!(!left.exists());
    assert_eq!(
        std::fs::read(left.with_extension("kept")).unwrap(),
        MINIDUMP,
        "the minidump stays on this device"
    );
}

/// An intake that never answers, as an offline machine gets.
struct Offline;

impl Post for Offline {
    fn post(&self, _: &Request) -> Result<Answer, String> {
        Err("no route to the intake".into())
    }
}

#[test]
fn a_native_crash_the_intake_gave_no_answer_for_waits_as_its_report_and_not_its_minidump() {
    purlis_core::unsteered!();
    let home = tempfile::tempdir().unwrap();
    let crashes = native::crashes_dir(home.path());
    let left = left_by_the_handler(&crashes, now() - 60);
    let offline = Reports::new(Known::default())
        .with_consent(Consent {
            crash_reports: Some(true),
            agent_drafts: None,
            asked: true,
            ..Consent::default()
        })
        .sending_to(
            Arc::new(Sentry::new(Dsn::parse(DSN).unwrap(), Offline)),
            home.path(),
        );

    let said = native::report_waiting(&offline, &crashes);

    assert_eq!(said, [Outcome::Queued]);
    let listed = SentList::read(home.path());
    assert!(
        matches!(&listed[..], [waiting] if waiting.waiting),
        "its report waits to send: {listed:?}"
    );
    assert!(!left.exists(), "its minidump is not reported again");
}

#[test]
fn a_minidump_the_intake_refused_is_not_sent_again() {
    purlis_core::unsteered!();
    let home = tempfile::tempdir().unwrap();
    let crashes = native::crashes_dir(home.path());
    left_by_the_handler(&crashes, now() - 60);
    let recorded = recorded(413);
    let reports = reports(&recorded, home.path(), Some(true));

    let said = native::report_waiting(&reports, &crashes);
    native::report_waiting(&reports, &crashes);

    assert!(
        matches!(said[..], [Outcome::Refused(Refusal::NotSent(_))]),
        "{said:?}"
    );
    assert_eq!(recorded.received().len(), 1);
}

#[test]
fn a_minidump_older_than_fourteen_days_is_removed_reported_or_not() {
    purlis_core::unsteered!();
    let home = tempfile::tempdir().unwrap();
    let crashes = native::crashes_dir(home.path());
    let old = now() - 15 * 24 * 60 * 60;
    let waiting = left_by_the_handler(&crashes, old);
    let kept = crashes.join(format!("{old}-1a2b3c.kept"));
    std::fs::write(&kept, MINIDUMP).unwrap();
    let recent = crashes.join(format!("{}-4d5e6f.kept", now() - 60));
    std::fs::write(&recent, MINIDUMP).unwrap();
    let recorded = recorded(200);

    let said = native::report_waiting(&reports(&recorded, home.path(), Some(true)), &crashes);

    assert_eq!(said, []);
    assert!(recorded.received().is_empty());
    assert!(!waiting.exists());
    assert!(!kept.exists());
    assert!(recent.exists(), "a recent one stays");
}

#[test]
fn without_a_yes_a_waiting_minidump_is_not_sent_and_stays_on_this_device() {
    purlis_core::unsteered!();
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
    assert!(left.with_extension("kept").exists());
}

#[test]
fn a_minidump_still_being_written_is_left_alone() {
    purlis_core::unsteered!();
    let home = tempfile::tempdir().unwrap();
    let crashes = native::crashes_dir(home.path());
    std::fs::create_dir_all(crashes.join("writing")).unwrap();
    std::fs::write(crashes.join("writing/a.dmp"), MINIDUMP).unwrap();
    let part = crashes.join(format!("{}-0f3c9a.part", now()));
    std::fs::write(&part, MINIDUMP).unwrap();
    let recorded = recorded(200);

    let said = native::report_waiting(&reports(&recorded, home.path(), Some(true)), &crashes);

    assert_eq!(said, []);
    assert!(recorded.received().is_empty());
    assert!(part.exists());
}

#[test]
fn a_minidump_the_handler_never_finished_is_removed_unread() {
    purlis_core::unsteered!();
    let home = tempfile::tempdir().unwrap();
    let crashes = native::crashes_dir(home.path());
    std::fs::create_dir_all(&crashes).unwrap();
    let unfinished = crashes.join(format!("{}-0f3c9a.part", now() - 2 * 60 * 60));
    std::fs::write(&unfinished, MINIDUMP).unwrap();
    let recorded = recorded(200);

    let said = native::report_waiting(&reports(&recorded, home.path(), Some(true)), &crashes);

    assert_eq!(said, []);
    assert!(recorded.received().is_empty());
    assert!(!unfinished.exists(), "what it wrote of the process goes");
}

#[test]
fn something_that_is_not_a_minidump_has_no_stack() {
    purlis_core::unsteered!();
    assert!(native::stack_of(MINIDUMP).is_err());
    assert!(native::stack_of(b"").is_err());
}

#[test]
fn no_crashes_folder_is_no_crash() {
    purlis_core::unsteered!();
    let home = tempfile::tempdir().unwrap();
    let recorded = recorded(200);

    let said = native::report_waiting(
        &reports(&recorded, home.path(), Some(true)),
        &native::crashes_dir(home.path()),
    );

    assert_eq!(said, []);
}
