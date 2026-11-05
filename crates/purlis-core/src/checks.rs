// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A branch's check results, read from SARIF 2.1.0 and placed on a comparison (ADR 0114, #5407).

use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_sarif::sarif;

use crate::files::Hunk;

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
