// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Which chats wait on which, by their dispatch records** (ADR 0121 decision 5, #6006): what
//! Asks reads to say an ask "blocks 3 chats".
//!
//! A chat that dispatched a task and owes nobody yet but waits on its report is blocked by that
//! task, and so is every chat above it that waits on its own report in turn: a dispatched chain
//! stops at the first link that waits on the person. So a chat blocks every open chat that
//! waits on its report, directly or through a chain of them.
//!
//! The facts are the app's own record of each chat ([`crate::reopen::HandedFrom`]): which chat
//! dispatched it, and whether that chat still waits on its report. Nothing a chat says of
//! itself is read. A chat that is not open waits on nothing, so it is never counted.

use std::collections::{HashMap, HashSet};

/// One open chat that waits on another's report: `task` owes `asker` a report, and `asker`
/// is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaitsOn {
    pub task: u32,
    pub asker: u32,
}

/// **How many chats each chat blocks**: for each chat that some open chat waits on, the number
/// of distinct chats that wait on it, directly or up a chain. A chat no one waits on is absent.
/// A loop in the records (which the app never writes) counts each chat once and never itself.
pub fn blocked_by(waits: &[WaitsOn]) -> HashMap<u32, u32> {
    let up: HashMap<u32, u32> = waits.iter().map(|one| (one.task, one.asker)).collect();
    let mut blocks = HashMap::new();
    for &task in up.keys() {
        let mut seen = HashSet::from([task]);
        let mut at = task;
        while let Some(&asker) = up.get(&at) {
            if !seen.insert(asker) {
                break;
            }
            at = asker;
        }
        // Every chat met above it, but itself.
        blocks.insert(task, u32::try_from(seen.len() - 1).unwrap_or(u32::MAX));
    }
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    fn on(task: u32, asker: u32) -> WaitsOn {
        WaitsOn { task, asker }
    }

    #[test]
    fn a_chat_blocks_every_chat_up_its_chain_that_waits_on_a_report() {
        let blocks = blocked_by(&[on(3, 2), on(2, 1)]);
        assert_eq!(blocks.get(&3), Some(&2));
        assert_eq!(blocks.get(&2), Some(&1));
        assert_eq!(
            blocks.get(&1),
            None,
            "nobody waits on the chat the person started"
        );
    }

    #[test]
    fn siblings_block_their_asker_and_not_each_other() {
        let blocks = blocked_by(&[on(2, 1), on(3, 1)]);
        assert_eq!(blocks.get(&2), Some(&1));
        assert_eq!(blocks.get(&3), Some(&1));
    }

    #[test]
    fn no_waits_block_nothing() {
        assert!(blocked_by(&[]).is_empty());
    }

    #[test]
    fn a_loop_in_the_records_counts_each_chat_once_and_never_itself() {
        let blocks = blocked_by(&[on(1, 2), on(2, 1)]);
        assert_eq!(blocks.get(&1), Some(&1));
        assert_eq!(blocks.get(&2), Some(&1));
    }
}
