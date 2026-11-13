// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A pinned artifact, rendered in its tab** (ADR 0122 §2–3, #5952): a native webview of its
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
//! - **Esc and F6 hand the keyboard back to purlis** ([`PAGE_SCRIPT`]): purlis's own script in the page
//!   asks its own origin for `/.purlis/key/<key>`, which purlis answers by focusing the window's
//!   own webview and telling it which key ([`ARTIFACT_KEY`]). An approved page can forge that
//!   request, and it moves only the focus.
//! - **Its script runs only once the person approved this template here** (ADR 0122 §4,
//!   #5959): the CSP of each response follows the standing read when the folder was
//!   ([`artifact::approval`]). Approval is [`artifact_approve`]: window-only, of exactly the pin
//!   and the fingerprint this webview was served, never what the window says; every window of
//!   the plane hears it ([`APPROVALS_CHANGED`]) and loads its page again.
//! - **It reloads when its data changes** ([`artifact_reload`], #5955): the window hears the
//!   change from the plane's watch, the folder is read again with where it stands, and the page
//!   is loaded again from the new bytes. purlis's script in the page ([`PAGE_SCRIPT`]) puts it
//!   back where it was scrolled. A change to its template tears the page down instead (#5959).
//!   The standing is read again with the bytes, so a template that changed in the same burst is
//!   never served under the approval of the one before.
//! - **It is drawn in purlis's theme** ([`artifact_theme`], #5955): the window hands over the
//!   theme it draws, and the page is served it as a stylesheet of its own origin
//!   ([`serve::THEME_SHEET`]), which the CSP already admits: `color-scheme` purlis's appearance,
//!   not the operating system's, and each colour token a `--purlis-` custom property.
//! - **An approved page reads its own data through one port per load** (ADR 0122 §7, #5961):
//!   purlis's script in the page ([`PORT`]) hands it `purlis.port`, a `MessagePort`, and carries
//!   what it is asked to purlis on its own origin's `/.purlis/port/<load>/…`, attributed by the
//!   webview's label. A load's port opens with its first word and is revoked when the page loads
//!   again ([`Artifacts::reloaded`]); an answer for a load that is gone is never delivered. Each
//!   read is gated again when it is asked: the pin and approval read again, and only a data file
//!   the page declared ([`artifact::data`]), answered in the catalogue's envelope.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;
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

/// **purlis's script in every artifact page** (`artifact-page.js`, tested in
/// `app/src/artifactPage.test.ts`): Esc and F6 (Shift+F6 back) are purlis's keys, not the
/// page's; purlis's theme sheet is linked before anything of the page's; and the scroll is kept
/// across a reload. It asks its own origin only, which the CSP lets an image and a stylesheet do.
const PAGE_SCRIPT: &str = include_str!("artifact-page.js");

/// What purlis runs in an artifact's page when the window's theme changed: the script's own
/// function, which asks for the theme sheet again.
const THEME_CHANGED: &str = "window.__purlisTheme?.()";

/// The keys [`PAGE_SCRIPT`] reports.
const KEY_NAMES: [&str; 3] = ["escape", "f6", "f6-back"];

/// **purlis's script in every artifact page that hands it its port** (#5961): `purlis.port`, a
/// `MessagePort` made for this load. The page posts `{ id, read: "<data file>" }` or, for a
/// catalogue query it declared, `{ id, query: "<id>@<N>", params }` (#5969, [`calls`]), or the
/// actions it offers, `{ id, offers: [{ does, subject }] }` (#5972, [`offers`]), on it and
/// hears `{ id, data | error: { kind }, at, cursor? }`. `{ id, subscribe: "<id>@<N>", params }`
/// hears that answer again with its `id` each time purlis pushes a new one, until `{ id,
/// unsubscribe: true }` or the next load (#5970, [`live`]). The load is named by a random word of its own,
/// opened first and then carried on each ask, as an image of its own origin, which is all the CSP
/// lets it reach; the answer comes back through [`ANSWER`], for this load's word only.
const PORT: &str = r#"(() => {
  const load = Array.from(crypto.getRandomValues(new Uint8Array(16)), (b) => b.toString(16).padStart(2, "0")).join("");
  const channel = new MessageChannel();
  const own = channel.port1;
  const send = (path) => { new Image().src = "/.purlis/port/" + load + "/" + path; };
  let open = false;
  let offered = 0;
  const waiting = [];
  const opening = new Image();
  opening.onload = opening.onerror = () => { open = true; waiting.splice(0).forEach(send); };
  opening.src = "/.purlis/port/" + load + "/open";
  own.onmessage = (event) => {
    const asked = event.data;
    const id = asked !== null && typeof asked === "object" ? asked.id : undefined;
    const refuse = () => own.postMessage({ id: Number.isSafeInteger(id) ? id : null, error: { kind: "refused" }, at: new Date().toISOString().replace(/\.\d+Z$/, "Z") });
    if (!Number.isSafeInteger(id) || id < 0 || (typeof asked.read !== "string" && typeof asked.query !== "string" && !Array.isArray(asked.offers) && typeof asked.subscribe !== "string" && asked.unsubscribe !== true)) {
      refuse();
      return;
    }
    const call = (word, query) => word + "/" + id + "?" + encodeURIComponent(JSON.stringify({ query, params: asked.params === undefined ? null : asked.params }));
    let path;
    try {
      path = Array.isArray(asked.offers)
        ? "offers/" + id + "/" + (offered += 1) + "?" + encodeURIComponent(JSON.stringify({ offers: asked.offers }))
        : typeof asked.query === "string" ? call("query", asked.query)
        : typeof asked.subscribe === "string" ? call("subscribe", asked.subscribe)
        : asked.unsubscribe === true ? "unsubscribe/" + id
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
    /// A subscription's subscribe or unsubscribe (#5970), not a read or a call.
    pub(crate) live: Option<live::Said>,
}

pub(crate) mod calls;
pub(crate) mod history;
pub(crate) mod live;
pub(crate) mod offers;
pub(crate) mod queried;

/// What every window of a plane hears when an artifact's approval on this machine changed: the
/// workspace and folder, so each tab of it loads its page again.
pub(crate) const APPROVALS_CHANGED: &str = "artifact-approvals-changed";

/// What [`APPROVALS_CHANGED`] carries.
#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct ArtifactApprovalChanged {
    pub workspace: String,
    pub folder: String,
}

/// One artifact webview: whose window it is in, its origin's host, what it is served, which pin
/// of which folder that is, standing how, and the theme sheet it is drawn in.
struct Served {
    window: String,
    host: String,
    snapshot: serve::Snapshot,
    of: Of,
    theme: String,
    /// What its page declared it reads, read from the bytes served.
    declared: Option<data::Declared>,
    /// The word of the load whose port is open, if one is.
    load: Option<String>,
    /// The number of the newest set of offers taken from the open load (#6232): an older set,
    /// gated after it, is not drawn over it.
    offered: u64,
}

/// The artifact a webview shows, as it was read when it was opened, or last read again.
#[derive(Clone)]
struct Of {
    plane: PlaneId,
    workspace: String,
    dir: std::path::PathBuf,
    folder: String,
    pin: String,
    standing: Standing,
    /// Shown as of a version (#5966): its data reads come from what was kept then, and it
    /// calls, subscribes to and offers nothing.
    then: Option<history::AsOf>,
}

/// Every artifact webview this run has open, by label.
#[derive(Default)]
pub(crate) struct Artifacts {
    served: Mutex<HashMap<String, Served>>,
    made: AtomicU64,
    /// The pages' subscriptions to catalogue queries (#5970).
    live: Mutex<live::Live>,
}

/// What the scheme answered, and the key to hand back to a window, if it was one.
pub(crate) struct Answered {
    pub(crate) response: Response<Vec<u8>>,
    /// `(window, key)`: the page pressed one of purlis's keys.
    pub(crate) key: Option<(String, String)>,
    /// A read the page asked for on its open port.
    pub(crate) ask: Option<Ask>,
    /// Whether the request came from a page purlis serves, at the origin it serves it from
    /// now: only then may its link reports and offers be heard (#6120).
    pub(crate) matched: bool,
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
            self.offered = 0;
            return None;
        }
        if self.load.as_deref() != Some(load) {
            return None;
        }
        if let Some(ask) = live::said(label, load, said, query) {
            return ask;
        }
        if let Some(id) = said.strip_prefix("query/") {
            let (name, params) = calls::asked(query?)?;
            return Some(Ask {
                label: label.to_owned(),
                load: load.to_owned(),
                id: page_id(id)?,
                name,
                params: Some(params),
                live: None,
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
            live: None,
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
        let matched = own.is_some();
        let mut key = None;
        let mut ask = None;
        let runs = own.as_ref().is_some_and(|one| one.of.standing.runs());
        let (status, body, kind) = match own {
            None => (StatusCode::NOT_FOUND, Vec::new(), "text/plain"),
            Some(one) => {
                if path == serve::THEME_SHEET {
                    let sheet = one.theme.clone().into_bytes();
                    (
                        StatusCode::OK,
                        sheet,
                        serve::content_type(serve::THEME_SHEET),
                    )
                } else if let Some(rest) = path.strip_prefix(serve::OWN_PATH) {
                    let pressed = rest.strip_prefix("key/").filter(|k| KEY_NAMES.contains(k));
                    key = pressed.map(|k| (one.window.clone(), k.to_owned()));
                    if let Some(port) = rest.strip_prefix("port/") {
                        ask = one.port(label, port, uri.query());
                    }
                    (StatusCode::NO_CONTENT, Vec::new(), "text/plain")
                } else {
                    match serve::key(path)
                        .and_then(|key| Some((one.snapshot.served(&key)?.to_vec(), key)))
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
        Answered {
            response,
            key,
            ask,
            matched,
        }
    }

    /// **Webview `label`'s page loads again**: the port its last load opened is revoked, so
    /// nothing it asked is answered and no answer for it is delivered.
    pub(crate) fn reloaded(&self, label: &str) {
        if let Some(one) = self.served().get_mut(label) {
            one.load = None;
            one.offered = 0;
        }
        self.unsubscribe_label(label);
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

    /// Whether artifact webview `label` may go to `url`: within its own origin only, and never
    /// to a data file ([`serve::is_data`]), which is no document of the template.
    pub(crate) fn may_navigate(&self, label: &str, url: &tauri::Url) -> bool {
        let data = serve::key(url.path()).is_some_and(|key| serve::is_data(&key));
        !data
            && self
                .served()
                .get(label)
                .is_some_and(|one| url.as_str().starts_with(&format!("{}/", origin(&one.host))))
    }

    /// **The link `href` a person followed in artifact webview `label`'s page** (#5956): its
    /// window, its origin, and whether the template it was served writes that link.
    pub(crate) fn followed(
        &self,
        label: &str,
        href: &str,
    ) -> Option<crate::artifact_links::Followed> {
        self.served()
            .get(label)
            .map(|one| crate::artifact_links::Followed {
                window: one.window.clone(),
                plane: one.of.plane.clone(),
                own: origin(&one.host),
                href: href.to_owned(),
                written: artifact::links::written(&one.snapshot, href),
            })
    }

    /// The artifact webview `label`, when it is in window `window`.
    fn owned_by(&self, label: &str, window: &str) -> bool {
        self.served()
            .get(label)
            .is_some_and(|one| one.window == window)
    }

    /// **Reads artifact webview `label`'s folder again** (in window `window`), and where it
    /// stands with it (`config` is this machine's config home), so its next load is served the
    /// bytes on disk now under the CSP their standing allows; and whether anything it is served
    /// moved. Nothing changes when it cannot be read: the page keeps what it was served, and the
    /// bar says why from the pins.
    fn read_again(
        &self,
        label: &str,
        window: &str,
        config: Option<&Path>,
    ) -> Result<(Standing, bool, bool), String> {
        let of = self
            .served()
            .get(label)
            .filter(|one| one.window == window)
            .map(|one| one.of.clone())
            .ok_or_else(|| "the artifact is not open in this window".to_owned())?;
        if of.then.is_some() {
            return Err("an artifact shown as of a version is not read again".to_owned());
        }
        let (pin, snapshot) = read_pinned(&of.dir, &of.folder)?;
        // An approval given or gone since is an origin of its own ([`approval::page_host`]):
        // the page's next load is from there, and the origin before it is served nothing. Both
        // from one read of the record, so they agree (#6120).
        let (standing, host) =
            approval::standing_and_host(config, &of.dir, &of.folder, &pin, &snapshot, "");
        // What the page declares is read again with its bytes (#5961), and so is whether that
        // data was generated here.
        let declared = declared_by(&snapshot);
        let generated =
            data::generated(&artifact::resolve(&of.dir, &of.folder)?, declared.as_ref());
        let mut moved = true;
        if let Some(one) = self.served().get_mut(label) {
            // Whether anything it is served differs: a burst the platform could not name tells
            // every tab its data may have changed (felt speed round 2).
            moved = one.snapshot != snapshot
                || one.of.pin != pin
                || one.of.standing != standing
                || one.host != host;
            one.snapshot = snapshot;
            one.declared = declared;
            one.of.pin = pin;
            one.of.standing = standing;
            one.host = host;
        }
        Ok((standing, generated, moved))
    }

    /// **Draws artifact webview `label` (in window `window`) in `sheet`**: whether it changed.
    fn draw_in(&self, label: &str, window: &str, sheet: String) -> bool {
        let mut served = self.served();
        match served.get_mut(label).filter(|one| one.window == window) {
            Some(one) if one.theme != sheet => {
                one.theme = sheet;
                true
            }
            _ => false,
        }
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
        drop(served);
        for label in &gone {
            self.unsubscribe_label(label);
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
        if ask.live.is_some() {
            live::asked(app.clone(), ask);
        } else {
            answer_ask(app.clone(), ask);
        }
    }
    // A request no page purlis serves now matched (a document left at an origin its page has
    // moved from) offers nothing and reports no link (#6120).
    if answered.matched {
        // The actions the page offers (#5972): drawn in purlis's bar, never done here.
        offers::heard(app, label, request.uri().path(), request.uri().query());
    }
    // A link the person pressed in the page (#5956), attributed by the webview's label, and
    // followed only with the one token purlis handed its open load (ADR 0122 §5).
    if let Some(links) = app.try_state::<crate::artifact_links::Links>() {
        use crate::artifact_links::{Heard, delivery, follow};
        let heard = if answered.matched {
            links.heard(label, &request.uri().to_string(), std::time::Instant::now())
        } else {
            Heard::Nothing
        };
        let hand = |load: &str, token: &str| {
            if let Some(webview) = app.get_webview(label) {
                let _ = webview.eval(delivery(load, token));
            }
        };
        match heard {
            Heard::Nothing => {}
            Heard::Hand { load, token } => hand(&load, &token),
            Heard::Follow { href, load, token } => {
                hand(&load, &token);
                if let Some(followed) = artifacts.followed(label, &href) {
                    follow(app, label, followed);
                }
            }
        }
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
    if !approved_now(config, of, snapshot) {
        return Answer::error(ErrorKind::NotApproved, now);
    }
    if let Some(as_of) = &of.then {
        return history::read_then(as_of, declared, name, now);
    }
    match artifact::resolve(&of.dir, &of.folder) {
        Ok(folder) => data::read(&folder, declared, name, now),
        Err(_) => Answer::error(ErrorKind::Failed, now),
    }
}

/// **Whether the page served as `snapshot` may still call, subscribe and offer**: it is shown as
/// it is now, not as of a version (#5966), and [`approved_now`].
fn still_runs(config: Option<&std::path::Path>, of: &Of, snapshot: &serve::Snapshot) -> bool {
    of.then.is_none() && approved_now(config, of, snapshot)
}

/// **Whether the page served as `snapshot` still runs its script**, read again at the ask: its
/// pin still the one served, and its standing on this machine still approved.
fn approved_now(config: Option<&std::path::Path>, of: &Of, snapshot: &serve::Snapshot) -> bool {
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
    let links = app.try_state::<crate::artifact_links::Links>();
    for label in artifacts.forget_window(window) {
        if let Some(links) = &links {
            links.forget(&label);
        }
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

/// **The theme the window draws, handed to an artifact's page** (#5955): its appearance
/// (`dark` or `light`) and its colour tokens' values, by token (`surface.base`).
#[derive(Debug, Clone, Deserialize, specta::Type)]
pub struct ArtifactTheme {
    pub appearance: String,
    pub values: BTreeMap<String, String>,
}

/// **When an artifact's data was last updated** (#5955, `purlis_core::artifact::fresh`).
#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct ArtifactFresh {
    /// Milliseconds since 1970.
    pub at: f64,
    /// When it is drawn stale, in milliseconds since 1970.
    pub stale_at: f64,
    /// Where the time was read: `declared` by the data, `committed` or `written`.
    pub source: String,
}

impl ArtifactFresh {
    fn of(folder: &Path) -> Option<Self> {
        let fresh = artifact::fresh::of(folder)?;
        let ms = |seconds: i64| seconds as f64 * 1000.0;
        Some(Self {
            at: ms(fresh.at),
            stale_at: ms(fresh.stale_at()),
            source: match fresh.source {
                artifact::fresh::Source::Declared => "declared",
                artifact::fresh::Source::Committed => "committed",
                artifact::fresh::Source::Written => "written",
            }
            .to_owned(),
        })
    }

    /// The freshness of pinned folder `folder` of `workspace`, when it resolves.
    fn of_pin(workspace: &Path, folder: &str) -> Option<Self> {
        Self::of(&artifact::resolve(workspace, folder).ok()?)
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
    /// Its folder is over the size cap: the sentence naming both sizes.
    pub too_large: Option<String>,
    /// When its data was last updated, when that can be read.
    pub fresh: Option<ArtifactFresh>,
    /// The number its icon shows, as its data declares it (#5965); none when it declares none.
    #[specta(optional)]
    pub badge: Option<u32>,
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
    /// When its data was last updated, when that can be read.
    pub fresh: Option<ArtifactFresh>,
}

/// **Pinned folder `folder` of `workspace`, read**: its pin's id and its files; refused once it
/// is pinned no more.
fn read_pinned(workspace: &Path, folder: &str) -> Result<(String, serve::Snapshot), String> {
    let pin = artifact::pinned(workspace, folder)?;
    let snapshot = serve::Snapshot::read(&artifact::resolve(workspace, folder)?, folder)?;
    Ok((pin.id, snapshot))
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
                fresh: listed
                    .missing
                    .is_none()
                    .then(|| ArtifactFresh::of_pin(&at, &listed.pin.folder))
                    .flatten(),
                badge: listed
                    .missing
                    .is_none()
                    .then(|| artifact::badge::of_pin(&at, &listed.pin.folder))
                    .flatten(),
                id: listed.pin.id,
                folder: listed.pin.folder,
                name: listed.pin.name,
                icon: listed.pin.icon,
                missing: listed.missing,
                live,
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
            return Err(artifact::not_pinned(&folder));
        };
        Ok(ArtifactAbout {
            fresh: listed
                .missing
                .is_none()
                .then(|| ArtifactFresh::of_pin(&at, &folder))
                .flatten(),
            pinned_by: artifact::pinned_by(&at, &listed.pin.id),
            name: listed.pin.name,
            missing: listed.missing,
            too_large: listed.too_large,
        })
    })
    .await
}

/// **Repin** ([`artifact::repin`]): pins artifact folder `folder` again as it is pinned, its name,
/// icon, command and working folder kept, so the folder is checked anew. The way out of a missing
/// or too-large artifact once its folder is back or smaller; the refusal's sentence when it is
/// not. Every window hears that the pins moved, as it does of an unpin.
#[tauri::command]
#[specta::specta]
pub async fn artifact_repin(
    app: tauri::AppHandle,
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: String,
    folder: String,
) -> Result<(), String> {
    repin_and_tell(&app, &planes, plane, workspace, folder).await
}

/// [`artifact_repin`]'s work, on any runtime, so a test on the mock one hears what it tells.
async fn repin_and_tell<R: Runtime>(
    app: &tauri::AppHandle<R>,
    planes: &Planes,
    plane: PlaneId,
    workspace: String,
    folder: String,
) -> Result<(), String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    let at = workspace_of(planes, &plane, &workspace)?;
    blocking(move || artifact::repin(&root, &at, &folder).map(|_| ())).await?;
    moved(app, plane, workspace);
    Ok(())
}

/// **Unpin**: takes artifact folder `folder`'s pin off the workspace; its files stay. A missing
/// or too-large artifact's tab asks it (#5954), and so does Unpin from workspace in a pin's menu
/// on the activity bar (#5953); every window hears that the pins moved.
#[tauri::command]
#[specta::specta]
pub async fn artifact_unpin(
    app: tauri::AppHandle,
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    workspace: String,
    folder: String,
) -> Result<(), String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    let at = workspace_of(&planes, &plane, &workspace)?;
    let config = purlis_core::machine::config_root();
    // Unpinned here, its approval on this machine goes with it, and so does the origin its
    // storage was kept in (ADR 0122, amended 2026-11-10).
    let kept = blocking(move || {
        artifact::unpin_here(&root, &at, &folder, config.as_deref()).map(|(_, kept)| kept)
    })
    .await?;
    if let Some(why) = kept {
        tracing::warn!("purlis: unpinned an artifact, but {why}");
    }
    moved(&app, plane, workspace);
    Ok(())
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
// A command's arguments are what the window passes by name: the bindings carry each one, and
// the two states Tauri hands in are not the window's.
#[allow(clippy::too_many_arguments)]
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
    theme: ArtifactTheme,
) -> Result<ArtifactOpened, String> {
    let at = workspace_of(&planes, &plane, &workspace)?;
    let sheet = serve::theme_sheet(&theme.appearance, &theme.values)?;
    let dir = at.clone();
    let read_folder = folder.clone();
    let config = purlis_core::machine::config_root();
    let (host, snapshot, pin, standing, changed_by, declared, generated, plane_granted) =
        blocking(move || {
            let real = artifact::resolve(&at, &read_folder)?;
            let (pin, snapshot) = read_pinned(&at, &read_folder)?;
            // The approval's own origin when its script runs here, so its storage is that
            // approval's alone (ADR 0122, amended 2026-11-10); from the one read of the record
            // its standing is, so the two agree (#6120).
            let (standing, host) = approval::standing_and_host(
                config.as_deref(),
                &at,
                &read_folder,
                &pin,
                &snapshot,
                "",
            );
            let changed_by = (standing == Standing::PinChanged)
                .then(|| artifact::pinned_by(&at, &pin))
                .flatten();
            let declared = declared_by(&snapshot);
            let generated = data::generated(&real, declared.as_ref());
            let plane_granted =
                approval::plane_granted(config.as_deref(), &at, &read_folder, &pin, &snapshot);
            Ok((
                host,
                snapshot,
                pin,
                standing,
                changed_by,
                declared,
                generated,
                plane_granted,
            ))
        })
        .await?;
    let queries = calls::listed(declared.as_ref());
    let of = Of {
        plane,
        workspace,
        dir,
        folder,
        pin,
        standing,
        then: None,
    };
    let served = Served {
        window: window.label().to_owned(),
        host,
        snapshot,
        of,
        theme: sheet,
        declared,
        load: None,
        offered: 0,
    };
    let label = lay(&window, &artifacts, served, place)?;
    Ok(ArtifactOpened {
        label,
        standing: standing.as_str().to_owned(),
        changed_by,
        generated,
        queries,
        plane_granted,
    })
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
        too_large: None,
        // Read with the pins again: every window hears they moved.
        fresh: None,
        badge: None,
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
    theme: ArtifactTheme,
) -> Result<ArtifactOpened, String> {
    let sheet = serve::theme_sheet(&theme.appearance, &theme.values)?;
    let root = planes.held(&plane)?.root().to_path_buf();
    let dir = root.clone();
    let workspace = page.workspace.clone();
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
    // No pin and no folder: rendered script-free whatever it holds, and asking nothing, so no
    // press in any bar can approve a preview's script.
    let of = Of {
        plane,
        workspace,
        dir,
        folder: String::new(),
        pin: String::new(),
        standing: Standing::NotHere,
        then: None,
    };
    let served = Served {
        window: window.label().to_owned(),
        host,
        snapshot,
        of,
        theme: sheet,
        declared: None,
        load: None,
        offered: 0,
    };
    Ok(ArtifactOpened {
        label: lay(&window, &artifacts, served, place)?,
        standing: Standing::NotHere.as_str().to_owned(),
        changed_by: None,
        // A preview declares no data of its own: there is nothing to have generated.
        generated: true,
        // Nor does it run script, so it calls nothing.
        queries: Vec::new(),
        plane_granted: false,
    })
}

/// Makes the webview of what `served` serves, at `place` in `window`: its label. `served`
/// names `window` as the one it is served to.
fn lay(
    window: &tauri::Window,
    artifacts: &Artifacts,
    served: Served,
    place: ArtifactPlace,
) -> Result<String, String> {
    let label = label(artifacts.made.fetch_add(1, Ordering::Relaxed) + 1);
    let own = origin(&served.host);
    artifacts.served().insert(label.clone(), served);
    let url: tauri::Url = format!("{own}/")
        .parse()
        .map_err(|e| format!("purlis could not name the artifact's page: {e}"))?;
    // Its origin moves when an approval is given or goes ([`artifact_reload`]), so where it may
    // go is asked of what it is served now, never fixed here.
    let app = window.app_handle().clone();
    let going = label.clone();
    let builder =
        tauri::webview::WebviewBuilder::new(&label, tauri::WebviewUrl::CustomProtocol(url))
            .on_navigation(move |to| {
                app.try_state::<Artifacts>()
                    .is_some_and(|artifacts| artifacts.may_navigate(&going, to))
            })
            .on_new_window(crate::navguard::no_new_window)
            .initialization_script(PAGE_SCRIPT)
            .initialization_script(PORT)
            .initialization_script(crate::artifact_links::SCRIPT)
            // Never incognito: on Linux wry gives an incognito webview an ephemeral WebKitGTK
            // context that the artifact scheme never reaches, so its page is served nothing. Its
            // storage is kept apart by its own origin ([`origin`]), as the probe proved.
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

/// **The person's approval of an artifact's script, pressed in its tab's bar** (ADR 0122 §4):
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
    if of.then.is_some() {
        return Err(
            "an artifact shown as of a version cannot be approved: go back to now to approve it"
                .into(),
        );
    }
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
        None => webview.hide().map_err(page_unreached),
        Some(place) => {
            let (position, size) = place.physical();
            webview.set_position(position).map_err(page_unreached)?;
            webview.set_size(size).map_err(page_unreached)?;
            webview.show().map_err(page_unreached)
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
        Some(webview) => webview.set_focus().map_err(page_unreached),
        None => Ok(()),
    }
}

/// **Loads an artifact's page again from its folder as it is now** (#5955): the window heard
/// that its data changed. The folder is read again with where it stands on this machine, which
/// is answered for the bar, and the page is put back where it was scrolled. A folder that cannot
/// be read leaves the page as it was, and says why.
#[tauri::command]
#[specta::specta]
pub async fn artifact_reload(
    window: tauri::Window,
    artifacts: tauri::State<'_, Artifacts>,
    label: String,
) -> Result<ArtifactOpened, String> {
    let Some(webview) = child(&window, &artifacts, &label) else {
        return Err("that artifact is not open in this window any more".into());
    };
    let owner = window.label().to_owned();
    let app = window.app_handle().clone();
    let read_label = label.clone();
    let config = purlis_core::machine::config_root();
    let (standing, changed_by, generated, (queries, plane_granted), moved, same) =
        blocking(move || {
            let artifacts = app
                .try_state::<Artifacts>()
                .ok_or_else(|| "no artifact is open".to_owned())?;
            let host = |artifacts: &Artifacts| {
                artifacts
                    .served()
                    .get(&read_label)
                    .map(|one| one.host.clone())
            };
            let before = host(&artifacts);
            let (standing, generated, changed) =
                artifacts.read_again(&read_label, &owner, config.as_deref())?;
            let now = host(&artifacts);
            let moved = now.filter(|now| before.as_ref() != Some(now));
            let changed_by = (standing == Standing::PinChanged)
                .then(|| artifacts.served_to(&read_label, &owner))
                .flatten()
                .and_then(|(of, _)| artifact::pinned_by(&of.dir, &of.pin));
            let queries = artifacts.queries_of(&read_label, config.as_deref());
            Ok((standing, changed_by, generated, queries, moved, !changed))
        })
        .await?;
    match moved {
        // Nothing it is served moved (a rescan told every tab its data may have): the page is
        // left as it is, where it was scrolled and what it drew (felt speed round 2).
        None if same => {}
        // A new origin is a new page, loaded there: a reload would ask the old one again.
        Some(host) => {
            let url: tauri::Url = format!("{}/", origin(&host))
                .parse()
                .map_err(|e| format!("purlis could not name the artifact's page: {e}"))?;
            webview.navigate(url).map_err(page_unreached)?;
        }
        None => webview.reload().map_err(page_unreached)?,
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

/// **Draws an artifact's page in the theme the window draws now** (#5955): its sheet is
/// written again, and the page asks for it again when it changed.
#[tauri::command]
#[specta::specta]
pub async fn artifact_theme(
    window: tauri::Window,
    artifacts: tauri::State<'_, Artifacts>,
    label: String,
    theme: ArtifactTheme,
) -> Result<(), String> {
    let sheet = serve::theme_sheet(&theme.appearance, &theme.values)?;
    let Some(webview) = child(&window, &artifacts, &label) else {
        return Ok(());
    };
    if artifacts.draw_in(&label, window.label(), sheet) {
        webview.eval(THEME_CHANGED).map_err(page_unreached)?;
    }
    Ok(())
}

/// **What the window is told when the platform webview refuses a call on an artifact's
/// page**: purlis's own sentence with what to do, never Tauri's words, which go to the log.
fn page_unreached(error: tauri::Error) -> String {
    tracing::warn!("purlis: an artifact's webview refused a call: {error}");
    "purlis could not reach this artifact's page. Close its tab and open it again.".to_owned()
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
    artifacts.unsubscribe_label(&label);
    if let Some(links) = window.try_state::<crate::artifact_links::Links>() {
        links.forget(&label);
    }
    if let Some(webview) = webview {
        webview.close().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// On Linux an incognito webview is served nothing at all by a custom scheme: WebKitGTK's
    /// ephemeral context that wry makes for it never reaches purlis's handler, so the page never
    /// loads and its script never reaches its port (the probe's `shape` run, CI 42769379065).
    /// Tauri's builder cannot be read back, so this reads the source that builds it.
    #[test]
    fn an_artifact_webview_is_never_incognito_because_webkitgtk_serves_it_nothing() {
        let source = include_str!("artifacts.rs");
        let builds = source
            .split("fn lay(")
            .nth(1)
            .and_then(|rest| rest.split("\n}\n").next())
            .expect("lay builds the artifact's webview");
        assert!(
            builds.contains("WebviewBuilder::new") && !builds.contains(".incognito("),
            "an artifact's webview is built incognito"
        );
    }

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
                    theme: format!(":root {{ color-scheme: dark; }} /* {host} */"),
                    declared: None,
                    load: None,
                    offered: 0,
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
            then: None,
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

    /// #6120: once a page's origin moves (an approval given or gone), the document still
    /// loaded at the old origin is answered nothing, and nor are its link reports or its
    /// offers heard: only a request matched to the page purlis serves now is.
    #[test]
    fn a_request_from_an_origin_the_page_has_left_is_heard_by_nothing() {
        let artifacts = served(&[("artifact-1", "main", "aaa")]);
        for path in [
            "/.purlis/link/open/0123456789abcdef",
            "/.purlis/offer/x",
            "/",
        ] {
            let left = artifacts.answer("artifact-1", &get(&at("bbb", path)));
            assert!(!left.matched, "{path}");
            assert!(
                artifacts
                    .answer("artifact-1", &get(&at("aaa", path)))
                    .matched,
                "{path}"
            );
        }
        assert!(
            !artifacts
                .answer("artifact-9", &get(&at("aaa", "/")))
                .matched
        );
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
        let host = host_now(&artifacts);
        let csp = |artifacts: &Artifacts| {
            artifacts
                .answer("artifact-1", &get(&at(&host, "/")))
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
        assert_eq!(csp(&artifacts), serve::csp(&origin(&host), true));
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

    /// The exploit of the review of #5978: data is outside the approval, so a page planted
    /// there (`git add -f data/details.html`, or a Regenerate writing it) must never load as a
    /// document of an approved template, where its script would run with the page's port.
    #[test]
    fn a_data_file_is_never_served_or_navigated_to_even_when_the_template_is_approved() {
        let artifacts = served(&[("artifact-1", "main", "aaa")]);
        let planted = "<script>parent.purlis.port.read('x')</script>";
        artifacts.served().get_mut("artifact-1").unwrap().snapshot = snapshot(&[
            (
                "index.html",
                "<a href='data/details.html'>details</a><script>1</script>",
            ),
            ("data/details.html", planted),
            ("data/chart.svg", "<svg onload='x()'/>"),
            ("data.json", "{}"),
        ]);
        standing_of(&artifacts, "artifact-1", Standing::Approved);
        let url = |text: &str| tauri::Url::parse(text).unwrap();
        for path in [
            "/data/details.html",
            "/data/chart.svg",
            "/data%2fdetails.html",
            "/data.json",
        ] {
            let answered = artifacts.answer("artifact-1", &get(&at("aaa", path)));
            assert_eq!(answered.response.status(), StatusCode::NOT_FOUND, "{path}");
            assert!(answered.response.body().is_empty(), "{path}");
            assert!(
                !artifacts.may_navigate("artifact-1", &url(&at("aaa", path))),
                "{path}"
            );
        }
        // The template itself is still served, and still runs.
        let page = artifacts.answer("artifact-1", &get(&at("aaa", "/")));
        assert_eq!(page.response.status(), StatusCode::OK);
        assert!(artifacts.may_navigate("artifact-1", &url(&at("aaa", "/index.html"))));
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
                live: None,
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
            live: None,
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
            then: None,
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
    fn the_theme_is_served_as_a_stylesheet_of_the_page_s_own_origin_under_the_csp() {
        let artifacts = served(&[("artifact-1", "main", "aaa"), ("artifact-2", "main", "bbb")]);
        let answered = artifacts.answer("artifact-1", &get(&at("aaa", "/.purlis/theme.css?17")));
        let response = answered.response;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers()["Content-Type"],
            "text/css; charset=utf-8"
        );
        assert_eq!(
            response.headers()["Content-Security-Policy"],
            serve::csp(&origin("aaa"), false).as_str()
        );
        assert_eq!(
            String::from_utf8_lossy(response.body()),
            ":root { color-scheme: dark; } /* aaa */"
        );
        let crossed = artifacts.answer("artifact-1", &get(&at("bbb", "/.purlis/theme.css")));
        assert_eq!(crossed.response.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn a_new_theme_is_drawn_once_and_only_by_the_artifact_s_own_window() {
        let artifacts = served(&[("artifact-1", "main", "aaa")]);
        assert!(!artifacts.draw_in("artifact-1", "window-2", "light".to_owned()));
        assert!(artifacts.draw_in("artifact-1", "main", "light".to_owned()));
        assert!(
            !artifacts.draw_in("artifact-1", "main", "light".to_owned()),
            "unchanged"
        );
        let sheet = artifacts.answer("artifact-1", &get(&at("aaa", "/.purlis/theme.css")));
        assert_eq!(sheet.response.body(), b"light");
    }

    /// A workspace with artifact `page` pinned, its page saying `said`.
    fn workspace(said: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("artifacts/page");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("index.html"), said).unwrap();
        std::fs::write(
            dir.path().join("artifacts/pins.toml"),
            "version = 1\npin = [\n  { id = \"a00000000001\", folder = \"page\", name = \"Page\", icon = \"chart\", order = 1 },\n]\n",
        )
        .unwrap();
        dir
    }

    fn opened(workspace: &Path) -> Artifacts {
        let artifacts = Artifacts::default();
        let (pin, snapshot) = read_pinned(workspace, "page").unwrap();
        // The host it is served from unapproved, as `artifact_open` draws it.
        let host = approval::page_host(None, workspace, "page", &pin, &snapshot, "");
        artifacts.served().insert(
            "artifact-1".to_owned(),
            Served {
                window: "main".to_owned(),
                host,
                snapshot,
                of: Of {
                    dir: workspace.to_path_buf(),
                    folder: "page".to_owned(),
                    pin,
                    ..of(Standing::ScriptFree)
                },
                theme: String::new(),
                declared: None,
                load: None,
                offered: 0,
            },
        );
        artifacts
    }

    /// The host webview `artifact-1` is served from now.
    fn host_now(artifacts: &Artifacts) -> String {
        artifacts.served()["artifact-1"].host.clone()
    }

    fn served_page(artifacts: &Artifacts) -> Vec<u8> {
        artifacts
            .answer("artifact-1", &get(&at(&host_now(artifacts), "/")))
            .response
            .into_body()
    }

    #[test]
    fn a_reload_serves_the_folder_as_it_is_now_and_not_as_it_was_at_the_press() {
        let dir = workspace("<h1>12 open</h1>");
        let artifacts = opened(dir.path());
        std::fs::write(
            dir.path().join("artifacts/page/index.html"),
            "<h1>9 open</h1>",
        )
        .unwrap();
        std::fs::write(dir.path().join("artifacts/page/style.css"), "h1 {}").unwrap();
        std::fs::write(dir.path().join("artifacts/page/data.json"), "{}").unwrap();
        assert_eq!(
            served_page(&artifacts),
            b"<h1>12 open</h1>",
            "until it is read again"
        );

        artifacts.read_again("artifact-1", "main", None).unwrap();

        assert_eq!(served_page(&artifacts), b"<h1>9 open</h1>");
        let host = host_now(&artifacts);
        let style = artifacts.answer("artifact-1", &get(&at(&host, "/style.css")));
        assert_eq!(style.response.status(), StatusCode::OK);
        // Its data is read on its port, never served as a file of the folder.
        let data = artifacts.answer("artifact-1", &get(&at(&host, "/data.json")));
        assert_eq!(data.response.status(), StatusCode::NOT_FOUND);
    }

    /// Felt speed round 2: a burst the platform could not name tells every artifact tab its
    /// data may have changed, and the page is loaded again only when something it is served did.
    #[test]
    fn a_reload_over_a_folder_nobody_changed_moves_nothing_and_one_after_a_data_write_does() {
        let dir = workspace("<h1>12 open</h1>");
        let artifacts = opened(dir.path());
        let moved =
            |artifacts: &Artifacts| artifacts.read_again("artifact-1", "main", None).unwrap().2;
        assert!(
            !moved(&artifacts),
            "nothing on disk changed since it was opened"
        );
        std::fs::write(dir.path().join("artifacts/page/data.json"), "{\"open\": 9}").unwrap();
        assert!(moved(&artifacts), "its data changed");
        assert!(!moved(&artifacts), "and was served at the reload before");
    }

    #[test]
    fn a_reload_that_cannot_read_the_folder_keeps_what_the_page_was_served() {
        let dir = workspace("<h1>12 open</h1>");
        let artifacts = opened(dir.path());
        // Another window's reload of it reads nothing.
        std::fs::write(
            dir.path().join("artifacts/page/index.html"),
            "<h1>9 open</h1>",
        )
        .unwrap();
        assert!(
            artifacts
                .read_again("artifact-1", "window-2", None)
                .is_err()
        );
        assert_eq!(served_page(&artifacts), b"<h1>12 open</h1>");
        // Unpinned since.
        std::fs::write(dir.path().join("artifacts/pins.toml"), "version = 1\n").unwrap();
        let why = artifacts
            .read_again("artifact-1", "main", None)
            .unwrap_err();
        assert!(why.contains("not pinned"), "{why}");
        assert_eq!(served_page(&artifacts), b"<h1>12 open</h1>");
    }

    #[test]
    fn a_reload_reads_where_the_page_stands_again_so_a_changed_template_runs_no_script() {
        let dir = workspace("<h1>12 open</h1><script>draw()</script>");
        let artifacts = opened(dir.path());
        standing_of(&artifacts, "artifact-1", Standing::Approved);
        let host = host_now(&artifacts);
        let csp = |artifacts: &Artifacts| {
            artifacts
                .answer("artifact-1", &get(&at(&host, "/")))
                .response
                .headers()["Content-Security-Policy"]
                .to_str()
                .unwrap()
                .to_owned()
        };
        assert_eq!(csp(&artifacts), serve::csp(&origin(&host), true));

        // Its data and its template both moved in one burst: the template is no longer the
        // one approved, so the reload serves it with no script.
        std::fs::write(dir.path().join("artifacts/page/data.json"), "{}").unwrap();
        std::fs::write(
            dir.path().join("artifacts/page/index.html"),
            "<h1>9 open</h1><script>steal()</script>",
        )
        .unwrap();
        let (standing, _, _) = artifacts.read_again("artifact-1", "main", None).unwrap();

        assert!(!standing.runs(), "{standing:?}");
        assert_eq!(
            host_now(&artifacts),
            host,
            "not approved, it keeps its folder's origin"
        );
        assert_eq!(csp(&artifacts), serve::csp(&origin(&host), false));
    }

    /// Finding 1 of the second review of #5978: storage is the origin's, so an approval is
    /// served from an origin of its own. A reload after the person approved the page moves it
    /// there, and the origin it was served from before is served nothing.
    #[test]
    fn a_reload_after_an_approval_serves_the_page_from_the_approval_s_own_origin() {
        let dir = workspace("<h1>12 open</h1><script>draw()</script>");
        let config = tempfile::tempdir().unwrap();
        let artifacts = opened(dir.path());
        let host = |artifacts: &Artifacts| artifacts.served()["artifact-1"].host.clone();
        let before = host(&artifacts);
        let (pin, snapshot) = read_pinned(dir.path(), "page").unwrap();
        approval::approve(
            config.path(),
            dir.path(),
            "page",
            &pin,
            snapshot.fingerprint(),
        )
        .unwrap();

        let (standing, _, _) = artifacts
            .read_again("artifact-1", "main", Some(config.path()))
            .unwrap();

        assert_eq!(standing, Standing::Approved);
        let approved =
            approval::page_host(Some(config.path()), dir.path(), "page", &pin, &snapshot, "");
        assert_eq!(host(&artifacts), approved);
        assert_ne!(approved, before);
        let old = artifacts.answer("artifact-1", &get(&at(&before, "/")));
        assert_eq!(old.response.status(), StatusCode::NOT_FOUND);
        let new = artifacts.answer("artifact-1", &get(&at(&approved, "/")));
        assert_eq!(new.response.status(), StatusCode::OK);
        let url = |text: &str| tauri::Url::parse(text).unwrap();
        assert!(artifacts.may_navigate("artifact-1", &url(&at(&approved, "/"))));
        assert!(!artifacts.may_navigate("artifact-1", &url(&at(&before, "/"))));
    }

    #[test]
    fn purlis_s_page_script_is_the_one_its_test_runs() {
        assert!(PAGE_SCRIPT.contains("/.purlis/key/"));
        assert!(PAGE_SCRIPT.contains(serve::THEME_SHEET));
        assert!(THEME_CHANGED.contains("__purlisTheme"));
        assert!(PAGE_SCRIPT.contains("__purlisTheme"));
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

    /// #5978's review: a Repin is heard by every window, as an Unpin is, so another window's
    /// group and tab read the pin again.
    #[test]
    fn a_repin_tells_every_window_that_the_pins_moved() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use tauri::{Listener, Manager};
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("project");
        let ws = root.join("workspaces").join("alpha");
        std::fs::create_dir_all(&ws).unwrap();
        std::fs::write(purlis_core::names::manifest(&root), "schema = 1\n").unwrap();
        artifact::adopt(&root, &ws, "board", "<title>Board</title>").unwrap();
        artifact::pin(
            &root,
            &ws,
            &artifact::Request {
                folder: "board".into(),
                ..artifact::Request::default()
            },
        )
        .unwrap();

        let app = tauri::test::mock_builder()
            .build(tauri_context!(test = true))
            .expect("the app builds");
        let planes = crate::planes::Planes::telling(
            std::sync::Arc::new(|_| {}),
            crate::Shipped::default(),
            None,
        );
        let plane = planes.open(&root);
        app.manage(planes);
        let heard = std::sync::Arc::new(AtomicUsize::new(0));
        let count = heard.clone();
        app.listen_any(ARTIFACT_PINS_MOVED, move |_| {
            count.fetch_add(1, Ordering::SeqCst);
        });

        tauri::async_runtime::block_on(repin_and_tell(
            app.handle(),
            &app.state::<Planes>(),
            plane,
            "alpha".to_owned(),
            "board".to_owned(),
        ))
        .expect("the repin is done");

        assert_eq!(heard.load(Ordering::SeqCst), 1);
        assert_eq!(artifact::pinned(&ws, "board").unwrap().name, "Board");
    }
}
