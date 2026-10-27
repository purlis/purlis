// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Inspect (#2028): the engine's own DevTools, for the person while they drive, speaking to the
//! inspected page through a tapped session, reaching nothing past it, and closed as they hand
//! the browser back.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine as _;
use serde_json::{Value, json};

use super::takeover::key;
use super::{browser, browser_judged, id, snapshot, wait_for};
use crate::drive::{
    Aim, Call, Content, Gate, InspectEnded, MouseButton, MouseKind, PageId, Painted, PersonInput,
    Refusal,
};

/// The engine answers DevTools' setting up as the real one does: a context of its own, its
/// front end's page in it, and a session of its own on the inspected page.
fn answers_devtools(engine: &super::Engine) {
    engine.on(|sent, engine| match sent.method.as_str() {
        "Target.createBrowserContext" => Some(Ok(json!({ "browserContextId": "CONTEXT-DEVTOOLS" }))),
        "Target.createTarget" => {
            engine.emit(
                "Target.targetCreated",
                json!({"targetInfo":{"targetId":"FRONT-1","type":"page","title":"","url":"about:blank","attached":false,"browserContextId":"CONTEXT-DEVTOOLS"}}),
                None,
            );
            Some(Ok(json!({ "targetId": "FRONT-1" })))
        }
        "Target.attachToTarget" if sent.params["targetId"] == "PAGE-1" => {
            engine.emit(
                "Target.attachedToTarget",
                json!({"sessionId":"S-INSPECTED","targetInfo":{"targetId":"PAGE-1","type":"page","title":"","url":"http://fixture.test/frames.html","attached":true,"browserContextId":"CONTEXT-1"},"waitingForDebugger":false}),
                None,
            );
            Some(Ok(json!({ "sessionId": "S-INSPECTED" })))
        }
        _ => None,
    });
}

type Ends = Arc<Mutex<Vec<InspectEnded>>>;

/// The person takes the browser over and inspects it: how its ends are told.
fn inspected(browser: &super::Browser, engine: &super::Engine) -> Ends {
    answers_devtools(engine);
    browser.take_over();
    let ends: Ends = Arc::default();
    let told = Arc::clone(&ends);
    browser
        .inspect(
            1000,
            600,
            Box::new(move |why| told.lock().unwrap().push(why)),
        )
        .expect("the person may inspect a browser they drive");
    ends
}

/// The front end sends `message` through its stand-in.
fn front_end_sends(engine: &super::Engine, message: &Value) {
    engine.emit(
        "Runtime.bindingCalled",
        json!({ "name": "__purlisInspect", "payload": message.to_string(), "executionContextId": 1 }),
        Some("S-FRONT-1"),
    );
}

/// Every message handed to the front end so far, in order.
fn handed_to_the_front_end(engine: &super::Engine) -> Vec<Value> {
    engine
        .sent("Runtime.evaluate")
        .into_iter()
        .filter(|s| s.session.as_deref() == Some("S-FRONT-1"))
        .flat_map(|s| {
            let expression = s.params["expression"].as_str().unwrap().to_owned();
            let literal = expression
                .strip_prefix("globalThis.__purlisInspector.deliver(")
                .and_then(|rest| rest.strip_suffix(')'))
                .expect("only the stand-in's deliver is run in the front end")
                .to_owned();
            let batch: String = serde_json::from_str(&literal).unwrap();
            serde_json::from_str::<Vec<String>>(&batch)
                .unwrap()
                .into_iter()
                .map(|m| serde_json::from_str::<Value>(&m).unwrap())
                .collect::<Vec<_>>()
        })
        .collect()
}

#[test]
fn devtools_open_only_for_the_person_driving_the_browser() {
    let (browser, engine) = browser();
    answers_devtools(&engine);
    assert_eq!(
        browser.inspect(1000, 600, Box::new(|_| {})),
        Err(Refusal::NotDriving)
    );
    assert!(engine.sent("Target.createBrowserContext").is_empty());
    assert!(!browser.inspecting());
}

#[test]
fn the_front_end_runs_the_engines_own_devtools_in_a_context_of_its_own_that_is_never_a_page() {
    let (browser, engine) = browser();
    let _ends = inspected(&browser, &engine);
    assert!(browser.inspecting());
    let made = engine.sent("Target.createTarget");
    assert_eq!(made[0].params["browserContextId"], "CONTEXT-DEVTOOLS");
    let went = engine.sent("Page.navigate");
    let front = went
        .iter()
        .find(|s| s.session.as_deref() == Some("S-FRONT-1"))
        .expect("the front end is sent to the engine's own DevTools");
    assert_eq!(front.params["url"], super::super::inspect::FRONT_END);
    // Its stand-in socket is in before any of its scripts.
    let stand_in = engine.sent("Page.addScriptToEvaluateOnNewDocument");
    assert!(
        stand_in
            .iter()
            .any(|s| s.session.as_deref() == Some("S-FRONT-1")
                && s.params["source"].as_str().unwrap().contains("'WebSocket'"))
    );
    // The driver neither closed it as a page that is not a web address, nor set it up, nor
    // let it go: it is not one of its pages.
    assert!(
        !engine
            .sent("Target.closeTarget")
            .iter()
            .any(|s| s.params["targetId"] == "FRONT-1")
    );
    assert!(!engine
        .sent("Target.detachFromTarget")
        .iter()
        .any(|s| s.params["sessionId"] == "S-FRONT-1" || s.params["sessionId"] == "S-INSPECTED"));
    browser.hand_back();
    let pages = browser.call(&id(1), Call::Pages).unwrap();
    let Content::Pages(listed) = pages.content else {
        panic!("the pages are listed");
    };
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert_eq!(listed[0].id, PageId::new("p1"));
}

#[test]
fn a_page_devtools_context_opens_is_closed_at_once() {
    let (browser, engine) = browser();
    let _ends = inspected(&browser, &engine);
    engine.emit(
        "Target.targetCreated",
        json!({"targetInfo":{"targetId":"OPENED","type":"page","title":"","url":"https://example.com/","attached":false,"browserContextId":"CONTEXT-DEVTOOLS"}}),
        None,
    );
    wait_for("the page DevTools opened is closed", || {
        engine
            .sent("Target.closeTarget")
            .iter()
            .any(|s| s.params["targetId"] == "OPENED")
    });
}

#[test]
fn what_the_page_says_reaches_the_front_end_in_order_and_what_it_asks_reaches_the_page() {
    let (browser, engine) = browser();
    let _ends = inspected(&browser, &engine);
    engine.emit("DOM.documentUpdated", json!({}), Some("S-INSPECTED"));
    engine.emit(
        "Target.attachedToTarget",
        json!({"sessionId":"S-WORKER","targetInfo":{"targetId":"W","type":"worker"},"waitingForDebugger":true}),
        Some("S-INSPECTED"),
    );
    engine.emit(
        "Runtime.consoleAPICalled",
        json!({"type":"log"}),
        Some("S-WORKER"),
    );
    front_end_sends(
        &engine,
        &json!({"id": 1, "method": "DOM.getDocument", "params": {"depth": 1}}),
    );
    front_end_sends(
        &engine,
        &json!({"id": 2, "method": "Runtime.evaluate", "params": {"expression": "1"}, "sessionId": "S-WORKER"}),
    );
    wait_for("both answers are handed to the front end", || {
        handed_to_the_front_end(&engine)
            .iter()
            .filter(|m| m.get("id").is_some())
            .count()
            == 2
    });
    let handed = handed_to_the_front_end(&engine);
    assert_eq!(handed[0]["method"], "DOM.documentUpdated");
    assert!(
        handed[0].get("sessionId").is_none(),
        "the page's own session is the front end's root"
    );
    assert_eq!(handed[1]["method"], "Target.attachedToTarget");
    assert_eq!(handed[2]["method"], "Runtime.consoleAPICalled");
    assert_eq!(handed[2]["sessionId"], "S-WORKER");
    assert_eq!(handed[3]["id"], 1);
    assert!(handed[3].get("result").is_some());
    assert_eq!(handed[4]["id"], 2);
    // Sent on the session they were meant for: the page's, and the worker's under it.
    assert!(
        engine
            .sent("DOM.getDocument")
            .iter()
            .any(|s| s.session.as_deref() == Some("S-INSPECTED"))
    );
    assert!(
        engine
            .sent("Runtime.evaluate")
            .iter()
            .any(|s| s.session.as_deref() == Some("S-WORKER"))
    );
    assert_eq!(
        browser.inspector_heard(),
        vec!["DOM.getDocument", "Runtime.evaluate"]
    );
}

#[test]
fn what_reaches_past_the_inspected_page_is_refused_to_the_front_end_and_never_sent() {
    let (browser, engine) = browser();
    let _ends = inspected(&browser, &engine);
    front_end_sends(
        &engine,
        &json!({"id": 1, "method": "Browser.close", "params": {}}),
    );
    front_end_sends(
        &engine,
        &json!({"id": 2, "method": "Target.createTarget", "params": {"url": "https://example.com/"}}),
    );
    front_end_sends(
        &engine,
        &json!({"id": 3, "method": "Page.navigate", "params": {"url": "file:///etc/hosts"}}),
    );
    // The driver's own session on the page is not the front end's to speak on.
    front_end_sends(
        &engine,
        &json!({"id": 4, "method": "Runtime.evaluate", "params": {"expression": "1"}, "sessionId": "S-PAGE-1"}),
    );
    wait_for("each is refused to the front end", || {
        handed_to_the_front_end(&engine)
            .iter()
            .filter(|m| m.get("error").is_some())
            .count()
            == 4
    });
    assert!(engine.sent("Browser.close").is_empty());
    assert_eq!(
        engine.sent("Target.createTarget").len(),
        1,
        "only the front end's own page"
    );
    assert!(
        !engine
            .sent("Page.navigate")
            .iter()
            .any(|s| s.params["url"] == "file:///etc/hosts")
    );
    assert!(
        !engine
            .sent("Runtime.evaluate")
            .iter()
            .any(|s| { s.session.as_deref() == Some("S-PAGE-1") && s.params["expression"] == "1" })
    );
}

#[test]
fn handing_the_browser_back_closes_devtools_and_throws_away_all_it_kept() {
    let (browser, engine) = browser();
    let ends = inspected(&browser, &engine);
    browser.hand_back();
    assert!(!browser.inspecting());
    assert_eq!(*ends.lock().unwrap(), vec![InspectEnded::Closed]);
    assert!(
        engine
            .sent("Target.detachFromTarget")
            .iter()
            .any(|s| s.params["sessionId"] == "S-INSPECTED")
    );
    assert!(
        engine
            .sent("Target.disposeBrowserContext")
            .iter()
            .any(|s| s.params["browserContextId"] == "CONTEXT-DEVTOOLS")
    );
    // What the page says after is the driver's again, and nothing more is handed to DevTools.
    let handed = handed_to_the_front_end(&engine).len();
    engine.emit("DOM.documentUpdated", json!({}), Some("S-INSPECTED"));
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(handed_to_the_front_end(&engine).len(), handed);
}

#[test]
fn devtools_close_when_the_page_they_inspect_goes() {
    let (browser, engine) = browser();
    let ends = inspected(&browser, &engine);
    engine.emit(
        "Target.detachedFromTarget",
        json!({"sessionId": "S-INSPECTED"}),
        None,
    );
    wait_for("the Inspect ends", || !browser.inspecting());
    assert_eq!(*ends.lock().unwrap(), vec![InspectEnded::PageGone]);
}

fn front_end_frame(engine: &super::Engine, ack: i64) {
    engine.emit(
        "Page.screencastFrame",
        json!({ "data": base64::engine::general_purpose::STANDARD.encode(b"devtools-jpeg"), "metadata": {}, "sessionId": ack }),
        Some("S-FRONT-1"),
    );
}

#[test]
fn the_front_ends_frames_go_to_its_own_watchers_alone() {
    let (browser, engine) = browser();
    let _ends = inspected(&browser, &engine);
    let page_frames: Arc<Mutex<Vec<Painted>>> = Arc::default();
    let seen = Arc::clone(&page_frames);
    browser.show(Arc::new(move |p| seen.lock().unwrap().push(p)));
    let devtools_frames: Arc<Mutex<Vec<Painted>>> = Arc::default();
    let seen = Arc::clone(&devtools_frames);
    browser.show_inspector(Arc::new(move |p| seen.lock().unwrap().push(p)));
    wait_for("the front end is cast", || {
        engine
            .sent("Page.startScreencast")
            .iter()
            .any(|s| s.session.as_deref() == Some("S-FRONT-1"))
    });
    front_end_frame(&engine, 9);
    wait_for("the frame is acknowledged", || {
        engine
            .sent("Page.screencastFrameAck")
            .iter()
            .any(|s| s.session.as_deref() == Some("S-FRONT-1"))
    });
    let frames = devtools_frames.lock().unwrap();
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].jpeg, b"devtools-jpeg");
    assert_eq!(frames[0].aim.page, PageId::new("devtools"));
    assert_eq!(
        frames[0].address, "",
        "a frame of DevTools names no page's address"
    );
    assert!(
        page_frames
            .lock()
            .unwrap()
            .iter()
            .all(|p| p.jpeg != b"devtools-jpeg")
    );
}

#[test]
fn the_persons_input_reaches_the_front_end_and_nothing_aimed_elsewhere_does() {
    let (browser, engine) = browser();
    let _ends = inspected(&browser, &engine);
    let at = browser.inspector_aim().expect("DevTools are open");
    browser
        .inspector_input(&PersonInput::Mouse {
            aim: at.clone(),
            kind: MouseKind::Pressed,
            x: 10.0,
            y: 20.0,
            button: MouseButton::Left,
            clicks: 1,
            modifiers: 0,
            delta_x: 0.0,
            delta_y: 0.0,
        })
        .unwrap();
    let pressed = engine.sent("Input.dispatchMouseEvent");
    assert_eq!(
        pressed.last().unwrap().session.as_deref(),
        Some("S-FRONT-1")
    );
    // A key aimed at the page, not at DevTools, is dropped.
    let page_aim = browser.aim().unwrap();
    assert_eq!(
        browser.inspector_input(&key(&page_aim, "a", true)),
        Err(Refusal::Missed)
    );
    let moved_on = Aim {
        document: at.document + 1,
        ..at
    };
    assert_eq!(
        browser.inspector_input(&key(&moved_on, "a", true)),
        Err(Refusal::Missed)
    );
}

#[test]
fn what_the_person_types_into_devtools_is_kept_out_of_every_later_answer() {
    let (browser, engine) = browser();
    let _ends = inspected(&browser, &engine);
    let at = browser.inspector_aim().unwrap();
    for letter in ["z", "q", "7", "k", "p"] {
        browser.inspector_input(&key(&at, letter, true)).unwrap();
        browser.inspector_input(&key(&at, letter, false)).unwrap();
    }
    assert!(
        engine
            .sent("Input.dispatchKeyEvent")
            .iter()
            .all(|s| s.session.as_deref() == Some("S-FRONT-1"))
    );
    browser.hand_back();
    // The page now shows what was typed into DevTools (written into it from the console).
    {
        let mut model = super::super::lock(&engine.model);
        let nodes = model.ax.get_mut("PAGE-1").unwrap().as_array_mut().unwrap();
        let line = nodes.iter_mut().find(|n| n["nodeId"] == "16").unwrap();
        line["name"]["value"] = json!("zq7kp");
    }
    let read = snapshot(&browser, 2);
    assert!(!read.contains("zq7kp"), "{read}");
}

#[test]
fn the_front_ends_own_requests_to_the_web_fail() {
    let (browser, engine) = browser_judged(Arc::new(|_| Gate::Go));
    let _ends = inspected(&browser, &engine);
    engine.emit(
        "Fetch.requestPaused",
        json!({"requestId":"R-DEVTOOLS","request":{"url":"https://example.com/app.js.map","method":"GET"},"frameId":"FRONT-1","resourceType":"Other"}),
        None,
    );
    wait_for("the front end's request fails", || {
        engine
            .sent("Fetch.failRequest")
            .iter()
            .any(|s| s.params["requestId"] == "R-DEVTOOLS")
    });
    assert!(
        !engine
            .sent("Fetch.continueRequest")
            .iter()
            .any(|s| s.params["requestId"] == "R-DEVTOOLS")
    );
}

#[test]
fn what_devtools_put_into_a_field_of_the_page_is_kept_out_once_the_browser_is_handed_back() {
    let (browser, engine) = browser();
    let _ends = inspected(&browser, &engine);
    // From the console, the person writes a value into the page's Name field.
    {
        let mut model = super::super::lock(&engine.model);
        model.value = "console-set-Secret8".into();
        let nodes = model.ax.get_mut("PAGE-1").unwrap().as_array_mut().unwrap();
        let field = nodes.iter_mut().find(|n| n["nodeId"] == "8").unwrap();
        field["value"]["value"] = json!("console-set-Secret8");
    }
    browser.hand_back();
    let read = snapshot(&browser, 2);
    assert!(!read.contains("console-set-Secret8"), "{read}");
}
