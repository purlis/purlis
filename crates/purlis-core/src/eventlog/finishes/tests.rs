// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

use super::*;
use crate::eventlog::Log;
use crate::eventlog::stops::{Answer, AskAnswered, AskGroup, Block, Qualifier, Source, Stop};

const DEVICE: &str = "01J9ZZDEVICE0000000000000";
const CHAT: &str = "01K6CHAT000000000000000004";
const OTHER: &str = "01K6CHAT000000000000000005";
const RUN: &str = "01K6RUN0000000000000000004";
const CHILD: &str = "01K6RUN0000000000000000009";

/// A force push's arguments, as the log keys them.
const PUSH: &str = "digest-of-git-push-force";
/// Another call's.
const LIST: &str = "digest-of-ls";

/// The turns of one chat, written to the log in `dir` as the recorder writes them.
struct Turns<'a> {
    log: Log,
    chat: &'a str,
}

impl<'a> Turns<'a> {
    fn in_dir(dir: &Path, chat: &'a str) -> Self {
        Self {
            log: Log::open(dir, DEVICE).unwrap(),
            chat,
        }
    }

    fn write(&mut self, parent: Option<&str>, kind: &str, body: serde_json::Value) -> &mut Self {
        let run = if parent.is_some() { CHILD } else { RUN };
        self.log
            .append(Some(self.chat), Some(run), parent, kind, body)
            .unwrap();
        self
    }

    fn prompt(&mut self) -> &mut Self {
        self.write(None, "hook.userpromptsubmit", serde_json::json!({}))
    }

    fn stop(&mut self) -> &mut Self {
        self.write(None, "hook.stop", serde_json::json!({}))
    }

    fn block(&mut self, args: &str, qualifier: Option<Qualifier>) -> &mut Self {
        self.block_by(Source::Guard, args, qualifier)
    }

    /// A block of `Bash` with `args` by `source`.
    fn block_by(&mut self, source: Source, args: &str, qualifier: Option<Qualifier>) -> &mut Self {
        let body = serde_json::to_value(Stop::Block(Block {
            tool: "Bash".to_owned(),
            source,
            qualifier,
            target: None,
            args: Some(args.to_owned()),
        }))
        .unwrap();
        self.write(None, crate::eventlog::stops::BLOCK, body)
    }

    /// A call of `Bash` with `args` that came back, or failed, under the kinds the recorder
    /// writes for them (`tool_kind`): `hook.` and the hook's own word.
    fn came_back(&mut self, args: &str, failed: bool) -> &mut Self {
        let kind = if failed {
            "hook.posttoolusefailure-blocked"
        } else {
            "hook.posttooluse-blocked"
        };
        self.write(
            None,
            kind,
            serde_json::json!({ "decision": "allow", "hook_ms": 1, "tool": "Bash", "args": args }),
        )
    }

    fn answered(&mut self, answer: Answer) -> &mut Self {
        self.answered_ask(AskGroup::Sandbox, answer)
    }

    fn answered_ask(&mut self, ask: AskGroup, answer: Answer) -> &mut Self {
        let body = serde_json::to_value(Stop::AskAnswered(AskAnswered { ask, answer })).unwrap();
        self.write(None, crate::eventlog::stops::ASK_ANSWERED, body)
    }

    fn ended(&mut self, state: &str) -> &mut Self {
        self.write(
            None,
            "run.ended",
            serde_json::json!({ "state": state, "cause": "exited", "code": 1 }),
        )
    }
}

/// How many blocks each of `chat`'s finishes counts, oldest first.
fn counted(dir: &Path, chat: &str) -> Vec<u32> {
    finishes_in(dir, chat)
        .unwrap()
        .into_iter()
        .map(|finish| finish.blocked)
        .collect()
}

/// R-178 (core), R-179: at a turn's end, the core counts the turn's blocks still unresolved, one
/// a target: a block on a target with no later successful call on that target and no Allow.
#[test]
fn a_turns_unresolved_blocks_are_counted_at_its_end() {
    // Block, then a success on the same target: resolved.
    let dir = tempfile::tempdir().unwrap();
    Turns::in_dir(dir.path(), CHAT)
        .prompt()
        .block(PUSH, None)
        .came_back(PUSH, false)
        .stop();
    assert_eq!(counted(dir.path(), CHAT), vec![0], "block then success");

    // A sandbox block, then the sandbox ask's Allow: resolved.
    let dir = tempfile::tempdir().unwrap();
    Turns::in_dir(dir.path(), CHAT)
        .prompt()
        .block_by(Source::Sandbox, PUSH, None)
        .answered(Answer::Allowed)
        .stop();
    assert_eq!(counted(dir.path(), CHAT), vec![0], "block then Allow");

    // A block alone; and one followed only by a refusal, a failed retry and another call's
    // success: each is still unresolved.
    let dir = tempfile::tempdir().unwrap();
    Turns::in_dir(dir.path(), CHAT)
        .prompt()
        .block(PUSH, None)
        .stop()
        .prompt()
        .block(PUSH, None)
        .answered(Answer::Refused)
        .came_back(PUSH, true)
        .came_back(LIST, false)
        .stop();
    assert_eq!(counted(dir.path(), CHAT), vec![1, 1], "block alone");

    // Two blocks on one target, unresolved: one. A block on another target is another.
    let dir = tempfile::tempdir().unwrap();
    Turns::in_dir(dir.path(), CHAT)
        .prompt()
        .block(PUSH, None)
        .block(PUSH, None)
        .stop()
        .prompt()
        .block(PUSH, None)
        .block(LIST, None)
        .stop();
    assert_eq!(
        counted(dir.path(), CHAT),
        vec![1, 2],
        "two blocks on one target"
    );

    // A success before the block resolves nothing; a block of an earlier turn is not this
    // turn's; and a report that is no proof (unverified) is never counted.
    let dir = tempfile::tempdir().unwrap();
    Turns::in_dir(dir.path(), CHAT)
        .prompt()
        .came_back(PUSH, false)
        .block(PUSH, None)
        .stop()
        .prompt()
        .block(LIST, Some(Qualifier::Unverified))
        .stop();
    assert_eq!(counted(dir.path(), CHAT), vec![1, 0], "order and turns");
}

/// R-241 (core): a turn that ended is a finished finish, a run of the chat's own that ended
/// failed is a failed one, and a sub-agent's stop or another chat's turn is none of this chat's.
#[test]
fn a_finished_turn_and_a_failed_run_are_each_a_finish() {
    let dir = tempfile::tempdir().unwrap();
    Turns::in_dir(dir.path(), CHAT)
        .prompt()
        .write(Some(RUN), "hook.stop", serde_json::json!({}))
        .stop()
        .prompt()
        .ended("failed");
    Turns::in_dir(dir.path(), OTHER).prompt().stop();
    let finishes = finishes_in(dir.path(), CHAT).unwrap();
    let hows: Vec<How> = finishes.iter().map(|finish| finish.how).collect();
    assert_eq!(hows, vec![How::Finished, How::Failed]);
    assert!(
        finishes[0].seq < finishes[1].seq,
        "each finish names its own event: {finishes:?}"
    );
    assert_eq!(finishes_in(dir.path(), OTHER).unwrap().len(), 1);
}

/// Q263: only a sandbox ask's Allow resolves, and only a sandbox block; an Allow of another
/// kind of ask resolves nothing, and a guard's block stands through any Allow.
#[test]
fn only_a_sandbox_asks_allow_resolves_and_only_a_sandbox_block() {
    let dir = tempfile::tempdir().unwrap();
    Turns::in_dir(dir.path(), CHAT)
        .prompt()
        .block(PUSH, None)
        .answered_ask(AskGroup::HarnessPrompt, Answer::Allowed)
        .stop()
        .prompt()
        .block(PUSH, None)
        .answered(Answer::Allowed)
        .stop()
        .prompt()
        .block_by(Source::Sandbox, PUSH, None)
        .answered_ask(AskGroup::Consent, Answer::Allowed)
        .stop()
        .prompt()
        .block(PUSH, None)
        .block_by(Source::Sandbox, LIST, None)
        .answered(Answer::Allowed)
        .stop();
    assert_eq!(counted(dir.path(), CHAT), vec![1, 1, 1, 1]);
}

/// Q263: a call comes back under every post-tool word the recorder writes (`posttooluse`,
/// `posttooluse-blocked`, ...), never under a failure's (`posttoolusefailure-*`), and the
/// harness-agnostic words are read off the recorder's own registry, not spelled here.
#[test]
fn a_call_comes_back_under_the_recorders_own_post_tool_words() {
    for (kind, left) in [
        ("hook.posttooluse", 0),
        ("hook.posttooluse-blocked", 0),
        ("hook.posttooluse-bash", 1),
        ("hook.posttoolusefailure-blocked", 1),
        ("hook.posttoolusefailure-purlis", 1),
        ("hook.PostToolUse", 1),
        ("hook.pretooluse", 1),
    ] {
        let dir = tempfile::tempdir().unwrap();
        Turns::in_dir(dir.path(), CHAT)
            .prompt()
            .block(PUSH, None)
            .write(
                None,
                kind,
                serde_json::json!({ "decision": "allow", "hook_ms": 1, "tool": "Bash", "args": PUSH }),
            )
            .stop();
        assert_eq!(counted(dir.path(), CHAT), vec![left], "{kind}");
    }
}

/// **R-300 (T1-33): a finish names its turn's unresolved blocks**, each by who blocked it and
/// what at, so its finish row offers that block's ways out. One a target, as it is counted; a
/// block that came back, or a qualified one, is none of them.
#[test]
fn a_finish_names_its_unresolved_blocks_by_source_and_target() {
    let dir = tempfile::tempdir().unwrap();
    let guarded = |args: &str| {
        serde_json::to_value(Stop::Block(Block {
            tool: "Bash".to_owned(),
            source: Source::Guard,
            qualifier: None,
            target: Some(crate::eventlog::stops::Target::guard_arm("A3b").unwrap()),
            args: Some(args.to_owned()),
        }))
        .unwrap()
    };
    let mut turns = Turns::in_dir(dir.path(), CHAT);
    turns
        .prompt()
        .write(None, crate::eventlog::stops::BLOCK, guarded(PUSH))
        // The same call blocked again: one a target.
        .write(None, crate::eventlog::stops::BLOCK, guarded(PUSH))
        // Another call blocked, then come back: resolved.
        .block(LIST, None)
        .came_back(LIST, false)
        // A report that can't be told counts toward nothing.
        .block("digest-of-cat", Some(Qualifier::CantTell))
        .stop();
    let finishes = finishes_in(dir.path(), CHAT).unwrap();
    assert_eq!(finishes.len(), 1);
    let finish = &finishes[0];
    assert_eq!(finish.blocked, 1);
    assert_eq!(
        finish.unresolved,
        vec![Unresolved {
            source: Source::Guard,
            qualifier: None,
            target: Some(crate::eventlog::stops::Target::guard_arm("A3b").unwrap()),
            // No `run.started` in this log: whose it was is unknown.
            harness: crate::eventlog::UNKNOWN.to_owned(),
        }],
    );
    assert_eq!(
        u32::try_from(finish.unresolved.len()).unwrap(),
        finish.blocked,
        "the count is the blocks it names"
    );
}
