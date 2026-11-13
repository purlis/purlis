// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The protection a chat started under** (R-227): what its start event records, so a reader
//! of the chat's story sees what protected it without asking the chat. Per layer, the effective
//! protection and the tier that set it; the `charter.toml` commit the project was at; and the
//! answers in force, none until a project's recommendations can be accepted (phase 2).
//!
//! **Read from what the start resolved, never from the chat**: the level is what the start
//! applied (its [`super::Applied`], or the [`super::Lifted`] it started without), and the tier
//! is the reason [`super::level_why`] gives for that level. It decides nothing.

use std::path::Path;

use super::level_why::{self, Why};
use super::personal::{self, Choice};
use super::{Applied, By, Level, Lifted, Made, policy};

/// Who set a layer's protection, by the tiers of who judges a chat (ADR 0124).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    /// An administrator's policy on this machine.
    Policy,
    /// The project: its own level, or its `policy = "never"`.
    Project,
    /// The person, by their own level on this machine.
    Person,
    /// The person, for this chat only.
    Chat,
    /// purlis itself, where no tier's level could be applied: no sandbox backend on this
    /// system, or a floor that could not be compiled here.
    Purlis,
}

/// One layer's protection at a start.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Layer {
    /// The layer, by name: `sandbox`, the only one a tier sets in phase 1.
    pub layer: String,
    /// Its effective protection: the sandbox level's word (`on`, `floor`, `off`).
    pub level: String,
    /// Who set it.
    pub tier: Tier,
}

/// The name of the sandbox layer.
pub const SANDBOX: &str = "sandbox";

/// What a chat's start event records of its protection (R-227).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct InForce {
    /// Each layer's effective protection and the tier that set it.
    pub layers: Vec<Layer>,
    /// The last commit of `charter.toml`, as git names it; [`crate::eventlog::NONE`] where no
    /// commit has it, and [`crate::eventlog::UNKNOWN`] where git did not answer.
    pub charter_commit: String,
    /// The answers in force (the Accepts of a project's recommendations): none in phase 1.
    pub answers_in_force: Vec<String>,
}

/// **The protection a chat of `persona`, made `made`, starts under** in the project at `root`:
/// under `sandbox` where the start applied one, without it where it `lifted` it, or with
/// neither where the floor could not be applied.
pub fn at_start(
    root: &Path,
    persona: Option<&str>,
    made: &Made,
    sandbox: Option<&Applied>,
    lifted: Option<&Lifted>,
) -> InForce {
    let (level, tier) = match (sandbox, lifted) {
        (_, Some(lifted)) => (
            Choice::Off,
            match lifted.by {
                By::Person => Tier::Chat,
                By::YourLevel(_) => Tier::Person,
                By::NoBackend(_) => Tier::Purlis,
            },
        ),
        (Some(applied), None) => (
            match applied.level() {
                Level::Full => Choice::On,
                Level::Floor => Choice::Floor,
            },
            tier_of(root, persona, made),
        ),
        // The floor could not be applied here: the chat starts with no sandbox, by no tier's
        // choice.
        (None, None) => (Choice::Off, Tier::Purlis),
    };
    InForce {
        layers: vec![Layer {
            layer: SANDBOX.to_owned(),
            level: level.word().to_owned(),
            tier,
        }],
        charter_commit: charter_commit(root),
        answers_in_force: Vec::new(),
    }
}

/// The tier that set the level of a sandboxed start, by why it runs at it
/// ([`level_why::next_start`]).
fn tier_of(root: &Path, persona: Option<&str>, made: &Made) -> Tier {
    let locks = policy::Locks::of(root);
    let Some(next) = level_why::next_start(root, persona, &locks, made) else {
        return Tier::Project;
    };
    match next.why {
        Why::Policy(_) if locks.policy_forbids_opt_out() => Tier::Policy,
        Why::Policy(_) => Tier::Project,
        Why::Chosen | Why::FileOnly | Why::Persona(_) | Why::AskedByStricter => Tier::Person,
        // The file's lower level does not count: what does is what the person confirmed, or
        // the project's own.
        Why::WaitsForConfirm(_) => {
            if personal::in_force(root).for_persona(persona).is_some() {
                Tier::Person
            } else {
                Tier::Project
            }
        }
        Why::NoneChosen | Why::ChainUnread | Why::PeerStart => Tier::Project,
    }
}

/// How long git may take to name `charter.toml`'s commit at a start.
const GIT_WAIT: std::time::Duration = std::time::Duration::from_secs(2);

/// The last commit of the project's `charter.toml` at `root`, as git names it.
fn charter_commit(root: &Path) -> String {
    let said = crate::worktree::git::run(
        root,
        &["log", "-1", "--format=%H", "--", super::FILE],
        GIT_WAIT,
    );
    match said {
        Ok(run) if run.ok() => {
            let commit = run.out.trim();
            if commit.is_empty() {
                crate::eventlog::NONE.to_owned()
            } else {
                commit.to_owned()
            }
        }
        // Not a repository is a project with no commit; git that did not run says nothing.
        Ok(_) => crate::eventlog::NONE.to_owned(),
        Err(_) => crate::eventlog::UNKNOWN.to_owned(),
    }
}

#[cfg(test)]
mod tests;
