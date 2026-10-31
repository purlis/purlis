// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn a_window_only_field_is_of_a_method_a_link_is_served_and_is_replaced_at_any_depth() {
    // A method of the window's alone is never served, so a field of it listed here would
    // say nothing.
    for kept in WINDOW_ONLY_FIELDS {
        assert!(!window_only(kept.method), "{}", kept.method);
    }
    let reply = serde_json::json!({
        "asks": [{"ask": "a", "writes": {"after": "x"}}, {"ask": "b"}],
        "nested": {"deeper": [{"writes": "y", "kept": 1}]},
        "none": {"writes": null}
    });
    let not_shown = an_edit_drawn_in_the_window_only();
    assert_eq!(
        as_a_link_is_served("pending_asks", reply.clone()),
        serde_json::json!({
            "asks": [{"ask": "a", "writes": not_shown}, {"ask": "b"}],
            "nested": {"deeper": [{"writes": not_shown, "kept": 1}]},
            "none": {"writes": null}
        })
    );
    assert_eq!(as_a_link_is_served("rename_chat", reply.clone()), reply);
}

/// Issue 1916: the folder watch is served on a link for a branch's folders, and refused
/// there for a folder of the plane root, which is named by no workspace and no repo.
#[test]
fn a_folder_watch_that_names_the_plane_root_is_not_a_links_to_ask() {
    let folder = |workspace: &str, repo: &str| {
        serde_json::json!({
            "plane": "/p", "workspace": workspace, "repo": repo, "piece": null, "folder": "docs"
        })
    };
    let watch = |folders: Vec<Value>| serde_json::json!({ "folders": folders });
    assert!(!window_only("files_watch"));
    assert!(!names_the_plane_root("files_watch", &watch(vec![])));
    assert!(!names_the_plane_root(
        "files_watch",
        &watch(vec![folder("alpha", "svc")])
    ));
    assert!(names_the_plane_root(
        "files_watch",
        &watch(vec![folder("", "")])
    ));
    // One such folder among a branch's refuses the whole call.
    assert!(names_the_plane_root(
        "files_watch",
        &watch(vec![folder("alpha", "svc"), folder("", "")])
    ));
    // A folder that leaves the names out names none.
    assert!(names_the_plane_root(
        "files_watch",
        &watch(vec![serde_json::json!({ "plane": "/p", "folder": "" })])
    ));
    assert!(!names_the_plane_root(
        "branch_tree",
        &watch(vec![folder("", "")])
    ));
    // A call that does not say its folders as a list names no branch.
    for args in [
        serde_json::json!({}),
        serde_json::json!([[folder("alpha", "svc")]]),
        serde_json::json!({ "folders": folder("alpha", "svc") }),
        Value::Null,
    ] {
        assert!(names_the_plane_root("files_watch", &args), "{args}");
    }
}

#[test]
fn a_host_serves_only_what_the_served_list_names_whatever_it_is_built_with() {
    // #1927: a command on neither list (one added to the app and not yet listed) is
    // refused to a link at the host, not only by a test of the lists.
    struct Nothing;
    impl Handler for Nothing {
        fn call(&self, _: String, _: Value) -> BoxFuture<'_, Result<Value, Value>> {
            Box::pin(async { Ok(Value::Null) })
        }
    }
    let server = Server::new(
        "0.0.0+test",
        [
            "rename_chat",
            "a_command_added_later",
            "send_input",
            "answer_ask",
        ],
        Nothing,
    );
    assert_eq!(
        server
            .methods
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["rename_chat"]
    );
}

#[test]
fn no_command_is_both_served_on_a_link_and_the_windows_alone() {
    // #1927: the two lists part the app's commands. The app's own test holds that every
    // command is on one; this holds that none is on both, and each list names one once.
    for command in SERVED_ON_A_LINK {
        assert!(!window_only(command), "{command} is on both lists");
        assert!(served_on_a_link(command), "{command}");
    }
    for command in window_only_commands() {
        assert!(!served_on_a_link(command), "{command}");
    }
    let mut once = SERVED_ON_A_LINK.to_vec();
    once.sort_unstable();
    once.dedup();
    assert_eq!(
        once.len(),
        SERVED_ON_A_LINK.len(),
        "a command is listed twice"
    );
}

#[test]
fn every_command_of_who_may_dispatch_to_whom_is_on_the_window_only_list() {
    for command in STANDING_DISPATCH {
        assert!(WINDOW_ONLY.contains(command), "{command}");
    }
    // Named once each.
    let mut sorted = STANDING_DISPATCH.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), STANDING_DISPATCH.len());
}
