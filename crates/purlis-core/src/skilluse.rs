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
//! **Each start says where the run happened** (SK-20, #2783): the harness the chat ran on, the
//! workspace it worked in (null at the plane root), and the persona. What a skill's tab totals
//! its runs by ([`stats`]): the one reading of the log's runs into totals, by harness, persona or
//! workspace, that every surface showing them draws.
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
    // chat worked in, null at the plane root. The workspace is said wherever the start knew the
    // chat's harness too (SK-20), so a total by workspace counts every run that says where.
    if let Some(level) = run.level {
        row["level"] = level.word().into();
    }
    if let Some(harness) = run.harness {
        row["harness"] = harness.into();
    }
    if run.level.is_some() || run.harness.is_some() {
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
    /// The harness the chat ran on, by the word the plane calls it (`claude`, `codex`), where
    /// the start knew it.
    pub harness: Option<&'a str>,
}

impl<'a> From<&'a Open> for Starting<'a> {
    fn from(open: &'a Open) -> Self {
        Starting {
            id: &open.id,
            skill: &open.skill,
            started_by: open.started_by,
            workspace: open.whose.workspace.as_deref(),
            level: open.whose.level,
            harness: None,
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
            harness: None,
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
    /// The harness the chat ran on, or `None` for a row that does not say (written before
    /// SK-20, or by a start that could not tell).
    pub harness: Option<String>,
    /// Where the chat worked: `Some(None)` at the plane root, and `None` for a row that does not
    /// say.
    pub workspace: Option<Option<String>>,
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
    harness: Option<String>,
    /// Present, null or not, only where the row says it.
    #[serde(deserialize_with = "said")]
    workspace: Option<Option<String>>,
}

/// A field a row says, null or not: `Some` of what it says, where a field left out is `None`.
fn said<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Option<String>>, D::Error> {
    serde::Deserialize::deserialize(deserializer).map(Some)
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
                (Some(skill), id, _) => {
                    let workspace = row
                        .workspace
                        .map(|ws| ws.filter(|ws| !ws.is_empty()));
                    runs.push(Run {
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
                            workspace: workspace.clone().flatten(),
                            level,
                        }),
                    harness: row.harness.filter(|h| !h.is_empty()),
                    workspace,
                    end: None,
                });
                }
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

/// **What a skill's runs are totalled by** (SK-20): the harness its chat ran on, the persona,
/// or the workspace. One reading, [`stats`], for every grouping a surface shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum By {
    Harness,
    Persona,
    Workspace,
}

impl By {
    /// Every grouping, in the order a skill's tab shows them.
    pub const ALL: [By; 3] = [By::Harness, By::Persona, By::Workspace];
}

/// **The group a run is totalled in**: a name, none (no persona; the plane root), or a run whose
/// row does not say (one written before its start said where).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize)]
#[serde(tag = "kind", content = "name", rename_all = "kebab-case")]
pub enum Group {
    Named(String),
    /// No persona, or the plane root: a fact, not a gap.
    None,
    /// The row does not say.
    NotRecorded,
}

impl Group {
    fn of(run: &Run, by: By) -> Group {
        let named = |name: &Option<String>| name.clone().map_or(Group::None, Group::Named);
        match by {
            By::Persona => named(&run.persona),
            By::Harness => run
                .harness
                .clone()
                .map_or(Group::NotRecorded, Group::Named),
            By::Workspace => run.workspace.as_ref().map_or(Group::NotRecorded, named),
        }
    }
}

/// **What a group's runs add up to**: signals and cost, never a pass or a fail (decision 24).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
pub struct Totals {
    /// Runs started.
    pub runs: u64,
    /// Runs whose end is logged: the ones the signals are counted from.
    pub ended: u64,
    pub tool_calls: u64,
    pub tool_errors: u64,
    pub refusals: u64,
    pub asks: u64,
    /// Tokens spent inside them: their cost, as the harness said it.
    pub tokens: u64,
    /// Runs whose harness said their tokens, of [`Totals::ended`].
    pub tokens_said: u64,
    /// Time inside them, in milliseconds.
    pub ms: u64,
}

impl Totals {
    fn add(&mut self, run: &Run) {
        self.runs += 1;
        let Some(end) = &run.end else {
            return;
        };
        self.ended += 1;
        self.tool_calls = self.tool_calls.saturating_add(end.tool_calls);
        self.tool_errors = self.tool_errors.saturating_add(end.tool_errors);
        self.refusals = self.refusals.saturating_add(end.refusals);
        self.asks = self.asks.saturating_add(end.asks);
        self.ms = self.ms.saturating_add(end.ms);
        if let Some(tokens) = end.tokens {
            self.tokens = self.tokens.saturating_add(tokens);
            self.tokens_said += 1;
        }
    }
}

/// One group's totals ([`stats`]).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Stats {
    pub group: Group,
    pub totals: Totals,
}

/// **The totals of `runs` by `by`** (SK-20): one row per group, the most runs first, then by
/// group, named groups by name before none and the runs that do not say. The one reading a
/// skill's tab, and any other surface that totals runs, draws.
pub fn stats(runs: &[Run], by: By) -> Vec<Stats> {
    let mut groups: std::collections::BTreeMap<Group, Totals> = std::collections::BTreeMap::new();
    for run in runs {
        groups.entry(Group::of(run, by)).or_default().add(run);
    }
    let mut rows: Vec<Stats> = groups
        .into_iter()
        .map(|(group, totals)| Stats { group, totals })
        .collect();
    // Stable, so groups with as many runs keep the map's order.
    rows.sort_by(|a, b| b.totals.runs.cmp(&a.totals.runs));
    rows
}

#[cfg(test)]
mod tests;
