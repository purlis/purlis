// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What a profile is drawn with** (#2922, ruling #2866): on its option in Start a chat, in
//! Settings, and wherever else a harness is named.
//!
//! - **A built-in harness has a glyph purlis draws** (·1), never a vendor's mark: a spark for
//!   Claude Code, braces for opencode, a terminal for Codex ([`glyph_of_kind`]). The glyphs are
//!   part of the theme's icon set ([`crate::theme::vocabulary::GLYPHS`], ·4), so an icon pack
//!   redraws them; the window draws its own where the theme draws none.
//! - **A profile can name its icon** (·2): `icon = "spark"` names a glyph, and
//!   `icon = "mine.svg"` names an SVG file in the `icons` folder of purlis's directory in the
//!   person's config home ([`folder`]), which is never shipped or committed. The file is read as
//!   untrusted input ([`crate::safesvg`]).
//! - **Never a blank** (·3): a profile that names no icon, or one purlis cannot draw, shows the
//!   monogram of its harness kind ([`monogram`]). The profile that stands for a built-in harness
//!   (the one named after its kind) is drawn with that harness's glyph.
//!
//! Which glyphs the theme in force draws is the window's to know: the core says the glyph's
//! name, and the window draws the theme's, or its own, or the monogram for a name it has none
//! for.

use std::path::{Path, PathBuf};

use crate::safesvg::{self, SafeSvg, Unfit};

/// The folder of purlis's directory in the config home that a profile's own SVG icons are in.
pub const FOLDER: &str = "icons";

/// The longest name a profile's `icon` may be.
pub const MOST_NAME: usize = 128;

/// What a profile's `icon` names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Named<'a> {
    /// A glyph of the icon set.
    Glyph(&'static str),
    /// An SVG file in [`folder`], by its file name.
    File(&'a str),
    /// Nothing purlis draws: an unknown glyph (a newer purlis may have it) or a file name
    /// purlis does not read.
    Neither,
}

/// What `icon` names.
pub fn named(icon: &str) -> Named<'_> {
    if let Some(glyph) = crate::theme::vocabulary::GLYPHS
        .iter()
        .copied()
        .find(|it| *it == icon)
    {
        return Named::Glyph(glyph);
    }
    if file_name_ok(icon) {
        return Named::File(icon);
    }
    Named::Neither
}

/// Whether `name` is the name of an SVG file directly in [`folder`]: letters, digits, `.`, `_`
/// and `-`, starting with a letter or digit, ending `.svg`, and no path in it.
pub fn file_name_ok(name: &str) -> bool {
    let mut chars = name.chars();
    name.len() <= MOST_NAME
        && name.to_ascii_lowercase().ends_with(".svg")
        && name.len() > ".svg".len()
        && chars.next().is_some_and(|c| c.is_ascii_alphanumeric())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        && !name.contains("..")
}

/// The glyph a built-in harness kind is drawn with.
pub fn glyph_of_kind(kind: &str) -> Option<&'static str> {
    match kind {
        "claude" => Some("spark"),
        "opencode" => Some("braces"),
        "codex" => Some("terminal"),
        _ => None,
    }
}

/// **A harness kind's monogram**: `C` for Claude Code, `OC` for opencode, `CX` for Codex, and
/// the first letter of any other kind's name, upper case.
pub fn monogram(kind: &str) -> String {
    match kind {
        "claude" => "C".to_owned(),
        "opencode" => "OC".to_owned(),
        "codex" => "CX".to_owned(),
        _ => kind
            .chars()
            .find(char::is_ascii_alphanumeric)
            .map_or_else(|| "?".to_owned(), |c| c.to_ascii_uppercase().to_string()),
    }
}

/// The folder a profile's own SVG icons are in, under the config home `config_root`.
pub fn folder(config_root: &Path) -> PathBuf {
    crate::machine::dir(config_root).join(FOLDER)
}

/// **Reads the icon file `name`** from [`folder`], from handles as a theme's files are read
/// ([`crate::theme::opened`]): the config home's folder opened as trusted, `icons` opened from it
/// refusing a link, and the file opened from that handle refusing a link and without waiting,
/// then read only when the descriptor is a plain file of one name, at most
/// [`safesvg::MOST_BYTES`]. Read
/// as shapes. Off unix nothing is read, and the profile shows its monogram.
pub fn read_file(config_root: &Path, name: &str) -> Result<SafeSvg, Unfit> {
    if !file_name_ok(name) {
        return Err(Unfit(
            "the icon is not the name of an .svg file in purlis's icons folder",
        ));
    }
    let unread = |_| Unfit(NOT_PLAIN);
    let held = crate::theme::opened::walk(
        &crate::machine::dir(config_root),
        Path::new(FOLDER),
        "purlis's icons folder",
    )
    .map_err(unread)?
    .ok_or(Unfit("the icon file is not there"))?;
    let bytes = sole_bytes(&held, name)?.ok_or(Unfit("the icon file is not there"))?;
    safesvg::read(&bytes)
}

/// **The bytes of the icon file `name` in `dir`**, as [`crate::theme::opened::bytes`] reads a
/// theme's file (opened refusing a link and without waiting, read only from a descriptor that is
/// a plain file under the cap), and refused too where the file has more than one name.
#[cfg(unix)]
fn sole_bytes(dir: &crate::theme::opened::Held, name: &str) -> Result<Option<Vec<u8>>, Unfit> {
    use std::io::Read;
    use std::os::unix::fs::MetadataExt;

    use rustix::fs::{Mode, OFlags, openat};
    use rustix::io::Errno;

    let flags = OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC;
    let fd = match openat(dir, name, flags, Mode::empty()) {
        Ok(fd) => fd,
        Err(Errno::NOENT) => return Ok(None),
        Err(_) => return Err(Unfit(NOT_PLAIN)),
    };
    let mut open = std::fs::File::from(fd);
    let found = open.metadata().map_err(|_| Unfit(NOT_PLAIN))?;
    if !found.file_type().is_file() || found.nlink() != 1 {
        return Err(Unfit(NOT_PLAIN));
    }
    if found.len() > safesvg::MOST_BYTES {
        return Err(Unfit(TOO_LARGE));
    }
    let mut bytes = Vec::new();
    (&mut open)
        .take(safesvg::MOST_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Unfit(NOT_PLAIN))?;
    if bytes.len() as u64 > safesvg::MOST_BYTES {
        return Err(Unfit(TOO_LARGE));
    }
    Ok(Some(bytes))
}

/// Off unix nothing is read ([`crate::theme::opened`]).
#[cfg(not(unix))]
fn sole_bytes(dir: &crate::theme::opened::Held, _name: &str) -> Result<Option<Vec<u8>>, Unfit> {
    match *dir {}
}

/// Why an icon file that cannot be read as a plain file of one name draws nothing.
const NOT_PLAIN: &str = "the icon file is not a plain file an icon may be read from";

/// Why an icon file past the cap draws nothing.
pub const TOO_LARGE: &str = "the icon file is larger than an icon may be";

/// Why an `icon` that is neither a glyph nor a file's name draws nothing.
pub const NO_SUCH_ICON: &str = "purlis draws no glyph by that name, and it is not the name of an .svg file in its icons folder";

/// Why an icon file cannot be looked for at all.
pub const NO_CONFIG_HOME: &str = "purlis has no config folder here to read the icon file from";

/// **Why a profile's icon fell back to its monogram** (#3137), `icon` what its definition names and
/// `config_root` the config home its own SVG is read from: `None` where it draws what it names or
/// names nothing. Always one of the sentences purlis wrote ([`Unfit`] holds only those), so nothing
/// from the file is repeated. Reads the file alone, past any answer's budget.
pub fn fell_back(icon: Option<&str>, config_root: Option<&Path>) -> Option<&'static str> {
    match named(icon?) {
        Named::Glyph(_) => None,
        Named::Neither => Some(NO_SUCH_ICON),
        Named::File(file) => match config_root {
            None => Some(NO_CONFIG_HOME),
            Some(root) => read_file(root, file).err().map(|unfit| unfit.0),
        },
    }
}

/// The most one answer's own SVGs may come to, all of them together: an answer that lists many
/// profiles (Start a chat's options) holds every person's icon in it to this, however many rows
/// name one. The window holds the same budget across a list (`HarnessIcon.tsx`).
pub const MOST_ANSWER: usize = 4 * safesvg::MOST_OUTPUT;

/// The most icon files one answer reads, kept or not.
pub const MOST_FILES: usize = 64;

/// The least an icon comes to: a view box and one shape. Once less than this is left of an
/// answer's budget, nothing more is read.
const LEAST_ICON: usize = "0 0 1 1".len() + "path".len() + "dM0 0".len();

/// **What one profile of an answer is drawn with**: its glyph, the file name of its own SVG in
/// the answer's [`Answer::svgs`], and the monogram that stands in for either.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Drawn {
    pub glyph: Option<&'static str>,
    /// The key of its SVG in [`Answer::svgs`].
    pub own: Option<String>,
    pub monogram: String,
}

/// **The icons of one answer that lists profiles**: each SVG a profile names read once, however
/// many rows name it, and sent once, by its file name; all of them together held to
/// [`MOST_ANSWER`]. A row whose file is past the budget, or cannot be drawn, gets the monogram.
#[derive(Debug, Default)]
pub struct Answer {
    /// The SVGs the answer carries, by file name.
    pub svgs: std::collections::BTreeMap<String, SafeSvg>,
    /// Every file asked for, read or not, by its name in lower case, to the key its SVG is under
    /// when it was kept: so none is read twice, whatever case a row spells it in.
    asked: std::collections::BTreeMap<String, Option<String>>,
    /// What [`Self::svgs`] come to.
    spent: usize,
}

impl Answer {
    /// What the profile `name` of `kind` is drawn with in this answer, `icon` what its
    /// definition names and `config_root` the config home its own SVG is read from.
    pub fn drawn(
        &mut self,
        name: &str,
        kind: &str,
        icon: Option<&str>,
        config_root: Option<&Path>,
    ) -> Drawn {
        let monogram = monogram(kind);
        let (glyph, own) = match icon.map(named) {
            Some(Named::Glyph(glyph)) => (Some(glyph), None),
            Some(Named::File(file)) => (None, self.own(file, config_root)),
            Some(Named::Neither) => (None, None),
            None if name == kind => (glyph_of_kind(kind), None),
            None => (None, None),
        };
        Drawn {
            glyph,
            own,
            monogram,
        }
    }

    /// The key of `file` in [`Self::svgs`], read the first time it is asked for in any case, and
    /// kept while the answer's budget holds it. No file is read once [`MOST_FILES`] have been, or
    /// once what is left of the budget could hold no icon.
    fn own(&mut self, file: &str, config_root: Option<&Path>) -> Option<String> {
        let folded = file.to_ascii_lowercase();
        if let Some(kept) = self.asked.get(&folded) {
            return kept.clone();
        }
        if self.asked.len() >= MOST_FILES || MOST_ANSWER - self.spent < LEAST_ICON {
            return None;
        }
        let kept = config_root
            .and_then(|root| read_file(root, file).ok())
            .filter(|svg| self.spent + svg.size() <= MOST_ANSWER)
            .map(|svg| {
                self.spent += svg.size();
                self.svgs.insert(file.to_owned(), svg);
                file.to_owned()
            });
        self.asked.insert(folded, kept.clone());
        kept
    }

    /// What the answer's SVGs come to.
    pub fn spent(&self) -> usize {
        self.spent
    }
}

#[cfg(test)]
mod tests;
