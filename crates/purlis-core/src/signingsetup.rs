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
    /// Whether the account has the key, and how many of the person's other devices' purlis
    /// keys it has beside.
    KeyOnAccount { on: bool, other_devices: usize },
    /// The window's sign-in may not read signing keys: the person adds the key by hand, and
    /// the window's own setup asks for the permission (#5627).
    NeedsPermission,
    /// What the forge says of the commit.
    Checked(Checked),
}

/// The forge a signing ask goes to: GitHub, on github.com. GitLab comes with #5629.
pub fn forge() -> Forge {
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
                Ok(listed) => {
                    let ours = |line: &str| signingkeys::same_key(line, key);
                    Ok(Heard::KeyOnAccount {
                        on: listed.keys.iter().any(|one| ours(&one.key)),
                        other_devices: listed
                            .keys
                            .iter()
                            .filter(|one| one.purlis_made() && !ours(&one.key))
                            .count(),
                    })
                }
                Err(Refused::NeedsPermission) => Ok(Heard::NeedsPermission),
                Err(refused) => Err(refused.said(forge.kind)),
            }
        }
        Asked::Check { repo, commit } => {
            signingkeys::check_over(transport, forge, repo, commit).map(Heard::Checked)
        }
    }
}

/// **The running window's answer to `asked` from the command line**, with its own sign-in, as
/// the person in it: where it has none on GitHub, a sentence that says how to sign in.
pub fn answered_in_the_window(asked: &Asked) -> Result<Heard, String> {
    let forge = forge();
    let transport = signingkeys::window(&forge, &Caller::window()).map_err(|_| {
        format!(
            "this window is not signed in to {} (Settings › You › Accounts)",
            forge.kind.display()
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
    /// The window did not answer in time, or answered what this build does not read.
    NoAnswer,
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
        Answered::Said(Said::Asked { .. }) | Answered::NoAnswer => Reached::NoAnswer,
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

/// **This device's key, made where it has none**, and whether it was there before.
pub fn key_made(
    config_root: &Path,
    store: &dyn crate::secrets::keyring::Store,
) -> Result<(Signer, bool), String> {
    let had = crate::commitsign::kept(config_root, store)?.is_some();
    Ok((crate::commitsign::make(config_root, store)?, had))
}

#[cfg(test)]
mod tests;
