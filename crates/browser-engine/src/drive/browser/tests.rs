// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The driver against a stand-in engine that answers DevTools as the real one does, from the
//! recorded trees of the fixture site's frames page. What only a real engine can show (that the
//! page really changed) is in `tests/the_fixture_sites_hard_pages_pass.rs`.

use std::collections::HashMap;
use std::sync::mpsc::{self, SyncSender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine as _;
use serde_json::{Value, json};

use super::{Browser, DevTools, READY_STATE, SELECT_OPTIONS};
use crate::WebAddress;
use crate::cdp::{CallError, Event};
use crate::drive::{
    Answer, Call, CallId, Content, DragTo, Field, Limits, PageId, Ref, Refusal, Settled,
};

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
    history: Value,
    screenshot: Vec<u8>,
    select: Value,
    pages: u32,
    pixel_ratio: f64,
}

/// A stand-in engine: answers the driver's calls and sends it events, as the real one would.
struct Engine {
    events: Mutex<SyncSender<Event>>,
    sent: Mutex<Vec<Sent>>,
    model: Mutex<Model>,
    hook: Mutex<Option<Hook>>,
    dropped: Mutex<u64>,
}

impl Engine {
    fn emit(&self, method: &str, params: Value, session: Option<&str>) {
        let _ = super::lock(&self.events).send(Event {
            method: method.to_owned(),
            params,
            session_id: session.map(str::to_owned),
        });
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
                    json!({"frameTree":{"frame":{"id":id,"loaderId":"L","url":"http://fixture.test/new.html","name":""}}})
                }
                None => json!({}),
            }),
            "Accessibility.getFullAXTree" => {
                let frame = p["frameId"].as_str().unwrap_or_default();
                let model = super::lock(&self.model);
                Ok(json!({ "nodes": model.ax.get(frame).cloned().unwrap_or(json!([])) }))
            }
            "DOM.getFrameOwner" => Ok(json!({"backendNodeId": 50, "nodeId": 0})),
            "DOM.getBoxModel" => Ok(
                json!({"model":{"content":[100,200,300,200,300,260,100,260],"padding":[],"border":[],"margin":[],"width":200,"height":60}}),
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
            "Page.getLayoutMetrics" => {
                let ratio = super::lock(&self.model).pixel_ratio;
                Ok(json!({
                    "layoutViewport":{"pageX":0,"pageY":0,"clientWidth":800.0 * ratio,"clientHeight":600.0 * ratio},
                    "cssLayoutViewport":{"pageX":0,"pageY":0,"clientWidth":800,"clientHeight":600},
                    "cssVisualViewport":{"offsetX":0,"offsetY":0,"pageX":0,"pageY":0,"clientWidth":800,"clientHeight":600,"scale":1,"zoom":1}
                }))
            }
            "DOM.resolveNode" => {
                Ok(json!({"object":{"type":"object","subtype":"node","objectId":"OBJECT-1"}}))
            }
            "Runtime.callFunctionOn" => {
                Ok(json!({"result": {"type":"object","value": super::lock(&self.model).select}}))
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
        _timeout: Duration,
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
        hooked.unwrap_or_else(|| self.0.answer(&sent))
    }

    fn dropped_events(&self) -> u64 {
        *super::lock(&self.0.dropped)
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
            ]),
            ready: "complete".into(),
            history: json!({"currentIndex":0,"entries":[{"id":1,"url":"http://fixture.test/frames.html","userTypedURL":"","title":"","transitionType":"typed"}]}),
            screenshot: white_png(),
            select: json!({"picked":["Blue"]}),
            pages: 1,
            pixel_ratio: 1.0,
        }),
        hook: Mutex::new(None),
        dropped: Mutex::new(0),
    });
    let browser = Browser::over(Arc::new(Stand(engine.clone())), receiver, limits).unwrap();
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
    assert_eq!(chose.params["arguments"], json!([{ "value": ["Blue"] }]));
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
            "Runtime.evaluate" => assert_eq!(sent.params["expression"], READY_STATE),
            "Runtime.callFunctionOn" => {
                assert_eq!(sent.params["functionDeclaration"], SELECT_OPTIONS);
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
    browser.when_it_ends(Box::new(move || *super::lock(&counting) += 1));

    // The engine's pipe closes: its events end.
    let (closed, _) = mpsc::sync_channel(1);
    *super::lock(&engine.events) = closed;

    wait_for("told", || *super::lock(&told) == 1);
}
