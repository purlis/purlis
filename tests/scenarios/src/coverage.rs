// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The matrix coverage gate, with the expected-fail ratchet** (T1.1-08, #6855).
//!
//! `coverage.json` names a cover for every row × dialect × OS of the behaviour matrix (its row
//! table is `rows.json`), every row the matrix leaves to fixtures (§3.20), each dialect's
//! real-binary network and file probes, and every escape × protection of `escapes.json`. It was
//! generated from the matrix by `tools/scenario-coverage.mjs` and is owned here since.
//!
//! - [`check`]: the list itself. Every entry has a cover that exists, of the kind the matrix
//!   carries unless a ruling moved it, with a reason from the closed list for its kind, under
//!   its reason's cap; and the list has exactly the matrix's rows, dialects and OSes, every row
//!   the matrix leaves to fixtures ([`FIXTURE_ROWS`]) and every real-binary probe.
//! - [`against_base`]: what the list may not change by itself against its base: a cap above the
//!   base's seed, a row's carried kind, kind or binaries, a real cover back to pending, a fixture
//!   row's OS, and an approved mark, each only with a ruling the base's `rulings.json` names.
//! - [`expectations`]: every cell a scenario cover names states what its row states.
//! - [`ratchet`]: an `expected_fail` mark is added on an existing cell only with a new issue.
//! - [`issues`]: no mark names a closed issue, or one the change closes.
//! - [`at_gate`]: at M74.1's gate nothing is pending and every mark is one the person approved.
//! - [`judge_runs`]: every covered entry passed, or failed as a mark pins, in the leg its cover
//!   requires.
//! - [`report`]: counts per cover kind, and the marks per issue.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::model::{
    Batch, Cell, Dialect, EntryKind, Leg, LegRecord, Os, Protection, ReportLeg, ReportVerdict,
    TimingsMode,
};
use crate::plan::{Scheduled, Trigger, schedule};
use crate::timings::TimingsCatalogue;
use crate::validate::{RowEntry, RowTable, Validator};

/// **The closed list of reasons a row may be left to the live pass** (ticket 37): model
/// behaviour; a person's look at wording or timing; an OS privacy prompt (TCC); a third-party
/// daemon (docker); a prompt answered in the harness's own terminal; a production-OS check on
/// the person's machine; a harness that can't run against a stand-in (ticket 3's fallback),
/// which covers that dialect's fidelity row.
pub const LIVE_REASONS: [&str; 7] = [
    "model_behaviour",
    "human_look",
    "tcc_prompt",
    "third_party_daemon",
    "harness_terminal_prompt",
    "production_os_check",
    "no_stand_in",
];

/// **The closed list of reasons a row is a fixture's**: a fault, a race, time, a third-party
/// service the matrix names in §3.20, machine-wide state (the policy file), an OS account or
/// privilege (a non-admin user), an internal state or a unit-only R-row, or no inert probe.
pub const FIXTURE_REASONS: [&str; 8] = [
    "fault",
    "race",
    "time",
    "third_party_service",
    "machine_wide_state",
    "os_account",
    "internal_state",
    "no_inert_probe",
];

/// **The closed list of reasons a row is an e2e spec's**: it is about the window's drawing,
/// focus or grouping.
pub const E2E_REASONS: [&str; 1] = ["window"];

/// **Every row the matrix leaves to fixtures** (test-matrix-phase-1.md §3.20, and §3.19's
/// M-POL-01…03, whose Need is `F: the policy file is machine-wide`): the list names a cover for
/// each, so deleting one from `coverage.json` alone is refused. A re-carry that changes §3.20
/// changes this list, and `node tools/scenario-coverage.mjs diff` says so.
pub const FIXTURE_ROWS: [&str; 37] = [
    "M-WAY-04",
    "M-LOOP-11",
    "M-HC-02",
    "M-HC-07",
    "M-ASK-03",
    "M-ASK-12",
    "M-GRD-08",
    "M-GRD-10",
    "M-ATT-01",
    "M-ATT-02",
    "M-ATT-03",
    "M-ATT-04",
    "M-ATT-07",
    "M-ATT-08",
    "M-RST-05",
    "M-HOLD-11",
    "M-HOLD-14",
    "M-BH-02",
    "M-BH-07",
    "M-BH-08b",
    "M-OBS-07",
    "M-OBS-11",
    "M-SET-05",
    "M-POL-01",
    "M-POL-02",
    "M-POL-03",
    "R-006",
    "R-008",
    "R-012",
    "R-032",
    "R-169",
    "R-281",
    "R-284",
    "R-289",
    "R-315",
    "R-327",
    "R-355",
];

/// The tickets of M74.1 (T1.1-01 to T1.1-57): a `pending` cover names one of them.
const TICKETS: std::ops::RangeInclusive<u32> = 1..=57;

const OSES: [Os; 2] = [Os::Linux, Os::Macos];

// ---- the list ----------------------------------------------------------------------------------

/// **`coverage.json`**.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Coverage {
    #[serde(rename = "$comment", default)]
    pub comment: Option<String>,
    /// The list's format. Nothing the gate requires depends on it: the probes and fixture rows
    /// every list owes are in this file's code ([`Repo::required`]).
    #[serde(default)]
    pub version: u32,
    /// Per cover kind (`live`, `fixture`, `e2e`), per reason: how many rows may use it.
    #[serde(default)]
    pub caps: BTreeMap<Kind, BTreeMap<String, Cap>>,
    /// The marks the person approved by name, for M74.1's gate (recorded in decisions.md).
    #[serde(default)]
    pub approved_marks: Vec<ApprovedMark>,
    /// Known gaps, each with its reason.
    #[serde(default)]
    pub gaps: Vec<Gap>,
    #[serde(default)]
    pub rows: Vec<RowCover>,
    #[serde(default)]
    pub fixtures: Vec<FixtureCover>,
    #[serde(default)]
    pub real_binary: Vec<RealCover>,
    #[serde(default)]
    pub escapes: Vec<EscapeCover>,
}

impl Coverage {
    pub fn from_value(value: serde_json::Value) -> Result<Self, serde_json::Error> {
        serde_json::from_value(value)
    }

    /// The repo's `coverage.json`.
    pub fn read() -> Result<Self, String> {
        read_json(&crate::root().join("coverage.json"))
    }
}

/// A cover's kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Scenario,
    Fixture,
    E2e,
    Live,
}

impl Kind {
    pub fn word(self) -> &'static str {
        match self {
            Kind::Scenario => "scenario",
            Kind::Fixture => "fixture",
            Kind::E2e => "e2e",
            Kind::Live => "live",
        }
    }

    /// The closed list of reasons for this kind; a scenario needs none.
    pub fn reasons(self) -> &'static [&'static str] {
        match self {
            Kind::Scenario => &[],
            Kind::Fixture => &FIXTURE_REASONS,
            Kind::E2e => &E2E_REASONS,
            Kind::Live => &LIVE_REASONS,
        }
    }
}

/// **A reason's cap**: `seed` from ticket 43's approved cover-kind column, and `cap` above it
/// only with the person's approving Q-id.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cap {
    pub seed: usize,
    pub cap: usize,
    #[serde(default)]
    pub approved: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovedMark {
    pub issue: u64,
    /// `<cells file>#<cell id>`, as a scenario cover names it.
    pub cell: String,
    pub approved: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Gap {
    pub rows: Vec<String>,
    pub os: Os,
    pub reason: String,
}

/// Which binaries a scenario row runs under, beside the rule that Claude Code on macOS runs only
/// in ticket 10's leg: the fake harness, the real one (ticket 53's leg), or both.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Binary {
    #[default]
    Fake,
    Real,
    Both,
}

/// **One row × dialect × OS** of the matrix (`rows.json`), with its Log/Window/Chat flags.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RowCover {
    pub row: String,
    pub dialect: Dialect,
    pub os: Os,
    pub log: bool,
    pub window: bool,
    pub chat: bool,
    /// The cover kind ticket 43's approved column gives the row.
    pub carried: Kind,
    pub kind: Kind,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub binary: Binary,
    /// The Q-id of the ruling that moved this row off `scenario`.
    #[serde(default)]
    pub moved: Option<String>,
    pub cover: String,
}

/// **A row the matrix leaves to fixtures** (§3.20), on the OS its test runs on.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixtureCover {
    pub row: String,
    pub os: Os,
    pub reason: String,
    /// The ruling that moved this row's test to another OS, or its cover back to pending.
    #[serde(default)]
    pub moved: Option<String>,
    pub cover: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Probe {
    Network,
    File,
}

/// **A dialect's real-binary probe** on one OS at one protection: ticket 10's leg for Claude
/// Code on macOS, ticket 53's everywhere else.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RealCover {
    pub probe: Probe,
    pub dialect: Dialect,
    pub os: Os,
    pub protection: Protection,
    /// The ruling that put this probe's cover back to pending (`network-probe`, `file-probe`).
    #[serde(default)]
    pub moved: Option<String>,
    pub cover: String,
}

/// **An escape's covers**, one per protection its row expects anything at, and its positive
/// control.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EscapeCover {
    pub id: String,
    pub covers: BTreeMap<Protection, String>,
    pub control: String,
    /// The ruling that put one of its covers or its control back to pending.
    #[serde(default)]
    pub moved: Option<String>,
}

// ---- a cover -----------------------------------------------------------------------------------

/// **A cover**, as the list writes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cover {
    /// `scenario: <cells file>#<cell id>`, the file relative to `tests/scenarios`.
    Scenario { file: String, cell: String },
    /// `fixture: <test file>::<test fn>`, the file relative to the repo.
    Fixture { file: String, test: String },
    /// `e2e: <spec>#<test title>`, the spec relative to the repo.
    E2e { spec: String, test: String },
    /// `live: <live-pass cell id>` (ticket 37).
    Live { cell: String },
    /// `pending: <ticket>`, which reaches zero by the gate.
    Pending { ticket: u32 },
}

impl Cover {
    pub fn parse(text: &str) -> Result<Cover, String> {
        let (kind, rest) = text
            .split_once(": ")
            .ok_or_else(|| format!("`{text}` is not `<kind>: <cover>`"))?;
        let two = |sep: &str| {
            rest.split_once(sep)
                .filter(|(a, b)| !a.is_empty() && !b.is_empty())
                .map(|(a, b)| (a.to_string(), b.to_string()))
                .ok_or_else(|| format!("`{text}` names no `<file>{sep}<name>`"))
        };
        match kind {
            "scenario" => two("#").map(|(file, cell)| Cover::Scenario { file, cell }),
            "fixture" => two("::").map(|(file, test)| Cover::Fixture { file, test }),
            "e2e" => two("#").map(|(spec, test)| Cover::E2e { spec, test }),
            "live" if !rest.is_empty() => Ok(Cover::Live {
                cell: rest.to_string(),
            }),
            "pending" => rest
                .parse::<u32>()
                .ok()
                .filter(|t| TICKETS.contains(t))
                .map(|ticket| Cover::Pending { ticket })
                .ok_or_else(|| format!("`{text}` names no M74.1 ticket")),
            _ => Err(format!("`{text}` is no cover kind")),
        }
    }

    pub fn kind(&self) -> Option<Kind> {
        match self {
            Cover::Scenario { .. } => Some(Kind::Scenario),
            Cover::Fixture { .. } => Some(Kind::Fixture),
            Cover::E2e { .. } => Some(Kind::E2e),
            Cover::Live { .. } => Some(Kind::Live),
            Cover::Pending { .. } => None,
        }
    }
}

// ---- what the list is checked against ------------------------------------------------------------

/// **One row of the escape table** (`escapes.json`, ticket 43), the columns the gate reads.
#[derive(Debug, Clone, Deserialize)]
pub struct EscapeRow {
    pub id: String,
    pub ticket: String,
    #[serde(rename = "Full")]
    pub full: String,
    #[serde(rename = "Basic")]
    pub basic: String,
    pub off: String,
    pub dialects: String,
    pub os: String,
    pub real_binary: String,
}

#[derive(Debug, Clone, Deserialize)]
struct EscapeTable {
    rows: Vec<EscapeRow>,
}

impl EscapeRow {
    /// The protections this escape expects anything at: those not `n/a`.
    pub fn protections(&self) -> BTreeSet<Protection> {
        [
            (Protection::Full, &self.full),
            (Protection::Basic, &self.basic),
            (Protection::None, &self.off),
        ]
        .into_iter()
        .filter(|(_, text)| !text.starts_with("n/a"))
        .map(|(p, _)| p)
        .collect()
    }

    pub fn real_binary(&self) -> bool {
        self.real_binary == "yes"
    }

    /// **Each dialect and OS the table applies this escape to**, read from its `dialects` and
    /// `os` columns: `claude, codex, opencode` and `both`, with a note in brackets; `opencode;
    /// claude on Linux`; `both for OpenCode; linux for Claude Code`; `macos for the seal; linux
    /// for the refusal` (both); and `macos (ticket 10's leg); Linux in ticket 53's leg` (Linux
    /// too) or `…; Linux is E-FILE-31` and `…; on Linux the whole process is wrapped` (notes).
    pub fn applies(&self) -> Result<Vec<(Dialect, Os)>, String> {
        let bad = |what: &str| format!("{}: its {what} column can't be read", self.id);
        let dialect = |word: &str| match word.trim().to_lowercase().as_str() {
            "claude" | "claude code" => Some(Dialect::Claude),
            "codex" => Some(Dialect::Codex),
            "opencode" => Some(Dialect::Opencode),
            _ => None,
        };
        let os_word = |word: &str| match word.trim().to_lowercase().as_str() {
            "both" => Some(vec![Os::Linux, Os::Macos]),
            "linux" => Some(vec![Os::Linux]),
            "macos" => Some(vec![Os::Macos]),
            _ => None,
        };
        let mut dialects = Vec::new();
        let mut own_os: BTreeMap<Dialect, Vec<Os>> = BTreeMap::new();
        for part in unbracketed(&self.dialects).split(';') {
            let (names, on) = match part.split_once(" on ") {
                Some((names, on)) => (names, Some(on)),
                None => (part, None),
            };
            for name in names.split(',') {
                let d = dialect(name).ok_or_else(|| bad("dialects"))?;
                dialects.push(d);
                if let Some(on) = on {
                    own_os.insert(d, os_word(on).ok_or_else(|| bad("dialects"))?);
                }
            }
        }
        let mut general: Vec<Os> = Vec::new();
        for part in unbracketed(&self.os).split(';') {
            let part = part.trim();
            // A note on another row's or leg's coverage, not an OS this row runs on.
            if part.starts_with("Linux is ") || part.starts_with("on Linux") {
                continue;
            }
            if part == "Linux in ticket 53's leg" {
                general.push(Os::Linux);
                continue;
            }
            // `both for OpenCode` scopes a dialect; `macos for the seal` names what is checked
            // there, and so is the OS for every dialect.
            match part.split_once(" for ") {
                Some((oses, who)) if dialect(who).is_some() => {
                    let d = dialect(who).ok_or_else(|| bad("os"))?;
                    own_os.insert(d, os_word(oses).ok_or_else(|| bad("os"))?);
                }
                Some((oses, _)) => general.extend(os_word(oses).ok_or_else(|| bad("os"))?),
                None => general.extend(os_word(part).ok_or_else(|| bad("os"))?),
            }
        }
        let mut pairs = Vec::new();
        for d in dialects {
            let oses = own_os.get(&d).unwrap_or(&general);
            if oses.is_empty() {
                return Err(bad("os"));
            }
            for &os in OSES.iter().filter(|o| oses.contains(o)) {
                pairs.push((d, os));
            }
        }
        Ok(pairs)
    }
}

/// `text` with every `(…)` taken out.
fn unbracketed(text: &str) -> String {
    let mut out = String::new();
    let mut depth = 0usize;
    for c in text.chars() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out
}

/// **The issues the marks name, and whether each is open** (`issues.json`): a cache read on a
/// pull request, refreshed from GitHub nightly (`node tools/scenario-coverage.mjs issues`).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Issues {
    #[serde(rename = "$comment", default)]
    pub comment: Option<String>,
    #[serde(default)]
    pub fetched: Option<String>,
    pub issues: BTreeMap<u64, Issue>,
}

/// **One issue in the cache**: whether it is open, and when it was opened (GitHub's
/// `createdAt`, `2026-11-15T05:08:34Z`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Issue {
    pub state: IssueState,
    pub opened: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueState {
    Open,
    Closed,
}

/// **Everything the list is checked against**: the repo's root, the cells files (by their path
/// in `tests/scenarios`, `cells/B0.json`), the row table, the escape table, the issue cache, the
/// person's rulings (`rulings.json`), and what every list owes beside the row table.
pub struct Repo {
    pub root: PathBuf,
    pub batches: Vec<(String, Batch)>,
    pub rows: RowTable,
    pub escapes: Vec<EscapeRow>,
    pub issues: Issues,
    pub rulings: Rulings,
    pub required: Required,
    pub catalogue: Box<dyn TimingsCatalogue>,
}

/// **The person's rulings of M74.1** (`rulings.json`): the Q-ids that may move a row, raise a
/// cap or approve a mark, each naming exactly what it approves. A change that cites a ruling is
/// judged by its base's `rulings.json` ([`against_base`]), so a ruling is added in a pull request
/// of its own, before the one that cites it.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rulings {
    #[serde(rename = "$comment", default)]
    pub comment: Option<String>,
    /// Each ruling by its exact id: `Q321`, or a point of one listed as its own, `Q321.8`.
    pub rulings: BTreeMap<String, Ruling>,
}

/// **One ruling**: the day it was made, and what it approves. An id names a row of the matrix
/// (`M-NET-01`), an escape (`E-NET-01`) or a real-binary probe (`network-probe`, `file-probe`);
/// a cap is `<kind>.<reason>` (`fixture.race`); a mark is its issue's number.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ruling {
    pub date: String,
    #[serde(default)]
    pub rows: Vec<String>,
    #[serde(default)]
    pub caps: Vec<String>,
    #[serde(default)]
    pub marks: Vec<u64>,
}

/// **What a ruling is cited for**.
#[derive(Debug, Clone, Copy)]
pub enum Target<'a> {
    /// A row, escape or probe moved, by its id.
    Row(&'a str),
    /// A cap raised.
    Cap(Kind, &'a str),
    /// An `expected_fail` mark approved, by its issue.
    Mark(u64),
}

impl std::fmt::Display for Target<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Target::Row(id) => write!(f, "`{id}` in its `rows`"),
            Target::Cap(kind, reason) => write!(f, "`{}.{reason}` in its `caps`", kind.word()),
            Target::Mark(issue) => write!(f, "{issue} in its `marks`"),
        }
    }
}

/// Said with every refusal of a ruling against the base.
pub const RULINGS_ALONE: &str = "a ruling is added to rulings.json, naming what it approves, in a pull request of its own, merged before the one that cites it";

impl Rulings {
    /// Why `q` does not approve `target`, or `None` when it does: `q` must be one of these
    /// rulings by its exact id, and name `target`.
    pub fn refuses(&self, q: &str, target: Target<'_>) -> Option<String> {
        if !is_q_id(q) {
            return Some(format!("`{q}`, which is no Q-id"));
        }
        let Some(ruling) = self.rulings.get(q) else {
            return Some(format!(
                "`{q}` is no ruling in rulings.json (a point of one is listed as its own id)"
            ));
        };
        let named = match target {
            Target::Row(id) => ruling.rows.iter().any(|r| r == id),
            Target::Cap(kind, reason) => ruling
                .caps
                .iter()
                .any(|c| c.split_once('.') == Some((kind.word(), reason))),
            Target::Mark(issue) => ruling.marks.contains(&issue),
        };
        (!named).then(|| format!("`{q}` does not name {target} in rulings.json"))
    }

    /// Every ruling whose id is not a Q-id (`Q300`, `Q308a`, `Q321.8`) or whose date is missing.
    pub fn problems(&self) -> Vec<String> {
        self.rulings
            .iter()
            .filter(|(id, r)| !is_q_id(id) || r.date.is_empty())
            .map(|(id, _)| format!("rulings.json: `{id}` is not a Q-id with its date"))
            .collect()
    }
}

/// **What every list owes beside the row table and the escape table**, from this file's code,
/// never from the list: the rows the matrix leaves to fixtures, and whether each dialect's
/// real-binary probes are owed. A test's few lines owe neither ([`Required::default`]).
#[derive(Debug, Clone, Default)]
pub struct Required {
    pub fixtures: Vec<String>,
    pub probes: bool,
}

impl Required {
    /// The repo's: [`FIXTURE_ROWS`], and every probe.
    pub fn repo() -> Self {
        Required {
            fixtures: FIXTURE_ROWS.iter().map(|r| r.to_string()).collect(),
            probes: true,
        }
    }
}

impl Repo {
    /// This checkout's.
    pub fn read() -> Result<Self, String> {
        let here = crate::root();
        let dir = here.join("cells");
        let mut names: Vec<String> = fs::read_dir(&dir)
            .map_err(|e| format!("{}: {e}", dir.display()))?
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".json"))
            .collect();
        names.sort();
        let mut batches = Vec::new();
        for name in names {
            batches.push((format!("cells/{name}"), read_json(&dir.join(&name))?));
        }
        let escapes: EscapeTable = read_json(&here.join("escapes.json"))?;
        Ok(Repo {
            root: here.join("../.."),
            batches,
            rows: read_json(&here.join("rows.json"))?,
            escapes: escapes.rows,
            issues: read_json(&here.join("issues.json"))?,
            rulings: read_json(&here.join("rulings.json"))?,
            required: Required::repo(),
            catalogue: crate::timings::catalogue(),
        })
    }

    fn cell(&self, file: &str, id: &str) -> Option<(&Batch, &Cell)> {
        let (_, batch) = self.batches.iter().find(|(f, _)| f == file)?;
        batch.cells.iter().find(|c| c.id == id).map(|c| (batch, c))
    }
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// **The row table the list carries**, for ticket 5's validator over the real cells.
pub fn row_table(list: &Coverage) -> RowTable {
    RowTable {
        entries: list
            .rows
            .iter()
            .map(|r| RowEntry {
                row: r.row.clone(),
                dialect: r.dialect,
                os: r.os,
                log: r.log,
                window: r.window,
                chat: r.chat,
            })
            .collect(),
    }
}

// ---- the gate ----------------------------------------------------------------------------------

fn os_word(os: Os) -> &'static str {
    match os {
        Os::Linux => "linux",
        Os::Macos => "macos",
    }
}

fn protection_word(p: Protection) -> &'static str {
    match p {
        Protection::None => "none",
        Protection::Basic => "basic",
        Protection::Full => "full",
    }
}

fn leg_word(leg: ReportLeg) -> &'static str {
    match leg {
        ReportLeg::Fake => "fake",
        ReportLeg::ClaudeSandbox => "claude_sandbox",
        ReportLeg::RealWrap => "real_wrap",
        ReportLeg::Live => "live",
        ReportLeg::Fixture => "fixture",
        ReportLeg::E2e => "e2e",
    }
}

fn model_leg(leg: Leg) -> ReportLeg {
    match leg {
        Leg::Fake => ReportLeg::Fake,
        Leg::ClaudeSandbox => ReportLeg::ClaudeSandbox,
        Leg::RealWrap => ReportLeg::RealWrap,
        Leg::Live => ReportLeg::Live,
    }
}

fn timings_word(t: TimingsMode) -> &'static str {
    match t {
        TimingsMode::Scaled => "scaled",
        TimingsMode::Production => "production",
    }
}

/// `Q300`, `Q321a`, `Q321.9`: a ruling's id in decisions.md.
fn is_q_id(text: &str) -> bool {
    let Some(rest) = text.strip_prefix('Q') else {
        return false;
    };
    let (number, point) = match rest.split_once('.') {
        Some((n, p)) => (n, Some(p)),
        None => (rest, None),
    };
    let number = number
        .strip_suffix(|c: char| c.is_ascii_lowercase())
        .unwrap_or(number);
    !number.is_empty()
        && number.chars().all(|c| c.is_ascii_digit())
        && point.is_none_or(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

/// The legs a scenario cover runs in: Claude Code on macOS only in ticket 10's, under its own
/// sandbox; elsewhere the fake harness's, the real binary's under purlis's wrap (ticket 53), or
/// both.
fn scenario_legs(dialect: Dialect, os: Os, binary: Binary) -> Vec<ReportLeg> {
    if dialect == Dialect::Claude && os == Os::Macos {
        return vec![ReportLeg::ClaudeSandbox];
    }
    match binary {
        Binary::Fake => vec![ReportLeg::Fake],
        Binary::Real => vec![ReportLeg::RealWrap],
        Binary::Both => vec![ReportLeg::Fake, ReportLeg::RealWrap],
    }
}

/// What `text` must name and be, for an entry `at`.
struct Want<'a> {
    at: &'a str,
    kind: Option<Kind>,
    dialects: &'a [Dialect],
    escape: Option<bool>,
}

fn cover_problems(text: &str, want: &Want<'_>, repo: &Repo, problems: &mut Vec<String>) {
    let at = want.at;
    let cover = match Cover::parse(text) {
        Ok(cover) => cover,
        Err(why) => {
            problems.push(format!("{at}: {why}"));
            return;
        }
    };
    if let (Some(is), Some(should)) = (cover.kind(), want.kind)
        && is != should
    {
        problems.push(format!(
            "{at}: `{text}` is a {} cover, and the entry's kind is {}",
            is.word(),
            should.word()
        ));
    }
    let holds = |file: &str, needle: &str| {
        fs::read_to_string(repo.root.join(file)).is_ok_and(|text| text.contains(needle))
    };
    match &cover {
        Cover::Scenario { file, cell } => match repo.cell(file, cell) {
            None => problems.push(format!("{at}: `{text}`: {file} has no cell {cell}")),
            Some((_, c)) => {
                for d in want.dialects.iter().filter(|d| !c.dialects.contains(d)) {
                    problems.push(format!("{at}: {file}#{cell} does not run in {}", d.word()));
                }
                match want.escape {
                    Some(true) if !c.escape => {
                        problems.push(format!("{at}: {file}#{cell} is not an escape cell"))
                    }
                    Some(false) if c.escape => problems.push(format!(
                        "{at}: {file}#{cell} is an escape cell, not a control"
                    )),
                    _ => {}
                }
            }
        },
        Cover::Fixture { file, test } => {
            if !holds(file, &format!("fn {test}(")) {
                problems.push(format!("{at}: `{text}`: {file} has no test {test}"));
            }
        }
        Cover::E2e { spec, test } => {
            let titled =
                fs::read_to_string(repo.root.join(spec)).is_ok_and(|text| names_test(&text, test));
            if !titled {
                problems.push(format!("{at}: `{text}`: {spec} has no test {test}"));
            }
        }
        Cover::Live { cell } => {
            let found = repo
                .batches
                .iter()
                .flat_map(|(_, b)| &b.cells)
                .any(|c| &c.id == cell && c.live_only.is_some());
            if !found {
                problems.push(format!("{at}: `{text}`: no live-pass cell {cell}"));
            }
        }
        Cover::Pending { .. } => {}
    }
}

/// Whether a spec's `text` has a `test(` whose title is exactly `title`, in any quotes, across
/// whatever line breaks a formatter puts after `test(`.
fn names_test(text: &str, title: &str) -> bool {
    ['"', '\'', '`'].iter().any(|q| {
        let quoted = format!("{q}{title}{q}");
        text.match_indices(&quoted).any(|(at, _)| {
            text[..at].trim_end().ends_with("test(")
                && text[at + quoted.len()..].trim_start().starts_with(',')
        })
    })
}

fn reason_problems(at: &str, kind: Kind, reason: Option<&str>, problems: &mut Vec<String>) {
    if kind == Kind::Scenario {
        return;
    }
    match reason {
        None => problems.push(format!(
            "{at}: a {} cover needs a reason from the closed list",
            kind.word()
        )),
        Some(reason) if !kind.reasons().contains(&reason) => problems.push(format!(
            "{at}: the reason `{reason}` is not on the list for a {} cover",
            kind.word()
        )),
        Some(_) => {}
    }
}

/// **Every problem with the list**, one line each; none when the gate holds.
pub fn check(list: &Coverage, repo: &Repo) -> Vec<String> {
    let mut problems = Vec::new();

    // The rows: exactly the matrix's row table, with its flags.
    let mut listed: BTreeMap<(&str, Dialect, Os), &RowCover> = BTreeMap::new();
    for r in &list.rows {
        if listed
            .insert((r.row.as_str(), r.dialect, r.os), r)
            .is_some()
        {
            problems.push(format!(
                "{} {} {}: listed twice",
                r.row,
                r.dialect.word(),
                os_word(r.os)
            ));
        }
    }
    let mut table = BTreeSet::new();
    for e in &repo.rows.entries {
        table.insert((e.row.as_str(), e.dialect, e.os));
        let at = format!("{} {} {}", e.row, e.dialect.word(), os_word(e.os));
        match listed.get(&(e.row.as_str(), e.dialect, e.os)) {
            None => problems.push(format!(
                "{at}: no cover: the matrix's row table has it and the list does not"
            )),
            Some(r) if (r.log, r.window, r.chat) != (e.log, e.window, e.chat) => problems.push(
                format!("{at}: the list's Log/Window/Chat flags are not the matrix's"),
            ),
            Some(_) => {}
        }
    }
    for r in &list.rows {
        let at = format!("{} {} {}", r.row, r.dialect.word(), os_word(r.os));
        if !table.contains(&(r.row.as_str(), r.dialect, r.os)) {
            problems.push(format!("{at}: not in the matrix's row table"));
        }
        if r.kind != r.carried {
            let off = format!("moved off {} to {}", r.carried.word(), r.kind.word());
            match &r.moved {
                None => problems.push(format!("{at}: {off} without a Q-id")),
                Some(q) => {
                    if let Some(why) = repo.rulings.refuses(q, Target::Row(&r.row)) {
                        problems.push(format!("{at}: {off} by {why}"));
                    }
                }
            }
        }
        reason_problems(&at, r.kind, r.reason.as_deref(), &mut problems);
        let want = Want {
            at: &at,
            kind: Some(r.kind),
            dialects: &[r.dialect],
            escape: Some(false),
        };
        cover_problems(&r.cover, &want, repo, &mut problems);
    }

    // The rows left to fixtures: every one the matrix leaves there, and no other.
    let fixture_rows: BTreeSet<&str> = list.fixtures.iter().map(|f| f.row.as_str()).collect();
    for row in &repo.required.fixtures {
        if !fixture_rows.contains(row.as_str()) {
            problems.push(format!(
                "{row}: no cover: the matrix leaves it to fixtures and the list does not name it"
            ));
        }
    }
    let mut fixture_seen = BTreeSet::new();
    for f in &list.fixtures {
        if !fixture_seen.insert((f.row.as_str(), f.os)) {
            problems.push(format!("{} {}: listed twice", f.row, os_word(f.os)));
        }
        if !repo.required.fixtures.is_empty() && !repo.required.fixtures.contains(&f.row) {
            problems.push(format!(
                "{} {}: not a row the matrix leaves to fixtures",
                f.row,
                os_word(f.os)
            ));
        }
    }
    for f in &list.fixtures {
        let at = format!("{} {}", f.row, os_word(f.os));
        reason_problems(&at, Kind::Fixture, Some(&f.reason), &mut problems);
        let want = Want {
            at: &at,
            kind: Some(Kind::Fixture),
            dialects: &[],
            escape: None,
        };
        cover_problems(&f.cover, &want, repo, &mut problems);
    }

    // Each dialect's real-binary network and file probes, on each OS, at Full and Basic.
    let mut probes = BTreeSet::new();
    for p in &list.real_binary {
        let at = format!(
            "the {} probe, {} {} at {}",
            match p.probe {
                Probe::Network => "network",
                Probe::File => "file",
            },
            p.dialect.word(),
            os_word(p.os),
            protection_word(p.protection)
        );
        if !probes.insert((p.probe, p.dialect, p.os, p.protection)) {
            problems.push(format!("{at}: listed twice"));
        }
        let want = Want {
            at: &at,
            kind: Some(Kind::Scenario),
            dialects: &[p.dialect],
            escape: Some(false),
        };
        cover_problems(&p.cover, &want, repo, &mut problems);
    }
    // The repo's list names every probe, whatever its `version` says.
    if repo.required.probes {
        for probe in [Probe::Network, Probe::File] {
            for d in Dialect::ALL {
                for os in OSES {
                    for protection in [Protection::Full, Protection::Basic] {
                        if !probes.contains(&(probe, d, os, protection)) {
                            problems.push(format!(
                                "the {} probe, {} {} at {}: no cover",
                                match probe {
                                    Probe::Network => "network",
                                    Probe::File => "file",
                                },
                                d.word(),
                                os_word(os),
                                protection_word(protection)
                            ));
                        }
                    }
                }
            }
        }
    }

    // The escapes: exactly the escape table's, a cover for each protection it expects anything
    // at, and a positive control.
    let table: BTreeMap<&str, &EscapeRow> =
        repo.escapes.iter().map(|e| (e.id.as_str(), e)).collect();
    let covered: BTreeSet<&str> = list.escapes.iter().map(|e| e.id.as_str()).collect();
    for id in table.keys().filter(|id| !covered.contains(*id)) {
        problems.push(format!(
            "{id}: no cover: the escape table has it and the list does not"
        ));
    }
    for e in &list.escapes {
        let Some(row) = table.get(e.id.as_str()) else {
            problems.push(format!("{}: not in the escape table", e.id));
            continue;
        };
        let dialects: Vec<Dialect> = match row.applies() {
            Ok(pairs) => {
                let mut ds: Vec<Dialect> = pairs.into_iter().map(|(d, _)| d).collect();
                ds.dedup();
                ds
            }
            Err(why) => {
                problems.push(why);
                Vec::new()
            }
        };
        let expects = row.protections();
        for p in &expects {
            if !e.covers.contains_key(p) {
                problems.push(format!("{} at {}: no cover", e.id, protection_word(*p)));
            }
        }
        for (p, text) in &e.covers {
            let at = format!("{} at {}", e.id, protection_word(*p));
            if !expects.contains(p) {
                problems.push(format!("{at}: the escape table expects nothing there"));
            }
            let want = Want {
                at: &at,
                kind: Some(Kind::Scenario),
                dialects: &dialects,
                escape: Some(true),
            };
            cover_problems(text, &want, repo, &mut problems);
        }
        let at = format!("{}'s positive control", e.id);
        let want = Want {
            at: &at,
            kind: Some(Kind::Scenario),
            dialects: &dialects,
            escape: Some(false),
        };
        cover_problems(&e.control, &want, repo, &mut problems);
    }

    // Each reason under its cap, and a cap above its seed only with the person's Q-id.
    let mut used: BTreeMap<(Kind, &str), BTreeSet<&str>> = BTreeMap::new();
    for r in list.rows.iter().filter(|r| r.kind != Kind::Scenario) {
        if let Some(reason) = &r.reason {
            used.entry((r.kind, reason)).or_default().insert(&r.row);
        }
    }
    for f in &list.fixtures {
        used.entry((Kind::Fixture, &f.reason))
            .or_default()
            .insert(&f.row);
    }
    for ((kind, reason), rows) in &used {
        match list.caps.get(kind).and_then(|caps| caps.get(*reason)) {
            None => problems.push(format!(
                "{} reason `{reason}`: {} rows use it, and the list sets it no cap",
                kind.word(),
                rows.len()
            )),
            Some(cap) if rows.len() > cap.cap => problems.push(format!(
                "{} reason `{reason}`: {} rows use it, over its cap of {}",
                kind.word(),
                rows.len(),
                cap.cap
            )),
            Some(_) => {}
        }
    }
    for (kind, caps) in &list.caps {
        for (reason, cap) in caps {
            let at = format!("the {} cap for `{reason}`", kind.word());
            if !kind.reasons().contains(&reason.as_str()) {
                problems.push(format!(
                    "{at}: the reason `{reason}` is not on the list for a {} cover",
                    kind.word()
                ));
            }
            match &cap.approved {
                None if cap.cap > cap.seed => problems.push(format!(
                    "{at}: raised from its seed {} to {} with no approving Q-id",
                    cap.seed, cap.cap
                )),
                None => {}
                Some(q) => {
                    if let Some(why) = repo.rulings.refuses(q, Target::Cap(*kind, reason)) {
                        problems.push(format!(
                            "{at}: seed {} and cap {}, approved by {why}",
                            cap.seed, cap.cap
                        ));
                    }
                }
            }
        }
    }

    for g in &list.gaps {
        if g.reason.trim().is_empty() {
            problems.push(format!("the gap for {}: no reason", g.rows.join(", ")));
        }
    }
    problems
}

fn binary_word(b: Binary) -> &'static str {
    match b {
        Binary::Fake => "the fake binary",
        Binary::Real => "the real binary",
        Binary::Both => "both binaries",
    }
}

/// Whether going from `base`'s binaries to `head`'s drops a leg the row ran in.
fn drops_a_leg(base: Binary, head: Binary) -> bool {
    base != head && head != Binary::Both
}

fn pending(cover: &str) -> bool {
    matches!(Cover::parse(cover), Ok(Cover::Pending { .. }))
}

/// **What the list may not change against its base by itself** (`base` is the base's
/// `coverage.json`, `rulings` the base's `rulings.json`, never the change's own):
///
/// - a cap above the most the base allowed (its seed, or a cap a ruling raised it to);
/// - a row's `carried` kind, its `kind` moved off what it carries, or its binaries going from
///   `both` to one;
/// - a real cover (a row's, a fixture row's, an escape's or a probe's) going back to `pending`,
///   and a fixture row's test leaving an OS it ran on;
/// - an `approved_marks` entry the base did not have.
///
/// Each needs a ruling in the base's `rulings.json` that names it (`approved` for a cap, `moved`
/// for an entry, `approved` for a mark) and that the base's entry did not already cite. A row or
/// cap new in this change is held to [`check`] alone.
pub fn against_base(base: &Coverage, head: &Coverage, rulings: &Rulings) -> Vec<String> {
    let mut problems = Vec::new();
    // Why `head` does not carry a ruling of the base's that names `target` and that `was` did
    // not already cite; `None` when it does.
    let unruled = |head: Option<&String>, was: Option<&String>, target: Target<'_>| match head {
        None => Some("no Q-id".to_string()),
        Some(q) if Some(q) == was => Some(format!("`{q}`, which its base already cited")),
        Some(q) => rulings
            .refuses(q, target)
            .map(|why| format!("{why} of the base")),
    };
    let mut refuse = |at: String, what: String, why: String| {
        problems.push(format!("{at}: {what}, with {why}; {RULINGS_ALONE}"));
    };

    for (kind, caps) in &head.caps {
        for (reason, cap) in caps {
            let was = base.caps.get(kind).and_then(|c| c.get(reason));
            let allowed = was.map_or(0, |c| c.seed.max(c.cap));
            let top = cap.seed.max(cap.cap);
            if top <= allowed {
                continue;
            }
            let target = Target::Cap(*kind, reason);
            if let Some(why) = unruled(
                cap.approved.as_ref(),
                was.and_then(|c| c.approved.as_ref()),
                target,
            ) {
                refuse(
                    format!("the {} cap for `{reason}`", kind.word()),
                    format!("its seed and cap reach {top}, above the {allowed} its base allowed"),
                    why,
                );
            }
        }
    }

    let before: BTreeMap<(&str, Dialect, Os), &RowCover> = base
        .rows
        .iter()
        .map(|r| ((r.row.as_str(), r.dialect, r.os), r))
        .collect();
    for r in &head.rows {
        let Some(b) = before.get(&(r.row.as_str(), r.dialect, r.os)) else {
            continue;
        };
        let mut moves = Vec::new();
        if r.carried != b.carried {
            moves.push(format!(
                "carried moves from {} to {}; the matrix carries a row's kind, and only a ruling moves it",
                b.carried.word(),
                r.carried.word()
            ));
        }
        if r.kind != b.kind && r.kind != r.carried {
            moves.push(format!(
                "its kind moves from {} to {}, off the {} it carries",
                b.kind.word(),
                r.kind.word(),
                r.carried.word()
            ));
        }
        if drops_a_leg(b.binary, r.binary) {
            moves.push(format!(
                "runs under {}, and its base under {}: a leg is dropped",
                binary_word(r.binary),
                binary_word(b.binary)
            ));
        }
        if pending(&r.cover) && !pending(&b.cover) {
            moves.push(format!(
                "its cover `{}` goes back to `{}`",
                b.cover, r.cover
            ));
        }
        if moves.is_empty() {
            continue;
        }
        if let Some(why) = unruled(r.moved.as_ref(), b.moved.as_ref(), Target::Row(&r.row)) {
            let at = format!("{} {} {}", r.row, r.dialect.word(), os_word(r.os));
            for m in moves {
                refuse(at.clone(), m, format!("{why} in `moved`"));
            }
        }
    }

    // A fixture row: its test leaves no OS it ran on, and no cover of it goes back to pending.
    let fixture_rows: BTreeSet<&str> = base.fixtures.iter().map(|f| f.row.as_str()).collect();
    for row in fixture_rows {
        let was: Vec<&FixtureCover> = base.fixtures.iter().filter(|f| f.row == row).collect();
        let now: Vec<&FixtureCover> = head.fixtures.iter().filter(|f| f.row == row).collect();
        if now.is_empty() {
            continue; // `check` refuses a fixture row the list no longer names.
        }
        let mut moves = Vec::new();
        for b in &was {
            match now.iter().find(|f| f.os == b.os) {
                None => moves.push(format!(
                    "its test leaves {}, where its base ran it",
                    os_word(b.os)
                )),
                Some(f) if pending(&f.cover) && !pending(&b.cover) => moves.push(format!(
                    "its cover on {} `{}` goes back to `{}`",
                    os_word(b.os),
                    b.cover,
                    f.cover
                )),
                Some(_) => {}
            }
        }
        if moves.is_empty() {
            continue;
        }
        let head_q = now.iter().find_map(|f| f.moved.as_ref());
        let base_q = was.iter().find_map(|f| f.moved.as_ref());
        if let Some(why) = unruled(head_q, base_q, Target::Row(row)) {
            for m in moves {
                refuse(row.to_string(), m, format!("{why} in `moved`"));
            }
        }
    }

    // An escape's covers and control, and a probe's cover: none goes back to pending.
    for e in &head.escapes {
        let Some(b) = base.escapes.iter().find(|b| b.id == e.id) else {
            continue;
        };
        let mut moves = Vec::new();
        for (p, was) in &b.covers {
            let now = e.covers.get(p).map_or("", String::as_str);
            if !pending(was) && (now.is_empty() || pending(now)) {
                moves.push(format!(
                    "its cover at {} `{was}` goes back to pending",
                    protection_word(*p)
                ));
            }
        }
        if pending(&e.control) && !pending(&b.control) {
            moves.push(format!("its control `{}` goes back to pending", b.control));
        }
        if moves.is_empty() {
            continue;
        }
        if let Some(why) = unruled(e.moved.as_ref(), b.moved.as_ref(), Target::Row(&e.id)) {
            for m in moves {
                refuse(e.id.clone(), m, format!("{why} in `moved`"));
            }
        }
    }
    for p in &head.real_binary {
        let Some(b) = base.real_binary.iter().find(|b| {
            (b.probe, b.dialect, b.os, b.protection) == (p.probe, p.dialect, p.os, p.protection)
        }) else {
            continue;
        };
        if !(pending(&p.cover) && !pending(&b.cover)) {
            continue;
        }
        let id = match p.probe {
            Probe::Network => "network-probe",
            Probe::File => "file-probe",
        };
        if let Some(why) = unruled(p.moved.as_ref(), b.moved.as_ref(), Target::Row(id)) {
            refuse(
                format!(
                    "the {id}, {} {} at {}",
                    p.dialect.word(),
                    os_word(p.os),
                    protection_word(p.protection)
                ),
                format!("its cover `{}` goes back to `{}`", b.cover, p.cover),
                format!("{why} in `moved`"),
            );
        }
    }

    // A mark approved for the gate: only by a ruling of the base's that names its issue.
    for a in &head.approved_marks {
        let known = base
            .approved_marks
            .iter()
            .any(|b| (b.issue, &b.cell, &b.approved) == (a.issue, &a.cell, &a.approved));
        if known {
            continue;
        }
        if let Some(why) = rulings.refuses(&a.approved, Target::Mark(a.issue)) {
            refuse(
                a.cell.clone(),
                format!("its mark for #{} is approved", a.issue),
                format!("{why} of the base"),
            );
        }
    }
    problems
}

/// **Every cell a scenario cover names, held to the Log, Window and Chat expectations its row
/// states**: ticket 5's validator over each cells file such a cover names, with the list's
/// flags keyed by the cell the cover names. A row whose cover is still pending holds no cell to
/// anything yet.
pub fn expectations(list: &Coverage, repo: &Repo, validator: &mut Validator) -> Vec<String> {
    let mut per_file: BTreeMap<String, BTreeMap<(String, Dialect, Os), RowEntry>> = BTreeMap::new();
    for r in list.rows.iter().filter(|r| r.kind == Kind::Scenario) {
        let Ok(Cover::Scenario { file, cell }) = Cover::parse(&r.cover) else {
            continue;
        };
        let e = per_file
            .entry(file)
            .or_default()
            .entry((cell.clone(), r.dialect, r.os))
            .or_insert(RowEntry {
                row: cell,
                dialect: r.dialect,
                os: r.os,
                log: false,
                window: false,
                chat: false,
            });
        e.log |= r.log;
        e.window |= r.window;
        e.chat |= r.chat;
    }
    let mut problems = Vec::new();
    for (file, entries) in per_file {
        let Some((_, batch)) = repo.batches.iter().find(|(f, _)| *f == file) else {
            continue;
        };
        validator.rows = RowTable {
            entries: entries.into_values().collect(),
        };
        problems.extend(
            validator
                .check(batch)
                .into_iter()
                .map(|p| format!("{file}: {p}")),
        );
    }
    problems
}

/// One mark's reach: a cell, a dialect, an OS, a leg, timings and its issue.
type MarkKey = (String, Dialect, Os, Leg, TimingsMode, u64);

const LEGS: [Leg; 4] = [Leg::Fake, Leg::ClaudeSandbox, Leg::RealWrap, Leg::Live];
const TIMINGS: [TimingsMode; 2] = [TimingsMode::Scaled, TimingsMode::Production];

/// Every mark's reach in `batches`, every cell (`<file>#<id>`), and every issue a mark names.
fn marks(batches: &[(String, Batch)]) -> (BTreeSet<MarkKey>, BTreeSet<String>, BTreeSet<u64>) {
    let (mut keys, mut cells, mut issues) = (BTreeSet::new(), BTreeSet::new(), BTreeSet::new());
    for (file, batch) in batches {
        for cell in &batch.cells {
            let name = format!("{file}#{}", cell.id);
            for m in &cell.expected_fail {
                issues.insert(m.issue);
                for &d in cell
                    .dialects
                    .iter()
                    .filter(|&&d| m.dialect.is_none_or(|x| x == d))
                {
                    for os in OSES.into_iter().filter(|&o| m.os.is_none_or(|x| x == o)) {
                        for leg in LEGS.into_iter().filter(|&l| m.leg.is_none_or(|x| x == l)) {
                            for t in TIMINGS
                                .into_iter()
                                .filter(|&t| m.timings.is_none_or(|x| x == t))
                            {
                                keys.insert((name.clone(), d, os, leg, t, m.issue));
                            }
                        }
                    }
                }
            }
            cells.insert(name);
        }
    }
    (keys, cells, issues)
}

/// **The marks a change adds against `base`'s cells**, where it may not: on a cell `base` has,
/// for an issue that is not new. A new issue is one no mark of `base` names, opened after the
/// change began (`base_time`, its merge base's commit time, `2026-11-15T05:08:34Z`, as `cache`
/// writes `opened`); an issue the cache doesn't have is not known to be new. Per cell × dialect ×
/// OS × leg × timings.
pub fn ratchet(
    base: &[(String, Batch)],
    head: &[(String, Batch)],
    cache: &Issues,
    base_time: &str,
) -> Vec<String> {
    let (before, cells, issues) = marks(base);
    let (after, _, _) = marks(head);
    let new = |issue: &u64| {
        !issues.contains(issue)
            && cache
                .issues
                .get(issue)
                .is_some_and(|i| i.opened.as_str() > base_time)
    };
    after
        .difference(&before)
        .filter(|(cell, .., issue)| cells.contains(cell) && !new(issue))
        .map(|(cell, d, os, leg, t, issue)| {
            format!(
                "{cell}: a mark for #{issue} is added for {} on {} in the {} leg at {} timings; on an existing cell only the mark of an issue opened after the base ({base_time}) may be added, otherwise marks there are only removed",
                d.word(),
                os_word(*os),
                leg_word(model_leg(*leg)),
                timings_word(*t)
            )
        })
        .collect()
}

/// **Marks naming an issue that is closed, not in the cache, or closed by this change.**
pub fn issues(head: &[(String, Batch)], cache: &Issues, closing: &[u64]) -> Vec<String> {
    let mut problems = BTreeSet::new();
    for (file, batch) in head {
        for cell in &batch.cells {
            for m in &cell.expected_fail {
                let at = format!("{file}#{}", cell.id);
                let n = m.issue;
                match cache.issues.get(&n).map(|i| i.state) {
                    None => problems.insert(format!(
                        "{at}: #{n} is not in issues.json; refresh it with `node tools/scenario-coverage.mjs issues`"
                    )),
                    Some(IssueState::Closed) => problems.insert(format!(
                        "{at}: an expected_fail names #{n}, which is closed; remove the mark"
                    )),
                    Some(IssueState::Open) => false,
                };
                if closing.contains(&n) {
                    problems.insert(format!(
                        "{at}: this change closes #{n}, which its expected_fail marks still name"
                    ));
                }
            }
        }
    }
    problems.into_iter().collect()
}

/// Every cover in the list, with what it covers.
fn every_cover(list: &Coverage) -> Vec<(String, &str)> {
    let mut all = Vec::new();
    for r in &list.rows {
        all.push((
            format!("{} {} {}", r.row, r.dialect.word(), os_word(r.os)),
            r.cover.as_str(),
        ));
    }
    for f in &list.fixtures {
        all.push((format!("{} {}", f.row, os_word(f.os)), f.cover.as_str()));
    }
    for p in &list.real_binary {
        all.push((
            format!(
                "a real-binary probe, {} {}",
                p.dialect.word(),
                os_word(p.os)
            ),
            p.cover.as_str(),
        ));
    }
    for e in &list.escapes {
        for (p, cover) in &e.covers {
            all.push((
                format!("{} at {}", e.id, protection_word(*p)),
                cover.as_str(),
            ));
        }
        all.push((format!("{}'s positive control", e.id), e.control.as_str()));
    }
    all
}

/// **What M74.1's gate refuses beyond [`check`]**: a cover still pending, and a mark the person
/// did not approve by name.
pub fn at_gate(list: &Coverage, head: &[(String, Batch)], rulings: &Rulings) -> Vec<String> {
    let mut problems = Vec::new();
    for (at, cover) in every_cover(list) {
        if let Ok(Cover::Pending { ticket }) = Cover::parse(cover) {
            problems.push(format!("{at}: still pending (ticket {ticket})"));
        }
    }
    let mut unapproved = BTreeSet::new();
    for (file, batch) in head {
        for cell in &batch.cells {
            let name = format!("{file}#{}", cell.id);
            for m in &cell.expected_fail {
                let approved = list.approved_marks.iter().any(|a| {
                    a.issue == m.issue
                        && a.cell == name
                        && rulings
                            .refuses(&a.approved, Target::Mark(a.issue))
                            .is_none()
                });
                if !approved {
                    unapproved.insert(format!(
                        "{name}: an expected_fail for #{} the person did not approve by name",
                        m.issue
                    ));
                }
            }
        }
    }
    problems.extend(unapproved);
    problems
}

/// **A leg that ran in this run**: on which OS, at which timings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ran {
    pub leg: ReportLeg,
    pub os: Os,
    pub timings: TimingsMode,
}

/// One entry a leg owes a verdict on.
struct Need<'a> {
    what: String,
    kind: EntryKind,
    row: String,
    test: Option<String>,
    dialect: Option<Dialect>,
    protection: Option<Protection>,
    os: Os,
    leg: ReportLeg,
    timings: Option<TimingsMode>,
    cell: Option<&'a Cell>,
}

/// Each scenario cell `cover` names, in each leg and at each timings `trigger`'s run owes it.
#[allow(clippy::too_many_arguments)]
fn cell_needs<'a>(
    repo: &'a Repo,
    trigger: Trigger,
    cover: &str,
    what: &str,
    kind: EntryKind,
    dialect: Dialect,
    os: Os,
    protection: Option<Protection>,
    legs: &[ReportLeg],
    needs: &mut Vec<Need<'a>>,
) {
    let Ok(Cover::Scenario { file, cell }) = Cover::parse(cover) else {
        return;
    };
    let Some((_, c)) = repo.cell(&file, &cell) else {
        return;
    };
    for t in TIMINGS {
        if schedule(c, repo.catalogue.as_ref(), t, trigger) != Scheduled::Run {
            continue;
        }
        for &leg in legs {
            needs.push(Need {
                what: what.to_string(),
                kind,
                row: c.id.clone(),
                test: None,
                dialect: Some(dialect),
                protection,
                os,
                leg,
                timings: Some(t),
                cell: Some(c),
            });
        }
    }
}

fn test_need<'a>(what: &str, row: &str, cover: &str, os: Os) -> Option<Need<'a>> {
    let (kind, leg, test) = match Cover::parse(cover).ok()? {
        Cover::Fixture { test, .. } => (EntryKind::Fixture, ReportLeg::Fixture, test),
        Cover::E2e { test, .. } => (EntryKind::E2e, ReportLeg::E2e, test),
        _ => return None,
    };
    Some(Need {
        what: format!("{what} ({test})"),
        kind,
        row: row.to_string(),
        test: Some(test),
        dialect: None,
        protection: None,
        os,
        leg,
        timings: None,
        cell: None,
    })
}

/// Everything the list owes a verdict on in a run `trigger` started.
fn needs<'a>(list: &Coverage, repo: &'a Repo, trigger: Trigger) -> Vec<Need<'a>> {
    let mut needs = Vec::new();
    for r in &list.rows {
        let what = format!("{} {} {}", r.row, r.dialect.word(), os_word(r.os));
        match r.kind {
            Kind::Scenario => {
                let legs = scenario_legs(r.dialect, r.os, r.binary);
                cell_needs(
                    repo,
                    trigger,
                    &r.cover,
                    &what,
                    EntryKind::Cell,
                    r.dialect,
                    r.os,
                    None,
                    &legs,
                    &mut needs,
                );
            }
            Kind::Live => {
                cell_needs(
                    repo,
                    trigger,
                    &r.cover,
                    &what,
                    EntryKind::Cell,
                    r.dialect,
                    r.os,
                    None,
                    &[ReportLeg::Live],
                    &mut needs,
                );
            }
            Kind::Fixture | Kind::E2e => needs.extend(test_need(&what, &r.row, &r.cover, r.os)),
        }
    }
    for f in &list.fixtures {
        let what = format!("{} {}", f.row, os_word(f.os));
        needs.extend(test_need(&what, &f.row, &f.cover, f.os));
    }
    for p in &list.real_binary {
        let what = format!(
            "the real-binary probe {} {} at {}",
            p.dialect.word(),
            os_word(p.os),
            protection_word(p.protection)
        );
        let legs = scenario_legs(p.dialect, p.os, Binary::Real);
        cell_needs(
            repo,
            trigger,
            &p.cover,
            &what,
            EntryKind::Cell,
            p.dialect,
            p.os,
            Some(p.protection),
            &legs,
            &mut needs,
        );
    }
    let table: BTreeMap<&str, &EscapeRow> =
        repo.escapes.iter().map(|e| (e.id.as_str(), e)).collect();
    for e in &list.escapes {
        let Some(row) = table.get(e.id.as_str()) else {
            continue;
        };
        let pairs = row.applies().unwrap_or_default();
        for &(d, os) in &pairs {
            let ten = d == Dialect::Claude && os == Os::Macos;
            let primary = if ten {
                ReportLeg::ClaudeSandbox
            } else {
                ReportLeg::Fake
            };
            for (p, cover) in &e.covers {
                let mut legs = vec![primary];
                if row.real_binary() && !ten {
                    legs.push(ReportLeg::RealWrap);
                }
                let what = format!(
                    "{} {} {} at {}",
                    e.id,
                    d.word(),
                    os_word(os),
                    protection_word(*p)
                );
                cell_needs(
                    repo,
                    trigger,
                    cover,
                    &what,
                    EntryKind::Escape,
                    d,
                    os,
                    Some(*p),
                    &legs,
                    &mut needs,
                );
            }
            if let Ok(Cover::Scenario { cell, .. }) = Cover::parse(&e.control) {
                let what = format!("{cell} {} {}", d.word(), os_word(os));
                cell_needs(
                    repo,
                    trigger,
                    &e.control,
                    &what,
                    EntryKind::Cell,
                    d,
                    os,
                    None,
                    &[primary],
                    &mut needs,
                );
            }
        }
    }
    needs
}

/// Whether one of `cell`'s marks pins a failure for `dialect` on `os` in `leg` at `timings`.
fn pinned(
    cell: &Cell,
    dialect: Option<Dialect>,
    os: Os,
    leg: ReportLeg,
    timings: Option<TimingsMode>,
) -> bool {
    cell.expected_fail.iter().any(|m| {
        dialect.is_none_or(|d| m.dialect.is_none_or(|x| x == d))
            && m.os.is_none_or(|x| x == os)
            && m.leg.is_none_or(|x| model_leg(x) == leg)
            && timings.is_none_or(|t| m.timings.is_none_or(|x| x == t))
    })
}

/// The pass a leg belongs to: the scenario legs run together, and the live pass, the fixtures
/// and the e2e specs each on their own.
fn pass_of(leg: ReportLeg) -> u8 {
    match leg {
        ReportLeg::Fake | ReportLeg::ClaudeSandbox | ReportLeg::RealWrap => 0,
        ReportLeg::Live => 1,
        ReportLeg::Fixture => 2,
        ReportLeg::E2e => 3,
    }
}

/// **Every covered entry this run owed, with neither a pass nor a pinned `expected_fail`** in
/// its reports. A run owes every entry of each pass it ran on an OS at a timings, in the leg its
/// cover requires, whether or not that leg is in `ran`: a scenario run on Linux that left out
/// ticket 53's real-binary leg owes that leg's verdicts. A pass that did not run on an OS owes
/// nothing there; a cover still pending is judged by [`at_gate`].
pub fn judge_runs(
    list: &Coverage,
    repo: &Repo,
    trigger: Trigger,
    ran: &[Ran],
    records: &[LegRecord],
) -> Vec<String> {
    let mut problems = Vec::new();
    for n in needs(list, repo, trigger) {
        let in_this_run = ran.iter().any(|r| {
            pass_of(r.leg) == pass_of(n.leg)
                && r.os == n.os
                && n.timings.is_none_or(|t| r.timings == t)
        });
        if !in_this_run {
            continue;
        }
        let found: Vec<&LegRecord> = records
            .iter()
            .filter(|r| {
                r.kind == n.kind
                    && r.row == n.row
                    && r.os == n.os
                    && r.leg == n.leg
                    && n.test.as_ref().is_none_or(|t| r.test.as_ref() == Some(t))
                    && n.dialect.is_none_or(|d| r.dialect == Some(d))
                    && n.protection.is_none_or(|p| r.protection == Some(p))
                    && n.timings
                        .is_none_or(|t| r.timings.unwrap_or(TimingsMode::Scaled) == t)
            })
            .collect();
        let at = match n.timings {
            Some(t) => format!(
                "the {} leg on {} at {} timings",
                leg_word(n.leg),
                os_word(n.os),
                timings_word(t)
            ),
            None => format!("the {} leg on {}", leg_word(n.leg), os_word(n.os)),
        };
        if found.iter().any(|r| r.verdict == ReportVerdict::Pass) {
            continue;
        }
        if found
            .iter()
            .any(|r| r.verdict == ReportVerdict::ExpectedFail)
        {
            if n.cell
                .is_some_and(|c| pinned(c, n.dialect, n.os, n.leg, n.timings))
            {
                continue;
            }
            problems.push(format!(
                "{}: expected_fail in {at}, and no mark pins it",
                n.what
            ));
            continue;
        }
        match found.first() {
            Some(r) => problems.push(format!(
                "{}: {} in {at}{}",
                n.what,
                match r.verdict {
                    ReportVerdict::Fail => "fail",
                    _ => "not_run",
                },
                r.reason
                    .as_deref()
                    .map(|why| format!(": {why}"))
                    .unwrap_or_default()
            )),
            None => problems.push(format!("{}: did not run in {at}", n.what)),
        }
    }
    problems
}

/// **The report**: covers per kind, in each part of the list, and the marks per issue.
pub fn report(list: &Coverage, repo: &Repo) -> String {
    let kind_of = |cover: &str| match Cover::parse(cover) {
        Ok(Cover::Pending { .. }) => "pending",
        Ok(c) => c.kind().map_or("pending", Kind::word),
        Err(_) => "unreadable",
    };
    let mut counts: BTreeMap<&str, [usize; 4]> = BTreeMap::new();
    let mut add = |part: usize, cover: &str| counts.entry(kind_of(cover)).or_default()[part] += 1;
    for r in &list.rows {
        add(0, &r.cover);
    }
    for f in &list.fixtures {
        add(1, &f.cover);
    }
    for p in &list.real_binary {
        add(2, &p.cover);
    }
    for e in &list.escapes {
        for cover in e.covers.values() {
            add(3, cover);
        }
        add(3, &e.control);
    }
    let mut out = String::from(
        "## Matrix coverage (T1.1-08)\n\n| cover | row × dialect × OS | fixture rows | real-binary probes | escape covers and controls |\n|---|---|---|---|---|\n",
    );
    for kind in [
        "scenario",
        "fixture",
        "e2e",
        "live",
        "pending",
        "unreadable",
    ] {
        if let Some([a, b, c, d]) = counts.get(kind) {
            out.push_str(&format!("| {kind} | {a} | {b} | {c} | {d} |\n"));
        }
    }
    let mut per_issue: BTreeMap<u64, Vec<String>> = BTreeMap::new();
    for (file, batch) in &repo.batches {
        for cell in &batch.cells {
            for m in &cell.expected_fail {
                per_issue
                    .entry(m.issue)
                    .or_default()
                    .push(format!("{file}#{}", cell.id));
            }
        }
    }
    out.push_str("\n## expected_fail per issue\n\n");
    if per_issue.is_empty() {
        out.push_str("none\n");
    }
    for (issue, cells) in per_issue {
        let mut names = cells.clone();
        names.dedup();
        out.push_str(&format!(
            "- #{issue}: {} ({})\n",
            cells.len(),
            names.join(", ")
        ));
    }
    out
}
