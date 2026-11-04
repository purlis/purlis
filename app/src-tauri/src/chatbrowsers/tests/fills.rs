// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A secret a chat asks purlis to fill (#5115), at the window's browser-call seam with the
//! session host stood in: it goes into the browser the chat already uses, the person is asked
//! about the origin the browser read for the field's own frame, and nothing is filled before
//! they allow it, or at all once they keep it blocked. A 2FA code is asked for as one (#5118).
//! What was filled is marked on its browser by key and site, and nothing goes into a browser
//! the person has taken over (#5119).

use std::cell::RefCell;

use purlis_session_protocol::browsing::{FieldKind, FieldSaid};

use super::*;
use crate::chatbrowsers::SecretFilled;
use crate::secretfills::FillAsked;

const VALUE: &str = "fixture-key-5115";

fn fill(target: &str) -> Browsing {
    Browsing::FillSecret {
        target: target.into(),
        vault: "qa".into(),
        key: "dash-api-key".into(),
        code: false,
    }
}

fn one_line_on(origin: &str) -> FieldSaid {
    FieldSaid {
        origin: Some(origin.into()),
        label: "API key".into(),
        kind: FieldKind::OneLine,
    }
}

/// `browse` answered with `fills` as what asks the person, on the runner of `pool` where one is
/// named.
fn answered_filling(
    browsers: &ChatBrowsers,
    hosted: &Arc<Hosted>,
    browse: &BrowseAsk,
    pool: Option<&str>,
    fills: super::super::Fills<'_>,
) -> Answer {
    let driver: Arc<dyn Driver> = hosted.clone();
    browsers
        .answer(
            Some(&driver),
            Path::new("/p"),
            pool,
            browse,
            &|| reach(),
            false,
            &AsPersona::default,
            &logins,
            fills,
            &|| Err("no folder here".to_owned()),
            1000,
        )
        .0
}

/// Chat 3 allowed a browser, and opened a page in it: `b1`, `program-1`.
fn using_a_browser() -> (ChatBrowsers, Arc<Hosted>) {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    answered(&browsers, &hosted, &ask(3, click()));
    assert!(browsers.allow(3, false));
    let (answer, _) = answered(&browsers, &hosted, &ask(3, click()));
    assert!(matches!(answer, Answer::Browsed { .. }), "{answer:?}");
    *hosted.at.lock().unwrap() = Some("https://dash.example.test/settings".into());
    (browsers, hosted)
}

/// The person allows once: the value, as the vault gave it.
fn allowed(asked: &RefCell<Vec<FillAsked>>) -> impl Fn(&FillAsked) -> Result<Secret, String> + '_ {
    move |one: &FillAsked| {
        asked.borrow_mut().push(one.clone());
        Ok(Secret::new(VALUE.into()))
    }
}

#[test]
fn a_secret_allowed_once_goes_into_the_browser_the_chat_already_uses() {
    let (browsers, hosted) = using_a_browser();
    *hosted.field.lock().unwrap() = Some(one_line_on("https://dash.example.test"));
    let asked = RefCell::new(Vec::new());

    let answer = answered_filling(
        &browsers,
        &hosted,
        &ask(3, fill("e12")),
        None,
        &allowed(&asked),
    );

    match answer {
        Answer::Browsed { browser, browsed } => {
            assert_eq!(browser, "b1");
            assert!(!format!("{browsed:?}").contains(VALUE));
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(hosted.started.lock().unwrap().len(), 1, "no new browser");
    // The person was asked about what the browser read of the field, on its page.
    let asked = asked.into_inner();
    assert_eq!(asked.len(), 1);
    assert_eq!(asked[0].browser, "b1");
    assert_eq!(asked[0].field, one_line_on("https://dash.example.test"));
    assert_eq!(
        asked[0].page.as_deref(),
        Some("https://dash.example.test/settings")
    );
    let fills = hosted.fills.lock().unwrap();
    assert_eq!(fills.len(), 1);
    let (program, filled) = &fills[0];
    assert_eq!(program, "program-1");
    assert_eq!(filled.target, "e12");
    assert_eq!(filled.origin, "https://dash.example.test");
    assert_eq!(filled.key, "dash-api-key");
    assert_eq!(filled.value.expose(), VALUE);
}

#[test]
fn the_origin_filled_on_is_the_one_the_browser_read_for_the_fields_own_frame() {
    let (browsers, hosted) = using_a_browser();
    // The page is on dash.example.test; the field's frame is not.
    *hosted.field.lock().unwrap() = Some(one_line_on("https://pay.example.test"));
    let asked = RefCell::new(Vec::new());

    answered_filling(
        &browsers,
        &hosted,
        &ask(3, fill("e12")),
        None,
        &allowed(&asked),
    );

    assert_eq!(
        asked.into_inner()[0].field.origin.as_deref(),
        Some("https://pay.example.test")
    );
    assert_eq!(
        hosted.fills.lock().unwrap()[0].1.origin,
        "https://pay.example.test",
        "the engine is held to the origin the person was shown"
    );
}

#[test]
fn a_2fa_code_is_asked_for_as_one_and_only_what_the_vault_side_made_reaches_the_browser() {
    let (browsers, hosted) = using_a_browser();
    *hosted.field.lock().unwrap() = Some(one_line_on("https://dash.example.test"));
    let asked = RefCell::new(Vec::new());
    // The vault's side: the code its seed makes now, never the seed.
    let made = |one: &FillAsked| -> Result<Secret, String> {
        asked.borrow_mut().push(one.clone());
        Ok(Secret::new("287082".into()))
    };

    let answer = answered_filling(
        &browsers,
        &hosted,
        &ask(
            3,
            Browsing::FillSecret {
                target: "e7".into(),
                vault: "qa".into(),
                key: "dash-2fa".into(),
                code: true,
            },
        ),
        None,
        &made,
    );

    assert!(matches!(answer, Answer::Browsed { .. }), "{answer:?}");
    let asked = asked.into_inner();
    assert!(asked[0].code, "the person is asked about a code");
    assert_eq!(asked[0].key, "dash-2fa");
    let fills = hosted.fills.lock().unwrap();
    assert_eq!(fills[0].1.value.expose(), "287082");
    assert_eq!(fills[0].1.key, "dash-2fa");
}

#[test]
fn a_secret_the_person_keeps_blocked_fills_nothing_and_says_so() {
    let (browsers, hosted) = using_a_browser();
    *hosted.field.lock().unwrap() = Some(one_line_on("https://dash.example.test"));
    let kept = |_: &FillAsked| -> Result<Secret, String> {
        Err(crate::secretfills::kept_said(
            "dash-api-key",
            "https://dash.example.test",
            None,
        ))
    };

    let told = refusal(answered_filling(
        &browsers,
        &hosted,
        &ask(3, fill("e12")),
        None,
        &kept,
    ));

    assert!(told.contains("kept dash-api-key blocked"), "{told}");
    assert!(told.contains("nothing was filled"), "{told}");
    assert!(hosted.fills.lock().unwrap().is_empty());
}

#[test]
fn a_field_the_browser_cannot_read_asks_nobody_and_fills_nothing() {
    let (browsers, hosted) = using_a_browser();
    let asked = RefCell::new(Vec::new());

    let told = refusal(answered_filling(
        &browsers,
        &hosted,
        &ask(3, fill("e12")),
        None,
        &allowed(&asked),
    ));

    assert!(told.contains("take a new snapshot"), "{told}");
    assert!(asked.into_inner().is_empty(), "nobody was asked");
    assert!(hosted.fills.lock().unwrap().is_empty());
}

#[test]
fn a_chat_with_no_browser_open_is_told_to_open_one_and_none_starts() {
    let browsers = ChatBrowsers::default();
    let hosted = Arc::new(Hosted::default());
    answered(&browsers, &hosted, &ask(3, click()));
    assert!(browsers.allow(3, false));
    let asked = RefCell::new(Vec::new());

    let told = refusal(answered_filling(
        &browsers,
        &hosted,
        &ask(3, fill("e12")),
        None,
        &allowed(&asked),
    ));

    assert!(told.contains("no browser open"), "{told}");
    assert!(
        hosted.started.lock().unwrap().is_empty(),
        "no browser started"
    );
    assert!(asked.into_inner().is_empty());
}

#[test]
fn a_chat_cannot_read_a_field_for_a_fill_itself() {
    let (browsers, hosted) = using_a_browser();
    let acts = hosted.acts.lock().unwrap().len();

    let told = refusal(
        answered(
            &browsers,
            &hosted,
            &ask(
                3,
                Browsing::Act(Act::DescribeField {
                    target: "e12".into(),
                }),
            ),
        )
        .0,
    );

    assert!(told.contains("no browser tool"), "{told}");
    assert_eq!(
        hosted.acts.lock().unwrap().len(),
        acts,
        "nothing reached the browser"
    );
}

#[test]
fn a_fill_on_a_runners_browser_is_refused_for_now() {
    let (browsers, hosted) = using_a_browser();
    *hosted.field.lock().unwrap() = Some(one_line_on("https://dash.example.test"));
    let asked = RefCell::new(Vec::new());

    let told = refusal(answered_filling(
        &browsers,
        &hosted,
        &ask(3, fill("e12")),
        Some("relayed"),
        &allowed(&asked),
    ));

    assert!(told.contains("pool relayed"), "{told}");
    assert!(asked.into_inner().is_empty());
    assert!(hosted.fills.lock().unwrap().is_empty());
}

/// What chat 3's browser `b1` lists of the secrets filled into it.
fn marks(browsers: &ChatBrowsers) -> Vec<SecretFilled> {
    browsers
        .listed()
        .into_iter()
        .find(|one| one.program == "program-1")
        .expect("listed")
        .secrets
}

#[test]
fn a_filled_field_is_marked_on_its_browser_by_key_site_and_label_never_value() {
    let (browsers, hosted) = using_a_browser();
    *hosted.field.lock().unwrap() = Some(one_line_on("https://dash.example.test"));
    let asked = RefCell::new(Vec::new());
    assert!(marks(&browsers).is_empty());

    answered_filling(
        &browsers,
        &hosted,
        &ask(3, fill("e12")),
        None,
        &allowed(&asked),
    );

    let marked = marks(&browsers);
    assert_eq!(
        marked,
        [SecretFilled {
            key: "dash-api-key".into(),
            origin: "https://dash.example.test".into(),
            label: "API key".into(),
        }]
    );
    assert!(!format!("{:?}", browsers.listed()).contains(VALUE));

    // The same key filled again on that site is marked once.
    answered_filling(
        &browsers,
        &hosted,
        &ask(3, fill("e12")),
        None,
        &allowed(&asked),
    );
    assert_eq!(marks(&browsers).len(), 1);
}

#[test]
fn a_fill_refused_marks_nothing() {
    let (browsers, hosted) = using_a_browser();
    *hosted.field.lock().unwrap() = Some(one_line_on("https://dash.example.test"));
    let kept = |_: &FillAsked| -> Result<Secret, String> { Err("kept".into()) };

    answered_filling(&browsers, &hosted, &ask(3, fill("e12")), None, &kept);

    assert!(marks(&browsers).is_empty());
}

#[test]
fn the_activity_line_names_the_key_and_the_site_it_went_to() {
    let (browsers, hosted) = using_a_browser();
    *hosted.field.lock().unwrap() = Some(one_line_on("https://dash.example.test"));
    let asked = RefCell::new(Vec::new());
    let browse = ask(3, fill("e12"));

    let answer = answered_filling(&browsers, &hosted, &browse, None, &allowed(&asked));

    let line = super::super::timeline_of(&browsers, &browse, &answer, 5).expect("a moment");
    assert_eq!(
        line.moment.did,
        "purlis filled dash-api-key into e12 on https://dash.example.test"
    );
    assert!(!line.moment.did.contains(VALUE));
}

#[test]
fn a_browser_the_person_has_taken_over_is_filled_nothing_and_the_chat_told_why() {
    let (browsers, hosted) = using_a_browser();
    *hosted.field.lock().unwrap() = Some(one_line_on("https://dash.example.test"));
    browsers.driven("program-1", true, 1000);
    let acts = hosted.acts.lock().unwrap().len();
    let asked = RefCell::new(Vec::new());

    let told = refusal(answered_filling(
        &browsers,
        &hosted,
        &ask(3, fill("e12")),
        None,
        &allowed(&asked),
    ));

    assert!(told.contains("taken b1 over"), "{told}");
    assert!(told.contains("nothing was filled"), "{told}");
    assert!(asked.into_inner().is_empty(), "nobody was asked");
    assert_eq!(
        hosted.acts.lock().unwrap().len(),
        acts,
        "not even the field was read"
    );
    assert!(hosted.fills.lock().unwrap().is_empty());
}

#[test]
fn a_browser_taken_over_while_the_person_is_asked_is_filled_nothing() {
    let (browsers, hosted) = using_a_browser();
    *hosted.field.lock().unwrap() = Some(one_line_on("https://dash.example.test"));
    // The person takes the browser over, then allows the fill.
    let taken_then_allowed = |_: &FillAsked| -> Result<Secret, String> {
        browsers.driven("program-1", true, 1001);
        Ok(Secret::new(VALUE.into()))
    };

    let told = refusal(answered_filling(
        &browsers,
        &hosted,
        &ask(3, fill("e12")),
        None,
        &taken_then_allowed,
    ));

    assert!(told.contains("taken b1 over"), "{told}");
    assert!(hosted.fills.lock().unwrap().is_empty());
    assert!(marks(&browsers).is_empty());
}
