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
//! - its host record ([`HOST_RECORD`]) names programs: a host killed outright left chats that
//!   may still run, and it stays ([`Live::Chats`]). A host that ends as it should empties it.
//!
//! Otherwise its binary's folder goes first, so nothing can start it again, then its host's
//! folder, then the socket, link socket and lock file its host left behind. A folder that is a
//! link is removed as a link; what it names stays.

use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use super::{SERVER_DIR, a_version, binary_at, for_login_shell, host_folder_of};

/// The host record's file in a host's own folder (`purlis_host::HOST_RECORD`, #2007): what
/// names the programs a host started and has not ended.
pub const HOST_RECORD: &str = "hosts.jsonl";

/// Why a version that may go stays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Live {
    /// Its host serves: it holds its lock, or answers on its socket.
    Serving,
    /// Its host's record names programs that may still run.
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
pub fn prune_in(config_root: &Path, running: &str, socket: &Path) -> Pruned {
    let server = crate::machine::dir(config_root).join(SERVER_DIR);
    let installed: Vec<String> = std::fs::read_dir(&server)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .filter(|name| a_version(name))
        .collect();
    let daemon = crate::machine::charterd(config_root);
    let mut pruned = Pruned::default();
    for version in may_go(running, &installed) {
        let socket = crate::hostsocket::of_version(socket, &version);
        let host = host_folder_of(&daemon, &version);
        match drained(&socket, &host) {
            Err(live) => pruned.kept.push((version, live)),
            Ok(_held) => {
                let removed = remove(&server.join(&version)).and_then(|()| remove(&host));
                match removed {
                    Ok(()) => {
                        for left in [
                            socket.with_extension("link.sock"),
                            socket.with_extension("lock"),
                            socket.clone(),
                        ] {
                            let _ = std::fs::remove_file(left);
                        }
                        pruned.removed.push(version);
                    }
                    Err(why) => pruned.failed.push((version, why.to_string())),
                }
            }
        }
    }
    pruned
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
    match std::fs::symlink_metadata(host.join(HOST_RECORD)) {
        Ok(record) if record.len() > 0 => Err(Live::Chats),
        Ok(_) => Ok(held),
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => Ok(held),
        Err(_) => Err(Live::Chats),
    }
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
    let socket = crate::hostsocket::here_for(
        std::env::var_os("XDG_RUNTIME_DIR").as_deref(),
        Some(&config),
    );
    Some(prune_in(&config, &running, &socket))
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
