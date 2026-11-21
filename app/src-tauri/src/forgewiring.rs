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
//!   ([`Quarantines`]). A push goes on only when [`check_push`] says it changes no CI file; a
//!   refusal, or an unreadable `ci_config_path`, sends nothing.
//!
//! **Tier** (ADR 0069): the quarantines are Machine, device-bound and rebuildable
//! (`docs/plane-format.md`, "A held CI-file push's quarantine"); the asks are memory only.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use purlis_core::forge::accounts::{self, ForgeAccount};
use purlis_core::forge::confirmed::{self, ConfirmedRepo};
use purlis_core::forge::credential::{
    CredentialError as SourceError, CredentialSource as HostSource, SignedIn,
};
use purlis_core::forge::held::{CredentialTokens, WorkflowTransports, held_forge};
use purlis_core::forge::http::TokenSource;
use purlis_core::forge::proxy::listener::{PushGate, PushRead, PushVerdict};
use purlis_core::forge::proxy::{ChatId, ForgeProxy, HeldSink};
use purlis_core::forge::route::HostScope;
use purlis_core::forge::transport::Transport;
use purlis_core::forge::{Forge, ForgeError, Kind};
use purlis_core::forgewire::{ExposeSecret, SecretString};
use purlis_core::heldact::{
    Handoff, HeldContext, NoClone, OwnedWorld, PushCheck, Quarantine, Recorded, Requested,
    TipFiles, Trees, Unavailable, WorkflowCredential, check_push,
};
use purlis_core::mergehold::{Base, HeadSha, Repo, Request};
use purlis_core::secrets::keyring::Secret;

use crate::forgeroute::{HeldSinks, SessionOfRun};
use crate::planes::Held;

/// The most quarantines kept for pushes that wait, in all: each is a repository on disk.
const MOST_KEPT: usize = 16;

/// A chat's session and the repo a push is of.
type Key = (u32, String);

/// **The quarantines of the pushes that wait**, by the chat's session and the repo: the push an ask
/// was made from is the push a press reads again.
#[derive(Default)]
pub struct Quarantines {
    kept: Mutex<Vec<(Key, Arc<Quarantine>)>>,
}

impl Quarantines {
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
}

impl WorkflowTransports for WorkflowOver {
    fn with(&self, forge: &Forge, secret: &Secret) -> Result<Arc<dyn Transport>, String> {
        let token = Arc::new(Fixed(SecretString::from(secret.expose().to_owned())));
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

/// **What a project's held acts read** ([`HeldContext`]).
pub struct ProjectContext {
    project: PathBuf,
    config_root: PathBuf,
    sessions: Arc<dyn SessionOfRun>,
    quarantines: Arc<Quarantines>,
    held: std::sync::Weak<Held>,
}

impl ProjectContext {
    fn confirmed(&self) -> Vec<ConfirmedRepo> {
        crate::forgeroute::prod::clones_of(&self.project)
            .iter()
            .filter_map(|clone| confirmed::repos_of_clone(&self.config_root, clone).ok())
            .flatten()
            .collect()
    }

    fn confirmed_repo(&self, repo: &Repo) -> Option<ConfirmedRepo> {
        self.confirmed()
            .into_iter()
            .find(|have| have.path.eq_ignore_ascii_case(repo.as_str()))
    }

    fn account_of(&self, repo: &ConfirmedRepo) -> Result<ForgeAccount, String> {
        if repo.account.is_empty() {
            return Err("no account is chosen for this repo".to_owned());
        }
        accounts::listed(&self.config_root)?
            .into_iter()
            .find(|account| account.id == repo.account)
            .ok_or_else(|| "the account chosen for this repo is gone".to_owned())
    }

    /// The forge as the person, for `repo`, reading a push's files from `quarantine` where there
    /// is one.
    fn forge_of(
        &self,
        confirmed: &ConfirmedRepo,
        quarantine: Option<&Quarantine>,
    ) -> Result<(Box<dyn purlis_core::heldact::HeldForge>, ForgeAccount), String> {
        let account = self.account_of(confirmed)?;
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
        let confirmed = self
            .confirmed_repo(repo)
            .ok_or_else(|| format!("{} is not a repo confirmed for this project", repo.as_str()))?;
        let quarantine = self.quarantines.of(session, repo.as_str());
        let (forge, account) = self.forge_of(&confirmed, quarantine.as_deref())?;
        let trees: Arc<dyn Trees> = match quarantine.as_deref() {
            Some(quarantine) => Arc::new(quarantine.trees()),
            None => Arc::new(NoClone),
        };
        let recorded: Vec<Repo> = self
            .confirmed()
            .iter()
            .filter_map(|have| Repo::try_from(have.path.as_str()).ok())
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
            .and_then(|repo| self.confirmed_repo(repo))
            .and_then(|confirmed| {
                crate::forgeroute::prod::clones_of(&self.project)
                    .into_iter()
                    .find(|clone| {
                        confirmed::repos_of_clone(&self.config_root, clone)
                            .is_ok_and(|repos| repos.iter().any(|r| r.path == confirmed.path))
                    })
            })
        else {
            return "purlis did not hold this merge: it names no repo of this project.".to_owned();
        };
        let ask = purlis_core::hookwire::MergeAsk {
            chat: session,
            request: request.clone(),
            cwd: cwd.display().to_string(),
        };
        match crate::mergeholds::asked(&self.held, &ask) {
            purlis_core::hookwire::Answer::MergeHeld { .. } => {
                "purlis holds merges. It has asked the person; the merge happens when they approve \
                 it in Asks."
                    .to_owned()
            }
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
}

impl CiPushGate {
    fn refuse(why: impl Into<String>) -> PushVerdict {
        PushVerdict::Refuse(why.into())
    }
}

impl PushGate for CiPushGate {
    fn check(&self, chat: &ChatId, push: &PushRead) -> PushVerdict {
        let Some(session) = self.context.session(chat) else {
            return Self::refuse("purlis did not send this push: this chat is not open.");
        };
        let Ok(repo) = Repo::try_from(push.repo.as_str()) else {
            return Self::refuse("purlis did not send this push: it names no repo it can read.");
        };
        let Some(confirmed) = self.context.confirmed_repo(&repo) else {
            return Self::refuse("purlis did not send this push: its repo is not confirmed.");
        };
        for update in &push.updates {
            // A delete moves no file; a tag or another ref is not a branch the policy lets by.
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
            let (forge, _) = match self.context.forge_of(&confirmed, None) {
                Ok(made) => made,
                Err(why) => return Self::refuse(format!("purlis did not send this push: {why}.")),
            };
            // The base is the branch's tip on the forge, or the default branch's for a new one.
            let base = match forge.branch_tip(&repo, &branch) {
                Ok(Some(base)) => base,
                Ok(None) => match forge.default_branch(&repo) {
                    Ok((_, base)) => base,
                    Err(why) => {
                        return Self::refuse(format!(
                            "purlis did not send this push: it could not read the default branch ({why})."
                        ));
                    }
                },
                Err(why) => {
                    return Self::refuse(format!(
                        "purlis did not send this push: it could not read the branch ({why})."
                    ));
                }
            };
            // An unreadable `ci_config_path` is a refusal, never a push let through.
            let config = match forge.ci_config_path(&repo) {
                Ok(config) => config,
                Err(why) => {
                    return Self::refuse(format!(
                        "purlis did not send this push: it could not read where the CI files are ({why})."
                    ));
                }
            };
            let url = format!("https://{}/{}.git", confirmed.host, confirmed.path);
            let from = match self.context.fetch_source(&confirmed, &url) {
                Ok(from) => from,
                Err(why) => return Self::refuse(format!("purlis did not send this push: {why}.")),
            };
            let quarantine = match Quarantine::open(&self.parent, &base, &from) {
                Ok(quarantine) => quarantine,
                Err(why) => {
                    return Self::refuse(format!(
                        "purlis did not send this push: it could not read the base ({why})."
                    ));
                }
            };
            if let Err(why) = quarantine.take_in(push.pack.clone(), &tip, &base) {
                return Self::refuse(format!(
                    "purlis did not send this push: git did not take its objects in ({why})."
                ));
            }
            let kind = forge.kind();
            match check_push(kind, config.as_deref(), &quarantine.trees(), &base, &tip) {
                PushCheck::NoCiFiles => continue,
                PushCheck::Refuse(why) => {
                    return Self::refuse(format!("purlis did not send this push: {why}"));
                }
                PushCheck::Hold(_) => {
                    // The ask is made from the quarantine this push was read in, and the press
                    // reads the same one again.
                    self.quarantines.keep(session, repo.as_str(), quarantine);
                    let asked = Requested::CiPush {
                        repo: repo.clone(),
                        branch,
                        tip,
                    };
                    return match self.proxy.hand_over(chat, &asked) {
                        Some(said) => PushVerdict::Held(said),
                        None => Self::refuse(
                            "purlis did not send this push: it changes CI files, and held acts are not on here.",
                        ),
                    };
                }
            }
        }
        PushVerdict::Forward
    }
}

impl ProjectContext {
    /// Where a quarantine fetches its base from: the repo's own address, with the person's
    /// credential for one fetch and no other.
    fn fetch_source(
        &self,
        confirmed: &ConfirmedRepo,
        url: &str,
    ) -> Result<purlis_core::heldact::FromForgeAs, String> {
        let account = self.account_of(confirmed)?;
        let scope = HostScope::claim()
            .ok_or_else(|| "only the app itself acts as the person".to_owned())?;
        let signed_in = SignedIn::new(&scope, &self.config_root);
        let credential = signed_in
            .credential_of(&account.host, &account.id, crate::sandboxing::now_secs())
            .map_err(|why| why.to_string())?;
        Ok(purlis_core::heldact::FromForgeAs {
            url: url.to_owned(),
            username: match account.kind {
                Kind::GitHub => "x-access-token",
                Kind::GitLab => "oauth2",
            }
            .to_owned(),
            token: SecretString::from(credential.expose().to_owned()),
        })
    }
}

/// **The project's held acts, for each route** ([`HeldSinks`]): the registry of the project the
/// app holds, with a context that reads the forge as the person.
pub struct AppHeldSinks {
    app: tauri::AppHandle,
    quarantines: Arc<Quarantines>,
    parent: PathBuf,
}

impl AppHeldSinks {
    pub fn new(app: tauri::AppHandle, parent: PathBuf) -> Self {
        Self {
            app,
            quarantines: Arc::new(Quarantines::default()),
            parent,
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
        let handoff: Handoff = held.held_acts().handoff()?;
        Some(Box::new(handoff))
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
        }))
    }
}
