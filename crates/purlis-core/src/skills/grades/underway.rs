// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A grade under way, held on this machine while it runs** (#3353): a grade is recorded when it
//! ends, so a crash, a fault or a quit while it ran would lose it, and leave nothing to say it
//! ever ran. While it runs, its row is held in `<state>/grades-under-way/<grade>.json`, its report
//! saying it was interrupted ([`GradeEnd::Interrupted`]); its end takes the file away
//! ([`ended`]). At the project's next start, a row still held there that no running purlis holds
//! is recorded as it is, interrupted ([`interrupted`]), so a grade never stays running and is
//! never lost. The chat it ran then is the session host's to end with every other chat nothing
//! reattached (#2006).
//!
//! **Tier:** Clone state, never committed: what this machine was doing, read back by this machine
//! alone, each file at most [`MOST_FILE_BYTES`] and read through no link.

use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};

use purlis_skills::grade::{self, GradeEnd, GradeReport};

use super::{Graded, Level, MOST_FILES, Spent, is_local, plain, record_to};

/// The folder of this machine's state the grades under way are held in.
const DIR: &str = "grades-under-way";

/// The most bytes of one grade under way read back.
const MOST_FILE_BYTES: u64 = 64 * 1024;

/// The file beside that folder a start holds the lock of while it records what it finds there:
/// two purlis processes starting one project at once never both record one grade (#3443).
const CLAIM: &str = "grades-under-way.lock";

/// One grade under way, as its file holds it.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct UnderWay {
    /// The process that runs it: a file whose process still runs is that purlis's, never
    /// recorded by another.
    pid: u32,
    /// Whether its copy is this machine's alone, so its row goes to this machine's record.
    local: bool,
    /// Its row as it would be recorded had it ended now: no chat scored, interrupted.
    graded: Graded,
}

/// The folder grades under way are held in, in the project at `root`.
fn dir(root: &Path) -> PathBuf {
    crate::plane::state_dir(root).join(DIR)
}

/// **The report of a grade purlis stopped under before it was scored**: no chat scored, the caps
/// it was held to, interrupted.
pub fn interrupted_report(cap: u64, cost_cap_usd: Option<f64>) -> GradeReport {
    GradeReport {
        grader: grade::BUILT_IN.to_owned(),
        triggers: Vec::new(),
        arms: Vec::new(),
        delta: None,
        ended: GradeEnd::Interrupted,
        not_started: None,
        cap: Some(cap),
        spent_tokens: None,
        cost_cap_usd,
        spent_usd: None,
    }
}

/// **Holds `graded`, a grade of the copy at `level` starting in the project at `root`, as under
/// way**: its row, with its report set to [`interrupted_report`] and nothing spent, in a private
/// file of this machine's state, opened following no link. The grade's id names the file, so an
/// id that is not a plain name is refused.
pub fn began(root: &Path, level: &Level, graded: &Graded) -> std::io::Result<PathBuf> {
    if !plain(&graded.grade) {
        return Err(std::io::Error::other("the grade's id is not a plain name"));
    }
    let held = UnderWay {
        pid: std::process::id(),
        local: is_local(root, level),
        graded: Graded {
            report: interrupted_report(
                graded.report.cap.unwrap_or_default(),
                graded.report.cost_cap_usd,
            ),
            spent: Spent::default(),
            ..graded.clone()
        },
    };
    let dir = dir(root);
    std::fs::create_dir_all(&dir)?;
    if !std::fs::symlink_metadata(&dir)?.is_dir() {
        return Err(std::io::Error::other("the folder is not a folder"));
    }
    let path = dir.join(format!("{}.json", graded.grade));
    // Written whole beside it, then renamed over it: a start that reads the folder meanwhile
    // finds no file, or the whole one, never half of it.
    let partial = dir.join(format!(".{}.json.partial", graded.grade));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let written = (|| {
        let mut file = crate::contain::nofollow(&mut options).open(&partial)?;
        let text = serde_json::to_string(&held).map_err(std::io::Error::other)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        std::fs::rename(&partial, &path)
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&partial);
    }
    written.map(|()| path)
}

/// **The grade `grade` ended**, recorded or not: it is no longer held as under way.
pub fn ended(root: &Path, grade: &str) {
    if plain(grade) {
        let _ = std::fs::remove_file(dir(root).join(format!("{grade}.json")));
    }
}

/// **Records every grade under way in the project at `root` that no running purlis holds, as
/// interrupted**, and holds it under way no more: what a project's start does (#3353). A file
/// whose process still runs is left as it is, unless it is this process and `runs_here` says this
/// process runs no such grade; a file that is a link, too large, or not a sound row is taken away
/// unrecorded. Two calls at once, of two processes or of one, take turns on the folder's claim
/// ([`CLAIM`], #3443), so the second finds what the first recorded gone and records it never
/// again; one that cannot take the claim records nothing. `device` names this machine's
/// committed record file, and is asked only for a
/// grade recorded there: a start with no grade under way reads nothing of this machine's store
/// (a fenced test's start among them). A file of a newer purlis is left for it, and one whose row could not be written
/// is kept for the next start, so a grade is never lost. The answer is the grades recorded, by
/// id.
pub fn interrupted(
    root: &Path,
    device: impl Fn() -> String,
    runs_here: impl Fn(&str) -> bool,
) -> Vec<String> {
    let dir = dir(root);
    if !std::fs::symlink_metadata(&dir).is_ok_and(|meta| meta.is_dir()) {
        return Vec::new();
    }
    let Some(_claim) = claimed(root) else {
        return Vec::new();
    };
    let Ok(reader) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in reader.take(MOST_FILES).filter_map(Result::ok) {
        let path = entry.path();
        if path.extension().is_none_or(|ext| ext != "json") {
            continue;
        }
        let held = match read(&path) {
            Held::Sound(held) => *held,
            Held::Newer => continue,
            Held::Unsound => {
                let _ = std::fs::remove_file(&path);
                continue;
            }
        };
        let held_by_a_running_purlis = if held.pid == std::process::id() {
            runs_here(&held.graded.grade)
        } else {
            crate::process::alive(held.pid)
        };
        if held_by_a_running_purlis {
            continue;
        }
        if record_to(root, held.local, &held.graded, &device).is_none() {
            continue;
        }
        out.push(held.graded.grade.clone());
        let _ = std::fs::remove_file(&path);
    }
    out
}

/// **The claim on the grades under way in the project at `root`, held**: its [`CLAIM`] file's
/// lock, opened following no link, waited for while another call records (a few rows, so a
/// wait that ends). `None` where it cannot be opened or locked.
fn claimed(root: &Path) -> Option<crate::filelock::Held> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let file = crate::contain::nofollow(&mut options)
        .open(crate::plane::state_dir(root).join(CLAIM))
        .ok()?;
    match file.lock() {
        Ok(()) => Some(crate::filelock::Held::locked(file)),
        Err(why) => {
            tracing::warn!("purlis: the grades under way were not claimed ({why})");
            None
        }
    }
}

/// What one file of the folder holds.
enum Held {
    Sound(Box<UnderWay>),
    /// A row a newer purlis wrote: left for it.
    Newer,
    /// A link, too large, not a row, named by another id, or not a sound row.
    Unsound,
}

/// The grade under way the file `path` holds, read through no link and at most
/// [`MOST_FILE_BYTES`], where it is a sound row named by its own id.
fn read(path: &Path) -> Held {
    let text = (|| {
        let meta = std::fs::symlink_metadata(path).ok()?;
        if !meta.is_file() || meta.len() > MOST_FILE_BYTES {
            return None;
        }
        let mut options = std::fs::OpenOptions::new();
        options.read(true);
        let file = crate::contain::nofollow(&mut options).open(path).ok()?;
        let mut text = String::new();
        file.take(MOST_FILE_BYTES).read_to_string(&mut text).ok()?;
        Some(text)
    })();
    let Some(held) = text.and_then(|text| serde_json::from_str::<UnderWay>(&text).ok()) else {
        return Held::Unsound;
    };
    let named = path.file_stem().and_then(|stem| stem.to_str()) == Some(held.graded.grade.as_str());
    if !named {
        Held::Unsound
    } else if held.graded.v > super::VERSION {
        Held::Newer
    } else if held.graded.sound() {
        Held::Sound(Box::new(held))
    } else {
        Held::Unsound
    }
}

#[cfg(test)]
mod tests;
