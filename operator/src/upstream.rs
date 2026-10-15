// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The upstream kubernetes-sigs/agent-sandbox objects the operator writes, pinned to one tested
//! version: [`VERSION`], whose definitions are vendored verbatim in
//! `vendor/agent-sandbox/<version>/`. Only the fields purlis sets or reads are declared here, and
//! a test checks each of them against the vendored definitions, so moving the pin is a change a
//! test reads.
//!
//! A Pool becomes, in each namespace one of its Claims is in, a `SandboxTemplate` holding the
//! runner's pod and a `SandboxWarmPool` of it; a Claim becomes a `SandboxClaim` on that warm
//! pool. Upstream's controller adopts a warm runner when there is one and otherwise makes one
//! from the template.

use crate::resources::{Claim, Pool};
use k8s_openapi::api::core::v1::{
    Capabilities, Container, PodSpec, PodTemplateSpec, SeccompProfile, SecurityContext,
};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{Condition, ObjectMeta};
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

fn owned_by_the_pool(pool: &Pool, namespace: &str) -> ObjectMeta {
    ObjectMeta {
        name: Some(name_for(&pool.name_any())),
        namespace: Some(namespace.to_owned()),
        owner_references: pool.controller_owner_ref(&()).map(|owner| vec![owner]),
        ..ObjectMeta::default()
    }
}

/// The template of a pool's runners in `namespace`: one container running the pool's image with
/// its own entrypoint (`purlis serve` under an init, the runner image contract), as a user who is
/// not root, with no added privilege, no Kubernetes credential and no service addresses.
pub fn template_for(pool: &Pool, namespace: &str) -> SandboxTemplate {
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
        metadata: owned_by_the_pool(pool, namespace),
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

/// A pool's warm pool in `namespace`. It keeps no runner started ahead of a claim: every claim
/// starts one from the template.
pub fn warm_pool_for(pool: &Pool, namespace: &str) -> SandboxWarmPool {
    SandboxWarmPool {
        metadata: owned_by_the_pool(pool, namespace),
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
            owner_references: claim.controller_owner_ref(&()).map(|owner| vec![owner]),
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
