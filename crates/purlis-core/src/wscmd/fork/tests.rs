// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn plane() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("charter.toml"), "").unwrap();
    std::fs::create_dir_all(dir.path().join(".claude")).unwrap();
    std::fs::write(
        dir.path().join(".claude").join("settings.json"),
        r#"{"env":{"CHARTER_HARNESS":"claude-code"}}"#,
    )
    .unwrap();
    std::fs::create_dir_all(dir.path().join("workspaces")).unwrap();
    dir
}

fn now() -> chrono::DateTime<chrono::Utc> {
    "2026-05-04T11:32:17Z".parse().unwrap()
}

fn run(root: &Path, src: &str, new: &str, live: bool) -> (u8, Vec<String>) {
    let mut lines = Vec::new();
    let code = fork(
        &Request {
            root,
            src,
            new,
            live,
            restore: false,
            now: now(),
            extension_folders: &[],
        },
        &mut |s| lines.push(s.to_string()),
    );
    (code, lines)
}

fn a_parent(root: &Path, name: &str) {
    ensure::ensure(root, name, now(), "fixture").unwrap();
    let plane = crate::workspaces::Plane::open(root);
    let ws = plane.workspace(name).unwrap();
    ws.set_vision("Ship the widget").unwrap();
    ws.remember("The API returns 418 on Mondays", now().naive_utc())
        .unwrap();
    ws.add_todo("Write the migration", now().naive_utc())
        .unwrap();
}

#[test]
fn a_fork_inherits_the_charter_the_memory_and_the_open_todos() {
    let dir = plane();
    a_parent(dir.path(), "alpha");
    let (code, lines) = run(dir.path(), "alpha", "gamma", false);
    assert_eq!(code, 0, "{lines:?}");
    let gamma = dir.path().join("workspaces/gamma");
    let charter = std::fs::read_to_string(gamma.join("workspace.md")).unwrap();
    assert!(charter.contains("Ship the widget"), "{charter}");
    // The parent's memory, and the fork's own note beside it.
    let index = std::fs::read_to_string(gamma.join("memory/MEMORY.md")).unwrap();
    assert!(index.contains("The API returns 418 on Mondays"), "{index}");
    assert!(index.contains("Forked from 'alpha'"), "{index}");
    let todos = std::fs::read_to_string(gamma.join("todos/MEMORY.md")).unwrap();
    assert!(todos.contains("Write the migration"), "{todos}");
    assert!(
        lines.iter().any(|l| l.contains("Inherited 1 open todo")),
        "{lines:?}"
    );
}

#[test]
fn an_extensions_folder_is_carried_and_a_clone_that_shares_its_name_is_not() {
    let dir = plane();
    a_parent(dir.path(), "alpha");
    let alpha = dir.path().join("workspaces/alpha");
    std::fs::create_dir_all(alpha.join("notes")).unwrap();
    std::fs::write(alpha.join("notes/kept.md"), "the extension's").unwrap();
    std::fs::create_dir_all(alpha.join("svc/.git")).unwrap();
    std::fs::write(alpha.join("svc/code.rs"), "the operator's").unwrap();

    let mut lines = Vec::new();
    let code = fork(
        &Request {
            root: dir.path(),
            src: "alpha",
            new: "gamma",
            live: false,
            restore: false,
            now: now(),
            extension_folders: &["absent".to_owned(), "notes".to_owned(), "svc".to_owned()],
        },
        &mut |s| lines.push(s.to_string()),
    );

    assert_eq!(code, 0, "{lines:?}");
    let gamma = dir.path().join("workspaces/gamma");
    assert_eq!(
        std::fs::read_to_string(gamma.join("notes/kept.md")).unwrap(),
        "the extension's"
    );
    assert!(!gamma.join("svc").exists(), "a clone was copied");
    assert!(!gamma.join("absent").exists());
    assert!(
        lines
            .iter()
            .any(|l| l.contains("Carried the folder extensions keep in 'alpha': notes/")),
        "{lines:?}"
    );
}

#[test]
fn the_two_todo_lists_diverge_from_the_fork() {
    let dir = plane();
    a_parent(dir.path(), "alpha");
    run(dir.path(), "alpha", "gamma", false);
    let plane = crate::workspaces::Plane::open(dir.path());
    // Closing one in the fork must not rewrite the parent's plan.
    let slug = plane.workspace("gamma").unwrap().todos().unwrap()[0]
        .slug
        .clone();
    plane
        .workspace("gamma")
        .unwrap()
        .close_todo(&slug, now().naive_utc())
        .unwrap();
    assert_eq!(plane.workspace("gamma").unwrap().todos().unwrap().len(), 0);
    assert_eq!(plane.workspace("alpha").unwrap().todos().unwrap().len(), 1);
}

#[test]
fn the_forks_manifest_names_the_parent_and_keeps_the_key_order_it_read() {
    let dir = plane();
    a_parent(dir.path(), "alpha");
    let alpha = dir.path().join("workspaces/alpha/workspace.json");
    std::fs::write(
        &alpha,
        r#"{
  "name": "alpha",
  "description": "the widget",
  "repos": [
    {
      "name": "svc",
      "branch": "main"
    }
  ],
  "updated_at": "2026-03-02T09:00:00+00:00",
  "updated_by": "fixture",
  "charter_generated": "0000000000000000000000000000000000000000000000000000000000000000"
}
"#,
    )
    .unwrap();
    run(dir.path(), "alpha", "gamma", false);
    let text = std::fs::read_to_string(dir.path().join("workspaces/gamma/workspace.json")).unwrap();
    let keys: Vec<&str> = text
        .lines()
        .filter_map(|l| l.trim().strip_prefix('"'))
        .filter_map(|l| l.split_once("\": "))
        .map(|(k, _)| k)
        .collect();
    // charter assigns into the dict it read, so a key that is already there keeps its
    // position and `forked_from` — the one key a fork adds — lands last.
    assert_eq!(
        keys.first().copied(),
        Some("name"),
        "the parent's own key order is kept: {text}"
    );
    assert_eq!(keys.last().copied(), Some("forked_from"), "{text}");
    assert!(text.contains("\"description\": \"the widget\""), "{text}");
    assert!(text.contains("\"forked_from\": \"alpha\""), "{text}");
}

#[test]
fn a_manifest_branch_beats_the_disk_and_a_disk_only_repo_is_said_out_loud() {
    let recorded = vec![
        serde_json::json!({"name": "svc", "branch": "main"}),
        // Membership with no branch: charter's own writer's shape. It must NOT take the
        // repo off the disk-only list, which is the whole of charter#884's correction.
        serde_json::json!({"name": "tool"}),
    ];
    let disk = BTreeMap::from([
        ("svc".to_string(), "feature/x".to_string()),
        ("tool".to_string(), "trunk".to_string()),
        ("extra".to_string(), "main".to_string()),
    ]);
    let (rows, disk_only) = merge_repo_rows(&recorded, &disk);
    assert_eq!(
        rows,
        vec![
            ("extra".to_string(), "main".to_string()),
            ("svc".to_string(), "main".to_string()),
            ("tool".to_string(), "trunk".to_string()),
        ]
    );
    assert_eq!(disk_only, vec!["extra".to_string(), "tool".to_string()]);
}

#[test]
fn a_workspace_that_recorded_a_repo_it_no_longer_has_still_inherits_it() {
    // The other half of the union: a teammate who has just cloned the plane has no repos
    // on disk at all, and the snapshot is the only thing that can tell the fork what to
    // clone.
    let recorded = vec![serde_json::json!({"name": "svc", "branch": "main"})];
    let (rows, disk_only) = merge_repo_rows(&recorded, &BTreeMap::new());
    assert_eq!(rows, vec![("svc".to_string(), "main".to_string())]);
    assert!(disk_only.is_empty(), "{disk_only:?}");
}

#[test]
fn a_memory_store_charter_cannot_list_is_named_and_nothing_of_it_is_copied() {
    let dir = plane();
    a_parent(dir.path(), "alpha");
    let memory = dir.path().join("workspaces/alpha/memory");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&memory, std::fs::Permissions::from_mode(0o000)).unwrap();
    }
    let (code, lines) = run(dir.path(), "alpha", "gamma", false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&memory, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    // The fork EXISTS and the exit says what it is missing. Both halves matter: raising
    // here cost the operator the whole fork.
    assert_eq!(code, 1, "{lines:?}");
    assert!(dir.path().join("workspaces/gamma/workspace.md").exists());
    assert!(
        lines[0].contains("without the memory purlis could not read in 'alpha'"),
        "{lines:?}"
    );
    assert!(lines[1].contains("cannot be checked"), "{lines:?}");
    // The fork's own note says what it did not inherit, so a later reader of that memory
    // is not told it holds context it never received.
    let index =
        std::fs::read_to_string(dir.path().join("workspaces/gamma/memory/MEMORY.md")).unwrap();
    assert!(index.contains("without the memory"), "{index}");
    assert!(
        !index.contains("The API returns 418"),
        "nothing of the parent's memory was copied: {index}"
    );
}

#[test]
fn a_memory_that_is_a_link_out_of_the_plane_is_named_and_never_copied() {
    let dir = plane();
    a_parent(dir.path(), "alpha");
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("secret.md"), "not yours\n").unwrap();
    let planted = dir.path().join("workspaces/alpha/memory/leak.md");
    #[cfg(unix)]
    std::os::unix::fs::symlink(outside.path().join("secret.md"), &planted).unwrap();
    let (code, lines) = run(dir.path(), "alpha", "gamma", false);
    assert_eq!(code, 1, "{lines:?}");
    let leaked = dir.path().join("workspaces/gamma/memory/leak.md");
    assert!(
        !leaked.exists(),
        "a link out of the plane is not a file this fork inherits"
    );
    assert!(
        lines.iter().any(|l| l.contains("leak.md")),
        "carrying nothing and saying nothing is the one answer a fork may not give: \
         {lines:?}"
    );
}

#[test]
fn a_name_that_is_not_a_workspace_name_forks_nothing() {
    let dir = plane();
    a_parent(dir.path(), "alpha");
    let (code, lines) = run(dir.path(), "alpha", "../esc", false);
    assert_eq!(code, 1);
    assert!(
        lines[0].contains("invalid workspace name '../esc'"),
        "{lines:?}"
    );
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(!dir.path().parent().unwrap().join("esc").exists());
}

#[test]
fn forking_onto_a_workspace_that_exists_changes_neither() {
    let dir = plane();
    a_parent(dir.path(), "alpha");
    ensure::ensure(dir.path(), "beta", now(), "fixture").unwrap();
    let before = std::fs::read_to_string(dir.path().join("workspaces/beta/workspace.md")).unwrap();
    let (code, lines) = run(dir.path(), "alpha", "beta", false);
    assert_eq!(code, 1);
    assert!(
        lines[0].contains("workspace 'beta' already exists"),
        "{lines:?}"
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("workspaces/beta/workspace.md")).unwrap(),
        before
    );
}

#[test]
fn forking_a_workspace_onto_its_own_name_is_refused_before_anything_is_read() {
    let dir = plane();
    a_parent(dir.path(), "alpha");
    let (code, lines) = run(dir.path(), "alpha", "alpha", false);
    assert_eq!(code, 1);
    assert!(lines[0].contains("nothing to fork"), "{lines:?}");
}

#[test]
fn a_source_that_is_no_workspace_forks_nothing() {
    let dir = plane();
    let (code, lines) = run(dir.path(), "nowhere", "gamma", false);
    assert_eq!(code, 1);
    assert!(lines[0].contains("no workspace 'nowhere'"), "{lines:?}");
    assert!(!dir.path().join("workspaces/gamma").exists());
}

#[test]
fn a_live_fork_is_un_ignored_and_told_how_to_share_itself() {
    let dir = plane();
    a_parent(dir.path(), "alpha");
    let (_code, lines) = run(dir.path(), "alpha", "gamma", true);
    assert!(lines[0].contains("(LIVE)"), "{lines:?}");
    assert!(lines.last().unwrap().contains("purlis save"), "{lines:?}");
    let ignore = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
    assert!(
        ignore.contains("!/workspaces/gamma/workspace.json"),
        "{ignore}"
    );
}

#[test]
fn the_pieces_a_fork_could_not_read_are_listed_the_way_charter_lists_them() {
    assert_eq!(and_list([]), "");
    assert_eq!(and_list(["memory"]), "memory");
    assert_eq!(and_list(["memory", "todos"]), "memory and todos");
    assert_eq!(
        and_list(["workspace.md", "memory", "todos"]),
        "workspace.md, memory and todos"
    );
}
