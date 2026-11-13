// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **There are no grants for merges** (R-117, Q135): a merge ask is never remembered. The store
//! of rules the person made from an answer ([`askrules::Store`]) refuses one for a merge ask,
//! whatever it would allow and for whom, and keeps nothing.

use purlis_core::askrules::{Rule, Scope, Store};
use purlis_core::mergehold;

fn a_rule_for(source: &str) -> Rule {
    Rule {
        id: "rule-1".to_owned(),
        at: 1_000,
        project: "alpha".to_owned(),
        root: "/plane".into(),
        session: 4,
        request: "o/r#12".to_owned(),
        source: source.to_owned(),
        says: "Merge request 12 of o/r".to_owned(),
        label: "Merge".to_owned(),
        option: "merge".to_owned(),
        scope: Scope::Chat,
        grant: Some("merge o/r#12".to_owned()),
        declared: None,
    }
}

#[test]
fn the_grant_store_rejects_a_merge_grant() {
    let data = tempfile::tempdir().expect("a folder");
    let store = Store::in_data(data.path());
    for scope in [Scope::Chat, Scope::Project, Scope::Machine] {
        let made = store.make(Rule {
            scope,
            ..a_rule_for(mergehold::SOURCE)
        });
        assert!(made.is_err(), "{scope:?}: {made:?}");
    }
    assert!(store.read().rules.is_empty(), "{:?}", store.read().rules);
    // Every other source's rule is still kept: the refusal is the merge hold's alone.
    store
        .make(a_rule_for("sandbox-host"))
        .expect("a host's rule is kept");
    assert_eq!(store.read().rules.len(), 1);
}
