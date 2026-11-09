// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A change to several skills at once** (#5331): a set deleted, brought back or turned off or on
//! as **one commit, all or nothing**, and what its review says before anything is written. The set
//! is any skills of the project, named by level and name: a source's page asks with the ones
//! installed from it, and nothing here knows a source but for the words it is said in.
//!
//! A delete that cannot be a commit (a local skill, or a project that cannot commit now) keeps its
//! skill on this machine, as [`super::delete`] does, marked with the same set: a set across shared
//! and local levels comes back as one group. Whatever fails after the first skill moved puts every
//! skill back, so what the review showed either happened whole or did not happen.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{
    Ctx, DeleteSet, Deleted, Deleting, Level, Source, Unmoved, Whose, after_edit_dropped, beside,
    bring_back, deleted, find, git, head, keep_in_set, kept_emptied, pending_in, put_back, rel,
    renamed_away, renamed_back, restored_trailer, specs_of, taken_back, tracked, under_checked,
    unstage,
};
use crate::counts::counted;
use crate::settings::Which;
use crate::skills::LINKED_PLUGIN;
use crate::skills::source::{self, Remote, lock};
use purlis_skills::change;

mod acts;
pub mod gone;
pub mod proposed;

pub use acts::{Moving, lifecycle_set, move_set, pin_set};
pub use gone::{Gone, delete_now};
pub use proposed::{SetChange, Shown};

/// The trailer that says which set a commit deleted.
pub(super) const SET_TRAILER: &str = "Purlis-Skill-Set";
/// The trailer that says what that set was deleted from, in words.
pub(super) const FROM_TRAILER: &str = "Purlis-Skill-Set-From";

/// **One skill of a set**: its level and its name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Member {
    pub level: Level,
    pub name: String,
}

/// **What is asked of a set**, which is what its review reads for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Act {
    Delete,
    TurnOff,
    TurnOn,
    /// The Library's Select mode's (#5332), each with its own review.
    Pin,
    Unpin,
    Archive,
    Unarchive,
    /// To the level the review is asked with.
    Move,
    Export,
}

/// One skill as its review shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReviewedMember {
    pub member: Member,
    /// Whether it was edited here since it was installed.
    pub changed: bool,
    /// Why it cannot be part of this change: the review takes it out before anything is written.
    pub cannot: Option<String>,
}

/// **A skill that stays naming one that goes** (or, turning on, one that stays off).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Naming {
    pub namer: String,
    pub named: String,
    /// The copy named: what the review offers to keep out of a delete or a turn-off, or to take
    /// into a turn-on. Never the namer: the offer does not widen a delete.
    pub member: Member,
}

/// What a set's review says before anything is written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SetReview {
    pub skills: Vec<ReviewedMember>,
    pub names: Vec<Naming>,
}

/// **A copy installed from a source** that is still there.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Installed {
    pub member: Member,
    /// Whether it was edited here since it was installed.
    pub changed: bool,
    /// Whether it is on in the workspace asked of; `None` where that workspace cannot turn it.
    pub on_here: Option<bool>,
}

/// Whether `text` names the skill `name`: a reference purlis reads, or its whole name in quotes
/// or backticks (#4510). Only ever a warning's reason: a word in a sentence names nothing.
fn names_skill(text: &str, name: &str) -> bool {
    change::references(text, LINKED_PLUGIN, name)
        || ['"', '\'', '`']
            .iter()
            .any(|quote| text.contains(&format!("{quote}{name}{quote}")))
}

/// Every copy the lockfiles hold that was installed from `origin`, with what it is now.
pub fn installed_from(
    plane: &Path,
    origin: &source::Origin,
    workspace: Option<&str>,
) -> Vec<Installed> {
    let (locked, _) = lock::every(plane);
    let off = workspace.map(|ws| crate::skills::turned_off(plane, ws));
    locked
        .into_iter()
        .filter(|(_, one)| one.source == *origin)
        .map(|(_, one)| {
            let member = Member {
                level: one.level.0.clone(),
                name: one.name.clone(),
            };
            let on_here = workspace.zip(off.as_ref()).and_then(|(ws, off)| {
                reaches(plane, &member, ws).then(|| !off.contains(&member.name))
            });
            Installed {
                changed: source::update::edited_since_install(plane, &one),
                on_here,
                member,
            }
        })
        .collect()
}

/// Whether `member` is a project skill whose grant reaches `workspace`: what it takes to turn it.
fn reaches(plane: &Path, member: &Member, workspace: &str) -> bool {
    member.level.at == Whose::Project
        && crate::skills::project_folder(plane, &member.name).is_some_and(|folder| {
            crate::skills::grant_of(&folder)
                .unwrap_or(crate::skills::Grant::Root)
                .reaches(workspace)
        })
}

/// Why `member` cannot be part of `act`, or `None`. A move goes `to`.
fn cannot(
    plane: &Path,
    act: Act,
    member: &Member,
    workspace: Option<&str>,
    to: Option<&Level>,
) -> Option<String> {
    let Ok(folder) = find(plane, &member.level, &member.name) else {
        return Some("It is not there any more.".to_owned());
    };
    match act {
        Act::Delete => {
            let folder_rel = rel(plane, &folder);
            let toml_rel = rel(plane, &beside(&folder));
            let pending = tracked(plane, &folder_rel)
                && pending_in(plane, &[&folder_rel, &toml_rel]).map_or(true, |p| !p.is_empty());
            pending
                .then(|| "It holds changes that are not committed yet: save them first.".to_owned())
        }
        Act::TurnOff | Act::TurnOn => match workspace {
            Some(ws) if reaches(plane, member, ws) => None,
            Some(ws) => Some(format!("It does not reach {ws}, so {ws} cannot turn it.")),
            None => Some("Only a workspace turns a skill off or on.".to_owned()),
        },
        Act::Pin | Act::Unpin | Act::Archive | Act::Unarchive => None,
        Act::Move => {
            let Some(to) = to else {
                return Some("No level was named to move it to.".to_owned());
            };
            if member.level == *to {
                return Some(format!("It is a {to} skill already."));
            }
            if crate::skills::name_reserved(&member.name) {
                return Some(crate::skills::NAME_RESERVED.to_owned());
            }
            super::free_at(plane, to, &member.name, &super::folder_name(&folder))
                .err()
                .map(|why| format!("{}.", crate::skills::router::capitalised(&why)))
        }
        Act::Export => crate::skills::name_reserved(&member.name).then(|| {
            "A name starting with purlis- belongs to purlis, so it cannot be exported: rename it \
             first."
                .to_owned()
        }),
    }
}

/// **The review of `act` on `members`**: each one's marks, and each skill left naming one that
/// goes. Reads only, and asks nothing of a source.
pub fn review_set(
    plane: &Path,
    act: Act,
    members: &[Member],
    workspace: Option<&str>,
    to: Option<&Level>,
) -> SetReview {
    let (locked, _) = lock::every(plane);
    let skills: Vec<ReviewedMember> = members
        .iter()
        .map(|member| ReviewedMember {
            member: member.clone(),
            changed: locked.iter().any(|(_, one)| {
                one.is(&member.level, &member.name)
                    && source::update::edited_since_install(plane, one)
            }),
            cannot: cannot(plane, act, member, workspace, to),
        })
        .collect();
    let going: Vec<&Member> = skills
        .iter()
        .filter(|one| one.cannot.is_none())
        .map(|one| &one.member)
        .collect();
    let mut names = match act {
        Act::Delete | Act::TurnOff | Act::Archive => left_naming(plane, act, &going, workspace),
        Act::TurnOn => naming_what_stays_off(plane, &going, workspace),
        Act::Pin | Act::Unpin | Act::Unarchive | Act::Move | Act::Export => Vec::new(),
    };
    names.sort_by(|a, b| (&a.namer, &a.named).cmp(&(&b.namer, &b.named)));
    names.dedup_by(|a, b| a.namer == b.namer && a.named == b.named);
    SetReview { skills, names }
}

/// Each skill outside the set whose text names one that goes, where no other copy of that name
/// stays to answer.
fn left_naming(plane: &Path, act: Act, going: &[&Member], workspace: Option<&str>) -> Vec<Naming> {
    let in_set = |level: &Level, name: &str| {
        going
            .iter()
            .any(|one| one.level == *level && one.name == name)
    };
    let mut stays: Vec<(Level, String, String)> = Vec::new();
    for level in Level::every(plane) {
        let Some(dir) = level.dir(plane) else {
            continue;
        };
        for skill in crate::skills::read(&dir) {
            if !in_set(&level, &skill.name) {
                let text = purlis_skills::read_skill_md(plane, &skill.path).unwrap_or_default();
                stays.push((level.clone(), skill.name, text));
            }
        }
    }
    let off = workspace.map(|ws| crate::skills::turned_off(plane, ws));
    let mut names = Vec::new();
    for member in going {
        if stays.iter().any(|(_, name, _)| *name == member.name) {
            continue;
        }
        for (level, name, text) in &stays {
            let namer = Member {
                level: level.clone(),
                name: name.clone(),
            };
            // Only what is on in the workspace is left naming it there; a delete or an archive
            // leaves it naming nothing anywhere.
            let counts = act != Act::TurnOff
                || workspace
                    .zip(off.as_ref())
                    .is_some_and(|(ws, off)| reaches(plane, &namer, ws) && !off.contains(name));
            if counts && names_skill(text, &member.name) {
                names.push(Naming {
                    namer: name.clone(),
                    named: member.name.clone(),
                    member: (*member).clone(),
                });
            }
        }
    }
    names
}

/// Each skill turned on whose text names one that stays off in `workspace`.
fn naming_what_stays_off(plane: &Path, going: &[&Member], workspace: Option<&str>) -> Vec<Naming> {
    let Some(ws) = workspace else {
        return Vec::new();
    };
    let off = crate::skills::turned_off(plane, ws);
    let mut names = Vec::new();
    for member in going {
        let Ok(folder) = find(plane, &member.level, &member.name) else {
            continue;
        };
        let text = purlis_skills::read_skill_md(plane, &folder.join(purlis_skills::FILE))
            .unwrap_or_default();
        for one in &off {
            if !going.iter().any(|other| other.name == *one) && names_skill(&text, one) {
                names.push(Naming {
                    namer: member.name.clone(),
                    named: one.clone(),
                    member: Member {
                        level: Level::project(),
                        name: one.clone(),
                    },
                });
            }
        }
    }
    names
}

/// A label as a line can hold it: one line of plain text.
fn one_line(text: &str) -> String {
    text.chars()
        .map(|one| if one.is_control() { ' ' } else { one })
        .collect::<String>()
        .trim()
        .to_owned()
}

/// A skill's place in a delete under way.
struct Going<'a> {
    member: &'a Member,
    folder: PathBuf,
    folder_rel: String,
    toml_rel: String,
    tracked: bool,
}

/// The file `member`'s copy reads from, as a chat that started with it holds it.
pub fn copy_file(plane: &Path, member: &Member) -> Option<PathBuf> {
    let folder = member.level.named(plane, &member.name)?;
    Some(folder.join(purlis_skills::FILE))
}

/// Those kept on this machine by a delete under way, put back: what could not be, in words.
fn kept_ones_back(plane: &Path, kept: &[(&Going<'_>, String)]) -> Vec<String> {
    let state = crate::plane::state_dir(plane);
    let mut left = Vec::new();
    for (one, name) in kept.iter().rev() {
        let from = state
            .join(super::DELETED_DIR)
            .join(name)
            .join(super::folder_name(&one.folder));
        match purlis_skills::move_skill(&state, &from, plane, &one.folder, one.member.level.depth())
        {
            Ok(()) => kept_emptied(plane, name),
            Err(Unmoved::Nothing(why) | Unmoved::Stranded(why)) => {
                left.push(format!("{} ({why})", one.member.name));
            }
        }
    }
    left
}

/// The sentence a delete put back says, with what could not be.
fn nothing_deleted(why: &str, left: &[String]) -> String {
    if left.is_empty() {
        format!("{why}, so no skill was deleted")
    } else {
        format!(
            "{why}, and {} could not be put back: Deleted lists any skill among them",
            left.join(", ")
        )
    }
}

/// **Deletes every skill of `members` as one change**: one commit of those git tracks, those it
/// does not kept on this machine, all listed as one group said to be `from` where it is given, and
/// each can come back with the group or alone. `remove_source` is taken out in the same commit.
/// All or nothing. Answers what [`deleted`] now lists of them.
pub fn delete_set(
    cx: &mut Ctx<'_>,
    members: &[Member],
    from: Option<&str>,
    remove_source: Option<&Remote>,
) -> Result<Vec<Deleted>, String> {
    let plane = cx.plane;
    crate::settings::writable(plane)?;
    if members.is_empty() {
        return Err("no skill was named to delete".to_owned());
    }
    // Every one is checked before the first is touched.
    let mut planned = Vec::new();
    for member in members {
        let name = &member.name;
        let folder =
            find(plane, &member.level, name).map_err(|why| format!("{why}, so nothing changed"))?;
        let folder_rel = rel(plane, &folder);
        let toml_rel = rel(plane, &beside(&folder));
        let is_tracked = tracked(plane, &folder_rel);
        if is_tracked && !pending_in(plane, &[&folder_rel, &toml_rel])?.is_empty() {
            return Err(format!(
                "{name} holds changes that are not committed yet, so nothing was deleted: save \
                 them first"
            ));
        }
        planned.push(Going {
            member,
            folder,
            folder_rel,
            toml_rel,
            tracked: is_tracked,
        });
    }
    // An org's project audits the removal of each skill installed from a source (SK-11), once
    // every one is checked and before the first is touched.
    for one in &planned {
        if let Some(audited) = source::policy::of_installed(
            plane,
            source::policy::Act::Remove,
            &one.member.level,
            &one.member.name,
        )? {
            cx.audited(&audited)
                .map_err(|why| format!("{why}, so nothing was deleted"))?;
        }
    }
    let source_file = match remove_source {
        Some(remote) => source::listed::shared_file_of(plane, remote)?,
        None => None,
    };
    let paths: Vec<String> = planned
        .iter()
        .filter(|one| one.tracked)
        .flat_map(|one| [one.folder_rel.clone(), one.toml_rel.clone()])
        .chain(source_file)
        .collect();
    let commits_now = crate::planegit::paths_unsaved(plane, &paths).is_none();
    let stamp = super::ms_since_epoch(std::time::SystemTime::now());
    let set = DeleteSet {
        id: stamp.to_string(),
        from: from.map(one_line).filter(|text| !text.is_empty()),
    };
    // The source goes first, while nothing else has moved: a refusal here changes nothing, and
    // every failure after it puts it back with the skills.
    let taken_out = match remove_source {
        Some(remote) => source::listed::take_out(plane, remote)?,
        None => None,
    };
    let put_all_back = |kept: &[(&Going<'_>, String)]| -> Vec<String> {
        let mut left = kept_ones_back(plane, kept);
        if let Some((source, which)) = &taken_out
            && let Err(why) = source::listed::put_in(plane, source, *which)
        {
            left.push(format!("the source {source} ({why})"));
        }
        left
    };
    // Those no commit can keep are moved first: a move is what can be taken back.
    let mut kept: Vec<(&Going<'_>, String)> = Vec::new();
    for (at, one) in planned.iter().enumerate() {
        if one.tracked && commits_now {
            continue;
        }
        let order = u128::try_from(at).unwrap_or(0);
        match keep_in_set(plane, &one.member.level, &one.folder, stamp + order, &set) {
            Ok(name) => kept.push((one, name)),
            Err(Unmoved::Nothing(why) | Unmoved::Stranded(why)) => {
                let left = put_all_back(&kept);
                return Err(nothing_deleted(
                    &format!("{} could not be deleted ({why})", one.member.name),
                    &left,
                ));
            }
        }
    }
    let by_git: Vec<&Going<'_>> = planned
        .iter()
        .filter(|one| one.tracked && commits_now)
        .collect();
    let mut notes = Vec::new();
    for one in &by_git {
        match Deleting::begin(plane, &one.member.level, &one.folder_rel) {
            Ok(note) => notes.push(note),
            Err(why) => {
                notes.into_iter().for_each(Deleting::end);
                let left = put_all_back(&kept);
                return Err(nothing_deleted(
                    &format!("{} could not be deleted ({why})", one.member.name),
                    &left,
                ));
            }
        }
    }
    let specs: Vec<String> = by_git
        .iter()
        .flat_map(|one| specs_of(plane, &one.folder_rel, &one.toml_rel, "HEAD"))
        .collect();
    if !specs.is_empty() {
        let mut args = vec!["rm", "-r", "-q", "--"];
        args.extend(specs.iter().map(String::as_str));
        if let Err(why) = git(plane, &args) {
            unstage(plane, &specs);
            let back = put_back(plane, &specs);
            notes.into_iter().for_each(Deleting::end);
            let mut left = put_all_back(&kept);
            if !back {
                left.push("those git holds".to_owned());
            }
            return Err(nothing_deleted(&format!("git refused ({why})"), &left));
        }
        unstage(plane, &specs);
    }
    let count = members.len();
    let (subject, trailers) = match &set.from {
        Some(label) => (
            format!("skill rm {count} from {label}"),
            format!("{SET_TRAILER}: {}\n{FROM_TRAILER}: {label}", set.id),
        ),
        None => (
            format!("skill rm {count}"),
            format!("{SET_TRAILER}: {}", set.id),
        ),
    };
    let before = head(plane);
    let committing = !paths.is_empty();
    if committing {
        cx.commit(&paths, &format!("{subject}\n\n{trailers}"));
    }
    if commits_now && committing && head(plane) == before {
        unstage(plane, &specs);
        let back = put_back(plane, &specs);
        let mut left = put_all_back(&kept);
        if back {
            notes.into_iter().for_each(Deleting::end);
        } else {
            // Neither committed nor put back: the next start tries again ([`super::interrupted`]).
            notes.into_iter().for_each(Deleting::left);
            left.push("those git holds".to_owned());
        }
        return Err(nothing_deleted("its commit could not be made", &left));
    }
    notes.into_iter().for_each(Deleting::end);
    for one in &planned {
        after_edit_dropped(cx, &one.member.level, &one.member.name);
    }
    cx.done(format!(
        "{} deleted; Bring all back returns them",
        counted(count, "skill", "skills")
    ));
    Ok(deleted(plane)
        .into_iter()
        .filter(|one| {
            members
                .iter()
                .any(|member| member.level == one.level && member.name == one.name)
        })
        .collect())
}

/// **Brings every skill of `set` back as one change**, each under its own name: see
/// [`restore_set_as`].
pub fn restore_set(
    cx: &mut Ctx<'_>,
    set: &[Deleted],
    source: Option<(&source::Source, Which)>,
) -> Result<(), String> {
    let named: Vec<(Deleted, Option<String>)> = set.iter().map(|one| (one.clone(), None)).collect();
    restore_set_as(cx, &named, source)
}

/// **Brings every skill of `set` back as one change**, one commit of those git holds: each under
/// its own name, or under the new name beside it where its own belongs to purlis now (#5457),
/// checked as [`super::restore`]'s is and against the rest of the set. Refused whole where one is
/// no longer listed, or comes back under a name it cannot take. `source`, what a delete took out
/// with them, goes back in the same commit.
pub fn restore_set_as(
    cx: &mut Ctx<'_>,
    set: &[(Deleted, Option<String>)],
    source: Option<(&source::Source, Which)>,
) -> Result<(), String> {
    let plane = cx.plane;
    crate::settings::writable(plane)?;
    if set.is_empty() {
        return Err("no skill was named to bring back".to_owned());
    }
    let listed = deleted(plane);
    let landing = |one: &Deleted, under: &Option<String>| -> (Level, String) {
        (
            one.level.clone(),
            under.clone().unwrap_or_else(|| one.name.clone()),
        )
    };
    for (at, (one, under)) in set.iter().enumerate() {
        let name = &one.name;
        if !listed.contains(one) {
            return Err(format!(
                "{name} cannot be brought back any more, or a skill of its name is there again, \
                 so none was brought back"
            ));
        }
        match under.as_deref() {
            None if cx.reserved(name) => {
                return Err(format!(
                    "{name} belongs to purlis now and comes back only under a new name, so none \
                     was brought back"
                ));
            }
            None => {}
            Some(_) if !cx.reserved(name) => {
                return Err(format!(
                    "{name} is brought back under its own name: bring it back, then rename it, \
                     so what names it follows"
                ));
            }
            Some(new) => {
                under_checked(cx, one, new)
                    .map_err(|why| format!("{why}, so none was brought back"))?;
            }
        }
        let lands = landing(one, under);
        if set[..at]
            .iter()
            .any(|(other, theirs)| landing(other, theirs) == lands)
        {
            return Err(format!(
                "{} is the name of another skill coming back to the {} skills: two skills cannot \
                 have one name, so none was brought back",
                lands.1, lands.0
            ));
        }
    }
    // A skill installed from a source comes back only where the org's controls allow it, and its
    // coming back is audited as an install (SK-11), once every one is checked.
    for (one, _) in set {
        if let Some(audited) = source::policy::hold_restored(plane, &one.level, &one.name)
            .map_err(|why| format!("{why}, so none was brought back"))?
        {
            cx.audited(&audited)
                .map_err(|why| format!("{why}, so none was brought back"))?;
        }
    }
    // What came back, where it landed, and whether the file beside it came from a commit.
    type Back<'a> = (&'a Deleted, PathBuf, PathBuf, Option<&'a str>, bool);
    let undo = |back: &[Back<'_>]| -> Vec<String> {
        let mut left = Vec::new();
        for (one, folder, landed, under, toml_there) in back.iter().rev() {
            let away = match under {
                Some(new) => renamed_away(plane, landed, folder, new, &one.name),
                None => Ok(()),
            };
            if let Err(why) = away.and_then(|()| taken_back(plane, one, folder, *toml_there)) {
                left.push(format!("{} ({why})", one.name));
            }
        }
        left
    };
    let say_none = |why: String, left: &[String]| {
        if left.is_empty() {
            format!("{why}, so none was brought back")
        } else {
            format!("{why}, and {} could not be taken back", left.join(", "))
        }
    };
    let mut back: Vec<Back<'_>> = Vec::new();
    for (one, under) in set {
        let brought = super::level_is_there(plane, &one.level).and_then(|dir| {
            let folder = dir.join(one.folder_name());
            bring_back(plane, one, &folder).map(|toml_there| (folder, toml_there))
        });
        let (folder, toml_there) = match brought {
            Ok(brought) => brought,
            Err(why) => {
                let left = undo(&back);
                return Err(say_none(why, &left));
            }
        };
        let Some(new) = under.as_deref() else {
            back.push((one, folder.clone(), folder, None, toml_there));
            continue;
        };
        match renamed_back(plane, &folder, &one.name, new) {
            Ok(landed) => back.push((one, folder, landed, Some(new), toml_there)),
            Err(unmoved) => {
                let (mut left, why) = match unmoved {
                    Unmoved::Nothing(why) => (
                        taken_back(plane, one, &folder, toml_there)
                            .err()
                            .map(|left| format!("{} ({left})", one.name))
                            .into_iter()
                            .collect::<Vec<_>>(),
                        why,
                    ),
                    Unmoved::Stranded(what) => (vec![format!("{} ({what})", one.name)], what),
                };
                left.extend(undo(&back));
                return Err(say_none(
                    format!("{} could not be named {new} ({why})", one.name),
                    &left,
                ));
            }
        }
    }
    let mut paths: Vec<String> = back
        .iter()
        .filter(|(one, ..)| !one.level.local)
        .flat_map(|(_, folder, landed, ..)| {
            let mut paths = vec![rel(plane, folder), rel(plane, &beside(folder))];
            if landed != folder {
                paths.extend([rel(plane, landed), rel(plane, &beside(landed))]);
            }
            paths
        })
        .collect();
    if let Some((source, which)) = source {
        match source::listed::put_in(plane, source, which) {
            Ok(file) => paths.extend(file),
            Err(why) => {
                let left = undo(&back);
                return Err(say_none(why, &left));
            }
        }
    }
    let commits_now = !paths.is_empty() && crate::planegit::paths_unsaved(plane, &paths).is_none();
    let before = head(plane);
    let count = set.len();
    if !paths.is_empty() {
        let trailers: Vec<String> = back
            .iter()
            .filter(|(_, _, _, under, _)| under.is_some())
            .filter_map(|(one, folder, ..)| restored_trailer(plane, one, folder))
            .collect();
        let message = if trailers.is_empty() {
            format!("skill restore {count}")
        } else {
            format!("skill restore {count}\n\n{}", trailers.join("\n"))
        };
        cx.commit(&paths, &message);
    }
    if commits_now && head(plane) == before {
        let mut left = undo(&back);
        if let Some((source, _)) = source
            && let Err(why) = source::listed::take_out(plane, &source.remote)
        {
            left.push(format!("the source {source} ({why})"));
        }
        return Err(say_none("its commit could not be made".to_owned(), &left));
    }
    for (one, _, _, under, _) in &back {
        if let Source::Here(kept) = &one.kept.0 {
            kept_emptied(plane, kept);
        } else if under.is_none()
            && let Err(why) = crate::skills::grades::earlier::refound(plane, &one.level, &one.name)
        {
            // One under a new name is another skill now, as [`super::restore`] leaves it.
            cx.warn(why);
        }
    }
    cx.done(format!("{} back", counted(count, "skill", "skills")));
    Ok(())
}

/// **Turns every skill of `names` off (`on` false) or on in `workspace`** as one commit, refused
/// whole where one cannot turn there. Answers those it changed, so an Undo leaves one that already
/// was so as it was.
pub fn turn_set(
    cx: &mut Ctx<'_>,
    workspace: &str,
    names: &[String],
    on: bool,
) -> Result<Vec<String>, String> {
    let plane = cx.plane;
    crate::settings::writable(plane)?;
    let held = crate::workspaces::Plane::open(plane);
    if !held
        .workspace(workspace)
        .is_ok_and(|one| one.dir().is_dir())
    {
        return Err(format!("this project has no workspace {workspace:?}"));
    }
    let state = if on { "on" } else { "off" };
    for name in names {
        let member = Member {
            level: Level::project(),
            name: name.clone(),
        };
        if !reaches(plane, &member, workspace) {
            return Err(format!(
                "{name} does not reach {workspace}, so no skill was turned {state}"
            ));
        }
    }
    let was = crate::skills::turned_off(plane, workspace);
    let changed: Vec<String> = names
        .iter()
        .filter(|name| was.contains(name) == on)
        .cloned()
        .collect();
    if changed.is_empty() {
        return Ok(changed);
    }
    let mut off = was.clone();
    if on {
        off.retain(|one| !changed.contains(one));
    } else {
        off.extend(changed.iter().cloned());
    }
    crate::skills::set_turned_off(plane, workspace, off)?;
    let file = crate::settings::workspace::named(workspace);
    let commits_now = crate::planegit::paths_unsaved(plane, std::slice::from_ref(&file)).is_none();
    let before = head(plane);
    let count = changed.len();
    cx.commit(
        std::slice::from_ref(&file),
        &format!("skill {state} {count} in {workspace}"),
    );
    if commits_now && head(plane) == before {
        crate::skills::set_turned_off(plane, workspace, was)?;
        return Err(format!(
            "its commit could not be made, so no skill was turned {state}"
        ));
    }
    cx.done(format!(
        "{} turned {state} in {workspace}",
        counted(count, "skill", "skills")
    ));
    Ok(changed)
}

#[cfg(test)]
mod tests;
