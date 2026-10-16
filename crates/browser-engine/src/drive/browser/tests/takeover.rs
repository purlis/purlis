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
fn aim_now(browser: &super::Browser) -> Aim {
    browser.aim().expect("the browser has a current page")
}

fn key(aim: &Aim, key: &str, down: bool) -> PersonInput {
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
