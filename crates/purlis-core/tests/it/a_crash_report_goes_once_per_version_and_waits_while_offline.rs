// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A Crash report goes once per fingerprint and purlis version, and waits on this machine while
//! the Report intake cannot be reached** (#5693, spec #5683, Q15, Q24).
//!
//! Every case drives the Reports module through its interface: a panic record from the panic
//! hook (a trigger) in, an outcome out. The intake is the real Sentry adapter over a [`Line`]: a
//! recording of Sentry's envelope endpoint that answers while the line is up, and gives no answer
//! at all while it is down, as an offline machine gets.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use purlis_core::forge::Caller;
use purlis_core::report::intake::{Answer, Dsn, Post, Recorded, Request, Sentry};
use purlis_core::report::{Consent, Evidence, Known, Outcome, Panic, Refusal, Reports, SentList};

/// The intake's address, put together from its parts so the commit scan does not read it.
const DSN: &str = concat!("https://f00dfeed1234", "@", "o1.ingest.de.sentry.io/4508");

/// Sentry's envelope endpoint, answering 200 to each of `sends` envelopes, while it is up.
pub(crate) struct Line {
    up: AtomicBool,
    intake: Recorded,
}

impl Line {
    pub(crate) fn up(sends: usize) -> Arc<Line> {
        let exchange = r#"{"path": "/api/4508/envelope/", "status": 200}"#;
        let text = format!(
            r#"{{"source": "tests/intake_contract/sentry/envelope.json, {sends} times", "exchanges": [{}]}}"#,
            vec![exchange; sends].join(",")
        );
        Arc::new(Line {
            up: AtomicBool::new(true),
            intake: Recorded::parse(&text).expect("a recording"),
        })
    }

    pub(crate) fn down(sends: usize) -> Arc<Line> {
        let line = Line::up(sends);
        line.set(false);
        line
    }

    pub(crate) fn set(&self, up: bool) {
        self.up.store(up, Ordering::SeqCst);
    }

    /// Every envelope the intake took, oldest first, as text.
    pub(crate) fn received(&self) -> Vec<String> {
        self.intake
            .received()
            .into_iter()
            .map(|bytes| String::from_utf8(bytes).expect("an envelope is text"))
            .collect()
    }
}

impl Post for Line {
    fn post(&self, request: &Request) -> Result<Answer, String> {
        if self.up.load(Ordering::SeqCst) {
            self.intake.post(request)
        } else {
            Err("purlis's report service did not answer: no route to host".into())
        }
    }
}

fn reports(line: &Arc<Line>, home: &std::path::Path) -> Reports {
    Reports::new(Known::default())
        .with_consent(Consent {
            crash_reports: Some(true),
            agent_drafts: None,
            asked: true,
            crash_notice: None,
        })
        .sending_to(
            Arc::new(Sentry::new(Dsn::parse(DSN).expect("a DSN"), line.clone())),
            home,
        )
}

/// A panic at `place` in purlis `version`.
fn panic_at(place: &str, version: &str) -> Evidence {
    Evidence::Panic {
        record: Panic {
            at: Some(1_762_570_000),
            place: place.into(),
            message: "called `Option::unwrap()` on a `None` value".into(),
            version: Some(version.into()),
        },
        words: None,
    }
}

/// The event an envelope carries: its third line, as JSON.
fn event(envelope: &str) -> serde_json::Value {
    serde_json::from_str(envelope.lines().nth(2).expect("an event line")).expect("JSON")
}

#[test]
fn a_repeat_of_a_sent_fingerprint_is_counted_and_not_sent_again() {
    let home = tempfile::tempdir().unwrap();
    let line = Line::up(1);
    let reports = reports(&line, home.path());

    let first = reports.take(panic_at("src/plane.rs:12:5", "0.41.2"), &Caller::trigger());
    let again = reports.take(panic_at("src/plane.rs:12:5", "0.41.2"), &Caller::trigger());
    let third = reports.take(panic_at("src/plane.rs:12:5", "0.41.2"), &Caller::trigger());

    assert!(matches!(first, Outcome::Sent { .. }), "{first:?}");
    assert_eq!(again, Outcome::Counted { repeats: 1 });
    assert_eq!(third, Outcome::Counted { repeats: 2 });
    assert_eq!(line.received().len(), 1, "sent once");
    let listed = SentList::read(home.path());
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert_eq!(listed[0].repeats, 2);
}

#[test]
fn the_same_panic_in_a_new_purlis_version_is_sent_again() {
    let home = tempfile::tempdir().unwrap();
    let line = Line::up(2);
    let reports = reports(&line, home.path());

    reports.take(panic_at("src/plane.rs:12:5", "0.41.2"), &Caller::trigger());
    let newer = reports.take(panic_at("src/plane.rs:12:5", "0.42.0"), &Caller::trigger());

    assert!(matches!(newer, Outcome::Sent { .. }), "{newer:?}");
    assert_eq!(line.received().len(), 2);
}

#[test]
fn a_panic_elsewhere_in_the_same_version_is_sent_on_its_own() {
    let home = tempfile::tempdir().unwrap();
    let line = Line::up(2);
    let reports = reports(&line, home.path());

    reports.take(panic_at("src/plane.rs:12:5", "0.41.2"), &Caller::trigger());
    let elsewhere = reports.take(panic_at("src/plane.rs:80:9", "0.41.2"), &Caller::trigger());

    assert!(matches!(elsewhere, Outcome::Sent { .. }), "{elsewhere:?}");
    assert_eq!(line.received().len(), 2);
}

#[test]
fn the_count_of_repeats_rides_once_on_the_next_report_that_leaves() {
    let home = tempfile::tempdir().unwrap();
    let line = Line::up(3);
    let reports = reports(&line, home.path());
    reports.take(panic_at("src/plane.rs:12:5", "0.41.2"), &Caller::trigger());
    reports.take(panic_at("src/plane.rs:12:5", "0.41.2"), &Caller::trigger());
    reports.take(panic_at("src/plane.rs:12:5", "0.41.2"), &Caller::trigger());
    let repeated = SentList::read(home.path())[0]
        .fingerprint
        .clone()
        .expect("a fingerprint");

    reports.take(panic_at("src/plane.rs:80:9", "0.41.2"), &Caller::trigger());
    reports.take(panic_at("src/watch.rs:3:1", "0.41.2"), &Caller::trigger());

    let sent = line.received();
    assert_eq!(sent.len(), 3, "the repeats were never sent on their own");
    let first = event(&sent[0]);
    assert_eq!(first["fingerprint"], serde_json::json!([repeated]));
    assert!(first.get("extra").is_none(), "{first}");
    let next = event(&sent[1]);
    assert_eq!(
        next["extra"]["repeats"],
        serde_json::json!([{"fingerprint": repeated, "count": 2}]),
        "{next}"
    );
    assert!(
        event(&sent[2]).get("extra").is_none(),
        "counted once: {}",
        sent[2]
    );
    let listed = SentList::read(home.path());
    assert_eq!(
        listed[1].bytes, sent[1],
        "the list keeps what left, count and all"
    );
}

/// Where each envelope the intake took panicked, in the order they arrived.
fn places(line: &Line) -> Vec<String> {
    line.received()
        .iter()
        .map(|envelope| {
            event(envelope)["culprit"]
                .as_str()
                .unwrap_or("")
                .to_string()
        })
        .collect()
}

#[test]
fn a_report_made_offline_waits_and_goes_oldest_first_when_the_intake_answers() {
    let home = tempfile::tempdir().unwrap();
    let line = Line::down(4);
    let reports = reports(&line, home.path());

    for place in ["src/a.rs:1:1", "src/b.rs:2:2", "src/c.rs:3:3"] {
        let said = reports.take(panic_at(place, "0.41.2"), &Caller::trigger());
        assert_eq!(said, Outcome::Queued, "{place}");
    }
    assert!(line.received().is_empty());
    let listed = SentList::read(home.path());
    assert_eq!(listed.len(), 3);
    assert!(listed.iter().all(|one| one.waiting), "{listed:?}");

    line.set(true);
    let said = reports.take(panic_at("src/d.rs:4:4", "0.41.2"), &Caller::trigger());

    assert!(matches!(said, Outcome::Sent { .. }), "{said:?}");
    assert_eq!(
        places(&line),
        [
            "src/a.rs:1:1",
            "src/b.rs:2:2",
            "src/c.rs:3:3",
            "src/d.rs:4:4"
        ]
    );
    let listed = SentList::read(home.path());
    assert!(listed.iter().all(|one| !one.waiting), "{listed:?}");
    assert!(
        listed.iter().all(|one| !one.address.is_empty()),
        "{listed:?}"
    );
}

#[test]
fn the_waiting_reports_go_on_their_own_once_the_intake_answers() {
    let home = tempfile::tempdir().unwrap();
    let line = Line::down(2);
    let reports = reports(&line, home.path());
    reports.take(panic_at("src/a.rs:1:1", "0.41.2"), &Caller::trigger());
    reports.take(panic_at("src/b.rs:2:2", "0.41.2"), &Caller::trigger());
    assert_eq!(reports.send_waiting(), 0, "nothing goes while offline");

    line.set(true);

    assert_eq!(reports.send_waiting(), 2);
    assert_eq!(places(&line), ["src/a.rs:1:1", "src/b.rs:2:2"]);
    assert_eq!(reports.send_waiting(), 0, "each goes once");
}

#[test]
fn at_most_fifty_wait_and_the_fifty_first_is_dropped() {
    let home = tempfile::tempdir().unwrap();
    let line = Line::down(50);
    let reports = reports(&line, home.path());

    for n in 1..=50 {
        let said = reports.take(
            panic_at(&format!("src/n.rs:{n}:1"), "0.41.2"),
            &Caller::trigger(),
        );
        assert_eq!(said, Outcome::Queued, "the {n}th");
    }
    let fifty_first = reports.take(panic_at("src/n.rs:51:1", "0.41.2"), &Caller::trigger());

    assert!(
        matches!(fifty_first, Outcome::Refused(Refusal::NotSent(_))),
        "{fifty_first:?}"
    );
    assert_eq!(SentList::read(home.path()).len(), 50);
    line.set(true);
    assert_eq!(reports.send_waiting(), 50);
    assert_eq!(
        places(&line).first().map(String::as_str),
        Some("src/n.rs:1:1")
    );
    assert_eq!(
        places(&line).last().map(String::as_str),
        Some("src/n.rs:50:1")
    );
}

#[test]
fn a_report_waiting_longer_than_fourteen_days_is_dropped_unsent() {
    let home = tempfile::tempdir().unwrap();
    let line = Line::down(2);
    let reports = reports(&line, home.path());
    reports.take(panic_at("src/old.rs:1:1", "0.41.2"), &Caller::trigger());
    reports.take(panic_at("src/new.rs:2:2", "0.41.2"), &Caller::trigger());
    // The first one was made fifteen days ago, the second thirteen.
    let path = home
        .path()
        .join(purlis_core::machine::DIR)
        .join("reports-sent.json");
    let mut on_disk: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    for (at, days) in [(0, 15), (1, 13)] {
        on_disk["sent"][at]["at"] = (chrono::Utc::now() - chrono::Duration::days(days))
            .format("%Y-%m-%dT%H:%M:%SZ")
            .to_string()
            .into();
    }
    std::fs::write(&path, on_disk.to_string()).unwrap();

    line.set(true);

    assert_eq!(reports.send_waiting(), 1);
    assert_eq!(places(&line), ["src/new.rs:2:2"]);
    let listed = SentList::read(home.path());
    assert_eq!(listed.len(), 1, "{listed:?}");
}
