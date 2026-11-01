// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **People in the cluster** (#2055, ruled 2026-10-29): each requester's runners run in a
//! namespace of their own, [`runner_namespace`], whichever namespace their Claims are in. In it
//! a role binding gives that person alone [`OWNER_ROLE`], and a quota, [`QUOTA`], caps what
//! their runners hold at once, from the chart's `perPerson.quota` ([`Quota`]). A Claim whose
//! runner would go over it says [`OVER_QUOTA`]. A Claim the operator acts on carries
//! [`FINALIZER`], so what it made for the Claim there goes before the Claim does: an owner in
//! another namespace cannot own it.

use crate::resources::{ClaimStatus, condition};
use k8s_openapi::api::core::v1::{
    Namespace, PodSpec, ResourceQuota, ResourceQuotaSpec, ResourceRequirements,
};
use k8s_openapi::api::rbac::v1::{RoleBinding, RoleRef, Subject};
use k8s_openapi::apimachinery::pkg::api::resource::Quantity;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The finalizer each Claim the operator acts on carries until its runner is removed.
pub const FINALIZER: &str = "runners.purlis.dev/runner";

/// What every runner namespace's name starts with.
pub const NAMESPACE_PREFIX: &str = "purlis-runners-";

/// How many hex digits of the requester's SHA-256 a runner namespace's name holds.
pub const HASH_DIGITS: usize = 16;

/// The cluster role the requester alone is given in their runner namespace, and the name of the
/// role binding that gives it. The chart installs the role.
pub const OWNER_ROLE: &str = "purlis-runner-owner";

/// The name of the quota in each runner namespace.
pub const QUOTA: &str = "purlis-person";

/// The label, and its value, on each runner namespace the operator makes.
pub const MANAGED_BY: (&str, &str) = ("app.kubernetes.io/managed-by", "purlis-cluster-operator");

/// The label, and its value, that has the cluster admit only pods meeting the restricted Pod
/// Security Standard to a runner namespace (#3824). Only the namespaces made since carry it: the
/// operator holds no right to change a namespace.
pub const POD_SECURITY: (&str, &str) = ("pod-security.kubernetes.io/enforce", "restricted");

/// The annotation a runner namespace names its requester by.
pub const REQUESTER_ANNOTATION: &str = "runners.purlis.dev/requester";

/// The annotation an upstream claim names its Claim by, its uid: an owner reference cannot
/// cross namespaces.
pub const CLAIM_UID_ANNOTATION: &str = "runners.purlis.dev/claim-uid";

/// The reason of a Claim's `Ready` when its runner would take its requester over their quota.
pub const OVER_QUOTA: &str = "OverQuota";

/// The reason of a Claim's `Ready` when the name of its requester's runner namespace is held by
/// a namespace the operator did not make for them.
pub const NAMESPACE_IN_THE_WAY: &str = "NamespaceInTheWay";

/// The variable the chart hands `perPerson.quota` to the operator in, as JSON.
pub const QUOTA_VAR: &str = "PURLIS_PER_PERSON_QUOTA";

/// The quota's keys, as a ResourceQuota names them, for each of [`Quota`]'s.
pub const CLAIMS_KEY: &str = "count/sandboxclaims.extensions.agents.x-k8s.io";
pub const CPU_KEY: &str = "requests.cpu";
pub const MEMORY_KEY: &str = "requests.memory";
pub const MEMORY_LIMIT_KEY: &str = "limits.memory";
pub const EPHEMERAL_STORAGE_KEY: &str = "requests.ephemeral-storage";
pub const STORAGE_KEY: &str = "requests.storage";

/// **The runner namespace of `requester`**: [`NAMESPACE_PREFIX`] and the first
/// [`HASH_DIGITS`] lower-case hex digits of the SHA-256 of the requester, exactly as the Claim
/// names them. The desktop works out the same name (`purlis_core::clusterpool`).
pub fn runner_namespace(requester: &str) -> String {
    let digest = ring::digest::digest(&ring::digest::SHA256, requester.as_bytes());
    let hex: String = digest
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    format!("{NAMESPACE_PREFIX}{}", &hex[..HASH_DIGITS])
}

/// The runner namespace the operator makes for `requester`.
pub fn namespace_for(requester: &str) -> Namespace {
    Namespace {
        metadata: ObjectMeta {
            name: Some(runner_namespace(requester)),
            labels: Some(
                [
                    (MANAGED_BY.0.to_owned(), MANAGED_BY.1.to_owned()),
                    (POD_SECURITY.0.to_owned(), POD_SECURITY.1.to_owned()),
                ]
                .into(),
            ),
            annotations: Some([(REQUESTER_ANNOTATION.to_owned(), requester.to_owned())].into()),
            ..ObjectMeta::default()
        },
        ..Namespace::default()
    }
}

/// Whether `held`, the namespace the cluster holds under `requester`'s runner namespace's name,
/// is one the operator made for them: it carries the operator's label and names them, and has
/// a uid its objects can be controlled by. Any other is in the way.
pub fn is_theirs(held: &Namespace, requester: &str) -> bool {
    let meta = &held.metadata;
    meta.name.as_deref() == Some(runner_namespace(requester).as_str())
        && meta.uid.as_deref().is_some_and(|uid| !uid.is_empty())
        && meta
            .labels
            .as_ref()
            .and_then(|labels| labels.get(MANAGED_BY.0))
            .map(String::as_str)
            == Some(MANAGED_BY.1)
        && meta
            .annotations
            .as_ref()
            .and_then(|annotations| annotations.get(REQUESTER_ANNOTATION))
            .map(String::as_str)
            == Some(requester)
}

/// What the cluster holds under a requester's runner namespace's name, as the operator reads it.
#[derive(Clone, Debug, PartialEq)]
pub enum RunnerNamespace {
    /// Nothing: the operator makes it, while the policy on runner namespaces is in force.
    Missing,
    /// Nothing, and the policy's binding is still settling: a namespace made now would be made
    /// before it was in force, so nothing is made until `until`.
    Settling { until: k8s_openapi::jiff::Timestamp },
    /// The one the operator made for them, under the policy: it acts in it.
    Theirs(Box<Namespace>),
    /// A namespace it did not make for them, or made before the policy was in force: nothing is
    /// made or changed in it.
    InTheWay { made_before_policy: bool },
    /// No binding puts the policy on runner namespaces in force: no runner namespace is made or
    /// used until one does.
    PolicyNotInForce,
}

/// How long past the time the policy on runner namespaces is in force from the operator waits
/// before it makes one: the API server's clock and the operator's may differ a little.
pub const SETTLE_MARGIN: std::time::Duration = std::time::Duration::from_secs(5);

/// **Whether `held` is `requester`'s runner namespace to act in** (#2055), at `now`:
/// [`is_theirs`], and made after `since`, when the policy on runner namespaces
/// ([`crate::admission::RUNNER_NAMESPACES_POLICY`]) is in force from (`None` while it is not).
/// Under that policy only the operator makes a runner namespace or changes who it is for, so
/// one made since then and marked for them is the operator's. With none, one is made only once
/// `since` is past, by [`SETTLE_MARGIN`].
pub fn runner_namespace_of(
    held: Option<Namespace>,
    requester: &str,
    since: Option<k8s_openapi::jiff::Timestamp>,
    now: k8s_openapi::jiff::Timestamp,
) -> RunnerNamespace {
    let Some(since) = since else {
        return RunnerNamespace::PolicyNotInForce;
    };
    let Some(held) = held else {
        let until = since
            .checked_add(SETTLE_MARGIN)
            .unwrap_or(k8s_openapi::jiff::Timestamp::MAX);
        return if now < until {
            RunnerNamespace::Settling { until }
        } else {
            RunnerNamespace::Missing
        };
    };
    if !is_theirs(&held, requester) {
        return RunnerNamespace::InTheWay {
            made_before_policy: false,
        };
    }
    match &held.metadata.creation_timestamp {
        Some(made) if made.0 > since => RunnerNamespace::Theirs(Box::new(held)),
        _ => RunnerNamespace::InTheWay {
            made_before_policy: true,
        },
    }
}

/// The runner namespace a deleted Claim's finalizer looks in: `held`, when it is marked for
/// `requester` ([`is_theirs`]), whatever the policy says now, so a Claim refused since still
/// takes its runner with it. It deletes there only what the operator made for that Claim, by
/// uid. With none, the Claim goes and nothing is deleted.
pub fn runner_to_clean(held: Option<Namespace>, requester: &str) -> Option<Namespace> {
    held.filter(|held| is_theirs(held, requester))
}

/// Who `requester` is to RBAC: the service account a `system:serviceaccount:<ns>:<name>`
/// username names, and otherwise the user of that name.
pub fn subject_of(requester: &str) -> Subject {
    if let Some((namespace, name)) = requester
        .strip_prefix("system:serviceaccount:")
        .and_then(|rest| rest.split_once(':'))
        .filter(|(namespace, name)| {
            !namespace.is_empty() && !name.is_empty() && !name.contains(':')
        })
    {
        return Subject {
            kind: "ServiceAccount".to_owned(),
            name: name.to_owned(),
            namespace: Some(namespace.to_owned()),
            api_group: None,
        };
    }
    Subject {
        kind: "User".to_owned(),
        name: requester.to_owned(),
        namespace: None,
        api_group: Some("rbac.authorization.k8s.io".to_owned()),
    }
}

/// The role binding in `namespace` that gives `requester` alone [`OWNER_ROLE`].
pub fn owner_binding(namespace: &str, requester: &str) -> RoleBinding {
    RoleBinding {
        metadata: ObjectMeta {
            name: Some(OWNER_ROLE.to_owned()),
            namespace: Some(namespace.to_owned()),
            ..ObjectMeta::default()
        },
        role_ref: RoleRef {
            api_group: Some("rbac.authorization.k8s.io".to_owned()),
            kind: "ClusterRole".to_owned(),
            name: OWNER_ROLE.to_owned(),
        },
        subjects: Some(vec![subject_of(requester)]),
    }
}

/// Whether `held` gives exactly what `wanted` gives, to exactly whom.
pub fn binding_says(held: &RoleBinding, wanted: &RoleBinding) -> bool {
    // The cluster writes a service account's empty API group out; it says the same.
    let subjects = |binding: &RoleBinding| -> Vec<Subject> {
        binding
            .subjects
            .iter()
            .flatten()
            .cloned()
            .map(|subject| Subject {
                api_group: subject.api_group.filter(|group| !group.is_empty()),
                ..subject
            })
            .collect()
    };
    held.role_ref == wanted.role_ref && subjects(held) == subjects(wanted)
}

/// **What each person's runners may hold at once**: the chart's `perPerson.quota`. A key left
/// out is not limited.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Quota {
    /// How many claims.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claims: Option<u32>,
    /// How much CPU their runners' pods ask for, as a Kubernetes quantity.
    #[serde(
        default,
        deserialize_with = "quantity",
        skip_serializing_if = "Option::is_none"
    )]
    pub cpu: Option<String>,
    /// How much memory their runners' pods ask for.
    #[serde(
        default,
        deserialize_with = "quantity",
        skip_serializing_if = "Option::is_none"
    )]
    pub memory: Option<String>,
    /// How much memory their runners' pods may use at most, together (#3844).
    #[serde(
        default,
        deserialize_with = "quantity",
        skip_serializing_if = "Option::is_none"
    )]
    pub memory_limit: Option<String>,
    /// How much of their nodes' disk their runners' pods ask for, together (#3844).
    #[serde(
        default,
        deserialize_with = "quantity",
        skip_serializing_if = "Option::is_none"
    )]
    pub ephemeral_storage: Option<String>,
    /// How much storage their kept runners' volumes ask for.
    #[serde(
        default,
        deserialize_with = "quantity",
        skip_serializing_if = "Option::is_none"
    )]
    pub storage: Option<String>,
}

/// A quantity as the chart's values may spell it: a string, or a bare number Helm's `--set`
/// made of one (`cpu=4`).
fn quantity<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    match Option::<serde_json::Value>::deserialize(deserializer)? {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::String(text)) => Ok(Some(text)),
        Some(serde_json::Value::Number(number)) => Ok(Some(number.to_string())),
        Some(other) => Err(serde::de::Error::custom(format!(
            "a quantity is a string or a number, not {other}"
        ))),
    }
}

impl Default for Quota {
    /// 5 claims, 4 CPUs, 8Gi of memory asked for and 24Gi at most, 10Gi of ephemeral storage and
    /// 50Gi of storage: five runners of the template and five kept volumes of the default 10Gi
    /// fit, with room for a warm one.
    fn default() -> Self {
        Quota {
            claims: Some(5),
            cpu: Some("4".to_owned()),
            memory: Some("8Gi".to_owned()),
            memory_limit: Some("24Gi".to_owned()),
            ephemeral_storage: Some("10Gi".to_owned()),
            storage: Some("50Gi".to_owned()),
        }
    }
}

impl Quota {
    /// The quota [`QUOTA_VAR`] holds: the default when it is not set, and an error when it is
    /// not one, or names a quantity Kubernetes would not read, so the operator does not start.
    pub fn from_var(value: Option<&str>) -> Result<Quota, String> {
        let Some(value) = value else {
            return Ok(Quota::default());
        };
        let quota: Quota = serde_json::from_str(value)
            .map_err(|error| format!("{QUOTA_VAR} is not a per-person quota: {error}"))?;
        for (name, quantity) in [
            ("cpu", &quota.cpu),
            ("memory", &quota.memory),
            ("memoryLimit", &quota.memory_limit),
            ("ephemeralStorage", &quota.ephemeral_storage),
            ("storage", &quota.storage),
        ] {
            if let Some(quantity) = quantity
                && milli(quantity).is_none()
            {
                return Err(format!(
                    "{QUOTA_VAR}'s {name} is not a Kubernetes quantity: {quantity}"
                ));
            }
        }
        Ok(quota)
    }

    /// Its keys and quantities, as a ResourceQuota's `hard` names them.
    pub fn hard(&self) -> BTreeMap<String, Quantity> {
        let mut hard = BTreeMap::new();
        if let Some(claims) = self.claims {
            hard.insert(CLAIMS_KEY.to_owned(), Quantity(claims.to_string()));
        }
        for (key, quantity) in [
            (CPU_KEY, &self.cpu),
            (MEMORY_KEY, &self.memory),
            (MEMORY_LIMIT_KEY, &self.memory_limit),
            (EPHEMERAL_STORAGE_KEY, &self.ephemeral_storage),
            (STORAGE_KEY, &self.storage),
        ] {
            if let Some(quantity) = quantity {
                hard.insert(key.to_owned(), Quantity(quantity.clone()));
            }
        }
        hard
    }
}

/// The quota [`QUOTA`] in `namespace`, as `quota` asks.
pub fn quota_for(namespace: &str, quota: &Quota) -> ResourceQuota {
    ResourceQuota {
        metadata: ObjectMeta {
            name: Some(QUOTA.to_owned()),
            namespace: Some(namespace.to_owned()),
            ..ObjectMeta::default()
        },
        spec: Some(ResourceQuotaSpec {
            hard: Some(quota.hard()),
            ..ResourceQuotaSpec::default()
        }),
        status: None,
    }
}

/// Whether `held`'s `hard` is `wanted`'s, quantity by quantity: the cluster may write a
/// quantity in other words (`0.5` as `500m`) than the chart did.
pub fn quota_says(held: &ResourceQuota, wanted: &ResourceQuota) -> bool {
    let hard = |quota: &ResourceQuota| -> Option<BTreeMap<String, Option<u128>>> {
        Some(
            quota
                .spec
                .as_ref()?
                .hard
                .as_ref()?
                .iter()
                .map(|(key, quantity)| (key.clone(), milli(&quantity.0)))
                .collect(),
        )
    };
    hard(held).unwrap_or_default() == hard(wanted).unwrap_or_default()
}

/// A quantity in thousandths of its unit, as Kubernetes reads it (`250m`, `4`, `0.5`, `8Gi`,
/// `1G`); `None` for any other spelling, which is then not checked ahead of the cluster.
pub fn milli(quantity: &str) -> Option<u128> {
    let quantity = quantity.trim();
    let split = quantity
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(quantity.len());
    let (number, suffix) = quantity.split_at(split);
    let scale: u128 = match suffix {
        "m" => 1,
        "" => 1_000,
        "k" => 1_000_000,
        "M" => 1_000_000_000,
        "G" => 1_000_000_000_000,
        "T" => 1_000_000_000_000_000,
        "P" => 1_000_000_000_000_000_000,
        "E" => 1_000_000_000_000_000_000_000,
        "Ki" => 1_000 << 10,
        "Mi" => 1_000 << 20,
        "Gi" => 1_000 << 30,
        "Ti" => 1_000 << 40,
        "Pi" => 1_000 << 50,
        "Ei" => 1_000 << 60,
        _ => return None,
    };
    let (whole, fraction) = number.split_once('.').unwrap_or((number, ""));
    if whole.is_empty() && fraction.is_empty() || fraction.contains('.') || fraction.len() > 18 {
        return None;
    }
    let whole: u128 = if whole.is_empty() {
        0
    } else {
        whole.parse().ok()?
    };
    let mut value = whole.checked_mul(scale)?;
    if !fraction.is_empty() {
        let digits: u128 = fraction.parse().ok()?;
        let denominator = 10u128.checked_pow(u32::try_from(fraction.len()).ok()?)?;
        // Rounded up, as Kubernetes rounds a quantity it cannot hold exactly.
        value = value.checked_add((digits.checked_mul(scale)?).div_ceil(denominator))?;
    }
    Some(value)
}

/// What one more runner asks of its requester's quota, by the quota's key: one claim, its pod's
/// CPU, memory, ephemeral storage and memory limit (`pod`, the operator's template), and for a
/// kept claim its volume's `storage`.
pub fn asks_of(pod: &PodSpec, kept_storage: Option<&str>) -> Vec<(&'static str, String)> {
    let mut asks = vec![(CLAIMS_KEY, "1".to_owned())];
    for (key, resource) in [
        (CPU_KEY, "cpu"),
        (MEMORY_KEY, "memory"),
        (EPHEMERAL_STORAGE_KEY, "ephemeral-storage"),
    ] {
        if let Some(asked) = pod_counted(pod, resource, |all| all.requests.as_ref()) {
            asks.push((key, asked));
        }
    }
    if let Some(asked) = pod_counted(pod, "memory", |all| all.limits.as_ref()) {
        asks.push((MEMORY_LIMIT_KEY, asked));
    }
    if let Some(storage) = kept_storage {
        asks.push((STORAGE_KEY, storage.to_owned()));
    }
    asks
}

/// What `pod` holds of `resource` in the requests or limits `side` picks, as a quota counts it:
/// the larger of its containers' together and its largest first step's.
fn pod_counted(
    pod: &PodSpec,
    resource: &str,
    side: fn(&ResourceRequirements) -> Option<&BTreeMap<String, Quantity>>,
) -> Option<String> {
    let of = |resources: Option<&ResourceRequirements>| {
        resources
            .and_then(side)
            .and_then(|quantities| quantities.get(resource))
            .and_then(|quantity| milli(&quantity.0))
            .unwrap_or(0)
    };
    let containers: u128 = pod
        .containers
        .iter()
        .map(|container| of(container.resources.as_ref()))
        .sum();
    let steps = pod
        .init_containers
        .iter()
        .flatten()
        .map(|container| of(container.resources.as_ref()))
        .max()
        .unwrap_or(0);
    let asked = containers.max(steps);
    (asked > 0).then(|| format!("{asked}m"))
}

/// **Why one more runner would take its requester over `held`, their quota as the cluster holds
/// it**, if it would: each of `asks` ([`asks_of`]) that the quota limits, added to what it says
/// is used, against what it allows. `None` when the quota has not counted yet, or every ask fits.
pub fn over_quota(held: &ResourceQuota, asks: &[(&str, String)]) -> Option<String> {
    let status = held.status.as_ref()?;
    let hard = status.hard.as_ref()?;
    let used = status.used.clone().unwrap_or_default();
    let over: Vec<String> = asks
        .iter()
        .filter_map(|(key, asked)| {
            let allowed = hard.get(*key)?;
            let in_use = used.get(*key).map(|q| q.0.as_str()).unwrap_or("0");
            let total = milli(in_use)?.checked_add(milli(asked)?)?;
            (total > milli(&allowed.0)?).then(|| {
                format!(
                    "{key}: {in_use} in use of {}, and this runner asks for {asked}",
                    allowed.0
                )
            })
        })
        .collect();
    (!over.is_empty()).then(|| over.join("; "))
}

/// Whether `error` is the cluster refusing a request because it would go over a quota.
pub fn is_over_quota(error: &kube::Error) -> bool {
    matches!(error, kube::Error::Api(status)
        if status.code == 403 && status.message.contains("exceeded quota"))
}

/// A Claim's status when its runner would take its requester over their quota in `namespace`,
/// and `why`.
pub fn status_over_quota(namespace: &str, why: &str) -> ClaimStatus {
    ClaimStatus {
        link_key: None,
        mint: None,
        conditions: vec![condition(
            "False",
            OVER_QUOTA,
            format!(
                "This runner would take its requester over their quota, {QUOTA} in {namespace} \
                 ({why}). Delete another of their claims, or ask a cluster admin to raise \
                 perPerson.quota, and this claim goes on."
            ),
        )],
    }
}

/// A Claim's status when the name of its requester's runner namespace, `namespace`, is held by
/// a namespace the operator did not make for them, or made before the policy on runner
/// namespaces was in force: nothing is made in it.
pub fn status_of_a_namespace_in_the_way(namespace: &str, made_before_policy: bool) -> ClaimStatus {
    let why = if made_before_policy {
        "was made before the policy on runner namespaces was in force"
    } else {
        "was not made by purlis's cluster operator for this claim's requester"
    };
    ClaimStatus {
        link_key: None,
        mint: None,
        conditions: vec![condition(
            "False",
            NAMESPACE_IN_THE_WAY,
            format!(
                "The namespace {namespace}, where this claim's runner would run, {why}, so \
                 nothing is made in it. A cluster admin can remove it, and this claim goes on."
            ),
        )],
    }
}

/// A Claim's status while no binding puts the policy on runner namespaces in force: no runner
/// namespace is made or used.
pub fn status_of_runner_namespaces_not_in_force() -> ClaimStatus {
    ClaimStatus {
        link_key: None,
        mint: None,
        conditions: vec![condition(
            "False",
            "PolicyNotInForce",
            format!(
                "The policy {} is not in force, so no runner namespace is made or used. A \
                 cluster admin can install it again, and this claim goes on.",
                crate::admission::RUNNER_NAMESPACES_POLICY
            ),
        )],
    }
}

/// A Claim's status once its requester's quota in `namespace` has not counted for a while
/// ([`crate::reconcile`]'s `UNCOUNTED_FOR`): its runner is not asked for until it has.
pub fn status_of_an_uncounted_quota(namespace: &str) -> ClaimStatus {
    ClaimStatus {
        link_key: None,
        mint: None,
        conditions: vec![condition(
            "False",
            "Starting",
            format!(
                "The runner is asked for once the quota {QUOTA} in {namespace} has counted what \
                 is in use there. If this lasts, a cluster admin can check that the cluster's \
                 quota controller runs."
            ),
        )],
    }
}

/// Whether `error` is the cluster refusing a request because a quota in the namespace has not
/// counted yet, as with a quota just made: tried again soon, not an error.
pub fn is_quota_not_counted_yet(error: &kube::Error) -> bool {
    matches!(error, kube::Error::Api(status)
        if status.code == 403 && status.message.contains("status unknown for quota"))
}
