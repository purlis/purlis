// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The `sandbox local ports` row (#1699): an administrator's Claude Code setting that turns
//! local binding on is said, with the file that sets it; nothing is said where none does.

use std::path::PathBuf;

use super::{grants_held_row, local_ports_row};
use crate::doctor::Status;
use crate::sandbox::HeldGrant;
use crate::sandbox::claude::LocalBinding;
use crate::sandbox::grant::{Level, What};

#[test]
fn an_administrators_local_binding_is_said_with_its_file() {
    let file = PathBuf::from("/managed/managed-settings.json");
    let row = local_ports_row(&LocalBinding::On(file)).expect("a row");
    assert_eq!(row.name, "sandbox local ports");
    assert_eq!(
        row.status,
        Status::Ok,
        "an administrator's choice, not a fault"
    );
    assert!(
        row.detail.contains("/managed/managed-settings.json")
            && row.detail.contains("every port on this machine"),
        "{}",
        row.detail
    );
}

#[test]
fn a_managed_file_purlis_cannot_read_is_a_warning() {
    let row = local_ports_row(&LocalBinding::Unread {
        file: PathBuf::from("/managed/10-x.json"),
        why: "permission denied".to_owned(),
    })
    .expect("a row");
    assert_eq!(row.status, Status::Warn);
    assert!(row.detail.contains("/managed/10-x.json"), "{}", row.detail);
}

#[test]
fn nothing_is_said_where_no_administrator_turns_it_on() {
    assert_eq!(local_ports_row(&LocalBinding::Off), None);
}

// The `sandbox grants held` row (#1927): what this machine keeps that policy holds is said,
// with the policy's own sentence and how to take it away; nothing is said where nothing is.

fn held(what: What, level: Level) -> HeldGrant {
    HeldGrant {
        what,
        level,
        why: format!(
            "Held by policy, which leaves out what was allowed {}. Locked by policy, set by IT \
             in /etc/purlis/policy.json.",
            level.said()
        ),
    }
}

#[test]
fn what_policy_holds_is_counted_per_scope_with_the_policy_s_sentence() {
    let row = grants_held_row(&[
        held(
            What::Socket(PathBuf::from("/var/run/docker.sock")),
            Level::Machine,
        ),
        held(
            What::Service("com.example.helper".to_owned()),
            Level::Machine,
        ),
        held(What::Write(PathBuf::from("/opt/cache")), Level::You),
    ])
    .expect("a row");
    assert_eq!(row.name, "sandbox grants held");
    assert_eq!(
        row.status,
        Status::Ok,
        "an administrator's choice, not a fault"
    );
    assert!(
        row.detail
            .starts_with("3 allowed on this machine reach no chat while policy holds them"),
        "{}",
        row.detail
    );
    assert!(
        row.detail
            .contains("1 for this project, 2 for every project"),
        "{}",
        row.detail
    );
    // Each of the policy's sentences once, and who set it.
    assert_eq!(
        row.detail
            .matches("on this machine, for every project. Locked by policy, set by IT")
            .count(),
        1,
        "{}",
        row.detail
    );
    assert!(
        row.detail.contains("for me on this machine"),
        "{}",
        row.detail
    );
    // It names nothing a repository or a chat could have written into the sentence.
    assert!(!row.detail.contains("docker.sock"), "{}", row.detail);
    assert!(row.detail.contains("Settings"), "{}", row.detail);
}

#[test]
fn nothing_is_said_where_policy_holds_nothing() {
    assert_eq!(grants_held_row(&[]), None);
}
