// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn a_removal_is_held_where_the_sandbox_holds_this_process_whatever_it_says() {
    // Either answer holds it.
    assert!(removal_held_by(false, || true));
    assert!(removal_held_by(true, || false));
    assert!(removal_held_by(true, || true));
    // Neither: a terminal, or a chat the app started without the sandbox.
    assert!(!removal_held_by(false, || false));
}

#[test]
fn a_create_the_app_cannot_finish_for_a_chat_is_refused_naming_the_way() {
    assert_eq!(create_refused("beta", false, false), None);
    let live = create_refused("beta", true, false).expect("refused");
    assert!(live.contains("nothing was created"), "{live}");
    assert!(live.contains("purlis workspace live beta"), "{live}");
    let uses = create_refused("beta", false, true).expect("refused");
    assert!(uses.contains("nothing was created"), "{uses}");
    assert!(uses.contains("-w beta"), "{uses}");
    assert!(uses.contains("purlis handoff beta"), "{uses}");
}

#[test]
fn a_branch_folder_a_chat_cannot_remove_is_named_by_its_palette_row() {
    let said = no_piece_removal_in_a_sandbox("charter", "probe");
    assert!(said.contains("nothing was removed"), "{said}");
    assert!(
        said.ends_with("Remove folder probe in charter, in the palette"),
        "{said}"
    );
}

#[test]
fn a_forced_removal_from_a_chat_names_the_window_and_the_plain_one() {
    let said = no_forced_piece_removal_in_a_chat("charter", "probe");
    assert!(said.contains("nothing was removed"), "{said}");
    assert!(
        said.contains("Without --force the app removes it"),
        "{said}"
    );
    assert!(
        said.ends_with("Remove folder probe in charter, in the palette"),
        "{said}"
    );
}

#[test]
fn its_own_workspace_spelled_out_is_taken_as_no_w() {
    assert_eq!(named(None, Some("alpha"), true), Named::Own);
    assert_eq!(named(Some("alpha"), Some("alpha"), true), Named::Own);
    assert_eq!(named(Some("alpha"), Some("alpha"), false), Named::Own);
    assert_eq!(named(None, None, false), Named::Own);
}

#[test]
fn a_chat_at_the_project_root_writes_any_workspace_here_as_before() {
    // Its sandbox writes every workspace's ordinary files, so nothing is refused.
    assert_eq!(named(Some("beta"), None, true), Named::Here);
    assert_eq!(named(Some("beta"), None, false), Named::Here);
}

#[test]
fn another_workspace_is_written_here_unless_the_chat_is_sandboxed() {
    assert_eq!(named(Some("beta"), Some("alpha"), false), Named::Here);
    let Named::Refused(why) = named(Some("beta"), Some("alpha"), true) else {
        panic!("a sandboxed chat naming another workspace was let through");
    };
    assert!(why.contains("'beta' is not the one it works in"), "{why}");
    assert!(why.contains("-w alpha"), "{why}");
    assert!(why.contains("nothing was changed"), "{why}");
}
