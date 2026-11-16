// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A project's shared compile cache** (#6388, #3560; ruling Q324): with
//! `[sandbox] compile-cache = "sccache"` in its committed `charter.toml`, a project's sandboxed
//! chats wrap rustc in sccache, so one branch folder's compile of a crate is the next one's hit
//! instead of each building from scratch.
//!
//! **Off unless the file says so.** The key is the future policy key `sandbox.compile-cache`
//! (Q307 Q6), whose default is `"none"`: no wrapper, and nothing added to what a chat writes.
//!
//! **Only the resolved sccache** ([`resolved`]). purlis never copies a `rustc-wrapper` from the
//! person's cargo config ([`super::caches::cargo_config`]). The one wrapper a chat is handed is
//! the `sccache` the person's `PATH` finds, as its real file, named `sccache`, outside every
//! folder a chat may write. A project whose machine has none gets no wrapper, and its chats build
//! as before.
//!
//! **One cache folder, in the project's cache home** ([`DIR`]): written by that project's
//! sandboxed chats and no one else, as its package caches are (D-1337-6). Compiled objects are
//! code a later build links, so the cache is never shared with another project or with the
//! person's own builds. The folder is one of the home's trees, so it is made and pinned by purlis
//! like the others, a `.git` at its top is denied, and a link planted for it is taken out. It
//! gives no write anywhere else.
//!
//! **A server of the chat's own, on a socket in the chat's own temp folder** ([`socket_in`]).
//! sccache's client hands each compile to a server, which runs the compiler. The default server
//! listens on loopback port 4226, where any program of this Mac may answer, the person's own
//! unsandboxed sccache among them. A chat that reached that one would have its compiles run
//! outside its sandbox. So purlis points the client at a unix socket in the chat's own temp folder
//! (`SCCACHE_SERVER_UDS`), and lets the chat bind and connect to that one
//! path. The chat starts its own server there, inside its own sandbox. No loopback port is opened,
//! and no chat reaches another chat's server.
//!
//! **Where purlis can grant it.** A Claude Code chat on macOS, whose sandbox grants a socket by
//! path (`network.allowUnixSockets`). Elsewhere the chat gets no wrapper, a declared gap: Claude
//! Code's sandbox on Linux reaches no unix socket, and a wrapped harness's profile does not name
//! the socket yet.

use std::path::{Path, PathBuf};

use super::Machine;

/// The `[sandbox]` key that turns a project's compile cache on.
pub const KEY: &str = "compile-cache";

/// The one value [`KEY`] takes that turns anything on, and the name of the program it wraps
/// rustc in.
pub const SCCACHE: &str = "sccache";

/// The value [`KEY`] has where the file says nothing: the default.
pub const NONE: &str = "none";

/// The cache's folder in the project's cache home ([`super::caches::CacheHome`]).
pub const DIR: &str = "sccache";

/// The name of the server's socket in the chat's own temp folder.
pub const SOCKET: &str = "sccache.sock";

/// What a project's `[sandbox]` asks of its compile cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CompileCache {
    /// No compile cache: the default.
    #[default]
    None,
    /// rustc wrapped in sccache, with a cache folder of the project's own.
    Sccache,
}

impl CompileCache {
    /// The value as the file writes it.
    pub fn word(self) -> &'static str {
        match self {
            Self::None => NONE,
            Self::Sccache => SCCACHE,
        }
    }

    /// What `written` asks for, or `None` where it is not a value of [`KEY`].
    pub fn of_word(written: &str) -> Option<Self> {
        [Self::None, Self::Sccache]
            .into_iter()
            .find(|one| one.word() == written)
    }
}

/// A project's sccache, resolved for its chats: the program and the cache folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sccache {
    /// The real file of the `sccache` program, outside every folder a chat may write.
    pub program: PathBuf,
    /// The cache folder in the project's cache home.
    pub dir: PathBuf,
}

impl Sccache {
    /// What points a chat's builds at this cache, with its server on `socket`: cargo's wrapper,
    /// sccache's folder, and the server's socket.
    pub fn env(&self, socket: &Path) -> Vec<(String, String)> {
        [
            ("RUSTC_WRAPPER", self.program.as_path()),
            ("SCCACHE_DIR", self.dir.as_path()),
            ("SCCACHE_SERVER_UDS", socket),
        ]
        .into_iter()
        .map(|(key, value)| (key.to_owned(), value.display().to_string()))
        .collect()
    }
}

/// Where a chat's sccache server listens: in the folder Claude Code roots the chat's commands'
/// `$TMPDIR` in, inside `tmp`, the chat's own temp folder (#5675). The chat writes there and no
/// other chat does, and the path stays short enough for a unix socket.
pub fn socket_in(tmp: &Path) -> PathBuf {
    tmp.join(format!("claude-{}", rustix::process::getuid().as_raw()))
        .join(SOCKET)
}

/// The `sccache` a chat on `machine` may be handed, where none of `writable` holds it: the first
/// on the person's `PATH`, as its real file. `None` where there is none, or where the one found
/// is not one a chat may run as its wrapper ([`resolved`]).
pub fn find(machine: &Machine, writable: &[PathBuf]) -> Option<PathBuf> {
    let dirs = crate::programs::search_dirs_from(
        machine.env.get("PATH").as_deref().map(std::ffi::OsStr::new),
        machine.home.as_deref(),
    );
    resolved(&crate::programs::find(SCCACHE, &dirs)?, writable)
}

/// `found` as the wrapper a chat may be handed, or `None`: it must be absolute, its real file
/// must be a file named `sccache`, and neither it nor its real file may lie in any of
/// `writable`, the folders a chat may write. A chat must never point its builds' wrapper at a
/// program of its own, nor at one it could change.
pub fn resolved(found: &Path, writable: &[PathBuf]) -> Option<PathBuf> {
    if !found.is_absolute() {
        return None;
    }
    let real = found.canonicalize().ok()?;
    if !real.is_file() || real.file_name()? != SCCACHE {
        return None;
    }
    let inside = |path: &Path| {
        writable.iter().any(|dir| {
            path.starts_with(dir) || dir.canonicalize().is_ok_and(|dir| path.starts_with(dir))
        })
    };
    (!inside(found) && !inside(&real)).then_some(real)
}

/// The folders a chat in the project at `root` may write, as far as the wrapper's check asks:
/// the project, its cache home, the temp folders, and every folder a person let its chats write.
pub fn writable_for(machine: &Machine, root: &Path, cache_home: &Path) -> Vec<PathBuf> {
    let env: Vec<(String, String)> = machine
        .env
        .get("TMPDIR")
        .map(|tmp| vec![("TMPDIR".to_owned(), tmp)])
        .unwrap_or_default();
    let mut out = vec![root.to_path_buf(), cache_home.to_path_buf()];
    out.extend(super::program::temp_roots(&env));
    out.extend(super::local::granted_writes(root));
    out
}
