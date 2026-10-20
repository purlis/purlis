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
//! **A project's and a workspace's fonts are read only as approved** (ADR 0035, ADR 0098 as
//! amended): each font file a project's or a workspace's theme folder brings is part of what
//! the project contributes ([`crate::machine::Contribution::fonts`]), named by its path in the
//! project and its sha-256, so the trust ask lists it. A face from such a folder is read only
//! when this machine's approval of the project holds that file with those bytes' digest; any
//! other refuses the theme. The person's own folders' faces need no approval.
//!
//! **A project's font file is hashed once for each read** (#2696, [`Measured`]): the trust
//! check's listing of a project's fonts hands its digests to the theme read made with it, and
//! each frame a window's creation resolves hands them to the next, for a file that is the same
//! file, unchanged and settled. The digest is still checked against the approval, and a face is
//! still served only once its bytes, read again, have it.
//!
//! **The CSS family name is purlis's**: a face is registered under a name the engine makes from
//! its digest ([`Font::stack`]); the theme's `family` is a label, said in Settings, and never
//! reaches a declaration.

use std::collections::{BTreeMap, HashMap};
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
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

/// The weight the window draws upright text in where it asks for `normal`: code, and the
/// terminal's text.
pub const NORMAL: u16 = 400;

/// The weight the window draws bold text in where it asks for `bold`: a syntax role drawn bold,
/// and the terminal's bold text (xterm's own bold).
pub const BOLD: u16 = 700;

/// The geometry tokens that say the weights the interface's text is drawn in.
const UI_WEIGHTS: &[&str] = &[
    "font-weight.regular",
    "font-weight.medium",
    "font-weight.strong",
];

/// **One style the window draws a theme's font in**: the font's slot, the weight and whether it
/// is italic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct StyleDrawn {
    /// `ui`, `mono` or `terminal`.
    pub slot: &'static str,
    pub weight: u16,
    pub style: Style,
}

/// **Every style `theme` draws text in** (#2709), in order: the interface at each of its weights,
/// upright and italic (emphasis); code upright, and each syntax role at its own italic and bold;
/// the terminal upright, bold and italic.
pub fn styles_drawn(theme: &super::ResolvedTheme) -> Vec<StyleDrawn> {
    let mut drawn = std::collections::BTreeSet::new();
    let mut add = |slot, weight, style| {
        drawn.insert(StyleDrawn {
            slot,
            weight,
            style,
        });
    };
    for geometry in [&theme.geometry, &theme.compact] {
        for token in UI_WEIGHTS {
            let Some(weight) = geometry.get(token) else {
                continue;
            };
            // A weight token is a whole number within its bounds (`geometry::STEPS`).
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let weight = *weight as u16;
            add("ui", weight, Style::Normal);
            add("ui", weight, Style::Italic);
        }
    }
    add("mono", NORMAL, Style::Normal);
    for token in super::vocabulary::FLAGS {
        let Some(role) = token.strip_suffix(".italic") else {
            continue;
        };
        let bold = theme.typed.flag(&format!("{role}.bold"));
        let italic = theme.typed.flag(token);
        add(
            "mono",
            if bold { BOLD } else { NORMAL },
            if italic { Style::Italic } else { Style::Normal },
        );
    }
    add("terminal", NORMAL, Style::Normal);
    add("terminal", BOLD, Style::Normal);
    add("terminal", NORMAL, Style::Italic);
    drawn.into_iter().collect()
}

/// **Each style `theme` draws that its font has no face for** (#2709): the browser would make
/// it from another face, slanting or thickening it. A font with no face is the platform's
/// own, which brings its styles, so only a font the theme brings is asked.
pub fn without_a_face(theme: &super::ResolvedTheme) -> Vec<StyleDrawn> {
    let font = |slot: &str| match slot {
        "ui" => &theme.fonts.ui,
        "mono" => &theme.fonts.mono,
        _ => &theme.fonts.terminal,
    };
    styles_drawn(theme)
        .into_iter()
        .filter(|one| {
            let faces = &font(one.slot).faces;
            !faces.is_empty()
                && !faces
                    .iter()
                    .any(|face| face.weight == one.weight && face.style == one.style)
        })
        .collect()
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

/// **Where a theme folder's font files are read from**: the folder, held by the handle its
/// `theme.json` was read through, or the files of a theme built into purlis, compiled in with it
/// (#2336). A built-in theme's faces are purlis's own: they are read and served as any folder's
/// are, under the same cap and signature, and never from a path.
#[derive(Clone)]
pub(crate) enum Store {
    /// A folder on disk, held.
    Held(held::Handle),
    /// A built-in theme's files, each by its name in `fonts/`.
    BuiltIn(&'static [(&'static str, &'static [u8])]),
}

impl std::fmt::Debug for Store {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Held(_) => f.write_str("Store::Held"),
            Self::BuiltIn(files) => f
                .debug_list()
                .entries(files.iter().map(|(name, _)| name))
                .finish(),
        }
    }
}

impl Store {
    /// The folder's `fonts/`: opened from a held folder's handle refusing a link; a built-in
    /// theme's files are its `fonts/` already.
    fn fonts(&self) -> Result<Self, Missed> {
        match self {
            Self::Held(dir) => held::fonts(dir).map(Self::Held),
            Self::BuiltIn(files) => Ok(Self::BuiltIn(files)),
        }
    }

    /// How many face names in this `fonts/` sort before `name`, as a project's contribution
    /// lists them: only a project's or a workspace's folder, which is held, is ever asked.
    fn names_before(&self, name: &str) -> usize {
        match self {
            Self::Held(fonts) => held::names_before(fonts, name),
            Self::BuiltIn(_) => 0,
        }
    }
}

/// **A theme folder, held**: where it is, for a sentence, and the handle its fonts are opened
/// from, or why its fonts are not read.
#[derive(Debug, Clone)]
pub(crate) struct ThemeDir {
    path: PathBuf,
    handle: Result<Store, String>,
    /// For a project's or a workspace's folder: the font files the person approved with the
    /// project, path to digest, and this folder's path in the project. `None` for the person's
    /// own folders, whose faces need no approval.
    approved: Option<Approved>,
}

/// The font files a project's approval holds, and a theme folder's path in that project.
#[derive(Debug, Clone)]
struct Approved {
    files: std::sync::Arc<BTreeMap<String, String>>,
    folder: String,
    /// What the read this folder is part of has hashed already.
    measured: Measured,
}

impl ThemeDir {
    /// The folder at `path`, held by `handle`: the one its `theme.json` was read through.
    #[cfg(unix)]
    pub(crate) fn held(path: PathBuf, handle: std::os::fd::OwnedFd) -> Self {
        Self {
            path,
            handle: Ok(Store::Held(std::sync::Arc::new(handle))),
            approved: None,
        }
    }

    /// A theme built into purlis, `path` its folder in purlis's source for a sentence, whose
    /// `fonts/` holds `files`, compiled in (#2336).
    pub(crate) fn built_in(path: PathBuf, files: &'static [(&'static str, &'static [u8])]) -> Self {
        Self {
            path,
            handle: Ok(Store::BuiltIn(files)),
            approved: None,
        }
    }

    /// The folder, whose faces are read only when `files` (a project's approved font files,
    /// keyed by their path in the project) holds each with its digest; `folder` is its path
    /// in the project, and `measured` what the read it is part of has hashed already.
    #[must_use]
    pub(crate) fn approved_only(
        mut self,
        files: std::sync::Arc<BTreeMap<String, String>>,
        folder: String,
        measured: Measured,
    ) -> Self {
        self.approved = Some(Approved {
            files,
            folder,
            measured,
        });
        self
    }

    /// The folder at `path`, whose fonts are not read, and `why`.
    pub(crate) fn refusing(path: PathBuf, why: String) -> Self {
        Self {
            path,
            handle: Err(why),
            approved: None,
        }
    }

    /// The folder at `path`, held by `dir`, a handle on it the caller opened refusing a link,
    /// or why it could not be: a theme folder named on the command line, and the person's own
    /// config folder (`super::opened`).
    pub(crate) fn opened(path: PathBuf, dir: Result<super::opened::Held, String>) -> Self {
        match dir {
            #[cfg(unix)]
            Ok(dir) => Self::held(path, dir),
            #[cfg(not(unix))]
            Ok(dir) => match dir {},
            Err(why) => Self::refusing(
                path.clone(),
                format!(
                    "purlis could not open {} to read its fonts: {why}",
                    path.display()
                ),
            ),
        }
    }

    /// **The handle the folder is held by**, or why it is not held: what an export packs from
    /// (`super::folders::FoundFolder`).
    pub(crate) fn dir(&self) -> Result<&super::opened::Held, String> {
        match &self.handle {
            #[cfg(unix)]
            Ok(Store::Held(handle)) => Ok(handle.as_ref()),
            #[cfg(not(unix))]
            Ok(Store::Held(handle)) => match *handle {},
            Ok(Store::BuiltIn(_)) => Err(format!(
                "{} is built into purlis, and is not a folder on disk",
                self.path.display()
            )),
            Err(why) => Err(why.clone()),
        }
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
    fonts: Store,
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

/// **The font files one read has hashed already** (#2696): each one's digest and size, by its
/// path in the project and by the file as it was opened — its device, its inode, its size, and
/// its modification and change times. A read that meets the same file again, with nothing of
/// that changed, takes its digest from here instead of reading and hashing it a second time.
///
/// - **It lives as long as the read it is made for**: one window's creation, or one ask of the
///   theme in force. Nothing keeps it for the process.
/// - **Only a settled file is kept**: one whose change time is at least [`SETTLED`] before it was
///   read. A file written since has a later change time, which no write can set back, so it is
///   never taken for the one measured.
/// - **Only a project's or a workspace's faces are asked here**, the files the trust check lists.
///   A face it answers for is drawn only when its digest is the one the approval holds, and
///   [`served`] answers its bytes only once they are read again and have that digest.
#[derive(Debug, Clone)]
pub struct Measured {
    kept: std::sync::Arc<Mutex<Digests>>,
    hashed: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    settled: std::time::Duration,
}

/// What a [`Measured`] holds: a file's digest and size, by its path in the project and the file
/// as it was opened.
type Digests = HashMap<(String, Stamp), (String, u64)>;

/// How long before a file is read its change time is, at least, for its digest to be kept.
pub const SETTLED: std::time::Duration = std::time::Duration::from_secs(1);

/// The most files one [`Measured`] keeps: every file a project's contribution lists, and the
/// faces of the themes read with it.
const MOST_MEASURED: usize = 4 * MOST_PROJECT_FONTS;

impl Default for Measured {
    fn default() -> Self {
        Self::settled_after(SETTLED)
    }
}

/// Two reads are the same read whatever they have measured: what is kept only saves work.
impl PartialEq for Measured {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Eq for Measured {}

impl Measured {
    /// One that keeps a file whose change time is at least `settled` before it is read: a test's
    /// fresh files are kept with [`std::time::Duration::ZERO`].
    pub fn settled_after(settled: std::time::Duration) -> Self {
        Self {
            kept: std::sync::Arc::default(),
            hashed: std::sync::Arc::default(),
            settled,
        }
    }

    /// How many font files the read it is made for has hashed.
    pub fn hashed(&self) -> usize {
        self.hashed.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// How many files it holds the digest of.
    pub fn len(&self) -> usize {
        self.lock().len()
    }

    /// Whether it holds none.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Digests> {
        self.kept
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// The digest and size of `named`, opened as `stamp`, when it was measured as that.
    fn recall(&self, named: &str, stamp: &Stamp) -> Option<(String, u64)> {
        self.lock().get(&(named.to_owned(), stamp.clone())).cloned()
    }

    /// Keeps `named`'s digest and size, hashed now from the file read as `stamp` at `read`,
    /// when it had settled by then.
    fn keep(
        &self,
        named: String,
        stamp: Option<Stamp>,
        read: std::time::SystemTime,
        known: (String, u64),
    ) {
        self.hashed
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        // A file whose size is not what was read changed while it was read.
        let Some(stamp) = stamp.filter(|stamp| stamp.bytes == known.1) else {
            return;
        };
        let settled = read
            .checked_sub(self.settled)
            .is_some_and(|by| stamp.changed <= by);
        if !settled {
            return;
        }
        let mut kept = self.lock();
        if kept.len() < MOST_MEASURED {
            kept.insert((named, stamp), known);
        }
    }
}

/// A file as it was opened: what a write to it, or a file put in its place, changes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Stamp {
    device: u64,
    inode: u64,
    bytes: u64,
    modified: (i64, i64),
    changed: std::time::SystemTime,
}

impl Stamp {
    /// The file `found` describes, from the descriptor it was opened as; `None` off unix.
    #[cfg(unix)]
    fn of(found: &std::fs::Metadata) -> Option<Self> {
        use std::os::unix::fs::MetadataExt;
        let changed = std::time::UNIX_EPOCH.checked_add(std::time::Duration::new(
            u64::try_from(found.ctime()).ok()?,
            u32::try_from(found.ctime_nsec()).ok()?,
        ))?;
        Some(Self {
            device: found.dev(),
            inode: found.ino(),
            bytes: found.len(),
            modified: (found.mtime(), found.mtime_nsec()),
            changed,
        })
    }

    #[cfg(not(unix))]
    fn of(_found: &std::fs::Metadata) -> Option<Self> {
        None
    }
}

/// Whether `name` is a face's file name in `fonts/`: what [`face_file`] takes after `fonts/`.
pub(crate) fn is_face_name(name: &str) -> bool {
    face_file(&format!("fonts/{name}")).is_some()
}

/// The most font files one theme folder's `fonts/` lists in a project's contribution, in name
/// order: every face a theme can name fits.
pub(crate) const MOST_FOLDER_FONTS: usize = 3 * MOST_FACES;

/// The most font files a project's contribution lists, all its theme folders together.
pub(crate) const MOST_PROJECT_FONTS: usize = 64;

/// The most bytes of font files a project's contribution reads, all together.
pub(crate) const MOST_PROJECT_FONT_BYTES: u64 = 32 * 1024 * 1024;

/// The most names one folder is read for, when a project's font files are listed.
pub(crate) const MOST_NAMES_SCANNED: usize = 1024;

/// What a project's contribution records of one font file: the sha-256 of its bytes, then
/// its size in bytes.
pub(crate) fn listed_as(digest: &str, bytes: u64) -> String {
    format!("{digest} {bytes}")
}

/// The digest a contribution's record of a font file holds.
fn digest_of(listed: &str) -> &str {
    listed.split(' ').next().unwrap_or_default()
}

/// **The face `name` in the folder `fonts` holds, measured** for a project's contribution:
/// opened from the handle refusing a link, a plain woff2 file of at most `most` bytes (and
/// never past [`MOST_FONT_BYTES`]); its digest and its size, or `None`. Kept in `memo` as
/// `named`, its path in the project, for the theme read the contribution is handed to.
#[cfg(unix)]
pub(crate) fn measured(
    fonts: &std::os::fd::OwnedFd,
    name: &str,
    most: u64,
    memo: (&Measured, String),
) -> Option<(String, u64)> {
    use std::io::Read;
    let most = most.min(MOST_FONT_BYTES);
    let fd = rustix::fs::openat(
        fonts,
        name,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::NONBLOCK
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .ok()?;
    let mut open = std::fs::File::from(fd);
    let found = open.metadata().ok()?;
    if !found.file_type().is_file() || found.len() > most {
        return None;
    }
    let read = std::time::SystemTime::now();
    let mut bytes = Vec::new();
    (&mut open).take(most + 1).read_to_end(&mut bytes).ok()?;
    let size = bytes.len() as u64;
    let known = (size <= most && bytes.starts_with(WOFF2)).then(|| (hex(&bytes), size))?;
    let (measured, named) = memo;
    measured.keep(named, Stamp::of(&found), read, known.clone());
    Some(known)
}

/// Why a face of a project's or a workspace's theme folder that is read is not drawn: `named`,
/// its path in the project, of `size` bytes, whose digest is not the one the approval `files`
/// holds for it, in a folder whose `fonts/` lists `before` face names ahead of it. And whether
/// the project's contribution lists it, so approving the project again can let it be drawn: a
/// face past a listing limit is never listed, so never approved.
fn unapproved(named: &str, size: u64, files: &Approved, before: usize) -> (String, bool) {
    let past = |why: String| (why, false);
    match files.files.get(named) {
        Some(_) => (
            format!("{named} has changed since you approved this project, so it is not read"),
            true,
        ),
        None if before >= MOST_FOLDER_FONTS => past(format!(
            "{named} is past the {MOST_FOLDER_FONTS} font files purlis lists in a theme \
             folder's fonts/, so it is not read"
        )),
        None => {
            let bytes: u64 = files
                .files
                .values()
                .filter_map(|listed| listed.split(' ').nth(1)?.parse::<u64>().ok())
                .sum();
            if files.files.len() >= MOST_PROJECT_FONTS || bytes + size > MOST_PROJECT_FONT_BYTES {
                past(format!(
                    "{named} is past the {MOST_PROJECT_FONTS} font files ({} MiB) purlis lists \
                     in a project, so it is not read",
                    MOST_PROJECT_FONT_BYTES / 1024 / 1024
                ))
            } else {
                (
                    format!(
                        "{named} is not a font file you approved with this project, so it is \
                         not read"
                    ),
                    true,
                )
            }
        }
    }
}

/// The digest a project's approval holds for a font file of these bytes.
pub(crate) fn digest(bytes: &[u8]) -> String {
    hex(bytes)
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
fn read_face(fonts: &Store, name: &str, shown: &str) -> Result<Vec<u8>, String> {
    let fonts = match fonts {
        Store::Held(fonts) => fonts,
        Store::BuiltIn(files) => {
            let (_, bytes) = files
                .iter()
                .find(|(one, _)| *one == name)
                .ok_or_else(|| missed(shown, Missed::Gone))?;
            if bytes.len() as u64 > MOST_FONT_BYTES {
                return Err(format!(
                    "{shown} is {} bytes, and a font is never larger than {MOST_FONT_BYTES}",
                    bytes.len()
                ));
            }
            if !bytes.starts_with(WOFF2) {
                return Err(format!("{shown} is not a woff2 file"));
            }
            return Ok(bytes.to_vec());
        }
    };
    let (mut open, _) = opened_face(fonts, name, shown)?;
    read_opened(&mut open, shown)
}

/// The face `name` in the held folder `fonts`, opened from that handle refusing a link: a plain
/// file under the cap, and what it is.
fn opened_face(
    fonts: &held::Handle,
    name: &str,
    shown: &str,
) -> Result<(std::fs::File, std::fs::Metadata), String> {
    let open = held::face(fonts, name).map_err(|why| missed(shown, why))?;
    let found = open
        .metadata()
        .map_err(|why| format!("purlis could not read {shown}: {why}"))?;
    if !found.file_type().is_file() {
        return Err(format!("{shown} is not a plain file"));
    }
    if found.len() > MOST_FONT_BYTES {
        return Err(too_big(shown, found.len()));
    }
    Ok((open, found))
}

/// Why `shown`, of `bytes` bytes, is not read.
fn too_big(shown: &str, bytes: u64) -> String {
    format!("{shown} is {bytes} bytes, and a font is never larger than {MOST_FONT_BYTES}")
}

/// The bytes of the face `open`, read whole under the cap, and a woff2 file by its signature.
fn read_opened(open: &mut std::fs::File, shown: &str) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let mut bytes = Vec::new();
    open.take(MOST_FONT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|why| format!("purlis could not read {shown}: {why}"))?;
    if bytes.len() as u64 > MOST_FONT_BYTES {
        return Err(too_big(shown, bytes.len() as u64));
    }
    if !bytes.starts_with(WOFF2) {
        return Err(format!("{shown} is not a woff2 file"));
    }
    Ok(bytes)
}

/// **The digest and size of the face `name`** in `fonts`, read as [`read_face`] reads it. For a
/// project's or a workspace's face, `memo` is what this read has measured and its path in the
/// project: the digest comes from there for the file already measured, unchanged, and is kept
/// there once it is hashed here.
fn digest_face(
    fonts: &Store,
    name: &str,
    shown: &str,
    memo: Option<(&Measured, String)>,
) -> Result<(String, u64), String> {
    let (Store::Held(held), Some((measured, named))) = (fonts, memo) else {
        return read_face(fonts, name, shown).map(|bytes| {
            let size = bytes.len() as u64;
            (hex(&bytes), size)
        });
    };
    let (mut open, found) = opened_face(held, name, shown)?;
    let stamp = Stamp::of(&found);
    if let Some(known) = stamp
        .as_ref()
        .and_then(|stamp| measured.recall(&named, stamp))
    {
        return Ok(known);
    }
    let read = std::time::SystemTime::now();
    let bytes = read_opened(&mut open, shown)?;
    let known = (hex(&bytes), bytes.len() as u64);
    measured.keep(named, stamp, read, known.clone());
    Ok(known)
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
/// own folder when it is read from one. Says in `complaints` what refuses the theme; a face the
/// project's approval does not hold is said as awaiting it (#2647), so the window can offer the
/// project's trust ask.
pub(super) fn read(
    raw: Option<&Value>,
    base: &Fonts,
    folder: Option<&ThemeDir>,
    complaints: &mut Vec<super::Complaint>,
) -> Fonts {
    let mut said = Vec::new();
    let mut awaiting: Vec<(usize, String)> = Vec::new();
    let fonts = read_said(raw, base, folder, &mut said, &mut awaiting);
    complaints.extend(said.into_iter().enumerate().map(|(at, one)| {
        match awaiting.iter().find(|(place, _)| *place == at) {
            Some((_, face)) => super::Complaint::awaiting_approval(one, face.clone()),
            None => super::Complaint::said(one),
        }
    }));
    fonts
}

/// [`read`], in sentences: `awaiting` gets the place in `complaints` of each that awaits the
/// project's approval, with its face as `<path> <sha-256>`.
fn read_said(
    raw: Option<&Value>,
    base: &Fonts,
    folder: Option<&ThemeDir>,
    complaints: &mut Vec<String>,
    awaiting: &mut Vec<(usize, String)>,
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
        fonts: dir.fonts(),
        folder: folder.path(),
        approved: folder.approved.as_ref(),
        awaiting: std::cell::RefCell::new(Vec::new()),
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
    awaiting.extend(inside.awaiting.into_inner());
    fonts
}

/// A theme folder's `fonts/`, opened from the folder's handle, or why not; and the folder, for a
/// sentence.
struct Inside<'a> {
    fonts: Result<Store, Missed>,
    approved: Option<&'a Approved>,
    folder: &'a Path,
    /// The place in the complaints of each face the approval does not hold but would, once the
    /// project is approved again.
    awaiting: std::cell::RefCell<Vec<(usize, String)>>,
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
            Some(name) => match super::portable::refusal(file, name) {
                Some(why) => {
                    complaints.push(format!("a face of fonts.{slot}: {why}"));
                    None
                }
                None => Some(name.to_owned()),
            },
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
    let named = folder
        .approved
        .map(|approved| format!("{}/fonts/{file}", approved.folder));
    let memo = folder
        .approved
        .zip(named.clone())
        .map(|(approved, named)| (&approved.measured, named));
    match digest_face(fonts, &file, &shown, memo) {
        Ok((asset, size)) => {
            if let Some((approved, named)) = folder.approved.zip(named) {
                let approved_digest = approved.files.get(&named).map(|listed| digest_of(listed));
                if approved_digest != Some(asset.as_str()) {
                    let before = fonts.names_before(&file);
                    let (why, listed) = unapproved(&named, size, approved, before);
                    if listed {
                        folder
                            .awaiting
                            .borrow_mut()
                            .push((complaints.len(), format!("{named} {asset}")));
                    }
                    complaints.push(why);
                    return None;
                }
            }
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

    /// The folder's `fonts/`, opened from its handle refusing a link.
    pub(super) fn fonts(dir: &Handle) -> Result<Handle, Missed> {
        openat(dir, "fonts", DIRECTORY, Mode::empty())
            .map(Arc::new)
            .map_err(missed)
    }

    /// How many face names in `fonts` sort before `name`, as a project's contribution lists
    /// them.
    pub(super) fn names_before(fonts: &Handle, name: &str) -> usize {
        let Ok(dir) = rustix::fs::Dir::read_from(fonts.as_ref()) else {
            return 0;
        };
        dir.filter_map(Result::ok)
            .take(super::MOST_NAMES_SCANNED)
            .filter_map(|entry| entry.file_name().to_str().ok().map(str::to_owned))
            .filter(|one| super::is_face_name(one) && one.as_str() < name)
            .count()
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

    use super::Missed;

    /// No folder is held here.
    pub(crate) type Handle = std::convert::Infallible;

    pub(super) fn fonts(dir: &Handle) -> Result<Handle, Missed> {
        match *dir {}
    }

    pub(super) fn face(fonts: &Handle, _name: &str) -> Result<std::fs::File, Missed> {
        match *fonts {}
    }

    pub(super) fn names_before(fonts: &Handle, _name: &str) -> usize {
        match *fonts {}
    }
}
