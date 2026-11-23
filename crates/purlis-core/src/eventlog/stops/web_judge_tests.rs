// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **One refused `WebFetch` counts once** (#7465): at the full sandbox the `pretooluse-web` hook
//! asks the chat's proxy for the host, and the proxy's refusal is a sandbox block. The hook's own
//! deny, which the host writes as a guard's block, is that same refusal heard twice. Both stay in
//! the log; the Blocked count and the Blocked log hold the refusal once.

use std::path::Path;
use std::time::Instant;

use super::*;
use crate::eventlog::{
    Attributes, Began, Log, Recorder, RunOf, args_hash, blockedlog, chatlog, read,
};
use crate::hookwire::{Decision, ToolCall};
use crate::toolhooks::{Answer, Hook, Tunnel};

const DEVICE: &str = "01J9ZZDEVICE0000000000000";
const PLANE: &str = "/planes/one";
const CHAT: u32 = 4;

fn recorder(dir: &Path) -> Recorder {
    Recorder::new(
        Log::open(dir, DEVICE).unwrap(),
        crate::eventlog::ArgsKey::open(dir).unwrap(),
    )
}

/// The call the `pretooluse-web` hook sends for a `WebFetch` of `url`, where the proxy answered
/// `tunnel`, exactly as the hook's own answer makes it.
fn web_fetch(url: &str, tunnel: Tunnel) -> ToolCall {
    let payload = serde_json::json!({
        "tool_name": "WebFetch",
        "tool_input": {"url": url, "prompt": "p"},
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
        call: Some("toolu_01".to_owned()),
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

/// The proxy's block for a refused connection to `host`, as the app writes it.
fn proxy_block(host: &str) -> Stop {
    Stop::Block(Block {
        tool: "WebFetch".to_owned(),
        source: Source::Sandbox,
        qualifier: None,
        target: Some(Target::host(host).unwrap()),
        args: None,
    })
}

/// A log with chat 4 begun under a verified sandbox, `call` heard by the hook and, where
/// `proxy_host` is named, the proxy's block for it.
fn logged(
    call: &ToolCall,
    proxy_host: Option<&str>,
) -> (tempfile::TempDir, Vec<crate::eventlog::Event>) {
    let dir = tempfile::tempdir().unwrap();
    let mut host = recorder(dir.path());
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
            sandbox: Some("seatbelt".to_owned()),
            ..Attributes::default()
        },
    )
    .unwrap();
    if let Some(proxy_host) = proxy_host {
        host.stop(Path::new(PLANE), Some(CHAT), &proxy_block(proxy_host))
            .unwrap();
    }
    host.tool(Path::new(PLANE), call, Instant::now()).unwrap();
    let events = read(dir.path()).unwrap();
    (dir, events)
}

fn blocks_in(events: &[crate::eventlog::Event]) -> Vec<Block> {
    events
        .iter()
        .filter(|event| event.kind == BLOCK)
        .map(|event| serde_json::from_value(event.body.clone()).unwrap())
        .collect()
}

fn now_ms() -> u64 {
    u64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis(),
    )
    .unwrap()
}

/// #7465: the proxy's refusal and the hook's deny are both written, and the count and the
/// Blocked log hold the one refusal once.
#[test]
fn one_refused_web_fetch_is_counted_and_listed_once_and_both_causes_are_in_the_log() {
    let call = web_fetch(
        "https://never.example.com/page",
        Tunnel::Refused("HTTP/1.1 403 Forbidden".to_owned()),
    );
    let (_dir, events) = logged(&call, Some("never.example.com"));

    let written = blocks_in(&events);
    assert_eq!(
        written.iter().map(|block| block.source).collect::<Vec<_>>(),
        vec![Source::Sandbox, Source::Guard],
        "both causes stay recorded"
    );

    let counts = chatlog::counts(&events);
    assert_eq!(
        counts.all.blocks + counts.all.unverified_blocks,
        1,
        "the Blocked count"
    );

    let chat = events.iter().find_map(|event| event.chat.clone()).unwrap();
    let listed = blockedlog::listed(&events, now_ms(), &|one| one == chat);
    assert_eq!(listed.len(), 1, "the Blocked log");
    assert_eq!(listed[0].block.source, Source::Sandbox);
    assert_eq!(
        listed[0].block.target,
        Some(Target::host("never.example.com").unwrap())
    );
}

/// Fail closed stays as it is: a deny the proxy never heard of (no proxy answered, so it wrote no
/// block) is the only record of the refusal, and counts.
#[test]
fn a_web_fetch_refused_because_the_proxy_did_not_answer_still_counts_once() {
    let call = web_fetch(
        "https://never.example.com/page",
        Tunnel::Unreached("the proxy did not answer".to_owned()),
    );
    let (_dir, events) = logged(&call, None);
    let counts = chatlog::counts(&events).all;
    assert_eq!(counts.blocks + counts.unverified_blocks, 1);
    let chat = events.iter().find_map(|event| event.chat.clone()).unwrap();
    assert_eq!(
        blockedlog::listed(&events, now_ms(), &|one| one == chat).len(),
        1
    );
}
