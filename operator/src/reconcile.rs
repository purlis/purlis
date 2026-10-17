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
    self, Found, SandboxClaim, SandboxTemplate, SandboxWarmPool, found, sandbox_claim_for,
    template_for, warm_pool_for,
};
use futures::StreamExt;
use k8s_openapi::api::admissionregistration::v1::ValidatingAdmissionPolicyBinding;
use kube::api::{Api, Patch, PatchParams, PostParams};
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
        None => status_of_a_missing_pool(&claim),
        Some(pool) => {
            let templates = Api::<SandboxTemplate>::namespaced(client.clone(), &namespace);
            let warm_pools = Api::<SandboxWarmPool>::namespaced(client.clone(), &namespace);
            let upstream_claims = Api::<SandboxClaim>::namespaced(client.clone(), &namespace);
            let template = template_for(&pool, &namespace);
            let warm = warm_pool_for(&pool, &namespace);
            let wanted = sandbox_claim_for(&claim);
            // All three are looked at before any is written: one in the way stops them all.
            let template_found = look(&templates, &template, &pool).await?;
            let warm_found = look(&warm_pools, &warm, &pool).await?;
            let claim_found = look(&upstream_claims, &wanted, &*claim).await?;
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

/// What the cluster holds under the name of `wanted`, an upstream object the operator would make
/// for `owner`, read from the API server itself.
async fn look<K, O>(api: &Api<K>, wanted: &K, owner: &O) -> Result<Found, Error>
where
    K: Resource + Clone + DeserializeOwned + Debug,
    O: Resource,
    O::DynamicType: Default,
{
    let held = api.get_opt(&wanted.name_any()).await?;
    Ok(found(held.as_ref().map(|held| held.meta()), owner))
}

/// `wanted` written to the cluster as [`look`] found it: made when nothing holds its name, and
/// applied server-side to the object the operator made, by its uid, so an object made again under
/// that name since it was looked at is refused and looked at again. Never called for an object
/// in the way.
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

fn error_policy(claim: Arc<Claim>, error: &Error, _: Arc<Context>) -> Action {
    tracing::warn!(claim = %claim.name_any(), %error, retry = ?RETRY, "trying again");
    Action::requeue(RETRY)
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
