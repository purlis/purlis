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
use purlis_core::forge::registered;
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
    /// `pinned`, `client-id` (another application replaced the pinned one) or `owner`.
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
        for (kind, host) in registered::pinned_hosts(&root)? {
            let Some(pin) = registered::pinned(&root, kind, &host)? else {
                continue;
            };
            let owner = registered::owner_of(&root, kind, &host)?;
            let signed_in = accounts::every_signed_in_on(&root, kind, &host)?.len();
            rows.push(ForgeApplicationRow {
                kind: kind.word().to_string(),
                consent: registered::consent_check(owner.as_deref(), &host),
                host,
                client_id: pin.client_id.to_string(),
                owner: owner.unwrap_or_default(),
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
    app: tauri::AppHandle,
    kind: String,
    host: String,
    client_id: String,
    owner: String,
) -> Result<ForgeApplicationPinned, String> {
    let kind = kind_of(&kind)?;
    blocking(move || {
        let scope = host_scope()?;
        let root = config_root()?;
        let pinned = registered::pin_with_owner(&scope, &root, kind, &host, &client_id, &owner)?;
        if pinned.replaced.is_some() {
            tell_of_changed_pin(&app, &root, kind, &pinned.host, client_id.trim());
        }
        let (change, replaced) = match (pinned.replaced, pinned.owner_was) {
            (Some(id), _) => ("client-id", Some(id)),
            (None, Some(owner)) => ("owner", Some(owner)),
            (None, None) => ("pinned", None),
        };
        Ok(ForgeApplicationPinned {
            consent: registered::consent_check(Some(owner.trim()), &pinned.host),
            host: pinned.host,
            change: change.to_string(),
            replaced,
        })
    })
    .await
}

/// **A changed pin is a Notice** (D1.3 rule 2): every account on `host` that signed in through
/// another application than `now` is told to the window as a trouble, which draws the Notice with
/// its way out (*Sign in again*). Those accounts stop serving chats until then
/// ([`confirmed::account_for`]).
fn tell_of_changed_pin(
    app: &tauri::AppHandle,
    root: &std::path::Path,
    kind: Kind,
    host: &str,
    now: &str,
) {
    use tauri::Emitter;
    let Ok(listed) = accounts::listed(root) else {
        return;
    };
    for account in listed.iter().filter(|a| {
        a.kind == kind
            && a.host.eq_ignore_ascii_case(host)
            && a.state == State::SignedIn
            && a.client_id.as_deref().is_some_and(|id| id != now)
    }) {
        let _ = app.emit(
            crate::forgeaccounts::SIGN_IN_TROUBLE,
            crate::forgeaccounts::trouble_said(
                &account.id,
                &purlis_core::forge::refresh::Trouble::PinChanged,
            ),
        );
    }
}

/// Takes `host`'s pinned application away. Accounts signed in through it stay listed and keep
/// their tokens until they end; they can no longer be renewed.
#[tauri::command]
#[specta::specta]
pub async fn forge_remove_application(
    app: tauri::AppHandle,
    kind: String,
    host: String,
) -> Result<(), String> {
    let kind = kind_of(&kind)?;
    blocking(move || {
        let scope = host_scope()?;
        let root = config_root()?;
        registered::unpin(&scope, &root, kind, &host)?;
        // Nothing is pinned now, so every account that signed in through an application is
        // another's: told at once, each time, not once per run.
        tell_of_changed_pin(&app, &root, kind, &host, "");
        Ok(())
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
    /// `github` or `gitlab`: with the host and the repo number, what names the repo.
    pub kind: String,
    /// The forge's own number for the repo, as text (a repo number can outgrow what the window's
    /// numbers hold).
    pub repo_id: String,
    pub host: String,
    pub path: String,
    /// The account serving it, or null when it is signed out or gone.
    pub serving: Option<ProjectAccountChoice>,
    /// `serving`; `signed-out` (the chosen account is signed out, or holds CI-file permission:
    /// the person signs it in again, or chooses another; **no other account serves in its
    /// place**); `unchosen` (several accounts are signed in, and none was chosen); or
    /// `application-changed` (the chosen account signed in through an application the host no
    /// longer pins).
    pub state: String,
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
                let (serving, state) = match confirmed::account_for(&root, &repo, &accounts_here) {
                    Serving::Account(account) => (Some(choice_of(&account)), "serving"),
                    Serving::ChosenIsSignedOut { .. } => (None, "signed-out"),
                    Serving::Unchosen => (None, "unchosen"),
                    Serving::ApplicationChanged { .. } => (None, "application-changed"),
                };
                ProjectAccountRow {
                    kind: repo.kind.word().to_string(),
                    clone_key: repo.clone_key,
                    repo_id: repo.repo_id.to_string(),
                    host: repo.host,
                    path: repo.path,
                    serving,
                    state: state.to_string(),
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
    kind: String,
    host: String,
    repo_id: String,
    account_id: String,
) -> Result<(), String> {
    let kind = kind_of(&kind)?;
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
        confirmed::choose_account(&scope, &root, &clone_key, kind, &host, repo_id, &account)
            .map(|_| ())
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
        let clone = clone_in_project(&project, &folder)?;
        confirm_clone(&scope, &clone, account_id.as_deref())
    })
    .await
}

/// The clone at `folder` of `project`: a child of the project, named relative to it, **as the
/// canonical path that was checked** (so a link swapped in after the check is never followed).
/// Refused when `folder` is empty, absolute, holds `..`, or (by a link) leads out of the project,
/// so a press can never confirm a repo outside the workspace.
fn clone_in_project(project: &std::path::Path, folder: &str) -> Result<std::path::PathBuf, String> {
    use std::path::Component;
    let refused = || "That folder is not inside this project.".to_string();
    let relative = std::path::Path::new(folder);
    if folder.is_empty()
        || !relative
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
    {
        return Err(refused());
    }
    let joined = project.join(relative);
    let real = joined.canonicalize().map_err(|_| refused())?;
    let root = project.canonicalize().map_err(|_| refused())?;
    if real == root || !real.starts_with(&root) {
        return Err(refused());
    }
    Ok(real)
}

/// Confirms `clone`, which the trust list holds an address for, with the account named, or the
/// host's only one. **Several accounts and none named is a question for the person, and the repo
/// is recorded all the same**, with no account ([`confirmed::record_unchosen`]): its number is read
/// as whichever signed-in account on the host can see it, which serves nothing, and the project
/// accounts list asks which account serves it. None signed in is a sign-in.
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
    let keyring = accounts::keyring_at(&root);
    let lookup = AsThePerson {
        keyring: keyring.as_ref(),
        reach: &token_check,
    };
    let account = match account_id {
        Some(id) => on_host
            .into_iter()
            .find(|a| a.id == id)
            .ok_or_else(|| format!("That account is not signed in on {host}."))?,
        None => match confirmed::default_account(on_host) {
            Pick::Only(account) => account,
            Pick::Choose(several) => {
                let url = format!("https://{place}");
                let found = several
                    .iter()
                    .find_map(|a| confirmed::candidate_of(&url, &lookup, a).ok());
                return Ok(match found {
                    Some(candidate) => {
                        let repo = confirmed::record_unchosen(scope, &root, clone, &candidate)?;
                        confirmed_answer("choose", Some(&repo))
                    }
                    None => confirmed_answer("choose", None),
                });
            }
            Pick::SignIn => return Ok(confirmed_answer("sign-in", None)),
        },
    };
    // The application the account signed in through, which the host still pins: an account that
    // another application made records nothing.
    let client_id = confirmed::application_of(&root, &account)?;
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

/// After *Trust this clone*: what was confirmed for the clone is taken away first and read afresh
/// (ADR 0128 D4: a trust is a new confirmation, so a repo the clone no longer is, or an account
/// that was removed, is not served by an old entry), then the same.
pub(crate) fn confirm_after_trust(clone: &std::path::Path) {
    if purlis_core::fence::FENCED {
        return;
    }
    // A confirmation that was not taken away is not read afresh over: stop, and say so.
    let (Ok(scope), Ok(root)) = (host_scope(), config_root()) else {
        return;
    };
    if let Err(why) = confirmed::forget_clone(&scope, &root, clone) {
        tracing::warn!(
            "purlis: an earlier confirmation of the clone was not taken away, so it was not read \
             again: {why}"
        );
        return;
    }
    confirm_after_clone(clone);
}

/// After a repo was removed from its workspace: what was confirmed for its clone, named by the key
/// read before it went, is taken away. Best effort.
pub(crate) fn forget_removed_clone(key: Option<&str>) {
    if purlis_core::fence::FENCED {
        return;
    }
    let (Some(key), Ok(scope), Ok(root)) = (key, host_scope(), config_root()) else {
        return;
    };
    if let Err(why) = confirmed::forget_by_key(&scope, &root, key) {
        tracing::info!("purlis: the confirmation of a removed clone was not taken away: {why}");
    }
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
        let scope = host_scope()?;
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
        // The second sign-in asks the application the account signed in through, and no other:
        // GitHub then reads it as the same grant (D1.1).
        if account
            .client_id
            .as_deref()
            .is_some_and(|id| id != registration.client_id)
        {
            return Err(format!(
                "{} now uses a different application than the one you signed in through. Sign in \
                 again, then allow CI files.",
                account.host
            ));
        }
        if purlis_core::fence::FENCED {
            return Err(
                "A test build of purlis does not ask GitHub for permission through the browser."
                    .to_string(),
            );
        }
        // The account is recorded as not known to be free of `workflow` before the flow starts:
        // the flow can end (cancelled, expired, quit) without reaching `keep`, and GitHub may
        // have widened the everyday grant by then. If this cannot be written, nothing starts.
        workflow::begin(&scope, &root, &account)?;
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
    // Like every command here it is the window's alone: a process a chat started cannot stop the
    // person's sign-in (D5).
    if host_scope().is_err() {
        return;
    }
    let mut held = CI_FLOW.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(flow) = held.take_if(|held| held.number == flow) {
        flow.cancelled.store(true, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::clone_in_project;

    #[test]
    fn a_folder_is_a_child_of_the_project_and_nothing_else() {
        let project = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(project.path().join("repos/app")).unwrap();
        let outside = tempfile::tempdir().unwrap();
        // What it answers is the canonical path it checked, not the spelling it was given.
        assert_eq!(
            clone_in_project(project.path(), "repos/app").unwrap(),
            project.path().join("repos/app").canonicalize().unwrap()
        );
        for bad in [
            "",
            ".",
            "..",
            "../x",
            "repos/../..",
            "/etc",
            "repos/missing",
        ] {
            assert!(clone_in_project(project.path(), bad).is_err(), "{bad:?}");
        }
        assert!(clone_in_project(project.path(), outside.path().to_str().unwrap()).is_err());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(outside.path(), project.path().join("link")).unwrap();
            assert!(clone_in_project(project.path(), "link").is_err());
        }
    }
}
