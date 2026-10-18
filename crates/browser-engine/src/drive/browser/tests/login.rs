// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A login purlis performs (#2016), against the stand-in engine: its frames page plays the
//! login form, its "Secret word" field the password field, and its line "You typed open sesame
//! just now" a page that echoes what was typed, as a show-password toggle does.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::json;

use super::super::login::{
    CLEAR_FIELD, FIELD_HAS_FOCUS, FIELD_LENGTH, NAME_FIELD, PASSWORD_FIELD, origin_of,
};
use super::{Engine, browser_with, id, limits, snapshot};
use crate::WebAddress;
use crate::drive::{Call, Content, Driver, Limits, Login, Refusal};

/// What the stand-in's frames page holds once purlis filled the password: the field's own tree
/// reads bullets, as Chrome's does for a password field, and the page echoes the value.
const FILLED: &str = "open sesame";
const BULLETS: &str = "•••••••••••";

/// The frames page's address, and its origin.
const ADDRESS: &str = "http://fixture.test/frames.html";
const ORIGIN: &str = "http://fixture.test";

fn quick() -> Limits {
    Limits {
        login_within: Duration::from_millis(600),
        ..limits()
    }
}

fn login_at(origin: &str) -> Login {
    Login {
        address: WebAddress::parse(ADDRESS).unwrap(),
        origin: origin.to_owned(),
        user: None,
        password: FILLED.to_owned().into(),
    }
}

/// The frames page as a login form: the "Secret word" field (backend 21) is the password
/// field, and reads bullets. `submitted` is set by an Enter, and with `goes`, the field goes.
/// The page purlis opens for the login (`PAGE-2`) shows it once it is sent to its address.
fn as_a_login_form(engine: &Arc<Engine>, goes: bool) -> Arc<AtomicBool> {
    as_a_login_form_that(engine, goes, Twist::None)
}

/// What a login form does to purlis's fill, in a test that says so.
#[derive(Clone, Copy, PartialEq)]
enum Twist {
    None,
    /// The page loads another document as purlis types.
    Moves,
    /// The field keeps less than was typed into it.
    Shortens,
    /// The field cannot be asked whether it has the focus.
    Unread,
}

/// [`as_a_login_form`], with `twist`.
fn as_a_login_form_that(engine: &Arc<Engine>, goes: bool, twist: Twist) -> Arc<AtomicBool> {
    {
        let mut model = super::super::lock(&engine.model);
        let nodes = model.ax.get_mut("PAGE-1").unwrap().as_array_mut().unwrap();
        for node in nodes.iter_mut() {
            match node["nodeId"].as_str() {
                Some("12") => node["value"] = json!({ "type": "string", "value": BULLETS }),
                Some("13") => node["name"] = json!({ "type": "computedString", "value": BULLETS }),
                _ => {}
            }
        }
        let login = model.ax["PAGE-1"].clone();
        model.ax.insert("PAGE-2".into(), login);
    }
    let submitted = Arc::new(AtomicBool::new(false));
    let seen = Arc::clone(&submitted);
    let typed = Arc::new(AtomicUsize::new(0));
    let navigated = |engine: &Engine, session: &str, loader: &str, url: &str| {
        let frame = session.trim_start_matches("S-");
        engine.emit(
            "Page.frameNavigated",
            json!({ "frame": { "id": frame, "loaderId": loader, "url": url, "name": "" } }),
            Some(session),
        );
    };
    engine.on(move |sent, engine| {
        let p = &sent.params;
        let on = sent.session.as_deref().unwrap_or_default();
        match sent.method.as_str() {
            "Page.navigate" => {
                navigated(engine, on, "LOADER-LOGIN", p["url"].as_str().unwrap_or_default());
                Some(Ok(json!({ "frameId": on.trim_start_matches("S-"), "loaderId": "LOADER-LOGIN" })))
            }
            "Input.insertText" => {
                let text = p["text"].as_str().unwrap_or_default();
                typed.store(text.encode_utf16().count(), Ordering::SeqCst);
                if twist == Twist::Moves {
                    navigated(engine, on, "LOADER-ELSEWHERE", ADDRESS);
                    std::thread::sleep(Duration::from_millis(50));
                }
                Some(Ok(json!({})))
            }
            "Runtime.callFunctionOn" if p["functionDeclaration"] == FIELD_HAS_FOCUS => {
                Some(if twist == Twist::Unread {
                    Err(crate::cdp::CallError::Engine {
                        code: -32000,
                        message: "Cannot find context with specified id".into(),
                    })
                } else {
                    Ok(json!({ "result": { "type": "boolean", "value": true } }))
                })
            }
            "Runtime.callFunctionOn" if p["functionDeclaration"] == FIELD_LENGTH => {
                let held = typed.load(Ordering::SeqCst);
                let held = if twist == Twist::Shortens { held - 1 } else { held };
                Some(Ok(json!({ "result": { "type": "number", "value": held } })))
            }
            "Runtime.evaluate" if p["expression"] == PASSWORD_FIELD => {
                Some(Ok(if goes && seen.load(Ordering::SeqCst) {
                    json!({ "result": { "type": "object", "subtype": "null", "value": null } })
                } else {
                    json!({ "result": { "type": "object", "subtype": "node", "objectId": "PASSWORD-1" } })
                }))
            }
            "Runtime.evaluate" if p["expression"] == NAME_FIELD => Some(Ok(
                json!({ "result": { "type": "object", "subtype": "null", "value": null } }),
            )),
            "DOM.describeNode" if p["objectId"] == "PASSWORD-1" => {
                Some(Ok(json!({ "node": { "backendNodeId": 21 } })))
            }
            "Input.dispatchKeyEvent" if p["key"] == "Enter" && p["type"] == "keyDown" => {
                seen.store(true, Ordering::SeqCst);
                Some(Ok(json!({})))
            }
            _ => None,
        }
    });
    submitted
}

fn inserted(engine: &Engine) -> Vec<String> {
    engine
        .sent("Input.insertText")
        .iter()
        .map(|sent| sent.params["text"].as_str().unwrap_or_default().to_owned())
        .collect()
}

#[test]
fn a_login_fills_the_declared_origin_and_its_value_is_in_no_later_answer() {
    let (browser, engine) = browser_with(quick());
    as_a_login_form(&engine, true);
    let bound = Arc::new(AtomicUsize::new(0));
    let filled_before_bound = Arc::new(Mutex::new(None));
    let ended = {
        let (bound, before, engine) = (
            Arc::clone(&bound),
            Arc::clone(&filled_before_bound),
            Arc::clone(&engine),
        );
        browser
            .log_in(&login_at(ORIGIN), &move || {
                bound.fetch_add(1, Ordering::SeqCst);
                *before.lock().unwrap() = Some(inserted(&engine).len());
                Ok(())
            })
            .unwrap()
    };
    let answer = ended.outcome.unwrap();
    assert_eq!(
        answer.note.as_deref(),
        Some(
            "the password field is gone from http://fixture.test since purlis submitted the login"
        )
    );
    assert_eq!(ended.driver, Driver::Agent);
    assert_eq!(browser.driver(), Driver::Agent);
    assert_eq!(browser.epoch(), ended.epoch);
    // The declared address, then the value, once, into the field; the reach held to the origin
    // before any value went in.
    assert!(
        engine
            .sent("Page.navigate")
            .iter()
            .any(|sent| sent.params["url"] == ADDRESS)
    );
    assert_eq!(inserted(&engine), vec![FILLED.to_owned()]);
    assert_eq!(bound.load(Ordering::SeqCst), 1);
    assert_eq!(*filled_before_bound.lock().unwrap(), Some(0));
    // Neither the field nor the page's echo of the value shows it to the agent: the value
    // itself is kept out, not the bullets the field's tree reads.
    let text = snapshot(&browser, 1);
    assert!(!text.to_lowercase().contains(FILLED), "{text}");
    let found = browser
        .call(
            &id(2),
            Call::Find {
                page: None,
                text: Some("sesame".into()),
                role: None,
                within: None,
            },
        )
        .unwrap();
    match found.content {
        Content::Text(text) => assert!(!text.to_lowercase().contains(FILLED), "{text}"),
        other => panic!("a find is text: {other:?}"),
    }
}

#[test]
fn a_page_on_another_origin_has_nothing_filled_and_the_browser_is_not_bound() {
    let (browser, engine) = browser_with(quick());
    as_a_login_form(&engine, true);
    let bound = Arc::new(AtomicUsize::new(0));
    let ended = {
        let bound = Arc::clone(&bound);
        browser
            .log_in(&login_at("https://shop.example.test"), &move || {
                bound.fetch_add(1, Ordering::SeqCst);
                Ok(())
            })
            .unwrap()
    };
    match ended.outcome {
        Err(Refusal::NotLoggedIn(why)) => assert!(
            why.contains("not on https://shop.example.test, so nothing was filled"),
            "{why}"
        ),
        other => panic!("refused: {other:?}"),
    }
    assert!(inserted(&engine).is_empty());
    assert_eq!(bound.load(Ordering::SeqCst), 0);
    assert_eq!(browser.driver(), Driver::Agent);
}

#[test]
fn a_browser_that_cannot_be_held_to_the_origin_has_nothing_filled() {
    let (browser, engine) = browser_with(quick());
    as_a_login_form(&engine, true);
    let ended = browser
        .log_in(&login_at(ORIGIN), &|| Err("its proxy is gone".to_owned()))
        .unwrap();
    match ended.outcome {
        Err(Refusal::NotLoggedIn(why)) => assert!(why.contains("nothing was filled"), "{why}"),
        other => panic!("refused: {other:?}"),
    }
    assert!(inserted(&engine).is_empty());
}

#[test]
fn a_login_that_does_not_go_through_clears_what_it_filled_and_still_keeps_it_out() {
    let (browser, engine) = browser_with(quick());
    as_a_login_form(&engine, false);
    let ended = browser.log_in(&login_at(ORIGIN), &|| Ok(())).unwrap();
    match ended.outcome {
        Err(Refusal::NotLoggedIn(why)) => {
            assert!(
                why.contains("did not go through: http://fixture.test still asks"),
                "{why}"
            );
            assert!(!why.contains(FILLED), "{why}");
        }
        other => panic!("refused: {other:?}"),
    }
    let cleared = engine
        .sent("Runtime.callFunctionOn")
        .iter()
        .filter(|sent| sent.params["functionDeclaration"] == CLEAR_FIELD)
        .count();
    assert_eq!(cleared, 1);
    assert!(
        engine
            .sent("DOM.resolveNode")
            .iter()
            .any(|sent| sent.params["backendNodeId"] == 21)
    );
    assert_eq!(browser.driver(), Driver::Agent);
    let text = snapshot(&browser, 1);
    assert!(!text.to_lowercase().contains(FILLED), "{text}");
}

#[test]
fn a_login_is_refused_while_the_person_drives_and_fills_nothing() {
    let (browser, engine) = browser_with(quick());
    as_a_login_form(&engine, true);
    browser.take_over();
    match browser.log_in(&login_at(ORIGIN), &|| Ok(())) {
        Err(Refusal::PersonDriving) => {}
        other => panic!("refused: {other:?}"),
    }
    assert!(inserted(&engine).is_empty());
    assert_eq!(browser.driver(), Driver::Person);
}

#[test]
fn a_reference_dealt_before_a_login_is_void_after_it() {
    let (browser, engine) = browser_with(quick());
    as_a_login_form(&engine, true);
    let remember = super::ref_of(&snapshot(&browser, 1), "- checkbox \"Remember me\"");
    let ended = browser.log_in(&login_at(ORIGIN), &|| Ok(())).unwrap();
    assert!(ended.outcome.is_ok());
    let clicked = browser.call(&id(2), Call::Click { target: remember });
    assert!(
        matches!(clicked, Err(Refusal::HandedBack(_))),
        "{clicked:?}"
    );
}

#[test]
fn a_page_that_loads_another_document_as_purlis_fills_it_ends_the_browser() {
    let (browser, engine) = browser_with(quick());
    as_a_login_form_that(&engine, true, Twist::Moves);
    let ended = browser.log_in(&login_at(ORIGIN), &|| Ok(())).unwrap();
    match ended.outcome {
        Err(Refusal::NotLoggedIn(why)) => {
            assert!(why.contains("ended this browser"), "{why}");
            assert!(!why.contains(FILLED), "{why}");
        }
        other => panic!("refused: {other:?}"),
    }
    assert_eq!(engine.sent("Browser.close").len(), 1);
    let after = browser.call(
        &id(9),
        Call::Snapshot {
            page: None,
            within: None,
            from: 1,
            lines: None,
        },
    );
    assert!(matches!(after, Err(Refusal::Ended)), "{after:?}");
}

#[test]
fn a_login_opens_a_page_of_its_own_and_leaves_the_agents_page_alone() {
    let (browser, engine) = browser_with(quick());
    as_a_login_form(&engine, true);
    let ended = browser.log_in(&login_at(ORIGIN), &|| Ok(())).unwrap();
    assert!(ended.outcome.is_ok());
    assert!(
        engine
            .sent("Page.navigate")
            .iter()
            .all(|sent| sent.session.as_deref() != Some("S-PAGE-1")),
        "the agent's page is never sent anywhere"
    );
    assert_eq!(engine.sent("Target.createTarget").len(), 1);
}

#[test]
fn a_login_the_person_cut_is_cleared_as_they_hand_the_browser_back() {
    let (browser, engine) = browser_with(quick());
    as_a_login_form(&engine, false);
    let browser = Arc::new(browser);
    let running = {
        let browser = Arc::clone(&browser);
        std::thread::spawn(move || browser.log_in(&login_at(ORIGIN), &|| Ok(())).unwrap())
    };
    // Once the password is in, the person takes the browser over.
    let start = std::time::Instant::now();
    while inserted(&engine).is_empty() {
        assert!(start.elapsed() < Duration::from_secs(5), "never filled");
        std::thread::sleep(Duration::from_millis(5));
    }
    browser.take_over();
    let ended = running.join().unwrap();
    assert_eq!(ended.driver, Driver::Person);
    assert!(matches!(ended.outcome, Err(Refusal::Interrupted)));
    let cleared = || {
        engine
            .sent("Runtime.callFunctionOn")
            .iter()
            .filter(|sent| sent.params["functionDeclaration"] == CLEAR_FIELD)
            .count()
    };
    assert_eq!(cleared(), 0, "nothing is cleared while the person drives");
    browser.hand_back();
    assert_eq!(cleared(), 1);
}

#[test]
fn an_address_is_on_the_origin_a_page_spells() {
    assert_eq!(
        origin_of("https://Shop.Example.test/login?next=/").as_deref(),
        Some("https://shop.example.test")
    );
    assert_eq!(
        origin_of("https://shop.example.test:443/").as_deref(),
        Some("https://shop.example.test")
    );
    assert_eq!(
        origin_of("http://127.0.0.1:8123/login").as_deref(),
        Some("http://127.0.0.1:8123")
    );
    assert_eq!(
        origin_of("http://[::1]:8123/").as_deref(),
        Some("http://[::1]:8123")
    );
    assert_eq!(origin_of("about:blank"), None);
    assert_eq!(origin_of("chrome-error://chromewebdata/"), None);
    assert_eq!(origin_of("https://user@shop.example.test/"), None);
}

/// A login whose fill could not be told to have gone in whole ends the browser.
fn ends_the_browser(twist: Twist) {
    let (browser, engine) = browser_with(quick());
    as_a_login_form_that(&engine, true, twist);
    let ended = browser.log_in(&login_at(ORIGIN), &|| Ok(())).unwrap();
    match ended.outcome {
        Err(Refusal::NotLoggedIn(why)) => assert!(why.contains("ended this browser"), "{why}"),
        other => panic!("refused: {other:?}"),
    }
    assert_eq!(engine.sent("Browser.close").len(), 1);
    let after = browser.call(
        &id(9),
        Call::Snapshot {
            page: None,
            within: None,
            from: 1,
            lines: None,
        },
    );
    assert!(matches!(after, Err(Refusal::Ended)), "{after:?}");
}

#[test]
fn a_field_that_keeps_less_than_purlis_typed_ends_the_browser() {
    ends_the_browser(Twist::Shortens);
}

#[test]
fn a_field_that_cannot_be_asked_whether_it_has_the_focus_ends_the_browser() {
    ends_the_browser(Twist::Unread);
}
