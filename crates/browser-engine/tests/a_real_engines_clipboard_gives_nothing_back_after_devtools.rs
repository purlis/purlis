// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A real engine's clipboard and DevTools (#2028): the engine's clipboard is one for all its
//! browser contexts, apart from the machine's, so what is copied in DevTools' own context, as
//! the person's copy in DevTools is, would be what the agent's paste gets: once DevTools were
//! open, the agent's pastes are withheld, and the clipboard of the machine the engine runs on,
//! the person's own, is left as it was. Which clipboard the engine uses is said on CI, as a
//! notice.
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

/// What the page's field `named` holds, as the page's tree reads it.
fn pasted(browser: &purlis_browser_engine::drive::Browser, named: &str) -> String {
    let field = format!("textbox \"{named}\"");
    snapshot(browser)
        .lines()
        .find(|line| line.contains(&field))
        .unwrap_or_default()
        .to_owned()
}

#[test]
fn what_is_copied_in_devtools_is_never_pasted_by_the_agent_and_the_machines_own_clipboard_is_left_alone()
 {
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
    let mut before = pasted(&browser, "Paste into");
    if !before.contains(COPIED) {
        call(
            &browser,
            Call::Press {
                page: None,
                keys: vec!["Meta+v".to_owned()],
            },
        );
        before = pasted(&browser, "Paste into");
    }
    notice(&format!(
        "on {}: a paste before DevTools got {before:?}",
        std::env::consts::OS
    ));
    let paste_works = before.contains(COPIED);

    // The person copies in DevTools, opens them on the page and presses Done.
    let theirs = "copied-in-devtools-77";
    assert!(
        browser.copy_in_a_context_of_devtools(theirs),
        "the copy in DevTools' context ran"
    );
    browser.take_over();
    browser
        .inspect(1000, 700, Box::new(|_| {}))
        .expect("DevTools open for the person driving the browser");
    browser.hand_back();

    // The engine's clipboard holds what was copied in DevTools: a paste after Done is withheld.
    let into = ref_on(&find(&browser, "Paste after"), "textbox \"Paste after\"");
    call(&browser, Call::Click { target: into });
    let withheld = browser.call(
        &purlis_browser_engine::drive::CallId::new("paste-after").unwrap(),
        Call::Press {
            page: None,
            keys: vec!["Control+v".to_owned()],
        },
    );
    let after = pasted(&browser, "Paste after");
    notice(&format!(
        "on {}: pasting works here: {paste_works}; a paste after Done answered {:?} and the field holds {after:?}",
        std::env::consts::OS,
        withheld.as_ref().err()
    ));
    assert_eq!(
        withheld.err(),
        Some(purlis_browser_engine::drive::Refusal::PasteWithheld)
    );
    assert!(
        !after.contains(theirs),
        "a paste after Done got what was copied in DevTools: {after}"
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
