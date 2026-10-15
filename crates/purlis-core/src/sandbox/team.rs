// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The team's say over each person's sandbox level** (#2071, PR C; the operator's rulings of
//! 2026-10-14, SO-5, SO-8 and SO-9, and of 2026-10-15 on the spelling), in the committed
//! `charter.toml`:
//!
//! ```toml
//! [sandbox]
//! mode = "off"        # or "on"
//! policy = "local"    # or "never"; absent is "local"
//! ```
//!
//! - **`policy = "local"`** (the default): each person's own level on their machine
//!   ([`super::personal::in_force`]) holds, the floor where they chose none.
//! - **`policy = "never"`**: nobody runs below the full sandbox. "Start without the sandbox" is
//!   refused, and a person's own lower level is clamped to the full sandbox, each with a
//!   sentence naming the policy ([`policy::Locks::opt_out_refused`], [`never_said`]). `mode = "off"` beside it
//!   is refused ([`super::Refusal::ModeOff`]).
//! - **`mode = "off"`** under `local` is **the team's off**: the default becomes no sandbox,
//!   and it reaches each person only on their one Accept ([`team_off`], [`answer`]): "Your
//!   project turned the sandbox off (commit <sha> by <author>)". Until they accept, their chats
//!   run at their own level, or the floor. Only an off that is committed asks anyone: a pushed
//!   file never silently weakens anyone, and a working-tree edit turns nothing off. The answer is
//!   kept for that commit in this machine's config home ([`super::local`]), which no sandboxed
//!   chat writes; an off committed again later asks again.
//!
//! **[`allowed_levels`] is the bound** a chat's start clamps the person's level to, and
//! [`level_for`] the level it runs at.

use std::path::Path;
use std::time::Duration;

use super::personal::Choice;
use super::policy;

/// The key of the team's policy, in `[sandbox]`.
pub const KEY: &str = "policy";

/// What `[sandbox] policy` says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Policy {
    /// Each person's own level on their machine.
    #[default]
    Local,
    /// Nobody runs below the full sandbox.
    Never,
}

impl Policy {
    /// The policy `value` names: absent is `local`, and anything that is not `"never"` or
    /// `"local"` is read as `never` (`Err`), the stricter, so a typo never loosens anyone.
    pub fn of_value(value: Option<&toml::Value>) -> Result<Self, Self> {
        match value {
            None => Ok(Self::Local),
            Some(toml::Value::String(word)) if word == "local" => Ok(Self::Local),
            Some(toml::Value::String(word)) if word == "never" => Ok(Self::Never),
            Some(_) => Err(Self::Never),
        }
    }
}

/// What the committed file says of the team's say.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Team {
    pub policy: Policy,
    /// `mode = "off"` under `policy = "local"`: the team's off.
    pub off: bool,
}

impl Team {
    /// What the `[sandbox]` of `top`, a parsed `charter.toml`, says.
    pub fn of_top(top: Option<&toml::Table>) -> Self {
        let Some(table) = top
            .and_then(|top| top.get(super::TABLE))
            .and_then(toml::Value::as_table)
        else {
            return Self::default();
        };
        let policy = Policy::of_value(table.get(KEY)).unwrap_or_else(|stricter| stricter);
        let off = matches!(table.get("mode"), Some(toml::Value::String(word)) if word == "off");
        Self {
            policy,
            off: off && policy == Policy::Local,
        }
    }

    /// What `text`, a whole `charter.toml`, says; nothing where it is not TOML.
    pub fn of_text(text: &str) -> Self {
        Self::of_top(text.parse::<toml::Table>().ok().as_ref())
    }

    /// What the project at `root` says now.
    pub fn of(root: &Path) -> Self {
        super::Plane::read(root).team()
    }
}

/// **The sentence a `never` policy is refused with**, wherever it refuses: Start without the
/// sandbox, a person's own lower level, Settings.
pub fn never_said() -> String {
    format!(
        "This project's policy requires the full sandbox ({}.{KEY} = \"never\" in {}): no chat \
         starts without it, and nobody turns it down on their machine.",
        super::TABLE,
        super::FILE
    )
}

/// The commit a team's off arrived in, and who made it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arrived {
    pub commit: String,
    pub author: String,
}

impl Arrived {
    /// What the ask says, before Accept and Keep it on for me.
    pub fn said(&self) -> String {
        let short: String = self.commit.chars().take(7).collect();
        format!(
            "Your project turned the sandbox off (commit {short} by {}).",
            crate::shown::short(&self.author)
        )
    }
}

/// Where the team's off stands for the person on this machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TeamOff {
    /// The file does not turn it off (or its policy is `never`).
    No,
    /// The file turns it off, but no commit does: nobody is asked, nothing is off.
    NotCommitted,
    /// Committed, and this person has not answered for that commit: asked once.
    Asks(Arrived),
    /// They accepted it: no sandbox by default.
    Accepted(Arrived),
    /// They kept it on: their own level, or the floor.
    KeptOn(Arrived),
}

/// How long the history may take to say which commit turned it off.
const GIT_WAIT: Duration = Duration::from_secs(5);

/// **Where the team's off stands** for the project at `root` on this machine.
pub fn team_off(root: &Path) -> TeamOff {
    if !Team::of(root).off {
        return TeamOff::No;
    }
    let Some(arrived) = arrived(root) else {
        return TeamOff::NotCommitted;
    };
    match super::local::team_off_answer(root) {
        Some(answer) if answer.commit == arrived.commit && answer.accepted => {
            TeamOff::Accepted(arrived)
        }
        Some(answer) if answer.commit == arrived.commit => TeamOff::KeptOn(arrived),
        _ => TeamOff::Asks(arrived),
    }
}

/// The commit that last changed the manifest's `mode`, where the committed file (`HEAD`'s)
/// turns the sandbox off too; none outside git, or where only the working tree says off.
fn arrived(root: &Path) -> Option<Arrived> {
    let manifest = crate::names::manifest(root);
    let name = manifest.file_name()?.to_str()?;
    let git = |args: &[&str]| {
        crate::worktree::git::run(root, args, GIT_WAIT)
            .ok()
            .filter(crate::worktree::git::Run::ok)
            .map(|run| run.out)
    };
    let committed = git(&["show", &format!("HEAD:./{name}")])?;
    if !Team::of_text(&committed).off {
        return None;
    }
    let found = git(&[
        "log",
        "-1",
        "--format=%H%x1f%an",
        "-G",
        "^[[:space:]]*mode[[:space:]]*=",
        "--",
        name,
    ])?;
    let (commit, author) = found.trim().split_once('\u{1f}')?;
    Some(Arrived {
        commit: commit.to_owned(),
        author: author.to_owned(),
    })
}

/// **The person's answer to the team's off** (Accept, or Keep it on for me, or a change of mind
/// in Settings), for `commit`, kept: refused unless that is the commit the off stands at now.
pub fn answer(root: &Path, commit: &str, accept: bool) -> Result<(), String> {
    let answer = answering(root, commit, accept)?;
    super::local::answer_team_off(root, answer)
        .map_err(|why| format!("purlis could not keep your answer on this machine: {why}"))
}

/// [`answer`]'s judgement alone: the answer to keep, or why there is none to give.
pub fn answering(
    root: &Path,
    commit: &str,
    accept: bool,
) -> Result<super::local::TeamOffAnswer, String> {
    match team_off(root) {
        TeamOff::Asks(arrived) | TeamOff::Accepted(arrived) | TeamOff::KeptOn(arrived)
            if arrived.commit == commit =>
        {
            Ok(super::local::TeamOffAnswer {
                commit: arrived.commit,
                accepted: accept,
            })
        }
        TeamOff::Asks(_) | TeamOff::Accepted(_) | TeamOff::KeptOn(_) => Err(
            "The project's sandbox off is from another commit now. Nothing was kept: look at \
             it again."
                .to_owned(),
        ),
        TeamOff::No | TeamOff::NotCommitted => Err(
            "The project does not turn the sandbox off in a commit, so there is nothing to \
             answer."
                .to_owned(),
        ),
    }
}

/// **How far a person may turn a chat's sandbox down** in a project: the lowest level, the
/// level where they chose none, and why it is held, where it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bound {
    /// The lowest level a chat may run at.
    pub lowest: Choice,
    /// The level of a chat whose person chose none.
    pub default: Choice,
    /// Why nobody may go lower, where a policy holds it: shown beside the switch, and given as
    /// the refusal of Start without the sandbox.
    pub why: Option<String>,
}

impl Bound {
    /// `policy = "local"`: every level, the floor by default, or no sandbox where the person
    /// accepted the team's off.
    pub fn local(accepted_off: bool) -> Self {
        Self {
            lowest: Choice::Off,
            default: if accepted_off {
                Choice::Off
            } else {
                Choice::Floor
            },
            why: None,
        }
    }

    /// The full sandbox and nothing lower, held by `why`.
    pub fn full(why: String) -> Self {
        Self {
            lowest: Choice::On,
            default: Choice::On,
            why: Some(why),
        }
    }

    /// **The level a chat runs at**, given what the person chose: their choice, or the default,
    /// never lower than [`Self::lowest`].
    pub fn clamp(&self, chosen: Option<Choice>) -> Choice {
        // `Choice` is ordered strictest first.
        chosen.unwrap_or(self.default).min(self.lowest)
    }

    /// Whether `choice` is one a person may pick.
    pub fn allows(&self, choice: Choice) -> bool {
        choice <= self.lowest
    }
}

/// **The bound for the project at `root` on this machine**: the full sandbox where the team's
/// `never` or an administrator's policy requires it, else every level, by default the floor, or
/// no sandbox once the person accepted the team's off.
pub fn allowed_levels(root: &Path) -> Bound {
    allowed_levels_under(root, &policy::Locks::of(root))
}

/// [`allowed_levels`] under `locks`, the locks a start already read.
pub fn allowed_levels_under(root: &Path, locks: &policy::Locks) -> Bound {
    if let Some(why) = locks.opt_out_refused() {
        return Bound::full(why);
    }
    Bound::local(matches!(team_off(root), TeamOff::Accepted(_)))
}

/// **The level a chat started as `persona` (or none) runs at** in the project at `root`: the
/// person's own level as far as they confirmed it ([`super::personal::in_force`]), within
/// [`allowed_levels`].
pub fn level_for(root: &Path, persona: Option<&str>) -> Choice {
    allowed_levels(root).clamp(super::personal::in_force(root).for_persona(persona))
}

#[cfg(test)]
mod tests;
