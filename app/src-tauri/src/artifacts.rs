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
//!   own webview and telling it which key ([`ARTIFACT_KEY`]). A page that could forge that
//!   request (none can yet, #5959) moves only the focus.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};

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

/// One artifact webview: whose window it is in, its origin's host, and what it is served.
struct Served {
    window: String,
    host: String,
    snapshot: serve::Snapshot,
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
            .header("Content-Security-Policy", serve::csp(&origin(&host), false))
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

/// **Opens a pinned artifact over its tab's frame**: the folder read once, a webview of its own
/// made at `place` in the asking window, and its label answered. Nothing is focused: the
/// keyboard stays where the person has it.
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
) -> Result<String, String> {
    let at = workspace_of(&planes, &plane, &workspace)?;
    let read_folder = folder.clone();
    let (host, snapshot) = blocking(move || {
        let pins = artifact::read(&at)?;
        if !pins.pins.iter().any(|pin| pin.folder == read_folder) {
            return Err(format!(
                "'{}' is not pinned to this workspace any more",
                purlis_core::personas::one_line(&read_folder)
            ));
        }
        let real = artifact::resolve(&at, &read_folder)?;
        Ok((
            serve::origin_host(&at, &read_folder),
            serve::Snapshot::read(&real)?,
        ))
    })
    .await?;
    let label = label(artifacts.made.fetch_add(1, Ordering::Relaxed) + 1);
    let own = origin(&host);
    artifacts.served().insert(
        label.clone(),
        Served {
            window: window.label().to_owned(),
            host,
            snapshot,
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
    Ok(label)
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
                },
            );
        }
        artifacts
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
