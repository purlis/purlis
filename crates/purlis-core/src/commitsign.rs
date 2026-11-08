// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The key the app signs a chat's commits with** (#5480, the person's ruling of 2026-11-06):
//! a repository that requires signed commits (`main` of purlis/purlis does) refuses a pull request
//! whose commits are unsigned, and a sandboxed chat's commits are made by the app.
//!
//! - **One Ed25519 key per device, in the Keyring tier**: one item, `purlis/@commit-signing/<device>`
//!   ([`SERVICE`]), holding the private key as OpenSSH writes one, with no passphrase, since
//!   nobody is there to type one when a chat commits. purlis makes it ([`make`]) or takes one of
//!   the person's own ([`take`]), from `purlis signing setup` in their terminal.
//! - **git's own SSH signature, made in this process** ([`Signer::sign`]): an `SSHSIG` in the
//!   namespace `git` over the commit object, as `git commit -S` with `gpg.format = ssh` makes
//!   one, put in the commit's `gpgsig` header ([`signed_copy`]). So no signer program runs (the
//!   runner turns every one off, `docs/git-policy.md`), no key file is ever written, and GitHub,
//!   GitLab and `git verify-commit` read it as they read any SSH-signed commit.
//! - **Signed after git makes the commit**: git commits as it always has, unsigned, and the app
//!   writes a signed copy of each commit it made (the same tree, parents, author, committer and
//!   message) and moves the branch to it only from where git left it ([`sign_made`]).
//! - **Chats never reach it**: the item is written by purlis's own binary and held to the app,
//!   as the link key is ([`crate::linkkey`]), and only the app's brokered git is handed a
//!   [`Signer`]. Nothing of it is in a chat's environment, a file, or an answer: an answer names
//!   the key's fingerprint at most.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use ssh_key::private::{Ed25519Keypair, KeypairData};
use ssh_key::{Algorithm, HashAlg, LineEnding, PrivateKey};
use zeroize::Zeroizing;

use crate::secrets::keyring;
use crate::worktree::git;

/// The keyring's service for this device's commit-signing key: `purlis/@commit-signing/<device>`.
pub const SERVICE: &str = "purlis/@commit-signing";

/// The keyring's account the key is kept under.
pub const ACCOUNT: &str = "key";

/// The namespace git signs a commit in (`ssh-keygen -Y sign -n git`).
pub const NAMESPACE: &str = "git";

/// The comment on a key purlis makes, which the person sees on GitHub's list of their keys.
pub const COMMENT: &str = "purlis signs commits made for chats";

/// The lock a key is made under, beside the machine store.
const LOCK: &str = "commit-signing.lock";

/// What the command that sets signing up is, said wherever signing is missing.
pub const SETUP: &str = "purlis signing setup";

/// **The key a commit is signed with**, read from the keyring. Shown as its fingerprint alone:
/// its `Debug` names no part of the private key.
#[derive(Clone)]
pub struct Signer {
    key: Arc<PrivateKey>,
}

impl std::fmt::Debug for Signer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Signer({})", self.fingerprint())
    }
}

impl PartialEq for Signer {
    fn eq(&self, other: &Self) -> bool {
        self.key.public_key().key_data() == other.key.public_key().key_data()
    }
}

impl Eq for Signer {}

impl Signer {
    /// A signer of `key`, which must be an Ed25519 key with no passphrase.
    fn of(key: PrivateKey) -> Result<Self, String> {
        if key.is_encrypted() {
            return Err(
                "the key has a passphrase, and purlis signs when nobody is there to \
                 type one"
                    .to_owned(),
            );
        }
        if key.algorithm() != Algorithm::Ed25519 {
            return Err(format!(
                "the key is {}, and purlis signs with an Ed25519 key only",
                key.algorithm()
            ));
        }
        Ok(Self { key: Arc::new(key) })
    }

    /// A new key from the operating system's generator. For a test, and for [`make`].
    pub fn generate() -> Result<Self, String> {
        let mut seed = Zeroizing::new([0u8; 32]);
        getrandom::fill(seed.as_mut())
            .map_err(|why| format!("the system gave no random bytes for a key ({why})"))?;
        let pair = Ed25519Keypair::from_seed(&seed);
        let key = PrivateKey::new(KeypairData::Ed25519(pair), COMMENT)
            .map_err(|why| format!("purlis could not make a key ({why})"))?;
        Self::of(key)
    }

    /// The public half as one line of `authorized_keys` or GitHub's form: `ssh-ed25519 AAAA… <comment>`.
    pub fn public_line(&self) -> String {
        self.key
            .public_key()
            .to_openssh()
            .unwrap_or_else(|_| String::new())
    }

    /// The key's SHA-256 fingerprint, as `ssh-keygen -l` and GitHub show it: `SHA256:…`.
    pub fn fingerprint(&self) -> String {
        self.key.fingerprint(HashAlg::Sha256).to_string()
    }

    /// `payload` signed as git signs a commit: an armored `SSHSIG` in the [`NAMESPACE`] `git`,
    /// over SHA-512, as `ssh-keygen -Y sign -n git` writes it.
    pub fn sign(&self, payload: &[u8]) -> Result<String, String> {
        let signed = self
            .key
            .sign(NAMESPACE, HashAlg::Sha512, payload)
            .map_err(|why| format!("the commit could not be signed ({why})"))?;
        signed
            .to_pem(LineEnding::LF)
            .map_err(|why| format!("the signature could not be written ({why})"))
    }

    /// The private key as OpenSSH writes one, for the keyring.
    fn kept_form(&self) -> Result<Zeroizing<String>, String> {
        self.key
            .to_openssh(LineEnding::LF)
            .map_err(|why| format!("the key could not be written out ({why})"))
    }
}

/// The keyring this build keeps the key in: the operating system's, or, in a fenced build, a
/// stub file in the machine store, so a test never reaches the person's keyring.
pub fn this_builds_keyring(config_root: &Path) -> Box<dyn keyring::Store> {
    crate::linkkey::this_builds_keyring(config_root)
}

/// The keyring service of this device's key, or why there is none.
fn service(config_root: &Path) -> Result<String, String> {
    let device = crate::machine::device_id(config_root)
        .map_err(|why| format!("this device has no id to keep a signing key under: {why}"))?;
    Ok(format!("{SERVICE}/{device}"))
}

/// **The key this device signs chats' commits with**, or `None` where none is set up. Read and
/// never made. A device with no id yet has none; an item that is there and does not read as one
/// Ed25519 key with no passphrase is refused with a sentence.
pub fn kept(config_root: &Path, store: &dyn keyring::Store) -> Result<Option<Signer>, String> {
    let Some(device) = crate::machine::known_device_id(config_root) else {
        return Ok(None);
    };
    read(store, &format!("{SERVICE}/{device}"))
}

/// The key in `service`, or `None` when the keyring provably holds none.
fn read(store: &dyn keyring::Store, service: &str) -> Result<Option<Signer>, String> {
    let Some(held) = store
        .get(service, ACCOUNT)
        .map_err(|why| format!("the signing key could not be read from your keychain ({why})"))?
    else {
        return Ok(None);
    };
    let text = Zeroizing::new(held.into_inner());
    let key = PrivateKey::from_openssh(text.as_bytes()).map_err(|_| {
        format!("the signing key in your keychain does not read as a key: run `{SETUP}` again")
    })?;
    Signer::of(key)
        .map(Some)
        .map_err(|why| format!("the signing key in your keychain cannot be used: {why}"))
}

/// **A new key made and kept**, where this device keeps none. One that is kept already is
/// answered as it is, so two setups at once make one key between them.
pub fn make(config_root: &Path, store: &dyn keyring::Store) -> Result<Signer, String> {
    let service = service(config_root)?;
    let _held = crate::machine::Lock::named(config_root, LOCK);
    if let Some(kept) = read(store, &service)? {
        return Ok(kept);
    }
    keep(store, &service, &Signer::generate()?)
}

/// **The person's own key kept** in place of any this device keeps: `openssh` as the file
/// holds it, opened with `passphrase` where it has one. An Ed25519 key only.
pub fn take(
    config_root: &Path,
    store: &dyn keyring::Store,
    openssh: &str,
    passphrase: Option<&str>,
) -> Result<Signer, String> {
    let key = PrivateKey::from_openssh(openssh.as_bytes()).map_err(|_| {
        "that file is not an OpenSSH private key, such as `~/.ssh/id_ed25519` (not the `.pub` \
         file beside it)"
            .to_owned()
    })?;
    let key = match (key.is_encrypted(), passphrase) {
        (true, Some(passphrase)) => key
            .decrypt(passphrase)
            .map_err(|_| "the passphrase does not open that key".to_owned())?,
        (true, None) => {
            return Err("that key has a passphrase, and none was given".to_owned());
        }
        (false, _) => key,
    };
    let signer = Signer::of(key)?;
    let service = service(config_root)?;
    let _held = crate::machine::Lock::named(config_root, LOCK);
    keep(store, &service, &signer)
}

/// Whether the OpenSSH private key `openssh` is locked with a passphrase. `false` for text that
/// is no key, which [`take`] then refuses with its sentence.
pub fn take_needs_passphrase(openssh: &str) -> bool {
    PrivateKey::from_openssh(openssh.as_bytes()).is_ok_and(|key| key.is_encrypted())
}

/// Whether a key was there to forget.
pub fn forget(config_root: &Path, store: &dyn keyring::Store) -> Result<bool, String> {
    let service = service(config_root)?;
    store
        .delete(&service, ACCOUNT)
        .map_err(|why| format!("the signing key could not be removed from your keychain ({why})"))
}

/// `signer` kept in `service`, and read back, so the key a commit is signed with is the one
/// the person added to their forge.
fn keep(store: &dyn keyring::Store, service: &str, signer: &Signer) -> Result<Signer, String> {
    let text = signer.kept_form()?;
    store
        .set(service, ACCOUNT, &text)
        .map_err(|why| format!("the signing key could not be kept in your keychain ({why})"))?;
    read(store, service)?
        .filter(|kept| kept == signer)
        .ok_or_else(|| "the signing key did not read back from your keychain as it was kept".into())
}

/// The headers a signature is kept under: git's for each object format.
const SIGNATURE_HEADERS: [&str; 2] = ["gpgsig", "gpgsig-sha256"];

/// **A signed copy of commit `id`** written in the repository at `dir`, and its id: the same
/// object with each parent replaced as `parents` maps it, any signature it had taken off, and
/// `signer`'s signature put in. The commit `id` names is left as it is.
pub fn signed_copy(
    dir: &Path,
    id: &str,
    parents: &HashMap<String, String>,
    signer: &Signer,
) -> Result<String, String> {
    let read = git::run_with_input(dir, &["cat-file", "commit", id], Vec::new(), git::READ)
        .map_err(|why| why.to_string())?;
    if read.code != Some(0) {
        return Err(format!("git could not read commit {id} to sign it"));
    }
    let header = signature_header(dir)?;
    let payload = unsigned(&read.out, parents)
        .ok_or_else(|| format!("commit {id} does not read as a commit, so it was not signed"))?;
    let signature = signer.sign(&payload)?;
    let object = with_signature(&payload, header, &signature);
    let wrote = git::run_with_input(
        dir,
        &["hash-object", "-t", "commit", "-w", "--stdin"],
        object,
        git::READ,
    )
    .map_err(|why| why.to_string())?;
    let new = String::from_utf8_lossy(&wrote.out).trim().to_owned();
    if wrote.code != Some(0) || !is_id(&new) {
        return Err(format!(
            "git did not write the signed copy of commit {id}: {}",
            String::from_utf8_lossy(&wrote.err)
                .lines()
                .next()
                .unwrap_or("")
        ));
    }
    Ok(new)
}

/// **Each commit made since `before`, signed**, oldest first, in the repository at `dir`: every
/// commit `head` reaches and none of `before` reaches, each written again as [`signed_copy`]
/// with its parents the signed copies, and the id that stands for `head` now. `head` itself
/// where there is none. Nothing is moved: the caller moves its branch from `head` to the answer.
pub fn sign_made(
    dir: &Path,
    head: &str,
    before: &[&str],
    signer: &Signer,
) -> Result<String, String> {
    let mut args = vec!["rev-list", "--reverse", "--topo-order", head, "--not"];
    args.extend(before.iter().copied());
    let listed = git::run(dir, &args, git::READ).map_err(|why| why.to_string())?;
    if !listed.ok() {
        return Err(format!(
            "git could not list the commits to sign: {}",
            listed.err.lines().next().unwrap_or("")
        ));
    }
    let mut copies: HashMap<String, String> = HashMap::new();
    let mut last = head.to_owned();
    for id in listed
        .out
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let new = signed_copy(dir, id, &copies, signer)?;
        copies.insert(id.to_owned(), new.clone());
        last = new;
    }
    Ok(copies.get(head).cloned().unwrap_or(last))
}

/// **Commit `id`, at the tip of `branch_ref`, signed**, and the branch moved to the signed copy
/// from `id` alone. Its parents are as git made them. The signed copy's id.
pub fn sign_one(dir: &Path, branch_ref: &str, id: &str, signer: &Signer) -> Result<String, String> {
    let signed = signed_copy(dir, id, &HashMap::new(), signer)?;
    moved(dir, branch_ref, &signed, id)?;
    Ok(signed)
}

/// **`signer`'s signed copy of what was made on `branch` since `before`**, and `branch` moved
/// to it from `head` alone, where git left it: a branch moved since is left where it is, and
/// said. The new head, which is `head` where nothing was made.
pub fn sign_branch(
    dir: &Path,
    branch_ref: &str,
    head: &str,
    before: &[&str],
    signer: &Signer,
) -> Result<String, String> {
    let signed = sign_made(dir, head, before, signer)?;
    if signed != head {
        moved(dir, branch_ref, &signed, head)?;
    }
    Ok(signed)
}

/// `branch_ref` moved to `signed` from `was` alone, or why it was left.
pub(crate) fn moved(dir: &Path, branch_ref: &str, signed: &str, was: &str) -> Result<(), String> {
    let moved = git::run(
        dir,
        &[
            "update-ref",
            "-m",
            "purlis: signed for a chat",
            branch_ref,
            signed,
            was,
        ],
        git::READ,
    )
    .map_err(|why| why.to_string())?;
    if !moved.ok() {
        return Err(format!(
            "the branch moved while purlis signed its commits, so it was left as it is ({})",
            moved.err.lines().next().unwrap_or("").trim_end_matches('.')
        ));
    }
    Ok(())
}

/// The header git keeps a signature under in the repository at `dir`: `gpgsig` for SHA-1,
/// `gpgsig-sha256` for SHA-256.
fn signature_header(dir: &Path) -> Result<&'static str, String> {
    let format = git::run(dir, &["rev-parse", "--show-object-format"], git::READ)
        .map_err(|why| why.to_string())?;
    match format.line().trim() {
        "sha1" | "" => Ok(SIGNATURE_HEADERS[0]),
        "sha256" => Ok(SIGNATURE_HEADERS[1]),
        other => Err(format!(
            "this repository's objects are named by {other}, which purlis does not sign"
        )),
    }
}

/// **Whether commit `id` in the repository at `dir` carries a signature**: a signature header
/// before the end of its headers. `false` where git cannot read it.
pub fn is_signed(dir: &Path, id: &str) -> bool {
    let Ok(read) = git::run_with_input(dir, &["cat-file", "commit", id], Vec::new(), git::READ)
    else {
        return false;
    };
    if read.code != Some(0) {
        return false;
    }
    let end = read
        .out
        .windows(2)
        .position(|pair| pair == b"\n\n")
        .unwrap_or(read.out.len());
    read.out[..end].split(|byte| *byte == b'\n').any(|line| {
        SIGNATURE_HEADERS
            .iter()
            .any(|header| line.starts_with(format!("{header} ").as_bytes()))
    })
}

/// Whether `id` is a full object id.
fn is_id(id: &str) -> bool {
    matches!(id.len(), 40 | 64) && id.bytes().all(|b| b.is_ascii_hexdigit())
}

/// **The commit object `raw` as it is signed**: its headers with every signature header (and
/// the lines continuing it) taken off and each `parent` replaced as `parents` maps it, then the
/// blank line and the message, byte for byte. What git signs, and what a verifier reads back
/// once it takes the signature header off. `None` where `raw` has no `tree` header first or no
/// end to its headers.
pub fn unsigned(raw: &[u8], parents: &HashMap<String, String>) -> Option<Vec<u8>> {
    let end = raw.windows(2).position(|pair| pair == b"\n\n")?;
    let (headers, rest) = (&raw[..=end], &raw[end + 1..]);
    if !headers.starts_with(b"tree ") {
        return None;
    }
    let mut out = Vec::with_capacity(raw.len());
    let mut dropping = false;
    for line in headers.split_inclusive(|byte| *byte == b'\n') {
        if line.starts_with(b" ") {
            if !dropping {
                out.extend_from_slice(line);
            }
            continue;
        }
        let name = line.split(|byte| *byte == b' ').next().unwrap_or_default();
        dropping = SIGNATURE_HEADERS
            .iter()
            .any(|header| header.as_bytes() == name);
        if dropping {
            continue;
        }
        if name == b"parent" {
            let id = String::from_utf8_lossy(&line[b"parent ".len()..])
                .trim_end()
                .to_owned();
            match parents.get(&id) {
                Some(new) => out.extend_from_slice(format!("parent {new}\n").as_bytes()),
                None => out.extend_from_slice(line),
            }
            continue;
        }
        out.extend_from_slice(line);
    }
    out.extend_from_slice(rest);
    Some(out)
}

/// `payload` ([`unsigned`]) with `signature` put in as the last header, `header`, each line
/// after its first begun with a space, as git writes it.
fn with_signature(payload: &[u8], header: &str, signature: &str) -> Vec<u8> {
    let end = payload
        .windows(2)
        .position(|pair| pair == b"\n\n")
        .map_or(payload.len(), |at| at + 1);
    let mut out = Vec::with_capacity(payload.len() + signature.len() + 64);
    out.extend_from_slice(&payload[..end]);
    for (at, line) in signature.trim_end().lines().enumerate() {
        if at == 0 {
            out.extend_from_slice(header.as_bytes());
        }
        out.push(b' ');
        out.extend_from_slice(line.as_bytes());
        out.push(b'\n');
    }
    out.extend_from_slice(&payload[end..]);
    out
}

/// **What GitHub says of a commit's signature**, from its answer to `GET
/// /repos/<owner>/<repo>/commits/<commit>`: whether it is verified, and GitHub's reason.
pub fn verification(answer: &str) -> Result<(bool, String), String> {
    let commit: serde_json::Value = serde_json::from_str(answer)
        .map_err(|_| "GitHub's answer about the commit did not read".to_owned())?;
    let said = commit
        .pointer("/commit/verification")
        .ok_or_else(|| "GitHub's answer names no signature check for the commit".to_owned())?;
    let verified = said
        .get("verified")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let reason = said
        .get("reason")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown")
        .to_owned();
    Ok((verified, reason))
}

/// **GitHub's reason for a commit's check, in plain words**, with what to do where it is not
/// verified. GitHub's own list: <https://docs.github.com/en/rest/commits/commits>.
pub fn reason_said(reason: &str) -> String {
    match reason {
        "valid" => "GitHub shows it as Verified".to_owned(),
        "unsigned" => "the commit is not signed. It was made before signing was set up, or \
             outside a chat. Commits a chat makes from now on are signed"
            .to_owned(),
        "unknown_key" | "not_signing_key" => format!(
            "GitHub does not know the key it is signed with as a signing key on your account. \
             Run `{SETUP}` again and add the key it shows as a Signing Key"
        ),
        "no_user" | "unverified_email" | "bad_email" => "the email address on the commit is \
             not a verified address of your GitHub account. Add and verify it in GitHub › \
             Settings › Emails, or set `git config --global user.email` to one that is, then \
             commit again"
            .to_owned(),
        "expired_key" => format!("the key has expired on GitHub. Run `{SETUP}` again"),
        other => format!(
            "GitHub says `{other}`. Run `{SETUP}` again, and if it still says so, remove the \
             key with `purlis signing remove` and set it up afresh"
        ),
    }
}

#[cfg(test)]
mod tests;
