// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **`purlis inbox` lists, denies and dismisses, and has no allow verb** (M75 slice 19, #7028;
//! spec §8.5, Q314.5): a headless machine loosens only through the Organisation, so the command
//! that answers needs-you from a terminal can refuse and clear, never allow.

use crate::support;
use purlis_core::policy::needsyou::{Caps, Key, Raise, Store, Why, digest_of};

const NOW: u64 = 1_800_000_000;

fn raise(store: &Store, request: &str, chat: &str) {
    let digest = digest_of("merge", "acme/web#main", request);
    store
        .raise(
            Raise {
                key: Key::new("guards-merge-hold", "acme/web#main", "web", &digest),
                action: "merge",
                said: request,
                request,
                why: Why::NoWindow,
                chat,
            },
            NOW,
            Caps::shipped(),
        )
        .expect("raised");
}

fn inbox(data: &std::path::Path, args: &[&str]) -> std::process::Output {
    // Run from nowhere: no plane is looked for, and none is the suite's to touch.
    let nowhere = tempfile::tempdir().expect("a directory");
    let mut command = support::purlis();
    support::at_a_terminal(&mut command)
        .current_dir(nowhere.path())
        .env(purlis_core::datahome::HOME_VAR, data)
        .arg("inbox")
        .args(args)
        .output()
        .expect("purlis runs")
}

fn said(out: &std::process::Output) -> (String, String) {
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn purlis_inbox_lists_what_is_waiting_with_its_chat_count() {
    let data = tempfile::tempdir().expect("data");
    let store = Store::in_data(data.path());
    raise(&store, "merge acme/web#7", "c1");
    raise(&store, "merge acme/web#7", "c2");
    let out = inbox(data.path(), &["list"]);
    let (stdout, stderr) = said(&out);
    assert!(out.status.success(), "{stderr}");
    assert!(stdout.contains("merge acme/web#7"), "{stdout}");
    assert!(stdout.contains("2 chats"), "{stdout}");
    assert!(stdout.contains("no window reachable"), "{stdout}");
}

#[test]
fn purlis_inbox_says_so_when_nothing_waits() {
    let data = tempfile::tempdir().expect("data");
    let (stdout, _) = said(&inbox(data.path(), &["list"]));
    assert!(stdout.contains("Nothing is waiting"), "{stdout}");
}

#[test]
fn purlis_inbox_denies_an_item_by_the_start_of_its_id() {
    let data = tempfile::tempdir().expect("data");
    let store = Store::in_data(data.path());
    raise(&store, "merge acme/web#7", "c1");
    let id = store.read(NOW, Caps::shipped()).expect("read").items()[0]
        .key
        .id();
    let out = inbox(data.path(), &["deny", &id[..6]]);
    assert!(out.status.success(), "{}", said(&out).1);
    assert!(
        store
            .read(NOW, Caps::shipped())
            .expect("read")
            .items()
            .is_empty()
    );
    // The chat is told it was denied, at its next call.
    let told = store.take_told("c1", NOW, Caps::shipped()).expect("read");
    assert_eq!(told.len(), 1);
    assert!(told[0].words().contains("denied"), "{}", told[0].words());
}

#[test]
fn purlis_inbox_dismisses_an_item_and_tells_nobody() {
    let data = tempfile::tempdir().expect("data");
    let store = Store::in_data(data.path());
    raise(&store, "merge acme/web#7", "c1");
    let id = store.read(NOW, Caps::shipped()).expect("read").items()[0]
        .key
        .id();
    let out = inbox(data.path(), &["dismiss", &id]);
    assert!(out.status.success(), "{}", said(&out).1);
    assert!(
        store
            .read(NOW, Caps::shipped())
            .expect("read")
            .items()
            .is_empty()
    );
    assert!(
        store
            .take_told("c1", NOW, Caps::shipped())
            .expect("read")
            .is_empty()
    );
}

#[test]
fn purlis_inbox_names_what_it_could_not_find() {
    let data = tempfile::tempdir().expect("data");
    let out = inbox(data.path(), &["deny", "zzzz"]);
    assert!(!out.status.success());
    assert!(
        said(&out).1.contains("purlis inbox list"),
        "{}",
        said(&out).1
    );
}

#[test]
fn purlis_inbox_has_no_allow_verb() {
    let data = tempfile::tempdir().expect("data");
    let store = Store::in_data(data.path());
    raise(&store, "merge acme/web#7", "c1");
    let id = store.read(NOW, Caps::shipped()).expect("read").items()[0]
        .key
        .id();
    for verb in ["allow", "approve", "grant", "accept"] {
        let out = inbox(data.path(), &[verb, &id]);
        assert!(!out.status.success(), "{verb} was taken");
        assert!(
            said(&out).1.contains("unrecognized subcommand"),
            "{}",
            said(&out).1
        );
    }
    // And its help names no way to allow.
    let (help, _) = said(&inbox(data.path(), &["--help"]));
    assert!(help.contains("deny") && help.contains("dismiss"), "{help}");
    assert!(!help.to_lowercase().contains("  allow"), "{help}");
    assert_eq!(
        store
            .read(NOW, Caps::shipped())
            .expect("read")
            .items()
            .len(),
        1,
        "nothing was answered"
    );
}
