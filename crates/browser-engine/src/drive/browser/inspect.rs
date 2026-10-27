// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Inspect: the engine's own DevTools, for the person** (#2028, spec #2008).
//!
//! The person, driving a browser, opens the real DevTools of the page it shows. Nothing listens
//! for them and nothing is embedded in the window:
//!
//! - **The DevTools front end is a page of the engine's own**, the one the engine ships
//!   ([`FRONT_END`]), opened in a browser context of its own that the driver never lists: no call
//!   of the agent's can name it, read it or screenshot it, and any other page that context opens
//!   is closed. It is watched as frames, as a browser is, and the person's input reaches it as a
//!   browser view's does.
//! - **It speaks to the inspected page through the host, over the browser's one pipe.** Inside
//!   the front end, the `WebSocket` it opens is a stand-in ([`STAND_IN`]) that never opens a
//!   connection: what it sends comes to the driver as a binding's call, and what the page answers
//!   is handed back into it. The page is spoken to on a session of the front end's own, tapped
//!   ([`crate::cdp::Connection::tap`]), so its messages and those of the frames and workers
//!   attached under it go to the front end alone, in order, and never to the driver.
//! - **The front end reaches the inspected page and nothing else**: a message on any other
//!   session is refused, and so is every method that reaches past the page ([`not_offered`]): the
//!   browser itself, other targets and contexts, downloads, and a navigation to anything but a
//!   web address. Its own requests to the web are failed: it loads only its own files.
//! - **It is the person's while they drive.** It opens only while they drive and ends as they
//!   hand the browser back, so nothing they set in it (a breakpoint, an override, throttling) acts
//!   on the agent's calls; its context, and all it kept, is thrown away. What they type into it
//!   is kept out of every later answer the agent gets, as what they type into a page is.
//! - **Bounded.** At most [`MOST_QUEUED`] messages wait for the front end; one that falls further
//!   behind is ended, and the person is told.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender};
use std::sync::{Arc, Mutex, Weak};
use std::thread;
use std::time::Duration;

use base64::Engine as _;
use serde_json::{Value, json};

use super::super::tree::NodeKey;
use super::takeover::{button_bit, key_event, metrics, mouse_event, typed};
use super::{Browser, Inner, refusal};
use crate::WebAddress;
use crate::cdp::{Tap, Tapped};
use crate::drive::{Aim, Driver, MouseKind, PageId, Painted, PersonInput, Refusal, lock};

/// The front end the engine ships, told that its connection is the stand-in's.
pub const FRONT_END: &str =
    "devtools://devtools/bundled/devtools_app.html?ws=inspect.purlis.invalid/page";

/// The binding the stand-in sends the front end's messages through.
pub(crate) const BINDING: &str = "__purlisInspect";

/// What a frame of the front end names as its page: never one of the browser's pages.
pub const INSPECTOR_PAGE: &str = "devtools";

/// The most messages that wait to be handed to the front end, or from it to the page.
pub const MOST_QUEUED: usize = 8192;

/// The roles of the fields a page's tree shows: what DevTools may have put a value into.
const FIELD_ROLES: &[&str] = &["textbox", "searchbox", "combobox", "spinbutton"];

/// The most bytes handed to the front end at once.
const MOST_BATCH_BYTES: usize = 4 << 20;

/// The stand-in for the front end's `WebSocket`, run in the front end before its own scripts: a
/// constant. It opens nothing: what is sent goes through [`BINDING`], and what the page answers
/// is handed to it by `deliver`, as JSON text.
pub(crate) const STAND_IN: &str = r"(() => {
  const send = globalThis.__purlisInspect;
  const open = [];
  class Socket extends EventTarget {
    constructor(url) {
      super();
      this.url = String(url);
      this.readyState = 0;
      this.binaryType = 'blob';
      this.onopen = null; this.onmessage = null; this.onclose = null; this.onerror = null;
      open.push(this);
      queueMicrotask(() => {
        this.readyState = 1;
        const opened = new Event('open');
        if (this.onopen) this.onopen(opened);
        this.dispatchEvent(opened);
      });
    }
    send(data) { if (this.readyState === 1) send(String(data)); }
    close() { this.readyState = 3; }
  }
  Socket.CONNECTING = 0; Socket.OPEN = 1; Socket.CLOSING = 2; Socket.CLOSED = 3;
  Object.defineProperty(globalThis, 'WebSocket', { value: Socket, writable: false, configurable: false });
  const deliver = (batch) => {
    for (const data of JSON.parse(batch)) {
      for (const socket of open) {
        if (socket.readyState !== 1) continue;
        const message = new MessageEvent('message', { data });
        if (socket.onmessage) socket.onmessage(message);
        socket.dispatchEvent(message);
      }
    }
  };
  Object.defineProperty(globalThis, '__purlisInspector', { value: Object.freeze({ deliver }), writable: false, configurable: false });
})()";

/// The methods the front end may not call: they reach past the inspected page.
const NOT_OFFERED: &[&str] = &[
    "Target.attachToTarget",
    "Target.attachToBrowserTarget",
    "Target.createTarget",
    "Target.closeTarget",
    "Target.createBrowserContext",
    "Target.disposeBrowserContext",
    "Target.exposeDevToolsProtocol",
    "Target.setDiscoverTargets",
    "Target.setRemoteLocations",
    "Target.getTargets",
    "Target.activateTarget",
    "Page.setDownloadBehavior",
    "Page.close",
    "Page.crash",
];

/// Whether the front end's call of `method` with `params` reaches past the inspected page, and
/// is refused: the browser's own domains, the methods of [`NOT_OFFERED`], and a navigation to
/// anything but a web address.
pub(crate) fn not_offered(method: &str, params: &Value) -> bool {
    if method.starts_with("Browser.") || method.starts_with("SystemInfo.") {
        return true;
    }
    if NOT_OFFERED.contains(&method) {
        return true;
    }
    if method == "Page.navigate" {
        let address = params
            .get("url")
            .and_then(Value::as_str)
            .unwrap_or_default();
        return WebAddress::parse(address).is_err();
    }
    false
}

/// Why an Inspect ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectEnded {
    /// The person handed the browser back, or closed it.
    Closed,
    /// The page it inspected went.
    PageGone,
    /// It fell more than [`MOST_QUEUED`] messages behind.
    FellBehind,
    /// Its front end, or the browser, ended.
    Ended,
}

impl InspectEnded {
    /// What the person is told, where they are told.
    pub fn said(self) -> &'static str {
        match self {
            InspectEnded::Closed => "DevTools closed.",
            InspectEnded::PageGone => "DevTools closed: the page it inspected closed.",
            InspectEnded::FellBehind => {
                "DevTools closed: it fell too far behind the page. Inspect again to reopen it."
            }
            InspectEnded::Ended => "DevTools closed: it ended. Inspect again to reopen it.",
        }
    }
}

/// Who is told once, as an Inspect ends.
type Told = Box<dyn FnOnce(InspectEnded) + Send>;

/// Where the front end's frames go.
type Sink = Arc<dyn Fn(Painted) + Send + Sync>;

/// What the front end's own thread does next.
enum Work {
    /// A message of the inspected page's, for the front end.
    Tapped(Tapped),
    /// A message of the front end's, for the page.
    FromFrontEnd(String),
    /// A frame of the front end's, to acknowledge.
    Frame(i64),
    Show,
    Hide,
}

/// One Inspect: the front end, its context and its session; the inspected page and the tapped
/// session it is spoken to on.
pub(crate) struct Inspection {
    pub(crate) context: String,
    pub(crate) front_end: String,
    pub(crate) front_session: String,
    pub(crate) inspected: String,
    pub(crate) root: String,
    tap: Mutex<Option<Tap>>,
    work: SyncSender<Work>,
    sink: Mutex<Option<Sink>>,
    /// Counts the front end's documents: what its frames' aims name.
    shows: AtomicU64,
    ended: AtomicBool,
    behind: AtomicBool,
    told: Mutex<Option<Told>>,
    /// The methods the front end asked of the page, by name alone, the first
    /// [`MOST_HEARD`] of them.
    heard: Mutex<Vec<String>>,
}

/// How many of the front end's methods are kept to say what it asked.
const MOST_HEARD: usize = 256;

impl Inspection {
    /// Takes in an event of the front end's own session, from the driver's event reader.
    pub(crate) fn front_end_event(&self, method: &str, params: &Value) {
        match method {
            "Runtime.bindingCalled" if params["name"] == BINDING => {
                if let Some(payload) = params["payload"].as_str() {
                    self.queue(Work::FromFrontEnd(payload.to_owned()));
                }
            }
            "Page.screencastFrame" => {
                let (Some(data), Some(ack)) = (
                    params.get("data").and_then(Value::as_str),
                    params.get("sessionId").and_then(Value::as_i64),
                ) else {
                    return;
                };
                if let Some(sink) = lock(&self.sink).clone()
                    && let Ok(jpeg) = base64::engine::general_purpose::STANDARD.decode(data)
                {
                    sink(Painted {
                        aim: self.aim(),
                        address: String::new(),
                        jpeg,
                    });
                }
                self.queue(Work::Frame(ack));
            }
            "Page.frameNavigated" if params.pointer("/frame/parentId").is_none() => {
                self.shows.fetch_add(1, Ordering::SeqCst);
            }
            _ => {}
        }
    }

    /// What a frame of the front end shows now.
    pub(crate) fn aim(&self) -> Aim {
        Aim {
            page: PageId::new(INSPECTOR_PAGE),
            document: self.shows.load(Ordering::SeqCst),
        }
    }

    fn queue(&self, work: Work) {
        if self.work.try_send(work).is_err() {
            self.behind.store(true, Ordering::SeqCst);
        }
    }

    /// The methods the front end asked of the page so far, by name.
    pub(crate) fn heard(&self) -> Vec<String> {
        lock(&self.heard).clone()
    }
}

impl Browser {
    /// **Opens the DevTools of the current page for the person** (#2028), its front end
    /// `width` by `height` CSS pixels; told to `ended` once, as it ends. Refused unless the
    /// person drives. Inspecting again while it is open only sizes it.
    pub fn inspect(
        &self,
        width: u16,
        height: u16,
        ended: Box<dyn FnOnce(InspectEnded) + Send>,
    ) -> Result<(), Refusal> {
        if self.driver() != Driver::Person {
            return Err(Refusal::NotDriving);
        }
        let (width, height) = super::takeover::sized(width, height);
        let devtools = self.devtools();
        if let Some(open) = lock(&self.inner.inspection).clone() {
            devtools
                .call(
                    "Emulation.setDeviceMetricsOverride",
                    metrics(width, height),
                    Some(&open.front_session),
                )
                .map_err(refusal)?;
            *lock(&open.told) = Some(ended);
            return Ok(());
        }
        let inspected = self.target(None)?;
        let context = devtools
            .call("Target.createBrowserContext", json!({}), None)
            .map_err(refusal)?["browserContextId"]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| Refusal::Engine("no context was made for DevTools".into()))?;
        lock(&self.inner.state)
            .inspecting
            .contexts
            .insert(context.clone());
        let opened = self.open_front_end(&context, &inspected, width, height, ended);
        if opened.is_err() {
            // Whatever of it opened goes: an Inspect is open whole or not at all.
            if self.inspecting() {
                self.inner.stop_inspecting(InspectEnded::Ended);
            } else {
                let _ = devtools.call(
                    "Target.disposeBrowserContext",
                    json!({ "browserContextId": context }),
                    None,
                );
                let mut state = lock(&self.inner.state);
                state.inspecting.contexts.remove(&context);
                state.inspecting.front_ends.clear();
            }
        }
        opened
    }

    fn open_front_end(
        &self,
        context: &str,
        inspected: &str,
        width: u16,
        height: u16,
        ended: Box<dyn FnOnce(InspectEnded) + Send>,
    ) -> Result<(), Refusal> {
        let devtools = self.devtools();
        let front_end = devtools
            .call(
                "Target.createTarget",
                json!({ "url": "about:blank", "browserContextId": context }),
                None,
            )
            .map_err(refusal)?["targetId"]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| Refusal::Engine("no page was made for DevTools".into()))?;
        lock(&self.inner.state)
            .inspecting
            .front_ends
            .insert(front_end.clone());
        let front_session = devtools
            .call(
                "Target.attachToTarget",
                json!({ "targetId": front_end, "flatten": true }),
                None,
            )
            .map_err(refusal)?["sessionId"]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| Refusal::Engine("DevTools could not be reached".into()))?;
        let on = Some(front_session.as_str());
        devtools
            .call("Runtime.addBinding", json!({ "name": BINDING }), on)
            .map_err(refusal)?;
        devtools
            .call("Runtime.enable", json!({}), on)
            .map_err(refusal)?;
        devtools
            .call("Page.enable", json!({}), on)
            .map_err(refusal)?;
        devtools
            .call(
                "Page.addScriptToEvaluateOnNewDocument",
                json!({ "source": STAND_IN }),
                on,
            )
            .map_err(refusal)?;
        devtools
            .call(
                "Emulation.setDeviceMetricsOverride",
                metrics(width, height),
                on,
            )
            .map_err(refusal)?;
        // The session the page is spoken to on: the driver takes its word of it for the front
        // end's, never for one of its own.
        lock(&self.inner.state)
            .inspecting
            .bridging
            .insert(inspected.to_owned());
        let root = devtools.call(
            "Target.attachToTarget",
            json!({ "targetId": inspected, "flatten": true }),
            None,
        );
        lock(&self.inner.state)
            .inspecting
            .bridging
            .remove(inspected);
        let root = root.map_err(refusal)?["sessionId"]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| Refusal::Engine("the page could not be reached".into()))?;
        let (work, todo) = mpsc::sync_channel(MOST_QUEUED);
        let inspection = Arc::new(Inspection {
            context: context.to_owned(),
            front_end: front_end.clone(),
            front_session: front_session.clone(),
            inspected: inspected.to_owned(),
            root: root.clone(),
            tap: Mutex::new(None),
            work,
            sink: Mutex::new(None),
            shows: AtomicU64::new(0),
            ended: AtomicBool::new(false),
            behind: AtomicBool::new(false),
            told: Mutex::new(Some(ended)),
            heard: Mutex::new(Vec::new()),
        });
        let tapping = Arc::downgrade(&inspection);
        let tap = devtools.tap(
            &root,
            Arc::new(move |tapped| {
                let Some(inspection) = tapping.upgrade() else {
                    return false;
                };
                let taken = inspection.work.try_send(Work::Tapped(tapped)).is_ok();
                if !taken {
                    inspection.behind.store(true, Ordering::SeqCst);
                }
                taken
            }),
        );
        let Some(tap) = tap else {
            return Err(Refusal::Engine("DevTools could not be reached".into()));
        };
        *lock(&inspection.tap) = Some(tap);
        *lock(&self.inner.inspection) = Some(Arc::clone(&inspection));
        let weak = Arc::downgrade(&self.inner);
        let theirs = Arc::clone(&inspection);
        thread::Builder::new()
            .name("browser-inspect".to_owned())
            .spawn(move || run(&weak, &theirs, &todo))
            .map_err(|e| Refusal::Engine(e.to_string()))?;
        devtools
            .call("Page.navigate", json!({ "url": FRONT_END }), on)
            .map_err(refusal)?;
        Ok(())
    }

    /// Whether DevTools are open.
    pub fn inspecting(&self) -> bool {
        lock(&self.inner.inspection).is_some()
    }

    /// The methods the open DevTools asked of the page, by name alone: none when none is open.
    pub fn inspector_heard(&self) -> Vec<String> {
        lock(&self.inner.inspection)
            .as_ref()
            .map(|open| open.heard())
            .unwrap_or_default()
    }

    /// Starts making frames of the open DevTools, each handed to `frames`.
    pub fn show_inspector(&self, frames: Arc<dyn Fn(Painted) + Send + Sync>) {
        if let Some(open) = lock(&self.inner.inspection).clone() {
            *lock(&open.sink) = Some(frames);
            open.queue(Work::Show);
        }
    }

    /// Stops making frames of the open DevTools.
    pub fn hide_inspector(&self) {
        if let Some(open) = lock(&self.inner.inspection).clone() {
            *lock(&open.sink) = None;
            open.queue(Work::Hide);
        }
    }

    /// What a frame of the open DevTools shows now.
    pub fn inspector_aim(&self) -> Option<Aim> {
        lock(&self.inner.inspection).as_ref().map(|open| open.aim())
    }

    /// One event of the person's into the open DevTools, while they drive: dropped
    /// ([`Refusal::Missed`]) where it was aimed at something else. What they type is kept out
    /// of every later answer the agent gets.
    pub fn inspector_input(&self, input: &PersonInput) -> Result<(), Refusal> {
        if self.driver() != Driver::Person {
            return Err(Refusal::NotDriving);
        }
        let open = lock(&self.inner.inspection)
            .clone()
            .ok_or(Refusal::Missed)?;
        let now = open.aim();
        if input.aim().page != now.page || input.aim().document != now.document {
            return Err(Refusal::Missed);
        }
        let on = Some(open.front_session.as_str());
        match input {
            PersonInput::Key {
                down,
                key,
                code,
                key_code,
                modifiers,
                ..
            } => {
                let text = (*down).then(|| typed(key, *modifiers)).flatten();
                let event = key_event(*down, key, code, *key_code, *modifiers, text.as_deref());
                if *down {
                    lock(&self.typing).typed(INSPECTOR_PAGE, key, text.as_deref());
                }
                self.devtools()
                    .call("Input.dispatchKeyEvent", event, on)
                    .map_err(refusal)?;
            }
            PersonInput::Mouse {
                kind,
                x,
                y,
                button,
                clicks,
                modifiers,
                delta_x,
                delta_y,
                ..
            } => {
                if *kind == MouseKind::Pressed {
                    lock(&self.typing).end_run(INSPECTOR_PAGE);
                }
                let buttons = match kind {
                    MouseKind::Pressed => button_bit(*button),
                    _ => 0,
                };
                let mut event = mouse_event(*kind, *x, *y, *button, buttons, *modifiers);
                if *clicks > 0 {
                    event["clickCount"] = (*clicks).into();
                }
                if *kind == MouseKind::Wheel {
                    event["deltaX"] = (*delta_x).into();
                    event["deltaY"] = (*delta_y).into();
                }
                self.devtools()
                    .call("Input.dispatchMouseEvent", event, on)
                    .map_err(refusal)?;
            }
        }
        Ok(())
    }

    /// **Every field of the page the open DevTools inspect counts as one the person typed into**
    /// (#2028): what DevTools put into it (from the console, say) is read from the field and kept
    /// out at the hand-back, as what they type is. A page whose trees cannot be read keeps only
    /// what was typed.
    pub(super) fn devtools_touched_every_field(&self) {
        let Some(open) = lock(&self.inner.inspection).clone() else {
            return;
        };
        let Ok(trees) = self.trees_of(&open.inspected) else {
            return;
        };
        let mut typing = lock(&self.typing);
        typing.dirty.insert(open.inspected.clone());
        for frame in &trees {
            for node in frame.nodes.as_array().into_iter().flatten() {
                let role = node
                    .pointer("/role/value")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let Some(backend) = node.get("backendDOMNodeId").and_then(Value::as_i64) else {
                    continue;
                };
                if FIELD_ROLES.contains(&role) {
                    typing.touched.insert((
                        open.inspected.clone(),
                        NodeKey {
                            frame: frame.id.clone(),
                            backend,
                        },
                    ));
                }
            }
        }
    }

    /// Ends the open DevTools, saying why: its context, and all it kept, thrown away.
    pub fn stop_inspecting(&self) {
        self.inner.stop_inspecting(InspectEnded::Closed);
    }
}

impl Inner {
    /// The open Inspect whose front end's session is `session`.
    pub(crate) fn inspection_on(&self, session: &str) -> Option<Arc<Inspection>> {
        lock(&self.inspection)
            .clone()
            .filter(|open| open.front_session == session)
    }

    /// Ends the open Inspect, if any, telling why.
    pub(crate) fn stop_inspecting(&self, why: InspectEnded) {
        let Some(open) = lock(&self.inspection).take() else {
            return;
        };
        if open.ended.swap(true, Ordering::SeqCst) {
            return;
        }
        *lock(&open.sink) = None;
        if let Some(tap) = lock(&open.tap).take() {
            self.devtools.untap(&tap);
        }
        if !self.ended() {
            let _ = self.devtools.call(
                "Target.detachFromTarget",
                json!({ "sessionId": open.root }),
                None,
            );
            let _ = self.devtools.call(
                "Target.disposeBrowserContext",
                json!({ "browserContextId": open.context }),
                None,
            );
        }
        {
            let mut state = lock(&self.state);
            state.inspecting.contexts.remove(&open.context);
            state.inspecting.front_ends.remove(&open.front_end);
        }
        if let Some(tell) = lock(&open.told).take() {
            tell(why);
        }
    }
}

/// The front end's own thread: hands the page's messages to it, a batch at a time, in order;
/// sends its messages on to the page; acknowledges its frames; ends it once it falls behind.
fn run(weak: &Weak<Inner>, open: &Arc<Inspection>, todo: &Receiver<Work>) {
    let mut batch: Vec<String> = Vec::new();
    let mut bytes = 0;
    loop {
        let next = todo.recv_timeout(Duration::from_millis(250));
        let Some(inner) = weak.upgrade() else {
            return;
        };
        if open.ended.load(Ordering::SeqCst) {
            return;
        }
        if inner.ended() {
            inner.stop_inspecting(InspectEnded::Ended);
            return;
        }
        if open.behind.load(Ordering::SeqCst) {
            inner.stop_inspecting(InspectEnded::FellBehind);
            return;
        }
        let mut work = match next {
            Ok(work) => Some(work),
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => return,
        };
        while let Some(one) = work.take() {
            match one {
                Work::Tapped(tapped) => {
                    if let Tapped::Event(event) = &tapped
                        && event.method == "Target.detachedFromTarget"
                        && event.params["sessionId"].as_str() == Some(open.root.as_str())
                    {
                        inner.stop_inspecting(InspectEnded::PageGone);
                        return;
                    }
                    let message = for_the_front_end(tapped, &open.root);
                    bytes += message.len();
                    batch.push(message);
                }
                Work::FromFrontEnd(payload) => {
                    if let Some(refused) = to_the_page(&inner, open, &payload) {
                        bytes += refused.len();
                        batch.push(refused);
                    }
                }
                Work::Frame(ack) => {
                    let _ = inner.devtools.call(
                        "Page.screencastFrameAck",
                        json!({ "sessionId": ack }),
                        Some(&open.front_session),
                    );
                }
                Work::Show => {
                    let _ = inner.devtools.call(
                        "Page.startScreencast",
                        json!({ "format": "jpeg", "quality": super::super::screencast::QUALITY, "everyNthFrame": 1 }),
                        Some(&open.front_session),
                    );
                }
                Work::Hide => {
                    let _ = inner.devtools.call(
                        "Page.stopScreencast",
                        json!({}),
                        Some(&open.front_session),
                    );
                }
            }
            if bytes < MOST_BATCH_BYTES {
                work = todo.try_recv().ok();
            }
        }
        if !batch.is_empty() {
            let whole = serde_json::to_string(&batch).expect("strings serialise");
            let literal = serde_json::to_string(&whole).expect("a string serialises");
            let _ = inner.devtools.call(
                "Runtime.evaluate",
                json!({ "expression": format!("globalThis.__purlisInspector.deliver({literal})") }),
                Some(&open.front_session),
            );
            batch.clear();
            bytes = 0;
        }
    }
}

/// A message of the page's, as the front end reads it: on the session it was given (the page's
/// own named by none).
fn for_the_front_end(tapped: Tapped, root: &str) -> String {
    let message = match tapped {
        Tapped::Event(event) => {
            let mut message = json!({ "method": event.method, "params": event.params });
            if let Some(session) = event.session_id.filter(|s| s != root) {
                message["sessionId"] = Value::String(session);
            }
            message
        }
        Tapped::Answer { id, outcome } => match outcome {
            Ok(result) => json!({ "id": id, "result": result }),
            Err(error) => json!({ "id": id, "error": error }),
        },
    };
    message.to_string()
}

/// Sends a message of the front end's on to the page: answers the refusal to hand back where it
/// is not one the page is sent.
fn to_the_page(inner: &Inner, open: &Inspection, payload: &str) -> Option<String> {
    let message: Value = serde_json::from_str(payload).ok()?;
    let id = message.get("id").and_then(Value::as_u64)?;
    let refused = |why: &str| {
        Some(json!({ "id": id, "error": { "code": -32601, "message": why } }).to_string())
    };
    let Some(method) = message.get("method").and_then(Value::as_str) else {
        return refused("a message with no method");
    };
    let params = message.get("params").cloned().unwrap_or_else(|| json!({}));
    if not_offered(method, &params) {
        return refused(&format!("{method} is not offered in purlis's DevTools"));
    }
    {
        let mut heard = lock(&open.heard);
        if heard.len() < MOST_HEARD {
            heard.push(method.to_owned());
        }
    }
    let session = message
        .get("sessionId")
        .and_then(Value::as_str)
        .unwrap_or(&open.root);
    let tap = lock(&open.tap);
    let tap = tap.as_ref()?;
    match inner.devtools.send_tapped(tap, id, method, params, session) {
        Ok(()) => None,
        Err(why) => refused(&why.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::not_offered;
    use serde_json::json;

    #[test]
    fn the_front_end_reaches_the_inspected_page_and_nothing_past_it() {
        for method in [
            "Browser.close",
            "Browser.setDownloadBehavior",
            "Target.createTarget",
            "Target.attachToTarget",
            "Target.attachToBrowserTarget",
            "Target.exposeDevToolsProtocol",
            "Target.disposeBrowserContext",
            "SystemInfo.getProcessInfo",
        ] {
            assert!(not_offered(method, &json!({})), "{method} is refused");
        }
        for method in [
            "DOM.getDocument",
            "Runtime.evaluate",
            "Network.enable",
            "Target.setAutoAttach",
            "Overlay.highlightNode",
        ] {
            assert!(!not_offered(method, &json!({})), "{method} is the page's");
        }
        assert!(!not_offered(
            "Page.navigate",
            &json!({ "url": "https://example.com/" })
        ));
        for address in [
            "file:///etc/passwd",
            "chrome://settings",
            "javascript:alert(1)",
            "devtools://devtools/bundled/devtools_app.html",
        ] {
            assert!(
                not_offered("Page.navigate", &json!({ "url": address })),
                "{address}"
            );
        }
    }
}
