// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The window's commands for a change to several skills at once** (#5331): what is installed
//! from a source, the one review, and the one commit that deletes, brings back or turns them
//! (`purlis_core::skills::verbs::set`); and the Library's Select mode's (#5332), which pins,
//! archives, moves and takes skills off Deleted the same way. Each change is the window's alone
//! (`purlis_session_protocol::ui::WINDOW_ONLY`): a chat may propose one, and the person approves.

use purlis_core::skills::source::update::set as set_update;
use purlis_core::skills::verbs::{self, set};
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::skillchange::{SkillGrant, SkillLevel, SkillPinner, heard, heard_audited};
use crate::skillhistory::SkillLifecycle;
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
    Pin,
    Unpin,
    Archive,
    Unarchive,
    Move,
    Export,
}

impl From<SkillSetAct> for set::Act {
    fn from(act: SkillSetAct) -> Self {
        match act {
            SkillSetAct::Delete => set::Act::Delete,
            SkillSetAct::TurnOff => set::Act::TurnOff,
            SkillSetAct::TurnOn => set::Act::TurnOn,
            SkillSetAct::Pin => set::Act::Pin,
            SkillSetAct::Unpin => set::Act::Unpin,
            SkillSetAct::Archive => set::Act::Archive,
            SkillSetAct::Unarchive => set::Act::Unarchive,
            SkillSetAct::Move => set::Act::Move,
            SkillSetAct::Export => set::Act::Export,
        }
    }
}

impl SkillSetAct {
    /// Whether an open chat that started with one of the skills keeps it until it restarts: it
    /// goes, or stops loading, for the chats started after.
    fn leaves_open_chats(self) -> bool {
        matches!(
            self,
            SkillSetAct::Delete | SkillSetAct::TurnOff | SkillSetAct::Archive | SkillSetAct::Move
        )
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
    /// The copy named: what the review offers to keep out of a delete or a turn-off, or to take
    /// into a turn-on.
    pub member: SkillSetMember,
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

/// **The copies installed from `source`**, a git source or an extension's (#3092), read from
/// the lockfiles and asking the source nothing, each with whether it changed since install and is
/// on in `workspace`.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_source_installed(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    source: crate::skillsources::SourceOrigin,
    workspace: Option<String>,
) -> Result<Vec<InstalledCopy>, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reading what is installed from the source", move || {
        Ok(
            set::installed_from(held.root(), &source.core(), workspace.as_deref())
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
    to: Option<SkillLevel>,
) -> Result<SkillSetReview, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reading the review of these skills", move || {
        let members: Vec<set::Member> = members.iter().map(SkillSetMember::core).collect();
        let to: Option<verbs::Level> = to.as_ref().map(Into::into);
        let review = set::review_set(
            held.root(),
            act.into(),
            &members,
            workspace.as_deref(),
            to.as_ref(),
        );
        // Those the review keeps: one taken out of it changes in no chat.
        let files: Vec<std::path::PathBuf> = review
            .skills
            .iter()
            .filter(|one| one.cannot.is_none())
            .filter_map(|one| set::copy_file(held.root(), &one.member))
            .collect();
        let turning = matches!(act, SkillSetAct::TurnOff | SkillSetAct::TurnOn)
            .then_some(workspace.as_deref())
            .flatten();
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
                    member: (&one.member).into(),
                })
                .collect(),
            open_chats: if act.leaves_open_chats() {
                held.chats().open_using(&files, turning)
            } else {
                0
            },
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
        let members: Vec<set::Member> = members.iter().map(SkillSetMember::core).collect();
        let remote = remove_source.as_ref().map(SourceRemote::core);
        let mut gone = Vec::new();
        let said = heard_audited(&held, |cx| {
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
    under: Vec<DeletedRename>,
) -> Result<crate::skillchange::SkillChanged, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("bringing the skills back", move || {
        let set_of: Vec<(verbs::Deleted, Option<String>)> = ids
            .iter()
            .map(|id| {
                let one = serde_json::from_str(id)
                    .map_err(|_| "That deleted skill is not one this project lists.".to_owned())?;
                let new = under
                    .iter()
                    .find(|rename| rename.id == *id)
                    .map(|rename| rename.to.clone());
                Ok((one, new))
            })
            .collect::<Result<_, String>>()?;
        let core = source.as_ref().map(|one| (one.core(), one.kept.into()));
        heard_audited(&held, |cx| {
            set::restore_set_as(cx, &set_of, core.as_ref().map(|(one, which)| (one, *which)))
        })
    })
    .await
}

/// **One deleted skill brought back under a new name** (#5457): its id, as Deleted lists it, and
/// the name it comes back under, where its own belongs to purlis now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct DeletedRename {
    pub id: String,
    pub to: String,
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

/// **Pins `names` for `by`'s chats, or unpins them, as one change** (#5332). All or nothing.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_set_pin(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    by: SkillPinner,
    names: Vec<String>,
    on: bool,
) -> Result<SkillSetTurned, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("pinning the skills", move || {
        let by = (&by).into();
        let mut changed = Vec::new();
        let said = heard(held.root(), |cx| {
            changed = set::pin_set(cx, &by, &names, on)?;
            Ok(())
        })?;
        Ok(SkillSetTurned {
            notes: said.notes,
            changed,
        })
    })
    .await
}

/// One skill's lifecycle state in a set's change: what it is set to, or what an Undo sets back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SkillSetLifecycle {
    pub member: SkillSetMember,
    pub lifecycle: SkillLifecycle,
}

/// What a set's archive or unarchive did, and what an Undo sets back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct SkillSetLifecycled {
    pub notes: Vec<String>,
    /// Each it changed, with the state it had.
    pub was: Vec<SkillSetLifecycle>,
}

/// **Archives or unarchives the skills of `changes` as one change** (#5332). All or nothing.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_set_lifecycle(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    changes: Vec<SkillSetLifecycle>,
) -> Result<SkillSetLifecycled, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("changing the skills' state", move || {
        let changes: Vec<(set::Member, purlis_core::skills::Lifecycle)> = changes
            .into_iter()
            .map(|one| (one.member.core(), one.lifecycle.into()))
            .collect();
        let mut was = Vec::new();
        let said = heard(held.root(), |cx| {
            was = set::lifecycle_set(cx, &changes)?;
            Ok(())
        })?;
        Ok(SkillSetLifecycled {
            notes: said.notes,
            was: was
                .iter()
                .map(|(member, lifecycle)| SkillSetLifecycle {
                    member: member.into(),
                    lifecycle: lifecycle.clone().into(),
                })
                .collect(),
        })
    })
    .await
}

/// One skill's move in a set's: where it goes, and the grant it takes into the project's skills.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SkillSetMove {
    pub member: SkillSetMember,
    pub to: SkillLevel,
    pub grant: Option<SkillGrant>,
}

/// What a set's move did, and the moves its Undo makes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct SkillSetMoved {
    pub notes: Vec<String>,
    pub back: Vec<SkillSetMove>,
}

/// **Moves the skills of `moves` as one change** (#5332). All or nothing.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_set_move(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    moves: Vec<SkillSetMove>,
) -> Result<SkillSetMoved, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("moving the skills", move || {
        let moves: Vec<set::Moving> = moves
            .into_iter()
            .map(|one| set::Moving {
                member: one.member.core(),
                to: (&one.to).into(),
                grant: one.grant.map(Into::into),
            })
            .collect();
        let mut back = Vec::new();
        let said = heard(held.root(), |cx| {
            back = set::move_set(cx, &moves)?;
            Ok(())
        })?;
        Ok(SkillSetMoved {
            notes: said.notes,
            back: back
                .into_iter()
                .map(|one| SkillSetMove {
                    member: (&one.member).into(),
                    to: (&one.to).into(),
                    grant: one.grant.map(Into::into),
                })
                .collect(),
        })
    })
    .await
}

/// What a Delete now did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct SkillSetGone {
    /// Those whose delete was a commit, off the list: the project's history still holds them.
    pub off_the_list: u32,
    /// Those this machine kept, removed from it for good.
    pub removed: u32,
    /// Each that could not be taken off, with why.
    pub failed: Vec<String>,
}

/// **Takes every deleted skill `ids` names off Deleted now** (#5332): the one change there with no
/// undo. Refused whole where one is no longer listed.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_set_delete_now(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    ids: Vec<String>,
) -> Result<SkillSetGone, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("deleting the skills now", move || {
        let set_of: Vec<verbs::Deleted> = ids
            .iter()
            .map(|id| {
                serde_json::from_str(id)
                    .map_err(|_| "That deleted skill is not one this project lists.".to_owned())
            })
            .collect::<Result<_, String>>()?;
        let gone = set::delete_now(held.root(), &set_of)?;
        Ok(SkillSetGone {
            off_the_list: u32::try_from(gone.off_the_list).unwrap_or(u32::MAX),
            removed: u32::try_from(gone.removed).unwrap_or(u32::MAX),
            failed: gone.failed,
        })
    })
    .await
}

// --- Review updates (#5354) ----------------------------------------------------------------------

/// One set update's Undo, held: its number, the project it was made in, and what it takes back.
struct HeldUndo {
    id: u32,
    plane: crate::planes::PlaneId,
    undo: set_update::Undo,
}

/// The Undos held, and the number the last one was given.
#[derive(Default)]
struct Undos {
    last: u32,
    held: Vec<HeldUndo>,
}

/// **The Undos of the updates accepted as a set**, held in this process alone: what an Undo puts
/// back is the copies as they were, which the window never holds. The newest [`UNDOS_HELD`] are
/// kept; an older one's Undo, or one from before purlis last started, says it is gone.
#[derive(Default)]
pub(crate) struct SetUpdateUndos(std::sync::Mutex<Undos>);

/// How many set updates' Undos are held at once.
const UNDOS_HELD: usize = 8;

/// One update of a set, as the window accepted it: the skill, what its review was accepted as,
/// the grant it showed, and each kind of new capability the person ticked it with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SkillSetUpdating {
    pub level: SkillLevel,
    pub name: String,
    pub reviewed: crate::skillsources::Reviewed,
    pub grant: Option<SkillGrant>,
    pub allowed: Vec<crate::skillsources::SkillGained>,
}

/// What a set's update did, and the Undo that takes it back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct SkillSetUpdated {
    pub notes: Vec<String>,
    /// What [`skill_set_update_undo`] is handed.
    pub undo: u32,
}

/// **Accepts every update of `updates` as one change** (#5354): one commit, all or nothing, and
/// one where a skill gains a kind of capability it was not ticked with is refused whole.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_set_update(
    views: tauri::State<'_, crate::views::Views>,
    planes: tauri::State<'_, crate::planes::Planes>,
    undos: tauri::State<'_, SetUpdateUndos>,
    plane: crate::planes::PlaneId,
    updates: Vec<SkillSetUpdating>,
) -> Result<SkillSetUpdated, String> {
    let held = planes.held(&plane)?;
    let set: Vec<set_update::Updating> = updates
        .iter()
        .map(|one| {
            Ok(set_update::Updating {
                level: (&one.level).into(),
                name: one.name.clone(),
                accepted: purlis_core::skills::source::Accepted::try_from(&one.reviewed)?,
                grant: one.grant.clone().map(Into::into),
                allowed: one.allowed.iter().copied().map(Into::into).collect(),
                held: None,
            })
        })
        .collect::<Result<_, String>>()?;
    let executor = views.executor();
    let (said, undo) = crate::off_the_window("updating the skills", move || {
        let root = held.root();
        // A skill from an extension's source copies what its own review read, and only while the
        // extension is still approved and on here (#5760; ADR 0100, amended 2026-11-08).
        let mut set = set;
        for one in &mut set {
            one.held = crate::skillsources::extension::held_for_a_set(
                &executor, root, &one.level, &one.name,
            )?;
        }
        let mut undo = None;
        let said = heard_audited(&held, |cx| {
            undo = Some(set_update::update_set(cx, &set)?);
            Ok(())
        })?;
        // Each review an extension's update copied is used once: the same accept again needs a
        // new review.
        let copied: Vec<_> = set
            .iter()
            .filter(|one| one.held.is_some())
            .map(|one| (one.level.clone(), one.name.clone()))
            .collect();
        crate::skillsources::extension::release_set(root, &copied);
        // update_set answers an Undo whenever it is done, so this is never met.
        undo.map(|undo| (said, undo)).ok_or_else(|| {
            "The skills were updated, but their Undo was lost: update them back one at a time."
                .to_owned()
        })
    })
    .await?;
    let mut kept = undos.0.lock().map_err(|_| {
        "The skills were updated, but their Undo could not be kept: update them back one at a \
         time."
            .to_owned()
    })?;
    kept.last = kept.last.wrapping_add(1);
    let id = kept.last;
    kept.held.push(HeldUndo { id, plane, undo });
    let over = kept.held.len().saturating_sub(UNDOS_HELD);
    kept.held.drain(..over);
    Ok(SkillSetUpdated {
        notes: said.notes,
        undo: id,
    })
}

/// **Takes back the set's update `undo` names**: each copy as it was and the pins back, as one
/// commit. Refused whole where one changed since.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_set_update_undo(
    planes: tauri::State<'_, crate::planes::Planes>,
    undos: tauri::State<'_, SetUpdateUndos>,
    plane: crate::planes::PlaneId,
    undo: u32,
) -> Result<crate::skillchange::SkillChanged, String> {
    let held = planes.held(&plane)?;
    let gone = || {
        format!(
            "This update can't be undone now: only the last {UNDOS_HELD} updates can be \
             undone, and only until the app quits. Update the skills back one at a time."
        )
    };
    let taken = {
        let mut kept = undos.0.lock().map_err(|_| gone())?;
        let at = kept
            .held
            .iter()
            .position(|one| one.id == undo && one.plane == plane)
            .ok_or_else(gone)?;
        kept.held.remove(at)
    };
    let done = crate::off_the_window("undoing the update", move || {
        let mut refused = false;
        let said = heard_audited(&held, |cx| {
            set_update::undo_update_set(cx, &taken.undo).map_err(|why| {
                refused = matches!(why, set_update::NotUndone::Refused(_));
                why.to_string()
            })
        });
        Ok((said, refused, taken))
    });
    let (said, refused, taken) = done.await?;
    // Refused with nothing touched: its Undo stays offered. Taken back in part, it is not.
    if said.is_err()
        && refused
        && let Ok(mut kept) = undos.0.lock()
    {
        kept.held.push(taken);
    }
    said
}
