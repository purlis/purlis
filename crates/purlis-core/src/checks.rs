// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A branch's check results, read from SARIF 2.1.0, kept per branch folder, and placed on a
//! comparison (ADR 0116, #5407).
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
            .map(held_message)
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
    /// Each Check reporter whose last answer could not be recorded (#5410). Absent in a record
    /// with none, so one written before it reads the same.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    faults: Vec<Fault>,
}

/// **A Check reporter whose last answer could not be recorded** (ADR 0116, #5410): what the
/// window draws as a Notice naming it. The results recorded before stay; its next answer that is
/// recorded takes this away.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fault {
    /// The extension, by its id.
    pub extension: String,
    /// What was wrong, in purlis's sentence, naming the extension.
    pub said: String,
    pub reported_at: String,
}

/// What a branch folder's results are, or why they cannot be read. None reported is none.
pub fn read_sets(
    plane: &Path,
    ws: &str,
    repo: &str,
    piece: Option<&str>,
) -> Result<Vec<Set>, String> {
    read_record(plane, ws, repo, piece).map(|record| record.sets)
}

/// The record of a branch folder, or why it cannot be read. None reported is an empty one.
fn read_record(plane: &Path, ws: &str, repo: &str, piece: Option<&str>) -> Result<Record, String> {
    let path = store_path(plane, ws, repo, piece);
    crate::contain::writable(plane, &path).map_err(|e| e.to_string())?;
    let text = match crate::contain::read_no_link(plane, &path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Record::default()),
        Err(e) => return Err(format!("purlis could not read {}: {e}", path.display())),
    };
    serde_json::from_slice::<Record>(&text).map_err(|e| {
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
    let folder = crate::files::folder(plane, branch, "").map_err(|e| e.to_string())?;
    let results = results_in(file, &folder).map_err(|why| not_recorded(file, &why))?;
    recorded(plane, branch, &folder, results, None, now)
}

/// [`report`] for a chat, by the app: `relative` is a file of the branch's folder, already confined
/// by [`crate::files::named`], and **it is opened following no link**. A link at it, which that
/// check lets through, would have the app read for a chat a file the chat's sandbox denies it.
pub fn report_inside(
    plane: &Path,
    branch: Branch<'_>,
    relative: &str,
    now: DateTime<Utc>,
) -> Result<Reported, String> {
    let folder = crate::files::folder(plane, branch, "").map_err(|e| e.to_string())?;
    let file = folder.join(relative);
    let results = crate::contain::open_no_link(&folder, &file)
        .map_err(|e| format!("it cannot be read ({e})"))
        .and_then(read_bounded)
        .and_then(|bytes| results_of(&bytes, &folder))
        .map_err(|why| not_recorded(&file, &why))?;
    recorded(plane, branch, &folder, results, None, now)
}

/// **A Check reporter's answer, recorded for `branch`** (ADR 0116, #5410): `answered` is the SARIF
/// it printed, or why it gave none. SARIF 2.1.0 is recorded as [`report`] records a file, and
/// takes away the reporter's fault. Anything else is kept as its fault, naming it, and refused
/// in the same sentence; the results recorded before stay, so the review goes on.
pub fn report_converted(
    plane: &Path,
    branch: Branch<'_>,
    reporter: &str,
    answered: Result<Vec<u8>, String>,
    now: DateTime<Utc>,
) -> Result<Reported, String> {
    let folder = crate::files::folder(plane, branch, "").map_err(|e| e.to_string())?;
    let results = answered.and_then(|sarif| {
        results_of(&sarif, &folder).map_err(|why| {
            format!(
                "The Check reporter '{reporter}' answered something purlis cannot read as SARIF \
                 {VERSION}: {why}"
            )
        })
    });
    match results {
        Ok(results) => recorded(plane, branch, &folder, results, Some(reporter), now),
        Err(why) => {
            let said = format!(
                "{why}. Nothing it said was recorded, and the results before stay. Run the check \
                 again once the extension is fixed"
            );
            faulted(plane, branch, reporter, &said, now)?;
            Err(said)
        }
    }
}

/// **What a Check reporter answers for a check's `output` and exit `status`** (ADR 0116, #5410),
/// asked through `executor` as its own process, never inside the window.
///
/// The outer `Err` is a refusal before anything ran: no extension by that name, one not approved,
/// turned off, changed since its yes, or one that does not ask for the capability. Nothing is
/// recorded for it. The inner result is the reporter's: the SARIF it printed when it exited 0,
/// or why it gave none, which [`report_converted`] keeps as its fault.
pub fn converted(
    executor: &crate::executor::Executor,
    config_root: &Path,
    project: &crate::extension::project::Choices,
    reporter: &str,
    output: &[u8],
    status: i32,
) -> Result<Result<Vec<u8>, String>, String> {
    let found = crate::executor::cleared(config_root, executor.built_in(), reporter)?;
    if !found.manifest.reports_checks() {
        return Err(format!(
            "'{reporter}' does not ask for the capability \"check-reporter\", so purlis does not \
             hand it a check's output. Nothing was recorded"
        ));
    }
    let output = String::from_utf8_lossy(output);
    let ran = match executor.check(config_root, project, reporter, &output, status) {
        Ok(ran) => ran,
        Err(why) => {
            return Ok(Err(format!(
                "The Check reporter '{reporter}' gave no answer: {}",
                why.trim_end_matches('.')
            )));
        }
    };
    if ran.status != 0 {
        let said = String::from_utf8_lossy(&ran.stderr);
        let said = said.trim();
        let last = if said.is_empty() {
            String::new()
        } else {
            format!(
                ", saying: {}",
                crate::shown::readable(said, crate::executor::MOST_STDERR_BYTES)
            )
        };
        return Ok(Err(format!(
            "The Check reporter '{reporter}' exited with status {}{last}",
            ran.status
        )));
    }
    Ok(Ok(ran.stdout))
}

/// [`converted`] and then [`report_converted`] for a chat, by the app (#5410): `relative` is the
/// check's output, a file of the branch's folder already confined by [`crate::files::named`], and
/// **it is opened following no link**, as [`report_inside`] opens a SARIF file. The reporter runs
/// as the app's own process, as every extension the app asks does, never in the chat's sandbox.
#[allow(
    clippy::too_many_arguments,
    reason = "the branch, the file, the reporter and how to ask it; a struct for one caller would only rename them"
)]
pub fn report_converted_inside(
    plane: &Path,
    branch: Branch<'_>,
    relative: &str,
    reporter: &str,
    status: i32,
    executor: &crate::executor::Executor,
    config_root: &Path,
    now: DateTime<Utc>,
) -> Result<Reported, String> {
    let folder = crate::files::folder(plane, branch, "").map_err(|e| e.to_string())?;
    let file = folder.join(relative);
    let output = crate::contain::open_no_link(&folder, &file)
        .map_err(|e| format!("it cannot be read ({e})"))
        .and_then(|opened| read_bounded_to(opened, crate::executor::MOST_CHECK_OUTPUT_BYTES as u64))
        .map_err(|why| {
            format!(
                "{} was not handed to '{reporter}': {why}. Nothing was recorded",
                file.display()
            )
        })?;
    let project = crate::extension::project::Choices::read(plane);
    let answered = converted(executor, config_root, &project, reporter, &output, status)?;
    report_converted(plane, branch, reporter, answered, now)
}

/// `reporter`'s fault kept in `branch`'s record, in place of any it had.
fn faulted(
    plane: &Path,
    branch: Branch<'_>,
    reporter: &str,
    said: &str,
    now: DateTime<Utc>,
) -> Result<(), String> {
    let path = store_path(plane, branch.ws, branch.repo, branch.piece);
    let held = crate::piececlaim::locked(plane, branch.ws)?;
    let mut record = read_record(plane, branch.ws, branch.repo, branch.piece)?;
    record.faults.retain(|kept| kept.extension != reporter);
    record.faults.push(Fault {
        extension: reporter.to_owned(),
        said: said.to_owned(),
        reported_at: now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    });
    written(plane, branch.ws, &path, &record)?;
    drop(held);
    Ok(())
}

/// `results` kept as the branch's, each tool's replacing its own and leaving the others; and,
/// where a Check reporter answered them, its fault taken away.
fn recorded(
    plane: &Path,
    branch: Branch<'_>,
    folder: &Path,
    results: Vec<ToolResults>,
    reporter: Option<&str>,
    now: DateTime<Utc>,
) -> Result<Reported, String> {
    let piece = branch.piece;
    let head = line_of(folder, &["rev-parse", "--verify", "--quiet", "HEAD"]);
    let name = line_of(folder, &["symbolic-ref", "--short", "--quiet", "HEAD"])
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
    let mut record = read_record(plane, branch.ws, branch.repo, piece)?;
    record
        .sets
        .retain(|kept| !fresh.iter().any(|set| set.tool == kept.tool));
    record.sets.extend(fresh);
    if let Some(reporter) = reporter {
        record.faults.retain(|kept| kept.extension != reporter);
    }
    written(plane, branch.ws, &path, &record)?;
    drop(held);
    Ok(Reported {
        path,
        branch: name,
        head,
        tools,
    })
}

/// The sentence a bad file is refused in: what is wrong with it, and what to do.
pub fn not_recorded(file: &Path, why: &str) -> String {
    format!(
        "{} was not recorded: {why}. Have the tool write SARIF {VERSION}, then report it again",
        file.display()
    )
}

/// The results in the SARIF file at `file`, for a branch whose folder is `folder`, or what is wrong
/// with it: what [`report`] records, and what a command checks before handing a file to the app.
pub fn results_in(file: &Path, folder: &Path) -> Result<Vec<ToolResults>, String> {
    results_of(&bounded(file)?, folder)
}

fn results_of(bytes: &[u8], folder: &Path) -> Result<Vec<ToolResults>, String> {
    let results = read_sarif(bytes, Some(folder))?;
    if results.is_empty() {
        return Err("it holds no run, so no tool's results".to_owned());
    }
    Ok(results)
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
    bounded_to(file, MOST_SARIF_BYTES)
}

/// A check's output in the file at `file`, to hand a Check reporter: at most
/// [`crate::executor::MOST_CHECK_OUTPUT_BYTES`], and only a plain file, as a SARIF file is read.
pub fn output_in(file: &Path) -> Result<Vec<u8>, String> {
    bounded_to(file, crate::executor::MOST_CHECK_OUTPUT_BYTES as u64)
}

/// The plain file at `file`, up to `most` bytes.
fn bounded_to(file: &Path, most: u64) -> Result<Vec<u8>, String> {
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
    read_bounded_to(opened, most)
}

/// What an open file holds, up to [`MOST_SARIF_BYTES`]; only a plain file.
fn read_bounded(opened: std::fs::File) -> Result<Vec<u8>, String> {
    read_bounded_to(opened, MOST_SARIF_BYTES)
}

/// What an open file holds, up to `most` bytes; only a plain file.
fn read_bounded_to(opened: std::fs::File, most: u64) -> Result<Vec<u8>, String> {
    let meta = opened
        .metadata()
        .map_err(|e| format!("it cannot be read ({e})"))?;
    if !meta.is_file() {
        return Err("it is not a file".to_owned());
    }
    if meta.len() > most {
        return Err(format!(
            "it is {} bytes, and purlis reads at most {most}",
            meta.len()
        ));
    }
    let mut bytes = Vec::new();
    opened
        .take(most)
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
        let clone = crate::repofolder::folder(plane, ws, repo).map_err(|e| e.to_string())?;
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

/// The clones of workspace `ws`, by name, in either layout (ADR 0114).
fn clones_of(plane: &Path, ws: &str) -> Vec<String> {
    crate::repofolder::repos(plane, ws)
        .map(|rows| rows.into_iter().map(|row| row.repo.name).collect())
        .unwrap_or_default()
}

/// A result placed on one file of a comparison.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Placed {
    pub tool: String,
    pub out_of_date: bool,
    /// The first of its lines a hunk shows, where one does: where the gutter marks it. A result
    /// that only runs into a hunk starts on a line the comparison does not draw.
    pub shown_at: Option<u32>,
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

/// The most results one list of [`OnFile`] holds. Past it they are counted and said, never dropped
/// without a word: a report of many thousands must not stop the file's diff from being drawn.
pub const MOST_LISTED: usize = 200;

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
    /// How many results past [`MOST_LISTED`] each list left out.
    pub more_on_lines: usize,
    pub more_outside: usize,
    pub more_elsewhere: usize,
    /// Why the results could not be read, where they could not. The diff is drawn regardless.
    pub trouble: Option<String>,
    /// Each Check reporter whose last answer could not be recorded (#5410), for the whole branch.
    pub faults: Vec<Fault>,
}

/// `text` held to [`MOST_MESSAGE_CHARS`], on a character.
fn held_message(text: String) -> String {
    match text.char_indices().nth(MOST_MESSAGE_CHARS) {
        Some((at, _)) => format!("{}…", &text[..at]),
        None => text,
    }
}

/// The longest message kept: a tool's explanation, and past this it is cut, saying so with `…`.
const MOST_MESSAGE_CHARS: usize = 4000;

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
            let ours = diagnostic.path.as_deref() == Some(path);
            let shown_at = if ours {
                shown_by(diagnostic, hunks)
            } else {
                None
            };
            let (list, more) = if !ours {
                (&mut out.elsewhere, &mut out.more_elsewhere)
            } else if shown_at.is_some() {
                (&mut out.on_lines, &mut out.more_on_lines)
            } else {
                (&mut out.outside, &mut out.more_outside)
            };
            if list.len() == MOST_LISTED {
                *more += 1;
                continue;
            }
            list.push(Placed {
                tool: set.tool.clone(),
                out_of_date,
                shown_at,
                diagnostic: diagnostic.clone(),
            });
        }
    }
    out
}

/// A branch's results on the file at `path`, read from its store: [`on_file`], or the sentence that
/// says why they could not be read. A branch's diff is never refused for its results.
pub fn of_branch(
    plane: &Path,
    branch: Branch<'_>,
    path: &str,
    hunks: &[Hunk],
    head: Option<&str>,
) -> OnFile {
    match read_record(plane, branch.ws, branch.repo, branch.piece) {
        Ok(record) => OnFile {
            faults: record.faults,
            ..on_file(&record.sets, path, hunks, head)
        },
        Err(why) => OnFile {
            trouble: Some(why),
            ..OnFile::default()
        },
    }
}

/// The first line of `diagnostic` a hunk shows on the head's side. git's `+n,0` shows none.
fn shown_by(diagnostic: &Diagnostic, hunks: &[Hunk]) -> Option<u32> {
    let first = diagnostic.line?;
    let last = diagnostic.end_line.unwrap_or(first);
    hunks
        .iter()
        .filter(|hunk| hunk.new_lines > 0)
        .filter_map(|hunk| {
            let past = hunk.new_start.saturating_add(hunk.new_lines);
            (first < past && last >= hunk.new_start).then(|| first.max(hunk.new_start))
        })
        .min()
}
