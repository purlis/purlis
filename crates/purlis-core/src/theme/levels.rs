// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The theme levels** (spec #2313, #2323): where themes are kept, where the person's choices
//! and the committed defaults are kept, and which one the window draws.
//!
//! # Theme folders at three levels
//!
//! - **the person's own**: `themes/` in purlis's config folder on this machine, never committed;
//! - **the project's**: `themes/` at the plane root, committed by `purlis save`;
//! - **a workspace's**: `workspaces/<ws>/themes/`, committed while the workspace is LIVE and kept
//!   in this clone while it is LOCAL.
//!
//! A theme folder is named by its folder's name, and the nearest level that has a folder of that
//! name is the one read: the workspace's, then the project's, then the person's. A theme may
//! `extend` a folder of any of the three. Each `themes/` folder is read only when neither it nor
//! anything between its level's root and it is a link ([`ThemesFolder`]).
//!
//! # The precedence, nearest first
//!
//! At each level the person's choice beats the committed default:
//!
//! 1. the person's choice for this workspace on this machine;
//! 2. the workspace's committed default (LIVE workspaces only);
//! 3. the person's choice for the project;
//! 4. the project's committed default;
//! 5. the person's machine default;
//! 6. Purlis, the default ([`super::DEFAULT`], #3034).
//!
//! **The plane root resolves from level 3 down**: it is on no workspace.
//!
//! A choice names a theme and, optionally, which of its appearances: one by its id, or `system`,
//! which follows the operating system with any theme that has a dark and a light appearance.
//! The person's choices are kept in machine config keyed by project id ([`super::choices`]); the
//! committed defaults are the project manifest's `[theme]` and a LIVE workspace's
//! `settings.theme`. Level 5, with no machine default chosen, is the person's own `theme.json`,
//! as before; level 6 is the first theme in force from an extension the project has on, else
//! Purlis.
//!
//! **A choice of a theme folder names the folder's level** (#2516), so that level's folder of
//! the name is drawn and never a folder of the name at another level; a choice made before
//! choices named one reads the nearest level within reach that has the name, as before, and
//! gains its level when it is next written. **A choice carries option values** (#2527): the
//! theme it chose is drawn with them, built onto the theme before it is drawn
//! ([`Theme::with`]), so the consent floors hold over every value; an option it names no value
//! for draws the theme's default. The values are the choice's own, as its appearance is: the
//! nearest level that chose draws its values, never another level's.
//!
//! A choice reaches theme folders of its own level and the levels farther out, never a nearer
//! one ([`super::folders`]). **A choice that names no theme folder within its reach, nor any
//! other theme purlis has, is passed over with a sentence and the next level's is used**, as a
//! value that is not a pick always was. A choice of a theme that is there and cannot be drawn
//! (a refused folder, an extension's theme that is not in force) draws Purlis with a sentence
//! saying why: what the person or the team chose is what they see named.

use std::path::Path;
use std::sync::Arc;

use super::choices::{self, Choices};
pub(crate) use super::folders::plane_fonts;
pub use super::folders::{Folders, Found, FoundFolder, MOST_FOLDERS, Reach, THEMES, ThemesFolder};
use super::resolve::{Drawn, Offered, On, follow};
use super::{
    Appearance, Complaint, Contrast, PURLIS, Theme, Values, built_in, classic, default_for,
    default_theme, shipped,
};
use crate::extension::project::theme::{Pick, SYSTEM, Said, is_appearance};
use crate::settings::Source;

/// **Where a theme drawn came from**: the level of the precedence that chose it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Level {
    /// 1: the person's choice for this workspace on this machine.
    WorkspaceChoice,
    /// 2: the workspace's committed default, LIVE workspaces only.
    WorkspaceDefault,
    /// 3: the person's choice for the project.
    ProjectChoice,
    /// 4: the project's committed default.
    ProjectDefault,
    /// 5: the person's machine default.
    MachineDefault,
    /// 6: nothing chose: Purlis.
    Purlis,
}

impl Level {
    /// How far out a choice made at this level reaches for theme folders.
    pub fn reach(self) -> Reach {
        match self {
            Self::WorkspaceChoice | Self::WorkspaceDefault => Reach::Workspace,
            Self::ProjectChoice | Self::ProjectDefault => Reach::Project,
            Self::MachineDefault | Self::Purlis => Reach::Machine,
        }
    }
}

/// **A choice of theme**: a theme, by what a pick names it (a theme folder's name, an
/// extension's `<extension>/<theme>`, Purlis Classic by its id or an old one, or `system`);
/// which of its appearances, by its id or `system`, none drawing the theme's first; for a theme
/// folder, the level of the folder it picked (#2516); and the value of each of the theme's
/// options it chose (#2527), every other option drawing its default.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Choice {
    pub theme: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub appearance: Option<String>,
    /// **The level of the theme folder it picked** (#2516): that level's folder of the name is
    /// drawn and no other. `None` for a choice of no folder, and for a choice made before
    /// choices named a level, which reads the nearest level within reach that has a folder of
    /// the name, as it always did, and gains its level the next time it is written
    /// ([`Self::placed`]).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<Reach>,
    /// **The value of each option it chose** (#2527), by the option's id: drawn over the theme
    /// with [`Theme::with`], each option it names no value for drawing its default.
    #[serde(skip_serializing_if = "Values::is_empty")]
    pub options: Values,
}

impl Choice {
    /// A choice of `theme` in `appearance`, or `None` when either is not a value a choice holds.
    pub fn new(theme: &str, appearance: Option<&str>) -> Option<Self> {
        Pick::parse(theme)?;
        if appearance.is_some_and(|it| !is_appearance(it)) {
            return None;
        }
        Some(Self {
            theme: theme.to_owned(),
            appearance: appearance.map(str::to_owned),
            from: None,
            options: Values::new(),
        })
    }

    /// **The choice of the theme folder at `level`** (#2516); a choice of anything else names no
    /// level, so it is unchanged.
    #[must_use]
    pub fn at(mut self, level: Reach) -> Self {
        self.from = matches!(Pick::parse(&self.theme), Some(Pick::Folder(_))).then_some(level);
        self
    }

    /// The choice with `options` as its option values.
    #[must_use]
    pub fn with_options(mut self, options: Values) -> Self {
        self.options = options;
        self
    }

    /// **The choice as it is written** (#2516): a choice of a theme folder that names no level
    /// gains the level a choice made at `reach` reads it from now, the nearest within reach
    /// that has a folder of the name in `folders`. A choice that already names one, a choice of
    /// no folder, and one of a name no level within reach has are unchanged.
    #[must_use]
    pub fn placed(self, folders: &Folders, reach: Reach) -> Self {
        let Some(Pick::Folder(name)) = Pick::parse(&self.theme) else {
            return self;
        };
        if self.from.is_some() {
            return self;
        }
        match folders
            .named(reach)
            .into_iter()
            .find(|(_, one)| *one == name)
        {
            Some((level, _)) => self.at(level),
            None => self,
        }
    }

    /// **Whether this choice and `other` pick the same theme**: the same name, and for a theme
    /// folder the same level, where both name one.
    pub fn same_theme(&self, other: &Self) -> bool {
        self.theme == other.theme
            && (self.from.is_none() || other.from.is_none() || self.from == other.from)
    }

    /// **What the window is told this choice draws**, as `project_theme_drawn` answers: a pick
    /// the window can draw in the same frame (an appearance of Purlis Classic, by the id a pick
    /// names it by) or any other theme's id, Purlis's included, which the window asks the engine
    /// to resolve.
    pub fn told(&self) -> String {
        let Some(Pick::BuiltIn(id)) = Pick::parse(&self.theme) else {
            return self.theme.clone();
        };
        if id == PURLIS {
            return self.theme.clone();
        }
        match self.appearance.as_deref() {
            None => id.to_owned(),
            Some(SYSTEM) => SYSTEM.to_owned(),
            Some(wanted) => classic()
                .appearances
                .iter()
                .find(|one| one.id == wanted)
                .map_or_else(|| id.to_owned(), |one| one.name.clone()),
        }
    }
}

/// **What each level chose**, the nearest first. A level that chose nothing is `None`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Levels {
    pub workspace_choice: Option<Choice>,
    pub workspace_default: Option<Choice>,
    pub project_choice: Option<Choice>,
    pub project_default: Option<Choice>,
    pub machine_default: Option<Choice>,
}

impl Levels {
    /// The nearest level that chose, and its choice; `None` when no level did.
    pub fn chosen(&self) -> Option<(Level, &Choice)> {
        self.each().next()
    }

    /// The choice the level `level` made, when it made one.
    pub fn of_level(&self, level: Level) -> Option<&Choice> {
        self.each()
            .find(|(one, _)| *one == level)
            .map(|(_, choice)| choice)
    }

    /// Every level that chose, nearest first.
    pub fn each(&self) -> impl Iterator<Item = (Level, &Choice)> {
        [
            (Level::WorkspaceChoice, &self.workspace_choice),
            (Level::WorkspaceDefault, &self.workspace_default),
            (Level::ProjectChoice, &self.project_choice),
            (Level::ProjectDefault, &self.project_default),
            (Level::MachineDefault, &self.machine_default),
        ]
        .into_iter()
        .filter_map(|(level, choice)| choice.as_ref().map(|choice| (level, choice)))
    }

    /// **The levels as this machine and the plane at `plane` hold them**, in `workspace` when
    /// one is named: the plane root (`None`) resolves from level 3 down. The person's choices
    /// are read from machine config by the project's id; the committed defaults from the
    /// project's manifest and, for a LIVE workspace, its `workspace.json`.
    ///
    /// **Today's in-plane picks are the person's choice where machine config has none**: the
    /// project's `charter.local.toml` `[theme]` is level 3, and a LOCAL workspace's
    /// `settings.theme` is level 1, since neither is committed. [`adopt`] copies them into
    /// machine config once, so they survive a re-clone.
    pub fn read(config_root: Option<&Path>, plane: Option<&Path>, workspace: Option<&str>) -> Self {
        let held = config_root.map(choices::read).unwrap_or_default();
        let Some(root) = plane else {
            return Self::of(&held, None);
        };
        let key = project_key(root);
        let said = Said::read_in(root, workspace);
        let workspace =
            workspace.map(|name| (name, crate::wscmd::live_workspaces(root).contains(name)));
        Self::of(
            &held,
            Some(Project {
                key: key.as_deref(),
                said: &said,
                workspace,
            }),
        )
    }

    /// [`Self::read`] of what was read: the person's choices, and the project in front, when
    /// there is one.
    pub fn of(held: &Choices, project: Option<Project<'_>>) -> Self {
        let machine_default = held.machine.clone();
        let Some(project) = project else {
            return Self {
                machine_default,
                ..Self::default()
            };
        };
        let said = project.said;
        let mine = project.key.and_then(|key| held.projects.get(key));
        let mut levels = Self {
            project_choice: mine
                .and_then(|one| one.project.clone())
                .or_else(|| said_choice(said, Source::Local)),
            project_default: said_choice(said, Source::Shared),
            machine_default,
            ..Self::default()
        };
        if let Some((name, live)) = project.workspace {
            let in_workspace = said_choice(said, Source::Workspace);
            let chosen = mine.and_then(|one| one.workspaces.get(name).cloned());
            if live {
                levels.workspace_choice = chosen;
                levels.workspace_default = in_workspace;
            } else {
                levels.workspace_choice = chosen.or(in_workspace);
            }
        }
        levels
    }
}

/// The project in front, as [`Levels::of`] reads it.
#[derive(Debug, Clone, Copy)]
pub struct Project<'a> {
    /// The key the person's choices for it are kept under ([`project_key`]).
    pub key: Option<&'a str>,
    /// What its manifests say, in the workspace when there is one.
    pub said: &'a Said,
    /// The workspace in front, and whether it is LIVE; `None` at the plane root.
    pub workspace: Option<(&'a str, bool)>,
}

/// The choice the layer `source` of `said` makes: a theme it picks, in the appearance it says,
/// with the option values it names (#2707). An appearance or option values with no theme choose
/// nothing.
fn said_choice(said: &Said, source: Source) -> Option<Choice> {
    let (theme, appearance) = said.picked_in(source);
    let theme = theme?;
    Choice::new(theme, appearance.filter(|it| is_appearance(it)))
        .map(|choice| choice.with_options(said.options_in(source)))
}

/// **The key the person's choices for the project at `root` are kept under**: its stable id
/// where its manifest keeps one (`[project] id`, committed, so it is the same in every clone),
/// otherwise the id purlis holds it by, derived from where it is. `None` when neither can be
/// had.
pub fn project_key(root: &Path) -> Option<String> {
    crate::scaffold::planefile::id_to_hold_by(root).ok()
}

/// What [`resolve_levels`] decides from.
#[derive(Debug, Clone, Copy)]
pub struct LevelInputs<'a> {
    /// What each level chose.
    pub levels: &'a Levels,
    /// The levels' theme folders.
    pub folders: &'a Folders,
    /// The person's own `theme.json`, when one is in force: level 5 when no machine default
    /// was chosen.
    pub theirs: Option<&'a Theme>,
    /// Whose extension themes may be drawn.
    pub on: On<'a>,
    /// The operating system's appearance right now.
    pub system: Appearance,
    /// The contrast the operating system asks for right now (#2328): more draws a theme's
    /// appearance tagged high contrast of the tag drawn, when it has one, wherever no appearance
    /// was chosen by its id.
    pub contrast: Contrast,
}

/// **Resolves the theme the window draws** from the six levels. `offered` is every theme
/// approved extensions contribute, asked for only when the answer needs it.
pub fn resolve_levels(inputs: &LevelInputs<'_>, offered: impl FnOnce() -> Arc<[Offered]>) -> Drawn {
    let mut survey = Some(offered);
    let mut kept: Option<Arc<[Offered]>> = None;
    let mut offered = || -> Arc<[Offered]> {
        if let Some(ask) = survey.take() {
            kept = Some(ask());
        }
        kept.clone().unwrap_or_else(|| Arc::from(Vec::new()))
    };
    let mut passed = Vec::new();
    for (level, choice) in inputs.levels.each() {
        match drawn_for(choice, level.reach(), inputs, &mut offered) {
            Ok(drawn) => {
                return Drawn {
                    level: Some(level),
                    complaints: [passed, drawn.complaints.clone()].concat(),
                    ..drawn
                };
            }
            // A folder no level within reach has: the next level's choice is used.
            Err(said) => {
                passed.extend(said);
                passed.push(Complaint::said(format!(
                    "{} names no theme folder or theme purlis has, so it is passed over and the \
                     next level's choice is used",
                    choice.theme
                )));
            }
        }
    }
    let drawn = if let Some(theirs) = inputs.theirs {
        Drawn {
            level: Some(Level::MachineDefault),
            theirs: true,
            ..Drawn::of(theirs.first_for(inputs.contrast))
        }
    } else {
        // Nothing chose: the first theme in force from an extension the project has on, as
        // before the levels, else Purlis.
        let offered = offered();
        let first = offered
            .iter()
            .filter(|one| inputs.on.has(&one.extension))
            .find(|one| !one.theme.refused);
        let drawn = match first {
            Some(one) => Drawn {
                complaints: one.theme.complaints.clone(),
                ..Drawn::of(one.theme.theme.first_for(inputs.contrast))
            },
            None => Drawn::of(default_for(inputs.contrast)),
        };
        Drawn {
            level: Some(Level::Purlis),
            ..drawn
        }
    };
    Drawn {
        complaints: [passed, drawn.complaints.clone()].concat(),
        ..drawn
    }
}

/// What `choice`, made at a level that reaches `reach`, draws: its theme in its appearance with
/// its option values, or Purlis and why; or, when it names a theme folder no level within reach
/// has, what reading the levels had to say.
///
/// **The option values are drawn onto the theme before it is drawn** ([`Theme::with`], then
/// [`appear`], which holds the consent floors): a value is never layered over a theme already
/// drawn, so every combination of values is held to the floors.
fn drawn_for(
    choice: &Choice,
    reach: Reach,
    inputs: &LevelInputs<'_>,
    offered: &mut impl FnMut() -> Arc<[Offered]>,
) -> Result<Drawn, Vec<Complaint>> {
    let appearance = choice.appearance.as_deref();
    // `theme` in the choice's appearance, drawn with its option values, after what reading it
    // said (`before`).
    let valued_in = |theme: &Theme, appearance: Option<&str>, before: Vec<Complaint>| {
        let (theme, left_out) = theme.with(&choice.options);
        let drawn = appear(&theme, appearance, inputs.system, inputs.contrast);
        Drawn {
            complaints: [before, left_out, drawn.complaints.clone()].concat(),
            ..drawn
        }
    };
    let valued = |theme: &Theme, before: Vec<Complaint>| valued_in(theme, appearance, before);
    Ok(match Pick::parse(&choice.theme) {
        // The old id names a theme as any id does (#2615): the person's own theme, else Purlis
        // Classic, following the system unless the choice names an appearance. A theme of theirs
        // with one appearance stays in it, and Settings › Theme says so.
        Some(Pick::System) => Drawn {
            theirs: inputs.theirs.is_some(),
            ..valued_in(
                inputs.theirs.unwrap_or_else(|| classic()),
                appearance.or(Some(SYSTEM)),
                Vec::new(),
            )
        },
        // Purlis Classic by an id one of its appearances has had draws that appearance; Purlis,
        // and either by an appearance chosen, are drawn as any theme is.
        Some(Pick::BuiltIn(id)) => match appearance {
            // An appearance of Purlis Classic by the id a pick names it by.
            None if id != PURLIS => {
                let (theme, left_out) = classic().with(&choice.options);
                let drawn = match built_in(id) {
                    Some(one) => Drawn::of(
                        theme
                            .appearances
                            .iter()
                            .find(|valued| valued.id == one.id)
                            .unwrap_or(one),
                    ),
                    None => Drawn::of(default_theme()),
                };
                Drawn {
                    complaints: left_out,
                    ..drawn
                }
            }
            _ => valued(shipped(id).unwrap_or_else(classic), Vec::new()),
        },
        Some(Pick::Extension { id, name }) => {
            let offered = offered();
            let found = offered
                .iter()
                .filter(|one| inputs.on.has(&one.extension))
                .find(|one| one.extension == id && one.name == name);
            match found {
                None => cannot(format!(
                    "{id}/{name} is not a theme in force here, so Purlis is drawn"
                )),
                Some(one) if one.theme.refused => Drawn {
                    complaints: one.theme.complaints.clone(),
                    refused: true,
                    ..Drawn::of(default_theme())
                },
                Some(one) => valued(&one.theme.theme, one.theme.complaints.clone()),
            }
        }
        Some(Pick::Folder(name)) => {
            let found = match choice.from {
                // A level nearer than the choice reaches is never its to draw.
                Some(level) if level < reach => {
                    return Err(vec![Complaint::said(format!(
                        "{name} is chosen from {}'s theme folders, which a choice here does not \
                         reach",
                        level.word()
                    ))]);
                }
                // The folder of that level, and no other of its name.
                Some(level) => inputs.folders.find_in(&name, level),
                // A choice made before choices named a level: the nearest within reach.
                None => inputs.folders.find(&name, reach),
            };
            match found {
                Found::Missing(said) => return Err(said),
                Found::Read(resolved) if resolved.refused => Drawn {
                    complaints: resolved.complaints,
                    refused: true,
                    ..Drawn::of(default_theme())
                },
                Found::Read(resolved) => valued(&resolved.theme, resolved.complaints),
            }
        }
        None => return Err(Vec::new()),
    })
}

/// Purlis, and why.
fn cannot(why: String) -> Drawn {
    Drawn {
        complaints: vec![Complaint::said(why)],
        ..Drawn::of(default_theme())
    }
}

/// `theme` in `appearance`: its first with none, following the system with `system`, else the
/// appearance of that id; one it does not have draws its first, and says so. With more
/// `contrast` asked for, its first (also in place of an id it does not have) and following the
/// system draw the appearance of their tag tagged high contrast, when it has one; an appearance
/// chosen by its id is drawn as chosen.
pub(super) fn appear(
    theme: &Theme,
    appearance: Option<&str>,
    system: Appearance,
    contrast: Contrast,
) -> Drawn {
    match appearance {
        None => Drawn::of(theme.first_for(contrast)),
        Some(SYSTEM) => follow(theme, system, contrast),
        Some(id) => match theme.appearances.iter().find(|one| one.id == id) {
            Some(one) => Drawn::of(one),
            None => Drawn {
                complaints: vec![Complaint::said(format!(
                    "{} has no appearance called {id}, so its first is drawn",
                    theme.name
                ))],
                ..Drawn::of(theme.first_for(contrast))
            },
        },
    }
}

/// **Copies today's in-plane picks into machine config, once** (spec #2313: the per-workspace
/// pick that exists today maps to the workspace level once, with a Notice): the project's
/// `charter.local.toml` `[theme]` becomes the person's choice for the project, and a LOCAL
/// workspace's `settings.theme` the person's choice for that workspace, each only where machine
/// config holds no choice there yet. So a pick made on this machine survives a re-clone of a
/// project that keeps a `[project] id`. A LIVE workspace's pick is its committed default
/// (level 2) and is not copied; where it now comes before the person's choice for the project,
/// which the local file's pick used to beat, that is said once.
///
/// Answers one sentence for each thing to say, for the Notice; none when there is nothing new,
/// so a second call says nothing.
pub fn adopt(
    config_root: &Path,
    plane: &Path,
    workspace: Option<&str>,
) -> std::io::Result<Vec<String>> {
    let Some(key) = project_key(plane) else {
        return Ok(Vec::new());
    };
    let said = Said::read_in(plane, workspace);
    let live = workspace.filter(|name| crate::wscmd::live_workspaces(plane).contains(*name));
    let local = said_choice(&said, Source::Local);
    let in_workspace = said_choice(&said, Source::Workspace);
    let held = choices::read(config_root);
    let mine = held.projects.get(&key).cloned().unwrap_or_default();
    let copy_local = local.clone().filter(|_| mine.project.is_none());
    let copy_workspace = workspace
        .filter(|_| live.is_none())
        .zip(in_workspace.clone())
        .filter(|(name, _)| !mine.workspaces.contains_key(*name));
    let mine_for_project = mine.project.clone().or(local);
    let outranked = live
        .zip(in_workspace)
        .zip(mine_for_project)
        .filter(|((name, _), _)| {
            !mine.workspaces.contains_key(*name) && !mine.told.iter().any(|one| one == name)
        });
    if copy_local.is_none() && copy_workspace.is_none() && outranked.is_none() {
        return Ok(Vec::new());
    }
    // A choice copied names the level of the folder it reads now (#2516).
    let folders = Folders::of(Some(config_root), Some(plane), workspace);
    let copy_local = copy_local.map(|choice| choice.placed(&folders, Reach::Project));
    let copy_workspace =
        copy_workspace.map(|(name, choice)| (name, choice.placed(&folders, Reach::Workspace)));
    let mut said = Vec::new();
    choices::edit(config_root, |held: &mut Choices| {
        let mine = held.projects.entry(key.clone()).or_default();
        if let Some(choice) = copy_local
            && mine.project.is_none()
        {
            said.push(format!(
                "The theme {} picked in {} is now your choice for this project on this machine.",
                choice.theme,
                crate::profiles::LOCAL_FILE
            ));
            mine.project = Some(choice);
        }
        if let Some((name, choice)) = copy_workspace
            && !mine.workspaces.contains_key(name)
        {
            said.push(format!(
                "The theme {} picked in {} is now your choice for the workspace {name} on this \
                 machine.",
                choice.theme,
                crate::settings::workspace::named(name)
            ));
            mine.workspaces.insert(name.to_owned(), choice);
        }
        if let Some(((name, committed), yours)) = outranked
            && !mine.told.iter().any(|one| one == name)
        {
            said.push(format!(
                "The workspace {name} is LIVE and commits the theme {}, which now comes before \
                 your choice for the project ({}). Choose your theme for the workspace to draw \
                 yours there.",
                committed.theme, yours.theme
            ));
            mine.told.push(name.to_owned());
        }
    })?;
    Ok(said)
}

/// What [`move_into_a_folder`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Moved {
    /// The theme folder the file is in now.
    pub name: String,
    /// Whether it was chosen as the machine default: not when one was chosen already, which is
    /// kept, and which the window drew before the move too.
    pub chosen: bool,
}

/// **Moves the person's format 1 `theme.json` into a theme folder** of their own (spec #2313:
/// an old single-file theme keeps working, and is offered once to move into a theme folder):
/// `themes/<name>/theme.json` in purlis's config folder, `<name>` made from the theme's name,
/// and the theme chosen as the machine default where none is chosen, so the window draws what
/// it drew. **All or nothing**: never over anything (a name taken gets `-2`, `-3`…); a folder
/// made for a move that did not happen is taken away again; and when choosing it fails, the
/// file is moved back. Refused, moving nothing, when there is no theme file in force or it is
/// not format 1.
pub fn move_into_a_folder(config_root: &Path) -> Result<Moved, String> {
    let theirs = super::read_theirs(Some(config_root));
    let Some(theme) = theirs.theme.as_ref().filter(|_| theirs.format_1) else {
        return Err(format!(
            "{} is not a theme file purlis draws in the old format, so nothing was moved",
            theirs.path
        ));
    };
    let stem = folder_name_for(&theme.name);
    let dir = crate::machine::private_dir(config_root)
        .map_err(|why| format!("purlis could not make its config folder: {why}"))?;
    let themes = dir.join(THEMES);
    crate::contain::no_link_on_the_way(config_root, &themes)
        .map_err(|why| format!("{} is not written: {why}", themes.display()))?;
    std::fs::create_dir_all(&themes)
        .map_err(|why| format!("purlis could not make {}: {why}", themes.display()))?;
    let from = crate::windowprefs::theme_path(config_root);
    for n in 1..=99 {
        let name = if n == 1 {
            stem.clone()
        } else {
            let tail = format!("-{n}");
            let mut cut = stem.clone();
            cut.truncate(32 - tail.len());
            format!("{}{tail}", cut.trim_end_matches('-'))
        };
        let folder = themes.join(&name);
        match std::fs::create_dir(&folder) {
            Ok(()) => {}
            Err(taken) if taken.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(why) => return Err(format!("purlis could not make {}: {why}", folder.display())),
        }
        let to = folder.join(super::THEME_JSON);
        // The folder was made for this move: when the move does not happen, it goes again.
        let undo_folder = || {
            let _ = std::fs::remove_dir(&folder);
        };
        match crate::guest::rename_new(&from, &to) {
            Ok(true) => {}
            Ok(false) => {
                undo_folder();
                return Err(format!(
                    "{} is there already, so nothing was moved",
                    to.display()
                ));
            }
            Err(why) => {
                undo_folder();
                return Err(format!("purlis could not move {}: {why}", from.display()));
            }
        }
        let choice = Choice::new(&name, None)
            .expect("a folder name made here is a pick")
            .at(Reach::Machine);
        let mut chosen = false;
        let kept = choices::edit(config_root, |held| {
            if held.machine.is_none() {
                held.machine = Some(choice);
                chosen = true;
            }
        });
        if let Err(why) = kept {
            // Choosing it failed: the file goes back where it was, and the folder with it.
            return match crate::guest::rename_new(&to, &from) {
                Ok(true) => {
                    undo_folder();
                    Err(format!(
                        "purlis could not choose the moved theme ({why}), so it moved it back"
                    ))
                }
                _ => Err(format!(
                    "purlis could not choose the moved theme ({why}), and it is in {}",
                    to.display()
                )),
            };
        }
        return Ok(Moved { name, chosen });
    }
    Err(format!(
        "every name from {stem} to {stem}-99 is taken in {}, so nothing was moved",
        themes.display()
    ))
}

/// A theme folder's name made from a theme's: lower-case letters and digits, every run of
/// anything else one dash, starting with a letter, at most 32 long; `mine` when nothing is left.
fn folder_name_for(theme: &str) -> String {
    let mut name = String::new();
    for c in theme.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_lowercase() || (c.is_ascii_digit() && !name.is_empty()) {
            name.push(c);
        } else if !name.is_empty() && !name.ends_with('-') {
            name.push('-');
        }
    }
    name.truncate(32);
    let name = name.trim_end_matches('-').to_owned();
    if super::is_folder_name(&name) {
        name
    } else {
        "mine".to_owned()
    }
}
