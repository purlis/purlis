//! `<config>/peers.json`: the devices paired with this one (ADR 0078 §8, iOS design §4.1).
//! Machine, device-bound: a pinned key says which two devices trust each other, and a second
//! machine of the same person pairs on its own. Written whole by a rename, `0600`, under the
//! machine store's lock, like `machine.json` beside it. Denied to chats.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::machine;

pub const FILE: &str = "peers.json";
pub const VERSION: u32 = 1;

/// One paired device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Peer {
    /// Its device id, a ULID it minted.
    pub device: String,
    /// What the person calls it.
    pub name: String,
    /// Its pinned public link key.
    pub key: [u8; 32],
    /// When it was paired, seconds since the epoch.
    pub paired: u64,
}

impl Peer {
    pub fn fingerprint(&self) -> String {
        super::fingerprint(&self.key)
    }
}

#[derive(Serialize, Deserialize)]
struct OnDisk {
    version: u32,
    peers: Vec<PeerOnDisk>,
}

#[derive(Serialize, Deserialize)]
struct PeerOnDisk {
    device: String,
    name: String,
    key: String,
    paired: u64,
}

pub fn path(config_root: &Path) -> PathBuf {
    machine::dir(config_root).join(FILE)
}

/// Every peer. No file is no peers; a file that cannot be read or parsed is an error, never an
/// empty list, so a torn file cannot look like "nobody is paired" to a writer that would then
/// replace it.
pub fn read(config_root: &Path) -> Result<Vec<Peer>, String> {
    let text = match std::fs::read_to_string(path(config_root)) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(format!("{FILE} could not be read: {e}")),
    };
    let disk: OnDisk =
        serde_json::from_str(&text).map_err(|e| format!("{FILE} does not parse: {e}"))?;
    disk.peers
        .into_iter()
        .map(|p| {
            Ok(Peer {
                key: super::unhex32(&p.key).map_err(|why| format!("{FILE}: a key is not {why}"))?,
                device: p.device,
                name: p.name,
                paired: p.paired,
            })
        })
        .collect()
}

fn write(config_root: &Path, peers: &[Peer]) -> Result<(), String> {
    let disk = OnDisk {
        version: VERSION,
        peers: peers
            .iter()
            .map(|p| PeerOnDisk {
                device: p.device.clone(),
                name: p.name.clone(),
                key: super::hex(&p.key),
                paired: p.paired,
            })
            .collect(),
    };
    let text = serde_json::to_string_pretty(&disk).expect("plain data");
    machine::write_beside(config_root, &path(config_root), (text + "\n").as_bytes())
        .map_err(|e| format!("{FILE} could not be written: {e}"))
}

/// Pin `peer`. A record under the same device id is replaced (the same phone paired again,
/// perhaps with a rotated key); the same key under another device id is refused, since one key
/// is one device.
pub fn add(config_root: &Path, peer: Peer) -> Result<(), String> {
    let _held = machine::Lock::on(config_root);
    let mut peers = read(config_root)?;
    if peers
        .iter()
        .any(|p| p.key == peer.key && p.device != peer.device)
    {
        return Err(format!(
            "the key {} is already pinned under another device",
            peer.fingerprint()
        ));
    }
    peers.retain(|p| p.device != peer.device);
    peers.push(peer);
    write(config_root, &peers)
}

/// Unpin `device`'s key. The record removed, or `None` when there was none.
pub fn remove(config_root: &Path, device: &str) -> Result<Option<Peer>, String> {
    let _held = machine::Lock::on(config_root);
    let mut peers = read(config_root)?;
    let at = peers.iter().position(|p| p.device == device);
    let removed = at.map(|i| peers.remove(i));
    if removed.is_some() {
        write(config_root, &peers)?;
    }
    Ok(removed)
}

/// The scope word a pinned key is admitted as, or `None`. Every peer of a desktop is a phone,
/// `remote-device`; a runner's peers are its desktops, `remote-link`, and live in the runner's
/// own file (RR-1). A file that cannot be read pins nothing.
pub fn scope_of(config_root: &Path, key: &[u8; 32]) -> Option<&'static str> {
    read(config_root)
        .ok()?
        .iter()
        .any(|p| &p.key == key)
        .then_some("remote-device")
}
