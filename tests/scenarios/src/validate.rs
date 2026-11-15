// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What the schema cannot say.** A batch that passed `schema/cells.schema.json` is checked
//! here for:
//!
//! - event kinds purlis-core actually writes ([`Kinds`]);
//! - a dialect that lacks an event a cell names: the cell (or the dialect's pins file) must
//!   say what that dialect shows instead;
//! - a `forbid` step that does not also `require` an event proving its call ran;
//! - Timings expressions naming real durations, and `production_only` naming a declared pair;
//! - `expected_fail` marks naming an assertion the step has, once each, and a global check by
//!   its name;
//! - the row table: no cell without any expectation, and none missing a kind (Log, Window,
//!   Chat) its row states for a dialect and OS.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use serde::Deserialize;
use serde_json::Value;

use crate::kinds::Kinds;
use crate::model::{
    Batch, Cell, Dialect, Expect, Mark, Os, Override, PinsFile, Press, Require, Restart, Step,
    StepPatch,
};
use crate::timings::{Expr, TimingsCatalogue};

/// The global checks a mark may name (matrix-check's, ticket 7 runs them in the suite): the
/// runner leaving the sandbox, a refused call with no block or hold, an ask never answered or
/// withdrawn, and a block with no source.
pub const GLOBAL_CHECKS: [&str; 4] = [
    "sandbox_left",
    "refusal_black_hole",
    "ask_black_hole",
    "block_without_source",
];

const OSES: [Os; 2] = [Os::Linux, Os::Macos];

/// **One row × dialect × OS of the row table** (ticket 43 through ticket 8): whether the
/// matrix states a Log, a Window and a Chat expectation there.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RowEntry {
    pub row: String,
    pub dialect: Dialect,
    pub os: Os,
    pub log: bool,
    pub window: bool,
    pub chat: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(transparent)]
pub struct RowTable {
    pub entries: Vec<RowEntry>,
}

impl RowTable {
    fn entry(&self, row: &str, dialect: Dialect, os: Os) -> Option<&RowEntry> {
        self.entries
            .iter()
            .find(|e| e.row == row && e.dialect == dialect && e.os == os)
    }
}

/// **The pins files' defaults** (ticket 3): what each dialect shows for an event it lacks.
#[derive(Debug, Clone, Default)]
pub struct Pins {
    pub shows: BTreeMap<Dialect, BTreeMap<String, Vec<Expect>>>,
}

impl Pins {
    pub fn from_files(files: Vec<PinsFile>) -> Self {
        let mut shows: BTreeMap<Dialect, BTreeMap<String, Vec<Expect>>> = BTreeMap::new();
        for file in files {
            shows.entry(file.dialect).or_default().extend(file.shows);
        }
        Pins { shows }
    }

    /// Every `*.json` in `dir`; none when `dir` does not exist yet.
    pub fn read(dir: &Path) -> Result<Self, String> {
        if !dir.is_dir() {
            return Ok(Pins::default());
        }
        let mut files = Vec::new();
        let entries = fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        for entry in entries {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.extension().is_none_or(|x| x != "json") {
                continue;
            }
            let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            files
                .push(serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?);
        }
        Ok(Pins::from_files(files))
    }
}

/// **The validator**, with what it checks against.
pub struct Validator {
    pub kinds: Kinds,
    pub catalogue: Box<dyn TimingsCatalogue>,
    pub pins: Pins,
    pub rows: RowTable,
}

impl Validator {
    /// purlis-core's kinds, the Timings catalogue, the repo's pins files and no row table.
    pub fn repo() -> Result<Self, String> {
        Ok(Validator {
            kinds: Kinds::from_core(),
            catalogue: crate::timings::catalogue(),
            pins: Pins::read(&crate::root().join("pins"))?,
            rows: RowTable::default(),
        })
    }

    pub fn with_rows(mut self, rows: RowTable) -> Self {
        self.rows = rows;
        self
    }

    pub fn with_pins(mut self, pins: Pins) -> Self {
        self.pins = pins;
        self
    }

    /// Every problem in `batch`, each naming the batch and the cell; none for a good batch.
    pub fn check(&self, batch: &Batch) -> Vec<String> {
        let mut found = BTreeSet::new();
        let mut earlier: BTreeMap<&str, usize> = BTreeMap::new();
        for cell in &batch.cells {
            let mut say = |msg: String| {
                found.insert(format!("{} {}: {msg}", batch.batch, cell.id));
            };
            if earlier.contains_key(cell.id.as_str()) {
                say("the id is used twice in the batch".into());
            }
            self.check_cell(batch, cell, &earlier, &mut say);
            earlier.insert(&cell.id, cell.steps.len());
        }
        found.into_iter().collect()
    }

    fn check_cell(
        &self,
        batch: &Batch,
        cell: &Cell,
        earlier: &BTreeMap<&str, usize>,
        say: &mut dyn FnMut(String),
    ) {
        for (dialect, over) in &cell.per_dialect {
            if !cell.dialects.contains(dialect) {
                say(format!(
                    "per_dialect.{} is for a dialect the cell does not run in",
                    dialect.word()
                ));
            }
            self.check_override(
                batch,
                cell,
                &format!("per_dialect.{}", dialect.word()),
                over,
                say,
            );
        }
        for (os, over) in &cell.per_os {
            if !over.shows.is_empty() {
                say(format!(
                    "per_os.{os:?} has shows: an OS does not change which events a dialect has"
                ));
            }
            self.check_override(batch, cell, &format!("per_os.{os:?}"), over, say);
        }
        for (n, step) in cell.steps.iter().enumerate() {
            let at = format!("step {}", n + 1);
            self.check_step(batch, cell, &at, step, earlier, say);
        }
        if let Some(pair) = &cell.production_only {
            let declared =
                self.catalogue.production_only_pairs().iter().any(|(a, b)| {
                    (pair[0] == *a && pair[1] == *b) || (pair[0] == *b && pair[1] == *a)
                });
            if !declared {
                say(format!(
                    "production_only names {} and {}, which Timings does not declare a production_only pair",
                    pair[0], pair[1]
                ));
            }
        }
        self.check_marks(cell, say);
        for &dialect in &cell.dialects {
            for os in OSES {
                let steps = resolve(cell, dialect, os, &self.pins);
                self.check_resolved(cell, dialect, os, &steps, say);
            }
        }
    }

    fn check_override(
        &self,
        batch: &Batch,
        cell: &Cell,
        at: &str,
        over: &Override,
        say: &mut dyn FnMut(String),
    ) {
        for (kind, instead) in &over.shows {
            self.check_kind(&format!("{at}.shows"), kind, say);
            for e in instead {
                self.check_event(batch, &format!("{at}.shows.{kind}"), e, say);
            }
        }
        for (n, patch) in &over.steps {
            let in_range = n
                .parse::<usize>()
                .is_ok_and(|n| (1..=cell.steps.len()).contains(&n));
            if !in_range {
                say(format!("{at}.steps.{n}: the cell has no step {n}"));
            }
            let at = format!("{at}.steps.{n}");
            for e in patch_events(patch) {
                self.check_event(batch, &at, e, say);
            }
        }
    }

    fn check_step(
        &self,
        batch: &Batch,
        cell: &Cell,
        at: &str,
        step: &Step,
        earlier: &BTreeMap<&str, usize>,
        say: &mut dyn FnMut(String),
    ) {
        if let Some(wait) = &step.wait {
            self.check_expr(&format!("{at} wait"), wait, say);
        }
        if let Some(Press::Button(button)) = &step.press
            && let Some(delay) = &button.delay
        {
            self.check_expr(&format!("{at} press delay"), delay, say);
        }
        for (i, r) in step.require.iter().enumerate() {
            for e in r.events() {
                self.check_event(batch, &format!("{at} require[{i}]"), e, say);
            }
        }
        for (i, e) in step.forbid.iter().enumerate() {
            self.check_event(batch, &format!("{at} forbid[{i}]"), e, say);
        }
        if let Some(r) = &step.repeat_of {
            let steps = if r.cell == cell.id {
                at.trim_start_matches("step ").parse::<usize>().unwrap_or(1) - 1
            } else {
                earlier.get(r.cell.as_str()).copied().unwrap_or(0)
            };
            if r.step as usize > steps || r.step == 0 {
                say(format!(
                    "{at} repeat_of names {} step {}, which is not an earlier step",
                    r.cell, r.step
                ));
            }
        }
    }

    fn check_event(&self, batch: &Batch, at: &str, e: &Expect, say: &mut dyn FnMut(String)) {
        self.check_kind(at, &e.kind, say);
        if let Some(within) = &e.within {
            self.check_expr(&format!("{at} within"), within, say);
        }
        if let Some(count) = &e.count
            && count_range(count).is_none()
        {
            say(format!(
                "{at}: count {count} is not a number, ≤N, ≥N or N..M"
            ));
        }
        for (field, value) in &e.fields {
            if let Some(host) = value.as_str().and_then(|v| v.strip_prefix('$'))
                && !batch.hosts.contains_key(host)
            {
                say(format!(
                    "{at}: {field} names ${host}, which the batch's hosts lack"
                ));
            }
            if field == "args" && value.is_string() && value != "this_call" {
                say(format!("{at}: args is null or \"this_call\""));
            }
        }
    }

    fn check_kind(&self, at: &str, kind: &str, say: &mut dyn FnMut(String)) {
        if !self.kinds.knows(kind) {
            say(format!(
                "{at}: '{kind}' is not an event kind purlis-core writes"
            ));
        }
    }

    fn check_expr(&self, at: &str, text: &str, say: &mut dyn FnMut(String)) {
        match Expr::parse(text) {
            Err(e) => say(format!("{at}: {e}")),
            Ok(expr) => {
                let known = self.catalogue.production_seconds();
                for name in expr.names() {
                    if !known.contains_key(name) {
                        say(format!("{at}: '{name}' in '{text}' is not a Timings name"));
                    }
                }
            }
        }
    }

    fn check_marks(&self, cell: &Cell, say: &mut dyn FnMut(String)) {
        for (i, mark) in cell.expected_fail.iter().enumerate() {
            let at = format!("expected_fail[{i}] (#{})", mark.issue);
            if cell.expected_fail[..i].contains(mark) {
                say(format!(
                    "{at}: the same mark twice; one entry per failing assertion and issue"
                ));
            }
            if mark.dialect.is_some_and(|d| !cell.dialects.contains(&d)) {
                say(format!(
                    "{at}: scoped to a dialect the cell does not run in"
                ));
            }
            if let Some(check) = &mark.check {
                if !GLOBAL_CHECKS.contains(&check.as_str()) {
                    say(format!(
                        "{at}: '{check}' is not a global check ({})",
                        GLOBAL_CHECKS.join(", ")
                    ));
                }
                continue;
            }
            let (Some(n), Some(assertion)) = (mark.step, mark.assertion.as_deref()) else {
                continue;
            };
            if n == 0 || n as usize > cell.steps.len() {
                say(format!("{at}: the cell has no step {n}"));
                continue;
            }
            for &dialect in &cell.dialects {
                for os in OSES {
                    if !applies_to(mark, dialect, os) {
                        continue;
                    }
                    let steps = resolve(cell, dialect, os, &self.pins);
                    let step = &steps[n as usize - 1];
                    match assertion_in(step, assertion) {
                        None => say(format!(
                            "{at}: step {n} has no assertion {assertion} for {} on {os:?}",
                            dialect.word()
                        )),
                        Some(within) => self.check_observed(&at, mark, within, say),
                    }
                }
            }
        }
    }

    fn check_observed(
        &self,
        at: &str,
        mark: &Mark,
        within: Option<&str>,
        say: &mut dyn FnMut(String),
    ) {
        let range = mark.observed.as_ref().and_then(Value::as_object);
        match (within, range) {
            (Some(_), None) => say(format!(
                "{at}: a within assertion's observed value is a range {{from, to}} in Timings units"
            )),
            (None, Some(_)) => say(format!(
                "{at}: only a within assertion's observed value is a range"
            )),
            (Some(_), Some(r)) => {
                for end in ["from", "to"] {
                    if let Some(text) = r.get(end).and_then(Value::as_str) {
                        self.check_expr(&format!("{at} observed.{end}"), text, say);
                    }
                }
            }
            (None, None) => {}
        }
    }

    fn check_resolved(
        &self,
        cell: &Cell,
        dialect: Dialect,
        os: Os,
        steps: &[Step],
        say: &mut dyn FnMut(String),
    ) {
        let d = dialect.word();
        for (n, step) in steps.iter().enumerate() {
            let at = format!("step {}", n + 1);
            for e in step_events(step) {
                if self.kinds.knows(&e.kind) && !self.kinds.produced_by(&e.kind, dialect) {
                    say(format!(
                        "{at} names '{}', which {d} does not produce: say what {d} shows in per_dialect.{d} (shows or steps) or in its pins file",
                        e.kind
                    ));
                }
            }
            if !step.forbid.is_empty() {
                let proven = step.require.iter().any(|r| match r {
                    Require::Event(e) => self.kinds.proves_a_call_ran(&e.kind),
                    Require::Order { order } => {
                        order.iter().any(|e| self.kinds.proves_a_call_ran(&e.kind))
                    }
                    Require::Either { either } => {
                        either.iter().all(|e| self.kinds.proves_a_call_ran(&e.kind))
                    }
                });
                if !proven {
                    say(format!(
                        "{at} for {d} forbids events but requires none that proves its call ran (a tool hook event, stop.block, stop.hold or ask.raised)"
                    ));
                }
            }
        }
        let log = steps
            .iter()
            .any(|s| !s.require.is_empty() || !s.forbid.is_empty());
        let window = steps.iter().any(|s| !s.window.is_empty());
        let chat = steps.iter().any(|s| !s.transcript.is_empty());
        if !(log || window || chat) {
            say(format!("no expectation at all for {d} on {os:?}"));
        }
        if let Some(row) = self.rows.entry(&cell.id, dialect, os) {
            for (stated, has, name) in [
                (row.log, log, "log"),
                (row.window, window, "window"),
                (row.chat, chat, "chat (transcript)"),
            ] {
                if stated && !has {
                    say(format!(
                        "its row states a {name} expectation for {d} on {os:?}, and the cell has none"
                    ));
                }
            }
        }
    }
}

/// Whether `mark`'s dialect and OS scope covers `dialect` on `os`.
pub fn applies_to(mark: &Mark, dialect: Dialect, os: Os) -> bool {
    mark.dialect.is_none_or(|d| d == dialect) && mark.os.is_none_or(|o| o == os)
}

/// `assertion` in `step`: `Some(within)` when the step has it, with the `within` expression
/// for an event assertion carrying one.
fn assertion_in<'a>(step: &'a Step, assertion: &str) -> Option<Option<&'a str>> {
    let indexed = |prefix: &str| {
        assertion
            .strip_prefix(prefix)
            .and_then(|r| r.strip_prefix('['))
            .and_then(|r| r.strip_suffix(']'))
            .and_then(|i| i.parse::<usize>().ok())
    };
    if let Some(i) = indexed("require") {
        return step.require.get(i).map(|r| match r {
            Require::Event(e) => e.within.as_deref(),
            _ => None,
        });
    }
    if let Some(i) = indexed("forbid") {
        return step.forbid.get(i).map(|e| e.within.as_deref());
    }
    if let Some(i) = indexed("window") {
        return step.window.get(i).map(|_| None);
    }
    if let Some(i) = indexed("transcript") {
        return step.transcript.get(i).map(|_| None);
    }
    let has = match assertion {
        "outcome" => step.outcome.is_some(),
        "calls" => !step.calls.is_empty(),
        "repeat_of" => step.repeat_of.is_some(),
        _ => false,
    };
    has.then_some(None)
}

/// `≤N`, `≥N`, `N..M` or an exact number, as an inclusive range.
pub fn count_range(count: &Value) -> Option<(u64, u64)> {
    if let Some(n) = count.as_u64() {
        return Some((n, n));
    }
    let text = count.as_str()?;
    if let Some(n) = text.strip_prefix('≤') {
        return Some((0, n.parse().ok()?));
    }
    if let Some(n) = text.strip_prefix('≥') {
        return Some((n.parse().ok()?, u64::MAX));
    }
    let (lo, hi) = text.split_once("..")?;
    let (lo, hi) = (lo.parse().ok()?, hi.parse().ok()?);
    (lo <= hi).then_some((lo, hi))
}

/// **A cell's steps as `dialect` on `os` runs them**: the dialect's step patches, then the
/// OS's, then what the dialect shows in place of each event it lacks (the cell's `shows` over
/// its pins file's).
pub fn resolve(cell: &Cell, dialect: Dialect, os: Os, pins: &Pins) -> Vec<Step> {
    let mut steps = cell.steps.clone();
    let patches = [
        cell.per_dialect.get(&dialect).map(|o| &o.steps),
        cell.per_os.get(&os).map(|o| &o.steps),
    ];
    for (n, patch) in patches.into_iter().flatten().flatten() {
        if let Some(step) = n
            .parse::<usize>()
            .ok()
            .and_then(|n| steps.get_mut(n.wrapping_sub(1)))
        {
            apply(step, patch);
        }
    }
    let mut shows = pins.shows.get(&dialect).cloned().unwrap_or_default();
    if let Some(over) = cell.per_dialect.get(&dialect) {
        shows.extend(over.shows.clone());
    }
    if shows.is_empty() {
        return steps;
    }
    for step in &mut steps {
        step.require = std::mem::take(&mut step.require)
            .into_iter()
            .flat_map(|r| substitute_require(r, &shows))
            .collect();
        step.forbid = std::mem::take(&mut step.forbid)
            .into_iter()
            .flat_map(|e| shows.get(&e.kind).cloned().unwrap_or_else(|| vec![e]))
            .collect();
    }
    steps
}

fn apply(step: &mut Step, patch: &StepPatch) {
    if let Some(r) = &patch.require {
        step.require = r.clone();
    }
    if let Some(f) = &patch.forbid {
        step.forbid = f.clone();
    }
    if let Some(w) = &patch.window {
        step.window = w.clone();
    }
    if let Some(t) = &patch.transcript {
        step.transcript = t.clone();
    }
}

fn substitute_require(r: Require, shows: &BTreeMap<String, Vec<Expect>>) -> Vec<Require> {
    match r {
        Require::Event(e) => match shows.get(&e.kind) {
            Some(instead) => instead.iter().cloned().map(Require::Event).collect(),
            None => vec![Require::Event(e)],
        },
        Require::Either { either } => {
            let either: Vec<Expect> = either
                .into_iter()
                .flat_map(|e| shows.get(&e.kind).cloned().unwrap_or_else(|| vec![e]))
                .collect();
            match either.len() {
                0 => vec![],
                1 => either.into_iter().map(Require::Event).collect(),
                _ => vec![Require::Either { either }],
            }
        }
        Require::Order { mut order } => {
            for e in &mut order {
                if let Some([one]) = shows.get(&e.kind).map(Vec::as_slice) {
                    *e = one.clone();
                }
            }
            vec![Require::Order { order }]
        }
    }
}

fn step_events(step: &Step) -> Vec<&Expect> {
    step.require
        .iter()
        .flat_map(Require::events)
        .chain(step.forbid.iter())
        .collect()
}

fn patch_events(patch: &StepPatch) -> Vec<&Expect> {
    patch
        .require
        .iter()
        .flatten()
        .flat_map(Require::events)
        .chain(patch.forbid.iter().flatten())
        .collect()
}

/// Whether any step of `cell`, in any dialect or OS, restarts with `restart`.
pub fn restarts(cell: &Cell, restart: Restart) -> bool {
    cell.steps.iter().any(|s| s.restart == Some(restart))
}

/// Every `within` expression `cell` carries, in its steps and its overrides.
pub fn withins(cell: &Cell) -> Vec<&str> {
    let overrides = cell.per_dialect.values().chain(cell.per_os.values());
    let from_overrides = overrides.flat_map(|o| {
        o.shows
            .values()
            .flatten()
            .chain(o.steps.values().flat_map(patch_events))
    });
    cell.steps
        .iter()
        .flat_map(step_events)
        .chain(from_overrides)
        .filter_map(|e| e.within.as_deref())
        .collect()
}
