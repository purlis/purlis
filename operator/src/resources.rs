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
    status = "PoolStatus",
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
    /// The volume each kept Claim's runner gets of its own (#2057): its home, holding its
    /// checkout, conversation, profile sign-in, kept browser logins and its pairing, kept when
    /// its pod is lost. A fresh Claim's runner keeps nothing.
    #[serde(default)]
    pub kept_volume: KeptVolume,
    /// Whether the pool is stopped (#2058): every runner of its Claims is suspended, never
    /// destroyed (its pod ends; a kept Claim's volume, its home, stays), no runner is kept warm
    /// for it, and a Claim made meanwhile gets no runner, until it is false again. False unless
    /// set.
    #[serde(default)]
    pub stopped: bool,
}

/// The volume of a kept Claim's runner: one PersistentVolumeClaim per kept Claim, named
/// `purlis-kept-<claim>`, made by upstream for the claim and removed with it.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KeptVolume {
    /// How much it holds, in Mi, Gi or Ti. 10Gi unless the pool says otherwise.
    #[serde(default = "ten_gibibytes")]
    #[schemars(regex(pattern = r"^[1-9][0-9]*(Mi|Gi|Ti)$"))]
    pub size: String,
    /// The storage class it is made in: the cluster's default class unless the pool names one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_class_name: Option<String>,
}

fn ten_gibibytes() -> String {
    "10Gi".to_owned()
}

impl Default for KeptVolume {
    fn default() -> Self {
        KeptVolume {
            size: ten_gibibytes(),
            storage_class_name: None,
        }
    }
}

/// What the operator says about a Pool.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PoolStatus {
    /// The public half of the key the operator signs each runner's link key with, `ed25519:`
    /// and 64 lower-case hex. A desktop pins it when it adds the pool, and refuses a runner
    /// whose link key it did not sign (#2056).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signing_key: Option<String>,
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
    /// Who asked for the claim, as the cluster signed them in: admitted only when it is the
    /// person making the claim, and never changed after (deploy/policy.yaml). The operator
    /// vouches for it with the runner's link key.
    #[schemars(length(min = 1))]
    #[x_kube(validation = Rule::new("self == oldSelf").message("A claim's requester cannot change."))]
    pub requester: String,
    /// Whether the runner is kept (#2057): started cold with a volume of its own, its home,
    /// which holds its work and its pairing through the loss of its pod. A fresh runner, the
    /// default, comes warm from the pool where one is waiting and keeps nothing. It never
    /// changes: the other kind is another claim.
    #[serde(default)]
    #[x_kube(validation = Rule::new("self == oldSelf").message("Whether a claim is kept cannot change. Make a new claim for the other kind."))]
    pub kept: bool,
    /// Whether the claim is stopped (#2058): its runner is suspended, never destroyed (its pod
    /// ends; a kept claim's volume, its home, stays), or not made yet, until it is false again.
    /// Its pool's `stopped` stops it too. False unless set.
    #[serde(default)]
    pub stopped: bool,
}

/// What the operator says about a Claim.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClaimStatus {
    /// `Ready`: whether the claim's runner is running, and if not, why.
    #[serde(default)]
    pub conditions: Vec<Condition>,
    /// The runner's link key, vouched for: the operator's statement of the claim, the pod, the
    /// public link key the pod made as it started and who asked for the claim, and its
    /// signature (#2056).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link_key: Option<LinkKey>,
}

/// A runner's link key, as the operator vouches for it.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LinkKey {
    /// The statement, exactly as signed (crate::vouch).
    pub statement: String,
    /// Its Ed25519 signature, 128 lower-case hex, by the key the Pool's status names.
    pub signature: String,
}

/// The condition that says whether a Claim's runner is running, and if not, why.
pub(crate) const READY: &str = "Ready";

/// The condition that says whether the operator vouches for a Claim's runner's link key yet.
pub(crate) const KEY_PUBLISHED: &str = "KeyPublished";

/// A Claim's `Ready` condition, moving now.
pub(crate) fn condition(status: &str, reason: &str, message: String) -> Condition {
    condition_of(READY, status, reason, message)
}

/// A Claim's condition `type_`, moving now.
pub(crate) fn condition_of(type_: &str, status: &str, reason: &str, message: String) -> Condition {
    Condition {
        type_: type_.to_owned(),
        status: status.to_owned(),
        reason: reason.to_owned(),
        message,
        last_transition_time: Time(k8s_openapi::jiff::Timestamp::now()),
        observed_generation: None,
    }
}
