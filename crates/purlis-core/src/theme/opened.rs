// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A folder opened, and every theme file read from it** (spec #2313, #2604, #2670).
//!
//! Every file of a theme is read from a handle: a folder is opened from the one before it
//! refusing a link, and each file is opened from that handle refusing a link, checked to be a
//! plain file under the cap and read from the descriptor that was checked. Nothing is looked up
//! again by path between the open and the read. The levels' folders ([`super::folders`]), a
//! theme folder named on the command line ([`super::Named`]), the person's own `theme.json`
//! ([`super::read_theirs`]) and export ([`super::export`]) all read through this.
//!
//! Off unix nothing is read: a guard that cannot be expressed refuses rather than degrades
//! (ADR 0031).

use std::path::Path;

use serde_json::Value;

/// The largest theme file read: `theme.json` and an icon set.
const MOST_BYTES: u64 = crate::windowprefs::MAX_BYTES;

/// What one entry of a folder is, asked of the entry itself and never of what it leads to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    File,
    Folder,
    Link,
    Other,
}

/// A theme folder's `theme.json`, parsed, and the icon set it names, parsed, or why not; `None`
/// when it names none.
pub(crate) type Document = (Value, Option<Result<Value, String>>);

#[cfg(unix)]
pub(crate) use unix::*;

#[cfg(not(unix))]
pub(crate) use elsewhere::*;

/// Whether `named` is a relative path of plain names no longer than an icon set's path may be.
fn inside(named: &str) -> bool {
    !named.is_empty()
        && named.len() <= super::format::MOST_ICONS_PATH
        && !named.contains(['\\', ':'])
        && Path::new(named)
            .components()
            .all(|part| matches!(part, std::path::Component::Normal(_)))
}

/// Why `named` is not an icon set's path inside a theme folder, or `None` when it is one.
fn not_inside(named: &str) -> Option<String> {
    if !inside(named) {
        return Some(format!(
            "icons is {}, which is not a path inside the theme's folder",
            serde_json::to_string(named).unwrap_or_default()
        ));
    }
    super::portable::path_refusal(named, named).map(|why| format!("icons: {why}"))
}

#[cfg(unix)]
mod unix {
    use std::io::Read;
    use std::os::fd::OwnedFd;
    use std::path::{Component, Path};

    use rustix::fs::{AtFlags, Dir, FileType, Mode, OFlags, openat, statat};
    use rustix::io::Errno;
    use serde_json::Value;

    use super::{Document, Kind, MOST_BYTES, not_inside};
    use crate::theme::THEME_JSON;

    /// A folder, held.
    pub(crate) type Held = OwnedFd;

    const DIRECTORY: OFlags = OFlags::RDONLY
        .union(OFlags::DIRECTORY)
        .union(OFlags::NOFOLLOW)
        .union(OFlags::CLOEXEC);

    const FILE: OFlags = OFlags::RDONLY
        .union(OFlags::NOFOLLOW)
        .union(OFlags::NONBLOCK)
        .union(OFlags::CLOEXEC);

    /// Whether an open was refused because what it met is a link or not a folder.
    fn linked(why: Errno) -> bool {
        matches!(why, Errno::LOOP | Errno::NOTDIR | Errno::MLINK)
    }

    /// **`root`, opened as the caller trusts it**: a level's root, or the folder that holds a
    /// folder the person named. `Ok(None)` when it is not there.
    pub(crate) fn root(root: &Path) -> Result<Option<Held>, String> {
        match rustix::fs::open(
            root,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        ) {
            Ok(fd) => Ok(Some(fd)),
            Err(Errno::NOENT) => Ok(None),
            Err(why) => Err(format!("purlis could not open {}: {why}", root.display())),
        }
    }

    /// **The folder `below` inside `from`**, `from` opened as the caller trusts it and every
    /// step below opened from the last refusing a link; `shown` names it in a sentence.
    /// `Ok(None)` when it is not there.
    pub(crate) fn walk(from: &Path, below: &Path, shown: &str) -> Result<Option<Held>, String> {
        if !below
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
        {
            return Err(format!("{shown} is not a folder purlis reads themes from"));
        }
        let Some(mut at) = root(from)? else {
            return Ok(None);
        };
        for step in below.components() {
            at = match openat(&at, step.as_os_str(), DIRECTORY, Mode::empty()) {
                Ok(fd) => fd,
                Err(Errno::NOENT) => return Ok(None),
                Err(why) if linked(why) => {
                    return Err(format!(
                        "{shown} is reached through a link or something that is not a folder, \
                         so no theme is read from it"
                    ));
                }
                Err(why) => return Err(format!("purlis could not open {shown}: {why}")),
            };
        }
        Ok(Some(at))
    }

    /// **The folder `name` inside `at`**, opened refusing a link; `shown` names it.
    pub(crate) fn folder(at: &Held, name: &str, shown: &str) -> Result<Held, String> {
        match openat(at, name, DIRECTORY, Mode::empty()) {
            Ok(fd) => Ok(fd),
            Err(Errno::NOENT) => Err(format!("there is no folder {shown}")),
            Err(why) if linked(why) => Err(format!(
                "{shown} is a link or not a folder, so it is not read"
            )),
            Err(why) => Err(format!("purlis could not open {shown}: {why}")),
        }
    }

    /// A second handle on the folder `at` holds.
    pub(crate) fn again(at: &Held, shown: &str) -> Result<Held, String> {
        rustix::io::dup(at).map_err(|why| format!("purlis could not open {shown}: {why}"))
    }

    /// **The theme folder's `theme.json` and the icon set it names**, each read from the
    /// folder's handle `dir`; `shown` names the folder.
    pub(crate) fn theme_json(dir: &Held, shown: &str) -> Result<Document, String> {
        let file = format!("{shown}/{THEME_JSON}");
        let text =
            text(dir, THEME_JSON, &file)?.ok_or_else(|| format!("{shown} has no {THEME_JSON}"))?;
        let document =
            crate::theme::load_text_document(&text).map_err(|why| format!("{file}: {why}"))?;
        let icons = match document.get("icons") {
            Some(Value::String(named)) => Some(icons(dir, shown, named)),
            _ => None,
        };
        Ok((document, icons))
    }

    /// The icon set `named` inside the folder `dir`, parsed, or why not: a relative path of
    /// plain names, each folder on the way opened from the last refusing a link.
    pub(crate) fn icons(dir: &Held, folder: &str, named: &str) -> Result<Value, String> {
        if let Some(why) = not_inside(named) {
            return Err(why);
        }
        let parts: Vec<_> = Path::new(named)
            .components()
            .map(|part| part.as_os_str())
            .collect();
        let Some((last, on_the_way)) = parts.split_last() else {
            return Err(format!("{folder} has no {named}"));
        };
        let mut at = again(dir, folder)?;
        for step in on_the_way {
            at = match openat(&at, *step, DIRECTORY, Mode::empty()) {
                Ok(fd) => fd,
                Err(Errno::NOENT) => return Err(format!("{folder} has no {named}")),
                Err(why) if linked(why) => {
                    return Err(format!(
                        "{folder}/{named} is reached through a link, so it is not read"
                    ));
                }
                Err(why) => return Err(format!("purlis could not read {folder}/{named}: {why}")),
            };
        }
        let file = format!("{folder}/{named}");
        let text = text(&at, *last, &file)?.ok_or_else(|| format!("{folder} has no {named}"))?;
        serde_json::from_str::<Value>(&text)
            .map_err(|why| format!("{file}: an icon set is JSON: {why}"))
    }

    /// **The text of the file `name` in `dir`**, opened refusing a link, a plain file under the
    /// cap, read from the descriptor that was checked; `None` when it is not there. `file`
    /// names it in a sentence.
    pub(crate) fn text(
        dir: &Held,
        name: impl rustix::path::Arg,
        file: &str,
    ) -> Result<Option<String>, String> {
        let Some(bytes) = bytes(dir, name, file, MOST_BYTES)? else {
            return Ok(None);
        };
        String::from_utf8(bytes)
            .map(Some)
            .map_err(|_| format!("purlis could not read {file}: it is not UTF-8 text"))
    }

    /// **The bytes of the file `name` in `dir`, at most `most`**, opened refusing a link and
    /// read only when the descriptor is a plain file; `None` when it is not there.
    pub(crate) fn bytes(
        dir: &Held,
        name: impl rustix::path::Arg,
        file: &str,
        most: u64,
    ) -> Result<Option<Vec<u8>>, String> {
        let fd = match openat(dir, name, FILE, Mode::empty()) {
            Ok(fd) => fd,
            Err(Errno::NOENT) => return Ok(None),
            Err(Errno::LOOP | Errno::MLINK) => {
                return Err(format!("{file} is a link, so it is not read"));
            }
            Err(why) => return Err(format!("purlis could not read {file}: {why}")),
        };
        let mut open = std::fs::File::from(fd);
        let found = open
            .metadata()
            .map_err(|why| format!("purlis could not read {file}: {why}"))?;
        if !found.file_type().is_file() {
            return Err(format!("{file} is not a plain file"));
        }
        let too_big = |bytes: u64| {
            format!("{file} is {bytes} bytes, and a theme's file is never larger than {most}")
        };
        if found.len() > most {
            return Err(too_big(found.len()));
        }
        // The file can grow after its size was asked: the read stops one byte past the cap.
        let mut bytes = Vec::new();
        (&mut open)
            .take(most + 1)
            .read_to_end(&mut bytes)
            .map_err(|why| format!("purlis could not read {file}: {why}"))?;
        if bytes.len() as u64 > most {
            return Err(too_big(bytes.len() as u64));
        }
        Ok(Some(bytes))
    }

    /// **Every entry of the folder `dir`** but `.` and `..`, by name in order, each with what
    /// it is, asked of the entry and never of what it leads to. A name that is not text is
    /// said as it is.
    pub(crate) fn entries(dir: &Held, shown: &str) -> Result<Vec<(String, Kind)>, String> {
        let failed = |why: Errno| format!("{shown} could not be read: {why}");
        let mut found = Vec::new();
        for entry in Dir::read_from(dir).map_err(failed)? {
            let entry = entry.map_err(failed)?;
            let bytes = entry.file_name().to_bytes();
            if bytes == b"." || bytes == b".." {
                continue;
            }
            let kind = match statat(dir, entry.file_name(), AtFlags::SYMLINK_NOFOLLOW)
                .map(|stat| FileType::from_raw_mode(stat.st_mode))
            {
                Ok(FileType::Symlink) => Kind::Link,
                Ok(FileType::Directory) => Kind::Folder,
                Ok(FileType::RegularFile) => Kind::File,
                _ => Kind::Other,
            };
            found.push((String::from_utf8_lossy(bytes).into_owned(), kind));
        }
        found.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(found)
    }
}

#[cfg(not(unix))]
mod elsewhere {
    //! **Nothing is read off unix**: every read opens a step refusing a link, which this build
    //! cannot express (ADR 0031).

    use std::path::Path;

    use serde_json::Value;

    use super::{Document, Kind};

    /// No folder is held here.
    pub(crate) type Held = std::convert::Infallible;

    fn unread(shown: &str) -> String {
        format!("{shown} is not read: theme folders are read on macOS and Linux")
    }

    pub(crate) fn root(root: &Path) -> Result<Option<Held>, String> {
        Err(unread(&root.display().to_string()))
    }

    pub(crate) fn walk(_root: &Path, _below: &Path, shown: &str) -> Result<Option<Held>, String> {
        Err(unread(shown))
    }

    pub(crate) fn folder(at: &Held, _name: &str, _shown: &str) -> Result<Held, String> {
        match *at {}
    }

    pub(crate) fn again(at: &Held, _shown: &str) -> Result<Held, String> {
        match *at {}
    }

    pub(crate) fn theme_json(dir: &Held, _shown: &str) -> Result<Document, String> {
        match *dir {}
    }

    pub(crate) fn icons(dir: &Held, _folder: &str, _named: &str) -> Result<Value, String> {
        match *dir {}
    }

    pub(crate) fn text(dir: &Held, _name: &str, _file: &str) -> Result<Option<String>, String> {
        match *dir {}
    }

    pub(crate) fn bytes(
        dir: &Held,
        _name: &str,
        _file: &str,
        _most: u64,
    ) -> Result<Option<Vec<u8>>, String> {
        match *dir {}
    }

    pub(crate) fn entries(dir: &Held, _shown: &str) -> Result<Vec<(String, Kind)>, String> {
        match *dir {}
    }
}
