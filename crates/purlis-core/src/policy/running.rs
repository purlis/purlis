// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **How a running chat picks up a change** (M75 slice 23, #7032; spec §9.5, ADR 0127).
//!
//! **Hook and permission decisions always use the current policy** (the person's ruling of
//! 2026-11-25, H1; ADR 0127 as amended that day). Only the shell sandbox, compiled at start, waits
//! for a restart.
//!
//! **A tightening reaches a chat at its next decision with nothing here:** every decision the host
//! makes reads the project at `HEAD` and the sealed store afresh ([`super::force::in_force`]), and
//! seals what it sees tighten into the accepted floor on the way. **A loosening reaches it at its
//! next decision too, once the person has pressed Accept** (the accepted copy only moves at the
//! press, and the policy in force is the stricter of the committed and the accepted side), and a
//! chat that starts after the press is held to the same. So no chat is pinned to the copy its
//! start accepted, and none is built.
//!
//! What a decision cannot reach is the sandbox its shell commands run in, **compiled at the
//! chat's start** ([`super::sandbox_rules::compile`]). So the app keeps, for each chat, what that
//! compile was made from ([`AtStart`], taken before the sandbox is applied) and asks again
//! ([`Shared`], read once for all chats) whenever the plane changes, its policy is committed, the
//! sealed store is written or a press is made: a chat is **out of date** ([`Why`]), in the sense
//! **Plane updated** gives the word, and offers a restart, when
//!
//! - a **Never** reaches it that did not at its start,
//! - a grant of `read` or `write` that served it at its start is **removed**,
//! - a **forbid** is in force that was not, or a **permit** that let a place through is gone or
//!   narrower: both compared by the places the rules **resolve to** over the plane's layout
//!   ([`super::sandbox_rules::ActionRules::forbidden`] and `allowed`, the permits intersected
//!   across the sides), never by their text, since a `PathSet`, `Repo::` or `Branch::` term keeps
//!   its text while its reach changes (a read is allowed only where a permit allows it on Seatbelt
//!   and Landlock, spec §11.2 and §11.5),
//! - the plane **entered the deny state**, or its Everywhere answers became unreadable,
//! - a **workspace was created** after it started (a path the new workspace owns resolved to
//!   none when the sandbox was compiled), or
//! - the **app restarted** (the chat was reattached: it was started by an app that is gone, whose
//!   listener no longer answers it).
//!
//! **Only in the direction that tightens** (for the sandbox). A forbid, a never or a workspace that
//! is gone, a grant that is new, and a press that loosens all leave the chat's sandbox as it was.
//! What is compared is in memory only and holds paths, so none of it is recorded (the start
//! event's [`super::sandbox_rules::Record`] is shapes only).

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::path::Path;

use super::decide::with_sources;
use super::resolver::{self, Layout};
use super::sandbox_rules::Reach;
use super::{force, grants, sandbox_rules};
use crate::secrets::answers;

/// The commit the plane root's `HEAD` names now: what the app compares to tell that the policy a
/// chat's sandbox was compiled under may have moved (a commit, a pull, a checkout).
pub fn head_of(root: &Path) -> Option<String> {
    force::head_sha(root)
}

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
    /// A forbid on a Path (or another sandbox forbid) is now in force, or a permit that let a
    /// place through is gone or narrower.
    PathTightened,
    /// The plane is in its deny state, which it was not.
    DenyState,
    /// The Everywhere answers can no longer be read, which they could.
    AnswersUnreadable,
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
            Why::DenyState => "the policy is in its deny state",
            Why::AnswersUnreadable => "your answers for everywhere cannot be read",
            Why::WorkspaceAdded => "a workspace was created",
            Why::AppRestarted => "purlis restarted",
        }
    }
}

/// One action's places: what its forbids hold (the union across the sides, the stricter winning)
/// and what its permits let through (the intersection across the sides), or `None` for the
/// intersection that cannot be written as places, which stands as "unknown" and fails closed.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct Places {
    forbids: Vec<Reach>,
    permits: Option<Vec<Reach>>,
}

impl Places {
    fn of<'a>(sides: impl Iterator<Item = &'a force::ActionPlaces>) -> Self {
        let mut forbids = Vec::new();
        let mut permits = Some(vec![Reach::Everything]);
        for side in sides {
            forbids.extend(side.forbids.iter().cloned());
            permits = permits.and_then(|held| meet_all(&held, &side.permits));
        }
        Self { forbids, permits }
    }

    /// Whether `self`, read now, tightens what `then` held: a forbid not held by `then`, or a
    /// place `then` let through that `self` no longer does.
    fn tightens(&self, then: &Places) -> bool {
        let forbids = self.forbids.iter().any(|now| !covered(now, &then.forbids));
        let permits = match (&then.permits, &self.permits) {
            (Some(then), Some(now)) => then.iter().any(|place| !covered(place, now)),
            (Some(_), None) => true,
            (None, _) => false,
        };
        forbids || permits
    }
}

/// Whether `reach` lies wholly inside the union of `by`. **Only clear containment counts**, so a
/// reach this cannot place stands as not covered, which marks more than it must, never less.
fn covered(reach: &Reach, by: &[Reach]) -> bool {
    by.iter().any(|one| match (reach, one) {
        (_, Reach::Everything) => true,
        (reach, one) if reach == one => true,
        (Reach::Entry(entry), one) => one.covers(entry),
        (Reach::Folder(inner), Reach::Folder(outer)) => resolver::within(inner, outer),
        (Reach::Folder(inner), Reach::Inside(outer)) => {
            inner != outer && resolver::within(inner, outer)
        }
        (Reach::Inside(inner), Reach::Folder(outer) | Reach::Inside(outer)) => {
            resolver::within(inner, outer)
        }
        (Reach::Unowned(of), Reach::Unowned(by)) => by
            .iter()
            .all(|folder| of.iter().any(|own| resolver::within(folder, own))),
        _ => false,
    })
}

/// The places both `a` and `b` hold, or `None` where they cannot be written as places.
fn meet_all(a: &[Reach], b: &[Reach]) -> Option<Vec<Reach>> {
    let mut met = Vec::new();
    for (one, other) in a
        .iter()
        .flat_map(|one| b.iter().map(move |other| (one, other)))
    {
        met.extend(meet(one, other)?);
    }
    met.sort_by_key(|place| format!("{place:?}"));
    met.dedup();
    Some(met)
}

/// What two places hold together: nothing (`Some(None)`), one place, or what cannot be written as
/// a place (`None`): a folder that holds a workspace's folder, met with what no workspace owns.
fn meet(a: &Reach, b: &Reach) -> Option<Option<Reach>> {
    use Reach::{Entry, Everything, Folder, Inside, Unowned};
    Some(match (a, b) {
        (Everything, place) | (place, Everything) => Some(place.clone()),
        (Entry(entry), place) | (place, Entry(entry)) => {
            place.covers(entry).then(|| Entry(entry.clone()))
        }
        (Folder(f), Folder(g)) => {
            if resolver::within(f, g) {
                Some(Folder(f.clone()))
            } else {
                resolver::within(g, f).then(|| Folder(g.clone()))
            }
        }
        (Folder(f), Inside(g)) | (Inside(g), Folder(f)) => {
            if resolver::within(g, f) {
                Some(Inside(g.clone()))
            } else {
                (f != g && resolver::within(f, g)).then(|| Folder(f.clone()))
            }
        }
        (Inside(g), Inside(h)) => {
            if resolver::within(g, h) {
                Some(Inside(g.clone()))
            } else {
                resolver::within(h, g).then(|| Inside(h.clone()))
            }
        }
        (Unowned(f), Unowned(g)) => {
            let mut both: Vec<String> = f.iter().chain(g).cloned().collect();
            both.sort();
            both.dedup();
            Some(Unowned(both))
        }
        (Folder(inner), Unowned(owned)) | (Unowned(owned), Folder(inner)) => {
            if owned.iter().any(|own| resolver::within(inner, own)) {
                None
            } else if owned.iter().any(|own| resolver::within(own, inner)) {
                return None;
            } else {
                Some(Folder(inner.clone()))
            }
        }
        (Inside(inner), Unowned(owned)) | (Unowned(owned), Inside(inner)) => {
            if owned.iter().any(|own| resolver::within(inner, own)) {
                None
            } else if owned.iter().any(|own| resolver::within(own, inner)) {
                return None;
            } else {
                Some(Inside(inner.clone()))
            }
        }
    })
}

/// The reading of the policy and the sealed answers one compile is made from.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct Reading {
    read: Places,
    write: Places,
    nevers: BTreeSet<String>,
    grants: BTreeSet<String>,
    workspaces: BTreeSet<String>,
    deny: bool,
    answers_unreadable: bool,
}

/// **The parts of a reading every chat shares**, read once: the sealed store, each side of the
/// policy as a decision reads it (the project at `HEAD`, the accepted copy, the floor) and the
/// plane's layout. What differs by chat is compiled from them ([`AtStart::out_of_date`]), and a
/// chat of the facts of one before it is not compiled again.
pub struct Shared {
    standing: answers::Standing,
    sides: Vec<force::LimitSide>,
    layout: Layout,
    workspaces: BTreeSet<String>,
    machine: Option<String>,
    memo: RefCell<Vec<(Facts, Reading)>>,
}

impl Shared {
    /// The plane at `root` read now.
    pub fn read(root: &Path) -> Self {
        let standing = answers::standing(root);
        let (sides, workspaces) = with_sources(root, |sources| {
            (
                force::decided_sides(sources, &standing),
                sources
                    .workspaces
                    .iter()
                    .map(|name| (*name).to_owned())
                    .collect::<Vec<_>>(),
            )
        });
        // A version records no clones (as `force`'s layout has none), so a path is in a `Repo`
        // here only as a branch folder of `.worktrees/<repo>`.
        let layout = Layout {
            root: root.to_string_lossy().into_owned(),
            workspaces: workspaces.clone(),
            clones: Vec::new(),
        };
        Self {
            standing,
            sides,
            layout,
            workspaces: workspaces.into_iter().collect(),
            machine: answers::context_machine(),
            memo: RefCell::new(Vec::new()),
        }
    }

    fn reading(&self, facts: &Facts) -> Reading {
        if let Some((_, reading)) = self.memo.borrow().iter().find(|(held, _)| held == facts) {
            return reading.clone();
        }
        let (nevers, grants) = grants::held_by(
            &self.standing,
            facts.workspace.as_deref(),
            facts.chat.as_deref(),
        );
        let chat = sandbox_rules::Chat {
            chat: facts.chat.as_deref().unwrap_or_default(),
            workspace: facts.workspace.as_deref().unwrap_or_default(),
            persona: facts.persona.as_deref().unwrap_or_default(),
            sandbox: &facts.sandbox,
            machine: self.machine.as_deref(),
            depth: 0,
        };
        let places = force::compiled_places(&self.sides, &self.layout, &chat);
        let reading = Reading {
            read: Places::of(places.iter().map(|side| &side.read)),
            write: Places::of(places.iter().map(|side| &side.write)),
            nevers,
            grants,
            workspaces: self.workspaces.clone(),
            deny: self.standing.is_deny(),
            answers_unreadable: self.standing.everywhere_unreadable,
        };
        self.memo
            .borrow_mut()
            .push((facts.clone(), reading.clone()));
        reading
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
        let reading = Shared::read(root).reading(&facts);
        Self {
            facts,
            reading,
            reattached,
        }
    }

    /// **Why this chat is out of date, in the order of [`Why`]; empty when it is not.** Only a
    /// change that tightens counts: see the module's words.
    pub fn out_of_date(&self, shared: &Shared) -> Vec<Why> {
        let then = &self.reading;
        let now = shared.reading(&self.facts);
        let mut why = Vec::new();
        if !now.nevers.is_subset(&then.nevers) {
            why.push(Why::Never);
        }
        if !then.grants.is_subset(&now.grants) {
            why.push(Why::GrantRemoved);
        }
        if now.read.tightens(&then.read) || now.write.tightens(&then.write) {
            why.push(Why::PathTightened);
        }
        if now.deny && !then.deny {
            why.push(Why::DenyState);
        }
        if now.answers_unreadable && !then.answers_unreadable {
            why.push(Why::AnswersUnreadable);
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
