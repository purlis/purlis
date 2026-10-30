// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A declared tool's run reads only repository configuration the person controls (#3868).

use std::fs;
use std::path::{Path, PathBuf};

use super::TEMPS_HERE;
use super::repo::held;

/// A project at `<tmp>/project`, which a chat writes but for what class 5 holds, and a scratch
/// folder at `<tmp>/scratch` a chat writes whole; the machine's temporary folders are none here.
struct Place {
    _dir: tempfile::TempDir,
    root: PathBuf,
    scratch: PathBuf,
    outside: PathBuf,
}

impl Place {
    fn new() -> Self {
        TEMPS_HERE.with(|temps| *temps.borrow_mut() = Some(Vec::new()));
        let dir = tempfile::tempdir().expect("tempdir");
        let base = dir.path().canonicalize().expect("real");
        let (root, scratch, outside) = (
            base.join("project"),
            base.join("scratch"),
            base.join("outside"),
        );
        for one in [&root, &scratch, &outside] {
            fs::create_dir_all(one).expect("mkdir");
        }
        Self {
            _dir: dir,
            root,
            scratch,
            outside,
        }
    }

    fn judged(&self, folder: &Path, glab: bool) -> Result<(), String> {
        held(folder, &self.root, &[self.scratch.clone()], glab)
    }
}

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    fs::write(path, text).expect("write");
}

/// A repository's git folder at `git`, with `config`.
fn git_folder(git: &Path, config: &str) {
    write(&git.join("HEAD"), "ref: refs/heads/main\n");
    fs::create_dir_all(git.join("objects")).expect("objects");
    fs::create_dir_all(git.join("refs")).expect("refs");
    write(&git.join("config"), config);
}

#[test]
fn a_folder_in_no_repository_runs() {
    let place = Place::new();
    let folder = place.scratch.join("work");
    fs::create_dir_all(&folder).expect("mkdir");
    assert_eq!(place.judged(&folder, false), Ok(()));
}

#[test]
fn the_project_s_own_clone_and_its_branch_folders_run() {
    let place = Place::new();
    let clone = place.root.join("ws/repo");
    git_folder(&clone.join(".git"), "[core]\n\tbare = false\n");
    fs::create_dir_all(clone.join("src")).expect("mkdir");
    assert_eq!(place.judged(&clone.join("src"), false), Ok(()));
    // A branch folder: its `.git` file names the clone's `.git/worktrees/<name>`.
    let wt = clone.join(".git/worktrees/piece");
    write(&wt.join("HEAD"), "ref: refs/heads/piece\n");
    write(&wt.join("commondir"), "../..\n");
    let piece = place.root.join("ws/.worktrees/repo/piece");
    write(&piece.join(".git"), &format!("gitdir: {}\n", wt.display()));
    assert_eq!(place.judged(&piece, false), Ok(()));
}

#[test]
fn a_repository_in_a_folder_a_chat_writes_whole_never_runs() {
    let place = Place::new();
    let repo = place.scratch.join("made");
    git_folder(&repo.join(".git"), "[core]\n\tfsmonitor = ./x\n");
    let why = place.judged(&repo, false).expect_err("refused");
    assert!(why.contains("own terminal"), "{why}");
}

#[test]
fn a_git_file_pointing_at_a_folder_a_chat_writes_never_runs() {
    let place = Place::new();
    git_folder(&place.scratch.join("gd"), "[core]\n\thooksPath = /x\n");
    let folder = place.root.join("ws/here");
    write(
        &folder.join(".git"),
        &format!("gitdir: {}\n", place.scratch.join("gd").display()),
    );
    assert!(place.judged(&folder, false).is_err());
    // Relative, as git reads it from the folder that holds the `.git` file.
    write(&folder.join(".git"), "gitdir: ../../../scratch/gd\n");
    assert!(place.judged(&folder, false).is_err());
}

#[test]
fn a_bare_repository_or_a_git_folder_itself_is_judged_as_a_repository() {
    let place = Place::new();
    let bare = place.root.join("ws/bare");
    git_folder(&bare, "[core]\n\tbare = true\n");
    assert!(place.judged(&bare.join("refs"), false).is_err());
    let clone = place.root.join("ws/repo");
    git_folder(&clone.join(".git"), "");
    assert_eq!(place.judged(&clone.join(".git/refs"), false), Ok(()));
}

#[test]
fn an_included_config_or_a_hooks_folder_a_chat_writes_never_runs() {
    let place = Place::new();
    let clone = place.root.join("ws/repo");
    for config in [
        "[include]\n\tpath = ../shared.gitconfig\n",
        "[includeIf \"gitdir:**\"]\n\tpath = ../../shared.gitconfig\n",
        "[core]\n\thooksPath = .husky\n",
        "[core]\n\tHooksPath = \"tools/hooks\"\n",
        &format!("[core]\n\thookspath = {}\n", place.scratch.display()),
    ] {
        git_folder(&clone.join(".git"), config);
        assert!(place.judged(&clone, false).is_err(), "{config}");
    }
    write(
        &place.outside.join("shared.gitconfig"),
        "[user]\n\tname = x\n",
    );
    for config in [
        format!(
            "[include]\n\tpath = {}\n",
            place.outside.join("shared.gitconfig").display()
        ),
        format!("[core]\n\thooksPath = {}\n", place.outside.display()),
        "[core]\n\thooksPath = .git/hooks\n".to_owned(),
    ] {
        git_folder(&clone.join(".git"), &config);
        assert_eq!(place.judged(&clone, false), Ok(()), "{config}");
    }
}

#[test]
fn a_config_included_in_turn_is_judged_too() {
    let place = Place::new();
    let clone = place.root.join("ws/repo");
    let first = place.outside.join("first.gitconfig");
    write(
        &first,
        &format!(
            "[include]\n\tpath = {}\n",
            place.scratch.join("x").display()
        ),
    );
    git_folder(
        &clone.join(".git"),
        &format!("[include]\n\tpath = {}\n", first.display()),
    );
    assert!(place.judged(&clone, false).is_err());
}

#[test]
fn glab_s_own_settings_in_a_repository_never_run_with_glab() {
    let place = Place::new();
    let clone = place.root.join("ws/repo");
    git_folder(&clone.join(".git"), "");
    write(
        &clone.join(".git/glab-cli/config.yml"),
        "host: example.com\n",
    );
    assert!(place.judged(&clone, true).is_err());
    assert_eq!(place.judged(&clone, false), Ok(()));
}

#[test]
fn a_repository_outside_every_folder_a_chat_writes_runs() {
    let place = Place::new();
    let repo = place.outside.join("theirs");
    git_folder(&repo.join(".git"), "[core]\n\thooksPath = hooks\n");
    assert_eq!(place.judged(&repo, false), Ok(()));
}
