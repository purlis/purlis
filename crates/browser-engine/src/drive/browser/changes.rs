// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What changed on a page since it was last read (#2137): the lines of its tree that are new or
//! gone, so the agent need not take a whole snapshot after every step.

use std::collections::HashMap;
use std::sync::Mutex;

use super::{Answer, Browser, Refusal, State, View, lock, refer_in};
use crate::drive::tree::Seen;

/// The most pages whose last read is remembered; past it every one is forgotten, and the next
/// changes of each answer its first lines.
const PAGES_KEPT: usize = 64;

/// Each page's tree as it was last read, by its target: the document it showed and its lines.
#[derive(Default)]
pub(super) struct ReadLast(Mutex<HashMap<String, (u64, Vec<Seen>)>>);

impl ReadLast {
    /// Remembers `seen` as `target`'s tree, read from its document `document`.
    pub(super) fn remember(&self, target: &str, document: Option<u64>, seen: Vec<Seen>) {
        let mut pages = lock(&self.0);
        let Some(document) = document else {
            pages.remove(target);
            return;
        };
        if pages.len() >= PAGES_KEPT && !pages.contains_key(target) {
            pages.clear();
        }
        pages.insert(target.to_owned(), (document, seen));
    }

    fn last(&self, target: &str) -> Option<(u64, Vec<Seen>)> {
        lock(&self.0).get(target).cloned()
    }
}

/// The document `target`'s page shows now, as a number that changes when it loads another.
pub(super) fn document_of(state: &State, target: &str) -> Option<u64> {
    state
        .page(target)
        .and_then(|page| page.frames.get(target))
        .map(|frame| frame.generation)
}

impl Browser {
    /// The lines of `target`'s tree new or gone since it was last read; its first lines where
    /// it was not read before, or has loaded another document since.
    pub(super) fn changes(&self, target: &str) -> Result<Answer, Refusal> {
        let last = self.read_last.last(target);
        let most = self.inner.limits.snapshot_lines;
        let first_lines = View {
            within: None,
            from: 1,
            lines: most,
        };
        let earlier = |state: &State| {
            last.as_ref()
                .filter(|(document, _)| Some(*document) == document_of(state, target))
                .map(|(_, seen)| seen)
        };
        self.reading_shown(
            target,
            |snapshot, state| {
                Ok(match earlier(state) {
                    Some(seen) => snapshot.keys_new_since(seen, most),
                    None => snapshot.keys_in(&first_lines).unwrap_or_default(),
                })
            },
            |snapshot, state| {
                let before = earlier(state);
                let scrub = state
                    .page(target)
                    .map(|page| page.scrubbed.clone())
                    .unwrap_or_default();
                let mut refer = refer_in(state, target);
                Ok(match before {
                    Some(seen) => snapshot.changes(seen, most, &scrub, &mut refer),
                    None => format!(
                        "This page was not read before, or has loaded another document since: \
                         its first lines.\n{}",
                        snapshot.page(&first_lines, &mut refer).unwrap_or_default()
                    ),
                })
            },
        )
    }
}
