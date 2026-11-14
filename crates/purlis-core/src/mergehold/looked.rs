// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Who looked at a merge's head, and who pushed it** (T1-41; R-122, R-123, R-124, Q210).
//!
//! A merge always takes two presses (Q268, Q269): nothing here lowers that. What it decides is
//! what the row says beside them: that the person viewed this head in purlis, that the forge
//! holds an approving review of it by a person who is neither its author nor an app or a bot,
//! or that neither is so, with the commits since the head the person last viewed. And whether
//! the head is one purlis recorded the asking chat pushing.
//!
//! Both records are held in the app's memory alone, for the window's life (Q210): nothing here
//! is written to disk, so a relaunch knows no viewed head and no push, and the person views
//! again.

use std::sync::{Mutex, PoisonError};

use super::{HeadSha, MergeKey, Repo};
use crate::forge::pr::{Review, ReviewState, Reviewer};

/// **How many presses a merge takes**, whatever was looked at (Q268, Q269): Merge, then the
/// once-more Merge.
pub const PRESSES: u8 = 2;

/// The most viewed heads, and pushes per chat, held: past it the oldest goes.
const AT_MOST: usize = 256;

/// **Whether `reviews` approve `head`**: some person who is not `author` (by login, in any
/// case) last said Approved, and of this very head. A comment changes nothing they said
/// before; a later request for changes, or a dismissal, takes an approval back. An app's or a
/// bot's approval never counts, nor one of an older head.
pub fn approves(reviews: &[Review], head: &HeadSha, author: &str) -> bool {
    let mut last: Vec<(&str, &Review)> = Vec::new();
    for review in reviews {
        if review.by != Reviewer::Person
            || review.login.eq_ignore_ascii_case(author)
            || review.state == ReviewState::Commented
        {
            continue;
        }
        match last
            .iter_mut()
            .find(|(login, _)| login.eq_ignore_ascii_case(&review.login))
        {
            Some(one) => one.1 = review,
            None => last.push((review.login.as_str(), review)),
        }
    }
    last.iter()
        .any(|(_, review)| review.state == ReviewState::Approved && review.commit == head.as_str())
}

/// **What the person has seen of a merge's head** (R-122, R-123).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Look {
    /// The person viewed this head's changes in purlis.
    Viewed,
    /// Not viewed here, but the forge holds an approving review of this head by a person who is
    /// neither its author nor an app or a bot.
    Approved,
    /// Neither. `since` is how many commits this head is ahead of the one the person viewed,
    /// where purlis knows it.
    NotViewed { since: Option<u32> },
}

impl Look {
    /// The look of `head`, where the person last viewed `viewed`, the forge's reviews approve
    /// it or not, and `since` commits lie between the viewed head and it, where known.
    pub fn of(
        viewed: Option<&HeadSha>,
        head: &HeadSha,
        approved: bool,
        since: Option<u32>,
    ) -> Look {
        if viewed == Some(head) {
            Look::Viewed
        } else if approved {
            Look::Approved
        } else {
            Look::NotViewed {
                since: viewed.and(since),
            }
        }
    }

    /// How many presses the merge takes: [`PRESSES`], always (Q268).
    pub fn presses(&self) -> u8 {
        PRESSES
    }
}

/// **The head the person last viewed of each request** (R-122, R-302), in memory only.
#[derive(Debug, Default)]
pub struct Viewed {
    heads: Mutex<Vec<((Repo, u64), HeadSha)>>,
}

impl Viewed {
    /// The person viewed `head` of request `number` of `repo`.
    pub fn mark(&self, repo: &Repo, number: u64, head: &HeadSha) {
        let mut heads = self.heads.lock().unwrap_or_else(PoisonError::into_inner);
        heads.retain(|((of, n), _)| !(of == repo && *n == number));
        if heads.len() >= AT_MOST {
            heads.remove(0);
        }
        heads.push(((repo.clone(), number), head.clone()));
    }

    /// The head of request `number` of `repo` the person last viewed, if any.
    pub fn of(&self, repo: &Repo, number: u64) -> Option<HeadSha> {
        self.heads
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .find(|((of, n), _)| of == repo && *n == number)
            .map(|(_, head)| head.clone())
    }
}

/// **The heads purlis recorded each chat pushing** (R-124): its brokered pushes, in memory only.
/// A push purlis did not make for the chat is not recorded, so its head is flagged.
#[derive(Debug, Default)]
pub struct Pushes {
    pushed: Mutex<Vec<(u32, Repo, HeadSha)>>,
}

impl Pushes {
    /// Chat `session` pushed `head` to `repo`.
    pub fn record(&self, session: u32, repo: &Repo, head: &HeadSha) {
        let mut pushed = self.pushed.lock().unwrap_or_else(PoisonError::into_inner);
        if pushed.iter().filter(|(of, _, _)| *of == session).count() >= AT_MOST
            && let Some(oldest) = pushed.iter().position(|(of, _, _)| *of == session)
        {
            pushed.remove(oldest);
        }
        pushed.push((session, repo.clone(), head.clone()));
    }

    /// Whether purlis recorded chat `session` pushing `head` to `repo`.
    pub fn pushed(&self, session: u32, repo: &Repo, head: &HeadSha) -> bool {
        self.pushed
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .any(|(of, to, at)| *of == session && to == repo && at == head)
    }

    /// Chat `session` has closed: none of its pushes is kept.
    pub fn forget(&self, session: u32) {
        self.pushed
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .retain(|(of, _, _)| *of != session);
    }
}

/// **A watched request whose head moved** (R-125): chat `session`'s ask at `was` is retired for
/// one at `now`, `commits` ahead where the forge said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadMoved {
    pub session: u32,
    pub was: MergeKey,
    pub now: MergeKey,
    pub commits: Option<u32>,
}
