// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The Signed commits card's management** (#5632, ADR 0116 §7): Remove signing, Replace key,
//! and another device's key removed, each over the forge accounts' own transports, so a
//! recording answers them in a test.
//!
//! - **Remove signing** takes the key out of this device's keychain first, so signing is off at
//!   once whatever a forge answers, then out of every account that has it. An account that
//!   refused says so, and its keys page is the way to finish there.
//! - **Replace key** makes a new key in place of the old one, adds it to every account that had
//!   the old one, then removes the old one there. The card checks again after.
//! - **Another device's key**, purlis's by its title, is removed from an account by its id, for
//!   a lost or wiped machine.

use std::path::Path;

use crate::commitsign::{self, Signer};
use crate::forge::Forge;
use crate::forge::signingkeys::{self, Refused};
use crate::forge::transport::Transport;
use crate::secrets::keyring::Store;

/// One forge account the card acts on: its forge and the window's transport to it.
pub struct OnAccount<'a> {
    pub forge: Forge,
    pub transport: &'a dyn Transport,
}

/// What became of one account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Done there.
    Done,
    /// The account never had the key: nothing to do there.
    NotThere,
    /// The forge refused, in a sentence; the account's keys page finishes it.
    Refused(String),
}

/// **Remove signing** (ADR 0116 §7): this device's key out of its keychain, then out of each of
/// `accounts` that has it. What became of each account, in order. `Err` where the keychain held
/// no key or would not give it up: nothing is asked of any forge then.
pub fn remove_everywhere(
    config_root: &Path,
    store: &dyn Store,
    accounts: &[OnAccount],
) -> Result<Vec<Outcome>, String> {
    let signer = commitsign::kept(config_root, store)?
        .ok_or_else(|| "this Mac has no signing key to remove".to_owned())?;
    commitsign::forget(config_root, store)?;
    Ok(accounts
        .iter()
        .map(|on| removed_from(on, &signer))
        .collect())
}

/// `signer`'s key out of the account `on`.
fn removed_from(on: &OnAccount, signer: &Signer) -> Outcome {
    let listed = match signingkeys::listed_over(on.transport, &on.forge) {
        Ok(listed) => listed,
        Err(why) => return Outcome::Refused(why.said(on.forge.kind)),
    };
    let Some(key) = listed.this_device(signer) else {
        return Outcome::NotThere;
    };
    match signingkeys::remove_over(on.transport, &on.forge, key.id) {
        Ok(()) => Outcome::Done,
        Err(why) => Outcome::Refused(why.said(on.forge.kind)),
    }
}

/// **Replace key** (ADR 0116 §7): a new key in place of this device's, added under `title` to
/// each of `accounts` that had the old one, and the old one removed there. The new key, and
/// what became of each account. The old key stays on an account whose forge refused the new
/// one, said so.
pub fn replace(
    config_root: &Path,
    store: &dyn Store,
    accounts: &[OnAccount],
    title: &str,
) -> Result<(Signer, Vec<Outcome>), String> {
    replace_with(config_root, store, accounts, title, Signer::generate()?)
}

/// [`replace`] with `new`, made already: a test's own key, known to its recordings.
pub fn replace_with(
    config_root: &Path,
    store: &dyn Store,
    accounts: &[OnAccount],
    title: &str,
    new: Signer,
) -> Result<(Signer, Vec<Outcome>), String> {
    let old = commitsign::kept(config_root, store)?
        .ok_or_else(|| "this Mac has no signing key to replace".to_owned())?;
    // Where the old one is, read before it is gone from the keychain.
    let had: Vec<Result<Option<u64>, Refused>> = accounts
        .iter()
        .map(|on| {
            signingkeys::listed_over(on.transport, &on.forge)
                .map(|listed| listed.this_device(&old).map(|key| key.id))
        })
        .collect();
    let new = commitsign::replace_with(config_root, store, &new)?;
    let outcomes = accounts
        .iter()
        .zip(had)
        .map(|(on, had)| match had {
            Err(why) => Outcome::Refused(why.said(on.forge.kind)),
            Ok(None) => Outcome::NotThere,
            Ok(Some(id)) => {
                if let Err(why) = signingkeys::add_over(on.transport, &on.forge, &new, title) {
                    return Outcome::Refused(why.said(on.forge.kind));
                }
                match signingkeys::remove_over(on.transport, &on.forge, id) {
                    Ok(()) => Outcome::Done,
                    Err(why) => Outcome::Refused(why.said(on.forge.kind)),
                }
            }
        })
        .collect();
    Ok((new, outcomes))
}

/// **Another device's purlis key removed** from the account `on`, by its `id`: refused where
/// the key is not purlis's or is this device's own, which Remove signing takes instead.
pub fn remove_other_device(on: &OnAccount, signer: Option<&Signer>, id: u64) -> Result<(), String> {
    let listed =
        signingkeys::listed_over(on.transport, &on.forge).map_err(|why| why.said(on.forge.kind))?;
    let key = listed
        .keys
        .iter()
        .find(|key| key.id == id)
        .ok_or_else(|| format!("{} has no such key any more", on.forge.kind.display()))?;
    if !key.purlis_made() || signer.is_some_and(|signer| key.is(signer)) {
        return Err("that key is not another of your devices' purlis keys".to_owned());
    }
    signingkeys::remove_over(on.transport, &on.forge, id).map_err(|why| why.said(on.forge.kind))
}

/// The forge's page of the account's keys, where a key purlis may not remove is removed by hand.
pub fn keys_page(forge: &Forge) -> String {
    match forge.kind {
        crate::forge::Kind::GitHub => "https://github.com/settings/keys".to_owned(),
        crate::forge::Kind::GitLab => format!("https://{}/-/user_settings/ssh_keys", forge.host),
    }
}

#[cfg(test)]
mod tests;
