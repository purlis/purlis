// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A secret purlis fills (#5115), against the stand-in engine: its frames page is on
//! `http://fixture.test` with its "Secret word" field, and its "payment" frame on
//! `http://other.test`. The engine judges the field's own frame and its kind at the moment of
//! filling, and the value is kept out of every later answer.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use serde_json::json;

use super::super::fill::FIELD_KIND;
use super::super::login::{FIELD_HAS_FOCUS, FIELD_LENGTH};
use super::{Engine, browser, id, ref_of, snapshot};
use crate::drive::{Call, Content, Ref, Refusal, SecretFill};

const VALUE: &str = "open sesame";

/// What the field does as purlis fills it, in a test that says so.
#[derive(Clone, Copy, PartialEq)]
enum Twist {
    None,
    /// The page loads another document as purlis types.
    Moves,
}

/// The stand-in's fields, each read as `kind`, and what they hold once typed into.
fn as_fields(engine: &Arc<Engine>, kind: &'static str, twist: Twist) {
    let typed = Arc::new(AtomicUsize::new(0));
    engine.on(move |sent, engine| {
        let p = &sent.params;
        match sent.method.as_str() {
            "Runtime.callFunctionOn" if p["functionDeclaration"] == FIELD_KIND => Some(Ok(
                json!({ "result": { "type": "object", "value": { "kind": kind, "label": "Secret word" } } }),
            )),
            "Runtime.callFunctionOn" if p["functionDeclaration"] == FIELD_HAS_FOCUS => {
                Some(Ok(json!({ "result": { "type": "boolean", "value": true } })))
            }
            "Runtime.callFunctionOn" if p["functionDeclaration"] == FIELD_LENGTH => {
                let held = typed.load(Ordering::SeqCst);
                Some(Ok(json!({ "result": { "type": "number", "value": held } })))
            }
            "Input.insertText" => {
                let text = p["text"].as_str().unwrap_or_default();
                typed.store(text.encode_utf16().count(), Ordering::SeqCst);
                if twist == Twist::Moves {
                    engine.emit(
                        "Page.frameNavigated",
                        json!({ "frame": { "id": "PAGE-1", "loaderId": "LOADER-ELSEWHERE",
                                           "url": "http://fixture.test/frames.html", "name": "" } }),
                        Some("S-PAGE-1"),
                    );
                    std::thread::sleep(Duration::from_millis(50));
                }
                Some(Ok(json!({})))
            }
            _ => None,
        }
    });
}

fn fill_of(target: Ref, origin: &str) -> SecretFill {
    SecretFill {
        target,
        origin: origin.to_owned(),
        key: "dash-api-key".to_owned(),
        value: VALUE.to_owned().into(),
    }
}

fn inserted(engine: &Engine) -> usize {
    engine.sent("Input.insertText").len()
}

/// How a fill ended, in words fixed in this file.
fn ended_as<T>(outcome: &Result<T, Refusal>) -> &'static str {
    match outcome {
        Ok(_) => "it filled",
        Err(Refusal::NotFilled(_)) => "not filled",
        Err(Refusal::Ended) => "the browser ended",
        Err(_) => "another refusal",
    }
}

#[test]
fn a_one_line_field_on_the_approved_origin_is_filled_and_its_value_kept_out() {
    let (browser, engine) = browser();
    as_fields(&engine, "one_line", Twist::None);
    let secret = ref_of(&snapshot(&browser, 1), "- textbox \"Secret word\"");

    let answer = browser
        .fill_secret(browser.epoch(), &fill_of(secret, "http://fixture.test"))
        .unwrap_or_else(|why| panic!("{}", ended_as(&Err::<(), _>(why))));

    assert!(answer.note.unwrap_or_default().contains("dash-api-key"));
    let typed: Vec<String> = engine
        .sent("Input.insertText")
        .iter()
        .map(|sent| sent.params["text"].as_str().unwrap_or_default().to_owned())
        .collect();
    assert_eq!(typed, [VALUE]);
    let text = snapshot(&browser, 2);
    assert!(!text.to_lowercase().contains(VALUE), "{text}");
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
    match found.content {
        Content::Text(text) => assert!(!text.to_lowercase().contains(VALUE), "{text}"),
        other => panic!("a find is text: {other:?}"),
    }
}

#[test]
fn a_field_in_a_frame_on_another_origin_is_refused_before_anything_is_typed() {
    let (browser, engine) = browser();
    as_fields(&engine, "one_line", Twist::None);
    let pay = ref_of(&snapshot(&browser, 1), "- button \"Pay now\"");

    let filled = browser.fill_secret(browser.epoch(), &fill_of(pay, "http://fixture.test"));

    match filled {
        Err(Refusal::NotFilled(why)) => {
            assert!(why.contains("http://other.test"), "{why}");
            assert!(why.contains("nothing was filled"), "{why}");
        }
        other => panic!("refused otherwise: {}", ended_as(&other)),
    }
    assert_eq!(inserted(&engine), 0);
}

#[test]
fn a_field_that_is_not_one_line_is_refused_before_anything_is_typed() {
    for kind in ["multi_line", "editable", "other"] {
        let (browser, engine) = browser();
        as_fields(&engine, kind, Twist::None);
        let secret = ref_of(&snapshot(&browser, 1), "- textbox \"Secret word\"");

        let filled = browser.fill_secret(browser.epoch(), &fill_of(secret, "http://fixture.test"));

        match filled {
            Err(Refusal::NotFilled(why)) => assert!(why.contains("one-line"), "{kind}: {why}"),
            other => panic!("{kind} refused otherwise: {}", ended_as(&other)),
        }
        assert_eq!(inserted(&engine), 0, "{kind}");
    }
}

#[test]
fn a_page_that_moves_as_the_value_goes_in_ends_the_browser() {
    let (browser, engine) = browser();
    as_fields(&engine, "one_line", Twist::Moves);
    let secret = ref_of(&snapshot(&browser, 1), "- textbox \"Secret word\"");

    let filled = browser.fill_secret(browser.epoch(), &fill_of(secret, "http://fixture.test"));

    match filled {
        Err(Refusal::NotFilled(why)) => assert!(why.contains("ended this browser"), "{why}"),
        other => panic!("refused otherwise: {}", ended_as(&other)),
    }
    assert!(
        !engine.sent("Browser.close").is_empty(),
        "the whole browser is ended"
    );
}

#[test]
fn a_fill_under_a_lease_the_agent_no_longer_holds_types_nothing() {
    let (browser, engine) = browser();
    as_fields(&engine, "one_line", Twist::None);
    let secret = ref_of(&snapshot(&browser, 1), "- textbox \"Secret word\"");
    let before = browser.epoch();
    browser.take_over();

    let filled = browser.fill_secret(before, &fill_of(secret, "http://fixture.test"));

    assert!(filled.is_err(), "{}", ended_as(&filled));
    assert_eq!(inserted(&engine), 0);
}

/// The value itself is kept out for the browser's life, not only what its field holds: a page
/// that echoes it elsewhere, while its field reads otherwise, shows the agent nothing of it.
#[test]
fn a_value_the_page_echoes_away_from_its_field_is_kept_out() {
    const ECHOED: &str = "plum tree 5115";
    let (browser, engine) = browser();
    as_fields(&engine, "one_line", Twist::None);
    let secret = ref_of(&snapshot(&browser, 1), "- textbox \"Secret word\"");
    let fill = SecretFill {
        value: ECHOED.to_owned().into(),
        ..fill_of(secret, "http://fixture.test")
    };

    browser
        .fill_secret(browser.epoch(), &fill)
        .unwrap_or_else(|why| panic!("{}", ended_as(&Err::<(), _>(why))));
    // The page writes it into its text; its field still reads as it did.
    {
        let mut model = super::super::super::lock(&engine.model);
        let nodes = model.ax.get_mut("PAGE-1").unwrap().as_array_mut().unwrap();
        let said = nodes.iter_mut().find(|n| n["nodeId"] == "16").unwrap();
        said["name"]["value"] = json!(format!("You typed {ECHOED} just now"));
    }

    let text = snapshot(&browser, 2);
    assert!(!text.contains(ECHOED), "{text}");
    assert!(text.contains("You typed"), "{text}");
}
