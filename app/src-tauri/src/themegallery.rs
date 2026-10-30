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
//! **A card is a theme and, for a theme folder, its level** (#2516): two folders of one name at
//! two levels are two cards, told apart by their [`GalleryCard::key`], and every command that
//! names a card names its level with it (`from`).
//!
//! **The chosen theme's options are choosable at the level** (#2527): [`theme_choose_option`]
//! keeps a value in the person's choice there, and each value the gallery shows says which
//! level's choice made it.
//!
//! Every command that writes the person's standing state is the window's alone
//! (`session_protocol::ui::WINDOW_ONLY`): the machine choice, an option's value, a duplicate,
//! and opening a folder, which starts the person's file manager.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use purlis_core::extension;
use purlis_core::theme::gallery::{self, Card, From as CardFrom, Shelf};
use purlis_core::theme::levels::Reach;
use purlis_core::theme::{
    self, Choice, Folders, Level, LevelInputs, Levels, Offered, On, Values, choices,
};
use purlis_core::themechat;

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
    /// The [`GalleryCard::key`] of the chosen card: the person's choice here, else what is drawn
    /// here; `null` when that is no card of this gallery.
    pub chosen: Option<String>,
    /// The chosen card's options, each with the value it is drawn with here and the level whose
    /// choice made it (#2527).
    pub options: Vec<WindowOption>,
    /// **The person's own `theme.json`, when it is what is drawn here** (#2615): it is no card,
    /// so this says that it is drawn and what following the system does with it; `null` when
    /// what is drawn is a card.
    pub theirs: Option<TheirsDrawn>,
}

/// **The person's own `theme.json`, drawn** (#2615).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct TheirsDrawn {
    /// Whether it follows the operating system's appearance here: it has a dark and a light one,
    /// and the choice drawn follows the system.
    pub follows_system: bool,
    /// What following the system does with it when it has one appearance: that it stays there.
    pub note: Option<String>,
}

/// **One card of a gallery**: what the theme engine read of one theme.
#[derive(Debug, Clone, PartialEq, serde::Serialize, specta::Type)]
pub struct GalleryCard {
    /// **What tells it from every other card**: its id, and for a theme folder its level too
    /// (`project:brand`), since two levels may each have a folder of one name (#2516).
    pub key: String,
    /// What a choice names it by.
    pub id: String,
    /// The level of the theme folder it is (`workspace`, `project` or `machine`); `null` for a
    /// card that is no folder.
    pub level: Option<String>,
    pub name: String,
    /// `built-in`, `machine`, `project`, `workspace` or `extension`.
    pub from: String,
    /// Whether it ships as a preview: Purlis, until it becomes the default (#2336).
    pub preview: bool,
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
        let level = card.level();
        let (from, extension) = match card.from {
            CardFrom::BuiltIn => ("built-in", None),
            CardFrom::Machine => ("machine", None),
            CardFrom::Project => ("project", None),
            CardFrom::Workspace => ("workspace", None),
            CardFrom::Extension { extension } => ("extension", Some(extension)),
        };
        Self {
            key: key_of(&card.id, level),
            level: level.map(|one| one.word().to_owned()),
            id: card.id,
            name: card.name,
            from: from.to_owned(),
            preview: card.preview,
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
                    level: None,
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
pub(crate) struct Place {
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
    pub(crate) fn read(config: PathBuf, plane: Option<PathBuf>, workspace: Option<String>) -> Self {
        Self::offering(config, plane, workspace, offered_now)
    }

    /// [`Self::read`], with the extension themes approved under the config home as `offered`
    /// surveys them.
    fn offering(
        config: PathBuf,
        plane: Option<PathBuf>,
        workspace: Option<String>,
        offered: impl FnOnce(&Path) -> Arc<[Offered]>,
    ) -> Self {
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
            offered: offered(&config),
            config,
            plane,
            workspace,
            reach,
            on,
        }
    }

    pub(crate) fn shelf(&self) -> Shelf<'_> {
        Shelf {
            folders: &self.folders,
            reach: self.reach,
            on: self.on.as_deref().map_or(On::Every, On::Only),
            offered: &self.offered,
        }
    }
}

/// **A card's key** ([`GalleryCard::key`]): its id, after its folder's level for a theme folder.
fn key_of(id: &str, level: Option<Reach>) -> String {
    match level {
        Some(level) => format!("{}:{id}", level.word()),
        None => id.to_owned(),
    }
}

/// The level a window names by its word; refused for any other word.
fn level_of(from: Option<&str>) -> Result<Option<Reach>, String> {
    from.map(|word| {
        Reach::of_word(word).ok_or_else(|| format!("{word} is not the level of a theme folder"))
    })
    .transpose()
}

impl Place {
    /// **The person's own choice at this level**, kept on this machine.
    fn yours(&self) -> Option<Choice> {
        let held = choices::read(&self.config);
        match (&self.plane, &self.workspace) {
            (None, _) => held.machine,
            (Some(root), workspace) => theme::levels::project_key(root).and_then(|key| {
                let mine = held.projects.get(&key)?;
                match workspace {
                    Some(name) => mine.workspaces.get(name).cloned(),
                    None => mine.project.clone(),
                }
            }),
        }
    }

    /// **The option values the card `theme` of `level` is drawn with here**: the person's
    /// choice's at this level when it is a choice of that card, else none, each option at its
    /// default. What a card shows, pointed at and in its pictures, is what choosing it draws.
    fn values_for(&self, theme: &str, level: Option<Reach>) -> Values {
        let asked = Choice::new(theme, None).map(|one| {
            match level {
                Some(level) => one.at(level),
                None => one,
            }
            .placed(&self.folders, self.reach)
        });
        match (self.yours(), asked) {
            (Some(yours), Some(asked))
                if yours
                    .clone()
                    .placed(&self.folders, self.reach)
                    .same_theme(&asked) =>
            {
                yours.options
            }
            _ => Values::new(),
        }
    }

    /// Where the person's choice at this level is kept; `None` for a project purlis cannot tell.
    fn slot<'a>(&'a self, key: Option<&'a str>) -> Option<choices::Slot<'a>> {
        match (&self.plane, key, self.workspace.as_deref()) {
            (None, _, _) => Some(choices::Slot::Machine),
            (Some(_), Some(key), Some(name)) => Some(choices::Slot::Workspace(key, name)),
            (Some(_), Some(key), None) => Some(choices::Slot::Project(key)),
            (Some(_), None, _) => None,
        }
    }

    /// **The card a choice made at a level that reaches `reach` names**, by its key: Purlis
    /// Classic by its id; Purlis by its id, by `purlis-dark` or `purlis-light` or an id either
    /// had (#3034); a theme folder at the level it names, or the nearest within reach that has
    /// it; and the old `system` id, the person's own theme when there is one (`theirs`), which is
    /// no card, else Purlis. `None` when it is no card of `cards`.
    fn card_of(
        &self,
        choice: &Choice,
        reach: Reach,
        cards: &[GalleryCard],
        theirs: bool,
    ) -> Option<String> {
        if choice.theme == extension::project::theme::SYSTEM && theirs {
            return None;
        }
        // Purlis Classic by its own id is its card; Purlis by its own, by `purlis-dark`,
        // `purlis-light` or an id either had, and by the old `system` id with no theme of the
        // person's (#3034).
        let key = if choice.theme == theme::CLASSIC {
            theme::CLASSIC.to_owned()
        } else if choice.theme == extension::project::theme::SYSTEM
            || theme::built_in(&choice.theme).is_some()
        {
            theme::PURLIS.to_owned()
        } else {
            let placed = choice.clone().placed(&self.folders, reach);
            key_of(&placed.theme, placed.from)
        };
        cards.iter().any(|card| card.key == key).then_some(key)
    }
}

/// **The theme `choice` names**, read from `folders` as a choice made at a level that reaches
/// `reach` reads it: Purlis Classic or a theme folder; `None` for an extension's theme, which a
/// choice is not read with here, and for a theme that is not there or is refused.
pub(crate) fn theme_named(
    folders: &Folders,
    reach: Reach,
    choice: &Choice,
) -> Option<theme::Theme> {
    let shelf = Shelf {
        folders,
        reach,
        on: On::Every,
        offered: &[],
    };
    gallery::theme_of(&shelf, &choice.theme, choice.from)
        .filter(|read| !read.refused)
        .map(|read| read.theme)
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
    // The card the level that chose names ([`Place::card_of`]); the default, Purlis, with
    // nothing chosen (#3034).
    let chose = levels.of_level(level);
    let drawn_key = match chose {
        None if level == Level::Purlis => Some(theme::DEFAULT.to_owned()),
        None => None,
        Some(choice) => place.card_of(choice, level.reach(), &cards, theirs.is_some()),
    };
    let yours = place.yours();
    let yours_key = yours
        .as_ref()
        .and_then(|choice| place.card_of(choice, place.reach, &cards, theirs.is_some()));
    // The chosen card, and the choice its values come from, at the level that made it.
    let (chosen, valued) = match (&yours_key, yours.as_ref()) {
        (Some(key), Some(choice)) => (Some(key.clone()), Some((choice, own_level(place)))),
        _ => (
            drawn_key.clone(),
            chose
                .map(|choice| (choice, level))
                .filter(|_| drawn_key.is_some()),
        ),
    };
    let options = chosen
        .as_deref()
        .and_then(|key| cards.iter().find(|card| card.key == key))
        .and_then(|card| {
            let (values, made) = valued.map_or((Values::new(), None), |(choice, made)| {
                (choice.options.clone(), Some(made))
            });
            let drawn = gallery::drawn(
                &place.shelf(),
                &card.id,
                card.level.as_deref().and_then(Reach::of_word),
                None,
                &values,
                theme::Appearance::Dark,
                theme::Contrast::Standard,
            )?;
            Some(
                drawn
                    .theme?
                    .options
                    .iter()
                    .map(|one| WindowOption::from(one).chosen_at(made))
                    .collect(),
            )
        })
        .unwrap_or_default();
    ThemeGallery {
        cards,
        yours: yours.map(YourTheme::from),
        drawn: drawn_key,
        level: serde_json::to_value(level)
            .ok()
            .and_then(|it| it.as_str().map(str::to_owned))
            .unwrap_or_default(),
        from: from_said(level, place.workspace.as_deref(), theirs.is_some()),
        chosen,
        options,
        theirs: drawn.theirs.then(|| TheirsDrawn {
            follows_system: drawn.follows_system,
            note: drawn.note.clone(),
        }),
    }
}

/// The level of the person's own choice where `place` is.
fn own_level(place: &Place) -> Level {
    match (&place.plane, &place.workspace) {
        (None, _) => Level::MachineDefault,
        (Some(_), None) => Level::ProjectChoice,
        (Some(_), Some(_)) => Level::WorkspaceChoice,
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

/// **What the card `theme` of the level `from` draws** at the level `at` names, in `appearance`
/// (an appearance's id, `system` to follow the operating system, which is light when `light`,
/// or `null` for its first), with more contrast when the system asks for it: what the window
/// draws while the pointer or the keyboard is on the card, and puts back when it leaves. A card
/// of the theme the person chose here is drawn with the option values they chose for it. Nothing
/// is written. `null` when the gallery has no such card.
#[tauri::command]
#[specta::specta]
// The state is Tauri's to hand in; the rest is the window's question.
#[allow(clippy::too_many_arguments)]
pub async fn theme_card_drawn(
    planes: tauri::State<'_, crate::planes::Planes>,
    at: Option<ThemeAt>,
    theme: String,
    from: Option<String>,
    appearance: Option<String>,
    light: bool,
    more_contrast: bool,
    person: Option<String>,
) -> Result<Option<ThemeInForce>, String> {
    let person = crate::extensions::person_layer(person.as_deref());
    let level = level_of(from.as_deref())?;
    let (config, plane, workspace) = place_of(&planes, at).await?;
    tauri::async_runtime::spawn_blocking(move || {
        let place = Place::read(config, plane, workspace);
        let values = place.values_for(&theme, level);
        card_drawn_of(
            &place,
            &theme,
            level,
            appearance.as_deref(),
            &values,
            light,
            more_contrast,
            &person,
        )
    })
    .await
    .map_err(|err| format!("reading that theme did not finish: {err}"))
}

/// [`theme_card_drawn`] without a runtime.
// The window's question, as it asks it, and the person's layer it hands.
#[allow(clippy::too_many_arguments)]
fn card_drawn_of(
    place: &Place,
    id: &str,
    level: Option<Reach>,
    appearance: Option<&str>,
    values: &Values,
    light: bool,
    more_contrast: bool,
    person: &purlis_core::theme::Layer,
) -> Option<ThemeInForce> {
    let system = if light {
        theme::Appearance::Light
    } else {
        theme::Appearance::Dark
    };
    let drawn = gallery::drawn(
        &place.shelf(),
        id,
        level,
        appearance,
        values,
        system,
        crate::extensions::contrast_of(more_contrast),
    )?
    // A card drawn on the window wears the person's colours and contrast too (#2925).
    .under(person);
    Some(ThemeInForce {
        // The gallery's card says it is refused; only the window's theme offers the review.
        review: None,
        refused: drawn.refused.then(|| crate::extensions::ThemeTrouble {
            name: id.to_owned(),
            complaints: drawn
                .complaints
                .iter()
                .map(|one| one.said.clone())
                .collect(),
            refused: true,
        }),
        options: drawn
            .theme
            .as_ref()
            .map(|one| one.options.iter().map(WindowOption::from).collect())
            .unwrap_or_default(),
        theme: drawn.theme.as_ref().map(WindowTheme::from),
        complaints: drawn.complaints.into_iter().map(|one| one.said).collect(),
        follows_system: drawn.follows_system,
        floors: drawn.held.map(|held| held.said()),
        draft: None,
        mine: drawn.mine.into_iter().map(|one| one.said).collect(),
    })
}

/// **The theme a specimen of the card `theme` of the level `from` is drawn in** (#2330's
/// painter, for the gallery's cards): the card in `appearance` at the level `at` names, with
/// the option values the person chose for it there ([`Place::values_for`]), as pointing at the
/// card draws it, with the floors held.
pub(crate) fn specimen_theme(
    place: Option<(PathBuf, Option<String>)>,
    theme: &str,
    from: Option<&str>,
    appearance: Option<&str>,
) -> Result<WindowTheme, String> {
    let level = level_of(from)?;
    let (plane, workspace) =
        place.map_or((None, None), |(root, workspace)| (Some(root), workspace));
    let place = Place::read(config_root()?, plane, workspace);
    let values = place.values_for(theme, level);
    card_drawn_of(
        &place,
        theme,
        level,
        appearance,
        &values,
        false,
        false,
        &purlis_core::theme::Layer::default(),
    )
    .and_then(|drawn| drawn.theme)
    .ok_or_else(|| format!("there is no theme {theme} here to draw the specimen of"))
}

/// **The person's machine default** (#2331, level 5): `theme` and `appearance` as a pick names
/// them, and `from`, the level of the theme folder it picks (#2516), kept beside `machine.json`
/// and never in a project file; `theme` of `null` takes the choice away. A choice of the theme
/// already chosen keeps its option values (#2527). What Settings › Theme writes at the You level.
#[tauri::command]
#[specta::specta]
pub async fn theme_choose_machine(
    theme: Option<String>,
    appearance: Option<String>,
    from: Option<String>,
) -> Result<(), String> {
    let config = config_root()?;
    tauri::async_runtime::spawn_blocking(move || {
        choose_machine_of(
            &config,
            theme.as_deref(),
            appearance.as_deref(),
            from.as_deref(),
        )
    })
    .await
    .map_err(|err| format!("keeping your theme choice did not finish: {err}"))?
}

/// [`theme_choose_machine`] without a runtime.
fn choose_machine_of(
    config: &Path,
    theme: Option<&str>,
    appearance: Option<&str>,
    from: Option<&str>,
) -> Result<(), String> {
    let choice = crate::extensions::choice_of(
        theme,
        appearance,
        from,
        &Folders::of(Some(config), None, None),
        Reach::Machine,
    )?;
    let folders = Folders::of(Some(config), None, None);
    let theme = choice
        .as_ref()
        .and_then(|one| theme_named(&folders, Reach::Machine, one));
    choices::choose(
        config,
        choices::Slot::Machine,
        choice,
        &folders,
        Reach::Machine,
        theme.as_ref(),
    )
    .map_err(|why| why.to_string())
}

/// **The value of the chosen theme's option `option`** at the level `at` names (#2527): `value`,
/// one of the option's values by its id, or `null` for the theme's default, kept in the
/// person's choice there on this machine, never in a project file. With no choice there yet,
/// the choice that draws the theme there is made the person's, with the value. Answers a
/// sentence for each value the choice held that its theme no longer has, which is dropped.
#[tauri::command]
#[specta::specta]
pub async fn theme_choose_option(
    planes: tauri::State<'_, crate::planes::Planes>,
    at: Option<ThemeAt>,
    option: String,
    value: Option<String>,
) -> Result<Vec<String>, String> {
    let (config, plane, workspace) = place_of(&planes, at).await?;
    tauri::async_runtime::spawn_blocking(move || {
        let place = Place::read(config, plane, workspace);
        let theirs = crate::windowprefs::THEIRS.get(Some(&place.config)).theme;
        choose_option_of(&place, theirs.as_ref(), &option, value.as_deref())
    })
    .await
    .map_err(|err| format!("keeping that option did not finish: {err}"))?
}

/// [`theme_choose_option`] without a runtime.
fn choose_option_of(
    place: &Place,
    theirs: Option<&theme::Theme>,
    option: &str,
    value: Option<&str>,
) -> Result<Vec<String>, String> {
    let base = match place.yours() {
        Some(yours) => yours,
        None => {
            // No choice here yet: the one that draws the theme here, at the level it reaches.
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
                    contrast: theme::Contrast::Standard,
                },
                || offered,
            );
            let level = drawn.level.unwrap_or(Level::Purlis);
            match levels.of_level(level) {
                Some(choice) => choice.clone().placed(&place.folders, level.reach()),
                None if level == Level::Purlis => {
                    Choice::new(theme::DEFAULT, None).expect("the default theme is a pick")
                }
                None => {
                    return Err(
                        "your own theme.json is drawn here, and it has no options to choose: \
                         choose a theme above first"
                            .to_owned(),
                    );
                }
            }
        }
    };
    let (choice, notes) = gallery::choose_option(&place.shelf(), &base, option, value)?;
    let key = place.plane.as_deref().and_then(theme::levels::project_key);
    let slot = place.slot(key.as_deref()).ok_or_else(|| {
        "purlis cannot tell which project this is, so it keeps no choice".to_owned()
    })?;
    choices::set(&place.config, slot, Some(choice)).map_err(|why| why.to_string())?;
    Ok(notes)
}

/// **Duplicate as my theme** (#2331): a theme folder of the person's own that extends the card
/// `theme` of the level `from` of the gallery at the level `at` names. Answers the new folder's
/// name.
#[tauri::command]
#[specta::specta]
pub async fn theme_duplicate(
    planes: tauri::State<'_, crate::planes::Planes>,
    at: Option<ThemeAt>,
    theme: String,
    from: Option<String>,
) -> Result<String, String> {
    let level = level_of(from.as_deref())?;
    let (config, plane, workspace) = place_of(&planes, at).await?;
    tauri::async_runtime::spawn_blocking(move || {
        let place = Place::read(config, plane, workspace);
        gallery::duplicate(&place.config, &place.shelf(), &theme, level)
    })
    .await
    .map_err(|err| format!("duplicating that theme did not finish: {err}"))?
}

/// **Open folder** (#2331): the card `theme`'s theme folder, of the level `from`, at the level
/// `at` names, opened in the operating system's file manager. The window names the card; the
/// folder is the one the engine places for it, never a path the window sent.
#[tauri::command]
#[specta::specta]
pub async fn theme_open_folder(
    app: tauri::AppHandle,
    planes: tauri::State<'_, crate::planes::Planes>,
    at: Option<ThemeAt>,
    theme: String,
    from: Option<String>,
) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt as _;
    let level = level_of(from.as_deref())?;
    let (config, plane, workspace) = place_of(&planes, at).await?;
    let asked = theme.clone();
    let folder = tauri::async_runtime::spawn_blocking(move || {
        let place = Place::read(config, plane, workspace);
        gallery::folder_of(&place.shelf(), &asked, level)
    })
    .await
    .map_err(|err| format!("finding that theme's folder did not finish: {err}"))?
    .ok_or_else(|| format!("{theme} has no theme folder here to open"))?;
    app.opener()
        .open_path(folder.display().to_string(), None::<&str>)
        .map_err(|e| format!("the system did not open {theme}'s folder: {e}"))
}

/// **New theme** and **Edit with a chat** (#2333): a chat at the level `at` names, with a prompt
/// that names purlis's theme skill typed into it and never sent (ADR 0061), as the Curate menu's
/// Create a theme opens one. With `theme`, the prompt names that card's theme, of the level
/// `from`; the card is one this level's gallery shows, never a name the window made up. The
/// machine's level has no project for a chat to work in, so it is never asked. A refused start
/// says the approval its profile needs, where it needs one (#3968).
#[tauri::command]
#[specta::specta]
pub async fn theme_chat(
    planes: tauri::State<'_, crate::planes::Planes>,
    at: ThemeAt,
    theme: Option<String>,
    from: Option<String>,
    columns: u16,
    rows: u16,
) -> Result<crate::curation::Curating, crate::curation::StartRefused> {
    let level = level_of(from.as_deref())?;
    let held = planes.held(&at.plane)?;
    let config = config_root()?;
    crate::curation::off_the_window_refused("starting the theme chat", move || {
        let root = held.root().to_path_buf();
        let workspace = at.workspace;
        let chosen = match theme {
            None => themechat::chat(&root, workspace.as_deref(), themechat::Asked::New)?,
            Some(theme) => {
                let place = Place::read(config, Some(root.clone()), workspace.clone());
                let card = gallery::cards(&place.shelf())
                    .into_iter()
                    .find(|card| card.is(&theme, level))
                    .ok_or_else(|| {
                        format!(
                            "{} is not a theme here any more, so no chat was opened",
                            purlis_core::shown::short(&theme)
                        )
                    })?;
                themechat::chat(&root, workspace.as_deref(), themechat::Asked::Edit(&card))?
            }
        };
        let shown = workspace.clone().unwrap_or_else(|| {
            root.file_name().map_or_else(
                || root.display().to_string(),
                |name| name.to_string_lossy().into_owned(),
            )
        });
        crate::curation::open_chosen(
            &held,
            chosen,
            &shown,
            purlis_core::engine::Size { columns, rows },
            // A theme chat is no curation action: its runs are the harness's own pick.
            None,
        )
    })
    .await
}

#[cfg(test)]
mod tests;
