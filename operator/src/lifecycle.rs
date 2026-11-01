// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What the operator does with a runner over time** (#3980, #3981, #3982, #3739): what a Claim
//! says of an idle, retained, aged or stuck runner, how the operator hears of one, and what is an
//! orphan. Nothing here writes to the cluster.
//!
//! Only the runner knows it is idle (ADR 0089 §9), and the operator has no `exec`, so a runner's
//! host ends itself with [`IDLE_MESSAGE`] as its termination message, which the pod's status
//! shows (`crates/purlis-core/src/clusterpool/idle.rs`). A runner can say this of itself and of
//! nothing else, and all it gets by saying it is its own suspension.

use crate::person::NAMESPACE_PREFIX;
use crate::resources::{Claim, ClaimStatus, Pool, READY, condition, condition_of};
use crate::upstream::{self, RUNNER_CONTAINER, SUSPENDED, Sandbox, Suspension};
use crate::vouch::Vouching;
use k8s_openapi::api::core::v1::Pod;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::Condition;
use k8s_openapi::jiff::{SignedDuration, Timestamp};
use kube::ResourceExt;
use kube::api::ObjectMeta;
use std::sync::Arc;
use std::time::Duration;

/// The condition that says a Claim's runner is suspended for being idle, and since when.
pub const IDLE: &str = "Idle";

/// The `Ready` reason of a runner suspended for being idle.
pub const IDLE_REASON: &str = "Idle";

/// What a desktop writes on a Claim to run its idle runner again: the `Idle` condition's
/// `lastTransitionTime`, so a word for an earlier suspension wakes nothing, and no clock but the
/// cluster's is read. Whoever may write it may already set `stopped` back.
pub const RUN_AGAIN_ANNOTATION: &str = "runners.purlis.dev/run-again";

/// The termination message a runner says it is idle with.
pub const IDLE_MESSAGE: &str = "purlis-runner-idle";

/// How long a runner's word is taken for: less than the five minutes its host waits before it
/// serves again, so a runner is never suspended under a link that came after the wait.
pub const IDLE_WORD_HOLDS: SignedDuration = SignedDuration::from_secs(4 * 60);

/// The condition saying `claim`'s runner is suspended for being idle, where it does: `True`, or
/// `Unknown` while it is also stopped ([`pausing_idle`]).
pub fn idle_condition(claim: &Claim) -> Option<&Condition> {
    claim
        .status
        .as_ref()?
        .conditions
        .iter()
        .find(|condition| condition.type_ == IDLE && condition.status != "False")
}

/// Whether `claim`'s desktop asked to run this suspension's runner again.
pub fn asked_to_run_again(claim: &Claim) -> bool {
    let Some(idle) = idle_condition(claim) else {
        return false;
    };
    claim
        .annotations()
        .get(RUN_AGAIN_ANNOTATION)
        .and_then(|asked| asked.parse::<Timestamp>().ok())
        .is_some_and(|asked| asked == idle.last_transition_time.0)
}

/// **Whether the runner of `pod` said it is idle, recently enough to take its word**: its
/// container's current or last end carries [`IDLE_MESSAGE`], and that end is within
/// [`IDLE_WORD_HOLDS`] of `now`.
pub fn said_idle(pod: &Pod, now: Timestamp) -> bool {
    let Some(runner) = pod
        .status
        .as_ref()
        .and_then(|status| status.container_statuses.as_ref())
        .and_then(|statuses| statuses.iter().find(|one| one.name == RUNNER_CONTAINER))
    else {
        return false;
    };
    [runner.state.as_ref(), runner.last_state.as_ref()]
        .into_iter()
        .flatten()
        .filter_map(|state| state.terminated.as_ref())
        .any(|ended| {
            ended.message.as_deref().map(str::trim) == Some(IDLE_MESSAGE)
                && ended
                    .finished_at
                    .as_ref()
                    .is_some_and(|at| now.duration_since(at.0) <= IDLE_WORD_HOLDS)
        })
}

/// `next`, with `previous`'s `Idle` condition kept where `next` does not say it: only a runner
/// asked to run again stops being idle ([`status_woken`]), so a pool stopped, deleted or in the
/// way meanwhile wakes nothing.
pub fn keeping_idle(previous: Option<&ClaimStatus>, mut next: ClaimStatus) -> ClaimStatus {
    let held = previous.and_then(|status| {
        status
            .conditions
            .iter()
            .find(|condition| condition.type_ == IDLE && condition.status != "False")
    });
    if let Some(held) = held
        && !next.conditions.iter().any(|one| one.type_ == IDLE)
    {
        next.conditions.push(held.clone());
    }
    next
}

/// `status`, of a stopped Claim, with the `Idle` of `previous` as `Unknown`: it stays idle, so its
/// runner does not run when the stop ends, and when the condition is `True` again its time has
/// moved, so stopped time never counts toward its retention (ruled 2026-11-01).
pub fn pausing_idle(status: &mut ClaimStatus, previous: Option<&ClaimStatus>) {
    let held = previous.and_then(|status| {
        status
            .conditions
            .iter()
            .find(|condition| condition.type_ == IDLE && condition.status != "False")
    });
    if let Some(held) = held {
        let mut paused = held.clone();
        paused.status = "Unknown".to_owned();
        paused.reason = "Stopped".to_owned();
        status.conditions.push(paused);
    }
}

/// A Claim's status as its idle runner is asked to run again: no `Idle`, written before its mode
/// is changed, so a pass that fails after it does not suspend the runner again.
pub fn status_woken() -> ClaimStatus {
    ClaimStatus {
        link_key: None,
        mint: None,
        conditions: vec![
            // Not `False`, so the time of a `False` to come is not the suspension's.
            condition(
                "Unknown",
                "Starting",
                "Its desktop asked for this idle runner to run again.".to_owned(),
            ),
            Vouching::Refused("its runner is starting again".into()).condition(),
        ],
    }
}

/// A Claim's status while its runner is suspended for being idle: `Ready` is `False` with the
/// reason [`IDLE_REASON`], `Idle` is `True`, and no key is vouched for.
pub fn status_of_an_idle_runner(suspension: Suspension) -> ClaimStatus {
    let runner = match suspension {
        Suspension::Suspended => "is suspended",
        Suspension::NotEnded => "was asked to suspend, but upstream has not ended its pod yet",
        Suspension::Suspending | Suspension::NotMade => "is being suspended",
    };
    let message = format!(
        "Nothing ran on this runner for its pool's idleAfter, so it {runner}. A kept \
                 claim's volume is kept, and the next chat its desktop starts there runs it again; a fresh \
                 claim's runner keeps nothing, so it comes back with a new key."
    );
    ClaimStatus {
        link_key: None,
        mint: None,
        conditions: vec![
            condition("False", IDLE_REASON, message.clone()),
            condition_of(IDLE, "True", IDLE_REASON, message),
            Vouching::Refused("its runner is suspended for being idle".into()).condition(),
        ],
    }
}

/// The condition a kept Claim says it will be deleted with (#3981): its message says when, and a
/// desktop reading its Claim makes an Update of it. Its time is when the requester was told.
pub const DELETION_NOTICE: &str = "DeletionNotice";

/// The operator's own condition on a kept Claim whose retention has ended: the only Claim the
/// cluster's policy lets it delete (`purlis-operator-deletes-only-a-retained-claim`).
pub const RETENTION_ENDED: &str = "RetentionEnded";

/// The `reason` of both.
pub const RETENTION_REASON: &str = "Retention";

/// What a Claim's retention asks now (#3981).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Retention {
    /// Nothing.
    Keep,
    /// Its requester is told it is deleted at this time.
    Notice { at: Timestamp },
    /// Its time, `at`, has come, with the notice its pool asks for already given.
    Delete { at: Timestamp },
}

fn since(duration: Duration) -> SignedDuration {
    SignedDuration::try_from(duration).unwrap_or(SignedDuration::MAX)
}

/// **What `claim`'s retention asks at `now`**. Only a kept Claim suspended for being idle is ever
/// deleted (ruled 2026-11-01): never a stopped one, by itself or by its pool, and nothing while
/// its runner runs. It counts from upstream's `Suspended` condition, so running again starts it
/// over. A requester is always given the whole notice, however late the operator looks: the
/// deletion waits for it.
pub fn retention(
    claim: &Claim,
    pool: &Pool,
    sandbox: Option<&Sandbox>,
    now: Timestamp,
) -> Retention {
    if !claim.spec.kept || upstream::is_stopped(claim, Some(pool)) {
        return Retention::Keep;
    }
    let Some(idle) = idle_condition(claim).filter(|idle| idle.status == "True") else {
        return Retention::Keep;
    };
    let Some(suspended) = sandbox
        .and_then(|sandbox| sandbox.status.as_ref())
        .and_then(|status| {
            status
                .conditions
                .iter()
                .find(|one| one.type_ == SUSPENDED && one.status == "True")
        })
    else {
        return Retention::Keep;
    };
    // Whichever is later: the runner's suspension, or the end of a stop.
    let Ok(at) = suspended
        .last_transition_time
        .0
        .max(idle.last_transition_time.0)
        .checked_add(since(pool.spec.delete_suspended_after()))
    else {
        return Retention::Keep;
    };
    let notice = since(pool.spec.notice_before_delete());
    let noticed = claim.status.as_ref().and_then(|status| {
        status
            .conditions
            .iter()
            .find(|one| one.type_ == DELETION_NOTICE && one.status == "True")
    });
    let Some(noticed) = noticed else {
        return match at.checked_sub(notice) {
            Ok(from) if now >= from => Retention::Notice {
                at: now.checked_add(notice).map_or(at, |given| given.max(at)),
            },
            _ => Retention::Keep,
        };
    };
    // The time it told its requester holds: a Pool's times changed since can only move it later.
    let Some(promised) = noticed
        .message
        .split_whitespace()
        .find_map(|word| word.trim_end_matches(',').parse::<Timestamp>().ok())
    else {
        return Retention::Notice {
            at: now.checked_add(notice).map_or(at, |given| given.max(at)),
        };
    };
    let due = promised.max(at);
    if now >= due {
        Retention::Delete { at: due }
    } else {
        Retention::Notice { at: due }
    }
}

/// `status`, saying what `retention` asks.
pub fn saying_retention(status: &mut ClaimStatus, retention: Retention) {
    let notice = |at: String| {
        condition_of(
            DELETION_NOTICE,
            "True",
            RETENTION_REASON,
            format!(
                "This kept claim has been suspended for its pool's deleteSuspendedAfter: it is \
                 deleted, with its volume, at {at}, unless its runner runs again before then."
            ),
        )
    };
    match retention {
        Retention::Keep => {}
        Retention::Notice { at } => status.conditions.push(notice(at.to_string())),
        Retention::Delete { at } => {
            status.conditions.push(notice(at.to_string()));
            status.conditions.push(condition_of(
                RETENTION_ENDED,
                "True",
                RETENTION_REASON,
                "Its retention has ended: this claim is being deleted, with its volume.".to_owned(),
            ));
        }
    }
}

/// Whether `claim` says its retention has ended, which is what the operator deletes it on.
pub fn retention_ended(claim: &Claim) -> bool {
    claim.status.as_ref().is_some_and(|status| {
        status
            .conditions
            .iter()
            .any(|one| one.type_ == RETENTION_ENDED && one.status == "True")
    })
}

/// The condition that says a running runner is past its pool's `maxAge` (#3982).
pub const PAST_MAX_AGE: &str = "PastMaxAge";

/// Whether the runner of `pod` is older than `pool`'s `maxAge`.
pub fn past_max_age(pod: &Pod, pool: &Pool, now: Timestamp) -> bool {
    pod.metadata
        .creation_timestamp
        .as_ref()
        .is_some_and(|made| now.duration_since(made.0) >= since(pool.spec.max_age()))
}

/// `status`, saying its running runner is past its pool's `maxAge`: it never ends a chat for it
/// (D-3982-1), so it takes the pool's image the next time nothing runs on it.
pub fn saying_past_max_age(status: &mut ClaimStatus) {
    status.conditions.push(condition_of(
        PAST_MAX_AGE,
        "True",
        "WaitsForIdle",
        "This runner is past its pool's maxAge. A chat is never ended for that: once it is next \
         idle, it is on the pool's current image when its desktop runs it again."
            .to_owned(),
    ));
}

/// **`pod_spec`, a Sandbox's pod template spec as the cluster holds it, with every container on
/// `image`**, and whether that changed anything (#3982). Only the image: what upstream added to
/// that spec as it made the Sandbox stays.
pub fn with_image(pod_spec: &mut serde_json::Value, image: &str) -> bool {
    let mut changed = false;
    for list in ["initContainers", "containers"] {
        let Some(containers) = pod_spec.get_mut(list).and_then(|held| held.as_array_mut()) else {
            continue;
        };
        for container in containers {
            if container.get("image").and_then(|held| held.as_str()) != Some(image) {
                container["image"] = serde_json::Value::from(image);
                changed = true;
            }
        }
    }
    changed
}

/// The condition that says a Claim's runner has not become ready for a long while (#3739).
pub const STUCK: &str = "Stuck";

/// How long a runner goes on not being ready before its Claim says it is stuck.
pub const STUCK_AFTER: Duration = Duration::from_secs(30 * 60);

/// How often the operator looks for what its Claims left behind, and how old an object must be to
/// be taken for that: a Claim being made has its objects before the operator's watch has it.
pub const ORPHANS_EVERY: Duration = Duration::from_secs(10 * 60);

/// `status` as written next, saying its runner is stuck where its `Ready` has been `False` for
/// `after` in `previous`, and its pod, where it has one, is that old: a `Ready` that was `False`
/// while it was stopped or idle keeps its time, and is no word on a runner just started. It only says so, as a Deployment's progress deadline does: a pod waiting
/// for a node may yet start, and the person decides. Its quota and volume stay held.
pub fn saying_stuck(
    status: &mut ClaimStatus,
    previous: Option<&ClaimStatus>,
    pod: Option<&Pod>,
    after: Duration,
    now: Timestamp,
) {
    let ready_of = |status: &ClaimStatus| {
        status
            .conditions
            .iter()
            .find(|one| one.type_ == READY)
            .cloned()
    };
    let (Some(ready), Some(held)) = (ready_of(status), previous.and_then(ready_of)) else {
        return;
    };
    // A stop or an idle suspension has just ended: `Unknown` for this pass, so the `False` to come
    // has a time of its own, as a woken runner's does.
    if ready.status != "True" && matches!(held.reason.as_str(), "Stopped" | IDLE_REASON) {
        if let Some(ready) = status.conditions.iter_mut().find(|one| one.type_ == READY) {
            ready.status = "Unknown".to_owned();
        }
        return;
    }
    let pod_is_new = pod
        .and_then(|pod| pod.metadata.creation_timestamp.as_ref())
        .is_some_and(|made| now.duration_since(made.0) < since(after));
    if ready.status == "True"
        || held.status != "False"
        || pod_is_new
        || now.duration_since(held.last_transition_time.0) < since(after)
    {
        return;
    }
    status.conditions.push(condition_of(
        STUCK,
        "True",
        &ready.reason,
        format!(
            "This claim's runner has not been ready for {} minutes: {} Nothing is deleted for you. \
             Set a kept claim stopped and back to take its pool's image, or delete the claim to \
             give it up, its volume with it.",
            after.as_secs() / 60,
            ready.message
        ),
    ));
}

/// Whether `object`, an upstream claim in a runner namespace, is one whose Claim has gone: it
/// names the uid of a Claim the operator's watch does not hold, and is older than `grace`.
pub fn upstream_claim_is_orphaned(
    object: &ObjectMeta,
    claims: &[Arc<Claim>],
    grace: Duration,
    now: Timestamp,
) -> bool {
    let in_a_runner_namespace = object
        .namespace
        .as_deref()
        .is_some_and(|namespace| namespace.starts_with(NAMESPACE_PREFIX));
    let Some(uid) = object
        .annotations
        .as_ref()
        .and_then(|marks| marks.get(crate::person::CLAIM_UID_ANNOTATION))
    else {
        return false;
    };
    in_a_runner_namespace
        && older_than(object, grace, now)
        && object.deletion_timestamp.is_none()
        && !claims
            .iter()
            .any(|claim| claim.metadata.uid.as_ref() == Some(uid))
}

/// Whether `object`, a pool's template or warm pool in a runner namespace, is one no Claim needs:
/// no Claim the operator's watch holds, of that namespace's person, is on its pool.
pub fn pool_object_is_orphaned(
    object: &ObjectMeta,
    claims: &[Arc<Claim>],
    grace: Duration,
    now: Timestamp,
) -> bool {
    let (Some(namespace), Some(pool)) = (
        object.namespace.as_deref(),
        object.name.as_deref().and_then(upstream::pool_of),
    ) else {
        return false;
    };
    namespace.starts_with(NAMESPACE_PREFIX)
        && older_than(object, grace, now)
        && object.deletion_timestamp.is_none()
        && !claims.iter().any(|claim| {
            claim.spec.pool == pool
                && crate::person::runner_namespace(&claim.spec.requester) == namespace
        })
}

fn older_than(object: &ObjectMeta, grace: Duration, now: Timestamp) -> bool {
    object
        .creation_timestamp
        .as_ref()
        .is_some_and(|made| now.duration_since(made.0) >= since(grace))
}
