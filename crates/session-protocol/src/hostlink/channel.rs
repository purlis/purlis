// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The link's frames, after the handshake: each sealed, each one kind and its bytes.
//!
//! **On the wire** a frame is a big-endian `u16` length, then that many sealed bytes
//! (tokio-util's `LengthDelimitedCodec`, as the handshake's messages are, and the most a Noise
//! message may be). Unsealed, its first byte is its kind:
//!
//! | Kind | Byte | What follows |
//! |---|---|---|
//! | the session protocol's bytes | `0` | at most [`MOST_DATA_BYTES`] of them |
//! | a ping | `1` | nothing |
//! | a ping's answer | `2` | nothing |
//! | the answering host admits the link | `3` | nothing |
//! | the answering host refuses it | `4` | why, in UTF-8 |
//!
//! A kind this version does not know ends the link: it fails closed.
//!
//! **Liveness.** Each end pings every [`super::Liveness::ping_every`] and answers each ping it
//! is sent; pings and answers go ahead of the session protocol's bytes still waiting. Every
//! frame is word that the other end is there, so an end that has read nothing at all for
//! [`super::Liveness::deadline`] ends the link, which is how a half-open link is noticed. A
//! link ends at once when its transport closes, or when the session protocol inside it closes.

use bytes::Bytes;
use futures::{SinkExt, StreamExt};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, DuplexStream, duplex};
use tokio::sync::{mpsc, oneshot};
use tokio::task::AbortHandle;
use tokio_util::codec::{Framed, LengthDelimitedCodec};

use super::handshake::{Open, Seal};
use super::{LinkRefused, Liveness};

/// The handshake's messages and then the frames, on the connector's stream.
pub(crate) type Messages<S> = Framed<S, LengthDelimitedCodec>;

/// The most bytes of the session protocol one frame carries: small, so a ping waits behind
/// little.
pub const MOST_DATA_BYTES: usize = 16 * 1024;

const DATA: u8 = 0;
const PING: u8 = 1;
const PONG: u8 = 2;
const ADMITTED: u8 = 3;
const REFUSED: u8 = 4;

pub(super) fn framed<S: AsyncRead + AsyncWrite>(io: S) -> Messages<S> {
    LengthDelimitedCodec::builder()
        .length_field_type::<u16>()
        .max_frame_length(usize::from(u16::MAX))
        .new_framed(io)
}

/// What the dialing host keeps of a link: word when it ends, and a way to end it.
pub(crate) struct Ends {
    /// Why it ended, once it has; closed without a word when it was retired.
    pub ended: oneshot::Receiver<String>,
    /// Ends it now: the session protocol inside sees its stream close.
    pub retire: AbortHandle,
}

/// The dialing host reads the answering host's word: admitted, or refused and why.
pub(super) async fn word<S: AsyncRead + AsyncWrite + Unpin>(
    messages: &mut Messages<S>,
    open: &mut dyn Open,
) -> Result<(), LinkRefused> {
    let frame = match messages.next().await {
        Some(frame) => frame?,
        None => {
            return Err(LinkRefused::HandshakeFailed(
                "the other end closed the stream".into(),
            ));
        }
    };
    let plain = open.open(&frame).map_err(LinkRefused::HandshakeFailed)?;
    match plain.split_first() {
        Some((&ADMITTED, [])) => Ok(()),
        Some((&REFUSED, why)) => Err(LinkRefused::ByThePeer(
            String::from_utf8_lossy(why).into_owned(),
        )),
        _ => Err(LinkRefused::HandshakeFailed(
            "the other host's word is not one this link knows".into(),
        )),
    }
}

/// The answering host admits the link, and carries it.
pub(super) async fn admit<S: AsyncRead + AsyncWrite + Unpin + Send + 'static>(
    mut messages: Messages<S>,
    mut seal: Box<dyn Seal>,
    open: Box<dyn Open>,
    liveness: Liveness,
) -> Result<(DuplexStream, Ends), LinkRefused> {
    send(&mut messages, seal.as_mut(), ADMITTED, &[]).await?;
    Ok(run(messages, seal, open, liveness))
}

/// The answering host refuses the link, saying why, and closes it.
pub(super) async fn refuse<S: AsyncRead + AsyncWrite + Unpin>(
    mut messages: Messages<S>,
    mut seal: Box<dyn Seal>,
    why: &str,
) {
    let _ = send(&mut messages, seal.as_mut(), REFUSED, why.as_bytes()).await;
    let _ = SinkExt::<Bytes>::close(&mut messages).await;
}

async fn send<S: AsyncRead + AsyncWrite + Unpin>(
    messages: &mut Messages<S>,
    seal: &mut dyn Seal,
    kind: u8,
    bytes: &[u8],
) -> Result<(), LinkRefused> {
    let mut plain = Vec::with_capacity(1 + bytes.len());
    plain.push(kind);
    plain.extend_from_slice(bytes);
    let sealed = seal.seal(&plain).map_err(LinkRefused::HandshakeFailed)?;
    messages.send(Bytes::from(sealed)).await?;
    Ok(())
}

/// Carry the link on a task of its own: the returned stream is the session protocol's, and
/// closes when the link ends.
pub(super) fn run<S: AsyncRead + AsyncWrite + Unpin + Send + 'static>(
    messages: Messages<S>,
    seal: Box<dyn Seal>,
    open: Box<dyn Open>,
    liveness: Liveness,
) -> (DuplexStream, Ends) {
    let (session, inside) = duplex(4 * MOST_DATA_BYTES);
    let (told, ended) = oneshot::channel();
    let task = tokio::spawn(async move {
        let why = carry(messages, seal, open, inside, liveness).await;
        let _ = told.send(why);
    });
    (
        session,
        Ends {
            ended,
            retire: task.abort_handle(),
        },
    )
}

/// Moves frames both ways until the link ends, and says why it did.
async fn carry<S: AsyncRead + AsyncWrite + Unpin + Send + 'static>(
    messages: Messages<S>,
    mut seal: Box<dyn Seal>,
    mut open: Box<dyn Open>,
    inside: DuplexStream,
    liveness: Liveness,
) -> String {
    let (mut sink, mut frames) = messages.split();
    let (mut from_the_session, mut to_the_session) = tokio::io::split(inside);
    // The link's own frames, pings and their answers, which go ahead of the session's bytes.
    let (own, mut own_waiting) = mpsc::channel::<u8>(4);
    let answers = own.clone();

    let reading = async {
        loop {
            let frame = match tokio::time::timeout(liveness.deadline, frames.next()).await {
                Err(_) => {
                    return format!(
                        "the other host sent nothing for {:?}, the link's deadline, so the link \
                         is taken as dead",
                        liveness.deadline
                    );
                }
                Ok(None) => return "the other host closed the link".to_owned(),
                Ok(Some(Err(why))) => return format!("the link's stream failed: {why}"),
                Ok(Some(Ok(frame))) => frame,
            };
            let plain = match open.open(&frame) {
                Ok(plain) => plain,
                Err(why) => return format!("a frame did not open ({why}), so the link ended"),
            };
            match plain.split_first() {
                Some((&DATA, bytes)) => {
                    if to_the_session.write_all(bytes).await.is_err() {
                        return "this host closed the link".to_owned();
                    }
                }
                Some((&PING, [])) => {
                    // An answer already waiting says the same.
                    let _ = answers.try_send(PONG);
                }
                Some((&PONG, [])) => {}
                _ => return "a frame of a kind this link does not know, so the link ended".into(),
            }
        }
    };

    let writing = async {
        let mut buffer = vec![0u8; MOST_DATA_BYTES];
        loop {
            let (kind, length) = tokio::select! {
                biased;
                Some(kind) = own_waiting.recv() => (kind, 0),
                read = from_the_session.read(&mut buffer) => match read {
                    Ok(0) | Err(_) => return "this host closed the link".to_owned(),
                    Ok(read) => (DATA, read),
                },
            };
            let mut plain = Vec::with_capacity(1 + length);
            plain.push(kind);
            plain.extend_from_slice(&buffer[..length]);
            let sealed = match seal.seal(&plain) {
                Ok(sealed) => sealed,
                Err(why) => return format!("a frame could not be sealed ({why})"),
            };
            if let Err(why) = sink.send(Bytes::from(sealed)).await {
                return format!("the link's stream failed: {why}");
            }
        }
    };

    let pinging = async {
        let start = tokio::time::Instant::now() + liveness.ping_every;
        let mut every = tokio::time::interval_at(start, liveness.ping_every);
        every.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            every.tick().await;
            if own.send(PING).await.is_err() {
                return "this host closed the link".to_owned();
            }
        }
    };

    tokio::select! {
        why = reading => why,
        why = writing => why,
        why = pinging => why,
    }
}
