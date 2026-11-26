// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Held acts on the forge route, in the app** (one login, slice K; ADR 0128 D8): what a project's
//! held acts need to read of the forge and the clone ([`purlis_core::heldact::HeldContext`]), the
//! check of a push's objects that decides whether a push goes on ([`CiPushGate`]), and the
//! [`HeldSinks`] that gives each route its project's registry of asks.
//!
//! - **Which chat is which session** is the route's ([`SessionOfRun`]): an ask is raised only by
//!   a chat that is open.
//! - **The forge is read and written as the person**, with the sign-in of the account they chose
//!   for the repo ([`AccountSource`]), never with a chat's sentinel.
//! - **A push's objects are read in a quarantine** of the app's own (`heldact::Quarantine`): a
//!   fresh repository, the base fetched from the forge with the person's credential, the chat's
//!   pack taken in by git, which hashes every object. **One quarantine per push**, kept for its
//!   ask, so the ask, the press and the files the push sends are read from the same objects
//!   ([`Quarantines`]). A push goes on only when [`check_push_since`] says it changes no CI file; a
//!   refusal, or an unreadable `ci_config_path`, sends nothing.
//!
//! **Tier** (ADR 0069): the quarantines are Machine, device-bound and rebuildable
//! (`docs/plane-format.md`, "A held CI-file push's quarantine"); the asks are memory only.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use purlis_core::forge::accounts::{self, ForgeAccount};
use purlis_core::forge::confirmed::{self, Admission, ConfirmedRepo, Serving};
use purlis_core::forge::credential::{
    CredentialError as SourceError, CredentialSource as HostSource, SignedIn,
};
use purlis_core::forge::held::{CredentialTokens, WorkflowTransports, held_forge};
use purlis_core::forge::http::TokenSource;
use purlis_core::forge::proxy::listener::{PushGate, PushRead, PushVerdict};
use purlis_core::forge::proxy::{ChatId, ForgeProxy, HeldSink};
use purlis_core::forge::route::HostScope;
use purlis_core::forge::transport::Transport;
use purlis_core::forge::{Forge, ForgeError};
use purlis_core::forgewire::{ExposeSecret, SecretString};
use purlis_core::heldact::{
    Handoff, HeldContext, NoClone, OwnedWorld, PushCheck, Quarantine, Recorded, Requested,
    TipFiles, Trees, Unavailable, WorkflowCredential, check_push_since,
};
use purlis_core::mergehold::{Base, HeadSha, Repo, Request};
use purlis_core::secrets::keyring::Secret;

use crate::forgeroute::{HeldSinks, Notices, SessionOfRun, SignInState};
use crate::planes::Held;

/// The most quarantines kept for pushes that wait, in all: each is a repository on disk.
const MOST_KEPT: usize = 16;

/// The most quarantine fetches in flight at once, in all: each is a `git fetch` as the person and a
/// repository on disk, and a chat can push as fast as it likes.
const MOST_FETCHING: usize = 4;

/// A chat's session and the repo a push is of.
type Key = (u32, String);

/// **The quarantines of the pushes that wait**, by the chat's session and the repo: the push an ask
/// was made from is the push a press reads again.
#[derive(Default)]
pub struct Quarantines {
    kept: Mutex<Vec<(Key, Arc<Quarantine>)>>,
    fetching: AtomicUsize,
}

/// A place among the [`MOST_FETCHING`], given back when dropped.
struct Fetching<'a>(&'a AtomicUsize);

impl Drop for Fetching<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

impl Quarantines {
    /// A place to fetch in, or `None` when [`MOST_FETCHING`] are fetching already.
    fn begin_fetching(&self) -> Option<Fetching<'_>> {
        self.fetching
            .try_update(Ordering::AcqRel, Ordering::Acquire, |now| {
                (now < MOST_FETCHING).then_some(now + 1)
            })
            .ok()
            .map(|_| Fetching(&self.fetching))
    }

    fn keep(&self, session: u32, repo: &str, quarantine: Quarantine) {
        let mut kept = self.kept.lock().unwrap_or_else(PoisonError::into_inner);
        let key = (session, repo.to_ascii_lowercase());
        kept.retain(|(of, _)| *of != key);
        // The oldest goes first, and its repository with it.
        while kept.len() >= MOST_KEPT {
            kept.remove(0);
        }
        kept.push((key, Arc::new(quarantine)));
    }

    fn of(&self, session: u32, repo: &str) -> Option<Arc<Quarantine>> {
        let key = (session, repo.to_ascii_lowercase());
        self.kept
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .find(|(of, _)| *of == key)
            .map(|(_, quarantine)| quarantine.clone())
    }

    /// The push of chat `session` to `repo` waits on nobody: its quarantine goes.
    fn release(&self, session: u32, repo: &str) {
        let key = (session, repo.to_ascii_lowercase());
        self.kept
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .retain(|(of, _)| *of != key);
    }

    /// A chat's session closed: none of its pushes waits.
    pub fn forget(&self, session: u32) {
        self.kept
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .retain(|((of, _), _)| *of != session);
    }
}

/// The person's token for one account, read for each request so a renewal is picked up.
struct AccountSource {
    signed_in: SignedIn,
    account: String,
}

impl HostSource for AccountSource {
    fn credential_for(
        &self,
        host: &str,
    ) -> Result<purlis_core::forge::credential::Credential, SourceError> {
        self.signed_in
            .credential_of(host, &self.account, crate::sandboxing::now_secs())
    }
}

/// A token for one request.
struct Fixed(SecretString);

impl TokenSource for Fixed {
    fn token(&self) -> Result<SecretString, ForgeError> {
        Ok(self.0.clone())
    }
}

/// **Slice G's second token** as the transport of a held CI-file push.
struct WorkflowOver {
    config_root: PathBuf,
    account: ForgeAccount,
}

impl WorkflowCredential for WorkflowOver {
    fn available(&self, _: &str) -> Result<(), Unavailable> {
        use accounts::workflow::{Availability, availability};
        let keyring = accounts::keyring_at(&self.config_root);
        match availability(keyring.as_ref(), &self.account) {
            Availability::Ready => Ok(()),
            Availability::NeedsPress => Err(Unavailable(
                "pushing a CI file needs a second sign-in that grants the workflow permission; \
                 make it in Settings › You › Accounts, then try again"
                    .to_owned(),
            )),
            Availability::NotOffered => Err(Unavailable(
                "this account's everyday sign-in already holds the workflow permission, so purlis \
                 does not push CI files for it: push this one from your own terminal"
                    .to_owned(),
            )),
        }
    }

    fn secret(&self, _: &str) -> Result<Secret, Unavailable> {
        use accounts::workflow::{CiPushCredential, for_ci_push};
        let scope = HostScope::claim()
            .ok_or_else(|| Unavailable("only the app itself may make this push".to_owned()))?;
        let keyring = accounts::keyring_at(&self.config_root);
        match for_ci_push(&scope, keyring.as_ref(), &self.account) {
            Ok(CiPushCredential::Token(token)) => Ok(Secret::new(token.expose_secret().to_owned())),
            Ok(_) => Err(Unavailable(
                "the CI permission is not made for this account".to_owned(),
            )),
            Err(why) => Err(Unavailable(why)),
        }
    }

    fn refused(&self, _: &str) {
        if let Some(scope) = HostScope::claim() {
            self.drop_refused(&scope);
        }
    }
}

impl WorkflowOver {
    /// [`WorkflowCredential::refused`] once the host's scope is in hand: the dead second token is
    /// deleted, and the window told. Best effort: the ask has ended whatever the keychain says,
    /// and the next push reads the item again. The keychain's words name the item, never the
    /// token.
    fn drop_refused(&self, scope: &HostScope) {
        let keyring = accounts::keyring_at(&self.config_root);
        if let Err(why) = accounts::workflow::refused(scope, keyring.as_ref(), &self.account) {
            tracing::warn!(account = %self.account.id, "a refused second sign-in stays: {why}");
        }
    }
}

impl WorkflowTransports for WorkflowOver {
    fn with(&self, forge: &Forge, secret: &Secret) -> Result<Arc<dyn Transport>, String> {
        let token = Arc::new(Fixed(secret.to_secret_string()));
        accounts::over_https(forge, token).map_err(|why| why.to_string())
    }
}

/// **The repos the person confirmed for this project's chats**, as the held acts recognise them.
struct RecordedRepos {
    repos: Vec<Repo>,
}

impl Recorded for RecordedRepos {
    fn resolve(&self, _: purlis_core::forge::Kind, id: &str) -> Option<Repo> {
        let id = id.to_ascii_lowercase();
        self.repos
            .iter()
            .find(|repo| repo.as_str().eq_ignore_ascii_case(&id))
            .cloned()
    }

    fn holds(&self, repo: &Repo) -> bool {
        self.repos
            .iter()
            .any(|have| have.as_str().eq_ignore_ascii_case(repo.as_str()))
    }
}

/// A repo served as the person: its confirmed entry, its account and its clone.
struct Served {
    repo: ConfirmedRepo,
    account: ForgeAccount,
    clone: PathBuf,
}

/// The one of `candidates` whose path is `path`: none is a refusal, and so is one confirmed on
/// more than one host, which a path alone cannot tell apart.
fn pick(
    candidates: Vec<(ConfirmedRepo, PathBuf)>,
    path: &str,
) -> Result<(ConfirmedRepo, PathBuf), String> {
    let mut named = candidates
        .into_iter()
        .filter(|(have, _)| have.path.eq_ignore_ascii_case(path));
    let Some(first) = named.next() else {
        return Err(format!("{path} is not a repo confirmed for this project"));
    };
    if named.any(|(other, _)| !other.host.eq_ignore_ascii_case(&first.0.host)) {
        return Err(format!(
            "{path} is confirmed on more than one forge, and purlis cannot tell which this is"
        ));
    }
    Ok(first)
}

/// Whether `repo` is the one at `host` and `port`.
fn is_at(repo: &ConfirmedRepo, host: &str, port: u16) -> bool {
    let (bare, at) = confirmed::address_of(&repo.host);
    bare.eq_ignore_ascii_case(host) && at == port
}

/// **What a project's held acts read** ([`HeldContext`]).
pub struct ProjectContext {
    project: PathBuf,
    config_root: PathBuf,
    sessions: Arc<dyn SessionOfRun>,
    quarantines: Arc<Quarantines>,
    held: std::sync::Weak<Held>,
}

impl ProjectContext {
    /// Every confirmed repo of the project's clones, with its clone.
    fn candidates(&self) -> Vec<(ConfirmedRepo, PathBuf)> {
        crate::forgeroute::prod::clones_of(&self.project)
            .into_iter()
            .flat_map(|clone| {
                confirmed::repos_of_clone(&self.config_root, &clone)
                    .unwrap_or_default()
                    .into_iter()
                    .map(move |repo| (repo, clone.clone()))
            })
            .collect()
    }

    /// **The one repo `repo` is**, served as the person now, as a chat's own route serves it
    /// (`ChatRoute`): the clone trusted and the repo confirmed by its numeric id
    /// ([`confirmed::serves`]), and its chosen account signed in and allowed
    /// ([`confirmed::account_for`]). A path confirmed on more than one forge is refused: a path is
    /// not a repo, and the held acts name a repo by its path alone.
    fn served(&self, repo: &Repo) -> Result<Served, String> {
        let (confirmed, clone) = pick(self.candidates(), repo.as_str())?;
        match confirmed::serves(
            &self.config_root,
            &clone,
            confirmed.kind,
            &confirmed.host,
            &confirmed.path,
            confirmed.repo_id,
        )? {
            Admission::Confirmed(_) | Admission::Alias(_) => {}
            Admission::NotConfirmed => {
                return Err(format!(
                    "{} is not confirmed for this project's clone now",
                    repo.as_str()
                ));
            }
        }
        let listed = accounts::listed(&self.config_root)?;
        match confirmed::account_for(&self.config_root, &confirmed, &listed) {
            Serving::Account(account) => Ok(Served {
                repo: confirmed,
                account,
                clone,
            }),
            _ => Err("the account chosen for this repo cannot serve it now".to_owned()),
        }
    }

    /// The forge as the person, for `repo`, reading a push's files from `quarantine` where there
    /// is one.
    fn forge_of(
        &self,
        served: &Served,
        quarantine: Option<&Quarantine>,
    ) -> Result<(Box<dyn purlis_core::heldact::HeldForge>, ForgeAccount), String> {
        let account = served.account.clone();
        let scope = HostScope::claim()
            .ok_or_else(|| "only the app itself acts as the person".to_owned())?;
        let signed_in = SignedIn::new(&scope, &self.config_root);
        let forge = Forge {
            kind: account.kind,
            host: account.host.clone(),
        };
        let tokens = Arc::new(CredentialTokens::new(
            &account.host,
            Arc::new(AccountSource {
                signed_in,
                account: account.id.clone(),
            }),
        ));
        let person = crate::forgeaccounts::token_check(&forge, tokens)
            .map_err(|why| format!("purlis could not reach {}: {why}", account.host))?;
        let files: Arc<dyn TipFiles> = match quarantine {
            Some(quarantine) => Arc::new(quarantine.trees()),
            None => Arc::new(NoClone),
        };
        let workflow = Arc::new(WorkflowOver {
            config_root: self.config_root.clone(),
            account: account.clone(),
        });
        Ok((held_forge(forge, person, files, workflow), account))
    }
}

/// Nobody's session: for a read made outside a chat's run.
struct NoSessions;

impl SessionOfRun for NoSessions {
    fn session(&self, _: &str) -> Option<u32> {
        None
    }

    fn is_open(&self, _: u32) -> bool {
        false
    }
}

/// **The tip of `branch` in the repo `repo_id` at `host` as the forge shows it to the person
/// now** (ADR 0128, amended 2026-11-23): what Let this chat push records as the branch's tip.
/// Read with the sign-in of the account the person chose for the repo, never a chat's, and only
/// for a repo the project confirmed **by the numeric id the ask was raised at**, so a repo made
/// again under the name is not the one the person was asked about. A branch the forge no longer
/// has is an error, not a tip.
pub fn branch_tip_as_the_person(
    project: &Path,
    config_root: &Path,
    host: &str,
    repo_path: &str,
    repo_id: u64,
    branch: &str,
) -> Result<String, String> {
    let context = ProjectContext {
        project: project.to_owned(),
        config_root: config_root.to_owned(),
        sessions: Arc::new(NoSessions),
        quarantines: Arc::new(Quarantines::default()),
        held: std::sync::Weak::new(),
    };
    let repo = Repo::try_from(repo_path).map_err(|_| format!("{repo_path} is not a repo name"))?;
    let base = Base::try_from(branch).map_err(|_| format!("{branch} is not a branch name"))?;
    let served = context.served(&repo)?;
    if !confirmed::address_of(&served.repo.host)
        .0
        .eq_ignore_ascii_case(host)
        || served.repo.repo_id != repo_id
    {
        return Err(format!(
            "{repo_path} is not the repo this ask was raised for, so no branch of it is recorded"
        ));
    }
    let (forge, _) = context.forge_of(&served, None)?;
    match forge.branch_tip(&repo, &base)? {
        Some(tip) => Ok(tip.as_str().to_owned()),
        None => Err(format!(
            "{branch} is not on {repo_path} any more, so there is nothing to record"
        )),
    }
}

impl HeldContext for ProjectContext {
    fn session(&self, chat: &ChatId) -> Option<u32> {
        self.sessions.session(chat.as_str())
    }

    /// The person's window is this app's: where it holds the project, a person can answer.
    fn attended(&self) -> bool {
        self.held.upgrade().is_some()
    }

    fn now(&self) -> u64 {
        crate::sandboxing::now_secs()
    }

    fn world(&self, session: u32, repo: &Repo) -> Result<OwnedWorld, String> {
        // At the ask and again at each press: a chat that has closed is nobody's to act for.
        if !self.sessions.is_open(session) {
            return Err("this chat is not open".to_owned());
        }
        let served = self.served(repo)?;
        let quarantine = self.quarantines.of(session, repo.as_str());
        let (forge, account) = self.forge_of(&served, quarantine.as_deref())?;
        let trees: Arc<dyn Trees> = match quarantine.as_deref() {
            Some(quarantine) => Arc::new(quarantine.trees()),
            None => Arc::new(NoClone),
        };
        let recorded: Vec<Repo> = self
            .candidates()
            .iter()
            .filter_map(|(have, _)| Repo::try_from(have.path.as_str()).ok())
            .collect();
        Ok(OwnedWorld {
            forge: Arc::from(forge),
            trees,
            workflow: Arc::new(WorkflowOver {
                config_root: self.config_root.clone(),
                account,
            }),
            recorded: Arc::new(RecordedRepos { repos: recorded }),
        })
    }

    fn merge(&self, chat: &ChatId, request: &Request) -> String {
        let Some(session) = self.session(chat) else {
            return "purlis did not hold this merge: this chat is not open.".to_owned();
        };
        let Some(cwd) = request
            .repo
            .as_ref()
            .and_then(|repo| self.served(repo).ok())
            .map(|served| served.clone)
        else {
            return "purlis did not hold this merge: it names no repo of this project.".to_owned();
        };
        let ask = purlis_core::hookwire::MergeAsk {
            chat: session,
            request: request.clone(),
            cwd: cwd.display().to_string(),
        };
        match crate::mergeholds::asked(&self.held, &ask) {
            purlis_core::hookwire::Answer::MergeHeld { .. } => purlis_core::heldact::said::asked(
                &purlis_core::heldact::Requested::Merge(request.clone()),
            ),
            purlis_core::hookwire::Answer::No { why } => {
                format!("purlis did not hold this merge: {why}")
            }
            _ => "purlis did not hold this merge.".to_owned(),
        }
    }
}

/// **The check of a push's objects** (ADR 0128 D8.3), made before the listener sends a push on.
pub struct CiPushGate {
    context: Arc<ProjectContext>,
    proxy: Arc<ForgeProxy>,
    quarantines: Arc<Quarantines>,
    parent: PathBuf,
    /// Where the forge repos' object caches are: the machine's data folder, which no chat reads or
    /// writes. `None` where the machine has none, and a push then fetches into its quarantine.
    cache_parent: Option<PathBuf>,
}

impl CiPushGate {
    /// The object cache of the repo `served` names.
    fn cache_of(&self, served: &Served) -> Option<Arc<purlis_core::heldact::ObjectCache>> {
        let parent = self.cache_parent.as_ref()?;
        Some(purlis_core::heldact::ObjectCache::of(
            parent,
            &served.repo.host,
            &served.repo.path,
        ))
    }

    fn refuse(why: impl Into<String>) -> PushVerdict {
        PushVerdict::Refuse(why.into())
    }
}

impl PushGate for CiPushGate {
    /// Starts the first fill of the object cache of each repo this project confirms, on a thread
    /// of the app's own: signing in and reading the keyring are not the chat's start to wait for.
    /// A repo that cannot be served now is left to its first push, which says why.
    fn warm(&self) {
        if self.cache_parent.is_none() {
            return;
        }
        let (context, cache_parent) = (self.context.clone(), self.cache_parent.clone());
        let gate = CiPushGate {
            context,
            proxy: self.proxy.clone(),
            quarantines: self.quarantines.clone(),
            parent: self.parent.clone(),
            cache_parent,
        };
        let _ = std::thread::Builder::new()
            .name("object-cache-warm".to_owned())
            .spawn(move || {
                for (confirmed, _) in gate.context.candidates() {
                    let Ok(repo) = Repo::try_from(confirmed.path.as_str()) else {
                        continue;
                    };
                    let Ok(served) = gate.context.served(&repo) else {
                        continue;
                    };
                    let url = format!("https://{}/{}.git", served.repo.host, served.repo.path);
                    let Ok(from) = gate.context.fetch_source(&served, &url) else {
                        continue;
                    };
                    purlis_core::heldact::FromForge {
                        cache: gate
                            .cache_of(&served)
                            .map(|cache| cache.for_a_push(purlis_core::heldact::PUSH)),
                        ..from
                    }
                    .warm();
                }
            });
    }

    fn check(&self, chat: &ChatId, push: &PushRead) -> PushVerdict {
        let Some(session) = self.context.session(chat) else {
            return Self::refuse("purlis did not send this push: this chat is not open.");
        };
        let Ok(repo) = Repo::try_from(push.repo.as_str()) else {
            return Self::refuse("purlis did not send this push: it names no repo it can read.");
        };
        let served = match self.context.served(&repo) {
            Ok(served) => served,
            Err(why) => return Self::refuse(format!("purlis did not send this push: {why}.")),
        };
        // The repo is the one at this host and port, never only at this path: a path confirmed on
        // GitHub is not the repo of the same path on GitLab, whose CI files are other files.
        if !is_at(&served.repo, &push.host, push.port) {
            return Self::refuse(
                "purlis did not send this push: its repo is not the one confirmed at that address.",
            );
        }
        let Some(_fetching) = self.quarantines.begin_fetching() else {
            return Self::refuse(
                "purlis did not send this push: it is reading other pushes now, so try again in a moment.",
            );
        };
        // One deadline for every fetch of this push, whatever the number of branches it updates.
        let started = std::time::Instant::now();
        for update in &push.updates {
            let budget = purlis_core::heldact::PUSH.saturating_sub(started.elapsed());
            // A delete moves no file; a tag or another ref is not a branch the policy lets by.
            let created = update.old.bytes().all(|b| b == b'0');
            if update.new.bytes().all(|b| b == b'0') {
                continue;
            }
            let Some(name) = update.name.strip_prefix("refs/heads/") else {
                return Self::refuse("purlis sends pushes of branches only.");
            };
            let (Ok(branch), Ok(tip)) =
                (Base::try_from(name), HeadSha::try_from(update.new.as_str()))
            else {
                return Self::refuse("purlis did not send this push: a name it cannot read.");
            };
            let (forge, _) = match self.context.forge_of(&served, None) {
                Ok(made) => made,
                Err(why) => return Self::refuse(format!("purlis did not send this push: {why}.")),
            };
            let url = format!("https://{}/{}.git", served.repo.host, served.repo.path);
            let from = match self.context.fetch_source(&served, &url) {
                // Each push fetches what changed since the last into the repo's own cache, all
                // its fetches within one deadline (inside the listener's connection total).
                Ok(from) => purlis_core::heldact::FromForge {
                    cache: self.cache_of(&served).map(|cache| cache.for_a_push(budget)),
                    ..from
                },
                Err(why) => return Self::refuse(format!("purlis did not send this push: {why}.")),
            };
            let one = Update {
                branch,
                tip,
                old: &update.old,
                created,
                pack: &push.pack,
            };
            if let Err(verdict) =
                self.read_update(chat, session, forge.as_ref(), &from, &repo, &one)
            {
                return verdict;
            }
        }
        PushVerdict::Forward
    }
}

/// One branch's update of a push, as the gate reads it.
struct Update<'a> {
    branch: Base,
    tip: HeadSha,
    /// The tip the push says the branch starts from (all zeros where it makes the branch).
    old: &'a str,
    created: bool,
    pack: &'a [u8],
}

impl CiPushGate {
    /// **One branch's update, read against what the forge has now** (ADR 0128 D8.3). The branch's
    /// base is its tip on the forge, and the push must start from it. The CI files it changes are
    /// those that differ from `against`, **the default branch's tip now**, for a branch the push
    /// makes as for one that is there; for one that is there, less what the branch had on the forge
    /// already (`already`, its old tip), so a file the push leaves as it was is no change of its
    /// own. `Err` is the verdict to answer with: a refusal, or the held act's ask.
    fn read_update(
        &self,
        chat: &ChatId,
        session: u32,
        forge: &dyn purlis_core::heldact::HeldForge,
        from: &dyn purlis_core::heldact::BaseObjects,
        repo: &Repo,
        update: &Update<'_>,
    ) -> Result<(), PushVerdict> {
        let Update {
            branch,
            tip,
            old,
            created,
            pack,
        } = update;
        let created = *created;
        let refuse = |why: String| {
            Err(Self::refuse(format!(
                "purlis did not send this push: {why}"
            )))
        };
        // The base is the branch's tip on the forge, and the push must start from it: a push
        // that names another old tip is not the push whose diff is read here. A new branch's
        // base is the default branch's.
        let base = match (created, forge.branch_tip(repo, branch)) {
            (false, Ok(Some(base))) if base.as_str().eq_ignore_ascii_case(old) => base,
            (false, Ok(_)) => {
                return refuse("its branch is not at the commit the push starts from.".to_owned());
            }
            (true, Ok(Some(_))) => {
                return refuse("it makes a branch that is there already.".to_owned());
            }
            (true, Ok(None)) => match forge.default_branch(repo) {
                Ok((_, base)) => base,
                Err(why) => {
                    return refuse(format!("it could not read the default branch ({why})."));
                }
            },
            (_, Err(why)) => {
                return refuse(format!("it could not read the branch ({why})."));
            }
        };
        // An unreadable `ci_config_path` is a refusal, never a push let through.
        let config = match forge.ci_config_path(repo) {
            Ok(config) => config,
            Err(why) => {
                return refuse(format!("it could not read where the CI files are ({why})."));
            }
        };
        let quarantine = match Quarantine::open(&self.parent, &base, from) {
            Ok(quarantine) => quarantine,
            Err(why) => return refuse(format!("it could not read the base ({why}).")),
        };
        // The default branch's tip now: what a new branch's base is, and what a branch that is
        // there is read against, which a push over it (a rebase) holds only by reference: its
        // commits are read in too.
        let default_tip = if created {
            base.clone()
        } else {
            match forge.default_branch(repo) {
                Ok((_, default_tip)) => default_tip,
                Err(why) => {
                    return refuse(format!("it could not read the default branch ({why})."));
                }
            }
        };
        if !created && let Err(why) = quarantine.also_have(from, &default_tip) {
            return refuse(format!(
                "it could not read the default branch's commits ({why})."
            ));
        }
        // A thin pack may be built on any branch the forge advertises (an integration branch a
        // task branch is cut from), and a push may send no new objects at all (an empty pack) when
        // its tip is already in the repo: either way the advertised tips are fetched, and only if
        // the pack needs them. A tip that is in none of them is refused, whatever the forge
        // resolves by id (a fork's commit, a dangling one).
        if let Err(why) = quarantine.take_in_known(pack, tip, &base, from) {
            return refuse(format!("git did not take its objects in ({why})."));
        }
        let trees = quarantine.trees();
        let already = (!created).then_some(&base);
        match check_push_since(
            forge.kind(),
            config.as_deref(),
            &trees,
            &default_tip,
            already,
            tip,
        ) {
            PushCheck::NoCiFiles => Ok(()),
            PushCheck::Refuse(why) => refuse(why),
            PushCheck::Hold(_) => {
                // The ask is made from the quarantine this push was read in, and the press
                // reads the same one again.
                self.quarantines.keep(session, repo.as_str(), quarantine);
                let asked = Requested::CiPush {
                    repo: repo.clone(),
                    branch: branch.clone(),
                    tip: tip.clone(),
                };
                let said = self.proxy.hand_over(chat, &asked);
                // An ask nobody waits on (withdrawn, refused, or no sink to raise it) keeps no
                // quarantine until the chat closes.
                if !self.context.waits_in(session, repo) {
                    self.quarantines.release(session, repo.as_str());
                }
                Err(match said {
                    Some(said) => PushVerdict::Held(said),
                    None => Self::refuse(
                        "purlis did not send this push: it changes CI files, and held acts are not on here.",
                    ),
                })
            }
        }
    }
}

impl ProjectContext {
    /// Whether a held act of chat `session` in `repo` waits on the person in this project.
    fn waits_in(&self, session: u32, repo: &Repo) -> bool {
        self.held
            .upgrade()
            .is_some_and(|held| held.held_acts().waits_in(session, repo.as_str()))
    }

    /// Where a quarantine fetches its base from: the repo's own address, with the person's
    /// credential for that host and no other, given to git by the app's own credential helper,
    /// which reads it from the keyring and answers git's pipe. The token is in no file.
    fn fetch_source(
        &self,
        served: &Served,
        url: &str,
    ) -> Result<purlis_core::heldact::FromForge, String> {
        let scope = HostScope::claim()
            .ok_or_else(|| "only the app itself acts as the person".to_owned())?;
        // A sign-in about to expire is renewed here, as the helper never renews: it reads what the
        // keyring item then holds.
        SignedIn::new(&scope, &self.config_root)
            .credential_of(
                &served.account.host,
                &served.account.id,
                crate::sandboxing::now_secs(),
            )
            .map_err(|why| why.to_string())?;
        let exe = std::env::current_exe().map_err(|why| why.to_string())?;
        let helper = purlis_core::forge::gitcredential::helper_for_account(
            &scope,
            &exe,
            &self.config_root,
            &served.account,
        )
        .ok_or_else(|| "purlis has no credential helper for this account".to_owned())?;
        Ok(purlis_core::heldact::FromForge {
            url: url.to_owned(),
            helper: Some(helper),
            cache: None,
        })
    }
}

/// **The project's held acts, for each route** ([`HeldSinks`]): the registry of the project the
/// app holds, with a context that reads the forge as the person.
pub struct AppHeldSinks {
    app: tauri::AppHandle,
    quarantines: Arc<Quarantines>,
    parent: PathBuf,
    cache_parent: Option<PathBuf>,
}

impl AppHeldSinks {
    pub fn new(app: tauri::AppHandle, parent: PathBuf, cache_parent: Option<PathBuf>) -> Self {
        // A quarantine an earlier app left (it ended without dropping them) is gone before a
        // new one is made: the one-per-user lock means none belongs to a live app.
        purlis_core::heldact::sweep_quarantines(&parent);
        // The folder exists from the start, so a native Linux read denial (which applies to a path
        // that is there) has one to apply to before the first push makes a cache in it.
        if let Some(folder) = &cache_parent {
            let _ = std::fs::create_dir_all(folder);
        }
        Self {
            app,
            quarantines: Arc::new(Quarantines::default()),
            parent,
            cache_parent,
        }
    }

    fn context_of(
        &self,
        project: &Path,
        sessions: Arc<dyn SessionOfRun>,
    ) -> Option<(Arc<Held>, Arc<ProjectContext>)> {
        use tauri::Manager as _;
        let held = self.app.state::<crate::planes::Planes>().held_at(project)?;
        let config_root = purlis_core::machine::config_root()?;
        let context = Arc::new(ProjectContext {
            project: project.to_path_buf(),
            config_root,
            sessions,
            quarantines: self.quarantines.clone(),
            held: Arc::downgrade(&held),
        });
        held.held_acts().set_context(context.clone());
        Some((held, context))
    }
}

impl HeldSinks for AppHeldSinks {
    fn sink(&self, project: &Path, sessions: Arc<dyn SessionOfRun>) -> Option<Box<dyn HeldSink>> {
        let (held, _) = self.context_of(project, sessions)?;
        sink_of(&held)
    }

    fn forget(&self, session: u32) {
        self.quarantines.forget(session);
    }

    fn push_gate(
        &self,
        project: &Path,
        sessions: Arc<dyn SessionOfRun>,
        proxy: Arc<ForgeProxy>,
    ) -> Option<Arc<dyn PushGate>> {
        let (_, context) = self.context_of(project, sessions)?;
        Some(Arc::new(CiPushGate {
            context,
            proxy,
            quarantines: self.quarantines.clone(),
            parent: self.parent.clone(),
            cache_parent: self.cache_parent.clone(),
        }))
    }
}

/// **The forge route's sink for project `held`'s held acts**, over its registry and the context
/// last set on it: `None` until one is.
/// Each ask it raises is told to the project's window, which lists it in Asks (#7685).
pub(crate) fn sink_of(held: &Arc<Held>) -> Option<Box<dyn HeldSink>> {
    let window = Arc::new(crate::heldacts::InTheWindow(Arc::downgrade(held)));
    let handoff: Handoff = held.held_acts().handoff(window)?;
    Some(Box::new(handoff))
}

/// **The Notice of a chat's missing or ended sign-in** ([`Notices`], ADR 0128, #7290): told to
/// the chat's project, whose pane says it with a way into Settings › You › Accounts. Nothing is
/// kept here: the project's [`crate::stuck::Noticed`] holds it until the chat closes.
pub struct AppNotices {
    app: tauri::AppHandle,
}

impl AppNotices {
    pub fn new(app: tauri::AppHandle) -> Self {
        Self { app }
    }
}

impl AppNotices {
    /// The open project at `project`, where one is open.
    fn open_at(&self, project: &Path) -> Option<Arc<crate::planes::Held>> {
        use tauri::Manager as _;
        self.app.state::<crate::planes::Planes>().held_at(project)
    }
}

impl Notices for AppNotices {
    fn sign_in_needed(&self, project: &Path, session: u32, host: &str, state: SignInState) -> bool {
        // A project that is not open has no pane to say it on.
        let Some(held) = self.open_at(project) else {
            return false;
        };
        // A chat that has closed has no pane, and its entry is not made again.
        if !held
            .chats()
            .open_now()
            .iter()
            .any(|open| open.session == session)
        {
            return false;
        }
        held.noticed().sign_in_needed(
            session,
            crate::stuck::SignInNeeded {
                host: host.to_owned(),
                ended: state == SignInState::Ended,
            },
        );
        true
    }

    fn sign_in_ok(&self, project: &Path, session: u32, host: &str) {
        if let Some(held) = self.open_at(project) {
            held.noticed().sign_in_ok(session, host);
        }
    }
}

#[cfg(test)]
mod tests;
