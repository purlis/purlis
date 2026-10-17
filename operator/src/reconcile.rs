// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The control loop: each Claim, whenever it, its upstream claim, its Pool or the policy's
//! binding changes, is brought to what [`crate::upstream`] says it asks for, and its status to
//! what upstream says of it. A Claim not admitted under the policy ([`crate::admission`]) is
//! never acted on: only its status says why.
//!
//! It changes only upstream objects it made ([`upstream::found`]). Each is read first: one it did
//! not make is left as it is, and nothing is written for the Claim until that object is gone.

use crate::admission::{POLICY, admission};
use crate::resources::{Claim, ClaimStatus, Pool, READY, condition};
use crate::upstream::{
    self, Found, SandboxClaim, SandboxTemplate, SandboxWarmPool, found, found_for_pool,
    owners_with, sandbox_claim_for, template_for, warm_pool_for,
};
use futures::StreamExt;
use k8s_openapi::api::admissionregistration::v1::ValidatingAdmissionPolicyBinding;
use kube::api::{Api, ObjectMeta, Patch, PatchParams, PostParams};
use kube::runtime::controller::{Action, Controller};
use kube::runtime::reflector::ObjectRef;
use kube::runtime::watcher;
use kube::{Client, Resource, ResourceExt};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::fmt::Debug;
use std::sync::Arc;
use std::time::Duration;

/// Who the operator's server-side applies are from.
pub const FIELD_MANAGER: &str = "purlis-cluster-operator";

/// Why a Claim could not be brought to what it asks for. It is tried again.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("the cluster refused or failed a request: {0}")]
    Kube(#[from] kube::Error),
    #[error("the claim {0} has no namespace")]
    NoNamespace(String),
    #[error("the upstream object {0} was not made by the operator")]
    InTheWay(String),
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
        conditions: vec![ready],
    }
}

/// A Claim's status when the pool it names does not exist.
pub fn status_of_a_missing_pool(claim: &Claim) -> ClaimStatus {
    ClaimStatus {
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
        conditions: vec![condition(status, "PoolDeleted", message)],
    }
}

/// A Claim's status when an upstream object the operator would make or change, the `kind` named
/// `name` in the Claim's namespace, is one it did not make: it is left as it is, and the Claim
/// waits until it is removed or renamed.
pub fn status_of_an_object_in_the_way(kind: &str, name: &str) -> ClaimStatus {
    ClaimStatus {
        conditions: vec![condition(
            "False",
            "UpstreamObjectInTheWay",
            format!(
                "The {kind} {name} in this namespace was not made by purlis's cluster operator, \
                 so it is left as it is. Remove or rename it, and this claim goes on."
            ),
        )],
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
}

async fn reconcile(claim: Arc<Claim>, context: Arc<Context>) -> Result<Action, Error> {
    let client = &context.client;
    let name = claim.name_any();
    let namespace = claim
        .namespace()
        .ok_or_else(|| Error::NoNamespace(name.clone()))?;
    let binding = Api::<ValidatingAdmissionPolicyBinding>::all(client.clone())
        .get_opt(POLICY)
        .await?;
    let admitted = admission(&claim, binding.as_ref());
    if let Some(refused) = admitted.status() {
        admitted.log(&name, &namespace);
        write_status(client, &claim, &namespace, refused).await?;
        return Ok(Action::await_change());
    }
    let pool = Api::<Pool>::all(client.clone())
        .get_opt(&claim.spec.pool)
        .await?;
    let status = match pool {
        // The runner it was given, if any, keeps running: the operator changes nothing for it,
        // and says what upstream says of it.
        None => {
            let upstream_claims = Api::<SandboxClaim>::namespaced(client.clone(), &namespace);
            match upstream_claims.get_opt(&name).await? {
                Some(held) if matches!(found(Some(held.meta()), &*claim), Found::Ours { .. }) => {
                    status_of_a_deleted_pool(&claim, &held)
                }
                _ => status_of_a_missing_pool(&claim),
            }
        }
        Some(pool) => {
            let templates = Api::<SandboxTemplate>::namespaced(client.clone(), &namespace);
            let warm_pools = Api::<SandboxWarmPool>::namespaced(client.clone(), &namespace);
            let upstream_claims = Api::<SandboxClaim>::namespaced(client.clone(), &namespace);
            // All three are looked at before any is written: one in the way stops them all.
            let wanted = sandbox_claim_for(&claim);
            let pool_object = upstream::name_for(&pool.name_any());
            let template_held = templates.get_opt(&pool_object).await?.map(meta);
            let warm_held = warm_pools.get_opt(&pool_object).await?.map(meta);
            let claim_held = upstream_claims.get_opt(&name).await?.map(meta);
            let template_found = found_for_pool(template_held.as_ref(), &pool);
            let warm_found = found_for_pool(warm_held.as_ref(), &pool);
            let claim_found = found(claim_held.as_ref(), &*claim);
            // The pool's two objects are shared by its Claims in the namespace, each adding
            // itself as an owner: each is applied only to the version read, so a Claim that
            // added itself in the meantime is never dropped.
            let mut template = template_for(
                &pool,
                &namespace,
                owners_with(template_held.as_ref(), &claim),
            );
            template.metadata.resource_version =
                template_held.and_then(|held| held.resource_version);
            let mut warm =
                warm_pool_for(&pool, &namespace, owners_with(warm_held.as_ref(), &claim));
            warm.metadata.resource_version = warm_held.and_then(|held| held.resource_version);
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
                tracing::warn!(
                    claim = %name, %namespace, %kind, %object,
                    "left as it is: an upstream object the operator did not make has the name it would use"
                );
                write_status(
                    client,
                    &claim,
                    &namespace,
                    status_of_an_object_in_the_way(&kind, &object),
                )
                .await?;
                return Ok(Action::requeue(LOOK_AGAIN));
            }
            put(&templates, template, template_found).await?;
            put(&warm_pools, warm, warm_found).await?;
            let applied = put(&upstream_claims, wanted, claim_found).await?;
            status_of(Some(&applied))
        }
    };
    write_status(client, &claim, &namespace, status).await?;
    Ok(Action::await_change())
}

/// An object's metadata, which is all [`found`] reads of it.
fn meta<K: Resource>(object: K) -> ObjectMeta {
    object.meta().clone()
}

/// `wanted` written to the cluster as it was `found`: made when nothing holds its name, and
/// applied server-side to the object the operator made, by its uid (and the resource version
/// `wanted` names, if any), so an object made again or changed under that name since it was
/// looked at is refused and looked at again. Never called for an object in the way.
async fn put<K>(api: &Api<K>, mut wanted: K, found: Found) -> Result<K, Error>
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

/// `status` written on the Claim, unless it says what the Claim already says.
async fn write_status(
    client: &Client,
    claim: &Claim,
    namespace: &str,
    status: ClaimStatus,
) -> Result<(), Error> {
    let status = settled(claim.status.as_ref(), status);
    if claim.status.as_ref() != Some(&status) {
        Api::<Claim>::namespaced(client.clone(), namespace)
            .patch_status(
                &claim.name_any(),
                &PatchParams::default(),
                &Patch::Merge(serde_json::json!({ "status": status })),
            )
            .await?;
    }
    Ok(())
}

/// How long a claim that could not be reconciled waits before it is tried again.
const RETRY: Duration = Duration::from_secs(10);

/// How often a claim an upstream object is in the way of looks again whether it is gone: the
/// operator does not watch objects it did not make.
const LOOK_AGAIN: Duration = Duration::from_secs(15);

/// How long a claim waits before it is tried again after an object changed under it between
/// reading and writing it: soon, from what the cluster holds now.
const READ_AGAIN: Duration = Duration::from_secs(1);

fn error_policy(claim: Arc<Claim>, error: &Error, _: Arc<Context>) -> Action {
    let retry = match error {
        Error::Kube(kube::Error::Api(status)) if status.code == 409 => READ_AGAIN,
        _ => RETRY,
    };
    tracing::warn!(claim = %claim.name_any(), %error, ?retry, "trying again");
    Action::requeue(retry)
}

/// Run the operator until the process is asked to stop (SIGTERM or Ctrl-C).
pub async fn run(client: Client) {
    let controller = Controller::new(
        Api::<Claim>::all(client.clone()),
        watcher::Config::default(),
    );
    let claims = controller.store();
    let every_claim = controller.store();
    controller
        .owns(
            Api::<SandboxClaim>::all(client.clone()),
            watcher::Config::default(),
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
        .run(reconcile, error_policy, Arc::new(Context { client }))
        .for_each(|result| async move {
            match result {
                Ok((claim, _)) => tracing::info!(claim = %claim, "reconciled"),
                Err(error) => tracing::warn!(%error, "a claim was not reconciled"),
            }
        })
        .await;
    tracing::info!("stopped; upstream {} is pinned", upstream::VERSION);
}
