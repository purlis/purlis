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

pub mod relayed;

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
    let live = crate::wscmd::live_workspaces(root);
    let local = run.workspace.is_some_and(|ws| !live.contains(ws));
    if local {
        return crate::dispatch::append(&path_for(root, when, device), root, &row);
    }
    // Another workspace's copy its persona names by its workspace (SK-5c, #6041), whose name is
    // said too: unless that workspace is LOCAL, when the run is no copy's, never the chat's own.
    let copy = copy_workspace(root, run, persona);
    let copy_local = copy.as_ref().is_some_and(|ws| !live.contains(ws));
    let level = run.level.filter(|_| !copy_local);
    // Which copy it is of, where the start knew it (SK-13): its level, and the workspace the
    // chat worked in, null at the plane root. The workspace is said wherever the start knew the
    // chat's harness too (SK-20), so a total by workspace counts every run that says where.
    if let Some(level) = level {
        row["level"] = level.word().into();
        if let Some(copy) = &copy {
            row["copy_workspace"] = copy.as_str().into();
        }
    }
    if run.level.is_some() || run.harness.is_some() {
        row["workspace"] = run.workspace.into();
    }
    // What the chat ran (SK-19): the tree git holds of the copy's folder, or that its folder held
    // changes not committed, so the run is matched to its version by content, not by time.
    let content = if copy_local {
        None
    } else {
        run.ran
            .cloned()
            .or_else(|| ran_of(root, run, persona, copy.as_deref()))
    };
    if let Some(content) = content {
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

/// **The skill a run started as `skill` is of, by its own name**, where its start found it the
/// copy at `level` ([`crate::skills::whose`]): without the namespace a harness started it by
/// (`purlis:purlis-secrets`). A run recorded before each built-in skill took the prefix reads as it
/// now, by the namespace it was started under (`purlis:secrets`) or its built-in copy, so a
/// skill's history is one history and never a plane skill's of the old name. The one reading of a
/// run's name, for the event log's runs and for the chip of the run open in a chat.
pub fn own_name(skill: &str, level: Option<Level>) -> std::borrow::Cow<'_, str> {
    let name = purlis_skills::run_name(skill);
    match level {
        Some(level)
            if level.origin == crate::skills::Origin::BuiltIn
                && !name.starts_with(purlis_skills::RESERVED_PREFIX) =>
        {
            let own = format!("{}{name}", purlis_skills::RESERVED_PREFIX);
            // A built-in skill's run by a short name it had before a rename (#5724).
            purlis_skills::name_now(&own).to_owned().into()
        }
        _ => name,
    }
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
    /// What the run ran, where whoever started it read it ([`Begun::ran`]): a runner's start, off
    /// its own checkout (#4219). `None` reads it off the plane the row is logged in.
    pub ran: Option<&'a Content>,
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
            ran: None,
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
            ran: None,
        }
    }
}

/// **Which copy run `run`, as `persona`, is of, as its start says it** (#6083), read once off the
/// plane at `root`: what its `skill.started` says beside its level.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Begun {
    /// What it ran (SK-19): the tree git holds of its copy's folder, or that the folder held
    /// changes not committed. What a start's row says and, on a runner, what its
    /// `skill.started` tells its desktop (#4219).
    pub ran: Option<Content>,
    /// Whose copy it is ([`copy_said`]).
    pub copy: CopySaid,
}

/// **[`Begun`] of run `run`, as `persona`, read off the plane at `root`**: its copy worked out
/// once, for both what it ran and whose it is.
pub fn begun(root: &Path, run: &Starting<'_>, persona: Option<&str>) -> Begun {
    let named = copy_workspace(root, run, persona);
    let copy = CopySaid::of(root, named.clone());
    Begun {
        // Nothing of a copy its row says nothing of.
        ran: (copy != CopySaid::Unsaid)
            .then(|| ran_of(root, run, persona, named.as_deref()))
            .flatten(),
        copy,
    }
}

/// **Whose copy a run is of, as its start may say it** (#6083), by the rule its row is written
/// by ([`started`]).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum CopySaid {
    /// The copy its level names in the chat's own place: every run but the two below.
    #[default]
    Own,
    /// Another workspace's copy its persona names by its workspace (`workspaces/<ws>:<name>`,
    /// [`crate::skills::named_copy_of`]), a LIVE one, said beside its level (`workspace ops`).
    Named(String),
    /// A LOCAL workspace's copy its persona names: neither that workspace's name (SK-20) nor
    /// its level is said, since the level alone would read as the chat's own workspace's copy.
    Unsaid,
}

impl CopySaid {
    /// What the named copy of workspace `copy` of the plane at `root` may be said as.
    fn of(root: &Path, copy: Option<String>) -> Self {
        match copy {
            None => CopySaid::Own,
            Some(ws) if crate::wscmd::live_workspaces(root).contains(&ws) => CopySaid::Named(ws),
            Some(_) => CopySaid::Unsaid,
        }
    }

    /// The level its start says, of `level` the start knew: none for [`CopySaid::Unsaid`].
    pub fn level(&self, level: Option<Level>) -> Option<Level> {
        level.filter(|_| *self != CopySaid::Unsaid)
    }

    /// The workspace it names beside its level, for [`CopySaid::Named`] alone.
    pub fn workspace(&self) -> Option<&str> {
        match self {
            CopySaid::Named(ws) => Some(ws),
            CopySaid::Own | CopySaid::Unsaid => None,
        }
    }
}

/// **Whose copy run `run`, as `persona`, is of, as its start may say it** (#6083), read off the
/// plane at `root`: what the window's run chip says beside its level. [`begun`] also reads what
/// it ran.
pub fn copy_said(root: &Path, run: &Starting<'_>, persona: Option<&str>) -> CopySaid {
    CopySaid::of(root, copy_workspace(root, run, persona))
}

/// [`Begun::ran`], of the copy whose workspace, at the workspace level, is `copy` where that is
/// not the chat's own ([`copy_workspace`]), worked out once by whoever asks.
fn ran_of(
    root: &Path,
    run: &Starting<'_>,
    persona: Option<&str>,
    copy: Option<&str>,
) -> Option<Content> {
    let level = run.level?;
    let skill = crate::memstore::py_strip(run.skill);
    content_of(
        root,
        &own_name(skill, Some(level)),
        level,
        copy.or(run.workspace),
        stripped(persona),
    )
}

/// A persona's name as a row writes it, or `None` for none or a blank one.
fn stripped(persona: Option<&str>) -> Option<&str> {
    persona
        .map(crate::memstore::py_strip)
        .filter(|p| !p.is_empty())
}

/// **The workspace whose copy run `run`, as `persona`, ran, where that is not the chat's own**
/// ([`crate::skills::named_copy_of`], SK-5c, #6041): another workspace's skill its persona names
/// by its workspace. `None` for every other run.
fn copy_workspace(root: &Path, run: &Starting<'_>, persona: Option<&str>) -> Option<String> {
    let level = run.level?;
    let skill = crate::memstore::py_strip(run.skill);
    crate::skills::named_copy_of(
        root,
        run.workspace,
        stripped(persona),
        level,
        &own_name(skill, Some(level)),
    )
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
    ended_as(
        root,
        &Ending {
            id: &run.open.id,
            ended: run.ended,
            ms: run.ms,
            signals: run.open.signals,
            tokens,
            parent: run.open.parent.as_deref(),
        },
        when,
        device,
    )
}

/// A skill run's end, as [`ended_as`] logs it: the run the event log's writer closed
/// ([`ended`]), or one a runner's event log says ended ([`relayed`], #3084).
#[derive(Debug, Clone, Copy)]
pub struct Ending<'a> {
    pub id: &'a str,
    pub ended: Ended,
    /// How long it was open, in milliseconds.
    pub ms: u64,
    pub signals: purlis_skills::runs::Signals,
    /// The tokens spent inside it, where the harness said them.
    pub tokens: Option<u64>,
    /// The run it started inside, where it is a child.
    pub parent: Option<&'a str>,
}

/// **Logs the end `run`**, the one row every end is written as ([`ended`]).
pub fn ended_as(
    root: &Path,
    run: &Ending<'_>,
    when: chrono::DateTime<chrono::Utc>,
    device: &str,
) -> Option<PathBuf> {
    if !writable(root) {
        return None;
    }
    let signals = run.signals;
    let mut row = serde_json::json!({
        "asks": signals.asks,
        "ended": run.ended,
        "ms": run.ms,
        "refusals": signals.refusals,
        "run": run.id,
        "tool_calls": signals.tool_calls,
        "tool_errors": signals.tool_errors,
        "ts": crate::dispatch::stamp(when),
    });
    if let Some(tokens) = run.tokens {
        row["tokens"] = tokens.into();
    }
    if let Some(parent) = run.parent {
        row["parent"] = parent.into();
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
    /// plugin's namespace a harness started it by (`purlis:purlis-secrets` is a run of
    /// `purlis-secrets`), as [`own_name`] reads it.
    pub fn is_of(&self, name: &str) -> bool {
        self.skill_name() == name || (name.contains(':') && self.skill == name)
    }

    /// Its skill by name, as [`own_name`] reads it.
    pub fn skill_name(&self) -> std::borrow::Cow<'_, str> {
        own_name(&self.skill, self.copy.as_ref().map(|copy| copy.level))
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
    /// The workspace whose copy it is, where that is not the chat's own: another workspace's
    /// skill its persona names by its workspace (`workspaces/<ws>:<name>`, SK-5c, #6041). `None`
    /// for every other run, whose copy, at the workspace level, is of [`Self::workspace`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub copy_workspace: Option<String>,
}

impl RunCopy {
    /// **The workspace whose copy it is**, at the workspace level: the one its persona named it
    /// by ([`Self::copy_workspace`]), else the chat's own, `None` at the plane root.
    pub fn workspace_of_copy(&self) -> Option<&str> {
        self.copy_workspace.as_deref().or(self.workspace.as_deref())
    }
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
    copy_workspace: Option<String>,
    tree: Option<String>,
    harness: Option<String>,
    /// The skill a miss is of (#5738): a miss's row has no `skill`, so a purlis before misses
    /// reads none of it as a run.
    missed: Option<String>,
    signal: Option<String>,
    harness_version: Option<String>,
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

/// **How many runs each skill had from `since` on** (seconds since 1970), by its own name
/// ([`Run::skill_name`]): what Make room sorts the Library by, least used first (#5355). A run
/// whose stamp cannot be read is no run of the window. A skill with none is not in it.
pub fn runs_since(root: &Path, since: u64) -> std::collections::BTreeMap<String, u32> {
    let mut counted = std::collections::BTreeMap::new();
    for run in runs(root) {
        if run.started_secs().is_some_and(|at| at >= since) {
            *counted.entry(run.skill_name().into_owned()).or_insert(0u32) += 1;
        }
    }
    counted
}

/// **Every skill run the plane's skill log holds**, newest first: each start with its end
/// where one is logged, and each row written before runs were recorded as a run of its own.
pub fn runs(root: &Path) -> Vec<Run> {
    let mut runs: Vec<Run> = Vec::new();
    let mut ends: std::collections::HashMap<String, End> = std::collections::HashMap::new();
    for (device, rows) in rows(root) {
        for row in rows {
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
                                copy_workspace: row
                                    .copy_workspace
                                    .clone()
                                    // One plain name, as a workspace's folder is.
                                    .filter(|ws| {
                                        purlis_skills::level_rel(
                                            &purlis_skills::Whose::Workspace(ws.clone()),
                                            false,
                                        )
                                        .is_some()
                                    }),
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

/// **Every row of the plane's skill log**, file by file in the order the files sort, each with
/// the device whose log holds it: a line that is no row is skipped.
fn rows(root: &Path) -> Vec<(String, Vec<Row>)> {
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
    files
        .into_iter()
        .filter_map(|file| {
            let device = file
                .file_stem()
                .and_then(|stem| stem.to_str())
                .and_then(|stem| stem.split_once('.'))
                .map(|(_, device)| device.to_owned())
                .unwrap_or_default();
            let text = std::fs::read_to_string(&file).ok()?;
            let rows = crate::mdsection::split_lines(&text)
                .into_iter()
                .filter_map(|line| serde_json::from_str::<Row>(line).ok())
                .collect();
            Some((device, rows))
        })
        .collect()
}

/// **What says a built-in skill was missed** (#5738, spec #5714): a [`Signal::Correction`], the
/// person starting it themselves right after a turn that did not, or a [`Signal::Report`] the
/// person filed that it should have started.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum Signal {
    Correction,
    Report,
    /// A signal a newer purlis wrote, which this one does not know: still a miss.
    #[serde(other)]
    Unknown,
}

impl Signal {
    /// The word a miss records: `correction`, `report` or `unknown`.
    pub fn word(self) -> &'static str {
        match self {
            Self::Correction => "correction",
            Self::Report => "report",
            Self::Unknown => "unknown",
        }
    }

    fn parse(word: &str) -> Self {
        match word {
            "correction" => Self::Correction,
            "report" => Self::Report,
            _ => Self::Unknown,
        }
    }
}

/// A miss of a built-in skill, as [`missed`] logs it.
#[derive(Debug, Clone, Copy)]
pub struct Missing<'a> {
    /// The skill, by its own name ([`own_name`]).
    pub skill: &'a str,
    /// The harness the chat ran on, by the word the plane calls it, where it is known.
    pub harness: Option<&'a str>,
    /// The harness's version, where this purlis heard it.
    pub harness_version: Option<&'a str>,
    pub persona: Option<&'a str>,
    pub signal: Signal,
}

/// **Logs a miss of a built-in skill** (#5738): one row beside the runs, with the skill, the
/// harness and its version, the persona and the signal, and never a prompt. Its skill is
/// `missed`, never `skill`, so a purlis from before misses reads no run of it. Nothing is
/// logged in a project this purlis may not write.
pub fn missed(
    root: &Path,
    miss: &Missing<'_>,
    when: chrono::DateTime<chrono::Utc>,
    device: &str,
) -> Option<PathBuf> {
    let skill = crate::memstore::py_strip(miss.skill);
    if skill.is_empty() || !writable(root) {
        return None;
    }
    fn said(value: Option<&str>) -> Option<&str> {
        value
            .map(crate::memstore::py_strip)
            .filter(|value| !value.is_empty())
    }
    let row = serde_json::json!({
        "missed": skill,
        "signal": miss.signal.word(),
        "harness": said(miss.harness),
        "harness_version": said(miss.harness_version),
        "persona": said(miss.persona),
        "ts": crate::dispatch::stamp(when),
    });
    crate::dispatch::append(&path_for(root, when, device), root, &row)
}

/// **What a case-6 Report says was missed** (#5739, spec #5714): the built-in skill its
/// `Skill:` line names, by the name it has now, and the harness and version its `Harness:` line
/// names, where it names one purlis starts. The lines are the ones purlis-report's case 6 asks
/// for (#5698); a Report with no `Skill:` line naming a built-in (`purlis-…`) is of another case.
///
/// Read from the words the person filed, never from a harness's output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reported {
    pub skill: String,
    /// The harness by the word the plane calls it (`claude`).
    pub harness: Option<String>,
    pub harness_version: Option<String>,
}

impl Reported {
    /// The miss Report `body` says, or none where it is not case 6.
    pub fn of(body: &str) -> Option<Reported> {
        let said = |field: &str| {
            body.lines().find_map(|line| {
                let line = line.trim().trim_start_matches(['-', '*', '>', ' ']);
                let (name, value) = line.split_once(':')?;
                name.trim_matches(['*', '`', ' '])
                    .eq_ignore_ascii_case(field)
                    .then(|| value.trim().trim_matches(['`', '"', '*', ' ']))
            })
        };
        let skill = said("Skill")?.split_whitespace().next()?;
        let skill = purlis_skills::run_name(skill);
        if !purlis_skills::name_reserved(&skill) {
            return None;
        }
        let skill = purlis_skills::name_now(&skill.to_ascii_lowercase()).to_owned();
        let (harness, harness_version) = said("Harness").map_or((None, None), harness_named);
        Some(Reported {
            skill,
            harness,
            harness_version,
        })
    }
}

/// The harness `value` names by its title or its word (`Claude Code 2.1.40`, `codex`), and the
/// version it gives after it, where it gives one.
fn harness_named(value: &str) -> (Option<String>, Option<String>) {
    let lower = value.to_lowercase();
    let named = crate::harness::Harness::ALL
        .into_iter()
        .find_map(|harness| {
            let declared = harness.declared();
            [declared.title.to_lowercase(), declared.name.clone()]
                .into_iter()
                .find(|name| {
                    lower.starts_with(name.as_str())
                        && !lower[name.len()..].starts_with(|c: char| c.is_alphanumeric())
                })
                .map(|name| {
                    (
                        declared.name.clone(),
                        value.get(name.len()..).unwrap_or("").to_owned(),
                    )
                })
        });
    let Some((word, rest)) = named else {
        return (None, None);
    };
    let version = rest
        .split(|c: char| c.is_whitespace() || matches!(c, ',' | '(' | ')' | ';'))
        .map(|token| token.trim_start_matches(['v', 'V']).trim_end_matches('.'))
        .find(|token| token.starts_with(|c: char| c.is_ascii_digit()))
        .map(str::to_owned);
    (Some(word), version)
}

/// One miss of a built-in skill, as the plane's skill log holds it (#5738).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Miss {
    /// The skill, by its own name.
    pub skill: String,
    pub harness: Option<String>,
    pub harness_version: Option<String>,
    pub persona: Option<String>,
    pub signal: Signal,
    /// When it was logged, as the row stamped it.
    pub at: String,
    /// The device whose log holds it.
    pub device: String,
}

impl Miss {
    /// Whether this is a miss of skill `name`, by its own name or the name a harness starts it
    /// by (`purlis:purlis-secrets`).
    pub fn is_of(&self, name: &str) -> bool {
        self.skill == purlis_skills::run_name(name)
    }
}

/// **Every miss the plane's skill log holds** (#5738), newest first.
pub fn misses(root: &Path) -> Vec<Miss> {
    let mut misses: Vec<Miss> = rows(root)
        .into_iter()
        .flat_map(|(device, rows)| {
            rows.into_iter().filter_map(move |row| {
                let skill = row.missed.filter(|skill| !skill.is_empty())?;
                // A miss logged under a name the skill had before a rename is of it now (#5724).
                let skill = purlis_skills::name_now(&skill).to_owned();
                let some = |value: Option<String>| value.filter(|value| !value.is_empty());
                Some(Miss {
                    skill,
                    harness: some(row.harness),
                    harness_version: some(row.harness_version),
                    persona: some(row.persona),
                    signal: row.signal.as_deref().map_or(Signal::Unknown, Signal::parse),
                    at: row.ts.unwrap_or_default(),
                    device: device.clone(),
                })
            })
        })
        .collect();
    misses.sort_by(|a, b| b.at.cmp(&a.at));
    misses
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
    /// Misses of the skill (#5738): times it should have started and did not. Counted by
    /// harness alone ([`stats_with_misses`]).
    pub misses: u64,
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
    stats_with_misses(runs, &[], by)
}

/// **The totals of `runs` and `misses` by `by`** (#5738): [`stats`], with each harness's misses
/// counted beside its runs, and a harness that had misses and no run a row of its own. Misses
/// are totalled per harness alone: by persona, workspace or version none is counted.
pub fn stats_with_misses(runs: &[Run], misses: &[Miss], by: By) -> Vec<Stats> {
    let mut groups: std::collections::BTreeMap<Group, Totals> = std::collections::BTreeMap::new();
    for run in runs {
        groups.entry(Group::of(run, by)).or_default().add(run);
    }
    if by == By::Harness {
        for miss in misses {
            let group = miss
                .harness
                .clone()
                .map_or(Group::NotRecorded, Group::Named);
            groups.entry(group).or_default().misses += 1;
        }
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
