// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Using an existing `gh` or `glab` login** (ADR 0077 §5, #1376): offered, never taken.
//!
//! A person who already signed `gh` or `glab` in on this machine need not sign in again: purlis
//! can copy that login's token into an account of its own. Two acts, kept apart on purpose.
//!
//! - [`offered`] says which logins could be imported: a forge CLI installed and signed in to its
//!   forge's own host (github.com, gitlab.com), where purlis has no signed-in account yet. It asks
//!   the CLI whether it is signed in (`auth status`) and **never asks it for a token**: the offer
//!   reads nothing secret, writes nothing, and imports nothing.
//! - [`import`] is the person's yes. Only then is the CLI asked for its stored token (`gh auth
//!   token`, `glab config get token`), with none of either CLI's token variables in its
//!   environment, so the answer is the login the CLI keeps. The token is checked with the forge
//!   and kept as the account's own keyring item exactly as a pasted one is
//!   ([`accounts::sign_in`]), and the account's method is `import`.
//!
//! **What the import costs, which the offer says** (V22c, ADR 0070 §4): the imported login
//! becomes the person's. From then on a chat on that host gets no forge credential from purlis
//! ([`accounts::imported_on`], [`super::route::route`]): it had the CLI's login, and it loses it,
//! until chats get sign-ins of their own (SD-7a/b). The CLI keeps its own copy. purlis never
//! refreshes the copy it took: a CLI login that expires is signed in again, or imported again.
//!
//! **The token is held as the import holds a pasted one**: the CLI's answer is wiped as it is
//! taken ([`accounts::Pasted`]), it is registered with [`crate::applog::keep_out`] before the
//! forge is asked, and no sentence here holds it. A CLI that refused says so in purlis's words,
//! never in its own, which could quote what it printed.
//!
//! **Only the host imports.** [`import`] takes a [`HostScope`], which a process a chat started is
//! refused, and the `purlis` binary never names this module.

use std::path::Path;

use super::accounts::{self, ForgeAccount, Method, Pasted, Reach};
use super::cli::{self, Cli};
use super::route::HostScope;
use super::transport::{NoAnswer, Transport};
use super::{Forge, Kind, find_cli};
use crate::secrets::keyring::Store;

/// The forges whose CLI login can be imported, in the order they are offered.
const FORGES: [Kind; 2] = [Kind::GitHub, Kind::GitLab];

/// A forge CLI's login that could be imported: `cli` is signed in to `host`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliLogin {
    pub kind: Kind,
    /// The forge's own host: github.com or gitlab.com.
    pub host: String,
    /// `gh` or `glab`.
    pub cli: &'static str,
}

/// The CLI logins on this machine that could be imported, `installed` and `signed_in` asking
/// each forge's CLI: one per forge whose CLI is installed and signed in to the forge's own host,
/// on a host where purlis has no signed-in account yet. Nothing here asks a CLI for a token.
pub fn offered(
    config_root: &Path,
    installed: &dyn Fn(Kind) -> bool,
    signed_in: &dyn Fn(&Forge) -> bool,
) -> Vec<CliLogin> {
    FORGES
        .iter()
        .map(|&kind| Forge::default_of(kind))
        .filter(|forge| {
            // An accounts file that cannot be read offers nothing: an import would write over
            // it, and the import refuses it anyway.
            matches!(
                accounts::signed_in_on(config_root, forge.kind, &forge.host),
                Ok(None)
            )
        })
        .filter(|forge| installed(forge.kind) && signed_in(forge))
        .map(|forge| CliLogin {
            kind: forge.kind,
            cli: forge.kind.cli(),
            host: forge.host,
        })
        .collect()
}

/// [`offered`], asking this machine's own `gh` and `glab`: found as every forge call finds them,
/// and asked whether their stored login is signed in, never a token in this process's
/// environment.
pub fn offered_here(config_root: &Path) -> Vec<CliLogin> {
    offered(
        config_root,
        &|kind| find_cli(kind.cli()).is_some(),
        &|forge| Cli::as_the_operator().check_auth(forge).is_ok(),
    )
}

/// The person's yes: `kind`'s CLI's stored login on `host` copied into an account of purlis's
/// own, checked with the forge through `reach` and kept in its own keyring item, its method
/// `import`. The account, as listed; or why not, in a sentence that never holds the token.
pub fn import(
    scope: &HostScope,
    config_root: &Path,
    keyring: &dyn Store,
    reach: Reach<'_>,
    kind: Kind,
    host: &str,
) -> Result<ForgeAccount, String> {
    let host = accounts::host_named(kind, host)?;
    let token = stored(kind, &host)?;
    accounts::sign_in(
        scope,
        config_root,
        keyring,
        reach,
        kind,
        &host,
        token,
        Method::Import,
    )
    .map_err(|why| format!("purlis could not use {}'s login: {why}", kind.cli()))
}

/// The token `kind`'s CLI keeps for `host`, taken into a [`Pasted`] and its answer wiped.
fn stored(kind: Kind, host: &str) -> Result<Pasted, String> {
    let name = kind.cli();
    let mut reply = match cli::stored_token(kind, host) {
        Ok(reply) => reply,
        Err(NoAnswer::Timeout(_)) => {
            return Err(format!(
                "{name} did not answer when asked for its login on {host}. Try again, or sign in \
                 with a token instead."
            ));
        }
        Err(_) => {
            return Err(format!(
                "purlis could not run {name} to read its login on {host}. Sign in with a token \
                 instead."
            ));
        }
    };
    // Whatever the CLI printed, it is wiped here: on a refusal it may still hold a value.
    let printed = std::mem::take(&mut reply.out);
    let ok = reply.ok();
    let token = Pasted::new(printed);
    reply.err.clear();
    if !ok || token.is_empty() {
        return Err(format!(
            "{name} has no login on {host} for purlis to use. Run `{name} auth status` to see \
             it, or sign in with a token instead."
        ));
    }
    Ok(token)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn home() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn a_cli_signed_in_to_its_forges_own_host_is_offered() {
        let config = home();
        let offers = offered(config.path(), &|_| true, &|_| true);
        assert_eq!(
            offers,
            [
                CliLogin {
                    kind: Kind::GitHub,
                    host: "github.com".into(),
                    cli: "gh",
                },
                CliLogin {
                    kind: Kind::GitLab,
                    host: "gitlab.com".into(),
                    cli: "glab",
                },
            ]
        );
    }

    #[test]
    fn a_cli_not_installed_or_not_signed_in_is_not_offered() {
        let config = home();
        let only_gh = |kind: Kind| kind == Kind::GitHub;
        assert_eq!(
            offered(config.path(), &only_gh, &|_| true)
                .iter()
                .map(|o| o.cli)
                .collect::<Vec<_>>(),
            ["gh"]
        );
        let lab_signed_in = |forge: &Forge| forge.kind == Kind::GitLab;
        assert_eq!(
            offered(config.path(), &|_| true, &lab_signed_in)
                .iter()
                .map(|o| o.cli)
                .collect::<Vec<_>>(),
            ["glab"]
        );
        // Asked of an installed one only.
        let asked = std::cell::Cell::new(0);
        let counted = |_: &Forge| {
            asked.set(asked.get() + 1);
            true
        };
        assert!(offered(config.path(), &|_| false, &counted).is_empty());
        assert_eq!(asked.get(), 0);
    }

    #[test]
    fn an_accounts_file_that_cannot_be_read_offers_nothing() {
        let config = home();
        let at = accounts::path(config.path());
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(&at, "not json").unwrap();
        assert!(offered(config.path(), &|_| true, &|_| true).is_empty());
    }
}
