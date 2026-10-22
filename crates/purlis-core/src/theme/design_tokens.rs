// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Design tokens** (spec #2897, #2926): a theme folder written in the W3C Design Tokens
//! Community Group format 2025.10, and read back.
//!
//! `theme.json` (the format purlis reads, ADR 0097 and ADR 0103) stays the only theme format.
//! A design-tokens file is a way in and a way out, so design tools can exchange a theme's
//! tokens with purlis: [`pack`] writes one from a theme folder's files, and
//! [`super::import`] of a [`super::SourceKind::DesignTokens`] reads one back ([`unpack`]).
//! **Packing a folder and reading it back gives the same folder**: the same `theme.json`,
//! component parts and grey tiers included, and the same files beside it.
//!
//! # What is a token
//!
//! - **The palette** is the group `palette`, one `color` token for each colour.
//! - **Each appearance** is the group `appearance.<id>`. A colour token is a `color` token whose
//!   value is a reference to the palette (`{palette.lavender}`) where the theme names a palette
//!   colour, and the colour itself where it gives a hex. A geometry token is a `dimension` in
//!   px, a font weight a `fontWeight`, a line height a `number`. Its motion is the group
//!   `appearance.<id>.motion`: each duration a `duration` in ms, each easing a `cubicBezier`.
//! - **The `compact` layer** is the group `compact`, typed as an appearance's geometry is.
//!
//! A token name is split at its dots into groups (`surface.base` is `surface` › `base`); a name
//! that is both a token and a group's start is that group's `$root`.
//!
//! # What is not a token
//!
//! The format has no type for a flag, a word or a component part, so what a token cannot hold is
//! kept, as the theme writes it, under the vendor key [`VENDOR`] in `$extensions`, which the
//! format asks every tool to keep: on an appearance's group, its tag, `high-contrast` and the
//! values no token type holds (the flags, the cursor's shape, a value left to the platform); on
//! the document, the theme's name, what it extends, its icons, fonts, options, tint and component
//! parts, and the files beside `theme.json` (a font, a licence, an icon set), each as base64. A
//! colour spelled other than as the format writes it (`#FFF`) keeps its spelling there too, and
//! so does an appearance that cannot be a group (one with no id, or an id the format does not
//! allow as a name), in its place in the order.
//!
//! # Colours are hex
//!
//! A colour is written as the format asks, in `srgb` with its `hex`, and its alpha when it has
//! one. Reading, purlis takes the colour's hex: from `hex`, or from its `srgb` components. A
//! colour in another space with no `hex`, or whose `srgb` components and `hex` say different
//! colours, refuses the file with a sentence that names the token.
//!
//! # A file from a design tool
//!
//! A file purlis did not write is read by the same rules: its `appearance.<id>` groups are the
//! appearances (or, with none, the tokens at its top whose names purlis has are one), a
//! reference to its `palette` keeps the palette name, and any other reference is followed to
//! the colour it ends at. Every token no purlis token takes is listed with why.
//!
//! **Imports are bounded**, each bound checked before the work it limits: a file is at most
//! [`MOST_BYTES`] and holds at most [`MOST_VALUES`] JSON values, counted before it is parsed, with
//! no number and no more strings past [`MOST_SCALAR_BYTES`] than the files it may carry; a
//! group's or a token's name is at most [`MOST_NAME_BYTES`] and a token's path at most
//! [`MOST_PATH_BYTES`]; its groups nest at most [`MOST_DEPTH`] deep and it holds at most
//! [`MOST_NODES`] groups and tokens; it has at most a theme's [`MOST_APPEARANCES`]; a reference is
//! followed at most [`MOST_HOPS`] times; the `theme.json` it makes is held to a theme folder's
//! file cap, and the files it carries to a theme folder's zip's bounds (`super::archive`), each
//! file's size checked before it is decoded. Paths and values are borrowed from the parsed file
//! and a value is copied only once it is taken, a number only when it is short; a message names a
//! type the format has, never the file's own words. So reading one holds it once and what purlis
//! makes of it, however many tokens refer to one value, as `design_tokens_memory.rs` measures.

use std::collections::BTreeSet;

use base64::Engine as _;
use serde_json::{Map, Number, Value, json};

use super::archive::{
    MOST_FILE_BYTES, MOST_FILES, MOST_FOLDER_BYTES, fold, is_folder_id, is_kept, not_an_id,
};
use super::geometry::{self, Unit};
use super::import::{Files, LeftOut};
use super::vocabulary::{DURATIONS, EASINGS, LINE_HEIGHT, TOKENS, is_colour};
use super::{MOST_APPEARANCES, THEME_JSON};

/// The key purlis keeps what a token cannot hold under, in each `$extensions`: reverse domain
/// notation, as the format recommends.
pub const VENDOR: &str = "dev.purlis.theme";

/// The version of what purlis writes under [`VENDOR`]: it grows, and a purlis reads every
/// version up to its own.
pub const VERSION: u64 = 1;

/// The format's version this writes and reads.
pub const FORMAT: &str = "2025.10";

/// The address of the format's schema at [`FORMAT`], which every file [`pack`] writes holds to.
pub const SCHEMA_ID: &str = "https://www.designtokens.org/schemas/2025.10/format.json";

/// The media type the format names for its files.
pub const MEDIA_TYPE: &str = "application/design-tokens+json";

/// The file name ending a design-tokens file has, as the format names it.
pub const FILE_ENDING: &str = ".tokens.json";

/// The most a design-tokens file may be: room for a theme folder's files at their most, as
/// base64, and for the tokens.
pub const MOST_BYTES: u64 = 24 * 1024 * 1024;

const _: () = assert!(MOST_BYTES >= MOST_FOLDER_BYTES.div_ceil(3) * 4 + 1024 * 1024);

/// The deepest groups nest.
pub const MOST_DEPTH: usize = 32;

/// The most groups and tokens a file holds.
pub const MOST_NODES: usize = 50_000;

/// The longest name a group or a token may have, in bytes: a theme's own are at most 32.
pub const MOST_NAME_BYTES: usize = 64;

/// The longest path a token may have, its groups' names and its own joined by dots, in bytes.
pub const MOST_PATH_BYTES: usize = 256;

/// The most JSON values a file holds (every object, array, string, number, flag and null, in
/// `$extensions` too), counted in its text before it is parsed: a theme with every appearance a
/// theme may have, each naming every token, holds far fewer.
pub const MOST_VALUES: usize = 400_000;

/// The most references one value is followed through.
pub const MOST_HOPS: usize = 16;

/// The longest theme name a file's name gives, in characters, as the other imports take.
const MOST_NAME: usize = 64;

/// The group the palette is written in.
const PALETTE: &str = "palette";

/// The group the appearances are written in, each as a group of its id.
const APPEARANCE: &str = "appearance";

/// The group an appearance's motion is written in.
const MOTION: &str = "motion";

/// Where an appearance's group keeps the order of its tokens and motion: a `$` key, which no
/// appearance of a theme purlis draws has.
const ORDER: &str = "$order";

/// The group the `compact` layer is written in.
const COMPACT: &str = "compact";

/// **`files`, a theme folder named `folder`, as a design-tokens file**: its `theme.json` as
/// tokens, what no token holds under [`VENDOR`], and every other file as base64. A folder name
/// that is not a theme folder's id, and a `theme.json` that is not a JSON object, are refused.
pub fn pack(folder: &str, files: &Files) -> Result<Vec<u8>, String> {
    if !is_folder_id(folder) {
        return Err(not_an_id(folder));
    }
    let text = files
        .get(THEME_JSON)
        .ok_or_else(|| format!("{folder} has no {THEME_JSON}, so it is not a theme folder"))?;
    let theme = match serde_json::from_slice::<Value>(text) {
        Ok(Value::Object(theme)) => theme,
        Ok(_) => return Err(format!("{folder}/{THEME_JSON} is not a JSON object")),
        Err(why) => return Err(format!("{folder}/{THEME_JSON} is not JSON: {why}")),
    };
    let mut document = Map::new();
    if let Some(name) = theme.get("name").and_then(Value::as_str) {
        document.insert(
            "$description".to_owned(),
            Value::String(format!("{name}, a purlis theme")),
        );
    }
    let mut own = Map::new();
    own.insert("version".to_owned(), json!(VERSION));
    own.insert("folder".to_owned(), json!(folder));
    own.insert(
        "keys".to_owned(),
        Value::Array(theme.keys().map(|key| json!(key)).collect()),
    );
    let mut kept = Map::new();
    let mut orders = Map::new();
    let palette_names: BTreeSet<String> = match theme.get(PALETTE) {
        Some(Value::Object(palette)) => {
            let (group, left) = palette_tokens(palette);
            let names = palette
                .keys()
                .filter(|name| group.contains_key(name.as_str()))
                .cloned()
                .collect();
            if !group.is_empty() {
                document.insert(PALETTE.to_owned(), group_typed(group, "color"));
            }
            own.insert(PALETTE.to_owned(), Value::Object(left));
            orders.insert(PALETTE.to_owned(), key_list(palette));
            names
        }
        _ => BTreeSet::new(),
    };
    let mut appearance_groups = Map::new();
    if let Some(Value::Array(appearances)) = theme.get("appearances") {
        let mut order = Vec::new();
        for appearance in appearances {
            match appearance_group(appearance, &palette_names, &appearance_groups) {
                Some((id, group)) => {
                    order.push(json!(id));
                    appearance_groups.insert(id, Value::Object(group));
                }
                None => order.push(json!({ "kept": appearance })),
            }
        }
        own.insert("appearances".to_owned(), Value::Array(order));
    }
    if !appearance_groups.is_empty() {
        document.insert(APPEARANCE.to_owned(), Value::Object(appearance_groups));
    }
    if let Some(Value::Object(compact)) = theme.get(COMPACT) {
        let mut group = Map::new();
        let mut left = Map::new();
        for (key, value) in compact {
            match (geometry_token(key, value), path(key)) {
                (Some(token), Some(at)) => insert(&mut group, &at, token),
                _ => {
                    left.insert(key.clone(), value.clone());
                }
            }
        }
        if !group.is_empty() {
            document.insert(COMPACT.to_owned(), Value::Object(group));
        }
        own.insert(COMPACT.to_owned(), Value::Object(left));
        orders.insert(COMPACT.to_owned(), key_list(compact));
    }
    if !orders.is_empty() {
        own.insert("order".to_owned(), Value::Object(orders));
    }
    for (key, value) in &theme {
        let handled = match key.as_str() {
            PALETTE | COMPACT => value.is_object(),
            "appearances" => value.is_array(),
            _ => false,
        };
        if !handled {
            kept.insert(key.clone(), value.clone());
        }
    }
    own.insert("theme".to_owned(), Value::Object(kept));
    let carried: Map<String, Value> = files
        .iter()
        .filter(|(path, _)| path.as_str() != THEME_JSON)
        .map(|(path, bytes)| {
            (
                path.clone(),
                Value::String(base64::engine::general_purpose::STANDARD.encode(bytes)),
            )
        })
        .collect();
    if !carried.is_empty() {
        own.insert("files".to_owned(), Value::Object(carried));
    }
    document.insert("$extensions".to_owned(), vendor(Value::Object(own)));
    let mut text = serde_json::to_string_pretty(&Value::Object(document))
        .map_err(|why| format!("the design tokens could not be written: {why}"))?;
    text.push('\n');
    if text.len() as u64 > MOST_BYTES {
        return Err(format!(
            "{folder} as design tokens is larger than {} MiB, the most a design-tokens file may \
             be: export it as a zip",
            MOST_BYTES / 1024 / 1024
        ));
    }
    Ok(text.into_bytes())
}

/// `value` under [`VENDOR`], as an `$extensions` object.
fn vendor(value: Value) -> Value {
    let mut extensions = Map::new();
    extensions.insert(VENDOR.to_owned(), value);
    Value::Object(extensions)
}

/// A group of tokens that all have the type `kind`, said once on the group.
fn group_typed(mut tokens: Map<String, Value>, kind: &str) -> Value {
    for token in tokens.values_mut() {
        if let Value::Object(token) = token {
            token.remove("$type");
        }
    }
    let mut group = Map::new();
    group.insert("$type".to_owned(), json!(kind));
    group.extend(tokens);
    Value::Object(group)
}

/// The palette's colours as `color` tokens, and the entries none can be, as the theme writes
/// them.
fn palette_tokens(palette: &Map<String, Value>) -> (Map<String, Value>, Map<String, Value>) {
    let mut group = Map::new();
    let mut left = Map::new();
    for (name, value) in palette {
        match value.as_str().and_then(colour_token) {
            Some(token) if is_segment(name) => {
                group.insert(name.clone(), token);
            }
            _ => {
                left.insert(name.clone(), value.clone());
            }
        }
    }
    (group, left)
}

/// **One appearance as a group**: its id and the group, or `None` when it cannot be one (it is
/// not an object, or its id is missing, not a name the format allows, or taken), and is kept
/// as the theme writes it instead.
fn appearance_group(
    appearance: &Value,
    palette: &BTreeSet<String>,
    taken: &Map<String, Value>,
) -> Option<(String, Map<String, Value>)> {
    let fields = appearance.as_object()?;
    let id = fields.get("id")?.as_str()?;
    if !is_segment(id) || taken.contains_key(id) {
        return None;
    }
    let mut group = Map::new();
    let mut own = Map::new();
    let mut orders = Map::new();
    for (key, value) in fields {
        match (key.as_str(), value) {
            ("id", _) => {}
            ("tokens", Value::Object(tokens)) => {
                let mut left = Map::new();
                for (name, value) in tokens {
                    match (token_of(name, value, palette), path(name)) {
                        (Some(token), Some(at))
                            if at.first().map(String::as_str) != Some(MOTION) =>
                        {
                            insert(&mut group, &at, token);
                        }
                        _ => {
                            left.insert(name.clone(), value.clone());
                        }
                    }
                }
                own.insert(key.clone(), Value::Object(left));
                orders.insert(key.clone(), key_list(tokens));
            }
            ("motion", Value::Object(motion)) => {
                let mut left = Map::new();
                let mut moving = Map::new();
                for (name, value) in motion {
                    match (motion_token(name, value), path(name)) {
                        (Some(token), Some(at)) => insert(&mut moving, &at, token),
                        _ => {
                            left.insert(name.clone(), value.clone());
                        }
                    }
                }
                if !moving.is_empty() {
                    insert_group(&mut group, MOTION, moving);
                }
                own.insert(key.clone(), Value::Object(left));
                orders.insert(key.clone(), key_list(motion));
            }
            _ => {
                own.insert(key.clone(), value.clone());
            }
        }
    }
    if !orders.is_empty() {
        own.insert(ORDER.to_owned(), Value::Object(orders));
    }
    group.insert("$extensions".to_owned(), vendor(Value::Object(own)));
    Some((id.to_owned(), group))
}

/// An object's keys in its order, which the import writes them back in.
fn key_list(object: &Map<String, Value>) -> Value {
    Value::Array(object.keys().map(|key| json!(key)).collect())
}

/// Puts the group `inner` at `name` in `group`, beside a token already there as its `$root`.
fn insert_group(group: &mut Map<String, Value>, name: &str, inner: Map<String, Value>) {
    match group.remove(name) {
        Some(Value::Object(existing)) if is_token(&existing) => {
            let mut merged = Map::new();
            merged.insert("$root".to_owned(), Value::Object(existing));
            merged.extend(inner);
            group.insert(name.to_owned(), Value::Object(merged));
        }
        Some(Value::Object(mut existing)) => {
            existing.extend(inner);
            group.insert(name.to_owned(), Value::Object(existing));
        }
        _ => {
            group.insert(name.to_owned(), Value::Object(inner));
        }
    }
}

/// **The token a theme's `tokens` entry is**: a colour as a `color` (a palette name as a
/// reference to it), a geometry number as its type, the line height as a `number`; `None` for
/// what no token type holds.
fn token_of(name: &str, value: &Value, palette: &BTreeSet<String>) -> Option<Value> {
    if let Some(text) = value.as_str() {
        if palette.contains(text) {
            return Some(json!({ "$type": "color", "$value": format!("{{{PALETTE}.{text}}}") }));
        }
        return colour_token(text);
    }
    if name == LINE_HEIGHT {
        return value
            .is_number()
            .then(|| json!({ "$type": "number", "$value": value }));
    }
    geometry_token(name, value)
}

/// A geometry token's number as its type: px for a length, a `fontWeight`, a `number` for a
/// ratio. `None` for a name that is not geometry or a value that is not a number its type holds.
fn geometry_token(name: &str, value: &Value) -> Option<Value> {
    let step = geometry::step_of(name)?;
    let number = value
        .as_f64()
        .filter(|n| value.is_number() && n.is_finite())?;
    Some(match step.unit {
        Unit::Text | Unit::Px => {
            json!({ "$type": "dimension", "$value": { "value": value, "unit": "px" } })
        }
        Unit::Weight if (1.0..=1000.0).contains(&number) => {
            json!({ "$type": "fontWeight", "$value": value })
        }
        Unit::Weight => return None,
        Unit::Ratio => json!({ "$type": "number", "$value": value }),
    })
}

/// A motion entry as its type: a duration in ms, an easing as a `cubicBezier` whose x stays
/// within 0 and 1, as the format holds it.
fn motion_token(name: &str, value: &Value) -> Option<Value> {
    if DURATIONS.contains(&name) {
        let number = value.as_f64().filter(|n| value.is_number() && *n >= 0.0)?;
        return number
            .is_finite()
            .then(|| json!({ "$type": "duration", "$value": { "value": value, "unit": "ms" } }));
    }
    if EASINGS.contains(&name) {
        let four = value.as_array().filter(|it| it.len() == 4)?;
        let numbers: Vec<f64> = four.iter().filter_map(Value::as_f64).collect();
        let fits = numbers.len() == 4
            && four.iter().all(Value::is_number)
            && (0.0..=1.0).contains(&numbers[0])
            && (0.0..=1.0).contains(&numbers[2]);
        return fits.then(|| json!({ "$type": "cubicBezier", "$value": value }));
    }
    None
}

/// **A colour as a `color` token**: `srgb`, its components, its alpha when it is not opaque, its
/// `hex`; a spelling other than the one written back (`#FFF`) is kept under [`VENDOR`].
fn colour_token(text: &str) -> Option<Value> {
    let (rgb, alpha) = channels(text)?;
    let component = |byte: u8| Number::from_f64(f64::from(byte) / 255.0).map(Value::Number);
    let mut colour = Map::new();
    colour.insert("colorSpace".to_owned(), json!("srgb"));
    colour.insert(
        "components".to_owned(),
        Value::Array(
            rgb.iter()
                .map(|byte| component(*byte))
                .collect::<Option<_>>()?,
        ),
    );
    if alpha != 255 {
        colour.insert("alpha".to_owned(), component(alpha)?);
    }
    colour.insert(
        "hex".to_owned(),
        json!(format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2])),
    );
    let mut token = Map::new();
    token.insert("$type".to_owned(), json!("color"));
    token.insert("$value".to_owned(), Value::Object(colour));
    if canonical(rgb, alpha) != text {
        token.insert("$extensions".to_owned(), vendor(json!({ "hex": text })));
    }
    Some(Value::Object(token))
}

/// A hex colour's red, green and blue, and its alpha: `None` for what is not one.
fn channels(text: &str) -> Option<([u8; 3], u8)> {
    if !is_colour(text) {
        return None;
    }
    let digits: Vec<u8> = text[1..]
        .chars()
        .map(|c| c.to_digit(16).map(|d| d as u8))
        .collect::<Option<_>>()?;
    let pair = |at: usize| digits[at] * 16 + digits[at + 1];
    Some(match digits.len() {
        3 | 4 => {
            let short = |at: usize| digits[at] * 17;
            let alpha = if digits.len() == 4 { short(3) } else { 255 };
            ([short(0), short(1), short(2)], alpha)
        }
        _ => {
            let alpha = if digits.len() == 8 { pair(6) } else { 255 };
            ([pair(0), pair(2), pair(4)], alpha)
        }
    })
}

/// How a colour is written back: `#rrggbb`, or `#rrggbbaa` when it is not opaque, lower case.
fn canonical(rgb: [u8; 3], alpha: u8) -> String {
    let mut text = format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2]);
    if alpha != 255 {
        text.push_str(&format!("{alpha:02x}"));
    }
    text
}

/// Whether `name` is one name the format allows a group or a token: not empty, not starting
/// with `$`, and with no `{`, `}` or `.` in it.
fn is_segment(name: &str) -> bool {
    !name.is_empty() && !name.starts_with('$') && !name.contains(['{', '}', '.'])
}

/// A token name split at its dots, each part a name the format allows; `None` when one is not.
fn path(name: &str) -> Option<Vec<String>> {
    let parts: Vec<String> = name.split('.').map(str::to_owned).collect();
    parts.iter().all(|it| is_segment(it)).then_some(parts)
}

/// Whether a group-or-token object is a token: it has a value or a reference.
fn is_token(object: &Map<String, Value>) -> bool {
    object.contains_key("$value") || object.contains_key("$ref")
}

/// Puts `token` at `at` inside `group`: a name that is already a group takes it as its `$root`,
/// and a token in the way of a group becomes that group's `$root`.
fn insert(group: &mut Map<String, Value>, at: &[String], token: Value) {
    let Some((first, rest)) = at.split_first() else {
        return;
    };
    if rest.is_empty() {
        match group.get_mut(first) {
            Some(Value::Object(existing)) if !is_token(existing) => {
                existing.insert("$root".to_owned(), token);
            }
            _ => {
                group.insert(first.clone(), token);
            }
        }
        return;
    }
    let inner = group
        .entry(first.clone())
        .or_insert_with(|| Value::Object(Map::new()));
    if let Value::Object(existing) = inner {
        if is_token(existing) {
            let root = std::mem::take(existing);
            existing.insert("$root".to_owned(), Value::Object(root));
        }
        insert(existing, rest, token);
    }
}

// ---------------------------------------------------------------------------------------------
// Reading
// ---------------------------------------------------------------------------------------------

/// **A design-tokens file read back into a theme folder**: its name, its folder, its files and
/// every token no purlis token takes.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Read {
    pub name: String,
    pub folder: String,
    pub files: Files,
    pub left_out: Vec<LeftOut>,
}

/// One token of the file, found by its path.
#[derive(Debug, Clone)]
struct Found<'a> {
    /// Its path, the names of its groups and its own (a `$root` is its group's path), borrowed
    /// from the document.
    path: Vec<&'a str>,
    /// The token's object, in the document.
    token: &'a Map<String, Value>,
    /// The type it has: its own, or its nearest group's.
    kind: Option<&'a str>,
}

/// The file's tokens, by their dotted path, and what its groups say that purlis does not read.
struct Tokens<'a> {
    all: Vec<Found<'a>>,
    /// Where each token is in `all`, by its dotted path.
    index: std::collections::HashMap<String, usize>,
    notes: Vec<LeftOut>,
}

impl<'a> Tokens<'a> {
    fn read(document: &'a Map<String, Value>) -> Result<Self, String> {
        let mut tokens = Self {
            all: Vec::new(),
            index: std::collections::HashMap::new(),
            notes: Vec::new(),
        };
        let mut nodes = 0;
        tokens.walk(document, &mut Vec::new(), 0, None, &mut nodes)?;
        Ok(tokens)
    }

    fn walk(
        &mut self,
        group: &'a Map<String, Value>,
        at: &mut Vec<&'a str>,
        length: usize,
        kind: Option<&'a str>,
        nodes: &mut usize,
    ) -> Result<(), String> {
        if at.len() > MOST_DEPTH {
            return Err(format!(
                "the design tokens nest deeper than {MOST_DEPTH} groups at {}",
                at.join(".")
            ));
        }
        let kind = group.get("$type").and_then(Value::as_str).or(kind);
        if group.contains_key("$extends") {
            self.notes.push(LeftOut::new(
                shown(at),
                "a group's $extends is not read: write the tokens it would bring in the group",
            ));
        }
        for (name, value) in group {
            *nodes += 1;
            if *nodes > MOST_NODES {
                return Err(format!(
                    "the design tokens hold more than {MOST_NODES} groups and tokens"
                ));
            }
            if name == "$root" {
                if let Value::Object(token) = value {
                    self.found(at.clone(), token, kind);
                }
                continue;
            }
            if name.starts_with('$') {
                continue;
            }
            if name.len() > MOST_NAME_BYTES {
                return Err(format!(
                    "the design tokens name a group or token {} bytes long, more than the \
                     {MOST_NAME_BYTES} a name may be",
                    name.len()
                ));
            }
            let longer = length + usize::from(length > 0) + name.len();
            if longer > MOST_PATH_BYTES {
                return Err(format!(
                    "the design tokens name a token whose path is longer than the \
                     {MOST_PATH_BYTES} bytes a path may be"
                ));
            }
            at.push(name);
            if !is_segment(name) {
                self.notes.push(LeftOut::new(
                    shown(at),
                    "is not a name the format allows: it has a dot or a brace in it",
                ));
            } else {
                match value {
                    Value::Object(object) if is_token(object) => {
                        self.found(at.clone(), object, kind);
                    }
                    Value::Object(object) => self.walk(object, at, longer, kind, nodes)?,
                    _ => self
                        .notes
                        .push(LeftOut::new(shown(at), "is neither a token nor a group")),
                }
            }
            at.pop();
        }
        Ok(())
    }

    fn found(&mut self, path: Vec<&'a str>, token: &'a Map<String, Value>, kind: Option<&'a str>) {
        let kind = token.get("$type").and_then(Value::as_str).or(kind);
        self.index.insert(path.join("."), self.all.len());
        self.all.push(Found { path, token, kind });
    }

    fn at(&self, dotted: &str) -> Option<&Found<'a>> {
        self.index.get(dotted).map(|at| &self.all[*at])
    }
}

/// A path as a sentence names it.
fn shown(at: &[&str]) -> String {
    if at.is_empty() {
        "the document".to_owned()
    } else {
        at.join(".")
    }
}

/// What a token's value is, as `theme.json` writes it.
enum Taken {
    /// A value `theme.json` writes.
    Value(Value),
    /// Not taken, and why.
    Not(String),
}

/// **Reads a design-tokens file** named `file_name` (whose stem names the theme when the file
/// does not) into a theme folder's files. A file past the bounds, one that is not a JSON object,
/// a colour purlis cannot read as hex and a reference that leads nowhere or round in a circle
/// refuse it, with a sentence that names the token; a token no purlis token takes is listed.
pub(super) fn unpack(file_name: &str, bytes: &[u8]) -> Result<Read, String> {
    if bytes.len() as u64 > MOST_BYTES {
        return Err(format!(
            "{file_name} is larger than {} MiB, the most a design-tokens file may be",
            MOST_BYTES / 1024 / 1024
        ));
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|_| format!("{file_name} is not text"))?
        .trim_start_matches('\u{feff}');
    scanned(file_name, text)?;
    let document = match serde_json::from_str::<Value>(text) {
        Ok(Value::Object(document)) => document,
        Ok(_) => {
            return Err(format!(
                "{file_name} is not a design-tokens file: it is not a JSON object"
            ));
        }
        Err(why) => return Err(format!("{file_name} is not JSON: {why}")),
    };
    let empty = Map::new();
    let foreign = vendor_of(&document).is_none();
    let own = vendor_of(&document).unwrap_or(&empty);
    match own.get("version") {
        None => {}
        Some(version) => match version.as_u64() {
            Some(version) if version <= VERSION => {}
            Some(version) => {
                return Err(format!(
                    "{file_name} was written by a newer purlis (its {VENDOR} version is \
                     {version}; this one reads up to {VERSION}): update purlis to import it"
                ));
            }
            None => {
                return Err(format!(
                    "{file_name}'s {VENDOR} version is {}, which is not a whole number purlis \
                     reads",
                    crate::shown::short(&super::stringify(version))
                ));
            }
        },
    }
    let tokens = Tokens::read(&document)?;
    let mut left_out = tokens.notes.clone();
    let kept = own
        .get("theme")
        .and_then(Value::as_object)
        .unwrap_or(&empty);
    let orders = own.get("order");

    // The palette.
    let mut palette = own
        .get(PALETTE)
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let mut palette_names = BTreeSet::new();
    for found in tokens
        .all
        .iter()
        .filter(|one| one.path.first().copied() == Some(PALETTE))
    {
        let name = found.path[1..].join(".");
        if found.path.len() != 2 || !super::format::is_name(&name) {
            left_out.push(LeftOut::new(
                found.path.join("."),
                "is not a palette colour's name: lower-case letters, digits and dashes starting \
                 with a letter, at most 32 long",
            ));
            continue;
        }
        match colour_value(found, &tokens, &BTreeSet::new())? {
            Taken::Value(value) => {
                palette.insert(name.clone(), value);
                palette_names.insert(name);
            }
            Taken::Not(why) => left_out.push(LeftOut::new(found.path.join("."), why)),
        }
    }
    let palette = in_order(palette, orders.and_then(|it| it.get(PALETTE)));

    // The appearances: each group, its tokens found once.
    let mut groups: Vec<&str> = Vec::new();
    if let Some(Value::Object(group)) = document.get(APPEARANCE) {
        for (name, value) in group {
            if name.starts_with('$') {
                continue;
            }
            match value.as_object() {
                Some(object) if !is_token(object) => groups.push(name),
                _ => left_out.push(LeftOut::new(
                    format!("{APPEARANCE}.{name}"),
                    "is not an appearance: an appearance is a group of tokens",
                )),
            }
        }
    }
    let order: &[Value] = own
        .get("appearances")
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice);
    if groups.len() > MOST_APPEARANCES || order.len() > MOST_APPEARANCES {
        return Err(format!(
            "{file_name} has more than {MOST_APPEARANCES} appearances, the most a theme may have"
        ));
    }
    let named: std::collections::HashSet<&str> = groups.iter().copied().collect();
    let mut of_group: std::collections::HashMap<&str, Vec<&Found<'_>>> =
        std::collections::HashMap::new();
    for found in &tokens.all {
        if found.path.len() > 2 && found.path[0] == APPEARANCE {
            of_group.entry(found.path[1]).or_default().push(found);
        }
    }
    let mut appearances = Vec::new();
    let mut placed = BTreeSet::new();
    let listed = order.iter().filter_map(|one| match one.as_str() {
        Some(id) => Some(Ok(id)),
        None => one.get("kept").map(Err),
    });
    // A group the order does not name (a design tool added it) comes after the rest.
    let rest = groups.iter().map(|id| Ok(*id));
    for one in listed.chain(rest) {
        let id = match one {
            Ok(id) => id,
            Err(kept) => {
                appearances.push(kept.clone());
                continue;
            }
        };
        if !named.contains(id) || !placed.insert(id) {
            continue;
        }
        let found = of_group.get(id).map_or(&[][..], Vec::as_slice);
        appearances.push(appearance_of(
            id,
            &document,
            found,
            &tokens,
            &palette_names,
            &mut left_out,
        )?);
    }
    if appearances.is_empty() && foreign {
        if let Some(one) = top_level(&tokens, &palette_names, &mut left_out)? {
            appearances.push(one);
        }
    } else {
        for found in &tokens.all {
            let first = found.path.first().copied();
            if !matches!(first, Some(PALETTE | APPEARANCE | COMPACT)) {
                left_out.push(LeftOut::new(
                    found.path.join("."),
                    "no purlis token takes it: an appearance's tokens are in appearance.<id>",
                ));
            }
        }
    }

    // The compact layer.
    let mut compact = own.get(COMPACT).and_then(Value::as_object).cloned();
    for found in tokens
        .all
        .iter()
        .filter(|one| one.path.first().copied() == Some(COMPACT))
    {
        let name = found.path[1..].join(".");
        match number_value(found, &tokens, &name)? {
            Taken::Value(value) if geometry::step_of(&name).is_some() => {
                compact.get_or_insert_with(Map::new).insert(name, value);
            }
            Taken::Value(_) => left_out.push(LeftOut::new(
                found.path.join("."),
                "the compact layer holds geometry tokens only",
            )),
            Taken::Not(why) => left_out.push(LeftOut::new(found.path.join("."), why)),
        }
    }
    let compact = compact.map(|it| in_order(it, orders.and_then(|it| it.get(COMPACT))));

    // The theme, in the order it was written.
    let name = kept
        .get("name")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .filter(|it| !it.trim().is_empty())
        .unwrap_or_else(|| stem(file_name).trim().chars().take(MOST_NAME).collect());
    let mut keys: Vec<&str> = own
        .get("keys")
        .and_then(Value::as_array)
        .map(|keys| keys.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    if keys.is_empty() {
        keys = vec!["$schema", "name", PALETTE, "appearances", COMPACT];
    }
    let mut seen: std::collections::HashSet<&str> = keys.iter().copied().collect();
    for key in kept.keys() {
        if seen.insert(key) {
            keys.push(key);
        }
    }
    let has_own_palette = own.contains_key(PALETTE);
    // Written from the file's own values, which are not copied: only what purlis made is owned.
    let mut theme: Vec<(&str, std::borrow::Cow<'_, Value>)> = Vec::new();
    let mut palette = Some(palette);
    let mut appearances = Some(appearances);
    let mut compact = compact;
    let mut written = BTreeSet::new();
    for key in keys {
        if !written.insert(key) {
            continue;
        }
        use std::borrow::Cow::{Borrowed, Owned};
        let value = match key {
            PALETTE if has_own_palette || palette.as_ref().is_some_and(|it| !it.is_empty()) => {
                Owned(Value::Object(palette.take().unwrap_or_default()))
            }
            "appearances"
                if own.contains_key("appearances")
                    || appearances.as_ref().is_some_and(|it| !it.is_empty()) =>
            {
                Owned(Value::Array(appearances.take().unwrap_or_default()))
            }
            COMPACT if compact.is_some() => {
                Owned(Value::Object(compact.take().unwrap_or_default()))
            }
            "$schema" if foreign => Owned(json!(super::SCHEMA_URL)),
            "name" if !kept.contains_key("name") => Owned(json!(name)),
            _ => match kept.get(key) {
                Some(value) => Borrowed(value),
                None => continue,
            },
        };
        theme.push((key, value));
    }

    // The folder and its files.
    let folder = own
        .get("folder")
        .and_then(Value::as_str)
        .filter(|it| is_folder_id(it))
        .map_or_else(|| super::folder_name(&name), str::to_owned);
    if !is_folder_id(&folder) {
        return Err(format!(
            "{} would be the theme folder {folder}: {}; rename the theme and import it again",
            crate::shown::short(&name),
            not_an_id(&folder)
        ));
    }
    let mut files = carried(own)?;
    let mut text = Capped::default();
    if serde_json::to_writer_pretty(&mut text, &Entries(&theme)).is_err() || text.over {
        return Err(format!(
            "the {THEME_JSON} {file_name} makes is larger than {} MiB, the most a theme \
             folder's file may be",
            MOST_FILE_BYTES / 1024 / 1024
        ));
    }
    let mut text = text.bytes;
    text.push(b'\n');
    files.insert(THEME_JSON.to_owned(), text);
    Ok(Read {
        name,
        folder,
        files,
        left_out,
    })
}

/// An object's entries, in their order, written as a JSON object.
struct Entries<'a, 'v>(&'a [(&'v str, std::borrow::Cow<'v, Value>)]);

impl serde::Serialize for Entries<'_, '_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap as _;
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (key, value) in self.0 {
            map.serialize_entry(key, value.as_ref())?;
        }
        map.end()
    }
}

/// Bytes written up to a theme folder's file cap, and whether more were offered.
#[derive(Default)]
struct Capped {
    bytes: Vec<u8>,
    over: bool,
}

impl std::io::Write for Capped {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if (self.bytes.len() + buf.len()) as u64 >= MOST_FILE_BYTES {
            self.over = true;
            return Err(std::io::Error::other("past the cap"));
        }
        self.bytes.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// The most bytes one string or number of a file may be, but for the files it carries.
pub const MOST_SCALAR_BYTES: usize = 64 * 1024;

/// **`text` read once before it is parsed**: at most [`MOST_VALUES`] values (every object and
/// array, and every value after the first in one, counted outside strings), no number longer than
/// [`MOST_SCALAR_BYTES`], and no more strings longer than that than a theme folder carries files
/// ([`MOST_FILES`]), since only a carried file is that long. One pass, no allocation.
fn scanned(file_name: &str, text: &str) -> Result<(), String> {
    let mut count = 1usize;
    let mut long_strings = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    let mut run = 0usize;
    for byte in text.bytes() {
        if in_string {
            run += 1;
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
                if run > MOST_SCALAR_BYTES {
                    long_strings += 1;
                    if long_strings >= MOST_FILES {
                        return Err(format!(
                            "{file_name} holds more strings longer than {} KiB than the files a \
                             theme folder carries",
                            MOST_SCALAR_BYTES / 1024
                        ));
                    }
                }
                run = 0;
            }
            continue;
        }
        match byte {
            b'"' => {
                in_string = true;
                run = 0;
            }
            b'{' | b'[' | b',' | b':' => {
                count += 1;
                if count > MOST_VALUES {
                    return Err(format!(
                        "{file_name} holds more than {MOST_VALUES} values, the most a \
                         design-tokens file may"
                    ));
                }
                run = 0;
            }
            b'0'..=b'9' | b'-' | b'+' | b'.' | b'e' | b'E' => {
                run += 1;
                if run > MOST_SCALAR_BYTES {
                    return Err(format!(
                        "{file_name} holds a number longer than {} KiB",
                        MOST_SCALAR_BYTES / 1024
                    ));
                }
            }
            _ => run = 0,
        }
    }
    Ok(())
}

/// `object`'s entries in the order `order` lists their keys, then the rest in their own order.
fn in_order(object: Map<String, Value>, order: Option<&Value>) -> Map<String, Value> {
    let Some(order) = order.and_then(Value::as_array) else {
        return object;
    };
    let at: std::collections::HashMap<&str, usize> = order
        .iter()
        .filter_map(Value::as_str)
        .enumerate()
        .map(|(at, key)| (key, at))
        .collect();
    let mut entries: Vec<(usize, usize, String, Value)> = object
        .into_iter()
        .enumerate()
        .map(|(own, (key, value))| {
            (
                at.get(key.as_str()).copied().unwrap_or(usize::MAX),
                own,
                key,
                value,
            )
        })
        .collect();
    entries.sort_by_key(|(at, own, _, _)| (*at, *own));
    entries
        .into_iter()
        .map(|(_, _, key, value)| (key, value))
        .collect()
}

/// What purlis wrote under [`VENDOR`] on the document, if it wrote anything.
fn vendor_of(object: &Map<String, Value>) -> Option<&Map<String, Value>> {
    object
        .get("$extensions")?
        .as_object()?
        .get(VENDOR)?
        .as_object()
}

/// A file name without its folder and its ending (`.tokens.json`, `.tokens`, `.json`).
fn stem(file_name: &str) -> String {
    let base = file_name.rsplit(['/', '\\']).next().unwrap_or(file_name);
    let lower = base.to_ascii_lowercase();
    for ending in [FILE_ENDING, ".tokens", ".json"] {
        if lower.ends_with(ending) && base.len() > ending.len() {
            return base[..base.len() - ending.len()].to_owned();
        }
    }
    base.to_owned()
}

/// **One appearance, from its group `appearance.<id>`** and the tokens found in it: what
/// purlis kept on the group, its tokens and its motion, in the order they were written.
fn appearance_of(
    id: &str,
    document: &Map<String, Value>,
    found_here: &[&Found<'_>],
    tokens: &Tokens<'_>,
    palette: &BTreeSet<String>,
    left_out: &mut Vec<LeftOut>,
) -> Result<Value, String> {
    let own = document
        .get(APPEARANCE)
        .and_then(|it| it.get(id))
        .and_then(Value::as_object)
        .and_then(vendor_of);
    let orders = own.and_then(|it| it.get(ORDER));
    let mut appearance = Map::new();
    appearance.insert("id".to_owned(), json!(id));
    let mut values = own
        .and_then(|it| it.get("tokens"))
        .and_then(Value::as_object)
        .cloned();
    let mut motion = own
        .and_then(|it| it.get("motion"))
        .and_then(Value::as_object)
        .cloned();
    for found in found_here {
        let inner = &found.path[2..];
        if inner[0] == MOTION && inner.len() > 1 {
            let name = inner[1..].join(".");
            match motion_value(found, tokens)? {
                Taken::Value(value) => {
                    motion.get_or_insert_with(Map::new).insert(name, value);
                }
                Taken::Not(why) => left_out.push(LeftOut::new(found.path.join("."), why)),
            }
            continue;
        }
        let name = inner.join(".");
        match token_value(found, tokens, palette, &name)? {
            Taken::Value(value) => {
                values.get_or_insert_with(Map::new).insert(name, value);
            }
            Taken::Not(why) => left_out.push(LeftOut::new(found.path.join("."), why)),
        }
    }
    let mut values = values.map(|it| in_order(it, orders.and_then(|it| it.get("tokens"))));
    let mut motion = motion.map(|it| in_order(it, orders.and_then(|it| it.get("motion"))));
    match own {
        Some(own) => {
            for (key, value) in own {
                let value = match key.as_str() {
                    ORDER => continue,
                    "tokens" if value.is_object() => {
                        Value::Object(values.take().unwrap_or_default())
                    }
                    "motion" if value.is_object() => {
                        Value::Object(motion.take().unwrap_or_default())
                    }
                    _ => value.clone(),
                };
                appearance.insert(key.clone(), value);
            }
        }
        None => {
            let light = id.contains("light")
                || values
                    .as_ref()
                    .and_then(|it| {
                        it.get("surface.base")
                            .or_else(|| it.get("window.background"))
                    })
                    .and_then(Value::as_str)
                    .and_then(channels)
                    .is_some_and(|(rgb, _)| is_light(rgb));
            appearance.insert(
                "appearance".to_owned(),
                json!(if light { "light" } else { "dark" }),
            );
        }
    }
    if let Some(values) = values {
        appearance.insert("tokens".to_owned(), Value::Object(values));
    }
    if let Some(motion) = motion {
        appearance.insert("motion".to_owned(), Value::Object(motion));
    }
    Ok(Value::Object(appearance))
}

/// **A file from a design tool with no appearance groups**: the tokens at its top whose names
/// purlis has, as one appearance, dark or light by its ground. `None` when it has none.
fn top_level(
    tokens: &Tokens,
    palette: &BTreeSet<String>,
    left_out: &mut Vec<LeftOut>,
) -> Result<Option<Value>, String> {
    let mut values = Map::new();
    let mut motion = Map::new();
    for found in &tokens.all {
        if matches!(found.path.first().copied(), Some(PALETTE | COMPACT)) {
            continue;
        }
        let name = found.path.join(".");
        if DURATIONS.contains(&name.as_str()) || EASINGS.contains(&name.as_str()) {
            match motion_value(found, tokens)? {
                Taken::Value(value) => {
                    motion.insert(name, value);
                }
                Taken::Not(why) => left_out.push(LeftOut::new(name, why)),
            }
            continue;
        }
        let known = TOKENS.contains(&name.as_str())
            || super::typed::is_typed(&name)
            || geometry::step_of(&name).is_some();
        if !known {
            left_out.push(LeftOut::new(name, "no purlis token has this name"));
            continue;
        }
        match token_value(found, tokens, palette, &name)? {
            Taken::Value(value) => {
                values.insert(name, value);
            }
            Taken::Not(why) => left_out.push(LeftOut::new(name, why)),
        }
    }
    if values.is_empty() && motion.is_empty() {
        return Ok(None);
    }
    let light = values
        .get("surface.base")
        .or_else(|| values.get("window.background"))
        .and_then(Value::as_str)
        .and_then(channels)
        .is_some_and(|(rgb, _)| is_light(rgb));
    let tag = if light { "light" } else { "dark" };
    let mut appearance = Map::new();
    appearance.insert("id".to_owned(), json!(tag));
    appearance.insert("appearance".to_owned(), json!(tag));
    if !values.is_empty() {
        appearance.insert("tokens".to_owned(), Value::Object(values));
    }
    if !motion.is_empty() {
        appearance.insert("motion".to_owned(), Value::Object(motion));
    }
    Ok(Some(Value::Object(appearance)))
}

/// Whether a ground is nearer white than black, as the other imports decide an appearance.
fn is_light(rgb: [u8; 3]) -> bool {
    let [r, g, b] = rgb.map(f64::from);
    0.2126 * r + 0.7152 * g + 0.0722 * b > 127.5
}

/// **The token at the end of `found`'s references**, and the palette name the first one names
/// when it names the palette. Only a token that refers to the palette itself keeps the palette
/// name: one that refers to it through another token takes the colour. A reference that leads
/// nowhere, or round in a circle, refuses the file.
fn followed<'a, 'd>(
    found: &'a Found<'d>,
    tokens: &'a Tokens<'d>,
) -> Result<(&'a Found<'d>, Option<String>), String> {
    let mut at = found;
    let mut palette = None;
    for hop in 0..=MOST_HOPS {
        let Some(reference) = at
            .token
            .get("$value")
            .and_then(Value::as_str)
            .and_then(reference_of)
        else {
            return Ok((at, palette));
        };
        if hop == 0
            && let Some(name) = reference.strip_prefix("palette.")
            && !name.contains('.')
        {
            palette = Some(name.to_owned());
        }
        at = tokens.at(reference).ok_or_else(|| {
            format!(
                "{} refers to {{{reference}}}, which is no token of the file",
                found.path.join(".")
            )
        })?;
    }
    Err(format!(
        "{} refers through more than {MOST_HOPS} tokens, or round in a circle",
        found.path.join(".")
    ))
}

/// The dotted path a value's curly-brace reference names (`{palette.ink}` is `palette.ink`).
/// A reference longer than a path may be is no reference.
fn reference_of(text: &str) -> Option<&str> {
    if text.len() > MOST_PATH_BYTES + 2 + "$root".len() + 1 {
        return None;
    }
    let inner = text.strip_prefix('{')?.strip_suffix('}')?;
    let inner = inner.strip_suffix(".$root").unwrap_or(inner);
    (!inner.is_empty() && inner.split('.').all(is_segment)).then_some(inner)
}

/// The type of the token at the end of `found`'s references.
fn kind_of<'d>(found: &Found<'d>, end: &Found<'d>) -> Option<&'d str> {
    found.kind.or(end.kind)
}

/// What a token whose value is a JSON pointer is not.
const POINTER: &str = "a JSON pointer ($ref) is not read: only curly-brace references are followed";

/// **A token of an appearance, as `theme.json` writes it**: by its type.
fn token_value(
    found: &Found,
    tokens: &Tokens,
    palette: &BTreeSet<String>,
    name: &str,
) -> Result<Taken, String> {
    if found.token.contains_key("$ref") {
        return Ok(Taken::Not(POINTER.to_owned()));
    }
    let (end, _) = followed(found, tokens)?;
    match kind_of(found, end) {
        Some("color") => colour_value(found, tokens, palette),
        Some("dimension" | "fontWeight" | "number") => number_value(found, tokens, name),
        Some(other) => Ok(Taken::Not(format!(
            "{} is not one purlis takes here",
            a_type(other)
        ))),
        None => Ok(Taken::Not("has no $type".to_owned())),
    }
}

/// The format's token types, as a sentence may name one.
const TYPES: &[&str] = &[
    "color",
    "dimension",
    "fontFamily",
    "fontWeight",
    "duration",
    "cubicBezier",
    "number",
    "strokeStyle",
    "border",
    "transition",
    "shadow",
    "gradient",
    "typography",
];

/// A token of type `kind`, as a sentence names it: the format's own type by name, any other as an
/// unknown type, so nothing of the file's own text reaches a message.
fn a_type(kind: &str) -> String {
    match TYPES.iter().find(|it| **it == kind) {
        Some(known) => format!("a {known} token"),
        None => "a token of a type the format does not have".to_owned(),
    }
}

/// The most characters a number's text may have to be taken: a theme's are a few.
const MOST_NUMBER: usize = 32;

/// `value` when it is a number whose text is short, as a theme's numbers are.
fn short_number(value: &Value) -> Option<&Value> {
    match value {
        Value::Number(number) if number.as_str().len() <= MOST_NUMBER => Some(value),
        _ => None,
    }
}

/// **A colour token's value**: the palette name a reference to the palette names, or its hex.
fn colour_value(
    found: &Found,
    tokens: &Tokens,
    palette: &BTreeSet<String>,
) -> Result<Taken, String> {
    if found.token.contains_key("$ref") {
        return Ok(Taken::Not(POINTER.to_owned()));
    }
    let (end, named) = followed(found, tokens)?;
    if let Some(name) = named
        && palette.contains(&name)
    {
        return Ok(Taken::Value(json!(name)));
    }
    if kind_of(found, end) != Some("color") {
        return Ok(Taken::Not("is not a color token".to_owned()));
    }
    let at = found.path.join(".");
    let hex = hex_of(end.token.get("$value"), &at)?;
    let spelled = end
        .token
        .get("$extensions")
        .and_then(Value::as_object)
        .and_then(|it| it.get(VENDOR))
        .and_then(|it| it.get("hex"))
        .and_then(Value::as_str)
        .filter(|spelled| {
            channels(spelled).map(|(rgb, alpha)| canonical(rgb, alpha)) == Some(hex.clone())
        });
    Ok(Taken::Value(json!(spelled.map_or(hex, str::to_owned))))
}

/// **A colour value's hex**, `#rrggbb` or `#rrggbbaa`: from `srgb` components, from `hex` in any
/// space or when a component is `none`, and the two agreeing when both are given. What is not one is said as a refusal.
fn hex_of(value: Option<&Value>, at: &str) -> Result<String, String> {
    let colour = value
        .and_then(Value::as_object)
        .ok_or_else(|| format!("{at} is a color token whose value is not a colour object"))?;
    let byte = |value: &Value| -> Option<u8> {
        let number = short_number(value)?.as_f64()?;
        (0.0..=1.0)
            .contains(&number)
            .then(|| (number * 255.0).round() as u8)
    };
    let alpha = match colour.get("alpha") {
        None => 255,
        Some(alpha) => byte(alpha)
            .ok_or_else(|| format!("{at} has an alpha that is not a number from 0 to 1"))?,
    };
    let from_hex = match colour.get("hex") {
        None => None,
        Some(hex) => {
            let text = hex.as_str().filter(|it| it.len() == 7).and_then(channels);
            Some(
                text.map(|(rgb, _)| rgb)
                    .ok_or_else(|| format!("{at} has a hex that is not #rrggbb"))?,
            )
        }
    };
    let space = colour.get("colorSpace").and_then(Value::as_str);
    let from_components = match (space, colour.get("components")) {
        // A component may be "none": then the hex says the colour.
        (Some("srgb"), Some(Value::Array(three)))
            if three.len() == 3 && !three.iter().any(|it| it == "none") =>
        {
            let rgb: Option<Vec<u8>> = three.iter().map(byte).collect();
            Some(rgb.ok_or_else(|| {
                format!("{at} has srgb components that are not three numbers from 0 to 1")
            })?)
        }
        _ => None,
    };
    let rgb = match (from_components, from_hex) {
        (Some(components), Some(hex)) if components != hex => {
            return Err(format!(
                "{at} says two colours: its srgb components are {} and its hex is {}; make them \
                 one colour and import it again",
                canonical([components[0], components[1], components[2]], 255),
                canonical(hex, 255)
            ));
        }
        (Some(components), _) => [components[0], components[1], components[2]],
        (None, Some(hex)) => hex,
        (None, None) => {
            return Err(format!(
                "{at} is a colour in {} with no hex: purlis reads colours as hex, so give it its \
                 hex or srgb components",
                space.map_or_else(|| "no colour space".to_owned(), crate::shown::short)
            ));
        }
    };
    Ok(canonical(rgb, alpha))
}

/// **A number token's value**: a dimension in px, a font weight, a number, each a short number.
/// A dimension in rem and a font weight given as a word are listed, not taken. The value is read
/// where it is in the file and copied only once it is taken.
fn number_value(found: &Found, tokens: &Tokens, name: &str) -> Result<Taken, String> {
    if found.token.contains_key("$ref") {
        return Ok(Taken::Not(POINTER.to_owned()));
    }
    let (end, _) = followed(found, tokens)?;
    let value = end.token.get("$value").unwrap_or(&Value::Null);
    let at = found.path.join(".");
    let short = |value: &Value| short_number(value).map(|it| Taken::Value(it.clone()));
    let not_short = || {
        Taken::Not(format!(
            "is not a number of at most {MOST_NUMBER} characters"
        ))
    };
    Ok(match kind_of(found, end) {
        Some("dimension") => match (
            value.get("value"),
            value.get("unit").and_then(Value::as_str),
        ) {
            (Some(number), Some("px")) => short(number).unwrap_or_else(not_short),
            (_, Some("rem")) => {
                Taken::Not("is a dimension in rem: purlis reads its geometry in px".to_owned())
            }
            (_, Some(_)) => Taken::Not(
                "is a dimension in a unit the format does not have: purlis reads px".to_owned(),
            ),
            _ => return Err(format!("{at} is a dimension with no value and unit")),
        },
        Some("fontWeight") if value.is_number() => short(value).unwrap_or_else(not_short),
        Some("fontWeight") => Taken::Not(
            "is a font weight given as a word: purlis reads a font weight as a number".to_owned(),
        ),
        Some("number") => short(value).unwrap_or_else(not_short),
        Some(other) => Taken::Not(format!("{} is not one {name} takes", a_type(other))),
        None => Taken::Not("has no $type".to_owned()),
    })
}

/// **A motion token's value**: a duration in ms (or whole ms in s), an easing as four numbers,
/// each number short. The value is read where it is in the file and copied only once it is taken.
fn motion_value(found: &Found, tokens: &Tokens) -> Result<Taken, String> {
    if found.token.contains_key("$ref") {
        return Ok(Taken::Not(POINTER.to_owned()));
    }
    let (end, _) = followed(found, tokens)?;
    let value = end.token.get("$value").unwrap_or(&Value::Null);
    Ok(match kind_of(found, end) {
        Some("duration") => match (
            value.get("value").and_then(short_number),
            value.get("unit").and_then(Value::as_str),
        ) {
            (Some(number), Some("ms")) => Taken::Value(number.clone()),
            (Some(number), Some("s")) => match number.as_f64().map(|s| s * 1000.0) {
                Some(ms) if ms.fract() == 0.0 && (0.0..=f64::from(u32::MAX)).contains(&ms) => {
                    Taken::Value(json!(ms as u64))
                }
                _ => Taken::Not("is a duration that is not a whole number of ms".to_owned()),
            },
            _ => Taken::Not("is a duration with no short value in ms or s".to_owned()),
        },
        Some("cubicBezier") => match value.as_array() {
            Some(four) if four.len() == 4 && four.iter().all(|it| short_number(it).is_some()) => {
                Taken::Value(value.clone())
            }
            _ => Taken::Not("is a cubicBezier that is not four short numbers".to_owned()),
        },
        Some(other) => Taken::Not(format!("{} is not motion", a_type(other))),
        None => Taken::Not("has no $type".to_owned()),
    })
}

/// **The files purlis carried beside `theme.json`**, decoded and held to a theme folder's zip's
/// bounds: each where a theme folder keeps a file, no two a filesystem writes as one, at most
/// [`MOST_FILES`] with `theme.json`, each at most [`MOST_FILE_BYTES`], all at most
/// [`MOST_FOLDER_BYTES`].
fn carried(own: &Map<String, Value>) -> Result<Files, String> {
    let mut files = Files::new();
    let Some(given) = own.get("files") else {
        return Ok(files);
    };
    let given = given
        .as_object()
        .ok_or_else(|| format!("{VENDOR}.files is not an object of files"))?;
    if given.len() >= MOST_FILES {
        return Err(format!(
            "the design tokens carry {} files, more than a theme folder holds beside its {THEME_JSON} ({})",
            given.len(),
            MOST_FILES - 1
        ));
    }
    let mut total = 0u64;
    let mut folded = BTreeSet::from([fold(THEME_JSON)]);
    for (path, text) in given {
        let last = path.rsplit('/').next().unwrap_or(path);
        if path == THEME_JSON || !is_kept(path) || super::portable::refusal(path, last).is_some() {
            return Err(format!(
                "the design tokens carry {}, which is not where a theme folder keeps a file",
                crate::shown::short(path)
            ));
        }
        if !folded.insert(fold(path)) {
            return Err(format!(
                "the design tokens carry {} and another file a filesystem writes as the same \
                 file, in other case or other accents",
                crate::shown::short(path)
            ));
        }
        let encoded = text.as_str().unwrap_or_default();
        if (encoded.len() / 4 * 3) as u64 > MOST_FILE_BYTES + 2 {
            return Err(format!(
                "the design tokens' {} is larger than {} MiB, the most a theme folder's file may be",
                crate::shown::short(path),
                MOST_FILE_BYTES / 1024 / 1024
            ));
        }
        let bytes = text
            .as_str()
            .and_then(|it| base64::engine::general_purpose::STANDARD.decode(it).ok())
            .ok_or_else(|| {
                format!(
                    "the design tokens' {} is not base64",
                    crate::shown::short(path)
                )
            })?;
        if bytes.len() as u64 > MOST_FILE_BYTES {
            return Err(format!(
                "the design tokens' {} is larger than {} MiB, the most a theme folder's file may be",
                crate::shown::short(path),
                MOST_FILE_BYTES / 1024 / 1024
            ));
        }
        total += bytes.len() as u64;
        if total > MOST_FOLDER_BYTES {
            return Err(format!(
                "the design tokens carry more than {} MiB of files, the most a theme folder holds",
                MOST_FOLDER_BYTES / 1024 / 1024
            ));
        }
        files.insert(path.clone(), bytes);
    }
    Ok(files)
}

/// Whether a JSON document reads as design tokens: it has purlis's own mark, or a token (an
/// object with `$value`) in its first few groups.
pub(super) fn looks_like(document: &Value) -> bool {
    fn has_token(value: &Value, depth: usize) -> bool {
        let Some(object) = value.as_object() else {
            return false;
        };
        if object.contains_key("$value") {
            return true;
        }
        depth < 6 && object.values().any(|it| has_token(it, depth + 1))
    }
    document
        .as_object()
        .is_some_and(|it| vendor_of(it).is_some())
        || has_token(document, 0)
}
