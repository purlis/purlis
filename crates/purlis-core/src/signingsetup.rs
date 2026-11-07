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
use crate::forge::signingkeys::{self, Checked, Refused};
use crate::forge::transport::Transport;
use crate::forge::{Caller, Forge, Kind};
use crate::secrets::keyring::Store;

/// What `purlis signing` asks the window, on the person's behalf.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Asked {
    /// Whether the account has the public key `key` (one OpenSSH line) as a signing key.
    KeyOnAccount { key: String },
    /// Whether commit `commit` of `repo` (`owner/name`) shows as Verified.
    Check { repo: String, commit: String },
}

/// What the window heard from the forge.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Heard {
    /// Whether the account has the key.
    KeyOnAccount { on: bool },
    /// The window's sign-in may not read signing keys: the person adds the key by hand, and
    /// the window's own setup asks for the permission (#5627).
    NeedsPermission,
    /// What the forge says of the commit.
    Checked(Checked),
}

/// The forge a signing ask goes to: GitHub, on github.com. GitLab comes with #5629.
pub fn github() -> Forge {
    Forge::default_of(Kind::GitHub)
}

/// **The window's answer to `asked`**, sent over `transport` (the window's own sign-in, in the
/// app; a recording, in a test). Refused, in a sentence, where the forge refused it or `key`
/// is not one public key.
pub fn answered_over(
    transport: &dyn Transport,
    forge: &Forge,
    asked: &Asked,
) -> Result<Heard, String> {
    match asked {
        Asked::KeyOnAccount { key } => {
            if ssh_key::PublicKey::from_openssh(key).is_err() {
                return Err("that is not one public key".to_owned());
            }
            match signingkeys::listed_over(transport, forge) {
                Ok(listed) => Ok(Heard::KeyOnAccount {
                    on: listed.has(key).is_some(),
                }),
                Err(Refused::NeedsPermission) => Ok(Heard::NeedsPermission),
                Err(refused) => Err(refused.said(forge.kind)),
            }
        }
        Asked::Check { repo, commit } => {
            signingkeys::check_over(transport, forge, repo, commit).map(Heard::Checked)
        }
    }
}

/// **The running window's answer to `asked` from the command line**, with its own sign-in:
/// the window asks as itself, the person in it, since the command holds none (the ruling of
/// 2026-11-07 on #5625). What comes back is a read: whether the account has a public key, or
/// what the forge says of a commit; never a token. Where the window has no sign-in on
/// github.com, a sentence that says so.
pub fn answered_in_the_window(asked: &Asked) -> Result<Heard, String> {
    let forge = github();
    let transport = signingkeys::window(&forge, &Caller::window()).map_err(|_| {
        format!(
            "this window is not signed in to {} on {} (Settings › You › Accounts)",
            forge.kind.display(),
            forge.host
        )
    })?;
    answered_over(transport.as_ref(), &forge, asked)
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

#[cfg(test)]
mod tests;
