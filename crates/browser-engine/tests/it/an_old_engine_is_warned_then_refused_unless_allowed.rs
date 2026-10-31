// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! An engine is warned about at 7 days behind and refused at 21, with "Update now", unless the
//! person allows an outdated engine in Settings. Behind is counted from when the newest trusted
//! manifest was issued: until a newer one is verified, the engine in use ages.

#![cfg(unix)]

use crate::common;

use std::time::Duration;

use common::fixture::{Served, VERSION, archive, at, manifest, sha256, url};
use purlis_browser_engine::install::{Age, EngineSettings, Engines, NoSigner, Platform, Refused};

const ISSUED: &str = "2026-10-01T12:00:00Z";

fn prepare(settings: &EngineSettings, now: &str) -> Result<Age, Refused> {
    let root = tempfile::tempdir().unwrap();
    let bytes = archive("pinned");
    let served = Served::with(url(VERSION), bytes.clone());
    let engines = Engines::new(
        root.path(),
        Platform::Linux64,
        manifest(VERSION, ISSUED, &sha256(&bytes)),
        Box::new(NoSigner),
    );
    engines
        .prepare(settings, &served, at(now))
        .map(|ready| ready.age())
}

#[test]
fn an_engine_less_than_seven_days_behind_is_current() {
    let age = prepare(&EngineSettings::default(), "2026-10-08T11:59:59Z").unwrap();
    assert_eq!(age, Age::Current);
}

#[test]
fn an_engine_seven_to_twenty_days_behind_runs_with_a_warning() {
    for (now, days) in [("2026-10-08T12:00:00Z", 7), ("2026-10-21T23:00:00Z", 20)] {
        let age = prepare(&EngineSettings::default(), now).unwrap();
        assert_eq!(age, Age::Behind { days }, "{now}");
        assert!(
            age.warning()
                .unwrap()
                .contains(&format!("{days} days behind")),
            "{now}"
        );
    }
}

#[test]
fn an_engine_twenty_one_days_behind_is_refused_with_update_now() {
    let refused = prepare(&EngineSettings::default(), "2026-10-22T12:00:00Z").unwrap_err();
    assert!(
        matches!(refused, Refused::Outdated { days: 21 }),
        "{refused:?}"
    );
    let words = refused.to_string();
    assert!(
        words.contains("21 days behind") && words.contains("Update now"),
        "{words}"
    );
    assert!(
        words.contains("Settings"),
        "the refusal names the setting that decides it: {words}"
    );
}

#[test]
fn an_outdated_engine_runs_when_the_person_allows_it_and_is_still_warned_about() {
    let settings = EngineSettings {
        run_outdated: true,
        ..EngineSettings::default()
    };
    let age = prepare(&settings, "2026-11-30T12:00:00Z").unwrap();
    assert_eq!(age, Age::Behind { days: 60 });
    assert!(age.warning().is_some());
}

#[test]
fn the_warning_and_the_refusal_follow_the_settings() {
    let settings = EngineSettings {
        warn_after: Duration::from_secs(86_400),
        refuse_after: Duration::from_secs(3 * 86_400),
        ..EngineSettings::default()
    };
    assert_eq!(
        prepare(&settings, "2026-10-02T12:00:00Z").unwrap(),
        Age::Behind { days: 1 }
    );
    assert!(matches!(
        prepare(&settings, "2026-10-04T12:00:00Z").unwrap_err(),
        Refused::Outdated { days: 3 }
    ));
}

#[test]
fn a_clock_before_the_manifest_was_issued_reads_as_current() {
    let age = prepare(&EngineSettings::default(), "2026-09-01T00:00:00Z").unwrap();
    assert_eq!(age, Age::Current);
}
