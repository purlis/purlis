// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A hibernated chat's kept screen** (SC-4b, #7631; spec #1391, *The snapshot*; ADR 0082 as
//! amended 2026-11-25): what a chat showed, its screen and its scrollback, as the app or the
//! operator hibernated it, which its pane shows read-only with Resume while it sleeps. The
//! conversation itself is the harness's, resumed by id; this is only a view of it.
//!
//! - **The engine's snapshot**, the escape sequences that draw it into a blank terminal, as a
//!   task's last screen is ([`crate::chatscreens`]), and written and read the one way
//!   ([`crate::keptscreen`]): whole through a temp file, `0600` in a `0700` folder, through no
//!   link.
//! - **At most [`MOST_BYTES`]** (spec D3). A longer one is taken again with fewer lines of
//!   history until it fits ([`fitted`]), so it is cut only at a line the engine drew and never
//!   inside an escape sequence, the oldest lines first.
//! - **Masked before it is written**, by the caller, with `secrets::handedto`, as a task's last
//!   screen is: this store never sees an unmasked screen, and one that could not be masked is
//!   never handed to it.
//! - **Never in the way of the hibernation**: a screen refused here leaves no file, not even the
//!   one kept before ([`keep`]), and the chat is hibernated with none. A file that is missing,
//!   not text, past the bound or a link reads as none kept ([`kept`]).
//! - **Removed** when the chat is closed or killed, or woken and drawn again ([`forget`]), and by
//!   the sweeps ([`sweep`], [`on_open`]): a screen no hibernated chat names, and one written
//!   [`crate::retention::KEEP_FOR`] or more before.
//! - **One writer**, the app's act in the chat's lane, so there is no lock, as there is no
//!   setting to race (`chatscreens`' lock is for its setting).
//!
//! **Tier:** Clone state (ADR 0069), never committed; not rebuildable: the process that held
//! the grid is gone. Losing it loses only the kept view.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// The store's folder under the state folder's `app/`.
pub const DIR_NAME: &str = "hibernated-screens";

/// The most bytes of one kept screen: 1 MiB (spec D3; binary units, ADR 0082).
pub const MOST_BYTES: usize = 1024 * 1024;

/// How young a killed write's temp file may be and still be one under way, which the sweep
/// leaves.
pub const A_WRITE_TAKES_AT_MOST: std::time::Duration = std::time::Duration::from_secs(5);

/// How many times [`fitted`] takes a screen again before it keeps none.
const MOST_LOOKS: usize = 16;

const SUFFIX: &str = ".screen";

// Within what every kept screen may be.
const _: () = assert!(MOST_BYTES <= crate::keptscreen::MOST_SCREEN_BYTES);

/// The store of the project at `root`.
pub fn dir(root: &Path) -> PathBuf {
    crate::names::state(root).join("app").join(DIR_NAME)
}

fn an_id(id: &str) -> bool {
    ulid::Ulid::from_string(id).is_ok_and(|read| read.to_string() == id)
}

/// The file of chat `id`'s screen, or `None` for an id that is not a ULID.
fn path_of(root: &Path, id: &str) -> Option<PathBuf> {
    an_id(id).then(|| dir(root).join(format!("{id}{SUFFIX}")))
}

/// **Keeps `screen`, already masked, as what hibernated chat `id` showed**, in the project at
/// `root`, replacing whole any kept before. Refused for an id that is not a ULID and for a
/// screen past [`MOST_BYTES`]; **refused or failed, no file is left for the chat**, the one kept
/// before included, so its pane never shows an older screen as this hibernation's.
pub fn keep(root: &Path, id: &str, screen: &str) -> std::io::Result<()> {
    let path = path_of(root, id)
        .ok_or_else(|| std::io::Error::other("a chat's screen is kept by its ULID"))?;
    let written = if screen.len() > MOST_BYTES {
        Err(std::io::Error::other("the screen is past 1 MiB"))
    } else {
        crate::keptscreen::made_private(&dir(root))
            .and_then(|()| crate::keptscreen::write(&[dir(root)], &path, screen))
    };
    if written.is_err() {
        forget(root, id);
    }
    written
}

/// **What hibernated chat `id` showed as it was hibernated**, where it was kept: read through
/// no link, at most [`MOST_BYTES`]. `None` where nothing was kept, and for a file that is not
/// text, past the bound or not a plain file: all read as *none kept*.
pub fn kept(root: &Path, id: &str) -> Option<String> {
    let path = path_of(root, id)?;
    let meta = std::fs::symlink_metadata(&path).ok()?;
    if !meta.is_file() || meta.len() > MOST_BYTES as u64 {
        return None;
    }
    crate::keptscreen::read(&[dir(root)], &path)
}

/// **Takes away chat `id`'s kept screen**: it was closed or killed, or woken and has drawn
/// again. A screen already gone, or one that could not be deleted, is said in the log at most.
pub fn forget(root: &Path, id: &str) {
    if let Some(path) = path_of(root, id) {
        match std::fs::remove_file(&path) {
            Err(why) if why.kind() != std::io::ErrorKind::NotFound => {
                tracing::info!("purlis: a hibernated chat's screen was not deleted ({why})");
            }
            _ => {}
        }
    }
}

/// Whether `name` is a temp file a write of one of this store's screens leaves when it is
/// killed before its rename ([`crate::rewrite::temp_of`]).
fn a_screen_s_temp(name: &str) -> bool {
    crate::rewrite::temp_of(name)
        .and_then(|temp| temp.of)
        .and_then(|of| of.strip_suffix(SUFFIX))
        .is_some_and(an_id)
}

/// **Takes away every kept screen whose chat `named` does not name, or written
/// [`crate::retention::KEEP_FOR`] or more before `now`**, in the project at `root`, and every
/// killed write's temp file older than [`A_WRITE_TAKES_AT_MOST`]. Nothing else in the folder is
/// touched. Answers how many went.
pub fn sweep(root: &Path, named: impl Fn(&str) -> bool, now: SystemTime) -> usize {
    let dir = dir(root);
    if !std::fs::symlink_metadata(&dir).is_ok_and(|meta| meta.is_dir()) {
        return 0;
    }
    let Ok(reader) = std::fs::read_dir(&dir) else {
        return 0;
    };
    let age = |meta: &std::fs::Metadata| {
        meta.modified()
            .ok()
            .and_then(|written| now.duration_since(written).ok())
    };
    reader
        .filter_map(Result::ok)
        .filter(|entry| {
            let Some(meta) = entry.metadata().ok().filter(std::fs::Metadata::is_file) else {
                return false;
            };
            let Some(name) = entry.file_name().into_string().ok() else {
                return false;
            };
            if let Some(id) = name.strip_suffix(SUFFIX).filter(|id| an_id(id)) {
                !named(id) || age(&meta).is_some_and(|age| age >= crate::retention::KEEP_FOR)
            } else {
                a_screen_s_temp(&name) && age(&meta).is_some_and(|age| age > A_WRITE_TAKES_AT_MOST)
            }
        })
        .filter(|entry| std::fs::remove_file(entry.path()).is_ok())
        .count()
}

/// **The sweep at a project's open** ([`sweep`]): until SC-4e (#7634) makes the hibernated
/// state durable, no chat is hibernated as a project opens, so no screen is named and every one
/// goes. SC-4e makes this read the reopen record's `hibernated` key.
pub fn on_open(root: &Path, now: SystemTime) -> usize {
    sweep(root, |_| false, now)
}

/// **The screen `take` gives, fitted to [`MOST_BYTES`]**: `take(None)` is the engine's whole
/// snapshot, and `take(Some(history))` the same with at most `history` lines above the screen,
/// the newest. A snapshot past the bound is taken again with fewer lines, each look's count
/// scaled from the last's by how far it was past, until it fits: so it is cut only where the
/// engine drew a line, never inside an escape sequence. `None` where it cannot be taken, where
/// the screen alone is past the bound, or after [`MOST_LOOKS`] looks.
pub fn fitted(mut take: impl FnMut(Option<u32>) -> Option<String>) -> Option<String> {
    let mut shown = take(None)?;
    if shown.len() <= MOST_BYTES {
        return Some(shown);
    }
    // Its lines are what it has of history and more: the first look asks for fewer.
    let mut history = u64::try_from(shown.matches('\n').count()).unwrap_or(u64::MAX);
    for _ in 0..MOST_LOOKS {
        if history == 0 {
            return None;
        }
        let size = u64::try_from(shown.len()).unwrap_or(u64::MAX).max(1);
        // A tenth under the bound, as lines are not all one length.
        let scaled = history.saturating_mul(MOST_BYTES as u64 / 10 * 9) / size;
        history = scaled.min(history - 1);
        shown = take(Some(u32::try_from(history).unwrap_or(u32::MAX)))?;
        if shown.len() <= MOST_BYTES {
            return Some(shown);
        }
    }
    None
}

#[cfg(test)]
mod tests;
