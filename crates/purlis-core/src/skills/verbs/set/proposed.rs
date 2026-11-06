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

use purlis_skills::{Grant, Lifecycle};
use serde::{Deserialize, Serialize};

use super::{
    Act, Member, Moving, delete_now, delete_set, lifecycle_set, move_set, one_line, pin_set,
    restore_set, review_set, turn_set,
};
use crate::counts::counted;
use crate::skills::routing::{self, PinnedBy};
use crate::skills::source;
use crate::skills::verbs::{Ctx, Deleted, GRANT_NEEDED, Level, deleted, find, needs_grant};

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
    /// Bring each back from Deleted, as one commit ([`restore_set`]): each the one deleted skill
    /// of its name and level the plane lists ([`listed`]).
    Restore { members: Vec<Member> },
    /// Take each off Deleted now, the one change to skills with no undo ([`delete_now`]): each
    /// as [`Self::Restore`] names it.
    DeleteNow { members: Vec<Member> },
    /// Move each to `to`, as one commit ([`move_set`]). `grant` is where each that goes into the
    /// project's skills reaches, in words ([`Grant::from_words`]), and empty where none does.
    Move {
        members: Vec<Member>,
        to: Level,
        grant: Vec<String>,
    },
    /// Write each, and the project's skills it names, outside purlis: a zip, or a folder of skill
    /// folders. **It names no place** (#5492, the person's choice of 2026-11-06): it is written
    /// only where the person picks, at their terminal or at Apply in the window
    /// ([`Self::export_to`]), never where a chat says.
    Export { members: Vec<Member>, zip: bool },
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
    /// The act its review reads for: `None` for one of skills in Deleted, which have no review
    /// of a set but their own lines ([`Self::shown`]).
    fn act(&self) -> Option<Act> {
        Some(match self {
            Self::Restore { .. } | Self::DeleteNow { .. } => return None,
            Self::Delete { .. } => Act::Delete,
            Self::Move { .. } => Act::Move,
            Self::Export { .. } => Act::Export,
            Self::Turn { on: false, .. } => Act::TurnOff,
            Self::Turn { on: true, .. } => Act::TurnOn,
            Self::Pin { on: true, .. } => Act::Pin,
            Self::Pin { on: false, .. } => Act::Unpin,
            Self::Archive { archive: true, .. } => Act::Archive,
            Self::Archive { archive: false, .. } => Act::Unarchive,
        })
    }

    /// The skills it names, each at its level: a turn's and a pin's, by name, are the project's
    /// and as found.
    fn members(&self) -> Vec<Member> {
        match self {
            Self::Delete { members }
            | Self::Archive { members, .. }
            | Self::Restore { members }
            | Self::DeleteNow { members }
            | Self::Move { members, .. }
            | Self::Export { members, .. } => members.clone(),
            Self::Turn { names, .. } | Self::Pin { names, .. } => names
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
        self.members().len()
    }

    /// **Whether it may be asked at all**: it names a skill, and no more than [`MOST_SKILLS`].
    pub fn checked(&self) -> Result<(), String> {
        if let Self::Move { grant, .. } = self {
            granted(grant)?;
        }
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
            Self::Restore { .. } => format!("Bring back {skills}"),
            Self::DeleteNow { .. } => format!("Delete {skills} now, with no undo"),
            Self::Move { to, .. } => format!("Move {skills} to the {to} skills"),
            Self::Export { zip, .. } => {
                format!(
                    "Export {skills} to a {}",
                    if *zip { "zip" } else { "folder" }
                )
            }
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
        let review = self.reviewed(plane);
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
                self.held_as(plane, &one.member)
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

    /// **Its review**: each skill with why it cannot be part of it, and each left naming one
    /// that goes. A skill in Deleted is reviewed by whether the plane lists it there.
    fn reviewed(&self, plane: &Path) -> super::SetReview {
        let members = self.members();
        let Some(act) = self.act() else {
            return super::SetReview {
                skills: members
                    .into_iter()
                    .map(|member| super::ReviewedMember {
                        cannot: listed(plane, &member).err(),
                        changed: false,
                        member,
                    })
                    .collect(),
                names: Vec::new(),
            };
        };
        let mut review = review_set(plane, act, &members, self.workspace(), self.to());
        if let Self::Move { to, grant, .. } = self {
            for one in &mut review.skills {
                if one.cannot.is_none() && grant.is_empty() && needs_grant(&one.member.level, to) {
                    one.cannot = Some(format!(
                        "It goes into the project's skills, and the move names no grant: {}.",
                        GRANT_NEEDED
                    ));
                }
            }
        }
        review
    }

    /// The level a move goes to.
    fn to(&self) -> Option<&Level> {
        match self {
            Self::Move { to, .. } => Some(to),
            _ => None,
        }
    }

    /// What one skill holds as Apply compares it: its copy's digest, or a skill in Deleted's
    /// entry there.
    fn held_as(&self, plane: &Path, member: &Member) -> String {
        match self {
            Self::Restore { .. } | Self::DeleteNow { .. } => listed(plane, member)
                .ok()
                .and_then(|one| serde_json::to_string(&one).ok())
                .unwrap_or_default(),
            _ => copy_digest(plane, member),
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
            Self::Delete { .. } | Self::Move { .. } | Self::Export { .. } => if found.is_some() {
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
            Self::Restore { .. } | Self::DeleteNow { .. } => if listed(plane, member).is_ok() {
                "deleted"
            } else {
                "not in Deleted"
            }
            .to_owned(),
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
            Self::Restore { .. } => "back".to_owned(),
            Self::DeleteNow { .. } => "gone for good".to_owned(),
            Self::Move { to, .. } => format!("moved to the {to} skills"),
            Self::Export { .. } => "exported".to_owned(),
        }
    }

    /// **Makes it**: every skill its review keeps, as one commit, all or nothing. One the review
    /// takes out (not there, or one it cannot be part of) is left as it is, as the window's
    /// review leaves it.
    pub fn apply(&self, cx: &mut Ctx<'_>) -> Result<(), String> {
        self.checked()?;
        let review = self.reviewed(cx.plane);
        let kept: Vec<Member> = review
            .skills
            .into_iter()
            .filter(|one| one.cannot.is_none())
            .map(|one| one.member)
            .collect();
        if kept.is_empty() {
            return Err(
                "None of these skills can take this change any more, so nothing was changed. \
                 Dismiss it, and ask the chat to propose it again if it is still wanted."
                    .into(),
            );
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
            Self::Restore { .. } => {
                let set = in_deleted(cx.plane, &kept)?;
                restore_set(cx, &set, None)
            }
            Self::Move { to, grant, .. } => {
                let grant = granted(grant)?;
                let moves: Vec<Moving> = kept
                    .into_iter()
                    .map(|member| Moving {
                        member,
                        to: to.clone(),
                        grant: grant.clone(),
                    })
                    .collect();
                move_set(cx, &moves).map(drop)
            }
            Self::Export { .. } => Err(NO_PLACE.to_owned()),
            Self::DeleteNow { .. } => {
                let set = in_deleted(cx.plane, &kept)?;
                let gone = delete_now(cx.plane, &set)?;
                for why in gone.failed {
                    cx.warn(format!("not taken off Deleted: {why}"));
                }
                let taken = gone.off_the_list + gone.removed;
                cx.done(format!(
                    "{} taken off Deleted",
                    counted(taken, "skill", "skills")
                ));
                Ok(())
            }
        }
    }
}

/// Why an export is not written by [`SetChange::apply`].
const NO_PLACE: &str = "An export is written only where the person picks, so nothing was written. \
                        Pick where in the window, or say where at your terminal.";

impl SetChange {
    /// **Writes an export to `to`**, a place the person picked that is not there yet: every skill
    /// its review keeps, and the project's skills they name. Refused for any other change.
    pub fn export_to(
        &self,
        plane: &Path,
        built_in: Option<&Path>,
        to: &Path,
    ) -> Result<(), String> {
        let Self::Export { zip, .. } = self else {
            return Err("only an export is written to a place".to_owned());
        };
        self.checked()?;
        let skills: Vec<(Level, String)> = self
            .reviewed(plane)
            .skills
            .into_iter()
            .filter(|one| one.cannot.is_none())
            .map(|one| (one.member.level, one.member.name))
            .collect();
        if skills.is_empty() {
            return Err(
                "None of these skills can be exported, so nothing was written. `purlis skill \
                 list` shows which are there."
                    .to_owned(),
            );
        }
        let out = source::gather_set(plane, built_in, &skills, source::Purpose::Export)?;
        if *zip {
            source::export_zip(&out, to)
        } else {
            source::export_folder(&out, to)
        }
    }
}

/// **The one deleted skill `member` names**, as the plane lists it in Deleted now: refused where
/// there is none, or more than one of its name and level, which only the window tells apart.
fn listed(plane: &Path, member: &Member) -> Result<Deleted, String> {
    let mut found = deleted(plane)
        .into_iter()
        .filter(|one| one.level == member.level && one.name == member.name);
    match (found.next(), found.next()) {
        (Some(one), None) => Ok(one),
        (None, _) => Err("It is not in Deleted.".to_owned()),
        (Some(_), Some(_)) => Err(
            "It was deleted more than once: choose which in the Skill studio's Deleted list."
                .to_owned(),
        ),
    }
}

/// A move's grant in words, read: `None` where it names none.
fn granted(words: &[String]) -> Result<Option<Grant>, String> {
    if words.is_empty() {
        return Ok(None);
    }
    Grant::from_words(words)
        .map(Some)
        .map_err(|why| format!("{why}, so nothing was moved"))
}

/// [`listed`] of each of `members`.
fn in_deleted(plane: &Path, members: &[Member]) -> Result<Vec<Deleted>, String> {
    members
        .iter()
        .map(|member| {
            listed(plane, member).map_err(|why| format!("{}: {why}", one_line(&member.name)))
        })
        .collect()
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

/// The most files of one skill's folder its digest reads: past it, the count stands for the rest.
const MOST_FILES: usize = 4096;

/// **What a skill's copy holds, as a digest**: every file of its folder by its path in it and its
/// bytes (a link by where it points, never followed), and what purlis keeps beside it. A script
/// or a reference changed between the hold and Apply makes the proposal stale, as its
/// `SKILL.md` does.
fn copy_digest(plane: &Path, member: &Member) -> String {
    let Ok(folder) = find(plane, &member.level, &member.name) else {
        return String::new();
    };
    let mut files = Vec::new();
    every_file(&folder, Path::new(""), &mut files);
    files.sort();
    let mut bytes = Vec::new();
    for (rel, held) in &files {
        bytes.extend(rel.as_bytes());
        bytes.push(0);
        bytes.extend(crate::skills::source::review::sha256(held).as_bytes());
        bytes.push(b'\n');
    }
    bytes.extend(std::fs::read(super::beside(&folder)).unwrap_or_default());
    crate::skills::source::review::sha256(&bytes)
}

/// Each file below `dir` (at `rel` in the skill's folder) with what it holds, into `out`, at most
/// [`MOST_FILES`]: a link's target in place of its bytes, and what cannot be read as a mark.
fn every_file(dir: &Path, rel: &Path, out: &mut Vec<(String, Vec<u8>)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        out.push((rel.display().to_string(), b"unreadable folder".to_vec()));
        return;
    };
    for entry in entries.flatten() {
        if out.len() >= MOST_FILES {
            out.push(("…".to_owned(), out.len().to_string().into_bytes()));
            return;
        }
        let path = entry.path();
        let at = rel.join(entry.file_name());
        let Ok(kind) = entry.file_type() else {
            out.push((at.display().to_string(), b"unreadable".to_vec()));
            continue;
        };
        if kind.is_symlink() {
            let target = std::fs::read_link(&path).unwrap_or_default();
            out.push((
                at.display().to_string(),
                target.display().to_string().into_bytes(),
            ));
        } else if kind.is_dir() {
            every_file(&path, &at, out);
        } else {
            let held = std::fs::read(&path).unwrap_or_else(|_| b"unreadable".to_vec());
            out.push((at.display().to_string(), held));
        }
    }
}

#[cfg(test)]
mod tests;
