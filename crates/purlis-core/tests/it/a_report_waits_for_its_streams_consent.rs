// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Nothing is drafted or sent for a stream the person has not said yes to** (#5688, spec
//! #5683, Q1/Q10).
//!
//! The person answers two streams once: **Crash reports** (machine-made, sent with nobody
//! asking) and **Agent drafts** (a report an agent or purlis itself writes for the person to
//! read). Every case drives the Reports module through its interface, evidence from a source
//! in, an outcome out, and reads only the outcome: drafted, or refused for want of a yes.
//!
//! A person's own report, from the window or a command they typed, belongs to no stream: they
//! are there, writing it, so it never waits on either answer.

use purlis_core::forge::Caller;
use purlis_core::report::{
    Consent, Evidence, Kind, Outcome, Panic, PurlisFailure, Refusal, Reports, Stream,
};
use purlis_core::sandboxblock::{Block, Kind as BlockKind, Operation};

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

fn a_sandbox_block() -> Evidence {
    Evidence::PurlisFailure(PurlisFailure::SandboxBlock {
        block: Block {
            operation: Operation::of_word("write").expect("an operation the sandbox reports"),
            kind: BlockKind::of_word("home").expect("a kind the sandbox reports"),
            ours: true,
        },
        ran_as: purlis_core::report::RanAs::default(),
    })
}

fn an_agents_words() -> Evidence {
    Evidence::PersonDraft {
        kind: Kind::Bug,
        text: "purlis worktree update refused a clean branch".to_string(),
        title: None,
    }
}

fn answered(crash_reports: Option<bool>, agent_drafts: Option<bool>) -> Consent {
    Consent {
        crash_reports,
        agent_drafts,
        always_send: None,
        asked: true,
        crash_notice: None,
    }
}

fn outcome(consent: Consent, evidence: Evidence, source: &Caller) -> Outcome {
    Reports::default()
        .with_consent(consent)
        .take(evidence, source)
}

fn refused_for(outcome: &Outcome) -> Option<Stream> {
    match outcome {
        Outcome::Refused(Refusal::NoConsent(stream)) => Some(*stream),
        _ => None,
    }
}

#[test]
fn a_panic_nobody_asked_for_waits_for_a_yes_to_crash_reports() {
    for unanswered in [None, Some(false)] {
        let said = outcome(
            answered(unanswered, Some(true)),
            a_panic(),
            &Caller::trigger(),
        );
        assert_eq!(refused_for(&said), Some(Stream::CrashReports), "{said:?}");
    }
    // With the yes it is a Crash report, sent rather than drafted: here, with no intake to send
    // to, it says so (`a_panic_becomes_a_crash_report_in_the_report_intake`, #5689).
    let said = outcome(answered(Some(true), None), a_panic(), &Caller::trigger());
    assert_eq!(said, Outcome::Refused(Refusal::NoIntake));
}

#[test]
fn a_failure_purlis_caught_itself_waits_for_a_yes_to_agent_drafts() {
    for unanswered in [None, Some(false)] {
        let said = outcome(
            answered(Some(true), unanswered),
            a_sandbox_block(),
            &Caller::trigger(),
        );
        assert_eq!(refused_for(&said), Some(Stream::AgentDrafts), "{said:?}");
    }
    let said = outcome(
        answered(None, Some(true)),
        a_sandbox_block(),
        &Caller::trigger(),
    );
    assert!(matches!(said, Outcome::Drafted { .. }), "{said:?}");
}

#[test]
fn a_draft_a_chat_writes_waits_for_a_yes_to_agent_drafts_whatever_it_holds() {
    for evidence in [an_agents_words(), a_panic(), a_sandbox_block()] {
        for source in [Caller::chat("01CHAT"), Caller::mcp("01CHAT")] {
            let said = outcome(answered(Some(true), Some(false)), evidence.clone(), &source);
            assert_eq!(refused_for(&said), Some(Stream::AgentDrafts), "{said:?}");
            let said = outcome(answered(None, Some(true)), evidence.clone(), &source);
            assert!(matches!(said, Outcome::Drafted { .. }), "{said:?}");
        }
    }
}

#[test]
fn filing_is_refused_unsent_for_a_stream_with_no_yes() {
    let said = Reports::default()
        .with_consent(answered(Some(true), None))
        .file(an_agents_words(), &Caller::chat("01CHAT"), "any digest");
    assert_eq!(refused_for(&said), Some(Stream::AgentDrafts), "{said:?}");
}

#[test]
fn a_persons_own_report_never_waits_on_either_answer() {
    for evidence in [an_agents_words(), a_panic(), a_sandbox_block()] {
        for source in [Caller::window(), Caller::command()] {
            let said = outcome(Consent::default(), evidence.clone(), &source);
            assert!(matches!(said, Outcome::Drafted { .. }), "{said:?}");
        }
    }
}

#[test]
fn an_answer_kept_on_this_machine_is_what_the_next_read_finds() {
    let home = tempfile::tempdir().expect("a temporary config home");
    assert_eq!(Consent::read(home.path()), Consent::default());
    assert!(!Consent::read(home.path()).asked);

    let said = answered(Some(true), Some(false));
    said.write(home.path()).expect("the answer is kept");
    assert_eq!(Consent::read(home.path()), said);

    let not_now = Consent {
        asked: true,
        ..Consent::default()
    };
    not_now.write(home.path()).expect("Not now is kept");
    assert_eq!(Consent::read(home.path()), not_now);
}

#[test]
fn a_kept_answer_purlis_cannot_read_is_no_yes_and_not_yet_asked() {
    let home = tempfile::tempdir().expect("a temporary config home");
    answered(Some(true), Some(true))
        .write(home.path())
        .expect("the answer is kept");
    let file = std::fs::read_dir(home.path())
        .expect("the config home")
        .flatten()
        .flat_map(|dir| {
            std::fs::read_dir(dir.path())
                .into_iter()
                .flatten()
                .flatten()
        })
        .map(|entry| entry.path())
        .find(|path| path.file_name().is_some_and(|name| name == "reports.json"))
        .expect("the answer is kept in reports.json");
    std::fs::write(&file, "{ not json").expect("the file is spoiled");
    assert_eq!(Consent::read(home.path()), Consent::default());
}
