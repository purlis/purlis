// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A plane with one clone in it, built from nothing, for tests that need real git.
//!
//! **Pinned author and dates**, so two runs of the same steps produce the same commit shas —
//! the differential compares what two implementations did to one repository, and that is only
//! meaningful if the repository itself is reproducible.
//!
//! Not every test binary that includes this needs every helper in it, and an unused one here
//! is not a defect in the helper.
#![allow(dead_code)]

pub mod folders;
pub mod forge_cli;
pub mod shipped;

use std::path::{Path, PathBuf};
use std::process::Command;

pub struct Fixture {
    _dir: tempfile::TempDir,
    pub plane: PathBuf,
    pub ws: String,
    pub repo: String,
    pub clone: PathBuf,
}

const WHO: [(&str, &str); 6] = [
    ("GIT_AUTHOR_NAME", "charter tests"),
    ("GIT_AUTHOR_EMAIL", "tests@example.invalid"),
    ("GIT_AUTHOR_DATE", "2026-01-01T00:00:00+00:00"),
    ("GIT_COMMITTER_NAME", "charter tests"),
    ("GIT_COMMITTER_EMAIL", "tests@example.invalid"),
    ("GIT_COMMITTER_DATE", "2026-01-01T00:00:00+00:00"),
];

/// The template every fixture repository is made from: `commit.gpgsign` and `tag.gpgsign` off,
/// and `core.excludesFile` pointed at nothing (charter-app#191, #262).
pub const TEMPLATE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support/git-template");

/// `git`, with every repository it creates made from [`TEMPLATE`], so none asks a developer's
/// signer (charter-app#191) — `src/testgit.rs`'s `unsigned`, for the integration tests.
pub fn unsigned() -> Command {
    let mut cmd = Command::new("git");
    cmd.arg("-c").arg(format!("init.templateDir={TEMPLATE}"));
    cmd
}

/// git, for the test's own setup. Not the code under test.
pub fn git(dir: &Path, args: &[&str]) -> std::process::Output {
    let out = git_tried(dir, args);
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

/// [`git`], whether or not git succeeds: its output, for a test that expects it to stop.
pub fn git_tried(dir: &Path, args: &[&str]) -> std::process::Output {
    let mut cmd = unsigned();
    cmd.arg("-C").arg(dir).args(args);
    for (k, v) in WHO {
        cmd.env(k, v);
    }
    // A developer's own `init.defaultBranch`, hooks or templates must not reach a fixture.
    cmd.env("GIT_CONFIG_GLOBAL", "/dev/null");
    cmd.env("GIT_CONFIG_SYSTEM", "/dev/null");
    purlis_core::forklock::output(&mut cmd).expect("git runs")
}

pub fn plane_with_clone(repo: &str) -> Fixture {
    plane_with_clone_at(repo, repo)
}

/// [`plane_with_clone`], with the clone in the workspace's `repos/` folder, where a repo goes
/// from structure version 6 on (ADR 0114).
pub fn plane_with_clone_in_repos(repo: &str) -> Fixture {
    plane_with_clone_at(repo, &format!("repos/{repo}"))
}

/// A plane with clone `repo` at `below` its workspace's folder.
fn plane_with_clone_at(repo: &str, below: &str) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    // Resolved: on macOS a temp dir is under `/var/folders/…`, itself a link to
    // `/private/var/…`, and a test that compares an unresolved path against a resolved one
    // fails for a reason that has nothing to do with what it is testing.
    let plane = std::fs::canonicalize(dir.path()).unwrap();
    std::fs::write(plane.join("charter.toml"), "schema = 1\n").unwrap();
    let ws = "alpha".to_string();
    let clone = plane.join("workspaces").join(&ws).join(below);
    std::fs::create_dir_all(&clone).unwrap();
    git(&clone, &["init", "-q", "-b", "main", "."]);
    std::fs::write(clone.join("README.md"), "one\n").unwrap();
    git(&clone, &["add", "README.md"]);
    git(&clone, &["commit", "-q", "-m", "one"]);
    Fixture {
        _dir: dir,
        plane,
        ws,
        repo: repo.to_string(),
        clone,
    }
}

impl Fixture {
    /// Give the plane a layer worth carrying: the settings that hold `$CHARTER_HARNESS`, the
    /// plugin and a shared ask rule; a machine-local deny; and one persona agent.
    ///
    /// Not part of `plane_with_clone`, because a plane with nothing to carry is its own case —
    /// `want` is empty there, charter writes nothing, and no chat is refused over it.
    pub fn give_the_plane_a_layer(&self) {
        let claude = self.plane.join(".claude");
        std::fs::create_dir_all(claude.join("agents")).unwrap();
        std::fs::write(
            claude.join("settings.json"),
            concat!(
                "{\n",
                "  \"enabledPlugins\": {\"charter@charter\": true},\n",
                "  \"env\": {\"CHARTER_HARNESS\": \"claude-code\"},\n",
                "  \"permissions\": {\"allow\": [\"Bash(ls *)\"], ",
                "\"ask\": [\"Bash(charter handoff *)\"]},\n",
                "  \"hooks\": {\"PreToolUse\": []}\n",
                "}\n"
            ),
        )
        .unwrap();
        std::fs::write(
            claude.join("settings.local.json"),
            "{\"permissions\": {\"deny\": [\"Bash(rm -rf /*)\"]}}\n",
        )
        .unwrap();
        std::fs::write(
            claude.join("agents").join("steward.md"),
            "# steward\n\nThe control plane steward.\n",
        )
        .unwrap();
    }

    /// What `git status` says in `tree` — empty when charter left nothing showing.
    pub fn status(&self, tree: &Path) -> String {
        String::from_utf8_lossy(&git(tree, &["status", "--porcelain"]).stdout).into_owned()
    }

    /// A commit in any tree of this fixture.
    pub fn commit(&self, tree: &Path, message: &str) {
        std::fs::write(tree.join(message), message).unwrap();
        git(tree, &["add", "-A"]);
        git(tree, &["commit", "-q", "-m", message]);
    }

    /// The workspace directory, which is what a worktree path must stay inside.
    pub fn workspace(&self) -> PathBuf {
        self.plane.join("workspaces").join(&self.ws)
    }
}

/// **In a test of something else: no sandbox for the project at `root`'s chats on this machine
/// where purlis wraps a Claude Code chat** (Linux; ADR 0067 as amended 2026-11-10), as a person
/// sets it in Settings: `[sandbox] level = "off"` in the local file, confirmed in this machine's
/// record. There a sandboxed Claude Code chat runs only on a sandbox-only profile, which a
/// stand-in on a project's profile is not; elsewhere the chat starts sandboxed as it did.
pub fn no_claude_sandbox_here(root: &Path) {
    use purlis_core::harness::Harness;
    if !purlis_core::sandbox::Machine::this().purlis_wraps(Harness::ClaudeCode) {
        return;
    }
    let local = root.join(purlis_core::profiles::LOCAL_FILE);
    let text = std::fs::read_to_string(&local).unwrap_or_default();
    // First in the file, so what a test later appends to its last table stays in that table.
    if !text.contains("[sandbox]") {
        std::fs::write(&local, format!("[sandbox]\nlevel = \"off\"\n\n{text}"))
            .expect("the local file");
    }
    purlis_core::sandbox::personal::set_by_you(
        root,
        None,
        Some(purlis_core::sandbox::personal::Choice::Off),
    )
    .expect("your level");
}

/// Set on the child [`sandboxed_claude_profile`] started: the folder its homes are.
const SANDBOX_ONLY_HOMES: &str = "PURLIS_TEST_SANDBOX_ONLY_HOMES";

/// Where a test of a sandboxed Claude Code chat starts it ([`sandboxed_claude_profile`]).
pub enum ClaudeProfile {
    /// On the project's own profile `work`, as it always did: purlis does not wrap a Claude Code
    /// chat on this machine.
    Project,
    /// On this machine's sandbox-only profile `work`, whose config folder this is, in a child
    /// whose homes are the test's own.
    SandboxOnly(PathBuf),
    /// The child ran the test, and failed this one if it failed there.
    Ran,
}

/// **Where a test of a sandboxed Claude Code chat starts it.** On Linux every sandboxed Claude
/// Code chat runs inside purlis's wrap, on a sandbox-only profile (ADR 0067 as amended
/// 2026-11-10), which a project's own profile is not. There `test` (its full path, module first)
/// runs again in a child whose data and config homes are a folder of its own, with a stand-in
/// `claude` that answers `--version` as Claude Code does first on its `PATH`, and the child
/// makes the sandbox-only profile `work`. Elsewhere the test runs on the project's own profile.
pub fn sandboxed_claude_profile(test: &str) -> ClaudeProfile {
    use purlis_core::harness::Harness;
    if !purlis_core::sandbox::Machine::this().purlis_wraps(Harness::ClaudeCode) {
        return ClaudeProfile::Project;
    }
    if std::env::var_os(SANDBOX_ONLY_HOMES).is_some() {
        let homes = purlis_core::profilestore::Homes::here().expect("the test's homes");
        let made = purlis_core::profilestore::create_sandbox_only(&homes, "work", "claude", 1)
            .expect("the sandbox-only profile is made");
        return ClaudeProfile::SandboxOnly(made.record.folders.config);
    }
    let homes = tempfile::tempdir().expect("a folder for the homes");
    // Outside the system's temp folders, which a sandboxed chat may write: a sandboxed start
    // refuses a program there.
    let programs =
        tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).expect("a folder for the stand-in");
    stand_in::program(
        programs.path(),
        "claude",
        "#!/bin/sh\necho '2.1.288 (Claude Code)'\n",
    );
    let path = std::env::join_paths(std::iter::once(programs.path().to_path_buf()).chain(
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()),
    ))
    .expect("a PATH");
    purlis_core::testrun::rerun_steered(
        &[test],
        &[
            (purlis_core::datahome::HOME_VAR, homes.path().as_os_str()),
            (purlis_core::machine::HOME_VAR, homes.path().as_os_str()),
            (SANDBOX_ONLY_HOMES, homes.path().as_os_str()),
            ("PATH", path.as_os_str()),
        ],
    );
    ClaudeProfile::Ran
}
