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
use purlis_core::sandbox;
use purlis_core::secrets::{Ctx, Env};
use serde::Serialize;

use crate::forgepkce::tries;
use crate::planes::{PlaneId, Planes};

/// The address of the page that posts the manifest, while a try waits on it: what Open the page
/// again opens. In memory alone, and gone with the try.
static OPENED: Mutex<Option<String>> = Mutex::new(None);

/// An owner the App can be made under.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct IdentityOwner {
    pub name: String,
    /// The person's own account, as their forge sign-in says.
    pub personal: bool,
}

/// One forge host's row (Q6, Q17): what it acts as, on how many repos, and for whom.
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
    /// Personas with an Allow of their own still to give: what else their list holds wasn't shown
    /// here, so Finish doesn't allow it.
    pub to_allow: Vec<String>,
}

/// What a removal left, for the row to say.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct IdentityRemoved {
    /// The App's page on GitHub, where what remains is deleted.
    pub settings_page: Option<String>,
    pub tokens_revoked: u32,
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
        install_page: format!(
            "https://{}/apps/{}/installations/new",
            core::HOST,
            stand.slug
        ),
        settings_page: stand.settings_page(),
    }
}

fn row_of(ctx: &Ctx, held: core::Held) -> IdentityRow {
    let proved = core::prove(ctx, &held.vault, &held.key, &held.app);
    let mut row = IdentityRow {
        host: core::HOST.to_owned(),
        app: held.app,
        personas: held.personas,
        acts_as: None,
        repos: None,
        settings_page: None,
        trouble: None,
    };
    match proved {
        Ok(proof) => {
            row.acts_as = Some(proof.stand.acts_as());
            row.settings_page = Some(proof.stand.settings_page());
            row.repos = Some(u32::try_from(proof.repos).unwrap_or(u32::MAX));
        }
        Err(why) => row.trouble = Some(why),
    }
    row
}

/// The owners offered: those the project's clones are under, the person's own account marked.
fn owners(root: &std::path::Path) -> Vec<IdentityOwner> {
    let mine: Vec<String> = crate::forgeaccounts::config_root()
        .ok()
        .and_then(|config| accounts::listed(&config).ok())
        .unwrap_or_default()
        .into_iter()
        .filter(|account| account.host == core::HOST)
        .map(|account| account.login)
        .collect();
    core::owners_on_github(root)
        .into_iter()
        .map(|name| IdentityOwner {
            personal: mine.iter().any(|login| login.eq_ignore_ascii_case(&name)),
            name,
        })
        .collect()
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
            .map(
                |app| match core::stand(&ctx, core::VAULT, &core::key_of(&app), &app) {
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
                },
            )
            .collect();
        Ok(IdentitiesShown {
            rows,
            pending,
            owners: owners(&root),
            personas: purlis_core::workspaces::Plane::open(root.clone())
                .personas()
                .map_err(|e| e.to_string())?,
        })
    })
    .await
}

/// **Makes the App on GitHub**: the page opened in the person's browser, its one redirect taken,
/// the key stored, and the page for choosing its repos opened. `owner` is an organisation, or
/// none with `personal` for the person's own account. Answers the client id.
#[tauri::command]
#[specta::specta]
pub async fn agent_identity_create(
    app: tauri::AppHandle,
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    attempt: String,
    owner: Option<String>,
    personal: bool,
    workflows: bool,
) -> Result<String, String> {
    let ctx = ctx_of(&planes, &plane)?;
    let cancelled = tries(|all| all.begin(&attempt))?;
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
                Owner::Organisation(owner.ok_or("Choose who owns the App.")?)
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
            *OPENED.lock().unwrap_or_else(|e| e.into_inner()) = Some(page.to_owned());
            browse(page)
        };
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
        *OPENED.lock().unwrap_or_else(|e| e.into_inner()) = None;
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

/// **Opens GitHub's page again** for the try under way. The page's address never reaches the window.
#[tauri::command]
#[specta::specta]
pub async fn agent_identity_reopen(app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt as _;
    let page = OPENED
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
    blocking(move || {
        core::stand(&ctx, core::VAULT, &core::key_of(&app), &app).map(|stand| stand_of(&stand))
    })
    .await
}

/// **The exact lines Finish adds** (Q11), from the one source Finish writes from.
#[tauri::command]
#[specta::specta]
pub fn agent_identity_lines(app: String, personas: Vec<String>) -> String {
    core::lines_shown(&app, &personas)
}

/// **Finish** (Q11, Q17): proves a real token first, so a failed proof writes nothing, then adds
/// the lines, then allows them on this machine. A persona with anything else waiting for an Allow
/// isn't allowed here: only these lines were shown, so that stays the person's own Allow.
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
        let key = core::key_of(&app);
        let proof = core::prove(&ctx, core::VAULT, &key, &app)?;
        let before = sandbox::persona::shown(&root);
        let clear = |persona: &str| {
            before
                .iter()
                .find(|one| one.persona == persona)
                .is_none_or(|one| one.allowed() || (one.hosts.is_empty() && one.tools.is_empty()))
        };
        core::add_lines(&root, &app, &personas)?;
        let after = sandbox::persona::shown(&root);
        let mut to_allow = Vec::new();
        let at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| since.as_secs());
        for persona in &personas {
            let now = after.iter().find(|one| &one.persona == persona);
            match now {
                Some(now) if clear(persona) => crate::sandboxing::allow_persona(
                    &root,
                    persona,
                    &now.digest,
                    &held.audit_then_record(network.as_ref()),
                    at,
                )?,
                _ => to_allow.push(persona.clone()),
            }
        }
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
            },
            read: proof.read,
            to_allow,
        })
    })
    .await
}

/// **Removes an identity at once** (Q13): key, tool lines and live tokens. What remains on
/// GitHub's side is the App itself, and the answer names its page.
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
        let (vault, key) = core::held(&root)?
            .into_iter()
            .find(|one| one.app == app)
            .map_or_else(
                || (core::VAULT.to_owned(), core::key_of(&app)),
                |one| (one.vault, one.key),
            );
        // Asked first: once the key is gone, GitHub can't be asked as the App.
        let settings_page = core::stand(&ctx, &vault, &key, &app)
            .ok()
            .map(|stand| stand.settings_page());
        let revoked = core::remove(&ctx, &root, &vault, &key, &app)?;
        Ok(IdentityRemoved {
            settings_page,
            tokens_revoked: u32::try_from(revoked).unwrap_or(u32::MAX),
        })
    })
    .await
}
