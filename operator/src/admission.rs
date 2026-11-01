// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The operator acts only on Claims admitted under its policy, however it was installed.
//!
//! The policy (`deploy/policy.yaml`) admits a Claim only for someone the cluster lets use its
//! Pool. It is in force once it and its binding have been stored and every API server enforces
//! them, which the operator takes to be [`SETTLE`] after the later of their creation times. A Claim made later than
//! that was admitted under it and is acted on; a Claim made earlier was not, and is refused for
//! good. With no binding in force, no Claim is acted on until there is one.

use crate::resources::{Claim, ClaimStatus, condition};
use k8s_openapi::api::admissionregistration::v1::{
    ValidatingAdmissionPolicy, ValidatingAdmissionPolicyBinding,
};
use k8s_openapi::jiff::SignedDuration;

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

/// Whether `binding` puts the policy `policy` in force: it binds that policy, and denies.
fn in_force(binding: &ValidatingAdmissionPolicyBinding, policy: &str) -> bool {
    let spec = &binding.spec;
    spec.policy_name == policy && spec.validation_actions.iter().any(|a| a == "Deny")
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

/// **When the policy `name` is in force from**, given it and its binding as the cluster holds them
/// (#3854): [`SETTLE`] after the later of their creation times, so a policy made again under a
/// binding that stayed counts as made again. `None` while either is missing, or the binding does
/// not deny, or it has changed since it was made (its `generation` is not 1): a cluster keeps no
/// time for that change, so nothing is under it until the binding is made again. An object made
/// no later than this was not admitted under the policy.
pub fn in_force_since(
    policy: Option<&ValidatingAdmissionPolicy>,
    binding: Option<&ValidatingAdmissionPolicyBinding>,
    name: &str,
) -> Option<k8s_openapi::jiff::Timestamp> {
    let binding = binding
        .filter(|binding| in_force(binding, name))
        .filter(|binding| binding.metadata.generation == Some(1))?;
    let bound = settled(binding.metadata.creation_timestamp.as_ref()?);
    let held = settled(policy?.metadata.creation_timestamp.as_ref()?);
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
                    "The policy {POLICY} is not in force, or its binding has changed since it was \
                     made, so no claim is acted on. A cluster admin can install it again; then \
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
