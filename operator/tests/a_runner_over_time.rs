// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What becomes of a runner over time**: a long-suspended kept Claim is deleted after a notice
//! (#3981), a suspended kept runner takes its pool's image (#3982), and a Claim stuck or an object
//! left behind is said or removed (#3739).

use k8s_openapi::apimachinery::pkg::apis::meta::v1::{Condition, Time};
use k8s_openapi::jiff::{SignedDuration, Timestamp};
use kube::api::ObjectMeta;
use purlis_cluster_operator::lifecycle::{
    DELETION_NOTICE, Retention, STUCK, pool_object_is_orphaned, retention, saying_stuck,
    status_of_an_idle_runner, upstream_claim_is_orphaned, with_image,
};
use purlis_cluster_operator::person::CLAIM_UID_ANNOTATION;
use purlis_cluster_operator::upstream::{
    SUSPENDED, Sandbox, SandboxSpec, SandboxStatus, Suspension,
};
use purlis_cluster_operator::{Claim, ClaimSpec, ClaimStatus, Pool, PoolSpec};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;

const IMAGE: &str = "ghcr.io/purlis/purlis-runner@sha256:1111111111111111111111111111111111111111111111111111111111111111";
const HOUR: i64 = 3600;

fn pool() -> Pool {
    let mut spec = PoolSpec::new(IMAGE);
    spec.delete_suspended_after = "100h".into();
    spec.notice_before_delete = "10h".into();
    Pool::new("team", spec)
}

fn claim(kept: bool) -> Claim {
    let mut claim = Claim::new(
        "ada-1",
        ClaimSpec {
            pool: "team".into(),
            requester: "ada@example.com".into(),
            kept,
            stopped: false,
            forge: None,
        },
    );
    claim.metadata.uid = Some("c1".into());
    claim.status = Some(status_of_an_idle_runner(Suspension::Suspended));
    claim
}

fn suspended_at(at: Timestamp) -> Sandbox {
    let mut sandbox = Sandbox::new(
        "ada-1",
        SandboxSpec {
            operating_mode: Some(SUSPENDED.into()),
        },
    );
    sandbox.status = Some(SandboxStatus {
        conditions: vec![Condition {
            type_: SUSPENDED.into(),
            status: "True".into(),
            reason: String::new(),
            message: String::new(),
            last_transition_time: Time(at),
            observed_generation: None,
        }],
    });
    sandbox
}

fn hours(count: i64) -> SignedDuration {
    SignedDuration::from_secs(count * HOUR)
}

#[test]
fn a_kept_claim_is_noticed_then_deleted_and_never_sooner() {
    let start = Timestamp::now();
    let sandbox = suspended_at(start);
    let at =
        |claim: &Claim, after: i64| retention(claim, &pool(), Some(&sandbox), start + hours(after));
    let held = claim(true);
    assert_eq!(at(&held, 89), Retention::Keep, "before the notice");
    assert!(
        matches!(at(&held, 90), Retention::Notice { at } if at == start + hours(100)),
        "the notice comes its time before"
    );
    // No notice yet at the deletion's own time: the whole notice is still given.
    assert!(
        matches!(at(&held, 150), Retention::Notice { at } if at == start + hours(160)),
        "an operator that looks late still gives the whole notice"
    );
    let mut noticed = claim(true);
    let status: &mut ClaimStatus = noticed.status.as_mut().unwrap();
    let mut told = status.conditions[0].clone();
    told.type_ = DELETION_NOTICE.into();
    told.last_transition_time = Time(start + hours(90));
    status.conditions.push(told);
    assert!(matches!(at(&noticed, 99), Retention::Notice { .. }));
    assert!(matches!(at(&noticed, 100), Retention::Delete { .. }));
}

#[test]
fn only_a_kept_idle_claim_that_is_not_stopped_is_retained() {
    let start = Timestamp::now();
    let sandbox = suspended_at(start);
    let late = start + hours(1000);
    assert_eq!(
        retention(&claim(false), &pool(), Some(&sandbox), late),
        Retention::Keep
    );
    let mut stopped = claim(true);
    stopped.spec.stopped = true;
    assert_eq!(
        retention(&stopped, &pool(), Some(&sandbox), late),
        Retention::Keep
    );
    let mut pool_stopped = pool();
    pool_stopped.spec.stopped = true;
    assert_eq!(
        retention(&claim(true), &pool_stopped, Some(&sandbox), late),
        Retention::Keep
    );
    let mut running = claim(true);
    running.status = None;
    assert_eq!(
        retention(&running, &pool(), Some(&sandbox), late),
        Retention::Keep
    );
    assert_eq!(
        retention(&claim(true), &pool(), None, late),
        Retention::Keep,
        "no word from upstream"
    );
}

#[test]
fn only_the_images_of_a_sandboxs_pod_change() {
    let mut spec = json!({
        "dnsPolicy": "None",
        "initContainers": [{ "name": "link-key", "image": "old", "args": ["a"] }],
        "containers": [{ "name": "runner", "image": "old", "env": [{ "name": "HOME" }] }],
    });
    assert!(with_image(&mut spec, "new"));
    assert_eq!(
        spec,
        json!({
            "dnsPolicy": "None",
            "initContainers": [{ "name": "link-key", "image": "new", "args": ["a"] }],
            "containers": [{ "name": "runner", "image": "new", "env": [{ "name": "HOME" }] }],
        })
    );
    assert!(!with_image(&mut spec, "new"), "nothing to do twice");
}

#[test]
fn a_runner_not_ready_for_long_is_said_to_be_stuck_and_nothing_more() {
    let now = Timestamp::now();
    let says = |ready: &str, ago: i64| {
        let mut previous = purlis_cluster_operator::status_of(None);
        previous.conditions[0].status = ready.into();
        previous.conditions[0].last_transition_time = Time(now - hours(ago));
        let mut next = purlis_cluster_operator::status_of(None);
        saying_stuck(&mut next, Some(&previous), Duration::from_secs(1800), now);
        next.conditions.iter().any(|one| one.type_ == STUCK)
    };
    assert!(says("False", 1));
    assert!(!says("False", 0), "not for long yet");
    assert!(!says("True", 1), "it was ready until now");
}

fn object(name: &str, namespace: &str, age: i64, uid: Option<&str>) -> ObjectMeta {
    ObjectMeta {
        name: Some(name.into()),
        namespace: Some(namespace.into()),
        creation_timestamp: Some(Time(Timestamp::now() - hours(age))),
        annotations: uid.map(|uid| [(CLAIM_UID_ANNOTATION.to_owned(), uid.to_owned())].into()),
        ..ObjectMeta::default()
    }
}

#[test]
fn only_what_the_operators_claims_have_left_is_orphaned() {
    let grace = Duration::from_secs(600);
    let now = Timestamp::now();
    let claims = vec![Arc::new(claim(true))];
    let ns = purlis_cluster_operator::person::runner_namespace("ada@example.com");
    let orphaned = |meta: &ObjectMeta| upstream_claim_is_orphaned(meta, &claims, grace, now);
    assert!(orphaned(&object("gone", &ns, 1, Some("c2"))));
    assert!(
        !orphaned(&object("ada-1", &ns, 1, Some("c1"))),
        "its claim is here"
    );
    assert!(
        !orphaned(&object("new", &ns, 0, Some("c2"))),
        "too new to say"
    );
    assert!(
        !orphaned(&object("other", "team-a", 1, Some("c2"))),
        "not a runner namespace"
    );
    assert!(
        !orphaned(&object("theirs", &ns, 1, None)),
        "not the operator's"
    );

    let of_pool = |name: &str, namespace: &str| {
        pool_object_is_orphaned(&object(name, namespace, 1, None), &claims, grace, now)
    };
    assert!(!of_pool("purlis-pool-team", &ns), "a claim is on it");
    assert!(of_pool("purlis-pool-gone", &ns));
    assert!(
        !of_pool("purlis-pool-gone", "team-a"),
        "not a runner namespace"
    );
    assert!(!of_pool("something-else", &ns), "not named for a pool");
}
