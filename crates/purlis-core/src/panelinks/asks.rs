// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **An ask's answer is its token** (#6216, from #6236's review; #6218 reuses it).
//!
//! When opening a link asks the person first, the core issues a **one-use token** bound to the
//! pane the link was found in and to the target the ask showed. Only that token turns the ask
//! into an open. What it proves is narrow: the window that sends it was handed this ask, for
//! this target in this pane, by the core, and answers it once. It does not prove a person read
//! the ask; that is the window's dialog's to show. A token is spent on its first use whether or
//! not it matches, and one for another pane or another target opens nothing. Tokens last
//! [`LIFE`] and at most [`MOST`] are kept, the oldest dropped first.
//!
//! **A file outside the project, opened on an answered ask, is kept as opened**
//! ([`Asks::opened`]), for the pane it was asked in, for [`OPENED_LIFE`], and as the file it was
//! then (its [`Identity`]): the window reads it only then (`pane_link_file`), and a file put in
//! its place since reads nothing.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::{Pane, Target};
pub use crate::files::folder::Identity;

/// How long an ask's token is good for: an ask is answered in the window that drew it.
pub const LIFE: Duration = Duration::from_secs(10 * 60);

/// The most tokens kept: a window shows one ask at a time.
pub const MOST: usize = 32;

/// How long a file outside the project stays readable after its ask was answered.
pub const OPENED_LIFE: Duration = Duration::from_secs(60 * 60);

/// The most files outside the project kept as opened.
pub const MOST_OPENED: usize = 256;

/// Which pane something was asked in: its project and its chat.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Asked {
    project: Option<PathBuf>,
    session: Option<u32>,
}

impl From<&Pane> for Asked {
    fn from(pane: &Pane) -> Self {
        Self {
            project: pane.project.clone(),
            session: pane.session,
        }
    }
}

/// One token, as issued.
#[derive(Debug)]
struct Issued {
    token: String,
    pane: Asked,
    target: Target,
    at: Instant,
}

/// One file outside the project, as opened.
#[derive(Debug)]
struct Opened {
    pane: Asked,
    path: PathBuf,
    identity: Identity,
    at: Instant,
}

/// One file on a runner, outside its chat's workspace, as opened (#6387), and which file it
/// was then (`onrunner::identity_of`).
#[derive(Debug)]
struct OnRunner {
    pane: Asked,
    runner: String,
    path: String,
    identity: String,
    at: Instant,
}

/// **The asks the core issued tokens for, and the files outside the project opened on one.**
#[derive(Debug)]
pub struct Asks {
    life: Duration,
    opened_life: Duration,
    issued: Mutex<VecDeque<Issued>>,
    opened: Mutex<VecDeque<Opened>>,
    on_runners: Mutex<VecDeque<OnRunner>>,
}

impl Default for Asks {
    fn default() -> Self {
        Self::with_lives(LIFE, OPENED_LIFE)
    }
}

impl Asks {
    /// Asks whose tokens are good for `life`, and whose files stay opened for `opened_life`.
    pub fn with_lives(life: Duration, opened_life: Duration) -> Self {
        Self {
            life,
            opened_life,
            issued: Mutex::new(VecDeque::new()),
            opened: Mutex::new(VecDeque::new()),
            on_runners: Mutex::new(VecDeque::new()),
        }
    }

    /// **A token for an ask** shown in `pane` for `target`. `None` where the system gives no
    /// randomness: no token that could be guessed is ever issued.
    pub fn issue(&self, pane: &Pane, target: &Target) -> Option<String> {
        let token = fresh()?;
        let mut issued = lock(&self.issued);
        issued.retain(|one| one.at.elapsed() < self.life);
        while issued.len() >= MOST {
            issued.pop_front();
        }
        issued.push_back(Issued {
            token: token.clone(),
            pane: pane.into(),
            target: target.clone(),
            at: Instant::now(),
        });
        Some(token)
    }

    /// **Whether `token` answers the ask issued in `pane` for `target`**. Spent either way: a
    /// token is good once.
    pub fn accepted(&self, token: &str, pane: &Pane, target: &Target) -> bool {
        let mut issued = lock(&self.issued);
        let Some(at) = issued.iter().position(|one| one.token == token) else {
            return false;
        };
        let Some(one) = issued.remove(at) else {
            return false;
        };
        one.at.elapsed() < self.life && one.pane == Asked::from(pane) && one.target == *target
    }

    /// `path`, a file outside the project, was opened in `pane` on an answered ask, and was
    /// then the file `identity` names.
    pub fn keep_opened(&self, pane: &Pane, path: &Path, identity: Identity) {
        let pane = Asked::from(pane);
        let mut opened = lock(&self.opened);
        opened.retain(|one| {
            one.at.elapsed() < self.opened_life && !(one.pane == pane && one.path == path)
        });
        while opened.len() >= MOST_OPENED {
            opened.pop_front();
        }
        opened.push_back(Opened {
            pane,
            path: path.to_path_buf(),
            identity,
            at: Instant::now(),
        });
    }

    /// **Which file `path` was when it was opened in `pane`** on an answered ask, while that is
    /// within [`OPENED_LIFE`]; `None` for a file nobody opened there.
    pub fn opened(&self, pane: &Pane, path: &Path) -> Option<Identity> {
        let pane = Asked::from(pane);
        lock(&self.opened)
            .iter()
            .rev()
            .find(|one| one.pane == pane && one.path == path && one.at.elapsed() < self.opened_life)
            .map(|one| one.identity)
    }
}

impl Asks {
    /// `path`, a file on `runner` outside its chat's workspace, was opened in `pane` on an
    /// answered ask (#6387), and was then the file `identity` names: the runner reads it for
    /// that pane's tab from now on, while it is still that file.
    pub fn keep_opened_on_runner(&self, pane: &Pane, runner: &str, path: &str, identity: &str) {
        let pane = Asked::from(pane);
        let mut on_runners = lock(&self.on_runners);
        on_runners.retain(|one| {
            one.at.elapsed() < self.opened_life
                && !(one.pane == pane && one.runner == runner && one.path == path)
        });
        while on_runners.len() >= MOST_OPENED {
            on_runners.pop_front();
        }
        on_runners.push_back(OnRunner {
            pane,
            runner: runner.to_owned(),
            path: path.to_owned(),
            identity: identity.to_owned(),
            at: Instant::now(),
        });
    }

    /// **Which file `path` on `runner` was when it was opened in `pane`** on an answered ask,
    /// while that is within [`OPENED_LIFE`]; `None` for a file nobody opened there.
    pub fn opened_on_runner(&self, pane: &Pane, runner: &str, path: &str) -> Option<String> {
        let pane = Asked::from(pane);
        lock(&self.on_runners)
            .iter()
            .rev()
            .find(|one| {
                one.pane == pane
                    && one.runner == runner
                    && one.path == path
                    && one.at.elapsed() < self.opened_life
            })
            .map(|one| one.identity.clone())
    }
}

/// A new token: 128 bits of the operating system's randomness, as hex.
fn fresh() -> Option<String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).ok()?;
    Some(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

fn lock<T>(held: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    held.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
