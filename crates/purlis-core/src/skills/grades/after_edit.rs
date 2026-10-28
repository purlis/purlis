// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Grade after each edit, as an opt-in** (SK-58, #3238; spec #2762 decision 22, "optionally
//! after each edit, never on a schedule"; ADR 0105 §4).
//!
//! A grade spends the person's account, so grading after each edit is **off for every skill
//! until the person turns it on for that skill, on this machine, from a grade's plan**: the plan
//! shows the chats it runs and the dollar limit it is held to, and turning it on keeps exactly
//! those as what the person accepted ([`Accepted`]). Each saved edit after that grades the skill
//! on the profile and model accepted, held to the same cap and dollar limit as a grade pressed by
//! hand, and only while what it would run stays within what was accepted ([`next`]): a grade that
//! would run more chats, or may spend more, is not started, and the opt-in waits until the
//! person sees the cost again and turns it on again. Nothing grades on a timer, and an edit
//! whose copy was graded already is not graded twice.
//!
//! **Tier:** device-bound. It lives in this machine's state, `<state>/skill-grade-after-edit.json`
//! ([`FILE`]), which no chat writes and no commit carries: the person who pays turns it on, and a
//! teammate's push never does. Read through no link, at most [`MOST_FILE_BYTES`], and a record
//! that does not hold together is read as off.

use std::collections::BTreeMap;
use std::io::Read as _;
use std::path::{Path, PathBuf};

use super::{Level, cap, plain};

/// The file in this machine's state that holds each skill's opt-in.
pub const FILE: &str = "skill-grade-after-edit.json";

/// The version of the file this purlis writes and reads.
const VERSION: u32 = 1;

/// The most bytes of the file read: far past any machine's opt-ins.
const MOST_FILE_BYTES: u64 = 256 * 1024;

/// The most skills one project keeps an opt-in for.
const MOST_SKILLS: usize = 512;

/// The most chats an opt-in may have accepted: past any plan's.
const MOST_CHATS: u32 = 100_000;

/// **What the person accepted when they turned grading after each edit on**: the profile and
/// model its chats run on, the chats a grade of it ran then, and the cap and dollar limit it was
/// held to, as the plan they pressed it on said them. A grade after an edit runs only within
/// them ([`next`]).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Accepted {
    pub profile: String,
    /// The model picked, or `None` for the profile's own.
    #[serde(default)]
    pub model: Option<String>,
    pub chats: u32,
    pub token_cap: u64,
    pub limit_usd: f64,
    /// When it was turned on.
    pub at: String,
    /// What the copy held when an edit last graded it, by its plan's consent: a save that
    /// changed nothing since is not graded again.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graded: Option<String>,
    /// Why it waits for the person, where it does: a grade after an edit would have run more
    /// than they accepted, so it was not started.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paused: Option<String>,
}

impl Accepted {
    /// Whether it says only what purlis writes.
    fn sound(&self) -> bool {
        plain(&self.profile)
            && self.model.as_deref().is_none_or(plain)
            && (1..=MOST_CHATS).contains(&self.chats)
            && (cap::LEAST..=cap::MOST).contains(&self.token_cap)
            && self.limit_usd.is_finite()
            && self.limit_usd > 0.0
            && self
                .graded
                .as_deref()
                .is_none_or(|word| word.len() == 64 && word.bytes().all(|b| b.is_ascii_hexdigit()))
            && self
                .paused
                .as_deref()
                .is_none_or(|why| why.len() <= 1024 && !why.chars().any(char::is_control))
    }
}

/// The file, as it is written.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct Kept {
    v: u32,
    #[serde(default)]
    skills: BTreeMap<String, Accepted>,
}

/// Where the file of the project at `root` is.
fn path_of(root: &Path) -> PathBuf {
    crate::plane::state_dir(root).join(FILE)
}

/// **The copy a skill's opt-in is kept by**: its level's folder relative to the project, and its
/// name (`skills/deploy`). `None` for a copy no folder of the project holds.
pub fn key(level: &Level, name: &str) -> Option<String> {
    level.rel().map(|rel| format!("{rel}/{name}"))
}

/// The file as it reads: nothing where it is absent, not a file of its own, too large, or not
/// what purlis writes.
fn read(root: &Path) -> Kept {
    let path = path_of(root);
    let Ok(meta) = std::fs::symlink_metadata(&path) else {
        return Kept::default();
    };
    if !meta.is_file() || meta.len() > MOST_FILE_BYTES {
        return Kept::default();
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    let Ok(file) = crate::contain::nofollow(&mut options).open(&path) else {
        return Kept::default();
    };
    let mut text = String::new();
    if file
        .take(MOST_FILE_BYTES)
        .read_to_string(&mut text)
        .is_err()
    {
        return Kept::default();
    }
    match serde_json::from_str::<Kept>(&text) {
        Ok(kept) if kept.v == VERSION && kept.skills.len() <= MOST_SKILLS => Kept {
            v: VERSION,
            skills: kept
                .skills
                .into_iter()
                .filter(|(_, accepted)| accepted.sound())
                .collect(),
        },
        _ => Kept::default(),
    }
}

/// Writes `kept` as the file, whole or not at all, private, through no link.
fn write(root: &Path, kept: &Kept) -> Result<(), String> {
    let path = path_of(root);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|why| not_kept(&why.to_string()))?;
    }
    let text = serde_json::to_string_pretty(kept).map_err(|why| not_kept(&why.to_string()))?;
    crate::plane::write_private(root, &path, text.as_bytes())
        .map_err(|why| not_kept(&why.to_string()))
}

/// What a write that failed says.
fn not_kept(why: &str) -> String {
    format!(
        "Grading after each edit could not be kept on this machine ({}), so nothing was changed.",
        crate::shown::short(why)
    )
}

/// **The opt-in of the copy of `name` at `level`** in the project at `root`, where it is on.
pub fn of(root: &Path, level: &Level, name: &str) -> Option<Accepted> {
    let key = key(level, name)?;
    read(root).skills.remove(&key)
}

/// **Turns grading after each edit on for the copy of `name` at `level`**, with `accepted`: what
/// the plan the person pressed it on showed. Refused for a copy no folder holds, and for what
/// purlis would not read back.
pub fn turn_on(root: &Path, level: &Level, name: &str, accepted: Accepted) -> Result<(), String> {
    let key = key(level, name).ok_or_else(|| {
        format!("{name} is no copy of this project's, so it cannot be graded after each edit.")
    })?;
    if !accepted.sound() {
        return Err(format!(
            "What a grade of {name} runs could not be kept as accepted, so grading after each \
             edit was not turned on."
        ));
    }
    let mut kept = read(root);
    if !kept.skills.contains_key(&key) && kept.skills.len() >= MOST_SKILLS {
        return Err(format!(
            "This project grades {MOST_SKILLS} skills after each edit already: turn one off \
             first."
        ));
    }
    kept.v = VERSION;
    kept.skills.insert(key, accepted);
    write(root, &kept)
}

/// **Turns grading after each edit off for the copy of `name` at `level`**: nothing where it was
/// off.
pub fn turn_off(root: &Path, level: &Level, name: &str) -> Result<(), String> {
    let Some(key) = key(level, name) else {
        return Ok(());
    };
    let mut kept = read(root);
    if kept.skills.remove(&key).is_none() {
        return Ok(());
    }
    kept.v = VERSION;
    write(root, &kept)
}

/// **Changes the opt-in of the copy of `name` at `level` by `change`**, where it is on: what a
/// grade after an edit keeps (what it graded, or why it waits). Nothing where it is off.
pub fn update(
    root: &Path,
    level: &Level,
    name: &str,
    change: impl FnOnce(&mut Accepted),
) -> Result<(), String> {
    let Some(key) = key(level, name) else {
        return Ok(());
    };
    let mut kept = read(root);
    let Some(accepted) = kept.skills.get_mut(&key) else {
        return Ok(());
    };
    change(accepted);
    kept.v = VERSION;
    write(root, &kept)
}

/// **What a grade after an edit would run now**: the chats of the copy's plan, the cap and the
/// dollar limit it would be held to, and its consent, the digest of what it would grade.
#[derive(Debug, Clone, PartialEq)]
pub struct Now<'a> {
    pub chats: u32,
    pub token_cap: u64,
    pub limit_usd: f64,
    pub consent: &'a str,
}

/// What a saved edit does under an opt-in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Next {
    /// It grades the copy.
    Grade,
    /// The copy holds what an edit last graded: nothing runs.
    Same,
    /// It waits for the person, and why: what it would run is past what they accepted, or it
    /// waits already.
    Waits(String),
}

/// **What a saved edit does under `accepted`, with `now` what a grade would run**: it grades
/// only while the grade runs no more chats, and is held to no larger a cap or dollar limit,
/// than the person accepted; never twice for the same copy; and not while the opt-in waits for
/// the person.
pub fn next(accepted: &Accepted, now: &Now<'_>) -> Next {
    if let Some(why) = &accepted.paused {
        return Next::Waits(why.clone());
    }
    if accepted.graded.as_deref() == Some(now.consent) {
        return Next::Same;
    }
    if now.chats > accepted.chats {
        return Next::Waits(format!(
            "A grade of it now runs {}, more than the {} you accepted, so no grade was started \
             after your edit: see the cost again and turn grading after each edit on again.",
            crate::counts::counted(now.chats, "chat", "chats"),
            accepted.chats
        ));
    }
    // A cent's rounding past what was shown is still what was shown.
    if now.token_cap > accepted.token_cap || now.limit_usd > accepted.limit_usd + 0.005 {
        return Next::Waits(format!(
            "A grade of it may now spend up to ${:.2} ({}), more than the ${:.2} you accepted, so \
             no grade was started after your edit: see the cost again and turn grading after \
             each edit on again.",
            now.limit_usd,
            crate::counts::counted(now.token_cap, "token", "tokens"),
            accepted.limit_usd
        ));
    }
    Next::Grade
}

#[cfg(test)]
mod tests;
