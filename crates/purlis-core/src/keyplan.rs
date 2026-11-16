// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A run's key-proxy plan on a runner** (design §3.18, T1a-51, #6807): what the desktop sends
//! with a run's start, and what the runner checks at arrival and keeps for that run alone.
//!
//! The plan is the session protocol's [`KeyProxyPlan`] (1.91). It is treated like a vault value
//! (R-469): it crosses once, the runner keeps it for that run only ([`Plans`]) and discards it,
//! key and all, at run end.
//!
//! **The runner approves nothing of its own** (R-168, R-471). The plan names 1a's built-in by id
//! and digest, and the runner looks for that digest among its own binary's built-ins ([`Binary`]):
//! one it doesn't have refuses the start, naming both purlis versions (W75), so a runner on
//! another build never gets the key. A runner chat then uses the background model and the prices
//! of **its own binary's** built-in (H-18): the digest leaves out `[models] background` and
//! `[[models.table]]`, so they never travel and never have to.
//!
//! **The sign-in kind comes from the plan** (R-652): a runner's kept profile ([`Kept`]) has no
//! `sign_in`, and a run started on it takes the plan's.
//!
//! **No runner takes a plan for a real start yet** (B-9): [`Kept::of`] still refuses a key
//! profile on the desktop, and the runner host refuses the command, until T1a-73's swap. This
//! module is the plan seam those changes plug into.

use std::collections::{BTreeMap, HashMap};

use purlis_session_protocol::keyplan::{
    BuiltInReference, KeyProxyPlan, PLAN_DOES_NOT_MATCH, PlanKey, PlanLocks,
};
use purlis_session_protocol::session::{MALFORMED, Refusal};

use crate::harness_declaration::SignInKind;
use crate::model_provider::{Declaration, Price};
use crate::profilestore::onrunner::Kept;
use crate::window_copy::{fill, line};

/// **What a runner's binary carries**: its purlis version and its built-in declarations.
#[derive(Debug, Clone)]
pub struct Binary {
    pub version: String,
    pub built_ins: Vec<Declaration>,
}

impl Binary {
    /// This build's.
    pub fn this() -> Binary {
        Binary {
            version: env!("CARGO_PKG_VERSION").to_owned(),
            built_ins: crate::model_provider::built_ins().to_vec(),
        }
    }
}

/// **The plan the desktop sends for `run`** (R-470): `declaration` (a built-in) by id and digest,
/// with this build's version, the profile's sign-in kind, the key's bound origins and its pinned
/// certificate's public key, the desktop's locks, and the key itself.
pub fn plan_for(
    run: &str,
    declaration: &Declaration,
    sign_in: SignInKind,
    origins: Vec<String>,
    pinned_key: Option<String>,
    locks: PlanLocks,
    key: PlanKey,
) -> KeyProxyPlan {
    KeyProxyPlan {
        run: run.to_owned(),
        built_in: BuiltInReference {
            id: declaration.id.clone(),
            digest: declaration.digest().to_owned(),
            purlis: env!("CARGO_PKG_VERSION").to_owned(),
        },
        sign_in: sign_in.word().to_owned(),
        template: BTreeMap::new(),
        locks,
        origins,
        pinned_key,
        key: Some(key),
        project_declaration: None,
        your_declaration: None,
        policy_declaration: None,
        confirmations: None,
        remaining_budget: None,
        hosting_terms: None,
    }
}

/// **Why a runner refused a plan at arrival.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// Not 1a's schema: a field it doesn't name, or one of the wrong shape (B-17).
    NotTheSchema(String),
    /// A field reserved for a later phase is set, or no key came with it (R-470).
    Refused(String),
    /// A sign-in kind 1a doesn't route.
    SignIn(String),
    /// The built-in's digest isn't one the runner's binary has (R-471).
    DoesNotMatch {
        id: String,
        /// The desktop's purlis version.
        version: String,
        /// The runner's.
        runner_version: String,
    },
    /// The run holds a plan already: a plan crosses once per run.
    HeldAlready { run: String },
}

impl Refused {
    /// The refusal as the runner answers it on the link.
    pub fn on_the_wire(&self) -> Refusal {
        // Skeleton (T1a-51).
        Refusal {
            code: MALFORMED.into(),
            why: "refused".into(),
        }
    }

    /// The sentence the desktop shows for a start on pool `pool` this refused: W75 where the
    /// built-in didn't match, naming both versions.
    pub fn said(&self, pool: &str) -> String {
        // Skeleton (T1a-51).
        let _ = (pool, fill, line::W10);
        "refused".into()
    }
}

/// **A plan as it arrives**, one line of JSON: read whole against 1a's schema, or refused (B-17,
/// R-470).
pub fn arrive(json: &str) -> Result<KeyProxyPlan, Refused> {
    serde_json::from_str(json).map_err(|e| Refused::NotTheSchema(e.to_string()))
}

/// **A plan the runner keeps for its run**, checked.
#[derive(Debug, Clone)]
pub struct Held {
    pub run: String,
    /// The runner's own binary's built-in the plan named by id and digest (R-168, H-18).
    pub declaration: Declaration,
    /// The sign-in kind the plan names (R-652).
    pub sign_in: SignInKind,
    pub template: BTreeMap<String, String>,
    pub locks: PlanLocks,
    pub origins: Vec<String>,
    pub pinned_key: Option<String>,
    pub key: PlanKey,
}

/// **What a run on a runner starts as**: the kept profile's name and harness, and the plan's
/// sign-in kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunnerStart {
    pub profile: String,
    pub harness: String,
    pub sign_in: SignInKind,
}

impl Held {
    /// The run's background model: its own binary's built-in's (H-18).
    pub fn background_model(&self) -> Option<&str> {
        self.declaration.background_model()
    }

    /// The price of `model` in its own binary's built-in's table (H-18).
    pub fn price_of(&self, model: &str) -> Option<&Price> {
        self.declaration
            .models
            .table
            .iter()
            .find(|row| row.id == model)
            .and_then(|row| row.price.as_ref())
    }

    /// The run, started on the runner's kept profile `kept`, with this plan's sign-in kind
    /// (R-652).
    pub fn start_on(&self, kept: &Kept) -> RunnerStart {
        RunnerStart {
            profile: kept.name.clone(),
            harness: kept.harness.clone(),
            sign_in: self.sign_in,
        }
    }
}

/// **The plans a runner holds, one per run** (R-469): taken at a run's start, discarded at its
/// end.
#[derive(Debug)]
pub struct Plans {
    binary: Binary,
    held: HashMap<String, Held>,
}

impl Plans {
    /// None held yet, on a runner whose binary is `binary`.
    pub fn on(binary: Binary) -> Plans {
        Plans {
            binary,
            held: HashMap::new(),
        }
    }

    /// **Checks `plan` at arrival and keeps it for its run**, or refuses it.
    pub fn take(&mut self, plan: KeyProxyPlan) -> Result<&Held, Refused> {
        // Skeleton (T1a-51): checks nothing until built.
        let run = plan.run.clone();
        let held = Held {
            run: run.clone(),
            declaration: self.binary.built_ins[0].clone(),
            sign_in: SignInKind::Subscription,
            template: plan.template,
            locks: plan.locks,
            origins: plan.origins,
            pinned_key: plan.pinned_key,
            key: plan.key.unwrap_or_else(|| PlanKey::of("")),
        };
        Ok(self.held.entry(run).or_insert(held))
    }

    /// The plan `run` holds, while it runs.
    pub fn held(&self, run: &str) -> Option<&Held> {
        self.held.get(run)
    }

    /// **`run` ended**: its plan and key are discarded.
    pub fn end(&mut self, run: &str) {
        let _ = (run, MALFORMED, PLAN_DOES_NOT_MATCH);
    }

    /// How many runs hold a plan.
    pub fn len(&self) -> usize {
        self.held.len()
    }

    /// Whether no run holds one.
    pub fn is_empty(&self) -> bool {
        self.held.is_empty()
    }
}

#[cfg(test)]
mod tests;
