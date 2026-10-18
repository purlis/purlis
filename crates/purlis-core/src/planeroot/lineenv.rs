// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The environment each git on a line may run with, shared between them (#2648).
//!
//! Every git on a line inherits everything the line exported and assigned before it. Copying
//! that into each git made a line that alternates exports and gits cost its square. Here the
//! line's exports and assignments are frozen into chains of shared nodes as the walk meets
//! them, each entry once, and a git's environment is a handful of pointers into them. What the
//! walk reads of every git (the directory variables and a `core.worktree` key) is found through
//! skip pointers, past the entries that are not one; the rest is read only where a reader asks
//! for the whole list, as an alias does.

use std::sync::Arc;

use super::GIT_DIR_ENV;

/// Whether the walk reads this assignment for every git: `GIT_DIR` or `GIT_WORK_TREE`
/// ([`super::git_target`], [`super::invocation_readings`]), or any word that spells
/// `core.worktree` ([`super::pointed_by_name`]). Nothing else in a git's environment changes
/// where it runs.
fn read_by_the_walk(assign: &str) -> bool {
    assign.to_ascii_lowercase().contains("core.worktree")
        || GIT_DIR_ENV
            .iter()
            .any(|(name, _)| assign.split_once('=').is_some_and(|(n, _)| n == *name))
}

/// Entries added to a chain at once, after those of the node before.
#[derive(Debug)]
struct Node {
    entries: Vec<String>,
    prev: Option<Arc<Node>>,
    /// How many entries the chain holds up to and with this node.
    len: usize,
    /// Whether one of this node's own entries is [`read_by_the_walk`].
    read: bool,
    /// The nearest node before this one with such an entry: the walk skips the rest.
    read_before: Option<Arc<Node>>,
}

impl Drop for Node {
    /// Let go of the nodes before this one in a loop, not a call each: a line of tens of
    /// thousands of exports makes a chain as long, and dropping it node by node recursively
    /// overflowed the stack.
    fn drop(&mut self) {
        let mut pending: Vec<Arc<Node>> = [self.prev.take(), self.read_before.take()]
            .into_iter()
            .flatten()
            .collect();
        while let Some(node) = pending.pop() {
            if let Some(mut node) = Arc::into_inner(node) {
                pending.extend(node.prev.take());
                pending.extend(node.read_before.take());
            }
        }
    }
}

/// An append-only list of assignments whose every prefix is kept as cheaply as a pointer.
#[derive(Clone, Debug, Default)]
pub(super) struct Chain(Option<Arc<Node>>);

impl Chain {
    fn len(&self) -> usize {
        self.0.as_ref().map_or(0, |node| node.len)
    }

    /// The chain as `all` is now: `all` only ever grew from what the chain last held, and what
    /// it gained since is copied in once.
    pub(super) fn freeze(&mut self, all: &[String]) -> Chain {
        let held = self.len();
        if all.len() > held {
            let entries = all[held..].to_vec();
            let read = entries.iter().any(|assign| read_by_the_walk(assign));
            let read_before = self.0.as_ref().and_then(|node| {
                if node.read {
                    Some(node.clone())
                } else {
                    node.read_before.clone()
                }
            });
            self.0 = Some(Arc::new(Node {
                entries,
                prev: self.0.take(),
                len: all.len(),
                read,
                read_before,
            }));
        }
        self.clone()
    }

    /// The nodes from the first to the last.
    fn nodes(&self) -> Vec<&Node> {
        let mut out = Vec::new();
        let mut at = self.0.as_deref();
        while let Some(node) = at {
            out.push(node);
            at = node.prev.as_deref();
        }
        out.reverse();
        out
    }

    /// The nodes with an entry the walk reads, from the first to the last.
    fn read_nodes(&self) -> Vec<&Node> {
        let mut out = Vec::new();
        let mut at = self.0.as_deref().and_then(|node| {
            if node.read {
                Some(node)
            } else {
                node.read_before.as_deref()
            }
        });
        while let Some(node) = at {
            out.push(node);
            at = node.read_before.as_deref();
        }
        out.reverse();
        out
    }

    /// Every entry, the last first, read only as far as the caller goes.
    fn newest_first(&self) -> impl Iterator<Item = &String> {
        std::iter::successors(self.0.as_deref(), |node| node.prev.as_deref())
            .flat_map(|node| node.entries.iter().rev())
    }
}

/// The environment one git invocation may run with as the line sets it, in order: the parts
/// the walk joined for it, each a prefix of a chain the line shares
/// ([`super::RootInvocation::env`]).
#[derive(Clone, Default)]
pub struct LineEnv {
    parts: Vec<Chain>,
}

impl LineEnv {
    /// The parts, in order.
    pub(super) fn of(parts: Vec<Chain>) -> Self {
        LineEnv { parts }
    }

    /// Every assignment, in order. As long as the line: read only where a reader needs all of
    /// it.
    pub fn to_vec(&self) -> Vec<String> {
        self.parts
            .iter()
            .flat_map(|part| part.nodes())
            .flat_map(|node| node.entries.iter().cloned())
            .collect()
    }

    /// The assignments the walk reads for every git ([`read_by_the_walk`]), in order: as long as
    /// those are, not as the line.
    pub(super) fn read_by_the_walk(&self) -> Vec<String> {
        self.parts
            .iter()
            .flat_map(|part| part.read_nodes())
            .flat_map(|node| node.entries.iter())
            .filter(|assign| read_by_the_walk(assign))
            .cloned()
            .collect()
    }

    /// Every assignment, the last first, read only as far as the caller goes: for a question
    /// whose answer does not depend on the order, which a recent entry often settles.
    pub fn newest_first(&self) -> impl Iterator<Item = &String> {
        self.parts.iter().rev().flat_map(|part| part.newest_first())
    }
}

impl From<Vec<String>> for LineEnv {
    fn from(entries: Vec<String>) -> Self {
        let mut chain = Chain::default();
        LineEnv {
            parts: vec![chain.freeze(&entries)],
        }
    }
}

impl PartialEq for LineEnv {
    fn eq(&self, other: &Self) -> bool {
        self.to_vec() == other.to_vec()
    }
}

impl Eq for LineEnv {}

impl std::fmt::Debug for LineEnv {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.to_vec()).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(words: &[&str]) -> Vec<String> {
        words.iter().map(|w| w.to_string()).collect()
    }

    #[test]
    fn a_frozen_prefix_keeps_what_the_list_held_then() {
        let mut all = strings(&["A=1"]);
        let mut chain = Chain::default();
        let first = chain.freeze(&all);
        all.extend(strings(&["GIT_DIR=/x", "B=2"]));
        let second = chain.freeze(&all);
        all.push("C=3".into());
        assert_eq!(LineEnv::of(vec![first]).to_vec(), strings(&["A=1"]));
        assert_eq!(
            LineEnv::of(vec![second.clone()]).to_vec(),
            strings(&["A=1", "GIT_DIR=/x", "B=2"])
        );
        let own = Chain::default().freeze(&strings(&["GIT_WORK_TREE=/w"]));
        let env = LineEnv::of(vec![second, own]);
        assert_eq!(
            env.newest_first().cloned().collect::<Vec<_>>(),
            strings(&["GIT_WORK_TREE=/w", "B=2", "GIT_DIR=/x", "A=1"])
        );
    }

    /// A chain as long as a line of single exports, each frozen on its own, is let go of
    /// without a call per node: dropped recursively it overflowed the stack (#2648). None of
    /// its entries is one the walk reads, so no skip pointer breaks the run of nodes.
    #[test]
    fn a_chain_of_many_nodes_is_dropped_without_overflowing_the_stack() {
        let mut all = Vec::new();
        let mut chain = Chain::default();
        for i in 0..200_000 {
            all.push(format!("A{i}=1"));
            chain.freeze(&all);
        }
        let env = LineEnv::of(vec![chain.clone()]);
        assert!(env.read_by_the_walk().is_empty());
        drop(chain);
        drop(env);
    }

    #[test]
    fn the_walk_reads_every_directory_variable_and_worktree_key_in_order() {
        let mut all = Vec::new();
        let mut chain = Chain::default();
        for word in [
            "GIT_DIR=/a",
            "A=1",
            "GIT_DIR_X=/no",
            "B=2",
            "GIT_CONFIG_PARAMETERS='core.worktree=/t'",
            "C=3",
            "GIT_WORK_TREE=/w",
            "XGIT_DIR=/no",
        ] {
            all.push(word.to_string());
            chain.freeze(&all);
        }
        let own = Chain::default().freeze(&strings(&["GIT_DIR=/own"]));
        let env = LineEnv::of(vec![chain.clone(), Chain::default(), own]);
        assert_eq!(
            env.read_by_the_walk(),
            strings(&[
                "GIT_DIR=/a",
                "GIT_CONFIG_PARAMETERS='core.worktree=/t'",
                "GIT_WORK_TREE=/w",
                "GIT_DIR=/own"
            ])
        );
        let mut whole = all.clone();
        whole.push("GIT_DIR=/own".into());
        assert_eq!(env.to_vec(), whole);
    }
}
