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
//!
//! **The helper never refreshes a token.** A browser sign-in's token expires, and renewing it
//! rotates its refresh token, which only one process may do (ADR 0077 §3). So the window renews
//! a token that is about to expire before it names the helper ([`for_the_window`]), and the
//! helper only reads what the keyring item then holds.

use std::cell::RefCell;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use secrecy::ExposeSecret;

use super::Kind;
use super::accounts::{self, ForgeAccount, Method, State};
use super::refresh::Refresher;
use super::route::HostScope;
use crate::worktree::git;

pub mod remote;
#[cfg(test)]
pub(crate) mod tests;

/// The argument that starts the app's binary as the credential helper.
pub const HELPER_ARG: &str = "purlis-git-credential";

/// The most the helper reads of what git sends: a handful of `key=value` lines.
const MOST_INPUT: u64 = 64 * 1024;

/// What the helper says, on its standard error, when a chat's process tree starts it.
pub const REFUSED_IN_A_CHAT: &str =
    "purlis's git credential helper answers only the git the purlis window runs, never a chat's.";

/// The git the window runs on this thread: the app's binary, and the config home its accounts are
/// in. Carried to a command's own threads with [`held`] and [`within`].
#[derive(Clone)]
pub struct Window {
    exe: PathBuf,
    config_root: PathBuf,
    /// Renews a browser sign-in's token before the git that will ask for it starts.
    refresher: std::sync::Arc<Refresher>,
}

thread_local! {
    static WINDOW: RefCell<Option<Window>> = const { RefCell::new(None) };
}

/// Runs `then` with every git this thread starts being the window's: a network call to a host
/// the person has a purlis account on gets purlis's helper, the binary `exe` started again, with
/// the accounts under `config_root`. Only the host can claim the [`HostScope`] this takes.
pub fn as_the_window<T>(
    host: &HostScope,
    exe: &Path,
    config_root: &Path,
    then: impl FnOnce() -> T,
) -> T {
    let window = Window {
        exe: exe.to_path_buf(),
        config_root: config_root.to_path_buf(),
        refresher: std::sync::Arc::new(Refresher::new(host, config_root)),
    };
    within(Some(&window), then)
}

/// The scope this thread runs under ([`as_the_window`]), for a command that hands work to threads
/// of its own, as [`crate::worktree::git::isolation`] is. A chat's is never the window's:
/// [`for_the_window`] asks that of the thread that runs the git.
pub fn held() -> Option<Window> {
    WINDOW.with(|held| held.borrow().clone())
}

/// Runs `then` with this thread in `window`'s scope, where there is one.
pub fn within<T>(window: Option<&Window>, then: impl FnOnce() -> T) -> T {
    struct Restore(Option<Window>);
    impl Drop for Restore {
        fn drop(&mut self) {
            let before = self.0.take();
            WINDOW.with(|held| *held.borrow_mut() = before);
        }
    }
    let Some(window) = window else {
        return then();
    };
    let before = WINDOW.with(|held| held.borrow_mut().replace(window.clone()));
    let _restore = Restore(before);
    then()
}

/// Whether the git this thread runs is the window's, and no chat's.
pub fn in_the_window() -> bool {
    git::isolation().is_none() && held().is_some()
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
    if !still_signed_in(&window.refresher, &account) {
        return None;
    }
    helper_command(&window.exe, &window.config_root, &account.id)
}

/// Whether `account` is still signed in once its token was renewed, if it was about to expire:
/// the window is the one process that refreshes, so it does before git starts the helper. A
/// pasted or imported token is never read here. `false` only when the forge ended the sign-in;
/// a forge that could not be reached leaves the helper to answer with what the item holds.
fn still_signed_in(refresher: &Refresher, account: &ForgeAccount) -> bool {
    if !matches!(account.method, Method::Device | Method::Pkce) {
        return true;
    }
    match refresher.token(account) {
        Ok(_) => true,
        Err(why) => *why.failure() != crate::forge::Failure::Auth,
    }
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

/// The file in purlis's private folder of the config home that holds, for each clone the window
/// cloned or saved, the remote it pushes to: `{ "<clone's key>": "<host>/<owner>/<repo>" }`,
/// keyed by [`crate::sandbox::local::clone_key`].
/// Machine tier. A chat cannot write the config home.
pub const PUSH_REMOTES: &str = "push-remotes.json";

/// What an automatic push answers when the clone's remote is not the one recorded for it.
pub const REMOTE_CHANGED: &str = "the remote of this repository changed since purlis last saved \
    it, so the automatic save did not push. Confirm it in the window: save it from there";

/// What an automatic push answers from a clone purlis has no remote recorded for, on a machine
/// with no forge CLI to push it with: the person saves it once from the window, which confirms
/// the remote and records it (#1848).
pub const NOT_SAVED_FROM_THE_WINDOW: &str = "purlis has not saved this repository from the \
    window yet, so the automatic save did not push. Save it once from the window: from then on \
    automatic saves push with your purlis sign-in";

/// What a push answers from a clone whose remote pushes to another address than it fetches
/// from: purlis signs in for one remote, not two (#1851).
pub const REMOTE_SPLIT: &str = "this repository's remote pushes to another address than the \
    one it fetches from, so purlis did not push it with your sign-in. Give origin one address \
    (or keep the one purlis saved to, in the window), then save it from the window";

/// What a fetch answers from a clone whose remote is not the one recorded for it (#1851).
pub const FETCH_REMOTE_CHANGED: &str = "the remote of this repository changed since purlis last \
    saved it, so purlis did not fetch from it. Confirm it in the window: save it from there";

/// The credential helper a fetch or pull of `clone` by its remote's name gets (#1851): held to
/// the remote recorded for it as a push is ([`helper_for_push`]). Where the window's git would
/// get purlis's own helper, it gets it only when every address the clone's remote names, its
/// push URL too, is the recorded one; with another, `Err` with [`FETCH_REMOTE_CHANGED`] and
/// nothing is fetched. With none recorded it gets the forge CLI's helper, as before purlis had
/// its own: a fetch never trusts a remote on first use either. Elsewhere it is
/// [`super::helper_for`] as it is.
pub fn helper_for_fetch(forge: &super::Forge, clone: &Path) -> Result<String, &'static str> {
    helper_for_fetch_with(forge, clone, remote::places_now(clone).as_deref())
}

/// [`helper_for_fetch`], with the addresses the clone's remote names now given as `places`
/// (`None` when git could not say).
pub(crate) fn helper_for_fetch_with(
    forge: &super::Forge,
    clone: &Path,
    places: Option<&[String]>,
) -> Result<String, &'static str> {
    let Some(own) = for_the_window(&forge.host) else {
        return Ok(super::helper_for(forge));
    };
    let Some(window) = WINDOW.with(|held| held.borrow().clone()) else {
        return Ok(super::helper_for(forge));
    };
    let key = crate::sandbox::local::clone_key(clone);
    match read_push_remotes(&window.config_root).get(&key) {
        None => Ok(super::cli_helper_for(forge)),
        Some(held) => match places {
            Some(places) if !places.is_empty() && places.iter().all(|named| named == held) => {
                Ok(own)
            }
            _ => Err(FETCH_REMOTE_CHANGED),
        },
    }
}

thread_local! {
    static CONFIRMED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Runs `then` with the pushes it makes on this thread confirmed by the person (the window's own
/// Save), when `confirmed`: each records the remote it pushes to ([`helper_for_push`]).
pub fn confirmed_if<T>(confirmed: bool, then: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            CONFIRMED.with(|held| held.set(self.0));
        }
    }
    let before = CONFIRMED.with(|held| held.replace(confirmed || held.get()));
    let _restore = Restore(before);
    then()
}

/// The credential helper a save's push from `clone` to `url` gets.
///
/// Where the window's git would get purlis's own helper ([`for_the_window`]), the helper is held
/// to the remote recorded for the clone in the config home ([`PUSH_REMOTES`]). A push the person
/// confirmed ([`confirmed_if`]) records `url`. Any other push gets the helper only when `url` is
/// the one recorded. With a different one recorded it gets `Err` with [`REMOTE_CHANGED`] and
/// pushes nothing. With none recorded yet (a clone from before the record, or made outside the
/// window) it gets the forge CLI's helper, as before purlis had its own: an automatic push never
/// records, so a remote a chat rewrote is never trusted on first use. On a machine with no such
/// CLI it gets `Err` with [`NOT_SAVED_FROM_THE_WINDOW`] rather than a helper that is not there.
/// Elsewhere it is [`super::helper_for`] as it is.
pub fn helper_for_push(
    forge: &super::Forge,
    clone: &Path,
    url: &str,
) -> Result<String, &'static str> {
    helper_for_push_with(
        forge,
        clone,
        url,
        &|kind| super::find_cli(kind.cli()).is_some(),
        remote::places_now(clone).as_deref(),
    )
}

/// [`helper_for_push`], asking `has_cli` whether the forge CLI of a kind is on this machine.
pub(crate) fn helper_for_push_with(
    forge: &super::Forge,
    clone: &Path,
    url: &str,
    has_cli: &dyn Fn(Kind) -> bool,
    places: Option<&[String]>,
) -> Result<String, &'static str> {
    let Some(own) = for_the_window(&forge.host) else {
        return Ok(super::helper_for(forge));
    };
    let Some(window) = WINDOW.with(|held| held.borrow().clone()) else {
        return Ok(super::helper_for(forge));
    };
    let Some(place) = place_of(url) else {
        return Err(REMOTE_CHANGED);
    };
    // Every address the clone's remote names, its push URL too, is the one pushed to: a remote
    // that would push by name somewhere else is never signed in for (#1851).
    if places.is_some_and(|places| places.iter().any(|named| *named != place)) {
        return Err(REMOTE_SPLIT);
    }
    // #1808's key: the real path and the `.git`'s identity, so another repository cloned into
    // the same folder later is another clone, with no record.
    let key = crate::sandbox::local::clone_key(clone);
    let mut recorded = read_push_remotes(&window.config_root);
    let confirmed = CONFIRMED.with(std::cell::Cell::get);
    match recorded.get(&key) {
        Some(held) if *held == place => Ok(own),
        _ if confirmed => {
            recorded.insert(key, place);
            // A record that cannot be written is not a reason to refuse the push the person
            // asked for; their next Save records it again.
            let _ = write_push_remotes(&window.config_root, &recorded);
            Ok(own)
        }
        Some(_) => Err(REMOTE_CHANGED),
        None if !has_cli(forge.kind) => Err(NOT_SAVED_FROM_THE_WINDOW),
        None => Ok(super::cli_helper_for(forge)),
    }
}

/// Records `url` as the remote of `clone`, for a clone the window just made. Nothing outside the
/// window's git.
pub fn record_push_remote(clone: &Path, url: &str) {
    if git::isolation().is_some() {
        return;
    }
    let (Some(window), Some(place)) = (WINDOW.with(|held| held.borrow().clone()), place_of(url))
    else {
        return;
    };
    // #1808's key: the real path and the `.git`'s identity, so another repository cloned into
    // the same folder later is another clone, with no record.
    let key = crate::sandbox::local::clone_key(clone);
    let mut recorded = read_push_remotes(&window.config_root);
    recorded.insert(key, place);
    let _ = write_push_remotes(&window.config_root, &recorded);
}

/// `<host>/<owner>/<repo>` of a remote's URL, lowercased, without a user, a trailing `/` or
/// `.git`. An HTTPS URL keeps its port, which names another server. An SSH one, `ssh://` or git's
/// short `[user@]host:path`, drops its port, which is SSH's: a save pushes over HTTPS.
pub(crate) fn place_of(url: &str) -> Option<String> {
    let (host, path) = match url.split_once("://") {
        Some((scheme, rest)) => {
            let (authority, path) = rest.split_once('/')?;
            let host = authority
                .rsplit_once('@')
                .map_or(authority, |(_, host)| host);
            if scheme.eq_ignore_ascii_case("ssh") {
                (host.split(':').next().unwrap_or(host), path)
            } else {
                (host, path)
            }
        }
        None => {
            // git's short SSH spelling: a colon before any slash.
            let (authority, path) = url.split_once(':')?;
            if authority.contains('/') {
                return None;
            }
            let host = authority
                .rsplit_once('@')
                .map_or(authority, |(_, host)| host);
            (host, path.trim_start_matches('/'))
        }
    };
    let path = path.trim_end_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    if host.is_empty() || path.is_empty() {
        return None;
    }
    Some(format!("{host}/{path}").to_ascii_lowercase())
}

/// The most [`PUSH_REMOTES`] may be: a line per clone the window saved.
const PUSH_REMOTES_MOST: u64 = 1 << 20;

/// The recorded remotes, or none when the file is missing, unreadable or not a record: a push
/// then gets no purlis helper until the person saves from the window.
pub(crate) fn read_push_remotes(config_root: &Path) -> std::collections::BTreeMap<String, String> {
    crate::machine::read_beside(
        config_root,
        PUSH_REMOTES,
        PUSH_REMOTES_MOST,
        "the remotes the window saved to",
    )
    .ok()
    .flatten()
    .and_then(|text| serde_json::from_str(&text).ok())
    .unwrap_or_default()
}

/// Writes the record into purlis's own folder of the config home, private to the person.
pub(crate) fn write_push_remotes(
    config_root: &Path,
    recorded: &std::collections::BTreeMap<String, String>,
) -> std::io::Result<()> {
    let dir = crate::machine::private_dir(config_root)?;
    let text = serde_json::to_string_pretty(recorded).map_err(std::io::Error::other)? + "\n";
    crate::machine::write_beside(config_root, &dir.join(PUSH_REMOTES), text.as_bytes())
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
    // From the file alone: the read below says whether the item is there, so asking the
    // keychain that first would be a second question for one answer (#2656).
    let account = accounts::listed(&named.config_root)
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
