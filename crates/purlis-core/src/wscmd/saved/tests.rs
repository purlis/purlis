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

/// Where `path` of workspace `alpha` is kept, as the plane's git, made a repo if it is not one,
/// says.
fn kept(root: &Path, path: &str) -> Kept {
    if !root.join(".git").exists() {
        assert!(git(root, &["init", "-q"]).status.success());
    }
    Marks::of(root, "alpha").unwrap().of_paths(&[path]).unwrap()[0]
}

/// `git commit` of everything git would add, by an identity of the test's own.
fn commit_all(root: &Path) {
    assert!(git(root, &["init", "-q"]).status.success());
    assert!(git(root, &["add", "-A"]).status.success());
    let committed = git(
        root,
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "-qm",
            "saved",
        ],
    );
    assert!(committed.status.success(), "{committed:?}");
}

#[test]
fn the_marks_follow_the_allowlist() {
    let dir = plane();
    let root = dir.path();
    assert_eq!(kept(root, "notes"), Kept::ThisMachineOnly);
    assert_eq!(kept(root, "notes/plan.md"), Kept::ThisMachineOnly);
    assert_eq!(kept(root, "deploy.sh"), Kept::ThisMachineOnly);
    assert_eq!(kept(root, "memory"), Kept::WithTheProject);
    assert_eq!(kept(root, "memory/MEMORY.md"), Kept::WithTheProject);
    assert_eq!(
        kept(root, "changes/log/this.jsonl"),
        Kept::ThisMachineOnly,
        "the block re-ignores the change log under the changes it shares"
    );

    assert!(add(root, "alpha", "notes").unwrap());

    assert_eq!(kept(root, "notes"), Kept::WithTheProject);
    assert_eq!(kept(root, "notes/plan.md"), Kept::WithTheProject);
    assert_eq!(kept(root, "deploy.sh"), Kept::ThisMachineOnly);
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

/// `git <args>` in the plane at `root`, read with no config of the person's.
fn git(root: &Path, args: &[&str]) -> std::process::Output {
    crate::forklock::output(
        std::process::Command::new("git")
            .args(args)
            .current_dir(root)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1"),
    )
    .unwrap()
}

/// What `git ls-files <args>` lists of workspace `alpha`.
fn listed(root: &Path, args: &[&str]) -> Vec<String> {
    let listed = git(
        root,
        &[&["-c", "core.quotepath=false", "ls-files"], args].concat(),
    );
    assert!(listed.status.success());
    String::from_utf8(listed.stdout)
        .unwrap()
        .lines()
        .filter(|line| line.starts_with("workspaces/alpha/"))
        .map(str::to_string)
        .collect()
}

/// What git would add from the plane at `root`: every file not ignored, as git reads the
/// `.gitignore`.
fn unignored(root: &Path) -> Vec<String> {
    assert!(git(root, &["init", "-q"]).status.success());
    listed(root, &["--others", "--exclude-standard"])
}

#[test]
fn a_saved_entry_git_never_tracked_does_not_stop_live_off_untracking_the_rest() {
    let dir = plane();
    let root = dir.path();
    std::fs::create_dir_all(root.join("workspaces/alpha/drafts")).unwrap();
    add(root, "alpha", "drafts").unwrap();
    add(root, "alpha", "deploy.sh").unwrap();
    assert!(git(root, &["init", "-q"]).status.success());
    assert!(git(root, &["add", "-A"]).status.success());
    assert!(
        git(
            root,
            &[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "commit",
                "-qm",
                "saved"
            ]
        )
        .status
        .success()
    );
    // Saved after the last commit, so git has never tracked it: an empty `drafts/` is the same.
    std::fs::write(root.join("workspaces/alpha/later.md"), "new\n").unwrap();
    add(root, "alpha", "later.md").unwrap();
    assert_eq!(
        listed(root, &[]),
        [
            "workspaces/alpha/deploy.sh",
            "workspaces/alpha/memory/MEMORY.md"
        ]
    );

    let mut said = Vec::new();
    let code = wscmd::live::live(root, "alpha", true, &mut |line| said.push(line.to_string()));

    assert_eq!(code, 0, "{said:?}");
    assert_eq!(listed(root, &[]), Vec::<String>::new(), "{said:?}");
    assert!(!wscmd::live_workspaces(root).contains("alpha"));
}

#[test]
fn an_entry_that_holds_a_repo_is_never_saved_and_its_line_goes_when_it_becomes_one() {
    let dir = plane();
    let root = dir.path();
    std::fs::create_dir_all(root.join("workspaces/alpha/tool/.git")).unwrap();
    let before = gitignore(root);
    assert!(matches!(
        add(root, "alpha", "tool"),
        Err(NotSaved::IsARepo(_))
    ));
    assert_eq!(gitignore(root), before);

    add(root, "alpha", "notes").unwrap();
    std::fs::create_dir_all(root.join("workspaces/alpha/notes/.git")).unwrap();
    wscmd::set_live(root, "beta", true).unwrap();
    assert!(
        !gitignore(root).contains("!/workspaces/alpha/notes"),
        "{}",
        gitignore(root)
    );
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

#[test]
fn a_tracked_entry_whose_line_is_gone_is_marked_as_shared_as_git_commits_it() {
    let dir = plane();
    let root = dir.path();
    add(root, "alpha", "notes").unwrap();
    commit_all(root);
    // An older purlis on a teammate's machine rewrites the block without the saved line.
    let without = gitignore(root).replace("!/workspaces/alpha/notes\n", "");
    std::fs::write(root.join(".gitignore"), without).unwrap();
    std::fs::write(root.join("workspaces/alpha/notes/new.md"), "new\n").unwrap();

    assert_eq!(kept(root, "notes"), Kept::WithTheProject);
    assert_eq!(kept(root, "notes/plan.md"), Kept::WithTheProject);
    assert_eq!(
        kept(root, "notes/new.md"),
        Kept::ThisMachineOnly,
        "git adds no file it ignores and does not track"
    );
    assert_eq!(kept(root, "deploy.sh"), Kept::ThisMachineOnly);
}

#[test]
fn the_marks_read_a_nested_gitignore_and_info_exclude_as_git_does() {
    let dir = plane();
    let root = dir.path();
    add(root, "alpha", "notes").unwrap();
    assert!(git(root, &["init", "-q"]).status.success());
    std::fs::write(
        root.join("workspaces/alpha/notes/.gitignore"),
        "secret.md\n",
    )
    .unwrap();
    std::fs::write(
        root.join(".git/info/exclude"),
        "/workspaces/alpha/notes/draft.md\n",
    )
    .unwrap();
    for file in ["secret.md", "draft.md"] {
        std::fs::write(root.join("workspaces/alpha/notes").join(file), "x\n").unwrap();
    }

    assert_eq!(kept(root, "notes/plan.md"), Kept::WithTheProject);
    assert_eq!(kept(root, "notes/secret.md"), Kept::ThisMachineOnly);
    assert_eq!(kept(root, "notes/draft.md"), Kept::ThisMachineOnly);
}

#[test]
fn an_nfd_named_entry_that_is_saved_is_committed_by_git_however_git_reads_names() {
    // `café` spelled decomposed, as a name from an older Mac disk or an archive is.
    let nfd = "cafe\u{301}";
    for precompose in ["true", "false"] {
        let dir = plane();
        let root = dir.path();
        std::fs::create_dir_all(root.join("workspaces/alpha").join(nfd)).unwrap();
        std::fs::write(
            root.join("workspaces/alpha").join(nfd).join("menu.md"),
            "x\n",
        )
        .unwrap();
        assert!(git(root, &["init", "-q"]).status.success());
        assert!(
            git(root, &["config", "core.precomposeunicode", precompose])
                .status
                .success()
        );

        add(root, "alpha", nfd).unwrap();

        let listed: Vec<String> = unignored(root)
            .into_iter()
            .filter(|path| path.ends_with("/menu.md"))
            .collect();
        assert_eq!(
            listed.len(),
            1,
            "precompose {precompose}: {}",
            gitignore(root)
        );
        assert_eq!(
            kept(root, &format!("{nfd}/menu.md")),
            Kept::WithTheProject,
            "precompose {precompose}"
        );
        assert_eq!(
            saved(root, "alpha"),
            BTreeSet::from([nfd.to_string()]),
            "one entry, however many spellings its lines take"
        );
    }
}

#[test]
fn an_nfd_named_entry_is_marked_saved_only_when_git_adds_it_whichever_of_its_lines_is_left() {
    // `café` spelled decomposed on disk, with only one of its two lines left in the block, as a
    // hand edit or an older purlis leaves it.
    let nfd = "cafe\u{301}";
    let nfc = "caf\u{e9}";
    for precompose in ["true", "false"] {
        for gone in [nfc, nfd] {
            let dir = plane();
            let root = dir.path();
            let entry = root.join("workspaces/alpha").join(nfd);
            std::fs::create_dir_all(&entry).unwrap();
            std::fs::write(entry.join("menu.md"), "x\n").unwrap();
            assert!(git(root, &["init", "-q"]).status.success());
            assert!(
                git(root, &["config", "core.precomposeunicode", precompose])
                    .status
                    .success()
            );
            add(root, "alpha", nfd).unwrap();
            let without = gitignore(root).replace(&format!("!/workspaces/alpha/{gone}\n"), "");
            std::fs::write(root.join(".gitignore"), without).unwrap();

            let added = unignored(root)
                .iter()
                .any(|path| path.ends_with("/menu.md"));
            let marked = kept(root, &format!("{nfd}/menu.md")) == Kept::WithTheProject;
            assert_eq!(
                marked,
                added,
                "precompose {precompose}, {} line gone: {}",
                if gone == nfc {
                    "composed"
                } else {
                    "decomposed"
                },
                gitignore(root)
            );
        }
    }
}

#[test]
fn a_name_git_would_read_as_a_pattern_is_never_marked_saved() {
    let dir = plane();
    let root = dir.path();
    add(root, "alpha", "notes").unwrap();
    commit_all(root);
    // Untracked and ignored, but each would match the tracked `notes/plan.md` as a pathspec.
    for name in ["n*", "*", "note?", "[n]otes", "n\\otes"] {
        assert_eq!(kept(root, name), Kept::ThisMachineOnly, "{name}");
    }
    for path in ["notes/pl*", "notes/plan.m?"] {
        assert_eq!(kept(root, path), Kept::ThisMachineOnly, "{path}");
    }
    assert_eq!(kept(root, "notes/plan.md"), Kept::WithTheProject);
}
