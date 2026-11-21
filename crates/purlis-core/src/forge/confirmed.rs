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

use super::accounts::{self, ForgeAccount, Method, Reach, State};
use super::http::TokenSource;
use super::registration;
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
    /// The id of the account that serves it ([`ForgeAccount::id`]). **Empty while the person has
    /// not chosen** (several accounts are signed in on the host, and purlis never picks between
    /// them): the repo is recorded, and nothing serves it until [`choose_account`].
    pub account: String,
    /// The public client id of the application the account was signed in through: the host's
    /// pin at that press ([`application_of`]). Empty for a pasted token or an imported login,
    /// which no application granted.
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

/// **The application `account` was signed in through, as the host pins it now** (D1.3 rule 2, D4):
/// what a repo is recorded with, and what serving it is checked against. A browser sign-in is
/// through the application its account recorded, and it counts only while the host's pin still
/// names that very one: a pin the person changed since, or took away, makes it another
/// application's, and the answer is a refusal that says what to do. A browser sign-in made before
/// the id was kept, on a host purlis has its own application for, is that application's. A pasted
/// token and an imported login were granted by no application, and answer an empty id.
pub fn application_of(config_root: &Path, account: &ForgeAccount) -> Result<String, String> {
    if matches!(account.method, Method::Pat | Method::Import) {
        return Ok(String::new());
    }
    let current =
        registration::resolve(config_root, account.kind, &account.host)?.map(|r| r.client_id);
    match (current, account.client_id.as_deref()) {
        (Some(now), Some(signed_in_through)) if now == signed_in_through => Ok(now.to_string()),
        (Some(now), None) if registration::on(account.kind, &account.host).is_some() => {
            Ok(now.to_string())
        }
        _ => Err(application_changed_said(account)),
    }
}

/// What the person is told when an account's application is not the host's pinned one.
pub fn application_changed_said(account: &ForgeAccount) -> String {
    format!(
        "{} now uses a different application than the one {} on {} signed in through, so purlis \
         does not use that sign-in. Check the application in Settings › You › Accounts and sign \
         in again.",
        account.host, account.login, account.host
    )
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

/// **Records a repo whose account the person has not chosen yet** (D2: several accounts are signed
/// in on the host, and purlis never picks between them). The repo is on the list by its numeric
/// id with **no account**, so the project accounts list shows it and asks, and nothing serves it
/// until [`choose_account`]. A repo already recorded for that clone keeps the account it has: a
/// second look at the clone never undoes a choice.
pub fn record_unchosen(
    _host: &HostScope,
    config_root: &Path,
    clone: &Path,
    candidate: &Candidate,
) -> Result<ConfirmedRepo, String> {
    let path = normalised_path(candidate.kind, &candidate.path).ok_or_else(|| {
        format!(
            "{} is not a repo path on {}, so nothing was recorded.",
            candidate.path, candidate.host
        )
    })?;
    let host = candidate.host.to_ascii_lowercase();
    if !host_ok(&host) || candidate.repo_id == 0 {
        return Err(format!(
            "{} did not say which repo this is, so nothing was recorded.",
            candidate.host
        ));
    }
    let key = clone_key(clone);
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
    if let Some(repo) = same.and_then(|at| repo_of(&file.repos[at])) {
        return Ok(repo);
    }
    file.repos.push(serde_json::json!({
        "clone_key": key,
        "kind": candidate.kind.word(),
        "host": host,
        "path": path,
        "repo_id": candidate.repo_id,
        "account": "",
        "client_id": "",
    }));
    write(config_root, file)?;
    Ok(ConfirmedRepo {
        clone_key: key,
        kind: candidate.kind,
        host,
        path,
        repo_id: candidate.repo_id,
        account: String::new(),
        client_id: String::new(),
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
    let candidate = candidate_of(remote_url, lookup, account)?;
    confirm(host, config_root, clone, &candidate, account, client_id)
}

/// What the forge says `remote_url` is, read through `lookup` as `account`: the host, the path and
/// the numeric id. Refused when the remote is on another host than `account`, or names no repo.
pub fn candidate_of(
    remote_url: &str,
    lookup: &dyn RepoLookup,
    account: &ForgeAccount,
) -> Result<Candidate, String> {
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
    Ok(Candidate {
        kind: account.kind,
        host: account.host.clone(),
        path,
        repo_id,
    })
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

/// A confirmed host as the address the proxy reaches: the bare host and its port, with the
/// default `https` port (443) written out for an entry that names none (an `ssh` or scp-style
/// remote, whose port is not the forge's web one).
pub fn address_of(host: &str) -> (String, u16) {
    if let Some((bare, port)) = host.rsplit_once(':')
        && !bare.is_empty()
        && let Ok(port) = port.parse::<u16>()
    {
        return (bare.to_string(), port);
    }
    (host.to_string(), 443)
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
    let admitted = admission(config_root, clone, kind, host, path, id_of_path)?;
    let (Admission::Confirmed(repo) | Admission::Alias(repo)) = &admitted else {
        return Ok(admitted);
    };
    // The address the person trusted the clone at must still be the entry's: a remote that was
    // recorded again or moved since (`remote::record`, `remote::moved`) is another repo's address,
    // and what was read from the forge for the old one says nothing of it.
    let recorded = super::gitcredential::vouch::recorded_remote(config_root, clone);
    let same = recorded.as_deref().is_some_and(|place| {
        let place = place.trim_end_matches('/').to_ascii_lowercase();
        let place = place.strip_suffix(".git").unwrap_or(&place);
        place == format!("{}/{}", repo.host, repo.path)
    });
    if !same {
        return Ok(Admission::NotConfirmed);
    }
    // The application the repo was recorded under must still be the host's pin (D1.3 rule 2): a
    // pin changed since means the account that serves it is another application's.
    if !repo.client_id.is_empty() {
        let current = registration::resolve(config_root, repo.kind, &repo.host)?
            .map(|r| r.client_id.to_string());
        if current.as_deref() != Some(repo.client_id.as_str()) {
            if let Some(account) = accounts::listed(config_root)?
                .into_iter()
                .find(|a| a.id == repo.account)
            {
                super::refresh::raise_once(&account, &super::refresh::Trouble::PinChanged);
            }
            return Ok(Admission::NotConfirmed);
        }
    }
    Ok(admitted)
}

/// **The press that changes which account serves a confirmed repo** (D2): the account must be
/// signed in on the repo's own host. The repo is named by its clone's key and its numeric id, as
/// [`all`] lists them.
pub fn choose_account(
    _host: &HostScope,
    config_root: &Path,
    clone_key: &str,
    kind: Kind,
    host: &str,
    repo_id: u64,
    account: &ForgeAccount,
) -> Result<ConfirmedRepo, String> {
    let _one = WRITING.lock().unwrap_or_else(|e| e.into_inner());
    let mut file = read(config_root)?;
    let at = file
        .repos
        .iter()
        .position(|value| {
            repo_of(value).is_some_and(|r| {
                r.clone_key == clone_key
                    && r.kind == kind
                    && r.host.eq_ignore_ascii_case(host)
                    && r.repo_id == repo_id
            })
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
    // The repo is then served through the application this account is signed in through.
    let client_id = application_of(config_root, account)?;
    if let Some(Value::Object(fields)) = file.repos.get_mut(at) {
        fields.insert("account".to_string(), Value::String(account.id.clone()));
        fields.insert("client_id".to_string(), Value::String(client_id.clone()));
    }
    write(config_root, file)?;
    Ok(ConfirmedRepo {
        account: account.id.clone(),
        client_id,
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

/// Takes the confirmation of every repo of `clone` away (the person no longer trusts it, or trusts
/// it again and it is read afresh). A clone that is gone has no identity to read: use
/// [`forget_by_key`] with the key taken before it was removed ([`key_of_clone`]).
pub fn forget_clone(host: &HostScope, config_root: &Path, clone: &Path) -> Result<(), String> {
    forget_by_key(host, config_root, &clone_key(clone))
}

/// [`forget_clone`] for the clone named by `key` ([`key_of_clone`]).
pub fn forget_by_key(_host: &HostScope, config_root: &Path, key: &str) -> Result<(), String> {
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
    /// The person has not chosen: several accounts were signed in on the host when the repo was
    /// recorded. Nothing serves it, and no account is picked.
    Unchosen,
    /// The chosen account was signed in through another application than the host pins now (the
    /// pin changed since, ADR 0128 D1.3 rule 2). Nothing serves it until the person signs in
    /// again; the window raises a Notice.
    ApplicationChanged { account_id: String },
}

/// The account that serves `repo`, among `accounts` (every account on this machine). Refused, with
/// no other account standing in, when it is signed out or gone, when its token holds `workflow`
/// ([`accounts::everyday_holds_workflow`]: never a chat's), and when it was signed in through
/// another application than the host's pin ([`application_of`]), which also raises a Notice.
pub fn account_for(config_root: &Path, repo: &ConfirmedRepo, accounts: &[ForgeAccount]) -> Serving {
    use super::refresh::{Trouble, raise_once};
    if repo.account.is_empty() {
        return Serving::Unchosen;
    }
    let signed_out = || Serving::ChosenIsSignedOut {
        account_id: repo.account.clone(),
    };
    let Some(account) = accounts.iter().find(|a| {
        a.id == repo.account
            && a.state == State::SignedIn
            && a.kind == repo.kind
            && a.host.eq_ignore_ascii_case(&repo.host)
    }) else {
        return signed_out();
    };
    match super::accounts::workflow_doubt(account) {
        super::accounts::WorkflowDoubt::No => {}
        super::accounts::WorkflowDoubt::Holds => {
            raise_once(account, &Trouble::EverydayHoldsWorkflow);
            return signed_out();
        }
        super::accounts::WorkflowDoubt::Unknown => {
            raise_once(account, &Trouble::ScopesUnknown);
            return signed_out();
        }
    }
    match application_of(config_root, account) {
        Ok(now) if now == repo.client_id => Serving::Account(account.clone()),
        _ => {
            raise_once(account, &Trouble::PinChanged);
            Serving::ApplicationChanged {
                account_id: repo.account.clone(),
            }
        }
    }
}

/// A repo a token connection names: not confirmed by a press, only configured.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfiguredRepo {
    pub host: String,
    pub path: String,
    /// The forge's own number for the repo, where the runner's environment gives it for this very
    /// repo (a CI job names its own: `GITHUB_REPOSITORY_ID` and `CI_PROJECT_ID`). `None` where it
    /// does not: [`reconcile`] then has only the host and the path.
    pub repo_id: Option<u64>,
}

/// The numeric id the runner's environment gives for `path`, when `path` is the repo of the job.
fn id_of_the_jobs_repo(
    kind: Kind,
    env: &dyn Fn(&str) -> Option<String>,
    path: &str,
) -> Option<u64> {
    let (named, id) = match kind {
        Kind::GitHub => ("GITHUB_REPOSITORY", "GITHUB_REPOSITORY_ID"),
        Kind::GitLab => ("CI_PROJECT_PATH", "CI_PROJECT_ID"),
    };
    let jobs = normalised_path(kind, &env(named)?)?;
    (jobs == path)
        .then(|| env(id)?.trim().parse::<u64>().ok())
        .flatten()
        .filter(|id| *id != 0)
}

/// **The repos a headless runner serves** (D9), read from the token connection's own
/// configuration at start, in memory, never written to [`FILE`]: `PURLIS_FORGE_REPOS`, a
/// comma-separated list of `host/owner/name`, else the CI checkout's `origin` (the one repo the
/// job runs in). Fails closed: a list with an entry that is not a repo on a host `kind` has, and a
/// checkout whose remote names none, start nothing. **It takes the host's scope, so it is never run
/// inside a chat's process tree**: a chat that could name its own repos would confirm itself.
pub fn headless_repos(
    _host: &HostScope,
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
            let repo_id = id_of_the_jobs_repo(kind, env, &path);
            repos.push(ConfiguredRepo {
                host,
                path,
                repo_id,
            });
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
        (false, Some(path)) => {
            let repo_id = id_of_the_jobs_repo(kind, env, &path);
            Ok(vec![ConfiguredRepo {
                host,
                path,
                repo_id,
            }])
        }
        _ => Err("The checkout's origin does not name a repo, so nothing was started.".to_string()),
    }
}

/// **Reconciles what a press confirmed with what a headless runner was configured with**: the
/// repos in `configured` that are also in `confirmed` by host and path, **and by numeric id where
/// the runner's environment gave one**: a lookalike that has the same path and another id is not
/// served. A desktop that forwards for a runner trusts its own list for that runner's chats, never
/// the runner's (D9): this is the intersection, so a runner naming a repo the desktop never
/// confirmed is not served. Each repo answered carries the id the press recorded.
pub fn reconcile(confirmed: &[ConfirmedRepo], configured: &[ConfiguredRepo]) -> Vec<ConfirmedRepo> {
    confirmed
        .iter()
        .filter(|c| {
            configured.iter().any(|r| {
                r.host.eq_ignore_ascii_case(&c.host)
                    && r.path == c.path
                    && r.repo_id.is_none_or(|id| id == c.repo_id)
            })
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests;
