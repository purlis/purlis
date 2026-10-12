// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The welcome's "Create a project" on GitHub or GitLab (#1819): the window's door to
//! `purlis_core::createproject`.
//!
//! Four commands, all of them the window's alone (`WINDOW_ONLY`, and a channel for the creation
//! itself): the owners the person's account can make a repo under, the folder a name goes into,
//! the creation with its stages, and the person's Cancel. The creation answers the folder it
//! made; the window then opens it through the same open-and-trust question as every other open.
//!
//! **Only with a purlis sign-in.** The forge is asked through the native client as the person in
//! the window, on their signed-in account, and the first commit is pushed through purlis's own
//! credential helper (#752). A forge with no account answers "sign in first", which the guided
//! steps turn into their sign-in step; nothing falls back to `gh` or `glab`. Every act claims
//! [`HostScope`], which a process a chat started is refused.
//!
//! One creation runs at a time, which is all a welcome page can start.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use purlis_core::createproject::{self, Created, Plan, Stage};
use purlis_core::forge::accounts::{self, ForgeAccount, State};
use purlis_core::forge::route::HostScope;
use purlis_core::forge::{Caller, Forge, ForgeBackend, Kind, OwnerKind};
use tauri::ipc::Channel;

use crate::firstrun::ForgeWord;

/// The running creation's stop, while one runs.
static RUNNING: Mutex<Option<Arc<AtomicBool>>> = Mutex::new(None);

/// What the owner picker offers: the account's host and login, and whom it can make a repo for.
/// `host` is null when this machine has no purlis sign-in to that forge: sign in first.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct ForgeOwners {
    pub host: Option<String>,
    pub login: String,
    pub owners: Vec<OwnerRow>,
}

/// One owner in the picker.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct OwnerRow {
    /// The login or the group's full path: what the creation is asked for.
    pub name: String,
    /// What the person reads.
    pub title: String,
    /// Whether it is the account itself.
    pub mine: bool,
}

/// What the person asked to create.
#[derive(Debug, Clone, serde::Deserialize, specta::Type)]
pub struct CreateOnForge {
    pub forge: ForgeWord,
    pub owner: String,
    pub name: String,
    /// Private unless the person said public.
    pub private: bool,
    /// A project template's id (FR-17), or null for an empty project.
    pub template: Option<String>,
    /// Where it goes: absolute, from [`new_project_folder`].
    pub folder: String,
}

/// How far a creation has got: the stage running, its place, and every stage's words, so the
/// window lists them all from the first message.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct CreateProgress {
    pub stage: String,
    pub at: u32,
    pub stages: Vec<String>,
}

/// The person's signed-in account on `kind`: the forge's own instance first, then any other host
/// in the order the accounts were added. `None` with no sign-in to that forge.
fn account_for(kind: Kind) -> Result<Option<ForgeAccount>, String> {
    let root = purlis_core::machine::config_root()
        .ok_or("purlis found no home directory to read forge accounts from")?;
    let keyring = accounts::keyring_at(&root);
    let mut signed_in: Vec<ForgeAccount> = accounts::list(&root, keyring.as_ref())?
        .into_iter()
        .filter(|account| account.kind == kind && account.state == State::SignedIn)
        .collect();
    signed_in.sort_by_key(|account| account.host != kind.default_host());
    Ok(signed_in.into_iter().next())
}

/// A forge's host, and its backend as the window asks it.
type OnTheForge = (String, Box<dyn ForgeBackend>);

/// The forge as the window asks it for a creation: the person's account's host, and its backend
/// through the window's transports. In the scenario build, a stand-in forge when the run names
/// one ([`scenario`]).
fn forge_for(kind: Kind, recording: &str) -> Result<Option<OnTheForge>, String> {
    #[cfg(feature = "e2e")]
    if let Some(stand_in) = scenario::forge(kind, recording) {
        return stand_in.map(Some);
    }
    let _ = recording;
    let Some(account) = account_for(kind)? else {
        return Ok(None);
    };
    let forge = Forge {
        kind,
        host: account.host.clone(),
    };
    Ok(Some((account.host, forge.backend())))
}

fn host_scope() -> Result<HostScope, String> {
    HostScope::claim().ok_or_else(|| "Creating a project is the window's, never a chat's.".into())
}

/// Whom the person's account on `forge` can make a repo for. `host` is null when this machine
/// has no purlis sign-in to that forge, and the window asks for one first.
#[tauri::command]
#[specta::specta]
pub async fn forge_owners(forge: ForgeWord) -> Result<ForgeOwners, String> {
    let _scope = host_scope()?;
    let kind = Kind::from(forge);
    tauri::async_runtime::spawn_blocking(move || {
        let Some((host, backend)) = forge_for(kind, "owners")? else {
            return Ok(ForgeOwners {
                host: None,
                login: String::new(),
                owners: Vec::new(),
            });
        };
        let owners = backend
            .owners_to_create_under(&Caller::window())
            .map_err(|e| format!("purlis could not list who you can create for on {host}: {e}"))?;
        let login = owners
            .iter()
            .find(|o| o.kind == OwnerKind::You)
            .map(|o| o.name.clone())
            .unwrap_or_default();
        Ok(ForgeOwners {
            host: Some(host),
            login,
            owners: owners
                .into_iter()
                .map(|o| OwnerRow {
                    mine: o.kind == OwnerKind::You,
                    name: o.name,
                    title: o.title,
                })
                .collect(),
        })
    })
    .await
    .map_err(|err| format!("purlis could not ask the forge: {err}"))?
}

/// The folder a new project named `name` goes into: `<parent>/<name>`, with `parent` the place
/// the person picked last, or `~/Projects`. Answers why `name` cannot be one.
#[tauri::command]
#[specta::specta]
pub fn new_project_folder(name: String, parent: Option<String>) -> Result<String, String> {
    if let Some(why) = createproject::name_refusal(name.trim()) {
        return Err(why);
    }
    let home = purlis_core::getproject::home()
        .ok_or("purlis could not tell which folder is your home. Choose a folder with Choose….")?;
    Ok(
        createproject::folder_for(name.trim(), parent.as_deref().map(Path::new), &home)
            .display()
            .to_string(),
    )
}

/// Creates the project `ask` names, telling `progress` as each stage starts. Answers the folder,
/// to be opened through the trust question; `null` when the person called it off and nothing was
/// left; or the sentence that says why it failed and what was taken back.
#[tauri::command]
#[specta::specta]
pub async fn create_project_on_forge(
    ask: CreateOnForge,
    progress: Channel<CreateProgress>,
) -> Result<Option<String>, String> {
    let _scope = host_scope()?;
    let kind = Kind::from(ask.forge);
    let off = Arc::new(AtomicBool::new(false));
    {
        let mut running = RUNNING
            .lock()
            .map_err(|_| "the creation's state is unreadable")?;
        if running.is_some() {
            return Err("A project is being created already. Wait for it, or cancel it.".into());
        }
        *running = Some(Arc::clone(&off));
    }
    let stopped = Arc::clone(&off);
    let answer = tauri::async_runtime::spawn_blocking(move || {
        let Some((host, backend)) = forge_for(kind, "create")? else {
            return Err(format!(
                "purlis is not signed in to {}. Sign in, then Retry.",
                kind.display()
            ));
        };
        let plan = Plan {
            kind,
            host,
            owner: ask.owner.trim().to_string(),
            name: ask.name.trim().to_string(),
            private: ask.private,
            template: ask.template.filter(|id| !id.is_empty()),
            folder: PathBuf::from(&ask.folder),
        };
        let stages: Vec<String> = Stage::ALL.iter().map(|s| s.words(kind)).collect();
        let hands = hands();
        // The person's own creation, run by the window: their purlis account for its host
        // (#752), and no other credential helper.
        let made = crate::forgeaccounts::window_git(|| {
            createproject::create(
                &plan,
                backend.as_ref(),
                &Caller::window(),
                hands.as_ref(),
                &stopped,
                &mut |stage| {
                    // A window that has gone has nobody to tell; the creation goes on until it is
                    // called off.
                    let at = Stage::ALL.iter().position(|s| *s == stage).unwrap_or(0);
                    let _ = progress.send(CreateProgress {
                        stage: stage.words(kind),
                        at: u32::try_from(at).unwrap_or(0),
                        stages: stages.clone(),
                    });
                },
            )
        });
        Ok(made)
    })
    .await;
    if let Ok(mut running) = RUNNING.lock() {
        *running = None;
    }
    match answer.map_err(|err| format!("purlis could not create the project: {err}"))?? {
        Ok(Created::Made { folder, .. }) => Ok(Some(folder.display().to_string())),
        Ok(Created::CalledOff { left }) if left.is_empty() => Ok(None),
        Ok(Created::CalledOff { left }) => Err(format!("Cancelled. {}", left.join(" "))),
        Err(refused) => Err(refused.sentence),
    }
}

/// The person's Cancel: the running creation stops at the end of the stage it is in, then takes
/// back what it made. Nothing happens when none runs.
#[tauri::command]
#[specta::specta]
pub fn cancel_create_project() {
    if let Ok(running) = RUNNING.lock()
        && let Some(off) = running.as_ref()
    {
        off.store(true, Ordering::SeqCst);
    }
}

/// The work on this machine: the window's own, or in the scenario build the stand-in's.
fn hands() -> Box<dyn createproject::Hands + Send> {
    #[cfg(feature = "e2e")]
    if let Some(stand_in) = scenario::hands() {
        return stand_in;
    }
    Box::new(createproject::OnThisMachine::for_the_window())
}

/// **The scenario build's stand-in forge** (#1819): the run's machine has no forge account, and
/// no spec may make a repo on a real forge. In the `e2e` build only, a directory named by
/// [`scenario::FORGE`] stands in for one: `owners.json` and `create.json` are recordings of the
/// forge contract's shape that answer the owner picker and the creation, `remote.git` is the
/// repository the first commit is pushed to, and while a file `hold-before-push` is there the
/// push waits (until it goes, or the person calls the creation off), so a spec can press Cancel
/// mid-way. **Every other build reads nothing**: the variable is never looked up.
#[cfg(feature = "e2e")]
mod scenario {
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::time::Duration;

    use purlis_core::createproject::{Author, Hands, OnThisMachine, Plan};
    use purlis_core::forge::recorded::Recorded;
    use purlis_core::forge::{Forge, Kind, MadeRepo};

    /// The variable a scenario run sets to the stand-in forge's directory.
    pub const FORGE: &str = "PURLIS_E2E_FORGE";

    fn dir() -> Option<PathBuf> {
        std::env::var_os(FORGE).map(PathBuf::from)
    }

    /// The stand-in's backend for `recording`, on `kind`'s own host; `None` when the run names
    /// no stand-in. A missing recording is no sign-in, as a machine without one answers.
    pub fn forge(kind: Kind, recording: &str) -> Option<Result<super::OnTheForge, String>> {
        let dir = dir()?;
        let file = dir.join(format!("{recording}.json"));
        let text = std::fs::read_to_string(&file).ok()?;
        Some(Recorded::parse(&text).map(|recorded| {
            let forge = Forge::default_of(kind);
            (forge.host.clone(), forge.backend_over(Arc::new(recorded)))
        }))
    }

    /// The window's own hands, pushing to the stand-in's repository after the hold.
    pub fn hands() -> Option<Box<dyn Hands + Send>> {
        let dir = dir()?;
        Some(Box::new(Held {
            hold: dir.join("hold-before-push"),
            inner: OnThisMachine {
                push_to: Some(dir.join("remote.git")),
            },
        }))
    }

    struct Held {
        hold: PathBuf,
        inner: OnThisMachine,
    }

    impl Hands for Held {
        fn lay_out(&self, folder: &Path, plan: &Plan) -> Result<(), String> {
            self.inner.lay_out(folder, plan)
        }
        fn commit(&self, folder: &Path, plan: &Plan, author: &Author) -> Result<(), String> {
            self.inner.commit(folder, plan, author)
        }
        fn push(&self, folder: &Path, plan: &Plan, made: &MadeRepo) -> Result<(), String> {
            while self.hold.exists() {
                if super::called_off() {
                    return Err("called off while held".into());
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            self.inner.push(folder, plan, made)
        }
    }
}

/// Whether the running creation was called off: what the scenario build's held push listens for.
#[cfg(feature = "e2e")]
fn called_off() -> bool {
    RUNNING
        .lock()
        .ok()
        .and_then(|running| running.as_ref().map(|off| off.load(Ordering::SeqCst)))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_is_its_folder_under_the_place_picked() {
        let folder = new_project_folder("garden".into(), Some("/Volumes/work".into())).unwrap();
        assert_eq!(folder, "/Volumes/work/garden");
        assert!(new_project_folder("not a name".into(), None).is_err());
    }

    #[test]
    fn every_stage_is_listed_in_order_on_each_forge() {
        let words: Vec<String> = Stage::ALL.iter().map(|s| s.words(Kind::GitLab)).collect();
        assert_eq!(words.len(), 5);
        assert_eq!(words[3], "Creating the repo on GitLab");
    }

    #[test]
    fn a_cancel_with_nothing_running_does_nothing() {
        cancel_create_project();
        assert!(RUNNING.lock().unwrap().is_none());
    }
}
