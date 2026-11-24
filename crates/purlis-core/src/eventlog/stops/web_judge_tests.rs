// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **One refused `WebFetch` counts once, never twice and never not at all** (#7465): at the full
//! sandbox the `pretooluse-web` hook asks the chat's proxy for the host. The proxy may write a
//! sandbox block; the hook's own deny, which the host writes as a guard's block aimed at the host,
//! is that same refusal heard twice only when such a block is recorded beside it
//! ([`crate::eventlog::web_repeats`]). Both stay in the log; the Blocked count, the Blocked log,
//! the Chat log's rows, a turn's finish and the marker's running tally hold the refusal once.

use std::path::Path;
use std::time::Instant;

use super::*;
use crate::eventlog::chatlog::{Blocks, Row};
use crate::eventlog::{
    Attributes, Began, Delivery, Event, Log, Recorder, RunOf, args_hash, blockedlog, chatlog,
    finishes, read,
};
use crate::hookwire::{Decision, ToolCall};
use crate::sandboxblock::Kind;
use crate::sandboxblock::stop::{Reported, sandbox_block};
use crate::toolhooks::{Answer, Hook, Tunnel};

const DEVICE: &str = "01J9ZZDEVICE0000000000000";
const PLANE: &str = "/planes/one";
const CHAT: u32 = 4;
/// The proxy's refusal, as its client reads it.
const FORBIDDEN: &str = "HTTP/1.1 403 Forbidden";

/// The call the `pretooluse-web` hook sends for the `n`th `WebFetch` of `url`, where the proxy
/// answered `tunnel`, exactly as the hook's own answer makes it.
fn web_fetch(n: u32, url: &str, tunnel: Tunnel) -> ToolCall {
    let payload = serde_json::json!({
        "tool_name": "WebFetch",
        "tool_input": {"url": url, "prompt": format!("p{n}")},
    });
    let env = |name: &str| match name {
        n if n == crate::hookwire::SANDBOXED_ENV => Some("1".to_owned()),
        n if n == crate::hookwire::PROXY_ENV => Some("http://127.0.0.1:41000".to_owned()),
        _ => None,
    };
    let root = std::env::temp_dir();
    let hook = Hook {
        root: &root,
        in_plane: false,
        payload: &payload,
        env: &env,
        cwd: &root,
        now: "2026-05-04T11:32:17+00:00".parse().unwrap(),
        host: "box",
    };
    let Answer::Deny(verdict) = crate::toolhooks::pretooluse_web(&hook, &|_, _, _| tunnel.clone())
    else {
        panic!("the web judge did not refuse");
    };
    ToolCall {
        chat: CHAT,
        tool_hook: crate::hookreg::WEB_JUDGE.to_owned(),
        tool: Some("WebFetch".to_owned()),
        call: Some(format!("toolu_{n:02}")),
        args: Some(args_hash(&payload)),
        decision: Decision::Deny,
        rule: Some(verdict.reason),
        arm: verdict.arm.map(|arm| arm.id().to_owned()),
        hook_ms: 3,
        agent: None,
        at_ms: 0,
        skill: None,
    }
}

/// The proxy's block for a refused connection to `host`, written by the code the app writes it
/// with (`sandboxblock::stop`), as the call the proxy heard.
fn proxy_block(host: &str) -> Stop {
    block_of(host, Some("WebFetch"), Reported::ByTheProxy)
}

/// A sandbox block for `host` stamped with `tool`, reported as `reported`.
fn block_of(host: &str, tool: Option<&str>, reported: Reported) -> Stop {
    sandbox_block(
        Kind::Host,
        Some(host),
        Path::new("/project"),
        tool,
        reported,
    )
}

/// One chat's log, written as the host writes it.
struct Chat {
    dir: tempfile::TempDir,
    host: Recorder,
    next: u32,
}

impl Chat {
    /// Chat 4 begun under a verified sandbox.
    fn begun() -> Self {
        Self::begun_under("seatbelt")
    }

    /// Chat 4 begun under `sandbox`: `landlock` leaves the hook's reports unverified.
    fn begun_under(sandbox: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let mut host = Recorder::new(
            Log::open(dir.path(), DEVICE).unwrap(),
            crate::eventlog::ArgsKey::open(dir.path()).unwrap(),
        );
        let (chat, run) = (crate::reopen::mint(), crate::reopen::mint());
        host.begin(
            Path::new(PLANE),
            CHAT,
            RunOf {
                chat: &chat,
                run: &run,
            },
            Began::Start,
            &Attributes {
                harness: Some("claude-code".to_owned()),
                sandbox: Some(sandbox.to_owned()),
                ..Attributes::default()
            },
        )
        .unwrap();
        Self { dir, host, next: 0 }
    }

    /// The proxy's own block for `host`.
    fn proxy_blocks(&mut self, host: &str) -> &mut Self {
        self.host
            .stop(Path::new(PLANE), Some(CHAT), &proxy_block(host))
            .unwrap();
        self
    }

    /// A sandbox block for `host`, stamped `tool` and reported as `reported`.
    fn blocks(&mut self, host: &str, tool: Option<&str>, reported: Reported) -> &mut Self {
        self.host
            .stop(
                Path::new(PLANE),
                Some(CHAT),
                &block_of(host, tool, reported),
            )
            .unwrap();
        self
    }

    /// The hook's deny of a `WebFetch` of `host`, the proxy having answered `tunnel`.
    fn denies(&mut self, host: &str, tunnel: Tunnel) -> &mut Self {
        self.next += 1;
        let call = web_fetch(self.next, &format!("https://{host}/page"), tunnel);
        self.host
            .tool(Path::new(PLANE), &call, Instant::now())
            .unwrap();
        self
    }

    fn refuses(&mut self, host: &str) -> &mut Self {
        self.denies(host, Tunnel::Refused(FORBIDDEN.to_owned()))
    }

    /// The events written so far, one second apart in the order written: the log's own ids hold
    /// the time they were written at, which a test of a window cannot wait for.
    fn events(&self) -> Vec<Event> {
        stamped(read(self.dir.path()).unwrap())
    }
}

/// `events` with ids that say each was written one second after the one before.
fn stamped(mut events: Vec<Event>) -> Vec<Event> {
    for (at, event) in events.iter_mut().enumerate() {
        event.ulid = at_ms(1_800_000_000_000 + at as u64 * 1_000, at);
    }
    events
}

/// A ULID written at `ms`.
fn at_ms(ms: u64, random: usize) -> String {
    ulid::Ulid::from_parts(ms, random as u128).to_string()
}

/// Moves the `n`th block of `events` to `ms`.
fn written_at(events: &mut [Event], n: usize, ms: u64) {
    let (at, event) = events
        .iter_mut()
        .enumerate()
        .filter(|(_, event)| event.kind == BLOCK)
        .nth(n)
        .unwrap();
    event.ulid = at_ms(ms, at);
}

fn blocks_in(events: &[Event]) -> Vec<Block> {
    events
        .iter()
        .filter(|event| event.kind == BLOCK)
        .map(|event| serde_json::from_value(event.body.clone()).unwrap())
        .collect()
}

/// What the Blocked count says, the sidebar's and the unverified together.
fn counted(events: &[Event]) -> u32 {
    let counts = chatlog::counts(events).all;
    counts.blocks + counts.unverified_blocks
}

fn chat_of(events: &[Event]) -> String {
    events.iter().find_map(|event| event.chat.clone()).unwrap()
}

/// How many blocks the Blocked log lists.
fn listed(events: &[Event]) -> usize {
    let chat = chat_of(events);
    blockedlog::listed(events, 1_800_100_000_000, &|one| one == chat).len()
}

/// How many block rows the Chat log draws.
fn rows(events: &[Event]) -> usize {
    chatlog::rows(events, &chat_of(events))
        .iter()
        .filter(|row| matches!(row, Row::Blocked { .. }))
        .count()
}

/// What the marker's running tally says, the events delivered one by one as the log's
/// subscription delivers them.
fn marker(events: &[Event]) -> u32 {
    let chat = chat_of(events);
    let mut blocks = Blocks::default();
    for event in events {
        blocks.take(Delivery::Event(event.clone()));
    }
    let tally = blocks.since(&chat, None).tally;
    tally.blocks + tally.unverified_blocks
}

/// Every place that counts blocks says `n`.
#[track_caller]
fn everywhere(events: &[Event], n: u32) {
    assert_eq!(counted(events), n, "the Blocked count");
    assert_eq!(listed(events), n as usize, "the Blocked log");
    assert_eq!(rows(events), n as usize, "the Chat log's rows");
    assert_eq!(marker(events), n, "the marker's running tally");
}

/// The proxy's refusal and the hook's deny are both written, and the count and the Blocked log
/// hold the one refusal once.
#[test]
fn one_refused_web_fetch_is_counted_and_listed_once_and_both_causes_are_in_the_log() {
    let mut chat = Chat::begun();
    chat.proxy_blocks("never.example.com")
        .refuses("never.example.com");
    let events = chat.events();

    let written = blocks_in(&events);
    assert_eq!(
        written.iter().map(|block| block.source).collect::<Vec<_>>(),
        vec![Source::Sandbox, Source::Guard],
        "both causes stay recorded"
    );
    assert_eq!(
        written[1].target,
        Some(Target::host("never.example.com").unwrap()),
        "the deny names the host it refused"
    );
    everywhere(&events, 1);
    let chat_id = chat_of(&events);
    let shown = blockedlog::listed(&events, 1_800_100_000_000, &|one| one == chat_id);
    assert_eq!(shown[0].block.source, Source::Sandbox);
}

/// The proxy's other answers are no refusal of the proxy's to record: a `502` for an allowed host
/// that does not resolve, a `503` at its connection limit, a `400` for a request it could not
/// read. No sandbox block is behind them, so the deny is the refusal's one record and counts.
#[test]
fn a_web_fetch_the_proxy_answered_without_writing_a_block_counts_once() {
    for status in [
        "HTTP/1.1 502 Bad Gateway",
        "HTTP/1.1 503 Service Unavailable",
        "HTTP/1.1 400 Bad Request",
        "HTTP/1.1 403 Forbidden",
    ] {
        let mut chat = Chat::begun();
        chat.denies("allowed.example.com", Tunnel::Refused(status.to_owned()));
        everywhere(&chat.events(), 1);
    }
}

/// A host the person kept blocked: the proxy refuses its retries at once and writes no new block,
/// so each retried fetch counts for its own deny, once.
#[test]
fn a_retry_of_a_host_kept_blocked_counts_once_per_refused_fetch() {
    let mut chat = Chat::begun();
    chat.proxy_blocks("kept.example.com")
        .refuses("kept.example.com")
        .refuses("kept.example.com")
        .refuses("kept.example.com");
    everywhere(&chat.events(), 3);
}

/// A refusal the person later allowed has no sandbox block (an Allow writes none): the deny that
/// was heard counts, once.
#[test]
fn a_refusal_the_person_later_allowed_counts_once() {
    let mut chat = Chat::begun();
    chat.refuses("later.example.com");
    everywhere(&chat.events(), 1);
}

/// A refusal still an open ask has no block until the ask leaves: the deny counts once while it
/// is open, and counts the same once the block is written, never twice and never not at all.
#[test]
fn an_open_ask_counts_once_while_open_and_once_after_it_leaves_as_a_block() {
    let mut chat = Chat::begun();
    chat.refuses("asked.example.com");
    everywhere(&chat.events(), 1);
    // Keep blocked: the held connection's block is written, up to the quiet person's hold later.
    chat.proxy_blocks("asked.example.com");
    let mut events = chat.events();
    written_at(&mut events, 1, 1_800_000_000_000 + 14 * 60_000);
    everywhere(&events, 1);
}

/// The proxy tells the app of a host once, and of eight hosts at most: the ninth is refused and
/// recorded by the hook's deny alone.
#[test]
fn the_ninth_refused_host_counts_once() {
    let mut chat = Chat::begun();
    for n in 1..=9 {
        let host = format!("host{n}.example.com");
        if n <= 8 {
            chat.proxy_blocks(&host);
        }
        chat.refuses(&host);
    }
    everywhere(&chat.events(), 9);
}

/// A quiet person: the hook's read timed out (the proxy never answered it, which the hook counts
/// as its own refusal), and the hold's block is written when the hold ends, up to 15 minutes
/// later. The two are one refusal.
#[test]
fn a_web_fetch_the_quiet_person_never_answered_in_time_counts_once() {
    let mut chat = Chat::begun();
    chat.denies(
        "quiet.example.com",
        Tunnel::Unreached("the proxy did not answer".to_owned()),
    );
    everywhere(&chat.events(), 1);
    chat.proxy_blocks("quiet.example.com");
    let mut events = chat.events();
    written_at_ms_after_the_deny(&mut events, 15 * 60_000);
    everywhere(&events, 1);
}

/// Moves the log's last block `after` ms past the deny before it.
fn written_at_ms_after_the_deny(events: &mut [Event], after: u64) {
    written_at(events, 1, 1_800_000_000_000 + after);
}

/// What pairs is a plain sandbox block of the same chat and host near the deny: not a block of
/// another host, and not one a long time away. Its tool is not asked: the proxy stamps a block
/// with the chat's latest call, and the web hook's own call is not heard until after its verdict.
#[test]
fn only_a_sandbox_block_of_the_same_host_and_time_pairs_with_its_deny() {
    // Another host.
    let mut chat = Chat::begun();
    chat.proxy_blocks("one.example.com")
        .refuses("two.example.com");
    everywhere(&chat.events(), 2);

    // The same host an hour later.
    let mut chat = Chat::begun();
    chat.proxy_blocks("late.example.com")
        .refuses("late.example.com");
    let mut events = chat.events();
    written_at(&mut events, 1, 1_800_000_000_000 + 60 * 60_000);
    everywhere(&events, 2);
}

/// **The proxy's block is stamped with the chat's latest call**, which for a `WebFetch` after a
/// `Bash` is `Bash`, or nothing where no call was heard: the deny pairs with it all the same.
#[test]
fn a_block_stamped_with_another_tool_still_pairs_with_the_web_fetchs_deny() {
    for tool in [Some("Bash"), Some("Read"), Some("WebFetch"), None] {
        let mut chat = Chat::begun();
        chat.blocks("stamped.example.com", tool, Reported::ByTheProxy)
            .refuses("stamped.example.com");
        everywhere(&chat.events(), 1);

        // And in the other order: the deny first, the block when the held ask leaves.
        let mut chat = Chat::begun();
        chat.refuses("stamped.example.com").blocks(
            "stamped.example.com",
            tool,
            Reported::ByTheProxy,
        );
        everywhere(&chat.events(), 1);
    }
}

/// A sandbox block the chat's own hook line wrote (unverified) is the chat's word: it must not
/// absorb a refusal the proxy and the hook verified.
#[test]
fn an_unverified_sandbox_block_never_absorbs_a_deny() {
    for tool in [None, Some("WebFetch")] {
        let mut chat = Chat::begun();
        chat.blocks("forged.example.com", tool, Reported::ByItsHook)
            .refuses("forged.example.com");
        let events = chat.events();
        assert_eq!(
            blocks_in(&events)[0].qualifier,
            Some(Qualifier::Unverified),
            "the block is the chat's own line"
        );
        everywhere(&events, 2);

        // The deny first, the unverified block after it.
        let mut chat = Chat::begun();
        chat.refuses("forged.example.com")
            .blocks("forged.example.com", tool, Reported::ByItsHook);
        everywhere(&chat.events(), 2);
    }
}

/// A block absorbs one deny, never two: the second fetch of the host counts for itself.
#[test]
fn one_sandbox_block_holds_one_deny_of_two() {
    let mut chat = Chat::begun();
    chat.proxy_blocks("two.example.com")
        .refuses("two.example.com")
        .refuses("two.example.com");
    everywhere(&chat.events(), 2);

    let mut chat = Chat::begun();
    chat.refuses("two.example.com")
        .refuses("two.example.com")
        .proxy_blocks("two.example.com");
    everywhere(&chat.events(), 2);
}

/// A block another chat's proxy wrote pairs with nothing of this chat's.
#[test]
fn a_block_of_another_chat_never_pairs() {
    let blocks = |chat: &str, source, host: &str| {
        let block = Block {
            tool: String::new(),
            source,
            qualifier: None,
            target: Some(Target::host(host).unwrap()),
            args: None,
        };
        (chat.to_owned(), block)
    };
    let mut pairing = crate::eventlog::web_repeats::Pairing::default();
    let fed = |pairing: &mut crate::eventlog::web_repeats::Pairing<u32>,
               key: u32,
               (chat, block): (String, Block)| {
        pairing.feed(&chat, key, Some(1_800_000_000_000), &block)
    };
    use crate::eventlog::web_repeats::Fed;
    assert_eq!(
        fed(
            &mut pairing,
            1,
            blocks("chat-a", Source::Sandbox, "x.example.com")
        ),
        Fed::Counts
    );
    assert_eq!(
        fed(
            &mut pairing,
            2,
            blocks("chat-b", Source::Guard, "x.example.com")
        ),
        Fed::Counts,
        "the other chat's deny is no repeat of chat a's block"
    );
    assert_eq!(
        fed(
            &mut pairing,
            3,
            blocks("chat-a", Source::Guard, "x.example.com")
        ),
        Fed::Repeat,
        "its own chat's deny is"
    );
}

/// **One spelling of a host on both sides**: the deny names the host as the url crate writes it
/// (lower case, trailing dot kept, a bare IPv6 address, `_` kept, the whole name), the proxy's
/// block as the sandbox names it (a default port dropped or kept, an address in brackets, a
/// target clipped at 120 characters with or without its port).
#[test]
fn a_hosts_spellings_on_the_deny_and_the_block_are_the_same_host() {
    let long = |n: usize| {
        let first = "a".repeat(60);
        let rest = "b".repeat(n - 61);
        format!("{first}.{rest}")
    };
    let hosts: Vec<(String, Vec<String>)> = vec![
        (
            "example.com.".to_owned(),
            vec!["example.com".into(), "example.com:443".into()],
        ),
        ("EXAMPLE.com".to_owned(), vec!["example.com:443".into()]),
        (
            "[2001:db8::1]".to_owned(),
            vec!["[2001:db8::1]".into(), "[2001:db8::1]:443".into()],
        ),
        (
            "a_b.example.com".to_owned(),
            vec!["a_b.example.com".into(), "a_b.example.com:443".into()],
        ),
    ]
    .into_iter()
    .chain([118, 119, 120, 121, 125, 200].map(|n| {
        let host = long(n);
        (host.clone(), vec![host.clone(), format!("{host}:443")])
    }))
    .collect();
    for (denied, named) in hosts {
        for named in named {
            let mut chat = Chat::begun();
            chat.proxy_blocks(&named).refuses(&denied);
            let events = chat.events();
            assert_eq!(
                blocks_in(&events).len(),
                2,
                "{denied} / {named}: both are written"
            );
            assert!(
                blocks_in(&events)
                    .iter()
                    .all(|block| block.target.is_some()),
                "{denied} / {named}: both name a host: {:?}",
                blocks_in(&events)
            );
            assert_eq!(counted(&events), 1, "{denied} refused, {named} blocked");
        }
    }
}

/// A rule with a host is no tally bucket of its own: the log's tool-call event holds the rule
/// whole, whatever host the model asked for; the host is only in the deny's block's target.
#[test]
fn the_rule_the_log_tallies_is_one_whatever_the_host() {
    let mut chat = Chat::begun();
    chat.refuses("one.example.com")
        .refuses("two.example.com")
        .denies(
            "three.example.com",
            Tunnel::Unreached("the proxy did not answer".to_owned()),
        );
    let events = chat.events();
    let rules: Vec<String> = events
        .iter()
        .filter(|event| event.kind.starts_with("hook.") && event.body.get("rule").is_some())
        .map(|event| event.body["rule"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(rules.len(), 3, "{rules:?}");
    assert_eq!(rules[0], crate::toolhooks::WEB_PROXY_REFUSED);
    assert_eq!(rules[1], crate::toolhooks::WEB_PROXY_REFUSED);
    assert_eq!(rules[2], crate::toolhooks::WEB_PROXY_UNREACHED);
    let targets: Vec<_> = blocks_in(&events)
        .iter()
        .filter_map(|block| block.target.clone())
        .collect();
    assert_eq!(targets.len(), 3, "the hosts are the blocks' targets");
}

/// A turn's finish names the turn's blocks: a deny with no sandbox block behind it stays one of
/// them, and one that has a block counts for the block's alone.
#[test]
fn a_turn_keeps_the_block_of_a_web_fetch_no_sandbox_block_holds() {
    let finished = |chat: &mut Chat, proxy: bool| {
        let mut events = {
            if proxy {
                chat.proxy_blocks("turn.example.com");
            }
            chat.refuses("turn.example.com");
            read(chat.dir.path()).unwrap()
        };
        let template = events[0].clone();
        let mark = |kind: &str| Event {
            kind: kind.to_owned(),
            body: serde_json::json!({}),
            ..template.clone()
        };
        // A prompt, the turn's call (already written), the turn's end.
        events.insert(1, mark("hook.userpromptsubmit"));
        events.push(mark("hook.stop"));
        let events = stamped(events);
        let chat_id = chat_of(&events);
        finishes::finishes(&events, &chat_id)
            .iter()
            .map(|finish| finish.blocked)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        finished(&mut Chat::begun(), false),
        [1],
        "no block behind it"
    );
    assert_eq!(finished(&mut Chat::begun(), true), [1], "a block behind it");
}

/// The marker's running tally is fed one event at a time and holds the pair once in either order:
/// the deny counted when it came is let go of when its block follows.
#[test]
fn the_markers_running_tally_holds_a_pair_once_in_either_order() {
    let mut chat = Chat::begun();
    chat.proxy_blocks("a.example.com").refuses("a.example.com");
    assert_eq!(marker(&chat.events()), 1, "the block, then the deny");

    let mut chat = Chat::begun();
    chat.refuses("b.example.com").proxy_blocks("b.example.com");
    assert_eq!(marker(&chat.events()), 1, "the deny, then the block");
    let events = chat.events();
    let deny_alone: Vec<Event> = events[..events.len() - 1].to_vec();
    assert_eq!(
        marker(&deny_alone),
        1,
        "the deny, its block not yet written"
    );
}

/// Fail closed stays as it is: a deny the proxy never heard of (no proxy answered, so it wrote no
/// block) is the only record of the refusal, and counts.
#[test]
fn a_web_fetch_refused_because_the_proxy_did_not_answer_still_counts_once() {
    let mut chat = Chat::begun();
    chat.denies(
        "never.example.com",
        Tunnel::Unreached("the proxy did not answer".to_owned()),
    );
    everywhere(&chat.events(), 1);
}

/// A guard's deny of any other tool, or of a `WebFetch` whose rule names no host, is no web
/// deny: it stays a plain, counted block.
#[test]
fn a_guards_other_deny_is_never_taken_for_a_web_fetchs() {
    let call = ToolCall {
        tool_hook: crate::hookreg::WEB_JUDGE.to_owned(),
        rule: Some(crate::toolhooks::WEB_PROXY_REFUSED.to_owned()),
        ..web_fetch(
            1,
            "https://x.example.com/",
            Tunnel::Refused(FORBIDDEN.to_owned()),
        )
    };
    let block = block_of_line(&call, true).unwrap();
    assert_eq!(block.target, None, "a rule that names no host");

    let call = ToolCall {
        tool: Some("Bash".to_owned()),
        ..web_fetch(
            1,
            "https://x.example.com/",
            Tunnel::Refused(FORBIDDEN.to_owned()),
        )
    };
    assert_eq!(
        block_of_line(&call, true).unwrap().target,
        None,
        "another tool"
    );

    let call = ToolCall {
        tool_hook: "pretooluse-bash".to_owned(),
        ..web_fetch(
            1,
            "https://x.example.com/",
            Tunnel::Refused(FORBIDDEN.to_owned()),
        )
    };
    assert_eq!(
        block_of_line(&call, true).unwrap().target,
        None,
        "another hook"
    );
}

/// A block held for a quiet person comes in a later turn than its deny: the refusal stays counted
/// in the turn of the deny, and the later turn counts nothing for it.
#[test]
fn a_late_block_keeps_the_count_in_the_denys_turn() {
    let mut chat = Chat::begun();
    chat.refuses("late.example.com")
        .proxy_blocks("late.example.com");
    let mut events = read(chat.dir.path()).unwrap();
    let template = events[0].clone();
    let mark = |kind: &str| Event {
        kind: kind.to_owned(),
        body: serde_json::json!({}),
        ..template.clone()
    };
    // Turn 1: the prompt, the deny, the turn's end. Turn 2: a prompt, the block, its end.
    let block = events.pop().unwrap();
    events.insert(1, mark("hook.userpromptsubmit"));
    events.push(mark("hook.stop"));
    events.push(mark("hook.userpromptsubmit"));
    events.push(block);
    events.push(mark("hook.stop"));
    let mut events = stamped(events);
    let block_at = events
        .iter()
        .rposition(|event| event.kind == BLOCK)
        .unwrap();
    events[block_at].ulid = at_ms(1_800_000_000_000 + 13 * 60_000, block_at);
    let chat_id = chat_of(&events);
    let turns: Vec<_> = finishes::finishes(&events, &chat_id)
        .iter()
        .map(|finish| finish.blocked)
        .collect();
    assert_eq!(turns, [1, 0], "one block, in the deny's turn");
    assert_eq!(counted(&events), 1);
}

/// A refusal already looked at is not shown again when its block follows: the deny was counted
/// and looked at; the block that retires it adds nothing after the look.
#[test]
fn a_block_that_follows_a_looked_at_deny_does_not_show_it_again() {
    let mut chat = Chat::begun();
    chat.refuses("seen.example.com")
        .proxy_blocks("seen.example.com");
    let events = chat.events();
    let (deny, block) = {
        let at = |source| {
            events
                .iter()
                .find(|event| {
                    event.kind == BLOCK
                        && serde_json::from_value::<Block>(event.body.clone())
                            .is_ok_and(|one| one.source == source)
                })
                .unwrap()
                .seq
        };
        (at(Source::Guard), at(Source::Sandbox))
    };
    assert!(block > deny);
    let chat_id = chat_of(&events);
    let mut blocks = Blocks::default();
    for event in &events {
        blocks.take(Delivery::Event(event.clone()));
        if event.seq == deny {
            let held = blocks.since(&chat_id, None);
            assert_eq!(held.blocks + held.unverified, 1, "the deny counts");
        }
    }
    assert_eq!(
        blocks.since(&chat_id, None).tally.blocks
            + blocks.since(&chat_id, None).tally.unverified_blocks,
        1,
        "still one in all"
    );
    let after_the_look = blocks.since(&chat_id, Some(deny));
    assert_eq!(
        after_the_look.blocks + after_the_look.unverified,
        0,
        "the refusal looked at is not shown again"
    );
}

/// What the log lost with `Ahead` takes the held denies and blocks too: a block heard before it
/// pairs with nothing after it, so the deny that follows counts.
#[test]
fn lines_the_log_lost_let_go_of_the_pairing_too() {
    let mut chat = Chat::begun();
    chat.proxy_blocks("lost.example.com")
        .refuses("lost.example.com");
    let events = chat.events();
    let chat_id = chat_of(&events);
    let [block, deny] = events
        .iter()
        .filter(|event| event.kind == BLOCK)
        .collect::<Vec<_>>()[..]
    else {
        panic!("a block and a deny: {events:?}");
    };
    let mut blocks = Blocks::default();
    blocks.take(Delivery::Event(block.clone()));
    blocks.take(Delivery::Ahead {
        cursor: 99,
        log_ends_at: 0,
    });
    blocks.take(Delivery::Event(deny.clone()));
    let tally = blocks.since(&chat_id, None);
    assert_eq!(
        tally.blocks + tally.unverified,
        1,
        "the deny is no repeat of a block the stream let go of"
    );
}

/// **A chat begun under landlock writes its deny unverified, and the proxy's block is plain**
/// (R-174): whichever is written first, the one refusal counts once, and as the plain block says
/// it, verified, in the marker's tally, the Blocked count and a turn's finish alike.
#[test]
fn a_deny_unverified_under_landlock_is_counted_as_the_plain_block_that_holds_it() {
    for deny_first in [true, false] {
        let mut chat = Chat::begun_under("landlock");
        if deny_first {
            chat.refuses("land.example.com")
                .proxy_blocks("land.example.com");
        } else {
            chat.proxy_blocks("land.example.com")
                .refuses("land.example.com");
        }
        let events = chat.events();
        let deny = blocks_in(&events)
            .into_iter()
            .find(|block| block.source == Source::Guard)
            .unwrap();
        assert_eq!(
            deny.qualifier,
            Some(Qualifier::Unverified),
            "the premise: the deny is unverified under landlock"
        );
        everywhere(&events, 1);
        let chat_id = chat_of(&events);
        let mut blocks = Blocks::default();
        for event in &events {
            blocks.take(Delivery::Event(event.clone()));
        }
        let tally = blocks.since(&chat_id, None).tally;
        assert_eq!(
            (tally.blocks, tally.unverified_blocks),
            (1, 0),
            "deny first: {deny_first}: the marker reads one verified block"
        );
        assert_eq!(counted_verified(&events), 1, "the Blocked count agrees");
    }
}

/// What the Blocked count says is verified.
fn counted_verified(events: &[Event]) -> u32 {
    chatlog::counts(events).all.blocks
}

/// A turn's finish names the plain block's source and qualifier for a deny it holds.
#[test]
fn a_turn_names_the_plain_block_that_holds_an_unverified_deny() {
    let mut chat = Chat::begun_under("landlock");
    chat.refuses("named.example.com")
        .proxy_blocks("named.example.com");
    let mut events = read(chat.dir.path()).unwrap();
    let template = events[0].clone();
    let mark = |kind: &str| Event {
        kind: kind.to_owned(),
        body: serde_json::json!({}),
        ..template.clone()
    };
    events.insert(1, mark("hook.userpromptsubmit"));
    events.push(mark("hook.stop"));
    let events = stamped(events);
    let chat_id = chat_of(&events);
    let finished = finishes::finishes(&events, &chat_id);
    assert_eq!(finished.len(), 1);
    assert_eq!(finished[0].blocked, 1);
    assert_eq!(finished[0].unresolved[0].source, Source::Sandbox);
    assert_eq!(finished[0].unresolved[0].qualifier, None);
}

/// **The bounded undercount**: a plain block of a `Bash` command's own refusal of a host absorbs
/// a `WebFetch` deny of it within the window, so the two count as one; it absorbs one deny and
/// never two, and not one outside the window.
#[test]
fn a_bash_commands_plain_block_absorbs_at_most_one_web_deny_of_its_host_inside_the_window() {
    let bash = |chat: &mut Chat| {
        chat.blocks("curl.example.com", Some("Bash"), Reported::ByTheProxy);
    };
    let mut chat = Chat::begun();
    bash(&mut chat);
    chat.refuses("curl.example.com");
    assert_eq!(
        blocks_in(&chat.events())[0].tool,
        "Bash",
        "the premise: the block is a Bash command's"
    );
    everywhere(&chat.events(), 1);

    let mut chat = Chat::begun();
    bash(&mut chat);
    chat.refuses("curl.example.com").refuses("curl.example.com");
    everywhere(&chat.events(), 2);

    // Outside the window (the proxy writes as it answers: the deny follows in seconds).
    let mut chat = Chat::begun();
    bash(&mut chat);
    chat.refuses("curl.example.com");
    let mut events = chat.events();
    written_at(&mut events, 1, 1_800_000_000_000 + 30_000);
    everywhere(&events, 2);
}

/// Two long hosts that share their first hundred characters are two hosts.
#[test]
fn two_long_hosts_sharing_their_first_hundred_characters_never_pair() {
    let first = format!("{}.{}", "a".repeat(60), "b".repeat(50));
    let (one, other) = (
        format!("{first}.one.example.com"),
        format!("{first}.two.example.com"),
    );
    let mut chat = Chat::begun();
    chat.proxy_blocks(&one).refuses(&other);
    everywhere(&chat.events(), 2);
    let mut chat = Chat::begun();
    chat.proxy_blocks(&one).refuses(&one);
    everywhere(&chat.events(), 1);
}
