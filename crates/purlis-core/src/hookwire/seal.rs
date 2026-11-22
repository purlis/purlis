// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A spooled tool line carries its argument digest sealed** (#6880, Q290).
//!
//! A hook sends the host the arguments' *unkeyed* SHA-256, which anyone who reads it can match
//! against a guess (#977), so a spooled line cannot hold it in the clear. The host keys it with
//! the device's [`crate::eventlog::ArgsKey`] before it writes it, and an HMAC cannot be keyed
//! again, so the plain hash has to come back at the drain: sealed until then, so the chat
//! itself cannot read it back.
//!
//! - The app keeps one **private** key per chat of a plane in the machine store, beside
//!   `args.key` ([`public_for`]), and puts only the **public** key in the chat's environment
//!   ([`SEAL_ENV`]). The key is named for the plane as well as the chat number, so one key never
//!   covers the same number in two projects. A chat's sandbox neither reads nor writes the key
//!   files (`sandbox::event_keys_held`); the key goes when its chat has ended and its spool is
//!   drained ([`forget`], [`forget_ended`]), and the folder is left out of backups.
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

/// The folder of the seal keys, in the device's events folder.
pub const DIR: &str = crate::eventlog::SEAL_DIR;

/// What names `plane` in a key's file name: the first 16 bytes of the SHA-256 of its path, in hex.
fn plane_id(plane: &Path) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(plane.to_string_lossy().as_bytes());
    digest[..16]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn key_path(dir: &Path, plane: &Path, chat: u32) -> PathBuf {
    dir.join(DIR)
        .join(format!("{}-{chat}.key", plane_id(plane)))
}

/// The public key of chat `chat`'s seal in the project at `plane`, whose private key is kept in
/// `dir` (the device's events folder), made on first use. The same chat has the same key on
/// every start, so a spool from before a restart still opens.
pub fn public_for(dir: &Path, plane: &Path, chat: u32) -> io::Result<String> {
    // The folder is made, and left out of this device's backups, before a key is put in it.
    let folder = dir.join(DIR);
    crate::contain::no_link_on_the_way(dir, &folder)?;
    crate::secrets::make_private_dir(&folder)?;
    crate::browserlogins::exclude_from_backups(&folder).map_err(io::Error::other)?;
    let bytes = zeroize::Zeroizing::new(crate::secrets::key_file(
        dir,
        &key_path(dir, plane, chat),
        crypto_box::KEY_SIZE,
    )?);
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

/// A sealed box is the sender's ephemeral public key (this many bytes) and then the encrypted
/// text, so a blob of no more than this holds none.
const EPHEMERAL: usize = crypto_box::KEY_SIZE;

/// The hash a sealed `args` of chat `chat` in the project at `plane` holds, opened with the key
/// kept in `dir`. `None` for a key that is missing, unreadable or not the one it was sealed to,
/// for a blob too short to be a sealed box, and for anything not sealed: the key is never made
/// here.
pub fn open(dir: &Path, plane: &Path, chat: u32, args: &str) -> Option<String> {
    let sealed = base64::engine::general_purpose::STANDARD
        .decode(args.strip_prefix(SEALED)?)
        .ok()?;
    if sealed.len() <= EPHEMERAL {
        return None;
    }
    let key = zeroize::Zeroizing::new(
        crate::contain::read_no_link(dir, &key_path(dir, plane, chat)).ok()?,
    );
    let plain = SecretKey::from_slice(&key).ok()?.unseal(&sealed).ok()?;
    String::from_utf8(plain).ok()
}

/// Deletes chat `chat`'s seal key in the project at `plane`, once the chat has ended and its
/// spool is drained: a key that is not there is done.
pub fn forget(dir: &Path, plane: &Path, chat: u32) -> io::Result<()> {
    match std::fs::remove_file(key_path(dir, plane, chat)) {
        Err(why) if why.kind() != io::ErrorKind::NotFound => Err(why),
        _ => Ok(()),
    }
}

/// Deletes the seal key of every chat of the project at `plane` that `ended` says has ended.
/// The caller says so only for a chat that is not coming back and whose spool holds no line.
pub fn forget_ended(dir: &Path, plane: &Path, ended: impl Fn(u32) -> bool) -> io::Result<()> {
    let prefix = format!("{}-", plane_id(plane));
    let Ok(names) = std::fs::read_dir(dir.join(DIR)) else {
        return Ok(());
    };
    for name in names.filter_map(Result::ok) {
        let name = name.file_name();
        let chat = name
            .to_str()
            .and_then(|name| name.strip_prefix(&prefix))
            .and_then(|rest| rest.strip_suffix(".key"))
            .and_then(|number| number.parse::<u32>().ok());
        if let Some(chat) = chat
            && ended(chat)
        {
            forget(dir, plane, chat)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
