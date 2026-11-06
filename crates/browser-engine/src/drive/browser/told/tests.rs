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
        filled: kept.clone(),
        named_by_their_sites: Vec::new(),
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
fn a_download_name_as_long_in_bytes_as_the_engine_cuts_one_to_says_nothing_of_it() {
    // Chrome cuts a name by its bytes: 75 letters of three bytes each, well short of purlis's
    // cut in characters, may have left a part of the value.
    let name = format!("{}hunter", "\u{65e5}".repeat(75));
    assert!(name.chars().count() < MOST_FILE_NAME_CHARS);
    let told = scrub().answer(Answer {
        downloads: vec![
            download(&name, "http://a.test"),
            download("report hunter.csv", "http://a.test"),
        ],
        ..answer()
    });
    assert_eq!(told.downloads[0].name, "download");
    assert_eq!(
        told.downloads[1].name, "report hunter.csv",
        "a short name is said"
    );
}

#[test]
fn what_a_page_or_the_engine_gave_a_refusal_has_the_value_taken_out_and_its_own_words_stay() {
    let every = &scrub().every;
    for refusal in [
        Refusal::Engine("net error at http://a.test/?q=hunter-5310".to_owned()),
        Refusal::NotFilled(VALUE.to_owned()),
        Refusal::NotLoggedIn(VALUE.to_owned()),
        Refusal::NoFormOnCleanLoad(VALUE.to_owned()),
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
        filled: Kept::default(),
        named_by_their_sites: Vec::new(),
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

/// #5152: a page on the site the agent is kept off, whose host holds a text kept out, as a login's
/// user name may be part of its site's name.
fn kept_off_acme() -> KeptOff {
    std::sync::Arc::new(|address: &str| {
        address
            .starts_with("https://login.acme.test")
            .then(|| "login.acme.test:443".to_owned())
    })
}

#[test]
fn a_page_kept_off_is_judged_by_its_address_before_a_text_is_taken_out_of_it() {
    let mut kept = Kept::default();
    kept.run(&["acme"]);
    let keeping = Keeping {
        every: Scrub::of(&kept),
        filled: kept.clone(),
        named_by_their_sites: Vec::new(),
        browser: kept,
        pages: Vec::new(),
    };
    let on_it = page("https://login.acme.test/account?id=7", "Your account");

    let told = keeping.answer(kept_off_told(
        Answer {
            page: Some(on_it.clone()),
            opened: vec![on_it.clone()],
            content: Content::Pages(vec![on_it, page("https://a.test/x", "Another")]),
            ..answer()
        },
        &|page: &crate::drive::PageInfo| kept_off_acme()(&page.address),
    ));

    let shown = told.page.expect("a page");
    assert_eq!(shown.address, "https://login.(blanked).test");
    assert!(shown.title.is_empty());
    assert_eq!(told.opened[0].address, "https://login.(blanked).test");
    assert!(told.opened[0].title.is_empty());
    let Content::Pages(pages) = told.content else {
        panic!("pages");
    };
    assert_eq!(pages[0].address, "https://login.(blanked).test");
    assert_eq!(
        pages[1].address, "https://a.test/x",
        "another site as it is"
    );
    assert_eq!(pages[1].title, "Another");
    let note = told.note.expect("the chat is told");
    assert!(note.contains("you may not read or act on it"), "{note}");
    assert!(!note.contains("acme"), "{note}");
}

/// What purlis filled on `https://acme.com`, a login's user name that its site's name holds, and
/// a value it doesn't.
fn user_name_on_its_own_site() -> Keeping {
    let mut state = State::default();
    let mut filled = Kept::default();
    filled.run(&["acme", "hunter-5310"]);
    state.keep_filled_out(&filled, "https://acme.com");
    Keeping {
        every: Scrub::of(&state.kept),
        filled: state.filled.clone(),
        named_by_their_sites: state.named_by_their_sites.clone(),
        browser: state.kept.clone(),
        pages: Vec::new(),
    }
}

#[test]
fn a_site_named_by_what_was_filled_on_it_is_still_said_and_filled() {
    let told = user_name_on_its_own_site().answer(Answer {
        asked: vec!["acme.com:443".to_owned(), "api.acme.com:443".to_owned()],
        page: Some(page("https://acme.com/sign-in", "Sign in")),
        content: Content::Field(FieldInfo {
            origin: Some("https://acme.com".to_owned()),
            label: String::new(),
            kind: FieldKind::OneLine,
        }),
        ..answer()
    });
    assert_eq!(told.asked, ["acme.com:443", "api.acme.com:443"]);
    assert!(told.note.is_none());
    let Content::Field(field) = told.content else {
        panic!("a field");
    };
    assert_eq!(field.origin.as_deref(), Some("https://acme.com"));
    assert_eq!(
        told.page.expect("a page").address,
        "https://acme.com",
        "so the window reads the field as on its page's own site"
    );
}

#[test]
fn a_site_named_by_what_was_filled_elsewhere_stays_unsaid() {
    let told = user_name_on_its_own_site().answer(Answer {
        asked: vec![
            "acme.evil.test:443".to_owned(),
            "acme.com.evil.test:443".to_owned(),
            "notacme.com:443".to_owned(),
            "hunter-5310.acme.com:443".to_owned(),
        ],
        page: Some(page("https://acme.evil.test/", "")),
        content: Content::Field(FieldInfo {
            origin: Some("https://acme.evil.test".to_owned()),
            label: String::new(),
            kind: FieldKind::OneLine,
        }),
        ..answer()
    });
    assert!(told.asked.is_empty(), "{:?}", told.asked);
    assert!(told.note.expect("said").contains("nobody is asked"));
    let Content::Field(field) = told.content else {
        panic!("a field");
    };
    assert_eq!(field.origin, None);
    assert_eq!(
        told.page.expect("a page").address,
        "https://(blanked).evil.test/"
    );
}

#[test]
fn what_the_person_typed_while_driving_hides_a_site_asked_about_and_takes_no_origin_from_a_fill() {
    let mut state = State::default();
    let mut typed = Kept::default();
    typed.run(&["https://", "github.com"]);
    state.keep_out_everywhere(&typed);
    let keeping = Keeping {
        every: Scrub::of(&state.kept),
        filled: state.filled.clone(),
        named_by_their_sites: Vec::new(),
        browser: state.kept.clone(),
        pages: Vec::new(),
    };
    let told = keeping.answer(Answer {
        asked: vec![
            "github.com.evil.test:443".to_owned(),
            "a.test:443".to_owned(),
        ],
        content: Content::Field(FieldInfo {
            origin: Some("https://any.test".to_owned()),
            label: String::new(),
            kind: FieldKind::OneLine,
        }),
        ..answer()
    });
    assert_eq!(told.asked, ["a.test:443"]);
    assert!(told.note.expect("said").contains("nobody is asked"));
    let Content::Field(field) = told.content else {
        panic!("a field");
    };
    assert_eq!(field.origin.as_deref(), Some("https://any.test"));
}

#[test]
fn a_name_cut_where_a_space_falls_is_still_a_name_that_was_cut() {
    let name = crate::drive::plain_file_name(&format!("{} hunter-5310", "a".repeat(119)));
    assert_eq!(name.chars().count(), MOST_FILE_NAME_CHARS);
    assert_eq!(name_shown(&name, &scrub().every), "download");
}
