// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A theme's fonts** (spec #2313, #2326, ADR 0097 §2–3): the UI, mono and terminal families,
//! each from woff2 files in the theme's own folder.
//!
//! ```json
//! "fonts": {
//!   "ui": { "family": "IBM Plex Sans", "faces": [
//!     { "file": "fonts/IBMPlexSans-Regular.woff2", "weight": 400 },
//!     { "file": "fonts/IBMPlexSans-SemiBold.woff2", "weight": 600 }
//!   ] },
//!   "terminal": { "family": "JetBrains Mono", "faces": [
//!     { "file": "fonts/JetBrainsMono-Regular.woff2", "weight": 400 }
//!   ] }
//! }
//! ```
//!
//! **The grammar is closed.** A face's file is `fonts/<name>.woff2`, one plain name in the
//! folder's `fonts/`: no other folder, no `..`, no link on the way, no URL. The file is read by
//! the engine when the theme is, under a size cap, and must be a woff2 file by its signature.
//! A theme that is not read from a folder (an extension's text, a test's document) has no
//! `fonts/` to name, so naming a font refuses it.
//!
//! **purlis serves a font to the window itself, by its content**. The window is handed, for each
//! face, the sha-256 of the bytes the engine read ([`Face::asset`]), never a path or an
//! address; it asks for those bytes by that digest ([`served`]) and makes the face from them
//! (`FontFace` over an `ArrayBuffer`), so nothing is fetched from a URL and no scheme reaches a
//! file. [`served`] answers only a digest of a theme folder the engine read and accepted,
//! reads that file again under the same containment and cap, and answers its bytes only when they
//! still have that digest.
//!
//! **A face is read through the theme folder's own handle** (#2555): `fonts/` is opened from
//! the handle of the folder its `theme.json` was read through, refusing a link, and each face
//! from the handle of `fonts/`, refusing a link again. [`served`] reads it again from the same
//! handles. Nothing on the way is looked up by path after `theme.json` was read.
//!
//! **A project's and a workspace's fonts need the project opened** (ADR 0035): a level's
//! folders say whether their fonts are read (`super::folders`), and only a project the window
//! opened through the trust gate reads them.
//!
//! **The CSS family name is purlis's**: a face is registered under a name the engine makes from
//! its digest ([`Font::stack`]); the theme's `family` is a label, said in Settings, and never
//! reaches a declaration.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use serde_json::{Map, Value};

use super::stringify;

/// The three fonts a theme may set.
pub const SLOTS: &[&str] = &["ui", "mono", "terminal"];

/// The largest font file purlis reads.
pub const MOST_FONT_BYTES: u64 = 2 * 1024 * 1024;

/// The most faces one font may have.
pub const MOST_FACES: usize = 8;

/// The longest a family's label may be.
const MOST_FAMILY: usize = 64;

/// The first four bytes of every woff2 file.
const WOFF2: &[u8; 4] = b"wOF2";

/// Core's family for each slot, Purlis Classic's: what every theme falls back to, and what a
/// face of the theme's comes in front of.
pub const CLASSIC_UI: &str = "ui-sans-serif, Inter, Avenir, Helvetica, Arial, sans-serif";
pub const CLASSIC_MONO: &str = "ui-monospace, Menlo, Consolas, monospace";
pub const CLASSIC_TERMINAL: &str = "ui-monospace, SFMono-Regular, Menlo, monospace";

/// Whether a face is upright or italic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Style {
    Normal,
    Italic,
}

/// **One face of a theme's font**: the digest the window asks for its bytes by, its weight and
/// its style.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Face {
    /// The sha-256 of the file's bytes, as purlis read them.
    pub asset: String,
    pub weight: u16,
    pub style: Style,
    /// Where it was read from: registered for [`served`] once the theme is accepted.
    #[serde(skip)]
    pub(crate) from: Option<Kept>,
}

/// **One of a theme's fonts, complete**: the family the window draws, and the faces it loads.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Font {
    /// The theme's label for it, or `None` for core's.
    pub family: Option<String>,
    /// The CSS family list the window draws with, written by the engine: the face's own name,
    /// when the theme brings one, in front of core's family for the slot.
    pub stack: String,
    /// The faces to load, none for core's.
    pub faces: Vec<Face>,
}

impl Font {
    fn core(stack: &str) -> Self {
        Self {
            family: None,
            stack: stack.to_owned(),
            faces: Vec::new(),
        }
    }
}

/// **A theme's three fonts.**
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Fonts {
    pub ui: Font,
    pub mono: Font,
    pub terminal: Font,
}

impl Fonts {
    /// Core's, Purlis Classic's: the platform's own families, no face to load.
    pub fn classic() -> Self {
        Self {
            ui: Font::core(CLASSIC_UI),
            mono: Font::core(CLASSIC_MONO),
            terminal: Font::core(CLASSIC_TERMINAL),
        }
    }

    fn slot_mut(&mut self, slot: &str) -> &mut Font {
        match slot {
            "ui" => &mut self.ui,
            "mono" => &mut self.mono,
            _ => &mut self.terminal,
        }
    }
}

/// Core's family for `slot`.
pub fn core_stack(slot: &str) -> &'static str {
    match slot {
        "ui" => CLASSIC_UI,
        "mono" => CLASSIC_MONO,
        _ => CLASSIC_TERMINAL,
    }
}

/// Whether `name` is a family a person or a theme may name: letters, digits, spaces, `-`, `_`
/// and `.`, starting with a letter or a digit, at most 64 long. Nothing that could end a name,
/// a list or a declaration.
pub fn is_family(name: &str) -> bool {
    let mut chars = name.chars();
    name.chars().count() <= MOST_FAMILY
        && chars.next().is_some_and(char::is_alphanumeric)
        && chars.all(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.'))
        && !name.ends_with(' ')
}

/// The grammar of a family, as a sentence.
pub const FAMILY_RULE: &str = "is not letters, digits, spaces, dashes, underscores and dots \
     starting with a letter or a digit, at most 64 long";

/// A file a face names, `fonts/<name>.woff2`: the name, or `None`.
fn face_file(file: &str) -> Option<&str> {
    let name = file.strip_prefix("fonts/")?;
    let stem = name.strip_suffix(".woff2")?;
    let plain = !stem.is_empty()
        && name.len() <= 100
        && !name.starts_with('.')
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'));
    plain.then_some(name)
}

/// **A theme folder, held**: where it is, for a sentence, and the handle its fonts are opened
/// from, or why its fonts are not read.
#[derive(Debug, Clone)]
pub(crate) struct ThemeDir {
    path: PathBuf,
    handle: Result<held::Handle, String>,
}

impl ThemeDir {
    /// The folder at `path`, held by `handle`: the one its `theme.json` was read through.
    #[cfg(unix)]
    pub(crate) fn held(path: PathBuf, handle: std::os::fd::OwnedFd) -> Self {
        Self {
            path,
            handle: Ok(std::sync::Arc::new(handle)),
        }
    }

    /// The folder at `path`, whose fonts are not read, and `why`.
    pub(crate) fn refusing(path: PathBuf, why: String) -> Self {
        Self {
            path,
            handle: Err(why),
        }
    }

    /// The folder `name` inside `root`, or `root` itself, opened now: `root` is the caller's to
    /// trust, and `name` is opened from it refusing a link. For a theme whose `theme.json` is
    /// read from that folder by its path: the person's own, and one named on the command line.
    pub(crate) fn open(root: &Path, name: Option<&str>) -> Self {
        let path = name.map_or_else(|| root.to_path_buf(), |name| root.join(name));
        let handle = held::open(root, name).map_err(|why| {
            format!(
                "purlis could not open {} to read its fonts: {why}",
                path.display()
            )
        });
        Self { path, handle }
    }

    /// The folder, for a sentence.
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

/// Why a folder or a file a face is read from was not opened.
#[derive(Debug, Clone)]
enum Missed {
    /// It is not there.
    Gone,
    /// It is a link, or something that is not what it should be.
    Linked,
    /// Anything else, said.
    Failed(String),
}

/// One file the engine read for a face, kept so it can be served by its digest: the handle of
/// the `fonts/` it was opened from, and its name there.
#[derive(Debug, Clone)]
pub(crate) struct Kept {
    fonts: held::Handle,
    name: String,
    /// Where it is, for a sentence.
    file: PathBuf,
}

impl PartialEq for Kept {
    fn eq(&self, other: &Self) -> bool {
        self.file == other.file && self.name == other.name
    }
}

impl Eq for Kept {}

/// Every font file of an accepted theme the engine read, by its digest: each place it was read
/// from, since two themes may bring the same face.
///
/// It is kept for the process: a theme that is no longer in force leaves its faces here. That
/// serves nothing new: each is a file of a theme folder purlis accepted, answered only while its
/// bytes still have the digest, and the window asks only for the faces of the theme it draws.
fn kept() -> std::sync::MutexGuard<'static, BTreeMap<String, Vec<Kept>>> {
    static KEPT: OnceLock<Mutex<BTreeMap<String, Vec<Kept>>>> = OnceLock::new();
    KEPT.get_or_init(|| Mutex::new(BTreeMap::new()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn hex(bytes: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// What a missed open of `shown` says.
fn missed(shown: &str, why: Missed) -> String {
    match why {
        Missed::Gone => format!("{shown} is not in the theme's folder"),
        Missed::Linked => format!(
            "{shown} is reached through a link or something that is not a plain file, so it is \
             not read"
        ),
        Missed::Failed(why) => format!("purlis could not read {shown}: {why}"),
    }
}

/// The face `name` in the folder `fonts` holds, opened from that handle refusing a link, read
/// whole under the cap, and a woff2 file by its signature: its bytes, or why not.
fn read_face(fonts: &held::Handle, name: &str, shown: &str) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let mut open = held::face(fonts, name).map_err(|why| missed(shown, why))?;
    let found = open
        .metadata()
        .map_err(|why| format!("purlis could not read {shown}: {why}"))?;
    if !found.file_type().is_file() {
        return Err(format!("{shown} is not a plain file"));
    }
    let too_big = |bytes: u64| {
        format!("{shown} is {bytes} bytes, and a font is never larger than {MOST_FONT_BYTES}")
    };
    if found.len() > MOST_FONT_BYTES {
        return Err(too_big(found.len()));
    }
    let mut bytes = Vec::new();
    (&mut open)
        .take(MOST_FONT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|why| format!("purlis could not read {shown}: {why}"))?;
    if bytes.len() as u64 > MOST_FONT_BYTES {
        return Err(too_big(bytes.len() as u64));
    }
    if !bytes.starts_with(WOFF2) {
        return Err(format!("{shown} is not a woff2 file"));
    }
    Ok(bytes)
}

/// **The bytes of the font file whose digest is `asset`**, for the window to make the face
/// from. Only a file of a theme folder the engine accepted is served, read again under
/// the same containment and cap, and only while its bytes still have that digest.
pub fn served(asset: &str) -> Result<Vec<u8>, String> {
    let Some(places) = kept().get(asset).cloned() else {
        return Err("that is not a font of a theme purlis read".to_owned());
    };
    let mut why = String::new();
    for one in places {
        let shown = one.file.display().to_string();
        match read_face(&one.fonts, &one.name, &shown) {
            Ok(bytes) if hex(&bytes) == asset => return Ok(bytes),
            Ok(_) => why = format!("{shown} changed since purlis read the theme"),
            Err(refused) => why = refused,
        }
    }
    Err(why)
}

/// **Registers the faces of an accepted theme's fonts** for [`served`]. A theme that is refused
/// registers nothing, so none of its files is served.
///
/// A file is kept once, under the digest it was last read with and the handles it was last read
/// through: a file read again with other bytes is no longer kept as what it was.
pub(super) fn register(fonts: &Fonts) {
    let mut kept = kept();
    for font in [&fonts.ui, &fonts.mono, &fonts.terminal] {
        for face in &font.faces {
            let Some(place) = &face.from else { continue };
            kept.retain(|asset, places| {
                if *asset != face.asset {
                    places.retain(|one| one != place);
                }
                !places.is_empty()
            });
            let places = kept.entry(face.asset.clone()).or_default();
            match places.iter_mut().find(|one| *one == place) {
                Some(one) => *one = place.clone(),
                None => places.push(place.clone()),
            }
        }
    }
}

/// The name purlis registers a font's faces under: the slot and the digest of its faces, so two
/// themes' fonts never share a name.
fn face_name(slot: &str, faces: &[Face]) -> String {
    let joined: Vec<&str> = faces.iter().map(|one| one.asset.as_str()).collect();
    let digest = hex(joined.join(",").as_bytes());
    format!("purlis-{slot}-{}", &digest[..12])
}

/// **Reads a theme's `fonts`** over `base` (its parent's, or core's), from `folder`, the theme's
/// own folder when it is read from one. Says in `complaints` what refuses the theme.
pub(super) fn read(
    raw: Option<&Value>,
    base: &Fonts,
    folder: Option<&ThemeDir>,
    complaints: &mut Vec<String>,
) -> Fonts {
    let mut fonts = base.clone();
    let Some(raw) = raw else {
        return fonts;
    };
    let Some(given) = raw.as_object() else {
        complaints.push("fonts is an object of ui, mono and terminal".to_owned());
        return fonts;
    };
    let Some(folder) = folder else {
        complaints.push(
            "fonts are read from a theme folder's fonts/, and this theme is not read from a \
             folder"
                .to_owned(),
        );
        return fonts;
    };
    if given.is_empty() {
        return fonts;
    }
    let dir = match &folder.handle {
        Ok(dir) => dir,
        Err(why) => {
            complaints.push(why.clone());
            return fonts;
        }
    };
    // `fonts/`, from the handle the folder's `theme.json` was read through, once for every face.
    let inside = Inside {
        fonts: held::fonts(dir),
        folder: folder.path(),
    };
    for (slot, font) in given {
        if !SLOTS.contains(&slot.as_str()) {
            complaints.push(format!(
                "{slot} is not a font of a theme: ui, mono or terminal"
            ));
            continue;
        }
        if let Some(read) = font_of(slot, font, &inside, complaints) {
            *fonts.slot_mut(slot) = read;
        }
    }
    fonts
}

/// A theme folder's `fonts/`, opened from the folder's handle, or why not; and the folder, for a
/// sentence.
struct Inside<'a> {
    fonts: Result<held::Handle, Missed>,
    folder: &'a Path,
}

/// One slot's font, or `None` (and why).
fn font_of(
    slot: &str,
    raw: &Value,
    folder: &Inside<'_>,
    complaints: &mut Vec<String>,
) -> Option<Font> {
    let before = complaints.len();
    let empty = Map::new();
    let given = raw.as_object().unwrap_or_else(|| {
        complaints.push(format!(
            "fonts.{slot} is an object with a family and its faces"
        ));
        &empty
    });
    for key in given.keys() {
        if key != "family" && key != "faces" {
            complaints.push(format!("{key} is not a key of fonts.{slot}"));
        }
    }
    let family = match given.get("family") {
        Some(Value::String(name)) if is_family(name) => Some(name.clone()),
        Some(other) => {
            complaints.push(format!(
                "fonts.{slot}.family {} {FAMILY_RULE}",
                stringify(other)
            ));
            None
        }
        None => {
            if raw.is_object() {
                complaints.push(format!("fonts.{slot} has no family"));
            }
            None
        }
    };
    let listed: &[Value] = match given.get("faces") {
        Some(Value::Array(faces)) if !faces.is_empty() && faces.len() <= MOST_FACES => faces,
        Some(Value::Array(faces)) if faces.len() > MOST_FACES => {
            complaints.push(format!(
                "fonts.{slot} has {} faces, and a font has at most {MOST_FACES}",
                faces.len()
            ));
            &[]
        }
        _ => {
            if raw.is_object() {
                complaints.push(format!(
                    "fonts.{slot}.faces is a list of one to {MOST_FACES} woff2 files in the \
                     folder's fonts/"
                ));
            }
            &[]
        }
    };
    let mut faces = Vec::new();
    for one in listed {
        if let Some(face) = face_of(slot, one, folder, complaints) {
            faces.push(face);
        }
    }
    if complaints.len() != before {
        return None;
    }
    let name = face_name(slot, &faces);
    Some(Font {
        family,
        stack: format!("\"{name}\", {}", core_stack(slot)),
        faces,
    })
}

/// One face, read and registered, or `None` (and why).
fn face_of(
    slot: &str,
    raw: &Value,
    folder: &Inside<'_>,
    complaints: &mut Vec<String>,
) -> Option<Face> {
    let Some(given) = raw.as_object() else {
        complaints.push(format!("a face of fonts.{slot} is an object with its file"));
        return None;
    };
    let before = complaints.len();
    for key in given.keys() {
        if !matches!(key.as_str(), "file" | "weight" | "style") {
            complaints.push(format!("{key} is not a key of a face of fonts.{slot}"));
        }
    }
    let file = match given.get("file") {
        Some(Value::String(file)) => match face_file(file) {
            Some(name) => Some(name.to_owned()),
            None => {
                complaints.push(format!(
                    "a face of fonts.{slot} names {}, which is not fonts/<name>.woff2 inside \
                     the theme's folder",
                    stringify(&Value::String(file.clone()))
                ));
                None
            }
        },
        other => {
            complaints.push(format!(
                "a face of fonts.{slot} names its file as fonts/<name>.woff2, not {}",
                other.map_or_else(|| "nothing".to_owned(), stringify)
            ));
            None
        }
    };
    let weight = match given.get("weight") {
        None => Some(400),
        Some(value) => match value.as_u64().filter(|w| (100..=900).contains(w)) {
            Some(weight) => u16::try_from(weight).ok(),
            None => {
                complaints.push(format!(
                    "a face's weight in fonts.{slot} is {}, which is not a whole number from 100 \
                     to 900",
                    stringify(value)
                ));
                None
            }
        },
    };
    let style = match given.get("style") {
        None => Some(Style::Normal),
        Some(Value::String(style)) if style == "normal" => Some(Style::Normal),
        Some(Value::String(style)) if style == "italic" => Some(Style::Italic),
        Some(other) => {
            complaints.push(format!(
                "a face's style in fonts.{slot} is {}, which is \"normal\" or \"italic\"",
                stringify(other)
            ));
            None
        }
    };
    let (Some(file), Some(weight), Some(style)) = (file, weight, style) else {
        return None;
    };
    if complaints.len() != before {
        return None;
    }
    let shown = format!("fonts/{file}");
    let fonts = match &folder.fonts {
        Ok(fonts) => fonts,
        Err(why) => {
            complaints.push(missed(&shown, why.clone()));
            return None;
        }
    };
    match read_face(fonts, &file, &shown) {
        Ok(bytes) => {
            let asset = hex(&bytes);
            Some(Face {
                asset,
                weight,
                style,
                from: Some(Kept {
                    fonts: fonts.clone(),
                    file: folder.folder.join("fonts").join(&file),
                    name: file,
                }),
            })
        }
        Err(why) => {
            complaints.push(why);
            None
        }
    }
}

#[cfg(unix)]
mod held {
    //! The opens, from handles: each step opened with `O_NOFOLLOW`, the file read from the
    //! descriptor that was opened.

    use std::os::fd::OwnedFd;
    use std::path::Path;
    use std::sync::Arc;

    use rustix::fs::{Mode, OFlags, openat};
    use rustix::io::Errno;

    use super::Missed;

    /// A folder, held.
    pub(crate) type Handle = Arc<OwnedFd>;

    const DIRECTORY: OFlags = OFlags::RDONLY
        .union(OFlags::DIRECTORY)
        .union(OFlags::NOFOLLOW)
        .union(OFlags::CLOEXEC);

    fn missed(why: Errno) -> Missed {
        match why {
            Errno::NOENT => Missed::Gone,
            Errno::LOOP | Errno::NOTDIR | Errno::MLINK => Missed::Linked,
            why => Missed::Failed(why.to_string()),
        }
    }

    /// `root`, opened as the caller trusts it, and then `name` inside it refusing a link.
    pub(super) fn open(root: &Path, name: Option<&str>) -> rustix::io::Result<Handle> {
        let at = rustix::fs::open(
            root,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )?;
        let at = match name {
            Some(name) => openat(&at, name, DIRECTORY, Mode::empty())?,
            None => at,
        };
        Ok(Arc::new(at))
    }

    /// The folder's `fonts/`, opened from its handle refusing a link.
    pub(super) fn fonts(dir: &Handle) -> Result<Handle, Missed> {
        openat(dir, "fonts", DIRECTORY, Mode::empty())
            .map(Arc::new)
            .map_err(missed)
    }

    /// The face `name` in `fonts`, opened from its handle refusing a link.
    pub(super) fn face(fonts: &Handle, name: &str) -> Result<std::fs::File, Missed> {
        openat(
            fonts,
            name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map(std::fs::File::from)
        .map_err(missed)
    }
}

#[cfg(not(unix))]
mod held {
    //! **No font is read off unix**: a face is opened from its folder's handle refusing a link,
    //! which this build cannot express, and a guard that cannot be expressed refuses rather
    //! than degrades (ADR 0031), as the theme folders' reads do.

    use std::path::Path;

    use super::Missed;

    /// No folder is held here.
    pub(crate) type Handle = std::convert::Infallible;

    pub(super) fn open(_root: &Path, _name: Option<&str>) -> Result<Handle, String> {
        Err("fonts are read from theme folders on macOS and Linux".to_owned())
    }

    pub(super) fn fonts(dir: &Handle) -> Result<Handle, Missed> {
        match *dir {}
    }

    pub(super) fn face(fonts: &Handle, _name: &str) -> Result<std::fs::File, Missed> {
        match *fonts {}
    }
}
