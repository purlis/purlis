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
//! **The lock never outlives the forward** (ADR 0123, amended 2026-11-17): the holder lets go of
//! its socket and its lock the moment its ssh ends ([`watch`]), not when it next looks at the
//! pool, which it may never do. And a second process uses a forward another holds only while its
//! socket answers: a lock held over a socket nothing answers is waited out, and the forward
//! opened anew once it is let go.
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
    /// Its ssh, its socket and its lock, let go of together.
    lease: Arc<Mutex<Lease>>,
    /// The process id of its ssh.
    pid: u32,
    /// purlis's end of the pipe the forward's session waits on ([`super::HELD_BY`]): closed as
    /// this process goes, which ends the forward.
    _held_by: Option<std::process::ChildStdin>,
    socket: PathBuf,
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

/// **What a running forward holds**: its ssh, its socket and the pool's lock, let go of together
/// and once, so the lock is never held over a socket nothing answers.
struct Lease {
    child: Child,
    socket: PathBuf,
    /// The lock that says this process holds the pool's forward; `None` once let go.
    held: Option<std::fs::File>,
}

impl Lease {
    /// Whether its ssh has ended.
    fn ended(&mut self) -> bool {
        !matches!(self.child.try_wait(), Ok(None))
    }

    /// Ends ssh, removes the socket, then lets go of the lock: in that order, so another process
    /// that takes the lock next never has its own socket removed by this one.
    fn let_go(&mut self) {
        let Some(held) = self.held.take() else {
            return;
        };
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_file(&self.socket);
        drop(held);
    }
}

/// How often a forward's watch asks whether its ssh has ended.
const WATCHED_EVERY: Duration = Duration::from_millis(200);

/// **Lets go of a forward the moment its ssh ends** (ADR 0123, amended 2026-11-17), from a thread
/// of its own: the process holding it may not look at the pool again for hours, and another
/// process of the person's waits on its lock meanwhile. Ends with the forward.
fn watch(lease: &Arc<Mutex<Lease>>) {
    let lease = Arc::downgrade(lease);
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(WATCHED_EVERY);
            let Some(lease) = lease.upgrade() else {
                return;
            };
            let mut lease = lease
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if lease.held.is_none() {
                return;
            }
            if lease.ended() {
                lease.let_go();
                return;
            }
        }
    });
}

impl Forward {
    fn lease(&self) -> std::sync::MutexGuard<'_, Lease> {
        self.lease
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// **Whether it still forwards**: ssh is running, it still holds the lock, and its socket is
    /// there.
    pub fn alive(&mut self) -> bool {
        let mut lease = self.lease();
        lease.held.is_some() && !lease.ended() && std::fs::symlink_metadata(&self.socket).is_ok()
    }

    /// The local socket.
    #[must_use]
    pub fn socket(&self) -> &Path {
        &self.socket
    }

    /// The process id of the ssh holding it.
    #[must_use]
    pub fn ssh(&self) -> u32 {
        self.pid
    }
}

impl Drop for Forward {
    fn drop(&mut self) {
        self.lease().let_go();
    }
}

/// **Opens pool `pool`'s forward**, or answers the one another process holds. Refused, with what
/// ssh said, where ssh ends before its socket is made: a machine that presents another key than
/// the pinned one is refused by ssh itself.
pub fn open(from: &Opening<'_>) -> Result<Held, String> {
    purlis_session_protocol::steps::begin_in("ssh_forward_open", from.pool).ended(opened(from))
}

fn opened(from: &Opening<'_>) -> Result<Held, String> {
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
    if let Taken::Shared = taken(&held, &socket, from.pool, folder)? {
        return Ok(Held::Shared(socket));
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
    let pid = child.id();
    let forward = Forward {
        lease: Arc::new(Mutex::new(Lease {
            child,
            socket: socket.clone(),
            held: Some(held),
        })),
        pid,
        _held_by: held_by,
        socket: socket.clone(),
        _settings: settings,
        said,
    };
    let waited = wait_for(&socket, FORWARD_WITHIN, &mut || {
        Ok(!forward.lease().ended())
    });
    match waited {
        Ok(()) => {
            watch(&forward.lease);
            Ok(Held::Own(forward))
        }
        Err(why) => {
            // Its stderr is read to its end once ssh has ended.
            forward.lease().let_go();
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

/// Waits up to `within` for something to answer on `socket`, while `going` says the forward
/// still runs.
fn wait_for(
    socket: &Path,
    within: Duration,
    going: &mut dyn FnMut() -> Result<bool, String>,
) -> Result<(), String> {
    let until = Instant::now() + within;
    loop {
        if answers(socket) {
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

/// What [`taken`] found.
enum Taken {
    /// This process holds the pool's lock now.
    Held,
    /// Another holds it, and its socket answers.
    Shared,
}

/// How long a lock held over a socket nothing answers is waited out: a holder whose ssh ended lets
/// go within [`WATCHED_EVERY`]. Short, since the wait holds this process's table of forwards.
const LET_GO_WITHIN: Duration = Duration::from_secs(2);

/// **Takes the pool's lock, or answers that another process's forward is to be used**. A holder
/// with no socket yet is still opening it, and is waited for up to [`FORWARD_WITHIN`]. A lock held
/// over a socket nothing answers is the forward of a process whose ssh ended and that has not let
/// go yet: waited out up to [`LET_GO_WITHIN`], and refused after that.
fn taken(held: &std::fs::File, socket: &Path, pool: &str, folder: &Path) -> Result<Taken, String> {
    let until = Instant::now() + FORWARD_WITHIN;
    let mut silent_since = None;
    loop {
        match held.try_lock() {
            Ok(()) => return Ok(Taken::Held),
            Err(std::fs::TryLockError::WouldBlock) if answers(socket) => return Ok(Taken::Shared),
            Err(std::fs::TryLockError::WouldBlock) => {
                if std::fs::symlink_metadata(socket).is_ok() {
                    let since = *silent_since.get_or_insert_with(Instant::now);
                    if since.elapsed() >= LET_GO_WITHIN {
                        return Err(format!(
                            "purlis could not reach pool {pool}: another purlis process holds its \
                             connection to that machine, and nothing answers on it. Quit that \
                             process, or the purlis window, and try again"
                        ));
                    }
                }
            }
            Err(std::fs::TryLockError::Error(why)) => {
                return Err(format!("purlis could not hold {}: {why}", folder.display()));
            }
        }
        if Instant::now() >= until {
            return Err(format!(
                "the forward didn't open within {} seconds",
                FORWARD_WITHIN.as_secs()
            ));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// **Whether something answers on `socket`**: an ssh still forwarding listens there; the file a
/// killed one left does not. Each ask is a connection ssh carries to the engine as a channel of
/// its own, closed at once.
fn answers(socket: &Path) -> bool {
    std::os::unix::net::UnixStream::connect(socket).is_ok()
}

/// **Whether another process holds the forward at `socket` and it still forwards.**
fn shared_alive(socket: &Path) -> bool {
    another_holds(socket) && answers(socket)
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
            // Another process's: still holding while it holds the lock and its socket answers.
            Held::Shared(socket) => shared_alive(socket),
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
        Some(Held::Shared(socket)) => !shared_alive(socket),
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
        // Not one whose ssh has ended and that has let go: its id may be another process's now.
        Some(Held::Own(forward)) if forward.lease().held.is_some() => Some(forward.ssh()),
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

#[cfg(test)]
mod tests;
