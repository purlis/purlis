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

/// **What one profile is drawn with**: the glyph or the SVG it is drawn with, if any, and the
/// monogram that stands in for either.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct HarnessIcon {
    /// A glyph of [`crate::theme::vocabulary::GLYPHS`].
    pub glyph: Option<&'static str>,
    /// The person's own SVG, as shapes.
    pub svg: Option<SafeSvg>,
    /// The harness kind's monogram: `C`, `OC`, `CX`. Always present.
    pub monogram: String,
}

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
    let unread = |_| Unfit("the icon file is not a plain file an icon may be read from");
    let held = crate::theme::opened::walk(
        &crate::machine::dir(config_root),
        Path::new(FOLDER),
        "purlis's icons folder",
    )
    .map_err(unread)?
    .ok_or(Unfit("the icon file is not there"))?;
    let bytes = sole_bytes(&held, name)
        .map_err(unread)?
        .ok_or(Unfit("the icon file is not there"))?;
    safesvg::read(&bytes)
}

/// **The bytes of the icon file `name` in `dir`**, as [`crate::theme::opened::bytes`] reads a
/// theme's file (opened refusing a link and without waiting, read only from a descriptor that is
/// a plain file under the cap), and refused too where the file has more than one name.
#[cfg(unix)]
fn sole_bytes(dir: &crate::theme::opened::Held, name: &str) -> Result<Option<Vec<u8>>, String> {
    use std::io::Read;
    use std::os::unix::fs::MetadataExt;

    use rustix::fs::{Mode, OFlags, openat};
    use rustix::io::Errno;

    let flags = OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC;
    let fd = match openat(dir, name, flags, Mode::empty()) {
        Ok(fd) => fd,
        Err(Errno::NOENT) => return Ok(None),
        Err(why) => return Err(why.to_string()),
    };
    let mut open = std::fs::File::from(fd);
    let found = open.metadata().map_err(|why| why.to_string())?;
    if !found.file_type().is_file() || found.nlink() != 1 || found.len() > safesvg::MOST_BYTES {
        return Err("not a plain file of an icon's size with one name".to_owned());
    }
    let mut bytes = Vec::new();
    (&mut open)
        .take(safesvg::MOST_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|why| why.to_string())?;
    if bytes.len() as u64 > safesvg::MOST_BYTES {
        return Err("larger than an icon may be".to_owned());
    }
    Ok(Some(bytes))
}

/// Off unix nothing is read ([`crate::theme::opened`]).
#[cfg(not(unix))]
fn sole_bytes(dir: &crate::theme::opened::Held, _name: &str) -> Result<Option<Vec<u8>>, String> {
    match *dir {}
}

/// **What the profile `name` of `kind` is drawn with**, `icon` what its definition names, and
/// `config_root` the config home its own SVG is read from.
pub fn of(name: &str, kind: &str, icon: Option<&str>, config_root: Option<&Path>) -> HarnessIcon {
    let monogram = monogram(kind);
    let drawn = |glyph, svg| HarnessIcon {
        glyph,
        svg,
        monogram: monogram.clone(),
    };
    match icon.map(named) {
        Some(Named::Glyph(glyph)) => drawn(Some(glyph), None),
        Some(Named::File(file)) => drawn(
            None,
            config_root.and_then(|root| read_file(root, file).ok()),
        ),
        Some(Named::Neither) => drawn(None, None),
        // A profile that names no icon: the built-in's own row, or one standing in its place
        // under its name, is drawn as its harness; any other shows the monogram.
        None if name == kind => drawn(glyph_of_kind(kind), None),
        None => drawn(None, None),
    }
}

#[cfg(test)]
mod tests;
