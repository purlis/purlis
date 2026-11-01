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
#[x_kube(validation = Rule::new("duration(self.maxAge) > duration('0s')").message("maxAge is too long to read as a duration."))]
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
    /// at least. 30m unless the pool says otherwise. A runner takes a change to it when it next starts.
    #[serde(default = "thirty_minutes")]
    #[schemars(regex(pattern = r"^[1-9][0-9]*[smh]$"), length(max = 12))]
    pub idle_after: String,
    /// How long a kept Claim stays suspended for being idle before it is deleted, with its volume
    /// (#3737). A whole number of `s`, `m` or `h`. 720h (30 days) unless the pool says otherwise.
    /// A stopped Claim is never deleted.
    #[serde(default = "thirty_days")]
    #[schemars(regex(pattern = r"^[1-9][0-9]*[smh]$"), length(max = 12))]
    pub delete_suspended_after: String,
    /// How long before that deletion its requester is sent an Update (#3737). A whole number of
    /// `s`, `m` or `h`, shorter than `deleteSuspendedAfter`. 168h (7 days) unless the pool says
    /// otherwise.
    #[serde(default = "seven_days")]
    #[schemars(regex(pattern = r"^[1-9][0-9]*[smh]$"), length(max = 12))]
    pub notice_before_delete: String,
    /// A Claim's maximum age (#3737): past it, a kept Claim's runner starts again on the pool's
    /// current image when next it runs, and a fresh Claim's is made again, once nothing runs on it: a
    /// chat is never ended for it. A whole number of `s`, `m` or `h`. 336h (14 days) unless the pool says
    /// otherwise.
    #[serde(default = "fourteen_days")]
    #[schemars(regex(pattern = r"^[1-9][0-9]*[smh]$"), length(max = 12))]
    pub max_age: String,
    /// How many runners are kept warm for fresh Claims (#3737), in each runner namespace the
    /// pool has a Claim in; none while the pool is stopped. The operator keeps the warm pool at
    /// this count, 20 at most. They count toward each person's quota. Kept only for a person
    /// with a fresh Claim on the pool that is not stopped. 0 unless the pool says otherwise.
    #[serde(default)]
    #[schemars(range(min = 0, max = 20))]
    pub warm_replicas: i32,
    /// A team's GitHub App, for runners' forge tools (#4204). Left out, the operator reads no
    /// Secret for this pool.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forge: Option<PoolForge>,
}

/// A Pool's GitHub App, and which repos its runners' tokens may ever cover (#3807).
#[derive(KubeSchema, Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PoolForge {
    /// The Secret in the operator's own namespace holding the App's client id (`app`) and its
    /// private key (`key`). The chart's `forge.keySecretNames` must name it too, and whoever
    /// writes this needs `use` on it (`forgekeys`).
    pub app_key_secret_ref: SecretRef,
    /// The most a token may cover: `owner/name` or `owner/*`. A token covers only these and the
    /// repos of the Claim.
    #[schemars(length(min = 1, max = 256), inner(length(max = 140), regex(pattern = crate::forge::POOL_REPO)))]
    pub repos: Vec<String>,
    /// Whether the pool opts in to the operator minting tokens for its runners. Anyone who may use
    /// the Pool can use these repos through it. False unless set.
    #[serde(default)]
    pub mint_in_cluster: bool,
}

/// A Secret, by name, in the operator's own namespace: it says no namespace.
#[derive(KubeSchema, Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SecretRef {
    /// The Secret's name, never the operator's own signing key's.
    #[schemars(length(min = 1, max = 253), regex(pattern = crate::forge::SECRET_NAME))]
    #[x_kube(validation = Rule::new("self != 'purlis-cluster-operator-signing-key'").message("A pool cannot name the operator's own signing key."))]
    pub name: String,
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

/// `spelled`, a whole number of `s`, `m` or `h` as the Pool's times are, as a duration. The API
/// server takes only what reads as one (each time is at most 12 characters and read by a
/// `duration()` rule), so `otherwise` is read only for a Pool stored some other way.
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
            forge: None,
        }
    }

    /// The pool's `idleAfter`, as a duration.
    pub fn idle_after(&self) -> Duration {
        duration_of(&self.idle_after, &thirty_minutes())
    }

    /// The pool's `deleteSuspendedAfter`, as a duration.
    pub fn delete_suspended_after(&self) -> Duration {
        duration_of(&self.delete_suspended_after, &thirty_days())
    }

    /// The pool's `noticeBeforeDelete`, as a duration.
    pub fn notice_before_delete(&self) -> Duration {
        duration_of(&self.notice_before_delete, &seven_days())
    }

    /// The pool's `maxAge`, as a duration.
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
    /// The repos the runner works on (#4204). Changed only by who asked for the claim, while they
    /// may use its pool (deploy/policy.yaml). A token covers only these and the Pool's allow-list.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forge: Option<ClaimForge>,
}

/// The repos of the workspace a Claim's runner was sent (#3807).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClaimForge {
    /// Each repo, `owner/name`: never a wildcard, which only a Pool's allow-list may hold.
    #[schemars(length(max = 256), inner(length(max = 140), regex(pattern = crate::forge::CLAIM_REPO)))]
    pub repos: Vec<String>,
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
    /// Where the Claim's runner asks for a GitHub App token and the key it holds that port to
    /// (#4207): written only for a Claim whose runner the operator vouches for, on a Pool that
    /// opts in to minting. A desktop tells its runner these once, as it pairs it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mint: Option<ClaimMint>,
}

/// Where a Claim's runner asks for a token (#4207).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClaimMint {
    /// The mint port's address, `<ip>:<port>`, by the Service's cluster IP, since a runner's pod
    /// is given public name servers.
    pub address: String,
    /// The operator's key the port's TLS is held to, spelled as a Pool's `signingKey`.
    pub operator_key: String,
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
