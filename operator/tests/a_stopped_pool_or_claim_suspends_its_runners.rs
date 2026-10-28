// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **`stopped` suspends, and never destroys** (#2058, spec #2041): a Pool or a Claim set
//! `stopped` from kubectl has the operator put each of its runners' sandboxes in upstream's
//! `Suspended` mode, which ends the pod and keeps a kept Claim's volume; set back, the operator
//! runs again only the runners it suspended itself.

use k8s_openapi::apimachinery::pkg::apis::meta::v1::{FieldsV1, ManagedFieldsEntry};
use kube::api::ObjectMeta;
use purlis_cluster_operator::upstream::{self, Sandbox, SandboxSpec};
use purlis_cluster_operator::{
    Claim, ClaimSpec, FIELD_MANAGER, Pool, PoolSpec, status_of_a_stopped_claim,
};
use serde_json::json;

const IMAGE: &str = "ghcr.io/purlis/purlis-runner@sha256:1111111111111111111111111111111111111111111111111111111111111111";

fn pool(stopped: bool) -> Pool {
    Pool::new(
        "team",
        PoolSpec {
            image: IMAGE.into(),
            kept_volume: Default::default(),
            stopped,
        },
    )
}

fn claim(stopped: bool) -> Claim {
    let mut claim = Claim::new(
        "ada-1",
        ClaimSpec {
            pool: "team".into(),
            requester: "ada@example.com".into(),
            kept: true,
            stopped,
        },
    );
    claim.metadata.namespace = Some("ada".into());
    claim
}

/// A sandbox in `mode`, whose `operatingMode` is owned by `manager`, if any.
fn sandbox(mode: Option<&str>, manager: Option<&str>) -> Sandbox {
    Sandbox {
        metadata: ObjectMeta {
            name: Some("ada-1".into()),
            namespace: Some("ada".into()),
            managed_fields: manager.map(|manager| {
                vec![ManagedFieldsEntry {
                    manager: Some(manager.into()),
                    operation: Some("Apply".into()),
                    fields_v1: Some(FieldsV1(json!({ "f:spec": { "f:operatingMode": {} } }))),
                    ..ManagedFieldsEntry::default()
                }]
            }),
            ..ObjectMeta::default()
        },
        spec: SandboxSpec {
            operating_mode: mode.map(str::to_owned),
        },
    }
}

#[test]
fn a_stopped_claim_or_pool_has_its_running_runner_suspended() {
    let running = sandbox(Some("Running"), None);
    for (claim, pool) in [(true, false), (false, true), (true, true)] {
        assert_eq!(
            upstream::operating_mode_for(&self::claim(claim), &self::pool(pool), &running),
            Some("Suspended"),
            "claim stopped {claim}, pool stopped {pool}"
        );
    }
    // A sandbox upstream defaulted, which names no mode, runs.
    assert_eq!(
        upstream::operating_mode_for(&claim(true), &pool(false), &sandbox(None, None)),
        Some("Suspended")
    );
    // One already suspended is left as it is.
    assert_eq!(
        upstream::operating_mode_for(
            &claim(true),
            &pool(false),
            &sandbox(Some("Suspended"), Some(FIELD_MANAGER))
        ),
        None
    );
}

#[test]
fn a_runner_runs_again_once_neither_is_stopped_only_where_the_operator_suspended_it() {
    let (claim, pool) = (claim(false), pool(false));
    assert_eq!(
        upstream::operating_mode_for(
            &claim,
            &pool,
            &sandbox(Some("Suspended"), Some(FIELD_MANAGER))
        ),
        Some("Running")
    );
    // Suspended by someone else, by hand: the operator leaves it suspended.
    assert_eq!(
        upstream::operating_mode_for(
            &claim,
            &pool,
            &sandbox(Some("Suspended"), Some("kubectl-patch"))
        ),
        None
    );
    assert_eq!(
        upstream::operating_mode_for(&claim, &pool, &sandbox(Some("Suspended"), None)),
        None
    );
    // Running, it is left running.
    assert_eq!(
        upstream::operating_mode_for(
            &claim,
            &pool,
            &sandbox(Some("Running"), Some(FIELD_MANAGER))
        ),
        None
    );
}

#[test]
fn the_operator_writes_only_the_sandboxs_mode() {
    let written = serde_json::to_value(upstream::sandbox_in_mode("ada-1", "ada", "Suspended"))
        .expect("serialises");
    assert_eq!(
        written,
        json!({
            "apiVersion": "agents.x-k8s.io/v1beta1",
            "kind": "Sandbox",
            "metadata": { "name": "ada-1", "namespace": "ada" },
            "spec": { "operatingMode": "Suspended" },
        })
    );
}

#[test]
fn a_stopped_claim_says_so_and_no_key_is_vouched_for() {
    let said = |claim: &Claim, pool: &Pool| {
        let status = status_of_a_stopped_claim(claim, pool);
        assert!(status.link_key.is_none());
        let ready = status
            .conditions
            .iter()
            .find(|c| c.type_ == "Ready")
            .expect("Ready");
        assert_eq!(
            (ready.status.as_str(), ready.reason.as_str()),
            ("False", "Stopped")
        );
        let key = status
            .conditions
            .iter()
            .find(|c| c.type_ == "KeyPublished")
            .expect("KeyPublished");
        assert_eq!(
            (key.status.as_str(), key.reason.as_str()),
            ("False", "NotVouched")
        );
        ready.message.clone()
    };
    let by_claim = said(&claim(true), &pool(false));
    assert!(by_claim.contains("This claim is stopped"), "{by_claim}");
    let by_pool = said(&claim(false), &pool(true));
    assert!(by_pool.contains("The pool team is stopped"), "{by_pool}");
}

#[test]
fn a_stopped_pool_keeps_no_runner_warm_whatever_its_warm_pool_held() {
    let mut held = upstream::warm_pool_for(&pool(false), "ada", Vec::new());
    held.spec.replicas = 3;
    assert_eq!(upstream::replicas_kept(&pool(false), Some(&held)), 3);
    assert_eq!(upstream::replicas_kept(&pool(true), Some(&held)), 0);
    assert_eq!(upstream::replicas_kept(&pool(false), None), 0);
}

#[test]
fn a_pool_or_claim_is_running_unless_set_stopped() {
    let pool: Pool = serde_json::from_value(json!({
        "apiVersion": "runners.purlis.dev/v1alpha1",
        "kind": "Pool",
        "metadata": { "name": "team" },
        "spec": { "image": IMAGE },
    }))
    .expect("a pool");
    assert!(!pool.spec.stopped);
    let claim: Claim = serde_json::from_value(json!({
        "apiVersion": "runners.purlis.dev/v1alpha1",
        "kind": "Claim",
        "metadata": { "name": "ada-1", "namespace": "ada" },
        "spec": { "pool": "team", "requester": "ada@example.com" },
    }))
    .expect("a claim");
    assert!(!claim.spec.stopped);
}
