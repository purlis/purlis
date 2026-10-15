// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

use super::{EngineBrowsers, KEEPER, browsers_in, engines_in};

#[test]
fn engines_and_browsers_live_apart_under_the_data_home() {
    let data = Path::new("/home/me/.local/share/purlis");
    assert_eq!(engines_in(data), data.join("engines/chrome-for-testing"));
    assert_eq!(browsers_in(data), data.join("browsers"));
}

#[test]
fn the_keeper_is_this_binary_started_in_its_keepers_part() {
    let keeper = EngineBrowsers::keeper("/bin/purlis".into());
    assert_eq!(keeper.program, Path::new("/bin/purlis"));
    assert_eq!(keeper.args, [std::ffi::OsString::from(KEEPER)]);
}

#[test]
fn each_browsers_data_folder_is_made_under_the_data_home_it_was_given() {
    use purlis_host::Browsers;
    let browsers = EngineBrowsers::new("/home/me/.local/share/purlis".into(), "/bin/purlis".into());
    assert_eq!(
        browsers.data_folders().unwrap(),
        Path::new("/home/me/.local/share/purlis/browsers")
    );
}

// ---- A chat's acts, its answers and its reach (#2013) ----

use std::time::Duration;

use purlis_browser_engine::drive::{
    Answer, Call, Content, DragTo, Field, PageId, PageInfo, Ref, Settled,
};
use purlis_host::protocol::browsing::{self as wire, Act, Browsed, BrowserReach, PageSaid};
use purlis_host::protocol::session::{Layer, Listed};

use super::{browsed, call_of, reach_of};
use crate::sandbox::browser_reach::NewSite;

#[test]
fn the_acts_a_chat_asks_become_the_drivers_own_calls() {
    assert_eq!(
        call_of(Act::Click {
            target: "e3".into()
        }),
        Ok(Call::Click {
            target: Ref::new("e3")
        })
    );
    assert_eq!(
        call_of(Act::Snapshot {
            page: Some("p2".into()),
            within: None,
            from: None,
            lines: Some(50),
        }),
        Ok(Call::Snapshot {
            page: Some(PageId::new("p2")),
            within: None,
            from: 1,
            lines: Some(50),
        }),
        "a snapshot with no start reads from the first line"
    );
    assert_eq!(
        call_of(Act::WaitForText {
            page: None,
            text: "Saved".into(),
            gone: true,
            within_ms: 2500,
        }),
        Ok(Call::WaitForText {
            page: None,
            text: "Saved".into(),
            gone: true,
            within: Duration::from_millis(2500),
        })
    );
    assert_eq!(
        call_of(Act::Drag {
            from: "e1".into(),
            to: wire::DragTo::Offset { x: 40, y: -8 },
        }),
        Ok(Call::Drag {
            from: Ref::new("e1"),
            to: DragTo::Offset { x: 40.0, y: -8.0 },
        })
    );
    assert_eq!(
        call_of(Act::Type {
            page: None,
            fields: vec![wire::Typed {
                into: None,
                text: "hi".into(),
                replace: true,
            }],
        }),
        Ok(Call::Type {
            page: None,
            fields: vec![Field {
                into: None,
                text: "hi".into(),
                replace: true,
            }],
        })
    );
}

#[test]
fn an_address_that_is_not_a_web_address_is_refused_in_words_before_the_driver_hears_it() {
    let refused = call_of(Act::Open {
        address: "file:///etc/passwd".into(),
    })
    .unwrap_err();

    assert!(refused.contains("http and https"), "{refused}");
}

#[test]
fn the_drivers_answer_is_said_back_whole() {
    let page = PageInfo {
        id: PageId::new("p1"),
        address: "https://docs.example.com/".into(),
        title: "Docs".into(),
        current: true,
    };
    let answer = Answer {
        page: Some(page.clone()),
        settled: Settled::No(vec!["a request still open".into()]),
        opened: vec![page],
        content: Content::Text("- heading \"Docs\" [e1]".into()),
        note: Some("the page did not load".into()),
        repeated: true,
    };

    let said = PageSaid {
        page: "p1".into(),
        address: "https://docs.example.com/".into(),
        title: "Docs".into(),
        current: true,
    };
    assert_eq!(
        browsed(answer, usize::MAX),
        Browsed {
            page: Some(said.clone()),
            settled: false,
            waiting_for: vec!["a request still open".into()],
            opened: vec![said],
            text: Some("- heading \"Docs\" [e1]".into()),
            note: Some("the page did not load".into()),
            repeated: true,
            ..Browsed::default()
        }
    );
}

#[test]
fn a_browser_reaches_what_its_chat_lists_and_a_new_site_only_on_the_open_web() {
    let listed = BrowserReach {
        listed: vec![Listed {
            host: "docs.example.com".into(),
            layer: Layer::Open,
        }],
        never: Vec::new(),
        open_web: false,
    };

    let (reach, new_sites) = reach_of(Some(&listed)).unwrap();
    assert_eq!(reach.hosts(), ["docs.example.com"]);
    assert!(!new_sites.carries(&NewSite::of("news.example.org", 443)));

    let (_, new_sites) = reach_of(Some(&BrowserReach {
        open_web: true,
        ..listed
    }))
    .unwrap();
    assert!(new_sites.carries(&NewSite::of("news.example.org", 443)));

    let (reach, new_sites) = reach_of(None).unwrap();
    assert!(reach.hosts().is_empty(), "none given, nothing listed");
    assert!(!new_sites.carries(&NewSite::of("news.example.org", 443)));
}

#[test]
fn an_act_on_an_ended_browser_is_refused_in_the_words_the_window_reads_as_gone() {
    assert_eq!(
        purlis_browser_engine::drive::Refusal::Ended.to_string(),
        super::ENDED
    );
}
