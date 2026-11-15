// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A drained version's folders removed from a runner** (#6688; ADR 0078 §5, ADR 0068 §8).
//!
//! Versions sit side by side ([`super`]): an install never stops an older version's host, which
//! keeps its chats until each ends. Once one has drained, its binary's folder
//! (`<config>/server/<version>/`) and its host's (`<config>/purlisd/server/<version>/`) are
//! removed, so a runner does not keep one of each per update.
//!
//! **Which versions may go** ([`may_go`]): every one older than the one before the running one.
//! The running version, the one before it, any newer one and any folder whose name is not read
//! as a version stay, whatever their hosts do.
//!
//! **Which of those go** ([`prune_in`]), decided on the runner by the running version's own
//! binary (`purlis bridge --prune`, run as [`prune_command`] says), never by reading what a shell
//! printed: a version goes only once its host has drained. For each, holding its host's lock
//! (the exclusive `flock` a host holds for as long as it serves), so no host of it starts while
//! it is looked at and removed:
//!
//! - its lock is held by a host: it serves, and stays ([`Live::Serving`]);
//! - something answers on its socket (a host whose lock file the system cleaned away): it
//!   stays ([`Live::Serving`]);
//! - its host record ([`HOST_RECORD`]) names a program started and not ended whose process still
//!   runs, by the host's own check ([`purlis_host::Started::still_running`]): noted in this boot,
//!   and its process number's start the one noted. It stays ([`Live::Chats`]). A host that ends
//!   as it should empties its record; one stopped by a signal leaves each start beside its end;
//!   one killed outright (`SIGKILL`, a reboot, #6700) leaves starts with no end, whose processes
//!   died with it or have since ended, and keep nothing.
//!
//! Otherwise its binary's folder goes first, so nothing can start it again, then the socket, link
//! socket and lock file its host left behind, then its host's folder. A folder that is a link is
//! removed as a link; what it names stays. A version with a host's folder and no binary's folder
//! is pruned too, so a removal cut short is taken up again by the next prune.

use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use super::{SERVER_DIR, a_version, binary_at, for_login_shell, host_folder_of};

/// The host record's file in a host's own folder (`purlis_host::HOST_RECORD`, #2007): what
/// names the programs a host started and has not ended.
pub use purlis_host::HOST_RECORD;

/// The most of a host record that is read: one larger is not one a host wrote.
pub use purlis_host::READ_AT_MOST;

/// Why a version that may go stays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Live {
    /// Its host serves: it holds its lock, or answers on its socket.
    Serving,
    /// Its host's record names a program started and not ended, which may still run.
    Chats,
}

impl Live {
    fn word(self) -> &'static str {
        match self {
            Live::Serving => "serving",
            Live::Chats => "chats",
        }
    }
}

/// **What pruning did**: the versions removed, those that may go but stay, with why, and those
/// that could not be removed, with what the system said.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Pruned {
    pub removed: Vec<String>,
    pub kept: Vec<(String, Live)>,
    pub failed: Vec<(String, String)>,
}

impl Pruned {
    /// What `purlis bridge --prune` prints: a `removed=<version>` line for each version
    /// removed, and a `kept=<version> <why>` line for each that stays. What could not be
    /// removed is said on standard error ([`Pruned::said`]).
    #[must_use]
    pub fn printed(&self) -> String {
        let mut printed = String::new();
        for version in &self.removed {
            printed.push_str(&format!("removed={version}\n"));
        }
        for (version, live) in &self.kept {
            printed.push_str(&format!("kept={version} {}\n", live.word()));
        }
        printed
    }

    /// What could not be removed, a sentence a line, for standard error.
    #[must_use]
    pub fn said(&self) -> String {
        self.failed
            .iter()
            .map(|(version, why)| {
                format!("purlis {version} could not be removed from this runner: {why}\n")
            })
            .collect()
    }
}

/// **The versions of `installed` that may go while `running` runs**: each one older than the
/// newest one older than `running`, ordered as versions are (semver: `1.10.0` after `1.9.0`, a
/// dev build before its release). None where `running` is not read as a version; a name not
/// read as one is never chosen.
#[must_use]
pub fn may_go(running: &str, installed: &[String]) -> Vec<String> {
    let Ok(running) = semver::Version::parse(running) else {
        return Vec::new();
    };
    let mut older: Vec<(semver::Version, &String)> = installed
        .iter()
        .filter_map(|name| Some((semver::Version::parse(name).ok()?, name)))
        .filter(|(version, _)| *version < running)
        .collect();
    older.sort_by(|a, b| a.0.cmp(&b.0));
    // The one before the running one stays.
    older.pop();
    older.into_iter().map(|(_, name)| name.clone()).collect()
}

/// **The drained versions that may go removed from the config home `config_root`**, `running`
/// being the version that runs, and `socket` the host socket `crate::hostsocket::here_for` names
/// for that config home, before a version names its own ([`crate::hostsocket::of_version`]).
#[must_use]
///
/// **A version's service goes first** (RR-15, #724): the systemd user unit or LaunchAgent
/// `purlis serve --install` gave its host, taken back by its name alone
/// ([`crate::runnerhost::remove_version`]) while the version's lock is held, so the manager can
/// never start a host of it again once its folders are gone. Its own binary is never run: an
/// older one may not know `serve --remove`. Where the service could not be taken back, nothing of
/// the version is removed.
pub fn prune_in(config_root: &Path, running: &str, socket: &Path) -> Pruned {
    let here = crate::runnerhost::Here::this(PathBuf::new());
    prune_with(
        config_root,
        running,
        socket,
        &mut |version| match &here {
            Some(here) => crate::runnerhost::remove_version(here, version),
            None => Ok(()),
        },
        &mut remove,
    )
}

/// [`prune_in`], each version's service taken back by `unservice`, and each folder and file
/// removed by `remove`: a test's, to watch the lock held as each goes, or to fail one.
fn prune_with(
    config_root: &Path,
    running: &str,
    socket: &Path,
    unservice: &mut dyn FnMut(&str) -> Result<(), String>,
    remove: &mut dyn FnMut(&Path) -> std::io::Result<()>,
) -> Pruned {
    let server = crate::machine::dir(config_root).join(SERVER_DIR);
    let daemon = crate::machine::charterd(config_root);
    let installed = versions_in(&server);
    let mut candidates = may_go(running, &installed);
    // A host's folder left by a removal cut short, its binary's folder gone: nothing to start it
    // again, and nothing to keep it for.
    if let Ok(running) = semver::Version::parse(running) {
        for version in versions_in(&daemon.join(SERVER_DIR)) {
            let older = semver::Version::parse(&version).is_ok_and(|version| version < running);
            if older && !installed.contains(&version) && !candidates.contains(&version) {
                candidates.push(version);
            }
        }
    }
    let mut pruned = Pruned::default();
    for version in candidates {
        let socket = crate::hostsocket::of_version(socket, &version);
        let host = host_folder_of(&daemon, &version);
        match drained(&socket, &host) {
            Err(live) => pruned.kept.push((version, live)),
            Ok(_held) => {
                // Its service first, under the lock: kept whole where it would not go.
                if let Err(why) = unservice(&version) {
                    pruned
                        .failed
                        .push((version, format!("its service could not be removed: {why}")));
                    continue;
                }
                // The lock stays held, by `_held`, until each is removed; the host's folder goes
                // last, so a removal cut short leaves it for the next prune to find.
                let removed = [
                    server.join(&version),
                    socket.with_extension("link.sock"),
                    socket.clone(),
                    socket.with_extension("lock"),
                    host,
                ]
                .iter()
                .try_for_each(|path| remove(path));
                match removed {
                    Ok(()) => pruned.removed.push(version),
                    Err(why) => pruned.failed.push((version, why.to_string())),
                }
            }
        }
    }
    pruned
}

/// The names in `folder` read as versions.
fn versions_in(folder: &Path) -> Vec<String> {
    std::fs::read_dir(folder)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .filter(|name| a_version(name))
        .collect()
}

/// **Whether the host of the version whose socket is `socket` and whose folder is `host` has
/// drained**: its lock, held until the returned file is dropped where it could be taken, or why
/// it has not.
fn drained(socket: &Path, host: &Path) -> Result<Option<std::fs::File>, Live> {
    let held = match std::fs::File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC)
        .open(socket.with_extension("lock"))
    {
        Ok(file) => match file.try_lock() {
            Ok(()) => Some(file),
            Err(std::fs::TryLockError::WouldBlock) => return Err(Live::Serving),
            Err(std::fs::TryLockError::Error(_)) => return Err(Live::Serving),
        },
        // No folder for it: no host of it ever served here, or the system cleaned it away; the
        // socket answers for a host whose folder went.
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => None,
        // A lock not this user's to open is not one to judge by: it stays.
        Err(_) => return Err(Live::Serving),
    };
    if std::os::unix::net::UnixStream::connect(socket).is_ok() {
        return Err(Live::Serving);
    }
    match left_running(&host.join(HOST_RECORD)) {
        Some(false) => Ok(held),
        Some(true) | None => Err(Live::Chats),
    }
}

/// **Whether the host record at `record` names a program started and not ended whose process
/// still runs** ([`purlis_host::Started::still_running`]): `Some(false)` where there is no
/// record, or none it names started and not ended is still the process it started (a host that
/// ended as it should empties it; one stopped by a signal leaves each start beside its end; one
/// killed outright leaves starts whose processes died with it, or ran on and have since ended).
/// `None` where it can't be read: not a regular file, or larger than any host writes
/// ([`READ_AT_MOST`]). A line that can't be read, a torn last one among them, is passed over, as
/// the host passes it over.
fn left_running(record: &Path) -> Option<bool> {
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "snake_case")]
    enum Line {
        Started(purlis_host::Started),
        Ended { program: String },
    }
    match std::fs::symlink_metadata(record) {
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => return Some(false),
        Ok(found) if found.is_file() && found.len() <= READ_AT_MOST => {}
        _ => return None,
    }
    let text = std::fs::read(record).ok()?;
    let mut running = std::collections::HashMap::new();
    for line in text.split(|byte| *byte == b'\n') {
        match serde_json::from_slice::<Line>(line) {
            Ok(Line::Started(started)) => {
                running.insert(started.program.clone(), started);
            }
            Ok(Line::Ended { program }) => {
                running.remove(&program);
            }
            Err(_) => {}
        }
    }
    Some(
        running
            .values()
            .any(|started| started.still_running().is_some()),
    )
}

/// `path` removed, a link as a link; one that is not there is removed already.
fn remove(path: &Path) -> std::io::Result<()> {
    let removed = match std::fs::symlink_metadata(path) {
        Ok(found) if found.is_dir() => std::fs::remove_dir_all(path),
        Ok(_) => std::fs::remove_file(path),
        Err(why) => Err(why),
    };
    match removed {
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

/// **The drained versions removed from the config home this process resolves**, this binary
/// being the running version: what `purlis bridge --prune` does on a runner. `None` for a
/// binary not installed as a version in `<config>/server/`, which prunes nothing.
#[must_use]
pub fn prune_here() -> Option<Pruned> {
    let config = crate::machine::config_root_unheld()?;
    let running = super::version_of_binary(&config, &std::env::current_exe().ok()?)?;
    let run_user = Path::new("/run/user").join(rustix::process::getuid().as_raw().to_string());
    let runtime = runtime_dir(std::env::var_os("XDG_RUNTIME_DIR").as_deref(), &run_user);
    let socket =
        crate::hostsocket::here_for(runtime.as_deref().map(Path::as_os_str), Some(&config));
    Some(prune_in(&config, &running, &socket))
}

/// **The runtime folder whose sockets and locks prune judges by**: the one the environment names
/// where it names one, as every host and bridge reads it; else `run_user` (`/run/user/<uid>`)
/// where it is there, which systemd's user manager gives every unit, a host `purlis serve
/// --install` handed it among them (RR-15, #724). So a prune run without the variable (from cron,
/// or `sudo -u`) still finds a supervised host's lock, and never takes a serving version for a
/// drained one.
fn runtime_dir(named: Option<&std::ffi::OsStr>, run_user: &Path) -> Option<PathBuf> {
    match named {
        Some(named) => Some(PathBuf::from(named)),
        None => run_user.is_dir().then(|| run_user.to_path_buf()),
    }
}

/// **The versions the runner said it removed**, from what [`Pruned::printed`] printed: for the
/// desktop to say, never to decide anything by. A line not naming a version is passed over.
#[must_use]
pub fn removed_from(printed: &str) -> Vec<String> {
    printed
        .lines()
        .filter_map(|line| line.strip_prefix("removed="))
        .filter(|version| a_version(version))
        .map(str::to_owned)
        .collect()
}

/// **What prunes the runner**: the running version's binary there,
/// `<config>/server/<version>/purlis bridge --prune`, for its login shell. `config` is plain
/// ([`super::a_plain_path`], as [`super::probed_from`] holds it) and `version` is one
/// ([`a_version`]), as an install's are.
#[must_use]
pub fn prune_command(config: &str, version: &str) -> String {
    for_login_shell(&format!("{} bridge --prune", binary_at(config, version)))
}

#[cfg(test)]
mod tests;
