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
            requester: "ada@example.com".into(),
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
    assert_eq!(upstream::made_for(None, &pool()), Found::Nothing);
}

#[test]
fn an_object_whose_controller_is_its_owner_is_the_operators_by_its_uid() {
    let made = held("held-uid", Some(vec![owner("pool-uid", Some(true))]));
    assert_eq!(
        upstream::made_for(Some(&made), &pool()),
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
        upstream::made_for(Some(&made), &claim()),
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
            upstream::made_for(Some(object), &pool()),
            Found::InTheWay,
            "{object:?}"
        );
    }
    // A pool's template is never taken for a claim's upstream claim.
    let template = upstream::template_for(
        &pool(),
        "ada",
        upstream::owners_with(None, &claim(), &[&claim()]),
    );
    let made = held("u", template.metadata.owner_references.clone());
    assert_eq!(upstream::made_for(Some(&made), &claim()), Found::InTheWay);
}

fn claim_owner(api_version: &str) -> OwnerReference {
    OwnerReference {
        api_version: api_version.into(),
        kind: "Claim".into(),
        name: "ada-1".into(),
        uid: "claim-uid".into(),
        controller: None,
        block_owner_deletion: None,
    }
}

fn on_pool(name: &str, uid: &str, pool: &str) -> Claim {
    let mut claim = claim();
    claim.metadata.name = Some(name.into());
    claim.metadata.uid = Some(uid.into());
    claim.spec.pool = pool.into();
    claim
}

#[test]
fn a_pools_template_or_warm_pool_is_the_operators_when_a_claim_on_the_pool_there_owns_it() {
    let known = [&claim()];
    let template = upstream::template_for(
        &pool(),
        "ada",
        upstream::owners_with(None, &claim(), &known),
    );
    let made = held("held-uid", template.metadata.owner_references.clone());
    assert_eq!(
        upstream::made_for_pool(Some(&made), &pool(), &known),
        Found::Ours {
            uid: "held-uid".into()
        }
    );
    // One owner the operator knows is enough, beside one that is gone.
    let mut gone = claim_owner("runners.purlis.dev/v1alpha1");
    gone.name = "ada-0".into();
    gone.uid = "gone-uid".into();
    let made = held(
        "held-uid",
        Some(vec![gone, claim_owner("runners.purlis.dev/v1alpha1")]),
    );
    assert_eq!(
        upstream::made_for_pool(Some(&made), &pool(), &known),
        Found::Ours {
            uid: "held-uid".into()
        }
    );
    // One the pool itself controlled, as the operator made them before its claims owned them.
    let made = held("held-uid", Some(vec![owner("pool-uid", Some(true))]));
    assert_eq!(
        upstream::made_for_pool(Some(&made), &pool(), &known),
        Found::Ours {
            uid: "held-uid".into()
        }
    );
    assert_eq!(
        upstream::made_for_pool(None, &pool(), &known),
        Found::Nothing
    );
}

#[test]
fn a_pools_template_or_warm_pool_owned_by_no_claim_the_operator_knows_there_is_in_the_way() {
    let mut elsewhere = claim();
    elsewhere.metadata.namespace = Some("bea".into());
    let known_claims = [
        claim(),
        // The same name in another namespace, and a Claim of another pool.
        elsewhere,
        on_pool("ada-2", "other-pool-claim-uid", "other"),
    ];
    let known: Vec<&Claim> = known_claims.iter().collect();
    let mut unknown_uid = claim_owner("runners.purlis.dev/v1alpha1");
    unknown_uid.uid = "unknown-uid".into();
    let mut another_pool = claim_owner("runners.purlis.dev/v1alpha1");
    another_pool.name = "ada-2".into();
    another_pool.uid = "other-pool-claim-uid".into();
    let mut renamed = claim_owner("runners.purlis.dev/v1alpha1");
    renamed.name = "ada-9".into();
    let in_the_way = [
        held("u", None),
        held("u", Some(vec![])),
        held("u", Some(vec![owner("pool-uid", None)])),
        held("u", Some(vec![owner("another-uid", Some(true))])),
        // A Claim of another API group is not purlis's.
        held("u", Some(vec![claim_owner("example.com/v1")])),
        // A purlis Claim reference to no Claim the operator knows: an unknown uid, a known uid
        // under another name, and a Claim on another pool.
        held("u", Some(vec![unknown_uid])),
        held("u", Some(vec![renamed])),
        held("u", Some(vec![another_pool])),
    ];
    for object in &in_the_way {
        assert_eq!(
            upstream::made_for_pool(Some(object), &pool(), &known),
            Found::InTheWay,
            "{object:?}"
        );
    }
    // The namespace's own Claim, read for an object in another namespace.
    let mut in_bea = held("u", Some(vec![claim_owner("runners.purlis.dev/v1alpha1")]));
    in_bea.namespace = Some("bea".into());
    assert_eq!(
        upstream::made_for_pool(Some(&in_bea), &pool(), &[&claim()]),
        Found::InTheWay
    );
}

#[test]
fn only_claims_the_operator_knows_stay_owners() {
    let known_claims = [claim(), on_pool("ada-2", "claim-2-uid", "team")];
    let known: Vec<&Claim> = known_claims.iter().collect();
    let mut second = claim_owner("runners.purlis.dev/v1alpha1");
    second.name = "ada-2".into();
    second.uid = "claim-2-uid".into();
    let mut forged = claim_owner("runners.purlis.dev/v1alpha1");
    forged.name = "ada-3".into();
    forged.uid = "forged-uid".into();
    let made = held("u", Some(vec![second, forged]));
    let owners = upstream::owners_with(Some(&made), &claim(), &known);
    let named: Vec<&str> = owners.iter().map(|owner| owner.uid.as_str()).collect();
    assert_eq!(named, ["claim-uid", "claim-2-uid"]);
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
    let mut pool = pool();
    pool.metadata.uid = None;
    let made = held("u", Some(vec![owner("", Some(true))]));
    assert_eq!(upstream::made_for(Some(&made), &pool), Found::InTheWay);
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

#[test]
fn only_claims_the_operator_acts_on_vouch_and_every_claim_on_the_pool_there_stays_an_owner() {
    use k8s_openapi::api::admissionregistration::v1::{
        ValidatingAdmissionPolicyBinding, ValidatingAdmissionPolicyBindingSpec,
    };
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;
    use purlis_cluster_operator::{POLICY, claims_kept_as_owners, claims_that_vouch};
    use std::sync::Arc;
    let at = |time: &str| Time(time.parse().expect("a timestamp"));
    let binding = ValidatingAdmissionPolicyBinding {
        metadata: ObjectMeta {
            name: Some(POLICY.into()),
            creation_timestamp: Some(at("2026-10-17T10:00:00Z")),
            ..ObjectMeta::default()
        },
        spec: ValidatingAdmissionPolicyBindingSpec {
            policy_name: POLICY.into(),
            validation_actions: vec!["Deny".into()],
            ..ValidatingAdmissionPolicyBindingSpec::default()
        },
    };
    let made = |name: &str, uid: &str, pool: &str, namespace: &str, time: &str| {
        let mut claim = on_pool(name, uid, pool);
        claim.metadata.namespace = Some(namespace.into());
        claim.metadata.creation_timestamp = Some(at(time));
        Arc::new(claim)
    };
    let this = made("ada-1", "claim-uid", "team", "ada", "2026-10-17T11:00:00Z");
    let watched = vec![
        this.clone(),
        made("ada-2", "under-uid", "team", "ada", "2026-10-17T11:00:00Z"),
        // Made before the policy was in force: refused, so never an owner.
        made(
            "ada-3",
            "refused-uid",
            "team",
            "ada",
            "2026-10-17T09:00:00Z",
        ),
        made(
            "ada-4",
            "other-pool-uid",
            "other",
            "ada",
            "2026-10-17T11:00:00Z",
        ),
        made(
            "bea-1",
            "elsewhere-uid",
            "team",
            "bea",
            "2026-10-17T11:00:00Z",
        ),
    ];
    let mut uids: Vec<String> = claims_that_vouch(watched.clone(), &this, Some(&binding))
        .iter()
        .map(|claim| claim.metadata.uid.clone().unwrap_or_default())
        .collect();
    uids.sort();
    assert_eq!(uids, ["claim-uid", "under-uid"]);
    // With no binding in force, only the claim at hand, which reconcile acts on only under it.
    let alone: Vec<_> = claims_that_vouch(vec![this.clone()], &this, None)
        .iter()
        .map(|claim| claim.metadata.uid.clone().unwrap_or_default())
        .collect();
    assert_eq!(alone, ["claim-uid"]);
    // Once an object is the operator's, a refused Claim on the pool there stays among its
    // owners: its runner may still run, and the object and what upstream makes of it with it.
    let mut kept: Vec<String> = claims_kept_as_owners(watched, &this)
        .iter()
        .map(|claim| claim.metadata.uid.clone().unwrap_or_default())
        .collect();
    kept.sort();
    assert_eq!(kept, ["claim-uid", "refused-uid", "under-uid"]);
}
