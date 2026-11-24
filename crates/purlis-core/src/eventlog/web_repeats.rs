// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **One refused `WebFetch` counts once, never twice and never not at all** (#7465): at the full
//! sandbox the `pretooluse-web` hook asks the chat's proxy for the host. When the proxy refuses
//! it may write a sandbox block, and the hook's own deny is written as a guard's block aimed at
//! the same host. Both stay in the log (two causes). Whether the second *repeats* the first is
//! decided here, from the log's own events, never from the proxy's reply: a deny repeats only
//! when a sandbox block for the same chat and host is recorded near it, and each sandbox block
//! absorbs at most one deny.
//!
//! **What pairs is a plain sandbox block** (no qualifier: the proxy's own report, which only the
//! app writes; a chat's own hook line can write an unverified one, and that must not absorb a
//! verified refusal), **never the block's tool**: the proxy's block takes the tool of the chat's
//! latest call, and the web hook tells the host of its own call only after its verdict, so a block
//! held for a `WebFetch` is usually stamped with the call before it. The host is compared as
//! [`host_key`] says, one spelling for both sides. A deny with no such block (the proxy answered 502 or 503, a host
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

/// How much of a host two blocks compare: a target is clipped to [`stops::TARGET_CHARS`] as it is
/// written, and clips differently with a port than without, so only a prefix every spelling of
/// the same host keeps whole is compared.
const MATCHED_CHARS: usize = 100;

/// **A host as both sides of a pair are compared**, from the target as a block holds it (the
/// deny's, written bare by the web judge, and the proxy's, `host:port` as the sandbox names it):
/// lower case, without a port, brackets, a trailing dot or the mark of a clipped target, an IPv6
/// address as Rust writes it, and no more than [`MATCHED_CHARS`] of it.
pub fn host_key(target: &str) -> String {
    let lowered = target.trim().trim_end_matches("...").to_ascii_lowercase();
    let name = if let Some(inner) = lowered.strip_prefix('[') {
        inner.split(']').next().unwrap_or(inner)
    } else if lowered.parse::<std::net::Ipv6Addr>().is_ok() {
        lowered.as_str()
    } else {
        match lowered.rsplit_once(':') {
            // `host:port`, or a port clipped away.
            Some((name, port)) if port.chars().all(|c| c.is_ascii_digit()) => name,
            _ => lowered.as_str(),
        }
    };
    let name = name.parse::<std::net::Ipv6Addr>().map_or_else(
        |_| name.trim_end_matches('.').to_owned(),
        |address| address.to_string(),
    );
    name.chars().take(MATCHED_CHARS).collect()
}

/// The host a block names, and whether it is the web judge's deny (`true`) or a plain sandbox
/// block the proxy could have written for a `WebFetch` (`false`); `None` for any other block.
fn side(block: &Block) -> Option<(bool, String)> {
    let Some(Target::Host(host)) = &block.target else {
        return None;
    };
    match block.source {
        // A guard's block is aimed at a host only by the web judge's deny.
        Source::Guard => Some((true, host_key(host.as_str()))),
        // The proxy's own, verified block: its tool is whatever call the chat made last, so it
        // says nothing of whether a `WebFetch` made the connection.
        Source::Sandbox if block.qualifier.is_none() => Some((false, host_key(host.as_str()))),
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

/// **How a log's web denies and sandbox blocks pair**, by the events' places in the log.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Pairs {
    /// Denies written after the block that holds them: they count for nothing.
    pub after_block: BTreeSet<usize>,
    /// Denies written before the block that holds them, by deny, with that block's place. The
    /// deny counted when it was written, in its own turn.
    pub before_block: BTreeMap<usize, usize>,
}

impl Pairs {
    /// Every deny a recorded block holds, whichever was written first.
    pub fn denies(&self) -> BTreeSet<usize> {
        self.after_block
            .iter()
            .chain(self.before_block.keys())
            .copied()
            .collect()
    }
}

/// **How the web judge's denies and the proxy's blocks of `events` pair.**
pub fn pairs(events: &[Event]) -> Pairs {
    let mut pairing = Pairing::default();
    let mut pairs = Pairs::default();
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
                pairs.after_block.insert(at);
            }
            Fed::Retires(deny) => {
                pairs.before_block.insert(deny, at);
            }
        }
    }
    pairs
}

/// **The events (by their place in `events`) that are a web judge's deny a recorded sandbox block
/// holds**: left out of every count and list of blocks, still in the log.
pub fn repeats(events: &[Event]) -> BTreeSet<usize> {
    pairs(events).denies()
}
