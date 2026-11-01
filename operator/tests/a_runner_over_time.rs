// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What becomes of a runner over time**: a long-suspended kept Claim is deleted after a notice
//! (#3981), a suspended kept runner takes its pool's image (#3982), and a Claim stuck or an object
//! left behind is said or removed (#3739).

use k8s_openapi::apimachinery::pkg::apis::meta::v1::{Condition, Time};
use k8s_openapi::jiff::{SignedDuration, Timestamp};
use kube::api::ObjectMeta;
use purlis_cluster_operator::lifecycle::{
    DELETION_NOTICE, Retention, RetentionFloor, STUCK, idle_condition, pausing_idle,
    pool_object_is_orphaned, retention, retention_above, saying_stuck, status_of_an_idle_runner,
    upstream_claim_is_orphaned, with_image,
};
use purlis_cluster_operator::person::CLAIM_UID_ANNOTATION;
use purlis_cluster_operator::upstream::{
    SUSPENDED, Sandbox, SandboxSpec, SandboxStatus, Suspension,
};
use purlis_cluster_operator::{
    Claim, ClaimSpec, ClaimStatus, Pool, PoolSpec, status_of_a_stopped_claim,
    status_of_an_object_in_the_way,
};
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

/// `claim(true)`, its `Idle` from `at`.
fn idle_from(at: Timestamp) -> Claim {
    let mut held = claim(true);
    for one in &mut held.status.as_mut().unwrap().conditions {
        one.last_transition_time = Time(at);
    }
    held
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
    let held = idle_from(start);
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
    let mut noticed = idle_from(start);
    let status: &mut ClaimStatus = noticed.status.as_mut().unwrap();
    let mut told = status.conditions[0].clone();
    told.type_ = DELETION_NOTICE.into();
    told.status = "True".into();
    told.message = format!(
        "it is deleted, with its volume, at {}, unless",
        start + hours(100)
    );
    told.last_transition_time = Time(start + hours(90));
    status.conditions.push(told);
    assert!(matches!(at(&noticed, 99), Retention::Notice { .. }));
    assert!(matches!(at(&noticed, 100), Retention::Delete { .. }));
    // A Pool whose times are shortened later does not bring forward the time it told.
    let mut hurried = pool();
    hurried.spec.delete_suspended_after = "2h".into();
    hurried.spec.notice_before_delete = "1h".into();
    assert!(matches!(
        retention(&noticed, &hurried, Some(&sandbox), start + hours(99)),
        Retention::Notice { .. }
    ));
}

/// **A typed `30s` does not lose a volume** (#4405): a Pool's retention holds above its floor, two
/// hours with an hour's notice, and a run that needs it shorter says so to the operator.
#[test]
fn a_pools_retention_has_a_floor() {
    let start = Timestamp::now();
    let sandbox = suspended_at(start);
    let mut hasty = pool();
    hasty.spec.delete_suspended_after = "30s".into();
    hasty.spec.notice_before_delete = "10s".into();
    let held = idle_from(start);
    assert_eq!(
        retention(
            &held,
            &hasty,
            Some(&sandbox),
            start + SignedDuration::from_secs(59 * 60)
        ),
        Retention::Keep,
        "before an hour has passed, nothing is said"
    );
    assert!(
        matches!(
            retention(&held, &hasty, Some(&sandbox), start + hours(1)),
            Retention::Notice { at } if at == start + hours(2)
        ),
        "the notice is an hour, before two hours"
    );
    assert!(
        matches!(
            retention_above(
                &held,
                &hasty,
                Some(&sandbox),
                start + SignedDuration::from_secs(25),
                RetentionFloor::of(1)
            ),
            Retention::Notice { .. }
        ),
        "a floor lowered for a kind run lets the Pool's own times hold"
    );
}

/// **A stop while an upstream object is in the way is still a stop** (#4405): the Claim's `Idle`
/// says so as any stop's does, so when it is `True` again the count starts over.
#[test]
fn a_stop_with_something_in_the_way_does_not_count_either() {
    let held = idle_from(Timestamp::now());
    let mut status = status_of_an_object_in_the_way("SandboxTemplate", "x", "purlis-runners-ada");
    pausing_idle(&mut status, held.status.as_ref());
    let paused = status
        .conditions
        .iter()
        .find(|one| one.type_ == "Idle")
        .expect("Idle is said");
    assert_eq!(paused.status, "Unknown");
}

#[test]
fn stopped_time_never_counts_toward_retention() {
    let start = Timestamp::now();
    let sandbox = suspended_at(start);
    // Idle again 200 hours on, as the stop ended: the runner's own suspension is far older.
    let held = idle_from(start + hours(200));
    let at = |after| retention(&held, &pool(), Some(&sandbox), start + hours(after));
    assert_eq!(at(200), Retention::Keep);
    assert_eq!(at(289), Retention::Keep);
    assert!(matches!(at(290), Retention::Notice { .. }));
    // While stopped it stays idle, says Unknown, and is not retained.
    let mut paused = status_of_a_stopped_claim(&held, None, Suspension::Suspended);
    pausing_idle(&mut paused, held.status.as_ref());
    let mut stopped = held.clone();
    stopped.status = Some(paused);
    assert!(idle_condition(&stopped).is_some());
    assert_eq!(
        retention(&stopped, &pool(), Some(&sandbox), start + hours(1000)),
        Retention::Keep
    );
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
    let says = |ready: &str, reason: &str, ago: i64, pod_age: Option<i64>| {
        let mut previous = purlis_cluster_operator::status_of(None);
        previous.conditions[0].status = ready.into();
        previous.conditions[0].reason = reason.into();
        previous.conditions[0].last_transition_time = Time(now - hours(ago));
        let mut next = purlis_cluster_operator::status_of(None);
        let pod = pod_age.map(|age| k8s_openapi::api::core::v1::Pod {
            metadata: ObjectMeta {
                creation_timestamp: Some(Time(now - hours(age))),
                ..ObjectMeta::default()
            },
            ..Default::default()
        });
        saying_stuck(
            &mut next,
            Some(&previous),
            pod.as_ref(),
            Duration::from_secs(1800),
            now,
        );
        next.conditions.iter().any(|one| one.type_ == STUCK)
    };
    assert!(says("False", "Pulling", 1, Some(1)));
    assert!(says("False", "Pulling", 1, None), "no pod ever came");
    assert!(!says("False", "Pulling", 0, Some(1)), "not for long yet");
    assert!(!says("True", "Ready", 1, Some(1)), "it was ready until now");
    assert!(!says("False", "Pulling", 1, Some(0)), "its pod is new");
    assert!(
        !says("False", "Idle", 1, None),
        "it was suspended until now"
    );
    assert!(
        !says("False", "Stopped", 1, None),
        "it was stopped until now"
    );
    // Its time starts over: `Ready` is `Unknown` for the pass after a stop.
    let mut previous = purlis_cluster_operator::status_of(None);
    previous.conditions[0].reason = "Stopped".into();
    let mut next = purlis_cluster_operator::status_of(None);
    saying_stuck(
        &mut next,
        Some(&previous),
        None,
        Duration::from_secs(1800),
        now,
    );
    assert_eq!(next.conditions[0].status, "Unknown");
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
