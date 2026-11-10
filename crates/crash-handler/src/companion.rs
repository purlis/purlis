// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0
//
// The shape of this server (a minidumper `Server` that writes one dump and exits, and exits too
// when its one client goes) follows minidumper-child's `server.rs`
// (https://github.com/timfish/minidumper-child, at 7500498), copyright the minidumper-child
// authors, licensed MIT OR Apache-2.0, and used here under Apache-2.0.

//! **The companion**: this binary started again with [`crate::ENV`] set, serving the app's
//! socket until the app crashes or ends. On a crash it writes the minidump straight into the
//! crashes folder as `<seconds since 1970>-<id>.part`, `0600`, and renames it to `.dmp` once it
//! is whole, so the app's next launch never reads half a file. A crash within
//! [`crate::PANIC_COVERS`] of a panic the app told it of leaves nothing: the panic's own report
//! has it.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use minidumper::{LoopAction, MinidumpBinary, Server, ServerHandler, SocketName};

/// Serves the app at `socket` until it crashes or ends, keeping a crash's minidump in `crashes`.
pub(crate) fn serve(socket: &Path, crashes: &Path) -> Result<(), minidumper::Error> {
    let mut server = Server::with_name(SocketName::Path(socket))?;
    let handler = Keeper {
        crashes: crashes.to_path_buf(),
        panicked_at: Mutex::new(None),
    };
    let never = AtomicBool::new(false);
    let served = server.run(Box::new(handler), &never, Some(crate::STALE_AFTER));
    let _ = std::fs::remove_file(socket);
    served
}

/// What the companion keeps of a crash, and where.
struct Keeper {
    crashes: PathBuf,
    /// When the app said it panicked, if it did.
    panicked_at: Mutex<Option<Instant>>,
}

impl Keeper {
    fn a_panic_already_told(&self) -> bool {
        self.panicked_at
            .lock()
            .ok()
            .and_then(|told| *told)
            .is_some_and(|at| at.elapsed() <= crate::PANIC_COVERS)
    }
}

impl ServerHandler for Keeper {
    fn create_minidump_file(&self) -> std::io::Result<(File, PathBuf)> {
        private_dir(&self.crashes)?;
        let at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.as_secs());
        let part = self
            .crashes
            .join(format!("{at}-{}.part", uuid::Uuid::new_v4().simple()));
        Ok((create_private(&part)?, part))
    }

    fn on_minidump_created(&self, result: Result<MinidumpBinary, minidumper::Error>) -> LoopAction {
        if let Ok(written) = result {
            let MinidumpBinary { file, path, .. } = written;
            let whole = file.sync_all();
            drop(file);
            if whole.is_ok() && !self.a_panic_already_told() {
                let _ = std::fs::rename(&path, path.with_extension("dmp"));
            } else {
                let _ = std::fs::remove_file(&path);
            }
        }
        // One crash, one dump: the app is gone.
        LoopAction::Exit
    }

    fn on_message(&self, kind: u32, _buffer: Vec<u8>) {
        if kind == crate::PANICKED
            && let Ok(mut told) = self.panicked_at.lock()
        {
            *told = Some(Instant::now());
        }
    }

    fn on_client_disconnected(&self, _num_clients: usize) -> LoopAction {
        // The app ended without crashing.
        LoopAction::Exit
    }
}

/// Makes `dir` this user's alone (`0700`): a minidump holds the crashed process's stacks.
#[cfg(unix)]
fn private_dir(dir: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
}

#[cfg(not(unix))]
fn private_dir(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)
}

/// A new file at `path`, this user's alone (`0600`).
#[cfg(unix)]
fn create_private(path: &Path) -> std::io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
}

#[cfg(not(unix))]
fn create_private(path: &Path) -> std::io::Result<File> {
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(path)
}
