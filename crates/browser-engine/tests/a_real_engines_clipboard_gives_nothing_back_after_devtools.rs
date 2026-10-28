// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A real engine's clipboard after DevTools (#2028): what was copied in the engine before Done
//! is not pasted after it, and the clipboard of the machine the engine runs on, the person's
//! own, is left as it was. Which clipboard the engine uses is said on CI, as a notice.
//!
//! These need a real Chrome and the right to start it and to listen on a loopback port (the
//! site's stand-in reach proxy), which a sandboxed agent does not have. They run where
//! `PURLIS_TEST_ENGINE` names a Chrome executable, and on CI against the runner's own Chrome.
//! Anywhere else each test says why it did not run and passes.

#![cfg(unix)]

mod common;

use std::io::Write;
use std::process::{Command, Stdio};

use common::drive::{call, find, open, ref_on, snapshot};
use common::site::started;
use purlis_browser_engine::drive::{Call, Field};

const COPIED: &str = "copied-in-the-engine-31";
const THEIRS: &str = "the-persons-own-clipboard-31";

/// The machine's own clipboard, where this machine has one a test can read: macOS's.
fn system_clipboard() -> Option<String> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    let out = Command::new("pbpaste").output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

fn set_system_clipboard(text: &str) -> bool {
    if !cfg!(target_os = "macos") {
        return false;
    }
    let Ok(mut child) = Command::new("pbcopy").stdin(Stdio::piped()).spawn() else {
        return false;
    };
    if let Some(stdin) = child.stdin.as_mut() {
        let _ = stdin.write_all(text.as_bytes());
    }
    child.wait().is_ok_and(|status| status.success())
}

/// Says `what` beside the run on CI, whatever the test harness keeps of its output.
fn notice(what: &str) {
    let line = if std::env::var("GITHUB_ACTIONS").as_deref() == Ok("true") {
        format!("::notice title=engine clipboard::{what}\n")
    } else {
        format!("{what}\n")
    };
    let _ = std::io::stderr().write_all(line.as_bytes());
}

/// What the page's paste field holds, as the page's tree reads it.
fn pasted(browser: &purlis_browser_engine::drive::Browser) -> String {
    snapshot(browser)
        .lines()
        .find(|line| line.contains("textbox \"Paste into\""))
        .unwrap_or_default()
        .to_owned()
}

#[test]
fn what_was_copied_is_not_pasted_after_devtools_and_the_machines_own_clipboard_is_left_alone() {
    let Some((browser, _data, _site)) = started() else {
        return;
    };
    let ours = set_system_clipboard(THEIRS);
    open(&browser, "two-fields.html");
    let from = ref_on(&find(&browser, "Copy from"), "textbox \"Copy from\"");
    call(
        &browser,
        Call::Type {
            page: None,
            fields: vec![Field {
                into: Some(from.clone()),
                text: COPIED.to_owned(),
                replace: true,
            }],
        },
    );
    let copy = ref_on(&find(&browser, "Copy"), "button \"Copy\"");
    call(&browser, Call::Click { target: copy });
    let read = snapshot(&browser);
    let copied = if read.contains("text \"yes\"") {
        "yes"
    } else if read.contains("text \"no\"") {
        "no"
    } else {
        "not said"
    };
    let system = system_clipboard();
    let engine_is_the_systems = system.as_deref() == Some(COPIED);
    notice(&format!(
        "on {}: the page's copy said {copied:?}; the engine's clipboard is {}",
        std::env::consts::OS,
        match (ours, engine_is_the_systems) {
            (false, _) => "not compared with a system clipboard here",
            (true, true) => "the system's",
            (true, false) => "its own, apart from the system's",
        }
    ));
    // A paste before DevTools gets what was copied: the clipboard works.
    let into = ref_on(&find(&browser, "Paste into"), "textbox \"Paste into\"");
    call(
        &browser,
        Call::Click {
            target: into.clone(),
        },
    );
    call(
        &browser,
        Call::Press {
            page: None,
            keys: vec!["Control+v".to_owned()],
        },
    );
    let mut before = pasted(&browser);
    if !before.contains(COPIED) {
        call(
            &browser,
            Call::Press {
                page: None,
                keys: vec!["Meta+v".to_owned()],
            },
        );
        before = pasted(&browser);
    }
    notice(&format!(
        "on {}: a paste before DevTools got {before:?}",
        std::env::consts::OS
    ));
    let paste_works = before.contains(COPIED);

    // The person opens DevTools and presses Done.
    browser.take_over();
    browser
        .inspect(1000, 700, Box::new(|_| {}))
        .expect("DevTools open for the person driving the browser");
    browser.hand_back();
    notice(&format!(
        "on {}: the clipboard was emptied at Done: {:?}",
        std::env::consts::OS,
        browser.devtools_clipboard_emptied()
    ));

    // A paste after Done gets nothing of it.
    let into = ref_on(&find(&browser, "Paste into"), "textbox \"Paste into\"");
    call(
        &browser,
        Call::Type {
            page: None,
            fields: vec![Field {
                into: Some(into.clone()),
                text: String::new(),
                replace: true,
            }],
        },
    );
    call(&browser, Call::Click { target: into });
    call(
        &browser,
        Call::Press {
            page: None,
            keys: vec!["Control+v".to_owned(), "Meta+v".to_owned()],
        },
    );
    let after = pasted(&browser);
    notice(&format!(
        "on {}: a paste after Done got {after:?}; pasting works here: {paste_works}",
        std::env::consts::OS
    ));
    assert!(
        !after.contains(COPIED),
        "a paste after Done got what was copied: {after}"
    );
    // The machine's own clipboard is the person's, and left as it was.
    if ours && !engine_is_the_systems {
        assert_eq!(system_clipboard().as_deref(), Some(THEIRS));
    }
    assert!(
        !engine_is_the_systems,
        "the engine shares the machine's own clipboard, so Done must not empty it"
    );
}
