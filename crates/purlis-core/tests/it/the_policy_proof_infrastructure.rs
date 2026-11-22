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
        }),
        gap: None,
    }
}

#[test]
fn a_cell_with_no_policy_runs_under_strict() {
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

#[test]
fn the_oracle_never_calls_the_hook_request_builder() {
    let source = include_str!("policy_proof/oracle.rs");
    let code: String = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    // The host's decision path: its request builder (`Asker`, `for_chat`, `in_plane`,
    // `decide_locally`), its function (`decide::`), its loader (`levels`), its judge (`force`)
    // and the hook's socket (`hookwire`). The oracle reaches only Cedar's authorizer, through
    // `policy::Policy::decide`.
    for name in [
        "Asker",
        "for_chat",
        "in_plane",
        "decide_locally",
        "decide::decide",
        "levels",
        "force::",
        "hookwire",
        "Loaded",
    ] {
        assert!(
            !code.contains(name),
            "the oracle names `{name}`, which is on the host's decision path"
        );
    }
    // And it does reach Cedar's authorizer.
    assert!(code.contains("policy.decide("));
    // The hook's request builder is still where this test thinks it is, so the list above
    // cannot go stale unseen.
    let decide = include_str!("../../src/policy/decide.rs");
    for name in ["pub struct Asker", "pub fn for_chat", "pub fn in_plane"] {
        assert!(
            decide.contains(name),
            "`{name}` has moved: update this test"
        );
    }
}

#[test]
fn an_equality_cell_takes_its_expectation_from_the_oracle() {
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
}
