// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The `sandbox local ports` row (#1699): an administrator's Claude Code setting that turns
//! local binding on is said, with the file that sets it; nothing is said where none does.

use std::path::PathBuf;

use super::{grants_held_row, kept_for, kept_row, local_ports_row, relayed_internal_row};
use crate::doctor::Row;
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

// The `sandbox kept files` row: said whenever the list of what purlis keeps from a chat is not
// whole, sandbox on or off, and where it is whole and has something to say.

fn machine_beside(root: &std::path::Path) -> crate::sandbox::Machine {
    crate::sandbox::Machine {
        env: crate::secrets::Env::of(&[]),
        home: Some(root.with_file_name("home")),
        os: crate::sandbox::Os::this(),
    }
}

/// A project's folder with a home beside it, and `registry` as its committed vault registry.
fn project_registering(registry: Option<&str>) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("a folder");
    let held = dir.path().canonicalize().expect("as the kernel names it");
    let root = held.join("project");
    std::fs::create_dir_all(&root).expect("a project");
    std::fs::create_dir_all(held.join("home")).expect("a home");
    if let Some(text) = registry {
        std::fs::write(root.join("vaults.json"), text).expect("the registry");
    }
    (dir, root)
}

#[test]
fn a_project_whose_list_is_whole_and_says_nothing_has_no_row() {
    for registry in [
        None,
        Some("{}"),
        Some(r#"{"vaults": {"box": {"provider": "keyring"}}}"#),
    ] {
        let (_dir, root) = project_registering(registry);
        for in_force in [true, false] {
            assert_eq!(
                kept_for(&root, &machine_beside(&root), in_force),
                None,
                "{registry:?}"
            );
        }
    }
}

#[test]
fn a_list_that_is_not_whole_is_a_warning_whether_or_not_chats_run_sandboxed() {
    let (_dir, root) = project_registering(Some("{ not json"));
    let file = root.join("vaults.json");

    let on = kept_for(&root, &machine_beside(&root), true).expect("a row");
    assert_eq!(on.name, "sandbox kept files");
    assert_eq!(on.status, Status::Warn);
    assert_eq!(
        on.detail,
        format!(
            "purlis cannot tell what to keep from a chat: the vault registry {} could not be \
             read",
            file.display()
        )
    );
    assert!(
        on.hint
            .starts_with("No sandboxed chat starts in this project, and its own files are not")
            && on.hint.contains("Fix the file named above")
            && on.hint.contains("purlis vault remove <name>"),
        "{}",
        on.hint
    );

    let off = kept_for(&root, &machine_beside(&root), false).expect("a row");
    assert_eq!((off.status, &off.detail), (Status::Warn, &on.detail));
    assert!(
        off.hint
            .starts_with("The project's own files are not listed in the window until"),
        "{}",
        off.hint
    );
}

#[test]
fn a_reason_the_registrys_own_row_reads_fine_for_is_said_here() {
    // The registry reads and every entry has a provider purlis implements.
    let (_dir, root) = project_registering(Some(
        r#"{"vaults": {"dev": {"provider": "plain-file", "config": {"file": 7}}}}"#,
    ));
    let row = kept_for(&root, &machine_beside(&root), false).expect("a row");
    assert_eq!(row.status, Status::Warn);
    assert!(
        row.detail.contains("the vault 'dev' in the vault registry")
            && row.detail.ends_with("does not say where its file is"),
        "{}",
        row.detail
    );
}

#[test]
fn what_a_whole_list_noted_is_said_and_a_vault_not_set_up_here_is_no_warning() {
    let (_dir, root) =
        project_registering(Some(r#"{"vaults": {"dev": {"provider": "plain-file"}}}"#));
    assert_eq!(
        kept_for(&root, &machine_beside(&root), true),
        Some(Row::ok(
            "sandbox kept files",
            "'dev' is not set up on this machine: it does not say where its file is"
        ))
    );

    let (_dir, root) = project_registering(Some(
        r#"{"vaults": {"gone": {}, "odd": {"provider": "carrier-pigeon"}}}"#,
    ));
    let row = kept_for(&root, &machine_beside(&root), true).expect("a row");
    assert_eq!(row.status, Status::Warn);
    assert_eq!(
        row.detail,
        "'gone' is left over on this machine: it names no kind of vault; 'odd' is a vault of a \
         kind this purlis does not know (carrier-pigeon): update purlis"
    );
    assert!(
        row.hint.starts_with("Chats start.") && row.hint.contains("purlis vault remove <name>"),
        "{}",
        row.hint
    );
}

#[test]
fn a_file_with_more_names_than_purlis_can_look_for_is_named_with_its_own_repair() {
    let untold = crate::sandbox::Untold {
        why: "the vault file /p/s/dev.json has more than one name, and this project holds more \
              files than purlis looks through for the others"
            .to_owned(),
        registry: None,
    };
    let row = kept_row(Some(&untold), &[], true).expect("a row");
    assert!(row.detail.contains("/p/s/dev.json"), "{}", row.detail);
    assert!(
        row.hint.contains("Leave the file named above one name"),
        "{}",
        row.hint
    );
}

#[test]
fn what_the_row_says_stays_on_one_line() {
    let untold = crate::sandbox::Untold {
        why: "a reason\nover two lines".to_owned(),
        registry: None,
    };
    let row = kept_row(Some(&untold), &[], true).expect("a row");
    assert!(!row.detail.contains('\n'), "{:?}", row.detail);
}

/// An entry of the policy's `relayed-internal` that is not taken is said on its own (#2530).
#[test]
fn a_relayed_internal_entry_not_taken_is_a_warning_naming_it() {
    let row = relayed_internal_row(&[
        "its relayed-internal entry \"10.0.0.5/8\" is not taken: it has bits set".to_owned(),
    ])
    .expect("a row");

    assert_eq!(row.name, "sandbox relayed internal");
    assert_eq!(row.status, Status::Warn);
    assert!(row.detail.contains("\"10.0.0.5/8\""), "{}", row.detail);
    assert_eq!(relayed_internal_row(&[]), None);
}
