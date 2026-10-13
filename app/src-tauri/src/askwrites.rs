// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What an edit ask would write, as its Inbox row draws it** (#1146).
//!
//! The core reads a Claude Code edit's change from the tool's own input
//! (`purlis_core::harness::asked::edit`) and offers Allow only where it is shown whole. This is
//! its window side: each change as the merge view the comparison tab draws it in takes it, a
//! before, an after and the hunk between them, or the sentence that says why it is not shown,
//! with nothing that lets it run.

use purlis_core::harness::model::{Action, Ask, Unshown, Writes, Written};

use crate::piecefiles::GitHunk;

/// What an edit ask would write, as its row draws it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum WritesShown {
    /// Every change, whole, in the order the tool makes them, to the file at `path`.
    Whole {
        path: String,
        changes: Vec<ChangeShown>,
    },
    /// Not shown, and why, in a sentence: no Allow is offered, and the pane answers it.
    NotShown { why: String },
}

/// One change, as the merge view draws it: `before` on the left, `after` on the right, and the
/// one hunk between them, which is the whole of each.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ChangeShown {
    /// What the change does, in purlis's fixed words.
    pub says: String,
    pub before: String,
    pub after: String,
    pub hunks: Vec<GitHunk>,
}

/// What a change that writes the whole file says.
pub const WRITES_THE_FILE: &str = "Writes the whole file, whatever it holds now";
/// What a change that replaces a text once says.
pub const REPLACES: &str = "Replaces this text";
/// What a change that replaces a text wherever it appears says.
pub const REPLACES_EVERY: &str = "Replaces this text everywhere it appears";
/// What follows the reason an edit is not shown.
pub const IN_ITS_PANE: &str = "Its pane shows it whole: allow it there.";

/// `ask`'s change as its row draws it, or none for an ask that is not an edit with one.
pub fn of(ask: &Ask) -> Option<WritesShown> {
    let writes = ask.writes.as_ref()?;
    let Action::Edit { path } = &ask.action else {
        return None;
    };
    Some(match writes {
        Writes::Whole { changes } => WritesShown::Whole {
            path: path.clone(),
            changes: changes.iter().map(shown).collect(),
        },
        Writes::NotShown { why } => WritesShown::NotShown {
            why: format!("{} {IN_ITS_PANE}", why_said(*why)),
        },
    })
}

/// Why a change is not shown, in a sentence.
pub fn why_said(why: Unshown) -> String {
    match why {
        Unshown::TooBig => format!(
            "This change is longer than the {} lines the Inbox shows.",
            purlis_core::harness::asked::edit::MOST_LINES
        ),
        Unshown::Binary => "This change is not text.".to_owned(),
        Unshown::DrawnOtherwise => {
            "This change holds characters the Inbox cannot show as they are, such as a \
             carriage return."
                .to_owned()
        }
        Unshown::Credential => {
            "This change holds what looks like a credential, which the Inbox does not show."
                .to_owned()
        }
        Unshown::Unread => "The Inbox cannot read this change whole.".to_owned(),
    }
}

fn shown(change: &Written) -> ChangeShown {
    let (says, before, after) = match change {
        Written::File { content } => (WRITES_THE_FILE, "", content.as_str()),
        Written::Replace {
            before,
            after,
            every,
        } => (
            if *every { REPLACES_EVERY } else { REPLACES },
            before.as_str(),
            after.as_str(),
        ),
    };
    ChangeShown {
        says: says.to_owned(),
        before: before.to_owned(),
        after: after.to_owned(),
        hunks: vec![whole(before, after)],
    }
}

/// The one hunk that is the whole of `before` and of `after`, in git's numbers: a side with no
/// lines names line 0, the top, as the hunk comes after it.
fn whole(before: &str, after: &str) -> GitHunk {
    let lines = |text: &str| u32::try_from(text.lines().count()).unwrap_or(u32::MAX);
    let (old_lines, new_lines) = (lines(before), lines(after));
    GitHunk {
        old_start: u32::from(old_lines > 0),
        old_lines,
        new_start: u32::from(new_lines > 0),
        new_lines,
    }
}

#[cfg(test)]
#[path = "askwrites_tests.rs"]
mod tests;
