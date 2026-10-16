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
    self, Answer, Call, Content, DragTo, Field, PageId, PageInfo, Ref, Settled,
};
use purlis_host::protocol::browsing::{self as wire, Act, Browsed, BrowserReach, PageSaid};
use purlis_host::protocol::session::{Layer, Listed};

use super::{aim_of, browsed, call_of, input_of, reach_of};
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
        asked: vec!["news.example.org:443".into()],
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
            asked: vec!["news.example.org:443".into()],
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
        withdrawn: Vec::new(),
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
fn a_browser_reaches_no_port_of_this_machine_its_chat_had_withdrawn() {
    let given = BrowserReach {
        listed: vec![
            Listed {
                host: "127.0.0.1:5173".into(),
                layer: Layer::You,
            },
            Listed {
                host: "127.0.0.1:8080".into(),
                layer: Layer::You,
            },
        ],
        withdrawn: vec![5173],
        ..BrowserReach::default()
    };

    let (reach, _) = reach_of(Some(&given)).unwrap();

    assert!(!reach.decide("127.0.0.1", 5173, &[], &[]).carries());
    assert!(reach.decide("127.0.0.1", 8080, &[], &[]).carries());
}

#[test]
fn the_persons_key_becomes_the_drivers_own_event_aimed_where_they_aimed_it() {
    let input = wire::Input::Key {
        aim: wire::Aim {
            page: "p2".into(),
            document: 9,
        },
        action: wire::KeyAction::Down,
        key: "A".into(),
        physical: "KeyA".into(),
        key_code: 65,
        modifiers: 8,
    };

    assert_eq!(
        input_of(&input),
        drive::PersonInput::Key {
            aim: drive::Aim {
                page: drive::PageId::new("p2"),
                document: 9,
            },
            down: true,
            key: "A".into(),
            code: "KeyA".into(),
            key_code: 65,
            modifiers: 8,
        }
    );
}

#[test]
fn the_persons_wheel_becomes_the_drivers_own_wheel_by_the_same_distance() {
    let input = wire::Input::Mouse {
        aim: wire::Aim {
            page: "p1".into(),
            document: 3,
        },
        action: wire::MouseAction::Wheel,
        x: 640,
        y: 400,
        button: wire::Button::None,
        clicks: 0,
        modifiers: 0,
        delta_x: 0,
        delta_y: -120,
    };

    assert_eq!(
        input_of(&input),
        drive::PersonInput::Mouse {
            aim: drive::Aim {
                page: drive::PageId::new("p1"),
                document: 3,
            },
            kind: drive::MouseKind::Wheel,
            x: 640.0,
            y: 400.0,
            button: drive::MouseButton::None,
            clicks: 0,
            modifiers: 0,
            delta_x: 0.0,
            delta_y: -120.0,
        }
    );
}

#[test]
fn a_frame_names_its_page_and_document_in_the_protocols_words() {
    let aim = drive::Aim {
        page: drive::PageId::new("p4"),
        document: 12,
    };
    assert_eq!(
        aim_of(&aim),
        wire::Aim {
            page: "p4".into(),
            document: 12,
        }
    );
}

// ---- The engine kept current in the background (#2073) ----

use std::cell::RefCell;

use purlis_browser_engine::install::{
    EngineSettings, Engines, Fetch, Manifest, NoSigner, PUBLISHED_BUNDLE, PUBLISHED_MANIFEST,
    Platform, Refused,
};

use super::keep_current;
use crate::netlog::{Feature, Party};

/// Serves only the published manifest, and remembers what it was asked for.
#[derive(Default)]
struct Asked(RefCell<Vec<String>>);

impl Fetch for Asked {
    fn fetch(&self, url: &str) -> std::io::Result<Box<dyn std::io::Read + '_>> {
        self.0.borrow_mut().push(url.to_owned());
        if url == PUBLISHED_MANIFEST {
            Ok(Box::new(std::io::Cursor::new(b"{}".to_vec())))
        } else {
            Err(std::io::Error::new(std::io::ErrorKind::NotFound, "404"))
        }
    }
}

fn engines(root: &Path) -> Engines {
    Engines::new(
        root,
        Platform::Linux64,
        Manifest::shipped(),
        Box::new(NoSigner),
    )
}

#[test]
fn a_machine_that_never_used_the_pinned_engine_is_never_checked_for_one() {
    let root = tempfile::tempdir().unwrap();
    let asked = Asked::default();
    let checked = keep_current(
        &engines(&root.path().join("engines")),
        &EngineSettings::default(),
        &asked,
        &asked,
        chrono::Utc::now(),
    );
    assert!(checked.is_none());
    assert!(asked.0.borrow().is_empty(), "nothing reached the network");
}

#[test]
fn a_machine_with_the_pinned_engine_installed_reads_the_published_manifest_and_its_bundle() {
    let root = tempfile::tempdir().unwrap();
    let installed = root.path().join("155.0.8059.79-linux64");
    std::fs::create_dir_all(&installed).unwrap();
    std::fs::write(installed.join("installed.json"), r#"{"sha256":"ab"}"#).unwrap();
    let (manifests, archives) = (Asked::default(), Asked::default());
    let checked = keep_current(
        &engines(root.path()),
        &EngineSettings::default(),
        &manifests,
        &archives,
        chrono::Utc::now(),
    );
    assert!(
        matches!(checked, Some(Err(Refused::Download(_)))),
        "{checked:?}"
    );
    assert_eq!(
        *manifests.0.borrow(),
        [PUBLISHED_MANIFEST.to_owned(), PUBLISHED_BUNDLE.to_owned()]
    );
    assert!(archives.0.borrow().is_empty());
}

#[test]
fn the_engines_archive_is_a_third_partys_and_the_manifest_is_purliss_own() {
    assert_eq!(Feature::BrowserEngine.party(), Party::ThirdParty);
    assert_eq!(
        serde_json::to_value(Feature::BrowserEngine).unwrap(),
        "browser-engine"
    );
    let path = PUBLISHED_MANIFEST
        .strip_prefix("https://github.com")
        .unwrap();
    assert!(crate::netlog::is_charters("github.com", path));
}
