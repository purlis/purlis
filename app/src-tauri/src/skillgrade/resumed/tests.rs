// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! #5718: Resume is planned and confirmed like any grade, offered only while what decides the
//! result holds, waits for a usage limit's reset, and Needs you says when it can run. No chat is
//! run here: what a grade ran is written by hand.

use std::collections::BTreeMap;

use purlis_core::skills::grade::{ChatEnd, ChatRan, Earlier, GradeEnd, Kept};
use purlis_core::skills::grades::SkillCopy;
use purlis_core::skills::verbs::drafts::Snapshot;
use purlis_core::skilluse::Content;

use super::*;
use crate::skillgrade::tests::planned;
use crate::skillstudio::needs::{NeedFix, NeedKind};

fn copy(text: &str) -> SkillCopy {
    SkillCopy {
        snapshot: Snapshot {
            name: "deploy".into(),
            files: BTreeMap::from([("SKILL.md".to_owned(), text.to_owned())]),
        },
        content: Content::Uncommitted,
        commit: None,
        previous: None,
        previous_refused: None,
        inputs: BTreeMap::new(),
    }
}

fn on() -> On {
    On {
        profile: Some("work".into()),
        checked: Ok("full sandbox".into()),
        priced: grades::price::of_model(None),
        models: Vec::new(),
        model: None,
        evals_refused: None,
        harness: Some("claude".into()),
    }
}

/// What a grade of `deploy` reads now, its skill's text `text`.
fn now(text: &str) -> Now {
    let mut planned = planned("ship it");
    planned.copy = Some(copy(text));
    let on = on();
    let deciding = resumable::Deciding::of(
        planned.request.as_ref().unwrap(),
        planned.copy.as_ref().unwrap(),
        resumable::On {
            profile: on.profile.as_deref(),
            program: on.harness.as_deref(),
            model: on.priced.model.as_deref().or(on.model.as_deref()),
        },
        None,
    );
    Now {
        planned,
        on,
        deciding,
        level: Level::project(),
        draft: None,
    }
}

fn done() -> ChatRan {
    ChatRan {
        chat: Some("01CHAT".into()),
        started: vec!["deploy".into()],
        tokens: Some(1_000),
        cost_usd: Some(0.02),
        ms: 1000,
        end: ChatEnd::Turn,
        answer: None,
    }
}

/// A grade of `deploy` as it read with the text `v1`, stopped as `ended` says on its second run:
/// one chat of each arm had its result.
fn stopped(ended: GradeEnd, resets_at: Option<&str>) -> resumable::Stopped {
    let then = now("v1");
    resumable::Stopped {
        v: resumable::VERSION,
        grade: "01STOPPED".into(),
        skill: "deploy".into(),
        level: Some("skills".into()),
        draft: None,
        draft_of: None,
        request: then.planned.request.clone().unwrap(),
        kept: Kept {
            triggers: vec![vec![vec![done()], vec![done()]]],
            outcomes: Vec::new(),
            earlier: Earlier {
                tokens: Some(2_000),
                usd: Some(0.04),
                ..Earlier::default()
            },
        },
        deciding: then.deciding,
        ended,
        resets_at: resets_at.map(str::to_owned),
        ts: "2026-11-08T10:00:00+00:00".into(),
    }
}

fn at(time: &str) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339(time)
        .unwrap()
        .with_timezone(&chrono::Utc)
}

#[test]
fn a_resume_is_planned_with_its_own_cap_and_what_the_stopped_grade_spent() {
    let stopped = stopped(GradeEnd::OverCap, None);
    let plan = shown(&stopped, Ok(now("v1")), None, at("2026-11-08T11:00:00Z"));
    assert_eq!(plan.ended, "Stopped at the token cap");
    assert_eq!(plan.refused, None);
    // Three runs of two arms; one of each had its result.
    assert_eq!(plan.left, 4);
    // 2,000 tokens over the two chats it ran, for four more.
    assert_eq!(plan.tokens_left, Some(4_000.0));
    assert_eq!(plan.token_cap, 50_000.0);
    assert!(plan.cap_set_here);
    assert_eq!(plan.earlier_tokens, Some(2_000.0));
    assert!(plan.consent.is_some(), "Resume is offered");
}

#[test]
fn a_changed_skill_offers_grade_again_alone_and_says_what_changed() {
    let stopped = stopped(GradeEnd::Cancelled, None);
    let plan = shown(&stopped, Ok(now("v2")), None, at("2026-11-08T11:00:00Z"));
    assert_eq!(
        plan.refused.as_deref(),
        Some(
            "Resume is not offered: the skill's text changed since it stopped. Grade again runs \
             it whole."
        )
    );
    assert_eq!(plan.consent, None, "a press consents to nothing");
}

#[test]
fn a_copy_that_cannot_be_read_now_offers_grade_again_alone() {
    let stopped = stopped(GradeEnd::Cancelled, None);
    let plan = shown(
        &stopped,
        Err("the copy it graded is not found here".into()),
        None,
        at("2026-11-08T11:00:00Z"),
    );
    assert_eq!(
        plan.refused.as_deref(),
        Some(
            "Resume is not offered: the copy it graded is not found here. Grade again runs it \
             whole."
        )
    );
}

#[test]
fn a_stop_at_the_usage_limit_names_the_program_and_waits_for_its_reset() {
    let stopped = stopped(GradeEnd::UsageLimit, Some("2026-11-08T14:00:00Z"));
    let before = shown(&stopped, Ok(now("v1")), None, at("2026-11-08T11:00:00Z"));
    assert_eq!(
        before.ended,
        "Stopped: Claude Code's usage limit was reached"
    );
    assert_eq!(before.waits_until.as_deref(), Some("2026-11-08T14:00:00Z"));
    let after = shown(&stopped, Ok(now("v1")), None, at("2026-11-08T14:01:00Z"));
    assert_eq!(after.waits_until, None);
    assert!(after.consent.is_some());
}

#[test]
fn each_stop_reason_is_said_in_the_operators_words() {
    let said = |end| grades::ended_said_on(end, Some("claude"));
    assert_eq!(said(GradeEnd::OverCap), "Stopped at the token cap");
    assert_eq!(
        said(GradeEnd::UsageLimit),
        "Stopped: Claude Code's usage limit was reached"
    );
    assert_eq!(said(GradeEnd::Unreachable), "Stopped: network unreachable");
    assert_eq!(said(GradeEnd::Cancelled), "Stopped by you");
}

#[test]
fn needs_you_says_when_each_stopped_grade_can_resume_and_leaves_the_persons_own_stop_out() {
    let limit = stopped(GradeEnd::UsageLimit, Some("2026-11-08T14:00:00Z"));
    let network = resumable::Stopped {
        grade: "01NETWORK".into(),
        ..stopped(GradeEnd::Unreachable, None)
    };
    let by_you = resumable::Stopped {
        grade: "01BYYOU".into(),
        ..stopped(GradeEnd::Cancelled, None)
    };
    let items = needs_of(vec![limit, network, by_you], at("2026-11-08T11:00:00Z"));
    assert_eq!(items.len(), 2, "{items:?}");
    assert!(items.iter().all(|one| one.kind == NeedKind::Resume));
    assert!(
        items[0]
            .reason
            .starts_with("Its grade stopped: Claude Code's usage limit was reached. Resume can run after your limit resets at "),
        "{}",
        items[0].reason
    );
    assert_eq!(
        items[1].reason,
        "Its grade stopped: network unreachable. Resume can run now. It runs the 4 chats left and \
         keeps what ran."
    );
    assert_eq!(
        items[1].fix,
        NeedFix::OpenGrade {
            name: "deploy".into(),
            grade: "01NETWORK".into(),
            draft: None,
        }
    );
}

#[test]
fn a_kept_run_of_another_shape_is_never_resumed() {
    let mut stopped = stopped(GradeEnd::Cancelled, None);
    stopped.kept.triggers.push(vec![Vec::new(), Vec::new()]);
    let plan = shown(&stopped, Ok(now("v1")), None, at("2026-11-08T11:00:00Z"));
    assert!(plan.refused.is_some());
    assert_eq!(plan.consent, None);
}
