// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **When an artifact's data was last updated** (ADR 0120 §6, #5955): what its icon and its bar
//! say, and whether the icon is dimmed as stale.
//!
//! - **A date the data declares comes first**: `data.json`'s top-level [`DECLARED`] key, an
//!   RFC 3339 time, a `YYYY-MM-DD` day (its midnight UTC) or whole seconds since 1970. A
//!   regenerated file says when its values were true, which no file time can.
//! - **Else the data files' commit time**, while git holds them as committed: a fresh clone
//!   writes every file at once, so a write time there says when it was cloned.
//! - **Else their write time**: data is gitignored (ADR 0120 §1), so this is the usual answer.
//!
//! The data is `data.json` and everything under `data/` ([`super::serve::is_data`]); a page
//! with none is its own data, and its `index.html` is read for the times. A link is never
//! followed, and nothing here writes.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::{DATA_DIR, DATA_FILE, PAGE};

/// The key of `data.json` that declares when its data was true.
pub const DECLARED: &str = "updated";

/// How long after its data's time an artifact is drawn stale: a day, the slowest schedule a
/// dashboard is regenerated on (spec #5948, story 45).
pub const STALE_AFTER: Duration = Duration::from_secs(24 * 60 * 60);

/// The most bytes of `data.json` read for its declared date; a larger file declares nothing.
const DECLARED_MOST: u64 = 8 * 1024 * 1024;

/// How deep `data/` is walked for its files' times.
const DEEPEST: usize = 8;

/// Where an artifact's freshness was read from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    /// The data's own [`DECLARED`] date.
    Declared,
    /// The data files' last commit.
    Committed,
    /// The data files' last write.
    Written,
}

/// When an artifact's data was last updated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Freshness {
    /// Seconds since 1970.
    pub at: i64,
    pub source: Source,
}

impl Freshness {
    /// When it turns stale, in seconds since 1970.
    pub fn stale_at(&self) -> i64 {
        self.at
            .saturating_add(i64::try_from(STALE_AFTER.as_secs()).unwrap_or(i64::MAX))
    }

    /// Whether it is stale at `now`, in seconds since 1970.
    pub fn stale(&self, now: i64) -> bool {
        now >= self.stale_at()
    }
}

/// **The freshness of the artifact in `folder`** (a folder [`super::resolve`] answered), or
/// nothing when it has no data and no page to read a time from.
pub fn of(folder: &Path) -> Option<Freshness> {
    if let Some(at) = declared(folder) {
        return Some(Freshness {
            at,
            source: Source::Declared,
        });
    }
    let mut files = data_files(folder);
    if files.is_empty() && plain_file(&folder.join(PAGE)) {
        files.push(PathBuf::from(PAGE));
    }
    if files.is_empty() {
        return None;
    }
    if let Some(at) = committed(folder, &files) {
        return Some(Freshness {
            at,
            source: Source::Committed,
        });
    }
    written(folder, &files).map(|at| Freshness {
        at,
        source: Source::Written,
    })
}

/// The [`DECLARED`] date of `folder`'s `data.json`, if it declares one that reads.
fn declared(folder: &Path) -> Option<i64> {
    let path = folder.join(DATA_FILE);
    if std::fs::symlink_metadata(&path).ok()?.len() > DECLARED_MOST {
        return None;
    }
    let bytes = crate::contain::read_no_link(folder, &path).ok()?;
    let data: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    match data.get(DECLARED)? {
        serde_json::Value::Number(seconds) => seconds.as_i64(),
        serde_json::Value::String(text) => date(text.trim()),
        _ => None,
    }
}

/// An RFC 3339 time, or a `YYYY-MM-DD` day at midnight UTC, in seconds since 1970.
fn date(text: &str) -> Option<i64> {
    if let Ok(at) = chrono::DateTime::parse_from_rfc3339(text) {
        return Some(at.timestamp());
    }
    let day = chrono::NaiveDate::parse_from_str(text, "%Y-%m-%d").ok()?;
    Some(day.and_hms_opt(0, 0, 0)?.and_utc().timestamp())
}

fn plain_file(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|found| found.file_type().is_file())
}

/// `folder`'s data files, relative to it: `data.json` and every plain file under `data/`.
fn data_files(folder: &Path) -> Vec<PathBuf> {
    fn walk(folder: &Path, inside: &Path, depth: usize, files: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(folder.join(inside)) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            if name.to_string_lossy().starts_with('.') {
                continue;
            }
            // The entry's own type: a link is a link, and never followed.
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let path = inside.join(&name);
            if kind.is_file() {
                files.push(path);
            } else if kind.is_dir() && depth < DEEPEST {
                walk(folder, &path, depth + 1, files);
            }
        }
    }
    let mut files = Vec::new();
    if plain_file(&folder.join(DATA_FILE)) {
        files.push(PathBuf::from(DATA_FILE));
    }
    let dir = folder.join(DATA_DIR);
    if std::fs::symlink_metadata(&dir).is_ok_and(|found| found.file_type().is_dir()) {
        walk(folder, Path::new(DATA_DIR), 0, &mut files);
    }
    files
}

/// When `files` (relative to `folder`) were last committed, while git holds every one of them
/// as committed: none untracked, none changed since.
fn committed(folder: &Path, files: &[PathBuf]) -> Option<i64> {
    let specs: Vec<String> = files
        .iter()
        .map(|file| format!(":(literal){}", file.to_string_lossy()))
        .collect();
    let ask = |head: &[&str]| {
        let mut args: Vec<&str> = head.to_vec();
        args.push("--");
        args.extend(specs.iter().map(String::as_str));
        crate::worktree::git::run(folder, &args, Duration::from_secs(5))
            .ok()
            .filter(crate::worktree::git::Run::ok)
    };
    let status = ask(&[
        "status",
        "--porcelain",
        "--untracked-files=all",
        "--ignored",
    ])?;
    if !status.out.trim().is_empty() {
        return None;
    }
    let log = ask(&["log", "-1", "--format=%ct"])?;
    log.out.trim().parse().ok()
}

/// When the newest of `files` (relative to `folder`) was written.
fn written(folder: &Path, files: &[PathBuf]) -> Option<i64> {
    files
        .iter()
        .filter_map(|file| {
            std::fs::symlink_metadata(folder.join(file))
                .ok()?
                .modified()
                .ok()
        })
        .max()
        .and_then(seconds)
}

fn seconds(at: SystemTime) -> Option<i64> {
    i64::try_from(at.duration_since(UNIX_EPOCH).ok()?.as_secs()).ok()
}

/// Now, in seconds since 1970.
pub fn now() -> i64 {
    seconds(SystemTime::now()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (path, text) in files {
            let at = dir.path().join(path);
            std::fs::create_dir_all(at.parent().unwrap()).unwrap();
            std::fs::write(at, text).unwrap();
        }
        dir
    }

    fn set_written(path: &Path, at: i64) {
        let time = UNIX_EPOCH + Duration::from_secs(u64::try_from(at).unwrap());
        std::fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(time)
            .unwrap();
    }

    #[test]
    fn a_date_the_data_declares_comes_before_any_file_time() {
        for (declared, at) in [
            (r#""2026-11-08T09:30:00Z""#, 1_794_130_200),
            (r#""2026-11-08T10:30:00+01:00""#, 1_794_130_200),
            (r#""2026-11-08""#, 1_794_096_000),
            ("1794130200", 1_794_130_200),
        ] {
            let dir = folder(&[
                ("index.html", "page"),
                (
                    "data.json",
                    &format!(r#"{{"updated": {declared}, "open": 12}}"#),
                ),
            ]);
            assert_eq!(
                of(dir.path()),
                Some(Freshness {
                    at,
                    source: Source::Declared
                }),
                "{declared}"
            );
        }
    }

    #[test]
    fn data_that_declares_no_readable_date_is_dated_by_its_newest_write() {
        for data in [
            r#"{"open": 12}"#,
            r#"{"updated": "last tuesday"}"#,
            r#"{"updated": null}"#,
            "[1, 2]",
            "not json",
        ] {
            let dir = folder(&[
                ("index.html", "page"),
                ("data.json", data),
                ("data/weeks.json", "[]"),
            ]);
            set_written(&dir.path().join("index.html"), 3_000_000_000);
            set_written(&dir.path().join("data.json"), 1_700_000_000);
            set_written(&dir.path().join("data/weeks.json"), 1_700_000_500);
            assert_eq!(
                of(dir.path()),
                Some(Freshness {
                    at: 1_700_000_500,
                    source: Source::Written
                }),
                "{data}: the page's own time is not its data's"
            );
        }
    }

    #[test]
    fn a_page_with_no_data_is_its_own_data() {
        let dir = folder(&[("index.html", "page")]);
        set_written(&dir.path().join("index.html"), 1_700_000_000);
        assert_eq!(
            of(dir.path()),
            Some(Freshness {
                at: 1_700_000_000,
                source: Source::Written
            })
        );
        assert_eq!(of(folder(&[]).path()), None);
    }

    #[test]
    fn committed_data_is_dated_by_its_commit_and_changed_data_by_its_write() {
        let dir = folder(&[("index.html", "page"), ("data.json", r#"{"open": 1}"#)]);
        let at = dir.path();
        let git = |args: &[&str]| {
            let done = crate::forklock::output(
                std::process::Command::new("git")
                    .args(["-c", "user.name=t", "-c", "user.email=t@t"])
                    .args(args)
                    .current_dir(at)
                    .env("GIT_COMMITTER_DATE", "1600000000 +0000")
                    .env("GIT_AUTHOR_DATE", "1600000000 +0000"),
            )
            .unwrap();
            assert!(done.status.success(), "{args:?}: {done:?}");
        };
        git(&["init", "-q"]);
        git(&["add", "."]);
        git(&["commit", "-q", "-m", "data"]);
        set_written(&at.join("data.json"), 1_700_000_000);
        assert_eq!(
            of(at),
            Some(Freshness {
                at: 1_600_000_000,
                source: Source::Committed
            }),
            "a clone's write time is when it was cloned"
        );

        std::fs::write(at.join("data.json"), r#"{"open": 2}"#).unwrap();
        set_written(&at.join("data.json"), 1_700_000_000);
        assert_eq!(
            of(at),
            Some(Freshness {
                at: 1_700_000_000,
                source: Source::Written
            })
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_linked_data_file_is_not_read_for_its_date() {
        let dir = folder(&[("index.html", "page")]);
        let outside = folder(&[("data.json", r#"{"updated": "2026-11-08"}"#)]);
        std::os::unix::fs::symlink(
            outside.path().join("data.json"),
            dir.path().join("data.json"),
        )
        .unwrap();
        set_written(&dir.path().join("index.html"), 1_700_000_000);
        assert_eq!(
            of(dir.path()),
            Some(Freshness {
                at: 1_700_000_000,
                source: Source::Written
            })
        );
    }

    #[test]
    fn it_is_stale_a_day_after_its_time() {
        let fresh = Freshness {
            at: 1_000_000,
            source: Source::Declared,
        };
        assert_eq!(fresh.stale_at(), 1_000_000 + 86_400);
        assert!(!fresh.stale(1_000_000 + 86_399));
        assert!(fresh.stale(1_000_000 + 86_400));
    }
}
