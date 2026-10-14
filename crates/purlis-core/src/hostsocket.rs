// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Where the session host listens on this device (ADR 0068 §1): a socket in a folder that is
//! this user's alone, under the per-user runtime directory.
//!
//! - **Linux:** `$XDG_RUNTIME_DIR/purlis/purlisd.sock`, the standard per-user `0700` directory
//!   for a socket.
//! - **macOS**, and Linux without a runtime directory: `purlis-<uid>/purlisd.sock` in the
//!   temporary directory, which on macOS is already per user. The uid in the folder's name keeps
//!   two accounts apart where it is shared (`/tmp`); the folder's `0700`, checked by the host as
//!   it is made, is what keeps others out.
//!
//! The credentials beside it are in the machine store ([`crate::machine::charterd`]), which a
//! chat's sandbox denies; the socket's folder is denied to a chat as well
//! ([`crate::sandbox`]'s human powers). The window, `purlis serve` and the sandbox all ask this
//! module, so they name one place.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// The socket's file name.
pub const SOCKET: &str = "purlisd.sock";

/// Where the host's socket is, given the runtime directory's variable (`$XDG_RUNTIME_DIR`),
/// the temporary directory's (`$TMPDIR`), and this user's uid. A runtime directory that is not
/// an absolute path to a directory is passed over, and a temporary directory that is not an
/// absolute path is `/tmp`.
pub fn socket_in(runtime: Option<&OsStr>, temporary: Option<&OsStr>, uid: u32) -> PathBuf {
    let folder = match runtime.map(Path::new) {
        Some(runtime) if runtime.is_absolute() && runtime.is_dir() => runtime.join("purlis"),
        _ => temporary
            .map(Path::new)
            .filter(|temporary| temporary.is_absolute())
            .unwrap_or(Path::new("/tmp"))
            .join(format!("purlis-{uid}")),
    };
    folder.join(SOCKET)
}

/// [`socket_in`], with the two variables read through `var`: this process's environment, or
/// the one a chat's sandbox is compiled against, so both name the same folder.
#[cfg(unix)]
pub fn socket_for(var: impl Fn(&str) -> Option<std::ffi::OsString>) -> PathBuf {
    socket_in(
        var("XDG_RUNTIME_DIR").as_deref(),
        var("TMPDIR").as_deref(),
        purlis_same_user::Uid::effective().as_raw(),
    )
}

/// Where the host's socket is for this process.
#[cfg(unix)]
pub fn here() -> PathBuf {
    socket_for(|name| std::env::var_os(name))
}

#[cfg(test)]
mod tests;
