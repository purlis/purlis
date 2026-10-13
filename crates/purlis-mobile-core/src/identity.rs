//! Who this phone is: its Noise link key (minted once, in the Keychain on a phone) and the
//! device id and name the desktop shows ("Pair 'Noro's iPhone'?").

use purlis_session_protocol::noise::Identity;

use crate::store::Store;
use crate::{Error, unhex32};

/// The record of the link key: 64 hex digits of the private key.
pub const LINK_KEY: &str = "link-key";
/// The record of the device's id and name.
pub const DEVICE: &str = "device";

/// What the name is until the person gives one.
pub const DEFAULT_NAME: &str = "purlis on this phone";

/// The device as the desktop lists it: a ULID it minted for itself (ADR 0066), and a name.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Device {
    pub id: String,
    pub name: String,
}

/// The link key, minted and stored on the first ask. A stored record that is not a key is
/// refused, never written over: somebody put it there.
pub fn link_key(store: &dyn Store) -> Result<Identity, Error> {
    match store.get(LINK_KEY)? {
        Some(stored) => {
            let private = unhex32(&stored)
                .map_err(|_| Error::Store("the stored link key is not a key".into()))?;
            Identity::from_private(private).map_err(|err| Error::Store(err.to_string()))
        }
        None => {
            let minted = Identity::generate();
            store.set(LINK_KEY, &minted.private_hex())?;
            Ok(minted)
        }
    }
}

/// Forget the key: every pairing made with it is void until the phone pairs again.
pub fn forget_link_key(store: &dyn Store) -> Result<(), Error> {
    store.delete(LINK_KEY)
}

/// A name the desktop will take: the proof carries it, and the gateway refuses an empty one
/// or one over 64 bytes after the code is spent, so it is refused here first.
fn checked_name(name: &str) -> Result<&str, Error> {
    let name = name.trim();
    if name.is_empty() || name.len() > 64 {
        return Err(Error::Store(
            "the phone's name must be 1 to 64 characters".into(),
        ));
    }
    Ok(name)
}

/// The device, minted on the first ask. `name` renames it where it is given; `None` keeps the
/// stored name, or the default for a device minted now.
pub fn device(store: &dyn Store, name: Option<&str>) -> Result<Device, Error> {
    let name = name.map(checked_name).transpose()?;
    let mut device = match store.get(DEVICE)? {
        Some(json) => serde_json::from_str::<Device>(&json)
            .map_err(|err| Error::Store(format!("the device record is unreadable: {err}")))?,
        None => Device {
            id: ulid::Ulid::generate().to_string(),
            name: name.unwrap_or(DEFAULT_NAME).to_owned(),
        },
    };
    let stored = store.get(DEVICE)?.is_some();
    let renamed = name.is_some_and(|name| name != device.name);
    if let Some(name) = name
        && renamed
    {
        device.name = name.to_owned();
    }
    if !stored || renamed {
        let json = serde_json::to_string(&device).map_err(|err| Error::Store(err.to_string()))?;
        store.set(DEVICE, &json)?;
    }
    Ok(device)
}

/// Rename the device: what the desktop shows from the next pairing on.
pub fn rename(store: &dyn Store, name: &str) -> Result<Device, Error> {
    device(store, Some(name))
}
