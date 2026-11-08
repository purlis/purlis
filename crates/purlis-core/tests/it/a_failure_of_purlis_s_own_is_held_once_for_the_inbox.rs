// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A failure of purlis's own is a draft in the Inbox, held once** (#5697, spec #5683 layer
//! 1): an internal failure a hook caught, and a sandbox block of purlis's own operation (#1338),
//! by one road. The draft shows the call, its exit and its scrubbed error, and what its chat ran
//! under (Q5); the agent may add what it was doing, and only to a draft of its own chat. The
//! same failure (its call and error class, in this purlis version, Q15) from any chat is one
//! draft, and each time it is met again is counted: on the draft, or on the sent list's entry
//! once it left.

use std::path::Path;
use std::sync::Arc;

use purlis_core::caught::Caught;
use purlis_core::forge::Caller;
use purlis_core::report::intake::{Dsn, Recorded, Sentry};
use purlis_core::report::{
    Added, CameFrom, Consent, Evidence, Known, PersonaKind, PurlisFailure, RanAs, Refusal, Reports,
    SentList, Stream,
};

use crate::a_chats_report_draft_is_filed_from_the_inbox::recorded_intake;

fn reports(root: &Path, agent_drafts: Option<bool>) -> Reports {
    reports_sending_to(root, agent_drafts, None, recorded_intake())
}

/// Reports kept under `root`, with these answers, sending to `intake`.
fn reports_sending_to(
    root: &Path,
    agent_drafts: Option<bool>,
    always_send: Option<bool>,
    intake: Arc<Recorded>,
) -> Reports {
    Reports::new(Known {
        home: Some("/Users/dev".into()),
        ..Known::default()
    })
    .with_consent(Consent {
        crash_reports: None,
        agent_drafts,
        asked: true,
        crash_notice: None,
        always_send,
    })
    .sending_to(
        Arc::new(Sentry::new(
            Dsn::parse(concat!(
                "https://f00dfeed1234",
                "@",
                "o1.ingest.de.sentry.io/4508"
            ))
            .expect("a DSN"),
            intake,
        )),
        root,
    )
}

fn from(session: u32) -> CameFrom {
    CameFrom {
        session: Some(session),
        chat: format!("steward {session}"),
        project: "/plane".into(),
    }
}

fn internal() -> Evidence {
    failing("purlis: internal failure: no index at /Users/dev/plane/.git/index")
}

/// The commit command failing inside purlis with `error`, in a Claude Code chat on the
/// project's own persona.
fn failing(error: &str) -> Evidence {
    Evidence::PurlisFailure(PurlisFailure::Internal {
        caught: Caught {
            call: "purlis worktree commit".into(),
            exit: Some(70),
            error: error.into(),
        },
        ran_as: RanAs {
            harness: Some(purlis_core::harness::Harness::ClaudeCode),
            harness_version: Some("2.1.4".into()),
            persona: PersonaKind::Own,
        },
    })
}

#[test]
fn an_internal_failure_is_held_with_its_call_its_exit_and_its_scrubbed_error() {
    purlis_core::unsteered!();
    let root = tempfile::tempdir().unwrap();
    let reports = reports(root.path(), Some(true));

    let one = reports
        .hold_once(internal(), &Caller::chat("4"), from(4))
        .expect("held")
        .expect("new");

    assert!(
        one.line
            .starts_with("purlis worktree commit · exit 70 · purlis "),
        "{}",
        one.line
    );
    assert!(
        one.title.contains("purlis worktree commit"),
        "{}",
        one.title
    );
    assert!(one.body.contains("no index at"), "{}", one.body);
    assert!(
        !one.body.contains("/Users/dev"),
        "a path left: {}",
        one.body
    );
    let harness = purlis_core::harness::Harness::ClaudeCode.name();
    assert!(
        one.body.contains(&format!("**harness:** {harness}")),
        "{}",
        one.body
    );
    assert!(
        one.body.contains("**harness version:** 2.1.4"),
        "{}",
        one.body
    );
    assert_eq!(reports.held(), [one]);
}

#[test]
fn its_footer_says_the_harness_its_version_and_the_persona_s_kind_never_its_name() {
    purlis_core::unsteered!();
    let root = tempfile::tempdir().unwrap();
    let reports = reports(root.path(), Some(true));

    let one = reports
        .hold_once(internal(), &Caller::chat("4"), from(4))
        .expect("held")
        .expect("new");

    let footer = one.body.rsplit("\n---\n").next().unwrap_or_default();
    let harness = purlis_core::harness::Harness::ClaudeCode.name();
    assert!(
        footer.contains(&format!(" · {harness} 2.1.4 · the project's own persona")),
        "{footer}"
    );
    assert!(
        !one.body.contains("steward"),
        "the persona's name: {}",
        one.body
    );
}

#[test]
fn the_same_failure_from_any_chat_is_one_draft_and_each_repeat_is_counted() {
    purlis_core::unsteered!();
    let root = tempfile::tempdir().unwrap();
    let reports = reports(root.path(), Some(true));
    let first = reports
        .hold_once(internal(), &Caller::chat("4"), from(4))
        .expect("held")
        .expect("new");

    assert_eq!(
        reports.hold_once(internal(), &Caller::chat("4"), from(4)),
        Ok(None),
        "met again in the same chat"
    );
    assert_eq!(
        reports.hold_once(internal(), &Caller::chat("5"), from(5)),
        Ok(None),
        "met in another chat: the same failure of purlis's"
    );

    let held = reports.held();
    assert_eq!(held.len(), 1, "{held:?}");
    assert_eq!(
        (held[0].id.as_str(), held[0].repeats),
        (first.id.as_str(), 2)
    );
}

#[test]
fn another_path_or_number_in_the_error_is_the_same_failure_and_another_error_is_not() {
    purlis_core::unsteered!();
    let root = tempfile::tempdir().unwrap();
    let reports = reports(root.path(), Some(true));
    let hold = |error: &str, chat: u32| {
        reports
            .hold_once(failing(error), &Caller::chat(&chat.to_string()), from(chat))
            .expect("held")
    };

    assert!(
        hold(
            "purlis: internal failure: no index at /Users/dev/a/.git/index (7)",
            4
        )
        .is_some()
    );
    assert!(
        hold(
            "purlis: internal failure: no index at /Users/dev/b/.git/index (12)",
            5
        )
        .is_none(),
        "the same failure on another repo"
    );
    assert!(
        hold("purlis: internal failure: the lock file is held", 6).is_some(),
        "another failure of the same call is a draft of its own"
    );
    assert_eq!(reports.held().len(), 2);
}

#[test]
fn a_failure_met_again_after_its_draft_left_is_counted_and_never_drafted_again() {
    purlis_core::unsteered!();
    let root = tempfile::tempdir().unwrap();
    let intake = recorded_intake();
    let reports = reports_sending_to(root.path(), Some(true), Some(true), intake.clone());
    let one = reports
        .hold_once(internal(), &Caller::chat("4"), from(4))
        .expect("held")
        .expect("new");
    assert!(one.sends_at.is_some(), "Always send holds it to send");
    assert_eq!(
        reports.hold_once(internal(), &Caller::chat("5"), from(5)),
        Ok(None)
    );
    let later = chrono::Utc::now() + chrono::Duration::seconds(60);
    assert_eq!(reports.send_held_due(later), 1);

    let first = String::from_utf8(intake.received()[0].clone()).unwrap();
    let fingerprint = one
        .fingerprint
        .clone()
        .expect("a failure of purlis's own has one");
    assert!(
        first.contains(&format!("\"fingerprint\":[\"{fingerprint}\"]")),
        "the intake groups it by its fingerprint: {first}"
    );
    assert!(
        first.contains(&format!(
            "{{\"fingerprint\":\"{fingerprint}\",\"count\":1}}"
        )),
        "the repeat met while it waited rides on it: {first}"
    );
    assert!(first.contains("\"persona_kind\":\"own\""), "{first}");
    assert!(first.contains("\"harness_version\":\"2.1.4\""), "{first}");
    assert!(!first.contains("steward"), "the persona's name: {first}");

    assert_eq!(
        reports.hold_once(internal(), &Caller::chat("6"), from(6)),
        Ok(None),
        "met again after it left: counted, never drafted again"
    );
    assert!(reports.held().is_empty());
    let sent = SentList::read(root.path());
    assert_eq!(
        (sent.len(), sent[0].repeats, sent[0].repeats_sent),
        (1, 2, 1),
        "{sent:?}"
    );
}

#[test]
fn a_sandbox_block_of_purlis_s_own_takes_the_same_road() {
    purlis_core::unsteered!();
    use purlis_core::sandboxblock::{Block, Kind, Operation};
    let root = tempfile::tempdir().unwrap();
    let reports = reports(root.path(), Some(true));
    let block = || {
        Evidence::PurlisFailure(PurlisFailure::SandboxBlock {
            block: Block {
                operation: Operation::Write,
                kind: Kind::ProjectState,
                ours: true,
            },
            ran_as: RanAs::default(),
        })
    };

    let one = reports
        .hold_once(block(), &Caller::chat("4"), from(4))
        .expect("held")
        .expect("new");

    assert!(
        one.title.starts_with("Sandbox blocked purlis's own write"),
        "{}",
        one.title
    );
    assert_eq!(
        reports.hold_once(block(), &Caller::chat("5"), from(5)),
        Ok(None),
        "the same block from another chat is counted"
    );
}

#[test]
fn without_a_yes_to_agent_drafts_nothing_of_purlis_s_own_is_held() {
    purlis_core::unsteered!();
    let root = tempfile::tempdir().unwrap();
    let reports = reports(root.path(), None);
    assert_eq!(
        reports.hold_once(internal(), &Caller::chat("4"), from(4)),
        Err(Refusal::NoConsent(Stream::AgentDrafts))
    );
    assert!(reports.held().is_empty());
}

#[test]
fn the_agent_adds_what_it_was_doing_to_its_own_chat_s_draft_only() {
    purlis_core::unsteered!();
    let root = tempfile::tempdir().unwrap();
    let reports = reports(root.path(), Some(true));
    let one = reports
        .hold_once(internal(), &Caller::chat("4"), from(4))
        .expect("held")
        .expect("new");

    assert_eq!(
        reports.add_to_held(&one.id, 5, "I was committing for chat 5"),
        Err(Refusal::NotHeld),
        "another chat's draft is not this chat's to add to"
    );
    let Ok(Added::ToTheDraft(added)) = reports.add_to_held(
        &one.id,
        4,
        "Committing the reporting ticket in /Users/dev/plane.",
    ) else {
        panic!("not added to the draft");
    };

    assert!(
        added.body.contains("Committing the reporting ticket"),
        "{}",
        added.body
    );
    assert!(
        !added.body.contains("/Users/dev"),
        "a path left: {}",
        added.body
    );
    assert!(
        added.body.contains("no index at"),
        "the error stays: {}",
        added.body
    );
    assert_eq!(reports.held(), [*added]);
}
