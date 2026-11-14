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
//! again. Each is keyed by the forge's host and the repo's path ([`OnHost`], Q277): two forges'
//! `o/r` are two repos.

use std::sync::{Mutex, PoisonError};

use super::{HeadSha, MergeKey, Repo};
use crate::forge::pr::{Review, ReviewState, Reviewer};

/// **How many presses a merge takes**, whatever was looked at (Q268, Q269): Merge, then the
/// once-more Merge.
pub const PRESSES: u8 = 2;

/// The most viewed heads, and pushes per chat, held: past it the oldest goes.
const AT_MOST: usize = 256;

/// **A repo on one forge** (Q277): the forge's host, in lower case, and the repo's path. Two
/// forges' `o/r` are two repos, so nothing viewed or pushed on one is said of the other.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OnHost {
    host: String,
    repo: Repo,
}

impl OnHost {
    /// `repo` on the forge at `host`, which is compared in any case.
    pub fn new(host: &str, repo: Repo) -> OnHost {
        OnHost {
            host: host.to_ascii_lowercase(),
            repo,
        }
    }

    /// The forge's host, in lower case.
    pub fn host(&self) -> &str {
        &self.host
    }

    /// The repo's path on it.
    pub fn repo(&self) -> &Repo {
        &self.repo
    }
}

/// **Who approves `head`, of `reviews`**: the first person who is not `author` (by login, in
/// any case) and whose last say was Approved, of this very head. A comment changes nothing
/// they said before; a later request for changes, or a dismissal, takes an approval back. An
/// app's or a bot's approval never counts, nor one of an older head. `None` where nobody does.
pub fn approves(reviews: &[Review], head: &HeadSha, author: &str) -> Option<String> {
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
        .find(|(_, review)| review.state == ReviewState::Approved && review.commit == head.as_str())
        .map(|(login, _)| (*login).to_owned())
}

/// **What the person has seen of a merge's head** (R-122, R-123).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Look {
    /// The person viewed this head's changes in purlis.
    Viewed,
    /// Not viewed here, but the forge holds an approving review of this head by `by`, a person
    /// who is neither its author nor an app or a bot (Q277: the row names them).
    Approved { by: String },
    /// Neither. `since` is how many commits this head is ahead of the one the person viewed,
    /// where purlis knows it.
    NotViewed { since: Option<u32> },
}

impl Look {
    /// The look of `head`, where the person last viewed `viewed`, the forge's reviews approve
    /// it by `approved_by` or by nobody, and `since` commits lie between the viewed head and
    /// it, where known.
    pub fn of(
        viewed: Option<&HeadSha>,
        head: &HeadSha,
        approved_by: Option<&str>,
        since: Option<u32>,
    ) -> Look {
        if viewed == Some(head) {
            Look::Viewed
        } else if let Some(by) = approved_by {
            Look::Approved { by: by.to_owned() }
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
    heads: Mutex<Vec<((OnHost, u64), HeadSha)>>,
}

impl Viewed {
    /// The person viewed `head` of request `number` of `repo`.
    pub fn mark(&self, repo: &OnHost, number: u64, head: &HeadSha) {
        let mut heads = self.heads.lock().unwrap_or_else(PoisonError::into_inner);
        heads.retain(|((of, n), _)| !(of == repo && *n == number));
        if heads.len() >= AT_MOST {
            heads.remove(0);
        }
        heads.push(((repo.clone(), number), head.clone()));
    }

    /// The head of request `number` of `repo` the person last viewed, if any.
    pub fn of(&self, repo: &OnHost, number: u64) -> Option<HeadSha> {
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
    pushed: Mutex<Vec<(u32, OnHost, HeadSha)>>,
}

impl Pushes {
    /// Chat `session` pushed `head` to `repo`: the commit the push sent, never one read after
    /// it (Q277).
    pub fn record(&self, session: u32, repo: &OnHost, head: &HeadSha) {
        let mut pushed = self.pushed.lock().unwrap_or_else(PoisonError::into_inner);
        if pushed.iter().filter(|(of, _, _)| *of == session).count() >= AT_MOST
            && let Some(oldest) = pushed.iter().position(|(of, _, _)| *of == session)
        {
            pushed.remove(oldest);
        }
        pushed.push((session, repo.clone(), head.clone()));
    }

    /// Whether purlis recorded chat `session` pushing `head` to `repo`.
    pub fn pushed(&self, session: u32, repo: &OnHost, head: &HeadSha) -> bool {
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

/// **How many commits one head is ahead of another** (R-123), as the forge said, in memory
/// only: what "N commits since you looked" counts, from the head the person viewed to the
/// ask's head, whichever path found the move (Q277).
#[derive(Debug, Default)]
pub struct Ahead {
    counted: Mutex<Vec<(OnHost, HeadSha, HeadSha, u32)>>,
}

impl Ahead {
    /// The forge said `to` is `commits` ahead of `from` in `repo`.
    pub fn count(&self, repo: &OnHost, from: &HeadSha, to: &HeadSha, commits: u32) {
        let mut counted = self.counted.lock().unwrap_or_else(PoisonError::into_inner);
        counted.retain(|(of, a, b, _)| !(of == repo && a == from && b == to));
        if counted.len() >= AT_MOST {
            counted.remove(0);
        }
        counted.push((repo.clone(), from.clone(), to.clone(), commits));
    }

    /// How many commits `to` is ahead of `from` in `repo`, where the forge said.
    pub fn of(&self, repo: &OnHost, from: &HeadSha, to: &HeadSha) -> Option<u32> {
        self.counted
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .find(|(of, a, b, _)| of == repo && a == from && b == to)
            .map(|(_, _, _, commits)| *commits)
    }
}

/// **A watched request whose head moved** (R-125): chat `session`'s ask at `was` is retired for
/// one at `now`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadMoved {
    pub session: u32,
    pub was: MergeKey,
    pub now: MergeKey,
}

/// **What one reading of the watch found that the window must redraw** (R-125, Q277).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Watched {
    /// The head moved: the ask was retired for one at the new head.
    Moved(HeadMoved),
    /// The head is where it was, and what the ask shows of it changed: chat `session`'s ask
    /// for `key` shows the new read.
    Changed { session: u32, key: MergeKey },
}
