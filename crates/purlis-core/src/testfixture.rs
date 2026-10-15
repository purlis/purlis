// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Test fixtures tracked under neutral names (#2089), copied to their real names at test time.
//!
//! A file whose name a sandboxed chat may never write (an agent's `.claude/settings.json`, an
//! `opencode.json`) is tracked as `<name>.fixture`, so no checkout of this repository has to
//! write one: a chat's sandbox would stop that checkout partway, and the clone guard refuses
//! it. A test that needs the real name copies the fixture with [`copy`], which takes the
//! suffix off every name on the way. The guard test
//! `no_tracked_file_is_one_a_sandboxed_chat_may_not_write` holds the tree to it.

use std::ffi::{OsStr, OsString};
use std::path::{Component, Path, PathBuf};

/// The suffix a fixture's tracked name carries.
pub const SUFFIX: &str = ".fixture";

/// `name` as the test sees it: [`SUFFIX`] taken off where it ends with it.
pub fn real_name(name: &OsStr) -> OsString {
    match name.to_str().and_then(|text| text.strip_suffix(SUFFIX)) {
        Some(real) if !real.is_empty() => OsString::from(real),
        _ => name.to_os_string(),
    }
}

/// `rel`, each of its names as the test sees it ([`real_name`]).
pub fn real_path(rel: &Path) -> PathBuf {
    rel.components()
        .map(|part| match part {
            Component::Normal(name) => PathBuf::from(real_name(name)),
            other => PathBuf::from(other.as_os_str()),
        })
        .collect()
}

/// Copies the fixture tree `from` to `to`, every name as the test sees it, files with their
/// modes and links as links.
pub fn copy(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let src = entry.path();
        let dst = to.join(real_name(&entry.file_name()));
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            std::os::unix::fs::symlink(std::fs::read_link(&src)?, &dst)?;
        } else if kind.is_dir() {
            copy(&src, &dst)?;
        } else {
            std::fs::copy(&src, &dst)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
