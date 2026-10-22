// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The plane's skill log: every skill run** (SK-3, #2766), and which skills each persona
//! actually uses. Once the writer of `charter/skilluse.py`.
//!
//! `personas/_skills/<YYYY-MM>.<device>.jsonl`, append-only. A skill run is two rows under one
//! id, both written by the event log's writer: its start as the run opens ([`started`]), and its
//! end as the turn ends ([`ended`]), with its signals. A chat no host records (a terminal's own
//! harness) has its start written by the hook itself, and no end: nothing counts its signals. It is committed beside the
//! dispatch log for the same reason it always was: a persona whose declared `skills:` are never
//! used, or which leans on skills it never declared, is a fact about the whole team's use of it,
//! not about one laptop. `charter persona` reads it back as drift between what a persona
//! declares and what it does, and the Skill studio and `purlis skill runs` read its runs
//! ([`runs`]).
//!
//! **The rows written before runs were recorded** were `{persona, skill, ts}`, one per use of a
//! skill. They are read as runs too, each a run of its own with no start and no signals
//! ([`Run::started_by`] `None`): the log stays append-only and nothing is rewritten.

use std::path::{Path, PathBuf};

use purlis_skills::runs::{Closed, Ended, Level, Open, StartedBy};

/// The directory under `personas/` — `skilluse.DIR_NAME`.
pub const DIR_NAME: &str = "_skills";

/// `personas/_skills/<YYYY-MM>.<device>.jsonl` — `skilluse.path_for`; `<device>` is
/// [`crate::machine::log_name`]'s (FD-25).
pub fn path_for(root: &Path, when: chrono::DateTime<chrono::Utc>, device: &str) -> PathBuf {
    root.join("personas")
        .join(DIR_NAME)
        .join(format!("{}.{device}.jsonl", when.format("%Y-%m")))
}

/// **Logs the start of skill run `run`**, by `persona`: the row the event log's writer adds as
/// the run opens, once per run however many times its turn read the skill, or the row a hook
/// with no host to tell adds itself. Nothing is logged
/// in a project this purlis may not write ([`crate::compat::read_only`], #826): the log is
/// committed.
pub fn started(
    root: &Path,
    run: &Starting<'_>,
    persona: Option<&str>,
    when: chrono::DateTime<chrono::Utc>,
    device: &str,
) -> Option<PathBuf> {
    let skill = crate::memstore::py_strip(run.skill);
    if skill.is_empty() || !writable(root) {
        return None;
    }
    let persona = persona
        .map(crate::memstore::py_strip)
        .filter(|p| !p.is_empty());
    let mut row = serde_json::json!({
        "persona": persona,
        "run": run.id,
        "skill": skill,
        "started_by": run.started_by,
        "ts": crate::dispatch::stamp(when),
    });
    // Which copy it is of, where the start knew it (SK-13): its level, and the workspace the
    // chat worked in, null at the plane root.
    if let Some(level) = run.level {
        row["level"] = level.word().into();
        row["workspace"] = run.workspace.into();
    }
    crate::dispatch::append(&path_for(root, when, device), root, &row)
}

/// A skill run's start, as [`started`] logs it: the run the event log's writer opened, or the
/// one a hook with no host to tell records itself ([`crate::toolhooks::skill_started`]).
#[derive(Debug, Clone, Copy)]
pub struct Starting<'a> {
    pub id: &'a str,
    pub skill: &'a str,
    pub started_by: StartedBy,
    /// The workspace the chat worked in, `None` at the plane root.
    pub workspace: Option<&'a str>,
    /// The copy of the skill, by its level, where the start knew it.
    pub level: Option<Level>,
}

impl<'a> From<&'a Open> for Starting<'a> {
    fn from(open: &'a Open) -> Self {
        Starting {
            id: &open.id,
            skill: &open.skill,
            started_by: open.started_by,
            workspace: open.whose.workspace.as_deref(),
            level: open.whose.level,
        }
    }
}

impl<'a> From<&'a crate::hookwire::SkillStarted> for Starting<'a> {
    fn from(started: &'a crate::hookwire::SkillStarted) -> Self {
        Starting {
            id: &started.run,
            skill: &started.skill,
            started_by: started.started_by,
            workspace: started.workspace.as_deref(),
            level: started.level,
        }
    }
}

/// **Logs the end of skill run `run`**, with its signals and the `tokens` spent inside it where
/// the harness said them: the row the event log's writer adds as the turn ends. Under the
/// device it ran on, as its start is.
pub fn ended(
    root: &Path,
    run: &Closed,
    tokens: Option<u64>,
    when: chrono::DateTime<chrono::Utc>,
    device: &str,
) -> Option<PathBuf> {
    if !writable(root) {
        return None;
    }
    let signals = run.open.signals;
    let mut row = serde_json::json!({
        "asks": signals.asks,
        "ended": run.ended,
        "ms": run.ms,
        "refusals": signals.refusals,
        "run": run.open.id,
        "tool_calls": signals.tool_calls,
        "tool_errors": signals.tool_errors,
        "ts": crate::dispatch::stamp(when),
    });
    if let Some(tokens) = tokens {
        row["tokens"] = tokens.into();
    }
    if let Some(parent) = &run.open.parent {
        row["parent"] = parent.as_str().into();
    }
    crate::dispatch::append(&path_for(root, when, device), root, &row)
}

/// Whether the log may be written at `root`: a plane this purlis may write.
fn writable(root: &Path) -> bool {
    root.join(crate::plane::MANIFEST).is_file() && crate::compat::read_only(root).is_none()
}

/// One skill run, as the plane's skill log holds it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Run {
    /// Its id, or `None` for a row written before runs were recorded.
    pub id: Option<String>,
    pub skill: String,
    pub persona: Option<String>,
    /// What started it, or `None` for a row written before runs were recorded, which did not
    /// say.
    pub started_by: Option<StartedBy>,
    /// When it started, as the row stamped it.
    pub started: String,
    /// The device whose log holds it.
    pub device: String,
    /// Which copy of the skill it is of, and where it ran, or `None` for a row that does not say
    /// (written before SK-13, or by a start that could not tell): such a run is no copy's.
    pub copy: Option<RunCopy>,
    /// How it ended and what it counted, once its end is logged.
    pub end: Option<End>,
}

impl Run {
    /// Whether this is a run of skill `name`: by its whole name, or by its name without the
    /// plugin's namespace a harness started it by (`purlis:secrets` is a run of `secrets`).
    pub fn is_of(&self, name: &str) -> bool {
        self.skill == name
            || self
                .skill
                .split_once(':')
                .is_some_and(|(_, leaf)| leaf == name)
    }
}

/// Which copy of a skill a run is of, and where it ran (SK-13).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct RunCopy {
    /// The workspace the chat worked in, or `None` at the plane root.
    pub workspace: Option<String>,
    pub level: Level,
}

/// How a skill run ended, and its signals: never a pass or a fail.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct End {
    pub ended: Ended,
    pub at: String,
    pub ms: u64,
    pub tool_calls: u64,
    pub tool_errors: u64,
    pub refusals: u64,
    pub asks: u64,
    /// The tokens spent inside it, where the harness said them.
    pub tokens: Option<u64>,
    /// The run it started inside, where it is a child.
    pub parent: Option<String>,
}

/// One row of the log, start or end, as it is read: every field it may hold.
#[derive(Debug, Default, serde::Deserialize)]
#[serde(default)]
struct Row {
    persona: Option<String>,
    skill: Option<String>,
    run: Option<String>,
    started_by: Option<StartedBy>,
    ended: Option<Ended>,
    ts: Option<String>,
    ms: Option<u64>,
    tool_calls: Option<u64>,
    tool_errors: Option<u64>,
    refusals: Option<u64>,
    asks: Option<u64>,
    tokens: Option<u64>,
    parent: Option<String>,
    level: Option<String>,
    workspace: Option<String>,
}

/// **Every skill run the plane's skill log holds**, newest first: each start with its end
/// where one is logged, and each row written before runs were recorded as a run of its own.
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
            let Ok(row) = serde_json::from_str::<Row>(line) else {
                continue;
            };
            match (row.skill.filter(|s| !s.is_empty()), row.run, row.ended) {
                (Some(skill), id, _) => runs.push(Run {
                    id,
                    skill,
                    persona: row.persona.filter(|p| !p.is_empty()),
                    started_by: row.started_by,
                    started: row.ts.unwrap_or_default(),
                    device: device.clone(),
                    copy: row
                        .level
                        .as_deref()
                        .and_then(Level::parse)
                        .map(|level| RunCopy {
                            workspace: row.workspace.filter(|ws| !ws.is_empty()),
                            level,
                        }),
                    end: None,
                }),
                (None, Some(id), Some(ended)) => {
                    ends.insert(
                        id,
                        End {
                            ended,
                            at: row.ts.unwrap_or_default(),
                            ms: row.ms.unwrap_or_default(),
                            tool_calls: row.tool_calls.unwrap_or_default(),
                            tool_errors: row.tool_errors.unwrap_or_default(),
                            refusals: row.refusals.unwrap_or_default(),
                            asks: row.asks.unwrap_or_default(),
                            tokens: row.tokens,
                            parent: row.parent,
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
