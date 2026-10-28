// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The masked shape of a secret — `charter/secrets/fingerprint.py`: a keyed fingerprint and a
//! size band.
//!
//! `secret get` without `--reveal` answers "is it present", "is it the same value the other
//! vault holds" and "did the whole token land" — and must NOT answer "is the value
//! `Summer2024!`". An unkeyed digest plus an exact length is an offline oracle for a guessed
//! value (#436), so:
//!
//! - **the digest is keyed**: `HMAC-SHA256(plane key, value)`, the key 32 random bytes kept
//!   0600 at `<state>/fingerprint.key`, generated on first use. Comparable within one plane,
//!   uncomputable without the key;
//! - **the size is a band**, not a count.
//!
//! A plane whose key cannot be read or made prints NO fingerprint — never an unkeyed one.

use std::path::{Path, PathBuf};

use hmac::{Hmac, KeyInit as _, Mac};
use sha2::Sha256;

use super::Ctx;

/// 256 bits. A key file of any other length is regenerated, never used short.
pub const KEY_BYTES: usize = 32;

/// The key's file name under the state directory.
pub const KEY_FILE: &str = "fingerprint.key";

/// Where the plane's fingerprint key lives.
pub fn key_path(ctx: &Ctx) -> PathBuf {
    ctx.state.join(KEY_FILE)
}

/// `_key`: the plane's key, generating it on first use; `None` when it can be neither read
/// nor made.
///
/// Read and written from `trust` ([`Ctx::trust`]), never through a link (#440): a key that is
/// a link, or a `.charter/` that is one, gives no key at all — a linked file of the right
/// length would otherwise BE the key — and no new key is written there either.
fn key(trust: &Path, p: &Path) -> Option<Vec<u8>> {
    super::key_file(trust, p, KEY_BYTES).ok()
}

/// **A seal of `parts` for `purpose`**: the 64 hex characters of `HMAC-SHA256` of each part, each
/// ended by a NUL, under a key derived for `purpose` alone ([`purpose_key`]), or `None` with no
/// plane key. What purlis writes into a file it alone writes, so that it can tell its own file
/// from one written otherwise: computing one needs the plane's key, which no sandboxed chat
/// reads, and a seal for one purpose is never one for another, nor anything the plane's key
/// signs itself.
pub fn seal(ctx: &Ctx, purpose: &str, parts: &[&str]) -> Option<String> {
    let k = purpose_key(&key(ctx.trust(), &key_path(ctx))?, purpose)?;
    let mut mac = Hmac::<Sha256>::new_from_slice(&k).ok()?;
    for part in parts {
        mac.update(part.as_bytes());
        mac.update(b"\0");
    }
    Some(hex(&mac.finalize().into_bytes()))
}

/// The key a seal for `purpose` is made under: `HMAC-SHA256(plane key, "purlis seal key\0" +
/// purpose)`, so each purpose has a key of its own, and none is the plane's key itself.
fn purpose_key(plane_key: &[u8], purpose: &str) -> Option<Vec<u8>> {
    let mut mac = Hmac::<Sha256>::new_from_slice(plane_key).ok()?;
    mac.update(b"purlis seal key\0");
    mac.update(purpose.as_bytes());
    Some(mac.finalize().into_bytes().to_vec())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// `fingerprint`: 12 hex characters of `HMAC-SHA256(key, value)`, or `None` with no key.
pub fn fingerprint(ctx: &Ctx, value: &str) -> Option<String> {
    let k = key(ctx.trust(), &key_path(ctx))?;
    let mut mac = Hmac::<Sha256>::new_from_slice(&k).ok()?;
    mac.update(value.as_bytes());
    let digest = mac.finalize().into_bytes();
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    Some(hex[..12].to_string())
}

/// `size_band`: `empty`, `1–15 bytes`, `16–31 bytes`, … `1024+ bytes`, counted in UTF-8 bytes.
pub fn size_band(value: &str) -> String {
    let n = value.len();
    if n == 0 {
        return "empty".into();
    }
    if n < 16 {
        return "1–15 bytes".into();
    }
    if n >= 1024 {
        return "1024+ bytes".into();
    }
    let lo = 1usize << (usize::BITS - 1 - n.leading_zeros());
    format!("{lo}–{} bytes", (lo << 1) - 1)
}

/// `masked`: the size band, and the keyed fingerprint when this plane has a key.
pub fn masked(ctx: &Ctx, value: &str) -> String {
    match fingerprint(ctx, value) {
        Some(fp) => format!("{} · fp:{fp}", size_band(value)),
        None => size_band(value),
    }
}

#[cfg(test)]
mod tests;
