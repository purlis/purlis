// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Proof infrastructure, first half** (M75 slice 09, #7018; spec §14.1 to §14.4): a cell's
//! policy is an input, strict when none is named; an equality cell's expectation comes from the
//! oracle, which shares nothing with the host's request builder; and "equal" is the same
//! outcome, the same deciding `@id` and the same outcome after the mode.
//!
//! Not here: the fake leg and the two S-B3 cells (a scrubbed environment, a fake socket), which
//! wait for M74.1's scenario host on `main` (P3). Each needs the plane's host to judge it.

use purlis_core::enforcement::Template;
use purlis_core::policy::decide::Outcome;

use crate::policy_proof::{
    Answer, Batch, Cell, Mismatch, PolicyInput, compare, engine, equality, oracle,
};

/// A chat's merge of `acme/web#main`: the context a host makes for every action.
fn merge() -> Cell {
    Cell {
        policy: None,
        action: "merge",
        resource: ("Branch", "acme/web#main"),
        context: serde_json::json!({
            "sandbox": "floor",
            "attended": true,
            "harness_mode": "default",
            "depth": 0,
            "via": "hook",
            "owner": "none",
        }),
        gap: None,
    }
}

#[test]
fn a_cell_with_no_policy_runs_under_strict() {
    purlis_core::unsteered!();
    let batch = Batch {
        policy: None,
        cells: vec![merge()],
    };
    let input = batch.policy_of(&batch.cells[0]);
    assert_eq!(input.template, Template::Strict);
    assert_eq!(input.rules, None);
    // The cell's own policy beats the batch's, and the batch's beats strict.
    let open = PolicyInput::template(Template::Open);
    let batched = Batch {
        policy: Some(open.clone()),
        cells: vec![merge()],
    };
    assert_eq!(batched.policy_of(&batched.cells[0]), open);
    let own = PolicyInput::template(Template::Team);
    let cell = Cell {
        policy: Some(own.clone()),
        ..merge()
    };
    assert_eq!(batched.policy_of(&cell), own);
    // And strict is what runs: the merge is held for the person.
    assert_eq!(engine(&input, &batch.cells[0]).after_mode, Outcome::Ask);
}

/// A `use` statement with its whitespace and trailing comma gone, so rustfmt's wrapping of one
/// reads as the line it came from.
fn squeezed(statement: &str) -> String {
    statement
        .split_whitespace()
        .collect::<String>()
        .replace(",}", "}")
}

/// The oracle's `use` statements, and no others: Cedar, the loader's four names, and the
/// harness's own types.
const ORACLE_MAY_USE: [&str; 4] = [
    "use std::str::FromStr;",
    "use cedar_policy::{Authorizer, Context, Decision, Entities, EntityId, EntityTypeName, \
     EntityUid, Policy, PolicyId, PolicySet, Request, Schema};",
    "use purlis_core::policy::{DEFAULT, File, Level, SCHEMA, load};",
    "use super::{Answer, CHAT, Cell, Outcome, PolicyInput};",
];

#[test]
fn the_oracle_names_nothing_of_the_host_decision_path() {
    purlis_core::unsteered!();
    // Comments count: a name of the host's path in a doc line is a name of it.
    let source = include_str!("policy_proof/oracle.rs");
    let allowed: Vec<String> = ORACLE_MAY_USE.iter().map(|s| squeezed(s)).collect();
    let mut rest = String::new();
    for statement in source.split_inclusive(';') {
        let code: String = statement
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        let code = code.trim();
        if code.starts_with("use ") || code.starts_with("pub use ") {
            assert!(
                allowed.contains(&squeezed(code)),
                "the oracle imports `{code}`, which is not on its allow-list"
            );
        } else {
            rest.push_str(statement);
        }
    }
    assert!(
        !rest.contains("purlis_core"),
        "the oracle reaches purlis_core other than by an allowed `use`"
    );
    // The host's path, wherever it appears in the file: the harness's `engine`, the loaded
    // policy, `decide::`, the evaluation behind it, and what the hook builds a request from.
    // Each is read where it lives, so a rename cannot leave this list guarding nothing.
    let mod_rs = include_str!("policy_proof/mod.rs");
    let policy_rs = include_str!("../../src/policy.rs");
    let decide_rs = include_str!("../../src/policy/decide.rs");
    for (name, home, there) in [
        ("engine", mod_rs, "pub fn engine"),
        (".loaded(", mod_rs, "pub fn loaded"),
        ("decide::", mod_rs, "decide::decide("),
        ("evaluate", policy_rs, "pub fn evaluate"),
        ("Asker", decide_rs, "pub struct Asker"),
        ("for_chat", decide_rs, "pub fn for_chat"),
        ("in_plane", decide_rs, "pub fn in_plane"),
        ("decide_locally", decide_rs, "pub fn decide_locally"),
        ("levels", mod_rs, "levels::load_held"),
        ("force::", decide_rs, "force::"),
        ("hookwire", decide_rs, "hookwire"),
        ("Loaded", mod_rs, "levels::Loaded"),
        ("canonical_host", policy_rs, "fn canonical_host"),
        (
            "world_with_suffix_sets",
            policy_rs,
            "fn world_with_suffix_sets",
        ),
    ] {
        assert!(
            home.contains(there),
            "`{there}` has moved: update this test"
        );
        assert!(
            !source.contains(name),
            "the oracle names `{name}`, which is on the host's decision path"
        );
    }
    // And it does reach Cedar's authorizer.
    assert!(source.contains("is_authorized("));
}

#[test]
fn a_malformed_cell_fails_loudly_in_the_oracle() {
    purlis_core::unsteered!();
    let without_via = Cell {
        context: serde_json::json!({
            "sandbox": "floor",
            "attended": true,
            "harness_mode": "default",
            "depth": 0,
        }),
        ..merge()
    };
    let misspelt_action = Cell {
        action: "mrege",
        ..merge()
    };
    let wrong_resource = Cell {
        resource: ("Host", "example.com"),
        ..merge()
    };
    let wrong_type = Cell {
        resource: ("Repo", "acme/web"),
        ..merge()
    };
    let mistyped_field = Cell {
        context: serde_json::json!({
            "sandbox": "floor",
            "attended": "yes",
            "harness_mode": "default",
            "depth": 0,
            "via": "hook",
            "owner": "none",
        }),
        ..merge()
    };
    for cell in [
        without_via,
        misspelt_action,
        wrong_resource,
        wrong_type,
        mistyped_field,
    ] {
        let caught = std::panic::catch_unwind(|| oracle(&PolicyInput::default(), &cell));
        let payload = caught.expect_err(&format!("the oracle answered {cell:?}"));
        let said = payload
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| payload.downcast_ref::<&str>().map(|s| (*s).to_owned()))
            .unwrap_or_default();
        assert!(
            said.contains(cell.action),
            "the oracle's panic does not name the cell {cell:?}: {said}"
        );
    }
}

#[test]
fn an_equality_cell_takes_its_expectation_from_the_oracle() {
    purlis_core::unsteered!();
    let cell = merge();
    let batch = Batch::default();
    let input = batch.policy_of(&cell);
    // The oracle expects the shipped default's ask, by its own `@id`, with no mode lifting it.
    let expected = oracle(&input, &cell);
    assert_eq!(
        expected,
        Answer {
            outcome: Outcome::Ask,
            deciding: vec!["guards-merge-hold".to_owned()],
            after_mode: Outcome::Ask,
        }
    );
    // The merge hold's equality cell under strict: the engine says the same.
    assert_eq!(equality(&batch, &cell), Ok(()));
    // A project's rule moves the expectation with it, and the engine follows.
    let denying = PolicyInput::with_rules(
        Template::Strict,
        "@id(\"guards-merge-never\") @level(\"project\")\n\
         forbid (principal, action == Action::\"merge\", resource);\n",
    );
    let batch = Batch {
        policy: Some(denying),
        cells: vec![cell.clone()],
    };
    let expected = oracle(&batch.policy_of(&cell), &cell);
    assert_eq!(expected.outcome, Outcome::Deny);
    assert!(expected.deciding.contains(&"guards-merge-never".to_owned()));
    assert_eq!(equality(&batch, &cell), Ok(()));
}

#[test]
fn a_harness_block_with_no_rule_is_a_mismatch_unless_its_gap_is_declared() {
    purlis_core::unsteered!();
    let cell = merge();
    let expected = oracle(&PolicyInput::default(), &cell);
    // The harness stopped the merge and no rule says why.
    let blocked = Answer {
        outcome: Outcome::Deny,
        deciding: Vec::new(),
        after_mode: Outcome::Deny,
    };
    assert_eq!(
        compare(&expected, &blocked, None),
        Err(Mismatch::BlockWithNoRule)
    );
    assert_eq!(
        compare(
            &expected,
            &blocked,
            Some("the harness has no hook for merges")
        ),
        Ok(())
    );
    // A looser answer is an escape, a stricter one a bug, whatever gap is declared.
    let allowed = Answer {
        outcome: Outcome::Allow,
        deciding: vec!["policy-everything-else".to_owned()],
        after_mode: Outcome::Allow,
    };
    assert_eq!(
        compare(&expected, &allowed, Some("a gap")),
        Err(Mismatch::Looser)
    );
    let stricter = Answer {
        outcome: Outcome::Deny,
        deciding: vec!["guards-merge-hold".to_owned()],
        after_mode: Outcome::Deny,
    };
    assert_eq!(compare(&expected, &stricter, None), Err(Mismatch::Stricter));
    // The same outcome decided by another rule is not equal.
    let other = Answer {
        deciding: vec!["guards-other".to_owned()],
        ..expected.clone()
    };
    assert_eq!(compare(&expected, &other, None), Err(Mismatch::OtherRule));
    // The same outcome after the mode, but another before it, is its own mismatch.
    let lifted = Answer {
        outcome: Outcome::Allow,
        ..expected.clone()
    };
    assert_eq!(
        compare(&expected, &lifted, None),
        Err(Mismatch::OutcomeBeforeMode)
    );
}
