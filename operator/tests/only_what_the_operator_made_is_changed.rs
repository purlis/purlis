// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The operator changes only upstream objects it made. What it makes is in a person's runner
//! namespace and controlled by that namespace, by uid (#2055); an upstream claim also names its
//! Claim's uid. An upstream object under the name it would use that is not that is left as it
//! is, and the Claim's `Ready` names it, until it is removed or renamed.

use k8s_openapi::api::core::v1::Namespace;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::OwnerReference;
use kube::api::ObjectMeta;
use purlis_cluster_operator::person::{self, CLAIM_UID_ANNOTATION};
use purlis_cluster_operator::upstream::{self, Found};
use purlis_cluster_operator::{Claim, ClaimSpec, Pool, PoolSpec, status_of_an_object_in_the_way};

const IMAGE: &str = "ghcr.io/purlis/purlis-runner@sha256:1111111111111111111111111111111111111111111111111111111111111111";

fn pool() -> Pool {
    let mut pool = Pool::new(
        "team",
        PoolSpec {
            image: IMAGE.into(),
            kept_volume: Default::default(),
        },
    );
    pool.metadata.uid = Some("pool-uid".into());
    pool
}

fn claim() -> Claim {
    Claim {
        metadata: ObjectMeta {
            name: Some("ada-1".into()),
            namespace: Some("ada".into()),
            uid: Some("claim-uid".into()),
            ..ObjectMeta::default()
        },
        spec: ClaimSpec {
            pool: "team".into(),
            requester: "ada@example.com".into(),
            kept: false,
        },
        status: None,
    }
}

fn runner() -> Namespace {
    let mut runner = person::namespace_for("ada@example.com");
    runner.metadata.uid = Some("runner-uid".into());
    runner
}

fn held(uid: &str, owners: Option<Vec<OwnerReference>>) -> ObjectMeta {
    ObjectMeta {
        name: Some("purlis-pool-team".into()),
        namespace: runner().metadata.name,
        uid: Some(uid.into()),
        owner_references: owners,
        ..ObjectMeta::default()
    }
}

fn namespace_owner(uid: &str, controller: Option<bool>) -> OwnerReference {
    OwnerReference {
        api_version: "v1".into(),
        kind: "Namespace".into(),
        name: person::runner_namespace("ada@example.com"),
        uid: uid.into(),
        controller,
        block_owner_deletion: None,
    }
}

#[test]
fn nothing_under_the_name_is_free_to_make() {
    assert_eq!(upstream::made_for(None, &runner()), Found::Nothing);
    assert_eq!(
        upstream::made_for_claim(None, &runner(), &claim()),
        Found::Nothing
    );
}

#[test]
fn what_the_operator_makes_is_controlled_by_the_runner_namespace_and_blocks_nothing() {
    let owners = upstream::runner_owner(&runner());
    assert_eq!(owners, [namespace_owner("runner-uid", Some(true))]);
    let template = upstream::template_for(&pool(), "ns", owners.clone());
    let made = held("held-uid", template.metadata.owner_references.clone());
    assert_eq!(
        upstream::made_for(Some(&made), &runner()),
        Found::Ours {
            uid: "held-uid".into()
        }
    );
    let upstream_claim = upstream::sandbox_claim_for(&claim(), &Default::default(), &runner());
    assert_eq!(
        upstream_claim.metadata.namespace,
        Some(person::runner_namespace("ada@example.com"))
    );
    assert_eq!(upstream_claim.metadata.owner_references, Some(owners));
    let mut made = held(
        "claimed-uid",
        upstream_claim.metadata.owner_references.clone(),
    );
    made.annotations = upstream_claim.metadata.annotations.clone();
    assert_eq!(
        upstream::made_for_claim(Some(&made), &runner(), &claim()),
        Found::Ours {
            uid: "claimed-uid".into()
        }
    );
}

#[test]
fn an_object_without_the_runner_namespaces_controller_reference_is_in_the_way() {
    let mut claim_owner = namespace_owner("claim-uid", Some(true));
    claim_owner.kind = "Claim".into();
    claim_owner.api_version = "runners.purlis.dev/v1alpha1".into();
    let in_the_way = [
        // Made by hand: no owner at all.
        held("u", None),
        held("u", Some(vec![])),
        // The namespace's reference, but not as its controller.
        held("u", Some(vec![namespace_owner("runner-uid", None)])),
        held("u", Some(vec![namespace_owner("runner-uid", Some(false))])),
        // A controller of the same name and kind that is another namespace: another uid.
        held("u", Some(vec![namespace_owner("another-uid", Some(true))])),
        // The Claim's own uid as another kind: an owner cannot cross namespaces.
        held("u", Some(vec![claim_owner])),
    ];
    for object in &in_the_way {
        assert_eq!(
            upstream::made_for(Some(object), &runner()),
            Found::InTheWay,
            "{object:?}"
        );
    }
}

#[test]
fn an_upstream_claim_is_this_claims_only_when_it_names_its_uid() {
    let owners = Some(upstream::runner_owner(&runner()));
    let named = |uid: Option<&str>| {
        let mut made = held("u", owners.clone());
        made.annotations =
            uid.map(|uid| [(CLAIM_UID_ANNOTATION.to_owned(), uid.to_owned())].into());
        made
    };
    // Another Claim's of the same name, from another of the person's namespaces, or one that
    // names none: in the way.
    for made in [named(Some("another-claim-uid")), named(None)] {
        assert_eq!(
            upstream::made_for_claim(Some(&made), &runner(), &claim()),
            Found::InTheWay,
            "{made:?}"
        );
    }
    // Its uid, but not controlled by the runner namespace: in the way.
    let mut forged = named(Some("claim-uid"));
    forged.owner_references = None;
    assert_eq!(
        upstream::made_for_claim(Some(&forged), &runner(), &claim()),
        Found::InTheWay
    );
}

#[test]
fn a_claim_an_object_keeps_changing_under_waits_longer_each_time() {
    use purlis_cluster_operator::retry_after_conflict;
    use std::time::Duration;
    let waits: Vec<u64> = (1..=6)
        .map(|attempt| retry_after_conflict(attempt).as_secs())
        .collect();
    assert_eq!(waits, [1, 2, 4, 8, 10, 10]);
    assert_eq!(retry_after_conflict(u32::MAX), Duration::from_secs(10));
}

#[test]
fn an_owner_without_a_uid_owns_nothing() {
    let mut runner = runner();
    runner.metadata.uid = None;
    let made = held("u", Some(vec![namespace_owner("", Some(true))]));
    assert_eq!(upstream::made_for(Some(&made), &runner), Found::InTheWay);
    assert!(upstream::runner_owner(&runner).is_empty());
}

#[test]
fn a_claim_names_the_object_in_its_way_where_it_is_and_what_frees_it() {
    let status = status_of_an_object_in_the_way(
        "SandboxTemplate",
        "purlis-pool-team",
        "purlis-runners-0123456789abcdef",
    );
    let ready = &status.conditions[0];
    assert_eq!(ready.type_, "Ready");
    assert_eq!(ready.status, "False");
    assert_eq!(ready.reason, "UpstreamObjectInTheWay");
    for words in [
        "SandboxTemplate purlis-pool-team",
        "purlis-runners-0123456789abcdef",
        "Remove or rename it",
    ] {
        assert!(ready.message.contains(words), "{}", ready.message);
    }
}

#[test]
fn the_pool_is_not_the_owner_of_what_the_operator_makes_for_it() {
    // A pool is cluster-wide and may be deleted while its runners run: it never controls them.
    let template = upstream::template_for(&pool(), "ns", upstream::runner_owner(&runner()));
    let made = held("u", template.metadata.owner_references.clone());
    assert_eq!(upstream::made_for(Some(&made), &pool()), Found::InTheWay);
}
