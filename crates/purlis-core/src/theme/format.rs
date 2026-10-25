// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Format 2** (spec #2313, #2322): a theme folder's `theme.json`.
//!
//! ```json
//! {
//!   "$schema": "…/v<version>/docs/schema/theme.v2.schema.json",
//!   "name": "Dusk",
//!   "extends": "purlis-classic",
//!   "palette": { "lavender": "#b4a7f5" },
//!   "appearances": [
//!     { "id": "dark", "appearance": "dark", "tokens": { "accent.base": "lavender" } },
//!     { "id": "contrast", "appearance": "dark", "high-contrast": true, "tokens": { … } }
//!   ]
//! }
//! ```
//!
//! - **`extends`** names another theme: Purlis Classic (`purlis-classic`, or an old id), or a
//!   theme folder the [`Library`] holds, by the folder's name. The child starts from its parent's
//!   appearances; an appearance with a parent's id layers over that one, and a new id is added
//!   after them. A parent that is not there, a parent purlis cannot draw, and themes that extend
//!   one another in a circle each refuse the theme with a sentence that names them.
//! - **The palette** is named colours, hex only. A token's value is hex or a palette name: a
//!   closed grammar, never CSS. A name is local to the theme that declares it, so a child cannot
//!   name its parent's (ADR 0041's amendment of 2026-10-16).
//! - **An appearance** has an id, a tag (`dark` or `light`) and optionally `high-contrast`, and
//!   its token and motion values. One without a parent's appearance to layer over starts from
//!   Purlis Classic's of its tag, so a theme holds only what it changes.
//!
//! **A format 2 theme that breaks its grammar anywhere is refused whole** (ADR 0097 §2): it draws
//! Purlis, and every complaint says what is wrong. A token or motion name purlis does not have is
//! said and left out instead: a name reaches nothing, and a teammate's purlis may know a token
//! this one does not yet.
//!
//! - **A token that is not a colour** (#2327, [`super::typed`]) is given under `tokens` as a
//!   colour is: a flag as `true` or `false`, the terminal cursor's shape by its name, the
//!   terminal's line height as a number in its bound. A value of the wrong type refuses the theme.
//! - **`icons`** names the theme's icon set (#2337): a file inside the theme's folder, read under
//!   the same containment and cap as `theme.json` ([`super::icons`]). A view the set does not
//!   draw is its parent's.
//! - **Geometry** (#2326, [`super::geometry`]): an appearance's `tokens` may give any geometry
//!   token a number inside its documented bound; the theme's `compact` layer tunes what compact
//!   draws, for every appearance. A value outside its bound refuses the theme and names the
//!   bound.
//! - **Fonts** (#2326, [`super::fonts`]): the UI, mono and terminal fonts, from woff2 files in
//!   the folder's `fonts/`, for every appearance.
//! - **`options`** are the theme's toggles and choices (#2325, [`super::options`]), each value a
//!   layer of tokens drawn over every appearance. A child has its parent's, and one of its own
//!   with a parent's id takes that one's place. A theme past [`super::MOST_COMBINATIONS`]
//!   appearance × option combinations is refused with its count.
//! - **`tint`** says how a workspace's colour tints the theme (#2329): on or off, which tokens,
//!   and its own hues ([`super::tint`]). What it does not say is its parent's.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use super::fonts::{Fonts, ThemeDir};
use super::geometry::{self, Geometry};
use super::icons::{self, Icons};
use super::opened;
use super::options::{self, MOST_COMBINATIONS};
use super::tint;
use super::typed::Typed;
use super::vocabulary::{PLATFORM, TOKENS, follow_kin, follow_muted, is_colour, is_platform};
use super::{
    Appearance, CLASSIC, Complaint, PURLIS, Resolved, ResolvedTheme, Theme, built_in_for, classic,
    is_format_2, load_format_1, load_motion, purlis, stringify,
};

/// The file a theme folder is read from.
pub const THEME_JSON: &str = "theme.json";

/// The most appearances a theme may have: the spec's cap on what the check covers (every
/// appearance × option combination, 64 at most).
pub const MOST_APPEARANCES: usize = 64;

/// The most colours a palette may name.
const MOST_COLOURS: usize = 256;

/// The keys of a format 2 `theme.json` this purlis reads.
const KEYS: &[&str] = &[
    "$schema",
    "name",
    "extends",
    "palette",
    "appearances",
    "icons",
    "compact",
    "fonts",
    "options",
    "tint",
    "parts",
];

/// The longest path to an icon set purlis follows, in bytes.
pub(super) const MOST_ICONS_PATH: usize = 256;

/// The keys of an appearance.
const APPEARANCE_KEYS: &[&str] = &["id", "appearance", "high-contrast", "tokens", "motion"];

/// The grammar of a palette name and an appearance id, as a sentence.
const NAME_RULE: &str =
    "is not lower-case letters, digits and dashes starting with a letter, at most 32 long";

/// Whether `name` is a palette name or an appearance id: `[a-z][a-z0-9-]{0,31}`.
pub(super) fn is_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    name.len() <= 32
        && bytes.next().is_some_and(|b| b.is_ascii_lowercase())
        && bytes.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// **The themes a theme may extend**: Purlis Classic, and the theme folders handed to it, by
/// their ids (a folder's name). Purlis Classic's ids are its own: no folder takes them.
#[derive(Debug, Clone, Default)]
pub struct Library {
    /// Each folder's `theme.json` and the files beside it, or why it could not be read.
    folders: BTreeMap<String, Result<Held, Complaint>>,
    /// Whether [`Self::with_folder`] and [`Self::load_folder`] read the fonts of the folders they
    /// name ([`Self::reading_named_fonts`]).
    named_fonts: bool,
    /// Whether it reads only to check: a theme it accepts registers no face to serve.
    checking: bool,
}

/// A theme the library holds: its `theme.json`, and what was read beside it.
#[derive(Debug, Clone)]
struct Held {
    document: Value,
    beside: Beside,
}

/// What a theme has beside its `theme.json`.
#[derive(Debug, Clone)]
enum Beside {
    /// Nothing: the theme is one document, not a folder.
    Nothing,
    /// A folder, and the icon set its `theme.json` names, parsed, or why not; `None` when it
    /// names none. `folder` is the theme's folder, held: its fonts are read from its `fonts/`.
    Folder {
        icons: Option<Result<Value, String>>,
        folder: ThemeDir,
    },
}

/// A theme folder as read: its `theme.json`, the icon set it names, parsed, or why not, and the
/// folder itself, held by the handle `theme.json` was read through: its fonts are read from its
/// `fonts/`.
pub(crate) type FolderRead = (Value, Option<Result<Value, String>>, ThemeDir);

/// A theme read: the theme, and the names it gave that purlis left out.
type Read = (Theme, Vec<Complaint>);

/// Why a theme could not be read.
struct Refused {
    complaints: Vec<Complaint>,
    why: Why,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Why {
    /// Its grammar: the complaints say where.
    Broken,
    /// A parent that is not there, which the theme that names it says.
    Missing,
    /// A circle of `extends`: said once, where it closes, and handed up unchanged.
    Circle,
}

impl Refused {
    fn because(complaints: Vec<Complaint>) -> Self {
        Self {
            complaints,
            why: Why::Broken,
        }
    }
}

impl Library {
    /// Purlis Classic, and no folder.
    pub fn new() -> Self {
        Self::default()
    }

    /// **The library, reading only to check** (#2587): a theme it accepts is drawn and checked
    /// as any other, but registers none of its faces for [`super::served`], so a theme folder
    /// that is about to move or be removed (a staged import) leaves nothing served behind.
    #[must_use]
    pub fn checking(mut self) -> Self {
        self.checking = true;
        self
    }

    /// The library with `document` as the theme `id`.
    #[must_use]
    pub fn with(mut self, id: impl Into<String>, document: Value) -> Self {
        self.folders.insert(
            id.into(),
            Ok(Held {
                document,
                beside: Beside::Nothing,
            }),
        );
        self
    }

    /// **The library reading the fonts of the folders [`Self::with_folder`] and
    /// [`Self::load_folder`] name**, which it does not otherwise: for a folder the person named
    /// themselves, as `purlis theme check <folder>` does. A theme folder of a level is read
    /// through [`super::Folders`], whose project and workspace fonts are read only as approved.
    #[must_use]
    pub fn reading_named_fonts(mut self) -> Self {
        self.named_fonts = true;
        self
    }

    /// The library with the theme folder `name` inside `themes`, a level's `themes/` folder. A
    /// folder whose `theme.json` cannot be read is held with why, and a theme that extends it is
    /// told that. Its fonts are read only by a library [`Self::reading_named_fonts`].
    #[must_use]
    pub fn with_folder(mut self, themes: &Path, name: &str) -> Self {
        let read = read_folder(themes, name, self.named_fonts).map_err(Complaint::said);
        self.folders.insert(name.to_owned(), read);
        self
    }

    /// The library with the theme folder `name` as it was read: its `theme.json` and the icon
    /// set it names (`None` when it names none), or why it could not be read
    /// (`super::folders`, which reads a level's folders from handles).
    #[must_use]
    pub(crate) fn with_read(mut self, name: &str, read: Result<FolderRead, Complaint>) -> Self {
        self.folders.insert(
            name.to_owned(),
            read.map(|(document, icons, folder)| Held {
                document,
                beside: Beside::Folder { icons, folder },
            }),
        );
        self
    }

    /// **Reads one theme document**, `id` the theme's own id when it has one (a folder's name),
    /// and always answers with a complete theme.
    pub fn load(&self, id: Option<&str>, document: &Value) -> Resolved {
        self.load_beside(id, document, &Beside::Nothing)
    }

    /// [`Self::load`] of a `theme.json` read from the folder `folder` that `dir` holds, the
    /// theme's own folder: the icon set and the fonts it names are read from that handle. The
    /// person's own theme is read so, from the config folder its `theme.json` is in, which holds
    /// more than the theme: so its icon set is a plain file beside `theme.json` or one in its
    /// own `icons/`, never a deeper path. `placed` is where the folder is, as its level and its
    /// path there: what every sentence names it by.
    pub(crate) fn load_held_in(
        &self,
        document: &Value,
        folder: &Path,
        placed: &super::Placed,
        dir: &opened::Held,
    ) -> Resolved {
        let shown = placed.to_string();
        let icons = match document.get("icons") {
            Some(Value::String(named)) if beside_or_in_icons(named) => {
                Some(opened::icons(dir, &shown, named))
            }
            Some(Value::String(named)) => Some(Err(format!(
                "icons is {}, which is not a file beside theme.json or in its icons/ folder",
                stringify(&Value::String(named.clone()))
            ))),
            _ => None,
        };
        let beside = Beside::Folder {
            icons,
            folder: ThemeDir::opened(folder.to_path_buf(), &shown, opened::again(dir, &shown))
                .placed(placed.clone()),
        };
        self.load_beside(None, document, &beside)
    }

    /// **Reads a theme built into purlis** (#2336): its `theme.json`, the icon set it names
    /// as compiled in beside it (`icons`, the set's text, when it names one), and its folder,
    /// whose fonts are compiled in too ([`ThemeDir::built_in`]). Read as any theme folder is.
    pub(crate) fn load_built_in(
        &self,
        id: &str,
        document: &Value,
        icons: Option<&str>,
        folder: ThemeDir,
    ) -> Resolved {
        let icons = icons.map(|text| {
            serde_json::from_str::<Value>(text).map_err(|why| format!("the icon set: {why}"))
        });
        self.load_beside(Some(id), document, &Beside::Folder { icons, folder })
    }

    /// [`Self::load`] of a theme with what was read beside it: each complaint said of the
    /// folder it was read from, when nothing nearer was named.
    fn load_beside(&self, id: Option<&str>, document: &Value, beside: &Beside) -> Resolved {
        let Some(from) = document.as_object() else {
            return Resolved::refusal(placed_at(
                vec![Complaint::said("a theme is a JSON object")],
                beside,
            ));
        };
        if !is_format_2(from) {
            let mut loaded = load_format_1(from);
            loaded.complaints = placed_at(loaded.complaints, beside);
            return loaded;
        }
        let mut path: Vec<String> = id.into_iter().map(str::to_owned).collect();
        match self.format_2(from, beside, &mut path) {
            Ok((theme, notes)) => {
                // A face is served only once the theme that brings it is accepted, and never
                // for a theme read only to check.
                if !self.checking {
                    super::fonts::register(&theme.first().fonts);
                }
                Resolved {
                    theme,
                    complaints: notes,
                    refused: false,
                }
            }
            Err(refused) => Resolved::refusal(refused.complaints),
        }
    }

    /// [`Self::load`] of the theme the library holds as `id`.
    pub fn load_id(&self, id: &str) -> Resolved {
        match self.folders.get(id) {
            Some(Ok(held)) => self.load_beside(Some(id), &held.document, &held.beside),
            Some(Err(why)) => Resolved::refusal(vec![why.clone()]),
            None => Resolved::refusal(vec![Complaint::said(format!(
                "\"{id}\" is not a theme purlis has"
            ))]),
        }
    }

    /// **Reads the theme folder `name` inside `themes`**, a level's `themes/` folder: its
    /// `theme.json`, which may sit alone or beside `fonts/`, `icons/`, a preview and a licence.
    /// Its id is `name`. A folder that is a link, a name that is not one plain folder name, and
    /// a `theme.json` that is a link, not a plain file, or larger than the cap are refused; so
    /// is an icon set that is any of those, or outside the folder. Its fonts are read only by a
    /// library [`Self::reading_named_fonts`].
    pub fn load_folder(&self, themes: &Path, name: &str) -> Resolved {
        match read_folder(themes, name, self.named_fonts) {
            Ok(held) => self.load_beside(Some(name), &held.document, &held.beside),
            Err(why) => Resolved::refusal(vec![Complaint::said(why)]),
        }
    }

    /// A format 2 theme, or why not, each complaint said of the folder it was read from when
    /// nothing nearer was named. `path` is the ids of the themes being read, the one that
    /// extends first, so a circle is seen when it closes.
    fn format_2(
        &self,
        from: &Map<String, Value>,
        beside: &Beside,
        path: &mut Vec<String>,
    ) -> Result<Read, Refused> {
        match self.format_2_unplaced(from, beside, path) {
            Ok((theme, notes)) => Ok((theme, placed_at(notes, beside))),
            Err(refused) => Err(Refused {
                complaints: placed_at(refused.complaints, beside),
                ..refused
            }),
        }
    }

    /// [`Self::format_2`], each complaint as it was said.
    fn format_2_unplaced(
        &self,
        from: &Map<String, Value>,
        beside: &Beside,
        path: &mut Vec<String>,
    ) -> Result<Read, Refused> {
        let mut complaints = Vec::new();
        let mut notes = Vec::new();
        for key in from.keys() {
            if !KEYS.contains(&key.as_str()) {
                complaints.push(Complaint::said(format!("{key} is not a key of theme.json")));
            }
        }
        if from.get("$schema").is_some_and(|it| !it.is_string()) {
            complaints.push(Complaint::said("$schema is the address of a schema"));
        }
        let name = match from.get("name").and_then(Value::as_str).map(str::trim) {
            Some(name) if !name.is_empty() => name.to_owned(),
            _ => {
                complaints.push(Complaint::said("a theme has a name"));
                String::new()
            }
        };
        let called = if name.is_empty() { "this theme" } else { &name };
        let palette = palette(from.get("palette"), &mut complaints);
        let folder = match beside {
            Beside::Folder { folder, .. } => Some(folder),
            Beside::Nothing => None,
        };
        let own_options = options::read(from.get("options"), &palette, &mut complaints, &mut notes);
        let mut parents_options = Vec::new();

        // The parent's appearances, complete: what this theme's layer over.
        let mut appearances: Vec<ResolvedTheme> = match from.get("extends") {
            None => Vec::new(),
            Some(Value::String(parent)) => match self.parent(parent, path) {
                Ok((theme, parents)) => {
                    notes.extend(from_theme(parent, parents));
                    parents_options.clone_from(&theme.options);
                    theme.bases().to_vec()
                }
                Err(refused) if refused.why == Why::Circle => return Err(refused),
                Err(refused) if refused.why == Why::Missing => {
                    complaints.push(Complaint::said(format!(
                        "{called} extends \"{parent}\", which is not a theme purlis has"
                    )));
                    Vec::new()
                }
                Err(refused) => {
                    complaints.push(Complaint::said(format!(
                        "{called} extends \"{parent}\", which purlis cannot draw"
                    )));
                    complaints.extend(from_theme(parent, refused.complaints));
                    Vec::new()
                }
            },
            Some(_) => {
                complaints.push(Complaint::said("extends is the id of another theme"));
                Vec::new()
            }
        };

        let given: &[Value] = match from.get("appearances") {
            None => &[],
            Some(Value::Array(given)) => given,
            Some(_) => {
                complaints.push(Complaint::said("appearances is a list of appearances"));
                &[]
            }
        };
        // The cap is held before anything is read, so a list of any length costs one count.
        if given.len() > MOST_APPEARANCES {
            complaints.push(Complaint::said(format!(
                "{called} has {} appearances, and a theme has at most {MOST_APPEARANCES}",
                given.len()
            )));
            return Err(Refused::because(complaints));
        }
        // What the theme's `compact` layer and fonts are over: its parent's, else core's. Both are
        // the whole theme's, so a parent's first appearance holds them.
        let (parent_tuned, parent_fonts) = appearances.first().map_or_else(
            || (Geometry::new(), Fonts::classic()),
            |first| (first.tuned.clone(), first.fonts.clone()),
        );
        let mut tuned = parent_tuned;
        tuned.extend(compact_layer(from.get("compact"), &mut complaints));
        let fonts = super::fonts::read(from.get("fonts"), &parent_fonts, folder, &mut complaints);
        let inherited = appearances.len();
        // Every appearance of a theme carries its one set, so the parent's first has it.
        let parents_icons = appearances
            .first()
            .map(|one| one.icons.clone())
            .unwrap_or_default();
        let own_icons = icon_set(from.get("icons"), beside, &mut complaints, &mut notes);
        // Every appearance of a theme carries its one recipe too.
        let parents_tint = appearances
            .first()
            .map(|one| one.tint.clone())
            .unwrap_or_default();
        let tint = match from.get("tint") {
            None => parents_tint,
            Some(raw) => tint::read(raw, &parents_tint, &palette, &mut complaints, &mut notes),
        };
        // Every appearance of a theme carries its one set of component parts too (#2903).
        let parents_parts = appearances
            .first()
            .map(|one| one.parts.clone())
            .unwrap_or_default();
        let parts = super::parts::over(
            &parents_parts,
            super::parts::read(from.get("parts"), &mut complaints, &mut notes),
        );
        let mut seen: BTreeSet<String> = BTreeSet::new();
        for (at, raw) in given.iter().enumerate() {
            let Some(one) = raw.as_object() else {
                complaints.push(Complaint::said(format!(
                    "appearance {} is not an object",
                    at + 1
                )));
                continue;
            };
            let Some(id) = appearance_id(one, at, &mut complaints) else {
                continue;
            };
            if !seen.insert(id.clone()) {
                complaints.push(Complaint::said(format!("two appearances are called {id}")));
                continue;
            }
            let parents = appearances[..inherited].iter().position(|it| it.id == id);
            let layered = appearance(
                one,
                &id,
                parents.map(|at| &appearances[at]),
                &palette,
                &mut complaints,
                &mut notes,
            );
            match (layered, parents) {
                (Some(layered), Some(at)) => appearances[at] = layered,
                (Some(layered), None) => appearances.push(layered),
                (None, _) => {}
            }
        }
        if appearances.is_empty() && complaints.is_empty() {
            complaints.push(Complaint::said(format!("{called} has no appearance")));
        }
        // A parent's appearances and this theme's new ones are held to the cap together.
        if appearances.len() > MOST_APPEARANCES {
            complaints.push(Complaint::said(format!(
                "{called} has {} appearances with the theme it extends, and a theme has at most \
                 {MOST_APPEARANCES}",
                appearances.len()
            )));
        }
        let options = options::over(&parents_options, own_options);
        let ids: BTreeSet<&str> = appearances.iter().map(|one| one.id.as_str()).collect();
        options::hold_appearances(&options, &ids, &mut complaints);
        // Every combination is checked, so their number is held, with the count said.
        if complaints.is_empty()
            && options::combinations(appearances.len(), &options) > MOST_COMBINATIONS
        {
            complaints.push(Complaint::said(options::too_many(
                called,
                appearances.len(),
                &options,
            )));
        }
        if !complaints.is_empty() {
            return Err(Refused::because(complaints));
        }
        let icons = own_icons.over(&parents_icons);
        for one in &mut appearances {
            one.name.clone_from(&name);
            one.icons.clone_from(&icons);
            one.compact = geometry::compact_of(&one.geometry, &tuned);
            one.tuned.clone_from(&tuned);
            one.fonts.clone_from(&fonts);
            one.tint.clone_from(&tint);
            one.parts.clone_from(&parts);
        }
        // A part's size is held in both densities, which are known once every appearance is.
        let mut unfit: Vec<String> = Vec::new();
        for one in &appearances {
            for said in super::parts::fits(&parts, &one.geometry, &one.compact) {
                if !unfit.contains(&said) {
                    unfit.push(said);
                }
            }
        }
        if !unfit.is_empty() {
            return Err(Refused::because(
                unfit.into_iter().map(Complaint::said).collect(),
            ));
        }
        Ok((Theme::made(name, appearances, options), notes))
    }

    /// The theme `parent` names, read, or why not.
    fn parent(&self, parent: &str, path: &mut Vec<String>) -> Result<Read, Refused> {
        if path.iter().any(|id| id == parent) {
            let from = path.iter().position(|id| id == parent).unwrap_or_default();
            let mut circle: Vec<&str> = path[from..].iter().map(String::as_str).collect();
            circle.push(parent);
            return Err(Refused {
                complaints: vec![Complaint::said(format!(
                    "the themes extend one another in a circle: {}",
                    circle.join(" extends ")
                ))],
                why: Why::Circle,
            });
        }
        if is_classic(parent) {
            return Ok((classic().clone(), Vec::new()));
        }
        if parent == PURLIS {
            return Ok((purlis().clone(), Vec::new()));
        }
        let held = match self.folders.get(parent) {
            None => {
                return Err(Refused {
                    complaints: Vec::new(),
                    why: Why::Missing,
                });
            }
            Some(Err(why)) => return Err(Refused::because(vec![why.clone()])),
            Some(Ok(held)) => held,
        };
        let Some(from) = held.document.as_object() else {
            return Err(Refused::because(vec![Complaint::said(
                "a theme is a JSON object",
            )]));
        };
        if !is_format_2(from) {
            let loaded = load_format_1(from);
            let complaints = placed_at(loaded.complaints, &held.beside);
            return if loaded.refused {
                Err(Refused::because(complaints))
            } else {
                Ok((loaded.theme, complaints))
            };
        }
        path.push(parent.to_owned());
        let read = self.format_2(from, &held.beside, path);
        path.pop();
        read
    }
}

/// `complaints`, each said of the folder `beside` is when nothing nearer was named.
fn placed_at(complaints: Vec<Complaint>, beside: &Beside) -> Vec<Complaint> {
    let at = match beside {
        Beside::Folder { folder, .. } => folder.at(),
        Beside::Nothing => None,
    };
    complaints.into_iter().map(|one| one.at_least(at)).collect()
}

/// Whether `id` names Purlis Classic: its id, or an id one of its appearances is picked by.
fn is_classic(id: &str) -> bool {
    id == CLASSIC
        || [crate::names::THEME_DARK, crate::names::THEME_LIGHT]
            .iter()
            .any(|name| name.recognises(id) || name.write == id)
}

/// Why `name` is not one plain theme folder name, or `None` when it is.
fn not_a_folder_name(name: &str) -> Option<String> {
    let plain = !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains(['/', '\\', ':'])
        && Path::new(name).components().count() == 1;
    if !plain {
        return Some(format!(
            "{} is not the name of a theme folder",
            stringify(&Value::String(name.to_owned()))
        ));
    }
    super::portable::refusal(name, name)
}

/// The theme folder `name` inside `themes`: its `theme.json`, parsed, and the icon set it names,
/// or why not. `themes` is opened as the caller trusts it, `name` is one plain folder name
/// opened from it refusing a link, and every file is read from that handle ([`opened`]). A theme
/// folder may be anybody's. Its fonts are read when `fonts`, the library's
/// [`Library::reading_named_fonts`], says so.
fn read_folder(themes: &Path, name: &str, fonts: bool) -> Result<Held, String> {
    if let Some(why) = not_a_folder_name(name) {
        return Err(why);
    }
    let folder = themes.join(name);
    let shown = folder.display().to_string();
    let at = opened::root(themes)?.ok_or_else(|| format!("there is no folder {shown}"))?;
    let dir = opened::folder(&at, name, &shown)?;
    read_held(&dir, folder, fonts)
}

/// The theme folder at `folder`, held by `dir`: its `theme.json`, the icon set it names and its
/// fonts, each read from the handle. Its fonts are read when `fonts` says so.
fn read_held(dir: &opened::Held, folder: PathBuf, fonts: bool) -> Result<Held, String> {
    let shown = folder.display().to_string();
    let (document, icons) = opened::theme_json(dir, &shown)?;
    let held = if fonts {
        ThemeDir::opened(folder, &shown, opened::again(dir, &shown))
    } else {
        ThemeDir::refusing(
            folder,
            "a theme folder's fonts are read when it is one of your theme folders, or one you \
             name yourself"
                .to_owned(),
        )
    };
    Ok(Held {
        document,
        beside: Beside::Folder {
            icons,
            folder: held,
        },
    })
}

/// The most parents a named folder's `extends` chain is read through: the engine says a circle
/// or a missing parent itself, so this only bounds the walk.
const MOST_PARENTS: usize = 16;

/// **A theme folder the person named** (`purlis theme check <folder>`, #2604), opened: the
/// folder that holds it, as they typed it, and the folder itself, opened from that one refusing
/// a link. Its `theme.json`, its icon set, its fonts and every folder beside it that its
/// `extends` chain names are read from these handles when it is [`Self::load`]ed, so nothing
/// is looked up again by path after it is opened.
#[derive(Debug)]
pub struct Named {
    folder: PathBuf,
    name: String,
    opened: Result<(opened::Held, opened::Held), String>,
    /// Whether it is read only to check ([`Library::checking`]).
    checking: bool,
}

impl Named {
    /// The theme folder at `path`, opened now.
    pub fn open(path: &Path) -> Self {
        let name = path
            .file_name()
            .and_then(|it| it.to_str())
            .unwrap_or_default()
            .to_owned();
        let themes = match path.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent,
            _ => Path::new("."),
        };
        let folder = themes.join(&name);
        let shown = folder.display().to_string();
        let opened = match not_a_folder_name(&name) {
            Some(why) => Err(why),
            None => opened::root(themes)
                .and_then(|at| at.ok_or_else(|| format!("there is no folder {shown}")))
                .and_then(|at| opened::folder(&at, &name, &shown).map(|dir| (at, dir))),
        };
        Self {
            folder,
            name,
            opened,
            checking: false,
        }
    }

    /// **The folder, read only to check** ([`Library::checking`]): a theme it accepts registers
    /// none of its faces to be served, for a folder about to move or go (a staged import).
    #[must_use]
    pub fn checking(mut self) -> Self {
        self.checking = true;
        self
    }

    /// The folder's name: the theme's id.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// **The theme**, read from the handles [`Self::open`] opened, with its fonts, and the
    /// folders beside it its `extends` chain names, each read from the same handles.
    pub fn load(&self) -> Resolved {
        let (themes, dir) = match &self.opened {
            Ok(opened) => opened,
            Err(why) => return Resolved::refusal(vec![Complaint::said(why.clone())]),
        };
        let own = match read_held(dir, self.folder.clone(), true) {
            Ok(own) => own,
            Err(why) => return Resolved::refusal(vec![Complaint::said(why)]),
        };
        let mut library = Library {
            checking: self.checking,
            ..Library::default()
        };
        let mut chain: Vec<String> = Vec::new();
        let mut parent = extends(&own.document);
        while let Some(at) = parent.take() {
            if chain.len() >= MOST_PARENTS
                || at == self.name
                || chain.contains(&at)
                || not_a_folder_name(&at).is_some()
            {
                break;
            }
            let folder = self.folder.with_file_name(&at);
            let shown = folder.display().to_string();
            // A parent that is not a folder there ends the walk: the engine says it is missing.
            let Ok(beside) = opened::folder(themes, &at, &shown) else {
                break;
            };
            let read = read_held(&beside, folder, true);
            parent = read.as_ref().ok().and_then(|held| extends(&held.document));
            library
                .folders
                .insert(at.clone(), read.map_err(Complaint::said));
            chain.push(at);
        }
        library.load_beside(Some(&self.name), &own.document, &own.beside)
    }
}

/// The theme a `theme.json` extends, when it names one.
fn extends(document: &Value) -> Option<String> {
    document
        .get("extends")
        .and_then(Value::as_str)
        .map(str::to_owned)
}

/// Whether `named` is one plain file name, or `icons/` and one plain file name.
fn beside_or_in_icons(named: &str) -> bool {
    let parts: Vec<std::path::Component> = Path::new(named).components().collect();
    let plain = |part: &std::path::Component| matches!(part, std::path::Component::Normal(_));
    !named.contains(['\\', ':'])
        && match parts.as_slice() {
            [one] => plain(one),
            [first, one] => *first == std::path::Component::Normal("icons".as_ref()) && plain(one),
            _ => false,
        }
}

/// The icon set a theme names in `raw`, read from what is `beside` it, saying what is wrong with
/// it in `complaints` and the views it left out in `notes`. Empty when it names none.
fn icon_set(
    raw: Option<&Value>,
    beside: &Beside,
    complaints: &mut Vec<Complaint>,
    notes: &mut Vec<Complaint>,
) -> Icons {
    let named = match raw {
        None => return Icons::default(),
        Some(Value::String(named)) => named,
        Some(_) => {
            complaints.push(Complaint::said(
                "icons is the path of an icon set inside the theme's folder",
            ));
            return Icons::default();
        }
    };
    match beside {
        Beside::Nothing => complaints.push(Complaint::said(format!(
            "icons names {}, and an icon set is read only from a theme folder",
            stringify(&Value::String(named.clone()))
        ))),
        Beside::Folder {
            icons: Some(Ok(set)),
            ..
        } => return icons::read(named, set, complaints, notes),
        Beside::Folder {
            icons: Some(Err(why)),
            ..
        } => complaints.push(Complaint::said(why.clone())),
        // A folder whose `theme.json` names a set always has it read, or why not.
        Beside::Folder { icons: None, .. } => complaints.push(Complaint::said(format!(
            "purlis could not read the icon set {}",
            stringify(&Value::String(named.clone()))
        ))),
    }
    Icons::default()
}

/// `complaints` said about the theme `id`, each saying so: a parent's complaints reach its child
/// and keep their source.
fn from_theme(id: &str, complaints: Vec<Complaint>) -> Vec<Complaint> {
    complaints
        .into_iter()
        .map(|one| Complaint {
            said: format!("in \"{id}\": {}", one.said),
            ..one
        })
        .collect()
}

/// A theme's palette, each name to its hex, saying what is wrong with any of it.
fn palette(raw: Option<&Value>, complaints: &mut Vec<Complaint>) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let Some(raw) = raw else {
        return out;
    };
    let Some(given) = raw.as_object() else {
        complaints.push(Complaint::said("palette is an object of names to colours"));
        return out;
    };
    if given.len() > MOST_COLOURS {
        complaints.push(Complaint::said(format!(
            "palette names {} colours, and a palette has at most {MOST_COLOURS}",
            given.len()
        )));
        return out;
    }
    for (name, value) in given {
        if !is_name(name) {
            complaints.push(Complaint::said(format!(
                "palette name {} {NAME_RULE}",
                stringify(&Value::String(name.clone()))
            )));
            continue;
        }
        if name == PLATFORM {
            complaints.push(Complaint::said(format!(
                "palette name \"{PLATFORM}\" is purlis's own word for what the platform draws, so \
                 a palette cannot name a colour by it"
            )));
            continue;
        }
        match value.as_str() {
            Some(hex) if is_colour(hex) => {
                out.insert(name.clone(), hex.to_owned());
            }
            _ => complaints.push(Complaint::said(format!(
                "palette colour {name} is {}, which is not #rgb, #rgba, #rrggbb or #rrggbbaa",
                stringify(value)
            ))),
        }
    }
    out
}

/// An appearance's id, or `None` (and why) when it has no usable one.
fn appearance_id(
    one: &Map<String, Value>,
    at: usize,
    complaints: &mut Vec<Complaint>,
) -> Option<String> {
    match one.get("id") {
        None => {
            complaints.push(Complaint::said(format!("appearance {} has no id", at + 1)));
            None
        }
        Some(Value::String(id)) if is_name(id) => Some(id.clone()),
        Some(other) => {
            complaints.push(Complaint::said(format!(
                "appearance id {} {NAME_RULE}",
                stringify(other)
            )));
            None
        }
    }
}

/// One appearance, layered over `parent` (the parent theme's of the same id) or else over Purlis
/// Classic's of its tag; `None` when anything in it is wrong, which is said.
fn appearance(
    one: &Map<String, Value>,
    id: &str,
    parent: Option<&ResolvedTheme>,
    palette: &BTreeMap<String, String>,
    complaints: &mut Vec<Complaint>,
    notes: &mut Vec<Complaint>,
) -> Option<ResolvedTheme> {
    let before = complaints.len();
    for key in one.keys() {
        if !APPEARANCE_KEYS.contains(&key.as_str()) {
            complaints.push(Complaint::said(format!(
                "{key} is not a key of an appearance"
            )));
        }
    }
    let tag = match one.get("appearance") {
        Some(Value::String(tag)) if tag == "dark" => Some(Appearance::Dark),
        Some(Value::String(tag)) if tag == "light" => Some(Appearance::Light),
        None => None,
        Some(other) => {
            complaints.push(Complaint::said(format!(
                "appearance {id} is {}; an appearance is \"dark\" or \"light\"",
                stringify(other)
            )));
            return None;
        }
    };
    let appearance = match (tag, parent) {
        (Some(tag), Some(parent)) if tag != parent.appearance => {
            complaints.push(Complaint::said(format!(
                "appearance {id} is {} in the theme it extends, so it cannot be {}",
                tag_word(parent.appearance),
                tag_word(tag)
            )));
            return None;
        }
        (Some(tag), _) => tag,
        (None, Some(parent)) => parent.appearance,
        (None, None) => {
            complaints.push(Complaint::said(format!(
                "appearance {id} says neither \"dark\" nor \"light\""
            )));
            return None;
        }
    };
    let high_contrast = match one.get("high-contrast") {
        None => parent.is_some_and(|it| it.high_contrast),
        Some(Value::Bool(high)) => *high,
        Some(other) => {
            complaints.push(Complaint::said(format!(
                "high-contrast in appearance {id} is {}, which is not true or false",
                stringify(other)
            )));
            false
        }
    };
    let base = parent.unwrap_or_else(|| built_in_for(appearance));
    let mut given = tokens(
        one.get("tokens"),
        &format!("appearance {id}"),
        palette,
        complaints,
        notes,
    );
    follow_muted(&mut given.colours, &base.values);
    follow_kin(&mut given.colours, &base.values);
    let mut values = base.values.clone();
    values.extend(given.colours);
    let mut sizes = base.geometry.clone();
    sizes.extend(given.geometry);
    let mut typed = base.typed.clone();
    given.typed.onto(&mut typed);
    let mut said = Vec::new();
    let motion = load_motion(one.get("motion"), &base.motion, &mut said, notes);
    complaints.extend(said);
    if complaints.len() != before {
        return None;
    }
    Some(ResolvedTheme {
        name: String::new(),
        id: id.to_owned(),
        appearance,
        high_contrast,
        values,
        motion,
        typed,
        icons: Icons::default(),
        // The compact geometry, the theme's layer and its fonts are the whole theme's, set
        // once every appearance is read.
        compact: geometry::compact_of(&sizes, &base.tuned),
        geometry: sizes,
        tuned: base.tuned.clone(),
        fonts: base.fonts.clone(),
        options: Vec::new(),
        tint: base.tint.clone(),
        parts: base.parts.clone(),
        least: super::Least::Standard,
    })
}

/// **Token values a theme gives in one place**: an appearance's `tokens`, or a layer of a theme
/// option's value (#2325). Each kind of value is typed and bounded as the token it is for: a
/// colour is hex or a palette name, a geometry token a number inside its bound (#2326).
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct Tokens {
    pub(super) colours: BTreeMap<&'static str, String>,
    pub(super) geometry: Geometry,
    /// The tokens that are not colours (#2327): only those given.
    pub(super) typed: Typed,
}

impl Tokens {
    /// These values over `theme`, one appearance drawn: what compact draws follows the
    /// geometry, as it does for an appearance's own.
    pub(super) fn onto(&self, theme: &mut ResolvedTheme) {
        let mut colours = self.colours.clone();
        follow_muted(&mut colours, &theme.values);
        follow_kin(&mut colours, &theme.values);
        theme.values.extend(colours);
        self.typed.onto(&mut theme.typed);
        if !self.geometry.is_empty() {
            theme
                .geometry
                .extend(self.geometry.iter().map(|(token, n)| (*token, *n)));
            theme.compact = geometry::compact_of(&theme.geometry, &theme.tuned);
        }
    }
}

/// **The one reader of token values** a theme gives, `at` saying where (`appearance dark`, `the
/// on layer of option borders`): what is wrong with a value goes in `complaints` (it refuses the
/// theme), a token name purlis does not have in `notes` (it is left out).
pub(super) fn tokens(
    raw: Option<&Value>,
    at: &str,
    palette: &BTreeMap<String, String>,
    complaints: &mut Vec<Complaint>,
    notes: &mut Vec<Complaint>,
) -> Tokens {
    let mut out = Tokens::default();
    let given = match raw {
        None => return out,
        Some(Value::Object(given)) => given,
        Some(_) => {
            complaints.push(Complaint::said(format!(
                "tokens in {at} is an object of token names to values"
            )));
            return out;
        }
    };
    for (token, value) in given {
        // A flag, the terminal cursor's shape or its line height (#2327).
        if let Some(read) = super::typed::read(token, value, &format!("in {at}"), &mut out.typed) {
            if let Err(why) = read {
                complaints.push(Complaint::about(token, why));
            }
            continue;
        }
        if let Some(step) = geometry::step_of(token) {
            match geometry::number(step, value, &format!("in {at}")) {
                Ok(number) => {
                    out.geometry.insert(step.token, number);
                }
                Err(why) => complaints.push(Complaint::about(token, why)),
            }
            continue;
        }
        let Some(known) = TOKENS.iter().copied().find(|it| *it == token) else {
            notes.push(Complaint::about(
                token,
                format!("{token} in {at} is not a purlis token, so it is left out"),
            ));
            continue;
        };
        // What the platform draws may be left to it by name (#2327).
        if value
            .as_str()
            .is_some_and(|given| is_platform(known, given))
        {
            out.colours.insert(known, PLATFORM.to_owned());
            continue;
        }
        match colour(value, palette) {
            Some(hex) => {
                out.colours.insert(known, hex);
            }
            None => complaints.push(Complaint::about(
                token,
                format!(
                    "{token} in {at} is {}, which is neither a hex colour nor a name in this \
                     theme's palette",
                    stringify(value)
                ),
            )),
        }
    }
    out
}

/// A theme's `compact` layer: geometry tokens and their compact numbers, each in its bound.
fn compact_layer(raw: Option<&Value>, complaints: &mut Vec<Complaint>) -> Geometry {
    let mut out = Geometry::new();
    let Some(raw) = raw else {
        return out;
    };
    let Some(given) = raw.as_object() else {
        complaints.push(Complaint::said(
            "compact is an object of geometry tokens to numbers",
        ));
        return out;
    };
    for (token, value) in given {
        let Some(step) = geometry::step_of(token) else {
            complaints.push(Complaint::about(
                token,
                format!("{token} in compact is not a purlis geometry token"),
            ));
            continue;
        };
        match geometry::number(step, value, "in compact") {
            Ok(number) => {
                out.insert(step.token, number);
            }
            Err(why) => complaints.push(Complaint::about(token, why)),
        }
    }
    out
}

/// A token's value: hex, or the hex of a name in `palette`. Anything else is not a colour.
pub(super) fn colour(value: &Value, palette: &BTreeMap<String, String>) -> Option<String> {
    let given = value.as_str()?;
    if is_colour(given) {
        return Some(given.to_owned());
    }
    palette.get(given).cloned()
}

fn tag_word(appearance: Appearance) -> &'static str {
    match appearance {
        Appearance::Dark => "dark",
        Appearance::Light => "light",
    }
}
