// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The fixture site's download page and file-input page, through a real engine** (#3747,
//! #2020): a download sent as an attachment and one a page makes from a `blob:` each land in
//! the holding folder under the engine's own name, and are told to the agent by the page's
//! name, their size and their origin; one past the size limit is cancelled and its file
//! removed, its size stated up front or not; and a file input takes the files given, whose
//! bytes the page then reads.
//!
//! These need a real Chrome, which a sandboxed agent cannot start: they run where
//! `PURLIS_TEST_ENGINE` names one, and on CI against the runner's own Chrome. Anywhere else each
//! says why it did not run and passes.

#![cfg(unix)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::common;

use common::drive::{call, find, open, ref_on, sees};
use common::site::{BIG, EXPORTED_AS, started, started_limited};
use purlis_browser_engine::drive::{Browser, Call, DownloadInfo, DownloadState, Limits};

/// What the fixture site's `report.csv` holds.
const REPORT: &[u8] = b"quarter,total\nQ3,1234\n";

/// What the download page's "Make a file" makes.
const MADE: &[u8] = b"made in the page\n";

/// The size limit the tests of it give a browser: far under [`BIG`].
const LIMIT: u64 = 256 * 1024;

/// The browser's downloads folder, made and given to it as the session host gives it one.
fn holding(browser: &Browser, data: &Path) -> PathBuf {
    let held = data.join("downloads");
    std::fs::create_dir_all(&held).unwrap();
    browser.hold_downloads_in(&held).unwrap();
    held
}

/// The files in `folder`, by name.
fn files_in(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(folder)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// Clicks the link or button named `words` on the download page.
fn click(browser: &Browser, words: &str) {
    let target = ref_on(&find(browser, words), words);
    call(browser, Call::Click { target });
}

/// The browser's one download once `ready` holds of it, as the agent's acts are told of it:
/// each answer's downloads are those that began or moved on since the last.
fn told_until(browser: &Browser, ready: impl Fn(&DownloadInfo) -> bool) -> DownloadInfo {
    let start = Instant::now();
    let mut told: BTreeMap<String, DownloadInfo> = BTreeMap::new();
    loop {
        for one in call(browser, Call::Pages).downloads {
            told.insert(one.id.clone(), one);
        }
        if let Some(one) = told.values().find(|one| ready(one)) {
            assert_eq!(told.len(), 1, "one download: {told:?}");
            return one.clone();
        }
        assert!(
            start.elapsed() < Duration::from_secs(30),
            "the download was never told as asked: {told:?}"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Whether `folder` is empty within `limit`.
fn emptied_within(folder: &Path, limit: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < limit {
        if files_in(folder).is_empty() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    files_in(folder).is_empty()
}

#[test]
fn a_download_sent_as_an_attachment_is_held_under_the_engines_name_and_told_by_the_headers() {
    let Some((browser, data, _site)) = started() else {
        return;
    };
    let held = holding(&browser, data.path());
    open(&browser, "downloads.html");
    click(&browser, "Export the report");

    let done = told_until(&browser, |one| one.state == DownloadState::Done);
    assert_eq!(done.id, "d1");
    assert_eq!(
        done.name, EXPORTED_AS,
        "the header's name, not the address's"
    );
    assert_eq!(done.origin.as_deref(), Some("http://fixture.test"));
    assert_eq!(done.received, REPORT.len() as u64);
    assert_eq!(done.total, Some(REPORT.len() as u64));
    // In the holding folder under the engine's own name, never the page's, and nothing else.
    assert_eq!(files_in(&held), [done.file.clone()]);
    assert_ne!(done.file, EXPORTED_AS);
    assert_eq!(std::fs::read(held.join(&done.file)).unwrap(), REPORT);
    // Listed whole as it is.
    let listed = call(&browser, Call::Downloads).downloads;
    assert_eq!(listed, [done]);
}

#[test]
fn a_download_a_page_makes_from_a_blob_is_held_and_from_the_pages_origin() {
    let Some((browser, data, _site)) = started() else {
        return;
    };
    let held = holding(&browser, data.path());
    open(&browser, "downloads.html");
    click(&browser, "Make a file");

    let done = told_until(&browser, |one| one.state == DownloadState::Done);
    assert_eq!(done.name, "made.txt");
    assert_eq!(
        done.origin.as_deref(),
        Some("http://fixture.test"),
        "a blob has no web address: the page's origin"
    );
    assert_eq!(done.received, MADE.len() as u64);
    assert_eq!(files_in(&held), [done.file.clone()]);
    assert_eq!(std::fs::read(held.join(&done.file)).unwrap(), MADE);
}

/// A download past the limit, from `link` of the download page: said too large, cancelled, and
/// its file gone, and still gone once the site would have sent it whole.
fn past_the_limit_is_cancelled_and_removed(link: &str) {
    let Some((browser, data, _site)) = started_limited(|limits| Limits {
        most_download_bytes: LIMIT,
        ..limits
    }) else {
        return;
    };
    let held = holding(&browser, data.path());
    open(&browser, "downloads.html");
    let clicked = Instant::now();
    click(&browser, link);

    let cut = told_until(&browser, |one| one.state == DownloadState::TooLarge);
    assert!(
        cut.received < BIG as u64,
        "cut before it came whole: {cut:?}"
    );
    assert!(
        emptied_within(&held, Duration::from_secs(10)),
        "its file was removed: {:?}",
        files_in(&held)
    );
    // The site would have sent it whole by now: nothing of it came back.
    let whole = Duration::from_secs(8);
    if let Some(left) = whole.checked_sub(clicked.elapsed()) {
        std::thread::sleep(left);
    }
    assert!(files_in(&held).is_empty(), "{:?}", files_in(&held));
    let listed = call(&browser, Call::Downloads).downloads;
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert_eq!(listed[0].state, DownloadState::TooLarge, "{listed:?}");
}

#[test]
fn a_download_whose_stated_size_is_past_the_limit_is_cancelled_and_its_file_removed() {
    past_the_limit_is_cancelled_and_removed("Export everything");
}

#[test]
fn a_download_that_does_not_state_its_size_is_cancelled_and_removed_once_past_the_limit() {
    past_the_limit_is_cancelled_and_removed("Stream everything");
}

#[test]
fn a_file_input_takes_the_files_given_and_the_page_reads_their_bytes() {
    let Some((browser, data, _site)) = started() else {
        return;
    };
    let notes = data.path().join("notes.txt");
    std::fs::write(&notes, b"hello from purlis\n").unwrap();
    open(&browser, "upload.html");
    let input = ref_on(&find(&browser, "Attachment"), "Attachment");
    call(
        &browser,
        Call::Upload {
            target: input,
            files: vec![notes],
        },
    );
    assert!(
        sees(&browser, "received notes.txt, 18 bytes: hello from purlis"),
        "the page read the file's bytes"
    );
}
