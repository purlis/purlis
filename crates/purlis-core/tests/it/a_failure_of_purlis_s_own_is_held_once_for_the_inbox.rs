// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A failure of purlis's own is a draft in the Inbox, held once** (#5697, spec #5683 layer
//! 1): an internal failure a hook caught, and a sandbox block of purlis's own operation (#1338),
//! by one road. The draft shows the call, its exit and its scrubbed error; the agent may add
//! what it was doing, and only to a draft of its own chat.

use std::path::Path;
use std::sync::Arc;

use purlis_core::caught::Caught;
use purlis_core::forge::Caller;
use purlis_core::report::intake::{Dsn, Sentry};
use purlis_core::report::{
    CameFrom, Consent, Evidence, Known, PurlisFailure, Refusal, Reports, Stream,
};

use crate::a_chats_report_draft_is_filed_from_the_inbox::recorded_intake;

fn reports(root: &Path, agent_drafts: Option<bool>) -> Reports {
    Reports::new(Known {
        home: Some("/Users/dev".into()),
        ..Known::default()
    })
    .with_consent(Consent {
        crash_reports: None,
        agent_drafts,
        asked: true,
        crash_notice: None,
    })
    .sending_to(
        Arc::new(Sentry::new(
            Dsn::parse(concat!(
                "https://f00dfeed1234",
                "@",
                "o1.ingest.de.sentry.io/4508"
            ))
            .expect("a DSN"),
            recorded_intake(),
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
    Evidence::PurlisFailure(PurlisFailure::Internal {
        caught: Caught {
            call: "purlis worktree commit".into(),
            exit: Some(70),
            error: "purlis: internal failure: no index at /Users/dev/plane/.git/index".into(),
        },
        harness: Some(purlis_core::harness::Harness::ClaudeCode),
        harness_version: Some("2.1.4".into()),
    })
}

#[test]
fn an_internal_failure_is_held_with_its_call_its_exit_and_its_scrubbed_error() {
    let root = tempfile::tempdir().unwrap();
    let reports = reports(root.path(), Some(true));

    let (one, new) = reports
        .hold_once(internal(), &Caller::chat("4"), from(4))
        .expect("held");

    assert!(new);
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
fn the_same_failure_from_the_same_chat_is_held_once_while_its_draft_waits() {
    let root = tempfile::tempdir().unwrap();
    let reports = reports(root.path(), Some(true));
    let (first, _) = reports
        .hold_once(internal(), &Caller::chat("4"), from(4))
        .expect("held");

    let (again, new) = reports
        .hold_once(internal(), &Caller::chat("4"), from(4))
        .expect("held");
    assert!(!new);
    assert_eq!(again.id, first.id);

    let (other, new) = reports
        .hold_once(internal(), &Caller::chat("5"), from(5))
        .expect("held");
    assert!(new, "another chat's failure is its own draft");
    assert_eq!(reports.held().len(), 2, "{:?}", reports.held());
    assert_ne!(other.id, first.id);
}

#[test]
fn a_sandbox_block_of_purlis_s_own_takes_the_same_road() {
    use purlis_core::sandboxblock::{Block, Kind, Operation};
    let root = tempfile::tempdir().unwrap();
    let reports = reports(root.path(), Some(true));
    let block = Evidence::PurlisFailure(PurlisFailure::SandboxBlock {
        block: Block {
            operation: Operation::Write,
            kind: Kind::ProjectState,
            ours: true,
        },
        harness: None,
        harness_version: None,
    });

    let (one, new) = reports
        .hold_once(block, &Caller::chat("4"), from(4))
        .expect("held");

    assert!(new);
    assert!(
        one.title.starts_with("Sandbox blocked purlis's own write"),
        "{}",
        one.title
    );
}

#[test]
fn without_a_yes_to_agent_drafts_nothing_of_purlis_s_own_is_held() {
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
    let root = tempfile::tempdir().unwrap();
    let reports = reports(root.path(), Some(true));
    let (one, _) = reports
        .hold_once(internal(), &Caller::chat("4"), from(4))
        .expect("held");

    assert_eq!(
        reports.add_to_held(&one.id, 5, "I was committing for chat 5"),
        Err(Refusal::NotHeld),
        "another chat's draft is not this chat's to add to"
    );
    let added = reports
        .add_to_held(
            &one.id,
            4,
            "Committing the reporting ticket in /Users/dev/plane.",
        )
        .expect("added");

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
    assert_eq!(reports.held(), [added]);
}
