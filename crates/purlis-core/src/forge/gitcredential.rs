// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! purlis's own git credential helper, for the git the window runs (ADR 0070, amended
//! 2026-10-12; #752).
//!
//! A forge sign-in (ADR 0077, [`super::accounts`]) gives purlis a token for the forge's API. A
//! `git clone`, `fetch` or `push` over HTTPS asks git's credential helper instead, and until now
//! that helper was the forge CLI's (`gh auth git-credential`). This module lets the git the
//! window runs use the person's purlis account, so it needs no `gh` or `glab`.
//!
//! **Where the helper is named.** Only on the command line of a git the window runs, and only
//! while the window's thread says so ([`as_the_window`]). Never in a clone's, a worktree's or the
//! person's global git config, where a chat's own `git push` would find it. A call the app runs
//! for a chat (a brokered call, [`git::isolated`]) never gets it, inside the window's scope or
//! not. [`super::helper_for`] asks [`for_the_window`] first and falls back to the CLI's helper,
//! so a host with no purlis account keeps the CLI's login.
//!
//! **No other helper ever sees the token.** Git hands a credential that worked to every helper
//! it is configured with, and asks each to store it. Apple's git configures `osxkeychain`
//! system-wide, so purlis's helper always comes after an empty `credential.helper`, which drops
//! every helper configured before it: [`git::run_network`]'s rule already does, and
//! [`settings_for_the_window`] does for a call that has no such rule. The helper itself answers
//! `store` and `erase` with nothing: the token belongs to the account store.
//!
//! **The helper is the app's own binary**, started again by git with [`HELPER_ARG`]
//! ([`serve_if_asked`]). It reads the token from the account's keyring item, which the app's
//! binary holds, and writes it to git's pipe and nowhere else: not to an argument, not to the
//! environment, not to a file. It answers only `get`, only over HTTPS, only for the host of the
//! account the window named, and never inside a chat's process tree ([`HostScope`]).

use std::cell::RefCell;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use secrecy::ExposeSecret;

use super::Kind;
use super::accounts::{self, ForgeAccount, State};
use super::route::HostScope;
use crate::worktree::git;

#[cfg(test)]
mod tests;

/// The argument that starts the app's binary as the credential helper.
pub const HELPER_ARG: &str = "purlis-git-credential";

/// The most the helper reads of what git sends: a handful of `key=value` lines.
const MOST_INPUT: u64 = 64 * 1024;

/// What the helper says, on its standard error, when a chat's process tree starts it.
pub const REFUSED_IN_A_CHAT: &str =
    "purlis's git credential helper answers only the git the purlis window runs, never a chat's.";

/// The git the window runs on this thread: the app's binary, and the config home its accounts are
/// in.
#[derive(Clone)]
struct Window {
    exe: PathBuf,
    config_root: PathBuf,
}

thread_local! {
    static WINDOW: RefCell<Option<Window>> = const { RefCell::new(None) };
}

/// Runs `then` with every git this thread starts being the window's: a network call to a host
/// the person has a purlis account on gets purlis's helper, the binary `exe` started again, with
/// the accounts under `config_root`. Only the host can claim the [`HostScope`] this takes.
pub fn as_the_window<T>(
    _host: &HostScope,
    exe: &Path,
    config_root: &Path,
    then: impl FnOnce() -> T,
) -> T {
    struct Restore(Option<Window>);
    impl Drop for Restore {
        fn drop(&mut self) {
            let before = self.0.take();
            WINDOW.with(|held| *held.borrow_mut() = before);
        }
    }
    let window = Window {
        exe: exe.to_path_buf(),
        config_root: config_root.to_path_buf(),
    };
    let before = WINDOW.with(|held| held.borrow_mut().replace(window));
    let _restore = Restore(before);
    then()
}

/// The `credential.helper` value for a git the window runs against `host`: purlis's helper, for
/// the first signed-in account on that host. `None` outside [`as_the_window`], in a call the app
/// runs for a chat, on a host with no signed-in account, or where a path cannot be quoted for
/// the shell git runs the helper through.
pub fn for_the_window(host: &str) -> Option<String> {
    if git::isolation().is_some() {
        return None;
    }
    let window = WINDOW.with(|held| held.borrow().clone())?;
    let host = host.to_ascii_lowercase();
    let keyring = accounts::keyring_at(&window.config_root);
    let account = accounts::list(&window.config_root, keyring.as_ref())
        .ok()?
        .into_iter()
        .find(|account| account.state == State::SignedIn && account.host == host)?;
    helper_command(&window.exe, &window.config_root, &account.id)
}

/// The `-c` settings for a git the window runs against `host` that has no reset of its own: an
/// empty `credential.helper`, which drops every helper configured before it, then purlis's.
/// `None` where [`for_the_window`] is.
pub fn settings_for_the_window(host: &str) -> Option<[String; 2]> {
    let helper = for_the_window(host)?;
    Some([
        "credential.helper=".to_string(),
        format!("credential.helper={helper}"),
    ])
}

/// `!'<exe>' purlis-git-credential --config '<root>' --account <id>`: git runs a `!` helper
/// through the shell and adds the operation last.
fn helper_command(exe: &Path, config_root: &Path, id: &str) -> Option<String> {
    if id.is_empty() || !id.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return None;
    }
    Some(format!(
        "!{} {HELPER_ARG} --config {} --account {id}",
        quoted(exe)?,
        quoted(config_root)?
    ))
}

/// `path` single-quoted for the shell, or `None` when it holds a quote or is not UTF-8.
fn quoted(path: &Path) -> Option<String> {
    let text = path.to_str()?;
    (!text.contains('\'')).then(|| format!("'{text}'"))
}

/// Started again by git as the credential helper: answer git and exit, before anything of the
/// app starts. `None` when this process was not started as one.
pub fn serve_if_asked() -> Option<i32> {
    let mut args = std::env::args_os().skip(1);
    if args.next()? != HELPER_ARG {
        return None;
    }
    let rest: Vec<String> = args.map(|a| a.to_string_lossy().into_owned()).collect();
    Some(serve(
        &rest,
        &mut std::io::stdin().lock(),
        &mut std::io::stdout().lock(),
        &mut std::io::stderr().lock(),
        &crate::envvar::var,
    ))
}

/// The helper: `args` are what follow [`HELPER_ARG`], git's operation last; `input` is what git
/// sends; the answer goes to `output`. The environment is asked through `env`. The exit status.
pub fn serve(
    args: &[String],
    input: &mut dyn Read,
    output: &mut dyn Write,
    error: &mut dyn Write,
    env: &dyn Fn(&str) -> Option<String>,
) -> i32 {
    // Everything git sends is read, whatever the answer, so git never writes into a closed pipe.
    let mut sent = Vec::new();
    if input.take(MOST_INPUT).read_to_end(&mut sent).is_err() {
        return 1;
    }
    let Some((operation, flags)) = args.split_last() else {
        return 1;
    };
    // `store` and `erase`, and any operation a later git adds: nothing. The token belongs to the
    // account store, and git never writes it anywhere.
    if operation != "get" {
        return 0;
    }
    let Some(host) = HostScope::claim_in(env) else {
        let _ = writeln!(error, "{REFUSED_IN_A_CHAT}");
        return 0;
    };
    let Some(named) = Named::from(flags) else {
        let _ = writeln!(
            error,
            "purlis's git credential helper was started without the account to answer for."
        );
        return 1;
    };
    match answer(&host, &named, &Asked::from(&sent)) {
        Some(answer) => {
            if output.write_all(answer.as_bytes()).is_err() || output.flush().is_err() {
                return 1;
            }
            0
        }
        None => 0,
    }
}

/// What the window named on the helper's command line.
struct Named {
    config_root: PathBuf,
    account: String,
}

impl Named {
    fn from(flags: &[String]) -> Option<Named> {
        match flags {
            [config, root, account, id] if config == "--config" && account == "--account" => {
                Some(Named {
                    config_root: PathBuf::from(root),
                    account: id.clone(),
                })
            }
            _ => None,
        }
    }
}

/// What git asked: the protocol and host of the URL it is fetching or pushing.
#[derive(Default)]
struct Asked {
    protocol: String,
    host: String,
}

impl Asked {
    fn from(sent: &[u8]) -> Asked {
        let mut asked = Asked::default();
        for line in String::from_utf8_lossy(sent).lines() {
            match line.split_once('=') {
                Some(("protocol", value)) => asked.protocol = value.to_string(),
                Some(("host", value)) => asked.host = value.to_ascii_lowercase(),
                _ => {}
            }
        }
        asked
    }
}

/// `username=…` and `password=…` for the account named, when git asked over HTTPS for that
/// account's host and its token is in the keyring.
fn answer(_host: &HostScope, named: &Named, asked: &Asked) -> Option<String> {
    let keyring = accounts::keyring_at(&named.config_root);
    let account = accounts::list(&named.config_root, keyring.as_ref())
        .ok()?
        .into_iter()
        .find(|account| account.id == named.account && account.state == State::SignedIn)?;
    if asked.host != account.host || !secure(&asked.protocol, &account.host) {
        return None;
    }
    // Read as the window's forge calls read it: the keyring's `String` wiped, the value kept
    // out of every log line (#1816).
    let token = accounts::token(keyring.as_ref(), &account).ok()??;
    let token = token.expose_secret();
    // A value git would split is not a token: nothing rather than a broken answer.
    if token.is_empty() || token.contains(['\n', '\r', '\0']) {
        return None;
    }
    Some(format!(
        "username={}\npassword={token}\n",
        username(&account)
    ))
}

/// Whether the token may go over `protocol` to `host`: HTTPS only. A test build also lets a
/// stand-in forge on this machine's loopback be reached over plain HTTP, which no build a person
/// runs does.
fn secure(protocol: &str, host: &str) -> bool {
    if protocol == "https" {
        return true;
    }
    crate::fence::FENCED && protocol == "http" && {
        let name = host.rsplit_once(':').map_or(host, |(name, _)| name);
        name == "127.0.0.1" || name == "localhost"
    }
}

/// The user name git sends with the token: the login on GitHub, which takes any; `oauth2` on
/// GitLab, which takes it for an OAuth token and for a personal access token alike.
fn username(account: &ForgeAccount) -> &str {
    match account.kind {
        Kind::GitHub => &account.login,
        Kind::GitLab => "oauth2",
    }
}
