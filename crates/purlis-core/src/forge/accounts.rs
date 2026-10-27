// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! This machine's forge accounts (ADR 0077 §6, §7 and §8): which accounts there are, signing in
//! to one with a token on any host, and signing out.
//!
//! **Two stores, and the token is in one of them only.**
//!
//! - `<config>/forge-accounts.json` ([`FILE`], Machine tier, device-bound) lists the accounts:
//!   each one's id (a ULID), forge, host, login, how it was signed in, whether it is signed in,
//!   and the scopes its token had when it was checked. Nothing secret.
//! - The token is a keyring item of its own, service `purlis/@forge/<host>/<id>` ([`service`]),
//!   never a vault entry: a vault entry can be asked for through an approval, and a forge token
//!   must not be reachable by any approval (ADR 0077 §7, D-0077b). The `@` marks the item as
//!   purlis's own, as `purlis/@identity/…` is, so a vault named `forge` cannot collide with it.
//!
//! **A token sign-in** ([`sign_in_with_token`], ADR 0077 §4) takes the forge, the host and the
//! pasted token. It checks the token with the forge (who it belongs to, and its scopes), refuses
//! one missing a scope purlis needs and names that scope, then writes the keyring item and the
//! account. Signing in again as the same login on the same host replaces the token and keeps the
//! account. **Sign-out** ([`sign_out`]) deletes the keyring item and the account.
//!
//! **A browser sign-in** ([`sign_in_with_grant`], ADR 0077 §3) lands in the same two stores. What
//! the forge granted (the device flow on github.com, [`super::device`]; PKCE on gitlab.com) is
//! checked the same way and kept in the same keyring item, which then holds the access token
//! with its refresh token and expiry, written together in one write. [`token`] answers the
//! access token whichever way the account was signed in, so nothing that sends one need know.
//! Only [`super::refresh`] reads the refresh token, and a refresh the forge refuses leaves the
//! account listed, signed out, with nothing in the keyring ([`signed_out_by_the_forge`]).
//!
//! **Only the host signs in or out.** Each takes a [`HostScope`], which is refused inside a chat's
//! process tree, and the `purlis` binary never claims one (ADR 0077 §3, V22c): a `purlis` a chat
//! runs is part of that chat. The window runs these on the `local-ui` scope.
//!
//! **A token is never in an error.** Every sentence here is built from the host, the forge and the
//! scopes. A forge's own words are passed through [`unechoed`] first, so even a forge that quoted
//! the token back would not have it repeated.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use zeroize::{Zeroize, Zeroizing};

use super::http::{ApiRoot, Http, TokenSource};
use super::oauth::Granted;
use super::route::HostScope;
use super::transport::{Call, Reply, Transport};
use super::{Forge, ForgeError, Kind, host_ok};
use crate::names::KEYCHAIN_PREFIX;
use crate::secrets::keyring::{FileStore, OsStore, Presence, STUB_FILE, Store};

mod memory;
use memory::Remembering;

/// The accounts file, in purlis's directory of the config home (ADR 0077 §8).
pub const FILE: &str = "forge-accounts.json";

/// The version [`FILE`] is written at.
const VERSION: u64 = 1;

/// The most [`FILE`] may be. A machine has a handful of accounts.
const MOST_BYTES: u64 = 1 << 20;

/// The keyring item's account field: the token. A browser sign-in keeps its pair under the same
/// item (ADR 0077 §3, one write): see [`Pair`].
const ITEM_ACCOUNT: &str = "token";

/// Where [`FILE`] is under the config home `config_root`.
pub fn path(config_root: &Path) -> PathBuf {
    crate::machine::dir(config_root).join(FILE)
}

/// How an account was signed in (ADR 0077 §6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Method {
    Device,
    Pkce,
    Pat,
    Import,
}

/// Whether an account has a token purlis can use (ADR 0077 §6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum State {
    SignedIn,
    SignedOut,
}

/// One forge account, as [`FILE`] keeps it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForgeAccount {
    /// A ULID, minted when the account was added: what its keyring item is named by.
    pub id: String,
    pub kind: Kind,
    /// Lower-case, with a port when it has one.
    pub host: String,
    /// The forge login the token belongs to.
    pub login: String,
    pub method: Method,
    pub state: State,
    /// The token's scopes when it was checked, or `None` when the forge does not say (a GitHub
    /// fine-grained token, whose permissions are set on GitHub).
    pub scopes: Option<Vec<String>>,
}

/// The keyring service an account's token is kept under: `purlis/@forge/<host>/<id>`.
pub fn service(account: &ForgeAccount) -> String {
    service_of(&account.host, &account.id)
}

fn service_of(host: &str, id: &str) -> String {
    format!("{}@forge/{host}/{id}", KEYCHAIN_PREFIX.write)
}

/// The keyring this build keeps forge tokens in: the system's, or, in a fenced build (every test
/// build and the app's e2e build), a stub file in purlis's directory of the config home, so no
/// test reaches the person's keychain.
///
/// **Each process asks it about an item once** (#2656): every store this hands out in one
/// process shares that process's memory of what it read and wrote ([`memory`]), so a listing
/// or a request answered once is answered from memory for as long as the process runs.
pub fn keyring_at(config_root: &Path) -> Box<dyn Store> {
    let remembering = if crate::fence::FENCED {
        let stub = crate::machine::dir(config_root).join(STUB_FILE);
        Remembering::of(&stub, true, || Box::new(FileStore::at(stub.clone())))
    } else {
        // On macOS asking whether an item is there is a Keychain search, a question of its
        // own: a read answers it and keeps the value. Elsewhere the store does not search.
        Remembering::of(Path::new(""), cfg!(target_os = "macos"), || {
            Box::new(OsStore)
        })
    };
    Box::new(remembering)
}

// ---------------------------------------------------------------------------------------
// What a token needs.

/// The scopes a token needs on `kind`, in the words the forge's token page uses.
///
/// GitHub (a classic token): `repo`, to read, create and push to repositories and work on their
/// pull requests and issues; `read:org`, to list the organisations' repositories. GitLab: `api`
/// alone, the only scope that lets purlis write (ADR 0077 §2), which covers git over HTTPS too.
pub fn needed(kind: Kind) -> &'static [&'static str] {
    match kind {
        Kind::GitHub => &["repo", "read:org"],
        Kind::GitLab => &["api"],
    }
}

/// The scopes that also grant `scope` on `kind`: GitHub's organisation scopes are nested.
fn granted_by(kind: Kind, scope: &str) -> &'static [&'static str] {
    match (kind, scope) {
        (Kind::GitHub, "read:org") => &["read:org", "write:org", "admin:org"],
        (Kind::GitHub, "repo") => &["repo"],
        (Kind::GitLab, "api") => &["api"],
        _ => &[],
    }
}

/// The needed scopes `scopes` lacks, in [`needed`]'s order.
pub fn missing(kind: Kind, scopes: &[String]) -> Vec<&'static str> {
    needed(kind)
        .iter()
        .copied()
        .filter(|want| {
            !granted_by(kind, want)
                .iter()
                .any(|by| scopes.iter().any(|have| have == by))
        })
        .collect()
}

/// The scopes of `scopes` beyond what purlis needs (ADR 0077 §2: listed, and marked wider).
pub fn wider(kind: Kind, scopes: &[String]) -> Vec<String> {
    scopes
        .iter()
        .filter(|have| !needed(kind).contains(&have.as_str()))
        .cloned()
        .collect()
}

/// What the sign-in page shows for a host before a token is pasted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenHelp {
    /// The host as purlis will use it.
    pub host: String,
    /// The scopes to tick ([`needed`]).
    pub scopes: &'static [&'static str],
    /// The forge's own page that makes a token, with those scopes filled in where it takes them.
    pub create_at: String,
}

/// The scopes `kind` needs and where a token is made on `typed`, or why `typed` is no host.
pub fn token_help(kind: Kind, typed: &str) -> Result<TokenHelp, String> {
    let host = host_named(kind, typed)?;
    let create_at = match kind {
        Kind::GitHub => format!(
            "https://{host}/settings/tokens/new?description=purlis&scopes={}",
            needed(kind).join(",")
        ),
        Kind::GitLab => format!(
            "https://{host}/-/user_settings/personal_access_tokens?name=purlis&scopes={}",
            needed(kind).join(",")
        ),
    };
    Ok(TokenHelp {
        host,
        scopes: needed(kind),
        create_at,
    })
}

/// `typed` as the host a sign-in on `kind` uses: trimmed, lower-case, an `https://` and a
/// trailing `/` taken off. Refused, in a sentence that says what is wrong, when it is not a host
/// name, when it asks for plain HTTP, or when it is the other forge's own host.
///
/// **What was typed is never repeated** in the refusal: a person who pasted the token into the
/// host box would otherwise see it in an error.
pub fn host_named(kind: Kind, typed: &str) -> Result<String, String> {
    let trimmed = typed.trim();
    if trimmed.is_empty() {
        return Err(format!(
            "Enter the {} host, like {}.",
            forge_name(kind),
            kind.default_host()
        ));
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("http://") {
        return Err(
            "purlis signs in over HTTPS only. Enter the host without http://, like \
             git.example.com."
                .to_string(),
        );
    }
    let bare = lower.strip_prefix("https://").unwrap_or(&lower);
    let bare = bare.strip_suffix('/').unwrap_or(bare);
    // A full host name, with a dot: a single word is far more often a token pasted into the
    // wrong box than an intranet host, and a token must never be looked up as a host name.
    let named = bare.rsplit_once(':').map_or(bare, |(name, _)| name);
    if !host_ok(bare) || !named.contains('.') {
        return Err(
            "That is not a host name. Enter only the forge's host, like git.example.com, or \
             git.example.com:8443 with a port."
                .to_string(),
        );
    }
    let other = match kind {
        Kind::GitHub => Kind::GitLab,
        Kind::GitLab => Kind::GitHub,
    };
    if bare == other.default_host() {
        return Err(format!(
            "{bare} is {}'s own host, not a {} one. Choose {} instead.",
            forge_name(other),
            forge_name(kind),
            forge_name(other)
        ));
    }
    Ok(bare.to_string())
}

/// How a forge is named in a sentence.
fn forge_name(kind: Kind) -> &'static str {
    match kind {
        Kind::GitHub => "GitHub",
        Kind::GitLab => "GitLab",
    }
}

// ---------------------------------------------------------------------------------------
// The token, pasted.

/// A token the person pasted: held as a [`SecretString`] from the moment it is taken, and never
/// printed by `Debug`.
pub struct Pasted(SecretString);

impl Pasted {
    /// Takes `text`, wiping the `String` it came in: only the secret's own copy is left.
    /// Nothing here writes it anywhere git could later hand to a credential store: it goes to
    /// purlis's own keyring item alone.
    pub fn new(mut text: String) -> Pasted {
        let held = SecretString::from(text.trim());
        text.zeroize();
        Pasted(held)
    }

    pub(super) fn is_empty(&self) -> bool {
        self.0.expose_secret().is_empty()
    }
}

impl std::fmt::Debug for Pasted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Pasted(***)")
    }
}

/// The token being checked, handed to the transport per request.
struct Checking(SecretString);

impl TokenSource for Checking {
    fn token(&self) -> Result<SecretString, ForgeError> {
        Ok(self.0.clone())
    }
}

/// Builds the transport a token is checked over: the forge at `forge`, sending `tokens`.
pub type Reach<'a> =
    &'a dyn Fn(&Forge, Arc<dyn TokenSource>) -> Result<Arc<dyn Transport>, ForgeError>;

/// The production [`Reach`]: the native transport to the forge's own API, over HTTPS.
pub fn over_https(
    forge: &Forge,
    tokens: Arc<dyn TokenSource>,
) -> Result<Arc<dyn Transport>, ForgeError> {
    Ok(Arc::new(Http::new(
        forge.kind,
        ApiRoot::of(forge.kind, &forge.host),
        tokens,
    )?))
}

/// A [`Reach`] to a recorded forge at `root`. Only a test build has it.
#[cfg(any(test, feature = "fenced"))]
pub fn over_root(
    root: ApiRoot,
) -> impl Fn(&Forge, Arc<dyn TokenSource>) -> Result<Arc<dyn Transport>, ForgeError> {
    move |forge, tokens| Ok(Arc::new(Http::new(forge.kind, root.clone(), tokens)?))
}

/// `said` with the token taken out, wherever it is in it, and every other token this process
/// holds ([`crate::applog::keep_out`]).
pub(super) fn unechoed(said: String, token: &SecretString) -> String {
    let said = crate::applog::without_values_kept_out(&said);
    let token = token.expose_secret();
    if token.is_empty() || !said.contains(token) {
        return said;
    }
    said.replace(token, "[the token]")
}

// ---------------------------------------------------------------------------------------
// Checking a token with its forge.

/// Who a token belongs to, and its scopes (`None` when the forge does not say).
struct Checked {
    login: String,
    scopes: Option<Vec<String>>,
}

/// The token page's address, for a sentence that sends the person there.
fn make_one_at(kind: Kind, host: &str) -> String {
    token_help(kind, host)
        .map(|help| help.create_at)
        .unwrap_or_else(|_| format!("https://{host}"))
}

/// Why `reply`, a refusal, refused the token: in purlis's words, by its status.
fn refused(kind: Kind, host: &str, reply: &Reply) -> String {
    match reply.status {
        Some(401) => format!(
            "{host} refused this token: it is mistyped, expired or revoked. Make a new one at {}.",
            make_one_at(kind, host)
        ),
        Some(403) => format!(
            "{host} would not let this token read your account ({}). Check the token's scopes, \
             and that your organisation allows it.",
            reply.said(kind)
        ),
        Some(404) if kind == Kind::GitHub && host != kind.default_host() => format!(
            "{host} did not answer as a GitHub Enterprise server. Check the host, and choose \
             GitLab if it is a GitLab."
        ),
        Some(404) => format!(
            "{host} did not answer as a {}. Check the host.",
            forge_name(kind)
        ),
        _ => format!("{host} could not check this token: {}", reply.said(kind)),
    }
}

/// `call` sent to `forge`, its answer when the forge answered it with a success, or why not.
fn asked(transport: &dyn Transport, forge: &Forge, call: &Call) -> Result<Reply, String> {
    let reply = transport.send(forge, call).map_err(|no| {
        format!(
            "purlis could not reach {}: {}. Check the host and your connection.",
            forge.host,
            no.said()
        )
    })?;
    if reply.ok() {
        Ok(reply)
    } else {
        Err(refused(forge.kind, &forge.host, &reply))
    }
}

/// The answer read as a JSON object, or a sentence saying the host is not that forge.
fn object(forge: &Forge, reply: &Reply) -> Result<Map<String, Value>, String> {
    match serde_json::from_str::<Value>(&reply.out) {
        Ok(Value::Object(map)) => Ok(map),
        _ => Err(format!(
            "{} did not answer as a {} would. Check the host, and the forge chosen for it.",
            forge.host,
            forge_name(forge.kind)
        )),
    }
}

/// Who the token at `transport` is, and its scopes.
///
/// GitHub: `GET /user`, with the classic token's scopes in its `X-OAuth-Scopes` header (absent
/// for a fine-grained token). GitLab: `GET /user`, then the token's own record,
/// `GET /personal_access_tokens/self`, for its scopes and whether it is still active.
///
/// A token a browser sign-in was `granted` is no personal access token, so GitLab has no such
/// record of it: its scopes are the grant's own, which the caller holds.
fn check(transport: &dyn Transport, forge: &Forge, granted: bool) -> Result<Checked, String> {
    let user = asked(transport, forge, &Call::get("user", super::STATUS_TIMEOUT))?;
    let me = object(forge, &user)?;
    let login_key = match forge.kind {
        Kind::GitHub => "login",
        Kind::GitLab => "username",
    };
    let login = me
        .get(login_key)
        .and_then(Value::as_str)
        .filter(|login| !login.is_empty() && !login.chars().any(char::is_control))
        .ok_or_else(|| {
            format!(
                "{} answered without saying whose token this is. Check the host.",
                forge.host
            )
        })?
        .to_string();
    let scopes = match forge.kind {
        Kind::GitHub => user.header("x-oauth-scopes").map(|listed| {
            listed
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect()
        }),
        Kind::GitLab if granted => None,
        Kind::GitLab => {
            let call = Call::get("personal_access_tokens/self", super::STATUS_TIMEOUT);
            let token = asked(transport, forge, &call).map_err(|why| {
                format!(
                    "{why} purlis reads a GitLab token's scopes from its own record, so it takes \
                     a personal access token."
                )
            })?;
            let token = object(forge, &token)?;
            if token.get("active").and_then(Value::as_bool) == Some(false)
                || token.get("revoked").and_then(Value::as_bool) == Some(true)
            {
                return Err(format!(
                    "{} says this token has expired or was revoked. Make a new one at {}.",
                    forge.host,
                    make_one_at(forge.kind, &forge.host)
                ));
            }
            Some(
                token
                    .get("scopes")
                    .and_then(Value::as_array)
                    .map(|all| {
                        all.iter()
                            .filter_map(Value::as_str)
                            .map(str::to_string)
                            .collect()
                    })
                    .unwrap_or_default(),
            )
        }
    };
    Ok(Checked { login, scopes })
}

/// The sentence refusing a token that lacks `missing`.
fn lacks(kind: Kind, host: &str, missing: &[&str]) -> String {
    let named = match missing {
        [one] => format!("the {one} scope"),
        many => format!("the {} scopes", many.join(" and ")),
    };
    format!(
        "This token is missing {named}. purlis needs {} on {host}: add {} to the token, or make \
         a new one at {}.",
        needed(kind).join(" and "),
        if missing.len() == 1 { "it" } else { "them" },
        make_one_at(kind, host)
    )
}

/// The sentence refusing a browser sign-in the forge granted without `missing`.
fn lacks_granted(kind: Kind, host: &str, missing: &[&str]) -> String {
    let named = match missing {
        [one] => format!("the {one} scope"),
        many => format!("the {} scopes", many.join(" and ")),
    };
    format!(
        "{host} signed you in without {named}, and purlis needs {}. Sign in again and approve \
         what {} asks for, or sign in with a token instead.",
        needed(kind).join(" and "),
        forge_name(kind)
    )
}

// ---------------------------------------------------------------------------------------
// What the keyring item holds.

/// What a browser sign-in's keyring item holds, as JSON: the access token, and the refresh
/// token and the access token's expiry where the forge gave them (ADR 0077 §8). A pasted or
/// imported token's item holds the token alone, as text.
#[derive(Serialize, Deserialize)]
struct Pair {
    access: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    refresh: Option<String>,
    /// When the access token expires, in seconds since the epoch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    expires_at: Option<u64>,
}

impl Drop for Pair {
    fn drop(&mut self) {
        self.access.zeroize();
        if let Some(refresh) = &mut self.refresh {
            refresh.zeroize();
        }
    }
}

/// An account's keyring item, read: its access token, and what a refresh needs.
pub struct Kept {
    pub access: SecretString,
    /// The refresh token, for an account a browser sign-in made on a forge whose tokens expire.
    pub refresh: Option<SecretString>,
    /// When the access token expires, in seconds since the epoch, when it does.
    pub expires_at: Option<u64>,
}

impl std::fmt::Debug for Kept {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Kept")
            .field("expires_at", &self.expires_at)
            .finish_non_exhaustive()
    }
}

/// Whether `method` is a browser sign-in, whose item holds a [`Pair`].
fn granted_by_a_flow(method: Method) -> bool {
    matches!(method, Method::Device | Method::Pkce)
}

/// Now, in seconds since the epoch.
pub(super) fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

/// The item text for what was `granted` at `now`: one value, so one write. `refresh_else` is
/// the refresh token kept when the forge granted no new one.
fn pair_of(granted: &Granted, refresh_else: Option<&SecretString>, now: u64) -> Zeroizing<String> {
    let pair = Pair {
        access: granted.access.expose_secret().to_string(),
        refresh: granted
            .refresh
            .as_ref()
            .or(refresh_else)
            .map(|refresh| refresh.expose_secret().to_string()),
        expires_at: granted
            .expires_in
            .map(|lasts| now.saturating_add(lasts.as_secs())),
    };
    Zeroizing::new(serde_json::to_string(&pair).unwrap_or_default())
}

// ---------------------------------------------------------------------------------------
// The accounts file.

/// One process writes the file at a time: a sign-in and a sign-out never lose each other's
/// change.
static WRITING: Mutex<()> = Mutex::new(());

/// The file as read: every account as it was written, so a field a later purlis adds, and an
/// account this one cannot read, survive a write by this one.
#[derive(Default, Serialize, Deserialize)]
struct OnDisk {
    #[serde(default)]
    version: u64,
    #[serde(default)]
    accounts: Vec<Value>,
    #[serde(flatten)]
    rest: Map<String, Value>,
}

/// An account as its JSON object has it.
#[derive(Serialize, Deserialize)]
struct Written {
    id: String,
    kind: String,
    host: String,
    login: String,
    method: Method,
    state: State,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    scopes: Option<Vec<String>>,
    #[serde(flatten)]
    rest: Map<String, Value>,
}

/// The account `value` holds, or `None` when it is not one this purlis can use: an unknown
/// forge, a host that is not a host name, or an id that could not name a keyring item.
fn account_of(value: &Value) -> Option<ForgeAccount> {
    let written: Written = serde_json::from_value(value.clone()).ok()?;
    let kind = Kind::parse(&written.kind)?;
    let id_ok = !written.id.is_empty() && written.id.bytes().all(|b| b.is_ascii_alphanumeric());
    if !id_ok || !host_ok(&written.host) || written.host != written.host.to_ascii_lowercase() {
        return None;
    }
    Some(ForgeAccount {
        id: written.id,
        kind,
        host: written.host,
        login: written.login,
        method: written.method,
        state: written.state,
        scopes: written.scopes,
    })
}

fn written(account: &ForgeAccount, rest: Map<String, Value>) -> Value {
    serde_json::to_value(Written {
        id: account.id.clone(),
        kind: account.kind.word().to_string(),
        host: account.host.clone(),
        login: account.login.clone(),
        method: account.method,
        state: account.state,
        scopes: account.scopes.clone(),
        rest,
    })
    .unwrap_or(Value::Null)
}

/// The file, or an empty one when there is none yet. One that cannot be read is an error, never
/// an empty list: writing over it would lose every account in it.
fn read(config_root: &Path) -> Result<OnDisk, String> {
    let at = path(config_root);
    let file = match std::fs::File::open(&at) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(OnDisk::default()),
        Err(e) => return Err(format!("purlis cannot read {}: {e}", at.display())),
    };
    let mut text = String::new();
    std::io::Read::read_to_string(&mut std::io::Read::take(file, MOST_BYTES + 1), &mut text)
        .map_err(|e| format!("purlis cannot read {}: {e}", at.display()))?;
    if text.len() as u64 > MOST_BYTES {
        return Err(format!(
            "{} is larger than a list of accounts can be",
            at.display()
        ));
    }
    serde_json::from_str(&text)
        .map_err(|e| format!("{} is not a list of forge accounts: {e}", at.display()))
}

fn write(config_root: &Path, mut file: OnDisk) -> Result<(), String> {
    file.version = VERSION;
    let at = path(config_root);
    let dir = crate::machine::private_dir(config_root)
        .map_err(|e| format!("purlis cannot write {}: {e}", at.display()))?;
    let text = serde_json::to_string_pretty(&file).unwrap_or_default() + "\n";
    crate::machine::write_beside(config_root, &dir.join(FILE), text.as_bytes())
        .map_err(|e| format!("purlis cannot write {}: {e}", at.display()))
}

// ---------------------------------------------------------------------------------------
// The three acts.

/// This machine's accounts, in the order they were added. An account whose keyring item is
/// provably gone is listed signed out.
pub fn list(config_root: &Path, keyring: &dyn Store) -> Result<Vec<ForgeAccount>, String> {
    Ok(read(config_root)?
        .accounts
        .iter()
        .filter_map(account_of)
        .map(|mut account| {
            if keyring.exists(&service(&account), ITEM_ACCOUNT) == Presence::Absent {
                account.state = State::SignedOut;
            }
            account
        })
        .collect())
}

/// This machine's accounts as the accounts file lists them, without asking the keyring whether
/// each item is there: for a caller about to read the token anyway, whose read says that itself
/// (#2656). An account whose item is gone is listed as the file says.
pub fn listed(config_root: &Path) -> Result<Vec<ForgeAccount>, String> {
    Ok(read(config_root)?
        .accounts
        .iter()
        .filter_map(account_of)
        .collect())
}

/// The account the person in the window is signed in to `kind` on `host` as: the first signed-in
/// account on that host, in the order they were added. Several accounts on one host are told
/// apart by repo bindings (ADR 0077 §6), which are not written yet; until they are, the first
/// one is used. `None` when no account on the host is signed in.
pub fn signed_in_on(
    config_root: &Path,
    kind: Kind,
    host: &str,
) -> Result<Option<ForgeAccount>, String> {
    Ok(every_signed_in_on(config_root, kind, host)?
        .into_iter()
        .next())
}

/// Every account the person is signed in to `kind` on `host` as, in the order they were added:
/// what the window may ask that host as. [`signed_in_on`] is the first of them.
pub fn every_signed_in_on(
    config_root: &Path,
    kind: Kind,
    host: &str,
) -> Result<Vec<ForgeAccount>, String> {
    Ok(read(config_root)?
        .accounts
        .iter()
        .filter_map(account_of)
        .filter(|a| {
            a.kind == kind && a.state == State::SignedIn && a.host.eq_ignore_ascii_case(host)
        })
        .collect())
}

/// Whether a forge CLI's login was imported into an account on `host` (ADR 0077 §5): from then
/// on that login is the person's, and a chat on that host gets no forge credential from purlis
/// (ADR 0070 §4, V22c). An accounts file that cannot be read answers no, as it holds no sign-in
/// for the window either (D-1816-4): a chat keeps the CLI's login, as it had before any import.
pub fn imported_on(config_root: &Path, kind: Kind, host: &str) -> bool {
    read(config_root).is_ok_and(|file| {
        file.accounts.iter().filter_map(account_of).any(|a| {
            a.kind == kind
                && a.method == Method::Import
                && a.state == State::SignedIn
                && a.host.eq_ignore_ascii_case(host)
        })
    })
}

/// `account`'s token, read from its own keyring item once per process (ADR 0070 §4 as amended
/// 2026-10-18, [`keyring_at`]) and handed out for one request, or `None`
/// when the item is provably gone: the access token, whichever way the account was signed in.
/// The `String` the keyring answered with is wiped before this returns, so the only copy left
/// is the `SecretString`, and the value is registered with [`crate::applog::keep_out`] so no log
/// line or error carries it.
pub fn token(keyring: &dyn Store, account: &ForgeAccount) -> Result<Option<SecretString>, String> {
    Ok(kept(keyring, account)?.map(|kept| kept.access))
}

/// `account`'s keyring item, read whole: [`token`]'s access token, and for a browser sign-in
/// the refresh token and the expiry kept beside it. Every value is wiped and kept out of the
/// log as [`token`] says. An item of a browser sign-in that is not the pair it wrote is an
/// error, never sent anywhere as a token.
pub(super) fn kept(keyring: &dyn Store, account: &ForgeAccount) -> Result<Option<Kept>, String> {
    kept_as_remembered(keyring, account)
}

/// [`kept`], read afresh from the keyring past this process's memory (#2656): what a renewal
/// and a sign-out read, so a pair another process renewed is the one renewed again or revoked,
/// and never the spent one this process remembers.
pub(super) fn kept_afresh(
    keyring: &dyn Store,
    account: &ForgeAccount,
) -> Result<Option<Kept>, String> {
    forget(keyring, account);
    kept_as_remembered(keyring, account)
}

/// Forget what this process remembers of `account`'s item, so the next read asks the keyring:
/// for a token the forge refused (#2656).
pub(super) fn forget(keyring: &dyn Store, account: &ForgeAccount) {
    keyring.forget(&service(account), ITEM_ACCOUNT);
}

fn kept_as_remembered(keyring: &dyn Store, account: &ForgeAccount) -> Result<Option<Kept>, String> {
    let read = keyring.get(&service(account), ITEM_ACCOUNT).map_err(|e| {
        format!(
            "purlis could not read the token for {} on {} from the keychain: {}",
            account.login,
            account.host,
            unechoed(e.message.to_string(), &SecretString::from(""))
        )
    })?;
    let Some(secret) = read else {
        return Ok(None);
    };
    let mut text = secret.into_inner();
    if !granted_by_a_flow(account.method) {
        // A browser sign-in's pair under an account listed as a token's (the keyring was
        // written and the list was not) is never sent anywhere as a token.
        let a_pair =
            text.trim_start().starts_with('{') && serde_json::from_str::<Pair>(&text).is_ok();
        if a_pair {
            text.zeroize();
            return Err(format!(
                "purlis's sign-in to {} as {} cannot be read from the keychain. Sign in again in \
                 Settings › You › Accounts.",
                account.host, account.login
            ));
        }
        let held = SecretString::from(text.as_str());
        text.zeroize();
        crate::applog::keep_out(&held);
        return Ok(Some(Kept {
            access: held,
            refresh: None,
            expires_at: None,
        }));
    }
    let pair = serde_json::from_str::<Pair>(&text);
    text.zeroize();
    let Ok(pair) = pair else {
        return Err(format!(
            "purlis's sign-in to {} as {} cannot be read from the keychain. Sign in again in \
             Settings › You › Accounts.",
            account.host, account.login
        ));
    };
    let access = SecretString::from(pair.access.as_str());
    let refresh = pair.refresh.as_deref().map(SecretString::from);
    crate::applog::keep_out(&access);
    if let Some(refresh) = &refresh {
        crate::applog::keep_out(refresh);
    }
    Ok(Some(Kept {
        access,
        refresh,
        expires_at: pair.expires_at,
    }))
}

/// Sign in to `kind` on the host `typed` with the pasted `token` (ADR 0077 §4): checked with the
/// forge through `reach`, refused when it lacks a scope [`needed`] names, then kept as the
/// account's keyring item and listed in [`FILE`]. The account, as listed.
pub fn sign_in_with_token(
    host: &HostScope,
    config_root: &Path,
    keyring: &dyn Store,
    reach: Reach<'_>,
    kind: Kind,
    typed: &str,
    token: Pasted,
) -> Result<ForgeAccount, String> {
    sign_in(
        host,
        config_root,
        keyring,
        reach,
        kind,
        typed,
        token,
        Method::Pat,
    )
}

/// [`sign_in_with_token`], with the account recorded as signed in by `method`: a pasted token
/// is [`Method::Pat`], a forge CLI's login copied in is [`Method::Import`]
/// ([`super::cliimport`], ADR 0077 §5). Either way the token is checked with the forge first and
/// kept in the account's own keyring item.
#[allow(clippy::too_many_arguments)]
pub(super) fn sign_in(
    _host: &HostScope,
    config_root: &Path,
    keyring: &dyn Store,
    reach: Reach<'_>,
    kind: Kind,
    typed: &str,
    token: Pasted,
    method: Method,
) -> Result<ForgeAccount, String> {
    let host = host_named(kind, typed)?;
    if !token.is_empty() && typed.contains(token.0.expose_secret()) {
        return Err(
            "The host box holds the token. Enter the forge's host there, and the token below it."
                .to_string(),
        );
    }
    if token.is_empty() {
        return Err(format!(
            "Paste a token first. Make one at {}.",
            make_one_at(kind, &host)
        ));
    }
    keep(
        config_root,
        keyring,
        reach,
        kind,
        host,
        Held::Token(token.0),
        method,
    )
}

/// Sign in to `kind` on the host `typed` with what a browser sign-in was `granted` (ADR 0077
/// §3): the device flow ([`Method::Device`]) or PKCE ([`Method::Pkce`]). The forge is asked
/// whose token it is through `reach`, the scopes are the grant's own, and a grant that lacks a
/// scope [`needed`] names is refused. Then the access token, the refresh token and the expiry
/// are kept together in the account's keyring item, in one write, and the account is listed in
/// [`FILE`]: the same account, with the same id, when that login was already signed in on that
/// host. The account, as listed.
///
/// `typed` is the host of the registration the grant came from, and the account is that host's
/// alone: the token is checked with, and only ever sent to, the forge at that host.
#[allow(clippy::too_many_arguments)]
pub fn sign_in_with_grant(
    _host: &HostScope,
    config_root: &Path,
    keyring: &dyn Store,
    reach: Reach<'_>,
    kind: Kind,
    typed: &str,
    granted: Granted,
    method: Method,
) -> Result<ForgeAccount, String> {
    if !granted_by_a_flow(method) {
        return Err("A browser sign-in is by the device flow or by PKCE.".to_string());
    }
    let host = host_named(kind, typed)?;
    if granted.access.expose_secret().is_empty() {
        return Err(format!(
            "{host} signed you in without a token. Sign in again."
        ));
    }
    keep(
        config_root,
        keyring,
        reach,
        kind,
        host,
        Held::Grant(granted),
        method,
    )
}

/// What is being signed in with: a token alone, or what a browser sign-in granted.
enum Held {
    Token(SecretString),
    Grant(Granted),
}

/// Checks what is `held` with the forge and keeps it as the account's keyring item and its
/// line in [`FILE`]: the end of every sign-in.
fn keep(
    config_root: &Path,
    keyring: &dyn Store,
    reach: Reach<'_>,
    kind: Kind,
    host: String,
    held: Held,
    method: Method,
) -> Result<ForgeAccount, String> {
    let secret = match &held {
        Held::Token(token) => token.clone(),
        Held::Grant(granted) => granted.access.clone(),
    };
    // From here on no log line or error carries it, whatever its shape (a GitLab or a
    // self-managed token has no prefix a shape rule knows).
    crate::applog::keep_out(&secret);
    if let Held::Grant(Granted {
        refresh: Some(refresh),
        ..
    }) = &held
    {
        crate::applog::keep_out(refresh);
    }
    let forge = Forge {
        kind,
        host: host.clone(),
    };
    let granted = matches!(held, Held::Grant(_));
    let mut checked = reach(&forge, Arc::new(Checking(secret.clone())))
        .map_err(|e| e.to_string())
        .and_then(|transport| check(transport.as_ref(), &forge, granted))
        .map_err(|why| unechoed(why, &secret))?;
    if let Held::Grant(Granted {
        scopes: Some(scopes),
        ..
    }) = &held
    {
        // The grant's own word on its scopes, where the forge gave one.
        checked.scopes = Some(scopes.clone());
    }
    if let Some(scopes) = &checked.scopes {
        let missing = missing(kind, scopes);
        if !missing.is_empty() {
            return Err(if granted {
                lacks_granted(kind, &host, &missing)
            } else {
                lacks(kind, &host, &missing)
            });
        }
    }
    let item = match &held {
        Held::Token(token) => Zeroizing::new(token.expose_secret().to_string()),
        Held::Grant(granted) => pair_of(granted, None, now()),
    };
    drop(held);

    let _one = WRITING.lock().unwrap_or_else(|e| e.into_inner());
    let mut file = read(config_root)?;
    let same = file.accounts.iter().position(|value| {
        account_of(value)
            .is_some_and(|a| a.kind == kind && a.host == host && a.login == checked.login)
    });
    let (id, rest) = match same.and_then(|at| file.accounts.get(at)) {
        Some(value) => {
            let was: Written = serde_json::from_value(value.clone())
                .map_err(|e| format!("an account in {FILE} cannot be read: {e}"))?;
            (was.id, was.rest)
        }
        None => (ulid::Ulid::generate().to_string(), Map::new()),
    };
    let account = ForgeAccount {
        id,
        kind,
        host,
        login: checked.login,
        method,
        state: State::SignedIn,
        scopes: checked.scopes,
    };
    keyring
        .set(&service(&account), ITEM_ACCOUNT, &item)
        .map_err(|e| unechoed(e.message.to_string(), &secret))?;
    drop(item);
    drop(secret);
    let value = written(&account, rest);
    match same {
        Some(at) => file.accounts[at] = value,
        None => file.accounts.push(value),
    }
    if let Err(why) = write(config_root, file) {
        // A new account's item would otherwise be left with nothing naming it.
        if same.is_none() {
            let _ = keyring.delete(&service(&account), ITEM_ACCOUNT);
        }
        return Err(why);
    }
    Ok(account)
}

/// Sign out of the account `id` (ADR 0077 §3): its keyring item deleted, then the account taken
/// off the list. A token the person pasted is not revoked at the forge: it may be theirs for other
/// uses, and they delete it there. The account as it was.
pub fn sign_out(
    _host: &HostScope,
    config_root: &Path,
    keyring: &dyn Store,
    id: &str,
) -> Result<ForgeAccount, String> {
    let _one = WRITING.lock().unwrap_or_else(|e| e.into_inner());
    let mut file = read(config_root)?;
    let at = file
        .accounts
        .iter()
        .position(|value| account_of(value).is_some_and(|a| a.id == id))
        .ok_or_else(|| "That account is not on this machine any more.".to_string())?;
    let account = account_of(&file.accounts[at])
        .ok_or_else(|| "That account is not on this machine any more.".to_string())?;
    keyring
        .delete(&service(&account), ITEM_ACCOUNT)
        .map_err(|e| {
            format!(
                "purlis could not delete the token from the keychain: {}",
                e.message
            )
        })?;
    file.accounts.remove(at);
    write(config_root, file)?;
    Ok(account)
}

/// Keeps what a refresh `granted` for `account`, at `now`: the new access token, its expiry and
/// the new refresh token replace the old pair in the account's keyring item, in one write (ADR
/// 0077 §3: a refresh rotates the refresh token). `was` is the refresh token that was spent,
/// kept only when the forge granted no new one. Only [`super::refresh`] calls this, in the
/// host's process.
pub(super) fn keep_refreshed(
    _host: &HostScope,
    keyring: &dyn Store,
    account: &ForgeAccount,
    granted: &Granted,
    was: Option<&SecretString>,
    now: u64,
) -> Result<(), String> {
    crate::applog::keep_out(&granted.access);
    if let Some(refresh) = &granted.refresh {
        crate::applog::keep_out(refresh);
    }
    let item = pair_of(granted, was, now);
    keyring
        .set(&service(account), ITEM_ACCOUNT, &item)
        .map_err(|e| {
            format!(
                "purlis could not keep the renewed sign-in in the keychain: {}",
                unechoed(e.message.to_string(), &granted.access)
            )
        })
        .map(|_| ())
}

/// The forge ended the sign-in of the account `id` (ADR 0077 §3: a refresh it refuses): the
/// keyring item is deleted, and the account stays listed, **signed out**, so the window can say
/// which sign-in ended and offer it again. Nothing else of the account changes.
pub(super) fn signed_out_by_the_forge(
    _host: &HostScope,
    config_root: &Path,
    keyring: &dyn Store,
    id: &str,
) -> Result<(), String> {
    let _one = WRITING.lock().unwrap_or_else(|e| e.into_inner());
    let mut file = read(config_root)?;
    let Some(at) = file
        .accounts
        .iter()
        .position(|value| account_of(value).is_some_and(|a| a.id == id))
    else {
        return Ok(());
    };
    let Some(account) = account_of(&file.accounts[at]) else {
        return Ok(());
    };
    // The item first: an account listed signed in must never be left with a token that the
    // forge has ended.
    let _ = keyring.delete(&service(&account), ITEM_ACCOUNT);
    if let Some(Value::Object(fields)) = file.accounts.get_mut(at) {
        fields.insert(
            "state".to_string(),
            serde_json::to_value(State::SignedOut).unwrap_or(Value::Null),
        );
    }
    write(config_root, file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_host_is_trimmed_lowered_and_stripped_of_https_and_a_slash() {
        assert_eq!(
            host_named(Kind::GitLab, "  https://Git.Example.com/ ").as_deref(),
            Ok("git.example.com")
        );
        assert_eq!(
            host_named(Kind::GitHub, "ghe.example.com:8443").as_deref(),
            Ok("ghe.example.com:8443")
        );
    }

    #[test]
    fn a_host_that_is_none_is_refused_without_repeating_what_was_typed() {
        for typed in [
            "",
            "   ",
            "http://git.example.com",
            "git example",
            "a/b",
            "x@y.z",
            "oneword",
            "oneword:8443",
        ] {
            let why = host_named(Kind::GitLab, typed).unwrap_err();
            assert!(why.ends_with('.'), "{why}");
            if !typed.trim().is_empty() {
                assert!(!why.contains(typed.trim()), "{why}");
            }
        }
        assert!(
            host_named(Kind::GitLab, "github.com")
                .unwrap_err()
                .contains("Choose GitHub")
        );
        assert!(
            host_named(Kind::GitHub, "gitlab.com")
                .unwrap_err()
                .contains("Choose GitLab")
        );
    }

    #[test]
    fn a_needed_scope_is_granted_by_itself_or_a_wider_one_of_its_family() {
        let have = |all: &[&str]| all.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(missing(Kind::GitHub, &have(&["repo", "admin:org"])).is_empty());
        assert_eq!(missing(Kind::GitHub, &have(&["repo"])), ["read:org"]);
        assert_eq!(
            missing(Kind::GitLab, &have(&["read_api", "write_repository"])),
            ["api"]
        );
        assert_eq!(
            wider(Kind::GitHub, &have(&["repo", "read:org", "gist"])),
            ["gist"]
        );
    }

    #[test]
    fn the_item_is_purlis_own_under_the_forge_host_and_the_account_id() {
        assert_eq!(
            service_of("git.example.com", "01J9ACCOUNT"),
            "purlis/@forge/git.example.com/01J9ACCOUNT"
        );
    }

    #[test]
    fn a_token_page_is_the_forges_own_with_the_scopes_filled_in() {
        let github = token_help(Kind::GitHub, "github.com").unwrap();
        assert_eq!(
            github.create_at,
            "https://github.com/settings/tokens/new?description=purlis&scopes=repo,read:org"
        );
        let gitlab = token_help(Kind::GitLab, "git.example.com").unwrap();
        assert!(
            gitlab
                .create_at
                .starts_with("https://git.example.com/-/user_settings/personal_access_tokens?"),
            "{}",
            gitlab.create_at
        );
    }

    #[test]
    fn a_pasted_token_never_prints() {
        let pasted = Pasted::new("not-printed-anywhere".to_string());
        assert_eq!(format!("{pasted:?}"), "Pasted(***)");
    }

    #[test]
    fn a_forges_words_that_quote_the_token_lose_it() {
        let token = SecretString::from("quoted-back");
        assert_eq!(
            unechoed("bad token quoted-back here".into(), &token),
            "bad token [the token] here"
        );
    }
}
