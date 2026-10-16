// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The control loop: each Claim, whenever it, its upstream claim, its Pool or the policy's
//! binding changes, is brought to what [`crate::upstream`] says it asks for, and its status to
//! what upstream says of it. A Claim not admitted under the policy ([`crate::admission`]) is
//! never acted on: only its status says why.

use crate::admission::{Admission, POLICY, admission};
use crate::resources::{Claim, ClaimStatus, Pool};
use crate::upstream::{
    self, SandboxClaim, SandboxTemplate, SandboxWarmPool, sandbox_claim_for, template_for,
    warm_pool_for,
};
use futures::StreamExt;
use k8s_openapi::api::admissionregistration::v1::ValidatingAdmissionPolicyBinding;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{Condition, Time};
use kube::api::{Api, Patch, PatchParams};
use kube::runtime::controller::{Action, Controller};
use kube::runtime::reflector::ObjectRef;
use kube::runtime::watcher;
use kube::{Client, ResourceExt};
use std::sync::Arc;
use std::time::Duration;

/// Who the operator's server-side applies are from.
pub const FIELD_MANAGER: &str = "purlis-cluster-operator";

const READY: &str = "Ready";

/// Why a Claim could not be brought to what it asks for. It is tried again.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("the cluster refused or failed a request: {0}")]
    Kube(#[from] kube::Error),
    #[error("the claim {0} has no namespace")]
    NoNamespace(String),
}

pub(crate) fn condition(status: &str, reason: &str, message: String) -> Condition {
    Condition {
        type_: READY.to_owned(),
        status: status.to_owned(),
        reason: reason.to_owned(),
        message,
        last_transition_time: Time(k8s_openapi::jiff::Timestamp::now()),
        observed_generation: None,
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
        match admitted {
            Admission::NoPolicy => tracing::warn!(
                claim = %name, %namespace, policy = POLICY,
                "not acted on: the policy on claims is not in force"
            ),
            _ => tracing::warn!(
                claim = %name, %namespace, policy = POLICY,
                "refused: the claim was made before the policy on claims was in force"
            ),
        }
        write_status(client, &claim, &namespace, refused).await?;
        return Ok(Action::await_change());
    }
    let apply = PatchParams::apply(FIELD_MANAGER).force();
    let pool = Api::<Pool>::all(client.clone())
        .get_opt(&claim.spec.pool)
        .await?;
    let status = match pool {
        None => status_of_a_missing_pool(&claim),
        Some(pool) => {
            let template = template_for(&pool, &namespace);
            Api::<SandboxTemplate>::namespaced(client.clone(), &namespace)
                .patch(&template.name_any(), &apply, &Patch::Apply(&template))
                .await?;
            let warm = warm_pool_for(&pool, &namespace);
            Api::<SandboxWarmPool>::namespaced(client.clone(), &namespace)
                .patch(&warm.name_any(), &apply, &Patch::Apply(&warm))
                .await?;
            let wanted = sandbox_claim_for(&claim);
            let applied = Api::<SandboxClaim>::namespaced(client.clone(), &namespace)
                .patch(&name, &apply, &Patch::Apply(&wanted))
                .await?;
            status_of(Some(&applied))
        }
    };
    write_status(client, &claim, &namespace, status).await?;
    Ok(Action::await_change())
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
