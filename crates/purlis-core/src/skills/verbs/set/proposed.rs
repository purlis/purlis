// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A change to several skills that a chat proposes, and only the person makes** (#5357; the
//! operator's ruling of 2026-11-05). `purlis skill rm`, `off`, `on`, `pin` and `archive` with
//! several names or `--source` make a [`SetChange`]: from the person's terminal it is applied at
//! once, after its review; from a chat it travels to the app as
//! `crate::hookwire::Ask::ProposeSkillSet`, is held with the other proposed changes (#1975), and
//! is applied only when the person presses Apply in the window.
//!
//! **What the person is shown and what Apply makes are read here, from the plane, never from the
//! ask**: the ask names skills and an act, and nothing else. [`SetChange::shown`] draws each skill
//! as it is now and as the change leaves it, with the review's warnings, and its
//! [`Shown::state`] is what Apply compares: a skill edited, deleted or turned since makes the
//! proposal stale, and it is never applied to a plane other than the one shown.

use std::path::Path;

use purlis_skills::Lifecycle;
use serde::{Deserialize, Serialize};

use super::{Act, Member, delete_set, lifecycle_set, pin_set, review_set, turn_set};
use crate::counts::counted;
use crate::skills::routing::{self, PinnedBy};
use crate::skills::verbs::{Ctx, Level, find};

/// The most skills one change names: a source's whole set fits, and an ask stays bounded.
pub const MOST_SKILLS: usize = 500;

/// **A change to several skills**, as the person's terminal makes it and a chat proposes it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "act", rename_all = "snake_case")]
pub enum SetChange {
    /// Delete each, as one commit ([`delete_set`]).
    Delete { members: Vec<Member> },
    /// Turn each project skill off (`on` false) or on in `workspace` ([`turn_set`]).
    Turn {
        workspace: String,
        names: Vec<String>,
        on: bool,
    },
    /// Pin each for `by`'s chats, or unpin it ([`pin_set`]).
    Pin {
        by: PinnedBy,
        names: Vec<String>,
        on: bool,
    },
    /// Archive each (`archive`), or put it back in use ([`lifecycle_set`]).
    Archive { members: Vec<Member>, archive: bool },
}

/// **A change as the person is shown it**: each skill as it is and as the change leaves it, one
/// line each, and the plane's state it was read from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shown {
    pub before: String,
    pub after: String,
    /// What Apply compares with the state it was shown at: another means the plane moved.
    pub state: String,
}

impl SetChange {
    /// The act its review reads for.
    pub fn act(&self) -> Act {
        match self {
            Self::Delete { .. } => Act::Delete,
            Self::Turn { on: false, .. } => Act::TurnOff,
            Self::Turn { on: true, .. } => Act::TurnOn,
            Self::Pin { on: true, .. } => Act::Pin,
            Self::Pin { on: false, .. } => Act::Unpin,
            Self::Archive { archive: true, .. } => Act::Archive,
            Self::Archive { archive: false, .. } => Act::Unarchive,
        }
    }

    /// The skills it names, each at its level: a turn's and a pin's, by name, are the project's
    /// and as found.
    fn members(&self) -> Vec<Member> {
        match self {
            Self::Delete { members } | Self::Archive { members, .. } => members.clone(),
            Self::Turn { names, .. } => names
                .iter()
                .map(|name| Member {
                    level: Level::project(),
                    name: name.clone(),
                })
                .collect(),
            Self::Pin { names, .. } => names
                .iter()
                .map(|name| Member {
                    level: Level::project(),
                    name: name.clone(),
                })
                .collect(),
        }
    }

    fn workspace(&self) -> Option<&str> {
        match self {
            Self::Turn { workspace, .. } => Some(workspace),
            _ => None,
        }
    }

    fn count(&self) -> usize {
        match self {
            Self::Delete { members } | Self::Archive { members, .. } => members.len(),
            Self::Turn { names, .. } | Self::Pin { names, .. } => names.len(),
        }
    }

    /// **Whether it may be asked at all**: it names a skill, and no more than [`MOST_SKILLS`].
    pub fn checked(&self) -> Result<(), String> {
        match self.count() {
            0 => Err("no skill was named, so nothing was changed".to_owned()),
            n if n > MOST_SKILLS => Err(format!(
                "one change names at most {MOST_SKILLS} skills, and this one names {n}: nothing \
                 was changed"
            )),
            _ => Ok(()),
        }
    }

    /// **What it is called, in a few words**: the proposal's name in the window.
    pub fn title(&self) -> String {
        let skills = counted(self.count(), "skill", "skills");
        match self {
            Self::Delete { .. } => format!("Delete {skills}"),
            Self::Turn { workspace, on, .. } => {
                let state = if *on { "on" } else { "off" };
                format!("Turn {skills} {state} in {workspace}")
            }
            Self::Pin { by, on, .. } => {
                let word = if *on { "Pin" } else { "Unpin" };
                format!("{word} {skills} for {}", whose(by))
            }
            Self::Archive { archive, .. } => {
                let word = if *archive { "Archive" } else { "Unarchive" };
                format!("{word} {skills}")
            }
        }
    }

    /// **What the person is shown**, read from the plane at `plane` now.
    pub fn shown(&self, plane: &Path) -> Shown {
        let review = review_set(plane, self.act(), &self.members(), self.workspace(), None);
        let (mut before, mut after, mut state) = (String::new(), String::new(), String::new());
        for one in &review.skills {
            let label = self.label(&one.member);
            let now = self.state_of(plane, &one.member);
            before.push_str(&format!("{label}: {now}\n"));
            match &one.cannot {
                Some(why) => after.push_str(&format!("{label}: stays ({})\n", one_line(why))),
                None => after.push_str(&format!("{label}: {}\n", self.state_after())),
            }
            state.push_str(&format!(
                "{label}\t{now}\t{}\n",
                copy_digest(plane, &one.member)
            ));
        }
        for naming in &review.names {
            let says = match self {
                Self::Turn { on: true, .. } => "names",
                _ => "still names",
            };
            after.push_str(&format!(
                "! {} {says} {}\n",
                one_line(&naming.namer),
                one_line(&naming.named)
            ));
        }
        state.push_str(&after);
        Shown {
            before,
            after,
            state,
        }
    }

    /// One skill's name as its line starts: a delete's and an archive's with its level.
    fn label(&self, member: &Member) -> String {
        let name = one_line(&member.name);
        match self {
            Self::Pin { .. } => name,
            _ => format!("{name} · {}", member.level),
        }
    }

    /// One skill's state now, in the words of this change.
    fn state_of(&self, plane: &Path, member: &Member) -> String {
        let found = find(plane, &member.level, &member.name).ok();
        match self {
            Self::Delete { .. } => if found.is_some() {
                "there"
            } else {
                "not there"
            }
            .to_owned(),
            Self::Turn { workspace, .. } => {
                if crate::skills::turned_off(plane, workspace).contains(&member.name) {
                    format!("off in {workspace}")
                } else {
                    format!("on in {workspace}")
                }
            }
            Self::Pin { by, .. } => if routing::pins_written(plane, by).contains(&member.name) {
                "pinned"
            } else {
                "not pinned"
            }
            .to_owned(),
            Self::Archive { .. } => match found.map(|folder| purlis_skills::about_of(&folder)) {
                Some(Ok(about)) => lifecycle_said(&about.lifecycle).to_owned(),
                Some(Err(_)) => "in a state purlis does not read".to_owned(),
                None => "not there".to_owned(),
            },
        }
    }

    /// Every skill's state once it is made.
    fn state_after(&self) -> String {
        match self {
            Self::Delete { .. } => "deleted".to_owned(),
            Self::Turn { workspace, on, .. } => {
                format!("{} in {workspace}", if *on { "on" } else { "off" })
            }
            Self::Pin { on, .. } => if *on { "pinned" } else { "not pinned" }.to_owned(),
            Self::Archive { archive, .. } => {
                if *archive { "archived" } else { "in use" }.to_owned()
            }
        }
    }

    /// **Makes it**: every skill its review keeps, as one commit, all or nothing. One the review
    /// takes out (not there, or one it cannot be part of) is left as it is, as the window's
    /// review leaves it.
    pub fn apply(&self, cx: &mut Ctx<'_>) -> Result<(), String> {
        self.checked()?;
        let review = review_set(
            cx.plane,
            self.act(),
            &self.members(),
            self.workspace(),
            None,
        );
        let kept: Vec<Member> = review
            .skills
            .into_iter()
            .filter(|one| one.cannot.is_none())
            .map(|one| one.member)
            .collect();
        if kept.is_empty() {
            return Err("none of these skills can be changed so, so nothing was changed".into());
        }
        let names: Vec<String> = kept.iter().map(|one| one.name.clone()).collect();
        match self {
            Self::Delete { .. } => delete_set(cx, &kept, None, None).map(drop),
            Self::Turn { workspace, on, .. } => turn_set(cx, workspace, &names, *on).map(drop),
            Self::Pin { by, on, .. } => pin_set(cx, by, &names, *on).map(drop),
            Self::Archive { archive, .. } => {
                let to = if *archive {
                    Lifecycle::Archived
                } else {
                    Lifecycle::Active
                };
                let changes: Vec<(Member, Lifecycle)> =
                    kept.into_iter().map(|one| (one, to.clone())).collect();
                lifecycle_set(cx, &changes).map(drop)
            }
        }
    }
}

/// Whose chats `by` names, in words.
fn whose(by: &PinnedBy) -> String {
    match by {
        PinnedBy::Workspace(ws) => format!("{}'s chats", one_line(ws)),
        PinnedBy::Persona(persona) => format!("chats as {}", one_line(persona)),
    }
}

/// A lifecycle state, in words.
fn lifecycle_said(lifecycle: &Lifecycle) -> &'static str {
    match lifecycle {
        Lifecycle::Active => "in use",
        Lifecycle::Deprecated(_) => "deprecated",
        Lifecycle::Archived => "archived",
    }
}

/// What a skill's copy holds, as a digest: its `SKILL.md` and what purlis keeps beside it.
fn copy_digest(plane: &Path, member: &Member) -> String {
    let Ok(folder) = find(plane, &member.level, &member.name) else {
        return String::new();
    };
    let mut bytes = std::fs::read(folder.join(purlis_skills::FILE)).unwrap_or_default();
    bytes.extend(std::fs::read(super::beside(&folder)).unwrap_or_default());
    crate::skills::source::review::sha256(&bytes)
}

/// A name as a line can hold it: one line, every control character a space.
fn one_line(text: &str) -> String {
    super::one_line(text)
}

#[cfg(test)]
mod tests;
