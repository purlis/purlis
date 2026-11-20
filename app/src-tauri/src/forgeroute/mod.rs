// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The app runs the forge proxy for every chat** (one login, slice J; ADR 0128).
//!
//! Slice A is the proxy's decisions ([`purlis_core::forge::proxy`]); this is who starts it, for
//! which run of which chat, with whose credential, and who stops it. Plain Rust, no Tauri: the
//! chat start and end, the doctor and the exit call it, and the tests drive it without a window.
//!
//! - **A route belongs to a run, not to a chat.** A restart keeps the chat's id and begins a new
//!   run, so the old run ending (or being closed) can never tear down the new run's route: each
//!   is keyed by its own run id, its own sentinel, its own folder. A start holds a
//!   [`RouteGuard`] that stops the route on every early return, and `commit`s it to the chat's
//!   session once the chat is open; from then on its end, its close, its program exiting on its
//!   own and its project closing each stop it ([`ForgeRoutes::stop_session`],
//!   [`ForgeRoutes::stop_project`]).
//! - **Per run** ([`ForgeRoutes::start`]): a run with recorded repos gets its own [`ForgeProxy`]
//!   (its credential lookup is its project's), a sentinel, a folder of mode 0700 in the app's own
//!   private root and, from the listener, a socket, an egress URL and a certificate authority.
//!   The [`RouteParams`] go to slice B's route.
//! - **The root is the app's alone** ([`ensure_root`]): made 0700 and refused unless it is a
//!   directory (never a link) this user owns. A folder is named `<pid>-<run>`, and the sweep
//!   ([`sweep`]) removes only folders of that shape, owned by this user, whose app is gone.
//! - **Credentials** ([`ProjectCredentials`]): the forge's host ([`forge_host`], one place) and the
//!   project's account, asked of slice C. A missing or ended sign-in refuses the request in
//!   words that name the sign-in step and raises one Notice per chat. A forge that could not be
//!   reached is said to be unreachable, not ended.
//! - **Held acts**: every request goes through [`Handler::handle`], so a refusal the person may
//!   allow registers an ask once ([`HeldAsks`]) and raises slice F's. Only a registered ask can
//!   be performed ([`ForgeRoutes::perform_held`]): one time, for the run and the request it was
//!   raised for, with the person's credential.
//! - **Runners** ([`ForgeRoutes::for_runner`]): a runner's chat is forwarded to this app's proxy
//!   only for a runner paired with this desktop (D9), and the project is the pairing's, never
//!   the caller's.
//!
//! **Stubs, each filled by the slice named.** [`Listener`]: A2. [`RouteParams`] into the chat's
//! environment: B. [`HeldActs`] and its list of held refusals: F (F2's registry replaces
//! [`HeldAsks`]). [`Recorded`]: G. [`Notices`]'s real sink: G's settings group. [`RunnerLink`]:
//! the pools' pairing.
//!
//! **Tier** (ADR 0069): the root is Machine, device-bound, transient, named in
//! `docs/plane-format.md`. The sentinels, policies, asks and audit are memory only.

// The seams are called by the slices that fill them (A2's listener, B's route, F's asks, the
// pools' pairing); until they merge, only the tests reach them.
#![allow(
    dead_code,
    reason = "slices A2, B, F and the pools call these; see the module note"
)]

use std::collections::{HashMap, HashSet};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use purlis_core::forge::proxy::{
    AllowedRepo, AuditRecord, BodyNeed, ChatId, Credential, CredentialError, CredentialRequest,
    CredentialSource, ForgeProxy, Forwarded, Inbound, NoObjects, Outcome, Refusal, RequestHead,
    TokenKind,
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
    /// There was one, and it has ended (revoked, or the forge refused its renewal).
    Ended,
    /// The forge could not be asked now. The sign-in may be fine: this is not "sign in again".
    Unavailable,
}

/// Why a credential was not had: the state, and the sentence that names the step.
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

/// The forge's own host for a request's host: GitHub's API host is the forge `github.com`, every
/// other host is itself, in lower case. **The one place** a credential's host is made, for a
/// request and for a held act alike.
pub fn forge_host(host: &str) -> String {
    let host = host.trim().to_ascii_lowercase();
    match host.as_str() {
        "api.github.com" => "github.com".to_owned(),
        _ => host,
    }
}

/// The person's credential for a host and an account (**slice C's**, through
/// [`AccountCredentials`]); a trait so a test stands where the keyring would. `host` is always
/// [`forge_host`]'s.
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
        use purlis_core::forge::credential::{CredentialError as Why, TokenKind as Made};
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let got = self
            .0
            .credential_of(host, account, now)
            .map_err(|e| SignInFailure {
                state: match &e {
                    Why::None { .. } => SignInState::Missing,
                    // The forge could not be asked: not a sign-in that ended.
                    Why::Unreachable { .. } => SignInState::Unavailable,
                    _ => SignInState::Ended,
                },
                // C's sentence names the step and carries no token.
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
    /// `host` is [`forge_host`]'s.
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

/// What the listener serves a run's requests through. **Every request goes through
/// [`Handler::handle`]** and nothing reaches the proxy any other way, so a held act's refusal
/// registers and raises its ask wherever the request came from.
pub trait Handler: Send + Sync {
    fn handle(&self, inbound: &Inbound<'_>) -> Outcome;
    fn body_need(&self, head: &RequestHead) -> BodyNeed;
    /// The listener read a push's ref advertisement.
    fn note_advertised(&self, host: &str, repo_path: &str, branches: Vec<String>);
    /// The forge confirmed a push [`Handler::handle`] forwarded.
    fn confirm_push(&self, forwarded: &Forwarded);
}

/// What a bound run endpoint holds. Dropping the guard closes the listener.
pub struct Bound {
    pub socket: PathBuf,
    pub egress_url: String,
    pub ca: PathBuf,
    pub guard: Box<dyn Send + Sync>,
}

/// **Slice A2's** listener: binds the run's socket and egress endpoint inside `dir`, makes the
/// run's own CA (its key in memory only) permitting exactly `hosts`, and serves every request
/// through `handler`.
pub trait Listener: Send + Sync {
    fn bind(
        &self,
        run: &str,
        dir: &Path,
        handler: Arc<dyn Handler>,
        hosts: &[String],
    ) -> io::Result<Bound>;
    /// Whether it can bind at all in this build (the doctor's "proxy up").
    fn ready(&self) -> bool;
}

/// The stub until A2 lands: nothing listens.
pub struct NoListener;

impl Listener for NoListener {
    fn bind(&self, _: &str, _: &Path, _: Arc<dyn Handler>, _: &[String]) -> io::Result<Bound> {
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
    /// Told once per chat, with the host that had no usable sign-in. Never for
    /// [`SignInState::Unavailable`], which is not the person's to fix.
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

/// One held act, for the person to allow. It holds no credential and no query string. Only
/// [`HeldAsks`] makes one, so its `id` names a registered ask.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeldAsk {
    pub id: u64,
    /// The run it was raised for.
    pub run: String,
    /// [`forge_host`]'s host.
    pub host: String,
    pub method: String,
    /// The request's path, without its query.
    pub path: String,
    /// The recorded repo the path names, where it names one.
    pub repo: Option<String>,
    pub act: HeldAct,
}

/// **Slice F's** held acts: the ask the person sees, and the act made when they press.
pub trait HeldActs: Send + Sync {
    fn raise(&self, ask: &HeldAsk);
    /// Performs `ask` with `credential`, the person's own. Only [`ForgeRoutes::perform_held`]
    /// calls it, for an ask it registered.
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

/// A runner as its link authenticated it (D9): its device and its link key's public half.
/// Made only from a link's own handshake, never from a request's words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunnerPeer {
    pub device: String,
    pub key: [u8; 32],
}

/// **The pools' pairing** (D9): which runners are paired with this desktop, and which project a
/// paired runner's run belongs to.
pub trait RunnerLink: Send + Sync {
    /// Whether `peer` is a runner this desktop is paired with (its key pinned at pairing).
    fn paired(&self, peer: &RunnerPeer) -> bool;
    /// The project `run` of a chat on `peer` belongs to, from the pairing's records.
    fn project_of(&self, peer: &RunnerPeer, run: &str) -> Option<PathBuf>;
}

/// The stub until the pools carry the pairing: no runner is paired.
pub struct NoRunnerLink;

impl RunnerLink for NoRunnerLink {
    fn paired(&self, _: &RunnerPeer) -> bool {
        false
    }
    fn project_of(&self, _: &RunnerPeer, _: &str) -> Option<PathBuf> {
        None
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

// ---- what a run is handed -----------------------------------------------------------------

/// What **slice B's** `Route` is handed for a run: where its proxy is, and the stand-in it holds.
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
    /// This app has no route for it (nothing recorded, or no listener): the runner keeps the
    /// token connections it had before one login (D9).
    TokenConnections,
    /// The caller is not a runner paired with this desktop, or names no run of its own.
    Refused,
}

// ---- held asks ----------------------------------------------------------------------------

/// The asks raised and not yet performed or dropped. **F2's registry replaces this**; the
/// contract is the same: an ask is made only here, performed once, and gone with its run.
#[derive(Default)]
pub struct HeldAsks {
    next: AtomicU64,
    pending: Mutex<HashMap<u64, HeldAsk>>,
}

impl HeldAsks {
    /// Registers an ask for a run, or returns `None` where the same request is already pending
    /// (a chat that retries is one ask, not many).
    fn register(&self, mut ask: HeldAsk) -> Option<HeldAsk> {
        let mut pending = self.pending.lock().unwrap_or_else(PoisonError::into_inner);
        let same = |a: &HeldAsk| {
            a.run == ask.run && a.method == ask.method && a.host == ask.host && a.path == ask.path
        };
        if pending.values().any(same) {
            return None;
        }
        ask.id = self.next.fetch_add(1, Ordering::SeqCst) + 1;
        pending.insert(ask.id, ask.clone());
        Some(ask)
    }

    /// The ask `id`, removed: it can be performed once.
    fn take(&self, id: u64) -> Option<HeldAsk> {
        self.pending
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&id)
    }

    fn drop_run(&self, run: &str) {
        self.pending
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .retain(|_, ask| ask.run != run);
    }

    pub fn pending_for(&self, run: &str) -> Vec<HeldAsk> {
        let mut asks: Vec<HeldAsk> = self
            .pending
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .values()
            .filter(|ask| ask.run == run)
            .cloned()
            .collect();
        asks.sort_by_key(|ask| ask.id);
        asks
    }
}

// ---- the credential lookup ----------------------------------------------------------------

/// What one run's proxy asked of the sign-ins, kept for the words of its refusals.
#[derive(Default)]
struct SignInTrouble {
    last: Mutex<Option<(String, SignInFailure)>>,
}

/// A run's [`CredentialSource`]: the credential for the host and the account its project is
/// bound to. **A host with no bound account is a missing sign-in**, never another account's.
pub struct ProjectCredentials {
    chat: String,
    project: PathBuf,
    seams: Seams,
    trouble: Arc<SignInTrouble>,
    /// The chats already told, across their runs.
    told: Arc<Mutex<HashSet<String>>>,
}

impl CredentialSource for ProjectCredentials {
    fn credential_for(&self, request: &CredentialRequest) -> Result<Credential, CredentialError> {
        let host = forge_host(&request.host);
        let failure = match self.seams.recorded.account(&self.project, &host) {
            None => SignInFailure::missing(&host),
            Some(account) => match self.seams.credentials.credential(&host, &account) {
                Ok(credential) => return Ok(credential),
                Err(failure) => failure,
            },
        };
        let state = failure.state;
        *self
            .trouble
            .last
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some((host.clone(), failure));
        // One Notice per chat, however many requests and runs; none for a forge that could not
        // be reached, which is not the person's to fix.
        if state != SignInState::Unavailable
            && self
                .told
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .insert(self.chat.clone())
        {
            self.seams.notices.sign_in_needed(&self.chat, &host, state);
        }
        Err(CredentialError::Unavailable)
    }
}

// ---- the lifecycle ------------------------------------------------------------------------

/// The run's handler: the proxy, with held acts registered and raised.
struct RunHandler {
    run: String,
    proxy: Arc<ForgeProxy>,
    repos: Vec<String>,
    held: Arc<dyn HeldActs>,
    asks: Arc<HeldAsks>,
}

impl Handler for RunHandler {
    fn handle(&self, inbound: &Inbound<'_>) -> Outcome {
        let outcome = self.proxy.handle(&ChatId::new(&self.run), inbound);
        if let Outcome::Refused(refusal) = &outcome
            && let Some(act) = held_act(*refusal)
        {
            let path = inbound
                .head
                .target
                .split(['?', '#'])
                .next()
                .unwrap_or_default()
                .to_owned();
            let lower = path.to_ascii_lowercase();
            let repo = self
                .repos
                .iter()
                .find(|repo| {
                    lower.contains(&format!("/{repo}/")) || lower.ends_with(&format!("/{repo}"))
                })
                .cloned();
            let made = self.asks.register(HeldAsk {
                id: 0,
                run: self.run.clone(),
                host: forge_host(&inbound.head.host),
                method: inbound.head.method.to_ascii_uppercase(),
                path,
                repo,
                act,
            });
            if let Some(ask) = made {
                self.held.raise(&ask);
            }
        }
        outcome
    }
    fn body_need(&self, head: &RequestHead) -> BodyNeed {
        self.proxy.body_need(head)
    }
    fn note_advertised(&self, host: &str, repo_path: &str, branches: Vec<String>) {
        self.proxy
            .note_advertised(&ChatId::new(&self.run), host, repo_path, branches);
    }
    fn confirm_push(&self, forwarded: &Forwarded) {
        self.proxy.confirm_push(&ChatId::new(&self.run), forwarded);
    }
}

struct Live {
    chat: String,
    proxy: Arc<ForgeProxy>,
    handler: Arc<RunHandler>,
    project: PathBuf,
    trouble: Arc<SignInTrouble>,
    dir: PathBuf,
    /// The paired runner's device, for a run forwarded from one.
    peer: Option<String>,
    /// Dropped before the folder goes.
    bound: Option<Bound>,
}

/// Every run's route, in this app.
pub struct ForgeRoutes {
    root: PathBuf,
    helper_exe: PathBuf,
    seams: Seams,
    live: Mutex<HashMap<String, Live>>,
    by_session: Mutex<HashMap<u32, String>>,
    told: Arc<Mutex<HashSet<String>>>,
    asks: Arc<HeldAsks>,
}

impl ForgeRoutes {
    /// Routes made under `root`, the app's own private folder. Folders an earlier app left that
    /// belong to no running app are removed ([`sweep`]).
    pub fn new(root: PathBuf, helper_exe: PathBuf, seams: Seams) -> Self {
        if ensure_root(&root).is_ok() {
            sweep(&root);
        }
        Self {
            root,
            helper_exe,
            seams,
            live: Mutex::new(HashMap::new()),
            by_session: Mutex::new(HashMap::new()),
            told: Arc::new(Mutex::new(HashSet::new())),
            asks: Arc::new(HeldAsks::default()),
        }
    }

    /// A run starts. `None` when its project records no repo: there is nothing to proxy, and
    /// nothing else is stopped for it. An error leaves nothing behind.
    pub fn start(&self, chat: &str, run: &str, project: &Path) -> io::Result<Option<RouteParams>> {
        self.start_for(chat, run, project, None)
    }

    fn start_for(
        &self,
        chat: &str,
        run: &str,
        project: &Path,
        peer: Option<String>,
    ) -> io::Result<Option<RouteParams>> {
        let repos = self.seams.recorded.repos(project);
        if repos.is_empty() {
            return Ok(None);
        }
        ensure_root(&self.root)?;
        // The same run starting again replaces its route: the old sentinel stops working.
        self.stop(run);
        let run_dir = self.root.join(folder_name(run));
        make_private_dir(&run_dir)?;
        let trouble = Arc::new(SignInTrouble::default());
        let proxy = Arc::new(ForgeProxy::new(
            Box::new(ProjectCredentials {
                chat: chat.to_owned(),
                project: project.to_owned(),
                seams: self.seams.clone(),
                trouble: trouble.clone(),
                told: self.told.clone(),
            }),
            Box::new(NoObjects),
        ));
        let repo_view: Vec<(String, String)> = repos
            .iter()
            .map(|r| (r.host().to_owned(), r.path().to_owned()))
            .collect();
        let mut hosts: Vec<String> = repo_view.iter().map(|(h, _)| h.clone()).collect();
        hosts.sort();
        hosts.dedup();
        let id = ChatId::new(run);
        let sentinel = proxy.enrol(&id, repos);
        let handler = Arc::new(RunHandler {
            run: run.to_owned(),
            proxy: proxy.clone(),
            repos: repo_view
                .iter()
                .map(|(_, p)| p.to_ascii_lowercase())
                .collect(),
            held: self.seams.held.clone(),
            asks: self.asks.clone(),
        });
        let bound = match self
            .seams
            .listener
            .bind(run, &run_dir, handler.clone(), &hosts)
        {
            Ok(bound) => bound,
            Err(refused) => {
                proxy.retire(&id);
                let _ = std::fs::remove_dir_all(&run_dir);
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
            run.to_owned(),
            Live {
                chat: chat.to_owned(),
                proxy,
                handler,
                project: project.to_owned(),
                trouble,
                dir: run_dir,
                peer,
                bound: Some(bound),
            },
        );
        Ok(Some(params))
    }

    /// A start holding its route until it is committed: stopped if the start returns early.
    pub fn guard(&self, chat: &str, run: &str, project: &Path) -> RouteGuard<'_> {
        let params = match self.start(chat, run, project) {
            Ok(params) => params,
            Err(refused) => {
                tracing::warn!("purlis: the forge route of a chat did not start ({refused})");
                None
            }
        };
        RouteGuard {
            routes: self,
            run: run.to_owned(),
            params,
            armed: true,
        }
    }

    /// A run's route stops: its sentinel, its listener, its folder and its asks.
    pub fn stop(&self, run: &str) {
        let gone = self.live().remove(run);
        self.by_session
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .retain(|_, r| r != run);
        self.asks.drop_run(run);
        if let Some(mut live) = gone {
            live.proxy.retire(&ChatId::new(run));
            drop(live.bound.take());
            let _ = std::fs::remove_dir_all(&live.dir);
            // The chat is told again only by a run that starts after none of its runs is left.
            let chat_has_another = self.live().values().any(|other| other.chat == live.chat);
            if !chat_has_another {
                self.told
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .remove(&live.chat);
            }
        }
    }

    /// The route of the run that session `session` is, on its close, its program's end, or a
    /// restart's old run ending. Another run of the same chat is untouched.
    pub fn stop_session(&self, session: u32) {
        let run = self
            .by_session
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&session);
        if let Some(run) = run {
            self.stop(&run);
        }
    }

    /// Every route of `project`: the project is closing.
    pub fn stop_project(&self, project: &Path) {
        let runs: Vec<String> = self
            .live()
            .iter()
            .filter(|(_, live)| live.project == project)
            .map(|(run, _)| run.clone())
            .collect();
        for run in runs {
            self.stop(&run);
        }
    }

    /// Every route of the runner `device`: its link went.
    pub fn stop_peer(&self, device: &str) {
        let runs: Vec<String> = self
            .live()
            .iter()
            .filter(|(_, live)| live.peer.as_deref() == Some(device))
            .map(|(run, _)| run.clone())
            .collect();
        for run in runs {
            self.stop(&run);
        }
    }

    /// Every route, at the app's exit. Only this app's own folders are removed.
    pub fn stop_all(&self) {
        let runs: Vec<String> = self.live().keys().cloned().collect();
        for run in runs {
            self.stop(&run);
        }
    }

    pub fn is_live(&self, run: &str) -> bool {
        self.live().contains_key(run)
    }

    fn live(&self) -> std::sync::MutexGuard<'_, HashMap<String, Live>> {
        self.live.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// One request from `run`'s endpoint, decided through its [`Handler`].
    pub fn handle(&self, run: &str, inbound: &Inbound<'_>) -> Outcome {
        let handler = self.live().get(run).map(|live| live.handler.clone());
        match handler {
            Some(handler) => handler.handle(inbound),
            None => Outcome::Refused(Refusal::UnknownChat),
        }
    }

    /// The asks `run` has raised and nobody has answered.
    pub fn pending_asks(&self, run: &str) -> Vec<HeldAsk> {
        self.asks.pending_for(run)
    }

    /// The person pressed Allow on the registered ask `id`: performed once, with their
    /// credential for the ask's host and the project's account. **An ask that was not raised
    /// here, was performed already, or whose run ended, performs nothing.**
    pub fn perform_held(&self, id: u64) -> Result<(), String> {
        let ask = self
            .asks
            .take(id)
            .ok_or_else(|| "That request is not waiting for you any more.".to_owned())?;
        let project = self
            .live()
            .get(&ask.run)
            .map(|live| live.project.clone())
            .ok_or_else(|| "That chat has ended, so there is nothing to allow.".to_owned())?;
        let host = forge_host(&ask.host);
        let credential = self
            .seams
            .recorded
            .account(&project, &host)
            .ok_or_else(|| SignInFailure::missing(&host))
            .and_then(|account| self.seams.credentials.credential(&host, &account))
            .map_err(|failure| failure.said)?;
        self.seams.held.perform(&ask, credential)
    }

    /// What to tell the chat for `refusal`. A missing or ended sign-in names the step.
    pub fn refusal_message(&self, run: &str, refusal: Refusal) -> String {
        match refusal {
            Refusal::NoCredential => {
                let last = self.live().get(run).and_then(|live| {
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

    /// A runner's chat run, forwarded to this app's proxy (D9). **Only a runner paired with
    /// this desktop is served**, and the project is the pairing's record of the run, never the
    /// caller's word: an unpaired caller, or a run the pairing does not know, is refused. Its
    /// route stops with [`Self::stop`] or [`Self::stop_peer`].
    pub fn for_runner(&self, peer: &RunnerPeer, chat: &str, run: &str) -> RunnerForward {
        if !self.seams.runner_link.paired(peer) {
            return RunnerForward::Refused;
        }
        let Some(project) = self.seams.runner_link.project_of(peer, run) else {
            return RunnerForward::Refused;
        };
        match self.start_for(chat, run, &project, Some(peer.device.clone())) {
            Ok(Some(params)) => RunnerForward::Desktop(params),
            Ok(None) | Err(_) => RunnerForward::TokenConnections,
        }
    }

    /// The run's audit, a line per request: method, host, repo, class and verdict. The audit
    /// holds no token and no path, so nor does a line.
    pub fn audit_lines(&self, run: &str) -> Vec<String> {
        let proxy = self.live().get(run).map(|live| live.proxy.clone());
        proxy
            .map(|proxy| {
                proxy
                    .audit(&ChatId::new(run))
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
        let mut hosts: Vec<String> = repos.iter().map(|r| forge_host(r.host())).collect();
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

/// A start's hold on its route. Dropped without [`RouteGuard::commit`], it stops the route: every
/// `?` between the route's start and the chat being open ends the route with it.
#[must_use = "a dropped guard stops the route"]
pub struct RouteGuard<'a> {
    routes: &'a ForgeRoutes,
    run: String,
    params: Option<RouteParams>,
    armed: bool,
}

impl RouteGuard<'_> {
    /// What the run is handed (slice B), where it has a route.
    pub fn params(&self) -> Option<&RouteParams> {
        self.params.as_ref()
    }

    /// The chat is open as `session`: the route now stops with that session, by its close, its
    /// program's end or its project's. A run with no route commits nothing.
    pub fn commit(mut self, session: u32) {
        self.armed = false;
        if self.routes.is_live(&self.run) {
            self.routes
                .by_session
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .insert(session, self.run.clone());
        }
    }
}

impl Drop for RouteGuard<'_> {
    fn drop(&mut self) {
        if self.armed {
            self.routes.stop(&self.run);
        }
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
        SignInState::Unavailable => format!(
            "purlis could not reach {host} to use the sign-in just now. Try again in a moment."
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

// ---- the folder ---------------------------------------------------------------------------

/// A run id as the tail of a folder name: nothing that could climb out of the root.
fn safe_name(run: &str) -> String {
    run.chars()
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

/// `<pid>-<run>`: whose folder it is, so a sweep removes only a gone app's.
fn folder_name(run: &str) -> String {
    format!("{}-{}", std::process::id(), safe_name(run))
}

/// Whether `name` is a folder this module made: `<digits>-<safe name>`; its pid.
fn pid_of_folder(name: &str) -> Option<u32> {
    let (pid, rest) = name.split_once('-')?;
    let safe = !rest.is_empty()
        && rest
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if !safe || pid.is_empty() || !pid.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    pid.parse().ok()
}

/// Makes `root` this user's alone, or says why it cannot be: created `0700`, and refused unless
/// it is a directory (never a link) this user owns ([`purlis_same_user::private_directory`]).
fn ensure_root(root: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        purlis_same_user::private_directory(root)
    }
    #[cfg(not(unix))]
    {
        let _ = root;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "the forge route needs a unix socket",
        ))
    }
}

fn make_private_dir(dir: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new().mode(0o700).create(dir)
    }
    #[cfg(not(unix))]
    {
        std::fs::create_dir(dir)
    }
}

/// Removes the folders in `root` this module made for an app that is gone: a real directory
/// (never a link, never followed) owned by this user, named `<pid>-<run>`, whose pid is not a
/// running process. Anything else in `root` is left alone.
fn sweep(root: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let mine = purlis_same_user::Uid::effective().as_raw();
        let Ok(entries) = std::fs::read_dir(root) else {
            return;
        };
        for entry in entries.flatten() {
            let Ok(name) = entry.file_name().into_string() else {
                continue;
            };
            let Some(pid) = pid_of_folder(&name) else {
                continue;
            };
            if pid == std::process::id() || purlis_same_user::Parents::started(pid).is_some() {
                continue;
            }
            let path = entry.path();
            let Ok(found) = std::fs::symlink_metadata(&path) else {
                continue;
            };
            if found.is_dir() && !found.file_type().is_symlink() && found.uid() == mine {
                let _ = std::fs::remove_dir_all(&path);
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = root;
    }
}

/// Where the routes live: the app's private folder in this user's runtime or temporary
/// directory (short, because a unix socket's path may not pass 104 bytes on macOS), named for
/// the user.
pub fn route_dir() -> PathBuf {
    let base = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|dir| dir.is_absolute() && dir.is_dir())
        .unwrap_or_else(std::env::temp_dir);
    #[cfg(unix)]
    let who = purlis_same_user::Uid::effective().as_raw().to_string();
    #[cfg(not(unix))]
    let who = "user".to_owned();
    base.join(format!("purlis-forge-{who}"))
}

// ---- the app's one ------------------------------------------------------------------------

static ROUTES: std::sync::OnceLock<ForgeRoutes> = std::sync::OnceLock::new();

/// The app's routes, made once at setup, after the one-per-user lock (so a sweep never meets a
/// second app's folders). **Seams are the stubs until the slices named at the top fill them.**
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

/// A run starts (`chats.rs`): its route, held by a guard that stops it unless the chat opens.
/// **Slice B puts the params in the run's environment**; until it merges nothing is handed over.
pub fn guard(chat: &str, run: &str, project: &Path) -> Option<RouteGuard<'static>> {
    Some(routes()?.guard(chat, run, project))
}

/// A chat's session closes, or its program ends on its own (`chats.rs`).
pub fn stop_session(session: u32) {
    if let Some(routes) = routes() {
        routes.stop_session(session);
    }
}

/// A project closes (`Chats::end_all`).
pub fn stop_project(project: &Path) {
    if let Some(routes) = routes() {
        routes.stop_project(project);
    }
}

/// The app exits.
pub fn stop_all() {
    if let Some(routes) = routes() {
        routes.stop_all();
    }
}
