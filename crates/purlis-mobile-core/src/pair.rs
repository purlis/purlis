//! Pairing from the phone's side (iOS design §4.2): Noise XX against the scanned key, the one
//! proof frame, and the desktop's verdict, which the person gave in their window.

use std::time::Duration;

use purlis_session_protocol::noise::{self, Identity};
use purlis_session_protocol::pairing::{self, Proof, Verdict};
use purlis_session_protocol::version::HANDSHAKE_TIMEOUT;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::Error;
use crate::hosts::Host;
use crate::identity::Device;
use crate::payload::Payload;

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or(0)
}

/// A handshake that failed, in the crate's words: the pinned key not the one offered, or the
/// link.
pub(crate) fn handshake_failed(err: noise::Error) -> Error {
    match err {
        noise::Error::WrongHost { .. } => Error::WrongKey,
        other => Error::Link(other.to_string()),
    }
}

/// Pair over `io`, a connection to one of the payload's addresses: the handshake against the
/// scanned key, the proof, the verdict. `Ok` is the desktop to pin, named as the person will
/// name it (empty until they do).
pub async fn pair<S>(
    io: S,
    me: &Identity,
    device: &Device,
    payload: &Payload,
) -> Result<Host, Error>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let host_key = payload.host_key()?;
    let code = payload.code()?;
    // The handshake within the protocol's own deadline; the verdict waits on the person.
    let secured = tokio::time::timeout(
        HANDSHAKE_TIMEOUT,
        noise::connect(io, me, &payload.host_device, Some(host_key)),
    )
    .await
    .map_err(|_| Error::TimedOut)?
    .map_err(handshake_failed)?;
    let mut plain = noise::pump(secured);
    let proof = Proof {
        device: device.id.clone(),
        name: device.name.clone(),
        proof: pairing::prove(&code, &me.public(), &host_key, &payload.host_device),
    };
    pairing::send_proof(&mut plain, &proof)
        .await
        .map_err(|err| Error::Link(err.to_string()))?;
    match pairing::read_verdict(&mut plain)
        .await
        .map_err(|err| Error::Link(err.to_string()))?
    {
        // The verdict names the desktop: the one the code named, or the pairing is not the
        // one the person started.
        Verdict::Paired { device } if device == payload.host_device => Ok(Host {
            device,
            name: String::new(),
            key: host_key,
            addresses: payload.addresses.clone(),
            paired: now_secs(),
        }),
        Verdict::Paired { .. } => Err(Error::PairingRefused(
            "the desktop answered as another device".into(),
        )),
        Verdict::Refused { why } => Err(Error::PairingRefused(why)),
    }
}

/// [`pair`] over TCP: each of the payload's addresses in order, the first that connects
/// within `timeout`.
pub async fn pair_tcp(
    me: &Identity,
    device: &Device,
    payload: &Payload,
    timeout: Duration,
) -> Result<Host, Error> {
    let mut tried = Vec::new();
    for address in &payload.addresses {
        match tokio::time::timeout(timeout, tokio::net::TcpStream::connect(address)).await {
            Ok(Ok(tcp)) => {
                let _ = tcp.set_nodelay(true);
                return pair(tcp, me, device, payload).await;
            }
            Ok(Err(err)) => tried.push(format!("{address}: {err}")),
            Err(_) => tried.push(format!("{address}: no answer in time")),
        }
    }
    Err(Error::Unreachable(if tried.is_empty() {
        "the pairing code names no address".into()
    } else {
        tried.join("; ")
    }))
}
