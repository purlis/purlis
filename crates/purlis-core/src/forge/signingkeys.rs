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
//! - **GitHub and GitLab** (#5629): GitHub's signing keys, and GitLab's user keys with the usage
//!   type *Signing* only, which the account's `api` scope already allows. GitLab has no API that
//!   makes a signed commit, so its check reads a signed commit already pushed ([`check_over`]),
//!   and a server older than [`GITLAB_VERIFIES_FROM`] says it can't verify ([`verifies_over`]).
//!
//! A key is told apart by its type and data, never its comment: GitHub keeps the title apart.

use std::sync::Arc;
use std::time::Duration;

use super::transport::{Call, Field, Method, Reply, Transport};
use super::{Caller, Failure, Forge, ForgeError, Kind};
use crate::commitsign::{self, Signer};

/// **The scopes a GitHub sign-in needs for signing setup**, beyond a sign-in's own (ADR 0116 §5):
/// adding, reading and removing this device's signing key, and reading the account's email
/// addresses for the Check step. Asked for at that moment, never at sign-in
/// ([`super::device::start_asking`]).
pub const GITHUB_SCOPES: [&str; 2] = ["admin:ssh_signing_key", "user:email"];

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

/// Where the account's own signing keys are listed and added.
fn own_keys(kind: Kind) -> &'static str {
    match kind {
        Kind::GitHub => "user/ssh_signing_keys",
        Kind::GitLab => "user/keys",
    }
}

/// **GitLab's usage type for a key that only signs**: never `auth_and_signing`, GitLab's default
/// where none is sent, so the key cannot log in over SSH (ADR 0116 §5).
pub const GITLAB_SIGNING_ONLY: &str = "signing";

/// The oldest GitLab release that verifies SSH-signed commits: 15.7. Not confirmed by GitLab's
/// current docs (#5629), so a server older than this, or one whose version does not read, is
/// one that can't verify.
pub const GITLAB_VERIFIES_FROM: (u32, u32) = (15, 7);

/// The keys `out` lists, as the forge spells them: on GitLab only those that may sign.
fn keys_listed(kind: Kind, out: &str) -> Option<Vec<Key>> {
    let listed: Vec<serde_json::Value> = serde_json::from_str(out).ok()?;
    Some(
        listed
            .iter()
            .filter(|key| match kind {
                Kind::GitHub => true,
                Kind::GitLab => key
                    .get("usage_type")
                    .and_then(serde_json::Value::as_str)
                    .is_none_or(|usage| usage == "signing" || usage == "auth_and_signing"),
            })
            .filter_map(|key| {
                Some(Key {
                    id: key.get("id")?.as_u64()?,
                    title: key
                        .get("title")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("")
                        .to_owned(),
                    key: key.get("key")?.as_str()?.to_owned(),
                })
            })
            .collect(),
    )
}

fn unanswered(forge: &Forge, said: &str) -> Refused {
    Refused::Said(format!("{} did not answer ({said})", forge.kind.display()))
}

/// **The account's signing keys**, over `transport`.
pub fn listed_over(transport: &dyn Transport, forge: &Forge) -> Result<Listed, Refused> {
    let mut call = Call::get(own_keys(forge.kind), WITHIN);
    call.fields.push(Field::typed("per_page", PAGE));
    let reply = transport
        .send(forge, &call)
        .map_err(|no| unanswered(forge, no.said()))?;
    if !reply.ok() {
        return Err(refused_on_keys(&reply, forge.kind));
    }
    let keys = keys_listed(forge.kind, &reply.out).ok_or_else(|| {
        Refused::Said(format!(
            "{}'s list of your signing keys did not read as one",
            forge.kind.display()
        ))
    })?;
    Ok(Listed { keys })
}

/// **`login`'s signing keys as anyone may read them** (GitHub's public list, which needs no
/// scope): how setup notices a key the person added by hand, where the sign-in may not read the
/// account's own list.
pub fn listed_publicly_over(
    transport: &dyn Transport,
    forge: &Forge,
    login: &str,
) -> Result<Listed, Refused> {
    if !named(login) {
        return Err(Refused::Said(format!("{login} is not an account's name")));
    }
    let path = match forge.kind {
        Kind::GitHub => format!("users/{login}/ssh_signing_keys"),
        Kind::GitLab => format!("users/{login}/keys"),
    };
    let mut call = Call::get(path, WITHIN);
    call.fields.push(Field::typed("per_page", PAGE));
    let reply = transport
        .send(forge, &call)
        .map_err(|no| unanswered(forge, no.said()))?;
    if !reply.ok() {
        return Err(Refused::Said(format!(
            "{} did not list {login}'s signing keys ({})",
            forge.kind.display(),
            first_line(&reply.said(forge.kind))
        )));
    }
    // GitHub's public list names each key and its id, and no title.
    let keys = keys_listed(forge.kind, &reply.out).ok_or_else(|| {
        Refused::Said(format!(
            "{}'s list of {login}'s signing keys did not read as one",
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
    let mut fields = vec![
        Field::text("title", title),
        Field::text("key", &signer.public_line()),
    ];
    if forge.kind == Kind::GitLab {
        fields.push(Field::text("usage_type", GITLAB_SIGNING_ONLY));
    }
    let call = Call::write(Method::Post, own_keys(forge.kind), fields);
    let reply = transport
        .send(forge, &call)
        .map_err(|no| unanswered(forge, no.said()))?;
    if !reply.ok() {
        // GitLab answers a key it has already with 400 "has already been taken": the one it has.
        if forge.kind == Kind::GitLab && reply.said(forge.kind).contains("has already been taken") {
            return listed_over(transport, forge)?
                .this_device(signer)
                .cloned()
                .ok_or_else(|| {
                    Refused::Said(
                        "GitLab has this key on another account already, so it can't be added \
                         to this one"
                            .to_owned(),
                    )
                });
        }
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
    let call = Call::write(
        Method::Delete,
        format!("{}/{id}", own_keys(forge.kind)),
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
    /// The public line of the key the forge says signed it, where it names one (GitLab does).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
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

/// `repo` as a GitLab project's full path (`group/sub/name`), or why it is not one.
pub fn project_named(repo: &str) -> Result<(), String> {
    let parts: Vec<&str> = repo.split('/').collect();
    if parts.len() >= 2 && parts.iter().all(|part| named(part)) {
        Ok(())
    } else {
        Err(format!("{repo} is not a project as `group/name`."))
    }
}

/// **Whether `commit` in `repo` shows as Verified**, over `transport`: `owner/name` on GitHub, a
/// project's full path on GitLab.
pub fn check_over(
    transport: &dyn Transport,
    forge: &Forge,
    repo: &str,
    commit: &str,
) -> Result<Checked, String> {
    commit_named(commit)?;
    let path = match forge.kind {
        Kind::GitHub => {
            let (owner, name) = repo_named(repo)?;
            format!("repos/{owner}/{name}/commits/{commit}")
        }
        Kind::GitLab => {
            project_named(repo)?;
            format!(
                "projects/{}/repository/commits/{commit}/signature",
                super::quote(repo)
            )
        }
    };
    let reply = transport
        .send(forge, &Call::get(path, WITHIN))
        .map_err(|no| unanswered(forge, no.said()).said(forge.kind))?;
    if !reply.ok() {
        return Err(match forge.kind {
            Kind::GitLab if reply.failure() == Failure::NotFound => format!(
                "GitLab has no signature for commit {commit} in {repo}: it was pushed unsigned, \
                 or not pushed"
            ),
            _ => format!(
                "{} did not find commit {commit} in {repo}: is it pushed? ({})",
                forge.kind.display(),
                first_line(&reply.said(forge.kind))
            ),
        });
    }
    let (verified, reason, key) = match forge.kind {
        Kind::GitHub => {
            let (verified, reason) = commitsign::verification(&reply.out)?;
            (verified, reason, None)
        }
        Kind::GitLab => gitlab_verification(&reply.out)?,
    };
    Ok(Checked {
        verified,
        reason,
        key,
    })
}

/// **What GitLab says of a commit's signature**, from `GET
/// projects/:id/repository/commits/:sha/signature`, in GitHub's reasons so the window words
/// both alike: `verified` is `valid`, an email that is not the account's verified one is
/// `unverified_email`, a key GitLab does not know is `unknown_key`, and any other status is
/// GitLab's own word. With the public line of the key GitLab says signed it, where it names one.
pub fn gitlab_verification(answer: &str) -> Result<(bool, String, Option<String>), String> {
    let said: serde_json::Value = serde_json::from_str(answer)
        .map_err(|_| "GitLab's answer about the commit did not read".to_owned())?;
    let status = said
        .get("verification_status")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "GitLab's answer names no signature check for the commit".to_owned())?;
    let key = said
        .pointer("/key/key")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned);
    let (verified, reason) = match status {
        "verified" | "verified_system" | "verified_ca" => (true, "valid".to_owned()),
        "same_user_different_email" | "unverified_email" => (false, "unverified_email".to_owned()),
        "unknown_key" | "unverified_key" => (false, "unknown_key".to_owned()),
        other => (false, other.to_owned()),
    };
    Ok((verified, reason, key))
}

/// **Whether the forge verifies SSH-signed commits**, over `transport`: GitHub does; a GitLab
/// does from [`GITLAB_VERIFIES_FROM`], read from its `GET version`. A version that does not
/// read is one that can't verify. `Err` where the forge did not answer.
pub fn verifies_over(transport: &dyn Transport, forge: &Forge) -> Result<bool, String> {
    if forge.kind == Kind::GitHub {
        return Ok(true);
    }
    let reply = transport
        .send(forge, &Call::get("version", WITHIN))
        .map_err(|no| unanswered(forge, no.said()).said(forge.kind))?;
    if !reply.ok() {
        return Err(format!(
            "GitLab on {} did not say its version ({})",
            forge.host,
            first_line(&reply.said(forge.kind))
        ));
    }
    let said: serde_json::Value = serde_json::from_str(&reply.out).unwrap_or_default();
    let mut numbers = said
        .get("version")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .split(|c: char| !c.is_ascii_digit())
        .map(str::parse::<u32>);
    let release = match (numbers.next(), numbers.next()) {
        (Some(Ok(major)), Some(Ok(minor))) => (major, minor),
        _ => return Ok(false),
    };
    Ok(release >= GITLAB_VERIFIES_FROM)
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
    recording_with(case, &[("@KEY@", &signer.public_line())])
}

/// The recording `case`, each mark in `marks` replaced by its value, written as a JSON string's
/// inside is: a signature runs over several lines.
#[cfg(test)]
pub(crate) fn recording_with(case: &str, marks: &[(&str, &str)]) -> super::recorded::Recorded {
    recording_of("github", case, marks)
}

/// The recording `case` of `forge`'s signing keys (`github`, `gitlab`), each mark in `marks`
/// replaced by its value.
#[cfg(test)]
pub(crate) fn recording_of(
    forge: &str,
    case: &str,
    marks: &[(&str, &str)],
) -> super::recorded::Recorded {
    let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/forge_contract/signing_keys")
        .join(forge)
        .join(format!("{case}.json"));
    let mut text =
        std::fs::read_to_string(&file).unwrap_or_else(|why| panic!("{}: {why}", file.display()));
    for (mark, value) in marks {
        let quoted = serde_json::to_string(value).unwrap();
        text = text.replace(mark, &quoted[1..quoted.len() - 1]);
    }
    super::recorded::Recorded::parse(&text).unwrap_or_else(|why| panic!("{case}: {why}"))
}

#[cfg(test)]
mod tests;
