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
    Capabilities, Container, EmptyDirVolumeSource, PodSpec, PodTemplateSpec, SeccompProfile,
    SecurityContext, Volume, VolumeMount,
};
use k8s_openapi::apimachinery::pkg::api::resource::Quantity;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{Condition, ObjectMeta, OwnerReference};
use kube::{CustomResource, Resource, ResourceExt};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The upstream release the operator is tested against and the vendored definitions are from.
pub const VERSION: &str = "v1.0.6";

/// The image of upstream's controller, without a tag or digest, as the vendored manifest names it.
pub const CONTROLLER_IMAGE: &str = "registry.k8s.io/agent-sandbox/agent-sandbox-controller";

/// The digest of [`CONTROLLER_IMAGE`] at [`VERSION`], as Kubernetes' image promoter published it
/// (kubernetes/k8s.io, `registry.k8s.io/images/k8s-staging-agent-sandbox/images.yaml`). The
/// install names the controller by it: `vendor/agent-sandbox/<version>/kustomization.yaml`
/// records it beside the vendored manifest, and a test holds the two equal.
pub const CONTROLLER_DIGEST: &str =
    "sha256:14c479c9e89ca87d05dfcf597840473c0196d6e6eec622a7959e72108d75b24e";

/// The name of the container that runs `purlis serve` in a runner's pod.
pub const RUNNER_CONTAINER: &str = "runner";

/// **The runner pod's first step** (#2056): it runs before anything else in the pod, makes the
/// runner's link key, hands its private half to the runner over [`LINK_FOLDER`] and publishes
/// its public half as its termination message, for the operator to vouch for
/// ([`crate::vouch`]).
pub const KEY_STEP: &str = "link-key";

/// The folder in memory, shared by the pod's two containers alone, the key is handed over in.
/// Its path is the runner image contract's (docs/cluster-operator.md); the desktop's pairing
/// names the key there.
pub const LINK_FOLDER: &str = "/run/purlis-link";

/// The volume [`LINK_FOLDER`] is.
const LINK_VOLUME: &str = "purlis-link";

/// Where a container's termination message is written, which the cluster then shows in the
/// pod's status: Kubernetes' own default, named so a change of default never moves it.
pub const TERMINATION_LOG: &str = "/dev/termination-log";

/// **The name servers the pinned upstream gives a runner's pod**, with `dnsPolicy: None`, when it
/// manages the sandbox's network, its default: public resolvers, not the cluster's. Measured on
/// the kind run of v1.0.6; moving the pin moves it.
pub const MANAGED_NAME_SERVERS: [&str; 2] = ["8.8.8.8", "1.1.1.1"];

/// What [`KEY_STEP`] runs, after the image's own entrypoint (`purlis`).
pub fn key_step_args() -> Vec<String> {
    [
        "bridge",
        "--make-key",
        &format!("{LINK_FOLDER}/link-key"),
        "--publish",
        TERMINATION_LOG,
    ]
    .map(str::to_owned)
    .to_vec()
}

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

/// Upstream's sandbox, which makes and owns one runner's pod: purlis reads only its metadata,
/// to follow a claim to its pod by uid (#2056).
#[derive(CustomResource, Serialize, Deserialize, Clone, Debug, Default, PartialEq, JsonSchema)]
#[kube(
    group = "agents.x-k8s.io",
    version = "v1beta1",
    kind = "Sandbox",
    namespaced
)]
pub struct SandboxSpec {}

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

/// What upstream says about its claim: purlis reads its `Ready` condition, and the sandbox it
/// made, whose pod has its name.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SandboxClaimStatus {
    #[serde(default)]
    pub conditions: Vec<Condition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<Reference>,
}

/// A reference to an object of the same namespace, by name.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Reference {
    pub name: String,
}

/// The name of a pool's template and warm pool in a namespace. Prefixed, so an upstream object a
/// person made by hand in their namespace is never taken for purlis's.
pub fn name_for(pool: &str) -> String {
    format!("{POOL_PREFIX}{pool}")
}

const POOL_PREFIX: &str = "purlis-pool-";

/// The pool whose template or warm pool `name` would be, if it is named for one ([`name_for`]).
pub fn pool_of(name: &str) -> Option<&str> {
    name.strip_prefix(POOL_PREFIX)
        .filter(|pool| !pool.is_empty())
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
pub fn made_for<K>(held: Option<&ObjectMeta>, owner: &K) -> Found
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

/// Whether `reference` is to a purlis Claim, by its kind and API version.
pub fn is_purlis_claim(reference: &OwnerReference) -> bool {
    reference.kind == Claim::kind(&()) && reference.api_version == Claim::api_version(&())
}

/// Whether `reference` names one of `claims`, the Claims the operator knows: a purlis Claim of
/// that name and uid in `namespace`, on the pool named `pool`.
fn names_a_known_claim(
    reference: &OwnerReference,
    namespace: Option<&str>,
    pool: &str,
    claims: &[&Claim],
) -> bool {
    is_purlis_claim(reference)
        && claims.iter().any(|claim| {
            claim.metadata.uid.as_deref() == Some(reference.uid.as_str())
                && claim.metadata.name.as_deref() == Some(reference.name.as_str())
                && claim.metadata.namespace.as_deref() == namespace
                && claim.spec.pool == pool
        })
}

/// The owners of a pool's template or warm pool in a namespace once `claim` is among them: the
/// Claims among `claims`, the ones the operator knows, that own `held` (the object as the
/// cluster holds it), and `claim`, in a stable order. None is its controller: the garbage
/// collector removes the object once every one of them is gone, and not before, whatever
/// becomes of the pool, so a runner whose pool is deleted keeps its template and what upstream
/// makes of the template for its pods. Each Claim adds itself; any other owner is dropped.
pub fn owners_with(
    held: Option<&ObjectMeta>,
    claim: &Claim,
    claims: &[&Claim],
) -> Vec<OwnerReference> {
    let namespace = claim.metadata.namespace.as_deref();
    let mut owners: Vec<OwnerReference> = held
        .and_then(|held| held.owner_references.as_ref())
        .into_iter()
        .flatten()
        .filter(|owner| names_a_known_claim(owner, namespace, &claim.spec.pool, claims))
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
/// in a namespace, is one the operator made: one owned by a Claim among `claims`, the ones the
/// operator knows, in that namespace on that pool, by name and uid; or one `pool` itself
/// controls, as the operator made them before its Claims owned them. Anything else is in the
/// way, whatever it says of its owners.
pub fn made_for_pool(held: Option<&ObjectMeta>, pool: &Pool, claims: &[&Claim]) -> Found {
    let Some(held) = held else {
        return Found::Nothing;
    };
    let pool_name = pool.name_any();
    let namespace = held.namespace.as_deref();
    let by_a_known_claim = held
        .owner_references
        .iter()
        .flatten()
        .any(|reference| names_a_known_claim(reference, namespace, &pool_name, claims));
    match (&held.uid, by_a_known_claim) {
        (Some(uid), true) => Found::Ours { uid: uid.clone() },
        _ => made_for(Some(held), pool),
    }
}

/// The names of the Claims among `claims`, the ones the operator knows in `held`'s namespace on
/// `pool`, that own `held`, sorted, when they are all its owners; none when it has any other
/// owner, or none at all, since deleting those Claims would not free it then. For an object
/// [`made_for_pool`] finds in the way when given only the Claims the operator acts on, these are
/// Claims it now refuses: the object was made for them, and goes with them.
pub fn claims_holding(held: &ObjectMeta, pool: &Pool, claims: &[&Claim]) -> Vec<String> {
    let pool_name = pool.name_any();
    let namespace = held.namespace.as_deref();
    let owners = held.owner_references.as_deref().unwrap_or_default();
    if !owners
        .iter()
        .all(|reference| names_a_known_claim(reference, namespace, &pool_name, claims))
    {
        return Vec::new();
    }
    let mut names: Vec<String> = owners
        .iter()
        .map(|reference| reference.name.clone())
        .collect();
    names.sort();
    names.dedup();
    names
}

/// The template of a pool's runners in `namespace`, owned by `owners` ([`owners_with`]): one
/// container running the pool's image with its own entrypoint (`purlis serve` under an init,
/// the runner image contract), as a user who is not root, with no added privilege, no
/// Kubernetes credential and no service addresses.
pub fn template_for(pool: &Pool, namespace: &str, owners: Vec<OwnerReference>) -> SandboxTemplate {
    let link_folder = || {
        Some(vec![VolumeMount {
            name: LINK_VOLUME.to_owned(),
            mount_path: LINK_FOLDER.to_owned(),
            ..VolumeMount::default()
        }])
    };
    let runner = Container {
        name: RUNNER_CONTAINER.to_owned(),
        image: Some(pool.spec.image.clone()),
        volume_mounts: link_folder(),
        security_context: Some(confined()),
        ..Container::default()
    };
    // The pod's first step makes the runner's link key, and publishes it as its termination
    // message (#2056).
    let key_step = Container {
        name: KEY_STEP.to_owned(),
        image: Some(pool.spec.image.clone()),
        args: Some(key_step_args()),
        volume_mounts: link_folder(),
        termination_message_path: Some(TERMINATION_LOG.to_owned()),
        termination_message_policy: Some("File".to_owned()),
        security_context: Some(confined()),
        ..Container::default()
    };
    SandboxTemplate {
        metadata: owned_by(pool, namespace, owners),
        spec: SandboxTemplateSpec {
            pod_template: PodTemplateSpec {
                metadata: None,
                spec: Some(PodSpec {
                    init_containers: Some(vec![key_step]),
                    containers: vec![runner],
                    volumes: Some(vec![Volume {
                        name: LINK_VOLUME.to_owned(),
                        empty_dir: Some(EmptyDirVolumeSource {
                            medium: Some("Memory".to_owned()),
                            size_limit: Some(Quantity("1Mi".to_owned())),
                        }),
                        ..Volume::default()
                    }]),
                    automount_service_account_token: Some(false),
                    enable_service_links: Some(false),
                    ..PodSpec::default()
                }),
            },
        },
    }
}

/// How every container of a runner's pod is confined: a user who is not root, every capability
/// dropped, no privilege escalation, the runtime's default seccomp profile.
fn confined() -> SecurityContext {
    SecurityContext {
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
    }
}

/// A pool's warm pool in `namespace`, owned by `owners` ([`owners_with`]). It keeps no runner
/// started ahead of a claim: every claim starts one from the template.
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
