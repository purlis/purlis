// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

use super::super::{ArgsKey, Attributes, Began, Entry, Log, Recorder, RunOf, read};
use super::{Driver, Said, Typist};

const DEVICE: &str = "01J9ZZDEVICE0000000000000";
const CHAT: &str = "01K6E8ZK6V4Q9T0N3M2B1C5D7E";
const RUN: &str = "01K6E8ZK6V4Q9T0N3M2B1C5D7F";
const BROWSER: &str = "01K6E8ZK6V4Q9T0N3M2B1C5D7G";

fn recorder(dir: &Path) -> Recorder {
    Recorder::new(Log::open(dir, DEVICE).unwrap(), ArgsKey::open(dir).unwrap())
}

fn begun(log: &mut Recorder, plane: &Path) {
    log.begin(
        plane,
        3,
        RunOf {
            chat: CHAT,
            run: RUN,
        },
        Began::Start,
        &Attributes::default(),
    )
    .unwrap();
}

/// What the window asks, as it travels: written as JSON and read back.
fn sent(entry: Entry) -> Entry {
    serde_json::from_value(serde_json::to_value(&entry).expect("it is written")).expect("it reads")
}

#[test]
fn who_drives_a_chats_browser_is_written_under_the_chat_naming_the_browser_and_its_driver() {
    let dir = tempfile::tempdir().unwrap();
    let plane = Path::new("/planes/one");
    let mut log = recorder(dir.path());
    begun(&mut log, plane);

    for said in [
        Said::DriverChanged {
            browser: BROWSER.into(),
            driver: Driver::Person,
        },
        Said::Typed {
            browser: BROWSER.into(),
            by: Typist::Person,
        },
        Said::DriverChanged {
            browser: BROWSER.into(),
            driver: Driver::Agent,
        },
        Said::DriverChanged {
            browser: BROWSER.into(),
            driver: Driver::Purlis,
        },
        Said::Typed {
            browser: BROWSER.into(),
            by: Typist::Purlis,
        },
        Said::HandedOver {
            browser: BROWSER.into(),
        },
    ] {
        log.apply(sent(Entry::Browser {
            plane: plane.into(),
            number: Some(3),
            said,
        }))
        .expect("written");
    }

    let events = read(dir.path()).unwrap();
    let browsing: Vec<_> = events
        .iter()
        .filter(|event| event.kind.starts_with("browser."))
        .map(|event| {
            assert_eq!(event.chat.as_deref(), Some(CHAT));
            assert_eq!(event.run.as_deref(), Some(RUN));
            (event.kind.as_str(), event.body.clone())
        })
        .collect();
    assert_eq!(
        browsing,
        [
            (
                "browser.driver_changed",
                serde_json::json!({ "browser": BROWSER, "driver": "person" })
            ),
            (
                "browser.typed",
                serde_json::json!({ "browser": BROWSER, "by": "person" })
            ),
            (
                "browser.driver_changed",
                serde_json::json!({ "browser": BROWSER, "driver": "agent" })
            ),
            (
                "browser.driver_changed",
                serde_json::json!({ "browser": BROWSER, "driver": "purlis" })
            ),
            (
                "browser.typed",
                serde_json::json!({ "browser": BROWSER, "by": "purlis" })
            ),
            (
                "browser.handed_over",
                serde_json::json!({ "browser": BROWSER })
            ),
        ]
    );
}

#[test]
fn a_browser_no_chat_uses_is_written_with_no_chat() {
    let dir = tempfile::tempdir().unwrap();
    let mut log = recorder(dir.path());

    log.apply(Entry::Browser {
        plane: "/planes/one".into(),
        number: None,
        said: Said::DriverChanged {
            browser: BROWSER.into(),
            driver: Driver::Person,
        },
    })
    .unwrap();

    let events = read(dir.path()).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, "browser.driver_changed");
    assert_eq!(
        (events[0].chat.as_deref(), events[0].run.as_deref()),
        (None, None)
    );
}

#[test]
fn a_browser_named_by_anything_but_a_programs_id_is_refused_and_nothing_is_written() {
    let dir = tempfile::tempdir().unwrap();
    let mut log = recorder(dir.path());

    for browser in [
        "",
        "https://shop.example.test/login?token=s3cret",
        "hunter2 typed here",
        &"a".repeat(65),
    ] {
        let refused = log.apply(Entry::Browser {
            plane: "/planes/one".into(),
            number: None,
            said: Said::Typed {
                browser: browser.to_owned(),
                by: Typist::Person,
            },
        });
        assert!(refused.is_err(), "{browser:?} was written");
    }

    assert!(read(dir.path()).unwrap().is_empty());
}

#[test]
fn a_said_with_a_field_more_does_not_read() {
    // What the window sends is a closed set: a key, a value or an address has nowhere to go.
    let more = serde_json::json!({
        "entry": "browser",
        "plane": "/planes/one",
        "said": { "said": "typed", "browser": BROWSER, "by": "person", "text": "hunter2" },
    });
    assert!(serde_json::from_value::<Entry>(more).is_err());
}
