// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **One refused `WebFetch` counts once, never twice and never not at all** (#7465): at the full
//! sandbox the `pretooluse-web` hook asks the chat's proxy for the host. When the proxy refuses
//! it may write a sandbox block, and the hook's own deny is written as a guard's block aimed at
//! the same host. Both stay in the log (two causes). Whether the second *repeats* the first is
//! decided here, from the log's own events, never from the proxy's reply: a deny repeats only
//! when a sandbox block for the same chat and host is recorded near it, and each sandbox block
//! absorbs at most one deny. A deny with no such block (the proxy answered 502 or 503, a host
//! kept blocked that the proxy refuses without writing again, a refusal still an open ask) counts
//! as the refusal it is, and counts the same once the block is written, so a fetch is counted at
//! least once and at most once.
//!
//! [`Pairing`] is fed a log's blocks in order, whether read whole ([`repeats`]) or delivered one
//! by one ([`super::chatlog::Blocks`]), and answers in either order of the two.

use std::collections::{BTreeMap, BTreeSet};

use super::Event;
use super::stops::{self, Block, Source, Target};

/// How long before a deny the proxy's block may have been written: the hook waits on the proxy up
/// to its own timeout ([`crate::toolhooks::WEB_JUDGE_WAIT`]), and the proxy writes its block as it
/// answers, so the block is at most that long before the deny.
const BEFORE_MS: u64 = 120_000;

/// How long after a deny the proxy's block may be written: a refusal held as an ask is written as
/// a block when its ask leaves ([`crate::sandboxblock::stop`]), up to a quiet person's hold
/// (15 minutes) and a little more.
const AFTER_MS: u64 = 20 * 60_000;

/// The most unpaired blocks of each side held per chat: past it the oldest are let go, which
/// only means that a block that old pairs with nothing and counts.
const HELD: usize = 64;

/// What a block's feed says of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fed<K> {
    /// It counts: a block of any other kind, a deny no recorded block holds, or a sandbox block.
    Counts,
    /// It is a deny a recorded sandbox block already holds: it counts for nothing.
    Repeat,
    /// A sandbox block that paired with the earlier deny `K`, which counted when it was fed and
    /// counts no more.
    Retires(K),
}

/// A deny of a `WebFetch` or a sandbox block for a host, waiting for its pair.
#[derive(Debug)]
struct Waiting<K> {
    key: K,
    at_ms: u64,
    host: String,
}

#[derive(Debug)]
struct Chat<K> {
    sandbox: Vec<Waiting<K>>,
    denies: Vec<Waiting<K>>,
}

impl<K> Default for Chat<K> {
    fn default() -> Self {
        Self {
            sandbox: Vec::new(),
            denies: Vec::new(),
        }
    }
}

/// **Pairs the web judge's denies with the proxy's sandbox blocks** of each chat, as they are fed.
#[derive(Debug)]
pub struct Pairing<K> {
    chats: BTreeMap<String, Chat<K>>,
}

impl<K> Default for Pairing<K> {
    fn default() -> Self {
        Self {
            chats: BTreeMap::new(),
        }
    }
}

/// A host as two blocks compare it: lower case, without its port or brackets.
fn bare(host: &str) -> String {
    let host = host.to_ascii_lowercase();
    if let Some(inner) = host.strip_prefix('[') {
        return inner.split(']').next().unwrap_or(inner).to_owned();
    }
    match host.rsplit_once(':') {
        // `host:port`; a bare IPv6 address holds more than one colon and is left whole.
        Some((name, port)) if !name.contains(':') && port.chars().all(|c| c.is_ascii_digit()) => {
            name.to_owned()
        }
        _ => host,
    }
}

/// The host a block names, and whether it is the web judge's deny (`true`) or a sandbox block
/// the proxy could have written for a `WebFetch` (`false`); `None` for any other block.
fn side(block: &Block) -> Option<(bool, String)> {
    let Some(Target::Host(host)) = &block.target else {
        return None;
    };
    match block.source {
        // A guard's block is aimed at a host only by the web judge's deny.
        Source::Guard => Some((true, bare(host.as_str()))),
        // The proxy's block for the call a `WebFetch` made (its tool, or none where no call was
        // heard), never a command's connection that happens to name the same host.
        Source::Sandbox if block.tool == "WebFetch" || block.tool.is_empty() => {
            Some((false, bare(host.as_str())))
        }
        _ => None,
    }
}

fn within(sandbox_ms: u64, deny_ms: u64) -> bool {
    sandbox_ms + BEFORE_MS >= deny_ms && sandbox_ms <= deny_ms.saturating_add(AFTER_MS)
}

fn let_go_of_the_oldest<K>(held: &mut Vec<Waiting<K>>) {
    if held.len() > HELD {
        held.remove(0);
    }
}

impl<K: Copy> Pairing<K> {
    /// Feeds `block`, written for `chat` at `at_ms` (`None` where its id holds no time: it pairs
    /// with nothing), identified by `key` in what the reader holds of it.
    pub fn feed(&mut self, chat: &str, key: K, at_ms: Option<u64>, block: &Block) -> Fed<K> {
        let (Some(at_ms), Some((deny, host))) = (at_ms, side(block)) else {
            return Fed::Counts;
        };
        let held = self.chats.entry(chat.to_owned()).or_default();
        let waiting = Waiting { key, at_ms, host };
        if deny {
            // The nearest recorded sandbox block of this host that no deny has taken.
            let nearest = held
                .sandbox
                .iter()
                .enumerate()
                .filter(|(_, one)| one.host == waiting.host && within(one.at_ms, at_ms))
                .min_by_key(|(_, one)| one.at_ms.abs_diff(at_ms))
                .map(|(at, _)| at);
            if let Some(at) = nearest {
                held.sandbox.remove(at);
                return Fed::Repeat;
            }
            held.denies.push(waiting);
            let_go_of_the_oldest(&mut held.denies);
            Fed::Counts
        } else {
            // The earliest deny of this host waiting for a block.
            let earliest = held
                .denies
                .iter()
                .position(|one| one.host == waiting.host && within(at_ms, one.at_ms));
            if let Some(at) = earliest {
                return Fed::Retires(held.denies.remove(at).key);
            }
            held.sandbox.push(waiting);
            let_go_of_the_oldest(&mut held.sandbox);
            Fed::Counts
        }
    }
}

/// **The events (by their place in `events`) that are a web judge's deny a recorded sandbox block
/// holds**: left out of every count and list of blocks, still in the log.
pub fn repeats(events: &[Event]) -> BTreeSet<usize> {
    let mut pairing = Pairing::default();
    let mut repeats = BTreeSet::new();
    for (at, event) in events.iter().enumerate() {
        if event.kind != stops::BLOCK {
            continue;
        }
        let (Some(chat), Ok(block)) = (
            event.chat.as_deref(),
            serde_json::from_value::<Block>(event.body.clone()),
        ) else {
            continue;
        };
        match pairing.feed(chat, at, super::written_at_ms(&event.ulid), &block) {
            Fed::Counts => {}
            Fed::Repeat => {
                repeats.insert(at);
            }
            Fed::Retires(deny) => {
                repeats.insert(deny);
            }
        }
    }
    repeats
}
