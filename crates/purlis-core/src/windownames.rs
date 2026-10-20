// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The names the window keeps for its links to its own host** (#2623, plane-format.md).
//!
//! The window holds each project root on a link of its own to its session host, and names
//! itself on it (`name_window`, session protocol 1.31) by the name it keeps for that root here: a
//! random 128-bit name, minted the first time the root's link opens and the same on every link
//! after it, the window's restarts included. A host that relays the link to a runner names the
//! window there by a digest of it, so the window that comes back takes back its runner chats,
//! and a client that does not hold the name takes none of them.
//!
//! **The name is the window's alone.** It is kept in `<config>/purlisd/window-names.json`, `0600`
//! in the session host's `0700` folder, which every chat's sandbox is denied reading and
//! writing. It is never committed, never sent anywhere but to the window's own host over its
//! `local-ui` link, and never written to a log: [`Name`] shows nothing in `Debug`. Losing the
//! file loses only the take-back after the next restart; a name is minted again.

use std::collections::BTreeMap;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// The file's name, in the session host's folder ([`crate::machine::charterd`]).
pub const FILE: &str = "window-names.json";

/// The lock that makes one read-modify-write of [`FILE`] one act, beside it.
pub const LOCK: &str = "window-names.json.lock";

/// How many bytes a name is made of: 128 bits.
const A_NAME_IS: usize = 16;

/// The most roots the file keeps a name for. Past it, the name of a root not used for longest is
/// let go of: its link names itself anew, and takes nothing back after the window's next start.
/// Never one whose link is open now.
const MOST_NAMES: usize = 256;

/// **The name the window keeps for one root's link**, as hex digits. Never shown by `Debug`.
#[derive(Clone, PartialEq, Eq)]
pub struct Name(String);

impl Name {
    /// The name itself, for the one place that says it: the window's link to its own host.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Name {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Name(withheld)")
    }
}

/// What [`FILE`] holds.
#[derive(Default, Serialize, Deserialize)]
struct Kept {
    version: u32,
    /// Each root's name, by a digest of the root's path ([`key_of`]): a fixed size, so the file
    /// stays well within what a private file is read up to.
    names: BTreeMap<String, KeptName>,
}

/// One root's name, as [`FILE`] holds it.
#[derive(Clone, Serialize, Deserialize)]
struct KeptName {
    name: String,
    /// When its link last named itself, in milliseconds since the epoch: what [`MOST_NAMES`] lets go
    /// of first.
    used: u64,
}

/// **The name the window keeps for `root`'s link**, in the config home `config_root`: the one it
/// keeps, or one minted now and kept. A file that cannot be read as one, or is too long to be
/// one, is started again. The
/// roots in `open`, whose links are open now, are never let go of to make room.
pub fn name_for(config_root: &Path, root: &Path, open: &[std::path::PathBuf]) -> io::Result<Name> {
    let (folder, _lock) = locked(config_root)?;
    let mut kept = read(&folder)?;
    let key = key_of(root);
    let used = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| {
            u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
        });
    let name = match kept.names.get(&key) {
        Some(one) if is_a_name(&one.name) => one.name.clone(),
        _ => mint()?,
    };
    kept.names.insert(
        key.clone(),
        KeptName {
            name: name.clone(),
            used,
        },
    );
    // Never the root asked for, nor one whose link is open now.
    let mut open: Vec<String> = open.iter().map(|root| key_of(root)).collect();
    open.push(key);
    while kept.names.len() > MOST_NAMES {
        let Some(oldest) = kept
            .names
            .iter()
            .filter(|(root, _)| !open.contains(root))
            .min_by_key(|(_, one)| one.used)
            .map(|(root, _)| root.clone())
        else {
            break;
        };
        kept.names.remove(&oldest);
    }
    write(&folder, &kept)?;
    Ok(Name(name))
}

/// **Lets go of the name kept for `root`'s link** (#2738), as the person forgets the project:
/// a link of the root that opens again names itself anew, and takes nothing back. Every other
/// root's name is kept. Where no names are kept, nothing is written.
pub fn forget(config_root: &Path, root: &Path) -> io::Result<()> {
    if !crate::machine::charterd(config_root).join(FILE).exists() {
        return Ok(());
    }
    let (folder, _lock) = locked(config_root)?;
    let mut kept = read(&folder)?;
    if kept.names.remove(&key_of(root)).is_some() {
        write(&folder, &kept)?;
    }
    Ok(())
}

/// The session host's folder, and the [`LOCK`] held on it: what makes a read-modify-write of
/// [`FILE`] one act, until the file is dropped.
fn locked(config_root: &Path) -> io::Result<(purlis_same_user::PrivateDirectory, std::fs::File)> {
    let folder = purlis_same_user::PrivateDirectory::open(&crate::machine::charterd(config_root))?;
    let lock = {
        use std::os::unix::fs::OpenOptionsExt;
        std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(folder.path().join(LOCK))?
    };
    lock.lock()?;
    Ok((folder, lock))
}

/// What [`FILE`] in `folder` keeps: nothing where there is none, or where it does not read as
/// one, which starts it again. So does a file of this user's alone that is past what a private
/// file is read up to (#2744): it is no file the window wrote, and refusing it would leave every
/// later link unnamed. A file another user could read or write, or a link, is still refused.
fn read(folder: &purlis_same_user::PrivateDirectory) -> io::Result<Kept> {
    let path = folder.path().join(FILE);
    let mut kept: Kept = match purlis_same_user::read_private_file(&path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Kept::default(),
        Err(err) if too_long_but_private(&path) => {
            tracing::warn!("purlis: the window's names are started again: {err}");
            Kept::default()
        }
        Err(err) => return Err(err),
    };
    kept.version = 1;
    Ok(kept)
}

/// Whether `path` is a regular file, not a link, of this user's that nobody else may read or
/// write, and longer than [`purlis_same_user::read_private_file`] reads.
fn too_long_but_private(path: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    std::fs::symlink_metadata(path).is_ok_and(|found| {
        found.file_type().is_file()
            && found.uid() == purlis_same_user::Uid::effective().as_raw()
            && found.mode() & 0o077 == 0
            && found.len() > purlis_same_user::A_PRIVATE_FILE_IS_AT_MOST
    })
}

fn write(folder: &purlis_same_user::PrivateDirectory, kept: &Kept) -> io::Result<()> {
    let text = serde_json::to_vec(kept).map_err(io::Error::other)?;
    folder.replace_file(FILE, &text)
}

/// What the file keeps `root`'s name under: a SHA-256 of its path, in hex.
fn key_of(root: &Path) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(root.as_os_str().as_encoded_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn mint() -> io::Result<String> {
    let mut bytes = [0u8; A_NAME_IS];
    getrandom::fill(&mut bytes).map_err(io::Error::other)?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn is_a_name(name: &str) -> bool {
    name.len() == A_NAME_IS * 2 && name.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests;
