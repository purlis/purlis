// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The Skill studio's changes** (SK-7, #2770): make a skill from a template, open and save it
//! in the light editor with its live lint, rename it, move it between levels, delete it after
//! saying who uses it, restore it, change a project skill's grant and turn one off or on in a
//! workspace. Each is `purlis_core::skills::verbs`' own, which `purlis skill` calls too, and each
//! is one commit. Behind the `skill-studio` flag until its finish line (#2786).

use serde::{Deserialize, Serialize};
use specta::Type;

use purlis_core::repocmd::Say;
use purlis_core::skills::studio::Templates;
use purlis_core::skills::verbs::{self, By, Level, Whose};
use purlis_core::skills::{Grant, SkillTemplate as Offers, Template, TemplateId};

/// A level of the project's, as the studio names one: the project's root, a workspace's or a
/// persona's (SK-32, #3003), shared or kept on this machine (`local`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "at", rename_all = "kebab-case")]
pub enum SkillLevel {
    Project { local: bool },
    Workspace { name: String, local: bool },
    Persona { name: String, local: bool },
}

/// Whose skills a level is, as the studio names it without saying shared or local: what Share
/// takes, as a share is always of a local skill into its own level's shared ones.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "at", rename_all = "kebab-case")]
pub enum SkillWhose {
    Project,
    Workspace { name: String },
    Persona { name: String },
}

impl From<&SkillWhose> for Whose {
    fn from(whose: &SkillWhose) -> Whose {
        match whose {
            SkillWhose::Project => Whose::Project,
            SkillWhose::Workspace { name } => Whose::Workspace(name.clone()),
            SkillWhose::Persona { name } => Whose::Persona(name.clone()),
        }
    }
}

impl From<&SkillLevel> for Level {
    fn from(level: &SkillLevel) -> Level {
        match level {
            SkillLevel::Project { local } => Level::project().local(*local),
            SkillLevel::Workspace { name, local } => Level::workspace(name).local(*local),
            SkillLevel::Persona { name, local } => Level::persona(name).local(*local),
        }
    }
}

impl From<&Level> for SkillLevel {
    fn from(level: &Level) -> SkillLevel {
        let local = level.local;
        match &level.at {
            Whose::Project => SkillLevel::Project { local },
            Whose::Workspace(name) => SkillLevel::Workspace {
                name: name.clone(),
                local,
            },
            Whose::Persona(name) => SkillLevel::Persona {
                name: name.clone(),
                local,
            },
        }
    }
}

/// Where a project skill reaches, as the studio sets it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "to", rename_all = "kebab-case")]
pub enum SkillGrant {
    /// Chats at the project's root alone.
    Root,
    /// Every workspace's chats too.
    All,
    /// The workspaces named.
    Only { workspaces: Vec<String> },
}

impl From<Grant> for SkillGrant {
    fn from(grant: Grant) -> SkillGrant {
        match grant {
            Grant::Root => SkillGrant::Root,
            Grant::All => SkillGrant::All,
            Grant::Only(workspaces) => SkillGrant::Only { workspaces },
        }
    }
}

impl From<SkillGrant> for Grant {
    fn from(grant: SkillGrant) -> Grant {
        match grant {
            SkillGrant::Root => Grant::Root,
            SkillGrant::All => Grant::All,
            SkillGrant::Only { workspaces } => Grant::Only(workspaces),
        }
    }
}

/// Which template a new skill starts from, as the window names it: one of purlis's own, or one
/// an extension offers, by its id and the template's (ADR 0101).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum SkillTemplateId {
    Blank,
    Steps,
    Extension { extension: String, template: String },
}

impl From<SkillTemplateId> for TemplateId {
    fn from(id: SkillTemplateId) -> TemplateId {
        match id {
            SkillTemplateId::Blank => TemplateId::BuiltIn(Template::Blank),
            SkillTemplateId::Steps => TemplateId::BuiltIn(Template::Steps),
            SkillTemplateId::Extension {
                extension,
                template,
            } => TemplateId::Extension {
                extension,
                template,
            },
        }
    }
}

impl From<TemplateId> for SkillTemplateId {
    fn from(id: TemplateId) -> SkillTemplateId {
        match id {
            TemplateId::BuiltIn(Template::Blank) => SkillTemplateId::Blank,
            TemplateId::BuiltIn(Template::Steps) => SkillTemplateId::Steps,
            TemplateId::Extension {
                extension,
                template,
            } => SkillTemplateId::Extension {
                extension,
                template,
            },
        }
    }
}

/// One template, as the New skill form offers it: purlis's own, then each extension's that
/// declares `skill-template` (ADR 0101).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct SkillTemplate {
    pub id: SkillTemplateId,
    pub label: String,
    pub about: String,
}

/// What a change had to say beyond being done: that it was left for the next save, or that a
/// place naming the skill was left as it was. Empty for a change that only did what was asked.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Type)]
pub struct SkillChanged {
    pub notes: Vec<String>,
}

/// Who uses a skill, as Delete shows before it deletes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct SkillUsers {
    /// Whether chats at the project's root get it.
    pub root: bool,
    pub workspaces: Vec<String>,
    /// The persona whose own skill it is: its chats get it in every workspace.
    pub persona: Option<String>,
    pub personas: Vec<String>,
    pub skills: Vec<String>,
}

/// A deleted skill Restore can bring back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct DeletedSkill {
    /// What Restore is handed back: the core's own record of it, whole, which the window holds
    /// and never reads.
    pub id: String,
    pub name: String,
    pub level: SkillLevel,
    /// Its level as a person reads it: `project`, `local workspace ops`.
    pub label: String,
}

/// A level a skill can be moved to or made at, and its words.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct StudioLevel {
    pub level: SkillLevel,
    /// As a person reads it: `project`, `local workspace ops` (the core's `Level`).
    pub label: String,
}

/// Every level of the project at `root`, in the core's order, with its words.
pub(crate) fn levels(root: &std::path::Path) -> Vec<StudioLevel> {
    Level::every(root)
        .iter()
        .map(|level| StudioLevel {
            level: level.into(),
            label: level.to_string(),
        })
        .collect()
}

/// Runs `change` with a context of the project at `root`, asked from the window, and answers
/// the lines worth showing: what was not done, or left. A `Done` line is the command line's.
pub(crate) fn heard<T>(
    root: &std::path::Path,
    change: impl FnOnce(&mut verbs::Ctx<'_>) -> Result<T, String>,
) -> Result<SkillChanged, String> {
    let mut notes = Vec::new();
    let mut say = |line: Say| match line {
        Say::Info(text) | Say::Warn(text) | Say::Fail(text) => notes.push(text.to_string()),
        Say::Done(_) | Say::Plain(_) | Say::Out(_) => {}
    };
    let built_in = built_in();
    change(&mut verbs::Ctx {
        plane: root,
        built_in: built_in.as_deref(),
        by: By::Window,
        say: &mut say,
    })?;
    Ok(SkillChanged { notes })
}

/// Refused while the flag is off.
pub(crate) fn flag_on() -> Result<(), String> {
    if purlis_core::skills::studio_on_here() {
        Ok(())
    } else {
        Err("The Skill studio is not turned on on this machine.".to_owned())
    }
}

/// The built-in skills' directory of the plugin this app carries.
pub(crate) fn built_in() -> Option<std::path::PathBuf> {
    crate::bundled_plugin_here().and_then(|plugin| purlis_core::skills::in_bundle(&plugin))
}

/// The templates a new skill can start from in the project `plane`: purlis's own, then each
/// extension's (ADR 0101).
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_templates(
    views: tauri::State<'_, crate::views::Views>,
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
) -> Result<Vec<SkillTemplate>, String> {
    let held = planes.held(&plane)?;
    let executor = views.executor();
    crate::off_the_window("reading the templates", move || {
        Ok(purlis_core::skills::studio::with_studio(
            &executor,
            held.root(),
            |studio| templates(&Templates { studio }),
        ))
    })
    .await
}

/// [`skill_templates`]' list, for the window.
fn templates(templates: &dyn Offers) -> Vec<SkillTemplate> {
    templates
        .offered()
        .into_iter()
        .map(|offer| SkillTemplate {
            id: offer.id.into(),
            label: offer.label,
            about: offer.about,
        })
        .collect()
}

/// How much a lint's finding stops, as the editor draws it: the core's `Severity`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum SkillLintLevel {
    /// The skill is not whole: Save waits until it is.
    Blocks,
    /// Worth knowing, and Save does not wait on it.
    Warns,
}

/// One line of the live lint (SK-8, #2771).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct SkillLintFinding {
    pub level: SkillLintLevel,
    /// What it says, the core's sentence.
    pub says: String,
}

/// **The live lint** of `text`, a `SKILL.md` being edited for the skill `name` at `level` of
/// `plane` (SK-7; SK-8, #2771): each line the editor shows, checked against the project's skills
/// and this machine's programs. Save waits only on one that blocks.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_lint(
    views: tauri::State<'_, crate::views::Views>,
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    level: SkillLevel,
    name: String,
    text: String,
) -> Result<Vec<SkillLintFinding>, String> {
    let held = planes.held(&plane)?;
    let executor = views.executor();
    crate::off_the_window("linting the skill", move || {
        // purlis's own lints, then each extension's that declares `skill-lint`, through the one
        // interface (ADR 0101).
        Ok(purlis_core::skills::studio::with_studio(
            &executor,
            held.root(),
            |studio| {
                let lints = studio.map(|studio| studio.lints()).unwrap_or_default();
                let more: Vec<&dyn purlis_core::skills::SkillLint> = lints
                    .iter()
                    .map(|lint| lint as &dyn purlis_core::skills::SkillLint)
                    .collect();
                findings(held.root(), &(&level).into(), &name, &text, &more)
            },
        ))
    })
    .await
}

/// [`skill_lint`]'s findings, for the window: purlis's own lints and `more`.
fn findings(
    root: &std::path::Path,
    level: &Level,
    name: &str,
    text: &str,
    more: &[&dyn purlis_core::skills::SkillLint],
) -> Vec<SkillLintFinding> {
    purlis_core::skills::lint_with(root, level, name, text, more)
        .into_iter()
        .map(|finding| SkillLintFinding {
            level: match finding.severity {
                purlis_core::skills::Severity::Blocks => SkillLintLevel::Blocks,
                purlis_core::skills::Severity::Warns => SkillLintLevel::Warns,
            },
            says: finding.to_string(),
        })
        .collect()
}

/// **Makes a skill** from `template` at `level`.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_new(
    views: tauri::State<'_, crate::views::Views>,
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    level: SkillLevel,
    name: String,
    description: String,
    template: SkillTemplateId,
) -> Result<SkillChanged, String> {
    let held = planes.held(&plane)?;
    let executor = views.executor();
    crate::off_the_window("making the skill", move || {
        flag_on()?;
        purlis_core::skills::studio::with_studio(&executor, held.root(), |studio| {
            heard(held.root(), |cx| {
                verbs::create(
                    cx,
                    &(&level).into(),
                    &name,
                    &description,
                    &Templates { studio },
                    &template.clone().into(),
                )
            })
        })
    })
    .await
}

/// The text of a skill's `SKILL.md`, which the light editor opens.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_text(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    level: SkillLevel,
    name: String,
) -> Result<String, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reading the skill", move || {
        flag_on()?;
        verbs::text_of(held.root(), &(&level).into(), &name).map(|(_, text)| text)
    })
    .await
}

/// **Saves an edit**: `now` as the skill's `SKILL.md`, only while it is still `was`, the text the
/// editor opened.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_save(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    level: SkillLevel,
    name: String,
    was: String,
    now: String,
) -> Result<SkillChanged, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("saving the skill", move || {
        flag_on()?;
        heard(held.root(), |cx| {
            verbs::edit(cx, &(&level).into(), &name, &was, &now)
        })
    })
    .await
}

/// **Renames a skill**, and every persona and skill that names it.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_rename(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    level: SkillLevel,
    name: String,
    to: String,
) -> Result<SkillChanged, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("renaming the skill", move || {
        flag_on()?;
        heard(held.root(), |cx| {
            verbs::rename(cx, &(&level).into(), &name, &to)
        })
    })
    .await
}

/// Whether moving a skill from `from` to `to` asks for a grant: it becomes a project skill.
#[tauri::command]
#[specta::specta]
pub(crate) fn skill_move_needs_grant(from: SkillLevel, to: SkillLevel) -> bool {
    verbs::needs_grant(&(&from).into(), &(&to).into())
}

/// **Moves a skill** from `from` to `to`, with `grant` where it becomes a project skill.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_move(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    from: SkillLevel,
    name: String,
    to: SkillLevel,
    grant: Option<SkillGrant>,
) -> Result<SkillChanged, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("moving the skill", move || {
        flag_on()?;
        let grant: Option<Grant> = grant.map(Into::into);
        heard(held.root(), |cx| {
            verbs::move_to(cx, &(&from).into(), &name, &(&to).into(), grant.as_ref())
        })
    })
    .await
}

/// **Who uses a skill**: what Delete shows first.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_users(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    level: SkillLevel,
    name: String,
) -> Result<SkillUsers, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reading who uses the skill", move || {
        flag_on()?;
        let users = verbs::users(held.root(), &(&level).into(), &name)?;
        Ok(SkillUsers {
            root: users.root,
            workspaces: users.workspaces,
            persona: users.persona,
            personas: users.personas,
            skills: users.skills,
        })
    })
    .await
}

/// **Deletes a skill**, so Restore can bring it back.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_delete(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    level: SkillLevel,
    name: String,
) -> Result<SkillChanged, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("deleting the skill", move || {
        flag_on()?;
        heard(held.root(), |cx| verbs::delete(cx, &(&level).into(), &name))
    })
    .await
}

/// What became of the skill deletes a crash or a quit cut short, one line each (SK-35, #3013).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Type)]
pub struct DeletesCutShort {
    /// Finished, or put back.
    pub settled: Vec<String>,
    /// Neither finished nor put back: tried again the next time the studio opens.
    pub unsettled: Vec<String>,
}

/// **Finishes or undoes every skill delete a crash or a quit cut short** (SK-35, #3013), as the
/// studio opens, and answers one line for each.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_deletes_cut_short(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
) -> Result<DeletesCutShort, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("finishing a skill delete", move || {
        flag_on()?;
        Ok(cut_short(held.root()))
    })
    .await
}

/// [`skill_deletes_cut_short`] in the project at `root`: the core's lines, a warning being one
/// it could neither finish nor undo.
pub(crate) fn cut_short(root: &std::path::Path) -> DeletesCutShort {
    let mut ended = DeletesCutShort::default();
    let mut say = |line: Say| match line {
        Say::Warn(text) | Say::Fail(text) => ended.unsettled.push(text.to_string()),
        Say::Info(text) => ended.settled.push(text.to_string()),
        Say::Done(_) | Say::Plain(_) | Say::Out(_) => {}
    };
    let built_in = built_in();
    verbs::interrupted(&mut verbs::Ctx {
        plane: root,
        built_in: built_in.as_deref(),
        by: By::Window,
        say: &mut say,
    });
    ended
}

/// The deleted skills Restore can bring back, newest first.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skills_deleted(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
) -> Result<Vec<DeletedSkill>, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("reading the deleted skills", move || {
        flag_on()?;
        verbs::deleted(held.root())
            .into_iter()
            .map(|one| {
                Ok(DeletedSkill {
                    id: serde_json::to_string(&one)
                        .map_err(|_| format!("{} could not be listed to restore", one.name))?,
                    label: one.level.to_string(),
                    level: (&one.level).into(),
                    name: one.name,
                })
            })
            .collect()
    })
    .await
}

/// **Restores** the deleted skill `id` names: the core's record of it, as [`skills_deleted`]
/// handed it out.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_restore(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    id: String,
) -> Result<SkillChanged, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("restoring the skill", move || {
        flag_on()?;
        let deleted: verbs::Deleted = serde_json::from_str(&id)
            .map_err(|_| "That deleted skill is not one this project lists.".to_owned())?;
        heard(held.root(), |cx| verbs::restore(cx, &deleted))
    })
    .await
}

/// **Sets where the project skill `name` reaches.**
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_grant(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    name: String,
    grant: SkillGrant,
) -> Result<SkillChanged, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("granting the skill", move || {
        flag_on()?;
        let grant: Grant = grant.into();
        heard(held.root(), |cx| verbs::grant(cx, &name, &grant))
    })
    .await
}

/// **Turns the project skill `name` off, or back on, in `workspace`.** A refused "on" names the
/// grant that would reach the workspace.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_turn(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    workspace: String,
    name: String,
    on: bool,
) -> Result<SkillChanged, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("turning the skill on or off", move || {
        flag_on()?;
        heard(held.root(), |cx| verbs::switch(cx, &workspace, &name, on))
    })
    .await
}

/// **Whose pins a pin is** (SK-15d, #3232): a workspace's or a persona's, by name. The core's
/// `PinnedBy`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "at", rename_all = "kebab-case")]
pub enum SkillPinner {
    Workspace { name: String },
    Persona { name: String },
}

impl From<&SkillPinner> for purlis_core::skills::PinnedBy {
    fn from(by: &SkillPinner) -> purlis_core::skills::PinnedBy {
        match by {
            SkillPinner::Workspace { name } => {
                purlis_core::skills::PinnedBy::Workspace(name.clone())
            }
            SkillPinner::Persona { name } => purlis_core::skills::PinnedBy::Persona(name.clone()),
        }
    }
}

impl From<purlis_core::skills::PinnedBy> for SkillPinner {
    fn from(by: purlis_core::skills::PinnedBy) -> SkillPinner {
        match by {
            purlis_core::skills::PinnedBy::Workspace(name) => SkillPinner::Workspace { name },
            purlis_core::skills::PinnedBy::Persona(name) => SkillPinner::Persona { name },
        }
    }
}

/// **Pins `name` for `by`'s chats, or unpins it** (SK-15d, #3232): the core's `verbs::pin`,
/// which `purlis skill pin` calls too, one commit.
pub(crate) fn pin(
    root: &std::path::Path,
    by: &SkillPinner,
    name: &str,
    on: bool,
) -> Result<SkillChanged, String> {
    let by = by.into();
    heard(root, |cx| verbs::pin(cx, &by, name, on))
}

/// **Pins the skill `name` for a workspace's or a persona's chats, or unpins it**, so no router
/// hides it from them. One commit.
#[tauri::command]
#[specta::specta]
pub(crate) async fn skill_pin(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    by: SkillPinner,
    name: String,
    on: bool,
) -> Result<SkillChanged, String> {
    let held = planes.held(&plane)?;
    crate::off_the_window("pinning the skill", move || {
        flag_on()?;
        pin(held.root(), &by, &name, on)
    })
    .await
}

/// **Shares** a local skill: commits it to the project at its level (#2767), one commit.
pub(crate) fn share(root: &std::path::Path, at: &Whose, name: &str) -> Result<(), String> {
    heard(root, |cx| verbs::share(cx, at, name)).map(|_| ())
}

#[cfg(test)]
mod tests;
