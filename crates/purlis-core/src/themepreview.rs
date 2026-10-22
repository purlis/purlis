// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The theme specimen and the `theme_preview` tool** (#2330, spec #2313).
//!
//! The **theme specimen** is a fixed page of sample content (every control, a chat pane, an
//! editor with syntax, a terminal with ANSI colour, a Settings page, a dialog and a Notice),
//! drawn by the app in a hidden window of its own and photographed. `theme_preview` is the chat
//! tool that asks for that picture, for any theme and appearance, alone or two side by side, so
//! a chat making a theme can see its work. **A chat never sees the person's window**: the
//! specimen is handed the theme and nothing else, its window is granted no command, and every
//! word and line in it is the page's own fixture.
//!
//! This module is the theme engine's half of the tool: what it takes ([`asked`], and held again
//! by the app, [`checked`]), which theme each specimen draws where the chat works ([`resolved`],
//! with the consent floors held as the window holds them), the font files handed to the page
//! ([`faces`]), what the model is told ([`said`]), whose turn the one painter is ([`turns`]: a
//! chat's ask ahead of Settings › Theme's pictures) and how often one chat is drawn
//! ([`spacing`]). The app draws; the MCP server carries the ask over the chat's hook socket
//! ([`crate::hookwire::Ask::ThemePreview`]) and the pictures back as images on the framed
//! stream.
//!
//! **A theme is named by id, never by a path** (#2536), as `purlis theme list` names it, and
//! each id is looked up where the asking chat works, by the app's own record of that chat
//! ([`Reached`]), never by anything the chat sends:
//!
//! - Purlis Classic (`purlis-classic`), Purlis (`purlis`, #2336), or one of Purlis Classic's
//!   appearances by its own id (`purlis-dark`, `purlis-light`, and the ids they had before the
//!   rename);
//! - a theme folder by its name (`dusk`), the nearest within the chat's reach, or with its level
//!   (`workspace:dusk`, `project:dusk`, `machine:dusk`; `workspace:<workspace>:dusk` names the
//!   chat's own workspace only);
//! - this chat's own theme draft (`draft:dusk`), and no other chat's;
//! - an extension's theme in force where the chat works (`<extension>/<theme>`).
//!
//! **Option values** are given by option id and value id, and drawn onto the theme before it is
//! drawn ([`theme::Theme::with`]), as the window draws a choice's. **Draft or kept** is the id:
//! `draft:dusk` beside `dusk` compares the two. Each specimen is resolved by the theme engine as
//! the window resolves a choice of the same theme, appearance and option values at the chat's
//! place ([`resolved`]), drawn through [`theme::draw`], so the consent floors hold; the window
//! tints it with the colour of the chat's workspace, as it tints the window there, and hands it
//! the theme's own fonts ([`faces`]).

use std::path::Path;

use serde_json::{Map, Value, json};

use crate::chattools::Tool;
use crate::theme::gallery::{self, Shelf};
use crate::theme::levels::Reach;
use crate::theme::{self, Appearance, Complaint, ResolvedTheme, Theme, Values, drafts};

pub mod turns;

/// The tool's name.
pub const TOOL: &str = "theme_preview";

/// Purlis Classic's id: today's look, whose two appearances are the two built-ins.
pub const PURLIS_CLASSIC: &str = theme::CLASSIC;

/// How wide the specimen page is laid out, in CSS pixels.
pub const WIDTH: u32 = 1280;

/// How tall the specimen page is laid out, in CSS pixels.
pub const HEIGHT: u32 = 800;

/// **The time budget for one specimen**: from asking for the hidden window to holding its
/// picture. A specimen not drawn within it is given up, its window closed, and the tool says so.
pub const BUDGET: std::time::Duration = std::time::Duration::from_secs(5);

/// The most specimens one call draws: one, or two side by side.
pub const AT_MOST: usize = 2;

/// **The time a chat's turn may take to look its themes up**: the theme folders read, the
/// workspace's colour, and the fonts' bytes read to be handed to the page (#2536).
pub const LOOKUP: std::time::Duration = std::time::Duration::from_secs(5);

/// **The most one chat's turn on the painter takes**: the lookup and each specimen's budget. No
/// picture is begun that could end past it.
pub const TURN: std::time::Duration =
    std::time::Duration::from_secs(LOOKUP.as_secs() + BUDGET.as_secs() * AT_MOST as u64);

/// **How long the tool waits on the app**: one budget for a gallery picture to give way to the
/// chat, or to end when the gallery was owed it ([`turns`], #2614, #2751), the chat's whole
/// [`TURN`], and a little for the reply.
pub const ASKED_WITHIN: std::time::Duration =
    std::time::Duration::from_secs(BUDGET.as_secs() + TURN.as_secs() + 5);

/// The most option values one specimen names: more than any theme's combinations allow.
pub const MOST_OPTIONS: usize = 16;

/// **The most font bytes one specimen's page is handed** (#2536): every face of the theme's three
/// fonts, each file once. A theme past it is not drawn, and the tool says so.
pub const MOST_FACE_BYTES: u64 = 16 * 1024 * 1024;

/// How an id names the asking chat's own theme draft (#2332): `draft:<folder>`.
pub const DRAFT: &str = "draft";

/// **One specimen asked for**: which theme by its id, in which appearance, whether in its
/// appearance tagged high contrast (#2709), and with which option values. No appearance is the
/// theme's own first (Purlis Classic's is dark); no option values draw each option at its
/// default.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Specimen {
    pub theme: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub appearance: Option<Appearance>,
    /// Asked for the appearance tagged high contrast: only ever with an appearance.
    #[serde(default, skip_serializing_if = "is_false")]
    pub high_contrast: bool,
    /// The value of each option, by the option's id and the value's (#2536).
    #[serde(default, skip_serializing_if = "Values::is_empty")]
    pub options: Values,
}

#[allow(clippy::trivially_copy_pass_by_ref)] // serde's `skip_serializing_if` passes a reference.
fn is_false(value: &bool) -> bool {
    !value
}

/// **One specimen drawn**: the theme and appearance it was drawn in, its picture's size in
/// pixels, and how long it took. The picture itself travels as an image after the answer
/// ([`crate::hookwire::framed::Reply`]), never inside it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Painted {
    /// The theme's id, as asked.
    pub theme: String,
    pub appearance: Appearance,
    /// Whether the appearance drawn is tagged high contrast (#2709).
    #[serde(default, skip_serializing_if = "is_false")]
    pub high_contrast: bool,
    /// Whether the theme is a preview theme ([`theme::is_preview`]): none is since Purlis became
    /// the default (#3034).
    #[serde(default, skip_serializing_if = "is_false")]
    pub preview: bool,
    pub width: u32,
    pub height: u32,
    /// From asking for the window to holding the picture.
    pub millis: u32,
    /// The picture, PNG. Taken out of the answer and attached after it on the framed stream.
    #[serde(skip)]
    pub png: Vec<u8>,
}

/// The tool, as a harness lists it.
pub static TOOLS: [Tool; 1] = [Tool {
    name: TOOL,
    description: "Draw the theme specimen, a fixed page of sample content (every control, a \
                  chat pane, an editor with syntax, a terminal with ANSI colour, a Settings \
                  page, a dialog and a Notice), in a theme, appearance and option values, as \
                  the window draws it where this chat works (its workspace's tint and the \
                  theme's own fonts), and return its picture. A theme is named by its id, as \
                  `purlis theme list` names it; draft:<folder> is this chat's own theme draft. \
                  Give side_by_side to get a second specimen beside it to compare, such as a \
                  draft beside the theme as kept. purlis is Purlis, the default theme; \
                  purlis-classic is Purlis Classic, the alternative. The specimen never shows the person's window, \
                  chats, paths or terminal output.",
    schema,
    read_only: true,
}];

fn choice_properties() -> Value {
    json!({
        "theme": {
            "type": "string",
            "description": "The theme's id: purlis, Purlis, the default; purlis-classic, \
                            Purlis Classic, the alternative; one of Purlis Classic's appearances by id \
                            (purlis-dark, purlis-light); a theme folder by its name (dusk), or \
                            with its level (workspace:dusk, project:dusk, \
                            machine:dusk); draft:<folder>, this chat's own theme draft; or an \
                            extension's theme (<extension>/<theme>).",
        },
        "appearance": {
            "type": "string",
            "enum": APPEARANCES,
            "description": "Which of the theme's appearances; its first when left out. \
                            dark-high-contrast is a theme's high-contrast dark, such as \
                            Purlis's.",
        },
        "options": {
            "type": "object",
            "description": "The value of each of the theme's options, by the option's id and \
                            the value's id (\"borders\": \"on\"); an option left out is drawn \
                            at its default.",
            "additionalProperties": { "type": "string" },
            "maxProperties": MOST_OPTIONS,
        },
    })
}

fn schema() -> Value {
    let mut properties = choice_properties();
    properties["side_by_side"] = json!({
        "type": "object",
        "description": "A second specimen, drawn beside the first: what differs from it. A \
                        field left out is the first specimen's.",
        "properties": choice_properties(),
        "additionalProperties": false,
    });
    json!({
        "type": "object",
        "properties": properties,
        "required": ["theme"],
        "additionalProperties": false,
    })
}

/// The appearances the tool's `appearance` names.
const APPEARANCES: [&str; 3] = ["dark", "light", HIGH_CONTRAST_DARK];

/// The high-contrast dark, as the tool's arguments spell it: a theme's dark appearance tagged
/// high contrast, such as Purlis's.
pub const HIGH_CONTRAST_DARK: &str = "dark-high-contrast";

/// Whether `name` is this module's tool.
pub fn is_one(name: &str) -> bool {
    name == TOOL
}

/// **What a call with `args` asks for**: one specimen, or two side by side, each a theme by an
/// id of the shape [`named`] reads, or why it is refused in words that say what to give. Which
/// theme an id names is the app's to answer, where the chat works ([`resolved`]); a built-in's
/// id is held to the built-ins here.
pub fn asked(args: &Map<String, Value>) -> Result<Vec<Specimen>, String> {
    let (appearance_asked, high_contrast) = appearance(args.get("appearance"))?;
    let first = Specimen {
        theme: match args.get("theme") {
            Some(Value::String(theme)) if !theme.trim().is_empty() => theme.trim().to_owned(),
            Some(Value::String(_)) | None => {
                return Err(format!("give `theme`, a theme's id: {}", known()));
            }
            Some(_) => return Err("`theme` is a theme's id, as text".into()),
        },
        appearance: appearance_asked,
        high_contrast,
        options: options(args.get("options"), "options")?.unwrap_or_default(),
    };
    shaped(&first)?;
    let mut specimens = vec![first];
    match args.get("side_by_side") {
        None | Some(Value::Null) => {}
        Some(Value::Object(second)) => {
            let theme = match second.get("theme") {
                None | Some(Value::Null) => specimens[0].theme.clone(),
                Some(Value::String(theme)) if !theme.trim().is_empty() => theme.trim().to_owned(),
                Some(_) => return Err("`side_by_side.theme` is a theme's id, as text".into()),
            };
            let (given, high_contrast) = appearance(second.get("appearance"))?;
            let values = options(second.get("options"), "side_by_side.options")?;
            if second.get("theme").is_none() && given.is_none() && values.is_none() {
                return Err(
                    "say what the second specimen changes: `side_by_side` with a `theme`, an \
                     `appearance`, `options` or more than one"
                        .into(),
                );
            }
            if let Some(unknown) = second
                .keys()
                .find(|key| !["theme", "appearance", "options"].contains(&key.as_str()))
            {
                return Err(format!(
                    "`side_by_side` takes `theme`, `appearance` and `options`, not `{unknown}`"
                ));
            }
            // An appearance left out is the first's, high contrast included.
            let (appearance, high_contrast) = match given {
                Some(_) => (given, high_contrast),
                None => (specimens[0].appearance, specimens[0].high_contrast),
            };
            let other = Specimen {
                theme,
                appearance,
                high_contrast,
                options: values.unwrap_or_else(|| specimens[0].options.clone()),
            };
            shaped(&other)?;
            specimens.push(other);
        }
        Some(_) => {
            return Err(
                "`side_by_side` is an object: what the second specimen changes, its `theme`, \
                 `appearance` or `options`"
                    .into(),
            );
        }
    }
    if let Some(unknown) = args
        .keys()
        .find(|key| !["theme", "appearance", "options", "side_by_side"].contains(&key.as_str()))
    {
        return Err(format!(
            "{TOOL} takes `theme`, `appearance`, `options` and `side_by_side`, not `{unknown}`"
        ));
    }
    Ok(specimens)
}

/// Whether `specimen` names a theme by an id of a shape [`named`] reads, and a built-in as one of
/// the built-ins draws it.
fn shaped(specimen: &Specimen) -> Result<(), String> {
    match named(&specimen.theme) {
        None => Err(format!(
            "`{}` is not a theme's id: name {}",
            specimen.theme,
            known()
        )),
        Some(Named::BuiltIn) => chosen(specimen).map(|_| ()),
        Some(_) => Ok(()),
    }
}

/// The option values `given` names, `None` when it names none; `field` is how the arguments
/// spell where they are.
fn options(given: Option<&Value>, field: &str) -> Result<Option<Values>, String> {
    let given = match given {
        None | Some(Value::Null) => return Ok(None),
        Some(Value::Object(given)) => given,
        Some(_) => {
            return Err(format!(
                "`{field}` is an object: each option's id and the id of its value"
            ));
        }
    };
    if given.len() > MOST_OPTIONS {
        return Err(format!(
            "`{field}` names {} options, and a specimen is drawn with at most {MOST_OPTIONS}",
            given.len()
        ));
    }
    given
        .iter()
        .map(|(option, value)| match value {
            Value::String(value) if !option.is_empty() && !value.is_empty() => {
                Ok((option.clone(), value.clone()))
            }
            _ => Err(format!(
                "`{field}` gives each option the id of one of its values, as text"
            )),
        })
        .collect::<Result<Values, String>>()
        .map(Some)
}

/// The appearance `given` names, and whether it is the high-contrast one.
fn appearance(given: Option<&Value>) -> Result<(Option<Appearance>, bool), String> {
    match given {
        None | Some(Value::Null) => Ok((None, false)),
        Some(Value::String(said)) if said == "dark" => Ok((Some(Appearance::Dark), false)),
        Some(Value::String(said)) if said == "light" => Ok((Some(Appearance::Light), false)),
        Some(Value::String(said)) if said == HIGH_CONTRAST_DARK => {
            Ok((Some(Appearance::Dark), true))
        }
        Some(_) => Err(format!(
            "`appearance` is `dark`, `light` or `{HIGH_CONTRAST_DARK}`"
        )),
    }
}

/// The ids a specimen may be drawn in, as a sentence's end.
fn known() -> String {
    let ids: Vec<&str> = theme::built_ins().iter().map(|t| t.name.as_str()).collect();
    format!(
        "{} (the default: dark, light or {HIGH_CONTRAST_DARK}), {PURLIS_CLASSIC} (dark or light), \
         or {}; a theme folder by its name (dusk), or \
         with its level (workspace:dusk, project:dusk, machine:dusk); {DRAFT}:<folder>, this \
         chat's own theme draft; or an extension's theme (<extension>/<theme>), each as `purlis \
         theme list` names it",
        theme::PURLIS,
        ids.join(" or ")
    )
}

/// **What a theme's id names** ([`named`]): its shape, before it is looked up where the chat
/// works.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Named<'a> {
    /// Purlis Classic, Purlis, or one of Purlis Classic's appearances by its id.
    BuiltIn,
    /// A theme folder: `name`, of `level` when the id names one (else the nearest within reach),
    /// and of the workspace `workspace` when it names one (`workspace:<workspace>:<folder>`).
    Folder {
        name: &'a str,
        level: Option<Reach>,
        workspace: Option<&'a str>,
    },
    /// The asking chat's own theme draft of the folder `name`.
    Draft(&'a str),
    /// An extension's theme, `<extension>/<theme>`.
    Extension,
}

/// **The shape of the id `id`**, or `None` when it is no theme's id: what a chat may name a
/// theme by, never a path.
pub fn named(id: &str) -> Option<Named<'_>> {
    use crate::extension::project::theme::Pick;
    if let Some(folder) = id.strip_prefix(&format!("{DRAFT}:")) {
        return theme::is_folder_name(folder).then_some(Named::Draft(folder));
    }
    if let Some((level, rest)) = id
        .split_once(':')
        .and_then(|(level, rest)| Some((Reach::of_word(level)?, rest)))
    {
        let (workspace, name) = match (level, rest.split_once(':')) {
            (Reach::Workspace, Some((workspace, name))) => {
                crate::contain::workspace_name_ok(workspace).then_some((Some(workspace), name))?
            }
            (_, Some(_)) => return None,
            (_, None) => (None, rest),
        };
        return theme::is_folder_name(name).then_some(Named::Folder {
            name,
            level: Some(level),
            workspace,
        });
    }
    match Pick::parse(id)? {
        Pick::BuiltIn(_) => Some(Named::BuiltIn),
        Pick::Folder(_) => Some(Named::Folder {
            name: id,
            level: None,
            workspace: None,
        }),
        Pick::Extension { .. } => Some(Named::Extension),
        Pick::System => None,
    }
}

/// **The resolved theme a built-in's `specimen` is drawn in**, by the theme engine, or why there
/// is none: Purlis Classic and Purlis in the appearance asked, and Purlis Classic's appearances
/// by their own ids.
pub fn chosen(specimen: &Specimen) -> Result<&'static ResolvedTheme, String> {
    if specimen.theme == PURLIS_CLASSIC || specimen.theme == theme::PURLIS {
        let shipped = theme::shipped(&specimen.theme).unwrap_or_else(theme::classic);
        let Some(asked) = specimen.appearance else {
            return Ok(shipped.first());
        };
        let exactly = shipped
            .appearances
            .iter()
            .find(|one| one.appearance == asked && one.high_contrast == specimen.high_contrast);
        return match exactly {
            Some(found) => Ok(found),
            None if specimen.high_contrast => Err(no_high_contrast(&specimen.theme)),
            None => Ok(shipped.of(asked).unwrap_or_else(|| shipped.first())),
        };
    }
    let Some(found) = theme::built_in(&specimen.theme) else {
        return Err(format!(
            "this purlis has no theme `{}`: name {}",
            specimen.theme,
            known()
        ));
    };
    if specimen.high_contrast {
        return Err(no_high_contrast(&specimen.theme));
    }
    match specimen.appearance {
        Some(asked) if asked != found.appearance => Err(format!(
            "`{}` has only a {} appearance: ask for {PURLIS_CLASSIC} with appearance `{}`",
            specimen.theme,
            word(found.appearance),
            word(asked)
        )),
        _ => Ok(found),
    }
}

/// Why `theme` has no high-contrast dark to draw, naming the theme that has one.
fn no_high_contrast(theme: &str) -> String {
    format!(
        "`{theme}` has no high-contrast appearance: ask for {} with appearance \
         `{HIGH_CONTRAST_DARK}`",
        theme::PURLIS
    )
}

/// **Where the asking chat works, as the app knows it** (#2536): what a theme's id is looked up
/// in. Every field is the app's own record of the chat whose token asked, never what the chat
/// sent.
#[derive(Debug, Clone, Copy)]
pub struct Reached<'a> {
    /// This machine's config home: the person's own theme folders, and the project's approval.
    pub config: Option<&'a Path>,
    /// The plane the chat works in.
    pub plane: Option<&'a Path>,
    /// The workspace it works in, `None` at the plane root.
    pub workspace: Option<&'a str>,
    /// The chat's id (its ULID): whose theme drafts are its own.
    pub chat: Option<&'a str>,
    /// The themes a choice where it works reaches: the levels' theme folders, read as
    /// [`theme::Folders::of`] reads them for `config`, `plane` and `workspace`, a reach of the
    /// chat's own level, and the extension themes in force there.
    pub shelf: Shelf<'a>,
}

impl Reached<'static> {
    /// **Nowhere**: the built-in themes, and nothing else.
    pub fn nowhere() -> Self {
        static NO_FOLDERS: std::sync::LazyLock<theme::Folders> =
            std::sync::LazyLock::new(theme::Folders::default);
        Self {
            config: None,
            plane: None,
            workspace: None,
            chat: None,
            shelf: Shelf {
                folders: &NO_FOLDERS,
                reach: Reach::Machine,
                on: theme::On::Only(&[]),
                offered: &[],
            },
        }
    }
}

/// **What `specimen` draws where the chat at `at` works, as the window draws it**: the theme its
/// id names there ([`named`]), with its option values drawn onto it ([`theme::Theme::with`]), in
/// the appearance asked, through the theme engine's [`theme::draw`], so the consent floors hold
/// in the specimen as they hold in the window and [`theme::Drawn::held`] says which, when one is
/// held. What the window draws for a choice of the same theme, appearance and option values
/// there.
///
/// Refused, with why, for an id that names nothing within the chat's reach; a theme the engine
/// refuses, among them one whose font files the project's approval does not hold; this chat's
/// draft when it breaks a hard floor, which the window does not draw; an option or value the
/// theme does not have; and an appearance it does not have.
pub fn resolved(at: &Reached<'_>, specimen: &Specimen) -> Result<theme::Drawn, String> {
    let id = specimen.theme.as_str();
    let Some(shape) = named(id) else {
        return Err(format!("`{id}` is not a theme's id: name {}", known()));
    };
    let theme = match shape {
        Named::BuiltIn => {
            let shipped = theme::shipped(id).unwrap_or_else(theme::classic);
            with_options(id, shipped, &specimen.options)?;
            return chosen(specimen).map(theme::draw);
        }
        Named::Draft(folder) => draft_of(at, id, folder)?,
        Named::Folder {
            name,
            level,
            workspace,
        } => folder_of(at, id, name, level, workspace)?,
        Named::Extension => accepted(
            id,
            gallery::theme_of(&at.shelf, id, None).ok_or_else(|| missing(id))?,
            &[],
        )?,
    };
    let theme = with_options(id, &theme, &specimen.options)?;
    // High contrast asked for is the appearance of the tag tagged so, and never the plain one
    // in its place; otherwise the tag's plain appearance, as a choice of the tag draws it.
    let picked = match specimen.appearance {
        None => Some(theme.first()),
        Some(asked) if specimen.high_contrast => theme
            .appearances
            .iter()
            .find(|one| one.appearance == asked && one.high_contrast),
        Some(asked) => theme.of(asked),
    };
    let one = picked.ok_or_else(|| {
        let has: Vec<String> = theme
            .appearances
            .iter()
            .map(|one| {
                format!(
                    "{} ({})",
                    one.id,
                    word_tagged(one.appearance, one.high_contrast)
                )
            })
            .collect();
        format!(
            "`{id}` has no {} appearance: it has {}",
            word_tagged(
                specimen.appearance.unwrap_or(Appearance::Dark),
                specimen.high_contrast
            ),
            has.join(", ")
        )
    })?;
    Ok(theme::draw(one))
}

/// Why `id` names nothing here.
fn missing(id: &str) -> String {
    format!(
        "there is no theme `{id}` within this chat's reach (its own workspace's theme folders, the \
         project's, the person's own and the extensions in force here): name {}",
        known()
    )
}

/// The level a chat at `at` works at, in a sentence's words.
fn here(at: &Reached<'_>) -> String {
    match (at.plane, at.workspace) {
        (Some(_), Some(name)) => format!("in the workspace {name}"),
        (Some(_), None) => "at the plane root".to_owned(),
        (None, _) => "in no project this purlis has open".to_owned(),
    }
}

/// **The theme folder `name`** the id `id` names, within the reach of the chat at `at`: of
/// `level` when it names one, else the nearest that has it, as a choice there reads it.
fn folder_of(
    at: &Reached<'_>,
    id: &str,
    name: &str,
    level: Option<Reach>,
    workspace: Option<&str>,
) -> Result<Theme, String> {
    let beyond = || {
        format!(
            "`{id}` is beyond this chat's reach: it works {}, and a specimen reaches the theme \
             folders of its own workspace, the project's and the person's own",
            here(at)
        )
    };
    if workspace.is_some() && workspace != at.workspace {
        return Err(beyond());
    }
    if level.is_some_and(|level| level < at.shelf.reach) {
        return Err(beyond());
    }
    let found = match level {
        Some(level) => at.shelf.folders.find_in(name, level),
        None => at.shelf.folders.find(name, at.shelf.reach),
    };
    match found {
        // What reading the levels said of every other folder is not this theme's to tell.
        theme::levels::Found::Read(resolved) => accepted(
            id,
            resolved,
            &levels_said(at.shelf.folders.find("", at.shelf.reach)),
        ),
        theme::levels::Found::Missing(_) => Err(missing(id)),
    }
}

/// **This chat's own theme draft of `folder`**, read as the window reads it where the chat works,
/// and held to the hard floors the window holds a draft to.
fn draft_of(at: &Reached<'_>, id: &str, folder: &str) -> Result<Theme, String> {
    let (Some(plane), Some(chat)) = (at.plane, at.chat) else {
        return Err(format!(
            "`{id}` is this chat's own theme draft, and this purlis keeps no theme drafts for \
             this chat: `purlis theme draft {folder}` starts one in a chat purlis started"
        ));
    };
    let draft = drafts::Draft {
        chat: chat.to_owned(),
        folder: folder.to_owned(),
        kept_to: drafts::kept_to(at.shelf.folders, at.workspace, folder),
    };
    let place = drafts::Place {
        plane,
        workspace: at.workspace,
    };
    // What reading the drafts and the levels says of every other folder.
    let noise = drafts::level(at.config, place, at.shelf.folders, chat)
        .map(|level| levels_said(at.shelf.folders.find_draft(&level, "", true)))
        .unwrap_or_default();
    let read = drafts::read(at.config, place, &draft, false).map_err(|said| {
        let own = own(&said, &noise);
        format!(
            "this chat has no theme draft {folder} to draw{}. `purlis theme draft {folder}` \
             starts one",
            if own.is_empty() {
                String::new()
            } else {
                format!(": {}", sentences(&own))
            }
        )
    })?;
    let theme = accepted(id, read, &noise)?;
    let broken = drafts::broken(&theme);
    if !broken.is_empty() {
        let said: Vec<String> = broken.iter().take(8).map(drafts::BrokenIn::said).collect();
        return Err(format!(
            "the theme draft {folder} breaks a hard floor, so the window does not draw it, and \
             its specimen is not drawn either: {}",
            said.join("; ")
        ));
    }
    Ok(theme)
}

/// What reading the levels said with nothing found: `found` for a name no folder has.
fn levels_said(found: theme::levels::Found) -> Vec<Complaint> {
    match found {
        theme::levels::Found::Missing(said) => said,
        theme::levels::Found::Read(_) => Vec::new(),
    }
}

/// `complaints` without those `noise` holds (each once): what is said of the folder named alone.
fn own(complaints: &[Complaint], noise: &[Complaint]) -> Vec<Complaint> {
    let mut left: Vec<&Complaint> = noise.iter().collect();
    complaints
        .iter()
        .filter(
            |one| match left.iter().position(|said| said.said == one.said) {
                Some(at) => {
                    left.swap_remove(at);
                    false
                }
                None => true,
            },
        )
        .cloned()
        .collect()
}

/// `resolved`'s theme, or why the engine refused it, in what was said of it alone (`noise` is
/// what reading the levels said of other folders): a theme whose font files wait on the
/// project's approval says so, since purlis draws no such font before it. The engine names every
/// path relative to its level (`theme::Placed`), so what it says is told as it is said.
fn accepted(id: &str, resolved: theme::Resolved, noise: &[Complaint]) -> Result<Theme, String> {
    if !resolved.refused {
        return Ok(resolved.theme);
    }
    let complaints = own(&resolved.complaints, noise);
    let mut waiting: Vec<&str> = complaints
        .iter()
        .filter_map(|one| one.awaits_approval.as_deref())
        .filter_map(|face| face.rsplit_once(' ').map(|(path, _)| path))
        .collect();
    waiting.sort_unstable();
    waiting.dedup();
    if !waiting.is_empty() {
        return Err(format!(
            "`{id}` brings font files the project's approval does not hold yet ({}). No font a \
             project or a workspace brings is drawn until the person allows it with the project, \
             so neither the window nor its specimen draws this theme. The person allows them in \
             the window, from the Notice that names them",
            waiting.join(", ")
        ));
    }
    Err(format!(
        "`{id}` is refused, so no specimen of it is drawn: {}",
        sentences(&complaints)
    ))
}

/// Complaints as one run of sentences, as a chat is told them.
fn sentences(complaints: &[Complaint]) -> String {
    let said: Vec<&str> = complaints
        .iter()
        .take(8)
        .map(|one| one.said.as_str())
        .collect();
    if said.is_empty() {
        "purlis cannot draw it".to_owned()
    } else {
        said.join("; ")
    }
}

/// `theme` with the option values `values` drawn onto it, or why not: an option it does not
/// have, or a value its option does not have.
fn with_options(id: &str, theme: &Theme, values: &Values) -> Result<Theme, String> {
    let options = || {
        if theme.options.is_empty() {
            return "it has none".to_owned();
        }
        let each: Vec<String> = theme
            .options
            .iter()
            .map(|one| {
                let values: Vec<&str> = one.values.iter().map(|value| value.id.as_str()).collect();
                format!("{} ({})", one.id, values.join(" or "))
            })
            .collect();
        format!("its options are {}", each.join(", "))
    };
    for (option, value) in values {
        let Some(one) = theme.options.iter().find(|one| one.id == *option) else {
            return Err(format!("`{id}` has no option `{option}`: {}", options()));
        };
        if !one.values.iter().any(|one| one.id == *value) {
            return Err(format!(
                "the option `{option}` of `{id}` has no value `{value}`: {}",
                options()
            ));
        }
    }
    Ok(theme.with(values).0)
}

/// **The font files `theme` draws, as handed to the specimen's page** (#2536): each face of its
/// UI, mono and terminal fonts, by the digest the page makes it from, with its bytes, each file
/// once. They are the bytes purlis serves the window for that face ([`theme::served`]): only of a
/// theme the engine read and accepted, as approved, and only while they still have that digest.
/// Refused past [`MOST_FACE_BYTES`], or for a face that can no longer be served.
pub fn faces(theme: &ResolvedTheme) -> Result<Vec<(String, Vec<u8>)>, String> {
    faces_of(
        [&theme.fonts.ui, &theme.fonts.mono, &theme.fonts.terminal]
            .into_iter()
            .flat_map(|font| font.faces.iter().map(|face| face.asset.as_str())),
    )
}

/// [`faces`] of the faces whose digests are `assets`, in their order.
pub fn faces_of<'a>(
    assets: impl IntoIterator<Item = &'a str>,
) -> Result<Vec<(String, Vec<u8>)>, String> {
    let mut out: Vec<(String, Vec<u8>)> = Vec::new();
    let mut total: u64 = 0;
    for asset in assets {
        if out.iter().any(|(had, _)| had == asset) {
            continue;
        }
        let bytes = theme::served(asset).map_err(|why| {
            format!("a font of the theme could not be handed to its specimen: {why}")
        })?;
        total = total.saturating_add(bytes.len() as u64);
        if total > MOST_FACE_BYTES {
            return Err(format!(
                "the theme's fonts are more than {} MiB, the most a specimen is handed, so its \
                 specimen is not drawn",
                MOST_FACE_BYTES / (1024 * 1024)
            ));
        }
        out.push((asset.to_owned(), bytes));
    }
    Ok(out)
}

/// **What a line that reached the app asks for, held to the tool's own bounds** (#2536): the
/// app is the boundary, since a chat may write the hook line itself rather than through the
/// tool. One specimen or [`AT_MOST`], each an id of a shape [`named`] reads (a built-in as one
/// of the built-ins draws it), at most [`MOST_OPTIONS`] option values each named by text, and
/// high contrast only with an appearance.
pub fn checked(specimens: &[Specimen]) -> Result<(), String> {
    if specimens.is_empty() || specimens.len() > AT_MOST {
        return Err(format!(
            "ask for one theme specimen, or {AT_MOST} side by side"
        ));
    }
    for specimen in specimens {
        if specimen.theme.trim() != specimen.theme || specimen.theme.is_empty() {
            return Err(format!("give `theme`, a theme's id: {}", known()));
        }
        shaped(specimen)?;
        if specimen.options.len() > MOST_OPTIONS {
            return Err(format!(
                "a specimen is drawn with at most {MOST_OPTIONS} option values, not {}",
                specimen.options.len()
            ));
        }
        if specimen
            .options
            .iter()
            .any(|(option, value)| option.is_empty() || value.is_empty())
        {
            return Err("each option value names an option and one of its values".to_owned());
        }
        if specimen.high_contrast && specimen.appearance.is_none() {
            return Err(format!(
                "high contrast is asked for with an appearance: `{HIGH_CONTRAST_DARK}`"
            ));
        }
    }
    Ok(())
}

/// **The colour the window is tinted with in the workspace `workspace` of the plane at
/// `plane`**: its `workspace.json`'s, read as anything a chat could have written is read (no
/// link on the way or at the leaf, a plain file asked of the open descriptor, non-blocking, at
/// most [`crate::windowprefs::MAX_BYTES`]); `None` for a workspace with none, or a file that is
/// not one to read.
pub fn workspace_colour(plane: &Path, workspace: &str) -> Option<String> {
    use std::io::Read as _;
    if !crate::contain::workspace_name_ok(workspace) {
        return None;
    }
    let path = plane
        .join("workspaces")
        .join(workspace)
        .join("workspace.json");
    let file = crate::contain::open_no_link(plane, &path).ok()?;
    let found = file.metadata().ok()?;
    let most = crate::windowprefs::MAX_BYTES;
    if !found.file_type().is_file() || found.len() > most {
        return None;
    }
    let mut text = String::new();
    file.take(most + 1).read_to_string(&mut text).ok()?;
    if text.len() as u64 > most {
        return None;
    }
    let doc: Value = serde_json::from_str(&text).ok()?;
    crate::extension::project::theme::colour_of_manifest(workspace, &doc)
        .as_ref()
        .map(crate::extension::project::theme::Colour::value)
}

/// An appearance as the tool's arguments spell it.
pub fn word(appearance: Appearance) -> &'static str {
    match appearance {
        Appearance::Dark => "dark",
        Appearance::Light => "light",
    }
}

/// An appearance as the tool's arguments spell it, high contrast included.
pub fn word_tagged(appearance: Appearance, high_contrast: bool) -> &'static str {
    match (appearance, high_contrast) {
        (Appearance::Dark, true) => HIGH_CONTRAST_DARK,
        (Appearance::Light, true) => "light high-contrast",
        (appearance, false) => word(appearance),
    }
}

/// **What the model is told** of the specimens drawn, before their pictures.
pub fn said(painted: &[Painted]) -> String {
    let one = |p: &Painted| {
        format!(
            "{} ({}{}), {}×{} pixels, drawn in {} ms",
            p.theme,
            if p.preview { "a preview theme, " } else { "" },
            word_tagged(p.appearance, p.high_contrast),
            p.width,
            p.height,
            p.millis
        )
    };
    let head = match painted {
        [only] => format!("The theme specimen in {}.", one(only)),
        [left, right] => format!(
            "Two theme specimens, in the order of their pictures: {}; then {}.",
            one(left),
            one(right)
        ),
        many => format!(
            "{} theme specimens: {}.",
            many.len(),
            many.iter().map(one).collect::<Vec<_>>().join("; ")
        ),
    };
    format!(
        "{head} It shows sample content only, never the person's window: its chat, files, \
         terminal and settings are the specimen's own."
    )
}

pub mod spacing;

#[cfg(test)]
mod tests;
