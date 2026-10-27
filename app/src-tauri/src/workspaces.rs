// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Making a workspace and deleting one, as the window can reach them.
//!
//! Thin by design, exactly like `worktrees.rs`: what a workspace may be called, what it is
//! scaffolded with, and **what may be deleted** all live in `purlis_core::wscmd`, and this
//! layer converts. A rule implemented here as well would be a second rule, and the two would
//! drift — which for [`workspace_remove`] means a recursive delete deciding for itself what is
//! safe to destroy.
//!
//! # The guard, and why the window cannot be past it
//!
//! `charter workspace remove` refuses when the workspace holds work that removing it would
//! discard: a clone charter could not read, a dirty tree, unpushed commits, a worktree holding
//! commits reachable from no other ref. That guard is [`purlis_core::wscmd::work_at_risk`] and
//! it runs **inside `wscmd::remove`**, between the name check and `remove_dir_all`.
//!
//! So [`workspace_remove`] calls `wscmd::remove` and nothing else. It does not call
//! `remove_dir_all`, it does not read `work_at_risk` and then decide, and it does not have a
//! path that deletes a directory. There is no way to reach a delete in this module that has not
//! been through the guard, because there is only one delete and the guard is in front of it.
//! `force` is the operator's own second answer to a refusal they have read, and it is passed
//! only from a click on a button that named what would be lost.
//!
//! [`workspace_at_risk`] reads the same guard for the dialog to draw, and it is **advisory
//! only**: it decides nothing. The dialog shows it so that the operator knows before pressing
//! what charter is about to say; the delete asks again, in the core, at the moment it matters.
//! A preview that decided would be a check made against a disk that can change between the two.
//!
//! # The words on the button, which are not the decision (charter-app#182)
//!
//! Those two reads happen at two moments, and a workspace can move between them — a clone goes
//! dirty, a commit is made, a worktree appears. Nothing is then wrongly deleted, because the
//! core's own read still governs. What went wrong is smaller and still bad: the button that
//! offers to discard work drew its words from the **preview**, so it could name one clone while
//! the verbatim refusal printed above it named another, at the moment somebody is deciding
//! whether to throw work away.
//!
//! So [`workspace_remove`] refuses with a [`Refused`], which carries the core's sentence **and
//! the list that sentence was made from** — `wscmd::remove::Removal::refused_over`, read inside
//! the delete. The window draws the refusal's own list once there is one, and the preview only
//! before the first press.
//!
//! **This changes nothing about what decides.** The list is an output of the read that already
//! happened; it is not read here, not compared here, and no code in this module branches on it.
//!
//! **A refusal crosses unchanged**, for `worktrees.rs`' reason: the core's sentence names the
//! repair, and an operator shown a reworded version of it can neither follow it nor search for
//! it.

use std::path::Path;

use purlis_core::extension::events::Event;
use purlis_core::repocmd::Say;
use purlis_core::wscmd;

use crate::planes::{PlaneId, Planes};

// What each command was doing, for the sentence a blocking thread that did not finish is
// answered with (`crate::off_the_window`).
const CREATING: &str = "creating the workspace";
const READING_RISK: &str = "reading what the workspace holds";
const REMOVING: &str = "removing the workspace";
const RECORDING_VISION: &str = "recording the workspace's vision";

/// One reason a workspace holds work that deleting it would discard.
///
/// A mirror of [`wscmd::AtRisk`] rather than the thing itself, because `purlis-core` never
/// depends on the app and the app's wire types are generated into TypeScript.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct AtRisk {
    /// What to look at: a clone's name, or `<repo>/<piece>` for a worktree.
    pub what: String,
    /// charter's own sentence about it, name included: `svc: 2 unpushed commit(s)`.
    pub said: String,
}

impl From<wscmd::AtRisk> for AtRisk {
    fn from(risk: wscmd::AtRisk) -> Self {
        Self {
            what: risk.what,
            said: risk.said,
        }
    }
}

/// Why a delete made nothing — **and the reading it was refused on** (charter-app#182).
///
/// Two fields and they are one answer. `said` is the core's sentence, verbatim, because it
/// names the repair. `at_risk` is `work_at_risk`'s list as the core read it *inside* the
/// delete, so the surface offering to discard that work names the same things the sentence
/// above it names. Empty for every refusal that is not the guard's — a name that is not a
/// workspace, a `workspaces/<ws>` that links out of the plane, a `remove_dir_all` that failed
/// — and that is the honest shape: `--force` does not get past any of those, so a window that
/// drew a force button beside one would be offering a way through that does not exist.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct Refused {
    /// The core's own words, unchanged and all of them.
    pub said: String,
    /// What the core refused on, in the order it said them.
    pub at_risk: Vec<AtRisk>,
}

/// What a workspace command said, turned into what a window draws.
///
/// **Exit 0 keeps every line, marks and all.** A removal can succeed and still have something
/// to say — charter#870's exclude block, a count of todos discarded — and those lines are the
/// only place the operator is told. Dropping everything but the `✓` would lose exactly the
/// news that is worth reading.
///
/// **A non-zero exit is a refusal, in the core's words and nothing else.** The `✗` lines are
/// the sentence; the mark is left off because a window draws a refusal as a refusal and would
/// otherwise say so twice. A command that failed with nothing marked hands back everything it
/// said rather than an empty refusal, because a refusal with no words is not one.
pub(crate) fn ran(code: u8, said: Vec<Say>) -> Result<Vec<String>, String> {
    if code == 0 {
        return Ok(said.iter().map(ToString::to_string).collect());
    }
    let refusals: Vec<String> = said
        .iter()
        .filter_map(|line| match line {
            Say::Fail(text) | Say::Plain(text) => Some(text.to_string()),
            _ => None,
        })
        .collect();
    Err(if refusals.is_empty() {
        said.iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        refusals.join("\n")
    })
}

/// Make a workspace: `charter workspace create <name>`, with the vision when one was typed.
///
/// **The name is checked by the core and by nothing in the window.** `wscmd::create` runs
/// `wscmd::ensure`, which is where `contain::workspace_name_ok` is, so the app refuses exactly
/// the names a terminal refuses and says the same sentence about them. A second alphabet in
/// the dialog would be a second answer to what a workspace may be called, and the two would
/// drift the first time either moved.
///
/// LOCAL unless the dialog's Live box was ticked (charter-app#301): a LIVE workspace's manifest
/// and memory are published with the plane, so that is the operator's choice, never a default,
/// and a workspace born LIVE is saved at once, as switching one to LIVE is. It selects nothing:
/// `--use` writes a session lock that belongs to a terminal.
// Its plane is a `PlaneId` the registry vouches for, like every other command's
// (charter-app#127). Not a doc comment, because the generated bindings carry those.
#[tauri::command]
#[specta::specta]
pub async fn workspace_create(
    app: tauri::AppHandle,
    planes: tauri::State<'_, Planes>,
    heard: tauri::State<'_, crate::heard::Heard>,
    plane: PlaneId,
    name: String,
    vision: Option<String>,
    live: bool,
) -> Result<Vec<String>, String> {
    let held = planes.held(&plane)?;
    let root = held.root().to_path_buf();
    let (at, named) = (root.clone(), name.clone());
    // Off the window's thread (#1007): the scaffold is written, a LIVE one is saved with git,
    // and the sidebar's model is read again under its lock.
    let said = crate::off_the_window(CREATING, move || {
        let made = create_in(&at, &named, vision.as_deref(), live);
        // The window reads the sidebar straight after, made or not (FD-10b).
        held.workspaces_moved();
        let mut said = made?;
        if live && let Some(not_saved) = crate::live::save_after(&at, &mut said) {
            said.push(not_saved);
        }
        Ok(said)
    })
    .await?;
    // Told once it is made, on a thread of its own: nothing an extension answers can change
    // what this answers (charter-app#343).
    heard.tell(
        &app,
        plane,
        root,
        Event::WorkspaceCreated { workspace: name },
    );
    Ok(said)
}

/// **Record a workspace's vision**: what `purlis workspace vision "<the goal>"` writes, the
/// `## Vision` of its `workspace.md`, from the window's Add a vision (#3344). The core's own
/// brokered write decides everything, as it does for a chat's `workspace_vision`: an empty
/// vision, a line that would start a section of its own, a workspace that is not there, a file
/// over its budget. Its refusal is the core's sentence. It answers with the sentence the window
/// says.
// Its plane is a `PlaneId` the registry vouches for; see `workspace_create`.
#[tauri::command]
#[specta::specta]
pub async fn workspace_vision(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: String,
    vision: String,
) -> Result<String, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    // Off the window's thread (#1007, #1949): it reads and writes the workspace's file.
    crate::off_the_window(RECORDING_VISION, move || {
        vision_in(&root, &workspace, &vision)
    })
    .await
}

/// The vision written, against a root the registry has already vouched for.
fn vision_in(root: &Path, workspace: &str, vision: &str) -> Result<String, String> {
    let place = purlis_core::active::Place::Workspace(workspace.to_owned());
    let written = purlis_core::brokered::set_vision(root, &place, "this window", vision)?;
    Ok(format!("{} has a vision now.", written.to))
}

/// A workspace for another module's test, as the window would make it: LOCAL.
#[cfg(test)]
pub(crate) fn create_for_tests(root: &Path, name: &str) {
    create_in(root, name, None, false).expect("a workspace for the test");
}

/// The operator brought a workspace to the front: tell the extensions that hear it
/// (charter-app#343). It does nothing else and answers nothing — focusing is the window's own
/// state, and this is only the report of it.
// Its plane is a `PlaneId` the registry vouches for; see `workspace_create`.
#[tauri::command]
#[specta::specta]
pub fn workspace_focused(
    app: tauri::AppHandle,
    planes: tauri::State<'_, Planes>,
    heard: tauri::State<'_, crate::heard::Heard>,
    plane: PlaneId,
    workspace: String,
) -> Result<(), String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    if !purlis_core::contain::workspace_name_ok(&workspace) {
        return Err(format!("'{workspace}' is not a workspace's name"));
    }
    heard.tell(&app, plane, root, Event::WorkspaceFocused { workspace });
    Ok(())
}

/// The creation itself, against a root the registry has already vouched for.
fn create_in(
    root: &Path,
    name: &str,
    vision: Option<&str>,
    live: bool,
) -> Result<Vec<String>, String> {
    // An empty box is no vision, not a vision that is empty: `set_vision` would otherwise write
    // an empty `## Vision` section into the workspace's charter and charter would read it back
    // as one that had been recorded.
    let vision = vision.map(str::trim).filter(|text| !text.is_empty());
    let ids = purlis_core::active::Ids::default();
    let mut said = Vec::new();
    let owner = format!("workspaces/{name}");
    let was_there = purlis_core::skills::levels::there(root, &owner);
    let code = wscmd::create::create(
        &wscmd::create::Request {
            root,
            name,
            vision,
            live,
            use_it: false,
            force: false,
            repos: &[],
            hosts: None,
            now: chrono::Utc::now(),
            ids: &ids,
        },
        &mut |line: Say| said.push(line),
    );
    // The person made it, in their window: no chat did, so its skills reach chats (SK-9), where
    // the create made its folder.
    if code == 0
        && let Some(note) = purlis_core::skills::levels::vouch_if_made(root, &owner, was_there)
    {
        said.push(Say::Info(note.into()));
    }
    ran(code, said)
}

/// What deleting this workspace would discard, for the dialog to show **before** anything is
/// pressed.
///
/// The core's own guard, read for drawing. It decides nothing: [`workspace_remove`] asks again,
/// inside `wscmd::remove`, against the disk as it is at the moment of the delete. A window that
/// treated this answer as the decision would be deciding on a reading that is already old — and
/// worse, one taken while the operator read a dialog.
// Its plane is a `PlaneId` the registry vouches for; see `workspace_create`.
#[tauri::command]
#[specta::specta]
pub async fn workspace_at_risk(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: String,
) -> Result<Vec<AtRisk>, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    // Off the window's thread (#1007): it asks git about every clone and worktree.
    crate::off_the_window(READING_RISK, move || Ok(at_risk_in(&root, &workspace))).await
}

/// The reading itself, against a root the registry has already vouched for.
fn at_risk_in(root: &Path, workspace: &str) -> Vec<AtRisk> {
    wscmd::work_at_risk(root, workspace)
        .into_iter()
        .map(AtRisk::from)
        .collect()
}

/// Delete a workspace and its clones: `charter workspace remove <name> [--force]`.
///
/// **This is the one delete, and the guard is inside it.** See this module's header. `force` is
/// the operator saying to discard work the core found — it is never passed on their behalf, and
/// the window asks for it only after showing them the refusal the core gave.
// Its plane is a `PlaneId` the registry vouches for; see `workspace_create`.
#[tauri::command]
#[specta::specta]
pub async fn workspace_remove(
    app: tauri::AppHandle,
    planes: tauri::State<'_, Planes>,
    heard: tauri::State<'_, crate::heard::Heard>,
    plane: PlaneId,
    workspace: String,
    force: bool,
) -> Result<Vec<String>, Refused> {
    let unread = |said: String| Refused {
        said,
        at_risk: Vec::new(),
    };
    let held = planes.held(&plane).map_err(unread)?;
    remove_for_the_person(&app, &held, &heard, plane, workspace, force).await
}

/// **The window's one removal of a workspace**, behind [`workspace_remove`]: the Delete
/// workspace dialog, opened from the sidebar or by Remove… on a question a chat raised (qw356).
/// The guard is inside it, and a chat that asked the person to remove the workspace is told
/// here that they did.
pub(crate) async fn remove_for_the_person(
    app: &tauri::AppHandle,
    held: &std::sync::Arc<crate::planes::Held>,
    heard: &crate::heard::Heard,
    plane: PlaneId,
    workspace: String,
    force: bool,
) -> Result<Vec<String>, Refused> {
    let unread = |said: String| Refused {
        said,
        at_risk: Vec::new(),
    };
    let root = held.root().to_path_buf();
    let (at, named, held) = (root.clone(), workspace.clone(), std::sync::Arc::clone(held));
    // Off the window's thread (#1007): the guard asks git about every clone, the delete walks
    // the tree, and the sidebar's model is read again under its lock.
    let removed = crate::off_the_window(REMOVING, move || {
        let removed = remove_in(&at, &named, force);
        // The window reads the sidebar straight after (FD-10b).
        held.workspaces_moved();
        if removed.is_ok() {
            // An open Activity tab stops showing what the removal forgot (#1556).
            crate::activity::forgotten_in(&held, &named);
            // A chat that asked the person to remove it is told they did (qw356).
            crate::removalasks::removed(&held, &named);
        }
        Ok(removed)
    })
    .await
    .map_err(unread)??;
    heard.tell(app, plane, root, Event::WorkspaceRemoved { workspace });
    Ok(removed)
}

/// The removal itself, against a root the registry has already vouched for.
///
/// Still one call, deliberately: everything this command is, is `wscmd::remove`. What it
/// answers with now carries `refused_over` — the list the core made its refusal from — and
/// that list is copied across the wire and read by nothing on this side (charter-app#182).
fn remove_in(root: &Path, workspace: &str, force: bool) -> Result<Vec<String>, Refused> {
    let mut said = Vec::new();
    let done = wscmd::remove::remove(root, workspace, force, &mut |line: Say| said.push(line));
    // `ran` is untouched and shared with `workspace_create`: which lines are a refusal, and
    // whether there was one at all, is still decided in exactly one place.
    ran(done.code, said).map_err(|said| Refused {
        said,
        at_risk: done.refused_over.into_iter().map(AtRisk::from).collect(),
    })
}

/// Rename a workspace: `charter workspace rename <workspace> <name>` (charter#367).
///
/// **The core decides and does everything**: which names are refused, the move, the worktree
/// repair, every record that names the workspace — the app's record on disk and this machine's
/// pins included — and the plane save a LIVE workspace's move takes. This layer adds the one
/// thing only the window knows, which chats are running in it: a running chat refuses the
/// rename, named as its tab names it. Once renamed, what the window holds in memory follows
/// ([`crate::chats::Chats::follow`]), or its next write of the record would put the old name
/// back.
// Its plane is a `PlaneId` the registry vouches for; see `workspace_create`.
#[tauri::command]
#[specta::specta]
pub async fn workspace_rename(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: String,
    name: String,
) -> Result<Vec<String>, String> {
    let held = planes.held(&plane)?;
    let config = planes.config().map(Path::to_path_buf);
    tauri::async_runtime::spawn_blocking(move || {
        let renamed = rename_in(&held, config.as_deref(), &workspace, &name);
        // The window reads the sidebar straight after (FD-10b).
        held.workspaces_moved();
        renamed
    })
    .await
    .map_err(|err| format!("the rename did not finish: {err}"))?
}

/// The core's sentence naming the chats that will start a fresh conversation if `workspace` is
/// renamed, or none — what the Rename dialog says before it is answered (charter#367, D10).
///
/// Asked of what the window holds, which is what the record is written from: a Claude Code
/// chat in the workspace with a conversation to resume. A chat that is running is left out: it
/// refuses the rename instead, and is named then. The rename itself still goes ahead.
// Its plane is a `PlaneId` the registry vouches for; see `workspace_create`.
#[tauri::command]
#[specta::specta]
pub async fn workspace_starts_fresh(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: String,
) -> Result<Option<String>, String> {
    let held = planes.held(&plane)?;
    tauri::async_runtime::spawn_blocking(move || starts_fresh_in(&held, &workspace))
        .await
        .map_err(|err| format!("could not ask which chats start fresh: {err}"))
}

/// [`workspace_starts_fresh`], against a plane the registry holds.
pub(crate) fn starts_fresh_in(held: &crate::planes::Held, workspace: &str) -> Option<String> {
    let chats = held.chats();
    let running = chats.sessions().running();
    let mut record = chats.record();
    record
        .chats
        .retain(|chat| chat.number.is_none_or(|number| !running.contains(&number)));
    wscmd::rename::fresh_warning(&wscmd::rename::starts_fresh(
        held.root(),
        workspace,
        &record,
    ))
}

/// The rename itself, against a plane the registry holds.
pub(crate) fn rename_in(
    held: &crate::planes::Held,
    config_root: Option<&Path>,
    workspace: &str,
    name: &str,
) -> Result<Vec<String>, String> {
    let root = held.root().to_path_buf();
    let chats = held.chats();
    let on_disk = purlis_core::workspaces::Plane::open(&root);
    // Under either name, so a rename finished after a crash is guarded as well.
    let running: Vec<String> = chats
        .open_now()
        .into_iter()
        .filter(|one| {
            one.cwd
                .as_deref()
                .and_then(|cwd| on_disk.workspace_of(cwd))
                .is_some_and(|ws| ws == workspace || ws == name)
        })
        .filter_map(|one| chats.shown_name(one.session))
        .collect();
    let mut said = Vec::new();
    let code = wscmd::rename::rename(
        &wscmd::rename::Request {
            root: &root,
            old: workspace,
            new: name,
            running: &running,
            config_root,
        },
        &mut |line: Say| said.push(line),
    );
    if code == 0 {
        chats.follow(&wscmd::rename::Move::in_plane(&root, workspace, name));
    }
    ran(code, said)
}

/// One repo the picker offers: what the operator reads to choose it.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct ReachableRepo {
    /// The name it is cloned under, and asked for by.
    pub name: String,
    /// `owner/name` on its forge.
    pub path: String,
    pub description: String,
}

/// What the picker draws: the repos this operator's forge logins reach, and a sentence for
/// each forge that did not answer.
#[derive(Debug, Clone, Default, serde::Serialize, specta::Type)]
pub struct ReachableRepos {
    pub repos: Vec<ReachableRepo>,
    pub trouble: Vec<ForgeTrouble>,
}

/// A forge that did not answer the picker: its sentence, and the login that would cure it.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct ForgeTrouble {
    pub said: String,
    /// `<cli> auth login --hostname <host>` where the forge's CLI said it is not logged in
    /// there, which the picker offers to type in a shell tab (NO-8). Null where logging in
    /// would not help.
    pub login: Option<String>,
}

/// The repos the operator's own forge login reaches, asked now and held nowhere (ADR 0055).
///
/// On a blocking thread: it is one forge call per page and per host, each with the forge
/// CLI's own deadline.
// Its plane is a `PlaneId` the registry vouches for; see `workspace_create`.
#[tauri::command]
#[specta::specta]
pub async fn reachable_repos(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<ReachableRepos, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    tauri::async_runtime::spawn_blocking(move || reachable_in(&root))
        .await
        .map_err(|err| format!("the forge listing did not finish: {err}"))?
}

fn reachable_in(root: &Path) -> Result<ReachableRepos, String> {
    let found = purlis_core::repocmd::reachable::reachable(root)?;
    let text = |r: &serde_json::Value, key: &str| {
        r.get(key)
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    Ok(ReachableRepos {
        repos: found
            .repos
            .iter()
            .map(|r| ReachableRepo {
                name: text(r, "name"),
                path: text(r, "path_with_namespace"),
                description: text(r, "description"),
            })
            .collect(),
        trouble: found
            .trouble
            .into_iter()
            .map(|t| ForgeTrouble {
                said: t.said,
                login: t.login,
            })
            .collect(),
    })
}

/// Add the repos the operator picked to the inventory, beside what it lists, so each can be
/// cloned by name (ADR 0055). Asked once for a whole pick, before the clones.
// Its plane is a `PlaneId` the registry vouches for; see `workspace_create`.
#[tauri::command]
#[specta::specta]
pub async fn take_repos(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    repos: Vec<String>,
) -> Result<(), String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        purlis_core::repocmd::reachable::take(&root, &repos)
    })
    .await
    .map_err(|err| format!("the inventory was not updated: {err}"))?
}

/// Clone ONE repo into a workspace: `charter clone <repo> -w <workspace>`.
///
/// One per call, and the window calls it once per repo in turn: that is what lets it show each
/// repo's own state as it lands and retry one that failed, and no two calls race to write the
/// workspace's manifest. On a blocking thread, because a clone can take the network's two
/// minutes.
// Its plane is a `PlaneId` the registry vouches for; see `workspace_create`.
#[tauri::command]
#[specta::specta]
pub async fn clone_repo(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: String,
    repo: String,
) -> Result<Vec<String>, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    tauri::async_runtime::spawn_blocking(move || clone_into(&root, &workspace, &repo))
        .await
        .map_err(|err| format!("the clone did not finish: {err}"))?
}

fn clone_into(root: &Path, workspace: &str, repo: &str) -> Result<Vec<String>, String> {
    let author = wscmd::ensure::author();
    let mut said = Vec::new();
    let code = crate::forgeaccounts::window_git(|| {
        purlis_core::repocmd::clone::clone(
            &purlis_core::repocmd::clone::Request {
                root,
                ws: workspace,
                repos: &[repo.to_string()],
                now: chrono::Utc::now(),
                author: &author,
                hosts: None,
            },
            &mut |line: Say| said.push(line),
        )
    });
    ran(code, said)
}

/// Take one repo out of a workspace, clone and manifest row both (ADR 0055).
///
/// **The guard is inside the delete**, exactly as [`workspace_remove`]'s is: this calls
/// `wscmd::drop::drop_repo` and nothing else, and there is no `force`.
// Its plane is a `PlaneId` the registry vouches for; see `workspace_create`.
#[tauri::command]
#[specta::specta]
pub async fn drop_repo(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: String,
    repo: String,
) -> Result<Vec<String>, Refused> {
    let root = planes
        .held(&plane)
        .map_err(|why| Refused {
            said: why,
            at_risk: Vec::new(),
        })?
        .root()
        .to_path_buf();
    tauri::async_runtime::spawn_blocking(move || drop_in(&root, &workspace, &repo))
        .await
        .map_err(|err| Refused {
            said: format!("the removal did not finish: {err}"),
            at_risk: Vec::new(),
        })?
}

fn drop_in(root: &Path, workspace: &str, repo: &str) -> Result<Vec<String>, Refused> {
    let mut said = Vec::new();
    let done = wscmd::drop::drop_repo(root, workspace, repo, &mut |line: Say| said.push(line));
    ran(done.code, said).map_err(|said| Refused {
        said,
        at_risk: done.refused_over.into_iter().map(AtRisk::from).collect(),
    })
}

/// Take a repo the workspace names and this machine has NOT cloned out of the workspace: its
/// row in `workspace.json`, and nothing else (#1228).
///
/// A cloned repo is refused here and goes through [`drop_repo`], whose guard weighs the work
/// in the clone; this calls `wscmd::drop::drop_membership` and nothing else.
// Its plane is a `PlaneId` the registry vouches for; see `workspace_create`.
#[tauri::command]
#[specta::specta]
pub async fn drop_repo_membership(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: String,
    repo: String,
) -> Result<Vec<String>, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    tauri::async_runtime::spawn_blocking(move || membership_dropped(&root, &workspace, &repo))
        .await
        .map_err(|err| format!("the removal did not finish: {err}"))?
}

fn membership_dropped(root: &Path, workspace: &str, repo: &str) -> Result<Vec<String>, String> {
    let mut said = Vec::new();
    let code =
        wscmd::drop::drop_membership(root, workspace, repo, &mut |line: Say| said.push(line));
    ran(code, said)
}

#[cfg(test)]
mod tests;
