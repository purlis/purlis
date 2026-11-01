// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The driver against a stand-in engine that answers DevTools as the real one does, from the
//! recorded trees of the fixture site's frames page. What only a real engine can show (that the
//! page really changed) is in `tests/it/the_fixture_sites_hard_pages_pass.rs`.

use std::collections::HashMap;
use std::sync::mpsc::{self, SyncSender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine as _;
use serde_json::{Value, json};

use super::{
    ACTIVE_ELEMENT, Browser, DevTools, FOCUSED_HERE, READ_VALUE, READY_STATE, SELECT_OPTIONS, TITLE,
};
use crate::WebAddress;
use crate::cdp::{CallError, Closed, Event};
use crate::drive::{
    Answer, Call, CallId, Content, DragTo, Field, Limits, PageId, Ref, Refusal, Settled,
};
use crate::ending::{EndedBy, Ending};

mod aim;
mod drawn;
mod files;
mod inspect;
mod login;
mod reach;
mod reading;
mod takeover;

const MAIN_FRAME: &str = include_str!("../../../tests/fixtures/devtools/ax-main-frame.json");
const PAYMENT_FRAME: &str = include_str!("../../../tests/fixtures/devtools/ax-payment-frame.json");

/// One call the driver made.
#[derive(Debug, Clone)]
struct Sent {
    method: String,
    params: Value,
    session: Option<String>,
}

type Hook = Box<dyn FnMut(&Sent, &Engine) -> Option<Result<Value, CallError>> + Send>;

/// What the stand-in engine's page is.
struct Model {
    ax: HashMap<String, Value>,
    /// Content quads by backend node id.
    quads: HashMap<i64, Vec<f64>>,
    ready: String,
    /// The title each page's document has now, by target: what the engine last told of, unless
    /// a test sets it with no word from the engine (#2753).
    titles: HashMap<String, String>,
    /// The element that has the focus, by its engine id, where the document has the focus
    /// (#2295).
    focused_backend: Option<i64>,
    /// Whether a box the pointer passes through (`pointer-events: none`) of the page's own
    /// document lies over the payment frame (#2638).
    overlay: bool,
    /// Whether the payment frame's element says no frame it holds (#2638).
    frame_unnamed: bool,
    /// The elements inside each shadow host's shadow root, by the host's engine id (#2972).
    shadow_of: HashMap<i64, Vec<i64>>,
    history: Value,
    screenshot: Vec<u8>,
    select: Value,
    /// What a field the person typed into holds, as its DOM says it (#2014).
    value: String,
    /// What the document says has the focus: a tag name, or empty for nothing (#2014).
    active: String,
    pages: u32,
    pixel_ratio: f64,
    /// How many nodes the page's process holds, as `Memory.getDOMCounters` counts them (#2564).
    nodes: u64,
    /// What each session's documents draw, as `DOMSnapshot.captureSnapshot` answers (#2303);
    /// a session with none draws nothing.
    drawn: HashMap<String, Value>,
    /// The address each page the stand-in was told of was opened at: the document its frame
    /// tree says it shows.
    opened_at: HashMap<String, String>,
    /// Whether the engine makes worlds without telling of them (#2138).
    worlds_untold: bool,
    /// Whether what a drag presses on is a native drag source (#2136): moved with the button
    /// held, it starts the engine's own drag, which the stand-in hands over once drags are
    /// intercepted.
    native_drag: bool,
    /// Whether the engine hands its drags over rather than carrying them (#2136).
    intercepting: bool,
    /// Whether the engine's own drag has started.
    dragging: bool,
    /// What the engine's own drag carries (#2136).
    drag_data: Value,
    /// Whether the engine holds its word that its own drag started until the test tells it to
    /// say it ([`Engine::tell_of_the_held_drag`]), rather than saying it with the move that
    /// starts it.
    drag_told_when_asked: bool,
    /// The word of the engine's own drag, held until the test asks for it.
    held_drag: Option<Event>,
    /// How long a read of a frame's whole tree takes, where it takes long: as on a large page
    /// (#3043). A call that waits less than that fails as the connection's does, and the read
    /// it asked for runs on.
    ax_takes: Option<Duration>,
    /// How long a call of each method named takes, past its limit failing as not answered in
    /// time (#3277).
    slow: HashMap<String, Duration>,
    /// The element that has the focus of each session's document, where a test says it apart
    /// from `focused_backend` (#3043): `None` where that document does not have the focus.
    focused_on: HashMap<String, Option<i64>>,
}

/// A stand-in engine: answers the driver's calls and sends it events, as the real one would.
struct Engine {
    events: Mutex<SyncSender<Event>>,
    sent: Mutex<Vec<Sent>>,
    model: Mutex<Model>,
    hook: Mutex<Option<Hook>>,
    dropped: Mutex<u64>,
    /// The worlds of purlis's own made in the page's frames. The page's own world lies about
    /// everything purlis's scripts read (#2138).
    worlds: Mutex<Vec<StandWorld>>,
    /// Ids of ended worlds that now name the page's own world, by session: what a process the
    /// page moved to may give the same number.
    reused: Mutex<Vec<(String, i64)>>,
    /// Why its connection closed, as the real one says it (#2745).
    closed: Mutex<Option<Closed>>,
    /// Its taps (#2028): each one's number, its sessions, and where its messages go.
    taps: Mutex<Vec<(u64, std::collections::HashSet<String>, crate::cdp::TapSink)>>,
}

/// A world of purlis's own the stand-in made.
struct StandWorld {
    id: i64,
    unique: String,
    session: String,
    live: bool,
}

impl Engine {
    fn emit(&self, method: &str, params: Value, session: Option<&str>) {
        if matches!(method, "Target.targetCreated" | "Target.targetInfoChanged")
            && let (Some(target), Some(title)) = (
                params
                    .pointer("/targetInfo/targetId")
                    .and_then(Value::as_str),
                params.pointer("/targetInfo/title").and_then(Value::as_str),
            )
        {
            super::lock(&self.model)
                .titles
                .insert(target.to_owned(), title.to_owned());
        }
        if method == "Target.targetCreated"
            && let (Some(target), Some(url)) = (
                params
                    .pointer("/targetInfo/targetId")
                    .and_then(Value::as_str),
                params.pointer("/targetInfo/url").and_then(Value::as_str),
            )
        {
            super::lock(&self.model)
                .opened_at
                .insert(target.to_owned(), url.to_owned());
        }
        let event = Event {
            method: method.to_owned(),
            params,
            session_id: session.map(str::to_owned),
        };
        // A tapped session's word goes to its tap, as the connection hands it (#2028).
        {
            let mut taps = super::lock(&self.taps);
            let gone = (method == "Target.detachedFromTarget")
                .then(|| event.params["sessionId"].as_str().map(str::to_owned))
                .flatten();
            if let Some(tap) = taps.iter_mut().find(|(_, sessions, _)| {
                session.is_some_and(|s| sessions.contains(s))
                    || gone.as_ref().is_some_and(|g| sessions.contains(g))
            }) {
                if method == "Target.attachedToTarget"
                    && let Some(under) = event.params["sessionId"].as_str()
                {
                    tap.1.insert(under.to_owned());
                }
                if let Some(gone) = &gone {
                    tap.1.remove(gone);
                }
                (tap.2)(crate::cdp::Tapped::Event(event));
                return;
            }
        }
        let _ = super::lock(&self.events).send(event);
    }

    /// Says, now, that the engine's own drag started: the word it held back
    /// (`drag_told_when_asked`). Fails the test when no drag has started.
    fn tell_of_the_held_drag(&self) {
        let told = super::lock(&self.model)
            .held_drag
            .take()
            .expect("the engine's own drag started");
        let _ = super::lock(&self.events).send(told);
    }

    fn sent(&self, method: &str) -> Vec<Sent> {
        super::lock(&self.sent)
            .iter()
            .filter(|s| s.method == method)
            .cloned()
            .collect()
    }

    fn all_sent(&self) -> Vec<Sent> {
        super::lock(&self.sent).clone()
    }

    /// Every world of purlis's own ends, and its number now names the page's own world, with no
    /// word of it yet: as when the page moves to another process.
    fn worlds_reused_by_the_page(&self) {
        let mut reused = super::lock(&self.reused);
        for world in super::lock(&self.worlds).iter_mut() {
            world.live = false;
            reused.push((world.session.clone(), world.id));
        }
    }

    fn no_context() -> CallError {
        CallError::Engine {
            code: -32000,
            message: "Cannot find context with specified id".into(),
        }
    }

    /// The world a unique id names on `session`: `None` for the page's own world (none named),
    /// refused for one the engine does not have.
    fn by_unique(&self, unique: &Value, session: Option<&str>) -> Result<Option<i64>, CallError> {
        let Some(unique) = unique.as_str() else {
            return Ok(None);
        };
        super::lock(&self.worlds)
            .iter()
            .find(|w| w.live && w.unique == unique && Some(w.session.as_str()) == session)
            .map(|w| Some(w.id))
            .ok_or_else(Self::no_context)
    }

    /// The world a number names on `session`: `None` for the page's own world, refused for one
    /// the engine does not have.
    fn by_id(&self, id: &Value, session: Option<&str>) -> Result<Option<i64>, CallError> {
        let Some(id) = id.as_i64() else {
            return Ok(None);
        };
        let session = session.unwrap_or_default();
        if super::lock(&self.worlds)
            .iter()
            .any(|w| w.live && w.id == id && w.session == session)
        {
            return Ok(Some(id));
        }
        if super::lock(&self.reused)
            .iter()
            .any(|(s, reused)| s == session && *reused == id)
        {
            return Ok(None);
        }
        Err(Self::no_context())
    }

    /// The element that has the focus of `session`'s document, where it has the focus.
    fn focused_on(&self, session: Option<&str>) -> Option<i64> {
        let model = super::lock(&self.model);
        match session.and_then(|session| model.focused_on.get(session)) {
            Some(focused) => *focused,
            None => model.focused_backend,
        }
    }

    fn on(
        &self,
        hook: impl FnMut(&Sent, &Engine) -> Option<Result<Value, CallError>> + Send + 'static,
    ) {
        *super::lock(&self.hook) = Some(Box::new(hook));
    }

    fn answer(&self, sent: &Sent) -> Result<Value, CallError> {
        let p = &sent.params;
        let on = sent.session.as_deref();
        match sent.method.as_str() {
            "Target.setDiscoverTargets" => {
                self.emit(
                    "Target.targetCreated",
                    json!({"targetInfo":{"targetId":"PAGE-1","type":"page","title":"Frames and fields","url":"http://fixture.test/frames.html","attached":false,"canAccessOpener":false,"browserContextId":"CONTEXT-1"}}),
                    None,
                );
                Ok(json!({}))
            }
            "Target.attachToTarget" => {
                let target = p["targetId"].as_str().unwrap();
                let session = format!("S-{target}");
                self.emit(
                    "Target.attachedToTarget",
                    json!({"sessionId":session,"targetInfo":{"targetId":target,"type":"page","title":"","url":"","attached":true},"waitingForDebugger":false}),
                    None,
                );
                Ok(json!({ "sessionId": session }))
            }
            "Target.setAutoAttach" if on == Some("S-PAGE-1") => {
                self.emit(
                    "Target.attachedToTarget",
                    json!({"sessionId":"S-PAYMENT","targetInfo":{"targetId":"PAYMENT-FRAME","type":"iframe","title":"","url":"http://other.test/pay.html","attached":true},"waitingForDebugger":true}),
                    Some("S-PAGE-1"),
                );
                Ok(json!({}))
            }
            "Page.getFrameTree" => Ok(match on {
                Some("S-PAGE-1") => json!({"frameTree":{
                    "frame":{"id":"PAGE-1","loaderId":"LOADER-1","url":"http://fixture.test/frames.html","domainAndRegistry":"","securityOrigin":"http://fixture.test","mimeType":"text/html","name":""},
                    "childFrames":[{"frame":{"id":"PAYMENT-FRAME","parentId":"PAGE-1","loaderId":"LOADER-2","name":"payment","url":"http://other.test/pay.html","securityOrigin":"http://other.test","mimeType":"text/html"}}]}}),
                Some("S-PAYMENT") => json!({"frameTree":{
                    "frame":{"id":"PAYMENT-FRAME","parentId":"PAGE-1","loaderId":"LOADER-2","name":"payment","url":"http://other.test/pay.html","securityOrigin":"http://other.test","mimeType":"text/html"}}}),
                Some(other) => {
                    let id = other.trim_start_matches("S-");
                    let url = super::lock(&self.model)
                        .opened_at
                        .get(id)
                        .filter(|url| !url.is_empty())
                        .cloned()
                        .unwrap_or_else(|| "http://fixture.test/new.html".to_owned());
                    json!({"frameTree":{"frame":{"id":id,"loaderId":"L","url":url,"name":""}}})
                }
                None => json!({}),
            }),
            // The tree around one element (#2967): it, the nodes around it up to the root, its
            // siblings and its children, as the engine answers with `fetchRelatives`.
            "Accessibility.getPartialAXTree" => {
                let frame = on.unwrap_or_default().trim_start_matches("S-");
                let frame = if frame == "PAYMENT" {
                    "PAYMENT-FRAME"
                } else {
                    frame
                };
                let model = super::lock(&self.model);
                let nodes = model.ax.get(frame).cloned().unwrap_or(json!([]));
                let nodes = nodes.as_array().cloned().unwrap_or_default();
                let by_id = |id: &Value| nodes.iter().find(|n| n["nodeId"] == *id).cloned();
                let Some(node) = nodes
                    .iter()
                    .find(|n| n["backendDOMNodeId"] == p["backendNodeId"])
                    .cloned()
                else {
                    return Err(CallError::Engine {
                        code: -32000,
                        message: "Node not found".into(),
                    });
                };
                let mut out = vec![node.clone()];
                let mut up = node["parentId"].clone();
                while let Some(parent) = by_id(&up) {
                    up = parent["parentId"].clone();
                    for sibling in parent["childIds"].as_array().into_iter().flatten() {
                        if let Some(sibling) = by_id(sibling)
                            && !out.iter().any(|n| n["nodeId"] == sibling["nodeId"])
                        {
                            out.push(sibling);
                        }
                    }
                    out.push(parent);
                }
                for child in node["childIds"].as_array().into_iter().flatten() {
                    if let Some(child) = by_id(child) {
                        out.push(child);
                    }
                }
                Ok(json!({ "nodes": out }))
            }
            "Accessibility.getFullAXTree" => {
                let frame = p["frameId"].as_str().unwrap_or_default();
                let model = super::lock(&self.model);
                Ok(json!({ "nodes": model.ax.get(frame).cloned().unwrap_or(json!([])) }))
            }
            "DOM.getFrameOwner" => Ok(json!({"backendNodeId": 50, "nodeId": 0})),
            // The payment frame's element is drawn at (100, 200), 200 by 60, as its box model
            // says; anywhere else is the page's own document (#2638).
            "DOM.getNodeForLocation" => {
                let (x, y) = (
                    p["x"].as_f64().unwrap_or(-1.0),
                    p["y"].as_f64().unwrap_or(-1.0),
                );
                let through = p["ignorePointerEventsNone"] == true;
                Ok(
                    if (100.0..300.0).contains(&x)
                        && (200.0..260.0).contains(&y)
                        && super::lock(&self.model).overlay
                        && through
                    {
                        // Asked to find boxes the pointer passes through, it finds the overlay.
                        json!({"backendNodeId": 3, "frameId": "PAGE-1"})
                    } else if (100.0..300.0).contains(&x) && (200.0..260.0).contains(&y) {
                        json!({"backendNodeId": 50, "frameId": "PAGE-1"})
                    } else {
                        json!({"backendNodeId": 3, "frameId": "PAGE-1"})
                    },
                )
            }
            // A shadow host described with everything inside it, its shadow roots pierced.
            "DOM.describeNode"
                if p["pierce"] == true
                    && p["backendNodeId"].as_i64().is_some_and(|host| {
                        super::lock(&self.model).shadow_of.contains_key(&host)
                    }) =>
            {
                let host = p["backendNodeId"].as_i64().unwrap();
                let inside: Vec<Value> = super::lock(&self.model).shadow_of[&host]
                    .iter()
                    .map(|b| json!({"nodeId":0,"backendNodeId":b,"nodeType":1,"nodeName":"INPUT"}))
                    .collect();
                Ok(
                    json!({"node":{"nodeId":0,"backendNodeId":host,"nodeType":1,"nodeName":"X-FIELD",
                    "shadowRoots":[{"nodeId":0,"backendNodeId":host + 100_000,"nodeType":11,"nodeName":"#document-fragment","shadowRootType":"closed","children":inside}]}}),
                )
            }
            "DOM.describeNode"
                if p["objectId"].is_null()
                    && p["backendNodeId"] == 50
                    && super::lock(&self.model).frame_unnamed =>
            {
                Ok(
                    json!({"node":{"nodeId":0,"backendNodeId":50,"nodeType":1,"nodeName":"IFRAME","localName":"iframe","nodeValue":""}}),
                )
            }
            "DOM.describeNode" if p["objectId"].is_null() => Ok(if p["backendNodeId"] == 50 {
                json!({"node":{"nodeId":0,"backendNodeId":50,"nodeType":1,"nodeName":"IFRAME","localName":"iframe","nodeValue":"","frameId":"PAYMENT-FRAME"}})
            } else {
                json!({"node":{"nodeId":0,"backendNodeId":p["backendNodeId"],"nodeType":1,"nodeName":"DIV","localName":"div","nodeValue":""}})
            }),
            "DOM.getBoxModel" => Ok(
                json!({"model":{"content":[100,200,300,200,300,260,100,260],"padding":[100,200,300,200,300,260,100,260],"border":[100,200,300,200,300,260,100,260],"margin":[100,200,300,200,300,260,100,260],"width":200,"height":60}}),
            ),
            "DOM.getContentQuads" => {
                let backend = p["backendNodeId"].as_i64().unwrap();
                match super::lock(&self.model).quads.get(&backend) {
                    Some(quad) => Ok(json!({ "quads": [quad] })),
                    None => Err(CallError::Engine {
                        code: -32000,
                        message: "Could not compute content quads.".into(),
                    }),
                }
            }
            "Page.createIsolatedWorld" => {
                let id = {
                    let mut worlds = super::lock(&self.worlds);
                    let id = 100 + i64::try_from(worlds.len()).unwrap();
                    worlds.push(StandWorld {
                        id,
                        unique: format!("U-{id}"),
                        session: on.unwrap_or_default().to_owned(),
                        live: true,
                    });
                    id
                };
                if super::lock(&self.model).worlds_untold {
                    return Ok(json!({ "executionContextId": id }));
                }
                self.emit(
                    "Runtime.executionContextCreated",
                    json!({"context":{"id":id,"uniqueId":format!("U-{id}"),"origin":"","name":p["worldName"],"auxData":{"frameId":p["frameId"],"isDefault":false,"type":"isolated"}}}),
                    on,
                );
                Ok(json!({ "executionContextId": id }))
            }
            // The page's own world says what the page made it say: its document still loading,
            // nothing focused.
            "Runtime.evaluate"
                if self.by_unique(&p["uniqueContextId"], on)?.is_none()
                    && self.by_id(&p["contextId"], on)?.is_none() =>
            {
                let lie = if p["expression"] == ACTIVE_ELEMENT {
                    ""
                } else {
                    "loading"
                };
                Ok(json!({"result":{"type":"string","value":lie}}))
            }
            "Runtime.evaluate" if p["expression"] == FOCUSED_HERE => {
                Ok(match self.focused_on(on) {
                    Some(_) => {
                        json!({"result":{"type":"object","subtype":"node","objectId":"FOCUSED"}})
                    }
                    None => json!({"result":{"type":"object","subtype":"null","value":null}}),
                })
            }
            "DOM.describeNode" if p["objectId"] == "FOCUSED" => {
                let backend = self.focused_on(on);
                Ok(
                    json!({"node":{"nodeId":0,"backendNodeId":backend,"nodeType":1,"nodeName":"INPUT"}}),
                )
            }
            "Runtime.evaluate" if p["expression"] == TITLE => {
                let target = on.unwrap_or_default().trim_start_matches("S-");
                let title = super::lock(&self.model)
                    .titles
                    .get(target)
                    .cloned()
                    .unwrap_or_default();
                Ok(json!({"result":{"type":"string","value":title}}))
            }
            "Runtime.evaluate" if p["expression"] == ACTIVE_ELEMENT => {
                Ok(json!({"result":{"type":"string","value":super::lock(&self.model).active}}))
            }
            "Runtime.evaluate" => {
                Ok(json!({"result":{"type":"string","value":super::lock(&self.model).ready}}))
            }
            "Target.createTarget" => {
                let n = {
                    let mut model = super::lock(&self.model);
                    model.pages += 1;
                    model.pages
                };
                let target = format!("PAGE-{n}");
                self.emit(
                    "Target.targetCreated",
                    json!({"targetInfo":{"targetId":target,"type":"page","title":"","url":p["url"],"attached":false}}),
                    None,
                );
                Ok(json!({ "targetId": target }))
            }
            "Page.captureScreenshot" => Ok(json!({
                "data": base64::engine::general_purpose::STANDARD.encode(&super::lock(&self.model).screenshot)
            })),
            "Memory.getDOMCounters" => Ok(
                json!({"documents": 1, "nodes": super::lock(&self.model).nodes, "jsEventListeners": 0}),
            ),
            "Page.getLayoutMetrics" => {
                let ratio = super::lock(&self.model).pixel_ratio;
                Ok(json!({
                    "layoutViewport":{"pageX":0,"pageY":0,"clientWidth":800.0 * ratio,"clientHeight":600.0 * ratio},
                    "cssLayoutViewport":{"pageX":0,"pageY":0,"clientWidth":800,"clientHeight":600},
                    "cssVisualViewport":{"offsetX":0,"offsetY":0,"pageX":0,"pageY":0,"clientWidth":800,"clientHeight":600,"scale":1,"zoom":1}
                }))
            }
            "DOM.resolveNode" => {
                let object = match self.by_id(&p["executionContextId"], on)? {
                    Some(world) => format!("OBJECT-{world}"),
                    None => "PAGE-OBJECT".to_owned(),
                };
                Ok(json!({"object":{"type":"object","subtype":"node","objectId":object}}))
            }
            // A function run in a world by its unique id is given an element of that world, or
            // refused.
            "Runtime.callFunctionOn" if !p["uniqueContextId"].is_null() => {
                let world = self.by_unique(&p["uniqueContextId"], on)?;
                if p["arguments"][0]["objectId"] != json!(format!("OBJECT-{}", world.unwrap())) {
                    return Err(CallError::Engine {
                        code: -32000,
                        message:
                            "Argument should belong to the same JavaScript world as target object"
                                .into(),
                    });
                }
                Ok(if p["functionDeclaration"] == READ_VALUE {
                    json!({"result": {"type":"string","value": super::lock(&self.model).value}})
                } else {
                    json!({"result": {"type":"object","value": super::lock(&self.model).select}})
                })
            }
            // In the page's own world a field holds nothing and no drop-down is one.
            "Runtime.callFunctionOn" if p["objectId"] == "PAGE-OBJECT" => {
                Ok(if p["functionDeclaration"] == READ_VALUE {
                    json!({"result": {"type":"string","value": ""}})
                } else {
                    json!({"result": {"type":"object","value": {"notSelect": true}}})
                })
            }
            "Runtime.callFunctionOn" if p["functionDeclaration"] == READ_VALUE => {
                Ok(json!({"result": {"type":"string","value": super::lock(&self.model).value}}))
            }
            "Runtime.callFunctionOn" => {
                Ok(json!({"result": {"type":"object","value": super::lock(&self.model).select}}))
            }
            "DOMSnapshot.captureSnapshot" => {
                let session = on.unwrap_or_default();
                Ok(super::lock(&self.model)
                    .drawn
                    .get(session)
                    .cloned()
                    .unwrap_or_else(|| drawn::nothing(session.trim_start_matches("S-"))))
            }
            "Input.setInterceptDrags" => {
                super::lock(&self.model).intercepting = p["enabled"] == true;
                Ok(json!({}))
            }
            "Input.dispatchMouseEvent" => {
                let starts = {
                    let mut model = super::lock(&self.model);
                    let starts = p["type"] == "mouseMoved"
                        && p["buttons"] == 1
                        && model.native_drag
                        && model.intercepting
                        && !model.dragging;
                    model.dragging |= starts;
                    starts
                };
                if starts {
                    let mut model = super::lock(&self.model);
                    let told = Event {
                        method: "Input.dragIntercepted".into(),
                        params: json!({ "data": model.drag_data }),
                        session_id: on.map(str::to_owned),
                    };
                    if model.drag_told_when_asked {
                        model.held_drag = Some(told);
                    } else {
                        drop(model);
                        let _ = super::lock(&self.events).send(told);
                    }
                }
                Ok(json!({}))
            }
            "Page.getNavigationHistory" => Ok(super::lock(&self.model).history.clone()),
            "Page.navigate" => Ok(json!({"frameId":"PAGE-1","loaderId":"LOADER-9"})),
            _ => Ok(json!({})),
        }
    }
}

struct Stand(Arc<Engine>);

impl DevTools for Stand {
    fn call_within(
        &self,
        method: &str,
        params: Value,
        session: Option<&str>,
        timeout: Duration,
    ) -> Result<Value, CallError> {
        let sent = Sent {
            method: method.to_owned(),
            params,
            session: session.map(str::to_owned),
        };
        super::lock(&self.0.sent).push(sent.clone());
        let hooked = super::lock(&self.0.hook)
            .as_mut()
            .and_then(|h| h(&sent, &self.0));
        let answer = hooked.unwrap_or_else(|| self.0.answer(&sent));
        // The tree as it was when the read began, given when the read ends.
        let takes = super::lock(&self.0.model).ax_takes;
        if method == "Accessibility.getFullAXTree"
            && let Some(takes) = takes
        {
            std::thread::sleep(takes.min(timeout));
            if takes > timeout {
                return Err(CallError::TimedOut);
            }
        }
        let slow = super::lock(&self.0.model).slow.get(method).copied();
        if let Some(takes) = slow {
            std::thread::sleep(takes.min(timeout));
            if takes > timeout {
                return Err(CallError::TimedOut);
            }
        }
        answer
    }

    /// As the connection holds an answer to a bound: past it, the call fails, alone (#2694).
    fn call_within_bounded(
        &self,
        method: &str,
        params: Value,
        session: Option<&str>,
        timeout: Duration,
        most: usize,
    ) -> Result<Value, CallError> {
        let answer = self.call_within(method, params, session, timeout)?;
        if serde_json::to_vec(&answer).map_or(0, |bytes| bytes.len()) > most {
            return Err(CallError::TooLarge { most });
        }
        Ok(answer)
    }

    fn closed(&self) -> Option<Closed> {
        super::lock(&self.0.closed).clone()
    }

    fn dropped_events(&self) -> u64 {
        *super::lock(&self.0.dropped)
    }

    fn tap(&self, session: &str, sink: crate::cdp::TapSink) -> Option<crate::cdp::Tap> {
        let mut taps = super::lock(&self.0.taps);
        let number = taps.len() as u64 + 1;
        taps.push((number, std::iter::once(session.to_owned()).collect(), sink));
        Some(crate::cdp::Tap::for_a_test(number))
    }

    fn untap(&self, tap: &crate::cdp::Tap) {
        super::lock(&self.0.taps).retain(|(number, _, _)| *number != tap.number());
    }

    /// Answers as the stand-in answers any call, to the tap, under the tap's own id.
    fn send_tapped(
        &self,
        tap: &crate::cdp::Tap,
        id: u64,
        method: &str,
        params: Value,
        session: &str,
    ) -> Result<(), CallError> {
        let sink = super::lock(&self.0.taps)
            .iter()
            .find(|(number, sessions, _)| *number == tap.number() && sessions.contains(session))
            .map(|(_, _, sink)| Arc::clone(sink))
            .ok_or(CallError::Engine {
                code: -32001,
                message: "the session is not this tap's".into(),
            })?;
        let sent = Sent {
            method: method.to_owned(),
            params,
            session: Some(session.to_owned()),
        };
        super::lock(&self.0.sent).push(sent.clone());
        let outcome = self
            .0
            .answer(&sent)
            .map_err(|why| json!({ "code": -32000, "message": why.to_string() }));
        sink(crate::cdp::Tapped::Answer { id, outcome });
        Ok(())
    }
}

fn limits() -> Limits {
    Limits {
        quiet: Duration::from_millis(30),
        settle_within: Duration::from_millis(1500),
        ..Limits::default()
    }
}

/// A white 10 by 10 PNG.
fn white_png() -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, 10, 10);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&[255; 300]).unwrap();
    }
    out
}

/// A browser over the stand-in, with the frames page open and set up.
fn browser() -> (Browser, Arc<Engine>) {
    browser_with(limits())
}

fn browser_with(limits: Limits) -> (Browser, Arc<Engine>) {
    browser_over(limits, None)
}

/// A browser over the stand-in whose every request is told to `judge` (#2018).
fn browser_judged(judge: crate::drive::Judge) -> (Browser, Arc<Engine>) {
    browser_over(limits(), Some(judge))
}

fn browser_over(limits: Limits, judge: Option<crate::drive::Judge>) -> (Browser, Arc<Engine>) {
    let (events, receiver) = mpsc::sync_channel(1024);
    let nodes = |text: &str| serde_json::from_str::<Value>(text).unwrap()["nodes"].clone();
    let engine = Arc::new(Engine {
        events: Mutex::new(events),
        sent: Mutex::new(Vec::new()),
        model: Mutex::new(Model {
            ax: HashMap::from([
                ("PAGE-1".to_owned(), nodes(MAIN_FRAME)),
                ("PAYMENT-FRAME".to_owned(), nodes(PAYMENT_FRAME)),
            ]),
            quads: HashMap::from([
                (7, vec![10.0, 10.0, 30.0, 10.0, 30.0, 20.0, 10.0, 20.0]),
                (12, vec![40.0, 40.0, 60.0, 40.0, 60.0, 50.0, 40.0, 50.0]),
                (20, vec![0.0, 0.0, 4.0, 0.0, 4.0, 2.0, 0.0, 2.0]),
                (21, vec![2.0, 3.0, 6.0, 3.0, 6.0, 5.0, 2.0, 5.0]),
                (
                    40,
                    vec![100.0, 100.0, 200.0, 100.0, 200.0, 110.0, 100.0, 110.0],
                ),
                (60, vec![0.0, 300.0, 50.0, 300.0, 50.0, 320.0, 0.0, 320.0]),
                // Below the bottom of the 800 by 600 viewport.
                (71, vec![0.0, 900.0, 80.0, 900.0, 80.0, 920.0, 0.0, 920.0]),
            ]),
            ready: "complete".into(),
            titles: HashMap::new(),
            focused_backend: None,
            overlay: false,
            frame_unnamed: false,
            shadow_of: HashMap::new(),
            history: json!({"currentIndex":0,"entries":[{"id":1,"url":"http://fixture.test/frames.html","userTypedURL":"","title":"","transitionType":"typed"}]}),
            screenshot: white_png(),
            select: json!({"picked":["Blue"]}),
            pages: 1,
            pixel_ratio: 1.0,
            nodes: 120,
            value: String::new(),
            active: String::new(),
            drawn: HashMap::new(),
            opened_at: HashMap::new(),
            worlds_untold: false,
            native_drag: false,
            intercepting: false,
            dragging: false,
            drag_data: json!({"items":[{"mimeType":"text/plain","data":"card"}],"dragOperationsMask":1}),
            drag_told_when_asked: false,
            held_drag: None,
            ax_takes: None,
            slow: HashMap::new(),
            focused_on: HashMap::new(),
        }),
        hook: Mutex::new(None),
        dropped: Mutex::new(0),
        worlds: Mutex::new(Vec::new()),
        reused: Mutex::new(Vec::new()),
        closed: Mutex::new(None),
        taps: Mutex::new(Vec::new()),
    });
    let browser = Browser::over(Arc::new(Stand(engine.clone())), receiver, limits, judge).unwrap();
    // Set up means both sessions resumed: every frame is known by then.
    let start = std::time::Instant::now();
    while engine
        .sent("Runtime.runIfWaitingForDebugger")
        .iter()
        .filter(|s| s.session.as_deref() == Some("S-PAYMENT"))
        .count()
        == 0
    {
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "the frames were never set up"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    std::thread::sleep(Duration::from_millis(20));
    (browser, engine)
}

fn id(n: u32) -> CallId {
    CallId::new(format!("call-{n}")).unwrap()
}

fn snapshot(browser: &Browser, n: u32) -> String {
    let answer = browser
        .call(
            &id(n),
            Call::Snapshot {
                page: None,
                within: None,
                from: 1,
                lines: None,
            },
        )
        .unwrap();
    match answer.content {
        Content::Text(text) => text,
        other => panic!("a snapshot is text: {other:?}"),
    }
}

/// The reference on the line of `text` that starts with `line`.
fn ref_of(text: &str, line: &str) -> Ref {
    let found = text
        .lines()
        .find(|l| l.trim_start().starts_with(line))
        .unwrap_or_else(|| panic!("no line {line} in\n{text}"));
    let start = found.find('[').unwrap() + 1;
    let end = found[start..].find(']').unwrap() + start;
    Ref::new(&found[start..end])
}

fn mouse(engine: &Engine) -> Vec<(String, f64, f64, i64)> {
    engine
        .sent("Input.dispatchMouseEvent")
        .iter()
        .map(|s| {
            (
                s.params["type"].as_str().unwrap().to_owned(),
                s.params["x"].as_f64().unwrap(),
                s.params["y"].as_f64().unwrap(),
                s.params["clickCount"].as_i64().unwrap_or(0),
            )
        })
        .collect()
}

#[test]
fn a_snapshot_names_its_page_and_the_frames_in_it_once_the_page_has_settled() {
    let (browser, _engine) = browser();
    let text = snapshot(&browser, 1);
    assert!(
        text.starts_with("page p1 \"Frames and fields\" http://fixture.test/frames.html\n"),
        "{text}"
    );
    assert!(text.contains("- frame \"payment\" ["), "{text}");
    assert!(text.contains("  - button \"Pay now\" ["), "{text}");
}

#[test]
fn a_snapshot_marks_what_is_on_the_page_but_out_of_view() {
    let (browser, _engine) = browser();
    let text = snapshot(&browser, 1);
    let line = |words: &str| {
        text.lines()
            .find(|l| l.contains(words))
            .unwrap_or_else(|| panic!("no {words:?} in\n{text}"))
            .to_owned()
    };
    assert!(line("Below the fold").ends_with(" (out of view)"), "{text}");
    assert!(!line("Read more").contains("out of view"), "{text}");
    // An element whose place the engine cannot say is not said to be out of view.
    assert!(!line("Remember me").contains("out of view"), "{text}");
    let found = browser
        .call(
            &id(2),
            Call::Find {
                page: None,
                text: Some("fold".into()),
                role: None,
                within: None,
            },
        )
        .unwrap();
    assert!(
        matches!(&found.content, Content::Text(t) if t.contains("\"Below the fold\" [") && t.contains("(out of view)")),
        "{found:?}"
    );
}

#[test]
fn a_snapshot_marks_what_a_scrolling_box_in_view_does_not_show() {
    // #2564: "Read more" (node 12) is placed inside the viewport, but the engine lays it out
    // below the bottom of the list that scrolls it.
    let (browser, engine) = browser();
    lays_read_more_out_below_a_list(&engine);
    let text = snapshot(&browser, 1);
    let line = |words: &str| {
        text.lines()
            .find(|l| l.contains(words))
            .unwrap_or_else(|| panic!("no {words:?} in\n{text}"))
            .to_owned()
    };
    assert!(line("Read more").ends_with(" (out of view)"), "{text}");
    assert!(line("Below the fold").ends_with(" (out of view)"), "{text}");
    assert!(!line("Remember me").contains("out of view"), "{text}");
}

#[test]
fn a_page_too_heavy_to_read_whole_is_not_read_for_what_its_boxes_cut_off() {
    // #2564: past the bound the layout is never asked for, nothing is said to be cut off, and
    // the browser goes on answering.
    let (browser, engine) = browser();
    lays_read_more_out_below_a_list(&engine);
    super::lock(&engine.model).nodes = super::view::MOST_NODES_READ + 1;
    let text = snapshot(&browser, 1);
    let line = |words: &str| {
        text.lines()
            .find(|l| l.contains(words))
            .unwrap_or_else(|| panic!("no {words:?} in\n{text}"))
            .to_owned()
    };
    assert!(!line("Read more").contains("out of view"), "{text}");
    // What the viewport alone shows is still marked.
    assert!(line("Below the fold").ends_with(" (out of view)"), "{text}");
    assert!(
        !super::lock(&engine.sent)
            .iter()
            .any(|sent| sent.method == "DOMSnapshot.captureSnapshot"),
        "the layout was read"
    );
    assert!(snapshot(&browser, 2).contains("Read more"));
}

#[test]
fn a_layout_answer_past_its_bound_marks_nothing_and_the_browser_still_answers() {
    // #2694: a page whose layout answer carries more inline text than the read takes has the
    // read fail alone: nothing is said to be cut off, and the browser goes on answering.
    let (browser, engine) = browser();
    lays_read_more_out_below_a_list(&engine);
    {
        let mut model = super::lock(&engine.model);
        let drawn = model.drawn.get_mut("S-PAGE-1").unwrap();
        drawn["strings"]
            .as_array_mut()
            .unwrap()
            .push(json!("x".repeat(super::view::MOST_LAYOUT_BYTES)));
    }
    let text = snapshot(&browser, 1);
    let line = |words: &str| {
        text.lines()
            .find(|l| l.contains(words))
            .unwrap_or_else(|| panic!("no {words:?} in\n{text}"))
            .to_owned()
    };
    assert!(!line("Read more").contains("out of view"), "{text}");
    assert!(line("Below the fold").ends_with(" (out of view)"), "{text}");
    assert!(
        super::lock(&engine.sent)
            .iter()
            .any(|sent| sent.method == "DOMSnapshot.captureSnapshot"),
        "the layout was asked for"
    );
    assert!(snapshot(&browser, 2).contains("Read more"));
}

/// The stand-in's page lays "Read more" (node 12) out below the bottom of a list that scrolls.
fn lays_read_more_out_below_a_list(engine: &Engine) {
    super::lock(&engine.model).drawn.insert(
        "S-PAGE-1".to_owned(),
        json!({
            "strings": ["#document", "HTML", "BODY", "DIV", "A", "visible", "static", "auto", "block", "0px", "inline", "none"],
            "documents": [{
                "nodes": {
                    "parentIndex": [-1, 0, 1, 2, 3],
                    "nodeName": [0, 1, 2, 3, 4],
                    "backendNodeId": [1, 2, 3, 4, 12],
                },
                "layout": {
                    "nodeIndex": [1, 2, 3, 4],
                    "bounds": [[0, 0, 800, 600], [0, 0, 800, 600], [0, 0, 100, 30], [40, 40, 20, 10]],
                    "styles": [[5, 5, 6, 8, 9, 9, 9, 9, 9, 9, 9, 9, 9, 11, 11, 11, 11], [5, 5, 6, 8, 9, 9, 9, 9, 9, 9, 9, 9, 9, 11, 11, 11, 11], [7, 7, 6, 8, 9, 9, 9, 9, 9, 9, 9, 9, 9, 11, 11, 11, 11], [5, 5, 6, 10, 9, 9, 9, 9, 9, 9, 9, 9, 9, 11, 11, 11, 11]],
                },
            }],
        }),
    );
}

/// The page puts the payment frame (its element node 50, at (100, 200), 200 by 60, as its box
/// model says) in a box that cuts off what overflows it below `box_bottom`; the frame, from
/// another site and read in a session of its own, lays "Pay now" (node 7) out 10 to 20 down
/// its document (#2689).
fn boxes_in_the_payment_frame_down_to(engine: &Engine, box_bottom: f64) {
    let mut model = super::lock(&engine.model);
    model.drawn.insert(
        "S-PAGE-1".to_owned(),
        json!({
            "strings": ["#document", "HTML", "BODY", "DIV", "IFRAME", "visible", "static", "hidden", "block", "0px", "none"],
            "documents": [{
                "nodes": {
                    "parentIndex": [-1, 0, 1, 2, 3],
                    "nodeName": [0, 1, 2, 3, 4],
                    "backendNodeId": [1, 2, 3, 4, 50],
                },
                "layout": {
                    "nodeIndex": [1, 2, 3, 4],
                    "bounds": [[0, 0, 800, 600], [0, 0, 800, 600], [0, 0, 800, box_bottom], [100, 200, 200, 60]],
                    "styles": [[5, 5, 6, 8, 9, 9, 9, 9, 9, 9, 9, 9, 9, 10, 10, 10, 10], [5, 5, 6, 8, 9, 9, 9, 9, 9, 9, 9, 9, 9, 10, 10, 10, 10], [7, 7, 6, 8, 9, 9, 9, 9, 9, 9, 9, 9, 9, 10, 10, 10, 10], [5, 5, 6, 8, 9, 9, 9, 9, 9, 9, 9, 9, 9, 10, 10, 10, 10]],
                    "clientRects": [[0, 0, 800, 600], [0, 0, 800, 600], [0, 0, 800, box_bottom], [0, 0, 200, 60]],
                },
            }],
        }),
    );
    model.drawn.insert(
        "S-PAYMENT".to_owned(),
        json!({
            "strings": ["#document", "HTML", "BODY", "BUTTON", "visible", "static", "block", "0px", "inline-block", "none"],
            "documents": [{
                "nodes": {
                    "parentIndex": [-1, 0, 1, 2],
                    "nodeName": [0, 1, 2, 3],
                    "backendNodeId": [1, 2, 3, 7],
                },
                "layout": {
                    "nodeIndex": [1, 2, 3],
                    "bounds": [[0, 0, 200, 60], [0, 0, 200, 60], [10, 10, 20, 10]],
                    "styles": [[4, 4, 5, 6, 7, 7, 7, 7, 7, 7, 7, 7, 7, 9, 9, 9, 9], [4, 4, 5, 6, 7, 7, 7, 7, 7, 7, 7, 7, 7, 9, 9, 9, 9], [4, 4, 5, 8, 7, 7, 7, 7, 7, 7, 7, 7, 7, 9, 9, 9, 9]],
                    "clientRects": [[0, 0, 200, 60], [0, 0, 200, 60], [0, 0, 20, 10]],
                },
            }],
        }),
    );
}

#[test]
fn a_box_on_the_page_that_cuts_off_a_cross_site_frame_marks_what_it_hides_of_the_frame() {
    let marked = |box_bottom: f64| {
        let (browser, engine) = browser();
        boxes_in_the_payment_frame_down_to(&engine, box_bottom);
        let text = snapshot(&browser, 1);
        text.lines()
            .find(|l| l.contains("Pay now"))
            .unwrap_or_else(|| panic!("no Pay now in\n{text}"))
            .ends_with(" (out of view)")
    };
    // The box ends above the frame: none of it shows.
    assert!(marked(100.0), "the whole frame cut off");
    // It ends 5 into the frame: "Pay now", 10 down the frame, is hidden.
    assert!(marked(205.0), "the frame cut off above its button");
    // It ends 15 into the frame: "Pay now" shows in part.
    assert!(
        !marked(215.0),
        "the frame cut off below the top of its button"
    );
    // No box cuts it off.
    assert!(!marked(600.0), "the frame shown whole");
}

/// Renames the main frame's link, as a page that changed it would show it.
fn rename_the_link(engine: &Engine, to: &str) {
    let mut model = super::lock(&engine.model);
    for node in model.ax.get_mut("PAGE-1").unwrap().as_array_mut().unwrap() {
        if matches!(node["backendDOMNodeId"].as_i64(), Some(12 | 13)) {
            node["name"]["value"] = json!(to);
        }
    }
}

fn changes(browser: &Browser, n: u32) -> String {
    match browser
        .call(&id(n), Call::Changes { page: None })
        .unwrap()
        .content
    {
        Content::Text(text) => text,
        other => panic!("changes are text: {other:?}"),
    }
}

#[test]
fn changes_answer_the_lines_new_and_gone_since_the_page_was_last_read() {
    let (browser, engine) = browser();
    snapshot(&browser, 1);
    rename_the_link(&engine, "Read less");
    let changed = changes(&browser, 2);
    assert!(changed.contains("1 new, 1 gone"), "{changed}");
    assert!(
        changed
            .lines()
            .any(|l| l.starts_with("new: - link \"Read less\" [e") && l.contains("(line ")),
        "{changed}"
    );
    assert!(
        changed
            .lines()
            .any(|l| l.starts_with("gone: - link \"Read more\"") && !l.contains('[')),
        "{changed}"
    );
    assert!(
        !changed.contains("Pay now"),
        "what did not change is not said: {changed}"
    );
    // The new line's reference is one to act on.
    let less = ref_of(
        changed
            .lines()
            .find(|l| l.starts_with("new: "))
            .unwrap()
            .trim_start_matches("new: "),
        "- link",
    );
    browser.call(&id(3), Call::Hover { target: less }).unwrap();
    assert!(changes(&browser, 4).contains("Nothing changed"));
}

#[test]
fn changes_of_a_page_not_read_before_answer_all_of_it() {
    let (browser, _engine) = browser();
    let all = changes(&browser, 1);
    assert!(all.contains("not read before"), "{all}");
    assert!(all.contains("- button \"Pay now\" ["), "{all}");
}

#[test]
fn changes_are_answered_again_to_a_retry_of_the_same_call() {
    let (browser, engine) = browser();
    snapshot(&browser, 1);
    rename_the_link(&engine, "Read less");
    let first = changes(&browser, 2);
    let again = browser.call(&id(2), Call::Changes { page: None }).unwrap();
    assert!(again.repeated);
    assert_eq!(again.content, Content::Text(first));
}

#[test]
fn a_click_in_a_cross_site_frame_lands_where_the_page_shows_the_element() {
    let (browser, engine) = browser();
    let pay = ref_of(&snapshot(&browser, 1), "- button \"Pay now\"");
    let answer = browser.call(&id(2), Call::Click { target: pay }).unwrap();
    assert_eq!(answer.settled, Settled::Yes);
    // The button's centre in its frame (20, 15), moved by where the frame sits (100, 200).
    assert_eq!(
        mouse(&engine),
        vec![
            ("mouseMoved".into(), 120.0, 215.0, 0),
            ("mousePressed".into(), 120.0, 215.0, 1),
            ("mouseReleased".into(), 120.0, 215.0, 1),
        ]
    );
    assert!(
        engine
            .sent("Input.dispatchMouseEvent")
            .iter()
            .all(|s| s.session.as_deref() == Some("S-PAGE-1")),
        "input goes to the page, which routes it to the frame"
    );
}

#[test]
fn a_double_click_is_two_clicks_the_second_counted_two() {
    let (browser, engine) = browser();
    let more = ref_of(&snapshot(&browser, 1), "- link \"Read more\"");
    browser
        .call(&id(2), Call::DoubleClick { target: more })
        .unwrap();
    assert_eq!(
        mouse(&engine),
        vec![
            ("mouseMoved".into(), 50.0, 45.0, 0),
            ("mousePressed".into(), 50.0, 45.0, 1),
            ("mouseReleased".into(), 50.0, 45.0, 1),
            ("mousePressed".into(), 50.0, 45.0, 2),
            ("mouseReleased".into(), 50.0, 45.0, 2),
        ]
    );
}

#[test]
fn a_drag_by_an_offset_ends_that_far_from_where_it_started() {
    let (browser, engine) = browser();
    let volume = ref_of(&snapshot(&browser, 1), "- slider \"Volume\"");
    browser
        .call(
            &id(2),
            Call::Drag {
                from: volume,
                to: DragTo::Offset { x: 40.0, y: -5.0 },
            },
        )
        .unwrap();
    let events = mouse(&engine);
    assert_eq!(events[0], ("mouseMoved".into(), 150.0, 105.0, 0));
    assert_eq!(events[1], ("mousePressed".into(), 150.0, 105.0, 1));
    assert_eq!(
        events.last().unwrap(),
        &("mouseReleased".into(), 190.0, 100.0, 1)
    );
    let moves = &events[2..events.len() - 1];
    assert_eq!(moves.len(), 10);
    assert!(moves.iter().all(|m| m.0 == "mouseMoved"));
    assert!(
        engine.sent("Input.dispatchMouseEvent")[2..12]
            .iter()
            .all(|s| s.params["buttons"] == 1),
        "the button is held through the moves"
    );
}

#[test]
fn a_drag_of_a_native_drag_source_is_carried_by_the_engines_own_drag_events() {
    // #2136: once the engine's own drag starts, the rest of the way is its drag events, and
    // the drop lands where the drag ends.
    let (browser, engine) = browser();
    super::lock(&engine.model).native_drag = true;
    let text = snapshot(&browser, 1);
    let volume = ref_of(&text, "- slider \"Volume\"");
    let pay = ref_of(&text, "- link \"Read more\"");
    let dragged = browser.call(
        &id(2),
        Call::Drag {
            from: volume,
            to: DragTo::Element(pay),
        },
    );
    assert!(dragged.is_ok(), "{dragged:?}");
    let drags: Vec<(String, f64, f64, Value)> = engine
        .sent("Input.dispatchDragEvent")
        .iter()
        .map(|s| {
            (
                s.params["type"].as_str().unwrap().to_owned(),
                s.params["x"].as_f64().unwrap(),
                s.params["y"].as_f64().unwrap(),
                s.params["data"].clone(),
            )
        })
        .collect();
    assert_eq!(
        drags.first().map(|d| d.0.as_str()),
        Some("dragEnter"),
        "{drags:?}"
    );
    let (kind, x, y, data) = drags.last().unwrap();
    // The link's centre (50, 45).
    assert_eq!((kind.as_str(), *x, *y), ("drop", 50.0, 45.0), "{drags:?}");
    assert_eq!(data["items"][0]["data"], "card");
    assert!(
        drags[1..drags.len() - 1].iter().all(|d| d.0 == "dragOver"),
        "{drags:?}"
    );
    // The engine is no longer handed drags: a drag of the person's is its own again.
    let intercepts = engine.sent("Input.setInterceptDrags");
    assert_eq!(intercepts.first().unwrap().params["enabled"], true);
    assert_eq!(intercepts.last().unwrap().params["enabled"], false);
    assert_eq!(
        mouse(&engine).last().map(|m| m.0.clone()).as_deref(),
        Some("mouseReleased")
    );
}

#[test]
fn a_drag_of_a_native_drag_source_by_an_offset_drops_that_far_from_where_it_started() {
    let (browser, engine) = browser();
    super::lock(&engine.model).native_drag = true;
    let volume = ref_of(&snapshot(&browser, 1), "- slider \"Volume\"");
    browser
        .call(
            &id(2),
            Call::Drag {
                from: volume,
                to: DragTo::Offset { x: 40.0, y: -5.0 },
            },
        )
        .unwrap();
    let drop = engine.sent("Input.dispatchDragEvent");
    let drop = drop.last().unwrap();
    assert_eq!(drop.params["type"], "drop");
    // From the slider's centre (150, 105), 40 right and 5 up.
    assert_eq!(
        (drop.params["x"].as_f64(), drop.params["y"].as_f64()),
        (Some(190.0), Some(100.0))
    );
}

/// The kinds of the engine's own drag events sent, in order.
fn drag_kinds(engine: &Engine) -> Vec<String> {
    engine
        .sent("Input.dispatchDragEvent")
        .iter()
        .map(|s| s.params["type"].as_str().unwrap().to_owned())
        .collect()
}

/// Whether the engine was last told to stop handing its drags over.
fn drags_handed_back(engine: &Engine) -> bool {
    engine
        .sent("Input.setInterceptDrags")
        .last()
        .is_some_and(|s| s.params["enabled"] == false)
}

fn drag_volume_by(browser: &Browser, n: u32) -> Result<Answer, Refusal> {
    let volume = ref_of(&snapshot(browser, n), "- slider \"Volume\"");
    browser.call(
        &id(n + 1),
        Call::Drag {
            from: volume,
            to: DragTo::Offset { x: 40.0, y: -5.0 },
        },
    )
}

#[test]
fn drags_are_handed_back_even_when_the_engine_refused_to_hand_them_over() {
    let (browser, engine) = browser();
    engine.on(|sent, _| {
        (sent.method == "Input.setInterceptDrags" && sent.params["enabled"] == true)
            .then_some(Err(CallError::TimedOut))
    });
    assert!(drag_volume_by(&browser, 1).is_err());
    assert!(drags_handed_back(&engine));
    assert!(mouse(&engine).is_empty(), "nothing was pressed");
}

#[test]
fn a_drag_that_fails_on_the_way_is_cancelled_let_go_and_handed_back() {
    let (browser, engine) = browser();
    super::lock(&engine.model).native_drag = true;
    engine.on(|sent, _| {
        (sent.method == "Input.dispatchDragEvent" && sent.params["type"] == "drop").then_some(Err(
            CallError::Engine {
                code: -32000,
                message: "no".into(),
            },
        ))
    });
    assert!(drag_volume_by(&browser, 1).is_err());
    let kinds = drag_kinds(&engine);
    assert_eq!(
        kinds.last().map(String::as_str),
        Some("dragCancel"),
        "{kinds:?}"
    );
    assert_eq!(
        mouse(&engine).last().map(|m| m.0.clone()).as_deref(),
        Some("mouseReleased")
    );
    assert!(drags_handed_back(&engine));
}

#[test]
fn a_drag_cut_by_the_person_taking_over_is_cancelled_let_go_and_handed_back() {
    // In this order: every move is made with the button held, the person takes over, and only
    // then does the engine say its own drag started. The driver waits for that word as long as
    // the test needs, so no timing decides the order.
    let (browser, engine) = browser_with(Limits {
        drag_starts_within: Duration::from_secs(30),
        ..limits()
    });
    {
        let mut model = super::lock(&engine.model);
        model.native_drag = true;
        model.drag_told_when_asked = true;
    }
    let browser = Arc::new(browser);
    let volume = ref_of(&snapshot(&browser, 1), "- slider \"Volume\"");
    let dragging = Arc::clone(&browser);
    let call = std::thread::spawn(move || {
        dragging.call(
            &id(2),
            Call::Drag {
                from: volume,
                to: DragTo::Offset { x: 40.0, y: -5.0 },
            },
        )
    });
    let held_moves = || {
        engine
            .sent("Input.dispatchMouseEvent")
            .iter()
            .filter(|s| s.params["type"] == "mouseMoved" && s.params["buttons"] == 1)
            .count()
    };
    let started = std::time::Instant::now();
    while held_moves() < super::DRAG_STEPS {
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "the drag never made its moves"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    // Waits for the last move's answer: the lease is held while a call is in flight.
    browser.take_over();
    engine.tell_of_the_held_drag();
    assert!(call.join().unwrap().is_err());
    let kinds = drag_kinds(&engine);
    assert_eq!(kinds, ["dragCancel"], "{kinds:?}");
    assert_eq!(
        mouse(&engine).last().map(|m| m.0.clone()).as_deref(),
        Some("mouseReleased")
    );
    assert!(drags_handed_back(&engine));
}

#[test]
fn a_drag_cut_before_the_engine_tells_of_its_own_drag_cancels_that_drag_once_it_does() {
    // In this order: every move made with the button held, the person takes over, the drag
    // stops waiting for the engine's word and ends, and only then does the engine say its own
    // drag started (#2839).
    let (browser, engine) = browser_with(Limits {
        drag_starts_within: Duration::from_millis(50),
        ..limits()
    });
    {
        let mut model = super::lock(&engine.model);
        model.native_drag = true;
        model.drag_told_when_asked = true;
    }
    let browser = Arc::new(browser);
    let volume = ref_of(&snapshot(&browser, 1), "- slider \"Volume\"");
    let dragging = Arc::clone(&browser);
    let call = std::thread::spawn(move || {
        dragging.call(
            &id(2),
            Call::Drag {
                from: volume,
                to: DragTo::Offset { x: 40.0, y: -5.0 },
            },
        )
    });
    let held_moves = || {
        engine
            .sent("Input.dispatchMouseEvent")
            .iter()
            .filter(|s| s.params["type"] == "mouseMoved" && s.params["buttons"] == 1)
            .count()
    };
    let started = std::time::Instant::now();
    while held_moves() < super::DRAG_STEPS {
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "the drag never made its moves"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    browser.take_over();
    assert!(call.join().unwrap().is_err());
    assert!(
        drag_kinds(&engine).is_empty(),
        "no word yet, nothing to cancel"
    );
    assert!(drags_handed_back(&engine));
    engine.tell_of_the_held_drag();
    let told = std::time::Instant::now();
    while drag_kinds(&engine).is_empty() {
        assert!(
            told.elapsed() < Duration::from_secs(5),
            "the engine's late drag was never cancelled"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(drag_kinds(&engine), ["dragCancel"]);
    let cancel = engine.sent("Input.dispatchDragEvent");
    assert_eq!(cancel[0].params["data"]["items"][0]["data"], "card");
    assert_eq!(cancel[0].session.as_deref(), Some("S-PAGE-1"));
}

#[test]
fn a_drag_s_mark_goes_with_its_session() {
    let (browser, engine) = browser();
    super::lock(&browser.inner.state).cancels_late = Some(("S-PAYMENT".to_owned(), (1.0, 2.0)));
    engine.emit(
        "Target.detachedFromTarget",
        json!({"sessionId":"S-PAYMENT","targetId":"PAYMENT-FRAME"}),
        None,
    );
    let start = std::time::Instant::now();
    while super::lock(&browser.inner.state).cancels_late.is_some() {
        assert!(start.elapsed() < Duration::from_secs(5), "the mark stayed");
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn a_word_of_the_engine_s_drag_while_no_drag_was_asked_for_is_not_acted_on() {
    let (browser, engine) = browser();
    engine.emit(
        "Input.dragIntercepted",
        json!({"data":{"items":[],"dragOperationsMask":1}}),
        Some("S-PAGE-1"),
    );
    std::thread::sleep(Duration::from_millis(60));
    snapshot(&browser, 1);
    assert!(drag_kinds(&engine).is_empty());
}

#[test]
fn a_drag_whose_browser_ends_on_the_way_is_handed_back() {
    let (browser, engine) = browser();
    super::lock(&engine.model).native_drag = true;
    engine
        .on(|sent, _| (sent.method == "Input.dispatchDragEvent").then_some(Err(CallError::Ended)));
    assert!(drag_volume_by(&browser, 1).is_err());
    assert!(
        drags_handed_back(&engine),
        "told to stop, whatever the engine answers"
    );
    assert_eq!(
        drag_kinds(&engine).last().map(String::as_str),
        Some("dragCancel")
    );
}

#[test]
fn a_native_drag_carries_no_files() {
    let (browser, engine) = browser();
    {
        let mut model = super::lock(&engine.model);
        model.native_drag = true;
        model.drag_data = json!({"items":[{"mimeType":"text/plain","data":"card"}],"files":["/etc/hosts"],"dragOperationsMask":1});
    }
    drag_volume_by(&browser, 1).unwrap();
    let sent = engine.sent("Input.dispatchDragEvent");
    assert!(!sent.is_empty());
    for event in sent {
        assert!(event.params["data"].get("files").is_none(), "{event:?}");
    }
}

#[test]
fn a_native_drag_carrying_a_text_kept_out_is_cancelled_before_it_drops() {
    let (browser, engine) = browser();
    let secret = ref_of(&snapshot(&browser, 1), "- textbox \"Secret word\"");
    browser.blank(&secret).unwrap();
    {
        let mut model = super::lock(&engine.model);
        model.native_drag = true;
        model.drag_data = json!({"items":[{"mimeType":"text/plain","data":"OPEN SESAME"}],"dragOperationsMask":1});
    }
    let dragged = drag_volume_by(&browser, 2);
    assert!(
        matches!(&dragged, Err(Refusal::Partly { why, .. }) if *why == Refusal::DragKeptOut.to_string()),
        "{dragged:?}"
    );
    let kinds = drag_kinds(&engine);
    assert_eq!(
        kinds,
        ["dragCancel"],
        "nothing reached the page's drop: {kinds:?}"
    );
    assert!(drags_handed_back(&engine));
}

#[test]
fn a_native_drag_into_a_site_new_to_the_project_waits_on_the_person() {
    let asked: crate::drive::Judge = Arc::new(|out: &crate::drive::Outbound| {
        if out.address.starts_with("http://other.test") {
            crate::drive::Gate::Ask {
                site: "other.test:80".to_owned(),
            }
        } else {
            crate::drive::Gate::Go
        }
    });
    let (browser, engine) = browser_judged(asked);
    super::lock(&engine.model).native_drag = true;
    let text = snapshot(&browser, 1);
    let volume = ref_of(&text, "- slider \"Volume\"");
    let pay = ref_of(&text, "- button \"Pay now\"");
    let dragged = browser
        .call(
            &id(2),
            Call::Drag {
                from: volume,
                to: DragTo::Element(pay),
            },
        )
        .unwrap();
    assert!(
        dragged
            .note
            .as_deref()
            .is_some_and(|n| n.contains("nothing was dropped")),
        "{dragged:?}"
    );
    assert_eq!(dragged.asked, ["other.test:80"]);
    assert_eq!(drag_kinds(&engine), ["dragCancel"]);
    assert!(drags_handed_back(&engine));
}

#[test]
fn a_native_drop_on_an_element_of_the_page_is_judged_for_every_site_the_page_shows() {
    // A frame of another site may be drawn over the element the drop aims at.
    let asked: crate::drive::Judge = Arc::new(|out: &crate::drive::Outbound| {
        if out.address.starts_with("http://other.test") {
            crate::drive::Gate::Ask {
                site: "other.test:80".to_owned(),
            }
        } else {
            crate::drive::Gate::Go
        }
    });
    let (browser, engine) = browser_judged(asked);
    super::lock(&engine.model).native_drag = true;
    let text = snapshot(&browser, 1);
    let volume = ref_of(&text, "- slider \"Volume\"");
    let more = ref_of(&text, "- link \"Read more\"");
    let dragged = browser
        .call(
            &id(2),
            Call::Drag {
                from: volume,
                to: DragTo::Element(more),
            },
        )
        .unwrap();
    assert_eq!(dragged.asked, ["other.test:80"], "{dragged:?}");
    assert_eq!(drag_kinds(&engine), ["dragCancel"]);
}

#[test]
fn a_drag_that_starts_no_engine_drag_presses_moves_and_releases() {
    let (browser, engine) = browser();
    let volume = ref_of(&snapshot(&browser, 1), "- slider \"Volume\"");
    browser
        .call(
            &id(2),
            Call::Drag {
                from: volume,
                to: DragTo::Offset { x: 40.0, y: -5.0 },
            },
        )
        .unwrap();
    assert!(engine.sent("Input.dispatchDragEvent").is_empty());
    assert_eq!(
        engine
            .sent("Input.setInterceptDrags")
            .last()
            .map(|s| s.params["enabled"].clone()),
        Some(json!(false))
    );
}

#[test]
fn a_retried_click_with_the_same_id_clicks_once_and_says_it_is_a_repeat() {
    let (browser, engine) = browser();
    let more = ref_of(&snapshot(&browser, 1), "- link \"Read more\"");
    let first = browser
        .call(
            &id(2),
            Call::Click {
                target: more.clone(),
            },
        )
        .unwrap();
    let again = browser
        .call(
            &id(2),
            Call::Click {
                target: more.clone(),
            },
        )
        .unwrap();
    assert!(!first.repeated);
    assert!(again.repeated);
    let presses = |e: &Engine| mouse(e).iter().filter(|m| m.0 == "mousePressed").count();
    assert_eq!(presses(&engine), 1);
    let other = browser.call(&id(2), Call::Hover { target: more });
    assert_eq!(other, Err(Refusal::IdReused("call-2".into())));
}

#[test]
fn an_action_whose_data_is_still_loading_says_it_has_not_settled_and_what_it_waits_for() {
    let (browser, engine) = browser_with(Limits {
        settle_within: Duration::from_millis(300),
        ..limits()
    });
    let more = ref_of(&snapshot(&browser, 1), "- link \"Read more\"");
    engine.on(|sent, engine| {
        if sent.method == "Input.dispatchMouseEvent" && sent.params["type"] == "mouseReleased" {
            engine.emit(
                "Network.requestWillBeSent",
                json!({"requestId":"7.40","request":{"url":"http://fixture.test/slow-data","method":"GET"},"type":"Fetch","frameId":"PAGE-1"}),
                Some("S-PAGE-1"),
            );
        }
        None
    });
    let answer = browser.call(&id(2), Call::Click { target: more }).unwrap();
    assert_eq!(
        answer.settled,
        Settled::No(vec![
            "1 request still loading: http://fixture.test/slow-data".into()
        ])
    );
}

#[test]
fn an_action_answers_once_its_late_data_has_arrived() {
    let (browser, engine) = browser();
    let more = ref_of(&snapshot(&browser, 1), "- link \"Read more\"");
    engine.on(|sent, engine| {
        if sent.method == "Input.dispatchMouseEvent" && sent.params["type"] == "mouseReleased" {
            engine.emit(
                "Network.requestWillBeSent",
                json!({"requestId":"7.41","request":{"url":"http://fixture.test/slow-data"},"type":"Fetch"}),
                Some("S-PAGE-1"),
            );
            let events = super::lock(&engine.events).clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(300));
                let _ = events.send(Event {
                    method: "Network.loadingFinished".into(),
                    params: json!({"requestId":"7.41"}),
                    session_id: Some("S-PAGE-1".into()),
                });
            });
        }
        None
    });
    let started = std::time::Instant::now();
    let answer = browser.call(&id(2), Call::Click { target: more }).unwrap();
    assert_eq!(answer.settled, Settled::Yes);
    assert!(
        started.elapsed() >= Duration::from_millis(300),
        "it waited for the data"
    );
}

/// The listing of `browser`'s pages.
fn pages_of(browser: &Browser, n: u32) -> Vec<crate::drive::PageInfo> {
    match browser.call(&id(n), Call::Pages).unwrap().content {
        Content::Pages(pages) => pages,
        other => panic!("not pages: {other:?}"),
    }
}

/// A second page, opened by the engine and attached to.
fn a_second_page(browser: &Browser, engine: &Engine) {
    engine.emit(
        "Target.targetCreated",
        json!({"targetInfo":{"targetId":"PAGE-7","type":"page","title":"More","url":"http://fixture.test/more.html","attached":false}}),
        None,
    );
    let start = std::time::Instant::now();
    while super::lock(&browser.inner.state)
        .page("PAGE-7")
        .is_none_or(|p| p.session.is_none())
    {
        assert!(start.elapsed() < Duration::from_secs(5), "never attached");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn a_page_whose_title_does_not_come_does_not_hold_the_listing_past_its_deadline() {
    let (browser, engine) = browser();
    a_second_page(&browser, &engine);
    // Every page's renderer is stuck: each read takes 1.5 s, past the listing's whole second.
    engine.on(|sent, _| {
        if sent.method == "Runtime.evaluate" && sent.params["expression"] == TITLE {
            std::thread::sleep(Duration::from_millis(1500));
            return Some(Err(CallError::TimedOut));
        }
        None
    });
    let started = std::time::Instant::now();
    let pages = pages_of(&browser, 1);
    assert!(
        started.elapsed() < Duration::from_millis(2500),
        "{:?}",
        started.elapsed()
    );
    let titles: Vec<&str> = pages.iter().map(|p| p.title.as_str()).collect();
    assert_eq!(
        titles,
        ["Frames and fields", "More"],
        "as the engine told them"
    );
    let reads = engine
        .sent("Runtime.evaluate")
        .iter()
        .filter(|s| s.params["expression"] == TITLE)
        .count();
    assert_eq!(reads, 1, "no read is started past the deadline");
}

#[test]
fn a_title_read_from_a_document_the_page_has_since_left_is_not_kept() {
    let (browser, engine) = browser();
    // The page loads another document while its title is read: the read is of the one before.
    engine.on(|sent, engine| {
        if sent.method == "Runtime.evaluate" && sent.params["expression"] == TITLE {
            engine.emit(
                "Page.frameNavigated",
                json!({"frame":{"id":"PAGE-1","loaderId":"LOADER-5","url":"http://fixture.test/more.html","name":"","securityOrigin":"http://fixture.test","mimeType":"text/html"},"type":"Navigation"}),
                Some("S-PAGE-1"),
            );
            std::thread::sleep(Duration::from_millis(50));
            return Some(Ok(json!({"result":{"type":"string","value":"The page before"}})));
        }
        None
    });
    assert_eq!(pages_of(&browser, 1)[0].title, "Frames and fields");
}

#[test]
fn a_huge_title_is_cut_and_one_past_the_read_s_bound_keeps_the_title_told() {
    let (browser, engine) = browser();
    super::lock(&engine.model)
        .titles
        .insert("PAGE-1".into(), "t".repeat(5000));
    assert_eq!(
        pages_of(&browser, 1)[0].title.chars().count(),
        super::MOST_TITLE_CHARS
    );
    super::lock(&engine.model)
        .titles
        .insert("PAGE-1".into(), "t".repeat(200_000));
    // Past the read's bound the read fails, and the title kept is the last one read.
    assert_eq!(
        pages_of(&browser, 2)[0].title.chars().count(),
        super::MOST_TITLE_CHARS
    );
}

#[test]
fn a_listed_title_is_scrubbed_before_it_is_cut() {
    let (browser, engine) = browser();
    let secret = ref_of(&snapshot(&browser, 1), "- textbox \"Secret word\"");
    browser.blank(&secret).unwrap();
    // Across the most a listed title holds: cut first, its start would no longer match.
    let title = format!("{}OPEN SESAME", "t".repeat(super::MOST_TITLE_CHARS - 4));
    super::lock(&engine.model)
        .titles
        .insert("PAGE-1".into(), title);
    let listed = pages_of(&browser, 2).remove(0).title;
    assert!(!listed.contains("OPEN"), "{listed}");
    assert!(listed.chars().count() <= super::MOST_TITLE_CHARS);
}

#[test]
fn a_huge_title_the_tree_names_is_cut() {
    let (browser, engine) = browser();
    {
        let mut model = super::lock(&engine.model);
        for node in model.ax.get_mut("PAGE-1").unwrap().as_array_mut().unwrap() {
            if node["role"]["value"] == "RootWebArea" {
                node["name"]["value"] = json!("t".repeat(5000));
            }
        }
    }
    snapshot(&browser, 1);
    let kept = super::lock(&browser.inner.state)
        .page("PAGE-1")
        .unwrap()
        .title
        .chars()
        .count();
    // The tree cuts a long name itself; the page keeps no more than a title's bound either way.
    assert!(kept <= super::MOST_TITLE_CHARS, "{kept}");
}

#[test]
fn a_title_a_script_set_is_listed_though_the_engine_never_told_of_it() {
    let (browser, engine) = browser();
    // The engine told of "Frames and fields" as the page opened, and of nothing since.
    super::lock(&engine.model)
        .titles
        .insert("PAGE-1".into(), "Retitled by its script".into());
    let listed = browser.call(&id(1), Call::Pages).unwrap();
    let Content::Pages(pages) = &listed.content else {
        panic!("{listed:?}")
    };
    assert_eq!(pages[0].title, "Retitled by its script");
    assert_eq!(listed.page.unwrap().title, "Retitled by its script");
}

#[test]
fn a_page_slow_to_read_does_not_cost_the_current_page_its_title() {
    let (browser, engine) = browser();
    a_second_page(&browser, &engine);
    browser
        .call(
            &id(1),
            Call::SwitchPage {
                page: PageId::new("p2"),
            },
        )
        .unwrap();
    super::lock(&engine.model)
        .titles
        .insert("PAGE-7".into(), "Retitled by its script".into());
    // The page opened first, read first by age, takes past the listing's whole second.
    engine.on(|sent, _| {
        if sent.method == "Runtime.evaluate"
            && sent.params["expression"] == TITLE
            && sent.session.as_deref() == Some("S-PAGE-1")
        {
            std::thread::sleep(Duration::from_millis(1500));
            return Some(Err(CallError::TimedOut));
        }
        None
    });

    let listed = browser.call(&id(2), Call::Pages).unwrap();

    assert_eq!(listed.page.unwrap().title, "Retitled by its script");
}

#[test]
fn a_newer_page_slow_to_read_does_not_cost_an_older_current_page_its_title() {
    let (browser, engine) = browser();
    a_second_page(&browser, &engine);
    super::lock(&engine.model)
        .titles
        .insert("PAGE-1".into(), "Retitled by its script".into());
    engine.on(|sent, _| {
        if sent.method == "Runtime.evaluate"
            && sent.params["expression"] == TITLE
            && sent.session.as_deref() == Some("S-PAGE-7")
        {
            std::thread::sleep(Duration::from_millis(1500));
            return Some(Err(CallError::TimedOut));
        }
        None
    });

    let listed = browser.call(&id(1), Call::Pages).unwrap();

    assert_eq!(listed.page.unwrap().title, "Retitled by its script");
}

#[test]
fn a_page_whose_title_cannot_be_read_keeps_the_title_the_engine_told_of() {
    let (browser, engine) = browser();
    engine.on(|sent, _| {
        (sent.method == "Runtime.evaluate" && sent.params["expression"] == TITLE)
            .then_some(Err(CallError::TimedOut))
    });
    let listed = browser.call(&id(1), Call::Pages).unwrap();
    let Content::Pages(pages) = &listed.content else {
        panic!("{listed:?}")
    };
    assert_eq!(pages[0].title, "Frames and fields");
}

#[test]
fn a_page_settles_once_a_frame_with_a_request_open_detaches() {
    let (browser, engine) = browser();
    // A frame inside the page asks for data, and goes before the engine says it ended.
    engine.emit(
        "Page.frameAttached",
        json!({"frameId":"ADS-FRAME","parentFrameId":"PAGE-1"}),
        Some("S-PAGE-1"),
    );
    engine.emit(
        "Network.requestWillBeSent",
        json!({"requestId":"9.1","loaderId":"L9","frameId":"ADS-FRAME","request":{"url":"http://fixture.test/ads-data"},"type":"Fetch"}),
        Some("S-PAGE-1"),
    );
    // What the page waits for, as its settling reads it now.
    let waits = || {
        super::lock(&browser.inner.state)
            .page("PAGE-1")
            .unwrap()
            .settle
            .waiting_for(std::time::Instant::now(), limits().quiet)
    };
    let until = |done: &dyn Fn(&[String]) -> bool, what: &str| {
        let start = std::time::Instant::now();
        while !done(&waits()) {
            assert!(
                start.elapsed() < Duration::from_secs(5),
                "{what}: {:?}",
                waits()
            );
            std::thread::sleep(Duration::from_millis(2));
        }
    };
    until(
        &|w| w.iter().any(|w| w.contains("ads-data")) && w.len() == 1,
        "the request never counted alone",
    );
    let before = browser.call(&id(1), Call::Pages).unwrap();
    assert!(
        matches!(&before.settled, Settled::No(why) if why.iter().any(|w| w.contains("ads-data"))),
        "{:?}",
        before.settled
    );
    engine.emit(
        "Page.frameDetached",
        json!({"frameId":"ADS-FRAME","reason":"remove"}),
        Some("S-PAGE-1"),
    );
    // The page settles a quiet while after the detach.
    until(&|w| w.is_empty(), "the page never settled");
    let after = browser.call(&id(2), Call::Pages).unwrap();
    assert_eq!(after.settled, Settled::Yes);
}

#[test]
fn a_page_whose_document_has_not_loaded_has_not_settled() {
    let (browser, engine) = browser_with(Limits {
        settle_within: Duration::from_millis(200),
        ..limits()
    });
    super::lock(&engine.model).ready = "interactive".into();
    // Quiet for longer than the quiet while, so only the document is left to wait for.
    std::thread::sleep(Duration::from_millis(60));
    let answer = browser.call(&id(1), Call::Pages).unwrap();
    assert_eq!(
        answer.settled,
        Settled::No(vec!["the page's document is still loading".into()])
    );
}

#[test]
fn an_action_that_opens_a_page_answers_with_it_and_makes_it_current() {
    let (browser, engine) = browser();
    let more = ref_of(&snapshot(&browser, 1), "- link \"Read more\"");
    engine.on(|sent, engine| {
        if sent.method == "Input.dispatchMouseEvent" && sent.params["type"] == "mouseReleased" {
            engine.emit(
                "Target.targetCreated",
                json!({"targetInfo":{"targetId":"PAGE-7","type":"page","title":"More","url":"http://fixture.test/more.html","openerId":"PAGE-1","attached":false}}),
                None,
            );
        }
        None
    });
    let answer = browser.call(&id(2), Call::Click { target: more }).unwrap();
    assert_eq!(answer.settled, Settled::Yes);
    assert_eq!(answer.opened.len(), 1, "{answer:?}");
    assert_eq!(answer.opened[0].id, PageId::new("p2"));
    assert_eq!(answer.opened[0].address, "http://fixture.test/more.html");
    assert!(answer.opened[0].current);
    assert_eq!(answer.page.as_ref().unwrap().id, PageId::new("p2"));
    assert!(
        engine
            .sent("Target.attachToTarget")
            .iter()
            .any(|s| s.params["targetId"] == "PAGE-7")
    );
}

#[test]
fn a_page_opened_before_it_has_an_address_is_answered_once_it_has_one() {
    let (browser, engine) = browser();
    let more = ref_of(&snapshot(&browser, 1), "- link \"Read more\"");
    engine.on(|sent, engine| {
        if sent.method == "Input.dispatchMouseEvent" && sent.params["type"] == "mouseReleased" {
            engine.emit(
                "Target.targetCreated",
                json!({"targetInfo":{"targetId":"PAGE-6","type":"page","title":"","url":"","attached":false}}),
                None,
            );
            engine.emit(
                "Target.targetInfoChanged",
                json!({"targetInfo":{"targetId":"PAGE-6","type":"page","title":"More","url":"http://fixture.test/more.html","attached":true}}),
                None,
            );
        }
        None
    });
    let answer = browser.call(&id(2), Call::Click { target: more }).unwrap();
    assert_eq!(answer.opened.len(), 1, "{answer:?}");
    assert_eq!(answer.opened[0].address, "http://fixture.test/more.html");
    assert!(engine.sent("Target.closeTarget").is_empty());
}

#[test]
fn a_page_opened_with_no_address_has_not_settled_until_it_has_one() {
    // As a link with `target="_blank"` opens one in a real engine: the page is made empty, is
    // attached and reads as loaded, and is given its address only once it has gone on.
    let (browser, engine) = browser();
    let more = ref_of(&snapshot(&browser, 1), "- link \"Read more\"");
    engine.on(|sent, engine| {
        if sent.method == "Input.dispatchMouseEvent" && sent.params["type"] == "mouseReleased" {
            engine.emit(
                "Target.targetCreated",
                json!({"targetInfo":{"targetId":"PAGE-10","type":"page","title":"","url":"","openerId":"PAGE-1","attached":false}}),
                None,
            );
        }
        if sent.method == "Runtime.runIfWaitingForDebugger"
            && sent.session.as_deref() == Some("S-PAGE-10")
        {
            // The engine goes on in its own time: the address comes well after the quiet while.
            let events = super::lock(&engine.events).clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(300));
                let _ = events.send(Event {
                    method: "Target.targetInfoChanged".to_owned(),
                    params: json!({"targetInfo":{"targetId":"PAGE-10","type":"page","title":"More","url":"http://fixture.test/more.html","attached":true}}),
                    session_id: None,
                });
            });
        }
        None
    });
    let answer = browser.call(&id(2), Call::Click { target: more }).unwrap();
    assert_eq!(answer.settled, Settled::Yes, "{answer:?}");
    assert_eq!(answer.opened.len(), 1, "{answer:?}");
    assert_eq!(answer.opened[0].address, "http://fixture.test/more.html");
}

#[test]
fn a_page_the_engine_opens_at_a_file_or_app_address_is_closed_and_never_listed() {
    let (browser, engine) = browser();
    let more = ref_of(&snapshot(&browser, 1), "- link \"Read more\"");
    engine.on(|sent, engine| {
        if sent.method == "Input.dispatchMouseEvent" && sent.params["type"] == "mouseReleased" {
            engine.emit(
                "Target.targetCreated",
                json!({"targetInfo":{"targetId":"PAGE-8","type":"page","title":"","url":"file:///etc/hosts","openerId":"PAGE-1","attached":false}}),
                None,
            );
            engine.emit(
                "Target.targetCreated",
                json!({"targetInfo":{"targetId":"PAGE-9","type":"page","title":"","url":"about:blank","openerId":"PAGE-1","attached":false}}),
                None,
            );
            engine.emit(
                "Target.targetInfoChanged",
                json!({"targetInfo":{"targetId":"PAGE-9","type":"page","title":"","url":"purlis-app://open","openerId":"PAGE-1","attached":false}}),
                None,
            );
        }
        None
    });
    let answer = browser.call(&id(2), Call::Click { target: more }).unwrap();
    assert!(answer.opened.is_empty(), "{answer:?}");
    let closed: Vec<Value> = engine
        .sent("Target.closeTarget")
        .iter()
        .map(|s| s.params["targetId"].clone())
        .collect();
    assert!(closed.contains(&json!("PAGE-8")), "{closed:?}");
    assert!(closed.contains(&json!("PAGE-9")), "{closed:?}");
    let listed = browser.call(&id(3), Call::Pages).unwrap();
    let Content::Pages(pages) = listed.content else {
        panic!()
    };
    assert_eq!(pages.len(), 1, "{pages:?}");
    assert_eq!(pages[0].address, "http://fixture.test/frames.html");
    assert!(
        !engine
            .sent("Target.attachToTarget")
            .iter()
            .any(|s| s.params["targetId"] == "PAGE-8"),
        "a page at a file address is never driven"
    );
}

#[test]
fn a_reference_from_before_its_frame_loaded_another_document_is_refused_as_stale() {
    let (browser, engine) = browser();
    let text = snapshot(&browser, 1);
    let more = ref_of(&text, "- link \"Read more\"");
    let pay = ref_of(&text, "- button \"Pay now\"");
    engine.emit(
        "Page.frameNavigated",
        json!({"frame":{"id":"PAGE-1","loaderId":"LOADER-5","url":"http://fixture.test/more.html","name":"","securityOrigin":"http://fixture.test","mimeType":"text/html"},"type":"Navigation"}),
        Some("S-PAGE-1"),
    );
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(
        browser.call(
            &id(2),
            Call::Click {
                target: more.clone()
            }
        ),
        Err(Refusal::StaleRef(more))
    );
    // The frame's own document did not change: its references live on.
    assert!(browser.call(&id(3), Call::Click { target: pay }).is_ok());
    assert_eq!(
        browser.call(
            &id(4),
            Call::Click {
                target: Ref::new("e999")
            }
        ),
        Err(Refusal::NoSuchRef(Ref::new("e999")))
    );
}

#[test]
fn typing_into_several_fields_focuses_each_and_types_its_text() {
    let (browser, engine) = browser();
    let text = snapshot(&browser, 1);
    let name = ref_of(&text, "- textbox \"Name\"");
    let more = ref_of(&text, "- link \"Read more\"");
    browser
        .call(
            &id(2),
            Call::Type {
                page: None,
                fields: vec![
                    Field {
                        into: Some(name),
                        text: "Grace".into(),
                        replace: true,
                    },
                    Field {
                        into: Some(more),
                        text: "!".into(),
                        replace: false,
                    },
                    Field {
                        into: None,
                        text: "?".into(),
                        replace: false,
                    },
                ],
            },
        )
        .unwrap();
    let steps: Vec<String> = engine
        .all_sent()
        .iter()
        .filter_map(|s| match s.method.as_str() {
            "DOM.focus" => Some(format!("focus {}", s.params["backendNodeId"])),
            "Input.insertText" => Some(format!("type {}", s.params["text"])),
            "Input.dispatchKeyEvent" if s.params["type"] != "keyUp" => {
                Some(format!("key {}", s.params["key"]))
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        steps,
        vec![
            "focus 20",
            "key \"Control\"",
            "key \"a\"",
            "type \"Grace\"",
            "focus 12",
            "type \"!\"",
            "type \"?\"",
        ]
    );
}

#[test]
fn pressing_several_keys_sends_each_in_turn_and_an_unknown_key_sends_none() {
    let (browser, engine) = browser();
    browser
        .call(
            &id(1),
            Call::Press {
                page: None,
                keys: vec!["Tab".into(), "Enter".into()],
            },
        )
        .unwrap();
    let downs: Vec<Value> = engine
        .sent("Input.dispatchKeyEvent")
        .iter()
        .filter(|s| s.params["type"] != "keyUp")
        .map(|s| s.params["key"].clone())
        .collect();
    assert_eq!(downs, vec![json!("Tab"), json!("Enter")]);
    let refused = browser.call(
        &id(2),
        Call::Press {
            page: None,
            keys: vec!["Enter".into(), "Hyper+x".into()],
        },
    );
    assert!(matches!(refused, Err(Refusal::NotAKey(_))), "{refused:?}");
    assert_eq!(
        engine.sent("Input.dispatchKeyEvent").len(),
        4,
        "nothing more was pressed"
    );
}

#[test]
fn choosing_options_passes_them_as_a_value_to_purlis_own_function() {
    let (browser, engine) = browser();
    let volume = ref_of(&snapshot(&browser, 1), "- slider \"Volume\"");
    browser
        .call(
            &id(2),
            Call::Select {
                target: volume.clone(),
                options: vec!["Blue".into()],
            },
        )
        .unwrap();
    let chose = &engine.sent("Runtime.callFunctionOn")[0];
    assert_eq!(chose.params["functionDeclaration"], SELECT_OPTIONS);
    // The drop-down, as purlis's world holds it, then the options as a value.
    assert_eq!(chose.params["arguments"][1], json!({ "value": ["Blue"] }));
    assert!(
        chose.params["arguments"][0]["objectId"]
            .as_str()
            .is_some_and(|o| o.starts_with("OBJECT-")),
        "{chose:?}"
    );
    super::lock(&engine.model).select = json!({"missing":["Teal"],"options":["Red","Blue"]});
    assert_eq!(
        browser.call(
            &id(3),
            Call::Select {
                target: volume.clone(),
                options: vec!["Teal".into()],
            },
        ),
        Err(Refusal::NoSuchOption {
            target: volume,
            missing: "Teal".into(),
            options: "Red, Blue".into(),
        })
    );
}

#[test]
fn purlis_scripts_read_and_act_in_a_world_of_their_own_in_the_frame_they_act_on() {
    // The stand-in's page lies in its own world about everything purlis's scripts read
    // (#2138): its document is still loading and no drop-down is one.
    let (browser, engine) = browser();
    let text = snapshot(&browser, 1);
    let volume = ref_of(&text, "- slider \"Volume\"");
    let chose = browser.call(
        &id(2),
        Call::Select {
            target: volume,
            options: vec!["Blue".into()],
        },
    );
    assert!(
        matches!(&chose, Ok(a) if a.settled == Settled::Yes),
        "{chose:?}"
    );
    let worlds = engine.sent("Page.createIsolatedWorld");
    assert!(!worlds.is_empty());
    for world in &worlds {
        assert_eq!(
            world.params["grantUniveralAccess"],
            json!(false),
            "{world:?}"
        );
        let frame = match world.session.as_deref() {
            Some("S-PAGE-1") => "PAGE-1",
            Some("S-PAYMENT") => "PAYMENT-FRAME",
            other => panic!("a world on no frame's session: {other:?}"),
        };
        assert_eq!(world.params["frameId"], frame, "{world:?}");
    }
    every_script_ran_in_a_world_of_purlis(&engine);
}

/// Every script purlis ran named a world the engine told of by its unique id, and ran on an
/// element of that world.
fn every_script_ran_in_a_world_of_purlis(engine: &Engine) {
    for sent in engine.all_sent() {
        if matches!(
            sent.method.as_str(),
            "Runtime.evaluate" | "Runtime.callFunctionOn"
        ) {
            assert!(
                sent.params["uniqueContextId"]
                    .as_str()
                    .is_some_and(|u| u.starts_with("U-")),
                "{sent:?}"
            );
        }
    }
}

#[test]
fn a_world_whose_number_now_names_the_pages_own_world_is_never_run_in() {
    // The page moved to another process before the engine said so: the number of purlis's
    // world there names the page's own world.
    let (browser, engine) = browser();
    let volume = ref_of(&snapshot(&browser, 1), "- slider \"Volume\"");
    let made = engine.sent("Page.createIsolatedWorld").len();
    engine.worlds_reused_by_the_page();
    let chose = browser.call(
        &id(2),
        Call::Select {
            target: volume,
            options: vec!["Blue".into()],
        },
    );
    assert!(
        matches!(&chose, Ok(a) if a.settled == Settled::Yes),
        "{chose:?}"
    );
    assert!(engine.sent("Page.createIsolatedWorld").len() > made);
    every_script_ran_in_a_world_of_purlis(&engine);
}

#[test]
fn a_frame_that_loads_another_document_gets_a_new_world() {
    let (browser, engine) = browser();
    snapshot(&browser, 1);
    let made = engine.sent("Page.createIsolatedWorld").len();
    let first = engine.sent("Runtime.evaluate").last().unwrap().params["uniqueContextId"].clone();
    engine.emit(
        "Page.frameNavigated",
        json!({"frame":{"id":"PAGE-1","loaderId":"LOADER-9","url":"http://fixture.test/frames.html","securityOrigin":"http://fixture.test","mimeType":"text/html"}}),
        Some("S-PAGE-1"),
    );
    std::thread::sleep(Duration::from_millis(20));
    snapshot(&browser, 2);
    assert!(engine.sent("Page.createIsolatedWorld").len() > made);
    let after: Vec<Value> = engine
        .sent("Runtime.evaluate")
        .iter()
        .map(|s| s.params["uniqueContextId"].clone())
        .collect();
    assert_ne!(after.last(), Some(&first), "the old world is not run in");
}

#[test]
fn a_world_whose_document_has_gone_is_made_again() {
    let (browser, engine) = browser();
    let volume = ref_of(&snapshot(&browser, 1), "- slider \"Volume\"");
    let made = engine.sent("Page.createIsolatedWorld").len();
    for world in super::lock(&engine.worlds).iter_mut() {
        world.live = false;
    }
    let chose = browser.call(
        &id(2),
        Call::Select {
            target: volume,
            options: vec!["Blue".into()],
        },
    );
    assert!(
        matches!(&chose, Ok(a) if a.settled == Settled::Yes),
        "{chose:?}"
    );
    assert!(engine.sent("Page.createIsolatedWorld").len() > made);
}

#[test]
fn no_call_runs_script_the_caller_wrote() {
    let (browser, engine) = browser();
    let text = snapshot(&browser, 1);
    let name = ref_of(&text, "- textbox \"Name\"");
    let words = "document.cookie";
    browser
        .call(
            &id(2),
            Call::Type {
                page: None,
                fields: vec![Field {
                    into: Some(name.clone()),
                    text: words.into(),
                    replace: false,
                }],
            },
        )
        .unwrap();
    let _ = browser.call(
        &id(3),
        Call::Select {
            target: name,
            options: vec![words.into()],
        },
    );
    browser
        .call(
            &id(4),
            Call::WaitForText {
                page: None,
                text: words.into(),
                gone: true,
                within: Duration::from_millis(10),
            },
        )
        .unwrap();
    for sent in engine.all_sent() {
        match sent.method.as_str() {
            "Runtime.evaluate" => assert!(
                [READY_STATE, ACTIVE_ELEMENT]
                    .contains(&sent.params["expression"].as_str().unwrap()),
                "{sent:?}"
            ),
            "Runtime.callFunctionOn" => {
                assert!(
                    [SELECT_OPTIONS, READ_VALUE]
                        .contains(&sent.params["functionDeclaration"].as_str().unwrap()),
                    "{sent:?}"
                );
            }
            "Runtime.compileScript"
            | "Runtime.runScript"
            | "Page.addScriptToEvaluateOnNewDocument" => {
                panic!("{sent:?}")
            }
            _ => {}
        }
    }
}

#[test]
fn a_blanked_field_shows_nothing_in_a_snapshot_find_or_wait() {
    let (browser, _engine) = browser();
    let secret = ref_of(&snapshot(&browser, 1), "- textbox \"Secret word\"");
    browser.blank(&secret).unwrap();
    let text = snapshot(&browser, 2);
    assert!(
        text.contains(&format!("- textbox [{secret}] (blanked)")),
        "{text}"
    );
    assert!(!text.contains("sesame"), "{text}");
    let found = browser
        .call(
            &id(3),
            Call::Find {
                page: None,
                text: Some("sesame".into()),
                role: None,
                within: None,
            },
        )
        .unwrap();
    assert!(
        matches!(&found.content, Content::Text(t) if t.ends_with("Nothing on the page matches.\n")),
        "{found:?}"
    );
    let waited = browser
        .call(
            &id(4),
            Call::WaitForText {
                page: None,
                text: "sesame".into(),
                gone: false,
                within: Duration::from_millis(10),
            },
        )
        .unwrap();
    assert_eq!(waited.content, Content::Seen(false));
}

fn pixel(png_bytes: &[u8], x: usize, y: usize) -> Vec<u8> {
    let mut reader = png::Decoder::new(std::io::Cursor::new(png_bytes))
        .read_info()
        .unwrap();
    let mut buf = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut buf).unwrap();
    let channels = info.line_size / info.width as usize;
    let at = y * info.line_size + x * channels;
    buf[at..at + channels].to_vec()
}

#[test]
fn a_screenshot_paints_over_a_blanked_field_and_nothing_else() {
    let (browser, _engine) = browser();
    let name = ref_of(&snapshot(&browser, 1), "- textbox \"Name\"");
    let plain = browser
        .call(&id(2), Call::Screenshot { page: None })
        .unwrap();
    let Content::Image(plain) = plain.content else {
        panic!()
    };
    assert_eq!(pixel(&plain, 1, 1), vec![255, 255, 255]);
    browser.blank(&name).unwrap();
    let shot = browser
        .call(&id(3), Call::Screenshot { page: None })
        .unwrap();
    let Content::Image(shot) = shot.content else {
        panic!()
    };
    // The Name field's box is (0, 0) to (4, 2), with a margin of 2 pixels.
    assert_eq!(pixel(&shot, 1, 1), vec![128, 128, 128]);
    assert_eq!(pixel(&shot, 5, 3), vec![128, 128, 128]);
    assert_eq!(pixel(&shot, 7, 7), vec![255, 255, 255]);
}

#[test]
fn a_screenshot_is_withheld_when_a_blanked_field_cannot_be_placed() {
    let (browser, engine) = browser();
    let name = ref_of(&snapshot(&browser, 1), "- textbox \"Name\"");
    browser.blank(&name).unwrap();
    engine.on(|sent, _| (sent.method == "DOM.getContentQuads").then_some(Err(CallError::TimedOut)));
    let shot = browser.call(&id(2), Call::Screenshot { page: None });
    assert!(matches!(shot, Err(Refusal::Withheld(_))), "{shot:?}");
}

#[test]
fn a_blanked_field_that_is_not_laid_out_does_not_withhold_the_screenshot() {
    let (browser, engine) = browser();
    let name = ref_of(&snapshot(&browser, 1), "- textbox \"Name\"");
    browser.blank(&name).unwrap();
    super::lock(&engine.model).quads.remove(&20);
    let shot = browser.call(&id(2), Call::Screenshot { page: None });
    assert!(
        matches!(
            shot,
            Ok(Answer {
                content: Content::Image(_),
                ..
            })
        ),
        "{shot:?}"
    );
}

#[test]
fn going_back_to_an_address_that_is_not_a_web_address_is_refused() {
    let (browser, engine) = browser();
    super::lock(&engine.model).history = json!({"currentIndex":1,"entries":[
        {"id":1,"url":"about:blank","title":""},
        {"id":2,"url":"http://fixture.test/frames.html","title":""}]});
    assert_eq!(
        browser.call(&id(1), Call::Back { page: None }),
        Err(Refusal::EarlierNotWeb)
    );
    assert!(engine.sent("Page.navigateToHistoryEntry").is_empty());
    super::lock(&engine.model).history = json!({"currentIndex":1,"entries":[
        {"id":1,"url":"http://fixture.test/index.html","title":""},
        {"id":2,"url":"http://fixture.test/frames.html","title":""}]});
    browser.call(&id(2), Call::Back { page: None }).unwrap();
    assert_eq!(
        engine.sent("Page.navigateToHistoryEntry")[0].params["entryId"],
        1
    );
}

#[test]
fn opening_a_page_takes_a_web_address_and_makes_the_page_current() {
    let (browser, engine) = browser();
    let address = WebAddress::parse("http://fixture.test/long.html").unwrap();
    let answer = browser.call(&id(1), Call::Open { address }).unwrap();
    assert_eq!(answer.page.as_ref().unwrap().id, PageId::new("p2"));
    assert!(answer.page.unwrap().current);
    assert_eq!(
        engine.sent("Target.createTarget")[0].params["url"],
        "about:blank"
    );
    let went = &engine.sent("Page.navigate")[0];
    assert_eq!(went.params["url"], "http://fixture.test/long.html");
    assert_eq!(went.session.as_deref(), Some("S-PAGE-2"));
}

#[test]
fn a_page_the_engine_made_and_has_not_told_of_is_not_called_missing() {
    // Each answers its own way on a loaded machine (#4472): the page exists, and only its event
    // is late.
    let (browser, engine) = browser_with(Limits {
        settle_within: Duration::from_millis(200),
        ..limits()
    });
    engine.on(|sent, _| {
        (sent.method == "Target.createTarget").then(|| Ok(json!({ "targetId": "PAGE-LATE" })))
    });
    let address = WebAddress::parse("http://fixture.test/long.html").unwrap();
    assert_eq!(
        browser.call(&id(1), Call::Open { address }).unwrap_err(),
        Refusal::PageUntold
    );
}

#[test]
fn no_frame_or_worker_is_held_at_its_start() {
    // Held frames are what make Chrome end itself on a page whose frame reloads quickly (#4472).
    let (browser, engine) = browser();
    let start = std::time::Instant::now();
    while engine.sent("Target.setAutoAttach").is_empty() {
        assert!(start.elapsed() < Duration::from_secs(5), "never set up");
        std::thread::sleep(Duration::from_millis(5));
    }
    for sent in engine.sent("Target.setAutoAttach") {
        assert_eq!(sent.params["waitForDebuggerOnStart"], false, "{sent:?}");
    }
    // So there is nothing to let go of, and Chrome can leave that call unanswered.
    let _ = snapshot(&browser, 1);
    assert_eq!(engine.sent("Runtime.runIfWaitingForDebugger").len(), 0);
}

#[test]
fn a_slow_call_to_the_engine_never_stops_its_events_being_read() {
    // Chrome can leave a call unanswered, and an event read behind it comes too late (#4472).
    let (browser, engine) = browser();
    super::lock(&engine.model)
        .slow
        .insert("Page.enable".to_owned(), Duration::from_secs(3));
    engine.emit(
        "Target.attachedToTarget",
        json!({"sessionId":"S-LATER","targetInfo":{"targetId":"LATER-FRAME","type":"iframe","title":"","url":"http://other.test/","attached":true},"waitingForDebugger":false}),
        Some("S-PAGE-1"),
    );
    let start = std::time::Instant::now();
    while engine.sent("Page.enable").iter().all(|s| s.session.as_deref() != Some("S-LATER")) {
        assert!(start.elapsed() < Duration::from_secs(1), "never set up");
        std::thread::sleep(Duration::from_millis(5));
    }
    engine.emit(
        "Target.targetCreated",
        json!({"targetInfo":{"targetId":"PAGE-7","type":"page","title":"","url":"about:blank","attached":false}}),
        None,
    );
    let start = std::time::Instant::now();
    while super::lock(&browser.inner.state).page("PAGE-7").is_none() {
        assert!(start.elapsed() < Duration::from_secs(1), "the event waited on the call");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn a_wait_for_text_sees_text_that_shows_later() {
    let (browser, engine) = browser();
    let later = engine.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(300));
        let mut model = super::lock(&later.model);
        let nodes = model.ax.get_mut("PAGE-1").unwrap().as_array_mut().unwrap();
        nodes.push(json!({"nodeId":"90","ignored":false,"role":{"type":"internalRole","value":"StaticText"},"name":{"type":"computedString","value":"Data arrived"},"childIds":[],"backendDOMNodeId":90}));
        let root = nodes.iter_mut().find(|n| n["nodeId"] == "3").unwrap();
        root["childIds"].as_array_mut().unwrap().push(json!("90"));
    });
    let answer = browser
        .call(
            &id(1),
            Call::WaitForText {
                page: None,
                text: "data ARRIVED".into(),
                gone: false,
                within: Duration::from_secs(3),
            },
        )
        .unwrap();
    assert_eq!(answer.content, Content::Seen(true));
}

#[test]
fn a_screenshot_on_a_screen_of_two_pixels_to_the_point_paints_the_whole_field() {
    let (browser, engine) = browser();
    super::lock(&engine.model).pixel_ratio = 2.0;
    let name = ref_of(&snapshot(&browser, 1), "- textbox \"Name\"");
    browser.blank(&name).unwrap();
    let shot = browser
        .call(&id(2), Call::Screenshot { page: None })
        .unwrap();
    let Content::Image(shot) = shot.content else {
        panic!()
    };
    // The Name field's box (0, 0) to (4, 2) is (0, 0) to (8, 4) on the picture, and a margin.
    assert_eq!(pixel(&shot, 9, 5), vec![128, 128, 128]);
    assert_eq!(pixel(&shot, 9, 7), vec![255, 255, 255]);
}

#[test]
fn a_screenshot_whose_scale_is_not_known_is_withheld() {
    let (browser, engine) = browser();
    let name = ref_of(&snapshot(&browser, 1), "- textbox \"Name\"");
    browser.blank(&name).unwrap();
    engine.on(|sent, _| {
        (sent.method == "Page.getLayoutMetrics")
            .then(|| Ok(json!({"cssVisualViewport":{"clientWidth":10}})))
    });
    let shot = browser.call(&id(2), Call::Screenshot { page: None });
    assert!(matches!(shot, Err(Refusal::Withheld(_))), "{shot:?}");
}

#[test]
fn what_a_blanked_field_held_stays_out_of_every_answer_after_the_field_is_gone() {
    let (browser, engine) = browser();
    let secret = ref_of(&snapshot(&browser, 1), "- textbox \"Secret word\"");
    browser.blank(&secret).unwrap();
    // The page takes the field away and puts what it held in its title and an address.
    {
        let mut model = super::lock(&engine.model);
        let nodes = model.ax.get_mut("PAGE-1").unwrap().as_array_mut().unwrap();
        nodes.retain(|n| n["nodeId"] != "12" && n["nodeId"] != "13");
        nodes[0]["name"]["value"] = json!("Welcome, OPEN SESAME");
    }
    engine.emit(
        "Target.targetInfoChanged",
        json!({"targetInfo":{"targetId":"PAGE-1","type":"page","title":"Open Sesame","url":"http://fixture.test/frames.html?w=open%20sesame&v=Open+Sesame"}}),
        None,
    );
    std::thread::sleep(Duration::from_millis(50));
    let text = snapshot(&browser, 2);
    assert!(!text.to_lowercase().contains("sesame"), "{text}");
    let found = browser
        .call(
            &id(3),
            Call::Find {
                page: None,
                text: Some("SESAME".into()),
                role: None,
                within: None,
            },
        )
        .unwrap();
    assert!(
        matches!(&found.content, Content::Text(t) if t.ends_with("Nothing on the page matches.\n")),
        "{found:?}"
    );
    let listed = browser.call(&id(4), Call::Pages).unwrap();
    let page = listed.page.unwrap();
    assert!(!page.title.to_lowercase().contains("sesame"), "{page:?}");
    assert!(!page.address.to_lowercase().contains("sesame"), "{page:?}");
}

#[test]
fn the_element_that_has_the_focus_can_be_blanked_without_a_reference() {
    let (browser, _engine) = browser();
    browser.blank_focused(None).unwrap();
    let text = snapshot(&browser, 1);
    assert!(text.contains("- textbox [e3] (blanked)"), "{text}");
    assert!(!text.contains("Ada"), "{text}");
}

#[test]
fn typing_that_stops_part_way_says_how_far_it_got_and_that_the_state_is_not_known() {
    let (browser, engine) = browser();
    let text = snapshot(&browser, 1);
    let name = ref_of(&text, "- textbox \"Name\"");
    let more = ref_of(&text, "- link \"Read more\"");
    engine.on(|sent, _| {
        (sent.method == "Input.insertText" && sent.params["text"] == "second")
            .then_some(Err(CallError::TimedOut))
    });
    let typed = browser.call(
        &id(2),
        Call::Type {
            page: None,
            fields: vec![
                Field {
                    into: Some(name),
                    text: "first".into(),
                    replace: false,
                },
                Field {
                    into: Some(more),
                    text: "second".into(),
                    replace: false,
                },
            ],
        },
    );
    let Err(Refusal::Partly { done, .. }) = typed else {
        panic!("{typed:?}")
    };
    assert_eq!(done, "typed into 1 of 2 fields, and maybe part of the next");
}

#[test]
fn a_click_cut_off_after_the_press_says_it_acted_in_part() {
    let (browser, engine) = browser();
    let more = ref_of(&snapshot(&browser, 1), "- link \"Read more\"");
    engine.on(|sent, _| {
        (sent.method == "Input.dispatchMouseEvent" && sent.params["type"] == "mouseReleased")
            .then_some(Err(CallError::TimedOut))
    });
    let clicked = browser.call(&id(2), Call::Click { target: more });
    assert!(
        matches!(clicked, Err(Refusal::Partly { .. })),
        "{clicked:?}"
    );
}

#[test]
fn a_call_that_only_reads_reads_again_when_retried() {
    let (browser, engine) = browser();
    snapshot(&browser, 1);
    snapshot(&browser, 1);
    let reads = engine
        .sent("Accessibility.getFullAXTree")
        .iter()
        .filter(|s| s.params["frameId"] == "PAGE-1")
        .count();
    assert_eq!(reads, 2);
}

#[test]
fn a_snapshot_gives_no_more_lines_than_the_most_allowed() {
    let (browser, _engine) = browser_with(Limits {
        most_snapshot_lines: 3,
        ..limits()
    });
    let answer = browser
        .call(
            &id(1),
            Call::Snapshot {
                page: None,
                within: None,
                from: 1,
                lines: Some(1_000_000),
            },
        )
        .unwrap();
    let Content::Text(text) = answer.content else {
        panic!()
    };
    assert!(
        text.contains("(lines 1 to 3 of 11; the next page starts at line 4)"),
        "{text}"
    );
}

/// The engine's id for the element the tree of `frame` marks focused.
fn ax_backend_focused(engine: &Engine, frame: &str) -> i64 {
    super::lock(&engine.model).ax[frame]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| {
            n["properties"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|p| p["name"] == "focused" && p["value"]["value"] == true)
        })
        .and_then(|n| n["backendDOMNodeId"].as_i64())
        .expect("an element the tree marks focused")
}

/// Moves the focus in the stand-in's trees: off the Name field, and onto each of `onto`
/// (frame, node id).
fn focus_on(engine: &Engine, onto: &[(&str, &str)]) {
    let mut model = super::lock(&engine.model);
    for nodes in model.ax.values_mut() {
        for node in nodes.as_array_mut().unwrap() {
            if let Some(props) = node.get_mut("properties").and_then(Value::as_array_mut) {
                props.retain(|p| p["name"] != "focused");
            }
        }
    }
    for (frame, id) in onto {
        let nodes = model.ax.get_mut(*frame).unwrap().as_array_mut().unwrap();
        let node = nodes.iter_mut().find(|n| n["nodeId"] == *id).unwrap();
        node["properties"]
            .as_array_mut()
            .unwrap()
            .push(json!({"name":"focused","value":{"type":"booleanOrUndefined","value":true}}));
    }
}

/// The tree's id for the node of `frame` whose element is `backend`.
fn ax_node_of(engine: &Engine, frame: &str, backend: i64) -> String {
    super::lock(&engine.model).ax[frame]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["backendDOMNodeId"] == backend)
        .and_then(|n| n["nodeId"].as_str().map(str::to_owned))
        .unwrap()
}

#[test]
fn the_element_with_the_documents_focus_is_blanked_where_the_tree_marks_another() {
    // #2972: a field that names an active option (`aria-activedescendant`, a combo box) has the
    // document's focus while the tree marks the option focused: the field is blanked.
    let (browser, engine) = browser();
    let option = ax_node_of(&engine, "PAGE-1", 12);
    focus_on(&engine, &[("PAGE-1", option.as_str())]);
    super::lock(&engine.model).focused_backend = Some(20);
    browser.blank_focused(None).unwrap();
    let text = snapshot(&browser, 1);
    let line = |words: &str| {
        text.lines()
            .find(|l| l.contains(words))
            .map(str::to_owned)
            .unwrap_or_default()
    };
    assert!(line("textbox").contains("(blanked)"), "{text}");
    assert!(text.contains("- link \"Read more\""), "{text}");
}

#[test]
fn a_field_inside_a_shadow_root_is_blanked_though_its_document_names_the_host() {
    // #2972: a field inside a shadow root (open or closed) is the one the keys go to, while its
    // document names the shadow's host as what has the focus: the field is still blanked.
    let (browser, engine) = browser();
    let field = ax_node_of(&engine, "PAGE-1", 20);
    focus_on(&engine, &[("PAGE-1", field.as_str())]);
    {
        let mut model = super::lock(&engine.model);
        model.focused_backend = Some(30);
        model.shadow_of.insert(30, vec![20]);
    }
    browser.blank_focused(None).unwrap();
    let text = snapshot(&browser, 1);
    let line = |words: &str| {
        text.lines()
            .find(|l| l.contains(words))
            .map(str::to_owned)
            .unwrap_or_default()
    };
    assert!(line("textbox").contains("(blanked)"), "{text}");
}

#[test]
fn the_element_the_tree_marks_focused_is_blanked_where_its_document_agrees_or_cannot_say() {
    let (browser, engine) = browser();
    let field = ax_node_of(&engine, "PAGE-1", 20);
    focus_on(&engine, &[("PAGE-1", field.as_str())]);
    browser.blank_focused(None).unwrap();
    super::lock(&engine.model).focused_backend = Some(20);
    browser.blank_focused(None).unwrap();
    let text = snapshot(&browser, 1);
    assert!(text.contains("(blanked)"), "{text}");
}

#[test]
fn the_focused_element_in_a_frame_is_blanked_there() {
    let (browser, engine) = browser();
    focus_on(&engine, &[("PAYMENT-FRAME", "3")]);
    browser.blank_focused(None).unwrap();
    let text = snapshot(&browser, 1);
    assert!(text.contains("  - button ["), "{text}");
    assert!(text.contains("(blanked)"), "{text}");
    assert!(!text.contains("Pay now"), "{text}");
}

#[test]
fn the_focused_element_in_shadow_content_is_blanked() {
    let (browser, engine) = browser();
    focus_on(&engine, &[("PAGE-1", "21")]);
    browser.blank_focused(None).unwrap();
    let text = snapshot(&browser, 1);
    assert!(!text.contains("Inside the shadow"), "{text}");
    assert!(text.contains("- textbox \"Name\""), "{text}");
}

#[test]
fn of_two_elements_the_tree_marks_focused_the_one_its_document_says_has_the_focus_is_blanked() {
    // #2295: a real engine may mark a field's inner editor, or its document, focused with it.
    let (browser, engine) = browser();
    focus_on(&engine, &[("PAGE-1", "8"), ("PAYMENT-FRAME", "3")]);
    let backend = {
        let model = super::lock(&engine.model);
        model.ax["PAGE-1"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["nodeId"] == "8")
            .and_then(|n| n["backendDOMNodeId"].as_i64())
            .unwrap()
    };
    // Only the page's own document is asked here; the payment frame's says nothing.
    engine.on(move |sent, _| {
        (sent.method == "Runtime.evaluate"
            && sent.params["expression"] == FOCUSED_HERE
            && sent.session.as_deref() == Some("S-PAYMENT"))
        .then(|| Ok(json!({"result":{"type":"object","subtype":"null","value":null}})))
    });
    super::lock(&engine.model).focused_backend = Some(backend);
    assert!(browser.blank_focused(None).is_ok());
    assert!(
        engine
            .sent("DOM.describeNode")
            .iter()
            .any(|s| s.params["objectId"] == "FOCUSED")
    );
}

#[test]
fn one_element_the_tree_lists_twice_as_focused_is_blanked() {
    // A real engine may give one element two nodes of its tree (a field and its inner editor
    // share the field's id): it is still one element (#2295).
    let (browser, engine) = browser();
    focus_on(&engine, &[("PAGE-1", "8"), ("PAGE-1", "4")]);
    let backend = {
        let mut model = super::lock(&engine.model);
        let nodes = model.ax.get_mut("PAGE-1").unwrap().as_array_mut().unwrap();
        let backend = nodes
            .iter()
            .find(|n| n["nodeId"] == "8")
            .and_then(|n| n["backendDOMNodeId"].as_i64())
            .unwrap();
        let other = nodes.iter_mut().find(|n| n["nodeId"] == "4").unwrap();
        other["backendDOMNodeId"] = json!(backend);
        backend
    };
    super::lock(&engine.model).focused_backend = Some(backend);
    assert!(browser.blank_focused(None).is_ok());
}

#[test]
fn blanking_the_focus_is_refused_when_more_than_one_element_says_it_has_it() {
    let (browser, engine) = browser();
    focus_on(&engine, &[("PAGE-1", "8"), ("PAYMENT-FRAME", "3")]);
    assert_eq!(browser.blank_focused(None), Err(Refusal::FocusUnclear));
    focus_on(&engine, &[]);
    assert_eq!(browser.blank_focused(None), Err(Refusal::NothingFocused));
}

// Frames (#2012): the engine's screencast of the current page, only while shown, paced by
// when each frame is acknowledged.

type Shown = Arc<Mutex<Vec<Vec<u8>>>>;

fn shown_into(shown: &Shown) -> Arc<dyn Fn(crate::drive::Painted) + Send + Sync> {
    let shown = shown.clone();
    Arc::new(move |painted| super::lock(&shown).push(painted.jpeg))
}

fn a_frame(engine: &Engine, image: &[u8], ack: i64) {
    engine.emit(
        "Page.screencastFrame",
        json!({
            "data": base64::engine::general_purpose::STANDARD.encode(image),
            "metadata": {"offsetTop":0,"pageScaleFactor":1,"deviceWidth":1280,"deviceHeight":800,"scrollOffsetX":0,"scrollOffsetY":0},
            "sessionId": ack,
        }),
        Some("S-PAGE-1"),
    );
}

fn wait_for(what: &str, mut done: impl FnMut() -> bool) {
    let start = std::time::Instant::now();
    while !done() {
        assert!(start.elapsed() < Duration::from_secs(5), "{what}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn no_frame_is_asked_of_the_engine_until_the_browser_is_shown() {
    let (browser, engine) = browser();
    let shown: Shown = Arc::default();
    a_frame(&engine, b"early", 1);
    std::thread::sleep(Duration::from_millis(50));
    assert!(engine.sent("Page.startScreencast").is_empty());

    browser.show(shown_into(&shown));

    wait_for("the current page's screencast starts", || {
        !engine.sent("Page.startScreencast").is_empty()
    });
    let started = &engine.sent("Page.startScreencast")[0];
    assert_eq!(started.session.as_deref(), Some("S-PAGE-1"));
    assert_eq!(started.params["format"], "jpeg");
    assert!(
        super::lock(&shown).is_empty(),
        "a frame from before is not handed over"
    );
}

#[test]
fn each_frame_shown_is_handed_over_as_its_image_and_acknowledged() {
    let (browser, engine) = browser();
    let shown: Shown = Arc::default();
    browser.show(shown_into(&shown));
    wait_for("started", || {
        !engine.sent("Page.startScreencast").is_empty()
    });

    a_frame(&engine, b"\xff\xd8 one", 11);

    wait_for("acknowledged", || {
        !engine.sent("Page.screencastFrameAck").is_empty()
    });
    assert_eq!(*super::lock(&shown), [b"\xff\xd8 one".to_vec()]);
    let ack = &engine.sent("Page.screencastFrameAck")[0];
    assert_eq!(ack.params["sessionId"], 11);
    assert_eq!(ack.session.as_deref(), Some("S-PAGE-1"));
}

#[test]
fn each_frame_names_the_document_its_page_has_loaded_as_the_engine_records_it() {
    // #2300: what a browser view's chip names, from the engine's record, never the page.
    let (browser, engine) = browser();
    let addresses: Arc<Mutex<Vec<String>>> = Arc::default();
    let seen = Arc::clone(&addresses);
    browser.show(Arc::new(move |painted: crate::drive::Painted| {
        super::lock(&seen).push(painted.address);
    }));
    wait_for("started", || {
        !engine.sent("Page.startScreencast").is_empty()
    });
    a_frame(&engine, b"\xff\xd8 one", 1);
    wait_for("the first frame", || super::lock(&addresses).len() == 1);

    engine.emit(
        "Page.frameNavigated",
        json!({"frame":{"id":"PAGE-1","loaderId":"LOADER-5","url":"http://fixture.test/more.html?q=1","name":"","securityOrigin":"http://fixture.test","mimeType":"text/html"},"type":"Navigation"}),
        Some("S-PAGE-1"),
    );
    std::thread::sleep(Duration::from_millis(50));
    a_frame(&engine, b"\xff\xd8 two", 2);
    wait_for("the second frame", || super::lock(&addresses).len() == 2);

    assert_eq!(
        *super::lock(&addresses),
        [
            "http://fixture.test/frames.html",
            "http://fixture.test/more.html?q=1"
        ]
    );
}

#[test]
fn a_paced_browser_acknowledges_no_sooner_than_its_pace_so_the_engine_makes_fewer_frames() {
    let (browser, engine) = browser();
    let shown: Shown = Arc::default();
    browser.pace(Duration::from_millis(300));
    browser.show(shown_into(&shown));
    wait_for("started", || {
        !engine.sent("Page.startScreencast").is_empty()
    });

    let start = std::time::Instant::now();
    a_frame(&engine, b"one", 1);
    wait_for("the first acknowledged", || {
        engine.sent("Page.screencastFrameAck").len() == 1
    });
    a_frame(&engine, b"two", 2);
    wait_for("the second acknowledged", || {
        engine.sent("Page.screencastFrameAck").len() == 2
    });

    assert!(
        start.elapsed() >= Duration::from_millis(300),
        "the second frame was asked for {:?} after the first",
        start.elapsed()
    );
}

#[test]
fn a_hidden_browser_stops_its_screencast_and_hands_over_no_late_frame() {
    let (browser, engine) = browser();
    let shown: Shown = Arc::default();
    browser.show(shown_into(&shown));
    wait_for("started", || {
        !engine.sent("Page.startScreencast").is_empty()
    });

    browser.hide();

    wait_for("stopped", || !engine.sent("Page.stopScreencast").is_empty());
    a_frame(&engine, b"late", 5);
    std::thread::sleep(Duration::from_millis(50));
    assert!(super::lock(&shown).is_empty());
}

#[test]
fn whoever_asked_is_told_once_the_engine_ends() {
    let (browser, engine) = browser();
    let told = Arc::new(Mutex::new(0));
    let counting = told.clone();
    browser.when_it_ends(Box::new(move |_| *super::lock(&counting) += 1));

    // The engine's pipe closes: its events end.
    let (closed, _) = mpsc::sync_channel(1);
    *super::lock(&engine.events) = closed;

    wait_for("told", || *super::lock(&told) == 1);
}

#[test]
fn whoever_asked_is_told_the_connection_closed_and_why() {
    // #2745: a message the connection could not read ended it while the engine still ran.
    let (browser, engine) = browser();
    let told: Arc<Mutex<Option<Ending>>> = Arc::default();
    let telling = told.clone();
    browser.when_it_ends(Box::new(move |ending| {
        *super::lock(&telling) = Some(ending)
    }));

    let why = Closed::TooLarge {
        method: Some("Page.frameNavigated".into()),
        most: 64 << 20,
    };
    *super::lock(&engine.closed) = Some(why.clone());
    let (closed, _) = mpsc::sync_channel(1);
    *super::lock(&engine.events) = closed;

    wait_for("told", || super::lock(&told).is_some());
    assert_eq!(
        super::lock(&told).clone(),
        Some(Ending {
            by: EndedBy::LostConnection,
            exit: None,
            closed: Some(why),
        })
    );
}

#[test]
fn one_who_asks_after_the_end_is_told_at_once_how_it_ended() {
    let (browser, engine) = browser();
    let first: Arc<Mutex<Option<Ending>>> = Arc::default();
    let telling = first.clone();
    browser.when_it_ends(Box::new(move |ending| {
        *super::lock(&telling) = Some(ending)
    }));
    *super::lock(&engine.closed) = Some(Closed::PipeEnded);
    let (closed, _) = mpsc::sync_channel(1);
    *super::lock(&engine.events) = closed;
    wait_for("told", || super::lock(&first).is_some());

    let later: Arc<Mutex<Option<Ending>>> = Arc::default();
    let telling = later.clone();
    browser.when_it_ends(Box::new(move |ending| {
        *super::lock(&telling) = Some(ending)
    }));
    assert_eq!(*super::lock(&later), *super::lock(&first));
    assert_eq!(
        super::lock(&later).as_ref().map(|ending| ending.by),
        Some(EndedBy::Itself)
    );
}

/// An engine the stand-in browser holds: what was done to it, and how long it takes to end by
/// itself once its pipe has closed.
struct StandHold {
    stopped: Arc<Mutex<Option<Duration>>>,
    ends_after: Duration,
}

impl super::EngineHold for StandHold {
    fn pid(&self) -> u32 {
        4242
    }

    fn stop(self: Box<Self>, grace: Duration) -> Option<crate::ending::EngineExit> {
        *super::lock(&self.stopped) = Some(grace);
        Some(crate::ending::EngineExit::Code(0))
    }

    fn wait_or_end(self: Box<Self>, _grace: Duration) -> (Option<crate::ending::EngineExit>, bool) {
        std::thread::sleep(self.ends_after);
        (Some(crate::ending::EngineExit::Code(0)), true)
    }
}

#[test]
fn a_browser_dropped_unstopped_ends_its_engine_and_says_purlis_stopped_it() {
    // #2745: its event reader keeps what the engine lives in until the pipe closes, so the
    // drop is what ends it.
    let (browser, _engine) = browser();
    let stopped: Arc<Mutex<Option<Duration>>> = Arc::default();
    browser.holding(Box::new(StandHold {
        stopped: Arc::clone(&stopped),
        ends_after: Duration::ZERO,
    }));
    let told: Arc<Mutex<Option<Ending>>> = Arc::default();
    let telling = told.clone();
    browser.when_it_ends(Box::new(move |ending| {
        *super::lock(&telling) = Some(ending)
    }));

    drop(browser);

    assert_eq!(*super::lock(&stopped), Some(Duration::ZERO));
    assert_eq!(
        super::lock(&told).as_ref().map(|ending| ending.by),
        Some(EndedBy::Stopped)
    );
}

#[test]
fn a_stop_while_the_engine_ends_by_itself_returns_once_its_end_is_told() {
    // #2745: a quit's stop waits for the end its pipe began, so a kept login's write-back in
    // the teller is not skipped.
    let (browser, engine) = browser();
    browser.holding(Box::new(StandHold {
        stopped: Arc::default(),
        ends_after: Duration::from_millis(400),
    }));
    let told = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let telling = Arc::clone(&told);
    browser.when_it_ends(Box::new(move |_| {
        telling.store(true, std::sync::atomic::Ordering::SeqCst);
    }));
    let (closed, _) = mpsc::sync_channel(1);
    *super::lock(&engine.events) = closed;
    // The event reader has taken the engine, and waits for it to end by itself.
    wait_for("taken", || browser.process().is_none());

    browser.stop(Duration::from_millis(100));

    assert!(told.load(std::sync::atomic::Ordering::SeqCst));
}
