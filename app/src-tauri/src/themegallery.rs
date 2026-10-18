// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Settings › Theme's gallery, for the window** (spec #2313, #2331): the theme engine's cards
//! for the level the tab is at ([`purlis_core::theme::gallery`]), what a card draws while the
//! pointer is on it, and what a card does — the person's choice at the level (the machine's
//! here; a project's and a workspace's are [`crate::extensions::theme_choose`]), **Duplicate as
//! my theme** and **Open folder**.
//!
//! **The level is where the window is** ([`ThemeAt`]): `None` is the machine's (Settings at
//! the You level), a project with no workspace the project's, and a workspace its own. The
//! engine reads that level's reach, its theme folders, and the extensions in force there; the
//! window draws what it is handed and nothing else.
//!
//! Every command that writes the person's standing state is the window's alone
//! (`session_protocol::ui::WINDOW_ONLY`): the machine choice, a duplicate, and opening a folder,
//! which starts the person's file manager.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use purlis_core::extension;
use purlis_core::theme::gallery::{self, Card, From as CardFrom, Shelf};
use purlis_core::theme::levels::Reach;
use purlis_core::theme::{self, Folders, Level, LevelInputs, Levels, Offered, On, choices};

use crate::extensions::{
    ThemeAt, ThemeInForce, WindowOption, WindowOptionValue, WindowTheme, YourTheme, built_in,
    config_root, offered_now,
};

/// **A level's gallery**, as Settings › Theme draws it.
#[derive(Debug, Clone, PartialEq, serde::Serialize, specta::Type)]
pub struct ThemeGallery {
    /// Every theme a choice at this level may name, Purlis Classic first.
    pub cards: Vec<GalleryCard>,
    /// The person's own choice at this level, kept on this machine; `null` when they made none.
    pub yours: Option<YourTheme>,
    /// The card the window draws here, as the levels resolve it; `null` when what is drawn is
    /// no card of this gallery (the person's own `theme.json`).
    pub drawn: Option<String>,
    /// Which level chose what is drawn (`workspace-choice`, `workspace-default`,
    /// `project-choice`, `project-default`, `machine-default`, `purlis`).
    pub level: String,
    /// Where that level is, in a sentence's words: "your choice for this workspace on this
    /// machine", "charter.toml", …
    pub from: String,
}

/// **One card of a gallery**: what the theme engine read of one theme.
#[derive(Debug, Clone, PartialEq, serde::Serialize, specta::Type)]
pub struct GalleryCard {
    /// What a choice names it by.
    pub id: String,
    pub name: String,
    /// `built-in`, `machine`, `project`, `workspace` or `extension`.
    pub from: String,
    /// The extension it comes from, for an extension's theme.
    pub extension: Option<String>,
    /// Its appearances, the first what it draws when nothing chose one.
    pub appearances: Vec<GalleryAppearance>,
    /// Its options, each at its default.
    pub options: Vec<WindowOption>,
    pub refused: bool,
    /// Why it is refused, or what was left out of it: a sentence each.
    pub said: Vec<String>,
    /// The theme check's warnings, a line each.
    pub warnings: Vec<String>,
    /// The consent floors it breaks, a sentence each.
    pub floors: Vec<String>,
    /// What following the system does with a theme of one tag; `null` with a dark and a light.
    pub follows: Option<String>,
    /// Whether Duplicate as my theme is offered.
    pub duplicable: bool,
    /// Whether Open folder is offered.
    pub folder: bool,
    /// Changes whenever what the theme draws changes: what its pictures are kept by.
    pub stamp: String,
}

/// One appearance of a card.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct GalleryAppearance {
    pub id: String,
    /// `dark` or `light`.
    pub appearance: String,
    pub high_contrast: bool,
}

impl From<Card> for GalleryCard {
    fn from(card: Card) -> Self {
        let (from, extension) = match card.from {
            CardFrom::BuiltIn => ("built-in", None),
            CardFrom::Machine => ("machine", None),
            CardFrom::Project => ("project", None),
            CardFrom::Workspace => ("workspace", None),
            CardFrom::Extension { extension } => ("extension", Some(extension)),
        };
        Self {
            id: card.id,
            name: card.name,
            from: from.to_owned(),
            extension,
            appearances: card
                .appearances
                .into_iter()
                .map(|one| GalleryAppearance {
                    id: one.id,
                    appearance: purlis_core::themepreview::word(one.appearance).to_owned(),
                    high_contrast: one.high_contrast,
                })
                .collect(),
            options: card
                .options
                .into_iter()
                .map(|option| WindowOption {
                    kind: match option.kind {
                        theme::Kind::Toggle => "toggle",
                        theme::Kind::Choice => "choice",
                    }
                    .to_owned(),
                    values: option
                        .values
                        .iter()
                        .map(|one| WindowOptionValue {
                            id: one.id.clone(),
                            label: one.label.clone(),
                        })
                        .collect(),
                    value: option.default.clone(),
                    chosen: false,
                    id: option.id,
                    label: option.label,
                    description: option.description,
                    default: option.default,
                })
                .collect(),
            refused: card.refused,
            said: card.said,
            warnings: card.warnings,
            floors: card.floors,
            follows: card.follows,
            duplicable: card.duplicable,
            folder: card.folder,
            stamp: card.stamp,
        }
    }
}

/// **What a gallery is read from at one level**: the levels' theme folders, how far a choice
/// there reaches, and whose extension themes are in force there.
struct Place {
    config: PathBuf,
    plane: Option<PathBuf>,
    workspace: Option<String>,
    folders: Folders,
    reach: Reach,
    /// The extensions the project has on; `None` with no project, when every approved one is.
    on: Option<Vec<String>>,
    offered: Arc<[Offered]>,
}

impl Place {
    /// The level `at` names, read from the config home and the plane the window holds.
    fn read(config: PathBuf, plane: Option<PathBuf>, workspace: Option<String>) -> Self {
        let workspace = plane.as_ref().and(workspace);
        let reach = match (&plane, &workspace) {
            (None, _) => Reach::Machine,
            (Some(_), None) => Reach::Project,
            (Some(_), Some(_)) => Reach::Workspace,
        };
        let on = plane.as_deref().map(|root| {
            let choices = extension::project::Choices::read_in(root, workspace.as_deref());
            extension::project::resolve(
                &extension::project::Installed::from_record(&extension::read(&config, &built_in())),
                &choices,
            )
            .into_iter()
            .filter(|it| it.state == extension::project::State::On)
            .map(|it| it.id)
            .collect()
        });
        Self {
            folders: Folders::of(Some(&config), plane.as_deref(), workspace.as_deref()),
            offered: offered_now(&config),
            config,
            plane,
            workspace,
            reach,
            on,
        }
    }

    fn shelf(&self) -> Shelf<'_> {
        Shelf {
            folders: &self.folders,
            reach: self.reach,
            on: self.on.as_deref().map_or(On::Every, On::Only),
            offered: &self.offered,
        }
    }
}

/// The level `at` names, read off the window's thread.
async fn place_of(
    planes: &crate::planes::Planes,
    at: Option<ThemeAt>,
) -> Result<(PathBuf, Option<PathBuf>, Option<String>), String> {
    let config = config_root()?;
    Ok(match at {
        Some(ThemeAt { plane, workspace }) => (
            config,
            Some(planes.held(&plane)?.root().to_path_buf()),
            workspace,
        ),
        None => (config, None, None),
    })
}

/// **The gallery at the level `at` names** (#2331): a card per theme a choice there may name,
/// the person's own choice there, and which card is drawn and which level chose it.
#[tauri::command]
#[specta::specta]
pub async fn theme_gallery(
    planes: tauri::State<'_, crate::planes::Planes>,
    at: Option<ThemeAt>,
) -> Result<ThemeGallery, String> {
    let (config, plane, workspace) = place_of(&planes, at).await?;
    tauri::async_runtime::spawn_blocking(move || {
        let place = Place::read(config, plane, workspace);
        let theirs = crate::windowprefs::THEIRS.get(Some(&place.config)).theme;
        gallery_of(&place, theirs.as_ref())
    })
    .await
    .map_err(|err| format!("reading the theme gallery did not finish: {err}"))
}

/// [`theme_gallery`] without a runtime.
fn gallery_of(place: &Place, theirs: Option<&theme::Theme>) -> ThemeGallery {
    let levels = Levels::read(
        Some(&place.config),
        place.plane.as_deref(),
        place.workspace.as_deref(),
    );
    let offered = place.offered.clone();
    let drawn = theme::resolve_levels(
        &LevelInputs {
            levels: &levels,
            folders: &place.folders,
            theirs,
            on: place.shelf().on,
            system: theme::Appearance::Dark,
            // Which level chose, for Settings: the same whatever contrast is asked.
            contrast: theme::Contrast::Standard,
        },
        || offered,
    );
    let level = drawn.level.unwrap_or(Level::Purlis);
    let cards: Vec<GalleryCard> = gallery::cards(&place.shelf())
        .into_iter()
        .map(GalleryCard::from)
        .collect();
    // The card the level that chose names: Purlis Classic by any id one of its appearances
    // has had, and with nothing chosen.
    let chose = levels
        .each()
        .find(|(one, _)| *one == level)
        .map(|(_, choice)| choice.theme.clone());
    let drawn_id = match (level, chose) {
        (Level::Purlis, _) => Some(theme::CLASSIC.to_owned()),
        (_, Some(id)) if id == theme::CLASSIC || theme::built_in(&id).is_some() => {
            Some(theme::CLASSIC.to_owned())
        }
        (_, chose) => chose,
    }
    .filter(|id| cards.iter().any(|card| card.id == *id));
    let held = choices::read(&place.config);
    let yours = match (&place.plane, &place.workspace) {
        (None, _) => held.machine,
        (Some(root), workspace) => theme::levels::project_key(root).and_then(|key| {
            let mine = held.projects.get(&key)?;
            match workspace {
                Some(name) => mine.workspaces.get(name).cloned(),
                None => mine.project.clone(),
            }
        }),
    }
    .map(|one| YourTheme {
        theme: one.theme,
        appearance: one.appearance,
    });
    ThemeGallery {
        cards,
        yours,
        drawn: drawn_id,
        level: serde_json::to_value(level)
            .ok()
            .and_then(|it| it.as_str().map(str::to_owned))
            .unwrap_or_default(),
        from: from_said(level, place.workspace.as_deref(), theirs.is_some()),
    }
}

/// Where the level that chose is, in a sentence's words.
fn from_said(level: Level, workspace: Option<&str>, theirs: bool) -> String {
    match level {
        Level::WorkspaceChoice => "your choice for this workspace on this machine".to_owned(),
        Level::WorkspaceDefault => workspace
            .map(purlis_core::settings::workspace::named)
            .unwrap_or_default(),
        Level::ProjectChoice => "your choice for this project on this machine".to_owned(),
        Level::ProjectDefault => {
            extension::project::Source::file(extension::project::Source::Shared)
                .unwrap_or_default()
                .to_owned()
        }
        Level::MachineDefault if theirs => "your own theme.json on this machine".to_owned(),
        Level::MachineDefault => "your machine default".to_owned(),
        Level::Purlis => "nothing chose one".to_owned(),
    }
}

/// **What the card `theme` draws** at the level `at` names, in `appearance` (an appearance's
/// id, `system` to follow the operating system, which is light when `light`, or `null` for its
/// first), with more contrast when the system asks for it: what the window draws while the pointer or the keyboard is on the card, and puts
/// back when it leaves. Nothing is written. `null` when the gallery has no such card.
#[tauri::command]
#[specta::specta]
pub async fn theme_card_drawn(
    planes: tauri::State<'_, crate::planes::Planes>,
    at: Option<ThemeAt>,
    theme: String,
    appearance: Option<String>,
    light: bool,
    more_contrast: bool,
) -> Result<Option<ThemeInForce>, String> {
    let (config, plane, workspace) = place_of(&planes, at).await?;
    tauri::async_runtime::spawn_blocking(move || {
        let place = Place::read(config, plane, workspace);
        card_drawn_of(&place, &theme, appearance.as_deref(), light, more_contrast)
    })
    .await
    .map_err(|err| format!("reading that theme did not finish: {err}"))
}

/// [`theme_card_drawn`] without a runtime.
fn card_drawn_of(
    place: &Place,
    id: &str,
    appearance: Option<&str>,
    light: bool,
    more_contrast: bool,
) -> Option<ThemeInForce> {
    let system = if light {
        theme::Appearance::Light
    } else {
        theme::Appearance::Dark
    };
    let drawn = gallery::drawn(
        &place.shelf(),
        id,
        appearance,
        system,
        crate::extensions::contrast_of(more_contrast),
    )?;
    Some(ThemeInForce {
        options: drawn
            .theme
            .as_ref()
            .map(|one| one.options.iter().map(WindowOption::from).collect())
            .unwrap_or_default(),
        theme: drawn.theme.as_ref().map(WindowTheme::from),
        complaints: drawn.complaints.into_iter().map(|one| one.said).collect(),
        follows_system: drawn.follows_system,
        floors: drawn.held.map(|held| held.said()),
    })
}

/// **The theme a specimen of the card `theme` is drawn in** (#2330's painter, for the
/// gallery's cards): the card in `appearance` at the level `at` names, with the floors held.
pub(crate) fn specimen_theme(
    place: Option<(PathBuf, Option<String>)>,
    theme: &str,
    appearance: Option<&str>,
) -> Result<WindowTheme, String> {
    let (plane, workspace) =
        place.map_or((None, None), |(root, workspace)| (Some(root), workspace));
    let place = Place::read(config_root()?, plane, workspace);
    card_drawn_of(&place, theme, appearance, false, false)
        .and_then(|drawn| drawn.theme)
        .ok_or_else(|| format!("there is no theme {theme} here to draw the specimen of"))
}

/// **The person's machine default** (#2331, level 5): `theme` and `appearance` as a pick names
/// them, kept beside `machine.json` and never in a project file; `theme` of `null` takes the
/// choice away. What Settings › Theme writes at the You level.
#[tauri::command]
#[specta::specta]
pub async fn theme_choose_machine(
    theme: Option<String>,
    appearance: Option<String>,
) -> Result<(), String> {
    let config = config_root()?;
    tauri::async_runtime::spawn_blocking(move || {
        choose_machine_of(&config, theme.as_deref(), appearance.as_deref())
    })
    .await
    .map_err(|err| format!("keeping your theme choice did not finish: {err}"))?
}

/// [`theme_choose_machine`] without a runtime.
fn choose_machine_of(
    config: &Path,
    theme: Option<&str>,
    appearance: Option<&str>,
) -> Result<(), String> {
    let choice = match theme {
        None => None,
        Some(theme) => Some(theme::Choice::new(theme, appearance).ok_or_else(|| {
            format!("{theme} is not a theme a choice can name, so nothing was kept")
        })?),
    };
    choices::set(config, choices::Slot::Machine, choice).map_err(|why| why.to_string())
}

/// **Duplicate as my theme** (#2331): a theme folder of the person's own that extends the card
/// `theme` of the gallery at the level `at` names. Answers the new folder's name.
#[tauri::command]
#[specta::specta]
pub async fn theme_duplicate(
    planes: tauri::State<'_, crate::planes::Planes>,
    at: Option<ThemeAt>,
    theme: String,
) -> Result<String, String> {
    let (config, plane, workspace) = place_of(&planes, at).await?;
    tauri::async_runtime::spawn_blocking(move || {
        let place = Place::read(config, plane, workspace);
        gallery::duplicate(&place.config, &place.shelf(), &theme)
    })
    .await
    .map_err(|err| format!("duplicating that theme did not finish: {err}"))?
}

/// **Open folder** (#2331): the card `theme`'s theme folder, at the level `at` names, opened in
/// the operating system's file manager. The window names the card; the folder is the one the
/// engine places for it, never a path the window sent.
#[tauri::command]
#[specta::specta]
pub async fn theme_open_folder(
    app: tauri::AppHandle,
    planes: tauri::State<'_, crate::planes::Planes>,
    at: Option<ThemeAt>,
    theme: String,
) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt as _;
    let (config, plane, workspace) = place_of(&planes, at).await?;
    let asked = theme.clone();
    let folder = tauri::async_runtime::spawn_blocking(move || {
        let place = Place::read(config, plane, workspace);
        gallery::folder_of(&place.shelf(), &asked)
    })
    .await
    .map_err(|err| format!("finding that theme's folder did not finish: {err}"))?
    .ok_or_else(|| format!("{theme} has no theme folder here to open"))?;
    app.opener()
        .open_path(folder.display().to_string(), None::<&str>)
        .map_err(|e| format!("the system did not open {theme}'s folder: {e}"))
}

#[cfg(test)]
mod tests;
