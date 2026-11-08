// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The CI grader's scoring** (#5726 of spec #5714; ADR 0105 as amended 2026-11-08): recorded
//! harness output in, a score per built-in skill and its **trigger bar** out.
//!
//! The CI grader starts Claude Code, Codex and opencode headless on each trigger test's query;
//! this module never starts anything. It reads what a harness wrote ([`parse`]) and scores it
//! ([`score`]), so it runs offline on recorded output and no test of it calls a model.
//!
//! **"Triggered" means what it means in a grade in the window** ([`super`]): a run of the skill
//! started, whichever route its harness took (the Skill tool, a plain read of a briefed
//! `SKILL.md` on Codex, or purlis's `skill_load`), by the skill's name without a plugin's
//! namespace ([`crate::leaf_name`]). A test passes when the share of its runs that started the
//! skill is at least [`TRIGGER_THRESHOLD`] for a query that should start it and below it for one
//! that should not, over the runs that ran to the end of their turn; the CI grader runs each
//! [`super::RUNS`] times.
//!
//! **A wrong built-in counts on both sides**: a query of one graded skill's that started another
//! built-in is a miss for its owner (its test fails, naming what started instead) and a false
//! trigger for the one that started, when that one is graded too. **Crowded and bare runs are
//! scored apart**, and only the crowded runs are held to the bar.
//!
//! What a skill is held to is in the **sources file** beside its trigger tests
//! ([`SOURCES_FILE`]): its trigger bar, its neighbours and a note per test saying where its query
//! came from. A skill without one, or with one that is not whole, is reported, never scored.
//!
//! **Where each harness's format was read** (#5728), its recordings under `ci/recorded/` made to
//! match. Neither Codex nor opencode was run for them: no model is called from where they were
//! written.
//!
//! - **Codex**, from the source at `rust-v0.167.0`: `exec/src/exec_events.rs` (the events and
//!   items, `turn.failed`, `mcp_tool_call`'s `server`, `tool` and `arguments`),
//!   `app-server-protocol/src/protocol/item_builders.rs` (a command is `shlex_join` of its
//!   argv) and `core/src/shell.rs` (the argv is the shell's full path, `-lc` and the command),
//!   so a read is `/bin/bash -lc 'cat …/SKILL.md'`.
//! - **opencode**, from the source at `v1.19.5`: `cli/cmd/run.ts` (each event is `type`,
//!   `timestamp`, `sessionID` and a `part` or an `error`; a tool is reported once completed or
//!   failed), `core/src/tool/skill.ts` (the tool `skill`, its input `name`),
//!   `mcp/catalog.ts` (an MCP tool is `<server>_<tool>`, so `purlis_skill_load`) and
//!   `session/processor.ts` (a `step-finish`'s `reason` is the model's finish reason, `stop` at
//!   the end of the turn).
//! - **Claude Code**'s is #5727's.
//!
//! **Not verified**: what a real model does with them, the finish reason opencode's providers
//! map an Anthropic `end_turn` to (`stop` by the AI SDK's own mapping), and which shell a Codex
//! run on the runner names. None of them names the model it ran on but Claude Code.

use std::path::Path;

use super::{EVALS_DIR, TRIGGER_THRESHOLD, TriggerTest};

/// The sources file, beside `triggers.json` in a skill's `evals` folder.
pub const SOURCES_FILE: &str = "triggers.sources.md";

/// The sources file's path in a skill's folder, `/` between the parts.
pub fn sources_path() -> String {
    format!("{EVALS_DIR}/{SOURCES_FILE}")
}

/// **A harness the CI grader runs**, each read from its own machine-readable output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Harness {
    /// `claude -p --output-format stream-json --verbose`.
    ClaudeCode,
    /// `codex exec --json`.
    Codex,
    /// `opencode run --format json`.
    Opencode,
}

impl Harness {
    /// Its word in a result: `claude-code`, `codex`, `opencode`.
    pub fn word(self) -> &'static str {
        match self {
            Self::ClaudeCode => "claude-code",
            Self::Codex => "codex",
            Self::Opencode => "opencode",
        }
    }

    /// Its name, as a verdict says it.
    pub fn name(self) -> &'static str {
        match self {
            Self::ClaudeCode => "Claude Code",
            Self::Codex => "Codex",
            Self::Opencode => "opencode",
        }
    }
}

/// **What else a run's listing held.**
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Listing {
    /// Among the crowd fixture: what the trigger bar is held to.
    Crowded,
    /// purlis's built-in skills alone: reported beside, to tell a wording problem from a
    /// crowding one.
    Bare,
}

/// **A built-in skill's trigger bar**, declared in its sources file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Bar {
    /// At least 95% recall, at most 5% false triggers: where a miss leaks a secret, loses work
    /// or loses context.
    Strict,
    /// At least 85% recall, at most 5% false triggers.
    Standard,
}

impl Bar {
    /// Its word, as the sources file and a verdict write it.
    pub fn word(self) -> &'static str {
        match self {
            Self::Strict => "strict",
            Self::Standard => "standard",
        }
    }

    /// The least recall it holds, in percent.
    pub fn least_recall(self) -> u32 {
        match self {
            Self::Strict => 95,
            Self::Standard => 85,
        }
    }

    /// The most false triggers it allows, in percent.
    pub fn most_false_triggers(self) -> u32 {
        5
    }

    fn parse(word: &str) -> Option<Self> {
        match word {
            "strict" => Some(Self::Strict),
            "standard" => Some(Self::Standard),
            _ => None,
        }
    }
}

/// **The skills a briefed chat was listed**, each with its `SKILL.md`, and the folder it ran in:
/// what a Codex run's read of a `SKILL.md` is matched against
/// ([`crate::runs::started_by_a_read`]). The other harnesses name the skill they start.
#[derive(Debug, Clone, Copy)]
pub struct Briefed<'a> {
    pub cwd: &'a Path,
    pub skills: &'a [crate::Skill],
}

/// **What one recorded run did.**
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Parsed {
    /// Whether it ran to the end of its turn. A run cut short is no run, never a "did not
    /// trigger", as in a grade in the window.
    pub ran: bool,
    /// Each skill it started, once, by its name without a plugin's namespace, in order.
    pub started: Vec<String>,
}

/// purlis's MCP server, as each harness names it.
const PURLIS_SERVER: &str = "purlis";

/// purlis's tool that loads a skill.
const SKILL_LOAD: &str = "skill_load";

/// **What a harness's recorded output says its run started.** A line that is not JSON (a
/// warning a harness printed before it began) is left out.
pub fn parse(harness: Harness, output: &str, briefed: &Briefed<'_>) -> Parsed {
    let mut parsed = Parsed::default();
    for line in output.lines() {
        let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        match harness {
            Harness::ClaudeCode => claude_code(&event, &mut parsed),
            Harness::Codex => codex(&event, briefed, &mut parsed),
            Harness::Opencode => opencode(&event, &mut parsed),
        }
    }
    parsed
}

impl Parsed {
    fn start(&mut self, name: &str) {
        let name = crate::leaf_name(name.trim());
        if !name.is_empty() && !self.started.iter().any(|one| one == name) {
            self.started.push(name.to_owned());
        }
    }
}

/// The string at `key` of `value`.
fn text<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(serde_json::Value::as_str)
}

/// Claude Code's stream-json: each `tool_use` block of an `assistant` message, the Skill tool's
/// `skill` or `skill_load`'s `name`; a `result` of `success` ends the turn.
fn claude_code(event: &serde_json::Value, parsed: &mut Parsed) {
    match text(event, "type") {
        Some("assistant") => {
            let blocks = event
                .pointer("/message/content")
                .and_then(serde_json::Value::as_array);
            for block in blocks.into_iter().flatten() {
                if text(block, "type") != Some("tool_use") {
                    continue;
                }
                let input = block.get("input").unwrap_or(&serde_json::Value::Null);
                let named = match text(block, "name") {
                    Some("Skill") => text(input, "skill"),
                    Some(tool) if tool == format!("mcp__{PURLIS_SERVER}__{SKILL_LOAD}") => {
                        text(input, "name")
                    }
                    _ => None,
                };
                if let Some(name) = named {
                    parsed.start(name);
                }
            }
        }
        Some("result") => {
            parsed.ran = text(event, "subtype") == Some("success")
                && event.get("is_error").and_then(serde_json::Value::as_bool) != Some(true);
        }
        _ => {}
    }
}

/// Codex's `exec --json`: each completed item, a command that plainly reads a briefed skill's
/// `SKILL.md` or purlis's `skill_load`; `turn.completed` ends the turn.
fn codex(event: &serde_json::Value, briefed: &Briefed<'_>, parsed: &mut Parsed) {
    match text(event, "type") {
        Some("item.completed") => {
            let Some(item) = event.get("item") else {
                return;
            };
            match text(item, "type") {
                Some("command_execution") => {
                    let Some(command) = text(item, "command") else {
                        return;
                    };
                    let command = unwrapped(command);
                    if crate::runs::is_a_plain_read(&command)
                        && let Some(skill) =
                            crate::runs::started_by_a_read(&command, briefed.cwd, briefed.skills)
                    {
                        parsed.start(&skill.name);
                    }
                }
                Some("mcp_tool_call")
                    if text(item, "server") == Some(PURLIS_SERVER)
                        && text(item, "tool") == Some(SKILL_LOAD) =>
                {
                    if let Some(name) = item.get("arguments").and_then(|args| text(args, "name")) {
                        parsed.start(name);
                    }
                }
                _ => {}
            }
        }
        Some("turn.completed") => parsed.ran = true,
        Some("turn.failed" | "error") => parsed.ran = false,
        _ => {}
    }
}

/// The command a Codex command line runs in its shell: `bash -lc '<command>'` is `<command>`, its
/// words split and unquoted as the shell would (`shell-words`), so the program that reads or edits
/// is the one held to [`crate::runs::is_a_plain_read`]. Anything else is the command as it came.
fn unwrapped(command: &str) -> std::borrow::Cow<'_, str> {
    if let Ok(words) = shell_words::split(command)
        && let [shell, flag, inner] = words.as_slice()
        && matches!(shell.rsplit('/').next(), Some("bash" | "sh" | "zsh"))
        && matches!(flag.as_str(), "-c" | "-lc")
    {
        return inner.clone().into();
    }
    command.into()
}

/// opencode's `run --format json`: each `tool_use`, its `skill` tool's `name` or purlis's
/// `purlis_skill_load`; a `step_finish` whose reason is `stop` ends the turn, and an `error`
/// cuts it short.
fn opencode(event: &serde_json::Value, parsed: &mut Parsed) {
    match text(event, "type") {
        Some("tool_use") => {
            let Some(part) = event.get("part") else {
                return;
            };
            let is_a_start = match text(part, "tool") {
                Some(tool) => {
                    tool.eq_ignore_ascii_case("skill")
                        || tool == format!("{PURLIS_SERVER}_{SKILL_LOAD}")
                }
                None => false,
            };
            if is_a_start
                && let Some(name) = part.pointer("/state/input/name").and_then(|v| v.as_str())
            {
                parsed.start(name);
            }
        }
        Some("step_finish") => {
            if event.pointer("/part/reason").and_then(|v| v.as_str()) == Some("stop") {
                parsed.ran = true;
            }
        }
        Some("error") => parsed.ran = false,
        _ => {}
    }
}

/// **A built-in skill's sources file, read**: what it is held to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sources {
    pub bar: Bar,
    /// The skills whose requests its near misses are, and its "Not for" clauses name.
    pub neighbours: Vec<String>,
    /// Where each trigger test's query came from, in the order of its trigger tests.
    pub notes: Vec<String>,
}

/// **A sources file's text, read against `tests` trigger tests**, or a sentence saying what is
/// wrong with it. The file is Markdown a reviewer reads, holding three things purlis reads:
///
/// ```markdown
/// Trigger bar: strict
/// Neighbours: purlis-browser, purlis-persona
///
/// 1. Mined transcript, paraphrased and scrubbed.
/// 2. A correction the person made.
/// ```
///
/// one numbered note per trigger test, in the order of `triggers.json`. Every other line (a
/// heading, a paragraph) is the reviewer's.
pub fn sources_of(text: &str, tests: usize) -> Result<Sources, String> {
    let mut bar = None;
    let mut neighbours = None;
    let mut notes: Vec<String> = Vec::new();
    for line in text.lines().map(str::trim) {
        if let Some(word) = line.strip_prefix("Trigger bar:") {
            let word = word.trim();
            if bar.is_some() {
                return Err("it names its trigger bar twice".to_owned());
            }
            bar = Some(Bar::parse(word).ok_or_else(|| {
                format!("its trigger bar is `{word}`, which is neither strict nor standard")
            })?);
        } else if let Some(names) = line.strip_prefix("Neighbours:") {
            if neighbours.is_some() {
                return Err("it names its neighbours twice".to_owned());
            }
            let names = names.trim();
            neighbours = Some(if names == "none" {
                Vec::new()
            } else {
                names
                    .split(',')
                    .map(str::trim)
                    .filter(|name| !name.is_empty())
                    .map(str::to_owned)
                    .collect()
            });
        } else if let Some((number, note)) = numbered(line) {
            let should = notes.len() + 1;
            if number != should {
                return Err(format!(
                    "source note {number} comes where note {should} should"
                ));
            }
            if note.is_empty() {
                return Err(format!("source note {number} is empty"));
            }
            notes.push(note.to_owned());
        }
    }
    let bar = bar.ok_or(
        "it names no trigger bar (a line `Trigger bar: strict` or `Trigger bar: standard`)",
    )?;
    let neighbours = neighbours
        .ok_or("it names no neighbours (a line `Neighbours: ...`, or `Neighbours: none`)")?;
    if notes.len() != tests {
        let noun = if notes.len() == 1 { "note" } else { "notes" };
        return Err(format!(
            "it has {} source {noun} for {tests} trigger tests",
            notes.len()
        ));
    }
    Ok(Sources {
        bar,
        neighbours,
        notes,
    })
}

/// A line `<n>. <note>`: its number and its note.
fn numbered(line: &str) -> Option<(usize, &str)> {
    let (number, note) = line.split_once('.')?;
    let number = number.parse().ok()?;
    Some((number, note.trim()))
}

/// **A built-in skill to score**: its trigger tests and its sources file's text, `None` where it
/// has none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Graded {
    pub skill: String,
    pub tests: Vec<TriggerTest>,
    pub sources: Option<String>,
}

/// **One run's recorded output**: of `skill`'s trigger test `test` (its place in
/// `triggers.json`), in `listing`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recorded {
    pub skill: String,
    pub test: usize,
    pub listing: Listing,
    pub output: String,
}

/// **One harness's grade**: the skills it graded, what each run of their tests wrote, and what
/// it ran as.
#[derive(Debug, Clone, Copy)]
pub struct Grade<'a> {
    pub harness: Harness,
    pub version: &'a str,
    pub model: &'a str,
    pub briefed: Briefed<'a>,
    /// Every built-in skill's name: a start of one that is not a query's own skill is a wrong
    /// built-in.
    pub built_ins: &'a [String],
    pub skills: &'a [Graded],
    pub runs: &'a [Recorded],
}

/// `count` of `of`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Rate {
    pub count: u32,
    pub of: u32,
}

impl Rate {
    /// In whole percent, rounded down; none of nothing.
    pub fn percent(self) -> Option<u32> {
        (self.of > 0).then(|| self.count * 100 / self.of)
    }

    /// Whether it is at least `percent`, exactly: none of nothing is not.
    fn at_least(self, percent: u32) -> bool {
        self.of > 0 && u64::from(self.count) * 100 >= u64::from(percent) * u64::from(self.of)
    }

    /// Whether it is at most `percent`, exactly: none of nothing is.
    fn at_most(self, percent: u32) -> bool {
        u64::from(self.count) * 100 <= u64::from(percent) * u64::from(self.of)
    }

    fn said(self) -> String {
        format!(
            "{}% ({} of {})",
            self.percent().unwrap_or(0),
            self.count,
            self.of
        )
    }
}

/// **One query, scored for one skill in one listing.**
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PromptScore {
    pub query: String,
    /// The skill whose trigger test it is: another's, where this skill took a query of a graded
    /// built-in's that should have started that one.
    pub owner: String,
    /// Whether this skill should start on it.
    pub should_trigger: bool,
    /// The runs that ran to the end of their turn.
    pub runs: u32,
    /// Of those, the runs that started this skill.
    pub triggered: u32,
    /// `triggered / runs`, or `None` where nothing ran.
    pub rate: Option<f64>,
    /// Whether it passed: a query nothing ran for did not.
    pub pass: bool,
    /// The other built-ins at least half its runs started.
    pub started_instead: Vec<String>,
}

/// **A skill's score in one listing.**
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ListingScore {
    pub listing: Listing,
    /// Its queries that should start it and did.
    pub recall: Rate,
    /// Its queries that should not start it and did, with another graded built-in's queries it
    /// took, of all those.
    pub false_triggers: Rate,
    /// Its own trigger tests in order, then the other skills' queries it took.
    pub prompts: Vec<PromptScore>,
}

/// **A skill held to its trigger bar** among the crowd.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Verdict {
    pub passed: bool,
    pub bar: Bar,
    pub recall: Rate,
    pub false_triggers: Rate,
    /// The queries that failed among the crowd, in the order they are scored.
    pub failing: Vec<String>,
    /// The verdict in words: the skill, the harness, the bar, the score and the failing prompts.
    pub said: String,
}

/// **One skill's score.**
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SkillScore {
    pub skill: String,
    pub bar: Bar,
    pub crowded: ListingScore,
    /// Where the grade ran it bare too.
    pub bare: Option<ListingScore>,
    pub verdict: Verdict,
}

/// **One skill of a grade**: scored, or why it was not.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum SkillGrade {
    Scored(SkillScore),
    NotScored { skill: String, why: String },
}

/// **One harness's grade, scored**: each skill in the order it was given, and what it ran as.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Scorecard {
    pub harness: Harness,
    pub version: String,
    pub model: String,
    pub skills: Vec<SkillGrade>,
}

/// **A harness's grade, scored**: each skill's recall and false triggers among the crowd and
/// bare, held to the trigger bar its sources file declares.
pub fn score(grade: &Grade<'_>) -> Scorecard {
    // Every run read once: by skill, test and listing.
    let read: Vec<(&Recorded, Parsed)> = grade
        .runs
        .iter()
        .map(|run| (run, parse(grade.harness, &run.output, &grade.briefed)))
        .collect();
    let held: Vec<Result<Sources, String>> = grade.skills.iter().map(held_to).collect();
    let skills = grade
        .skills
        .iter()
        .zip(&held)
        .map(|(graded, sources)| match sources {
            Err(why) => SkillGrade::NotScored {
                skill: graded.skill.clone(),
                why: format!("{} is not scored: {why}.", graded.skill),
            },
            Ok(sources) => {
                let in_listing = |listing| scored_in(grade, &read, graded, listing);
                let crowded = in_listing(Listing::Crowded);
                let has_bare = read
                    .iter()
                    .any(|(run, _)| run.listing == Listing::Bare && run.skill == graded.skill);
                let bare = has_bare.then(|| in_listing(Listing::Bare));
                let verdict = verdict(grade, &graded.skill, sources.bar, &crowded);
                SkillGrade::Scored(SkillScore {
                    skill: graded.skill.clone(),
                    bar: sources.bar,
                    crowded,
                    bare,
                    verdict,
                })
            }
        })
        .collect();
    Scorecard {
        harness: grade.harness,
        version: grade.version.to_owned(),
        model: grade.model.to_owned(),
        skills,
    }
}

/// What `graded` is held to, or why it cannot be scored.
fn held_to(graded: &Graded) -> Result<Sources, String> {
    if graded.tests.is_empty() {
        return Err("it has no trigger tests".to_owned());
    }
    let path = sources_path();
    let text = graded
        .sources
        .as_deref()
        .ok_or_else(|| format!("it has no {path} beside its trigger tests"))?;
    sources_of(text, graded.tests.len()).map_err(|why| format!("its {path} {}", why_after(&why)))
}

/// A sources refusal, `it ...`, as it reads after `its <file>`.
fn why_after(why: &str) -> &str {
    why.strip_prefix("it ").unwrap_or(why)
}

/// `graded`'s queries, and the other graded skills' that should have started them, scored for
/// `graded` in `listing`.
fn scored_in(
    grade: &Grade<'_>,
    read: &[(&Recorded, Parsed)],
    graded: &Graded,
    listing: Listing,
) -> ListingScore {
    let skill = graded.skill.as_str();
    let runs_of = |owner: &str, test: usize| -> Vec<&Parsed> {
        read.iter()
            .filter(|(run, parsed)| {
                parsed.ran && run.listing == listing && run.skill == owner && run.test == test
            })
            .map(|(_, parsed)| parsed)
            .collect()
    };
    let mut prompts: Vec<PromptScore> = graded
        .tests
        .iter()
        .enumerate()
        .map(|(n, test)| {
            prompt(
                grade,
                skill,
                skill,
                &test.query,
                test.should_trigger,
                &runs_of(skill, n),
            )
        })
        .collect();
    // The other graded skills' queries that should have started them, and started this one.
    for other in grade.skills.iter().filter(|other| other.skill != skill) {
        for (n, test) in other.tests.iter().enumerate() {
            if !test.should_trigger {
                continue;
            }
            let taken = prompt(
                grade,
                skill,
                &other.skill,
                &test.query,
                false,
                &runs_of(&other.skill, n),
            );
            if taken.rate.is_some_and(|rate| rate >= TRIGGER_THRESHOLD) {
                prompts.push(taken);
            }
        }
    }
    // A query no run of ran is in neither rate: the verdict fails on it by itself.
    let count = |should: bool, passed: bool| {
        let n = prompts
            .iter()
            .filter(|one| one.runs > 0 && one.should_trigger == should && one.pass == passed)
            .count();
        u32::try_from(n).unwrap_or(u32::MAX)
    };
    let positives = count(true, true) + count(true, false);
    let negatives = count(false, true) + count(false, false);
    ListingScore {
        listing,
        recall: Rate {
            count: count(true, true),
            of: positives,
        },
        false_triggers: Rate {
            count: count(false, false),
            of: negatives,
        },
        prompts,
    }
}

/// One query scored for `skill`, over the runs of it that ran.
fn prompt(
    grade: &Grade<'_>,
    skill: &str,
    owner: &str,
    query: &str,
    should_trigger: bool,
    runs: &[&Parsed],
) -> PromptScore {
    let share = |name: &str| {
        let started = runs
            .iter()
            .filter(|one| one.started.iter().any(|it| it == name))
            .count();
        (!runs.is_empty()).then(|| started as f64 / runs.len() as f64)
    };
    let triggered = runs
        .iter()
        .filter(|one| one.started.iter().any(|it| it == skill))
        .count();
    let rate = share(skill);
    let started_instead = grade
        .built_ins
        .iter()
        .filter(|name| *name != skill)
        .filter(|name| share(name).is_some_and(|rate| rate >= TRIGGER_THRESHOLD))
        .cloned()
        .collect();
    PromptScore {
        query: query.to_owned(),
        owner: owner.to_owned(),
        should_trigger,
        runs: u32::try_from(runs.len()).unwrap_or(u32::MAX),
        triggered: u32::try_from(triggered).unwrap_or(u32::MAX),
        rate,
        pass: rate.is_some_and(|rate| (rate >= TRIGGER_THRESHOLD) == should_trigger),
        started_instead,
    }
}

/// `skill` held to `bar` by its crowded score.
fn verdict(grade: &Grade<'_>, skill: &str, bar: Bar, crowded: &ListingScore) -> Verdict {
    let passed = crowded.recall.at_least(bar.least_recall())
        && crowded.false_triggers.at_most(bar.most_false_triggers())
        && crowded.prompts.iter().all(|one| one.runs > 0);
    let failing: Vec<&PromptScore> = crowded.prompts.iter().filter(|one| !one.pass).collect();
    let mut said = format!(
        "{skill} on {} {} ({}) {} its {} trigger bar (at least {}% recall, at most {}% false \
         triggers): among the crowd it recalled {} and falsely triggered on {}.",
        grade.harness.name(),
        grade.version,
        grade.model,
        if passed { "meets" } else { "is below" },
        bar.word(),
        bar.least_recall(),
        bar.most_false_triggers(),
        crowded.recall.said(),
        crowded.false_triggers.said(),
    );
    if !failing.is_empty() {
        said.push_str(" Failing prompts:");
        for one in &failing {
            said.push_str(&format!(
                "\n- \"{}\" ({})",
                one.query,
                why_failed(one, skill)
            ));
        }
    }
    Verdict {
        passed,
        bar,
        recall: crowded.recall,
        false_triggers: crowded.false_triggers,
        failing: failing.iter().map(|one| one.query.clone()).collect(),
        said,
    }
}

/// Why a query failed for `skill`, in a few words.
fn why_failed(one: &PromptScore, skill: &str) -> String {
    if one.runs == 0 {
        "no run ran to the end of its turn".to_owned()
    } else if one.owner != skill {
        format!("false trigger; it is {}'s", one.owner)
    } else if !one.should_trigger {
        "false trigger".to_owned()
    } else if one.started_instead.is_empty() {
        "missed".to_owned()
    } else {
        format!("missed; started {}", one.started_instead.join(", "))
    }
}

#[cfg(test)]
mod tests;
