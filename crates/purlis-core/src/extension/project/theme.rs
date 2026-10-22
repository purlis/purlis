// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Which theme a project draws** (charter-app#273, ADR 0048) — the sibling of
//! [`super::resolve`], and asked with its answer — **and, in a workspace, which colour**
//! (charter-app#281).
//!
//! A project picks in `[theme] use`, in `charter.toml` (Shared) or `charter.local.toml` (Local),
//! and a workspace in `settings.theme.use` in its `workspace.json`:
//!
//! - `purlis` or `purlis-classic`, the two built-in themes;
//! - `purlis-dark` or `purlis-light`, Purlis's dark or light (#3034) — or an id either had before
//!   the rename (`charter-*`, [`crate::names::THEME_DARK`]), which is read as the renamed id
//!   (#1838). They were written as the default's dark or light, so they draw the default's;
//! - `system`, which follows the operating system's light or dark appearance;
//! - `<extension-id>/<theme name>`, a theme an extension contributes.
//!
//! # The order
//!
//! 1. **Local**'s value, when it is one of those shapes; else **the workspace**'s; else
//!    **Shared**'s; else nothing, and the window keeps its own theme — exactly what it drew
//!    before a project could pick. A workspace refines its project for the team, and this
//!    machine's Local file has the last word, as it does for extensions (charter-app#280).
//! 2. **An extension's theme only while that extension is on in this project** — which is
//!    [`super::resolve`]'s answer, so this machine's approval still comes first. A pick whose
//!    extension is off, unapproved, not installed, or does not contribute that theme falls back
//!    to the built-in [`FALLBACK`], and [`Resolved::why`] says why in a sentence the settings tab
//!    draws. A project file can name a theme; it can never bring one.
//!
//! A value that is none of the shapes is ignored with a sentence and the next layer down is used,
//! as a setting's is ([`super::Ignored`]).
//!
//! # Option values (#2707)
//!
//! `[theme] options`, and a workspace's `settings.theme.options`, name a value of each of the
//! picked theme's options by the option's id (`options = { accent = "teal" }`), as the person's
//! choice does (`crate::theme::choices`). They belong to the layer's pick: the theme levels draw
//! them over that theme when its layer is the one that chose (`crate::theme::levels`), and an
//! option a layer names no value for draws the theme's default. Options with no pick choose
//! nothing.
//!
//! # The icon theme (FM-3, #1106)
//!
//! `[theme] icons` picks the icons the file trees draw, from the same layers in the same order:
//! `purlis-icons`, purlis's own (or the id it had before the rename, `charter-*`,
//! [`crate::names::ICON_THEME`], read as it, #1854), or `<extension-id>/<icon theme name>`, one
//! an extension contributes. It is a key of its own because the two are picked apart: an icon
//! theme takes its colours from the colour theme's `icon.*` tokens, so either one can change
//! without the other. Nothing picked, or a pick that cannot be drawn, draws [`ICONS_FALLBACK`]
//! ([`resolve_icons`]).
//!
//! # A workspace's colour
//!
//! `settings.theme.colour`, in a workspace's `workspace.json` and nowhere else: one of the
//! [`PALETTE`]'s names, or a colour as `#rrggbb` whose hue is taken. It is what tells one
//! workspace from another, so a project's files cannot set one. It picks no theme: the window
//! tints the accent and the tab shades of the theme it draws with that hue (`theme.ts`), and
//! leaves the text and the terminal alone.

use super::{Effective, Ignored, Source, State, WORKSPACE_AT};
use crate::extension::{BUILT_IN_ICON_THEMES, BUILT_IN_THEMES};
use crate::profiles::{COMMITTED_FILE, LOCAL_FILE};
use crate::settings::Refusal;

/// The table a project's theme is picked in, in either file — and in a workspace's `settings`.
pub const TABLE: &str = "theme";

/// The key in [`TABLE`] that picks the theme.
pub const USE: &str = "use";

/// The key in [`TABLE`] that picks the theme's appearance (spec #2313, #2323): `system`, which
/// follows the operating system, or the id of one of the theme's appearances (`dark`, `day`).
pub const APPEARANCE: &str = "appearance";

/// Whether `value` is an [`APPEARANCE`] a file may hold: `system`, or an appearance's id.
pub fn is_appearance(value: &str) -> bool {
    value == SYSTEM || crate::theme::is_folder_name(value)
}

/// What a value that is not an appearance is told.
pub const NOT_AN_APPEARANCE: &str = "which is not system or the id of one of the theme's \
                                     appearances";

/// The key in [`TABLE`] that names a value of each of the picked theme's options (#2707): a table
/// of option ids, each with one of its values' ids.
pub const OPTIONS: &str = "options";

/// The key in [`TABLE`] that picks the icon theme (FM-3, #1106).
pub const ICONS: &str = "icons";

/// The icon theme the trees draw when nothing picked one, or the pick cannot be drawn.
pub const ICONS_FALLBACK: &str = crate::names::ICON_THEME.write;

/// The key in a workspace's [`TABLE`] that gives it a colour (charter-app#281).
pub const COLOUR: &str = "colour";

/// The value that follows the operating system's appearance.
pub const SYSTEM: &str = "system";

/// The built-in a pick that cannot be drawn falls back to: the default theme, Purlis (#3034).
pub const FALLBACK: &str = crate::theme::DEFAULT;

/// The colours a workspace may name (charter-app#281). Each is a hue the window tints with;
/// `theme.ts` holds the hues, and a test there holds the two lists to one another.
pub const PALETTE: [&str; 8] = [
    "red", "orange", "yellow", "green", "teal", "blue", "purple", "pink",
];

/// What a project can pick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pick {
    /// One of [`BUILT_IN_THEMES`], Purlis's dark and light (#3034), by its purlis id whichever
    /// id the file named it by, or Purlis Classic by its own id ([`crate::theme::CLASSIC`]) or
    /// Purlis by its own ([`crate::theme::PURLIS`], #2336), each of which draws its first
    /// appearance ([`crate::theme::shipped_pick`]).
    BuiltIn(&'static str),
    /// The default theme, Purlis, in its light appearance when the system is light and its dark
    /// one otherwise; the person's own theme in its place when they have one (#2615, #3034).
    System,
    /// A theme an extension contributes, by the extension's id and the theme's name.
    Extension { id: String, name: String },
    /// A theme folder, by its folder's name: the nearest level's `themes/<name>/` (spec #2313,
    /// #2323) — the workspace's, the project's, then the person's own.
    Folder(String),
}

impl Pick {
    /// `value` as a pick, or `None` when it is not one of the shapes.
    ///
    /// A theme's name is held to the rule an extension's manifest holds it to
    /// (`facts::drawable_label`, #1145): no extension can contribute a longer name or one with a
    /// control or invisible formatting character, so a pick of one could never be drawn, and it
    /// would only carry that name into the sentence that says so.
    pub fn parse(value: &str) -> Option<Self> {
        if value == SYSTEM {
            return Some(Self::System);
        }
        if value == crate::theme::CLASSIC {
            return Some(Self::BuiltIn(crate::theme::CLASSIC));
        }
        if value == crate::theme::PURLIS {
            return Some(Self::BuiltIn(crate::theme::PURLIS));
        }
        if let Some(built) = BUILT_IN_THEMES.iter().find(|name| **name == value) {
            return Some(Self::BuiltIn(built));
        }
        // An old id draws the renamed theme (#1838): the name module knows each one's spellings.
        if let Some(renamed) = [crate::names::THEME_DARK, crate::names::THEME_LIGHT]
            .into_iter()
            .find(|name| name.recognises(value))
        {
            return Some(Self::BuiltIn(renamed.write));
        }
        // A theme folder by its name; an icon theme's id, purlis's own or an old one, is never a
        // colour theme's.
        let an_icon_id =
            BUILT_IN_ICON_THEMES.contains(&value) || crate::names::ICON_THEME.recognises(value);
        if crate::theme::is_folder_name(value) && !an_icon_id {
            return Some(Self::Folder(value.to_owned()));
        }
        let (id, name) = value.split_once('/')?;
        (super::id_ok(id)
            && !name.trim().is_empty()
            && crate::extension::facts::drawable_label(name))
        .then(|| Self::Extension {
            id: id.to_owned(),
            name: name.to_owned(),
        })
    }

    /// `value` as an icon theme pick ([`ICONS`]), or `None` when it is not one of its shapes:
    /// one of [`BUILT_IN_ICON_THEMES`], or `<extension-id>/<name>`. There is no `system`: an
    /// icon theme follows the appearance through the colour theme's tokens.
    pub fn parse_icons(value: &str) -> Option<Self> {
        if let Some(built) = BUILT_IN_ICON_THEMES.iter().find(|name| **name == value) {
            return Some(Self::BuiltIn(built));
        }
        // The old id draws the renamed icon theme (#1854), as an old colour theme id does.
        if crate::names::ICON_THEME.recognises(value) {
            return Some(Self::BuiltIn(crate::names::ICON_THEME.write));
        }
        match Self::parse(value)? {
            extension @ Self::Extension { .. } => Some(extension),
            Self::BuiltIn(_) | Self::System | Self::Folder(_) => None,
        }
    }

    /// The pick as a file holds it.
    pub fn value(&self) -> String {
        match self {
            Self::BuiltIn(name) => (*name).to_owned(),
            Self::System => SYSTEM.to_owned(),
            Self::Extension { id, name } => format!("{id}/{name}"),
            Self::Folder(name) => name.clone(),
        }
    }
}

/// A workspace's colour (charter-app#281).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Colour {
    /// One of [`PALETTE`].
    Palette(&'static str),
    /// `#rrggbb`, as written: the window takes its hue.
    Custom(String),
}

impl Colour {
    /// `value` as a colour, or `None` when it is neither a [`PALETTE`] name nor `#rrggbb`.
    pub fn parse(value: &str) -> Option<Self> {
        if let Some(name) = PALETTE.iter().find(|name| **name == value) {
            return Some(Self::Palette(name));
        }
        let digits = value.strip_prefix('#')?;
        (digits.len() == 6 && digits.bytes().all(|b| b.is_ascii_hexdigit()))
            .then(|| Self::Custom(value.to_owned()))
    }

    /// The colour as the file holds it.
    pub fn value(&self) -> String {
        match self {
            Self::Palette(name) => (*name).to_owned(),
            Self::Custom(hex) => hex.clone(),
        }
    }
}

/// One key's value in each layer: Shared, the workspace, Local.
#[derive(Debug, Clone, Default, PartialEq)]
struct Layers {
    shared: Option<toml::Value>,
    workspace: Option<toml::Value>,
    local: Option<toml::Value>,
}

impl Layers {
    /// The layer `source`'s value; a source that is none of the three has none.
    fn of(&self, source: Source) -> Option<&toml::Value> {
        match source {
            Source::Shared => self.shared.as_ref(),
            Source::Local => self.local.as_ref(),
            Source::Workspace => self.workspace.as_ref(),
            Source::Default => None,
        }
    }
}

/// What a project's two files — and, in a workspace, its `workspace.json` — say about its theme:
/// each layer's `use` as found, and the workspace's `colour`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Said {
    shared: Option<toml::Value>,
    /// `None` outside a workspace, and for a workspace whose settings pick nothing.
    workspace: Option<toml::Value>,
    local: Option<toml::Value>,
    /// Each layer's [`ICONS`], as found, in the same order (FM-3).
    icons: Layers,
    /// Each layer's [`APPEARANCE`], as found (#2323).
    appearance: Layers,
    /// Each layer's [`OPTIONS`], as found (#2707).
    options: Layers,
    /// The workspace's `settings.theme.colour`, as found.
    colour: Option<toml::Value>,
    /// The workspace these were read in, when they were.
    workspace_name: Option<String>,
    /// Why `charter.local.toml` is not among these layers, when it is there and git would carry
    /// it (charter-app#319).
    local_left_out: Option<String>,
    /// The theme folders a pick may name (#2323): `None` when the caller did not look, and a
    /// folder's name is taken as it is; else those within the project's reach, and those within
    /// the workspace's.
    folders: Option<Reachable>,
}

/// The theme folders within each layer's reach (#2323, `crate::theme::folders`).
#[derive(Debug, Clone, Default, PartialEq)]
struct Reachable {
    /// What `charter.toml` and `charter.local.toml` may name: the project's and the person's.
    project: Vec<String>,
    /// What a workspace's `settings.theme` may name: the workspace's too.
    workspace: Vec<String>,
}

impl Said {
    /// The two files' text: `None` for a file that is not there.
    pub fn from_text(shared: Option<&str>, local: Option<&str>) -> Self {
        let said = |text: &str, key: &str| {
            text.parse::<toml::Table>()
                .ok()
                .and_then(|top| said_in(&top, key))
        };
        Self {
            shared: shared.and_then(|text| said(text, USE)),
            local: local.and_then(|text| said(text, USE)),
            icons: Layers {
                shared: shared.and_then(|text| said(text, ICONS)),
                local: local.and_then(|text| said(text, ICONS)),
                workspace: None,
            },
            appearance: Layers {
                shared: shared.and_then(|text| said(text, APPEARANCE)),
                local: local.and_then(|text| said(text, APPEARANCE)),
                workspace: None,
            },
            options: Layers {
                shared: shared.and_then(|text| said(text, OPTIONS)),
                local: local.and_then(|text| said(text, OPTIONS)),
                workspace: None,
            },
            ..Self::default()
        }
    }

    /// The same, in the workspace `name` whose `workspace.json` has this text (`None`: it has
    /// none). Its `settings` go between Shared and Local, as `super::Choices::in_workspace`'s do.
    pub fn in_workspace(self, name: &str, manifest: Option<&str>) -> Self {
        self.with_workspace(
            name,
            manifest.and_then(crate::settings::workspace::table_in),
        )
    }

    fn with_workspace(mut self, name: &str, settings: Option<toml::Table>) -> Self {
        self.workspace = settings.as_ref().and_then(|top| said_in(top, USE));
        self.icons.workspace = settings.as_ref().and_then(|top| said_in(top, ICONS));
        self.appearance.workspace = settings.as_ref().and_then(|top| said_in(top, APPEARANCE));
        self.options.workspace = settings.as_ref().and_then(|top| said_in(top, OPTIONS));
        self.colour = settings.as_ref().and_then(|top| said_in(top, COLOUR));
        self.workspace_name = Some(name.to_owned());
        self
    }

    /// The plane at `root`'s two files, as [`crate::settings::layer_text`] hands them: a file
    /// that cannot be read says nothing, and neither does a `charter.local.toml` git would carry
    /// (charter-app#308).
    pub fn read(root: &std::path::Path) -> Self {
        use crate::settings::{Which, layer_text};
        Self::from_layers(
            &layer_text(root, Which::Shared),
            &layer_text(root, Which::Local),
        )
    }

    /// The two files as [`crate::settings::layer_text`] hands them — [`Self::from_text`], and,
    /// when the Local file was left out having said something here, why (charter-app#319).
    pub fn from_layers(
        shared: &crate::settings::LayerText,
        local: &crate::settings::LayerText,
    ) -> Self {
        Self {
            local_left_out: local
                .left_out_where(|top| said_in(top, USE).is_some() || said_in(top, ICONS).is_some()),
            ..Self::from_text(shared.text(), local.text())
        }
    }

    /// Why `charter.local.toml` is not among what the files say, when it is there, git would carry it,
    /// and it said something here: the ignore check's sentence, which the Settings tab says at the Project level too
    /// (charter-app#319). A settings tab says it in every group that shows these in force, so a
    /// value set in Local and not applied is never shown without its reason.
    pub fn local_left_out(&self) -> Option<&str> {
        self.local_left_out.as_deref()
    }

    /// [`Self::read`], in `workspace` when there is one — `super::Choices::read_in`'s twin, so
    /// the theme and the extensions it depends on are read in the same place.
    pub fn read_in(root: &std::path::Path, workspace: Option<&str>) -> Self {
        let project = Self::read(root);
        match workspace {
            None => project,
            Some(name) => {
                project.with_workspace(name, crate::settings::workspace::read(root, name))
            }
        }
    }

    /// What the layer `source` picks, as text: its [`USE`] and its [`APPEARANCE`], each `None`
    /// when the layer does not say it or says something that is not a string. The theme levels
    /// read these (`crate::theme::levels`, #2323); a layer other than Shared, Local and the
    /// workspace says nothing.
    pub fn picked_in(&self, source: Source) -> (Option<&str>, Option<&str>) {
        let used = match source {
            Source::Shared => self.shared.as_ref(),
            Source::Local => self.local.as_ref(),
            Source::Workspace => self.workspace.as_ref(),
            _ => None,
        };
        (
            used.and_then(toml::Value::as_str),
            self.appearance.of(source).and_then(toml::Value::as_str),
        )
    }

    /// **The option values the layer `source` names** (#2707), as the person's choice holds them
    /// ([`crate::theme::choices::values_from`]): each whose option and value are ids, at most as
    /// many as a theme may have options; anything else is left out, as is a value that is not a
    /// table. A layer other than Shared, Local and the workspace names none.
    pub fn options_in(&self, source: Source) -> crate::theme::Values {
        let table = self.options.of(source).and_then(toml::Value::as_table);
        crate::theme::choices::values_from(
            table
                .into_iter()
                .flatten()
                .map(|(id, value)| (id.as_str(), value.as_str())),
        )
    }

    /// The same, knowing which theme folders each layer may name: a pick of a folder no level
    /// within its reach has is passed over with a sentence, as a value that is not a pick is.
    #[must_use]
    pub fn with_folders(mut self, project: Vec<String>, workspace: Vec<String>) -> Self {
        self.folders = Some(Reachable { project, workspace });
        self
    }

    /// Whether the layer `source` may name the theme folder `name`.
    fn reaches(&self, source: Source, name: &str) -> bool {
        let Some(folders) = &self.folders else {
            return true;
        };
        let within = match source {
            Source::Workspace => &folders.workspace,
            _ => &folders.project,
        };
        within.iter().any(|one| one == name)
    }

    /// The workspace these were read in, or `None` outside a workspace.
    pub fn workspace_name(&self) -> Option<&str> {
        self.workspace_name.as_deref()
    }

    /// The workspace's file as a sentence names it, or `None` outside a workspace.
    pub fn workspace_file(&self) -> Option<String> {
        self.workspace_name
            .as_deref()
            .map(crate::settings::workspace::named)
    }
}

/// `[theme] <key>` in `top`: a whole TOML file, or a workspace's `settings` read as one.
fn said_in(top: &toml::Table, key: &str) -> Option<toml::Value> {
    top.get(TABLE)?.as_table()?.get(key).cloned()
}

/// One theme an approved extension contributes, as a survey found it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offered {
    pub id: String,
    pub name: String,
}

/// A project's theme, in a workspace when it was asked in one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    /// What the layers picked, in force: `None` when none picked anything usable.
    pub picked: Option<Pick>,
    /// Which layer [`Self::picked`] came from.
    pub source: Source,
    /// What the window draws for this project: [`Self::picked`], or the built-in [`FALLBACK`]
    /// when that cannot be drawn. `None` leaves the window its own theme.
    pub draws: Option<Pick>,
    /// Why [`Self::draws`] is not [`Self::picked`], when it is not.
    pub why: Option<String>,
    /// The workspace's colour, when it has one charter reads (charter-app#281).
    pub colour: Option<Colour>,
    /// Each value a layer set that charter did not use, and why.
    pub ignored: Vec<Ignored>,
}

/// **The theme, in one place**: Local's pick over the workspace's over Shared's, and an
/// extension's only while `extensions` — [`super::resolve`]'s answer for the same project and
/// workspace — has it on. And the workspace's colour, which no other layer has.
///
/// `offered` is every theme the approved extensions contribute, when the caller surveyed them;
/// with it, a pick an extension does not contribute falls back too. Without it (the window's
/// cheap question, which reads no extension's directory) that is left to the window, which only
/// ever holds what a survey found.
pub fn resolve(extensions: &[Effective], offered: Option<&[Offered]>, said: &Said) -> Resolved {
    let mut ignored = Vec::new();
    let workspace_file = said.workspace_file().unwrap_or_default();
    let colour = colour(said, &workspace_file, &mut ignored);
    let layers = Layers {
        shared: said.shared.clone(),
        workspace: said.workspace.clone(),
        local: said.local.clone(),
    };
    Resolved {
        colour,
        ..resolve_key(Key::Colours, &layers, extensions, offered, said, ignored)
    }
}

/// **The icon theme, in one place** (FM-3, #1106): [`resolve`]'s order and rules for
/// `[theme] icons`. `offered` is every icon theme the approved extensions contribute, when the
/// caller surveyed them. [`Resolved::draws`] `None` draws [`ICONS_FALLBACK`]; a pick that cannot
/// be drawn draws it too, with [`Resolved::why`]. A workspace's colour is not an icon theme's,
/// so [`Resolved::colour`] is always `None` here.
pub fn resolve_icons(
    extensions: &[Effective],
    offered: Option<&[Offered]>,
    said: &Said,
) -> Resolved {
    resolve_key(
        Key::Icons,
        &said.icons,
        extensions,
        offered,
        said,
        Vec::new(),
    )
}

/// Which of `[theme]`'s two picks is being resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Key {
    Colours,
    Icons,
}

impl Key {
    fn name(self) -> &'static str {
        match self {
            Self::Colours => USE,
            Self::Icons => ICONS,
        }
    }

    fn parse(self, value: &str) -> Option<Pick> {
        match self {
            Self::Colours => Pick::parse(value),
            Self::Icons => Pick::parse_icons(value),
        }
    }

    fn not_a_pick(self) -> &'static str {
        match self {
            Self::Colours => NOT_A_PICK,
            Self::Icons => NOT_AN_ICON_PICK,
        }
    }

    fn fallback(self) -> &'static str {
        match self {
            Self::Colours => FALLBACK,
            Self::Icons => ICONS_FALLBACK,
        }
    }

    /// [`Self::fallback`] as a sentence names it: the colour theme by its name, the icon theme
    /// by its id.
    fn fallback_said(self) -> &'static str {
        match self {
            Self::Colours => "theme, Purlis,",
            Self::Icons => ICONS_FALLBACK,
        }
    }

    /// What a contribution of this kind is called.
    fn noun(self) -> &'static str {
        match self {
            Self::Colours => "theme",
            Self::Icons => "icon theme",
        }
    }

    /// What is drawn when no layer picks anything usable.
    fn when_none(self) -> &'static str {
        match self {
            Self::Colours => "the window keeps its own theme",
            Self::Icons => "purlis's own icons are drawn",
        }
    }
}

/// [`resolve`] and [`resolve_icons`]: the first usable pick of `layers`, Local first.
fn resolve_key(
    key: Key,
    layers: &Layers,
    extensions: &[Effective],
    offered: Option<&[Offered]>,
    said: &Said,
    mut ignored: Vec<Ignored>,
) -> Resolved {
    let workspace_file = said.workspace_file().unwrap_or_default();
    // The theme levels' order (#2323): the workspace's, then the project's local file (the
    // person's until it is mapped), then `charter.toml`.
    let mut candidates = [
        (
            Source::Workspace,
            workspace_file.as_str(),
            WORKSPACE_AT,
            &layers.workspace,
        ),
        (Source::Local, LOCAL_FILE, "", &layers.local),
        (Source::Shared, COMMITTED_FILE, "", &layers.shared),
    ]
    .into_iter()
    .filter_map(|(source, file, at, value)| value.as_ref().map(|value| (source, file, at, value)))
    .peekable();
    let mut picked = None;
    while let Some((source, file, at, value)) = candidates.next() {
        let pick = value.as_str().and_then(|value| key.parse(value));
        let reached = match &pick {
            Some(Pick::Folder(name)) => said.reaches(source, name),
            _ => true,
        };
        let unreached = pick.is_some() && !reached;
        if let Some(pick) = pick.filter(|_| reached) {
            picked = Some((pick, source, file));
            break;
        }
        let instead = match candidates.peek() {
            Some((_, next, _, _)) => format!("{next}'s pick is used"),
            None => key.when_none().to_owned(),
        };
        ignored.push(Ignored {
            source,
            why: format!(
                "{file} sets {at}{TABLE}.{} to {value}, {} — so {instead}",
                key.name(),
                if unreached {
                    NO_SUCH_FOLDER
                } else {
                    key.not_a_pick()
                }
            ),
        });
    }
    let Some((pick, source, file)) = picked else {
        return Resolved {
            picked: None,
            source: Source::Default,
            draws: None,
            why: None,
            colour: None,
            ignored,
        };
    };
    let why = unavailable(key, &pick, file, extensions, offered);
    Resolved {
        draws: Some(if why.is_some() {
            Pick::BuiltIn(key.fallback())
        } else {
            pick.clone()
        }),
        picked: Some(pick),
        source,
        why,
        colour: None,
        ignored,
    }
}

/// The workspace's colour, saying why a value that is not one was passed over.
fn colour(said: &Said, file: &str, ignored: &mut Vec<Ignored>) -> Option<Colour> {
    let value = said.colour.as_ref()?;
    let colour = value.as_str().and_then(Colour::parse);
    if colour.is_none() {
        ignored.push(Ignored {
            source: Source::Workspace,
            why: format!(
                "{file} sets {WORKSPACE_AT}{TABLE}.{COLOUR} to {value}, {} — so this workspace \
                 has no colour",
                not_a_colour()
            ),
        });
    }
    colour
}

/// The colour of the workspace `ws`, as [`resolve`] reads it — for the window's workspace tabs,
/// which each show their own. `None` for a workspace with no colour, or one charter does not
/// read. Handed the workspace the caller already opened, so a listing of every workspace reads
/// each manifest once.
pub fn colour_of(ws: &crate::workspaces::Workspace) -> Option<Colour> {
    colour_of_manifest(ws.name(), ws.manifest().0.as_ref()?)
}

/// The colour the workspace `name`'s manifest `doc` gives it, as [`colour_of`] reads it, for a
/// caller that read the manifest itself.
pub fn colour_of_manifest(name: &str, doc: &serde_json::Value) -> Option<Colour> {
    let settings = crate::settings::workspace::table_of(doc);
    let said = Said::default().with_workspace(name, settings);
    colour(&said, "", &mut Vec::new())
}

/// What a value that is not a pick is told.
pub const NOT_A_PICK: &str = "which is not purlis-dark, purlis-light, system, a theme \
                              folder's name or <extension>/<theme>";

/// What a pick of a theme folder no level within its reach has is told (#2323).
pub const NO_SUCH_FOLDER: &str = "which names no theme folder this layer reaches";

/// What a value that is not an icon theme pick is told.
pub const NOT_AN_ICON_PICK: &str = "which is not purlis-icons or <extension>/<icon theme>";

/// What a value that is not a colour is told.
fn not_a_colour() -> String {
    format!("which is not {} or #rrggbb", PALETTE.join(", "))
}

/// **Whether `pick` can be drawn** in a project whose extensions are `extensions`, as far as the
/// record says: an extension's theme only while its extension is on there (#2323's levels ask
/// this of whichever level chose).
pub fn drawable(extensions: &[Effective], pick: &Pick) -> bool {
    unavailable(Key::Colours, pick, "", extensions, None).is_none()
}

/// Why `pick`, which `file` made, cannot be drawn in this project, or `None` when it can.
fn unavailable(
    key: Key,
    pick: &Pick,
    file: &str,
    extensions: &[Effective],
    offered: Option<&[Offered]>,
) -> Option<String> {
    let Pick::Extension { id, name } = pick else {
        return None;
    };
    let it = extensions.iter().find(|it| &it.id == id);
    let because = match it.map(|it| (it.state, it.source)) {
        Some((State::On, _)) => {
            let contributes =
                offered.is_none_or(|all| all.iter().any(|one| &one.id == id && &one.name == name));
            if contributes {
                return None;
            }
            format!("{id} contributes no {} called “{name}”", key.noun())
        }
        // Off in the workspace's layer is off there, not in the project (charter-app#281).
        Some((State::Off, Source::Workspace)) => format!("{id} is off in this workspace"),
        Some((State::Off, _)) => format!("{id} is off in this project"),
        Some((State::NeedsApproval, _)) => format!("this machine has not approved {id}"),
        Some((State::NotInstalled, _)) | None => {
            format!("{id} is not installed on this machine")
        }
    };
    Some(format!(
        "{file} picks “{name}” from {id}, but {because} — so the built-in {} is drawn",
        key.fallback_said()
    ))
}

/// Everything in `text`'s `[theme]` that charter would not read, as `file` holds it, one sentence
/// each — what the Settings tab refuses to save. Empty when the text is not TOML at all:
/// that is the file's own reader's refusal.
pub fn refusals(text: &str, file: &str) -> Vec<String> {
    keyed(text, file).into_iter().map(|one| one.why).collect()
}

/// [`refusals`], each with the key it is about (#1292).
pub fn keyed(text: &str, file: &str) -> Vec<Refusal> {
    let Ok(top) = text.parse::<toml::Table>() else {
        return Vec::new();
    };
    let Some(table) = top.get(TABLE) else {
        return Vec::new();
    };
    let Some(table) = table.as_table() else {
        return vec![Refusal::at(
            format!("{TABLE} in {file} is not a table — write [{TABLE}] with {USE} = \"<theme>\""),
            &[TABLE],
        )];
    };
    let mut out = Vec::new();
    for (key, value) in table {
        let at = [TABLE, key.as_str()];
        match key.as_str() {
            USE if value.as_str().and_then(Pick::parse).is_none() => {
                out.push(Refusal::at(
                    format!("{TABLE}.{USE} in {file} is {value}, {NOT_A_PICK}"),
                    &at,
                ));
            }
            ICONS if value.as_str().and_then(Pick::parse_icons).is_none() => {
                out.push(Refusal::at(
                    format!("{TABLE}.{ICONS} in {file} is {value}, {NOT_AN_ICON_PICK}"),
                    &at,
                ));
            }
            APPEARANCE if !value.as_str().is_some_and(is_appearance) => {
                out.push(Refusal::at(
                    format!("{TABLE}.{APPEARANCE} in {file} is {value}, {NOT_AN_APPEARANCE}"),
                    &at,
                ));
            }
            OPTIONS => out.extend(
                options_refused(value, &format!("{TABLE}.{OPTIONS} in {file}"), |option| {
                    format!("{TABLE}.{OPTIONS}.{option} in {file}")
                })
                .into_iter()
                .map(|(why, option)| match option {
                    Some(option) => Refusal::at(why, &[TABLE, OPTIONS, option]),
                    None => Refusal::at(why, &[TABLE, OPTIONS]),
                }),
            ),
            USE | APPEARANCE | ICONS => {}
            _ => {
                let colour = if key == COLOUR {
                    "; a colour is a workspace's"
                } else {
                    ""
                };
                out.push(Refusal::at(
                    format!(
                        "{TABLE}.{} in {file} is not read — [{TABLE}] holds {USE}, {APPEARANCE}, \
                         {OPTIONS} and {ICONS} and nothing else{colour}",
                        toml_edit::Key::new(key.as_str()).display_repr()
                    ),
                    &at,
                ));
            }
        }
    }
    out
}

/// [`refusals`], for a workspace's `settings` read as the table they mirror
/// (`crate::settings::workspace`), where the theme may also hold a [`COLOUR`]. `file` is the
/// manifest as a sentence names it; each key is named by its path in the JSON.
pub fn refusals_in_workspace(top: &toml::Table, file: &str) -> Vec<String> {
    keyed_in_workspace(top, file)
        .into_iter()
        .map(|one| one.why)
        .collect()
}

/// [`refusals_in_workspace`], each with the key it is about under the workspace's `settings`
/// (#1292), as a setting at that level holds it.
pub fn keyed_in_workspace(top: &toml::Table, file: &str) -> Vec<Refusal> {
    let Some(table) = top.get(TABLE) else {
        return Vec::new();
    };
    let Some(table) = table.as_table() else {
        return vec![Refusal::at(
            format!(
                "{WORKSPACE_AT}{TABLE} in {file} is not an object — write \
                 {{\"{USE}\": \"<theme>\", \"{COLOUR}\": \"<colour>\"}}"
            ),
            &[TABLE],
        )];
    };
    let mut out = Vec::new();
    for (key, value) in table {
        let at = format!("{WORKSPACE_AT}{TABLE}.{key} in {file}");
        let said = match key.as_str() {
            USE if value.as_str().and_then(Pick::parse).is_none() => {
                format!("{at} is {value}, {NOT_A_PICK}")
            }
            COLOUR if value.as_str().and_then(Colour::parse).is_none() => {
                format!("{at} is {value}, {}", not_a_colour())
            }
            ICONS if value.as_str().and_then(Pick::parse_icons).is_none() => {
                format!("{at} is {value}, {NOT_AN_ICON_PICK}")
            }
            APPEARANCE if !value.as_str().is_some_and(is_appearance) => {
                format!("{at} is {value}, {NOT_AN_APPEARANCE}")
            }
            OPTIONS => {
                let named =
                    |option: &str| format!("{WORKSPACE_AT}{TABLE}.{OPTIONS}.{option} in {file}");
                for (why, option) in options_refused(value, &at, named) {
                    let path: &[&str] = match option {
                        Some(option) => &[TABLE, OPTIONS, option],
                        None => &[TABLE, OPTIONS],
                    };
                    out.push(Refusal::at(why, path));
                }
                continue;
            }
            USE | APPEARANCE | COLOUR | ICONS => continue,
            _ => format!(
                "{at} is not read — a workspace's {TABLE} holds {USE}, {APPEARANCE}, {OPTIONS}, \
                 {ICONS} and {COLOUR} and nothing else"
            ),
        };
        out.push(Refusal::at(said, &[TABLE, key.as_str()]));
    }
    out
}

/// What is refused in an [`OPTIONS`] value, `at` naming the key and `named` each option's key
/// in a sentence: not a table; more options than a theme may have; an option whose id is not
/// one; a value that is not an id. Each with the option it is about, when it is about one.
///
/// What the file holds is quoted as a sentence may show it: each option's key as TOML writes it
/// (`display_repr`), so no character of it is drawn raw, and each value escaped and cut to
/// [`QUOTED`].
fn options_refused<'a>(
    value: &'a toml::Value,
    at: &str,
    named: impl Fn(&str) -> String,
) -> Vec<(String, Option<&'a str>)> {
    use crate::theme::choices::{MOST_VALUES, is_id};
    let quoted = |value: &toml::Value| crate::shown::readable(&value.to_string(), QUOTED);
    let named = |option: &str| named(&toml_edit::Key::new(option).display_repr());
    let Some(table) = value.as_table() else {
        return vec![(
            format!(
                "{at} is {}, which is not a table of option ids and their values",
                quoted(value)
            ),
            None,
        )];
    };
    let mut out = Vec::new();
    if table.len() > MOST_VALUES {
        out.push((
            format!(
                "{at} names {} options, more than the {MOST_VALUES} a theme may have",
                table.len()
            ),
            None,
        ));
        return out;
    }
    for (option, given) in table {
        if !is_id(option) {
            out.push((
                format!(
                    "{} is {} for an option whose id is not one — an option and its value are \
                     each named by their id",
                    named(option),
                    quoted(given)
                ),
                Some(option.as_str()),
            ));
        } else if !given.as_str().is_some_and(is_id) {
            out.push((
                format!(
                    "{} is {}, which is not the id of one of the option's values",
                    named(option),
                    quoted(given)
                ),
                Some(option.as_str()),
            ));
        }
    }
    out
}

/// The most of a value from a file an options refusal quotes.
const QUOTED: usize = 80;

#[cfg(test)]
mod tests;
