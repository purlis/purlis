// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! [`super::arrived`]: what is told, what is not, and that nothing found ever runs.

use std::fs;
use std::path::{Path, PathBuf};

use super::arrived::{self, Watched};
use crate::testgit;

const CHAT: &str = "01M6M3VNN8GNE85XEV3194WJE6";
const OTHER: &str = "01M6M3VNN8GNE85XEV3194WJE7";

struct Fixture {
    _dir: tempfile::TempDir,
    plane: PathBuf,
    folder: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("a folder");
        let plane = dir.path().canonicalize().expect("real").join("plane");
        let folder = plane.join("work");
        fs::create_dir_all(&folder).expect("a chat's folder");
        Self {
            _dir: dir,
            plane,
            folder,
        }
    }

    fn watched(&self) -> Watched {
        Watched {
            folders: vec![self.folder.clone()],
            ..Watched::default()
        }
    }

    fn begin(&self) {
        arrived::begin(&self.plane, CHAT, Some("Fix the build"), &self.watched())
            .expect("a record");
    }

    /// What a look has newly told, each below the chat's folder.
    fn look(&self) -> Vec<String> {
        let looked = arrived::look(&self.plane, CHAT, None)
            .expect("a look")
            .expect("a record");
        looked
            .new
            .iter()
            .map(|path| {
                Path::new(path)
                    .strip_prefix(&self.folder)
                    .expect("in the folder")
                    .display()
                    .to_string()
            })
            .collect()
    }
}

fn file(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().expect("its folder")).expect("its folder");
    fs::write(path, text).expect("a file");
}

fn git(dir: &Path, args: &[&str]) {
    let ran = testgit::run_unconfigured(dir, args);
    assert!(ran.ok(), "git {args:?}: {}", ran.err);
}

#[test]
fn a_name_that_appears_is_told_once_and_one_that_was_there_is_not() {
    let fix = Fixture::new();
    file(&fix.folder.join(".envrc"), "there at the start\n");
    fix.begin();
    assert_eq!(fix.look(), Vec::<String>::new());

    file(&fix.folder.join("deep/er/.vscode/tasks.json"), "{}\n");
    file(&fix.folder.join("deep/.git/hooks/pre-commit"), "x\n");
    file(&fix.folder.join("Library/LaunchAgents/a.plist"), "x\n");
    assert_eq!(
        fix.look(),
        ["Library/LaunchAgents", "deep/.git", "deep/er/.vscode"],
    );
    // Each once: the Notice has them, and a second look adds nothing.
    assert_eq!(fix.look(), Vec::<String>::new());
    let told = arrived::told(&fix.plane);
    assert_eq!(told.len(), 1);
    assert_eq!(told[0].paths.len(), 3);
}

#[test]
fn a_name_in_another_case_is_told() {
    let fix = Fixture::new();
    fix.begin();
    file(&fix.folder.join("a/.MCP.json"), "{}\n");
    assert_eq!(fix.look(), ["a/.MCP.json"]);
}

#[test]
fn what_is_not_a_name_and_what_is_empty_says_nothing_until_it_holds_something() {
    let fix = Fixture::new();
    fix.begin();
    file(&fix.folder.join("src/main.rs"), "fn main() {}\n");
    file(&fix.folder.join("a/purlis.toml"), "schema = 1\n");
    file(&fix.folder.join(".claude/workflows/x.md"), "x\n");
    file(&fix.folder.join("sub/.envrc"), "");
    fs::create_dir_all(fix.folder.join("sub/.vscode")).expect("a folder");
    assert_eq!(fix.look(), Vec::<String>::new());
    file(&fix.folder.join("sub/.envrc"), "export A=1\n");
    assert_eq!(fix.look(), ["sub/.envrc"]);
}

#[test]
fn a_package_tree_is_not_entered_and_a_temp_folder_is_not_looked_at() {
    let fix = Fixture::new();
    fix.begin();
    file(&fix.folder.join("node_modules/x/.vscode/a"), "x\n");
    file(&fix.folder.join("target/debug/.envrc"), "x\n");
    file(&fix.plane.join("elsewhere/.envrc"), "x\n");
    assert_eq!(fix.look(), Vec::<String>::new());
}

#[test]
#[cfg(unix)]
fn a_link_is_never_followed_and_one_named_as_a_name_is_told_as_itself() {
    let fix = Fixture::new();
    let outside = fix.plane.join("outside");
    file(&outside.join(".envrc"), "x\n");
    fix.begin();
    std::os::unix::fs::symlink(&outside, fix.folder.join("through")).expect("a link");
    assert_eq!(fix.look(), Vec::<String>::new());
    std::os::unix::fs::symlink("/nowhere", fix.folder.join(".envrc")).expect("a link");
    assert_eq!(fix.look(), [".envrc"]);
}

#[test]
#[cfg(unix)]
fn a_fifo_with_a_protected_name_holds_no_look() {
    let fix = Fixture::new();
    fix.begin();
    let at = fix.folder.join(".envrc");
    let made =
        crate::forklock::status(std::process::Command::new("mkfifo").arg(&at)).expect("mkfifo");
    assert!(made.success());
    // A `.git` that is a FIFO too, which the check of a pointer would read.
    fs::create_dir_all(fix.folder.join("a")).expect("a folder");
    let git = fix.folder.join("a/.git");
    let made =
        crate::forklock::status(std::process::Command::new("mkfifo").arg(&git)).expect("mkfifo");
    assert!(made.success());
    assert_eq!(fix.look(), [".envrc", "a/.git"]);
}

#[test]
#[cfg(unix)]
fn nothing_in_a_repository_that_appeared_is_ever_run() {
    use std::os::unix::fs::PermissionsExt;
    let fix = Fixture::new();
    let mark = fix.plane.join("it-ran");
    let script = fix.plane.join("runs.sh");
    file(&script, &format!("#!/bin/sh\ntouch '{}'\n", mark.display()));
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).expect("executable");
    fix.begin();

    let repo = fix.folder.join("brought");
    fs::create_dir_all(&repo).expect("a folder");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["commit", "-q", "--allow-empty", "-m", "start"]);
    let config = repo.join(".git/config");
    let mut text = fs::read_to_string(&config).expect("its config");
    text.push_str(&format!(
        "[core]\n\tfsmonitor = {0}\n\tsshCommand = {0}\n\thooksPath = {1}\n",
        script.display(),
        repo.join(".git/hooks").display()
    ));
    fs::write(&config, text).expect("its config");
    for hook in ["pre-commit", "post-checkout", "post-index-change"] {
        let at = repo.join(".git/hooks").join(hook);
        fs::create_dir_all(at.parent().expect("its folder")).expect("its folder");
        fs::copy(&script, &at).expect("a hook");
    }
    file(&repo.join(".vscode/tasks.json"), "{}\n");

    let told = fix.look();
    assert_eq!(told, ["brought/.git", "brought/.vscode"]);
    assert_eq!(arrived::told(&fix.plane).len(), 1);
    assert!(!mark.exists(), "something the look found was run");

    // The script does run, so its absence above means something: a git status there runs it.
    let status = crate::forklock::output(
        std::process::Command::new("git")
            .args(["status", "--short"])
            .current_dir(&repo),
    )
    .expect("git");
    assert!(status.status.success());
    assert!(mark.exists(), "the marker can appear");
}

#[test]
fn a_look_cut_short_says_so_and_never_reads_as_nothing_appeared() {
    let fix = Fixture::new();
    fix.begin();
    assert!(arrived::told(&fix.plane).is_empty());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let shut = fix.folder.join("shut");
        fs::create_dir_all(&shut).expect("a folder");
        fs::set_permissions(&shut, fs::Permissions::from_mode(0o000)).expect("shut");
        let looked = arrived::look(&fix.plane, CHAT, None)
            .expect("a look")
            .expect("a record");
        fs::set_permissions(&shut, fs::Permissions::from_mode(0o755)).expect("open");
        // Root reads anything, and that is no folder left unread.
        if fs::read_dir("/root").is_err() {
            assert!(looked.cut_short);
            let told = arrived::told(&fix.plane);
            assert_eq!(told.len(), 1);
            assert!(told[0].paths.is_empty());
            let words = told[0].words(&fix.plane);
            assert!(words.contains("this check is incomplete"), "{words}");

            // Put away, it stays away until something does appear, and a whole look says all.
            arrived::put_away(&fix.plane, CHAT).expect("put away");
            assert!(arrived::told(&fix.plane).is_empty());
        }
    }
}

#[test]
fn a_walk_that_reaches_its_bound_says_so() {
    let fix = Fixture::new();
    // [`arrived::MOST_NAMES`] names are far quicker to make than the bound on entries.
    let many = fix.folder.join("many");
    fs::create_dir_all(&many).expect("a folder");
    for at in 0..arrived::MOST_NAMES + 1 {
        let folder = many.join(at.to_string());
        fs::create_dir(&folder).expect("a folder");
        fs::write(folder.join(".envrc"), "x").expect("a name");
    }
    arrived::begin(&fix.plane, CHAT, None, &fix.watched()).expect("a record");
    let looked = arrived::look(&fix.plane, CHAT, None)
        .expect("a look")
        .expect("a record");
    assert!(looked.cut_short);
    let told = arrived::told(&fix.plane);
    assert_eq!(told.len(), 1);
    assert!(told[0].words(&fix.plane).contains("incomplete"));
}

#[test]
fn a_name_first_reached_when_the_bound_moves_is_not_told_as_new() {
    let fix = Fixture::new();
    let many = fix.folder.join("many");
    fs::create_dir_all(&many).expect("a folder");
    for at in 0..arrived::MOST_NAMES + 20 {
        let folder = many.join(format!("{at:06}"));
        fs::create_dir(&folder).expect("a folder");
        fs::write(folder.join(".envrc"), "x").expect("a name");
    }
    arrived::begin(&fix.plane, CHAT, None, &fix.watched()).expect("a record");
    // Gone, so a look reaches further than the first did, and finds what was there all along.
    for at in 0..100 {
        fs::remove_dir_all(many.join(format!("{at:06}"))).expect("gone");
    }
    let looked = arrived::look(&fix.plane, CHAT, None)
        .expect("a look")
        .expect("a record");
    assert_eq!(looked.new, Vec::<String>::new());
    assert!(!looked.cut_short);

    // Whole from here on, what appears is told, at any depth.
    file(&many.join("000500/sub/.mcp.json"), "{}\n");
    assert_eq!(fix.look(), ["many/000500/sub/.mcp.json"]);
}

#[test]
fn a_look_cut_short_is_told_once_for_a_set_of_folders_and_what_appears_is_still_told() {
    let fix = Fixture::new();
    let many = fix.folder.join("many");
    fs::create_dir_all(&many).expect("a folder");
    for at in 0..=arrived::MOST_NAMES {
        let folder = many.join(format!("{at:06}"));
        fs::create_dir(&folder).expect("a folder");
        fs::write(folder.join(".envrc"), "x").expect("a name");
    }
    arrived::begin(&fix.plane, CHAT, None, &fix.watched()).expect("a record");
    arrived::begin(&fix.plane, OTHER, None, &fix.watched()).expect("a record");
    arrived::look(&fix.plane, CHAT, None).expect("a look");
    arrived::look(&fix.plane, OTHER, None).expect("a look");
    assert_eq!(arrived::told(&fix.plane).len(), 2);

    arrived::put_away(&fix.plane, CHAT).expect("put away");
    assert!(arrived::told(&fix.plane).is_empty());

    // Near the top, where the walk reaches first.
    file(&fix.folder.join(".envrc"), "x\n");
    assert_eq!(fix.look(), [".envrc"]);
    let told = arrived::told(&fix.plane);
    assert_eq!(told.len(), 1);
    assert!(told[0].words(&fix.plane).contains("incomplete"));
}

#[test]
fn the_record_survives_a_restart_and_what_appeared_while_the_app_was_closed_is_told() {
    let fix = Fixture::new();
    fix.begin();
    file(&fix.folder.join(".envrc"), "x\n");
    assert_eq!(fix.look(), [".envrc"]);
    // Everything below is read from the file alone, as a new launch does.
    file(&fix.folder.join(".mcp.json"), "{}\n");
    // A chat that comes back starts again over its record, keeping what it knew.
    fix.begin();
    assert_eq!(fix.look(), [".mcp.json"]);
    assert_eq!(
        arrived::told(&fix.plane)[0].name.as_deref(),
        Some("Fix the build")
    );
    assert_eq!(arrived::told(&fix.plane)[0].paths.len(), 2);
}

#[test]
fn put_away_keeps_the_files_and_tells_them_no_more_and_what_comes_back_is_told_again() {
    let fix = Fixture::new();
    fix.begin();
    let at = fix.folder.join(".envrc");
    file(&at, "x\n");
    fix.look();
    arrived::put_away(&fix.plane, CHAT).expect("put away");
    assert!(arrived::told(&fix.plane).is_empty());
    assert!(at.exists(), "the file is the person's");
    assert_eq!(fix.look(), Vec::<String>::new());

    fs::remove_file(&at).expect("gone");
    assert_eq!(fix.look(), Vec::<String>::new());
    file(&at, "again\n");
    assert_eq!(fix.look(), [".envrc"]);
}

#[test]
fn a_path_is_on_one_chats_notice_only() {
    let fix = Fixture::new();
    fix.begin();
    arrived::begin(&fix.plane, OTHER, Some("Another"), &fix.watched()).expect("a record");
    file(&fix.folder.join(".envrc"), "x\n");
    assert_eq!(fix.look(), [".envrc"]);
    let other = arrived::look(&fix.plane, OTHER, None)
        .expect("a look")
        .expect("a record");
    assert!(other.new.is_empty());
    assert_eq!(arrived::told(&fix.plane).len(), 1);
}

#[test]
fn a_chat_with_no_record_is_one_with_no_sandbox_and_says_nothing() {
    let fix = Fixture::new();
    assert_eq!(arrived::look(&fix.plane, CHAT, None).expect("a look"), None);
    assert!(arrived::look(&fix.plane, "../x", None).is_err());
}

#[test]
fn a_lifted_name_says_nothing() {
    let fix = Fixture::new();
    let watched = Watched {
        lifted: vec![".config/gh".to_owned()],
        ..fix.watched()
    };
    arrived::begin(&fix.plane, CHAT, None, &watched).expect("a record");
    file(&fix.folder.join(".config/gh/config.yml"), "x\n");
    assert_eq!(fix.look(), Vec::<String>::new());
    file(&fix.folder.join(".config/fish/config.fish"), "x\n");
    assert_eq!(fix.look(), [".config/fish"]);
}

#[test]
fn at_the_floor_the_homes_own_names_are_asked_for_and_the_home_is_not_walked() {
    let fix = Fixture::new();
    let home = fix.plane.join("home");
    fs::create_dir_all(&home).expect("a home");
    let watched = Watched {
        home: Some(home.clone()),
        ..fix.watched()
    };
    arrived::begin(&fix.plane, CHAT, None, &watched).expect("a record");
    file(&home.join("project/.envrc"), "not looked at\n");
    file(&home.join(".config/fish/config.fish"), "x\n");
    let looked = arrived::look(&fix.plane, CHAT, None)
        .expect("a look")
        .expect("a record");
    assert_eq!(
        looked.new,
        [home.join(".config/fish").display().to_string()]
    );
}

/// A remote with a tracked `.vscode/tasks.json` on its `main`, its clone in the folder a chat
/// watches, and the clone's first commit before the file.
fn clone_behind(fix: &Fixture) -> PathBuf {
    let remote = fix.plane.join("remote");
    fs::create_dir_all(&remote).expect("a folder");
    git(&remote, &["init", "-q", "-b", "main"]);
    git(&remote, &["commit", "-q", "--allow-empty", "-m", "start"]);
    let clone = fix.folder.join("clone");
    git(
        &fix.folder,
        &["clone", "-q", remote.to_str().expect("a path"), "clone"],
    );
    file(&remote.join(".vscode/tasks.json"), "tracked\n");
    git(&remote, &["add", "."]);
    git(&remote, &["commit", "-q", "-m", "editor"]);
    clone
}

#[test]
fn what_an_update_of_a_clone_brings_from_its_remote_is_not_told() {
    let fix = Fixture::new();
    let clone = clone_behind(&fix);
    fix.begin();
    git(&clone, &["pull", "-q", "--ff-only"]);
    assert!(clone.join(".vscode/tasks.json").exists());
    assert_eq!(fix.look(), Vec::<String>::new());
}

#[test]
fn a_file_only_a_local_commit_or_the_index_has_is_told() {
    let fix = Fixture::new();
    let clone = clone_behind(&fix);
    fix.begin();
    file(&clone.join(".husky/pre-commit"), "x\n");
    git(&clone, &["add", "."]);
    git(&clone, &["commit", "-q", "-m", "local"]);
    assert_eq!(fix.look(), ["clone/.husky"]);
}

#[test]
fn a_tracked_name_with_other_contents_is_told() {
    let fix = Fixture::new();
    let clone = clone_behind(&fix);
    git(&clone, &["fetch", "-q"]);
    fix.begin();
    file(&clone.join(".vscode/tasks.json"), "not what it tracks\n");
    assert_eq!(fix.look(), ["clone/.vscode"]);
}

#[test]
fn a_branch_folder_the_clone_registers_is_not_told_and_a_file_its_index_does_not_have_is() {
    let fix = Fixture::new();
    let clone = clone_behind(&fix);
    git(&clone, &["pull", "-q", "--ff-only"]);
    fix.begin();
    let branch = fix.folder.join("branch");
    git(
        &clone,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "lane",
            branch.to_str().expect("a path"),
        ],
    );
    assert!(branch.join(".vscode/tasks.json").exists());
    assert_eq!(fix.look(), Vec::<String>::new());

    file(&branch.join(".envrc"), "x\n");
    assert_eq!(fix.look(), ["branch/.envrc"]);
}

#[test]
fn a_git_file_that_only_says_it_is_a_branch_folder_is_told() {
    let fix = Fixture::new();
    let clone = clone_behind(&fix);
    git(&clone, &["worktree", "add", "-q", "-b", "other", "../real"]);
    fix.begin();
    // It names a real registration, which names another folder back.
    let forged = fix.folder.join("forged");
    file(
        &forged.join(".git"),
        &format!("gitdir: {}\n", clone.join(".git/worktrees/real").display()),
    );
    assert_eq!(fix.look(), ["forged/.git"]);
}

#[test]
fn a_clone_a_workspaces_manifest_names_is_not_told_and_another_is() {
    let dir = tempfile::tempdir().expect("a folder");
    let plane = dir.path().canonicalize().expect("real");
    let workspace = plane.join("workspaces/alpha");
    fs::create_dir_all(&workspace).expect("a workspace");
    let watched = Watched {
        folders: vec![workspace.clone()],
        ..Watched::default()
    };
    arrived::begin(&plane, CHAT, None, &watched).expect("a record");
    file(
        &workspace.join("workspace.json"),
        r#"{"name": "alpha", "repos": [{"name": "named"}]}"#,
    );
    for repo in ["named", "unnamed"] {
        let at = workspace.join(repo);
        fs::create_dir_all(&at).expect("a folder");
        git(&at, &["init", "-q", "-b", "main"]);
    }
    let looked = arrived::look(&plane, CHAT, None)
        .expect("a look")
        .expect("a record");
    assert_eq!(
        looked.new,
        [workspace.join("unnamed/.git").display().to_string()]
    );
}

#[test]
fn the_notice_names_the_chat_and_each_path_once_and_a_name_cannot_forge_a_line() {
    let fix = Fixture::new();
    fix.begin();
    file(&fix.folder.join("a\nb/.envrc"), "x\n");
    file(&fix.folder.join(".mcp.json"), "{}\n");
    fix.look();
    let words = arrived::told(&fix.plane)[0].words(&fix.plane);
    assert!(words.starts_with("Fix the build: "), "{words}");
    assert!(words.contains("work/.mcp.json"), "{words}");
    assert!(!words.contains('\n'), "{words}");
    assert_eq!(words.matches(".envrc").count(), 1, "{words}");
    assert!(
        words.contains("can run outside the sandbox if you or a tool work in that folder"),
        "{words}"
    );
}

#[test]
fn a_notice_with_more_paths_than_it_shows_counts_the_rest() {
    let fix = Fixture::new();
    fix.begin();
    for at in 0..arrived::MOST_SHOWN + 3 {
        file(&fix.folder.join(format!("{at}/.envrc")), "x\n");
    }
    fix.look();
    let words = arrived::told(&fix.plane)[0].words(&fix.plane);
    assert!(words.contains("and 3 more"), "{words}");
}

/// The record is what decides what is told, so no sandboxed chat writes it: it is under the app's
/// folder of the state, as every harness is compiled to deny.
#[test]
fn the_record_is_in_the_folder_every_harness_denies_a_chat_writing() {
    let fix = Fixture::new();
    assert!(
        arrived::dir(&fix.plane).starts_with(crate::names::state(&fix.plane).join("app")),
        "{}",
        arrived::dir(&fix.plane).display()
    );
}
