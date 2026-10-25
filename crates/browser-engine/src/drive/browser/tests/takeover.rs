// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Taking a browser over (#2014): one driver at a time, a call in flight cut, references void
//! after the hand-back, and what the person types kept out of every answer.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::json;

use super::{a_frame, browser, focus_on, id, ref_of, snapshot, wait_for};
use crate::drive::{
    Aim, Call, Driver, MouseButton, MouseKind, PageId, Painted, PersonInput, Refusal,
};

/// What the browser's first page shows now, as a frame of it would say.
pub(super) fn aim_now(browser: &super::Browser) -> Aim {
    browser.aim().expect("the browser has a current page")
}

pub(super) fn key(aim: &Aim, key: &str, down: bool) -> PersonInput {
    let code = if key.chars().count() == 1 {
        format!("Key{}", key.to_ascii_uppercase())
    } else {
        key.to_owned()
    };
    PersonInput::Key {
        aim: aim.clone(),
        down,
        key: key.to_owned(),
        code,
        key_code: u32::from(key.chars().next().unwrap_or(' ').to_ascii_uppercase()),
        modifiers: 0,
    }
}

fn chord(aim: &Aim, key: &str, modifiers: u8) -> PersonInput {
    PersonInput::Key {
        aim: aim.clone(),
        down: true,
        key: key.to_owned(),
        code: String::new(),
        key_code: 0,
        modifiers,
    }
}

/// The page writes `said` into its text, as a page that echoes what was typed does.
fn the_page_says(engine: &super::Engine, frame: &str, said: &str) {
    let mut model = super::super::lock(&engine.model);
    let nodes = model.ax.get_mut(frame).unwrap().as_array_mut().unwrap();
    let line = nodes.iter_mut().find(|n| n["nodeId"] == "16").unwrap();
    line["name"]["value"] = json!(said);
}

fn press(aim: &Aim, x: f64, y: f64) -> PersonInput {
    PersonInput::Mouse {
        aim: aim.clone(),
        kind: MouseKind::Pressed,
        x,
        y,
        button: MouseButton::Left,
        clicks: 1,
        modifiers: 0,
        delta_x: 0.0,
        delta_y: 0.0,
    }
}

#[test]
fn a_call_in_flight_when_the_person_takes_over_answers_interrupted_state_unknown() {
    let (browser, _engine) = browser();
    let browser = Arc::new(browser);
    let waiting = Arc::clone(&browser);
    let started = Instant::now();
    let call = std::thread::spawn(move || {
        waiting.call(
            &id(1),
            Call::WaitForText {
                page: None,
                text: "never shown".into(),
                gone: false,
                within: Duration::from_secs(20),
            },
        )
    });
    std::thread::sleep(Duration::from_millis(150));

    browser.take_over();

    let answered = call.join().unwrap();
    assert_eq!(answered, Err(Refusal::Interrupted));
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "the call was cut, not left to run out its wait"
    );
    assert_eq!(browser.driver(), Driver::Person);
}

#[test]
fn while_the_person_drives_every_call_is_refused_in_words_and_acts_on_nothing() {
    let (browser, engine) = browser();
    let more = ref_of(&snapshot(&browser, 1), "- link \"Read more\"");
    browser.take_over();
    let reads_before = engine.sent("Accessibility.getFullAXTree").len();

    let clicked = browser.call(&id(2), Call::Click { target: more });
    let read = browser.call(
        &id(3),
        Call::Snapshot {
            page: None,
            within: None,
            from: 1,
            lines: None,
        },
    );
    let shot = browser.call(&id(4), Call::Screenshot { page: None });

    assert_eq!(clicked, Err(Refusal::PersonDriving));
    assert_eq!(read, Err(Refusal::PersonDriving));
    assert_eq!(shot, Err(Refusal::PersonDriving));
    assert!(engine.sent("Input.dispatchMouseEvent").is_empty());
    assert!(engine.sent("Page.captureScreenshot").is_empty());
    assert_eq!(
        engine.sent("Accessibility.getFullAXTree").len(),
        reads_before,
        "nothing of the page is read for the agent while the person drives"
    );
    assert!(
        Refusal::PersonDriving
            .to_string()
            .contains("the person has taken this browser over"),
        "{}",
        Refusal::PersonDriving
    );
}

#[test]
fn a_call_made_under_an_earlier_lease_is_refused_as_interrupted() {
    let (browser, engine) = browser();
    let more = ref_of(&snapshot(&browser, 1), "- link \"Read more\"");
    let before = browser.epoch();
    browser.take_over();
    browser.hand_back();

    let late = browser.call_at(before, &id(2), Call::Click { target: more });

    assert_eq!(late, Err(Refusal::Interrupted));
    assert!(engine.sent("Input.dispatchMouseEvent").is_empty());
}

#[test]
fn references_dealt_before_a_take_over_are_refused_after_the_hand_back() {
    let (browser, _engine) = browser();
    let more = ref_of(&snapshot(&browser, 1), "- link \"Read more\"");
    browser.take_over();
    browser.hand_back();

    let clicked = browser.call(
        &id(2),
        Call::Click {
            target: more.clone(),
        },
    );
    let again = ref_of(&snapshot(&browser, 3), "- link \"Read more\"");

    assert_eq!(clicked, Err(Refusal::HandedBack(more.clone())));
    assert_ne!(again, more, "a new snapshot deals new references");
    assert_eq!(browser.driver(), Driver::Agent);
    browser
        .call(&id(4), Call::Click { target: again })
        .expect("a reference dealt after the hand-back is live");
}

#[test]
fn the_persons_input_is_refused_until_they_take_the_browser_over() {
    let (browser, engine) = browser();
    let aim = aim_now(&browser);

    let refused = browser.person_input(&press(&aim, 10.0, 10.0));

    assert_eq!(refused, Err(Refusal::NotDriving));
    assert!(engine.sent("Input.dispatchMouseEvent").is_empty());
}

#[test]
fn the_persons_click_reaches_the_page_where_they_aimed_it() {
    let (browser, engine) = browser();
    browser.take_over();
    let aim = aim_now(&browser);

    browser.person_input(&press(&aim, 42.0, 17.5)).unwrap();

    let sent = engine.sent("Input.dispatchMouseEvent");
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].session.as_deref(), Some("S-PAGE-1"));
    assert_eq!(sent[0].params["type"], "mousePressed");
    assert_eq!(sent[0].params["x"], 42.0);
    assert_eq!(sent[0].params["y"], 17.5);
    assert_eq!(sent[0].params["button"], "left");
    assert_eq!(sent[0].params["clickCount"], 1);
}

#[test]
fn input_aimed_at_a_document_the_page_has_left_is_dropped() {
    let (browser, engine) = browser();
    browser.take_over();
    let aim = aim_now(&browser);
    // The page loads another document.
    engine.emit(
        "Page.frameNavigated",
        json!({"frame":{"id":"PAGE-1","loaderId":"LOADER-7","url":"http://fixture.test/next.html","name":""}}),
        Some("S-PAGE-1"),
    );
    wait_for("the page's new document is known", || {
        aim_now(&browser) != aim
    });

    let late = browser.person_input(&press(&aim, 42.0, 17.5));
    let elsewhere = browser.person_input(&press(
        &Aim {
            page: PageId::new("p9"),
            document: aim_now(&browser).document,
        },
        1.0,
        1.0,
    ));

    assert_eq!(late, Err(Refusal::Missed));
    assert_eq!(elsewhere, Err(Refusal::Missed));
    assert!(engine.sent("Input.dispatchMouseEvent").is_empty());
}

#[test]
fn the_field_the_person_types_into_is_blanked_before_their_key_reaches_it() {
    let (browser, engine) = browser();
    browser.take_over();
    let aim = aim_now(&browser);
    let reads_before = engine.sent("Accessibility.getFullAXTree").len();

    browser.person_input(&key(&aim, "x", true)).unwrap();
    browser.person_input(&key(&aim, "x", false)).unwrap();

    let sent = engine.all_sent();
    let first_key = sent
        .iter()
        .position(|s| s.method == "Input.dispatchKeyEvent")
        .expect("the key reached the page");
    let read_before_key = sent[..first_key]
        .iter()
        .filter(|s| s.method == "Accessibility.getFullAXTree")
        .count();
    assert!(
        read_before_key > reads_before,
        "the page was read (and its focused field blanked) before the key was sent"
    );
    let down = &engine.sent("Input.dispatchKeyEvent")[0];
    assert_eq!(down.params["type"], "keyDown");
    assert_eq!(down.params["text"], "x");

    browser.hand_back();
    let text = snapshot(&browser, 1);
    assert!(text.contains("- textbox [e"), "{text}");
    assert!(text.contains("(blanked)"), "{text}");
    assert!(!text.contains("Ada"), "{text}");
}

#[test]
fn a_key_waits_when_the_page_cannot_say_which_field_has_the_focus() {
    let (browser, engine) = browser();
    focus_on(&engine, &[("PAGE-1", "8"), ("PAYMENT-FRAME", "3")]);
    browser.take_over();
    let aim = aim_now(&browser);

    let typed = browser.person_input(&key(&aim, "x", true));

    assert_eq!(typed, Err(Refusal::FocusUnclear));
    assert!(engine.sent("Input.dispatchKeyEvent").is_empty());
}

#[test]
fn what_the_person_types_with_nothing_focused_stays_out_of_later_answers() {
    let (browser, engine) = browser();
    focus_on(&engine, &[]);
    browser.take_over();
    let aim = aim_now(&browser);
    for letter in ["q", "z", "w", "v"] {
        browser.person_input(&key(&aim, letter, true)).unwrap();
        browser.person_input(&key(&aim, letter, false)).unwrap();
    }
    // The page writes what it heard into its text.
    {
        let mut model = super::super::lock(&engine.model);
        let nodes = model.ax.get_mut("PAGE-1").unwrap().as_array_mut().unwrap();
        let said = nodes.iter_mut().find(|n| n["nodeId"] == "16").unwrap();
        said["name"]["value"] = json!("You typed qzwv just now");
    }

    browser.hand_back();

    let text = snapshot(&browser, 1);
    assert!(!text.contains("qzwv"), "{text}");
}

#[test]
fn each_frame_names_the_page_and_document_it_shows() {
    let (browser, engine) = browser();
    let shown: Arc<Mutex<Vec<Painted>>> = Arc::default();
    let into = Arc::clone(&shown);
    browser.show(Arc::new(move |painted| {
        super::super::lock(&into).push(painted);
    }));
    wait_for("started", || {
        !engine.sent("Page.startScreencast").is_empty()
    });

    a_frame(&engine, b"one", 1);
    wait_for("the first shown", || super::super::lock(&shown).len() == 1);
    engine.emit(
        "Page.frameNavigated",
        json!({"frame":{"id":"PAGE-1","loaderId":"LOADER-7","url":"http://fixture.test/next.html","name":""}}),
        Some("S-PAGE-1"),
    );
    a_frame(&engine, b"two", 2);
    wait_for("the second shown", || super::super::lock(&shown).len() == 2);

    let shown = super::super::lock(&shown);
    assert_eq!(shown[0].aim.page, PageId::new("p1"));
    assert_eq!(shown[0].jpeg, b"one");
    assert_eq!(shown[1].aim.page, PageId::new("p1"));
    assert_ne!(
        shown[0].aim.document, shown[1].aim.document,
        "a new document is a new aim"
    );
}

#[test]
fn a_size_is_set_on_every_page_the_browser_has() {
    let (browser, engine) = browser();
    browser.take_over();

    browser.size_to(390, 844).unwrap();

    let set = engine.sent("Emulation.setDeviceMetricsOverride");
    assert_eq!(set.len(), 1);
    assert_eq!(set[0].session.as_deref(), Some("S-PAGE-1"));
    assert_eq!(set[0].params["width"], 390);
    assert_eq!(set[0].params["height"], 844);
    assert_eq!(browser.size(), Some((390, 844)));
}

#[test]
fn a_size_is_set_only_by_the_person_driving() {
    let (browser, engine) = browser();

    assert_eq!(browser.size_to(390, 844), Err(Refusal::NotDriving));
    assert!(engine.sent("Emulation.setDeviceMetricsOverride").is_empty());
}

#[test]
fn what_a_call_read_after_the_person_took_over_is_withheld_from_the_agent() {
    let (browser, engine) = browser();
    let browser = Arc::new(browser);
    let taking = Arc::downgrade(&browser);
    // The person takes over while the snapshot's read of the page is under way.
    engine.on(move |sent, _| {
        if sent.method == "Accessibility.getFullAXTree"
            && let Some(browser) = taking.upgrade()
        {
            browser.take_over();
        }
        None
    });

    let read = browser.call(
        &id(1),
        Call::Snapshot {
            page: None,
            within: None,
            from: 1,
            lines: None,
        },
    );

    assert_eq!(read, Err(Refusal::Interrupted));
}

#[test]
fn no_input_of_a_cut_call_reaches_the_page_once_the_person_has_taken_over() {
    let (browser, engine) = browser();
    let more = ref_of(&snapshot(&browser, 1), "- link \"Read more\"");
    let browser = Arc::new(browser);
    let taking = Arc::downgrade(&browser);
    // The person takes over while the click is finding where its element is.
    engine.on(move |sent, _| {
        if sent.method == "DOM.getContentQuads"
            && let Some(browser) = taking.upgrade()
        {
            browser.take_over();
        }
        None
    });

    let clicked = browser.call(&id(2), Call::Click { target: more });

    assert_eq!(clicked, Err(Refusal::Interrupted));
    assert!(engine.sent("Input.dispatchMouseEvent").is_empty());
}

#[test]
fn what_the_person_typed_stays_out_of_later_answers_even_once_its_field_is_gone() {
    let (browser, engine) = browser();
    browser.take_over();
    let aim = aim_now(&browser);
    for letter in ["x", "y", "z", "k"] {
        browser.person_input(&key(&aim, letter, true)).unwrap();
        browser.person_input(&key(&aim, letter, false)).unwrap();
    }
    // The page loads another document by itself, which says what was typed.
    {
        let mut model = super::super::lock(&engine.model);
        let nodes = model.ax.get_mut("PAGE-1").unwrap().as_array_mut().unwrap();
        let said = nodes.iter_mut().find(|n| n["nodeId"] == "16").unwrap();
        said["name"]["value"] = json!("Welcome back, XYZK");
    }
    engine.emit(
        "Page.frameNavigated",
        json!({"frame":{"id":"PAGE-1","loaderId":"LOADER-7","url":"http://fixture.test/next.html","name":""}}),
        Some("S-PAGE-1"),
    );
    std::thread::sleep(Duration::from_millis(30));

    browser.hand_back();

    let text = snapshot(&browser, 1);
    assert!(!text.to_lowercase().contains("xyzk"), "{text}");
}

#[test]
fn a_character_typed_with_alt_or_option_reaches_the_field_once_it_is_blanked() {
    let (browser, engine) = browser();
    browser.take_over();
    let aim = aim_now(&browser);
    let reads_before = engine.sent("Accessibility.getFullAXTree").len();

    browser.person_input(&chord(&aim, "@", 1)).unwrap();

    let sent = engine.all_sent();
    let key_at = sent
        .iter()
        .position(|s| s.method == "Input.dispatchKeyEvent")
        .expect("the key reached the page");
    assert!(
        sent[..key_at]
            .iter()
            .filter(|s| s.method == "Accessibility.getFullAXTree")
            .count()
            > reads_before,
        "the field was blanked before the key"
    );
    assert_eq!(sent[key_at].params["type"], "keyDown");
    assert_eq!(sent[key_at].params["text"], "@");
}

#[test]
fn a_paste_into_a_field_that_cannot_be_read_waits() {
    // #2138: the engine tells of no world purlis makes, so the field cannot be read.
    let (browser, engine) = browser();
    browser.take_over();
    let aim = aim_now(&browser);
    {
        let mut model = super::super::lock(&engine.model);
        model.worlds_untold = true;
        model.value = "pasted-Secret9".into();
    }

    let pasted = browser.person_input(&chord(&aim, "v", 4));

    assert_eq!(pasted, Err(Refusal::FieldUnread));
    assert!(
        engine
            .sent("Input.dispatchKeyEvent")
            .iter()
            .all(|s| s.params["key"] != "v"),
        "the paste never reached the page"
    );
    browser.hand_back();
    super::super::lock(&engine.model).worlds_untold = false;
    let text = snapshot(&browser, 1);
    assert!(!text.contains("pasted-Secret9"), "{text}");
}

/// Reads of what a field holds fail, by their number from the first (1), where `fails` says.
fn field_reads_fail(engine: &super::Engine, fails: fn(usize) -> bool) {
    let mut reads = 0;
    engine.on(move |sent, _| {
        if sent.method == "Runtime.callFunctionOn"
            && sent.params["functionDeclaration"] == super::super::READ_VALUE
        {
            reads += 1;
            if fails(reads) {
                return Some(Err(crate::cdp::CallError::TimedOut));
            }
        }
        None
    });
}

#[test]
fn a_field_read_that_fails_once_after_a_paste_is_read_again() {
    let (browser, engine) = browser();
    browser.take_over();
    let aim = aim_now(&browser);
    super::super::lock(&engine.model).value = "pasted-Secret9".into();
    // The read before the paste works; the first after it does not.
    field_reads_fail(&engine, |read| read == 2);

    browser.person_input(&chord(&aim, "v", 4)).unwrap();
    let reads = engine
        .sent("Runtime.callFunctionOn")
        .iter()
        .filter(|s| s.params["functionDeclaration"] == super::super::READ_VALUE)
        .count();
    assert_eq!(reads, 3, "read before, after, and again");
    // Every later read fails: what the paste put in was kept as the key went in.
    field_reads_fail(&engine, |_| true);
    the_page_says(&engine, "PAGE-1", "You entered pasted-Secret9");
    browser.hand_back();
    let text = snapshot(&browser, 1);
    assert!(!text.contains("pasted-Secret9"), "{text}");
}

#[test]
fn a_field_that_cannot_be_read_after_a_paste_is_read_as_the_browser_is_handed_back() {
    let (browser, engine) = browser();
    browser.take_over();
    let aim = aim_now(&browser);
    super::super::lock(&engine.model).value = "pasted-Secret9".into();
    field_reads_fail(&engine, |read| read == 2 || read == 3);

    let pasted = browser.person_input(&chord(&aim, "v", 4));
    assert_eq!(pasted, Err(Refusal::FieldUnreadAfterKey));
    the_page_says(&engine, "PAGE-1", "You entered pasted-Secret9");
    browser.hand_back();

    let text = snapshot(&browser, 1);
    assert!(!text.contains("pasted-Secret9"), "{text}");
}

#[test]
fn a_field_that_cannot_be_read_at_the_hand_back_keeps_what_it_was_read_to_hold() {
    let (browser, engine) = browser();
    browser.take_over();
    let aim = aim_now(&browser);
    super::super::lock(&engine.model).value = "pasted-Secret9".into();
    // Read before and after the paste; never again.
    field_reads_fail(&engine, |read| read > 2);

    browser.person_input(&chord(&aim, "v", 4)).unwrap();
    the_page_says(&engine, "PAGE-1", "You entered pasted-Secret9");
    browser.hand_back();

    let text = snapshot(&browser, 1);
    assert!(!text.contains("pasted-Secret9"), "{text}");
}

#[test]
fn what_the_person_pastes_stays_out_of_later_answers() {
    let (browser, engine) = browser();
    browser.take_over();
    let aim = aim_now(&browser);
    // The field holds what was pasted into it; its tree shows only that it holds something.
    super::super::lock(&engine.model).value = "pasted-Secret9".into();

    browser.person_input(&chord(&aim, "v", 4)).unwrap();
    the_page_says(&engine, "PAGE-1", "You entered pasted-Secret9");
    browser.hand_back();

    let text = snapshot(&browser, 1);
    assert!(!text.contains("pasted-Secret9"), "{text}");
}

#[test]
fn what_the_person_typed_in_pieces_stays_out_of_later_answers_whole() {
    let (browser, engine) = browser();
    browser.take_over();
    let aim = aim_now(&browser);
    for letter in ["k", "r"] {
        browser.person_input(&key(&aim, letter, true)).unwrap();
    }
    browser.person_input(&press(&aim, 12.0, 12.0)).unwrap();
    for letter in ["w", "t"] {
        browser.person_input(&key(&aim, letter, true)).unwrap();
    }
    super::super::lock(&engine.model).value = "krwt".into();
    the_page_says(&engine, "PAGE-1", "Welcome, krwt");

    browser.hand_back();

    let text = snapshot(&browser, 1);
    assert!(!text.contains("krwt"), "{text}");
}

#[test]
fn what_the_person_typed_stays_out_of_answers_about_every_page_of_the_browser() {
    let (browser, engine) = browser();
    focus_on(&engine, &[]);
    browser.take_over();
    let aim = aim_now(&browser);
    for letter in ["q", "z", "w", "v"] {
        browser.person_input(&key(&aim, letter, true)).unwrap();
    }
    browser.hand_back();
    // Another page of the same browser shows what was typed.
    {
        let mut model = super::super::lock(&engine.model);
        let main = model.ax["PAGE-1"].clone();
        model.ax.insert("PAGE-2".into(), main);
    }
    the_page_says(&engine, "PAGE-2", "Signed in as qzwv");

    browser
        .call(
            &id(7),
            Call::Open {
                address: crate::WebAddress::parse("http://fixture.test/other.html").unwrap(),
            },
        )
        .unwrap();
    let text = snapshot(&browser, 8);

    assert!(text.contains("Signed in as"), "{text}");
    assert!(!text.contains("qzwv"), "{text}");
}

#[test]
fn a_key_waits_when_the_focus_is_on_something_the_page_does_not_show_as_focused() {
    let (browser, engine) = browser();
    focus_on(&engine, &[]);
    super::super::lock(&engine.model).active = "CANVAS".into();
    browser.take_over();
    let aim = aim_now(&browser);

    let typed = browser.person_input(&key(&aim, "x", true));

    assert_eq!(typed, Err(Refusal::FocusUnseen));
    assert!(engine.sent("Input.dispatchKeyEvent").is_empty());
}

#[test]
fn what_the_person_pasted_stays_out_even_once_the_page_has_moved_on() {
    let (browser, engine) = browser();
    browser.take_over();
    let aim = aim_now(&browser);
    super::super::lock(&engine.model).value = "pasted-Other7".into();
    browser.person_input(&chord(&aim, "v", 4)).unwrap();
    // The page sends the form by itself and loads another document: the field is gone.
    super::super::lock(&engine.model).value = String::new();
    engine.emit(
        "Page.frameNavigated",
        json!({"frame":{"id":"PAGE-1","loaderId":"LOADER-7","url":"http://fixture.test/next.html","name":""}}),
        Some("S-PAGE-1"),
    );
    the_page_says(&engine, "PAGE-1", "Welcome, pasted-Other7");
    std::thread::sleep(Duration::from_millis(30));

    browser.hand_back();

    let text = snapshot(&browser, 1);
    assert!(!text.contains("pasted-Other7"), "{text}");
}

#[test]
fn input_aimed_at_a_frame_whose_inside_has_since_changed_is_dropped() {
    let (browser, engine) = browser();
    browser.take_over();
    let aim = aim_now(&browser);
    // The payment frame inside the page loads another document; the page's own stays.
    engine.emit(
        "Page.frameNavigated",
        json!({"frame":{"id":"PAYMENT-FRAME","parentId":"PAGE-1","loaderId":"LOADER-8","url":"http://other.test/next.html","name":"payment"}}),
        Some("S-PAYMENT"),
    );
    wait_for("the frame's new document is known", || {
        aim_now(&browser) != aim
    });

    let late = browser.person_input(&press(&aim, 120.0, 215.0));

    assert_eq!(late, Err(Refusal::Missed));
    assert!(engine.sent("Input.dispatchMouseEvent").is_empty());
}

/// How many times the page's tree was read.
fn tree_reads(engine: &super::Engine) -> usize {
    engine.sent("Accessibility.getFullAXTree").len()
}

#[test]
fn keys_typed_into_a_field_that_keeps_the_focus_read_the_page_once() {
    // #2295: the first key blanks the focused field, reading the page; while that field keeps
    // the focus, the next keys only ask the document what has it.
    let (browser, engine) = browser();
    browser.take_over();
    let aim = aim_now(&browser);
    browser.person_input(&key(&aim, "x", true)).unwrap();
    let field = engine
        .sent("DOM.resolveNode")
        .last()
        .map(|s| s.params["backendNodeId"].as_i64().unwrap())
        .expect("the field was read after the key");
    super::super::lock(&engine.model).focused_backend = Some(field);
    let reads = tree_reads(&engine);

    for _ in 0..5 {
        browser.person_input(&key(&aim, "y", true)).unwrap();
        browser.person_input(&key(&aim, "y", false)).unwrap();
    }

    assert_eq!(tree_reads(&engine), reads, "the page was read again");
    assert_eq!(engine.sent("Input.dispatchKeyEvent").len(), 11);
    // What the field holds is still read after each key, and kept out.
    super::super::lock(&engine.model).value = "xyyyyy".into();
    browser.person_input(&key(&aim, "y", true)).unwrap();
    browser.hand_back();
    assert!(!snapshot(&browser, 1).contains("xyyyyy"));
}

#[test]
fn a_key_after_the_focus_moved_reads_the_page_again() {
    let (browser, engine) = browser();
    browser.take_over();
    let aim = aim_now(&browser);
    browser.person_input(&key(&aim, "x", true)).unwrap();
    // The page's script moves the focus to another field, as a code's boxes do.
    super::super::lock(&engine.model).focused_backend = Some(9999);
    let reads = tree_reads(&engine);

    browser.person_input(&key(&aim, "y", true)).unwrap();

    assert!(
        tree_reads(&engine) > reads,
        "the new field was not blanked first"
    );
    // And where the document says nothing has the focus.
    super::super::lock(&engine.model).focused_backend = None;
    let reads = tree_reads(&engine);
    browser.person_input(&key(&aim, "z", true)).unwrap();
    assert!(tree_reads(&engine) > reads);
}

/// The payment frame inside the page loads another document, as a rotating advert does; the
/// page's own document stays (#2638).
fn the_payment_frame_moves_on(browser: &super::Browser, engine: &super::Engine, aim: &Aim) {
    engine.emit(
        "Page.frameNavigated",
        json!({"frame":{"id":"PAYMENT-FRAME","parentId":"PAGE-1","loaderId":"LOADER-8","url":"http://other.test/next.html","name":"payment"}}),
        Some("S-PAYMENT"),
    );
    wait_for("the frame's new document is known", || {
        aim_now(browser) != *aim
    });
}

#[test]
fn a_key_typed_into_the_page_while_another_frame_of_it_moves_on_reaches_the_field() {
    let (browser, engine) = browser();
    browser.take_over();
    let aim = aim_now(&browser);
    the_payment_frame_moves_on(&browser, &engine, &aim);

    browser.person_input(&key(&aim, "x", true)).unwrap();
    browser.person_input(&key(&aim, "x", false)).unwrap();

    let sent = engine.sent("Input.dispatchKeyEvent");
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[0].session.as_deref(), Some("S-PAGE-1"));
}

#[test]
fn a_key_for_a_frame_with_the_focus_that_moved_on_is_dropped() {
    let (browser, engine) = browser();
    super::focus_on(&engine, &[("PAYMENT-FRAME", "3")]);
    browser.take_over();
    let aim = aim_now(&browser);
    the_payment_frame_moves_on(&browser, &engine, &aim);

    let late = browser.person_input(&key(&aim, "x", true));

    assert_eq!(late, Err(Refusal::Missed));
    assert!(engine.sent("Input.dispatchKeyEvent").is_empty());
}

/// The person's pointer moved to (`x`, `y`).
fn move_to(aim: &Aim, x: f64, y: f64) -> PersonInput {
    PersonInput::Mouse {
        aim: aim.clone(),
        kind: MouseKind::Moved,
        x,
        y,
        button: MouseButton::None,
        clicks: 0,
        modifiers: 0,
        delta_x: 0.0,
        delta_y: 0.0,
    }
}

#[test]
fn a_move_on_the_page_away_from_a_frame_that_moved_on_reaches_the_page() {
    let (browser, engine) = browser();
    browser.take_over();
    let aim = aim_now(&browser);
    the_payment_frame_moves_on(&browser, &engine, &aim);

    browser.person_input(&move_to(&aim, 42.0, 17.5)).unwrap();

    let sent = engine.sent("Input.dispatchMouseEvent");
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].params["type"], "mouseMoved");
}

#[test]
fn a_press_while_a_frame_of_the_page_moved_on_is_dropped_wherever_it_is() {
    // What the frame that moved on draws may be moved under the point between the check and the
    // press: only a move goes on.
    let (browser, engine) = browser();
    browser.take_over();
    let aim = aim_now(&browser);
    the_payment_frame_moves_on(&browser, &engine, &aim);

    let pressed = browser.person_input(&press(&aim, 42.0, 17.5));

    assert_eq!(pressed, Err(Refusal::Missed));
    assert!(engine.sent("Input.dispatchMouseEvent").is_empty());
}

#[test]
fn a_move_through_a_box_the_pointer_passes_through_onto_a_frame_that_moved_on_is_dropped() {
    // A box of the page's own lies over the payment frame and lets the pointer through: the
    // event goes to the frame below, and that is what it is judged by.
    let (browser, engine) = browser();
    super::super::lock(&engine.model).overlay = true;
    browser.take_over();
    let aim = aim_now(&browser);
    the_payment_frame_moves_on(&browser, &engine, &aim);

    let moved = browser.person_input(&move_to(&aim, 120.0, 215.0));

    assert_eq!(moved, Err(Refusal::Missed));
    assert!(engine.sent("Input.dispatchMouseEvent").is_empty());
}

#[test]
fn a_move_onto_a_frames_element_that_names_no_frame_is_dropped() {
    let (browser, engine) = browser();
    super::super::lock(&engine.model).frame_unnamed = true;
    browser.take_over();
    let aim = aim_now(&browser);
    the_payment_frame_moves_on(&browser, &engine, &aim);

    let moved = browser.person_input(&move_to(&aim, 120.0, 215.0));

    assert_eq!(moved, Err(Refusal::Missed));
}

#[test]
fn a_key_where_the_tree_shows_no_focus_but_the_page_has_one_is_dropped_once_a_frame_moved_on() {
    // A key that does not edit goes wherever the focus is; where the tree shows none and the
    // page's document has an element focused, it may be in a frame purlis has not read.
    let (browser, engine) = browser();
    super::focus_on(&engine, &[]);
    super::super::lock(&engine.model).active = "IFRAME".into();
    browser.take_over();
    let aim = aim_now(&browser);
    the_payment_frame_moves_on(&browser, &engine, &aim);

    let typed = browser.person_input(&key(&aim, "ArrowDown", true));

    assert_eq!(typed, Err(Refusal::Missed));
    assert!(engine.sent("Input.dispatchKeyEvent").is_empty());
}

#[test]
fn a_click_while_a_frame_of_the_page_went_is_dropped() {
    // Where a frame went, the page around it may have moved: what is under the point now is
    // not known to be what the person saw.
    let (browser, engine) = browser();
    browser.take_over();
    let aim = aim_now(&browser);
    engine.emit(
        "Page.frameDetached",
        json!({"frameId":"PAYMENT-FRAME","reason":"remove"}),
        Some("S-PAGE-1"),
    );
    wait_for("the frame is gone", || aim_now(&browser) != aim);

    let late = browser.person_input(&press(&aim, 42.0, 17.5));
    let typed = browser.person_input(&key(&aim, "x", true));

    assert_eq!(late, Err(Refusal::Missed));
    assert!(typed.is_ok(), "{typed:?}");
}

/// How many times the page's own tree was read (each read of the page reads every frame's).
fn page_reads(engine: &super::Engine) -> usize {
    engine
        .sent("Accessibility.getFullAXTree")
        .iter()
        .filter(|s| s.params["frameId"] == "PAGE-1")
        .count()
}

/// The payment frame loads document `n` of a rotating advert.
fn the_payment_frame_loads(engine: &super::Engine, n: u32) {
    engine.emit(
        "Page.frameNavigated",
        json!({"frame":{"id":"PAYMENT-FRAME","parentId":"PAGE-1","loaderId":format!("ADVERT-{n}"),"url":format!("http://other.test/advert-{n}.html"),"name":"payment"}}),
        Some("S-PAYMENT"),
    );
}

#[test]
fn a_key_reads_the_pages_tree_at_most_once_while_another_frame_reloads() {
    // #2974: the payment frame loads another document before every key.
    let (browser, engine) = browser();
    browser.take_over();
    let aim = aim_now(&browser);
    let mut n = 0;
    let mut reloads = |browser: &super::Browser| {
        n += 1;
        let before = aim_now(browser);
        the_payment_frame_loads(&engine, n);
        wait_for("the advert's new document is known", || {
            aim_now(browser) != before
        });
    };

    // The first key into the field reads the tree once: where it lands and what to blank.
    reloads(&browser);
    let reads = page_reads(&engine);
    browser.person_input(&key(&aim, "x", true)).unwrap();
    assert_eq!(page_reads(&engine) - reads, 1, "the first key");

    // While the field keeps the focus, a key reads it not at all.
    let field = engine
        .sent("DOM.resolveNode")
        .last()
        .map(|s| s.params["backendNodeId"].as_i64().unwrap())
        .unwrap();
    super::super::lock(&engine.model).focused_backend = Some(field);
    for _ in 0..3 {
        reloads(&browser);
        let reads = page_reads(&engine);
        browser.person_input(&key(&aim, "y", true)).unwrap();
        assert_eq!(page_reads(&engine), reads, "a key into the same field");
    }
    assert_eq!(engine.sent("Input.dispatchKeyEvent").len(), 4);
}

#[test]
fn a_move_onto_a_frame_swapped_into_a_process_of_its_own_since_is_dropped() {
    // #2973: the payment frame moves to a process of its own, as a frame that navigates to
    // another site does: the engine attaches a new session for it and says nothing more.
    let (browser, engine) = browser();
    browser.take_over();
    let aim = aim_now(&browser);
    engine.emit(
        "Target.attachedToTarget",
        json!({"sessionId":"S-PAYMENT-FRAME","targetInfo":{"targetId":"PAYMENT-FRAME","type":"iframe","title":"","url":"http://third.test/pay.html","attached":true},"waitingForDebugger":true}),
        Some("S-PAGE-1"),
    );
    wait_for("the swap is known", || aim_now(&browser) != aim);

    let moved = browser.person_input(&move_to(&aim, 120.0, 215.0));

    assert_eq!(moved, Err(Refusal::Missed));
    assert!(engine.sent("Input.dispatchMouseEvent").is_empty());
}

#[test]
fn the_first_key_into_the_first_field_reads_only_the_tree_around_it() {
    // #2967: the field that has the document's focus is blanked from the tree around it alone.
    let (browser, engine) = browser();
    browser.take_over();
    let aim = aim_now(&browser);
    let field = super::ax_backend_focused(&engine, "PAGE-1");
    super::super::lock(&engine.model).focused_backend = Some(field);
    let reads = page_reads(&engine);

    browser.person_input(&key(&aim, "x", true)).unwrap();

    assert_eq!(page_reads(&engine), reads, "the whole tree was read");
    assert!(!engine.sent("Accessibility.getPartialAXTree").is_empty());
    let resolved = engine.sent("DOM.resolveNode");
    assert_eq!(
        resolved.last().map(|s| s.params["backendNodeId"].as_i64()),
        Some(Some(field)),
        "the field was read after the key"
    );
    super::super::lock(&engine.model).value = "first-field-secret".into();
    browser.person_input(&key(&aim, "y", true)).unwrap();
    browser.hand_back();
    let text = snapshot(&browser, 1);
    assert!(!text.contains("first-field-secret"), "{text}");
}

#[test]
fn a_first_key_on_an_element_that_is_no_field_reads_the_whole_tree() {
    // Only a field the person types into is read from the tree around it: a link, or a frame's
    // element, is not (#2967).
    for backend in [12, 50] {
        let (browser, engine) = browser();
        let node = super::super::lock(&engine.model).ax["PAGE-1"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["backendDOMNodeId"] == backend)
            .and_then(|n| n["nodeId"].as_str().map(str::to_owned))
            .unwrap();
        super::focus_on(&engine, &[("PAGE-1", node.as_str())]);
        super::super::lock(&engine.model).focused_backend = Some(backend);
        browser.take_over();
        let aim = aim_now(&browser);
        let reads = page_reads(&engine);

        let _ = browser.person_input(&key(&aim, "x", true));

        assert!(
            page_reads(&engine) > reads,
            "{backend}: the whole tree was not read"
        );
    }
}

#[test]
fn a_key_into_a_second_field_reads_the_whole_tree() {
    // A code typed one box at a time is kept out whole from the whole page's tree (#2144).
    let (browser, engine) = browser();
    browser.take_over();
    let aim = aim_now(&browser);
    let field = super::ax_backend_focused(&engine, "PAGE-1");
    super::super::lock(&engine.model).focused_backend = Some(field);
    browser.person_input(&key(&aim, "x", true)).unwrap();
    // The page moves the focus to another field.
    super::super::lock(&engine.model).focused_backend = Some(field + 1000);
    let reads = page_reads(&engine);

    let _ = browser.person_input(&key(&aim, "y", true));

    assert!(page_reads(&engine) > reads, "the whole tree was not read");
}

#[test]
fn a_value_typed_key_by_key_leaves_no_part_of_it_in_any_later_answer() {
    let (browser, engine) = browser();
    browser.take_over();
    let aim = aim_now(&browser);
    let value = "squirrel9x";
    for (typed, letter) in value.char_indices() {
        // The field holds what was typed so far, this key included, when it is read after it.
        super::super::lock(&engine.model).value = value[..typed + letter.len_utf8()].to_owned();
        browser
            .person_input(&key(&aim, &letter.to_string(), true))
            .unwrap();
    }
    browser.hand_back();
    the_page_says(&engine, "PAGE-1", "Signed in as squirrel9x");
    engine.emit(
        "Target.targetInfoChanged",
        json!({"targetInfo":{"targetId":"PAGE-1","type":"page","title":"squirrel9x","url":"http://fixture.test/frames.html?q=squirrel9x"}}),
        None,
    );
    std::thread::sleep(Duration::from_millis(30));

    let text = snapshot(&browser, 1);
    let found = browser
        .call(
            &id(2),
            Call::Find {
                page: None,
                text: Some("rrel9x".into()),
                role: None,
                within: None,
            },
        )
        .unwrap();
    let waited = browser
        .call(
            &id(3),
            Call::WaitForText {
                page: None,
                text: "rel9".into(),
                gone: false,
                within: Duration::from_millis(10),
            },
        )
        .unwrap();
    let listed = browser.call(&id(4), Call::Pages).unwrap();

    assert!(text.contains("Signed in as (blanked)\""), "{text}");
    for part in ["rrel", "el9", "l9x", "irre"] {
        assert!(!text.contains(part), "{part} in\n{text}");
    }
    assert!(
        matches!(&found.content, crate::drive::Content::Text(t) if t.ends_with("Nothing on the page matches.\n")),
        "{found:?}"
    );
    assert_eq!(waited.content, crate::drive::Content::Seen(false));
    let page = listed.page.unwrap();
    assert!(!page.address.contains("rel9"), "{page:?}");
    assert!(!page.title.contains("rel9"), "{page:?}");
}

/// The field the person types into (the Name field) holds `value`, as its tree reads it.
fn the_field_shows(engine: &super::Engine, value: &str) {
    let mut model = super::super::lock(&engine.model);
    let nodes = model.ax.get_mut("PAGE-1").unwrap().as_array_mut().unwrap();
    let field = nodes.iter_mut().find(|n| n["nodeId"] == "8").unwrap();
    field["value"]["value"] = json!(value);
}

/// Types `value` into the focused field key by key, its tree and its DOM each reading what it
/// holds as a real page's would: the tree what the keys before typed, the DOM this key too.
fn type_key_by_key(browser: &super::Browser, engine: &super::Engine, value: &str) {
    let aim = aim_now(browser);
    for (typed, letter) in value.char_indices() {
        the_field_shows(engine, &value[..typed]);
        super::super::lock(&engine.model).value = value[..typed + letter.len_utf8()].to_owned();
        browser
            .person_input(&key(&aim, &letter.to_string(), true))
            .unwrap();
    }
    the_field_shows(engine, value);
}

#[test]
fn a_word_the_person_typed_only_on_the_way_to_a_longer_value_is_not_kept_out() {
    let (browser, engine) = browser();
    browser.take_over();
    type_key_by_key(&browser, &engine, "themes");
    browser.hand_back();
    the_page_says(&engine, "PAGE-1", "Pick the one: them, or your themes");

    let text = snapshot(&browser, 1);

    assert!(
        text.contains("Pick the one: them, or your (blanked)"),
        "{text}"
    );
}

#[test]
fn a_long_value_typed_key_by_key_stays_out_when_the_page_shows_it_cut_short() {
    let (browser, engine) = browser();
    browser.take_over();
    type_key_by_key(&browser, &engine, "plumtreewalkerquiet7");
    browser.hand_back();
    the_page_says(&engine, "PAGE-1", "Your key: plumtreew…");

    let text = snapshot(&browser, 1);

    assert!(!text.contains("plumtreew"), "{text}");
}

#[test]
fn what_the_person_pasted_stays_out_once_they_edit_around_it() {
    let (browser, engine) = browser();
    browser.take_over();
    let aim = aim_now(&browser);
    // The field holds what was pasted once the paste reaches the page.
    engine.on(|sent, engine| {
        if sent.method == "Input.dispatchKeyEvent" && sent.params["key"] == "v" {
            super::super::lock(&engine.model).value = "zq7kp".into();
        }
        None
    });
    browser.person_input(&chord(&aim, "v", 4)).unwrap();
    browser.person_input(&key(&aim, "Home", true)).unwrap();
    super::super::lock(&engine.model).value = "xzq7kp".into();
    browser.person_input(&key(&aim, "x", true)).unwrap();
    browser.hand_back();
    the_page_says(&engine, "PAGE-1", "Code zq7kp saved");

    let text = snapshot(&browser, 1);

    assert!(text.contains("Code (blanked) saved"), "{text}");
}

/// The field holds `held`; the person selects all of it, pastes `pasted` over it, goes to its
/// end and types `typed`; the page then says it saved `pasted`.
fn paste_over_a_selection(held: &str, pasted: &'static str, typed: &str) -> String {
    let (browser, engine) = browser();
    browser.take_over();
    let aim = aim_now(&browser);
    super::super::lock(&engine.model).value = held.into();
    engine.on(move |sent, engine| {
        if sent.method == "Input.dispatchKeyEvent" && sent.params["key"] == "v" {
            super::super::lock(&engine.model).value = pasted.into();
        }
        None
    });
    browser.person_input(&chord(&aim, "a", 4)).unwrap();
    browser.person_input(&chord(&aim, "v", 4)).unwrap();
    browser.person_input(&key(&aim, "End", true)).unwrap();
    super::super::lock(&engine.model).value = format!("{pasted}{typed}");
    browser.person_input(&key(&aim, typed, true)).unwrap();
    browser.hand_back();
    the_page_says(&engine, "PAGE-1", &format!("Code {pasted} saved"));
    snapshot(&browser, 1)
}

#[test]
fn what_the_person_pasted_over_a_selection_stays_out_once_they_edit_after_it() {
    let text = paste_over_a_selection("zq7kpx", "zq7kp", "9");
    assert!(text.contains("Code (blanked) saved"), "{text}");
}

#[test]
fn a_code_pasted_over_the_one_before_it_stays_out_once_they_edit_after_it() {
    let text = paste_over_a_selection("123456", "123789", "0");
    assert!(text.contains("Code (blanked) saved"), "{text}");
}

#[test]
fn a_screenshot_paints_over_what_the_person_typed_wherever_the_page_shows_it_as_text() {
    let (browser, engine) = browser();
    focus_on(&engine, &[]);
    browser.take_over();
    let aim = aim_now(&browser);
    for letter in ["q", "z", "w", "v"] {
        browser.person_input(&key(&aim, letter, true)).unwrap();
    }
    browser.hand_back();
    the_page_says(&engine, "PAGE-1", "Signed in as qzwv");
    // Where the page draws that line of text.
    super::super::lock(&engine.model)
        .quads
        .insert(31, vec![7.0, 0.0, 9.0, 0.0, 9.0, 1.0, 7.0, 1.0]);

    let shot = browser
        .call(&id(1), Call::Screenshot { page: None })
        .unwrap();

    let crate::drive::Content::Image(shot) = shot.content else {
        panic!("a screenshot is an image")
    };
    assert_eq!(super::pixel(&shot, 9, 0), vec![128, 128, 128]);
    assert_eq!(super::pixel(&shot, 1, 8), vec![255, 255, 255]);
}

/// The page asks for a code in six boxes of one character each (nodes 101 to 106), before its
/// paragraph.
pub(super) fn the_page_asks_for_a_code(engine: &super::Engine) {
    let mut model = super::super::lock(&engine.model);
    let nodes = model.ax.get_mut("PAGE-1").unwrap().as_array_mut().unwrap();
    for at in 1..=6 {
        nodes.push(json!({
            "nodeId": format!("10{at}"),
            "ignored": false,
            "role": {"type": "role", "value": "textbox"},
            "name": {"type": "computedString", "value": format!("Digit {at}")},
            "value": {"type": "string", "value": ""},
            "properties": [],
            "parentId": "3",
            "childIds": [],
            "backendDOMNodeId": 100 + at,
            "frameId": "PAGE-1",
        }));
    }
    let wrapper = nodes.iter_mut().find(|n| n["nodeId"] == "3").unwrap();
    let children = wrapper["childIds"].as_array_mut().unwrap();
    let paragraph = children.iter().position(|c| c == "15").unwrap();
    for at in (1..=6).rev() {
        children.insert(paragraph, json!(format!("10{at}")));
    }
}

/// Box `at` of the code (1 to 6) holds `held`, as its tree reads it.
fn the_box_shows(engine: &super::Engine, at: usize, held: &str) {
    let mut model = super::super::lock(&engine.model);
    let nodes = model.ax.get_mut("PAGE-1").unwrap().as_array_mut().unwrap();
    let field = nodes
        .iter_mut()
        .find(|n| n["nodeId"] == format!("10{at}"))
        .unwrap();
    field["value"]["value"] = json!(held);
}

/// The page moves on from its boxes: they are gone from its tree.
fn the_boxes_go(engine: &super::Engine) {
    let mut model = super::super::lock(&engine.model);
    let nodes = model.ax.get_mut("PAGE-1").unwrap().as_array_mut().unwrap();
    let wrapper = nodes.iter_mut().find(|n| n["nodeId"] == "3").unwrap();
    wrapper["childIds"]
        .as_array_mut()
        .unwrap()
        .retain(|c| !c.as_str().unwrap().starts_with("10"));
}

#[test]
fn a_code_the_person_types_one_box_at_a_time_stays_out_whole_once_the_boxes_are_gone() {
    let (browser, engine) = browser();
    the_person_types_the_code(&browser, &engine, &[1, 2, 3, 4, 5, 6], false);
    the_page_says(&engine, "PAGE-1", "Your code 739214 is right");
    engine.emit(
        "Target.targetInfoChanged",
        json!({"targetInfo":{"targetId":"PAGE-1","type":"page","title":"Code 739214","url":"http://fixture.test/frames.html?code=739214"}}),
        None,
    );
    std::thread::sleep(Duration::from_millis(30));

    let text = snapshot(&browser, 1);
    let found = browser
        .call(
            &id(2),
            Call::Find {
                page: None,
                text: Some("739214".into()),
                role: None,
                within: None,
            },
        )
        .unwrap();
    let waited = browser
        .call(
            &id(3),
            Call::WaitForText {
                page: None,
                text: "739214".into(),
                gone: false,
                within: Duration::from_millis(10),
            },
        )
        .unwrap();
    let listed = browser.call(&id(4), Call::Pages).unwrap();

    assert!(text.contains("\"Your code (blanked) is right\""), "{text}");
    assert!(
        matches!(&found.content, crate::drive::Content::Text(t) if t.ends_with("Nothing on the page matches.\n")),
        "{found:?}"
    );
    assert_eq!(waited.content, crate::drive::Content::Seen(false));
    let page = listed.page.unwrap();
    assert!(page.address.ends_with("code=(blanked)"), "{page:?}");
    assert_eq!(page.title, "Code (blanked)", "{page:?}");
}

/// The person types "739214" into the page's six boxes, one digit a box, in the boxes `boxes`
/// names in turn (1 to 6), moving on with Tab; each box's tree shows a bullet in place of its
/// digit when `masked`. The page then takes the code and moves on: the boxes are gone before
/// the hand-back.
pub(super) fn the_person_types_the_code(
    browser: &super::Browser,
    engine: &super::Engine,
    boxes: &[usize],
    masked: bool,
) {
    the_page_asks_for_a_code(engine);
    browser.take_over();
    let aim = aim_now(browser);
    for (&at, digit) in boxes.iter().zip("739214".chars()) {
        focus_on(engine, &[("PAGE-1", &format!("10{at}"))]);
        super::super::lock(&engine.model).value = digit.to_string();
        browser
            .person_input(&key(&aim, &digit.to_string(), true))
            .unwrap();
        let shown = if masked {
            "•".to_owned()
        } else {
            digit.to_string()
        };
        the_box_shows(engine, at, &shown);
        // The person moves to the next box with Tab, which ends what they were typing.
        browser.person_input(&key(&aim, "Tab", true)).unwrap();
    }
    the_boxes_go(engine);
    browser.hand_back();
}

#[test]
fn a_code_typed_into_masked_boxes_stays_out_whole() {
    let (browser, engine) = browser();
    the_person_types_the_code(&browser, &engine, &[1, 2, 3, 4, 5, 6], true);
    the_page_says(&engine, "PAGE-1", "Your code 739214 is right");

    let text = snapshot(&browser, 1);

    assert!(text.contains("\"Your code (blanked) is right\""), "{text}");
}

#[test]
fn a_code_typed_into_its_boxes_in_another_order_than_the_page_lists_them_stays_out_whole() {
    let (browser, engine) = browser();
    // The page lists its boxes from the last digit to the first, as one laid out right to left.
    the_person_types_the_code(&browser, &engine, &[6, 5, 4, 3, 2, 1], false);
    the_page_says(&engine, "PAGE-1", "Your code 739214 is right");

    let text = snapshot(&browser, 1);

    assert!(text.contains("\"Your code (blanked) is right\""), "{text}");
}

#[test]
fn a_code_typed_across_several_boxes_stays_out_of_answers_about_another_page() {
    let (browser, engine) = browser();
    the_person_types_the_code(&browser, &engine, &[1, 2, 3, 4, 5, 6], false);
    {
        let mut model = super::super::lock(&engine.model);
        let main = model.ax["PAGE-1"].clone();
        model.ax.insert("PAGE-2".into(), main);
    }
    the_page_says(&engine, "PAGE-2", "Your code 7 3 9 - 2 1 4 is right");

    browser
        .call(
            &id(7),
            Call::Open {
                address: crate::WebAddress::parse("http://fixture.test/other.html").unwrap(),
            },
        )
        .unwrap();
    let text = snapshot(&browser, 8);

    assert!(text.contains("\"Your code (blanked) is right\""), "{text}");
}

/// A browser over a large page (#3043): a read of its whole tree takes `takes`, a key waits
/// `key_waits` for it, and a read may take `read_may_take`.
fn a_large_page(
    takes: Duration,
    key_waits: Duration,
    read_may_take: Duration,
) -> (super::Browser, Arc<super::Engine>) {
    let (browser, engine) = super::browser_with(crate::drive::Limits {
        key_reads_within: key_waits,
        tree_reads_within: read_may_take,
        ..super::limits()
    });
    super::super::lock(&engine.model).ax_takes = Some(takes);
    (browser, engine)
}

#[test]
fn a_key_whose_page_reads_past_its_limit_is_held_and_said_so() {
    // #3043: the page's tree takes longer to read than the key waits for it.
    let (browser, engine) = a_large_page(
        Duration::from_millis(600),
        Duration::from_millis(100),
        Duration::from_secs(30),
    );
    browser.take_over();
    let aim = aim_now(&browser);

    let started = Instant::now();
    let held = browser.person_input(&key(&aim, "x", true));

    assert_eq!(held, Err(Refusal::PageUnreadInTime));
    assert!(
        started.elapsed() < Duration::from_millis(500),
        "the key waited {:?}",
        started.elapsed()
    );
    assert!(
        held.unwrap_err().to_string().contains("held back"),
        "the key is said to be held"
    );
    assert!(engine.sent("Input.dispatchKeyEvent").is_empty());
}

#[test]
fn the_read_a_held_key_left_running_is_what_the_next_key_is_blanked_from() {
    // #3043: the read goes on past the key it was made for, and the next key takes it.
    let (browser, engine) = a_large_page(
        Duration::from_millis(300),
        Duration::from_millis(100),
        Duration::from_secs(30),
    );
    browser.take_over();
    let aim = aim_now(&browser);
    assert_eq!(
        browser.person_input(&key(&aim, "x", true)),
        Err(Refusal::PageUnreadInTime)
    );
    std::thread::sleep(Duration::from_millis(400));

    browser.person_input(&key(&aim, "y", true)).unwrap();

    assert_eq!(page_reads(&engine), 1, "the page was read again");
    assert_eq!(engine.sent("Input.dispatchKeyEvent").len(), 1);
    // The field keeps the focus: the next key reads nothing of the page.
    let field = super::ax_backend_focused(&engine, "PAGE-1");
    super::super::lock(&engine.model).focused_backend = Some(field);
    super::super::lock(&engine.model).value = "held-then-typed".into();
    browser.person_input(&key(&aim, "z", true)).unwrap();
    browser.hand_back();
    let text = snapshot(&browser, 1);
    assert!(!text.contains("held-then-typed"), "{text}");
}

#[test]
fn a_key_whose_page_read_outlived_the_reads_own_limit_is_held_never_unknown() {
    // #3043: the read itself ran past what a read may take, so no key knows the page.
    let (browser, engine) = a_large_page(
        Duration::from_millis(400),
        Duration::from_millis(100),
        Duration::from_millis(150),
    );
    browser.take_over();
    let aim = aim_now(&browser);
    assert_eq!(
        browser.person_input(&key(&aim, "x", true)),
        Err(Refusal::PageUnreadInTime)
    );
    std::thread::sleep(Duration::from_millis(200));

    let next = browser.person_input(&key(&aim, "y", true));

    assert_eq!(next, Err(Refusal::PageUnreadInTime));
    assert!(engine.sent("Input.dispatchKeyEvent").is_empty());
}

#[test]
fn a_key_after_a_press_of_the_mouse_reads_the_page_anew() {
    // #3043: a press may move the focus, so a read made before it is not the key's.
    let (browser, engine) = a_large_page(
        Duration::from_millis(300),
        Duration::from_millis(100),
        Duration::from_secs(30),
    );
    browser.take_over();
    let aim = aim_now(&browser);
    let _ = browser.person_input(&key(&aim, "x", true));
    std::thread::sleep(Duration::from_millis(400));
    browser.person_input(&press(&aim, 5.0, 5.0)).unwrap();

    let _ = browser.person_input(&key(&aim, "y", true));

    assert_eq!(page_reads(&engine), 2, "the page was not read anew");
}
