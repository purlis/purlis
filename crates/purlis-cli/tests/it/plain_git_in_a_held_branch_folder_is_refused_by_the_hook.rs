// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! #3931: a chat that runs git itself is held to a branch folder's one live writer too. Its
//! `git commit` or `git push` in a folder another chat holds is refused by the real
//! `purlis hook pretooluse`, naming the holder in the words a brokered write is refused in; the
//! holder's own git runs, a free folder is taken by the chat that writes first, and a harness
//! the app did not start (the person's own) is never asked.
//!
//! No app answers here, so the piece log alone decides, as `purlis worktree push` does with no
//! app: a folder another chat holds is refused, since whether that chat still works cannot be
//! told. The app's own answer, judged against the chats it has, is held at the core seam
//! (`gitbroker::tests::plain_git_s_claim_is_taken_by_the_first_live_chat_and_refused_to_a_second`).

use crate::support;

use std::io::Write;
use std::path::PathBuf;
use std::process::Stdio;

use purlis_core::pieces::{self, Event, Who};

/// A project with a clone in workspace `alpha` and one branch folder of it, `widget/p1`, laid
/// out as git lays out a linked worktree: the folder's `.git` names its record in the clone's
/// `.git/worktrees`, and the record's `gitdir` names the folder's `.git` back.
struct Project {
    _tmp: tempfile::TempDir,
    root: PathBuf,
    home: PathBuf,
    folder: PathBuf,
}

impl Project {
    fn new() -> Self {
        let tmp = tempfile::tempdir().expect("a directory");
        let base = tmp.path().canonicalize().expect("a real path");
        let root = base.join("plane");
        let home = base.join("home");
        std::fs::create_dir_all(&home).expect("the home");
        std::fs::create_dir_all(&root).expect("the plane");
        std::fs::write(root.join("charter.toml"), "schema = 1\n").expect("the marker");
        let record = root.join("workspaces/alpha/widget/.git/worktrees/p1");
        let folder = root.join("workspaces/alpha/.worktrees/widget/p1");
        std::fs::create_dir_all(&record).expect("the record");
        std::fs::create_dir_all(folder.join("src")).expect("the folder");
        std::fs::write(
            folder.join(".git"),
            format!("gitdir: {}\n", record.display()),
        )
        .expect("the folder's .git");
        std::fs::write(
            record.join("gitdir"),
            format!("{}\n", folder.join(".git").display()),
        )
        .expect("the record's gitdir");
        Self {
            _tmp: tmp,
            root,
            home,
            folder,
        }
    }

    /// The folder claimed by chat `number`, whose stable id is `id`.
    fn claimed_by(&self, number: u32, id: &str) {
        let who = Who {
            session: Some(number.to_string()),
            persona: Some("builder".to_owned()),
            chat: Some(id.to_owned()),
            host: "box".to_owned(),
            log: "box".to_owned(),
        };
        pieces::record(
            &self.root,
            "alpha",
            Event::Claimed,
            "widget",
            "p1",
            None,
            &who,
            chrono::Utc::now(),
        )
        .expect("the claim");
    }

    /// The `permissionDecisionReason` the hook printed for `command` run in the branch folder,
    /// in a chat whose environment is `chat`, or `None` where it refused nothing.
    fn denial(&self, command: &str, chat: &[(&str, &str)]) -> Option<String> {
        let payload = serde_json::json!({
            "hook_event_name": "PreToolUse",
            "session_id": "s",
            "cwd": self.folder,
            "tool_name": "Bash",
            "permission_mode": "default",
            "tool_input": {"command": command},
        })
        .to_string();
        let mut child = support::purlis()
            .args(["hook", "pretooluse"])
            .current_dir(&self.root)
            .env_clear()
            .env("CHARTER_ROOT", &self.root)
            .env("HOME", &self.home)
            .env("CHARTER_CONFIG_HOME", self.home.join(".config"))
            .env("CHARTER_HARNESS", "claude-code")
            .env("PATH", "/usr/bin:/bin")
            // The verdict is what is held here, not the clock (see `hook.rs`).
            .env("PURLIS_TEST_GUARD_DEADLINE_MS", "60000")
            .envs(chat.iter().copied())
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
        assert_eq!(
            out.status.code(),
            Some(0),
            "{command}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let v: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
        let o = v.get("hookSpecificOutput")?;
        (o.get("permissionDecision")?.as_str()? == "deny").then(|| {
            o.get("permissionDecisionReason")?
                .as_str()
                .map(str::to_owned)
        })?
    }
}

/// An unsandboxed chat the app started: its number and its stable id, and no sandbox.
fn chat<'a>(number: &'a str, id: &'a str) -> [(&'a str, &'a str); 2] {
    [
        (purlis_core::hookwire::CHAT_ENV, number),
        (purlis_core::hookwire::CHAT_ID_ENV, id),
    ]
}

#[test]
fn plain_git_in_a_folder_another_chat_holds_is_refused_naming_the_holder() {
    let p = Project::new();
    p.claimed_by(4, "01HOLDER");
    for (command, act) in [
        ("git push", "push"),
        ("git commit -am 'mine too'", "commit"),
        ("git pull --rebase", "update"),
    ] {
        let why = p
            .denial(command, &chat("9", "01SECOND"))
            .unwrap_or_else(|| panic!("{command:?} was let through"));
        assert!(
            why.contains("widget's branch folder 'p1' is claimed by chat 4"),
            "{why}"
        );
        assert!(why.contains(&format!("will not {act} in it")), "{why}");
        assert!(why.contains("purlis worktree done"), "{why}");
    }
}

#[test]
fn the_holders_own_git_and_the_persons_own_harness_are_never_refused() {
    let p = Project::new();
    p.claimed_by(4, "01HOLDER");
    assert_eq!(p.denial("git push", &chat("4", "01HOLDER")), None);
    // A harness the app did not start has no chat number: it is the person's own.
    assert_eq!(p.denial("git push", &[]), None);
    // Read-only git is never asked about.
    assert_eq!(p.denial("git status", &chat("9", "01SECOND")), None);
}

#[test]
fn the_first_chat_to_commit_in_a_free_folder_takes_it_and_a_second_is_refused() {
    let p = Project::new();
    assert_eq!(p.denial("git commit -m first", &chat("4", "01FIRST")), None);
    let why = p
        .denial("git push", &chat("9", "01SECOND"))
        .expect("the second chat's push was let through");
    assert!(why.contains("claimed by chat 4"), "{why}");
}
