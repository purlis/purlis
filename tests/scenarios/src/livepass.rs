// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The live-pass tools** (T1.1-37, #6884): a brief generator and a live-versus-suite diff.
//!
//! The live pass runs the checks only a live harness shows (spec, "What only a live harness
//! shows"), on an installed release-profile build of `phase-1-proven`'s head, with no
//! `test-support`. It is **record only**: the runner does what the brief says and keeps the
//! event log; nothing in the run judges anything.
//!
//! - [`brief`] builds one harness's brief on one OS from the files the suite already owns:
//!   the cells `coverage.json` covers `live:` (including an escape's), one control per hook
//!   kind the harness is armed with, the escapes ticket 43's table marks live, one check per
//!   switch or substitution of ticket 6's reviewed list that the macOS release-build pass
//!   covers, ticket 25's timed (g) cell on macOS, and the macOS denial receiver's check
//!   when its row is carried.
//! - [`live_report`] judges a live run's log by the cells' own marks and writes the suite's
//!   report format (`schema/leg-report.schema.json`, leg `live`).
//! - [`diff`] compares a live report with a suite report, per cell: verdicts, and event
//!   sequences where both sides recorded them.
//!
//! Nothing here names a harness: a dialect is the key to its armed hook words and nothing else.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Write as _};
use std::fs;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::coverage::{Cover, Coverage, Repo};
use crate::judge::{Assertion, GlobalFailure, Observed, Run, Verdict, judge};
use crate::kinds::dialect_words;
use crate::model::{
    Batch, Cell, Dialect, EntryKind, Expect, Leg, LegRecord, Os, Protection, ReportLeg,
    ReportVerdict, Require, TimingsMode,
};
use crate::timings::catalogue;

/// The test whose cover, on a carried macOS `M-FILE` row, asks for the receiver's live check
/// (ticket 52): the receiver reports for a standard, non-admin user.
pub const RECEIVER_FIXTURE: &str = "the_receiver_reports_for_a_standard_non_admin_user";

/// Ticket 25's timed (g) confirmation cell on macOS: the host-to-window hop.
pub const HELD_ASK_TIMING: &str = "held-ask-timing-g";

/// The cover of ticket 6's switch list that this pass answers.
const MACOS_LIVE_CHECK: &str = "macos-live-check";

// ---- the brief ---------------------------------------------------------------------------------

/// Why a check is in the brief.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Why {
    /// A cell the coverage list covers `live:`.
    LiveCover,
    /// One control per hook kind: the hook fires, and purlis records it.
    HookControl,
    /// An escape the escape table marks live, at one protection.
    Escape,
    /// A test-only switch whose cover is the macOS release-build pass (ticket 6's list).
    ReleaseSwitch,
    /// A fixture substitution whose cover is the macOS release-build pass (ticket 6's list).
    ReleaseSubstitution,
    /// Ticket 25's timed (g) confirmation cell.
    HeldAskTiming,
    /// The macOS denial receiver, for a standard, non-admin user.
    Receiver,
}

impl Why {
    fn heading(self) -> &'static str {
        match self {
            Why::LiveCover => "Cells the matrix covers live",
            Why::HookControl => "Controls: one per hook kind",
            Why::Escape => "Escapes the table marks live",
            Why::ReleaseSubstitution => "Release build: fixture substitutions",
            Why::ReleaseSwitch => "Release build: test-only switches",
            Why::HeldAskTiming => "Timed held ask (ticket 25, hypothesis (g))",
            Why::Receiver => "The macOS denial receiver",
        }
    }
}

/// **One check a runner does and records.**
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Check {
    /// What the runner's record names it by: a cell's id, or `control:…`, `escape:…`,
    /// `release:…`.
    pub id: String,
    pub why: Why,
    /// What to do.
    pub what: String,
    /// What the record should show.
    pub observable: String,
}

/// **One harness's brief on one OS.**
#[derive(Debug, Clone, Serialize)]
pub struct Brief {
    pub harness: Dialect,
    pub os: Os,
    pub checks: Vec<Check>,
}

/// **Ticket 6's reviewed list** (`app/src-tauri/src/scenarios/switches.json`): the fixture's
/// substitutions and the test-only switches on a scenario's paths, each with the covers that
/// stand for it per OS. Read here only for the entries whose macOS cover is this pass. Every
/// key is named, so a list that grew or renamed one is an error, not a quiet omission.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Switches {
    /// The covers' own names, with what each stands for.
    #[serde(default)]
    pub covers: BTreeMap<String, String>,
    #[serde(default)]
    pub substitutions: Vec<Substitution>,
    #[serde(default)]
    pub switches: Vec<Switch>,
}

/// A fixture substitution of the list.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Substitution {
    pub name: String,
    pub reason: String,
    /// Per OS, the covers that stand for the entry.
    #[serde(default)]
    pub covers: BTreeMap<String, Vec<String>>,
}

/// A test-only switch of the list: its file and the line it sits at.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Switch {
    pub file: String,
    pub at: String,
    pub kind: String,
    #[serde(default)]
    pub count: Option<u32>,
    /// How a scenario forces the switch's other branch, where it does.
    #[serde(default)]
    pub forced: Option<Value>,
    pub reason: String,
    /// Per OS, the covers that stand for the entry.
    #[serde(default)]
    pub covers: BTreeMap<String, Vec<String>>,
}

fn covered_by_macos_pass(covers: &BTreeMap<String, Vec<String>>) -> bool {
    covers
        .get("macos")
        .is_some_and(|c| c.iter().any(|c| c == MACOS_LIVE_CHECK))
}

impl Switches {
    /// This checkout's list.
    pub fn repo() -> Result<Self, String> {
        let path = crate::root().join("../../app/src-tauri/src/scenarios/switches.json");
        let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
    }
}

/// **What a brief is made from.**
pub struct Sources<'a> {
    pub coverage: &'a Coverage,
    pub repo: &'a Repo,
    pub switches: &'a Switches,
}

/// **The brief for `harness` on `os`.** An error lists every `live:` cover that names no live
/// cell for this harness: a brief that quietly dropped one would pass for complete.
pub fn brief(harness: Dialect, os: Os, src: &Sources<'_>) -> Result<Brief, String> {
    let mut checks = Vec::new();
    let mut problems = Vec::new();

    // 1. Every cell the list covers live, for this harness on this OS. A cover that cannot be
    // read, or an escape the table does not have, is a problem: never a quiet skip.
    let live = |text: &str| match Cover::parse(text) {
        Ok(Cover::Live { cell }) => Ok(Some(cell)),
        Ok(_) => Ok(None),
        Err(e) => Err(e),
    };
    let mut wanted: Vec<String> = Vec::new();
    for r in &src.coverage.rows {
        if r.dialect != harness || r.os != os {
            continue;
        }
        match live(&r.cover) {
            Ok(cell) => wanted.extend(cell),
            Err(e) => problems.push(format!("{}: {e}", r.row)),
        }
    }
    for e in &src.coverage.escapes {
        let Some(row) = src.repo.escapes.iter().find(|row| row.id == e.id) else {
            problems.push(format!(
                "{}: the cover list has it, the escape table does not",
                e.id
            ));
            continue;
        };
        let applies = match row.applies() {
            Ok(pairs) => pairs.contains(&(harness, os)),
            Err(err) => {
                problems.push(err);
                continue;
            }
        };
        if !applies {
            continue;
        }
        for text in e.covers.values().chain([&e.control]) {
            match live(text) {
                Ok(cell) => wanted.extend(cell),
                Err(err) => problems.push(format!("{}: {err}", e.id)),
            }
        }
    }
    let mut done = BTreeSet::new();
    for id in wanted {
        if !done.insert(id.clone()) {
            continue;
        }
        let found = src
            .repo
            .batches
            .iter()
            .flat_map(|(_, b)| &b.cells)
            .find(|c| c.id == id && c.live_only.is_some());
        match found {
            Some(cell) if cell.dialects.contains(&harness) => checks.push(Check {
                id: id.clone(),
                why: Why::LiveCover,
                what: what_of(cell),
                observable: observable_of(cell),
            }),
            Some(_) => problems.push(format!(
                "live cover `{id}` is for {harness:?} on {os:?}, but the cell does not run in that dialect"
            )),
            None => problems.push(format!("live cover `{id}` names no live-only cell")),
        }
    }

    // 2. One control per hook kind this harness is armed with.
    for word in dialect_words(harness) {
        checks.push(Check {
            id: format!("control:hook.{word}"),
            why: Why::HookControl,
            what: format!(
                "Make the harness fire its `{word}` hook once, with nothing refused: a control, so that a missing event elsewhere is the cell's, not the hook's."
            ),
            observable: format!("`hook.{word}` is in the run's event log."),
        });
    }

    // 3. The escapes the table marks live, at each protection that owes something.
    for row in &src.repo.escapes {
        match row.live.as_str() {
            "yes" => {}
            "no" | "" => continue,
            other => {
                problems.push(format!(
                    "{}: its `live` column is `{other}`, not yes or no",
                    row.id
                ));
                continue;
            }
        }
        match row.applies() {
            Ok(pairs) if pairs.contains(&(harness, os)) => {}
            Ok(_) => continue,
            Err(e) => {
                problems.push(e);
                continue;
            }
        }
        for (word, owed) in [
            ("full", &row.full),
            ("basic", &row.basic),
            ("none", &row.off),
        ] {
            if owed.starts_with("n/a") || owed.starts_with("allowed by design") {
                continue;
            }
            checks.push(Check {
                id: format!("escape:{}:{word}", row.id),
                why: Why::Escape,
                what: format!("Try escape {} at {word}; purlis owes: {owed}", row.id),
                observable: row.live_observable.clone(),
            });
        }
    }

    // 4. The release build's switches and substitutions that this pass covers, on macOS.
    if os == Os::Macos {
        for e in src
            .switches
            .substitutions
            .iter()
            .filter(|e| covered_by_macos_pass(&e.covers))
        {
            let name = &e.name;
            checks.push(Check {
                id: format!("release:substitution:{name}"),
                why: Why::ReleaseSubstitution,
                what: format!(
                    "On the installed release build, use what the scenario host replaced with a stand-in: {name}. Why it is a stand-in there: {}",
                    e.reason
                ),
                observable: RELEASE_OBSERVABLE.to_owned(),
            });
        }
        for e in src
            .switches
            .switches
            .iter()
            .filter(|e| covered_by_macos_pass(&e.covers))
        {
            let (file, at) = (&e.file, &e.at);
            checks.push(Check {
                id: format!("release:switch:{file}:{at}"),
                why: Why::ReleaseSwitch,
                what: format!(
                    "On the installed release build, do one thing that goes through `{at}` in {file}, where the test build takes its own branch. Why it differs there: {}",
                    e.reason
                ),
                observable: RELEASE_OBSERVABLE.to_owned(),
            });
        }
    }

    // 5. Ticket 25's timed (g) cell, and 6. the receiver's check, on macOS.
    if os == Os::Macos {
        checks.push(Check {
            id: HELD_ASK_TIMING.to_owned(),
            why: Why::HeldAskTiming,
            what: "From the chat, connect to a name nothing allows, with a client timeout of 150 s, and leave the ask unanswered until the hold ends. Time the ask and the block against the connection.".to_owned(),
            observable: "`ask.raised` comes about `group` after the connection (chat 682 saw 35 to 45 s), and `stop.block` comes at `hold`, in the same turn, not at the next prompt. A mismatch reopens ticket 26.".to_owned(),
        });
        if receiver_row_is_carried(src.coverage) {
            checks.push(Check {
                id: RECEIVER_FIXTURE.to_owned(),
                why: Why::Receiver,
                what: "As a standard, non-admin user, make a chat's wrapped process touch a file its sandbox refuses, and read what the macOS denial receiver reports.".to_owned(),
                observable: "The receiver reports the refusal as a verified, counted block (the same `Refusal` the Linux supervisor tells), for a user without admin rights.".to_owned(),
            });
        }
    }

    if problems.is_empty() {
        Ok(Brief {
            harness,
            os,
            checks,
        })
    } else {
        Err(problems.join("\n"))
    }
}

const RELEASE_OBSERVABLE: &str = "It runs as the scenario host's cell does: the same events in the log, and no error that names a missing kit, host process, plugin or hook.";

/// Whether a macOS `M-FILE` row names the receiver's test as its fixture cover.
fn receiver_row_is_carried(coverage: &Coverage) -> bool {
    coverage.fixtures.iter().any(|f| {
        f.os == Os::Macos
            && f.row.starts_with("M-FILE")
            && matches!(Cover::parse(&f.cover), Ok(Cover::Fixture { test, .. }) if test == RECEIVER_FIXTURE)
    })
}

/// What a cell asks the runner to do: each step's calls, press and wait.
fn what_of(cell: &Cell) -> String {
    let mut out = format!(
        "Live-only ({}).",
        cell.live_only.as_ref().map_or("", |l| &l.reason)
    );
    for (i, step) in cell.steps.iter().enumerate() {
        let n = i + 1;
        for call in &step.calls {
            let input = call
                .input
                .get("command")
                .and_then(Value::as_str)
                .map_or_else(|| call.input.to_string(), str::to_owned);
            let _ = write!(out, " Step {n}: {} `{input}`.", call.tool);
        }
        if let Some(wait) = &step.wait {
            let _ = write!(out, " Step {n}: wait {wait}.");
        }
        if step.press.is_some() {
            let _ = write!(out, " Step {n}: the person answers as the cell says.");
        }
    }
    out
}

/// What the record should show: the step's `require` and `forbid`, as the suite reads them.
fn observable_of(cell: &Cell) -> String {
    let mut parts = Vec::new();
    for (i, step) in cell.steps.iter().enumerate() {
        let n = i + 1;
        for r in &step.require {
            let events: Vec<String> = r.events().into_iter().map(expect_text).collect();
            let joined = match r {
                Require::Either { .. } => events.join(" or "),
                Require::Order { .. } => events.join(" before "),
                Require::Event(_) => events.join(""),
            };
            parts.push(format!("step {n} requires {joined}"));
        }
        for f in &step.forbid {
            parts.push(format!("step {n} forbids {}", expect_text(f)));
        }
        for s in &step.transcript {
            parts.push(format!(
                "step {n} transcript {} \"{}\"",
                if s.forbid { "forbids" } else { "says" },
                s.says
            ));
        }
    }
    parts.join("; ")
}

fn expect_text(e: &Expect) -> String {
    let mut out = format!("`{}`", e.kind);
    for (k, v) in &e.fields {
        let _ = write!(out, " {k}={v}");
    }
    if let Some(c) = &e.count {
        let _ = write!(out, " count={c}");
    }
    if let Some(w) = &e.within {
        let _ = write!(out, " within {w}");
    }
    if let Some(b) = &e.before {
        let _ = write!(out, " before {b}");
    }
    out
}

impl Why {
    /// Whether `live-pass report` judges this check's record. The rest are read by a person.
    pub fn judged_by_report(self) -> bool {
        self == Why::LiveCover
    }
}

impl Brief {
    /// **The brief a runner chat reads.**
    pub fn markdown(&self) -> String {
        let os = match self.os {
            Os::Linux => "Linux",
            Os::Macos => "macOS",
        };
        let mut out = format!("# Live pass: {} on {os}\n\n", self.harness.word());
        out.push_str(&format!(
            "You are a runner. This pass is **record only**: do what each check says and keep the event log; the run itself decides nothing.\n\n\
## Set-up\n\n\
- Use an installed **release** build of `phase-1-proven`'s head, built in the release profile without `test-support` or `fenced`. Never a test build.\n\
- Run the `{}` harness in its **non-interactive** mode, one check at a time, so nobody answers a prompt unless a check says the person does.\n\
- Keep each check's event log slice under the check's id below.\n\
- Do not retry a check. If it cannot run, say so with the reason; a cell you leave out of the log is `not_run`.\n\n\
## How each record is judged\n\n\
- **Cells** (the first group) are judged by `tools/live-pass report`, by the cell's own expectations. Write the log it reads as one JSON file, an entry per cell you ran:\n\n\
```json\n\
{{\"cells\": {{\"<cell id>\": {{\"failed\": [[\"require[0]\", <observed>]], \"globals\": [], \"events\": [\"hook.pretooluse\", \"stop.block\"]}}}}}}\n\
```\n\n\
  `failed` and `globals` are both required, and empty when nothing failed. A failed assertion is `[assertion, observed]` of step 1, or `[step, assertion, observed]`; every assertion not listed passed. A global check is `{{\"check\": \"…\", \"refusal\": \"…\"}}`. `events` is the cell's event kinds in log order, and optional. Any other key, a cell id that is not in the cells files, or a wrong type is an error, never a pass.\n\
- **Everything else** (controls, escapes, release checks, the held-ask timing and the receiver check) is **judged by a person**, not by `report` or `diff`: write, for each, its id and what the log showed, and the person reads it against that check's `Record:` line and files a bug for any that does not show it, before tagging.\n\n",
            self.harness.word()
        ));
        for why in [
            Why::LiveCover,
            Why::HookControl,
            Why::Escape,
            Why::ReleaseSubstitution,
            Why::ReleaseSwitch,
            Why::HeldAskTiming,
            Why::Receiver,
        ] {
            let group: Vec<&Check> = self.checks.iter().filter(|c| c.why == why).collect();
            if group.is_empty() {
                continue;
            }
            let judge = if why.judged_by_report() {
                "judged by `report`"
            } else {
                "judged by a person"
            };
            let _ = write!(out, "## {} ({}, {judge})\n\n", why.heading(), group.len());
            for c in group {
                let _ = write!(
                    out,
                    "### `{}`\n\n{}\n\nRecord: {}\n\n",
                    c.id, c.what, c.observable
                );
            }
        }
        out
    }
}

// ---- the live report ---------------------------------------------------------------------------

/// **A leg's record, with the events the leg recorded for it** (`events` is optional in
/// `schema/leg-report.schema.json`): what the diff compares as the cell's event sequence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Recorded {
    #[serde(flatten)]
    pub leg: LegRecord,
    /// Event kinds, in the order the log wrote them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub events: Option<Vec<String>>,
}

/// The live log: `{"cells": {"<id>": {...}}}`. Every key is named.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Log {
    /// Free text about where the log came from.
    #[serde(default, rename = "note")]
    _note: Option<String>,
    cells: BTreeMap<String, Value>,
}

/// **The live log as text**, read so that a cell id given twice is an error: parsed into a
/// `Value` a repeated key is last-wins, and the first entry would vanish unseen.
pub fn read_log(path: &str, text: &str) -> Result<Value, String> {
    let log: Value = {
        let mut de = serde_json::Deserializer::from_str(text);
        let v = UniqueCells::deserialize(&mut de).map_err(|e| format!("{path}: {e}"))?;
        de.end().map_err(|e| format!("{path}: {e}"))?;
        v.0
    };
    Ok(log)
}

/// A JSON document whose top-level `cells` object may not repeat a key.
struct UniqueCells(Value);

impl<'de> Deserialize<'de> for UniqueCells {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Top;
        impl<'de> serde::de::Visitor<'de> for Top {
            type Value = Value;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a JSON object")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(self, mut m: A) -> Result<Value, A::Error> {
                let mut out = serde_json::Map::new();
                while let Some(key) = m.next_key::<String>()? {
                    let value = if key == "cells" {
                        m.next_value::<Cells>()?.0
                    } else {
                        m.next_value::<Value>()?
                    };
                    if out.insert(key.clone(), value).is_some() {
                        return Err(serde::de::Error::custom(format!("`{key}` is given twice")));
                    }
                }
                Ok(Value::Object(out))
            }
        }
        d.deserialize_map(Top).map(UniqueCells)
    }
}

struct Cells(Value);

impl<'de> Deserialize<'de> for Cells {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = Value;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an object of cells")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(self, mut m: A) -> Result<Value, A::Error> {
                let mut out = serde_json::Map::new();
                while let Some((id, entry)) = m.next_entry::<String, Value>()? {
                    if out.insert(id.clone(), entry).is_some() {
                        return Err(serde::de::Error::custom(format!(
                            "the cell `{id}` is in the log twice: one entry would hide the other"
                        )));
                    }
                }
                Ok(Value::Object(out))
            }
        }
        // a `cells` that is not an object is left for `Log` to refuse with its own message
        d.deserialize_any(V).map(Cells)
    }
}

/// One cell's entry. `failed` and `globals` are required: an entry that leaves one out says
/// nothing about it, and nothing is read as "nothing failed".
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LogEntry {
    failed: Vec<LogFailure>,
    globals: Vec<LogGlobal>,
    #[serde(default, deserialize_with = "present")]
    events: Option<Vec<String>>,
}

/// `[assertion, observed]` of step 1, or `[step, assertion, observed]`.
#[derive(Deserialize)]
#[serde(untagged)]
enum LogFailure {
    First(String, Value),
    At(u32, String, Value),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LogGlobal {
    check: String,
    refusal: String,
}

/// A field that is there is the type it names: `null` is no list.
fn present<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Vec<String>>, D::Error> {
    Vec::<String>::deserialize(d).map(Some)
}

/// **The live report of `log` for the cells of `batches` that run in `harness`.**
///
/// `log` is the live checker's account of a run, per cell: `{"cells": {"<id>": {"failed":
/// [["require[0]", 2]], "globals": [{"check": "…", "refusal": "…"}], "events": ["…"]}}}`.
/// A failed assertion is `[assertion, observed]` of step 1, or `[step, assertion, observed]`;
/// every assertion not listed passed. `failed` and `globals` are required, `events` is
/// optional, and no other key is read. Each cell is judged by its own `expected_fail` marks, as
/// the suite judges it. A cell the log does not mention is `not_run`; a log entry that is
/// malformed, or names a cell no batch has for this harness, is an error.
pub fn live_report(
    batches: &[Batch],
    log: &Value,
    harness: Dialect,
    os: Os,
) -> Result<Vec<Recorded>, String> {
    let log: Log = serde_json::from_value(log.clone()).map_err(|e| format!("the log: {e}"))?;
    let known: BTreeSet<&str> = batches
        .iter()
        .flat_map(|b| &b.cells)
        .filter(|c| c.dialects.contains(&harness))
        .map(|c| c.id.as_str())
        .collect();
    let unknown: Vec<&str> = log
        .cells
        .keys()
        .map(String::as_str)
        .filter(|id| !known.contains(id))
        .collect();
    if !unknown.is_empty() {
        return Err(format!(
            "the log names cells that no given cells file has for {}: {}",
            harness.word(),
            unknown.join(", ")
        ));
    }
    let mut ids = BTreeSet::new();
    for c in batches.iter().flat_map(|b| &b.cells) {
        if c.dialects.contains(&harness) && !ids.insert(c.id.as_str()) {
            return Err(format!(
                "the cell `{}` is in more than one given cells file: one log entry would stand for both",
                c.id
            ));
        }
    }
    let catalogue = catalogue();
    let run = Run {
        dialect: harness,
        os,
        leg: Leg::Live,
        timings: TimingsMode::Production,
        seconds: catalogue.production_seconds(),
    };
    let mut out = Vec::new();
    for batch in batches {
        for cell in batch.cells.iter().filter(|c| c.dialects.contains(&harness)) {
            let (verdict, reason, events) = match log.cells.get(&cell.id) {
                None => (
                    ReportVerdict::NotRun,
                    Some("not in the live log".to_owned()),
                    None,
                ),
                Some(one) => {
                    let entry: LogEntry = serde_json::from_value(one.clone())
                        .map_err(|e| format!("{}: the log's entry: {e}", cell.id))?;
                    if let Some(bad) = entry.events.iter().flatten().find(|k| k.is_empty()) {
                        return Err(format!("{}: the log's events hold `{bad}`", cell.id));
                    }
                    let (assertions, globals) = failures(&cell.id, &entry)?;
                    let events = entry.events;
                    match judge(cell, &run, &assertions, &globals) {
                        Verdict::Pass => (ReportVerdict::Pass, None, events),
                        Verdict::ExpectedFail { issues } => {
                            let list: Vec<String> =
                                issues.iter().map(|i| format!("#{i}")).collect();
                            (
                                ReportVerdict::ExpectedFail,
                                Some(format!("pinned: {}", list.join(", "))),
                                events,
                            )
                        }
                        Verdict::Fail { reasons } => {
                            (ReportVerdict::Fail, Some(reasons.join("; ")), events)
                        }
                    }
                }
            };
            out.push(Recorded {
                leg: LegRecord {
                    kind: if cell.escape {
                        EntryKind::Escape
                    } else {
                        EntryKind::Cell
                    },
                    row: cell.id.clone(),
                    test: None,
                    dialect: Some(harness),
                    protection: Some(batch.protection),
                    os,
                    leg: ReportLeg::Live,
                    timings: Some(TimingsMode::Production),
                    verdict,
                    reason,
                    refusal_expected: cell.escape || cell.steps.iter().any(|s| s.refusal_expected),
                },
                events,
            });
        }
    }
    Ok(out)
}

/// The assertions and global checks a cell's log entry says failed.
fn failures(id: &str, one: &LogEntry) -> Result<(Vec<Assertion>, Vec<GlobalFailure>), String> {
    let mut assertions = Vec::new();
    for f in &one.failed {
        let (step, name, seen) = match f {
            LogFailure::First(name, seen) => (1, name, seen),
            LogFailure::At(0, ..) => return Err(format!("{id}: the log's steps count from 1")),
            LogFailure::At(step, name, seen) => (*step, name, seen),
        };
        assertions.push(Assertion {
            step,
            assertion: name.clone(),
            observed: Observed::Fail(seen.clone()),
        });
    }
    let globals = one
        .globals
        .iter()
        .map(|g| GlobalFailure {
            check: g.check.clone(),
            refusal: g.refusal.clone(),
        })
        .collect();
    Ok((assertions, globals))
}

// ---- reading a report --------------------------------------------------------------------------

const REPORT_SCHEMA: &str = include_str!("../schema/leg-report.schema.json");

/// **A report: one JSON record per line**, each checked against `schema/leg-report.schema.json`
/// before it is read. An empty report, or a line that is not a record, is an error.
pub fn read_report(path: &str, text: &str) -> Result<Vec<Recorded>, String> {
    let schema: Value = serde_json::from_str(REPORT_SCHEMA).map_err(|e| e.to_string())?;
    let validator = jsonschema::validator_for(&schema).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for (i, line) in text
        .lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
    {
        let at = format!("{path}:{}", i + 1);
        let value: Value = serde_json::from_str(line).map_err(|e| format!("{at}: {e}"))?;
        let errors: Vec<String> = validator
            .iter_errors(&value)
            .map(|e| e.to_string())
            .collect();
        if !errors.is_empty() {
            return Err(format!(
                "{at}: not a record of the leg-report schema: {}",
                errors.join("; ")
            ));
        }
        out.push(serde_json::from_value(value).map_err(|e| format!("{at}: {e}"))?);
    }
    if out.is_empty() {
        return Err(format!("{path}: the report holds no records"));
    }
    Ok(out)
}

/// **The suite's records the live run can be compared with**: those whose row, dialect,
/// protection and OS the live report also has, so a suite report of every cell does not list an
/// `only suite` line for each cell the live report was never about. Returns the kept records.
pub fn scope_suite(live: &[Recorded], suite: Vec<Recorded>) -> Vec<Recorded> {
    let rows: BTreeSet<&str> = live.iter().map(|r| r.leg.row.as_str()).collect();
    let dialects: BTreeSet<_> = live.iter().map(|r| r.leg.dialect).collect();
    let protections: BTreeSet<_> = live.iter().map(|r| r.leg.protection).collect();
    let oses: BTreeSet<_> = live.iter().map(|r| r.leg.os).collect();
    suite
        .into_iter()
        .filter(|s| {
            rows.contains(s.leg.row.as_str())
                && dialects.contains(&s.leg.dialect)
                && protections.contains(&s.leg.protection)
                && oses.contains(&s.leg.os)
        })
        .collect()
}

/// **A live report's own checks**: every record is of the `live` leg, and the file is not
/// empty. A suite report on the left would compare a leg with itself and agree.
pub fn check_live(live: &[Recorded]) -> Result<(), String> {
    match live.iter().find(|r| r.leg.leg != ReportLeg::Live) {
        Some(r) => Err(format!(
            "the live report holds a {:?} record ({}): its left file must be leg `live`",
            r.leg.leg, r.leg.row
        )),
        None => Ok(()),
    }
}

// ---- the diff ----------------------------------------------------------------------------------

/// **What names a cell in both reports**: its kind, row, dialect, protection and OS. The leg
/// and the timings are what is compared across.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Key {
    pub kind: EntryKind,
    pub row: String,
    pub dialect: Option<Dialect>,
    pub protection: Option<Protection>,
    pub os: Os,
}

impl Key {
    fn of(r: &LegRecord) -> Key {
        Key {
            kind: r.kind,
            row: r.row.clone(),
            dialect: r.dialect,
            protection: r.protection,
            os: r.os,
        }
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} {}", self.kind, self.row)?;
        if let Some(d) = self.dialect {
            write!(f, " {}", d.word())?;
        }
        if let Some(p) = self.protection {
            write!(f, " {p:?}")?;
        }
        write!(f, " {:?}", self.os)
    }
}

/// A cell the two reports judge differently.
#[derive(Debug, Clone, PartialEq)]
pub struct VerdictDiff {
    pub key: Key,
    pub live: ReportVerdict,
    pub suite: ReportVerdict,
    pub suite_leg: ReportLeg,
}

/// A cell whose event sequences differ, at the first event that does.
#[derive(Debug, Clone, PartialEq)]
pub struct EventsDiff {
    pub key: Key,
    pub at: usize,
    pub live: Option<String>,
    pub suite: Option<String>,
}

/// **How a live report differs from a suite report.**
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Diff {
    pub verdicts: Vec<VerdictDiff>,
    pub events: Vec<EventsDiff>,
    /// Cells the live run reports and the suite does not.
    pub only_live: Vec<Key>,
    /// Cells the suite reports and the live run does not.
    pub only_suite: Vec<Key>,
    /// Cells the live run did not run, with its reason, whatever the suite says: a cell that
    /// was not run shows nothing, and agreeing with another `not_run` is not agreement.
    pub live_not_run: Vec<(Key, Option<String>)>,
    /// Every cell the live run **failed** (a verdict `fail`, not a pinned `expected_fail`), with
    /// its reason, whatever the suite says: a live fail the suite also shows is two reds, not
    /// agreement, and an escape cell's fail means the refused action happened.
    pub live_fail: Vec<(Key, Option<String>, bool)>,
    /// How many pairs of records were compared.
    pub compared: usize,
    /// Of those, how many had events on both sides, and so an event comparison.
    pub events_compared: usize,
    /// Pairs where one side recorded events and the other did not.
    pub events_one_side: Vec<Key>,
}

impl Diff {
    /// Whether nothing differs. It says nothing of whether anything was compared: see
    /// [`Diff::agrees`].
    pub fn no_differences(&self) -> bool {
        self.verdicts.is_empty()
            && self.events.is_empty()
            && self.only_live.is_empty()
            && self.only_suite.is_empty()
            && self.live_not_run.is_empty()
            && self.live_fail.is_empty()
    }

    /// **Whether the two reports agree**: at least one pair was compared, and nothing differs.
    /// A diff of nothing against nothing agrees with no one.
    pub fn agrees(&self) -> bool {
        self.compared > 0 && self.no_differences()
    }

    /// One line per difference, then what was compared, for a person to read.
    pub fn render(&self) -> String {
        let mut out = String::new();
        for v in &self.verdicts {
            let _ = writeln!(
                out,
                "verdict  {}: live {:?}, suite ({:?}) {:?}",
                v.key, v.live, v.suite_leg, v.suite
            );
        }
        for e in &self.events {
            let show = |o: &Option<String>| o.clone().unwrap_or_else(|| "(none)".to_owned());
            let _ = writeln!(
                out,
                "events   {}: at {} live {}, suite {}",
                e.key,
                e.at,
                show(&e.live),
                show(&e.suite)
            );
        }
        for (k, why) in &self.live_not_run {
            let _ = writeln!(
                out,
                "not run  {k}: live did not run it ({})",
                why.as_deref().unwrap_or("no reason")
            );
        }
        for (k, why, refusal_expected) in &self.live_fail {
            let what = if *refusal_expected || k.kind == EntryKind::Escape {
                "ESCAPE: the refused action happened live"
            } else {
                "failed live"
            };
            let _ = writeln!(
                out,
                "FAIL     {k}: {what} ({}); a live fail never reads as agreement, whatever the suite says",
                why.as_deref().unwrap_or("no reason")
            );
        }
        for k in &self.only_live {
            let _ = writeln!(out, "only live   {k}");
        }
        for k in &self.only_suite {
            let _ = writeln!(out, "only suite  {k}");
        }
        for k in &self.events_one_side {
            let _ = writeln!(out, "note     {k}: only one side recorded events");
        }
        let _ = writeln!(
            out,
            "compared {} cells' verdicts; {} had an event comparison, {} had none",
            self.compared,
            self.events_compared,
            self.compared - self.events_compared
        );
        if self.compared == 0 {
            let _ = writeln!(out, "nothing was compared: that is not agreement");
        }
        out
    }
}

/// **`live` against `suite`, per cell.** A pair is compared when its kind, row, dialect,
/// protection and OS agree: verdicts always, event sequences when both sides recorded them. A
/// cell on one side alone is listed, never ignored, and so are every live `not_run` and every
/// live `fail`: a live fail blocks agreement even when the suite fails the same cell.
pub fn diff(live: &[Recorded], suite: &[Recorded]) -> Diff {
    let mut by_key: BTreeMap<Key, Vec<&Recorded>> = BTreeMap::new();
    for s in suite {
        by_key.entry(Key::of(&s.leg)).or_default().push(s);
    }
    let mut out = Diff::default();
    let mut seen = BTreeSet::new();
    for l in live {
        let key = Key::of(&l.leg);
        seen.insert(key.clone());
        if l.leg.verdict == ReportVerdict::NotRun {
            out.live_not_run.push((key.clone(), l.leg.reason.clone()));
        }
        if l.leg.verdict == ReportVerdict::Fail {
            out.live_fail
                .push((key.clone(), l.leg.reason.clone(), l.leg.refusal_expected));
        }
        let Some(against) = by_key.get(&key) else {
            out.only_live.push(key);
            continue;
        };
        for s in against {
            out.compared += 1;
            if l.leg.verdict != s.leg.verdict {
                out.verdicts.push(VerdictDiff {
                    key: key.clone(),
                    live: l.leg.verdict,
                    suite: s.leg.verdict,
                    suite_leg: s.leg.leg,
                });
            }
            match (&l.events, &s.events) {
                (Some(a), Some(b)) => {
                    out.events_compared += 1;
                    if let Some(at) = (0..a.len().max(b.len())).find(|&i| a.get(i) != b.get(i)) {
                        out.events.push(EventsDiff {
                            key: key.clone(),
                            at,
                            live: a.get(at).cloned(),
                            suite: b.get(at).cloned(),
                        });
                    }
                }
                (None, None) => {}
                _ => out.events_one_side.push(key.clone()),
            }
        }
    }
    out.only_suite = by_key.into_keys().filter(|k| !seen.contains(k)).collect();
    out
}
