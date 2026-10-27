// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

use super::super::{ArgsKey, Attributes, Began, Entry, Log, Recorder, RunOf, read};
use super::{Called, Closer, Driver, Outcome, Said, Typist};

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
            origin: Some("https://shop.example.test".into()),
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
            origin: None,
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
                serde_json::json!({ "browser": BROWSER, "by": "person", "origin": "https://shop.example.test" })
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
                serde_json::json!({ "browser": BROWSER, "by": "purlis", "origin": null })
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
                origin: None,
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

/// What the log holds of `said`, each written under chat 3: its kind and body.
fn written(said: Vec<Said>) -> Vec<(String, serde_json::Value)> {
    let dir = tempfile::tempdir().unwrap();
    let plane = Path::new("/planes/one");
    let mut log = recorder(dir.path());
    begun(&mut log, plane);
    for said in said {
        log.apply(sent(Entry::Browser {
            plane: plane.into(),
            number: Some(3),
            said,
        }))
        .expect("written");
    }
    read(dir.path())
        .unwrap()
        .into_iter()
        .filter(|event| event.kind.starts_with("browser."))
        .map(|event| {
            assert_eq!(event.chat.as_deref(), Some(CHAT));
            (event.kind, event.body)
        })
        .collect()
}

#[test]
fn a_browser_opening_and_each_way_it_ends_is_written_saying_who_ended_it() {
    let closed = |by| Said::Closed {
        browser: BROWSER.into(),
        by,
    };
    let events = written(vec![
        Said::Opened {
            browser: BROWSER.into(),
        },
        closed(Closer::Chat),
        closed(Closer::Person),
        closed(Closer::Idle),
        closed(Closer::KillSwitch),
        Said::Crashed {
            browser: BROWSER.into(),
        },
    ]);

    use serde_json::json;
    assert_eq!(
        events,
        [
            ("browser.opened".into(), json!({ "browser": BROWSER })),
            (
                "browser.closed".into(),
                json!({ "browser": BROWSER, "by": "chat" })
            ),
            (
                "browser.closed".into(),
                json!({ "browser": BROWSER, "by": "person" })
            ),
            (
                "browser.closed".into(),
                json!({ "browser": BROWSER, "by": "idle" })
            ),
            (
                "browser.closed".into(),
                json!({ "browser": BROWSER, "by": "kill_switch" })
            ),
            (
                "browser.crashed".into(),
                json!({ "browser": BROWSER, "by": "crash" })
            ),
        ]
    );
}

#[test]
fn pinning_and_unpinning_a_browser_is_written_naming_the_browser() {
    let events = written(vec![
        Said::Pinned {
            browser: BROWSER.into(),
        },
        Said::Unpinned {
            browser: BROWSER.into(),
        },
    ]);

    assert_eq!(
        events,
        [
            (
                "browser.pinned".into(),
                serde_json::json!({ "browser": BROWSER })
            ),
            (
                "browser.unpinned".into(),
                serde_json::json!({ "browser": BROWSER })
            ),
        ]
    );
}

#[test]
fn a_browser_tool_call_is_written_with_its_word_its_pages_origin_and_its_outcome() {
    let events = written(vec![
        Said::Call {
            browser: BROWSER.into(),
            call: Called::Type,
            origin: Some("https://shop.example.test:8443".into()),
            outcome: Outcome::Done,
        },
        Said::Call {
            browser: BROWSER.into(),
            call: Called::LogIn,
            origin: None,
            outcome: Outcome::Refused,
        },
    ]);

    assert_eq!(
        events,
        [
            (
                "browser.call".into(),
                serde_json::json!({
                    "browser": BROWSER,
                    "call": "type",
                    "origin": "https://shop.example.test:8443",
                    "outcome": "done",
                })
            ),
            (
                "browser.call".into(),
                serde_json::json!({
                    "browser": BROWSER,
                    "call": "log_in",
                    "origin": null,
                    "outcome": "refused",
                })
            ),
        ]
    );
}

#[test]
fn an_origin_that_is_more_than_an_origin_is_refused_and_nothing_is_written() {
    let dir = tempfile::tempdir().unwrap();
    let mut log = recorder(dir.path());

    for origin in [
        "https://shop.example.test/login",
        "https://shop.example.test?token=s3cret",
        "https://user:hunter2@shop.example.test",
        "https://shop.example.test#hunter2",
        "HTTPS://Shop.Example.Test",
        "hunter2",
        "file:///etc/passwd",
    ] {
        for said in [
            Said::Typed {
                browser: BROWSER.into(),
                by: Typist::Person,
                origin: Some(origin.into()),
            },
            Said::Call {
                browser: BROWSER.into(),
                call: Called::Navigate,
                origin: Some(origin.into()),
                outcome: Outcome::Done,
            },
        ] {
            let refused = log.apply(Entry::Browser {
                plane: "/planes/one".into(),
                number: None,
                said,
            });
            assert!(refused.is_err(), "{origin:?} was written");
        }
    }

    assert!(read(dir.path()).unwrap().is_empty());
}

#[test]
fn a_call_with_a_field_more_does_not_read() {
    // An argument or a value has nowhere to go.
    let more = serde_json::json!({
        "entry": "browser",
        "plane": "/planes/one",
        "said": {
            "said": "call", "browser": BROWSER, "call": "type", "origin": null,
            "outcome": "done", "text": "hunter2",
        },
    });
    assert!(serde_json::from_value::<Entry>(more).is_err());
}

#[test]
fn a_call_named_by_a_word_outside_the_closed_set_does_not_read() {
    let unknown_call = serde_json::json!({
        "entry": "browser",
        "plane": "/planes/one",
        "said": {
            "said": "call", "browser": BROWSER, "call": "evaluate", "origin": null,
            "outcome": "done",
        },
    });
    assert!(serde_json::from_value::<Entry>(unknown_call).is_err());
}

#[test]
fn a_close_said_to_be_a_crash_is_refused_and_nothing_is_written() {
    let dir = tempfile::tempdir().unwrap();
    let mut log = recorder(dir.path());

    let refused = log.apply(Entry::Browser {
        plane: "/planes/one".into(),
        number: None,
        said: Said::Closed {
            browser: BROWSER.into(),
            by: Closer::Crash,
        },
    });

    assert!(refused.is_err());
    assert!(read(dir.path()).unwrap().is_empty());
}

#[test]
fn a_typed_said_by_a_window_before_origins_is_read_with_none() {
    let old = serde_json::json!({
        "entry": "browser",
        "plane": "/planes/one",
        "said": { "said": "typed", "browser": BROWSER, "by": "person" },
    });
    let Entry::Browser { said, .. } = serde_json::from_value::<Entry>(old).unwrap() else {
        panic!("not a browser entry");
    };
    assert_eq!(
        said,
        Said::Typed {
            browser: BROWSER.into(),
            by: Typist::Person,
            origin: None,
        }
    );
}
