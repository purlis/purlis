// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A runner's side of its link** (ADR 0078 §3 and §8, spec #2041, #2044): the runner's own
//! link key and the one desktop it is paired with.
//!
//! - **The runner makes its key at its first pairing** ([`pair`]), from the operating system's
//!   generator, and keeps its private half in [`KEY_FILE`], `0600`, in the session host's own
//!   folder ([`crate::machine::charterd`]), which every chat's sandbox denies reading and
//!   writing. Each runner has a key of its own: a new container is a new runner, with a new
//!   key.
//! - **A runner holds one peer** (ADR 0078, ruled 2026-10-15): the desktop that paired it, in
//!   `<config>/peers.json` ([`PEERS_FILE`]), which chats may not write. Pairing again with that
//!   desktop answers the same key and changes nothing; pairing with any other is refused, and
//!   the first stays.
//! - **Its host answers a link only once it is paired** ([`answering`]), reading both files as
//!   each link arrives, so a pairing made while the host runs is in force at the next link. A
//!   file another user could read or write, a link in its place, or one that is not what this
//!   module writes refuses every link.

use std::path::Path;

use purlis_session_protocol::hostlink::{Identity, LinkKey, Pinned, PublicKey, Zeroizing};

/// The runner's link key's private half, as 64 lower-case hex.
pub const KEY_FILE: &str = "link-key";

/// The devices paired with this runner, in the machine store: `{"version": 1, "peers":
/// [{"device", "key", "paired"}]}`, the key as 64 lower-case hex and `paired` in seconds since
/// the epoch. It holds one peer; a file of another shape, or with more than one, admits none.
pub const PEERS_FILE: &str = "peers.json";

/// Pair this runner, whose config home is `config_root` and whose device id is `device`, with
/// `desktop`; answers the runner's own pin, for the desktop to keep.
pub fn pair(config_root: &Path, device: &str, desktop: &Pinned) -> Result<Pinned, String> {
    let folder = &crate::machine::charterd(config_root);
    let held = purlis_same_user::PrivateDirectory::open(folder)
        .map_err(|why| format!("the session host's folder could not be opened: {why}"))?;
    let store = purlis_same_user::PrivateDirectory::open(&crate::machine::dir(config_root))
        .map_err(|why| format!("the machine store could not be opened: {why}"))?;
    if let Some(paired) = peer(config_root)?
        && &paired != desktop
    {
        return Err(format!(
            "this runner is already paired with desktop {} (key {}), and holds one \
                 desktop; remove the runner and make a new one to pair it with another",
            paired.device,
            paired.key.fingerprint()
        ));
    }
    let key = match key(folder)? {
        Some(key) => key,
        None => {
            let made = LinkKey::generate().map_err(|why| why.to_string())?;
            let hex = Zeroizing::new(hex_of(made_private(&made)?.as_ref()));
            held.replace_file(KEY_FILE, hex.as_bytes())
                .map_err(|why| format!("the runner's link key could not be kept: {why}"))?;
            made
        }
    };
    if peer(config_root)?.is_none() {
        let paired = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|since| since.as_secs())
            .unwrap_or_default();
        let peers = serde_json::json!({
            "version": 1,
            "peers": [{
                "device": desktop.device,
                "key": hex_of(desktop.key.as_bytes()),
                "paired": paired,
            }],
        });
        store
            .replace_file(PEERS_FILE, format!("{peers:#}\n").as_bytes())
            .map_err(|why| format!("the desktop's pin could not be kept: {why}"))?;
    }
    Ok(Pinned {
        device: device.to_owned(),
        key: key.public(),
    })
}

/// Who this runner, whose config home is `config_root`, is on a link and the desktop it
/// admits, once it is paired; `None` before.
pub fn answering(config_root: &Path, device: &str) -> Result<Option<(Identity, Pinned)>, String> {
    let Some(desktop) = peer(config_root)? else {
        return Ok(None);
    };
    let key = key(&crate::machine::charterd(config_root))?.ok_or_else(|| {
        "this runner is paired but keeps no link key; remove the runner and add it again".to_owned()
    })?;
    Ok(Some((
        Identity {
            device: device.to_owned(),
            key,
        },
        desktop,
    )))
}

/// A pin as one line: `{"device":"<id>","key":"<64 hex>"}`.
pub fn to_line(pinned: &Pinned) -> String {
    serde_json::json!({ "device": pinned.device, "key": hex_of(pinned.key.as_bytes()) }).to_string()
}

/// A pin from one line of [`to_line`]: exactly those two keys, a device id of letters and
/// digits, and a key of 64 lower-case hex.
pub fn from_line(line: &str) -> Result<Pinned, String> {
    let bad = || format!("\"{}\" is not a device's pin", line.trim());
    let value: serde_json::Value = serde_json::from_str(line.trim()).map_err(|_| bad())?;
    let fields = value.as_object().ok_or_else(bad)?;
    if fields.len() != 2 {
        return Err(bad());
    }
    let device = fields
        .get("device")
        .and_then(serde_json::Value::as_str)
        .filter(|device| {
            !device.is_empty()
                && device.len() <= 64
                && device.bytes().all(|b| b.is_ascii_alphanumeric())
        })
        .ok_or_else(bad)?;
    let key = fields
        .get("key")
        .and_then(serde_json::Value::as_str)
        .and_then(bytes_of)
        .ok_or_else(bad)?;
    Ok(Pinned {
        device: device.to_owned(),
        key: PublicKey::from_bytes(key),
    })
}

fn peer(config_root: &Path) -> Result<Option<Pinned>, String> {
    let Some(text) = kept(&crate::machine::dir(config_root), PEERS_FILE)? else {
        return Ok(None);
    };
    let unreadable = || {
        format!(
            "this runner's {PEERS_FILE} is not as purlis writes it, so it links nobody; \
             remove the runner and add it again"
        )
    };
    let value: serde_json::Value = serde_json::from_str(&text).map_err(|_| unreadable())?;
    if value.get("version").and_then(serde_json::Value::as_u64) != Some(1) {
        return Err(unreadable());
    }
    let one = match value
        .get("peers")
        .and_then(serde_json::Value::as_array)
        .map(Vec::as_slice)
    {
        Some([one]) => one,
        _ => return Err(unreadable()),
    };
    let device = one.get("device").cloned().unwrap_or_default();
    let key = one.get("key").cloned().unwrap_or_default();
    if one
        .get("paired")
        .and_then(serde_json::Value::as_u64)
        .is_none()
    {
        return Err(unreadable());
    }
    from_line(&serde_json::json!({ "device": device, "key": key }).to_string())
        .map(Some)
        .map_err(|_| unreadable())
}

fn key(folder: &Path) -> Result<Option<LinkKey>, String> {
    let Some(hex) = kept(folder, KEY_FILE)? else {
        return Ok(None);
    };
    let hex = Zeroizing::new(hex);
    let private = bytes_of(hex.trim())
        .map(Zeroizing::new)
        .ok_or_else(|| "the runner's link key is not one key".to_owned())?;
    LinkKey::from_private(private)
        .map(Some)
        .map_err(|why| format!("the runner's link key is not one key: {why}"))
}

/// The file `name` in `folder`, read only when it is this user's alone; `None` when absent.
fn kept(folder: &Path, name: &str) -> Result<Option<String>, String> {
    match purlis_same_user::read_private_file(&folder.join(name)) {
        Ok(text) => Ok(Some(text)),
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(why) => Err(format!(
            "the runner's {name} could not be read safely: {why}"
        )),
    }
}

fn made_private(key: &LinkKey) -> Result<Zeroizing<[u8; 32]>, String> {
    key.private_half()
        .ok_or_else(|| "a link key with no private half cannot be kept".to_owned())
}

fn hex_of(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn bytes_of(hex: &str) -> Option<[u8; 32]> {
    if hex.len() != 64 || !hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
        return None;
    }
    let mut bytes = [0u8; 32];
    for (at, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[at * 2..at * 2 + 2], 16).ok()?;
    }
    Some(bytes)
}

#[cfg(test)]
mod tests;
