// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The control loop: each Claim, whenever it, its upstream claim, its pool's template or warm
//! pool in its requester's runner namespace, its Pool or the policy's binding changes, is
//! brought to what [`crate::upstream`] says it asks for, and its status to what upstream says of
//! it. A Claim not admitted under the policy ([`crate::admission`]) is never acted on: only its
//! status says why.
//!
//! A Claim's runner runs in its requester's runner namespace ([`crate::person`], #2055), which
//! the operator makes with the requester's role binding and quota; the Claim carries
//! [`person::FINALIZER`], and what the operator made for it there goes before it does
//! ([`cleanup`]).
//!
//! It changes only upstream objects it made ([`upstream::made_for`],
//! [`upstream::made_for_claim`]). Each is read first: one it did not make is left as it is, and
//! nothing is written for the Claim until that object is gone.

use crate::admission::{POLICY, RUNNER_NAMESPACES_POLICY, admission, in_force_since};
use crate::lifecycle;
use crate::metrics::{Cause, Metrics};
use crate::person::{self, QUOTA, Quota, RunnerNamespace};
use crate::resources::{Claim, ClaimStatus, Pool, PoolStatus, READY, condition};
use crate::stage;
use crate::upstream::{
    self, Found, Sandbox, SandboxClaim, SandboxTemplate, SandboxWarmPool, Suspension, made_for,
    made_for_claim, runner_owner, sandbox_claim_for, template_for, warm_pool_for,
};
use crate::vouch::{self, MountedKeys, Vouching};
use futures::StreamExt;
use k8s_openapi::api::admissionregistration::v1::ValidatingAdmissionPolicyBinding;
use k8s_openapi::api::core::v1::{Namespace, Pod, ResourceQuota};
use k8s_openapi::api::networking::v1::NetworkPolicy;
use k8s_openapi::api::rbac::v1::RoleBinding;
use kube::api::{
    Api, DeleteParams, ObjectMeta, PartialObjectMeta, Patch, PatchParams, PostParams, Preconditions,
};
use kube::runtime::controller::{Action, Controller};
use kube::runtime::finalizer::{self, Event, finalizer};
use kube::runtime::reflector::{ObjectRef, Store};
use kube::runtime::{WatchStreamExt, watcher};
use kube::{Client, Resource, ResourceExt};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Who the operator's server-side applies are from.
pub const FIELD_MANAGER: &str = "purlis-cluster-operator";

/// How soon a Claim over its requester's quota is looked at again: the quota frees as their
/// other Claims go, which the operator does not watch for.
pub const OVER_QUOTA_AGAIN: Duration = Duration::from_secs(30);

/// Why a Claim could not be brought to what it asks for. It is tried again.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("the cluster refused or failed a request: {0}")]
    Kube(#[from] kube::Error),
    #[error("the claim {0} has no namespace")]
    NoNamespace(String),
    #[error("the upstream object {0} was not made by the operator")]
    InTheWay(String),
    #[error("the claim's finalizer could not be read: {0}")]
    Finalizer(String),
}

impl Error {
    /// The error a pass under the Claim's finalizer ended with, as the operator's own.
    fn of_finalizer(error: finalizer::Error<Error>) -> Error {
        match error {
            finalizer::Error::ApplyFailed(error) | finalizer::Error::CleanupFailed(error) => error,
            finalizer::Error::AddFinalizer(error) | finalizer::Error::RemoveFinalizer(error) => {
                Error::Kube(error)
            }
            other => Error::Finalizer(other.to_string()),
        }
    }
}

/// A Claim's status from its upstream claim: `Ready` is upstream's `Ready`, with upstream's own
/// reason and words; with no upstream claim or no word from upstream yet, the claim is starting.
pub fn status_of(upstream: Option<&SandboxClaim>) -> ClaimStatus {
    let ready = upstream
        .and_then(|claim| claim.status.as_ref())
        .and_then(|status| status.conditions.iter().find(|c| c.type_ == READY));
    let ready = match ready {
        Some(ready) => condition(&ready.status, &ready.reason, ready.message.clone()),
        None => condition(
            "False",
            "Starting",
            "The runner has been asked for.".to_owned(),
        ),
    };
    ClaimStatus {
        link_key: None,
        mint: None,
        conditions: vec![ready],
    }
}

/// A Claim's status when the pool it names does not exist.
pub fn status_of_a_missing_pool(claim: &Claim) -> ClaimStatus {
    ClaimStatus {
        link_key: None,
        mint: None,
        conditions: vec![condition(
            "False",
            "PoolNotFound",
            format!("There is no pool named {}.", claim.spec.pool),
        )],
    }
}

/// A Claim's status when its pool was deleted after its runner was asked for: the runner keeps
/// running until the Claim is deleted, so `Ready` is still what upstream says of it, with the
/// reason `PoolDeleted`.
pub fn status_of_a_deleted_pool(claim: &Claim, upstream: &SandboxClaim) -> ClaimStatus {
    let ready = upstream
        .status
        .as_ref()
        .and_then(|status| status.conditions.iter().find(|c| c.type_ == READY));
    let mut message = format!(
        "The pool {} was deleted. This claim's runner keeps running, as the pool last defined \
         it, until the claim is deleted; no new claim is served from the pool.",
        claim.spec.pool
    );
    let status = match ready {
        Some(ready) => {
            if !ready.message.is_empty() {
                message.push_str(&format!(" The runner: {}", ready.message));
            }
            ready.status.as_str()
        }
        None => "False",
    };
    ClaimStatus {
        link_key: None,
        mint: None,
        conditions: vec![condition(status, "PoolDeleted", message)],
    }
}

/// A Claim's status when an upstream object the operator would make or change, the `kind` named
/// `name` in its requester's runner namespace `namespace`, is one it did not make for this
/// Claim: it is left as it is, and the Claim waits until it is removed or renamed.
pub fn status_of_an_object_in_the_way(kind: &str, name: &str, namespace: &str) -> ClaimStatus {
    ClaimStatus {
        link_key: None,
        mint: None,
        conditions: vec![condition(
            "False",
            "UpstreamObjectInTheWay",
            format!(
                "The {kind} {name} in the runner namespace {namespace} was not made by purlis's \
                 cluster operator for this claim, so it is left as it is. Remove or rename it, \
                 and this claim goes on."
            ),
        )],
    }
}

/// A Claim's status while it, or its `pool` (where it still exists), is stopped (#2058):
/// `Ready` is `False` with the reason `Stopped`, saying which is stopped and how far its runner
/// is suspended ([`Suspension`]), and no key is vouched for.
pub fn status_of_a_stopped_claim(
    claim: &Claim,
    pool: Option<&Pool>,
    suspension: Suspension,
) -> ClaimStatus {
    let runner = match suspension {
        Suspension::NotMade => "its runner is not made",
        Suspension::Suspending => "its runner is being suspended, a kept claim's volume kept",
        Suspension::Suspended => "its runner is suspended, a kept claim's volume kept",
        Suspension::NotEnded => {
            "its runner was asked to suspend, but upstream has not ended its pod yet"
        }
    };
    let message = match pool {
        Some(pool) if !claim.spec.stopped => format!(
            "The pool {} is stopped: {runner}, until the pool's stopped is false.",
            pool.name_any()
        ),
        _ => format!("This claim is stopped: {runner}, until the claim's stopped is false."),
    };
    ClaimStatus {
        link_key: None,
        mint: None,
        conditions: vec![
            condition("False", "Stopped", message),
            Vouching::Refused("its runner is stopped".into()).condition(),
        ],
    }
}

/// A Claim's status when its pool was deleted while its runner was suspended (#2058): the
/// runner stays suspended, since nothing is left to set the pool running again; its claim's own
/// `stopped` still suspends it, and deleting the claim ends it.
pub fn status_of_a_suspended_runner_of_a_deleted_pool(claim: &Claim) -> ClaimStatus {
    ClaimStatus {
        link_key: None,
        mint: None,
        conditions: vec![condition(
            "False",
            "PoolDeleted",
            format!(
                "The pool {} was deleted, and this claim's runner is suspended: it stays \
                 suspended, a kept claim's volume kept, until the claim is deleted. No new \
                 claim is served from the pool.",
                claim.spec.pool
            ),
        )],
    }
}

/// How many times in a row a stopped claim's runner is looked at again while upstream has not
/// ended its pod, before its claim says so ([`Suspension::NotEnded`]).
const SUSPEND_PATIENCE: u32 = 8;

/// How soon a stopped claim whose runner is being suspended is looked at again after its
/// `attempts`-th look in a row: three seconds, doubling each time, never longer than a minute;
/// none once [`SUSPEND_PATIENCE`] looks have passed (about four and a half minutes), when the
/// claim says upstream has not ended the runner's pod and is looked at only now and then.
pub fn suspending_again(attempts: u32) -> Option<Duration> {
    (attempts <= SUSPEND_PATIENCE).then(|| {
        vouch::KEY_WAIT
            .saturating_mul(2u32.saturating_pow(attempts.saturating_sub(1)))
            .min(vouch::RECHECK)
    })
}

/// `suspension`, as far as `claim`'s runner has got, counted against the runs of looks of
/// `context`, and how soon the claim is looked at again: soon while its runner is being
/// suspended ([`suspending_again`]), now and then otherwise, so a sandbox made since is
/// suspended too.
fn suspension_seen(
    context: &Context,
    claim: &Claim,
    suspension: Suspension,
) -> (Suspension, Duration) {
    let mut runs = context
        .suspending
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let at = ObjectRef::from_obj(claim);
    if suspension != Suspension::Suspending {
        runs.remove(&at);
        return (suspension, vouch::RECHECK);
    }
    let attempts = runs.entry(at).or_insert(0);
    *attempts = attempts.saturating_add(1);
    match suspending_again(*attempts) {
        Some(again) => (Suspension::Suspending, again),
        None => (Suspension::NotEnded, vouch::RECHECK),
    }
}

/// `next`, with each condition's transition time kept from `previous` while its status has not
/// changed, which is when Kubernetes' conventions say the time moves. A status that says the same
/// thing as before is then equal to it, and is not written again.
pub fn settled(previous: Option<&ClaimStatus>, mut next: ClaimStatus) -> ClaimStatus {
    for condition in &mut next.conditions {
        let before = previous.and_then(|status| {
            status
                .conditions
                .iter()
                .find(|c| c.type_ == condition.type_)
        });
        if let Some(before) = before
            && before.status == condition.status
        {
            condition.last_transition_time = before.last_transition_time.clone();
        }
    }
    next
}

struct Context {
    client: Client,
    /// What the operator signs each runner's link key with, as mounted now.
    keys: Arc<MountedKeys>,
    /// The cluster it runs in, which every statement names ([`cluster_of`]).
    cluster: String,
    /// What each person's runners may hold at once, in their runner namespace's quota.
    quota: Quota,
    /// Every Claim, as the operator's own watch holds them: whether a Claim being deleted is the
    /// last of its requester's on its pool.
    claims: Store<Claim>,
    /// How many times in a row each Claim met an object changed under it.
    conflicts: Mutex<HashMap<ObjectRef<Claim>, u32>>,
    /// Since when each Claim has waited for its requester's quota to count.
    uncounted: Mutex<HashMap<ObjectRef<Claim>, std::time::Instant>>,
    /// How many times in a row each stopped Claim's runner was found still being suspended.
    suspending: Mutex<HashMap<ObjectRef<Claim>, u32>>,
    /// What the operator counts for its metrics (#3737).
    metrics: Arc<Metrics>,
}

/// How long a Claim waits for its requester's quota to count before it says so.
pub const UNCOUNTED_FOR: Duration = Duration::from_secs(60);

/// One pass over `claim`. Whenever it ends without an error, the Claim's run of conflicts is
/// over, and the counts of Claims that are gone are dropped.
async fn reconcile(claim: Arc<Claim>, context: Arc<Context>) -> Result<Action, Error> {
    let done = reconcile_claim(claim.clone(), context.clone()).await?;
    let mut conflicts = context
        .conflicts
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    conflicts.remove(&ObjectRef::from_obj(&*claim));
    conflicts.retain(|claim, _| context.claims.get(claim).is_some());
    drop(conflicts);
    context
        .suspending
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .retain(|claim, _| context.claims.get(claim).is_some());
    Ok(done)
}

async fn reconcile_claim(claim: Arc<Claim>, context: Arc<Context>) -> Result<Action, Error> {
    let client = &context.client;
    let name = claim.name_any();
    let namespace = claim
        .namespace()
        .ok_or_else(|| Error::NoNamespace(name.clone()))?;
    // A Claim being deleted has what was made for it removed, whatever the policy now says of
    // it: a Claim made before the policy was in force never had anything made.
    if claim.metadata.deletion_timestamp.is_none() {
        let binding = Api::<ValidatingAdmissionPolicyBinding>::all(client.clone())
            .get_opt(POLICY)
            .await?;
        let admitted = admission(&claim, binding.as_ref());
        if let Some(refused) = admitted.status() {
            admitted.log(&name, &namespace);
            write_status(client, &claim, &namespace, refused).await?;
            return Ok(Action::await_change());
        }
    }
    let claims = Api::<Claim>::namespaced(client.clone(), &namespace);
    finalizer(&claims, person::FINALIZER, claim, |event| async move {
        match event {
            Event::Apply(claim) => apply(claim, &context, &namespace).await,
            Event::Cleanup(claim) => cleanup(&claim, &context).await,
        }
    })
    .await
    .map_err(Error::of_finalizer)
}

/// **`requester`'s runner namespace**, as the cluster holds it and as the policy on runner
/// namespaces lets the operator take it ([`person::runner_namespace_of`]), and when that policy
/// is in force from.
async fn runner_namespace_held(
    client: &Client,
    requester: &str,
) -> Result<(RunnerNamespace, Option<k8s_openapi::jiff::Timestamp>), Error> {
    let binding = Api::<ValidatingAdmissionPolicyBinding>::all(client.clone())
        .get_opt(RUNNER_NAMESPACES_POLICY)
        .await?;
    let since = in_force_since(binding.as_ref(), RUNNER_NAMESPACES_POLICY);
    let held = Api::<Namespace>::all(client.clone())
        .get_opt(&person::runner_namespace(requester))
        .await?;
    let now = k8s_openapi::jiff::Timestamp::now();
    Ok((
        person::runner_namespace_of(held, requester, since, now),
        since,
    ))
}

/// **`requester`'s runner namespace, made ready** (#2055): made where there is none, with the
/// role binding that gives them alone [`person::OWNER_ROLE`] and the quota `quota` asks for,
/// each made or brought back to what it should say. The quota as the cluster holds it, with
/// what it counts, comes back with the namespace; what keeps the operator from acting there
/// comes back instead when anything does.
async fn runner_namespace_ready(
    client: &Client,
    requester: &str,
    quota: &Quota,
) -> Result<Result<(Namespace, ResourceQuota), RunnerNamespace>, Error> {
    let runner = match runner_namespace_held(client, requester).await? {
        (RunnerNamespace::Theirs(runner), _) => *runner,
        // Made, not applied: a namespace someone made under the name since is refused, and the
        // next pass finds it in the way. What was made is acted in only once it reads as theirs.
        (RunnerNamespace::Missing, since) => {
            let made = Api::<Namespace>::all(client.clone())
                .create(
                    &PostParams {
                        field_manager: Some(FIELD_MANAGER.to_owned()),
                        ..PostParams::default()
                    },
                    &person::namespace_for(requester),
                )
                .await?;
            match person::runner_namespace_of(
                Some(made),
                requester,
                since,
                k8s_openapi::jiff::Timestamp::now(),
            ) {
                RunnerNamespace::Theirs(runner) => *runner,
                refused => return Ok(Err(refused)),
            }
        }
        (refused, _) => return Ok(Err(refused)),
    };
    let name = runner.name_any();
    let wanted = person::owner_binding(&name, requester);
    made_or_applied(
        &Api::<RoleBinding>::namespaced(client.clone(), &name),
        wanted.clone(),
        |held| person::binding_says(held, &wanted),
    )
    .await?;
    // Applied server-side from the first, so a key later left out of perPerson.quota is taken
    // out of every quota the operator applied it to.
    let wanted = person::quota_for(&name, quota);
    let api = Api::<ResourceQuota>::namespaced(client.clone(), &name);
    let held = match api.get_opt(QUOTA).await? {
        Some(held) if person::quota_says(&held, &wanted) => held,
        _ => {
            api.patch(
                QUOTA,
                &PatchParams::apply(FIELD_MANAGER).force(),
                &Patch::Apply(&wanted),
            )
            .await?
        }
    };
    Ok(Ok((runner, held)))
}

/// `wanted` as the cluster holds it: made where nothing holds its name, applied server-side
/// where what is held does not say what it should (`says`), and otherwise left as it is. For an
/// object whose fields are all atomic, as a role binding's are.
async fn made_or_applied<K>(api: &Api<K>, wanted: K, says: impl Fn(&K) -> bool) -> Result<K, Error>
where
    K: Resource + Clone + DeserializeOwned + Serialize + Debug,
{
    let name = wanted.name_any();
    match api.get_opt(&name).await? {
        None => Ok(api
            .create(
                &PostParams {
                    field_manager: Some(FIELD_MANAGER.to_owned()),
                    ..PostParams::default()
                },
                &wanted,
            )
            .await?),
        Some(held) if says(&held) => Ok(held),
        Some(_) => Ok(api
            .patch(
                &name,
                &PatchParams::apply(FIELD_MANAGER).force(),
                &Patch::Apply(&wanted),
            )
            .await?),
    }
}

/// **A pass over a Claim the operator acts on**, under its finalizer: its runner made, or kept,
/// in its requester's runner namespace, and its status written.
async fn apply(claim: Arc<Claim>, context: &Context, namespace: &str) -> Result<Action, Error> {
    let client = &context.client;
    let name = claim.name_any();
    let requester = &claim.spec.requester;
    let pool = Api::<Pool>::all(client.clone())
        .get_opt(&claim.spec.pool)
        .await?;
    let Some(pool) = pool else {
        // The runner it was given, if any, keeps running: the operator changes nothing for it,
        // and says what upstream says of it.
        let held = match runner_namespace_held(client, requester).await?.0 {
            RunnerNamespace::Theirs(runner) => {
                let upstream_claims =
                    Api::<SandboxClaim>::namespaced(client.clone(), &runner.name_any());
                upstream_claims.get_opt(&name).await?.filter(|held| {
                    matches!(
                        made_for_claim(Some(held.meta()), &runner, &claim),
                        Found::Ours { .. }
                    )
                })
            }
            _ => None,
        };
        let status = match held {
            Some(held) => {
                // Still stopped by its own claim (#2058).
                let runner = held.namespace().unwrap_or_default();
                let sandbox = sandbox_of(client, &runner, &held).await?;
                let idle = lifecycle::idle_condition(&claim).is_some()
                    || said_idle_of(client, &runner, sandbox.as_ref()).await?;
                if idle && lifecycle::idle_condition(&claim).is_none() {
                    let status = lifecycle::status_of_an_idle_runner(Suspension::Suspending);
                    write_status_saying(client, &claim, namespace, status, LinkKeyIs::Said).await?;
                }
                set_operating_mode(context, &claim, None, &held, sandbox.as_ref(), idle).await?;
                if upstream::is_stopped(&claim, None) {
                    let (suspension, again) =
                        suspension_seen(context, &claim, upstream::suspension_of(sandbox.as_ref()));
                    let status = status_of_a_stopped_claim(&claim, None, suspension);
                    write_status_saying(client, &claim, namespace, status, LinkKeyIs::Said).await?;
                    return Ok(Action::requeue(again));
                }
                let suspended = sandbox.as_ref().is_some_and(|sandbox| {
                    sandbox.spec.operating_mode.as_deref() == Some(upstream::SUSPENDED)
                });
                if suspended {
                    status_of_a_suspended_runner_of_a_deleted_pool(&claim)
                } else {
                    status_of_a_deleted_pool(&claim, &held)
                }
            }
            None => status_of_a_missing_pool(&claim),
        };
        write_status(client, &claim, namespace, status).await?;
        // A runner left running is looked at again, as it may say it is idle (#3980).
        return Ok(Action::requeue(vouch::RECHECK));
    };
    let (runner, quota) = match runner_namespace_ready(client, requester, &context.quota).await? {
        Ok(ready) => {
            // A person's namespace with a Claim on an opted-in Pool gets a way to the mint port
            // (#4206), for all their runners there and until the namespace goes: reaching the
            // port gets nothing without a proof and an opted-in Pool.
            if crate::forge::key_to_read(&pool.spec).is_some() {
                let wanted =
                    crate::mint::egress_policy(&ready.0.name_any(), client.default_namespace());
                made_or_applied(
                    &Api::<NetworkPolicy>::namespaced(client.clone(), &ready.0.name_any()),
                    wanted.clone(),
                    |held| held.spec == wanted.spec,
                )
                .await?;
            }
            ready
        }
        // The policy's binding is settling: nothing is made, and the claim is looked at again
        // once it is in force.
        Err(RunnerNamespace::Settling { until }) => {
            let wait = until
                .duration_since(k8s_openapi::jiff::Timestamp::now())
                .unsigned_abs()
                .min(vouch::RECHECK);
            tracing::info!(claim = %name, %namespace, ?wait, "waiting for the policy on runner namespaces to be in force");
            return Ok(Action::requeue(wait.max(Duration::from_secs(1))));
        }
        Err(refused) => {
            let runner = person::runner_namespace(requester);
            let status = match refused {
                RunnerNamespace::InTheWay { made_before_policy } => {
                    tracing::warn!(
                        claim = %name, %namespace, runner_namespace = %runner, made_before_policy,
                        "left as it is: the runner namespace's name is held by a namespace the operator may not use"
                    );
                    person::status_of_a_namespace_in_the_way(&runner, made_before_policy)
                }
                _ => {
                    tracing::warn!(
                        claim = %name, %namespace, policy = RUNNER_NAMESPACES_POLICY,
                        "not acted on: the policy on runner namespaces is not in force"
                    );
                    person::status_of_runner_namespaces_not_in_force()
                }
            };
            write_status_saying(client, &claim, namespace, status, LinkKeyIs::Said).await?;
            // Namespaces and that binding are not watched: looked at again now and then.
            return Ok(Action::requeue(vouch::RECHECK));
        }
    };
    let runner_name = runner.name_any();
    let templates = Api::<SandboxTemplate>::namespaced(client.clone(), &runner_name);
    let warm_pools = Api::<SandboxWarmPool>::namespaced(client.clone(), &runner_name);
    let upstream_claims = Api::<SandboxClaim>::namespaced(client.clone(), &runner_name);
    // All three are looked at before any is written: one in the way stops them all.
    let mut wanted = sandbox_claim_for(&claim, &pool.spec.kept_volume, &runner);
    let pool_object = upstream::name_for(&pool.name_any());
    let template_held = templates.get_opt(&pool_object).await?.map(meta);
    let warm_held = warm_pools.get_opt(&pool_object).await?.map(meta);
    let claim_full = upstream_claims.get_opt(&name).await?;
    // The volume an upstream claim was made with stays as it is: upstream never changes it, and
    // refuses an apply that does (#2057).
    upstream::keep_volumes_held(&mut wanted, claim_full.as_ref());
    let claim_held = claim_full.as_ref().map(|held| meta(held.clone()));
    let template_found = made_for(template_held.as_ref(), &runner);
    let warm_found = made_for(warm_held.as_ref(), &runner);
    let claim_found = made_for_claim(claim_held.as_ref(), &runner, &claim);
    // The pool's two objects are shared by the requester's Claims on it: each is applied only to
    // the version read, so a change made in the meantime is looked at again.
    let mut template = template_for(&pool, &runner_name, runner_owner(&runner));
    template.metadata.resource_version = template_held
        .as_ref()
        .and_then(|held| held.resource_version.clone());
    let mut warm = warm_pool_for(&pool, &runner_name, runner_owner(&runner));
    warm.metadata.resource_version = warm_held
        .as_ref()
        .and_then(|held| held.resource_version.clone());
    // Runners are kept warm only for a requester who has a fresh Claim on the pool that runs:
    // a kept or stopped Claim never takes one (#3737).
    let wanted_warm = warm_wanted(&claim, &pool, context.claims.state());
    warm.spec.replicas = upstream::replicas_kept(&pool, wanted_warm);
    let stopped = upstream::is_stopped(&claim, Some(&pool));
    let in_the_way = [
        (
            &template_found,
            SandboxTemplate::kind(&()),
            template.name_any(),
        ),
        (&warm_found, SandboxWarmPool::kind(&()), warm.name_any()),
        (&claim_found, SandboxClaim::kind(&()), wanted.name_any()),
    ]
    .into_iter()
    .find(|(found, _, _)| **found == Found::InTheWay);
    if let Some((_, kind, object)) = in_the_way {
        // A runner its claim already has is still suspended or run again, whatever is in the
        // way of the rest (#2058).
        if let (Found::Ours { .. }, Some(held)) = (&claim_found, claim_full.as_ref()) {
            let sandbox = sandbox_of(client, &runner_name, held).await?;
            let idle = lifecycle::idle_condition(&claim).is_some();
            set_operating_mode(context, &claim, Some(&pool), held, sandbox.as_ref(), idle).await?;
        }
        tracing::warn!(
            claim = %name, %namespace, runner_namespace = %runner_name, %kind, %object,
            "left as it is: an upstream object the operator did not make has the name it would use"
        );
        let status = status_of_an_object_in_the_way(&kind, &object, &runner_name);
        write_status(client, &claim, namespace, status).await?;
        return Ok(Action::await_change());
    }
    // A stopped claim that has no runner yet is not given one until it runs (#2058).
    if stopped && claim_found == Found::Nothing {
        write_ours(&templates, template, template_found).await?;
        write_ours(&warm_pools, warm, warm_found).await?;
        let status = status_of_a_stopped_claim(&claim, Some(&pool), Suspension::NotMade);
        write_status_saying(client, &claim, namespace, status, LinkKeyIs::Said).await?;
        return Ok(Action::await_change());
    }
    // A runner not asked for yet is asked for only within its requester's quota (#2055). A fresh
    // Claim on a pool that keeps runners warm takes one already counted, so it asks for nothing
    // more; one made cold instead is still held to the quota by the cluster (#3737).
    let takes_a_warm_runner = !claim.spec.kept && warm.spec.replicas > 0;
    if claim_found == Found::Nothing && !takes_a_warm_runner {
        let pod = template.spec.pod_template.spec.clone().unwrap_or_default();
        let storage = claim
            .spec
            .kept
            .then_some(pool.spec.kept_volume.size.as_str());
        if let Some(why) = person::over_quota(&quota, &person::asks_of(&pod, storage)) {
            return over_quota(client, &claim, namespace, &runner_name, &why).await;
        }
    }
    let written: Result<_, Error> = async {
        let template_now = write_ours(&templates, template, template_found).await?;
        write_ours(&warm_pools, warm, warm_found).await?;
        let applied = write_ours(&upstream_claims, wanted, claim_found).await?;
        Ok((template_now, applied))
    }
    .await;
    let (template_now, applied) = match written {
        Ok(written) => {
            context
                .uncounted
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .remove(&ObjectRef::from_obj(&*claim));
            written
        }
        // A quota just made has not counted yet: looked at again soon, saying nothing new.
        Err(Error::Kube(error)) if person::is_quota_not_counted_yet(&error) => {
            let since = *context
                .uncounted
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .entry(ObjectRef::from_obj(&*claim))
                .or_insert_with(std::time::Instant::now);
            if since.elapsed() >= UNCOUNTED_FOR {
                let status = person::status_of_an_uncounted_quota(&runner_name);
                write_status_saying(client, &claim, namespace, status, LinkKeyIs::Said).await?;
            }
            return Ok(Action::requeue(vouch::KEY_WAIT));
        }
        Err(Error::Kube(error)) if person::is_over_quota(&error) => {
            return over_quota(client, &claim, namespace, &runner_name, &error.to_string()).await;
        }
        // A statement the operator can no longer look at again goes (#2057): only a conflict,
        // which is tried again at once, leaves it.
        Err(error) => {
            if !is_conflict(&error) {
                let withdrawn = status_withdrawn(claim.status.as_ref(), &error.to_string());
                if let Err(unwritten) =
                    write_status_saying(client, &claim, namespace, withdrawn, LinkKeyIs::Said).await
                {
                    tracing::warn!(claim = %name, %namespace, error = %unwritten, "the statement could not be taken away");
                }
            }
            return Err(error);
        }
    };
    let sandbox = sandbox_of(client, &runner_name, &applied).await?;
    // An idle runner stays suspended until its desktop asks for it (#3980), and is run again only
    // once its pod has gone, so its pod's word that it was idle is not read a second time.
    let gone =
        sandbox.is_none() || upstream::suspension_of(sandbox.as_ref()) == Suspension::Suspended;
    let idle = lifecycle::idle_condition(&claim).is_some();
    // Said on the Claim first, so what runs it again is that word and not this pass's success.
    if !stopped && idle && lifecycle::asked_to_run_again(&claim) && gone {
        tracing::info!(claim = %name, %namespace, "its desktop asks for its idle runner to run again");
        let status = lifecycle::status_woken();
        write_status_exactly(client, &claim, namespace, status, LinkKeyIs::Said).await?;
        return Ok(Action::requeue(vouch::KEY_WAIT));
    }
    let stays_idle = !stopped && idle;
    set_operating_mode(
        context,
        &claim,
        Some(&pool),
        &applied,
        sandbox.as_ref(),
        stays_idle,
    )
    .await?;
    if stopped {
        let (suspension, again) =
            suspension_seen(context, &claim, upstream::suspension_of(sandbox.as_ref()));
        let status = status_of_a_stopped_claim(&claim, Some(&pool), suspension);
        write_status_saying(client, &claim, namespace, status, LinkKeyIs::Said).await?;
        return Ok(Action::requeue(again));
    }
    if stays_idle {
        let (suspension, again) =
            suspension_seen(context, &claim, upstream::suspension_of(sandbox.as_ref()));
        let status = lifecycle::status_of_an_idle_runner(suspension);
        write_status_saying(client, &claim, namespace, status, LinkKeyIs::Said).await?;
        return Ok(Action::requeue(again));
    }
    let (vouching, pod) = vouch_for(
        context,
        &claim,
        &pool,
        &applied,
        &template_now,
        sandbox.map(|sandbox| sandbox.metadata),
    )
    .await?;
    // The runner says it is idle (#3980): said on the Claim first, so a pass after a restart
    // knows, and suspended by the pass this write wakes.
    if pod
        .as_ref()
        .is_some_and(|pod| lifecycle::said_idle(pod, k8s_openapi::jiff::Timestamp::now()))
    {
        tracing::info!(claim = %name, %namespace, "its runner says it is idle");
        let status = lifecycle::status_of_an_idle_runner(Suspension::Suspending);
        write_status_saying(client, &claim, namespace, status, LinkKeyIs::Said).await?;
        return Ok(Action::requeue(vouch::KEY_WAIT));
    }
    if let Vouching::Refused(why) = &vouching {
        tracing::warn!(claim = %name, %namespace, %why, "no link key vouched for");
    }
    // A runner not ready yet says its stage (#2057).
    let mut status = stage::staged(status_of(Some(&applied)), pod.as_ref());
    status.conditions.push(vouching.condition());
    status.link_key = vouching.link_key();
    status.mint = crate::mint::said_on_a_claim(
        client,
        &pool,
        status.link_key.is_some(),
        &context.keys.now().current.public(),
    )
    .await;
    write_status_saying(client, &claim, namespace, status, LinkKeyIs::Said).await?;
    // The pod is not watched: a claim waiting for its key is looked at again soon, and one
    // vouched for or refused now and then, so a pod changed since is found.
    Ok(Action::requeue(vouching.again()))
}

/// `claim`'s status once its runner would take its requester over their quota in `runner`, and
/// `why`: no statement, and looked at again soon.
async fn over_quota(
    client: &Client,
    claim: &Claim,
    namespace: &str,
    runner: &str,
    why: &str,
) -> Result<Action, Error> {
    tracing::info!(claim = %claim.name_any(), %namespace, runner_namespace = %runner, %why, "over its requester's quota");
    let status = person::status_over_quota(runner, why);
    write_status_saying(client, claim, namespace, status, LinkKeyIs::Said).await?;
    Ok(Action::requeue(OVER_QUOTA_AGAIN))
}

/// Whether the objects of `claim`'s pool in its requester's runner namespace go with it: no other
/// Claim among `watched`, the ones the operator's watch holds, by the same requester on the same
/// pool is left, one being deleted aside. Two deleted at once both remove them.
pub fn pool_objects_go_with(claim: &Claim, watched: impl IntoIterator<Item = Arc<Claim>>) -> bool {
    !watched.into_iter().any(|other| {
        other.uid() != claim.uid()
            && other.metadata.deletion_timestamp.is_none()
            && other.spec.requester == claim.spec.requester
            && other.spec.pool == claim.spec.pool
    })
}

/// Whether the requester of `claim` wants runners of `pool` kept warm (#3737): `claim` or another
/// of theirs on `pool` among `watched`, not being deleted, is fresh, not stopped and not idle. A
/// kept Claim starts cold, and a stopped or idle one takes no runner, so none keeps one warm.
pub fn warm_wanted(
    claim: &Claim,
    pool: &Pool,
    watched: impl IntoIterator<Item = Arc<Claim>>,
) -> bool {
    let takes_one = |other: &Claim| {
        other.metadata.deletion_timestamp.is_none()
            && !other.spec.kept
            && !upstream::is_stopped(other, Some(pool))
            && lifecycle::idle_condition(other).is_none()
    };
    takes_one(claim)
        || watched.into_iter().any(|other| {
            other.uid() != claim.uid()
                && other.spec.requester == claim.spec.requester
                && other.spec.pool == claim.spec.pool
                && takes_one(&other)
        })
}

/// **What a deleted Claim's finalizer removes** (#2055): in its requester's runner namespace, the
/// upstream claim the operator made for it, which takes its runner with it, and, when it is the
/// last of theirs on its pool ([`pool_objects_go_with`]), the pool's template and warm pool there.
/// Each only when the operator made it, by uid. The namespace, its role binding and its quota
/// stay for the person's next Claim.
async fn cleanup(claim: &Claim, context: &Context) -> Result<Action, Error> {
    let client = &context.client;
    let held = Api::<Namespace>::all(client.clone())
        .get_opt(&person::runner_namespace(&claim.spec.requester))
        .await?;
    // Only what the operator made, in a namespace marked for this requester, is deleted; with
    // none there, the Claim goes and nothing else does.
    let Some(runner) = person::runner_to_clean(held, &claim.spec.requester) else {
        return Ok(Action::await_change());
    };
    let runner_name = runner.name_any();
    let upstream_claims = Api::<SandboxClaim>::namespaced(client.clone(), &runner_name);
    let held = upstream_claims
        .get_metadata_opt(&claim.name_any())
        .await?
        .map(|held| held.metadata);
    // Counted once, by the look that starts the runner's deletion (#3737): a Claim is looked at
    // again while its upstream claim is still going.
    let going = held
        .as_ref()
        .is_some_and(|held| held.deletion_timestamp.is_some());
    if let Found::Ours { uid } = made_for_claim(held.as_ref(), &runner, claim)
        && delete_by_uid(&upstream_claims, &claim.name_any(), uid).await?
        && !going
    {
        context.metrics.deleted(&claim.spec.pool, Cause::Requested);
    }
    // Every Claim known first, so one not read yet never lets the pool's objects go under it.
    let _ = context.claims.wait_until_ready().await;
    if pool_objects_go_with(claim, context.claims.state()) {
        let object = upstream::name_for(&claim.spec.pool);
        let templates = Api::<SandboxTemplate>::namespaced(client.clone(), &runner_name);
        let held = templates
            .get_metadata_opt(&object)
            .await?
            .map(|held| held.metadata);
        if let Found::Ours { uid } = made_for(held.as_ref(), &runner) {
            delete_by_uid(&templates, &object, uid).await?;
        }
        let warm_pools = Api::<SandboxWarmPool>::namespaced(client.clone(), &runner_name);
        let held = warm_pools
            .get_metadata_opt(&object)
            .await?
            .map(|held| held.metadata);
        if let Found::Ours { uid } = made_for(held.as_ref(), &runner) {
            delete_by_uid(&warm_pools, &object, uid).await?;
        }
    }
    tracing::info!(claim = %claim.name_any(), runner_namespace = %runner_name, "its runner removed");
    Ok(Action::await_change())
}

/// The object `name` deleted, only while it is the one of `uid`; one already gone is fine. Whether
/// the API server took the deletion (`false` when it was gone).
async fn delete_by_uid<K>(api: &Api<K>, name: &str, uid: String) -> Result<bool, Error>
where
    K: Resource + Clone + DeserializeOwned + Debug,
{
    let params = DeleteParams {
        preconditions: Some(Preconditions {
            uid: Some(uid),
            resource_version: None,
        }),
        ..DeleteParams::background()
    };
    match api.delete(name, &params).await {
        Ok(_) => Ok(true),
        Err(kube::Error::Api(status)) if status.code == 404 => Ok(false),
        Err(error) => Err(error.into()),
    }
}

/// The sandbox `upstream`, a Claim's upstream claim, names in its status, if there is one.
async fn sandbox_of(
    client: &Client,
    namespace: &str,
    upstream: &SandboxClaim,
) -> Result<Option<Sandbox>, Error> {
    let Some(named) = upstream
        .status
        .as_ref()
        .and_then(|status| status.sandbox.as_ref())
    else {
        return Ok(None);
    };
    Ok(Api::<Sandbox>::namespaced(client.clone(), namespace)
        .get_opt(&named.name)
        .await?)
}

/// Whether the pod `sandbox` records says its runner is idle (#3980).
async fn said_idle_of(
    client: &Client,
    namespace: &str,
    sandbox: Option<&Sandbox>,
) -> Result<bool, Error> {
    let Some(sandbox) = sandbox else {
        return Ok(false);
    };
    let pod = Api::<Pod>::namespaced(client.clone(), namespace)
        .get_opt(&vouch::pod_name_of(&sandbox.metadata))
        .await?;
    Ok(pod.is_some_and(|pod| lifecycle::said_idle(&pod, k8s_openapi::jiff::Timestamp::now())))
}

/// **A stopped or idle claim's runner suspended, and run again once it is not** (#2058, #3980):
/// `sandbox`'s operating mode applied as [`upstream::operating_mode_for`] says, only where the sandbox is
/// the one `upstream`, the claim's upstream claim, controls by uid. Its mode is the one field
/// the operator writes on it.
async fn set_operating_mode(
    context: &Context,
    claim: &Claim,
    pool: Option<&Pool>,
    upstream: &SandboxClaim,
    sandbox: Option<&Sandbox>,
    idle: bool,
) -> Result<(), Error> {
    let Some(sandbox) = sandbox else {
        return Ok(());
    };
    if !vouch::controlled_by(
        &sandbox.metadata,
        "extensions.agents.x-k8s.io",
        "SandboxClaim",
        &upstream.metadata,
    ) {
        return Ok(());
    }
    let stopped = upstream::is_stopped(claim, pool);
    let mode = if idle && !stopped {
        upstream::operating_mode_while_idle(sandbox)
    } else {
        upstream::operating_mode_for(claim, pool, sandbox)
    };
    let Some(mode) = mode else {
        return Ok(());
    };
    let (name, namespace) = (sandbox.name_any(), sandbox.namespace().unwrap_or_default());
    tracing::info!(claim = %claim.name_any(), %namespace, sandbox = %name, %mode, "the runner's mode set");
    Api::<Sandbox>::namespaced(context.client.clone(), &namespace)
        .patch(
            &name,
            &PatchParams::apply(FIELD_MANAGER).force(),
            &Patch::Apply(&upstream::sandbox_in_mode(sandbox, mode)),
        )
        .await?;
    if mode == upstream::SUSPENDED {
        let cause = if stopped { Cause::Stopped } else { Cause::Idle };
        context.metrics.suspended(&claim.spec.pool, cause);
    }
    Ok(())
}

/// **The operator's word on `claim`'s runner's link key** (#2056): the pod `sandbox`, the one
/// its upstream claim names, records ([`vouch::pod_name_of`]), read, then [`vouch::vouch`]; and
/// the pod, signed with the keys mounted now for the cluster `context` runs in.
async fn vouch_for(
    context: &Context,
    claim: &Claim,
    pool: &Pool,
    upstream: &SandboxClaim,
    template: &SandboxTemplate,
    sandbox: Option<ObjectMeta>,
) -> Result<(Vouching, Option<Pod>), Error> {
    // The runner is in its upstream claim's namespace, its requester's runner namespace.
    let namespace = upstream.namespace().unwrap_or_default();
    let pod = match &sandbox {
        Some(sandbox) => {
            Api::<Pod>::namespaced(context.client.clone(), &namespace)
                .get_opt(&vouch::pod_name_of(sandbox))
                .await?
        }
        None => None,
    };
    let wanted = upstream::template_for(pool, &namespace, Vec::new())
        .spec
        .pod_template;
    let vouching = vouch::vouch(
        &vouch::Held {
            now: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|since| since.as_secs())
                .unwrap_or_default(),
            cluster: &context.cluster,
            claim,
            upstream,
            template: Some(&template.metadata),
            template_spec: Some(&template.spec.pod_template),
            sandbox: sandbox.as_ref(),
            pod: pod.as_ref(),
        },
        &wanted,
        &context.keys.now(),
    );
    Ok((vouching, pod))
}

/// An object's metadata, which is all [`made_for`] and [`made_for_claim`] read of it.
fn meta<K: Resource>(object: K) -> ObjectMeta {
    object.meta().clone()
}

/// `wanted` written to the cluster as it was found: made when nothing holds its name, and
/// applied server-side to the object the operator made, by its uid (and the resource version
/// `wanted` names, if any), so an object made again or changed under that name since it was
/// looked at is refused and looked at again. Never called for an object in the way.
async fn write_ours<K>(api: &Api<K>, mut wanted: K, found: Found) -> Result<K, Error>
where
    K: Resource + Clone + DeserializeOwned + Serialize + Debug,
{
    let name = wanted.name_any();
    match found {
        // Made, not applied: if someone made an object under the name since it was looked at,
        // the cluster refuses this, and the next pass finds it in the way.
        Found::Nothing => Ok(api
            .create(
                &PostParams {
                    field_manager: Some(FIELD_MANAGER.to_owned()),
                    ..PostParams::default()
                },
                &wanted,
            )
            .await?),
        Found::Ours { uid } => {
            wanted.meta_mut().uid = Some(uid);
            Ok(api
                .patch(
                    &name,
                    &PatchParams::apply(FIELD_MANAGER).force(),
                    &Patch::Apply(&wanted),
                )
                .await?)
        }
        Found::InTheWay => Err(Error::InTheWay(name)),
    }
}

/// Whether a status written for a Claim says what becomes of its link key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkKeyIs {
    /// Left as it was: the pass did not look at the runner's key.
    Kept,
    /// Exactly the status's: none means the statement written before goes.
    Said,
}

/// The merge patch that brings a Claim's `previous` status to `next`, or none when it already
/// says that ([`settled`]). With [`LinkKeyIs::Said`] a status that names no link key writes
/// `linkKey: null`, so a statement that no longer holds is taken away (#2056), and one with no
/// next key's signature writes `nextSignature: null`, so a roll's second signature goes (#3305).
pub fn status_patch(
    previous: Option<&ClaimStatus>,
    next: ClaimStatus,
    link_key: LinkKeyIs,
) -> Option<serde_json::Value> {
    let mut status = settled(previous, next);
    if link_key == LinkKeyIs::Kept {
        status.link_key = previous.and_then(|held| held.link_key.clone());
        status.mint = previous.and_then(|held| held.mint.clone());
    }
    if previous == Some(&status) {
        return None;
    }
    let mut value = serde_json::to_value(&status).unwrap_or_default();
    if let Some(fields) = value.as_object_mut() {
        match fields
            .get_mut("linkKey")
            .and_then(serde_json::Value::as_object_mut)
        {
            // A statement no longer signed by a next key says so, so a roll's second
            // signature goes with the roll (#3305).
            Some(link_key) => {
                link_key
                    .entry("nextSignature")
                    .or_insert(serde_json::Value::Null);
            }
            None => {
                fields.insert("linkKey".into(), serde_json::Value::Null);
            }
        }
        // Where a runner asks goes with the statement, or with the Pool's opt-in (#4207).
        fields.entry("mint").or_insert(serde_json::Value::Null);
    }
    Some(serde_json::json!({ "status": value }))
}

/// `status` written on the Claim, unless it says what the Claim already says.
async fn write_status(
    client: &Client,
    claim: &Claim,
    namespace: &str,
    status: ClaimStatus,
) -> Result<(), Error> {
    write_status_saying(client, claim, namespace, status, LinkKeyIs::Kept).await
}

async fn write_status_saying(
    client: &Client,
    claim: &Claim,
    namespace: &str,
    status: ClaimStatus,
    link_key: LinkKeyIs,
) -> Result<(), Error> {
    let status = lifecycle::keeping_idle(claim.status.as_ref(), status);
    write_status_exactly(client, claim, namespace, status, link_key).await
}

/// [`write_status_saying`], with `status`'s `Idle` taken as it is.
async fn write_status_exactly(
    client: &Client,
    claim: &Claim,
    namespace: &str,
    status: ClaimStatus,
    link_key: LinkKeyIs,
) -> Result<(), Error> {
    if let Some(patch) = status_patch(claim.status.as_ref(), status, link_key) {
        Api::<Claim>::namespaced(client.clone(), namespace)
            .patch_status(
                &claim.name_any(),
                &PatchParams::default(),
                &Patch::Merge(patch),
            )
            .await?;
    }
    Ok(())
}

/// How long a claim that could not be reconciled waits before it is tried again.
const RETRY: Duration = Duration::from_secs(10);

/// The status code of a request refused because the object changed since it was read, or was
/// made since it was found missing.
const CONFLICT: u16 = 409;

/// Whether `error` is a conflict, which is tried again soon ([`retry_after_conflict`]).
fn is_conflict(error: &Error) -> bool {
    matches!(error, Error::Kube(kube::Error::Api(status)) if status.code == CONFLICT)
}

/// A Claim's status once the operator could not apply what it asks for, with `why`: what it
/// said before, but no statement, and `KeyPublished` saying why. A statement stays only while
/// the operator can look at it again.
pub fn status_withdrawn(previous: Option<&ClaimStatus>, why: &str) -> ClaimStatus {
    let refused = Vouching::Refused(format!(
        "what its claim asks for could not be applied ({why})"
    ));
    let mut conditions: Vec<_> = previous
        .map(|status| status.conditions.clone())
        .unwrap_or_default()
        .into_iter()
        .filter(|condition| condition.type_ != crate::resources::KEY_PUBLISHED)
        .collect();
    conditions.push(refused.condition());
    ClaimStatus {
        conditions,
        link_key: None,
        mint: None,
    }
}

/// How long a claim waits before it is tried again after its `attempts`-th conflict in a row:
/// one second, doubling each time, and never longer than [`RETRY`]. Claims sharing an object
/// take turns, and an object changed over and over does not keep them at one try a second.
pub fn retry_after_conflict(attempts: u32) -> Duration {
    Duration::from_secs(1)
        .saturating_mul(2u32.saturating_pow(attempts.saturating_sub(1)))
        .min(RETRY)
}

fn error_policy(claim: Arc<Claim>, error: &Error, context: Arc<Context>) -> Action {
    let retry = match error {
        error if is_conflict(error) => {
            let mut conflicts = context
                .conflicts
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let attempts = conflicts.entry(ObjectRef::from_obj(&*claim)).or_insert(0);
            *attempts = attempts.saturating_add(1);
            retry_after_conflict(*attempts)
        }
        _ => RETRY,
    };
    tracing::warn!(claim = %claim.name_any(), %error, ?retry, "trying again");
    Action::requeue(retry)
}

/// The Claims to look at again when `object`, a template or warm pool, changes or goes: every
/// Claim on the pool it is named for whose requester's runner namespace it is in, whoever made
/// it. A Claim that still needs it then makes it again, and one it was in the way of goes on
/// once it is gone.
pub fn claims_woken_by(
    object: &ObjectMeta,
    claims: impl IntoIterator<Item = Arc<Claim>>,
) -> Vec<ObjectRef<Claim>> {
    let Some(pool) = object.name.as_deref().and_then(upstream::pool_of) else {
        return Vec::new();
    };
    claims
        .into_iter()
        .filter(|claim| {
            claim.spec.pool == pool
                && object.namespace.as_deref()
                    == Some(person::runner_namespace(&claim.spec.requester).as_str())
        })
        .map(|claim| ObjectRef::from_obj(&*claim))
        .collect()
}

/// The Claims to look at again when `object`, an upstream claim, changes or goes: those among
/// `claims` of its name whose requester's runner namespace it is in. The one it was made for,
/// and one of the same name from another namespace that it was in the way of, which goes on
/// once it is gone.
pub fn claims_of_upstream(
    object: &ObjectMeta,
    claims: impl IntoIterator<Item = Arc<Claim>>,
) -> Vec<ObjectRef<Claim>> {
    claims
        .into_iter()
        .filter(|claim| {
            object.name.is_some()
                && claim.metadata.name == object.name
                && object.namespace.as_deref()
                    == Some(person::runner_namespace(&claim.spec.requester).as_str())
        })
        .map(|claim| ObjectRef::from_obj(&*claim))
        .collect()
}

/// The namespace whose uid names a cluster: every cluster has it, and it is made once, with the
/// cluster (#3305).
pub const CLUSTER_NAMESPACE: &str = "kube-system";

/// **The cluster the operator runs in** (#3305): the uid of its `kube-system` namespace, which
/// every Pool's status and every statement names, so a desktop refuses a statement made in
/// another cluster.
pub async fn cluster_of(client: &Client) -> Result<String, String> {
    let namespace = Api::<Namespace>::all(client.clone())
        .get(CLUSTER_NAMESPACE)
        .await
        .map_err(|error| format!("the namespace {CLUSTER_NAMESPACE} could not be read: {error}"))?;
    namespace
        .metadata
        .uid
        .filter(|uid| vouch::is_uid(uid))
        .ok_or_else(|| format!("the namespace {CLUSTER_NAMESPACE} has no uid purlis reads"))
}

/// Run the operator until the process is asked to stop (SIGTERM or Ctrl-C).
pub async fn run(client: Client, keys: MountedKeys, cluster: String, quota: Quota) {
    let keys = Arc::new(keys);
    let mint_keys = keys.clone();
    let mint_cluster = cluster.clone();
    let metrics = Arc::new(Metrics::default());
    tokio::select! {
        () = run_claims(client.clone(), keys.clone(), cluster.clone(), quota, metrics) => {}
        () = publish_pool_status(client.clone(), keys, cluster) => {}
        () = crate::forge::check_keys(client.clone(), vouch::RECHECK) => {}
        () = crate::mint::serve(client, mint_keys, mint_cluster) => {}
    }
    tracing::info!("stopped; upstream {} is pinned", upstream::VERSION);
}

/// **Every Pool's status names the operator's signing key and its cluster** (#2056, #3305), for a
/// desktop to pin when it adds the pool, and during a roll the next key with the current key's
/// word for it: written whenever a Pool says anything else, and looked at again every
/// [`vouch::RECHECK`], since the kubelet changes the mounted keys without a word to the operator.
async fn publish_pool_status(client: Client, keys: Arc<MountedKeys>, cluster: String) {
    let pools = Api::<Pool>::all(client);
    let mut events = watcher(pools.clone(), watcher::Config::default())
        .default_backoff()
        .applied_objects()
        .boxed();
    let mut every = tokio::time::interval(vouch::RECHECK);
    every.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        let looked_at = tokio::select! {
            event = events.next() => match event {
                None => return,
                Some(Ok(pool)) => vec![pool],
                Some(Err(error)) => {
                    tracing::warn!(%error, "the pools could not be watched; trying again");
                    continue;
                }
            },
            _ = every.tick() => match pools.list(&kube::api::ListParams::default()).await {
                Ok(all) => all.items,
                Err(error) => {
                    tracing::warn!(%error, "the pools could not be listed; trying again");
                    continue;
                }
            },
        };
        let keys = keys.now();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|since| since.as_secs())
            .unwrap_or_default();
        for pool in looked_at {
            let next = vouch::pool_status(&keys, &cluster, pool.status.as_ref(), now);
            let Some(patch) = pool_status_patch(pool.status.as_ref(), &next) else {
                continue;
            };
            let written = pools
                .patch_status(
                    &pool.name_any(),
                    &PatchParams::default(),
                    &Patch::Merge(patch),
                )
                .await;
            if let Err(error) = written {
                tracing::warn!(pool = %pool.name_any(), %error, "the pool's status was not written");
            }
        }
    }
}

/// The merge patch that brings a Pool's `previous` status to `next`, or none when it says that
/// already. A next key and its vouch no longer held are written `null`, so they go.
pub fn pool_status_patch(
    previous: Option<&PoolStatus>,
    next: &PoolStatus,
) -> Option<serde_json::Value> {
    if previous == Some(next) {
        return None;
    }
    let mut status = serde_json::to_value(next).unwrap_or_default();
    if let Some(fields) = status.as_object_mut() {
        for field in ["nextSigningKey", "nextSigningKeyVouch"] {
            fields.entry(field).or_insert(serde_json::Value::Null);
        }
    }
    Some(serde_json::json!({ "status": status }))
}

async fn run_claims(
    client: Client,
    keys: Arc<MountedKeys>,
    cluster: String,
    quota: Quota,
    metrics: Arc<Metrics>,
) {
    let controller = Controller::new(
        Api::<Claim>::all(client.clone()),
        watcher::Config::default(),
    );
    let claims = controller.store();
    let every_claim = controller.store();
    let known_claims = controller.store();
    let by_upstream_claim = controller.store();
    let by_template = controller.store();
    let by_warm_pool = controller.store();
    // Served on the operator's own port, from the Claims this watch holds (#3737).
    tokio::spawn(crate::metrics::serve(
        client.clone(),
        metrics.clone(),
        controller.store(),
    ));
    // Upstream's objects are watched by their metadata alone, whoever made them: each wakes the
    // Claims it is named for, so a Claim that still needs one it made makes it again at once,
    // and one an object was in the way of goes on as soon as it is gone. The API server sends
    // only their metadata (`PartialObjectMeta`), so no one's pod template reaches the operator.
    controller
        // A Claim's upstream claim has its name, in its requester's runner namespace.
        .watches(
            Api::<PartialObjectMeta<SandboxClaim>>::all(client.clone()),
            watcher::Config::default(),
            move |upstream| claims_of_upstream(&upstream.metadata, by_upstream_claim.state()),
        )
        .watches(
            Api::<PartialObjectMeta<SandboxTemplate>>::all(client.clone()),
            watcher::Config::default(),
            move |object| claims_woken_by(&object.metadata, by_template.state()),
        )
        .watches(
            Api::<PartialObjectMeta<SandboxWarmPool>>::all(client.clone()),
            watcher::Config::default(),
            move |object| claims_woken_by(&object.metadata, by_warm_pool.state()),
        )
        .watches(
            Api::<Pool>::all(client.clone()),
            watcher::Config::default(),
            move |pool| {
                let pool = pool.name_any();
                claims
                    .state()
                    .into_iter()
                    .filter(|claim| claim.spec.pool == pool)
                    .map(|claim| ObjectRef::from_obj(&*claim))
                    .collect::<Vec<_>>()
            },
        )
        // The policy's binding, by name: when it comes or goes, every Claim is looked at again.
        .watches(
            Api::<ValidatingAdmissionPolicyBinding>::all(client.clone()),
            watcher::Config::default().fields(&format!("metadata.name={POLICY}")),
            move |_| {
                every_claim
                    .state()
                    .into_iter()
                    .map(|claim| ObjectRef::from_obj(&*claim))
                    .collect::<Vec<_>>()
            },
        )
        .shutdown_on_signal()
        .run(
            reconcile,
            error_policy,
            Arc::new(Context {
                client,
                keys,
                cluster,
                quota,
                claims: known_claims,
                conflicts: Mutex::new(HashMap::new()),
                suspending: Mutex::new(HashMap::new()),
                uncounted: Mutex::new(HashMap::new()),
                metrics,
            }),
        )
        .for_each(|result| async move {
            match result {
                Ok((claim, _)) => tracing::info!(claim = %claim, "reconciled"),
                Err(error) => tracing::warn!(%error, "a claim was not reconciled"),
            }
        })
        .await;
}
