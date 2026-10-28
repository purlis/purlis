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
//! machine's [`DELETED_DIR`], where Restore finds it. A delete by git holds a note locked in
//! [`DELETING_DIR`] while it runs, so one a crash cuts short between its removal and its commit
//! is finished or put back the next time skills are read for a change ([`interrupted`], SK-35).
//!
//! **What it says is the window's words too.** A refusal or a warning names the skill and what to
//! do, never a file's path or a command; a `Done` line, which only the command line prints, may.

use std::path::{Path, PathBuf};

use purlis_skills::change::{self, SkillTemplate, TemplateId};

use super::{Grant, LINKED_PLUGIN, Origin, beside, grant_of, level_dir, set_grant};
use crate::planegit::literally;
use crate::repocmd::{Say, Sink};
use purlis_skills::{Unmoved, move_skill};

pub mod drafts;

/// Where a level is: the project's root, one workspace or one persona.
pub use purlis_skills::Whose;

/// A plane level a skill lives at (ADR 0099 §1): the project's, one workspace's or one
/// persona's (SK-32, #3003), shared or local.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Level {
    pub at: Whose,
    /// Its `skills.local/`, kept on this machine.
    pub local: bool,
}

impl Level {
    /// The project's shared skills: `skills/` at its root.
    pub fn project() -> Level {
        Level {
            at: Whose::Project,
            local: false,
        }
    }

    /// `workspace`'s shared skills.
    pub fn workspace(workspace: &str) -> Level {
        Level {
            at: Whose::Workspace(workspace.to_owned()),
            local: false,
        }
    }

    /// `persona`'s shared skills (SK-32, #3003).
    pub fn persona(persona: &str) -> Level {
        Level {
            at: Whose::Persona(persona.to_owned()),
            local: false,
        }
    }

    /// The same level's local skills, where `local`.
    pub fn local(self, local: bool) -> Level {
        Level { local, ..self }
    }

    /// The level a skill in it comes from.
    pub fn origin(&self) -> Origin {
        match self.at {
            Whose::Project => Origin::Project,
            Whose::Workspace(_) => Origin::Workspace,
            Whose::Persona(_) => Origin::Persona,
        }
    }

    /// Its folder in the plane at `plane`, or `None` for a workspace name that names no folder.
    pub fn dir(&self, plane: &Path) -> Option<PathBuf> {
        level_dir(plane, &self.at, self.local)
    }

    /// The folder of its skill named `name` in the plane at `plane`, found by a walk from the
    /// plane's root that follows no link ([`purlis_skills::named_at`], SK-36), or `None`.
    pub fn named(&self, plane: &Path, name: &str) -> Option<PathBuf> {
        purlis_skills::named_at(plane, &self.at, self.local, name)
    }

    /// Its folder relative to the plane, `/` between the parts, by the one rule
    /// ([`purlis_skills::level_rel`]): `skills`, `workspaces/ops/skills.local`,
    /// `personas/reviewer/skills`.
    pub(in crate::skills) fn rel(&self) -> Option<String> {
        purlis_skills::level_rel(&self.at, self.local)
    }

    /// The level whose folder `rel` is, read by the same rule.
    pub(in crate::skills) fn of_rel(rel: &str) -> Option<Level> {
        let (at, local) = purlis_skills::level_of_rel(rel)?;
        Some(Level { at, local })
    }

    /// How many folders its [`Self::rel`] has: the first one a new skill may make is the last.
    pub(in crate::skills) fn depth(&self) -> usize {
        self.rel().map_or(usize::MAX, |rel| rel.split('/').count())
    }

    /// **Every level of the plane at `plane`**: the project's, then each workspace's, then each
    /// persona's that has a folder of its own (a one-file persona holds no skills), shared then
    /// local. What the studio's level pickers offer, and where a rename looks for names.
    pub fn every(plane: &Path) -> Vec<Level> {
        let mut levels = vec![Level::project(), Level::project().local(true)];
        for ws in workspaces(plane) {
            levels.push(Level::workspace(&ws));
            levels.push(Level::workspace(&ws).local(true));
        }
        for persona in personas(plane) {
            levels.push(Level::persona(&persona));
            levels.push(Level::persona(&persona).local(true));
        }
        levels
    }

    /// **Every copy of skill `name` in the plane at `plane`, in the order its tab shows them**
    /// (#3443): each level of [`Self::every`] that holds one, with its folder. The one place that
    /// order is decided: the first is the copy the tab shows first, and the one its Grades panel
    /// reads until the tab names another.
    pub fn copies_of<'a>(
        plane: &'a Path,
        name: &'a str,
    ) -> impl Iterator<Item = (Level, PathBuf)> + 'a {
        Self::every(plane).into_iter().filter_map(move |level| {
            let folder = level.named(plane, name)?;
            Some((level, folder))
        })
    }
}

/// `project`, `local project`, `workspace ops`, `local workspace ops`: as a sentence names it.
impl std::fmt::Display for Level {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let shown = super::level_shown(self.origin(), self.local);
        match &self.at {
            Whose::Project => f.write_str(&shown),
            Whose::Workspace(ws) => write!(f, "{shown} {ws}"),
            Whose::Persona(persona) => write!(f, "{shown} {persona}"),
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

/// The plane's personas that can hold skills of their own: those with a folder
/// (`personas/<p>/persona.md`), by name. A one-file persona has no folder, so no skills
/// (`docs/plane-format.md`).
fn personas(plane: &Path) -> Vec<String> {
    crate::personaverbs::names(plane)
        .into_iter()
        .filter(|persona| holds_skills(plane, persona))
        .collect()
}

/// **Whether `persona` is one with a folder of its own** (`personas/<p>/persona.md`), which a
/// write into its skills is gated on: its folders opened one at a time with `O_NOFOLLOW`
/// ([`store::open_dir`]) and its `persona.md` looked at through that handle, so no link on the
/// way is followed.
#[cfg(unix)]
pub(in crate::skills) fn holds_skills(plane: &Path, persona: &str) -> bool {
    use rustix::fs::{AtFlags, FileType, statat};
    if !purlis_skills::level_rel(&Whose::Persona(persona.to_owned()), false).is_some() {
        return false;
    }
    let Ok(dir) = store::open_dir(
        plane,
        &["personas".to_owned(), persona.to_owned()],
        usize::MAX,
    ) else {
        return false;
    };
    statat(&dir, "persona.md", AtFlags::SYMLINK_NOFOLLOW)
        .is_ok_and(|stat| FileType::from_raw_mode(stat.st_mode) == FileType::RegularFile)
}

/// [`holds_skills`] where no folder can be held open: no persona holds skills there.
#[cfg(not(unix))]
pub(in crate::skills) fn holds_skills(_: &Path, _: &str) -> bool {
    false
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
            (self.say)(Say::Info(why.into()));
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
                    .into(),
            ));
        }
    }

    /// **Refused where `name` belongs to a built-in skill** ([`super::name_reserved`], spec
    /// #3679 S12), with [`super::NAME_RESERVED`]: so no skill of the plane stands in for a
    /// guarded built-in such as `secrets`. Every change that lands a skill under a name asks it
    /// first: [`create`], [`rename`], [`move_to`], [`share`] and a draft's accept, and Make my own
    /// copy of the copy's name (#3686). [`restore`], an install and an update's skills brought
    /// along ask [`Self::reserved`], and say it in their own words.
    pub fn not_built_in(&self, name: &str) -> Result<(), String> {
        if self.reserved(name) {
            return Err(super::NAME_RESERVED.to_owned());
        }
        Ok(())
    }

    /// Whether `name` belongs to a built-in skill ([`super::name_reserved`]): what a change that
    /// cannot offer another name (a restore, an install) asks, and says in its own words.
    pub fn reserved(&self, name: &str) -> bool {
        super::name_reserved(self.built_in, name)
    }

    /// Says `line` as a warning.
    pub(in crate::skills) fn warn(&mut self, line: String) {
        (self.say)(Say::Warn(line.into()));
    }

    /// Says `line` as done.
    pub(in crate::skills) fn done(&mut self, line: String) {
        (self.say)(Say::Done(line.into()));
    }
}

/// Refused where `level` is a workspace's and the plane has no such workspace.
pub(in crate::skills) fn level_is_there(plane: &Path, level: &Level) -> Result<PathBuf, String> {
    match &level.at {
        Whose::Project => {}
        Whose::Workspace(ws) => {
            let held = crate::workspaces::Plane::open(plane);
            if !held.workspace(ws).is_ok_and(|one| one.dir().is_dir()) {
                return Err(format!(
                    "this project has no workspace {ws:?}, so nothing was changed"
                ));
            }
        }
        Whose::Persona(persona) => {
            if !crate::personaverbs::names(plane).contains(persona) {
                return Err(format!(
                    "this project has no persona {persona:?}, so nothing was changed"
                ));
            }
            if !holds_skills(plane, persona) {
                return Err(format!(
                    "persona {persona} keeps no skills of its own, as a persona made as one file \
                     does not: add the skill to its skills by name instead"
                ));
            }
        }
    }
    level
        .dir(plane)
        .ok_or_else(|| format!("{level} is no level of this project"))
}

/// The folder of the skill named `name` at `level`, or why there is none.
pub(super) fn find(plane: &Path, level: &Level, name: &str) -> Result<PathBuf, String> {
    level
        .named(plane, name)
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
    if level.named(plane, name).is_some() {
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
    template: &TemplateId,
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
            "that template does not make a whole skill: {problem}"
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
pub(super) fn pending_in(plane: &Path, paths: &[&str]) -> Result<Vec<String>, String> {
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
        (cx.say)(Say::Info(
            format!("{name} is unchanged, so nothing was saved.").into(),
        ));
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
            crate::counts::plural(others, "has", "have")
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
    } else if level.named(plane, new).is_some() {
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
        match move_skill(plane, &from, plane, &to, usize::MAX) {
            Ok(()) => {}
            Err(Unmoved::Nothing(why)) => {
                // The folder stayed where it was: put its name back.
                let _ = write(plane, &file, &what, Some(&renamed), &text, usize::MAX);
                return Err(format!("{old} was not renamed: {why}"));
            }
            // Its folder went and could not come back (SK-45): say so, never "not renamed".
            Err(Unmoved::Stranded(what)) => {
                return Err(format!("{old} was renamed only in part: {what}"));
            }
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
        .find(|other| other.named(plane, old).is_some());
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
    if level.at == Whose::Project {
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
    to.at == Whose::Project && from.at != Whose::Project
}

/// What a move into the project's skills is told when it names no grant.
pub const GRANT_NEEDED: &str = "a project skill reaches only the chats its grant names, so say \
     where it reaches: the project's root, every workspace, or the workspaces named";

/// **Moves the skill named `name` from `from` to `to`** (#2770, decisions 14 and 37): its folder
/// and the file beside it, never over anything there. Into the project's skills it takes
/// `grant`, which it must have ([`needs_grant`]); out of them it leaves its grant behind, since a
/// workspace's or a persona's skill has none, and each workspace's turn-off of it is taken out.
///
/// **A persona keeps what it had** (SK-32, #3003, decision 4): a persona's own skill moved into
/// the project's skills is added to that persona's `skills:` list, which brings a project skill
/// into every workspace, so the persona's chats keep it; one moved into a persona's skills is
/// taken out of that persona's list, which no longer needs to name it. What another persona or
/// a workspace no longer gets is said. One commit.
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
    cx.not_built_in(name)?;
    let folder = find(plane, from, name)?;
    let folder_is = folder_name(&folder);
    let dir = free_at(plane, to, name, &folder_is)?;
    let moved = dir.join(&folder_is);
    move_skill(plane, &folder, plane, &moved, to.depth()).map_err(|unmoved| match unmoved {
        Unmoved::Nothing(why) => format!("{name} was not moved: {why}"),
        Unmoved::Stranded(what) => format!("{name} was moved only in part: {what}"),
    })?;
    let mut touched = vec![
        rel(plane, &folder),
        rel(plane, &beside(&folder)),
        rel(plane, &moved),
        rel(plane, &beside(&moved)),
    ];
    if to.at == Whose::Project {
        if let Some(grant) = grant
            && let Err(why) = set_grant(&moved, grant)
        {
            cx.warn(format!(
                "{name} is moved, and its grant was not written: {why}"
            ));
        }
    } else if from.at == Whose::Project {
        // A workspace's or a persona's skill reaches its own chats alone: it keeps no grant, and
        // nothing turns it off.
        if grant_of(&moved).is_ok_and(|was| was != Grant::Root) {
            let _ = set_grant(&moved, &Grant::Root);
        }
        touched.extend(turned_off_renamed(cx, name, None));
    }
    if from.at.persona() != to.at.persona() {
        touched.extend(personas_followed(cx, from, name, to));
    }
    cx.done(format!(
        "{name} is a {to} skill now, in {}",
        rel(plane, &moved)
    ));
    cx.commit(&touched, &format!("skill mv {name} {to}"));
    Ok(moved)
}

/// The persona lists a move of the skill `name` from `from` to `to` changes, between a persona's
/// own skills and another level (SK-32, #3003), and the persona files it changed. What could not
/// be written, and what a persona or a workspace no longer gets, is said.
fn personas_followed(cx: &mut Ctx<'_>, from: &Level, name: &str, to: &Level) -> Vec<String> {
    use super::persona::{Change, change};
    let plane = cx.plane;
    let mut changed = Vec::new();
    let mut listed =
        |cx: &mut Ctx<'_>, persona: &str, how: Change| match change(plane, persona, name, how) {
            Ok(true) => changed.push(rel(plane, &crate::personas::def_path(plane, persona))),
            Ok(false) => {}
            Err(why) => cx.warn(format!(
                "persona {persona}'s skills could not be changed for {name}: {why}"
            )),
        };
    if let Some(owner) = from.at.persona() {
        match &to.at {
            Whose::Project => listed(cx, owner, Change::Add),
            Whose::Workspace(ws) => cx.warn(format!(
                "persona {owner} gets {name} only in workspace {ws} now"
            )),
            Whose::Persona(other) => cx.warn(format!(
                "persona {owner} no longer gets {name}: it is persona {other}'s own now"
            )),
        }
    }
    if let Some(owner) = to.at.persona() {
        listed(cx, owner, Change::Remove);
        let others: Vec<String> = crate::personaverbs::names(plane)
            .into_iter()
            .filter(|persona| persona != owner)
            .filter(|persona| {
                let file = crate::personas::def_path(plane, persona);
                text_at(plane, &file, persona)
                    .is_ok_and(|text| change::persona_names(&text, LINKED_PLUGIN, name))
            })
            .collect();
        if !others.is_empty() {
            cx.warn(format!(
                "{} {} {name}, and a persona's own skill reaches that persona's chats alone, so \
                 {} no longer get it",
                crate::counts::counted(others.len(), "persona", "personas"),
                crate::counts::plural(others.len(), "names", "name"),
                others.join(", ")
            ));
        }
        let reached = match &from.at {
            Whose::Project => Some("the project's chats".to_owned()),
            Whose::Workspace(ws) => Some(format!("workspace {ws}'s chats")),
            Whose::Persona(_) => None,
        };
        if let Some(who) = reached {
            cx.warn(format!("{who} get {name} now only as persona {owner}"));
        }
    }
    changed
}

/// Who uses a skill: what Delete shows before it deletes (decision 14).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Users {
    /// Whether chats at the project's root get it: a project skill's always do.
    pub root: bool,
    /// The workspaces whose chats get it: its own, or those its grant reaches and that did not
    /// turn it off.
    pub workspaces: Vec<String>,
    /// The persona whose own skill it is: chats as that persona get it in every workspace
    /// (decision 4).
    pub persona: Option<String>,
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
        Whose::Project => {
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
        Whose::Workspace(ws) => users.workspaces.push(ws.clone()),
        Whose::Persona(persona) => users.persona = Some(persona.clone()),
    }
    for persona in crate::personaverbs::names(plane) {
        let file = crate::personas::def_path(plane, &persona);
        if users.persona.as_ref() == Some(&persona) {
            continue;
        }
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

/// The folder in this machine's state that holds a note for each delete by git while it runs,
/// held locked by the delete (SK-35, #3013, `docs/plane-format.md`): one left unlocked is a delete
/// a crash, a quit or a failed git cut short, which [`interrupted`] finishes or undoes.
const DELETING_DIR: &str = "skills-deleting";

/// The folders of this machine's state that skill deletes keep, which only purlis writes: a
/// chat's sandbox denies writing them (`crate::sandbox`).
pub(crate) const KEPT_BY_DELETES: [&str; 2] = [DELETED_DIR, DELETING_DIR];

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

    /// **How long it stays, in a phrase** (SK-33): one git keeps stays in the project's history;
    /// one this machine keeps can be restored [`KEPT_HERE_FOR`] after its delete, and the phrase
    /// counts the days left at `now`, never more than that (a delete stamped by a clock that ran
    /// ahead).
    pub fn stays(&self, now: std::time::SystemTime) -> String {
        match &self.kept.0 {
            Source::Commit(_) => "kept in the project's history".to_owned(),
            Source::Here(_) => {
                let now = u64::try_from(ms_since_epoch(now) / 1000).unwrap_or(u64::MAX);
                let kept_for = KEPT_HERE_FOR.as_secs();
                let until = self.at.saturating_add(kept_for);
                let days = until.saturating_sub(now).min(kept_for).div_ceil(DAY);
                match days {
                    0 => "removed from this machine soon".to_owned(),
                    1 => "kept on this machine for 1 more day".to_owned(),
                    days => format!("kept on this machine for {days} more days"),
                }
            }
        }
    }
}

/// A day, in seconds.
const DAY: u64 = 24 * 60 * 60;

/// **How long a skill deleted on this machine can be restored from its [`DELETED_DIR`]**
/// (SK-33): 30 days after its delete, as a desktop's trash or a mail folder's deleted items keep
/// theirs. Past that, [`deleted`] lists it no more, and [`expire`] removes it a day later
/// ([`EXPIRY_AFTER`]), so a restore begun within its time never meets its removal. A skill whose
/// delete was a commit is never removed: git keeps it.
pub const KEPT_HERE_FOR: std::time::Duration = std::time::Duration::from_secs(30 * DAY);

/// How long after its delete [`expire`] removes a kept delete: [`KEPT_HERE_FOR`] and a day.
pub const EXPIRY_AFTER: std::time::Duration = std::time::Duration::from_secs(31 * DAY);

/// `at` in milliseconds since the epoch, 0 before it.
fn ms_since_epoch(at: std::time::SystemTime) -> u128 {
    at.duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_millis())
        .unwrap_or_default()
}

/// Whether a kept delete stamped `stamp` is more than `kept` old at `now_ms`. One stamped later
/// than now is not.
#[cfg_attr(not(unix), allow(dead_code))]
fn past(stamp: u128, now_ms: u128, kept: std::time::Duration) -> bool {
    now_ms
        .checked_sub(stamp)
        .is_some_and(|age| age > kept.as_millis())
}

/// What one [`expire`] did: the kept deletes it removed, by their folder's name, and each it
/// could not, with why.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Expired {
    pub removed: Vec<String>,
    pub failed: Vec<(String, String)>,
}

/// **Removes the skills this machine kept deleted for longer than [`EXPIRY_AFTER`]** (SK-33):
/// each kept delete in the state folder's [`DELETED_DIR`] whose stamp, the time of its delete,
/// is that long before `now`. One deleted later, or with a stamp after `now`, is kept, so each
/// copy, the newest of a skill's included, stays its whole time.
///
/// **Only what a delete made is removed, by the names it gives it**: a folder named
/// `<stamp>-<folder>`, holding nothing but `from`, the skill's folder `<folder>` and the
/// `<folder>.toml` beside it. A kept delete holding anything else is left whole, and said. The
/// state folder, [`DELETED_DIR`] and each folder in it are opened as folders themselves
/// (`O_NOFOLLOW`) and everything is removed relative to them, so no link is followed, even one
/// put in place after a check: a link inside the skill's folder is removed as a link, and
/// nothing outside [`DELETED_DIR`] is ever reached. Off Unix nothing is removed, as nothing is
/// kept there.
pub fn expire(plane: &Path, now: std::time::SystemTime) -> Expired {
    let state = crate::plane::state_dir(plane);
    store::expire(&state, DELETED_DIR, KEPT_FROM, ms_since_epoch(now))
}

/// [`expire`] in `plane` at this moment, each removal and each failure said in the log: what
/// the Skill studio does as it opens, and `purlis skill` before a change other than a restore.
pub fn expire_now(plane: &Path) {
    let expired = expire(plane, std::time::SystemTime::now());
    let days = EXPIRY_AFTER.as_secs() / DAY;
    for kept in &expired.removed {
        tracing::info!(
            "purlis: removed the deleted skill kept as {kept}: it was deleted over {days} days ago"
        );
    }
    for (kept, why) in &expired.failed {
        tracing::warn!(
            "purlis: the deleted skill kept as {kept} is past its {days} days but was not \
             removed ({why}); it is tried again next time"
        );
    }
}

/// The stamp and the skill's folder name of a kept delete named `<stamp>-<folder>`, where its
/// name is one a delete gives ([`keep_here`]): digits, a `-`, and one plain folder name.
#[cfg_attr(not(unix), allow(dead_code))]
fn kept_name(name: &str) -> Option<(u128, &str)> {
    let (stamp, folder) = name.split_once('-')?;
    let digits = !stamp.is_empty() && stamp.bytes().all(|b| b.is_ascii_digit());
    let plain = !matches!(folder, "" | "." | "..") && !folder.contains(['/', '\\', '\0']);
    if !(digits && plain) {
        return None;
    }
    Some((stamp.parse().ok()?, folder))
}

/// `git <args>` in the plane: its stdout, or what it said.
pub(super) fn git(plane: &Path, args: &[&str]) -> Result<String, String> {
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
            return deleted_by_git(cx, level, name, &folder_rel, &toml_rel);
        }
    }
    let kept = keep_here(plane, level, &folder).map_err(|unmoved| match unmoved {
        Unmoved::Nothing(why) => format!("{name} was not deleted: {why}"),
        // Its folder left and could not come back (SK-45): say so, never "not deleted".
        Unmoved::Stranded(what) => format!("{name} was deleted only in part: {what}"),
    })?;
    cx.done(format!(
        "{name} is deleted; it is kept on this machine, and Restore brings it back"
    ));
    cx.commit(&paths, &format!("skill rm {name}"));
    Ok(Kept(Source::Here(kept)))
}

/// The committed skill `name`'s delete, where its commit can land: `git rm`, taken out of the
/// index again so the save stages it with the change, and committed. Put back as `HEAD` has it
/// where no commit was made. A note of it is held locked from before the removal until it ended
/// either way, so a delete cut short is found at the next start ([`interrupted`]).
fn deleted_by_git(
    cx: &mut Ctx<'_>,
    level: &Level,
    name: &str,
    folder_rel: &str,
    toml_rel: &str,
) -> Result<Kept, String> {
    let plane = cx.plane;
    let note = Deleting::begin(plane, level, folder_rel)
        .map_err(|why| format!("{name} was not deleted: {why}"))?;
    let specs = specs_of(plane, folder_rel, toml_rel, "HEAD");
    let mut args = vec!["rm", "-r", "-q", "--"];
    args.extend(specs.iter().map(String::as_str));
    if let Err(why) = git(plane, &args) {
        note.end();
        return Err(format!("{name} was not deleted: {why}"));
    }
    #[cfg(test)]
    cut::at(cut::Step::Removed);
    unstage(plane, &specs);
    #[cfg(test)]
    cut::at(cut::Step::Unstaged);
    let removed = removal_committed(cx, name, folder_rel, toml_rel, &specs);
    #[cfg(test)]
    cut::at(cut::Step::Committed);
    if removed == Err(false) {
        // Neither committed nor put back: the note stays, let go of, so the next start tries
        // again ([`interrupted`]).
        note.left();
    } else {
        note.end();
    }
    match removed {
        Ok(after) => {
            cx.done(format!(
                "{name} is deleted; Restore brings it back from the commit"
            ));
            Ok(Kept(Source::Commit(after)))
        }
        Err(true) => Err(format!(
            "{name} was not deleted: its commit could not be made, so it was put back"
        )),
        Err(false) => Err(format!(
            "{name}'s commit could not be made, and it could not be put back: the project's \
             last commit still has it, and purlis tries again next time"
        )),
    }
}

/// The git pathspecs of a skill's folder at `folder_rel`, and of the file beside it at
/// `toml_rel` where `commit` has it.
fn specs_of(plane: &Path, folder_rel: &str, toml_rel: &str, commit: &str) -> Vec<String> {
    let mut specs = vec![literally(folder_rel)];
    if git(plane, &["cat-file", "-e", &format!("{commit}:{toml_rel}")]).is_ok() {
        specs.push(literally(toml_rel));
    }
    specs
}

/// Takes what is staged of `specs` out of the index, back to what `HEAD` has.
fn unstage(plane: &Path, specs: &[String]) {
    let mut args = vec!["reset", "-q", "--"];
    args.extend(specs.iter().map(String::as_str));
    let _ = git(plane, &args);
}

/// Puts the files of `specs` back in the working tree as `HEAD` has them: whether they are.
fn put_back(plane: &Path, specs: &[String]) -> bool {
    let mut args = vec!["restore", "--source=HEAD", "--worktree", "--"];
    args.extend(specs.iter().map(String::as_str));
    git(plane, &args).is_ok()
}

/// **Commits a skill's removal from the working tree**, not staged, as `skill rm <name>`, and
/// answers the commit. Where no commit was made, what the save staged of it is taken out of the
/// index and the files are put back as `HEAD` has them: `Err(true)`, or `Err(false)` where they
/// could not be.
fn removal_committed(
    cx: &mut Ctx<'_>,
    name: &str,
    folder_rel: &str,
    toml_rel: &str,
    specs: &[String],
) -> Result<String, bool> {
    let plane = cx.plane;
    let before = head(plane);
    cx.commit(
        &[folder_rel.to_owned(), toml_rel.to_owned()],
        &format!("skill rm {name}"),
    );
    let after = head(plane);
    if after.is_empty() || after == before {
        unstage(plane, specs);
        return Err(put_back(plane, specs));
    }
    Ok(after)
}

/// **A delete by git that runs**: its note in this machine's [`DELETING_DIR`], naming the level
/// and the folder, held locked until [`Deleting::end`] takes it away. A crash lets go of the lock
/// and leaves the note, which is how [`interrupted`] tells a delete cut short from one running.
struct Deleting {
    state: PathBuf,
    note: String,
    _held: std::fs::File,
}

impl Deleting {
    /// Writes and locks the note of a delete of the skill at `folder_rel`, at `level`.
    fn begin(plane: &Path, level: &Level, folder_rel: &str) -> Result<Deleting, String> {
        let state = crate::plane::state_dir(plane);
        let folder = folder_rel.rsplit('/').next().unwrap_or(folder_rel);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|at| at.as_nanos())
            .unwrap_or_default();
        let note = format!("{stamp}-{folder}");
        let text = format!("{}\n{folder}\n", level.rel().unwrap_or_default());
        crate::profiletrust::private_dir(&state).map_err(|why| why.to_string())?;
        let held = store::note_begun(&state, DELETING_DIR, &note, &text)
            .map_err(|why| format!("its delete could not be noted ({why})"))?;
        Ok(Deleting {
            state,
            note,
            _held: held,
        })
    }

    /// Takes the note away, then lets go of its lock: the delete ended.
    fn end(self) {
        store::note_ended(&self.state, DELETING_DIR, &self.note);
    }

    /// Lets go of the lock and leaves the note: the delete did not end, and the next start
    /// finishes or undoes it as one a crash cut short.
    fn left(self) {}
}

/// The level and the skill folder a delete's note names, or `None` where it names no level, or
/// no plain folder name.
fn noted(text: &str) -> Option<(Level, String)> {
    let mut lines = text.lines();
    let level = Level::of_rel(lines.next()?.trim())?;
    let folder = lines.next()?.trim();
    let plain = !folder.is_empty()
        && folder != "."
        && folder != ".."
        && !folder.contains(['/', '\\', '\0'])
        && lines.all(|rest| rest.trim().is_empty());
    plain.then(|| (level, folder.to_owned()))
}

/// **Finishes or undoes every delete of a skill a crash, a quit or a failed git cut short**
/// between its removal and its commit (SK-35, #3013), and says which, one line each. Run where
/// the skills are next read for a change: as the Skill studio opens and as `purlis skill` starts
/// a change. A delete still running holds its note locked and is left to end.
///
/// One whose commit landed has nothing left to do. One whose skill is gone from the project is
/// committed where its commit can land now, so Restore brings it back from that commit, as the
/// delete would have. Otherwise its files are put back as the last commit has them. Either way
/// nothing of it is left staged, so the next delete is never kept on this machine for it.
pub fn interrupted(cx: &mut Ctx<'_>) {
    let state = crate::plane::state_dir(cx.plane);
    for (note, held, text) in store::notes_left(&state, DELETING_DIR) {
        let settled = match noted(&text) {
            Some((level, folder)) => match finished_or_undone(cx, &level, &folder) {
                Some(Ended::Settled(line)) => {
                    (cx.say)(Say::Info(line.into()));
                    true
                }
                // Neither finished nor put back: said, and the note is left for the next start.
                Some(Ended::Unsettled(line)) => {
                    (cx.say)(Say::Warn(line.into()));
                    false
                }
                None => true,
            },
            None => {
                tracing::warn!(
                    target: "purlis::skills",
                    "a note of a skill's delete named no skill, and was taken away: {note}"
                );
                true
            }
        };
        if settled {
            store::note_ended(&state, DELETING_DIR, &note);
        }
        drop(held);
    }
}

/// How [`interrupted`] left a delete cut short, in the line it says.
enum Ended {
    /// Finished, or put back: the note goes.
    Settled(String),
    /// Neither: the project's last commit still has the skill, and the note stays for the next
    /// start to try again.
    Unsettled(String),
}

/// How [`interrupted`] finished or undid the delete of the skill in `folder` at `level`, or
/// `None` where its commit had landed.
fn finished_or_undone(cx: &mut Ctx<'_>, level: &Level, folder: &str) -> Option<Ended> {
    let plane = cx.plane;
    let folder_rel = format!("{}/{folder}", level.rel()?);
    let toml_rel = rel(plane, &beside(&plane.join(&folder_rel)));
    let skill_md = format!("HEAD:{folder_rel}/SKILL.md");
    let text = git(plane, &["show", &skill_md]).ok()?;
    let name = name_in(&text).unwrap_or_else(|| folder.to_owned());
    let specs = specs_of(plane, &folder_rel, &toml_rel, "HEAD");
    unstage(plane, &specs);
    if plane
        .join(&folder_rel)
        .join("SKILL.md")
        .symlink_metadata()
        .is_ok()
    {
        return Some(Ended::Settled(format!(
            "{name}'s delete was cut short before it removed anything, so it is still here"
        )));
    }
    let unsettled = || {
        Ended::Unsettled(format!(
            "{name}'s delete was cut short and could not be finished or put back: the project's \
             last commit still has it, and purlis tries again next time"
        ))
    };
    let paths = [folder_rel.clone(), toml_rel.clone()];
    if crate::planegit::paths_unsaved(plane, &paths).is_some() {
        return Some(if put_back(plane, &specs) {
            Ended::Settled(format!(
                "{name}'s delete was cut short and the project cannot save it now, so it was \
                 put back"
            ))
        } else {
            unsettled()
        });
    }
    // The save's own lines go to the log: this says how the delete ended, in one line.
    let mut quiet = |line: Say| tracing::info!(target: "purlis::skills", "{line}");
    let mut inner = Ctx {
        plane,
        built_in: cx.built_in,
        by: cx.by,
        say: &mut quiet,
    };
    Some(
        match removal_committed(&mut inner, &name, &folder_rel, &toml_rel, &specs) {
            Ok(_) => Ended::Settled(format!(
                "{name}'s delete was cut short; it is finished now, and Restore brings it back"
            )),
            Err(true) => Ended::Settled(format!(
                "{name}'s delete was cut short and could not be finished, so it was put back"
            )),
            Err(false) => unsettled(),
        },
    )
}

/// Moves the skill's `folder`, and the file beside it, at `level` into a new folder of this
/// machine's [`DELETED_DIR`] with a `from` naming its level, and answers that folder's name.
fn keep_here(plane: &Path, level: &Level, folder: &Path) -> Result<String, Unmoved> {
    let state = crate::plane::state_dir(plane);
    let folder_is = folder_name(folder);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|at| at.as_millis())
        .unwrap_or_default();
    let kept = format!("{stamp}-{folder_is}");
    let from_level = level.rel().unwrap_or_default();
    // Private to the person, as every folder in the state directory is (`plane-format.md`).
    crate::profiletrust::private_dir(&state).map_err(|why| Unmoved::Nothing(why.to_string()))?;
    store::made(&state, &[DELETED_DIR], true).map_err(|why| Unmoved::Nothing(why.to_string()))?;
    store::made(&state, &[DELETED_DIR, &kept], false)
        .map_err(|why| Unmoved::Nothing(why.to_string()))?;
    store::write_new(
        &state,
        &[DELETED_DIR, &kept],
        KEPT_FROM,
        &format!("{from_level}\n"),
    )
    .map_err(|why| Unmoved::Nothing(why.to_string()))?;
    let into = state.join(DELETED_DIR).join(&kept).join(&folder_is);
    // The folder and the file beside it go together, or neither does (SK-48).
    match move_skill(plane, folder, &state, &into, usize::MAX) {
        Ok(()) => Ok(kept),
        Err(Unmoved::Nothing(why)) => {
            // Nothing moved: the kept folder made for it, and its `from`, are taken away again.
            store::put_away(&state, DELETED_DIR, &kept, KEPT_FROM);
            Err(Unmoved::Nothing(why))
        }
        // Its folder is kept and named by its `from`, and a file of its own left in the project
        // takes nothing (`taken_again`): it is listed, and Restore brings it back.
        Err(Unmoved::Stranded(what)) if into.symlink_metadata().is_ok() => Err(Unmoved::Stranded(
            format!("{what}. It is in this machine's deleted skills: restore it from there"),
        )),
        Err(stranded) => Err(stranded),
    }
}

/// How many deleted skills from git [`deleted`] lists at most, newest first.
const MOST_DELETED: usize = 30;

/// The git-log pathspecs of every shared skill's `SKILL.md`: the project's, each workspace's and
/// each persona's, by the one rule of where a level's skills are ([`purlis_skills::level_rel`]).
pub(in crate::skills) fn shared_globs() -> Vec<String> {
    [
        Whose::Project,
        Whose::Workspace("*".to_owned()),
        Whose::Persona("*".to_owned()),
    ]
    .iter()
    .filter_map(|at| purlis_skills::level_rel(at, false))
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
    let now_ms = ms_since_epoch(std::time::SystemTime::now());
    for (kept, from, folder, text) in store::kept(&state, DELETED_DIR, KEPT_FROM) {
        // Only a kept delete named as a delete names it, and only within its time: one past it
        // is [`expire`]'s and is offered back no more, so no restore meets its removal (SK-33).
        let Some((stamp, _)) = kept_name(&kept) else {
            continue;
        };
        if past(stamp, now_ms, KEPT_HERE_FOR) {
            continue;
        }
        let Some(level) = Level::of_rel(from.trim()) else {
            continue;
        };
        let Some(name) = name_in(&text) else {
            continue;
        };
        // A file beside its folder left in the project (a delete that could not move it) takes
        // its place only where the kept skill has one of its own to bring back over it.
        let toml_kept = beside(&state.join(DELETED_DIR).join(&kept).join(&folder))
            .symlink_metadata()
            .is_ok();
        if taken_again(plane, &level, &name, &folder, toml_kept) {
            continue;
        }
        let at = u64::try_from(stamp / 1000).unwrap_or(u64::MAX);
        out.push(Deleted {
            name,
            level,
            folder,
            kept: Kept(Source::Here(kept)),
            at,
        });
    }
    let globs = shared_globs();
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
            if here_too || taken_again(plane, &level, &name, folder, true) {
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
/// output, an editor's), take nothing: Restore puts the skill back beside them. The file beside
/// the folder there takes it only where `toml_counts`: where the deleted skill would bring one
/// back over it (SK-48).
fn taken_again(plane: &Path, level: &Level, name: &str, folder: &str, toml_counts: bool) -> bool {
    let Some(dir) = level.dir(plane) else {
        return true;
    };
    let at = dir.join(folder);
    let held = at.join("SKILL.md").symlink_metadata().is_ok()
        || (toml_counts && beside(&at).symlink_metadata().is_ok())
        || (at.symlink_metadata().is_ok() && tracked(plane, &rel(plane, &at)));
    held || level.named(plane, name).is_some()
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
    if cx.reserved(name) {
        return Err(format!(
            "{name} cannot be restored under its name: it belongs to a built-in skill now"
        ));
    }
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
            // The folder and the file beside it come back together, or neither does (SK-48).
            move_skill(&state, &from, plane, &folder, deleted.level.depth()).map_err(
                |unmoved| match unmoved {
                    Unmoved::Nothing(why) => format!("{name} was not restored: {why}"),
                    // It left the kept folder and could not go back (SK-45): say so.
                    Unmoved::Stranded(what) => {
                        format!("{name} was restored only in part: {what}")
                    }
                },
            )?;
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

/// **Pins `name` (`on`) or unpins it** for a workspace's or a persona's chats (SK-15, ADR 0102
/// §4), committed. `Ok(false)` where it already was so, and nothing is written.
pub fn pin(cx: &mut Ctx<'_>, by: &super::PinnedBy, name: &str, on: bool) -> Result<bool, String> {
    let changed = super::pin(cx.plane, by, name, on)?;
    if changed {
        let (whose, file) = match by {
            super::PinnedBy::Workspace(ws) => (
                format!("{ws}'s chats"),
                crate::settings::workspace::named(ws),
            ),
            super::PinnedBy::Persona(persona) => (
                format!("chats as {persona}"),
                rel(cx.plane, &crate::personas::def_path(cx.plane, persona)),
            ),
        };
        let verb = if on { "pinned" } else { "unpinned" };
        cx.done(format!("{name} is {verb} for {whose}"));
        let word = if on { "pin" } else { "unpin" };
        cx.commit(&[file], &format!("skill {word} {name} for {whose}"));
    }
    Ok(changed)
}

/// **Share** (#2767): the local skill `name` at the project's root, in a workspace or of a
/// persona (`at`) moved into its level's `skills/`, committed (#2770).
pub fn share(cx: &mut Ctx<'_>, at: &Whose, name: &str) -> Result<PathBuf, String> {
    let plane = cx.plane;
    cx.not_built_in(name)?;
    // What is committed is what the share moved, never a folder looked up again by path.
    let super::Shared { from, to: folder } = super::share_here(plane, at, name)?;
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

/// **This machine's kept deletes, through handles** (#2770): every folder opened one component
/// at a time with `O_NOFOLLOW`, every file made new with `O_EXCL | O_NOFOLLOW`, read only where it
/// is one plain file, and nothing removed but the names a kept delete is made of.
#[cfg(unix)]
pub(in crate::skills) mod store {
    use std::io::{Read as _, Write as _};
    use std::path::Path;

    use rustix::fd::OwnedFd;
    use rustix::fs::{AtFlags, FileType, Mode, OFlags, fstat, mkdirat, openat, unlinkat};

    /// The one opener of a skill's folders, following no link (SK-36): the skills crate's.
    pub(in crate::skills) use purlis_skills::moving::{FOLDER, open_dir};

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

    /// **Writes and locks the note `note`** in the folder `dir` under `state`, made where it is
    /// not there, with `text`, and answers it held locked until it is dropped. Written whole under
    /// a name [`notes_left`] never reads, then renamed to `note` already locked, so a note is never
    /// read half written or unlocked while its delete runs.
    pub(super) fn note_begun(
        state: &Path,
        dir: &str,
        note: &str,
        text: &str,
    ) -> std::io::Result<std::fs::File> {
        made(state, &[dir], true)?;
        let at = open_dir(state, &[dir.to_owned()], usize::MAX)?;
        let writing = format!(".{note}");
        let file = openat(
            &at,
            writing.as_str(),
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o600),
        )?;
        let mut file = std::fs::File::from(file);
        file.lock()?;
        let written = file
            .write_all(text.as_bytes())
            .and_then(|()| file.sync_all())
            .and_then(|()| {
                rustix::fs::renameat(&at, writing.as_str(), &at, note).map_err(Into::into)
            });
        match written {
            Ok(()) => Ok(file),
            Err(why) => {
                let _ = unlinkat(&at, writing.as_str(), AtFlags::empty());
                Err(why)
            }
        }
    }

    /// Takes the note `note` in the folder `dir` under `state` away. The folder stays: another
    /// delete may be about to write its own note in it.
    pub(super) fn note_ended(state: &Path, dir: &str, note: &str) {
        let Ok(at) = open_dir(state, &[dir.to_owned()], usize::MAX) else {
            return;
        };
        let _ = unlinkat(&at, note, AtFlags::empty());
    }

    /// **Every note in the folder `dir` under `state` that nothing holds locked**: its name, its
    /// lock, held now, and its text, of at most a kilobyte. A note still being written that
    /// nothing holds (its writer died) is taken away; a note that is not one plain file with one
    /// name is passed over.
    pub(super) fn notes_left(state: &Path, dir: &str) -> Vec<(String, std::fs::File, String)> {
        let Ok(at) = open_dir(state, &[dir.to_owned()], usize::MAX) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for name in names_in(&at, FileType::RegularFile) {
            let Ok(fd) = openat(
                &at,
                name.as_str(),
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
                Mode::empty(),
            ) else {
                continue;
            };
            let Ok(stat) = fstat(&fd) else {
                continue;
            };
            if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile || stat.st_nlink != 1
            {
                continue;
            }
            let mut file = std::fs::File::from(fd);
            if file.try_lock().is_err() {
                continue;
            }
            // Taken away between the read of the folder and the lock: a delete that ended.
            let still = rustix::fs::statat(&at, name.as_str(), AtFlags::SYMLINK_NOFOLLOW)
                .is_ok_and(|now| now.st_ino == stat.st_ino && now.st_dev == stat.st_dev);
            if !still {
                continue;
            }
            if name.starts_with('.') {
                let _ = unlinkat(&at, name.as_str(), AtFlags::empty());
                continue;
            }
            let mut text = String::new();
            if (&mut file).take(1024).read_to_string(&mut text).is_err() {
                text.clear();
            }
            out.push((name, file, text));
        }
        out
    }

    /// **[`super::expire`] through handles**: every kept delete in `dir` under `state` past
    /// [`super::EXPIRY_AFTER`] at `now_ms`, removed. `state` and `dir` are opened as folders themselves, so a
    /// link at either keeps everything.
    pub(super) fn expire(state: &Path, dir: &str, from: &str, now_ms: u128) -> super::Expired {
        let mut expired = super::Expired::default();
        let Ok(state_dir) = openat(rustix::fs::CWD, state, FOLDER, Mode::empty()) else {
            return expired;
        };
        let Ok(kept) = openat(&state_dir, dir, FOLDER, Mode::empty()) else {
            return expired;
        };
        for name in names_in(&kept, FileType::Directory) {
            let Some((stamp, folder)) = super::kept_name(&name) else {
                continue;
            };
            if !super::past(stamp, now_ms, super::EXPIRY_AFTER) {
                continue;
            }
            if let Err(why) = remove_kept(&kept, &name, folder, from) {
                expired.failed.push((name, why));
                continue;
            }
            expired.removed.push(name);
        }
        expired
    }

    /// Removes the kept delete `name` in `kept`, whose skill's folder is `folder`: only where it
    /// holds nothing but `from`, that folder and the file beside it, each named in text. `from`
    /// goes last, so a removal cut short is still one [`super::expire`] knows and finishes next
    /// time; past its time, no list offers it back meanwhile.
    fn remove_kept(kept: &OwnedFd, name: &str, folder: &str, from: &str) -> Result<(), String> {
        use crate::skills::at;
        let one = openat(kept, name, FOLDER, Mode::empty()).map_err(|why| why.to_string())?;
        let toml = format!("{folder}.toml");
        let entries = at::entries(&one).map_err(|why| why.to_string())?;
        // A name that is not text is one no delete gives, and `at::entries` does not list it.
        let named = rustix::fs::Dir::read_from(&one)
            .map_err(|why| why.to_string())?
            .filter_map(Result::ok)
            .filter(|entry| !matches!(entry.file_name().to_bytes(), b"." | b".."))
            .count();
        if named != entries.len() {
            return Err(
                "it holds a name that is not text, which no delete puts there, so it \
                        was left as it is"
                    .to_owned(),
            );
        }
        let ours = |entry: &at::Entry| {
            (entry.name == folder && entry.kind == FileType::Directory)
                || ((entry.name == from || entry.name == toml)
                    && entry.kind == FileType::RegularFile)
        };
        if let Some(theirs) = entries.iter().find(|entry| !ours(entry)) {
            return Err(format!(
                "it holds {:?}, which no delete puts there, so it was left as it is",
                theirs.name
            ));
        }
        for entry in entries.iter().filter(|entry| entry.name != from) {
            if entry.kind == FileType::Directory {
                at::remove_tree_in(&one, &entry.name).map_err(|why| why.to_string())?;
            } else {
                at::remove_file_in(&one, &entry.name).map_err(|why| why.to_string())?;
            }
        }
        if entries.iter().any(|entry| entry.name == from) {
            at::remove_file_in(&one, from).map_err(|why| why.to_string())?;
        }
        at::remove_empty_folder_in(kept, name).map_err(|why| why.to_string())
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

    pub(super) fn expire(_: &Path, _: &str, _: &str, _: u128) -> super::Expired {
        super::Expired::default()
    }

    // A delete's note where no folder can be held open: by path, written whole under a dot-name
    // and renamed into place already locked, as on Unix, and opened without following a link
    // (on Windows, the reparse point itself).

    /// The options a note is opened with: never through a link where the system can say so.
    fn options() -> std::fs::OpenOptions {
        #[allow(unused_mut)]
        let mut options = std::fs::OpenOptions::new();
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt as _;
            // FILE_FLAG_OPEN_REPARSE_POINT: the link itself, never what it names.
            options.custom_flags(0x0020_0000);
        }
        options
    }

    pub(super) fn note_begun(
        state: &Path,
        dir: &str,
        note: &str,
        text: &str,
    ) -> std::io::Result<std::fs::File> {
        use std::io::Write as _;
        let at = state.join(dir);
        std::fs::create_dir_all(&at)?;
        let writing = at.join(format!(".{note}"));
        let mut file = options().write(true).create_new(true).open(&writing)?;
        let written = file
            .lock()
            .and_then(|()| file.write_all(text.as_bytes()))
            .and_then(|()| file.sync_all())
            .and_then(|()| std::fs::rename(&writing, at.join(note)));
        match written {
            Ok(()) => Ok(file),
            Err(why) => {
                drop(file);
                let _ = std::fs::remove_file(&writing);
                Err(why)
            }
        }
    }

    pub(super) fn note_ended(state: &Path, dir: &str, note: &str) {
        let _ = std::fs::remove_file(state.join(dir).join(note));
    }

    pub(super) fn notes_left(state: &Path, dir: &str) -> Vec<(String, std::fs::File, String)> {
        use std::io::Read as _;
        let Ok(names) = std::fs::read_dir(state.join(dir)) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for one in names.filter_map(Result::ok) {
            let Ok(name) = one.file_name().into_string() else {
                continue;
            };
            // A plain file alone: never a link or a folder.
            if !std::fs::symlink_metadata(one.path()).is_ok_and(|meta| meta.file_type().is_file()) {
                continue;
            }
            let Ok(file) = options().read(true).open(one.path()) else {
                continue;
            };
            if file.try_lock().is_err() {
                continue;
            }
            if name.starts_with('.') {
                drop(file);
                let _ = std::fs::remove_file(one.path());
                continue;
            }
            let mut text = String::new();
            if (&file).take(1024).read_to_string(&mut text).is_err() {
                text.clear();
            }
            out.push((name, file, text));
        }
        out
    }
}

#[cfg(test)]
mod cut;

#[cfg(test)]
mod tests;
