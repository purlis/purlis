// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **One login, the window's half** (ADR 0128 D1, D2, D4, D5): the person-facing side of "sign in
//! once per forge host".
//!
//! - **An application an owner made** on a GitHub Enterprise Server host or a self-hosted GitLab:
//!   [`forge_use_application`] is the person's press of *Use this application*, pinning its
//!   client id with the owner they expect the forge's consent page to show;
//!   [`forge_applications`] lists the pins with the sentence to read before signing in;
//!   [`forge_application_request`] is the text to send an owner who has not made one.
//! - **Which account serves a project** ([`forge_project_accounts`], [`forge_choose_project_account`]):
//!   a person with a personal and an enterprise account on one host chooses, per project, on a
//!   press. One account on a host is the default and asks nothing.
//! - **Recording the repo** at a clone from the window ([`confirm_after_clone`]) and at *Trust this
//!   clone* ([`confirm_after_trust`]): host, owner/name and the numeric id, read from the forge as
//!   the person ([`forge_trust_clone_repo`] is the press that retries or chooses).
//! - **The second token** ([`forge_ci_permission_start`], [`forge_ci_permission_wait`]): the
//!   `workflow`-scoped sign-in, asked on the press a held CI-file push needs, kept apart from the
//!   everyday token and never on a chat's route.
//!
//! **Every command here is the window's alone** (`WINDOW_ONLY`, D5): none is served on a link, none
//! is in the `purlis` binary, and each claims the host's scope first
//! ([`purlis_core::forge::provision::on_the_persons_press`]), which a process a chat started is
//! refused.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use purlis_core::forge::accounts::{self, ForgeAccount, State, workflow};
use purlis_core::forge::confirmed::{self, AsThePerson, Pick, Serving};
use purlis_core::forge::registered::{self, Change};
use purlis_core::forge::registration::{self, Registration};
use purlis_core::forge::{Kind, device, oauth};

use crate::forgeaccounts::{
    ForgeDeviceCode, UntilCancelled, blocking, config_root, host_scope, kind_of, next_flow_number,
    token_check,
};
use crate::planes::{PlaneId, Planes};

/// A host's pinned application, as Settings lists it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ForgeApplicationRow {
    pub kind: String,
    pub host: String,
    pub client_id: String,
    /// The owner the person expects the forge's page to show.
    pub owner: String,
    /// What to read before signing in: *check that the application is owned by …*.
    pub consent: String,
    /// Accounts signed in on the host now.
    pub signed_in: u32,
}

/// What a press of *Use this application* changed.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ForgeApplicationPinned {
    pub host: String,
    /// `new`, `same`, `client-id` or `owner`.
    pub change: String,
    /// The client id (or owner) this replaced, for the Notice a changed pin raises.
    pub replaced: Option<String>,
    pub consent: String,
}

/// This machine's pinned applications, in the order they were pinned.
#[tauri::command]
#[specta::specta]
pub async fn forge_applications() -> Result<Vec<ForgeApplicationRow>, String> {
    blocking(|| {
        host_scope()?;
        let root = config_root()?;
        let mut rows = Vec::new();
        for (kind, host) in registered::hosts(&root)? {
            let Some(pin) = registered::pin(&root, kind, &host)? else {
                continue;
            };
            let signed_in = accounts::every_signed_in_on(&root, kind, &host)?.len();
            rows.push(ForgeApplicationRow {
                kind: kind.word().to_string(),
                consent: registered::consent_check(&pin.owner, &host),
                host,
                client_id: pin.client_id,
                owner: pin.owner,
                signed_in: u32::try_from(signed_in).unwrap_or(u32::MAX),
            });
        }
        Ok(rows)
    })
    .await
}

/// **The person's press of *Use this application*** (D1.3 rule 2): pins `client_id` for `kind` on
/// `host`, with `owner`, the organization or group the person expects the application to be owned
/// by. A changed client id or owner is a new press, and the answer says what it replaced.
#[tauri::command]
#[specta::specta]
pub async fn forge_use_application(
    kind: String,
    host: String,
    client_id: String,
    owner: String,
) -> Result<ForgeApplicationPinned, String> {
    let kind = kind_of(&kind)?;
    blocking(move || {
        let scope = host_scope()?;
        let root = config_root()?;
        let pinned = registered::use_application(&scope, &root, kind, &host, &client_id, &owner)?;
        let (change, replaced) = match pinned.change {
            Change::New => ("new", None),
            Change::Same => ("same", None),
            Change::ClientId { previous } => ("client-id", Some(previous)),
            Change::Owner { previous } => ("owner", Some(previous)),
        };
        Ok(ForgeApplicationPinned {
            consent: registered::consent_check(owner.trim(), &pinned.host),
            host: pinned.host,
            change: change.to_string(),
            replaced,
        })
    })
    .await
}

/// Takes `host`'s pinned application away. Accounts signed in through it stay listed and keep
/// their tokens until they end; they can no longer be renewed.
#[tauri::command]
#[specta::specta]
pub async fn forge_remove_application(kind: String, host: String) -> Result<(), String> {
    let kind = kind_of(&kind)?;
    blocking(move || {
        let scope = host_scope()?;
        registered::unregister(&scope, &config_root()?, kind, &host)
    })
    .await
}

/// **The request to send an owner** who has not made the application on `host`: what to enter,
/// as text to copy (D1.3 rule 3).
#[tauri::command]
#[specta::specta]
pub async fn forge_application_request(kind: String, host: String) -> Result<String, String> {
    let kind = kind_of(&kind)?;
    blocking(move || {
        host_scope()?;
        let host = accounts::host_named(kind, &host)?;
        Ok(registered::owner_request(kind, &host))
    })
    .await
}

// ---------------------------------------------------------------------------------------
// Which account serves a project.

/// One account the person may choose for a repo.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ProjectAccountChoice {
    pub id: String,
    pub login: String,
}

/// A confirmed repo, and the account that serves it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ProjectAccountRow {
    /// What the choice names the clone by.
    pub clone_key: String,
    /// The forge's own number for the repo, as text (a repo number can outgrow what the window's
    /// numbers hold).
    pub repo_id: String,
    pub host: String,
    pub path: String,
    /// The account serving it, or null when it is signed out or gone.
    pub serving: Option<ProjectAccountChoice>,
    /// Set when the chosen account is signed out: the person signs it in again, or chooses
    /// another. **No other account serves in its place.**
    pub chosen_is_signed_out: bool,
    /// The signed-in accounts on this repo's host, for the choice.
    pub choices: Vec<ProjectAccountChoice>,
}

/// Every confirmed repo, with the account that serves it and the accounts it could be served by.
#[tauri::command]
#[specta::specta]
pub async fn forge_project_accounts() -> Result<Vec<ProjectAccountRow>, String> {
    blocking(|| {
        host_scope()?;
        let root = config_root()?;
        let accounts_here = accounts::listed(&root)?;
        let rows = confirmed::all(&root)?
            .into_iter()
            .map(|repo| {
                let choices: Vec<ProjectAccountChoice> = accounts_here
                    .iter()
                    .filter(|a| {
                        a.state == State::SignedIn
                            && a.kind == repo.kind
                            && a.host.eq_ignore_ascii_case(&repo.host)
                    })
                    .map(choice_of)
                    .collect();
                let (serving, chosen_is_signed_out) =
                    match confirmed::account_for(&repo, &accounts_here) {
                        Serving::Account(account) => (Some(choice_of(&account)), false),
                        Serving::ChosenIsSignedOut { .. } => (None, true),
                    };
                ProjectAccountRow {
                    clone_key: repo.clone_key,
                    repo_id: repo.repo_id.to_string(),
                    host: repo.host,
                    path: repo.path,
                    serving,
                    chosen_is_signed_out,
                    choices,
                }
            })
            .collect();
        Ok(rows)
    })
    .await
}

fn choice_of(account: &ForgeAccount) -> ProjectAccountChoice {
    ProjectAccountChoice {
        id: account.id.clone(),
        login: account.login.clone(),
    }
}

/// **The press that chooses which account serves a repo.**
#[tauri::command]
#[specta::specta]
pub async fn forge_choose_project_account(
    clone_key: String,
    repo_id: String,
    account_id: String,
) -> Result<(), String> {
    blocking(move || {
        let scope = host_scope()?;
        let root = config_root()?;
        let repo_id: u64 = repo_id
            .parse()
            .map_err(|_| "That repo is not one purlis knows.".to_string())?;
        let account = accounts::listed(&root)?
            .into_iter()
            .find(|a| a.id == account_id)
            .ok_or_else(|| "That account is not on this machine any more.".to_string())?;
        confirmed::choose_account(&scope, &root, &clone_key, repo_id, &account).map(|_| ())
    })
    .await
}

// ---------------------------------------------------------------------------------------
// Recording the repo.

/// What a recording found, for the window to say.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ConfirmedAnswer {
    /// `recorded`, `choose` (several accounts on the host: the person picks) or `sign-in` (none).
    pub state: String,
    /// The repo as recorded, `owner/name`.
    pub path: Option<String>,
    pub host: Option<String>,
}

fn confirmed_answer(state: &str, repo: Option<&confirmed::ConfirmedRepo>) -> ConfirmedAnswer {
    ConfirmedAnswer {
        state: state.to_string(),
        path: repo.map(|r| r.path.clone()),
        host: repo.map(|r| r.host.clone()),
    }
}

/// The person's press that records the clone at `folder` (relative to the project) for D4: the
/// repo's numeric id is read from the forge as `account_id` (the only account on the host, when
/// none is named) and kept with the account. The clone must already be trusted.
#[tauri::command]
#[specta::specta]
pub async fn forge_trust_clone_repo(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    folder: String,
    account_id: Option<String>,
) -> Result<ConfirmedAnswer, String> {
    let project = planes.held(&plane)?.root().to_path_buf();
    blocking(move || {
        let scope = host_scope()?;
        confirm_clone(&scope, &project.join(&folder), account_id.as_deref())
    })
    .await
}

/// Confirms `clone`, which the trust list holds an address for, with the account named, or the
/// host's only one. Several accounts and none named is a question for the person; none is a
/// sign-in.
fn confirm_clone(
    scope: &purlis_core::forge::route::HostScope,
    clone: &std::path::Path,
    account_id: Option<&str>,
) -> Result<ConfirmedAnswer, String> {
    let root = config_root()?;
    let place = purlis_core::forge::confirmed::trusted_place(&root, clone)
        .ok_or_else(|| "That clone is not trusted yet. Trust it first.".to_string())?;
    let host = place.split('/').next().unwrap_or_default().to_string();
    let on_host: Vec<ForgeAccount> = accounts::listed(&root)?
        .into_iter()
        .filter(|a| a.state == State::SignedIn && a.host.eq_ignore_ascii_case(&host))
        .collect();
    let account = match account_id {
        Some(id) => on_host
            .into_iter()
            .find(|a| a.id == id)
            .ok_or_else(|| format!("That account is not signed in on {host}."))?,
        None => match confirmed::default_account(on_host) {
            Pick::Only(account) => account,
            Pick::Choose(_) => return Ok(confirmed_answer("choose", None)),
            Pick::SignIn => return Ok(confirmed_answer("sign-in", None)),
        },
    };
    let client_id = registration::resolve(&root, account.kind, &account.host)?
        .map(|r| r.client_id.to_string())
        .unwrap_or_default();
    let keyring = accounts::keyring_at(&root);
    let lookup = AsThePerson {
        keyring: keyring.as_ref(),
        reach: &token_check,
    };
    let repo = confirmed::confirm_trusted(scope, &root, clone, &lookup, &account, &client_id)?;
    Ok(confirmed_answer("recorded", Some(&repo)))
}

/// After a clone from the window: records the repo as the person's, best effort. A forge that
/// cannot be asked leaves the clone trusted and unconfirmed, and the project accounts list says
/// so; nothing about the clone itself fails for it.
pub(crate) fn confirm_after_clone(clone: &std::path::Path) {
    if purlis_core::fence::FENCED {
        return;
    }
    let Ok(scope) = host_scope() else { return };
    match confirm_clone(&scope, clone, None) {
        Ok(_) => {}
        Err(why) => tracing::info!("purlis: the clone was not confirmed with the forge: {why}"),
    }
}

/// After *Trust this clone*: the same.
pub(crate) fn confirm_after_trust(clone: &std::path::Path) {
    confirm_after_clone(clone);
}

// ---------------------------------------------------------------------------------------
// The second token.

/// A second sign-in in progress: held in memory for its length and nowhere else.
struct CiFlow {
    number: u32,
    registration: &'static Registration,
    started: Arc<device::Started>,
    cancelled: Arc<AtomicBool>,
    account: ForgeAccount,
}

static CI_FLOW: Mutex<Option<CiFlow>> = Mutex::new(None);

/// How a second sign-in ended.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct CiPermissionAnswer {
    /// `ready` (the second token is kept) or `not-offered` (GitHub folded the permission into
    /// the everyday sign-in, so a CI-file push is not offered).
    pub state: String,
    pub says: String,
}

/// **Ask GitHub for CI-file permission** on the press a held CI-file push needs (D1.1, D8.3): a
/// device flow asking for what the account holds and `workflow`. The code and the forge's page
/// are answered for the window to show. Waited on with [`forge_ci_permission_wait`].
#[tauri::command]
#[specta::specta]
pub async fn forge_ci_permission_start(account_id: String) -> Result<ForgeDeviceCode, String> {
    blocking(move || {
        host_scope()?;
        let root = config_root()?;
        let account = accounts::listed(&root)?
            .into_iter()
            .find(|a| a.id == account_id && a.state == State::SignedIn)
            .ok_or_else(|| "That account is not signed in on this machine.".to_string())?;
        if account.kind != Kind::GitHub {
            return Err(format!(
                "{} has no separate permission for CI files: its one sign-in covers them, and \
                 purlis asks you before it pushes one.",
                account.host
            ));
        }
        let registration = registration::resolve(&root, account.kind, &account.host)?
            .filter(|r| r.flow == registration::Flow::Device)
            .ok_or_else(|| {
                format!(
                    "purlis has no application for {} yet. Use your organization's application \
                     first.",
                    account.host
                )
            })?;
        if purlis_core::fence::FENCED {
            return Err(
                "A test build of purlis does not ask GitHub for permission through the browser."
                    .to_string(),
            );
        }
        let scopes = workflow::scopes_to_ask(&account);
        let started = device::start_asking(
            &oauth::Https::new(),
            &oauth::Endpoints::of(registration),
            registration,
            &scopes,
        )?;
        let number = next_flow_number();
        let code = ForgeDeviceCode {
            flow: number,
            host: registration.host.to_string(),
            title: account.kind.display().to_string(),
            user_code: started.user_code.clone(),
            verification_uri: started.verification_uri.clone(),
            expires_in: u32::try_from(started.expires_in.as_secs()).unwrap_or(u32::MAX),
            scopes,
        };
        let mut held = CI_FLOW.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(before) = held.replace(CiFlow {
            number,
            registration,
            started: Arc::new(started),
            cancelled: Arc::new(AtomicBool::new(false)),
            account,
        }) {
            before.cancelled.store(true, Ordering::SeqCst);
        }
        Ok(code)
    })
    .await
}

/// Wait for the person to allow CI files on GitHub's page for `flow`, then keep the second token
/// as its own keychain item. The everyday token is read back first: if GitHub folded the
/// permission into it, nothing is kept and a CI-file push is not offered.
#[tauri::command]
#[specta::specta]
pub async fn forge_ci_permission_wait(flow: u32) -> Result<CiPermissionAnswer, String> {
    blocking(move || {
        let scope = host_scope()?;
        let (registration, started, cancelled, account) = {
            let held = CI_FLOW.lock().unwrap_or_else(|e| e.into_inner());
            match held.as_ref().filter(|held| held.number == flow) {
                Some(held) => (
                    held.registration,
                    held.started.clone(),
                    held.cancelled.clone(),
                    held.account.clone(),
                ),
                None => return Err(device::Ended::Cancelled.said("the forge")),
            }
        };
        let root = config_root()?;
        let keyring = accounts::keyring_at(&root);
        let granted = device::wait(
            &oauth::Https::new(),
            &oauth::Endpoints::of(registration),
            registration,
            &started,
            &UntilCancelled(cancelled),
        );
        let mut held = CI_FLOW.lock().unwrap_or_else(|e| e.into_inner());
        if held.as_ref().is_some_and(|held| held.number == flow) {
            *held = None;
        }
        drop(held);
        let granted = granted.map_err(|why| match why {
            device::Ended::Denied => {
                "You didn't allow it on GitHub, so nothing changed. A push that changes a CI file \
                 stays held."
                    .to_string()
            }
            other => other.said(registration.kind.display()),
        })?;
        match workflow::keep(&scope, &root, keyring.as_ref(), &token_check, &account, granted)? {
            workflow::Outcome::Kept => Ok(CiPermissionAnswer {
                state: "ready".to_string(),
                says: format!(
                    "{} may now push CI files when you approve one. Nothing pushes one without that.",
                    account.login
                ),
            }),
            workflow::Outcome::Folded(_) => Ok(CiPermissionAnswer {
                state: "not-offered".to_string(),
                says: "GitHub added this permission to your everyday sign-in, so purlis cannot keep \
                       the two apart and will not push CI files. Push that change from your own \
                       terminal."
                    .to_string(),
            }),
        }
    })
    .await
}

/// Stop waiting on a CI-permission `flow`: the polling ends within a moment, and nothing of it
/// is kept.
#[tauri::command]
#[specta::specta]
pub async fn forge_ci_permission_cancel(flow: u32) {
    let mut held = CI_FLOW.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(flow) = held.take_if(|held| held.number == flow) {
        flow.cancelled.store(true, Ordering::SeqCst);
    }
}
