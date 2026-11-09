// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A pinned artifact, rendered in its tab** (ADR 0120 §2–3, #5952): a native webview of its
//! own, a child of the window, laid over the tab's frame.
//!
//! - **Its label matches no capability** ([`label`], `artifact-<n>`), and the capabilities name
//!   charter's webviews, not its windows, so a child webview of the main window inherits
//!   nothing: the page holds an empty grant (`ipc.rs` holds both).
//! - **It is served only the bytes read at the press**, from memory, under the CSP on every
//!   response ([`Artifacts::answer`], [`purlis_core::artifact::serve`]). A request is
//!   attributed by the label of the webview it came from, never by anything the page says:
//!   another artifact's origin asked from this webview is not found.
//! - **Its own origin per artifact** ([`origin`]): one per folder of a workspace.
//! - **It navigates only within its own origin** ([`Artifacts::may_navigate`], which the
//!   navigation wall asks for an artifact's webview), and opens no window.
//! - **Esc and F6 hand the keyboard back to purlis** ([`KEYS`]): purlis's own script in the page
//!   asks its own origin for `/.purlis/key/<key>`, which purlis answers by focusing the window's
//!   own webview and telling it which key ([`ARTIFACT_KEY`]). An approved page can forge that
//!   request, and it moves only the focus.
//! - **Its script runs only once the person approved this template here** (ADR 0120 §4,
//!   #5959): the CSP of each response follows the standing read when the folder was
//!   ([`artifact::approval`]). Approval is [`artifact_approve`]: window-only, of exactly the pin
//!   and the fingerprint this webview was served, never what the window says; every window of
//!   the plane hears it ([`APPROVALS_CHANGED`]) and loads its page again.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};

use purlis_core::artifact::approval::{self, Standing};
use purlis_core::artifact::{self, serve};
use serde::{Deserialize, Serialize};
use tauri::http::{Request, Response, StatusCode};
use tauri::{Emitter, Manager, Runtime};

use crate::planes::{PlaneId, Planes};

/// What every artifact webview's label starts with.
const LABEL: &str = "artifact-";

/// The label of the `n`th artifact webview this run made.
pub(crate) fn label(n: u64) -> String {
    format!("{LABEL}{n}")
}

/// Whether `label` is an artifact webview's.
pub(crate) fn is_artifact(label: &str) -> bool {
    label
        .strip_prefix(LABEL)
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// An artifact's origin, as its webview spells it: WebView2 reaches a custom scheme only as
/// `http://<scheme>.<host>` (wry's workaround), so Windows spells it so.
pub(crate) fn origin(host: &str) -> String {
    if cfg!(windows) {
        format!("http://{}.{host}", serve::SCHEME)
    } else {
        format!("{}://{host}", serve::SCHEME)
    }
}

/// The artifact host a request's URL host names.
fn host_of(host: &str) -> &str {
    host.strip_prefix(&format!("{}.", serve::SCHEME))
        .unwrap_or(host)
}

/// What the window hears when the person presses Esc or F6 in an artifact's page: the label
/// of its webview and which key (`escape`, `f6`, `f6-back`).
pub(crate) const ARTIFACT_KEY: &str = "artifact-key";

/// purlis's script in every artifact page: Esc and F6 (Shift+F6 back) are purlis's keys, not
/// the page's. It asks its own origin, which the CSP lets an image do and nothing else.
const KEYS: &str = r#"(() => {
  addEventListener("keydown", (e) => {
    const key = e.key === "Escape" ? "escape" : e.key === "F6" ? (e.shiftKey ? "f6-back" : "f6") : null;
    if (key === null) return;
    e.preventDefault();
    e.stopImmediatePropagation();
    new Image().src = "/.purlis/key/" + key;
  }, true);
})();"#;

/// The keys [`KEYS`] reports.
const KEY_NAMES: [&str; 3] = ["escape", "f6", "f6-back"];

/// What every window of a plane hears when an artifact's approval on this machine changed: the
/// workspace and folder, so each tab of it loads its page again.
pub(crate) const APPROVALS_CHANGED: &str = "artifact-approvals-changed";

/// What [`APPROVALS_CHANGED`] carries.
#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct ArtifactApprovalChanged {
    pub workspace: String,
    pub folder: String,
}

/// One artifact webview: whose window it is in, its origin's host, what it is served, and
/// which pin of which folder that is, standing how.
struct Served {
    window: String,
    host: String,
    snapshot: serve::Snapshot,
    of: Of,
}

/// The artifact a webview shows, as it was read when it was opened.
#[derive(Clone)]
struct Of {
    plane: PlaneId,
    workspace: String,
    dir: std::path::PathBuf,
    folder: String,
    pin: String,
    standing: Standing,
}

/// Every artifact webview this run has open, by label.
#[derive(Default)]
pub(crate) struct Artifacts {
    served: Mutex<HashMap<String, Served>>,
    made: AtomicU64,
}

/// What the scheme answered, and the key to hand back to a window, if it was one.
pub(crate) struct Answered {
    pub(crate) response: Response<Vec<u8>>,
    /// `(window, key)`: the page pressed one of purlis's keys.
    pub(crate) key: Option<(String, String)>,
}

impl Artifacts {
    fn served(&self) -> std::sync::MutexGuard<'_, HashMap<String, Served>> {
        self.served.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// **The artifact scheme's answer to webview `label`'s request**: the bytes read for the
    /// key it names, from its own origin only, under the CSP; or not found.
    pub(crate) fn answer(&self, label: &str, request: &Request<Vec<u8>>) -> Answered {
        let uri = request.uri();
        let host = host_of(uri.host().unwrap_or("")).to_owned();
        let path = uri.path();
        let served = self.served();
        let own = served.get(label).filter(|one| one.host == host);
        let mut key = None;
        let runs = own.is_some_and(|one| one.of.standing.runs());
        let (status, body, kind) = match own {
            None => (StatusCode::NOT_FOUND, Vec::new(), "text/plain"),
            Some(one) => {
                if let Some(rest) = path.strip_prefix(serve::OWN_PATH) {
                    let pressed = rest.strip_prefix("key/").filter(|k| KEY_NAMES.contains(k));
                    key = pressed.map(|k| (one.window.clone(), k.to_owned()));
                    (StatusCode::NO_CONTENT, Vec::new(), "text/plain")
                } else {
                    match serve::key(path)
                        .and_then(|key| Some((one.snapshot.get(&key)?.to_vec(), key)))
                    {
                        Some((bytes, key)) => (StatusCode::OK, bytes, serve::content_type(&key)),
                        None => (StatusCode::NOT_FOUND, Vec::new(), "text/plain"),
                    }
                }
            }
        };
        let response = Response::builder()
            .status(status)
            .header("Content-Type", kind)
            .header("Content-Security-Policy", serve::csp(&origin(&host), runs))
            .header("X-Content-Type-Options", "nosniff")
            .header("Cache-Control", "no-store")
            .header("Referrer-Policy", "no-referrer")
            .body(body)
            .unwrap_or_else(|_| Response::new(Vec::new()));
        Answered { response, key }
    }

    /// Whether artifact webview `label` may go to `url`: within its own origin only.
    pub(crate) fn may_navigate(&self, label: &str, url: &tauri::Url) -> bool {
        self.served()
            .get(label)
            .is_some_and(|one| url.as_str().starts_with(&format!("{}/", origin(&one.host))))
    }

    /// The artifact webview `label`, when it is in window `window`.
    fn owned_by(&self, label: &str, window: &str) -> bool {
        self.served()
            .get(label)
            .is_some_and(|one| one.window == window)
    }

    /// What artifact webview `label` of window `window` was served: the artifact, and the
    /// fingerprint of the template it was served.
    fn served_to(&self, label: &str, window: &str) -> Option<(Of, String)> {
        self.served()
            .get(label)
            .filter(|one| one.window == window)
            .map(|one| (one.of.clone(), one.snapshot.fingerprint().to_owned()))
    }

    /// Forgets every artifact webview of window `window`, answering their labels.
    pub(crate) fn forget_window(&self, window: &str) -> Vec<String> {
        let mut served = self.served();
        let gone: Vec<String> = served
            .iter()
            .filter(|(_, one)| one.window == window)
            .map(|(label, _)| label.clone())
            .collect();
        for label in &gone {
            served.remove(label);
        }
        gone
    }
}

/// The scheme's handler: answers, and hands a pressed key back to the page's window.
pub(crate) fn serve<R: Runtime>(
    app: &tauri::AppHandle<R>,
    label: &str,
    request: &Request<Vec<u8>>,
) -> Response<Vec<u8>> {
    let Some(artifacts) = app.try_state::<Artifacts>() else {
        return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Vec::new())
            .unwrap_or_else(|_| Response::new(Vec::new()));
    };
    let answered = artifacts.answer(label, request);
    if let Some((window, key)) = answered.key {
        // The window's own webview has the window's label.
        if let Some(own) = app.get_webview(&window) {
            let _ = own.set_focus();
        }
        let _ = app.emit_to(
            tauri::EventTarget::webview(window.as_str()),
            ARTIFACT_KEY,
            ArtifactKey {
                label: label.to_owned(),
                key,
            },
        );
    }
    answered.response
}

/// Closes every artifact webview of window `window`: the window went, or its page loads again.
pub(crate) fn close_window_children<R: Runtime>(app: &tauri::AppHandle<R>, window: &str) {
    let Some(artifacts) = app.try_state::<Artifacts>() else {
        return;
    };
    for label in artifacts.forget_window(window) {
        if let Some(webview) = app.get_webview(&label) {
            let _ = webview.close();
        }
    }
}

/// What [`ARTIFACT_KEY`] carries.
#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct ArtifactKey {
    pub label: String,
    pub key: String,
}

/// Where an artifact's frame is in the window: CSS pixels of the window's page, and the page's
/// `devicePixelRatio`, which takes its zoom into account where the window's scale would not.
#[derive(Debug, Clone, Copy, Deserialize, specta::Type)]
pub struct ArtifactPlace {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub ratio: f64,
}

impl ArtifactPlace {
    fn physical(self) -> (tauri::PhysicalPosition<i32>, tauri::PhysicalSize<u32>) {
        let ratio = if self.ratio.is_finite() && self.ratio > 0.0 {
            self.ratio
        } else {
            1.0
        };
        let at = |v: f64| (v * ratio).round();
        // Clamped casts: a frame is never negative in size, and a window is never 2^31 wide.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let size = tauri::PhysicalSize::new(
            at(self.width).max(1.0) as u32,
            at(self.height).max(1.0) as u32,
        );
        #[allow(clippy::cast_possible_truncation)]
        let position = tauri::PhysicalPosition::new(at(self.x) as i32, at(self.y) as i32);
        (position, size)
    }
}

/// One pin, as the activity bar's artifact group draws it.
#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct ArtifactPinShown {
    pub id: String,
    pub folder: String,
    pub name: String,
    /// One of purlis's icons (`PERSONA_ICONS`).
    pub icon: String,
    /// Why its folder cannot be opened, when it cannot.
    pub missing: Option<String>,
    /// Whether its workspace is LIVE: unpinning it takes it off everyone's bar (#5953).
    pub live: bool,
}

/// What an artifact tab's bar says about it before anything runs.
#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct ArtifactAbout {
    pub name: String,
    /// Who pinned it, from the plane's history; none before the pin is saved.
    pub pinned_by: Option<String>,
    /// Why it cannot be opened, when it cannot.
    pub missing: Option<String>,
}

fn workspace_of(
    planes: &Planes,
    plane: &PlaneId,
    workspace: &str,
) -> Result<std::path::PathBuf, String> {
    let root = planes.held(plane)?.root().to_path_buf();
    purlis_core::wscmd::workspace_dir(&root, workspace)
        .ok_or_else(|| format!("'{workspace}' is not a workspace purlis knows"))
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|e| format!("purlis could not read the artifact: {e}"))?
}

/// **The workspace's pinned artifacts, in their order**: the activity bar's artifact group.
#[tauri::command]
#[specta::specta]
pub async fn artifact_pins(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: String,
) -> Result<Vec<ArtifactPinShown>, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    let at = workspace_of(&planes, &plane, &workspace)?;
    blocking(move || {
        let live = purlis_core::wscmd::live_workspaces(&root).contains(&workspace);
        Ok(artifact::list(&at)?
            .into_iter()
            .map(|listed| ArtifactPinShown {
                id: listed.pin.id,
                folder: listed.pin.folder,
                name: listed.pin.name,
                icon: listed.pin.icon,
                missing: listed.missing,
                live,
            })
            .collect())
    })
    .await
}

/// **What an artifact tab's bar names**: the pin's name and who pinned it. Runs nothing of the
/// page's, so a tab that waits for a press asks it too.
#[tauri::command]
#[specta::specta]
pub async fn artifact_about(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: String,
    folder: String,
) -> Result<ArtifactAbout, String> {
    let at = workspace_of(&planes, &plane, &workspace)?;
    blocking(move || {
        let pins = artifact::list(&at)?;
        let Some(listed) = pins.into_iter().find(|one| one.pin.folder == folder) else {
            return Err(format!(
                "'{}' is not pinned to this workspace any more",
                purlis_core::personas::one_line(&folder)
            ));
        };
        Ok(ArtifactAbout {
            pinned_by: artifact::pinned_by(&at, &listed.pin.id),
            name: listed.pin.name,
            missing: listed.missing,
        })
    })
    .await
}

/// What [`artifact_open`] answers: the webview's label, and where the artifact stands on this
/// machine, which its tab's bar says.
#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct ArtifactOpened {
    pub label: String,
    /// `script-free`, `approved`, `new`, `changed`, `pin-changed` or `not-here`
    /// ([`Standing::as_str`]).
    pub standing: String,
    /// Who made the pin it now has, for `pin-changed`, from the plane's history.
    pub changed_by: Option<String>,
}

/// **Opens a pinned artifact over its tab's frame**: the folder read once, where it stands on
/// this machine read with it (its script runs only when approved), a webview of its own made
/// at `place` in the asking window, and its label answered. Nothing is focused: the keyboard
/// stays where the person has it.
#[tauri::command]
#[specta::specta]
pub async fn artifact_open(
    window: tauri::Window,
    planes: tauri::State<'_, Planes>,
    artifacts: tauri::State<'_, Artifacts>,
    plane: PlaneId,
    workspace: String,
    folder: String,
    place: ArtifactPlace,
) -> Result<ArtifactOpened, String> {
    let at = workspace_of(&planes, &plane, &workspace)?;
    let dir = at.clone();
    let read_folder = folder.clone();
    let config = purlis_core::machine::config_root();
    let (host, snapshot, pin, standing, changed_by) = blocking(move || {
        let pins = artifact::read(&at)?;
        let Some(pin) = pins.pins.iter().find(|pin| pin.folder == read_folder) else {
            return Err(format!(
                "'{}' is not pinned to this workspace any more",
                purlis_core::personas::one_line(&read_folder)
            ));
        };
        let real = artifact::resolve(&at, &read_folder)?;
        let snapshot = serve::Snapshot::read(&real)?;
        let standing = approval::standing(config.as_deref(), &at, &read_folder, &pin.id, &snapshot);
        let changed_by = (standing == Standing::PinChanged)
            .then(|| artifact::pinned_by(&at, &pin.id))
            .flatten();
        Ok((
            serve::origin_host(&at, &read_folder),
            snapshot,
            pin.id.clone(),
            standing,
            changed_by,
        ))
    })
    .await?;
    lay(&window, &artifacts, host, snapshot, place)
}

/// What every window hears when a window changed a workspace's pins (#5953): which plane and
/// workspace, so each artifact group of it reads them again at once.
pub(crate) const ARTIFACT_PINS_MOVED: &str = "artifact-pins-moved";

/// What [`ARTIFACT_PINS_MOVED`] carries.
#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct ArtifactPinsMoved {
    pub plane: PlaneId,
    pub workspace: String,
}

/// **An `.html` file the window shows** (#5953): the place it is in, as the file preview names
/// it, and its path there. The plane root is no workspace and no repo; a workspace's own files
/// are a workspace and no repo.
#[derive(Debug, Clone, Deserialize, specta::Type)]
pub struct ArtifactPage {
    pub workspace: String,
    pub repo: String,
    pub piece: Option<String>,
    pub path: String,
}

/// **What the Pin to workspace dialog offers** for a page (#5953).
#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct ArtifactPinAsk {
    /// Whether the workspace is LIVE: everyone in the project sees the pin once it is saved.
    /// A LOCAL workspace's pin stays on this machine.
    pub live: bool,
    /// The folder of `artifacts/` the pin names: the page's own, or the free one it is
    /// copied into.
    pub folder: String,
    /// Whether the page is copied into `folder` as its `index.html` (it is not in
    /// `artifacts/` yet).
    pub copied: bool,
    /// The name offered: the pin's, or the page's title, or the folder's name.
    pub name: String,
    /// The icon offered: the pin's, or the default.
    pub icon: String,
    /// Whether the folder is pinned already: pinning it again changes its name or icon.
    pub pinned: bool,
}

/// The folder of `artifacts/` that `page` already is the page of, in workspace `workspace`:
/// `artifacts/<folder>/index.html` of the workspace's own files.
fn own_folder(workspace: &str, page: &ArtifactPage) -> Option<String> {
    if page.workspace != workspace || !page.repo.is_empty() || page.piece.is_some() {
        return None;
    }
    let rest = page.path.strip_prefix(&format!("{}/", artifact::DIR))?;
    let folder = rest.strip_suffix(&format!("/{}", artifact::PAGE))?;
    (!folder.is_empty() && !folder.contains('/')).then(|| folder.to_owned())
}

/// The text of `page`, read as the file preview reads it; refused for anything but text.
fn page_text(root: &std::path::Path, page: &ArtifactPage) -> Result<String, String> {
    let name = purlis_core::personas::one_line(&page.path);
    match crate::piecefiles::file_at(root, &page.workspace, &page.repo, &page.piece, &page.path)? {
        crate::piecefiles::PieceFile::Text { text } => Ok(text),
        crate::piecefiles::PieceFile::TooLarge { .. } => Err(format!(
            "{name} is past what the preview reads (2 MiB), so it is not pinned"
        )),
        _ => Err(format!("{name} is not a text file, so it is not a page")),
    }
}

/// [`ArtifactPinAsk`], read from disk.
fn pin_ask(
    root: &std::path::Path,
    at: &std::path::Path,
    workspace: &str,
    page: &ArtifactPage,
) -> Result<ArtifactPinAsk, String> {
    let live = purlis_core::wscmd::live_workspaces(root).contains(workspace);
    if let Some(folder) = own_folder(workspace, page) {
        let real = artifact::resolve(at, &folder)?;
        let pinned = artifact::read(at)?
            .pins
            .into_iter()
            .find(|pin| pin.folder == folder);
        let text = std::fs::read_to_string(real.join(artifact::PAGE)).unwrap_or_default();
        return Ok(ArtifactPinAsk {
            live,
            name: pinned.as_ref().map_or_else(
                || artifact::suggested_name(&text, &folder),
                |pin| pin.name.clone(),
            ),
            icon: pinned
                .as_ref()
                .map_or_else(|| artifact::DEFAULT_ICON.to_owned(), |pin| pin.icon.clone()),
            pinned: pinned.is_some(),
            folder,
            copied: false,
        });
    }
    let text = page_text(root, page)?;
    let stem = page
        .path
        .rsplit('/')
        .next()
        .unwrap_or_default()
        .trim_end_matches(".html")
        .trim_end_matches(".htm");
    let name = artifact::suggested_name(&text, stem);
    Ok(ArtifactPinAsk {
        live,
        folder: artifact::free_folder(at, &name),
        copied: true,
        name,
        icon: artifact::DEFAULT_ICON.to_owned(),
        pinned: false,
    })
}

/// **What Pin to workspace offers for `page`** in workspace `workspace` (#5953): whether the
/// workspace is LIVE, the folder it is pinned as, and a name and icon to start from.
#[tauri::command]
#[specta::specta]
pub async fn artifact_pin_ask(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: String,
    page: ArtifactPage,
) -> Result<ArtifactPinAsk, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    let at = workspace_of(&planes, &plane, &workspace)?;
    blocking(move || pin_ask(&root, &at, &workspace, &page)).await
}

/// Tells every window that workspace `workspace`'s pins changed.
fn moved<R: Runtime>(app: &tauri::AppHandle<R>, plane: PlaneId, workspace: String) {
    let _ = app.emit(ARTIFACT_PINS_MOVED, ArtifactPinsMoved { plane, workspace });
}

fn shown(pin: artifact::Pin, live: bool) -> ArtifactPinShown {
    ArtifactPinShown {
        id: pin.id,
        folder: pin.folder,
        name: pin.name,
        icon: pin.icon,
        missing: None,
        live,
    }
}

/// **What the person answered Pin to workspace with** (#5953): the folder the ask offered, and
/// the name and icon they chose.
#[derive(Debug, Clone, Deserialize, specta::Type)]
pub struct ArtifactPinAnswer {
    pub folder: String,
    pub name: String,
    pub icon: String,
}

/// **Pins `page` to workspace `workspace`** as `folder`, named `name` with icon `icon`
/// (#5953): the person's answer to Pin to workspace. A page not in `artifacts/` yet is copied
/// into `folder` first, never over a folder that is there. Written to the pins file, which the
/// project's next save shares; nothing is committed here.
#[tauri::command]
#[specta::specta]
pub async fn artifact_pin(
    app: tauri::AppHandle,
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: String,
    page: ArtifactPage,
    answer: ArtifactPinAnswer,
) -> Result<ArtifactPinShown, String> {
    let ArtifactPinAnswer { folder, name, icon } = answer;
    let root = planes.held(&plane)?.root().to_path_buf();
    let at = workspace_of(&planes, &plane, &workspace)?;
    let ws = workspace.clone();
    let (pinned, live) = blocking(move || {
        let live = purlis_core::wscmd::live_workspaces(&root).contains(&ws);
        if own_folder(&ws, &page).as_deref() != Some(folder.as_str()) {
            artifact::adopt(&root, &at, &folder, &page_text(&root, &page)?)?;
        }
        let request = artifact::Request {
            folder,
            name: Some(name),
            icon: Some(icon),
            ..artifact::Request::default()
        };
        let pin = match artifact::pin(&root, &at, &request)? {
            artifact::Pinned::New(pin) | artifact::Pinned::Updated(pin) => pin,
            artifact::Pinned::Replaced { new, .. } => new,
        };
        Ok((pin, live))
    })
    .await?;
    moved(&app, plane, workspace);
    Ok(shown(pinned, live))
}

/// **Unpins pin `id` from workspace `workspace`** (#5953): it leaves the activity bar, for
/// everyone once the project is saved, and its folder stays.
#[tauri::command]
#[specta::specta]
pub async fn artifact_unpin(
    app: tauri::AppHandle,
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: String,
    id: String,
) -> Result<(), String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    let at = workspace_of(&planes, &plane, &workspace)?;
    blocking(move || artifact::unpin(&root, &at, &id).map(|_| ())).await?;
    moved(&app, plane, workspace);
    Ok(())
}

/// **Puts workspace `workspace`'s pins in the order `ids` gives** (#5953): a drag or a Move
/// on the activity bar. A pin `ids` leaves out keeps its place after them.
#[tauri::command]
#[specta::specta]
pub async fn artifact_reorder(
    app: tauri::AppHandle,
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: String,
    ids: Vec<String>,
) -> Result<(), String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    let at = workspace_of(&planes, &plane, &workspace)?;
    blocking(move || artifact::reorder(&root, &at, &ids).map(|_| ())).await?;
    moved(&app, plane, workspace);
    Ok(())
}

/// **Opens an `.html` file's private preview over its frame** (#5953, spec story 15): the page
/// alone, read once, served from an origin of its own under the same CSP as an artifact's, with
/// no script and nothing beside it. Nothing is pinned and nothing is written.
#[tauri::command]
#[specta::specta]
pub async fn artifact_preview_open(
    window: tauri::Window,
    planes: tauri::State<'_, Planes>,
    artifacts: tauri::State<'_, Artifacts>,
    plane: PlaneId,
    page: ArtifactPage,
    place: ArtifactPlace,
) -> Result<String, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    let (host, snapshot) = blocking(move || {
        let text = page_text(&root, &page)?;
        let host = serve::origin_host(
            &root,
            &format!(
                "preview\u{0}{}\u{0}{}\u{0}{}\u{0}{}",
                page.workspace,
                page.repo,
                page.piece.as_deref().unwrap_or(""),
                page.path
            ),
        );
        Ok((host, serve::Snapshot::page(text.into_bytes())))
    })
    .await?;
    lay(&window, &artifacts, host, snapshot, place)
}

/// Makes the webview of what `snapshot` serves from `host`, at `place` in `window`: its label.
fn lay(
    window: &tauri::Window,
    artifacts: &Artifacts,
    host: String,
    snapshot: serve::Snapshot,
    place: ArtifactPlace,
) -> Result<String, String> {
    let label = label(artifacts.made.fetch_add(1, Ordering::Relaxed) + 1);
    let own = origin(&host);
    artifacts.served().insert(
        label.clone(),
        Served {
            window: window.label().to_owned(),
            host,
            snapshot,
            of: Of {
                plane,
                workspace,
                dir,
                folder,
                pin,
                standing,
            },
        },
    );
    let url: tauri::Url = format!("{own}/")
        .parse()
        .map_err(|e| format!("purlis could not name the artifact's page: {e}"))?;
    let leave = format!("{own}/");
    let builder =
        tauri::webview::WebviewBuilder::new(&label, tauri::WebviewUrl::CustomProtocol(url))
            .on_navigation(move |to| to.as_str().starts_with(&leave))
            .on_new_window(crate::navguard::no_new_window)
            .initialization_script(KEYS)
            .incognito(true)
            .focused(false)
            .devtools(false)
            .disable_drag_drop_handler();
    let (position, size) = place.physical();
    if let Err(why) = window.add_child(builder, position, size) {
        artifacts.served().remove(&label);
        return Err(format!("purlis could not draw the artifact: {why}"));
    }
    Ok(ArtifactOpened {
        label,
        standing: standing.as_str().to_owned(),
        changed_by,
    })
}

/// **The person's approval of an artifact's script, pressed in its tab's bar** (ADR 0120 §4):
/// of exactly the pin and the template webview `label` was served, read here and never taken
/// from the window, so a change since it was served is not approved by this press. Every window
/// of the plane hears it ([`APPROVALS_CHANGED`]) and loads the page again, now with its script.
/// Window-only: nothing a link serves, and no command a chat runs, can approve.
#[tauri::command]
#[specta::specta]
pub async fn artifact_approve(
    app: tauri::AppHandle,
    window: tauri::Window,
    artifacts: tauri::State<'_, Artifacts>,
    label: String,
) -> Result<(), String> {
    let Some((of, fingerprint)) = artifacts.served_to(&label, window.label()) else {
        return Err("that artifact is not open in this window any more".into());
    };
    if !of.standing.asks() {
        return Err("that artifact is not asking to be approved".into());
    }
    let config = crate::extensions::config_root()
        .map_err(|_| "this machine has no config home, so purlis keeps no approvals".to_owned())?;
    let (dir, folder, pin) = (of.dir.clone(), of.folder.clone(), of.pin.clone());
    blocking(move || approval::approve(&config, &dir, &folder, &pin, &fingerprint)).await?;
    crate::windows::emit_for_plane(
        &app,
        &of.plane,
        APPROVALS_CHANGED,
        ArtifactApprovalChanged {
            workspace: of.workspace,
            folder: of.folder,
        },
    );
    Ok(())
}

/// The artifact webview `label` of the asking window.
fn child<R: Runtime>(
    window: &tauri::Window<R>,
    artifacts: &Artifacts,
    label: &str,
) -> Option<tauri::Webview<R>> {
    if !is_artifact(label) || !artifacts.owned_by(label, window.label()) {
        return None;
    }
    window.app_handle().get_webview(label)
}

/// **Moves an artifact's webview to its frame's place**, or hides it (`None`): its tab went
/// behind another, or something purlis draws is over its frame.
#[tauri::command]
#[specta::specta]
pub async fn artifact_place(
    window: tauri::Window,
    artifacts: tauri::State<'_, Artifacts>,
    label: String,
    place: Option<ArtifactPlace>,
) -> Result<(), String> {
    let Some(webview) = child(&window, &artifacts, &label) else {
        return Ok(());
    };
    match place {
        None => webview.hide().map_err(|e| e.to_string()),
        Some(place) => {
            let (position, size) = place.physical();
            webview.set_position(position).map_err(|e| e.to_string())?;
            webview.set_size(size).map_err(|e| e.to_string())?;
            webview.show().map_err(|e| e.to_string())
        }
    }
}

/// **Puts the keyboard in an artifact's page**: the person entered its frame.
#[tauri::command]
#[specta::specta]
pub async fn artifact_enter(
    window: tauri::Window,
    artifacts: tauri::State<'_, Artifacts>,
    label: String,
) -> Result<(), String> {
    match child(&window, &artifacts, &label) {
        Some(webview) => webview.set_focus().map_err(|e| e.to_string()),
        None => Ok(()),
    }
}

/// **Closes an artifact's webview**: its tab closed, or its frame went.
#[tauri::command]
#[specta::specta]
pub async fn artifact_close(
    window: tauri::Window,
    artifacts: tauri::State<'_, Artifacts>,
    label: String,
) -> Result<(), String> {
    let webview = child(&window, &artifacts, &label);
    artifacts.served().remove(&label);
    if let Some(webview) = webview {
        webview.close().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(files: &[(&str, &str)]) -> serve::Snapshot {
        let dir = tempfile::tempdir().unwrap();
        for (path, text) in files {
            let at = dir.path().join(path);
            std::fs::create_dir_all(at.parent().unwrap()).unwrap();
            std::fs::write(at, text).unwrap();
        }
        serve::Snapshot::read(dir.path()).unwrap()
    }

    fn served(pairs: &[(&str, &str, &str)]) -> Artifacts {
        let artifacts = Artifacts::default();
        for (label, window, host) in pairs {
            artifacts.served().insert(
                (*label).to_owned(),
                Served {
                    window: (*window).to_owned(),
                    host: (*host).to_owned(),
                    snapshot: snapshot(&[
                        ("index.html", &format!("<h1>{host}</h1>")),
                        ("img/a.png", "png"),
                    ]),
                    of: of(Standing::ScriptFree),
                },
            );
        }
        artifacts
    }

    fn of(standing: Standing) -> Of {
        Of {
            plane: serde_json::from_str("\"plane\"").unwrap(),
            workspace: "alpha".to_owned(),
            dir: std::path::PathBuf::from("/p/workspaces/alpha"),
            folder: "board".to_owned(),
            pin: "0123456789ab".to_owned(),
            standing,
        }
    }

    fn standing_of(artifacts: &Artifacts, label: &str, standing: Standing) {
        artifacts.served().get_mut(label).unwrap().of.standing = standing;
    }

    fn get(url: &str) -> Request<Vec<u8>> {
        Request::builder().uri(url).body(Vec::new()).unwrap()
    }

    fn at(host: &str, path: &str) -> String {
        format!("{}{path}", origin(host))
    }

    fn page(workspace: &str, repo: &str, piece: Option<&str>, path: &str) -> ArtifactPage {
        ArtifactPage {
            workspace: workspace.to_owned(),
            repo: repo.to_owned(),
            piece: piece.map(str::to_owned),
            path: path.to_owned(),
        }
    }

    #[test]
    fn only_a_folder_s_index_html_in_the_workspace_s_own_artifacts_is_pinned_where_it_is() {
        let own = page("alpha", "", None, "artifacts/backlog/index.html");
        assert_eq!(own_folder("alpha", &own).as_deref(), Some("backlog"));
        // Pinned to another workspace, it is copied there.
        assert_eq!(own_folder("beta", &own), None);
        for elsewhere in [
            page("alpha", "", None, "artifacts/backlog/other.html"),
            page("alpha", "", None, "artifacts/a/b/index.html"),
            page("alpha", "", None, "artifacts//index.html"),
            page("alpha", "", None, "notes/index.html"),
            page("alpha", "web", None, "artifacts/backlog/index.html"),
            page("alpha", "web", Some("fix"), "artifacts/backlog/index.html"),
            page("", "", None, "artifacts/backlog/index.html"),
        ] {
            assert_eq!(own_folder("alpha", &elsewhere), None, "{elsewhere:?}");
        }
    }

    #[test]
    fn an_artifact_label_matches_no_window_purlis_calls_its_own() {
        for n in [1, 27, 1000] {
            let label = label(n);
            assert!(is_artifact(&label));
            assert!(!crate::windows::is_charter_window(&label), "{label}");
        }
        for not in ["artifact-", "artifact-x", "artifact-1a", "main", "window-1"] {
            assert!(!is_artifact(not), "{not}");
        }
    }

    #[test]
    fn a_webview_is_served_its_own_bytes_under_the_csp_on_every_response() {
        let artifacts = served(&[("artifact-1", "main", "aaa")]);
        for (path, status, body) in [
            ("/", 200, "<h1>aaa</h1>"),
            ("/index.html", 200, "<h1>aaa</h1>"),
            ("/img/a.png", 200, "png"),
            ("/missing.css", 404, ""),
            ("/..%2f..%2fetc/passwd", 404, ""),
        ] {
            let answered = artifacts.answer("artifact-1", &get(&at("aaa", path)));
            let response = answered.response;
            assert_eq!(response.status().as_u16(), status, "{path}");
            assert_eq!(response.body(), body.as_bytes(), "{path}");
            assert_eq!(
                response.headers()["Content-Security-Policy"],
                serve::csp(&origin("aaa"), false).as_str(),
                "{path}"
            );
            assert_eq!(response.headers()["X-Content-Type-Options"], "nosniff");
        }
    }

    #[test]
    fn a_request_is_attributed_by_its_webview_s_label_never_by_the_origin_it_names() {
        let artifacts = served(&[("artifact-1", "main", "aaa"), ("artifact-2", "main", "bbb")]);
        // Another artifact's origin, asked from this webview.
        let crossed = artifacts.answer("artifact-1", &get(&at("bbb", "/index.html")));
        assert_eq!(crossed.response.status(), StatusCode::NOT_FOUND);
        assert!(crossed.response.body().is_empty());
        // The window's own webview, or any webview purlis did not open for an artifact.
        for label in ["main", "window-1", "artifact-9"] {
            let asked = artifacts.answer(label, &get(&at("aaa", "/index.html")));
            assert_eq!(asked.response.status(), StatusCode::NOT_FOUND, "{label}");
        }
    }

    #[test]
    fn a_page_runs_script_only_when_its_person_approved_it_here() {
        let artifacts = served(&[("artifact-1", "main", "aaa")]);
        let csp = |artifacts: &Artifacts| {
            artifacts
                .answer("artifact-1", &get(&at("aaa", "/")))
                .response
                .headers()["Content-Security-Policy"]
                .to_str()
                .unwrap()
                .to_owned()
        };
        for standing in [
            Standing::ScriptFree,
            Standing::New,
            Standing::Changed,
            Standing::PinChanged,
            Standing::NotHere,
        ] {
            standing_of(&artifacts, "artifact-1", standing);
            assert_eq!(
                csp(&artifacts),
                serve::csp(&origin("aaa"), false),
                "{standing:?}"
            );
        }
        standing_of(&artifacts, "artifact-1", Standing::Approved);
        assert_eq!(csp(&artifacts), serve::csp(&origin("aaa"), true));
        // Another webview asking for the approved one's origin is still answered nothing.
        let crossed = artifacts.answer("artifact-2", &get(&at("aaa", "/")));
        assert_eq!(crossed.response.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn an_approval_is_of_what_its_own_window_was_served_and_nothing_else() {
        let artifacts = served(&[("artifact-1", "window-2", "aaa")]);
        let (of, fingerprint) = artifacts.served_to("artifact-1", "window-2").unwrap();
        assert_eq!(of.pin, "0123456789ab");
        assert_eq!(fingerprint.len(), 64);
        assert!(artifacts.served_to("artifact-1", "main").is_none());
        assert!(artifacts.served_to("artifact-9", "window-2").is_none());
    }

    #[test]
    fn esc_and_f6_in_the_page_hand_the_keyboard_to_its_own_window_only() {
        let artifacts = served(&[("artifact-1", "window-2", "aaa")]);
        for key in KEY_NAMES {
            let answered = artifacts.answer(
                "artifact-1",
                &get(&at("aaa", &format!("/.purlis/key/{key}"))),
            );
            assert_eq!(answered.response.status(), StatusCode::NO_CONTENT);
            assert_eq!(answered.key, Some(("window-2".to_owned(), key.to_owned())));
        }
        let other = artifacts.answer("artifact-1", &get(&at("aaa", "/.purlis/key/enter")));
        assert_eq!(other.key, None);
        let elsewhere = artifacts.answer("main", &get(&at("aaa", "/.purlis/key/escape")));
        assert_eq!(elsewhere.key, None);
    }

    #[test]
    fn a_page_navigates_within_its_own_origin_only() {
        let artifacts = served(&[("artifact-1", "main", "aaa"), ("artifact-2", "main", "bbb")]);
        let url = |text: &str| tauri::Url::parse(text).unwrap();
        assert!(artifacts.may_navigate("artifact-1", &url(&at("aaa", "/"))));
        assert!(artifacts.may_navigate("artifact-1", &url(&at("aaa", "/sub/page.html"))));
        for away in [
            at("bbb", "/"),
            at("aaab", "/"),
            "https://example.com/".to_owned(),
            "file:///etc/passwd".to_owned(),
            "tauri://localhost/".to_owned(),
            "about:blank".to_owned(),
        ] {
            assert!(!artifacts.may_navigate("artifact-1", &url(&away)), "{away}");
        }
        assert!(!artifacts.may_navigate("main", &url(&at("aaa", "/"))));
    }

    #[test]
    fn a_window_that_goes_takes_only_its_own_artifacts_with_it() {
        let artifacts = served(&[
            ("artifact-1", "main", "aaa"),
            ("artifact-2", "window-1", "bbb"),
            ("artifact-3", "main", "ccc"),
        ]);
        let mut gone = artifacts.forget_window("main");
        gone.sort();
        assert_eq!(gone, ["artifact-1", "artifact-3"]);
        assert!(artifacts.owned_by("artifact-2", "window-1"));
        assert!(!artifacts.owned_by("artifact-1", "main"));
        assert!(!artifacts.owned_by("artifact-2", "main"));
    }

    #[test]
    fn a_frame_s_place_is_drawn_at_the_page_s_pixel_ratio() {
        let place = ArtifactPlace {
            x: 10.5,
            y: 20.0,
            width: 300.0,
            height: 0.0,
            ratio: 2.0,
        };
        let (position, size) = place.physical();
        assert_eq!((position.x, position.y), (21, 40));
        assert_eq!((size.width, size.height), (600, 1));
        let odd = ArtifactPlace {
            ratio: f64::NAN,
            ..place
        };
        assert_eq!(odd.physical().1.width, 300);
    }
}
