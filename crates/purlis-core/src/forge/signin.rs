// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The window's forge calls, sent with the person's purlis sign-in (#1816, ADR 0070 §4, ADR
//! 0077 §6 and §7).
//!
//! **The host hands its sign-ins over once, and only the host can.** The app calls
//! [`hand_over`] with the [`HostScope`] it claimed, which [`HostScope::claim`] refuses inside a
//! chat's process tree, and which the `purlis` binary never claims. From then on, in that
//! process alone, [`Forge::backend`](super::Forge::backend) resolves each call through a
//! [`Resolver`] that holds the sign-in of the host's account, if this machine has one
//! ([`accounts::signed_in_on`]). Where nothing was handed over (the `purlis` binary, any process
//! a chat started) every call takes the forge's CLI, as before.
//!
//! **Who gets the token is still [`route`](super::route::route)'s to say.** Handing over changes
//! which transports exist, not who is sent through them: only a human in the window, on a
//! signed-in account, takes the native transport. A window call that names no account is asked
//! as the host's signed-in account; every other caller is passed on unchanged, so a chat, an
//! MCP call, a trigger and a `purlis` command keep the CLI route.
//!
//! **The token is read per request.** [`KeyringToken`] reads it from the account's own keyring
//! item each time a request is sent, wipes the `String` the keyring answered with, and registers
//! the value with [`crate::applog::keep_out`]. Nothing here keeps it between requests.
//!
//! **With no account for a host**, the window's calls fall back to `gh`/`glab`, and [`ready`]'s
//! refusal names the purlis sign-in as the fix, so a machine without either CLI is told where to
//! sign in rather than to install one.

use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use secrecy::SecretString;

use super::accounts::{self, ForgeAccount, Method};
use super::backend::{Account, Caller, Principal, Surface, Transports};
use super::cli::Cli;
use super::http::TokenSource;
use super::route::{HostScope, Resolver, SignIn};
use super::transport::Transport;
use super::{Failure, Forge, ForgeError};

/// Where the person signs in, in the window's words.
pub const SIGN_IN_AT: &str = "Settings › You › Accounts";

/// One account's token, read from its own keyring item each time a request is sent.
pub struct KeyringToken {
    config_root: PathBuf,
    account: ForgeAccount,
}

impl TokenSource for KeyringToken {
    fn token(&self) -> Result<SecretString, ForgeError> {
        let keyring = accounts::keyring_at(&self.config_root);
        match accounts::token(keyring.as_ref(), &self.account) {
            Ok(Some(token)) => Ok(token),
            Ok(None) => Err(ForgeError::of(
                Failure::Auth,
                format!(
                    "purlis's sign-in to {} as {} has no token in the keychain any more. Sign in \
                     again in {SIGN_IN_AT}.",
                    self.account.host, self.account.login
                ),
            )),
            Err(why) => Err(ForgeError::transport(unechoed(&why))),
        }
    }
}

/// What the host handed over: its scope, the config home its accounts are kept in, and, in a
/// test build, the stand-ins its transports are replaced with.
struct Host {
    scope: HostScope,
    config_root: PathBuf,
    cli: Option<Arc<dyn Transport>>,
    #[cfg(any(test, feature = "fenced"))]
    native_through: Option<super::route::NativeThrough>,
    #[cfg(any(test, feature = "fenced"))]
    root: Option<super::http::ApiRoot>,
}

/// The host's sign-ins, once handed over. Nothing else in the crate writes it.
static HANDED: RwLock<Option<Arc<Host>>> = RwLock::new(None);

fn handed() -> Option<Arc<Host>> {
    HANDED.read().unwrap_or_else(|e| e.into_inner()).clone()
}

fn set(host: Option<Host>) {
    *HANDED.write().unwrap_or_else(|e| e.into_inner()) = host.map(Arc::new);
}

/// The host hands this process's sign-ins over: from now on the person's calls in the window
/// use the accounts kept under `config_root`. Called once, by the app, as it starts.
pub fn hand_over(scope: HostScope, config_root: &Path) {
    set(Some(Host {
        scope,
        config_root: config_root.to_path_buf(),
        cli: None,
        #[cfg(any(test, feature = "fenced"))]
        native_through: None,
        #[cfg(any(test, feature = "fenced"))]
        root: None,
    }));
}

/// [`hand_over`], with the host's scope claimed here: inside a chat's process tree the claim is
/// refused and nothing is handed over. Whether it was handed over. The app's one way in.
pub fn hand_over_unless_in_a_chat(config_root: &Path) -> bool {
    match HostScope::claim() {
        Some(scope) => {
            hand_over(scope, config_root);
            true
        }
        None => false,
    }
}

/// [`hand_over`], with the CLI route answered by `cli` and the native route by `native`: a
/// recorded forge in this process. Only a test build has it.
#[cfg(any(test, feature = "fenced"))]
pub fn hand_over_for_a_test(
    scope: HostScope,
    config_root: &Path,
    native: super::route::NativeThrough,
    cli: Arc<dyn Transport>,
) {
    set(Some(Host {
        scope,
        config_root: config_root.to_path_buf(),
        cli: Some(cli),
        native_through: Some(native),
        root: None,
    }));
}

/// [`hand_over`], with native calls sent over HTTP to `root` (a recorded forge on this machine's
/// loopback) and the CLI route answered by `cli`. Only a test build has it.
#[cfg(any(test, feature = "fenced"))]
pub fn hand_over_at_root_for_a_test(
    scope: HostScope,
    config_root: &Path,
    root: super::http::ApiRoot,
    cli: Arc<dyn Transport>,
) {
    set(Some(Host {
        scope,
        config_root: config_root.to_path_buf(),
        cli: Some(cli),
        native_through: None,
        root: Some(root),
    }));
}

/// Take the sign-ins back, as if none had been handed over. Only a test build has it.
#[cfg(any(test, feature = "fenced"))]
pub fn take_back() {
    set(None);
}

/// Whether the host handed its sign-ins over and this machine has a signed-in account: whether
/// a forge call the window makes could be sent with one.
pub fn holds_a_sign_in() -> bool {
    handed().is_some_and(|host| {
        accounts::list(
            &host.config_root,
            accounts::keyring_at(&host.config_root).as_ref(),
        )
        .is_ok_and(|all| all.iter().any(|a| a.state == accounts::State::SignedIn))
    })
}

/// This machine's forge accounts, where the host handed its sign-ins over: the accounts the
/// window may ask a forge as. `None` where nothing was handed over (the `purlis` binary, a
/// process a chat started), so nothing there is asked as a purlis account. An accounts file that
/// cannot be read holds none.
pub fn accounts() -> Option<Vec<ForgeAccount>> {
    let host = handed()?;
    Some(
        accounts::list(
            &host.config_root,
            accounts::keyring_at(&host.config_root).as_ref(),
        )
        .unwrap_or_default(),
    )
}

/// Whether `caller` is the person in the window: the one caller a sign-in is for.
fn the_person_in_the_window(caller: &Caller) -> bool {
    !caller.is_a_chat()
        && caller.surface() == Surface::Window
        && *caller.principal() == Principal::Human
}

fn account_of(account: &ForgeAccount) -> Account {
    Account {
        kind: account.kind,
        host: account.host.clone(),
        login: account.login.clone(),
    }
}

/// The host's signed-in account on `forge`'s host, or `None`. An accounts file that cannot be
/// read holds no sign-in purlis can use, so the call takes the CLI and says so if that fails.
fn signed_in(host: &Host, forge: &Forge) -> Option<ForgeAccount> {
    accounts::signed_in_on(&host.config_root, forge.kind, &forge.host)
        .ok()
        .flatten()
}

/// The transports `forge`'s calls take in this process, where the host handed its sign-ins
/// over; `None` where it did not, and every call takes the CLI.
pub(super) fn transports(forge: &Forge) -> Option<Arc<dyn Transports>> {
    let host = handed()?;
    let cli = host.cli.clone();
    Some(window_over(&host, forge, cli))
}

/// `forge`'s backend for a call that may be sent only with the person's purlis sign-in, and
/// never through `gh` or `glab` under whatever login the CLI holds: `purlis report` filing on
/// purlis's own tracker under the reporter's own account (#806, ADR 0059).
///
/// Refused, before anything is sent, unless the host handed its sign-ins over, `caller` is the
/// person in the window, and an account is signed in on `forge`'s host. The backend's CLI
/// route refuses every call, so a call the router would send to the CLI is refused too rather
/// than sent under another identity (ADR 0070 ruling 7). A `purlis` command and a chat hold no
/// sign-in (ADR 0070 ruling 5), so they are always refused here.
pub fn signed_in_backend(
    forge: &Forge,
    caller: &Caller,
) -> Result<Box<dyn super::ForgeBackend>, ForgeError> {
    let host = handed()
        .filter(|host| the_person_in_the_window(caller) && signed_in(host, forge).is_some());
    let Some(host) = host else {
        return Err(ForgeError::of(
            Failure::Auth,
            format!(
                "purlis sends this only with its own sign-in to {} on {}, which is the app's, \
                 for the person in its window ({SIGN_IN_AT})",
                forge.kind.display(),
                forge.host
            ),
        ));
    };
    let refused: Arc<dyn Transport> = Arc::new(NoCliRoute);
    Ok(forge.backend_for(window_over(&host, forge, Some(refused))))
}

/// The CLI route of [`signed_in_backend`]: every call and login check refused, unsent.
struct NoCliRoute;

impl NoCliRoute {
    const SAID: &'static str =
        "purlis sends this only with its own sign-in, never through gh or glab";
}

impl Transport for NoCliRoute {
    fn send(
        &self,
        _forge: &Forge,
        _call: &super::transport::Call,
    ) -> Result<super::transport::Reply, super::transport::NoAnswer> {
        Err(super::transport::NoAnswer::Refused(Self::SAID.to_string()))
    }

    fn check_auth(&self, _forge: &Forge) -> Result<(), ForgeError> {
        Err(ForgeError::of(Failure::Auth, Self::SAID))
    }
}

/// The window's transports for `forge` under `host`'s sign-ins, the CLI route answered by `cli`
/// when one is given and by the forge's own CLI otherwise.
fn window_over(host: &Host, forge: &Forge, cli: Option<Arc<dyn Transport>>) -> Arc<dyn Transports> {
    let mut resolver = Resolver::new(forge.kind, &forge.host)
        .etags_in(&host.config_root)
        .budgets_in(&host.config_root);
    if let Some(cli) = cli {
        resolver = resolver.cli_over(cli);
    }
    #[cfg(any(test, feature = "fenced"))]
    if let Some(native) = &host.native_through {
        resolver = resolver.native_through(native.clone());
    }
    #[cfg(any(test, feature = "fenced"))]
    if let Some(root) = &host.root {
        resolver = resolver.at_root(root.clone());
    }
    // Every account signed in on this host, so a window call that names one of them (the repo
    // list asks as each, #1818) is sent as that one; a call that names none is asked as the
    // first.
    let every = accounts::every_signed_in_on(&host.config_root, forge.kind, &forge.host)
        .unwrap_or_default();
    for account in &every {
        resolver = resolver.signed_in(
            &host.scope,
            account_of(account),
            SignIn {
                tokens: Arc::new(KeyringToken {
                    config_root: host.config_root.clone(),
                    account: account.clone(),
                }),
                imported_from_cli: account.method == Method::Import,
            },
        );
    }
    Arc::new(Window {
        resolver,
        account: every.first().map(account_of),
    })
}

/// One forge's transports in the window's process: the resolver, and the account a window call
/// that names none is asked as.
struct Window {
    resolver: Resolver,
    account: Option<Account>,
}

impl Transports for Window {
    fn for_caller(&self, caller: &Caller) -> Result<Arc<dyn Transport>, ForgeError> {
        let resolved = match &self.account {
            Some(account) if caller.account().is_none() && the_person_in_the_window(caller) => self
                .resolver
                .for_caller(&caller.clone().as_account(account.clone())),
            _ => self.resolver.for_caller(caller),
        };
        match resolved {
            Ok(transport) => Ok(Arc::new(Unechoed(transport))),
            Err(why) => {
                let said = unechoed(why.said());
                Err(why.reworded(said))
            }
        }
    }
}

/// `said` with every token this process holds taken out ([`accounts::unechoed`]): what each
/// answer and refusal of the window's forge calls goes through, so a forge or a transport that
/// quoted a token back never has it repeated.
fn unechoed(said: &str) -> String {
    accounts::unechoed(said.to_string(), &SecretString::from(""))
}

/// A transport whose answers and refusals go through [`unechoed`].
struct Unechoed(Arc<dyn Transport>);

impl Transport for Unechoed {
    fn send(
        &self,
        forge: &Forge,
        call: &super::transport::Call,
    ) -> Result<super::transport::Reply, super::transport::NoAnswer> {
        use super::transport::NoAnswer;
        match self.0.send(forge, call) {
            Ok(mut reply) => {
                reply.out = unechoed(&reply.out);
                reply.err = unechoed(&reply.err);
                Ok(reply)
            }
            Err(NoAnswer::Timeout(why)) => Err(NoAnswer::Timeout(unechoed(&why))),
            Err(NoAnswer::Missing(why)) => Err(NoAnswer::Missing(unechoed(&why))),
            Err(NoAnswer::Refused(why)) => Err(NoAnswer::Refused(unechoed(&why))),
            Err(NoAnswer::HeldBack { said, reset }) => Err(NoAnswer::HeldBack {
                said: unechoed(&said),
                reset,
            }),
        }
    }

    fn check_auth(&self, forge: &Forge) -> Result<(), ForgeError> {
        self.0.check_auth(forge).map_err(|why| {
            let said = unechoed(why.said());
            why.reworded(said)
        })
    }
}

/// Whether `forge` can be asked as `caller`, before a call that needs a login: the window's
/// [`Forge::check_auth`].
///
/// Where the host handed its sign-ins over and `caller` is the person in the window, a
/// signed-in account is enough, with no CLI asked at all. Otherwise the CLI is asked, as
/// [`Forge::check_auth`] asks it; in the window, its refusal names the purlis sign-in as the
/// fix. Where nothing was handed over this is [`Forge::check_auth`], word for word.
pub fn ready(forge: &Forge, caller: &Caller) -> Result<(), ForgeError> {
    let Some(host) = handed() else {
        return forge.check_auth();
    };
    if the_person_in_the_window(caller) && signed_in(&host, forge).is_some() {
        return Ok(());
    }
    let cli: Arc<dyn Transport> = host.cli.clone().unwrap_or_else(|| Arc::new(Cli::default()));
    cli.check_auth(forge)
        .map_err(|why| sign_in_instead(forge, why))
}

/// `why`, the CLI's refusal, preceded by the fix the window offers: the purlis sign-in.
fn sign_in_instead(forge: &Forge, why: ForgeError) -> ForgeError {
    let said = format!(
        "purlis is not signed in to {} on {}: sign in in {SIGN_IN_AT}. ({})",
        forge.kind.display(),
        forge.host,
        why.said()
    );
    why.reworded(said)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forge::Kind;

    #[test]
    fn only_a_human_in_the_window_is_the_person_a_sign_in_is_for() {
        assert!(the_person_in_the_window(&Caller::window()));
        assert!(the_person_in_the_window(&Caller::window().background()));
        for other in [
            Caller::command(),
            Caller::trigger(),
            Caller::chat("c1"),
            Caller::mcp("c1"),
        ] {
            assert!(!the_person_in_the_window(&other), "{other:?}");
        }
    }

    #[test]
    fn the_clis_refusal_in_the_window_names_the_sign_in_and_keeps_its_kind() {
        let forge = Forge::default_of(Kind::GitHub);
        let why = ForgeError::transport(
            "purlis could not find gh on PATH — install it and log in (`gh auth login`)",
        );
        let said = sign_in_instead(&forge, why);
        assert_eq!(said.failure(), &Failure::Transport);
        assert!(
            said.said()
                .starts_with("purlis is not signed in to GitHub on github.com")
        );
        assert!(said.said().contains(SIGN_IN_AT), "{said}");
        assert!(said.said().contains("could not find gh"), "{said}");
    }
}
