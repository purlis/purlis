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
//! This module is the tool's half that needs no app: what it takes ([`asked`]), which resolved
//! theme each specimen draws ([`chosen`], the theme engine's answer, drawn by [`drawn`] with the
//! consent floors held as the window holds them), and what the model is told
//! ([`said`]), and whose turn the one painter is ([`turns`]: a chat's ask ahead of Settings ›
//! Theme's pictures). The app draws; the MCP server carries the ask over the chat's hook socket
//! ([`crate::hookwire::Ask::ThemePreview`]) and the pictures back as images on the framed
//! stream.
//!
//! **Today a theme is named by id**: Purlis Classic (`purlis-classic`) in either appearance,
//! Purlis (`purlis`, #2336), a preview theme, in its dark, light or high-contrast dark (#2709),
//! or one of Purlis Classic's appearances by its own id (`purlis-dark`, `purlis-light`, and the
//! ids they had before the rename). The ask carries a name, an appearance and whether it is the
//! high-contrast one, and nothing else, so the theme folders at every level, option values and
//! drafts are fields added beside them, each resolved by the theme engine, without changing what
//! is already said.

use serde_json::{Map, Value, json};

use crate::chattools::Tool;
use crate::theme::{self, Appearance, ResolvedTheme};

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

/// **One specimen asked for**: which theme, in which appearance, and whether in its appearance
/// tagged high contrast (#2709). No appearance is the theme's own first (Purlis Classic's is
/// dark).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Specimen {
    pub theme: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub appearance: Option<Appearance>,
    /// Asked for the appearance tagged high contrast: only ever with an appearance.
    #[serde(default, skip_serializing_if = "is_false")]
    pub high_contrast: bool,
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
    /// Whether the theme is a preview theme ([`theme::is_preview`]): Purlis, until it is the
    /// default.
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
                  page, a dialog and a Notice), in a theme and appearance, and return its \
                  picture. Give side_by_side to get a second specimen beside it to compare. \
                  purlis is Purlis, a preview theme: purlis-classic is still the default. The \
                  specimen never shows the person's window, chats, paths or terminal output.",
    schema,
    read_only: true,
}];

fn choice_properties() -> Value {
    json!({
        "theme": {
            "type": "string",
            "description": "The theme's id: purlis-classic, the default; purlis, Purlis, a \
                            preview theme; or one of Purlis Classic's appearances by id \
                            (purlis-dark, purlis-light).",
        },
        "appearance": {
            "type": "string",
            "enum": APPEARANCES,
            "description": "Which of the theme's appearances; its first when left out. \
                            dark-high-contrast is Purlis's high-contrast dark.",
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

/// The high-contrast dark, as the tool's arguments spell it: Purlis's appearance of that id.
pub const HIGH_CONTRAST_DARK: &str = "dark-high-contrast";

/// Whether `name` is this module's tool.
pub fn is_one(name: &str) -> bool {
    name == TOOL
}

/// **What a call with `args` asks for**: one specimen, or two side by side, each checked
/// against the themes this purlis has, or why it is refused in words that say what to give.
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
    };
    chosen(&first)?;
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
            if second.get("theme").is_none() && given.is_none() {
                return Err(
                    "say what the second specimen changes: `side_by_side` with a `theme`, an \
                     `appearance` or both"
                        .into(),
                );
            }
            let other = match given {
                Some(_) => Specimen {
                    theme,
                    appearance: given,
                    high_contrast,
                },
                None => Specimen {
                    theme,
                    ..specimens[0].clone()
                },
            };
            chosen(&other)?;
            specimens.push(other);
        }
        Some(_) => {
            return Err(
                "`side_by_side` is an object: the second specimen's `theme`, `appearance` or both"
                    .into(),
            );
        }
    }
    if let Some(unknown) = args
        .keys()
        .find(|key| !["theme", "appearance", "side_by_side"].contains(&key.as_str()))
    {
        return Err(format!(
            "{TOOL} takes `theme`, `appearance` and `side_by_side`, not `{unknown}`"
        ));
    }
    Ok(specimens)
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

/// The themes a specimen may be drawn in today, as a sentence's end.
fn known() -> String {
    let ids: Vec<&str> = theme::built_ins().iter().map(|t| t.name.as_str()).collect();
    format!(
        "{PURLIS_CLASSIC} (dark or light), {} (a preview: dark, light or {HIGH_CONTRAST_DARK}), \
         or {}",
        theme::PURLIS,
        ids.join(" or ")
    )
}

/// **The resolved theme `specimen` is drawn in**, by the theme engine, or why there is none.
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

/// **What `specimen` draws, as the window draws it**: the theme [`chosen`] names, through the
/// theme engine's [`theme::draw`], so the consent floors hold in the specimen as they hold in
/// the window, and [`theme::Drawn::held`] says which, when one is held.
pub fn drawn(specimen: &Specimen) -> Result<theme::Drawn, String> {
    chosen(specimen).map(theme::draw)
}

/// An appearance as the tool's arguments spell it.
pub fn word(appearance: Appearance) -> &'static str {
    match appearance {
        Appearance::Dark => "dark",
        Appearance::Light => "light",
    }
}

/// A specimen's appearance as the tool's arguments spell it, high contrast included.
fn word_of(painted: &Painted) -> &'static str {
    match (painted.appearance, painted.high_contrast) {
        (Appearance::Dark, true) => HIGH_CONTRAST_DARK,
        (appearance, _) => word(appearance),
    }
}

/// **What the model is told** of the specimens drawn, before their pictures.
pub fn said(painted: &[Painted]) -> String {
    let one = |p: &Painted| {
        format!(
            "{} ({}{}), {}×{} pixels, drawn in {} ms",
            p.theme,
            if p.preview { "a preview theme, " } else { "" },
            word_of(p),
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

#[cfg(test)]
mod tests;
