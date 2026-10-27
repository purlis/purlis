// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Inspect: the engine's own DevTools, for the person** (#2028, spec #2008).
//!
//! The person, driving a browser, opens the real DevTools of the page it shows. Nothing listens
//! for them and nothing is embedded in the window:
//!
//! - **The DevTools front end is a page of the engine's own**, the one the engine ships
//!   ([`FRONT_END`]), opened in a browser context of its own, in a window of its own, that the
//!   driver never lists: no call of the agent's can name it, read it or screenshot it, and any
//!   other page that context opens is closed. It stays at that page: one that moves to any
//!   other address is closed. It is watched as frames, as a browser is, and the person's input
//!   reaches it as a browser view's does.
//! - **It speaks to the inspected page through the host, over the browser's one pipe.** Inside
//!   the front end, the `WebSocket` it opens is a stand-in ([`STAND_IN_SOURCE`]) that never opens a
//!   connection, and the engine's own embedder interface is not given to it: what it sends comes
//!   to the driver as a binding's call from its own main world, and what the page answers is
//!   handed back into it as a value. The page is spoken to on a session of the front end's own,
//!   and both that session and the front end's are tapped ([`crate::cdp::Connection::tap`]):
//!   their messages go to the Inspect's own thread, in order, never to the driver's, and a
//!   message of theirs too large to read closes DevTools alone.
//! - **What the front end may ask of the page is an allow-list** (`allowed.rs`): any other
//!   method, any session but its own and those under it, any origin but the inspected page's,
//!   and purlis's own world in the page are refused. Its own requests to the web are failed.
//! - **It is the person's while they drive.** It opens only while they drive, and cannot finish
//!   opening once they have handed the browser back. It ends as they hand it back, so nothing
//!   they set in it (a breakpoint, an override, throttling, emulation) acts on the agent's calls:
//!   its sessions are detached and its context, with all it kept, thrown away; purlis's own
//!   worlds in every page are made afresh. What they type into it, what it wrote into a page
//!   (`written.rs`) and what every field of every page holds are kept out of every later answer
//!   the agent gets, as what they type into a page is, however DevTools ended; and the engine's
//!   clipboard is emptied.
//! - **Bounded.** At most [`MOST_QUEUED`] messages wait, and none larger than
//!   [`MOST_RELAYED_BYTES`] is handed on; DevTools that fall further behind close, saying why.

mod allowed;
mod written;

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender};
use std::sync::{Arc, Mutex, Weak};
use std::thread;
use std::time::Duration;

use base64::Engine as _;
use serde_json::{Value, json};

use super::super::kept::Kept;
use super::super::tree::NodeKey;
use super::takeover::{button_bit, key_event, metrics, mouse_event, typed};
use super::{Browser, Inner, refusal};
use crate::cdp::{Tap, Tapped};
use crate::drive::{Aim, Driver, MouseKind, PageId, Painted, PersonInput, Refusal, lock};

/// The front end the engine ships, told that its connection is the stand-in's.
pub const FRONT_END: &str =
    "devtools://devtools/bundled/devtools_app.html?ws=inspect.purlis.invalid/page";

/// The page of [`FRONT_END`], without what follows it: the one page the front end may show.
const FRONT_END_PAGE: &str = "devtools://devtools/bundled/devtools_app.html";

/// The binding the stand-in sends the front end's messages through.
pub(crate) const BINDING: &str = "__purlisInspect";

/// What a frame of the front end names as its page: never one of the browser's pages.
pub const INSPECTOR_PAGE: &str = "devtools";

/// The most messages that wait to be handed to the front end, or from it to the page.
pub const MOST_QUEUED: usize = 8192;

/// The largest message handed between the front end and the page: an answer past it is
/// answered as an error, any other message past it closes DevTools.
pub const MOST_RELAYED_BYTES: usize = 8 << 20;

/// The most bytes handed to the front end at once, and the most that wait for it to load.
const MOST_BATCH_BYTES: usize = 4 << 20;

/// The roles of the fields a page's tree shows: what DevTools may have put a value into.
const FIELD_ROLES: &[&str] = &["textbox", "searchbox", "combobox", "spinbutton", "listbox"];

/// The stand-in for the front end's `WebSocket`, run in the front end before its own scripts: a
/// constant. It opens nothing: what is sent goes through [`BINDING`], and what the page answers
/// is handed to it by `deliver`, as a list of messages. Where the engine gives the front end its
/// embedder interface (`DevToolsHost`), it is wrapped before anything reads it, so only the asks
/// of [`EMBEDDER_ALLOWED`] reach the engine and every other is answered with nothing.
pub(crate) const STAND_IN_SOURCE: &str = r"(() => {
  // The engine may install its embedder interface before this runs or after: either way, what
  // the front end reads is the wrapped one.
  const refusedNames = new Set();
  const passedNames = new Set();
  const errors = [];
  globalThis.addEventListener('error', (e) => { if (errors.length < 5) errors.push(String(e.message).slice(0, 160)); });
  globalThis.addEventListener('unhandledrejection', (e) => { if (errors.length < 5) errors.push(String(e.reason && e.reason.message || e.reason).slice(0, 160)); });
  const wrap = (native) => {
    const allowed = new Set(EMBEDDER_ALLOWED);
    const refuse = (message) => {
      let id;
      try { const asked = JSON.parse(message); id = asked.id; if (refusedNames.size < 64) refusedNames.add(String(asked.method).slice(0, 64)); } catch (e) { return; }
      if (typeof id !== 'number') return;
      setTimeout(() => {
        const api = globalThis.DevToolsAPI;
        if (api && typeof api.embedderMessageAck === 'function') api.embedderMessageAck(id, null);
      }, 0);
    };
    const toEmbedder = (message) => {
      let method;
      try { method = JSON.parse(message).method; } catch (e) { return; }
      if (passedNames.size < 64) passedNames.add(String(method).slice(0, 64));
      if (allowed.has(method)) native.sendMessageToEmbedder(message);
      else refuse(message);
    };
    return new Proxy(native, {
      get(target, key) {
        if (key === '__purlisWrapped') return true;
        if (key === 'sendMessageToEmbedder') return toEmbedder;
        const value = Reflect.get(target, key);
        return typeof value === 'function' ? value.bind(target) : value;
      },
      set() { return false; },
      defineProperty() { return false; },
    });
  };
  let host = globalThis.DevToolsHost ? wrap(globalThis.DevToolsHost) : undefined;
  try {
    Object.defineProperty(globalThis, 'DevToolsHost', {
      get() { return host; },
      set(native) { if (host === undefined && native) host = wrap(native); },
      configurable: false,
    });
  } catch (e) {}
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
    for (const data of batch) {
      for (const socket of open) {
        if (socket.readyState !== 1) continue;
        const message = new MessageEvent('message', { data });
        if (socket.onmessage) socket.onmessage(message);
        socket.dispatchEvent(message);
      }
    }
  };
  Object.defineProperty(globalThis, '__purlisInspector', { value: Object.freeze({ deliver, refused: () => [...refusedNames], asked: () => [...passedNames], errors: () => [...errors], sockets: () => open.length }), writable: false, configurable: false });
})()";

/// **What the front end may ask of the engine's embedder interface** (#2028), where the engine
/// gives it one: its own settings and what it draws, and nothing that reaches past the front
/// end (no network, file, window, tab or device of the engine's). Every other ask is answered
/// with nothing and never reaches the engine.
pub(crate) const EMBEDDER_ALLOWED: &[&str] = &[
    "getPreferences",
    "getPreference",
    "setPreference",
    "removePreference",
    "clearPreferences",
    "getSyncInformation",
    "getHostConfig",
    "setInspectedPageBounds",
    "setIsDocked",
    "setWhitelistedShortcuts",
    "setEyeDropperActive",
    "loadCompleted",
    "zoomIn",
    "zoomOut",
    "resetZoom",
    "copyText",
];

/// [`STAND_IN_SOURCE`] with [`EMBEDDER_ALLOWED`] in it: what runs in the front end.
pub(crate) fn stand_in() -> String {
    let allowed = serde_json::to_string(EMBEDDER_ALLOWED).expect("names serialise");
    STAND_IN_SOURCE.replacen("EMBEDDER_ALLOWED", &allowed, 1)
}

/// The function a batch is handed to the front end with: a constant, the batch its argument.
pub(crate) const DELIVER: &str =
    "function (batch) { globalThis.__purlisInspector.deliver(batch); }";

/// The script that says whether the front end runs as a hosted one: a constant, whose answer
/// names what it found and nothing the front end showed.
const HOSTED: &str = r"JSON.stringify({
  host: typeof globalThis.DevToolsHost,
  wrapped: !!(globalThis.DevToolsHost && globalThis.DevToolsHost.__purlisWrapped),
  refused: (globalThis.__purlisInspector && globalThis.__purlisInspector.refused) ? globalThis.__purlisInspector.refused() : null,
  asked: (globalThis.__purlisInspector && globalThis.__purlisInspector.asked) ? globalThis.__purlisInspector.asked() : null,
  errors: (globalThis.__purlisInspector && globalThis.__purlisInspector.errors) ? globalThis.__purlisInspector.errors() : null,
  sockets: (globalThis.__purlisInspector && globalThis.__purlisInspector.sockets) ? globalThis.__purlisInspector.sockets() : null,
  hosted: (() => { try { const h = globalThis.InspectorFrontendHost; return h && typeof h.isHostedMode === 'function' ? h.isHostedMode() : null; } catch (e) { return 'threw'; } })()
})";

/// The script that empties the clipboard of the page it runs in, the engine's: a constant.
pub(crate) const EMPTY_CLIPBOARD: &str = r"(() => {
  document.addEventListener('copy', (e) => { e.clipboardData.setData('text/plain', ''); e.preventDefault(); }, { once: true, capture: true });
  return document.execCommand('copy');
})()";

/// Why an Inspect ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectEnded {
    /// The person handed the browser back, or closed it.
    Closed,
    /// The page it inspected went.
    PageGone,
    /// It fell more than [`MOST_QUEUED`] messages behind.
    FellBehind,
    /// The page or DevTools sent a message too large to hand on.
    TooLarge,
    /// The front end left its own page.
    Moved,
    /// Its front end, or the browser, ended.
    Ended,
}

impl InspectEnded {
    /// What the person is told: what happened, and what to do.
    pub fn said(self) -> &'static str {
        match self {
            InspectEnded::Closed => "DevTools closed when the browser was handed back.",
            InspectEnded::PageGone => {
                "DevTools closed: the page they inspected closed. Press Inspect on the browser to open them on its page now."
            }
            InspectEnded::FellBehind => {
                "DevTools closed: they fell too far behind the page. Press Inspect on the browser to open them again."
            }
            InspectEnded::TooLarge => {
                "DevTools closed: the page sent them more than they can show at once. Press Inspect on the browser to open them again."
            }
            InspectEnded::Moved => {
                "DevTools closed: they left their own page. Press Inspect on the browser to open them again."
            }
            InspectEnded::Ended => {
                "DevTools closed: they ended. Press Inspect on the browser to open them again."
            }
        }
    }
}

/// Who is told once, as an Inspect ends.
type Told = Box<dyn FnOnce(InspectEnded) + Send>;

/// Where the front end's frames go.
type Sink = Arc<dyn Fn(Painted) + Send + Sync>;

/// What the Inspect's own thread does next.
enum Work {
    /// A message of the inspected page's session, or of one under it.
    Page(Tapped),
    /// A message of the front end's own session.
    Front(Tapped),
    Show,
    Hide,
    /// The Inspect ends, as the driver heard.
    Stop(InspectEnded),
}

/// One Inspect: the front end, its context and its session; the inspected page and the tapped
/// session it is spoken to on.
pub(crate) struct Inspection {
    pub(crate) context: String,
    pub(crate) front_end: String,
    pub(crate) front_session: String,
    pub(crate) inspected: String,
    /// The session the page is spoken to on, once it is made.
    root: Mutex<Option<String>>,
    tap: Mutex<Option<Tap>>,
    front_tap: Mutex<Option<Tap>>,
    work: SyncSender<Work>,
    behind: Arc<AtomicBool>,
    sink: Mutex<Option<Sink>>,
    /// Counts the front end's documents: what its frames' aims name.
    shows: AtomicU64,
    ended: AtomicBool,
    /// Whether its frames are asked for: asked again once its page has loaded, since the
    /// engine refuses a screencast while the page moves to DevTools' own process.
    showing: AtomicBool,
    tell: Mutex<Option<Told>>,
    /// The front end's own main world, where its stand-in runs.
    main_context: Mutex<Option<i64>>,
    /// purlis's own worlds in the inspected page, as the page's session tells of them: never
    /// told to DevTools, and never reached by them.
    ours: Mutex<HashSet<i64>>,
    ours_unique: Mutex<HashSet<String>>,
    /// The methods the front end asked of the page, by name alone, the first [`MOST_HEARD`].
    heard: Mutex<Vec<String>>,
    /// What the engine told of the front end's own page, by method, and which of purlis's
    /// calls on it it refused: the first [`MOST_HEARD`], each once.
    told: Mutex<Vec<String>>,
}

/// How many of the front end's methods are kept to say what it asked.
const MOST_HEARD: usize = 256;

impl Inspection {
    /// What a frame of the front end shows now.
    pub(crate) fn aim(&self) -> Aim {
        Aim {
            page: PageId::new(INSPECTOR_PAGE),
            document: self.shows.load(Ordering::SeqCst),
        }
    }

    /// Hands `work` to the Inspect's thread: one that cannot take it has fallen behind.
    fn queue(&self, work: Work) {
        if self.work.try_send(work).is_err() {
            self.behind.store(true, Ordering::SeqCst);
        }
    }

    /// The Inspect ends, as the driver heard: on its own thread, never the driver's.
    pub(crate) fn ends(&self, why: InspectEnded) {
        if self.work.try_send(Work::Stop(why)).is_err() {
            self.behind.store(true, Ordering::SeqCst);
        }
    }

    /// Notes `what` once, among the first [`MOST_HEARD`].
    fn noted(&self, what: &str) {
        let mut told = lock(&self.told);
        if told.len() < MOST_HEARD && !told.iter().any(|one| one == what) {
            told.push(what.to_owned());
        }
    }

    fn root(&self) -> Option<String> {
        lock(&self.root).clone()
    }
}

/// Whether the front end may show `address`: its own page, or the blank page it opens on.
pub(crate) fn front_end_may_show(address: &str) -> bool {
    let page = address.split(['?', '#']).next().unwrap_or_default();
    address == "about:blank" || page == FRONT_END_PAGE
}

/// A tap's sink, handing each message to the Inspect's thread as `wrap` makes it.
fn sink_to(
    work: &SyncSender<Work>,
    behind: &Arc<AtomicBool>,
    wrap: fn(Tapped) -> Work,
) -> crate::cdp::TapSink {
    let work = work.clone();
    let behind = Arc::clone(behind);
    Arc::new(move |tapped| {
        let taken = work.try_send(wrap(tapped)).is_ok();
        if !taken {
            behind.store(true, Ordering::SeqCst);
        }
        taken
    })
}

impl Browser {
    /// **Opens the DevTools of the current page for the person** (#2028), its front end
    /// `width` by `height` CSS pixels; told to `ended` once, as it ends. Refused unless the
    /// person drives, and refused once they have handed the browser back since it began.
    /// Inspecting again while it is open only sizes it. The host carries one `inspect` of a
    /// browser at a time (its program lane).
    pub fn inspect(&self, width: u16, height: u16, ended: Told) -> Result<(), Refusal> {
        if self.driver() != Driver::Person {
            return Err(Refusal::NotDriving);
        }
        let cut = self.inspect_cut.load(Ordering::SeqCst);
        let (width, height) = super::takeover::sized(width, height);
        if let Some(open) = lock(&self.inner.inspection).clone() {
            self.devtools()
                .call(
                    "Emulation.setDeviceMetricsOverride",
                    metrics(width, height),
                    Some(&open.front_session),
                )
                .map_err(refusal)?;
            *lock(&open.tell) = Some(ended);
            return Ok(());
        }
        let inspected = self.target(None)?;
        let devtools = self.devtools();
        let context = devtools
            .call("Target.createBrowserContext", json!({}), None)
            .map_err(refusal)?["browserContextId"]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| Refusal::Engine("no context was made for DevTools".into()))?;
        // Kept as DevTools' for as long as the browser lives: nothing it opens is ever a page.
        lock(&self.inner.state)
            .inspecting
            .contexts
            .insert(context.clone());
        let made = devtools
            .call(
                "Target.createTarget",
                // A window of its own, so it paints beside the page it inspects, each in front
                // in its own window: a headless engine paints only the page in front.
                json!({ "url": "about:blank", "browserContextId": context, "newWindow": true }),
                None,
            )
            .map_err(refusal)
            .and_then(|made| {
                made["targetId"]
                    .as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| Refusal::Engine("no page was made for DevTools".into()))
            });
        let front_end = match made {
            Ok(front_end) => front_end,
            Err(why) => {
                let _ = devtools.call(
                    "Target.disposeBrowserContext",
                    json!({ "browserContextId": context }),
                    None,
                );
                return Err(why);
            }
        };
        lock(&self.inner.state)
            .inspecting
            .front_ends
            .insert(context.clone(), front_end.clone());
        let attached = devtools
            .call(
                "Target.attachToTarget",
                json!({ "targetId": front_end, "flatten": true }),
                None,
            )
            .map_err(refusal)
            .and_then(|attached| {
                attached["sessionId"]
                    .as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| Refusal::Engine("DevTools could not be reached".into()))
            });
        let front_session = match attached {
            Ok(session) => session,
            Err(why) => {
                let _ = devtools.call("Target.closeTarget", json!({ "targetId": front_end }), None);
                let _ = devtools.call(
                    "Target.disposeBrowserContext",
                    json!({ "browserContextId": context }),
                    None,
                );
                return Err(why);
            }
        };
        let (work, todo) = mpsc::sync_channel(MOST_QUEUED);
        let behind = Arc::new(AtomicBool::new(false));
        let inspection = Arc::new(Inspection {
            context,
            front_end,
            front_session: front_session.clone(),
            inspected: inspected.clone(),
            root: Mutex::new(None),
            tap: Mutex::new(None),
            front_tap: Mutex::new(
                devtools.tap(&front_session, sink_to(&work, &behind, Work::Front)),
            ),
            work,
            behind,
            sink: Mutex::new(None),
            shows: AtomicU64::new(0),
            ended: AtomicBool::new(false),
            showing: AtomicBool::new(false),
            tell: Mutex::new(Some(ended)),
            main_context: Mutex::new(None),
            ours: Mutex::new(HashSet::new()),
            ours_unique: Mutex::new(HashSet::new()),
            heard: Mutex::new(Vec::new()),
            told: Mutex::new(Vec::new()),
        });
        if let Err(why) = self.set_up(&inspection, width, height) {
            self.inner.tear_down(&inspection);
            return Err(why);
        }
        // Published under the lease, as a hand-back cuts: one that came since this began finds
        // nothing to close, so this is refused instead.
        {
            let lease = lock(&self.lease);
            if lease.driver != Driver::Person || self.inspect_cut.load(Ordering::SeqCst) != cut {
                drop(lease);
                self.inner.tear_down(&inspection);
                return Err(Refusal::NotDriving);
            }
            *lock(&self.inner.inspection) = Some(Arc::clone(&inspection));
            self.inner.devtools_opened.store(true, Ordering::SeqCst);
        }
        let weak = Arc::downgrade(&self.inner);
        let theirs = Arc::clone(&inspection);
        if let Err(why) = thread::Builder::new()
            .name("browser-inspect".to_owned())
            .spawn(move || run(&weak, &theirs, &todo))
        {
            self.inner.stop_inspecting(InspectEnded::Ended);
            return Err(Refusal::Engine(why.to_string()));
        }
        let on = Some(front_session.as_str());
        // In front in its own window: a headless engine paints only a page in front.
        if devtools.call("Page.bringToFront", json!({}), on).is_err() {
            inspection.noted("Page.bringToFront refused");
        }
        if let Err(why) = devtools.call("Page.navigate", json!({ "url": FRONT_END }), on) {
            self.inner.stop_inspecting(InspectEnded::Ended);
            return Err(refusal(why));
        }
        Ok(())
    }

    /// Readies the front end's page and makes the session the page is spoken to on.
    fn set_up(&self, open: &Inspection, width: u16, height: u16) -> Result<(), Refusal> {
        let devtools = self.devtools();
        let on = Some(open.front_session.as_str());
        if lock(&open.front_tap).is_none() {
            return Err(Refusal::Engine("DevTools could not be reached".into()));
        }
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
                json!({ "source": stand_in() }),
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
        // The session the page is spoken to on: the driver takes the engine's word of it for
        // the front end's, never for one of its own, and lets the mark go as it hears it.
        lock(&self.inner.state)
            .inspecting
            .bridging
            .insert(open.inspected.clone());
        let root = devtools.call(
            "Target.attachToTarget",
            json!({ "targetId": open.inspected, "flatten": true }),
            None,
        );
        let root = match root {
            Ok(root) => root["sessionId"].as_str().map(str::to_owned),
            Err(why) => {
                lock(&self.inner.state)
                    .inspecting
                    .bridging
                    .remove(&open.inspected);
                return Err(refusal(why));
            }
        }
        .ok_or_else(|| Refusal::Engine("the page could not be reached".into()))?;
        *lock(&open.root) = Some(root.clone());
        let tap = devtools.tap(&root, sink_to(&open.work, &open.behind, Work::Page));
        let Some(tap) = tap else {
            return Err(Refusal::Engine("DevTools could not be reached".into()));
        };
        *lock(&open.tap) = Some(tap);
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
            .map(|open| lock(&open.heard).clone())
            .unwrap_or_default()
    }

    /// What the engine told of the open DevTools' own page, by method, and which of purlis's
    /// calls on it it refused: for the host's log and the tests, never anything they showed.
    pub fn inspector_told(&self) -> Vec<String> {
        lock(&self.inner.inspection)
            .as_ref()
            .map(|open| lock(&open.told).clone())
            .unwrap_or_default()
    }

    /// Starts making frames of the open DevTools, each handed to `frames`.
    pub fn show_inspector(&self, frames: Arc<dyn Fn(Painted) + Send + Sync>) {
        if let Some(open) = lock(&self.inner.inspection).clone() {
            *lock(&open.sink) = Some(frames);
            open.showing.store(true, Ordering::SeqCst);
            open.queue(Work::Show);
        }
    }

    /// Stops making frames of the open DevTools.
    pub fn hide_inspector(&self) {
        if let Some(open) = lock(&self.inner.inspection).clone() {
            *lock(&open.sink) = None;
            open.showing.store(false, Ordering::SeqCst);
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

    /// Ends the open DevTools: its sessions detached, its context, and all it kept, thrown away.
    pub fn stop_inspecting(&self) {
        self.inner.stop_inspecting(InspectEnded::Closed);
    }

    /// **What DevTools left behind, undone and kept out at the hand-back** (#2028), whatever
    /// ended them: once DevTools were open while the person drove, purlis's own worlds in every
    /// page are made afresh, the engine's clipboard is emptied, every field of every page counts
    /// as one the person typed into (read from the field and kept out), and every text DevTools
    /// wrote into a page is kept out as a run of the person's keys is.
    pub(super) fn after_devtools(&self) {
        self.stop_inspecting();
        if !self.inner.devtools_opened.swap(false, Ordering::SeqCst) {
            return;
        }
        self.inner.worlds.forget_all();
        self.inner.empty_clipboard();
        let writes = std::mem::take(&mut *lock(&self.inner.devtools_writes));
        if !writes.is_empty() {
            let mut held = Kept::default();
            held.run(&writes);
            lock(&self.inner.state).keep_out_everywhere(&held);
        }
        let targets: Vec<String> = lock(&self.inner.state)
            .pages
            .iter()
            .map(|page| page.target.clone())
            .collect();
        for target in targets {
            self.every_field_touched(&target);
        }
    }

    /// Every field of `target`'s page counts as one the person typed into: read again and kept
    /// out at the hand-back. A page whose trees cannot be read keeps what was typed and written.
    fn every_field_touched(&self, target: &str) {
        let Ok(trees) = self.trees_of(target) else {
            return;
        };
        let mut typing = lock(&self.typing);
        typing.dirty.insert(target.to_owned());
        for frame in &trees {
            for node in frame.nodes.as_array().into_iter().flatten() {
                let role = node
                    .pointer("/role/value")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let editable = node
                    .get("properties")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .any(|p| p["name"] == "editable");
                let Some(backend) = node.get("backendDOMNodeId").and_then(Value::as_i64) else {
                    continue;
                };
                if FIELD_ROLES.contains(&role) || editable {
                    typing.touched.insert((
                        target.to_owned(),
                        NodeKey {
                            frame: frame.id.clone(),
                            backend,
                        },
                    ));
                }
            }
        }
    }
}

impl Inner {
    /// The open Inspect whose front end's session is `session`.
    pub(crate) fn inspection_on(&self, session: &str) -> Option<Arc<Inspection>> {
        lock(&self.inspection)
            .clone()
            .filter(|open| open.front_session == session)
    }

    /// The open Inspect, if any.
    pub(crate) fn inspection(&self) -> Option<Arc<Inspection>> {
        lock(&self.inspection).clone()
    }

    /// Ends the open Inspect, if any, telling why.
    pub(crate) fn stop_inspecting(&self, why: InspectEnded) {
        let Some(open) = lock(&self.inspection).take() else {
            return;
        };
        if open.ended.swap(true, Ordering::SeqCst) {
            return;
        }
        self.tear_down(&open);
        if let Some(tell) = lock(&open.tell).take() {
            tell(why);
        }
    }

    /// Takes down what `open` made: the page's session detached before it is untapped, so none
    /// of its last words reach the driver; the front end's page closed and its context thrown
    /// away. The context stays DevTools' to the driver, so nothing it might still open is ever
    /// a page.
    pub(crate) fn tear_down(&self, open: &Inspection) {
        open.ended.store(true, Ordering::SeqCst);
        *lock(&open.sink) = None;
        let alive = !self.ended();
        if alive && let Some(root) = open.root() {
            let _ = self.devtools.call(
                "Target.detachFromTarget",
                json!({ "sessionId": root }),
                None,
            );
        }
        if let Some(tap) = lock(&open.tap).take() {
            self.devtools.untap(&tap);
        }
        if alive {
            let _ = self.devtools.call(
                "Target.closeTarget",
                json!({ "targetId": open.front_end }),
                None,
            );
        }
        if let Some(tap) = lock(&open.front_tap).take() {
            self.devtools.untap(&tap);
        }
        if alive {
            let _ = self.devtools.call(
                "Target.disposeBrowserContext",
                json!({ "browserContextId": open.context }),
                None,
            );
        }
        lock(&self.state)
            .inspecting
            .bridging
            .remove(&open.inspected);
    }

    /// Empties the engine's clipboard (#2028): what the person copied in DevTools is not the
    /// agent's to paste. Done in a blank page of a context of DevTools' own, thrown away after.
    pub(crate) fn empty_clipboard(&self) {
        if self.ended() {
            return;
        }
        let devtools = &self.devtools;
        let Ok(context) = devtools.call("Target.createBrowserContext", json!({}), None) else {
            return;
        };
        let Some(context) = context["browserContextId"].as_str().map(str::to_owned) else {
            return;
        };
        lock(&self.state)
            .inspecting
            .contexts
            .insert(context.clone());
        let page = devtools
            .call(
                "Target.createTarget",
                json!({ "url": "about:blank", "browserContextId": context, "newWindow": true }),
                None,
            )
            .ok()
            .and_then(|made| made["targetId"].as_str().map(str::to_owned));
        if let Some(page) = &page {
            lock(&self.state)
                .inspecting
                .front_ends
                .insert(context.clone(), page.clone());
            let session = devtools
                .call(
                    "Target.attachToTarget",
                    json!({ "targetId": page, "flatten": true }),
                    None,
                )
                .ok()
                .and_then(|attached| attached["sessionId"].as_str().map(str::to_owned));
            if let Some(session) = session {
                let _ = devtools.call(
                    "Runtime.evaluate",
                    json!({ "expression": EMPTY_CLIPBOARD, "userGesture": true, "returnByValue": true }),
                    Some(&session),
                );
            }
            let _ = devtools.call("Target.closeTarget", json!({ "targetId": page }), None);
        }
        let _ = devtools.call(
            "Target.disposeBrowserContext",
            json!({ "browserContextId": context }),
            None,
        );
    }

    /// The origins of `target`'s page's frames: what DevTools of it may reach.
    fn origins_of(&self, target: &str) -> HashSet<String> {
        let state = lock(&self.state);
        state
            .page(target)
            .map(|page| {
                page.frames
                    .values()
                    .map(|frame| frame.url.as_str())
                    .chain(std::iter::once(page.url.as_str()))
                    .filter_map(allowed::origin_root)
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// The Inspect's own thread: hands the page's messages to the front end, a batch at a time, in
/// order; sends its messages on to the page; acknowledges its frames; ends it once it falls
/// behind or is told to.
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
        let stop = |why| inner.stop_inspecting(why);
        if inner.ended() {
            return stop(InspectEnded::Ended);
        }
        if open.behind.load(Ordering::SeqCst) {
            return stop(InspectEnded::FellBehind);
        }
        let mut work = match next {
            Ok(work) => Some(work),
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => return,
        };
        while let Some(one) = work.take() {
            match one {
                Work::Stop(why) => return stop(why),
                Work::Page(Tapped::TooLarge) | Work::Front(Tapped::TooLarge) => {
                    return stop(InspectEnded::TooLarge);
                }
                Work::Page(tapped) => {
                    if let Tapped::Event(event) = &tapped {
                        if event.method == "Target.detachedFromTarget"
                            && event.params["sessionId"].as_str() == open.root().as_deref()
                        {
                            return stop(InspectEnded::PageGone);
                        }
                        if ours(open, event) {
                            continue;
                        }
                    }
                    match for_the_front_end(tapped, open.root().as_deref()) {
                        Some(message) => {
                            bytes += message.len();
                            batch.push(message);
                        }
                        None => return stop(InspectEnded::TooLarge),
                    }
                }
                Work::Front(Tapped::Event(event)) => {
                    if let Some(why) = front_end_event(&inner, open, &event, &mut batch, &mut bytes)
                    {
                        return stop(why);
                    }
                }
                Work::Front(Tapped::Answer { .. }) => {}
                Work::Show => {
                    let started = inner.devtools.call(
                        "Page.startScreencast",
                        json!({ "format": "jpeg", "quality": super::super::screencast::QUALITY, "everyNthFrame": 1 }),
                        Some(&open.front_session),
                    );
                    if let Err(why) = started {
                        open.noted(&format!("Page.startScreencast refused: {why}"));
                    }
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
        if batch.is_empty() {
            continue;
        }
        // Handed over as a value, never written into code; held while the front end has no
        // world to take it, and given up on once more waits than a batch holds.
        let Some(context) = *lock(&open.main_context) else {
            if bytes > MOST_RELAYED_BYTES {
                return stop(InspectEnded::FellBehind);
            }
            continue;
        };
        let handed = inner.devtools.call(
            "Runtime.callFunctionOn",
            json!({
                "functionDeclaration": DELIVER,
                "executionContextId": context,
                "arguments": [{ "value": batch }],
            }),
            Some(&open.front_session),
        );
        if let Err(why) = handed {
            open.noted(&format!("Runtime.callFunctionOn refused: {why}"));
        }
        batch.clear();
        bytes = 0;
    }
}

/// Notes how the front end runs, for the host's log and the tests: whether the engine's
/// embedder interface reaches it, wrapped, and which of its asks were answered with nothing.
fn probe(inner: &Inner, open: &Inspection) {
    let found = inner.devtools.call(
        "Runtime.evaluate",
        json!({ "expression": HOSTED, "returnByValue": true }),
        Some(&open.front_session),
    );
    if let Ok(found) = found
        && let Some(found) = found.pointer("/result/value").and_then(Value::as_str)
    {
        open.noted(&format!("front end: {found}"));
    }
}

/// Whether `event` tells of one of purlis's own worlds in the page: kept, and never told to
/// DevTools.
fn ours(open: &Inspection, event: &crate::cdp::Event) -> bool {
    match event.method.as_str() {
        "Runtime.executionContextCreated" => {
            let context = &event.params["context"];
            if context["name"] != super::world::WORLD_NAME
                || context["auxData"]["type"] != "isolated"
            {
                return false;
            }
            if let Some(id) = context["id"].as_i64() {
                lock(&open.ours).insert(id);
            }
            if let Some(unique) = context["uniqueId"].as_str() {
                lock(&open.ours_unique).insert(unique.to_owned());
            }
            true
        }
        "Runtime.executionContextDestroyed" => {
            let id = event.params["executionContextId"].as_i64();
            let unique = event.params["executionContextUniqueId"].as_str();
            id.is_some_and(|id| lock(&open.ours).contains(&id))
                || unique.is_some_and(|unique| lock(&open.ours_unique).contains(unique))
        }
        _ => false,
    }
}

/// Takes in an event of the front end's own session: its calls to the page, its frames, its
/// documents and worlds. Answers why the Inspect ends, where it does.
fn front_end_event(
    inner: &Inner,
    open: &Inspection,
    event: &crate::cdp::Event,
    batch: &mut Vec<String>,
    bytes: &mut usize,
) -> Option<InspectEnded> {
    let params = &event.params;
    open.noted(&event.method);
    match event.method.as_str() {
        "Runtime.bindingCalled" if params["name"] == BINDING => {
            // Only the front end's own main world speaks for it.
            let from = params["executionContextId"].as_i64();
            if from.is_none() || from != *lock(&open.main_context) {
                open.noted("a call from another world was not taken");
                return None;
            }
            if let Some(payload) = params["payload"].as_str()
                && let Some(refused) = to_the_page(inner, open, payload)
            {
                *bytes += refused.len();
                batch.push(refused);
            }
        }
        "Runtime.executionContextCreated" => {
            let context = &params["context"];
            if context["auxData"]["isDefault"] == true
                && context["auxData"]["frameId"].as_str() == Some(open.front_end.as_str())
            {
                *lock(&open.main_context) = context["id"].as_i64();
                open.noted("its main world was made");
            }
        }
        "Runtime.executionContextsCleared" => *lock(&open.main_context) = None,
        "Runtime.executionContextDestroyed" => {
            let gone = params["executionContextId"].as_i64();
            let mut main = lock(&open.main_context);
            if gone.is_some() && *main == gone {
                *main = None;
            }
        }
        "Page.screencastFrame" => {
            let (Some(data), Some(ack)) = (
                params.get("data").and_then(Value::as_str),
                params.get("sessionId").and_then(Value::as_i64),
            ) else {
                return None;
            };
            if let Some(sink) = lock(&open.sink).clone()
                && let Ok(jpeg) = base64::engine::general_purpose::STANDARD.decode(data)
            {
                sink(Painted {
                    aim: open.aim(),
                    address: String::new(),
                    jpeg,
                });
            }
            let _ = inner.devtools.call(
                "Page.screencastFrameAck",
                json!({ "sessionId": ack }),
                Some(&open.front_session),
            );
            if ack < 4 {
                probe(inner, open);
            }
        }
        "Page.frameNavigated" if params.pointer("/frame/parentId").is_none() => {
            open.shows.fetch_add(1, Ordering::SeqCst);
        }
        "Page.loadEventFired" => {
            if open.showing.load(Ordering::SeqCst) {
                let _ = inner.devtools.call(
                    "Page.startScreencast",
                    json!({ "format": "jpeg", "quality": super::super::screencast::QUALITY, "everyNthFrame": 1 }),
                    Some(&open.front_session),
                );
            }
            probe(inner, open);
        }
        "Target.detachedFromTarget"
            if params["sessionId"].as_str() == Some(open.front_session.as_str()) =>
        {
            return Some(InspectEnded::Ended);
        }
        _ => {}
    }
    None
}

/// A message of the page's, as the front end reads it: on the session it was given (the page's
/// own named by none). An answer too large to hand on is answered as an error; any other
/// message too large is `None`.
fn for_the_front_end(tapped: Tapped, root: Option<&str>) -> Option<String> {
    let message = match tapped {
        Tapped::Event(event) => {
            let mut message = json!({ "method": event.method, "params": event.params });
            if let Some(session) = event.session_id.filter(|s| Some(s.as_str()) != root) {
                message["sessionId"] = Value::String(session);
            }
            let message = message.to_string();
            return (message.len() <= MOST_RELAYED_BYTES).then_some(message);
        }
        Tapped::Answer { id, outcome } => {
            let message = match outcome {
                Ok(result) => json!({ "id": id, "result": result }),
                Err(error) => json!({ "id": id, "error": error }),
            }
            .to_string();
            if message.len() <= MOST_RELAYED_BYTES {
                return Some(message);
            }
            json!({ "id": id, "error": { "code": -32000, "message": "the answer was too large for purlis's DevTools to show" } })
        }
        Tapped::TooLarge => return None,
    };
    Some(message.to_string())
}

/// Sends a message of the front end's on to the page: answers the refusal to hand back where it
/// is not one the page is sent.
fn to_the_page(inner: &Inner, open: &Inspection, payload: &str) -> Option<String> {
    let refused = |id: u64, why: &str| {
        Some(json!({ "id": id, "error": { "code": -32601, "message": why } }).to_string())
    };
    if payload.len() > MOST_RELAYED_BYTES {
        return None;
    }
    let message: Value = serde_json::from_str(payload).ok()?;
    let id = message.get("id").and_then(Value::as_u64)?;
    let Some(method) = message.get("method").and_then(Value::as_str) else {
        return refused(id, "a message with no method");
    };
    let params = message.get("params").cloned().unwrap_or_else(|| json!({}));
    let origins = inner.origins_of(&open.inspected);
    let ours = lock(&open.ours).clone();
    let ours_unique = lock(&open.ours_unique).clone();
    let reach = allowed::Reach {
        origins: &origins,
        ours: &ours,
        ours_unique: &ours_unique,
    };
    if let Some(why) = allowed::refused(method, &params, &reach) {
        if let Some(name) = crate::cdp::method_name(method) {
            open.noted(&format!("refused {name}"));
        }
        return refused(id, &why);
    }
    {
        let mut heard = lock(&open.heard);
        if heard.len() < MOST_HEARD {
            heard.push(method.to_owned());
        }
    }
    let wrote = written::written(method, &params);
    if !wrote.is_empty() {
        lock(&inner.devtools_writes).extend(wrote);
    }
    let root = open.root()?;
    let session = message
        .get("sessionId")
        .and_then(Value::as_str)
        .unwrap_or(&root);
    let tap = lock(&open.tap);
    let tap = tap.as_ref()?;
    match inner.devtools.send_tapped(tap, id, method, params, session) {
        Ok(()) => None,
        Err(why) => refused(id, &why.to_string()),
    }
}

#[cfg(test)]
mod stand_in_tests {
    use super::{EMBEDDER_ALLOWED, stand_in};

    #[test]
    fn the_stand_in_carries_the_embedder_asks_it_lets_through_and_no_other_name() {
        let source = stand_in();
        assert!(!source.contains("EMBEDDER_ALLOWED"));
        assert!(source.contains(&serde_json::to_string(EMBEDDER_ALLOWED).unwrap()));
        for never in [
            "loadNetworkResource",
            "openInNewTab",
            "save",
            "addFileSystem",
            "setDevicesDiscoveryConfig",
        ] {
            assert!(!EMBEDDER_ALLOWED.contains(&never), "{never}");
        }
    }
}
