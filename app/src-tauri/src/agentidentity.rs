// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Agent identity, for the window** (#4485, spec #4469): the commands of one view tab and of
//! Pool setup's step, which share them. The flow is the core's
//! ([`purlis_core::forge::agentidentity`]); this file runs it where the person is and gives it
//! the browser.
//!
//! **Nothing secret crosses to the window.** It names an owner, an App's public client id and
//! personas, and hears back the same. The address the browser is sent to, the `state`, GitHub's
//! code and the App's key stay in this process, and the key goes to the vault at once.
//!
//! Every command is the window's alone (`WINDOW_ONLY`): no link and no chat runs any of them.

use std::sync::Mutex;

use purlis_core::forge::agentidentity::{self as core, Owner, Wanted};
use purlis_core::forge::{accounts, loopback, oauth};
use purlis_core::secrets::{Ctx, Env};
use serde::Serialize;

use crate::forgepkce::tries;
use crate::planes::{PlaneId, Planes};

/// The try waiting on GitHub and the address of the page that posts its manifest: what Open the
/// page again opens. In memory alone, and gone with the try.
static OPENED: Mutex<Option<(String, String)>> = Mutex::new(None);

/// An owner the App can be made under.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct IdentityOwner {
    pub name: String,
    /// The person's own account, as their forge sign-in says.
    pub personal: bool,
}

/// One App's row (Q6, Q17): what it acts as, on how many repos, and for whom. A forge host has
/// one, or one per owner where a project's repos are under several.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct IdentityRow {
    pub host: String,
    /// The App's public client id.
    pub app: String,
    pub personas: Vec<String>,
    /// What it acts as, where GitHub could say.
    pub acts_as: Option<String>,
    pub repos: Option<u32>,
    /// The App's page on GitHub, where it is deleted.
    pub settings_page: Option<String>,
    /// Why GitHub couldn't say. The row stays, and can still be removed.
    pub trouble: Option<String>,
    /// A line this flow didn't write: shown, never read with, and removed in the project file.
    pub by_hand: bool,
}

/// An App whose key is kept and whose lines Finish hasn't written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct IdentityPending {
    pub app: String,
    pub stand: Option<IdentityStand>,
    pub trouble: Option<String>,
}

/// An App as GitHub says it is, before Finish.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct IdentityStand {
    pub acts_as: String,
    pub owner: String,
    pub installed: bool,
    pub install_page: String,
    pub settings_page: String,
}

/// What the tab draws.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct IdentitiesShown {
    pub rows: Vec<IdentityRow>,
    pub pending: Vec<IdentityPending>,
    pub owners: Vec<IdentityOwner>,
    pub personas: Vec<String>,
}

/// What Finish did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct IdentityFinished {
    pub row: IdentityRow,
    /// What it read as proof.
    pub read: Option<String>,
    /// Personas with an Allow of their own still to give: Finish allows only what it showed.
    pub to_allow: Vec<String>,
}

/// What a removal left, for the row to say.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct IdentityRemoved {
    /// The App's page on GitHub, where what remains is deleted.
    pub settings_page: Option<String>,
}

fn ctx_of(planes: &Planes, plane: &PlaneId) -> Result<Ctx, String> {
    Ok(Ctx::new(planes.held(plane)?.root(), Env::from_process()))
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|e| format!("Agent identity did not finish: {e}"))?
}

fn stand_of(stand: &core::Stand) -> IdentityStand {
    IdentityStand {
        acts_as: stand.acts_as(),
        owner: stand.owner.clone(),
        installed: !stand.installations.is_empty(),
        install_page: stand.install_page(),
        settings_page: stand.settings_page(),
    }
}

fn row_of(ctx: &Ctx, held: core::Held) -> IdentityRow {
    let mut row = IdentityRow {
        host: core::HOST.to_owned(),
        by_hand: !held.ours(),
        personas: held.personas.clone(),
        app: held.app.clone(),
        acts_as: None,
        repos: None,
        settings_page: None,
        trouble: None,
    };
    // A committed line this flow didn't write opens no vault on this machine.
    if held.ours() {
        match core::prove(ctx, &held.app) {
            Ok(proof) => {
                row.acts_as = Some(proof.stand.acts_as());
                row.settings_page = Some(proof.stand.settings_page());
                row.repos = Some(u32::try_from(proof.repos).unwrap_or(u32::MAX));
            }
            Err(why) => row.trouble = Some(why),
        }
    }
    row
}

/// The owners offered: those the project's clones are under, then the person's own account.
fn owners(root: &std::path::Path) -> Vec<IdentityOwner> {
    let mine: Vec<String> = crate::forgeaccounts::config_root()
        .ok()
        .and_then(|config| accounts::listed(&config).ok())
        .unwrap_or_default()
        .into_iter()
        .filter(|account| account.host == core::HOST)
        .map(|account| account.login)
        .collect();
    let mut offered: Vec<IdentityOwner> = core::owners_on_github(root)
        .into_iter()
        .map(|name| IdentityOwner {
            personal: mine.iter().any(|login| login.eq_ignore_ascii_case(&name)),
            name,
        })
        .collect();
    for login in mine {
        if !offered
            .iter()
            .any(|one| one.name.eq_ignore_ascii_case(&login))
        {
            offered.push(IdentityOwner {
                name: login,
                personal: true,
            });
        }
    }
    offered
}

/// **What the Agent identity tab draws**, read fresh. Each row proves itself against GitHub, so
/// it shows what it acts as now.
#[tauri::command]
#[specta::specta]
pub async fn agent_identities(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<IdentitiesShown, String> {
    let ctx = ctx_of(&planes, &plane)?;
    let root = planes.held(&plane)?.root().to_path_buf();
    blocking(move || {
        let held = core::held(&root)?;
        let rows = held.into_iter().map(|one| row_of(&ctx, one)).collect();
        let pending = core::pending(&ctx, &root)?
            .into_iter()
            .map(|app| match core::stand(&ctx, &app) {
                Ok(stand) => IdentityPending {
                    app,
                    stand: Some(stand_of(&stand)),
                    trouble: None,
                },
                Err(why) => IdentityPending {
                    app,
                    stand: None,
                    trouble: Some(why),
                },
            })
            .collect();
        Ok(IdentitiesShown {
            rows,
            pending,
            owners: owners(&root),
            personas: purlis_core::workspaces::Plane::open(root.clone())
                .personas()
                .map_err(|_| {
                    "purlis couldn't read this project's personas. Try again.".to_owned()
                })?,
        })
    })
    .await
}

/// **Makes the App on GitHub**: the page opened in the person's browser, its one redirect taken,
/// the key stored, and the page for choosing its repos opened. `owner` is the account's name, an
/// organisation's, or with `personal` the person's own. Answers the client id.
#[tauri::command]
#[specta::specta]
pub async fn agent_identity_create(
    app: tauri::AppHandle,
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    attempt: String,
    owner: String,
    personal: bool,
    workflows: bool,
) -> Result<String, String> {
    let ctx = ctx_of(&planes, &plane)?;
    let cancelled = tries(|all| all.begin(&attempt))?;
    let this_try = attempt.clone();
    let done = blocking(move || {
        crate::forgeaccounts::host_scope()?;
        // A test build has no person at a browser, and must never reach the real forge.
        if purlis_core::fence::FENCED {
            return Err("A test build of purlis doesn't make GitHub Apps.".to_owned());
        }
        let wanted = Wanted {
            owner: if personal {
                Owner::Personal(owner)
            } else {
                Owner::Organisation(owner)
            },
            workflows,
        };
        let browse = |page: &str| {
            use tauri_plugin_opener::OpenerExt as _;
            // The opener's own words could repeat the address.
            app.opener()
                .open_url(page, None::<&str>)
                .map_err(|_| "the browser did not open".to_owned())
        };
        let open = |page: &str| {
            *OPENED.lock().unwrap_or_else(|e| e.into_inner()) =
                Some((this_try.clone(), page.to_owned()));
            browse(page)
        };
        core::ready_to_keep(&ctx)?;
        let exchange = oauth::Https::shared();
        let created = core::create_app(
            &wanted,
            &cancelled,
            &core::Ways {
                bind: &loopback::bind_any,
                open: &open,
                exchange: exchange.as_ref(),
                clock: loopback::Clock::system(),
            },
        );
        let _ = OPENED
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take_if(|(opened_by, _)| *opened_by == this_try);
        let created = created?;
        core::keep_key(&ctx, &created)?;
        // The key is safe by now, and Open the page again in the window does the same.
        if browse(&created.install_page()).is_err() {
            tracing::warn!("purlis: the page for choosing an App's repos did not open");
        }
        Ok(created.app.clone())
    })
    .await;
    tries(|all| all.end(&attempt));
    done
}

/// Stop making the App: the listener closes at once, and nothing is kept.
#[tauri::command]
#[specta::specta]
pub fn agent_identity_create_cancel(attempt: String) {
    tries(|all| all.stop(&attempt));
}

/// **Opens GitHub's page again** for the try under way. The page's address never reaches the
/// window.
#[tauri::command]
#[specta::specta]
pub async fn agent_identity_reopen(app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt as _;
    let (_, page) = OPENED
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .ok_or("There's nothing to open. Start over.")?;
    app.opener()
        .open_url(page, None::<&str>)
        .map_err(|_| "the browser did not open".to_owned())
}

/// **How an App stands on GitHub**, asked as the tab waits for it to be installed. Changes nothing.
#[tauri::command]
#[specta::specta]
pub async fn agent_identity_stand(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    app: String,
) -> Result<IdentityStand, String> {
    let ctx = ctx_of(&planes, &plane)?;
    blocking(move || core::stand(&ctx, &app).map(|stand| stand_of(&stand))).await
}

/// **The exact lines Finish adds** (Q11), from the one source Finish writes from.
#[tauri::command]
#[specta::specta]
pub async fn agent_identity_lines(app: String, personas: Vec<String>) -> Result<String, String> {
    Ok(core::lines_shown(&app, &personas))
}

/// **Finish** (Q11, Q17): proves a real token first, so a failed proof writes nothing, then adds
/// the lines, then allows them on this machine. A persona with anything else waiting for an Allow
/// isn't allowed here, nor is the default persona, whose Allow reaches every chat that names no
/// persona: only these lines were shown, so those stay the person's own Allow.
#[tauri::command]
#[specta::specta]
pub async fn agent_identity_finish(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    app: String,
    personas: Vec<String>,
) -> Result<IdentityFinished, String> {
    if personas.is_empty() {
        return Err("Tick at least one persona.".to_owned());
    }
    let ctx = ctx_of(&planes, &plane)?;
    let held = planes.held(&plane)?;
    let network = planes.network().cloned();
    blocking(move || {
        let root = held.root().to_path_buf();
        let proof = core::prove(&ctx, &app)?;
        let at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| since.as_secs());
        let to_allow = core::write_and_allow(&root, &app, &personas, &mut |shown| {
            crate::sandboxing::allow_shown(
                &root,
                shown,
                &held.audit_then_record(network.as_ref()),
                at,
            )
        })?;
        let personas_now = core::held(&root)?
            .into_iter()
            .find(|one| one.app == app)
            .map_or(personas.clone(), |one| one.personas);
        Ok(IdentityFinished {
            row: IdentityRow {
                host: core::HOST.to_owned(),
                app,
                personas: personas_now,
                acts_as: Some(proof.stand.acts_as()),
                repos: Some(u32::try_from(proof.repos).unwrap_or(u32::MAX)),
                settings_page: Some(proof.stand.settings_page()),
                trouble: None,
                by_hand: false,
            },
            read: proof.read,
            to_allow,
        })
    })
    .await
}

/// **Removes an identity at once** (Q13): key, tool lines and live tokens, of what this flow
/// made. What remains on GitHub's side is the App itself, and the answer names its page.
#[tauri::command]
#[specta::specta]
pub async fn agent_identity_remove(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    app: String,
) -> Result<IdentityRemoved, String> {
    let ctx = ctx_of(&planes, &plane)?;
    let root = planes.held(&plane)?.root().to_path_buf();
    blocking(move || {
        // Asked first: once the key is gone, GitHub can't be asked as the App.
        let settings_page = core::stand(&ctx, &app)
            .ok()
            .map(|stand| stand.settings_page());
        core::remove(&ctx, &root, &app)?;
        Ok(IdentityRemoved { settings_page })
    })
    .await
}
