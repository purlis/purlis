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
//!   [`ForgeRoutes::stop_project`]). **A program that exits before the commit** (its exit
//!   arrives on another thread while the start is still opening the chat) is remembered, and the
//!   commit then stops the route instead of keeping it.
//! - **Per run** ([`ForgeRoutes::start`]): a run with recorded repos gets its own [`ForgeProxy`]
//!   (its credential lookup is its project's), a sentinel, a folder of mode 0700 in the app's own
//!   private root and, from the listener, a socket, an egress URL and a certificate authority.
//!   A run's repos not being recorded any more stops the route that same run had before.
//!   The [`RouteParams`] go to slice B's route.
//! - **The root is the app's alone** ([`ensure_root`]): made 0700 and refused unless it is a
//!   directory (never a link) this user owns. A folder is named `<pid>-<8 hex>` (short, because a
//!   unix socket's path may not pass 104 bytes on macOS: a route whose socket path would pass
//!   [`MAX_SOCKET_PATH`] is refused with that said), and the sweep ([`sweep`]) removes only
//!   folders of that shape, owned by this user, whose app is gone.
//! - **Credentials** ([`ProjectCredentials`]): the forge's host ([`forge_host`], one place) and the
//!   project's account, asked of slice C. A missing or ended sign-in refuses the request in
//!   words that name the sign-in step and raises one Notice per chat. A forge that could not be
//!   reached is said to be unreachable, not ended.
//! - **Held acts**: every request goes through [`Handler::handle`], so a refusal the person may
//!   allow registers an ask once ([`HeldAsks`]) and raises slice F's. Only a registered ask can
//!   be performed ([`ForgeRoutes::perform_held`]): one time, for the run and the request it was
//!   raised for, with the person's credential. **An ask names a recorded repo of its run** (read
//!   from the path, or, for GraphQL, from the ids the mutation names) or is not raised, and it
//!   is performed only while that repo is still one of the route's. Two different acts are two
//!   asks: each is keyed by the digest of its whole request.
//! - **Runners** ([`ForgeRoutes::for_runner`]): a runner's chat is forwarded to this app's proxy
//!   only for a runner paired with this desktop (D9), and the project is the pairing's, never
//!   the caller's.
//!
//! **Stubs, each filled by the slice named.** [`Listener`]: A2. [`RouteParams`] into the chat's
//! environment: B. [`HeldActs`] and its list of held refusals: F (F2's registry replaces
//! [`HeldAsks`]). [`Recorded`]: G. [`Notices`]'s sink: the app's ([`crate::forgewiring::AppNotices`]). [`RunnerLink`]:
//! the pools' pairing, read by [`pairings::DesktopPairings`]; **nothing calls [`ForgeRoutes::for_runner`]
//! yet** (the forwarding channel is ADR 0078's, not built), so a runner's chat keeps its token
//! connections.
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
use std::sync::{Arc, Mutex, PoisonError};

use purlis_core::forge::createdbranches::CreatedBranch;
use purlis_core::forge::proxy::listener::{PushGate, PushRead, PushVerdict};
use purlis_core::forge::proxy::{
    AllowedRepo, AuditRecord, ChatId, Class, Credential, CredentialError, CredentialRequest,
    CredentialSource, ForgeProxy, HeldSink, Inbound, Outcome, Refusal, TokenKind, Verdict,
};
use purlis_core::secrets::keyring::Secret;

pub mod objects;
pub mod pairings;
pub mod prod;

#[cfg(test)]
mod tests;

// ---- the seams ----------------------------------------------------------------------------

/// Why a person's sign-in could not be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
            account: Some(account.to_owned()),
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
    /// The tip the chat `chat` itself last put on `branch` of the repo with forge id `repo_id`
    /// on `host` through the route, from the app's own record, **asked at every update** and
    /// never kept: the route lets an update through only from this tip, so a branch another chat
    /// moved, or one made again, is refused. Any doubt is none (the person pushes it), and the
    /// project and the chat's id are the key, so a chat with another id, or of another project,
    /// owns none of them. **One exception**: a branch recorded to a chat `chat` dispatched
    /// (directly, or up the chain, by the app's dispatch records) is `chat`'s to update from the
    /// recorded tip, and its confirmed push is then recorded under `chat`.
    fn own_tip(
        &self,
        _project: &Path,
        _chat: &str,
        _host: &str,
        _repo_id: u64,
        _branch: &str,
    ) -> Option<String> {
        None
    }
    /// The branches of the repo with forge id `repo_id` on `host` whose record is `chat`'s now,
    /// **asked each time the forge's advertisement is read**: any of them the forge no longer
    /// lists has its record ended. Any doubt is none.
    fn own_branches(
        &self,
        _project: &Path,
        _chat: &str,
        _host: &str,
        _repo_id: u64,
    ) -> Vec<String> {
        Vec::new()
    }
    /// The forge confirmed that `chat` made or moved these branches, to these tips: keep that
    /// where no chat can write, keyed by the project's stable id, the chat and the forge's id of
    /// the repo.
    fn record_created(&self, _project: &Path, _chat: &str, _created: &[CreatedBranch]) {}
    /// The forge no longer lists these recorded branches: their record ends.
    fn forget_created(&self, _gone: &[CreatedBranch]) {}
    /// The chat that owns the branch `branch` of the repo with forge id `repo_id` on `host` now,
    /// for the Let this chat push ask to name. Any doubt is none.
    fn branch_owner(&self, _host: &str, _repo_id: u64, _branch: &str) -> Option<String> {
        None
    }
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

/// **Slice G's gate on a credential** (ADR 0128 D2, D4): a chat's request is given the person's
/// token only for a repo the person confirmed, by its numeric id, under the account they chose
/// for it, from a clone of this chat's ([`purlis_core::forge::credential::ChatRoute`]). Where
/// there is no gate (a test, or a process with no host scope), the project's bound account
/// answers as it did before.
pub trait Gate: Send + Sync {
    /// A run starts: these are its project's clones.
    fn enrol(&self, run: &str, project: &Path);
    /// A run ended.
    fn leave(&self, run: &str);
    /// The credential for `request`, if every condition holds.
    fn credential(&self, request: &CredentialRequest) -> Result<Credential, CredentialError>;
}

/// What a bound run endpoint holds. Dropping the guard closes the listener.
pub struct Bound {
    pub socket: PathBuf,
    /// The listener's own `CONNECT` address, `http://127.0.0.1:<port>`. A chat is not given it: it
    /// reaches the door through its own egress proxy, which is handed [`Bound::door`].
    pub egress_url: String,
    pub ca: PathBuf,
    /// The listener's `CONNECT` door, on loopback.
    pub door: std::net::SocketAddr,
    /// The run's hand-off secret for the door, given to the chat's egress proxy with its address.
    pub handoff: purlis_core::sandbox::egress::handoff::Secret,
    /// The hosts the door terminates.
    pub hosts: Vec<String>,
    pub guard: Box<dyn Send + Sync>,
}

/// **Slice A2's** listener: binds the run's socket and `CONNECT` door inside `dir`, makes the
/// run's own CA (its key in memory only) permitting the hosts the run's repos record, and serves
/// every request through `proxy`, which is the one door a request goes in by: a held act's
/// refusal registers and raises its ask in the proxy itself ([`ForgeProxy::with_held`]).
pub trait Listener: Send + Sync {
    /// `push` checks every push's objects before it is sent ([`PushGate`]).
    fn bind(
        &self,
        run: &str,
        dir: &Path,
        proxy: Arc<ForgeProxy>,
        push: Arc<dyn PushGate>,
    ) -> io::Result<Bound>;
    /// Whether it can bind at all in this build (the doctor's "proxy up").
    fn ready(&self) -> bool;
}

/// The stub until A2 lands: nothing listens.
pub struct NoListener;

impl Listener for NoListener {
    fn bind(
        &self,
        _: &str,
        _: &Path,
        _: Arc<ForgeProxy>,
        _: Arc<dyn PushGate>,
    ) -> io::Result<Bound> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "the forge listener is not in this build",
        ))
    }
    fn ready(&self) -> bool {
        false
    }
}

/// Where a Notice goes. The app's sink ([`crate::forgewiring::AppNotices`]) tells the chat's
/// pane, which draws it with a way out into Settings › You › Accounts; [`NoNotices`] is the stub
/// of a test or a build with no window.
pub trait Notices: Send + Sync {
    /// Told once per open chat `session` of `project`, host and state, with the host that had no
    /// usable sign-in. Never for [`SignInState::Unavailable`], which is not the person's to fix.
    /// **True where it was delivered**: a project with no pane to say it on returns false, and
    /// the route tells it again on its next request.
    fn sign_in_needed(&self, project: &Path, session: u32, host: &str, state: SignInState) -> bool;

    /// A credential for `host` was given to chat `session` of `project`: a Notice told for that
    /// host no longer stands.
    fn sign_in_ok(&self, project: &Path, session: u32, host: &str);
}

pub struct NoNotices;

impl Notices for NoNotices {
    fn sign_in_needed(&self, _: &Path, _: u32, _: &str, _: SignInState) -> bool {
        false
    }

    fn sign_in_ok(&self, _: &Path, _: u32, _: &str) {}
}

/// **Slice F's** held acts, as the route needs them: where a project's held acts are asked. The
/// sink is F's [`purlis_core::heldact::Handoff`] over the project's registry, and `sessions` says
/// which session a run's chat is, once it is open.
pub trait HeldSinks: Send + Sync {
    fn sink(&self, project: &Path, sessions: Arc<dyn SessionOfRun>) -> Option<Box<dyn HeldSink>>;
    /// The check of a push's objects for a CI-file push: it reads the push in a quarantine of the
    /// app's own, hands a push that changes CI files to `proxy` as an ask, and lets through only
    /// what changes none. None where the project holds no acts.
    fn push_gate(
        &self,
        project: &Path,
        sessions: Arc<dyn SessionOfRun>,
        proxy: Arc<ForgeProxy>,
    ) -> Option<Arc<dyn PushGate>>;
    /// A chat's session closed: the quarantines of its waiting pushes go.
    fn forget(&self, _session: u32) {}
}

/// **The gate that sends no push**, for a project that holds no acts: a push cannot be read for CI
/// files there, so none goes (a fetch, a comment or a pull request is not a push).
pub struct BlindGate;

impl PushGate for BlindGate {
    fn check(&self, _: &ChatId, _: &PushRead) -> PushVerdict {
        PushVerdict::Refuse(
            "purlis cannot check this push for CI files here, so it was not sent.".to_owned(),
        )
    }
}

/// Which session a run's chat is, while it is open.
pub trait SessionOfRun: Send + Sync {
    fn session(&self, run: &str) -> Option<u32>;
    /// Whether `session` is a chat that is open now.
    fn is_open(&self, session: u32) -> bool;
}

/// The stub where no project holds acts: a held act stays refused.
pub struct NoHeldSinks;

impl HeldSinks for NoHeldSinks {
    fn sink(&self, _: &Path, _: Arc<dyn SessionOfRun>) -> Option<Box<dyn HeldSink>> {
        None
    }
    fn push_gate(
        &self,
        _: &Path,
        _: Arc<dyn SessionOfRun>,
        _: Arc<ForgeProxy>,
    ) -> Option<Arc<dyn PushGate>> {
        None
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
    /// The project `run` of `chat` on `peer` belongs to, from the pairing's records. None where
    /// the pairing records no project, or records it for another chat than `chat`.
    fn project_of(&self, peer: &RunnerPeer, chat: &str, run: &str) -> Option<PathBuf>;
}

/// No runner is paired: only [`Seams::stubs`]'s default. The app installs
/// [`pairings::DesktopPairings`].
pub struct NoRunnerLink;

impl RunnerLink for NoRunnerLink {
    fn paired(&self, _: &RunnerPeer) -> bool {
        false
    }
    fn project_of(&self, _: &RunnerPeer, _: &str, _: &str) -> Option<PathBuf> {
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
    pub held: Arc<dyn HeldSinks>,
    pub runner_link: Arc<dyn RunnerLink>,
    /// Slice G's gate, where there is one ([`Gate`]).
    pub gate: Option<Arc<dyn Gate>>,
    /// The administrator's never-allow pin for a project's chats (ADR 0128 D3g), made for each
    /// run and asked by its proxy, its door, its socket and the app's own git on every request.
    pub pin: PinOf,
    /// How the project's `guards.merge` runs for a run's merges (ADR 0128 D8, 2026-11-22), read
    /// by its proxy on each merge. The stub holds every merge.
    pub merges: MergesOf,
}

/// What makes the pin a run is held to, from its project.
pub type PinOf = Arc<dyn Fn(&Path) -> Arc<dyn purlis_core::forge::pin::Pin> + Send + Sync>;

/// What makes the merge stance a run is held to, from its project and the strictest policy along
/// the chain that dispatched its chat (`None` for a chat nobody dispatched).
pub type MergesOf = Arc<
    dyn Fn(&Path, Option<&str>) -> Arc<dyn purlis_core::forge::mergestance::MergeStance>
        + Send
        + Sync,
>;

impl Seams {
    /// Every seam as its stub: a build with `forge-wiring` and none of the other slices.
    pub fn stubs() -> Self {
        Self {
            listener: Arc::new(NoListener),
            credentials: Arc::new(NoSignIns),
            recorded: Arc::new(NothingRecorded),
            notices: Arc::new(NoNotices),
            held: Arc::new(NoHeldSinks),
            runner_link: Arc::new(NoRunnerLink),
            gate: None,
            pin: Arc::new(|_| Arc::new(purlis_core::forge::pin::NoPin)),
            merges: Arc::new(|_, _| Arc::new(purlis_core::forge::mergestance::HoldMerges)),
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
    /// What the chat's sandbox is wired from (slice B), with the listener's `CONNECT` door, which
    /// the chat's egress proxy hands a forge connection to.
    pub inputs: Box<purlis_core::forgewire::Inputs>,
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
    run: String,
    sessions: Arc<dyn SessionOfRun>,
    project: PathBuf,
    seams: Seams,
    trouble: Arc<SignInTrouble>,
    /// What is already told, mirroring what the pane shows: per open chat session and host, the
    /// state told. So a restarted chat's new session, a second host, and a state that changes
    /// (missing, ended, missing again) are each told.
    told: Arc<Mutex<Told>>,
}

/// The Notices told: the state of each open chat session's host.
type Told = HashMap<(u32, String), SignInState>;

impl ProjectCredentials {
    /// A credential was given for `host`: a Notice told to this chat for it is taken down.
    fn signed_in(&self, host: &str) {
        let Some(session) = self.sessions.session(&self.run) else {
            return;
        };
        let cleared = {
            let mut told = self.told.lock().unwrap_or_else(PoisonError::into_inner);
            told.remove(&(session, host.to_owned())).is_some()
        };
        if cleared {
            self.seams.notices.sign_in_ok(&self.project, session, host);
        }
    }
}

impl CredentialSource for ProjectCredentials {
    fn credential_for(&self, request: &CredentialRequest) -> Result<Credential, CredentialError> {
        let host = forge_host(&request.host);
        // Where there is a gate, it decides first: a repo that is not confirmed for this chat, or
        // whose chosen account cannot serve it, is given nothing, whatever the project's bound
        // account could have answered.
        let gated = self
            .seams
            .gate
            .as_ref()
            .map(|gate| gate.credential(request));
        if let Some(Ok(credential)) = gated {
            self.signed_in(&host);
            return Ok(credential);
        }
        let lookup = match self.seams.recorded.account(&self.project, &host) {
            None => Err(SignInFailure::missing(&host)),
            Some(account) => self.seams.credentials.credential(&host, &account),
        };
        let failure = match (gated, lookup) {
            (None, Ok(credential)) => {
                self.signed_in(&host);
                return Ok(credential);
            }
            // A sign-in that is missing or ended names its step, as it always did.
            (_, Err(failure)) => failure,
            (Some(_), Ok(_)) => SignInFailure {
                state: SignInState::Unavailable,
                said: format!(
                    "purlis did not use a sign-in for this repo on {host}: it is not one you \
                     confirmed for this project, or its chosen account cannot serve it now. Check \
                     Settings › You › Accounts, then try again."
                ),
            },
        };
        let state = failure.state;
        *self
            .trouble
            .last
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some((host.clone(), failure));
        // One Notice per open chat, host and state, however many requests; none for a forge that
        // could not be reached, which is not the person's to fix. A chat that is not open yet, or
        // whose pane is not there, has nowhere to say it, so it is not counted as told: its next
        // request tells it.
        if state != SignInState::Unavailable
            && let Some(session) = self.sessions.session(&self.run)
        {
            let key = (session, host.clone());
            let new = self
                .told
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .get(&key)
                != Some(&state);
            if new
                && self
                    .seams
                    .notices
                    .sign_in_needed(&self.project, session, &host, state)
            {
                self.told
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .insert(key, state);
                // The chat may have closed while the pane was told, after its told state was
                // forgotten: nothing of it stays, here or on its pane.
                if !self.sessions.is_open(session) {
                    self.told
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .remove(&(session, host.clone()));
                    self.seams.notices.sign_in_ok(&self.project, session, &host);
                }
            }
        }
        Err(CredentialError::Unavailable)
    }
}

// ---- the lifecycle ------------------------------------------------------------------------

/// [`SessionOfRun`] over the committed runs.
struct RunSessions {
    sessions: Arc<Mutex<Sessions>>,
}

impl SessionOfRun for RunSessions {
    fn session(&self, run: &str) -> Option<u32> {
        self.sessions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .of_run
            .iter()
            .find(|(_, of)| of.as_str() == run)
            .map(|(session, _)| *session)
    }

    fn is_open(&self, session: u32) -> bool {
        self.sessions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .of_run
            .contains_key(&session)
    }
}

struct Live {
    chat: String,
    proxy: Arc<ForgeProxy>,
    project: PathBuf,
    trouble: Arc<SignInTrouble>,
    dir: PathBuf,
    /// The paired runner's device, for a run forwarded from one.
    peer: Option<String>,
    /// Dropped before the folder goes.
    bound: Option<Bound>,
    /// What the run's sandbox was wired from: the app's own git for this chat ([`ForgeRoutes::git_route`])
    /// goes through the same door.
    inputs: Box<purlis_core::forgewire::Inputs>,
    /// The app's own git's way to the door: a proxy that proves the hand-off and asks the pin
    /// (the git cannot prove it itself). `None` where it could not start, and then the app's git
    /// has no route for this chat.
    relay: Option<purlis_core::sandbox::egress::relay::DoorRelay>,
}

/// Which session each committed run is, and the sessions whose program ended while a start was
/// still opening its chat: the start commits after its program may already have exited, on
/// another thread, and an exit that finds nothing to stop must not be lost.
#[derive(Default)]
struct Sessions {
    of_run: HashMap<u32, String>,
    /// Sessions that ended with an uncommitted route pending; kept only while one is.
    exited: HashSet<u32>,
}

/// How long the app's git may sit idle on a relayed connection.
const RELAY_IDLE: std::time::Duration = std::time::Duration::from_secs(120);

/// Every run's route, in this app.
pub struct ForgeRoutes {
    root: PathBuf,
    helper_exe: PathBuf,
    seams: Seams,
    /// Held for the whole of [`Self::start_for`], so the check of who a run belongs to and the
    /// insert of its route are one step. **Lock order: `starting`, then `live`, then
    /// `sessions`.** Nothing under it starts a route (the seams are called, none re-enter), and
    /// `stop` and the others never take it.
    starting: Mutex<()>,
    live: Mutex<HashMap<String, Live>>,
    sessions: Arc<Mutex<Sessions>>,
    told: Arc<Mutex<Told>>,
}

/// How a start ended: it made a route (or had nothing to proxy), or the run was another owner's.
enum Started {
    Route(Option<RouteParams>),
    /// A runner asked for a run live for another peer or a desktop chat: nothing was changed.
    Taken,
}

impl Started {
    /// The route made. A start with no peer never finds its run taken, so `Taken` is `None`.
    fn into_route(self) -> Option<RouteParams> {
        match self {
            Self::Route(params) => params,
            Self::Taken => None,
        }
    }
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
            starting: Mutex::new(()),
            live: Mutex::new(HashMap::new()),
            sessions: Arc::new(Mutex::new(Sessions::default())),
            told: Arc::new(Mutex::new(Told::new())),
        }
    }

    /// The project's recorded repos that are not on a forge the pin pins.
    fn unpinned(
        &self,
        project: &Path,
        pin: &dyn purlis_core::forge::pin::Pin,
    ) -> Vec<purlis_core::forge::proxy::AllowedRepo> {
        let mut repos = self.seams.recorded.repos(project);
        repos.retain(|repo| {
            purlis_core::forge::pin::pinned_forge(pin, repo.host(), repo.port()).is_none()
        });
        repos
    }

    /// A run starts. `None` when its project records no repo: there is nothing to proxy, and
    /// nothing else is stopped for it. An error leaves nothing behind.
    pub fn start(&self, chat: &str, run: &str, project: &Path) -> io::Result<Option<RouteParams>> {
        self.start_for(chat, run, project, None, None)
            .map(Started::into_route)
    }

    fn start_for(
        &self,
        chat: &str,
        run: &str,
        project: &Path,
        peer: Option<String>,
        chain: Option<&str>,
    ) -> io::Result<Started> {
        // One start at a time: the check below and the insert at the end are atomic against
        // another start, so two peers asking for one run cannot both pass the check.
        let _starting = self.starting.lock().unwrap_or_else(PoisonError::into_inner);
        // **A runner never takes another route**: a run live for another peer or a desktop chat
        // is refused before anything of it is stopped.
        if let Some(device) = peer.as_deref() {
            let taken = self
                .live()
                .get(run)
                .is_some_and(|live| live.peer.as_deref() != Some(device));
            if taken {
                return Ok(Started::Taken);
            }
        }
        // The same run starting again replaces its route, or ends it where nothing is recorded
        // now: the old sentinel stops working either way.
        self.stop(run);
        let pin = (self.seams.pin)(project);
        // **No repo on a pinned forge is enrolled** (D3g): a chat has no route to a forge an
        // administrator pinned, and its authority is made for no name of it.
        let repos = self.unpinned(project, pin.as_ref());
        if repos.is_empty() {
            return Ok(Started::Route(None));
        }
        ensure_root(&self.root)?;
        let run_dir = make_run_dir(&self.root)?;
        let trouble = Arc::new(SignInTrouble::default());
        let credentials = Arc::new(ProjectCredentials {
            chat: chat.to_owned(),
            run: run.to_owned(),
            sessions: Arc::new(RunSessions {
                sessions: self.sessions.clone(),
            }),
            project: project.to_owned(),
            seams: self.seams.clone(),
            trouble: trouble.clone(),
            told: self.told.clone(),
        });
        // A GraphQL mutation that names a pull request by node id is tied to its repo by asking
        // the forge, with this run's own credential (`objects`).
        let objects = objects::RunObjects::new(
            ChatId::new(run),
            repos.clone(),
            Arc::new(objects::ForgeLookup::new(credentials.clone())),
        );
        let mut proxy = ForgeProxy::new(Box::new(objects::Shared(credentials)), Box::new(objects))
            .with_pin(pin.clone())
            .with_merge_stance((self.seams.merges)(project, chain))
            .with_refusals(refusals_counted_in(project.to_owned()));
        // Each branch the forge confirms this chat's push made is recorded by the app, under this
        // chat's id: the brokered `purlis worktree push` goes through this same route.
        {
            let recorded = self.seams.recorded.clone();
            let chat = chat.to_owned();
            let project = project.to_owned();
            let forgets = recorded.clone();
            let tips = recorded.clone();
            let listed = recorded.clone();
            let (tip_chat, tip_project) = (chat.clone(), project.clone());
            let (list_chat, list_project) = (chat.clone(), project.clone());
            // A branch is this chat's to move only from the tip it last put there, read from the
            // app's record at each update: a push of another chat through the route moves it, so
            // a branch someone made again is refused, whatever this run read or was told at start.
            proxy = proxy
                .with_tips(Arc::new(move |host, repo_id, branch| {
                    tips.own_tip(&tip_project, &tip_chat, host, repo_id, branch)
                }))
                .with_recorded(Arc::new(move |host, repo_id| {
                    listed.own_branches(&list_project, &list_chat, host, repo_id)
                }))
                .with_created(Box::new(move |created| {
                    recorded.record_created(&project, &chat, created);
                }))
                .with_gone(Box::new(move |gone| forgets.forget_created(gone)));
        }
        // An update of a branch this chat has no record of raises one ask, Let this chat push it
        // (ADR 0128, amended 2026-11-23): in the window, where only the person's press records
        // an owner. The chat is its session while it is open.
        {
            let sessions = RunSessions {
                sessions: self.sessions.clone(),
            };
            let run = run.to_owned();
            let owners = self.seams.recorded.clone();
            proxy = proxy.with_branch_ask(Box::new(purlis_core::windowonly::PushAsks {
                root: project.to_owned(),
                chat: chat.to_owned(),
                session: Box::new(move || sessions.session(&run)),
                owner_of: Box::new(move |host, repo_id, branch| {
                    owners.branch_owner(host, repo_id, branch)
                }),
            }));
        }
        // Held acts (ADR 0128 D8) are asked by the proxy itself, through the project's own
        // registry of them, so no request reaches the forge by a way that skips the hold.
        if let Some(sink) = self.seams.held.sink(
            project,
            Arc::new(RunSessions {
                sessions: self.sessions.clone(),
            }),
        ) {
            proxy = proxy.with_held(sink);
        }
        let proxy = Arc::new(proxy);
        let repo_view: Vec<(String, String)> = repos
            .iter()
            .map(|r| (r.host().to_owned(), r.path().to_owned()))
            .collect();
        // The repos as the chat's wiring names them. One that wiring cannot name (a host with a
        // port, a path with an odd character) is left to the policy alone: it is wired to nothing.
        let wire_repos: Vec<purlis_core::forgewire::Repo> = repos
            .iter()
            .filter_map(|r| {
                let (owner, name) = r.path().rsplit_once('/')?;
                let kind = match r.kind() {
                    purlis_core::forge::proxy::Kind::GitHub => {
                        purlis_core::forgewire::ForgeKind::GitHub
                    }
                    purlis_core::forge::proxy::Kind::GitLab => {
                        purlis_core::forgewire::ForgeKind::GitLab
                    }
                };
                purlis_core::forgewire::Repo::new(r.host(), owner, name, kind).ok()
            })
            .collect();
        let id = ChatId::new(run);
        if let Some(gate) = &self.seams.gate {
            gate.enrol(run, project);
        }
        let sentinel = proxy.enrol(&id, repos.clone());
        let sessions = Arc::new(RunSessions {
            sessions: self.sessions.clone(),
        });
        let push = self
            .seams
            .held
            .push_gate(project, sessions, proxy.clone())
            .unwrap_or_else(|| Arc::new(BlindGate));
        // Reading the repo's objects starts now, not at the first push (#7532).
        push.warm();
        let bound = match self.seams.listener.bind(run, &run_dir, proxy.clone(), push) {
            Ok(bound) => bound,
            Err(refused) => {
                proxy.retire(&id);
                if let Some(gate) = &self.seams.gate {
                    gate.leave(run);
                }
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
            inputs: Box::new(purlis_core::forgewire::Inputs {
                gh_socket: bound.socket.clone(),
                ca_cert: bound.ca.clone(),
                sentinel: purlis_core::forgewire::SecretString::from(sentinel.expose().to_owned()),
                helper_exe: self.helper_exe.clone(),
                repos: wire_repos,
                door: bound.door,
                handoff: bound.handoff.clone(),
                hosts: bound.hosts.clone(),
                root: self.root.clone(),
            }),
        };
        let inputs = params.inputs.clone();
        // The app's own git reaches the door through a relay that proves the hand-off (the door
        // answers 407 to a CONNECT without it) and asks the pin on each connection.
        let relay = purlis_core::sandbox::egress::relay::DoorRelay::start(
            purlis_core::sandbox::egress::ForgeRoute {
                addr: inputs.door,
                handoff: inputs.handoff.clone(),
                hosts: inputs.hosts.clone(),
            },
            pin,
            RELAY_IDLE,
        )
        .map_err(|why| {
            tracing::warn!("purlis: the app's git has no route for a chat ({why})");
            why
        })
        .ok();
        self.live().insert(
            run.to_owned(),
            Live {
                chat: chat.to_owned(),
                proxy,
                project: project.to_owned(),
                trouble,
                dir: run_dir,
                peer,
                bound: Some(bound),
                inputs,
                relay,
            },
        );
        Ok(Started::Route(Some(params)))
    }

    /// A start holding its route until it is committed: stopped if the start returns early.
    pub fn guard(
        &self,
        chat: &str,
        run: &str,
        project: &Path,
        chain: Option<&str>,
    ) -> RouteGuard<'_> {
        let params = match self.start_for(chat, run, project, None, chain) {
            Ok(started) => started.into_route(),
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

    /// **A chat put back from the session host** (#7694): its program runs on, and the route of
    /// the app that started it went with that app, so the app's own git had none for it
    /// ([`Self::git_route`]) and `purlis worktree push` was refused. A route this app already holds
    /// for the run is kept as it is, since the chat's sandbox may be wired to it; otherwise one is
    /// started by [`Self::start_for`], through every gate a start's goes through (the recorded
    /// repos, the pin, the person's sign-ins). The running program is handed nothing: it holds no
    /// sentinel of the new route, and its own git keeps what it had.
    pub fn guard_reattached(
        &self,
        chat: &str,
        run: &str,
        project: &Path,
        chain: Option<&str>,
    ) -> RouteGuard<'_> {
        if self.is_live(run) {
            return RouteGuard {
                routes: self,
                run: run.to_owned(),
                params: None,
                armed: false,
            };
        }
        let mut guard = self.guard(chat, run, project, chain);
        guard.params = None;
        guard
    }

    /// A run's route stops: its sentinel, its listener, its folder and its asks.
    pub fn stop(&self, run: &str) {
        let gone = self.live().remove(run);
        {
            let mut sessions = self.sessions();
            let ended: Vec<u32> = sessions
                .of_run
                .iter()
                .filter_map(|(session, r)| (r == run).then_some(*session))
                .collect();
            sessions.of_run.retain(|_, r| r != run);
            self.forget_exits_if_none_pending(&mut sessions);
            self.forget_told(&ended);
        }
        if let Some(mut live) = gone {
            live.proxy.retire(&ChatId::new(run));
            if let Some(gate) = &self.seams.gate {
                gate.leave(run);
            }
            drop(live.bound.take());
            let _ = std::fs::remove_dir_all(&live.dir);
        }
    }

    /// The route of the run that session `session` is, on its close, its program's end, or a
    /// restart's old run ending. Another run of the same chat is untouched.
    ///
    /// **If no run is committed to `session` yet**, and a start is still holding a route it has
    /// not committed, the session is remembered as ended: the program's exit arrives on its own
    /// thread, and the start commits afterwards ([`RouteGuard::commit`]), which then stops the
    /// route it was about to keep.
    pub fn stop_session(&self, session: u32) {
        let run = {
            let mut sessions = self.sessions();
            let run = sessions.of_run.remove(&session);
            self.forget_told(&[session]);
            if run.is_none() && self.has_pending(&sessions) {
                sessions.exited.insert(session);
            }
            run
        };
        self.seams.held.forget(session);
        if let Some(run) = run {
            self.stop(&run);
        }
    }

    /// What was told of `ended` sessions is no longer kept: a session's Notice goes with it, and a
    /// restart's new session is told afresh.
    fn forget_told(&self, ended: &[u32]) {
        self.told
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .retain(|(session, _), _| !ended.contains(session));
    }

    fn sessions(&self) -> std::sync::MutexGuard<'_, Sessions> {
        self.sessions.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Whether a desktop route is live that no session holds yet: a start still opening its chat.
    /// A runner's route is never committed to a session, so it is not one.
    fn has_pending(&self, sessions: &Sessions) -> bool {
        let committed: HashSet<&String> = sessions.of_run.values().collect();
        self.live()
            .iter()
            .any(|(run, live)| live.peer.is_none() && !committed.contains(run))
    }

    fn forget_exits_if_none_pending(&self, sessions: &mut Sessions) {
        if !sessions.exited.is_empty() && !self.has_pending(sessions) {
            sessions.exited.clear();
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

    /// The folder `run`'s route is made in, while it is live.
    pub fn folder_of(&self, run: &str) -> Option<PathBuf> {
        self.live().get(run).map(|live| live.dir.clone())
    }

    fn live(&self) -> std::sync::MutexGuard<'_, HashMap<String, Live>> {
        self.live.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// One request from `run`'s endpoint, decided by its proxy.
    pub fn handle(&self, run: &str, inbound: &Inbound<'_>) -> Outcome {
        let proxy = self.live().get(run).map(|live| live.proxy.clone());
        match proxy {
            Some(proxy) => proxy.handle(&ChatId::new(run), inbound),
            None => Outcome::Refused(Refusal::UnknownChat),
        }
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
            _ => "purlis does not let a chat do that on this forge.".to_owned(),
        }
    }

    /// A runner's chat run, forwarded to this app's proxy (D9). **Only a runner paired with
    /// this desktop is served**, and the project is the link's record of the run, never the
    /// caller's word. The link ([`RunnerLink`]) refuses an unpaired caller, a chat the pairing
    /// does not name for that runner, and a project it has none for; it knows no runs, so a
    /// `run` it has no project for is refused only by being unnamed. **A runner never takes
    /// another route**: `start_for` refuses, under its lock, a `run` already live for another
    /// peer or for a desktop chat, so a runner cannot stop or replace it (a run of the same peer
    /// starting again replaces its own route). Its route stops with [`Self::stop`] or
    /// [`Self::stop_peer`].
    pub fn for_runner(&self, peer: &RunnerPeer, chat: &str, run: &str) -> RunnerForward {
        if !self.seams.runner_link.paired(peer) {
            return RunnerForward::Refused;
        }
        let Some(project) = self.seams.runner_link.project_of(peer, chat, run) else {
            return RunnerForward::Refused;
        };
        // A project whose every repo is on a pinned forge keeps no token connection either: the
        // pin is the administrator's, and the runner's chat is refused, not left to its tokens.
        let pin = (self.seams.pin)(&project);
        if !self.seams.recorded.repos(&project).is_empty()
            && self.unpinned(&project, pin.as_ref()).is_empty()
        {
            return RunnerForward::Refused;
        }
        match self.start_for(
            chat,
            run,
            &project,
            Some(peer.device.clone()),
            // A runner's chat carries no record of its chain here, so its merges are held.
            Some(&purlis_core::enforcement::Resolved::strict().spelled()),
        ) {
            Ok(Started::Route(Some(params))) => RunnerForward::Desktop(params),
            Ok(Started::Taken) => RunnerForward::Refused,
            Ok(Started::Route(None)) | Err(_) => RunnerForward::TokenConnections,
        }
    }

    /// **The route the app's own git takes for chat `session`** (one login, slice K): what `purlis
    /// worktree push` and `update` run for a chat through, so the git the app runs for it reaches
    /// the forge through this run's proxy with its sentinel, as the chat's own git does, and holds
    /// none of the person's credentials. `None` where the session has no live route: a chat with
    /// no recorded repo, one on a pool's runner put back from the session host, one whose route
    /// stopped.
    pub fn git_route(
        &self,
        session: u32,
    ) -> Option<std::io::Result<purlis_core::worktree::git::NetworkRoute>> {
        let run = self.sessions().of_run.get(&session).cloned()?;
        self.live().get(&run).map(Self::route_of)
    }

    /// The route git takes for a live run, read from the one snapshot of it the caller holds.
    fn route_of(live: &Live) -> std::io::Result<purlis_core::worktree::git::NetworkRoute> {
        // **Never the bare door**: it answers 407 to a CONNECT that lacks the hand-off proof, which
        // git cannot send. The relay proves it, and asks the administrator's pin on each connection.
        let Some(relay) = live.relay.as_ref().map(|relay| relay.addr()) else {
            return Err(io::Error::other(
                "purlis's own git has no route to this chat's forge: its relay did not start",
            ));
        };
        let route = live.inputs.route(format!("http://{relay}"));
        purlis_core::forgewire::git_network(&route)
    }

    /// **The ref tips of the remote at `url`, asked for chat `session`'s hook** (#7302): the
    /// pre-push scan's ask of the remote, which a chat's sandbox cannot make under the one login.
    /// The app asks it here, outside the sandbox, on the run's own route ([`Self::git_route`])
    /// with the run's own CA, and only where `url` is the address of a repository recorded for the
    /// run the session belongs to: never another host or repository, and never another run's
    /// route. The error is the sentence the hook's chat is told.
    pub fn remote_refs(&self, session: u32, url: &str) -> Result<Vec<String>, String> {
        self.remote_refs_asking(session, url, purlis_core::diffscan::remote_tips_through)
    }

    /// [`Self::remote_refs`], with `ask` as what asks the remote (a test's stand-in for git).
    /// The run's repos and its route are read from one look at it, so a run that stops between
    /// the two cannot be checked for one and asked on another's route.
    fn remote_refs_asking(
        &self,
        session: u32,
        url: &str,
        ask: impl FnOnce(&str, purlis_core::worktree::git::NetworkRoute) -> Option<Vec<String>>,
    ) -> Result<Vec<String>, String> {
        let no_route = || "this chat has no forge route".to_owned();
        let run = self
            .sessions()
            .of_run
            .get(&session)
            .cloned()
            .ok_or_else(no_route)?;
        // Only a recorded repository's route is built.
        let route = self
            .live()
            .get(&run)
            .map(|live| {
                live.inputs
                    .repos
                    .iter()
                    .any(|repo| repo.is_url(url))
                    .then(|| Self::route_of(live))
            })
            .ok_or_else(no_route)?;
        let Some(route) = route else {
            return Err("that remote is not a repository recorded for this chat".to_owned());
        };
        let route = route.map_err(|why| why.to_string())?;
        ask(url, route)
            .ok_or_else(|| "the remote could not be asked which commits it has".to_owned())
    }

    /// The run's audit, a line per request: method, host, repo, class and verdict. The audit
    /// holds no token and no path, so nor does a line.
    pub fn audit_lines(&self, run: &str) -> Vec<String> {
        self.audit_records(run).iter().map(audit_line).collect()
    }

    fn audit_records(&self, run: &str) -> Vec<AuditRecord> {
        let proxy = self.live().get(run).map(|live| live.proxy.clone());
        proxy
            .map(|proxy| proxy.audit(&ChatId::new(run)))
            .unwrap_or_default()
    }

    /// **The doctor's `forge blocks` row for `project`**: the newest [`MOST_REFUSALS`] requests
    /// its running chats' routes refused, a line each with the chat's name and the rule that
    /// stopped it, so a person can read why `gh` or `git` in a chat was refused. The audit keeps
    /// no token and no path (the lines [`audit_lines`](Self::audit_lines) shows), and it lives
    /// in memory: a chat that ended has no lines.
    pub fn refusals_row(&self, project: &Path) -> RouteRow {
        let runs: Vec<(String, String)> = self
            .live()
            .iter()
            .filter(|(_, live)| live.project == project)
            .map(|(run, live)| (run.clone(), live.chat.clone()))
            .collect();
        let mut refused: Vec<String> = Vec::new();
        for (run, chat) in runs {
            for record in self.audit_records(&run) {
                if matches!(record.verdict, Verdict::Refused(_)) {
                    refused.push(format!("{chat}: {}", audit_line(&record)));
                }
            }
        }
        if refused.is_empty() {
            return RouteRow::ok("no running chat has had a request blocked by its forge route");
        }
        let skipped = refused.len().saturating_sub(MOST_REFUSALS);
        let mut shown = refused.split_off(skipped);
        if skipped > 0 {
            shown.insert(0, format!("{skipped} older blocked requests not shown"));
        }
        RouteRow {
            problem: false,
            detail: shown.join("; "),
            hint: "each line is a chat, the request (method, host, repo, kind) and why it was \
                   blocked; a chat's route blocks anything it does not list"
                .to_owned(),
        }
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
        let ended = {
            let mut sessions = self.routes.sessions();
            let ended = sessions.exited.remove(&session);
            if !ended && self.routes.is_live(&self.run) {
                sessions.of_run.insert(session, self.run.clone());
            }
            self.routes.forget_exits_if_none_pending(&mut sessions);
            ended
        };
        // Its program ended before this commit: the route goes now, not at the app's exit.
        if ended {
            self.routes.stop(&self.run);
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

/// How many blocked requests the doctor's `forge blocks` row lists, the newest.
pub const MOST_REFUSALS: usize = 5;

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

/// **Where the forge refusals of a route in the project at `project` are counted** (#7779): each
/// is one line in the chat's enforcement log, which Settings › Project › Policy counts under
/// "Would have blocked": its kind and act, and the `host:port`, never a path, a body or a number.
/// The proxy tells it a refusal once a minute for each chat, rule and `host:port`. This is the
/// app, so the line is written here ([`purlis_core::enforcement::write_line`], what `record` does
/// in the app).
fn refusals_counted_in(project: PathBuf) -> purlis_core::forge::proxy::RefusalSink {
    Box::new(move |chat, refused| {
        purlis_core::enforcement::write_line(
            &project,
            chat.as_str(),
            purlis_core::enforcement::Family::GuardsForge,
            refused.rule,
            Some(&refused.host),
        );
    })
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

/// The host a line shows: only a host name (`[A-Za-z0-9.-]`, at most 80 bytes, labels of at most
/// 63, never the sentinel's prefix) with an optional `:port`, else a fixed placeholder. The host
/// is the chat's own word, and a path, a query, its sentinel, bidi controls or `; …` in it would
/// forge a line of the doctor's row.
const UNREADABLE_HOST: &str = "(unreadable host)";

fn shown_host(host: &str) -> &str {
    let (name, port) = match host.split_once(':') {
        Some((name, port)) => (name, Some(port)),
        None => (host, None),
    };
    // A DNS label is at most 63 bytes, and the chat's own sentinel (80 bytes of host-name
    // characters) is refused by its prefix, so neither reaches the row.
    let name_ok = !name.is_empty()
        && name.len() <= 80
        && name.split('.').all(|label| label.len() <= 63)
        && !name.to_ascii_lowercase().starts_with("purlis-sentinel-")
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-');
    let port_ok = port.is_none_or(|port| {
        (1..=5).contains(&port.len()) && port.bytes().all(|b| b.is_ascii_digit())
    });
    if name_ok && port_ok {
        host
    } else {
        UNREADABLE_HOST
    }
}

/// The method as a word: letters only, else a placeholder (it is the chat's own word too).
fn shown_method(method: &str) -> &str {
    if !method.is_empty() && method.bytes().all(|b| b.is_ascii_alphabetic()) {
        method
    } else {
        "(unreadable method)"
    }
}

fn class_words(class: &Class) -> &'static str {
    match class {
        Class::GitRefs => "git ref listing",
        Class::GitFetch => "git fetch",
        Class::GitPush => "git push",
        Class::ApiRead => "API read",
        Class::PublicRead => "archive or release download",
        Class::PullRequestWrite => "pull request change",
        Class::MergeWrite => "merge",
        Class::CommentWrite => "comment",
        Class::IssueWrite => "issue change",
        Class::LabelWrite => "label change",
        Class::CiWrite => "CI rerun or cancel",
        Class::GraphqlQuery => "GraphQL query",
        Class::LfsBatch => "Git LFS download request",
        Class::UploadWrite => "file upload",
        Class::GraphqlMutation(_) => "GraphQL mutation",
    }
}

fn refusal_words(refusal: Refusal) -> &'static str {
    match refusal {
        Refusal::NoSentinel => "it carried no credential",
        Refusal::WrongSentinel => "its credential was not this chat's",
        Refusal::Locked => {
            "too many wrong credentials, so the chat is locked out until it restarts"
        }
        Refusal::UnknownChat => "the chat has no route any more",
        Refusal::MalformedRequest => "the request could not be read one way",
        Refusal::HostNotAllowed => "the host is not one of the chat's repos' hosts",
        Refusal::AmbiguousHost => {
            "the chat has repos on more than one host, so which is meant is unclear"
        }
        Refusal::OtherRepo => "the repo is not one of the chat's",
        Refusal::Merge => "merging is not allowed from a chat",
        Refusal::MergeCommand => "the text tells the forge or a bot to merge or approve",
        Refusal::QuickAction => "the text holds a quick action",
        Refusal::LabelById => "labels were named by id",
        Refusal::Approval => "approving or reviewing is not allowed from a chat",
        Refusal::ReleaseOrTag => "releases and tags are not allowed from a chat",
        Refusal::Ruleset => "rules over the repo are not allowed from a chat",
        Refusal::Admin => "administering the repo is not allowed from a chat",
        Refusal::ProtectedRef => {
            "the push is to a protected branch or a branch the chat did not create"
        }
        Refusal::NotYourBranch => {
            "the push updates a branch this chat has no record of (the person can let it)"
        }
        Refusal::PushOption => "the push carried push options",
        Refusal::BodyUnreadable => "the body was missing, too big or could not be read",
        Refusal::GraphqlParse => "the GraphQL document could not be read as one request",
        Refusal::ProtectedPullHead => {
            "the pull request's head is the default or a protected branch, or not a branch of the chat's repo that purlis can see"
        }
        Refusal::GraphqlUnknownMutation => "the GraphQL mutation is not on the list",
        Refusal::GraphqlOtherRepo | Refusal::GraphqlPullRequestOtherRepo => {
            "the GraphQL document reaches a repo that is not the chat's"
        }
        Refusal::CredentialInQuery => "a credential was in the query string",
        Refusal::Multipart => "a multipart body is not allowed here",
        Refusal::CredentialInBody => "a credential was in the request body",
        Refusal::NoAdvertisement => "purlis does not know the repo's branches yet",
        Refusal::OAuthFlow => "signing in at the forge is not allowed from a chat",
        Refusal::Lfs => "Git LFS is not allowed from a chat",
        Refusal::NotOnList => "that call is not on the list of what a chat may do",
        Refusal::NoCredential => "purlis had no sign-in to use for it",
        Refusal::Pinned => "an administrator's policy does not allow this forge for chats",
    }
}

fn audit_line(record: &AuditRecord) -> String {
    format!(
        "{} {} {} {} {}",
        shown_method(&record.method),
        shown_host(&record.host),
        record.repo.as_deref().unwrap_or("-"),
        record.class.as_ref().map_or("-", class_words),
        match record.verdict {
            Verdict::Allowed => "allowed".to_owned(),
            Verdict::Refused(why) => format!("blocked ({})", refusal_words(why)),
        },
    )
}

// ---- the folder ---------------------------------------------------------------------------

/// The most bytes a run's socket path may have. A unix socket's path holds 104 bytes on macOS
/// (108 on Linux) with its terminator, so this keeps a margin.
pub const MAX_SOCKET_PATH: usize = 100;

/// The name the listener gives a run's socket in the run's folder (slice A2's `bind`).
pub const SOCKET_NAME: &str = "proxy.sock";

/// `<pid>-<8 hex>`: whose folder it is, so a sweep removes only a gone app's, and a name short
/// enough for a socket's path. The hex is random: a run's id is not in it (the app keeps which
/// folder is whose), and a crashed app's folder of a reused pid is never met by it.
fn folder_name() -> io::Result<String> {
    let mut random = [0u8; 4];
    getrandom::fill(&mut random).map_err(io::Error::other)?;
    let id: String = random.iter().map(|b| format!("{b:02x}")).collect();
    Ok(format!("{}-{id}", std::process::id()))
}

/// Whether `name` is a folder this module made: `<digits>-<8 hex>`; its pid.
fn pid_of_folder(name: &str) -> Option<u32> {
    let (pid, id) = name.split_once('-')?;
    let is_id = id.len() == 8
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    if !is_id || pid.is_empty() || !pid.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    pid.parse().ok()
}

/// A new private folder for a run in `root`, refused, before it is made, when the socket the
/// listener will bind in it would not fit a unix socket's path.
fn make_run_dir(root: &Path) -> io::Result<PathBuf> {
    let mut tries = 0;
    loop {
        let dir = root.join(folder_name()?);
        let socket = dir.join(SOCKET_NAME);
        let length = socket.as_os_str().len();
        if length > MAX_SOCKET_PATH {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "the forge route's socket path would be {length} bytes, over the {MAX_SOCKET_PATH} \
                     a unix socket allows ({}); purlis needs a shorter folder for it",
                    socket.display()
                ),
            ));
        }
        match make_private_dir(&dir) {
            Ok(()) => return Ok(dir),
            Err(refused) if refused.kind() == io::ErrorKind::AlreadyExists && tries < 4 => {
                tries += 1;
            }
            Err(refused) => return Err(refused),
        }
    }
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
    sweep_with(root, &process_may_run);
}

/// Whether process `pid` may be running. **Only "no such process" says it is gone**: a process
/// this one may not signal (`EPERM`: another user's process, or the macOS sandbox), or any other
/// answer, is taken as running, so a live app's folders are never removed on a guess. Another pid
/// namespace answers `ESRCH`, not `EPERM`, so there the one-per-user instance lock is what keeps
/// two apps from sweeping each other. A dead app's folder stays until its pid is free again, and
/// a reused pid keeps it longer: it fails closed. Where this process may not look at all, a
/// folder is left, never removed.
#[cfg(unix)]
fn process_may_run(pid: u32) -> bool {
    use rustix::process::{Pid, test_kill_process};
    let Some(pid) = i32::try_from(pid).ok().and_then(Pid::from_raw) else {
        return true;
    };
    probe_says_running(test_kill_process(pid))
}

#[cfg(not(unix))]
fn process_may_run(_: u32) -> bool {
    true
}

/// A `kill(pid, 0)` answer, read: gone only for `ESRCH`.
#[cfg(unix)]
fn probe_says_running(answer: Result<(), rustix::io::Errno>) -> bool {
    !matches!(answer, Err(rustix::io::Errno::SRCH))
}

/// [`sweep`] with its liveness probe given, so a test can answer as another namespace would.
fn sweep_with(root: &Path, may_run: &dyn Fn(u32) -> bool) {
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
            if pid == std::process::id() || may_run(pid) {
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
        let _ = (root, may_run);
    }
}

/// Where the routes live: the app's private folder, short because a unix socket's path may not
/// pass 104 bytes on macOS. This user's runtime directory where there is one (Linux); without it
/// on Linux, this user's own machine folder (not a shared directory); on macOS,
/// the per-user temporary directory the system makes private. `None` where there is none of
/// these, and then no chat has a route.
pub fn route_dir() -> Option<PathBuf> {
    // **One resolver** with the sandbox's, which holds every chat out of this folder
    // ([`purlis_core::forgewire::route_root`]): the folder the app makes and the folder a chat is
    // denied are the same one, whatever `XDG_RUNTIME_DIR` or `TMPDIR` say.
    purlis_core::forgewire::route_root(&purlis_core::sandbox::Machine::this())
}

/// [`route_dir`] for the directories it was given.
fn route_dir_in(
    runtime: Option<PathBuf>,
    machine: Option<PathBuf>,
    temporary: PathBuf,
    who: &str,
    linux: bool,
) -> Option<PathBuf> {
    purlis_core::forgewire::route_dir_in(runtime, machine, temporary, who, linux)
}

// ---- the app's one ------------------------------------------------------------------------

static ROUTES: std::sync::OnceLock<ForgeRoutes> = std::sync::OnceLock::new();

/// The app's routes, made once at setup, after the one-per-user lock (so a sweep never meets a
/// second app's folders). **Seams are the stubs until the slices named at the top fill them.**
pub fn install(held: Arc<dyn HeldSinks>, notices: Arc<dyn Notices>) {
    let Some(root) = route_dir() else {
        tracing::warn!("purlis: no private folder for the forge route; chats keep their tokens");
        return;
    };
    let _ = ROUTES.get_or_init(|| {
        let mut seams = Seams::stubs();
        seams.held = held;
        seams.notices = notices;
        // Slice C's sign-ins and slice G's confirmed repos, where this process may read them (only
        // the app's own, never a process a chat started) and a config root exists. Without them
        // no repo is recorded, so no chat has a route.
        if let (Some(scope), Some(config)) = (
            purlis_core::forge::route::HostScope::claim(),
            purlis_core::machine::config_root(),
        ) {
            seams.credentials = Arc::new(AccountCredentials::new(
                purlis_core::forge::credential::SignedIn::new(&scope, &config),
            ));
            seams.recorded = Arc::new(prod::ConfirmedRecorded::new(config.clone()));
            seams.gate = Some(Arc::new(prod::ChatGate::new(
                purlis_core::forge::credential::chatroute::ChatRoute::new(&scope, &config),
            )));
            seams.listener = Arc::new(prod::CoreListener);
            seams.runner_link = Arc::new(pairings::DesktopPairings::from_store(&config));
            // Read from policy on each ask: a pin set while a chat is open reaches it at once.
            seams.pin =
                Arc::new(|project| Arc::new(purlis_core::forge::pin::PolicyPin::of(project)));
            // Likewise `guards.merge`: `log` and `off` let a merge through, `enforce` holds it.
            seams.merges = Arc::new(|project, chain| {
                Arc::new(purlis_core::forge::mergestance::PolicyMergeStance::of(
                    project, chain,
                ))
            });
        }
        ForgeRoutes::new(root, std::env::current_exe().unwrap_or_default(), seams)
    });
}

/// Where a held CI-file push's quarantines are made: a folder of the app's own, in the machine
/// folder no chat writes, beside its route folders. Removed with each push's ask, and swept with
/// the route folders' sweep when the app next starts.
pub fn quarantine_dir() -> PathBuf {
    route_dir()
        .map(|root| root.join("quarantine"))
        .unwrap_or_else(|| std::env::temp_dir().join("purlis-quarantine"))
}

/// Where the forge repos' object caches are kept (ADR 0128, amended 2026-11-24): in the app's data
/// folder on this machine, which survives a reboot and is no chat's (the sandbox holds every chat
/// out of it, reading and writing, by the same resolver). `None` where the machine has no data
/// folder: a push then fetches its base into its own quarantine.
pub fn object_cache_dir() -> Option<PathBuf> {
    purlis_core::forgewire::object_cache_root(&purlis_core::sandbox::Machine::this())
}

/// The app's routes, once [`install`] has run.
pub fn routes() -> Option<&'static ForgeRoutes> {
    ROUTES.get()
}

/// A run starts (`chats.rs`): its route, held by a guard that stops it unless the chat opens.
/// **Slice B puts the params in the run's environment**; until it merges nothing is handed over.
pub fn guard(
    chat: &str,
    run: &str,
    project: &Path,
    chain: Option<&str>,
) -> Option<RouteGuard<'static>> {
    Some(routes()?.guard(chat, run, project, chain))
}

/// A chat put back from the session host (`chats.rs`): the route the app's own git takes for it
/// ([`ForgeRoutes::guard_reattached`]), with nothing handed to its program.
pub fn guard_reattached(
    chat: &str,
    run: &str,
    project: &Path,
    chain: Option<&str>,
) -> Option<RouteGuard<'static>> {
    Some(routes()?.guard_reattached(chat, run, project, chain))
}

/// The route the app's own git takes for chat `session` ([`ForgeRoutes::git_route`]).
pub fn git_route(session: u32) -> Option<io::Result<purlis_core::worktree::git::NetworkRoute>> {
    routes()?.git_route(session)
}

/// The ref tips of a recorded remote, for chat `session`'s hook ([`ForgeRoutes::remote_refs`]).
pub fn remote_refs(session: u32, url: &str) -> Result<Vec<String>, String> {
    routes()
        .ok_or_else(|| "this chat has no forge route".to_owned())?
        .remote_refs(session, url)
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
    // No quarantine outlives the app: each holds a repository a held push was read in.
    purlis_core::heldact::sweep_quarantines(&quarantine_dir());
}
