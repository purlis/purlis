// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Signed commits, set up** (#5625, spec #5607, ADR 0116): the one module the window and
//! `purlis signing` share for this device's **signing key** and the forge account it is added to.
//!
//! - **The key** is ADR 0115's, kept in the Keyring tier: [`crate::commitsign`] makes, reads and
//!   forgets it.
//! - **The forge account** is reached only with the window's own sign-in
//!   ([`crate::forge::signingkeys`]): listing its signing keys, adding this device's, removing
//!   one, and asking whether a pushed commit shows as Verified. Never through `gh`.
//! - **`purlis signing` holds no sign-in** (ADR 0070 ruling 5, unchanged by the person's ruling
//!   of 2026-11-07 on #5625): it asks the running window over its line ([`crate::windowline`])
//!   with an [`Asked`], and the window answers with what the forge told it, a [`Heard`]
//!   ([`answered_over`]). From the command a key is added by hand, and with no window running the
//!   command gives those steps and says to open the window.

use std::path::Path;

use crate::commitsign::Signer;
use crate::forge::signingkeys::{self, Checked};
use crate::forge::transport::Transport;
use crate::forge::{Caller, Forge, Kind};
use crate::secrets::keyring::Store;

/// What `purlis signing` asks the window, on the person's behalf.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Asked {
    /// Whether the account has the public key `key` (one OpenSSH line) as a signing key: read
    /// from GitHub's public list of the account's signing keys.
    KeyOnAccount { key: String },
    /// Whether commit `commit` of `repo` (`owner/name`) shows as Verified. Asked of GitHub only
    /// on the person's yes in the window, to this repository, this commit and `code`, which the
    /// command's terminal shows too, as the line's other asks are.
    Check {
        repo: String,
        commit: String,
        code: String,
    },
}

/// What the window heard from the forge.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Heard {
    /// Whether the account has the key.
    KeyOnAccount { on: bool },
    /// What the forge says of the commit.
    Checked(Checked),
}

/// The forge a signing ask goes to: GitHub, on github.com. GitLab comes with #5629.
pub fn github() -> Forge {
    Forge::default_of(Kind::GitHub)
}

/// **The window's answer to `asked`** for the account `login`, sent over `transport` (the
/// window's own sign-in, in the app; a recording, in a test). Refused, in a sentence, where the
/// forge refused it or `key` is not one public key. A Check asks only once the person said yes.
pub fn answered_over(
    transport: &dyn Transport,
    forge: &Forge,
    login: &str,
    asked: &Asked,
) -> Result<Heard, String> {
    match asked {
        Asked::KeyOnAccount { key } => {
            if ssh_key::PublicKey::from_openssh(key).is_err() {
                return Err("that is not one public key".to_owned());
            }
            signingkeys::listed_publicly_over(transport, forge, login)
                .map(|listed| Heard::KeyOnAccount {
                    on: listed.has(key).is_some(),
                })
                .map_err(|refused| refused.said(forge.kind))
        }
        Asked::Check { repo, commit, .. } => {
            signingkeys::check_over(transport, forge, repo, commit).map(Heard::Checked)
        }
    }
}

/// **The running window's answer to `asked` from the command line**, with its own sign-in:
/// the window asks as itself, since the command holds none (the ruling of 2026-11-07 on #5625).
/// What comes back is a read: whether the account's public list has a key, or what the forge says
/// of a commit the person said yes to checking; never a token. Where the window has no sign-in on
/// github.com, a sentence that says so.
pub fn answered_in_the_window(asked: &Asked) -> Result<Heard, String> {
    let forge = github();
    let not_signed_in = || {
        format!(
            "this window is not signed in to {} on {} (Settings › You › Accounts)",
            forge.kind.display(),
            forge.host
        )
    };
    let config = crate::machine::config_root()
        .ok_or_else(|| "this window found no config folder".to_owned())?;
    let account = crate::forge::accounts::signed_in_on(&config, forge.kind, &forge.host)?
        .ok_or_else(not_signed_in)?;
    let transport =
        signingkeys::window(&forge, &Caller::window()).map_err(|why| why.to_string())?;
    answered_over(transport.as_ref(), &forge, &account.login, asked)
}

/// How an ask of the window went, from the command's side.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reached {
    /// The window answered with what the forge said.
    Heard(Heard),
    /// No purlis window is running here.
    NoWindow,
    /// The window did not answer in time.
    NoAnswer,
    /// The window answered another question than this one: a window of another build.
    OtherAnswer,
    /// The window answered that it could not ask, in a sentence.
    Refused(String),
}

/// **`asked`, put to the running window by `ask`** ([`crate::windowline::ask`] in the command).
#[cfg(unix)]
pub fn reach(
    asked: Asked,
    ask: impl FnOnce(&crate::windowline::Asked) -> crate::windowline::Answered,
) -> Reached {
    use crate::windowline::{Answered, Said};
    match ask(&crate::windowline::Asked::Signing(asked)) {
        Answered::Said(Said::Signing(heard)) => Reached::Heard(heard),
        Answered::Said(Said::Refused { why }) => Reached::Refused(why),
        Answered::Said(Said::Asked { .. }) => Reached::OtherAnswer,
        Answered::NoAnswer => Reached::NoAnswer,
        Answered::NoWindow => Reached::NoWindow,
    }
}

/// The window's line answer to a signing ask: what [`answered_in_the_window`] says.
#[cfg(unix)]
pub fn said_in_the_window(asked: &Asked) -> crate::windowline::Said {
    match answered_in_the_window(asked) {
        Ok(heard) => crate::windowline::Said::Signing(heard),
        Err(why) => crate::windowline::Said::Refused { why },
    }
}

/// The page where the person adds a signing key to their GitHub account by hand.
pub const NEW_KEY_PAGE: &str = "https://github.com/settings/ssh/new";

/// **This device's key**, or `None` where it has none: signing is on exactly while it has one
/// (ADR 0116 §1).
pub fn key_kept(config_root: &Path, store: &dyn Store) -> Result<Option<Signer>, String> {
    crate::commitsign::kept(config_root, store)
}

/// **Whether this device has a key**, asked of the keychain's attributes and never its value, so
/// asking never makes macOS ask the person: `false` only where the keychain provably has none,
/// `true` where it has one, and the key read where it cannot say.
pub fn key_is_there(config_root: &Path, store: &dyn Store) -> Result<bool, String> {
    let Some(device) = crate::machine::known_device_id(config_root) else {
        return Ok(false);
    };
    match store.exists(
        &format!("{}/{device}", crate::commitsign::SERVICE),
        crate::commitsign::ACCOUNT,
    ) {
        crate::secrets::keyring::Presence::Present => Ok(true),
        crate::secrets::keyring::Presence::Absent => Ok(false),
        crate::secrets::keyring::Presence::Unknown => Ok(key_kept(config_root, store)?.is_some()),
    }
}

/// **This device's key, made where it has none**, and whether it was there before.
pub fn key_made(config_root: &Path, store: &dyn Store) -> Result<(Signer, bool), String> {
    let had = key_kept(config_root, store)?.is_some();
    Ok((crate::commitsign::make(config_root, store)?, had))
}

/// **The person's own key file kept** as this device's key (`purlis signing setup --key`):
/// `openssh` as the file holds it, opened with `passphrase` where it has one.
pub fn key_taken(
    config_root: &Path,
    store: &dyn Store,
    openssh: &str,
    passphrase: Option<&str>,
) -> Result<Signer, String> {
    crate::commitsign::take(config_root, store, openssh, passphrase)
}

/// **This device's key removed from its keychain**, and whether one was there. Signing is off
/// from then on; the forge accounts it was added to keep it until it is removed there too.
pub fn key_removed(config_root: &Path, store: &dyn Store) -> Result<bool, String> {
    crate::commitsign::forget(config_root, store)
}

/// **Who a chat's commit is made as**: the person's own git name and email, which a forge
/// matches against the account's verified addresses.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Who {
    pub name: String,
    pub email: String,
}

/// The person's own git name and email (`git config --global`), or a sentence saying which is
/// not set.
pub fn who_commits() -> Result<Who, String> {
    let read = |key: &str| {
        crate::worktree::git::run(
            Path::new("/"),
            &["config", "--global", "--get", key],
            crate::worktree::git::READ,
        )
        .ok()
        .filter(crate::worktree::git::Run::ok)
        .map(|run| run.line().trim().to_owned())
        .filter(|value| !value.is_empty())
    };
    match (read("user.name"), read("user.email")) {
        (Some(name), Some(email)) => Ok(Who { name, email }),
        _ => Err(
            "your git has no name or email set, so a commit can't say who made it. Set them \
             in Terminal with `git config --global user.name` and `user.email`."
                .to_owned(),
        ),
    }
}

/// The message of the test commit Check signs. Never on a branch: the forge keeps it unreferenced.
pub const TEST_MESSAGE: &str = "purlis checks that this device's signing key is verified\n";

/// **The test commit object Check signs, as git writes it**: `tree`, no parent, `who` as author
/// and committer at `when` (seconds since the epoch, UTC), and [`TEST_MESSAGE`]. What the forge
/// rebuilds from the same fields, byte for byte, and checks the signature over.
pub fn test_commit(tree: &str, who: &Who, when: i64) -> Vec<u8> {
    let line = |role: &str| format!("{role} {} <{}> {when} +0000\n", who.name, who.email);
    format!(
        "tree {tree}\n{}{}\n{TEST_MESSAGE}",
        line("author"),
        line("committer")
    )
    .into_bytes()
}

/// `when` as GitHub reads a commit's date: ISO 8601 in UTC, to the second.
fn iso(when: i64) -> Result<String, String> {
    chrono::DateTime::from_timestamp(when, 0)
        .map(|at| at.format("%Y-%m-%dT%H:%M:%SZ").to_string())
        .ok_or_else(|| "the clock gave no time to date the test with".to_owned())
}

/// **Check, as the forge answers it** (ADR 0116 §6): a test commit signed with `signer` is made
/// in `repo` (`owner/name`), on the tree of its default branch, and never put on a branch; the
/// forge's own verdict on it comes back. Verified means GitHub said so, not that it lists the key.
pub fn check_with_a_test_commit(
    transport: &dyn Transport,
    forge: &Forge,
    signer: &Signer,
    repo: &str,
    who: &Who,
    when: i64,
) -> Result<Checked, String> {
    use crate::forge::transport::{Call, Field, Method};
    let (owner, name) = signingkeys::repo_named(repo)?;
    let shown = forge.kind.display();
    let head = transport
        .send(
            forge,
            &Call::get(
                format!("repos/{owner}/{name}/commits/HEAD"),
                std::time::Duration::from_secs(30),
            ),
        )
        .map_err(|no| format!("{shown} did not answer ({})", no.said()))?;
    if !head.ok() {
        return Err(format!(
            "{shown} could not read {repo} to check with ({})",
            head.said(forge.kind).lines().next().unwrap_or("").trim()
        ));
    }
    let head: serde_json::Value = serde_json::from_str(&head.out)
        .map_err(|_| format!("{shown}'s answer about {repo} did not read"))?;
    let tree = head
        .pointer("/commit/tree/sha")
        .and_then(serde_json::Value::as_str)
        .filter(|sha| sha.len() >= 40 && sha.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or_else(|| format!("{shown} named no tree for {repo}'s default branch"))?;
    let signature = signer.sign(&test_commit(tree, who, when))?;
    let date = iso(when)?;
    let mut fields = vec![
        Field::text("message", TEST_MESSAGE),
        Field::text("tree", tree),
    ];
    for role in ["author", "committer"] {
        fields.push(Field::text(&format!("{role}[name]"), &who.name));
        fields.push(Field::text(&format!("{role}[email]"), &who.email));
        fields.push(Field::text(&format!("{role}[date]"), &date));
    }
    fields.push(Field::text("signature", &signature));
    let made = transport
        .send(
            forge,
            &Call::write(
                Method::Post,
                format!("repos/{owner}/{name}/git/commits"),
                fields,
            ),
        )
        .map_err(|no| format!("{shown} did not answer ({})", no.said()))?;
    if !made.ok() {
        return Err(format!(
            "{shown} did not take the test commit in {repo} ({})",
            made.said(forge.kind).lines().next().unwrap_or("").trim()
        ));
    }
    let made: serde_json::Value = serde_json::from_str(&made.out)
        .map_err(|_| format!("{shown}'s answer about the test commit did not read"))?;
    let said = made
        .get("verification")
        .ok_or_else(|| format!("{shown} named no signature check for the test commit"))?;
    Ok(Checked {
        verified: said
            .get("verified")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        reason: said
            .get("reason")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unknown")
            .to_owned(),
    })
}

/// **A repository to check in, where setup was opened with none**: the account's own most
/// recently pushed, which its sign-in may write to. `None` where it owns none.
pub fn a_repo_to_check_in(
    transport: &dyn Transport,
    forge: &Forge,
) -> Result<Option<String>, String> {
    use crate::forge::transport::{Call, Field};
    let mut call = Call::get("user/repos", std::time::Duration::from_secs(30));
    for (name, value) in [
        ("affiliation", "owner"),
        ("sort", "pushed"),
        ("per_page", "1"),
    ] {
        call.fields.push(Field::typed(name, value));
    }
    let shown = forge.kind.display();
    let reply = transport
        .send(forge, &call)
        .map_err(|no| format!("{shown} did not answer ({})", no.said()))?;
    if !reply.ok() {
        return Err(format!(
            "{shown} did not list your repositories ({})",
            reply.said(forge.kind).lines().next().unwrap_or("").trim()
        ));
    }
    let repos: Vec<serde_json::Value> = serde_json::from_str(&reply.out)
        .map_err(|_| format!("{shown}'s list of your repositories did not read"))?;
    Ok(repos
        .first()
        .and_then(|repo| repo.get("full_name"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned))
}

#[cfg(test)]
mod tests;
