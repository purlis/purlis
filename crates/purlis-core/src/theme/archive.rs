// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Export** (spec #2313, #2334): a theme folder as one zip to hand to someone, and back.
//!
//! [`export`] packs a theme folder: its `theme.json`, every plain file beside it (a licence, a
//! preview), and the plain files in its `fonts/` and `icons/`, all under one folder named as the
//! theme folder is, so unzipping it gives the folder back. What it does not carry (a hidden
//! file, a link, another folder) is listed, never packed. [`unpack`] reads such a zip back into
//! the same files, and [`super::import`] of a zip ([`super::SourceKind::Zip`]) is that.
//!
//! The zip is the same bytes every time for the same folder: its entries in order, each with
//! one fixed time and mode.
//!
//! **Bounds**, the same both ways: at most [`MOST_FILES`] files, each at most
//! [`MOST_FILE_BYTES`] (the most a font may be), all at most [`MOST_FOLDER_BYTES`], and a zip at
//! most [`MOST_ZIP_BYTES`]. An entry is read only under one folder and only where a theme folder
//! keeps a file: an entry whose name climbs out (`..`), starts at the root, names a drive, is a
//! link or is encrypted refuses the whole zip, and nothing of it is written.
//!
//! [`stage`] writes a theme folder's files beside where it goes, so the engine can read it
//! before it is put in place; [`Staged::commit`] puts it there, and an existing folder is
//! replaced only when asked.

use std::collections::BTreeSet;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use super::THEME_JSON;
use super::import::{Files, LeftOut};

/// The most files a theme folder's zip holds.
pub const MOST_FILES: usize = 64;

/// The most one file of a theme folder may be: a font's cap.
pub const MOST_FILE_BYTES: u64 = super::MOST_FONT_BYTES;

/// The most a theme folder's files may be together.
pub const MOST_FOLDER_BYTES: u64 = 16 * 1024 * 1024;

/// The most a theme folder's zip may be.
pub const MOST_ZIP_BYTES: u64 = MOST_FOLDER_BYTES + 1024 * 1024;

/// The most entries a zip may list: its files, and the three folders an unzip-and-zip by hand
/// adds (the theme folder, `fonts/`, `icons/`).
const MOST_ENTRIES: usize = MOST_FILES + 3;

/// The most entries a zip's end may list before any is read: a theme folder's own, and what
/// the macOS Finder adds to a zip it makes (a shadow entry for each, and its own files), which
/// is passed over and is not one of [`MOST_ENTRIES`].
const MOST_LISTED: usize = 2 * MOST_ENTRIES + 8;

/// The ids no theme folder takes: Purlis Classic's, the ids its appearances have had, the
/// person's own theme (`mine`) and following the system (`system`).
pub fn is_reserved(id: &str) -> bool {
    id == super::CLASSIC || super::built_in(id).is_some() || matches!(id, "mine" | "system")
}

/// Whether `id` is an id a theme folder may have: a name of the palette's grammar (lower-case
/// letters, digits and dashes, starting with a letter, at most 32 long) that Windows holds and
/// that is not reserved.
pub fn is_folder_id(id: &str) -> bool {
    super::is_folder_name(id) && !is_reserved(id)
}

/// Why `id` is not a theme folder's id, as a sentence.
fn not_an_id(id: &str) -> String {
    if is_reserved(id) {
        format!(
            "{} is an id purlis keeps for its own themes, so no theme folder takes it",
            crate::shown::short(id)
        )
    } else if let Some(why) = super::portable::refusal(id, id) {
        why
    } else {
        format!(
            "{} is not a theme folder's id: lower-case letters, digits and dashes, starting \
             with a letter, at most 32 long",
            crate::shown::short(id)
        )
    }
}

/// What the macOS Finder adds to every zip it makes: a folder of its own beside the theme
/// folder, and a file in each folder.
const FINDER_FOLDER: &str = "__MACOSX";
const FINDER_FILE: &str = ".DS_Store";

/// The folders inside a theme folder whose files are carried.
const KEPT_FOLDERS: &[&str] = &["fonts", "icons"];

/// Whether `name` is one plain file or folder name a theme folder may hold: not empty, not `.`
/// or `..`, not hidden, with no separator, drive or nul in it, at most 128 bytes.
fn is_plain(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && !name.starts_with('.')
        && !name.contains(['/', '\\', ':', '\0'])
}

/// Whether `path`, inside a theme folder, is where a theme folder keeps a file: a plain name
/// beside `theme.json`, or one in `fonts/` or `icons/`.
fn is_kept(path: &str) -> bool {
    match path.split('/').collect::<Vec<_>>().as_slice() {
        [file] => is_plain(file),
        [folder, file] => KEPT_FOLDERS.contains(folder) && is_plain(file),
        _ => false,
    }
}

/// The files of a theme folder, packed.
#[derive(Debug, Clone, PartialEq)]
pub struct Packed {
    /// The folder's name, the zip's one top folder.
    pub name: String,
    /// The zip.
    pub bytes: Vec<u8>,
    /// How many files it holds.
    pub files: usize,
    /// What the folder holds that the zip does not carry, and why.
    pub left_out: Vec<LeftOut>,
}

/// **Packs the theme folder `folder`** into a zip whose one top folder is named as `folder` is.
/// A folder that is a link, one without a plain `theme.json`, and one past the bounds are
/// refused with a sentence.
pub fn export(folder: &Path) -> Result<Packed, String> {
    let name = folder
        .file_name()
        .and_then(|it| it.to_str())
        .ok_or_else(|| format!("{} is not a theme folder purlis can name", folder.display()))?
        .to_owned();
    if !is_folder_id(&name) {
        return Err(not_an_id(&name));
    }
    let (files, left_out) = gather(folder)?;
    let bytes = pack(&name, &files)?;
    Ok(Packed {
        name,
        bytes,
        files: files.len(),
        left_out,
    })
}

/// The files of the theme folder `folder` a zip carries, and what it does not.
pub fn gather(folder: &Path) -> Result<(Files, Vec<LeftOut>), String> {
    let shown = folder.display();
    let meta = std::fs::symlink_metadata(folder)
        .map_err(|why| format!("{shown} could not be read: {why}"))?;
    if !meta.file_type().is_dir() {
        return Err(format!("{shown} is not a folder"));
    }
    let theme = std::fs::symlink_metadata(folder.join(THEME_JSON));
    if !theme.is_ok_and(|it| it.file_type().is_file()) {
        return Err(format!(
            "{shown} has no {THEME_JSON}, so it is not a theme folder"
        ));
    }
    let mut files = Files::new();
    let mut left_out = Vec::new();
    let mut total = 0u64;
    let mut take = |path: String, at: &Path, files: &mut Files| -> Result<(), String> {
        if files.len() >= MOST_FILES {
            return Err(format!(
                "{shown} holds more than {MOST_FILES} files, the most a theme folder's zip carries"
            ));
        }
        let bytes = read_capped(folder, at, MOST_FILE_BYTES)?;
        total += bytes.len() as u64;
        if total > MOST_FOLDER_BYTES {
            return Err(format!(
                "{shown} holds more than {} MiB, the most a theme folder's zip carries",
                MOST_FOLDER_BYTES / 1024 / 1024
            ));
        }
        files.insert(path, bytes);
        Ok(())
    };
    for (name, at, kind) in entries(folder)? {
        // A name Windows cannot hold is not carried: the zip opens on every platform.
        if let Some(why) = super::portable::refusal(&name, &name) {
            left_out.push(LeftOut::new(name, why));
            continue;
        }
        match kind {
            Entry::File if is_plain(&name) => take(name, &at, &mut files)?,
            Entry::Folder if KEPT_FOLDERS.contains(&name.as_str()) => {
                for (inner, inner_at, inner_kind) in entries(&at)? {
                    let path = format!("{name}/{inner}");
                    if let Some(why) = super::portable::refusal(&path, &inner) {
                        left_out.push(LeftOut::new(path, why));
                        continue;
                    }
                    match inner_kind {
                        Entry::File if is_plain(&inner) => take(path, &inner_at, &mut files)?,
                        other => left_out.push(LeftOut::new(path, other.why(&inner))),
                    }
                }
            }
            other => left_out.push(LeftOut::new(name.clone(), other.why(&name))),
        }
    }
    Ok((files, left_out))
}

/// What a folder's entry is.
#[derive(Clone, Copy)]
enum Entry {
    File,
    Folder,
    Link,
    Other,
}

impl Entry {
    /// Why a zip does not carry an entry `name` of this kind.
    fn why(self, name: &str) -> &'static str {
        match self {
            _ if name.starts_with('.') => "a hidden file is not carried",
            Self::Link => "a link is not carried",
            Self::Folder => "a theme folder's zip carries only fonts/ and icons/",
            Self::File | Self::Other => "is not a plain file a theme folder keeps",
        }
    }
}

/// The entries of `folder`, by name, in order: a name that is not text is said as it is.
fn entries(folder: &Path) -> Result<Vec<(String, PathBuf, Entry)>, String> {
    let listed = std::fs::read_dir(folder)
        .map_err(|why| format!("{} could not be read: {why}", folder.display()))?;
    let mut found = Vec::new();
    for entry in listed {
        let entry =
            entry.map_err(|why| format!("{} could not be read: {why}", folder.display()))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let kind = match std::fs::symlink_metadata(entry.path()).map(|it| it.file_type()) {
            Ok(kind) if kind.is_symlink() => Entry::Link,
            Ok(kind) if kind.is_dir() => Entry::Folder,
            Ok(kind) if kind.is_file() => Entry::File,
            _ => Entry::Other,
        };
        found.push((name, entry.path(), kind));
    }
    found.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(found)
}

/// The bytes of the plain file `at` inside `folder`, at most `most`: opened refusing a link on
/// the way or at the file, and read only when the handle is a plain file.
fn read_capped(folder: &Path, at: &Path, most: u64) -> Result<Vec<u8>, String> {
    let failed = |why: std::io::Error| format!("{} could not be read: {why}", at.display());
    let mut open = crate::contain::open_no_link(folder, at).map_err(failed)?;
    if !open.metadata().map_err(failed)?.file_type().is_file() {
        return Err(format!("{} is not a plain file", at.display()));
    }
    let mut bytes = Vec::new();
    (&mut open)
        .take(most + 1)
        .read_to_end(&mut bytes)
        .map_err(failed)?;
    if bytes.len() as u64 > most {
        return Err(format!(
            "{} is larger than {} KiB, the most one file of a theme folder may be",
            at.display(),
            most / 1024
        ));
    }
    Ok(bytes)
}

/// **`files` as a zip** under the one folder `name`: each file deflated, in order, with one
/// fixed time and mode, so the same files are always the same bytes.
pub fn pack(name: &str, files: &Files) -> Result<Vec<u8>, String> {
    if !is_folder_id(name) {
        return Err(not_an_id(name));
    }
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .last_modified_time(zip::DateTime::default())
        .unix_permissions(0o644);
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let failed = |why: zip::result::ZipError| format!("the zip could not be written: {why}");
    for (path, bytes) in files {
        writer
            .start_file(format!("{name}/{path}"), options)
            .map_err(failed)?;
        writer
            .write_all(bytes)
            .map_err(|why| format!("the zip could not be written: {why}"))?;
    }
    Ok(writer.finish().map_err(failed)?.into_inner())
}

/// **The theme folder a zip holds**: its name and its files. A zip past the bounds, one with
/// more than one top folder or a file outside it, an entry that climbs out, starts at the root,
/// is a link or is encrypted, a file where a theme folder keeps none, and a zip without a
/// `theme.json` are refused whole, with a sentence that names the entry.
pub fn unpack(bytes: &[u8]) -> Result<(String, Files), String> {
    if bytes.len() as u64 > MOST_ZIP_BYTES {
        return Err(format!(
            "the zip is larger than {} MiB, the most a theme folder's zip may be",
            MOST_ZIP_BYTES / 1024 / 1024
        ));
    }
    // The count the zip's end says, read before the archive lists a single entry.
    let too_many = |listed: usize| {
        format!(
            "the zip lists {listed} entries, more than a theme folder's zip may ({MOST_ENTRIES})"
        )
    };
    if let Some(listed) = listed_entries(bytes)
        && listed > MOST_LISTED
    {
        return Err(too_many(listed));
    }
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|why| format!("this is not a zip purlis can read: {why}"))?;
    // A zip ends with its end record: nothing is after it but the record's own comment.
    let Some(listed) = listed_entries(bytes) else {
        return Err("the zip does not end where its last record says it does".to_owned());
    };
    // Each entry is named once: the end's count and the entries read agree.
    if listed != archive.len() {
        return Err("the zip names an entry twice".to_owned());
    }
    let mut kept = 0usize;
    let mut folder: Option<String> = None;
    let mut files = Files::new();
    let mut seen = BTreeSet::new();
    let mut folded = BTreeSet::new();
    let mut total = 0u64;
    for at in 0..archive.len() {
        // Asked of the entry as it is stored, before anything decrypts or inflates it.
        if archive
            .by_index_raw(at)
            .map_err(|why| format!("entry {at} of the zip cannot be read: {why}"))?
            .encrypted()
        {
            return Err(format!("entry {at} of the zip is encrypted"));
        }
        let mut entry = archive
            .by_index(at)
            .map_err(|why| format!("entry {at} of the zip cannot be read: {why}"))?;
        let raw = String::from_utf8(entry.name_raw().to_vec())
            .map_err(|_| format!("entry {at} of the zip is not named in UTF-8"))?;
        let named = crate::shown::short(&raw);
        if entry.encrypted() {
            return Err(format!("{named} in the zip is encrypted"));
        }
        if entry.is_symlink() {
            return Err(format!(
                "{named} in the zip is a link, which a theme folder never holds"
            ));
        }
        if raw.starts_with('/') || raw.contains(['\\', ':', '\0']) {
            return Err(format!(
                "{named} in the zip is not a path inside one theme folder"
            ));
        }
        let dir = entry.is_dir();
        let trimmed = if dir {
            raw.trim_end_matches('/')
        } else {
            raw.as_str()
        };
        let mut parts = trimmed.split('/');
        let top = parts.next().unwrap_or_default();
        let rest: Vec<&str> = parts.collect();
        // What the macOS Finder adds to a zip it makes is not the theme's, and is passed over.
        if top == FINDER_FOLDER || top == FINDER_FILE || rest.last() == Some(&FINDER_FILE) {
            continue;
        }
        kept += 1;
        if kept > MOST_ENTRIES {
            return Err(too_many(kept));
        }
        if !is_plain(top) || rest.iter().any(|part| matches!(*part, "" | "." | "..")) {
            return Err(format!(
                "{named} in the zip is not a path inside one theme folder"
            ));
        }
        let entry_named = format!("{named} in the zip");
        if let Some(why) = super::portable::path_refusal(&entry_named, trimmed) {
            return Err(why);
        }
        if !is_folder_id(top) {
            return Err(format!("the zip's folder: {}", not_an_id(top)));
        }
        match &folder {
            None => folder = Some(top.to_owned()),
            Some(one) if one == top => {}
            Some(one) => {
                return Err(format!(
                    "the zip holds {} beside {}: a theme folder's zip holds one folder",
                    crate::shown::short(top),
                    crate::shown::short(one)
                ));
            }
        }
        let inside = rest.join("/");
        if dir {
            if inside.is_empty() || KEPT_FOLDERS.contains(&inside.as_str()) {
                continue;
            }
            return Err(format!(
                "{named} in the zip is a folder a theme folder does not keep"
            ));
        }
        if !is_kept(&inside) {
            return Err(format!(
                "{named} in the zip is not where a theme folder keeps a file"
            ));
        }
        if !seen.insert(inside.clone()) {
            return Err(format!("{named} is in the zip twice"));
        }
        // Two names a case-blind or normalising filesystem writes as one file.
        if !folded.insert(fold(&inside)) {
            return Err(format!(
                "{named} in the zip is another entry's name in other case or other accents, \
                 which some filesystems write as one file"
            ));
        }
        if files.len() >= MOST_FILES {
            return Err(format!("the zip holds more than {MOST_FILES} files"));
        }
        if entry.size() > MOST_FILE_BYTES {
            return Err(format!(
                "{named} in the zip is larger than {} KiB, the most one file of a theme folder may be",
                MOST_FILE_BYTES / 1024
            ));
        }
        let mut file = Vec::new();
        (&mut entry)
            .take(MOST_FILE_BYTES + 1)
            .read_to_end(&mut file)
            .map_err(|why| format!("{named} in the zip cannot be read: {why}"))?;
        if file.len() as u64 > MOST_FILE_BYTES {
            return Err(format!(
                "{named} in the zip is larger than {} KiB, the most one file of a theme folder may be",
                MOST_FILE_BYTES / 1024
            ));
        }
        total += file.len() as u64;
        if total > MOST_FOLDER_BYTES {
            return Err(format!(
                "the zip's files are more than {} MiB together",
                MOST_FOLDER_BYTES / 1024 / 1024
            ));
        }
        files.insert(inside, file);
    }
    let Some(folder) = folder else {
        return Err("the zip is empty".to_owned());
    };
    if !files.contains_key(THEME_JSON) {
        return Err(format!(
            "the zip's {} has no {THEME_JSON}, so it is not a theme folder",
            crate::shown::short(&folder)
        ));
    }
    Ok((folder, files))
}

/// How many entries the end of central directory of `bytes` says the zip holds, the larger of
/// its two counts; `None` when there is no end to read, which the archive then says. A zip64
/// end (the counts at their most) is read as more than any theme folder's zip holds.
fn listed_entries(bytes: &[u8]) -> Option<usize> {
    const END: &[u8; 4] = b"PK\x05\x06";
    // The end is 22 bytes and its comment, which runs to the end of the file: the last record
    // whose comment's length lands exactly there is the zip's end.
    let from = bytes.len().saturating_sub(22 + 65_535);
    let field = |at: usize, offset: usize| -> Option<usize> {
        let two = bytes.get(at + offset..at + offset + 2)?;
        Some(usize::from(u16::from_le_bytes([two[0], two[1]])))
    };
    let at = (from..bytes.len().saturating_sub(21)).rev().find(|at| {
        bytes[*at..].starts_with(END)
            && field(*at, 20).is_some_and(|comment| at + 22 + comment == bytes.len())
    })?;
    Some(field(at, 8)?.max(field(at, 10)?))
}

/// A path as a case-blind, normalising filesystem compares it: composed, in lower case.
fn fold(path: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    path.nfc().flat_map(char::to_lowercase).collect()
}

/// A theme folder written beside where it goes, not yet in place: removed when dropped unless
/// [`Self::commit`] put it there.
#[derive(Debug)]
pub struct Staged {
    /// The folder the theme folder goes in.
    pub into: PathBuf,
    /// The theme folder's name there.
    pub name: String,
    /// Where it is written for now: a hidden folder in `into`. Read only through
    /// [`Self::read`]; [`Self::path`] says where it is.
    path: PathBuf,
    /// Whether a theme folder of its name that is there is replaced.
    force: bool,
    done: bool,
}

/// Why a theme folder was not put in place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotWritten {
    /// A theme folder of that name is there, and replacing it was not asked for.
    Exists(PathBuf),
    /// Anything else, said.
    Failed(String),
}

impl std::fmt::Display for NotWritten {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Exists(path) => write!(
                f,
                "{} is there already, and is replaced only with --force",
                path.display()
            ),
            Self::Failed(why) => f.write_str(why),
        }
    }
}

/// **Writes `files` as the theme folder `name` beside where it goes in `into`**, creating `into`
/// when it is not there. A name that is not one plain folder name, a folder of that name that is
/// there when `force` is not given, and one that is a link are refused before anything is
/// written.
pub fn stage(into: &Path, name: &str, files: &Files, force: bool) -> Result<Staged, NotWritten> {
    if !is_folder_id(name) {
        return Err(NotWritten::Failed(not_an_id(name)));
    }
    let target = into.join(name);
    match std::fs::symlink_metadata(&target) {
        Ok(meta) if meta.file_type().is_symlink() => {
            return Err(NotWritten::Failed(format!(
                "{} is a link: purlis does not replace it",
                target.display()
            )));
        }
        Ok(meta) if !meta.file_type().is_dir() => {
            return Err(NotWritten::Failed(format!(
                "{} is there and is not a folder: purlis does not replace it",
                target.display()
            )));
        }
        Ok(_) if !force => return Err(NotWritten::Exists(target)),
        _ => {}
    }
    for path in files.keys() {
        if let Some(why) = super::portable::path_refusal(path, path) {
            return Err(NotWritten::Failed(why));
        }
        if !is_kept(path) {
            return Err(NotWritten::Failed(format!(
                "{} is not where a theme folder keeps a file",
                crate::shown::short(path)
            )));
        }
    }
    let failed = |why: std::io::Error| NotWritten::Failed(format!("{}: {why}", into.display()));
    std::fs::create_dir_all(into).map_err(failed)?;
    sweep(into);
    let path = into.join(format!(".{name}{STAGED}{}", std::process::id()));
    if std::fs::symlink_metadata(&path).is_ok() {
        std::fs::remove_dir_all(&path).map_err(failed)?;
    }
    std::fs::create_dir(&path).map_err(failed)?;
    let staged = Staged {
        into: into.to_path_buf(),
        name: name.to_owned(),
        path,
        force,
        done: false,
    };
    for (inside, bytes) in files {
        let at = staged.path.join(inside);
        if let Some((folder, _)) = inside.split_once('/') {
            let folder = staged.path.join(folder);
            if std::fs::symlink_metadata(&folder).is_err() {
                std::fs::create_dir(&folder).map_err(failed)?;
            }
        }
        crate::contain::create_no_link(&staged.path, &at)
            .and_then(|mut file| file.write_all(bytes))
            .map_err(failed)?;
    }
    Ok(staged)
}

/// What a staged folder's name holds after the theme folder's: `.<name>.importing-<pid>`.
const STAGED: &str = ".importing-";

/// How old a staged folder is before it is taken for one an import left behind.
const STALE: std::time::Duration = std::time::Duration::from_secs(60 * 60);

/// Removes the staged folders in `into` an import left behind more than [`STALE`] ago. Only a
/// folder, never a link, by a name only a stage has.
fn sweep(into: &Path) {
    let Ok(listed) = std::fs::read_dir(into) else {
        return;
    };
    for entry in listed.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if !name.starts_with('.') || !name.contains(STAGED) {
            continue;
        }
        let path = entry.path();
        let Ok(meta) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        let old = meta
            .modified()
            .ok()
            .and_then(|at| at.elapsed().ok())
            .is_some_and(|age| age > STALE);
        if meta.file_type().is_dir() && old {
            let _ = std::fs::remove_dir_all(&path);
        }
    }
}

impl Staged {
    /// Where the theme folder is written for now: a hidden folder in [`Self::into`]. To read
    /// it, [`Self::read`].
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// **The engine's read of the staged folder**, as it reads any theme folder the person
    /// named, its fonts included (`library` holds the folders beside it its `extends` may name),
    /// but only to check ([`super::Library::checking`]): the stage is moved or removed next, so
    /// none of its fonts is registered to be served from where it is now.
    pub fn read(&self, library: super::Library) -> super::Resolved {
        let staged = self
            .path
            .file_name()
            .and_then(|it| it.to_str())
            .unwrap_or_default();
        library
            .reading_named_fonts()
            .checking()
            .load_folder(&self.into, staged)
    }

    /// **Puts the theme folder in place**: `into/name`. Without `force` given to [`stage`], it
    /// never replaces anything there, an empty folder included: the move itself refuses
    /// (`NotWritten::Exists`). With `force`, a folder there is exchanged for the new one in one
    /// step and then removed; anything there that is not a folder is put back and refused.
    /// Answers where it is.
    pub fn commit(mut self) -> Result<PathBuf, NotWritten> {
        let target = self.into.join(&self.name);
        let failed =
            |why: std::io::Error| NotWritten::Failed(format!("{}: {why}", target.display()));
        match move_in(&self.path, &target, self.force) {
            Ok(Moved::Into) => {}
            Ok(Moved::Exchanged) => {
                // The stage's path now holds what was there.
                let displaced = std::fs::symlink_metadata(&self.path);
                if !displaced.is_ok_and(|it| it.file_type().is_dir()) {
                    let _ = exchange(&self.path, &target);
                    return Err(NotWritten::Failed(format!(
                        "{} is there and is not a folder: purlis does not replace it",
                        target.display()
                    )));
                }
                let _ = std::fs::remove_dir_all(&self.path);
            }
            Ok(Moved::There) => return Err(NotWritten::Exists(target)),
            Err(why) => return Err(failed(why)),
        }
        self.done = true;
        Ok(target)
    }
}

/// How a staged folder went in.
enum Moved {
    /// Into a place nothing held.
    Into,
    /// In exchange for what was there.
    Exchanged,
    /// Not: something is there, and replacing it was not asked for.
    There,
}

/// Moves `from` to `to`: never over anything without `force`, and with it in exchange for
/// what is there.
fn move_in(from: &Path, to: &Path, force: bool) -> std::io::Result<Moved> {
    if rename_no_replace(from, to)? {
        return Ok(Moved::Into);
    }
    if !force {
        return Ok(Moved::There);
    }
    exchange(from, to)?;
    Ok(Moved::Exchanged)
}

/// **Renames `from` to `to` only when nothing is at `to`**, in one step where the system has
/// one (`renameat2(RENAME_NOREPLACE)`, `renameatx_np(RENAME_EXCL)`): `false` when something
/// is. A filesystem without it is looked at, then renamed onto.
pub fn rename_no_replace(from: &Path, to: &Path) -> std::io::Result<bool> {
    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios"
    ))]
    match rustix::fs::renameat_with(
        rustix::fs::CWD,
        from,
        rustix::fs::CWD,
        to,
        rustix::fs::RenameFlags::NOREPLACE,
    ) {
        Ok(()) => return Ok(true),
        Err(rustix::io::Errno::EXIST | rustix::io::Errno::NOTEMPTY) => return Ok(false),
        Err(rustix::io::Errno::INVAL | rustix::io::Errno::NOSYS | rustix::io::Errno::NOTSUP) => {}
        Err(why) => return Err(why.into()),
    }
    if std::fs::symlink_metadata(to).is_ok() {
        return Ok(false);
    }
    std::fs::rename(from, to)?;
    Ok(true)
}

/// Exchanges `a` and `b` in one step (`RENAME_EXCHANGE`, `RENAME_SWAP`). Where the system has
/// none, the two are swapped through a third name, each step undone if the next fails.
fn exchange(a: &Path, b: &Path) -> std::io::Result<()> {
    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios"
    ))]
    match rustix::fs::renameat_with(
        rustix::fs::CWD,
        a,
        rustix::fs::CWD,
        b,
        rustix::fs::RenameFlags::EXCHANGE,
    ) {
        Ok(()) => return Ok(()),
        Err(rustix::io::Errno::INVAL | rustix::io::Errno::NOSYS | rustix::io::Errno::NOTSUP) => {}
        Err(why) => return Err(why.into()),
    }
    let mut aside = a.as_os_str().to_owned();
    aside.push(".swap");
    let aside = PathBuf::from(aside);
    std::fs::rename(a, &aside)?;
    if let Err(why) = std::fs::rename(b, a) {
        let _ = std::fs::rename(&aside, a);
        return Err(why);
    }
    if let Err(why) = std::fs::rename(&aside, b) {
        let _ = std::fs::rename(a, b);
        let _ = std::fs::rename(&aside, a);
        return Err(why);
    }
    Ok(())
}

impl Drop for Staged {
    fn drop(&mut self) {
        if !self.done {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}
