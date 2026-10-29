// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A pool's template and warm pool in a person's runner namespace go with the last of their
//! Claims on that pool, by its finalizer (#2055), and a Claim made at that moment is still
//! served: whatever happens to either object, every Claim of that person on that pool is looked
//! at again, so one that still needs it makes it again. An upstream claim wakes the Claims of
//! its name in its runner namespace.

use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;
use kube::api::ObjectMeta;
use kube::runtime::reflector::ObjectRef;
use purlis_cluster_operator::person::{CLAIM_UID_ANNOTATION, runner_namespace};
use purlis_cluster_operator::{
    Claim, ClaimSpec, claims_of_upstream, claims_woken_by, pool_objects_go_with, upstream,
};
use std::sync::Arc;

fn claim_by(requester: &str, namespace: &str, name: &str, pool: &str) -> Arc<Claim> {
    Arc::new(Claim {
        metadata: ObjectMeta {
            name: Some(name.into()),
            namespace: Some(namespace.into()),
            uid: Some(format!("uid-{namespace}-{name}")),
            ..ObjectMeta::default()
        },
        spec: ClaimSpec {
            pool: pool.into(),
            requester: requester.into(),
            kept: false,
        },
        status: None,
    })
}

fn claim(namespace: &str, name: &str, pool: &str) -> Arc<Claim> {
    claim_by("ada@example.com", namespace, name, pool)
}

fn object(namespace: &str, name: &str) -> ObjectMeta {
    ObjectMeta {
        name: Some(name.into()),
        namespace: Some(namespace.into()),
        ..ObjectMeta::default()
    }
}

#[test]
fn a_pools_object_wakes_every_claim_of_its_person_on_that_pool_and_no_other() {
    let claims = [
        claim("ada", "ada-1", "team"),
        claim("work", "ada-2", "team"),
        claim("ada", "ada-3", "other"),
        claim_by("bea@example.com", "bea", "bea-1", "team"),
    ];
    let runner = runner_namespace("ada@example.com");
    let mut woken = claims_woken_by(
        &object(&runner, &upstream::name_for("team")),
        claims.clone(),
    );
    woken.sort_by_key(|claim| claim.name.clone());
    assert_eq!(
        woken,
        [
            ObjectRef::<Claim>::new("ada-1").within("ada"),
            ObjectRef::<Claim>::new("ada-2").within("work"),
        ]
    );
    // The same name in a Claim's own namespace is no runner namespace's: it wakes nothing.
    assert!(claims_woken_by(&object("ada", &upstream::name_for("team")), claims).is_empty());
}

#[test]
fn an_object_not_named_for_a_pool_wakes_nothing() {
    let claims = [claim("ada", "ada-1", "team")];
    let runner = runner_namespace("ada@example.com");
    for name in ["team", "purlis-pool-", "pool-team", "purlis-pool-other"] {
        assert!(
            claims_woken_by(&object(&runner, name), claims.clone()).is_empty(),
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

fn upstream_claim(namespace: &str, name: &str, uid: Option<&str>) -> ObjectMeta {
    ObjectMeta {
        annotations: uid.map(|uid| [(CLAIM_UID_ANNOTATION.to_owned(), uid.to_owned())].into()),
        ..object(namespace, name)
    }
}

#[test]
fn an_upstream_claim_wakes_the_claims_of_its_name_in_its_runner_namespace() {
    let claims = [
        claim("ada", "ada-1", "team"),
        // The same name in another of Ada's namespaces: the one it may be in the way of.
        claim("work", "ada-1", "team"),
        claim("ada", "ada-2", "team"),
        claim_by("bea@example.com", "bea", "ada-1", "team"),
    ];
    let runner = runner_namespace("ada@example.com");
    let mut woken = claims_of_upstream(&upstream_claim(&runner, "ada-1", None), claims.clone());
    woken.sort_by_key(|claim| claim.namespace.clone());
    assert_eq!(
        woken,
        [
            ObjectRef::<Claim>::new("ada-1").within("ada"),
            ObjectRef::<Claim>::new("ada-1").within("work"),
        ]
    );
    // In a Claim's own namespace, as an operator before runner namespaces made it: none.
    assert!(claims_of_upstream(&upstream_claim("ada", "ada-1", None), claims.clone()).is_empty());
    assert!(claims_of_upstream(&ObjectMeta::default(), claims).is_empty());
}

fn deleting(claim: Arc<Claim>) -> Arc<Claim> {
    let mut claim = (*claim).clone();
    claim.metadata.deletion_timestamp = Some(Time(k8s_openapi::jiff::Timestamp::UNIX_EPOCH));
    Arc::new(claim)
}

#[test]
fn a_pools_objects_go_with_the_last_claim_of_its_person_on_it() {
    let gone = claim("ada", "ada-1", "team");
    assert!(pool_objects_go_with(&gone, [gone.clone()]));
    // Another Claim of the person on the pool, in any namespace, keeps them.
    assert!(!pool_objects_go_with(
        &gone,
        [gone.clone(), claim("work", "ada-2", "team")]
    ));
    // Another pool's Claim, or another person's, does not.
    assert!(pool_objects_go_with(
        &gone,
        [
            gone.clone(),
            claim("ada", "ada-3", "other"),
            claim_by("bea@example.com", "ada", "bea-1", "team"),
        ]
    ));
    // Two deleted at once both remove them, so neither leaves them behind.
    assert!(pool_objects_go_with(
        &gone,
        [gone.clone(), deleting(claim("ada", "ada-2", "team"))]
    ));
}
