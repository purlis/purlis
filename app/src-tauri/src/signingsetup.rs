// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Signed commits, set up from the window** (#5628, spec #5607, ADR 0116): the commands the
//! Signed commits card in Settings › You › Accounts and its setup dialog call. Each is the
//! window's alone (`session_protocol::ui::WINDOW_ONLY`): no chat or link reaches the signing key
//! or adds it to an account.
//!
//! The steps are `purlis_core::signingsetup`'s and `purlis_core::forge::signingkeys`'s; the
//! forge is asked with the window's own sign-in on github.com, never `gh`. The permission
//! step-up is `forgeaccounts::forge_signing_permission_start`'s.

use std::collections::HashMap;
use std::sync::Mutex;

use purlis_core::commitsign;
use purlis_core::forge::accounts::ForgeAccount;
use purlis_core::forge::signingkeys::{self, Refused};
use purlis_core::forge::{Caller, Forge, Kind, accounts};
use purlis_core::signingsetup;

use crate::forgeaccounts::{blocking, config_root};

/// What the card shows: whether this device has a signing key, and the forge accounts it goes to.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct SigningState {
    /// "On": this device has a signing key (ADR 0116 §1). Nothing else turns it on.
    pub on: bool,
    /// What the key is titled on the forge: `purlis · <device>`.
    pub title: String,
    /// Each forge account signed in on this machine, one per forge and host, GitHub and GitLab
    /// alike, in the order they were added.
    pub accounts: Vec<SigningAccount>,
}

/// Which forge account a signing step acts on: its forge (`github`, `gitlab`) and host.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct SigningForge {
    pub kind: String,
    pub host: String,
}

impl SigningForge {
    fn forge(&self) -> Result<Forge, String> {
        let kind = Kind::parse(&self.kind)
            .ok_or_else(|| format!("{} is not a forge purlis signs for", self.kind))?;
        Ok(signingsetup::forge_of(
            kind,
            &self.host.to_ascii_lowercase(),
        ))
    }
}

/// A forge account setup adds the key to.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct SigningAccount {
    /// `github` or `gitlab`.
    pub kind: String,
    pub login: String,
    pub host: String,
    /// The forge, as a sentence names it: `GitHub`, `GitLab`.
    pub forge: String,
    /// Whether its sign-in may add a signing key already, so setup asks for no permission:
    /// GitLab's `api` always may.
    pub may_add: bool,
    /// The email the last Check in this window found verified, for "Verified on <forge> ✓".
    /// Only the forge's own answer sets it, and it lasts the window's life.
    pub verified_as: Option<String>,
    /// Whether the forge verifies SSH-signed commits: a GitLab older than 15.7 does not, and is
    /// offered no setup (#5629).
    pub verifies: bool,
}

/// What Check found: the forge's own verdict on a signed commit.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct SigningCheck {
    pub verified: bool,
    /// The forge's reason, in GitHub's words, such as `valid`, `unverified_email` or
    /// `unknown_key`, or `nothing_pushed` where GitLab has no signed chat commit to read yet:
    /// the window words it.
    pub reason: String,
    /// The email the commit was made as: chat commits use it too.
    pub email: String,
    /// The repository the commit is in.
    pub repo: String,
}

/// How adding the key went.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum SigningAdded {
    /// The forge has it now, or had it already.
    Added,
    /// The sign-in may not add it: setup asks for the permission, or the person adds it by hand.
    NeedsPermission,
}

/// An account, by its forge, host and login.
type AccountKey = (Kind, String, String);

/// The email the last Check in this window found verified, by account.
static VERIFIED: Mutex<Option<HashMap<AccountKey, String>>> = Mutex::new(None);

/// Whether each GitLab host verifies SSH-signed commits, as its version said: read once a window.
static VERIFIES: Mutex<Option<HashMap<String, bool>>> = Mutex::new(None);

/// What a sentence says to do where no account is signed in on `forge`.
fn sign_in_first(forge: &Forge) -> String {
    format!(
        "Sign in to {} on {} first, in Settings › You › Accounts.",
        forge.kind.display(),
        forge.host
    )
}

/// The account signed in on `forge`, where one is.
fn the_account(root: &std::path::Path, forge: &Forge) -> Result<ForgeAccount, String> {
    accounts::signed_in_on(root, forge.kind, &forge.host)?.ok_or_else(|| sign_in_first(forge))
}

/// Every forge account signed in on this machine, the first on each forge and host.
fn every_account(root: &std::path::Path) -> Result<Vec<ForgeAccount>, String> {
    let mut seen = Vec::new();
    Ok(accounts::listed(root)?
        .into_iter()
        .filter(|account| account.state == accounts::State::SignedIn)
        .filter(|account| {
            let key = (account.kind, account.host.to_ascii_lowercase());
            let new = !seen.contains(&key);
            seen.push(key);
            new
        })
        .collect())
}

/// The window's transport to `forge`, with its own sign-in.
fn transport(
    forge: &Forge,
) -> Result<std::sync::Arc<dyn purlis_core::forge::transport::Transport>, String> {
    signingkeys::window(forge, &Caller::window()).map_err(|_| sign_in_first(forge))
}

/// This device's signing key, or a sentence where it has none.
fn the_key(root: &std::path::Path) -> Result<commitsign::Signer, String> {
    let keyring = commitsign::this_builds_keyring(root);
    signingsetup::key_kept(root, keyring.as_ref())?
        .ok_or_else(|| "This Mac has no signing key yet. Make it first.".to_string())
}

/// Whether `forge` verifies SSH-signed commits: GitHub does; a GitLab's version is read once a
/// window. Where it cannot be read, setup is offered, and its Check says what GitLab says.
fn verifies(forge: &Forge) -> bool {
    if forge.kind == Kind::GitHub {
        return true;
    }
    let mut known = VERIFIES.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(said) = known.get_or_insert_default().get(&forge.host) {
        return *said;
    }
    drop(known);
    let Ok(said) =
        transport(forge).and_then(|sent| signingkeys::verifies_over(sent.as_ref(), forge))
    else {
        return true;
    };
    VERIFIES
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get_or_insert_default()
        .insert(forge.host.clone(), said);
    said
}

/// The email the last Check here found `account` verified as.
fn verified_as(account: &ForgeAccount) -> Option<String> {
    VERIFIED
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get_or_insert_default()
        .get(&(account.kind, account.host.clone(), account.login.clone()))
        .cloned()
}

/// `account` found verified as `email` by a Check, or not verified.
fn note_verified(account: &ForgeAccount, email: Option<String>) {
    let mut verified = VERIFIED.lock().unwrap_or_else(|e| e.into_inner());
    let map = verified.get_or_insert_default();
    let key = (account.kind, account.host.clone(), account.login.clone());
    match email {
        Some(email) => map.insert(key, email),
        None => map.remove(&key),
    };
}

/// The scopes an account needs to add a signing key: GitHub's step-up, GitLab's `api`.
fn may_add(account: &ForgeAccount) -> bool {
    match account.kind {
        Kind::GitHub => accounts::holds(account, &signingkeys::GITHUB_SCOPES),
        Kind::GitLab => accounts::holds(account, &["api"]),
    }
}

/// **What the Signed commits card shows.** Asks the keychain only whether the key is there, never
/// for the key, so opening Settings never makes macOS ask ([`signing_key_shown`] reads it). A
/// GitLab's version is asked once a window, to tell a server that can't verify signed commits.
#[tauri::command]
#[specta::specta]
pub async fn signing_state() -> Result<SigningState, String> {
    blocking(|| {
        let root = config_root()?;
        let keyring = commitsign::this_builds_keyring(&root);
        let on = signingsetup::key_is_there(&root, keyring.as_ref())?;
        let accounts = every_account(&root)?
            .into_iter()
            .map(|account| {
                let forge = signingsetup::forge_of(account.kind, &account.host);
                SigningAccount {
                    kind: account.kind.word().to_string(),
                    may_add: may_add(&account),
                    verified_as: verified_as(&account),
                    verifies: verifies(&forge),
                    forge: account.kind.display().to_string(),
                    login: account.login,
                    host: account.host,
                }
            })
            .collect();
        Ok(SigningState {
            on,
            title: signingkeys::title(),
            accounts,
        })
    })
    .await
}

/// This device's key as Details and the by-hand step show it: never its private half.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct SigningKeyShown {
    /// `SHA256:…`, as GitHub shows it.
    pub fingerprint: String,
    /// The public line to paste on GitHub's page.
    pub public_key: String,
}

/// **This device's key, shown**: read from the Keychain only when the person opens Details or
/// adds the key by hand, so the card itself never makes macOS ask.
#[tauri::command]
#[specta::specta]
pub async fn signing_key_shown() -> Result<SigningKeyShown, String> {
    blocking(|| {
        let key = the_key(&config_root()?)?;
        Ok(SigningKeyShown {
            fingerprint: key.fingerprint(),
            public_key: key.public_line(),
        })
    })
    .await
}

/// **Step 1: this device's key, made and kept in the Keychain**, where it has none. macOS asks the
/// person here; where they say no, the sentence says to try again and choose Always Allow.
#[tauri::command]
#[specta::specta]
pub async fn signing_make_key() -> Result<String, String> {
    blocking(|| {
        let root = config_root()?;
        let keyring = commitsign::this_builds_keyring(&root);
        signingsetup::key_made(&root, keyring.as_ref())
            .map(|(key, _)| key.fingerprint())
            .map_err(|_| {
                "macOS said no, so purlis couldn't keep the key. Try again and choose Always \
                 Allow."
                    .to_string()
            })
    })
    .await
}

/// **Step 2: the key added to the account on `forge`** with the window's sign-in, under
/// `purlis · <device>`: on GitLab as a signing key only. One the forge has already is added again
/// as nothing.
#[tauri::command]
#[specta::specta]
pub async fn signing_add_key(forge: SigningForge) -> Result<SigningAdded, String> {
    blocking(move || {
        let forge = forge.forge()?;
        let root = config_root()?;
        let key = the_key(&root)?;
        the_account(&root, &forge)?;
        let transport = transport(&forge)?;
        match signingkeys::listed_over(transport.as_ref(), &forge) {
            Ok(listed) if listed.this_device(&key).is_some() => return Ok(SigningAdded::Added),
            Ok(_) => {}
            Err(Refused::NeedsPermission) => return Ok(SigningAdded::NeedsPermission),
            Err(why) => return Err(why.said(forge.kind)),
        }
        match signingkeys::add_over(transport.as_ref(), &forge, &key, &signingkeys::title()) {
            Ok(_) => Ok(SigningAdded::Added),
            Err(Refused::NeedsPermission) => Ok(SigningAdded::NeedsPermission),
            Err(why) => Err(why.said(forge.kind)),
        }
    })
    .await
}

/// **Whether `forge` lists this device's key on the account yet**, from its public list, which
/// needs no permission: how the by-hand path notices the person added it.
#[tauri::command]
#[specta::specta]
pub async fn signing_key_listed(forge: SigningForge) -> Result<bool, String> {
    blocking(move || {
        let forge = forge.forge()?;
        let root = config_root()?;
        let key = the_key(&root)?;
        let account = the_account(&root, &forge)?;
        let transport = transport(&forge)?;
        signingkeys::listed_publicly_over(transport.as_ref(), &forge, &account.login)
            .map(|listed| listed.this_device(&key).is_some())
            .map_err(|why| why.said(forge.kind))
    })
    .await
}

/// **Step 3, Check**, on the account on `forge`.
///
/// - **GitHub**: a test commit signed with this device's key, made in `repo` (`owner/name`) or,
///   with none, in the account's own most recently pushed repository, and GitHub's own verdict
///   on it. Never on a branch.
/// - **GitLab**, which makes no signed commit of its own (#5629): GitLab's verdict on `commit` of
///   `repo` where setup was opened for one, else on the newest signed chat commit the app
///   pushed to that GitLab. With none yet, the reason is `nothing_pushed` and nothing is verified.
///
/// A verdict of Verified is kept for the card while the window runs.
#[tauri::command]
#[specta::specta]
pub async fn signing_check(
    forge: SigningForge,
    repo: Option<String>,
    commit: Option<String>,
) -> Result<SigningCheck, String> {
    blocking(move || {
        let forge = forge.forge()?;
        let root = config_root()?;
        let key = the_key(&root)?;
        let account = the_account(&root, &forge)?;
        let who = signingsetup::who_commits_here(&root)?;
        let transport = transport(&forge)?;
        let shown = forge.kind.display();
        let (checked, repo) = match forge.kind {
            Kind::GitHub => {
                let repo = match repo {
                    Some(repo) => repo,
                    None => signingsetup::a_repo_to_check_in(transport.as_ref(), &forge)?
                        .ok_or_else(|| {
                            format!(
                                "Your {shown} account has no repository of its own to check \
                                 in. Make one, then check again."
                            )
                        })?,
                };
                let when = chrono::Utc::now().timestamp();
                let checked = signingsetup::check_with_a_test_commit(
                    transport.as_ref(),
                    &forge,
                    &key,
                    &repo,
                    &who,
                    when,
                )?;
                (checked, repo)
            }
            Kind::GitLab => {
                let asked = match (repo, commit) {
                    (Some(repo), Some(commit)) => Some((repo, commit)),
                    _ => signingsetup::last_signed_push(&root, &forge)
                        .map(|push| (push.repo, push.commit)),
                };
                let Some((repo, commit)) = asked else {
                    note_verified(&account, None);
                    return Ok(SigningCheck {
                        verified: false,
                        reason: "nothing_pushed".to_string(),
                        email: who.email,
                        repo: String::new(),
                    });
                };
                let checked = signingsetup::check_a_pushed_commit(
                    transport.as_ref(),
                    &forge,
                    &key,
                    &repo,
                    &commit,
                )?;
                (checked, repo)
            }
        };
        note_verified(&account, checked.verified.then(|| who.email.clone()));
        Ok(SigningCheck {
            verified: checked.verified,
            reason: checked.reason,
            email: who.email,
            repo,
        })
    })
    .await
}

// ---------------------------------------------------------------------------------------
// A forge's refusal of unsigned commits, and Sign and retry (#5630, #5633)
// ---------------------------------------------------------------------------------------

/// **A chat's branch folder whose push the forge refused for unsigned commits** (#5630), as the
/// lane, the chat and the grouped notice show it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct SigningWaiting {
    pub ws: String,
    pub repo: String,
    pub piece: String,
    pub branch: String,
    /// `github` or `gitlab`, and the forge as a sentence names it.
    pub kind: String,
    pub forge: String,
    pub host: String,
    /// The repository's path on the forge: what setup is checked in.
    pub path: String,
    /// The commit whose push was refused.
    pub commit: String,
    /// The app's number for the chat that pushed, while it is open.
    #[specta(optional)]
    pub chat: Option<u32>,
    /// Signed and sent since: shown as done until the window has shown it.
    pub sent: bool,
}

/// One branch folder, by its workspace, repo and piece.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct SigningLane {
    pub ws: String,
    pub repo: String,
    pub piece: String,
}

/// How Sign and retry went for one branch folder.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct SigningRetried {
    pub ws: String,
    pub repo: String,
    pub piece: String,
    /// How many commits were signed again, where it was sent.
    pub signed: u32,
    /// Why it was not sent, where it was not: it stays waiting.
    #[specta(optional)]
    pub why: Option<String>,
}

fn root_of(
    planes: &crate::planes::Planes,
    plane: &crate::planes::PlaneId,
) -> Result<std::path::PathBuf, String> {
    Ok(planes.held(plane)?.root().to_path_buf())
}

/// **The work in `plane` waiting on signing**: each branch folder whose push the forge refused
/// for unsigned commits, oldest first, and those signed and sent since.
#[tauri::command]
#[specta::specta]
pub async fn signing_waiting(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
) -> Result<Vec<SigningWaiting>, String> {
    let root = root_of(&planes, &plane)?;
    blocking(move || {
        Ok(signingsetup::retry::waiting(&root)
            .into_iter()
            .map(|one| SigningWaiting {
                forge: one.kind.display().to_string(),
                kind: one.kind.word().to_string(),
                ws: one.ws,
                repo: one.repo,
                piece: one.piece,
                branch: one.branch,
                host: one.host,
                path: one.path,
                commit: one.commit,
                chat: one.chat,
                sent: one.sent,
            })
            .collect())
    })
    .await
}

/// **Sign and retry** (#5630, ADR 0116 §4), for each of `lanes` that waits on signing: the
/// commits on its branch its remote has none of are signed with this device's key, and the
/// branch is sent again, as a chat's push is, with the window's own sign-in. Behind the person's
/// press in the window alone (`WINDOW_ONLY`). One that fails again stays waiting, with why.
#[tauri::command]
#[specta::specta]
pub async fn signing_sign_and_retry(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    lanes: Vec<SigningLane>,
) -> Result<Vec<SigningRetried>, String> {
    let root = root_of(&planes, &plane)?;
    blocking(move || {
        let config = config_root()?;
        let key = the_key(&config)?;
        let waiting = signingsetup::retry::waiting(&root);
        Ok(lanes
            .into_iter()
            .map(|lane| {
                let retried = match waiting
                    .iter()
                    .find(|one| one.is(&lane.ws, &lane.repo, &lane.piece) && !one.sent)
                {
                    None => Err("it isn't waiting on signing any more".to_string()),
                    Some(one) => retried(&root, &config, one, &key),
                };
                let (signed, why) = match retried {
                    Ok(signed) => (signed, None),
                    Err(why) => (0, Some(why)),
                };
                SigningRetried {
                    ws: lane.ws,
                    repo: lane.repo,
                    piece: lane.piece,
                    signed,
                    why,
                }
            })
            .collect())
    })
    .await
}

/// One branch folder signed and sent again, as the app runs git for a chat: no global or system
/// git config, and the window's own sign-in to reach the forge. How many commits it signed.
fn retried(
    root: &std::path::Path,
    config: &std::path::Path,
    one: &signingsetup::retry::Waiting,
    key: &commitsign::Signer,
) -> Result<u32, String> {
    let isolation = purlis_core::worktree::git::Isolated::operators();
    let done = purlis_core::worktree::git::isolated(&isolation, || {
        crate::forgeaccounts::window_git(|| {
            signingsetup::retry::sign_and_retry(root, one, key, &mut |_| {})
        })
    })?;
    let piece = purlis_core::pieceupdate::Piece {
        root,
        ws: &one.ws,
        repo: &one.repo,
        piece: &one.piece,
    };
    purlis_core::pieceupdate::note_if_signed(&piece, config);
    Ok(u32::try_from(done.signed).unwrap_or(u32::MAX))
}

/// **What was signed and sent, cleared** once the window has shown it.
#[tauri::command]
#[specta::specta]
pub async fn signing_forget_sent(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
) -> Result<(), String> {
    let root = root_of(&planes, &plane)?;
    blocking(move || signingsetup::retry::forget_sent(&root)).await
}

// ---------------------------------------------------------------------------------------
// The email chat commits use (#5631)
// ---------------------------------------------------------------------------------------

/// One verified email address of the account, as the Check step offers it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct SigningEmail {
    pub email: String,
    pub primary: bool,
    /// The forge's private address for commits (GitHub's noreply).
    pub private: bool,
}

/// The account's verified addresses, and the forge's page to add and verify one.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct SigningEmails {
    pub emails: Vec<SigningEmail>,
    /// The forge's email settings page, for an account with none verified.
    pub settings: String,
}

/// **The verified email addresses of the account on `forge`** (#5631): what Check offers to sign
/// chat commits as, where the email they use now is not one the forge verifies.
#[tauri::command]
#[specta::specta]
pub async fn signing_emails(forge: SigningForge) -> Result<SigningEmails, String> {
    blocking(move || {
        let forge = forge.forge()?;
        let root = config_root()?;
        the_account(&root, &forge)?;
        let transport = transport(&forge)?;
        let emails = signingsetup::verified_emails(transport.as_ref(), &forge)?
            .into_iter()
            .map(|one| SigningEmail {
                email: one.email,
                primary: one.primary,
                private: one.private,
            })
            .collect();
        Ok(SigningEmails {
            emails,
            settings: signingsetup::email_settings(&forge),
        })
    })
    .await
}

/// **Sign chat commits on this Mac as `email`** (#5631): one of the account on `forge`'s
/// verified addresses, asked again of the forge, so nothing else is taken. Chat commits made
/// from now on carry it, and Check is run again by the dialog.
#[tauri::command]
#[specta::specta]
pub async fn signing_use_email(forge: SigningForge, email: String) -> Result<(), String> {
    blocking(move || {
        let forge = forge.forge()?;
        let root = config_root()?;
        the_account(&root, &forge)?;
        let transport = transport(&forge)?;
        let verified = signingsetup::verified_emails(transport.as_ref(), &forge)?;
        if !verified
            .iter()
            .any(|one| one.email.eq_ignore_ascii_case(&email))
        {
            return Err(format!(
                "{email} isn't a verified email on your {} account, so purlis didn't use it.",
                forge.kind.display()
            ));
        }
        signingsetup::set_commit_email(&root, Some(&email))
    })
    .await
}
