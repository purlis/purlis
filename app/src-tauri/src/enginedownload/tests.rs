// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

const MB: u64 = 1024 * 1024;

#[test]
fn a_download_says_how_far_it_has_come_in_megabytes_of_its_whole_where_known() {
    assert_eq!(
        says(46 * MB, Some(152 * MB)),
        "Downloading the browser engine… 46 of 152 MB."
    );
    assert_eq!(
        says(3 * MB, None),
        "Downloading the browser engine… 3 MB so far."
    );
    assert_eq!(
        says(0, Some(152 * MB)),
        "Downloading the browser engine… 0 of 152 MB."
    );
}

#[test]
fn a_download_is_shown_from_its_first_word_until_its_start_answers() {
    let downloads = Downloads::default();
    assert!(downloads.shown().is_empty());

    downloads.heard("b-2", MB, Some(10 * MB));
    downloads.heard("b-1", 2 * MB, None);
    downloads.heard("b-2", 5 * MB, Some(10 * MB));

    assert_eq!(
        downloads.shown(),
        [
            EngineDownloadShown {
                program: "b-1".into(),
                says: "Downloading the browser engine… 2 MB so far.".into(),
            },
            EngineDownloadShown {
                program: "b-2".into(),
                says: "Downloading the browser engine… 5 of 10 MB.".into(),
            },
        ]
    );
    downloads.finished("b-2");
    assert_eq!(downloads.shown().len(), 1, "its start answered");
    downloads.finished("b-1");
    assert!(downloads.shown().is_empty());
}

#[test]
fn each_change_is_told_to_the_window() {
    let told = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counting = std::sync::Arc::clone(&told);
    let downloads = Downloads::telling(Box::new(move || {
        counting.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }));
    downloads.heard("b-1", MB, None);
    downloads.finished("b-1");
    downloads.finished("b-1");
    assert_eq!(
        told.load(std::sync::atomic::Ordering::SeqCst),
        2,
        "a finish of nothing tells nothing"
    );
}
