// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **This device's signing key on a forge account** (#5625, spec #5607, ADR 0116 §5 and §6): the
//! account's signing keys listed, this device's added and removed, and a pushed commit's
//! Verified check, each through the forge [`Transport`] with the account's own sign-in, never
//! `gh`.
//!
//! - **Every operation takes its transport** ([`listed_over`], [`add_over`], [`remove_over`],
//!   [`check_over`]), so the forge contract's recordings answer it in a test.
//! - **The window's own sign-in sends it** ([`window`]): only the person in the window holds
//!   one (ADR 0070 ruling 5), so `purlis signing` in their terminal asks the running window
//!   over its line ([`crate::windowline`]) and holds no token itself (the person's ruling of
//!   2026-11-07 on #5625).
//! - **GitHub only, for now**: GitLab's user keys with the usage type *Signing* come with #5629.
//!
//! A key is told apart by its type and data, never its comment: GitHub keeps the title apart.

use std::sync::Arc;
use std::time::Duration;

use super::transport::{Call, Field, Method, Reply, Transport};
use super::{Caller, Failure, Forge, ForgeError, Kind};
use crate::commitsign::{self, Signer};

/// How long one call may take.
const WITHIN: Duration = Duration::from_secs(30);

/// The most keys one page lists, GitHub's own most. A person with more signing keys than this
/// on one account is told of the first hundred.
const PAGE: &str = "100";

/// What every title purlis gives a key starts with, so the card can tell this person's other
/// devices' keys from the keys they added themselves: `purlis · <device>`.
pub const TITLED: &str = "purlis · ";

/// What the title of a key `purlis signing setup` had the person add by hand started with
/// before #5625: `purlis on <machine>`. Still purlis's.
const TITLED_BEFORE: &str = "purlis on ";

/// **The title this device's key is added under**: `purlis · <device>`, the device being this
/// machine's host name ([`crate::dispatch::host`]).
pub fn title() -> String {
    format!("{TITLED}{}", crate::dispatch::host())
}

/// One signing key on the account.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Key {
    /// The forge's id of it, which removes it.
    pub id: u64,
    pub title: String,
    /// Its public line, as the forge lists it.
    pub key: String,
}

impl Key {
    /// Whether it is `signer`'s, by its type and data.
    pub fn is(&self, signer: &Signer) -> bool {
        same_key(&self.key, &signer.public_line())
    }

    /// Whether purlis added it, on some device: its title is [`TITLED`] and a name.
    pub fn purlis_made(&self) -> bool {
        self.title.starts_with(TITLED) || self.title.starts_with(TITLED_BEFORE)
    }
}

/// Whether two public lines are one key: their type and data, without their comments.
pub fn same_key(one: &str, other: &str) -> bool {
    let one: Vec<&str> = one.split_whitespace().take(2).collect();
    let other: Vec<&str> = other.split_whitespace().take(2).collect();
    one.len() == 2 && one == other
}

/// **The account's signing keys**, as the forge listed them.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Listed {
    pub keys: Vec<Key>,
}

impl Listed {
    /// This device's key, `signer`'s, where the account has it.
    pub fn this_device(&self, signer: &Signer) -> Option<&Key> {
        self.has(&signer.public_line())
    }

    /// The key whose public line is `line`, where the account has it.
    pub fn has(&self, line: &str) -> Option<&Key> {
        self.keys.iter().find(|key| same_key(&key.key, line))
    }

    /// The keys purlis added on the person's other devices: purlis's title, and not `signer`'s.
    pub fn other_devices(&self, signer: &Signer) -> Vec<&Key> {
        let line = signer.public_line();
        self.keys
            .iter()
            .filter(|key| key.purlis_made() && !same_key(&key.key, &line))
            .collect()
    }
}

/// Why the forge did not do what it was asked.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Refused {
    /// The account's sign-in may not read or add signing keys: on GitHub it lacks
    /// `admin:ssh_signing_key`, which #5627 asks for. Setup goes on by hand.
    NeedsPermission,
    /// Anything else, in a sentence.
    Said(String),
}

impl Refused {
    /// The refusal in a sentence.
    pub fn said(&self, kind: Kind) -> String {
        match self {
            Refused::NeedsPermission => format!(
                "purlis's sign-in to {} may not add or read your signing keys yet",
                kind.display()
            ),
            Refused::Said(said) => said.clone(),
        }
    }
}

/// A refusal of a call on the account's own keys: a 404 or 403 there is a missing permission,
/// since the account itself is always there to the sign-in that asks.
fn refused_on_keys(reply: &Reply, kind: Kind) -> Refused {
    match reply.failure() {
        Failure::NotFound | Failure::Forbidden => Refused::NeedsPermission,
        _ => Refused::Said(format!(
            "{} refused it ({})",
            kind.display(),
            first_line(&reply.said(kind))
        )),
    }
}

fn first_line(said: &str) -> &str {
    said.lines().next().unwrap_or("").trim()
}

fn github_only(forge: &Forge) -> Result<(), Refused> {
    match forge.kind {
        Kind::GitHub => Ok(()),
        Kind::GitLab => Err(Refused::Said(
            "purlis adds a signing key to a GitLab account from the window only once it \
             supports GitLab's signing keys"
                .to_owned(),
        )),
    }
}

fn unanswered(forge: &Forge, said: &str) -> Refused {
    Refused::Said(format!("{} did not answer ({said})", forge.kind.display()))
}

/// **The account's signing keys**, over `transport`.
pub fn listed_over(transport: &dyn Transport, forge: &Forge) -> Result<Listed, Refused> {
    github_only(forge)?;
    let mut call = Call::get("user/ssh_signing_keys", WITHIN);
    call.fields.push(Field::typed("per_page", PAGE));
    let reply = transport
        .send(forge, &call)
        .map_err(|no| unanswered(forge, no.said()))?;
    if !reply.ok() {
        return Err(refused_on_keys(&reply, forge.kind));
    }
    let keys: Vec<Key> = serde_json::from_str(&reply.out).map_err(|_| {
        Refused::Said(format!(
            "{}'s list of your signing keys did not read as one",
            forge.kind.display()
        ))
    })?;
    Ok(Listed { keys })
}

/// **`signer`'s key added to the account** as a signing key under `title`, over `transport`,
/// and the key as the forge holds it.
pub fn add_over(
    transport: &dyn Transport,
    forge: &Forge,
    signer: &Signer,
    title: &str,
) -> Result<Key, Refused> {
    github_only(forge)?;
    let call = Call::write(
        Method::Post,
        "user/ssh_signing_keys",
        vec![
            Field::text("title", title),
            Field::text("key", &signer.public_line()),
        ],
    );
    let reply = transport
        .send(forge, &call)
        .map_err(|no| unanswered(forge, no.said()))?;
    if !reply.ok() {
        return Err(refused_on_keys(&reply, forge.kind));
    }
    serde_json::from_str(&reply.out).map_err(|_| {
        Refused::Said(format!(
            "{} took the key, and its answer did not read",
            forge.kind.display()
        ))
    })
}

/// **The signing key `id` removed from the account**, over `transport`.
pub fn remove_over(transport: &dyn Transport, forge: &Forge, id: u64) -> Result<(), Refused> {
    github_only(forge)?;
    let call = Call::write(
        Method::Delete,
        format!("user/ssh_signing_keys/{id}"),
        Vec::new(),
    );
    let reply = transport
        .send(forge, &call)
        .map_err(|no| unanswered(forge, no.said()))?;
    if reply.ok() {
        Ok(())
    } else {
        Err(refused_on_keys(&reply, forge.kind))
    }
}

/// **What the forge says of a pushed commit's signature.**
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Checked {
    /// Whether the forge itself shows it as Verified: the only ground for a ✓ (ADR 0116 §6).
    pub verified: bool,
    /// The forge's own reason, such as `valid` or `unverified_email`.
    pub reason: String,
}

impl Checked {
    /// The reason in plain words, with what to do where it is not verified.
    pub fn said(&self) -> String {
        commitsign::reason_said(&self.reason)
    }
}

/// Whether `part` may be an owner's or a repository's name in a path.
fn named(part: &str) -> bool {
    !part.is_empty()
        && part != "."
        && part != ".."
        && part
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// `repo` as `owner/name`, or why it is not one.
pub fn repo_named(repo: &str) -> Result<(&str, &str), String> {
    repo.split_once('/')
        .filter(|(owner, name)| named(owner) && named(name))
        .ok_or_else(|| format!("{repo} is not a repository as `owner/name`."))
}

/// Whether `commit` reads as a commit id, full or short.
pub fn commit_named(commit: &str) -> Result<(), String> {
    if (4..=64).contains(&commit.len()) && commit.bytes().all(|b| b.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(format!("{commit} is not a commit id."))
    }
}

/// **Whether `commit` in `repo` (`owner/name`) shows as Verified**, over `transport`.
pub fn check_over(
    transport: &dyn Transport,
    forge: &Forge,
    repo: &str,
    commit: &str,
) -> Result<Checked, String> {
    github_only(forge).map_err(|why| why.said(forge.kind))?;
    let (owner, name) = repo_named(repo)?;
    commit_named(commit)?;
    let call = Call::get(format!("repos/{owner}/{name}/commits/{commit}"), WITHIN);
    let reply = transport
        .send(forge, &call)
        .map_err(|no| unanswered(forge, no.said()).said(forge.kind))?;
    if !reply.ok() {
        return Err(format!(
            "{} did not find commit {commit} in {repo}: is it pushed? ({})",
            forge.kind.display(),
            first_line(&reply.said(forge.kind))
        ));
    }
    let (verified, reason) = commitsign::verification(&reply.out)?;
    Ok(Checked { verified, reason })
}

/// **The transport the window's own sign-in sends a signing call with**, for the person in the
/// window alone, and never `gh` (ADR 0070 ruling 7): refused where the host handed no sign-in
/// over, the caller is not that person, or no account is signed in on `forge`'s host.
pub fn window(forge: &Forge, caller: &Caller) -> Result<Arc<dyn Transport>, ForgeError> {
    super::signin::signed_in_transport(forge, caller, crate::netlog::Feature::Forge)
}

/// The forge contract's recording `case` of GitHub's signing keys, `signer`'s public line put in
/// for `@KEY@`: a key is made afresh by each test, so a recording names it by that mark.
#[cfg(test)]
pub(crate) fn recording(case: &str, signer: &Signer) -> super::recorded::Recorded {
    let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/forge_contract/signing_keys/github")
        .join(format!("{case}.json"));
    let text = std::fs::read_to_string(&file)
        .unwrap_or_else(|why| panic!("{}: {why}", file.display()))
        .replace("@KEY@", &signer.public_line());
    super::recorded::Recorded::parse(&text).unwrap_or_else(|why| panic!("{case}: {why}"))
}

#[cfg(test)]
mod tests;
