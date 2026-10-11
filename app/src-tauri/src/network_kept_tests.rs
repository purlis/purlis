// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A person's Keep blocked, as the network record keeps it and the Inbox's look back reads it
//! (#1700).

use super::*;

fn chat(name: &str) -> record::Chat {
    record::Chat {
        id: Some(format!("id-{name}")),
        name: Some(name.to_owned()),
        session: Some(3),
    }
}

#[test]
fn each_keep_blocked_is_listed_newest_first_with_its_chat() {
    let entries = vec![
        Entry::kept_blocked("api.example.com:443", chat("fix the build"), None, 100),
        Entry::kept_blocked("pypi.org:443", chat("tidy"), Some("steward"), 200),
    ];
    assert_eq!(
        kept_blocked_of(&entries),
        vec![
            KeptBlocked {
                target: "pypi.org:443".into(),
                chat: Some("tidy".into()),
                at: 200,
            },
            KeptBlocked {
                target: "api.example.com:443".into(),
                chat: Some("fix the build".into()),
                at: 100,
            },
        ]
    );
}

#[test]
fn an_allow_an_ask_and_a_block_are_never_listed_as_kept_blocked() {
    let entries = vec![
        Entry::allowed(
            record::What::Host,
            "api.example.com:443",
            record::Scope::Chat,
            chat("fix the build"),
            None,
            100,
        ),
        Entry::connected(
            Some("api.example.com:443"),
            record::ASKED,
            1,
            chat("fix the build"),
            None,
            100,
        ),
        Entry::connected(
            Some("api.example.com:443"),
            record::TIMED_OUT,
            1,
            chat("fix the build"),
            None,
            100,
        ),
    ];
    assert_eq!(kept_blocked_of(&entries), Vec::new());
}

#[test]
fn at_most_the_newest_are_listed() {
    let entries: Vec<Entry> = (0..(AT_MOST_LISTED as u64 + 5))
        .map(|at| Entry::kept_blocked("api.example.com:443", chat("one"), None, at))
        .collect();
    let listed = kept_blocked_of(&entries);
    assert_eq!(listed.len(), AT_MOST_LISTED);
    assert_eq!(listed[0].at, AT_MOST_LISTED as u32 + 4);
}

#[test]
fn a_keep_blocked_is_written_to_the_record_and_read_back_for_the_inbox() {
    let dir = tempfile::tempdir().expect("a directory");
    let root = dir.path().join("project");
    std::fs::create_dir_all(&root).expect("a project");
    let record = Record::in_data(&dir.path().join("data"));
    let chats = crate::chats::Chats::new();

    record_kept_blocked(Some(&record), &root, &chats, 4, "paste.example:8443");
    // A folder is no host: nothing is kept for it.
    record_kept_blocked(Some(&record), &root, &chats, 4, "/w/out");

    let listed = kept_blocked_of(&record.read(&root, now_secs()));
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].target, "paste.example:8443");
}
