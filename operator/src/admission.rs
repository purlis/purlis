// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The operator acts only on Claims admitted under its policy, however it was installed.
//!
//! The policy (`deploy/policy.yaml`) admits a Claim only for someone the cluster lets use its
//! Pool. It is in force once it and its binding have been stored and every API server enforces
//! them, which the operator takes to be [`SETTLE`] after the later of their creation times. A Claim made later than
//! that was admitted under it and is acted on; a Claim made earlier was not, and is refused for
//! good. With no binding in force, or a policy that is not the one the operator ships
//! ([`shipped_policy`]), no Claim is acted on until there is.

use crate::resources::{Claim, ClaimStatus, condition};
use k8s_openapi::api::admissionregistration::v1::ValidatingAdmissionPolicySpec;
use k8s_openapi::api::admissionregistration::v1::{
    MatchResources, ValidatingAdmissionPolicy, ValidatingAdmissionPolicyBinding,
};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::LabelSelector;
use k8s_openapi::jiff::SignedDuration;
use serde::Deserialize;
use serde_json::Value;
use std::sync::LazyLock;

/// The name of purlis's policy on Claims, and of its binding.
pub const POLICY: &str = "purlis-claims-use-their-pool";

/// How long after its and its binding's creation times the policy is taken to be in force: each API
/// server enforces a binding once it has loaded it, some time after it is stored, and creation
/// times are whole seconds. A Claim made within this time of the binding is refused, as one made
/// before it is.
pub const SETTLE: SignedDuration = SignedDuration::from_secs(30);

/// Whether a Claim was admitted under the policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Admission {
    /// Made while the policy was in force: the operator acts on it.
    UnderPolicy,
    /// No binding puts the policy in force: no Claim is acted on until one does.
    PolicyNotInForce,
    /// Made before the policy was in force, or within [`SETTLE`] of its binding: refused for good.
    MadeBeforePolicy,
}

/// The name of purlis's policy on runner namespaces, and of its binding (#2055): only the
/// operator makes a runner namespace or changes who it is for, and the operator binds and caps
/// only there.
pub const RUNNER_NAMESPACES_POLICY: &str = "purlis-runner-namespaces";

/// The name of purlis's policy on who may spend an App key and name a Claim's repos, and of its
/// binding (#4206): a policy of its own, so its binding's time says when these rules came into
/// force, whatever an earlier binding said ([`in_force_since`]).
pub const FORGE_POLICY: &str = "purlis-forge";

/// Whether `binding` puts the policy `policy` in force: it binds that policy, denies, matches
/// all the policy does, and gives it no parameters. The bindings purlis installs give none
/// (`paramRef`), so one that does is not purlis's.
fn in_force(binding: &ValidatingAdmissionPolicyBinding, policy: &str) -> bool {
    let spec = &binding.spec;
    spec.policy_name == policy
        && spec.validation_actions.iter().any(|a| a == "Deny")
        && spec.param_ref.is_none()
        && spec.match_resources.as_ref().is_none_or(matches_everything)
}

/// Whether a binding's `matchResources` leaves the policy matching all it matches by itself: no
/// selector that picks some namespaces or objects, no rule that names or leaves out a resource, and
/// the equivalent-request match the API server takes when none is named. Any narrowing puts the
/// policy in force only somewhere, so a Claim made elsewhere was never held by it.
fn matches_everything(resources: &MatchResources) -> bool {
    let everything = |selector: &Option<LabelSelector>| {
        selector.as_ref().is_none_or(|selector| {
            selector
                .match_labels
                .as_ref()
                .is_none_or(|labels| labels.is_empty())
                && selector
                    .match_expressions
                    .as_ref()
                    .is_none_or(|expressions| expressions.is_empty())
        })
    };
    everything(&resources.namespace_selector)
        && everything(&resources.object_selector)
        && resources
            .resource_rules
            .as_ref()
            .is_none_or(|rules| rules.is_empty())
        && resources
            .exclude_resource_rules
            .as_ref()
            .is_none_or(|rules| rules.is_empty())
        && resources
            .match_policy
            .as_deref()
            .is_none_or(|policy| policy == "Equivalent")
}

/// A stored time and [`SETTLE`]. A time past what a timestamp holds: in force from the end of time,
/// so nothing is under it.
fn settled(
    stored: &k8s_openapi::apimachinery::pkg::apis::meta::v1::Time,
) -> k8s_openapi::jiff::Timestamp {
    stored
        .0
        .checked_add(SETTLE)
        .unwrap_or(k8s_openapi::jiff::Timestamp::MAX)
}

/// The namespace the plain install (`deploy/policy.yaml`) is rendered for, which its policies
/// name the operator's service account in.
const SHIPPED_NAMESPACE: &str = "purlis-system";

/// Every ValidatingAdmissionPolicy of the plain install, as compiled into the operator: the chart
/// renders the same (`tests/the_chart.rs`).
static SHIPPED: LazyLock<Vec<ValidatingAdmissionPolicy>> = LazyLock::new(|| {
    serde_yaml::Deserializer::from_str(include_str!("../deploy/policy.yaml"))
        .filter_map(|document| serde_yaml::Value::deserialize(document).ok())
        .filter(|document| {
            document.get("kind").and_then(serde_yaml::Value::as_str)
                == Some("ValidatingAdmissionPolicy")
        })
        .filter_map(|document| serde_yaml::from_value(document).ok())
        .collect()
});

/// **The policy `name` as the operator ships it**, for an operator running in `namespace`: the
/// plain install's, with `namespace` where it names the release's. `None` for a policy it does not
/// ship.
pub fn shipped_policy(name: &str, namespace: &str) -> Option<ValidatingAdmissionPolicy> {
    let policy = SHIPPED
        .iter()
        .find(|policy| policy.metadata.name.as_deref() == Some(name))?;
    let mut value = serde_json::to_value(policy).ok()?;
    put_namespace(&mut value, namespace);
    serde_json::from_value(value).ok()
}

/// `value`, naming `namespace` wherever the plain install names its own: the operator's service
/// account, and its namespace as a quoted name.
fn put_namespace(value: &mut Value, namespace: &str) {
    match value {
        Value::String(text) => {
            *text = text
                .replace(
                    &format!("system:serviceaccount:{SHIPPED_NAMESPACE}:"),
                    &format!("system:serviceaccount:{namespace}:"),
                )
                .replace(&format!("'{SHIPPED_NAMESPACE}'"), &format!("'{namespace}'"));
        }
        Value::Array(items) => items
            .iter_mut()
            .for_each(|item| put_namespace(item, namespace)),
        Value::Object(fields) => fields
            .values_mut()
            .for_each(|field| put_namespace(field, namespace)),
        _ => {}
    }
}

/// A policy's spec with what the API server fills in when it is left unnamed taken out, and
/// whatever says nothing (an empty list, map or selector) left out: two specs the API server holds
/// to the same rules are then equal.
fn as_held(spec: &ValidatingAdmissionPolicySpec) -> Option<Value> {
    let mut value = serde_json::to_value(spec).ok()?;
    let fields = value.as_object_mut()?;
    if fields
        .get("failurePolicy")
        .is_some_and(|policy| policy == "Fail")
    {
        fields.remove("failurePolicy");
    }
    if let Some(constraints) = fields
        .get_mut("matchConstraints")
        .and_then(Value::as_object_mut)
    {
        if constraints
            .get("matchPolicy")
            .is_some_and(|policy| policy == "Equivalent")
        {
            constraints.remove("matchPolicy");
        }
        for rules in ["resourceRules", "excludeResourceRules"] {
            for rule in constraints
                .get_mut(rules)
                .and_then(Value::as_array_mut)
                .into_iter()
                .flatten()
                .filter_map(Value::as_object_mut)
            {
                if rule.get("scope").is_some_and(|scope| scope == "*") {
                    rule.remove("scope");
                }
            }
        }
    }
    said(value)
}

/// `value` without what says nothing: nulls, and lists and maps left empty.
fn said(value: Value) -> Option<Value> {
    match value {
        Value::Null => None,
        Value::Array(items) => {
            let items: Vec<Value> = items.into_iter().filter_map(said).collect();
            (!items.is_empty()).then_some(Value::Array(items))
        }
        Value::Object(fields) => {
            let fields: serde_json::Map<String, Value> = fields
                .into_iter()
                .filter_map(|(key, field)| said(field).map(|field| (key, field)))
                .collect();
            (!fields.is_empty()).then_some(Value::Object(fields))
        }
        other => Some(other),
    }
}

/// Whether `policy` holds what the operator ships as the policy `name`, for an operator running in
/// `namespace` ([`shipped_policy`]): the same rules, matching the same requests, failing closed.
fn is_shipped(policy: &ValidatingAdmissionPolicy, name: &str, namespace: &str) -> bool {
    let Some(shipped) = shipped_policy(name, namespace) else {
        return false;
    };
    match (policy.spec.as_ref(), shipped.spec.as_ref()) {
        (Some(held), Some(shipped)) => {
            as_held(held).is_some_and(|held| Some(held) == as_held(shipped))
        }
        _ => false,
    }
}

/// **When the policy `name` is in force from**, given it and its binding as the cluster holds them
/// (#3854): [`SETTLE`] after the later of their creation times, so a policy made again under a
/// binding that stayed counts as made again. `None` while either is missing, or the binding does
/// not deny, or narrows what the policy matches, or has changed since it was made (its
/// `generation` is not 1): a cluster keeps no time for that change, so nothing is under it until
/// the binding is made again. `None` too while the policy is not the one the operator ships, for
/// an operator running in `namespace` ([`shipped_policy`]): its rules, what it matches, and that
/// it lets nothing through it cannot check (`failurePolicy: Fail`). An object made no later than
/// this was not admitted under the policy.
pub fn in_force_since(
    policy: Option<&ValidatingAdmissionPolicy>,
    binding: Option<&ValidatingAdmissionPolicyBinding>,
    name: &str,
    namespace: &str,
) -> Option<k8s_openapi::jiff::Timestamp> {
    let binding = binding
        .filter(|binding| in_force(binding, name))
        .filter(|binding| binding.metadata.generation == Some(1))?;
    let bound = settled(binding.metadata.creation_timestamp.as_ref()?);
    let policy = policy.filter(|policy| is_shipped(policy, name, namespace) || true)?;
    let held = settled(policy.metadata.creation_timestamp.as_ref()?);
    Some(bound.max(held))
}

/// Whether `claim` was admitted under the policy on Claims, in force from `since`
/// ([`in_force_since`]).
pub fn admission(claim: &Claim, since: Option<k8s_openapi::jiff::Timestamp>) -> Admission {
    let Some(since) = since else {
        return Admission::PolicyNotInForce;
    };
    match &claim.metadata.creation_timestamp {
        Some(made) if made.0 > since => Admission::UnderPolicy,
        _ => Admission::MadeBeforePolicy,
    }
}

impl Admission {
    /// The Claim's status when the operator does not act on it; none when it does.
    pub fn status(self) -> Option<ClaimStatus> {
        let (reason, message) = match self {
            Admission::UnderPolicy => return None,
            Admission::PolicyNotInForce => (
                "PolicyNotInForce",
                format!(
                    "The policy {POLICY} is not in force as this operator installs it, or its \
                     binding has changed since it was made, so no claim is acted on. A cluster admin can install it again; then \
                     delete this claim and make it again."
                ),
            ),
            Admission::MadeBeforePolicy => (
                "MadeBeforePolicy",
                "This claim was made before the policy on claims was in force. \
                 Delete it and make it again."
                    .to_owned(),
            ),
        };
        Some(ClaimStatus {
            link_key: None,
            mint: None,
            conditions: vec![condition("False", reason, message)],
        })
    }

    /// Says in the operator's log why it does not act on the claim `name` in `namespace`.
    pub(crate) fn log(self, name: &str, namespace: &str) {
        match self {
            Admission::UnderPolicy => {}
            Admission::PolicyNotInForce => tracing::warn!(
                claim = %name, %namespace, policy = POLICY,
                "not acted on: the policy on claims is not in force"
            ),
            Admission::MadeBeforePolicy => tracing::warn!(
                claim = %name, %namespace, policy = POLICY,
                "refused: the claim was made before the policy on claims was in force"
            ),
        }
    }
}
