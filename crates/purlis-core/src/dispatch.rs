// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The dispatch log: who work was actually routed to. A port of the part of
//! `charter/dispatch.py` the README's persona roster reads — [`tally`].
//!
//! `personas/_dispatch/<month>.<device>.jsonl` was appended to, one JSON object a line, one
//! file per month per device ([`crate::machine::log_name`], FD-25). It is **committed**, which
//! is what makes the roster block a fact every engineer sees the same way rather than a reading
//! of one laptop.
//!
//! **Nothing writes it now.** Three kinds of row are read here, and an older build wrote each:
//! a dispatch, when a `Task`/`Agent` call returned, and a resume, when a `SendMessage` resumed
//! one, while a persona could be sent out as a sub-agent (#1451, #1460); and a bare `handoff`
//! row, once the app had opened a handed-off chat (#1457). A persona is a role a chat runs as
//! and never a sub-agent, and every dispatch, handoffs among them, has its own record on the
//! machine that made it ([`crate::dispatchrecord`]). What a log already holds is still read:
//! [`tally`] counts its dispatches, for the README's roster block and `persona stats`, and
//! skips every row that carries an `event`.
//!
//! **A handoff leaves no trace another machine can see** (#1457): its record is this
//! machine's, never committed, and kept 30 days. The bare row was the one committed trace, and
//! it said only when, here or elsewhere, and whether a workspace was made; nothing read it but
//! a total in `persona stats`, which the records already count by persona.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

/// The directory under `personas/` the log lives in — `dispatch.DIR_NAME`.
pub const DIR_NAME: &str = "_dispatch";

pub fn dir(root: &Path) -> PathBuf {
    root.join("personas").join(DIR_NAME)
}

/// Every row of every month file, in filename order — `dispatch._read_all`.
///
/// A file that cannot be read is skipped (Python catches `OSError` per file), and so is a
/// line that is not a JSON object carrying an `agent` or an `event`. Bytes that are not
/// UTF-8 are replaced rather than refused, as Python's `read_text(errors="replace")` does:
/// this log is appended to by a hook, and a half-written line must not cost the whole file.
pub fn rows(root: &Path) -> Vec<serde_json::Value> {
    let d = dir(root);
    let Ok(reader) = std::fs::read_dir(&d) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = reader
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "jsonl"))
        .collect();
    files.sort();
    let mut out = Vec::new();
    for file in files {
        let Ok(bytes) = std::fs::read(&file) else {
            continue;
        };
        let text = String::from_utf8_lossy(&bytes);
        for line in text.lines() {
            let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
            };
            if !value.is_object() {
                continue;
            }
            if truthy(value.get("agent")) || truthy(value.get("event")) {
                out.push(value);
            }
        }
    }
    out
}

/// Python's truth of a JSON value, for the `o.get("agent") or o.get("event")` guard: a
/// missing key, `null`, `false`, `0` and `""` are all false.
pub(crate) fn truthy(value: Option<&serde_json::Value>) -> bool {
    match value {
        None | Some(serde_json::Value::Null) => false,
        Some(serde_json::Value::Bool(b)) => *b,
        Some(serde_json::Value::String(s)) => !s.is_empty(),
        Some(serde_json::Value::Array(a)) => !a.is_empty(),
        Some(serde_json::Value::Object(o)) => !o.is_empty(),
        Some(serde_json::Value::Number(n)) => n.as_f64() != Some(0.0),
    }
}

/// agent → dispatch count — `dispatch.tally()` with no day window.
///
/// **A resume row carries an agent too, and must not land here.** The column is read as
/// "times work was dispatched to it", and it is what personas get retired on — so a row
/// with an `event` is excluded, whatever else it holds.
///
/// The agent is taken as the string it is. A row whose `agent` is a number is truthy to
/// Python and lands in its `Counter` under that value; here it is skipped, because a name
/// that is not a name matches no persona and could only ever be counted against nothing.
pub fn tally(root: &Path) -> BTreeMap<String, u64> {
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    for row in rows(root) {
        if truthy(row.get("event")) {
            continue;
        }
        if let Some(agent) = row.get("agent").and_then(serde_json::Value::as_str)
            && !agent.is_empty()
        {
            *counts.entry(agent.to_string()).or_default() += 1;
        }
    }
    counts
}

/// This machine's name as a filename part — `dispatch._host`: the first label of the host
/// name, everything outside `[A-Za-z0-9_-]` removed, at most 32 characters, `unknown` for
/// nothing. One file per machine is what keeps two laptops from conflicting on one line.
pub fn host() -> String {
    #[cfg(unix)]
    let raw = rustix::system::uname()
        .nodename()
        .to_string_lossy()
        .into_owned();
    #[cfg(not(unix))]
    let raw = String::new();
    host_of(&raw)
}

/// [`host`]'s rule, for a name already in hand.
pub fn host_of(raw: &str) -> String {
    let first = raw.split('.').next().unwrap_or_default();
    let kept: String = first
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'))
        .take(32)
        .collect();
    if kept.is_empty() {
        "unknown".to_string()
    } else {
        kept
    }
}

/// Append one row, keys sorted, as `json.dumps(…, sort_keys=True)` writes it: the shared body
/// of the project's append-only logs (the skill log, the piece log, `changes/`), as it was of
/// the three `dispatch.record*` writers. `None` when containment refuses the path or the write
/// fails.
///
/// **Opened `O_NOFOLLOW`** (#420): containment answers for the path when it is asked, and a
/// link swapped in at the log between that answer and the open would otherwise be followed.
/// A log that is a link is refused wherever it points; `std` opens `O_CLOEXEC` already.
pub fn append(path: &Path, root: &Path, row: &serde_json::Value) -> Option<PathBuf> {
    try_append(path, root, row).ok()
}

/// [`append`], answering why a row was not written: what containment refused, or the
/// filesystem's refusal naming the log ([`crate::rewrite::refused_at`], #1359).
pub fn try_append(path: &Path, root: &Path, row: &serde_json::Value) -> io::Result<PathBuf> {
    use std::io::Write;
    crate::contain::writable(root, path)
        .map_err(|why| io::Error::new(io::ErrorKind::PermissionDenied, why.to_string()))?;
    let dir = path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{} has no directory", path.display()),
        )
    })?;
    crate::rewrite::create_dir_all(dir)?;
    let mut options = std::fs::OpenOptions::new();
    options.append(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o644);
    }
    let mut file = crate::contain::nofollow(&mut options)
        .open(path)
        .map_err(crate::rewrite::refused_at(path))?;
    let line = format!("{}\n", crate::pyjson::dumps_sorted(row));
    file.write_all(line.as_bytes())
        .map_err(crate::rewrite::refused_at(path))?;
    Ok(path.to_path_buf())
}

/// `isoformat(timespec="seconds")` of a UTC instant.
pub fn stamp(when: chrono::DateTime<chrono::Utc>) -> String {
    when.format("%Y-%m-%dT%H:%M:%S+00:00").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plane(lines: &[&str]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let d = super::dir(dir.path());
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("2026-03.host.jsonl"), lines.join("\n")).unwrap();
        dir
    }

    #[test]
    fn a_dispatch_is_counted_and_a_resume_is_not() {
        // A resume row carries an agent; counting it would inflate the column personas are
        // retired on, which is the one thing this tally decides.
        let dir = plane(&[
            r#"{"agent": "devops", "ts": "2026-03-02T09:33:00+00:00"}"#,
            r#"{"agent": "devops", "event": "resume", "ts": "2026-03-02T10:00:00+00:00"}"#,
            r#"{"agent": "Explore", "ts": "2026-03-02T11:00:00+00:00"}"#,
        ]);
        let counts = tally(dir.path());

        assert_eq!(counts.get("devops"), Some(&1));
        assert_eq!(counts.get("Explore"), Some(&1));
        assert_eq!(counts.len(), 2);
    }

    #[test]
    fn a_line_that_is_not_a_row_is_skipped_rather_than_ending_the_file() {
        // The log is appended to by a hook: a truncated last line must not lose the rows
        // above it, and a blank line is the same non-row as a broken one.
        let dir = plane(&[
            r#"{"agent": "devops"}"#,
            "",
            "{not json",
            r#"["agent"]"#,
            r#"{"ts": "2026-03-02T09:33:00+00:00"}"#,
            r#"{"agent": "steward"}"#,
        ]);

        assert_eq!(tally(dir.path()).len(), 2);
    }

    #[test]
    fn a_plane_with_no_log_has_no_dispatches_rather_than_an_error() {
        let dir = tempfile::tempdir().unwrap();

        assert!(tally(dir.path()).is_empty());
    }

    #[test]
    fn a_handoff_row_an_older_build_wrote_is_not_a_dispatch() {
        // #1457: nothing writes the bare handoff row now, and one already committed names no
        // persona, so it is never counted against one.
        let dir = plane(&[
            r#"{"created": true, "event": "handoff", "placement": "elsewhere", "ts": "2026-05-04T11:32:17+00:00"}"#,
            r#"{"agent": "devops", "ts": "2026-03-02T09:33:00+00:00"}"#,
        ]);

        assert_eq!(rows(dir.path()).len(), 2, "still read");
        assert_eq!(
            tally(dir.path()).into_iter().collect::<Vec<_>>(),
            [("devops".to_owned(), 1)]
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn a_row_the_filesystem_refuses_names_the_log_it_was_appending_to() {
        // #1359: a refused append said only that the row was not added.
        let dir = tempfile::tempdir().unwrap();
        let log = super::dir(dir.path()).join("2026-05.box.jsonl");
        std::fs::create_dir_all(log.parent().unwrap()).unwrap();
        let _frozen = crate::rewrite::frozen::Frozen::at(log.parent().unwrap());

        let refused =
            try_append(&log, dir.path(), &serde_json::json!({"agent": "devops"})).unwrap_err();

        crate::rewrite::frozen::names(&refused, &log);
    }

    #[cfg(unix)]
    #[test]
    fn a_log_that_is_a_link_is_refused_and_what_it_points_at_is_left_alone() {
        // A link that stays INSIDE the plane's data directories passes containment, so only
        // the open's own `O_NOFOLLOW` stands between the row and the file the link names (#420).
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("charter.toml"), "").unwrap();
        let victim = dir.path().join("personas/devops/memory/fact.md");
        std::fs::create_dir_all(victim.parent().unwrap()).unwrap();
        std::fs::write(&victim, "PRECIOUS\n").unwrap();
        let log = super::dir(dir.path()).join("2026-05.box.jsonl");
        std::fs::create_dir_all(log.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(&victim, &log).unwrap();

        assert!(try_append(&log, dir.path(), &serde_json::json!({"agent": "devops"})).is_err());
        assert_eq!(std::fs::read_to_string(&victim).unwrap(), "PRECIOUS\n");
        assert!(std::fs::symlink_metadata(&log).unwrap().is_symlink());
    }

    #[test]
    fn the_host_is_one_safe_label() {
        assert_eq!(host_of("Aarons-MacBook.local"), "Aarons-MacBook");
        assert_eq!(host_of("bad host!"), "badhost");
        assert_eq!(host_of(""), "unknown");
        assert_eq!(host_of(&"x".repeat(40)).len(), 32);
    }
}
