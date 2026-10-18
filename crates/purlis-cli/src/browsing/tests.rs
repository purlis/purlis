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

fn browsed(text: Option<&str>) -> Reply {
    Reply::from(Answer::Browsed {
        browser: "b1".into(),
        browsed: Box::new(Browsed {
            page: Some(purlis_core::browsertools::PageSaid {
                page: "p2".into(),
                address: "https://docs.example.com/".into(),
                title: "Docs".into(),
                current: true,
            }),
            settled: true,
            text: text.map(str::to_owned),
            ..Browsed::default()
        }),
    })
}

#[test]
fn an_act_asked_for_its_changes_asks_them_of_the_browser_and_page_it_answered_with() {
    // #2137: the browser the app answered with, and the page the act was on.
    assert_eq!(
        super::changes_of(&Ok(Ok(browsed(None)))),
        Some(("b1".to_owned(), "p2".to_owned()))
    );
    let refused = Reply::from(Answer::No {
        why: "no such element".into(),
    });
    assert_eq!(super::changes_of(&Ok(Ok(refused))), None);
    assert_eq!(super::changes_of(&Err(NotTaken::AppGone)), None);
}

#[test]
fn what_changed_follows_the_acts_own_answer() {
    let said = what_the_app_said(Ok(Ok(browsed(None))));
    let changed = "Changed since the page was last read: 1 new, 0 gone.\nnew: - text \"Saved\" [e9] (line 4)\n";
    let (text, _) = super::with_changes(said, Ok(Ok(browsed(Some(changed)))));
    let text = text.unwrap();
    assert!(
        text.starts_with("Browser b1, page p2: \"Docs\" at https://docs.example.com/\n\n"),
        "{text}"
    );
    assert!(text.ends_with(changed), "{text}");
}

#[test]
fn changes_that_cannot_be_read_say_to_take_a_snapshot_and_keep_the_acts_answer() {
    let said = what_the_app_said(Ok(Ok(browsed(None))));
    let refused = Reply::from(Answer::No {
        why: "the browser has ended".into(),
    });
    let (text, _) = super::with_changes(said, Ok(Ok(refused)));
    let text = text.unwrap();
    assert!(text.starts_with("Browser b1, page p2"), "{text}");
    assert!(
        text.ends_with("What changed could not be read (the browser has ended): take a snapshot."),
        "{text}"
    );
}
