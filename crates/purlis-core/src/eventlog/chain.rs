// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The event log's hash chain, and its head sealed at each append** (M75 slice 13, spec §10.3;
//! review round 1, S5; review round 2, S2-SF5; review round 3, SEC3-SF5).
//!
//! A chat at the sandbox level `off` can reach the log's files, so it could rewrite them. When the
//! host chains the log ([`super::Log::chain`]), every line it writes ends in the chain of the line
//! before it: `chain = SHA-256(previous chain, this line without its chain)`. An edited, dropped
//! or inserted line then breaks every chain after it.
//!
//! **A bare chain can be recomputed** by whoever rewrites it, so the app also seals the chain's
//! head, its `seq` and chain, in the keyring ([`crate::secrets::seal::keep`], kind
//! [`HEAD_KIND`], keyed by the device) at each append: the manifest's anchor for this log. A
//! recomputed chain ends in a head the seal does not hold. [`check`] reads the log against both,
//! at each chat's start and whenever doctor runs, and names the first line that no longer
//! matches.
//!
//! **What is written:** the chain is the line's last key, `,"chain":"<64 hex>"}`, so a reader that
//! knows nothing of it reads the same event, and the chain is taken over the line as it was
//! before the key was added. **Once chained, always chained:** a log whose last line is chained
//! stays chained when it is opened again, and a line with no chain after one that has it is a
//! break, so turning the chain off is not a way to write unseen.
//!
//! **What is sealed** (M75-13 round 2A): two items, both kept by the app in the keyring and keyed
//! by the device. The **head** ([`HEAD_KIND`]) is the last line's `seq` and chain, sealed again at
//! each append. The **start** ([`START_KIND`]) is the first chained line's `seq` and chain, sealed
//! once where the chain begins and again only where the log's own retention deletes its oldest
//! segments ([`reanchor`]), the one legitimate way a chain's start moves. So deleting or
//! rewriting the lines before the head reads as a break at the start, and a log deleted whole, or
//! with every `chain` key stripped, reads as a break because a seal is kept for a chain that is
//! gone.
//!
//! **Who seals.** Only the app's own binary can ([`crate::secrets::seal`]): the desktop's session
//! host is that binary started again, and marks itself the app before it writes the log. A
//! `purlis host` with no app beside it (a runner, a standalone host) cannot seal, and the check
//! says [`Checked::Unsealed`] there, never that the log is sound.
//!
//! **A writer verifies before it carries on** ([`opened`]): a chained log opened again is read
//! against the seals, and one that does not match keeps its lines chained but is never sealed
//! again, so a forged tail is not adopted as the head and the check keeps reporting the break.
//!
//! **One seal per device covers the machine-wide log.** The log is one per device, whatever
//! project or chat wrote a line, so the seals cover every project's events at once, and a
//! rewrite of one project's lines breaks the chain for all of them.
//!
//! **Where it is weaker:** the keyring keeps one head, so a chain that was recomputed from a line
//! on is told from the sealed head's line, not from the edited one. A crash between an append
//! and its seal leaves the sealed head one line behind the log's: that single line is told from
//! tampering as [`Checked::Stale`] (a warning), at the price that a line forged onto the tail
//! reads the same until the next append seals it. And on Linux and Windows any of the person's
//! programs can read the keyring, so there the seal stops a hand edit and a sandboxed chat, no
//! more.

use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

use sha2::{Digest, Sha256};

use super::{Event, FILE, segments};

/// The kind of the seal the app keeps of a chained log's head ([`crate::secrets::seal::keep`]).
pub const HEAD_KIND: &str = "eventlog-head";

/// The kind of the seal the app keeps of a chained log's start, its first chained line
/// ([`crate::secrets::seal::keep`]).
pub const START_KIND: &str = "eventlog-start";

/// The key a chained line ends in.
const KEY: &str = ",\"chain\":\"";

/// How long a chain is, in hex characters.
const HEX: usize = 64;

/// **The chain of a line** that follows `previous`: the SHA-256 of both, each length-prefixed so
/// no line can pass for a longer chain.
pub fn link(previous: &str, line: &[u8]) -> String {
    let mut hash = Sha256::new();
    for part in [previous.as_bytes(), line] {
        hash.update((part.len() as u64).to_be_bytes());
        hash.update(part);
    }
    crate::extension::hex(&hash.finalize())
}

/// **`line` (an event as JSON, with no newline) with its chain added**, and the chain.
pub fn chained(previous: &str, mut line: Vec<u8>) -> (Vec<u8>, String) {
    let chain = link(previous, &line);
    // An event serialises as a JSON object, which ends in `}`: the key goes before it.
    let closing = line.pop();
    debug_assert_eq!(closing, Some(b'}'), "an event is an object");
    line.extend_from_slice(KEY.as_bytes());
    line.extend_from_slice(chain.as_bytes());
    line.extend_from_slice(b"\"}");
    (line, chain)
}

/// **A line split into what its chain was taken over and the chain**, or none for a line that
/// carries none.
pub fn split(line: &[u8]) -> Option<(Vec<u8>, &str)> {
    let end = line.strip_suffix(b"\"}")?;
    let at = end.len().checked_sub(HEX)?;
    let (before, chain) = end.split_at(at);
    let before = before.strip_suffix(KEY.as_bytes())?;
    let chain = std::str::from_utf8(chain).ok()?;
    if !chain.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let mut over = before.to_vec();
    over.push(b'}');
    Some((over, chain))
}

/// **The text a head is sealed as**: its `seq` and its chain.
pub fn head_text(seq: u64, chain: &str) -> String {
    format!("{seq} {chain}")
}

/// **Seals the head** `chain` at `seq` for the device's log: `false` where this process cannot
/// seal (it is not the app, or the keyring cannot be read), which leaves the chain alone.
pub fn seal_head(device: &str, seq: u64, chain: &str) -> bool {
    crate::secrets::seal::keep(HEAD_KIND, device, &head_text(seq, chain))
}

/// **Seals the start**, the first chained line `chain` at `seq`, for the device's log: `false`
/// where this process cannot seal.
pub fn seal_start(device: &str, seq: u64, chain: &str) -> bool {
    crate::secrets::seal::keep(START_KIND, device, &head_text(seq, chain))
}

/// **The `seq` and chain of the log's first chained line**, from the oldest segment on: where the
/// chain starts as the log holds it now. `None` where no line is chained.
pub fn first_chain(dir: &Path) -> io::Result<Option<(u64, String)>> {
    let mut paths: Vec<std::path::PathBuf> = segments(dir)?.into_iter().map(|(_, p)| p).collect();
    paths.push(dir.join(FILE));
    for path in paths {
        let text = match std::fs::read(&path) {
            Ok(text) => text,
            Err(why) if why.kind() == io::ErrorKind::NotFound => continue,
            Err(why) => return Err(why),
        };
        for line in text.split(|byte| *byte == b'\n') {
            if let (Ok(event), Some((_, chain))) =
                (serde_json::from_slice::<Event>(line), split(line))
            {
                return Ok(Some((event.seq, chain.to_owned())));
            }
        }
    }
    Ok(None)
}

/// **Moves the sealed start to where the log now starts**, after the log's own retention deleted
/// its oldest segments: the one legitimate move of a chain's start. `false` where nothing was
/// sealed (this process cannot, or no line is chained). Called only by the writer that deleted
/// them ([`super::Log`]), never by a reader, so a deletion made by anyone else stays a break.
pub fn reanchor(dir: &Path, device: &str) -> bool {
    match first_chain(dir) {
        Ok(Some((seq, chain))) => seal_start(device, seq, &chain),
        _ => false,
    }
}

/// **The chain and `seq` of the log's last chained line**, read from the end of the live
/// segment or, where it holds none, of the newest sealed one: where a writer carries on.
pub fn last_chain(dir: &Path) -> io::Result<Option<(u64, String)>> {
    let mut paths: Vec<std::path::PathBuf> = vec![dir.join(FILE)];
    paths.extend(segments(dir)?.into_iter().rev().map(|(_, path)| path));
    for path in paths {
        let mut file = match std::fs::File::open(&path) {
            Ok(file) => file,
            Err(why) if why.kind() == io::ErrorKind::NotFound => continue,
            Err(why) => return Err(why),
        };
        let len = file.metadata()?.len();
        file.seek(SeekFrom::Start(len.saturating_sub(super::TAIL)))?;
        let mut tail = Vec::new();
        file.read_to_end(&mut tail)?;
        let found = tail.split(|byte| *byte == b'\n').rev().find_map(|line| {
            let event = serde_json::from_slice::<Event>(line).ok()?;
            Some((event.seq, split(line)?.1.to_owned()))
        });
        if found.is_some() {
            return Ok(found);
        }
        // A last line that is an event with no chain ends the search here: the log is not
        // chained to its end, and `check` says so.
        if tail
            .split(|byte| *byte == b'\n')
            .any(|line| serde_json::from_slice::<Event>(line).is_ok())
        {
            return Ok(None);
        }
    }
    Ok(None)
}

/// **What [`check`] found.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Checked {
    /// The log holds no chained line, and no seal is kept for one: nothing was chained, so
    /// nothing is claimed.
    NotChained,
    /// Every chain holds, and the start and head are the ones the app sealed.
    Sealed {
        /// The head's `seq`.
        seq: u64,
    },
    /// Every chain holds, and the head is the line before the one the log ends in: a crash
    /// between an append and its seal, or a line forged onto the tail. The next append seals.
    Stale {
        /// The log's last `seq`, one past the sealed head.
        seq: u64,
    },
    /// Every chain holds, and nothing could be checked against a seal: this process is not the
    /// app, or none was ever kept. **Never an all-clear.**
    Unsealed {
        /// The head's `seq`.
        seq: u64,
    },
    /// **The log is not as it was written**: the first line that no longer matches.
    Broken {
        /// That line's `seq`; 0 where the whole chain is gone ([`Why::ChainIsGone`]).
        seq: u64,
        /// What does not match.
        why: Why,
    },
}

/// Why a line no longer matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Why {
    /// Its chain is not the chain of the line before it and its own text.
    ChainDoesNotHold,
    /// It carries no chain, after one that does.
    NotChained,
    /// Its `seq` does not follow the line before it: a line was dropped or inserted.
    OutOfOrder,
    /// Every chain holds, and the head is not the one the app sealed: the chain was recomputed,
    /// or the log's tail was cut or replaced.
    HeadIsNotTheSealed,
    /// The chain's first line is not the one the app sealed as its start: the lines before the
    /// head were deleted or rewritten.
    StartIsNotTheSealed,
    /// A seal is kept for a chained log and the log holds no chained line: it was deleted, or
    /// every `chain` key was stripped from it.
    ChainIsGone,
}

impl Checked {
    /// **Whether the log is not as it was written.**
    pub fn is_broken(&self) -> bool {
        matches!(self, Checked::Broken { .. })
    }

    /// **The sentence doctor and the start's Notice say** of a break, naming the log and the first
    /// line that no longer matches; none where the log holds.
    pub fn said(&self, dir: &Path) -> Option<String> {
        let Checked::Broken { seq, why } = self else {
            return None;
        };
        if *why == Why::ChainIsGone {
            return Some(format!(
                "The event log at {} was changed after purlis wrote it: purlis sealed its hash \
                 chain, and the log holds none, so it was deleted or every chain was stripped. \
                 Check what ran with the sandbox off.",
                dir.display()
            ));
        }
        let what = match why {
            Why::ChainDoesNotHold => "its chain does not hold",
            Why::NotChained => "it carries no chain after one that does",
            Why::OutOfOrder => "a line before it was dropped or added",
            Why::HeadIsNotTheSealed => {
                "the log's end is not the one purlis sealed, so the chain was rewritten or cut"
            }
            Why::StartIsNotTheSealed => {
                "the log's start is not the one purlis sealed, so the lines before the end were \
                 deleted or rewritten"
            }
            // Said above, whole.
            Why::ChainIsGone => "its chain is gone",
        };
        Some(format!(
            "The event log at {} was changed after purlis wrote it: line {seq} no longer \
             matches, because {what}. Check what ran with the sandbox off.",
            dir.display()
        ))
    }

    /// **What doctor and the start say where the log is not broken but is not proven either**:
    /// a head one line behind the log's, and a chain no seal could be checked against. None for
    /// a log that holds and one that was never chained.
    pub fn warned(&self, dir: &Path) -> Option<String> {
        match self {
            Checked::Stale { seq } => Some(format!(
                "The event log at {} ends at line {seq}, one past the head purlis sealed: \
                 purlis stopped between writing that line and sealing it, or a line was added to \
                 the end. The next line purlis writes seals it.",
                dir.display()
            )),
            Checked::Unsealed { seq } => Some(format!(
                "The event log at {} is hash-chained to line {seq}, and nothing could be checked \
                 against a seal: only purlis's app seals the head, and it has not (or its \
                 keyring could not be read). The chain alone can be rewritten.",
                dir.display()
            )),
            _ => None,
        }
    }
}

/// **Checks the log in `dir`** (the device's own directory) against its chain and the start and
/// head the app sealed for `device`: every chained line is read in order, and the first that no
/// longer matches is named. A chat's start and doctor run it.
pub fn check(dir: &Path, device: &str) -> io::Result<Checked> {
    use crate::secrets::seal;
    let mut paths: Vec<std::path::PathBuf> = segments(dir)?.into_iter().map(|(_, p)| p).collect();
    paths.push(dir.join(FILE));
    let mut first: Option<(u64, String)> = None;
    let mut before_last: Option<(u64, String)> = None;
    let mut previous: Option<(u64, String)> = None;
    for path in paths {
        let text = match std::fs::read(&path) {
            Ok(text) => text,
            Err(why) if why.kind() == io::ErrorKind::NotFound => continue,
            Err(why) => return Err(why),
        };
        for line in text.split(|byte| *byte == b'\n') {
            let Ok(event) = serde_json::from_slice::<Event>(line) else {
                continue;
            };
            let split = split(line);
            let Some((seq_before, chain_before)) = previous.clone() else {
                // The chain starts at its first chained line, which has nothing before it to be
                // checked against: the sealed start is what vouches for it. A line before it is
                // from before the chain.
                if let Some((_, chain)) = split {
                    first = Some((event.seq, chain.to_owned()));
                    previous = Some((event.seq, chain.to_owned()));
                }
                continue;
            };
            if event.seq != seq_before + 1 {
                return Ok(Checked::Broken {
                    seq: event.seq,
                    why: Why::OutOfOrder,
                });
            }
            let Some((over, chain)) = split else {
                return Ok(Checked::Broken {
                    seq: event.seq,
                    why: Why::NotChained,
                });
            };
            if link(&chain_before, &over) != chain {
                return Ok(Checked::Broken {
                    seq: event.seq,
                    why: Why::ChainDoesNotHold,
                });
            }
            before_last = Some((seq_before, chain_before));
            previous = Some((event.seq, chain.to_owned()));
        }
    }
    let (Some((first_seq, first_chain)), Some((seq, chain))) = (first, previous) else {
        // No chained line. A seal kept for one says the log was chained and is no longer.
        return Ok(if seal::kept_exists(HEAD_KIND, device) {
            Checked::Broken {
                seq: 0,
                why: Why::ChainIsGone,
            }
        } else {
            Checked::NotChained
        });
    };
    if seal::kept_holds(START_KIND, device, &head_text(first_seq, &first_chain)) == Some(false) {
        return Ok(Checked::Broken {
            seq: first_seq,
            why: Why::StartIsNotTheSealed,
        });
    }
    match seal::kept_holds(HEAD_KIND, device, &head_text(seq, &chain)) {
        Some(true) => Ok(Checked::Sealed { seq }),
        Some(false) => {
            let stale = before_last.is_some_and(|(before, chain)| {
                seal::kept_holds(HEAD_KIND, device, &head_text(before, &chain)) == Some(true)
            });
            Ok(if stale {
                Checked::Stale { seq }
            } else {
                Checked::Broken {
                    seq,
                    why: Why::HeadIsNotTheSealed,
                }
            })
        }
        None => Ok(Checked::Unsealed { seq }),
    }
}

/// **[`check`], read only where the log is chained or a seal says it was**: the tail of the log
/// and one keyring item are looked at first, so a log that was never chained costs a chat's start
/// no more than that.
pub fn check_if_chained(dir: &Path, device: &str) -> io::Result<Checked> {
    if last_chain(dir)?.is_none() && !crate::secrets::seal::kept_exists(HEAD_KIND, device) {
        return Ok(Checked::NotChained);
    }
    check(dir, device)
}

#[cfg(any(test, feature = "test-support"))]
thread_local! {
    /// A test's log folder, for what a chat's start looks at.
    static TEST_DIR: std::cell::RefCell<Option<std::path::PathBuf>> =
        const { std::cell::RefCell::new(None) };
}

/// **A test's machine log folder** for this thread: where [`at_chat_start`] looks.
#[cfg(any(test, feature = "test-support"))]
pub fn log_dir_for_this_thread(dir: Option<std::path::PathBuf>) {
    TEST_DIR.with(|held| *held.borrow_mut() = dir);
}

/// This machine's log folder, where a chat's start looks.
fn machine_log_dir() -> Option<std::path::PathBuf> {
    #[cfg(any(test, feature = "test-support"))]
    if let Some(dir) = TEST_DIR.with(|held| held.borrow().clone()) {
        return Some(dir);
    }
    super::Recorder::dir_to_look_at()
}

/// **What a chat's start says of the machine's event log** (spec §10.3, M75-13 round 2A, B2): the
/// sentence of a break, or of a head one line behind, to show on the chat's tab as a Notice; none
/// where the log holds, was never chained, or cannot be read. [`crate::start::ready`] calls it
/// for every chat it readies.
pub fn at_chat_start() -> Option<String> {
    let dir = machine_log_dir()?;
    let device = dir.file_name()?.to_str()?.to_owned();
    let checked = check_if_chained(&dir, &device).ok()?;
    checked.said(&dir).or_else(|| match checked {
        Checked::Stale { .. } => checked.warned(&dir),
        _ => None,
    })
}

/// **What a writer finds as it opens a chained log** ([`super::Log`]): the log is read against
/// its seals, and one that is broken is not sealed again. `true` where the writer may seal.
pub fn opened(dir: &Path, device: &str) -> bool {
    match check_if_chained(dir, device) {
        Ok(checked) if checked.is_broken() => {
            tracing::warn!(
                "purlis: {} The log keeps its lines chained and is not sealed again, so the \
                 break stays reported.",
                checked.said(dir).unwrap_or_default()
            );
            false
        }
        Ok(checked) => {
            // A chain sealed before the start was (an older purlis, or one first sealed by an
            // app that adopted an unsealed log) has its start sealed where it stands now.
            if !matches!(checked, Checked::NotChained)
                && !crate::secrets::seal::kept_exists(START_KIND, device)
                && let Ok(Some((seq, chain))) = first_chain(dir)
            {
                seal_start(device, seq, &chain);
            }
            true
        }
        // A log that cannot be read to be checked is not one to vouch for.
        Err(why) => {
            tracing::warn!(
                "purlis: the event log's chain could not be checked as it opened ({why}), so it \
                 is not sealed again"
            );
            false
        }
    }
}

#[cfg(test)]
#[path = "chain_tests.rs"]
mod tests;
