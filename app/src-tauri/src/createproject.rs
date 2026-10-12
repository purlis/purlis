// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The welcome's "Create a project" on GitHub or GitLab (#1819): the window's door to
//! `purlis_core::createproject`; and **Publish** (#1820) of a project that lives only on this
//! machine, `purlis_core::publishproject`, with the same form, the same stages' progress and the
//! same Cancel.
//!
//! Six commands, all of them the window's alone (`WINDOW_ONLY`, and a channel for the creation
//! and the publish themselves): the owners the person's account can make a repo under, the
//! folder a name goes into, the creation with its stages, whether a project lives only here, the
//! publish with its stages, and the person's Cancel. The creation answers the folder it made;
//! the window then opens it through the same open-and-trust question as every other open.
//!
//! **Only with a purlis sign-in.** The forge is asked through the native client as the person in
//! the window, on their signed-in account, and the first commit is pushed through purlis's own
//! credential helper (#752). A forge with no account answers "sign in first", which the guided
//! steps turn into their sign-in step; nothing falls back to `gh` or `glab`. Every act claims
//! [`HostScope`], which a process a chat started is refused.
//!
//! One creation or publish runs at a time, and Cancel (`cancel_create_project`) calls off
//! whichever runs.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use purlis_core::createproject::{self, Created, Plan, Stage};
use purlis_core::forge::accounts::{self, ForgeAccount, State};
use purlis_core::forge::route::HostScope;
use purlis_core::forge::{Caller, Forge, ForgeBackend, Kind, OwnerKind};
use tauri::ipc::Channel;

use crate::firstrun::ForgeWord;
use crate::planes::{PlaneId, Planes};

/// The running creation's stop, while one runs.
static RUNNING: Mutex<Option<Arc<AtomicBool>>> = Mutex::new(None);

/// What a creation was asked for: the forge, its host, the owner and the name.
type Asked = (Kind, String, String, String);

/// **The repo an earlier creation made and the forge would not delete**, kept empty (dispatcher
/// ruling, 2026-10-12: purlis does not ask GitHub for `delete_repo`), with the ask it was made
/// for. A Retry of that same ask pushes into it instead of making another. It is the core's own
/// answer, held in this process; the window never names a repo, so it cannot point a Retry
/// anywhere else.
static KEPT: Mutex<Option<(Asked, purlis_core::forge::MadeRepo)>> = Mutex::new(None);

/// The kept repo for `asked`, when the last creation kept one for that same ask.
fn kept_for(asked: &Asked) -> Option<purlis_core::forge::MadeRepo> {
    let kept = KEPT.lock().ok()?;
    kept.as_ref()
        .filter(|(for_ask, _)| for_ask == asked)
        .map(|(_, repo)| repo.clone())
}

/// Remembers what this creation for `asked` kept, or forgets an earlier one once it is used.
fn keep(asked: Asked, repo: Option<&purlis_core::forge::MadeRepo>) {
    if let Ok(mut kept) = KEPT.lock() {
        *kept = repo.map(|repo| (asked, repo.clone()));
    }
}

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
    let off = start_running()?;
    let stopped = Arc::clone(&off);
    let answer = tauri::async_runtime::spawn_blocking(move || {
        let Some((host, backend)) = forge_for(kind, "create")? else {
            return Err(format!(
                "purlis is not signed in to {}. Sign in, then Retry.",
                kind.display()
            ));
        };
        let asked: Asked = (
            kind,
            host.clone(),
            ask.owner.trim().to_string(),
            ask.name.trim().to_string(),
        );
        let plan = Plan {
            kind,
            host,
            owner: ask.owner.trim().to_string(),
            name: ask.name.trim().to_string(),
            private: ask.private,
            template: ask.template.filter(|id| !id.is_empty()),
            folder: PathBuf::from(&ask.folder),
            kept: kept_for(&asked),
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
        match &made {
            Ok(Created::Made { .. }) => keep(asked, None),
            Ok(Created::CalledOff { kept, .. }) => keep(asked, kept.as_ref()),
            Err(refused) => keep(asked, refused.kept.as_deref()),
        }
        Ok(made)
    })
    .await;
    if let Ok(mut running) = RUNNING.lock() {
        *running = None;
    }
    match answer.map_err(|err| format!("purlis could not create the project: {err}"))?? {
        Ok(Created::Made { folder, .. }) => Ok(Some(folder.display().to_string())),
        Ok(Created::CalledOff { left, .. }) if left.is_empty() => Ok(None),
        Ok(Created::CalledOff { left, .. }) => Err(format!("Cancelled. {}", left.join(" "))),
        Err(refused) => Err(refused.sentence),
    }
}

/// Marks a creation or a publish as running, so a Cancel reaches it; refused while one runs.
fn start_running() -> Result<Arc<AtomicBool>, String> {
    let off = Arc::new(AtomicBool::new(false));
    let mut running = RUNNING
        .lock()
        .map_err(|_| "the creation's state is unreadable")?;
    if running.is_some() {
        return Err(
            "A project is being created or published already. Wait for it, or cancel it.".into(),
        );
    }
    *running = Some(Arc::clone(&off));
    Ok(off)
}

/// What the person asked to publish: #1819's form, less the template and the folder, which the
/// project already has.
#[derive(Debug, Clone, serde::Deserialize, specta::Type)]
pub struct PublishOnForge {
    pub forge: ForgeWord,
    pub owner: String,
    pub name: String,
    /// Private unless the person said public.
    pub private: bool,
}

/// Whether the project lives only on this machine: its folder is no repository yet, or one with
/// no remote. The window says so on the project's tab and in its settings, with Publish beside
/// it (#1820). Answers the name Publish suggests for its repo when it does, and null when the
/// project is somewhere else too.
#[tauri::command]
#[specta::specta]
pub async fn project_lives_here(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<Option<String>, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    crate::off_the_window("reading where the project lives", move || {
        Ok(purlis_core::publishproject::lives_only_here(&root)
            .then(|| purlis_core::publishproject::suggested_name(&root)))
    })
    .await
}

/// Publishes the project that lives only on this machine to the forge `ask` names (#1820),
/// telling `progress` as each stage starts. Answers the repo's address once it is pushed;
/// `null` when the person called it off and nothing was left; or the sentence that says why it
/// failed and what was taken back. A repo the forge would not delete is kept for a Retry of the
/// same ask, as a creation's is.
#[tauri::command]
#[specta::specta]
pub async fn publish_project(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    ask: PublishOnForge,
    progress: Channel<CreateProgress>,
) -> Result<Option<String>, String> {
    let _scope = host_scope()?;
    let root = planes.held(&plane)?.root().to_path_buf();
    let kind = Kind::from(ask.forge);
    let off = start_running()?;
    let stopped = Arc::clone(&off);
    let answer = tauri::async_runtime::spawn_blocking(move || {
        let Some((host, backend)) = forge_for(kind, "publish")? else {
            return Err(format!(
                "purlis is not signed in to {}. Sign in in {}, then Retry.",
                kind.display(),
                purlis_core::forge::signin::SIGN_IN_AT
            ));
        };
        let asked: Asked = (
            kind,
            host.clone(),
            ask.owner.trim().to_string(),
            ask.name.trim().to_string(),
        );
        let plan = Plan {
            kind,
            host,
            owner: ask.owner.trim().to_string(),
            name: ask.name.trim().to_string(),
            private: ask.private,
            template: None,
            folder: root,
            kept: kept_for(&asked),
        };
        let stages: Vec<String> = purlis_core::publishproject::STAGES
            .iter()
            .map(|s| purlis_core::publishproject::words(*s, kind))
            .collect();
        let hands = publish_hands();
        // The person's own publish, run by the window: their purlis account for its host
        // (#752), and no other credential helper.
        let made = crate::forgeaccounts::window_git(|| {
            purlis_core::publishproject::publish(
                &plan,
                backend.as_ref(),
                &Caller::window(),
                hands.as_ref(),
                &stopped,
                &mut |stage| {
                    let at = purlis_core::publishproject::STAGES
                        .iter()
                        .position(|s| *s == stage)
                        .unwrap_or(0);
                    let _ = progress.send(CreateProgress {
                        stage: purlis_core::publishproject::words(stage, kind),
                        at: u32::try_from(at).unwrap_or(0),
                        stages: stages.clone(),
                    });
                },
            )
        });
        match &made {
            Ok(Created::Made { .. }) => keep(asked, None),
            Ok(Created::CalledOff { kept, .. }) => keep(asked, kept.as_ref()),
            Err(refused) => keep(asked, refused.kept.as_deref()),
        }
        Ok(made)
    })
    .await;
    if let Ok(mut running) = RUNNING.lock() {
        *running = None;
    }
    match answer.map_err(|err| format!("purlis could not publish the project: {err}"))?? {
        Ok(Created::Made { repo, .. }) => Ok(Some(repo.web_url().to_string())),
        Ok(Created::CalledOff { left, .. }) if left.is_empty() => Ok(None),
        Ok(Created::CalledOff { left, .. }) => Err(format!("Cancelled. {}", left.join(" "))),
        Err(refused) => Err(refused.sentence),
    }
}

/// The person's Cancel: the running creation or publish stops at the end of the stage it is in,
/// then takes back what it made. Nothing happens when none runs.
#[tauri::command]
#[specta::specta]
pub fn cancel_create_project() {
    if let Ok(running) = RUNNING.lock()
        && let Some(off) = running.as_ref()
    {
        off.store(true, Ordering::SeqCst);
    }
}

/// The work on this machine for a publish: the window's own, or in the scenario build the
/// stand-in's.
fn publish_hands() -> Box<dyn purlis_core::publishproject::Hands + Send> {
    #[cfg(feature = "e2e")]
    if let Some(stand_in) = scenario::publish_hands() {
        return stand_in;
    }
    Box::new(purlis_core::publishproject::OnThisMachine::for_the_window())
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
/// mid-way. A publish (#1820) answers from `publish.json` and pushes to the same repository.
/// **Every other build reads nothing**: the variable is never looked up.
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

    /// The window's own publishing hands, pushing to the stand-in's repository after the hold
    /// (#1820): `publish.json` answers the forge's side.
    pub fn publish_hands() -> Option<Box<dyn purlis_core::publishproject::Hands + Send>> {
        let dir = dir()?;
        Some(Box::new(HeldPublish {
            hold: dir.join("hold-before-push"),
            inner: purlis_core::publishproject::OnThisMachine {
                push_to: Some(dir.join("remote.git")),
            },
        }))
    }

    struct HeldPublish {
        hold: PathBuf,
        inner: purlis_core::publishproject::OnThisMachine,
    }

    impl purlis_core::publishproject::Hands for HeldPublish {
        fn commit(&self, root: &Path, plan: &Plan, author: &Author) -> Result<bool, String> {
            self.inner.commit(root, plan, author)
        }
        fn push(&self, root: &Path, plan: &Plan, made: &MadeRepo) -> Result<(), String> {
            while self.hold.exists() {
                if super::called_off() {
                    return Err("called off while held".into());
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            self.inner.push(root, plan, made)
        }
        fn take_back(
            &self,
            root: &Path,
            made_repo: bool,
            named: Option<&MadeRepo>,
        ) -> Result<(), String> {
            self.inner.take_back(root, made_repo, named)
        }
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
