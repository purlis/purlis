// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A login purlis performs (#2016), against the stand-in engine: its frames page plays the
//! login form, its "Secret word" field the password field, and its line "You typed open sesame
//! just now" a page that echoes what was typed, as a show-password toggle does. In the page's
//! own world its scripts hide the form: only a script run in purlis's own world finds it (#2613).

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::json;

use super::super::fill::GUARD_FIELD;
use super::super::login::{
    CLEAR_FIELD, FIELD_HAS_FOCUS, FIELD_LENGTH, FOCUS_FIELD, NAME_FIELD, PASSWORD_FIELD,
    SAYS_IT_IS_A_NAME, origin_of,
};
use super::{Engine, browser_with, id, limits, snapshot};
use crate::WebAddress;
use crate::drive::{Call, Content, Driver, Hold, Limits, Login, Refusal};

/// What the stand-in's frames page holds once purlis filled the password: the field's own tree
/// reads bullets, as Chrome's does for a password field, and the page echoes the value.
const FILLED: &str = "open sesame";
const BULLETS: &str = "•••••••••••";

/// The frames page's address, and its origin.
const ADDRESS: &str = "http://fixture.test/frames.html";
const ORIGIN: &str = "http://fixture.test";

/// Where a site that signs in on another host sends the browser back to (#4995).
const BACK: &str = "http://app.fixture.test/home";

fn quick() -> Limits {
    Limits {
        login_within: Duration::from_millis(600),
        ..limits()
    }
}

/// A holder that binds, and is told nothing.
fn holds() -> Arc<dyn Hold> {
    Arc::new(|| -> Result<(), String> { Ok(()) })
}

fn login_at(origin: &str) -> Login {
    Login {
        address: WebAddress::parse(ADDRESS).unwrap(),
        origin: origin.to_owned(),
        user: None,
        password: FILLED.to_owned().into(),
        only_if_asked: false,
        suggested: false,
        clean_already: true,
        back_to: None,
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
    /// The page asks for no password at all: signed in already.
    AsksNothing,
    /// The page shows a plain text field and no password: a search box, say.
    PlainField,
    /// The engine makes no world of purlis's own in the page.
    NoWorld,
    /// purlis's world in the page ends once the field is found, before it is filled.
    WorldGone,
    /// As purlis clears what it filled, its world's number comes to name the page's own world,
    /// with no word of it yet: as when the page moves to another process.
    WorldReused,
    /// Submitted, the form sends the browser back to the site's own page on another origin, as
    /// a site that signs in on another host does (#4995).
    GoesBack,
    /// The field can't be given the stops that keep its value from a copy or a drag (#5121).
    Unguarded,
    /// Another page opens a page of its own as purlis's opens (#5121).
    PopupAsItOpens,
    /// The login's page lands on another origin, asking for nothing there (#5121).
    LandsElsewhere,
    /// Its sites' cookies are in the browser, with those of others (#5121).
    HoldsCookies,
    /// Those cookies don't go when they're deleted (#5121).
    CookiesStay,
    /// Its service workers can't be cleared (#5121).
    WorkersStay,
    /// Another page shows the login's site once it's cleared (#5272).
    PageComesBack,
    /// A page won't hold still (#5272).
    WontHoldStill,
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
        model.ax.insert("PAGE-2".into(), login.clone());
        // The tab a clean load opens.
        model.ax.insert("PAGE-3".into(), login);
    }
    let submitted = Arc::new(AtomicBool::new(false));
    let seen = Arc::clone(&submitted);
    let typed = Arc::new(AtomicUsize::new(0));
    let reused = AtomicBool::new(false);
    let loads = AtomicUsize::new(0);
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
                if twist == Twist::PopupAsItOpens {
                    engine.emit(
                        "Target.targetCreated",
                        json!({"targetInfo":{"targetId":"PAGE-7","type":"page","title":"A popup","url":ADDRESS,"attached":false,"openerId":"PAGE-1"}}),
                        None,
                    );
                    std::thread::sleep(Duration::from_millis(100));
                }
                let lands = if twist == Twist::LandsElsewhere {
                    "http://elsewhere.test/"
                } else {
                    p["url"].as_str().unwrap_or_default()
                };
                let loader = format!("LOADER-LOGIN-{}", loads.fetch_add(1, Ordering::SeqCst));
                navigated(engine, on, &loader, lands);
                Some(Ok(json!({ "frameId": on.trim_start_matches("S-"), "loaderId": loader })))
            }
            "Network.setCacheDisabled"
                if twist == Twist::PageComesBack && p["cacheDisabled"] == true =>
            {
                navigated(engine, "S-PAGE-1", "LOADER-COMES-BACK", ADDRESS);
                std::thread::sleep(Duration::from_millis(50));
                Some(Ok(json!({})))
            }
            "Page.setWebLifecycleState" if twist == Twist::WontHoldStill => {
                Some(Err(crate::cdp::CallError::Engine {
                    code: -32000,
                    message: "not now".into(),
                }))
            }
            "Storage.getCookies" if matches!(twist, Twist::HoldsCookies | Twist::CookiesStay) => {
                let deleted = engine.sent("Network.deleteCookies").len();
                Some(Ok(if deleted > 0 && twist == Twist::HoldsCookies {
                    json!({ "cookies": [{ "name": "other", "domain": "other.test", "path": "/" }] })
                } else {
                    json!({ "cookies": [
                        { "name": "own", "domain": "fixture.test", "path": "/deep" },
                        { "name": "parent", "domain": ".fixture.test", "path": "/" },
                        { "name": "under", "domain": "under.fixture.test", "path": "/" },
                        { "name": "other", "domain": "other.test", "path": "/" }
                    ] })
                }))
            }
            "Storage.clearDataForOrigin" if twist == Twist::WorkersStay => {
                Some(Err(crate::cdp::CallError::Engine {
                    code: -32000,
                    message: "not now".into(),
                }))
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
            "Runtime.callFunctionOn"
                if p["functionDeclaration"] == GUARD_FIELD && twist == Twist::Unguarded =>
            {
                Some(Err(crate::cdp::CallError::Engine {
                    code: -32000,
                    message: "Cannot find context with specified id".into(),
                }))
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
            "DOM.resolveNode"
                if twist == Twist::WorldReused && !reused.swap(true, Ordering::SeqCst) =>
            {
                engine.worlds_reused_by_the_page();
                None
            }
            "Page.createIsolatedWorld" if twist == Twist::NoWorld => {
                Some(Err(crate::cdp::CallError::Engine {
                    code: -32000,
                    message: "No frame for given id found".into(),
                }))
            }
            "Runtime.callFunctionOn"
                if p["functionDeclaration"] == FOCUS_FIELD && twist == Twist::WorldGone =>
            {
                Some(Err(crate::cdp::CallError::Engine {
                    code: -32000,
                    message: "Cannot find context with specified id".into(),
                }))
            }
            // The page's own world finds no field: the page redefined what finding one uses.
            "Runtime.evaluate"
                if (p["expression"] == PASSWORD_FIELD || p["expression"] == NAME_FIELD)
                    && !matches!(engine.by_unique(&p["uniqueContextId"], sent.session.as_deref()), Ok(Some(_))) =>
            {
                Some(engine.by_unique(&p["uniqueContextId"], sent.session.as_deref()).map(|_| {
                    json!({ "result": { "type": "object", "subtype": "null", "value": null } })
                }))
            }
            "Runtime.evaluate" if p["expression"] == SAYS_IT_IS_A_NAME => Some(Ok(
                json!({ "result": { "type": "object", "subtype": "null", "value": null } }),
            )),
            "Runtime.evaluate"
                if twist == Twist::PlainField && p["expression"] == NAME_FIELD =>
            {
                Some(Ok(
                    json!({ "result": { "type": "object", "subtype": "node", "objectId": "NAME-1" } }),
                ))
            }
            "Runtime.evaluate" if p["expression"] == PASSWORD_FIELD => {
                Some(Ok(if matches!(twist, Twist::AsksNothing | Twist::PlainField | Twist::LandsElsewhere)
                    || (goes && seen.load(Ordering::SeqCst))
                {
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
            "DOM.describeNode" if p["objectId"] == "NAME-1" => {
                Some(Ok(json!({ "node": { "backendNodeId": 22 } })))
            }
            "Input.dispatchKeyEvent" if p["key"] == "Enter" && p["type"] == "keyDown" => {
                seen.store(true, Ordering::SeqCst);
                if twist == Twist::GoesBack {
                    navigated(engine, on, "LOADER-BACK", BACK);
                }
                Some(Ok(json!({})))
            }
            _ => None,
        }
    });
    submitted
}

/// How a login ended, in words fixed in this file: what a login answers may carry what its page
/// showed, so a failing test names only the kind of answer, never its words.
fn ended_as<T>(outcome: &Result<T, Refusal>) -> &'static str {
    match outcome {
        Ok(_) => "it answered",
        Err(Refusal::NotLoggedIn(_)) => "not logged in",
        Err(Refusal::NoFormOnCleanLoad(_)) => "no form on the clean load",
        Err(Refusal::Interrupted) => "cut",
        Err(Refusal::PersonDriving) => "the person drives",
        Err(Refusal::PurlisDriving) => "another login runs",
        Err(Refusal::Ended) => "the browser ended",
        Err(_) => "another refusal",
    }
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
            .log_in(
                &login_at(ORIGIN),
                Arc::new(move || -> Result<(), String> {
                    bound.fetch_add(1, Ordering::SeqCst);
                    *before.lock().unwrap() = Some(inserted(&engine).len());
                    Ok(())
                }),
            )
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

/// A chat chose this page and these keys (#4595): a key it named as the user name is typed into
/// no field that's only text, where a page would show it, send it or keep it.
#[test]
fn a_login_a_chat_chose_fills_no_plain_field() {
    let (browser, engine) = browser_with(quick());
    as_a_login_form_that(&engine, true, Twist::PlainField);
    let login = Login {
        user: Some("named-as-a-user".to_owned().into()),
        suggested: true,
        ..login_at(ORIGIN)
    };
    let ended = browser.log_in(&login, holds()).unwrap();
    match ended.outcome {
        Err(Refusal::NotLoggedIn(why)) => {
            assert!(why.contains("says it's for a user name"), "{why}");
        }
        other => panic!("refused otherwise: {}", ended_as(&other)),
    }
    assert!(inserted(&engine).is_empty());
}

#[test]
fn a_page_on_another_origin_has_nothing_filled_and_the_browser_is_not_bound() {
    let (browser, engine) = browser_with(quick());
    as_a_login_form(&engine, true);
    let bound = Arc::new(AtomicUsize::new(0));
    let ended = {
        let bound = Arc::clone(&bound);
        browser
            .log_in(
                &login_at("https://shop.example.test"),
                Arc::new(move || -> Result<(), String> {
                    bound.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                }),
            )
            .unwrap()
    };
    match ended.outcome {
        Err(Refusal::NotLoggedIn(why)) => assert!(
            why.contains("not on https://shop.example.test, so nothing was filled"),
            "{why}"
        ),
        other => panic!("refused otherwise: {}", ended_as(&other)),
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
        .log_in(
            &login_at(ORIGIN),
            Arc::new(|| -> Result<(), String> { Err("its proxy is gone".to_owned()) }),
        )
        .unwrap();
    match ended.outcome {
        Err(Refusal::NotLoggedIn(why)) => assert!(why.contains("nothing was filled"), "{why}"),
        other => panic!("refused otherwise: {}", ended_as(&other)),
    }
    assert!(inserted(&engine).is_empty());
}

#[test]
fn a_login_that_does_not_go_through_clears_what_it_filled_and_still_keeps_it_out() {
    let (browser, engine) = browser_with(quick());
    as_a_login_form(&engine, false);
    let ended = browser.log_in(&login_at(ORIGIN), holds()).unwrap();
    match ended.outcome {
        Err(Refusal::NotLoggedIn(why)) => {
            assert!(
                why.contains("did not go through: http://fixture.test still asks"),
                "{why}"
            );
            assert!(!why.contains(FILLED), "{why}");
        }
        other => panic!("refused otherwise: {}", ended_as(&other)),
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
    match browser.log_in(&login_at(ORIGIN), holds()) {
        Err(Refusal::PersonDriving) => {}
        other => panic!("refused otherwise: {}", ended_as(&other)),
    }
    assert!(inserted(&engine).is_empty());
    assert_eq!(browser.driver(), Driver::Person);
}

#[test]
fn a_reference_dealt_before_a_login_is_void_after_it() {
    let (browser, engine) = browser_with(quick());
    as_a_login_form(&engine, true);
    let remember = super::ref_of(&snapshot(&browser, 1), "- checkbox \"Remember me\"");
    let ended = browser.log_in(&login_at(ORIGIN), holds()).unwrap();
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
    let ended = browser.log_in(&login_at(ORIGIN), holds()).unwrap();
    match ended.outcome {
        Err(Refusal::NotLoggedIn(why)) => {
            assert!(why.contains("ended this browser"), "{why}");
            assert!(!why.contains(FILLED), "{why}");
        }
        other => panic!("refused otherwise: {}", ended_as(&other)),
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
    let ended = browser.log_in(&login_at(ORIGIN), holds()).unwrap();
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
        std::thread::spawn(move || browser.log_in(&login_at(ORIGIN), holds()).unwrap())
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
    let ended = browser.log_in(&login_at(ORIGIN), holds()).unwrap();
    match ended.outcome {
        Err(Refusal::NotLoggedIn(why)) => assert!(why.contains("ended this browser"), "{why}"),
        other => panic!("refused otherwise: {}", ended_as(&other)),
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

/// A browser opened from kept logins (#2021): where its page asks for no password, purlis
/// fills nothing and says so; where it asks, purlis fills it as for any login.
#[test]
fn a_page_that_asks_for_nothing_is_signed_in_from_kept_logins_and_nothing_is_filled() {
    let (browser, engine) = browser_with(quick());
    as_a_login_form_that(&engine, true, Twist::AsksNothing);
    let login = Login {
        only_if_asked: true,
        clean_already: false,
        suggested: false,
        ..login_at(ORIGIN)
    };
    let ended = browser
        .log_in(
            &login,
            Arc::new(|| -> Result<(), String> {
                panic!("nothing is bound for a fill that never happens")
            }),
        )
        .unwrap();
    let answer = ended.outcome.unwrap();
    let note = answer.note.unwrap_or_default();
    assert!(
        note.contains("asks for no password"),
        "the note says the page asks for no password"
    );
    assert!(inserted(&engine).is_empty(), "nothing filled");
}

#[test]
fn a_page_that_asks_is_filled_though_the_browser_opened_from_kept_logins() {
    let (browser, engine) = browser_with(quick());
    as_a_login_form(&engine, true);
    let login = Login {
        only_if_asked: true,
        clean_already: false,
        suggested: false,
        ..login_at(ORIGIN)
    };
    let ended = browser.log_in(&login, holds()).unwrap();
    assert!(ended.outcome.is_ok());
    assert_eq!(inserted(&engine), vec![FILLED.to_owned()]);
}

/// Every script a login runs on its page runs in purlis's own world (#2613): the fields are
/// found by scripts that name the world, and focused, asked and cleared through objects of it.
#[test]
fn every_script_a_login_runs_on_its_page_runs_in_purlis_s_own_world() {
    let (browser, engine) = browser_with(quick());
    as_a_login_form(&engine, false);
    let ended = browser.log_in(&login_at(ORIGIN), holds()).unwrap();
    assert!(
        matches!(ended.outcome, Err(Refusal::NotLoggedIn(_))),
        "not logged in: {}",
        ended_as(&ended.outcome)
    );
    assert_eq!(inserted(&engine), vec![FILLED.to_owned()]);
    let finds: Vec<_> = engine
        .sent("Runtime.evaluate")
        .into_iter()
        .filter(|sent| {
            sent.params["expression"] == PASSWORD_FIELD || sent.params["expression"] == NAME_FIELD
        })
        .collect();
    assert!(!finds.is_empty());
    for find in &finds {
        assert!(
            find.params["uniqueContextId"].is_string(),
            "a find names purlis's world: {find:?}"
        );
    }
    let run_on: Vec<_> = engine
        .sent("Runtime.callFunctionOn")
        .into_iter()
        .filter(|sent| {
            [FOCUS_FIELD, FIELD_HAS_FOCUS, FIELD_LENGTH, CLEAR_FIELD]
                .iter()
                .any(|f| sent.params["functionDeclaration"] == *f)
        })
        .collect();
    assert!(
        run_on
            .iter()
            .any(|sent| sent.params["functionDeclaration"] == CLEAR_FIELD)
    );
    for sent in &run_on {
        let object = sent.params["objectId"]
            .as_str()
            .or_else(|| sent.params["arguments"][0]["objectId"].as_str())
            .unwrap_or_default();
        assert!(
            object == "PASSWORD-1" || object.starts_with("OBJECT-"),
            "run on an object of purlis's world: {sent:?}"
        );
    }
    for resolved in engine.sent("DOM.resolveNode") {
        assert!(
            resolved.params["executionContextId"].is_i64(),
            "resolved into purlis's world: {resolved:?}"
        );
    }
}

#[test]
fn a_login_with_no_world_of_purlis_s_own_fills_nothing_and_hands_the_browser_back() {
    let (browser, engine) = browser_with(quick());
    as_a_login_form_that(&engine, true, Twist::NoWorld);
    let ended = browser.log_in(&login_at(ORIGIN), holds()).unwrap();
    match ended.outcome {
        Err(Refusal::NotLoggedIn(why)) => assert!(
            why.contains("could not read the login page on http://fixture.test"),
            "{why}"
        ),
        other => panic!("refused otherwise: {}", ended_as(&other)),
    }
    assert!(inserted(&engine).is_empty());
    assert_eq!(ended.driver, Driver::Agent);
    assert_eq!(browser.driver(), Driver::Agent);
    assert!(engine.sent("Browser.close").is_empty());
}

#[test]
fn a_login_whose_world_ends_before_the_fill_fills_nothing_and_hands_the_browser_back() {
    let (browser, engine) = browser_with(quick());
    as_a_login_form_that(&engine, true, Twist::WorldGone);
    let ended = browser.log_in(&login_at(ORIGIN), holds()).unwrap();
    match ended.outcome {
        Err(Refusal::NotLoggedIn(why)) => {
            assert!(why.contains("could not fill the login"), "{why}");
        }
        other => panic!("refused otherwise: {}", ended_as(&other)),
    }
    assert!(inserted(&engine).is_empty());
    assert_eq!(ended.driver, Driver::Agent);
    assert!(engine.sent("Browser.close").is_empty());
}

#[test]
fn a_field_is_cleared_only_in_purlis_s_world_though_its_number_now_names_the_pages() {
    let (browser, engine) = browser_with(quick());
    as_a_login_form_that(&engine, false, Twist::WorldReused);
    let ended = browser.log_in(&login_at(ORIGIN), holds()).unwrap();
    assert!(
        matches!(ended.outcome, Err(Refusal::NotLoggedIn(_))),
        "not logged in: {}",
        ended_as(&ended.outcome)
    );
    let clears: Vec<_> = engine
        .sent("Runtime.callFunctionOn")
        .into_iter()
        .filter(|sent| sent.params["functionDeclaration"] == CLEAR_FIELD)
        .collect();
    assert!(!clears.is_empty(), "the field was cleared");
    for clear in &clears {
        assert!(
            clear.params["uniqueContextId"]
                .as_str()
                .is_some_and(|unique| unique.starts_with("U-")),
            "a clear names purlis's world by its unique id"
        );
        assert!(clear.params["objectId"].is_null());
    }
    // The element the stale number resolved to is the page's own: the engine refuses it in
    // purlis's world, and the world is made again for the clear.
    assert!(
        clears
            .iter()
            .any(|clear| clear.params["arguments"][0]["objectId"]
                .as_str()
                .is_some_and(|object| object.starts_with("OBJECT-"))),
        "cleared in a world of purlis's own"
    );
}

/// #4995: purlis learns where a page sends the browser to sign in, from a page of its own and
/// the engine's record of its document, typing nothing.
#[test]
fn purlis_finds_where_a_page_shows_its_login_form_and_types_nothing() {
    let (browser, engine) = browser_with(quick());
    as_a_login_form(&engine, false);
    let answer = browser
        .call(
            &id(1),
            Call::FindLoginForm {
                address: WebAddress::parse(ADDRESS).unwrap(),
            },
        )
        .unwrap();
    let page = answer.page.expect("a page");
    assert_eq!(origin_of(&page.address).as_deref(), Some(ORIGIN));
    assert!(inserted(&engine).is_empty());
    assert!(engine.sent("Input.dispatchKeyEvent").is_empty());
}

#[test]
fn a_page_that_shows_no_login_form_in_time_is_refused() {
    let (browser, engine) = browser_with(quick());
    as_a_login_form_that(&engine, false, Twist::AsksNothing);
    let refused = browser
        .call(
            &id(1),
            Call::FindLoginForm {
                address: WebAddress::parse(ADDRESS).unwrap(),
            },
        )
        .unwrap_err();
    match refused {
        Refusal::NotLoggedIn(why) => assert!(why.contains("no login form appeared"), "{why}"),
        other => panic!("refused otherwise: {}", ended_as::<()>(&Err(other))),
    }
}

/// #4995: a form on another host sends the browser back to the site's page once the login went
/// through; that is a login that went through, where it names that page's origin alone.
#[test]
fn a_login_whose_form_sends_the_browser_back_to_its_page_went_through() {
    let (browser, engine) = browser_with(quick());
    as_a_login_form_that(&engine, false, Twist::GoesBack);
    let back = Login {
        back_to: Some("http://app.fixture.test".to_owned()),
        ..login_at(ORIGIN)
    };
    let ended = browser.log_in(&back, holds()).unwrap();
    let note = ended
        .outcome
        .expect("went through")
        .note
        .unwrap_or_default();
    assert!(
        note.contains("went back to http://app.fixture.test"),
        "{note}"
    );
    assert_eq!(inserted(&engine), vec![FILLED.to_owned()]);

    let (browser, engine) = browser_with(quick());
    as_a_login_form_that(&engine, false, Twist::GoesBack);
    let ended = browser.log_in(&login_at(ORIGIN), holds()).unwrap();
    assert_eq!(ended_as(&ended.outcome), "not logged in");
}

/// A holder that counts what it was told.
#[derive(Default)]
struct Counted {
    begun: AtomicUsize,
    bound: AtomicUsize,
    back: AtomicUsize,
    /// Refuses the login as it begins.
    refuses: bool,
}

impl Hold for Counted {
    fn begins(&self) -> Result<(), String> {
        self.begun.fetch_add(1, Ordering::SeqCst);
        if self.refuses {
            return Err("its sign-in did not open".to_owned());
        }
        Ok(())
    }

    fn bind(&self) -> Result<(), String> {
        self.bound.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn back(&self) {
        self.back.fetch_add(1, Ordering::SeqCst);
    }
}

/// `count` reaches `want` within a few seconds: events are read on a thread of their own.
fn comes_to(count: &AtomicUsize, want: usize) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(5) {
        if count.load(Ordering::SeqCst) == want {
            return true;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    false
}

/// Page `session`'s own document moves to `url`, as the engine tells it.
fn page_moves(engine: &Engine, session: &str, url: &str) {
    engine.emit(
        "Page.frameNavigated",
        json!({ "frame": {
            "id": session.trim_start_matches("S-"),
            "loaderId": "LOADER-MOVED",
            "url": url,
            "name": "",
        } }),
        Some(session),
    );
}

/// #4995: a login whose form sends its page back to the page's origin has its holder told so,
/// once, however often the page loads a document there after.
#[test]
fn a_login_whose_page_comes_back_to_its_origin_tells_its_holder_once() {
    let (browser, engine) = browser_with(quick());
    as_a_login_form_that(&engine, false, Twist::GoesBack);
    let back = Login {
        back_to: Some("http://app.fixture.test".to_owned()),
        ..login_at(ORIGIN)
    };
    let held = Arc::new(Counted::default());
    let ended = browser
        .log_in(&back, Arc::clone(&held) as Arc<dyn Hold>)
        .unwrap();
    assert!(ended.outcome.is_ok(), "{}", ended_as(&ended.outcome));
    assert_eq!(held.begun.load(Ordering::SeqCst), 1);
    assert_eq!(held.bound.load(Ordering::SeqCst), 1);
    assert!(comes_to(&held.back, 1), "told it came back once");
    page_moves(&engine, "S-PAGE-2", BACK);
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(held.back.load(Ordering::SeqCst), 1, "told once only");
}

/// #4995: a form whose site asks something more once the password went (its page stays on the
/// form's origin) has the holder told only when the login's own page comes back to the page's
/// origin later; another page there tells it nothing.
#[test]
fn a_login_whose_page_comes_back_after_it_answered_tells_its_holder_then() {
    let (browser, engine) = browser_with(quick());
    as_a_login_form(&engine, true);
    let back = Login {
        back_to: Some("http://app.fixture.test".to_owned()),
        ..login_at(ORIGIN)
    };
    let held = Arc::new(Counted::default());
    let ended = browser
        .log_in(&back, Arc::clone(&held) as Arc<dyn Hold>)
        .unwrap();
    assert!(ended.outcome.is_ok(), "{}", ended_as(&ended.outcome));
    assert_eq!(
        held.back.load(Ordering::SeqCst),
        0,
        "still on the form's origin"
    );
    page_moves(&engine, "S-PAGE-1", BACK);
    page_moves(&engine, "S-PAGE-2", "http://fixture.test/next-step");
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(
        held.back.load(Ordering::SeqCst),
        0,
        "not its page, or still on the form's origin"
    );
    page_moves(&engine, "S-PAGE-2", BACK);
    assert!(comes_to(&held.back, 1), "told once its page came back");
}

/// #4995: a login's page that leaves the form's origin for anywhere but its own page's (a chat
/// that sends it elsewhere, a page that can't load) tells its holder as well: the sign-in is
/// done with.
#[test]
fn a_login_whose_page_leaves_its_forms_origin_for_elsewhere_tells_its_holder() {
    let (browser, engine) = browser_with(quick());
    as_a_login_form(&engine, true);
    let back = Login {
        back_to: Some("http://app.fixture.test".to_owned()),
        ..login_at(ORIGIN)
    };
    let held = Arc::new(Counted::default());
    let ended = browser
        .log_in(&back, Arc::clone(&held) as Arc<dyn Hold>)
        .unwrap();
    assert!(ended.outcome.is_ok(), "{}", ended_as(&ended.outcome));
    assert_eq!(held.back.load(Ordering::SeqCst), 0);
    page_moves(&engine, "S-PAGE-2", "chrome-error://chromewebdata/");
    assert!(comes_to(&held.back, 1), "told once its page left");
    page_moves(&engine, "S-PAGE-2", BACK);
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(held.back.load(Ordering::SeqCst), 1, "told once only");
}

/// A login whose form is on its page's own origin waits for no page to come back: it tells
/// nothing.
#[test]
fn a_login_with_no_page_to_come_back_to_tells_its_holder_nothing() {
    let (browser, engine) = browser_with(quick());
    as_a_login_form_that(&engine, false, Twist::GoesBack);
    let held = Arc::new(Counted::default());
    let ended = browser
        .log_in(&login_at(ORIGIN), Arc::clone(&held) as Arc<dyn Hold>)
        .unwrap();
    assert_eq!(ended_as(&ended.outcome), "not logged in");
    page_moves(&engine, "S-PAGE-2", BACK);
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(held.back.load(Ordering::SeqCst), 0);
}

/// #4995: a login whose holder can't let the browser reach the login as it begins opens no page
/// and fills nothing.
#[test]
fn a_login_whose_holder_refuses_it_as_it_begins_opens_nothing_and_fills_nothing() {
    let (browser, engine) = browser_with(quick());
    as_a_login_form(&engine, true);
    let held = Arc::new(Counted {
        refuses: true,
        ..Counted::default()
    });
    let ended = browser
        .log_in(&login_at(ORIGIN), Arc::clone(&held) as Arc<dyn Hold>)
        .unwrap();
    match ended.outcome {
        Err(Refusal::NotLoggedIn(why)) => {
            assert!(why.contains("its sign-in did not open"), "{why}");
            assert!(why.contains("nothing was filled"), "{why}");
        }
        other => panic!("refused otherwise: {}", ended_as(&other)),
    }
    assert_eq!(held.bound.load(Ordering::SeqCst), 0);
    assert!(engine.sent("Page.navigate").is_empty(), "no page opened");
    assert!(inserted(&engine).is_empty());
    assert_eq!(ended.driver, Driver::Agent);
}

/// #5014: reopened from kept logins, a page whose form is on another origin and that sends the
/// browser there by script a moment after it loaded is no page signed in already: purlis waits
/// for it, and fills the form.
#[test]
fn a_kept_login_whose_page_sends_it_to_its_form_late_is_filled_there() {
    let (browser, engine) = browser_with(Limits {
        login_within: Duration::from_secs(4),
        ..limits()
    });
    as_a_login_form(&engine, true);
    let login = Login {
        address: WebAddress::parse(BACK).unwrap(),
        only_if_asked: true,
        clean_already: false,
        back_to: Some("http://app.fixture.test".to_owned()),
        ..login_at(ORIGIN)
    };
    let late = {
        let engine = Arc::clone(&engine);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(500));
            page_moves(&engine, "S-PAGE-2", ADDRESS);
            // And so does the page of the clean load, as it's filled only on its form's origin.
            while went_on(&engine, "S-PAGE-3").is_empty() {
                std::thread::sleep(Duration::from_millis(20));
            }
            std::thread::sleep(Duration::from_millis(500));
            page_moves(&engine, "S-PAGE-3", ADDRESS);
        })
    };
    let ended = browser
        .log_in(&login, Arc::new(Counted::default()) as Arc<dyn Hold>)
        .unwrap();
    late.join().unwrap();
    assert!(ended.outcome.is_ok(), "{}", ended_as(&ended.outcome));
    assert_eq!(inserted(&engine), vec![FILLED.to_owned()]);
}

/// #5121, SF-7: a login's field gives nothing to a copy, a cut or a drag from before its value
/// goes in, as any fill's does.
#[test]
fn a_logins_field_is_guarded_before_its_value_goes_in() {
    let (browser, engine) = browser_with(quick());
    as_a_login_form(&engine, true);
    let ended = browser.log_in(&login_at(ORIGIN), holds()).unwrap();
    assert!(ended.outcome.is_ok(), "{}", ended_as(&ended.outcome));
    let guards: Vec<_> = engine
        .sent("Runtime.callFunctionOn")
        .into_iter()
        .filter(|sent| sent.params["functionDeclaration"] == GUARD_FIELD)
        .collect();
    assert_eq!(guards.len(), 1, "the one field filled");
    assert_eq!(guards[0].params["arguments"][0]["value"], FILLED);
    assert!(
        guards[0].params["objectId"].as_str().is_some(),
        "run on the field, an object of purlis's own world"
    );
}

/// #5121: where the field can't be guarded, its value doesn't go in.
#[test]
fn a_field_that_cannot_be_guarded_is_not_filled() {
    let (browser, engine) = browser_with(quick());
    as_a_login_form_that(&engine, true, Twist::Unguarded);
    let ended = browser.log_in(&login_at(ORIGIN), holds()).unwrap();
    assert!(
        matches!(ended.outcome, Err(Refusal::NotLoggedIn(_))),
        "{}",
        ended_as(&ended.outcome)
    );
    assert!(inserted(&engine).is_empty(), "nothing went in");
}

/// #5121: from a login on, the agent copies and drags nothing in that browser, as after a fill.
#[test]
fn after_a_login_the_agent_copies_nothing_in_that_browser() {
    let (browser, engine) = browser_with(quick());
    as_a_login_form(&engine, true);
    let ended = browser.log_in(&login_at(ORIGIN), holds()).unwrap();
    assert!(ended.outcome.is_ok(), "{}", ended_as(&ended.outcome));
    let copied = browser.call(
        &id(2),
        Call::Press {
            page: None,
            keys: vec!["Control+c".to_owned()],
        },
    );
    assert_eq!(copied.err(), Some(Refusal::CopyWithheld));
}

/// #5121: a login that never reached a fill leaves the agent's copies alone.
#[test]
fn a_login_of_a_page_that_asks_nothing_withholds_no_copy() {
    let (browser, engine) = browser_with(quick());
    as_a_login_form_that(&engine, true, Twist::AsksNothing);
    let login = Login {
        only_if_asked: true,
        clean_already: false,
        ..login_at(ORIGIN)
    };
    let ended = browser.log_in(&login, holds()).unwrap();
    assert!(ended.outcome.is_ok(), "{}", ended_as(&ended.outcome));
    let note = ended.outcome.ok().and_then(|answer| answer.note);
    assert!(
        note.unwrap_or_default().contains("already logged in"),
        "the note says so"
    );
    let copied = browser.call(
        &id(2),
        Call::Press {
            page: None,
            keys: vec!["Control+c".to_owned()],
        },
    );
    assert_ne!(copied.err(), Some(Refusal::CopyWithheld));
}

/// #5121: the page a login fills is the one purlis made, by what the engine said it made, though
/// another tab opens one as it opens, which would have become the browser's current page.
#[test]
fn a_page_another_tab_opens_as_the_login_opens_is_not_the_one_filled() {
    let (browser, engine) = browser_with(quick());
    as_a_login_form_that(&engine, true, Twist::PopupAsItOpens);
    let ended = browser.log_in(&login_at(ORIGIN), holds()).unwrap();
    assert!(ended.outcome.is_ok(), "{}", ended_as(&ended.outcome));
    let typed_on: Vec<_> = engine
        .sent("Input.insertText")
        .iter()
        .map(|sent| sent.session.clone().unwrap_or_default())
        .collect();
    assert_eq!(typed_on, ["S-PAGE-2"]);
}

/// #5121: a page that went elsewhere asks for nothing, and says nothing of a session.
#[test]
fn a_login_page_that_went_elsewhere_is_not_taken_for_one_signed_in() {
    let (browser, engine) = browser_with(quick());
    as_a_login_form_that(&engine, true, Twist::LandsElsewhere);
    let login = Login {
        only_if_asked: true,
        clean_already: false,
        ..login_at(ORIGIN)
    };
    let ended = browser.log_in(&login, holds()).unwrap();
    assert!(
        matches!(ended.outcome, Err(Refusal::NotLoggedIn(_))),
        "{}",
        ended_as(&ended.outcome)
    );
    assert!(inserted(&engine).is_empty(), "nothing filled");
}

/// A login on `fixture.test`, in a browser that has held something.
fn in_a_used_browser() -> Login {
    Login {
        clean_already: false,
        ..login_at(ORIGIN)
    }
}

/// What was sent to the page on `session`, of the addresses it was sent to.
fn went_on(engine: &Engine, session: &str) -> Vec<String> {
    engine
        .sent("Page.navigate")
        .iter()
        .filter(|sent| sent.session.as_deref() == Some(session))
        .map(|sent| sent.params["url"].as_str().unwrap_or_default().to_owned())
        .collect()
}

/// #5272, the person's ruling: every login that fills starts clean on its own site, whoever chose
/// it, with all its sites store gone before anything is filled, and what others store left alone.
#[test]
fn a_login_in_a_used_browser_clears_all_its_site_stores_before_anything_is_filled() {
    for suggested in [false, true] {
        let (browser, engine) = browser_with(quick());
        as_a_login_form_that(&engine, true, Twist::HoldsCookies);
        let login = Login {
            suggested,
            ..in_a_used_browser()
        };
        let ended = browser.log_in(&login, holds()).unwrap();
        assert!(ended.outcome.is_ok(), "{}", ended_as(&ended.outcome));
        let deleted: Vec<_> = engine
            .sent("Network.deleteCookies")
            .iter()
            .map(|sent| sent.params["name"].as_str().unwrap_or_default().to_owned())
            .collect();
        assert_eq!(deleted, ["own", "parent"], "others' are left alone");
        let stores = engine.sent("Storage.clearDataForOrigin");
        assert_eq!(stores.len(), 1);
        assert_eq!(stores[0].params["storageTypes"], "all");
        assert_eq!(stores[0].params["origin"], ORIGIN);
        assert_eq!(inserted(&engine).len(), 1, "and only then filled");
    }
}

/// #5272: what a tab holds for its own life goes with the tab, so the form shows in a new one,
/// past the HTTP cache, and what the used browser would have shown isn't even loaded.
#[test]
fn a_used_browsers_login_loads_its_page_in_a_new_tab_past_the_cache() {
    let (browser, engine) = browser_with(quick());
    as_a_login_form_that(&engine, true, Twist::None);
    let ended = browser.log_in(&in_a_used_browser(), holds()).unwrap();
    assert!(ended.outcome.is_ok(), "{}", ended_as(&ended.outcome));
    assert_eq!(went_on(&engine, "S-PAGE-2"), ["about:blank"]);
    assert_eq!(went_on(&engine, "S-PAGE-3"), [ADDRESS]);
    let closed = engine.sent("Target.closeTarget");
    assert_eq!(closed[0].params["targetId"], "PAGE-2", "{closed:?}");
    let cache: Vec<_> = engine
        .sent("Network.setCacheDisabled")
        .iter()
        .map(|sent| (sent.session.clone(), sent.params["cacheDisabled"].clone()))
        .collect();
    assert_eq!(
        cache,
        [
            (Some("S-PAGE-3".to_owned()), json!(true)),
            (Some("S-PAGE-3".to_owned()), json!(false)),
        ],
        "past the cache for the login alone"
    );
    let all = super::super::lock(&engine.sent).clone();
    let at = |method: &str, session: &str| {
        all.iter()
            .position(|sent| sent.method == method && sent.session.as_deref() == Some(session))
            .unwrap_or_else(|| panic!("{method}"))
    };
    assert!(
        at("Network.setCacheDisabled", "S-PAGE-3") < at("Page.navigate", "S-PAGE-3"),
        "before it loads"
    );
}

#[test]
fn a_cookie_that_will_not_go_fills_nothing() {
    let (browser, engine) = browser_with(quick());
    as_a_login_form_that(&engine, true, Twist::CookiesStay);
    let ended = browser.log_in(&in_a_used_browser(), holds()).unwrap();
    assert!(
        matches!(ended.outcome, Err(Refusal::NotLoggedIn(_))),
        "{}",
        ended_as(&ended.outcome)
    );
    assert!(inserted(&engine).is_empty(), "nothing filled");
}

#[test]
fn a_store_that_will_not_go_fills_nothing() {
    let (browser, engine) = browser_with(quick());
    as_a_login_form_that(&engine, true, Twist::WorkersStay);
    let ended = browser.log_in(&in_a_used_browser(), holds()).unwrap();
    assert!(
        matches!(ended.outcome, Err(Refusal::NotLoggedIn(_))),
        "{}",
        ended_as(&ended.outcome)
    );
    assert!(inserted(&engine).is_empty(), "nothing filled");
}

/// #5272: no other page runs from before the site is cleared until the login has answered.
#[test]
fn every_other_page_is_held_still_while_a_login_clears_and_fills() {
    let (browser, engine) = browser_with(quick());
    as_a_login_form_that(&engine, true, Twist::None);
    // Its pages are told of as the agent's calls find them.
    let _ = snapshot(&browser, 1);
    let ended = browser.log_in(&in_a_used_browser(), holds()).unwrap();
    assert!(ended.outcome.is_ok(), "{}", ended_as(&ended.outcome));
    let all = super::super::lock(&engine.sent).clone();
    let at = |method: &str, state: Option<&str>| {
        all.iter()
            .position(|sent| {
                sent.method == method
                    && state.is_none_or(|state| sent.params["state"] == state)
                    && (state.is_none() || sent.session.as_deref() == Some("S-PAGE-1"))
            })
            .unwrap_or_else(|| panic!("{method} {state:?}"))
    };
    let frozen = at("Page.setWebLifecycleState", Some("frozen"));
    let active = at("Page.setWebLifecycleState", Some("active"));
    assert!(frozen < at("Storage.clearDataForOrigin", None));
    assert!(at("Input.insertText", None) < active, "until it answered");
    assert!(
        !all.iter()
            .any(|sent| sent.method == "Page.setWebLifecycleState"
                && sent.session.as_deref() == Some("S-PAGE-3")),
        "never the login's own"
    );
}

#[test]
fn a_page_that_will_not_hold_still_fills_nothing() {
    let (browser, engine) = browser_with(quick());
    as_a_login_form_that(&engine, true, Twist::WontHoldStill);
    let _ = snapshot(&browser, 1);
    let ended = browser.log_in(&in_a_used_browser(), holds()).unwrap();
    assert!(
        matches!(ended.outcome, Err(Refusal::NotLoggedIn(_))),
        "{}",
        ended_as(&ended.outcome)
    );
    assert!(engine.sent("Storage.clearDataForOrigin").is_empty());
    assert!(inserted(&engine).is_empty(), "nothing filled");
}

#[test]
fn a_page_of_its_site_that_comes_back_after_the_clearing_fills_nothing() {
    let (browser, engine) = browser_with(quick());
    as_a_login_form_that(&engine, true, Twist::PageComesBack);
    let _ = snapshot(&browser, 1);
    let ended = browser.log_in(&in_a_used_browser(), holds()).unwrap();
    assert!(
        matches!(ended.outcome, Err(Refusal::NotLoggedIn(_))),
        "{}",
        ended_as(&ended.outcome)
    );
    assert!(inserted(&engine).is_empty(), "nothing filled");
}

/// #5272: a held login clears only for a form it sees: a page that went elsewhere says nothing of
/// its session, and it's kept.
#[test]
fn a_held_login_whose_page_went_elsewhere_clears_nothing() {
    let (browser, engine) = browser_with(quick());
    as_a_login_form_that(&engine, true, Twist::LandsElsewhere);
    let login = Login {
        only_if_asked: true,
        clean_already: false,
        ..login_at(ORIGIN)
    };
    let ended = browser.log_in(&login, holds()).unwrap();
    assert!(
        matches!(ended.outcome, Err(Refusal::NotLoggedIn(_))),
        "{}",
        ended_as(&ended.outcome)
    );
    assert!(engine.sent("Storage.clearDataForOrigin").is_empty());
    assert!(engine.sent("Network.deleteCookies").is_empty());
}

/// #4995: a form on another origin tells its holder from the page the clean load opened.
#[test]
fn a_used_browsers_login_whose_page_comes_back_tells_its_holder_from_the_new_tab() {
    let (browser, engine) = browser_with(quick());
    as_a_login_form_that(&engine, true, Twist::GoesBack);
    let holder = Arc::new(Counted::default());
    let login = Login {
        back_to: Some("http://app.fixture.test".to_owned()),
        ..in_a_used_browser()
    };
    let ended = browser
        .log_in(&login, Arc::clone(&holder) as Arc<dyn Hold>)
        .unwrap();
    assert!(ended.outcome.is_ok(), "{}", ended_as(&ended.outcome));
    assert!(comes_to(&holder.back, 1), "told once it's back");
}

/// #5272: its page's first load is as clean as a load can be, and is the one a login page that
/// needs another site can load on.
#[test]
fn a_browser_that_has_held_nothing_clears_nothing() {
    let (browser, engine) = browser_with(quick());
    as_a_login_form_that(&engine, true, Twist::HoldsCookies);
    let ended = browser.log_in(&login_at(ORIGIN), holds()).unwrap();
    assert!(ended.outcome.is_ok(), "{}", ended_as(&ended.outcome));
    assert!(engine.sent("Network.deleteCookies").is_empty());
    assert!(engine.sent("Storage.clearDataForOrigin").is_empty());
    assert_eq!(went_on(&engine, "S-PAGE-2"), [ADDRESS]);
    assert!(engine.sent("Target.closeTarget").is_empty());
}

/// #5272: a login the browser holds answers "already logged in", and clears nothing.
#[test]
fn a_held_login_whose_page_asks_for_nothing_clears_nothing() {
    let (browser, engine) = browser_with(quick());
    as_a_login_form_that(&engine, true, Twist::AsksNothing);
    let login = Login {
        only_if_asked: true,
        clean_already: false,
        ..in_a_used_browser()
    };
    let ended = browser.log_in(&login, holds()).unwrap();
    let answer = ended.outcome.unwrap();
    assert!(answer.note.unwrap().contains("already logged in"));
    assert!(engine.sent("Network.deleteCookies").is_empty());
    assert!(engine.sent("Storage.clearDataForOrigin").is_empty());
    assert!(inserted(&engine).is_empty());
}

/// #5272: a held login whose page asks fills, so it starts clean too.
#[test]
fn a_held_login_whose_page_asks_starts_clean() {
    let (browser, engine) = browser_with(quick());
    as_a_login_form_that(&engine, true, Twist::None);
    let login = Login {
        only_if_asked: true,
        clean_already: false,
        ..in_a_used_browser()
    };
    let ended = browser.log_in(&login, holds()).unwrap();
    assert!(ended.outcome.is_ok(), "{}", ended_as(&ended.outcome));
    assert_eq!(engine.sent("Storage.clearDataForOrigin").len(), 1);
    assert_eq!(went_on(&engine, "S-PAGE-2"), [ADDRESS, "about:blank"]);
    assert_eq!(went_on(&engine, "S-PAGE-3"), [ADDRESS]);
}

/// #5272: a form that doesn't appear on the clean load is said by its own kind, for the caller to
/// open a new browser on, and nothing else happens.
#[test]
fn a_form_that_does_not_appear_on_the_clean_load_is_said_by_its_own_kind() {
    let (browser, engine) = browser_with(quick());
    as_a_login_form_that(&engine, true, Twist::AsksNothing);
    let ended = browser.log_in(&in_a_used_browser(), holds()).unwrap();
    assert!(
        matches!(ended.outcome, Err(Refusal::NoFormOnCleanLoad(_))),
        "{}",
        ended_as(&ended.outcome)
    );
    assert!(inserted(&engine).is_empty(), "nothing filled");
    let cache = engine.sent("Network.setCacheDisabled");
    assert_eq!(cache.last().unwrap().params["cacheDisabled"], false);
}

/// #5272: one that has held nothing has nowhere cleaner to go, so it's said as any other.
#[test]
fn a_form_that_does_not_appear_in_a_browser_that_has_held_nothing_is_a_plain_failure() {
    let (browser, engine) = browser_with(quick());
    as_a_login_form_that(&engine, true, Twist::AsksNothing);
    let ended = browser.log_in(&login_at(ORIGIN), holds()).unwrap();
    assert!(
        matches!(ended.outcome, Err(Refusal::NotLoggedIn(_))),
        "{}",
        ended_as(&ended.outcome)
    );
}

#[test]
fn a_cookie_reaches_its_host_and_every_host_under_a_parent_domain_alone() {
    use super::super::login::reaches;
    assert!(reaches("app.example.test", "app.example.test"));
    assert!(reaches(".example.test", "app.example.test"), "a parent's");
    assert!(reaches(".app.example.test", "app.example.test"), "its own");
    assert!(reaches(".EXAMPLE.test", "app.example.test"), "in any case");
    assert!(
        !reaches("example.test", "app.example.test"),
        "a bare parent"
    );
    assert!(!reaches("other.example.test", "app.example.test"), "beside");
    assert!(
        !reaches("sub.app.example.test", "app.example.test"),
        "under"
    );
    assert!(!reaches(".notexample.test", "app.example.test"), "alike");
    assert!(!reaches(".ample.test", "app.example.test"), "alike");
}

/// #5121: a form on another host counts: its page's site is cleared with it.
#[test]
fn a_login_with_its_form_elsewhere_clears_both_sites() {
    let (browser, engine) = browser_with(quick());
    as_a_login_form_that(&engine, true, Twist::None);
    let login = Login {
        back_to: Some("https://app.example.test".to_owned()),
        ..in_a_used_browser()
    };
    let ended = browser.log_in(&login, holds()).unwrap();
    assert!(ended.outcome.is_ok(), "{}", ended_as(&ended.outcome));
    let origins: Vec<_> = engine
        .sent("Storage.clearDataForOrigin")
        .iter()
        .map(|sent| {
            sent.params["origin"]
                .as_str()
                .unwrap_or_default()
                .to_owned()
        })
        .collect();
    assert_eq!(origins, [ORIGIN, "https://app.example.test"]);
}

#[test]
fn a_host_is_read_from_an_origin_as_a_cookie_spells_it() {
    use super::super::login::host_of;
    assert_eq!(
        host_of("https://app.example.test"),
        Some("app.example.test")
    );
    assert_eq!(host_of("http://localhost:8080"), Some("localhost"));
    assert_eq!(host_of("no-scheme"), None);
    assert_eq!(host_of("https://"), None);
}
