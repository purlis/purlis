// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A guard's deny is a block** (R-316, R-101, R-102): every rule the tool gate can refuse
//! with, refused in a fixture, reaches the recorder as the hook sends it and is written as one
//! `stop.block` with source guard; and where the report comes from inside the chat's reach it
//! is tagged `unverified`.

use std::path::{Path, PathBuf};
use std::time::Instant;

use super::*;
use crate::eventlog::{Attributes, Began, Log, NONE, Recorder, RunOf, args_hash, read};
use crate::forge::{Forge, Kind};
use crate::handoffguard::Caller;
use crate::harness::Harness;
use crate::hookwire::{Decision, ToolCall};
use crate::toolgate::{self, Arm, Call, Launched, Plane};

const DEVICE: &str = "01J9ZZDEVICE0000000000000";
const PLANE: &str = "/planes/one";

fn recorder(dir: &Path) -> Recorder {
    Recorder::new(
        Log::open(dir, DEVICE).unwrap(),
        crate::eventlog::ArgsKey::open(dir).unwrap(),
    )
}

fn git(dir: &Path, args: &[&str]) {
    let mut full = vec!["-c", "user.name=t", "-c", "user.email=t@t"];
    full.extend_from_slice(args);
    let r = crate::testgit::run(dir, &full);
    assert!(r.ok(), "git {args:?} failed: {}", r.err);
}

/// One arm's denied fixture: a plane folder and the call the hook would judge in it.
struct Case {
    _dir: tempfile::TempDir,
    root: PathBuf,
    command: String,
    cwd: PathBuf,
    sandboxed: bool,
    chat_dir: Option<PathBuf>,
    agent_id: Option<&'static str>,
    permission_mode: &'static str,
}

impl Case {
    /// A plane folder with its state folder, and the call standing at its root.
    fn plane(command: impl Into<String>) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = PathBuf::from(crate::pypath::realpath(dir.path().to_str().unwrap()));
        // The vault itself is not made: a chat's sandbox refuses a folder of that name, and the
        // leak guard refuses the read by its path.
        std::fs::create_dir_all(root.join(".charter")).unwrap();
        Self {
            cwd: root.clone(),
            root,
            _dir: dir,
            command: command.into(),
            sandboxed: false,
            chat_dir: None,
            agent_id: None,
            permission_mode: "default",
        }
    }

    /// [`Case::plane`], the plane root a repository with an upstream and one unpushed commit,
    /// and a branch `feature`: what A3 and A3b judge.
    fn repository(command: &str) -> Self {
        let case = Self::plane(command);
        let up = case.root.join("up.git");
        git(
            &case.root,
            &["init", "-q", "--bare", "-b", "main", up.to_str().unwrap()],
        );
        git(&case.root, &["init", "-q", "-b", "main", "."]);
        std::fs::write(case.root.join("README"), "r\n").unwrap();
        git(&case.root, &["add", "README"]);
        git(&case.root, &["commit", "-q", "-m", "one"]);
        git(&case.root, &["branch", "feature"]);
        git(
            &case.root,
            &["remote", "add", "origin", up.to_str().unwrap()],
        );
        git(&case.root, &["push", "-q", "-u", "origin", "main"]);
        git(
            &case.root,
            &["commit", "-q", "--allow-empty", "-m", "unpushed"],
        );
        case
    }

    /// A folder below the root, made, and the call standing in it.
    fn in_folder(mut self, folder: &str) -> Self {
        let at = self.root.join(folder);
        std::fs::create_dir_all(&at).unwrap();
        self.cwd = at;
        self
    }

    fn sandboxed(mut self, chat_dir: Option<&Path>) -> Self {
        self.sandboxed = true;
        self.chat_dir = chat_dir.map(Path::to_path_buf);
        self
    }

    fn verdict(&self) -> Option<toolgate::Verdict> {
        let root = self.root.display().to_string();
        let forges = vec![
            Forge::default_of(Kind::GitHub),
            Forge::default_of(Kind::GitLab),
        ];
        let chat_dir = self.chat_dir.as_ref().map(|dir| dir.display().to_string());
        let plane = Plane {
            root: &root,
            forges: &forges,
            session_dir: "",
            launched: Launched {
                sandboxed: self.sandboxed,
                chat_dir: chat_dir.as_deref(),
            },
        };
        let cwd = self.cwd.display().to_string();
        let state = self.root.join(".charter");
        let call = Call {
            command: &self.command,
            cwd: &cwd,
            state_dir: &state,
            caller: Caller {
                agent_id: self.agent_id,
                harness: Some("claude-code"),
                permission_mode: Some(self.permission_mode),
            },
        };
        toolgate::verdict(&call, Some(&plane))
    }
}

/// **The denied fixture of each arm.** A `match` with no catch-all: an arm added to the gate's
/// list does not compile here until it has its case.
fn case(arm: Arm) -> Case {
    match arm {
        Arm::Caps => Case::plane("x".repeat(crate::guardcaps::MAX_COMMAND_BYTES + 1)),
        Arm::Leak => Case::plane("cat .charter/vaults/db.json"),
        Arm::SingleCredential => Case::plane("git clone git@github.com:o/r.git"),
        Arm::PlaneRootBranch => Case::repository("git checkout feature"),
        Arm::PlaneRootReset => Case::repository("git reset --hard origin/main"),
        Arm::RootUpdate => {
            let case = Case::plane("git rebase origin/main");
            std::fs::write(
                case.root.join(crate::names::PLANE_MANIFEST.write),
                "[plane]\n",
            )
            .unwrap();
            std::fs::create_dir_all(case.root.join(".git")).unwrap();
            let root = case.root.clone();
            case.sandboxed(Some(&root))
        }
        Arm::CloneUpdate => {
            let case = Case::plane("git -C \"$REPO\" pull").in_folder("workspaces/ide");
            let at = case.cwd.clone();
            case.sandboxed(Some(&at))
        }
        Arm::ReleaseFloor => Case {
            permission_mode: crate::floorguard::UNATTENDED_MODE,
            ..Case::plane("gh release create v1.0.0")
        },
        Arm::ForgeSubstitution => Case::plane("gh issue create --body \"$(x)\""),
        Arm::CharterSubstitution => {
            Case::plane("charter persona remember devops \"$(cat /etc/hosts)\"")
        }
        Arm::Handoff => Case::plane("charter handoff beta"),
        Arm::ConsentSpelling => Case::plane("purlis report bug --yes x"),
        Arm::RuleSpelling => {
            let case = Case::plane("/usr/local/bin/terraform apply");
            std::fs::create_dir_all(case.root.join(".claude")).unwrap();
            std::fs::write(
                case.root.join(".claude/settings.json"),
                r#"{"permissions": {"ask": ["Bash(terraform apply *)"]}}"#,
            )
            .unwrap();
            case
        }
        Arm::SubAgentDispatch => Case {
            agent_id: Some("sub-1"),
            ..Case::plane("purlis dispatch --name \"check the queue\" <<'BRIEF'\nlook\nBRIEF")
        },
        Arm::DispatchText => {
            Case::plane("purlis dispatch report --outcome done \"$(cat notes.md)\"")
        }
        Arm::HookSkip => Case::plane("git commit --no-verify -m x"),
        Arm::ProjectGit => {
            let case = Case::plane("git fetch").in_folder("workspaces/ide");
            std::fs::create_dir_all(case.root.join(".git")).unwrap();
            let at = case.cwd.clone();
            case.sandboxed(Some(&at))
        }
        Arm::BranchGit => {
            let case = Case::plane("git commit -m x");
            let record = case.root.join("workspaces/ide/purlis/.git/worktrees/port");
            let folder = case.root.join("workspaces/ide/.worktrees/purlis/port");
            std::fs::create_dir_all(&record).unwrap();
            std::fs::create_dir_all(&folder).unwrap();
            std::fs::write(
                folder.join(".git"),
                format!("gitdir: {}\n", record.display()),
            )
            .unwrap();
            std::fs::write(
                record.join("gitdir"),
                format!("{}\n", folder.join(".git").display()),
            )
            .unwrap();
            Case {
                cwd: folder,
                ..case
            }
            .sandboxed(None)
        }
        Arm::Rider => Case::plane("purlis dispatch --name x <<'B'\nlook\nB\ntouch /tmp/zz"),
    }
}

/// The line `purlis hook pretooluse` sends the host for `verdict`, as it sends it
/// (`tell_the_host_about_the_tool_call`): the tool, the hash of its input, deny, the rule and
/// the arm.
fn denied(chat: u32, command: &str, verdict: &toolgate::Verdict) -> ToolCall {
    ToolCall {
        chat,
        tool_hook: "pretooluse".to_owned(),
        tool: Some("Bash".to_owned()),
        call: Some("toolu_01".to_owned()),
        args: Some(args_hash(&serde_json::json!({ "command": command }))),
        decision: Decision::Deny,
        rule: Some(verdict.reason.clone()),
        arm: verdict.arm.map(|arm| arm.id().to_owned()),
        hook_ms: 3,
        agent: None,
        at_ms: 0,
        skill: None,
    }
}

/// The blocks written since `from` events.
fn blocks_since(dir: &Path, from: usize) -> Vec<crate::eventlog::Event> {
    read(dir)
        .unwrap()
        .into_iter()
        .skip(from)
        .filter(|event| event.kind == BLOCK)
        .collect()
}

/// The keys a block's body may hold: its own fields, and the skill run every event inside a
/// run carries.
const BLOCK_KEYS: [&str; 7] = [
    "tool",
    "source",
    "qualifier",
    "target",
    "args",
    "skill",
    "skill_run",
];

/// R-316, S2: a deny by each rule in the gate's own list ([`Arm::ALL`]) gives exactly one
/// block event with source guard, the arm as its target and the arguments' keyed digest only.
#[test]
fn every_rule_the_tool_gate_can_deny_with_writes_one_block_event_with_source_guard() {
    let dir = tempfile::tempdir().unwrap();
    let mut host = recorder(dir.path());
    let key = crate::eventlog::ArgsKey::open(dir.path()).unwrap();
    for &arm in Arm::ALL {
        let case = case(arm);
        let verdict = case
            .verdict()
            .unwrap_or_else(|| panic!("{arm:?}: its fixture is not refused"));
        assert_eq!(
            verdict.arm,
            Some(arm),
            "{arm:?}'s fixture is refused by another arm"
        );
        let call = denied(4, &case.command, &verdict);

        let from = read(dir.path()).unwrap().len();
        host.tool(Path::new(PLANE), &call, Instant::now()).unwrap();

        let blocks = blocks_since(dir.path(), from);
        assert_eq!(blocks.len(), 1, "{arm:?}: one block per deny");
        let body = &blocks[0].body;
        assert_eq!(body["source"], "guard", "{arm:?}");
        assert_eq!(body["tool"], "Bash", "{arm:?}");
        assert_eq!(
            body["target"],
            serde_json::json!({ "guard_arm": arm.id() }),
            "{arm:?}"
        );
        let sent = call.args.as_deref().unwrap();
        assert_eq!(body["args"], key.digest(sent), "{arm:?}: the keyed digest");
        assert_ne!(body["args"], sent, "{arm:?}: never the hook's own hash");
        for (field, _) in body.as_object().unwrap() {
            assert!(
                BLOCK_KEYS.contains(&field.as_str()),
                "{arm:?}: `{field}` in {body}"
            );
        }
        assert!(
            !body.to_string().contains(&verdict.denial),
            "{arm:?}: the denial's text is not written"
        );
    }
}

/// A chat begun on `harness`, sandboxed or at No sandbox, as the host begins one.
fn begun(host: &mut Recorder, number: u32, harness: Harness, sandboxed: bool) {
    begun_under(
        host,
        number,
        harness,
        if sandboxed { "seatbelt" } else { NONE },
    );
}

/// A chat begun on `harness` under the sandbox backend the host records as `sandbox`.
fn begun_under(host: &mut Recorder, number: u32, harness: Harness, sandbox: &str) {
    let attributes = Attributes {
        harness: Some(harness.name().to_owned()),
        sandbox: Some(sandbox.to_owned()),
        ..Attributes::default()
    };
    let chat = crate::reopen::mint();
    let run = crate::reopen::mint();
    host.begin(
        Path::new(PLANE),
        number,
        RunOf {
            chat: &chat,
            run: &run,
        },
        Began::Start,
        &attributes,
    )
    .unwrap();
}

/// The qualifier of the block a leak guard deny in chat `number` is written with.
fn qualifier_of_a_deny(dir: &Path, host: &mut Recorder, number: u32) -> serde_json::Value {
    let case = case(Arm::Leak);
    let verdict = case.verdict().unwrap();
    let from = read(dir).unwrap().len();
    host.tool(
        Path::new(PLANE),
        &denied(number, &case.command, &verdict),
        Instant::now(),
    )
    .unwrap();
    let blocks = blocks_since(dir, from);
    assert_eq!(blocks.len(), 1);
    blocks[0].body["qualifier"].clone()
}

/// R-101, Q235: a guard's report is verified only where its hook runs outside the chat's
/// sandbox: a Claude Code chat (whose sandbox does not hold what it starts) under a backend
/// that leaves its hooks outside, Seatbelt or bubblewrap. Codex and opencode run theirs inside
/// the wrap, a Claude Code chat purlis wraps whole (`landlock`, #4083) runs its hooks inside
/// it too, and a backend word nobody named is not trusted.
#[test]
fn a_guard_report_is_verified_only_from_claude_code_under_a_backend_that_leaves_its_hooks_outside()
{
    let dir = tempfile::tempdir().unwrap();
    let mut host = recorder(dir.path());
    let unverified = serde_json::json!("unverified");
    let verified = serde_json::Value::Null;
    let mut number = 0;
    for (harness, sandbox, said) in [
        (Harness::Codex, "seatbelt", &unverified),
        (Harness::Codex, "bubblewrap", &unverified),
        (Harness::Codex, "landlock", &unverified),
        (Harness::Opencode, "seatbelt", &unverified),
        (Harness::Opencode, "landlock", &unverified),
        (Harness::ClaudeCode, "seatbelt", &verified),
        (Harness::ClaudeCode, "bubblewrap", &verified),
        (Harness::ClaudeCode, "landlock", &unverified),
        (Harness::ClaudeCode, "firejail", &unverified),
        (Harness::ClaudeCode, "Seatbelt", &unverified),
    ] {
        number += 1;
        begun_under(&mut host, number, harness, sandbox);
        assert_eq!(
            &qualifier_of_a_deny(dir.path(), &mut host, number),
            said,
            "{harness:?} under {sandbox}"
        );
    }
}

/// R-102: at No sandbox the chat can reach whatever reports for it, so on every harness the
/// guard's report is tagged `unverified`; so is a chat the host cannot say is sandboxed.
#[test]
fn a_guard_report_from_a_no_sandbox_chat_is_tagged_unverified() {
    let dir = tempfile::tempdir().unwrap();
    let mut host = recorder(dir.path());
    for (number, harness) in (1..).zip(Harness::ALL) {
        begun(&mut host, number, harness, false);
        assert_eq!(
            qualifier_of_a_deny(dir.path(), &mut host, number),
            "unverified",
            "{harness:?} at No sandbox"
        );
    }
    // A chat the host was never told of.
    assert_eq!(qualifier_of_a_deny(dir.path(), &mut host, 9), "unverified");
}

/// A deny line no gate arm gave (`arm: None`): A11, a file tool's guard, a guard that crashed
/// or did not answer.
fn denied_by_no_arm(chat: u32) -> ToolCall {
    ToolCall {
        arm: None,
        rule: Some("guard-crashed".to_owned()),
        ..denied(chat, "ls", &case(Arm::Leak).verdict().unwrap())
    }
}

/// R-316: a deny that names no arm of the gate is still one block with source guard, and no
/// target, since nothing names which rule refused.
#[test]
fn a_deny_with_no_arm_writes_one_block_with_no_target() {
    let dir = tempfile::tempdir().unwrap();
    let mut host = recorder(dir.path());
    begun(&mut host, 4, Harness::ClaudeCode, true);
    host.tool(Path::new(PLANE), &denied_by_no_arm(4), Instant::now())
        .unwrap();
    let blocks = blocks_since(dir.path(), 0);
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].body["source"], "guard");
    assert!(blocks[0].body.get("target").is_none(), "{}", blocks[0].body);
}

/// R-316: a line that is not a deny (allowed, asked, or said nothing) writes no block.
#[test]
fn a_tool_line_that_is_not_a_deny_writes_no_block() {
    let dir = tempfile::tempdir().unwrap();
    let mut host = recorder(dir.path());
    begun(&mut host, 4, Harness::ClaudeCode, true);
    for decision in [Decision::Allow, Decision::Ask, Decision::None] {
        let call = ToolCall {
            decision,
            arm: Some(Arm::Leak.id().to_owned()),
            ..denied_by_no_arm(4)
        };
        host.tool(Path::new(PLANE), &call, Instant::now()).unwrap();
    }
    assert_eq!(blocks_since(dir.path(), 0), Vec::new());
}

/// R-316: a deny from a chat whose start was taken back is of no run, and its block is too,
/// under the chat's number.
#[test]
fn a_deny_from_a_chat_of_no_run_writes_one_block_under_its_number() {
    let dir = tempfile::tempdir().unwrap();
    let mut host = recorder(dir.path());
    let (chat, run) = (crate::reopen::mint(), crate::reopen::mint());
    host.begin_held(
        Path::new(PLANE),
        6,
        RunOf {
            chat: &chat,
            run: &run,
        },
        Began::Start,
        &Attributes::default(),
        None,
    );
    host.taken_back(Path::new(PLANE), 6);
    host.tool(Path::new(PLANE), &denied_by_no_arm(6), Instant::now())
        .unwrap();
    let blocks = blocks_since(dir.path(), 0);
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].body["chat_number"], 6);
    assert_eq!(blocks[0].chat, None);
    assert_eq!(blocks[0].body["qualifier"], "unverified");
}

/// R-316, FD-30: a deny drained from the spool of a chat the recorder does not know (one that
/// had closed) is written as one block, of no run, under its number and its spool number.
#[cfg(unix)]
#[test]
fn a_deny_drained_for_a_chat_not_known_writes_one_block_under_its_number() {
    use crate::hookwire::spool::{Drained, Spooled};
    let dir = tempfile::tempdir().unwrap();
    let mut host = recorder(dir.path());
    host.spooled(
        Path::new(PLANE),
        Drained::Line {
            chat: 8,
            seq: 3,
            line: Spooled::Tool(denied_by_no_arm(8)),
        },
    )
    .unwrap();
    let blocks = blocks_since(dir.path(), 0);
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].body["chat_number"], 8);
    assert_eq!(blocks[0].body["spooled"], 3);
    assert_eq!(blocks[0].chat, None);
}

/// R-316: the arm comes from the chat's side of the channel, so a word that is no arm's id is
/// never written: not as the target, and not anywhere in the log.
#[test]
fn a_spoofed_arm_on_the_line_writes_no_target_and_is_not_on_disk() {
    let dir = tempfile::tempdir().unwrap();
    let mut host = recorder(dir.path());
    begun(&mut host, 4, Harness::ClaudeCode, true);
    let long = "Z".repeat(200);
    for spoofed in ["rm -rf /", "a2", long.as_str()] {
        let from = read(dir.path()).unwrap().len();
        let call = ToolCall {
            arm: Some(spoofed.to_owned()),
            ..denied_by_no_arm(4)
        };
        host.tool(Path::new(PLANE), &call, Instant::now()).unwrap();
        let blocks = blocks_since(dir.path(), from);
        assert_eq!(blocks.len(), 1, "{spoofed}");
        assert!(blocks[0].body.get("target").is_none(), "{spoofed}");
    }
    let mut on_disk = String::new();
    for file in walk(dir.path()) {
        on_disk.push_str(&String::from_utf8_lossy(&std::fs::read(file).unwrap()));
    }
    for spoofed in ["rm -rf /", "\"a2\"", long.as_str()] {
        assert!(!on_disk.contains(spoofed), "`{spoofed}` is on disk");
    }
}

/// Every file below `dir`.
fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files.extend(walk(&path));
        } else {
            files.push(path);
        }
    }
    files
}

/// The block is as durable as its tool event: the writer makes durable the last event an
/// entry answers, so a deny's entry answers its tool event and then its block.
#[test]
fn a_deny_entry_answers_its_block_last_so_the_writer_makes_it_durable() {
    let dir = tempfile::tempdir().unwrap();
    let mut host = recorder(dir.path());
    begun(&mut host, 4, Harness::ClaudeCode, true);
    let written = host
        .apply(crate::eventlog::Entry::Tool {
            plane: PathBuf::from(PLANE),
            call: denied_by_no_arm(4),
        })
        .unwrap();
    let kinds: Vec<&str> = written.iter().map(|event| event.kind.as_str()).collect();
    assert_eq!(kinds, ["hook.pretooluse", BLOCK]);
}

/// The same for a drained deny: the drain makes durable the event `spooled` answers, which is
/// its block, whether the chat is known or not.
#[cfg(unix)]
#[test]
fn a_drained_deny_answers_its_block_so_the_drain_makes_it_durable() {
    use crate::hookwire::spool::{Drained, Spooled};
    let dir = tempfile::tempdir().unwrap();
    let mut host = recorder(dir.path());
    begun(&mut host, 4, Harness::ClaudeCode, true);
    for chat in [4, 9] {
        let answered = host
            .spooled(
                Path::new(PLANE),
                Drained::Line {
                    chat,
                    seq: 2,
                    line: Spooled::Tool(denied_by_no_arm(chat)),
                },
            )
            .unwrap();
        assert_eq!(answered.kind, BLOCK, "chat {chat}");
        assert_eq!(answered.body["spooled"], 2, "chat {chat}");
    }
}

/// R-114: a merge the hook held is a hold, which the app writes once it has raised the ask, so
/// its deny line writes no block; a merge whose request cannot be read raises no ask, and is a
/// block with source guard as any refusal.
#[test]
fn a_held_merge_is_no_block_and_a_merge_nobody_was_asked_about_is_one() {
    let dir = tempfile::tempdir().unwrap();
    let mut host = recorder(dir.path());
    begun(&mut host, 4, Harness::ClaudeCode, true);
    let held = ToolCall {
        rule: Some(crate::mergehold::REASON.to_owned()),
        ..denied_by_no_arm(4)
    };
    host.tool(Path::new(PLANE), &held, Instant::now()).unwrap();
    assert_eq!(blocks_since(dir.path(), 0), Vec::new());

    let unread = ToolCall {
        rule: Some(crate::mergehold::REASON_UNREADABLE.to_owned()),
        ..denied_by_no_arm(4)
    };
    host.tool(Path::new(PLANE), &unread, Instant::now())
        .unwrap();
    let blocks = blocks_since(dir.path(), 0);
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].body["source"], "guard");
}
