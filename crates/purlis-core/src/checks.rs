// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A branch's check results, read from SARIF 2.1.0, kept per branch folder, and placed on a
//! comparison (ADR 0114, #5407).
//!
//! **purlis runs no check here and has no parser for any one tool.** `purlis check report` hands a
//! SARIF file over; the store (`docs/plane-format.md`) keeps what each tool last said, with the
//! commit the branch was at; [`on_file`] places it on one file's hunks. Every result lands in one of
//! its three lists, so nothing a tool said is dropped.

use std::io::Read;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_sarif::sarif;

use crate::files::{Branch, Hunk};
use crate::worktree::git;

/// The one SARIF version purlis reads.
pub const VERSION: &str = "2.1.0";

/// How much a result matters, in SARIF's own four words.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Note,
    None,
}

/// One result of a check, as purlis keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    /// The file, relative to the branch's folder where it could be placed there, else as the
    /// tool spelled it; `None` for a result that names no file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Its first line, from 1; `None` for a result on the file as a whole.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    /// Its last line, where it runs past its first.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_line: Option<u32>,
    pub severity: Severity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule: Option<String>,
    pub message: String,
}

/// What one tool said in one SARIF file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolResults {
    pub tool: String,
    pub diagnostics: Vec<Diagnostic>,
}

/// The results in `sarif`, tool by tool, or what is wrong with the file. `folder` is the branch's
/// folder, which an absolute file URI is read against; a tool may name files either way.
pub fn read_sarif(sarif: &[u8], folder: Option<&Path>) -> Result<Vec<ToolResults>, String> {
    let value: serde_json::Value =
        serde_json::from_slice(sarif).map_err(|e| format!("it is not JSON ({e})"))?;
    match value.get("version").and_then(serde_json::Value::as_str) {
        Some(VERSION) => {}
        Some(other) => {
            return Err(format!(
                "it says SARIF version {other}, and purlis reads {VERSION}"
            ));
        }
        None => return Err(format!("it has no `version` saying SARIF {VERSION}")),
    }
    if !value.get("runs").is_some_and(serde_json::Value::is_array) {
        return Err("its `runs` is missing or is not a list".to_owned());
    }
    let log: sarif::Sarif = serde_json::from_value(value)
        .map_err(|e| format!("it does not follow SARIF {VERSION} ({e})"))?;
    Ok(log.runs.iter().map(|run| run_read(run, folder)).collect())
}

/// One run's tool and every result it holds, one for each location a result names.
fn run_read(run: &sarif::Run, folder: Option<&Path>) -> ToolResults {
    let mut diagnostics = Vec::new();
    for result in run.results.iter().flatten() {
        let severity = severity_of(run, result);
        let rule = result.rule_id.clone();
        let message = result
            .message
            .text
            .clone()
            .or_else(|| result.message.markdown.clone())
            .or_else(|| result.message.id.clone())
            .unwrap_or_default();
        let located: Vec<&sarif::PhysicalLocation> = result
            .locations
            .iter()
            .flatten()
            .filter_map(|one| one.physical_location.as_ref())
            .collect();
        if located.is_empty() {
            diagnostics.push(Diagnostic {
                path: None,
                line: None,
                end_line: None,
                severity,
                rule: rule.clone(),
                message: message.clone(),
            });
        }
        for one in located {
            let region = one.region.as_ref();
            let line = region
                .and_then(|r| r.start_line)
                .and_then(|n| u32::try_from(n).ok());
            let end_line = region
                .and_then(|r| r.end_line)
                .and_then(|n| u32::try_from(n).ok())
                .filter(|end| Some(*end) != line);
            diagnostics.push(Diagnostic {
                path: one
                    .artifact_location
                    .as_ref()
                    .and_then(|at| path_of(at, run, folder)),
                line,
                end_line,
                severity,
                rule: rule.clone(),
                message: message.clone(),
            });
        }
    }
    ToolResults {
        tool: run.tool.driver.name.clone(),
        diagnostics,
    }
}

/// SARIF's rule (§3.27.10): the result's level; a result that is not a failure has none; else its
/// rule's default; else `warning`.
fn severity_of(run: &sarif::Run, result: &sarif::Result) -> Severity {
    use sarif::{ResultKind, ResultLevel};
    let named = |level: &ResultLevel| match level {
        ResultLevel::Error => Severity::Error,
        ResultLevel::Warning => Severity::Warning,
        ResultLevel::Note => Severity::Note,
        ResultLevel::None => Severity::None,
    };
    if let Some(level) = &result.level {
        return named(level);
    }
    if result
        .kind
        .as_ref()
        .is_some_and(|kind| !matches!(kind, ResultKind::Fail))
    {
        return Severity::None;
    }
    let rule = run
        .tool
        .driver
        .rules
        .iter()
        .flatten()
        .find(|rule| result.rule_id.as_deref() == Some(rule.id.as_str()));
    rule.and_then(|rule| rule.default_configuration.as_ref())
        .and_then(|config| config.level.as_ref())
        .and_then(|level| serde_json::from_value::<ResultLevel>(level.clone()).ok())
        .map_or(Severity::Warning, |level| named(&level))
}

/// The file an artifact location names: relative to the branch's top where it can be placed there,
/// else as the tool spelled it, without its `file://`.
fn path_of(
    at: &sarif::ArtifactLocation,
    run: &sarif::Run,
    folder: Option<&Path>,
) -> Option<String> {
    let uri = decoded(at.uri.as_deref()?);
    if let Some(absolute) = absolute_of(&uri) {
        return Some(under(absolute, folder));
    }
    let base = at
        .uri_base_id
        .as_ref()
        .and_then(|id| run.original_uri_base_ids.as_ref()?.get(id))
        .and_then(|base| base.uri.as_deref())
        .map(decoded)
        .unwrap_or_default();
    let relative = uri.strip_prefix("./").unwrap_or(&uri);
    let Some(absolute) = absolute_of(&base) else {
        return Some(format!("{base}{relative}"));
    };
    // A base on the machine the tool ran on: the part under this branch's folder, if any.
    let based = under(absolute, folder);
    if based.starts_with('/') {
        return Some(relative.to_owned());
    }
    Some(format!("{}/{relative}", based.trim_end_matches('/')))
}

/// A URI's own path where it is absolute: `file:///a/b` and `/a/b` are; `src/a.ts` is not.
fn absolute_of(uri: &str) -> Option<&str> {
    let path = uri
        .strip_prefix("file://localhost")
        .or_else(|| uri.strip_prefix("file://"))
        .unwrap_or(uri);
    path.starts_with('/').then_some(path)
}

/// `absolute`, relative to `folder` if it is under it, else unchanged.
fn under(absolute: &str, folder: Option<&Path>) -> String {
    folder
        .and_then(|folder| Path::new(absolute).strip_prefix(folder).ok())
        .map_or_else(
            || absolute.to_owned(),
            |rest| rest.to_string_lossy().into_owned(),
        )
}

/// A URI with its `%xx` read: what the file is called on disk.
fn decoded(uri: &str) -> String {
    percent_encoding::percent_decode_str(uri)
        .decode_utf8_lossy()
        .into_owned()
}

/// The largest SARIF file purlis reads. A report is results, so this is many thousands of them, and
/// past it the app is not asked to hold a file a chat named.
pub const MOST_SARIF_BYTES: u64 = 32 * 1024 * 1024;

/// The folder under the workspace's `pieces/` that holds the results.
const STORE_DIR: &str = "checks";

/// Where a branch folder's results are kept. The clone's own record is beside the piece folder,
/// as [`crate::pieces::seen_path`]'s is, so no piece's name can collide with it.
pub fn store_path(plane: &Path, ws: &str, repo: &str, piece: Option<&str>) -> PathBuf {
    let base = crate::pieces::dir_for(plane, ws).join(STORE_DIR);
    match piece {
        None => base.join(format!("{repo}.json")),
        Some(piece) => base.join(repo).join(format!("{piece}.json")),
    }
}

/// The record's one shape on disk.
#[derive(Debug, Default, Serialize, Deserialize)]
struct Record {
    sets: Vec<Set>,
}

/// What a branch folder's results are, or why they cannot be read. None reported is none.
pub fn read_sets(
    plane: &Path,
    ws: &str,
    repo: &str,
    piece: Option<&str>,
) -> Result<Vec<Set>, String> {
    let path = store_path(plane, ws, repo, piece);
    crate::contain::writable(plane, &path).map_err(|e| e.to_string())?;
    let text = match std::fs::read(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("purlis could not read {}: {e}", path.display())),
    };
    serde_json::from_slice::<Record>(&text)
        .map(|record| record.sets)
        .map_err(|e| {
            format!(
                "purlis could not read {} ({e}): delete it, and report the results again",
                path.display()
            )
        })
}

/// What [`report`] recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reported {
    /// The record.
    pub path: PathBuf,
    pub branch: String,
    pub head: Option<String>,
    /// Each tool, with how many results it has.
    pub tools: Vec<(String, usize)>,
}

/// Records the SARIF file at `file` as `branch`'s results, at the commit the branch is at now. A
/// file that is not SARIF 2.1.0 is refused, saying what is wrong and what to do, and the record is
/// left as it was.
pub fn report(
    plane: &Path,
    branch: Branch<'_>,
    file: &Path,
    now: DateTime<Utc>,
) -> Result<Reported, String> {
    let piece = branch.piece;
    let folder = crate::files::folder(plane, branch, "").map_err(|e| e.to_string())?;
    let refused = |why: String| {
        format!(
            "{} was not recorded: {why}. Have the tool write SARIF {VERSION}, then report it again",
            file.display()
        )
    };
    let bytes = bounded(file).map_err(&refused)?;
    let results = read_sarif(&bytes, Some(&folder)).map_err(&refused)?;
    if results.is_empty() {
        return Err(refused("it holds no run, so no tool's results".to_owned()));
    }
    let head = line_of(&folder, &["rev-parse", "--verify", "--quiet", "HEAD"]);
    let name = line_of(&folder, &["symbolic-ref", "--short", "--quiet", "HEAD"])
        .unwrap_or_else(|| "(detached)".to_owned());
    let path = store_path(plane, branch.ws, branch.repo, piece);
    let stamp = now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let mut fresh: Vec<Set> = Vec::new();
    for one in results {
        // A file may hold several runs of one tool: together they are what it said.
        if let Some(same) = fresh.iter_mut().find(|set| set.tool == one.tool) {
            same.diagnostics.extend(one.diagnostics);
        } else {
            fresh.push(Set {
                tool: one.tool,
                branch: name.clone(),
                head: head.clone(),
                reported_at: stamp.clone(),
                diagnostics: one.diagnostics,
            });
        }
    }
    let tools = fresh
        .iter()
        .map(|set| (set.tool.clone(), set.diagnostics.len()))
        .collect();
    let held = crate::piececlaim::locked(plane, branch.ws)?;
    let mut sets = read_sets(plane, branch.ws, branch.repo, piece)?;
    sets.retain(|kept| !fresh.iter().any(|set| set.tool == kept.tool));
    sets.extend(fresh);
    written(plane, branch.ws, &path, &Record { sets })?;
    drop(held);
    Ok(Reported {
        path,
        branch: name,
        head,
        tools,
    })
}

/// The record put in place whole, through the workspace's folder held by descriptor: a link at the
/// record, or at a folder down to it, is refused and never written through.
fn written(plane: &Path, ws: &str, path: &Path, record: &Record) -> Result<(), String> {
    crate::contain::writable(plane, path).map_err(|e| e.to_string())?;
    let mut text = serde_json::to_vec_pretty(record).map_err(|e| e.to_string())?;
    text.push(b'\n');
    let said = |e: std::io::Error| format!("purlis could not write {}: {e}", path.display());
    #[cfg(unix)]
    {
        let (held, name) = crate::held::files::made_for(plane, ws, path).map_err(said)?;
        crate::held::files::replace(&held, &name, &text, path).map_err(said)
    }
    #[cfg(not(unix))]
    {
        let _ = ws;
        let dir = path.parent().ok_or_else(|| "no folder".to_owned())?;
        std::fs::create_dir_all(dir).map_err(said)?;
        crate::rewrite::replace(dir, path, &text, crate::rewrite::Mode::Kept).map_err(said)
    }
}

/// One line of git's answer in `folder`, through the hardened runner; `None` where it has none.
fn line_of(folder: &Path, args: &[&str]) -> Option<String> {
    let run = git::run(folder, args, git::READ).ok()?;
    run.ok()
        .then(|| run.out.trim().to_owned())
        .filter(|line| !line.is_empty())
}

/// The file's bytes, up to [`MOST_SARIF_BYTES`]. Only a plain file: the app reads this for a chat,
/// and a FIFO at the path must not hold it.
fn bounded(file: &Path) -> Result<Vec<u8>, String> {
    let mut open = std::fs::OpenOptions::new();
    open.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        open.custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC);
    }
    let opened = open
        .open(file)
        .map_err(|e| format!("it cannot be read ({e})"))?;
    let meta = opened
        .metadata()
        .map_err(|e| format!("it cannot be read ({e})"))?;
    if !meta.is_file() {
        return Err("it is not a file".to_owned());
    }
    if meta.len() > MOST_SARIF_BYTES {
        return Err(format!(
            "it is {} bytes, and purlis reads at most {MOST_SARIF_BYTES}",
            meta.len()
        ));
    }
    let mut bytes = Vec::new();
    opened
        .take(MOST_SARIF_BYTES)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("it cannot be read ({e})"))?;
    Ok(bytes)
}

/// The folder of the branch called `name` in workspace `ws`: `(repo, piece)`, with no piece for a
/// clone that has it checked out. Asked of `repo` alone when one is named, else of each clone; a
/// name two clones have is refused, asking which.
pub fn branch_named(
    plane: &Path,
    ws: &str,
    repo: Option<&str>,
    name: &str,
) -> Result<(String, Option<String>), String> {
    let repos = match repo {
        Some(repo) => vec![repo.to_owned()],
        None => clones_of(plane, ws),
    };
    let mut found = Vec::new();
    for repo in &repos {
        if let Some(piece) = crate::worktree::list(plane, ws, repo)
            .map_err(|e| e.to_string())?
            .into_iter()
            .find(|piece| piece.branch.as_deref() == Some(name))
        {
            found.push((repo.clone(), Some(piece.piece)));
            continue;
        }
        let clone = plane.join("workspaces").join(ws).join(repo);
        if line_of(&clone, &["symbolic-ref", "--short", "--quiet", "HEAD"]).as_deref() == Some(name)
        {
            found.push((repo.clone(), None));
        }
    }
    let mut found = found.into_iter();
    match (found.next(), found.next()) {
        (Some(only), None) => Ok(only),
        (Some(_), Some(_)) => Err(format!(
            "two repos of workspace '{ws}' have a branch '{name}': name one with --repo"
        )),
        (None, _) => Err(format!(
            "no folder of workspace '{ws}' has the branch '{name}' checked out: run `purlis worktree list` to see them, and report it for one of those"
        )),
    }
}

/// The clones of workspace `ws`, by name.
fn clones_of(plane: &Path, ws: &str) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(plane.join("workspaces").join(ws)) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .filter(|entry| crate::worktree::is_own_git_dir(&entry.path().join(".git")))
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| crate::contain::repo_name_ok(name))
        .collect();
    names.sort();
    names
}

/// A result placed on one file of a comparison.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Placed {
    pub tool: String,
    pub out_of_date: bool,
    pub diagnostic: Diagnostic,
}

/// One set of results, as the window says it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetSaid {
    pub tool: String,
    pub head: Option<String>,
    pub out_of_date: bool,
    pub results: usize,
}

/// A set of results as the store keeps it: one per tool, per branch folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Set {
    pub tool: String,
    pub branch: String,
    /// The commit the branch was at when these were reported; `None` for a branch with no commit.
    pub head: Option<String>,
    pub reported_at: String,
    pub diagnostics: Vec<Diagnostic>,
}

/// A branch's results as one file of its comparison shows them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OnFile {
    pub sets: Vec<SetSaid>,
    /// On a line a hunk shows on the head's side.
    pub on_lines: Vec<Placed>,
    /// In this file, on no line a hunk shows, or on no line at all.
    pub outside: Vec<Placed>,
    /// In any other file, or in none.
    pub elsewhere: Vec<Placed>,
}

/// `sets` placed on the file at `path`, whose comparison has `hunks`, with the branch at `head`.
/// **Every result lands in one of the three lists**: nothing a tool said is dropped. A result that
/// a hunk's lines touch only in part is on those lines.
pub fn on_file(sets: &[Set], path: &str, hunks: &[Hunk], head: Option<&str>) -> OnFile {
    let mut out = OnFile::default();
    for set in sets {
        let out_of_date = set.head.as_deref() != head;
        out.sets.push(SetSaid {
            tool: set.tool.clone(),
            head: set.head.clone(),
            out_of_date,
            results: set.diagnostics.len(),
        });
        for diagnostic in &set.diagnostics {
            let list = if diagnostic.path.as_deref() != Some(path) {
                &mut out.elsewhere
            } else if shown_by(diagnostic, hunks) {
                &mut out.on_lines
            } else {
                &mut out.outside
            };
            list.push(Placed {
                tool: set.tool.clone(),
                out_of_date,
                diagnostic: diagnostic.clone(),
            });
        }
    }
    out
}

/// Whether any line of `diagnostic` is one a hunk shows on the head's side. git's `+n,0` has none.
fn shown_by(diagnostic: &Diagnostic, hunks: &[Hunk]) -> bool {
    let Some(first) = diagnostic.line else {
        return false;
    };
    let last = diagnostic.end_line.unwrap_or(first);
    hunks.iter().any(|hunk| {
        hunk.new_lines > 0
            && first < hunk.new_start.saturating_add(hunk.new_lines)
            && last >= hunk.new_start
    })
}
