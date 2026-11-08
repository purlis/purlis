// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A grade a source carries** (ADR 0100, amended 2026-11-08; #4346): a file beside the skill's
//! folder, `<parent>/<folder>.grades.json`, never inside it (spec #2762's decision 21). Publish
//! writes it, and the install and update review shows it as the publisher's: whose it is, that
//! purlis has not checked it, and whether it was measured on the version under review.
//!
//! It is read as anyone may have written it, failing closed: a file that is not exactly what
//! publish writes is left out whole, with the reason, and nothing from it is shown. It never
//! reaches the eval record (ADR 0105, amended 2026-11-08).

use gix::objs::tree::{Entry, EntryKind};
use gix::objs::{Kind, WriteTo as _};
use purlis_skills::grade::GradeEnd;
use serde::{Deserialize, Serialize};

use crate::skills::grades;

/// What the grade file's name adds to the skill folder's.
pub const SUFFIX: &str = ".grades.json";

/// The most bytes a grade file may hold.
pub const MOST_BYTES: u64 = 64 * 1024;

/// The most grades a file may hold: as many as a publish's description lists.
pub const MOST_GRADES: usize = 10;

/// The format publish writes.
const FORMAT: u32 = 1;

/// **The grade file beside `folder`** (a skill's folder in the source, `/` between the parts), or
/// `None` for a skill whose folder is the source's root, which has nowhere beside it.
pub fn beside(folder: &str) -> Option<String> {
    let folder = folder.trim_matches('/');
    (!folder.is_empty()).then(|| format!("{folder}{SUFFIX}"))
}

/// The file, as publish writes it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    format: u32,
    skill: String,
    grades: Vec<Row>,
}

/// One grade, as the file holds it: only what a publish's description holds, and the tree it
/// measured.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Row {
    /// When, `YYYY-MM-DD`.
    pub date: String,
    /// The git tree id of the skill's folder the grade measured.
    pub tree: String,
    pub harness: String,
    pub grader: String,
    /// The pass rate with the skill and without it, in percent; `None` for an arm not run.
    pub with: Option<u8>,
    pub without: Option<u8>,
    pub runs: u8,
    pub ended: GradeEnd,
}

/// **One grade, as the review shows it.**
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Carried {
    pub date: String,
    pub harness: String,
    pub grader: String,
    pub with: Option<u8>,
    pub without: Option<u8>,
    pub runs: u8,
    /// How it ended, in words.
    pub ended: String,
}

/// **The grade a source carries for a skill, as its review shows it.**
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SourceGrade {
    /// The newest grade, `whose` it is, whether it measured the folder under review, and how many
    /// older ones the file holds.
    Shown {
        whose: String,
        newest: Carried,
        this_version: bool,
        more: usize,
    },
    /// A file that is not what publish writes: nothing from it is shown, and `why` says so.
    LeftOut { whose: String, why: String },
}

/// **The grade `bytes` hold for `skill`**, shown as `whose` it is, against the folder whose tree
/// is `tree`.
pub fn shown(bytes: &[u8], skill: &str, whose: &str, tree: &str) -> SourceGrade {
    match read(bytes, skill) {
        Ok(rows) => {
            let newest = &rows[0];
            SourceGrade::Shown {
                whose: whose.to_owned(),
                newest: Carried {
                    date: newest.date.clone(),
                    harness: newest.harness.clone(),
                    grader: newest.grader.clone(),
                    with: newest.with,
                    without: newest.without,
                    runs: newest.runs,
                    ended: grades::ended_said(newest.ended).to_owned(),
                },
                this_version: newest.tree.eq_ignore_ascii_case(tree),
                more: rows.len() - 1,
            }
        }
        Err(why) => SourceGrade::LeftOut {
            whose: whose.to_owned(),
            why,
        },
    }
}

/// **The rows `bytes` hold for `skill`**, newest first, or why they are left out.
pub fn read(bytes: &[u8], skill: &str) -> Result<Vec<Row>, String> {
    if bytes.len() as u64 > MOST_BYTES {
        return Err(format!("it is larger than {} KiB", MOST_BYTES >> 10));
    }
    let file: File = serde_json::from_slice(bytes)
        .map_err(|_| "it is not a grade file purlis reads".to_owned())?;
    if file.format != FORMAT {
        return Err(format!(
            "it is in format {}, which purlis does not read",
            file.format
        ));
    }
    if file.skill != skill {
        return Err("it names another skill".to_owned());
    }
    if file.grades.is_empty() {
        return Err("it holds no grade".to_owned());
    }
    if file.grades.len() > MOST_GRADES {
        return Err(format!("it holds more than {MOST_GRADES} grades"));
    }
    for row in &file.grades {
        row_problem(row)?;
    }
    Ok(file.grades)
}

/// **The row a publish writes of `graded`**, tied to `tree`: only what the request's description
/// holds. `None` where any of it is not what the file may hold (a tree that is no git id, such as
/// a grade of changes never committed), so publish never writes a file its review would leave
/// out.
pub fn row_of(graded: &grades::Graded, tree: String) -> Option<Row> {
    let rate = |arm| {
        graded
            .report
            .arms
            .iter()
            .find(|score| score.arm == arm)
            .and_then(|score| score.pass_rate.as_ref())
            .map(|stat| (stat.mean * 100.0).round().clamp(0.0, 100.0) as u8)
    };
    let row = Row {
        date: graded.ts.chars().take(10).collect(),
        tree,
        harness: graded
            .harness
            .clone()
            .unwrap_or_else(|| "unknown".to_owned()),
        grader: graded.report.grader.clone(),
        with: rate(purlis_skills::grade::Arm::With),
        without: rate(purlis_skills::grade::Arm::Without),
        runs: graded.runs,
        ended: graded.report.ended,
    };
    row_problem(&row).is_ok().then_some(row)
}

/// **The grade file of `skill` holding `rows`**, newest first, as publish writes it; `None` where
/// there is no row.
pub fn file_of(skill: &str, rows: Vec<Row>) -> Option<Vec<u8>> {
    if rows.is_empty() {
        return None;
    }
    let file = File {
        format: FORMAT,
        skill: skill.to_owned(),
        grades: rows,
    };
    let mut bytes = serde_json::to_vec_pretty(&file).ok()?;
    bytes.push(b'\n');
    Some(bytes)
}

/// Why `row` holds something publish would not write.
fn row_problem(row: &Row) -> Result<(), String> {
    let date = row.date.as_bytes();
    let is_date = date.len() == 10
        && date.iter().enumerate().all(|(at, b)| {
            if at == 4 || at == 7 {
                *b == b'-'
            } else {
                b.is_ascii_digit()
            }
        });
    if !is_date {
        return Err("a grade's date is not a date".to_owned());
    }
    if !(matches!(row.tree.len(), 40 | 64) && row.tree.bytes().all(|b| b.is_ascii_hexdigit())) {
        return Err("a grade's tree is not a git id".to_owned());
    }
    for (what, name) in [("harness", &row.harness), ("grader", &row.grader)] {
        if !is_plain(name) {
            return Err(format!("a grade's {what} is not a plain name"));
        }
    }
    for (what, rate) in [("with", row.with), ("without", row.without)] {
        if let Some(rate) = rate
            && rate > 100
        {
            return Err(format!(
                "a grade's \"{what}\" rate is {rate}, outside 0 to 100"
            ));
        }
    }
    Ok(())
}

/// Whether `name` is what publish writes for a name: 1 to 64 letters, digits, `-`, `_`, `.` or
/// spaces.
fn is_plain(name: &str) -> bool {
    (1..=64).contains(&name.len())
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ' '))
}

/// **The git tree id of a folder holding `files`** (each its path's parts, whether it is
/// executable, and its bytes), as git's SHA-1 writes it, worked out by gix: the id a grade's
/// `tree` is compared with where no git tree was read, as for an extension's source.
pub fn tree_id<'a>(files: impl IntoIterator<Item = (&'a [String], bool, &'a [u8])>) -> String {
    let mut root = Folder::default();
    for (parts, executable, bytes) in files {
        root.add(parts, executable, bytes);
    }
    root.id().to_string()
}

/// [`tree_id`] of a skill's files as a source handed them.
pub fn tree_of(files: &[super::SourceFile]) -> String {
    tree_id(files.iter().map(|file| {
        (
            file.parts.as_slice(),
            file.executable,
            file.bytes.as_slice(),
        )
    }))
}

/// A folder of [`tree_id`], built from its files' paths.
#[derive(Default)]
struct Folder {
    files: Vec<(String, bool, gix::ObjectId)>,
    folders: Vec<(String, Folder)>,
}

impl Folder {
    fn add(&mut self, parts: &[String], executable: bool, bytes: &[u8]) {
        match parts {
            [] => {}
            [name] => self
                .files
                .push((name.clone(), executable, object(Kind::Blob, bytes))),
            [first, rest @ ..] => {
                let at = match self.folders.iter().position(|(name, _)| name == first) {
                    Some(at) => at,
                    None => {
                        self.folders.push((first.clone(), Folder::default()));
                        self.folders.len() - 1
                    }
                };
                self.folders[at].1.add(rest, executable, bytes);
            }
        }
    }

    /// Its tree object's id.
    fn id(&self) -> gix::ObjectId {
        let mut entries: Vec<Entry> = Vec::new();
        for (name, executable, oid) in &self.files {
            let kind = if *executable {
                EntryKind::BlobExecutable
            } else {
                EntryKind::Blob
            };
            entries.push(Entry {
                mode: kind.into(),
                filename: name.as_str().into(),
                oid: *oid,
            });
        }
        for (name, folder) in &self.folders {
            entries.push(Entry {
                mode: EntryKind::Tree.into(),
                filename: name.as_str().into(),
                oid: folder.id(),
            });
        }
        entries.sort();
        let mut body = Vec::new();
        // Writing to a Vec cannot fail, and the entries are sorted as git sorts them.
        let _ = gix::objs::Tree { entries }.write_to(&mut body);
        object(Kind::Tree, &body)
    }
}

/// The id git's SHA-1 gives an object of `kind` holding `body`.
fn object(kind: Kind, body: &[u8]) -> gix::ObjectId {
    gix::objs::compute_hash(gix::hash::Kind::Sha1, kind, body)
        .unwrap_or_else(|_| gix::ObjectId::null(gix::hash::Kind::Sha1))
}
