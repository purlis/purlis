// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The operator changes only upstream objects it made. An upstream object under the name it
//! would use that does not name the operator's owner as its controller is left as it is, and the
//! Claim's `Ready` names it, until it is removed or renamed.

use k8s_openapi::apimachinery::pkg::apis::meta::v1::OwnerReference;
use kube::api::ObjectMeta;
use purlis_cluster_operator::upstream::{self, Found};
use purlis_cluster_operator::{Claim, ClaimSpec, Pool, PoolSpec, status_of_an_object_in_the_way};

const IMAGE: &str = "ghcr.io/purlis/purlis-runner@sha256:1111111111111111111111111111111111111111111111111111111111111111";

fn pool() -> Pool {
    let mut pool = Pool::new(
        "team",
        PoolSpec {
            image: IMAGE.into(),
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
        },
        status: None,
    }
}

fn held(uid: &str, owners: Option<Vec<OwnerReference>>) -> ObjectMeta {
    ObjectMeta {
        name: Some("purlis-pool-team".into()),
        namespace: Some("ada".into()),
        uid: Some(uid.into()),
        owner_references: owners,
        ..ObjectMeta::default()
    }
}

fn owner(uid: &str, controller: Option<bool>) -> OwnerReference {
    OwnerReference {
        api_version: "runners.purlis.dev/v1alpha1".into(),
        kind: "Pool".into(),
        name: "team".into(),
        uid: uid.into(),
        controller,
        block_owner_deletion: None,
    }
}

#[test]
fn nothing_under_the_name_is_free_to_make() {
    assert_eq!(upstream::found(None, &pool()), Found::Nothing);
}

#[test]
fn an_object_whose_controller_is_its_owner_is_the_operators_by_its_uid() {
    let template = upstream::template_for(&pool(), "ada");
    let made = held("held-uid", template.metadata.owner_references.clone());
    assert_eq!(
        upstream::found(Some(&made), &pool()),
        Found::Ours {
            uid: "held-uid".into()
        }
    );
    let upstream_claim = upstream::sandbox_claim_for(&claim());
    let made = held(
        "claimed-uid",
        upstream_claim.metadata.owner_references.clone(),
    );
    assert_eq!(
        upstream::found(Some(&made), &claim()),
        Found::Ours {
            uid: "claimed-uid".into()
        }
    );
}

#[test]
fn an_object_without_the_operators_controller_reference_is_in_the_way() {
    let in_the_way = [
        // Made by hand: no owner at all.
        held("u", None),
        held("u", Some(vec![])),
        // The owner's reference, but not as its controller.
        held("u", Some(vec![owner("pool-uid", None)])),
        held("u", Some(vec![owner("pool-uid", Some(false))])),
        // A controller of the same name and kind that is another object: another pool's uid.
        held("u", Some(vec![owner("another-uid", Some(true))])),
    ];
    for object in &in_the_way {
        assert_eq!(
            upstream::found(Some(object), &pool()),
            Found::InTheWay,
            "{object:?}"
        );
    }
    // A pool's template is never taken for a claim's, nor a claim's for a pool's.
    let template = upstream::template_for(&pool(), "ada");
    let made = held("u", template.metadata.owner_references.clone());
    assert_eq!(upstream::found(Some(&made), &claim()), Found::InTheWay);
}

#[test]
fn an_owner_without_a_uid_owns_nothing() {
    let mut pool = pool();
    pool.metadata.uid = None;
    let made = held("u", Some(vec![owner("", Some(true))]));
    assert_eq!(upstream::found(Some(&made), &pool), Found::InTheWay);
}

#[test]
fn a_claim_names_the_object_in_its_way_and_what_frees_it() {
    let status = status_of_an_object_in_the_way("SandboxTemplate", "purlis-pool-team");
    let ready = &status.conditions[0];
    assert_eq!(ready.type_, "Ready");
    assert_eq!(ready.status, "False");
    assert_eq!(ready.reason, "UpstreamObjectInTheWay");
    assert!(
        ready.message.contains("SandboxTemplate purlis-pool-team"),
        "{}",
        ready.message
    );
    assert!(
        ready.message.contains("Remove or rename it"),
        "{}",
        ready.message
    );
}
