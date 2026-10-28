// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A pool's template and warm pool in a namespace go with the last Claim there on that pool, and
//! a Claim made at that moment is still served: whatever happens to either object, every Claim
//! on that pool in that namespace is looked at again, so one that still needs it makes it again.

use kube::api::ObjectMeta;
use kube::runtime::reflector::ObjectRef;
use purlis_cluster_operator::{Claim, ClaimSpec, claims_woken_by, upstream};
use std::sync::Arc;

fn claim(namespace: &str, name: &str, pool: &str) -> Arc<Claim> {
    Arc::new(Claim {
        metadata: ObjectMeta {
            name: Some(name.into()),
            namespace: Some(namespace.into()),
            ..ObjectMeta::default()
        },
        spec: ClaimSpec {
            pool: pool.into(),
            requester: "ada@example.com".into(),
            kept: false,
            stopped: false,
        },
        status: None,
    })
}

fn object(namespace: &str, name: &str) -> ObjectMeta {
    ObjectMeta {
        name: Some(name.into()),
        namespace: Some(namespace.into()),
        ..ObjectMeta::default()
    }
}

#[test]
fn a_pools_object_wakes_every_claim_on_that_pool_in_its_namespace_and_no_other() {
    let claims = [
        claim("ada", "ada-1", "team"),
        claim("ada", "ada-2", "team"),
        claim("ada", "ada-3", "other"),
        claim("bea", "bea-1", "team"),
    ];
    let mut woken = claims_woken_by(&object("ada", &upstream::name_for("team")), claims.clone());
    woken.sort_by_key(|claim| claim.name.clone());
    assert_eq!(
        woken,
        [
            ObjectRef::<Claim>::new("ada-1").within("ada"),
            ObjectRef::<Claim>::new("ada-2").within("ada"),
        ]
    );
}

#[test]
fn an_object_not_named_for_a_pool_wakes_nothing() {
    let claims = [claim("ada", "ada-1", "team")];
    for name in ["team", "purlis-pool-", "pool-team", "purlis-pool-other"] {
        assert!(
            claims_woken_by(&object("ada", name), claims.clone()).is_empty(),
            "{name}"
        );
    }
    assert!(claims_woken_by(&ObjectMeta::default(), claims.clone()).is_empty());
}

#[test]
fn a_pools_name_is_read_back_from_its_objects_name() {
    assert_eq!(upstream::pool_of(&upstream::name_for("team")), Some("team"));
    assert_eq!(upstream::pool_of("purlis-pool-"), None);
    assert_eq!(upstream::pool_of("team"), None);
}
