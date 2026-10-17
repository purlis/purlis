// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The upstream kubernetes-sigs/agent-sandbox objects the operator writes, pinned to one tested
//! version: [`VERSION`], whose definitions are vendored verbatim in
//! `vendor/agent-sandbox/<version>/`. Only the fields purlis sets or reads are declared here, and
//! a test checks each of them against the vendored definitions, so moving the pin is a change a
//! test reads.
//!
//! A Pool becomes, in each namespace one of its Claims is in, a `SandboxTemplate` holding the
//! runner's pod and a `SandboxWarmPool` of it, both owned by the pool's Claims there; a Claim
//! becomes a `SandboxClaim` on that warm pool. Upstream's controller adopts a warm runner when
//! there is one and otherwise makes one from the template.

use crate::resources::{Claim, Pool};
use k8s_openapi::api::core::v1::{
    Capabilities, Container, PodSpec, PodTemplateSpec, SeccompProfile, SecurityContext,
};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{Condition, ObjectMeta, OwnerReference};
use kube::{CustomResource, Resource, ResourceExt};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The upstream release the operator is tested against and the vendored definitions are from.
pub const VERSION: &str = "v1.0.6";

/// The name of the container that runs `purlis serve` in a runner's pod.
pub const RUNNER_CONTAINER: &str = "runner";

/// A runner's pod, as upstream keeps it for a pool.
#[derive(CustomResource, Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[kube(
    group = "extensions.agents.x-k8s.io",
    version = "v1beta1",
    kind = "SandboxTemplate",
    namespaced
)]
#[serde(rename_all = "camelCase")]
pub struct SandboxTemplateSpec {
    pub pod_template: PodTemplateSpec,
}

/// The runners upstream keeps started ahead of a claim, for one template.
#[derive(CustomResource, Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[kube(
    group = "extensions.agents.x-k8s.io",
    version = "v1beta1",
    kind = "SandboxWarmPool",
    namespaced
)]
#[serde(rename_all = "camelCase")]
pub struct SandboxWarmPoolSpec {
    pub replicas: i32,
    pub sandbox_template_ref: Reference,
}

/// Upstream's claim of one runner from a warm pool. Upstream's controller writes its status.
#[derive(CustomResource, Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[kube(
    group = "extensions.agents.x-k8s.io",
    version = "v1beta1",
    kind = "SandboxClaim",
    namespaced,
    status = "SandboxClaimStatus"
)]
#[serde(rename_all = "camelCase")]
pub struct SandboxClaimSpec {
    pub warm_pool_ref: Reference,
}

/// What upstream says about its claim: purlis reads its `Ready` condition.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SandboxClaimStatus {
    #[serde(default)]
    pub conditions: Vec<Condition>,
}

/// A reference to an object of the same namespace, by name.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Reference {
    pub name: String,
}

/// The name of a pool's template and warm pool in a namespace. Prefixed, so an upstream object a
/// person made by hand in their namespace is never taken for purlis's.
pub fn name_for(pool: &str) -> String {
    format!("purlis-pool-{pool}")
}

/// What the cluster holds under the name an upstream object of the operator's takes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Found {
    /// Nothing: the operator makes it.
    Nothing,
    /// An object the operator made, with this uid: the operator applies to it.
    Ours { uid: String },
    /// An object the operator did not make: it is left as it is.
    InTheWay,
}

/// Whether `held`, the object the cluster holds under the name the operator would use, is one
/// the operator made for `owner`: it names `owner` as its controller, by uid. Anything else is
/// in the way, however it is named or owned.
pub fn found<K>(held: Option<&ObjectMeta>, owner: &K) -> Found
where
    K: Resource,
    K::DynamicType: Default,
{
    let Some(held) = held else {
        return Found::Nothing;
    };
    let owner_uid = owner.meta().uid.as_deref().filter(|uid| !uid.is_empty());
    let controlled = held.owner_references.iter().flatten().any(|reference| {
        reference.controller == Some(true)
            && Some(reference.uid.as_str()) == owner_uid
            && reference.kind == K::kind(&K::DynamicType::default())
    });
    match (&held.uid, controlled) {
        (Some(uid), true) => Found::Ours { uid: uid.clone() },
        _ => Found::InTheWay,
    }
}

/// The one owner of a Claim's upstream claim: its controller, which never blocks its own
/// deletion. Blocking it would need update rights on the owner's finalizers wherever a cluster
/// enforces owner-reference permissions; the garbage collector removes the object either way.
fn owners(owner: Option<OwnerReference>) -> Option<Vec<OwnerReference>> {
    owner.map(|owner| {
        vec![OwnerReference {
            block_owner_deletion: None,
            ..owner
        }]
    })
}

/// The owners of a pool's template or warm pool in a namespace once `claim` is among them: the
/// purlis Claims that own `held`, the object as the cluster holds it, and `claim`, in a stable
/// order. None is its controller: the garbage collector removes the object once every one of
/// them is gone, and not before, whatever becomes of the pool, so a runner whose pool is deleted
/// keeps its template and what upstream makes of the template for its pods. Each Claim adds
/// itself; the garbage collector drops a Claim that is gone.
pub fn owners_with(held: Option<&ObjectMeta>, claim: &Claim) -> Vec<OwnerReference> {
    let claim_kind = Claim::kind(&());
    let claim_version = Claim::api_version(&());
    let mut owners: Vec<OwnerReference> = held
        .and_then(|held| held.owner_references.as_ref())
        .into_iter()
        .flatten()
        .filter(|owner| owner.kind == claim_kind && owner.api_version == claim_version)
        .filter(|owner| Some(&owner.uid) != claim.metadata.uid.as_ref())
        .cloned()
        .chain(claim.controller_owner_ref(&()))
        .map(|owner| OwnerReference {
            controller: None,
            block_owner_deletion: None,
            ..owner
        })
        .collect();
    owners.sort_by(|a, b| (&a.name, &a.uid).cmp(&(&b.name, &b.uid)));
    owners.dedup_by(|a, b| a.uid == b.uid);
    owners
}

fn owned_by(pool: &Pool, namespace: &str, owners: Vec<OwnerReference>) -> ObjectMeta {
    ObjectMeta {
        name: Some(name_for(&pool.name_any())),
        namespace: Some(namespace.to_owned()),
        owner_references: Some(owners),
        ..ObjectMeta::default()
    }
}

/// Whether `held`, the object the cluster holds under the name of `pool`'s template or warm pool
/// in a namespace, is one the operator made: one owned by a purlis Claim, whichever Claims own it
/// now, or controlled by `pool` itself, as the operator made them before its Claims owned them.
/// Anything else is in the way.
pub fn found_for_pool(held: Option<&ObjectMeta>, pool: &Pool) -> Found {
    let Some(held) = held else {
        return Found::Nothing;
    };
    let claim_kind = Claim::kind(&());
    let claim_version = Claim::api_version(&());
    let by_a_claim =
        held.owner_references.iter().flatten().any(|reference| {
            reference.kind == claim_kind && reference.api_version == claim_version
        });
    match (&held.uid, by_a_claim) {
        (Some(uid), true) => Found::Ours { uid: uid.clone() },
        _ => found(Some(held), pool),
    }
}

/// The template of a pool's runners in `namespace`, owned by `owners` ([`owners_with`]): one container running the pool's image with
/// its own entrypoint (`purlis serve` under an init, the runner image contract), as a user who is
/// not root, with no added privilege, no Kubernetes credential and no service addresses.
pub fn template_for(pool: &Pool, namespace: &str, owners: Vec<OwnerReference>) -> SandboxTemplate {
    let runner = Container {
        name: RUNNER_CONTAINER.to_owned(),
        image: Some(pool.spec.image.clone()),
        security_context: Some(SecurityContext {
            run_as_non_root: Some(true),
            allow_privilege_escalation: Some(false),
            capabilities: Some(Capabilities {
                drop: Some(vec!["ALL".to_owned()]),
                add: None,
            }),
            seccomp_profile: Some(SeccompProfile {
                type_: "RuntimeDefault".to_owned(),
                localhost_profile: None,
            }),
            ..SecurityContext::default()
        }),
        ..Container::default()
    };
    SandboxTemplate {
        metadata: owned_by(pool, namespace, owners),
        spec: SandboxTemplateSpec {
            pod_template: PodTemplateSpec {
                metadata: None,
                spec: Some(PodSpec {
                    containers: vec![runner],
                    automount_service_account_token: Some(false),
                    enable_service_links: Some(false),
                    ..PodSpec::default()
                }),
            },
        },
    }
}

/// A pool's warm pool in `namespace`, owned by `owners` ([`owners_with`]). It keeps no runner started ahead of a claim: every claim
/// starts one from the template.
pub fn warm_pool_for(pool: &Pool, namespace: &str, owners: Vec<OwnerReference>) -> SandboxWarmPool {
    SandboxWarmPool {
        metadata: owned_by(pool, namespace, owners),
        spec: SandboxWarmPoolSpec {
            replicas: 0,
            sandbox_template_ref: Reference {
                name: name_for(&pool.name_any()),
            },
        },
    }
}

/// The upstream claim a Claim becomes: same name and namespace, on its pool's warm pool, and
/// owned by the Claim, so deleting the Claim deletes it and its runner.
pub fn sandbox_claim_for(claim: &Claim) -> SandboxClaim {
    SandboxClaim {
        metadata: ObjectMeta {
            name: Some(claim.name_any()),
            namespace: claim.namespace(),
            owner_references: owners(claim.controller_owner_ref(&())),
            ..ObjectMeta::default()
        },
        spec: SandboxClaimSpec {
            warm_pool_ref: Reference {
                name: name_for(&claim.spec.pool),
            },
        },
        status: None,
    }
}
