// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;

use super::*;
use crate::eventlog::Log;
use crate::eventlog::stops::{
    Answer, AnsweredBy, AskAnswered, AskGroup, Block, Qualifier, Source, Stop,
};

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
        let body = serde_json::to_value(Stop::AskAnswered(AskAnswered {
            ask,
            answer,
            by: AnsweredBy::Window,
        }))
        .unwrap();
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
    // turn's; and a report that is no proof (unverified) is counted, as the marker counts it
    // (Q280).
    let dir = tempfile::tempdir().unwrap();
    Turns::in_dir(dir.path(), CHAT)
        .prompt()
        .came_back(PUSH, false)
        .block(PUSH, None)
        .stop()
        .prompt()
        .block(LIST, Some(Qualifier::Unverified))
        .stop();
    assert_eq!(counted(dir.path(), CHAT), vec![1, 1], "order and turns");
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
/// block that came back is none of them.
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

/// **Q280 (R-300, R-192, Q272): a harness-check block and a Can't-tell block reach the finish
/// row**, each with its qualifier and the chat's harness, so the ways out the row offers are
/// theirs (Show this check names the harness; Go to chat), and the finish counts what the
/// Blocked marker's tally counts of the same turn. Mutation: the finish keeping only blocks
/// `crate::looping::counts` takes (the turn reads 0 while the marker reads 2), or naming no
/// qualifier or no harness.
#[test]
fn a_harness_check_and_a_cant_tell_block_reach_the_finish_row_as_the_marker_counts_them() {
    let dir = tempfile::tempdir().unwrap();
    let mut turns = Turns::in_dir(dir.path(), CHAT);
    turns
        .write(
            None,
            "run.started",
            serde_json::json!({ "harness": "codex" }),
        )
        .prompt();
    // As the recorder writes the harness's own refusal (`stops::block_of_line`): no target.
    turns.block_by(Source::HarnessCheck, PUSH, None);
    turns.block_by(Source::Guard, LIST, Some(Qualifier::CantTell));
    turns.stop();

    let finishes = finishes_in(dir.path(), CHAT).unwrap();
    assert_eq!(finishes.len(), 1);
    let finish = &finishes[0];
    assert_eq!(
        finish.unresolved,
        vec![
            Unresolved {
                source: Source::HarnessCheck,
                qualifier: None,
                target: None,
                harness: "codex".to_owned(),
            },
            Unresolved {
                source: Source::Guard,
                qualifier: Some(Qualifier::CantTell),
                target: None,
                harness: "codex".to_owned(),
            },
        ],
    );

    // The marker's tally of the same log, as the window reads it.
    let mut marker = crate::eventlog::chatlog::Blocks::default();
    for delivery in crate::eventlog::subscribe(dir.path(), 0).poll().unwrap() {
        marker.take(delivery);
    }
    let tally = marker.since(CHAT, None).tally;
    assert_eq!(
        tally.blocks, 1,
        "the harness's own check is a plain block: {tally:?}"
    );
    assert_eq!(tally.cant_tell_blocks, 1, "{tally:?}");
    assert_eq!(
        finish.blocked,
        tally.blocks + tally.unverified_blocks + tally.cant_tell_blocks,
        "the finish row and the marker agree on the count"
    );
}

/// Q263, Q280: **one function decides what an Allow resolves**, for the block loop and the
/// finish row alike: a sandbox ask's Allow, and a sandbox block. Mutation: `&&` for `||` in
/// `looping::allow_resolves`, or it answering `true` or `false` whatever it is asked.
#[test]
fn one_function_decides_what_an_allow_resolves_for_the_loop_and_the_finish_row() {
    use crate::looping::allow_resolves;
    for ask in [
        AskGroup::Sandbox,
        AskGroup::HarnessPrompt,
        AskGroup::Consent,
        AskGroup::Hold,
        AskGroup::LoopPause,
        AskGroup::Budget,
        AskGroup::HookGiveUp,
    ] {
        for source in [
            Source::Guard,
            Source::Sandbox,
            Source::Policy,
            Source::ProjectSetting,
            Source::HarnessCheck,
        ] {
            assert_eq!(
                allow_resolves(ask, source),
                ask == AskGroup::Sandbox && source == Source::Sandbox,
                "{ask:?} {source:?}"
            );
        }
    }
}

// Felt speed round 5 (H15): the window asks for finishes as every chat's turn ends, and each ask
// read and parsed the whole log (30 days, 125 MB on a busy machine).

/// Seals a segment after about two events.
const SMALL: crate::eventlog::Retention = crate::eventlog::Retention {
    segment_bytes: 300,
    keep: crate::eventlog::Retention::DEFAULT.keep,
    hold: crate::eventlog::Retention::DEFAULT.hold,
};

/// `turns` turns of `chat`, in a log of small segments.
fn sealed_turns(dir: &Path, chat: &str, turns: usize) {
    let mut written = Turns {
        log: Log::open_with(dir, DEVICE, SMALL).unwrap(),
        chat,
    };
    for _ in 0..turns {
        written.prompt().block(PUSH, None).stop();
    }
}

fn sealed_files(dir: &Path) -> Vec<std::path::PathBuf> {
    super::super::segments(dir)
        .unwrap()
        .into_iter()
        .map(|(_, path)| path)
        .collect()
}

fn aged(path: &Path, days: u64) {
    let then = std::time::SystemTime::now() - std::time::Duration::from_secs(days * 86_400);
    std::fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(then)
        .unwrap();
}

#[test]
fn recent_finishes_are_what_the_whole_log_says_across_sealed_segments() {
    let dir = tempfile::tempdir().unwrap();
    sealed_turns(dir.path(), CHAT, 4);
    sealed_turns(dir.path(), OTHER, 3);
    assert!(sealed_files(dir.path()).len() >= 3, "the log is sealed");
    let recent = Recent::default();
    let said = recent.finishes(dir.path(), &[CHAT, OTHER], 0).unwrap();
    for chat in [CHAT, OTHER] {
        assert_eq!(said[chat], finishes_in(dir.path(), chat).unwrap(), "{chat}");
    }
    assert_eq!(said[CHAT].len(), 4);
}

#[test]
fn a_log_that_has_not_changed_is_not_parsed_again() {
    let dir = tempfile::tempdir().unwrap();
    sealed_turns(dir.path(), CHAT, 4);
    let recent = Recent::default();
    recent.finishes(dir.path(), &[CHAT], 0).unwrap();
    let after_first = recent.parsed();
    assert!(
        after_first >= 3,
        "every file was parsed once: {after_first}"
    );
    for _ in 0..5 {
        recent.finishes(dir.path(), &[CHAT], 0).unwrap();
    }
    assert_eq!(recent.parsed(), after_first);
}

#[test]
fn a_finish_written_after_an_answer_is_in_the_next_one() {
    let dir = tempfile::tempdir().unwrap();
    sealed_turns(dir.path(), CHAT, 2);
    let recent = Recent::default();
    let before = recent.finishes(dir.path(), &[CHAT], 0).unwrap()[CHAT].len();
    sealed_turns(dir.path(), CHAT, 1);
    let after = recent.finishes(dir.path(), &[CHAT], 0).unwrap()[CHAT].len();
    assert_eq!((before, after), (2, 3));
}

impl Turns<'_> {
    /// The chat's run starting, which names its harness.
    fn started(&mut self, harness: &str) -> &mut Self {
        self.write(
            None,
            "run.started",
            serde_json::json!({ "harness": harness }),
        )
    }
}

/// `chat`'s events, in a log of small segments.
fn small<'a>(dir: &Path, chat: &'a str) -> Turns<'a> {
    Turns {
        log: Log::open_with(dir, DEVICE, SMALL).unwrap(),
        chat,
    }
}

/// Every sealed segment written ten days ago: the live file and what is sealed later are the
/// window's.
fn age_sealed(dir: &Path) {
    for path in sealed_files(dir) {
        aged(&path, 10);
    }
}

fn a_day_ago() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        - 86_400
}

/// What the whole log says of `chat`, and what [`Recent`] says of it for a window of the last day.
fn whole_and_recent(dir: &Path, chat: &str) -> (Vec<Finish>, Vec<Finish>) {
    let said = Recent::default()
        .finishes(dir, &[chat], a_day_ago())
        .unwrap();
    (finishes_in(dir, chat).unwrap(), said[chat].clone())
}

#[test]
fn a_turn_that_started_before_the_window_keeps_its_early_blocks() {
    let dir = tempfile::tempdir().unwrap();
    small(dir.path(), CHAT)
        .started("claude")
        .prompt()
        .block(PUSH, None);
    sealed_turns(dir.path(), OTHER, 6);
    age_sealed(dir.path());
    small(dir.path(), CHAT).stop();
    let (whole, recent) = whole_and_recent(dir.path(), CHAT);
    assert_eq!(whole.len(), 1);
    assert_eq!(whole[0].blocked, 1, "the premise: the whole log counts it");
    assert_eq!(recent, whole);
}

#[test]
fn a_blocks_harness_is_the_one_its_run_started_with_before_the_window() {
    let dir = tempfile::tempdir().unwrap();
    small(dir.path(), CHAT).started("codex");
    sealed_turns(dir.path(), OTHER, 6);
    age_sealed(dir.path());
    small(dir.path(), CHAT).prompt().block(PUSH, None).stop();
    let (whole, recent) = whole_and_recent(dir.path(), CHAT);
    assert_eq!(whole[0].unresolved[0].harness, "codex", "the premise");
    assert_eq!(recent, whole);
}

#[test]
fn a_window_that_needs_no_older_file_does_not_read_one() {
    let dir = tempfile::tempdir().unwrap();
    sealed_turns(dir.path(), CHAT, 6);
    sealed_turns(dir.path(), OTHER, 2);
    age_sealed(dir.path());
    small(dir.path(), CHAT).started("claude").prompt().stop();
    let files = sealed_files(dir.path()).len() + 1;
    let recent = Recent::default();
    let said = recent.finishes(dir.path(), &[CHAT], a_day_ago()).unwrap();
    let whole = finishes_in(dir.path(), CHAT).unwrap();
    assert_eq!(said[CHAT], whole[whole.len() - 1..], "the window's finish");
    assert!(
        recent.parsed() < files,
        "{} of {files} files parsed",
        recent.parsed()
    );
}

#[test]
fn a_file_that_is_gone_from_the_log_is_not_kept() {
    let dir = tempfile::tempdir().unwrap();
    sealed_turns(dir.path(), CHAT, 6);
    let recent = Recent::default();
    recent.finishes(dir.path(), &[CHAT], 0).unwrap();
    let files = sealed_files(dir.path());
    std::fs::remove_file(&files[0]).unwrap();
    let said = recent.finishes(dir.path(), &[CHAT], 0).unwrap();
    assert_eq!(said[CHAT], finishes_in(dir.path(), CHAT).unwrap());
    let kept = recent.files.lock().unwrap();
    assert!(!kept.contains_key(&files[0]), "the pruned segment is kept");
    assert_eq!(kept.len(), files.len(), "the segments left and the live file");
}

fn live(dir: &Path) -> std::path::PathBuf {
    dir.join(crate::eventlog::FILE)
}

#[test]
fn only_what_a_live_file_gained_is_parsed_again() {
    let dir = tempfile::tempdir().unwrap();
    let mut written = Turns::in_dir(dir.path(), CHAT);
    written.started("claude");
    for _ in 0..20 {
        written.prompt().block(PUSH, None).stop();
    }
    let recent = Recent::default();
    recent.finishes(dir.path(), &[CHAT], 0).unwrap();
    let before = recent.lines();
    assert_eq!(before, 61);
    written.prompt().block(PUSH, None).stop();
    let said = recent.finishes(dir.path(), &[CHAT], 0).unwrap();
    assert_eq!(said[CHAT], finishes_in(dir.path(), CHAT).unwrap());
    assert_eq!(said[CHAT].len(), 21);
    assert_eq!(recent.lines() - before, 3, "the three lines it gained");
}

#[test]
fn a_line_half_written_is_read_once_it_is_whole() {
    let dir = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    Turns::in_dir(elsewhere.path(), CHAT).stop();
    let line = std::fs::read(live(elsewhere.path())).unwrap();
    Turns::in_dir(dir.path(), CHAT)
        .started("claude")
        .prompt()
        .block(PUSH, None);
    let append = |bytes: &[u8]| {
        use std::io::Write;
        std::fs::OpenOptions::new()
            .append(true)
            .open(live(dir.path()))
            .unwrap()
            .write_all(bytes)
            .unwrap();
    };
    let recent = Recent::default();
    append(&line[..line.len() / 2]);
    assert!(recent.finishes(dir.path(), &[CHAT], 0).unwrap()[CHAT].is_empty());
    append(&line[line.len() / 2..]);
    let said = recent.finishes(dir.path(), &[CHAT], 0).unwrap();
    assert_eq!(said[CHAT], finishes_in(dir.path(), CHAT).unwrap());
    assert_eq!(said[CHAT].len(), 1);
}

#[test]
fn a_live_file_replaced_by_another_of_the_same_length_and_time_is_read_again() {
    let dir = tempfile::tempdir().unwrap();
    Turns::in_dir(dir.path(), CHAT).prompt().block(PUSH, None).stop();
    let recent = Recent::default();
    assert_eq!(recent.finishes(dir.path(), &[CHAT], 0).unwrap()[CHAT].len(), 1);
    let path = live(dir.path());
    let modified = std::fs::metadata(&path).unwrap().modified().unwrap();
    // The same bytes but for the chat: as long, and no longer anyone's turn of CHAT.
    let text = std::fs::read_to_string(&path).unwrap().replace(CHAT, OTHER);
    let swap = dir.path().join("swap");
    std::fs::write(&swap, text).unwrap();
    std::fs::File::options()
        .write(true)
        .open(&swap)
        .unwrap()
        .set_modified(modified)
        .unwrap();
    std::fs::rename(&swap, &path).unwrap();
    let said = recent.finishes(dir.path(), &[CHAT], 0).unwrap();
    assert_eq!(said[CHAT], finishes_in(dir.path(), CHAT).unwrap());
    assert!(said[CHAT].is_empty());
}

#[test]
fn a_live_file_that_shrank_is_read_whole_again() {
    let dir = tempfile::tempdir().unwrap();
    let mut written = Turns::in_dir(dir.path(), CHAT);
    written.prompt().block(PUSH, None).stop().prompt().stop();
    let recent = Recent::default();
    assert_eq!(recent.finishes(dir.path(), &[CHAT], 0).unwrap()[CHAT].len(), 2);
    let path = live(dir.path());
    let text = std::fs::read(&path).unwrap();
    let cut = text.iter().rposition(|b| *b == b'\n').unwrap();
    let cut = text[..cut].iter().rposition(|b| *b == b'\n').unwrap() + 1;
    std::fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_len(cut as u64)
        .unwrap();
    let said = recent.finishes(dir.path(), &[CHAT], 0).unwrap();
    assert_eq!(said[CHAT], finishes_in(dir.path(), CHAT).unwrap());
}
