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
    if let Some(harness) = run.harness {
        row["harness"] = harness.into();
    }
    // A workspace that is not LIVE is LOCAL, this machine's alone: its name is never written
    // into the committed log, nor the copy of it, which would say a workspace by its level
    // (SK-20). Such a run says neither where it ran nor which copy it is of. A LIVE workspace is
    // committed and shared, and its runs say both.
    let local = run
        .workspace
        .is_some_and(|ws| !crate::wscmd::live_workspaces(root).contains(ws));
    if local {
        return crate::dispatch::append(&path_for(root, when, device), root, &row);
    }
    // Which copy it is of, where the start knew it (SK-13): its level, and the workspace the
    // chat worked in, null at the plane root. The workspace is said wherever the start knew the
    // chat's harness too (SK-20), so a total by workspace counts every run that says where.
    if let Some(level) = run.level {
        row["level"] = level.word().into();
    }
    if run.level.is_some() || run.harness.is_some() {
        row["workspace"] = run.workspace.into();
    }
    // What the chat ran (SK-19): the tree git holds of the copy's folder, or that its folder held
    // changes not committed, so the run is matched to its version by content, not by time.
    if let Some(level) = run.level
        && let Some(content) = content_of(root, skill_name(skill), level, run.workspace, persona)
    {
        row["tree"] = content.word().into();
    }
    crate::dispatch::append(&path_for(root, when, device), root, &row)
}

/// **What a run of a skill ran, by content** (SK-19): the tree git holds of its folder, or that
/// the folder held changes git has not committed (or is no folder git has).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Content {
    /// The folder's tree, as a whole hex id: the version whose folder has this tree.
    Tree(String),
    /// The folder held changes not committed, or git does not have it.
    Uncommitted,
}

impl Content {
    /// The word a run's start records: the tree's id, or `uncommitted`.
    pub fn word(&self) -> &str {
        match self {
            Self::Tree(tree) => tree,
            Self::Uncommitted => "uncommitted",
        }
    }

    /// The content `word` says, or `None` for a word that is neither a whole hex id nor
    /// `uncommitted`.
    pub fn parse(word: &str) -> Option<Self> {
        if word == "uncommitted" {
            return Some(Self::Uncommitted);
        }
        let hex = matches!(word.len(), 40 | 64)
            && word
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
        hex.then(|| Self::Tree(word.to_owned()))
    }
}

impl serde::Serialize for Content {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.word())
    }
}

/// A run's skill by its name, without the namespace a harness started it by (`purlis:secrets`
/// is `secrets`).
fn skill_name(skill: &str) -> &str {
    skill.split_once(':').map_or(skill, |(_, leaf)| leaf)
}

/// What the copy at `level` of the skill `name` holds, in the plane at `root` (a chat in
/// `workspace`, as `persona`): `None` for a copy with no folder of the plane's levels a history
/// is kept of (a built-in or a harness skill), and for a plane git does not keep. A persona's
/// copy has its history as the other levels' do (SK-32, #3124).
fn content_of(
    root: &Path,
    name: &str,
    level: Level,
    workspace: Option<&str>,
    persona: Option<&str>,
) -> Option<Content> {
    let at = match level.origin {
        purlis_skills::Origin::Project => purlis_skills::Whose::Project,
        purlis_skills::Origin::Workspace => purlis_skills::Whose::Workspace(workspace?.to_owned()),
        purlis_skills::Origin::Persona => purlis_skills::Whose::Persona(persona?.to_owned()),
        _ => return None,
    };
    let folder = purlis_skills::named_at(root, &at, level.local, name)?;
    content_at(root, &folder)
}

/// **What the skill folder `folder` of the plane at `root` holds, by content**: the tree git
/// holds of it, or that it holds changes git has not committed. `None` for a folder outside the
/// plane, and for a plane git does not keep. The one reading a run's start and a grade (SK-16)
/// both take.
pub(crate) fn content_at(root: &Path, folder: &Path) -> Option<Content> {
    let rel = folder
        .strip_prefix(root)
        .ok()?
        .to_string_lossy()
        .replace('\\', "/");
    let git = |args: &[&str]| {
        crate::worktree::git::run(root, args, crate::worktree::git::READ)
            .ok()
            .filter(crate::worktree::git::Run::ok)
            .map(|run| run.out)
    };
    git(&["rev-parse", "--is-inside-work-tree"])?;
    let spec = crate::planegit::literally(&rel);
    let pending = git(&[
        "status",
        "--porcelain",
        "--untracked-files=all",
        "--",
        &spec,
    ]);
    if pending.is_none_or(|out| !out.trim().is_empty()) {
        return Some(Content::Uncommitted);
    }
    let tree = git(&["rev-parse", "--verify", "-q", &format!("HEAD:{rel}")]);
    Some(
        tree.and_then(|out| Content::parse(out.trim()))
            .filter(|content| matches!(content, Content::Tree(_)))
            .unwrap_or(Content::Uncommitted),
    )
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

impl<'a> Starting<'a> {
    /// The start of run `open`, the event log's writer opened, in a chat on `harness`.
    pub fn of_open(open: &'a Open, harness: Option<&'a str>) -> Self {
        Starting {
            id: &open.id,
            skill: &open.skill,
            started_by: open.started_by,
            workspace: open.whose.workspace.as_deref(),
            level: open.whose.level,
            harness,
        }
    }

    /// The start `started` a hook or purlis's MCP server records itself, in a chat on `harness`.
    pub fn of_started(
        started: &'a crate::hookwire::SkillStarted,
        harness: Option<&'a str>,
    ) -> Self {
        Starting {
            id: &started.run,
            skill: &started.skill,
            started_by: started.started_by,
            workspace: started.workspace.as_deref(),
            level: started.level,
            harness,
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
    /// What it ran, by content (SK-19), or `None` for a row that does not say (written before
    /// SK-19, or of a copy no history is kept of).
    pub content: Option<Content>,
    /// How it ended and what it counted, once its end is logged.
    pub end: Option<End>,
}

/// **What a harness is called**, by the word a run's start records it by (`claude`): its
/// declaration's title (`Claude Code`) for one purlis ships, and the word itself for one a project
/// declares.
pub fn harness_title(word: &str) -> String {
    crate::harness::Harness::of_kind(word).map_or_else(
        || word.to_owned(),
        |harness| harness.declared().title.clone(),
    )
}

impl Run {
    /// What it ran on, as a person calls it ([`harness_title`]), where its start says.
    pub fn ran_on(&self) -> Option<String> {
        self.harness.as_deref().map(harness_title)
    }

    /// Whether this is a run of skill `name`: by its whole name, or by its name without the
    /// plugin's namespace a harness started it by (`purlis:secrets` is a run of `secrets`), or,
    /// for a run of a built-in skill, without the prefix opencode knows it by
    /// (`purlis-secrets`).
    pub fn is_of(&self, name: &str) -> bool {
        self.skill == name || self.skill_name() == name
    }

    /// Its skill by name, without the namespace a harness started it by, nor the prefix of a
    /// built-in skill's own name on opencode ([`crate::skills::Route::built_in_name`]).
    pub fn skill_name(&self) -> &str {
        let name = skill_name(&self.skill);
        let built_in = self
            .copy
            .as_ref()
            .is_some_and(|copy| copy.level.origin == crate::skills::Origin::BuiltIn);
        if built_in {
            name.strip_prefix(crate::skills::BUILT_IN_OWN_PREFIX)
                .unwrap_or(name)
        } else {
            name
        }
    }

    /// When it started, in seconds since 1970, where its row's stamp can be read.
    pub fn started_secs(&self) -> Option<u64> {
        chrono::DateTime::parse_from_rfc3339(&self.started)
            .ok()
            .and_then(|at| u64::try_from(at.timestamp()).ok())
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
    tree: Option<String>,
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
                    let workspace = row.workspace.map(|ws| ws.filter(|ws| !ws.is_empty()));
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
                        content: row.tree.as_deref().and_then(Content::parse),
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum By {
    Harness,
    Persona,
    Workspace,
    /// What the run ran, by content (SK-19): a tree's id is [`Group::Named`], a folder with changes
    /// not committed [`Group::None`].
    Version,
}

/// **The group a run is totalled in**: a name, none (no persona; the plane root), or a run whose
/// row does not say (one written before its start said where).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Group {
    Named(String),
    /// No persona, or the plane root: a fact, not a gap.
    None,
    /// The row does not say.
    NotRecorded,
}

impl Group {
    /// This group as a person calls it where it is a harness: its declaration's title
    /// ([`harness_title`]).
    pub fn harness_titled(self) -> Group {
        match self {
            Group::Named(word) => Group::Named(harness_title(&word)),
            other => other,
        }
    }

    fn of(run: &Run, by: By) -> Group {
        let named = |name: &Option<String>| name.clone().map_or(Group::None, Group::Named);
        match by {
            By::Persona => named(&run.persona),
            By::Harness => run.harness.clone().map_or(Group::NotRecorded, Group::Named),
            By::Workspace => run.workspace.as_ref().map_or(Group::NotRecorded, named),
            By::Version => match &run.content {
                Some(Content::Tree(tree)) => Group::Named(tree.clone()),
                Some(Content::Uncommitted) => Group::None,
                None => Group::NotRecorded,
            },
        }
    }
}

/// **What a group's runs add up to**: signals and tokens, never a pass or a fail (decision 24).
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
    /// Tokens spent inside them, as the harness said them.
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
#[derive(Debug, Clone, PartialEq, Eq)]
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
    rows.sort_by_key(|row| std::cmp::Reverse(row.totals.runs));
    rows
}

#[cfg(test)]
mod tests;
