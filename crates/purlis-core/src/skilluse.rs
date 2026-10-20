// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The plane's skill log: every skill run** (SK-3, #2766), and which skills each persona
//! actually invokes. Once the writer of `charter/skilluse.py`.
//!
//! `personas/_skills/<YYYY-MM>.<device>.jsonl`, append-only. A skill run is two rows under one
//! id: its start, written by the hook that saw the skill invoked ([`record`]), and its end,
//! written by the event log's writer as the turn ends ([`ended`]), with its signals. A chat no
//! host records (a terminal's own harness) leaves its start alone. It is committed beside the
//! dispatch log for the same reason it always was: a persona whose declared `skills:` are never
//! used, or which leans on skills it never declared, is a fact about the whole team's use of it,
//! not about one laptop. `charter persona` reads it back as drift between what a persona
//! declares and what it does, and the Skill studio and `purlis skill runs` read its runs
//! ([`runs`]).
//!
//! **The rows written before runs** were `{persona, skill, ts}`, one per `Skill` call. They are
//! read as runs too, each a run of its own that says it was recorded before runs and holds no
//! signals ([`Run::started_by`] `None`): the log stays append-only and nothing is rewritten.

use std::path::{Path, PathBuf};

/// The directory under `personas/` — `skilluse.DIR_NAME`.
pub const DIR_NAME: &str = "_skills";

/// `personas/_skills/<YYYY-MM>.<device>.jsonl` — `skilluse.path_for`; `<device>` is
/// [`crate::machine::log_name`]'s (FD-25).
pub fn path_for(root: &Path, when: chrono::DateTime<chrono::Utc>, device: &str) -> PathBuf {
    root.join("personas")
        .join(DIR_NAME)
        .join(format!("{}.{device}.jsonl", when.format("%Y-%m")))
}

/// **Logs the start of skill run `run`**, of `skill` by `persona`, started by `started_by`: the
/// row the hook that saw it invoked writes. Nothing is logged in a project this purlis may not
/// write ([`crate::compat::read_only`], #826): the log is committed.
pub fn record(
    root: &Path,
    started: &Started<'_>,
    when: chrono::DateTime<chrono::Utc>,
    device: &str,
) -> Option<PathBuf> {
    let skill = crate::memstore::py_strip(started.skill);
    if skill.is_empty() || crate::compat::read_only(root).is_some() {
        return None;
    }
    let persona = started
        .persona
        .map(crate::memstore::py_strip)
        .filter(|p| !p.is_empty())
        .map_or(serde_json::Value::Null, |p| {
            serde_json::Value::String(p.to_string())
        });
    crate::dispatch::append(
        &path_for(root, when, device),
        root,
        &serde_json::json!({
            "persona": persona,
            "run": started.run,
            "skill": skill,
            "started_by": started.started_by.word(),
            "ts": crate::dispatch::stamp(when),
        }),
    )
}

/// The start of one skill run, as [`record`] logs it.
#[derive(Debug, Clone, Copy)]
pub struct Started<'a> {
    /// The run's id.
    pub run: &'a str,
    pub skill: &'a str,
    pub persona: Option<&'a str>,
    pub started_by: purlis_skills::runs::StartedBy,
}

/// The end of one skill run, as [`ended`] logs it.
#[derive(Debug, Clone, Copy)]
pub struct Ending<'a> {
    pub run: &'a purlis_skills::runs::Closed,
    /// The tokens spent inside it, where the harness said them.
    pub tokens: Option<u64>,
}

/// **Logs the end of a skill run**, with its signals: the row the event log's writer adds as
/// the turn ends. Under the device it ran on, as its start is. Nothing in a project this
/// purlis may not write.
pub fn ended(
    root: &Path,
    ending: &Ending<'_>,
    when: chrono::DateTime<chrono::Utc>,
    device: &str,
) -> Option<PathBuf> {
    if crate::compat::read_only(root).is_some() || !root.join(crate::plane::MANIFEST).is_file() {
        return None;
    }
    let run = ending.run;
    let signals = run.open.signals;
    let mut row = serde_json::json!({
        "asks": signals.asks,
        "ended": run.ended.word(),
        "ms": run.ms,
        "refusals": signals.refusals,
        "run": run.open.id,
        "tool_calls": signals.tool_calls,
        "tool_errors": signals.tool_errors,
        "ts": crate::dispatch::stamp(when),
    });
    if let Some(tokens) = ending.tokens {
        row["tokens"] = tokens.into();
    }
    if let Some(parent) = &run.open.parent {
        row["parent"] = parent.as_str().into();
    }
    crate::dispatch::append(&path_for(root, when, device), root, &row)
}

/// One skill run, as the plane's skill log holds it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Run {
    /// Its id, or `None` for a row written before runs.
    pub id: Option<String>,
    pub skill: String,
    pub persona: Option<String>,
    /// What started it: [`purlis_skills::runs::StartedBy::word`], or `None` for a row written
    /// before runs, which did not say.
    pub started_by: Option<String>,
    /// When it started, as the row stamped it.
    pub started: String,
    /// The device whose log holds it.
    pub device: String,
    /// How it ended and what it counted, once its end is logged.
    pub end: Option<End>,
}

impl Run {
    /// Whether this is a run of skill `name`: by its whole name, or by its name without the
    /// plugin's namespace a harness invoked it by (`purlis:secrets` is a run of `secrets`).
    pub fn is_of(&self, name: &str) -> bool {
        self.skill == name
            || self
                .skill
                .split_once(':')
                .is_some_and(|(_, leaf)| leaf == name)
    }
}

/// How a skill run ended, and its signals: never a pass or a fail.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct End {
    /// `turn`, `interrupted` or `session`.
    pub ended: String,
    pub at: String,
    pub ms: u64,
    pub tool_calls: u64,
    pub tool_errors: u64,
    pub refusals: u64,
    pub asks: u64,
    /// The tokens spent inside it, where the harness said them.
    pub tokens: Option<u64>,
    /// The run it was invoked inside, where it is a child.
    pub parent: Option<String>,
}

/// **Every skill run the plane's skill log holds**, newest first: each start with its end
/// where one is logged, and each row written before runs as a run of its own.
pub fn runs(root: &Path) -> Vec<Run> {
    let dir = root.join("personas").join(DIR_NAME);
    let Ok(reader) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = reader
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "jsonl"))
        .collect();
    files.sort();
    let mut runs: Vec<Run> = Vec::new();
    let mut ends: std::collections::HashMap<String, End> = std::collections::HashMap::new();
    for file in files {
        let device = file
            .file_stem()
            .and_then(|stem| stem.to_str())
            .and_then(|stem| stem.split_once('.'))
            .map(|(_, device)| device.to_owned())
            .unwrap_or_default();
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        for line in crate::mdsection::split_lines(&text) {
            let Ok(row) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
            };
            let text = |key: &str| row.get(key).and_then(|v| v.as_str()).map(str::to_owned);
            let id = text("run");
            match (text("skill").filter(|s| !s.is_empty()), id) {
                (Some(skill), id) => runs.push(Run {
                    id,
                    skill,
                    persona: text("persona").filter(|p| !p.is_empty()),
                    started_by: text("started_by"),
                    started: text("ts").unwrap_or_default(),
                    device: device.clone(),
                    end: None,
                }),
                (None, Some(id)) if text("ended").is_some() => {
                    let count = |key: &str| row.get(key).and_then(|v| v.as_u64()).unwrap_or(0);
                    ends.insert(
                        id,
                        End {
                            ended: text("ended").unwrap_or_default(),
                            at: text("ts").unwrap_or_default(),
                            ms: count("ms"),
                            tool_calls: count("tool_calls"),
                            tool_errors: count("tool_errors"),
                            refusals: count("refusals"),
                            asks: count("asks"),
                            tokens: row.get("tokens").and_then(|v| v.as_u64()),
                            parent: text("parent"),
                        },
                    );
                }
                _ => {}
            }
        }
    }
    for run in &mut runs {
        if let Some(id) = &run.id {
            run.end = ends.remove(id);
        }
    }
    runs.sort_by(|a, b| b.started.cmp(&a.started).then(b.id.cmp(&a.id)));
    runs
}

#[cfg(test)]
mod tests;
