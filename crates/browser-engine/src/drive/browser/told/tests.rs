// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::drive::{DownloadState, FieldKind, MOST_FILE_NAME_CHARS};

const VALUE: &str = "hunter-5310";

fn scrub() -> Keeping {
    let mut kept = Kept::default();
    kept.run(&[VALUE]);
    Keeping {
        every: Scrub::of(&kept),
        filled: Scrub::of(&kept),
        browser: kept,
        pages: Vec::new(),
    }
}

fn page(address: &str, title: &str) -> PageInfo {
    PageInfo {
        id: PageId::new("p1"),
        address: address.to_owned(),
        title: title.to_owned(),
        current: true,
    }
}

fn download(name: &str, origin: &str) -> DownloadInfo {
    DownloadInfo {
        id: "d1".to_owned(),
        name: name.to_owned(),
        origin: Some(origin.to_owned()),
        received: 0,
        total: None,
        state: DownloadState::Going,
        file: "guid".to_owned(),
    }
}

fn answer() -> Answer {
    Answer {
        page: None,
        settled: Settled::Yes,
        opened: Vec::new(),
        content: Content::Nothing,
        note: None,
        repeated: false,
        asked: Vec::new(),
        downloads: Vec::new(),
    }
}

#[test]
fn every_string_of_an_answer_has_the_value_taken_out() {
    let told = scrub().answer(Answer {
        page: Some(page("http://a.test/?q=hunter-5310", "hunter-5310 results")),
        settled: Settled::No(vec!["1 request still loading: hunter-5310.test".to_owned()]),
        opened: vec![page("http://a.test/hunter-5310", "")],
        content: Content::Text("said hunter-5310".to_owned()),
        note: Some("a note of hunter-5310".to_owned()),
        repeated: false,
        asked: Vec::new(),
        downloads: vec![download("hunter-5310.txt", "http://hunter-5310.test")],
    });
    let said = format!("{told:?}");
    assert!(!said.contains(VALUE), "{said}");
    assert_eq!(told.downloads[0].name, "(blanked).txt");
}

#[test]
fn a_page_listed_and_a_fields_label_have_the_value_taken_out() {
    let pages = scrub().answer(Answer {
        content: Content::Pages(vec![page("http://a.test/hunter-5310", "hunter-5310")]),
        ..answer()
    });
    assert!(!format!("{pages:?}").contains(VALUE));
    let field = scrub().answer(Answer {
        content: Content::Field(FieldInfo {
            origin: Some("http://a.test".to_owned()),
            label: "hunter-5310".to_owned(),
            kind: FieldKind::OneLine,
        }),
        ..answer()
    });
    let Content::Field(field) = field.content else {
        panic!("a field");
    };
    assert_eq!(field.label, "(blanked)");
    assert_eq!(field.origin.as_deref(), Some("http://a.test"));
}

#[test]
fn an_origin_that_holds_the_value_is_one_the_engine_cannot_name() {
    let field = scrub().answer(Answer {
        content: Content::Field(FieldInfo {
            origin: Some("http://hunter-5310.test".to_owned()),
            label: String::new(),
            kind: FieldKind::OneLine,
        }),
        ..answer()
    });
    let Content::Field(field) = field.content else {
        panic!("a field");
    };
    assert_eq!(field.origin, None);
}

#[test]
fn a_site_asked_about_that_holds_the_value_is_not_said_and_the_call_says_so() {
    let told = scrub().answer(Answer {
        asked: vec!["hunter-5310.test:443".to_owned(), "a.test:443".to_owned()],
        ..answer()
    });
    assert_eq!(told.asked, vec!["a.test:443".to_owned()]);
    let note = told.note.expect("the call says so");
    assert!(note.contains("nobody is asked"), "{note}");
    assert!(!note.contains(VALUE), "{note}");
}

#[test]
fn a_download_name_as_long_as_one_is_cut_to_says_nothing_of_it() {
    // What is cut may have left a part of the value, which no scrub knows.
    let name = format!("{}hunter", "x".repeat(MOST_FILE_NAME_CHARS - 6));
    let told = scrub().answer(Answer {
        downloads: vec![download(&name, "http://a.test")],
        ..answer()
    });
    assert_eq!(told.downloads[0].name, "download");
}

#[test]
fn what_a_page_or_the_engine_gave_a_refusal_has_the_value_taken_out_and_its_own_words_stay() {
    let every = &scrub().every;
    for refusal in [
        Refusal::Engine("net error at http://a.test/?q=hunter-5310".to_owned()),
        Refusal::NotFilled(VALUE.to_owned()),
        Refusal::NotLoggedIn(VALUE.to_owned()),
        Refusal::Withheld(VALUE.to_owned()),
        Refusal::Partly {
            done: VALUE.to_owned(),
            why: VALUE.to_owned(),
        },
        Refusal::NoSuchOption {
            target: crate::drive::Ref::new("e1"),
            missing: "another".to_owned(),
            options: VALUE.to_owned(),
        },
        Refusal::KeptOff {
            site: "hunter-5310.test".to_owned(),
            asks_to_sign_in: false,
        },
    ] {
        let said = refusal_kept_out(refusal, every);
        assert!(!format!("{said:?}").contains(VALUE), "{said:?}");
        assert!(!said.to_string().contains(VALUE), "{said}");
    }
    assert_eq!(
        refusal_kept_out(Refusal::Ended, every).to_string(),
        Refusal::Ended.to_string()
    );
}

#[test]
fn what_the_chat_wrote_is_said_back_as_written_so_a_guess_is_told_nothing() {
    let every = &scrub().every;
    for said_back in [
        Refusal::NoSuchRef(crate::drive::Ref::new(VALUE)),
        Refusal::NoSuchPage(PageId::new(VALUE)),
        Refusal::NotAKey(VALUE.to_owned()),
        Refusal::IdReused(VALUE.to_owned()),
        Refusal::NoSuchOption {
            target: crate::drive::Ref::new(VALUE),
            missing: VALUE.to_owned(),
            options: String::new(),
        },
    ] {
        let as_written = said_back.to_string();
        assert_eq!(refusal_kept_out(said_back, every).to_string(), as_written);
    }
}

#[test]
fn what_the_person_typed_on_a_page_does_not_hide_a_site_that_has_it_in_its_name() {
    let mut typed = Kept::default();
    typed.run(&["github"]);
    let keeping = Keeping {
        every: Scrub::of(&typed),
        filled: Scrub::of(&Kept::default()),
        browser: Kept::default(),
        pages: Vec::new(),
    };
    let told = keeping.answer(Answer {
        asked: vec!["api.github.com:443".to_owned()],
        content: Content::Field(FieldInfo {
            origin: Some("https://github.com".to_owned()),
            label: String::new(),
            kind: FieldKind::OneLine,
        }),
        ..answer()
    });
    assert_eq!(told.asked, vec!["api.github.com:443".to_owned()]);
    let Content::Field(field) = told.content else {
        panic!("a field");
    };
    assert_eq!(field.origin.as_deref(), Some("https://github.com"));
}
