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
use crate::WebAddress;
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
    // The front end's own main world, where its stand-in runs.
    engine.emit(
        "Runtime.executionContextCreated",
        json!({"context":{"id":1,"origin":"devtools://devtools","name":"","uniqueId":"FRONT-WORLD","auxData":{"isDefault":true,"type":"default","frameId":"FRONT-1"}}}),
        Some("S-FRONT-1"),
    );
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
        .sent("Runtime.callFunctionOn")
        .into_iter()
        .filter(|s| s.session.as_deref() == Some("S-FRONT-1"))
        .flat_map(|s| {
            assert_eq!(
                s.params["functionDeclaration"],
                super::super::inspect::DELIVER,
                "only the stand-in's deliver is run in the front end"
            );
            assert_eq!(s.params["executionContextId"], 1, "in its own main world");
            s.params["arguments"][0]["value"]
                .as_array()
                .unwrap()
                .iter()
                .map(|m| serde_json::from_str::<Value>(m.as_str().unwrap()).unwrap())
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
    assert_eq!(
        made[0].params["newWindow"], true,
        "in front in a window of its own"
    );
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

#[test]
fn devtools_watched_before_their_page_has_loaded_are_cast_once_it_has() {
    let (browser, engine) = browser();
    let _ends = inspected(&browser, &engine);
    browser.show_inspector(Arc::new(|_| {}));
    wait_for("the front end is cast", || {
        engine
            .sent("Page.startScreencast")
            .iter()
            .any(|s| s.session.as_deref() == Some("S-FRONT-1"))
    });
    let asked = || {
        engine
            .sent("Page.startScreencast")
            .iter()
            .filter(|s| s.session.as_deref() == Some("S-FRONT-1"))
            .count()
    };
    let before = asked();
    engine.emit(
        "Page.loadEventFired",
        json!({ "timestamp": 1.0 }),
        Some("S-FRONT-1"),
    );
    wait_for("the front end is cast again once loaded", || {
        asked() > before
    });
}

/// The page shows `said` in its text, as a page that DevTools wrote into does.
fn the_page_shows(engine: &super::Engine, said: &str) {
    let mut model = super::super::lock(&engine.model);
    let nodes = model.ax.get_mut("PAGE-1").unwrap().as_array_mut().unwrap();
    let line = nodes.iter_mut().find(|n| n["nodeId"] == "16").unwrap();
    line["name"]["value"] = json!(said);
}

#[test]
fn devtools_that_leave_their_own_page_close() {
    let (browser, engine) = browser();
    let ends = inspected(&browser, &engine);
    engine.emit(
        "Target.targetInfoChanged",
        json!({"targetInfo":{"targetId":"FRONT-1","type":"page","title":"","url":"chrome://settings/","attached":true,"browserContextId":"CONTEXT-DEVTOOLS"}}),
        None,
    );
    wait_for("DevTools close", || !browser.inspecting());
    assert_eq!(*ends.lock().unwrap(), vec![InspectEnded::Moved]);
}

#[test]
fn devtools_at_their_own_page_stay_open() {
    let (browser, engine) = browser();
    let _ends = inspected(&browser, &engine);
    engine.emit(
        "Target.targetInfoChanged",
        json!({"targetInfo":{"targetId":"FRONT-1","type":"page","title":"DevTools","url":super::super::inspect::FRONT_END,"attached":true,"browserContextId":"CONTEXT-DEVTOOLS"}}),
        None,
    );
    std::thread::sleep(Duration::from_millis(100));
    assert!(browser.inspecting());
}

#[test]
fn what_devtools_put_into_a_field_is_kept_out_even_when_they_closed_before_the_hand_back() {
    let (browser, engine) = browser();
    let ends = inspected(&browser, &engine);
    {
        let mut model = super::super::lock(&engine.model);
        model.value = "console-set-Secret8".into();
        let nodes = model.ax.get_mut("PAGE-1").unwrap().as_array_mut().unwrap();
        let field = nodes.iter_mut().find(|n| n["nodeId"] == "8").unwrap();
        field["value"]["value"] = json!("console-set-Secret8");
    }
    // The engine says DevTools' session on the page went: they close before Done.
    engine.emit(
        "Target.detachedFromTarget",
        json!({"sessionId": "S-INSPECTED"}),
        None,
    );
    wait_for("DevTools close", || !browser.inspecting());
    assert_eq!(*ends.lock().unwrap(), vec![InspectEnded::PageGone]);
    browser.hand_back();
    let read = snapshot(&browser, 2);
    assert!(!read.contains("console-set-Secret8"), "{read}");
}

#[test]
fn what_devtools_wrote_outside_a_field_is_kept_out_after_the_hand_back() {
    let (browser, engine) = browser();
    let _ends = inspected(&browser, &engine);
    front_end_sends(
        &engine,
        &json!({"id": 3, "method": "Runtime.evaluate", "params": {"expression": "document.body.append('devtools-wrote-Q7')"}}),
    );
    wait_for("the call reaches the page", || {
        !engine
            .sent("Runtime.evaluate")
            .iter()
            .all(|s| s.session.as_deref() != Some("S-INSPECTED"))
    });
    browser.hand_back();
    the_page_shows(&engine, "devtools-wrote-Q7");
    let read = snapshot(&browser, 2);
    assert!(!read.contains("devtools-wrote-Q7"), "{read}");
}

#[test]
fn purlis_own_world_in_the_page_is_never_told_to_devtools_nor_reached_by_them() {
    let (browser, engine) = browser();
    let _ends = inspected(&browser, &engine);
    engine.emit(
        "Runtime.executionContextCreated",
        json!({"context":{"id":9,"origin":"http://fixture.test","name":"purlis","uniqueId":"PURLIS-9","auxData":{"isDefault":false,"type":"isolated","frameId":"PAGE-1"}}}),
        Some("S-INSPECTED"),
    );
    engine.emit("DOM.documentUpdated", json!({}), Some("S-INSPECTED"));
    front_end_sends(
        &engine,
        &json!({"id": 4, "method": "Runtime.evaluate", "params": {"expression": "1", "contextId": 9}}),
    );
    wait_for("the call is answered", || {
        handed_to_the_front_end(&engine)
            .iter()
            .any(|m| m["id"] == 4)
    });
    let handed = handed_to_the_front_end(&engine);
    assert!(
        handed
            .iter()
            .all(|m| m["method"] != "Runtime.executionContextCreated"),
        "{handed:?}"
    );
    assert!(handed.iter().any(|m| m["method"] == "DOM.documentUpdated"));
    assert!(
        handed
            .iter()
            .find(|m| m["id"] == 4)
            .unwrap()
            .get("error")
            .is_some()
    );
}

#[test]
fn purlis_own_worlds_are_made_afresh_after_devtools_were_open() {
    let (browser, engine) = browser();
    snapshot(&browser, 1);
    let made = engine.sent("Page.createIsolatedWorld").len();
    let _ends = inspected(&browser, &engine);
    browser.hand_back();
    snapshot(&browser, 2);
    let worlds = engine.sent("Page.createIsolatedWorld");
    assert!(worlds.len() > made);
    // Under a name of its own, so the engine makes a new world, not the one it had.
    let before = &worlds[made - 1].params["worldName"];
    let after = &worlds.last().unwrap().params["worldName"];
    assert_ne!(before, after);
    assert!(after.as_str().unwrap().starts_with("purlis"));
}

#[test]
fn a_message_of_another_world_of_the_front_end_is_not_its_own() {
    let (browser, engine) = browser();
    let _ends = inspected(&browser, &engine);
    engine.emit(
        "Runtime.bindingCalled",
        json!({ "name": "__purlisInspect", "payload": json!({"id": 5, "method": "DOM.getDocument"}).to_string(), "executionContextId": 77 }),
        Some("S-FRONT-1"),
    );
    front_end_sends(&engine, &json!({"id": 6, "method": "DOM.enable"}));
    wait_for("its own is answered", || {
        handed_to_the_front_end(&engine)
            .iter()
            .any(|m| m["id"] == 6)
    });
    assert!(engine.sent("DOM.getDocument").is_empty());
}

#[test]
fn a_message_too_large_on_devtools_sessions_closes_devtools_alone() {
    let (browser, engine) = browser();
    let ends = inspected(&browser, &engine);
    let sink = super::super::lock(&engine.taps)
        .iter()
        .find(|(_, sessions, _)| sessions.contains("S-INSPECTED"))
        .map(|(_, _, sink)| Arc::clone(sink))
        .unwrap();
    sink(crate::cdp::Tapped::TooLarge);
    wait_for("DevTools close", || !browser.inspecting());
    assert_eq!(*ends.lock().unwrap(), vec![InspectEnded::TooLarge]);
    // The browser answers on.
    browser.hand_back();
    snapshot(&browser, 2);
}

#[test]
fn an_inspect_under_way_never_finishes_opening_once_the_browser_is_handed_back() {
    let (browser, engine) = browser();
    let browser = Arc::new(browser);
    answers_devtools(&engine);
    let (reached, waits) = std::sync::mpsc::channel::<()>();
    let (go, gate) = std::sync::mpsc::channel::<()>();
    let gate = Mutex::new(gate);
    let reached = Mutex::new(reached);
    let devtools = Arc::clone(&engine);
    engine.on(move |sent, engine| {
        if sent.method == "Runtime.addBinding" {
            let _ = reached.lock().unwrap().send(());
            let _ = gate.lock().unwrap().recv_timeout(Duration::from_secs(1));
        }
        let _ = &devtools;
        match sent.method.as_str() {
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
                Some(Ok(json!({ "sessionId": "S-INSPECTED" })))
            }
            _ => None,
        }
    });
    browser.take_over();
    let opening = Arc::clone(&browser);
    let opened = std::thread::spawn(move || opening.inspect(1000, 600, Box::new(|_| {})));
    waits.recv_timeout(Duration::from_secs(5)).unwrap();
    // Handed back, and taken over again, while it was opening: the person drives again, under
    // another lease.
    browser.hand_back();
    browser.take_over();
    go.send(()).unwrap();
    assert_eq!(opened.join().unwrap(), Err(Refusal::NotDriving));
    assert!(!browser.inspecting());
}

#[test]
fn an_inspect_begun_while_the_browser_is_being_handed_back_never_opens() {
    let (browser, engine) = browser();
    answers_devtools(&engine);
    browser.take_over();
    // As a hand-back is between its first step and the lease changing hands.
    super::super::lock(&browser.lease).handing_back = true;
    assert_eq!(
        browser.inspect(1000, 600, Box::new(|_| {})),
        Err(Refusal::NotDriving)
    );
    assert!(!browser.inspecting());
}

/// The front end says how it runs, as the engine answers purlis's question once it has loaded.
fn the_front_end_runs(engine: &Arc<super::Engine>, found: &'static str) {
    answers_devtools(engine);
    let mut earlier = super::super::lock(&engine.hook).take();
    engine.on(move |sent, engine| {
        if sent.method == "Runtime.evaluate"
            && sent.params["expression"] == super::super::inspect::HOSTED
        {
            return Some(Ok(
                json!({ "result": { "type": "string", "value": found } }),
            ));
        }
        earlier.as_mut().and_then(|hook| hook(sent, engine))
    });
}

#[test]
fn devtools_close_where_the_engines_embedder_interface_reaches_the_front_end() {
    for found in [
        r#"{"host":"object","guarded":true,"sockets":1,"hosted":true}"#,
        r#"{"host":"undefined","guarded":true,"sockets":1,"hosted":false}"#,
        r#"{"host":"undefined","guarded":false,"sockets":1,"hosted":null}"#,
    ] {
        let (browser, engine) = browser();
        the_front_end_runs(&engine, found);
        browser.take_over();
        let ends: Ends = Arc::default();
        let told = Arc::clone(&ends);
        browser
            .inspect(
                1000,
                600,
                Box::new(move |why| told.lock().unwrap().push(why)),
            )
            .unwrap();
        engine.emit(
            "Page.loadEventFired",
            json!({ "timestamp": 1.0 }),
            Some("S-FRONT-1"),
        );
        wait_for("DevTools close", || !browser.inspecting());
        assert_eq!(*ends.lock().unwrap(), vec![InspectEnded::Ended], "{found}");
    }
}

#[test]
fn devtools_stay_open_while_the_front_end_runs_hosted_or_has_no_host_yet() {
    for found in [
        r#"{"host":"undefined","guarded":true,"sockets":1,"hosted":true}"#,
        r#"{"host":"undefined","guarded":true,"sockets":0,"hosted":null}"#,
    ] {
        let (browser, engine) = browser();
        the_front_end_runs(&engine, found);
        browser.take_over();
        browser.inspect(1000, 600, Box::new(|_| {})).unwrap();
        engine.emit(
            "Page.loadEventFired",
            json!({ "timestamp": 1.0 }),
            Some("S-FRONT-1"),
        );
        std::thread::sleep(Duration::from_millis(100));
        assert!(browser.inspecting(), "{found}");
    }
}

#[test]
fn a_page_that_cannot_be_read_at_the_hand_back_answers_nothing_until_it_can() {
    let (browser, engine) = browser();
    let _ends = inspected(&browser, &engine);
    {
        let mut model = super::super::lock(&engine.model);
        model.value = "console-set-Secret8".into();
        let nodes = model.ax.get_mut("PAGE-1").unwrap().as_array_mut().unwrap();
        let field = nodes.iter_mut().find(|n| n["nodeId"] == "8").unwrap();
        field["value"]["value"] = json!("console-set-Secret8");
    }
    // The page's tree cannot be read as the browser is handed back.
    let unreadable = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let failing = Arc::clone(&unreadable);
    let mut earlier = super::super::lock(&engine.hook).take();
    engine.on(move |sent, engine| {
        if sent.method == "Accessibility.getFullAXTree"
            && failing.load(std::sync::atomic::Ordering::SeqCst)
        {
            return Some(Err(crate::cdp::CallError::TimedOut));
        }
        earlier.as_mut().and_then(|hook| hook(sent, engine))
    });
    browser.hand_back();
    let refused = browser.call(&id(2), Call::Pages);
    assert_eq!(
        refused,
        Err(Refusal::NotKeptOutYet),
        "nothing is answered yet"
    );
    // Once it can be read, what DevTools put into its field is kept out.
    unreadable.store(false, std::sync::atomic::Ordering::SeqCst);
    let read = snapshot(&browser, 3);
    assert!(!read.contains("console-set-Secret8"), "{read}");
}

#[test]
fn devtools_that_show_their_page_with_another_query_close() {
    let (browser, engine) = browser();
    let ends = inspected(&browser, &engine);
    let other = format!(
        "{}&remoteBase=https://example.com/",
        super::super::inspect::FRONT_END
    );
    engine.emit(
        "Target.targetInfoChanged",
        json!({"targetInfo":{"targetId":"FRONT-1","type":"page","title":"","url":other,"attached":true,"browserContextId":"CONTEXT-DEVTOOLS"}}),
        None,
    );
    wait_for("DevTools close", || !browser.inspecting());
    assert_eq!(*ends.lock().unwrap(), vec![InspectEnded::Moved]);
}

#[test]
fn a_call_too_large_for_devtools_to_send_is_answered_with_an_error() {
    let (browser, engine) = browser();
    let _ends = inspected(&browser, &engine);
    let huge = "x".repeat(super::super::inspect::MOST_RELAYED_BYTES + 1);
    front_end_sends(
        &engine,
        &json!({"id": 9, "method": "Runtime.evaluate", "params": {"expression": huge}}),
    );
    wait_for("the call is answered", || {
        handed_to_the_front_end(&engine)
            .iter()
            .any(|m| m["id"] == 9)
    });
    let answer = handed_to_the_front_end(&engine)
        .into_iter()
        .find(|m| m["id"] == 9)
        .unwrap();
    assert!(answer.get("error").is_some(), "{answer}");
}

#[test]
fn a_session_made_for_devtools_that_answered_too_late_is_let_go_and_never_the_drivers() {
    let (browser, engine) = browser();
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
        // The page's session for DevTools is not answered in time.
        "Target.attachToTarget" if sent.params["targetId"] == "PAGE-1" => {
            Some(Err(crate::cdp::CallError::TimedOut))
        }
        _ => None,
    });
    browser.take_over();
    assert!(browser.inspect(1000, 600, Box::new(|_| {})).is_err());
    // The engine's word of that session comes after all.
    engine.emit(
        "Target.attachedToTarget",
        json!({"sessionId":"S-LATE","targetInfo":{"targetId":"PAGE-1","type":"page","title":"","url":"http://fixture.test/frames.html","attached":true,"browserContextId":"CONTEXT-1"},"waitingForDebugger":false}),
        None,
    );
    wait_for("the late session is let go", || {
        engine
            .sent("Target.detachFromTarget")
            .iter()
            .any(|s| s.params["sessionId"] == "S-LATE")
    });
    browser.hand_back();
    // The driver still speaks to the page on its own session.
    snapshot(&browser, 2);
    assert!(
        !engine
            .sent("Accessibility.getFullAXTree")
            .iter()
            .any(|s| s.session.as_deref() == Some("S-LATE"))
    );
}

#[test]
fn the_agents_paste_is_withheld_once_devtools_were_open_and_its_other_keys_go() {
    let (browser, engine) = browser();
    let _ends = inspected(&browser, &engine);
    browser.hand_back();
    for chord in ["Control+v", "Meta+v", "Control+Shift+v", "Shift+Insert"] {
        assert_eq!(
            browser.call(
                &id(chord.len() as u32 + 40),
                Call::Press {
                    page: None,
                    keys: vec!["Control+c".to_owned(), chord.to_owned()],
                },
            ),
            Err(Refusal::PasteWithheld),
            "{chord}"
        );
    }
    browser
        .call(
            &id(60),
            Call::Press {
                page: None,
                keys: vec!["Enter".to_owned()],
            },
        )
        .expect("a key that pastes nothing goes");
}

#[test]
fn a_page_never_read_at_the_hand_back_lets_the_agent_leave_it_and_a_second_hand_back_lets_go() {
    let (browser, engine) = browser();
    let _ends = inspected(&browser, &engine);
    let mut earlier = super::super::lock(&engine.hook).take();
    engine.on(move |sent, engine| {
        if sent.method == "Accessibility.getFullAXTree" {
            return Some(Err(crate::cdp::CallError::TimedOut));
        }
        earlier.as_mut().and_then(|hook| hook(sent, engine))
    });
    browser.hand_back();
    let refused = browser.call(&id(2), Call::Pages).unwrap_err();
    assert_eq!(refused, Refusal::NotKeptOutYet);
    let said = refused.to_string();
    assert!(!said.contains("in a moment"), "{said}");
    assert!(
        said.contains("go back") || said.contains("another address"),
        "{said}"
    );
    // Leaving the page is let through: it throws the page's document away.
    let went = browser.call(
        &id(3),
        Call::Navigate {
            page: None,
            address: WebAddress::parse("http://fixture.test/other.html").unwrap(),
        },
    );
    assert_ne!(went, Err(Refusal::NotKeptOutYet));
    // A second hand-back lets go of the hold.
    browser.take_over();
    browser.hand_back();
    assert_ne!(
        browser.call(&id(4), Call::Pages),
        Err(Refusal::NotKeptOutYet)
    );
}

#[test]
fn devtools_reach_no_object_by_its_heap_id_and_never_see_purlis_own_scripts() {
    let (browser, engine) = browser();
    let _ends = inspected(&browser, &engine);
    engine.emit(
        "Runtime.executionContextCreated",
        json!({"context":{"id":9,"origin":"http://fixture.test","name":"purlis","uniqueId":"PURLIS-9","auxData":{"isDefault":false,"type":"isolated","frameId":"PAGE-1"}}}),
        Some("S-INSPECTED"),
    );
    engine.emit(
        "Debugger.scriptParsed",
        json!({"scriptId":"71","url":"","executionContextId":9,"hash":"x"}),
        Some("S-INSPECTED"),
    );
    page_world(&engine);
    engine.emit(
        "Debugger.scriptParsed",
        json!({"scriptId":"72","url":"http://fixture.test/app.js","executionContextId":3,"hash":"y"}),
        Some("S-INSPECTED"),
    );
    front_end_sends(
        &engine,
        &json!({"id": 11, "method": "HeapProfiler.getObjectByHeapObjectId", "params": {"objectId": "1"}}),
    );
    front_end_sends(
        &engine,
        &json!({"id": 12, "method": "Debugger.setBreakpoint", "params": {"location": {"scriptId": "71", "lineNumber": 0}}}),
    );
    front_end_sends(
        &engine,
        &json!({"id": 13, "method": "Debugger.getScriptSource", "params": {"scriptId": "72"}}),
    );
    front_end_sends(
        &engine,
        &json!({"id": 14, "method": "HeapProfiler.addInspectedHeapObject", "params": {"heapObjectId": "1"}}),
    );
    wait_for("all are answered", || {
        let handed = handed_to_the_front_end(&engine);
        [11, 12, 13, 14]
            .iter()
            .all(|n| handed.iter().any(|m| m["id"] == *n))
    });
    let handed = handed_to_the_front_end(&engine);
    let answer = |n: i64| handed.iter().find(|m| m["id"] == n).unwrap().clone();
    assert!(answer(11).get("error").is_some());
    assert!(answer(12).get("error").is_some());
    assert!(answer(13).get("error").is_none());
    assert!(answer(14).get("error").is_some());
    assert!(
        engine
            .sent("HeapProfiler.addInspectedHeapObject")
            .is_empty()
    );
    assert!(
        handed.iter().all(|m| m["params"]["scriptId"] != "71"),
        "{handed:?}"
    );
    assert!(handed.iter().any(|m| m["params"]["scriptId"] == "72"));
}

#[test]
fn devtools_paused_in_purlis_own_script_go_on_at_once_and_are_never_told() {
    let (browser, engine) = browser();
    let _ends = inspected(&browser, &engine);
    engine.emit(
        "Runtime.executionContextCreated",
        json!({"context":{"id":9,"origin":"http://fixture.test","name":"purlis","uniqueId":"PURLIS-9","auxData":{"isDefault":false,"type":"isolated","frameId":"PAGE-1"}}}),
        Some("S-INSPECTED"),
    );
    engine.emit(
        "Debugger.scriptParsed",
        json!({"scriptId":"71","url":"","executionContextId":9,"hash":"x"}),
        Some("S-INSPECTED"),
    );
    engine.emit(
        "Debugger.paused",
        json!({"reason":"exception","callFrames":[{"callFrameId":"f","functionName":"","location":{"scriptId":"71","lineNumber":0},"scopeChain":[],"this":{"type":"undefined"}}]}),
        Some("S-INSPECTED"),
    );
    engine.emit("DOM.documentUpdated", json!({}), Some("S-INSPECTED"));
    wait_for("the page's next word reaches DevTools", || {
        handed_to_the_front_end(&engine)
            .iter()
            .any(|m| m["method"] == "DOM.documentUpdated")
    });
    assert!(
        handed_to_the_front_end(&engine)
            .iter()
            .all(|m| m["method"] != "Debugger.paused")
    );
    assert!(
        engine
            .sent("Debugger.resume")
            .iter()
            .any(|s| s.session.as_deref() == Some("S-INSPECTED"))
    );
}

#[test]
fn devtools_close_where_the_front_end_does_not_answer_how_it_runs() {
    for answer in [
        Err(crate::cdp::CallError::TimedOut),
        Ok(json!({ "result": { "type": "string", "value": "not json" } })),
        Ok(json!({ "result": { "type": "undefined" } })),
    ] {
        let (browser, engine) = browser();
        answers_devtools(&engine);
        let mut earlier = super::super::lock(&engine.hook).take();
        let answer = Mutex::new(Some(answer));
        engine.on(move |sent, engine| {
            if sent.method == "Runtime.evaluate"
                && sent.params["expression"] == super::super::inspect::HOSTED
            {
                return answer.lock().unwrap().take();
            }
            earlier.as_mut().and_then(|hook| hook(sent, engine))
        });
        browser.take_over();
        let ends: Ends = Arc::default();
        let told = Arc::clone(&ends);
        browser
            .inspect(
                1000,
                600,
                Box::new(move |why| told.lock().unwrap().push(why)),
            )
            .unwrap();
        engine.emit(
            "Page.loadEventFired",
            json!({ "timestamp": 1.0 }),
            Some("S-FRONT-1"),
        );
        wait_for("DevTools close", || !browser.inspecting());
        assert_eq!(*ends.lock().unwrap(), vec![InspectEnded::Ended]);
    }
}

/// The page's own main world, id 3, as its session tells DevTools of it.
fn page_world(engine: &super::Engine) {
    engine.emit(
        "Runtime.executionContextCreated",
        json!({"context":{"id":3,"origin":"http://fixture.test","name":"","uniqueId":"PAGE-3","auxData":{"isDefault":true,"type":"default","frameId":"PAGE-1"}}}),
        Some("S-INSPECTED"),
    );
}

/// purlis's own world (id 9) with its script 71, and the page's world (id 3) with its script
/// 72, told in that order.
fn both_worlds(engine: &super::Engine) {
    engine.emit(
        "Runtime.executionContextCreated",
        json!({"context":{"id":9,"origin":"http://fixture.test","name":"purlis","uniqueId":"PURLIS-9","auxData":{"isDefault":false,"type":"isolated","frameId":"PAGE-1"}}}),
        Some("S-INSPECTED"),
    );
    engine.emit(
        "Debugger.scriptParsed",
        json!({"scriptId":"71","url":"","executionContextId":9,"hash":"x"}),
        Some("S-INSPECTED"),
    );
    page_world(engine);
    engine.emit(
        "Debugger.scriptParsed",
        json!({"scriptId":"72","url":"http://fixture.test/app.js","executionContextId":3,"hash":"y"}),
        Some("S-INSPECTED"),
    );
}

/// A frame of a stack, in `script`.
fn frame_in(script: &str) -> Value {
    json!({"functionName":"f","scriptId":script,"url":"","lineNumber":0,"columnNumber":0})
}

/// What DevTools were handed once the page's next word reached them.
fn handed_up_to_the_next_word(engine: &super::Engine) -> Vec<Value> {
    engine.emit("DOM.documentUpdated", json!({}), Some("S-INSPECTED"));
    wait_for("the page's next word reaches DevTools", || {
        handed_to_the_front_end(engine)
            .iter()
            .any(|m| m["method"] == "DOM.documentUpdated")
    });
    handed_to_the_front_end(engine)
}

/// Whether `message` names purlis's script 71 anywhere.
fn names_ours(message: &Value) -> bool {
    message.to_string().contains(r#""scriptId":"71""#)
}

#[test]
fn a_console_message_devtools_are_told_has_no_frame_of_purlis_own_scripts() {
    let (browser, engine) = browser();
    let _ends = inspected(&browser, &engine);
    both_worlds(&engine);
    engine.emit(
        "Runtime.consoleAPICalled",
        json!({"type":"log","args":[{"type":"string","value":"page-said"}],"executionContextId":3,"timestamp":1.0,
               "stackTrace":{"callFrames":[frame_in("71"), frame_in("72")]}}),
        Some("S-INSPECTED"),
    );
    engine.emit(
        "Runtime.consoleAPICalled",
        json!({"type":"log","args":[{"type":"string","value":"purlis-said"}],"executionContextId":9,"timestamp":2.0}),
        Some("S-INSPECTED"),
    );
    engine.emit(
        "Log.entryAdded",
        json!({"entry":{"source":"javascript","level":"error","text":"logged","timestamp":3.0,
               "stackTrace":{"callFrames":[frame_in("71"), frame_in("72")]}}}),
        Some("S-INSPECTED"),
    );
    let handed = handed_up_to_the_next_word(&engine);
    assert!(!handed.iter().any(names_ours), "{handed:?}");
    let console: Vec<&Value> = handed
        .iter()
        .filter(|m| m["method"] == "Runtime.consoleAPICalled")
        .collect();
    assert_eq!(
        console.len(),
        1,
        "a message of purlis's world is never told"
    );
    assert_eq!(
        console[0]["params"]["stackTrace"]["callFrames"],
        json!([frame_in("72")])
    );
    let logged = handed
        .iter()
        .find(|m| m["method"] == "Log.entryAdded")
        .unwrap();
    assert_eq!(
        logged["params"]["entry"]["stackTrace"]["callFrames"],
        json!([frame_in("72")])
    );
}

#[test]
fn an_exception_devtools_are_told_has_no_frame_of_purlis_own_scripts() {
    let (browser, engine) = browser();
    let _ends = inspected(&browser, &engine);
    both_worlds(&engine);
    engine.emit(
        "Runtime.exceptionThrown",
        json!({"timestamp":1.0,"exceptionDetails":{"exceptionId":1,"text":"Uncaught","lineNumber":0,"columnNumber":0,
               "scriptId":"71","url":"","executionContextId":3,
               "stackTrace":{"callFrames":[frame_in("71"), frame_in("72")],
                             "parent":{"description":"await","callFrames":[frame_in("71")]}}}}),
        Some("S-INSPECTED"),
    );
    engine.emit(
        "Runtime.exceptionThrown",
        json!({"timestamp":2.0,"exceptionDetails":{"exceptionId":2,"text":"Uncaught","lineNumber":0,"columnNumber":0,"executionContextId":9}}),
        Some("S-INSPECTED"),
    );
    let handed = handed_up_to_the_next_word(&engine);
    assert!(!handed.iter().any(names_ours), "{handed:?}");
    let thrown: Vec<&Value> = handed
        .iter()
        .filter(|m| m["method"] == "Runtime.exceptionThrown")
        .collect();
    assert_eq!(
        thrown.len(),
        1,
        "an exception of purlis's world is never told"
    );
    let details = &thrown[0]["params"]["exceptionDetails"];
    assert_eq!(details["stackTrace"]["callFrames"], json!([frame_in("72")]));
    assert_eq!(details["stackTrace"]["parent"]["callFrames"], json!([]));
}

#[test]
fn a_pause_devtools_are_told_has_no_frame_of_purlis_own_scripts() {
    let (browser, engine) = browser();
    let _ends = inspected(&browser, &engine);
    both_worlds(&engine);
    engine.emit(
        "Debugger.paused",
        json!({"reason":"other","callFrames":[{"callFrameId":"f","functionName":"","location":{"scriptId":"72","lineNumber":0},"scopeChain":[],"this":{"type":"undefined"}}],
               "asyncStackTrace":{"description":"setTimeout","callFrames":[frame_in("71"), frame_in("72")]}}),
        Some("S-INSPECTED"),
    );
    engine.emit(
        "Debugger.breakpointResolved",
        json!({"breakpointId":"b","location":{"scriptId":"71","lineNumber":0}}),
        Some("S-INSPECTED"),
    );
    let handed = handed_up_to_the_next_word(&engine);
    assert!(!handed.iter().any(names_ours), "{handed:?}");
    let paused = handed
        .iter()
        .find(|m| m["method"] == "Debugger.paused")
        .expect("a pause in the page's own script is told");
    assert_eq!(
        paused["params"]["asyncStackTrace"]["callFrames"],
        json!([frame_in("72")])
    );
    assert!(
        handed
            .iter()
            .all(|m| m["method"] != "Debugger.breakpointResolved")
    );
}

#[test]
fn a_script_told_before_its_world_waits_for_it_and_purlis_own_is_never_shown() {
    let (browser, engine) = browser();
    let _ends = inspected(&browser, &engine);
    // The scripts come before the worlds they run in.
    engine.emit(
        "Debugger.scriptParsed",
        json!({"scriptId":"71","url":"","executionContextId":9,"hash":"x"}),
        Some("S-INSPECTED"),
    );
    engine.emit(
        "Debugger.scriptParsed",
        json!({"scriptId":"72","url":"http://fixture.test/app.js","executionContextId":3,"hash":"y"}),
        Some("S-INSPECTED"),
    );
    // Until its world is told of, a script is not DevTools' to reach.
    front_end_sends(
        &engine,
        &json!({"id": 21, "method": "Debugger.getScriptSource", "params": {"scriptId": "72"}}),
    );
    wait_for("it is answered", || {
        handed_to_the_front_end(&engine)
            .iter()
            .any(|m| m["id"] == 21)
    });
    engine.emit(
        "Runtime.executionContextCreated",
        json!({"context":{"id":9,"origin":"http://fixture.test","name":"purlis","uniqueId":"PURLIS-9","auxData":{"isDefault":false,"type":"isolated","frameId":"PAGE-1"}}}),
        Some("S-INSPECTED"),
    );
    page_world(&engine);
    front_end_sends(
        &engine,
        &json!({"id": 22, "method": "Debugger.getScriptSource", "params": {"scriptId": "71"}}),
    );
    front_end_sends(
        &engine,
        &json!({"id": 23, "method": "Debugger.getScriptSource", "params": {"scriptId": "72"}}),
    );
    wait_for("both are answered", || {
        let handed = handed_to_the_front_end(&engine);
        [22, 23]
            .iter()
            .all(|n| handed.iter().any(|m| m["id"] == *n))
    });
    let handed = handed_up_to_the_next_word(&engine);
    let answer = |n: i64| handed.iter().find(|m| m["id"] == n).unwrap().clone();
    assert!(answer(21).get("error").is_some());
    assert!(answer(22).get("error").is_some());
    assert!(answer(23).get("error").is_none());
    assert!(!handed.iter().any(names_ours), "{handed:?}");
    let world = handed
        .iter()
        .position(|m| {
            m["method"] == "Runtime.executionContextCreated" && m["params"]["context"]["id"] == 3
        })
        .expect("the page's world is told");
    let script = handed
        .iter()
        .position(|m| m["method"] == "Debugger.scriptParsed" && m["params"]["scriptId"] == "72")
        .expect("the page's script is told once its world is");
    assert!(world < script, "{handed:?}");
}

#[test]
fn a_page_never_read_at_the_hand_back_lets_the_agent_go_back_and_its_next_document_lets_go() {
    let (browser, engine) = browser();
    let _ends = inspected(&browser, &engine);
    let mut earlier = super::super::lock(&engine.hook).take();
    engine.on(move |sent, engine| {
        if sent.method == "Accessibility.getFullAXTree" {
            return Some(Err(crate::cdp::CallError::TimedOut));
        }
        earlier.as_mut().and_then(|hook| hook(sent, engine))
    });
    browser.hand_back();
    assert_eq!(
        browser.call(&id(2), Call::Pages),
        Err(Refusal::NotKeptOutYet)
    );
    // Going back is let through: it leaves the page's document.
    assert_ne!(
        browser.call(&id(3), Call::Back { page: None }),
        Err(Refusal::NotKeptOutYet)
    );
    // Until another document loads, the page is held still.
    assert_eq!(
        browser.call(&id(4), Call::Pages),
        Err(Refusal::NotKeptOutYet)
    );
    engine.emit(
        "Page.frameNavigated",
        json!({"frame":{"id":"PAGE-1","loaderId":"LOADER-8","url":"http://fixture.test/previous.html","name":""}}),
        Some("S-PAGE-1"),
    );
    wait_for("the page's next document lets go of it", || {
        browser.call(&id(5), Call::Pages) != Err(Refusal::NotKeptOutYet)
    });
}
