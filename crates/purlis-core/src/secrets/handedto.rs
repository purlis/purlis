// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The secrets purlis handed each chat, known well enough to mask them** (#6309, parent
//! #5820): what a dispatched task's kept screen ([`crate::chatscreens`]) is masked with before
//! it is written, so a kept screen never holds one of purlis's own secrets.
//!
//! The person ruled it on 2026-11-15: every vault value purlis handed the chat and every
//! credential it brokered for it, matched exactly, each shown as a named mask,
//! `‹secret: github-token›` ([`mask`]), so *Read it* says what was there without showing it.
//!
//! - **Never the value itself**, as [`super::seen`] keeps none: each is held as its entry's name,
//!   its length and its keyed digest under [`super::seen`]'s key for the process, so nothing here
//!   can give a value back, in a crash report or anywhere else.
//! - **Per chat, by project and chat number**: what the app's own code knows of the chat asking
//!   (`brokered::Asker`), remembered as each value is handed ([`remember`]) and forgotten when
//!   the chat closes for good ([`forget`]).
//! - **And named on disk, never a value**: each chat's entries, by vault and key, in
//!   `<state>/app/chat-handed/<chat>.names` ([`dir`]), so a launch after a crash, which kept
//!   nothing in memory of what the chat was handed before, reads those entries again to mask its
//!   screen ([`masked_for`]). Clone state, denied to every sandboxed chat, gone as the chat
//!   closes for good, and swept at the project's open once 30 days old ([`on_open`]).
//! - **Matched in the text a screen shows, not the sequences that draw it** ([`masked`]): a
//!   value a colour change or a soft wrap runs through is still one run of characters there.
//!   A hard line break is not: a value the program printed in two pieces is two words. A value
//!   of several lines is known by each of its lines too ([`Known::of`]), so it is masked
//!   however a program lays its lines out.
//! - **Every overlapping find masked whole**: two values that overlap on a screen become one
//!   stretch, shown as the mask of each, in the order they start, so no tail of either is left.
//! - **A value shorter than [`super::seen::SHORTEST`] is not masked**: as `seen` says, three
//!   characters or fewer is an ordinary word as often as a secret.
//! - **It fails closed**: a screen with a value handed that cannot be known is not maskable
//!   ([`masked_for`] answers `None`), so it is not kept. That is where no key could be made for
//!   the process, and where an entry named on disk cannot be read again without asking the
//!   person ([`read_again`]): a vault whose provider asks macOS, an entry gone or changed, and a
//!   GitHub App token, which is minted and never read.
//! - **What purlis never handed it cannot mask**: a value a `purlis secret exec` read itself
//!   where no app answered, and anything the chat found on its own.

use std::collections::{BTreeSet, HashMap};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

use super::seen::{self, Digest};

/// How a masked value is shown: its entry's name in angle quotes, never its value.
pub fn mask(name: &str) -> String {
    format!("‹secret: {name}›")
}

/// One secret handed to a chat, as it is known: its entry's name, and its length and digest.
#[derive(Clone)]
pub struct Known {
    name: String,
    length: usize,
    digest: Digest,
}

impl Known {
    /// `value`, handed as `name`, as it is known: as written and with the white space around it
    /// trimmed, which a value read from a file often carries. A value of several lines is known
    /// as a terminal shows it too, its lines ended by a carriage return and a line feed, and by
    /// each line alone, trimmed, as a program that prints it line by line, or indents it, shows
    /// it. Any form shorter than [`seen::SHORTEST`] is left out; `None` where no key could be
    /// made for the process.
    pub fn of(name: &str, value: &str) -> Option<Vec<Self>> {
        let keyed = seen::keyed()?;
        let trimmed = value.trim();
        let mut forms: Vec<zeroize::Zeroizing<String>> = [value, trimmed]
            .into_iter()
            .map(|one| zeroize::Zeroizing::new(one.to_owned()))
            .collect();
        if trimmed.contains('\n') {
            let lines: Vec<&str> = trimmed
                .split('\n')
                .map(|line| line.trim_end_matches('\r'))
                .collect();
            forms.push(zeroize::Zeroizing::new(lines.join("\r\n")));
            forms.extend(
                lines
                    .iter()
                    .map(|line| zeroize::Zeroizing::new(line.trim().to_owned())),
            );
        }
        let mut known: Vec<Self> = Vec::new();
        for one in &forms {
            let bytes = one.as_bytes();
            if bytes.len() < seen::SHORTEST {
                continue;
            }
            let digest = seen::digest(keyed, bytes);
            if known.iter().any(|k| k.digest == digest) {
                continue;
            }
            known.push(Self {
                name: name.to_owned(),
                length: bytes.len(),
                digest,
            });
        }
        Some(known)
    }
}

/// What a chat was handed, or that something was handed it that could not be known.
#[derive(Clone, Default)]
struct Handed {
    known: Vec<Known>,
    /// The entries `known` holds, by vault and key: named on disk once, and not read again.
    entries: BTreeSet<Entry>,
    blind: bool,
}

/// A vault's entry, by its vault and its key: what [`dir`] names, never a value.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct Entry {
    pub vault: String,
    pub key: String,
}

impl Entry {
    pub fn new(vault: &str, key: &str) -> Self {
        Self {
            vault: vault.to_owned(),
            key: key.to_owned(),
        }
    }
}

static HANDED: Mutex<Option<HashMap<(PathBuf, u32), Handed>>> = Mutex::new(None);

fn handed() -> MutexGuard<'static, Option<HashMap<(PathBuf, u32), Handed>>> {
    HANDED.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The store's folder under the state folder's `app/`.
pub const DIR_NAME: &str = "chat-handed";

const SUFFIX: &str = ".names";

/// The store of the project at `root`: each chat's entries, by its number.
pub fn dir(root: &Path) -> PathBuf {
    crate::names::state(root).join("app").join(DIR_NAME)
}

fn file(root: &Path, chat: u32) -> PathBuf {
    dir(root).join(format!("{chat}{SUFFIX}"))
}

/// **Remembers that chat `chat` of the project at `root` was handed `value`, the entry `key` of
/// vault `vault`**: from now on its kept screen masks it ([`masked_for`]).
pub fn remember(root: &Path, chat: u32, vault: &str, key: &str, value: &str) {
    remember_known(root, chat, &[Entry::new(vault, key)], Known::of(key, value));
}

/// **Remembers that chat `chat` of the project at `root` was handed `entries`, known as
/// `known`**, or, for `None`, as something that could not be known ([`Known::of`]): for values
/// read before the chat had its number, as a profile's sign-in is. Each entry not named for the
/// chat before is named on disk ([`dir`]), best effort: one that cannot be leaves only a
/// launch after a crash without it.
pub fn remember_known(root: &Path, chat: u32, entries: &[Entry], known: Option<Vec<Known>>) {
    let mut handed = handed();
    let one = handed
        .get_or_insert_with(HashMap::new)
        .entry((root.to_path_buf(), chat))
        .or_default();
    match known {
        Some(known) => one.known.extend(known),
        None => one.blind = true,
    }
    let new: Vec<Entry> = entries
        .iter()
        .filter(|entry| one.entries.insert((*entry).clone()))
        .cloned()
        .collect();
    drop(handed);
    if !new.is_empty()
        && let Err(why) = name_on_disk(root, chat, new)
    {
        tracing::info!("purlis: what chat {chat} was handed was not named on disk ({why})");
    }
}

/// `new` added to what chat `chat` has named on disk, the file written whole and private.
fn name_on_disk(root: &Path, chat: u32, new: Vec<Entry>) -> std::io::Result<()> {
    let mut all = named(root, chat);
    for entry in new {
        if !all.contains(&entry) {
            all.push(entry);
        }
    }
    let mut text = String::new();
    for entry in &all {
        text.push_str(&serde_json::to_string(entry).map_err(std::io::Error::other)?);
        text.push('\n');
    }
    crate::keptscreen::made_private(&dir(root))?;
    crate::rewrite::replace(
        &dir(root),
        &file(root, chat),
        text.as_bytes(),
        crate::rewrite::Mode::Private,
    )
}

/// **Forgets what chat `chat` of the project at `root` was handed**, as it closes for good: a
/// chat number is given again to a later chat.
pub fn forget(root: &Path, chat: u32) {
    if let Some(handed) = handed().as_mut() {
        handed.remove(&(root.to_path_buf(), chat));
    }
    match std::fs::remove_file(file(root, chat)) {
        Err(why) if why.kind() != std::io::ErrorKind::NotFound => {
            tracing::info!("purlis: what chat {chat} was handed is still named on disk ({why})");
        }
        _ => {}
    }
}

/// The entries named on disk for chat `chat`, read through no link: a line that does not read
/// is skipped.
fn named(root: &Path, chat: u32) -> Vec<Entry> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    let mut text = String::new();
    let read = crate::contain::nofollow(&mut options)
        .open(file(root, chat))
        .and_then(|mut opened| std::io::Read::read_to_string(&mut opened, &mut text));
    if read.is_err() {
        return Vec::new();
    }
    text.lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

/// **Sweeps the store at the project's open**: a chat's names last written
/// [`crate::retention::KEEP_FOR`] or more before go, as a task's screen and its record do.
pub fn on_open(root: &Path, now: std::time::SystemTime) {
    let Ok(entries) = std::fs::read_dir(dir(root)) else {
        return;
    };
    for entry in entries.flatten() {
        let old = entry
            .metadata()
            .and_then(|meta| meta.modified())
            .ok()
            .and_then(|written| now.duration_since(written).ok())
            .is_some_and(|age| age >= crate::retention::KEEP_FOR);
        if old && entry.file_name().to_string_lossy().ends_with(SUFFIX) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// What reads an entry's value again: its vault and key, to the value, or `None`.
pub type ReadAgain<'a> = &'a dyn Fn(&Entry) -> Option<zeroize::Zeroizing<String>>;

/// **Reads an entry of the project at `root` again, where that asks nobody**: through its vault's
/// provider, the read every hand-out makes ([`super::cmd::get_value`]), where opening the vault
/// reads no secret ([`super::reads_on_open`]: a keyring, plain-file or reference vault). `None`
/// for any other provider, which macOS may ask the person about, and for an entry gone, or one
/// never read raw (a GitHub App's key).
pub fn read_again(root: &Path) -> impl Fn(&Entry) -> Option<zeroize::Zeroizing<String>> + '_ {
    move |entry| {
        let ctx = super::Ctx::new(root, super::Env::from_process());
        let vault = super::cmd::provider(&ctx, &entry.vault).ok()?;
        if super::reads_on_open(&vault.provider) {
            return None;
        }
        super::cmd::get_value(&ctx, &vault, &entry.key)
            .ok()
            .map(zeroize::Zeroizing::new)
    }
}

/// **`screen`, what chat `chat` of the project at `root` showed, masked** with every secret it
/// was handed ([`masked`]): those this process knows, and each entry named on disk that it does
/// not, read again by `read` (a launch after a crash). `None` where one could not be known.
pub fn masked_for(root: &Path, chat: u32, screen: &str, read: ReadAgain<'_>) -> Option<String> {
    let one = handed()
        .as_ref()
        .and_then(|handed| handed.get(&(root.to_path_buf(), chat)).cloned())
        .unwrap_or_default();
    if one.blind {
        return None;
    }
    let mut known = one.known;
    for entry in named(root, chat) {
        if one.entries.contains(&entry) {
            continue;
        }
        let value = read(&entry)?;
        known.extend(Known::of(&entry.key, &value)?);
    }
    masked(screen, &known)
}

/// **`screen` with each of `known` it shows replaced by its [`mask`]**, the sequences that draw
/// it left as they were. `None` where no key could be made for the process to look with.
///
/// A string sequence's payload (a window title, a link's address) is text a reader may be
/// shown too, so it is looked in as text, and a value there is masked in it.
pub fn masked(screen: &str, known: &[Known]) -> Option<String> {
    if known.is_empty() {
        return Some(screen.to_owned());
    }
    let keyed = seen::keyed()?;
    let raw = screen.as_bytes();
    let pieces = pieces(raw);
    // The text the screen shows, and where in `screen` each of its bytes stands.
    let at: Vec<usize> = pieces
        .iter()
        .filter(|piece| piece.shown)
        .flat_map(|piece| piece.bytes.clone())
        .collect();
    let shown: Vec<u8> = at.iter().map(|&i| raw[i]).collect();

    let mut by_length: HashMap<usize, Vec<&Known>> = HashMap::new();
    for one in known {
        by_length.entry(one.length).or_default().push(one);
    }
    let mut found: Vec<(Range<usize>, &str)> = Vec::new();
    for (length, of_it) in &by_length {
        for (start, run) in shown.windows(*length).enumerate() {
            let digest = seen::digest(keyed, run);
            for one in of_it.iter().filter(|one| one.digest == digest) {
                found.push((start..start + length, one.name.as_str()));
            }
        }
    }
    let spans = joined(found);

    let mut out: Vec<u8> = Vec::with_capacity(raw.len());
    let mut spans = spans.iter().peekable();
    let mut index = 0;
    for piece in &pieces {
        if !piece.shown {
            out.extend_from_slice(&raw[piece.bytes.clone()]);
            continue;
        }
        for i in piece.bytes.clone() {
            while spans.next_if(|(span, _)| span.end <= index).is_some() {}
            match spans.peek() {
                Some((span, names)) if span.start == index => {
                    for name in names {
                        out.extend_from_slice(mask(name).as_bytes());
                    }
                }
                Some((span, _)) if span.contains(&index) => {}
                _ => out.push(raw[i]),
            }
            index += 1;
        }
    }
    Some(String::from_utf8_lossy(&out).into_owned())
}

/// `found` in order, each run of finds that overlap joined into one stretch, named by each
/// entry found in it once, in the order they start.
fn joined(mut found: Vec<(Range<usize>, &str)>) -> Vec<(Range<usize>, Vec<&str>)> {
    found.sort_by_key(|(span, name)| (span.start, std::cmp::Reverse(span.end), *name));
    let mut joined: Vec<(Range<usize>, Vec<&str>)> = Vec::new();
    for (span, name) in found {
        match joined.last_mut() {
            Some((last, names)) if span.start < last.end => {
                last.end = last.end.max(span.end);
                if !names.contains(&name) {
                    names.push(name);
                }
            }
            _ => joined.push((span, vec![name])),
        }
    }
    joined
}

/// A stretch of a screen's bytes: text it shows, or a sequence that draws it.
struct Piece {
    bytes: Range<usize>,
    shown: bool,
}

/// `raw` cut into the text it shows and the escape sequences that draw it (ECMA-48): a control
/// sequence (`ESC [` … final byte), a string sequence's introducer and terminator (`ESC ]`,
/// `ESC P`, `ESC X`, `ESC ^`, `ESC _` … `BEL` or `ESC \`) with its payload as text, and any
/// other escape with its intermediates and final byte. Every other byte is shown, a line break
/// among them.
fn pieces(raw: &[u8]) -> Vec<Piece> {
    const ESC: u8 = 0x1b;
    const BEL: u8 = 0x07;
    let mut pieces: Vec<Piece> = Vec::new();
    let mut push = |bytes: Range<usize>, shown: bool| {
        if bytes.is_empty() {
            return;
        }
        match pieces.last_mut() {
            Some(last) if last.shown == shown && last.bytes.end == bytes.start => {
                last.bytes.end = bytes.end;
            }
            _ => pieces.push(Piece { bytes, shown }),
        }
    };
    let mut i = 0;
    while i < raw.len() {
        if raw[i] != ESC {
            push(i..i + 1, true);
            i += 1;
            continue;
        }
        let start = i;
        i += 1;
        match raw.get(i) {
            Some(b'[') => {
                i += 1;
                while i < raw.len() && !(0x40..=0x7e).contains(&raw[i]) {
                    i += 1;
                }
                i = (i + 1).min(raw.len());
                push(start..i, false);
            }
            Some(b']' | b'P' | b'X' | b'^' | b'_') => {
                i += 1;
                push(start..i, false);
                let payload = i;
                while i < raw.len() && raw[i] != BEL && raw[i] != ESC {
                    i += 1;
                }
                push(payload..i, true);
                let end = match raw.get(i) {
                    Some(&BEL) => i + 1,
                    Some(&ESC) if raw.get(i + 1) == Some(&b'\\') => i + 2,
                    _ => i,
                };
                push(i..end, false);
                i = end;
            }
            Some(_) => {
                while i < raw.len() && (0x20..=0x2f).contains(&raw[i]) {
                    i += 1;
                }
                i = (i + 1).min(raw.len());
                push(start..i, false);
            }
            None => push(start..i, false),
        }
    }
    pieces
}

#[cfg(test)]
mod tests;
