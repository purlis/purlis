// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The index's one writer (ADR 0079 §3, §4, §5).
//!
//! [`Writer::open`] takes `writer.lock`, then either opens the live generation, if it is one this
//! build wrote, and brings it up to date by a stat sweep, or builds a new generation from every
//! source and swaps it in. After that, [`Writer::sweep`] re-reads only the sources a set of paths
//! touches, and in each, only the files whose size or modification time moved: a file whose
//! content hash is unchanged keeps its row.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, params};

use super::sources::{self, Kind, Source};
use super::{CURRENT, DB, LOCK, Result, SCHEMA, words};

// [DEBUG-5672] phase timers, temporary.
pub mod dbg5672 {
    use std::sync::atomic::{AtomicU64, Ordering};
    pub static EVERY: AtomicU64 = AtomicU64::new(0);
    pub static RECORDED: AtomicU64 = AtomicU64::new(0);
    pub static LIST: AtomicU64 = AtomicU64::new(0);
    pub static CHANGE_WALL: AtomicU64 = AtomicU64::new(0);
    pub static STAT: AtomicU64 = AtomicU64::new(0);
    pub static READ: AtomicU64 = AtomicU64::new(0);
    pub static SCAN: AtomicU64 = AtomicU64::new(0);
    pub static APPLY: AtomicU64 = AtomicU64::new(0);
    pub static WAIT: AtomicU64 = AtomicU64::new(0);
    pub static COMMIT: AtomicU64 = AtomicU64::new(0);
    pub static CLOSE: AtomicU64 = AtomicU64::new(0);
    pub static WS: AtomicU64 = AtomicU64::new(0);
    pub static WHOLE: AtomicU64 = AtomicU64::new(0);
    pub static LINES: AtomicU64 = AtomicU64::new(0);
    pub fn add(at: &AtomicU64, since: std::time::Instant) {
        at.fetch_add(since.elapsed().as_nanos() as u64, Ordering::Relaxed);
    }
    pub fn threads() -> usize {
        std::env::var("PURLIS_DEBUG_5672_THREADS")
            .ok()
            .and_then(|t| t.parse().ok())
            .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get))
    }
    pub fn report() -> String {
        let ms = |a: &AtomicU64| a.load(Ordering::Relaxed) as f64 / 1e6;
        format!(
            "[DEBUG-5672] threads={} every={:.0} recorded={:.0} list_wall={:.0} change_wall={:.0} stat_sum={:.0} read_sum={:.0} scan_sum={:.0} apply={:.0} wait_plan={:.0} ws_rows={:.0} commit={:.0} close={:.0} scan_whole={:.0} scan_lines={:.0}",
            threads(), ms(&EVERY), ms(&RECORDED), ms(&LIST), ms(&CHANGE_WALL), ms(&STAT), ms(&READ), ms(&SCAN), ms(&APPLY), ms(&WAIT), ms(&WS), ms(&COMMIT), ms(&CLOSE), ms(&WHOLE), ms(&LINES)
        )
    }
}

/// The tables of one generation.
const TABLES: &str = "
CREATE TABLE meta(key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE item(
    id INTEGER PRIMARY KEY,
    path TEXT NOT NULL UNIQUE,
    base TEXT NOT NULL,
    kind TEXT NOT NULL,
    owner TEXT NOT NULL,
    audience TEXT NOT NULL,
    rank INTEGER NOT NULL,
    basekey TEXT NOT NULL,
    pathkey TEXT NOT NULL,
    title TEXT NOT NULL,
    date TEXT,
    size INTEGER NOT NULL,
    mtime INTEGER NOT NULL,
    hash TEXT NOT NULL
);
CREATE INDEX item_base ON item(base);
CREATE INDEX item_owner ON item(kind, owner, date);
CREATE INDEX item_owner_mtime ON item(kind, owner, mtime);
CREATE INDEX item_newest ON item(date DESC, rank, basekey, pathkey);
CREATE TABLE unread(base TEXT NOT NULL, kind TEXT NOT NULL, owner TEXT NOT NULL, path TEXT NOT NULL, code INTEGER);
CREATE INDEX unread_base ON unread(base);
CREATE TABLE workspace(name TEXT PRIMARY KEY, vision TEXT NOT NULL, mtime REAL);
CREATE VIRTUAL TABLE words USING fts5(
    title, text, reach,
    content = '', contentless_delete = 1, detail = full,
    tokenize = \"ascii tokenchars '_'\", prefix = '2 3'
);
INSERT INTO words(words, rank) VALUES ('rank', 'bm25(3.0, 1.0, 0.0)');
";

/// Every row of one generation is visible to the project's audience: this clone's own files, and
/// the project's committed ones. V4's audience settings (KN-28) are not built yet, and nothing
/// written for "this machine" alone is ever a source (ADR 0079 §1).
const AUDIENCE: &str = "project";

/// The index's writer: it holds `writer.lock` for as long as it lives.
pub struct Writer {
    root: PathBuf,
    home: PathBuf,
    conn: Connection,
    _lock: crate::filelock::Held,
}

/// What a sweep changed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Swept {
    /// Rows written, new or changed.
    pub written: usize,
    /// Rows deleted.
    pub deleted: usize,
}

impl Writer {
    /// The writer of the index of the project at `root`, with the index up to date: the live
    /// generation swept, or, when there is none this build can read, a new one built and swapped
    /// in. Waits for another writer to let go of the lock.
    pub fn open(root: &Path) -> Result<Self> {
        let home = super::home(root);
        private_dir(&home)?;
        let lock = lock(&home)?;
        remove_old_generations(&home);
        let mut writer = match super::current_db(&home).and_then(|db| valid(&db)) {
            Some(conn) => Self {
                root: root.to_path_buf(),
                home,
                conn,
                _lock: lock,
            },
            None => {
                let conn = build(root, &home)?;
                return Ok(Self {
                    root: root.to_path_buf(),
                    home,
                    conn,
                    _lock: lock,
                });
            }
        };
        writer.sweep(None)?;
        Ok(writer)
    }

    /// Says to readers that work is queued (`true`) or done (`false`). Done also records when.
    pub fn refreshing(&self, queued: bool) -> Result<()> {
        set(&self.conn, "pending", if queued { "1" } else { "0" })?;
        if !queued {
            set(&self.conn, "as_of", &super::now().to_string())?;
        }
        Ok(())
    }

    /// Brings the index up to date with the files: every source, with `None`, or only those
    /// `touched` names (a path in a source, a source itself, or a folder holding one). Sources
    /// that have gone lose their rows either way. One transaction: a reader sees it all or none.
    pub fn sweep(&mut self, touched: Option<&[PathBuf]>) -> Result<Swept> {
        let tx = self.conn.transaction()?;
        let swept = sweep(&self.root, &tx, touched)?;
        tx.commit()?;
        if swept.deleted > 0 {
            set(&self.conn, "deleted", "1")?;
        }
        Ok(swept)
    }

    /// The commit the index last reconciled against, if it recorded one.
    pub fn commit(&self) -> Option<String> {
        get(&self.conn, "commit")
    }

    /// Records the commit the index now reflects.
    pub fn reconciled(&self, commit: &str) -> Result<()> {
        set(&self.conn, "commit", commit)
    }

    /// Drops the terms of deleted rows from the database file (ADR 0079 §6): FTS5's `optimize`,
    /// then a checkpoint that truncates the WAL, with `secure_delete` on so freed pages are
    /// zeroed. Only when a deletion has happened since the last time.
    pub fn compact(&self) -> Result<()> {
        if get(&self.conn, "deleted").as_deref() != Some("1") {
            return Ok(());
        }
        self.conn
            .execute("INSERT INTO words(words) VALUES ('optimize')", [])?;
        set(&self.conn, "deleted", "0")?;
        self.conn
            .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))?;
        Ok(())
    }

    /// Records what of the project's memory its git standing has not committed, for the
    /// briefing's unshared-memory line (#5503): the hook reads it here, and runs no `git status`
    /// of its own. Each changed file under `personas/` or `workspaces/` with a `memory/` or
    /// `refs/` folder in its path counts, as a persona's or a workspace's.
    pub fn record_standing(&self, standing: &crate::planegit::Standing) -> Result<()> {
        let memory = |path: &&String| path.contains("/memory/") || path.contains("/refs/");
        let personas = standing
            .changed
            .iter()
            .filter(|p| p.starts_with("personas/"))
            .filter(memory)
            .count();
        let workspaces = standing
            .changed
            .iter()
            .filter(|p| p.starts_with("workspaces/"))
            .filter(memory)
            .count();
        set(&self.conn, "unshared", &format!("{personas} {workspaces}"))
    }

    /// The project root this writer indexes.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The index's folder.
    pub fn home(&self) -> &Path {
        &self.home
    }
}

/// `dir`, made `0700` where it is not there.
fn private_dir(dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

/// `writer.lock`, held: a second writer waits here until the first lets go (ADR 0079 §3).
fn lock(home: &Path) -> Result<crate::filelock::Held> {
    let mut open = std::fs::OpenOptions::new();
    open.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        open.mode(0o600);
    }
    let file = open.open(home.join(LOCK))?;
    file.lock()?;
    Ok(crate::filelock::Held::locked(file))
}

/// Every generation `current` does not name, removed. Only the lock's holder does this, and a
/// reader that opened one still finishes its query on the open file.
fn remove_old_generations(home: &Path) {
    let live = super::current_db(home).and_then(|db| db.parent().map(Path::to_path_buf));
    let Ok(reader) = std::fs::read_dir(home) else {
        return;
    };
    for entry in reader.flatten() {
        let path = entry.path();
        if entry.file_type().is_ok_and(|t| t.is_dir()) && Some(&path) != live.as_ref() {
            let _ = std::fs::remove_dir_all(path);
        }
    }
}

/// The generation at `db`, opened for writing, when it is one this build wrote.
fn valid(db: &Path) -> Option<Connection> {
    if !db.is_file() {
        return None;
    }
    let conn = Connection::open(db).ok()?;
    configure(&conn).ok()?;
    let schema = get(&conn, "schema");
    let rules = get(&conn, "rules");
    (schema.as_deref() == Some(SCHEMA) && rules == Some(super::rules_version())).then_some(conn)
}

fn configure(conn: &Connection) -> Result<()> {
    conn.pragma_update(None, "journal_mode", std::env::var("PURLIS_DEBUG_5672_JOURNAL").unwrap_or_else(|_| "WAL".into()))?; // [DEBUG-5672]
    conn.pragma_update(None, "synchronous", std::env::var("PURLIS_DEBUG_5672_SYNC").unwrap_or_else(|_| "NORMAL".into()))?; // [DEBUG-5672]
    conn.pragma_update(None, "secure_delete", "ON")?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    Ok(())
}

/// A new generation, built from every source and swapped in as the live one; opened for writing.
fn build(root: &Path, home: &Path) -> Result<Connection> {
    let name = format!(
        "g{}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos()),
        std::process::id()
    );
    let dir = home.join(&name);
    private_dir(&dir)?;
    let db = dir.join(DB);
    {
        let mut conn = Connection::open(&db)?;
        private_file(&db)?;
        configure(&conn)?;
        conn.execute_batch(TABLES)?;
        set(&conn, "schema", SCHEMA)?;
        set(&conn, "rules", &super::rules_version())?;
        set(&conn, "built", &super::now().to_string())?;
        let tx = conn.transaction()?;
        sweep(root, &tx, None)?;
        let t = std::time::Instant::now(); // [DEBUG-5672]
        tx.commit()?;
        dbg5672::add(&dbg5672::COMMIT, t); // [DEBUG-5672]
        set(&conn, "pending", "0")?;
        set(&conn, "as_of", &super::now().to_string())?;
        // Closing checkpoints the WAL and removes it, so the generation is one file when
        // `current` first names it.
        let t = std::time::Instant::now(); // [DEBUG-5672]
        conn.close().map_err(|(_, e)| e)?;
        dbg5672::add(&dbg5672::CLOSE, t); // [DEBUG-5672]
    }
    let staged = home.join(format!("{CURRENT}.{name}"));
    std::fs::write(&staged, &name)?;
    private_file(&staged)?;
    std::fs::rename(&staged, home.join(CURRENT))?;
    let conn = Connection::open(&db)?;
    configure(&conn)?;
    remove_old_generations(home);
    Ok(conn)
}

fn private_file(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    let _ = path;
    Ok(())
}

pub(super) fn get(conn: &Connection, key: &str) -> Option<String> {
    conn.query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0))
        .optional()
        .ok()
        .flatten()
}

fn set(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO meta(key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

/// `path` relative to `root`, with `/` between its parts.
fn relative(root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?;
    let parts: Vec<String> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    Some(parts.join("/"))
}

/// A relative path's sort key: its parts in order, as `Path` compares them. `\u{1}` sorts below
/// every character a name holds, which is what a shorter part does in a component comparison.
pub(super) fn sort_key(rel: &str) -> String {
    rel.replace('/', "\u{1}")
}

/// Whether `a` and `b` are the same path, or one holds the other.
fn related(a: &Path, b: &Path) -> bool {
    a.starts_with(b) || b.starts_with(a)
}

/// One row as it was recorded.
struct Recorded {
    id: i64,
    size: i64,
    mtime: i64,
    hash: String,
}

/// [`Writer::sweep`]'s work, inside the caller's transaction.
fn sweep(
    root: &Path,
    tx: &rusqlite::Transaction<'_>,
    touched: Option<&[PathBuf]>,
) -> Result<Swept> {
    let t = std::time::Instant::now(); // [DEBUG-5672]
    let every = sources::every(root);
    dbg5672::add(&dbg5672::EVERY, t); // [DEBUG-5672]
    let mut swept = Swept::default();
    // Rows of a source that is no longer one: a workspace or persona removed, a refs folder gone.
    let bases: BTreeSet<String> = every
        .iter()
        .filter_map(|s| relative(root, &s.dir))
        .collect();
    let recorded: Vec<String> = {
        let mut stmt = tx.prepare("SELECT DISTINCT base FROM item")?;
        stmt.query_map([], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?
    };
    for base in recorded.iter().filter(|b| !bases.contains(*b)) {
        swept.deleted += delete_base(tx, base)?;
    }
    tx.execute(
        "DELETE FROM unread WHERE base NOT IN (SELECT value FROM json_each(?1))",
        [serde_json::to_string(&bases).unwrap_or_default()],
    )?;
    let targets: Vec<&Source> = match touched {
        None => every.iter().collect(),
        Some(paths) => every
            .iter()
            .filter(|s| paths.iter().any(|p| related(p, &s.dir)))
            .collect(),
    };
    let one = sweep_sources(root, tx, &targets)?;
    swept.written += one.written;
    swept.deleted += one.deleted;
    let workspaces: BTreeSet<&str> = match touched {
        None => every
            .iter()
            .filter(|s| s.kind == Kind::Workspace)
            .map(|s| s.owner.as_str())
            .collect(),
        Some(paths) => every
            .iter()
            .filter(|s| s.kind == Kind::Workspace)
            .filter(|s| {
                let dir = root.join("workspaces").join(&s.owner);
                paths.iter().any(|p| related(p, &dir))
            })
            .map(|s| s.owner.as_str())
            .collect(),
    };
    tx.execute(
        "DELETE FROM workspace WHERE name NOT IN (SELECT value FROM json_each(?1))",
        [serde_json::to_string(
            &every
                .iter()
                .filter(|s| s.kind == Kind::Workspace)
                .map(|s| s.owner.as_str())
                .collect::<Vec<_>>(),
        )
        .unwrap_or_default()],
    )?;
    let t = std::time::Instant::now(); // [DEBUG-5672]
    for name in workspaces {
        workspace_row(root, tx, name)?;
    }
    dbg5672::add(&dbg5672::WS, t); // [DEBUG-5672]
    Ok(swept)
}

/// Every row listed from `base`, deleted.
fn delete_base(tx: &rusqlite::Transaction<'_>, base: &str) -> Result<usize> {
    let ids: Vec<i64> = {
        let mut stmt = tx.prepare("SELECT id FROM item WHERE base = ?1")?;
        stmt.query_map([base], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?
    };
    for id in &ids {
        delete_row(tx, *id)?;
    }
    tx.execute("DELETE FROM unread WHERE base = ?1", [base])?;
    Ok(ids.len())
}

fn delete_row(tx: &rusqlite::Transaction<'_>, id: i64) -> Result<()> {
    tx.execute("DELETE FROM words WHERE rowid = ?1", [id])?;
    tx.execute("DELETE FROM item WHERE id = ?1", [id])?;
    Ok(())
}

/// How many sources are planned at once, while the rows of the ones before are written: enough
/// to keep every thread busy, few enough that the planned rows of a batch stay small.
const BATCH: usize = 16;

/// Each source brought up to date: listed through the gate, each file whose stat moved read
/// again, each file gone deleted.
///
/// **Planned in parallel, written in order** (#5486). Listing, reading, hashing and scanning a
/// file need nothing from the database but the source's recorded rows, so each batch of sources
/// is planned on every thread at once ([`plan`]) while the one connection writes the batch
/// before it ([`apply`]). The writes are the ones a source-by-source sweep makes, in its order.
fn sweep_sources(
    root: &Path,
    tx: &rusqlite::Transaction<'_>,
    targets: &[&Source],
) -> Result<Swept> {
    let mut swept = Swept::default();
    let batches: Vec<Vec<(&Source, String)>> = targets
        .chunks(BATCH)
        .map(|chunk| {
            chunk
                .iter()
                .filter_map(|s| Some((*s, relative(root, &s.dir)?)))
                .collect()
        })
        .collect();
    std::thread::scope(|scope| -> Result<()> {
        let mut planning = None;
        for (n, batch) in batches.iter().enumerate() {
            let t = std::time::Instant::now(); // [DEBUG-5672]
            let planned = match planning.take() {
                Some(handle) => join(handle),
                None => plan(root, batch, recorded(tx, batch)?),
            };
            dbg5672::add(&dbg5672::WAIT, t); // [DEBUG-5672]
            if let Some(next) = batches.get(n + 1) {
                let t = std::time::Instant::now(); // [DEBUG-5672]
                let rows = recorded(tx, next)?;
                dbg5672::add(&dbg5672::RECORDED, t); // [DEBUG-5672]
                planning = Some(scope.spawn(move || plan(root, next, rows)));
            }
            let t = std::time::Instant::now(); // [DEBUG-5672]
            for (one, (source, _)) in planned.into_iter().zip(batch) {
                let done = apply(root, tx, source, one)?;
                swept.written += done.written;
                swept.deleted += done.deleted;
            }
            dbg5672::add(&dbg5672::APPLY, t); // [DEBUG-5672]
        }
        Ok(())
    })?;
    Ok(swept)
}

fn join<T>(handle: std::thread::ScopedJoinHandle<'_, T>) -> T {
    handle
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
}

/// The rows each source of `batch` recorded, by path.
fn recorded(
    tx: &rusqlite::Transaction<'_>,
    batch: &[(&Source, String)],
) -> Result<Vec<BTreeMap<String, Recorded>>> {
    let mut stmt = tx.prepare("SELECT path, id, size, mtime, hash FROM item WHERE base = ?1")?;
    batch
        .iter()
        .map(|(_, base)| {
            stmt.query_map([base], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    Recorded {
                        id: r.get(1)?,
                        size: r.get(2)?,
                        mtime: r.get(3)?,
                        hash: r.get(4)?,
                    },
                ))
            })?
            .collect::<rusqlite::Result<_>>()
            .map_err(Into::into)
        })
        .collect()
}

/// What one source's sweep writes, decided without the database.
struct Planned {
    base: String,
    unread: crate::memstore::Unread,
    changes: Vec<Change>,
}

/// One write of a sweep.
enum Change {
    /// The file's stat moved and its text did not.
    Touch { id: i64, size: i64, mtime: i64 },
    /// The file is gone, or no longer readable as text.
    Delete(i64),
    /// The file is new, or its text changed: the old row, if any, goes first.
    Write { was: Option<i64>, row: Row },
}

/// A row as it is written: its text already scanned and split into words.
struct Row {
    rel: String,
    title: String,
    date: Option<String>,
    size: i64,
    mtime: i64,
    hash: String,
    title_words: String,
    text_words: String,
}

/// Every source of `batch` planned, each against the rows it `recorded`, on every thread.
fn plan(
    root: &Path,
    batch: &[(&Source, String)],
    recorded: Vec<BTreeMap<String, Recorded>>,
) -> Vec<Planned> {
    let t = std::time::Instant::now(); // [DEBUG-5672]
    let listed = in_parallel(batch, |(source, _)| {
        crate::memstore::read_files(root, &source.dir)
    });
    dbg5672::add(&dbg5672::LIST, t); // [DEBUG-5672]
    let mut files: Vec<(usize, PathBuf, String, Option<Recorded>)> = Vec::new();
    let mut gone: Vec<Vec<i64>> = Vec::new();
    let mut unread_of = Vec::new();
    for (at, ((found, unread), mut recorded)) in listed.into_iter().zip(recorded).enumerate() {
        for path in found {
            let Some(rel) = relative(root, &path) else {
                continue;
            };
            let was = recorded.remove(&rel);
            files.push((at, path, rel, was));
        }
        gone.push(recorded.into_values().map(|r| r.id).collect());
        unread_of.push(unread);
    }
    let t = std::time::Instant::now(); // [DEBUG-5672]
    let changes = in_parallel(&files, |(_, path, rel, was)| {
        change(path, rel, was.as_ref())
    });
    dbg5672::add(&dbg5672::CHANGE_WALL, t); // [DEBUG-5672]
    let mut planned: Vec<Planned> = batch
        .iter()
        .zip(unread_of)
        .map(|((_, base), unread)| Planned {
            base: base.clone(),
            unread,
            changes: Vec::new(),
        })
        .collect();
    for ((at, ..), change) in files.iter().zip(changes) {
        planned[*at].changes.extend(change);
    }
    for (one, gone) in planned.iter_mut().zip(gone) {
        one.changes.extend(gone.into_iter().map(Change::Delete));
    }
    planned
}

/// What the sweep writes for the file at `path`, recorded as `was`: nothing when its stat has
/// not moved.
fn change(path: &Path, rel: &str, was: Option<&Recorded>) -> Option<Change> {
    let t = std::time::Instant::now(); // [DEBUG-5672]
    let Ok(meta) = std::fs::metadata(path) else {
        return was.map(|was| Change::Delete(was.id));
    };
    let size = meta.len() as i64;
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos() as i64);
    if let Some(was) = was
        && was.size == size
        && was.mtime == mtime
    {
        return None;
    }
    dbg5672::add(&dbg5672::STAT, t); // [DEBUG-5672]
    let t = std::time::Instant::now(); // [DEBUG-5672]
    let read = std::fs::read(path); // [DEBUG-5672]
    dbg5672::add(&dbg5672::READ, t); // [DEBUG-5672]
    let t = std::time::Instant::now(); // [DEBUG-5672]
    let Some(text) = read.ok().and_then(crate::memstore::text_of) else {
        // Not readable as text: not a memory to the scan either.
        return was.map(|was| Change::Delete(was.id));
    };
    let hash = {
        use sha2::Digest;
        crate::extension::hex(&sha2::Sha256::digest(text.as_bytes()))
    };
    if let Some(was) = was
        && was.hash == hash
    {
        return Some(Change::Touch {
            id: was.id,
            size,
            mtime,
        });
    }
    let title = words::scanned(&crate::memstore::title_in(path, &text));
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let date = crate::memstore::memory_date(&text, &name).map(|d| d.format("%Y-%m-%d").to_string());
    let (title_words, text_words) = (words::indexed(&title), words::indexed(&words::scanned(&text))); // [DEBUG-5672]
    dbg5672::add(&dbg5672::SCAN, t); // [DEBUG-5672]
    Some(Change::Write {
        was: was.map(|was| was.id),
        row: Row {
            rel: rel.to_string(),
            title_words,
            text_words,
            title,
            date,
            size,
            mtime,
            hash,
        },
    })
}

/// `work` done on each of `items` across the machine's threads, the answers in `items`' order.
fn in_parallel<T: Sync, R: Send>(items: &[T], work: impl Fn(&T) -> R + Sync) -> Vec<R> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let threads = dbg5672::threads().min(items.len()); // [DEBUG-5672]
    if threads <= 1 {
        return items.iter().map(work).collect();
    }
    let next = AtomicUsize::new(0);
    let mut done: Vec<(usize, R)> = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..threads)
            .map(|_| {
                scope.spawn(|| {
                    let mut mine = Vec::new();
                    loop {
                        let at = next.fetch_add(1, Ordering::Relaxed);
                        let Some(item) = items.get(at) else {
                            return mine;
                        };
                        mine.push((at, work(item)));
                    }
                })
            })
            .collect();
        workers.into_iter().flat_map(join).collect()
    });
    done.sort_unstable_by_key(|(at, _)| *at);
    done.into_iter().map(|(_, r)| r).collect()
}

/// One source's planned writes, made.
fn apply(
    root: &Path,
    tx: &rusqlite::Transaction<'_>,
    source: &Source,
    planned: Planned,
) -> Result<Swept> {
    let base = planned.base;
    let mut swept = Swept::default();
    tx.execute("DELETE FROM unread WHERE base = ?1", [&base])?;
    for (path, code) in &planned.unread {
        tx.execute(
            "INSERT INTO unread(base, kind, owner, path, code) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                base,
                source.kind.as_str(),
                source.owner,
                relative(root, path).unwrap_or_default(),
                code
            ],
        )?;
    }
    let rank = sources::rank(source.kind, &source.owner);
    let basekey = sort_key(&base);
    let reach = sources::reach_words(source.kind, &source.owner);
    for change in planned.changes {
        match change {
            Change::Touch { id, size, mtime } => {
                tx.execute(
                    "UPDATE item SET size = ?2, mtime = ?3 WHERE id = ?1",
                    params![id, size, mtime],
                )?;
            }
            Change::Delete(id) => {
                delete_row(tx, id)?;
                swept.deleted += 1;
            }
            Change::Write { was, row } => {
                if let Some(was) = was {
                    delete_row(tx, was)?;
                }
                tx.execute(
                    "INSERT INTO item(path, base, kind, owner, audience, rank, basekey, pathkey,
                                      title, date, size, mtime, hash)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                    params![
                        row.rel,
                        base,
                        source.kind.as_str(),
                        source.owner,
                        AUDIENCE,
                        rank,
                        basekey,
                        sort_key(&row.rel),
                        row.title,
                        row.date,
                        row.size,
                        row.mtime,
                        row.hash
                    ],
                )?;
                let id = tx.last_insert_rowid();
                tx.execute(
                    "INSERT INTO words(rowid, title, text, reach) VALUES (?1, ?2, ?3, ?4)",
                    params![id, row.title_words, row.text_words, reach],
                )?;
                swept.written += 1;
            }
        }
    }
    Ok(swept)
}

/// A workspace's summary row: its vision's first line, scanned, and the newest modification time
/// of its own files (`workspace.md`, `workspace.json`).
fn workspace_row(root: &Path, tx: &rusqlite::Transaction<'_>, name: &str) -> Result<()> {
    let plane = crate::workspaces::Plane::open(root);
    let vision = plane
        .workspace(name)
        .map(|ws| ws.vision())
        .unwrap_or_default();
    let first = crate::mdsection::split_lines(crate::memstore::py_strip(&vision))
        .first()
        .map(|l| crate::memstore::py_strip(l).to_string())
        .unwrap_or_default();
    let dir = root.join("workspaces").join(name);
    let mtime = ["workspace.md", "workspace.json"]
        .iter()
        .filter_map(|f| std::fs::metadata(dir.join(f)).ok()?.modified().ok())
        .filter_map(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs_f64())
        .fold(None, |best: Option<f64>, m| {
            Some(best.map_or(m, |b| b.max(m)))
        });
    tx.execute(
        "INSERT INTO workspace(name, vision, mtime) VALUES (?1, ?2, ?3)
         ON CONFLICT(name) DO UPDATE SET vision = excluded.vision, mtime = excluded.mtime",
        params![name, words::scanned(&first), mtime],
    )?;
    Ok(())
}
