// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The oracle**: the written action straight to Cedar's authorizer (spec §14.4, Q317.4).
//!
//! It builds its own `cedar_policy::Request` from the cell, parses its own `PolicySet` from the
//! shipped default's text and the cell's rules, and calls `Authorizer::is_authorized` over no
//! entities. The outcome and the deciding `@id`s are read off Cedar's `Response` by the
//! documented rules: a permit with no forbid allows; a forbid with `@ask` asks, if a permit
//! stands under it; any other forbid denies; with no permit, the catch-all
//! `policy-everything-else` is what denies.
//!
//! **What it shares with the judged path: the loader, and only as a gate.** `policy::load` is
//! called to refuse a policy the host would refuse (its layering, `@id` and `@level` checks), and
//! its result is dropped. The loader's text constants (the shipped default and the schema) are
//! read, not its code. The request builder, the schema's parsed form, the host spelling, the
//! suffix sets, the ask and deny derivation and the permits-only pass are all written here.
//!
//! A cell the schema cannot ask is a broken cell, so the oracle panics on it, naming the cell:
//! the host answers an unfit request with a deny and no rule, which would otherwise agree with an
//! oracle that said the same. A `Host` resource also panics: its spelling and wildcard sets are
//! the family slices' to write here.
//!
//! `the_oracle_names_nothing_of_the_host_decision_path` reads this file, comments included, so a
//! change here that reaches for the host's path fails there.

use std::str::FromStr;

use cedar_policy::{
    Authorizer, Context, Decision, Entities, EntityId, EntityTypeName, EntityUid, Policy, PolicyId,
    PolicySet, Request, Schema,
};
use purlis_core::policy::{DEFAULT, File, Level, SCHEMA, load};

use super::{Answer, CHAT, Cell, Outcome, PolicyInput};

/// **The oracle's expectation** for `cell` under `input`: straight to Cedar's authorizer, with
/// the outcome as Cedar leaves it and no mode applied.
///
/// # Panics
///
/// On a cell the schema cannot ask, or one that makes Cedar err: either is a broken cell.
pub fn judge(input: &PolicyInput, cell: &Cell) -> Answer {
    let mut files = vec![File::shipped_default()];
    if let Some(rules) = &input.rules {
        files.push(File::at("policy.cedar", rules, Level::Project));
    }
    load(&files).expect("the policy of a cell loads");

    let (all, asks) = policies(
        [Some(DEFAULT), input.rules.as_deref()]
            .into_iter()
            .flatten()
            .collect(),
    );
    let request = request(cell);
    let authorizer = Authorizer::new();
    let response = authorizer.is_authorized(&request, &all, &Entities::empty());
    let erred: Vec<String> = response
        .diagnostics()
        .errors()
        .map(|e| e.to_string())
        .collect();
    assert!(erred.is_empty(), "a rule erred on {cell:?}: {erred:?}");
    let mut reasons: Vec<String> = response
        .diagnostics()
        .reason()
        .map(|id| id.to_string())
        .collect();
    reasons.sort();
    let no_permit = vec!["policy-everything-else".to_owned()];

    let (outcome, deciding) = match response.decision() {
        Decision::Allow => (Outcome::Allow, reasons),
        Decision::Deny if reasons.is_empty() => (Outcome::Deny, no_permit),
        Decision::Deny => {
            let plain: Vec<String> = reasons
                .iter()
                .filter(|id| !asks.contains(id))
                .cloned()
                .collect();
            if !plain.is_empty() {
                (Outcome::Deny, plain)
            } else if permits_stand(&authorizer, &request, &all) {
                (Outcome::Ask, reasons)
            } else {
                (Outcome::Deny, no_permit)
            }
        }
    };
    Answer {
        outcome,
        deciding,
        after_mode: outcome,
    }
}

/// The policies of `texts`, layered in order and keyed by `@id` (a later rule of one `@id`
/// replaces an earlier), and the `@id`s of those that ask.
fn policies(texts: Vec<&str>) -> (PolicySet, Vec<String>) {
    let mut held: Vec<(String, Policy)> = Vec::new();
    for text in texts {
        let set = PolicySet::from_str(text).expect("the rules are Cedar");
        for policy in set.policies() {
            let id = policy.annotation("id").expect("a rule has an @id");
            let policy = policy.new_id(PolicyId::new(id));
            match held.iter_mut().find(|(held, _)| held == id) {
                Some(slot) => slot.1 = policy,
                None => held.push((id.to_owned(), policy)),
            }
        }
    }
    let asks = held
        .iter()
        .filter(|(_, policy)| policy.annotation("ask").is_some())
        .map(|(id, _)| id.clone())
        .collect();
    let mut all = PolicySet::new();
    for (_, policy) in held {
        all.add(policy).expect("each @id is held once");
    }
    (all, asks)
}

/// Whether some permit applies to `request` with every forbid left out.
fn permits_stand(authorizer: &Authorizer, request: &Request, all: &PolicySet) -> bool {
    let mut permits = PolicySet::new();
    for policy in all
        .policies()
        .filter(|p| p.effect() == cedar_policy::Effect::Permit)
    {
        permits.add(policy.clone()).expect("a permit is held once");
    }
    authorizer
        .is_authorized(request, &permits, &Entities::empty())
        .decision()
        == Decision::Allow
}

/// The Cedar request `cell` writes, checked against the schema.
fn request(cell: &Cell) -> Request {
    assert!(
        cell.resource.0 != "Host",
        "the oracle does not spell hosts yet: {cell:?}"
    );
    let (schema, _) = Schema::from_cedarschema_str(SCHEMA).expect("the shipped schema parses");
    let uid = |kind: &str, id: &str| {
        EntityUid::from_type_name_and_id(
            EntityTypeName::from_str(kind).expect("an entity type name"),
            EntityId::new(id),
        )
    };
    let action = uid("Action", cell.action);
    let context = Context::from_json_value(cell.context.clone(), Some((&schema, &action)))
        .unwrap_or_else(|e| panic!("the schema cannot ask {cell:?}: its context: {e}"));
    Request::new(
        uid("Chat", CHAT),
        action,
        uid(cell.resource.0, cell.resource.1),
        context,
        Some(&schema),
    )
    .unwrap_or_else(|e| panic!("the schema cannot ask {cell:?}: {e}"))
}
