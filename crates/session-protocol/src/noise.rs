//! A device link's Noise `XX` handshake (ADR 0078 §3, iOS design §4): both static keys are
//! proved inside the stream, against what each side pinned, before any transport message.
//!
//! `XX` sends each side's static key encrypted inside the handshake: the host learns the
//! device's key in the third message and looks it up in its peers; the device compares the
//! host's key, from the second message, with the one it pinned at pairing (or read from the QR
//! code). Either side aborts on a mismatch before a transport message. The prologue binds the
//! protocol's name and the host's device id, so a link to a different host than the one named
//! fails even when that host would accept the key.
//!
//! **On the wire,** each Noise message is one frame: a big-endian `u16` length, then the
//! message. After the handshake every frame is one Noise transport message holding up to
//! [`MOST_PLAINTEXT`] bytes of the session protocol's own bytes ([`pump`]).
//!
//! This module never logs. [`Identity`]'s `Debug` prints the public key's fingerprint only.

use std::sync::{Arc, Mutex};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, DuplexStream};

use crate::auth::Scope;
use crate::link::ProvenDevice;

/// The Noise pattern and cipher suite (ADR 0078 §3). A change of suite is a protocol version.
pub const PATTERN: &str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";

/// The first part of the prologue: which protocol, which version of its handshake.
pub const PROTOCOL: &str = "purlis session protocol 1";

/// The most a Noise message holds, tag included.
const MOST_MESSAGE: usize = 65_535;

/// The most plaintext one transport message carries: the message less its 16-byte tag.
pub const MOST_PLAINTEXT: usize = MOST_MESSAGE - 16;

/// A device's static X25519 key pair. The private half never leaves this struct in the clear
/// except through [`Identity::private_hex`], for the Keyring.
#[derive(Clone)]
pub struct Identity {
    private: [u8; 32],
    public: [u8; 32],
}

impl Identity {
    /// A fresh key pair from the operating system's generator.
    pub fn generate() -> Identity {
        let pair = snow::Builder::new(pattern())
            .generate_keypair()
            .expect("the OS generator works");
        let mut private = [0u8; 32];
        private.copy_from_slice(&pair.private);
        let mut public = [0u8; 32];
        public.copy_from_slice(&pair.public);
        Identity { private, public }
    }

    /// The key pair whose private half is `private`, as read back from the Keyring.
    pub fn from_private(private: [u8; 32]) -> Result<Identity, Error> {
        let public = x25519_public(&private)?;
        Ok(Identity { private, public })
    }

    pub fn public(&self) -> [u8; 32] {
        self.public
    }

    /// The private half as 64 hex digits: what the Keyring item holds.
    pub fn private_hex(&self) -> String {
        hex(&self.private)
    }
}

impl std::fmt::Debug for Identity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Identity({})", fingerprint(&self.public))
    }
}

/// The public key from a private one, through snow's own resolver so there is one X25519 in
/// the crate.
fn x25519_public(private: &[u8; 32]) -> Result<[u8; 32], Error> {
    use snow::resolvers::{CryptoResolver, DefaultResolver};
    let mut dh = DefaultResolver
        .resolve_dh(&snow::params::DHChoice::Curve25519)
        .ok_or(Error::Prologue)?;
    dh.set(private);
    let mut public = [0u8; 32];
    public.copy_from_slice(dh.pubkey());
    Ok(public)
}

/// A key's fingerprint, as the window and the audit show it: the first 16 hex digits of its
/// SHA-256.
pub fn fingerprint(public: &[u8; 32]) -> String {
    use sha2::{Digest, Sha256};
    hex(&Sha256::digest(public))[..16].to_owned()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn pattern() -> snow::params::NoiseParams {
    PATTERN.parse().expect("the pattern is well formed")
}

/// Why a handshake, or the pump, failed. `Debug` and `Display` carry keys only as
/// fingerprints.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("noise: {0}")]
    Noise(#[from] snow::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("the peer sent a message of {0} bytes, more than a Noise message holds")]
    TooLong(usize),
    /// The host's static key is not the one this device pinned.
    #[error(
        "the host's key is {} and this device pinned {}: not the host that was paired",
        fingerprint(offered),
        fingerprint(expected)
    )]
    WrongHost {
        expected: [u8; 32],
        offered: [u8; 32],
    },
    /// The handshake's prologue, or its key, could not be set up.
    #[error("the handshake could not be set up")]
    Prologue,
}

fn prologue(host_device: &str) -> Vec<u8> {
    let mut p = Vec::with_capacity(PROTOCOL.len() + 1 + host_device.len());
    p.extend_from_slice(PROTOCOL.as_bytes());
    p.push(0);
    p.extend_from_slice(host_device.as_bytes());
    p
}

fn builder<'a>(ours: &'a Identity, prologue: &'a [u8]) -> Result<snow::Builder<'a>, Error> {
    Ok(snow::Builder::new(pattern())
        .local_private_key(&ours.private)?
        .prologue(prologue)?)
}

/// One framed Noise message out: one buffer, one write, so no length waits for its body.
async fn send_frame<S: AsyncWrite + Unpin>(io: &mut S, message: &[u8]) -> Result<(), Error> {
    if message.len() > MOST_MESSAGE {
        return Err(Error::TooLong(message.len()));
    }
    let mut frame = Vec::with_capacity(2 + message.len());
    frame.extend_from_slice(&(message.len() as u16).to_be_bytes());
    frame.extend_from_slice(message);
    io.write_all(&frame).await?;
    io.flush().await?;
    Ok(())
}

async fn read_frame<S: AsyncRead + Unpin>(io: &mut S) -> Result<Vec<u8>, Error> {
    let mut len = [0u8; 2];
    io.read_exact(&mut len).await?;
    let mut message = vec![0u8; usize::from(u16::from_be_bytes(len))];
    io.read_exact(&mut message).await?;
    Ok(message)
}

/// A stream whose handshake is done: the other side's static key is known, and transport
/// messages may flow. Made only by [`accept`] and [`connect`].
pub struct Secured<S> {
    io: S,
    remote_public: [u8; 32],
    transport: snow::TransportState,
}

impl<S> Secured<S> {
    /// The other side's static key, as the handshake delivered it.
    pub fn remote_public(&self) -> [u8; 32] {
        self.remote_public
    }

    /// The wire, given back raw: for a host that drops an unpinned key, and for tests.
    pub fn into_io(self) -> S {
        self.io
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin + Send + 'static> Secured<S> {
    /// The host's word that the key is pinned as `scope`: the proof the link is served under,
    /// and the plaintext end of the encrypted stream.
    pub fn proven_as(self, scope: Scope) -> (ProvenDevice, DuplexStream) {
        let device = ProvenDevice::proven(scope, self.remote_public);
        (device, pump(self))
    }
}

/// The host's side: run `XX` as the responder with `ours`, binding `host_device` (this host's
/// device id) into the prologue, and return the stream with the device's static key known.
/// Whether that key is pinned is the caller's next question; nothing of the session protocol
/// has crossed yet.
pub async fn accept<S: AsyncRead + AsyncWrite + Unpin>(
    mut io: S,
    ours: &Identity,
    host_device: &str,
) -> Result<Secured<S>, Error> {
    let prologue = prologue(host_device);
    let mut hs = builder(ours, &prologue)?.build_responder()?;
    let mut buf = vec![0u8; MOST_MESSAGE];
    // <- e
    let m = read_frame(&mut io).await?;
    hs.read_message(&m, &mut buf)?;
    // -> e, ee, s, es
    let n = hs.write_message(&[], &mut buf)?;
    send_frame(&mut io, &buf[..n]).await?;
    // <- s, se
    let m = read_frame(&mut io).await?;
    hs.read_message(&m, &mut buf)?;
    finish(io, hs)
}

/// The device's side: run `XX` as the initiator with `ours`, binding `host_device` (the host's
/// device id as this device expects it) into the prologue, and abort before any transport
/// message when the host's static key is not `expected_host`. `None` accepts any host key: for
/// the first link of a pairing, where the QR code gave the key and the caller compares it.
pub async fn connect<S: AsyncRead + AsyncWrite + Unpin>(
    mut io: S,
    ours: &Identity,
    host_device: &str,
    expected_host: Option<[u8; 32]>,
) -> Result<Secured<S>, Error> {
    let prologue = prologue(host_device);
    let mut hs = builder(ours, &prologue)?.build_initiator()?;
    let mut buf = vec![0u8; MOST_MESSAGE];
    // -> e
    let n = hs.write_message(&[], &mut buf)?;
    send_frame(&mut io, &buf[..n]).await?;
    // <- e, ee, s, es
    let m = read_frame(&mut io).await?;
    hs.read_message(&m, &mut buf)?;
    if let Some(expected) = expected_host {
        let offered = remote_of(&hs);
        if offered != expected {
            return Err(Error::WrongHost { expected, offered });
        }
    }
    // -> s, se
    let n = hs.write_message(&[], &mut buf)?;
    send_frame(&mut io, &buf[..n]).await?;
    finish(io, hs)
}

fn remote_of(hs: &snow::HandshakeState) -> [u8; 32] {
    let mut key = [0u8; 32];
    key.copy_from_slice(
        hs.get_remote_static()
            .expect("XX has delivered the remote static key by now"),
    );
    key
}

fn finish<S>(io: S, hs: snow::HandshakeState) -> Result<Secured<S>, Error> {
    let remote_public = remote_of(&hs);
    Ok(Secured {
        io,
        remote_public,
        transport: hs.into_transport_mode()?,
    })
}

/// Pump plaintext between the returned stream and the wire, encrypting on the way out and
/// decrypting on the way in: two tasks, one per direction, both ending when either side closes
/// or a frame cannot be read. The transport state is behind a mutex that is never held across
/// an await.
pub fn pump<S>(secured: Secured<S>) -> DuplexStream
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let (ours, theirs) = tokio::io::duplex(256 * 1024);
    let transport = Arc::new(Mutex::new(secured.transport));
    let (mut wire_r, mut wire_w) = tokio::io::split(secured.io);
    let (mut plain_r, mut plain_w) = tokio::io::split(theirs);

    let out = Arc::clone(&transport);
    let outbound = tokio::spawn(async move {
        let mut plain = vec![0u8; MOST_PLAINTEXT];
        let mut cipher = vec![0u8; MOST_MESSAGE];
        loop {
            let n = match plain_r.read(&mut plain).await {
                Ok(0) | Err(_) => break,
                Ok(n) => n,
            };
            let len = {
                let mut t = out.lock().unwrap_or_else(|p| p.into_inner());
                match t.write_message(&plain[..n], &mut cipher) {
                    Ok(len) => len,
                    Err(_) => break,
                }
            };
            if send_frame(&mut wire_w, &cipher[..len]).await.is_err() {
                break;
            }
        }
        let _ = wire_w.shutdown().await;
    });
    tokio::spawn(async move {
        let mut plain = vec![0u8; MOST_MESSAGE];
        loop {
            let frame = match read_frame(&mut wire_r).await {
                Ok(frame) => frame,
                Err(_) => break,
            };
            let n = {
                let mut t = transport.lock().unwrap_or_else(|p| p.into_inner());
                match t.read_message(&frame, &mut plain) {
                    Ok(n) => n,
                    Err(_) => break,
                }
            };
            if plain_w.write_all(&plain[..n]).await.is_err() {
                break;
            }
        }
        let _ = plain_w.shutdown().await;
        outbound.abort();
    });
    ours
}
