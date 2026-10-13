//! Pairing's one frame (iOS design §4.2): inside the encrypted channel a device not yet pinned
//! proves it holds the one-time code the window showed, by an HMAC under that code over both
//! static keys and the host's device id. The host answers with one verdict, once the window has
//! confirmed. Nothing else may cross such a link.
//!
//! The frames use the handshake's framing ([`crate::version`]): the magic, a length, JSON.
//! `{"pair":{"device":"<ULID>","name":"Noro's iPhone","proof":"<hex>"}}`, then
//! `{"paired":{"device":"<host ULID>"}}` or `{"refused":{"why":"…"}}`.

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::version::{self, Refused};

/// What the proof is made over first, so a MAC the code made for anything else is never a
/// proof.
pub const LABEL: &[u8] = b"purlis session protocol 1: a device proves the pairing code";

/// The most bytes a device's name may be.
pub const MOST_NAME_BYTES: usize = 64;

/// A one-time pairing code: 256 bits from the operating system's generator, shown to the phone
/// inside the QR code, compared in constant time, never printed.
#[derive(Clone)]
pub struct Code([u8; 32]);

impl Code {
    pub fn mint() -> Code {
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes).expect("the OS generator works");
        Code(bytes)
    }

    pub fn to_hex(&self) -> String {
        self.0.iter().map(|b| format!("{b:02x}")).collect()
    }

    pub fn from_hex(text: &str) -> Result<Code, String> {
        if text.len() != 64 || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("a pairing code is 64 hex digits".into());
        }
        let mut bytes = [0u8; 32];
        for (i, pair) in text.as_bytes().chunks(2).enumerate() {
            bytes[i] = u8::from_str_radix(std::str::from_utf8(pair).unwrap_or("00"), 16)
                .map_err(|e| e.to_string())?;
        }
        Ok(Code(bytes))
    }
}

impl PartialEq for Code {
    fn eq(&self, other: &Self) -> bool {
        use subtle::ConstantTimeEq;
        bool::from(self.0.ct_eq(&other.0))
    }
}

impl Eq for Code {}

impl std::fmt::Debug for Code {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Code(…)")
    }
}

/// The one frame a device sends on an unpinned link inside a pairing window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Proof {
    /// The device's id, a ULID it minted for itself.
    pub device: String,
    /// What the person calls it, as the window shows it ("Pair 'Noro's iPhone'?").
    pub name: String,
    /// [`prove`]'s hex.
    pub proof: String,
}

impl Proof {
    /// Whether the frame is well formed: a ULID for the device, a name within the bound. Checked
    /// before the proof is, so a refusal of shape says nothing about the code.
    pub fn check(&self) -> Result<(), String> {
        if self.device.len() != 26 || !self.device.bytes().all(|b| b.is_ascii_alphanumeric()) {
            return Err("a device id is a ULID".into());
        }
        if self.name.is_empty() || self.name.len() > MOST_NAME_BYTES {
            return Err(format!("a device's name is 1 to {MOST_NAME_BYTES} bytes"));
        }
        Ok(())
    }
}

/// The host's one answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// Both sides pin: here is the host's device id for the phone's record.
    Paired {
        device: String,
    },
    Refused {
        why: String,
    },
}

/// The proof: HMAC-SHA256 keyed by the code over [`LABEL`], the device's public key, the
/// host's public key and the host's device id, each ended by a zero byte.
pub fn prove(
    code: &Code,
    device_public: &[u8; 32],
    host_public: &[u8; 32],
    host_device: &str,
) -> String {
    use hmac::{KeyInit, Mac};
    let mut mac = hmac::Hmac::<sha2::Sha256>::new_from_slice(&code.0)
        .unwrap_or_else(|_| unreachable!("HMAC takes a key of any length"));
    for part in [
        LABEL,
        device_public.as_slice(),
        host_public.as_slice(),
        host_device.as_bytes(),
    ] {
        mac.update(part);
        mac.update(&[0]);
    }
    mac.finalize()
        .into_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Whether `proof` is [`prove`]'s answer for these keys and this code, compared in constant
/// time.
pub fn verify(
    code: &Code,
    proof: &Proof,
    device_public: &[u8; 32],
    host_public: &[u8; 32],
    host_device: &str,
) -> bool {
    use subtle::ConstantTimeEq;
    let expected = prove(code, device_public, host_public, host_device);
    bool::from(expected.as_bytes().ct_eq(proof.proof.as_bytes()))
}

#[derive(Serialize, Deserialize)]
struct Pair {
    pair: Proof,
}

pub async fn send_proof<W: AsyncWrite + Unpin>(io: &mut W, proof: &Proof) -> Result<(), Refused> {
    version::send(
        io,
        &Pair {
            pair: proof.clone(),
        },
    )
    .await
}

/// The one frame, or a refusal when it is not a proof, and whatever bytes were read past it:
/// nothing may follow a proof, so a caller treats anything there as a second frame.
pub async fn read_proof<R: AsyncRead + Unpin>(
    io: &mut R,
) -> Result<(Proof, bytes::BytesMut), Refused> {
    let (frame, left): (Pair, _) = version::receive(io).await?;
    Ok((frame.pair, left))
}

pub async fn send_verdict<W: AsyncWrite + Unpin>(
    io: &mut W,
    verdict: &Verdict,
) -> Result<(), Refused> {
    version::send(io, verdict).await
}

pub async fn read_verdict<R: AsyncRead + Unpin>(io: &mut R) -> Result<Verdict, Refused> {
    let (verdict, _left): (Verdict, _) = version::receive(io).await?;
    Ok(verdict)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_frames_are_snake_case_json() {
        let v = serde_json::to_value(Verdict::Paired { device: "d".into() }).unwrap();
        assert_eq!(v, serde_json::json!({"paired": {"device": "d"}}));
        let p = serde_json::to_value(Pair {
            pair: Proof {
                device: "d".into(),
                name: "n".into(),
                proof: "p".into(),
            },
        })
        .unwrap();
        assert_eq!(
            p,
            serde_json::json!({"pair": {"device": "d", "name": "n", "proof": "p"}})
        );
    }
}
