// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Proof that the person saw an ask** (#6216, from #6236's review; #6218 reuses it).
//!
//! When opening a link asks the person first, the core issues a **one-use token** bound to the
//! pane the link was found in and to the target the ask showed. Only that token turns the ask
//! into an open: a window that never drew the ask has no token to send, a token is spent on its
//! first use whether or not it matches, and one for another pane or another target opens
//! nothing. Tokens last [`LIFE`] and at most [`MOST`] are kept, the oldest dropped first.
//!
//! **A file outside the project, opened on the person's answer, is kept as opened**
//! ([`Asks::opened`]): the window reads it only then (`pane_link_file`), so nothing reads a file
//! there that the person was not shown and did not say to open.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::{Pane, Target};

/// How long an ask's token is good for: the person answers an ask in the window that drew it.
pub const LIFE: Duration = Duration::from_secs(10 * 60);

/// The most tokens kept: a window shows one ask at a time.
pub const MOST: usize = 32;

/// The most files outside the project kept as opened.
pub const MOST_OPENED: usize = 256;

/// One token, as issued.
#[derive(Debug)]
struct Issued {
    token: String,
    project: Option<PathBuf>,
    session: Option<u32>,
    target: Target,
    at: Instant,
}

/// **The asks the core has shown, and the files outside the project the person opened.**
#[derive(Debug)]
pub struct Asks {
    life: Duration,
    issued: Mutex<VecDeque<Issued>>,
    opened: Mutex<VecDeque<PathBuf>>,
}

impl Default for Asks {
    fn default() -> Self {
        Self::with_life(LIFE)
    }
}

impl Asks {
    /// Asks whose tokens are good for `life`.
    pub fn with_life(life: Duration) -> Self {
        Self {
            life,
            issued: Mutex::new(VecDeque::new()),
            opened: Mutex::new(VecDeque::new()),
        }
    }

    /// **A token for an ask** shown in `pane` for `target`.
    pub fn issue(&self, pane: &Pane, target: &Target) -> String {
        // With no randomness there is no token that could not be guessed: none is kept, and the
        // ask is never answered.
        let Some(token) = fresh() else {
            return String::new();
        };
        let mut issued = lock(&self.issued);
        issued.retain(|one| one.at.elapsed() < self.life);
        while issued.len() >= MOST {
            issued.pop_front();
        }
        issued.push_back(Issued {
            token: token.clone(),
            project: pane.project.clone(),
            session: pane.session,
            target: target.clone(),
            at: Instant::now(),
        });
        token
    }

    /// **Whether `token` is an answer to the ask shown in `pane` for `target`**. Spent either
    /// way: a token is good once.
    pub fn accepted(&self, token: &str, pane: &Pane, target: &Target) -> bool {
        let mut issued = lock(&self.issued);
        let Some(at) = issued.iter().position(|one| one.token == token) else {
            return false;
        };
        let Some(one) = issued.remove(at) else {
            return false;
        };
        one.at.elapsed() < self.life
            && one.project == pane.project
            && one.session == pane.session
            && one.target == *target
    }

    /// `path`, a file outside the project, was opened on the person's answer.
    pub fn keep_opened(&self, path: &Path) {
        let mut opened = lock(&self.opened);
        opened.retain(|one| one != path);
        while opened.len() >= MOST_OPENED {
            opened.pop_front();
        }
        opened.push_back(path.to_path_buf());
    }

    /// Whether `path` is a file outside the project the person said to open.
    pub fn opened(&self, path: &Path) -> bool {
        lock(&self.opened).iter().any(|one| one == path)
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
