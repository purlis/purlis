// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The oracle**: the written action straight to Cedar's authorizer (spec §14.4, Q317.4).
//!
//! This file names nothing of the host's decision path: not its request builder, not its
//! loader, not its modes. `the_oracle_never_calls_the_hook_request_builder` reads it to keep it
//! so, so a change here that reaches for one fails there.

use purlis_core::enforcement::{Family, Mode};
use purlis_core::policy::decide::Outcome;
use purlis_core::policy::{self, File, Level, Question, World};

use super::{Answer, CHAT, Cell, PolicyInput};

/// **The oracle's expectation** for `cell` under `input`: straight to Cedar's authorizer.
///
/// The mode is the template's, read from `enforcement::Template`: a cell under a template that
/// does not enforce the action's family waits for slice 10's modes, so it is refused here rather
/// than guessed.
pub fn oracle(input: &PolicyInput, cell: &Cell) -> Answer {
    let mut files = vec![File::shipped_default()];
    if let Some(rules) = &input.rules {
        files.push(File::at("policy.cedar", rules, Level::Project));
    }
    let policy = policy::load(&files).expect("the oracle's policy loads");
    let decided = policy.decide(
        &Question {
            chat: CHAT,
            action: cell.action,
            resource: cell.resource,
            context: cell.context.clone(),
        },
        &World::from_json(serde_json::json!([])).expect("an empty world fits any schema"),
    );
    let ids = |ids: &[String]| ids.to_vec();
    let (outcome, deciding) = match &decided {
        policy::Decision::Allow { permits } => (Outcome::Allow, ids(permits)),
        policy::Decision::Ask { asks } => (Outcome::Ask, ids(asks)),
        policy::Decision::Deny { forbids } => (Outcome::Deny, ids(forbids)),
        policy::Decision::NoPermit => (Outcome::Deny, vec![policy::EVERYTHING_ELSE.to_owned()]),
        policy::Decision::Erred { rules } => (Outcome::Deny, ids(rules)),
        policy::Decision::Unasked { .. } => (Outcome::Deny, Vec::new()),
    };
    let family = policy::family_of(cell.action)
        .and_then(Family::of_key)
        .expect("the action is in a family");
    assert_eq!(
        input.template.mode(family),
        Mode::Enforce,
        "the oracle judges what a template enforces; the other modes are slice 10's"
    );
    Answer {
        outcome,
        deciding,
        after_mode: outcome,
    }
}
