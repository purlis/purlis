// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The host's event log (FD-9, #649): one structured event per hook call, under the chat and
//! the run it happened in.
//!
//! **One log per device, one writer** (ADR 0066, ADR 0068): the app today, `charterd` once
//! FD-5 moves the board there. A `charter` command never writes it. It reaches the writer over
//! the hook channel, which carries a chat's number and token and nothing more, and the host
//! maps the number to the chat's id and current run itself ([`Recorder`]).
//!
//! **Where:** `<data>/events/<device>/`, as `events.jsonl` and the sealed segments before it
//! ([`Retention`]), with the writer's lock in `events.lock` ([`crate::datahome`]). Machine,
//! device-bound, backed up by FR-10 (ADR 0069 row 63), never committed and never sent.
//! `docs/plane-format.md` records it.
//!
//! **What a line is:** ADR 0066's envelope ([`Event`]). The `ulid`'s time part is the event's
//! time, so there is no timestamp field. `seq` counts from 1 per device and is never reused,
//! including across launches.
//!
//! **What a tool event holds:** the tool, the harness's id for the call, a digest of its
//! arguments keyed by this device ([`ArgsKey`], O3: "an args digest, never raw args"), what
//! charter answered, the rule that refused, and how long the hook and the tool took. The
//! arguments never reach this process: a hook sends only their SHA-256.
//!
//! **What it feeds:** the audit is written from it, one entry per event (ADR 0075 §1), and
//! telemetry derives its OTel logs from it (ADR 0083). It is neither of them (O1). Each is a
//! [`Consumer`] the host opens the log with, and what one has not taken the log keeps past
//! its retention, up to a hard limit (#940).
//!
//! **How it is read** (FD-24): a client [`subscribe`]s from its cursor, the last `seq` it
//! holds, and gets every later event in order, each once, across sealed segments and across a
//! writer that was killed and started again. A cursor older than what [`Retention`] keeps is
//! told so ([`Delivery::Missed`]), never given a silent gap; and so is a cursor ahead of the
//! log ([`Delivery::Ahead`]), after a power loss took lines the client had read.

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::state::run::{Cause, RunState};

pub mod browsing;
mod entry;
pub use entry::Entry;

/// One event, in ADR 0066's envelope.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Event {
    pub v: u32,
    pub device_id: String,
    pub seq: u64,
    pub ulid: String,
    pub chat: Option<String>,
    pub run: Option<String>,
    pub parent_run: Option<String>,
    pub kind: String,
    pub body: serde_json::Value,
}

/// The version of the envelope, and of every kind's body this module writes.
pub const VERSION: u32 = 1;

/// The directory under `<data>` that holds each device's log.
pub const EVENTS: &str = "events";

/// The file in a device's directory the events are appended to.
pub const FILE: &str = "events.jsonl";

/// How far into a segment, from its end or its start, charter reads to find its last or its
/// first event: far more than one line.
const TAIL: u64 = 64 * 1024;

/// The file in a device's directory whose lock is the device's one writer. A file of its own,
/// so the lock outlives each segment the writer seals.
pub const LOCK: &str = "events.lock";

/// How long the log is kept, and how big a segment grows before it is sealed.
///
/// **What is kept:** every event of the last [`Retention::keep`], and more. The log is
/// `events.jsonl`, the segment being written, and the sealed segments before it,
/// `events.<first seq>.jsonl`, each sealed once it reaches [`Retention::segment_bytes`]. A
/// sealed segment is deleted, when the log is opened or a segment sealed, once its newest
/// event is older than `keep`; the segment being written and the newest sealed one never are.
/// So a light user keeps more than `keep`, and nobody keeps less.
///
/// **What a consumer has not taken is kept longer** (#940): a sealed segment older than `keep`
/// stays until every [`Consumer`] the log was opened with has taken its last event, and no
/// longer than [`Retention::hold`], the hard limit. Past `hold` it is deleted whoever has not
/// taken it, the app's log says which consumer lost which events ([`Unread`]), and that
/// consumer is told what it missed when it reads on ([`Delivery::Missed`]): a consumer that
/// never comes back does not fill the disk.
///
/// **Why 30 days:** the audit (ADR 0075) and telemetry (ADR 0083) are written from this log,
/// and a writer of either that was down must find what it has not taken still here: "days,
/// not minutes" (ADR 0075). Telemetry's own default is 30 days, so the log reaches as far back
/// as the longest store derived from it by default. About 30 MB a busy day (ADR 0075 §7's
/// estimate) is about 1 GB at the most.
///
/// **Why 90 days at the most:** three times `keep`, so a consumer that is down has a season
/// to come back in, and the log of a machine whose consumer never does stops at about 3 GB by
/// the same estimate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Retention {
    pub segment_bytes: u64,
    pub keep: std::time::Duration,
    /// The longest a sealed segment is kept for a consumer that has not taken it.
    pub hold: std::time::Duration,
}

impl Retention {
    pub const DEFAULT: Retention = Retention {
        segment_bytes: 16 * 1024 * 1024,
        keep: std::time::Duration::from_secs(30 * 24 * 60 * 60),
        hold: std::time::Duration::from_secs(90 * 24 * 60 * 60),
    };
}

/// A store written from this log, which the log's retention waits for (#940): the audit's
/// writer (ADR 0075) and telemetry (ADR 0083), each reading through a cursor of its own.
///
/// **Its place is read from its own store, not kept beside the log.** `taken` answers the
/// last `seq` the consumer has taken (0 for none), as that store records it: the audit's last
/// entry names the event it was written from, and telemetry's last record does. So there is
/// one account of what a consumer holds, the one a crash leaves consistent with what it
/// wrote, and no second copy of a cursor that could say otherwise. The host names its
/// consumers when it opens the log ([`Log::open_held`]), every time: one that is switched off
/// or no longer built is not named, and holds nothing.
///
/// **A consumer that cannot say holds everything.** While `taken` answers an error, nothing
/// it might not have taken is deleted before [`Retention::hold`]: nothing is deleted on a
/// guess.
///
/// A client that subscribes over a link is not one of these. It is told what it missed
/// ([`Delivery::Missed`]) and rebuilds; only the stores that must lose nothing hold the log.
pub struct Consumer {
    name: &'static str,
    taken: Box<dyn Fn() -> io::Result<u64> + Send>,
}

impl Consumer {
    /// A consumer called `name` in the app's log, whose `taken` answers the last `seq` it
    /// took.
    pub fn new(
        name: &'static str,
        taken: impl Fn() -> io::Result<u64> + Send + 'static,
    ) -> Consumer {
        Consumer {
            name,
            taken: Box::new(taken),
        }
    }
}

/// Events [`Retention::hold`] deleted before `consumer` took them: those after `after`
/// through `through`, one segment's worth. The log's writer says each in the app's log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unread {
    pub consumer: &'static str,
    pub after: u64,
    pub through: u64,
}

/// What [`Log::on_seal`] is given.
type SealHook = Box<dyn FnMut(&File) + Send>;

/// What makes a log's events durable: one `fsync` for every event appended before it, so
/// hooks recorded at once share one (group commit).
///
/// The operating system's ordinary `fsync`. On macOS that hands the data to the drive, not
/// through the drive's own cache (`F_FULLFSYNC`, measured at about 10 ms against 0.1 ms on
/// the operator's machine), which is what SQLite does by default there.
///
/// **It follows the segment being written** (FD-24): each seal swaps in a handle on the new
/// `events.jsonl` before any event is written to it. Every event of the sealed segment was
/// `fsync`ed by the seal itself, so a number at or below the last one appended before the swap
/// is durable whichever handle is synced. A seal that could not open the next segment leaves
/// no handle, and `through` answers an error until one is open, so nothing is told durable that
/// is not.
pub struct Durable {
    appended: std::sync::atomic::AtomicU64,
    /// The last number known durable, and the segment being written.
    state: std::sync::Mutex<(u64, Option<File>)>,
}

impl Durable {
    /// Returns once every event up to `seq` is durable.
    pub fn through(&self, seq: u64) -> io::Result<()> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.0 >= seq {
            return Ok(());
        }
        let appended = self.appended.load(std::sync::atomic::Ordering::SeqCst);
        let Some(file) = state.1.as_ref() else {
            return Err(io::Error::other(
                "the event log has no segment open to make durable",
            ));
        };
        #[cfg(unix)]
        rustix::fs::fsync(file)?;
        #[cfg(not(unix))]
        file.sync_data()?;
        state.0 = appended.max(seq);
        Ok(())
    }

    /// The last number known to be durable.
    pub fn synced(&self) -> u64 {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .0
    }

    /// The inode of the segment it syncs, for the test that it follows a seal.
    #[cfg(all(test, unix))]
    fn inode(&self) -> Option<u64> {
        use std::os::unix::fs::MetadataExt;
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .1
            .as_ref()
            .and_then(|file| file.metadata().ok())
            .map(|meta| meta.ino())
    }

    /// Notes that every event up to `seq` is durable: a seal `fsync`ed them.
    fn durable_through(&self, seq: u64) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.0 = state.0.max(seq);
    }

    /// Moves onto `file`, the segment a seal just opened.
    fn follow(&self, file: Option<File>) {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .1 = file;
    }
}

/// The log of one device, open for appending. Holding it is being the device's one writer.
pub struct Log {
    dir: PathBuf,
    /// The segment being written, `events.jsonl`. `None` once a seal renamed it and the next
    /// one could not be opened yet: nothing is written until it is, so no line ever goes into
    /// a segment already sealed.
    file: Option<File>,
    /// What makes its events durable, which follows each seal.
    durable: std::sync::Arc<Durable>,
    device: String,
    retention: Retention,
    /// Whom the retention waits for ([`Consumer`]).
    consumers: Vec<Consumer>,
    next: u64,
    /// The time of the newest event the log held as it was opened, in milliseconds since 1970
    /// by its id: every id the log held then was minted no later (#3394). 0 for an empty log,
    /// and `u64::MAX` for one whose newest event's id is not one, so nothing is taken as newer.
    held_until_ms: u64,
    /// How long the file is up to the end of its last whole line.
    good: u64,
    /// Whether a write failed partway, so the file may end in a torn line.
    torn: bool,
    /// How long the segment grows before a seal is tried: [`Retention::segment_bytes`], and
    /// further after a seal whose rename failed, so a failing rename is not retried at every
    /// append.
    seal_at: u64,
    /// Told the segment being written after each seal ([`Log::on_seal`]).
    on_seal: Option<SealHook>,
    /// A test's way to make the next write fail after this many bytes.
    #[cfg(test)]
    fail_after: Option<usize>,
    /// A test's way to make the next opens of a new segment fail.
    #[cfg(test)]
    fail_opens: u32,
    /// Held locked for as long as the `Log` lives, and unlocked when it is dropped, so a
    /// program forked meanwhile does not keep the next writer out (#1316). Declared last, so
    /// it is dropped last: the log's own files close before another writer can start.
    _lock: crate::filelock::Held,
}

impl Log {
    /// [`Log::open_with`] the [`Retention::DEFAULT`].
    pub fn open(dir: &Path, device: &str) -> io::Result<Log> {
        Log::open_with(dir, device, Retention::DEFAULT)
    }

    /// Opens the log in `dir`, the device's own directory under `<data>/events/`, making it if
    /// it is not there, and deletes the sealed segments `retention` no longer keeps.
    ///
    /// **One writer per device** (ADR 0066, ADR 0068): [`LOCK`] is locked for as long as the
    /// `Log` lives, and a second `open` while it is held is refused rather than interleaved.
    /// A lock held for only a moment is waited for, up to [`LOCK_WAIT`] (#972).
    /// The numbering carries on from the last event already in the log, so a `seq` is never
    /// used twice.
    pub fn open_with(dir: &Path, device: &str, retention: Retention) -> io::Result<Log> {
        Log::open_held(dir, device, retention, Vec::new())
    }

    /// [`Log::open_with`], with the `consumers` its retention waits for: a sealed segment one
    /// of them has not taken is kept past [`Retention::keep`], up to [`Retention::hold`]
    /// ([`Consumer`]). They are asked each time the log prunes, here and at every seal.
    pub fn open_held(
        dir: &Path,
        device: &str,
        retention: Retention,
        consumers: Vec<Consumer>,
    ) -> io::Result<Log> {
        crate::secrets::make_private_dir(dir)?;
        let lock = private_file(&dir.join(LOCK))?;
        lock_as_the_writer(&lock)?;
        let mut file = private_file(&dir.join(FILE))?;
        let good = cut_a_torn_line(&mut file)?;
        let last = match last_event(&mut file)? {
            Some(last) => Some(last),
            None if good == 0 => match segments(dir)?.last() {
                Some((_, sealed)) => {
                    Some(last_event(&mut File::open(sealed)?)?.ok_or_else(|| {
                        unreadable("its newest sealed segment holds no event purlis can read")
                    })?)
                }
                None => None,
            },
            // Lines, and not one of them an event charter wrote: counting from 1 again would
            // reuse every number already handed out, which is the one thing a seq may not do.
            None => return Err(unreadable("the event log holds no event purlis can read")),
        };
        let next = match &last {
            Some(last) => after(last.seq)?,
            None => 1,
        };
        let held_until_ms = match &last {
            None => 0,
            Some(last) => {
                ulid::Ulid::from_string(&last.ulid).map_or(u64::MAX, |ulid| ulid.timestamp_ms())
            }
        };
        let durable = std::sync::Arc::new(Durable {
            appended: std::sync::atomic::AtomicU64::new(next - 1),
            state: std::sync::Mutex::new((next - 1, Some(file.try_clone()?))),
        });
        let mut log = Log {
            dir: dir.to_owned(),
            file: Some(file),
            durable,
            device: device.to_owned(),
            retention,
            consumers,
            next,
            held_until_ms,
            good,
            torn: false,
            seal_at: retention.segment_bytes,
            on_seal: None,
            #[cfg(test)]
            fail_after: None,
            #[cfg(test)]
            fail_opens: 0,
            _lock: crate::filelock::Held::locked(lock),
        };
        if log.good >= log.seal_at {
            log.seal_or_warn();
        }
        log.prune();
        Ok(log)
    }

    /// Appends one event and answers it as written. When this returns `Ok` the whole line has
    /// been handed to the operating system, which a reader sees at once; it is durable once
    /// [`Durable::through`] its number has returned, which the host waits for before it tells
    /// a hook its line is taken (ADR 0075 §7, FD-30).
    pub fn append(
        &mut self,
        chat: Option<&str>,
        run: Option<&str>,
        parent_run: Option<&str>,
        kind: &str,
        body: serde_json::Value,
    ) -> io::Result<Event> {
        // Before anything else, so an append with no segment open uses no seq.
        self.live()?;
        if self.torn {
            // A write that failed partway may have left half a line, and the next line would
            // be glued to it: cut back to the last whole line first.
            let good = self.good;
            self.live()?.set_len(good)?;
            self.torn = false;
        }
        let seq = self.next;
        let event = Event {
            v: VERSION,
            device_id: self.device.clone(),
            seq,
            ulid: ulid::Ulid::generate().to_string(),
            chat: chat.map(str::to_owned),
            run: run.map(str::to_owned),
            parent_run: parent_run.map(str::to_owned),
            kind: kind.to_owned(),
            body,
        };
        let mut line = serde_json::to_vec(&event).map_err(io::Error::other)?;
        line.push(b'\n');
        // One `write_all` of one whole line on an append-mode file.
        #[cfg(test)]
        if let Some(after) = self.fail_after.take() {
            let _ = self.live()?.write_all(&line[..after.min(line.len())]);
            self.torn = true;
            return Err(io::Error::other("a write the test made fail"));
        }
        if let Err(why) = self.live()?.write_all(&line) {
            self.torn = true;
            return Err(why);
        }
        self.good += line.len() as u64;
        self.next = after(seq)?;
        self.durable
            .appended
            .store(seq, std::sync::atomic::Ordering::SeqCst);
        if self.good >= self.seal_at {
            // The event is written whatever sealing makes of it.
            self.seal_or_warn();
        }
        Ok(event)
    }

    /// Tells `hook` the segment being written each time a seal starts a new one, so whatever
    /// holds its own handle on the log's file (an `fsync`er, FD-30) moves to the new segment.
    /// Every event of the sealed segment is `fsync`ed before the seal, so nothing written
    /// before the hook runs is left behind on the old handle.
    pub fn on_seal(&mut self, hook: impl FnMut(&File) + Send + 'static) {
        self.on_seal = Some(Box::new(hook));
    }

    /// The segment being written, opened again if a seal left none open.
    fn live(&mut self) -> io::Result<&mut File> {
        if self.file.is_none() {
            let file = self.open_next()?;
            self.file = Some(file);
        }
        self.file
            .as_mut()
            .ok_or_else(|| io::Error::other("no segment of the event log is open"))
    }

    /// Opens a new, empty `events.jsonl`, `fsync`s the directory so its name is durable, and
    /// tells [`Log::on_seal`]'s hook.
    fn open_next(&mut self) -> io::Result<File> {
        #[cfg(test)]
        if self.fail_opens > 0 {
            self.fail_opens -= 1;
            return Err(io::Error::other("an open the test made fail"));
        }
        let file = private_file(&self.dir.join(FILE))?;
        sync_dir(&self.dir)?;
        self.good = 0;
        self.torn = false;
        // Before any event is written to it, so `Durable::through` never syncs a sealed one.
        self.durable.follow(file.try_clone().ok());
        if let Some(hook) = self.on_seal.as_mut() {
            hook(&file);
        }
        Ok(file)
    }

    /// [`Log::seal`], with a failure logged: the event before it is written either way. A
    /// rename that failed is tried again only after another quarter of a segment.
    fn seal_or_warn(&mut self) {
        match self.seal() {
            Ok(()) => self.seal_at = self.retention.segment_bytes,
            Err(why) => {
                if self.file.is_some() {
                    self.seal_at = self
                        .good
                        .saturating_add((self.retention.segment_bytes / 4).max(1));
                }
                tracing::warn!(error = %why, "the event log could not seal its segment");
            }
        }
    }

    /// Seals the segment being written as `events.<its first seq>.jsonl`, starts the next one,
    /// and deletes the sealed segments the retention no longer keeps.
    ///
    /// **The order is a subscriber's guarantee:** the sealed name appears only after its last
    /// line is written, so a reader that sees it can read the file to its end and be sure it
    /// has every line ([`Subscription::poll`]).
    ///
    /// **And a seal is durable:** the segment is `fsync`ed before its rename, and the
    /// directory after the rename and after the next segment is made. Once the rename is done
    /// the old file is never written again: if the next segment cannot be opened, appends
    /// fail until it can ([`Log::live`]).
    fn seal(&mut self) -> io::Result<()> {
        let Some(file) = self.file.as_mut() else {
            return Ok(());
        };
        let Some(first) = first_seq(file)? else {
            return Ok(());
        };
        file.sync_data()?;
        // Every event so far is in this segment and durable now.
        self.durable.durable_through(self.next.saturating_sub(1));
        let sealed = self.dir.join(sealed_name(first));
        std::fs::rename(self.dir.join(FILE), &sealed)
            .map_err(crate::rewrite::refused_at(&sealed))?;
        self.file = None;
        sync_dir(&self.dir)?;
        let next = self.open_next()?;
        self.file = Some(next);
        self.prune();
        Ok(())
    }

    /// [`prune`], with what it could not do and what its hard limit took logged.
    fn prune(&self) {
        let now = std::time::SystemTime::now();
        match prune(&self.dir, &self.retention, &self.consumers, now) {
            Ok(unread) => {
                for Unread {
                    consumer,
                    after,
                    through,
                } in unread
                {
                    tracing::warn!(
                        consumer,
                        after,
                        through,
                        held_at_most_secs = self.retention.hold.as_secs(),
                        "the event log deleted events a consumer had not taken: they were \
                         older than the longest it holds any"
                    );
                }
            }
            // A segment that could not be deleted is kept a little longer, never a refusal to
            // log.
            Err(why) => {
                tracing::warn!(error = %why, "the event log could not delete an old segment");
            }
        }
    }
}

impl Log {
    /// What makes this log's events durable, shared so the `fsync` can happen outside whatever
    /// lock holds the log.
    pub fn durable(&self) -> std::sync::Arc<Durable> {
        std::sync::Arc::clone(&self.durable)
    }
}

#[cfg(test)]
impl Log {
    /// Makes the next append write `bytes` of its line and fail, as a full disk would.
    pub fn fail_the_next_write_after(&mut self, bytes: usize) {
        self.fail_after = Some(bytes);
    }

    /// Makes the next `times` opens of a new segment fail, as a full directory would.
    pub fn fail_the_next_opens(&mut self, times: u32) {
        self.fail_opens = times;
    }
}

/// `fsync`s the directory `dir`, so a name made or renamed in it survives a power loss. A
/// directory cannot be opened as a file on Windows, where a rename is durable on its own.
fn sync_dir(dir: &Path) -> io::Result<()> {
    #[cfg(unix)]
    File::open(dir)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = dir;
    Ok(())
}

/// How long [`Log::open_with`] waits for a lock someone else holds before refusing (#972).
///
/// A lock is held by an open file, and every process forked while the log is open shares the
/// log's open files until it execs a program. A `Log` that is dropped unlocks first, so that
/// copy does not keep its lock (#1316); a host that just exited never unlocked, and its lock
/// outlives it while one of its children has not exec'd yet. That is not a second writer, and
/// it is over in far less than this.
pub const LOCK_WAIT: std::time::Duration = std::time::Duration::from_secs(1);

/// Locks `lock`, the device's [`LOCK`], waiting up to [`LOCK_WAIT`] for whoever holds it.
fn lock_as_the_writer(lock: &File) -> io::Result<()> {
    let deadline = Instant::now() + LOCK_WAIT;
    loop {
        match lock.try_lock() {
            Ok(()) => return Ok(()),
            Err(std::fs::TryLockError::WouldBlock) if Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Err(std::fs::TryLockError::WouldBlock) => {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "another host is already writing this device's event log",
                ));
            }
            Err(std::fs::TryLockError::Error(why)) => return Err(why),
        }
    }
}

fn unreadable(what: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("{what}, so it is not written to (starting again from 1 would reuse its numbers)"),
    )
}

/// `path`, opened to append and read, made private to this user if it is new.
fn private_file(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.create(true).append(true).read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path).map_err(crate::rewrite::refused_at(path))
}

/// The name of the sealed segment whose first event is `first`: zero-padded, so the names
/// sort as the numbers do.
fn sealed_name(first: u64) -> String {
    format!("events.{first:020}.jsonl")
}

/// The sealed segments in `dir`, by their first seq, oldest first.
fn segments(dir: &Path) -> io::Result<Vec<(u64, PathBuf)>> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(why) if why.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(why) => return Err(why),
    };
    let mut found = Vec::new();
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name();
        let Some(first) = name
            .to_str()
            .and_then(|name| name.strip_prefix("events."))
            .and_then(|rest| rest.strip_suffix(".jsonl"))
            .filter(|digits| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
            .and_then(|digits| digits.parse::<u64>().ok())
        else {
            continue;
        };
        found.push((first, entry.path()));
    }
    found.sort();
    Ok(found)
}

/// Deletes each sealed segment in `dir` whose newest event is older than `retention.keep` at
/// `now` and that every one of `consumers` has taken to its end, but never the newest sealed
/// segment: while the segment being written is empty, it alone holds the last `seq`, and
/// without it the next writer would count from 1 again. One whose newest event cannot be read
/// or dated is kept: nothing is deleted on a guess.
///
/// A segment some consumer has not taken is kept until its newest event is older than
/// `retention.hold`, and then deleted too. Answers what that took from whom.
fn prune(
    dir: &Path,
    retention: &Retention,
    consumers: &[Consumer],
    now: std::time::SystemTime,
) -> io::Result<Vec<Unread>> {
    // In milliseconds since the epoch, as a ULID dates an event. A span that reaches back
    // past the clock's start dates nothing as older than it.
    let before = |span: std::time::Duration| {
        now.checked_sub(span).map(|then| {
            then.duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |since| {
                    u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
                })
        })
    };
    let Some(oldest) = before(retention.keep) else {
        return Ok(Vec::new());
    };
    let held_since = before(retention.hold);
    let mut sealed = segments(dir)?;
    sealed.pop();
    if sealed.is_empty() {
        return Ok(Vec::new());
    }
    // Each consumer's place, asked once. One that cannot say has taken nothing.
    let places: Vec<(&'static str, u64)> = consumers
        .iter()
        .map(|consumer| match (consumer.taken)() {
            Ok(taken) => (consumer.name, taken),
            Err(why) => {
                tracing::warn!(
                    consumer = consumer.name,
                    error = %why,
                    "a consumer of the event log could not say what it has taken, so the log \
                     keeps everything for it"
                );
                (consumer.name, 0)
            }
        })
        .collect();
    let mut unread = Vec::new();
    for (first, path) in sealed {
        let Some(newest) = last_event(&mut File::open(&path)?)? else {
            continue;
        };
        let Ok(written) = ulid::Ulid::from_string(&newest.ulid).map(|id| id.timestamp_ms()) else {
            continue;
        };
        if written >= oldest {
            continue;
        }
        let behind = places.iter().filter(|(_, taken)| *taken < newest.seq);
        if behind.clone().next().is_some() {
            if held_since.is_none_or(|held_since| written >= held_since) {
                continue;
            }
            unread.extend(behind.map(|(consumer, taken)| Unread {
                consumer,
                after: (*taken).max(first.saturating_sub(1)),
                through: newest.seq,
            }));
        }
        std::fs::remove_file(&path)?;
    }
    Ok(unread)
}

/// The number after `seq`, or an error at the end of the range rather than a wrap to 0.
fn after(seq: u64) -> io::Result<u64> {
    seq.checked_add(1)
        .ok_or_else(|| io::Error::other("the event log has used every seq there is"))
}

/// Cuts `file` back to the end of its last whole line, where a crash left a part of one, and
/// answers how long it is then.
fn cut_a_torn_line(file: &mut File) -> io::Result<u64> {
    let len = file.metadata()?.len();
    let mut end = len;
    while end > 0 {
        let from = end.saturating_sub(TAIL);
        file.seek(SeekFrom::Start(from))?;
        let mut chunk = vec![0; usize::try_from(end - from).map_err(io::Error::other)?];
        file.read_exact(&mut chunk)?;
        if let Some(at) = chunk.iter().rposition(|byte| *byte == b'\n') {
            let whole = from + at as u64 + 1;
            if whole < len {
                file.set_len(whole)?;
            }
            return Ok(whole);
        }
        end = from;
    }
    if len > 0 {
        file.set_len(0)?;
    }
    Ok(0)
}

/// The last whole event in `file`, read from its end.
fn last_event(file: &mut File) -> io::Result<Option<Event>> {
    let len = file.metadata()?.len();
    let from = len.saturating_sub(TAIL);
    file.seek(SeekFrom::Start(from))?;
    let mut tail = Vec::new();
    file.read_to_end(&mut tail)?;
    Ok(tail
        .split(|byte| *byte == b'\n')
        .rev()
        .find_map(|line| serde_json::from_slice::<Event>(line).ok()))
}

/// The `seq` of the last whole event in `file`, read from its end.
fn last_seq(file: &mut File) -> io::Result<Option<u64>> {
    Ok(last_event(file)?.map(|event| event.seq))
}

/// The `seq` of the first event in `file`: its first line's, read alone, or the first event
/// in its first [`TAIL`] bytes where that line is not one.
fn first_seq(file: &mut File) -> io::Result<Option<u64>> {
    use std::io::BufRead;
    file.seek(SeekFrom::Start(0))?;
    let mut line = Vec::new();
    io::BufReader::new((&*file).take(TAIL)).read_until(b'\n', &mut line)?;
    if let Ok(event) = serde_json::from_slice::<Event>(&line) {
        return Ok(Some(event.seq));
    }
    file.seek(SeekFrom::Start(0))?;
    let mut head = Vec::new();
    file.take(TAIL).read_to_end(&mut head)?;
    Ok(head
        .split(|byte| *byte == b'\n')
        .find_map(|line| serde_json::from_slice::<Event>(line).ok())
        .map(|event| event.seq))
}

/// Every event in the log in `dir`, oldest first, across its sealed segments and the one being
/// written. A line that is not an event (a write the machine died in the middle of) is
/// skipped.
pub fn read(dir: &Path) -> io::Result<Vec<Event>> {
    let mut paths: Vec<PathBuf> = segments(dir)?.into_iter().map(|(_, path)| path).collect();
    paths.push(dir.join(FILE));
    let mut events = Vec::new();
    for path in paths {
        let text = match std::fs::read(&path) {
            Ok(text) => text,
            Err(why) if why.kind() == io::ErrorKind::NotFound => continue,
            Err(why) => return Err(why),
        };
        events.extend(
            text.split(|byte| *byte == b'\n')
                .filter_map(|line| serde_json::from_slice::<Event>(line).ok()),
        );
    }
    Ok(events)
}

/// What a subscription hands its client: an event, or word that some were missed.
#[derive(Debug, Clone, PartialEq)]
#[allow(
    clippy::large_enum_variant,
    reason = "nearly every delivery is an event: boxing each would allocate once per event \
              for the sake of a marker that comes rarely"
)]
pub enum Delivery {
    Event(Event),
    /// The events after `after` up to `resumes_at` are no longer kept ([`Retention`]): the
    /// stream carries on from the oldest event the log still holds, which is `resumes_at`. A
    /// client that held state built from the events it missed rebuilds it from what follows.
    /// Never a silent gap (ADR 0066).
    Missed {
        after: u64,
        resumes_at: u64,
    },
    /// The client's `cursor` is past the log's last event, `log_ends_at` (0 for none): the
    /// log lost lines the client had already read (a power loss, since a line is `fsync`ed
    /// only as the host answers it), and the next host numbers its events from after
    /// `log_ends_at` again. The stream carries on from there, so each of those events reaches
    /// the client; what it held past `log_ends_at` is no longer in the log, and it rebuilds
    /// from what follows. Never a silent skip of an event that reuses a number (#941).
    Ahead {
        cursor: u64,
        log_ends_at: u64,
    },
}

/// A client's place in one device's log ([`subscribe`]): it reads every event after its
/// cursor, in order, and then each one written after that, for as long as it is polled.
///
/// **It holds nothing but its cursor and the file it is reading**, so it outlives the writer:
/// a host that is killed and started again carries on in the same files, and a subscription
/// made again from [`Subscription::cursor`] reads on from the same place. A line the writer
/// died in the middle of is never delivered, because only whole lines are read, and the next
/// writer cuts it off and writes that `seq` whole.
pub struct Subscription {
    dir: PathBuf,
    /// The last `seq` delivered, or the cursor it was opened at.
    last: u64,
    reading: Option<Reading>,
    /// The first seq of the last segment read to its end, so it is not read again.
    past: u64,
    /// Whether the log has been seen to reach the cursor, so it is not ahead of it. Asked
    /// until it has: a log that has reached the cursor can only lose lines past it by losing
    /// the client with them, as one machine.
    reached: bool,
}

/// The segment a subscription is reading.
struct Reading {
    file: File,
    /// How far into it the whole lines read reach.
    offset: u64,
    /// The seq of its first event, once one is read: the name it is sealed under.
    first: Option<u64>,
    /// Whether it was a sealed segment when opened, so its end is the end.
    sealed: bool,
}

/// Subscribes to the log in `dir` from `since`, the last `seq` the client holds (0 for none):
/// ADR 0066's `subscribe(since)`.
pub fn subscribe(dir: &Path, since: u64) -> Subscription {
    Subscription {
        dir: dir.to_owned(),
        last: since,
        reading: None,
        past: 0,
        reached: since == 0,
    }
}

impl Subscription {
    /// The last `seq` this subscription delivered, which is what a client resubscribes from.
    pub fn cursor(&self) -> u64 {
        self.last
    }

    /// Every event written since the last poll, in order, each once. Never blocks: an empty
    /// answer means nothing new yet.
    pub fn poll(&mut self) -> io::Result<Vec<Delivery>> {
        let mut got = self.read_on()?;
        if !got.is_empty() {
            self.reached = true;
        } else if !self.reached {
            let end = log_end(&self.dir)?;
            self.reached = true;
            if end < self.last {
                got.push(Delivery::Ahead {
                    cursor: self.last,
                    log_ends_at: end,
                });
                self.last = end;
                // From the start again, with the cursor where the log is.
                self.reading = None;
                self.past = 0;
                got.extend(self.read_on()?);
            }
        }
        Ok(got)
    }

    /// What [`Subscription::poll`] reads: every event after the cursor that is there now.
    fn read_on(&mut self) -> io::Result<Vec<Delivery>> {
        let mut got = Vec::new();
        loop {
            let mut reading = match self.reading.take() {
                Some(reading) => reading,
                None => match self.find()? {
                    Some(reading) => reading,
                    None => return Ok(got),
                },
            };
            // Whether the segment is finished is asked BEFORE it is read: it is renamed only
            // after its last line was written, so what is read next is all of it. A segment
            // still being written is the live end, asked again at the next poll (by then with
            // its first seq, read now, where it had none).
            let sealed = reading.sealed || self.is_finished(reading.first)?;
            read_on(&mut reading, &mut self.last, &mut got)?;
            if !sealed {
                self.reading = Some(reading);
                return Ok(got);
            }
            self.past = reading.first.unwrap_or(self.past);
        }
    }

    /// Whether the live segment being read, whose first event is `first`, is no longer the
    /// one being written: `events.jsonl` is now another file, or none. That holds whether the
    /// segment is still there under its sealed name or was sealed and pruned between two
    /// polls. Told by the first seq, not an inode, so it means the same on every platform:
    /// two segments never begin with the same `seq`, and a new one begins empty.
    fn is_finished(&self, first: Option<u64>) -> io::Result<bool> {
        let Some(first) = first else {
            // Nothing read yet, so nothing to tell it by; an empty segment is never sealed.
            return Ok(false);
        };
        match File::open(self.dir.join(FILE)) {
            Ok(mut now) => Ok(first_seq(&mut now)? != Some(first)),
            Err(why) if why.kind() == io::ErrorKind::NotFound => Ok(true),
            Err(why) => Err(why),
        }
    }

    /// Opens the segment that holds the event after the cursor: the newest sealed segment not
    /// yet read that begins at or before it; else, when the cursor falls in a hole before a
    /// sealed segment (one pruned, or older than every one kept), the oldest sealed segment
    /// not yet read; else the one being written. The gap itself is told by [`read_on`].
    fn find(&mut self) -> io::Result<Option<Reading>> {
        loop {
            // The segment being written is opened BEFORE the sealed ones are listed. Listed
            // first, a seal between the two would go unseen: the list would lack the segment
            // just sealed and the open would find the new, later one, and every event in
            // between would be skipped. Opened first, the file held is either listed as sealed
            // by then, or it is still the newest segment there is.
            let live = match File::open(self.dir.join(FILE)) {
                Ok(file) => Some(file),
                Err(why) if why.kind() == io::ErrorKind::NotFound => None,
                Err(why) => return Err(why),
            };
            let sealed = segments(&self.dir)?;
            let wanted = self.last.saturating_add(1);
            let unread = |first: &u64| *first > self.past;
            let next = sealed
                .iter()
                .rev()
                .find(|(first, _)| unread(first) && *first <= wanted)
                .or_else(|| sealed.iter().find(|(first, _)| unread(first)));
            let Some((first, path)) = next else {
                return Ok(live.map(|file| Reading {
                    file,
                    offset: 0,
                    first: None,
                    sealed: false,
                }));
            };
            match File::open(path) {
                Ok(file) => {
                    return Ok(Some(Reading {
                        file,
                        offset: 0,
                        first: Some(*first),
                        sealed: true,
                    }));
                }
                // Pruned since it was listed: list again.
                Err(why) if why.kind() == io::ErrorKind::NotFound => continue,
                Err(why) => return Err(why),
            }
        }
    }
}

/// The `seq` of the last whole event in the log in `dir`, 0 for none: the segment being
/// written's, or where that has none, the newest sealed segment's, which is never pruned.
/// Read in that order, so a seal between the two reads is seen in one or the other.
fn log_end(dir: &Path) -> io::Result<u64> {
    let live = match File::open(dir.join(FILE)) {
        Ok(mut file) => last_seq(&mut file)?,
        Err(why) if why.kind() == io::ErrorKind::NotFound => None,
        Err(why) => return Err(why),
    };
    if let Some(seq) = live {
        return Ok(seq);
    }
    match segments(dir)?.last() {
        Some((_, path)) => match File::open(path) {
            Ok(mut file) => Ok(last_seq(&mut file)?.unwrap_or(0)),
            Err(why) if why.kind() == io::ErrorKind::NotFound => Ok(0),
            Err(why) => Err(why),
        },
        None => Ok(0),
    }
}

/// Reads the whole lines of `reading` after its offset, delivering each event after `last`.
fn read_on(reading: &mut Reading, last: &mut u64, got: &mut Vec<Delivery>) -> io::Result<()> {
    reading.file.seek(SeekFrom::Start(reading.offset))?;
    let mut bytes = Vec::new();
    (&reading.file).read_to_end(&mut bytes)?;
    // Only whole lines: one the writer is still writing is read on a later poll.
    let whole = bytes
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |at| at + 1);
    reading.offset += whole as u64;
    for line in bytes[..whole].split(|byte| *byte == b'\n') {
        let Ok(event) = serde_json::from_slice::<Event>(line) else {
            continue;
        };
        reading.first.get_or_insert(event.seq);
        if event.seq <= *last {
            continue;
        }
        if event.seq > last.saturating_add(1) {
            // Events between the cursor and this one are no longer kept: say so, never a
            // silent gap (ADR 0066).
            got.push(Delivery::Missed {
                after: *last,
                resumes_at: event.seq,
            });
        }
        *last = event.seq;
        got.push(Delivery::Event(event));
    }
    Ok(())
}

/// The file in a device's directory that holds the key its args digests are made with.
pub const ARGS_KEY: &str = "args.key";

/// How many random bytes the args key is.
const KEY_BYTES: usize = 32;

/// The device's key for args digests (O3: "with an args digest, never raw args").
///
/// **Keyed, because a plain hash of a short command can be guessed.** `ls -la` has one SHA-256
/// and anybody can compute it; an HMAC under a key only this device holds cannot be matched
/// from outside, and still lets two calls on this device be seen to have had the same
/// arguments. The key is made once and kept, as `fingerprint.key` is for a project's `fp:`
/// values, so a digest compares with yesterday's; AU-19 may move it into the keyring.
///
/// **Who can read it:** anything running as the same OS user, and whoever holds a backup that
/// carries it (FR-10 backs it up with the log). The key keeps the digests from everyone else.
/// The hook sends the host the arguments' *unkeyed* SHA-256, on the chat's own hook channel,
/// which only the same user can reach.
pub struct ArgsKey(Vec<u8>);

impl ArgsKey {
    /// The key in `dir`, made on first use: [`crate::secrets::key_file`], as `fingerprint.key`
    /// is.
    pub fn open(dir: &Path) -> io::Result<ArgsKey> {
        crate::secrets::key_file(dir, &dir.join(ARGS_KEY), KEY_BYTES).map(ArgsKey)
    }

    /// The digest the log holds for arguments whose [`args_hash`] is `hash`: HMAC-SHA256 under
    /// this device's key, in hex.
    pub fn digest(&self, hash: &str) -> String {
        use hmac::{KeyInit, Mac};
        let mut mac = hmac::Hmac::<sha2::Sha256>::new_from_slice(&self.0)
            .expect("HMAC takes a key of any length");
        mac.update(hash.as_bytes());
        crate::extension::hex(&mac.finalize().into_bytes())
    }
}

/// SHA-256 of a tool call's arguments as JSON, in hex: what a hook sends the host in place of
/// the arguments, which never leave the hook's process.
pub fn args_hash(args: &serde_json::Value) -> String {
    use sha2::Digest;
    let text = serde_json::to_vec(args).unwrap_or_default();
    crate::extension::hex(&sha2::Sha256::digest(&text))
}

/// What the board did with a report's conversation, which is how the host tells a new run
/// (ADR 0066, "A run is a stretch…").
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Followed {
    /// The chat is in the conversation it was in, or the board refused the report.
    No,
    /// The harness named its conversation for the first time (Codex and opencode do, in the
    /// first turn's hook). The run gains a conversation; it does not start again.
    FirstNamed,
    /// The board followed the chat off one conversation onto another: `/clear`, a new run.
    Moved,
}

/// Why a run began: ADR 0066's `cause`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Began {
    /// The chat was created: its first run. Also a chat's first line when the host was never
    /// told who it is, which no chat charter starts produces.
    Start,
    /// The board followed the chat onto another conversation.
    Clear,
    /// A sub-agent's first call: a child run of the run that was current.
    Child,
    /// The app relaunched the chat and resumed its conversation (`reopen.json`'s `resume`).
    Reopen,
    /// A hibernated chat was resumed (SC-4).
    Wake,
    /// The chat was started again without its conversation: its harness could not find it
    /// (`lostOnResume`), or a workspace rename dropped it.
    Fresh,
    /// An attribute of the run changed while the chat carried on: its harness, profile, model,
    /// persona or sandbox.
    Switch,
}

impl Began {
    /// The cause's word, as `run.started`'s body says it.
    pub fn word(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Clear => "clear",
            Self::Child => "child",
            Self::Reopen => "reopen",
            Self::Wake => "wake",
            Self::Fresh => "fresh",
            Self::Switch => "switch",
        }
    }

    /// Why a chat a relaunch put back begins a run, from how it came back: `reopen` when it
    /// is in the conversation it had, or had none to lose (a program that is no harness), and
    /// `fresh` when a harness chat came back without it.
    ///
    /// A chat whose own arguments name a session is `reopen`: the operator's words resume it,
    /// and charter has no other answer to hold them to.
    pub fn at_relaunch(how: &crate::reopen::Reopened, harness: bool) -> Began {
        use crate::reopen::{Fresh, Reopened};
        match how {
            Reopened::Resumed(_) | Reopened::Fresh(Fresh::SessionNamedByTheOperator) => {
                Self::Reopen
            }
            Reopened::Fresh(_) if !harness => Self::Reopen,
            Reopened::Fresh(_) => Self::Fresh,
        }
    }
}

/// A run by its two ids, the chat's and its own: named, so a call cannot swap the two ULIDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunOf<'a> {
    pub chat: &'a str,
    pub run: &'a str,
}

/// What `run.started` says for an attribute the host does not have (#868): said, never left
/// out.
pub const UNKNOWN: &str = "unknown";

/// What `run.started` says for a chat on no persona, a chat that runs no harness (a shell), or a
/// run under no sandbox: a fact, not a gap.
pub const NONE: &str = "none";

/// What a run is beyond its ids and its cause: ADR 0066's attributes ("A run is a stretch…"),
/// with the `provider` and `model` ADR 0087 adds. Fixed for the run's whole life, and every one
/// written into its `run.started`, read from what the host decided as it started the chat and
/// never from a hook's word but the conversation the board took.
///
/// **A fact the host does not have is said** (#868): `None` is written as [`UNKNOWN`], never
/// left out, so a reader tells "not known" from "not written by this version". The chat and
/// the parent run are the envelope's, and the cause is the [`Began`] the run began for.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Attributes {
    /// The persona the chat has adopted, by name, or [`NONE`].
    pub persona: Option<String>,
    /// The harness, by the word the plane calls it (`claude`, `codex`, `opencode`), or [`NONE`]
    /// for a chat that runs no harness.
    pub harness: Option<String>,
    /// The harness profile, by name and never by its command (ADR 0022).
    pub profile: Option<String>,
    /// Where the model is reached, from the run's resolved model choice (ADR 0087: `login`,
    /// `key`, `gateway` or `local`). Nothing resolves a choice yet, so a run says [`UNKNOWN`].
    pub model_source: Option<String>,
    /// The provider of the run's resolved model choice (ADR 0087 §6).
    pub provider: Option<String>,
    /// The model of the run's resolved choice (ADR 0087 §6). A model a harness reports later is
    /// a `switch`, and is not written into the run it reports in.
    pub model: Option<String>,
    /// The sandbox backend the run is under (`seatbelt`, `bubblewrap`), or [`NONE`].
    pub sandbox: Option<String>,
    /// The harness's own id for the conversation, where one is known as the run begins.
    pub conversation: Option<String>,
    /// Who started the chat, by the session protocol's word for the scope of the link the start
    /// came on (`local-ui` for the window, `terminal`, `remote-link`), where it is known
    /// (#2171).
    pub started_by: Option<String>,
}

impl Attributes {
    /// `run.started`'s body for a run that began for `cause` on `device`: every attribute, each
    /// [`UNKNOWN`] where it is not known. A conversation not in a session id's shape is not one
    /// (it came from a hook), and reads as not known.
    fn body(&self, cause: Began, device: &str) -> serde_json::Value {
        let said = |v: &Option<String>| -> String {
            v.as_deref()
                .filter(|v| !v.is_empty())
                .unwrap_or(UNKNOWN)
                .to_owned()
        };
        let conversation = self
            .conversation
            .as_deref()
            .filter(|id| crate::harness::SessionId::new(*id).is_ok())
            .map(str::to_owned);
        serde_json::json!({
            "cause": cause.word(),
            "device": device,
            "persona": said(&self.persona),
            "harness": said(&self.harness),
            "profile": said(&self.profile),
            "model_source": said(&self.model_source),
            "provider": said(&self.provider),
            "model": said(&self.model),
            "sandbox": said(&self.sandbox),
            "conversation": said(&conversation),
            "started_by": said(&self.started_by),
        })
    }

    /// A child run's (ADR 0066, "A child agent…"): its parent's, but for the conversation, which
    /// is the sub-agent's own and one no harness names to purlis.
    fn of_a_child(&self) -> Attributes {
        Attributes {
            conversation: None,
            ..self.clone()
        }
    }
}

/// Which end of a tool call a tool hook is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Before the tool runs: `PreToolUse`.
    Pre,
    /// After it ran: `PostToolUse`, or `PostToolUseFailure` for a call that failed.
    Post,
}

/// The phase of a tool hook word charter answers, from `hookreg` and nothing else: a word it
/// does not answer is no phase, and no event kind.
pub fn phase(word: &str) -> Option<Phase> {
    let event = crate::hookreg::find(word)
        .filter(|handler| handler.matcher.is_some())
        .map(|handler| handler.event)
        .or_else(|| {
            crate::hookreg::NO_OPS
                .contains(&word)
                .then_some("PostToolUse")
        })?;
    match event {
        "PreToolUse" => Some(Phase::Pre),
        // A failed call's post hook is still the end of the call.
        "PostToolUse" | "PostToolUseFailure" => Some(Phase::Post),
        _ => None,
    }
}

/// A spool drain's kinds (ADR 0075 §4's registry): a gap in a chat's sequence, a line that did
/// not check, a sequence drained, and a refused commit's line drained from a spool.
pub const SPOOL_GAP: &str = "hook.spool.gap";
pub const SPOOL_REJECTED: &str = "hook.spool.rejected";
pub const SPOOL_DRAINED: &str = "hook.spool.drained";
pub const COMMIT_REFUSED: &str = "hook.commit_refused";

/// `body` with `spooled`, the number a drained line had in its chat's spool, when it had one
/// (FD-30): the one place an event is marked as having come through the spool. Added after
/// what the body already says, so every event keeps its keys in the order it always had.
fn spooled_as(mut body: serde_json::Value, seq: Option<u64>) -> serde_json::Value {
    if let Some(seq) = seq {
        body["spooled"] = seq.into();
    }
    body
}

/// The kind a tool call's event is recorded under: `hook.<word>` for a word charter answers.
fn tool_kind(call: &crate::hookwire::ToolCall) -> String {
    match phase(&call.tool_hook) {
        Some(_) => format!("hook.{}", call.tool_hook),
        None if call.tool_hook == crate::hookwire::SKILL_LOAD => SKILL_LOADED.to_owned(),
        None => UNKNOWN_HOOK.to_owned(),
    }
}

/// The kind a skill purlis's MCP server loaded is recorded under (SK-13, #2776): the tool call
/// that loaded it, just after the `skill.started` of the run it opened.
pub const SKILL_LOADED: &str = "hook.skill-load";

/// The kind a tool hook word charter does not answer is recorded under, so a line cannot mint
/// a kind of its own: the word goes in the body, shortened.
pub const UNKNOWN_HOOK: &str = "hook.unknown";

/// Who a chat is in the log: its id and its current run's.
#[derive(Debug, Clone)]
struct Identity {
    chat: String,
    run: String,
    /// What the current run is: the next run a clear begins, and every child run, carries it.
    attributes: Attributes,
    /// Why the run began, while its `run.started` is held ([`Recorder::begin_held`]).
    held: Option<Began>,
    /// The trust event held with it, written just before it (#2252).
    held_trust: Option<HeldTrust>,
}

/// A start's trust event held with its run's `run.started` ([`Recorder::begin_held`]): the
/// sandbox coming back on for a start the host may still take back (#2252). Written just
/// before the `run.started`, as at any start (ADR 0067 §7), and never for a start taken back.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HeldTrust {
    kind: String,
    body: serde_json::Value,
}

impl HeldTrust {
    /// The trust event for `change`, with the chat's `harness` and `persona`, as
    /// [`Recorder::trust`] writes it.
    pub fn of(
        change: &crate::sandbox::Change,
        harness: Option<crate::harness::Harness>,
        persona: Option<&str>,
    ) -> Self {
        Self {
            kind: change.kind().to_owned(),
            body: change.body(harness, persona),
        }
    }

    /// The event's kind: `trust.sandbox.on` or `trust.sandbox.off`.
    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// The event's body.
    pub fn body(&self) -> &serde_json::Value {
        &self.body
    }
}

/// The run an event is recorded under, and the run that one came from.
#[derive(Debug, Clone)]
struct Under {
    chat: String,
    run: String,
    parent: Option<String>,
}

/// The kind a kill-switch journal line is recorded as (ADR 0075's registry).
pub fn kill_switch_kind(event: crate::halt::Event) -> &'static str {
    match event {
        crate::halt::Event::Stop => "killswitch.stop",
        crate::halt::Event::Rearm => "killswitch.rearm",
        crate::halt::Event::Tamper => "killswitch.tamper",
    }
}

/// A kill-switch event's body: who wrote the line, as the journal says (`by`), the actor kind
/// that makes it (ADR 0075, "ADR 0071, amended"), and the line's own time (`at`, epoch
/// seconds), which can be earlier than the event's for a line read at the next start.
///
/// A `window` line is a `human` one, the operator on `local-ui`; an `app` line, the tamper the
/// app put right, is the `host`'s; a `cli` line is `unattributed`, because any process of the
/// user can run `purlis stop --all` and nothing says a person did.
fn kill_switch_body(entry: &crate::halt::Entry) -> serde_json::Value {
    use crate::halt::Actor;
    let (actor_kind, actor) = match (entry.by, entry.event) {
        (Actor::Window, _) => ("human", "operator"),
        (Actor::App, _) => ("host", "purlis"),
        (Actor::Cli, _) => ("unattributed", "unattributed"),
        // A link's re-arm is the desktop's window's, a person's; its stop may come from any
        // scope of the desktop (ADR 0075, "ADR 0071, amended").
        (Actor::Link, crate::halt::Event::Rearm) => ("human", "operator"),
        (Actor::Link, _) => ("unattributed", "unattributed"),
    };
    serde_json::json!({
        "actor_kind": actor_kind,
        "actor": actor,
        "by": entry.by,
        "at": entry.at,
    })
}

/// The journal line a kill-switch event records, read back from it.
fn kill_switch_entry(event: &Event) -> Option<crate::halt::Entry> {
    use crate::halt::Event as Switch;
    let switched = match event.kind.as_str() {
        "killswitch.stop" => Switch::Stop,
        "killswitch.rearm" => Switch::Rearm,
        "killswitch.tamper" => Switch::Tamper,
        _ => return None,
    };
    Some(crate::halt::Entry {
        at: event.body.get("at")?.as_u64()?,
        event: switched,
        by: serde_json::from_value(event.body.get("by")?.clone()).ok()?,
    })
}

/// What the log holds of the kill switch's journal: each line, by how many events record it.
#[derive(Debug, Default)]
struct KillSwitchSeen {
    logged: HashMap<crate::halt::Entry, usize>,
    /// Each line this host wrote, by how many times.
    wrote: HashMap<crate::halt::Entry, usize>,
    /// The journal as it last was read whole, so a look that finds it unchanged reads nothing.
    read_as: Option<(u64, std::time::SystemTime)>,
}

impl KillSwitchSeen {
    /// Lets go of lines older than the log keeps, which are never recorded again.
    fn forget_before(&mut self, oldest: u64) {
        self.logged.retain(|entry, _| entry.at >= oldest);
        self.wrote.retain(|entry, _| entry.at >= oldest);
    }
}

/// How many of `owed` are `entry`.
fn owed_of(owed: &[crate::halt::Entry], entry: &crate::halt::Entry) -> usize {
    owed.iter().filter(|owed| *owed == entry).count()
}

/// The journal's length and time of change, or none where it cannot be read.
fn journal_signature(config: &Path) -> Option<(u64, std::time::SystemTime)> {
    let found = std::fs::metadata(crate::halt::journal_path(config)).ok()?;
    Some((found.len(), found.modified().ok()?))
}

/// How many events in the log at `dir` record each kill-switch line from `since` (epoch
/// seconds) on. Only the segments that end after it are read, and only the lines that name a
/// kill-switch kind are parsed. An hour's slack is read before it, for a clock set back.
fn kill_switch_logged(dir: &Path, since: u64) -> io::Result<HashMap<crate::halt::Entry, usize>> {
    let from_ms = since.saturating_sub(3600).saturating_mul(1000);
    let mut paths = Vec::new();
    for (_, sealed) in segments(dir)? {
        let newest = last_event(&mut File::open(&sealed)?)?
            .and_then(|event| ulid::Ulid::from_string(&event.ulid).ok())
            .map(|id| id.timestamp_ms());
        if newest.is_none_or(|newest| newest >= from_ms) {
            paths.push(sealed);
        }
    }
    paths.push(dir.join(FILE));
    let mut logged = HashMap::new();
    for path in paths {
        let text = match std::fs::read(&path) {
            Ok(text) => text,
            Err(why) if why.kind() == io::ErrorKind::NotFound => continue,
            Err(why) => return Err(why),
        };
        for line in text.split(|byte| *byte == b'\n') {
            if !line
                .windows(KILL_SWITCH_MARK.len())
                .any(|at| at == KILL_SWITCH_MARK)
            {
                continue;
            }
            if let Some(entry) = serde_json::from_slice::<Event>(line)
                .ok()
                .as_ref()
                .and_then(kill_switch_entry)
            {
                *logged.entry(entry).or_default() += 1;
            }
        }
    }
    Ok(logged)
}

/// The file beside the log that says the journal, by its length and time of change, as it was
/// when the log last held every line of it (#1375). A host that starts with the journal still
/// so does not read back through the log for it: that read can cover the log's whole
/// [`Retention::keep`], and it would otherwise run at every start while a line is that recent.
/// Only a cache: a missing or different one is a read.
pub const KILL_SWITCH_READ: &str = "killswitch.read";

/// How [`KILL_SWITCH_READ`] says a journal signature.
fn kill_switch_signature_text(signature: (u64, std::time::SystemTime)) -> Option<String> {
    let changed = signature.1.duration_since(std::time::UNIX_EPOCH).ok()?;
    Some(format!("{} {}\n", signature.0, changed.as_nanos()))
}

/// What every kill-switch kind's line holds, looked for before a line is parsed.
const KILL_SWITCH_MARK: &[u8] = b"\"killswitch.";

/// The host's side of the log: it turns what the hook channel hears into events, one per hook
/// call, each under the chat and the run it happened in.
///
/// **The host decides runs, never a hook** (ADR 0066, "What changes where"): a hook carries
/// the chat's number and token and nothing more, and the host maps the number to the chat's
/// id and current run in its own memory.
///
/// - The host tells it each chat it starts ([`Recorder::begin`]): the chat's id, which the app
///   keeps across a relaunch, and the run it begins, with why (`start`, `reopen`, `fresh`,
///   `switch`, `wake`).
/// - A chat it was never told of is given an id and its first run, `cause: start`, at its
///   first line, whatever the board made of that line. Only a chat charter started holds a
///   token the channel admits, so such a line is from a chat this host started.
/// - A chat the board follows onto another conversation begins a run with `cause: clear`. A
///   line the board refused (a harness nested in the chat's shell, ADR 0024 C5) is `No` and
///   never does.
/// - A sub-agent's calls are a child run (`cause: child`) of the run that was current when it
///   first appeared, until its `SubagentStop` (ADR 0066, "A child agent…").
pub struct Recorder {
    /// The device's directory under `<data>/events/`, where it is known.
    dir: PathBuf,
    /// The machine store it read its device id from, where it was opened from one: where the
    /// kill switch's journal is ([`Entry::KillSwitchWrote`]).
    config: Option<PathBuf>,
    log: Log,
    key: ArgsKey,
    /// By project and number: a number means nothing outside the project that dealt it.
    chats: HashMap<(PathBuf, u32), Identity>,
    /// The numbers whose held run was taken back ([`Recorder::taken_back`]), by project: a
    /// later line from one is recorded with no chat or run, never under a run of its own. It
    /// stays small: only a start put aside adds a number, one each, and a number told again
    /// ([`Recorder::begin_held`], [`Recorder::knows`]) leaves it.
    taken_back: std::collections::HashSet<(PathBuf, u32)>,
    /// Each live sub-agent's child run, by chat id and the harness's agent id.
    children: HashMap<(String, String), (String, String)>,
    /// The sub-agents whose `SubagentStop` was heard, by chat id and agent id: a later line
    /// from one is its parent run's, never a second child run.
    stopped: std::collections::HashSet<(String, String)>,
    /// The first of each tool call's pre and post hooks heard, by run and the harness's id for
    /// the call: which it was, when the host heard it, and when the hook said it began.
    calls: HashMap<(String, String), Heard>,
    /// What the log holds of the kill switch's journal, once it was first looked at.
    kill_switch: Option<KillSwitchSeen>,
    /// The journal as it was when a start found the log already held all of it
    /// ([`KILL_SWITCH_READ`]), so the log was not read for it.
    kill_switch_skipped: Option<(u64, std::time::SystemTime)>,
    /// Every skill run open, by the chat run it is a span of (SK-3, [`crate::skillspan`]).
    skills: crate::skillspan::Tracker,
    /// The prompt a curation action or a trigger put in each chat, by project and number, until
    /// the chat's next report hands it to its run ([`Recorder::skill_prompted`]): held from
    /// before the chat starts, so it is there however soon the chat's first prompt comes.
    prompted: HashMap<(PathBuf, u32), crate::skillspan::Prompted>,
    /// Every run this writer began in the log, ended or not, up to [`RUNS_HELD`]: an id no run
    /// a clear begins may take (#3394).
    begun: std::collections::HashSet<String>,
    /// The time of the newest run let go of from [`Self::begun`] when it was full, in
    /// milliseconds since 1970 by its id: an id no later is not taken either (#3394).
    begun_until_ms: u64,
}

/// How many runs a writer remembers it began ([`Recorder::never_held`]). Past it, they are let
/// go of, and only an id minted later than the newest of them is taken.
const RUNS_HELD: usize = 4096;

/// **How far ahead of the newest event the log held as it was opened an id must be minted** for
/// a run a clear begins to take it (#3394): the window that mints the id and the host that
/// writes the log keep time apart, and their clocks are taken to agree within this.
const CLOCKS_AGREE_WITHIN_MS: u64 = 5 * 60 * 1000;

/// How many chats' prompts [`Recorder::skill_prompted`] holds at most: a start that never
/// reported is let go when there are more.
const PROMPTED_HELD: usize = 256;

/// One end of a tool call, heard and waiting for the other.
#[derive(Debug, Clone, Copy)]
struct Heard {
    phase: Phase,
    at: Instant,
    at_ms: u64,
}

/// How long one end of a tool call is waited on for the other. One that never came (the
/// harness died, or a guard refused the call) is a duration lost, not a leak: it is let go
/// when anything is next recorded after this long, and an end heard later gives no duration.
pub const CALL_IS_OPEN_AT_MOST: std::time::Duration = std::time::Duration::from_secs(3600);

/// How many stopped sub-agents are remembered.
const AGENTS_HELD: usize = 4096;

impl Recorder {
    /// The recorder for this device, its log under `<data>` and its device id from the
    /// machine store under `config`.
    ///
    /// `<data>/events/<device>/`: by device, as the audit is (ADR 0075 §6), so a backup
    /// restored onto another machine is kept as that device's records and never appended to
    /// (ADR 0069 §5). A `<data>` inside a project or a git work tree is refused before
    /// anything is made there.
    pub fn open_in(config: &Path, data: &Path) -> io::Result<Recorder> {
        if let Some(why) = crate::datahome::refusal(data) {
            return Err(io::Error::new(io::ErrorKind::PermissionDenied, why));
        }
        let device = crate::machine::device_id(config)?;
        let dir = data.join(EVENTS).join(&device);
        let mut recorder = Recorder::new(Log::open(&dir, &device)?, ArgsKey::open(&dir)?);
        recorder.dir = dir;
        recorder.config = Some(config.to_path_buf());
        // The kill switch's journal, read as the host starts (#1375): a line written while no
        // host ran is recorded now. Never a refusal to log: the switch holds whatever this says.
        if let Err(why) = recorder.kill_switch_read(config) {
            tracing::warn!(error = %why, "the kill switch's journal was not recorded");
        }
        Ok(recorder)
    }

    /// [`Recorder::open_in`] for this process: the machine store's and `<data>`'s own homes.
    pub fn open() -> io::Result<Recorder> {
        let config = crate::machine::config_root()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no config home"))?;
        let data = crate::datahome::root()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no data home"))?;
        Recorder::open_in(&config, &data)
    }

    /// Where [`Recorder::open`] keeps this machine's log, without opening it: what the window
    /// says of the log while the session host writes it (#2003). `None` where there is none to
    /// open.
    pub fn dir_on_this_machine() -> Option<PathBuf> {
        let config = crate::machine::config_root()?;
        let data = crate::datahome::root()?;
        if crate::datahome::refusal(&data).is_some() {
            return None;
        }
        let device = crate::machine::device_id(&config).ok()?;
        Some(data.join(EVENTS).join(device))
    }

    pub fn new(log: Log, key: ArgsKey) -> Recorder {
        Recorder {
            dir: PathBuf::new(),
            config: None,
            log,
            key,
            chats: HashMap::new(),
            children: HashMap::new(),
            stopped: std::collections::HashSet::new(),
            taken_back: std::collections::HashSet::new(),
            calls: HashMap::new(),
            kill_switch: None,
            kill_switch_skipped: None,
            skills: crate::skillspan::Tracker::default(),
            prompted: HashMap::new(),
            begun: std::collections::HashSet::new(),
            begun_until_ms: 0,
        }
    }

    /// The device's directory the log is in, as [`Recorder::open_in`] found it.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// **Whether `run` is an id the log never held** (#3394), so a run may begin under it: an
    /// id minted more than [`CLOCKS_AGREE_WITHIN_MS`] after the newest event the log held as it
    /// was opened, and no run's this writer began or holds since. The log is never read back
    /// for it: an id is minted before the run it names is written, so one minted later than
    /// every event the log held then, by clocks that agree within that margin, is none of
    /// theirs.
    pub(crate) fn never_held(&self, run: &str) -> bool {
        let bound = self
            .log
            .held_until_ms
            .saturating_add(CLOCKS_AGREE_WITHIN_MS);
        ulid::Ulid::from_string(run)
            .is_ok_and(|id| id.timestamp_ms() > bound && id.timestamp_ms() > self.begun_until_ms)
            && !self.begun.contains(run)
            && !self.chats.values().any(|who| who.run == run)
    }

    /// The conversation chat `number` of `plane`'s current run is in, as this writer recorded
    /// it: `None` for a chat it holds no run of, `Some(None)` for a run in none yet (#3394).
    pub(crate) fn run_conversation(&self, plane: &Path, number: u32) -> Option<Option<&str>> {
        self.chats
            .get(&(plane.to_path_buf(), number))
            .map(|who| who.attributes.conversation.as_deref())
    }

    /// **What a state hook's line followed, as this writer takes it from a window** (#3394):
    /// what the window's board made of `report` (`claimed`), checked against the conversation
    /// this writer recorded the chat's run in. A move is taken only onto a conversation the
    /// line names that is not the run's, and a first name only for a run in none. A line naming
    /// another conversation than its run's is a move whatever else the board said of it, since
    /// a run is a stretch of one conversation (ADR 0066): a first name there begins the run
    /// that follows, as a `clear` (the chat was followed onto another conversation, which is
    /// what that cause says). That the chat stayed is always taken: it begins nothing and names
    /// nothing. A chat this writer holds no run of is taken as said.
    pub(crate) fn followed_as_heard(
        &self,
        plane: &Path,
        report: &crate::hookwire::Report,
        claimed: Followed,
    ) -> Followed {
        let Some(current) = self.run_conversation(plane, report.chat) else {
            return claimed;
        };
        let named = match &report.conversation {
            crate::hookwire::Conversation::Named(id) => id.as_str(),
            _ => return Followed::No,
        };
        match (current, claimed) {
            (Some(current), _) if current == named => Followed::No,
            (Some(_), Followed::FirstNamed) => Followed::Moved,
            (_, claimed) => claimed,
        }
    }

    /// The event for one state hook: `hook.<word>`, after a `run.started` when the report
    /// begins a run.
    pub fn report(
        &mut self,
        plane: &Path,
        report: &crate::hookwire::Report,
        followed: Followed,
    ) -> io::Result<Event> {
        self.report_with(plane, report, followed, None)
    }

    /// [`Recorder::report`], with the run the host minted for a move (`cleared`): the run a
    /// `/clear` begins is the host's, so the record can name it whether or not there is a log.
    /// `None` mints one here.
    pub fn report_with(
        &mut self,
        plane: &Path,
        report: &crate::hookwire::Report,
        followed: Followed,
        cleared: Option<&str>,
    ) -> io::Result<Event> {
        self.report_spooled(plane, report, followed, cleared, None)
    }

    /// [`Recorder::report_with`], saying the spool number a drained report had.
    fn report_spooled(
        &mut self,
        plane: &Path,
        report: &crate::hookwire::Report,
        followed: Followed,
        cleared: Option<&str>,
        spooled: Option<u64>,
    ) -> io::Result<Event> {
        // A run held for its start is said before any line of it.
        self.begun(plane, report.chat)?;
        let known = self.chats.contains_key(&(plane.to_path_buf(), report.chat));
        let named = match &report.conversation {
            crate::hookwire::Conversation::Named(id) => Some(id.as_str()),
            _ => None,
        };
        let top = match followed {
            Followed::Moved if known => {
                // The run the window named for the clear, where it is an id the log never
                // held; any other is minted here, and the line still recorded (#3304, #3394).
                let cleared = cleared.filter(|run| self.never_held(run));
                Some(self.new_run(plane, report.chat, Began::Clear, cleared, named)?)
            }
            Followed::Moved | Followed::No | Followed::FirstNamed => {
                self.identity(plane, report.chat)?
            }
        };
        let Some(top) = top else {
            let body = spooled_as(serde_json::json!({}), spooled);
            return self.of_no_run(report.chat, &format!("hook.{}", report.event.word()), body);
        };
        // A harness naming its conversation for the first time keeps the run, which gains it
        // (ADR 0066): what a clear or a child of the run carries on from here.
        if followed == Followed::FirstNamed
            && let Some(id) = named
            && let Some(who) = self.chats.get_mut(&(plane.to_path_buf(), report.chat))
        {
            who.attributes.conversation = Some(id.to_owned());
        }
        let who = Who::of(&top.attributes);
        let under = self.under(top, report.agent.as_deref())?;
        self.skills_heard_of_a_report(plane, &under, &who, report)?;
        let mut body = serde_json::json!({});
        if report.event == crate::state::Event::SessionStart {
            body["started"] = started(report.detail.started).into();
        }
        let body = spooled_as(body, spooled);
        let event = self.append(&under, &format!("hook.{}", report.event.word()), body)?;
        match (report.event, &report.agent) {
            (crate::state::Event::SubagentStop, Some(agent)) => {
                let key = (under.chat.clone(), agent.clone());
                let live = self.children.remove(&key);
                if self.stopped.len() >= AGENTS_HELD {
                    self.stopped.clear();
                }
                self.stopped.insert(key);
                // Its own stop ends its run (ADR 0076 §6): once, for the child that was live.
                if let Some((run, parent)) = live {
                    self.end_child(
                        &under.chat,
                        run,
                        parent,
                        RunState::Completed,
                        Cause::ChildEnded,
                    )?;
                }
            }
            // The chat's session is over, and every sub-agent of its run with it: in the end
            // its parent's run comes to. A clear supersedes the run; a session over for good
            // is the run that ends `completed | exited` at the exit that follows (ADR 0076,
            // ADR 0068 amended). A child has no exit code of its own to say otherwise.
            (crate::state::Event::SessionEnd, None) => {
                let cause = match report.detail.ending {
                    crate::state::Ending::Cleared => Cause::Superseded,
                    crate::state::Ending::ForGood => Cause::Exited(crate::state::run::Exit::Lost {
                        session_ended: true,
                    }),
                };
                self.end_children(&under.chat, RunState::Completed, cause)?;
            }
            _ => {}
        }
        Ok(event)
    }

    /// The event for one tool hook: `hook.<word>` for a word charter answers, with the tool,
    /// the keyed digest of its arguments, what charter answered and how long the hook took. A
    /// post hook whose call's pre hook was heard also says how long the tool ran, from one to
    /// the other, as the host heard them (`now`). A word charter does not answer is recorded
    /// as [`UNKNOWN_HOOK`].
    pub fn tool(
        &mut self,
        plane: &Path,
        call: &crate::hookwire::ToolCall,
        now: Instant,
    ) -> io::Result<Event> {
        self.tool_spooled(plane, call, now, None)
    }

    /// [`Recorder::tool`], saying the spool number a drained call had.
    fn tool_spooled(
        &mut self,
        plane: &Path,
        call: &crate::hookwire::ToolCall,
        now: Instant,
        spooled: Option<u64>,
    ) -> io::Result<Event> {
        let Some(top) = self.identity(plane, call.chat)? else {
            let body = spooled_as(self.tool_body(call), spooled);
            return self.of_no_run(call.chat, &tool_kind(call), body);
        };
        let who = Who::of(&top.attributes);
        let under = self.under(top, call.agent.as_deref())?;
        self.skills_heard_of_a_call(plane, &under, &who, call)?;
        let phase = phase(&call.tool_hook);
        let mut body = spooled_as(self.tool_body(call), spooled);
        if let Some(id) = &call.call {
            let key = (under.run.clone(), id.clone());
            if let Some(phase) = phase
                && let Some(ms) = self.pair(key, phase, now, call.at_ms)
            {
                body["tool_ms"] = ms.into();
            }
        }
        self.append(&under, &tool_kind(call), body)
    }

    /// What a tool event's body says of its call, before any duration.
    fn tool_body(&self, call: &crate::hookwire::ToolCall) -> serde_json::Value {
        let mut body = serde_json::json!({
            "decision": call.decision.word(),
            "hook_ms": call.hook_ms,
        });
        if tool_kind(call) == UNKNOWN_HOOK {
            body["word"] =
                crate::shown::readable(&call.tool_hook, crate::shown::DISPLAY_LIMIT).into();
        }
        if let Some(tool) = &call.tool {
            body["tool"] = tool.as_str().into();
        }
        if let Some(args) = &call.args {
            body["args"] = self.key.digest(args).into();
        }
        if let Some(rule) = &call.rule {
            body["rule"] = rule.as_str().into();
        }
        if let Some(id) = &call.call {
            body["call"] = id.as_str().into();
        }
        body
    }

    /// The id chat `number` in `plane` is known by here, where this recorder was told it or
    /// gave it one: what the session host finds a chat's dispatch by as its program ends
    /// (#2003). `None` for a chat it knows nothing of.
    pub fn chat_of(&self, plane: &Path, number: u32) -> Option<String> {
        self.chats
            .get(&(plane.to_path_buf(), number))
            .map(|who| who.chat.clone())
    }

    /// Tells the recorder who chat `number` in `plane` is and which run it is in, without an
    /// event: a chat the reopen record names, before the host starts anything, so a line it
    /// spooled before the host went away is recorded under the run it ran in (FD-30).
    pub fn knows(&mut self, plane: &Path, number: u32, RunOf { chat, run }: RunOf<'_>) {
        self.taken_back.remove(&(plane.to_path_buf(), number));
        self.chats.insert(
            (plane.to_path_buf(), number),
            Identity {
                chat: chat.to_owned(),
                run: run.to_owned(),
                attributes: Attributes::default(),
                held: None,
                held_trust: None,
            },
        );
    }

    /// The event for one thing a drain of the hook spool found (FD-30, ADR 0075 §4): a line
    /// recorded as it would have been live, with `spooled` its number, or `hook.spool.gap`,
    /// `hook.spool.rejected` or `hook.spool.drained`.
    ///
    /// **A chat the recorder does not know is not given a run.** A drain runs before the host
    /// starts a chat, so a chat it cannot name ([`Recorder::knows`]) is one that had closed; its
    /// events are recorded with no chat or run, under its number (`chat_number`).
    #[cfg(unix)]
    pub fn spooled(
        &mut self,
        plane: &Path,
        item: crate::hookwire::spool::Drained,
    ) -> io::Result<Event> {
        use crate::hookwire::spool::{Drained, Spooled};
        let number = item.chat();
        self.begun(plane, number)?;
        let known = self.chats.get(&(plane.to_path_buf(), number)).cloned();
        let (kind, mut body) = match item {
            Drained::Line { seq, line, .. } => match (line, &known) {
                (Spooled::Report(report), Some(_)) => {
                    return self.report_spooled(plane, &report, Followed::No, None, Some(seq));
                }
                (Spooled::Tool(call), Some(_)) => {
                    return self.tool_spooled(plane, &call, Instant::now(), Some(seq));
                }
                (Spooled::Refused(refused), Some(_)) => {
                    return self.refused_spooled(plane, refused.chat, Some(seq));
                }
                (Spooled::Report(report), None) => (
                    format!("hook.{}", report.event.word()),
                    spooled_as(serde_json::json!({}), Some(seq)),
                ),
                (Spooled::Tool(call), None) => (
                    tool_kind(&call),
                    spooled_as(self.tool_body(&call), Some(seq)),
                ),
                (Spooled::Refused(_), None) => (
                    COMMIT_REFUSED.to_owned(),
                    spooled_as(serde_json::json!({}), Some(seq)),
                ),
            },
            Drained::Gap { from, to, .. } => (
                SPOOL_GAP.to_owned(),
                serde_json::json!({ "from": from, "to": to }),
            ),
            Drained::Rejected { seq, why, .. } => (
                SPOOL_REJECTED.to_owned(),
                serde_json::json!({ "seq": seq, "why": why.word() }),
            ),
            Drained::Spool { from, to, .. } => (
                SPOOL_DRAINED.to_owned(),
                serde_json::json!({ "from": from, "to": to }),
            ),
        };
        body["chat_number"] = number.into();
        match known {
            Some(who) => self.write(Some(&who.chat), Some(&who.run), None, &kind, body),
            None => self.write(None, None, None, &kind, body),
        }
    }

    /// The event for a commit charter's git hook refused in chat `number` (SQ-16):
    /// `hook.commit_refused`, under the chat's run. Metadata only: what was found is the
    /// needs-you item's, never the log's.
    pub fn refused(&mut self, plane: &Path, number: u32) -> io::Result<Event> {
        self.refused_spooled(plane, number, None)
    }

    fn refused_spooled(
        &mut self,
        plane: &Path,
        number: u32,
        spooled: Option<u64>,
    ) -> io::Result<Event> {
        let body = spooled_as(serde_json::json!({}), spooled);
        let Some(who) = self.identity(plane, number)? else {
            return self.of_no_run(number, COMMIT_REFUSED, body);
        };
        self.write(Some(&who.chat), Some(&who.run), None, COMMIT_REFUSED, body)
    }

    /// The trust event for a change to a chat's sandbox: `trust.sandbox.off` or
    /// `trust.sandbox.on` (ADR 0067 §7), under the chat and the run the host is about to begin
    /// for it ([`RunOf`]), with the chat's `harness` and `persona`. Written **before** the
    /// chat's program runs, so an unsandboxed chat is never running unrecorded; an error is
    /// the host's to act on (a person's opt-out is then not started). Written here until the
    /// audit exists, which is written from this log (ADR 0075, amending ADR 0067 §7).
    pub fn trust(
        &mut self,
        RunOf { chat, run }: RunOf<'_>,
        change: &crate::sandbox::Change,
        harness: Option<crate::harness::Harness>,
        persona: Option<&str>,
    ) -> io::Result<Event> {
        self.trust_said(
            RunOf { chat, run },
            change.kind(),
            change.body(harness, persona),
        )
    }

    /// [`Recorder::trust`], its kind and body composed already: what an [`Entry::Trust`]
    /// carries from the window.
    fn trust_said(
        &mut self,
        RunOf { chat, run }: RunOf<'_>,
        kind: &str,
        body: serde_json::Value,
    ) -> io::Result<Event> {
        self.write(Some(chat), Some(run), None, kind, body)
    }

    /// The audit of a sandbox grant or revoke (#1342, [`crate::sandbox::grant::Audited`]):
    /// under chat `number`'s current run where it came from one, with no chat for one taken
    /// back in Settings. Made durable before it answers, so no grant reaches a chat unrecorded.
    pub fn sandbox_grant(
        &mut self,
        plane: &Path,
        number: Option<u32>,
        audited: &crate::sandbox::grant::Audited<'_>,
    ) -> io::Result<Event> {
        self.audited(plane, number, audited.kind(), audited.body())
    }

    /// One event that audits a person's act, `kind` with `body`, under chat `number`'s current
    /// run where it came from one, made durable before it answers: what a grant, a revoke and
    /// an applied change each write, and what an [`Entry::Audited`] carries from the window.
    fn audited(
        &mut self,
        plane: &Path,
        number: Option<u32>,
        kind: &str,
        body: serde_json::Value,
    ) -> io::Result<Event> {
        let who = number
            .map(|number| self.identity(plane, number))
            .transpose()?
            .flatten();
        let event = self.write(
            who.as_ref().map(|who| who.chat.as_str()),
            who.as_ref().map(|who| who.run.as_str()),
            None,
            kind,
            body,
        )?;
        self.log.durable().through(event.seq)?;
        Ok(event)
    }

    /// **The audit of a proposed change the person applied** (#1975, PC-4):
    /// `trust.proposed.apply` under the chat that proposed it, with who applied it (the person,
    /// in the window), the persona the chat runs as, the file and the hash of what was written.
    /// Made durable before the file is written, so no change is applied unrecorded.
    pub fn proposed_applied(
        &mut self,
        plane: &Path,
        number: u32,
        applied: &crate::proposed::Applied<'_>,
    ) -> io::Result<Event> {
        self.audited(
            plane,
            Some(number),
            crate::proposed::APPLIED_KIND,
            applied.body(),
        )
    }

    /// The audit of a dispatch grant or revoke (#1437, [`crate::dispatchgrant::Audited`]):
    /// under chat `number`'s current run where it was allowed from one, with no chat for one
    /// taken back in Settings. Made durable before it answers, so no grant is in force
    /// unrecorded.
    pub fn dispatch_grant(
        &mut self,
        plane: &Path,
        number: Option<u32>,
        audited: &crate::dispatchgrant::Audited<'_>,
    ) -> io::Result<Event> {
        self.dispatch_grant_said(plane, number, audited, None)
    }

    /// [`Recorder::dispatch_grant`] for a grant or a never the person made somewhere other
    /// than a chat's Notice or Settings: the same event, whose body also says where, as
    /// `from` (#1507: `"away"` for one made on a refusal kept while nobody was there,
    /// [`crate::dispatchaway::AUDITED_FROM`]). So a later reader can tell such a grant from
    /// one made in Settings.
    pub fn dispatch_grant_from(
        &mut self,
        plane: &Path,
        number: Option<u32>,
        audited: &crate::dispatchgrant::Audited<'_>,
        from: &str,
    ) -> io::Result<Event> {
        self.dispatch_grant_said(plane, number, audited, Some(from))
    }

    fn dispatch_grant_said(
        &mut self,
        plane: &Path,
        number: Option<u32>,
        audited: &crate::dispatchgrant::Audited<'_>,
        from: Option<&str>,
    ) -> io::Result<Event> {
        self.audited(
            plane,
            number,
            audited.kind(),
            dispatch_grant_body(audited, from),
        )
    }

    /// Chat `number`'s current run has ended, in `state` for `cause`: each of its sub-agents
    /// still live ends with it, in the same state and for the same cause (ADR 0076 §6), with a
    /// `run.ended` under its child run. What the host calls at the run's exit or its own stop;
    /// a `/clear`, a `SessionEnd` and a new run begun for the chat end them here already. The
    /// events written, none when no child was live.
    ///
    /// Refused, with nothing ended, for a `state` that is not an end state.
    pub fn end_children_with(
        &mut self,
        plane: &Path,
        number: u32,
        state: RunState,
        cause: Cause,
    ) -> io::Result<Vec<Event>> {
        if !state.is_end() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("a run does not end {state:?}"),
            ));
        }
        let Some(chat) = self
            .chats
            .get(&(plane.to_path_buf(), number))
            .map(|who| who.chat.clone())
        else {
            return Ok(Vec::new());
        };
        self.end_children(&chat, state, cause)
    }

    /// Records each line of the kill switch's journal under `config` that the log does not
    /// hold yet (#1375, ADR 0075's "ADR 0071, amended"): `killswitch.stop`, `killswitch.rearm`
    /// or `killswitch.tamper`, with no chat and no run, a `window` line as a person's, an `app`
    /// line as the host's and a `cli` line as unattributed ([`kill_switch_body`]). What the host
    /// calls as it starts and each time its watch reads the journal; the events written, made
    /// durable before it answers.
    ///
    /// **A line read twice is one event**, in one launch or across them: lines are counted
    /// against the events the log already holds for the same line, and the log is read for
    /// them once, back only as far as the oldest line that could still be recorded. Two lines
    /// alike in every field are two events. A line older than the log keeps
    /// ([`Retention::keep`]) is not recorded: the log could not hold it.
    ///
    /// **The journal is not the authority on what happened** here, only on what the switch
    /// is: any process of the user can rewrite it. A line the host wrote itself is recorded
    /// from what it wrote ([`Recorder::kill_switch_wrote`]), whatever the journal says later.
    pub fn kill_switch_read(&mut self, config: &Path) -> io::Result<Vec<Event>> {
        let signature = journal_signature(config);
        if self
            .kill_switch
            .as_ref()
            .is_some_and(|seen| seen.read_as.is_some() && seen.read_as == signature)
        {
            return Ok(Vec::new());
        }
        // Not read back yet in this launch: a journal the log already held whole, as the last
        // read left it, is not read back for (KILL_SWITCH_READ).
        if self.kill_switch.is_none() && signature.is_some() {
            if self.kill_switch_skipped == signature {
                return Ok(Vec::new());
            }
            let held = std::fs::read_to_string(self.log.dir.join(KILL_SWITCH_READ)).ok();
            if held.is_some() && held == signature.and_then(kill_switch_signature_text) {
                self.kill_switch_skipped = signature;
                return Ok(Vec::new());
            }
        }
        let written = self.kill_switch_reconcile(config, None, None)?;
        if let Some(seen) = self.kill_switch.as_mut() {
            seen.read_as = signature;
        }
        Ok(written)
    }

    /// Records `entry`, a kill-switch journal line the host itself wrote, or tried to: the
    /// window's stop or re-arm, or the tamper the app put right. Recorded once whether or not
    /// the journal kept it, and whatever a process does to the journal after, so nothing a chat
    /// does to that file takes the event away; then the journal is read as
    /// [`Recorder::kill_switch_read`] reads it. The events written.
    pub fn kill_switch_wrote(
        &mut self,
        entry: crate::halt::Entry,
        config: &Path,
    ) -> io::Result<Vec<Event>> {
        self.kill_switch_reconcile(config, Some(entry), None)
    }

    /// [`Recorder::kill_switch_wrote`], the line's event naming `device`, the desktop whose link
    /// wrote it on a runner (#2051, ADR 0075 as amended: its device id in `meta`).
    pub fn kill_switch_wrote_by(
        &mut self,
        entry: crate::halt::Entry,
        device: Option<&str>,
        config: &Path,
    ) -> io::Result<Vec<Event>> {
        self.kill_switch_reconcile(config, Some(entry), device)
    }

    fn kill_switch_reconcile(
        &mut self,
        config: &Path,
        wrote: Option<crate::halt::Entry>,
        device: Option<&str>,
    ) -> io::Result<Vec<Event>> {
        let mut device = device.and_then(|device| wrote.map(|line| (line, device.to_owned())));
        let keep = self.log.retention.keep;
        let oldest = crate::halt::now().saturating_sub(keep.as_secs());
        // Taken before the journal is read: a journal changed after it reads as changed.
        let signature = journal_signature(config);
        let lines: Vec<crate::halt::Entry> = crate::halt::journal(config)
            .into_iter()
            .filter(|entry| entry.at >= oldest)
            .collect();
        if self.kill_switch.is_none() {
            let since = lines.iter().chain(wrote.iter()).map(|entry| entry.at).min();
            let logged = match since {
                Some(since) => kill_switch_logged(&self.log.dir, since)?,
                None => HashMap::new(),
            };
            self.kill_switch = Some(KillSwitchSeen {
                logged,
                wrote: HashMap::new(),
                read_as: None,
            });
        }
        let Some(seen) = self.kill_switch.as_mut() else {
            return Ok(Vec::new());
        };
        if let Some(entry) = wrote {
            *seen.wrote.entry(entry).or_default() += 1;
        }
        seen.forget_before(oldest);
        // In the journal's order, each line past as many alike as the log holds; then what the
        // host wrote that the journal no longer says.
        let mut owed = Vec::new();
        let mut counted: HashMap<crate::halt::Entry, usize> = HashMap::new();
        for entry in &lines {
            let nth = counted.entry(*entry).or_default();
            *nth += 1;
            if *nth > seen.logged.get(entry).copied().unwrap_or_default() + owed_of(&owed, entry) {
                owed.push(*entry);
            }
        }
        for (entry, times) in &seen.wrote {
            let held = seen.logged.get(entry).copied().unwrap_or_default() + owed_of(&owed, entry);
            for _ in held..*times {
                owed.push(*entry);
            }
        }
        let mut written = Vec::with_capacity(owed.len());
        for entry in owed {
            let mut body = kill_switch_body(&entry);
            // The line a link wrote names the desktop's device, once (#2051).
            if device.as_ref().is_some_and(|(line, _)| *line == entry)
                && let Some((_, by)) = device.take()
                && let Some(meta) = body.as_object_mut()
            {
                meta.insert("device".into(), serde_json::Value::String(by));
            }
            let event = self.write(None, None, None, kill_switch_kind(entry.event), body)?;
            if let Some(seen) = self.kill_switch.as_mut() {
                *seen.logged.entry(entry).or_default() += 1;
            }
            written.push(event);
        }
        if let Some(last) = written.last() {
            self.log.durable().through(last.seq)?;
        }
        // The log holds every line of the journal as it was read: a cache for the next start,
        // whose loss only costs a read.
        if let Some(text) = signature.and_then(kill_switch_signature_text) {
            // A torn write reads as different, which is a read.
            let _ = std::fs::write(self.log.dir.join(KILL_SWITCH_READ), text);
        }
        Ok(written)
    }

    /// What makes this recorder's events durable: [`Durable::through`] after each write.
    pub fn durable(&self) -> std::sync::Arc<Durable> {
        self.log.durable()
    }

    /// How long a tool call ran, once both of its ends have been heard, in either order.
    ///
    /// The first end heard is kept. The second gives the duration: by the hooks' own clocks
    /// when both said when they began (`at_ms`), else by when the host heard each. A second
    /// pre hook of one call (`pretooluse` and `pretooluse-read`) is not a later start, and an
    /// end left waiting longer than [`CALL_IS_OPEN_AT_MOST`] is let go: the other end, heard
    /// later than that, gives no duration.
    fn pair(
        &mut self,
        key: (String, String),
        phase: Phase,
        now: Instant,
        at_ms: u64,
    ) -> Option<u64> {
        self.calls
            .retain(|_, heard| now.saturating_duration_since(heard.at) <= CALL_IS_OPEN_AT_MOST);
        match self.calls.get(&key) {
            Some(first) if first.phase != phase => {
                let first = self.calls.remove(&key)?;
                let (pre, post) = match phase {
                    Phase::Post => (first.at_ms, at_ms),
                    Phase::Pre => (at_ms, first.at_ms),
                };
                if pre > 0 && post > 0 {
                    return Some(post.saturating_sub(pre));
                }
                let ran = now.saturating_duration_since(first.at).as_millis();
                Some(u64::try_from(ran).unwrap_or(u64::MAX))
            }
            Some(_) => None,
            None => {
                self.calls.insert(
                    key,
                    Heard {
                        phase,
                        at: now,
                        at_ms,
                    },
                );
                None
            }
        }
    }

    fn append(&mut self, under: &Under, kind: &str, body: serde_json::Value) -> io::Result<Event> {
        self.write(
            Some(&under.chat),
            Some(&under.run),
            under.parent.as_deref(),
            kind,
            body,
        )
    }

    /// **Every event this recorder writes goes through here**, so every event inside a skill run
    /// carries the skill (SK-3): `skill` and `skill_run` in its body, the innermost run open in
    /// the chat's run (a sub-agent's child run counts toward its parent's).
    fn write(
        &mut self,
        chat: Option<&str>,
        run: Option<&str>,
        parent: Option<&str>,
        kind: &str,
        mut body: serde_json::Value,
    ) -> io::Result<Event> {
        if kind == "run.started"
            && let Some(run) = run
        {
            if self.begun.len() >= RUNS_HELD {
                let newest = self
                    .begun
                    .iter()
                    .filter_map(|run| ulid::Ulid::from_string(run).ok())
                    .map(|id| id.timestamp_ms())
                    .max()
                    .unwrap_or_default();
                self.begun_until_ms = self.begun_until_ms.max(newest);
                self.begun.clear();
            }
            self.begun.insert(run.to_owned());
        }
        if let Some(open) = parent.or(run).and_then(|key| self.skills.inside(key))
            && let Some(fields) = body.as_object_mut()
        {
            fields.insert("skill".to_owned(), open.skill.as_str().into());
            fields.insert("skill_run".to_owned(), open.id.as_str().into());
        }
        self.log.append(chat, run, parent, kind, body)
    }

    /// **What a tool call does to the skill runs of its chat's run** (SK-3,
    /// [`crate::skillspan`]): one that starts a skill opens a run of it, said with a
    /// `skill.started` before the call's own event, which is inside it, and logged in the
    /// plane's skill log as it opens ([`crate::skilluse::started`]); every call is counted.
    fn skills_heard_of_a_call(
        &mut self,
        plane: &Path,
        under: &Under,
        who: &Who,
        call: &crate::hookwire::ToolCall,
    ) -> io::Result<()> {
        let chat = under.chat.clone();
        let Some(opened) = self
            .skills
            .call(span_of(under), call, now_ms(), || tokens_of(plane, &chat))
        else {
            return Ok(());
        };
        self.skill_started(plane, under, who, &opened)
    }

    /// Says that skill run `opened` began, with a `skill.started`, and logs it in the plane's
    /// skill log as it opens ([`crate::skilluse::started`]), with the chat's persona and harness
    /// (`who`). The `skill.started` says what started it and which copy of the skill it is of,
    /// where that is known: with the `skill` and `skill_run` every entry inside it carries, the
    /// attributes an exporter puts on the run's span (ADR 0083, amended 2026-10-23).
    fn skill_started(
        &mut self,
        plane: &Path,
        under: &Under,
        who: &Who,
        opened: &purlis_skills::runs::Open,
    ) -> io::Result<()> {
        let mut body = serde_json::json!({ "started_by": opened.started_by });
        if let Some(parent) = &opened.parent {
            body["parent_skill_run"] = parent.as_str().into();
        }
        if let Some(level) = opened.whose.level {
            body["skill_level"] = level.word().into();
        }
        self.append(under, "skill.started", body)?;
        let starting = crate::skilluse::Starting::of_open(opened, who.harness.as_deref());
        crate::skilluse::started(
            plane,
            &starting,
            who.persona.as_deref(),
            chrono::Utc::now(),
            &self.log.device,
        );
        Ok(())
    }

    /// What a state hook does to the skill runs of its chat's run (SK-3): the turn's end, a new
    /// prompt and the session's end close them, each before the hook's own event, which is
    /// outside them; a question put to the person is counted. A new prompt then opens the run
    /// of a skill Use skill… typed into it (SK-12), said before the prompt's own event, which is
    /// inside it.
    fn skills_heard_of_a_report(
        &mut self,
        plane: &Path,
        under: &Under,
        who: &Who,
        report: &crate::hookwire::Report,
    ) -> io::Result<()> {
        if under.parent.is_some() {
            return Ok(());
        }
        if let Some(prompted) = self.prompted.remove(&(plane.to_path_buf(), report.chat)) {
            self.skills.prompted(&under.run, prompted);
        }
        let chat = under.chat.clone();
        let reported = self
            .skills
            .report(&under.run, report, now_ms(), || tokens_of(plane, &chat));
        self.skills_closed(plane, under, reported.closed)?;
        for opened in &reported.opened {
            self.skill_started(plane, under, who, opened)?;
        }
        Ok(())
    }

    /// **Use skill… typed `typed` into chat `number` of `plane`**, unsent (SK-12): its run opens
    /// once a prompt shows it was sent ([`crate::skillspan::Tracker::typed`]). Nothing is
    /// written now; a chat this writer knows no run of holds nothing.
    pub fn skill_typed(&mut self, plane: &Path, number: u32, typed: crate::skillspan::Typed) {
        let Some(who) = self.chats.get(&(plane.to_path_buf(), number)) else {
            return;
        };
        let key = who.run.clone();
        self.skills.typed(&key, typed, now_ms());
    }

    /// **A curation action or a trigger put prompt `prompted` in chat `number` of `plane`**,
    /// unsent (SK-20): the runs the turn it is sent in opens are theirs
    /// ([`crate::skillspan::Tracker::prompted`]). Nothing is written now. It is held by the
    /// chat's number, as it may come before the chat's run begins, and handed to its run at the
    /// chat's next report.
    pub fn skill_prompted(
        &mut self,
        plane: &Path,
        number: u32,
        prompted: crate::skillspan::Prompted,
    ) {
        if self.prompted.len() >= PROMPTED_HELD {
            self.prompted.clear();
        }
        self.prompted
            .insert((plane.to_path_buf(), number), prompted);
    }

    /// Closes every skill run open in `under`'s chat run as `ended`.
    fn close_skills(
        &mut self,
        plane: &Path,
        under: &Under,
        ended: purlis_skills::runs::Ended,
    ) -> io::Result<()> {
        let closed = self.skills.close(span_of(under), ended, now_ms());
        self.skills_closed(plane, under, closed)
    }

    /// Says each of `closed` with a `skill.ended` that holds its signals, innermost first, and
    /// logs its end in the plane's skill log ([`crate::skilluse::ended`]).
    fn skills_closed(
        &mut self,
        plane: &Path,
        under: &Under,
        closed: Vec<purlis_skills::runs::Closed>,
    ) -> io::Result<()> {
        if closed.is_empty() {
            return Ok(());
        }
        let tokens = tokens_of(plane, &under.chat);
        for run in closed {
            let spent = tokens
                .zip(run.open.tokens_at_start)
                .map(|(now, then)| now.saturating_sub(then));
            let signals = run.open.signals;
            // The closed run's own, so it says that run's skill: none is open to tag it now.
            let mut body = serde_json::json!({
                "skill": run.open.skill,
                "skill_run": run.open.id,
                "ended": run.ended,
                "ms": run.ms,
                "tool_calls": signals.tool_calls,
                "tool_errors": signals.tool_errors,
                "refusals": signals.refusals,
                "asks": signals.asks,
            });
            if let Some(spent) = spent {
                body["tokens"] = spent.into();
            }
            self.append(under, "skill.ended", body)?;
            crate::skilluse::ended(plane, &run, spent, chrono::Utc::now(), &self.log.device);
        }
        Ok(())
    }

    /// The run an event of `top`'s chat is under: `top`'s, or the child run of `agent`, begun
    /// the first time that agent is seen.
    fn under(&mut self, top: Identity, agent: Option<&str>) -> io::Result<Under> {
        let Some(agent) = agent else {
            return Ok(Under {
                chat: top.chat,
                run: top.run,
                parent: None,
            });
        };
        let key = (top.chat.clone(), agent.to_owned());
        if self.stopped.contains(&key) {
            // A line from an agent whose stop was heard: its run is over, so the line is the
            // parent run's, as an unmeasured harness's sub-agent's would be.
            return Ok(Under {
                chat: top.chat,
                run: top.run,
                parent: None,
            });
        }
        if let Some((run, parent)) = self.children.get(&key) {
            return Ok(Under {
                chat: top.chat,
                run: run.clone(),
                parent: Some(parent.clone()),
            });
        }
        let under = Under {
            chat: top.chat,
            run: crate::reopen::mint(),
            parent: Some(top.run),
        };
        let body = top
            .attributes
            .of_a_child()
            .body(Began::Child, &self.log.device);
        self.append(&under, "run.started", body)?;
        self.children.insert(
            key,
            (under.run.clone(), under.parent.clone().unwrap_or_default()),
        );
        Ok(under)
    }

    /// Begins run `run` of chat `number`, under the chat id `chat` ([`RunOf`]), for `cause`, and says so
    /// in the log: what the host does when it starts a chat (ADR 0066, "A new run begins
    /// exactly when…").
    ///
    /// **The id is the host's, never minted here**: the app keeps it in `reopen.json`, so a
    /// chat put back after a relaunch is the chat it was, and its first event after the
    /// relaunch is this run's `run.started`. Every sub-agent of the run before ends with it.
    ///
    /// **Its `run.started` carries the run's `attributes`** (#868), each one the host does not
    /// have said as [`UNKNOWN`]; a clear and a child run of it carry them on.
    pub fn begin(
        &mut self,
        plane: &Path,
        number: u32,
        RunOf { chat, run }: RunOf<'_>,
        cause: Began,
        attributes: &Attributes,
    ) -> io::Result<Event> {
        self.begin_held(plane, number, RunOf { chat, run }, cause, attributes, None);
        self.begun(plane, number)?
            .ok_or_else(|| io::Error::other("the run was not held"))
    }

    /// [`Recorder::begin`], its `run.started` held until [`Recorder::begun`] says the run goes
    /// on: a start the host may still take back (#2250). A line of the run that comes first is
    /// written after it, so a run's first event is still its `run.started`; one never begun
    /// writes nothing. `trust` is its start's trust event, held with it and written just before
    /// it (#2252).
    pub fn begin_held(
        &mut self,
        plane: &Path,
        number: u32,
        RunOf { chat, run }: RunOf<'_>,
        cause: Began,
        attributes: &Attributes,
        trust: Option<HeldTrust>,
    ) {
        self.taken_back.remove(&(plane.to_path_buf(), number));
        self.chats.insert(
            (plane.to_path_buf(), number),
            Identity {
                chat: chat.to_owned(),
                run: run.to_owned(),
                attributes: attributes.clone(),
                held: Some(cause),
                held_trust: trust,
            },
        );
    }

    /// The `run.started` of chat `number`'s run held by [`Recorder::begin_held`], written now;
    /// `None` where none is held.
    pub fn begun(&mut self, plane: &Path, number: u32) -> io::Result<Option<Event>> {
        let Some(who) = self.chats.get_mut(&(plane.to_path_buf(), number)) else {
            return Ok(None);
        };
        // Taken before the line is written: a log that refuses the line must not leave the
        // chat's next hook line to mint it a second id, or to try the line again.
        let Some(cause) = who.held.take() else {
            return Ok(None);
        };
        let trust = who.held_trust.take();
        let who = who.clone();
        // Its start's trust event first, as at any start, made durable as the host makes it.
        // One the log refuses costs the record of the sandbox coming back on, never the run.
        if let Some(HeldTrust { kind, body }) = trust {
            let kept = self
                .write(Some(&who.chat), Some(&who.run), None, &kind, body)
                .and_then(|event| self.log.durable().through(event.seq));
            if let Err(why) = kept {
                tracing::warn!(error = %why, "the event log did not keep a held trust event");
            }
        }
        // A child still live is of the run this one takes the place of, which has ended
        // superseded (ADR 0076 §2): a run whose own end was told has none left.
        let ended = self.end_children(&who.chat, RunState::Completed, Cause::Superseded);
        ended?;
        let body = who.attributes.body(cause, &self.log.device);
        self.write(Some(&who.chat), Some(&who.run), None, "run.started", body)
            .map(Some)
    }

    /// Chat `number`'s start whose run is held ([`Recorder::begin_held`]) was put aside: its
    /// `run.started` and the trust event held with it are never written, and a later line from
    /// its program is recorded with no chat or run, under its number (`chat_number`), as a line
    /// of a chat the log does not know is (#2252); an audit naming the number is of no run. A
    /// run a line of it already said is kept: the log is append-only.
    pub fn taken_back(&mut self, plane: &Path, number: u32) {
        let key = (plane.to_path_buf(), number);
        if self.chats.get(&key).is_some_and(|who| who.held.is_some()) {
            self.chats.remove(&key);
            self.taken_back.insert(key);
        }
    }

    /// Whether chat `number`'s start was taken back ([`Recorder::taken_back`]).
    fn was_taken_back(&self, plane: &Path, number: u32) -> bool {
        self.taken_back.contains(&(plane.to_path_buf(), number))
    }

    /// A line of `kind` from chat `number`, under no chat or run.
    fn of_no_run(
        &mut self,
        number: u32,
        kind: &str,
        mut body: serde_json::Value,
    ) -> io::Result<Event> {
        body["chat_number"] = number.into();
        self.write(None, None, None, kind, body)
    }

    /// The chat's identity, giving it one and its first run when it has none. `None` for a
    /// number whose start was taken back ([`Recorder::taken_back`]): what it sends is of no run,
    /// and never begins one.
    fn identity(&mut self, plane: &Path, number: u32) -> io::Result<Option<Identity>> {
        if self.was_taken_back(plane, number) {
            return Ok(None);
        }
        self.begun(plane, number)?;
        match self.chats.get(&(plane.to_path_buf(), number)) {
            Some(who) => Ok(Some(who.clone())),
            None => self
                .new_run(plane, number, Began::Start, None, None)
                .map(Some),
        }
    }

    /// Ends every sub-agent of `chat` still live: its run ended in `state` for `cause`, and
    /// theirs with it, each with a `run.ended` (ADR 0076 §6): a child whose own `SubagentStop`
    /// was not heard before its parent ended.
    ///
    /// Every one is forgotten whether or not its line is written, and every line is tried:
    /// the first refusal is the answer.
    fn end_children(
        &mut self,
        chat: &str,
        state: RunState,
        cause: Cause,
    ) -> io::Result<Vec<Event>> {
        let mut live = Vec::new();
        self.children.retain(|(of, _), (run, parent)| {
            let keep = of != chat;
            if !keep {
                live.push((run.clone(), parent.clone()));
            }
            keep
        });
        self.stopped.retain(|(of, _)| of != chat);
        // In the order they began: a ULID's order is its time's.
        live.sort();
        let mut ended = Vec::new();
        let mut refused = None;
        for (run, parent) in live {
            match self.end_child(chat, run, parent, state, cause) {
                Ok(event) => ended.push(event),
                Err(why) => {
                    refused.get_or_insert(why);
                }
            }
        }
        refused.map_or(Ok(ended), Err)
    }

    /// The `run.ended` of child run `run` of `parent`: its end state and cause.
    fn end_child(
        &mut self,
        chat: &str,
        run: String,
        parent: String,
        state: RunState,
        cause: Cause,
    ) -> io::Result<Event> {
        let under = Under {
            chat: chat.to_owned(),
            run,
            parent: Some(parent),
        };
        self.append(
            &under,
            "run.ended",
            serde_json::json!({ "state": end_word(state), "cause": cause.word() }),
        )
    }

    /// Begins a run of the chat for `cause`, as `run` where the host minted it, and says so in
    /// the log.
    ///
    /// **It carries on the attributes of the run it follows** (#868): a clear changes the
    /// conversation, to the one the board followed it onto (`conversation`), and nothing else.
    /// A chat the host was never told of has none to carry on, and says each is not known.
    fn new_run(
        &mut self,
        plane: &Path,
        number: u32,
        cause: Began,
        run: Option<&str>,
        conversation: Option<&str>,
    ) -> io::Result<Identity> {
        let key = (plane.to_path_buf(), number);
        let (chat, before) = self.chats.get(&key).map_or_else(
            || (crate::reopen::mint(), Attributes::default()),
            |who| (who.chat.clone(), who.attributes.clone()),
        );
        let who = Identity {
            chat,
            run: run.map_or_else(crate::reopen::mint, str::to_owned),
            attributes: Attributes {
                conversation: conversation.map(str::to_owned),
                ..before
            },
            held: None,
            held_trust: None,
        };
        if cause == Began::Clear {
            // The cleared run is superseded, and its children and skill runs end with it,
            // before the run that takes its place begins.
            self.end_children(&who.chat, RunState::Completed, Cause::Superseded)?;
            if let Some(cleared) = self.chats.get(&key).map(|was| Under {
                chat: was.chat.clone(),
                run: was.run.clone(),
                parent: None,
            }) {
                self.close_skills(plane, &cleared, purlis_skills::runs::Ended::Session)?;
            }
        }
        let body = who.attributes.body(cause, &self.log.device);
        self.write(Some(&who.chat), Some(&who.run), None, "run.started", body)?;
        self.chats.insert(key, who.clone());
        Ok(who)
    }
}

/// The persona and harness of a chat's run, as a skill run's start logs them: each `None` where
/// the run says none or does not know.
struct Who {
    persona: Option<String>,
    harness: Option<String>,
}

impl Who {
    fn of(attributes: &Attributes) -> Who {
        let known = |v: &Option<String>| {
            v.clone()
                .filter(|v| !v.is_empty() && v != NONE && v != UNKNOWN)
        };
        Who {
            persona: known(&attributes.persona),
            harness: known(&attributes.harness),
        }
    }
}

/// The chat run an entry `under` counts toward for its skill runs: a sub-agent's child run
/// counts toward its parent's, where the skill started.
fn span_of(under: &Under) -> &str {
    under.parent.as_deref().unwrap_or(&under.run)
}

/// Now, in milliseconds since 1970: when a skill run opens and closes.
fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|since| u64::try_from(since.as_millis()).ok())
        .unwrap_or_default()
}

/// The tokens chat `chat` of `plane` has spent, as its harness last said them
/// ([`crate::usage::spent`]): what a skill run's tokens are counted from. `None` where it said
/// none.
fn tokens_of(plane: &Path, chat: &str) -> Option<u64> {
    let spent = crate::usage::spent(plane, chat)?;
    (spent.input_tokens.is_some() || spent.output_tokens.is_some()).then(|| {
        spent
            .input_tokens
            .unwrap_or_default()
            .saturating_add(spent.output_tokens.unwrap_or_default())
    })
}

/// A dispatch grant's audited body, saying `from` where the person made it somewhere other
/// than a chat's Notice or Settings ([`Recorder::dispatch_grant_from`]): what the window puts in
/// an [`Entry::Audited`] for one too.
pub fn dispatch_grant_body(
    audited: &crate::dispatchgrant::Audited<'_>,
    from: Option<&str>,
) -> serde_json::Value {
    let mut body = audited.body();
    if let (Some(from), Some(fields)) = (from, body.as_object_mut()) {
        fields.insert("from".to_owned(), from.into());
    }
    body
}

/// An end state's word, as `run.ended`'s body says it (ADR 0076 §1). Every caller passes an
/// end state; any other would read as no end at all.
fn end_word(state: RunState) -> &'static str {
    match state {
        RunState::Completed => "completed",
        RunState::Failed => "failed",
        RunState::Stopped => "stopped",
        _ => "not-ended",
    }
}

/// What a `SessionStart` said it was for, as its body says it.
fn started(how: crate::state::Started) -> &'static str {
    use crate::state::Started;
    match how {
        Started::Freshly => "fresh",
        Started::Cleared => "cleared",
        Started::Compacted => "compacted",
        Started::Unsaid => "unsaid",
    }
}

#[cfg(test)]
#[path = "eventlog/tests.rs"]
mod tests;
