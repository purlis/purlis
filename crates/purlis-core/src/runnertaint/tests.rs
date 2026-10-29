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
