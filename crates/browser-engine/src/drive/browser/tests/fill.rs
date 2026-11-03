// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A secret purlis fills (#5115), against the stand-in engine: its frames page is on
//! `http://fixture.test` with its "Secret word" field, and its "payment" frame on
//! `http://other.test`. The engine judges the field's own frame and its kind at the moment of
//! filling, and the value is kept out of every later answer.

use std::sync::Arc;

use serde_json::json;

use super::super::fill::{FIELD_KIND, WRITE_FIELD};
use super::{Engine, browser, id, ref_of, snapshot};
use crate::drive::{Call, Content, Ref, Refusal, SecretFill};

const VALUE: &str = "open sesame";

/// What the field does as purlis fills it, in a test that says so.
#[derive(Clone, Copy, PartialEq)]
enum Twist {
    None,
    /// The field's document has moved on, or off the origin, by the time the value is written.
    Moves,
    /// The field keeps less than was written into it: a number field given letters.
    Shortens,
}

/// The stand-in's fields, each read as `kind`, and what each write into them answers.
fn as_fields(engine: &Arc<Engine>, kind: &'static str, twist: Twist) {
    engine.on(move |sent, _engine| {
        let p = &sent.params;
        match sent.method.as_str() {
            "Runtime.callFunctionOn" if p["functionDeclaration"] == FIELD_KIND => Some(Ok(
                json!({ "result": { "type": "object", "value": { "kind": kind, "label": "Secret word" } } }),
            )),
            "Runtime.callFunctionOn" if p["functionDeclaration"] == WRITE_FIELD => {
                let value = p["arguments"][1]["value"].as_str().unwrap_or_default();
                let held = value.encode_utf16().count();
                Some(Ok(match twist {
                    Twist::None => json!({ "result": { "type": "number", "value": held } }),
                    Twist::Moves => json!({ "result": { "type": "string", "value": "moved" } }),
                    Twist::Shortens => json!({ "result": { "type": "number", "value": 0 } }),
                }))
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

/// The values purlis wrote into fields, in order, and whether it pressed any key or typed.
fn written(engine: &Engine) -> Vec<String> {
    assert!(
        engine.sent("Input.insertText").is_empty(),
        "nothing is typed by focus"
    );
    engine
        .sent("Runtime.callFunctionOn")
        .iter()
        .filter(|sent| sent.params["functionDeclaration"] == WRITE_FIELD)
        .map(|sent| {
            sent.params["arguments"][1]["value"]
                .as_str()
                .unwrap_or_default()
                .to_owned()
        })
        .collect()
}

fn inserted(engine: &Engine) -> usize {
    written(engine).len()
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
    assert_eq!(written(&engine), [VALUE]);
    // Written into that one element, checked against the approved origin in the same step.
    let write = engine
        .sent("Runtime.callFunctionOn")
        .into_iter()
        .find(|sent| sent.params["functionDeclaration"] == WRITE_FIELD)
        .expect("written");
    assert_eq!(write.params["arguments"][2]["value"], "http://fixture.test");
    assert!(
        !write.params["uniqueContextId"].is_null(),
        "in purlis's own world"
    );
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
fn a_field_whose_document_moved_when_the_value_is_written_takes_nothing() {
    let (browser, engine) = browser();
    as_fields(&engine, "one_line", Twist::Moves);
    let secret = ref_of(&snapshot(&browser, 1), "- textbox \"Secret word\"");

    let filled = browser.fill_secret(browser.epoch(), &fill_of(secret, "http://fixture.test"));

    match filled {
        Err(Refusal::NotFilled(why)) => assert!(why.contains("nothing was filled"), "{why}"),
        other => panic!("refused otherwise: {}", ended_as(&other)),
    }
}

#[test]
fn a_field_that_does_not_keep_the_value_whole_is_refused_in_words_that_say_why() {
    let (browser, engine) = browser();
    as_fields(&engine, "one_line", Twist::Shortens);
    let secret = ref_of(&snapshot(&browser, 1), "- textbox \"Secret word\"");

    let filled = browser.fill_secret(browser.epoch(), &fill_of(secret, "http://fixture.test"));

    match filled {
        Err(Refusal::NotFilled(why)) => {
            assert!(why.contains("did not keep the value whole"), "{why}")
        }
        other => panic!("refused otherwise: {}", ended_as(&other)),
    }
    assert!(engine.sent("Browser.close").is_empty(), "the browser stays");
}

#[test]
fn once_a_secret_is_filled_the_agent_copies_and_cuts_nothing_in_that_browser() {
    let (browser, engine) = browser();
    as_fields(&engine, "one_line", Twist::None);
    let secret = ref_of(&snapshot(&browser, 1), "- textbox \"Secret word\"");
    let press = |n: u32, keys: &[&str]| {
        browser.call(
            &id(n),
            Call::Press {
                page: None,
                keys: keys.iter().map(|key| (*key).to_owned()).collect(),
            },
        )
    };
    // Before a fill, a copy is the agent's to make.
    assert!(press(2, &["Control+c"]).is_ok());
    browser
        .fill_secret(browser.epoch(), &fill_of(secret, "http://fixture.test"))
        .unwrap_or_else(|why| panic!("{}", ended_as(&Err::<(), _>(why))));
    let keys_before = engine.sent("Input.dispatchKeyEvent").len();

    for (n, chord) in [
        (3, "Control+c"),
        (4, "Meta+x"),
        (5, "Control+Insert"),
        (6, "Shift+Delete"),
    ] {
        assert_eq!(
            press(n, &["Control+a", chord]),
            Err(Refusal::CopyWithheld),
            "{chord}"
        );
    }

    assert_eq!(
        engine.sent("Input.dispatchKeyEvent").len(),
        keys_before,
        "no key of a refused press reached the page"
    );
    assert!(press(7, &["Tab"]).is_ok(), "other keys still go");
    // Nor is any of it dragged out, whole or in part.
    let name = ref_of(&snapshot(&browser, 8), "- textbox \"Name\"");
    let dragged = browser.call(
        &id(9),
        Call::Drag {
            from: name,
            to: crate::drive::DragTo::Offset { x: 40.0, y: 0.0 },
        },
    );
    assert_eq!(dragged.err(), Some(Refusal::CopyWithheld));
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
