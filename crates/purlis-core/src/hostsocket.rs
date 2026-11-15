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

/// Where the host's socket is for this process: the place [`socket_for`] names, under the name
/// [`named_for`] gives this process's config home.
///
/// A binary installed on a runner as one version of several (`<config>/server/<version>/purlis`,
/// RR-14, #723) has a socket of its version's own, [`of_version`]: its host, its bridge and
/// every client it runs there name that one, so a new version's host serves beside an old one,
/// which keeps its chats until each ends (ADR 0068 §8).
#[cfg(unix)]
pub fn here() -> PathBuf {
    let config = crate::machine::config_root_unheld();
    let socket = here_for(
        std::env::var_os("XDG_RUNTIME_DIR").as_deref(),
        config.as_deref(),
    );
    let version = config.as_deref().and_then(|config| {
        crate::runnerinstall::version_of_binary(config, &std::env::current_exe().ok()?)
    });
    match version {
        Some(version) => of_version(&socket, &version),
        None => socket,
    }
}

/// **`socket`, named for the installed version `version`**: `purlisd-v<8 hex>.sock` for
/// `purlisd.sock`, in the same folder, the hex the start of the SHA-256 of the version. Its lock
/// and link socket are named from it as any socket's are, so they are the version's own too.
/// Not the version itself, which may be 64 characters ([`crate::runnerinstall::a_version`]):
/// a socket's whole path must fit in 104 bytes on macOS, and the user's own temporary directory
/// there takes half of them.
#[must_use]
pub fn of_version(socket: &Path, version: &str) -> PathBuf {
    let stem = socket
        .file_name()
        .and_then(OsStr::to_str)
        .and_then(|name| name.strip_suffix(".sock"))
        .unwrap_or("purlisd");
    socket.with_file_name(format!("{stem}-{}.sock", version_key(version)))
}

/// **What names the installed version `version` where its whole name may not fit**: `v` and the
/// first 8 hex digits of its SHA-256. Its socket ([`of_version`]) and the service that keeps its
/// host running on a runner (`crate::runnerhost`) are named by it.
#[must_use]
pub fn version_key(version: &str) -> String {
    use sha2::{Digest as _, Sha256};
    let digest = Sha256::digest(version.as_bytes());
    let short: String = digest[..4]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    format!("v{short}")
}

/// Where the session host of the config home `config` listens for this user, with `runtime`
/// as [`socket_for`] reads it: for a test that names both, and must find the host its child
/// serves at.
#[cfg(unix)]
pub fn here_for(runtime: Option<&OsStr>, config: Option<&Path>) -> PathBuf {
    let default =
        crate::machine::rooted(None, std::env::var_os("XDG_CONFIG_HOME"), dirs::home_dir());
    named_for(&socket_for(runtime), config, default.as_deref())
}

/// **The socket's name for the config home `config`** (#2114): `socket` as it is where `config`
/// is the one a person's purlis finds by itself (`default`), or none is known, so every build
/// finds its host where the last one listened; otherwise `purlisd-<key>.sock` in the same
/// folder, `<key>` the first 16 hex digits of the SHA-256 of `config` resolved through its
/// nearest folder that exists, so one config home spelled two ways, or named before it is made,
/// is one. So another
/// purlis on this device with a config home of its own, whose credentials are its own (a
/// scenario run, a development build beside an installed one), has a session host of its own,
/// and one host still serves every project of a config home (ADR 0068 §1).
#[cfg(unix)]
pub fn named_for(socket: &Path, config: Option<&Path>, default: Option<&Path>) -> PathBuf {
    use sha2::{Digest, Sha256};
    let config = config.map(resolved);
    let default = default.map(resolved);
    let Some(config) = config.filter(|config| Some(config) != default.as_ref()) else {
        return socket.to_path_buf();
    };
    let digest = Sha256::digest(config.as_os_str().as_encoded_bytes());
    let key: String = digest[..8]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    socket.with_file_name(format!("purlisd-{key}.sock"))
}

/// `path` resolved through its nearest folder that exists, with the rest as written: a config
/// home not made yet names the socket it will once it is.
#[cfg(unix)]
fn resolved(path: &Path) -> PathBuf {
    let mut rest = Vec::new();
    let mut at = path;
    loop {
        if let Ok(found) = std::fs::canonicalize(at) {
            return rest
                .iter()
                .rev()
                .fold(found, |whole, part| whole.join(part));
        }
        match (at.parent(), at.file_name()) {
            (Some(parent), Some(name)) => {
                rest.push(name.to_os_string());
                at = parent;
            }
            _ => return path.to_path_buf(),
        }
    }
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
