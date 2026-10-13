// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! When the app saves a plane by itself (charter-app#296, ADR 0051).
//!
//! **A decision, not a loop.** [`Quiet::tick`] is told what the plane looks like now and what
//! time it is, and answers whether to save. The loop that asks it, the clock and the save are
//! the app's; everything that decides is here, where a test can drive it with a made-up clock.
//!
//! **An auto-save is the app's commit, and carries no provenance trailers** (#1011, V67). After
//! the quiet period, at a session's end and at quit alike, the app commits whatever the tree
//! holds: several chats can have written one repo, and the person too, and nothing records which
//! of them wrote which change. Naming the chat that just ended, or any one chat, would put a
//! guess into history, which is permanent; a trailer is written only where an agent run made
//! the commit. An agent's own work is stamped where it commits it, through `commit-msg` and a
//! chat's `purlis save` ([`crate::provenance`]).

use std::time::{Duration, Instant};

use crate::planegit::{Stage, Standing};
use crate::planesave::{Mode, Plane};

/// How long a save that did not settle the plane — a push to a remote that is down — waits
/// before auto-save tries again, however short the quiet period is.
pub const RETRY_AFTER: Duration = Duration::from_secs(300);

/// What auto-save decides on one look at a plane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Wait,
    Save,
}

/// Whether auto-save is on for a plane: `[plane] autosave`, and a mode that saves. A plane that
/// names no mode has not been asked yet, and is never saved by itself before it has.
pub fn on(plane: &Plane) -> bool {
    plane.autosave.value && matches!(plane.mode.value, Some(mode) if mode != Mode::Off)
}

/// Whether auto-save is on for a workspace repo: `[repos.<name>] autosave`, and a mode that
/// saves.
pub fn repo_on(repo: &crate::planesave::Repo) -> bool {
    repo.autosave.value && repo.mode.value != Mode::Off
}

/// Whether a save of a plane standing like this would do anything: files to commit, or commits
/// a push would carry — to the target branch, or in a request mode to the save branch and its pull
/// request (`standing` counts those as committed until the pull request carries them).
pub fn worth_saving(standing: &Standing) -> bool {
    !standing.changed.is_empty() || (standing.stage == Stage::Committed && standing.pushes)
}

/// One plane's quiet period: what it last looked like, since when, and whether a save was
/// already tried on that look.
#[derive(Debug, Default)]
pub struct Quiet {
    seen: Option<Seen>,
}

#[derive(Debug)]
struct Seen {
    fingerprint: String,
    since: Instant,
    tried: bool,
}

impl Quiet {
    /// Look at the plane at `now`. `fingerprint` changes whenever what is unsaved changes
    /// ([`crate::planegit::fingerprint`]); `quiet` is `[plane] autosave_after`.
    pub fn tick(
        &mut self,
        now: Instant,
        plane: &Plane,
        standing: &Standing,
        fingerprint: &str,
    ) -> Decision {
        let saves = on(plane) && standing.stage != Stage::Blocked && worth_saving(standing);
        self.tick_when(now, saves, plane.autosave_after.value, fingerprint)
    }

    /// [`Quiet::tick`], for a workspace repo: saved by itself only when its `[repos.<name>]`
    /// table turns `autosave` on, which is off by default (ADR 0051).
    pub fn tick_repo(
        &mut self,
        now: Instant,
        repo: &crate::planesave::Repo,
        standing: &crate::reposave::Standing,
    ) -> Decision {
        let saves = repo_on(repo) && standing.stage != Stage::Blocked && standing.worth_saving();
        self.tick_when(
            now,
            saves,
            repo.autosave_after.value,
            &standing.fingerprint(),
        )
    }

    /// After [`Quiet::tick`] answered [`Decision::Save`]: whether the tree read again now still
    /// has the `fingerprint` the quiet period was measured on (FD-11). The standing `tick` was
    /// given may be shared, and a few seconds behind a new file; when the fresh one differs,
    /// the quiet period starts again from `now` and nothing is saved.
    pub fn still(&mut self, now: Instant, fingerprint: &str) -> bool {
        match &self.seen {
            Some(seen) if seen.fingerprint == fingerprint => true,
            _ => {
                self.seen = Some(Seen {
                    fingerprint: fingerprint.to_owned(),
                    since: now,
                    tried: false,
                });
                false
            }
        }
    }

    fn tick_when(
        &mut self,
        now: Instant,
        saves: bool,
        quiet: Duration,
        fingerprint: &str,
    ) -> Decision {
        // Off, paused while blocked, or nothing to take: forget the quiet period, so the next
        // change starts a fresh one.
        if !saves {
            self.seen = None;
            return Decision::Wait;
        }
        match &mut self.seen {
            Some(seen) if seen.fingerprint == fingerprint => {
                let wait = if seen.tried {
                    quiet.max(RETRY_AFTER)
                } else {
                    quiet
                };
                if now.saturating_duration_since(seen.since) < wait {
                    return Decision::Wait;
                }
                seen.since = now;
                seen.tried = true;
                Decision::Save
            }
            _ => {
                self.seen = Some(Seen {
                    fingerprint: fingerprint.to_owned(),
                    since: now,
                    tried: false,
                });
                Decision::Wait
            }
        }
    }
}

/// What saving a plane as the app quits came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtQuit {
    /// Auto-save is off, or there was nothing to save.
    Nothing,
    /// Committed; the save stops at the commit, or there is nothing to push to.
    Committed,
    /// Committed and pushed within the bound.
    Pushed,
    /// Committed, and the push had not finished within the bound. The launch after this one
    /// pushes what is left ([`on`] and [`worth_saving`] still say so then).
    CommittedPushStillRunning,
    /// The commit itself was refused — a secret, a signer — and is in the journal.
    Refused,
}

/// Save the plane at `root` as the app quits (ADR 0051): commit at once — a commit is local and
/// quick, and it is what makes the work safe on this machine — then give the push no more than
/// `bound`. A push still running then is left to finish or fail on its own; the app does not
/// wait for a remote that never answers.
pub fn at_quit(root: &std::path::Path, bound: Duration) -> AtQuit {
    let plane = crate::planesave::Settings::read(root).plane;
    let standing = crate::planegit::standing(root);
    if !on(&plane) || standing.stage == Stage::Blocked || !worth_saving(&standing) {
        return AtQuit::Nothing;
    }
    // Whatever else is saving this plane — the worker's last save — finishes first, within the
    // bound; the commit and the push below then hold the plane, as every save does.
    let Some(claim) = crate::planegit::Claim::within(root, bound) else {
        return AtQuit::Nothing;
    };
    if !standing.changed.is_empty() {
        let code = crate::planegit::save_claimed(
            &crate::planegit::Request {
                root,
                message: None,
                sign: false,
                no_push: true,
                cwd: root,
                provenance: None,
            },
            crate::planegit::Trigger::Quit,
            &claim,
            &mut |_| {},
        );
        if code != 0 {
            return AtQuit::Refused;
        }
    }
    if !crate::planegit::standing(root).pushes {
        return AtQuit::Committed;
    }
    let (told, heard) = std::sync::mpsc::channel();
    let at = root.to_path_buf();
    let sign = plane.sign.value;
    std::thread::spawn(move || {
        // The claim goes with the push, and is let go of when the push ends.
        let _claim = claim;
        // As the mode says: the target branch, or the save branch and its pull request.
        let pushed = crate::planegit::push_saved(&at, sign, &mut |_| {});
        let _ = told.send(matches!(
            pushed.outcome,
            crate::planegit::Outcome::Pushed | crate::planegit::Outcome::PrOpen
        ));
    });
    match heard.recv_timeout(bound) {
        Ok(true) => AtQuit::Pushed,
        Ok(false) => AtQuit::Committed,
        Err(_) => AtQuit::CommittedPushStillRunning,
    }
}

/// Save one workspace repo as the app quits, when its `[repos.<name>] autosave` is on — and
/// only then (ADR 0051). The same shape as [`at_quit`]: the commit at once, the push and any
/// pull request given no more than `bound`.
///
/// `mid_turn` is who was mid-turn where they could write this clone **before the quit ended
/// the chats**. Their turns were cut off, so what they left is half-written: the repo is not
/// saved, and the journal says so as skipped.
pub fn repo_at_quit(
    plane: &std::path::Path,
    workspace: &str,
    repo: &crate::repos::Repo,
    mid_turn: &[String],
    bound: Duration,
) -> AtQuit {
    let settings = crate::planesave::Settings::read(plane).repo(&repo.name);
    let standing = crate::reposave::standing(plane, workspace, repo);
    if !repo_on(&settings) || standing.stage == Stage::Blocked || !standing.worth_saving() {
        return AtQuit::Nothing;
    }
    let Some(claim) = crate::planegit::Claim::within(&repo.path, bound) else {
        return AtQuit::Nothing;
    };
    let cut_off = || mid_turn.to_vec();
    if !mid_turn.is_empty() {
        // The save sees them and journals itself as skipped.
        crate::reposave::save_claimed(
            &crate::reposave::Request {
                plane,
                workspace,
                name: &repo.name,
                clone: &repo.path,
                message: None,
                no_push: true,
                mid_turn: &cut_off,
            },
            crate::planegit::Trigger::Quit,
            &claim,
            &mut |_| {},
        );
        return AtQuit::Nothing;
    }
    let request = |no_push| crate::reposave::Request {
        plane,
        workspace,
        name: &repo.name,
        clone: &repo.path,
        message: None,
        no_push,
        mid_turn: &crate::reposave::nobody_working,
    };
    if standing.changed > 0 {
        let code = crate::reposave::save_claimed(
            &request(true),
            crate::planegit::Trigger::Quit,
            &claim,
            &mut |_| {},
        );
        if code != 0 {
            return AtQuit::Refused;
        }
    }
    if !standing.pushes {
        return AtQuit::Committed;
    }
    let (told, heard) = std::sync::mpsc::channel();
    let (plane, workspace, name, clone) = (
        plane.to_path_buf(),
        workspace.to_owned(),
        repo.name.clone(),
        repo.path.clone(),
    );
    std::thread::spawn(move || {
        let code = crate::reposave::save_claimed(
            &crate::reposave::Request {
                plane: &plane,
                workspace: &workspace,
                name: &name,
                clone: &clone,
                message: None,
                no_push: false,
                mid_turn: &crate::reposave::nobody_working,
            },
            crate::planegit::Trigger::Quit,
            &claim,
            &mut |_| {},
        );
        drop(claim);
        let _ = told.send(code == 0);
    });
    match heard.recv_timeout(bound) {
        Ok(true) => AtQuit::Pushed,
        Ok(false) => AtQuit::Committed,
        Err(_) => AtQuit::CommittedPushStillRunning,
    }
}

/// How long after the settle wait the first look at a new request is made: the wait itself is
/// measured on the forge's side of the push, and a look a second early would wait again.
pub const LOOK_MARGIN: Duration = Duration::from_secs(5);
/// The longest gap between two looks at one open request.
pub const LOOK_AT_MOST: Duration = Duration::from_secs(300);
/// How long purlis keeps looking at one open request by itself. Past it, only a save, a fetch
/// or the window coming back into focus asks again: a check that has not ended in two hours is
/// nobody's to wait on in a loop.
pub const LOOK_FOR: Duration = Duration::from_secs(2 * 60 * 60);
/// The least time between two looks, however often the window comes into focus.
pub const LOOK_AT_LEAST: Duration = Duration::from_secs(60);

/// **When the app looks again at a `pr-merge` save's open pull request** (ADR 0051, amended
/// 2026-10-13): what merges a green request without the person saving again, paced so that
/// it is never a busy poll.
///
/// For one head commit: the first look once its checks have had time to start
/// ([`crate::forge::pr::SETTLE`] and [`LOOK_MARGIN`] after the push), then after one minute,
/// two, four, and every five ([`LOOK_AT_MOST`]) until [`LOOK_FOR`] has passed since the
/// first. That is about thirty looks, each a handful of forge reads. The window coming back
/// into focus asks for one more at any time, at most every [`LOOK_AT_LEAST`]. A new head — the
/// next save pushed onto the request — starts over.
///
/// It only says when; the caller looks ([`crate::planegit::look`], [`crate::reposave::look`]).
#[derive(Debug, Default)]
pub struct Looks {
    head: String,
    /// When this head was first seen.
    since: Option<Instant>,
    /// When the next look is due.
    next: Option<Instant>,
    last: Option<Instant>,
    made: u32,
}

impl Looks {
    /// Whether a look at the request whose head is `head` is due at `now`. `pushed_ago` is
    /// how long ago that head was pushed, where that is known; `asked` is the window coming
    /// back into focus. A `true` counts as the look: the caller makes it now.
    pub fn due(
        &mut self,
        now: Instant,
        head: &str,
        pushed_ago: Option<Duration>,
        asked: bool,
    ) -> bool {
        if self.head != head {
            let wait = (crate::forge::pr::SETTLE + LOOK_MARGIN)
                .saturating_sub(pushed_ago.unwrap_or_default());
            *self = Looks {
                head: head.to_owned(),
                since: Some(now),
                next: Some(now + wait),
                last: None,
                made: 0,
            };
        }
        let rested = self
            .last
            .is_none_or(|last| now.duration_since(last) >= LOOK_AT_LEAST);
        let within = self
            .since
            .is_some_and(|since| now.duration_since(since) <= LOOK_FOR);
        let scheduled = within && self.next.is_some_and(|next| now >= next);
        // Focus never looks before the checks could have started: that look would only wait.
        let on_focus =
            asked && rested && self.next.is_some_and(|next| now >= next || self.made > 0);
        if !(scheduled || on_focus) {
            return false;
        }
        let gap = LOOK_AT_LEAST
            .saturating_mul(1u32 << self.made.min(8))
            .min(LOOK_AT_MOST);
        self.made = self.made.saturating_add(1);
        self.last = Some(now);
        self.next = Some(now + gap);
        true
    }
}

#[cfg(test)]
mod tests;
