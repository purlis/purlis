// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A Claim whose pool's template or warm pool belongs only to Claims the operator now refuses
//! (made before the policy's binding was made again) is told so, with a reason of its own and
//! what to do. A Claim blocked by an object the operator never made keeps
//! `UpstreamObjectInTheWay`.

use k8s_openapi::apimachinery::pkg::apis::meta::v1::OwnerReference;
use kube::api::ObjectMeta;
use purlis_cluster_operator::upstream;
use purlis_cluster_operator::{
    Claim, ClaimSpec, Pool, PoolSpec, status_of_an_object_in_the_way,
    status_of_objects_held_by_refused_claims,
};

fn pool() -> Pool {
    let mut pool = Pool::new(
        "team",
        PoolSpec {
            image: format!("ghcr.io/purlis/purlis-runner@sha256:{}", "1".repeat(64)),
            kept_volume: Default::default(),
            stopped: false,
        },
    );
    pool.metadata.uid = Some("pool-uid".into());
    pool
}

fn claim(name: &str, uid: &str, pool: &str) -> Claim {
    Claim {
        metadata: ObjectMeta {
            name: Some(name.into()),
            namespace: Some("ada".into()),
            uid: Some(uid.into()),
            ..ObjectMeta::default()
        },
        spec: ClaimSpec {
            pool: pool.into(),
            requester: "ada@example.com".into(),
            kept: false,
            stopped: false,
        },
        status: None,
    }
}

fn owned_by(owners: &[(&str, &str)]) -> ObjectMeta {
    ObjectMeta {
        name: Some("purlis-pool-team".into()),
        namespace: Some("ada".into()),
        uid: Some("held-uid".into()),
        owner_references: Some(
            owners
                .iter()
                .map(|(name, uid)| OwnerReference {
                    api_version: "runners.purlis.dev/v1alpha1".into(),
                    kind: "Claim".into(),
                    name: (*name).into(),
                    uid: (*uid).into(),
                    controller: None,
                    block_owner_deletion: None,
                })
                .collect(),
        ),
        ..ObjectMeta::default()
    }
}

#[test]
fn an_object_owned_by_claims_the_operator_knows_on_the_pool_names_them() {
    let refused = [claim("ada-3", "u3", "team"), claim("ada-1", "u1", "team")];
    let kept: Vec<&Claim> = refused.iter().collect();
    let held = owned_by(&[("ada-3", "u3"), ("ada-1", "u1")]);
    assert_eq!(
        upstream::claims_holding(&held, &pool(), &kept),
        ["ada-1", "ada-3"]
    );
}

#[test]
fn an_object_with_any_other_owner_is_held_by_none_since_deleting_them_would_not_free_it() {
    let refused = [claim("ada-3", "u3", "team"), claim("ada-1", "u1", "team")];
    let kept: Vec<&Claim> = refused.iter().collect();
    // A Claim reference the operator does not know beside them.
    let held = owned_by(&[("ada-3", "u3"), ("ada-1", "u1"), ("forged", "u9")]);
    assert!(upstream::claims_holding(&held, &pool(), &kept).is_empty());
    // An owner of another kind beside them.
    let mut held = owned_by(&[("ada-1", "u1")]);
    held.owner_references
        .get_or_insert_with(Vec::new)
        .push(OwnerReference {
            api_version: "v1".into(),
            kind: "ConfigMap".into(),
            name: "someone".into(),
            uid: "cm-uid".into(),
            controller: None,
            block_owner_deletion: None,
        });
    assert!(upstream::claims_holding(&held, &pool(), &kept).is_empty());
}

#[test]
fn an_object_owned_by_no_claim_the_operator_knows_is_held_by_none() {
    let kept_claims = [claim("ada-4", "u4", "other")];
    let kept: Vec<&Claim> = kept_claims.iter().collect();
    for held in [
        owned_by(&[]),
        owned_by(&[("ada-9", "u9")]),
        // A Claim on another pool, and one named right with another uid.
        owned_by(&[("ada-4", "u4")]),
        owned_by(&[("ada-4", "u5")]),
    ] {
        assert!(
            upstream::claims_holding(&held, &pool(), &kept).is_empty(),
            "{held:?}"
        );
    }
}

#[test]
fn a_claim_blocked_by_claims_made_before_the_policy_says_so_and_what_frees_it() {
    let status = status_of_objects_held_by_refused_claims(
        "SandboxTemplate",
        "purlis-pool-team",
        &["ada-1".to_owned(), "ada-3".to_owned()],
    );
    let ready = &status.conditions[0];
    assert_eq!(ready.type_, "Ready");
    assert_eq!(ready.status, "False");
    assert_eq!(ready.reason, "HeldByClaimsMadeBeforePolicy");
    let message = &ready.message;
    assert!(
        message.contains("SandboxTemplate purlis-pool-team"),
        "{message}"
    );
    assert!(message.contains("ada-1, ada-3"), "{message}");
    assert!(message.contains("made before the policy"), "{message}");
    assert!(message.contains("Delete them"), "{message}");
    assert!(!message.contains("not made by"), "{message}");
}

#[test]
fn a_claim_blocked_by_an_object_the_operator_never_made_keeps_its_reason_and_words() {
    let status = status_of_an_object_in_the_way("SandboxTemplate", "purlis-pool-team");
    let ready = &status.conditions[0];
    assert_eq!(ready.reason, "UpstreamObjectInTheWay");
    assert!(
        ready
            .message
            .contains("was not made by purlis's cluster operator"),
        "{}",
        ready.message
    );
}
