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
//! - **An approved page reads its own data through one port per load** (ADR 0120 §7, #5961):
//!   purlis's script in the page ([`PORT`]) hands it `purlis.port`, a `MessagePort`, and carries
//!   what it is asked to purlis on its own origin's `/.purlis/port/<load>/…`, attributed by the
//!   webview's label. A load's port opens with its first word and is revoked when the page loads
//!   again ([`Artifacts::reloaded`]); an answer for a load that is gone is never delivered. Each
//!   read is gated again when it is asked: the pin and approval read again, and only a data file
//!   the page declared ([`artifact::data`]), answered in the catalogue's envelope.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};

use purlis_core::artifact::approval::{self, Standing};
use purlis_core::artifact::{self, data, serve};
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

/// **purlis's script in every artifact page that hands it its port** (#5961): `purlis.port`, a
/// `MessagePort` made for this load. The page posts `{ id, read: "<data file>" }` or, for a
/// catalogue query it declared, `{ id, query: "<id>@<N>", params }` (#5969, [`calls`]) on it and
/// hears `{ id, data | error: { kind }, at, cursor? }`. The load is named by a random word of its own,
/// opened first and then carried on each ask, as an image of its own origin, which is all the CSP
/// lets it reach; the answer comes back through [`ANSWER`], for this load's word only.
const PORT: &str = r#"(() => {
  const load = Array.from(crypto.getRandomValues(new Uint8Array(16)), (b) => b.toString(16).padStart(2, "0")).join("");
  const channel = new MessageChannel();
  const own = channel.port1;
  const send = (path) => { new Image().src = "/.purlis/port/" + load + "/" + path; };
  let open = false;
  const waiting = [];
  const opening = new Image();
  opening.onload = opening.onerror = () => { open = true; waiting.splice(0).forEach(send); };
  opening.src = "/.purlis/port/" + load + "/open";
  own.onmessage = (event) => {
    const asked = event.data;
    const id = asked !== null && typeof asked === "object" ? asked.id : undefined;
    const refuse = () => own.postMessage({ id: Number.isSafeInteger(id) ? id : null, error: { kind: "refused" }, at: new Date().toISOString().replace(/\.\d+Z$/, "Z") });
    if (!Number.isSafeInteger(id) || id < 0 || (typeof asked.read !== "string" && typeof asked.query !== "string")) {
      refuse();
      return;
    }
    let path;
    try {
      path = typeof asked.query === "string"
        ? "query/" + id + "?" + encodeURIComponent(JSON.stringify({ query: asked.query, params: asked.params === undefined ? null : asked.params }))
        : "read/" + id + "?" + encodeURIComponent(asked.read);
    } catch {
      refuse();
      return;
    }
    if (open) send(path); else waiting.push(path);
  };
  Object.defineProperty(window, "__purlisPort", { value: (to, answer) => { if (to === load) own.postMessage(answer); } });
  Object.defineProperty(window, "purlis", { value: Object.freeze({ api: 1, port: channel.port2 }) });
})();"#;

/// The function [`PORT`] leaves for purlis to hand a load its answer with.
const ANSWER: &str = "__purlisPort";

/// Whether `word` is a load's word as [`PORT`] makes one: 32 lowercase hex digits.
fn is_load(word: &str) -> bool {
    word.len() == 32 && word.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// One read a page asked for on its port: the webview, its load, the page's id and the name:
/// a data file's, or with `params` a catalogue query's (#5969).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Ask {
    pub(crate) label: String,
    pub(crate) load: String,
    pub(crate) id: u64,
    pub(crate) name: String,
    pub(crate) params: Option<serde_json::Value>,
}

pub(crate) mod calls;

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
    /// What its page declared it reads, read from the bytes served.
    declared: Option<data::Declared>,
    /// The word of the load whose port is open, if one is.
    load: Option<String>,
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
    /// A read the page asked for on its open port.
    pub(crate) ask: Option<Ask>,
}

impl Served {
    /// What the page said on its port, `rest` after `/.purlis/port/`: `<load>/open` opens the
    /// load's port, closing any other; `<load>/read/<id>?<name>` asks, on the open port only.
    fn port(&mut self, label: &str, rest: &str, query: Option<&str>) -> Option<Ask> {
        let (load, said) = rest.split_once('/')?;
        if !is_load(load) {
            return None;
        }
        if said == "open" {
            self.load = Some(load.to_owned());
            return None;
        }
        if self.load.as_deref() != Some(load) {
            return None;
        }
        if let Some(id) = said.strip_prefix("query/") {
            let (name, params) = calls::asked(query?)?;
            return Some(Ask {
                label: label.to_owned(),
                load: load.to_owned(),
                id: page_id(id)?,
                name,
                params: Some(params),
            });
        }
        let id = said.strip_prefix("read/")?;
        if id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let name = percent_encoding::percent_decode_str(query.unwrap_or(""))
            .decode_utf8()
            .ok()?
            .into_owned();
        Some(Ask {
            label: label.to_owned(),
            load: load.to_owned(),
            id: id.parse().ok()?,
            name,
            params: None,
        })
    }
}

/// The page's id of an ask, as [`PORT`] sends it: digits only.
fn page_id(id: &str) -> Option<u64> {
    if id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    id.parse().ok()
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
        let mut served = self.served();
        let own = served.get_mut(label).filter(|one| one.host == host);
        let mut key = None;
        let mut ask = None;
        let runs = own.as_ref().is_some_and(|one| one.of.standing.runs());
        let (status, body, kind) = match own {
            None => (StatusCode::NOT_FOUND, Vec::new(), "text/plain"),
            Some(one) => {
                if let Some(rest) = path.strip_prefix(serve::OWN_PATH) {
                    let pressed = rest.strip_prefix("key/").filter(|k| KEY_NAMES.contains(k));
                    key = pressed.map(|k| (one.window.clone(), k.to_owned()));
                    if let Some(port) = rest.strip_prefix("port/") {
                        ask = one.port(label, port, uri.query());
                    }
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
        Answered { response, key, ask }
    }

    /// **Webview `label`'s page loads again**: the port its last load opened is revoked, so
    /// nothing it asked is answered and no answer for it is delivered.
    pub(crate) fn reloaded(&self, label: &str) {
        if let Some(one) = self.served().get_mut(label) {
            one.load = None;
        }
    }

    /// Whether `load` is still the load of webview `label` whose port is open.
    fn still_open(&self, label: &str, load: &str) -> bool {
        self.served()
            .get(label)
            .is_some_and(|one| one.load.as_deref() == Some(load))
    }

    /// What answering `ask` needs, read while its port is still open: the artifact, the bytes
    /// served, and what its page declared.
    fn for_ask(&self, ask: &Ask) -> Option<(Of, serve::Snapshot, Option<data::Declared>)> {
        self.served()
            .get(&ask.label)
            .filter(|one| one.load.as_deref() == Some(ask.load.as_str()))
            .map(|one| (one.of.clone(), one.snapshot.clone(), one.declared.clone()))
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
    if let Some(ask) = answered.ask {
        answer_ask(app.clone(), ask);
    }
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

/// **Answers a read a page asked for on its port** (#5961), off the scheme's thread, and hands
/// the answer to the page only while the load that asked is still the open one.
fn answer_ask<R: Runtime>(app: tauri::AppHandle<R>, ask: Ask) {
    tauri::async_runtime::spawn_blocking(move || {
        let Some(artifacts) = app.try_state::<Artifacts>() else {
            return;
        };
        let Some((of, snapshot, declared)) = artifacts.for_ask(&ask) else {
            return;
        };
        let config = purlis_core::machine::config_root();
        let answer = match &ask.params {
            None => read_asked(
                config.as_deref(),
                &of,
                &snapshot,
                declared.as_ref(),
                &ask.name,
            ),
            Some(params) => calls::answer(
                &app,
                config.as_deref(),
                &of,
                &snapshot,
                declared.as_ref(),
                &ask.name,
                params,
            ),
        };
        if !artifacts.still_open(&ask.label, &ask.load) {
            return;
        }
        let Some(webview) = app.get_webview(&ask.label) else {
            return;
        };
        let _ = webview.eval(delivery(&ask, &answer));
    });
}

/// **The answer to a page asking for data file `name`**, gated again at the ask: the pin is
/// still the one served and its standing on this machine still runs the page's script, or it
/// is `not-approved`; then [`data::read`].
fn read_asked(
    config: Option<&std::path::Path>,
    of: &Of,
    snapshot: &serve::Snapshot,
    declared: Option<&data::Declared>,
    name: &str,
) -> purlis_core::catalogue::Answer {
    use purlis_core::catalogue::{Answer, ErrorKind};
    let now = chrono::Utc::now();
    if !still_runs(config, of, snapshot) {
        return Answer::error(ErrorKind::NotApproved, now);
    }
    match artifact::resolve(&of.dir, &of.folder) {
        Ok(folder) => data::read(&folder, declared, name, now),
        Err(_) => Answer::error(ErrorKind::Failed, now),
    }
}

/// **Whether the page served as `snapshot` still runs its script**, read again at the ask: its
/// pin still the one served, and its standing on this machine still approved.
fn still_runs(config: Option<&std::path::Path>, of: &Of, snapshot: &serve::Snapshot) -> bool {
    let pinned = artifact::read(&of.dir)
        .ok()
        .and_then(|pins| pins.pins.into_iter().find(|pin| pin.folder == of.folder))
        .filter(|pin| pin.id == of.pin);
    pinned.is_some() && approval::standing(config, &of.dir, &of.folder, &of.pin, snapshot).runs()
}

/// The script that hands `answer` to the load that asked: data, as JSON, never markup.
fn delivery(ask: &Ask, answer: &purlis_core::catalogue::Answer) -> String {
    let mut said = answer.to_json();
    if let Some(fields) = said.as_object_mut() {
        fields.insert("id".into(), serde_json::json!(ask.id));
    }
    format!(
        "window.{ANSWER} && window.{ANSWER}({}, {});",
        serde_json::Value::String(ask.load.clone()),
        said
    )
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
    /// Its folder is over the size cap: the sentence naming both sizes.
    pub too_large: Option<String>,
}

/// What an artifact tab's bar says about it before anything runs.
#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct ArtifactAbout {
    pub name: String,
    /// Who pinned it, from the plane's history; none before the pin is saved.
    pub pinned_by: Option<String>,
    /// Why it cannot be opened, when it cannot.
    pub missing: Option<String>,
    /// Its folder is over the size cap: the sentence naming both sizes.
    pub too_large: Option<String>,
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
    let at = workspace_of(&planes, &plane, &workspace)?;
    blocking(move || {
        Ok(artifact::list(&at)?
            .into_iter()
            .map(|listed| ArtifactPinShown {
                id: listed.pin.id,
                folder: listed.pin.folder,
                name: listed.pin.name,
                icon: listed.pin.icon,
                missing: listed.missing,
                too_large: listed.too_large,
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
            too_large: listed.too_large,
        })
    })
    .await
}

/// **Repin**: pins artifact folder `folder` again as it is pinned, its name, icon, command and
/// working folder kept, so the folder is checked anew (there, inside `artifacts/`, within the
/// size cap, nothing shaped like a secret). The way out of a missing or too-large artifact once
/// its folder is back or smaller; the refusal's sentence when it is not.
#[tauri::command]
#[specta::specta]
pub async fn artifact_repin(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: String,
    folder: String,
) -> Result<(), String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    let at = workspace_of(&planes, &plane, &workspace)?;
    blocking(move || {
        let pins = artifact::read(&at)?;
        let Some(pin) = pins.pins.into_iter().find(|one| one.folder == folder) else {
            return Err(format!(
                "'{}' is not pinned to this workspace any more",
                purlis_core::personas::one_line(&folder)
            ));
        };
        artifact::pin(
            &root,
            &at,
            &artifact::Request {
                folder: pin.folder,
                name: Some(pin.name),
                icon: Some(pin.icon),
                command: pin.command,
                cwd: pin.cwd,
            },
        )
        .map(|_| ())
    })
    .await
}

/// **Unpin**: takes artifact folder `folder`'s pin off the workspace; its files stay.
#[tauri::command]
#[specta::specta]
pub async fn artifact_unpin(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: String,
    folder: String,
) -> Result<(), String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    let at = workspace_of(&planes, &plane, &workspace)?;
    blocking(move || artifact::unpin(&root, &at, &folder).map(|_| ())).await
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
    /// Whether the data its page declared was generated on this machine (#5961): false when it
    /// declares data files and none is here, which the bar says.
    pub generated: bool,
    /// The catalogue queries its page declared it calls, as its approval lists them (#5969).
    pub queries: Vec<calls::ArtifactQuery>,
    /// Whether it may call its plane-scoped queries: a grant of its own (#5969).
    pub plane_granted: bool,
}

/// What the page `snapshot` opens on declared it reads; an unreadable declaration is none.
fn declared_by(snapshot: &serve::Snapshot) -> Option<data::Declared> {
    let page = snapshot.get(artifact::PAGE)?;
    data::declared(&String::from_utf8_lossy(page))
        .ok()
        .flatten()
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
    let (host, snapshot, pin, standing, changed_by, declared, generated, plane_granted) =
        blocking(move || {
            let pins = artifact::read(&at)?;
            let Some(pin) = pins.pins.iter().find(|pin| pin.folder == read_folder) else {
                return Err(format!(
                    "'{}' is not pinned to this workspace any more",
                    purlis_core::personas::one_line(&read_folder)
                ));
            };
            let real = artifact::resolve(&at, &read_folder)?;
            let snapshot = serve::Snapshot::read(&real, &read_folder)?;
            let standing =
                approval::standing(config.as_deref(), &at, &read_folder, &pin.id, &snapshot);
            let changed_by = (standing == Standing::PinChanged)
                .then(|| artifact::pinned_by(&at, &pin.id))
                .flatten();
            let declared = declared_by(&snapshot);
            let generated = data::generated(&real, declared.as_ref());
            let plane_granted =
                approval::plane_granted(config.as_deref(), &at, &read_folder, &pin.id, &snapshot);
            Ok((
                serve::origin_host(&at, &read_folder),
                snapshot,
                pin.id.clone(),
                standing,
                changed_by,
                declared,
                generated,
                plane_granted,
            ))
        })
        .await?;
    let queries = calls::listed(declared.as_ref());
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
            declared,
            load: None,
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
            .initialization_script(PORT)
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
        generated,
        queries,
        plane_granted,
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
        serve::Snapshot::read(dir.path(), "board").unwrap()
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
                    declared: None,
                    load: None,
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

    const LOAD_A: &str = "0123456789abcdef0123456789abcdef";
    const LOAD_B: &str = "fedcba9876543210fedcba9876543210";

    fn said(artifacts: &Artifacts, label: &str, path: &str) -> Option<Ask> {
        let answered = artifacts.answer(label, &get(&at("aaa", path)));
        assert_eq!(answered.response.status(), StatusCode::NO_CONTENT, "{path}");
        answered.ask
    }

    fn read_path(load: &str, id: u64, name: &str) -> String {
        format!("/.purlis/port/{load}/read/{id}?{name}")
    }

    #[test]
    fn a_load_s_port_opens_with_its_first_word_and_answers_that_load_only() {
        let artifacts = served(&[("artifact-1", "main", "aaa")]);
        // Nothing is asked before the load opened its port.
        assert_eq!(
            said(&artifacts, "artifact-1", &read_path(LOAD_A, 1, "data.json")),
            None
        );
        assert_eq!(
            said(
                &artifacts,
                "artifact-1",
                &format!("/.purlis/port/{LOAD_A}/open")
            ),
            None
        );
        assert_eq!(
            said(
                &artifacts,
                "artifact-1",
                &read_path(LOAD_A, 7, "data%2Frows.json")
            ),
            Some(Ask {
                label: "artifact-1".to_owned(),
                load: LOAD_A.to_owned(),
                id: 7,
                name: "data/rows.json".to_owned(),
                params: None,
            })
        );
        assert!(artifacts.still_open("artifact-1", LOAD_A));
        // Another load's word is not this port.
        assert_eq!(
            said(&artifacts, "artifact-1", &read_path(LOAD_B, 1, "data.json")),
            None
        );
        for malformed in [
            "/.purlis/port/short/open".to_owned(),
            format!("/.purlis/port/{LOAD_A}/read/x?data.json"),
            format!("/.purlis/port/{LOAD_A}/read/?data.json"),
            format!("/.purlis/port/{LOAD_A}/write/1?data.json"),
            format!("/.purlis/port/{LOAD_A}/read/1?%ff"),
        ] {
            assert_eq!(
                said(&artifacts, "artifact-1", &malformed),
                None,
                "{malformed}"
            );
        }
    }

    #[test]
    fn a_reload_revokes_the_port_and_its_answers_and_the_new_load_gets_a_new_one() {
        let artifacts = served(&[("artifact-1", "main", "aaa")]);
        said(
            &artifacts,
            "artifact-1",
            &format!("/.purlis/port/{LOAD_A}/open"),
        );
        let asked = said(&artifacts, "artifact-1", &read_path(LOAD_A, 1, "data.json")).unwrap();

        artifacts.reloaded("artifact-1");
        assert!(!artifacts.still_open("artifact-1", LOAD_A));
        assert!(
            artifacts.for_ask(&asked).is_none(),
            "an answer for a gone load is never read"
        );
        assert_eq!(
            said(&artifacts, "artifact-1", &read_path(LOAD_A, 2, "data.json")),
            None
        );

        said(
            &artifacts,
            "artifact-1",
            &format!("/.purlis/port/{LOAD_B}/open"),
        );
        assert!(said(&artifacts, "artifact-1", &read_path(LOAD_B, 1, "data.json")).is_some());
        assert_eq!(
            said(&artifacts, "artifact-1", &read_path(LOAD_A, 3, "data.json")),
            None
        );
    }

    #[test]
    fn a_port_is_its_own_webview_s_never_another_artifact_s() {
        let artifacts = served(&[("artifact-1", "main", "aaa"), ("artifact-2", "main", "bbb")]);
        said(
            &artifacts,
            "artifact-1",
            &format!("/.purlis/port/{LOAD_A}/open"),
        );
        // The second artifact speaks the first one's word, on its own origin and on the first's.
        let theirs = artifacts.answer(
            "artifact-2",
            &get(&at("bbb", &read_path(LOAD_A, 1, "data.json"))),
        );
        assert_eq!(theirs.ask, None);
        let crossed = artifacts.answer(
            "artifact-2",
            &get(&at("aaa", &read_path(LOAD_A, 1, "data.json"))),
        );
        assert_eq!(crossed.response.status(), StatusCode::NOT_FOUND);
        assert_eq!(crossed.ask, None);
        assert!(!artifacts.still_open("artifact-2", LOAD_A));
    }

    #[test]
    fn an_answer_is_handed_to_its_load_as_json_data_with_the_page_s_id() {
        let ask = Ask {
            label: "artifact-1".to_owned(),
            load: LOAD_A.to_owned(),
            id: 3,
            name: "data.json".to_owned(),
            params: None,
        };
        let at = chrono::DateTime::parse_from_rfc3339("2026-11-09T12:00:00Z")
            .unwrap()
            .into();
        let answer = purlis_core::catalogue::Answer::data(
            serde_json::json!({ "title": "</script><script>alert(1)</script>\u{2028}" }),
            at,
        );
        let script = delivery(&ask, &answer);
        let prefix = format!("window.{ANSWER} && window.{ANSWER}(\"{LOAD_A}\", ");
        let json = script
            .strip_prefix(&prefix)
            .unwrap()
            .strip_suffix(");")
            .unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(json).unwrap(),
            serde_json::json!({
                "id": 3,
                "data": { "title": "</script><script>alert(1)</script>\u{2028}" },
                "at": "2026-11-09T12:00:00Z",
            })
        );
    }

    /// A plane with workspace `alpha` holding artifact folder `board`, pinned.
    fn pinned_board(files: &[(&str, &str)]) -> (tempfile::TempDir, std::path::PathBuf, String) {
        let root = tempfile::tempdir().unwrap();
        let ws = root.path().join("workspaces/alpha");
        let folder = ws.join("artifacts/board");
        std::fs::create_dir_all(&folder).unwrap();
        for (path, text) in files {
            let at = folder.join(path);
            std::fs::create_dir_all(at.parent().unwrap()).unwrap();
            std::fs::write(at, text).unwrap();
        }
        let request = artifact::Request {
            folder: "board".to_owned(),
            ..artifact::Request::default()
        };
        let pin = match artifact::pin(root.path(), &ws, &request).unwrap() {
            artifact::Pinned::New(pin) => pin.id,
            other => panic!("{other:?}"),
        };
        (root, ws, pin)
    }

    #[cfg(not(windows))]
    #[test]
    fn a_read_is_gated_again_when_asked_by_the_pin_and_this_machine_s_approval() {
        let page = r#"<script type="application/json" id="purlis">{"api":1,"data":["data.json"]}</script><script>go()</script>"#;
        let (_root, ws, pin) = pinned_board(&[("index.html", page), ("data.json", r#"{"n":1}"#)]);
        let config = tempfile::tempdir().unwrap();
        let snapshot = serve::Snapshot::read(&ws.join("artifacts/board"), "board").unwrap();
        let declared = declared_by(&snapshot);
        assert_eq!(
            declared.as_ref().map(|d| d.data.clone()),
            Some(vec!["data.json".to_owned()])
        );
        let of = Of {
            plane: serde_json::from_str("\"plane\"").unwrap(),
            workspace: "alpha".to_owned(),
            dir: ws.clone(),
            folder: "board".to_owned(),
            pin: pin.clone(),
            standing: Standing::Approved,
        };
        let kind = |answer: purlis_core::catalogue::Answer| answer.outcome.map_err(|k| k.as_str());
        let ask = |config: Option<&std::path::Path>, name: &str| {
            kind(read_asked(config, &of, &snapshot, declared.as_ref(), name))
        };

        // Not approved on this machine, or no config home to hold an approval: nothing is read.
        assert_eq!(ask(Some(config.path()), "data.json"), Err("not-approved"));
        assert_eq!(ask(None, "data.json"), Err("not-approved"));

        approval::approve(config.path(), &ws, "board", &pin, snapshot.fingerprint()).unwrap();
        assert_eq!(
            ask(Some(config.path()), "data.json"),
            Ok(serde_json::json!({ "n": 1 }))
        );
        assert_eq!(ask(Some(config.path()), "index.html"), Err("not-approved"));

        // Its pin changed since it was opened: a new pin, not approved.
        artifact::unpin(_root.path(), &ws, "board").unwrap();
        assert_eq!(ask(Some(config.path()), "data.json"), Err("not-approved"));
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
