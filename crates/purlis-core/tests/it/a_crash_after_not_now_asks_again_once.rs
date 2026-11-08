// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A crash after Not now asks again, once** (#5700, spec #5683, story 8).
//!
//! Someone who answered the first-run question with Not now left Crash reports unanswered. The
//! next crash purlis catches is refused, as every crash without a yes is, and the answers this
//! machine keeps then say the crash Notice is due, so the next window asks once. Once that
//! Notice was dismissed or answered it is settled, and no later crash raises it again. Each case
//! drives the Reports module through its interface (a panic from the panic hook in, an outcome
//! out) and reads what this machine keeps, as the window does.

use purlis_core::forge::Caller;
use purlis_core::report::{
    Consent, CrashNotice, Evidence, Outcome, Panic, Refusal, Reports, Stream,
};

fn a_panic() -> Evidence {
    Evidence::Panic {
        record: Panic {
            at: None,
            place: "crates/purlis-core/src/plane.rs:12:5".to_string(),
            message: "called `Option::unwrap()` on a `None` value".to_string(),
            version: Some("0.41.2".to_string()),
        },
        words: None,
    }
}

fn not_now() -> Consent {
    Consent {
        asked: true,
        ..Consent::default()
    }
}

/// Keeps `consent` on a fresh machine, lets the panic hook hand Reports a panic, and says what
/// came of it and what the machine keeps after.
fn a_crash_with(consent: Consent) -> (Outcome, Consent) {
    let home = tempfile::tempdir().unwrap();
    consent.write(home.path()).expect("the answers are kept");
    let said = Reports::default()
        .with_answers_in(home.path())
        .take(a_panic(), &Caller::trigger());
    (said, Consent::read(home.path()))
}

#[test]
fn a_crash_after_not_now_is_refused_and_makes_the_crash_notice_due() {
    let (said, kept) = a_crash_with(not_now());

    assert!(
        matches!(
            said,
            Outcome::Refused(Refusal::NoConsent(Stream::CrashReports))
        ),
        "{said:?}"
    );
    assert_eq!(kept.crash_notice, Some(CrashNotice::Due));
}

#[test]
fn the_crash_notice_due_keeps_every_answer_the_person_gave() {
    let (_, kept) = a_crash_with(Consent {
        agent_drafts: Some(true),
        ..not_now()
    });

    assert_eq!(
        kept,
        Consent {
            agent_drafts: Some(true),
            crash_notice: Some(CrashNotice::Due),
            ..not_now()
        }
    );
}

#[test]
fn a_crash_after_the_crash_notice_was_settled_never_raises_it_again() {
    let settled = Consent {
        crash_notice: Some(CrashNotice::Settled),
        ..not_now()
    };

    let (_, kept) = a_crash_with(settled);

    assert_eq!(kept, settled);
}

#[test]
fn a_crash_after_dont_send_raises_no_crash_notice() {
    let dont_send = Consent {
        crash_reports: Some(false),
        ..not_now()
    };

    let (_, kept) = a_crash_with(dont_send);

    assert_eq!(kept, dont_send);
}

#[test]
fn a_crash_before_the_first_run_question_raises_no_crash_notice() {
    let (_, kept) = a_crash_with(Consent::default());

    assert_eq!(kept.crash_notice, None);
}
