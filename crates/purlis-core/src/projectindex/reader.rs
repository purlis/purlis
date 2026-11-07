// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Reading the index, read-only, from any process (ADR 0079 §3).
//!
//! A reader opens the generation `current` names with `SQLITE_OPEN_READ_ONLY` and `query_only`,
//! and never writes. A generation this build did not write, or one it cannot open, is no index:
//! the caller scans the files instead. Every query names a [`Reach`], which the FTS5 query
//! filters on, and the row table is filtered by again.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags, params_from_iter};

use super::sources::{Kind, Reach};
use super::{LOCK, Result, SCHEMA};

/// How far behind the files an answer may be (ADR 0079 §5).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Freshness {
    /// The writer has work queued, or is in the middle of it.
    pub refreshing: bool,
    /// A writer holds the index: the host is running and keeps it up to date. With none, the
    /// index is as the host last left it.
    pub writer: bool,
    /// When the writer last finished its work, as seconds since the epoch.
    pub as_of: Option<f64>,
}

impl Freshness {
    /// Whether an answer may be behind the files: work is queued, or nobody keeps the index.
    pub fn behind(&self) -> bool {
        self.refreshing || !self.writer
    }
}

/// One indexed item, as a query answers it.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub path: PathBuf,
    /// The source folder it was listed from.
    pub base: PathBuf,
    pub kind: Kind,
    pub owner: String,
    /// Its title, after the shape scanner.
    pub title: String,
    pub date: Option<chrono::NaiveDate>,
    /// BM25, lower is better; `0.0` for a listing with no query.
    pub score: f64,
}

/// What the briefing needs of one workspace without reading its folders (#5503).
#[derive(Debug, Clone, PartialEq)]
pub struct WorkspaceSummary {
    pub name: String,
    /// The first line of its vision, after the shape scanner.
    pub vision: String,
    /// The newest modification time among its own files and its indexed items.
    pub last_active: Option<f64>,
    pub memories: usize,
    pub todos: usize,
    pub sessions: usize,
}

/// A read-only connection to the live generation.
pub struct Reader {
    root: PathBuf,
    home: PathBuf,
    conn: Connection,
}

impl Reader {
    /// The index of the project at `root`, or `None` where there is none this build can read.
    pub fn open(root: &Path) -> Option<Self> {
        let home = super::home(root);
        let db = super::current_db(&home)?;
        if !db.is_file() {
            return None;
        }
        let conn = open_read_only(&db)?;
        let meta = |key: &str| super::writer::get(&conn, key);
        if meta("schema").as_deref() != Some(SCHEMA)
            || meta("rules") != Some(super::rules_version())
        {
            return None;
        }
        Some(Self {
            root: root.to_path_buf(),
            home,
            conn,
        })
    }

    /// How far behind the files this index may be.
    pub fn freshness(&self) -> Freshness {
        let meta = |key: &str| super::writer::get(&self.conn, key);
        Freshness {
            refreshing: meta("pending").as_deref() == Some("1"),
            writer: writer_holds(&self.home),
            as_of: meta("as_of").and_then(|t| t.parse().ok()),
        }
    }

    /// What of the project's memory the host's last look at its git standing found
    /// uncommitted ([`super::Writer::record_standing`]): persona files, then workspace files.
    /// `None` when no look has been recorded.
    pub fn unshared(&self) -> Option<(usize, usize)> {
        let said = super::writer::get(&self.conn, "unshared")?;
        let (personas, workspaces) = said.split_once(' ')?;
        Some((personas.parse().ok()?, workspaces.parse().ok()?))
    }

    /// The rows in `reach` whose title or text holds a word starting with any of `terms`, best
    /// first by BM25 with the title weighing three times the body, ties by path. At most `limit`
    /// rows; `limit < 0` is no cap.
    pub fn search(&self, reach: &Reach, terms: &[String], limit: i64) -> Result<Vec<Row>> {
        if terms.is_empty() || reach.is_empty() {
            return Ok(Vec::new());
        }
        let words: Vec<String> = terms.iter().map(|t| format!("\"{t}\"*")).collect();
        let matched = format!(
            "({{title text}} : ({})) AND ({})",
            words.join(" OR "),
            reach.fts()
        );
        let (filter, mut values) = filter(reach);
        let sql = format!(
            "SELECT i.path, i.base, i.kind, i.owner, i.title, i.date, w.rank
             FROM (SELECT rowid, rank FROM words WHERE words MATCH ? ORDER BY rank LIMIT ?) AS w
             JOIN item AS i ON i.id = w.rowid
             WHERE {filter}
             ORDER BY w.rank, i.path"
        );
        let mut all = vec![
            rusqlite::types::Value::Text(matched),
            rusqlite::types::Value::Integer(limit),
        ];
        all.append(&mut values);
        self.rows(&sql, all)
    }

    /// The rows in `reach`, newest first by recorded date with the undated last, then in
    /// recall's order of bases, then by path. At most `limit`; `limit < 0` is no cap.
    pub fn newest(&self, reach: &Reach, limit: i64) -> Result<Vec<Row>> {
        if reach.is_empty() {
            return Ok(Vec::new());
        }
        let (filter, mut values) = filter(reach);
        let sql = format!(
            "SELECT i.path, i.base, i.kind, i.owner, i.title, i.date, 0.0 FROM item AS i
             WHERE {filter}
             ORDER BY i.date DESC, i.rank, i.basekey, i.pathkey
             LIMIT ?"
        );
        values.push(rusqlite::types::Value::Integer(limit));
        self.rows(&sql, values)
    }

    /// The oldest rows of one kind and owner, oldest first: a workspace's longest-waiting
    /// todos, by recorded date.
    pub fn oldest(&self, kind: Kind, owner: &str, limit: i64) -> Result<Vec<Row>> {
        let sql = "SELECT i.path, i.base, i.kind, i.owner, i.title, i.date, 0.0 FROM item AS i
                   WHERE i.kind = ?1 AND i.owner = ?2
                   ORDER BY i.date IS NULL, i.date, i.pathkey
                   LIMIT ?3";
        self.rows(
            sql,
            vec![
                kind.as_str().to_string().into(),
                owner.to_string().into(),
                limit.into(),
            ],
        )
    }

    /// How many rows one kind and owner has.
    pub fn count(&self, kind: Kind, owner: &str) -> Result<usize> {
        let n: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM item WHERE kind = ?1 AND owner = ?2",
            [kind.as_str(), owner],
            |r| r.get(0),
        )?;
        Ok(n as usize)
    }

    /// What could not be looked at in the sources of `reach`, as the last sweep found it.
    pub fn unread(&self, reach: &Reach) -> Result<crate::memstore::Unread> {
        let (filter, values) = filter(reach);
        let sql = format!("SELECT i.path, i.code FROM unread AS i WHERE {filter} ORDER BY i.path");
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt
            .query_map(params_from_iter(values), |r| {
                Ok((
                    self.root.join(r.get::<_, String>(0)?),
                    r.get::<_, Option<i32>>(1)?,
                ))
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows)
    }

    /// Every workspace's summary, by name.
    pub fn workspaces(&self) -> Result<Vec<WorkspaceSummary>> {
        let mut counts: BTreeMap<String, (usize, usize, usize, Option<f64>)> = BTreeMap::new();
        {
            let mut stmt = self.conn.prepare(
                "SELECT owner, kind, COUNT(*), MAX(mtime) FROM item
                 WHERE kind IN ('workspace', 'todos', 'sessions') AND owner != ''
                 GROUP BY owner, kind",
            )?;
            let rows = stmt.query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, Option<i64>>(3)?,
                ))
            })?;
            for row in rows {
                let (owner, kind, n, mtime) = row?;
                let entry = counts.entry(owner).or_default();
                match kind.as_str() {
                    "workspace" => entry.0 = n as usize,
                    "todos" => entry.1 = n as usize,
                    _ => entry.2 = n as usize,
                }
                if let Some(ns) = mtime {
                    let secs = ns as f64 / 1e9;
                    entry.3 = Some(entry.3.map_or(secs, |b: f64| b.max(secs)));
                }
            }
        }
        let mut stmt = self
            .conn
            .prepare("SELECT name, vision, mtime FROM workspace ORDER BY name")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<f64>>(2)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (name, vision, own) = row?;
            let (memories, todos, sessions, items) = counts.remove(&name).unwrap_or_default();
            let last_active = match (own, items) {
                (Some(a), Some(b)) => Some(a.max(b)),
                (a, b) => a.or(b),
            };
            out.push(WorkspaceSummary {
                name,
                vision,
                last_active,
                memories,
                todos,
                sessions,
            });
        }
        Ok(out)
    }

    fn rows(&self, sql: &str, values: Vec<rusqlite::types::Value>) -> Result<Vec<Row>> {
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt
            .query_map(params_from_iter(values), |r| {
                let kind: String = r.get(2)?;
                let date: Option<String> = r.get(5)?;
                Ok(Row {
                    path: self.root.join(r.get::<_, String>(0)?),
                    base: self.root.join(r.get::<_, String>(1)?),
                    kind: Kind::parse(&kind).unwrap_or(Kind::Workspace),
                    owner: r.get(3)?,
                    title: r.get(4)?,
                    date: date.and_then(|d| d.parse().ok()),
                    score: r.get(6)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows)
    }
}

/// The row table's filter for `reach`, and its values.
fn filter(reach: &Reach) -> (String, Vec<rusqlite::types::Value>) {
    let mut clauses = Vec::new();
    let mut values: Vec<rusqlite::types::Value> = Vec::new();
    for (kind, owner) in reach.pairs() {
        values.push(kind.as_str().to_string().into());
        match owner {
            None => clauses.push("(i.kind = ?)".to_string()),
            Some(owner) => {
                values.push(owner.clone().into());
                clauses.push("(i.kind = ? AND i.owner = ?)".to_string());
            }
        }
    }
    if clauses.is_empty() {
        clauses.push("0".to_string());
    }
    (format!("({})", clauses.join(" OR ")), values)
}

/// `db`, opened read-only. A generation with no WAL beside it, in a folder this process may not
/// write (a chat's sandbox denies writing the index, ADR 0067 as amended), is opened immutable:
/// no writer is in the middle of it, or its WAL would be there.
fn open_read_only(db: &Path) -> Option<Connection> {
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let checked = |conn: Connection| -> Option<Connection> {
        conn.pragma_update(None, "query_only", "ON").ok()?;
        conn.busy_timeout(std::time::Duration::from_millis(500))
            .ok()?;
        conn.query_row("SELECT COUNT(*) FROM meta", [], |_| Ok(()))
            .ok()?;
        Some(conn)
    };
    if let Some(conn) = Connection::open_with_flags(db, flags)
        .ok()
        .and_then(checked)
    {
        return Some(conn);
    }
    let mut wal = db.as_os_str().to_owned();
    wal.push("-wal");
    if Path::new(&wal).exists() {
        return None;
    }
    let uri = format!("file:{}?immutable=1", uri_path(db)?);
    Connection::open_with_flags(&uri, flags | OpenFlags::SQLITE_OPEN_URI)
        .ok()
        .and_then(checked)
}

/// `path` as a `file:` URI's path: `%`, `?` and `#` escaped.
fn uri_path(path: &Path) -> Option<String> {
    let text = path.to_str()?;
    Some(
        text.replace('%', "%25")
            .replace('?', "%3f")
            .replace('#', "%23"),
    )
}

/// Whether a writer holds `writer.lock` now: a shared lock this process cannot take.
pub(super) fn writer_holds(home: &Path) -> bool {
    let Ok(file) = std::fs::File::open(home.join(LOCK)) else {
        return false;
    };
    match file.try_lock_shared() {
        Ok(()) => {
            let _ = file.unlock();
            false
        }
        Err(std::fs::TryLockError::WouldBlock) => true,
        Err(std::fs::TryLockError::Error(_)) => false,
    }
}
