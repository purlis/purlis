// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The files, as Rust reads them.** `schema/cells.schema.json` and `schema/leg-report.schema.json`
//! are what a file must be; these types are how the suite reads one that passed. Where the two
//! could disagree, the schema is the contract, and `every_cells_file_validates` reads every
//! cells file through both.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// **A batch**: cells run in order in one chat (or two), which may depend on earlier cells'
/// state. Ticket 54 shuffles batches, never the cells inside one.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Batch {
    #[serde(rename = "$schema", default)]
    pub schema: Option<String>,
    pub batch: String,
    #[serde(default)]
    pub title: Option<String>,
    pub protection: Protection,
    pub persona: String,
    /// `$h1` and the like, bound to `.test` origins in the suite and to real hosts live.
    pub hosts: BTreeMap<String, String>,
    /// Rule ids refused by design with no new block (matrix-check's black-hole exemption).
    #[serde(default)]
    pub deny_exempt_rules: Vec<String>,
    pub cells: Vec<Cell>,
}

impl Batch {
    /// A batch from a value that already passed the schema.
    pub fn from_value(value: Value) -> Result<Self, serde_json::Error> {
        serde_json::from_value(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Protection {
    None,
    Basic,
    Full,
}

/// **A harness's dialect**: the hook words and payloads it speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Dialect {
    Claude,
    Codex,
    Opencode,
}

impl Dialect {
    pub const ALL: [Dialect; 3] = [Dialect::Claude, Dialect::Codex, Dialect::Opencode];

    pub fn word(self) -> &'static str {
        match self {
            Dialect::Claude => "claude",
            Dialect::Codex => "codex",
            Dialect::Opencode => "opencode",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Os {
    Linux,
    Macos,
}

/// **Where a cell runs**: the fake harness in the scenario host, the real Claude Code binary
/// under its own sandbox (ticket 10), the real harnesses under purlis's wrap (ticket 53), or
/// the live pass (ticket 44).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Leg {
    Fake,
    ClaudeSandbox,
    RealWrap,
    Live,
}

/// Scaled `Timings` (×0.5 or more) or `Timings::production()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimingsMode {
    Scaled,
    Production,
}

fn all_dialects() -> Vec<Dialect> {
    Dialect::ALL.to_vec()
}

/// **A cell**: one matrix row's steps and expectations, written once for every dialect.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cell {
    /// The matrix row (`M-NET-05`), or an escape, control or just-inside entry's id.
    pub id: String,
    /// An escape cell: its failure means the refused action happened, so it carries no
    /// `expected_fail` (Q301).
    #[serde(default)]
    pub escape: bool,
    #[serde(default = "all_dialects")]
    pub dialects: Vec<Dialect>,
    /// What a dialect shows where it lacks an event the cell names, and its own steps' wording.
    #[serde(default)]
    pub per_dialect: BTreeMap<Dialect, Override>,
    #[serde(default)]
    pub per_os: BTreeMap<Os, Override>,
    #[serde(default)]
    pub live_only: Option<LiveOnly>,
    /// One mark per failing assertion and issue. Any failure not listed is red.
    #[serde(default)]
    pub expected_fail: Vec<Mark>,
    /// The `production_only` pair (ticket 1) this cell hinges on: it runs only at production
    /// timings, never scaled.
    #[serde(default)]
    pub production_only: Option<[String; 2]>,
    /// The ruling that keeps this cell to the nightly production run (today Q300's M-NET-25).
    #[serde(default)]
    pub production_nightly_only: Option<String>,
    pub steps: Vec<Step>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LiveOnly {
    pub reason: String,
}

/// **An override for one dialect or one OS.** `shows` says, for an event kind the dialect
/// lacks, what it shows instead (an empty list: nothing), applied to every step; `steps`
/// replaces whole expectation lists of one step, keyed by its number from 1. The pins files
/// (ticket 3) carry `shows` as a dialect's defaults; a cell's own `shows` wins over them.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Override {
    #[serde(default)]
    pub shows: BTreeMap<String, Vec<Expect>>,
    #[serde(default)]
    pub steps: BTreeMap<String, StepPatch>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StepPatch {
    #[serde(default)]
    pub require: Option<Vec<Require>>,
    #[serde(default)]
    pub forbid: Option<Vec<Expect>>,
    #[serde(default)]
    pub window: Option<Vec<WindowQuery>>,
    #[serde(default)]
    pub transcript: Option<Vec<Sentence>>,
}

/// **A pins file** (`pins/<dialect>.json`, ticket 3): one dialect's defaults, in the shape of
/// a cell's `per_dialect` entry.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PinsFile {
    #[serde(rename = "$schema", default)]
    pub schema: Option<String>,
    pub dialect: Dialect,
    #[serde(default)]
    pub shows: BTreeMap<String, Vec<Expect>>,
    /// The recording these defaults were drawn from, and the facts it shows that no event names
    /// (ticket 3).
    #[serde(default)]
    pub recorded: Option<Recorded>,
}

/// Where a pins file's defaults were drawn from (ticket 3).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recorded {
    pub from: String,
    pub facts: BTreeMap<String, String>,
}

/// **A step**: what the chat is made to do, what the scripted person does, and what must and
/// must not follow.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    /// Which chat, in a two-chat cell.
    #[serde(default)]
    pub chat: Option<String>,
    #[serde(default)]
    pub calls: Vec<Call>,
    #[serde(default)]
    pub outcome: Option<Outcome>,
    #[serde(default)]
    pub refusal_expected: bool,
    #[serde(default)]
    pub repeat_of: Option<StepRef>,
    /// A Timings expression ("hold+1").
    #[serde(default)]
    pub wait: Option<String>,
    #[serde(default)]
    pub press: Option<Press>,
    #[serde(default)]
    pub restart: Option<Restart>,
    #[serde(default)]
    pub require: Vec<Require>,
    #[serde(default)]
    pub forbid: Vec<Expect>,
    #[serde(default)]
    pub window: Vec<WindowQuery>,
    #[serde(default)]
    pub transcript: Vec<Sentence>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Call {
    pub tool: String,
    pub input: Value,
    #[serde(default)]
    pub client: Option<String>,
    /// The harness's or the client's own timeout for the call ("150s").
    #[serde(default)]
    pub timeout: Option<String>,
    #[serde(default)]
    pub parallel: bool,
    #[serde(default)]
    pub background: bool,
    #[serde(default)]
    pub subagent: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    CameBack,
    Failed,
    Denied,
    NoPost,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StepRef {
    pub cell: String,
    pub step: u32,
}

/// **The scripted person's press** (ticket 7): a button, or a window action by name.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Press {
    Button(Button),
    Action(String),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Button {
    pub label: String,
    pub target: String,
    pub when: String,
    /// A Timings expression: how long after `when` the press comes.
    #[serde(default)]
    pub delay: Option<String>,
    #[serde(default)]
    pub then: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Restart {
    Quit,
    Window,
    HostKilled,
}

/// **A `require` entry**: one event, one of several, or two events in order.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Require {
    Either { either: Vec<Expect> },
    Order { order: [Expect; 2] },
    Event(Expect),
}

impl Require {
    /// Every event this entry names.
    pub fn events(&self) -> Vec<&Expect> {
        match self {
            Require::Either { either } => either.iter().collect(),
            Require::Order { order } => order.iter().collect(),
            Require::Event(e) => vec![e],
        }
    }

    pub fn events_mut(&mut self) -> Vec<&mut Expect> {
        match self {
            Require::Either { either } => either.iter_mut().collect(),
            Require::Order { order } => order.iter_mut().collect(),
            Require::Event(e) => vec![e],
        }
    }
}

/// **One event expectation**: its kind, how many, where in the step, and any body fields it
/// must carry (`"target.host": "$h1"`; `null` for a field that must be absent).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Expect {
    pub kind: String,
    /// An exact number, or `≤N`, `≥N`, `N..M`.
    #[serde(default)]
    pub count: Option<Value>,
    #[serde(default)]
    pub before: Option<String>,
    #[serde(default)]
    pub at: Option<String>,
    /// A Timings expression the event must come within.
    #[serde(default)]
    pub within: Option<String>,
    /// Body fields, dotted. `"args": "this_call"` asks for the step's own call's digest.
    #[serde(flatten)]
    pub fields: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindowQuery {
    pub query: String,
    #[serde(default)]
    pub forbid: bool,
    #[serde(default)]
    pub at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sentence {
    pub says: String,
    #[serde(default)]
    pub forbid: bool,
}

/// **An `expected_fail` mark**: a step's assertion (`require[0]`, `forbid[1]`, `window[0]`,
/// `transcript[0]`, `outcome`, `calls`, `repeat_of`) and what was observed, or a global check
/// and the refusal it is about; each scoped, where given, to a leg, timings, dialect and OS.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mark {
    pub issue: u64,
    #[serde(default)]
    pub step: Option<u32>,
    #[serde(default)]
    pub assertion: Option<String>,
    #[serde(default)]
    pub check: Option<String>,
    #[serde(default)]
    pub refusal: Option<String>,
    #[serde(default)]
    pub observed: Option<Value>,
    /// The observation pinned, cited from its run record.
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub leg: Option<Leg>,
    #[serde(default)]
    pub timings: Option<TimingsMode>,
    #[serde(default)]
    pub dialect: Option<Dialect>,
    #[serde(default)]
    pub os: Option<Os>,
}

/// **One record of a leg's report** (`schema/leg-report.schema.json`): one per entry run.
/// Ticket 6 writes them, ticket 8 judges them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LegRecord {
    pub kind: EntryKind,
    pub row: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dialect: Option<Dialect>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protection: Option<Protection>,
    pub os: Os,
    pub leg: ReportLeg,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timings: Option<TimingsMode>,
    pub verdict: ReportVerdict,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    Cell,
    Escape,
    Fixture,
    E2e,
}

/// The legs a report can come from: the cells' legs, and the runs of fixture and e2e covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportLeg {
    Fake,
    ClaudeSandbox,
    RealWrap,
    Live,
    Fixture,
    E2e,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportVerdict {
    Pass,
    Fail,
    ExpectedFail,
    NotRun,
}
