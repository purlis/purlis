// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Where the session host listens on this device (ADR 0068 §1): a socket in a folder that is
//! this user's alone, under the per-user runtime or temporary directory.
//!
//! - **macOS:** `purlis/purlisd.sock` in this user's own temporary directory, as the system
//!   names it for the user (`confstr(_CS_DARWIN_USER_TEMP_DIR)`, read through
//!   `/usr/bin/getconf DARWIN_USER_TEMP_DIR`), never `$TMPDIR`.
//! - **Linux:** `$XDG_RUNTIME_DIR/purlis/purlisd.sock`, where that variable names an absolute
//!   directory this user owns that nobody else may enter.
//! - **Otherwise:** `/tmp/purlis-<uid>/purlisd.sock`. The uid keeps two accounts apart in a
//!   shared `/tmp`; the folder's `0700`, checked by the host as it is made, keeps others out.
//!
//! A base that is not this user's alone is passed over for `/tmp`, so a variable a process
//! inherits never moves the socket somewhere another host would not look. The window,
//! `purlis serve` and a chat's sandbox all ask [`socket_for`], so they name one place; the
//! window makes the folder ([`make_the_folder`]) before any chat starts, so each sandbox
//! denies a folder that is there. The credentials beside it are in the machine store
//! ([`crate::machine::charterd`]).

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// The socket's file name.
pub const SOCKET: &str = "purlisd.sock";

/// Where the host's socket is, given the per-user base this platform names (`base`) and this
/// user's uid: `purlis/` in the base when it is an absolute directory, not a link, owned by
/// `uid` and closed to everyone else; else `/tmp/purlis-<uid>/`.
#[cfg(unix)]
pub fn socket_in(base: Option<&Path>, uid: u32) -> PathBuf {
    let folder = match base.filter(|base| private_to(base, uid)) {
        Some(base) => base.join("purlis"),
        None => Path::new("/tmp").join(format!("purlis-{uid}")),
    };
    folder.join(SOCKET)
}

/// Whether `base` is an absolute directory, not a link, owned by `uid`, that nobody else may
/// read, write or enter.
#[cfg(unix)]
fn private_to(base: &Path, uid: u32) -> bool {
    use std::os::unix::fs::MetadataExt;
    base.is_absolute()
        && std::fs::symlink_metadata(base)
            .is_ok_and(|found| found.is_dir() && found.uid() == uid && found.mode() & 0o077 == 0)
}

/// Where the host's socket is for this user. `runtime` is `$XDG_RUNTIME_DIR` as the caller's
/// environment has it, read on Linux alone; macOS asks the system for the user's own
/// temporary directory, whatever the environment says.
#[cfg(unix)]
pub fn socket_for(runtime: Option<&OsStr>) -> PathBuf {
    #[cfg(target_os = "macos")]
    let base = {
        let _ = runtime;
        users_own_temporary_directory()
    };
    #[cfg(not(target_os = "macos"))]
    let base = runtime.map(PathBuf::from);
    socket_in(base.as_deref(), purlis_same_user::Uid::effective().as_raw())
}

/// Where the host's socket is for this process.
#[cfg(unix)]
pub fn here() -> PathBuf {
    socket_for(std::env::var_os("XDG_RUNTIME_DIR").as_deref())
}

/// Makes the socket's folder this user's alone now (`purlis_same_user::private_directory`),
/// for the window to call before any chat starts, so every sandbox denies a folder that is
/// there.
#[cfg(unix)]
pub fn make_the_folder(socket: &Path) -> std::io::Result<()> {
    let folder = socket.parent().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "a socket with no folder")
    })?;
    purlis_same_user::private_directory(folder)
}

/// This user's own temporary directory as macOS names it (`_CS_DARWIN_USER_TEMP_DIR`): asked
/// once of `/usr/bin/getconf`, by its absolute path and with no environment, since reading it
/// in-process would take an `unsafe` call. `None` when it does not answer.
#[cfg(target_os = "macos")]
fn users_own_temporary_directory() -> Option<PathBuf> {
    static ASKED: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();
    ASKED
        .get_or_init(|| {
            let out = crate::forklock::output(
                std::process::Command::new("/usr/bin/getconf")
                    .arg("DARWIN_USER_TEMP_DIR")
                    .env_clear(),
            )
            .ok()
            .filter(|out| out.status.success())?;
            let text = String::from_utf8(out.stdout).ok()?;
            let named = text.trim_end_matches('\n');
            (!named.is_empty()).then(|| PathBuf::from(named.trim_end_matches('/')))
        })
        .clone()
}

#[cfg(test)]
mod tests;
