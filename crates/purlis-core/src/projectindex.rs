// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The project index (ADR 0079, #5498): a derived SQLite FTS5 index of one project clone's
//! memory bases, session records and todos, so `recall` answers without reading every file, and
//! of its workspaces' pinned artifacts (ADR 0122, #5968), so a search finds what a page says.
//!
//! **Never the truth.** Deleting it costs a rebuild and nothing else: every reader has a correct
//! answer without it, by scanning the files ([`crate::recall::scan`]). A schema or rules version
//! it did not write is discarded and built again, never migrated.
//!
//! **One writer.** The session host, `purlisd`, builds and keeps it ([`Keeper`]), holding
//! `writer.lock`. Every other process opens it read-only ([`Reader`]), and the CLI never writes
//! one: with no host running it reads the index as the host last left it, or scans.
//!
//! **Where:** `<state>/index/<clone-key>/` ([`home`]): a `current` file naming the live
//! generation, and one directory per generation holding `search.sqlite` in WAL mode. A full
//! build writes a new generation beside the live one and swaps `current` in one rename, so a
//! reader sees the old index or the new one, never half of either.
//!
//! **What it holds** ([`words`]): each item's row (path, the base it was listed from, its kind
//! and owner, its audience, its title, date, size, modification time and content hash), and a
//! contentless FTS5 table of its words. Every text passes the shape scanner first, so a span it
//! flags is indexed as its kind and the value never reaches the file. The words are purlis's own
//! word split, the one the file scan matches by, so a prefix query finds exactly the memories the
//! scan finds.
//!
//! **What it answers** is never wider than the files: the writer reads only through
//! [`crate::memstore::read_files`]' gate, or a pinned artifact's own ([`crate::artifact::search`]),
//! so a link out of the project, or out of an artifact's folder, is never indexed; and every
//! query names the caller's reach ([`Reach`]), which the FTS query itself filters on.

use std::path::{Path, PathBuf};

mod keeper;
mod reader;
mod sources;
mod words;
mod writer;

pub use keeper::Keeper;
pub use reader::{Freshness, Reader, Row, WorkspaceSummary};
pub use sources::{Kind, Reach};
pub use writer::Writer;

/// The schema this build writes and reads. A generation recording any other is discarded.
pub const SCHEMA: &str = "2";

/// The file naming the live generation.
const CURRENT: &str = "current";
/// Each generation's database.
const DB: &str = "search.sqlite";
/// The writer's lock.
const LOCK: &str = "writer.lock";

/// Why the index could not be written or read.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("the project index could not be used: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("the project index could not be used: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

/// The index's folder for the clone at `root`: `<state>/index/<clone-key>/`, where `<state>` is
/// the clone's state folder or `$PURLIS_HOME` (ADR 0079 §8).
pub fn home(root: &Path) -> PathBuf {
    crate::plane::state_dir(root)
        .join("index")
        .join(crate::plane::CloneKey::of(root).as_str())
}

/// Whether the host is building the project's first index: a writer holds `writer.lock`, and
/// there is no generation a reader can open yet. The briefing then says so, and walks nothing
/// in its place (#5503).
pub fn being_built(root: &Path) -> bool {
    reader::writer_holds(&home(root)) && Reader::open(root).is_none()
}

/// The version of what an index is built with besides its schema: the shape scanner's rules and
/// the word split. An index built with other rules is rebuilt, so a rule added to the scanner
/// reaches every span it now flags (ADR 0079 §6).
pub fn rules_version() -> String {
    use sha2::Digest;
    let mut hash = sha2::Sha256::new();
    for (kind, rule) in crate::secretshape::leak_rules() {
        hash.update(kind.as_bytes());
        hash.update([0]);
        hash.update(rule.as_bytes());
        hash.update([0]);
    }
    hash.update(words::VERSION.as_bytes());
    let mut out = crate::extension::hex(&hash.finalize());
    out.truncate(16);
    out
}

/// `text` as the index holds it: scanned for secrets ([`words`]), then split into purlis's
/// words. A scan that must find what the index finds splits a text with this (#5968).
pub(crate) fn words_of(text: &str) -> Vec<String> {
    crate::memstore::tokens(&words::scanned(text))
}

/// `text` with every span shaped like a secret replaced by its kind, as a title is kept.
pub(crate) fn scanned(text: &str) -> String {
    words::scanned(text)
}

/// The live generation's database under `home`, as `current` names it.
fn current_db(home: &Path) -> Option<PathBuf> {
    let name = std::fs::read_to_string(home.join(CURRENT)).ok()?;
    let name = name.trim();
    // A generation is a plain name inside `home`, never a path that leaves it.
    if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
        return None;
    }
    Some(home.join(name).join(DB))
}

/// Seconds since the epoch, now.
fn now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64())
}
