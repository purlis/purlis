// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A browser opened from what an earlier one kept is signed in (#2021), through a real engine
//! on the fixture site: the site sets a cookie that outlives the browser, the browser ends, what
//! purlis keeps of its folder (everything but what it caches, its own home and its log) is all
//! the next browser starts from, and the site finds it signed in. The sealing itself is
//! `purlis_core::browserlogins`'s tests.
//!
//! These need a real Chrome and the right to start it and to listen on a loopback port (the
//! site's stand-in reach proxy), which a sandboxed agent does not have. They run where
//! `PURLIS_TEST_ENGINE` names a Chrome executable, and on CI against the runner's own Chrome.
//! Anywhere else each test says why it did not run and passes.

#![cfg(unix)]

use crate::common;

use std::path::Path;
use std::time::Duration;

use common::drive::{open, sees};
use common::site::started_on;

/// What purlis does not keep of a browser's folder: the list its kept logins are sealed by.
use purlis_browser_engine::launch::NOT_KEPT;

/// `from`'s files and folders, less what is not kept, into `to`: what the next browser opens.
fn kept(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap().flatten() {
        let name = entry.file_name();
        if NOT_KEPT.iter().any(|skip| name == *skip) {
            continue;
        }
        let kind = entry.file_type().unwrap();
        if kind.is_dir() {
            kept(&entry.path(), &to.join(&name));
        } else if kind.is_file() {
            std::fs::copy(entry.path(), to.join(&name)).unwrap();
        }
    }
}

#[test]
fn a_browser_opened_from_kept_logins_is_signed_in_and_a_fresh_one_is_not() {
    let base = tempfile::tempdir().unwrap();
    let first = base.path().join("first");
    let Some((browser, _site)) = started_on(&first, None) else {
        return;
    };
    open(&browser, "sign-in-kept");
    open(&browser, "kept.html");
    assert!(
        sees(&browser, "Signed in from kept logins"),
        "the cookie took"
    );
    browser.stop(Duration::from_secs(5));
    drop(browser);

    let next = base.path().join("next");
    kept(&first, &next);
    let (opened, _site) = started_on(&next, None).expect("an engine runs here");
    open(&opened, "kept.html");
    assert!(
        sees(&opened, "Signed in from kept logins"),
        "the site finds it signed in"
    );
    opened.stop(Duration::from_secs(5));

    let fresh = base.path().join("fresh");
    let (fresh, _site) = started_on(&fresh, None).expect("an engine runs here");
    open(&fresh, "kept.html");
    assert!(sees(&fresh, "Signed out"), "a fresh browser is not");
}
