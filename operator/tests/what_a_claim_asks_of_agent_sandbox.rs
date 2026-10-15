// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What the operator asks of the pinned upstream agent-sandbox for a person's Claim on a team's
//! Pool, and what it tells the person back on the Claim. Read through the objects it would apply,
//! as the cluster would receive them.

use k8s_openapi::apimachinery::pkg::apis::meta::v1::{Condition, Time};
use kube::api::ObjectMeta;
use purlis_cluster_operator::{Claim, ClaimSpec, Pool, PoolSpec, upstream};
use serde_json::{Value, json};

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

fn json_of(value: &impl serde::Serialize) -> Value {
    serde_json::to_value(value).expect("serialises")
}

#[test]
fn a_claims_runner_runs_the_pools_image_and_its_own_command_as_a_user_who_is_not_root() {
    let template = json_of(&upstream::template_for(&pool(), "ada"));
    let pod = &template["spec"]["podTemplate"]["spec"];
    let containers = pod["containers"].as_array().expect("containers");
    assert_eq!(containers.len(), 1, "{pod}");
    let runner = &containers[0];
    assert_eq!(runner["image"], IMAGE);
    // The image's own entrypoint and command: `tini -- purlis serve` (docs/runner-image.md).
    assert!(runner.get("command").is_none(), "{runner}");
    assert!(runner.get("args").is_none(), "{runner}");
    let security = &runner["securityContext"];
    assert_eq!(security["runAsNonRoot"], true);
    assert_eq!(security["allowPrivilegeEscalation"], false);
    assert_eq!(security["capabilities"]["drop"], json!(["ALL"]));
    assert_eq!(security["seccompProfile"]["type"], "RuntimeDefault");
}

#[test]
fn a_runner_gets_no_kubernetes_credential_and_no_service_addresses() {
    let template = json_of(&upstream::template_for(&pool(), "ada"));
    let pod = &template["spec"]["podTemplate"]["spec"];
    assert_eq!(pod["automountServiceAccountToken"], false, "{pod}");
    assert_eq!(pod["enableServiceLinks"], false, "{pod}");
}

#[test]
fn a_pools_template_and_warm_pool_live_in_the_claims_namespace_and_belong_to_the_pool() {
    for object in [
        json_of(&upstream::template_for(&pool(), "ada")),
        json_of(&upstream::warm_pool_for(&pool(), "ada")),
    ] {
        assert_eq!(object["metadata"]["namespace"], "ada", "{object}");
        assert_eq!(object["metadata"]["name"], "purlis-pool-team", "{object}");
        let owners = object["metadata"]["ownerReferences"]
            .as_array()
            .expect("owners");
        assert_eq!(owners.len(), 1, "{object}");
        assert_eq!(owners[0]["kind"], "Pool");
        assert_eq!(owners[0]["apiVersion"], "runners.purlis.dev/v1alpha1");
        assert_eq!(owners[0]["name"], "team");
        assert_eq!(owners[0]["uid"], "pool-uid");
    }
}

#[test]
fn a_pool_keeps_no_runner_warm() {
    let warm = json_of(&upstream::warm_pool_for(&pool(), "ada"));
    assert_eq!(warm["spec"]["replicas"], 0, "{warm}");
    assert_eq!(
        warm["spec"]["sandboxTemplateRef"]["name"],
        "purlis-pool-team"
    );
}

#[test]
fn a_claim_becomes_an_upstream_claim_on_its_pools_warm_pool_that_goes_with_it() {
    let upstream = json_of(&upstream::sandbox_claim_for(&claim()));
    assert_eq!(upstream["apiVersion"], "extensions.agents.x-k8s.io/v1beta1");
    assert_eq!(upstream["kind"], "SandboxClaim");
    assert_eq!(upstream["metadata"]["name"], "ada-1");
    assert_eq!(upstream["metadata"]["namespace"], "ada");
    assert_eq!(upstream["spec"]["warmPoolRef"]["name"], "purlis-pool-team");
    let owners = upstream["metadata"]["ownerReferences"]
        .as_array()
        .expect("owners");
    assert_eq!(owners.len(), 1, "{upstream}");
    assert_eq!(owners[0]["kind"], "Claim");
    assert_eq!(owners[0]["uid"], "claim-uid");
    assert_eq!(owners[0]["controller"], true);
}

fn ready(status: &str, reason: &str, message: &str) -> Condition {
    Condition {
        type_: "Ready".into(),
        status: status.into(),
        reason: reason.into(),
        message: message.into(),
        last_transition_time: Time(k8s_openapi::jiff::Timestamp::UNIX_EPOCH),
        observed_generation: None,
    }
}

#[test]
fn a_claim_is_ready_when_its_upstream_claim_is_and_says_why_not_otherwise() {
    let mut given = upstream::sandbox_claim_for(&claim());
    given.status = Some(upstream::SandboxClaimStatus {
        conditions: vec![ready("True", "SandboxReady", "the sandbox is ready")],
    });
    let status = purlis_cluster_operator::status_of(Some(&given));
    let condition = &status.conditions[0];
    assert_eq!(
        (condition.type_.as_str(), condition.status.as_str()),
        ("Ready", "True")
    );

    given.status = Some(upstream::SandboxClaimStatus {
        conditions: vec![ready("False", "SandboxNotReady", "pulling the image")],
    });
    let status = purlis_cluster_operator::status_of(Some(&given));
    let condition = &status.conditions[0];
    assert_eq!(condition.status, "False");
    assert_eq!(condition.reason, "SandboxNotReady");
    assert_eq!(condition.message, "pulling the image");
}

#[test]
fn a_claim_whose_runner_has_not_been_asked_for_yet_is_starting() {
    let given = upstream::sandbox_claim_for(&claim());
    let status = purlis_cluster_operator::status_of(Some(&given));
    let condition = &status.conditions[0];
    assert_eq!(condition.status, "False");
    assert_eq!(condition.reason, "Starting");
}

#[test]
fn a_claim_on_a_pool_that_does_not_exist_says_so() {
    let status = purlis_cluster_operator::status_of_a_missing_pool(&claim());
    let condition = &status.conditions[0];
    assert_eq!(condition.type_, "Ready");
    assert_eq!(condition.status, "False");
    assert_eq!(condition.reason, "PoolNotFound");
    assert_eq!(condition.message, "There is no pool named team.");
}

#[test]
fn a_claims_ready_keeps_the_time_it_last_changed_until_it_changes_again() {
    let mut before = purlis_cluster_operator::status_of(None);
    before.conditions[0].last_transition_time = Time(k8s_openapi::jiff::Timestamp::UNIX_EPOCH);

    let again =
        purlis_cluster_operator::settled(Some(&before), purlis_cluster_operator::status_of(None));
    assert_eq!(again, before, "the same word is not written twice");

    let mut given = upstream::sandbox_claim_for(&claim());
    given.status = Some(upstream::SandboxClaimStatus {
        conditions: vec![ready("True", "SandboxReady", "ready")],
    });
    let changed = purlis_cluster_operator::settled(
        Some(&before),
        purlis_cluster_operator::status_of(Some(&given)),
    );
    assert_ne!(
        changed.conditions[0].last_transition_time,
        before.conditions[0].last_transition_time
    );
}
