//! **A spooled tool line carries its argument digest sealed** (#6880, Q290).
//!
//! A hook sends the host the arguments' *unkeyed* SHA-256, which anyone who reads it can match
//! against a guess (#977), so a spooled line cannot hold it in the clear. The host keys it with
//! the device's [`crate::eventlog::ArgsKey`] before it writes it, and an HMAC cannot be keyed
//! again, so the plain hash has to come back at the drain: sealed until then, so the chat
//! itself cannot read it back.
//!
//! - The app keeps one **private** key per chat number in the machine store, beside `args.key`
//!   ([`public_for`]), and puts only the **public** key in the chat's environment ([`SEAL_ENV`]).
//! - The hook seals the hash in a libsodium sealed box ([`seal`]): a fresh ephemeral key per
//!   line, so equal hashes never give equal ciphertext, and only the private key opens it.
//! - The drain opens it ([`open`]) and the host keys the hash. A key that is missing or
//!   unreadable opens nothing: the digest is left out and the event says so, and it is never
//!   written in the clear.

use std::io;
use std::path::{Path, PathBuf};

use base64::Engine as _;
use crypto_box::{PublicKey, SecretKey};

/// The chat's public key, in its environment: base64 of 32 bytes. Not inherited
/// ([`super::NOT_INHERITED`]), as the chat's token is not.
pub const SEAL_ENV: &str = "PURLIS_CHAT_SEAL";

/// What a sealed digest begins with in a spooled line's `args`. A hex digest has no `:`.
const SEALED: &str = "sealed:";

fn encode(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn key_path(dir: &Path, chat: u32) -> PathBuf {
    dir.join("seal").join(format!("{chat}.key"))
}

/// The public key of chat `chat`'s seal, whose private key is kept in `dir` (the device's
/// events folder), made on first use. The same chat number has the same key on every start, so
/// a spool from before a restart still opens.
pub fn public_for(dir: &Path, chat: u32) -> io::Result<String> {
    let bytes = crate::secrets::key_file(dir, &key_path(dir, chat), crypto_box::KEY_SIZE)?;
    let secret = SecretKey::from_slice(&bytes).map_err(io::Error::other)?;
    Ok(encode(secret.public_key().as_bytes()))
}

/// `hash` sealed to `public` (base64, [`SEAL_ENV`]), as a spooled line's `args`. `None` for a
/// public key that is not one: the line is then spooled without a digest.
pub fn seal(public: &str, hash: &str) -> Option<String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(public)
        .ok()?;
    let public = PublicKey::from_slice(&bytes).ok()?;
    let sealed = public
        .seal(&mut crypto_box::aead::OsRng, hash.as_bytes())
        .ok()?;
    Some(format!("{SEALED}{}", encode(&sealed)))
}

/// Whether a spooled line's `args` is a sealed digest, not the plain one an older build spooled.
pub fn is_sealed(args: &str) -> bool {
    args.starts_with(SEALED)
}

/// The hash a sealed `args` of chat `chat` holds, opened with the key kept in `dir`. `None` for
/// a key that is missing, unreadable or not the one it was sealed to, and for anything not
/// sealed: the key is never made here.
pub fn open(dir: &Path, chat: u32, args: &str) -> Option<String> {
    let sealed = base64::engine::general_purpose::STANDARD
        .decode(args.strip_prefix(SEALED)?)
        .ok()?;
    let key = crate::contain::read_no_link(dir, &key_path(dir, chat)).ok()?;
    let plain = SecretKey::from_slice(&key).ok()?.unseal(&sealed).ok()?;
    String::from_utf8(plain).ok()
}

#[cfg(test)]
mod tests;
