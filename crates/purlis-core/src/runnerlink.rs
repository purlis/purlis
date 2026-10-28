// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A runner's side of its link** (ADR 0078 §3 and §8, spec #2041, #2044): the runner's own
//! link key and the one desktop it is paired with.
//!
//! - **The runner mints its device id and makes its key at its one pairing** ([`pair`]), the
//!   key from the operating system's generator, and keeps the key's private half in
//!   [`KEY_FILE`], `0600`, in the session host's own folder ([`crate::machine::charterd`]),
//!   which every chat's sandbox denies reading and writing. Each runner has a device id and a
//!   key of its own: a new container is a new runner, with a new id and a new key, and a home
//!   that came with a device id is not paired.
//! - **A runner holds one peer** (ADR 0078, ruled 2026-10-15): the desktop that paired it, in
//!   `<config>/peers.json` ([`PEERS_FILE`]), which chats may neither read nor write. A runner
//!   is paired once: a second pairing is refused whichever desktop asks, and the first stays.
//! - **A kept claim's runner keeps its pairing** (#2057, ruled 2026-10-28): its home is on its
//!   pod's own volume, so a pod made again finds its device id, key and peer there, and its
//!   first step publishes the key it keeps ([`make_at_start_in`]). Its pairing is the claim's,
//!   not the pod's.
//! - **Its host answers a link only once it is paired** ([`answering`]), reading both files as
//!   each link arrives, so a pairing made while the host runs is in force at the next link. A
//!   file another user could read or write, a link in its place, or one that is not what this
//!   module writes refuses every link.

use std::path::Path;

use purlis_session_protocol::hostlink::{Identity, LinkKey, Pinned, PublicKey, Zeroizing};

/// The runner's link key's private half, as 64 lower-case hex.
pub const KEY_FILE: &str = "link-key";

/// The longest line of [`to_line`] a pairing reads or answers.
pub const MOST_LINE: u64 = 4096;

/// The devices paired with this runner, in the machine store: `{"version": 1, "peers":
/// [{"device", "key", "paired"}]}`, the key as 64 lower-case hex and `paired` in seconds since
/// the epoch. It holds one peer; a file of another shape, or with more than one, admits none.
pub const PEERS_FILE: &str = crate::machine::PEERS_FILE;

/// Pair this runner, whose config home is `config_root`, with `desktop`, once: the runner's own
/// device id and public key, for the desktop to keep.
///
/// **A runner is paired once, and only from a home that holds no pairing and no device id.**
/// Its device id is minted here ([`crate::machine::mint_device_id`]), so a home that came with
/// one, from its image or from a runner before it, is refused before anything is made, and two
/// runners started from one image never pair with one id. Its key and its peers file are each
/// made only where no file of that name is ([`purlis_same_user::PrivateDirectory::create_file`]),
/// the key first, so of two pairings at once one makes them and the other is refused, and a
/// home that already holds either is refused whichever desktop asks. What was made is read
/// back before it is answered.
pub fn pair(config_root: &Path, desktop: &Pinned) -> Result<Pinned, String> {
    pair_with(config_root, desktop, || {
        LinkKey::generate().map_err(|why| why.to_string())
    })
}

/// **Make a cluster runner's link key as its pod starts** (#2056): the pod's first step,
/// before anything else in the pod runs. The key's private half is written to `handoff`, a
/// file made here and nowhere else (`0600`, never through a link), for the runner's pairing
/// to take ([`pair_with_key_made_at_start`]); its public half, as 64 lower-case hex, to
/// `published`, which the cluster shows in the pod's status for its operator to vouch for.
pub fn make_at_start(handoff: &Path, published: &Path) -> Result<(), String> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let key = LinkKey::generate().map_err(|why| why.to_string())?;
    let hex = Zeroizing::new(hex_of(made_private(&key)?.as_ref()));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(handoff)
        .map_err(|why| {
            format!(
                "the runner's link key could not be made at {}: {why}",
                handoff.display()
            )
        })?;
    file.write_all(hex.as_bytes())
        .and_then(|()| file.sync_all())
        .map_err(|why| format!("the runner's link key could not be written: {why}"))?;
    std::fs::write(published, hex_of(key.public().as_bytes()))
        .map_err(|why| format!("the runner's public link key could not be published: {why}"))
}

/// **A cluster runner's pod's first step, with the runner's home on its pod's volume**
/// (#2057): the home is made at `home` from `image_home` (the image's own `HOME`) the first
/// time the volume is used ([`seed_home`]). A home whose config home `config` (as the runner
/// will read it, [`crate::machine::config_root_under`]) keeps the runner's pairing, as a kept
/// claim's does when its pod is made again, keeps its key: that key's public half is published,
/// and nothing is handed over, so the runner answers with the key it was paired with. Otherwise
/// the key is made as [`make_at_start`] makes it.
pub fn make_at_start_in(
    handoff: &Path,
    published: &Path,
    home: &Path,
    image_home: Option<&Path>,
    config: &Path,
    claim: &str,
) -> Result<(), String> {
    seed_home(home, image_home, claim)?;
    match answering(config)? {
        Some((ours, _desktop)) => std::fs::write(published, hex_of(ours.key.public().as_bytes()))
            .map_err(|why| {
                format!("the runner's kept public link key could not be published: {why}")
            }),
        None => make_at_start(handoff, published),
    }
}

/// **The runner's home on its pod's volume, made once** (#2057): where `home` is not yet, a
/// copy of `image_home` (its folders, files and links, each as the image holds it, nothing
/// followed) is made beside it and renamed into place, so a step stopped half-way leaves no
/// half home; with no image home, an empty folder this user's alone. A home already there is
/// left exactly as it is: it is the runner's, kept from its earlier pods. One that is not a
/// folder is refused.
///
/// **A home is made for one claim** (#2057): `claim`, the uid of the upstream claim the pod is
/// for, is written beside it ([`CLAIM_MARKER`]) before it is made, and a home already there is
/// taken only where the mark names that claim. So a volume someone made ready before the claim
/// was (whose uid they could not know) is refused rather than taken as the runner's. An image
/// `HOME` that holds the runner's home (`/`, or the volume itself) is refused too: it would be
/// copied into itself.
pub fn seed_home(home: &Path, image_home: Option<&Path>, claim: &str) -> Result<(), String> {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    let marker = home.with_file_name(CLAIM_MARKER);
    match std::fs::symlink_metadata(home) {
        Ok(found) if found.is_dir() => {
            let marked = match std::fs::read_to_string(&marker) {
                Ok(marked) => Some(marked),
                Err(why) if why.kind() == std::io::ErrorKind::NotFound => None,
                Err(why) => {
                    return Err(format!(
                        "the runner's home's claim could not be read: {why}"
                    ));
                }
            };
            if marked.as_deref().map(str::trim) != Some(claim) {
                return Err(format!(
                    "the runner's home {} was not made for this claim, so the runner is not \
                     started: its volume was made or filled elsewhere. Delete the claim and \
                     make a new one",
                    home.display()
                ));
            }
            return Ok(());
        }
        Ok(_) => {
            return Err(format!(
                "the runner's home {} is not a folder, so the runner is not started",
                home.display()
            ));
        }
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => {}
        Err(why) => return Err(format!("the runner's home could not be looked at: {why}")),
    }
    let name = home
        .file_name()
        .ok_or_else(|| format!("{} cannot be a runner's home", home.display()))?;
    let mut making = name.to_owned();
    making.push(".making");
    let making = home.with_file_name(making);
    let failed = |why: std::io::Error| {
        format!(
            "the runner's home could not be made at {}: {why}",
            home.display()
        )
    };
    match std::fs::remove_dir_all(&making) {
        Ok(()) => {}
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => {}
        Err(why) => return Err(failed(why)),
    }
    if let Some(source) = image_home
        && (source.parent().is_none() || home.starts_with(source))
    {
        return Err(format!(
            "the image's HOME, {}, holds the runner's home {}, so it cannot be copied into it: \
             the runner image contract wants HOME to be a folder of the image user's own \
             (docs/runner-image.md)",
            source.display(),
            home.display()
        ));
    }
    std::fs::write(&marker, format!("{claim}\n")).map_err(failed)?;
    let source = image_home.filter(|source| source.is_dir());
    match source {
        Some(source) => {
            copy_tree(source, &making).map_err(failed)?;
        }
        None => std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&making)
            .map_err(failed)?,
    }
    // A home is this user's alone, whatever mode the image gave its own.
    std::fs::set_permissions(&making, std::fs::Permissions::from_mode(0o700)).map_err(failed)?;
    std::fs::rename(&making, home).map_err(failed)
}

/// The variable a cluster runner's pod's first step reads the uid of its upstream claim from:
/// the pod's own `agents.x-k8s.io/claim-uid` label, by the downward API (#2057). Empty for a
/// runner made warm, before any claim, whose home is never kept.
pub const CLAIM_VAR: &str = "PURLIS_CLAIM_UID";

/// The file beside a cluster runner's home that names the claim it was made for
/// ([`seed_home`]).
pub const CLAIM_MARKER: &str = "claim";

/// `from`, copied to `to` (which is not there yet): folders and files with their modes, links
/// as links. Nothing else (a socket, a device) is copied.
fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    let mode = std::fs::symlink_metadata(from)?.permissions().mode() & 0o7777;
    std::fs::DirBuilder::new().mode(0o700).create(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let (source, target) = (entry.path(), to.join(entry.file_name()));
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            std::os::unix::fs::symlink(std::fs::read_link(&source)?, &target)?;
        } else if kind.is_dir() {
            copy_tree(&source, &target)?;
        } else if kind.is_file() {
            std::fs::copy(&source, &target)?;
        }
    }
    std::fs::set_permissions(to, std::fs::Permissions::from_mode(mode))
}

/// **Pair a cluster runner with the key its pod made as it started** (#2056), read from
/// `handoff` ([`make_at_start`]), and only from a file this user alone may read: as [`pair`]
/// pairs, with that key instead of a new one. Once the pairing is kept, `handoff` is removed,
/// so the key is left only in the session host's own folder. A runner is paired once: with no
/// key at `handoff`, its pairing has taken it already, or the pod made none.
pub fn pair_with_key_made_at_start(
    config_root: &Path,
    desktop: &Pinned,
    handoff: &Path,
) -> Result<Pinned, String> {
    let made = match purlis_same_user::read_private_file(handoff) {
        Ok(text) => Zeroizing::new(text),
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => {
            return Err(format!(
                "this runner's pod holds no link key it made as it started at {}: a runner is \
                 paired once, and its pairing takes that key; claim a new runner",
                handoff.display()
            ));
        }
        Err(why) => {
            return Err(format!(
                "the link key this runner's pod made as it started could not be read safely: {why}"
            ));
        }
    };
    let paired = pair_with(config_root, desktop, || {
        let private = bytes_of(made.trim())
            .map(Zeroizing::new)
            .ok_or_else(|| "the link key this runner's pod made is not one key".to_owned())?;
        LinkKey::from_private(private)
            .map_err(|why| format!("the link key this runner's pod made is not one key: {why}"))
    })?;
    std::fs::remove_file(handoff).map_err(|why| {
        format!("the runner is paired, but the key its pod made could not be taken away: {why}")
    })?;
    Ok(paired)
}

/// [`pair`], with the key `key` gives, asked for once the home is known to hold no pairing.
fn pair_with(
    config_root: &Path,
    desktop: &Pinned,
    key: impl FnOnce() -> Result<LinkKey, String>,
) -> Result<Pinned, String> {
    let folder = &crate::machine::charterd(config_root);
    let held = purlis_same_user::PrivateDirectory::open(folder)
        .map_err(|why| format!("the session host's folder could not be opened: {why}"))?;
    let store = purlis_same_user::PrivateDirectory::open(&crate::machine::dir(config_root))
        .map_err(|why| format!("the machine store could not be opened: {why}"))?;
    let made_already = |what: &str| {
        format!(
            "this runner's home already holds {what}, so it is not paired again: a runner is \
             paired once, from a home of its own; remove the runner and make a new one"
        )
    };
    let device = crate::machine::mint_device_id(config_root)
        .map_err(|why| format!("this runner's device id could not be minted: {why}"))?
        .ok_or_else(|| {
            // A home that holds a pairing was paired before (or is being paired now): say so,
            // rather than that its id came from elsewhere.
            if held.path().join(KEY_FILE).exists() || store.path().join(PEERS_FILE).exists() {
                return made_already("a pairing");
            }
            "this runner's home came with a device id, or one was minted in it before this \
             pairing (by its image's build, by a purlis command run in it first, or by a \
             pairing that did not finish), so it is not paired: a runner mints its device id \
             at its pairing. Remove the runner and make a new one, from an image whose home \
             holds no purlis machine store"
                .to_owned()
        })?;
    let key = key()?;
    let hex = Zeroizing::new(hex_of(made_private(&key)?.as_ref()));
    held.create_file(KEY_FILE, hex.as_bytes())
        .map_err(|why| match why.kind() {
            std::io::ErrorKind::AlreadyExists => made_already("a link key"),
            _ => format!("the runner's link key could not be kept: {why}"),
        })?;
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
        .create_file(PEERS_FILE, format!("{peers:#}\n").as_bytes())
        .map_err(|why| match why.kind() {
            std::io::ErrorKind::AlreadyExists => made_already("a peers file"),
            _ => format!("the desktop this runner is paired with could not be kept: {why}"),
        })?;
    match answering(config_root)? {
        Some((ours, peer))
            if ours.key.public() == key.public() && ours.device == device && &peer == desktop =>
        {
            Ok(Pinned {
                device,
                key: key.public(),
            })
        }
        _ => Err("this runner's pairing did not read back as it was made, so it is refused".into()),
    }
}

/// Who this runner, whose config home is `config_root`, is on a link and the desktop it
/// admits, once it is paired; `None` before. It never mints a device id: the runner's pairing
/// does.
pub fn answering(config_root: &Path) -> Result<Option<(Identity, Pinned)>, String> {
    let Some(desktop) = peer(config_root)? else {
        return Ok(None);
    };
    let device = crate::machine::known_device_id(config_root).ok_or_else(|| {
        "this runner is paired but keeps no device id; remove the runner and add it again"
            .to_owned()
    })?;
    let key = key(&crate::machine::charterd(config_root))?.ok_or_else(|| {
        "this runner is paired but keeps no link key; remove the runner and add it again".to_owned()
    })?;
    Ok(Some((Identity { device, key }, desktop)))
}

/// Take `desktop` out of this runner's peers, as it removes the runner (`unpair`, #2525): its
/// peers file is removed, so no later link is admitted, and the runner is not paired again (its
/// key stays). Nothing is done where no peer is kept; a peer that is another desktop's is
/// refused and kept.
pub fn unpair(config_root: &Path, desktop: &Pinned) -> Result<(), String> {
    match peer(config_root)? {
        None => Ok(()),
        Some(kept) if &kept == desktop => {
            let store = purlis_same_user::PrivateDirectory::open(&crate::machine::dir(config_root))
                .map_err(|why| format!("the machine store could not be opened: {why}"))?;
            match std::fs::remove_file(store.path().join(PEERS_FILE)) {
                Ok(()) => Ok(()),
                Err(why) if why.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(why) => Err(format!(
                    "the desktop could not be taken out of this runner's peers: {why}"
                )),
            }
        }
        Some(_) => Err("this runner is paired with another desktop, which stays its peer".into()),
    }
}

/// A device id and public link key as one line: `{"device":"<id>","key":"<64 hex>"}`.
pub fn to_line(pinned: &Pinned) -> String {
    serde_json::json!({ "device": pinned.device, "key": hex_of(pinned.key.as_bytes()) }).to_string()
}

/// A device id and public link key from one line of [`to_line`]: exactly those two keys, a device id of letters and
/// digits, and a key of 64 lower-case hex whose last bit, which X25519 ignores, is clear.
pub fn from_line(line: &str) -> Result<Pinned, String> {
    let bad = || format!("\"{}\" is not a device id and public link key", line.trim());
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
        // X25519 ignores a public key's last bit, so a key with it set is another spelling of
        // the key without it: a key is read in its one spelling, which every key made has.
        .filter(|key| key[31] & 0x80 == 0)
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

/// Whether `text` is exactly `len` lower-case hex digits: the one check of every key, digest
/// and id purlis spells in hex here and in [`crate::dockerpool`].
pub fn is_lower_hex(text: &str, len: usize) -> bool {
    text.len() == len && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

pub(crate) fn hex_of(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub(crate) fn bytes_of(hex: &str) -> Option<[u8; 32]> {
    if !is_lower_hex(hex, 64) {
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
