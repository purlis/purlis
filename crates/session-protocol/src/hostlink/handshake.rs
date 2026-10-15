// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The handshake that proves both hosts' link keys, and what seals each frame after it.
//!
//! ADR 0078 §3 makes it Noise `XX` (`25519_ChaChaPoly_BLAKE2s`) with the link's prologue: three
//! messages, after the second of which the dialing host knows the answering host's key, and
//! after the third of which the answering host knows the dialer's. This module drives those
//! three messages, checks the answering host's key against its pin before the third is sent,
//! and hands back what seals and opens every frame after.
//!
//! **A build has the handshakes [`Handshake`] names, and no other:** it is a closed set, so no
//! caller can hand the link one of its own. A build of the host links with Noise `XX`
//! ([`Handshake::THIS_BUILD`]), through the `snow` crate ADR 0078 names (ruled 2026-10-15).

use bytes::Bytes;
use futures::{SinkExt, StreamExt};
use tokio::io::{AsyncRead, AsyncWrite};

use super::channel::Messages;
use super::{Identity, LinkRefused, Pinned, PublicKey};

/// Which handshake a link runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handshake {
    /// Noise `XX`, `25519_ChaChaPoly_BLAKE2s` (ADR 0078 §3): what a build of the host links
    /// with ([`Handshake::THIS_BUILD`]).
    NoiseXx,
    /// The crate's stand-in, for its tests and its dependents' tests only (the `any-stream`
    /// feature): the keys cross in the clear and every frame is sealed by nothing. It checks
    /// the pins and binds the prologue as the real handshake does, so what the link does with a
    /// key is tested apart from how the key is proved.
    #[cfg(feature = "any-stream")]
    StandIn,
}

impl Handshake {
    /// The handshake a build of the host links with.
    pub const THIS_BUILD: Handshake = Handshake::NoiseXx;
}

/// One side of a handshake in progress, message by message.
trait Exchange: Send {
    fn write_message(&mut self) -> Result<Vec<u8>, String>;
    fn read_message(&mut self, message: &[u8]) -> Result<(), String>;
    /// The other side's key, once a message has carried it.
    fn remote_key(&self) -> Option<PublicKey>;
    fn into_transport(self: Box<Self>) -> Result<Sealing, String>;
}

/// The Noise protocol a link runs: ADR 0078 §3's pattern and suite. A change of either is a new
/// major version of the link protocol.
pub(super) fn noise_params() -> snow::params::NoiseParams {
    "Noise_XX_25519_ChaChaPoly_BLAKE2s"
        .parse()
        .expect("a protocol name snow knows")
}

/// What seals the frames this end sends, and what opens those it reads.
pub(super) type Sealing = (Box<dyn Seal>, Box<dyn Open>);

/// Seals one frame to send.
pub(super) trait Seal: Send {
    fn seal(&mut self, plain: &[u8]) -> Result<Vec<u8>, String>;
}

/// Opens one frame received; a frame that does not open ends the link.
pub(super) trait Open: Send {
    fn open(&mut self, sealed: &[u8]) -> Result<Vec<u8>, String>;
}

fn begin(
    handshake: Handshake,
    dialing: bool,
    prologue: &[u8],
    ours: &Identity,
) -> Result<Box<dyn Exchange>, LinkRefused> {
    match handshake {
        Handshake::NoiseXx => {
            let private = ours.key.private().ok_or_else(|| {
                LinkRefused::HandshakeFailed("this host's link key has no private half".into())
            })?;
            let failed = |why: snow::Error| LinkRefused::HandshakeFailed(why.to_string());
            let builder = snow::Builder::new(noise_params())
                .local_private_key(private)
                .map_err(failed)?
                .prologue(prologue)
                .map_err(failed)?;
            let state = if dialing {
                builder.build_initiator()
            } else {
                builder.build_responder()
            }
            .map_err(failed)?;
            Ok(Box::new(noise::Exchanging(state)))
        }
        #[cfg(feature = "any-stream")]
        Handshake::StandIn => Ok(Box::new(stand_in::Exchanging::new(
            dialing,
            prologue,
            ours.key.public(),
        ))),
    }
}

/// The dialing host's three messages: it checks the answering host's key against `answering`'s
/// pin before it sends the last.
pub(super) async fn dial<S: AsyncRead + AsyncWrite + Unpin>(
    handshake: Handshake,
    messages: &mut Messages<S>,
    prologue: &[u8],
    ours: &Identity,
    answering: &Pinned,
) -> Result<(Box<dyn Seal>, Box<dyn Open>), LinkRefused> {
    let mut exchange = begin(handshake, true, prologue, ours)?;
    send(messages, exchange.write_message()).await?;
    let second = receive(messages).await?;
    exchange
        .read_message(&second)
        .map_err(LinkRefused::HandshakeFailed)?;
    let offered = exchange
        .remote_key()
        .ok_or_else(|| LinkRefused::HandshakeFailed("no key was offered".into()))?;
    if offered != answering.key {
        return Err(LinkRefused::NotThePinnedHost {
            device: answering.device.clone(),
            offered: offered.fingerprint(),
            pinned: answering.key.fingerprint(),
        });
    }
    send(messages, exchange.write_message()).await?;
    exchange
        .into_transport()
        .map_err(LinkRefused::HandshakeFailed)
}

/// The answering host's three messages, and the dialer's key they proved, which the caller
/// checks against the one desktop it is paired with.
pub(super) async fn answer<S: AsyncRead + AsyncWrite + Unpin>(
    handshake: Handshake,
    messages: &mut Messages<S>,
    prologue: &[u8],
    ours: &Identity,
) -> Result<(Box<dyn Seal>, Box<dyn Open>, PublicKey), LinkRefused> {
    let mut exchange = begin(handshake, false, prologue, ours)?;
    let first = receive(messages).await?;
    exchange
        .read_message(&first)
        .map_err(LinkRefused::HandshakeFailed)?;
    send(messages, exchange.write_message()).await?;
    let third = receive(messages).await?;
    exchange
        .read_message(&third)
        .map_err(LinkRefused::HandshakeFailed)?;
    let offered = exchange
        .remote_key()
        .ok_or_else(|| LinkRefused::HandshakeFailed("no key was offered".into()))?;
    let (seal, open) = exchange
        .into_transport()
        .map_err(LinkRefused::HandshakeFailed)?;
    Ok((seal, open, offered))
}

async fn send<S: AsyncRead + AsyncWrite + Unpin>(
    messages: &mut Messages<S>,
    message: Result<Vec<u8>, String>,
) -> Result<(), LinkRefused> {
    let message = message.map_err(LinkRefused::HandshakeFailed)?;
    messages.send(Bytes::from(message)).await?;
    Ok(())
}

async fn receive<S: AsyncRead + AsyncWrite + Unpin>(
    messages: &mut Messages<S>,
) -> Result<Vec<u8>, LinkRefused> {
    match messages.next().await {
        Some(message) => Ok(message?.to_vec()),
        None => Err(LinkRefused::HandshakeFailed(
            "the other end closed the stream".into(),
        )),
    }
}

/// The public half of the X25519 key whose private half is `private`, as a handshake presents
/// it: what a kept pair is checked against. `snow` derives a static key's public half only
/// inside a handshake, so this runs one in memory with a throwaway key and reads what the other
/// side learned.
pub(super) fn public_half(private: &[u8; 32]) -> Result<PublicKey, String> {
    let fail = |why: snow::Error| why.to_string();
    let builder = || snow::Builder::new(noise_params());
    let throwaway = builder().generate_keypair().map_err(fail)?;
    let mut ours = builder()
        .local_private_key(private)
        .map_err(fail)?
        .build_initiator()
        .map_err(fail)?;
    let mut theirs = builder()
        .local_private_key(&throwaway.private)
        .map_err(fail)?
        .build_responder()
        .map_err(fail)?;
    let (mut message, mut payload) = (vec![0u8; 1024], vec![0u8; 1024]);
    // XX's three messages: ours, theirs, ours.
    for ours_writes in [true, false, true] {
        let (writer, reader) = if ours_writes {
            (&mut ours, &mut theirs)
        } else {
            (&mut theirs, &mut ours)
        };
        let written = writer.write_message(&[], &mut message).map_err(fail)?;
        reader
            .read_message(&message[..written], &mut payload)
            .map_err(fail)?;
    }
    let key: [u8; 32] = theirs
        .get_remote_static()
        .and_then(|key| key.try_into().ok())
        .ok_or("the handshake presented no key")?;
    Ok(PublicKey::from_bytes(key))
}

/// Noise `XX` through `snow`: its handshake state, then its transport state, which seals and
/// opens every frame after.
mod noise {
    use std::sync::{Arc, Mutex};

    use super::{Exchange, Open, PublicKey, Seal, Sealing};

    /// The most a Noise message may be.
    const MOST: usize = 65_535;

    pub(super) struct Exchanging(pub(super) snow::HandshakeState);

    impl Exchange for Exchanging {
        fn write_message(&mut self) -> Result<Vec<u8>, String> {
            let mut message = vec![0u8; MOST];
            let written = self
                .0
                .write_message(&[], &mut message)
                .map_err(|why| why.to_string())?;
            message.truncate(written);
            Ok(message)
        }

        fn read_message(&mut self, message: &[u8]) -> Result<(), String> {
            let mut payload = vec![0u8; MOST];
            self.0
                .read_message(message, &mut payload)
                .map(|_| ())
                .map_err(|why| why.to_string())
        }

        fn remote_key(&self) -> Option<PublicKey> {
            let key: [u8; 32] = self.0.get_remote_static()?.try_into().ok()?;
            Some(PublicKey::from_bytes(key))
        }

        fn into_transport(self: Box<Self>) -> Result<Sealing, String> {
            let transport = Arc::new(Mutex::new(
                self.0
                    .into_transport_mode()
                    .map_err(|why| why.to_string())?,
            ));
            Ok((
                Box::new(Transport(Arc::clone(&transport))),
                Box::new(Transport(transport)),
            ))
        }
    }

    /// One link's transport state: a writer seals with it and a reader opens with it, each its
    /// own direction's key and nonce.
    struct Transport(Arc<Mutex<snow::TransportState>>);

    impl Transport {
        fn state(&self) -> std::sync::MutexGuard<'_, snow::TransportState> {
            self.0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
        }
    }

    impl Seal for Transport {
        fn seal(&mut self, plain: &[u8]) -> Result<Vec<u8>, String> {
            let mut sealed = vec![0u8; MOST];
            let written = self
                .state()
                .write_message(plain, &mut sealed)
                .map_err(|why| why.to_string())?;
            sealed.truncate(written);
            Ok(sealed)
        }
    }

    impl Open for Transport {
        fn open(&mut self, sealed: &[u8]) -> Result<Vec<u8>, String> {
            let mut plain = vec![0u8; MOST];
            let read = self
                .state()
                .read_message(sealed, &mut plain)
                .map_err(|why| why.to_string())?;
            plain.truncate(read);
            Ok(plain)
        }
    }
}

#[cfg(feature = "any-stream")]
mod stand_in {
    use super::{Exchange, Open, PublicKey, Seal, Sealing};

    /// The stand-in's messages: the first says nothing, the second and third each carry their
    /// sender's key and its prologue, which the reader holds to its own.
    pub(super) struct Exchanging {
        dialing: bool,
        prologue: Vec<u8>,
        ours: PublicKey,
        theirs: Option<PublicKey>,
        written: usize,
    }

    impl Exchanging {
        pub(super) fn new(dialing: bool, prologue: &[u8], ours: PublicKey) -> Exchanging {
            Exchanging {
                dialing,
                prologue: prologue.to_vec(),
                ours,
                theirs: None,
                written: 0,
            }
        }
    }

    impl Exchange for Exchanging {
        fn write_message(&mut self) -> Result<Vec<u8>, String> {
            self.written += 1;
            if self.dialing && self.written == 1 {
                return Ok(b"stand-in".to_vec());
            }
            let mut message = self.ours.as_bytes().to_vec();
            message.extend_from_slice(&self.prologue);
            Ok(message)
        }

        fn read_message(&mut self, message: &[u8]) -> Result<(), String> {
            if !self.dialing && self.theirs.is_none() && message == b"stand-in" {
                return Ok(());
            }
            let (key, prologue) = message
                .split_at_checked(32)
                .ok_or("a message too short to carry a key")?;
            if prologue != self.prologue.as_slice() {
                return Err("the two ends bind different links".into());
            }
            let key: [u8; 32] = key.try_into().map_err(|_| "a key of the wrong size")?;
            self.theirs = Some(PublicKey::from_bytes(key));
            Ok(())
        }

        fn remote_key(&self) -> Option<PublicKey> {
            self.theirs
        }

        fn into_transport(self: Box<Self>) -> Result<Sealing, String> {
            Ok((Box::new(Clear), Box::new(Clear)))
        }
    }

    struct Clear;

    impl Seal for Clear {
        fn seal(&mut self, plain: &[u8]) -> Result<Vec<u8>, String> {
            Ok(plain.to_vec())
        }
    }

    impl Open for Clear {
        fn open(&mut self, sealed: &[u8]) -> Result<Vec<u8>, String> {
            Ok(sealed.to_vec())
        }
    }
}
