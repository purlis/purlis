// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat's own temporary folder, kept across a quit for its wake line** (#5827, spec #5743).
//!
//! A Claude Code chat writes the output of its background shells and monitors into its own
//! temporary folder (#5675), which goes when the chat ends. The wake line quotes the last lines
//! of each and names the file the rest is in (#5751), so a quit that removed the folder left it
//! saying "output gone". At the quit, the folder of a chat that ended with work of its own is
//! **moved aside, on the same disk** ([`keep`]): what then drops the chat's folder, in the
//! window or in the session host, finds nothing at its path, and the record names the kept
//! folder and points each piece at it ([`rehome`]). It goes once the chat's next run ends, or
//! the chat is closed for good ([`remove`]).
//!
//! The folder kept is the one the chat's confinement made, never one a chat named: a piece's
//! output path is the chat's word, so it only ever follows a folder that was kept.

use std::path::{Path, PathBuf};

use crate::ownwork::pieces::Piece;

/// What a chat's own temporary folder is named with ([`crate::sandbox::claude_wrap::own_tmp`]).
const CHAT_FOLDER: &str = "charter-chat-";

/// What a kept folder's name ends with.
const KEPT: &str = "-kept";

/// **Keeps `tmp`, a chat's own temporary folder**: moves it to a name of its own beside it, so
/// what removes `tmp` as the chat ends removes nothing of it. The kept folder.
pub fn keep(tmp: &Path) -> std::io::Result<PathBuf> {
    let name = tmp
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| name.starts_with(CHAT_FOLDER))
        .ok_or_else(|| std::io::Error::other("not a chat's own temporary folder"))?;
    let parent = tmp
        .parent()
        .ok_or_else(|| std::io::Error::other("a temporary folder with no folder above it"))?;
    // A name nothing holds: the first that is free, never over another kept folder.
    for again in 0..100_u32 {
        let kept = if again == 0 {
            parent.join(format!("{name}{KEPT}"))
        } else {
            parent.join(format!("{name}-{again}{KEPT}"))
        };
        if std::fs::symlink_metadata(&kept).is_ok() {
            continue;
        }
        std::fs::rename(tmp, &kept)?;
        return Ok(kept);
    }
    Err(std::io::Error::other(
        "every name a kept folder could take is taken",
    ))
}

/// **Points each of `pieces` whose output was in `from` at the same file in `to`**, the folder
/// `from` was kept as. A path outside `from` is left as it was. `from` is matched as it was
/// named and as its real path, which on macOS differs (`/tmp` is `/private/tmp`).
pub fn rehome(pieces: &mut [Piece], from: &Path, to: &Path) {
    let real = from
        .parent()
        .and_then(|parent| std::fs::canonicalize(parent).ok())
        .zip(from.file_name())
        .map(|(parent, name)| parent.join(name));
    for piece in pieces {
        let Some(output) = piece.output.as_deref().map(PathBuf::from) else {
            continue;
        };
        let below = output.strip_prefix(from).ok().or_else(|| {
            real.as_deref()
                .and_then(|real| output.strip_prefix(real).ok())
        });
        if let Some(below) = below {
            piece.output = Some(to.join(below).display().to_string());
        }
    }
}

/// **Removes `kept`, a folder [`keep`] made**, and nothing else: a path whose name is not a
/// kept folder's, or that is a link, is left alone, since it is read from a record on disk.
pub fn remove(kept: &Path) {
    let named = kept
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with(CHAT_FOLDER) && name.ends_with(KEPT));
    let a_folder = std::fs::symlink_metadata(kept).is_ok_and(|meta| meta.file_type().is_dir());
    if named
        && a_folder
        && let Err(why) = std::fs::remove_dir_all(kept)
    {
        tracing::warn!(
            "purlis: a chat's kept temporary folder {} was not removed ({why})",
            kept.display()
        );
    }
}

#[cfg(test)]
#[path = "kept_tests.rs"]
mod tests;
