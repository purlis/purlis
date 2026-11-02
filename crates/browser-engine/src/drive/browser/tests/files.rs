// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Files across a browser's edge (#2020): a download lands in the folder the browser was given,
//! under the engine's own name for it, and is told to the agent by a plain name, its size and
//! where it came from, cancelled past the browser's limits; an upload puts files purlis made
//! into a file input, and nothing else.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde_json::json;

use super::{browser, browser_with, frame_moves, id, limits, ref_of, snapshot};
use crate::drive::{Call, DownloadInfo, DownloadState, Limits, Refusal, plain_file_name};

fn began(engine: &super::Engine, guid: &str, url: &str, name: &str) {
    engine.emit(
        "Browser.downloadWillBegin",
        json!({ "frameId": "PAGE-1", "guid": guid, "url": url, "suggestedFilename": name }),
        None,
    );
}

fn progress(engine: &super::Engine, guid: &str, received: u64, total: u64, state: &str) {
    engine.emit(
        "Browser.downloadProgress",
        json!({ "guid": guid, "receivedBytes": received, "totalBytes": total, "state": state }),
        None,
    );
}

/// The downloads the browser lists, once `ready` holds of them (the engine's events are read
/// on a thread of their own).
fn listed_once(
    browser: &crate::drive::Browser,
    ready: impl Fn(&[DownloadInfo]) -> bool,
) -> Vec<DownloadInfo> {
    let start = Instant::now();
    let mut n = 900;
    loop {
        n += 1;
        let listed = browser.call(&id(n), Call::Downloads).unwrap().downloads;
        if ready(&listed) || start.elapsed() > Duration::from_secs(5) {
            return listed;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// The frame that began a `data:` download, as the engine says it; where it came from is its
/// document's.
fn began_in_the_frame(engine: &super::Engine) {
    engine.emit(
        "Browser.downloadWillBegin",
        json!({ "frameId": "PAYMENT-FRAME", "guid": "GUID-F", "url": "data:text/plain,hi", "suggestedFilename": "hi.txt" }),
        None,
    );
}

#[test]
fn a_download_a_frame_began_comes_from_the_address_it_attached_at() {
    let (browser, engine) = browser();
    let third = "http://third.test/pay.html";
    frame_moves(&engine, third, third, Duration::from_secs(3));
    began_in_the_frame(&engine);
    let listed = listed_once(&browser, |all| !all.is_empty());
    assert_eq!(listed[0].origin.as_deref(), Some("http://third.test"));
}

#[test]
fn a_download_from_a_frame_that_attached_at_no_address_comes_from_nowhere_known() {
    let (browser, engine) = browser();
    frame_moves(
        &engine,
        "",
        "http://third.test/pay.html",
        Duration::from_secs(3),
    );
    began_in_the_frame(&engine);
    let listed = listed_once(&browser, |all| !all.is_empty());
    assert_eq!(listed[0].origin, None);
}

#[test]
fn downloads_are_held_in_the_folder_given_each_named_by_the_engine_and_told_as_they_go() {
    let (browser, engine) = browser();
    browser
        .hold_downloads_in(&PathBuf::from("/data/browsers/b-1/downloads"))
        .unwrap();
    let told = &engine.sent("Browser.setDownloadBehavior")[0];
    assert_eq!(
        told.params,
        json!({
            "behavior": "allowAndName",
            "downloadPath": "/data/browsers/b-1/downloads",
            "eventsEnabled": true,
        })
    );
    assert_eq!(told.session, None, "browser-wide, every page's");
}

#[test]
fn a_download_is_told_once_in_the_next_answer_by_its_plain_name_its_size_and_its_origin() {
    let (browser, engine) = browser();
    began(
        &engine,
        "GUID-1",
        "https://files.example.com:8443/report?id=7",
        "../../.bashrc",
    );
    progress(&engine, "GUID-1", 2048, 2048, "completed");
    let listed = listed_once(&browser, |all| {
        all.first().is_some_and(|d| d.state == DownloadState::Done)
    });
    let expected = DownloadInfo {
        id: "d1".into(),
        name: plain_file_name("../../.bashrc"),
        origin: Some("https://files.example.com:8443".into()),
        received: 2048,
        total: Some(2048),
        state: DownloadState::Done,
        file: "GUID-1".into(),
    };
    assert_eq!(listed, vec![expected.clone()]);
    assert_eq!(expected.name, "_.._.bashrc");
    // Told once with the next act's answer, then not again until it moves on.
    let snapshot_answer = browser
        .call(
            &id(1),
            Call::Snapshot {
                page: None,
                within: None,
                from: 1,
                lines: None,
            },
        )
        .unwrap();
    assert!(snapshot_answer.downloads.is_empty(), "{snapshot_answer:?}");
}

#[test]
fn a_download_that_moves_on_between_acts_is_told_with_the_next_act() {
    let (browser, engine) = browser();
    began(&engine, "GUID-2", "https://example.com/a.zip", "a.zip");
    let start = Instant::now();
    let mut told = Vec::new();
    let mut n = 0;
    while told.is_empty() && start.elapsed() < Duration::from_secs(5) {
        n += 1;
        told = browser.call(&id(n), Call::Pages).unwrap().downloads;
    }
    assert_eq!(told.len(), 1, "{told:?}");
    assert_eq!(told[0].state, DownloadState::Going);
    assert_eq!(told[0].name, "a.zip");
}

#[test]
fn a_download_from_no_web_address_is_from_the_origin_of_the_page_that_began_it() {
    let (browser, engine) = browser();
    began(&engine, "GUID-3", "blob:null/1234", "made.csv");
    let listed = listed_once(&browser, |all| !all.is_empty());
    assert_eq!(listed[0].origin.as_deref(), Some("http://fixture.test"));
}

#[test]
fn a_download_past_the_most_bytes_is_cancelled_by_purlis_and_said_too_large() {
    let (browser, engine) = browser_with(Limits {
        most_download_bytes: 1000,
        ..limits()
    });
    began(&engine, "GUID-4", "https://example.com/big.iso", "big.iso");
    progress(&engine, "GUID-4", 10, 5000, "inProgress");
    let listed = listed_once(&browser, |all| {
        all.first()
            .is_some_and(|d| d.state == DownloadState::TooLarge)
    });
    assert_eq!(listed[0].state, DownloadState::TooLarge, "{listed:?}");
    let start = Instant::now();
    while engine.sent("Browser.cancelDownload").is_empty()
        && start.elapsed() < Duration::from_secs(5)
    {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        engine.sent("Browser.cancelDownload")[0].params,
        json!({ "guid": "GUID-4" })
    );
}

#[test]
fn a_download_that_does_not_say_its_size_is_cancelled_once_it_passes_the_most() {
    let (browser, engine) = browser_with(Limits {
        most_download_bytes: 1000,
        ..limits()
    });
    began(&engine, "GUID-5", "https://example.com/stream", "stream");
    progress(&engine, "GUID-5", 1001, 0, "inProgress");
    let listed = listed_once(&browser, |all| {
        all.first()
            .is_some_and(|d| d.state == DownloadState::TooLarge)
    });
    assert_eq!(listed[0].total, None);
    assert_eq!(listed[0].state, DownloadState::TooLarge);
}

#[test]
fn downloads_past_the_most_a_browser_makes_are_cancelled_listed_nowhere_and_counted_once() {
    let (browser, engine) = browser_with(Limits {
        most_downloads: 1,
        ..limits()
    });
    began(&engine, "GUID-6", "https://example.com/1", "one");
    for n in 7..50 {
        began(
            &engine,
            &format!("GUID-{n}"),
            "https://example.com/2",
            "two",
        );
    }
    let start = Instant::now();
    while engine.sent("Browser.cancelDownload").len() < 43
        && start.elapsed() < Duration::from_secs(5)
    {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(engine.sent("Browser.cancelDownload").len(), 43);
    assert_eq!(
        engine.sent("Browser.cancelDownload")[0].params["guid"],
        "GUID-7"
    );
    // Only the one within the most is listed: a page that begins downloads in a loop grows
    // nothing but a count.
    let answer = browser.call(&id(1), Call::Pages).unwrap();
    assert_eq!(answer.downloads.len(), 1, "{answer:?}");
    let note = answer.note.unwrap_or_default();
    assert!(note.contains("cancelled 43 more downloads"), "{note}");
    assert_eq!(
        browser
            .call(&id(2), Call::Downloads)
            .unwrap()
            .downloads
            .len(),
        1
    );
    assert_eq!(
        browser.call(&id(3), Call::Pages).unwrap().note,
        None,
        "told once"
    );
}

#[test]
fn a_download_past_the_most_bytes_has_its_file_removed_even_once_it_is_done() {
    let held = tempfile::tempdir().unwrap();
    let (browser, engine) = browser_with(Limits {
        most_download_bytes: 1000,
        ..limits()
    });
    browser.hold_downloads_in(held.path()).unwrap();
    std::fs::write(held.path().join("GUID-11"), vec![0u8; 2000]).unwrap();
    std::fs::write(held.path().join("GUID-12"), b"small").unwrap();
    began(&engine, "GUID-11", "https://example.com/big", "big");
    progress(&engine, "GUID-11", 2000, 2000, "completed");
    began(&engine, "GUID-12", "https://example.com/small", "small");
    progress(&engine, "GUID-12", 5, 5, "completed");
    let start = Instant::now();
    while held.path().join("GUID-11").exists() && start.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(
        !held.path().join("GUID-11").exists(),
        "the file past the most went"
    );
    assert!(
        held.path().join("GUID-12").exists(),
        "the one within it stays"
    );
    // Done already: nothing to cancel.
    assert!(engine.sent("Browser.cancelDownload").is_empty());
}

/// The engine finishes a download purlis has already cut, its cancel too late to stop it: the
/// file it then writes goes too.
#[test]
fn a_download_cut_while_going_that_the_engine_still_finishes_has_its_file_removed() {
    let held = tempfile::tempdir().unwrap();
    let (browser, engine) = browser_with(Limits {
        most_download_bytes: 1000,
        ..limits()
    });
    browser.hold_downloads_in(held.path()).unwrap();
    began(&engine, "GUID-13", "https://example.com/big", "big");
    progress(&engine, "GUID-13", 2000, 0, "inProgress");
    let start = Instant::now();
    while engine.sent("Browser.cancelDownload").is_empty()
        && start.elapsed() < Duration::from_secs(5)
    {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(engine.sent("Browser.cancelDownload").len(), 1);
    // The engine had already written the last of it as the cancel came.
    std::fs::write(held.path().join("GUID-13"), vec![0u8; 3000]).unwrap();
    progress(&engine, "GUID-13", 3000, 3000, "completed");
    let start = Instant::now();
    while held.path().join("GUID-13").exists() && start.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(
        !held.path().join("GUID-13").exists(),
        "the file the engine finished went"
    );
    let listed = listed_once(&browser, |_| true);
    assert_eq!(
        listed[0].state,
        DownloadState::TooLarge,
        "still said too large"
    );
}

/// A download refused as it begins, past the most a browser makes, that the engine still
/// finishes: its file goes too, and the one within the most stays.
#[test]
fn a_download_refused_as_it_began_that_the_engine_still_finishes_has_its_file_removed() {
    let held = tempfile::tempdir().unwrap();
    let (browser, engine) = browser_with(Limits {
        most_downloads: 1,
        ..limits()
    });
    browser.hold_downloads_in(held.path()).unwrap();
    began(&engine, "GUID-14", "https://example.com/one", "one");
    began(&engine, "GUID-15", "https://example.com/two", "two");
    let start = Instant::now();
    while engine.sent("Browser.cancelDownload").is_empty()
        && start.elapsed() < Duration::from_secs(5)
    {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        engine.sent("Browser.cancelDownload")[0].params["guid"],
        "GUID-15"
    );
    // A moment for the refusal's own removal, of a file not yet there, to pass.
    std::thread::sleep(Duration::from_millis(50));
    std::fs::write(held.path().join("GUID-14"), b"one").unwrap();
    std::fs::write(held.path().join("GUID-15"), b"two").unwrap();
    progress(&engine, "GUID-14", 3, 3, "completed");
    progress(&engine, "GUID-15", 3, 3, "completed");
    let start = Instant::now();
    while held.path().join("GUID-15").exists() && start.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(
        !held.path().join("GUID-15").exists(),
        "the refused file the engine finished went"
    );
    assert!(
        held.path().join("GUID-14").exists(),
        "the one within the most stays"
    );
    let listed = listed_once(&browser, |all| {
        all.first().is_some_and(|d| d.state == DownloadState::Done)
    });
    assert_eq!(
        listed.len(),
        1,
        "the refused one is listed nowhere: {listed:?}"
    );
}

#[test]
fn a_browser_whose_engine_refuses_its_downloads_folder_is_refused_too() {
    let (browser, engine) = browser();
    engine.on(|sent, _| {
        (sent.method == "Browser.setDownloadBehavior").then(|| {
            Err(crate::cdp::CallError::Engine {
                code: -32000,
                message: "Browser context management is not supported.".into(),
            })
        })
    });
    assert!(
        browser
            .hold_downloads_in(&PathBuf::from("/data/browsers/b-1/downloads"))
            .is_err()
    );
}

#[test]
fn a_cancelled_download_is_said_cancelled() {
    let (browser, engine) = browser();
    began(&engine, "GUID-8", "https://example.com/x", "x");
    progress(&engine, "GUID-8", 5, 10, "canceled");
    let listed = listed_once(&browser, |all| {
        all.first()
            .is_some_and(|d| d.state == DownloadState::Cancelled)
    });
    assert_eq!(listed[0].state, DownloadState::Cancelled);
}

#[test]
fn a_plain_name_keeps_no_path_control_or_direction_mark_and_is_never_empty() {
    assert_eq!(plain_file_name("report.pdf"), "report.pdf");
    assert_eq!(plain_file_name("a/b\\c:d"), "a_b_c_d");
    assert_eq!(plain_file_name("in\u{7}voice\n.pdf"), "invoice.pdf");
    assert_eq!(plain_file_name("photo\u{202e}gpj.exe"), "photogpj.exe");
    assert_eq!(
        plain_file_name("a\u{2028}b\u{2029}c\u{200b}d\u{feff}e\u{e0041}f.txt"),
        "abcdef.txt"
    );
    assert_eq!(plain_file_name("...hidden"), "hidden");
    assert_eq!(plain_file_name("  "), "download");
    assert_eq!(plain_file_name(&"x".repeat(500)).chars().count(), 120);
}

/// Answers `DOM.describeNode` of any element but the payment frame as an `<input>` of `kind`.
fn every_element_an_input(engine: &super::Engine, kind: &'static str) {
    engine.on(move |sent, _| {
        (sent.method == "DOM.describeNode"
            && sent.params["objectId"].is_null()
            && sent.params["backendNodeId"] != 50)
            .then(|| {
                Ok(
                    json!({"node":{"nodeId":0,"backendNodeId":sent.params["backendNodeId"],
                    "nodeType":1,"nodeName":"INPUT","localName":"input","nodeValue":"",
                    "attributes":["id","f","type",kind]}}),
                )
            })
    });
}

#[test]
fn an_upload_puts_the_files_given_into_the_file_input() {
    let (browser, engine) = browser();
    let field = ref_of(&snapshot(&browser, 1), "- slider \"Volume\"");
    every_element_an_input(&engine, "FILE");
    let files = vec![PathBuf::from("/data/browsers/b-1/uploads/1/notes.txt")];
    browser
        .call(
            &id(2),
            Call::Upload {
                target: field,
                files,
            },
        )
        .unwrap();
    let put = &engine.sent("DOM.setFileInputFiles")[0];
    assert_eq!(
        put.params["files"],
        json!(["/data/browsers/b-1/uploads/1/notes.txt"])
    );
    assert!(put.params["backendNodeId"].is_i64(), "{put:?}");
}

#[test]
fn an_upload_into_anything_but_a_file_input_is_refused_and_puts_nothing() {
    let (browser, engine) = browser();
    let field = ref_of(&snapshot(&browser, 1), "- slider \"Volume\"");
    every_element_an_input(&engine, "text");
    let answer = browser.call(
        &id(2),
        Call::Upload {
            target: field.clone(),
            files: vec![PathBuf::from("/x")],
        },
    );
    assert_eq!(answer, Err(Refusal::NotAFileInput(field)));
    assert!(engine.sent("DOM.setFileInputFiles").is_empty());
}

#[test]
fn a_download_that_takes_the_browser_past_what_its_downloads_may_hold_together_is_cancelled() {
    let (browser, engine) = browser_with(Limits {
        most_held_bytes: 1500,
        ..limits()
    });
    began(&engine, "GUID-9", "https://example.com/1", "one");
    progress(&engine, "GUID-9", 1000, 1000, "completed");
    began(&engine, "GUID-10", "https://example.com/2", "two");
    progress(&engine, "GUID-10", 600, 0, "inProgress");
    let listed = listed_once(&browser, |all| {
        all.get(1)
            .is_some_and(|d| d.state == DownloadState::TooLarge)
    });
    assert_eq!(listed[0].state, DownloadState::Done);
    assert_eq!(listed[1].state, DownloadState::TooLarge);
}
