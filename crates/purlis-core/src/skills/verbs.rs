// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Making and changing skills** (SK-7, #2770; spec #2762 decisions 13, 14 and 37): create from
//! a template, edit, rename, move between levels, delete and restore, and the grant and turn-off
//! the studio changes, as the Skill studio and `purlis skill new | edit | mv | rm | restore |
//! grant | share` do them. One copy of each, which the window and the command line both call,
//! each with a [`Ctx`]: the plane, who asked, and where what it says goes.
//!
//! **Each change is one commit** through the plane's one save, staged by exactly the paths it
//! touched ([`crate::planegit::save_paths`]), so a skill change never carries anyone's other
//! work. A project that has not been told how it is saved yet keeps the change on disk for the
//! next save, as a workspace rename does.
//!
//! **No link is followed.** A skill's `SKILL.md` is written through
//! [`crate::proposed::write_pinned_making`], and a folder is moved by `renameat` between folders
//! opened one component at a time with `O_NOFOLLOW`, never over anything already there. The
//! deleted skills this machine keeps are made, read and removed through the same handles.
//!
//! **A delete never loses a skill.** A committed skill is removed with `git rm` only where its
//! commit can land now ([`crate::planegit::paths_unsaved`]), and is put back where the commit was
//! not made; Restore brings it back from that commit. Any other skill is moved into this
//! machine's [`DELETED_DIR`], where Restore finds it.
//!
//! **What it says is the window's words too.** A refusal or a warning names the skill and what to
//! do, never a file's path or a command; a `Done` line, which only the command line prints, may.

use std::path::{Path, PathBuf};

use purlis_skills::change::{self, SkillTemplate};

use super::{Grant, LINKED_PLUGIN, Origin, beside, grant_of, level_dir, named, set_grant};
use crate::planegit::literally;
use crate::repocmd::{Say, Sink};

/// Where a level is: the project's root, or one workspace.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum At {
    Project,
    Workspace(String),
}

/// A plane level a skill lives at (ADR 0099 §1): the project's or one workspace's, shared or
/// local.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Level {
    pub at: At,
    /// Its `skills.local/`, kept on this machine.
    pub local: bool,
}

impl Level {
    /// The project's shared skills: `skills/` at its root.
    pub fn project() -> Level {
        Level {
            at: At::Project,
            local: false,
        }
    }

    /// `workspace`'s shared skills.
    pub fn workspace(workspace: &str) -> Level {
        Level {
            at: At::Workspace(workspace.to_owned()),
            local: false,
        }
    }

    /// The same level's local skills, where `local`.
    pub fn local(self, local: bool) -> Level {
        Level { local, ..self }
    }

    /// The workspace it is in, or `None` for the project's.
    pub fn workspace_name(&self) -> Option<&str> {
        match &self.at {
            At::Project => None,
            At::Workspace(ws) => Some(ws),
        }
    }

    /// The level a skill in it comes from.
    pub fn origin(&self) -> Origin {
        match self.at {
            At::Project => Origin::Project,
            At::Workspace(_) => Origin::Workspace,
        }
    }

    /// Its folder in the plane at `plane`, or `None` for a workspace name that names no folder.
    pub fn dir(&self, plane: &Path) -> Option<PathBuf> {
        level_dir(plane, self.workspace_name(), self.local)
    }

    /// Its folder relative to the plane, `/` between the parts, by the one rule
    /// ([`purlis_skills::level_rel`]): `skills`, `workspaces/ops/skills.local`.
    pub(in crate::skills) fn rel(&self) -> Option<String> {
        purlis_skills::level_rel(self.workspace_name(), self.local)
    }

    /// The level whose folder `rel` is, read by the same rule.
    pub(in crate::skills) fn of_rel(rel: &str) -> Option<Level> {
        let (workspace, local) = purlis_skills::level_of_rel(rel)?;
        Some(
            match workspace {
                None => Level::project(),
                Some(ws) => Level::workspace(&ws),
            }
            .local(local),
        )
    }

    /// How many folders its [`Self::rel`] has: the first one a new skill may make is the last.
    pub(in crate::skills) fn depth(&self) -> usize {
        self.rel().map_or(usize::MAX, |rel| rel.split('/').count())
    }

    /// **Every level of the plane at `plane`**: the project's, then each workspace's, shared
    /// then local. What the studio's level pickers offer, and where a rename looks for names.
    pub fn every(plane: &Path) -> Vec<Level> {
        let mut levels = vec![Level::project(), Level::project().local(true)];
        for ws in workspaces(plane) {
            levels.push(Level::workspace(&ws));
            levels.push(Level::workspace(&ws).local(true));
        }
        levels
    }
}

/// `project`, `local project`, `workspace ops`, `local workspace ops`: as a sentence names it.
impl std::fmt::Display for Level {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let shown = super::level_shown(self.origin(), self.local);
        match &self.at {
            At::Project => f.write_str(&shown),
            At::Workspace(ws) => write!(f, "{shown} {ws}"),
        }
    }
}

/// Who asked for a change: the save's journal and the forge are told (ADR 0070 §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum By {
    /// The Skill studio, in the window.
    Window,
    /// `purlis skill`.
    Command,
}

/// **What every change is made with**: the plane, the built-in skills (whose names are never
/// taken), who asked, and where what it says goes.
pub struct Ctx<'a> {
    pub plane: &'a Path,
    pub built_in: Option<&'a Path>,
    pub by: By,
    pub say: Sink<'a>,
}

/// The plane's workspaces, by name.
fn workspaces(plane: &Path) -> Vec<String> {
    crate::workspaces::Plane::open(plane)
        .workspaces()
        .unwrap_or_default()
}

/// `path` relative to `plane`, `/` between its parts.
pub(in crate::skills) fn rel(plane: &Path, path: &Path) -> String {
    path.strip_prefix(plane)
        .unwrap_or(path)
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// A folder's own name.
fn folder_name(folder: &Path) -> String {
    folder
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

impl Ctx<'_> {
    /// **Commits exactly `paths`** as one change, `message` its subject, through the plane's
    /// save. A project that cannot take it now (no save mode yet, saving off, something else
    /// staged) keeps it for the next save, and that is said.
    pub(in crate::skills) fn commit(&mut self, paths: &[String], message: &str) {
        let mut unique: Vec<String> = Vec::new();
        for path in paths {
            if !path.is_empty() && !unique.contains(path) {
                unique.push(path.clone());
            }
        }
        if let Some(why) = crate::planegit::paths_unsaved(self.plane, &unique) {
            (self.say)(Say::Info(why));
            return;
        }
        let message = format!("{} {message}", crate::names::BINARY.writes_for(self.plane));
        // The save's own lines are the command line's to print as they are. The window is told
        // only how it ended, in its own words below; what the save said is kept in the log.
        let by = self.by;
        let say = &mut *self.say;
        let mut relay = |line: Say| match by {
            By::Command => say(line),
            By::Window => {
                tracing::info!(target: "purlis::skills", "the skill change's save: {line}")
            }
        };
        let code = crate::planegit::save_paths(
            &crate::planegit::Request {
                root: self.plane,
                message: Some(&message),
                sign: false,
                no_push: false,
                cwd: self.plane,
                provenance: None,
            },
            crate::planegit::Trigger::Skill(self.by),
            &unique,
            &mut relay,
        );
        if code != 0 {
            (self.say)(Say::Warn(
                "The change is made, and it could not be saved yet: the next save takes it, and \
                 Saving says why."
                    .to_owned(),
            ));
        }
    }

    /// Refused where a built-in skill has `name`: a built-in skill's name is never taken (ADR
    /// 0099 §3), so a skill of that name would never reach a chat.
    pub(in crate::skills) fn not_built_in(&self, name: &str) -> Result<(), String> {
        let taken = self
            .built_in
            .is_some_and(|dir| super::read(dir).iter().any(|skill| skill.name == name));
        if taken {
            return Err(format!(
                "a built-in skill is named {name}, and a built-in skill's name is never taken: \
                 pick another name"
            ));
        }
        Ok(())
    }

    /// Says `line` as a warning.
    pub(in crate::skills) fn warn(&mut self, line: String) {
        (self.say)(Say::Warn(line));
    }

    /// Says `line` as done.
    pub(in crate::skills) fn done(&mut self, line: String) {
        (self.say)(Say::Done(line));
    }
}

/// Refused where `level` is a workspace's and the plane has no such workspace.
pub(in crate::skills) fn level_is_there(plane: &Path, level: &Level) -> Result<PathBuf, String> {
    if let Some(ws) = level.workspace_name() {
        let held = crate::workspaces::Plane::open(plane);
        if !held.workspace(ws).is_ok_and(|one| one.dir().is_dir()) {
            return Err(format!(
                "this project has no workspace {ws:?}, so nothing was changed"
            ));
        }
    }
    level
        .dir(plane)
        .ok_or_else(|| format!("{level} is no level of this project"))
}

/// The folder of the skill named `name` at `level`, or why there is none.
fn find(plane: &Path, level: &Level, name: &str) -> Result<PathBuf, String> {
    level
        .dir(plane)
        .and_then(|dir| named(&dir, name))
        .ok_or_else(|| format!("no {level} skill is named {name:?}"))
}

/// Refused where `level` already holds a skill named `name` or a folder named `folder`.
pub(in crate::skills) fn free_at(
    plane: &Path,
    level: &Level,
    name: &str,
    folder: &str,
) -> Result<PathBuf, String> {
    let dir = level_is_there(plane, level)?;
    if named(&dir, name).is_some() {
        return Err(format!(
            "the {level} skills already have one named {name}: pick another name, or change that \
             one"
        ));
    }
    if dir.join(folder).symlink_metadata().is_ok()
        || beside(&dir.join(folder)).symlink_metadata().is_ok()
    {
        return Err(format!(
            "the {level} skills already hold a folder named {folder}: pick another name"
        ));
    }
    Ok(dir)
}

/// What a write of `what` (`deploy's SKILL.md`) said, as a sentence.
fn not_written(what: &str, why: crate::proposed::NotMade) -> String {
    match why {
        crate::proposed::NotMade::Stale => {
            format!("{what} changed since it was opened, so nothing was written: open it again")
        }
        crate::proposed::NotMade::Unprepared => format!(
            "{what} could not be written: it, or a folder on the way, is not a plain file or \
             folder"
        ),
    }
}

/// The text of the file at `path` in the plane, read without following a link, or why not, as
/// the end of a sentence about `what`.
fn text_at(plane: &Path, path: &Path, what: &str) -> Result<String, String> {
    let rel_path = path.strip_prefix(plane).unwrap_or(path);
    let bytes = crate::proposed::read_pinned(plane, rel_path)
        .map_err(|_| format!("{what} is not a plain file, so purlis does not read it"))?
        .ok_or_else(|| format!("{what} is not there"))?;
    String::from_utf8(bytes).map_err(|_| format!("{what} is not text"))
}

/// `SKILL.md`'s text in `folder`, the skill `name`'s.
fn text_in(plane: &Path, folder: &Path, name: &str) -> Result<String, String> {
    text_at(
        plane,
        &folder.join("SKILL.md"),
        &format!("{name}'s instructions"),
    )
}

/// Writes `now` as the file at `rel_path`, `what` in a sentence, only while it is still `was`
/// (`None`: not there yet), making the folders from the `make_from`th on that are not there.
fn write(
    plane: &Path,
    rel_path: &str,
    what: &str,
    was: Option<&str>,
    now: &str,
    make_from: usize,
) -> Result<(), String> {
    let base = was.and_then(|text| crate::proposed::base_of(Some(text.as_bytes())));
    crate::proposed::write_pinned_making(
        plane,
        Path::new(rel_path),
        base.as_deref(),
        now,
        &|_, index| index + 1 >= make_from,
    )
    .map_err(|why| not_written(what, why))
}

/// **Creates a skill** named `name`, described by `description`, from the template `template`
/// that `templates` offers, at `level` (#2770): its folder and `SKILL.md`, committed. A project
/// skill starts granted to the project's root alone, the default. Refused for a name the open
/// format does not allow, one a built-in skill has, or one the level already holds, and for a
/// template whose text is not a whole skill that keeps its name.
///
/// `templates` is purlis's own ([`purlis_skills::change::BuiltInTemplates`]) or the studio's,
/// which adds each extension's (`crate::skills::studio::Templates`): one interface, the
/// SkillTemplate capability's (ADR 0101).
pub fn create(
    cx: &mut Ctx<'_>,
    level: &Level,
    name: &str,
    description: &str,
    templates: &dyn SkillTemplate,
    template: &str,
) -> Result<PathBuf, String> {
    let plane = cx.plane;
    crate::settings::writable(plane)?;
    if let Some(why) = change::name_problem(name) {
        return Err(format!("{name:?} cannot be a skill's name: {why}"));
    }
    if let Some(why) = change::description_problem(description) {
        return Err(format!("that description cannot be used: {why}"));
    }
    cx.not_built_in(name)?;
    let text = templates.text(template, name, description)?;
    if let Some(problem) = change::check(&text, name).first() {
        return Err(format!(
            "the template {template:?} does not make a whole skill: {problem}"
        ));
    }
    let dir = free_at(plane, level, name, name)?;
    let folder = dir.join(name);
    write(
        plane,
        &format!("{}/SKILL.md", rel(plane, &folder)),
        &format!("{name}'s instructions"),
        None,
        &text,
        level.depth(),
    )?;
    cx.done(format!(
        "{name} is made, a {level} skill in {}",
        rel(plane, &folder)
    ));
    cx.commit(&[rel(plane, &folder)], &format!("skill new {name}"));
    Ok(folder)
}

/// The text of the skill named `name` at `level`'s `SKILL.md`, which the light editor opens, and
/// its folder.
pub fn text_of(plane: &Path, level: &Level, name: &str) -> Result<(PathBuf, String), String> {
    let folder = find(plane, level, name)?;
    let text = text_in(plane, &folder, name)?;
    Ok((folder, text))
}

/// The paths under `folder` (plane-relative) with changes git has not committed, by their path.
fn pending_in(plane: &Path, paths: &[&str]) -> Result<Vec<String>, String> {
    let specs: Vec<String> = paths.iter().map(|path| literally(path)).collect();
    let mut args = vec!["status", "--porcelain", "-z", "--untracked-files=all", "--"];
    args.extend(specs.iter().map(String::as_str));
    let out = git(plane, &args)?;
    Ok(out
        .split('\0')
        .filter(|one| one.len() > 3)
        .map(|one| one[3..].to_owned())
        .collect())
}

/// **Saves an edit** of the skill named `name` at `level` (#2770): `now` as its `SKILL.md`, only
/// while the file is still `was`, the text the editor opened, and only while the live lint finds
/// nothing ([`change::check`]). Commits its `SKILL.md` alone: another file of the skill with
/// changes not committed is said, and left for the next save.
pub fn edit(
    cx: &mut Ctx<'_>,
    level: &Level,
    name: &str,
    was: &str,
    now: &str,
) -> Result<PathBuf, String> {
    let plane = cx.plane;
    crate::settings::writable(plane)?;
    let problems = change::check(now, name);
    if !problems.is_empty() {
        let lines: Vec<String> = problems.iter().map(ToString::to_string).collect();
        return Err(format!("nothing was saved: {}", lines.join("; ")));
    }
    let folder = find(plane, level, name)?;
    if was == now {
        (cx.say)(Say::Info(format!(
            "{name} is unchanged, so nothing was saved."
        )));
        return Ok(folder);
    }
    let file = format!("{}/SKILL.md", rel(plane, &folder));
    write(
        plane,
        &file,
        &format!("{name}'s instructions"),
        Some(was),
        now,
        usize::MAX,
    )?;
    cx.done(format!("{name} is saved"));
    let others = pending_in(plane, &[&rel(plane, &folder)])
        .unwrap_or_default()
        .into_iter()
        .filter(|one| *one != file)
        .count();
    if others > 0 {
        cx.warn(format!(
            "{} of {name} besides its instructions {} changes this save does not take; the next save \
             does",
            crate::counts::counted(others, "other file", "other files"),
            if others == 1 { "has" } else { "have" }
        ));
    }
    cx.commit(&[file], &format!("skill edit {name}"));
    Ok(folder)
}

/// **Renames the skill named `old` at `level` to `new`** (#2770, decision 14): its `SKILL.md`'s
/// `name`, its folder and the file beside it, and every place that names it: each persona's
/// `skills:` list and each reference in a persona's or a skill's text (`skills:<name>`), and, for
/// a project skill, each workspace's turn-off list. One commit. Where another skill keeps the
/// name `old`, the places that name it are left as they are, since they may mean that one, and
/// that is said.
pub fn rename(cx: &mut Ctx<'_>, level: &Level, old: &str, new: &str) -> Result<PathBuf, String> {
    let plane = cx.plane;
    crate::settings::writable(plane)?;
    if old == new {
        return Err(format!("it is already named {new}"));
    }
    if let Some(why) = change::name_problem(new) {
        return Err(format!("{new:?} cannot be a skill's name: {why}"));
    }
    cx.not_built_in(new)?;
    let from = find(plane, level, old)?;
    let renames_folder = folder_name(&from) != new;
    if renames_folder {
        free_at(plane, level, new, new)?;
    } else if level.dir(plane).and_then(|dir| named(&dir, new)).is_some() {
        return Err(format!("the {level} skills already have one named {new}"));
    }
    let what = format!("{old}'s instructions");
    let text = text_in(plane, &from, old)?;
    let renamed = change::with_name(&text, new)
        .ok_or_else(|| format!("{what} has no name: line to change"))?;
    let mut touched = vec![rel(plane, &from), rel(plane, &beside(&from))];
    let file = format!("{}/SKILL.md", rel(plane, &from));
    write(plane, &file, &what, Some(&text), &renamed, usize::MAX)?;
    let to = if renames_folder {
        let to = from.with_file_name(new);
        if let Err(why) = move_skill(plane, &from, &to, usize::MAX) {
            // The folder stayed where it was: put its name back.
            let _ = write(plane, &file, &what, Some(&renamed), &text, usize::MAX);
            return Err(format!("{old} was not renamed: {why}"));
        }
        touched.push(rel(plane, &to));
        touched.push(rel(plane, &beside(&to)));
        to
    } else {
        from.clone()
    };
    cx.done(format!("{old} is renamed {new}"));
    let still = Level::every(plane)
        .into_iter()
        .find(|other| other.dir(plane).and_then(|dir| named(&dir, old)).is_some());
    match still {
        Some(other) => cx.warn(format!(
            "the personas and skills that name {old} are left as they are: a {other} skill is \
             still named {old}"
        )),
        None => {
            let followed = references_renamed(cx, level, old, new);
            touched.extend(followed);
        }
    }
    cx.commit(&touched, &format!("skill rename {old} {new}"));
    Ok(to)
}

/// Rewrites every place in the plane that names the skill `old` to name `new`, and answers the
/// paths it changed. What could not be changed is said, and the rest is still changed.
fn references_renamed(cx: &mut Ctx<'_>, level: &Level, old: &str, new: &str) -> Vec<String> {
    let plane = cx.plane;
    let mut changed = Vec::new();
    let mut follow =
        |cx: &mut Ctx<'_>, path: &Path, what: &str, rewrite: &dyn Fn(&str) -> String| {
            let Ok(text) = text_at(plane, path, what) else {
                return;
            };
            let now = rewrite(&text);
            if now == text {
                return;
            }
            let shown = rel(plane, path);
            match write(plane, &shown, what, Some(&text), &now, usize::MAX) {
                Ok(()) => {
                    cx.done(format!("{shown} names {new} now"));
                    changed.push(shown);
                }
                Err(why) => cx.warn(format!("{why}, so it still names {old}")),
            }
        };
    for persona in crate::personaverbs::names(plane) {
        let file = crate::personas::def_path(plane, &persona);
        follow(cx, &file, &format!("persona {persona}"), &|text| {
            let listed = change::persona_list_renamed(text, LINKED_PLUGIN, old, new);
            change::references_renamed(&listed, LINKED_PLUGIN, old, new)
        });
    }
    for other in Level::every(plane) {
        let Some(dir) = other.dir(plane) else {
            continue;
        };
        for skill in super::read(&dir) {
            let what = format!("{}'s instructions", skill.name);
            follow(cx, &skill.path, &what, &|text| {
                change::references_renamed(text, LINKED_PLUGIN, old, new)
            });
        }
    }
    if level.at == At::Project {
        changed.extend(turned_off_renamed(cx, old, Some(new)));
    }
    changed
}

/// Each workspace's turn-off list with `old` named `new`, or taken out (`new` `None`), and the
/// workspace files it changed. What could not be written is said.
fn turned_off_renamed(cx: &mut Ctx<'_>, old: &str, new: Option<&str>) -> Vec<String> {
    let mut changed = Vec::new();
    for ws in workspaces(cx.plane) {
        let off = super::turned_off(cx.plane, &ws);
        if !off.iter().any(|one| one == old) {
            continue;
        }
        let now: Vec<String> = off
            .into_iter()
            .filter_map(|one| match new {
                _ if one != old => Some(one),
                Some(new) => Some(new.to_owned()),
                None => None,
            })
            .collect();
        match super::set_turned_off(cx.plane, &ws, now) {
            Ok(()) => changed.push(crate::settings::workspace::named(&ws)),
            Err(why) => cx.warn(format!("{ws} still turns {old} off by that name: {why}")),
        }
    }
    changed
}

/// Whether moving a skill from `from` to `to` makes it a project skill, which asks for its grant
/// (decision 14): a project skill reaches only the chats its grant names.
pub fn needs_grant(from: &Level, to: &Level) -> bool {
    to.at == At::Project && from.at != At::Project
}

/// What a move into the project's skills is told when it names no grant.
pub const GRANT_NEEDED: &str = "a project skill reaches only the chats its grant names, so say \
     where it reaches: the project's root, every workspace, or the workspaces named";

/// **Moves the skill named `name` from `from` to `to`** (#2770, decisions 14 and 37): its folder
/// and the file beside it, never over anything there. Into the project's skills it takes
/// `grant`, which it must have ([`needs_grant`]); out of them it leaves its grant behind, since a
/// workspace's skill has none, and each workspace's turn-off of it is taken out. One commit.
pub fn move_to(
    cx: &mut Ctx<'_>,
    from: &Level,
    name: &str,
    to: &Level,
    grant: Option<&Grant>,
) -> Result<PathBuf, String> {
    let plane = cx.plane;
    crate::settings::writable(plane)?;
    if from == to {
        return Err(format!("{name} is a {to} skill already"));
    }
    if needs_grant(from, to) && grant.is_none() {
        return Err(GRANT_NEEDED.to_owned());
    }
    if let Some(Grant::Only(names)) = grant {
        let held = crate::workspaces::Plane::open(plane);
        for ws in names {
            if !held.workspace(ws).is_ok_and(|one| one.dir().is_dir()) {
                return Err(format!(
                    "this project has no workspace {ws:?}, so nothing was moved"
                ));
            }
        }
    }
    let folder = find(plane, from, name)?;
    let folder_is = folder_name(&folder);
    let dir = free_at(plane, to, name, &folder_is)?;
    let moved = dir.join(&folder_is);
    move_skill(plane, &folder, &moved, to.depth())
        .map_err(|why| format!("{name} was not moved: {why}"))?;
    let mut touched = vec![
        rel(plane, &folder),
        rel(plane, &beside(&folder)),
        rel(plane, &moved),
        rel(plane, &beside(&moved)),
    ];
    if to.at == At::Project {
        if let Some(grant) = grant
            && let Err(why) = set_grant(&moved, grant)
        {
            cx.warn(format!(
                "{name} is moved, and its grant was not written: {why}"
            ));
        }
    } else if from.at == At::Project {
        // A workspace's skill reaches its workspace alone: it keeps no grant, and nothing turns
        // it off.
        if grant_of(&moved).is_ok_and(|was| was != Grant::Root) {
            let _ = set_grant(&moved, &Grant::Root);
        }
        touched.extend(turned_off_renamed(cx, name, None));
    }
    cx.done(format!(
        "{name} is a {to} skill now, in {}",
        rel(plane, &moved)
    ));
    cx.commit(&touched, &format!("skill mv {name} {to}"));
    Ok(moved)
}

/// Who uses a skill: what Delete shows before it deletes (decision 14).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Users {
    /// Whether chats at the project's root get it: a project skill's always do.
    pub root: bool,
    /// The workspaces whose chats get it: its own, or those its grant reaches and that did not
    /// turn it off.
    pub workspaces: Vec<String>,
    /// The personas that name it in their `skills:` list or their text.
    pub personas: Vec<String>,
    /// The other skills whose text names it.
    pub skills: Vec<String>,
}

/// **Who uses the skill named `name` at `level`**: the chats it reaches, and the personas and
/// skills that name it.
pub fn users(plane: &Path, level: &Level, name: &str) -> Result<Users, String> {
    let folder = find(plane, level, name)?;
    let mut users = Users::default();
    match &level.at {
        At::Project => {
            users.root = true;
            let grant = grant_of(&folder).unwrap_or(Grant::Root);
            for ws in workspaces(plane) {
                if grant.reaches(&ws)
                    && !super::turned_off(plane, &ws).iter().any(|one| one == name)
                {
                    users.workspaces.push(ws);
                }
            }
        }
        At::Workspace(ws) => users.workspaces.push(ws.clone()),
    }
    for persona in crate::personaverbs::names(plane) {
        let file = crate::personas::def_path(plane, &persona);
        if let Ok(text) = text_at(plane, &file, &persona)
            && (change::persona_names(&text, LINKED_PLUGIN, name)
                || change::references(&text, LINKED_PLUGIN, name))
        {
            users.personas.push(persona);
        }
    }
    for other in Level::every(plane) {
        let Some(dir) = other.dir(plane) else {
            continue;
        };
        for skill in super::read(&dir) {
            if skill.path.parent() == Some(folder.as_path()) {
                continue;
            }
            if let Ok(text) = text_at(plane, &skill.path, &skill.name)
                && change::references(&text, LINKED_PLUGIN, name)
                && !users.skills.contains(&skill.name)
            {
                users.skills.push(skill.name);
            }
        }
    }
    Ok(users)
}

/// The folder in this machine's state that keeps the skills deleted while their delete could not
/// be a commit, one folder per delete (`docs/plane-format.md`).
const DELETED_DIR: &str = "skills-deleted";

/// The file in a kept delete's folder that names the level its skill came from.
const KEPT_FROM: &str = "from";

/// Where a deleted skill can be restored from.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Kept(Source);

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Source {
    /// Git: the commit that removed it, whose parent still has it.
    Commit(String),
    /// This machine: its folder in [`DELETED_DIR`], by that folder's name.
    Here(String),
}

impl Kept {
    /// Whether git keeps it: a commit removed it, and Restore takes it back from there.
    pub fn in_git(&self) -> bool {
        matches!(self.0, Source::Commit(_))
    }
}

/// **A deleted skill that can be restored**, as [`deleted`] lists it and [`restore`] takes it
/// back: serialized whole, it is what names it in the window.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Deleted {
    pub name: String,
    pub level: Level,
    /// Its folder's name, which Restore gives it back.
    folder: String,
    kept: Kept,
    /// When it was deleted, in seconds since the epoch.
    pub at: u64,
}

impl Deleted {
    /// Where it can be restored from.
    pub fn kept(&self) -> &Kept {
        &self.kept
    }
}

/// `git <args>` in the plane: its stdout, or what it said.
fn git(plane: &Path, args: &[&str]) -> Result<String, String> {
    let run = crate::worktree::git::run(plane, args, crate::worktree::git::NETWORK)
        .map_err(|why| why.to_string())?;
    if run.ok() {
        Ok(run.out)
    } else {
        let said = run.err.trim();
        Err(if said.is_empty() {
            format!("git exited {}", run.code.unwrap_or(-1))
        } else {
            crate::shown::short(said)
        })
    }
}

/// Whether git tracks anything under `rel_path` in the plane.
fn tracked(plane: &Path, rel_path: &str) -> bool {
    git(plane, &["ls-files", "-z", "--", &literally(rel_path)]).is_ok_and(|out| !out.is_empty())
}

/// The commit the plane's `HEAD` names, or empty.
fn head(plane: &Path) -> String {
    git(plane, &["rev-parse", "HEAD"])
        .map(|out| out.trim().to_owned())
        .unwrap_or_default()
}

/// **Deletes the skill named `name` at `level`** (#2770, decision 14), so it can be restored, and
/// answers how. A skill git tracks is removed with `git rm` and committed where its commit can
/// land now, and Restore takes it back from that commit; one that holds files git does not have
/// yet is refused, as those could not come back. Where the commit cannot land now (no save mode
/// yet, saving off, something else staged), and for a skill git does not track, it is moved into
/// this machine's [`DELETED_DIR`] instead, and its removal is left for the next save.
pub fn delete(cx: &mut Ctx<'_>, level: &Level, name: &str) -> Result<Kept, String> {
    let plane = cx.plane;
    crate::settings::writable(plane)?;
    let folder = find(plane, level, name)?;
    let folder_rel = rel(plane, &folder);
    let toml_rel = rel(plane, &beside(&folder));
    let paths = [folder_rel.clone(), toml_rel.clone()];
    if tracked(plane, &folder_rel) {
        if !pending_in(plane, &[&folder_rel, &toml_rel])?.is_empty() {
            return Err(format!(
                "{name} holds changes that are not committed yet, and those could not be \
                 restored: save them first, then delete it"
            ));
        }
        if crate::planegit::paths_unsaved(plane, &paths).is_none() {
            return deleted_by_git(cx, name, &folder_rel, &toml_rel);
        }
    }
    let kept =
        keep_here(plane, level, &folder).map_err(|why| format!("{name} was not deleted: {why}"))?;
    cx.done(format!(
        "{name} is deleted; it is kept on this machine, and Restore brings it back"
    ));
    cx.commit(&paths, &format!("skill rm {name}"));
    Ok(Kept(Source::Here(kept)))
}

/// The committed skill `name`'s delete, where its commit can land: `git rm`, taken out of the
/// index again so the save stages it with the change, and committed. Put back as `HEAD` has it
/// where no commit was made.
fn deleted_by_git(
    cx: &mut Ctx<'_>,
    name: &str,
    folder_rel: &str,
    toml_rel: &str,
) -> Result<Kept, String> {
    let plane = cx.plane;
    let mut specs = vec![literally(folder_rel)];
    if tracked(plane, toml_rel) {
        specs.push(literally(toml_rel));
    }
    let mut args = vec!["rm", "-r", "-q", "--"];
    args.extend(specs.iter().map(String::as_str));
    git(plane, &args).map_err(|why| format!("{name} was not deleted: {why}"))?;
    let mut unstaged = vec!["reset", "-q", "--"];
    unstaged.extend(specs.iter().map(String::as_str));
    let _ = git(plane, &unstaged);
    let before = head(plane);
    cx.commit(
        &[folder_rel.to_owned(), toml_rel.to_owned()],
        &format!("skill rm {name}"),
    );
    let after = head(plane);
    if after.is_empty() || after == before {
        // What the save staged of the removal is taken out of the index, then the files are put
        // back as `HEAD` has them: the project is left as it was before the delete.
        let _ = git(plane, &unstaged);
        let mut back = vec!["restore", "--source=HEAD", "--worktree", "--"];
        back.extend(specs.iter().map(String::as_str));
        let restored = git(plane, &back).is_ok();
        return Err(if restored {
            format!("{name} was not deleted: its commit could not be made, so it was put back")
        } else {
            format!(
                "{name}'s commit could not be made, and it could not be put back: the project's \
                 last commit still has it"
            )
        });
    }
    cx.done(format!(
        "{name} is deleted; Restore brings it back from the commit"
    ));
    Ok(Kept(Source::Commit(after)))
}

/// Moves the skill's `folder`, and the file beside it, at `level` into a new folder of this
/// machine's [`DELETED_DIR`] with a `from` naming its level, and answers that folder's name.
fn keep_here(plane: &Path, level: &Level, folder: &Path) -> Result<String, String> {
    let state = crate::plane::state_dir(plane);
    let folder_is = folder_name(folder);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|at| at.as_millis())
        .unwrap_or_default();
    let kept = format!("{stamp}-{folder_is}");
    let from_level = level.rel().unwrap_or_default();
    // Private to the person, as every folder in the state directory is (`plane-format.md`).
    crate::profiletrust::private_dir(&state).map_err(|why| why.to_string())?;
    store::made(&state, &[DELETED_DIR], true).map_err(|why| why.to_string())?;
    store::made(&state, &[DELETED_DIR, &kept], false).map_err(|why| why.to_string())?;
    store::write_new(
        &state,
        &[DELETED_DIR, &kept],
        KEPT_FROM,
        &format!("{from_level}\n"),
    )
    .map_err(|why| why.to_string())?;
    let into = state.join(DELETED_DIR).join(&kept).join(&folder_is);
    move_between(plane, folder, &state, &into, usize::MAX)?;
    let toml = beside(folder);
    if toml.symlink_metadata().is_ok() {
        let _ = move_between(plane, &toml, &state, &beside(&into), usize::MAX);
    }
    Ok(kept)
}

/// How many deleted skills from git [`deleted`] lists at most, newest first.
const MOST_DELETED: usize = 30;

/// The git-log pathspecs of every shared skill's `SKILL.md`: the project's and each workspace's,
/// by the one rule of where a level's skills are ([`purlis_skills::level_rel`]).
fn deleted_globs() -> Vec<String> {
    [None, Some("*")]
        .into_iter()
        .filter_map(|ws| purlis_skills::level_rel(ws, false))
        .map(|dir| format!(":(glob){dir}/*/SKILL.md"))
        .collect()
}

/// **Every deleted skill that can be restored**, newest first: from the commits that removed a
/// committed skill's `SKILL.md`, and from this machine's [`DELETED_DIR`], read without following
/// a link. One whose folder, or whose name at its level, is taken again is left out: it would
/// come back as a second copy.
pub fn deleted(plane: &Path) -> Vec<Deleted> {
    let mut out = Vec::new();
    let state = crate::plane::state_dir(plane);
    for (kept, from, folder, text) in store::kept(&state, DELETED_DIR, KEPT_FROM) {
        let Some(level) = Level::of_rel(from.trim()) else {
            continue;
        };
        let Some(name) = name_in(&text) else {
            continue;
        };
        if taken_again(plane, &level, &name, &folder) {
            continue;
        }
        let at = kept
            .split_once('-')
            .and_then(|(ms, _)| ms.parse::<u64>().ok())
            .map_or(0, |ms| ms / 1000);
        out.push(Deleted {
            name,
            level,
            folder,
            kept: Kept(Source::Here(kept)),
            at,
        });
    }
    let globs = deleted_globs();
    let mut args = vec![
        "log",
        "--diff-filter=D",
        "--name-only",
        "-z",
        "--format=%x01%H %ct",
        "-n",
        "200",
        "--",
    ];
    args.extend(globs.iter().map(String::as_str));
    let log = git(plane, &args).unwrap_or_default();
    let mut seen: Vec<String> = Vec::new();
    for record in log.split('\u{1}').filter(|one| !one.trim().is_empty()) {
        let mut fields = record.split(['\0', '\n']).filter(|one| !one.is_empty());
        let Some(head) = fields.next() else {
            continue;
        };
        let (sha, at) = head.split_once(' ').unwrap_or((head, "0"));
        for path in fields {
            if out.len() >= MOST_DELETED {
                break;
            }
            let Some(folder_rel) = path.strip_suffix("/SKILL.md") else {
                continue;
            };
            if seen.iter().any(|one| one == folder_rel) {
                continue;
            }
            seen.push(folder_rel.to_owned());
            let Some((dir, folder)) = folder_rel.rsplit_once('/') else {
                continue;
            };
            let Some(level) = Level::of_rel(dir) else {
                continue;
            };
            let Ok(text) = git(plane, &["show", &format!("{sha}^:{path}")]) else {
                continue;
            };
            let Some(name) = name_in(&text) else {
                continue;
            };
            // Kept on this machine as well, where its delete could not be a commit then.
            let here_too = out
                .iter()
                .any(|one: &Deleted| one.level == level && one.folder == folder);
            if here_too || taken_again(plane, &level, &name, folder) {
                continue;
            }
            out.push(Deleted {
                name,
                level,
                folder: folder.to_owned(),
                kept: Kept(Source::Commit(sha.to_owned())),
                at: at.trim().parse().unwrap_or(0),
            });
        }
    }
    out.sort_by(|a, b| b.at.cmp(&a.at).then(a.name.cmp(&b.name)));
    out
}

/// The `name` a `SKILL.md`'s frontmatter names.
fn name_in(text: &str) -> Option<String> {
    purlis_skills::frontmatter(text)
        .into_iter()
        .find(|(key, value)| key == "name" && !value.is_empty())
        .map(|(_, value)| value)
}

/// Whether `level` holds a skill named `name`, or a folder named `folder` with a skill or a file
/// git tracks in it, again. Files git ignores, left behind in a deleted skill's folder (a build's
/// output, an editor's), take nothing: Restore puts the skill back beside them.
fn taken_again(plane: &Path, level: &Level, name: &str, folder: &str) -> bool {
    let Some(dir) = level.dir(plane) else {
        return true;
    };
    let at = dir.join(folder);
    let held = at.join("SKILL.md").symlink_metadata().is_ok()
        || beside(&at).symlink_metadata().is_ok()
        || (at.symlink_metadata().is_ok() && tracked(plane, &rel(plane, &at)));
    held || named(&dir, name).is_some()
}

/// **Restores the deleted skill `deleted`** to the level it came from: from the commit that
/// removed it, or from this machine's [`DELETED_DIR`]. Refused where it is no longer one
/// [`deleted`] lists (restored already, or its folder or its name at that level taken again).
/// One commit.
pub fn restore(cx: &mut Ctx<'_>, deleted: &Deleted) -> Result<PathBuf, String> {
    let plane = cx.plane;
    crate::settings::writable(plane)?;
    if !self::deleted(plane).contains(deleted) {
        return Err(format!(
            "{} cannot be restored: it is not in the project's history or on this machine any \
             more, or a skill of its name or folder is there again",
            deleted.name
        ));
    }
    let name = &deleted.name;
    let dir = level_is_there(plane, &deleted.level)?;
    let folder = dir.join(&deleted.folder);
    let folder_rel = rel(plane, &folder);
    let toml_rel = rel(plane, &beside(&folder));
    match &deleted.kept.0 {
        Source::Commit(sha) => {
            let source = format!("--source={sha}^");
            let folder_spec = literally(&folder_rel);
            let toml_spec = literally(&toml_rel);
            let mut args = vec!["restore", source.as_str(), "--worktree", "--", &folder_spec];
            let toml_there = git(plane, &["cat-file", "-e", &format!("{sha}^:{toml_rel}")]).is_ok();
            if toml_there {
                args.push(&toml_spec);
            }
            git(plane, &args).map_err(|why| format!("{name} was not restored: {why}"))?;
        }
        Source::Here(kept) => {
            let state = crate::plane::state_dir(plane);
            let from = state.join(DELETED_DIR).join(kept).join(&deleted.folder);
            move_between(&state, &from, plane, &folder, deleted.level.depth())
                .map_err(|why| format!("{name} was not restored: {why}"))?;
            let toml = beside(&from);
            if toml.symlink_metadata().is_ok() {
                let _ = move_between(&state, &toml, plane, &beside(&folder), usize::MAX);
            }
            // Its `from` and the folder it kept, now empty, and nothing else.
            store::put_away(&state, DELETED_DIR, kept, KEPT_FROM);
        }
    }
    cx.done(format!(
        "{name} is restored, a {} skill in {folder_rel}",
        deleted.level
    ));
    cx.commit(&[folder_rel, toml_rel], &format!("skill restore {name}"));
    Ok(folder)
}

/// **Grants the project skill `name`** `grant` (#2767), committed (#2770).
pub fn grant(cx: &mut Ctx<'_>, name: &str, grant: &Grant) -> Result<PathBuf, String> {
    let folder = super::grant(cx.plane, name, grant)?;
    cx.done(format!("{name} is granted to {grant}"));
    cx.commit(
        &[rel(cx.plane, &beside(&folder))],
        &format!("skill grant {name} {grant}"),
    );
    Ok(folder)
}

/// **Turns the project skill `name` off (`on` false) or back on in `workspace`** (#2767),
/// committed (#2770). `Ok(false)` where it already was so, and nothing is written.
pub fn switch(cx: &mut Ctx<'_>, workspace: &str, name: &str, on: bool) -> Result<bool, String> {
    let changed = super::switch(cx.plane, workspace, name, on)?;
    if changed {
        let state = if on { "on" } else { "off" };
        cx.done(format!("{name} is {state} in {workspace}"));
        cx.commit(
            &[crate::settings::workspace::named(workspace)],
            &format!("skill {state} {name} in {workspace}"),
        );
    }
    Ok(changed)
}

/// **Share** (#2767): the local skill `name` at the project's root or in `workspace` moved into
/// its level's `skills/`, committed (#2770).
pub fn share(cx: &mut Ctx<'_>, workspace: Option<&str>, name: &str) -> Result<PathBuf, String> {
    let plane = cx.plane;
    let from = level_dir(plane, workspace, true)
        .and_then(|dir| named(&dir, name))
        .ok_or_else(|| format!("no local skill is named {name:?} there"))?;
    let folder = super::share_here(plane, workspace, name)?;
    cx.done(format!(
        "{name} is shared: it is in {} now",
        rel(plane, &folder)
    ));
    cx.commit(
        &[
            rel(plane, &from),
            rel(plane, &folder),
            rel(plane, &beside(&folder)),
        ],
        &format!("skill share {name}"),
    );
    Ok(folder)
}

/// Moves a skill's folder, and the file beside it, from `from` to `to`, both in the plane.
fn move_skill(plane: &Path, from: &Path, to: &Path, make_from: usize) -> Result<(), String> {
    move_between(plane, from, plane, to, make_from)?;
    let (toml_from, toml_to) = (beside(from), beside(to));
    if toml_from.symlink_metadata().is_ok()
        && let Err(why) = move_between(plane, &toml_from, plane, &toml_to, usize::MAX)
    {
        // The folder moved and the file beside it did not: put the folder back.
        let _ = move_between(plane, to, plane, from, usize::MAX);
        return Err(why);
    }
    Ok(())
}

/// The names of `path`'s folders under `root`, and its own name, or why it is no plain path
/// under it.
fn steps(root: &Path, path: &Path) -> Result<(Vec<String>, String), String> {
    let inside = path
        .strip_prefix(root)
        .map_err(|_| "it is outside where it may be".to_owned())?;
    let mut names = Vec::new();
    for part in inside.components() {
        match part {
            std::path::Component::Normal(name) => names.push(name.to_string_lossy().into_owned()),
            _ => return Err("its path is not a plain one".to_owned()),
        }
    }
    let leaf = names
        .pop()
        .ok_or_else(|| "its path names nothing".to_owned())?;
    Ok((names, leaf))
}

/// Moves the entry at `from`, under `from_root`, to `to`, under `to_root`, never over anything
/// there and following no link on either way; folders of `to` from the `make_from`th on are made
/// where they are not there.
#[cfg(unix)]
fn move_between(
    from_root: &Path,
    from: &Path,
    to_root: &Path,
    to: &Path,
    make_from: usize,
) -> Result<(), String> {
    use rustix::fs::{AtFlags, FileType, statat};
    let (from_dirs, from_leaf) = steps(from_root, from)?;
    let (to_dirs, to_leaf) = steps(to_root, to)?;
    let from_at = store::open_dir(from_root, &from_dirs, usize::MAX)
        .map_err(|why| format!("its folder could not be opened ({why})"))?;
    let stat = statat(&from_at, from_leaf.as_str(), AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|why| format!("it is not there ({why})"))?;
    let kind = FileType::from_raw_mode(stat.st_mode);
    if kind != FileType::Directory && kind != FileType::RegularFile {
        return Err("it is not a plain folder or file, so purlis does not move it".to_owned());
    }
    let to_at = store::open_dir(to_root, &to_dirs, make_from)
        .map_err(|why| format!("the folder it goes into could not be made ({why})"))?;
    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios"
    ))]
    match rustix::fs::renameat_with(
        &from_at,
        from_leaf.as_str(),
        &to_at,
        to_leaf.as_str(),
        rustix::fs::RenameFlags::NOREPLACE,
    ) {
        Ok(()) => return Ok(()),
        Err(rustix::io::Errno::EXIST) => {
            return Err("something of its name is already where it goes".to_owned());
        }
        Err(rustix::io::Errno::INVAL | rustix::io::Errno::NOSYS | rustix::io::Errno::NOTSUP) => {}
        Err(why) => return Err(format!("it could not be moved ({why})")),
    }
    if statat(&to_at, to_leaf.as_str(), AtFlags::SYMLINK_NOFOLLOW).is_ok() {
        return Err("something of its name is already where it goes".to_owned());
    }
    rustix::fs::renameat(&from_at, from_leaf.as_str(), &to_at, to_leaf.as_str())
        .map_err(|why| format!("it could not be moved ({why})"))
}

/// [`move_between`] where no folder can be held open: refused, as a skill is never moved by a
/// path a link could redirect.
#[cfg(not(unix))]
fn move_between(_: &Path, _: &Path, _: &Path, _: &Path, _: usize) -> Result<(), String> {
    Err("a skill is moved only where its folders can be held open".to_owned())
}

/// **This machine's kept deletes, through handles** (#2770): every folder opened one component
/// at a time with `O_NOFOLLOW`, every file made new with `O_EXCL | O_NOFOLLOW`, read only where it
/// is one plain file, and nothing removed but the names a kept delete is made of.
#[cfg(unix)]
pub(in crate::skills) mod store {
    use std::io::{Read as _, Write as _};
    use std::path::Path;

    use rustix::fd::OwnedFd;
    use rustix::fs::{AtFlags, CWD, FileType, Mode, OFlags, fstat, mkdirat, openat, unlinkat};

    pub(in crate::skills) const FOLDER: OFlags = OFlags::RDONLY
        .union(OFlags::DIRECTORY)
        .union(OFlags::CLOEXEC)
        .union(OFlags::NOFOLLOW);

    /// The folder `names` names under `root`, opened one component at a time following no
    /// link, making those from the `make_from`th on (counting from 1) that are not there.
    pub(in crate::skills) fn open_dir(
        root: &Path,
        names: &[String],
        make_from: usize,
    ) -> rustix::io::Result<OwnedFd> {
        let mut at = openat(
            CWD,
            root,
            FOLDER.difference(OFlags::NOFOLLOW),
            Mode::empty(),
        )?;
        for (index, name) in names.iter().enumerate() {
            at = match openat(&at, name.as_str(), FOLDER, Mode::empty()) {
                Ok(next) => next,
                Err(rustix::io::Errno::NOENT) if index + 1 >= make_from => {
                    mkdirat(&at, name.as_str(), Mode::from_raw_mode(0o755))?;
                    openat(&at, name.as_str(), FOLDER, Mode::empty())?
                }
                Err(why) => return Err(why),
            };
        }
        Ok(at)
    }

    /// Makes the folder `names` names under `root`, `0700`, its parents being there: one
    /// already there is taken where `there_is_fine`, and only as a folder.
    pub(super) fn made(root: &Path, names: &[&str], there_is_fine: bool) -> std::io::Result<()> {
        let (leaf, parents) = names.split_last().ok_or(std::io::ErrorKind::InvalidInput)?;
        let parents: Vec<String> = parents.iter().map(|one| (*one).to_owned()).collect();
        let at = open_dir(root, &parents, usize::MAX)?;
        match mkdirat(&at, *leaf, Mode::from_raw_mode(0o700)) {
            Ok(()) => Ok(()),
            Err(rustix::io::Errno::EXIST) if there_is_fine => {
                openat(&at, *leaf, FOLDER, Mode::empty())
                    .map(|_| ())
                    .map_err(Into::into)
            }
            Err(why) => Err(why.into()),
        }
    }

    /// Writes `text` as the new file `name` in the folder `names` names under `root`, `0600`.
    pub(super) fn write_new(
        root: &Path,
        names: &[&str],
        name: &str,
        text: &str,
    ) -> std::io::Result<()> {
        let names: Vec<String> = names.iter().map(|one| (*one).to_owned()).collect();
        let at = open_dir(root, &names, usize::MAX)?;
        let file = openat(
            &at,
            name,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o600),
        )?;
        std::fs::File::from(file).write_all(text.as_bytes())
    }

    /// The text of `name` in the folder `at`, where it is one plain file with one name, of at
    /// most a megabyte.
    fn text_of(at: &OwnedFd, name: &str) -> Option<String> {
        let file = openat(
            at,
            name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .ok()?;
        let stat = fstat(&file).ok()?;
        if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile || stat.st_nlink != 1 {
            return None;
        }
        let mut text = String::new();
        std::fs::File::from(file)
            .take(1024 * 1024)
            .read_to_string(&mut text)
            .ok()?;
        Some(text)
    }

    /// The names in the folder `at`, of the kind `kind`, but `.` and `..`.
    fn names_in(at: &OwnedFd, kind: FileType) -> Vec<String> {
        let Ok(dir) = rustix::fs::Dir::read_from(at) else {
            return Vec::new();
        };
        dir.filter_map(Result::ok)
            .filter(|entry| entry.file_type() == kind)
            .filter_map(|entry| entry.file_name().to_str().ok().map(str::to_owned))
            .filter(|name| name != "." && name != "..")
            .collect()
    }

    /// Every kept delete in `dir` under `state`: its folder's name, its `from` file's text, the
    /// skill folder it holds and that folder's `SKILL.md`. One that is not whole is left out.
    pub(super) fn kept(
        state: &Path,
        dir: &str,
        from: &str,
    ) -> Vec<(String, String, String, String)> {
        let Ok(kept) = open_dir(state, &[dir.to_owned()], usize::MAX) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for name in names_in(&kept, FileType::Directory) {
            let Ok(one) = openat(&kept, name.as_str(), FOLDER, Mode::empty()) else {
                continue;
            };
            let Some(level) = text_of(&one, from) else {
                continue;
            };
            let folders = names_in(&one, FileType::Directory);
            let [folder] = folders.as_slice() else {
                continue;
            };
            let Ok(skill) = openat(&one, folder.as_str(), FOLDER, Mode::empty()) else {
                continue;
            };
            let Some(text) = text_of(&skill, "SKILL.md") else {
                continue;
            };
            out.push((name, level, folder.clone(), text));
        }
        out
    }

    /// Takes away the kept delete `kept` in `dir` under `state` once its skill went back: its
    /// `from` file, then the folder, which must be empty by then.
    pub(super) fn put_away(state: &Path, dir: &str, kept: &str, from: &str) {
        let Ok(at) = open_dir(state, &[dir.to_owned()], usize::MAX) else {
            return;
        };
        if let Ok(one) = openat(&at, kept, FOLDER, Mode::empty()) {
            let _ = unlinkat(&one, from, AtFlags::empty());
        }
        let _ = unlinkat(&at, kept, AtFlags::REMOVEDIR);
    }
}

/// [`store`] where no folder can be held open: nothing is kept, and nothing is read.
#[cfg(not(unix))]
mod store {
    use std::path::Path;

    fn refused() -> std::io::Error {
        std::io::Error::other("a deleted skill is kept only where its folders can be held open")
    }

    pub(super) fn made(_: &Path, _: &[&str], _: bool) -> std::io::Result<()> {
        Err(refused())
    }

    pub(super) fn write_new(_: &Path, _: &[&str], _: &str, _: &str) -> std::io::Result<()> {
        Err(refused())
    }

    pub(super) fn kept(_: &Path, _: &str, _: &str) -> Vec<(String, String, String, String)> {
        Vec::new()
    }

    pub(super) fn put_away(_: &Path, _: &str, _: &str, _: &str) {}
}

#[cfg(test)]
mod tests;
