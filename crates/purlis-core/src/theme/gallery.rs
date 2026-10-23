// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Settings › Theme's gallery** (spec #2313, #2331): every theme a choice at one level may
//! name, as a card, with what the engine says of each; and the two things a card does to the
//! person's own themes, **Duplicate as my theme** and **Open folder**.
//!
//! # A card per theme within reach
//!
//! A level's gallery holds the themes a choice made there reaches ([`Reach`]): Purlis, the
//! default (#3034), and Purlis Classic; the theme folders of that level and of the levels farther out,
//! nearest first, each with its level, so two folders of one name at two levels are two cards a
//! choice tells apart (#2516); and the themes of the extensions in force there. So the machine's
//! gallery is the built-in themes, the person's own folders and every approved extension's themes;
//! a project's adds the project's folders and keeps to the extensions it has on; a workspace's adds
//! the workspace's.
//!
//! Each card says what the engine read: the theme's name, its appearances and options, whether
//! it is refused and why, what was left out of it, the check's warnings in every combination of
//! appearance and option values ([`super::check_every`], in [`super::warnings_said`]'s words),
//! the consent floors it breaks, and what following the system does with it.
//!
//! # A card's option values
//!
//! [`drawn`] draws a card with option values, built onto the theme before it is drawn
//! ([`super::Theme::with`]), so the consent floors hold for every value; [`choose_option`] is
//! what the person's choice at the level becomes when they set one of its theme's options.
//!
//! # Duplicate as my theme
//!
//! [`duplicate`] makes a theme folder of the person's own (`themes/` in purlis's config folder)
//! whose `theme.json` names the card's theme as the one it `extends`, and nothing else: a theme
//! that starts as its parent and holds only what the person changes. A person's folder reaches
//! the person's own folders and the built-in themes, so those are the cards it is offered on
//! ([`Card::duplicable`]). It never writes over anything: the folder is made new, under a name
//! no folder there has, and every step to it is opened refusing a link.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use super::folders::{Folders, Found, Reach};
use super::levels::Choice;
use super::levels::appear;
use super::resolve::{Drawn, Offered, On};
use super::{
    Appearance, CLASSIC, Contrast, PURLIS, Resolved, SCHEMA_URL, THEME_JSON, ThemeOption, Values,
    check_every, classic, is_folder_name, purlis, shipped,
};
use crate::extension::project::theme::Pick;

/// **Where a card's theme comes from.**
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum From {
    /// Compiled into purlis: Purlis Classic, or Purlis.
    BuiltIn,
    /// A theme folder of the person's own, on this machine.
    Machine,
    /// A theme folder of the project's, at the plane root.
    Project,
    /// A theme folder of the workspace's.
    Workspace,
    /// An approved extension, by its id.
    Extension { extension: String },
}

/// One of a card's appearances.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CardAppearance {
    /// Its id, which a choice names it by.
    pub id: String,
    pub appearance: Appearance,
    pub high_contrast: bool,
}

/// **One theme of a gallery**, as the engine read it.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Card {
    /// What a choice names it by: `purlis-classic`, a theme folder's name, or an extension's
    /// `<extension>/<theme>`.
    pub id: String,
    /// What a person calls it.
    pub name: String,
    pub from: From,
    /// Whether it ships as a preview (spec #2313, "Preview theme"; [`super::is_preview`]). The
    /// card says so beside its name.
    pub preview: bool,
    /// Its appearances in its order: the first is what it draws when nothing chose one. A
    /// refused theme has none of its own.
    pub appearances: Vec<CardAppearance>,
    /// Its options, in its order.
    pub options: Vec<ThemeOption>,
    /// Whether the engine refused it, so a choice of it draws Purlis.
    pub refused: bool,
    /// Why it is refused, or what was left out of a theme it draws: one sentence each.
    pub said: Vec<String>,
    /// The theme check's warnings, one line each, in every combination of appearance and
    /// option values.
    pub warnings: Vec<String>,
    /// The consent floors an appearance of it breaks, with what purlis draws instead: one
    /// sentence each.
    pub floors: Vec<String>,
    /// What following the system does with it, when it has a dark and a light appearance
    /// `None`; else the sentence that says following stays in its one tag.
    pub follows: Option<String>,
    /// Whether **Duplicate as my theme** is offered: a folder of the person's own can extend it.
    pub duplicable: bool,
    /// Whether it has a theme folder **Open folder** shows.
    pub folder: bool,
    /// Changes whenever anything the theme draws changes: what a picture of it is kept by.
    pub stamp: String,
}

impl Card {
    /// **The level of the theme folder it is** (#2516); `None` for a card that is no folder.
    pub fn level(&self) -> Option<Reach> {
        match self.from {
            From::Workspace => Some(Reach::Workspace),
            From::Project => Some(Reach::Project),
            From::Machine => Some(Reach::Machine),
            From::BuiltIn | From::Extension { .. } => None,
        }
    }

    /// **Whether this is the card `id` of the level `level`**: a theme folder's card is told
    /// from another of its name by its level, where one is named.
    pub fn is(&self, id: &str, level: Option<Reach>) -> bool {
        self.id == id && (level.is_none() || self.level() == level)
    }
}

/// **What a gallery is read from**: the levels' folders, how far a choice at its level reaches,
/// whose extension themes are in force, and those themes.
#[derive(Debug, Clone, Copy)]
pub struct Shelf<'a> {
    pub folders: &'a Folders,
    pub reach: Reach,
    pub on: On<'a>,
    pub offered: &'a [Offered],
}

/// **Every theme a choice within `shelf`'s reach may name**, a card each: Purlis, the default,
/// first and Purlis Classic, the built-in themes; then the theme folders nearest level first and by name within one, a name two levels have
/// once at each (#2516), then each extension's theme in force in the order the survey found
/// them.
pub fn cards(shelf: &Shelf<'_>) -> Vec<Card> {
    let mut cards: Vec<Card> = [(PURLIS, purlis()), (CLASSIC, classic())]
        .into_iter()
        .map(|(id, theme)| {
            card(
                id.to_owned(),
                From::BuiltIn,
                &Resolved {
                    theme: theme.clone(),
                    complaints: Vec::new(),
                    refused: false,
                },
            )
        })
        .collect();
    for (level, name) in shelf.folders.each(shelf.reach) {
        // A folder whose name a pick reads as something else (an old built-in id) is no
        // folder a choice can name.
        if !matches!(Pick::parse(&name), Some(Pick::Folder(_))) {
            continue;
        }
        let Found::Read(resolved) = shelf.folders.find_in(&name, level) else {
            continue;
        };
        cards.push(card(name, from_level(level), &resolved));
    }
    for one in shelf
        .offered
        .iter()
        .filter(|one| shelf.on.has(&one.extension))
    {
        let id = Pick::Extension {
            id: one.extension.clone(),
            name: one.name.clone(),
        }
        .value();
        cards.push(card(
            id,
            From::Extension {
                extension: one.extension.clone(),
            },
            &one.theme,
        ));
    }
    cards
}

/// **The theme the card `id` names**, as the engine reads it within `shelf`'s reach: for a theme
/// folder, the folder of `level` when one is named, else the nearest within reach that has the
/// name, as a choice reads it. `None` when no card of the gallery has that id.
pub fn theme_of(shelf: &Shelf<'_>, id: &str, level: Option<Reach>) -> Option<Resolved> {
    match Pick::parse(id)? {
        Pick::BuiltIn(id) => Some(Resolved {
            theme: shipped(id).unwrap_or_else(classic).clone(),
            complaints: Vec::new(),
            refused: false,
        }),
        Pick::Folder(name) => match level {
            Some(level) if level < shelf.reach => None,
            Some(level) => match shelf.folders.find_in(&name, level) {
                Found::Read(resolved) => Some(resolved),
                Found::Missing(_) => None,
            },
            None => match shelf.folders.find(&name, shelf.reach) {
                Found::Read(resolved) => Some(resolved),
                Found::Missing(_) => None,
            },
        },
        Pick::Extension { id, name } => shelf
            .offered
            .iter()
            .filter(|one| shelf.on.has(&one.extension))
            .find(|one| one.extension == id && one.name == name)
            .map(|one| one.theme.clone()),
        Pick::System => None,
    }
}

/// **What the card `id` of `level` draws in `appearance`** with the option values `values`:
/// one of its appearances by id, `system` to follow `system`, or `None` for its first, with
/// `contrast` as the operating system asks it. The values are built onto the theme
/// ([`super::Theme::with`]) before it is drawn, so the consent floors are held over them, as
/// every theme drawn is. `None` when no card of the gallery has that id. A refused theme draws
/// Purlis.
pub fn drawn(
    shelf: &Shelf<'_>,
    id: &str,
    level: Option<Reach>,
    appearance: Option<&str>,
    values: &Values,
    system: Appearance,
    contrast: Contrast,
) -> Option<Drawn> {
    let resolved = theme_of(shelf, id, level)?;
    let (theme, left_out) = resolved.theme.with(values);
    let drawn = appear(&theme, appearance, system, contrast);
    Some(Drawn {
        complaints: [resolved.complaints, left_out, drawn.complaints.clone()].concat(),
        ..drawn
    })
}

/// **What the person's choice at `shelf`'s level becomes when they set `option` to `value`**
/// (#2527): `base` (their choice there, or the choice that draws the theme there when they
/// have none) with that value, or with none (`None`), so the theme's default is drawn. The
/// choice names its folder's level from now on ([`Choice::placed`]). Values `base` holds that
/// its theme no longer has are dropped, with a sentence each.
///
/// Refused, keeping nothing, when the theme is not one the gallery has or is refused, or when it
/// has no option `option` or no value `value` of it.
pub fn choose_option(
    shelf: &Shelf<'_>,
    base: &Choice,
    option: &str,
    value: Option<&str>,
) -> Result<(Choice, Vec<String>), String> {
    let resolved = theme_of(shelf, &base.theme, base.from)
        .ok_or_else(|| format!("{} is no theme here, so nothing was kept", base.theme))?;
    if resolved.refused {
        return Err(format!(
            "{} is refused, so it has no options to choose",
            base.theme
        ));
    }
    let theme = &resolved.theme;
    let Some(chosen) = theme.options.iter().find(|one| one.id == option) else {
        return Err(format!(
            "{} has no option {option}, so nothing was kept",
            theme.name
        ));
    };
    if let Some(value) = value
        && !chosen.values.iter().any(|one| one.id == value)
    {
        return Err(format!(
            "the option {option} of {} has no value {value}, so nothing was kept",
            theme.name
        ));
    }
    let mut notes = Vec::new();
    let mut values = Values::new();
    for (id, given) in &base.options {
        match theme.options.iter().find(|one| one.id == *id) {
            None => notes.push(format!(
                "{} has no option {id} any more, so your value for it was dropped.",
                theme.name
            )),
            Some(one) if !one.values.iter().any(|it| it.id == *given) => notes.push(format!(
                "The option {id} of {} has no value {given} any more, so it was dropped and its \
                 default is drawn.",
                theme.name
            )),
            Some(_) => {
                values.insert(id.clone(), given.clone());
            }
        }
    }
    match value {
        Some(value) => {
            values.insert(option.to_owned(), value.to_owned());
        }
        None => {
            values.remove(option);
        }
    }
    let choice = base
        .clone()
        .placed(shelf.folders, shelf.reach)
        .with_options(values);
    Ok((choice, notes))
}

/// **The theme folder the card `id` of `level` is**, for **Open folder**: that level's folder
/// when one is named, else the nearest level within reach that has a folder of its name, as a
/// choice reads it. `None` for a card with no folder of its own (a built-in theme, an extension's
/// theme) and for a name no level within reach has.
pub fn folder_of(shelf: &Shelf<'_>, id: &str, level: Option<Reach>) -> Option<PathBuf> {
    let Some(Pick::Folder(name)) = Pick::parse(id) else {
        return None;
    };
    let level = shelf
        .folders
        .each(shelf.reach)
        .into_iter()
        .find(|(at, one)| *one == name && level.is_none_or(|level| level == *at))
        .map(|(level, _)| level)?;
    let path = shelf.folders.at(level)?.path().join(&name);
    // Read refusing a link when the gallery was read; asked again now, as the folder is shown.
    std::fs::symlink_metadata(&path)
        .is_ok_and(|it| it.file_type().is_dir())
        .then_some(path)
}

/// What a card's level is called.
fn from_level(level: Reach) -> From {
    match level {
        Reach::Workspace => From::Workspace,
        Reach::Project => From::Project,
        Reach::Machine => From::Machine,
    }
}

/// The card for `resolved`, the theme `id` from `from`.
fn card(id: String, from: From, resolved: &Resolved) -> Card {
    let said: Vec<String> = resolved
        .complaints
        .iter()
        .map(|one| one.said.clone())
        .collect();
    let duplicable = !resolved.refused && matches!(from, From::BuiltIn | From::Machine);
    let folder = matches!(from, From::Machine | From::Project | From::Workspace);
    let preview = matches!(from, From::BuiltIn) && super::is_preview(&id);
    if resolved.refused {
        return Card {
            name: id.clone(),
            stamp: stamp_of(&said),
            id,
            from,
            preview,
            appearances: Vec::new(),
            options: Vec::new(),
            refused: true,
            said,
            warnings: Vec::new(),
            floors: Vec::new(),
            follows: None,
            duplicable,
            folder,
        };
    }
    let theme = &resolved.theme;
    let floors = theme
        .appearances
        .iter()
        .filter_map(|one| super::draw(one).held.map(|held| held.said()))
        .collect();
    Card {
        id,
        name: theme.name.clone(),
        from,
        preview,
        appearances: theme
            .appearances
            .iter()
            .map(|one| CardAppearance {
                id: one.id.clone(),
                appearance: one.appearance,
                high_contrast: one.high_contrast,
            })
            .collect(),
        options: theme.options.clone(),
        refused: false,
        warnings: super::warnings_said(theme, &check_every(theme)),
        floors,
        follows: super::follow(theme, Appearance::Dark, Contrast::Standard).note,
        duplicable,
        folder,
        stamp: stamp_of(theme),
        said,
    }
}

/// A stamp of what `drawn` is: the same for the same theme, another for any change.
fn stamp_of(drawn: &impl serde::Serialize) -> String {
    let mut hasher = DefaultHasher::new();
    serde_json::to_string(drawn)
        .unwrap_or_default()
        .hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// The most a name for a theme folder may be: the grammar's.
const MOST_NAME: usize = 32;

/// How a duplicate's folder is named after its parent's id.
const MINE: &str = "mine";

/// **Duplicate as my theme** (#2331): a theme folder of the person's own, in `themes/` in
/// purlis's config folder under `config_root`, whose `theme.json` names the card `id` as the
/// theme it `extends` and is called `<its name> (mine)`. Answers the new folder's name, which a
/// choice names it by.
///
/// Offered only on a card a folder of the person's own can extend ([`Card::duplicable`]):
/// the built-in themes and the person's own folders. Nothing is written over: the folder is made
/// new under the first name of `<id>-mine`, `<id>-mine-2`, … that no folder there has, and every
/// step to it is opened refusing a link.
pub fn duplicate(
    config_root: &Path,
    shelf: &Shelf<'_>,
    id: &str,
    level: Option<Reach>,
) -> Result<String, String> {
    let card = cards(shelf)
        .into_iter()
        .find(|one| one.is(id, level))
        .ok_or_else(|| format!("there is no theme {id} here, so nothing was made"))?;
    if !card.duplicable {
        return Err(format!(
            "{} cannot be the theme one of yours extends: a theme of your own extends Purlis \
             Classic or another of your own",
            card.name
        ));
    }
    let document = serde_json::json!({
        "$schema": SCHEMA_URL,
        "name": format!("{} (mine)", card.name),
        "extends": card.id,
    });
    let text = format!(
        "{}\n",
        serde_json::to_string_pretty(&document).map_err(|why| why.to_string())?
    );
    let names = candidates(&card.id);
    if names.is_empty() {
        return Err(format!(
            "{} has no folder name a duplicate can be given, so nothing was made",
            card.id
        ));
    }
    write::new_folder(config_root, &names, &text)
}

/// The names a duplicate of `id` may have, in the order they are tried.
fn candidates(id: &str) -> Vec<String> {
    (1..=MOST_TRIES)
        .map(|n| {
            let tail = if n == 1 {
                format!("-{MINE}")
            } else {
                format!("-{MINE}-{n}")
            };
            let room = MOST_NAME.saturating_sub(tail.len());
            let base: String = id.chars().take(room).collect();
            format!("{}{tail}", base.trim_end_matches('-'))
        })
        .filter(|name| is_folder_name(name))
        .collect()
}

/// The most names a duplicate tries before it says the folder is full.
const MOST_TRIES: usize = 64;

#[cfg(unix)]
pub(in crate::theme) mod write {
    //! The one write: a new folder under the person's `themes/`, every step opened refusing a
    //! link, the file created new in it.

    use std::io::Write as _;
    use std::os::fd::OwnedFd;
    use std::path::Path;

    use rustix::fs::{Mode, OFlags, mkdirat, openat};
    use rustix::io::Errno;

    use super::THEME_JSON;
    use crate::theme::folders::THEMES;

    const DIRECTORY: OFlags = OFlags::RDONLY
        .union(OFlags::DIRECTORY)
        .union(OFlags::NOFOLLOW)
        .union(OFlags::CLOEXEC);

    /// A folder's mode: the person's alone to write, as purlis's config folder is.
    const FOLDER: Mode = Mode::RWXU
        .union(Mode::RGRP)
        .union(Mode::XGRP)
        .union(Mode::ROTH)
        .union(Mode::XOTH);

    /// A theme file's mode: the person's to write, anyone's to read.
    const FILE: Mode = Mode::RUSR
        .union(Mode::WUSR)
        .union(Mode::RGRP)
        .union(Mode::ROTH);

    /// The folder `name` in `at`, made when it is not there; refused when it is a link.
    fn step(at: &OwnedFd, name: &std::ffi::OsStr, shown: &str) -> Result<OwnedFd, String> {
        match openat(at, name, DIRECTORY, Mode::empty()) {
            Ok(fd) => return Ok(fd),
            Err(Errno::NOENT) => {}
            Err(Errno::LOOP | Errno::NOTDIR | Errno::MLINK) => {
                return Err(format!(
                    "{shown} is reached through a link or something that is not a folder, so \
                     nothing was made there"
                ));
            }
            Err(why) => return Err(format!("purlis could not open {shown}: {why}")),
        }
        match mkdirat(at, name, FOLDER) {
            Ok(()) | Err(Errno::EXIST) => {}
            Err(why) => return Err(format!("purlis could not make {shown}: {why}")),
        }
        openat(at, name, DIRECTORY, Mode::empty())
            .map_err(|why| format!("purlis could not open {shown}: {why}"))
    }

    /// **Makes the first of `names` no folder has** in the person's `themes/` under
    /// `config_root`, with `text` as its `theme.json`, and answers its name.
    pub(super) fn new_folder(
        config_root: &Path,
        names: &[String],
        text: &str,
    ) -> Result<String, String> {
        let themes = crate::machine::dir(config_root).join(THEMES);
        let shown = themes.display().to_string();
        let below = themes
            .strip_prefix(config_root)
            .map_err(|_| format!("{shown} is not inside purlis's config folder"))?;
        // The config home is the caller's to trust; every step below it is opened refusing one.
        let mut at = rustix::fs::open(
            config_root,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|why| format!("purlis could not open {}: {why}", config_root.display()))?;
        for part in below.components() {
            at = step(&at, part.as_os_str(), &shown)?;
        }
        for name in names {
            match mkdirat(&at, name.as_str(), FOLDER) {
                Ok(()) => {}
                Err(Errno::EXIST) => continue,
                Err(why) => return Err(format!("purlis could not make {shown}/{name}: {why}")),
            }
            let mut created = false;
            let written = write_in(&at, name, text, &shown, &mut created);
            if written.is_err() {
                discard(&at, name, created);
            }
            return written.map(|()| name.clone());
        }
        Err(format!(
            "{shown} has a folder of every name a duplicate tries, so nothing was made"
        ))
    }

    /// Writes `text` as the `theme.json` of the folder `name` in `at`, just made, created new;
    /// `created` says whether this call made the file, so a failure takes away only its own.
    fn write_in(
        at: &OwnedFd,
        name: &str,
        text: &str,
        shown: &str,
        created: &mut bool,
    ) -> Result<(), String> {
        let folder = openat(at, name, DIRECTORY, Mode::empty())
            .map_err(|why| format!("purlis could not open {shown}/{name}: {why}"))?;
        let file = openat(
            &folder,
            THEME_JSON,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            FILE,
        )
        .map_err(|why| format!("purlis could not write {shown}/{name}/{THEME_JSON}: {why}"))?;
        *created = true;
        std::fs::File::from(file)
            .write_all(text.as_bytes())
            .map_err(|why| format!("purlis could not write {shown}/{name}/{THEME_JSON}: {why}"))
    }

    /// **Takes away what a failed duplicate made** in `at`: its `theme.json` only when this
    /// duplicate created it (`created`), then the folder, which is removed only when empty. A
    /// file purlis did not write is never removed.
    pub(in crate::theme) fn discard(at: &OwnedFd, name: &str, created: bool) {
        if created && let Ok(folder) = openat(at, name, DIRECTORY, Mode::empty()) {
            let _ = rustix::fs::unlinkat(&folder, THEME_JSON, rustix::fs::AtFlags::empty());
        }
        let _ = rustix::fs::unlinkat(at, name, rustix::fs::AtFlags::REMOVEDIR);
    }
}

#[cfg(not(unix))]
mod write {
    //! **No theme folder is written off unix**: the steps open refusing a link, which this
    //! build cannot express, as the reads cannot ([`super::super::folders`]).

    use std::path::Path;

    pub(super) fn new_folder(
        config_root: &Path,
        _names: &[String],
        _text: &str,
    ) -> Result<String, String> {
        Err(format!(
            "no theme folder is made under {}: theme folders are made on macOS and Linux",
            config_root.display()
        ))
    }
}
