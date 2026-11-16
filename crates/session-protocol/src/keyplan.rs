// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The key-proxy plan** (1.91, T1a-51, R-469, R-470): the route a run on a runner takes, sent
//! by the desktop on its own link with the run's start ([`crate::session::Command::KeyProxyPlan`]).
//!
//! It is treated like a vault value: it crosses once, the runner keeps it for that run only and
//! discards it at run end. It names the declaration a run uses by reference alone (R-168): 1a's
//! built-in by id and digest, which the runner checks against its own binary's copy, so no model,
//! background model or price travels in it (H-18). The key travels in the same message, so a
//! plan and its key are one round trip.
//!
//! **Unlike the rest of the protocol, a field this version doesn't name is refused**, not
//! ignored (B-17): a plan is read whole or not at all. The fields later phases fill are named
//! here and **reserved**: a plan that sets one is refused at arrival ([`KeyProxyPlan::refusal`]),
//! as is a plan with no key.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// **A run's key-proxy plan**, as the desktop sends it to the runner (1.91).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyProxyPlan {
    /// The run it is the route of: kept for that run alone.
    pub run: String,
    /// The built-in declaration the run uses, by id and digest (R-168, R-470).
    pub built_in: BuiltInReference,
    /// The profile's sign-in kind, by its word (`api-key` in 1a): the runner learns it here,
    /// never from the profile it keeps (R-652).
    pub sign_in: String,
    /// The declaration's template values, by name. None in 1a's built-in.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub template: BTreeMap<String, String>,
    /// The desktop's locks on where the key proxy may connect.
    #[serde(default, skip_serializing_if = "PlanLocks::is_empty")]
    pub locks: PlanLocks,
    /// The origins the key is bound to (R-179).
    pub origins: Vec<String>,
    /// The public key of a certificate the key is pinned to, where it is pinned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinned_key: Option<String>,
    /// The key, in the same round trip. Absent, the plan is refused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<PlanKey>,
    /// Reserved (1b): a project's declaration, by digest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_declaration: Option<serde_json::Value>,
    /// Reserved (1b): the person's own declaration, whole.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub your_declaration: Option<serde_json::Value>,
    /// Reserved: a declaration a policy carries.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_declaration: Option<serde_json::Value>,
    /// Reserved: the confirmations a run's route was given.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirmations: Option<serde_json::Value>,
    /// Reserved (4): the run's remaining budget.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remaining_budget: Option<serde_json::Value>,
    /// Reserved: the hosting-terms flag's state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hosting_terms: Option<serde_json::Value>,
}

/// **A built-in declaration, by reference** (R-470): its id and digest, and the purlis version of
/// the desktop that sent it, so a runner whose binary's copy differs names both versions (R-471).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuiltInReference {
    pub id: String,
    pub digest: String,
    /// The desktop's purlis version.
    pub purlis: String,
}

/// **The desktop's locks on a key proxy's upstream**, as its policy fixes them: the hosts any
/// upstream must be among, where fixed, and the hosts none may be.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanLocks {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hosts: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub never_hosts: Vec<String>,
}

impl PlanLocks {
    /// Whether the desktop fixes nothing.
    pub fn is_empty(&self) -> bool {
        self.hosts.is_none() && self.never_hosts.is_empty()
    }
}

/// **The key a plan carries** (R-470), a vault value: **its `Debug` never shows it**, so no log
/// line that prints a command carries it.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PlanKey(String);

impl PlanKey {
    /// The key `value` is.
    pub fn of(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The value, for the key proxy alone.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for PlanKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PlanKey(..)")
    }
}

/// The refusal code of a plan whose built-in doesn't match the runner's binary (R-471): its
/// `why` is the runner's purlis version, which the desktop names beside its own.
pub const PLAN_DOES_NOT_MATCH: &str = "plan_does_not_match";

impl KeyProxyPlan {
    /// The names of the fields reserved for later phases, in the order the plan writes them.
    pub const RESERVED: [&'static str; 6] = [
        "project_declaration",
        "your_declaration",
        "policy_declaration",
        "confirmations",
        "remaining_budget",
        "hosting_terms",
    ];

    /// **Why this plan is refused at arrival**, or `None`: a reserved field set, named, or no
    /// key in the same round trip (R-470).
    pub fn refusal(&self) -> Option<String> {
        // Skeleton (T1a-51): refuses nothing until built.
        None
    }
}
