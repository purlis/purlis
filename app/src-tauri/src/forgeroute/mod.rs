// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The app runs the forge proxy for every chat** (one login, slice J; ADR 0128).
//!
//! Slice A is the proxy's decisions ([`purlis_core::forge::proxy`]); this is who starts it, for
//! which chat, with whose credential, and who stops it. Plain Rust, no Tauri: the chat start and
//! end, the doctor and the exit call it, and the tests below drive it without a window.
//!
//! - **Per chat** ([`ForgeRoutes::start`]): a chat with recorded repos gets its own
//!   [`ForgeProxy`] (so its credential lookup is its project's, never another's), a sentinel, a
//!   folder `forge-route/<chat>/` of mode 0700 and, from the listener, a socket, an egress URL
//!   and a certificate authority in it. The [`RouteParams`] go to slice B's route, which puts
//!   them in the chat's environment.
//! - **Nothing outlives its chat** ([`ForgeRoutes::stop`]): the sentinel stops working, the
//!   listener's guard is dropped and the folder goes. The folders are also swept when the app
//!   starts ([`ForgeRoutes::new`], after a crash) and when it exits ([`Drop`], [`stop_all`]).
//! - **Credentials** ([`ProjectCredentials`]): the host and the project's account, asked of slice
//!   C. A missing or ended sign-in refuses the request in words that name the sign-in step
//!   ([`ForgeRoutes::refusal_message`]) and raises one Notice per chat ([`Notices`]).
//! - **Held acts** ([`ForgeRoutes::handle`], [`ForgeRoutes::press`]): a refusal the person may
//!   allow raises slice F's ask; their press performs it with their credential, and nothing else
//!   does.
//! - **Runners** ([`ForgeRoutes::for_runner`]): an unattended or pool runner's chat is forwarded
//!   to this app's proxy when the runner's link is up, else it keeps token connections (D9).
//! - **Audit** ([`ForgeRoutes::audit_lines`]) and the doctor's `forge route` row
//!   ([`ForgeRoutes::row`]).
//!
//! **Stubs, each filled by the slice named.** [`Listener`]: A2 (listener, per-chat CA, GraphQL
//! types). [`RouteParams`] into the chat's environment: B (`forgewire.rs`). [`Credentials`] is
//! C's now ([`AccountCredentials`], over `SignedIn::credential_of`). [`HeldActs`]: F. [`Recorded`]: G (recorded repos and the
//! project's account binding). [`Notices`]'s real sink and [`RunnerLink`]: the sign-in Notice's
//! settings group is G's, the runner link is the pools'. Until then each is the stub next to its
//! trait, which refuses: a chat with no listener has no route, and fails closed.
//!
//! **Tier** (ADR 0069): the folder is Machine, device-bound, transient, named in
//! `docs/plane-format.md`. The sentinels, policies and audit are memory only.

// The seams are called by the slices that fill them (A2's listener, B's route, F's asks, the
// pools' link); until they merge, only the tests reach them.
#![allow(
    dead_code,
    reason = "slices A2, B, F and the pools call these; see the module note"
)]

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use purlis_core::forge::proxy::{
    AllowedRepo, AuditRecord, ChatId, Credential, CredentialError, CredentialRequest,
    CredentialSource, ForgeProxy, Inbound, NoObjects, Outcome, Refusal, TokenKind,
};
use purlis_core::secrets::keyring::Secret;

#[cfg(test)]
mod tests;

// ---- the seams ----------------------------------------------------------------------------

/// Why a person's sign-in could not be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignInState {
    /// There is none on this host for this account.
    Missing,
    /// There was one, and it has ended (revoked or expired, and it could not be renewed).
    Ended,
}

/// Why a credential was not had: the state, and the sentence that names the sign-in step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignInFailure {
    pub state: SignInState,
    pub said: String,
}

impl SignInFailure {
    fn missing(host: &str) -> Self {
        Self {
            state: SignInState::Missing,
            said: sign_in_words(host, SignInState::Missing),
        }
    }
}

/// The person's credential for a host and an account (**slice C's**, through
/// [`AccountCredentials`]); a trait so a test stands where the keyring would.
pub trait Credentials: Send + Sync {
    fn credential(&self, host: &str, account: &str) -> Result<Credential, SignInFailure>;
    /// Whether [`Self::credential`] would answer.
    fn check(&self, host: &str, account: &str) -> Result<(), SignInFailure>;
}

/// Nobody is signed in: the app has no config root or no host scope (a process a chat started).
pub struct NoSignIns;

impl Credentials for NoSignIns {
    fn credential(&self, host: &str, _: &str) -> Result<Credential, SignInFailure> {
        Err(SignInFailure::missing(host))
    }
    fn check(&self, host: &str, _: &str) -> Result<(), SignInFailure> {
        Err(SignInFailure::missing(host))
    }
}

/// **Slice C's sign-ins** as this app's [`Credentials`]: the token of the account the project is
/// bound to on the host, renewed by C as it needs. No other account answers for it.
pub struct AccountCredentials(purlis_core::forge::credential::SignedIn);

impl AccountCredentials {
    pub fn new(signed_in: purlis_core::forge::credential::SignedIn) -> Self {
        Self(signed_in)
    }
}

impl Credentials for AccountCredentials {
    fn credential(&self, host: &str, account: &str) -> Result<Credential, SignInFailure> {
        use purlis_core::forge::credential::TokenKind as Made;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let got = self
            .0
            .credential_of(host, account, now)
            .map_err(|e| SignInFailure {
                // C's sentence names the step (`SIGN_IN_AT`) and carries no token.
                state: if e.is_none_given() {
                    SignInState::Missing
                } else {
                    SignInState::Ended
                },
                said: e.to_string(),
            })?;
        Ok(Credential {
            token_kind: match got.token_kind() {
                Made::SignIn | Made::GitHubUser => TokenKind::Oauth,
                _ => TokenKind::Pat,
            },
            secret: Secret::new(got.expose().to_owned()),
        })
    }
    fn check(&self, host: &str, account: &str) -> Result<(), SignInFailure> {
        self.credential(host, account).map(|_| ())
    }
}

/// **Slice G's** store: a project's recorded repos and the account it is bound to on a host.
pub trait Recorded: Send + Sync {
    fn repos(&self, project: &Path) -> Vec<AllowedRepo>;
    fn account(&self, project: &Path, host: &str) -> Option<String>;
}

/// The stub until G lands: nothing is recorded, so no chat has a route.
pub struct NothingRecorded;

impl Recorded for NothingRecorded {
    fn repos(&self, _: &Path) -> Vec<AllowedRepo> {
        Vec::new()
    }
    fn account(&self, _: &Path, _: &str) -> Option<String> {
        None
    }
}

/// What a bound chat endpoint holds. Dropping the guard closes the listener.
pub struct Bound {
    pub socket: PathBuf,
    pub egress_url: String,
    pub ca: PathBuf,
    pub guard: Box<dyn Send + Sync>,
}

/// **Slice A2's** listener: binds the chat's socket and egress endpoint inside `dir`, makes the
/// chat's own CA (its key in memory only) permitting exactly `hosts`, and serves every request
/// through `proxy.handle`.
pub trait Listener: Send + Sync {
    fn bind(
        &self,
        chat: &ChatId,
        dir: &Path,
        proxy: Arc<ForgeProxy>,
        hosts: &[String],
    ) -> io::Result<Bound>;
    /// Whether it can bind at all in this build (the doctor's "proxy up").
    fn ready(&self) -> bool;
}

/// The stub until A2 lands: nothing listens.
pub struct NoListener;

impl Listener for NoListener {
    fn bind(&self, _: &ChatId, _: &Path, _: Arc<ForgeProxy>, _: &[String]) -> io::Result<Bound> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "the forge listener is not in this build",
        ))
    }
    fn ready(&self) -> bool {
        false
    }
}

/// Where a Notice goes. The real sink is the Inbox's, with a way out into the sign-in settings
/// (slice G's group); until then it is [`NoNotices`].
pub trait Notices: Send + Sync {
    /// Told once per chat, with the host that had no usable sign-in.
    fn sign_in_needed(&self, chat: &str, host: &str, state: SignInState);
}

pub struct NoNotices;

impl Notices for NoNotices {
    fn sign_in_needed(&self, _: &str, _: &str, _: SignInState) {}
}

/// What a person may allow when the proxy refuses it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeldAct {
    Merge,
    Approval,
    ReleaseOrTag,
}

/// Which refusals are held acts, not plain refusals. **Slice F's call**: this is the list the
/// design names (the acts that end a task's work and are the person's alone).
pub fn held_act(refusal: Refusal) -> Option<HeldAct> {
    match refusal {
        Refusal::Merge | Refusal::MergeCommand => Some(HeldAct::Merge),
        Refusal::Approval => Some(HeldAct::Approval),
        Refusal::ReleaseOrTag => Some(HeldAct::ReleaseOrTag),
        _ => None,
    }
}

/// One held act, for the person to allow. It holds no credential and no query string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeldAsk {
    pub chat: String,
    pub host: String,
    pub method: String,
    /// The request's path, without its query.
    pub path: String,
    pub act: HeldAct,
}

/// **Slice F's** held acts: the ask the person sees, and the act made when they press.
pub trait HeldActs: Send + Sync {
    fn raise(&self, ask: &HeldAsk);
    /// Performs `ask` with `credential`, the person's own. Only [`ForgeRoutes::press`] calls it.
    fn perform(&self, ask: &HeldAsk, credential: Credential) -> Result<(), String>;
}

/// The stub until F lands: nothing is asked, so a held act stays refused.
pub struct NoHeldActs;

impl HeldActs for NoHeldActs {
    fn raise(&self, _: &HeldAsk) {}
    fn perform(&self, _: &HeldAsk, _: Credential) -> Result<(), String> {
        Err("held acts are not in this build".to_owned())
    }
}

/// Whether a runner's link to this desktop app is up.
pub trait RunnerLink: Send + Sync {
    fn reachable(&self, chat: &str) -> bool;
}

/// The stub until the pools carry the link: no runner reaches the desktop.
pub struct NoRunnerLink;

impl RunnerLink for NoRunnerLink {
    fn reachable(&self, _: &str) -> bool {
        false
    }
}

/// Every seam, so a test (or the app) names the ones it has and leaves the rest as stubs.
#[derive(Clone)]
pub struct Seams {
    pub listener: Arc<dyn Listener>,
    pub credentials: Arc<dyn Credentials>,
    pub recorded: Arc<dyn Recorded>,
    pub notices: Arc<dyn Notices>,
    pub held: Arc<dyn HeldActs>,
    pub runner_link: Arc<dyn RunnerLink>,
}

impl Seams {
    /// Every seam as its stub: a build with `forge-wiring` and none of the other slices.
    pub fn stubs() -> Self {
        Self {
            listener: Arc::new(NoListener),
            credentials: Arc::new(NoSignIns),
            recorded: Arc::new(NothingRecorded),
            notices: Arc::new(NoNotices),
            held: Arc::new(NoHeldActs),
            runner_link: Arc::new(NoRunnerLink),
        }
    }
}

// ---- what a chat is handed ----------------------------------------------------------------

/// What **slice B's** `Route` is handed for a chat: where its proxy is, and the stand-in it holds.
pub struct RouteParams {
    pub socket: PathBuf,
    pub egress_url: String,
    pub ca: PathBuf,
    pub sentinel: String,
    pub helper_exe: PathBuf,
    /// `(host, path)` of each recorded repo.
    pub repos: Vec<(String, String)>,
}

impl std::fmt::Debug for RouteParams {
    /// Never the sentinel.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RouteParams")
            .field("socket", &self.socket)
            .field("egress_url", &self.egress_url)
            .field("ca", &self.ca)
            .field("repos", &self.repos)
            .finish_non_exhaustive()
    }
}

/// How a runner's chat reaches its forges.
#[derive(Debug)]
pub enum RunnerForward {
    /// Through this app's proxy, as a desktop chat does.
    Desktop(RouteParams),
    /// Through the token connections it had before one login (D9).
    TokenConnections,
}

// ---- the credential lookup ----------------------------------------------------------------

/// What one chat's proxy asked of the sign-ins, kept for the words of its refusals.
#[derive(Default)]
struct SignInTrouble {
    told: AtomicBool,
    last: Mutex<Option<(String, SignInFailure)>>,
}

/// A chat's [`CredentialSource`]: the credential for the host and the account its project is
/// bound to. **A host with no bound account is a missing sign-in**, never another account's.
pub struct ProjectCredentials {
    chat: String,
    project: PathBuf,
    seams: Seams,
    trouble: Arc<SignInTrouble>,
}

impl CredentialSource for ProjectCredentials {
    fn credential_for(&self, request: &CredentialRequest) -> Result<Credential, CredentialError> {
        let failure = match self.seams.recorded.account(&self.project, &request.host) {
            None => SignInFailure::missing(&request.host),
            Some(account) => match self.seams.credentials.credential(&request.host, &account) {
                Ok(credential) => return Ok(credential),
                Err(failure) => failure,
            },
        };
        let state = failure.state;
        *self
            .trouble
            .last
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some((request.host.clone(), failure));
        // One Notice per chat, however many requests it makes.
        if !self.trouble.told.swap(true, Ordering::SeqCst) {
            self.seams
                .notices
                .sign_in_needed(&self.chat, &request.host, state);
        }
        Err(CredentialError::Unavailable)
    }
}

// ---- the lifecycle ------------------------------------------------------------------------

struct Live {
    proxy: Arc<ForgeProxy>,
    project: PathBuf,
    trouble: Arc<SignInTrouble>,
    dir: PathBuf,
    /// Dropped before the folder goes.
    bound: Option<Bound>,
}

/// Every chat's route, in this app.
pub struct ForgeRoutes {
    dir: PathBuf,
    helper_exe: PathBuf,
    seams: Seams,
    live: Mutex<HashMap<String, Live>>,
}

impl ForgeRoutes {
    /// Routes made under `dir`. **Whatever is already there is from an app that is gone** (a
    /// crash: its sentinels and CA keys died with it), so it is removed before anything starts.
    pub fn new(dir: PathBuf, helper_exe: PathBuf, seams: Seams) -> Self {
        sweep(&dir);
        Self {
            dir,
            helper_exe,
            seams,
            live: Mutex::new(HashMap::new()),
        }
    }

    /// A chat starts. `None` when its project records no repo: there is nothing to proxy and the
    /// chat is handed no route. An error leaves nothing behind.
    pub fn start(&self, chat: &str, project: &Path) -> io::Result<Option<RouteParams>> {
        let repos = self.seams.recorded.repos(project);
        if repos.is_empty() {
            return Ok(None);
        }
        // A chat that starts again replaces its route: the old sentinel stops working.
        self.stop(chat);
        let chat_dir = self.dir.join(safe_name(chat));
        make_private_dir(&chat_dir)?;
        let trouble = Arc::new(SignInTrouble::default());
        let proxy = Arc::new(ForgeProxy::new(
            Box::new(ProjectCredentials {
                chat: chat.to_owned(),
                project: project.to_owned(),
                seams: self.seams.clone(),
                trouble: trouble.clone(),
            }),
            Box::new(NoObjects),
        ));
        let id = ChatId::new(chat);
        let repo_view: Vec<(String, String)> = repos
            .iter()
            .map(|r| (r.host().to_owned(), r.path().to_owned()))
            .collect();
        let mut hosts: Vec<String> = repo_view.iter().map(|(h, _)| h.clone()).collect();
        hosts.sort();
        hosts.dedup();
        let sentinel = proxy.enrol(&id, repos);
        let bound = match self
            .seams
            .listener
            .bind(&id, &chat_dir, proxy.clone(), &hosts)
        {
            Ok(bound) => bound,
            Err(refused) => {
                proxy.retire(&id);
                let _ = std::fs::remove_dir_all(&chat_dir);
                return Err(refused);
            }
        };
        let params = RouteParams {
            socket: bound.socket.clone(),
            egress_url: bound.egress_url.clone(),
            ca: bound.ca.clone(),
            sentinel: sentinel.expose().to_owned(),
            helper_exe: self.helper_exe.clone(),
            repos: repo_view,
        };
        self.live().insert(
            chat.to_owned(),
            Live {
                proxy,
                project: project.to_owned(),
                trouble,
                dir: chat_dir,
                bound: Some(bound),
            },
        );
        Ok(Some(params))
    }

    /// A chat ends: its sentinel stops, its listener closes and its folder goes.
    pub fn stop(&self, chat: &str) {
        let gone = self.live().remove(chat);
        if let Some(mut live) = gone {
            live.proxy.retire(&ChatId::new(chat));
            drop(live.bound.take());
            let _ = std::fs::remove_dir_all(&live.dir);
        }
    }

    /// Every chat's route, at the app's exit.
    pub fn stop_all(&self) {
        let chats: Vec<String> = self.live().keys().cloned().collect();
        for chat in chats {
            self.stop(&chat);
        }
        sweep(&self.dir);
    }

    pub fn is_live(&self, chat: &str) -> bool {
        self.live().contains_key(chat)
    }

    fn live(&self) -> std::sync::MutexGuard<'_, HashMap<String, Live>> {
        self.live.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn proxy_of(&self, chat: &str) -> Option<Arc<ForgeProxy>> {
        self.live().get(chat).map(|live| live.proxy.clone())
    }

    /// One request from `chat`'s endpoint, decided. A refusal that is a held act raises the ask
    /// as well, and is still refused: only [`Self::press`] performs it.
    pub fn handle(&self, chat: &str, inbound: &Inbound<'_>) -> Outcome {
        let Some(proxy) = self.proxy_of(chat) else {
            return Outcome::Refused(Refusal::UnknownChat);
        };
        let outcome = proxy.handle(&ChatId::new(chat), inbound);
        if let Outcome::Refused(refusal) = &outcome
            && let Some(act) = held_act(*refusal)
        {
            self.seams.held.raise(&HeldAsk {
                chat: chat.to_owned(),
                host: inbound.head.host.clone(),
                method: inbound.head.method.to_ascii_uppercase(),
                path: inbound
                    .head
                    .target
                    .split(['?', '#'])
                    .next()
                    .unwrap_or_default()
                    .to_owned(),
                act,
            });
        }
        outcome
    }

    /// The person pressed Allow on `ask`: performed with their credential for the host and the
    /// project's account. A sign-in that is missing or ended is an error that says so.
    pub fn press(&self, ask: &HeldAsk) -> Result<(), String> {
        let project = self
            .live()
            .get(&ask.chat)
            .map(|live| live.project.clone())
            .ok_or_else(|| "That chat has ended, so there is nothing to allow.".to_owned())?;
        let account = self.seams.recorded.account(&project, &ask.host);
        let credential = account
            .ok_or_else(|| SignInFailure::missing(&ask.host))
            .and_then(|account| self.seams.credentials.credential(&ask.host, &account))
            .map_err(|failure| failure.said)?;
        self.seams.held.perform(ask, credential)
    }

    /// What to tell the chat for `refusal`. A missing or ended sign-in names the step.
    pub fn refusal_message(&self, chat: &str, refusal: Refusal) -> String {
        match refusal {
            Refusal::NoCredential => {
                let last = self.live().get(chat).and_then(|live| {
                    live.trouble
                        .last
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .clone()
                });
                match last {
                    Some((_, failure)) => failure.said,
                    None => "There is no sign-in for this forge. Sign in under Accounts in \
                         Settings, then try again."
                        .to_owned(),
                }
            }
            r if held_act(r).is_some() => "That needs the person's approval. purlis has asked \
                 them; it goes ahead only when they allow it."
                .to_owned(),
            _ => "purlis does not let a chat do that on this forge.".to_owned(),
        }
    }

    /// For a runner's chat: this app's proxy when the runner's link is up, else token
    /// connections (D9). A route that could not start is also token connections.
    pub fn for_runner(&self, chat: &str, project: &Path) -> RunnerForward {
        if !self.seams.runner_link.reachable(chat) {
            return RunnerForward::TokenConnections;
        }
        match self.start(chat, project) {
            Ok(Some(params)) => RunnerForward::Desktop(params),
            Ok(None) | Err(_) => RunnerForward::TokenConnections,
        }
    }

    /// The chat's audit, a line per request: method, host, repo, class and verdict. The audit
    /// holds no token and no path, so nor does a line.
    pub fn audit_lines(&self, chat: &str) -> Vec<String> {
        self.proxy_of(chat)
            .map(|proxy| {
                proxy
                    .audit(&ChatId::new(chat))
                    .iter()
                    .map(audit_line)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The doctor's `forge route` row for `project`: the proxy up, and a sign-in for every
    /// recorded host.
    pub fn row(&self, project: &Path) -> RouteRow {
        let repos = self.seams.recorded.repos(project);
        if repos.is_empty() {
            return RouteRow::ok("no repo is recorded for this project, so no chat has a route");
        }
        let mut hosts: Vec<String> = repos.iter().map(|r| r.host().to_owned()).collect();
        hosts.sort();
        hosts.dedup();
        if !self.seams.listener.ready() {
            return RouteRow::problem(
                "the forge proxy is not running".to_owned(),
                "restart purlis; chats keep their token connections until it runs".to_owned(),
            );
        }
        let without: Vec<String> = hosts
            .iter()
            .filter(|host| {
                self.seams
                    .recorded
                    .account(project, host)
                    .is_none_or(|account| self.seams.credentials.check(host, &account).is_err())
            })
            .cloned()
            .collect();
        if without.is_empty() {
            RouteRow::ok(&format!(
                "the proxy is up and signed in on {}",
                hosts.join(", ")
            ))
        } else {
            RouteRow::problem(
                format!("no usable sign-in on {}", without.join(", ")),
                "sign in under Accounts in Settings".to_owned(),
            )
        }
    }
}

impl Drop for ForgeRoutes {
    fn drop(&mut self) {
        self.stop_all();
    }
}

/// The doctor's row, before the doctor draws it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteRow {
    pub problem: bool,
    pub detail: String,
    pub hint: String,
}

impl RouteRow {
    fn ok(detail: &str) -> Self {
        Self {
            problem: false,
            detail: detail.to_owned(),
            hint: String::new(),
        }
    }
    fn problem(detail: String, hint: String) -> Self {
        Self {
            problem: true,
            detail,
            hint,
        }
    }
}

fn sign_in_words(host: &str, state: SignInState) -> String {
    match state {
        SignInState::Missing => format!(
            "There is no sign-in for {host} on this project. Sign in under Accounts in \
             Settings, then try again."
        ),
        SignInState::Ended => format!(
            "The sign-in for {host} has ended. Sign in again under Accounts in Settings, then \
             try again."
        ),
    }
}

fn audit_line(record: &AuditRecord) -> String {
    format!(
        "{} {} {} {} {:?}",
        record.method,
        record.host,
        record.repo.as_deref().unwrap_or("-"),
        record
            .class
            .as_ref()
            .map_or_else(|| "-".to_owned(), |class| format!("{class:?}")),
        record.verdict,
    )
}

/// A chat id as one folder name: nothing that could climb out of the folder.
fn safe_name(chat: &str) -> String {
    chat.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .take(40)
        .collect()
}

fn make_private_dir(dir: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        if let Some(parent) = dir.parent() {
            std::fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(parent)?;
        }
        std::fs::DirBuilder::new().mode(0o700).create(dir)
    }
    #[cfg(not(unix))]
    {
        std::fs::create_dir_all(dir)
    }
}

/// Removes every chat's folder under `dir`; the folder itself stays.
fn sweep(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let _ = if path.is_dir() {
            std::fs::remove_dir_all(&path)
        } else {
            std::fs::remove_file(&path)
        };
    }
}

/// Where the routes live: short, because a unix socket's path may not pass 104 bytes.
pub fn route_dir() -> PathBuf {
    std::env::temp_dir().join("purlis-forge")
}

// ---- the app's one ----

static ROUTES: std::sync::OnceLock<ForgeRoutes> = std::sync::OnceLock::new();

/// The app's routes, made once at setup. Whatever an earlier app left in [`route_dir`] is swept
/// here. **Seams are the stubs until the slices named at the top fill them**: B's helper
/// executable is this binary for now.
pub fn install() {
    let _ = ROUTES.get_or_init(|| {
        let mut seams = Seams::stubs();
        // Slice C's sign-ins, where this process may read them (only the app's own, never a
        // process a chat started) and a config root exists.
        if let (Some(scope), Some(root)) = (
            purlis_core::forge::route::HostScope::claim(),
            purlis_core::machine::config_root(),
        ) {
            seams.credentials = Arc::new(AccountCredentials::new(
                purlis_core::forge::credential::SignedIn::new(&scope, &root),
            ));
        }
        ForgeRoutes::new(
            route_dir(),
            std::env::current_exe().unwrap_or_default(),
            seams,
        )
    });
}

/// The app's routes, once [`install`] has run.
pub fn routes() -> Option<&'static ForgeRoutes> {
    ROUTES.get()
}

/// A chat starts (`chats.rs`): its route, if its project records repos. A failure is logged and
/// the chat starts without one: it holds no forge credential either way, so it fails closed.
/// **Slice B puts the result in the chat's environment**; until it merges nothing is handed over.
pub fn start_chat(chat: &str, project: &Path) -> Option<RouteParams> {
    match routes()?.start(chat, project) {
        Ok(params) => params,
        Err(refused) => {
            tracing::warn!("purlis: the forge route of a chat did not start ({refused})");
            None
        }
    }
}

/// A chat closes (`chats.rs`).
pub fn stop_chat(chat: &str) {
    if let Some(routes) = routes() {
        routes.stop(chat);
    }
}

/// The app exits.
pub fn stop_all() {
    if let Some(routes) = routes() {
        routes.stop_all();
    }
}
