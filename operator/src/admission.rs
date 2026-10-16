// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The operator acts only on Claims admitted under its policy, however it was installed.
//!
//! The policy (`deploy/policy.yaml`) admits a Claim only for someone the cluster lets use its
//! Pool. Its binding's creation time is when it came into force: a Claim made after it was
//! admitted under it, and a Claim made before it, or in the same second (creation times are
//! whole seconds), was not, and is refused for good. With no binding in force, no Claim is acted
//! on until there is one.

use crate::reconcile::condition;
use crate::resources::{Claim, ClaimStatus};
use k8s_openapi::api::admissionregistration::v1::ValidatingAdmissionPolicyBinding;

/// The name of purlis's policy on Claims, and of its binding.
pub const POLICY: &str = "purlis-claims-use-their-pool";

/// Whether a Claim was admitted under the policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Admission {
    /// Made while the policy was in force: the operator acts on it.
    UnderPolicy,
    /// No binding puts the policy in force: no Claim is acted on until one does.
    NoPolicy,
    /// Made before the policy came into force, or in the same second: refused for good.
    MadeBeforePolicy,
}

/// Whether `binding` puts purlis's policy in force: it binds that policy, and denies.
fn in_force(binding: &ValidatingAdmissionPolicyBinding) -> bool {
    let Some(spec) = &binding.spec else {
        return false;
    };
    spec.policy_name.as_deref() == Some(POLICY)
        && spec
            .validation_actions
            .as_ref()
            .is_some_and(|actions| actions.iter().any(|a| a == "Deny"))
}

/// Whether `claim` was admitted under the policy, given its binding as the cluster holds it.
pub fn admission(claim: &Claim, binding: Option<&ValidatingAdmissionPolicyBinding>) -> Admission {
    let Some(binding) = binding.filter(|binding| in_force(binding)) else {
        return Admission::NoPolicy;
    };
    let Some(since) = &binding.metadata.creation_timestamp else {
        return Admission::NoPolicy;
    };
    match &claim.metadata.creation_timestamp {
        Some(made) if made.0 > since.0 => Admission::UnderPolicy,
        _ => Admission::MadeBeforePolicy,
    }
}

impl Admission {
    /// The Claim's status when the operator does not act on it; none when it does.
    pub fn status(self) -> Option<ClaimStatus> {
        let (reason, message) = match self {
            Admission::UnderPolicy => return None,
            Admission::NoPolicy => (
                "PolicyNotInForce",
                format!(
                    "The policy {POLICY} is not in force, so no claim is acted on. \
                     A cluster admin can install it again."
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
            conditions: vec![condition("False", reason, message)],
        })
    }
}
