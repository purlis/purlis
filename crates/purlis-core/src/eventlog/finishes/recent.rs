// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The log's files, each indexed once by chat and kept while it does not change** (felt speed
//! round 5, H15; FS-C2, #7902). The window asks for finishes as each chat's turn ends, and each
//! ask used to read and parse the whole log (30 days) for it: with about fifty chats that was a
//! storm of parses of 100 MB, and the commands beside it waited on the cores it took.
//!
//! **What is kept of a file** is its chats' records ([`Record`]), each chat's in the order
//! written: what the fold reads of each event, and never its body. Every chat of the file is
//! indexed by the one parse, so an ask of another chat reads nothing again.
//!
//! **Which files an ask reads**: the live file, which it reads first (as [`super::super::read_back`]
//! does, so a file sealed meanwhile is listed after, never missed), and every sealed file written
//! to since `since`. Then, for each asked chat, the older ones until its first turn and its run's
//! start are in what was read ([`needs_older`]), so a turn that began before the window keeps
//! its early blocks and a block its harness, and the answer is the whole log's for every turn the
//! window reaches. A chat that began no turn the window reaches reads nothing older.
//!
//! **Each file has a lock of its own**, which an ask holds only while it brings that file up to
//! date and takes its records: asks of the same file share its one parse, and wait for it, as
//! asks of a [`crate::burst`] do; an ask of other files never waits for it. The map of files is
//! locked only to find a file's entry, never across a read.
//!
//! **The stat stays under the file's lock** (an invariant): a file's stamp is read only while its
//! lock is held, so an ask that waited for another's parse stats the file as it is once that
//! parse is done, and no answer is made from a file as it was before its ask. A file is parsed
//! again only when it is not what it was kept at. One that only grew (the same file, no shorter
//! than what was parsed, and its last whole line parsed still there as it was) has just what it
//! gained parsed, from the end of that line; one that shrank, or is another file (its length,
//! modified time and identity), is parsed whole. The stamp is taken from the file as opened, so a
//! seal between the stat and the read cannot pair the new file with the old one's offset.
//!
//! **A last line with no end is never kept** (an invariant): what is kept, and the offset, end at
//! the last whole line. A last line that is an event with no end yet is in the answer of the ask
//! that read it, as [`crate::eventlog::read`] reads it, and read again by the next ask: a write
//! torn after its event and before its end is cut back by the log ([`crate::eventlog::Log`]) and
//! written over, and what was written over is never in an answer after.
//!
//! **What is let go**: a file gone from the log (pruned, or the live file not yet written); the
//! records of the chats of a project closed ([`Recent::forget`]); and, past a cap on the bytes
//! kept ([`Recent::CAP`]), the files asked least recently. A file outside an ask's window is
//! kept: two projects whose chats need windows of different depths no longer drop each other's
//! files at every ask.
//!
//! In memory and in this process only: nothing is written, so there is no store to give a tier
//! (ADR 0069).

use std::collections::{HashMap, HashSet};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, TryLockError};

use super::{Finish, Record, fold, needs_older};
use crate::eventlog::Event;

/// The files kept, each by its path.
#[derive(Debug)]
pub struct Recent {
    files: Mutex<HashMap<PathBuf, Slot>>,
    /// The most bytes of records kept, about ([`Record::bytes`]).
    cap: usize,
    /// Counts asks of files, for which was asked least recently.
    ticks: AtomicU64,
    #[cfg(test)]
    seen: tests::Seen,
}

impl Default for Recent {
    fn default() -> Self {
        Self::with_cap(Self::CAP)
    }
}

/// One file's place in the map: its entry, and what the cap counts of it.
#[derive(Debug)]
struct Slot {
    entry: Arc<Mutex<Entry>>,
    bytes: usize,
    /// The tick it was last asked at.
    used: u64,
}

/// One file, as it was last read: nothing until it is first.
#[derive(Debug, Default)]
struct Entry {
    file: Option<Indexed>,
}

#[derive(Debug)]
struct Indexed {
    stamp: Stamp,
    /// How far the file is parsed: the end of its last whole line.
    offset: u64,
    /// Where that line starts and what it was, so a file cut back and written again past it is
    /// told from one that grew; none where no whole line is parsed.
    last_line: Option<(u64, u64)>,
    /// Its newest event's number, of any chat: what the live file's repeats of a seal are told by.
    last_seq: Option<u64>,
    /// Each chat's records, in the order written.
    chats: HashMap<String, Arc<Vec<Record>>>,
    /// The chats whose records were let go of ([`Recent::forget`]): an ask of one parses the file
    /// whole again, and what the file gains of one is not kept.
    forgotten: HashSet<String>,
    bytes: usize,
}

/// What an ask took of one file: its newest event's number, and each asked chat's records, by
/// the chat's place among those asked.
#[derive(Debug)]
struct Taken {
    last_seq: Option<u64>,
    chats: Vec<Option<Arc<Vec<Record>>>>,
}

/// How one file stood when an ask came to it.
enum At {
    /// Not there.
    Gone,
    /// Last written before the window, and not read.
    Older,
    Taken(Taken),
}

/// What a file was when it was read: what tells it is not the same file, nor the same length.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Stamp {
    len: u64,
    modified: Option<std::time::SystemTime>,
    /// Which file it is where the platform says (its inode), so one put in its place is seen.
    id: Option<u64>,
}

impl Stamp {
    fn of(meta: &std::fs::Metadata) -> Self {
        Self {
            len: meta.len(),
            modified: meta.modified().ok(),
            id: file_id(meta),
        }
    }
}

#[cfg(unix)]
fn file_id(meta: &std::fs::Metadata) -> Option<u64> {
    use std::os::unix::fs::MetadataExt;
    Some(meta.ino())
}

#[cfg(not(unix))]
fn file_id(_: &std::fs::Metadata) -> Option<u64> {
    None
}

/// `path` opened and its stamp, taken from the file opened; or none where it is not there (the
/// live file before its first event, or a segment pruned since). Any other failure is the ask's,
/// as it is [`crate::eventlog::read`]'s.
fn opened(path: &Path) -> io::Result<Option<(std::fs::File, Stamp)>> {
    match std::fs::File::open(path) {
        Ok(file) => {
            let stamp = Stamp::of(&file.metadata()?);
            Ok(Some((file, stamp)))
        }
        Err(why) if why.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(why) => Err(why),
    }
}

/// What a line was, to tell it from another.
fn line_hash(line: &[u8]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    line.hash(&mut hasher);
    hasher.finish()
}

fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Recent {
    /// The most bytes of records kept by default: about a month of fifty busy chats' log.
    pub const CAP: usize = 64 << 20;

    /// Keeping at most about `cap` bytes of records.
    pub fn with_cap(cap: usize) -> Self {
        Self {
            files: Mutex::default(),
            cap,
            ticks: AtomicU64::new(0),
            #[cfg(test)]
            seen: tests::Seen::default(),
        }
    }

    /// **The finishes of each of `chats`** in the log in `dir`, oldest first, as
    /// [`super::finishes_in`] answers them from the whole log, for every turn that ends at or
    /// after `since` seconds since 1970 (and others the files read happen to hold).
    pub fn finishes(
        &self,
        dir: &Path,
        chats: &[&str],
        since: u64,
    ) -> io::Result<HashMap<String, Vec<Finish>>> {
        // The live file first: a seal after it is read leaves its events in a segment listed below
        // (and dropped as the repeat they are), where one before would leave them in neither.
        let live_path = dir.join(crate::eventlog::FILE);
        let live = match self.at(&live_path, chats, None)? {
            At::Taken(taken) => Some(taken),
            At::Gone | At::Older => None,
        };
        // Newest first.
        let paths: Vec<PathBuf> = crate::eventlog::segments(dir)?
            .into_iter()
            .rev()
            .map(|(_, path)| path)
            .collect();
        self.let_go_of_gone(dir, &live_path, &paths);
        let since = std::time::UNIX_EPOCH + std::time::Duration::from_secs(since);
        let mut sealed = Vec::new();
        let mut rest = paths.iter().peekable();
        // The window: every file written to since `since`, until the first one older.
        while let Some(path) = rest.peek() {
            match self.at(path, chats, Some(since))? {
                At::Taken(taken) => sealed.push(taken),
                At::Gone => {}
                At::Older => break,
            }
            rest.next();
        }
        // Then older ones, while an asked chat's turn or run began before what was read.
        for path in rest {
            let more =
                (0..chats.len()).any(|chat| needs_older(records_of(live.as_ref(), &sealed, chat)));
            if !more {
                break;
            }
            if let At::Taken(taken) = self.at(path, chats, None)? {
                sealed.push(taken);
            }
        }
        Ok(chats
            .iter()
            .enumerate()
            .map(|(at, chat)| {
                (
                    (*chat).to_owned(),
                    fold(records_of(live.as_ref(), &sealed, at)),
                )
            })
            .collect())
    }

    /// **Lets go of the records of `chats`** in every file kept: a project closed, whose chats
    /// are asked of no more. An ask of one of them after reads each file that held its records
    /// whole again.
    ///
    /// It takes each file's lock in turn, so it waits for any parse in progress: call it off the
    /// window's thread.
    pub fn forget(&self, chats: &[&str]) {
        let entries: Vec<(PathBuf, Arc<Mutex<Entry>>)> = locked(&self.files)
            .iter()
            .map(|(path, slot)| (path.clone(), Arc::clone(&slot.entry)))
            .collect();
        for (path, entry) in entries {
            let mut held = locked(&entry);
            let Some(file) = held.file.as_mut() else {
                continue;
            };
            for chat in chats {
                if let Some(records) = file.chats.remove(*chat) {
                    file.bytes -= chat.len() + records.iter().map(Record::bytes).sum::<usize>();
                    file.forgotten.insert((*chat).to_owned());
                }
            }
            // Under the file's lock, so a later ask's count is never overwritten by this one.
            if let Some(slot) = locked(&self.files).get_mut(&path)
                && Arc::ptr_eq(&slot.entry, &entry)
            {
                slot.bytes = file.bytes;
            }
            drop(held);
        }
    }

    /// The entry of `path`, made where there is none (as asked now, so the cap does not take it
    /// first): the map's lock is held for this alone.
    fn entry(&self, path: &Path) -> Arc<Mutex<Entry>> {
        let tick = self.ticks.fetch_add(1, Ordering::Relaxed);
        let mut files = locked(&self.files);
        let slot = files.entry(path.to_path_buf()).or_insert_with(|| Slot {
            entry: Arc::default(),
            bytes: 0,
            used: tick,
        });
        Arc::clone(&slot.entry)
    }

    /// **`path` brought up to date and its records of `chats` taken**, under its own lock; or,
    /// with `since`, nothing read where it was last written before.
    fn at(
        &self,
        path: &Path,
        chats: &[&str],
        since: Option<std::time::SystemTime>,
    ) -> io::Result<At> {
        let entry = self.entry(path);
        #[cfg(test)]
        self.seen.waiting_for(path);
        let mut held = locked(&entry);
        // Under the file's lock: see the module's invariant.
        let Some((opened, stamp)) = opened(path)? else {
            drop(held);
            self.let_go_of(path, &entry);
            return Ok(At::Gone);
        };
        if since.is_some_and(|since| stamp.modified.is_some_and(|written| written < since)) {
            return Ok(At::Older);
        }
        let (file, tail) = self.brought_up_to_date(&mut held, opened, path, &stamp, chats)?;
        let mut taken = Taken {
            last_seq: file.last_seq,
            chats: chats
                .iter()
                .map(|chat| file.chats.get(*chat).map(Arc::clone))
                .collect(),
        };
        // A last line with no end, in this answer only.
        if let Some(event) = tail {
            taken.last_seq = Some(event.seq);
            let asked = event
                .chat
                .as_deref()
                .and_then(|chat| chats.iter().position(|one| *one == chat));
            if let (Some(at), Some(record)) = (asked, Record::of(&event)) {
                let mut records = taken.chats[at].as_deref().cloned().unwrap_or_default();
                records.push(record);
                taken.chats[at] = Some(Arc::new(records));
            }
        }
        // Under the file's lock, so a later ask's count is never overwritten by this one.
        self.used(path, &entry, file.bytes);
        drop(held);
        Ok(At::Taken(taken))
    }

    /// Makes what is kept of `path` (opened as `opened`) what `stamp` says it is now, reading as
    /// little as that takes; and answers its last line where that has no end and is an event.
    fn brought_up_to_date<'a>(
        &self,
        held: &'a mut Entry,
        mut opened: std::fs::File,
        path: &Path,
        stamp: &Stamp,
        chats: &[&str],
    ) -> io::Result<(&'a Indexed, Option<Event>)> {
        let from = match &held.file {
            // One of the chats asked was let go of here: the file is read whole for it.
            Some(one) if chats.iter().any(|chat| one.forgotten.contains(*chat)) => None,
            // As it was kept, to its end: nothing to read.
            Some(one) if one.stamp == *stamp && one.offset == stamp.len => {
                return Ok((held.file.as_ref().expect("the file is kept"), None));
            }
            // The same file, no shorter, with its last whole line parsed as it was: what follows.
            Some(one)
                if stamp.id.is_some()
                    && one.stamp.id == stamp.id
                    && one.offset <= stamp.len
                    && still_there(&mut opened, one.last_line, one.offset)? =>
            {
                Some(one.offset)
            }
            _ => None,
        };
        #[cfg(test)]
        self.seen.parsing(path, from.is_none());
        // Taken out while it is read: nothing half read is kept.
        let mut file = match (from, held.file.take()) {
            (Some(_), Some(kept)) => kept,
            _ => Indexed {
                stamp: *stamp,
                offset: 0,
                last_line: None,
                last_seq: None,
                chats: HashMap::new(),
                forgotten: HashSet::new(),
                bytes: 0,
            },
        };
        let mut lines = 0;
        let read = read_from(&mut opened, from.unwrap_or(0), |event| {
            lines += 1;
            file.last_seq = Some(event.seq);
            let Some(chat) = event.chat.as_deref() else {
                return;
            };
            if file.forgotten.contains(chat) {
                return;
            }
            let Some(record) = Record::of(&event) else {
                return;
            };
            file.bytes += record.bytes();
            let records = file.chats.entry(chat.to_owned()).or_insert_with(|| {
                file.bytes += chat.len();
                Arc::default()
            });
            Arc::make_mut(records).push(record);
        })?;
        #[cfg(test)]
        self.seen.parsed_lines(lines);
        #[cfg(not(test))]
        let _ = (lines, path);
        file.stamp = *stamp;
        file.offset = read.offset;
        if read.last_line.is_some() {
            file.last_line = read.last_line;
        }
        Ok((held.file.insert(file), read.tail))
    }

    /// `entry` of `path` was just asked and keeps `bytes`: past the cap, the files asked least
    /// recently are let go of until it holds.
    fn used(&self, path: &Path, entry: &Arc<Mutex<Entry>>, bytes: usize) {
        let tick = self.ticks.fetch_add(1, Ordering::Relaxed);
        let mut files = locked(&self.files);
        // One let go of meanwhile is no longer counted.
        if let Some(slot) = files.get_mut(path)
            && Arc::ptr_eq(&slot.entry, entry)
        {
            slot.bytes = bytes;
            slot.used = tick;
        }
        let mut kept: usize = files.values().map(|slot| slot.bytes).sum();
        while kept > self.cap {
            // Never one that frees nothing, nor one being read: an ask waiting for its parse
            // would be orphaned, and the next ask of it would parse it again.
            let Some(oldest) = files
                .iter()
                .filter(|(_, slot)| {
                    slot.bytes > 0
                        && !matches!(slot.entry.try_lock(), Err(TryLockError::WouldBlock))
                })
                .min_by_key(|(_, slot)| slot.used)
                .map(|(path, _)| path.clone())
            else {
                break;
            };
            if let Some(slot) = files.remove(&oldest) {
                kept -= slot.bytes;
            }
        }
    }

    /// Lets go of `path`, unless another entry took its place meanwhile.
    fn let_go_of(&self, path: &Path, entry: &Arc<Mutex<Entry>>) {
        let mut files = locked(&self.files);
        if files
            .get(path)
            .is_some_and(|slot| Arc::ptr_eq(&slot.entry, entry))
        {
            files.remove(path);
        }
    }

    /// Lets go of every file of the log in `dir` that is neither its live file nor in `sealed`,
    /// nor there now: one sealed after `sealed` was listed is another ask's to keep.
    fn let_go_of_gone(&self, dir: &Path, live: &Path, sealed: &[PathBuf]) {
        let unlisted: Vec<(PathBuf, Arc<Mutex<Entry>>)> = locked(&self.files)
            .iter()
            .filter(|(path, _)| {
                path.parent() == Some(dir)
                    && path.as_path() != live
                    && !sealed.iter().any(|one| one == *path)
            })
            .map(|(path, slot)| (path.clone(), Arc::clone(&slot.entry)))
            .collect();
        // Outside the map's lock: it is never held across a read of the disk.
        for (path, entry) in unlisted {
            if !path.exists() {
                self.let_go_of(&path, &entry);
            }
        }
    }
}

/// The records of the chat at `at` among those asked, oldest first: the sealed files taken
/// (newest first) and the live file. The live file's records repeat what was sealed meanwhile
/// (the live file was read before it was sealed), so only one newer than the newest sealed event
/// counts.
fn records_of<'a>(
    live: Option<&'a Taken>,
    sealed: &'a [Taken],
    at: usize,
) -> impl Iterator<Item = &'a Record> {
    let newest = sealed.iter().filter_map(|taken| taken.last_seq).max();
    let of = move |taken: &'a Taken| taken.chats[at].iter().flat_map(|records| records.iter());
    sealed.iter().rev().flat_map(of).chain(
        live.into_iter()
            .flat_map(of)
            .filter(move |record| newest.is_none_or(|seq| record.seq() > seq)),
    )
}

/// Whether the line that `last_line` says ended at `offset` is still there as it was: a file
/// cut back and written again over it is not the file that grew. Nothing to check before the
/// first whole line.
fn still_there(
    file: &mut std::fs::File,
    last_line: Option<(u64, u64)>,
    offset: u64,
) -> io::Result<bool> {
    use std::io::{Read, Seek, SeekFrom};
    let Some((start, hash)) = last_line else {
        return Ok(offset == 0);
    };
    let mut line = vec![0; usize::try_from(offset - start).map_err(io::Error::other)?];
    file.seek(SeekFrom::Start(start))?;
    match file.read_exact(&mut line) {
        Ok(()) => Ok(line.last() == Some(&b'\n') && line_hash(&line) == hash),
        Err(why) if why.kind() == io::ErrorKind::UnexpectedEof => Ok(false),
        Err(why) => Err(why),
    }
}

/// What [`read_from`] read.
struct Read {
    /// The end of the last whole line.
    offset: u64,
    /// Where that line starts and its hash, where a whole line was read.
    last_line: Option<(u64, u64)>,
    /// The last line, where it has no end and is an event.
    tail: Option<Event>,
}

/// Hands `each` the events in `file` from byte `from` to its last whole line, a line that is not
/// one skipped, as [`crate::eventlog::read`] reads a file. A last line with no end is not handed
/// on, nor read past: where it is an event it is answered as the tail, for one answer only.
fn read_from(file: &mut std::fs::File, from: u64, mut each: impl FnMut(Event)) -> io::Result<Read> {
    use std::io::{Read as _, Seek, SeekFrom};
    file.seek(SeekFrom::Start(from))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    let mut done = 0;
    let mut last_line = None;
    let mut rest = bytes.as_slice();
    while let Some(end) = rest.iter().position(|byte| *byte == b'\n') {
        if let Ok(event) = serde_json::from_slice::<Event>(&rest[..end]) {
            each(event);
        }
        last_line = Some((from + done as u64, line_hash(&rest[..=end])));
        done += end + 1;
        rest = &rest[end + 1..];
    }
    Ok(Read {
        offset: from + done as u64,
        last_line,
        tail: serde_json::from_slice::<Event>(rest).ok(),
    })
}

#[cfg(test)]
pub(super) mod tests {
    //! What the tests in `finishes/tests/recent.rs` are told of an ask's reads.

    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Mutex, mpsc};

    use super::{Recent, locked};

    /// A parse held: of this file, telling the test it began, until it is let go.
    type Held = (PathBuf, mpsc::Sender<()>, mpsc::Receiver<()>);

    /// What the asks of a [`Recent`] read, and a parse a test holds.
    #[derive(Debug, Default)]
    pub(in crate::eventlog::finishes) struct Seen {
        /// Files parsed whole, a path each time.
        parsed: Mutex<Vec<PathBuf>>,
        /// Lines parsed in all.
        lines: AtomicUsize,
        /// Times an ask came to a file's lock, by the file.
        waits: Mutex<Vec<PathBuf>>,
        /// A parse to hold.
        hold: Mutex<Option<Held>>,
    }

    impl Seen {
        pub(super) fn waiting_for(&self, path: &Path) {
            locked(&self.waits).push(path.to_path_buf());
        }

        pub(super) fn parsing(&self, path: &Path, whole: bool) {
            if whole {
                locked(&self.parsed).push(path.to_path_buf());
            }
            let mut hold = locked(&self.hold);
            if hold.as_ref().is_some_and(|(held, ..)| held == path) {
                let (_, began, release) = hold.take().expect("held");
                drop(hold);
                began.send(()).expect("the test hears the parse began");
                release.recv().expect("the test lets the parse go");
            }
        }

        pub(super) fn parsed_lines(&self, lines: usize) {
            self.lines.fetch_add(lines, Ordering::SeqCst);
        }
    }

    impl Recent {
        /// Files parsed whole.
        pub(in crate::eventlog::finishes) fn parsed(&self) -> usize {
            locked(&self.seen.parsed).len()
        }

        /// Times `path` was parsed whole.
        pub(in crate::eventlog::finishes) fn parsed_of(&self, path: &Path) -> usize {
            locked(&self.seen.parsed)
                .iter()
                .filter(|one| *one == path)
                .count()
        }

        /// Lines parsed in all.
        pub(in crate::eventlog::finishes) fn lines(&self) -> usize {
            self.seen.lines.load(Ordering::SeqCst)
        }

        /// How many times an ask came to the lock of `path`.
        pub(in crate::eventlog::finishes) fn waits_for(&self, path: &Path) -> usize {
            locked(&self.seen.waits)
                .iter()
                .filter(|one| *one == path)
                .count()
        }

        /// Holds the next parse of `path` from the moment it begins, which `began` hears, until
        /// `release` is sent to.
        pub(in crate::eventlog::finishes) fn hold_parse_of(
            &self,
            path: &Path,
        ) -> (mpsc::Receiver<()>, mpsc::Sender<()>) {
            let (began, heard) = mpsc::channel();
            let (release, released) = mpsc::channel();
            *locked(&self.seen.hold) = Some((path.to_path_buf(), began, released));
            (heard, release)
        }

        /// The files kept.
        pub(in crate::eventlog::finishes) fn kept(&self) -> Vec<PathBuf> {
            locked(&self.files).keys().cloned().collect()
        }

        /// The bytes of records kept, as the cap counts them.
        pub(in crate::eventlog::finishes) fn kept_bytes(&self) -> usize {
            locked(&self.files).values().map(|slot| slot.bytes).sum()
        }
    }
}
