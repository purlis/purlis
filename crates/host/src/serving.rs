// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The host's socket, its lock and its credentials, and the session protocol on each link it
//! admits ([`serve`]; the steps are the crate's docs).

use std::fs::{File, TryLockError};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use purlis_session_protocol::auth::{Credentials, Scope};
use purlis_session_protocol::hostlink::{self, Handshake, Identity, Liveness, Pinned};
use purlis_session_protocol::link;
use purlis_session_protocol::local::Listener;
use purlis_session_protocol::session;

use crate::hosting::Hosting;
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
    /// `credentials`: its lock is beside the socket, named as it is with `.lock` for `.sock`
    /// ([`LOCK`] beside `purlisd.sock`), so the host of another config home, whose socket has a
    /// name of its own in the same folder (#2114), holds a lock of its own.
    pub fn at(socket: PathBuf, credentials: PathBuf) -> Place {
        Place {
            lock: socket.with_extension("lock"),
            socket,
            credentials,
        }
    }

    /// Where a runner's host takes links from its desktop (ADR 0078 §2: the bridge a
    /// connector runs opens it): beside its socket, named as it is with `.link.sock` for
    /// `.sock`, in the same folder that is this user's alone.
    pub fn link(&self) -> PathBuf {
        self.socket.with_extension("link.sock")
    }
}

/// Who a runner's host is on a link and the one desktop it admits (ADR 0078 §3, ruled
/// 2026-10-15), read as each link arrives: `Ok(None)` while it is not paired, which admits no
/// link, and an `Err` when what it keeps cannot be read safely, which admits none either.
pub type Answering = Arc<dyn Fn() -> Result<Option<(Identity, Pinned)>, String> + Send + Sync>;

/// Takes the desktop a link proved out of a runner's peers, as that desktop removes the runner
/// (`unpair`, 1.25, #2525): no later link from it is admitted.
pub type Unpairing = Arc<dyn Fn(&Pinned) -> Result<(), String> + Send + Sync>;

/// Keeps a profile for the desktop a link proved, on the runner (`keep_profile`, 1.43, #2049),
/// and says whether its harness says it is signed in there.
pub type Keeping = Arc<
    dyn Fn(&Pinned, &session::ProfileToKeep) -> Result<session::ProfileStanding, String>
        + Send
        + Sync,
>;

/// Hands a sign-in's redirect to the login waiting on the runner's loopback, and says whether it
/// took it (`sign_in_redirect`, 1.58, #4044).
pub type Delivering = Arc<dyn Fn(&str) -> Result<(), String> + Send + Sync>;

/// A runner's pairing, as its host serves it: who it is and whom it admits ([`Answering`]),
/// how the desktop it admits is taken out of its peers ([`Unpairing`]), how it keeps that
/// desktop's profiles ([`Keeping`]), and how it hands a login that desktop's browser's answer
/// ([`Delivering`]).
#[derive(Clone)]
pub struct Pairing {
    pub answering: Answering,
    pub unpairing: Unpairing,
    pub keeping: Keeping,
    pub delivering: Delivering,
}

/// The lock's file name beside `purlisd.sock`, the socket of a person's own config home.
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

/// Serve the session protocol at `place` until `until` resolves or the window quits (`quit`),
/// then end every program it runs and remove the socket. `ready` is called once the host accepts. `window` is the window
/// that started this host, when one did: its chats, in its process or in this one, run below it.
/// `hosting` holds the programs, and the kill switch as it read it when it was made.
pub async fn serve(
    place: &Place,
    window: Option<Window>,
    hosting: Hosting,
    ready: impl FnOnce(),
    until: impl Future<Output = ()>,
) -> Result<(), HostError> {
    serve_answering(place, window, hosting, None, ready, until).await
}

/// [`serve`], and, for a runner's host, links from the desktop it is paired with on
/// [`Place::link`] (`pairing`): each is the link between hosts (ADR 0078 §3), served as
/// `remote-link` once its handshake proves the desktop's pinned key.
pub async fn serve_answering(
    place: &Place,
    window: Option<Window>,
    hosting: Hosting,
    pairing: Option<Pairing>,
    ready: impl FnOnce(),
    until: impl Future<Output = ()>,
) -> Result<(), HostError> {
    let folder = place.socket.parent().unwrap_or(Path::new("/"));
    let _private = purlis_same_user::PrivateDirectory::open(folder)
        .map_err(unsafe_at("make its folder this user's alone", folder))?;
    let _lock = lock(place)?;
    // What the last host left running, settled before anyone is served (#2007): its lock is
    // ours now, so it is not writing its host record any more.
    let settling = hosting.clone();
    let _ = tokio::task::spawn_blocking(move || settling.take_over_from_the_last_host()).await;
    let listener = bind(&place.socket)?;
    let links = match pairing {
        Some(pairing) => Some((bind(&place.link())?, pairing)),
        None => None,
    };
    let credentials = Arc::new(
        Credentials::mint_into(&place.credentials)
            .map_err(unsafe_at("mint its credentials", &place.credentials))?,
    );
    let hosting = hosting.started_by(window);
    // A stop written elsewhere is heard with no window there, as the host starts and every few
    // seconds while it serves; every start looks first too (#2190).
    let heeding = tokio::spawn(hosting.clone().heed_stops_written_elsewhere());
    let answering_links = links
        .map(|(listener, pairing)| tokio::spawn(answer_links(listener, pairing, hosting.clone())));
    ready();

    let served = accept(&listener, &credentials, &hosting, until).await;
    heeding.abort();
    if let Some(answering_links) = answering_links {
        answering_links.abort();
    }
    // Quitting still ends every chat (ADR 0068 §2), as does the grace period passing.
    let ending = hosting.clone();
    let _ = tokio::task::spawn_blocking(move || ending.end_all()).await;
    // The lock is still held, so the socket at the path is this host's.
    let _ = std::fs::remove_file(&place.socket);
    let _ = std::fs::remove_file(place.link());
    served
}

/// Answers each link on a runner's link socket: as `remote-link`, from the one desktop
/// `pairing` names, through the link between hosts' handshake. Nothing else is served there.
async fn answer_links(listener: Listener, pairing: Pairing, hosting: Hosting) {
    loop {
        let peer = match listener.accept().await {
            Ok(Ok(peer)) => peer,
            Ok(Err(not_this_user)) => {
                say(&format!("a link was refused: {not_this_user}"));
                continue;
            }
            Err(why) => {
                say(&format!("the link socket stopped accepting: {why}"));
                return;
            }
        };
        // What the runner keeps is read off the runtime: two files, read as each link arrives.
        let reading = Arc::clone(&pairing.answering);
        let paired = tokio::task::spawn_blocking(move || reading())
            .await
            .unwrap_or_else(|why| Err(format!("its pairing could not be read: {why}")));
        let (ours, desktop) = match paired {
            Ok(Some(paired)) => paired,
            Ok(None) => {
                say("a link was refused: this runner is not paired with a desktop");
                continue;
            }
            Err(why) => {
                say(&format!("a link was refused: {why}"));
                continue;
            }
        };
        let hosting = hosting.clone();
        let unpairing = Arc::clone(&pairing.unpairing);
        let keeping = Arc::clone(&pairing.keeping);
        let delivering = Arc::clone(&pairing.delivering);
        tokio::spawn(async move {
            let chats = Arc::new(hosting.clone());
            match hostlink::answer_local(
                peer,
                &chats,
                Handshake::THIS_BUILD,
                &ours,
                &desktop,
                Liveness::default(),
            )
            .await
            {
                Ok(answered) => {
                    let unpaired = Arc::new(tokio::sync::Notify::new());
                    let paired = PairedLink {
                        hosting,
                        desktop,
                        unpairing,
                        keeping,
                        delivering,
                        unpaired: Arc::clone(&unpaired),
                    };
                    let serving = session::serve(answered.link, paired, None);
                    tokio::pin!(serving);
                    // A desktop that unpaired is linked no more: its link ends once its
                    // `done` has had time to go out, whether or not it closes the link itself.
                    tokio::select! {
                        _ = &mut serving => {}
                        () = unpaired.notified() => {
                            let _ = tokio::time::timeout(UNPAIRED_LINK_ENDS, &mut serving).await;
                        }
                    }
                }
                Err(refused) => say(&format!("a link was refused: {refused}")),
            }
        });
    }
}

/// The session protocol on a link from the desktop a runner is paired with: the host's own,
/// `unpair` (1.25, #2525), which takes that desktop, the one the handshake proved, out of the
/// runner's peers, `keep_profile` (1.43, #2049), which keeps one of its profiles here, and
/// `sign_in_redirect` (1.58, #4044), which hands a login here its browser's answer.
struct PairedLink {
    hosting: Hosting,
    desktop: Pinned,
    unpairing: Unpairing,
    keeping: Keeping,
    delivering: Delivering,
    /// Told once the desktop is unpaired, so its link ends.
    unpaired: Arc<tokio::sync::Notify>,
}

/// How long the link of a desktop that unpaired stays open after its answer, for the answer to
/// reach it.
const UNPAIRED_LINK_ENDS: std::time::Duration = std::time::Duration::from_secs(2);

impl PairedLink {
    /// `unpair` (1.25): the desktop the handshake proved taken out of the runner's peers.
    async fn unpair(&self) -> Result<session::Answer, session::Refusal> {
        let (unpairing, desktop) = (Arc::clone(&self.unpairing), self.desktop.clone());
        let unpaired = tokio::task::spawn_blocking(move || unpairing(&desktop))
            .await
            .unwrap_or_else(|why| Err(format!("the unpairing did not finish: {why}")));
        match unpaired {
            Ok(()) => {
                say("the desktop this runner was paired with unpaired it");
                self.unpaired.notify_one();
                Ok(session::Answer::Done)
            }
            Err(why) => Err(session::Refusal {
                code: session::NOT_STARTED.into(),
                why,
            }),
        }
    }

    /// `keep_profile` (1.43, #2049): the desktop's profile kept here, off the runtime, since
    /// its harness is asked whether it is signed in.
    async fn keep(
        &self,
        profile: session::ProfileToKeep,
    ) -> Result<session::Answer, session::Refusal> {
        let (keeping, desktop) = (Arc::clone(&self.keeping), self.desktop.clone());
        tokio::task::spawn_blocking(move || keeping(&desktop, &profile))
            .await
            .unwrap_or_else(|why| Err(format!("the profile was not kept: {why}")))
            .map(|standing| session::Answer::ProfileKept { standing })
            .map_err(|why| session::Refusal {
                code: session::NOT_STARTED.into(),
                why,
            })
    }

    /// `sign_in_redirect` (1.58, #4044): the desktop's browser's answer handed to the login
    /// waiting here, off the runtime, since the login signs in with its vendor before it answers.
    async fn deliver(&self, target: String) -> Result<session::Answer, session::Refusal> {
        let delivering = Arc::clone(&self.delivering);
        tokio::task::spawn_blocking(move || delivering(&target))
            .await
            .unwrap_or_else(|why| Err(format!("the redirect was not handed on: {why}")))
            .map(|()| session::Answer::Done)
            .map_err(|why| session::Refusal {
                code: session::NOT_STARTED.into(),
                why,
            })
    }
}

impl session::Host for PairedLink {
    async fn call(
        &self,
        command: session::Command,
        peer: &session::Peer,
    ) -> Result<session::Answer, session::Refusal> {
        // The desktop's stop and re-arm are journalled on the runner by the link (#2051).
        let switching = match command {
            session::Command::StopAll => Some(false),
            session::Command::Rearm => Some(true),
            _ => None,
        };
        if let Some(rearm) = switching {
            let answer = session::Host::call(&self.hosting, command, peer).await;
            if let Ok(session::Answer::KillSwitch(switch)) = &answer {
                let changed = if rearm {
                    !switch.stopped
                } else {
                    switch.stopped && !switch.already
                };
                if changed {
                    self.hosting
                        .switched_by_a_link(rearm, &self.desktop.device)
                        .await;
                }
            }
            return answer;
        }
        match command {
            session::Command::KeepProfile(profile) => self.keep(profile).await,
            session::Command::SignInRedirect { target } => self.deliver(target.0).await,
            session::Command::Unpair => self.unpair().await,
            command => session::Host::call(&self.hosting, command, peer).await,
        }
    }

    async fn elicits_a_secret(&self, chat: &str, ask: &str) -> bool {
        session::Host::elicits_a_secret(&self.hosting, chat, ask).await
    }

    async fn link_ended(&self, peer: &session::Peer) {
        session::Host::link_ended(&self.hosting, peer).await
    }

    async fn subscribe(
        &self,
        since: session::Since,
        peer: &session::Peer,
    ) -> Result<tokio::sync::mpsc::Receiver<session::Pushed>, session::Refusal> {
        session::Host::subscribe(&self.hosting, since, peer).await
    }
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
    hosting: &Hosting,
    until: impl Future<Output = ()>,
) -> Result<(), HostError> {
    tokio::pin!(until);
    loop {
        let accepted = tokio::select! {
            () = &mut until => return Ok(()),
            // The window quit: every program has ended as a quit, and the host stops (#2006).
            () = hosting.until_it_quits() => return Ok(()),
            accepted = listener.accept() => accepted.map_err(HostError::Accept)?,
        };
        match accepted {
            Ok(peer) => {
                let credentials = Arc::clone(credentials);
                let hosting = hosting.clone();
                let pid = peer.pid();
                tokio::spawn(async move {
                    let chats = Arc::new(hosting.clone());
                    match link::serve(peer, session::speaks(), &credentials, &chats).await {
                        Ok(link) => {
                            // The window, back after the one that started this host went: it
                            // holds the chats as that one did (#2006).
                            if link.scope() == Scope::LocalUi && !hosting.a_window_linked_from(pid)
                            {
                                say("a window's link was refused: this host has taken as many \
                                     windows as it holds");
                                return;
                            }
                            // Its first command says whether it is this host's or relayed to
                            // a runner this machine keeps (`relay_to`, 1.33, #2679).
                            let _ =
                                session::serve(link, crate::routing::Routed::new(hosting), None)
                                    .await;
                        }
                        Err(refused) => say(&format!("a connection was refused: {refused}")),
                    }
                });
            }
            Err(not_this_user) => say(&format!("a connection was refused: {not_this_user}")),
        }
    }
}
