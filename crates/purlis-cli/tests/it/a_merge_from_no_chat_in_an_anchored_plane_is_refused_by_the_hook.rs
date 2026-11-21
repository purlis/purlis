// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! M75-07b, review F1 to F3, through the real `purlis hook pretooluse`: a merge asked by a process
//! the app did not start is refused when it acts in a plane the app anchored, and runs (the shipped
//! default asks the person in their own terminal) when it does not.
//!
//! **These run where no app is above the test and the test is not sandboxed** (CI): a process
//! under the app, or in a sandbox, is a chat's and is asked of the app instead, so in a chat
//! they say nothing about this path.
//!
//! **The hook cannot read the sealed registry** (it has no seal key), so the app's list of
//! anchored planes (`<config>/app/anchored-planes/`) is what it reads here: each test writes that
//! list as the app does, a file whose text is the plane's real path. No socket is bound, so these
//! run anywhere the binary does.

use crate::support;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;

struct Machine {
    _tmp: tempfile::TempDir,
    root: PathBuf,
    elsewhere: PathBuf,
    home: PathBuf,
    config: PathBuf,
}

impl Machine {
    /// A plane, a folder beside it that is no plane's, and a config home of the test's own.
    fn new() -> Self {
        let tmp = tempfile::tempdir().expect("a directory");
        let base = tmp.path().canonicalize().expect("a real path");
        let (root, elsewhere) = (base.join("plane"), base.join("elsewhere"));
        let (home, config) = (base.join("home"), base.join("config"));
        for dir in [&root, &elsewhere, &home, &config] {
            std::fs::create_dir_all(dir).expect("a folder");
        }
        std::fs::create_dir_all(root.join(".git")).expect("the plane's git folder");
        std::fs::write(root.join("charter.toml"), "schema = 1\n").expect("the marker");
        Self {
            _tmp: tmp,
            root,
            elsewhere,
            home,
            config,
        }
    }

    /// What the app writes when it anchors the plane: the list a process without the seal key reads.
    fn anchored(&self) -> &Self {
        let list = self.config.join("purlis/app/anchored-planes");
        std::fs::create_dir_all(&list).expect("the list's folder");
        std::fs::write(list.join("plane"), self.root.display().to_string()).expect("the list");
        self
    }

    /// The plane's remote, as git keeps it.
    fn remote(&self, url: &str) -> &Self {
        std::fs::write(
            self.root.join(".git/config"),
            format!("[remote \"origin\"]\n\turl = {url}\n"),
        )
        .expect("the config");
        self
    }

    /// The hook's refusal of `command`, run from `process_dir` with the payload's `cwd` as given,
    /// or `None` where it let the command run. The hook has no chat's environment at all.
    fn denial(&self, process_dir: &Path, payload_cwd: &str, command: &str) -> Option<String> {
        let payload = serde_json::json!({
            "cwd": payload_cwd,
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "permission_mode": "default",
            "tool_input": {"command": command},
        })
        .to_string();
        let mut hook = support::purlis();
        hook.args(["hook", "pretooluse"])
            .current_dir(process_dir)
            .env_clear();
        // A run whose temp folder is not the one the build was fenced to widens the fence.
        if let Some(fence) = std::env::var_os("CHARTER_PLANE_FENCE") {
            hook.env("CHARTER_PLANE_FENCE", fence);
        }
        let mut child = hook
            .env("CHARTER_ROOT", &self.root)
            .env("HOME", &self.home)
            .env("PURLIS_CONFIG_HOME", &self.config)
            .env("PATH", "/usr/bin:/bin")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the hook runs");
        child
            .stdin
            .take()
            .expect("stdin")
            .write_all(payload.as_bytes())
            .expect("the payload");
        let out = child.wait_with_output().expect("the hook finishes");
        assert_eq!(out.status.code(), Some(0), "{command}");
        let said: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
        let output = said.get("hookSpecificOutput")?;
        (output.get("permissionDecision")?.as_str()? == "deny").then(|| {
            output
                .get("permissionDecisionReason")?
                .as_str()
                .map(str::to_owned)
        })?
    }

    fn refused_for_want_of_a_host(&self, process_dir: &Path, payload_cwd: &str, command: &str) {
        let reason = self
            .denial(process_dir, payload_cwd, command)
            .unwrap_or_else(|| panic!("not refused: {command}"));
        assert!(
            reason.contains("no session host to ask"),
            "{command}: {reason}"
        );
    }
}

const MERGE: &str = "gh pr merge 12 --squash";

#[test]
fn a_merge_in_an_anchored_plane_is_refused_where_the_registry_cannot_be_read() {
    let machine = Machine::new();
    machine.anchored();
    let root = machine.root.display().to_string();
    machine.refused_for_want_of_a_host(&machine.root, &root, MERGE);
}

#[test]
fn the_same_merge_in_a_plane_the_app_never_anchored_runs() {
    let machine = Machine::new();
    let root = machine.root.display().to_string();
    assert_eq!(machine.denial(&machine.root, &root, MERGE), None);
}

#[test]
fn an_empty_payload_cwd_is_the_hooks_own_folder_and_an_anchored_plane_refuses() {
    let machine = Machine::new();
    machine.anchored();
    machine.refused_for_want_of_a_host(&machine.root, "", MERGE);
}

#[test]
fn a_cd_or_git_dash_c_into_an_anchored_plane_from_outside_is_refused() {
    let machine = Machine::new();
    machine.anchored();
    let outside = machine.elsewhere.display().to_string();
    let plane = machine.root.display().to_string();
    for command in [
        format!("cd {plane} && {MERGE}"),
        format!("git -C {plane} status && {MERGE}"),
        format!("git -C {plane} status && git -C /nowhere status && {MERGE}"),
        format!("git --git-dir={plane}/.git status && {MERGE}"),
    ] {
        machine.refused_for_want_of_a_host(&machine.elsewhere, &outside, &command);
    }
    // And a folder outside any plane stays the shipped default's, which runs.
    assert_eq!(
        machine.denial(&machine.elsewhere, &outside, MERGE),
        None,
        "a merge outside every plane runs under the shipped default"
    );
}

#[test]
fn a_folder_the_shell_would_expand_is_refused_never_taken_as_outside() {
    let machine = Machine::new();
    machine.anchored();
    let outside = machine.elsewhere.display().to_string();
    for command in [
        "git -C \"$PLANE\" status && gh pr merge 12",
        "git -C ~/plane status && gh pr merge 12",
        "cd \"$(git rev-parse --show-toplevel)\" && gh pr merge 12",
    ] {
        let reason = machine
            .denial(&machine.elsewhere, &outside, command)
            .unwrap_or_else(|| panic!("not refused: {command}"));
        assert!(reason.contains("cannot be read"), "{command}: {reason}");
    }
}

#[test]
fn a_merge_naming_an_anchored_planes_repo_is_refused_from_outside_it() {
    let machine = Machine::new();
    machine.anchored().remote("https://github.com/Acme/Web.git");
    let outside = machine.elsewhere.display().to_string();
    for command in [
        "gh pr merge 12 -R acme/web",
        "gh pr merge 12 --repo=github.com/acme/web",
        "GH_REPO=acme/web gh pr merge 12",
        "gh pr merge https://github.com/acme/web/pull/12",
    ] {
        machine.refused_for_want_of_a_host(&machine.elsewhere, &outside, command);
    }
    // A repo that is no plane's runs.
    assert_eq!(
        machine.denial(&machine.elsewhere, &outside, "gh pr merge 12 -R acme/other"),
        None
    );
}
