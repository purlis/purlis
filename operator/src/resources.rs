// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! purlis's own objects, `v1alpha1`: no stability promise before upstream agent-sandbox leaves
//! beta (spec #2041). `deploy/crds.yaml` is generated from these types, and a test fails until
//! it is regenerated.

use k8s_openapi::apimachinery::pkg::apis::meta::v1::{Condition, Time};
use kube::{CustomResource, KubeSchema};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Where a team's runners come from. Cluster-wide, defined once for the team. Who may claim from
/// it is the cluster's own access control: a Claim is admitted only for someone allowed to `use`
/// the Pool (deploy/policy.yaml).
#[derive(CustomResource, KubeSchema, Serialize, Deserialize, Clone, Debug, PartialEq)]
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
#[x_kube(validation = Rule::new("duration(self.noticeBeforeDelete) < duration(self.deleteSuspendedAfter)").message("noticeBeforeDelete must be shorter than deleteSuspendedAfter: the Update comes before the deletion."))]
#[x_kube(validation = Rule::new("duration(self.idleAfter) >= duration('1m')").message("idleAfter is a minute at least."))]
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
    /// How long a runner of the pool is idle before it is suspended (#3737): no running chat, no
    /// open stream, no forwarded request and no build (ADR 0089 §9); a runner a live chat owns,
    /// or whose browser holds an Ask, is never idle. A whole number of `s`, `m` or `h`, a minute
    /// at least. 30m unless the pool says otherwise.
    #[serde(default = "thirty_minutes")]
    #[schemars(regex(pattern = r"^[1-9][0-9]*[smh]$"))]
    pub idle_after: String,
    /// How long a kept Claim stays suspended before it is deleted, with its volume (#3737). A
    /// whole number of `s`, `m` or `h`. 720h (30 days) unless the pool says otherwise.
    #[serde(default = "thirty_days")]
    #[schemars(regex(pattern = r"^[1-9][0-9]*[smh]$"))]
    pub delete_suspended_after: String,
    /// How long before that deletion its requester is sent an Update (#3737). A whole number of
    /// `s`, `m` or `h`, shorter than `deleteSuspendedAfter`. 168h (7 days) unless the pool says
    /// otherwise.
    #[serde(default = "seven_days")]
    #[schemars(regex(pattern = r"^[1-9][0-9]*[smh]$"))]
    pub notice_before_delete: String,
    /// A Claim's maximum age (#3737): past it, a kept Claim's runner starts again on the pool's
    /// current image, and a fresh Claim's is made again. A whole number of `s`, `m` or `h`. 336h
    /// (14 days) unless the pool says otherwise.
    #[serde(default = "fourteen_days")]
    #[schemars(regex(pattern = r"^[1-9][0-9]*[smh]$"))]
    pub max_age: String,
    /// How many runners are kept warm for fresh Claims (#3737), in each runner namespace the
    /// pool has a Claim in; none while the pool is stopped. The operator keeps the warm pool at
    /// this count. 0 unless the pool says otherwise.
    #[serde(default)]
    #[schemars(range(min = 0))]
    pub warm_replicas: i32,
}

fn thirty_minutes() -> String {
    "30m".to_owned()
}

fn thirty_days() -> String {
    "720h".to_owned()
}

fn seven_days() -> String {
    "168h".to_owned()
}

fn fourteen_days() -> String {
    "336h".to_owned()
}

/// `spelled`, a whole number of `s`, `m` or `h` as the Pool's times are, as a duration; a value
/// the API server would not have taken reads as `otherwise`.
fn duration_of(spelled: &str, otherwise: &str) -> Duration {
    let read = |spelled: &str| {
        let unit = match spelled.chars().last()? {
            's' => 1,
            'm' => 60,
            'h' => 60 * 60,
            _ => return None,
        };
        let count: u64 = spelled[..spelled.len() - 1].parse().ok()?;
        Some(Duration::from_secs(count.checked_mul(unit)?))
    };
    read(spelled)
        .or_else(|| read(otherwise))
        .unwrap_or_default()
}

impl PoolSpec {
    /// A pool of runners of `image`, saying nothing else: what the API server makes of a Pool
    /// that names only its image.
    pub fn new(image: impl Into<String>) -> PoolSpec {
        PoolSpec {
            image: image.into(),
            kept_volume: KeptVolume::default(),
            stopped: false,
            idle_after: thirty_minutes(),
            delete_suspended_after: thirty_days(),
            notice_before_delete: seven_days(),
            max_age: fourteen_days(),
            warm_replicas: 0,
        }
    }

    /// [`PoolSpec::idle_after`], as a duration.
    pub fn idle_after(&self) -> Duration {
        duration_of(&self.idle_after, &thirty_minutes())
    }

    /// [`PoolSpec::delete_suspended_after`], as a duration.
    pub fn delete_suspended_after(&self) -> Duration {
        duration_of(&self.delete_suspended_after, &thirty_days())
    }

    /// [`PoolSpec::notice_before_delete`], as a duration.
    pub fn notice_before_delete(&self) -> Duration {
        duration_of(&self.notice_before_delete, &seven_days())
    }

    /// [`PoolSpec::max_age`], as a duration.
    pub fn max_age(&self) -> Duration {
        duration_of(&self.max_age, &fourteen_days())
    }
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
    /// The cluster the Pool is in: the uid of its `kube-system` namespace. A desktop pins it with
    /// the key, and refuses a statement made in any other cluster (#3305).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cluster: Option<String>,
    /// While a team admin rolls the operator's key, the key it rolls to, spelled as
    /// `signingKey` (#3305). A desktop pinned to `signingKey` moves its pin here once
    /// `nextSigningKeyVouch` checks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_signing_key: Option<String>,
    /// The current key's word for the next one, signed by `signingKey` and holding for seven
    /// days at most (#3305).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_signing_key_vouch: Option<NextSigningKeyVouch>,
}

/// The operator's current key's word for its next one (#3305).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NextSigningKeyVouch {
    /// The statement, exactly as signed (crate::vouch::roll_statement): the cluster, the key it
    /// rolls from, the key it rolls to, and until when it holds.
    pub statement: String,
    /// Its Ed25519 signature, 128 lower-case hex, by the key `signingKey` names.
    pub signature: String,
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
    /// While the operator's key is rolled, the next key's signature over the same statement,
    /// 128 lower-case hex, for a desktop that has moved its pin to it (#3305).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_signature: Option<String>,
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
