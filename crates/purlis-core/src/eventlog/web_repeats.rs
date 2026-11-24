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
//! [`host_key`] says, one spelling for both sides. A deny with no such block (the proxy answered
//! 502 or 503, a host kept blocked that the proxy refuses without writing again, a refusal still
//! an open ask) counts as the refusal it is, and counts the same once the block is written, so a
//! fetch is counted at most once, and at least once except in one bounded case.
//!
//! **A pair is one refusal, aimed at the deny's own call.** A block is written late by
//! `became_a_block` (a refusal held as an ask that timed out), with the tool and arguments of the
//! chat's latest call when it left, which may be a later command than the fetch; so what a pair is
//! resolved by, in a turn's finish, is the deny's own tool and arguments, and only the block's
//! qualifier and source are read from the block ([`super::finishes`]).
//!
//! **The bounded undercount**: a plain block says nothing of its tool, so a `Bash` command's own
//! refusal of host X, written within [`BEFORE_MS`] before a `WebFetch` deny of X, absorbs that
//! deny, and two refusals count as one. It costs at most one count per plain block, never two for
//! one refusal and never more than the block's own, and only for the same chat and host in the
//! window.
//!
//! **Known residual**: a host `url` accepts that [`stops::Target::host`] refuses (`~ ! * + , ; =`
//! in a name) gets no target on either side, so nothing pairs and its refusal counts twice.
//!
//! [`Pairing`] is fed a log's blocks in order, whether read whole ([`repeats`]) or delivered one
//! by one ([`super::chatlog::Blocks`]), and answers in either order of the two.

use std::collections::{BTreeMap, BTreeSet};

use super::Event;
use super::stops::{self, Block, Source, Target};

/// How long before a deny the proxy's block may have been written: the proxy writes its block as
/// it answers, and the hook's deny follows by the hook's own latency (a loopback round trip and
/// the host's write), not by the proxy's wait, so a few seconds are all it takes. Kept short
/// because a longer window lets another call's plain block of the host absorb the deny (see the
/// bounded undercount in the module doc).
const BEFORE_MS: u64 = 15_000;

/// How long after a deny the proxy's block may be written: a refusal held as an ask is written as
/// a block when its ask leaves (`became_a_block`, in the app's `chats.rs`, the late writer, with
/// [`crate::sandboxblock::stop`]), up to a quiet person's hold (15 minutes) and a little more.
const AFTER_MS: u64 = 20 * 60_000;

/// The most unpaired blocks of each side held per chat: past it the oldest are let go, which
/// only means that a block that old pairs with nothing and counts.
const HELD: usize = 64;

/// What a block's feed says of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fed<K> {
    /// It counts: a block of any other kind, a deny no recorded block holds, or a sandbox block.
    Counts,
    /// It is a deny the earlier sandbox block `K` already holds: it counts for nothing.
    Repeat(K),
    /// A sandbox block that paired with the earlier deny `K`, which counted when it was fed and
    /// counts no more.
    Retires(K),
}

/// A deny of a `WebFetch` or a sandbox block for a host, waiting for its pair.
#[derive(Debug)]
struct Waiting<K> {
    key: K,
    at_ms: u64,
    host: HostKey,
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

/// **A host as both sides of a pair are compared**: its whole name, and whether the target it was
/// read from was clipped (a block's target is clipped to [`stops::TARGET_CHARS`] as it is written,
/// differently with a port than without; the deny's never is).
#[derive(Debug, PartialEq, Eq)]
pub struct HostKey {
    name: String,
    clipped: bool,
}

impl HostKey {
    /// Whether both name one host: the whole names are equal, or, where a side was clipped, the
    /// clipped one is the beginning of the other.
    fn is(&self, other: &Self) -> bool {
        if self.clipped || other.clipped {
            self.name.starts_with(&other.name) || other.name.starts_with(&self.name)
        } else {
            self.name == other.name
        }
    }
}

/// **A host as both sides of a pair are compared**, from the target as a block holds it (the
/// deny's, written bare by the web judge, and the proxy's, `host:port` as the sandbox names it):
/// lower case, without a port, brackets, a trailing dot or the mark of a clipped target, and an
/// IPv6 address as Rust writes it.
pub fn host_key(target: &str) -> HostKey {
    let trimmed = target.trim();
    let clipped = trimmed.ends_with("...");
    let lowered = trimmed.trim_end_matches("...").to_ascii_lowercase();
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
    HostKey { name, clipped }
}

/// The host a block names, and whether it is the web judge's deny (`true`) or a plain sandbox
/// block the proxy could have written for a `WebFetch` (`false`); `None` for any other block.
fn side(block: &Block) -> Option<(bool, HostKey)> {
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
                .filter(|(_, one)| one.host.is(&waiting.host) && within(one.at_ms, at_ms))
                .min_by_key(|(_, one)| one.at_ms.abs_diff(at_ms))
                .map(|(at, _)| at);
            if let Some(at) = nearest {
                return Fed::Repeat(held.sandbox.remove(at).key);
            }
            held.denies.push(waiting);
            let_go_of_the_oldest(&mut held.denies);
            Fed::Counts
        } else {
            // The earliest deny of this host waiting for a block.
            let earliest = held
                .denies
                .iter()
                .position(|one| one.host.is(&waiting.host) && within(at_ms, one.at_ms));
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
    /// Denies written after the block that holds them, by deny, with that block's place: they
    /// count for nothing.
    pub after_block: BTreeMap<usize, usize>,
    /// Denies written before the block that holds them, by deny, with that block's place. The
    /// deny counted when it was written, in its own turn.
    pub before_block: BTreeMap<usize, usize>,
}

impl Pairs {
    /// Every deny a recorded block holds, whichever was written first.
    pub fn denies(&self) -> BTreeSet<usize> {
        self.after_block
            .keys()
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
            Fed::Repeat(block) => {
                pairs.after_block.insert(at, block);
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
