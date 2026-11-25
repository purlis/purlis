// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The engine's prepare is one timed step** (#7182): written only where it did something (a
//! refusal, a cancel, a download), never for the person's own Chrome or an engine already
//! installed, and a download is its child.

#![cfg(unix)]

use crate::common;

use std::io;
use std::sync::{Arc, Mutex};

use common::fixture::{Served, VERSION, archive, at, manifest, sha256, url};
use purlis_browser_engine::install::{
    EngineChoice, EngineSettings, Engines, Fetch, NoSigner, Platform, Refused,
};

const ISSUED: &str = "2026-10-01T12:00:00Z";
const NOW: &str = "2026-10-02T12:00:00Z";

#[derive(Clone, Default)]
struct Said(Arc<Mutex<Vec<u8>>>);
impl io::Write for Said {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Said {
    type Writer = Said;
    fn make_writer(&'a self) -> Said {
        self.clone()
    }
}

/// The lines `then` wrote at `info`.
fn said_while(then: impl FnOnce()) -> String {
    let said = Said::default();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(said.clone())
        .with_ansi(false)
        .with_max_level(tracing::Level::INFO)
        .finish();
    tracing::subscriber::with_default(subscriber, then);
    String::from_utf8(said.0.lock().unwrap().clone()).unwrap()
}

fn engines(root: &std::path::Path, pinned_digest: &str) -> Engines {
    Engines::new(
        root,
        Platform::Linux64,
        manifest(VERSION, ISSUED, pinned_digest),
        Box::new(NoSigner),
    )
}

fn line<'a>(out: &'a str, step: &str) -> &'a str {
    out.lines()
        .find(|line| line.contains(&format!("purlis: step {step} ")))
        .unwrap_or_else(|| panic!("no {step} line in {out}"))
}

#[test]
fn the_persons_own_chrome_is_looked_for_and_writes_nothing_when_found() {
    let root = tempfile::tempdir().unwrap();
    let chrome = root.path().join("chrome");
    std::fs::write(&chrome, b"").unwrap();
    let settings = EngineSettings {
        engine: EngineChoice::Machine(chrome),
        ..EngineSettings::default()
    };
    let engines = engines(root.path(), &"0".repeat(64));
    let out = said_while(|| {
        engines
            .prepare(&settings, &Served::default(), at(NOW))
            .expect("found");
    });
    assert!(out.is_empty(), "{out}");
}

#[test]
fn the_persons_own_chrome_that_is_not_there_is_a_refused_prepare() {
    let root = tempfile::tempdir().unwrap();
    let settings = EngineSettings {
        engine: EngineChoice::Machine(root.path().join("missing")),
        ..EngineSettings::default()
    };
    let engines = engines(root.path(), &"0".repeat(64));
    let out = said_while(|| {
        assert!(
            engines
                .prepare(&settings, &Served::default(), at(NOW))
                .is_err()
        );
    });
    assert!(
        line(&out, "browser_engine_prepare").contains("refused after"),
        "{out}"
    );
}

#[test]
fn settings_that_refuse_before_they_warn_are_a_refused_prepare_timed_whole() {
    let root = tempfile::tempdir().unwrap();
    let settings = EngineSettings {
        refuse_after: std::time::Duration::from_secs(1),
        ..EngineSettings::default()
    };
    let engines = engines(root.path(), &"0".repeat(64));
    let out = said_while(|| {
        assert!(matches!(
            engines.prepare(&settings, &Served::default(), at(NOW)),
            Err(Refused::Settings(_))
        ));
    });
    assert!(
        line(&out, "browser_engine_prepare").contains("refused after"),
        "{out}"
    );
}

#[test]
fn an_outdated_engine_refused_is_a_refused_prepare() {
    let root = tempfile::tempdir().unwrap();
    let engines = engines(root.path(), &"0".repeat(64));
    let out = said_while(|| {
        assert!(matches!(
            engines.prepare(
                &EngineSettings::default(),
                &Served::default(),
                at("2026-10-22T12:00:00Z")
            ),
            Err(Refused::Outdated { .. })
        ));
    });
    assert!(
        line(&out, "browser_engine_prepare").contains("refused after"),
        "{out}"
    );
}

#[test]
fn a_prepare_that_downloads_is_done_with_its_download_inside_it_and_the_next_writes_nothing() {
    let root = tempfile::tempdir().unwrap();
    let bytes = archive("pinned");
    let served = Served::with(url(VERSION), bytes.clone());
    let engines = engines(root.path(), &sha256(&bytes));
    let out = said_while(|| {
        engines
            .prepare(&EngineSettings::default(), &served, at(NOW))
            .expect("downloaded");
    });
    assert!(
        line(&out, "browser_engine_prepare").contains("done after"),
        "{out}"
    );
    assert!(
        !line(&out, "browser_engine_prepare").contains("(parent:"),
        "{out}"
    );
    let download = line(&out, "browser_engine_download");
    assert!(
        download.contains("done after") && download.contains("(parent: browser_engine_prepare)"),
        "{out}"
    );
    // Installed now: it is only looked for.
    let again = said_while(|| {
        engines
            .prepare(&EngineSettings::default(), &served, at(NOW))
            .expect("installed");
    });
    assert!(again.is_empty(), "{again}");
}

#[test]
fn a_download_that_fails_is_refused_in_both_its_steps() {
    let root = tempfile::tempdir().unwrap();
    let bytes = archive("pinned");
    let served = Served::with(url(VERSION), bytes);
    let engines = engines(root.path(), &"0".repeat(64));
    let out = said_while(|| {
        assert!(matches!(
            engines.prepare(&EngineSettings::default(), &served, at(NOW)),
            Err(Refused::Digest { .. })
        ));
    });
    assert!(
        line(&out, "browser_engine_prepare").contains("refused after"),
        "{out}"
    );
    assert!(
        line(&out, "browser_engine_download").contains("refused after"),
        "{out}"
    );
}

/// A download the person cancels: its words, as `purlis-core`'s progress reader says them.
struct Cancelled<'a>(&'a Served);
impl Fetch for Cancelled<'_> {
    fn fetch(&self, url: &str) -> io::Result<Box<dyn io::Read + '_>> {
        self.0.fetch(url)
    }
    fn still_wanted(&self) -> io::Result<()> {
        Err(io::Error::other("the person cancelled the download"))
    }
    fn cancelled(&self) -> bool {
        true
    }
}

#[test]
fn a_download_the_person_cancels_is_cancelled_not_refused() {
    let root = tempfile::tempdir().unwrap();
    let bytes = archive("pinned");
    let served = Served::with(url(VERSION), bytes.clone());
    let engines = engines(root.path(), &sha256(&bytes));
    let out = said_while(|| {
        assert!(
            engines
                .prepare(&EngineSettings::default(), &Cancelled(&served), at(NOW))
                .is_err()
        );
    });
    assert!(
        line(&out, "browser_engine_prepare").contains("cancelled after"),
        "{out}"
    );
    assert!(
        line(&out, "browser_engine_download").contains("cancelled after"),
        "{out}"
    );
    assert!(!out.contains("refused"), "{out}");
}
