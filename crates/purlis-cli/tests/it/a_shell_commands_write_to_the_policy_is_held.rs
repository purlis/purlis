// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! M75-16 (#7025), through the real `purlis hook pretooluse`: a shell command that writes the plane
//! root's `policy.cedar` is refused naming the rule, whatever the project's policy says, and a
//! branch folder's own copy is not. No socket is bound and no app is asked: the hold is fixed.

use crate::support;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;

struct Machine {
    _tmp: tempfile::TempDir,
    root: PathBuf,
    home: PathBuf,
    config: PathBuf,
}

impl Machine {
    /// A plane, and a home and a config home of the test's own.
    fn new() -> Self {
        let tmp = tempfile::tempdir().expect("a directory");
        let base = tmp.path().canonicalize().expect("a real path");
        let root = base.join("plane");
        let (home, config) = (base.join("home"), base.join("config"));
        for dir in [&root, &home, &config] {
            std::fs::create_dir_all(dir).expect("a folder");
        }
        std::fs::create_dir_all(root.join(".git")).expect("the plane's git folder");
        std::fs::write(root.join("charter.toml"), "schema = 1\n").expect("the marker");
        Self {
            _tmp: tmp,
            root,
            home,
            config,
        }
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
}

#[test]
fn a_shell_commands_write_to_the_policy_file_is_held_under_every_template() {
    purlis_core::unsteered!();
    let machine = Machine::new();
    let branch = machine.root.join(".worktrees/repo/branch");
    std::fs::create_dir_all(&branch).expect("a branch folder");
    for template in ["strict", "open"] {
        std::fs::write(
            machine.root.join("charter.toml"),
            format!("schema = 1\n[policy]\ntemplate = \"{template}\"\n"),
        )
        .expect("the project file");
        let root = machine.root.display();
        for command in [
            format!("echo x >> {root}/policy.cedar"),
            format!("sed -i '' s/a/b/ {root}/policy.cedar"),
        ] {
            let said = machine
                .denial(&branch, &branch.display().to_string(), &command)
                .unwrap_or_else(|| panic!("{template}: not refused: {command}"));
            assert!(said.contains("policy-self-change"), "{command}: {said}");
        }
        // The branch folder's own copy, and a read of the root's, run.
        for command in [
            "echo x >> policy.cedar".to_owned(),
            format!("cat {root}/policy.cedar"),
        ] {
            let said = machine.denial(&branch, &branch.display().to_string(), &command);
            assert!(
                said.as_deref()
                    .is_none_or(|said| !said.contains("policy-self-change")),
                "{template}: {command}: {said:?}"
            );
        }
    }
}
