//! The desktops this phone paired with: each by its device id, with the key it pinned and
//! where to reach it. One record, `hosts`, a JSON list.

use crate::store::Store;
use crate::{Error, hex, unhex32};

pub const HOSTS: &str = "hosts";

/// One paired desktop.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Host {
    /// The desktop's device id (ADR 0066): what the records are keyed by, never an address.
    pub device: String,
    /// What the person calls it; empty until they do.
    pub name: String,
    /// The desktop's public link key, pinned at pairing.
    #[serde(with = "key_hex")]
    pub key: [u8; 32],
    /// Where to reach it, `host:port` each, as the pairing code said and as edited since.
    pub addresses: Vec<String>,
    /// When it paired, seconds since the epoch.
    pub paired: u64,
}

mod key_hex {
    pub fn serialize<S: serde::Serializer>(key: &[u8; 32], to: S) -> Result<S::Ok, S::Error> {
        to.serialize_str(&super::hex(key))
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(from: D) -> Result<[u8; 32], D::Error> {
        let text: String = serde::Deserialize::deserialize(from)?;
        super::unhex32(&text).map_err(serde::de::Error::custom)
    }
}

pub fn read(store: &dyn Store) -> Result<Vec<Host>, Error> {
    match store.get(HOSTS)? {
        Some(json) => serde_json::from_str(&json)
            .map_err(|err| Error::Store(format!("the desktops record is unreadable: {err}"))),
        None => Ok(Vec::new()),
    }
}

fn write(store: &dyn Store, hosts: &[Host]) -> Result<(), Error> {
    let json = serde_json::to_string(hosts).map_err(|err| Error::Store(err.to_string()))?;
    store.set(HOSTS, &json)
}

/// Pin `host`, replacing a record of the same device (a desktop paired again).
pub fn add(store: &dyn Store, host: Host) -> Result<(), Error> {
    let mut hosts = read(store)?;
    hosts.retain(|one| one.device != host.device);
    hosts.push(host);
    write(store, &hosts)
}

/// Unpair: the record goes, and what it was.
pub fn remove(store: &dyn Store, device: &str) -> Result<Option<Host>, Error> {
    let mut hosts = read(store)?;
    let at = hosts.iter().position(|one| one.device == device);
    let removed = at.map(|at| hosts.remove(at));
    if removed.is_some() {
        write(store, &hosts)?;
    }
    Ok(removed)
}

pub fn find(store: &dyn Store, device: &str) -> Result<Option<Host>, Error> {
    Ok(read(store)?.into_iter().find(|one| one.device == device))
}

fn change(store: &dyn Store, device: &str, change: impl FnOnce(&mut Host)) -> Result<(), Error> {
    let mut hosts = read(store)?;
    let Some(host) = hosts.iter_mut().find(|one| one.device == device) else {
        return Err(Error::Store(format!("no desktop {device} is paired")));
    };
    change(host);
    write(store, &hosts)
}

pub fn rename(store: &dyn Store, device: &str, name: &str) -> Result<(), Error> {
    change(store, device, |host| host.name = name.to_owned())
}

pub fn set_addresses(store: &dyn Store, device: &str, addresses: Vec<String>) -> Result<(), Error> {
    change(store, device, |host| host.addresses = addresses)
}
