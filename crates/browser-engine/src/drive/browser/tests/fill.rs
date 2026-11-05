// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A secret purlis fills (#5115), against the stand-in engine: its frames page is on
//! `http://fixture.test` with its "Secret word" field, and its "payment" frame on
//! `http://other.test`. The engine judges the field's own frame and its kind at the moment of
//! filling, and the value is kept out of every later answer. While the page holding it lives,
//! the browser is held to its site (#5117).

use std::sync::{Arc, Mutex};

use serde_json::json;

use super::super::fill::{FIELD_KIND, WRITE_FIELD};
use super::{Engine, browser, id, ref_of, snapshot, wait_for};
use crate::drive::{Call, Content, FillHold, Ref, Refusal, SecretFill};

const VALUE: &str = "open sesame";

/// What the field does as purlis fills it, in a test that says so.
#[derive(Clone, Copy, PartialEq)]
enum Twist {
    None,
    /// The field's document has moved on, or off the origin, by the time the value is written.
    Moves,
    /// The field keeps less than was written into it: a number field given letters.
    Shortens,
    /// The field is a multi-line one by the time the value is written (#5120): it takes it only
    /// where the write is told the person was asked about one.
    TurnsMultiLine,
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
                    Twist::TurnsMultiLine if p["arguments"][3]["value"] != true => {
                        json!({ "result": { "type": "string", "value": "kind" } })
                    }
                    Twist::TurnsMultiLine => {
                        json!({ "result": { "type": "number", "value": held } })
                    }
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
        multi_line: false,
    }
}

/// **What holds the browser in a test** (#5117): what it was told, in order, each hold with how
/// many values had been written into fields by then; and whether it refuses.
#[derive(Default)]
struct Holding {
    told: Mutex<Vec<String>>,
    engine: Mutex<Option<Arc<Engine>>>,
    refuses: bool,
    /// What happens as it holds, in a test that says so.
    as_held: Mutex<Option<Box<dyn Fn() + Send>>>,
}

impl FillHold for Holding {
    fn hold(&self) -> Result<(), String> {
        let written = super::super::super::lock(&self.engine)
            .as_ref()
            .map_or(0, |engine| inserted(engine));
        super::super::super::lock(&self.told).push(format!("held, {written} written"));
        if let Some(happens) = super::super::super::lock(&self.as_held).as_ref() {
            happens();
        }
        if self.refuses {
            return Err("the stand-in holds nothing".to_owned());
        }
        Ok(())
    }

    fn released(&self) {
        super::super::super::lock(&self.told).push("released".to_owned());
    }
}

impl Holding {
    fn told(&self) -> Vec<String> {
        super::super::super::lock(&self.told).clone()
    }
}

/// `fill` made in `browser`, held by a stand-in that holds.
fn filling(
    browser: &crate::drive::Browser,
    fill: &SecretFill,
) -> Result<crate::drive::Answer, Refusal> {
    browser.fill_secret(browser.epoch(), fill, Arc::new(Holding::default()))
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

    let answer = filling(&browser, &fill_of(secret, "http://fixture.test"))
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

    let filled = filling(&browser, &fill_of(pay, "http://fixture.test"));

    match filled {
        // Named as on its own origin once its document is known, and as not known before: the
        // stand-in's frame of its own may not be read yet (the real engine's test names it).
        Err(Refusal::NotFilled(why)) => {
            assert!(
                why.contains("http://other.test") || why.contains("does not know yet"),
                "{why}"
            );
            assert!(why.contains("nothing was filled"), "{why}");
        }
        other => panic!("refused otherwise: {}", ended_as(&other)),
    }
    assert_eq!(inserted(&engine), 0);
}

#[test]
fn a_field_in_a_frame_whose_document_is_not_known_yet_is_refused_in_words_that_say_so() {
    let (browser, engine) = browser();
    as_fields(&engine, "one_line", Twist::None);
    let secret = ref_of(&snapshot(&browser, 1), "- textbox \"Secret word\"");
    engine.emit(
        "Page.frameStartedLoading",
        json!({ "frameId": "PAGE-1" }),
        Some("S-PAGE-1"),
    );
    wait_for("the page's start is read", || {
        super::super::super::lock(&browser.inner.state)
            .document_origins
            .awaits("PAGE-1", "PAGE-1")
    });

    let filled = filling(&browser, &fill_of(secret, "http://fixture.test"));

    match filled {
        Err(Refusal::NotFilled(why)) => {
            assert!(why.contains("does not know yet"), "{why}");
            assert!(why.contains("nothing was filled"), "{why}");
        }
        other => panic!("refused otherwise: {}", ended_as(&other)),
    }
    assert_eq!(inserted(&engine), 0);
}

#[test]
fn a_field_in_a_frame_the_agent_is_kept_off_is_neither_read_nor_filled() {
    let (browser, engine) = browser();
    as_fields(&engine, "one_line", Twist::None);
    let pay = ref_of(&snapshot(&browser, 1), "- button \"Pay now\"");
    browser.keep_off(Arc::new(|address: &str| {
        address
            .starts_with("http://other.test")
            .then(|| "other.test:80".to_owned())
    }));
    let reads = engine.sent("Runtime.callFunctionOn").len();

    let read = browser.call(
        &id(2),
        Call::DescribeField {
            target: pay.clone(),
        },
    );
    assert!(
        matches!(&read, Err(Refusal::KeptOff { site, .. }) if site == "other.test:80"),
        "{read:?}"
    );
    assert_eq!(
        engine.sent("Runtime.callFunctionOn").len(),
        reads,
        "its field was not read"
    );

    let filled = filling(&browser, &fill_of(pay, "http://other.test"));
    assert!(
        matches!(&filled, Err(Refusal::KeptOff { site, .. }) if site == "other.test:80"),
        "{filled:?}"
    );
    assert_eq!(inserted(&engine), 0);
}

#[test]
fn a_field_that_is_not_one_line_is_refused_before_anything_is_typed_where_nobody_was_asked() {
    for kind in ["multi_line", "editable", "other"] {
        let (browser, engine) = browser();
        as_fields(&engine, kind, Twist::None);
        let secret = ref_of(&snapshot(&browser, 1), "- textbox \"Secret word\"");

        let filled = filling(&browser, &fill_of(secret, "http://fixture.test"));

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

    let filled = filling(&browser, &fill_of(secret, "http://fixture.test"));

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

    let filled = filling(&browser, &fill_of(secret, "http://fixture.test"));

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
    filling(&browser, &fill_of(secret, "http://fixture.test"))
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
    assert_eq!(
        press(7, &["Tab"]),
        Err(Refusal::KeysWithheld),
        "nor does any other key, on the page that holds the value"
    );
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

    let filled = browser.fill_secret(
        before,
        &fill_of(secret, "http://fixture.test"),
        Arc::new(Holding::default()),
    );

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

    filling(&browser, &fill).unwrap_or_else(|why| panic!("{}", ended_as(&Err::<(), _>(why))));
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

/// The page's main frame shows another document, as the engine tells it.
fn main_frame_loads(engine: &Engine, url: &str) {
    engine.emit(
        "Page.frameNavigated",
        json!({ "frame": { "id": "PAGE-1", "loaderId": "LOADER-NEXT", "url": url, "name": "" } }),
        Some("S-PAGE-1"),
    );
}

/// The stand-in's page, filled under `holding`, which holds.
fn filled_under(holding: &Arc<Holding>) -> (crate::drive::Browser, Arc<Engine>) {
    let (browser, engine) = browser();
    as_fields(&engine, "one_line", Twist::None);
    *super::super::super::lock(&holding.engine) = Some(Arc::clone(&engine));
    let secret = ref_of(&snapshot(&browser, 1), "- textbox \"Secret word\"");
    browser
        .fill_secret(
            browser.epoch(),
            &fill_of(secret, "http://fixture.test"),
            Arc::clone(holding) as Arc<dyn FillHold>,
        )
        .unwrap_or_else(|why| panic!("{}", ended_as(&Err::<(), _>(why))));
    (browser, engine)
}

#[test]
fn the_browser_is_held_to_the_site_before_the_value_goes_in() {
    let holding = Arc::new(Holding::default());

    let (_browser, engine) = filled_under(&holding);

    assert_eq!(holding.told(), ["held, 0 written"]);
    assert_eq!(written(&engine), [VALUE]);
}

#[test]
fn a_browser_that_cannot_be_held_takes_no_value_and_says_why() {
    let (browser, engine) = browser();
    as_fields(&engine, "one_line", Twist::None);
    let secret = ref_of(&snapshot(&browser, 1), "- textbox \"Secret word\"");
    let holding = Arc::new(Holding {
        refuses: true,
        ..Holding::default()
    });

    let filled = browser.fill_secret(
        browser.epoch(),
        &fill_of(secret, "http://fixture.test"),
        Arc::clone(&holding) as Arc<dyn FillHold>,
    );

    match filled {
        Err(Refusal::NotFilled(why)) => {
            assert!(why.contains("could not hold this browser"), "{why}");
            assert!(why.contains("nothing was filled"), "{why}");
        }
        other => panic!("refused otherwise: {}", ended_as(&other)),
    }
    assert_eq!(inserted(&engine), 0);
}

#[test]
fn a_route_change_or_a_frames_document_keeps_the_hold_and_a_new_page_document_ends_it_once() {
    let holding = Arc::new(Holding::default());
    let (browser, engine) = filled_under(&holding);

    // A single-page app's route change loads no document; nor does the page, when a frame in it
    // loads one.
    engine.emit(
        "Page.navigatedWithinDocument",
        json!({ "frameId": "PAGE-1", "url": "http://fixture.test/frames.html#/settings" }),
        Some("S-PAGE-1"),
    );
    engine.emit(
        "Page.frameNavigated",
        json!({ "frame": { "id": "PAYMENT-FRAME", "parentId": "PAGE-1", "name": "payment", "url": "http://fixture.test/after.html" } }),
        Some("S-PAGE-1"),
    );
    // Read once the engine's words are in: a snapshot answers after them.
    let _ = snapshot(&browser, 2);
    assert_eq!(holding.told(), ["held, 0 written"], "still held");

    main_frame_loads(&engine, "http://fixture.test/next.html");
    wait_for("the hold is released", || holding.told().len() == 2);
    main_frame_loads(&engine, "http://fixture.test/later.html");
    let _ = snapshot(&browser, 3);

    assert_eq!(holding.told(), ["held, 0 written", "released"]);
}

/// A window opens at `url`, opened by `opener`, and the driver knows of it.
fn a_window_opens(
    browser: &crate::drive::Browser,
    engine: &Engine,
    target: &str,
    url: &str,
    opener: &str,
    linked: bool,
) {
    engine.emit(
        "Target.targetCreated",
        json!({"targetInfo": {
            "targetId": target, "type": "page", "title": "", "url": url, "attached": false,
            "openerId": opener, "canAccessOpener": linked,
        }}),
        None,
    );
    wait_for("the window is known", || {
        super::super::super::lock(&browser.inner.state)
            .page(target)
            .is_some()
    });
}

fn closes_asked_for(engine: &Engine) -> Vec<String> {
    engine
        .sent("Target.closeTarget")
        .iter()
        .filter_map(|sent| sent.params["targetId"].as_str().map(str::to_owned))
        .collect()
}

fn a_window_ends(engine: &Engine, target: &str) {
    engine.emit(
        "Target.targetDestroyed",
        json!({ "targetId": target }),
        None,
    );
}

#[test]
fn a_fill_is_refused_while_a_related_window_is_on_its_site_or_on_none() {
    for url in ["http://fixture.test/more.html", "about:blank"] {
        let (browser, engine) = browser();
        as_fields(&engine, "one_line", Twist::None);
        let secret = ref_of(&snapshot(&browser, 1), "- textbox \"Secret word\"");
        a_window_opens(&browser, &engine, "PAGE-2", url, "PAGE-1", true);
        let holding = Arc::new(Holding::default());

        let filled = browser.fill_secret(
            browser.epoch(),
            &fill_of(secret, "http://fixture.test"),
            Arc::clone(&holding) as Arc<dyn FillHold>,
        );

        match filled {
            Err(Refusal::NotFilled(why)) => {
                assert!(why.contains("related to this page"), "{why}");
                assert!(why.contains("nothing was filled"), "{why}");
            }
            other => panic!("refused otherwise: {}", ended_as(&other)),
        }
        assert_eq!(inserted(&engine), 0, "{url}");
        assert!(holding.told().is_empty(), "{url} is held for nothing");
    }
}

#[test]
fn a_fill_goes_in_beside_a_window_on_another_site_or_one_with_no_link_to_it() {
    for (url, linked) in [
        ("http://other.test/more.html", true),
        ("http://fixture.test/more.html", false),
    ] {
        let (browser, engine) = browser();
        as_fields(&engine, "one_line", Twist::None);
        let secret = ref_of(&snapshot(&browser, 1), "- textbox \"Secret word\"");
        a_window_opens(&browser, &engine, "PAGE-2", url, "PAGE-1", linked);

        let filled = filling(&browser, &fill_of(secret, "http://fixture.test"));

        assert!(filled.is_ok(), "{url}: {}", ended_as(&filled));
        assert_eq!(written(&engine), [VALUE], "{url}");
    }
}

#[test]
fn a_hold_ends_only_once_its_related_windows_are_gone() {
    let holding = Arc::new(Holding::default());
    let (browser, engine) = filled_under(&holding);
    a_window_opens(&browser, &engine, "PAGE-2", "about:blank", "PAGE-1", true);
    a_window_opens(&browser, &engine, "PAGE-3", "about:blank", "PAGE-2", true);
    a_window_opens(&browser, &engine, "PAGE-4", "about:blank", "PAGE-1", false);

    main_frame_loads(&engine, "http://fixture.test/next.html");

    wait_for("the related windows are closed", || {
        let closes = closes_asked_for(&engine);
        closes.contains(&"PAGE-2".to_owned()) && closes.contains(&"PAGE-3".to_owned())
    });
    assert!(
        super::super::super::lock(&browser.inner.state)
            .leaving_said
            .iter()
            .any(|said| said.contains(
                "purlis closed 2 other windows related to the page that held the secret: they \
                 could have read it."
            )),
        "the chat is told"
    );
    assert!(
        !closes_asked_for(&engine).contains(&"PAGE-4".to_owned()),
        "a window with no link to it stays"
    );
    assert_eq!(holding.told(), ["held, 0 written"], "still held");

    a_window_ends(&engine, "PAGE-2");
    std::thread::sleep(std::time::Duration::from_millis(100));
    assert_eq!(holding.told(), ["held, 0 written"], "one is left");

    a_window_ends(&engine, "PAGE-3");
    wait_for("the hold is released", || {
        holding.told() == ["held, 0 written", "released"]
    });
}

#[test]
fn a_window_opened_as_a_hold_ends_closes_too_and_the_hold_waits_for_it() {
    let holding = Arc::new(Holding::default());
    let (browser, engine) = filled_under(&holding);
    a_window_opens(&browser, &engine, "PAGE-2", "about:blank", "PAGE-1", true);
    main_frame_loads(&engine, "http://fixture.test/next.html");
    wait_for("the window is closed", || {
        closes_asked_for(&engine).contains(&"PAGE-2".to_owned())
    });

    a_window_opens(&browser, &engine, "PAGE-3", "about:blank", "PAGE-2", true);
    a_window_ends(&engine, "PAGE-2");

    wait_for("the later window is closed", || {
        closes_asked_for(&engine).contains(&"PAGE-3".to_owned())
    });
    assert_eq!(holding.told(), ["held, 0 written"], "still held");
    a_window_ends(&engine, "PAGE-3");
    wait_for("the hold is released", || {
        holding.told() == ["held, 0 written", "released"]
    });
}

#[test]
fn an_act_that_addresses_a_page_ends_the_chain_it_was_on() {
    let (browser, _engine) = browser();
    let _ = snapshot(&browser, 1);
    if let Some(page) = super::super::super::lock(&browser.inner.state).page_mut("PAGE-1") {
        page.chain_began = Some("http://other.test".to_owned());
    }

    drop(browser.addressing("PAGE-1", crate::drive::Why::Addressed));

    let state = super::super::super::lock(&browser.inner.state);
    assert_eq!(state.page("PAGE-1").unwrap().chain_began, None);
}

#[test]
fn a_chosen_load_that_shows_no_document_begins_no_chain() {
    let (browser, engine) = browser();
    let _ = snapshot(&browser, 1);
    let began = |browser: &crate::drive::Browser| {
        super::super::super::lock(&browser.inner.state)
            .page("PAGE-1")
            .and_then(|page| page.chain_began.clone())
    };
    let before = began(&browser);
    if let Some(page) = super::super::super::lock(&browser.inner.state).page_mut("PAGE-1") {
        page.chain_begins = Some(super::super::leaving::ChosenLoad {
            origin: "http://other.test".to_owned(),
            head: "http://other.test/stopped".to_owned(),
        });
    }

    main_frame_loads(&engine, "about:blank");
    let _ = snapshot(&browser, 2);

    assert_eq!(began(&browser), before);
}

#[test]
fn a_closed_page_ends_the_hold() {
    let holding = Arc::new(Holding::default());
    let (_browser, engine) = filled_under(&holding);

    engine.emit(
        "Target.targetDestroyed",
        json!({ "targetId": "PAGE-1" }),
        None,
    );

    wait_for("the hold is released", || {
        holding.told() == ["held, 0 written", "released"]
    });
}

#[test]
fn a_page_that_loads_another_document_as_it_is_held_is_released_and_takes_no_value() {
    let (browser, engine) = browser();
    as_fields(&engine, "one_line", Twist::None);
    let secret = ref_of(&snapshot(&browser, 1), "- textbox \"Secret word\"");
    let loads = Arc::clone(&engine);
    let inner = Arc::clone(&browser.inner);
    let document = move || {
        super::super::super::lock(&inner.state)
            .page("PAGE-1")
            .and_then(|page| page.frames.get("PAGE-1"))
            .map(|main| main.generation)
    };
    let before = document();
    let holding = Arc::new(Holding {
        as_held: Mutex::new(Some(Box::new(move || {
            main_frame_loads(&loads, "http://fixture.test/next.html");
            // The engine's word is read before the hold is listed.
            wait_for("the page's next document is read", || document() != before);
        }))),
        ..Holding::default()
    });

    let filled = browser.fill_secret(
        browser.epoch(),
        &fill_of(secret, "http://fixture.test"),
        Arc::clone(&holding) as Arc<dyn FillHold>,
    );

    match filled {
        Err(Refusal::NotFilled(why)) => assert!(why.contains("nothing was filled"), "{why}"),
        other => panic!("refused otherwise: {}", ended_as(&other)),
    }
    assert_eq!(inserted(&engine), 0);
    assert_eq!(holding.told(), ["held, 0 written", "released"]);
}

#[test]
fn a_page_that_starts_another_document_as_it_is_held_is_released_and_takes_no_value() {
    let (browser, engine) = browser();
    as_fields(&engine, "one_line", Twist::None);
    let secret = ref_of(&snapshot(&browser, 1), "- textbox \"Secret word\"");
    let starts = Arc::clone(&engine);
    let inner = Arc::clone(&browser.inner);
    let holding = Arc::new(Holding {
        as_held: Mutex::new(Some(Box::new(move || {
            // The engine has said the page starts another document, not yet where it is.
            starts.emit(
                "Page.frameStartedLoading",
                json!({ "frameId": "PAGE-1" }),
                Some("S-PAGE-1"),
            );
            wait_for("the page's start is read", || {
                super::super::super::lock(&inner.state)
                    .document_origins
                    .awaits("PAGE-1", "PAGE-1")
            });
        }))),
        ..Holding::default()
    });

    let filled = browser.fill_secret(
        browser.epoch(),
        &fill_of(secret, "http://fixture.test"),
        Arc::clone(&holding) as Arc<dyn FillHold>,
    );

    match filled {
        Err(Refusal::NotFilled(why)) => assert!(why.contains("nothing was filled"), "{why}"),
        other => panic!("refused otherwise: {}", ended_as(&other)),
    }
    assert_eq!(inserted(&engine), 0);
    assert_eq!(holding.told(), ["held, 0 written", "released"]);
}

/// The engine showed its own page for a load that failed: no page is left, and nothing holds the
/// browser (#5230).
#[test]
fn the_engines_own_page_for_a_load_that_failed_ends_the_hold() {
    let holding = Arc::new(Holding::default());
    let (_browser, engine) = filled_under(&holding);

    engine.emit(
        "Page.frameNavigated",
        json!({ "frame": {
            "id": "PAGE-1", "loaderId": "LOADER-ERROR", "name": "",
            "url": "chrome-error://chromewebdata/",
            "unreachableUrl": "http://fixture.test/gone.html",
        } }),
        Some("S-PAGE-1"),
    );

    wait_for("the hold is released", || holding.told().len() == 2);
}

/// What a call's answer says, of the page the test just moved (#5150).
fn note_of_a_read(browser: &crate::drive::Browser, n: u32) -> Option<String> {
    browser
        .call(
            &id(n),
            Call::Snapshot {
                page: None,
                within: None,
                from: 1,
                lines: None,
            },
        )
        .unwrap()
        .note
}

/// #5150: a page that shows the engine's own page for a load answers what the caller says of that
/// load, told the address, and says nothing once a document loads, or where the caller did not
/// refuse it.
#[test]
fn an_answer_on_the_engines_own_page_says_why_its_caller_refused_that_load() {
    let holding = Arc::new(Holding::default());
    let (browser, engine) = filled_under(&holding);
    let asked = Arc::new(Mutex::new(Vec::new()));
    let told = Arc::clone(&asked);
    browser.say_why_refused(Arc::new(move |address: &str| {
        told.lock().unwrap().push(address.to_owned());
        address
            .contains("elsewhere")
            .then(|| "held to the fixture".to_owned())
    }));
    assert_eq!(note_of_a_read(&browser, 2), None, "no load failed");

    let error_page_for = |address: &str| {
        engine.emit(
            "Page.frameNavigated",
            json!({ "frame": {
                "id": "PAGE-1", "loaderId": "LOADER-ERROR", "name": "",
                "url": "chrome-error://chromewebdata/",
                "unreachableUrl": address,
            } }),
            Some("S-PAGE-1"),
        );
    };
    error_page_for("http://elsewhere.test/search?q=open%20sesame");
    // The engine's word is read on its own thread, so each of these waits for it.
    wait_for("the answer says why", || {
        note_of_a_read(&browser, 3).as_deref() == Some("held to the fixture")
    });
    assert_eq!(
        asked.lock().unwrap().first().map(String::as_str),
        Some("http://elsewhere.test/search?q=open%20sesame")
    );

    error_page_for("http://fixture.test/gone");
    wait_for("its caller refused none, so nothing is said", || {
        note_of_a_read(&browser, 4).is_none()
    });

    error_page_for("http://elsewhere.test/");
    main_frame_loads(&engine, "http://fixture.test/next.html");
    wait_for("the hold is released", || holding.told().len() == 2);
    // What the failed load had to say is said once.
    let _ = note_of_a_read(&browser, 5);
    assert_eq!(note_of_a_read(&browser, 6), None, "a document loaded");
}

/// Going back to an address that carries a filled value would send it again, wherever that
/// address goes: refused, and nothing is sent.
#[test]
fn going_back_to_an_address_that_carries_a_filled_value_is_refused() {
    let holding = Arc::new(Holding::default());
    let (browser, engine) = filled_under(&holding);
    super::super::super::lock(&engine.model).history = json!({"currentIndex":1,"entries":[
        {"id":1,"url":"http://elsewhere.test/search?q=open%20sesame","title":""},
        {"id":2,"url":"http://fixture.test/frames.html","title":""}]});

    let back = browser.call(&id(5), Call::Back { page: None });

    assert_eq!(back.err(), Some(Refusal::EarlierHoldsKept));
    assert!(engine.sent("Page.navigateToHistoryEntry").is_empty());
}

/// SF-5 (#5120): a multi-line box or an editable area takes the value only where the person was
/// asked about this very fill, and the write is told so, to check again in the same step.
#[test]
fn a_multi_line_box_or_an_editable_area_takes_it_where_the_person_was_asked_about_it() {
    for kind in ["multi_line", "editable"] {
        let (browser, engine) = browser();
        as_fields(&engine, kind, Twist::None);
        let secret = ref_of(&snapshot(&browser, 1), "- textbox \"Secret word\"");
        let fill = SecretFill {
            multi_line: true,
            ..fill_of(secret, "http://fixture.test")
        };

        filling(&browser, &fill)
            .unwrap_or_else(|why| panic!("{kind}: {}", ended_as(&Err::<(), _>(why))));

        assert_eq!(written(&engine), [VALUE], "{kind}");
        let write = engine
            .sent("Runtime.callFunctionOn")
            .into_iter()
            .find(|sent| sent.params["functionDeclaration"] == WRITE_FIELD)
            .expect("written");
        assert_eq!(write.params["arguments"][3]["value"], true, "{kind}");
    }
}

/// A one-line fill tells the write so: a field that is a multi-line one when the value is
/// written takes nothing, though it read as one line a moment before.
#[test]
fn a_field_that_turned_multi_line_as_the_value_is_written_takes_nothing() {
    let (browser, engine) = browser();
    as_fields(&engine, "one_line", Twist::TurnsMultiLine);
    let secret = ref_of(&snapshot(&browser, 1), "- textbox \"Secret word\"");

    let filled = filling(&browser, &fill_of(secret, "http://fixture.test"));

    match filled {
        Err(Refusal::NotFilled(why)) => {
            assert!(why.contains("asked each time"), "{why}");
            assert!(why.contains("nothing was filled"), "{why}");
        }
        other => panic!("refused otherwise: {}", ended_as(&other)),
    }
}

/// **A filled field takes no keys from the chat** (#5115): while the page holding the value
/// lives, the agent presses no key in that page, and types or chooses nothing in the filled
/// field's document, whatever it names: what it would change there could be the value, and a page
/// that shows what the field holds would show what is left of it.
#[test]
fn while_the_page_holds_a_filled_value_the_agent_puts_no_key_into_its_document() {
    let holding = Arc::new(Holding::default());
    let (browser, engine) = filled_under(&holding);
    let text = snapshot(&browser, 2);
    let secret = ref_of(&text, "- textbox [");
    let name = ref_of(&text, "- textbox \"Name\"");
    let volume = ref_of(&text, "- slider \"Volume\"");
    let keys_before = engine.sent("Input.dispatchKeyEvent").len();
    let typed_before = engine.sent("Input.insertText").len();
    let typed = |n: u32, into: Option<Ref>| {
        browser.call(
            &id(n),
            Call::Type {
                page: None,
                fields: vec![crate::drive::Field {
                    into,
                    text: "x".into(),
                    replace: false,
                }],
            },
        )
    };

    for (n, keys) in [(3, ["Backspace"]), (4, ["Enter"]), (5, ["a"])] {
        let pressed = browser.call(
            &id(n),
            Call::Press {
                page: None,
                keys: keys.iter().map(|key| (*key).to_owned()).collect(),
            },
        );
        assert_eq!(pressed, Err(Refusal::KeysWithheld), "{keys:?}");
    }
    assert_eq!(
        typed(6, Some(secret)),
        Err(Refusal::KeysWithheld),
        "the field"
    );
    assert_eq!(
        typed(7, Some(name)),
        Err(Refusal::KeysWithheld),
        "its document"
    );
    assert_eq!(
        typed(8, None),
        Err(Refusal::KeysWithheld),
        "where the focus is"
    );
    let chosen = browser.call(
        &id(9),
        Call::Select {
            target: volume,
            options: vec!["Blue".to_owned()],
        },
    );
    assert_eq!(chosen, Err(Refusal::KeysWithheld), "a choice");

    assert_eq!(engine.sent("Input.dispatchKeyEvent").len(), keys_before);
    assert_eq!(engine.sent("Input.insertText").len(), typed_before);
    assert!(
        Refusal::KeysWithheld.to_string().contains("click"),
        "the refusal says what to do: {}",
        Refusal::KeysWithheld
    );
}

/// Once the page holding the value loads another document, its keys are the agent's again.
#[test]
fn once_the_filled_page_loads_another_document_its_keys_go_again() {
    let holding = Arc::new(Holding::default());
    let (browser, engine) = filled_under(&holding);

    main_frame_loads(&engine, "http://fixture.test/next.html");
    wait_for("the hold is released", || holding.told().len() == 2);

    let pressed = browser.call(
        &id(2),
        Call::Press {
            page: None,
            keys: vec!["Enter".to_owned()],
        },
    );
    assert!(pressed.is_ok(), "{}", ended_as(&pressed));
}

#[test]
fn a_window_takes_on_what_its_opener_keeps_and_what_a_page_kept_outlives_it() {
    let (browser, engine) = browser();
    let _ = snapshot(&browser, 1);
    super::super::super::lock(&browser.inner.state)
        .page_mut("PAGE-1")
        .expect("the page")
        .scrubbed
        .run(&["token-5310"]);

    a_window_opens(&browser, &engine, "W-1", "about:blank", "PAGE-1", true);
    let holds = {
        let state = super::super::super::lock(&browser.inner.state);
        let window = state.page("W-1").expect("the window");
        crate::drive::scrub::Scrub::of(&window.scrubbed).holds("token-5310")
    };
    assert!(holds, "it may carry what opened it wrote");

    a_window_ends(&engine, "W-1");
    a_window_ends(&engine, "PAGE-1");
    wait_for("the page is gone", || {
        super::super::super::lock(&browser.inner.state)
            .page("PAGE-1")
            .is_none()
    });
    let named = browser.names_kept_out(vec![crate::drive::DownloadInfo {
        id: "d1".to_owned(),
        name: "token-5310.txt".to_owned(),
        origin: None,
        received: 0,
        total: None,
        state: crate::drive::DownloadState::Done,
        file: "guid".to_owned(),
    }]);
    assert_eq!(named[0].name, "(blanked).txt");
}
