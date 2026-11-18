// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What a chat's browser may reach, as the person answered it, kept for the chat's life**
//! (#7190 item 1, the person's ruling of 2026-11-18): `app/browser-postures.json` in the
//! project's state folder.
//!
//! A chat's first browser call asks the person what its browser may reach. The answer used to
//! be held in memory, so every restart asked again. It is now kept for **that chat only**, by its
//! identity (ADR 0066), and it ends with the chat ([`remove`]):
//!
//! - **This machine's, never the plane**: under `.purlis/app/`, which is gitignored and never
//!   synced. No sandboxed chat writes it: `.purlis/app/` is the integrity class's.
//! - **Per chat**: a new chat has a new identity and finds no answer, so it asks. There is no
//!   answer "for every chat of this persona"; that is what the persona's Always is.
//! - **Only what the person pressed**: an answer that rests on a persona's Always is not kept
//!   (it follows that Always as it is now).
//! - **A resumed chat keeps its answer**: the same identity finds it again.
//! - **Narrowing is separate**: the browsers a narrowed reach ends are the app's act, and the
//!   answer stands, as before.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

/// The file, relative to the project's state folder ([`path`]).
pub const IN_STATE: &str = "app/browser-postures.json";

/// The most answers kept: a chat's answer ends with it, so this is a bound on a damaged file.
const MOST: usize = 4096;

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

/// One writer at a time in this process: every project's holder rewrites the whole file.
static WRITING: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn read(root: &Path) -> BTreeMap<String, String> {
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
        .and_then(|text| serde_json::from_str::<BTreeMap<String, String>>(&text).ok())
        .unwrap_or_default()
}

fn write(root: &Path, kept: &BTreeMap<String, String>) -> io::Result<()> {
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

/// **The answer kept for chat `chat`**, where there is one that reads.
pub fn kept(root: &Path, chat: &str) -> Option<Answer> {
    if !is_an_id(chat) {
        return None;
    }
    read(root).get(chat).and_then(|word| Answer::of(word))
}

/// **Keeps `answer` for chat `chat`**, replacing what it had.
pub fn keep(root: &Path, chat: &str, answer: Answer) -> io::Result<()> {
    if !is_an_id(chat) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a chat's identity is letters, digits, - and _",
        ));
    }
    let _one = WRITING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut all = read(root);
    if all.len() >= MOST && !all.contains_key(chat) {
        return Err(io::Error::other("too many chats' answers are kept"));
    }
    all.insert(chat.to_owned(), answer.word().to_owned());
    write(root, &all)
}

/// **The chat closed: its answer goes with it.** Nothing to remove is no failure.
pub fn remove(root: &Path, chat: &str) -> io::Result<()> {
    let _one = WRITING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut all = read(root);
    if all.remove(chat).is_none() {
        return Ok(());
    }
    write(root, &all)
}

#[cfg(test)]
mod tests;
