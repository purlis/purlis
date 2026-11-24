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

// -------------------------------------------------------------------------------------
// A chat writes the git record of the branch folder purlis cut for it, and no other (#7550)
// -------------------------------------------------------------------------------------

impl Project {
    /// What the app writes when it cuts a branch folder: the record names the folder back, as
    /// `git worktree add` leaves it, and a second record `q` of the same clone, with its folder.
    fn recorded(&self, repo: &str, clone: &str) -> PathBuf {
        let git = self.workspace.join(clone).join(".git");
        let record = git.join("worktrees/p1");
        std::fs::write(
            record.join("gitdir"),
            format!("{}\n", self.piece(repo).join(".git").display()),
        )
        .unwrap();
        let other = git.join("worktrees/q");
        std::fs::create_dir_all(&other).unwrap();
        std::fs::write(other.join("commondir"), "../..\n").unwrap();
        let folder = self.workspace.join(".worktrees").join(repo).join("q");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(
            folder.join(".git"),
            format!("gitdir: {}\n", other.display()),
        )
        .unwrap();
        std::fs::write(
            other.join("gitdir"),
            format!("{}\n", folder.join(".git").display()),
        )
        .unwrap();
        git
    }
}

#[test]
fn a_wrapped_chat_writes_the_git_record_of_its_own_branch_folder_and_nothing_else_of_the_clone() {
    let project = Project::new();
    let git = project.recorded("a", "repos/a");
    for harness in WRAPPED {
        let linux = profile_in(&project.started(harness, Os::Linux), &project.piece("a"));
        for mine in [
            "worktrees/p1/index.lock",
            "worktrees/p1/index",
            "worktrees/p1/HEAD",
            "worktrees/p1/FETCH_HEAD",
            "worktrees/p1/ORIG_HEAD",
            "worktrees/p1/MERGE_HEAD",
            "worktrees/p1/logs/HEAD",
            "worktrees/p1/refs/bisect/bad",
        ] {
            assert!(writes(&linux, &git.join(mine)), "{harness:?}: {mine}");
        }
        for not_mine in [
            // Another worktree's record, and a name that only starts like this one's.
            "worktrees/q/index.lock",
            "worktrees/q/FETCH_HEAD",
            "worktrees/p10/index.lock",
            // What inside its own record points git elsewhere or runs code.
            "worktrees/p1/commondir",
            "worktrees/p1/gitdir",
            "worktrees/p1/config.worktree",
            "worktrees/p1/hooks/pre-commit",
            "worktrees/p1/info/attributes",
            // The clone's own: config, hooks, the refs a branch is, HEAD, objects.
            "config",
            "hooks/pre-commit",
            "refs/heads/p1",
            "refs/heads/main",
            "refs/tags/v1",
            "packed-refs",
            "HEAD",
            "objects/ab/cdef",
            "objects/pack/pack-1.pack",
            "index.lock",
        ] {
            assert!(
                !writes(&linux, &git.join(not_mine)),
                "{harness:?}: {not_mine}"
            );
        }
        // Seatbelt is handed the same: the record allowed after the clone's folder is denied.
        let mac = profile_in(&project.started(harness, Os::MacOs), &project.piece("a"));
        let record = real(&git.join("worktrees/p1"));
        let allow = format!("(allow file-write* (subpath \"{}\"))", record.display());
        let deny = format!(
            "(deny file-write* (subpath \"{}/commondir\"))",
            record.display()
        );
        let (at_allow, at_deny) = (mac.rfind(&allow), mac.rfind(&deny));
        assert!(
            at_allow.is_some() && at_deny > at_allow,
            "{harness:?}: {mac}"
        );
    }
}

#[test]
fn a_wrapped_chat_is_given_no_git_record_that_does_not_name_its_folder_back() {
    let project = Project::new();
    let git = project.recorded("a", "repos/a");
    let piece = project.piece("a");
    let index = git.join("worktrees/p1/index.lock");
    let given = |project: &Project| {
        let linux = profile_in(&project.started(Harness::Codex, Os::Linux), &piece);
        writes(&linux, &index)
    };
    assert!(given(&project), "the record as the app leaves it");
    // `gitdir` naming another folder.
    std::fs::write(
        git.join("worktrees/p1/gitdir"),
        format!("{}\n", project.piece("b").join(".git").display()),
    )
    .unwrap();
    assert!(!given(&project), "a record that names another folder");
    // No `gitdir` at all, as the other tests' records.
    std::fs::remove_file(git.join("worktrees/p1/gitdir")).unwrap();
    assert!(!given(&project), "a record that names no folder");
    // A folder whose `.git` names the other worktree's record: that one names its own folder.
    std::fs::write(
        piece.join(".git"),
        format!("gitdir: {}\n", git.join("worktrees/q").display()),
    )
    .unwrap();
    let linux = profile_in(&project.started(Harness::Codex, Os::Linux), &piece);
    assert!(!writes(&linux, &git.join("worktrees/q/index.lock")));
    assert!(!writes(&linux, &index));
}

#[test]
fn a_chat_that_is_not_in_a_branch_folder_is_given_no_git_record() {
    let project = Project::new();
    let git = project.recorded("a", "repos/a");
    for cwd in [project.root.clone(), project.workspace.clone()] {
        let linux = profile_in(&project.started(Harness::Codex, Os::Linux), &cwd);
        assert!(
            !writes(&linux, &git.join("worktrees/p1/index.lock")),
            "{cwd:?}"
        );
    }
}

#[test]
fn a_claude_code_chat_names_the_git_record_of_its_own_branch_folder_to_write() {
    let project = Project::new();
    let git = project.recorded("a", "repos/a");
    let applied = project.started(Harness::ClaudeCode, Os::MacOs);
    let Form::ClaudeCode(settings) = applied.form_in(Some(&project.piece("a"))) else {
        panic!("Claude Code's own sandbox");
    };
    let strings = |key: &str| -> Vec<String> {
        settings.sandbox["filesystem"][key]
            .as_array()
            .map(|list| {
                list.iter()
                    .filter_map(|it| it.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    };
    let record = real(&git.join("worktrees/p1")).display().to_string();
    assert!(
        strings("allowWrite").contains(&record),
        "{:?}",
        strings("allowWrite")
    );
    for held in ["commondir", "gitdir", "config.worktree", "hooks", "info"] {
        assert!(
            strings("denyWrite").contains(&format!("{record}/{held}")),
            "{held}"
        );
    }
    // Never another record, and the clone's git folder stays denied whole.
    let other = real(&git.join("worktrees/q")).display().to_string();
    assert!(!strings("allowWrite").iter().any(|it| it.contains(&other)));
    assert!(strings("denyWrite").contains(&real(&git).display().to_string()));
    // In a chat of another folder: nothing.
    let Form::ClaudeCode(settings) = applied.form_in(Some(&project.workspace)) else {
        panic!("Claude Code's own sandbox");
    };
    assert!(!settings.sandbox["filesystem"].to_string().contains(&record));
}
