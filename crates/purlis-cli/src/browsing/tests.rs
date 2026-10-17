// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use purlis_core::browsertools::Browsed;
use purlis_core::hookwire::Answer;
use purlis_core::hookwire::framed::{Attached, Reply};

use super::what_the_app_said;
use crate::brokered::NotTaken;

#[test]
fn a_browsers_answer_is_said_in_words_with_its_screenshot_attached_as_an_image() {
    let reply = Reply::from(Answer::Browsed {
        browser: "b1".into(),
        browsed: Box::new(Browsed {
            settled: true,
            png: Some(vec![1, 2, 3]),
            ..Browsed::default()
        }),
    });

    let (said, attached) = what_the_app_said(Ok(Ok(reply)));

    assert_eq!(said, Ok("Browser b1, no page open".to_owned()));
    assert_eq!(
        attached,
        [Attached {
            media_type: "image/png".into(),
            bytes: vec![1, 2, 3],
        }]
    );
}

#[test]
fn a_refusal_is_the_apps_own_words() {
    let reply = Reply::from(Answer::No {
        why: "purlis asked the person what this chat's browser may reach".into(),
    });

    let (said, attached) = what_the_app_said(Ok(Ok(reply)));

    assert_eq!(
        said,
        Err("purlis asked the person what this chat's browser may reach".to_owned())
    );
    assert!(attached.is_empty());
}

#[test]
fn with_no_app_to_ask_there_is_no_browser_and_the_tool_says_so() {
    for not in [
        NotTaken::NotAChat,
        NotTaken::AppGone,
        NotTaken::NoConnection,
        NotTaken::Dropped,
    ] {
        let (said, _) = what_the_app_said(Err(not));
        let why = said.unwrap_err();
        assert!(why.contains("browser"), "{why}");
    }
}

#[test]
fn a_browser_question_answers_with_the_apps_words_alone() {
    // #2015: what the app says the person was asked, and nothing of a page.
    let reply = Reply::from(Answer::Browsed {
        browser: "b1".into(),
        browsed: Box::new(Browsed {
            settled: true,
            note: Some("purlis asked the person to take browser b1 over".into()),
            ..Browsed::default()
        }),
    });

    let said = super::the_person_was_asked(Ok(Ok(reply)));

    assert_eq!(
        said,
        Ok("purlis asked the person to take browser b1 over".to_owned())
    );
    let refused = super::the_person_was_asked(Ok(Ok(Reply::from(Answer::No {
        why: "this chat has no browser b9".into(),
    }))));
    assert_eq!(refused, Err("this chat has no browser b9".to_owned()));
    let gone = super::the_person_was_asked(Err(NotTaken::AppGone));
    assert!(gone.unwrap_err().contains("browser"));
}
