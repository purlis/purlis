// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A Claim whose Pool is deleted keeps its runner until the Claim is deleted, and says so: its
//! `Ready` is still what upstream says of the runner, with the reason `PoolDeleted`.

use k8s_openapi::apimachinery::pkg::apis::meta::v1::{Condition, Time};
use kube::api::ObjectMeta;
use purlis_cluster_operator::{Claim, ClaimSpec, status_of_a_deleted_pool, upstream};

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

fn upstream_saying(status: Option<(&str, &str, &str)>) -> upstream::SandboxClaim {
    let mut given = upstream::sandbox_claim_for(&claim(), &Default::default());
    given.status = status.map(|(status, reason, message)| upstream::SandboxClaimStatus {
        sandbox: None,
        conditions: vec![Condition {
            type_: "Ready".into(),
            status: status.into(),
            reason: reason.into(),
            message: message.into(),
            last_transition_time: Time(k8s_openapi::jiff::Timestamp::UNIX_EPOCH),
            observed_generation: None,
        }],
    });
    given
}

#[test]
fn a_running_runner_whose_pool_is_deleted_is_still_ready_and_the_claim_says_the_pool_is_gone() {
    let status = status_of_a_deleted_pool(
        &claim(),
        &upstream_saying(Some(("True", "SandboxReady", "the sandbox is ready"))),
    );
    let ready = &status.conditions[0];
    assert_eq!(ready.type_, "Ready");
    assert_eq!(ready.status, "True");
    assert_eq!(ready.reason, "PoolDeleted");
    assert!(
        ready.message.contains("The pool team was deleted"),
        "{}",
        ready.message
    );
    assert!(
        ready.message.contains("until the claim is deleted"),
        "{}",
        ready.message
    );
}

#[test]
fn a_runner_that_is_not_ready_says_so_and_why_after_its_pool_is_deleted() {
    let status = status_of_a_deleted_pool(
        &claim(),
        &upstream_saying(Some(("False", "SandboxNotReady", "pulling the image"))),
    );
    let ready = &status.conditions[0];
    assert_eq!(ready.status, "False");
    assert_eq!(ready.reason, "PoolDeleted");
    assert!(
        ready.message.contains("pulling the image"),
        "{}",
        ready.message
    );

    let status = status_of_a_deleted_pool(&claim(), &upstream_saying(None));
    assert_eq!(status.conditions[0].status, "False");
    assert_eq!(status.conditions[0].reason, "PoolDeleted");
}
