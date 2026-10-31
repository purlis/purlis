// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Import** (spec #2313, #2334): a theme someone already likes, made into a theme folder.
//!
//! [`import`] reads one source file of a [`SourceKind`] and answers with a theme folder's files
//! ([`Imported`]): a format 2 `theme.json` naming [`super::SCHEMA_URL`], and every key of the
//! source that reached no token ([`LeftOut`]). Nothing is guessed and nothing is dropped:
//!
//! - **A fixed mapping table per kind** ([`table`]) says which source key each token takes,
//!   nearest first: a token takes the first key of its row the source has. A token no row
//!   fills is Purlis Classic's of the same appearance, as in any theme that leaves it out.
//! - **Every key the source has that gave no token a value is listed**, with why: no token
//!   takes it, it is not a colour, it names another file, `theme.json` has no place for it.
//! - **Then each kind's recipes tune what the table filled** ([`recipes`], #2589): where the
//!   source makes two roles one colour, or names a hue that does not read on the ground it is
//!   drawn on, a token takes a nearer step on the source's own scale or a colour between two of
//!   its own. A terminal's own colours are never tuned.
//! - **A terminal palette, a base16 or a base24 scheme keeps its own colours as the theme's
//!   palette**, each named after its source key (`base0d`, `ansi-4`, `background`), so a
//!   colour changed once changes every token that uses it. A VS Code theme's colours are not a
//!   palette, and are written as hex.
//! - **The appearance** is the source's own word where it has one (VS Code's `type`, a tinted
//!   scheme's `variant`); otherwise it is dark when the background is nearer black than white.
//!
//! **Bounds**: a source is at most [`MOST_SOURCE_BYTES`]; it is read from the bytes it is
//! handed, never from another file it names, and never from the network. A source that is not
//! its kind's grammar is refused with a sentence that says where.
//!
//! A theme folder's own zip ([`SourceKind::Zip`], `super::archive`) is the other way in: it gives
//! back the folder [`super::archive::export`] packed, file for file. So is a design-tokens file
//! ([`SourceKind::DesignTokens`], `super::design_tokens`): the W3C format 2025.10, from purlis's
//! own export or a design tool's.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value};

use super::vocabulary::{FLAGS, TOKENS};
use super::{Appearance, ResolvedTheme};

/// The largest source file [`import`] reads: VS Code themes carry long `tokenColors` lists,
/// and a scheme or a terminal palette is far smaller.
pub const MOST_SOURCE_BYTES: u64 = 1024 * 1024;

/// The most rules of a VS Code theme's `tokenColors` read.
const MOST_RULES: usize = 4096;

/// The longest theme name an import writes, in characters.
const MOST_NAME: usize = 64;

/// What a source file is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceKind {
    /// A VS Code colour theme: JSON with comments.
    Vscode,
    /// A base16 scheme: YAML or JSON, `base00`–`base0F`.
    Base16,
    /// A base24 scheme: base16's sixteen and `base10`–`base17`.
    Base24,
    /// An iTerm2 colour preset, `.itermcolors` (a property list).
    Iterm,
    /// A Ghostty theme: `key = value` lines.
    Ghostty,
    /// A theme folder's zip, as `purlis theme export` writes one.
    Zip,
    /// A W3C design-tokens file, format 2025.10 (#2926), as `purlis theme export --kind
    /// design-tokens` writes one or a design tool does.
    #[serde(rename = "design-tokens")]
    DesignTokens,
}

impl SourceKind {
    /// Every kind, in the order the CLI names them.
    pub const ALL: &[Self] = &[
        Self::Vscode,
        Self::Base16,
        Self::Base24,
        Self::Iterm,
        Self::Ghostty,
        Self::Zip,
        Self::DesignTokens,
    ];

    /// Its name on the command line.
    pub fn name(self) -> &'static str {
        match self {
            Self::Vscode => "vscode",
            Self::Base16 => "base16",
            Self::Base24 => "base24",
            Self::Iterm => "iterm",
            Self::Ghostty => "ghostty",
            Self::Zip => "zip",
            Self::DesignTokens => "design-tokens",
        }
    }

    /// The kind `name` names.
    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|it| it.name() == name)
    }
}

/// One key of a source that gave no token a value, and why.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct LeftOut {
    /// The key as the source spells it (`colors.editorRuler.foreground`, `base06`,
    /// `palette 16`).
    pub key: String,
    /// Why nothing took it.
    pub why: String,
}

impl LeftOut {
    pub(super) fn new(key: impl Into<String>, why: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            why: why.into(),
        }
    }
}

/// A theme folder's files, by their path inside the folder (`theme.json`, `fonts/ui.woff2`).
pub type Files = BTreeMap<String, Vec<u8>>;

/// **A theme folder made from a source**: its name, the folder's name, its files and what
/// the source had that the theme does not carry.
#[derive(Debug, Clone, PartialEq)]
pub struct Imported {
    /// What it was made from.
    pub kind: SourceKind,
    /// The theme's name, as `theme.json` writes it.
    pub name: String,
    /// The folder's name: a theme's id.
    pub folder: String,
    /// The folder's files.
    pub files: Files,
    /// Every key of the source that gave no token a value, in the source's order.
    pub left_out: Vec<LeftOut>,
}

impl Imported {
    /// **The palette entry `token` is written as** in the appearance `appearance` (its id) of
    /// the theme this import wrote: `None` when it is written as hex, or when the import has no
    /// such appearance or token.
    pub fn palette_entry(&self, appearance: &str, token: &str) -> Option<String> {
        let document: Value = serde_json::from_slice(self.files.get(super::THEME_JSON)?).ok()?;
        let palette = document.get("palette")?;
        let written = document
            .get("appearances")?
            .as_array()?
            .iter()
            .find(|one| one.get("id").and_then(Value::as_str) == Some(appearance))?
            .get("tokens")?
            .get(token)?
            .as_str()?;
        palette.get(written).is_some().then(|| written.to_owned())
    }
}

/// **The kind of `file_name` holding `bytes`**, from its extension and then its first words;
/// `None` when neither says.
pub fn detect(file_name: &str, bytes: &[u8]) -> Option<SourceKind> {
    let extension = file_name
        .rsplit_once('.')
        .map(|(_, it)| it.to_ascii_lowercase())
        .unwrap_or_default();
    let text = std::str::from_utf8(bytes).ok();
    let scheme = || text.and_then(scheme_kind);
    if file_name
        .to_ascii_lowercase()
        .ends_with(super::design_tokens::FILE_ENDING)
    {
        return Some(SourceKind::DesignTokens);
    }
    match extension.as_str() {
        "zip" => return Some(SourceKind::Zip),
        "tokens" => return Some(SourceKind::DesignTokens),
        "itermcolors" => return Some(SourceKind::Iterm),
        "yaml" | "yml" => return scheme(),
        "json" | "jsonc" => {
            return text.and_then(|text| {
                // Only a design-tokens file may be larger than any other source: it is read,
                // within its own bounds, by its import, never parsed whole here. Any other
                // file that large is a VS Code theme's shape, whose import says it is too large.
                if text.len() as u64 > MOST_SOURCE_BYTES {
                    let tokens =
                        text.contains("\"$value\"") || text.contains(super::design_tokens::VENDOR);
                    return Some(if tokens {
                        SourceKind::DesignTokens
                    } else {
                        SourceKind::Vscode
                    });
                }
                let document = jsonc(text).ok()?;
                // Design tokens first: a tokens file's `colors` group holds tokens, where a
                // VS Code theme's holds colours.
                if super::design_tokens::looks_like(&document) {
                    Some(SourceKind::DesignTokens)
                } else if document.get("colors").is_some() || document.get("tokenColors").is_some()
                {
                    Some(SourceKind::Vscode)
                } else {
                    scheme()
                }
            });
        }
        _ => {}
    }
    if bytes.starts_with(b"PK\x03\x04") {
        return Some(SourceKind::Zip);
    }
    let text = text?.trim_start_matches('\u{feff}').trim_start();
    if text.starts_with("<?xml")
        || text.starts_with("<!DOCTYPE plist")
        || text.starts_with("<plist")
    {
        return Some(SourceKind::Iterm);
    }
    if text.starts_with('{') {
        return detect("source.json", text.as_bytes());
    }
    let ghostty = text.lines().any(|line| {
        line.split_once('=')
            .is_some_and(|(key, _)| matches!(key.trim(), "palette" | "background" | "foreground"))
    });
    if ghostty {
        return Some(SourceKind::Ghostty);
    }
    scheme_kind(text)
}

/// Whether `text` is a base16 or a base24 scheme, YAML or JSON.
fn scheme_kind(text: &str) -> Option<SourceKind> {
    let keys = scheme_keys(text).ok()?;
    let named = |key: &str| keys.iter().any(|(it, _)| it == key);
    let system = keys
        .iter()
        .find(|(key, _)| key == "system")
        .map(|(_, value)| value.as_str());
    if !named("base00") && !named("palette.base00") {
        return None;
    }
    let base24 = system == Some("base24")
        || keys.iter().any(|(key, _)| {
            let key = key.strip_prefix("palette.").unwrap_or(key);
            BASE24_ONLY.contains(&key)
        });
    Some(if base24 {
        SourceKind::Base24
    } else {
        SourceKind::Base16
    })
}

/// **Reads `bytes`, a source of `kind`** named `file_name` (whose stem names the theme when the
/// source does not), into a theme folder. A source larger than [`MOST_SOURCE_BYTES`], or not its
/// kind's grammar, is refused with a sentence that says where.
pub fn import(kind: SourceKind, file_name: &str, bytes: &[u8]) -> Result<Imported, String> {
    if kind == SourceKind::Zip {
        let (folder, files) = super::archive::unpack(bytes)?;
        let name = super::load_text_document(&String::from_utf8_lossy(
            files.get(super::THEME_JSON).map_or(&[][..], Vec::as_slice),
        ))
        .ok()
        .and_then(|it| it.get("name").and_then(Value::as_str).map(str::to_owned))
        .unwrap_or_else(|| folder.clone());
        return Ok(Imported {
            kind,
            name,
            folder,
            files,
            left_out: Vec::new(),
        });
    }
    if kind == SourceKind::DesignTokens {
        let read = super::design_tokens::unpack(file_name, bytes)?;
        return Ok(Imported {
            kind,
            name: read.name,
            folder: read.folder,
            files: read.files,
            left_out: read.left_out,
        });
    }
    if bytes.len() as u64 > MOST_SOURCE_BYTES {
        return Err(format!(
            "{file_name} is larger than {} KiB, the most a theme to import may be",
            MOST_SOURCE_BYTES / 1024
        ));
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|_| format!("{file_name} is not text"))?
        .trim_start_matches('\u{feff}');
    let source = match kind {
        SourceKind::Vscode => vscode(text)?,
        SourceKind::Base16 | SourceKind::Base24 => scheme(text, kind)?,
        SourceKind::Iterm => iterm(text)?,
        SourceKind::Ghostty => ghostty(text)?,
        SourceKind::Zip | SourceKind::DesignTokens => unreachable!("read above"),
    };
    let stem = stem(file_name);
    let name: String = source
        .name
        .clone()
        .filter(|it| !it.trim().is_empty())
        .unwrap_or(stem)
        .trim()
        .chars()
        .take(MOST_NAME)
        .collect();
    let name = if name.is_empty() {
        "Imported theme".to_owned()
    } else {
        name
    };
    let folder = folder_name(&name);
    if super::archive::is_reserved(&folder) {
        return Err(format!(
            "{name} would be the theme folder {folder}, an id purlis keeps for its own themes: \
             rename the theme in its source and import it again"
        ));
    }
    if super::portable::not_held(&folder).is_some() {
        return Err(format!(
            "{name} would be the theme folder {folder}, a name Windows keeps for a device: \
             rename the theme in its source and import it again"
        ));
    }
    let (document, left_out) = write(&name, kind, source);
    let mut text = serde_json::to_string_pretty(&document).unwrap_or_default();
    text.push('\n');
    Ok(Imported {
        kind,
        folder,
        name,
        files: Files::from([(super::THEME_JSON.to_owned(), text.into_bytes())]),
        left_out,
    })
}

/// A file name without its folder and its extension.
fn stem(file_name: &str) -> String {
    let base = file_name.rsplit(['/', '\\']).next().unwrap_or(file_name);
    match base.rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => stem.to_owned(),
        _ => base.to_owned(),
    }
}

/// **The folder a theme called `name` is written to**: lower-case letters, digits and dashes,
/// starting with a letter, at most 32 long; `imported-theme` when its name has none of those.
pub fn folder_name(name: &str) -> String {
    let mut slug = String::new();
    for c in name.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            slug.push(c);
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let mut slug = slug.trim_end_matches('-').to_owned();
    if slug.starts_with(|c: char| c.is_ascii_digit()) {
        slug.insert_str(0, "theme-");
    }
    slug.truncate(32);
    let slug = slug.trim_end_matches('-').to_owned();
    if slug.is_empty() {
        "imported-theme".to_owned()
    } else {
        slug
    }
}

// ------------------------------------------------------------------------------------------
// What a source says, before the table
// ------------------------------------------------------------------------------------------

/// One appearance a source describes: its colours by their source key, and what of a VS Code
/// theme's syntax rules each syntax token takes.
#[derive(Debug, Default)]
struct Look {
    /// The appearance's id, when the source has more than one.
    id: Option<&'static str>,
    tag: Option<Appearance>,
    high_contrast: bool,
    /// Each colour by its source key, hex, in the source's order.
    colours: Vec<(String, String)>,
    /// Each syntax token a rule gave a colour, and its flags.
    syntax: BTreeMap<&'static str, String>,
    flags: BTreeMap<&'static str, bool>,
}

impl Look {
    fn colour(&self, key: &str) -> Option<&str> {
        self.colours
            .iter()
            .rev()
            .find(|(it, _)| it == key)
            .map(|(_, hex)| hex.as_str())
    }
}

/// A source, read.
#[derive(Debug, Default)]
struct Parsed {
    name: Option<String>,
    looks: Vec<Look>,
    /// What the source had that is not a colour any table takes, said as it was read.
    left_out: Vec<LeftOut>,
    /// Whether a colour nothing took is listed as `colors.<key>`, as a VS Code theme keeps it.
    under_colors: bool,
    /// Whether the source's colours are the theme's palette.
    palette: bool,
}

/// A colour as a source spells it, as hex a theme holds: `#rgb`, `#rgba`, `#rrggbb` or
/// `#rrggbbaa`, the `#` optional, in lower case.
fn hex(value: &str) -> Option<String> {
    let digits = value.trim();
    let digits = digits.strip_prefix('#').unwrap_or(digits);
    (matches!(digits.len(), 3 | 4 | 6 | 8) && digits.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| format!("#{}", digits.to_ascii_lowercase()))
}

/// The appearance a background draws: dark when black text would read worse on it than white.
fn tag_of(background: Option<&str>) -> Appearance {
    match background {
        Some(back)
            if super::check::contrast(back, "#000000")
                > super::check::contrast(back, "#ffffff") =>
        {
            Appearance::Light
        }
        _ => Appearance::Dark,
    }
}

// ------------------------------------------------------------------------------------------
// The mapping tables
// ------------------------------------------------------------------------------------------

/// A mapping table: each token, and the source keys it takes, nearest first.
type Table = &'static [(&'static str, &'static [&'static str])];

/// **The mapping table of `kind`**: each token, in the order [`TOKENS`] lists them, and the
/// source keys it takes, nearest first. The tables in `purlis docs show themes` are these
/// ([`table_markdown`]). A zip has none: it is a theme folder already.
pub fn table(kind: SourceKind) -> Vec<(&'static str, &'static [&'static str])> {
    let layers: &[Table] = match kind {
        SourceKind::Vscode => &[VSCODE],
        SourceKind::Base16 => &[BASE16_SHARED, BASE16_BRIGHT],
        SourceKind::Base24 => &[BASE24_OWN, BASE16_SHARED],
        SourceKind::Iterm => &[ITERM],
        SourceKind::Ghostty => &[GHOSTTY],
        SourceKind::Zip | SourceKind::DesignTokens => &[],
    };
    TOKENS
        .iter()
        .filter_map(|token| {
            layers
                .iter()
                .find_map(|layer| layer.iter().find(|(it, _)| it == token))
                .copied()
        })
        .collect()
}

/// `kind`'s mapping table as the docs print it: a Markdown table of each token and the source
/// keys it takes, nearest first.
pub fn table_markdown(kind: SourceKind) -> String {
    let mut out = String::from("| Token | Takes, nearest first |\n|---|---|\n");
    for (token, keys) in table(kind) {
        let keys: Vec<String> = keys.iter().map(|key| format!("`{key}`")).collect();
        out.push_str(&format!("| `{token}` | {} |\n", keys.join(", ")));
    }
    out
}

/// [`SCOPES`] as the docs print it: a Markdown table of each syntax token and the TextMate
/// scopes it takes, nearest first.
pub fn scopes_markdown() -> String {
    let mut out = String::from("| Token | Scopes, nearest first |\n|---|---|\n");
    for (token, scopes) in SCOPES {
        let scopes: Vec<String> = scopes.iter().map(|it| format!("`{it}`")).collect();
        out.push_str(&format!("| `{token}` | {} |\n", scopes.join(", ")));
    }
    out
}

/// A VS Code theme's `colors`.
const VSCODE: Table = &[
    ("surface.base", &["editor.background"]),
    ("surface.sunken", &["sideBar.background"]),
    (
        "surface.deep",
        &["titleBar.activeBackground", "activityBar.background"],
    ),
    ("surface.raised", &["editorWidget.background"]),
    (
        "surface.overlay",
        &[
            "quickInput.background",
            "menu.background",
            "editorWidget.background",
        ],
    ),
    ("surface.hover", &["list.hoverBackground"]),
    ("control.base", &["input.background", "dropdown.background"]),
    ("control.hover", &["toolbar.hoverBackground"]),
    ("text.primary", &["editor.foreground", "foreground"]),
    ("text.secondary", &["descriptionForeground"]),
    (
        "text.muted",
        &["editorLineNumber.foreground", "disabledForeground"],
    ),
    ("text.quiet", &["editorLineNumber.foreground"]),
    ("text.faint", &["disabledForeground"]),
    (
        "border.subtle",
        &["panel.border", "sideBar.border", "editorGroup.border"],
    ),
    ("border.strong", &["input.border", "contrastBorder"]),
    ("accent.base", &["button.background", "focusBorder"]),
    ("focus.ring", &["focusBorder"]),
    ("tab.active", &["tab.activeBackground"]),
    ("list.selected", &["list.activeSelectionBackground"]),
    ("tree.guide", &["tree.indentGuidesStroke"]),
    (
        "danger.base",
        &["errorForeground", "editorError.foreground"],
    ),
    (
        "diff.deleted",
        &[
            "diffEditor.removedLineBackground",
            "diffEditor.removedTextBackground",
        ],
    ),
    (
        "diff.inserted",
        &[
            "diffEditor.insertedLineBackground",
            "diffEditor.insertedTextBackground",
        ],
    ),
    ("diff.deleted-text", &["diffEditor.removedTextBackground"]),
    ("diff.inserted-text", &["diffEditor.insertedTextBackground"]),
    ("icon.red", &["charts.red"]),
    ("icon.orange", &["charts.orange"]),
    ("icon.yellow", &["charts.yellow"]),
    ("icon.green", &["charts.green"]),
    ("icon.blue", &["charts.blue"]),
    ("icon.purple", &["charts.purple"]),
    ("state.running", &["progressBar.background"]),
    ("state.waiting", &["editorWarning.foreground"]),
    (
        "state.failed",
        &["editorError.foreground", "errorForeground"],
    ),
    ("state.success", &["testing.iconPassed"]),
    ("overlay.shadow", &["widget.shadow"]),
    (
        "terminal.background",
        &[
            "terminal.background",
            "panel.background",
            "editor.background",
        ],
    ),
    (
        "terminal.foreground",
        &["terminal.foreground", "editor.foreground", "foreground"],
    ),
    (
        "terminal.cursor",
        &["terminalCursor.foreground", "editorCursor.foreground"],
    ),
    ("terminal.cursor-accent", &["terminalCursor.background"]),
    (
        "terminal.selection",
        &["terminal.selectionBackground", "editor.selectionBackground"],
    ),
    (
        "terminal.find-match",
        &["terminal.findMatchHighlightBackground"],
    ),
    (
        "terminal.find-match-active",
        &["terminal.findMatchBackground"],
    ),
    (
        "terminal.selection-foreground",
        &["terminal.selectionForeground"],
    ),
    (
        "terminal.selection-inactive",
        &["terminal.inactiveSelectionBackground"],
    ),
    ("terminal.scrollbar", &["scrollbarSlider.background"]),
    (
        "terminal.scrollbar-hover",
        &["scrollbarSlider.hoverBackground"],
    ),
    (
        "terminal.scrollbar-active",
        &["scrollbarSlider.activeBackground"],
    ),
    ("terminal.ansi.black", &["terminal.ansiBlack"]),
    ("terminal.ansi.red", &["terminal.ansiRed"]),
    ("terminal.ansi.green", &["terminal.ansiGreen"]),
    ("terminal.ansi.yellow", &["terminal.ansiYellow"]),
    ("terminal.ansi.blue", &["terminal.ansiBlue"]),
    ("terminal.ansi.magenta", &["terminal.ansiMagenta"]),
    ("terminal.ansi.cyan", &["terminal.ansiCyan"]),
    ("terminal.ansi.white", &["terminal.ansiWhite"]),
    ("terminal.ansi.bright-black", &["terminal.ansiBrightBlack"]),
    ("terminal.ansi.bright-red", &["terminal.ansiBrightRed"]),
    ("terminal.ansi.bright-green", &["terminal.ansiBrightGreen"]),
    (
        "terminal.ansi.bright-yellow",
        &["terminal.ansiBrightYellow"],
    ),
    ("terminal.ansi.bright-blue", &["terminal.ansiBrightBlue"]),
    (
        "terminal.ansi.bright-magenta",
        &["terminal.ansiBrightMagenta"],
    ),
    ("terminal.ansi.bright-cyan", &["terminal.ansiBrightCyan"]),
    ("terminal.ansi.bright-white", &["terminal.ansiBrightWhite"]),
    ("markdown.link", &["textLink.foreground"]),
    ("markdown.code", &["textPreformat.foreground"]),
    ("markdown.code-block", &["textPreformat.foreground"]),
    ("selection.background", &["editor.selectionBackground"]),
    ("selection.text", &["editor.selectionForeground"]),
    ("caret", &["editorCursor.foreground"]),
    ("placeholder", &["input.placeholderForeground"]),
    ("scrollbar.thumb", &["scrollbarSlider.background"]),
    ("chart.1", &["charts.blue"]),
    ("chart.2", &["charts.green"]),
    ("chart.3", &["charts.yellow"]),
    ("chart.4", &["charts.purple"]),
    ("chart.5", &["charts.red"]),
    ("chart.6", &["charts.orange"]),
    ("chart.7", &["terminal.ansiCyan"]),
    ("chart.8", &["terminal.ansiMagenta"]),
    (
        "window.background",
        &["titleBar.activeBackground", "editor.background"],
    ),
];

/// **A VS Code theme's `tokenColors`**: each syntax token and the TextMate scopes it takes,
/// nearest first. A rule's scope takes a token when it is one of these or starts with one and a
/// dot (`keyword.control` is `keyword`'s), unless a longer scope of another row holds it
/// (`keyword.operator` is `syntax.operator`'s); a scope with a space in it (`meta.embedded string`)
/// is about a context and takes none.
pub const SCOPES: &[(&str, &[&str])] = &[
    ("syntax.keyword", &["keyword", "storage"]),
    ("syntax.string", &["string"]),
    ("syntax.number", &["constant.numeric"]),
    (
        "syntax.constant",
        &["constant.language", "constant", "support.constant"],
    ),
    ("syntax.comment", &["comment"]),
    (
        "syntax.function",
        &["entity.name.function", "support.function"],
    ),
    (
        "syntax.type",
        &[
            "entity.name.type",
            "entity.name.class",
            "support.type",
            "support.class",
        ],
    ),
    (
        "syntax.property",
        &[
            "variable.other.property",
            "support.type.property-name",
            "meta.object-literal.key",
        ],
    ),
    ("syntax.attribute", &["entity.other.attribute-name"]),
    ("syntax.tag", &["entity.name.tag"]),
    ("syntax.heading", &["markup.heading", "entity.name.section"]),
    ("syntax.invalid", &["invalid"]),
    ("syntax.variable", &["variable"]),
    ("syntax.operator", &["keyword.operator"]),
    ("syntax.punctuation", &["punctuation"]),
    ("markdown.heading", &["markup.heading"]),
];

/// The base16 rows every scheme shares (the styling guide's roles, and base16-shell's terminal).
macro_rules! base16_rows {
    () => {
        [
            ("surface.base", &["base00"]),
            ("surface.sunken", &["base01"]),
            ("surface.raised", &["base01"]),
            ("surface.overlay", &["base01"]),
            ("surface.hover", &["base02"]),
            ("control.base", &["base01"]),
            ("control.hover", &["base02"]),
            ("text.primary", &["base05"]),
            ("text.secondary", &["base04"]),
            ("text.muted", &["base03"]),
            ("text.faint", &["base03"]),
            ("border.subtle", &["base02"]),
            ("border.strong", &["base03"]),
            ("accent.base", &["base0d"]),
            ("focus.ring", &["base0d"]),
            ("list.selected", &["base02"]),
            ("danger.base", &["base08"]),
            ("icon.red", &["base08"]),
            ("icon.orange", &["base09"]),
            ("icon.yellow", &["base0a"]),
            ("icon.green", &["base0b"]),
            ("icon.teal", &["base0c"]),
            ("icon.blue", &["base0d"]),
            ("icon.purple", &["base0e"]),
            ("icon.pink", &["base0f"]),
            ("state.running", &["base0d"]),
            ("state.waiting", &["base0a"]),
            ("state.failed", &["base08"]),
            ("state.success", &["base0b"]),
            ("state.unreadable", &["base09"]),
            ("needs-you.base", &["base08"]),
            ("needs-you.text", &["base00"]),
            ("terminal.background", &["base00"]),
            ("terminal.foreground", &["base05"]),
            ("terminal.cursor", &["base05"]),
            ("terminal.cursor-accent", &["base00"]),
            ("terminal.selection", &["base02"]),
            ("terminal.ansi.black", &["base00"]),
            ("terminal.ansi.red", &["base08"]),
            ("terminal.ansi.green", &["base0b"]),
            ("terminal.ansi.yellow", &["base0a"]),
            ("terminal.ansi.blue", &["base0d"]),
            ("terminal.ansi.magenta", &["base0e"]),
            ("terminal.ansi.cyan", &["base0c"]),
            ("terminal.ansi.white", &["base05"]),
            ("terminal.ansi.bright-black", &["base03"]),
            ("terminal.ansi.bright-white", &["base07"]),
            ("syntax.keyword", &["base0e"]),
            ("syntax.string", &["base0b"]),
            ("syntax.number", &["base09"]),
            ("syntax.constant", &["base09"]),
            ("syntax.comment", &["base03"]),
            ("syntax.function", &["base0d"]),
            ("syntax.type", &["base0a"]),
            ("syntax.property", &["base0c"]),
            ("syntax.attribute", &["base09"]),
            ("syntax.tag", &["base08"]),
            ("syntax.heading", &["base0d"]),
            ("syntax.invalid", &["base08"]),
            ("syntax.variable", &["base08"]),
            ("syntax.operator", &["base05"]),
            ("syntax.punctuation", &["base05"]),
            ("markdown.link", &["base0d"]),
            ("markdown.code", &["base0b"]),
            ("markdown.code-block", &["base05"]),
            ("markdown.quote", &["base0c"]),
            ("markdown.heading", &["base0d"]),
            ("selection.background", &["base02"]),
            ("caret", &["base05"]),
            ("placeholder", &["base04"]),
            ("scrollbar.thumb", &["base03"]),
            ("chart.1", &["base0d"]),
            ("chart.2", &["base0b"]),
            ("chart.3", &["base0a"]),
            ("chart.4", &["base0e"]),
            ("chart.5", &["base08"]),
            ("chart.6", &["base0c"]),
            ("chart.7", &["base09"]),
            ("chart.8", &["base0f"]),
            ("window.background", &["base00"]),
        ]
    };
}

/// The rows every scheme shares.
const BASE16_SHARED: Table = &base16_rows!();

/// A base16 scheme's bright terminal colours, as base16-shell draws them: the normal ones.
const BASE16_BRIGHT: Table = &[
    ("terminal.ansi.bright-red", &["base08"]),
    ("terminal.ansi.bright-green", &["base0b"]),
    ("terminal.ansi.bright-yellow", &["base0a"]),
    ("terminal.ansi.bright-blue", &["base0d"]),
    ("terminal.ansi.bright-magenta", &["base0e"]),
    ("terminal.ansi.bright-cyan", &["base0c"]),
];

/// The keys only a base24 scheme has.
const BASE24_ONLY: &[&str] = &[
    "base10", "base11", "base12", "base13", "base14", "base15", "base16", "base17",
];

/// A base24 scheme's own rows, over the shared ones: its two deeper backgrounds take the strips
/// that read as further back, and its own bright colours.
const BASE24_OWN: Table = &[
    ("surface.sunken", &["base10", "base01"]),
    ("surface.deep", &["base11", "base10"]),
    ("terminal.ansi.bright-red", &["base12", "base08"]),
    ("terminal.ansi.bright-yellow", &["base13", "base0a"]),
    ("terminal.ansi.bright-green", &["base14", "base0b"]),
    ("terminal.ansi.bright-cyan", &["base15", "base0c"]),
    ("terminal.ansi.bright-blue", &["base16", "base0d"]),
    ("terminal.ansi.bright-magenta", &["base17", "base0e"]),
];

/// The rows a terminal palette fills, by `$ansi(n)`, `$bg`, `$fg` and the rest, written once for
/// iTerm2 and Ghostty with each one's own keys.
macro_rules! terminal_rows {
    ($a0:literal, $a1:literal, $a2:literal, $a3:literal, $a4:literal, $a5:literal, $a6:literal,
     $a7:literal, $a8:literal, $a9:literal, $a10:literal, $a11:literal, $a12:literal,
     $a13:literal, $a14:literal, $a15:literal, $bg:literal, $fg:literal, $cursor:literal,
     $cursor_text:literal, $selection:literal, $selected_text:literal, $link:expr) => {
        &[
            ("surface.base", &[$bg]),
            ("text.primary", &[$fg]),
            ("text.muted", &[$a8]),
            ("accent.base", &[$a4]),
            ("focus.ring", &[$a4]),
            ("danger.base", &[$a1]),
            ("icon.red", &[$a1]),
            ("icon.yellow", &[$a3]),
            ("icon.green", &[$a2]),
            ("icon.teal", &[$a6]),
            ("icon.blue", &[$a4]),
            ("icon.purple", &[$a5]),
            ("state.running", &[$a4]),
            ("state.waiting", &[$a3]),
            ("state.failed", &[$a1]),
            ("state.success", &[$a2]),
            ("terminal.background", &[$bg]),
            ("terminal.foreground", &[$fg]),
            ("terminal.cursor", &[$cursor]),
            ("terminal.cursor-accent", &[$cursor_text]),
            ("terminal.selection", &[$selection]),
            ("terminal.selection-foreground", &[$selected_text]),
            ("terminal.ansi.black", &[$a0]),
            ("terminal.ansi.red", &[$a1]),
            ("terminal.ansi.green", &[$a2]),
            ("terminal.ansi.yellow", &[$a3]),
            ("terminal.ansi.blue", &[$a4]),
            ("terminal.ansi.magenta", &[$a5]),
            ("terminal.ansi.cyan", &[$a6]),
            ("terminal.ansi.white", &[$a7]),
            ("terminal.ansi.bright-black", &[$a8]),
            ("terminal.ansi.bright-red", &[$a9]),
            ("terminal.ansi.bright-green", &[$a10]),
            ("terminal.ansi.bright-yellow", &[$a11]),
            ("terminal.ansi.bright-blue", &[$a12]),
            ("terminal.ansi.bright-magenta", &[$a13]),
            ("terminal.ansi.bright-cyan", &[$a14]),
            ("terminal.ansi.bright-white", &[$a15]),
            ("syntax.keyword", &[$a5]),
            ("syntax.string", &[$a2]),
            ("syntax.number", &[$a11]),
            ("syntax.constant", &[$a11]),
            ("syntax.comment", &[$a8]),
            ("syntax.function", &[$a4]),
            ("syntax.type", &[$a3]),
            ("syntax.property", &[$a6]),
            ("syntax.tag", &[$a1]),
            ("syntax.heading", &[$a4]),
            ("syntax.invalid", &[$a1]),
            ("syntax.variable", &[$a1]),
            ("markdown.link", $link),
            ("markdown.code", &[$a2]),
            ("markdown.quote", &[$a6]),
            ("markdown.heading", &[$a4]),
            ("selection.background", &[$selection]),
            ("selection.text", &[$selected_text]),
            ("caret", &[$cursor]),
            ("chart.1", &[$a4]),
            ("chart.2", &[$a2]),
            ("chart.3", &[$a3]),
            ("chart.4", &[$a5]),
            ("chart.5", &[$a1]),
            ("chart.6", &[$a6]),
            ("chart.7", &[$a12]),
            ("chart.8", &[$a13]),
            ("window.background", &[$bg]),
        ]
    };
}

/// An iTerm2 colour preset.
const ITERM: Table = terminal_rows!(
    "Ansi 0 Color",
    "Ansi 1 Color",
    "Ansi 2 Color",
    "Ansi 3 Color",
    "Ansi 4 Color",
    "Ansi 5 Color",
    "Ansi 6 Color",
    "Ansi 7 Color",
    "Ansi 8 Color",
    "Ansi 9 Color",
    "Ansi 10 Color",
    "Ansi 11 Color",
    "Ansi 12 Color",
    "Ansi 13 Color",
    "Ansi 14 Color",
    "Ansi 15 Color",
    "Background Color",
    "Foreground Color",
    "Cursor Color",
    "Cursor Text Color",
    "Selection Color",
    "Selected Text Color",
    &["Link Color", "Ansi 4 Color"]
);

/// A Ghostty theme.
const GHOSTTY: Table = terminal_rows!(
    "palette 0",
    "palette 1",
    "palette 2",
    "palette 3",
    "palette 4",
    "palette 5",
    "palette 6",
    "palette 7",
    "palette 8",
    "palette 9",
    "palette 10",
    "palette 11",
    "palette 12",
    "palette 13",
    "palette 14",
    "palette 15",
    "background",
    "foreground",
    "cursor-color",
    "cursor-text",
    "selection-background",
    "selection-foreground",
    &["palette 4"]
);

// ------------------------------------------------------------------------------------------
// The recipes (#2589)
// ------------------------------------------------------------------------------------------

/// **How a token is tuned once its table row filled it**, so the theme's words read on its
/// grounds where the source's own colours allow it. A base16 scheme makes some roles one colour
/// (ANSI black is the background, the selected row is the row under the pointer), and a scheme
/// or a terminal names hues, not words on a ground: the recipes take a nearer step on the
/// source's own scale, or a colour between two of its own, for the tokens that need a
/// difference. The tables in `purlis docs show themes` print each kind's
/// ([`recipes_markdown`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recipe {
    /// A token the table left unfilled takes the first of `keys` the source has, when the
    /// source filled `when`: selected text in the source's foreground, on a selection that is
    /// the source's own.
    Fill {
        keys: &'static [&'static str],
        when: &'static str,
    },
    /// The colour halfway between two the source has, as the screen mixes them: a step the
    /// source's scale does not have, between two roles it makes one colour.
    Between(&'static str, &'static str),
    /// The token's own colour, or a point on the way from it toward each of these keys in turn,
    /// a tenth of a step at a time ([`TOWARD_STEPS`]): the nearest point that leaves the theme
    /// check the fewest findings for the token, the consent floors it holds counted with them.
    /// A point no better than the source's own colour is never taken.
    Toward(&'static [&'static str]),
    /// Whichever of these keys the source has reads most on the window (`surface.base`), the
    /// first of them on a tie: a token two of the source's greys could fill takes the louder.
    Loudest(&'static [&'static str]),
    /// Left out when it reads louder on the window than `other`, so it follows the tier above
    /// it: a fainter tier is never drawn louder than the one it sits under.
    NoLouderThan(&'static str),
}

/// The points [`Recipe::Toward`] tries on each step of its way.
pub const TOWARD_STEPS: u32 = 10;

/// A kind's recipes: each token and how it is tuned, in the order they are applied (the
/// grounds before what is drawn on them).
type Recipes = &'static [(&'static str, Recipe)];

/// The tokens a scheme or a terminal fills with a hue (an accent, a state, a syntax role, an ANSI
/// colour on the terminal) that are drawn as words or marks on a ground: each is walked toward
/// the source's foreground when it does not read there.
const HUES: &[&str] = &[
    "danger.base",
    "accent.base",
    "focus.ring",
    "icon.red",
    "icon.orange",
    "icon.yellow",
    "icon.green",
    "icon.teal",
    "icon.blue",
    "icon.purple",
    "icon.pink",
    "state.running",
    "state.waiting",
    "state.failed",
    "state.success",
    "state.unreadable",
    "needs-you.base",
    "syntax.keyword",
    "syntax.string",
    "syntax.number",
    "syntax.constant",
    "syntax.function",
    "syntax.type",
    "syntax.property",
    "syntax.attribute",
    "syntax.tag",
    "syntax.heading",
    "syntax.invalid",
    "syntax.variable",
    "markdown.link",
    "markdown.code",
    "markdown.quote",
    "markdown.heading",
    "chart.1",
    "chart.2",
    "chart.3",
    "chart.4",
    "chart.5",
    "chart.6",
    "chart.7",
    "chart.8",
];

/// The ANSI colours a base16 scheme's hues fill by base16-shell's convention: a scheme names
/// hues on a scale, not a terminal's colours, so they are tuned as the window's hues are. A
/// terminal's own sixteen are its own statement, and are never tuned.
const BASE16_ANSI_HUES: &[&str] = &[
    "terminal.ansi.red",
    "terminal.ansi.green",
    "terminal.ansi.yellow",
    "terminal.ansi.blue",
    "terminal.ansi.magenta",
    "terminal.ansi.cyan",
    "terminal.ansi.bright-red",
    "terminal.ansi.bright-green",
    "terminal.ansi.bright-yellow",
    "terminal.ansi.bright-blue",
    "terminal.ansi.bright-magenta",
    "terminal.ansi.bright-cyan",
];

/// **A base16 or base24 scheme's recipes.** Its scale runs from `base00` (the background) to
/// `base07`, `base05` the words:
///
/// - the row under the pointer is halfway between `base01` and `base02`, so it is told apart from
///   the selected row (`base02`) and from the panes (`base01`);
/// - selected text is `base05`, the words' colour, as base16's editor templates draw it;
/// - ANSI black, which base16-shell makes the background itself, takes the nearest step up the
///   scale that the terminal shows;
/// - the muted words, comments and bright black (`base03`) and the secondary words and
///   placeholder (`base04`) take the nearest step toward `base05` that reads;
/// - faint, the marks (`base03`, the styling guide's invisibles), takes the nearest step toward
///   `base05` that reads as a mark, and is left out, following the muted words, when that step
///   reads louder than they do;
/// - each hue ([`HUES`], and the ANSI colours it fills) takes the nearest step toward `base05`
///   that reads, and stays as the scheme wrote it when it reads already.
const BASE16_RECIPES: Recipes = &[
    ("surface.hover", Recipe::Between("base01", "base02")),
    (
        "selection.text",
        Recipe::Fill {
            keys: &["base05"],
            when: "selection.background",
        },
    ),
    (
        "terminal.ansi.black",
        Recipe::Toward(&["base01", "base02", "base03"]),
    ),
    ("text.muted", Recipe::Toward(&["base04", "base05"])),
    ("text.faint", Recipe::Toward(&["base04", "base05"])),
    ("text.faint", Recipe::NoLouderThan("text.muted")),
    ("syntax.comment", Recipe::Toward(&["base04", "base05"])),
    (
        "terminal.ansi.bright-black",
        Recipe::Toward(&["base04", "base05"]),
    ),
    ("text.secondary", Recipe::Toward(&["base05"])),
    ("placeholder", Recipe::Toward(&["base05"])),
];

/// The foreground a base16 scheme's hues are walked toward.
const BASE16_FOREGROUND: &[&str] = &["base05"];

/// **A terminal palette's recipes**, iTerm2's and Ghostty's alike with each one's keys: selected
/// text in the foreground when the palette colours the selection and not its text, and the muted
/// words, the comments and each hue the nearest step toward the foreground that reads. The
/// terminal's own colours (its background, its words, its sixteen ANSI colours, its cursor and
/// its selection) are what the terminal draws, and are taken as they are.
macro_rules! terminal_recipes {
    ($fg:literal) => {
        &[
            (
                "selection.text",
                Recipe::Fill {
                    keys: &[$fg],
                    when: "selection.background",
                },
            ),
            ("text.muted", Recipe::Toward(&[$fg])),
            ("syntax.comment", Recipe::Toward(&[$fg])),
        ]
    };
}

/// An iTerm2 colour preset's recipes.
const ITERM_RECIPES: Recipes = terminal_recipes!("Foreground Color");

/// A Ghostty theme's recipes.
const GHOSTTY_RECIPES: Recipes = terminal_recipes!("foreground");

/// **A VS Code theme's recipes**: it names a colour for nearly every role, and those are taken
/// as they are. Its selected text, when it colours the selection and not the text, is the
/// editor's words, so the window's words on a selection read as the editor's do. Its muted words
/// take the louder of its line numbers' grey and its disabled grey, so muted is never fainter
/// than quiet or faint; and its faint (the disabled grey) is left out, following quiet, when it
/// reads louder than the line numbers.
const VSCODE_RECIPES: Recipes = &[
    (
        "selection.text",
        Recipe::Fill {
            keys: &["editor.foreground", "foreground"],
            when: "selection.background",
        },
    ),
    (
        "text.muted",
        Recipe::Loudest(&["editorLineNumber.foreground", "disabledForeground"]),
    ),
    ("text.faint", Recipe::NoLouderThan("text.quiet")),
];

/// **The recipes of `kind`**, in the order they are applied: its own, then each hue walked
/// toward its foreground. A VS Code theme's hues and a zip's are taken as they are.
pub fn recipes(kind: SourceKind) -> Vec<(&'static str, Recipe)> {
    let (own, foreground, ansi): (Recipes, &'static [&'static str], &[&'static str]) = match kind {
        SourceKind::Vscode => (VSCODE_RECIPES, &[], &[]),
        SourceKind::Base16 | SourceKind::Base24 => {
            (BASE16_RECIPES, BASE16_FOREGROUND, BASE16_ANSI_HUES)
        }
        SourceKind::Iterm => (ITERM_RECIPES, &["Foreground Color"], &[]),
        SourceKind::Ghostty => (GHOSTTY_RECIPES, &["foreground"], &[]),
        SourceKind::Zip | SourceKind::DesignTokens => (&[], &[], &[]),
    };
    let mut out = own.to_vec();
    if !foreground.is_empty() {
        for token in HUES.iter().chain(ansi) {
            out.push((*token, Recipe::Toward(foreground)));
        }
    }
    out
}

/// `kind`'s recipes as the docs print them: a Markdown table of each token and how it is tuned.
pub fn recipes_markdown(kind: SourceKind) -> String {
    let quoted = |keys: &[&str]| -> String {
        keys.iter()
            .map(|key| format!("`{key}`"))
            .collect::<Vec<_>>()
            .join(", then ")
    };
    let mut out = String::from("| Token | Tuned |\n|---|---|\n");
    for (token, recipe) in recipes(kind) {
        let said = match recipe {
            Recipe::Fill { keys, when } => format!(
                "when the theme's `{when}` is filled and the table left it: {}",
                keys.iter()
                    .map(|key| format!("`{key}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Recipe::Between(one, other) => format!("halfway between `{one}` and `{other}`"),
            Recipe::Toward(keys) => format!("toward {}", quoted(keys)),
            Recipe::Loudest(keys) => format!(
                "the one of {} that reads most on `surface.base`",
                keys.iter()
                    .map(|key| format!("`{key}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Recipe::NoLouderThan(other) => {
                format!("left out when it reads louder than `{other}` on `surface.base`")
            }
        };
        out.push_str(&format!("| `{token}` | {said} |\n"));
    }
    out
}

/// **`tokens`, one appearance of the theme being written, tuned by `kind`'s recipes.** Each
/// [`Recipe::Toward`] is measured by the theme check of the appearance as the engine reads it,
/// and the consent floors drawing it holds. A token the table did not fill is never walked: it
/// is Purlis Classic's, not the source's.
fn tune(
    kind: SourceKind,
    look: &Look,
    document: &Value,
    tokens: &mut Map<String, Value>,
    mut named: impl FnMut(&'static str, &str) -> Value,
) {
    let recipes = recipes(kind);
    if recipes.is_empty() {
        return;
    }
    let mut document = document.clone();
    let resolved = |document: &mut Value, tokens: &Map<String, Value>| -> Option<ResolvedTheme> {
        document["appearances"][0]["tokens"] = Value::Object(tokens.clone());
        // Read only to check: a walk registers none of the theme's fonts.
        let loaded = super::Library::new().checking().load(None, document);
        (!loaded.refused).then(|| loaded.theme.first().clone())
    };
    for (token, recipe) in recipes {
        match recipe {
            Recipe::Fill { keys, when } => {
                if tokens.contains_key(token) || !tokens.contains_key(when) {
                    continue;
                }
                if let Some((key, hex)) = keys
                    .iter()
                    .find_map(|key| look.colour(key).map(|hex| (*key, hex)))
                {
                    tokens.insert(token.to_owned(), named(key, hex));
                }
            }
            Recipe::Between(one, other) => {
                if let (Some(one), Some(other)) = (look.colour(one), look.colour(other))
                    && one.len() == 7
                    && other.len() == 7
                    && tokens.contains_key(token)
                {
                    let halfway = super::check::mix(one, other, 0.5);
                    tokens.insert(token.to_owned(), Value::from(halfway));
                }
            }
            Recipe::Toward(keys) => {
                if !tokens.contains_key(token) {
                    continue;
                }
                let Some(theme) = resolved(&mut document, tokens) else {
                    return;
                };
                if let Some(nearer) = toward(&theme, token, keys, look) {
                    tokens.insert(token.to_owned(), Value::from(nearer));
                }
            }
            Recipe::Loudest(keys) => {
                if !tokens.contains_key(token) {
                    continue;
                }
                let Some(theme) = resolved(&mut document, tokens) else {
                    return;
                };
                let window = theme.value("surface.base");
                let mut loudest: Option<(&'static str, &str, f64)> = None;
                for key in keys {
                    if let Some(hex) = look.colour(key) {
                        let reads = reads_on(hex, window);
                        if loudest.is_none_or(|(_, _, most)| reads > most) {
                            loudest = Some((*key, hex, reads));
                        }
                    }
                }
                if let Some((key, hex, _)) = loudest {
                    tokens.insert(token.to_owned(), named(key, hex));
                }
            }
            Recipe::NoLouderThan(other) => {
                if !tokens.contains_key(token) {
                    continue;
                }
                let Some(theme) = resolved(&mut document, tokens) else {
                    return;
                };
                let window = theme.value("surface.base");
                if reads_on(theme.value(token), window) > reads_on(theme.value(other), window) {
                    tokens.shift_remove(token);
                }
            }
        }
    }
}

/// How `colour` reads on the opaque `window`: its contrast, laid over the window when it has
/// alpha, as the check measures it.
fn reads_on(colour: &str, window: &str) -> f64 {
    super::check::contrast(&super::check::onto(colour, window), window)
}

/// The point [`Recipe::Toward`] takes for `token` in `theme`: `None` when its own colour leaves
/// as few findings as any point on the way.
fn toward(
    theme: &ResolvedTheme,
    token: &'static str,
    keys: &[&str],
    look: &Look,
) -> Option<String> {
    let own = theme.value(token).to_owned();
    if own.len() != 7 {
        return None;
    }
    let mut fewest = findings_for(theme, token);
    if fewest == 0 {
        return None;
    }
    let mut taken = None;
    let mut from = own;
    let mut trial = theme.clone();
    for key in keys {
        let Some(to) = look.colour(key).filter(|it| it.len() == 7) else {
            continue;
        };
        for step in 1..=TOWARD_STEPS {
            let point = super::check::mix(to, &from, f64::from(step) / f64::from(TOWARD_STEPS));
            trial.values.insert(token, point.clone());
            let found = findings_for(&trial, token);
            if found < fewest {
                fewest = found;
                taken = Some(point);
                if found == 0 {
                    return taken;
                }
            }
        }
        from = to.to_owned();
    }
    taken
}

/// The theme check's findings for `token` drawn on something in `theme`, as it is and tinted by
/// each workspace colour, as [`super::check`] counts them, and the tokens the consent floors
/// hold when it is drawn, for a token a floor names. Only the pairs `token` is drawn in are
/// measured: a walk measures one token at a time.
fn findings_for(theme: &ResolvedTheme, token: &'static str) -> usize {
    let pairs: Vec<&super::check::Pair> = super::check::PAIRS
        .iter()
        .filter(|pair| pair.front == token)
        .collect();
    let ansi = token.starts_with("terminal.ansi.");
    let count = |drawn: &ResolvedTheme| -> usize {
        let under = pairs
            .iter()
            .filter(|pair| super::check::ratio(drawn, pair).is_some_and(|ratio| ratio < pair.floor))
            .count();
        // The check holds an ANSI colour by its ratio unrounded.
        let ansi_under = ansi
            && super::check::contrast(drawn.value(token), drawn.value("terminal.background"))
                < super::check::ansi_floor(token);
        under + usize::from(ansi_under)
    };
    let mut found = count(theme);
    let tabs = theme.tint.tabs();
    if !tabs.is_empty() {
        let tinted_in = |it: &str| tabs.contains(&it);
        let touched = tinted_in(token)
            || (ansi && tinted_in("terminal.background"))
            || pairs.iter().any(|pair| pair.tokens().any(tinted_in));
        for one in &theme.tint.hues {
            // A tint that turns none of what is measured measures as the theme does.
            found += if touched {
                count(&super::tint::tinted(theme, one.name))
            } else {
                count(theme)
            };
        }
    }
    let held = if FLOORED.contains(&token) || token.starts_with("state.") {
        super::floors::hold(theme)
            .1
            .map_or(0, |held| held.tokens.len() + held.untinted.len())
    } else {
        0
    };
    found + held
}

/// The tokens a walk may move that a consent floor names besides the state marks: the danger
/// colour, Allow's edge and the needs-you mark under its count.
const FLOORED: &[&str] = &["danger.base", "accent.base", "needs-you.base"];

// ------------------------------------------------------------------------------------------
// Writing the theme
// ------------------------------------------------------------------------------------------

/// The palette name a source key's colour is kept under: its words in lower case joined by
/// dashes, without iTerm2's trailing "Color" (`Ansi 4 Color` is `ansi-4`, `palette 4` is
/// `palette-4`, `base0D` is `base0d`), prefixed by the appearance's id when there are several.
fn palette_name(key: &str, prefix: Option<&str>) -> String {
    let key = key.strip_suffix(" Color").unwrap_or(key);
    let mut name = String::new();
    if let Some(prefix) = prefix {
        name.push_str(prefix);
        name.push('-');
    }
    for c in key.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            name.push(c);
        } else if !name.is_empty() && !name.ends_with('-') {
            name.push('-');
        }
    }
    let mut name = name.trim_end_matches('-').to_owned();
    if !name.starts_with(|c: char| c.is_ascii_lowercase()) {
        name.insert(0, 'c');
    }
    name.truncate(32);
    name.trim_end_matches('-').to_owned()
}

/// The theme `name` written from `source`, and every key nothing took.
fn write(name: &str, kind: SourceKind, source: Parsed) -> (Value, Vec<LeftOut>) {
    let table = table(kind);
    let several = source.looks.len() > 1;
    let mut palette = Map::new();
    let mut used: BTreeSet<String> = BTreeSet::new();
    let mut appearances = Vec::new();
    for look in &source.looks {
        // The value a token takes from the source's `key`: the palette's name for it, or its hex.
        let prefix = look.id.filter(|_| several);
        let named = |key: &str,
                     hex: &str,
                     palette: &mut Map<String, Value>,
                     used: &mut BTreeSet<String>| {
            used.insert(key.to_owned());
            if source.palette {
                let named = palette_name(key, prefix);
                palette.insert(named.clone(), Value::from(hex.to_owned()));
                Value::from(named)
            } else {
                Value::from(hex.to_owned())
            }
        };
        let mut tokens = Map::new();
        for token in TOKENS {
            if let Some(hex) = look.syntax.get(token) {
                tokens.insert((*token).to_owned(), Value::from(hex.clone()));
                continue;
            }
            let Some((_, keys)) = table.iter().find(|(it, _)| it == token) else {
                continue;
            };
            let Some((key, hex)) = keys
                .iter()
                .find_map(|key| look.colour(key).map(|hex| (*key, hex)))
            else {
                continue;
            };
            let value = named(key, hex, &mut palette, &mut used);
            tokens.insert((*token).to_owned(), value);
        }
        let tag = look.tag.unwrap_or_else(|| {
            tag_of(
                table
                    .iter()
                    .find(|(it, _)| *it == "surface.base")
                    .and_then(|(_, keys)| keys.iter().find_map(|key| look.colour(key))),
            )
        });
        let tag_word = match tag {
            Appearance::Dark => "dark",
            Appearance::Light => "light",
        };
        let mut appearance = Map::new();
        appearance.insert(
            "id".to_owned(),
            Value::from(look.id.unwrap_or(tag_word).to_owned()),
        );
        appearance.insert("appearance".to_owned(), Value::from(tag_word));
        if look.high_contrast {
            appearance.insert("high-contrast".to_owned(), Value::Bool(true));
        }
        // The recipes measure this appearance alone, as the engine reads it.
        let mut alone = Map::new();
        alone.insert("$schema".to_owned(), Value::from(super::SCHEMA_URL));
        alone.insert("name".to_owned(), Value::from(name.to_owned()));
        alone.insert("palette".to_owned(), Value::Object(palette.clone()));
        alone.insert(
            "appearances".to_owned(),
            Value::Array(vec![Value::Object(appearance.clone())]),
        );
        tune(
            kind,
            look,
            &Value::Object(alone),
            &mut tokens,
            |key, hex| named(key, hex, &mut palette, &mut used),
        );
        for flag in FLAGS {
            if let Some(on) = look.flags.get(flag) {
                tokens.insert((*flag).to_owned(), Value::Bool(*on));
            }
        }
        appearance.insert("tokens".to_owned(), Value::Object(tokens));
        appearances.push(Value::Object(appearance));
    }
    let mut left_out = source.left_out;
    let mut seen = BTreeSet::new();
    for look in &source.looks {
        for (key, _) in &look.colours {
            if !used.contains(key) && seen.insert(key.clone()) {
                let listed = if source.under_colors {
                    format!("colors.{key}")
                } else {
                    key.clone()
                };
                // A key a row names lost to a nearer key of that row.
                let why = table
                    .iter()
                    .find(|(_, keys)| keys.contains(&key.as_str()))
                    .map_or_else(
                        || "no token takes it".to_owned(),
                        |(token, _)| format!("a nearer key gave {token} its value"),
                    );
                left_out.push(LeftOut::new(listed, why));
            }
        }
    }
    let mut document = Map::new();
    document.insert("$schema".to_owned(), Value::from(super::SCHEMA_URL));
    document.insert("name".to_owned(), Value::from(name.to_owned()));
    if !palette.is_empty() {
        document.insert("palette".to_owned(), Value::Object(palette));
    }
    document.insert("appearances".to_owned(), Value::Array(appearances));
    (Value::Object(document), left_out)
}

// ------------------------------------------------------------------------------------------
// VS Code
// ------------------------------------------------------------------------------------------

/// JSON with comments, as VS Code reads it: `//` and `/* */` comments and trailing commas. Also
/// how opencode's own config files are read (#3794, [`crate::opencode::OperatorRules`]).
pub(crate) fn jsonc(text: &str) -> Result<Value, String> {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut in_string = false;
    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            match c {
                '\\' => {
                    if let Some(next) = chars.next() {
                        out.push(next);
                    }
                }
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' => {
                in_string = true;
                out.push(c);
            }
            '/' if chars.peek() == Some(&'/') => {
                for next in chars.by_ref() {
                    if next == '\n' {
                        out.push('\n');
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut last = ' ';
                for next in chars.by_ref() {
                    if next == '\n' {
                        out.push('\n');
                    }
                    if last == '*' && next == '/' {
                        break;
                    }
                    last = next;
                }
                out.push(' ');
            }
            '}' | ']' => {
                // A comma with only space after it before the close is a trailing one.
                let kept = out.trim_end().len();
                if out[..kept].ends_with(',') {
                    out.replace_range(kept - 1..kept, " ");
                }
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    serde_json::from_str(&out)
        .map_err(|why| format!("a VS Code theme is JSON with comments: {why}"))
}

/// A VS Code colour theme.
fn vscode(text: &str) -> Result<Parsed, String> {
    let document = jsonc(text)?;
    let Some(fields) = document.as_object() else {
        return Err("a VS Code theme is a JSON object".to_owned());
    };
    let mut source = Parsed {
        under_colors: true,
        ..Parsed::default()
    };
    let mut look = Look::default();
    for (key, value) in fields {
        match key.as_str() {
            "name" => source.name = value.as_str().map(str::to_owned),
            "$schema" => {}
            "type" => match value.as_str() {
                Some("dark") => look.tag = Some(Appearance::Dark),
                Some("light") => look.tag = Some(Appearance::Light),
                Some("hc" | "hc-black" | "hcDark") => {
                    look.tag = Some(Appearance::Dark);
                    look.high_contrast = true;
                }
                Some("hcLight" | "hc-light") => {
                    look.tag = Some(Appearance::Light);
                    look.high_contrast = true;
                }
                _ => source.left_out.push(LeftOut::new(
                    "type",
                    "is not dark, light, hc or hcLight, so the background says the appearance",
                )),
            },
            "colors" => {
                let Some(colours) = value.as_object() else {
                    source
                        .left_out
                        .push(LeftOut::new("colors", "is not an object of colours"));
                    continue;
                };
                for (key, value) in colours {
                    match value.as_str().and_then(hex) {
                        Some(hex) => look.colours.push((key.clone(), hex)),
                        None => source
                            .left_out
                            .push(LeftOut::new(format!("colors.{key}"), "is not a hex colour")),
                    }
                }
            }
            "tokenColors" => token_colours(value, &mut look, &mut source.left_out),
            "include" => source.left_out.push(LeftOut::new(
                "include",
                "names another file, which import does not read: import that file too",
            )),
            other => source
                .left_out
                .push(LeftOut::new(other, "theme.json has no place for it")),
        }
    }
    source.looks.push(look);
    Ok(source)
}

/// A VS Code theme's `tokenColors`: the syntax tokens each rule's scopes take, the nearest
/// scope of a token's row winning and, between rules of one scope, the last (as TextMate
/// layers them). A rule nothing took is listed by its scopes.
fn token_colours(value: &Value, look: &mut Look, left_out: &mut Vec<LeftOut>) {
    let Some(rules) = value.as_array() else {
        let why = if value.is_string() {
            "names another file, which import does not read"
        } else {
            "is not a list of rules"
        };
        left_out.push(LeftOut::new("tokenColors", why));
        return;
    };
    if rules.len() > MOST_RULES {
        left_out.push(LeftOut::new(
            "tokenColors",
            format!("has more than {MOST_RULES} rules: the rest are not read"),
        ));
    }
    let rules: Vec<(Vec<String>, Option<String>, Option<String>)> = rules
        .iter()
        .take(MOST_RULES)
        .map(|rule| {
            let scopes = match rule.get("scope") {
                Some(Value::String(one)) => one.split(',').map(|it| it.trim().to_owned()).collect(),
                Some(Value::Array(many)) => many
                    .iter()
                    .filter_map(Value::as_str)
                    .flat_map(|it| it.split(','))
                    .map(|it| it.trim().to_owned())
                    .collect(),
                _ => Vec::new(),
            };
            let settings = rule.get("settings");
            let colour = settings
                .and_then(|it| it.get("foreground"))
                .and_then(Value::as_str)
                .and_then(hex);
            let style = settings
                .and_then(|it| it.get("fontStyle"))
                .and_then(Value::as_str)
                .map(str::to_owned);
            (scopes, colour, style)
        })
        .collect();
    let mut took = vec![false; rules.len()];
    for (token, wanted) in SCOPES {
        let found = wanted.iter().find_map(|want| {
            // A scope under `want` that a nearer row names (`keyword.operator` under
            // `keyword`) is that row's.
            let takes = |scope: &String| {
                under(scope, want)
                    && !SCOPES
                        .iter()
                        .flat_map(|(_, them)| them.iter())
                        .any(|deeper| {
                            deeper.len() > want.len() && under(deeper, want) && under(scope, deeper)
                        })
            };
            // The exact scope first, then one under it; the last rule of each.
            let exact = rules
                .iter()
                .enumerate()
                .rev()
                .find(|(_, (scopes, colour, _))| {
                    colour.is_some() && scopes.iter().any(|it| it == want)
                });
            exact.or_else(|| {
                rules
                    .iter()
                    .enumerate()
                    .rev()
                    .find(|(_, (scopes, colour, _))| {
                        colour.is_some() && scopes.iter().any(|it| !it.contains(' ') && takes(it))
                    })
            })
        });
        if let Some((at, (_, colour, style))) = found {
            took[at] = true;
            look.syntax
                .insert(token, colour.clone().unwrap_or_default());
            if let (Some(style), Some(role)) = (style, token.strip_prefix("syntax.")) {
                let words: Vec<&str> = style.split_whitespace().collect();
                for (flag, word) in [("italic", "italic"), ("bold", "bold")] {
                    let name = format!("syntax.{role}.{flag}");
                    if let Some(flag) = FLAGS.iter().copied().find(|it| *it == name) {
                        look.flags.insert(flag, words.contains(&word));
                    }
                }
            }
        }
    }
    for (at, (scopes, colour, _)) in rules.iter().enumerate() {
        if took[at] {
            continue;
        }
        let key = if scopes.is_empty() {
            format!("tokenColors[{at}]")
        } else {
            format!("tokenColors {}", scopes.join(", "))
        };
        let why = if colour.is_none() {
            "has no hex foreground"
        } else {
            "no token takes its scopes"
        };
        left_out.push(LeftOut::new(key, why));
    }
}

/// Whether the scope `scope` is `want` or under it (`keyword.control` is under `keyword`).
fn under(scope: &str, want: &str) -> bool {
    scope == want
        || scope
            .strip_prefix(want)
            .is_some_and(|rest| rest.starts_with('.'))
}

// ------------------------------------------------------------------------------------------
// base16 and base24
// ------------------------------------------------------------------------------------------

/// The keys of a scheme, YAML or JSON, flattened one level (`palette.base00`), each with its
/// text.
fn scheme_keys(text: &str) -> Result<Vec<(String, String)>, String> {
    if text.trim_start().starts_with('{') {
        let document: Value =
            serde_json::from_str(text).map_err(|why| format!("a scheme in JSON is JSON: {why}"))?;
        let Some(fields) = document.as_object() else {
            return Err("a scheme is an object".to_owned());
        };
        let mut keys = Vec::new();
        for (key, value) in fields {
            match value {
                Value::Object(inner) => {
                    for (inner_key, value) in inner {
                        keys.push((format!("{key}.{inner_key}"), scalar(value)));
                    }
                }
                other => keys.push((key.clone(), scalar(other))),
            }
        }
        return Ok(keys);
    }
    flat_yaml(text)
}

fn scalar(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

/// **The YAML a scheme is written in**, and no more: `key: value` lines, a value plain or
/// quoted, comments, a `---` start, and one level of nesting (a tinted scheme's `palette:`).
/// Anything else (a list, a block scalar, an anchor, a tag, a flow collection) is refused with
/// its line.
fn flat_yaml(text: &str) -> Result<Vec<(String, String)>, String> {
    let mut keys = Vec::new();
    let mut under: Option<String> = None;
    for (at, line) in text.lines().enumerate() {
        let number = at + 1;
        let trimmed = line.trim_end();
        let content = trimmed.trim_start();
        if content.is_empty() || content.starts_with('#') || (at == 0 && content == "---") {
            continue;
        }
        let indented = trimmed.len() != content.len();
        let refuse = || {
            format!(
                "line {number} is not a line of a base16 scheme's YAML: {}",
                crate::shown::short(content)
            )
        };
        let Some((key, rest)) = content.split_once(':') else {
            return Err(refuse());
        };
        let key = key.trim().trim_matches(['"', '\'']);
        if key.is_empty() || key.contains(char::is_whitespace) {
            return Err(refuse());
        }
        let value = yaml_value(rest.trim()).ok_or_else(refuse)?;
        match (indented, &under, value.is_empty()) {
            (false, _, true) => under = Some(key.to_owned()),
            (false, _, false) => {
                under = None;
                keys.push((key.to_owned(), value));
            }
            (true, Some(parent), false) => keys.push((format!("{parent}.{key}"), value)),
            (true, _, _) => return Err(refuse()),
        }
    }
    Ok(keys)
}

/// A YAML scalar as a scheme writes one: plain (to a ` #` comment), or in single or double
/// quotes. `None` for what a scheme never holds.
fn yaml_value(rest: &str) -> Option<String> {
    if rest.is_empty() || rest.starts_with('#') {
        return Some(String::new());
    }
    let first = rest.chars().next()?;
    if let Some(quote @ ('"' | '\'')) = Some(first) {
        let inner = &rest[1..];
        let end = inner.find(quote)?;
        let after = inner[end + 1..].trim();
        if !after.is_empty() && !after.starts_with('#') {
            return None;
        }
        return Some(inner[..end].to_owned());
    }
    if matches!(
        first,
        '[' | '{' | '|' | '>' | '&' | '*' | '!' | '-' | '@' | '`'
    ) {
        return None;
    }
    let plain = rest.split(" #").next().unwrap_or(rest).trim();
    Some(plain.to_owned())
}

/// The keys of a base16 and a base24 scheme, in order.
const BASE16_KEYS: &[&str] = &[
    "base00", "base01", "base02", "base03", "base04", "base05", "base06", "base07", "base08",
    "base09", "base0a", "base0b", "base0c", "base0d", "base0e", "base0f",
];

/// A base16 or base24 scheme: the classic form (`scheme:`, `base00: "181818"`) or the tinted
/// one (`system:`, `name:`, `variant:`, `palette:`).
fn scheme(text: &str, kind: SourceKind) -> Result<Parsed, String> {
    let keys = scheme_keys(text)?;
    let mut source = Parsed {
        palette: true,
        ..Parsed::default()
    };
    let mut look = Look::default();
    for (key, value) in keys {
        let colour_key = key
            .strip_prefix("palette.")
            .unwrap_or(&key)
            .to_ascii_lowercase();
        let is_colour_key = BASE16_KEYS.contains(&colour_key.as_str())
            || (kind == SourceKind::Base24 && BASE24_ONLY.contains(&colour_key.as_str()));
        if is_colour_key {
            match hex(&value) {
                Some(hex) => look.colours.push((colour_key, hex)),
                None => source
                    .left_out
                    .push(LeftOut::new(key, "is not a hex colour")),
            }
            continue;
        }
        match key.as_str() {
            "scheme" | "name" => source.name = Some(value),
            "system" => {
                if value != kind.name() {
                    source.left_out.push(LeftOut::new(
                        key,
                        format!("says {value}, and it was read as {}", kind.name()),
                    ));
                }
            }
            "variant" => match value.as_str() {
                "dark" => look.tag = Some(Appearance::Dark),
                "light" => look.tag = Some(Appearance::Light),
                _ => source.left_out.push(LeftOut::new(
                    key,
                    "is not dark or light, so the background says",
                )),
            },
            _ if BASE24_ONLY.contains(&colour_key.as_str()) => source.left_out.push(LeftOut::new(
                key,
                "is a base24 colour, and this was read as base16",
            )),
            "author" | "slug" | "description" => source
                .left_out
                .push(LeftOut::new(key, "theme.json has no place for it")),
            _ => source
                .left_out
                .push(LeftOut::new(key, "is not a key of a scheme")),
        }
    }
    if look.colours.is_empty() {
        return Err(format!("a {} scheme has base00 to base0F", kind.name()));
    }
    source.looks.push(look);
    Ok(source)
}

// ------------------------------------------------------------------------------------------
// iTerm2 and Ghostty
// ------------------------------------------------------------------------------------------

/// The suffixes iTerm2 gives a colour kept for one appearance, and the appearance.
const ITERM_VARIANTS: &[(&str, &str, Appearance)] = &[
    (" (Dark)", "dark", Appearance::Dark),
    (" (Light)", "light", Appearance::Light),
];

/// An iTerm2 colour preset: a property list of colours, each a dictionary of components. A
/// preset that keeps colours for dark and light mode (`Ansi 0 Color (Dark)`) has an appearance
/// for each, its plain colours under both.
fn iterm(text: &str) -> Result<Parsed, String> {
    if plist_depth(text.as_bytes(), MOST_PLIST_DEPTH).is_err() {
        return Err(format!(
            "an iTerm2 colour preset nests dictionaries and lists {MOST_PLIST_DEPTH} deep at most, \
             and this one nests deeper"
        ));
    }
    let value = plist::Value::from_reader_xml(text.as_bytes())
        .map_err(|why| format!("an iTerm2 colour preset is a property list: {why}"))?;
    let Some(fields) = value.as_dictionary() else {
        return Err("an iTerm2 colour preset is a dictionary of colours".to_owned());
    };
    let mut source = Parsed {
        palette: true,
        ..Parsed::default()
    };
    let mut plain = Vec::new();
    let mut variants: Vec<Vec<(String, String)>> = vec![Vec::new(); ITERM_VARIANTS.len()];
    for (key, value) in fields {
        let Some(colour) = iterm_colour(value) else {
            source
                .left_out
                .push(LeftOut::new(key.clone(), "is not a colour"));
            continue;
        };
        match ITERM_VARIANTS
            .iter()
            .position(|(suffix, ..)| key.ends_with(suffix))
        {
            Some(at) => {
                let bare = key.strip_suffix(ITERM_VARIANTS[at].0).unwrap_or(key);
                variants[at].push((bare.to_owned(), colour));
            }
            None => plain.push((key.clone(), colour)),
        }
    }
    if variants.iter().all(Vec::is_empty) {
        source.looks.push(Look {
            colours: plain,
            ..Look::default()
        });
    } else {
        for (at, (_, id, tag)) in ITERM_VARIANTS.iter().enumerate() {
            if variants[at].is_empty() {
                continue;
            }
            let mut colours = plain.clone();
            colours.extend(variants[at].iter().cloned());
            source.looks.push(Look {
                id: Some(id),
                tag: Some(*tag),
                colours,
                ..Look::default()
            });
        }
    }
    Ok(source)
}

/// How deep a property list may nest its dictionaries and lists: an iTerm2 preset nests two (a
/// dictionary of colours, each a dictionary of components).
const MOST_PLIST_DEPTH: usize = 8;

/// How deep `xml` nests its dictionaries and lists, counted on the property list reader's own
/// events before anything builds a value from them: `Err` with the depth once it is past `most`,
/// where the count stops. A reader that stops at an error ends the count, and the parse says it.
fn plist_depth(xml: &[u8], most: usize) -> Result<usize, usize> {
    use plist::stream::Event;
    let (mut depth, mut deepest) = (0usize, 0usize);
    for event in plist::stream::XmlReader::new(xml) {
        match event {
            Ok(Event::StartArray(_) | Event::StartDictionary(_)) => {
                depth += 1;
                deepest = deepest.max(depth);
                if depth > most {
                    return Err(depth);
                }
            }
            Ok(Event::EndCollection) => depth = depth.saturating_sub(1),
            Ok(_) => {}
            Err(_) => break,
        }
    }
    Ok(deepest)
}

/// One iTerm2 colour, its components from 0 to 1, as hex: with its alpha when it is not 1.
fn iterm_colour(value: &plist::Value) -> Option<String> {
    let fields = value.as_dictionary()?;
    let component = |name: &str| -> Option<u8> {
        let number = match fields.get(name)? {
            plist::Value::Real(it) => *it,
            plist::Value::Integer(it) => it.as_signed()? as f64,
            _ => return None,
        };
        number
            .is_finite()
            .then(|| (number.clamp(0.0, 1.0) * 255.0).round() as u8)
    };
    let (red, green, blue) = (
        component("Red Component")?,
        component("Green Component")?,
        component("Blue Component")?,
    );
    let alpha = match fields.get("Alpha Component") {
        Some(_) => component("Alpha Component")?,
        None => 255,
    };
    Some(if alpha == 255 {
        format!("#{red:02x}{green:02x}{blue:02x}")
    } else {
        format!("#{red:02x}{green:02x}{blue:02x}{alpha:02x}")
    })
}

/// The keys of a Ghostty theme that are colours, besides its palette.
const GHOSTTY_COLOURS: &[&str] = &[
    "background",
    "foreground",
    "cursor-color",
    "cursor-text",
    "selection-background",
    "selection-foreground",
];

/// A Ghostty theme: `key = value` lines, `#` comments, `palette = N=#rrggbb`.
fn ghostty(text: &str) -> Result<Parsed, String> {
    let mut source = Parsed {
        palette: true,
        ..Parsed::default()
    };
    let mut look = Look::default();
    for (at, line) in text.lines().enumerate() {
        let content = line.trim();
        if content.is_empty() || content.starts_with('#') {
            continue;
        }
        let Some((key, value)) = content.split_once('=') else {
            return Err(format!(
                "line {} is not a line of a Ghostty theme: {}",
                at + 1,
                crate::shown::short(content)
            ));
        };
        let (key, value) = (key.trim(), value.trim().trim_matches('"'));
        if key == "palette" {
            let Some((index, colour)) = value.split_once('=') else {
                source
                    .left_out
                    .push(LeftOut::new(format!("palette {value}"), "is not N=#rrggbb"));
                continue;
            };
            let named = format!("palette {}", index.trim());
            match (index.trim().parse::<u8>(), hex(colour)) {
                (Ok(_), Some(hex)) => look.colours.push((named, hex)),
                _ => source
                    .left_out
                    .push(LeftOut::new(named, "is not a hex colour")),
            }
            continue;
        }
        if GHOSTTY_COLOURS.contains(&key) {
            match hex(value) {
                Some(hex) => look.colours.push((key.to_owned(), hex)),
                None => source
                    .left_out
                    .push(LeftOut::new(key, "is not a hex colour")),
            }
            continue;
        }
        source
            .left_out
            .push(LeftOut::new(key, "is not a colour a theme takes"));
    }
    if look.colours.is_empty() {
        return Err("a Ghostty theme has a palette, a background or a foreground".to_owned());
    }
    source.looks.push(look);
    Ok(source)
}
