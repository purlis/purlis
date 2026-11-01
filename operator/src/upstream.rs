// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The upstream kubernetes-sigs/agent-sandbox objects the operator writes, pinned to one tested
//! version: [`VERSION`], whose definitions are vendored verbatim in
//! `vendor/agent-sandbox/<version>/`. Only the fields purlis sets or reads are declared here, and
//! a test checks each of them against the vendored definitions, so moving the pin is a change a
//! test reads.
//!
//! A Pool becomes, in the runner namespace of each person with a Claim on it
//! ([`crate::person`]), a `SandboxTemplate` holding the runner's pod and a `SandboxWarmPool` of
//! it; a Claim becomes a `SandboxClaim` on that warm pool there. Each is controlled by the
//! runner namespace, which the operator made: a Claim in another namespace cannot own them. Upstream's controller adopts a warm runner when
//! there is one and otherwise makes one from the template.

use crate::person::CLAIM_UID_ANNOTATION;
use crate::resources::{Claim, KeptVolume, Pool};
use k8s_openapi::api::core::v1::{
    Capabilities, Container, EmptyDirVolumeSource, EnvVar, EnvVarSource, Namespace,
    ObjectFieldSelector, PersistentVolumeClaimSpec, PersistentVolumeClaimVolumeSource,
    PodSecurityContext, PodSpec, PodTemplateSpec, ResourceRequirements, SeccompProfile,
    SecurityContext, Volume, VolumeMount, VolumeResourceRequirements,
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

/// **The volume a runner's home is on** (#2057), in every pod of a pool: in memory's place, a
/// folder of the node's (`emptyDir`) for a fresh claim's runner, which keeps nothing past its
/// pod; for a kept claim's, the volume claim upstream makes from the claim's own template of
/// this name, `purlis-kept-<claim>` ([`KEPT_PREFIX`]), which outlives its pod. Upstream puts a
/// claim's volume in place of the template's volume of the same name.
pub const KEPT_VOLUME: &str = "purlis-kept";

/// The prefix of a kept claim's volume claim: upstream names it `<template>-<sandbox>`, and a
/// kept claim's sandbox has the claim's name.
pub const KEPT_PREFIX: &str = "purlis-kept-";

/// Where [`KEPT_VOLUME`] is mounted in both of a pod's containers.
pub const KEPT_FOLDER: &str = "/var/lib/purlis";

/// **The runner's home** (#2057): a folder on [`KEPT_VOLUME`], made by the pod's first step as a
/// copy of the image's own `HOME` the first time the volume is used, and the runner's `HOME`.
/// So a kept claim's runner keeps everything under it, its pairing too, when its pod is lost.
pub const RUNNER_HOME: &str = "/var/lib/purlis/home";

/// The group every process of a runner's pod is in, and that owns [`KEPT_VOLUME`] where the
/// volume's kind lets Kubernetes set its owner: so a runner image's user, whatever its number,
/// can make its home on a volume made for root (`fsGroup`). purlis's own image's group.
pub const VOLUME_GROUP: i64 = 10001;

/// The variable [`KEY_STEP`] reads its upstream claim's uid from (`purlis_core::runnerlink::CLAIM_VAR`).
pub const CLAIM_VAR: &str = "PURLIS_CLAIM_UID";

/// **The variable that marks a runner made to refuse any start outside the sandbox but a shell**
/// (#4207, the person's ruling of 2026-10-31), set from the pod's birth on a Pool naming an App's key
/// (`purlis_core::forge::mintport::MARK_VAR`). A pod's containers never change, so the operator can
/// read that it has held since its first moment; nothing a runner says could tell it that.
pub const MINTING_MARK: &str = "PURLIS_MINTING_POOL";

/// What a runner's host is told its pool's `idleAfter` in (#3980): an environment variable, which
/// a runner image from before it ignores, where a word on its command line would end it. The same
/// name as `crates/purlis-core/src/clusterpool/idle.rs`.
pub const IDLE_AFTER_VAR: &str = "PURLIS_IDLE_AFTER";

/// **The variable naming the image a runner runs** (`purlis_core::sandbox::tools::POOL_IMAGE_VAR`,
/// #4354): said back to the person if a declared tool's program is not in the image.
pub const POOL_IMAGE_VAR: &str = "PURLIS_POOL_IMAGE";

/// What [`KEY_STEP`] runs, after the image's own entrypoint (`purlis`).
pub fn key_step_args() -> Vec<String> {
    [
        "bridge",
        "--make-key",
        &format!("{LINK_FOLDER}/link-key"),
        "--publish",
        TERMINATION_LOG,
        "--home",
        RUNNER_HOME,
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
    /// Whether a claim may add a volume claim template of its own: `Allowed`, so a kept claim
    /// adds [`KEPT_VOLUME`]'s, and none may replace one of the template's (it holds none).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volume_claim_templates_policy: Option<String>,
}

/// The runners upstream keeps started ahead of a claim, for one template.
#[derive(CustomResource, Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[kube(
    group = "extensions.agents.x-k8s.io",
    version = "v1beta1",
    kind = "SandboxWarmPool",
    namespaced,
    status = "SandboxWarmPoolStatus"
)]
#[serde(rename_all = "camelCase")]
pub struct SandboxWarmPoolSpec {
    pub replicas: i32,
    pub sandbox_template_ref: Reference,
    /// How upstream treats the runners it keeps warm once the template changes: always
    /// [`RECREATE`], so a warm runner made from an earlier template is replaced and never handed
    /// to a fresh claim, whose runner the operator would not vouch for (#2058).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub update_strategy: Option<UpdateStrategy>,
}

/// What upstream says about a warm pool: purlis reads how many of its runners are ready, for its
/// metrics (#3737).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SandboxWarmPoolStatus {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ready_replicas: Option<i32>,
}

/// A warm pool's update strategy.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct UpdateStrategy {
    #[serde(rename = "type")]
    pub type_: String,
}

/// Upstream's update strategy that deletes a warm runner no longer of its template, and has a
/// claim pass over one not deleted yet (agent-sandbox v1.0.6, `isSandboxStale`).
pub const RECREATE: &str = "Recreate";

/// Upstream's sandbox, which makes and owns one runner's pod: purlis reads its metadata, to
/// follow a claim to its pod by uid (#2056), its operating mode, which it writes to suspend a
/// stopped runner and run it again (#2058), and its `Suspended` condition. Its pod template is
/// never read.
#[derive(CustomResource, Serialize, Deserialize, Clone, Debug, Default, PartialEq, JsonSchema)]
#[kube(
    group = "agents.x-k8s.io",
    version = "v1beta1",
    kind = "Sandbox",
    namespaced,
    status = "SandboxStatus"
)]
#[serde(rename_all = "camelCase")]
pub struct SandboxSpec {
    /// `Running` (upstream's default where it is not set) or `Suspended`: upstream ends a
    /// suspended sandbox's pod and keeps its volumes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operating_mode: Option<String>,
}

/// What upstream says about a sandbox: purlis reads its `Suspended` condition.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, JsonSchema)]
pub struct SandboxStatus {
    #[serde(default)]
    pub conditions: Vec<Condition>,
}

/// A sandbox's mode that runs its pod.
pub const RUNNING: &str = "Running";

/// A sandbox's mode that ends its pod and keeps its volumes, and the condition upstream says
/// whether it has.
pub const SUSPENDED: &str = "Suspended";

/// Whether `claim` is stopped, by its own `stopped` or by its `pool`'s, where its pool still
/// exists (#2058): a runner kept after its pool is deleted is stopped by its claim alone.
pub fn is_stopped(claim: &Claim, pool: Option<&Pool>) -> bool {
    claim.spec.stopped || pool.is_some_and(|pool| pool.spec.stopped)
}

/// **The mode the operator puts `claim`'s runner's `sandbox` in**, if it changes it (#2058):
/// [`SUSPENDED`] while the claim or its pool (if it still exists) is stopped, and [`RUNNING`]
/// again once neither is (never while its pool is deleted), where the operator's own apply
/// suspended it, so a runner someone
/// suspended by hand is left suspended. That record is best effort (D-2058-2): a manager's name
/// is the client's to give, and a runner suspended by hand after the operator suspended it
/// changes no field's owner, so it runs again with the rest. `None` leaves the sandbox as it is.
pub fn operating_mode_for(
    claim: &Claim,
    pool: Option<&Pool>,
    sandbox: &Sandbox,
) -> Option<&'static str> {
    let mode = sandbox.spec.operating_mode.as_deref().unwrap_or(RUNNING);
    if is_stopped(claim, pool) {
        return (mode != SUSPENDED).then_some(SUSPENDED);
    }
    // With its pool deleted, a runner is only ever suspended: one its pool's stopped suspended
    // stays so, since nothing is left to set the pool running again (#2058).
    pool?;
    (mode == SUSPENDED && suspended_by_the_operator(&sandbox.metadata)).then_some(RUNNING)
}

/// The mode the operator puts an idle runner's `sandbox` in, if it changes it (#3980).
pub fn operating_mode_while_idle(sandbox: &Sandbox) -> Option<&'static str> {
    (sandbox.spec.operating_mode.as_deref() != Some(SUSPENDED)).then_some(SUSPENDED)
}

/// Whether the operator's own server-side apply owns `sandbox`'s operating mode.
fn suspended_by_the_operator(sandbox: &ObjectMeta) -> bool {
    sandbox.managed_fields.iter().flatten().any(|entry| {
        entry.manager.as_deref() == Some(crate::reconcile::FIELD_MANAGER)
            && entry.operation.as_deref() == Some("Apply")
            && entry
                .fields_v1
                .as_ref()
                .is_some_and(|fields| fields.0.pointer("/f:spec/f:operatingMode").is_some())
    })
}

/// What the operator applies to `held`, a sandbox as it read it, to put it in `mode`: its
/// operating mode and nothing else, with its uid as a precondition, so a sandbox made again
/// under the name since it was read is refused.
pub fn sandbox_in_mode(held: &Sandbox, mode: &str) -> Sandbox {
    Sandbox {
        metadata: ObjectMeta {
            name: held.metadata.name.clone(),
            namespace: held.metadata.namespace.clone(),
            uid: held.metadata.uid.clone(),
            ..ObjectMeta::default()
        },
        spec: SandboxSpec {
            operating_mode: Some(mode.to_owned()),
        },
        status: None,
    }
}

/// How far a stopped claim's runner is suspended (#2058).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Suspension {
    /// No runner was asked for: none is made while the claim is stopped.
    NotMade,
    /// Asked to suspend, or to be once its sandbox is made, and upstream does not say yet that
    /// its pod has ended.
    Suspending,
    /// Upstream says its pod has ended (its `Suspended` condition is `True`).
    Suspended,
    /// Asked to suspend a while ago, and upstream still does not say its pod has ended.
    NotEnded,
}

/// How far the runner whose sandbox is `sandbox`, if it has one yet, is suspended.
pub fn suspension_of(sandbox: Option<&Sandbox>) -> Suspension {
    let suspended = sandbox
        .and_then(|sandbox| sandbox.status.as_ref())
        .is_some_and(|status| {
            status
                .conditions
                .iter()
                .any(|condition| condition.type_ == SUSPENDED && condition.status == "True")
        });
    if suspended {
        Suspension::Suspended
    } else {
        Suspension::Suspending
    }
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
    /// The volume claims upstream makes for this claim's sandbox: a kept claim's
    /// [`KEPT_VOLUME`]. A claim with one is never given a warm runner: upstream makes its
    /// sandbox cold, under the claim's name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volume_claim_templates: Option<Vec<VolumeClaimTemplate>>,
}

/// A volume claim upstream makes for a sandbox, named `<name>-<sandbox>`, owned by the sandbox.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct VolumeClaimTemplate {
    pub metadata: VolumeClaimName,
    pub spec: PersistentVolumeClaimSpec,
}

/// The name of a [`VolumeClaimTemplate`], which is also its volume's in the pod.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct VolumeClaimName {
    pub name: String,
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

/// **Who controls what the operator makes in a runner namespace**: the namespace itself, by its
/// uid, which the operator made for its person ([`crate::person::is_theirs`]). It never blocks
/// its own deletion: that would need update rights on the namespace's finalizers wherever a
/// cluster enforces owner-reference permissions.
pub fn runner_owner(runner: &Namespace) -> Vec<OwnerReference> {
    runner
        .controller_owner_ref(&())
        .map(|owner| OwnerReference {
            block_owner_deletion: None,
            ..owner
        })
        .into_iter()
        .collect()
}

/// Whether `held`, the upstream claim the cluster holds under `claim`'s name in its runner
/// namespace `runner`, is the one the operator made for `claim`: controlled by `runner`
/// ([`made_for`]), and naming `claim`'s uid. Another Claim's of the same name, from another
/// namespace, is in the way.
pub fn made_for_claim(held: Option<&ObjectMeta>, runner: &Namespace, claim: &Claim) -> Found {
    let found = made_for(held, runner);
    let names_it = held
        .and_then(|held| held.annotations.as_ref())
        .and_then(|annotations| annotations.get(CLAIM_UID_ANNOTATION))
        .is_some_and(|uid| Some(uid) == claim.metadata.uid.as_ref());
    match found {
        Found::Ours { .. } if !names_it => Found::InTheWay,
        found => found,
    }
}

fn owned_by(pool: &Pool, namespace: &str, owners: Vec<OwnerReference>) -> ObjectMeta {
    ObjectMeta {
        name: Some(name_for(&pool.name_any())),
        namespace: Some(namespace.to_owned()),
        owner_references: Some(owners),
        ..ObjectMeta::default()
    }
}

/// What each container of a runner's pod asks for at least, so a person's quota can count it
/// (#2055): the runner's CPU and memory, and its first step's.
pub const RUNNER_REQUESTS: (&str, &str) = ("250m", "512Mi");
pub const KEY_STEP_REQUESTS: (&str, &str) = ("50m", "32Mi");

fn requests((cpu, memory): (&str, &str)) -> ResourceRequirements {
    ResourceRequirements {
        requests: Some(
            [
                ("cpu".to_owned(), Quantity(cpu.to_owned())),
                ("memory".to_owned(), Quantity(memory.to_owned())),
            ]
            .into_iter()
            .collect(),
        ),
        ..ResourceRequirements::default()
    }
}

/// The template of a pool's runners in `namespace`, controlled by `owners` ([`runner_owner`]): one
/// container running the pool's image with its own entrypoint (`purlis serve` under an init,
/// the runner image contract), as a user who is not root, with no added privilege, no
/// Kubernetes credential and no service addresses, its home on [`KEPT_VOLUME`]. One template
/// serves a pool's kept and fresh claims alike, so a fresh claim can take a warm runner of it.
pub fn template_for(pool: &Pool, namespace: &str, owners: Vec<OwnerReference>) -> SandboxTemplate {
    // Both containers mount the folder the key is handed over in and the volume the runner's
    // home is on (#2056, #2057).
    let mounts = || {
        Some(vec![
            VolumeMount {
                name: LINK_VOLUME.to_owned(),
                mount_path: LINK_FOLDER.to_owned(),
                ..VolumeMount::default()
            },
            VolumeMount {
                name: KEPT_VOLUME.to_owned(),
                mount_path: KEPT_FOLDER.to_owned(),
                ..VolumeMount::default()
            },
        ])
    };
    let runner = Container {
        name: RUNNER_CONTAINER.to_owned(),
        image: Some(pool.spec.image.clone()),
        // Its home is on the kept volume (#2057); the first step made it from the image's.
        env: Some(
            std::iter::once(EnvVar {
                name: "HOME".to_owned(),
                value: Some(RUNNER_HOME.to_owned()),
                ..EnvVar::default()
            })
            .chain(std::iter::once(EnvVar {
                name: IDLE_AFTER_VAR.to_owned(),
                value: Some(pool.spec.idle_after.clone()),
                ..EnvVar::default()
            }))
            .chain(std::iter::once(EnvVar {
                name: POOL_IMAGE_VAR.to_owned(),
                value: Some(pool.spec.image.clone()),
                ..EnvVar::default()
            }))
            .chain(pool.spec.forge.is_some().then(|| EnvVar {
                name: MINTING_MARK.to_owned(),
                value: Some("1".to_owned()),
                ..EnvVar::default()
            }))
            .collect(),
        ),
        volume_mounts: mounts(),
        resources: Some(requests(RUNNER_REQUESTS)),
        security_context: Some(confined()),
        ..Container::default()
    };
    // The pod's first step makes the runner's link key, and publishes it as its termination
    // message (#2056).
    let key_step = Container {
        name: KEY_STEP.to_owned(),
        image: Some(pool.spec.image.clone()),
        args: Some(key_step_args()),
        // The upstream claim the pod is for, from the label upstream gives it: the home on the
        // volume is made for it, and taken only for it (#2057).
        env: Some(vec![EnvVar {
            name: CLAIM_VAR.to_owned(),
            value_from: Some(EnvVarSource {
                field_ref: Some(ObjectFieldSelector {
                    api_version: Some("v1".to_owned()),
                    field_path: format!("metadata.labels['{}']", crate::vouch::CLAIM_UID_LABEL),
                }),
                ..EnvVarSource::default()
            }),
            ..EnvVar::default()
        }]),
        volume_mounts: mounts(),
        termination_message_path: Some(TERMINATION_LOG.to_owned()),
        termination_message_policy: Some("File".to_owned()),
        resources: Some(requests(KEY_STEP_REQUESTS)),
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
                    volumes: Some(vec![
                        Volume {
                            name: LINK_VOLUME.to_owned(),
                            empty_dir: Some(EmptyDirVolumeSource {
                                medium: Some("Memory".to_owned()),
                                size_limit: Some(Quantity("1Mi".to_owned())),
                            }),
                            ..Volume::default()
                        },
                        // A fresh claim's home, as big as a kept one's; a kept claim's volume
                        // takes its place (#2057).
                        Volume {
                            name: KEPT_VOLUME.to_owned(),
                            empty_dir: Some(EmptyDirVolumeSource {
                                medium: None,
                                size_limit: Some(Quantity(pool.spec.kept_volume.size.clone())),
                            }),
                            ..Volume::default()
                        },
                    ]),
                    security_context: Some(PodSecurityContext {
                        fs_group: Some(VOLUME_GROUP),
                        fs_group_change_policy: Some("OnRootMismatch".to_owned()),
                        ..PodSecurityContext::default()
                    }),
                    automount_service_account_token: Some(false),
                    enable_service_links: Some(false),
                    ..PodSpec::default()
                }),
            },
            volume_claim_templates_policy: Some("Allowed".to_owned()),
        },
    }
}

/// **The pod upstream makes for `claim`'s runner from `template`**, the operator's template
/// for its pool, when its sandbox is named `sandbox` (#2057): the template's pod, and for a
/// kept claim its volume claim in place of the template's [`KEPT_VOLUME`], last among the
/// volumes, as upstream puts it (agent-sandbox v1.0.6, `MergeVolumeClaimVolumes`). `None` for a
/// kept claim whose sandbox is not named as the claim: its volume claim would not be
/// `purlis-kept-<claim>`.
pub fn pod_for(template: &PodTemplateSpec, claim: &Claim, sandbox: &str) -> Option<PodSpec> {
    let mut spec = template.spec.clone().unwrap_or_default();
    if !claim.spec.kept {
        return Some(spec);
    }
    if sandbox != claim.name_any() {
        return None;
    }
    let mut volumes = spec.volumes.take().unwrap_or_default();
    volumes.retain(|volume| volume.name != KEPT_VOLUME);
    volumes.push(Volume {
        name: KEPT_VOLUME.to_owned(),
        persistent_volume_claim: Some(PersistentVolumeClaimVolumeSource {
            claim_name: kept_volume_name(claim),
            ..PersistentVolumeClaimVolumeSource::default()
        }),
        ..Volume::default()
    });
    spec.volumes = Some(volumes);
    Some(spec)
}

/// The name of a kept claim's volume claim: `purlis-kept-<claim>`.
pub fn kept_volume_name(claim: &Claim) -> String {
    format!("{KEPT_PREFIX}{}", claim.name_any())
}

/// The volume claim template a kept claim's upstream claim carries, as `volume` asks: one
/// volume, written to by one node at a time.
pub fn kept_volume_template(volume: &KeptVolume) -> VolumeClaimTemplate {
    VolumeClaimTemplate {
        metadata: VolumeClaimName {
            name: KEPT_VOLUME.to_owned(),
        },
        spec: PersistentVolumeClaimSpec {
            access_modes: Some(vec!["ReadWriteOnce".to_owned()]),
            resources: Some(VolumeResourceRequirements {
                requests: Some(
                    [("storage".to_owned(), Quantity(volume.size.clone()))]
                        .into_iter()
                        .collect(),
                ),
                ..VolumeResourceRequirements::default()
            }),
            storage_class_name: volume.storage_class_name.clone(),
            ..PersistentVolumeClaimSpec::default()
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

/// A pool's warm pool in `namespace`, controlled by `owners` ([`runner_owner`]). It keeps no runner
/// started ahead of a claim until the operator sets its count ([`replicas_kept`]).
pub fn warm_pool_for(pool: &Pool, namespace: &str, owners: Vec<OwnerReference>) -> SandboxWarmPool {
    SandboxWarmPool {
        metadata: owned_by(pool, namespace, owners),
        spec: SandboxWarmPoolSpec {
            replicas: 0,
            sandbox_template_ref: Reference {
                name: name_for(&pool.name_any()),
            },
            update_strategy: Some(UpdateStrategy {
                type_: RECREATE.to_owned(),
            }),
        },
        status: None,
    }
}

/// `wanted`, with the volume claim templates of `held`, the upstream claim as the cluster holds
/// it, once there is one: upstream makes a sandbox's volumes once, and its definitions refuse a
/// change to them. So a Pool's `keptVolume` applies to the kept Claims made after it changes.
pub fn keep_volumes_held(wanted: &mut SandboxClaim, held: Option<&SandboxClaim>) {
    if let Some(held) = held {
        wanted
            .spec
            .volume_claim_templates
            .clone_from(&held.spec.volume_claim_templates);
    }
}

/// How many runners `pool`'s warm pool in a requester's runner namespace is to keep warm: its
/// `warmReplicas` (#3737) where the requester `wants` them ([`crate::warm_wanted`]), and none
/// while the pool is stopped (#2058). The count is the operator's own: a warm pool scaled by
/// hand is brought back to it at the next pass.
pub fn replicas_kept(pool: &Pool, wants: bool) -> i32 {
    if pool.spec.stopped || !wants {
        0
    } else {
        pool.spec.warm_replicas.max(0)
    }
}

/// The upstream claim a Claim becomes: of the same name, in its requester's runner namespace
/// `runner`, on its pool's warm pool there, controlled by `runner` and naming the Claim by its
/// uid ([`made_for_claim`]); the Claim's finalizer deletes it, and its runner with it
/// (#2055). A kept Claim's carries its volume, as `volume` (its pool's) asks (#2057), so
/// upstream makes it a runner of its own, cold; a fresh one takes a warm runner where the warm
/// pool holds one.
pub fn sandbox_claim_for(claim: &Claim, volume: &KeptVolume, runner: &Namespace) -> SandboxClaim {
    SandboxClaim {
        metadata: ObjectMeta {
            name: Some(claim.name_any()),
            namespace: runner.metadata.name.clone(),
            owner_references: Some(runner_owner(runner)),
            annotations: claim
                .metadata
                .uid
                .clone()
                .map(|uid| [(CLAIM_UID_ANNOTATION.to_owned(), uid)].into()),
            ..ObjectMeta::default()
        },
        spec: SandboxClaimSpec {
            warm_pool_ref: Reference {
                name: name_for(&claim.spec.pool),
            },
            volume_claim_templates: claim.spec.kept.then(|| vec![kept_volume_template(volume)]),
        },
        status: None,
    }
}
