// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Settings › You › Accounts** and the token sign-in dialog (#1814, ADR 0077 §4, §6, §7): this
//! machine's forge accounts, listed, signed in to with a token on any host, and signed out of.
//!
//! The window is the host's human scope (`local-ui`, ADR 0077 §3): each act here claims
//! [`HostScope`], which a process a chat started is refused. All of it is the core's
//! ([`purlis_core::forge::accounts`]); this file only carries it to the window.
//!
//! **The token crosses once, inward.** [`forge_sign_in_with_token`] takes it from the window's
//! password box, held as a [`ForgeToken`] that is wiped as it is handed to the core, which keeps
//! it as a `SecretString` and then in its keyring item. No answer here carries it, and no error:
//! the core's sentences never hold it. The command is in `ipc_commands.rs`'s `vault_values`
//! class, which holds the commands that take a credential from the window (ADR 0052 amended
//! 2026-10-09).
//!
//! **The git the window runs signs in with these accounts** (#752, ADR 0070 amended 2026-10-12):
//! [`window_git`] runs a save, a fetch, a push or a clone with purlis's own credential helper for
//! a host the person has an account on.

use purlis_core::forge::Kind;
use purlis_core::forge::accounts::{self, ForgeAccount, Method, Pasted, State};
use purlis_core::forge::route::HostScope;

/// One forge account, as Settings lists it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ForgeAccountRow {
    /// What sign-out names it by.
    pub id: String,
    /// `github` or `gitlab`.
    pub kind: String,
    pub host: String,
    pub login: String,
    /// How it was signed in: `token`, `browser` or `imported`.
    pub method: String,
    pub signed_in: bool,
    /// The token's scopes when it was checked, or null when the forge does not say.
    pub scopes: Option<Vec<String>>,
    /// The scopes beyond what purlis needs (ADR 0077 §2).
    pub wider: Vec<String>,
}

impl From<ForgeAccount> for ForgeAccountRow {
    fn from(account: ForgeAccount) -> Self {
        let wider = account
            .scopes
            .as_deref()
            .map(|scopes| accounts::wider(account.kind, scopes))
            .unwrap_or_default();
        ForgeAccountRow {
            kind: account.kind.word().to_string(),
            method: match account.method {
                Method::Pat => "token",
                Method::Device | Method::Pkce => "browser",
                Method::Import => "imported",
            }
            .to_string(),
            signed_in: account.state == State::SignedIn,
            id: account.id,
            host: account.host,
            login: account.login,
            scopes: account.scopes,
            wider,
        }
    }
}

/// What the sign-in dialog shows for a host: the scopes to tick and where to make a token.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ForgeTokenHelp {
    pub host: String,
    pub scopes: Vec<String>,
    pub create_at: String,
}

/// A token the window hands over: never printed, and wiped as it is handed to the core.
#[derive(serde::Deserialize, specta::Type)]
#[serde(transparent)]
pub struct ForgeToken(String);

impl std::fmt::Debug for ForgeToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ForgeToken(***)")
    }
}

impl ForgeToken {
    /// The token, as the core holds it: the `String` it came in is wiped there.
    fn pasted(mut self) -> Pasted {
        Pasted::new(std::mem::take(&mut self.0))
    }
}

fn kind_of(word: &str) -> Result<Kind, String> {
    Kind::parse(word).ok_or_else(|| "Choose GitHub or GitLab.".to_string())
}

/// Runs `then` with the git it starts on this thread being the window's (#752): a network call to
/// a host the person has a signed-in account on gets purlis's own credential helper, this binary
/// started again, in place of the forge CLI's. Elsewhere, and in a chat's process tree, `then`
/// runs as it is, and the CLI's helper stays.
///
/// A test build (every one is fenced) runs `then` as it is too: the config home a test's window
/// would read is outside its fence. The core's tests cover the helper itself.
pub(crate) fn window_git<T>(then: impl FnOnce() -> T) -> T {
    if purlis_core::fence::FENCED {
        return then();
    }
    let (Some(scope), Ok(exe), Some(root)) = (
        HostScope::claim(),
        std::env::current_exe(),
        purlis_core::machine::config_root(),
    ) else {
        return then();
    };
    purlis_core::forge::gitcredential::as_the_window(&scope, &exe, &root, then)
}

fn config_root() -> Result<std::path::PathBuf, String> {
    purlis_core::machine::config_root()
        .ok_or_else(|| "purlis found no home directory to keep forge accounts in".to_string())
}

fn host_scope() -> Result<HostScope, String> {
    HostScope::claim().ok_or_else(|| "Forge sign-in is the window's, never a chat's.".to_string())
}

/// Hand this machine's forge sign-ins to the window's own forge calls (#1816): from here on a
/// call the person makes in the window, on a host they signed in to, goes over HTTPS with that
/// sign-in, and needs no `gh` or `glab`. Called once, as the app starts. A process a chat
/// started is refused the host's scope, so it hands nothing over.
pub fn hand_over_sign_ins() {
    let Some(root) = purlis_core::machine::config_root() else {
        tracing::warn!("purlis: no home directory, so no forge sign-in is used");
        return;
    };
    if !purlis_core::forge::signin::hand_over_unless_in_a_chat(&root) {
        tracing::warn!("purlis: forge sign-ins are not handed over inside a chat");
    }
}

/// Run `work` on a blocking thread: each act may wait on the keychain or the forge.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|err| format!("the forge accounts did not answer: {err}"))?
}

/// This machine's forge accounts.
#[tauri::command]
#[specta::specta]
pub async fn forge_accounts() -> Result<Vec<ForgeAccountRow>, String> {
    blocking(|| {
        let root = config_root()?;
        let keyring = accounts::keyring_at(&root);
        Ok(accounts::list(&root, keyring.as_ref())?
            .into_iter()
            .map(ForgeAccountRow::from)
            .collect())
    })
    .await
}

/// The scopes a token needs on `kind` and the page that makes one on `host`, or why `host` is not
/// a host.
#[tauri::command]
#[specta::specta]
pub fn forge_token_help(kind: String, host: String) -> Result<ForgeTokenHelp, String> {
    let help = accounts::token_help(kind_of(&kind)?, &host)?;
    Ok(ForgeTokenHelp {
        host: help.host,
        scopes: help.scopes.iter().map(|s| s.to_string()).collect(),
        create_at: help.create_at,
    })
}

/// Sign in to `kind` on `host` with a pasted `token`: checked with the forge, then kept in its
/// own keyring item. The account as Settings lists it, or the core's sentence saying why not.
#[tauri::command]
#[specta::specta]
pub async fn forge_sign_in_with_token(
    kind: String,
    host: String,
    token: ForgeToken,
) -> Result<ForgeAccountRow, String> {
    // Taken first, so a refusal below still leaves only the core's wiped copy.
    let token = token.pasted();
    let kind = kind_of(&kind)?;
    blocking(move || {
        let scope = host_scope()?;
        let root = config_root()?;
        let keyring = accounts::keyring_at(&root);
        accounts::sign_in_with_token(
            &scope,
            &root,
            keyring.as_ref(),
            &accounts::over_https,
            kind,
            &host,
            token,
        )
        .map(ForgeAccountRow::from)
    })
    .await
}

/// Sign out of the account `id`: its token deleted from the keychain and the account taken off
/// this machine.
#[tauri::command]
#[specta::specta]
pub async fn forge_sign_out(id: String) -> Result<(), String> {
    blocking(move || {
        let scope = host_scope()?;
        let root = config_root()?;
        let keyring = accounts::keyring_at(&root);
        accounts::sign_out(&scope, &root, keyring.as_ref(), &id).map(|_| ())
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_handed_token_never_prints() {
        let token: ForgeToken = serde_json::from_str("\"handed-over-once\"").unwrap();
        assert_eq!(format!("{token:?}"), "ForgeToken(***)");
    }

    #[test]
    fn a_row_says_how_it_was_signed_in_and_which_scopes_are_wider_than_needed() {
        let row = ForgeAccountRow::from(ForgeAccount {
            id: "01J9ACCOUNT".into(),
            kind: Kind::GitHub,
            host: "github.com".into(),
            login: "octocat".into(),
            method: Method::Pat,
            state: State::SignedIn,
            scopes: Some(vec!["repo".into(), "read:org".into(), "gist".into()]),
        });
        assert_eq!(row.method, "token");
        assert_eq!(row.kind, "github");
        assert!(row.signed_in);
        assert_eq!(row.wider, ["gist"]);
    }

    #[test]
    fn the_help_for_a_host_names_the_scopes_and_the_token_page() {
        let help = forge_token_help("gitlab".into(), "git.example.com".into()).unwrap();
        assert_eq!(help.scopes, ["api"]);
        assert!(help.create_at.starts_with("https://git.example.com/"));
        assert!(forge_token_help("svn".into(), "x".into()).is_err());
    }
}
