// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What a chat's browser may reach, as the person answered it, kept for the chat's life**
//! (#7190 item 1, the person's ruling of 2026-11-18): `app/browser-postures.json` in the
//! project's state folder.
//!
//! A chat's first browser call asks the person what its browser may reach. The answer used to
//! be held in memory, so every restart asked again. It is now kept for **that chat only**, by its
//! identity (ADR 0066), and it lasts as long as purlis keeps a record of the chat ([`prune`]):
//!
//! - **This machine's, never the plane**: under `.purlis/app/`, which is gitignored and never
//!   synced. No sandboxed chat writes it: `.purlis/app/` is the integrity class's.
//! - **Per chat**: a new chat has a new identity and finds no answer, so it asks. There is no
//!   answer "for every chat of this persona"; that is what the persona's Always is.
//! - **Only what the person pressed**: an answer that rests on a persona's Always is not kept
//!   (it follows that Always as it is now).
//! - **A resumed chat keeps its answer**: the same identity finds it again, and a chat that
//!   closed is resumable, so closing it keeps its answer. It goes only when purlis lets go of
//!   the chat's record (swept after its days, or never read back), never on a close.
//! - **A kept answer is checked against the chat's current reach before use**: it is kept with
//!   the reach the chat had when it was asked ([`Reach`]), and a restored answer is used only
//!   where the chat's reach now is no narrower ([`Reach::narrower_than`]). Where it narrowed
//!   meanwhile, the answer is not used and is removed, and the person is asked again.
//! - **One writer at a time, across processes**: every change reads the file, changes it and
//!   writes it whole, under a lock file beside it ([`crate::filelock::beside`]), through a temp
//!   file renamed over it ([`crate::rewrite::replace`]).
//! - **A chat purlis has no record of leaves nothing**: [`prune`] drops the answers of chats
//!   the caller says no longer exist, at start. "No record" is never "not open right now" or
//!   "not open in this window".

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// The file, relative to the project's state folder ([`path`]).
pub const IN_STATE: &str = "app/browser-postures.json";

/// The most answers kept: a chat's answer ends with it, so this is a bound on a damaged file.
const MOST: usize = 4096;

/// How long a writer waits for another's turn at the file.
const WAIT: Duration = Duration::from_secs(10);

/// What the person answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    /// This project's sites and its dev servers, and with `open_web` the open web too.
    Allowed { open_web: bool },
    /// Keep off.
    Off,
}

impl Answer {
    fn word(self) -> &'static str {
        match self {
            Answer::Allowed { open_web: false } => "allowed",
            Answer::Allowed { open_web: true } => "allowed-open-web",
            Answer::Off => "off",
        }
    }

    fn of(word: &str) -> Option<Answer> {
        Some(match word {
            "allowed" => Answer::Allowed { open_web: false },
            "allowed-open-web" => Answer::Allowed { open_web: true },
            "off" => Answer::Off,
            _ => return None,
        })
    }
}

/// **What a chat's browser reached when its answer was asked**: everything a chat's reach holds
/// (`BrowserReach`): the sites its sandbox lists, what it never reaches, the ports of this
/// machine it carries no longer, whether the open web is reached, and the sites the person
/// allowed it when asked. Sites by name, the layer that lists them aside: a site moving between
/// layers is no narrowing.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reach {
    #[serde(default)]
    pub listed: BTreeSet<String>,
    #[serde(default)]
    pub never: BTreeSet<String>,
    #[serde(default)]
    pub withdrawn: BTreeSet<u16>,
    #[serde(default)]
    pub open_web: bool,
    #[serde(default)]
    pub answered: BTreeSet<String>,
}

impl Reach {
    /// **Whether this reach, as it is now, is narrower than `then`**: a site `then` listed is no
    /// longer listed or answered, the open web was reached and is not, or something is never
    /// reached or withdrawn that `then` reached. A wider reach is not narrower, and nor is an
    /// equal one.
    pub fn narrower_than(&self, then: &Reach) -> bool {
        then.listed.iter().any(|site| !self.listed.contains(site))
            || then
                .answered
                .iter()
                .any(|site| !self.answered.contains(site))
            || (then.open_web && !self.open_web)
            || self.never.iter().any(|site| !then.never.contains(site))
            || self
                .withdrawn
                .iter()
                .any(|port| !then.withdrawn.contains(port))
    }
}

/// **An answer kept**, with the reach the chat had when it was asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kept {
    pub answer: Answer,
    pub reach: Reach,
}

#[derive(Serialize, Deserialize)]
struct Stored {
    answer: String,
    #[serde(default)]
    reach: Reach,
    /// When it was given, in seconds since 1970: what [`prune`] spares. None in an entry
    /// written without one.
    #[serde(default)]
    at: u64,
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

/// The file in the project at `root`.
pub fn path(root: &Path) -> PathBuf {
    crate::names::state(root).join(IN_STATE)
}

/// Whether `chat` is a chat identity as a canonical record spells one: no path, no space.
fn is_an_id(chat: &str) -> bool {
    !chat.is_empty()
        && chat.len() <= 64
        && chat
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn read(root: &Path) -> BTreeMap<String, Stored> {
    let path = path(root);
    // Never through a link, and never a folder or a device.
    let Ok(there) = std::fs::symlink_metadata(&path) else {
        return BTreeMap::new();
    };
    if !there.is_file() {
        return BTreeMap::new();
    }
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str::<BTreeMap<String, Stored>>(&text).ok())
        .unwrap_or_default()
}

fn write(root: &Path, kept: &BTreeMap<String, Stored>) -> io::Result<()> {
    let path = path(root);
    if kept.is_empty() {
        return match std::fs::remove_file(&path) {
            Err(none) if none.kind() == io::ErrorKind::NotFound => Ok(()),
            done => done,
        };
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let text = serde_json::to_string_pretty(kept).map_err(io::Error::other)?;
    crate::rewrite::replace(
        root,
        &path,
        format!("{text}\n").as_bytes(),
        crate::rewrite::Mode::Private,
    )
}

/// One read-modify-write of the file, alone: `change` returns whether it changed anything.
fn change(
    root: &Path,
    wait: Duration,
    change: impl FnOnce(&mut BTreeMap<String, Stored>) -> io::Result<bool>,
) -> io::Result<()> {
    let _alone = crate::filelock::beside(&path(root), wait, "the kept browser answers")?;
    let mut all = read(root);
    if change(&mut all)? {
        write(root, &all)?;
    }
    Ok(())
}

/// **The answer kept for chat `chat`**, where there is one that reads.
pub fn kept(root: &Path, chat: &str) -> Option<Kept> {
    if !is_an_id(chat) {
        return None;
    }
    read(root).get(chat).and_then(|one| {
        Some(Kept {
            answer: Answer::of(&one.answer)?,
            reach: one.reach.clone(),
        })
    })
}

/// **Keeps `answer` for chat `chat`**, replacing what it had, with the `reach` the chat had when
/// it was asked.
pub fn keep(root: &Path, chat: &str, answer: Answer, reach: &Reach) -> io::Result<()> {
    keep_within(root, chat, answer, reach, WAIT)
}

fn keep_within(
    root: &Path,
    chat: &str,
    answer: Answer,
    reach: &Reach,
    wait: Duration,
) -> io::Result<()> {
    if !is_an_id(chat) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a chat's identity is letters, digits, - and _",
        ));
    }
    change(root, wait, |all| {
        if all.len() >= MOST && !all.contains_key(chat) {
            return Err(io::Error::other("too many chats' answers are kept"));
        }
        all.insert(
            chat.to_owned(),
            Stored {
                answer: answer.word().to_owned(),
                reach: reach.clone(),
                at: now_secs(),
            },
        );
        Ok(true)
    })
}

/// **Takes chat `chat`'s answer away**: it was not used (the reach it was given for narrowed), or
/// the chat is no more. Never a chat's close. Nothing to remove is no failure.
pub fn remove(root: &Path, chat: &str) -> io::Result<()> {
    change(root, WAIT, |all| Ok(all.remove(chat).is_some()))
}

/// **Drops the answers of chats that no longer exist** (`exists` says which do), at start: a
/// chat whose record purlis let go while the app was not running left its answer behind. The
/// caller counts a closed chat, and a chat open in another window, as existing. **An answer given
/// at `since` or after is spared**: it may be a chat another window started that no record names
/// yet. Answers how many went.
pub fn prune(root: &Path, exists: impl Fn(&str) -> bool, since: u64) -> io::Result<usize> {
    let mut gone = 0;
    // Nothing kept, no file and no lock file to make.
    if read(root).is_empty() {
        return Ok(0);
    }
    change(root, WAIT, |all| {
        let before = all.len();
        all.retain(|chat, one| one.at >= since || exists(chat));
        gone = before - all.len();
        Ok(gone > 0)
    })?;
    Ok(gone)
}

#[cfg(test)]
mod tests;
