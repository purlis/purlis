// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The operator acts only on Claims admitted under its policy, however it was installed.
//!
//! The policy (`deploy/policy.yaml`) admits a Claim only for someone the cluster lets use its
//! Pool. It is in force once its binding has been stored and every API server enforces it, which
//! the operator takes to be [`SETTLE`] after the binding's creation time. A Claim made later than
//! that was admitted under it and is acted on; a Claim made earlier was not, and is refused for
//! good. With no binding in force, no Claim is acted on until there is one.

use crate::resources::{Claim, ClaimStatus, condition};
use k8s_openapi::api::admissionregistration::v1::ValidatingAdmissionPolicyBinding;
use k8s_openapi::jiff::SignedDuration;

/// The name of purlis's policy on Claims, and of its binding.
pub const POLICY: &str = "purlis-claims-use-their-pool";

/// How long after its binding's creation time the policy is taken to be in force: each API
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

/// Whether `binding` puts purlis's policy in force: it binds that policy, and denies.
fn in_force(binding: &ValidatingAdmissionPolicyBinding) -> bool {
    let spec = &binding.spec;
    spec.policy_name == POLICY && spec.validation_actions.iter().any(|a| a == "Deny")
}

/// Whether `claim` was admitted under the policy, given its binding as the cluster holds it.
pub fn admission(claim: &Claim, binding: Option<&ValidatingAdmissionPolicyBinding>) -> Admission {
    let Some(binding) = binding.filter(|binding| in_force(binding)) else {
        return Admission::PolicyNotInForce;
    };
    let Some(stored) = &binding.metadata.creation_timestamp else {
        return Admission::PolicyNotInForce;
    };
    let Ok(since) = stored.0.checked_add(SETTLE) else {
        return Admission::MadeBeforePolicy;
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
                    "The policy {POLICY} is not in force, so no claim is acted on. A cluster \
                     admin can install it again; then delete this claim and make it again."
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
