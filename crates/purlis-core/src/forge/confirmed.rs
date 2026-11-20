// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The repos the person confirmed, and which of their accounts serves each** (ADR 0128 D4, and
//! D2's *several accounts on one host*).
//!
//! The forge proxy serves a repository only if the person **cloned it from the window, or told
//! purlis to trust the clone** ("Trust this clone"). A clone made from the CLI is outside the list
//! until then (#7196). This file is that list. Each entry records:
//!
//! - **the clone**, by the same key the trust list uses ([`crate::sandbox::local::clone_key`]: its
//!   real path and the identity of its `.git`), so a copy of the repo somewhere else, or another
//!   repository cloned into the same folder later, is not served. **This file adds to the trust
//!   list, it does not replace it**: `push-remotes.json` ([`super::gitcredential::PUSH_REMOTES`])
//!   is what a clone from the window and *Trust this clone* ([`super::gitcredential::vouch`])
//!   already write, and says only that the person trusts the clone at an address. This file adds
//!   what D4 needs and the address cannot say: the numeric id and the account. A repo is served
//!   only if both name it ([`serves`]);
//! - **the host, the `owner/name` and the numeric repo id**, read from the forge as the person
//!   when they pressed. A rename or a transfer does not change the id, so an alias that resolves
//!   to the same id is the same repo and a **lookalike with another id is not** ([`admission`]);
//! - **the account** that serves it (`purlis/@forge/<host>/<account>`'s id): a person may hold a
//!   personal and an enterprise-managed account on one host, and *which serves a project is the
//!   person's choice, made on the press that confirmed it* ([`choose_account`] changes it, on
//!   another press). **purlis never picks between two accounts on its own** ([`account_for`]);
//! - **the registration's client id** that signed the account in, so an account made through a
//!   lookalike application is not mistaken for purlis's.
//!
//! **Only the host writes it, and a chat cannot.** Every writer takes a [`HostScope`], refused
//! inside a chat's process tree, and the file is Machine, device-bound: a clone's own `.charter/`
//! is writable by a chat in that clone, so a list kept there would let a chat confirm itself.
//!
//! **Headless** use (CI, `purlis serve`) has no press: [`headless_repos`] reads the repos from the
//! token connection's own configuration, **in memory only, never written here**, and fails closed
//! on a list it cannot read.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use std::sync::Arc;

use secrecy::SecretString;

use super::accounts::{self, ForgeAccount, Reach, State};
use super::http::TokenSource;
use super::route::HostScope;
use super::transport::Call;
use super::{Forge, ForgeError, Kind, host_of, host_ok, namespace_of};
use crate::secrets::keyring::Store;

/// The confirmed repos' file, in purlis's directory of the config home.
pub const FILE: &str = "forge-repos.json";

const VERSION: u64 = 1;

/// The most [`FILE`] may be. A person has a few hundred clones at most.
const MOST_BYTES: u64 = 4 << 20;

/// Where [`FILE`] is under the config home `config_root`.
pub fn path(config_root: &Path) -> PathBuf {
    crate::machine::dir(config_root).join(FILE)
}

static WRITING: Mutex<()> = Mutex::new(());

/// What the forge said of a repo when the person pressed: read as the person, never from a chat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub kind: Kind,
    /// Lower-case, with a port when the web address has one.
    pub host: String,
    /// `owner/name` (GitHub) or `group/…/project` (GitLab), as the forge spells it.
    pub path: String,
    /// The forge's own number for the repo (GitHub's `id`, GitLab's project `id`).
    pub repo_id: u64,
}

/// One confirmed repo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmedRepo {
    /// The clone's key ([`crate::sandbox::local::clone_key`]).
    pub clone_key: String,
    pub kind: Kind,
    pub host: String,
    /// Lower-case, no `.git`: what a request's path is compared with.
    pub path: String,
    pub repo_id: u64,
    /// The id of the account that serves it ([`ForgeAccount::id`]).
    pub account: String,
    /// The public client id the account was signed in through.
    pub client_id: String,
}

/// Asks the forge, **as the person**, which numeric id a repo has. The window's implementation
/// calls the forge with the sign-in; a test's answers from a table.
pub trait RepoLookup {
    fn repo_id(&self, account: &ForgeAccount, path: &str) -> Result<u64, String>;
}

/// **The window's [`RepoLookup`]**: asks the forge for the repo **as the account**, with its own
/// token, over `reach` (HTTPS in the app, a recorded forge in a test). GitHub: `GET repos/<path>`,
/// its `id`; GitLab: `GET projects/<path, percent-encoded>`, its `id`. A repo the account cannot
/// see is a refusal, never an id.
pub struct AsThePerson<'a> {
    pub keyring: &'a dyn Store,
    pub reach: Reach<'a>,
}

/// A token handed to the transport for one request.
struct Fixed(SecretString);

impl TokenSource for Fixed {
    fn token(&self) -> Result<SecretString, ForgeError> {
        Ok(self.0.clone())
    }
}

impl RepoLookup for AsThePerson<'_> {
    fn repo_id(&self, account: &ForgeAccount, path: &str) -> Result<u64, String> {
        let token = accounts::token(self.keyring, account)?.ok_or_else(|| {
            format!(
                "{} on {} is signed out. Sign in first, then trust the clone.",
                account.login, account.host
            )
        })?;
        let forge = Forge {
            kind: account.kind,
            host: account.host.clone(),
        };
        let transport = (self.reach)(&forge, Arc::new(Fixed(token))).map_err(|e| e.to_string())?;
        let at = match account.kind {
            Kind::GitHub => format!("repos/{path}"),
            Kind::GitLab => format!("projects/{}", path.replace('/', "%2F")),
        };
        let not_found = || {
            format!(
                "{} did not show {path} to {}. Check that this account can see the repo, then \
                 try again.",
                account.host, account.login
            )
        };
        let reply = transport
            .send(&forge, &Call::get(at, super::STATUS_TIMEOUT))
            .map_err(|no| {
                format!(
                    "purlis could not reach {}: {}. Check the host and your connection.",
                    account.host,
                    no.said()
                )
            })?;
        if !reply.ok() {
            return Err(not_found());
        }
        serde_json::from_str::<Value>(&reply.out)
            .ok()
            .and_then(|body| body.get("id").and_then(Value::as_u64))
            .filter(|id| *id != 0)
            .ok_or_else(|| {
                format!(
                    "{} answered without saying which repo {path} is, so nothing was recorded.",
                    account.host
                )
            })
    }
}

#[derive(Default, Serialize, Deserialize)]
struct OnDisk {
    #[serde(default)]
    version: u64,
    #[serde(default)]
    repos: Vec<Value>,
    #[serde(flatten)]
    rest: Map<String, Value>,
}

#[derive(Serialize, Deserialize)]
struct Entry {
    clone_key: String,
    kind: String,
    host: String,
    path: String,
    repo_id: u64,
    account: String,
    client_id: String,
    #[serde(flatten)]
    rest: Map<String, Value>,
}

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
            "{} is larger than a list of repos can be",
            at.display()
        ));
    }
    serde_json::from_str(&text)
        .map_err(|e| format!("{} is not a list of confirmed repos: {e}", at.display()))
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

fn repo_of(value: &Value) -> Option<ConfirmedRepo> {
    let entry: Entry = serde_json::from_value(value.clone()).ok()?;
    let kind = Kind::parse(&entry.kind)?;
    let usable = host_ok(&entry.host)
        && entry.host == entry.host.to_ascii_lowercase()
        && !entry.account.is_empty()
        && entry.repo_id != 0
        && normalised_path(kind, &entry.path).as_deref() == Some(entry.path.as_str());
    usable.then_some(ConfirmedRepo {
        clone_key: entry.clone_key,
        kind,
        host: entry.host,
        path: entry.path,
        repo_id: entry.repo_id,
        account: entry.account,
        client_id: entry.client_id,
    })
}

/// A repo's path as it is compared: lower case, no `.git`, no leading or trailing `/`; two
/// segments on GitHub, two or more on GitLab, each of letters, digits, `.`, `_` and `-`, and
/// never `.` or `..`.
pub fn normalised_path(kind: Kind, path: &str) -> Option<String> {
    let lowered = path.trim_matches('/').to_ascii_lowercase();
    let bare = lowered.strip_suffix(".git").unwrap_or(&lowered);
    let segments: Vec<&str> = bare.split('/').collect();
    let count_ok = match kind {
        Kind::GitHub => segments.len() == 2,
        Kind::GitLab => segments.len() >= 2,
    };
    let segments_ok = segments.iter().all(|s| {
        !s.is_empty()
            && *s != "."
            && *s != ".."
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
    });
    (count_ok && segments_ok).then(|| segments.join("/"))
}

/// The key the list names `clone` by: the trust list's own.
fn clone_key(clone: &Path) -> String {
    crate::sandbox::local::clone_key(clone)
}

/// **The press that confirms a repo** (D4): records `candidate` for `clone`, served by `account`,
/// which was signed in through `client_id`. Refused unless the account is signed in on the same
/// forge and host as the repo, so a repo is never served by an account of another host. A repo
/// already confirmed for that clone (the same host and numeric id) is updated in place, its
/// account and path replaced: a rename is the same repo.
pub fn confirm(
    _host: &HostScope,
    config_root: &Path,
    clone: &Path,
    candidate: &Candidate,
    account: &ForgeAccount,
    client_id: &str,
) -> Result<ConfirmedRepo, String> {
    if account.kind != candidate.kind || !account.host.eq_ignore_ascii_case(&candidate.host) {
        return Err(format!(
            "{} is not signed in to {}, so it cannot serve a repo there. Choose an account on {}.",
            account.login, candidate.host, candidate.host
        ));
    }
    if account.state != State::SignedIn {
        return Err(format!(
            "{} on {} is signed out. Sign in first, then trust the clone.",
            account.login, account.host
        ));
    }
    if candidate.repo_id == 0 {
        return Err(format!(
            "{} did not say which repo this is, so nothing was recorded.",
            candidate.host
        ));
    }
    let path = normalised_path(candidate.kind, &candidate.path).ok_or_else(|| {
        format!(
            "{} is not a repo path on {}, so nothing was recorded.",
            candidate.path, candidate.host
        )
    })?;
    let host = candidate.host.to_ascii_lowercase();
    if !host_ok(&host) {
        return Err(format!("{host} is not a host, so nothing was recorded."));
    }
    let key = clone_key(clone);
    let entry = serde_json::json!({
        "clone_key": key,
        "kind": candidate.kind.word(),
        "host": host,
        "path": path,
        "repo_id": candidate.repo_id,
        "account": account.id,
        "client_id": client_id,
    });
    let _one = WRITING.lock().unwrap_or_else(|e| e.into_inner());
    let mut file = read(config_root)?;
    let same = file.repos.iter().position(|value| {
        repo_of(value).is_some_and(|r| {
            r.clone_key == key
                && r.kind == candidate.kind
                && r.host == host
                && r.repo_id == candidate.repo_id
        })
    });
    match same {
        Some(at) => file.repos[at] = entry,
        None => file.repos.push(entry),
    }
    write(config_root, file)?;
    Ok(ConfirmedRepo {
        clone_key: key,
        kind: candidate.kind,
        host,
        path,
        repo_id: candidate.repo_id,
        account: account.id.clone(),
        client_id: client_id.to_string(),
    })
}

/// **Clones from the window, and "Trust this clone"** (D4): reads which repo `remote_url` is on
/// the forge, **as the person and through `lookup`**, then confirms it for `clone`. The repo is
/// named by the clone's remote, but trusted by the id the forge gave it. Refused when the
/// remote is on another host than `account`, or names no repo.
pub fn record_clone(
    host: &HostScope,
    config_root: &Path,
    clone: &Path,
    remote_url: &str,
    lookup: &dyn RepoLookup,
    account: &ForgeAccount,
    client_id: &str,
) -> Result<ConfirmedRepo, String> {
    let named = host_port_of(remote_url);
    if named.is_empty() || !named.eq_ignore_ascii_case(&account.host) {
        let on = if named.is_empty() { "no host" } else { &named };
        return Err(format!(
            "This clone's remote is on {on}, and {} is signed in to {}. Choose an account on {on}.",
            account.login, account.host
        ));
    }
    let path = namespace_of(remote_url)
        .and_then(|raw| normalised_path(account.kind, &raw))
        .ok_or_else(|| {
            "This clone's remote does not name a repo on the forge, so it was not trusted."
                .to_string()
        })?;
    let repo_id = lookup.repo_id(account, &path)?;
    confirm(
        host,
        config_root,
        clone,
        &Candidate {
            kind: account.kind,
            host: account.host.clone(),
            path,
            repo_id,
        },
        account,
        client_id,
    )
}

/// The host of `url` as a web address names it: with its port for `http` and `https`, where it
/// is part of the host a forge signs in on; without it for `ssh` and scp-style, where it is not.
fn host_port_of(url: &str) -> String {
    let host = host_of(url);
    let url = url.trim();
    for scheme in ["https://", "http://"] {
        let Some(rest) = url
            .get(..scheme.len())
            .filter(|head| head.eq_ignore_ascii_case(scheme))
            .map(|_| &url[scheme.len()..])
        else {
            continue;
        };
        let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
        let authority = authority.rsplit('@').next().unwrap_or(authority);
        if let Some((_, port)) = authority.rsplit_once(':')
            && !port.is_empty()
            && port.bytes().all(|b| b.is_ascii_digit())
        {
            return format!("{host}:{port}");
        }
    }
    host
}

/// Every repo confirmed for `clone`.
pub fn repos_of_clone(config_root: &Path, clone: &Path) -> Result<Vec<ConfirmedRepo>, String> {
    let key = clone_key(clone);
    Ok(read(config_root)?
        .repos
        .iter()
        .filter_map(repo_of)
        .filter(|r| r.clone_key == key)
        .collect())
}

/// What the list says of a request that names `path` on `host`, whose repo the forge says has
/// `id_of_path` (the proxy asks, and a request's own words are never trusted for it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Admission {
    /// Confirmed under this very path.
    Confirmed(ConfirmedRepo),
    /// Confirmed under **another path with the same id**: the repo was renamed or transferred, and
    /// the old address still reaches it. The same repo.
    Alias(ConfirmedRepo),
    /// Not confirmed: no entry, or an entry with **another id** under this path (a lookalike, or a
    /// name that now belongs to a different repo).
    NotConfirmed,
}

/// Whether the request for `path` on `host` is for a repo confirmed for `clone`, by **numeric id**.
pub fn admission(
    config_root: &Path,
    clone: &Path,
    kind: Kind,
    host: &str,
    path: &str,
    id_of_path: u64,
) -> Result<Admission, String> {
    let Some(path) = normalised_path(kind, path) else {
        return Ok(Admission::NotConfirmed);
    };
    let found = repos_of_clone(config_root, clone)?
        .into_iter()
        .find(|r| r.kind == kind && r.host.eq_ignore_ascii_case(host) && r.repo_id == id_of_path);
    Ok(match found {
        Some(r) if r.path == path => Admission::Confirmed(r),
        Some(r) => Admission::Alias(r),
        None => Admission::NotConfirmed,
    })
}

/// **Whether the proxy may serve a request for `path` on `host`** from `clone`: the person trusts
/// the clone (a clone from the window, or *Trust this clone*: [`super::gitcredential::vouch`]) **and**
/// this list holds the repo by numeric id ([`admission`]). Either alone is not enough: a trusted
/// clone with no confirmed id has never had its repo read from the forge as the person, and a
/// confirmed id on a clone the person no longer trusts is not theirs to serve.
pub fn serves(
    config_root: &Path,
    clone: &Path,
    kind: Kind,
    host: &str,
    path: &str,
    id_of_path: u64,
) -> Result<Admission, String> {
    if !super::gitcredential::vouch::trusted(config_root, clone) {
        return Ok(Admission::NotConfirmed);
    }
    admission(config_root, clone, kind, host, path, id_of_path)
}

/// **The press that changes which account serves a confirmed repo** (D2): the account must be
/// signed in on the repo's own host. The repo is named by its clone's key and its numeric id, as
/// [`all`] lists them.
pub fn choose_account(
    _host: &HostScope,
    config_root: &Path,
    clone_key: &str,
    repo_id: u64,
    account: &ForgeAccount,
) -> Result<ConfirmedRepo, String> {
    let _one = WRITING.lock().unwrap_or_else(|e| e.into_inner());
    let mut file = read(config_root)?;
    let at = file
        .repos
        .iter()
        .position(|value| {
            repo_of(value).is_some_and(|r| r.clone_key == clone_key && r.repo_id == repo_id)
        })
        .ok_or_else(|| "That clone is not trusted yet. Trust it first.".to_string())?;
    let repo = repo_of(&file.repos[at]).ok_or_else(|| "That entry cannot be read.".to_string())?;
    if account.kind != repo.kind || !account.host.eq_ignore_ascii_case(&repo.host) {
        return Err(format!(
            "{} is not an account on {}, so it cannot serve this repo.",
            account.login, repo.host
        ));
    }
    if account.state != State::SignedIn {
        return Err(format!(
            "{} on {} is signed out. Sign in first.",
            account.login, account.host
        ));
    }
    if let Some(Value::Object(fields)) = file.repos.get_mut(at) {
        fields.insert("account".to_string(), Value::String(account.id.clone()));
    }
    write(config_root, file)?;
    Ok(ConfirmedRepo {
        account: account.id.clone(),
        ..repo
    })
}

/// Every confirmed repo on this machine, in the order they were confirmed: what Settings lists.
pub fn all(config_root: &Path) -> Result<Vec<ConfirmedRepo>, String> {
    Ok(read(config_root)?
        .repos
        .iter()
        .filter_map(repo_of)
        .collect())
}

/// The key `clone` is named by in this list.
pub fn key_of_clone(clone: &Path) -> String {
    clone_key(clone)
}

/// The address (`<host>/<owner>/<repo>`) the trust list holds for `clone`, when the person trusts
/// it.
pub fn trusted_place(config_root: &Path, clone: &Path) -> Option<String> {
    super::gitcredential::read_push_remotes(config_root)
        .get(&clone_key(clone))
        .cloned()
}

/// **Confirms the clone the person trusts** (a clone from the window, or *Trust this clone*): the
/// address the trust list holds for it ([`super::gitcredential::vouch::trusted`] says whether
/// there is one) is read from the forge as `account`, and recorded. Refused when the clone is not
/// trusted, or its trusted address is on another host than the account.
pub fn confirm_trusted(
    host: &HostScope,
    config_root: &Path,
    clone: &Path,
    lookup: &dyn RepoLookup,
    account: &ForgeAccount,
    client_id: &str,
) -> Result<ConfirmedRepo, String> {
    let place = trusted_place(config_root, clone)
        .ok_or_else(|| "That clone is not trusted yet. Trust it first.".to_string())?;
    record_clone(
        host,
        config_root,
        clone,
        &format!("https://{place}"),
        lookup,
        account,
        client_id,
    )
}

/// Takes the confirmation of every repo of `clone` away (the clone is gone, or the person no
/// longer trusts it).
pub fn forget_clone(_host: &HostScope, config_root: &Path, clone: &Path) -> Result<(), String> {
    let key = clone_key(clone);
    let _one = WRITING.lock().unwrap_or_else(|e| e.into_inner());
    let mut file = read(config_root)?;
    let before = file.repos.len();
    file.repos
        .retain(|value| !repo_of(value).is_some_and(|r| r.clone_key == key));
    if file.repos.len() == before {
        return Ok(());
    }
    write(config_root, file)
}

/// What a person's press has to decide for a repo on `host`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pick {
    /// One account is signed in on the host: it serves, and nothing is asked.
    Only(ForgeAccount),
    /// Several are: the person chooses.
    Choose(Vec<ForgeAccount>),
    /// None is: sign in first.
    SignIn,
}

/// The account a press defaults to among `signed_in` (accounts signed in on one host).
pub fn default_account(signed_in: Vec<ForgeAccount>) -> Pick {
    match <[ForgeAccount; 1]>::try_from(signed_in) {
        Ok([only]) => Pick::Only(only),
        Err(many) if many.is_empty() => Pick::SignIn,
        Err(many) => Pick::Choose(many),
    }
}

/// Which account serves a confirmed repo right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Serving {
    /// The account the person chose, and it is signed in.
    Account(ForgeAccount),
    /// The chosen account is signed out or gone. **No other account serves in its place**: it is
    /// another principal, and a chat acting as it would act as someone the person did not choose.
    /// The window raises a Notice to sign that account in again.
    ChosenIsSignedOut { account_id: String },
}

/// The account that serves `repo`, among `accounts` (every account on this machine).
pub fn account_for(repo: &ConfirmedRepo, accounts: &[ForgeAccount]) -> Serving {
    accounts
        .iter()
        .find(|a| {
            a.id == repo.account
                && a.state == State::SignedIn
                && a.kind == repo.kind
                && a.host.eq_ignore_ascii_case(&repo.host)
        })
        .cloned()
        .map_or_else(
            || Serving::ChosenIsSignedOut {
                account_id: repo.account.clone(),
            },
            Serving::Account,
        )
}

/// A repo a token connection names: not confirmed by a press, only configured.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfiguredRepo {
    pub host: String,
    pub path: String,
}

/// **The repos a headless runner serves** (D9), read from the token connection's own
/// configuration at start, in memory, never written to [`FILE`]: `PURLIS_FORGE_REPOS`, a
/// comma-separated list of `host/owner/name`, else the CI checkout's `origin` (the one repo the
/// job runs in). Fails closed: a list with an entry that is not a repo on a host `kind` has, and a
/// checkout whose remote names none, start nothing.
pub fn headless_repos(
    kind: Kind,
    env: &dyn Fn(&str) -> Option<String>,
    checkout_origin: Option<&str>,
) -> Result<Vec<ConfiguredRepo>, String> {
    if let Some(list) = env("PURLIS_FORGE_REPOS").filter(|v| !v.trim().is_empty()) {
        let mut repos = Vec::new();
        for item in list.split(',').map(str::trim) {
            let named = item
                .split_once('/')
                .and_then(|(host, path)| {
                    let host = host.to_ascii_lowercase();
                    host_ok(&host).then_some((host, path))
                })
                .and_then(|(host, path)| Some((host, normalised_path(kind, path)?)));
            let Some((host, path)) = named else {
                return Err(
                    "PURLIS_FORGE_REPOS is a comma-separated list of host/owner/name, and one \
                     entry is not. Nothing was started."
                        .to_string(),
                );
            };
            repos.push(ConfiguredRepo { host, path });
        }
        return Ok(repos);
    }
    let origin = checkout_origin.filter(|o| !o.trim().is_empty()).ok_or_else(|| {
        "No repos were named (PURLIS_FORGE_REPOS) and the checkout has no origin, so nothing was \
         started."
            .to_string()
    })?;
    let host = host_port_of(origin);
    let path = namespace_of(origin).and_then(|raw| normalised_path(kind, &raw));
    match (host.is_empty(), path) {
        (false, Some(path)) => Ok(vec![ConfiguredRepo { host, path }]),
        _ => Err("The checkout's origin does not name a repo, so nothing was started.".to_string()),
    }
}

/// **Reconciles what a press confirmed with what a headless runner was configured with**: the
/// repos in `configured` that are also in `confirmed` by host and path. A desktop that forwards
/// for a runner trusts its own list for that runner's chats, never the runner's (D9): this is the
/// intersection, so a runner naming a repo the desktop never confirmed is not served.
pub fn reconcile(confirmed: &[ConfirmedRepo], configured: &[ConfiguredRepo]) -> Vec<ConfirmedRepo> {
    confirmed
        .iter()
        .filter(|c| {
            configured
                .iter()
                .any(|r| r.host.eq_ignore_ascii_case(&c.host) && r.path == c.path)
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests;
