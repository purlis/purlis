// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The runner tests have an in-memory link and a stand-in pool** (R-701, R-702, T1a-50).
//!
//! The two seams set (iii)'s runner rows are tested through: a desktop end and a runner end
//! joined in memory, which records every message that crossed and can be dropped and linked
//! again ([`crate::support::runner_link`]); and a pool with one switch per ownership outcome,
//! judged by the check that already decides whether a runner is the person's own
//! ([`crate::support::stand_in_pool`]).

#![cfg(unix)]

use crate::support::runner_link::{Crossed, End, InMemoryLink, LinkDown};
use crate::support::stand_in_pool::{Outcome, StandInPool, Switch};

/// A message shaped as the key-proxy plan T1a-51 sends with a run's start.
fn plan(run: &str) -> serde_json::Value {
    serde_json::json!({
        "key_proxy_plan": {
            "run": run,
            "route": "anthropic",
            "key": "a stand-in key",
        }
    })
}

#[test]
fn in_memory_link_records_each_message_that_crossed_and_survives_a_drop_and_relink() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let mut link = InMemoryLink::<serde_json::Value>::up();

        link.send(End::Desktop, plan("first")).await.unwrap();
        assert_eq!(link.receive(End::Runner).await, Some(plan("first")));

        link.drop_link();
        assert_eq!(
            link.send(End::Desktop, plan("while down")).await,
            Err(LinkDown),
            "a send on a dropped link fails with the link's down error"
        );
        assert_eq!(link.receive(End::Runner).await, None, "nothing reaches it");

        link.relink();
        link.send(End::Desktop, plan("second")).await.unwrap();
        assert_eq!(link.receive(End::Runner).await, Some(plan("second")));

        assert_eq!(
            link.crossed(),
            vec![
                Crossed {
                    from: End::Desktop,
                    message: plan("first"),
                },
                Crossed {
                    from: End::Desktop,
                    message: plan("second"),
                },
            ],
            "exactly the two delivered messages, in order"
        );
        assert_eq!(link.times_crossed(|m| *m == plan("first")), 1);
        assert_eq!(link.times_crossed(|m| *m == plan("while down")), 0);

        // The runner end answers over the same link.
        link.send(End::Runner, serde_json::json!({ "allowed": true }))
            .await
            .unwrap();
        assert_eq!(
            link.receive(End::Desktop).await,
            Some(serde_json::json!({ "allowed": true }))
        );
        assert_eq!(link.crossed().last().unwrap().from, End::Runner);
    });
}

#[test]
fn stand_in_pool_yields_each_ownership_outcome_as_the_built_check_judges_it() {
    for (switch, expected) in [
        (Switch::OwnedDocker, Outcome::Owned),
        (Switch::MarkedCluster, Outcome::Marked),
        (Switch::AnyOther, Outcome::NotOwned),
    ] {
        let pool = StandInPool::new(switch);
        assert_eq!(pool.judged(), expected, "{switch:?}");
    }

    // Each identity field, each claim field, the protocol version and the team-image flag is
    // set on its own: a caller reading the pool sees that one field changed, and no other.
    let was = StandInPool::new(Switch::MarkedCluster);
    let changes: [(&str, fn(&mut StandInPool)); 11] = [
        ("pool name", |p| p.identity.pool = "elsewhere".into()),
        ("context", |p| p.identity.context = "another".into()),
        ("cluster uid", |p| {
            p.identity.cluster_uid = "4e9a4b2c-7d10-4f5e-8a6b-9c0d1e2f3a4b".into();
        }),
        ("operator key", |p| p.identity.operator_key = [9; 32]),
        ("socket", |p| p.identity.socket = "/tmp/another.sock".into()),
        ("claim uid", |p| p.claim.claim_uid = "another".into()),
        ("pod", |p| p.claim.pod = "another".into()),
        ("device", |p| p.claim.device = "01JB0ANOTHER00000000000000".into()),
        ("link key", |p| p.claim.link_key = [7; 32]),
        ("protocol", |p| p.protocol = (1, 0)),
        ("team image", |p| p.team_image = !p.team_image),
    ];
    for (field, change) in changes {
        let mut pool = was.clone();
        change(&mut pool);
        assert_ne!(pool, was, "{field} changed");
        let differing = pool.fields_differing_from(&was);
        assert_eq!(differing, vec![field], "only {field} changed");
        // A marked cluster pool changed in any one field is still judged by the same check.
        assert_eq!(pool.judged(), Outcome::Marked, "{field}");
    }
}
