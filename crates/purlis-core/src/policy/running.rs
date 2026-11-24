// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **How a running chat picks up a change** (M75 slice 23, #7032; spec §9.5, ADR 0125 §3).
//!
//! **A tightening reaches a chat at its next decision with nothing here:** every decision the host
//! makes reads the project at `HEAD` and the sealed store afresh ([`super::force::in_force`]), and
//! seals what it sees tighten into the accepted floor on the way. **A loosening reaches it only at
//! its next start**, because the accepted copy only moves at the person's press, and a start reads
//! the press's result: the policy in force is the stricter of the committed and the accepted side.
//!
//! What a decision cannot reach is the sandbox its shell commands run in, **compiled at the
//! chat's start** ([`super::sandbox_rules::compile`]). So the app keeps, for each chat, what that
//! compile was made from ([`AtStart`]) and asks again whenever the plane changes: a chat is **out
//! of date** ([`Why`]), in the sense **Plane updated** gives the word, and offers a restart, when
//!
//! - a **Never** reaches it that did not at its start,
//! - a grant of `read` or `write` that served it at its start is **removed**,
//! - a forbid on a **Path** (or any sandbox forbid) is in force that was not,
//! - a **workspace was created** after it started (a path the new workspace owns resolved to
//!   none when the sandbox was compiled), or
//! - the **app restarted** (the chat was reattached: it was started by an app that is gone, whose
//!   listener no longer answers it).
//!
//! **Only in the direction that tightens.** A forbid, a never or a workspace that is gone, a grant
//! that is new, and a press that loosens all leave the chat as it was: it runs on what it started
//! on until it is started again, so no change a person could be tricked into pressing reaches a
//! chat that is already running. What is compared is in memory only and holds paths, so none of
//! it is recorded (the start event's [`super::sandbox_rules::Record`] is shapes only).

use std::collections::BTreeSet;
use std::path::Path;

use super::decide::with_sources;
use super::{force, grants, sandbox_rules};
use crate::secrets::answers;

/// What a chat's sandbox is compiled for: the host's record of the chat, never a word of its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Facts {
    /// The chat's lasting id, which a This-chat grant is keyed on.
    pub chat: Option<String>,
    /// Its workspace, where it works in one.
    pub workspace: Option<String>,
    /// The persona it was started as.
    pub persona: Option<String>,
    /// The sandbox level its start applied: `off`, `floor` or `full`.
    pub sandbox: String,
}

/// **What an out-of-date chat is out of date for.**
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
pub enum Why {
    /// A Never now reaches the chat.
    Never,
    /// A grant of `read` or `write` that served the chat is gone.
    GrantRemoved,
    /// A forbid on a Path (or another sandbox forbid) is now in force.
    PathTightened,
    /// A workspace was created after the chat started.
    WorkspaceAdded,
    /// The app that started the chat has restarted.
    AppRestarted,
}

impl Why {
    /// What the chat's mark says, as a short phrase.
    pub fn said(self) -> &'static str {
        match self {
            Why::Never => "a Never was given",
            Why::GrantRemoved => "a grant was removed",
            Why::PathTightened => "a path rule tightened",
            Why::WorkspaceAdded => "a workspace was created",
            Why::AppRestarted => "purlis restarted",
        }
    }
}

/// The reading of the policy and the sealed answers one compile is made from.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct Reading {
    forbids: BTreeSet<String>,
    nevers: BTreeSet<String>,
    grants: BTreeSet<String>,
    workspaces: BTreeSet<String>,
}

impl Reading {
    /// The plane at `root` read now for a chat of `facts`.
    fn of(root: &Path, facts: &Facts) -> Self {
        let standing = answers::standing(root);
        let (nevers, grants) =
            grants::held_by(&standing, facts.workspace.as_deref(), facts.chat.as_deref());
        let machine = answers::context_machine();
        let chat = sandbox_rules::Chat {
            chat: facts.chat.as_deref().unwrap_or_default(),
            workspace: facts.workspace.as_deref().unwrap_or_default(),
            persona: facts.persona.as_deref().unwrap_or_default(),
            sandbox: &facts.sandbox,
            machine: machine.as_deref(),
            depth: 0,
        };
        let forbids = with_sources(root, |sources| {
            force::compiled_forbids(sources, &standing, &chat)
        });
        let workspaces = crate::workspaces::Plane::open(root)
            .workspaces()
            .unwrap_or_default()
            .into_iter()
            .collect();
        Self {
            forbids,
            nevers,
            grants,
            workspaces,
        }
    }
}

/// **What a chat started on** (see the module's words): kept by the app from its start until it
/// ends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtStart {
    facts: Facts,
    reading: Reading,
    reattached: bool,
}

impl AtStart {
    /// What the plane at `root` says for a chat of `facts` that is starting now. `reattached` is a
    /// chat the app found already running, started by an app that is gone.
    pub fn take(root: &Path, facts: Facts, reattached: bool) -> Self {
        let reading = Reading::of(root, &facts);
        Self {
            facts,
            reading,
            reattached,
        }
    }

    /// **Why this chat is out of date, in the order of [`Why`]; empty when it is not.** Only a
    /// change that tightens counts: see the module's words.
    pub fn out_of_date(&self, root: &Path) -> Vec<Why> {
        self.out_of_date_by(&Reading::of(root, &self.facts))
    }

    fn out_of_date_by(&self, now: &Reading) -> Vec<Why> {
        let then = &self.reading;
        let mut why = Vec::new();
        if !now.nevers.is_subset(&then.nevers) {
            why.push(Why::Never);
        }
        if !then.grants.is_subset(&now.grants) {
            why.push(Why::GrantRemoved);
        }
        if !now.forbids.is_subset(&then.forbids) {
            why.push(Why::PathTightened);
        }
        if !now.workspaces.is_subset(&then.workspaces) {
            why.push(Why::WorkspaceAdded);
        }
        if self.reattached {
            why.push(Why::AppRestarted);
        }
        why
    }
}

#[cfg(test)]
#[path = "running_tests.rs"]
mod tests;
