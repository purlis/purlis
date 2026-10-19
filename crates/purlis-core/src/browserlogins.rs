// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Kept browser logins** (#2021, spec #2008; the person's rulings of 2026-10-18): what a
//! browser that performed a declared login keeps once it ends, so the next chat of the same
//! profile whose login is allowed starts signed in.
//!
//! - **Whose** ([`Store`]): one login as declared and allowed, never only its name: the profile,
//!   the persona, the login's name and an identity digest of the persona, the project and the
//!   login's declaration as allowed ([`identity_of`]). Another login, of another persona or
//!   project or declared otherwise, has a store of its own.
//! - **Where**: `<data>/browser-logins/<profile>/<persona>/<login>-<identity>/state.sealed`, on
//!   this device alone: never committed, never sent, excluded from backups, and every chat is
//!   denied it, reading and writing. Removing the profile wipes its stores ([`wipe_profile`]).
//! - **Sealed at rest** ([`seal`], [`unseal`]): the browser's own data, less what it caches, as
//!   one archive sealed with an AEAD under a key of this device's alone, kept in the keyring as
//!   `purlis/@browser-logins/<device>` with each store's generation ([`Keys`]). The store's place
//!   and generation are sealed in with it, so a store moved elsewhere, or an older copy of one,
//!   does not open. A store that does not open is never half opened: the browser starts fresh.
//! - **One writer** ([`hold`]): a browser that holds a store's lock writes it back when it ends;
//!   any other browser opened from it is a copy, and what it changes is discarded.
//! - **A browser's folder says whose it is** ([`Mark`]): a mark beside it, held locked while the
//!   browser runs, so what a browser that ended outright left behind is found and removed
//!   ([`sweep`]).

use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Read as _, Write as _};
use std::path::{Path, PathBuf};

use ring::aead::{self, Aad, LessSafeKey, Nonce, UnboundKey};
use ring::rand::{SecureRandom as _, SystemRandom};
use sha2::Digest as _;

use crate::secrets::keyring;

/// The folder of every kept store, under the data home.
pub const DIR: &str = "browser-logins";

/// A store's sealed file.
pub const FILE: &str = "state.sealed";

/// A store's lock, beside its sealed file.
const LOCK: &str = "lock";

/// The keyring's lock, in `<data>/browser-logins/`.
const KEYS_LOCK: &str = ".keys.lock";

/// What a sealed file starts with: its kind and the version it is written at.
const MAGIC: &[u8; 8] = b"PURLBL02";

/// The keyring's service for this device's key: `purlis/@browser-logins/<device>`.
pub const SERVICE: &str = "purlis/@browser-logins";

/// The keyring's account the key is kept under.
pub const ACCOUNT: &str = "key";

/// What a mark beside a browser's folder is named after: `<folder>.kept`.
pub const MARK: &str = "kept";

/// The most a store may hold, unsealed: a signed-in profile's cookies and storage, without
/// caches, is a few megabytes.
pub const MOST_BYTES: u64 = 256 << 20;

/// What of a browser's own data is never kept: what it caches, its own home and its log, as the
/// engine lays its folder out.
#[cfg(unix)]
use purlis_browser_engine::launch::NOT_KEPT;
#[cfg(not(unix))]
const NOT_KEPT: &[&str] = &[];

/// Whether `name` is a plain name a store's place may hold.
fn plain(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && !name.starts_with('.')
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

/// **What a login is declared as**, for its identity: its vault, the keys its user and password
/// are read from, and its page.
#[derive(Debug, Clone, Copy)]
pub struct Declared<'a> {
    pub vault: &'a str,
    pub user_key: Option<&'a str>,
    pub password_key: &'a str,
    pub page: &'a str,
}

/// **A login's identity** (#2021): a digest of the persona whose login it is, the project it is
/// declared in, and its declaration as allowed (its vault, keys and page). Another persona's, or
/// project's, or a declaration changed since, is another identity; how a login is worded to the
/// person is no part of it. Each field is tagged and length-prefixed under a versioned domain, so
/// no two declarations run together into one.
pub fn identity_of(persona: &str, project: &str, declared: &Declared<'_>) -> String {
    let mut digest = sha2::Sha256::new();
    digest.update(b"purlis kept browser login identity v1\0");
    let mut field = |tag: u8, value: Option<&str>| {
        digest.update([tag]);
        match value {
            Some(value) => {
                digest.update([1]);
                digest.update((value.len() as u64).to_be_bytes());
                digest.update(value.as_bytes());
            }
            None => digest.update([0]),
        }
    };
    field(b'p', Some(persona));
    field(b'j', Some(project));
    field(b'v', Some(declared.vault));
    field(b'u', declared.user_key);
    field(b'k', Some(declared.password_key));
    field(b'a', Some(declared.page));
    digest
        .finalize()
        .iter()
        .take(16)
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// **One kept store**: whose login, by its profile, persona, name and identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Store {
    profile: String,
    persona: String,
    login: String,
    identity: String,
}

impl Store {
    /// The store of `profile`'s login `login` of `persona` whose identity is `identity`
    /// ([`identity_of`]), or why the names cannot name one: each a plain name of letters,
    /// digits, `.`, `_` and `-`, and the identity hex.
    pub fn new(profile: &str, persona: &str, login: &str, identity: &str) -> Result<Self, String> {
        for (what, name) in [("profile", profile), ("persona", persona), ("login", login)] {
            if !plain(name) {
                return Err(format!(
                    "a {what}'s name holds only letters, digits, '.', '_' and '-', so no kept \
                     logins are kept for it"
                ));
            }
        }
        if identity.len() < 16 || !identity.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("a kept login's identity does not read".to_owned());
        }
        Ok(Self {
            profile: profile.to_owned(),
            persona: persona.to_owned(),
            login: login.to_owned(),
            identity: identity.to_ascii_lowercase(),
        })
    }

    /// The profile it is kept under.
    pub fn profile(&self) -> &str {
        &self.profile
    }

    /// Its place, as its folder, the keyring and its seal name it.
    pub fn place(&self) -> String {
        format!(
            "{}/{}/{}-{}",
            self.profile, self.persona, self.login, self.identity
        )
    }

    /// Its folder under the data home `data`.
    pub fn dir(&self, data: &Path) -> PathBuf {
        data.join(DIR)
            .join(&self.profile)
            .join(&self.persona)
            .join(format!("{}-{}", self.login, self.identity))
    }

    /// Whether it has been kept.
    pub fn exists(&self, data: &Path) -> bool {
        self.dir(data).join(FILE).is_file()
    }
}

/// **This device's key for kept logins, and each store's generation**, in `store`, the
/// keyring: one item, `purlis/@browser-logins/<device>`.
pub struct Keys<'a> {
    pub store: &'a dyn keyring::Store,
    pub device: String,
}

/// What the keyring item holds.
struct Held {
    key: [u8; 32],
    /// Each store's place and the generation it was last written at.
    stores: BTreeMap<String, u64>,
}

impl Drop for Held {
    fn drop(&mut self) {
        zeroize::Zeroize::zeroize(&mut self.key);
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Written {
    key: String,
    #[serde(default)]
    stores: BTreeMap<String, u64>,
}

impl Keys<'_> {
    fn service(&self) -> String {
        format!("{SERVICE}/{}", self.device)
    }

    fn read(&self) -> Result<Option<Held>, String> {
        let unread = || "this device's key for kept logins does not read".to_owned();
        let Some(kept) = self.store.get(&self.service(), ACCOUNT).map_err(|why| {
            format!("this device's key for kept logins could not be read ({why})")
        })?
        else {
            return Ok(None);
        };
        let mut text = kept.into_inner();
        let written: Result<Written, _> = serde_json::from_str(&text);
        zeroize::Zeroize::zeroize(&mut text);
        let written = written.map_err(|_| unread())?;
        let mut bytes = base64::engine::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            written.key.as_bytes(),
        )
        .map_err(|_| unread())?;
        let key = <[u8; 32]>::try_from(bytes.as_slice()).map_err(|_| unread());
        zeroize::Zeroize::zeroize(&mut bytes);
        Ok(Some(Held {
            key: key?,
            stores: written.stores,
        }))
    }

    fn write(&self, held: &Held) -> Result<(), String> {
        let mut text = serde_json::to_string(&Written {
            key: base64::engine::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                held.key,
            ),
            stores: held.stores.clone(),
        })
        .map_err(|_| "this device's key for kept logins was not kept".to_owned())?;
        let kept = self
            .store
            .set(&self.service(), ACCOUNT, &text)
            .map(|_| ())
            .map_err(|why| format!("this device's key for kept logins was not kept ({why})"));
        zeroize::Zeroize::zeroize(&mut text);
        kept
    }

    /// What the item holds, made on first use.
    fn read_or_make(&self) -> Result<Held, String> {
        if let Some(held) = self.read()? {
            return Ok(held);
        }
        let mut key = [0u8; 32];
        SystemRandom::new()
            .fill(&mut key)
            .map_err(|_| "no key for kept logins could be made".to_owned())?;
        Ok(Held {
            key,
            stores: BTreeMap::new(),
        })
    }

    /// Forgets the item: what was sealed with its key never opens again.
    fn forget(&self) -> Result<(), String> {
        self.store
            .delete(&self.service(), ACCOUNT)
            .map(|_| ())
            .map_err(|why| format!("this device's key for kept logins was not forgotten ({why})"))
    }
}

/// What a store is sealed to: its own place and the generation it is written at, so a sealed
/// file opens only where, and as last, it was written.
fn bound_to(place: &str, generation: u64) -> Vec<u8> {
    format!("purlis kept browser logins v2\n{place}\n{generation}").into_bytes()
}

fn sealing_key(key: &[u8; 32]) -> Result<LessSafeKey, String> {
    UnboundKey::new(&aead::CHACHA20_POLY1305, key)
        .map(LessSafeKey::new)
        .map_err(|_| "kept logins could not be sealed".to_owned())
}

/// **`folder`, a browser's own data, sealed** for the store at `place`, at `generation`, under
/// `key`: everything in it but what [`NOT_KEPT`] names, as regular files and folders alone (a
/// link, a device or a socket is never kept).
pub fn seal(
    folder: &Path,
    place: &str,
    generation: u64,
    key: &[u8; 32],
) -> Result<Vec<u8>, String> {
    let mut archive = tar::Builder::new(Vec::new());
    archive.follow_symlinks(false);
    let mut total: u64 = 0;
    add(&mut archive, folder, Path::new(""), &mut total)?;
    let mut plain = archive
        .into_inner()
        .map_err(|e| format!("kept logins could not be gathered ({e})"))?;
    let mut nonce = [0u8; aead::NONCE_LEN];
    SystemRandom::new()
        .fill(&mut nonce)
        .map_err(|_| "kept logins could not be sealed".to_owned())?;
    sealing_key(key)?
        .seal_in_place_append_tag(
            Nonce::assume_unique_for_key(nonce),
            Aad::from(bound_to(place, generation)),
            &mut plain,
        )
        .map_err(|_| "kept logins could not be sealed".to_owned())?;
    let mut out = Vec::with_capacity(MAGIC.len() + nonce.len() + plain.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&plain);
    zeroize::Zeroize::zeroize(&mut plain);
    Ok(out)
}

fn add(
    archive: &mut tar::Builder<Vec<u8>>,
    root: &Path,
    under: &Path,
    total: &mut u64,
) -> Result<(), String> {
    let here = root.join(under);
    let mut entries: Vec<_> = fs::read_dir(&here)
        .map_err(|e| format!("kept logins could not be gathered ({e})"))?
        .filter_map(Result::ok)
        .collect();
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let name = entry.file_name();
        if NOT_KEPT.iter().any(|skip| name == *skip) {
            continue;
        }
        let path = under.join(&name);
        let kind = entry
            .file_type()
            .map_err(|e| format!("kept logins could not be gathered ({e})"))?;
        if kind.is_dir() {
            archive
                .append_dir(&path, root.join(&path))
                .map_err(|e| format!("kept logins could not be gathered ({e})"))?;
            add(archive, root, &path, total)?;
        } else if kind.is_file() {
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            *total += size;
            if *total > MOST_BYTES {
                return Err(format!(
                    "the browser keeps more than {} MiB, so its logins were not kept",
                    MOST_BYTES >> 20
                ));
            }
            let mut file = fs::File::open(root.join(&path))
                .map_err(|e| format!("kept logins could not be gathered ({e})"))?;
            archive
                .append_file(&path, &mut file)
                .map_err(|e| format!("kept logins could not be gathered ({e})"))?;
        }
    }
    Ok(())
}

/// **What `sealed` held, written into `into`**, a fresh folder, where it opens under `key` for
/// the store at `place` as written at `generation`: regular files and folders alone, each inside
/// `into`, `0600` and `0700`. Anything else, a store that does not open included, is refused.
pub fn unseal(
    sealed: &[u8],
    place: &str,
    generation: u64,
    key: &[u8; 32],
    into: &Path,
) -> Result<(), String> {
    let refused = || "the kept logins do not open on this device, so none were used".to_owned();
    let rest = sealed.strip_prefix(MAGIC.as_slice()).ok_or_else(refused)?;
    if rest.len() < aead::NONCE_LEN {
        return Err(refused());
    }
    let (nonce, body) = rest.split_at(aead::NONCE_LEN);
    let nonce = Nonce::try_assume_unique_for_key(nonce).map_err(|_| refused())?;
    let mut body = body.to_vec();
    let plain = sealing_key(key)?
        .open_in_place(nonce, Aad::from(bound_to(place, generation)), &mut body)
        .map_err(|_| refused())?;
    let written = unpack(plain, into);
    zeroize::Zeroize::zeroize(&mut body);
    written
}

fn unpack(plain: &[u8], into: &Path) -> Result<(), String> {
    let refused = |why: String| format!("the kept logins could not be opened ({why})");
    let mut archive = tar::Archive::new(plain);
    let mut total: u64 = 0;
    for entry in archive.entries().map_err(|e| refused(e.to_string()))? {
        let mut entry = entry.map_err(|e| refused(e.to_string()))?;
        let path = entry
            .path()
            .map_err(|e| refused(e.to_string()))?
            .into_owned();
        // Inside `into` alone: a plain relative path, nothing that climbs out.
        if path.is_absolute()
            || path
                .components()
                .any(|part| !matches!(part, std::path::Component::Normal(_)))
        {
            return Err(refused("an entry named outside its folder".into()));
        }
        let to = into.join(&path);
        match entry.header().entry_type() {
            tar::EntryType::Directory => private_dir(&to).map_err(|e| refused(e.to_string()))?,
            tar::EntryType::Regular => {
                total += entry.size();
                if total > MOST_BYTES {
                    return Err(refused("more than a store may hold".into()));
                }
                if let Some(parent) = to.parent() {
                    private_dir(parent).map_err(|e| refused(e.to_string()))?;
                }
                let mut bytes = Vec::new();
                entry
                    .read_to_end(&mut bytes)
                    .map_err(|e| refused(e.to_string()))?;
                private_file(&to, &bytes).map_err(|e| refused(e.to_string()))?;
            }
            // A link, a device or anything else is never written.
            _ => return Err(refused("an entry that is no file or folder".into())),
        }
    }
    Ok(())
}

/// `path`, a folder of a store's place, made this user's alone; refused where it is a link
/// (or anything but a folder), so no store is written through one.
fn place_dir(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        purlis_same_user::private_directory(path)
    }
    #[cfg(not(unix))]
    {
        private_dir(path)
    }
}

fn private_dir(path: &Path) -> io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(path)
}

fn private_file(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path)?.write_all(bytes)
}

/// What macOS's backup reads in a folder's `com_apple_backup_excludeItem` to leave it out, as
/// `tmutil addexclusion` writes it: a binary property list of the string `com.apple.backupd`.
#[cfg(target_os = "macos")]
pub const BACKUP_EXCLUDED: &[u8] = &[
    0x62, 0x70, 0x6c, 0x69, 0x73, 0x74, 0x30, 0x30, 0x5f, 0x10, 0x11, 0x63, 0x6f, 0x6d, 0x2e, 0x61,
    0x70, 0x70, 0x6c, 0x65, 0x2e, 0x62, 0x61, 0x63, 0x6b, 0x75, 0x70, 0x64, 0x08, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1c,
];

/// The extended attribute macOS's backup leaves a folder out by.
#[cfg(target_os = "macos")]
pub const BACKUP_EXCLUSION: &str = "com.apple.metadata:com_apple_backup_excludeItem";

/// **`dir` left out of this device's backups** (the person's ruling of 2026-10-18): on macOS by
/// the exclusion `tmutil addexclusion` sets. Elsewhere, nothing yet.
pub fn exclude_from_backups(dir: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    xattr::set(dir, BACKUP_EXCLUSION, BACKUP_EXCLUDED).map_err(|e| {
        format!(
            "{} could not be left out of this Mac's backups ({e})",
            dir.display()
        )
    })?;
    let _ = dir;
    Ok(())
}

/// **`<data>/browser-logins/`, made this user's alone and left out of backups.**
fn kept_dir(data: &Path) -> Result<PathBuf, String> {
    let dir = data.join(DIR);
    place_dir(&dir).map_err(|e| format!("the kept logins' folder could not be made ({e})"))?;
    exclude_from_backups(&dir)?;
    Ok(dir)
}

/// **The keyring's lock** (#2021): held around every read, change and write of the keyring item,
/// so two browsers ending at once, or a profile's removal meanwhile, never lose a generation,
/// and no two make a key.
struct KeysLock(fs::File);

fn lock_keys(data: &Path) -> Result<KeysLock, String> {
    let dir = kept_dir(data)?;
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join(KEYS_LOCK))
        .map_err(|e| format!("the kept logins' key could not be held ({e})"))?;
    file.lock()
        .map_err(|e| format!("the kept logins' key could not be held ({e})"))?;
    Ok(KeysLock(file))
}

impl Drop for KeysLock {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}

/// **A browser's hold on one store**: its lock, kept for as long as the browser writes it.
#[derive(Debug)]
pub struct Hold {
    lock: fs::File,
    store: Store,
    data: PathBuf,
    dir: PathBuf,
}

/// **The hold on `store`**, or `None` where another browser holds it: then a browser opened
/// from it is a copy.
pub fn hold(data: &Path, store: &Store) -> Result<Option<Hold>, String> {
    let root = kept_dir(data)?;
    let dir = store.dir(data);
    let made = |e: io::Error| format!("the kept logins' folder could not be made ({e})");
    let profile = root.join(&store.profile);
    let persona = profile.join(&store.persona);
    for each in [&profile, &persona, &dir] {
        place_dir(each).map_err(made)?;
    }
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join(LOCK))
        .map_err(|e| format!("the kept logins could not be held ({e})"))?;
    match lock.try_lock() {
        Ok(()) => Ok(Some(Hold {
            lock,
            store: store.clone(),
            data: data.to_path_buf(),
            dir,
        })),
        Err(fs::TryLockError::WouldBlock) => Ok(None),
        Err(fs::TryLockError::Error(e)) => Err(format!("the kept logins could not be held ({e})")),
    }
}

impl Hold {
    /// Whether the lock it holds is still the store's: not, once the store was wiped (its
    /// profile removed) while the browser ran. Then nothing of it is written again.
    fn still_there(&self) -> bool {
        let Ok(now) = fs::metadata(self.dir.join(LOCK)) else {
            return false;
        };
        let Ok(held) = self.lock.metadata() else {
            return false;
        };
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt as _;
            now.dev() == held.dev() && now.ino() == held.ino()
        }
        #[cfg(not(unix))]
        {
            let _ = (now, held);
            true
        }
    }

    /// The store it holds.
    pub fn store(&self) -> &Store {
        &self.store
    }

    /// **Writes `folder`, a browser's own data, back as the store**, sealed under this device's
    /// key at the store's next generation; the file is replaced whole, never written over in
    /// place, and the keyring learns the generation once the file is there.
    pub fn keep(&self, folder: &Path, keys: &Keys<'_>) -> Result<(), String> {
        // Under the keyring's lock, which a profile's removal holds as it wipes.
        let _keys = lock_keys(&self.data)?;
        if !self.still_there() {
            return Err("its profile's kept logins were wiped, so nothing is kept".to_owned());
        }
        let mut held = keys.read_or_make()?;
        let place = self.store.place();
        let generation = held.stores.get(&place).copied().unwrap_or(0) + 1;
        let sealed = seal(folder, &place, generation, &held.key)?;
        let beside = self.dir.join(format!("{FILE}.{}", std::process::id()));
        let _ = fs::remove_file(&beside);
        private_file(&beside, &sealed)
            .and_then(|()| fs::rename(&beside, self.dir.join(FILE)))
            .map_err(|e| {
                let _ = fs::remove_file(&beside);
                format!("the kept logins could not be written ({e})")
            })?;
        held.stores.insert(place, generation);
        keys.write(&held)
    }
}

/// **Opens `store` into `into`**, a browser's fresh data folder: whether there was one that
/// opened, as last written. A store that does not open writes nothing into `into`.
pub fn open_into(data: &Path, store: &Store, into: &Path, keys: &Keys<'_>) -> Result<bool, String> {
    let file = store.dir(data).join(FILE);
    let sealed = match fs::read(&file) {
        Ok(sealed) => sealed,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(format!("the kept logins could not be read ({e})")),
    };
    let Some(held) = keys.read()? else {
        return Ok(false);
    };
    let place = store.place();
    let Some(generation) = held.stores.get(&place).copied() else {
        return Err("the kept logins do not open on this device, so none were used".to_owned());
    };
    let staged = opening(into);
    let _ = fs::remove_dir_all(&staged);
    private_dir(&staged).map_err(|e| format!("the kept logins could not be opened ({e})"))?;
    match unseal(&sealed, &place, generation, &held.key, &staged) {
        Ok(()) => {
            let _ = fs::remove_dir_all(into);
            fs::rename(&staged, into).map_err(|e| {
                let _ = fs::remove_dir_all(&staged);
                format!("the kept logins could not be opened ({e})")
            })?;
            Ok(true)
        }
        Err(why) => {
            let _ = fs::remove_dir_all(&staged);
            Err(why)
        }
    }
}

/// Where a store is unsealed into before it becomes `into`.
fn opening(into: &Path) -> PathBuf {
    into.with_extension("opening")
}

/// **A browser folder's mark** (#2021): beside the folder (`<folder>.kept`), naming whose kept
/// login its browser holds, locked for as long as the browser runs. Made, and locked, before
/// anything of a store is written into the folder.
#[derive(Debug)]
pub struct Mark {
    _held: fs::File,
}

/// **Marks `folder` as the browser of `store`'s**, held while it runs.
pub fn mark(folder: &Path, store: &Store) -> Result<Mark, String> {
    let path = folder.with_extension(MARK);
    if let Some(parent) = path.parent() {
        private_dir(parent)
            .map_err(|e| format!("the browser's folder could not be marked ({e})"))?;
    }
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)
        .map_err(|e| format!("the browser's folder could not be marked ({e})"))?;
    // Locked before a word is written, so no sweep reads a half-written mark as one left behind.
    file.lock()
        .map_err(|e| format!("the browser's folder could not be marked ({e})"))?;
    file.set_len(0)
        .and_then(|()| {
            (&file).write_all(format!("{}\n{}", store.profile, store.place()).as_bytes())
        })
        .map_err(|e| format!("the browser's folder could not be marked ({e})"))?;
    Ok(Mark { _held: file })
}

/// **Removes what a browser that ended outright left behind under `folders`** (as #2204 sweeps):
/// every folder whose mark `names` says so and that no running browser holds, with its mark and
/// what it was being opened into; and every folder being opened into that nothing holds. `own`,
/// a folder about to be used, is never one.
pub fn sweep(folders: &Path, own: Option<&Path>, names: &dyn Fn(&str, &str) -> bool) {
    let Ok(entries) = fs::read_dir(folders) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let extension = path.extension().and_then(|e| e.to_str());
        let folder = path.with_extension("");
        if own.is_some_and(|own| own == folder) {
            continue;
        }
        let held_by_none = |mark: &Path| match fs::OpenOptions::new().write(true).open(mark) {
            Ok(file) => file.try_lock().is_ok(),
            Err(e) => e.kind() == io::ErrorKind::NotFound,
        };
        match extension {
            Some(MARK) => {
                let said = fs::read_to_string(&path).unwrap_or_default();
                let (profile, place) = said.split_once('\n').unwrap_or_default();
                if names(profile, place) && held_by_none(&path) {
                    let _ = fs::remove_dir_all(&folder);
                    let _ = fs::remove_dir_all(opening(&folder));
                    let _ = fs::remove_file(&path);
                }
            }
            Some("opening") if held_by_none(&folder.with_extension(MARK)) => {
                let _ = fs::remove_dir_all(&path);
            }
            _ => {}
        }
    }
}

/// **Wipes every kept store of `profile`**, as its profile is removed or forgotten, with every
/// browser folder of it that a browser left behind under `folders`, and this device's key where
/// no store is left. Whether there was anything to wipe. A keyring that cannot be asked is
/// said, and the stores are wiped all the same.
pub fn wipe_profile(
    data: &Path,
    folders: &Path,
    profile: &str,
    keys: &Keys<'_>,
) -> Result<bool, String> {
    if !plain(profile) {
        return Ok(false);
    }
    sweep(folders, None, &|of, _| of == profile);
    if !data.join(DIR).is_dir() {
        return Ok(false);
    }
    let _keys = lock_keys(data)?;
    let dir = data.join(DIR).join(profile);
    let had = match fs::remove_dir_all(&dir) {
        Ok(()) => true,
        Err(e) if e.kind() == io::ErrorKind::NotFound => false,
        Err(e) => {
            return Err(format!(
                "the profile's kept logins could not be wiped ({e})"
            ));
        }
    };
    // A store is a folder: the keyring's lock beside them is none.
    let left = fs::read_dir(data.join(DIR))
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .any(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        })
        .unwrap_or(false);
    let forgotten = if left {
        keys.read()
            .map(|held| match held {
                Some(mut held) => {
                    let prefix = format!("{profile}/");
                    held.stores.retain(|place, _| !place.starts_with(&prefix));
                    keys.write(&held)
                }
                None => Ok(()),
            })
            .and_then(|done| done)
    } else {
        keys.forget()
    };
    if let Err(why) = forgotten {
        tracing::warn!("purlis: {why}; the profile's kept logins were wiped all the same");
    }
    Ok(had)
}

#[cfg(all(test, unix))]
#[path = "browserlogins/tests.rs"]
mod tests;
