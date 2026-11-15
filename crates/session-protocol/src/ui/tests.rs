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

/// A move's offer to raise a limit is the window's, as the save it goes with is (spec #4275).
#[test]
fn a_link_is_told_a_moves_failure_and_not_what_to_raise_beside_it() {
    let reply = serde_json::json!([{
        "session": 3,
        "why": "it needs 150 MiB",
        "limit": {"pool": "builds", "hits": [{"key": "profileSize"}]}
    }]);
    assert_eq!(
        as_a_link_is_served("chat_moves", reply),
        serde_json::json!([{"session": 3, "why": "it needs 150 MiB", "limit": null}])
    );
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

/// #5375: a workspace's own files (no repo named) are listed and opened in the person's window
/// alone, as `workspace_tree` and `workspace_file` are. A link is served a branch's paths, its
/// find and its watch, and refused each of those for a workspace's own files.
#[test]
fn a_call_that_names_a_workspaces_own_files_is_not_a_links_to_ask() {
    use serde_json::json;
    let path = |repo: &str| json!({"plane": "/p", "workspace": "alpha", "repo": repo, "piece": null, "path": "notes/a.md"});
    for method in ["copy_branch_path", "reveal_branch_path"] {
        assert!(served_on_a_link(method), "{method}");
        assert!(names_a_workspaces_own_files(method, &path("")), "{method}");
        assert!(
            !names_a_workspaces_own_files(method, &path("svc")),
            "{method}"
        );
        // A call that names no repo names none of a branch.
        assert!(
            names_a_workspaces_own_files(method, &json!({"workspace": "alpha"})),
            "{method}"
        );
    }

    let find = |scope: Value| json!({"session": 1, "scope": scope, "query": "plan"});
    assert!(names_a_workspaces_own_files(
        "find_files",
        &find(json!({"kind": "workspace-folder", "plane": "/p", "workspace": "alpha"}))
    ));
    assert!(names_a_workspaces_own_files(
        "find_files",
        &find(
            json!({"kind": "branch", "plane": "/p", "workspace": "alpha", "repo": "", "piece": null})
        )
    ));
    assert!(!names_a_workspaces_own_files(
        "find_files",
        &find(
            json!({"kind": "branch", "plane": "/p", "workspace": "alpha", "repo": "svc", "piece": null})
        )
    ));
    assert!(!names_a_workspaces_own_files(
        "find_files",
        &find(json!({"kind": "workspace", "plane": "/p", "workspace": "alpha", "near": null}))
    ));

    let folder = |repo: &str| json!({"plane": "/p", "workspace": "alpha", "repo": repo, "piece": null, "folder": "notes"});
    assert!(names_a_workspaces_own_files(
        "files_watch",
        &json!({"folders": [folder("svc"), folder("")]})
    ));
    assert!(!names_a_workspaces_own_files(
        "files_watch",
        &json!({"folders": [folder("svc")]})
    ));

    // A branch's read names a branch whatever its args say; the core refuses one with no repo.
    assert!(!names_a_workspaces_own_files("branch_tree", &path("")));
}

/// #5376: the person's own workspace files are written from their window alone. No link and
/// no chat makes, renames, deletes or imports one.
#[test]
fn a_workspaces_own_files_are_written_from_the_window_alone() {
    for command in [
        "workspace_files_new_file",
        "workspace_files_new_folder",
        "workspace_files_rename",
        "workspace_files_trash",
        "workspace_files_import",
        "workspace_files_import_dropped",
    ] {
        assert!(window_only(command), "{command}");
        assert!(!served_on_a_link(command), "{command}");
    }
}

/// Q251: **No sandbox is chosen only in the window on this Mac**: `open_repo` and
/// `create_project` asking for `protection: off` are not a link's to ask, whatever else they
/// say; any other level, or none, is the command's to answer.
#[test]
fn choosing_no_sandbox_at_setup_is_not_a_links_to_ask() {
    for method in ["open_repo", "create_project"] {
        assert!(served_on_a_link(method), "{method}");
        assert!(
            chooses_no_sandbox(
                method,
                &serde_json::json!({"path": "/home/dev/widget", "protection": "off"})
            ),
            "{method}"
        );
        for args in [
            serde_json::json!({"path": "/home/dev/widget", "protection": "floor"}),
            serde_json::json!({"path": "/home/dev/widget", "protection": "on"}),
            serde_json::json!({"path": "/home/dev/widget", "protection": null}),
            serde_json::json!({"path": "/home/dev/widget"}),
        ] {
            assert!(!chooses_no_sandbox(method, &args), "{method} {args}");
        }
    }
    assert!(!chooses_no_sandbox(
        "rename_chat",
        &serde_json::json!({"protection": "off"})
    ));
    // The sentence names no vault and no person: it is the same for every project.
    assert_eq!(
        NO_SANDBOX_IS_THE_WINDOWS,
        "No sandbox is chosen in purlis's window on this Mac."
    );
}
