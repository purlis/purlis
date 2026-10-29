// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use purlis_session_protocol::session::{ProgramKind, RunSandbox};

fn container(digit: char) -> ContainerId {
    ContainerId::new(&digit.to_string().repeat(64)).unwrap()
}

#[test]
fn a_chat_with_no_sandbox_that_holds_its_files_taints_its_runner_and_a_shell_or_browser_never() {
    let by = |backend: &str| RunSandbox::By(backend.to_owned());
    assert!(taints(ProgramKind::Harness, Some(&RunSandbox::Off)));
    assert!(
        taints(ProgramKind::Harness, Some(&by("proxy"))),
        "the proxy holds no files"
    );
    assert!(
        taints(ProgramKind::Harness, None),
        "a host that does not say"
    );
    for held in ["landlock", "bubblewrap", "seatbelt"] {
        assert!(!taints(ProgramKind::Harness, Some(&by(held))), "{held}");
    }
    assert!(
        !taints(ProgramKind::Shell, Some(&RunSandbox::Off)),
        "a person's shell"
    );
    assert!(!taints(ProgramKind::Browser, None));
}

#[test]
fn a_runner_a_chat_ran_on_without_the_sandbox_is_kept_as_one_and_no_other() {
    let config = tempfile::tempdir().unwrap();
    assert!(
        !ran_unsandboxed(config.path(), &container('a')),
        "none kept yet"
    );
    mark(config.path(), &container('a')).unwrap();
    assert!(ran_unsandboxed(config.path(), &container('a')));
    assert!(!ran_unsandboxed(config.path(), &container('b')));
}

#[test]
fn a_record_that_cannot_be_read_counts_every_runner_as_tainted() {
    let config = tempfile::tempdir().unwrap();
    let folder =
        purlis_same_user::PrivateDirectory::open(&crate::machine::dir(config.path())).unwrap();
    for text in [
        "not json",
        r#"{"version": 2, "containers": []}"#,
        r#"{"version": 1, "containers": ["short"]}"#,
        r#"{"version": 1, "containers": [], "more": true}"#,
    ] {
        folder.replace_file(FILE, text.as_bytes()).unwrap();
        assert!(ran_unsandboxed(config.path(), &container('c')), "{text}");
    }
}

#[test]
fn a_runner_no_longer_kept_is_let_go_of_as_another_is_marked() {
    let config = tempfile::tempdir().unwrap();
    mark(config.path(), &container('a')).unwrap();
    mark(config.path(), &container('b')).unwrap();
    // Neither runner is kept in `runners.json`: only the one marked last is kept.
    assert!(!ran_unsandboxed(config.path(), &container('a')));
    assert!(ran_unsandboxed(config.path(), &container('b')));
}

const IMAGE: &str = "ghcr.io/purlis/purlis-runner@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

/// Keeps pools `first` and `second` in `runners.json`: `first`'s runner of container `a`
/// draining after an update beside its new one of `b`, and `second`'s of `c`.
fn keep_runners(config: &Path) {
    let row = |name: &str, container: char, extra: serde_json::Value| {
        let mut row = serde_json::json!({
            "name": name,
            "pool": "docker",
            "container": container.to_string().repeat(64),
            "image": IMAGE,
            "user": "10001:10001",
            "home": "/home/purlis",
            "device": format!("01JB0RUNNER000000000000000{}", name.len() % 10),
            "key": "12".repeat(32),
            "paired": 1,
        });
        for (key, value) in extra.as_object().unwrap() {
            row[key] = value.clone();
        }
        row
    };
    let text = serde_json::json!({
        "version": 1,
        "pools": [
            { "name": "first", "kind": "docker", "image": IMAGE },
            { "name": "second", "kind": "docker", "image": IMAGE },
        ],
        "runners": [
            row("first", 'a', serde_json::json!({ "draining": true })),
            row("first-01234567", 'b', serde_json::json!({ "of": "first" })),
            row("second", 'c', serde_json::json!({})),
        ],
    });
    purlis_same_user::PrivateDirectory::open(&crate::machine::dir(config))
        .unwrap()
        .replace_file(crate::machine::RUNNERS_FILE, text.to_string().as_bytes())
        .unwrap();
}

fn the_record(config: &Path) -> Option<String> {
    std::fs::read_to_string(crate::machine::dir(config).join(FILE)).ok()
}

/// A link to a pool reaches whichever of its runners holds its chat, an old one still draining
/// after an update among them, and the window is not told which (#3702): a chat that ran there
/// without the sandbox marks every runner the pool keeps, and a runner of the pool marked leaves
/// the whole pool untrusted.
#[test]
fn a_chat_with_no_sandbox_on_a_pool_marks_every_runner_it_keeps_the_draining_ones_too() {
    let config = tempfile::tempdir().unwrap();
    keep_runners(config.path());
    assert!(!pool_ran_unsandboxed(config.path(), "first"));

    mark_pool(config.path(), "first").unwrap();
    assert!(
        ran_unsandboxed(config.path(), &container('a')),
        "the draining one"
    );
    assert!(
        ran_unsandboxed(config.path(), &container('b')),
        "the new one"
    );
    assert!(
        !ran_unsandboxed(config.path(), &container('c')),
        "another pool's"
    );
    assert!(pool_ran_unsandboxed(config.path(), "first"));
    assert!(!pool_ran_unsandboxed(config.path(), "second"));
    assert!(
        mark_pool(config.path(), "none").is_err(),
        "a pool with no runner kept"
    );
}

/// An update while the old runner still drains: a chat that ran without the sandbox on the old
/// one, before the update, leaves the pool untrusted though its new runner was never marked.
#[test]
fn an_old_runner_still_draining_that_was_marked_leaves_its_pool_untrusted_after_an_update() {
    let config = tempfile::tempdir().unwrap();
    keep_runners(config.path());
    mark(config.path(), &container('a')).unwrap();
    assert!(!ran_unsandboxed(config.path(), &container('b')));
    assert!(pool_ran_unsandboxed(config.path(), "first"));
    assert!(!pool_ran_unsandboxed(config.path(), "second"));
}

/// Neither record is ever written over where one cannot be read, and a record of runners that
/// cannot be read leaves every pool untrusted (#3702).
#[test]
fn a_mark_is_refused_and_nothing_written_where_either_record_cannot_be_read() {
    let config = tempfile::tempdir().unwrap();
    let folder =
        purlis_same_user::PrivateDirectory::open(&crate::machine::dir(config.path())).unwrap();

    folder
        .replace_file(crate::machine::RUNNERS_FILE, b"not json")
        .unwrap();
    assert!(mark(config.path(), &container('a')).is_err());
    assert!(mark_pool(config.path(), "first").is_err());
    assert_eq!(the_record(config.path()), None, "nothing written");
    assert!(pool_ran_unsandboxed(config.path(), "first"));

    keep_runners(config.path());
    let unreadable = r#"{"version": 2, "containers": []}"#;
    folder.replace_file(FILE, unreadable.as_bytes()).unwrap();
    assert!(mark(config.path(), &container('a')).is_err());
    assert!(mark_pool(config.path(), "first").is_err());
    assert_eq!(the_record(config.path()).as_deref(), Some(unreadable));
    assert!(pool_ran_unsandboxed(config.path(), "second"), "every pool");
}
