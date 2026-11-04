// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A secret purlis fills (#5115, spec #5100), through a real engine on the fixture site: one
//! vault value goes into a one-line field of a browser the agent is already using, only where
//! the field's own frame is on the origin the person approved at the moment of filling, and
//! nothing the agent reads after shows it: not a snapshot, a find, a wait or a screenshot, not
//! even where the page echoes it.
//!
//! These need a real Chrome and the right to start it and to listen on a loopback port (the
//! site's stand-in reach proxy), which a sandboxed agent does not have. They run where
//! `PURLIS_TEST_ENGINE` names a Chrome executable, and on CI against the runner's own Chrome.
//! Anywhere else each test says why it did not run and passes.

#![cfg(unix)]

use crate::common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use common::drive::{call, find, open, red_pixels, ref_on, screenshot, snapshot};
use common::site::started;
use purlis_browser_engine::drive::{
    Answer, Browser, Call, Content, FieldInfo, FieldKind, FillHold, Ref, Refusal, SecretFill,
};

const VALUE: &str = "fixture-key-5115";

fn fill_of(target: Ref, origin: &str) -> SecretFill {
    SecretFill {
        target,
        origin: origin.to_owned(),
        key: "dash-api-key".to_owned(),
        value: VALUE.to_owned().into(),
    }
}

/// What holds the browser to the field's site while its page lives (#5117): here, nothing.
struct Holds;

impl FillHold for Holds {
    fn hold(&self) -> Result<(), String> {
        Ok(())
    }

    fn released(&self) {}
}

fn filled(browser: &Browser, target: Ref, origin: &str) -> Result<Answer, Refusal> {
    browser.fill_secret(browser.epoch(), &fill_of(target, origin), Arc::new(Holds))
}

/// How a fill ended, in words fixed in this file: a refusal may carry what its page showed, so
/// a failing test names only its kind.
fn ended_as<T>(outcome: &Result<T, Refusal>) -> &'static str {
    match outcome {
        Ok(_) => "it filled",
        Err(Refusal::NotFilled(_)) => "not filled",
        Err(Refusal::Ended) => "the browser ended",
        Err(_) => "another refusal",
    }
}

fn holds_the_value(text: &str) -> bool {
    text.to_lowercase().contains(VALUE)
}

fn described(browser: &Browser, target: Ref) -> FieldInfo {
    match call(browser, Call::DescribeField { target }).content {
        Content::Field(field) => field,
        other => panic!("a field is described: {other:?}"),
    }
}

/// The reference of the element labelled `label`, waiting for it to show.
fn ref_of(browser: &Browser, line: &str) -> Ref {
    let label = line.split('"').nth(1).unwrap_or(line);
    let start = Instant::now();
    loop {
        let found = find(browser, label);
        if found.lines().any(|l| l.contains(line) && l.contains('[')) {
            return ref_on(&found, line);
        }
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "{label} never showed: {found}"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[test]
fn a_secret_goes_into_a_one_line_field_and_nothing_read_after_shows_it() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    open(&browser, "fill.html");
    let key = ref_of(&browser, "textbox \"API key\"");
    assert_eq!(
        described(&browser, key.clone()),
        FieldInfo {
            origin: Some("http://fixture.test".to_owned()),
            label: "API key".to_owned(),
            kind: FieldKind::OneLine,
        }
    );
    let answer = filled(&browser, key, "http://fixture.test");
    let answer = answer.unwrap_or_else(|why| panic!("filled: {}", ended_as(&Err::<(), _>(why))));
    let note = answer.note.unwrap_or_default();
    assert!(note.contains("dash-api-key"), "{note}");
    assert!(!holds_the_value(&note), "the answer names no value");
    // The page received it whole.
    let shown = snapshot(&browser);
    assert!(
        shown.contains(&format!("The key holds {} characters", VALUE.len())),
        "the page has the value"
    );
    // And nothing the agent reads shows it, though the page echoes it.
    assert!(!holds_the_value(&shown), "a snapshot shows the value");
    assert!(
        !find(&browser, VALUE).contains('['),
        "a find finds the value"
    );
    let seen = call(
        &browser,
        Call::WaitForText {
            page: None,
            text: VALUE.into(),
            gone: false,
            within: Duration::from_secs(2),
        },
    );
    assert_eq!(seen.content, Content::Seen(false));
    // What the page draws in red is the field and its echo: both are painted over.
    assert_eq!(red_pixels(&screenshot(&browser)), 0);
}

#[test]
fn a_field_whose_frame_is_on_another_origin_is_refused_and_says_why() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    open(&browser, "fill-frame.html");
    let card = ref_of(&browser, "textbox \"Card number\"");
    assert_eq!(
        described(&browser, card.clone()).origin.as_deref(),
        Some("http://other.test"),
        "the field's own frame is judged, never its page's"
    );
    // The page is on the approved origin; the field's frame is not.
    match filled(&browser, card, "http://fixture.test") {
        Err(Refusal::NotFilled(why)) => {
            assert!(why.contains("http://other.test"), "{why}");
            assert!(why.contains("nothing was filled"), "{why}");
        }
        other => panic!("refused otherwise: {}", ended_as(&other)),
    }
}

#[test]
fn a_multi_line_box_or_an_editable_area_takes_no_secret() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    open(&browser, "fill.html");
    for (label, kind) in [
        ("textbox \"Notes\"", FieldKind::MultiLine),
        ("textbox \"Editor\"", FieldKind::Editable),
    ] {
        let field = ref_of(&browser, label);
        assert_eq!(described(&browser, field.clone()).kind, kind, "{label}");
        match filled(&browser, field, "http://fixture.test") {
            Err(Refusal::NotFilled(why)) => assert!(why.contains("one-line"), "{why}"),
            other => panic!("{label} refused otherwise: {}", ended_as(&other)),
        }
    }
    let shown = snapshot(&browser);
    assert!(!holds_the_value(&shown), "nothing went in");
    assert!(shown.contains("Draft"), "the editable area is as it was");
}

#[test]
fn a_fill_on_an_origin_nobody_approved_fills_nothing() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    open(&browser, "fill.html");
    let key = ref_of(&browser, "textbox \"API key\"");
    match filled(&browser, key, "https://fixture.test") {
        Err(Refusal::NotFilled(why)) => assert!(why.contains("nothing was filled"), "{why}"),
        other => panic!("refused otherwise: {}", ended_as(&other)),
    }
    assert!(
        snapshot(&browser).contains("The key holds 0 characters"),
        "the field holds nothing"
    );
}

#[test]
fn a_number_field_given_letters_keeps_nothing_and_the_browser_stays() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    open(&browser, "fill.html");
    let port = ref_of(&browser, "spinbutton \"Port\"");
    match filled(&browser, port, "http://fixture.test") {
        Err(Refusal::NotFilled(why)) => {
            assert!(why.contains("did not keep the value whole"), "{why}")
        }
        other => panic!("refused otherwise: {}", ended_as(&other)),
    }
    // The browser still answers.
    assert!(!holds_the_value(&snapshot(&browser)));
}

#[test]
fn once_a_secret_is_filled_the_agent_cannot_copy_it_out() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    open(&browser, "fill.html");
    let key = ref_of(&browser, "textbox \"API key\"");
    filled(&browser, key.clone(), "http://fixture.test")
        .unwrap_or_else(|why| panic!("filled: {}", ended_as(&Err::<(), _>(why))));
    call(&browser, Call::Click { target: key });
    let copied = browser.call(
        &common::drive::next_id(),
        Call::Press {
            page: None,
            keys: vec!["Control+a".into(), "Control+c".into()],
        },
    );
    assert_eq!(copied.err(), Some(Refusal::CopyWithheld));
}

/// **A filled field takes no keys from the chat** (#5115): on a page that shows what its field
/// holds, a Backspace pressed into the filled field would leave the value cut short on show, so
/// the press is refused, the field keeps the value whole, and nothing of it shows.
#[test]
fn a_backspace_into_the_filled_field_is_refused_and_nothing_of_the_value_shows() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    open(&browser, "fill.html");
    let key = ref_of(&browser, "textbox \"API key\"");
    filled(&browser, key.clone(), "http://fixture.test")
        .unwrap_or_else(|why| panic!("filled: {}", ended_as(&Err::<(), _>(why))));
    call(&browser, Call::Click { target: key });

    let pressed = browser.call(
        &common::drive::next_id(),
        Call::Press {
            page: None,
            keys: vec!["Backspace".into()],
        },
    );

    assert_eq!(pressed.err(), Some(Refusal::KeysWithheld));
    let shown = snapshot(&browser);
    assert!(
        shown.contains(&format!("The key holds {} characters", VALUE.len())),
        "the field keeps the value whole"
    );
    let cut_short = &VALUE[..VALUE.len() - 1];
    assert!(
        !shown.to_lowercase().contains(cut_short),
        "the page's echo shows none of the value"
    );
    assert_eq!(red_pixels(&screenshot(&browser)), 0);
}
