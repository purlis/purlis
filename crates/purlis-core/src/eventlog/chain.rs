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
//! **Where it is weaker:** the keyring keeps one head, so a chain that was recomputed from a line
//! on is told from the sealed head's line, not from the edited one; and the first line a prune
//! left has no line before it to be chained to, so it is taken as the chain's start. Only the app
//! can seal, and on Linux and Windows any of the person's programs can read the keyring.

use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

use sha2::{Digest, Sha256};

use super::{Event, FILE, segments};

/// The kind of the seal the app keeps of a chained log's head ([`crate::secrets::seal::keep`]).
pub const HEAD_KIND: &str = "eventlog-head";

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
    /// The log holds no chained line: nothing was chained, so nothing is claimed.
    NotChained,
    /// Every chain holds, and the head is the one the app sealed.
    Sealed {
        /// The head's `seq`.
        seq: u64,
    },
    /// Every chain holds, and nothing could be checked against a seal: this process is not the
    /// app, or none was ever kept.
    Unsealed {
        /// The head's `seq`.
        seq: u64,
    },
    /// **The log is not as it was written**: the first line that no longer matches.
    Broken {
        /// That line's `seq`.
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
        let what = match why {
            Why::ChainDoesNotHold => "its chain does not hold",
            Why::NotChained => "it carries no chain after one that does",
            Why::OutOfOrder => "a line before it was dropped or added",
            Why::HeadIsNotTheSealed => {
                "the log's end is not the one purlis sealed, so the chain was rewritten or cut"
            }
        };
        Some(format!(
            "The event log at {} was changed after purlis wrote it: line {seq} no longer \
             matches, because {what}. Check what ran with the sandbox off.",
            dir.display()
        ))
    }
}

/// **Checks the log in `dir`** (the device's own directory) against its chain and the head the
/// app sealed for `device`: every chained line is read in order, and the first that no longer
/// matches is named. A chat's start and doctor run it.
pub fn check(dir: &Path, device: &str) -> io::Result<Checked> {
    let mut paths: Vec<std::path::PathBuf> = segments(dir)?.into_iter().map(|(_, p)| p).collect();
    paths.push(dir.join(FILE));
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
            let Some((before, seq_chain)) = previous.as_ref() else {
                // The chain starts at its first chained line, which has nothing before it to be
                // checked against; a line before it is from before the chain.
                if let Some((_, chain)) = split {
                    previous = Some((event.seq, chain.to_owned()));
                }
                continue;
            };
            let (seq_before, chain_before) = (*before, seq_chain.clone());
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
            previous = Some((event.seq, chain.to_owned()));
        }
    }
    let Some((seq, chain)) = previous else {
        return Ok(Checked::NotChained);
    };
    match crate::secrets::seal::kept_holds(HEAD_KIND, device, &head_text(seq, &chain)) {
        Some(false) => Ok(Checked::Broken {
            seq,
            why: Why::HeadIsNotTheSealed,
        }),
        Some(true) => Ok(Checked::Sealed { seq }),
        None => Ok(Checked::Unsealed { seq }),
    }
}

#[cfg(test)]
#[path = "chain_tests.rs"]
mod tests;
