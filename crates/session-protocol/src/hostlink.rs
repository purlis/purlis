// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The link between two session hosts (ADR 0078 §3, #2042): how one host reaches another
//! through a connector's byte stream, proves which host is at the far end, keeps the link alive
//! and links again when it drops. The session protocol runs inside it unchanged, as
//! `remote-link` ([`crate::link::serve_as_a_device`]).
//!
//! **Four steps, in order, each failing closed:**
//!
//! 1. **The link protocol's own version**, in the clear, before anything else: [`MAGIC`], then
//!    the same hello and answer [`crate::version`] negotiates the session protocol with. Its
//!    version is [`VERSION`], apart from the session protocol's: a minor of either moves
//!    without the other. **Hosts link only within one major version** ([`speaks`] names one
//!    major, never the one before it), and a host of another major is refused in words that
//!    say which side to update ([`LinkRefused::Majors`]).
//! 2. **The handshake**, Noise `XX` with `25519_ChaChaPoly_BLAKE2s` through the `snow` crate,
//!    which proves both hosts' link keys ([`Handshake`]). The dialing host
//!    checks the other host's key against the one it pinned for that host, before it sends
//!    its own last message; the answering host checks the dialer's key against its one peer,
//!    the desktop it is paired with, which it trusts as it trusts the window (ADR 0078, ruled
//!    2026-10-15). A key that is not pinned is refused, and the refusal names its
//!    fingerprint, never the key ([`PublicKey::fingerprint`]). The handshake binds both
//!    hellos, the version agreed and the answering host's device id ([`prologue`]), so a
//!    connector that reaches another host than the one the dialer paired with, or a hello
//!    changed on its way, fails the handshake.
//! 3. **The answering host's word**, the first sealed frame: admitted, or refused and why, so
//!    the dialer can say why it was turned away.
//! 4. **Frames**: everything after is sealed frames, each a kind and its bytes.
//!    The session protocol's bytes are one kind; the other two are the link's own **ping**
//!    and its answer. Each end pings every [`Liveness::ping_every`] and ends the link when it
//!    has heard nothing at all for [`Liveness::deadline`]: a link that dropped ends with its
//!    transport, and one that is half-open (open, carrying nothing) ends within the deadline.
//!
//! **The dialing host keeps the link up** ([`keep_linked`]): it links again on its own after a
//! drop, with a growing pause between tries, and links anew ahead of any limit on how long a
//! connector's stream may live ([`Liveness::renew_after`]), opening the new link before it
//! closes the old one, which it closes once its reader has moved to the new one
//! ([`Links::moved`]) or the ping's deadline has passed. Each new link is the session protocol from its start: the client
//! subscribes again from its cursor and attaches its views again, each with a snapshot
//! (ADR 0068 §4). The answering host never dials (ADR 0078 §4: the desktop is always the
//! client).
//!
//! **What crosses** is what `remote-link`'s row of [`crate::grants`] grants, checked by
//! [`crate::session::serve`] on the answering host like any other scope's: never what only
//! purlis's window may send.

mod channel;
mod handshake;
mod keep;

use std::time::Duration;

use sha2::{Digest, Sha256};
use tokio::io::{AsyncRead, AsyncWrite};
use zeroize::Zeroize;

use crate::link::{self, Link, LinkError, ProvenDevice};
use crate::session;
use crate::version::{self, Refused, Speaks, Version};

pub use handshake::Handshake;
pub use keep::{Connector, Down, FarEndGone, KeyNoLongerHeld, Linked, Links, keep_linked};
/// What holds a link key's private half: wiped when dropped.
pub use zeroize::Zeroizing;

/// The link protocol's version: its own, apart from the session protocol's
/// ([`crate::session::VERSION`]). A minor only adds; a new major is a break, and hosts of two
/// majors do not link.
///
/// 1.0: the hello, the handshake, the answering host's word, and frames of three kinds (the
/// session protocol's bytes, a ping and its answer).
pub const VERSION: Version = Version { major: 1, minor: 0 };

/// What a host offers and answers with: [`VERSION`] alone, so two hosts link only within one
/// major version.
pub fn speaks() -> Speaks {
    Speaks::new([VERSION])
}

/// What every message of the link's hello opens with: PNG's trick, as the session protocol's
/// [`crate::version::MAGIC`], with the letters of a host link.
pub const MAGIC: &[u8; 8] = b"\x89PHL\r\n\x1a\n";

/// The public half of a host's link key (ADR 0078 §3): what a pairing pins. It is shown only
/// as its fingerprint.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct PublicKey([u8; 32]);

impl PublicKey {
    pub fn from_bytes(bytes: [u8; 32]) -> PublicKey {
        PublicKey(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// How a person and a log tell keys apart: `sha256:` and the first eight bytes of the
    /// key's SHA-256, in hex.
    pub fn fingerprint(&self) -> String {
        let digest = Sha256::digest(self.0);
        let hex: String = digest[..8].iter().map(|b| format!("{b:02x}")).collect();
        format!("sha256:{hex}")
    }
}

/// Shows the fingerprint, never the key.
impl std::fmt::Debug for PublicKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PublicKey({})", self.fingerprint())
    }
}

/// A host's link key, the pair (ADR 0078 §3): a static X25519 key. Its private half never
/// leaves it, nothing prints it, and it is wiped from memory when the key is dropped (each
/// clone's own copy too).
#[derive(Clone)]
pub struct LinkKey {
    public: PublicKey,
    /// `None` only for a key of the crate's stand-in handshake, which holds no private half.
    private: Option<Zeroizing<[u8; 32]>>,
}

impl LinkKey {
    /// A new key, from the operating system's generator.
    pub fn generate() -> Result<LinkKey, LinkRefused> {
        let mut pair = snow::Builder::new(handshake::noise_params())
            .generate_keypair()
            .map_err(|why| LinkRefused::HandshakeFailed(format!("no key was made: {why}")))?;
        let made = (|| {
            let private: [u8; 32] = pair.private.as_slice().try_into().ok()?;
            let public: [u8; 32] = pair.public.as_slice().try_into().ok()?;
            Some(LinkKey {
                public: PublicKey::from_bytes(public),
                private: Some(Zeroizing::new(private)),
            })
        })();
        pair.private.zeroize();
        made.ok_or_else(|| LinkRefused::HandshakeFailed("a key of the wrong size".into()))
    }

    /// A key from its kept private half, its public half as the handshake presents it.
    pub fn from_private(private: Zeroizing<[u8; 32]>) -> Result<LinkKey, LinkRefused> {
        let public = handshake::public_half(&private).map_err(LinkRefused::HandshakeFailed)?;
        Ok(LinkKey {
            public,
            private: Some(private),
        })
    }

    /// A key as it was kept: its private half and its public half, together. Refused when the
    /// public half is not the private half's, since a pairing would then pin a key the
    /// handshake never presents.
    pub fn from_parts(
        private: Zeroizing<[u8; 32]>,
        public: PublicKey,
    ) -> Result<LinkKey, LinkRefused> {
        let key = LinkKey::from_private(private)?;
        if key.public != public {
            return Err(LinkRefused::HandshakeFailed(
                "the kept link key's public half is not its private half's".into(),
            ));
        }
        Ok(key)
    }

    /// A key the crate's stand-in handshake takes ([`Handshake::StandIn`]), for tests only: a
    /// public half and no private one.
    #[cfg(feature = "any-stream")]
    pub fn stand_in(public: PublicKey) -> LinkKey {
        LinkKey {
            public,
            private: None,
        }
    }

    /// Its public half, what a pairing pins.
    pub fn public(&self) -> PublicKey {
        self.public
    }

    /// A copy of its private half, wiped when dropped, for the host that keeps the key where
    /// only it may read it ([`LinkKey::from_private`] takes it back); `None` for a stand-in.
    pub fn private_half(&self) -> Option<Zeroizing<[u8; 32]>> {
        self.private.clone()
    }

    /// Its private half, for the handshake alone.
    fn private(&self) -> Option<&[u8; 32]> {
        self.private.as_deref()
    }
}

/// Shows the public half's fingerprint, never a key.
impl std::fmt::Debug for LinkKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LinkKey")
            .field("public", &self.public)
            .finish_non_exhaustive()
    }
}

/// Who a host is on a link: its device id (ADR 0066) and its link key.
#[derive(Debug)]
pub struct Identity {
    pub device: String,
    pub key: LinkKey,
}

/// A host pinned at pairing: its device id and its link key's public half.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pinned {
    pub device: String,
    pub key: PublicKey,
}

/// How a link keeps itself alive, and how long one may live.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Liveness {
    /// How often each end pings.
    pub ping_every: Duration,
    /// How long an end hears nothing at all before it ends the link as dead: well past
    /// [`Liveness::ping_every`], so a ping or two lost in a slow moment does not end it.
    pub deadline: Duration,
    /// After how long the dialing host links anew, ahead of a limit on how long a connector's
    /// stream may live; `None` for a connector with no such limit.
    pub renew_after: Option<Duration>,
}

impl Default for Liveness {
    /// A ping every 10 seconds, dead after 30 seconds of silence, renewed every hour: far
    /// inside the four-hour idle limit a Kubernetes node puts on an exec stream by default
    /// (`streamingConnectionIdleTimeout`), which the pings alone already keep from idling.
    fn default() -> Liveness {
        Liveness {
            ping_every: Duration::from_secs(10),
            deadline: Duration::from_secs(30),
            renew_after: Some(Duration::from_secs(60 * 60)),
        }
    }
}

/// Why a link did not come up, in words a person reads.
#[derive(Debug, thiserror::Error)]
pub enum LinkRefused {
    /// The other end does not open with [`MAGIC`].
    #[error("the other end is not a purlis session host's link")]
    NotALink,
    /// The hosts share no major version of the link protocol.
    #[error(
        "this host links with major version {ours:?} of the link protocol and the other host \
         with {theirs:?}; hosts link only within one major version, so update purlis on the \
         older host"
    )]
    Majors { ours: Vec<u16>, theirs: Vec<u16> },
    /// The handshake failed: the other host is not the one this link was paired with, or it
    /// speaks the handshake otherwise.
    #[error(
        "the encrypted handshake failed ({0}): the other end is not the host this link was \
         paired with"
    )]
    HandshakeFailed(String),
    /// The answering host's key is not the one pinned for it.
    #[error(
        "host {device} offered link key {offered}, not the key {pinned} pinned when it was \
         paired, so the link is refused"
    )]
    NotThePinnedHost {
        device: String,
        offered: String,
        pinned: String,
    },
    /// The dialing host's key is not that of the desktop the answering host is paired with.
    #[error(
        "a host offered link key {offered}, which is not the key of the desktop this host is \
         paired with, so the link is refused"
    )]
    UnknownPeer { offered: String },
    /// The connection to a runner's link socket came from inside one of its chats, or whether
    /// it did could not be read: a chat never links as a device.
    #[error("a link from inside a chat is refused: {0}")]
    InsideAChat(String),
    /// The answering host refused this link, and said why.
    #[error("the other host refused this link: {0}")]
    ByThePeer(String),
    /// The other end did not finish within [`crate::version::HANDSHAKE_TIMEOUT`].
    #[error(
        "the other end did not finish linking within {:?}",
        crate::version::HANDSHAKE_TIMEOUT
    )]
    TimedOut,
    /// Something in the hello that is not the link protocol's shape.
    #[error("the link's hello could not be read: {0}")]
    Hello(String),
    /// The session protocol inside the link did not start.
    #[error(transparent)]
    Session(#[from] LinkError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl LinkRefused {
    fn from_hello(refused: Refused) -> LinkRefused {
        match refused {
            Refused::NotThisProtocol => LinkRefused::NotALink,
            Refused::NoSharedMajor { ours, theirs } => LinkRefused::Majors { ours, theirs },
            Refused::ByPeer { speaks: theirs } => LinkRefused::Majors {
                ours: speaks().majors(),
                theirs,
            },
            Refused::TimedOut => LinkRefused::TimedOut,
            Refused::Io(e) => LinkRefused::Io(e),
            other => LinkRefused::Hello(other.to_string()),
        }
    }
}

/// What a link's handshake binds besides the keys: the link protocol, the versions the hello
/// offered and the one agreed, and the answering host's device id, as the dialer expects it and
/// as that host says it is. Each end builds it from what it sent and read, so a hello changed
/// on its way gives the two ends different prologues, and the handshake fails.
pub fn prologue(offered: &[Version], agreed: Version, answering: &str) -> Vec<u8> {
    let offered: Vec<String> = offered
        .iter()
        .map(|version| format!("{}.{}", version.major, version.minor))
        .collect();
    format!(
        "purlis host link; offered {}; agreed {}.{}; to {answering}",
        offered.join(","),
        agreed.major,
        agreed.minor
    )
    .into_bytes()
}

/// A link the answering host admitted: the session protocol on it, as `remote-link`, and the
/// device it is from.
pub struct Answered {
    pub link: Link,
    pub device: String,
}

/// The dialing host's end, on a connector's stream `io`: link to `answering`, the host pinned at
/// pairing, as `ours`, and speak the session protocol inside.
pub async fn connect<S: AsyncRead + AsyncWrite + Unpin + Send + 'static>(
    io: S,
    handshake: Handshake,
    ours: &Identity,
    answering: &Pinned,
    liveness: Liveness,
) -> Result<Link, LinkRefused> {
    connect_kept(io, handshake, ours, answering, liveness)
        .await
        .map(|(link, _)| link)
}

/// [`connect`], with the handle that tells when the link ends and ends it on purpose.
pub(crate) async fn connect_kept<S: AsyncRead + AsyncWrite + Unpin + Send + 'static>(
    io: S,
    handshake: Handshake,
    ours: &Identity,
    answering: &Pinned,
    liveness: Liveness,
) -> Result<(Link, channel::Ends), LinkRefused> {
    let (stream, ends) = within_the_handshake(async {
        let (version, offered, io) = version::offer_opening_with(io, &speaks(), MAGIC)
            .await
            .map_err(LinkRefused::from_hello)?;
        let mut messages = channel::framed(io);
        let (seal, mut open) = handshake::dial(
            handshake,
            &mut messages,
            &prologue(&offered, version, &answering.device),
            ours,
            answering,
        )
        .await?;
        channel::word(&mut messages, open.as_mut()).await?;
        Ok(channel::run(messages, seal, open, liveness))
    })
    .await?;
    let link = link::connect_proven(stream, session::speaks()).await?;
    Ok((link, ends))
}

/// The answering host's end, on a connector's stream `io`: admit `desktop`, the one peer this
/// host is paired with, as `ours`, and serve the session protocol inside as `remote-link`.
///
/// **A runner has one peer** (ADR 0078, ruled 2026-10-15). It cannot tell a command the
/// desktop's relay sent on for the window from one its peer composed, so it trusts its peer as
/// it trusts the window: to compose what it starts and to answer its chats' asks. That trust
/// goes to the one desktop that relays to it, and to no other device.
pub async fn answer<S: AsyncRead + AsyncWrite + Unpin + Send + 'static>(
    io: S,
    handshake: Handshake,
    ours: &Identity,
    desktop: &Pinned,
    liveness: Liveness,
) -> Result<Answered, LinkRefused> {
    let (stream, device) = within_the_handshake(async {
        let (version, offered_versions, io) = version::answer_opening_with(io, &speaks(), MAGIC)
            .await
            .map_err(LinkRefused::from_hello)?;
        let mut messages = channel::framed(io);
        let (seal, open, offered) = handshake::answer(
            handshake,
            &mut messages,
            &prologue(&offered_versions, version, &ours.device),
            ours,
        )
        .await?;
        if offered != desktop.key {
            let refused = LinkRefused::UnknownPeer {
                offered: offered.fingerprint(),
            };
            channel::refuse(messages, seal, &refused.to_string()).await;
            return Err(refused);
        }
        let device = desktop.device.clone();
        let (stream, _ends) = channel::admit(messages, seal, open, liveness).await?;
        Ok((stream, device))
    })
    .await?;
    let link = link::serve_as_a_device(stream, session::speaks(), ProvenDevice::proven()).await?;
    Ok(Answered { link, device })
}

/// [`answer`], on a connection to the runner's own link socket, which the bridge a connector
/// runs opens (ADR 0078 §2): a peer that is not this host's user never reaches here
/// ([`crate::local::Listener`]), and one inside a chat `chats` holds is closed before a byte
/// of it is read, as the host's own socket refuses it every scope.
#[cfg(unix)]
pub async fn answer_local(
    peer: crate::local::SameUser,
    chats: &std::sync::Arc<impl crate::local::Chats>,
    handshake: Handshake,
    ours: &Identity,
    desktop: &Pinned,
    liveness: Liveness,
) -> Result<Answered, LinkRefused> {
    let pid = peer.pid();
    let verdict = crate::local::inside_a_chat(pid, std::sync::Arc::clone(chats)).await;
    if let Some(refused) = refused_inside(pid, verdict) {
        // Dropped unread: not a byte of the connection is taken in.
        return Err(refused);
    }
    answer(peer.into_stream(), handshake, ours, desktop, liveness).await
}

/// Why a connection from process `pid` to a runner's link socket is refused before a byte of it
/// is read, given what was found of where it runs (`verdict`): inside a chat, or a doubt about
/// it, which is refused the same way; `None` admits it to the handshake.
#[cfg(unix)]
fn refused_inside(pid: u32, verdict: std::io::Result<Option<u32>>) -> Option<LinkRefused> {
    match verdict {
        Ok(None) => None,
        Ok(Some(chat)) => Some(LinkRefused::InsideAChat(format!(
            "process {pid} is inside chat program {chat}"
        ))),
        Err(why) => Some(LinkRefused::InsideAChat(format!(
            "whether process {pid} is inside a chat could not be read ({why})"
        ))),
    }
}

/// One deadline over the hello, the handshake and the answering host's word.
async fn within_the_handshake<T>(
    linking: impl Future<Output = Result<T, LinkRefused>>,
) -> Result<T, LinkRefused> {
    tokio::time::timeout(version::HANDSHAKE_TIMEOUT, linking)
        .await
        .unwrap_or(Err(LinkRefused::TimedOut))
}

#[cfg(all(test, unix))]
mod tests;
