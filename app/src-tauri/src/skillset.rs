// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The window's commands for a change to several skills at once** (#5331): what is installed
//! from a source, the one review, and the one commit that deletes, brings back or turns them
//! (`purlis_core::skills::verbs::set`). Each change is the window's alone
//! (`purlis_session_protocol::ui::WINDOW_ONLY`): a chat may propose one, and the person approves.

use purlis_core::skills::verbs::{self, set};
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::skillchange::{SkillLevel, flag_on, heard};
use crate::skillsources::{SkillSource, SourceRemote};

/// One skill of a set: its level and name, and its level as a person reads it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SkillSetMember {
    pub level: SkillLevel,
    pub name: String,
    pub label: String,
}

impl From<&set::Member> for SkillSetMember {
    fn from(member: &set::Member) -> Self {
        SkillSetMember {
            level: (&member.level).into(),
            name: member.name.clone(),
            label: member.level.to_string(),
        }
    }
}

impl SkillSetMember {
    fn core(&self) -> set::Member {
        set::Member {
            level: (&self.level).into(),
            name: self.name.clone(),
        }
    }
}

/// What is asked of a set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum SkillSetAct {
    Delete,
    TurnOff,
    TurnOn,
}

impl From<SkillSetAct> for set::Act {
    fn from(act: SkillSetAct) -> Self {
        match act {
            SkillSetAct::Delete => set::Act::Delete,
            SkillSetAct::TurnOff => set::Act::TurnOff,
            SkillSetAct::TurnOn => set::Act::TurnOn,
        }
    }
}

/// A copy installed from a source that is still there.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct InstalledCopy {
    pub member: SkillSetMember,
    /// Whether it was edited here since it was installed.
    pub changed: bool,
    /// Whether it is on in the workspace asked of; none where that workspace cannot turn it.
    pub on_here: Option<bool>,
}

/// One skill as a set's review shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct SkillSetReviewed {
    pub member: SkillSetMember,
    pub changed: bool,
    /// Why it cannot be part of this change: the review takes it out before anything is written.
    pub cannot: Option<String>,
}

/// A skill that stays naming one that goes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct SkillSetNaming {
    pub namer: String,
    pub named: String,
    /// The skill the set could take in, so nothing is left naming what is not there.
    pub add: SkillSetMember,
}

/// What a set's review says before anything is written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct SkillSetReview {
    pub skills: Vec<SkillSetReviewed>,
    pub names: Vec<SkillSetNaming>,
    /// How many open chats started with one of these and keep it until they restart.
    pub open_chats: u32,
}

/// What a set's delete did, and what an Undo hands back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct SkillSetDeleted {
    pub notes: Vec<String>,
    /// Each deleted skill's id, as the Deleted page lists it.
    pub deleted: Vec<String>,
}

/// What a set's turn did, and what an Undo turns back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct SkillSetTurned {
    pub notes: Vec<String>,
    /// Those it changed: one that already was so is not among them.
    pub changed: Vec<String>,
}

/// **The copies installed from the source `remote`**, read from the lockfiles and asking the
/// source nothing, each with whether it changed since install and is on in `workspace`.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_source_installed(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    remote: SourceRemote,
    workspace: Option<String>,
) -> Result<Vec<InstalledCopy>, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reading what is installed from the source", move || {
        flag_on()?;
        Ok(
            set::installed_from(held.root(), &remote.core(), workspace.as_deref())
                .iter()
                .map(|one| InstalledCopy {
                    member: (&one.member).into(),
                    changed: one.changed,
                    on_here: one.on_here,
                })
                .collect(),
        )
    })
    .await
}

/// **The one review of `act` on `members`**: what each is marked with, who is left naming one that
/// goes, and how many open chats started with them. Reads only.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_set_review(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    act: SkillSetAct,
    members: Vec<SkillSetMember>,
    workspace: Option<String>,
) -> Result<SkillSetReview, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reading the review of these skills", move || {
        flag_on()?;
        let members: Vec<set::Member> = members.iter().map(SkillSetMember::core).collect();
        let review = set::review_set(held.root(), act.into(), &members, workspace.as_deref());
        let names: Vec<&str> = members.iter().map(|one| one.name.as_str()).collect();
        Ok(SkillSetReview {
            skills: review
                .skills
                .iter()
                .map(|one| SkillSetReviewed {
                    member: (&one.member).into(),
                    changed: one.changed,
                    cannot: one.cannot.clone(),
                })
                .collect(),
            names: review
                .names
                .iter()
                .map(|one| SkillSetNaming {
                    namer: one.namer.clone(),
                    named: one.named.clone(),
                    add: (&one.add).into(),
                })
                .collect(),
            open_chats: held.chats().open_using(&names),
        })
    })
    .await
}

/// **Deletes `members` as one change**, the group said to be `from`, with `remove_source` taken out
/// in the same commit. All or nothing.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_set_delete(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    members: Vec<SkillSetMember>,
    from: Option<String>,
    remove_source: Option<SourceRemote>,
) -> Result<SkillSetDeleted, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("deleting the skills", move || {
        flag_on()?;
        let members: Vec<set::Member> = members.iter().map(SkillSetMember::core).collect();
        let remote = remove_source.as_ref().map(SourceRemote::core);
        let mut gone = Vec::new();
        let said = heard(held.root(), |cx| {
            gone = set::delete_set(cx, &members, from.as_deref(), remote.as_ref())?;
            Ok(())
        })?;
        let deleted = gone
            .iter()
            .map(|one| {
                serde_json::to_string(one)
                    .map_err(|_| format!("{} could not be listed to restore", one.name))
            })
            .collect::<Result<_, _>>()?;
        Ok(SkillSetDeleted {
            notes: said.notes,
            deleted,
        })
    })
    .await
}

/// **Brings back every deleted skill `ids` names as one change**, with `source` put back in the
/// same commit where a delete took it out. All or nothing.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_set_restore(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    ids: Vec<String>,
    source: Option<SkillSource>,
) -> Result<crate::skillchange::SkillChanged, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("bringing the skills back", move || {
        flag_on()?;
        let set_of: Vec<verbs::Deleted> = ids
            .iter()
            .map(|id| {
                serde_json::from_str(id)
                    .map_err(|_| "That deleted skill is not one this project lists.".to_owned())
            })
            .collect::<Result<_, String>>()?;
        let core = source.as_ref().map(|one| (one.core(), one.kept.into()));
        heard(held.root(), |cx| {
            set::restore_set(cx, &set_of, core.as_ref().map(|(one, which)| (one, *which)))
        })
    })
    .await
}

/// **Turns `names` off or on in `workspace` as one change.** All or nothing.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_set_turn(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    workspace: String,
    names: Vec<String>,
    on: bool,
) -> Result<SkillSetTurned, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("turning the skills on or off", move || {
        flag_on()?;
        let mut changed = Vec::new();
        let said = heard(held.root(), |cx| {
            changed = set::turn_set(cx, &workspace, &names, on)?;
            Ok(())
        })?;
        Ok(SkillSetTurned {
            notes: said.notes,
            changed,
        })
    })
    .await
}
