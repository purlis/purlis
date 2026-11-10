// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The git folders the app alone writes, held from a chat of every harness purlis wraps: the
//! project root's `.git`, and a branch folder's clone's, in both layouts and wherever its
//! record's `commondir` leads. Each profile is read as the Linux wrap reads it, and on macOS its
//! rules are the ones Seatbelt is handed.

use std::path::{Path, PathBuf};

use purlis_linux_sandbox::profile::{FileOp, Kind, Op, Profile};

use super::*;

const ON: &str = "[sandbox]\nmode = \"on\"\negress = [\"model-providers\", \"forge\"]\n";

/// A project with a git folder of its own, a workspace with a clone in each layout, and a
/// branch folder cut from each, whose record's `commondir` names its clone's git folder; and a
/// third branch folder whose record lives in a clone that is not its repo's by name.
struct Project {
    _plane: tempfile::TempDir,
    _home: tempfile::TempDir,
    home: PathBuf,
    root: PathBuf,
    workspace: PathBuf,
}

impl Project {
    fn new() -> Self {
        let plane = tempfile::tempdir().expect("a plane");
        std::fs::write(plane.path().join("charter.toml"), ON).expect("charter.toml");
        let root = real(plane.path());
        std::fs::create_dir_all(root.join(".git/refs/heads")).unwrap();
        let workspace = root.join("workspaces/w");
        for (repo, clone) in [("a", "repos/a"), ("b", "b"), ("c", "repos/elsewhere")] {
            let record = workspace.join(clone).join(".git/worktrees/p1");
            std::fs::create_dir_all(&record).unwrap();
            std::fs::write(record.join("commondir"), "../..\n").unwrap();
            let piece = workspace.join(".worktrees").join(repo).join("p1");
            std::fs::create_dir_all(&piece).unwrap();
            std::fs::write(
                piece.join(".git"),
                format!("gitdir: {}\n", record.display()),
            )
            .unwrap();
        }
        let home = tempfile::tempdir().expect("a home");
        Self {
            home: real(home.path()),
            _home: home,
            _plane: plane,
            root,
            workspace,
        }
    }

    fn piece(&self, repo: &str) -> PathBuf {
        self.workspace.join(".worktrees").join(repo).join("p1")
    }

    fn machine(&self, os: Os) -> Machine {
        Machine {
            env: crate::secrets::Env::of(&[]),
            home: Some(self.home.clone()),
            os,
        }
    }

    /// What a chat of `harness` on `os` starts under, on a profile for sandboxed chats where
    /// its harness keeps one.
    fn started(&self, harness: Harness, os: Os) -> Applied {
        let mut applied = for_start(harness, &self.root, &self.machine(os), &|_| true)
            .expect("starts")
            .expect("sandboxed");
        let config = self.home.join("profiles/sandboxed");
        std::fs::create_dir_all(&config).unwrap();
        applied.on_profile(&signin::ProfileFolders {
            config: Some(config),
            data: None,
            sandbox_only: Some("sandboxed".to_owned()),
            state: None,
            default_config: None,
        });
        applied
    }
}

fn real(path: &Path) -> PathBuf {
    path.canonicalize().expect("a real path")
}

/// The profile a chat in `cwd` is wrapped in.
fn profile_in(applied: &Applied, cwd: &Path) -> String {
    let tmp = Path::new("/private/var/folders/xx/chat-tmp");
    match applied.form_in(Some(cwd)) {
        Form::Opencode(wrap) => opencode::profile(&wrap, cwd, tmp, &[4040], None),
        Form::Codex(wrap) => codex::profile(&wrap, cwd, tmp, &[4040], None),
        Form::ClaudeWrapped(wrap) => claude_wrap::profile(&wrap, cwd, tmp, &[4040], None),
        Form::ClaudeCode(_) => panic!("Claude Code's own sandbox is not a profile"),
    }
    .expect("a profile")
}

/// Whether `profile`, read as the Linux wrap reads it, lets a file at `path` be written.
fn writes(profile: &str, path: &Path) -> bool {
    Profile::parse(profile)
        .expect("it reads")
        .allows(&Op::File {
            what: FileOp::WriteData,
            path,
            kind: Kind::Regular,
        })
}

/// Whether `profile` holds the folder `path` and what is below it from being written, by the
/// rules Seatbelt is handed for it.
fn holds(profile: &str, path: &Path) -> bool {
    seatbelt::denial_rules(&Denial {
        class: Class::LaterCode,
        path: path.to_path_buf(),
        access: Access::Write,
        named: None,
        reach: Reach::AndBelow,
        any_case: false,
    })
    .is_ok_and(|rules| !rules.is_empty() && rules.iter().all(|rule| profile.contains(rule)))
}

const WRAPPED: [Harness; 2] = [Harness::Codex, Harness::Opencode];

#[test]
fn a_wrapped_chat_anywhere_in_the_project_never_writes_the_project_roots_git_folder() {
    let project = Project::new();
    let held = project.root.join(".git");
    let chats = [
        project.root.clone(),
        project.workspace.clone(),
        project.piece("a"),
    ];
    for harness in WRAPPED {
        let applied = project.started(harness, Os::MacOs);
        for cwd in &chats {
            let profile = profile_in(&applied, cwd);
            assert!(holds(&profile, &held), "{harness:?} in {cwd:?}");
        }
        let applied = project.started(harness, Os::Linux);
        for cwd in &chats {
            let profile = profile_in(&applied, cwd);
            assert!(
                !writes(&profile, &held.join("refs/heads/main")),
                "{harness:?} in {cwd:?}"
            );
            assert!(
                !writes(&profile, &held.join("HEAD")),
                "{harness:?} in {cwd:?}"
            );
        }
        // What a chat at the root writes is still its own.
        let profile = profile_in(&applied, &project.root);
        assert!(
            writes(&profile, &project.root.join("notes.md")),
            "{harness:?}"
        );
    }
}

#[test]
fn a_wrapped_chat_in_a_branch_folder_never_writes_its_clones_git_folder_in_either_layout() {
    let project = Project::new();
    let cases = [
        ("a", project.workspace.join("repos/a/.git")),
        ("b", project.workspace.join("b/.git")),
        // The record's `commondir` names a clone that is not the repo's by name.
        ("c", project.workspace.join("repos/elsewhere/.git")),
    ];
    for harness in WRAPPED {
        for (repo, held) in &cases {
            let piece = project.piece(repo);
            let mac = profile_in(&project.started(harness, Os::MacOs), &piece);
            assert!(holds(&mac, held), "{harness:?} in {repo}");
            let linux = profile_in(&project.started(harness, Os::Linux), &piece);
            for inside in ["worktrees/p1/index", "worktrees/p1/HEAD", "refs/heads/p1"] {
                assert!(
                    !writes(&linux, &held.join(inside)),
                    "{harness:?} in {repo}: {inside}"
                );
            }
            assert!(
                writes(&linux, &piece.join("src.rs")),
                "{harness:?} in {repo}"
            );
        }
    }
}

#[test]
fn a_wrapped_chat_never_writes_a_later_code_name_under_another_spelling() {
    let project = Project::new();
    let cwd = project.piece("a");
    for harness in WRAPPED {
        for os in [Os::MacOs, Os::Linux] {
            let profile = profile_in(&project.started(harness, os), &cwd);
            for name in [
                ".GIT/config",
                ".Git/hooks/pre-commit",
                ".ZSHRC",
                ".VSCode/tasks.json",
            ] {
                assert!(
                    !writes(&profile, &cwd.join("sub").join(name)),
                    "{harness:?} on {os:?}: {name}"
                );
            }
            assert!(writes(&profile, &cwd.join("sub/README")), "{harness:?}");
        }
    }
}

#[test]
fn every_wrapped_chat_records_the_project_roots_git_folder_as_held() {
    let project = Project::new();
    for harness in WRAPPED {
        for os in [Os::MacOs, Os::Linux] {
            assert_eq!(
                project.started(harness, os).confines().roots_git,
                Some(project.root.join(".git")),
                "{harness:?} on {os:?}"
            );
        }
    }
}
