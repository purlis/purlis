// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The slow-answer and stall log (#4725): the window's slow moments, kept so a slow moment the
//! person felt can be read afterwards.
//!
//! The window writes two kinds (`app/src/slowLog.ts`, through the app's `record_slow`):
//!
//! - **an answer** to one of its commands that took 1 s or longer, with the command, how long it
//!   took, when it was asked and how many chats were in their turn;
//! - **a stall** of its own thread of 200 ms or longer (a gap between two frames), with how many
//!   chats were in their turn, the tab in front and whether a view of a chat was opening.
//!
//! And the core writes two of its own (`app/src-tauri/src/mainwatch.rs`), so a slow answer can
//! be told apart as waiting in line or work (felt speed round 2, H6b):
//!
//! - **a core stall**: the core's main thread, which every window command enters on, did not
//!   run a ping for 200 ms or longer, with the command it was running when that was noticed;
//! - **a core command**: a command that held the main thread for 100 ms or longer, with when
//!   the core received it. An answer's `at` (asked) beside its command's `at` (received) is its
//!   time in line; its `ms` here is its work there.
//!
//! # Where it lives
//!
//! [`FILE`] in the app's log folder ([`crate::applog`]'s, beside `panics.log` and the
//! diagnostic log; `$PURLIS_LOG_DIR` moves it), one JSON object a line, oldest first, the
//! newest [`KEPT`] kept. Machine, device-bound, transient (ADR 0069; `docs/plane-format.md`):
//! never in a plane, never sent, and nothing is lost but a record when it is deleted. Not the
//! audit and not telemetry (O1, ADR 0075): nothing reads it but a person, through [`recent`].
//!
//! **Never a chat's content.** An entry is the fields of [`Slow`] and nothing else: a
//! command's name and never its arguments, a tab's name and never what it shows.

use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

/// The log's file name in the app's log folder.
pub const FILE: &str = "slow.jsonl";

/// How many entries are kept, the newest.
pub const KEPT: usize = 1_000;

/// One slow moment the window felt, as a line of the log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Slow {
    /// An answer to `command` (the window's name for it) that took `ms` milliseconds, asked at
    /// `at` (milliseconds since 1970), while `hot` chats were in their turn.
    Answer {
        command: String,
        ms: u32,
        at: u64,
        hot: u32,
    },
    /// A gap of `ms` milliseconds between two of the window's frames, seen at `at`, while `hot`
    /// chats were in their turn, `front` the name of the tab in front (if any), and `opening`
    /// whether a view of a chat was being opened.
    ///
    /// And what the window's thread did in that frame (felt speed round 4, H9), the largest
    /// few of each: `react`, the regions whose React commits took longest; `events`, the core's
    /// events it handled, by name and count; `answers`, the commands whose answers it handled;
    /// `scripts`, the scripts the webview said made the frame long, where it says. Empty in a
    /// line written before they were kept.
    ///
    /// `dark`: the window's thread answered all through the gap, so its frames were withheld (a
    /// locked screen, a display asleep, a window covered), not held up. Time away, not a stall
    /// the person felt; `false` in a line written before it was kept.
    Stall {
        ms: u32,
        at: u64,
        hot: u32,
        front: Option<String>,
        opening: bool,
        #[serde(default)]
        dark: bool,
        #[serde(default)]
        react: Vec<Took>,
        #[serde(default)]
        events: Vec<Counted>,
        #[serde(default)]
        answers: Vec<Counted>,
        #[serde(default)]
        scripts: Vec<Took>,
    },
    /// The core's main thread ran a ping `ms` milliseconds after it was asked to, at `at`, and
    /// `running` is the window command it was running when the wait passed 200 ms, or `None`
    /// when it was busy with something that is not a command (an event, a terminal's output).
    CoreStall {
        ms: u32,
        at: u64,
        running: Option<String>,
    },
    /// The window command `command`, received on the core's main thread at `at`, held it for
    /// `ms` milliseconds before it went back to the window's next ask.
    CoreCommand { command: String, ms: u32, at: u64 },
}

/// Something the window's thread did in a stalled frame, by its name, and the milliseconds it
/// took there. A region's name or a script's, never what it showed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Took {
    pub name: String,
    pub ms: u32,
}

/// Something the window handled `n` times in a stalled frame, by its name: an event's or a
/// command's, never what it carried.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Counted {
    pub name: String,
    pub n: u32,
}

/// Where the log is on this machine, or `None` with no home directory to keep it under.
pub fn path() -> Option<PathBuf> {
    path_in(crate::applog::dir())
}

fn path_in(dir: Option<PathBuf>) -> Option<PathBuf> {
    dir.map(|dir| dir.join(FILE))
}

/// Appends `slow` to the log on this machine ([`path`]).
pub fn record(slow: &Slow) -> io::Result<()> {
    let file = path().ok_or_else(|| io::Error::other("no home directory to keep it under"))?;
    record_in(&file, slow)
}

/// One writer at a time in this process, so a cut to [`KEPT`] never loses a line appended
/// while it was being made.
static WRITING: Mutex<()> = Mutex::new(());

/// Appends `slow` to the log at `file`, making it and its folder when they are not there, and
/// keeps only the newest [`KEPT`] entries.
///
/// Appended while the log is under [`KEPT`]; once it is full, written again whole, through a
/// file beside it that takes its place, so a reader never finds half a log. At most a few
/// hundred kilobytes, and only on a slow moment.
pub fn record_in(file: &Path, slow: &Slow) -> io::Result<()> {
    let _one = WRITING.lock().unwrap_or_else(|held| held.into_inner());
    let mut line = serde_json::to_string(slow).map_err(io::Error::other)?;
    line.push('\n');
    let dir = file.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir)?;
    let was = match std::fs::read_to_string(file) {
        Ok(was) => was,
        Err(err) if err.kind() == io::ErrorKind::NotFound => String::new(),
        Err(err) => return Err(err),
    };
    let lines = was.lines().count();
    if lines < KEPT {
        let mut log = OpenOptions::new().create(true).append(true).open(file)?;
        // A log whose last line was cut short gets its own line back first.
        if !was.is_empty() && !was.ends_with('\n') {
            log.write_all(b"\n")?;
        }
        return log.write_all(line.as_bytes());
    }
    let mut kept = String::with_capacity(was.len());
    for one in was.lines().skip(lines + 1 - KEPT) {
        kept.push_str(one);
        kept.push('\n');
    }
    kept.push_str(&line);
    let mut next = tempfile::NamedTempFile::new_in(dir)?;
    next.write_all(kept.as_bytes())?;
    next.persist(file).map_err(|err| err.error)?;
    Ok(())
}

/// **The recent entries on this machine, newest last**, or none when there is no log: what a
/// person's diagnostics read (#4774).
pub fn recent() -> Vec<Slow> {
    path().map(|file| recent_in(&file)).unwrap_or_default()
}

/// The entries of the log at `file`, newest last. A line that is not an entry, one cut short by
/// a process that ended mid-write, is passed over.
pub fn recent_in(file: &Path) -> Vec<Slow> {
    let Ok(text) = std::fs::read_to_string(file) else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

#[cfg(test)]
mod tests;
