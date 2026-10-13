//! Where the phone keeps its few records, each a string under a key: the link key, the
//! device's own id and name, the pinned desktops, the cursors. Three stores: memory (tests),
//! a directory of private files (the app's data dir), and the Keychain (the link key).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use crate::Error;

pub trait Store: Send + Sync {
    fn get(&self, key: &str) -> Result<Option<String>, Error>;
    fn set(&self, key: &str, value: &str) -> Result<(), Error>;
    fn delete(&self, key: &str) -> Result<(), Error>;
}

/// A key is a short name: lowercase letters, digits and dashes.
fn checked(key: &str) -> Result<&str, Error> {
    if !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        Ok(key)
    } else {
        Err(Error::Store(format!("{key:?} is not a record's name")))
    }
}

/// Records held in memory: for a test, and for nothing else.
#[derive(Default)]
pub struct MemoryStore(Mutex<HashMap<String, String>>);

impl Store for MemoryStore {
    fn get(&self, key: &str) -> Result<Option<String>, Error> {
        Ok(self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(checked(key)?)
            .cloned())
    }

    fn set(&self, key: &str, value: &str) -> Result<(), Error> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(checked(key)?.to_owned(), value.to_owned());
        Ok(())
    }

    fn delete(&self, key: &str) -> Result<(), Error> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(checked(key)?);
        Ok(())
    }
}

/// One private file per key under `dir`, written whole to a temporary file beside it and
/// renamed over, so a write that fails leaves the old record as it was.
pub struct FileStore {
    dir: PathBuf,
}

impl FileStore {
    pub fn new(dir: impl Into<PathBuf>) -> FileStore {
        FileStore { dir: dir.into() }
    }

    fn path(&self, key: &str) -> Result<PathBuf, Error> {
        Ok(self.dir.join(format!("{}.json", checked(key)?)))
    }
}

fn io(what: &str, path: &Path, err: std::io::Error) -> Error {
    Error::Store(format!("{what} {}: {err}", path.display()))
}

impl Store for FileStore {
    fn get(&self, key: &str) -> Result<Option<String>, Error> {
        let path = self.path(key)?;
        match std::fs::read_to_string(&path) {
            Ok(text) => Ok(Some(text)),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(io("could not read", &path, err)),
        }
    }

    fn set(&self, key: &str, value: &str) -> Result<(), Error> {
        let path = self.path(key)?;
        std::fs::create_dir_all(&self.dir).map_err(|err| io("could not make", &self.dir, err))?;
        let tmp = path.with_extension("json.tmp");
        {
            let mut open = std::fs::OpenOptions::new();
            open.write(true).create(true).truncate(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                open.mode(0o600);
            }
            let mut file = open
                .open(&tmp)
                .map_err(|err| io("could not write", &tmp, err))?;
            use std::io::Write;
            file.write_all(value.as_bytes())
                .and_then(|()| file.sync_all())
                .map_err(|err| io("could not write", &tmp, err))?;
        }
        std::fs::rename(&tmp, &path).map_err(|err| {
            let _ = std::fs::remove_file(&tmp);
            io("could not replace", &path, err)
        })
    }

    fn delete(&self, key: &str) -> Result<(), Error> {
        let path = self.path(key)?;
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(io("could not remove", &path, err)),
        }
    }
}

/// The Keychain, for the link key (iOS design §4.1): the data-protection keychain through
/// `apple-native-keyring-store`'s `protected` store, each item `WhenUnlockedThisDeviceOnly` and
/// never synchronised, so the key is readable only on this phone while it is unlocked and is
/// in no backup. Service `purlis/phone`, account = the record's name. The same store serves a
/// Mac (10.15 and up), where a desktop build of the phone app runs.
#[cfg(any(target_os = "ios", target_os = "macos"))]
pub struct KeychainStore {
    service: String,
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
impl KeychainStore {
    /// The store, with the Keychain set as the process's credential store once.
    pub fn new() -> Result<KeychainStore, Error> {
        if keyring_core::get_default_store().is_none() {
            let store = apple_native_keyring_store::protected::Store::new()
                .map_err(|err| Error::Store(format!("the Keychain could not be reached: {err}")))?;
            keyring_core::set_default_store(store);
        }
        Ok(KeychainStore {
            service: "purlis/phone".into(),
        })
    }

    fn entry(&self, key: &str) -> Result<keyring_core::Entry, Error> {
        let modifiers = HashMap::from([("access-policy", "WhenUnlockedThisDeviceOnly")]);
        keyring_core::Entry::new_with_modifiers(&self.service, checked(key)?, &modifiers)
            .map_err(|err| Error::Store(format!("the Keychain could not be reached: {err}")))
    }
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
impl Store for KeychainStore {
    fn get(&self, key: &str) -> Result<Option<String>, Error> {
        match self.entry(key)?.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring_core::Error::NoEntry) => Ok(None),
            Err(err) => Err(Error::Store(format!(
                "the Keychain refused the read: {err}"
            ))),
        }
    }

    fn set(&self, key: &str, value: &str) -> Result<(), Error> {
        self.entry(key)?
            .set_password(value)
            .map_err(|err| Error::Store(format!("the Keychain refused the write: {err}")))
    }

    fn delete(&self, key: &str) -> Result<(), Error> {
        match self.entry(key)?.delete_credential() {
            Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
            Err(err) => Err(Error::Store(format!(
                "the Keychain refused the delete: {err}"
            ))),
        }
    }
}
