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
//!   `<state>/app/chat-handed/<chat>.names` ([`dir`]), each with the form its value was handed
//!   in ([`Form`]), and, for each way the value may be shown ([`Known::of`]'s forms), its length,
//!   its print and its seal under the plane's key ([`Named`], [`Shown`]), so a launch after a
//!   crash, which kept nothing in memory of what the chat was handed before, finds each value on
//!   the chat's screen by those alone and masks it ([`masked_for`]), **reading no vault** (#6907,
//!   ADR 0126): no provider is asked, whether macOS may ask the person about it or not, and a
//!   value made once is found as well as one read. Clone state, denied to every sandboxed chat,
//!   gone as the chat closes for good, and swept at the project's open once 30 days old, but a
//!   chat's the plane will bring back ([`on_open`]).
//! - **Matched in the text a screen shows, not the sequences that draw it** ([`masked`]): a
//!   value a colour change or a soft wrap runs through is still one run of characters there.
//!   A hard line break is not: a value the program printed in two pieces is two words. A value
//!   of several lines is known by each of its lines too ([`Known::of`]), so it is masked
//!   however a program lays its lines out.
//! - **Every overlapping find masked whole**: two values that overlap on a screen become one
//!   stretch, shown as the mask of each, in the order they start, so no tail of either is left.
//! - **A value shorter than [`super::seen::SHORTEST`] is not masked**: as `seen` says, three
//!   characters or fewer is an ordinary word as often as a secret.
//! - **It fails closed**: a value whose entry cannot be named on disk is not handed
//!   ([`remember`]), and a screen with a value handed that cannot be known is not maskable
//!   ([`masked_for`] answers `None`), so it is not kept. That is where no key could be made for
//!   the process, where the names on disk do not read, where the plane has no key to check them
//!   with, and where a line there is not sealed by purlis as it stands.
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

/// One secret handed to a chat, as it is known: its entry's name, and its length, its digest
/// and its print ([`Print`]).
#[derive(Clone)]
pub struct Known {
    name: String,
    length: usize,
    digest: Digest,
    print: u64,
}

/// **A rolling hash of a run of bytes** (Rabin–Karp, modulo the Mersenne prime 2⁶¹−1), under a
/// base drawn from the process's key: what [`masked`] slides along a screen to find the few
/// windows worth a digest, one step a byte, so the cost of a screen is its length times the
/// number of lengths looked for, and not that times the lengths themselves. A print says no more
/// of a value than its digest does, to anyone without the key.
#[derive(Clone, Copy)]
struct Print {
    base: u64,
}

impl Print {
    const MODULUS: u64 = (1 << 61) - 1;

    /// The process's: `None` where no key could be made for it.
    fn of_the_process() -> Option<Self> {
        let digest = seen::digest(
            seen::keyed()?,
            b"purlis: the base a kept screen is printed by",
        );
        let drawn = u64::from_le_bytes(digest[..8].try_into().ok()?);
        Some(Self {
            base: drawn % (Self::MODULUS - 2) + 2,
        })
    }

    /// **A base drawn from `seal`**, 64 hex characters a seal of the plane's made (#6907): its
    /// first 64 bits, below the modulus and above 1. `None` for anything else.
    fn drawn(seal: &str) -> Option<Self> {
        let drawn = u64::from_str_radix(seal.get(..16)?, 16).ok()?;
        Some(Self {
            base: drawn % (Self::MODULUS - 2) + 2,
        })
    }

    /// `a` times `b`, each below the modulus: the product, under 2¹²², reduced as a Mersenne
    /// prime allows, its bits above the 61st added to those below.
    fn times(a: u64, b: u64) -> u64 {
        let product = u128::from(a) * u128::from(b);
        // Both halves are under 2⁶¹: the one by the mask, the other as the product's top.
        let (low, high) = ((product as u64) & Self::MODULUS, (product >> 61) as u64);
        Self::plus(low, high)
    }

    /// `a` plus `b`, each below the modulus.
    fn plus(a: u64, b: u64) -> u64 {
        let sum = a + b;
        if sum >= Self::MODULUS {
            sum - Self::MODULUS
        } else {
            sum
        }
    }

    /// `bytes`' print: each byte counted as one more than its value, so a run of zero bytes is
    /// not the empty run.
    fn of(self, bytes: &[u8]) -> u64 {
        bytes.iter().fold(0, |print, &byte| {
            Self::plus(Self::times(print, self.base), u64::from(byte) + 1)
        })
    }

    /// The print of every run of `length` bytes of `text`, by where it starts.
    fn rolled(self, text: &[u8], length: usize) -> impl Iterator<Item = u64> + '_ {
        let first = (length > 0 && length <= text.len()).then(|| self.of(&text[..length]));
        // What the byte leaving a window had counted for: the base to the power `length - 1`.
        let top = (1..length).fold(1, |power, _| Self::times(power, self.base));
        let mut print = first.unwrap_or_default();
        let windows = first.map_or(0, |_| text.len() - length + 1);
        (0..windows).map(move |start| {
            if start > 0 {
                let gone = Self::times(u64::from(text[start - 1]) + 1, top);
                let kept = Self::plus(print, Self::MODULUS - gone);
                print = Self::plus(
                    Self::times(kept, self.base),
                    u64::from(text[start + length - 1]) + 1,
                );
            }
            print
        })
    }
}

/// **The most window steps one screen is looked at with** ([`masked`]): its length times the
/// number of lengths looked for. A screen it would take more is not maskable, so it is not kept
/// (never kept unmasked): 4 MiB, the most a kept screen holds, against 64 lengths, which three
/// dozen values of several lines each come to.
pub const MATCH_STEPS_AT_MOST: usize = 4 * 1024 * 1024 * 64;

impl Known {
    /// `value`, handed as `name`, as it is known: as written and with the white space around it
    /// trimmed, which a value read from a file often carries. A value of several lines is known
    /// as a terminal shows it too, its lines ended by a carriage return and a line feed, and by
    /// each line alone, trimmed, as a program that prints it line by line, or indents it, shows
    /// it. Any form shorter than [`seen::SHORTEST`] is left out; `None` where no key could be
    /// made for the process.
    pub fn of(name: &str, value: &str) -> Option<Vec<Self>> {
        let keyed = seen::keyed()?;
        let printed = Print::of_the_process()?;
        Some(
            Self::forms(value)
                .iter()
                .map(|one| {
                    let bytes = one.as_bytes();
                    Self {
                        name: name.to_owned(),
                        length: bytes.len(),
                        digest: seen::digest(keyed, bytes),
                        print: printed.of(bytes),
                    }
                })
                .collect(),
        )
    }

    /// **Each way `value` may be shown**, as [`Self::of`] knows it, each once and none shorter
    /// than [`seen::SHORTEST`]: what a line on disk names too ([`Named`]).
    fn forms(value: &str) -> Vec<zeroize::Zeroizing<String>> {
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
        let mut once: Vec<zeroize::Zeroizing<String>> = Vec::new();
        for one in forms {
            if one.len() >= seen::SHORTEST && !once.contains(&one) {
                once.push(one);
            }
        }
        once
    }
}

/// What a chat was handed, or that something was handed it that could not be known.
#[derive(Clone, Default)]
struct Handed {
    known: Vec<Known>,
    /// The entries `known` holds, each with its value's seal: named on disk once, and not read
    /// again.
    entries: BTreeSet<Named>,
    blind: bool,
}

/// A vault's entry, by its vault and its key, and the form its value was handed in: what [`dir`]
/// names, never a value.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct Entry {
    pub vault: String,
    pub key: String,
    #[serde(default, skip_serializing_if = "Form::is_as_read")]
    pub form: Form,
}

impl Entry {
    /// The entry `key` of vault `vault`, its value handed as it was read.
    pub fn new(vault: &str, key: &str) -> Self {
        Self {
            vault: vault.to_owned(),
            key: key.to_owned(),
            form: Form::AsRead,
        }
    }

    /// This entry, its value handed in `form`.
    #[must_use]
    pub fn in_form(self, form: Form) -> Self {
        Self { form, ..self }
    }
}

/// The purpose a way a value may be shown is sealed for ([`Shown`]), and no other seal is.
const SEALED_FOR: &str = "a value handed to a chat";

/// The purpose a line's own seal ([`Named`]) is made for.
const LINE_SEALED_FOR: &str = "a line naming what a chat was handed";

/// The purpose the base each [`Shown`] is printed by is drawn for.
const BASE_DRAWN_FOR: &str = "the base a value handed to a chat is printed by";

/// **An entry as [`dir`] names it, and how to find the value it was handed** (#6907, ADR 0126):
/// for each way that value may be shown ([`Shown`]), its length, its print and its seal, and a
/// seal of the whole line, each under a key derived from the plane's
/// ([`super::fingerprint::seal`]), which lasts beyond the process and which no sandboxed chat
/// reads. Never the value: a launch after a crash finds it on a screen by its print and confirms
/// it by its seal, and reads no vault. A line whose own seal is not purlis's, as it stands,
/// leaves the screen unmaskable.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct Named {
    #[serde(flatten)]
    entry: Entry,
    /// Each way the value may be shown, in [`Known::of`]'s order: what draws each one's base.
    #[serde(default)]
    shown: Vec<Shown>,
    seal: String,
}

/// **One way a handed value may be shown, as a launch after a crash finds it** (#6907): its
/// length in bytes, its [`Print`] under a base of its own, drawn from the plane's key for this
/// entry and this place in its line (so no two prints, of related forms or of the same value
/// handed twice, share a base, and none says how two forms are related), and the seal of the
/// form itself, which a window whose print matches must seal the same as before it is masked.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct Shown {
    bytes: usize,
    /// 16 hex characters: the print, below 2⁶¹−1.
    print: String,
    seal: String,
}

/// What a plane with no key, and none to be made, is said as.
fn no_plane_key() -> std::io::Error {
    std::io::Error::other("the plane's key, which seals what a chat was handed, could not be made")
}

impl Named {
    /// `entry`, handed `value` (in its form) by the chat whose id is `chat`, as the project at
    /// `root` names it: an error where no seal can be made, as there is no plane key and none can
    /// be made. **Every seal in it covers `chat`** (#7129), so the line holds for that chat's
    /// screen alone, whatever number another chat is dealt later.
    pub fn of(root: &Path, chat: &str, entry: Entry, value: &str) -> std::io::Result<Self> {
        let ctx = super::Ctx::new(root, super::Env::from_process());
        let sealed = |purpose: &str, parts: &[&str]| {
            super::fingerprint::seal(&ctx, purpose, parts).ok_or_else(no_plane_key)
        };
        let mut shown = Vec::new();
        for form in Known::forms(value) {
            let place = shown.len().to_string();
            let base = Print::drawn(&sealed(
                BASE_DRAWN_FOR,
                &[chat, &entry.vault, &entry.key, &place],
            )?)
            .ok_or_else(no_plane_key)?;
            shown.push(Shown {
                bytes: form.len(),
                print: format!("{:016x}", base.of(form.as_bytes())),
                seal: sealed(SEALED_FOR, &[chat, &entry.vault, &entry.key, &form])?,
            });
        }
        let seal = sealed(
            LINE_SEALED_FOR,
            &Self::line_parts(chat, &entry, &shown)?.as_strs(),
        )?;
        Ok(Self { entry, shown, seal })
    }

    /// What a line's own seal covers: the chat it was written for, by its id, and its entry and
    /// each way its value may be shown, as JSON.
    fn line_parts(chat: &str, entry: &Entry, shown: &[Shown]) -> std::io::Result<Parts> {
        Ok(Parts(vec![
            chat.to_owned(),
            entry.vault.clone(),
            entry.key.clone(),
            serde_json::to_string(&(entry, shown)).map_err(std::io::Error::other)?,
        ]))
    }

    /// **What finds this line's value on the screen of the chat whose id is `chat`**, each way it
    /// may be shown with its base drawn again: `None` where the line is not sealed by purlis, as
    /// it stands and for that chat (#7129), or the plane has no key to tell.
    fn sought(&self, ctx: &super::Ctx, chat: &str) -> Option<Vec<Sought>> {
        let parts = Self::line_parts(chat, &self.entry, &self.shown).ok()?;
        let sealed = |purpose: &str, parts: &[&str]| {
            super::fingerprint::seal_with_the_key_there(ctx, purpose, parts)
        };
        if sealed(LINE_SEALED_FOR, &parts.as_strs())? != self.seal {
            return None;
        }
        self.shown
            .iter()
            .enumerate()
            .map(|(place, shown)| {
                let place = place.to_string();
                Some(Sought {
                    chat: chat.to_owned(),
                    name: self.entry.key.clone(),
                    vault: self.entry.vault.clone(),
                    length: shown.bytes,
                    print: u64::from_str_radix(&shown.print, 16).ok()?,
                    base: Print::drawn(&sealed(
                        BASE_DRAWN_FOR,
                        &[chat, &self.entry.vault, &self.entry.key, &place],
                    )?)?,
                    seal: shown.seal.clone(),
                })
            })
            .collect()
    }
}

/// The parts a seal covers, owned.
struct Parts(Vec<String>);

impl Parts {
    fn as_strs(&self) -> Vec<&str> {
        self.0.iter().map(String::as_str).collect()
    }
}

/// **One way a value named on disk may be shown, as a screen is looked at for it**: found where
/// a window of its length prints as it does under its base, and its seal is the window's.
struct Sought {
    chat: String,
    name: String,
    vault: String,
    length: usize,
    print: u64,
    base: Print,
    seal: String,
}

impl Sought {
    /// Whether `window`, which prints as this does, is the value: it seals the same.
    fn is(&self, ctx: &super::Ctx, window: &[u8]) -> bool {
        std::str::from_utf8(window).ok().is_some_and(|window| {
            super::fingerprint::seal_with_the_key_there(
                ctx,
                SEALED_FOR,
                &[&self.chat, &self.vault, &self.name, window],
            )
            .is_some_and(|seal| seal == self.seal)
        })
    }
}

/// **The form an entry's value was handed in**: a record of how the chat was handed it, which
/// keeps two hand-outs of one entry in different forms apart. Nothing in it is secret, and a
/// launch after a crash needs none of it to find the value (#6907): each [`Shown`] is of the
/// value as it was handed.
#[derive(
    Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum Form {
    /// As it was read.
    #[default]
    AsRead,
    /// As a dotenv file holds it ([`super::dotenv::escaped`]).
    DotenvEscaped,
    /// Pointed at a tunnel on the loopback's port `port` (#1667) for a client with `pgsslmode`
    /// and `reads_hostaddr` ([`crate::sandbox::tunnel::Client`]), dotenv-escaped too where
    /// `escaped`.
    Tunnelled {
        port: u16,
        pgsslmode: Option<String>,
        reads_hostaddr: bool,
        escaped: bool,
    },
    /// Made once and never again: a one-time code, a token minted with the entry.
    Unrepeatable,
}

impl Form {
    fn is_as_read(&self) -> bool {
        *self == Self::AsRead
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

/// **Remembers that chat `chat` of the project at `root`, whose id is `id`, was handed `value`,
/// `entry`'s value in its form**: from now on its kept screen masks it ([`masked_for`]). Called
/// before the value is handed: where its entry could not be named on disk, it answers why, and
/// the value is not handed, or a launch after a crash would keep the chat's screen unmasked. A
/// chat with no id (`None`) is handed nothing: what is named on disk is sealed for one chat
/// alone (#7129), and there would be none to seal it for.
pub fn remember(
    root: &Path,
    chat: u32,
    id: Option<&str>,
    entry: &Entry,
    value: &str,
) -> std::io::Result<()> {
    let id = id.ok_or_else(|| {
        std::io::Error::other("this chat has no id to seal what it was handed for")
    })?;
    remember_known(
        root,
        chat,
        &[Named::of(root, id, entry.clone(), value)?],
        Known::of(&entry.key, value),
    )
}

/// **Remembers that chat `chat` of the project at `root` was handed `entries`, each sealed with
/// its value ([`Named::of`]), known as `known`**, or, for `None`, as something that could not
/// be known ([`Known::of`]): for values read before the chat had its number, as a profile's
/// sign-in is. Each entry not named for the chat before is named on disk ([`dir`]); where that
/// fails, it answers why, as [`remember`] does, and the entries are named again at the next
/// hand-out.
pub fn remember_known(
    root: &Path,
    chat: u32,
    entries: &[Named],
    known: Option<Vec<Known>>,
) -> std::io::Result<()> {
    let key = (root.to_path_buf(), chat);
    let mut held = handed();
    let one = held
        .get_or_insert_with(HashMap::new)
        .entry(key.clone())
        .or_default();
    match known {
        Some(known) => one.known.extend(known),
        None => one.blind = true,
    }
    let new: Vec<Named> = entries
        .iter()
        .filter(|entry| one.entries.insert((*entry).clone()))
        .cloned()
        .collect();
    drop(held);
    if new.is_empty() {
        return Ok(());
    }
    name_on_disk(root, chat, &new).inspect_err(|_| {
        if let Some(one) = handed().as_mut().and_then(|handed| handed.get_mut(&key)) {
            for entry in &new {
                one.entries.remove(entry);
            }
        }
    })
}

/// `new` added to what chat `chat` has named on disk, the file written whole and private. A
/// file there that does not read is not written over: what it named would be lost.
fn name_on_disk(root: &Path, chat: u32, new: &[Named]) -> std::io::Result<()> {
    let mut all = named(root, chat)?;
    for entry in new {
        if !all.contains(entry) {
            all.push(entry.clone());
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

/// What this process holds of chat `chat` of the project at `root` let go of, its names left on
/// disk: as a crash leaves them.
#[cfg(test)]
pub(crate) fn as_a_crash_leaves_it(root: &Path, chat: u32) {
    if let Some(handed) = handed().as_mut() {
        handed.remove(&(root.to_path_buf(), chat));
    }
}

/// The entries named on disk for chat `chat`, read through no link: none where it has no file.
/// A file that cannot be read, or a line in it that does not parse, is an error: what it names
/// is not known.
fn named(root: &Path, chat: u32) -> std::io::Result<Vec<Named>> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    let mut text = String::new();
    match crate::contain::nofollow(&mut options)
        .open(file(root, chat))
        .and_then(|mut opened| std::io::Read::read_to_string(&mut opened, &mut text))
    {
        Ok(_) => {}
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(why) => return Err(why),
    }
    text.lines()
        .map(|line| serde_json::from_str(line).map_err(std::io::Error::other))
        .collect()
}

/// **Sweeps the store at the project's open**: a chat's names last written
/// [`crate::retention::KEEP_FOR`] or more before go, as a task's screen and its record do, but
/// never those of a chat in `live`, one the plane's reopen record brings back, whose screen is
/// still to be kept. `None` where that record cannot say which they are: then none goes.
pub fn on_open(root: &Path, now: std::time::SystemTime, live: Option<&[u32]>) {
    let Some(live) = live else {
        return;
    };
    let Ok(entries) = std::fs::read_dir(dir(root)) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(chat) = name
            .to_str()
            .and_then(|name| name.strip_suffix(SUFFIX))
            .and_then(|chat| chat.parse::<u32>().ok())
        else {
            continue;
        };
        let old = entry
            .metadata()
            .and_then(|meta| meta.modified())
            .ok()
            .and_then(|written| now.duration_since(written).ok())
            .is_some_and(|age| age >= crate::retention::KEEP_FOR);
        if old && !live.contains(&chat) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// **`screen`, what chat `chat` of the project at `root`, whose id is `id`, showed, masked** with
/// every secret it was handed ([`masked`]): those this process knows, and each entry named on
/// disk that it does not (a launch after a crash), found by what its line says of it ([`Named`])
/// and no vault read (#6907). `None` where one could not be known: a line on disk not sealed by
/// purlis, as it stands and for this chat (#7129), among them.
pub fn masked_for(root: &Path, chat: u32, id: &str, screen: &str) -> Option<String> {
    let one = handed()
        .as_ref()
        .and_then(|handed| handed.get(&(root.to_path_buf(), chat)).cloned())
        .unwrap_or_default();
    if one.blind {
        return None;
    }
    let ctx = super::Ctx::new(root, super::Env::from_process());
    let mut sought = Vec::new();
    for named in named(root, chat).ok()? {
        if !one.entries.contains(&named) {
            sought.extend(named.sought(&ctx, id)?);
        }
    }
    masked_with(screen, &one.known, &sought, Some(&ctx))
}

/// **`screen` with each of `known` it shows replaced by its [`mask`]**, the sequences that draw
/// it left as they were. `None` where no key could be made for the process to look with, and
/// where looking would take more than [`MATCH_STEPS_AT_MOST`].
///
/// A string sequence's payload (a window title, a link's address) is text a reader may be
/// shown too, so it is looked in as text, and a value there is masked in it.
pub fn masked(screen: &str, known: &[Known]) -> Option<String> {
    masked_with(screen, known, &[], None)
}

/// [`masked`], with each of `sought` too, confirmed under the plane `ctx` is of: what a launch
/// after a crash finds by its names on disk. Each of `sought` is looked for under a base of its
/// own, so it counts as a length of its own against [`MATCH_STEPS_AT_MOST`]; with no `ctx`,
/// none of them is ever found.
fn masked_with(
    screen: &str,
    known: &[Known],
    sought: &[Sought],
    ctx: Option<&super::Ctx>,
) -> Option<String> {
    if known.is_empty() && sought.is_empty() {
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
    if shown.len().saturating_mul(by_length.len() + sought.len()) > MATCH_STEPS_AT_MOST {
        return None;
    }
    let printed = Print::of_the_process()?;
    let mut found: Vec<(Range<usize>, &str)> = Vec::new();
    for (length, of_it) in &by_length {
        let prints: std::collections::HashSet<u64> = of_it.iter().map(|one| one.print).collect();
        for (start, print) in printed.rolled(&shown, *length).enumerate() {
            if !prints.contains(&print) {
                continue;
            }
            let digest = seen::digest(keyed, &shown[start..start + length]);
            for one in of_it.iter().filter(|one| one.digest == digest) {
                found.push((start..start + length, one.name.as_str()));
            }
        }
    }
    for one in sought {
        for (start, print) in one.base.rolled(&shown, one.length).enumerate() {
            if print == one.print
                && ctx.is_some_and(|ctx| one.is(ctx, &shown[start..start + one.length]))
            {
                found.push((start..start + one.length, one.name.as_str()));
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
