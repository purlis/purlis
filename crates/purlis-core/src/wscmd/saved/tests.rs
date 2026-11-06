// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::wscmd::{self, reinit::Scope};

/// A plane whose `.gitignore` is `init`'s workspace lines and a LIVE block for `alpha`, with
/// `alpha` holding a note folder, a script, a dotfile, its memory and a change log.
fn plane() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(root.join("charter.toml"), "").unwrap();
    std::fs::write(
        root.join(".gitignore"),
        "/.charter/\n/workspaces/*/*\n!/workspaces/.gitkeep\n",
    )
    .unwrap();
    wscmd::set_live(root, "alpha", true).unwrap();
    let ws = root.join("workspaces/alpha");
    for folder in ["notes", "memory", "changes/log", "repos/api"] {
        std::fs::create_dir_all(ws.join(folder)).unwrap();
    }
    for (file, text) in [
        ("notes/plan.md", "the plan\n"),
        ("deploy.sh", "TOKEN=x ./go\n"),
        (".env", "TOKEN=x\n"),
        ("memory/MEMORY.md", "# memory\n"),
        ("changes/log/this.jsonl", "{}\n"),
        ("repos/api/README.md", "api\n"),
    ] {
        std::fs::write(ws.join(file), text).unwrap();
    }
    std::fs::create_dir_all(root.join("workspaces/beta/notes")).unwrap();
    dir
}

fn gitignore(root: &Path) -> String {
    std::fs::read_to_string(root.join(".gitignore")).unwrap()
}

fn kept(root: &Path, path: &str, folder: bool) -> Kept {
    Marks::of(root, "alpha").unwrap().of_path(path, folder)
}

#[test]
fn the_marks_follow_the_allowlist() {
    let dir = plane();
    let root = dir.path();
    assert_eq!(kept(root, "notes", true), Kept::ThisMachineOnly);
    assert_eq!(kept(root, "notes/plan.md", false), Kept::ThisMachineOnly);
    assert_eq!(kept(root, "deploy.sh", false), Kept::ThisMachineOnly);
    assert_eq!(kept(root, "memory", true), Kept::WithTheProject);
    assert_eq!(kept(root, "memory/MEMORY.md", false), Kept::WithTheProject);
    assert_eq!(
        kept(root, "changes/log/this.jsonl", false),
        Kept::ThisMachineOnly,
        "the block re-ignores the change log under the changes it shares"
    );

    assert!(add(root, "alpha", "notes").unwrap());

    assert_eq!(kept(root, "notes", true), Kept::WithTheProject);
    assert_eq!(kept(root, "notes/plan.md", false), Kept::WithTheProject);
    assert_eq!(kept(root, "deploy.sh", false), Kept::ThisMachineOnly);
    assert_eq!(saved(root, "alpha"), BTreeSet::from(["notes".to_string()]));
}

#[test]
fn a_workspace_that_is_not_live_has_no_marks_and_saves_nothing() {
    let dir = plane();
    let root = dir.path();
    let before = gitignore(root);
    assert!(Marks::of(root, "beta").is_none());
    assert!(matches!(
        add(root, "beta", "notes"),
        Err(NotSaved::NotLive(ws)) if ws == "beta"
    ));
    assert_eq!(gitignore(root), before);
}

#[test]
fn save_with_the_project_adds_exactly_one_line_after_the_workspaces_own() {
    let dir = plane();
    let root = dir.path();
    let before = gitignore(root);
    add(root, "alpha", "deploy.sh").unwrap();
    let after = gitignore(root);
    let mut added: Vec<&str> = after.lines().collect();
    for line in before.lines() {
        let at = added.iter().position(|one| *one == line).unwrap();
        added.remove(at);
    }
    assert_eq!(added, ["!/workspaces/alpha/deploy.sh"]);
    let own_last = after.find("/workspaces/alpha/changes/log/").unwrap();
    let saved_at = after.find("!/workspaces/alpha/deploy.sh").unwrap();
    let end = after.find(crate::names::LIVE_END.reads[0]).unwrap();
    assert!(own_last < saved_at && saved_at < end, "{after}");

    assert!(
        !add(root, "alpha", "deploy.sh").unwrap(),
        "a second press adds nothing"
    );
    assert_eq!(gitignore(root), after);
}

#[test]
fn managed_entries_keep_their_existing_lines() {
    let dir = plane();
    let root = dir.path();
    let before = gitignore(root);
    assert!(
        !add(root, "alpha", "memory").unwrap(),
        "memory is already shared by the block's own lines"
    );
    assert_eq!(gitignore(root), before);

    add(root, "alpha", "notes").unwrap();
    let after = gitignore(root);
    let own: Vec<String> = wscmd::workspace_lines("alpha");
    let in_order: Vec<&str> = after
        .lines()
        .filter(|line| own.iter().any(|one| one == line))
        .collect();
    assert_eq!(in_order, own.iter().map(String::as_str).collect::<Vec<_>>());
}

#[test]
fn only_one_entry_at_the_top_is_saved_and_never_a_pattern() {
    let dir = plane();
    let root = dir.path();
    let before = gitignore(root);
    for entry in [
        "notes/plan.md",
        "*",
        "no[te]s",
        "!notes",
        "notes ",
        "",
        "..",
        "a\\b",
    ] {
        assert!(
            matches!(add(root, "alpha", entry), Err(NotSaved::NotOneEntry(_))),
            "{entry:?}"
        );
    }
    assert_eq!(gitignore(root), before);
}

#[test]
fn the_line_survives_reinit_and_every_rewrite_of_the_block() {
    let dir = plane();
    let root = dir.path();
    add(root, "alpha", "notes").unwrap();
    let only_alpha = BTreeSet::from(["notes".to_string()]);

    wscmd::refresh_live_block(root).unwrap();
    assert_eq!(saved(root, "alpha"), only_alpha, "refresh_live_block");

    let mut said = Vec::new();
    wscmd::reinit::reinit(root, Scope::All, chrono::Utc::now(), &mut |say| {
        said.push(say)
    });
    assert_eq!(saved(root, "alpha"), only_alpha, "reinit: {said:?}");

    wscmd::set_live(root, "beta", true).unwrap();
    assert_eq!(
        saved(root, "alpha"),
        only_alpha,
        "another workspace made LIVE"
    );
    wscmd::write_live_block(root, ["alpha"]).unwrap();
    assert_eq!(saved(root, "alpha"), only_alpha, "the block written again");

    wscmd::rename_live(root, "alpha", "gamma").unwrap();
    assert_eq!(saved(root, "gamma"), only_alpha, "renamed");
    assert!(gitignore(root).contains("!/workspaces/gamma/notes\n"));
    assert!(!gitignore(root).contains("/workspaces/alpha/"));
}

#[test]
fn a_workspace_made_local_drops_its_saved_lines_and_its_shared_paths_name_them() {
    let dir = plane();
    let root = dir.path();
    add(root, "alpha", "notes").unwrap();
    assert!(
        wscmd::meta_paths(root, "alpha").contains(&"workspaces/alpha/notes".to_string()),
        "live --off untracks what was saved with the rest"
    );
    wscmd::set_live(root, "alpha", false).unwrap();
    assert!(!gitignore(root).contains("/workspaces/alpha/notes"));
    wscmd::set_live(root, "alpha", true).unwrap();
    assert!(
        saved(root, "alpha").is_empty(),
        "made LIVE again, nothing is saved unasked"
    );
}

/// What git would add from the plane at `root`: every file not ignored, as git reads the
/// `.gitignore`.
fn unignored(root: &Path) -> Vec<String> {
    let git = |args: &[&str]| {
        crate::forklock::output(
            std::process::Command::new("git")
                .args(args)
                .current_dir(root)
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_CONFIG_NOSYSTEM", "1"),
        )
        .unwrap()
    };
    assert!(git(&["init", "-q"]).status.success());
    let listed = git(&["ls-files", "--others", "--exclude-standard"]);
    assert!(listed.status.success());
    String::from_utf8(listed.stdout)
        .unwrap()
        .lines()
        .filter(|line| line.starts_with("workspaces/alpha/"))
        .map(str::to_string)
        .collect()
}

#[test]
fn save_with_the_project_unignores_exactly_that_entry_in_the_planes_git() {
    let dir = plane();
    let root = dir.path();
    assert_eq!(unignored(root), ["workspaces/alpha/memory/MEMORY.md"]);

    add(root, "alpha", "notes").unwrap();

    assert_eq!(
        unignored(root),
        [
            "workspaces/alpha/memory/MEMORY.md",
            "workspaces/alpha/notes/plan.md"
        ]
    );
}
