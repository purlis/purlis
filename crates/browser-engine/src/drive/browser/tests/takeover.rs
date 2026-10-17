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
