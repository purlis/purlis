// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The credential a chat's route is given, and only for what the person confirmed** (ADR 0128
//! D2, D4, D1.1, D1.3 rule 2).
//!
//! The forge proxy ([`crate::forge::proxy`]) decides what a chat may ask; this is the
//! [`proxy::CredentialSource`] it asks for the token to put in. It is the one place that joins the
//! person's sign-ins ([`SignedIn`]) to the list of repos the person confirmed
//! ([`crate::forge::confirmed`]), so that a repo is served **only** when all of these hold:
//!
//! 1. the clone is one of **this chat's** (the app names them when the chat starts, [`ChatRoute::enrol`]);
//! 2. the person trusts the clone **and** the list holds the repo by numeric id
//!    ([`confirmed::serves`]), and the application it was recorded under is still the host's pin;
//! 3. the person **chose** one account for it ([`confirmed::account_for`]): never another that
//!    happens to be signed in, never a signed-out one's stand-in, never one whose token holds
//!    `workflow` ([`crate::forge::accounts::everyday_holds_workflow`]), never one signed in through
//!    another application than the host pins now;
//! 4. that account's token is read ([`SignedIn::credential_of`]), renewed first when it is due.
//!
//! Anything else is [`CredentialError::Unavailable`], and the proxy answers the chat with a
//! refusal. A request that names no repo (the few credentialed reads outside one) is served only
//! when every repo of the chat on that host is served by the same account.
//!
//! **Tier** (ADR 0069): the chats' clones are memory, held by the one running app; nothing here is
//! written.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use super::SignedIn;
use crate::forge::Kind;
use crate::forge::accounts;
use crate::forge::confirmed::{self, Admission, ConfirmedRepo, Serving};
use crate::forge::proxy::{
    self, ChatId, Credential, CredentialError, CredentialRequest, CredentialSource, TokenKind,
};
use crate::forge::route::HostScope;
use crate::secrets::keyring::Secret;

/// The credential source of the forge proxy: the signed-in accounts, for what the person confirmed.
pub struct ChatRoute {
    config_root: PathBuf,
    signed_in: SignedIn,
    clones: Mutex<HashMap<ChatId, Vec<PathBuf>>>,
}

impl ChatRoute {
    /// The sign-ins under `config_root`, in the host's process: only the host can claim the scope
    /// this takes.
    pub fn new(host: &HostScope, config_root: &Path) -> ChatRoute {
        ChatRoute::through(SignedIn::new(host, config_root), config_root)
    }

    /// [`ChatRoute::new`] over `signed_in`: a test's stand-in forge renews through it.
    pub fn through(signed_in: SignedIn, config_root: &Path) -> ChatRoute {
        ChatRoute {
            config_root: config_root.to_path_buf(),
            signed_in,
            clones: Mutex::new(HashMap::new()),
        }
    }

    /// A chat starts: these are its clones, the ones the person put in its workspace. Replaces any
    /// it had.
    pub fn enrol(&self, chat: &ChatId, clones: Vec<PathBuf>) {
        self.clones
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(chat.clone(), clones);
    }

    /// A chat ended: nothing more is served for it.
    pub fn leave(&self, chat: &ChatId) {
        self.clones
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(chat);
    }

    /// The repos of `chat` the request is for, each with its clone, **served** by the list.
    fn served(&self, request: &CredentialRequest) -> Result<Vec<ConfirmedRepo>, String> {
        let kind = match request.kind {
            proxy::Kind::GitHub => Kind::GitHub,
            proxy::Kind::GitLab => Kind::GitLab,
        };
        let clones = self
            .clones
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&request.chat)
            .cloned()
            .ok_or_else(|| "That chat has no clones.".to_string())?;
        let wanted = (!request.repo.is_empty())
            .then(|| confirmed::normalised_path(kind, &request.repo))
            .map(|path| path.ok_or_else(|| "That is not a repo path.".to_string()))
            .transpose()?;
        // A named repo is served by id: the forge-read id the proxy recorded for it, never the
        // entry's own. Absent, it is refused; only a request outside a repo has none.
        if wanted.is_some() && request.repo_id.is_none() {
            return Err("The forge's id for that repo is not known.".to_string());
        }
        let mut found = Vec::new();
        for clone in &clones {
            for repo in confirmed::repos_of_clone(&self.config_root, clone)? {
                let (bare, port) = confirmed::address_of(&repo.host);
                if repo.kind != kind
                    || !bare.eq_ignore_ascii_case(&request.host)
                    || port != request.port
                    || wanted.as_ref().is_some_and(|path| *path != repo.path)
                {
                    continue;
                }
                // Trust and the numeric id and the pin, as the list holds them now. A request
                // outside a repo has no id of its own to compare: the entry's is the one it names.
                let id = request.repo_id.unwrap_or(repo.repo_id);
                let admitted = confirmed::serves(
                    &self.config_root,
                    clone,
                    repo.kind,
                    &repo.host,
                    &repo.path,
                    id,
                )?;
                match admitted {
                    Admission::Confirmed(repo) => found.push(repo),
                    // A request outside a repo is served only when every repo of the chat on that
                    // host is served: one that fails is a refusal, never one left out.
                    _ if wanted.is_none() => {
                        return Err(format!(
                            "{} on {} is not confirmed for this chat, so a request outside a repo \
                             is not served.",
                            repo.path, repo.host
                        ));
                    }
                    _ => {}
                }
            }
        }
        Ok(found)
    }
}

impl CredentialSource for ChatRoute {
    fn credential_for(&self, request: &CredentialRequest) -> Result<Credential, CredentialError> {
        let unavailable = |why: &str| {
            tracing::info!("purlis: a chat's forge request was given no credential: {why}");
            CredentialError::Unavailable
        };
        let repos = self.served(request).map_err(|why| unavailable(&why))?;
        let Some(first) = repos.first() else {
            return Err(unavailable("no confirmed repo of this chat matches"));
        };
        // One account, chosen: never a pick between two.
        if first.account.is_empty() || repos.iter().any(|r| r.account != first.account) {
            return Err(unavailable("no single account is chosen for this"));
        }
        let listed = accounts::listed(&self.config_root).map_err(|why| unavailable(&why))?;
        // Every repo counts, not the first alone: each must be served now by the one account.
        let mut serving = None;
        for repo in &repos {
            match confirmed::account_for(&self.config_root, repo, &listed) {
                Serving::Account(account)
                    if serving
                        .as_ref()
                        .is_none_or(|a: &accounts::ForgeAccount| a.id == account.id) =>
                {
                    serving = Some(account);
                }
                _ => return Err(unavailable("the chosen account cannot serve it now")),
            }
        }
        let Some(account) = serving else {
            return Err(unavailable("the chosen account cannot serve it now"));
        };
        let credential = self
            .signed_in
            .credential_of(&account.host, &account.id, accounts::now())
            .map_err(|why| unavailable(&format!("{why:?}")))?;
        Ok(Credential {
            token_kind: match credential.token_kind() {
                super::TokenKind::SignIn => TokenKind::Oauth,
                _ => TokenKind::Pat,
            },
            secret: Secret::new(credential.expose().to_string()),
        })
    }
}
