// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A terminal screen this machine keeps, written and read the one way** (#6214's review): what
//! a grade's chat showed ([`crate::skills::grades::kept`]) and what a dispatched task showed as it
//! closed ([`crate::chatscreens`]). Each store names its own folders and files; this writes and
//! reads one of them.
//!
//! - **Written whole or not at all**: to a temp file beside it, then renamed into place
//!   ([`crate::rewrite::replace`]), private (0600), through no link. A reader never sees half a
//!   screen, and a screen kept before stays as it was where a write fails.
//! - **Every folder on the way made private and checked to be a folder of its own** before the
//!   next is made inside it.
//! - **Read only as the app wrote it**: each folder a folder (never a link), the file a plain
//!   file of at most [`MOST_SCREEN_BYTES`], opened following no link.

use std::io::Read as _;
use std::path::{Path, PathBuf};

/// The most bytes of one screen kept: a screen past it is not kept, since cutting it would cut
/// the sequences that draw it.
pub const MOST_SCREEN_BYTES: usize = 4 * 1024 * 1024;

/// `dir` made where it is not, private, and refused where it is not a folder of its own.
pub(crate) fn made_private(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    if !std::fs::symlink_metadata(dir)?.is_dir() {
        return Err(std::io::Error::other("the folder is not a folder"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

/// **Keeps `screen` at `path`**, each of `on_the_way` (outermost first, `path`'s own folder
/// last) made private first. Refused for a screen past [`MOST_SCREEN_BYTES`].
pub(crate) fn write(on_the_way: &[PathBuf], path: &Path, screen: &str) -> std::io::Result<()> {
    if screen.len() > MOST_SCREEN_BYTES {
        return Err(std::io::Error::other("the screen is too big to keep"));
    }
    for dir in on_the_way {
        made_private(dir)?;
    }
    let dir = path
        .parent()
        .ok_or_else(|| std::io::Error::other("a screen is kept in a folder"))?;
    crate::rewrite::replace(dir, path, screen.as_bytes(), crate::rewrite::Mode::Private)
}

/// **The screen kept at `path`**, where every one of `on_the_way` is a folder of its own and the
/// file is a plain one of at most [`MOST_SCREEN_BYTES`]: read through no link. `None` otherwise.
pub(crate) fn read(on_the_way: &[PathBuf], path: &Path) -> Option<String> {
    if !on_the_way
        .iter()
        .all(|dir| std::fs::symlink_metadata(dir).is_ok_and(|meta| meta.is_dir()))
    {
        return None;
    }
    let meta = std::fs::symlink_metadata(path).ok()?;
    if !meta.is_file() || meta.len() > MOST_SCREEN_BYTES as u64 {
        return None;
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    let file = crate::contain::nofollow(&mut options).open(path).ok()?;
    let mut text = String::new();
    file.take(MOST_SCREEN_BYTES as u64)
        .read_to_string(&mut text)
        .ok()?;
    Some(text)
}
