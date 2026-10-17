// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! purlis's own objects, `v1alpha1`: no stability promise before upstream agent-sandbox leaves
//! beta (spec #2041). `deploy/crds.yaml` is generated from these types, and a test fails until
//! it is regenerated.

use k8s_openapi::apimachinery::pkg::apis::meta::v1::{Condition, Time};
use kube::{CustomResource, KubeSchema};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Where a team's runners come from. Cluster-wide, defined once for the team. Who may claim from
/// it is the cluster's own access control: a Claim is admitted only for someone allowed to `use`
/// the Pool (deploy/policy.yaml).
#[derive(CustomResource, Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[kube(
    group = "runners.purlis.dev",
    version = "v1alpha1",
    kind = "Pool",
    doc = "Where a team's purlis runners come from (purlis's cluster operator).",
    printcolumn = r#"{"name":"Image","type":"string","jsonPath":".spec.image"}"#,
    printcolumn = r#"{"name":"Age","type":"date","jsonPath":".metadata.creationTimestamp"}"#
)]
#[serde(rename_all = "camelCase")]
pub struct PoolSpec {
    /// The runner image, by digest (`<repository>@sha256:<digest>`), since a tag can move. Any
    /// image that keeps purlis's runner image contract (docs/runner-image.md).
    #[schemars(regex(pattern = r"^[^\s@]+@sha256:[0-9a-f]{64}$"))]
    pub image: String,
}

/// One person's request for a runner from a pool, and the runner it is given.
#[derive(CustomResource, KubeSchema, Serialize, Deserialize, Clone, Debug, PartialEq)]
#[kube(
    group = "runners.purlis.dev",
    version = "v1alpha1",
    kind = "Claim",
    namespaced,
    status = "ClaimStatus",
    doc = "A person's request for a purlis runner from a pool (purlis's cluster operator).",
    printcolumn = r#"{"name":"Pool","type":"string","jsonPath":".spec.pool"}"#,
    printcolumn = r#"{"name":"Ready","type":"string","jsonPath":".status.conditions[?(@.type=='Ready')].status"}"#,
    printcolumn = r#"{"name":"Reason","type":"string","jsonPath":".status.conditions[?(@.type=='Ready')].reason"}"#,
    printcolumn = r#"{"name":"Age","type":"date","jsonPath":".metadata.creationTimestamp"}"#
)]
#[serde(rename_all = "camelCase")]
pub struct ClaimSpec {
    /// The name of the Pool the runner comes from. It never changes: another pool is another
    /// claim.
    #[schemars(length(min = 1))]
    #[x_kube(validation = Rule::new("self == oldSelf").message("A claim's pool cannot change. Make a new claim for another pool."))]
    pub pool: String,
}

/// What the operator says about a Claim.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClaimStatus {
    /// `Ready`: whether the claim's runner is running, and if not, why.
    #[serde(default)]
    pub conditions: Vec<Condition>,
}

/// The condition that says whether a Claim's runner is running, and if not, why.
pub(crate) const READY: &str = "Ready";

/// A Claim's `Ready` condition, moving now.
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
