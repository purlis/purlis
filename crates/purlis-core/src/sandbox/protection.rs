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
    /// The project, where its file turns the sandbox on or its `policy = "never"` requires it.
    Project,
    /// The person, by their own level on this machine, set or confirmed in Settings.
    Person,
    /// The person, for this chat only.
    Chat,
    /// No tier set it: purlis's default floor, or nothing could be applied (no sandbox backend
    /// on this system, or a floor that could not be compiled here).
    Purlis,
    /// A level only the project's local file says, never set or confirmed in Settings. A chat
    /// can write that file, so it is never said to be the person's (Q238).
    Unconfirmed,
    /// A tier a newer version wrote that this one does not know: read, never refused, so the
    /// start it is part of is still read. Never written.
    #[serde(other)]
    Unknown,
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
    /// commit has it (no repository, or the file is not in its HEAD), and
    /// [`crate::eventlog::UNKNOWN`] where git did not answer, refused or failed.
    pub charter_commit: String,
    /// Whether the working `charter.toml` differs from that commit's: the start ran under text
    /// no commit has. False where there is no commit to differ from.
    #[serde(default)]
    pub charter_dirty: bool,
    /// The answers in force (the Accepts of a project's recommendations): none in phase 1. Each
    /// is kept as written, so an answer of a shape a later phase gives is still read.
    #[serde(default)]
    pub answers_in_force: Vec<serde_json::Value>,
}

/// Reads a start's `protection` leniently: one in a shape this version cannot read is not
/// known (`None`), never a refusal of the whole entry it is part of (Q238).
pub fn lenient<'de, D>(read: D) -> Result<Option<InForce>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = <serde_json::Value as serde::Deserialize>::deserialize(read)?;
    Ok(serde_json::from_value(value).ok())
}

/// **The protection a chat of `persona`, made `made`, starts under** in the project at `root`:
/// under `sandbox` where the start applied one, without it where it `lifted` it, or with
/// neither where the floor could not be applied or the program is not one purlis sandboxes.
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
        // No sandbox, and none lifted: the floor could not be applied here, or the program is
        // not one purlis sandboxes (a shell, a declared harness with no adapter). The chat
        // starts with no sandbox, by no tier's choice.
        (None, None) => (Choice::Off, Tier::Purlis),
    };
    let (charter_commit, charter_dirty) = charter_commit(root);
    InForce {
        layers: vec![Layer {
            layer: SANDBOX.to_owned(),
            level: level.word().to_owned(),
            tier,
        }],
        charter_commit,
        charter_dirty,
        answers_in_force: Vec::new(),
    }
}

/// The tier that set the level of a sandboxed start, by why it runs at it
/// ([`level_why::next_start`]) (Q238).
fn tier_of(root: &Path, persona: Option<&str>, made: &Made) -> Tier {
    let locks = policy::Locks::of(root);
    // Where no tier of the person's set it: the project's, only where its file turns the
    // sandbox on, and otherwise purlis's default floor.
    let own = if super::Plane::read(root).in_force(&locks).is_some() {
        Tier::Project
    } else {
        Tier::Purlis
    };
    let Some(next) = level_why::next_start(root, persona, &locks, made) else {
        return Tier::Purlis;
    };
    match next.why {
        Why::Policy(_) if locks.policy_forbids_opt_out() => Tier::Policy,
        Why::Policy(_) => Tier::Project,
        // A start that names no way it was made reads no level of the person's
        // ([`level_why::next_start`]), whatever their level happens to be.
        _ if matches!(made, Made::Unnamed) => own,
        Why::Chosen | Why::AskedByStricter => Tier::Person,
        Why::FileOnly => Tier::Unconfirmed,
        // The row's level, or, waiting for Confirm, what is in force instead: the person's
        // where they confirmed it, the file's alone where they did not, and the project's own
        // or the floor where neither says one.
        Why::Persona(_) | Why::WaitsForConfirm(_) => by_the_person(root, persona).unwrap_or(own),
        Why::NoneChosen | Why::ChainUnread | Why::PeerStart => own,
    }
}

/// Whose is the level in force for a chat on `persona` by the person's own levels:
/// [`Tier::Person`] where they set or confirmed it in Settings, [`Tier::Unconfirmed`] where only
/// the local file says it, and `None` where they have none.
fn by_the_person(root: &Path, persona: Option<&str>) -> Option<Tier> {
    let in_force = personal::in_force(root).for_persona(persona)?;
    Some(
        if super::local::confirmed_levels(root).for_persona(persona) == Some(in_force) {
            Tier::Person
        } else {
            Tier::Unconfirmed
        },
    )
}

/// How long git may take to name `charter.toml`'s commit at a start.
const GIT_WAIT: std::time::Duration = std::time::Duration::from_secs(2);

/// The last commit of the project's `charter.toml` at `root`, as git names it, and whether the
/// working file differs from it.
///
/// **`none` only where no commit can have it**: no repository at or above `root`, or a HEAD
/// that does not hold the file. A repository git refused or failed to read is `unknown`, so a
/// chat that can write `.git/config` cannot make the record say there is no commit.
fn charter_commit(root: &Path) -> (String, bool) {
    let unknown = || (crate::eventlog::UNKNOWN.to_owned(), false);
    let none = || (crate::eventlog::NONE.to_owned(), false);
    let git = |args: &[&str]| crate::worktree::git::run(root, args, GIT_WAIT).ok();
    let Some(log) = git(&["log", "-1", "--format=%H", "--", super::FILE]) else {
        return unknown();
    };
    if !log.ok() {
        let in_a_repository = root.ancestors().any(|dir| dir.join(".git").exists());
        return if in_a_repository { unknown() } else { none() };
    }
    let commit = log.out.trim();
    if commit.is_empty() {
        return none();
    }
    // The working file against HEAD's: changed, staged, or gone from HEAD and left untracked
    // (the last commit that touched it then removed it).
    let Some(status) = git(&[
        "status",
        "--porcelain=v1",
        "--untracked-files=all",
        "--",
        super::FILE,
    ])
    .filter(crate::worktree::git::Run::ok) else {
        return unknown();
    };
    let said = status.out.trim_end();
    if said.starts_with("??") {
        return none();
    }
    (commit.to_owned(), !said.is_empty())
}

#[cfg(test)]
mod tests;
