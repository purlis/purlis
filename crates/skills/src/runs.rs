// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Skill runs** (SK-3, #2766; spec #2762): every use of a skill is a span, from the moment the
//! skill starts to the end of the turn.
//!
//! This module is the span's bookkeeping, and knows no harness and no log. A host tells it a
//! skill started in a chat's run ([`Spans::open`]), what happened in that run while a span is
//! open ([`Spans::heard`]), and that the turn ended ([`Spans::close`]); it answers which skill an
//! entry is inside ([`Spans::inside`]) and, at the close, each span with its [`Signals`]. A span
//! opened while another is open is its child. A run shows signals only, never a verdict: pass or
//! fail comes from grading alone.
//!
//! How a harness shows that a skill started is its adapter's to read. The two neutral readings
//! a harness adapter leans on are here: [`started_by_a_read`], for a harness briefed on its
//! skills (the [`crate::Route::Briefing`] route), whose chat starts a skill by reading its
//! `SKILL.md`, and [`is_a_plain_read`], which keeps a command that changes a file from counting
//! as one.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::{Origin, Skill};

/// **What started a skill run** (decision 60): the harness's own pick, by the route its adapter
/// reads ([`StartedBy::SkillTool`], [`StartedBy::Read`]) or purlis's MCP server says
/// ([`StartedBy::McpLoad`]); the person, with Use skill…; a curation action; or a trigger. A run
/// a curation action's or a trigger's prompt opened is said to be theirs whichever route the
/// harness took, as that prompt is what asked for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum StartedBy {
    /// The harness's own Skill tool, as the chat's adapter reads it.
    SkillTool,
    /// The chat read a known `SKILL.md`: how a chat briefed on its skills starts one, as the
    /// chat's adapter reads it.
    Read,
    /// The chat loaded the skill through purlis's own `skill_load` tool (SK-13): an exact start
    /// on every harness, said by purlis's MCP server rather than read from the harness.
    McpLoad,
    /// The person picked it in Use skill…, which typed it into the chat, and sent that prompt
    /// (SK-12): the run opens as the prompt does.
    UseSkill,
    /// A curation action typed the prompt that started it, and the person sent it (SK-20): the
    /// harness started the skill in that prompt's turn, by whichever route it takes.
    Curation,
    /// A trigger sent the prompt that started it (AC-9), by the route the harness took.
    Trigger,
    /// A grade sent the prompt that started it (SK-16): a trigger test's chat, by the route the
    /// harness took. Its run is the grade's evidence, not a use of the skill.
    Grade,
    /// A start a newer purlis wrote, which this one does not know. It never opens a run.
    #[serde(other)]
    Unknown,
}

impl StartedBy {
    /// The word a run records: `skill-tool`, `read`, `mcp-load`, `use-skill`, `curation`,
    /// `trigger`, `grade` or `unknown`.
    pub fn word(self) -> &'static str {
        match self {
            Self::SkillTool => "skill-tool",
            Self::Read => "read",
            Self::McpLoad => "mcp-load",
            Self::UseSkill => "use-skill",
            Self::Curation => "curation",
            Self::Trigger => "trigger",
            Self::Grade => "grade",
            Self::Unknown => "unknown",
        }
    }

    /// The start `word` names, or none.
    pub fn parse(word: &str) -> Option<Self> {
        Some(match word {
            "skill-tool" => Self::SkillTool,
            "read" => Self::Read,
            "mcp-load" => Self::McpLoad,
            "use-skill" => Self::UseSkill,
            "curation" => Self::Curation,
            "trigger" => Self::Trigger,
            "grade" => Self::Grade,
            _ => return None,
        })
    }
}

/// How a span came to its end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum Ended {
    /// The turn it started in ended.
    Turn,
    /// A new prompt came before the turn said it ended: the person interrupted it.
    Interrupted,
    /// The chat's session or run ended under it.
    Session,
    /// An end a newer purlis wrote, which this one does not know.
    #[serde(other)]
    Unknown,
}

impl Ended {
    /// The word a run records: `turn`, `interrupted`, `session` or `unknown`.
    pub fn word(self) -> &'static str {
        match self {
            Self::Turn => "turn",
            Self::Interrupted => "interrupted",
            Self::Session => "session",
            Self::Unknown => "unknown",
        }
    }

    /// The end `word` names, or none.
    pub fn parse(word: &str) -> Option<Self> {
        Some(match word {
            "turn" => Self::Turn,
            "interrupted" => Self::Interrupted,
            "session" => Self::Session,
            _ => return None,
        })
    }
}

/// **Which copy of a skill a run is of**: its level, and whether it is the local copy (ADR 0099).
/// With the workspace the chat worked in, it tells one copy's runs from another's of the same
/// name, so a chat is shown the runs of its own copies alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Level {
    pub origin: Origin,
    pub local: bool,
}

impl Level {
    /// The word a run records, the level a person is shown ([`crate::level_shown`]):
    /// `built-in`, `installed`, `project`, `persona`, `workspace`, and `local …` of a level that
    /// has local skills.
    pub fn word(self) -> String {
        crate::level_shown(self.origin, self.local)
    }

    /// The level `word` names, or none for a word this purlis does not know (a newer purlis's).
    pub fn parse(word: &str) -> Option<Self> {
        let (local, rest) = match word.strip_prefix("local ") {
            Some(rest) => (true, rest),
            None => (false, word),
        };
        let origin = match rest {
            "built-in" if !local => Origin::BuiltIn,
            "installed" if !local => Origin::Harness,
            "project" => Origin::Project,
            "workspace" => Origin::Workspace,
            "persona" => Origin::Persona,
            _ => return None,
        };
        Some(Self { origin, local })
    }
}

impl serde::Serialize for Level {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.word())
    }
}

/// Reads a level's word as [`Level::parse`] does, for a `#[serde(deserialize_with)]` field: a
/// word this purlis does not know is no level, never a line refused.
pub fn level_or_none<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Level>, D::Error> {
    let word: Option<String> = serde::Deserialize::deserialize(deserializer)?;
    Ok(word.as_deref().and_then(Level::parse))
}

/// Where a skill run happened and which copy it is of: what a run's start carries so that its
/// runs are told apart by copy (SK-13).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Whose {
    /// The workspace the chat worked in, or `None` for a chat at the plane root.
    pub workspace: Option<String>,
    /// The copy of the skill, where the start knew it; `None` for one it could not tell (a
    /// harness's own skill), whose runs belong to no copy of the plane's.
    pub level: Option<Level>,
}

/// What a span counted while it was open: signals, never a pass or a fail (decision 24).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Signals {
    /// Tool calls made inside it, each counted once whichever of its hooks were heard.
    pub tool_calls: u32,
    /// Tool calls that failed.
    pub tool_errors: u32,
    /// Tool calls a guard refused.
    pub refusals: u32,
    /// Times the chat asked the person: a tool call put to them, or a question it raised.
    pub asks: u32,
}

/// A tool call heard, by the harness's id for it where it gave one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Call<'a> {
    /// The harness's id: a call's two hooks count once.
    Id(&'a str),
    /// No id: each hook counts.
    Unnamed,
}

/// One thing heard inside a chat's run while a span may be open.
#[derive(Debug, Clone, Copy, Default)]
pub struct Heard<'a> {
    /// A tool call, where it was one.
    pub call: Option<Call<'a>>,
    /// The call failed.
    pub failed: bool,
    /// A guard refused the call.
    pub refused: bool,
    /// The person was asked.
    pub asked: bool,
}

/// One open span.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Open {
    /// The skill, by the name the harness started it by.
    pub skill: String,
    /// The run's id.
    pub id: String,
    /// The span it opened inside, where it is a child.
    pub parent: Option<String>,
    pub started_by: StartedBy,
    /// Where it ran, and which copy of the skill it is of.
    pub whose: Whose,
    /// When it opened, in milliseconds since 1970.
    pub at_ms: u64,
    /// The chat's tokens as its harness last said them when it opened, where it said any.
    pub tokens_at_start: Option<u64>,
    pub signals: Signals,
    /// The tool calls counted, by id, so a call's two hooks count once.
    calls: HashSet<String>,
}

/// One span, closed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Closed {
    pub open: Open,
    pub ended: Ended,
    /// How long it was open, in milliseconds.
    pub ms: u64,
}

/// What [`Spans::open`] is told of a skill that started.
#[derive(Debug, Clone, Copy)]
pub struct Start<'a> {
    pub skill: &'a str,
    pub id: &'a str,
    pub started_by: StartedBy,
    pub whose: &'a Whose,
    pub at_ms: u64,
    pub tokens: Option<u64>,
}

/// Every open span, by the chat run it is in.
#[derive(Debug, Default)]
pub struct Spans {
    open: HashMap<String, Vec<Open>>,
}

impl Spans {
    /// **Opens a span for `start` in chat run `run`**, the child of the innermost span open
    /// there, and answers it. `None` where a span of that skill is already open in the run (one
    /// turn starts a skill once, however many times it is read), and for a start it does not
    /// know.
    pub fn open(&mut self, run: &str, start: Start<'_>) -> Option<&Open> {
        if start.started_by == StartedBy::Unknown {
            return None;
        }
        let stack = self.open.entry(run.to_owned()).or_default();
        if stack.iter().any(|open| open.skill == start.skill) {
            return None;
        }
        let parent = stack.last().map(|open| open.id.clone());
        stack.push(Open {
            skill: start.skill.to_owned(),
            id: start.id.to_owned(),
            parent,
            started_by: start.started_by,
            whose: start.whose.clone(),
            at_ms: start.at_ms,
            tokens_at_start: start.tokens,
            signals: Signals::default(),
            calls: HashSet::new(),
        });
        stack.last()
    }

    /// Whether a span of `skill` is open in chat run `run`.
    pub fn holds(&self, run: &str, skill: &str) -> bool {
        self.open
            .get(run)
            .is_some_and(|stack| stack.iter().any(|open| open.skill == skill))
    }

    /// The innermost span open in chat run `run`: the skill an entry there is inside.
    pub fn inside(&self, run: &str) -> Option<&Open> {
        self.open.get(run).and_then(|stack| stack.last())
    }

    /// Counts `heard` onto every span open in chat run `run`: a child's call is its parent's too.
    pub fn heard(&mut self, run: &str, heard: Heard<'_>) {
        let Some(stack) = self.open.get_mut(run) else {
            return;
        };
        for open in stack {
            let first = match heard.call {
                None => false,
                Some(Call::Unnamed) => true,
                Some(Call::Id(id)) => open.calls.insert(id.to_owned()),
            };
            open.signals.tool_calls += u32::from(first);
            open.signals.tool_errors += u32::from(heard.failed);
            open.signals.refusals += u32::from(heard.refused);
            open.signals.asks += u32::from(heard.asked);
        }
    }

    /// **Closes every span open in chat run `run`**, innermost first, as `ended` at `at_ms`.
    pub fn close(&mut self, run: &str, ended: Ended, at_ms: u64) -> Vec<Closed> {
        let mut stack = self.open.remove(run).unwrap_or_default();
        stack.reverse();
        stack
            .into_iter()
            .map(|open| Closed {
                ms: at_ms.saturating_sub(open.at_ms),
                open,
                ended,
            })
            .collect()
    }
}

/// **The skill a shell command or a file read starts by reading its `SKILL.md`**, of `skills`,
/// the chat's set: the first whose `SKILL.md` `read` names, by its whole path or by its path
/// from `cwd`. What a briefed harness's chat does to use a skill (decision 9). A caller holds a
/// command to [`is_a_plain_read`] first.
pub fn started_by_a_read<'a>(read: &str, cwd: &Path, skills: &'a [Skill]) -> Option<&'a Skill> {
    skills.iter().find(|skill| {
        let whole = skill.path.to_string_lossy();
        named(read, &whole)
            || skill
                .path
                .strip_prefix(cwd)
                .ok()
                .is_some_and(|from_cwd| named(read, &from_cwd.to_string_lossy()))
    })
}

/// Programs that change files or the repo: a command that runs one is not a read of a skill,
/// whatever file it names. A chat editing a skill does not start it.
const CHANGES_FILES: &[&str] = &[
    "git",
    "rm",
    "cp",
    "mv",
    "tee",
    "touch",
    "truncate",
    "dd",
    "install",
    "ln",
    "chmod",
    "chown",
    "mkdir",
    "rmdir",
    "vi",
    "vim",
    "nvim",
    "nano",
    "emacs",
    "ed",
    "ex",
    "code",
    "patch",
    "apply_patch",
];

/// **Whether `command` only reads**: no program of [`CHANGES_FILES`] runs in it, no `sed` or
/// `perl` edits in place, and nothing is redirected into a file (`>` or `>>`, other than to
/// `/dev/null` or another stream). The neutral half of a briefed chat's read (decision 9): a
/// plain read starts the skill, and an edit of it never does.
pub fn is_a_plain_read(command: &str) -> bool {
    !writes_by_redirection(command)
        && command
            .split(['|', ';', '&', '\n', '(', ')', '`'])
            .all(|segment| !segment_changes_files(segment))
}

/// Whether one simple command of a line changes files.
fn segment_changes_files(segment: &str) -> bool {
    let mut words = segment
        .split_whitespace()
        .map(|word| word.trim_matches(['"', '\'']))
        // Variables set for the command, and the wrappers that run one.
        .skip_while(|word| {
            word.contains('=') || matches!(*word, "sudo" | "env" | "command" | "exec")
        });
    let Some(program) = words.next() else {
        return false;
    };
    let program = program.rsplit('/').next().unwrap_or(program);
    if CHANGES_FILES.contains(&program) {
        return true;
    }
    let mut rest = words;
    match program {
        "sed" | "gsed" => {
            rest.any(|arg| arg.starts_with("--in-place") || short_flags_hold(arg, 'i'))
        }
        "perl" => rest.any(|arg| short_flags_hold(arg, 'i')),
        _ => false,
    }
}

/// Whether `arg` is a cluster of short flags holding `flag` (`-i`, `-ni`, `-pi.bak`).
fn short_flags_hold(arg: &str, flag: char) -> bool {
    arg.strip_prefix('-')
        .is_some_and(|flags| !flags.starts_with('-') && flags.contains(flag))
}

/// Whether `command` redirects into a file: a `>` or `>>` whose target is not `/dev/null` or
/// another stream (`&1`).
fn writes_by_redirection(command: &str) -> bool {
    let mut rest = command;
    while let Some(at) = rest.find('>') {
        let after = rest[at..].trim_start_matches('>');
        let target = after.trim_start();
        let into_a_stream = target.starts_with('&');
        let into_nothing = target.starts_with("/dev/null");
        if !into_a_stream && !into_nothing {
            return true;
        }
        rest = after;
    }
    false
}

/// Whether `text` names `path` as a word of its own, not as part of a longer path: between the
/// edges of a shell word, or after `./`.
fn named(text: &str, path: &str) -> bool {
    let edge = |c: Option<char>| {
        c.is_none_or(|c| {
            c.is_whitespace()
                || matches!(
                    c,
                    '"' | '\'' | '`' | '=' | ';' | '|' | '&' | '(' | ')' | '<' | '>' | ','
                )
        })
    };
    !path.is_empty()
        && text.match_indices(path).any(|(at, _)| {
            let before = &text[..at];
            (edge(before.chars().next_back())
                || before
                    .strip_suffix("./")
                    .is_some_and(|rest| edge(rest.chars().next_back())))
                && edge(text[at + path.len()..].chars().next())
        })
}
