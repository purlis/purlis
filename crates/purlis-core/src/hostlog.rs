// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The session host's `session-host.log`, capped by size** (#6687).
//!
//! A session host that is a login item (FD-17, #655) writes what it says to its standard error,
//! which launchd appends to `session-host.log` in the app's log directory (`StandardErrorPath`),
//! and it may run for days. launchd never rotates that file, so the host caps it: past [`CAP`]
//! bytes the log is copied whole to `session-host.log.1`, the older copies move up one, the
//! newest [`KEPT`] are kept, and the log is emptied in place.
//!
//! **Why a copy and not a rename.** launchd opened the file before the host started, and the
//! host's standard error holds it. Renamed, it would go on growing under its new name for as
//! long as the host runs. Emptied in place, it is the same file: launchd opens it to append, so
//! the next line goes to its new end, from its start. This is `logrotate`'s `copytruncate`, the
//! standard answer for a writer that cannot reopen its log. A line written between the copy and
//! the emptying is lost; the log holds warnings only.
//!
//! **Why not the app's own rotation.** The app's log (`crate::applog`) rotates through
//! `tracing-appender`, which writes the file itself and rotates by the day only. Here launchd
//! writes the file, and what has to be bounded is its size.
//!
//! Nothing but a person and the Notice that the host is failing reads the log, and the doctor's
//! `session host log` row says its size once it is far past its cap while a host serves: the
//! trimming has stopped working.
//! `docs/plane-format.md` records the files: Machine, device-bound, transient.

use std::fs::OpenOptions;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The log's file name, in the app's log directory.
pub const NAME: &str = "session-host.log";

/// The size past which the log is rotated, in bytes: 1 MB ([`CAP_SAID`]), thousands of
/// warnings.
pub const CAP: u64 = 1_000_000;

/// [`CAP`] as a person reads it, in the doctor and the docs.
pub const CAP_SAID: &str = "1 MB";

/// How many older files are kept beside the log, `session-host.log.1` the newest.
pub const KEPT: usize = 3;

/// How often a running host looks at its log's size. A stat a minute costs nothing, and a host
/// that says only warnings writes nowhere near [`CAP`] in one.
pub const LOOK_EVERY: Duration = Duration::from_secs(60);

/// What [`cap_at`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capped {
    /// The log is within its cap, or not there: nothing was done.
    Within,
    /// The log was copied aside and emptied.
    Rotated,
}

/// Where the log is, beside the app's own: `None` where there is no home to find it in.
pub fn path() -> Option<PathBuf> {
    crate::applog::app_log_dir().map(|dir| dir.join(NAME))
}

/// The `n`th older file of `log`, `1` the newest: `session-host.log.1`.
pub fn older(log: &Path, n: usize) -> PathBuf {
    let mut name = log.as_os_str().to_owned();
    name.push(format!(".{n}"));
    PathBuf::from(name)
}

/// [`cap_at`] with the host's own cap and count.
pub fn cap(log: &Path) -> io::Result<Capped> {
    cap_at(log, CAP, KEPT)
}

/// **Rotates `log` once it is past `cap` bytes**, keeping `kept` older files: the log is copied
/// beside them first (`<log>.1.tmp`), then the oldest goes, each other moves up one, the copy
/// becomes the first, and the log is emptied in place.
///
/// The copy comes first because it is what fails on a full disk: one that fails has moved and
/// removed nothing, so the host trying again each minute never loses the older files, and the
/// log is not emptied until its copy is in place.
///
/// Only a plain file is rotated: a link in the log's place is refused, and what it points to is
/// left alone. A log that is not there is within its cap.
pub fn cap_at(log: &Path, cap: u64, kept: usize) -> io::Result<Capped> {
    let meta = match std::fs::symlink_metadata(log) {
        Ok(meta) => meta,
        Err(why) if why.kind() == io::ErrorKind::NotFound => return Ok(Capped::Within),
        Err(why) => return Err(why),
    };
    if !meta.is_file() {
        return Err(io::Error::other(format!(
            "{} is not a plain file, so it is not trimmed",
            log.display()
        )));
    }
    if meta.len() <= cap {
        return Ok(Capped::Within);
    }
    if kept == 0 {
        return empty(log).map(|()| Capped::Rotated);
    }
    let copy = staged(log);
    // One a copy that failed left, or a link, which the copy would otherwise write through.
    if std::fs::symlink_metadata(&copy).is_ok_and(|meta| !meta.is_dir()) {
        std::fs::remove_file(&copy)?;
    }
    if let Err(why) = std::fs::copy(log, &copy) {
        // What a full disk let it write is no use, and only takes room; not a copy that cannot
        // be removed, such as a folder in its place.
        if std::fs::symlink_metadata(&copy).is_ok_and(|meta| meta.is_file()) {
            let _ = std::fs::remove_file(&copy);
        }
        return Err(why);
    }
    match std::fs::remove_file(older(log, kept)) {
        Err(why) if why.kind() != io::ErrorKind::NotFound => return Err(why),
        _ => {}
    }
    for n in (1..kept).rev() {
        match std::fs::rename(older(log, n), older(log, n + 1)) {
            Err(why) if why.kind() != io::ErrorKind::NotFound => return Err(why),
            _ => {}
        }
    }
    std::fs::rename(&copy, older(log, 1))?;
    empty(log)?;
    Ok(Capped::Rotated)
}

/// Where `log` is copied before the older files move: `session-host.log.1.tmp`.
fn staged(log: &Path) -> PathBuf {
    let mut name = older(log, 1).into_os_string();
    name.push(".tmp");
    PathBuf::from(name)
}

/// Empties `log` in place, never through a link: the writer that holds it open to append goes
/// on at its new end.
fn empty(log: &Path) -> io::Result<()> {
    let mut options = OpenOptions::new();
    options.write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    options.open(log)?.set_len(0)
}

/// **Keeps `log` capped while this process runs**: [`cap`] now, and every [`LOOK_EVERY`] after,
/// on a thread of its own. What the session host that is a login item starts once it serves.
pub fn keep_capped(log: PathBuf) {
    keep_capped_at(log, CAP, KEPT, LOOK_EVERY);
}

/// [`keep_capped`], past `cap` bytes, keeping `kept`, looking `every` so long.
///
/// A rotation that fails is said once, as a warning, which goes to the host's standard error and
/// so into the log itself; again only once it fails for another reason. The doctor's `session
/// host log` row says the log's size once it is far past its cap while a host serves.
pub fn keep_capped_at(log: PathBuf, cap: u64, kept: usize, every: Duration) {
    let spawned = std::thread::Builder::new()
        .name("session-host-log".into())
        .spawn(move || {
            let mut said: Option<String> = None;
            loop {
                match cap_at(&log, cap, kept) {
                    Ok(_) => said = None,
                    Err(why) => {
                        let why = why.to_string();
                        if said.as_deref() != Some(why.as_str()) {
                            tracing::warn!(
                                "purlis: the session host's log could not be trimmed: {why}"
                            );
                            said = Some(why);
                        }
                    }
                }
                std::thread::sleep(every);
            }
        });
    if let Err(why) = spawned {
        tracing::warn!("purlis: the session host's log is not kept trimmed: {why}");
    }
}
