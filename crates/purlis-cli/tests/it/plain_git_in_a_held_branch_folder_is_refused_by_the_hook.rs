// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! #3931: a chat that runs git itself is held to a branch folder's one live writer too. Its
//! `git commit` or `git push` in a folder another live chat holds is refused by the real
//! `purlis hook pretooluse`, naming the holder in the words a brokered write is refused in; the
//! holder's own git runs, a free folder is taken by the chat that writes first, and a harness
//! the app did not start (the person's own) is never asked.
//!
//! **The app here is the core's broker on a real listener** (`purlis_core::gitbroker::answer`,
//! which the app's answerer calls with its record of the chat): chat 3 asks, and chat 4 is the
//! other live chat. **An app that does not answer lets the git run** (the review of #4129):
//! this guards against accidents and is not a wall.

use crate::support;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use purlis_core::gitbroker::{self, Asker};
use purlis_core::hookwire::{
    self, Answer, Ask, CHAT_ENV, CHAT_ID_ENV, ChatToken, Listener, Reading, SOCKET_ENV, TOKEN_ENV,
};
use purlis_core::piececlaim::Live;
use purlis_core::pieces::{self, Event, Who};
use purlis_core::worktree::git;

/// Chat 3's stable id: the chat that runs the git.
const ME: &str = "01ASKING";
/// Chat 4's: the other live chat.
const HOLDER: &str = "01HOLDER";

/// How the stand-in app answers a git ask.
#[derive(Clone, Copy)]
enum App {
    /// As the core broker does.
    Broker,
    /// As the broker does, after longer than any guard waits.
    Slow,
    /// `No`, as the app answers a chat it does not have open.
    SaysNo,
}

/// A project with a clone in workspace `alpha` and two branch folders of it, `widget/p1` and
/// `widget/p2`, laid out as git lays out a linked worktree: each folder's `.git` names its
/// record in the clone's `.git/worktrees`, and the record's `gitdir` names the folder's `.git`
/// back.
struct Project {
    _tmp: tempfile::TempDir,
    base: PathBuf,
    root: PathBuf,
    home: PathBuf,
    p1: PathBuf,
    p2: PathBuf,
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
        let folder = |piece: &str| {
            let record = root
                .join("workspaces/alpha/widget/.git/worktrees")
                .join(piece);
            let folder = root.join("workspaces/alpha/.worktrees/widget").join(piece);
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
            folder
        };
        let (p1, p2) = (folder("p1"), folder("p2"));
        Self {
            _tmp: tmp,
            base,
            root,
            home,
            p1,
            p2,
        }
    }

    /// Folder `piece` claimed by chat `number`, whose stable id is `id`.
    fn claimed_by(&self, piece: &str, number: u32, id: &str) {
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
            piece,
            None,
            &who,
            chrono::Utc::now(),
        )
        .expect("the claim");
    }

    /// The stable id of the chat the piece log last names as claiming folder `piece`.
    fn last_claim(&self, piece: &str) -> Option<String> {
        pieces::events(&self.root, "alpha")
            .into_iter()
            .rev()
            .filter(|e| e["piece"] == piece && e["event"] == "claimed")
            .find_map(|e| e["chat"].as_str().map(str::to_owned))
    }

    /// An app listening for chat 3 whose answer to a git ask is the core broker's, judging
    /// claims against chats 3 and 4, both live; or one that answers [`App`] otherwise.
    fn an_app(&self, app: App) -> (Reading, ChatToken, PathBuf) {
        let socket = self.base.join("app").join("hooks.sock");
        let listener = Listener::bind(&self.base, &socket).expect("a socket");
        let token = listener.tokens().issue_to_this_process(3).expect("a token");
        let root = self.root.clone();
        let reading = listener.each_answering(
            Box::new(|_| {}),
            Box::new(move |_, ask| {
                match app {
                    App::Broker => {}
                    App::Slow => std::thread::sleep(Duration::from_secs(5)),
                    App::SaysNo => {
                        return Answer::No {
                            why: "chat 3 is not one this app has open".to_owned(),
                        };
                    }
                }
                let Ask::Git(asked) = ask else {
                    return Answer::No {
                        why: hookwire::NOTHING_ANSWERS.to_owned(),
                    };
                };
                let live = |id: &str, chat| Live {
                    id: id.to_owned(),
                    chat: Some(chat),
                    back: true,
                };
                gitbroker::answer(
                    &root,
                    &Asker {
                        chat: 3,
                        cwd: None,
                        persona: Some("steward".to_owned()),
                        harnessed: true,
                        unsandboxed: true,
                        config: None,
                        id: Some(ME.to_owned()),
                        live: Some(vec![live(ME, 3), live(HOLDER, 4)]),
                    },
                    &asked,
                    &git::Isolated::default(),
                    chrono::Utc::now(),
                )
            }),
        );
        (reading, token, socket)
    }

    /// The `permissionDecisionReason` the hook printed for `command` run in `cwd`, in a chat
    /// whose environment is `chat`, or `None` where it refused nothing.
    fn denial(&self, command: &str, cwd: &Path, chat: &[(&str, &str)]) -> Option<String> {
        let payload = serde_json::json!({
            "hook_event_name": "PreToolUse",
            "session_id": "s",
            "cwd": cwd,
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

/// Chat 3's environment, as the app starts it without the sandbox.
fn chat_3<'a>(socket: &'a Path, token: &'a ChatToken) -> [(&'a str, &'a str); 4] {
    [
        (SOCKET_ENV, socket.to_str().expect("a path")),
        (CHAT_ENV, "3"),
        (TOKEN_ENV, token.expose()),
        (CHAT_ID_ENV, ME),
    ]
}

#[test]
fn plain_git_in_a_folder_another_live_chat_holds_is_refused_naming_the_holder() {
    let p = Project::new();
    p.claimed_by("p1", 4, HOLDER);
    let (_app, token, socket) = p.an_app(App::Broker);
    for (command, act) in [
        ("git push", "push"),
        ("git commit -am 'mine too'", "commit"),
        ("git pull --rebase", "update"),
    ] {
        let why = p
            .denial(command, &p.p1, &chat_3(&socket, &token))
            .unwrap_or_else(|| panic!("{command:?} was let through"));
        assert!(
            why.contains("widget's branch folder 'p1' is claimed by chat 4"),
            "{why}"
        );
        assert!(why.contains(&format!("will not {act} in it")), "{why}");
        assert!(why.contains("purlis worktree done"), "{why}");
    }
}

/// The review of #4129: every write on the line is judged, and `env -C <folder>` places the
/// command it moves.
#[test]
fn a_second_write_on_the_line_and_one_moved_by_env_c_are_judged_too() {
    let p = Project::new();
    p.claimed_by("p1", 4, HOLDER);
    let (_app, token, socket) = p.an_app(App::Broker);
    let line = format!(
        "git -C {} commit -m free && env -C {} git push",
        p.p2.display(),
        p.p1.display()
    );
    let why = p
        .denial(&line, &p.root, &chat_3(&socket, &token))
        .expect("the push in the held folder was let through");
    assert!(
        why.contains("branch folder 'p1' is claimed by chat 4"),
        "{why}"
    );
    assert!(why.contains("will not push in it"), "{why}");
    let moved = format!("env -C {} git commit -m x", p.p1.display());
    assert!(
        p.denial(&moved, &p.root, &chat_3(&socket, &token))
            .is_some(),
        "{moved}"
    );
}

#[test]
fn the_holders_own_git_and_the_persons_own_harness_are_never_refused() {
    let p = Project::new();
    p.claimed_by("p1", 3, ME);
    p.claimed_by("p2", 4, HOLDER);
    let (_app, token, socket) = p.an_app(App::Broker);
    assert_eq!(p.denial("git push", &p.p1, &chat_3(&socket, &token)), None);
    // A harness the app did not start has no chat number: it is the person's own.
    assert_eq!(p.denial("git push", &p.p2, &[]), None);
    // Read-only git is never asked about.
    assert_eq!(
        p.denial("git status", &p.p2, &chat_3(&socket, &token)),
        None
    );
}

#[test]
fn the_first_chat_to_commit_in_a_free_folder_takes_it() {
    let p = Project::new();
    let (_app, token, socket) = p.an_app(App::Broker);
    assert_eq!(
        p.denial("git commit -m first", &p.p1, &chat_3(&socket, &token)),
        None
    );
    assert_eq!(p.last_claim("p1").as_deref(), Some(ME));
}

/// The review of #4129: no answer lets the call through, so neither a missing app nor a slow
/// one stops a chat, and a claim nobody can judge holds nothing.
#[test]
fn where_no_app_answers_in_time_the_git_runs() {
    let p = Project::new();
    p.claimed_by("p1", 4, HOLDER);
    // No app at all: the chat has a number and an id, and no socket.
    assert_eq!(
        p.denial("git push", &p.p1, &[(CHAT_ENV, "3"), (CHAT_ID_ENV, ME)]),
        None
    );
    // An app that takes longer than the guard waits.
    let (_app, token, socket) = p.an_app(App::Slow);
    assert_eq!(p.denial("git push", &p.p1, &chat_3(&socket, &token)), None);
}

/// The review of #4129: the guard refuses only where the app says another live chat holds the
/// folder. Any other `No` from the app (a chat it does not have open, a name it cannot read, a
/// claim it could not judge) lets the git run, as no answer does.
#[test]
fn an_app_that_answers_no_for_any_other_reason_lets_the_git_run() {
    let p = Project::new();
    p.claimed_by("p1", 4, HOLDER);
    let (_app, token, socket) = p.an_app(App::SaysNo);
    assert_eq!(p.denial("git push", &p.p1, &chat_3(&socket, &token)), None);
    assert_eq!(
        p.denial("git commit -m x", &p.p1, &chat_3(&socket, &token)),
        None
    );
}
