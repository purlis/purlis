// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What the operator does with a runner over time** (#3980): what a Claim says of an idle
//! runner, and how the operator hears of one. Nothing here writes to the cluster.
//!
//! Only the runner knows it is idle (ADR 0089 §9), and the operator has no `exec`, so a runner's
//! host ends itself with [`IDLE_MESSAGE`] as its termination message, which the pod's status
//! shows (`crates/purlis-core/src/clusterpool/idle.rs`). A runner can say this of itself and of
//! nothing else, and all it gets by saying it is its own suspension.

use crate::resources::{Claim, ClaimStatus, condition, condition_of};
use crate::upstream::{RUNNER_CONTAINER, Suspension};
use crate::vouch::Vouching;
use k8s_openapi::api::core::v1::Pod;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::Condition;
use k8s_openapi::jiff::{SignedDuration, Timestamp};
use kube::ResourceExt;

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

/// The condition saying `claim`'s runner is suspended for being idle, where it does.
pub fn idle_condition(claim: &Claim) -> Option<&Condition> {
    claim
        .status
        .as_ref()?
        .conditions
        .iter()
        .find(|condition| condition.type_ == IDLE && condition.status == "True")
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
            .find(|condition| condition.type_ == IDLE && condition.status == "True")
    });
    if let Some(held) = held
        && !next.conditions.iter().any(|one| one.type_ == IDLE)
    {
        next.conditions.push(held.clone());
    }
    next
}

/// A Claim's status as its idle runner is asked to run again: no `Idle`, written before its mode
/// is changed, so a pass that fails after it does not suspend the runner again.
pub fn status_woken() -> ClaimStatus {
    ClaimStatus {
        link_key: None,
        mint: None,
        conditions: vec![
            condition(
                "False",
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
