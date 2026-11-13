// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A pool's one SSH forward** (D-6257-1, ADR 0123 §8): `ssh -T -L` held for as long as the pool
//! is in use, its local socket in `<data>/engine-links/<pool>/`, its pinned `known_hosts` in a run
//! folder of its own under `<data>/docker-settings/`, both removed when it ends.
//!
//! **One process holds it** ([`hold`]): the one that holds `held.lock` in the pool's folder. A
//! second process (`purlis pool …` while the window holds the forward) uses the socket that
//! process keeps rather than opening a forward over it, and takes it over once that process
//! has let go. **A forward that dropped is opened again** by the next [`hold`], before anything
//! links over it, and ssh checks the pinned key as it does.
//!
//! **It ends with the process holding it**: the window ends every forward as it quits
//! ([`let_go_of_all`]), and a process that goes any other way closes the pipe its forward's
//! session waits on, which ends the whole connection there ([`super::HELD_BY`]).

use std::collections::BTreeMap;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::{FORWARD_WITHIN, HELD, HostKey, Route};

/// **One running forward.** Ending it (dropping it) ends ssh and removes its socket.
pub struct Forward {
    child: Child,
    /// purlis's end of the pipe the forward's session waits on ([`super::HELD_BY`]): closed as
    /// this process goes, which ends the forward.
    _held_by: Option<std::process::ChildStdin>,
    socket: PathBuf,
    /// The lock that says this process holds the pool's forward.
    _held: std::fs::File,
    /// The run folder its pinned `known_hosts` is in, removed with it.
    _settings: crate::dockerpool::settings::RunFolder,
    /// What ssh said, kept to be shown where it ends.
    said: Arc<Mutex<String>>,
}

/// How a pool's forward is held.
pub enum Held {
    /// By this process.
    Own(Forward),
    /// By another process of this user's, which keeps the socket.
    Shared(PathBuf),
}

impl Held {
    /// The local socket.
    #[must_use]
    pub fn socket(&self) -> &Path {
        match self {
            Held::Own(forward) => &forward.socket,
            Held::Shared(socket) => socket,
        }
    }
}

/// What a forward is opened from.
pub struct Opening<'a> {
    pub ssh: &'a Path,
    pub route: &'a Route,
    pub pool: &'a str,
    pub key: &'a HostKey,
    /// `<data>/engine-links`.
    pub engine_links: &'a Path,
    /// `<data>/docker-settings`.
    pub settings_in: &'a Path,
}

impl Forward {
    /// **Whether it still forwards**: ssh is running and its socket is there.
    pub fn alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None)) && std::fs::symlink_metadata(&self.socket).is_ok()
    }

    /// The local socket.
    #[must_use]
    pub fn socket(&self) -> &Path {
        &self.socket
    }

    /// The process id of the ssh holding it.
    #[must_use]
    pub fn ssh(&self) -> u32 {
        self.child.id()
    }
}

impl Drop for Forward {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_file(&self.socket);
    }
}

/// **Opens pool `pool`'s forward**, or answers the one another process holds. Refused, with what
/// ssh said, where ssh ends before its socket is made: a machine that presents another key than
/// the pinned one is refused by ssh itself.
pub fn open(from: &Opening<'_>) -> Result<Held, String> {
    let socket = super::socket_of(from.engine_links, from.pool)?;
    let folder = socket
        .parent()
        .ok_or_else(|| "the forward's socket has no folder".to_owned())?;
    for at in [from.engine_links, folder] {
        purlis_same_user::PrivateDirectory::open(at)
            .map_err(|why| format!("purlis could not make {}: {why}", at.display()))?;
    }
    let held = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(folder.join(HELD))
        .map_err(|why| format!("purlis could not hold {}: {why}", folder.display()))?;
    match held.try_lock() {
        Ok(()) => {}
        Err(std::fs::TryLockError::WouldBlock) => {
            // Another process holds it, and keeps its socket while it does.
            return wait_for(&socket, FORWARD_WITHIN, &mut || Ok(true))
                .map(|()| Held::Shared(socket.clone()));
        }
        Err(std::fs::TryLockError::Error(why)) => {
            return Err(format!("purlis could not hold {}: {why}", folder.display()));
        }
    }
    // What the person's settings add that would ride on it is refused before ssh runs.
    super::connect::resolve(from.ssh, from.route)
        .map_err(|finding| crate::poolsetup::connect_said(&finding))?;
    let settings =
        crate::dockerpool::settings::run_folder(from.settings_in).map_err(|why| why.0)?;
    let known_hosts = settings
        .keep(
            "known_hosts",
            super::known_hosts(from.pool, from.key).as_bytes(),
        )
        .map_err(|why| format!("purlis could not write the pinned host key: {why}"))?;
    let args = super::forward_args(from.route, from.pool, &known_hosts, &socket)?;
    let mut command = std::process::Command::new(from.ssh);
    command
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut child = crate::forklock::spawn(&mut command)
        .map_err(|why| format!("{} could not be run: {why}", from.ssh.display()))?;
    let said = Arc::new(Mutex::new(String::new()));
    if let Some(mut stderr) = child.stderr.take() {
        let said = Arc::clone(&said);
        std::thread::spawn(move || {
            let mut buffer = [0u8; 1024];
            while let Ok(read) = stderr.read(&mut buffer) {
                if read == 0 {
                    break;
                }
                if let Ok(mut kept) = said.lock()
                    && kept.len() < 16 * 1024
                {
                    kept.push_str(&String::from_utf8_lossy(&buffer[..read]));
                }
            }
        });
    }
    let held_by = child.stdin.take();
    let mut forward = Forward {
        child,
        _held_by: held_by,
        socket: socket.clone(),
        _held: held,
        _settings: settings,
        said,
    };
    let waited = wait_for(&socket, FORWARD_WITHIN, &mut || {
        Ok(matches!(forward.child.try_wait(), Ok(None)))
    });
    match waited {
        Ok(()) => Ok(Held::Own(forward)),
        Err(why) => {
            // Its stderr is read to its end once ssh has ended.
            let _ = forward.child.kill();
            let _ = forward.child.wait();
            std::thread::sleep(Duration::from_millis(50));
            let said = forward
                .said
                .lock()
                .map(|said| crate::shown::short(said.trim()))
                .unwrap_or_default();
            Err(if said.is_empty() {
                why
            } else {
                format!("{why}: ssh said: {said}")
            })
        }
    }
}

/// Waits up to `within` for `socket` to be there, while `going` says the forward still runs.
fn wait_for(
    socket: &Path,
    within: Duration,
    going: &mut dyn FnMut() -> Result<bool, String>,
) -> Result<(), String> {
    let until = Instant::now() + within;
    loop {
        if std::fs::symlink_metadata(socket).is_ok() {
            return Ok(());
        }
        if !going()? {
            return Err("ssh ended before the forward was open".to_owned());
        }
        if Instant::now() >= until {
            return Err(format!(
                "the forward didn't open within {} seconds",
                within.as_secs()
            ));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// The forwards this process holds, by pool.
static HELD_HERE: Mutex<BTreeMap<String, Held>> = Mutex::new(BTreeMap::new());

/// **Pool `pool`'s forward, opened where it isn't, or opened again where it dropped**: the
/// socket each run of `docker` for the pool is pointed at.
pub fn hold(from: &Opening<'_>) -> Result<PathBuf, String> {
    hold_noting(from).map(|(socket, _)| socket)
}

/// [`hold`], and whether it opened the forward now (it was not held, or it had dropped).
pub fn hold_noting(from: &Opening<'_>) -> Result<(PathBuf, bool), String> {
    let mut held = HELD_HERE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(kept) = held.get_mut(from.pool) {
        let alive = match kept {
            Held::Own(forward) => forward.alive(),
            // Another process's: still holding while its socket is there and it holds the lock.
            Held::Shared(socket) => {
                std::fs::symlink_metadata(&*socket).is_ok() && another_holds(socket)
            }
        };
        if alive {
            return Ok((kept.socket().to_path_buf(), false));
        }
        held.remove(from.pool);
    }
    let opened = open(from)?;
    let socket = opened.socket().to_path_buf();
    held.insert(from.pool.to_owned(), opened);
    Ok((socket, true))
}

/// Whether another process holds the lock beside `socket`.
fn another_holds(socket: &Path) -> bool {
    let Some(folder) = socket.parent() else {
        return false;
    };
    let Ok(file) = std::fs::File::open(folder.join(HELD)) else {
        return false;
    };
    matches!(file.try_lock(), Err(std::fs::TryLockError::WouldBlock))
}

/// **Whether pool `pool`'s forward, held by this process, has dropped** since it was opened: what
/// a link's liveness reads before it relinks (ADR 0123 §8).
pub fn dropped(pool: &str) -> bool {
    let mut held = HELD_HERE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    match held.get_mut(pool) {
        Some(Held::Own(forward)) => !forward.alive(),
        Some(Held::Shared(socket)) => !another_holds(socket),
        None => false,
    }
}

/// **Ends pool `pool`'s forward, where this process holds it**: as the pool is removed, or its
/// pin changes.
pub fn let_go(pool: &str) {
    let mut held = HELD_HERE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    held.remove(pool);
}

/// **The process id of the ssh holding pool `pool`'s forward**, where this process holds it.
#[must_use]
pub fn ssh_of(pool: &str) -> Option<u32> {
    let held = HELD_HERE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    match held.get(pool) {
        Some(Held::Own(forward)) => Some(forward.ssh()),
        _ => None,
    }
}

/// **Ends every forward this process holds**, at once: what the window does as it quits, so none
/// is left to its session's own ending ([`super::HELD_BY`]). Held in a static, a forward is
/// otherwise never dropped.
pub fn let_go_of_all() {
    let all = std::mem::take(
        &mut *HELD_HERE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
    );
    drop(all);
}
