// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The host's socket, its lock and its credentials, and the session protocol on each link it
//! admits ([`serve`]; the steps are the crate's docs).

use std::fs::{File, TryLockError};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use purlis_session_protocol::auth::Credentials;
use purlis_session_protocol::link;
use purlis_session_protocol::local::{Chats, Listener};
use purlis_session_protocol::session::{self, Answer, Command, Peer, Refusal};

use crate::say;

/// Where a host listens and keeps what it mints: its socket, the lock beside it, and the folder
/// of its scopes' credentials. The window and `purlis serve` name the same place
/// (`purlis_core::hostsocket`, `purlis_core::machine::charterd`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Place {
    pub socket: PathBuf,
    pub lock: PathBuf,
    pub credentials: PathBuf,
}

impl Place {
    /// The place of a host whose socket is `socket` and whose credentials are in
    /// `credentials`: its lock is [`LOCK`], beside the socket.
    pub fn at(socket: PathBuf, credentials: PathBuf) -> Place {
        Place {
            lock: socket.with_file_name(LOCK),
            socket,
            credentials,
        }
    }
}

/// The lock's file name, beside the socket.
pub const LOCK: &str = "purlisd.lock";

/// The window that started this host and runs its chats in its own process, by pid: a peer
/// below it is inside a chat, and the window itself is not (`local::Chats::holder`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window(pub u32);

/// Why a host did not start, or stopped.
#[derive(Debug, thiserror::Error)]
pub enum HostError {
    /// Another host of this user serves on this device (ADR 0068 §1: one per user per device).
    #[error(
        "a session host already serves this user on this device, at {}; this one exits and \
         leaves it serving",
        .socket.display()
    )]
    AlreadyServing { socket: PathBuf },
    /// The socket's folder, its lock, the socket or the credentials could not be made safely.
    #[error("the session host could not {doing} at {}: {why}", .at.display())]
    Unsafe {
        doing: &'static str,
        at: PathBuf,
        why: std::io::Error,
    },
    /// The listening socket failed.
    #[error("the session host's socket stopped accepting: {0}")]
    Accept(std::io::Error),
}

fn unsafe_at(doing: &'static str, at: &Path) -> impl FnOnce(std::io::Error) -> HostError {
    let at = at.to_path_buf();
    move |why| HostError::Unsafe { doing, at, why }
}

/// The chats this host holds: none of its own yet, and the window's, when it runs them.
struct Held {
    window: Option<Window>,
}

impl Chats for Held {
    fn programs(&self) -> Vec<u32> {
        Vec::new()
    }

    fn holder(&self) -> Option<u32> {
        self.window.map(|Window(pid)| pid)
    }
}

/// Serve the session protocol at `place` until `until` resolves, then remove the socket.
/// `ready` is called once the host accepts. `window` is the window that runs its chats in its
/// own process, when there is one.
pub async fn serve(
    place: &Place,
    window: Option<Window>,
    ready: impl FnOnce(),
    until: impl Future<Output = ()>,
) -> Result<(), HostError> {
    let folder = place.socket.parent().unwrap_or(Path::new("/"));
    let _private = purlis_same_user::PrivateDirectory::open(folder)
        .map_err(unsafe_at("make its folder this user's alone", folder))?;
    let _lock = lock(place)?;
    let listener = bind(&place.socket)?;
    let credentials = Arc::new(
        Credentials::mint_into(&place.credentials)
            .map_err(unsafe_at("mint its credentials", &place.credentials))?,
    );
    let chats = Arc::new(Held { window });
    ready();

    let served = accept(&listener, &credentials, &chats, until).await;
    // The lock is still held, so the socket at the path is this host's.
    let _ = std::fs::remove_file(&place.socket);
    served
}

/// The folder's lock, held for as long as the host serves: an exclusive `flock`, which the
/// kernel lets go of however the host ends. Made `0600`, never through a link.
fn lock(place: &Place) -> Result<File, HostError> {
    let file = File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_CLOEXEC)
        .open(&place.lock)
        .map_err(unsafe_at("open its lock", &place.lock))?;
    match file.try_lock() {
        Ok(()) => Ok(file),
        Err(TryLockError::WouldBlock) => Err(HostError::AlreadyServing {
            socket: place.socket.clone(),
        }),
        Err(TryLockError::Error(why)) => Err(unsafe_at("take its lock", &place.lock)(why)),
    }
}

/// The socket, bound where a host that ended left none, and made `0600`.
fn bind(socket: &Path) -> Result<Listener, HostError> {
    match std::fs::remove_file(socket) {
        Ok(()) => {}
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => {}
        Err(why) => return Err(unsafe_at("remove the socket a host left", socket)(why)),
    }
    let bound = std::os::unix::net::UnixListener::bind(socket)
        .map_err(unsafe_at("bind its socket", socket))?;
    std::fs::set_permissions(socket, std::fs::Permissions::from_mode(0o600))
        .map_err(unsafe_at("make its socket this user's alone", socket))?;
    bound
        .set_nonblocking(true)
        .and_then(|()| tokio::net::UnixListener::from_std(bound))
        .map(Listener::new)
        .map_err(unsafe_at("listen on its socket", socket))
}

async fn accept(
    listener: &Listener,
    credentials: &Arc<Credentials>,
    chats: &Arc<Held>,
    until: impl Future<Output = ()>,
) -> Result<(), HostError> {
    tokio::pin!(until);
    loop {
        let accepted = tokio::select! {
            () = &mut until => return Ok(()),
            accepted = listener.accept() => accepted.map_err(HostError::Accept)?,
        };
        match accepted {
            Ok(peer) => {
                let credentials = Arc::clone(credentials);
                let chats = Arc::clone(chats);
                tokio::spawn(async move {
                    match link::serve(peer, session::speaks(), &credentials, &chats).await {
                        Ok(link) => {
                            let _ = session::serve(link, NoPrograms, None).await;
                        }
                        Err(refused) => say(&format!("a connection was refused: {refused}")),
                    }
                });
            }
            Err(not_this_user) => say(&format!("a connection was refused: {not_this_user}")),
        }
    }
}

/// The session protocol's host while no program runs in it: it lists none, starts none, and
/// refuses every command about one as about a program it does not hold.
struct NoPrograms;

impl session::Host for NoPrograms {
    async fn call(&self, command: Command, _peer: &Peer) -> Result<Answer, Refusal> {
        match command {
            Command::List => Ok(Answer::Chats(Vec::new())),
            Command::StartProgram(start) => Err(Refusal {
                code: session::NO_SUCH_PROJECT.into(),
                why: format!(
                    "this session host holds no project yet, so it cannot start a program in {}",
                    start.project
                ),
            }),
            _ => Err(Refusal {
                code: session::NO_SUCH_CHAT.into(),
                why: "this session host runs no program yet".into(),
            }),
        }
    }
}
