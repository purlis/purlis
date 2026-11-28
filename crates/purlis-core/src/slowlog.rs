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
//! And the core's answers, read from their spans ([`answers`], FS-M1a): each answer that took
//! 200 ms or longer, and an hourly summary of every answer by command.
//!
//! # Where it lives
//!
//! [`FILE`] in the app's log folder ([`crate::applog`]'s, beside `panics.log` and the
//! diagnostic log; `$PURLIS_LOG_DIR` moves it), one JSON object a line, oldest first. Each
//! [`Kind`] of line has a threshold and a cap of its own ([`Kind::keeping`]): a moment under its
//! kind's threshold is not written, and only the newest of each kind up to its cap are kept, so
//! a kind that comes often never cuts into the hours another covers (#7891). Machine, device-bound, transient (ADR 0069; `docs/plane-format.md`):
//! never in a plane, never sent, and nothing is lost but a record when it is deleted. Not the
//! audit and not telemetry (O1, ADR 0075): nothing reads it but a person, through [`recent`].
//!
//! **Never a chat's content.** An entry is the fields of [`Slow`] and nothing else: a
//! command's name and never its arguments, a tab's name and never what it shows.

use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

/// The log's file name in the app's log folder.
pub const FILE: &str = "slow.jsonl";

/// **Each kind of line the log keeps**, by the name it is written with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Answer,
    Stall,
    CoreStall,
    CoreCommand,
    Opening,
    Visibility,
    AnswerSpan,
    AnswerSummary,
}

/// What the log keeps of one [`Kind`]: a moment of it that took `from_ms` milliseconds or longer,
/// and its newest `cap`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Keeping {
    pub from_ms: u32,
    pub cap: usize,
}

impl Kind {
    /// **What the log keeps of this kind** (#7891).
    ///
    /// Until #7891 every kind shared one cut to the newest 1,000 lines, which on 2026-11-27 held
    /// 2.3 h with 14 to 56 hot chats; answers and stalls each keep that 1,000 now, so they cover
    /// at least the hours they covered then, whatever else is written. A threshold is the one
    /// its writer held to before the log held it; the window's own (`app/src/slowLog.ts`) is
    /// only there so a quick moment is not sent at all.
    pub const fn keeping(self) -> Keeping {
        let (from_ms, cap) = match self {
            Kind::Answer => (1_000, 1_000),
            Kind::Stall => (200, 1_000),
            Kind::CoreStall => (200, 300),
            Kind::CoreCommand => (100, 300),
            Kind::Opening => (1_000, 100),
            // Each change, whatever it took: a transition has no time of its own.
            Kind::Visibility => (0, 300),
            // A window read's p99 target (#7889), past which an answer is a miss worth a line.
            Kind::AnswerSpan => (200, 1_000),
            // One an hour and one as the app quits: over a week of them.
            Kind::AnswerSummary => (0, 200),
        };
        Keeping { from_ms, cap }
    }
}

/// One slow moment the window felt, as a line of the log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Slow {
    /// An answer to `command` (the window's name for it) that took `ms` milliseconds, asked at
    /// `at` (milliseconds since 1970), while `hot` chats were in their turn.
    ///
    /// `phases`: where the core times a command's work itself, each phase it passed through in
    /// the order it ran them, so a slow answer on another machine names its culprit (h19). The
    /// core writes such a line of its own beside the window's, whose `ms` also counts the time
    /// in line; empty in the window's line, and in a line written before they were kept.
    Answer {
        command: String,
        ms: u32,
        at: u64,
        hot: u32,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        phases: Vec<Took>,
    },
    /// A gap of `ms` milliseconds between two of the frames of the window labelled `window`, seen
    /// at `at`, while `hot` chats were in their turn, `front` the name of the tab in front (if
    /// any), and `opening` whether a view of a chat was being opened. `window` is empty in a line
    /// written before it was kept.
    ///
    /// And what the window's thread did in that frame (felt speed round 4, H9; #7891), so a
    /// stall's causes can be told apart: `render_ms`, the milliseconds React spent rendering the
    /// window's measured regions, against the gap's `ms` (the rest is commit, layout, paint, the
    /// terminals and the collector, which nothing names); and, the largest few of each,
    /// `react`, the regions whose renders took longest by their own time; `events`, the core's
    /// events it handled, each counted once; `listeners`, the calls of its listeners those
    /// events made; `answers`, the commands whose answers it handled; and `marks`, what
    /// happened in it that costs a frame (a chat's pane mounted, a tab switched). Empty, and
    /// `render_ms` 0, in a line written before they were kept.
    ///
    /// `dark`: the window's thread answered all through the gap, so its frames were withheld (a
    /// locked screen, a display asleep, or a window covered whole that WebKit did not report as
    /// hidden), not held up. Time away, not a stall the person felt; `false` in a line written
    /// before it was kept.
    ///
    /// The scripts the webview said made the frame long were kept until #7891: WebKit names
    /// none, so the field was always empty here, and a line that has it reads back without it.
    Stall {
        ms: u32,
        at: u64,
        #[serde(default)]
        window: String,
        hot: u32,
        front: Option<String>,
        opening: bool,
        #[serde(default)]
        dark: bool,
        #[serde(default)]
        render_ms: u32,
        #[serde(default)]
        react: Vec<Rendered>,
        #[serde(default)]
        events: Vec<Counted>,
        #[serde(default)]
        listeners: Vec<Counted>,
        #[serde(default)]
        answers: Vec<Counted>,
        #[serde(default)]
        marks: Vec<Counted>,
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
    /// A new chat's opening that took `ms` milliseconds from the picker's Start to its program
    /// being asked to run, begun at `at`, while `hot` chats were open, and the `phases` it
    /// passed through in the order it ran them (H12), so a report of "a new chat takes
    /// minutes" names the phase. A phase's name is this code's own, never a chat's content.
    Opening {
        ms: u32,
        at: u64,
        hot: u32,
        phases: Vec<Took>,
    },
    /// The window labelled `window` was visible (`visible`) or not at `at`: once as it starts
    /// watching, then at each change WebKit reports as a `visibilitychange` (#7891). On macOS it
    /// fires one for a window minimised or hidden, and, by WebKit's occlusion handling, for one
    /// covered whole by other windows, which is unverified until checked by hand; what WebKitGTK
    /// on Linux reports is not known. Read beside a stall of the same window, it says whether
    /// the person could see it. `window` is empty in a line written before it was kept.
    Visibility {
        at: u64,
        #[serde(default)]
        window: String,
        visible: bool,
    },
    /// The core's answer to the window command `command` of class `class` (the name the app's
    /// `answer_class` gives it, #7892), asked at `at`: `queued_ms` milliseconds waiting for a
    /// thread to do it on, then `work_ms` doing it (FS-M1a, #7893). Never its arguments
    /// (ADR 0075).
    AnswerSpan {
        command: String,
        class: String,
        queued_ms: u32,
        work_ms: u32,
        at: u64,
    },
    /// How long the core's answers to each command took from `since` to `at`, every one and
    /// not only the slow (FS-M1a, #7893): written once an hour and as the app quits.
    AnswerSummary {
        at: u64,
        since: u64,
        commands: Vec<Spread>,
    },
}

impl Slow {
    /// The kind of line it is written as.
    pub fn kind(&self) -> Kind {
        match self {
            Slow::Answer { .. } => Kind::Answer,
            Slow::Stall { .. } => Kind::Stall,
            Slow::CoreStall { .. } => Kind::CoreStall,
            Slow::CoreCommand { .. } => Kind::CoreCommand,
            Slow::Opening { .. } => Kind::Opening,
            Slow::Visibility { .. } => Kind::Visibility,
            Slow::AnswerSpan { .. } => Kind::AnswerSpan,
            Slow::AnswerSummary { .. } => Kind::AnswerSummary,
        }
    }

    /// When it happened, in milliseconds since 1970.
    pub fn at(&self) -> u64 {
        match self {
            Slow::Answer { at, .. }
            | Slow::Stall { at, .. }
            | Slow::CoreStall { at, .. }
            | Slow::CoreCommand { at, .. }
            | Slow::Opening { at, .. }
            | Slow::Visibility { at, .. }
            | Slow::AnswerSpan { at, .. }
            | Slow::AnswerSummary { at, .. } => *at,
        }
    }

    /// How long it took, what its kind's threshold is held against: none for a kind that has no
    /// time of its own.
    fn ms(&self) -> Option<u32> {
        match self {
            Slow::Answer { ms, .. }
            | Slow::Stall { ms, .. }
            | Slow::CoreStall { ms, .. }
            | Slow::CoreCommand { ms, .. }
            | Slow::Opening { ms, .. } => Some(*ms),
            Slow::AnswerSpan {
                queued_ms, work_ms, ..
            } => Some(queued_ms.saturating_add(*work_ms)),
            Slow::Visibility { .. } | Slow::AnswerSummary { .. } => None,
        }
    }

    /// Whether the log keeps it: it took its kind's threshold or longer.
    fn worth_keeping(&self) -> bool {
        self.ms()
            .is_none_or(|ms| ms >= self.kind().keeping().from_ms)
    }
}

/// A region of the window React rendered in a stalled frame, by its name (#7891): `ms`, the
/// milliseconds its renders took there, the regions inside it included; `self_ms`, those of
/// its own body, without them; and `commits`, how many of React's commits in the frame rendered
/// anything inside it, its own body or a region within it, each commit counted once. A region's
/// name, never what it showed. `self_ms` and `commits` are 0 in a line written before they were
/// kept.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rendered {
    pub name: String,
    pub ms: u32,
    #[serde(default)]
    pub self_ms: u32,
    #[serde(default)]
    pub commits: u32,
}

/// How long the core's `n` answers to `command`, of class `class`, took: the median `p50_ms`
/// and the 99th percentile `p99_ms`, in milliseconds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Spread {
    pub command: String,
    pub class: String,
    pub n: u32,
    pub p50_ms: u32,
    pub p99_ms: u32,
}

/// Something that took `ms` milliseconds, by its name: a phase of an opening or of an answer.
/// This code's own name, never a chat's content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Took {
    pub name: String,
    pub ms: u32,
}

/// Something the window handled or did `n` times in a stalled frame, by its name: an event's,
/// a command's or a mark's, never what it carried.
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

/// One writer at a time in this process, so a cut never loses a line appended while it was
/// being made.
static WRITING: Mutex<()> = Mutex::new(());

/// Appends `slow` to the log at `file`, making it and its folder when they are not there, when
/// it took its kind's threshold or longer, and keeps only the newest of each kind up to its cap
/// ([`Kind::keeping`]).
///
/// Appended while every kind is under its cap; once one would pass it, written again whole
/// ([`kept`]), through a file beside it that takes its place, so a reader never finds half a
/// log. At most two megabytes or so (the 200 answer summaries near one of them, each held to
/// [`answers::MOST_COMMANDS`] commands), and only on a slow moment.
pub fn record_in(file: &Path, slow: &Slow) -> io::Result<()> {
    if !slow.worth_keeping() {
        return Ok(());
    }
    let _one = WRITING.lock().unwrap_or_else(|held| held.into_inner());
    let line = serde_json::to_string(slow).map_err(io::Error::other)?;
    let dir = file.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir)?;
    let was = match std::fs::read_to_string(file) {
        Ok(was) => was,
        Err(err) if err.kind() == io::ErrorKind::NotFound => String::new(),
        Err(err) => return Err(err),
    };
    let mut each = HashMap::<Kind, usize>::new();
    for kind in was.lines().filter_map(kind_of).chain([slow.kind()]) {
        *each.entry(kind).or_default() += 1;
    }
    if each.iter().all(|(kind, n)| *n <= kind.keeping().cap) {
        let mut log = OpenOptions::new().create(true).append(true).open(file)?;
        // A log whose last line was cut short gets its own line back first.
        if !was.is_empty() && !was.ends_with('\n') {
            log.write_all(b"\n")?;
        }
        return log.write_all(format!("{line}\n").as_bytes());
    }
    let mut next = tempfile::NamedTempFile::new_in(dir)?;
    for one in kept(was.lines().chain([line.as_str()])) {
        next.write_all(one.as_bytes())?;
        next.write_all(b"\n")?;
    }
    next.persist(file).map_err(|err| err.error)?;
    Ok(())
}

/// The kind `line` is written as, or `None` for a line that is not an entry this build knows.
fn kind_of(line: &str) -> Option<Kind> {
    #[derive(Deserialize)]
    struct Kinded {
        kind: Kind,
    }
    serde_json::from_str::<Kinded>(line)
        .ok()
        .map(|one| one.kind)
}

/// **The lines of a log the cut keeps**, oldest first: the newest of each kind up to its cap. A
/// line this build cannot read as an entry, one cut short or of a kind a later build writes, is
/// let go, as [`recent_in`] passes it over.
fn kept<'a>(lines: impl IntoIterator<Item = &'a str>) -> Vec<&'a str> {
    let lines: Vec<&str> = lines.into_iter().collect();
    let mut each = HashMap::<Kind, usize>::new();
    let mut kept: Vec<&str> = lines
        .into_iter()
        .rev()
        .filter(|line| {
            kind_of(line).is_some_and(|kind| {
                let n = each.entry(kind).or_default();
                *n += 1;
                *n <= kind.keeping().cap
            })
        })
        .collect();
    kept.reverse();
    kept
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

/// Milliseconds since 1970 by this machine's clock; 0 before then.
fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| {
            u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
        })
}

pub mod answers;

#[cfg(test)]
mod tests;
