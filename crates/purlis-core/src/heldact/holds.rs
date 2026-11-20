// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The held acts the app holds**, until the person answers (ADR 0128 D8).
//!
//! In memory only. One ask per chat and key (a tag's name, a release's tag, a branch, a source);
//! [`AT_MOST_PER_CHAT`] of one chat's at most.

use std::sync::Mutex;

use super::{
    AT_MOST_PER_CHAT, Act, MOST_QUEUED, Prepared, Refused, Requested, Shown, World, locked, pinned,
    prepare, scope_of,
};
use crate::harness::asks::{Answerer, HumanScope};

/// **What became of an ask.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Asked {
    /// The person is asked: the ask's id, and whether it is a new row (`false`: the chat's ask
    /// for it waits already, and shows what was read now).
    Held { id: u64, new: bool },
    /// Not offered, for this reason. Nobody is asked.
    Refused(Refused),
    /// The chat has as many held acts waiting as one chat may.
    TooMany,
}

/// **One held act that waits.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Waiting {
    pub id: u64,
    pub session: u32,
    /// What the chat asked, pinned to what the ask read ([`pinned`]).
    pub requested: Requested,
    /// What will happen, and what the person is shown of it.
    pub prepared: Prepared,
    pub since: u64,
    /// The first press was made, and the facts it asked once more of are what is shown.
    pub asked_once_more: bool,
}

/// **What a press did.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pressed {
    /// The first press: the facts were read again and are as shown. Press once more to perform.
    /// It hands back what the second press must be of.
    AsksOnceMore(Box<Shown>),
    /// The act was performed.
    Done(Box<Performed>),
}

/// **What a performed act was.** The act is the one the person was shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Performed {
    pub act: Act,
    /// The human who pressed, which no chat can be.
    pub by: HumanScope,
    /// A fork, made: the app records it (ADR 0128 D4).
    pub forked: Option<super::Forked>,
}

/// Why a press performed nothing.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NotDone {
    /// No such ask waits: answered already, put away, or its chat gone.
    #[error("this is not waiting any more, so nothing was done")]
    NotHeld,
    /// The press was of a row that is not what the ask shows now.
    #[error("the ask no longer shows what you pressed, so nothing was done. Read it again")]
    NotWhatYouSaw,
    /// The forge says something else now, so the ask is the new one and nothing was done.
    #[error(
        "what you were asked about has changed since: it is now {now}. Nothing was done, and the \
         ask shows it as it stands"
    )]
    Changed { now: String },
    /// What the ask shows changed while the act stayed the same: it asks again.
    #[error("what the ask shows changed since you read it, so nothing was done. Read it again")]
    ShowsOtherwise,
    /// The act is no longer offered, for this reason. The ask is gone.
    #[error("{0}")]
    Refused(Refused),
    /// The second sign-in is not there.
    #[error("{0}")]
    WorkflowCredential(String),
    /// The forge refused it, in its words. The ask waits.
    #[error("the forge did not do it: {0}")]
    Forge(String),
    /// The fork the forge made is not in the person's own account. Nothing was recorded.
    #[error(
        "the forge made {made}, which is not in your account {account}, so purlis did not record it"
    )]
    ForkElsewhere { made: String, account: String },
}

#[derive(Debug, Default)]
struct State {
    next: u64,
    held: Vec<Waiting>,
    desk: Vec<(u32, Requested)>,
}

/// **The held acts the app holds.**
#[derive(Debug, Default)]
pub struct HeldActs {
    state: Mutex<State>,
}

/// The key two asks of one chat share when they are the same act asked again.
fn key(requested: &Requested) -> String {
    match requested {
        Requested::Tag { repo, name, .. } => format!("tag {} {}", repo.as_str(), name.as_str()),
        Requested::Release { repo, tag, .. } => {
            format!("release {} {}", repo.as_str(), tag.as_str())
        }
        Requested::CiPush { repo, branch, .. } => {
            format!("ci {} {}", repo.as_str(), branch.as_str())
        }
        Requested::Fork { source } => format!("fork {}", source.as_str()),
        Requested::Merge(request) => format!("merge {}", request.number),
    }
}

impl HeldActs {
    /// **Chat `session` asks for `requested`** at `now`. The act is read from the forge and the
    /// clone ([`prepare`]), and held for the person.
    ///
    /// **Unattended, nothing is asked and nothing is done** (D8.4): `attended` false refuses with
    /// [`super::NOBODY_HERE`] and queues the request for a desktop that connects.
    ///
    /// A try again for the same key at the same act shows what is read now, the one ask still. At
    /// another act (a base that moved, another commit) it retires the old and waits as a new one,
    /// so what the person was looking at is never rewritten under them.
    pub fn ask(
        &self,
        world: &World<'_>,
        session: u32,
        attended: bool,
        requested: &Requested,
        now: u64,
    ) -> Asked {
        if !attended {
            let mut state = locked(&self.state);
            let queued = state.desk.len() < MOST_QUEUED
                && !state
                    .desk
                    .iter()
                    .any(|(of, one)| *of == session && key(one) == key(requested));
            if queued {
                state.desk.push((session, requested.clone()));
            }
            return Asked::Refused(Refused::Unattended { queued });
        }
        let prepared = match prepare(world, requested) {
            Ok(prepared) => prepared,
            Err(why) => return Asked::Refused(why),
        };
        let requested = pinned(requested, &prepared);
        let wanted = key(&requested);
        let mut state = locked(&self.state);
        if let Some(at) = state
            .held
            .iter()
            .position(|one| one.session == session && key(&one.requested) == wanted)
        {
            if state.held[at].prepared.act == prepared.act {
                let one = &mut state.held[at];
                if one.prepared != prepared {
                    one.prepared = prepared;
                    one.asked_once_more = false;
                }
                return Asked::Held {
                    id: one.id,
                    new: false,
                };
            }
            state.held.remove(at);
        }
        if state
            .held
            .iter()
            .filter(|one| one.session == session)
            .count()
            >= AT_MOST_PER_CHAT
        {
            return Asked::TooMany;
        }
        state.next += 1;
        let id = state.next;
        state.held.push(Waiting {
            id,
            session,
            requested,
            prepared,
            since: now,
            asked_once_more: false,
        });
        Asked::Held { id, new: true }
    }

    /// Every held act that waits, oldest first.
    pub fn every(&self) -> Vec<Waiting> {
        locked(&self.state).held.clone()
    }

    /// What the person is shown of ask `id`, with the digest a press hands back.
    pub fn shown(&self, id: u64) -> Option<(Shown, String)> {
        locked(&self.state)
            .held
            .iter()
            .find(|one| one.id == id)
            .map(|one| (one.prepared.shown.clone(), one.prepared.digest()))
    }

    /// The requests queued while nobody was present, taken by a desktop that connects. Each is
    /// asked again, and read again, when it is.
    pub fn take_queued(&self) -> Vec<(u32, Requested)> {
        std::mem::take(&mut locked(&self.state).desk)
    }

    /// **The person's press** of ask `id`, of the row whose digest is `digest`.
    ///
    /// The forge and the clone are read again ([`prepare`]) and the ask is checked against what
    /// they say now:
    ///
    /// - the act is no longer offered: the ask is gone, and nothing was done;
    /// - the act is another (a tag now exists, a base moved, a repository is another): the ask is
    ///   replaced by one at what is true now, and nothing was done ([`NotDone::Changed`]);
    /// - the act is the same and what is shown differs: it is shown as it is now, and asks again
    ///   ([`NotDone::ShowsOtherwise`]);
    /// - otherwise, the first press only asks once more ([`Pressed::AsksOnceMore`]) and the second
    ///   performs, **exactly the act the store holds**, once: a second press of a done act finds
    ///   nothing.
    ///
    /// `by` is the human who pressed, made only from the window's own connection.
    pub fn press(
        &self,
        world: &World<'_>,
        id: u64,
        digest: &str,
        by: Answerer,
    ) -> Result<Pressed, NotDone> {
        let held = {
            let state = locked(&self.state);
            state
                .held
                .iter()
                .find(|one| one.id == id)
                .cloned()
                .ok_or(NotDone::NotHeld)?
        };
        if held.prepared.digest() != digest {
            return Err(NotDone::NotWhatYouSaw);
        }
        // The read just before the press: the only thing that can be performed is what it says.
        let fresh = match prepare(world, &held.requested) {
            Ok(fresh) => fresh,
            Err(why) => {
                self.drop_ask(id);
                return Err(NotDone::Refused(why));
            }
        };
        if fresh.act != held.prepared.act {
            let now = fresh
                .shown
                .rows
                .iter()
                .map(|row| format!("{}: {}", row.label, row.value))
                .collect::<Vec<_>>()
                .join("; ");
            self.replace(id, fresh);
            return Err(NotDone::Changed { now });
        }
        if fresh != held.prepared {
            self.replace(id, fresh);
            return Err(NotDone::ShowsOtherwise);
        }
        // Decided on what the store holds now, in one hold of its lock: an ask another press or a
        // chat's try again replaced meanwhile is never taken as the one that was read.
        let taken = {
            let mut state = locked(&self.state);
            let at = state
                .held
                .iter()
                .position(|one| one.id == id)
                .ok_or(NotDone::NotHeld)?;
            if state.held[at].prepared != fresh {
                return Err(NotDone::ShowsOtherwise);
            }
            if !state.held[at].asked_once_more {
                state.held[at].asked_once_more = true;
                return Ok(Pressed::AsksOnceMore(Box::new(fresh.shown)));
            }
            // Taken out, so a second press of this ask finds nothing, whatever the forge says.
            state.held.remove(at)
        };
        match perform(world, &taken.prepared.act) {
            Ok(forked) => Ok(Pressed::Done(Box::new(Performed {
                act: taken.prepared.act,
                by: scope_of(by),
                forked,
            }))),
            Err(why) => {
                let mut state = locked(&self.state);
                // The ask waits again, as it was, unless the chat asked it again meanwhile.
                if !state.held.iter().any(|one| {
                    one.session == taken.session && key(&one.requested) == key(&taken.requested)
                }) {
                    state.held.push(taken);
                }
                Err(why)
            }
        }
    }

    /// **The person's Don't**: the ask waits no more and nothing is done. Answers whether it was
    /// waiting.
    pub fn dont(&self, id: u64, _by: Answerer) -> bool {
        self.drop_ask(id).is_some()
    }

    /// Chat `session` has closed: none of its held acts waits.
    pub fn forget(&self, session: u32) {
        locked(&self.state)
            .held
            .retain(|one| one.session != session);
    }

    fn drop_ask(&self, id: u64) -> Option<Waiting> {
        let mut state = locked(&self.state);
        let at = state.held.iter().position(|one| one.id == id)?;
        Some(state.held.remove(at))
    }

    /// Ask `id` now shows `fresh`, and has not been asked once more of it.
    fn replace(&self, id: u64, fresh: Prepared) {
        let mut state = locked(&self.state);
        if let Some(one) = state.held.iter_mut().find(|one| one.id == id) {
            one.requested = pinned(&one.requested, &fresh);
            one.prepared = fresh;
            one.asked_once_more = false;
        }
    }
}

/// **Performs `act`**, as the person, and nothing else.
fn perform(world: &World<'_>, act: &Act) -> Result<Option<super::Forked>, NotDone> {
    let forge = world.forge;
    let said = NotDone::Forge;
    match act {
        Act::Tag { repo, name, target } => {
            forge.create_tag(repo, name, target).map_err(said)?;
            Ok(None)
        }
        Act::Release {
            repo,
            tag,
            target,
            creates_tag,
            title,
            notes,
            draft,
            prerelease,
        } => {
            // The tag first, at the commit shown, and the release against the tag: a branch that
            // moves meanwhile changes nothing, and the release names no `target_commitish`.
            if *creates_tag {
                forge.create_tag(repo, tag, target).map_err(said)?;
            }
            forge
                .create_release(repo, tag, title, notes, *draft, *prerelease)
                .map_err(NotDone::Forge)?;
            Ok(None)
        }
        Act::CiPush {
            repo,
            branch,
            tip,
            against,
            base,
            ..
        } => {
            let secret = world
                .workflow
                .secret(&forge.host())
                .map_err(|why| NotDone::WorkflowCredential(why.0))?;
            let existing = matches!(against, super::Against::BranchTip).then_some(base);
            forge
                .push_tip(repo, branch, tip, existing, &secret)
                .map_err(NotDone::Forge)?;
            Ok(None)
        }
        Act::Fork {
            source,
            source_id,
            into,
        } => {
            let made = forge.make_fork(source, source_id).map_err(NotDone::Forge)?;
            // The person's own account is the only place a fork goes: anything else is not
            // recorded, and said.
            let owner = made.path.split('/').next().unwrap_or_default();
            if !owner.eq_ignore_ascii_case(into) {
                return Err(NotDone::ForkElsewhere {
                    made: made.path,
                    account: into.clone(),
                });
            }
            Ok(Some(made))
        }
    }
}
