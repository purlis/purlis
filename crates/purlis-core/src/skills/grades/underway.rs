// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A grade under way, held on this machine while it runs** (#3353): a grade is recorded when it
//! ends, so a crash, a fault or a quit while it ran would lose it, and leave nothing to say it
//! ever ran. While it runs, its row is held in this machine's app state of the project
//! (`crate::appstate`), `<data>/app-state/<clone key>/grades-under-way/<grade>.json`, its report
//! saying it was interrupted ([`GradeEnd::Interrupted`]), and, as each chat ends, what its chats
//! spent so far ([`Marker::spent`], #3582); its end takes the file away
//! ([`ended`]). At the project's next start, a row still held there that no running purlis holds
//! is recorded as it is, interrupted ([`interrupted`]), so a grade never stays running and is
//! never lost. The chat it ran then is the session host's to end with every other chat nothing
//! reattached (#2006).
//!
//! **Tier:** Clone state, never committed: what this machine was doing, read back by this machine
//! alone, each file at most [`MOST_FILE_BYTES`] and read through no link.
//!
//! **A grade holds its file's lock while it runs** (#3466): [`began`] answers a [`Marker`] whose
//! `flock` on the file lasts as long as the grade holds it, and goes with its process however
//! that ends. A start reads a file whose lock another holds as running, whatever process number
//! it names, so a crashed grade whose number another process took is recorded all the same, and
//! a running one is never recorded on a system where a process number says nothing. Where the
//! lock cannot be asked, the file's process number is read as before.

use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};

use purlis_skills::grade::{self, GradeEnd, GradeReport};

use super::{Graded, Level, MOST_FILES, Spent, is_local, plain, record_to};

/// **A grade's file under way, and its lock, held while the grade runs** (#3466): dropped once it
/// ends, after [`ended`] took the file away.
#[derive(Debug)]
pub struct Marker {
    path: PathBuf,
    /// What the file holds: what [`Marker::spent`] writes again, with the spend so far.
    held: UnderWay,
    /// `None` where the file could not be locked: a start then reads its process number.
    _lock: Option<crate::filelock::Held>,
}

impl Marker {
    /// The file's path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// **Says in the file what the grade's chats spent so far** (#3582): `spent`, its chats,
    /// tokens and cost as the chats said them, in the row and in its report, so a grade purlis
    /// stops under is recorded with what it spent, and the chats it asked for so far, ran or not
    /// (`Spent::asked_for`, #3602), so an estimate can share that spend over them. Written whole beside the file and renamed
    /// over it, as [`began`] writes it, and locked before it is in its place where the first
    /// was: the lock is the file's, so the new file takes it before the old one lets it go. A
    /// spend no sound row could hold is refused, and the file stays as it was, so the grade is
    /// never taken away unrecorded at the next start.
    pub fn spent(&mut self, spent: Spent) -> std::io::Result<()> {
        let mut held = self.held.clone();
        held.graded.spent = spent;
        held.graded.report.spent_tokens = spent.tokens;
        held.graded.report.spent_usd = spent.cost_usd;
        if !held.graded.sound(chrono::DateTime::<chrono::Utc>::MAX_UTC) {
            return Err(std::io::Error::other("no sound row says that spend"));
        }
        let dir = self
            .path
            .parent()
            .ok_or_else(|| std::io::Error::other("the file has no folder"))?;
        // The old file's lock goes once the new one, locked, is in its place.
        self._lock = put(dir, &self.path, &mut held, self.held.locked)?;
        self.held = held;
        Ok(())
    }

    /// The file's path, its lock let go of, as a crash lets go of it.
    #[cfg(test)]
    pub fn into_path(self) -> PathBuf {
        let Marker { path, .. } = self;
        path
    }
}

#[cfg(test)]
impl std::ops::Deref for Marker {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.path
    }
}

/// The folder of this machine's app state of the project the grades under way are held in.
pub const DIR: &str = "grades-under-way";

/// The most bytes of one grade under way read back.
const MOST_FILE_BYTES: u64 = 64 * 1024;

/// The file beside that folder a start holds the lock of while it records what it finds there:
/// two purlis processes starting one project at once never both record one grade (#3443).
pub const CLAIM: &str = "grades-under-way.lock";

/// One grade under way, as its file holds it.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct UnderWay {
    /// The process that runs it: a file whose process still runs is that purlis's, never
    /// recorded by another.
    pid: u32,
    /// Whether its copy is this machine's alone, so its row goes to this machine's record.
    local: bool,
    /// Whether the grade holds the file's lock while it runs ([`Marker`], #3466): where it does
    /// not, a lock no other holds says nothing, and its process number is read instead. `false`
    /// in a file written before this field.
    #[serde(default)]
    locked: bool,
    /// Its row as it would be recorded had it ended now: no chat scored, interrupted.
    graded: Graded,
}

/// The folder grades under way are held in, in this machine's app state of the project at `root`
/// (`crate::appstate`). `None` where this machine has no data home.
fn dir(root: &Path) -> Option<PathBuf> {
    Some(crate::appstate::dir(root)?.join(DIR))
}

/// **The report of a grade purlis stopped under before it was scored**: no chat scored, the caps
/// it was held to and the price its dollar limit was worked out at, interrupted.
pub fn interrupted_report(
    cap: u64,
    cost_cap_usd: Option<f64>,
    priced: Option<grade::Priced>,
) -> GradeReport {
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
        counted_usd: None,
        priced,
    }
}

/// **Holds `graded`, a grade of the copy at `level` starting in the project at `root`, as under
/// way**: its row, with its report set to [`interrupted_report`] and nothing spent, in a private
/// file of this machine's state, opened following no link, and locked before it is in its place
/// ([`Marker`]). The grade's id names the file, so an id that is not a plain name is refused.
pub fn began(root: &Path, level: &Level, graded: &Graded) -> std::io::Result<Marker> {
    if !plain(&graded.grade) {
        return Err(std::io::Error::other("the grade's id is not a plain name"));
    }
    let mut held = UnderWay {
        pid: std::process::id(),
        local: is_local(root, level),
        locked: false,
        graded: Graded {
            report: interrupted_report(
                graded.report.cap.unwrap_or_default(),
                graded.report.cost_cap_usd,
                graded.report.priced.clone(),
            ),
            spent: Spent::default(),
            ..graded.clone()
        },
    };
    let dir = crate::appstate::made(root)
        .map_err(std::io::Error::other)?
        .join(DIR);
    std::fs::create_dir_all(&dir)?;
    if !std::fs::symlink_metadata(&dir)?.is_dir() {
        return Err(std::io::Error::other("the folder is not a folder"));
    }
    let path = dir.join(format!("{}.json", graded.grade));
    let lock = put(&dir, &path, &mut held, false)?;
    Ok(Marker {
        path,
        held,
        _lock: lock,
    })
}

/// **Puts `held` in its place `path`, in the folder `dir`**: written whole beside it, then renamed
/// over it, so a start that reads the folder meanwhile finds no file, or the whole one, never half
/// of it. The new file is locked before it is written and renamed, so no start ever finds it in
/// its place unlocked, and `held` says whether it is: the lock is the file's, and the rename
/// keeps the file. Where `must_lock`, a file that cannot be locked is never put in its place, as
/// the one it would replace says it is locked. The answer is the new file's lock, where it took
/// one.
fn put(
    dir: &Path,
    path: &Path,
    held: &mut UnderWay,
    must_lock: bool,
) -> std::io::Result<Option<crate::filelock::Held>> {
    let partial = dir.join(format!(".{}.json.partial", held.graded.grade));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let written = (|| {
        let mut file = crate::contain::nofollow(&mut options).open(&partial)?;
        held.locked = match file.try_lock() {
            Ok(()) => true,
            Err(why) if must_lock => {
                return Err(std::io::Error::other(format!(
                    "the file could not be locked again ({why})"
                )));
            }
            Err(why) => {
                tracing::warn!(
                    "purlis: grade {} under way is not locked ({why}): its process number says \
                     whether it runs",
                    held.graded.grade
                );
                false
            }
        };
        let text = serde_json::to_string(&held).map_err(std::io::Error::other)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        std::fs::rename(&partial, path)?;
        Ok(held.locked.then(|| crate::filelock::Held::locked(file)))
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&partial);
    }
    written
}

/// **The grade `grade` ended**, recorded or not: it is no longer held as under way.
pub fn ended(root: &Path, grade: &str) {
    if plain(grade)
        && let Some(dir) = dir(root)
    {
        let _ = std::fs::remove_file(dir.join(format!("{grade}.json")));
    }
}

/// **Records every grade under way in the project at `root` that no running purlis holds, as
/// interrupted**, and holds it under way no more: what a project's start does (#3353). A file
/// whose lock another holds is left as it is ([`Marker`], #3466); where its lock cannot be asked,
/// one whose process still runs is, unless it is this process and `runs_here` says this process
/// runs no such grade; a file that is a link, too large, or not a sound row is taken away
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
    let Some(dir) = dir(root) else {
        return Vec::new();
    };
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
        // A write a crash cut short ([`put`]): taken away once no other holds its lock, as its
        // writer holds it while it writes.
        if is_partial(&path) {
            if locked_by_another(&path) == Some(false) {
                let _ = std::fs::remove_file(&path);
            }
            continue;
        }
        if path.extension().is_none_or(|ext| ext != "json") {
            continue;
        }
        // The lock first: a file another holds is never read, as its lock may keep it from
        // being read on some systems, and an unread file would be taken away.
        let locked = locked_by_another(&path);
        if locked == Some(true) {
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
        // A lock no other holds says the grade ended only where the grade took it.
        let held_by_a_running_purlis = match locked.filter(|_| held.locked) {
            Some(locked) => locked,
            None if held.pid == std::process::id() => runs_here(&held.graded.grade),
            None => crate::process::alive(held.pid),
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

/// **The grades of the copy of `name` at `level` under way in the project at `root`**, by id: what
/// a rename or a move of a skill only on this machine keeps, as each is recorded by the name and
/// level it began under once it ends (#4104). Read through no link, at most [`MOST_FILES`].
pub fn of(root: &Path, level: &Level, name: &str) -> Vec<String> {
    let Some(dir) = dir(root) else {
        return Vec::new();
    };
    if !std::fs::symlink_metadata(&dir).is_ok_and(|meta| meta.is_dir()) {
        return Vec::new();
    }
    let Ok(reader) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let rel = level.rel();
    // A row that says no level is a local workspace's copy's (`super::read`): another level's
    // copy of the name is another skill.
    let levelless = is_local(root, level) && level.at.workspace().is_some();
    let mut out: Vec<String> = reader
        .take(MOST_FILES)
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| !is_partial(path) && path.extension().is_some_and(|ext| ext == "json"))
        .filter_map(|path| match read(&path) {
            Held::Sound(held) => Some(held.graded),
            Held::Newer | Held::Unsound => None,
        })
        .filter(|row| row.skill == name && (row.level == rel || (levelless && row.level.is_none())))
        .map(|row| row.grade)
        .collect();
    out.sort();
    out
}

/// Whether `path` is a file [`put`] writes before it is renamed into its place.
fn is_partial(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with('.') && name.ends_with(".json.partial"))
}

/// **Whether another holds the lock of the grade's file at `path`** ([`Marker`]), or `None` where
/// that cannot be asked (the file will not open, or its system has no such lock). A lock this
/// call takes is let go of at once.
fn locked_by_another(path: &Path) -> Option<bool> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    let file = crate::contain::nofollow(&mut options).open(path).ok()?;
    match file.try_lock() {
        Ok(()) => {
            let _ = file.unlock();
            Some(false)
        }
        Err(std::fs::TryLockError::WouldBlock) => Some(true),
        Err(std::fs::TryLockError::Error(_)) => None,
    }
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
        .open(crate::appstate::dir(root)?.join(CLAIM))
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
    } else if
    // This machine's own row, stamped by its own clock as the grade began: a clock set back
    // since never makes it unsound, so no stamp is too late here (#3273).
    held.graded.sound(chrono::DateTime::<chrono::Utc>::MAX_UTC) {
        Held::Sound(Box::new(held))
    } else {
        Held::Unsound
    }
}

#[cfg(test)]
mod tests;
