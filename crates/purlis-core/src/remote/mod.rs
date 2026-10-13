//! A paired device's side of this desktop (iOS design §3, §4): what the desktop keeps so a
//! phone can link to it. The link itself, the handshake and the gateway are
//! `purlis-session-protocol`'s (`noise`, `pairing`, `gateway`, feature `noise`); the app wires
//! the two together. Everything here is plain, synchronous state, and moves to `purlisd` with
//! the host.
//!
//! - [`linkkey`]: this device's static X25519 key, in the Keyring.
//! - [`peers`]: `<config>/peers.json`, the devices paired with this one.
//! - [`pairing`]: one-time codes, the QR payload, the pending confirmation the window answers,
//!   and unpairing.
//! - [`audit`]: the `remote.*` events, metadata only.

pub mod audit;
pub mod linkkey;
pub mod pairing;
pub mod peers;

/// A key's fingerprint, as the window and the audit show it: the first 16 hex digits of its
/// SHA-256. The same as `purlis_session_protocol::noise::fingerprint`; written out here because
/// this crate does not depend on that one, and a test in the app holds the two equal.
pub fn fingerprint(public: &[u8; 32]) -> String {
    use sha2::{Digest, Sha256};
    hex(&Sha256::digest(public))[..16].to_owned()
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub(crate) fn unhex32(text: &str) -> Result<[u8; 32], String> {
    if text.len() != 64 || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("64 hex digits".into());
    }
    let mut out = [0u8; 32];
    for (i, pair) in text.as_bytes().chunks(2).enumerate() {
        out[i] = u8::from_str_radix(std::str::from_utf8(pair).unwrap_or("00"), 16)
            .map_err(|e| e.to_string())?;
    }
    Ok(out)
}
