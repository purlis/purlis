// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The published JSON Schema of format 2**, generated from the engine's own vocabulary
//! (spec #2313, ADR 0097 §3.1): the token and motion names, the colour grammar, the bounds. A
//! theme's `$schema` names [`SCHEMA_URL`], so an editor completes token names and flags a
//! mistake as it is typed.
//!
//! The schema is published as a file in the repository, `docs/schema/theme.schema.json`, which
//! a test holds equal to [`schema_text`]; `PURLIS_THEME_SCHEMA_WRITE=1 cargo test -p
//! purlis-core the_published_schema` writes it again after the vocabulary changes. The engine,
//! not the schema, is what a theme is held to: the schema describes the same grammar for an
//! editor, and a test proves each name in it is the engine's.

use serde_json::{Map, Value, json};

use super::fonts::{MOST_FACES, SLOTS};
use super::format::{MOST_APPEARANCES, MOST_ICONS_PATH, THEME_JSON};
use super::geometry::{STEPS, Step, Unit};
use super::vocabulary::{DURATIONS, EASINGS, LONGEST, TOKENS};

/// Where the schema is published: the file in the repository, as GitHub serves it.
pub const SCHEMA_URL: &str =
    "https://raw.githubusercontent.com/purlis/purlis/main/docs/schema/theme.schema.json";

/// The colour grammar as a pattern: `#rgb`, `#rgba`, `#rrggbb` or `#rrggbbaa`
/// ([`super::vocabulary::is_colour`]).
const HEX: &str = "^#([0-9a-fA-F]{3}|[0-9a-fA-F]{4}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})$";

/// A palette name's and an appearance id's grammar ([`super::format`]'s `is_name`).
const NAME: &str = "^[a-z][a-z0-9-]{0,31}$";

/// A bound as a schema writes it: a whole number as an integer.
fn bound(number: f64) -> Value {
    if number.fract() == 0.0 {
        json!(number as i64)
    } else {
        json!(number)
    }
}

/// One geometry token's schema: a number in its documented range.
fn geometry(step: &Step) -> Value {
    let kind = if matches!(step.unit, Unit::Ratio) {
        "number"
    } else {
        "integer"
    };
    let unit = match step.unit {
        Unit::Text => "Pixels at the default text size.",
        Unit::Px => "Pixels.",
        Unit::Weight => "A font weight.",
        Unit::Ratio => "A share of the text's own size.",
    };
    json!({
        "description": format!("{unit} Purlis Classic's is {}.", bound(step.classic)),
        "type": kind,
        "minimum": bound(step.least),
        "maximum": bound(step.most)
    })
}

/// The pattern of a face's file: one plain name in the folder's `fonts/`, ending `.woff2`.
const FACE_FILE: &str = "^fonts/[A-Za-z0-9_-][A-Za-z0-9._-]*\\.woff2$";

/// **The schema**, as a JSON value.
pub fn schema() -> Value {
    let mut tokens: Map<String, Value> = TOKENS
        .iter()
        .map(|token| ((*token).to_owned(), json!({ "$ref": "#/$defs/colour" })))
        .collect();
    let sizes: Map<String, Value> = STEPS
        .iter()
        .map(|step| (step.token.to_owned(), geometry(step)))
        .collect();
    tokens.extend(sizes.clone());
    let fonts: Map<String, Value> = SLOTS
        .iter()
        .map(|slot| ((*slot).to_owned(), json!({ "$ref": "#/$defs/font" })))
        .collect();
    let mut motion: Map<String, Value> = DURATIONS
        .iter()
        .map(|name| {
            (
                (*name).to_owned(),
                json!({
                    "description": "Milliseconds; 0 turns it off.",
                    "type": "integer",
                    "minimum": 0,
                    "maximum": LONGEST
                }),
            )
        })
        .collect();
    for name in EASINGS {
        motion.insert(
            (*name).to_owned(),
            json!({
                "description": "A cubic Bézier, [x1, y1, x2, y2]: each x from 0 to 1, each y from -1 to 2.",
                "type": "array",
                "prefixItems": [
                    { "type": "number", "minimum": 0, "maximum": 1 },
                    { "type": "number", "minimum": -1, "maximum": 2 },
                    { "type": "number", "minimum": 0, "maximum": 1 },
                    { "type": "number", "minimum": -1, "maximum": 2 }
                ],
                "items": false,
                "minItems": 4
            }),
        );
    }
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": SCHEMA_URL,
        "title": format!("A purlis theme folder's {THEME_JSON} (format 2)"),
        "description": "A theme: a name, optionally the theme it extends, a palette of named colours, its icon set, its appearances, each tagged dark or light and optionally high contrast, what compact draws, and its fonts. purlis refuses a theme that breaks this grammar and draws Purlis in its place.",
        "type": "object",
        "additionalProperties": false,
        "required": ["name"],
        "anyOf": [{ "required": ["appearances"] }, { "required": ["extends"] }],
        "properties": {
            "$schema": { "type": "string" },
            "name": {
                "description": "What to call the theme, as a person would.",
                "type": "string",
                "minLength": 1
            },
            "extends": {
                "description": "The id of the theme this one starts from: purlis-classic, or a theme folder's name. This theme holds only what differs.",
                "type": "string",
                "minLength": 1
            },
            "palette": {
                "description": "Named colours, hex only. A token names one by its name; the names are this theme's own.",
                "type": "object",
                "propertyNames": { "$ref": "#/$defs/name" },
                "additionalProperties": { "$ref": "#/$defs/hex" },
                "maxProperties": 256
            },
            "appearances": {
                "description": "The theme's appearances, the first what it draws when nothing chose another. One with an id its parent has layers over that one.",
                "type": "array",
                "items": { "$ref": "#/$defs/appearance" },
                "maxItems": MOST_APPEARANCES
            },
            "icons": {
                "description": "The theme's icon set: the path of a JSON file inside this theme's folder, such as icons/icons.json, that draws each view's tab on the rails. A view it does not draw is the parent's.",
                "type": "string",
                "minLength": 1,
                "maxLength": MOST_ICONS_PATH
            },
            "compact": {
                "description": "What compact draws, for every appearance, when the person chose compact density: geometry tokens and their numbers. A token not named is Purlis Classic's compact where that moves it, else the appearance's own.",
                "type": "object",
                "additionalProperties": false,
                "properties": sizes
            },
            "fonts": {
                "description": "The UI, mono and terminal fonts, each from woff2 files in this folder's fonts/. A font not named is the parent's, or the platform's.",
                "type": "object",
                "additionalProperties": false,
                "properties": fonts
            }
        },
        "$defs": {
            "hex": { "type": "string", "pattern": HEX },
            "name": { "type": "string", "pattern": NAME },
            "colour": {
                "description": "A hex colour, or the name of a colour in this theme's palette.",
                "anyOf": [{ "$ref": "#/$defs/hex" }, { "$ref": "#/$defs/name" }]
            },
            "appearance": {
                "type": "object",
                "additionalProperties": false,
                "required": ["id"],
                "properties": {
                    "id": { "$ref": "#/$defs/name" },
                    "appearance": {
                        "description": "Whether it is drawn on a dark window or a light one. Required unless the theme it extends has an appearance of this id.",
                        "enum": ["dark", "light"]
                    },
                    "high-contrast": { "type": "boolean" },
                    "tokens": { "$ref": "#/$defs/tokens" },
                    "motion": { "$ref": "#/$defs/motion" }
                }
            },
            "tokens": {
                "description": "Token values: a colour for a colour token, a number in its bound for a geometry token. A token not named is the parent's, or Purlis Classic's of the same appearance.",
                "type": "object",
                "additionalProperties": false,
                "properties": tokens
            },
            "motion": {
                "type": "object",
                "additionalProperties": false,
                "properties": motion
            },
            "font": {
                "type": "object",
                "additionalProperties": false,
                "required": ["family", "faces"],
                "properties": {
                    "family": {
                        "description": "What the font is called, as Settings says it.",
                        "type": "string",
                        "pattern": "^[\\p{L}\\p{N}][\\p{L}\\p{N} ._-]{0,63}$"
                    },
                    "faces": {
                        "type": "array",
                        "items": { "$ref": "#/$defs/face" },
                        "minItems": 1,
                        "maxItems": MOST_FACES
                    }
                }
            },
            "face": {
                "type": "object",
                "additionalProperties": false,
                "required": ["file"],
                "properties": {
                    "file": {
                        "description": "A woff2 file in this folder's fonts/: never a URL or a path out of the folder.",
                        "type": "string",
                        "pattern": FACE_FILE,
                        "maxLength": 106
                    },
                    "weight": { "type": "integer", "minimum": 100, "maximum": 900 },
                    "style": { "enum": ["normal", "italic"] }
                }
            }
        }
    })
}

/// [`schema`] as the published file holds it: pretty-printed, with a final newline.
pub fn schema_text() -> String {
    let mut text = serde_json::to_string_pretty(&schema()).expect("the schema serialises");
    text.push('\n');
    text
}
