// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What a runner of the cells format needs that does not touch a chat** (#7322): the one
//! loader, the closed word lists, the reading of a step's expectations against the events its
//! chat wrote, and the verdict that goes in the leg report.
//!
//! The scenario host (`app/src-tauri/src/scenarios/cells.rs`) starts the chats, presses what the
//! cell says the person presses and reads the window's queries. Everything it asks of the format
//! is decided here, so it runs in this crate's own tests, with no wrap and no chat:
//!
//! - [`load`]: every cells file in `cells/`, read, schema-checked and put through the
//!   [`Validator`]. A file that fails either is [`Malformed`], and a cell the runner cannot play
//!   is too: it is never skipped and never passed.
//! - [`Window`]: **the closed list of window expectations**. A `window` entry's `query` is one of
//!   its words and nothing else; the host maps each to a query the window reads
//!   (`scenarios/queries.rs`). A word not on the list makes the cell malformed.
//! - [`step_assertions`]: a step's `require` and `forbid` against its events, as
//!   [`judge::Assertion`]s with the observed value a mark pins (a count, or seconds for `within`).
//! - [`verdict`]: [`judge::judge`] under two rules the marks cannot bend (Q301): an escape cell
//!   is never `expected_fail`, and neither is a failed `outcome` of a step that expects a refusal,
//!   since that failure is the refused action having happened.
//!
//! ## What an expectation means
//!
//! A step's events are the log's events written after the step began, in `seq` order. For one
//! [`Expect`](crate::model::Expect):
//!
//! - it matches an event of its `kind` whose body holds every field it names. Fields are dotted
//!   (`target.host`); `null` asks for none; `$h1` is the cell's host; `"args": "this_call"` is the
//!   `args` digest of the step's first `hook.pretooluse`.
//! - `count` is exact (`1`), `≤N`, `≥N` or `N..M`; a `require` without one wants at least one,
//!   and a `forbid` wants none, whatever it says.
//! - `before: "post"` keeps the events written before the step's first `hook.posttooluse*`;
//!   `at: "next_prompt"` keeps the events written after the step's first `hook.stop`, the turn's
//!   end, which is when the next prompt is what comes next.
//! - `within` is a Timings expression: the event must come that long or less after the step's
//!   first `hook.pretooluse` (the ULIDs' own time), and a miss observes the seconds it took.
//!   `before`, `at` and `within` keep or drop events, so a count is of the events that remain.
//! - `{"either": [..]}` holds when any alternative does; `{"order": [a, b]}` when the first event
//!   of `a` is written before the last of `b`.
//!
//! Any other word in `before`, `at` or `args` is [`Malformed`].

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde_json::Value;

use crate::coverage::{Cover, Coverage};
use crate::judge::{Assertion, GlobalFailure, Observed, Run, Verdict, judge};
use crate::model::{
    Batch, Cell, EntryKind, Expect, Outcome, ReportVerdict, Require, Step, WindowQuery,
};
use crate::timings::Expr;
use crate::validate::{Validator, count_range};

/// **A cell or a file the runner cannot play**, and why. Never a pass, never a skip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Malformed(pub String);

impl std::fmt::Display for Malformed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// **The closed list of window expectations.** Each word is a fact about what the window shows
/// for the chat after a step, and the host reads it from the window's own query:
///
/// | Word | The query |
/// |---|---|
/// | `ask.listed` | `counted`: an ask of the chat is listed (the ✋) |
/// | `notice.standing` | `notices`: the chat is held on a block now |
/// | `blocked_log.lists` | `blocked_log`: it lists a block |
/// | `chat_log.block_row` | `blocks`: the chat's Chat log counts a block |
/// | `needs_you.queued` | `needs_you`: the chat is in the needs-you queue |
/// | `merge_row.listed` | `merge_row`: a merge ask waits |
/// | `settings.grant_listed` | `settings`: the Granted list holds a grant |
///
/// A `window` entry with `forbid: true` asks for the word not to hold.
///
/// **And the counted words** ([`Counted`]), written `word == N` as #7318's cells do: the number the
/// window shows for the chat, equal to `N`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Window {
    AskListed,
    NoticeStanding,
    BlockedLogLists,
    ChatLogBlockRow,
    NeedsYouQueued,
    MergeRowListed,
    SettingsGrantListed,
}

impl Window {
    pub const ALL: [Window; 7] = [
        Window::AskListed,
        Window::NoticeStanding,
        Window::BlockedLogLists,
        Window::ChatLogBlockRow,
        Window::NeedsYouQueued,
        Window::MergeRowListed,
        Window::SettingsGrantListed,
    ];

    pub fn word(self) -> &'static str {
        match self {
            Window::AskListed => "ask.listed",
            Window::NoticeStanding => "notice.standing",
            Window::BlockedLogLists => "blocked_log.lists",
            Window::ChatLogBlockRow => "chat_log.block_row",
            Window::NeedsYouQueued => "needs_you.queued",
            Window::MergeRowListed => "merge_row.listed",
            Window::SettingsGrantListed => "settings.grant_listed",
        }
    }

    pub fn parse(word: &str) -> Option<Window> {
        Window::ALL.into_iter().find(|w| w.word() == word)
    }
}

/// **The closed list of counted window expectations**, each a number the window shows for the
/// chat (`word == N`):
///
/// | Word | The query |
/// |---|---|
/// | `blocks.marker_blocked` | `blocks`: the marker's verified blocks |
/// | `blocks.marker_unverified` | `blocks`: the marker's unverified blocks |
/// | `finishes.rows_for_turn` | `finishes`: how many finish rows the chat has |
/// | `finishes.blocked` | `finishes`: the blocks its latest finish row counts |
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Counted {
    MarkerBlocked,
    MarkerUnverified,
    FinishRows,
    FinishBlocked,
}

impl Counted {
    pub const ALL: [Counted; 4] = [
        Counted::MarkerBlocked,
        Counted::MarkerUnverified,
        Counted::FinishRows,
        Counted::FinishBlocked,
    ];

    pub fn word(self) -> &'static str {
        match self {
            Counted::MarkerBlocked => "blocks.marker_blocked",
            Counted::MarkerUnverified => "blocks.marker_unverified",
            Counted::FinishRows => "finishes.rows_for_turn",
            Counted::FinishBlocked => "finishes.blocked",
        }
    }
}

/// **What a `window` entry asks**: a word that holds, or a number that equals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Query {
    Holds(Window),
    Equals(Counted, u64),
}

/// **A `window` entry, as the runner plays it**: a word of the list, and whether it must not hold.
pub fn window_of(query: &WindowQuery) -> Result<(Query, bool), Malformed> {
    if query.at.is_some() {
        return Err(Malformed(format!(
            "window `{}`: `at` is not played: a window entry is read when its step ends",
            query.query
        )));
    }
    let words = || {
        Window::ALL
            .map(Window::word)
            .into_iter()
            .chain(Counted::ALL.map(|c| c.word()))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let text = query.query.as_str();
    let parsed = match text.split_once("==") {
        Some((word, n)) => Counted::ALL
            .into_iter()
            .find(|c| c.word() == word.trim())
            .zip(n.trim().parse::<u64>().ok())
            .map(|(word, n)| Query::Equals(word, n)),
        None => Window::parse(text).map(Query::Holds),
    };
    parsed.map(|q| (q, query.forbid)).ok_or_else(|| {
        Malformed(format!(
            "window `{text}` is no word of the window list ({}; a counted word is `word == N`)",
            words()
        ))
    })
}

/// **Every batch in `dir`**, read, checked against the schema and put through the [`Validator`],
/// each with its file as a cover names it (`cells/B0.json`). A file that fails any is `Err` with
/// each problem, so a malformed file never reads as a batch with fewer cells.
pub fn load(dir: &Path, validator: &Validator) -> Result<Vec<(String, Batch)>, Malformed> {
    let mut paths: Vec<_> = std::fs::read_dir(dir)
        .map_err(|why| Malformed(format!("{}: {why}", dir.display())))?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|e| e == "json"))
        .collect();
    paths.sort();
    paths
        .iter()
        .map(|path| {
            let text = std::fs::read_to_string(path)
                .map_err(|why| Malformed(format!("{}: {why}", path.display())))?;
            let value: Value = serde_json::from_str(&text)
                .map_err(|why| Malformed(format!("{}: {why}", path.display())))?;
            let batch = Batch::from_value(value)
                .map_err(|why| Malformed(format!("{}: {why}", path.display())))?;
            match validator.check(&batch) {
                problems if problems.is_empty() => Ok((
                    format!(
                        "cells/{}",
                        path.file_name().unwrap_or_default().to_string_lossy()
                    ),
                    batch,
                )),
                problems => Err(Malformed(format!(
                    "{}: {}",
                    path.display(),
                    problems.join("; ")
                ))),
            }
        })
        .collect()
}

/// **The cells a `scenario` cover names**, as (cells file, cell id): the cells the fake leg owes
/// a verdict for, and so the cells the runner plays. A cell no cover names yet is data only, so a
/// finding in it (a real bug the cell is the first to see) is pinned in the pull request that
/// covers it, never in every other pull request's run.
pub fn covered(list: &Coverage) -> BTreeSet<(String, String)> {
    let named = list
        .rows
        .iter()
        .map(|r| r.cover.as_str())
        .chain(list.escapes.iter().flat_map(|e| {
            e.covers
                .values()
                .map(String::as_str)
                .chain([e.control.as_str()])
        }));
    named
        .filter_map(|text| match Cover::parse(text) {
            Ok(Cover::Scenario { file, cell }) => Some((file, cell)),
            _ => None,
        })
        .collect()
}

/// **One event of a step's log**, as an expectation reads it: its place in the log, the time its
/// ULID carries and what it said.
#[derive(Debug, Clone, PartialEq)]
pub struct Seen {
    pub seq: u64,
    pub at_ms: u64,
    pub kind: String,
    pub body: Value,
}

/// The cell's hosts as the run bound them: `h1` to the name its chats reach.
pub type Hosts = BTreeMap<String, String>;

/// **The words a step uses that nothing can read**: an unknown `before`, `at` or `args` word, a
/// host the cell does not bind, a `count` that is no count, a Timings expression with a name
/// the run lacks. All of them, so a cell says everything wrong with it at once.
pub fn malformed_step(
    step: &Step,
    hosts: &Hosts,
    seconds: &BTreeMap<&'static str, f64>,
) -> Vec<Malformed> {
    let mut found = Vec::new();
    let expects = step
        .require
        .iter()
        .flat_map(Require::events)
        .chain(step.forbid.iter());
    for e in expects {
        if let Some(before) = &e.before
            && before != "post"
        {
            found.push(Malformed(format!(
                "{}: `before: {before}` is not `post`",
                e.kind
            )));
        }
        if let Some(at) = &e.at
            && at != "next_prompt"
        {
            found.push(Malformed(format!(
                "{}: `at: {at}` is not `next_prompt`",
                e.kind
            )));
        }
        if let Some(count) = &e.count
            && count_range(count).is_none()
        {
            found.push(Malformed(format!(
                "{}: `count: {count}` is no count",
                e.kind
            )));
        }
        if let Some(within) = &e.within {
            match Expr::parse(within) {
                Ok(expr) if expr.seconds(seconds).is_some() => {}
                Ok(_) => found.push(Malformed(format!(
                    "{}: `within: {within}` names a Timings value this run lacks",
                    e.kind
                ))),
                Err(why) => found.push(Malformed(format!("{}: `within`: {why}", e.kind))),
            }
        }
        for (field, want) in &e.fields {
            match want {
                Value::String(s) if s.starts_with('$') && !hosts.contains_key(&s[1..]) => found
                    .push(Malformed(format!(
                        "{}: `{field}: {s}` names a host the cell does not bind",
                        e.kind
                    ))),
                Value::String(s) if field == "args" && s != "this_call" => found.push(Malformed(
                    format!("{}: `args: {s}` is not `this_call` or null", e.kind),
                )),
                _ => {}
            }
        }
    }
    for query in &step.window {
        if let Err(why) = window_of(query) {
            found.push(why);
        }
    }
    found
}

/// How a step's `require` and `forbid` came out against `events`, one [`Assertion`] each, named
/// `require[i]` and `forbid[i]`. `step_no` is the step's number from 1. `later` holds the events
/// written after the step's window, which are never counted for it but say how long a `within`
/// event took when it came late (a mark pins seconds, never "never"). The step's words must have
/// passed [`malformed_step`].
pub fn step_assertions(
    step_no: u32,
    step: &Step,
    events: &[Seen],
    later: &[Seen],
    hosts: &Hosts,
    seconds: &BTreeMap<&'static str, f64>,
) -> Vec<Assertion> {
    let frame = Frame::of(events, later);
    let one = |name: String, observed: Observed| Assertion {
        step: step_no,
        assertion: name,
        observed,
    };
    let mut out = Vec::new();
    for (i, require) in step.require.iter().enumerate() {
        let observed = match require {
            Require::Event(e) => frame.counted(e, hosts, seconds, true),
            Require::Either { either } => {
                let tries: Vec<Observed> = either
                    .iter()
                    .map(|e| frame.counted(e, hosts, seconds, true))
                    .collect();
                match tries.contains(&Observed::Pass) {
                    true => Observed::Pass,
                    false => tries.into_iter().next().unwrap_or(Observed::Fail(0.into())),
                }
            }
            Require::Order { order: [a, b] } => {
                let first = frame.matching(a, hosts, seconds).first().map(|s| s.seq);
                let last = frame.matching(b, hosts, seconds).last().map(|s| s.seq);
                match (first, last) {
                    (Some(a), Some(b)) if a < b => Observed::Pass,
                    _ => Observed::Fail(0.into()),
                }
            }
        };
        out.push(one(format!("require[{i}]"), observed));
    }
    for (i, e) in step.forbid.iter().enumerate() {
        let n = frame.matching(e, hosts, seconds).len();
        let observed = match n {
            0 => Observed::Pass,
            n => Observed::Fail(n.into()),
        };
        out.push(one(format!("forbid[{i}]"), observed));
    }
    out
}

/// A step's events with the marks `before` and `at` are read against.
struct Frame<'a> {
    events: &'a [Seen],
    later: &'a [Seen],
    first_post: Option<u64>,
    first_stop: Option<u64>,
    started_ms: Option<u64>,
    this_call: Option<&'a Value>,
}

impl<'a> Frame<'a> {
    fn of(events: &'a [Seen], later: &'a [Seen]) -> Self {
        let first = |pred: &dyn Fn(&Seen) -> bool| events.iter().find(|s| pred(s));
        let pre = first(&|s| s.kind == "hook.pretooluse");
        Frame {
            events,
            later,
            first_post: first(&|s| s.kind.starts_with("hook.posttooluse")).map(|s| s.seq),
            first_stop: first(&|s| s.kind == "hook.stop").map(|s| s.seq),
            started_ms: pre.map(|s| s.at_ms),
            this_call: pre
                .and_then(|s| s.body.get("args"))
                .filter(|a| !a.is_null()),
        }
    }

    /// The events `e` matches, after `before`, `at` and `within` have kept or dropped them.
    fn matching(
        &self,
        e: &Expect,
        hosts: &Hosts,
        seconds: &BTreeMap<&'static str, f64>,
    ) -> Vec<&Seen> {
        let limit = e
            .within
            .as_deref()
            .and_then(|w| Expr::parse(w).ok())
            .and_then(|x| x.seconds(seconds));
        self.events
            .iter()
            .filter(|s| s.kind == e.kind)
            .filter(|s| e.fields.iter().all(|(k, want)| self.has(s, k, want, hosts)))
            .filter(|s| match (&e.before, self.first_post) {
                (Some(_), Some(post)) => s.seq < post,
                _ => true,
            })
            .filter(|s| match (&e.at, self.first_stop) {
                (Some(_), Some(stop)) => s.seq > stop,
                (Some(_), None) => false,
                _ => true,
            })
            .filter(|s| match (limit, self.started_ms) {
                (Some(limit), Some(start)) => took(s, start) <= limit,
                _ => true,
            })
            .collect()
    }

    /// How `e` came out as a `require`: its count, or for a `within` miss the seconds the nearest
    /// event of its kind took.
    fn counted(
        &self,
        e: &Expect,
        hosts: &Hosts,
        seconds: &BTreeMap<&'static str, f64>,
        required: bool,
    ) -> Observed {
        let n = self.matching(e, hosts, seconds).len() as u64;
        let (lo, hi) = e
            .count
            .as_ref()
            .and_then(count_range)
            .unwrap_or((u64::from(required), u64::MAX));
        if (lo..=hi).contains(&n) {
            return Observed::Pass;
        }
        // Where only the time failed it, say how long the event took, even if it came after the
        // step: a mark pins seconds.
        if e.within.is_some()
            && n == 0
            && let (Some(start), Some(late)) = (self.started_ms, self.first_anywhere(e, hosts))
        {
            return Observed::Fail(serde_json::json!(took(late, start)));
        }
        Observed::Fail(n.into())
    }

    /// The first event of `e`'s kind that holds its fields, in the step or after it.
    fn first_anywhere(&self, e: &Expect, hosts: &Hosts) -> Option<&Seen> {
        self.events
            .iter()
            .chain(self.later)
            .find(|s| s.kind == e.kind && e.fields.iter().all(|(k, w)| self.has(s, k, w, hosts)))
    }

    fn has(&self, s: &Seen, field: &str, want: &Value, hosts: &Hosts) -> bool {
        let got = field
            .split('.')
            .try_fold(&s.body, |at, key| at.get(key))
            .filter(|v| !v.is_null());
        match want {
            Value::Null => got.is_none(),
            Value::String(w) if w == "this_call" && field == "args" => {
                self.this_call.is_some() && got == self.this_call
            }
            Value::String(w) if w.starts_with('$') => {
                let host = hosts.get(&w[1..]);
                got.and_then(Value::as_str).is_some_and(|g| {
                    host.is_some_and(|h| g == h || g.strip_suffix(":443") == Some(h))
                })
            }
            want => got == Some(want),
        }
    }
}

/// Seconds from `start_ms` to `s`.
fn took(s: &Seen, start_ms: u64) -> f64 {
    s.at_ms.saturating_sub(start_ms) as f64 / 1000.0
}

/// **The outcome a step's calls had**, as `outcome` names it, or why it is not what was asked.
pub fn outcome_assertion(step_no: u32, want: Outcome, seen: Outcome) -> Assertion {
    Assertion {
        step: step_no,
        assertion: "outcome".to_owned(),
        observed: if want == seen {
            Observed::Pass
        } else {
            Observed::Fail(Value::String(outcome_word(seen).to_owned()))
        },
    }
}

fn outcome_word(outcome: Outcome) -> &'static str {
    match outcome {
        Outcome::CameBack => "came_back",
        Outcome::Failed => "failed",
        Outcome::Denied => "denied",
        Outcome::NoPost => "no_post",
    }
}

/// **A cell's verdict** for the leg report: [`judge::judge`] under the marks, with Q301 held.
///
/// - An escape cell is never `expected_fail`. Its marks are refused by the validator, and a mark
///   that reached here anyway cannot turn a failure into a pin: the verdict is `Fail`.
/// - A failed `outcome` of a step with `refusal_expected` is never pinned either: the call the
///   cell expected refused came back, so the refused action happened. That is `Fail`, and the
///   record's `refusal_expected` makes the comparison of runs open the escape issue.
///
/// Answers the verdict and the reason a `Fail` goes in the record with.
pub fn verdict(
    cell: &Cell,
    run: &Run,
    assertions: &[Assertion],
    globals: &[GlobalFailure],
) -> (ReportVerdict, Option<String>) {
    let refused_action_happened: Vec<String> = assertions
        .iter()
        .filter(|a| a.assertion == "outcome" && matches!(a.observed, Observed::Fail(_)))
        .filter(|a| {
            cell.steps
                .get(a.step.wrapping_sub(1) as usize)
                .is_some_and(|s| s.refusal_expected)
        })
        .map(|a| {
            format!(
                "step {}: the refusal the cell expected did not happen",
                a.step
            )
        })
        .collect();
    let verdict = judge(cell, run, assertions, globals);
    match verdict {
        Verdict::Pass if refused_action_happened.is_empty() => (ReportVerdict::Pass, None),
        Verdict::ExpectedFail { .. } if cell.escape => (
            ReportVerdict::Fail,
            Some(format!(
                "{}: an escape cell cannot be expected_fail (Q301): the refused action happened",
                cell.id
            )),
        ),
        Verdict::ExpectedFail { .. } if refused_action_happened.is_empty() => {
            (ReportVerdict::ExpectedFail, None)
        }
        Verdict::Fail { reasons } => (
            ReportVerdict::Fail,
            Some(
                reasons
                    .into_iter()
                    .chain(refused_action_happened)
                    .collect::<Vec<_>>()
                    .join("; "),
            ),
        ),
        _ => (
            ReportVerdict::Fail,
            Some(format!(
                "{}: {} (Q301: reported as a failure, never expected_fail)",
                cell.id,
                refused_action_happened.join("; ")
            )),
        ),
    }
}

/// The entry kind a cell's record carries: an escape's when it is one.
pub fn entry_kind(cell: &Cell) -> EntryKind {
    if cell.escape {
        EntryKind::Escape
    } else {
        EntryKind::Cell
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Dialect, Leg, Os, TimingsMode};
    use serde_json::json;

    fn hosts() -> Hosts {
        [("h1".to_owned(), "one.test".to_owned())].into()
    }

    fn seconds() -> BTreeMap<&'static str, f64> {
        [("group", 2.0), ("hold", 10.0)].into()
    }

    fn seen(seq: u64, at_ms: u64, kind: &str, body: Value) -> Seen {
        Seen {
            seq,
            at_ms,
            kind: kind.to_owned(),
            body,
        }
    }

    fn step(value: Value) -> Step {
        let step: Step = serde_json::from_value(value).expect("a step");
        step
    }

    fn cell(value: Value) -> Cell {
        let mut batch = json!({
            "batch": "T", "protection": "full", "persona": "steward", "hosts": {"h1": "one.test"},
            "cells": [value],
        });
        batch["cells"][0]["dialects"] = json!(["claude"]);
        Batch::from_value(batch).expect("a batch").cells.remove(0)
    }

    fn run() -> Run {
        Run {
            dialect: Dialect::Claude,
            os: Os::Linux,
            leg: Leg::Fake,
            timings: TimingsMode::Scaled,
            seconds: seconds(),
        }
    }

    #[test]
    fn a_window_word_off_the_list_makes_the_cell_malformed() {
        for word in Window::ALL {
            let q = WindowQuery {
                query: word.word().to_owned(),
                forbid: true,
                at: None,
            };
            assert_eq!(window_of(&q), Ok((Query::Holds(word), true)));
        }
        for word in Counted::ALL {
            let q = WindowQuery {
                query: format!("{} == 2", word.word()),
                forbid: false,
                at: None,
            };
            assert_eq!(window_of(&q), Ok((Query::Equals(word, 2), false)));
        }
        for bad in [
            "blocks.marker_blocked",
            "blocks.marker_blocked == two",
            "ask.listed == 1",
            "nothing == 1",
        ] {
            let q = WindowQuery {
                query: bad.to_owned(),
                forbid: false,
                at: None,
            };
            assert!(window_of(&q).is_err(), "`{bad}` is no word");
        }
        let prose = WindowQuery {
            query: "blocks: marker +1".to_owned(),
            forbid: false,
            at: None,
        };
        let why = window_of(&prose).expect_err("prose is no word");
        assert!(why.0.contains("no word of the window list"), "{why}");
        let later = WindowQuery {
            query: "ask.listed".to_owned(),
            forbid: false,
            at: Some("next_prompt".to_owned()),
        };
        assert!(
            window_of(&later).is_err(),
            "`at` is not played, so it is not passed"
        );
    }

    #[test]
    fn a_step_with_an_unknown_word_is_malformed_in_every_place_at_once() {
        let s = step(json!({
            "calls": [{"tool": "Bash", "input": {"command": "true"}}],
            "require": [
                {"kind": "stop.block", "before": "later"},
                {"kind": "stop.block", "at": "tomorrow"},
                {"kind": "stop.block", "count": "lots"},
                {"kind": "stop.block", "within": "fortnight"},
                {"kind": "stop.block", "target.host": "$h9"},
                {"kind": "stop.block", "args": "that_call"},
            ],
            "window": [{"query": "the ask leaves"}],
        }));
        let found = malformed_step(&s, &hosts(), &seconds());
        assert_eq!(found.len(), 7, "{found:#?}");
        let fine = step(json!({
            "calls": [{"tool": "Bash", "input": {"command": "true"}}],
            "require": [{"kind": "stop.block", "before": "post", "at": "next_prompt",
                         "count": "1..2", "within": "group+1", "target.host": "$h1",
                         "args": "this_call"}],
            "window": [{"query": "ask.listed"}],
        }));
        assert_eq!(malformed_step(&fine, &hosts(), &seconds()), vec![]);
    }

    #[test]
    fn a_file_that_is_malformed_is_an_error_and_never_a_batch_with_fewer_cells() {
        let dir = tempfile::tempdir().expect("a folder");
        let validator = Validator::repo().expect("pins");
        std::fs::write(dir.path().join("A.json"), "{ not json").expect("a file");
        let why = load(dir.path(), &validator).expect_err("not JSON");
        assert!(why.0.contains("A.json"), "{why}");
        std::fs::write(
            dir.path().join("A.json"),
            json!({"batch": "A", "protection": "full", "persona": "steward", "hosts": {},
                   "cells": [{"id": "M-X", "steps": [{"calls": [{"tool": "Bash", "input": {}}],
                              "require": [{"kind": "no.such.kind"}]}]}]})
            .to_string(),
        )
        .expect("a file");
        let why = load(dir.path(), &validator).expect_err("an unknown kind");
        assert!(why.0.contains("no.such.kind"), "{why}");
        if let Err(why) = load(&crate::root().join("cells"), &validator) {
            panic!("the repo's cells do not load: {why}");
        }
    }

    #[test]
    fn require_counts_matches_with_fields_hosts_and_the_posts_that_bound_them() {
        let s = step(json!({
            "require": [
                {"kind": "ask.raised", "ask": "sandbox", "count": 1, "before": "post"},
                {"kind": "stop.block", "target.host": "$h1", "qualifier": null},
            ],
            "forbid": [{"kind": "ask.answered"}],
        }));
        let log = [
            seen(1, 1000, "hook.pretooluse", json!({"args": "d1"})),
            seen(2, 1100, "ask.raised", json!({"ask": "sandbox"})),
            seen(
                3,
                1200,
                "stop.block",
                json!({"target": {"host": "one.test"}}),
            ),
            seen(4, 1300, "hook.posttoolusefailure", json!({})),
            seen(5, 1400, "ask.raised", json!({"ask": "sandbox"})),
        ];
        let got = step_assertions(1, &s, &log, &[], &hosts(), &seconds());
        assert!(got.iter().all(|a| a.observed == Observed::Pass), "{got:#?}");
        // Without `before`, the late duplicate would be a second ask; with it, one.
        let twice = step(json!({"require": [{"kind": "ask.raised", "count": 1}]}));
        let got = step_assertions(1, &twice, &log, &[], &hosts(), &seconds());
        assert_eq!(
            got[0].observed,
            Observed::Fail(2.into()),
            "the count is what a mark pins"
        );
        // A forbid that matched says how many.
        let mut loud = log.to_vec();
        loud.push(seen(6, 1500, "ask.answered", json!({})));
        let got = step_assertions(1, &s, &loud, &[], &hosts(), &seconds());
        assert_eq!(
            got[2],
            Assertion {
                step: 1,
                assertion: "forbid[0]".into(),
                observed: Observed::Fail(1.into())
            }
        );
    }

    #[test]
    fn next_prompt_this_call_and_within_read_the_steps_own_marks() {
        let s = step(json!({
            "require": [
                {"kind": "ask.raised", "within": "group+1"},
                {"kind": "stop.block", "args": "this_call", "count": 1},
            ],
            "forbid": [{"kind": "stop.block", "at": "next_prompt"}],
        }));
        let log = [
            seen(1, 10_000, "hook.pretooluse", json!({"args": "d1"})),
            seen(2, 45_000, "ask.raised", json!({})),
            seen(3, 45_500, "stop.block", json!({"args": "d1"})),
            seen(4, 46_000, "hook.stop", json!({})),
            seen(5, 76_000, "stop.block", json!({"args": "d2"})),
        ];
        let got = step_assertions(1, &s, &log, &[], &hosts(), &seconds());
        // 35 s after the call, not within 2 * 1 + 1 = 3 s: the seconds are the observation.
        assert_eq!(got[0].observed, Observed::Fail(json!(35.0)));
        assert_eq!(
            got[1].observed,
            Observed::Pass,
            "the late block carries another call's args"
        );
        assert_eq!(
            got[2].observed,
            Observed::Fail(1.into()),
            "written after the turn's end"
        );
    }

    #[test]
    fn either_and_order_hold_on_what_they_name() {
        let s = step(json!({"require": [
            {"either": [{"kind": "stop.block"}, {"kind": "stop.hold"}]},
            {"order": [{"kind": "ask.raised"}, {"kind": "stop.block"}]},
        ]}));
        let log = [
            seen(1, 0, "ask.raised", json!({})),
            seen(2, 1, "stop.hold", json!({})),
            seen(3, 2, "stop.block", json!({})),
        ];
        assert!(
            step_assertions(1, &s, &log, &[], &hosts(), &seconds())
                .iter()
                .all(|a| a.observed == Observed::Pass)
        );
        let backwards = [
            seen(1, 0, "stop.block", json!({})),
            seen(2, 1, "ask.raised", json!({})),
        ];
        let got = step_assertions(1, &s, &backwards, &[], &hosts(), &seconds());
        assert_eq!(got[1].observed, Observed::Fail(0.into()));
    }

    #[test]
    fn only_the_cells_a_scenario_cover_names_are_covered() {
        let list = Coverage::from_value(json!({
            "rows": [
                {"row": "M-A", "dialect": "claude", "os": "linux", "log": true, "window": false,
                 "chat": false, "carried": "scenario", "kind": "scenario",
                 "cover": "scenario: cells/B0.json#M-A"},
                {"row": "M-B", "dialect": "claude", "os": "linux", "log": true, "window": false,
                 "chat": false, "carried": "scenario", "kind": "scenario", "cover": "pending: 11"},
                {"row": "M-C", "dialect": "claude", "os": "linux", "log": true, "window": false,
                 "chat": false, "carried": "e2e", "kind": "e2e",
                 "cover": "e2e: app/e2e/specs/x.e2e.ts#a title"},
            ],
            "escapes": [{"id": "E-1", "control": "scenario: cells/B9.json#E-1-CTL",
                         "covers": {"full": "scenario: cells/B9.json#E-1-FULL", "basic": "pending: 11"}}],
        }))
        .expect("a list");
        let named: Vec<(&str, &str)> = [
            ("cells/B0.json", "M-A"),
            ("cells/B9.json", "E-1-CTL"),
            ("cells/B9.json", "E-1-FULL"),
        ]
        .into();
        let got = covered(&list);
        assert_eq!(got.len(), 3, "{got:?}");
        for (file, cell) in named {
            assert!(
                got.contains(&(file.to_owned(), cell.to_owned())),
                "{file}#{cell}"
            );
        }
        assert!(
            !covered(&Coverage::default()).iter().any(|_| true),
            "an empty list covers nothing"
        );
    }

    #[test]
    fn a_within_event_written_after_the_step_says_how_long_it_took() {
        let s = step(json!({"require": [{"kind": "ask.raised", "within": "group+1"}]}));
        let log = [seen(1, 10_000, "hook.pretooluse", json!({"args": "d1"}))];
        // The ask came with the next prompt, 61 s after the call: not in this step, but its
        // seconds are what a mark pins.
        let later = [seen(9, 71_000, "ask.raised", json!({}))];
        let got = step_assertions(1, &s, &log, &later, &hosts(), &seconds());
        assert_eq!(got[0].observed, Observed::Fail(json!(61.0)));
        // Never written at all: a count of none, which no range can pin.
        let got = step_assertions(1, &s, &log, &[], &hosts(), &seconds());
        assert_eq!(got[0].observed, Observed::Fail(0.into()));
    }

    fn escape_cell(marked: bool) -> Cell {
        let mut c = json!({"id": "E-1", "escape": true,
            "steps": [{"calls": [{"tool": "Bash", "input": {"command": "true"}}],
                       "outcome": "failed", "refusal_expected": true}]});
        if marked {
            c["expected_fail"] =
                json!([{"issue": 1, "step": 1, "assertion": "outcome", "observed": "came_back"}]);
        }
        cell(c)
    }

    #[test]
    fn an_escape_is_reported_as_an_escape_and_never_as_expected_fail() {
        let came_back = [outcome_assertion(1, Outcome::Failed, Outcome::CameBack)];
        // Unmarked: a failure of an escape cell, whose record is an escape's.
        let c = escape_cell(false);
        assert_eq!(entry_kind(&c), EntryKind::Escape);
        let (v, why) = verdict(&c, &run(), &came_back, &[]);
        assert_eq!(v, ReportVerdict::Fail);
        assert!(why.expect("a reason").contains("did not happen"));
        // A mark that pins exactly this failure still cannot make it expected_fail.
        let c = escape_cell(true);
        let (v, why) = verdict(&c, &run(), &came_back, &[]);
        assert_eq!(v, ReportVerdict::Fail, "{why:?}");
        assert!(why.expect("a reason").contains("Q301"));
        // Held: the same cell passes.
        let held = [outcome_assertion(1, Outcome::Failed, Outcome::Failed)];
        assert_eq!(
            verdict(&escape_cell(false), &run(), &held, &[]).0,
            ReportVerdict::Pass
        );
    }

    #[test]
    fn a_refusal_that_did_not_happen_is_never_pinned_by_a_mark() {
        let mut c = json!({"id": "M-REF",
            "steps": [{"calls": [{"tool": "Bash", "input": {"command": "true"}}],
                       "outcome": "failed", "refusal_expected": true}],
            "expected_fail": [{"issue": 1, "step": 1, "assertion": "outcome", "observed": "came_back"}]});
        let marked = cell(c.clone());
        let came_back = [outcome_assertion(1, Outcome::Failed, Outcome::CameBack)];
        let (v, _) = verdict(&marked, &run(), &came_back, &[]);
        assert_eq!(v, ReportVerdict::Fail);
        // The same mark on a step that expects no refusal pins as usual.
        c["steps"][0]["refusal_expected"] = json!(false);
        let plain = cell(c);
        assert_eq!(
            verdict(&plain, &run(), &came_back, &[]).0,
            ReportVerdict::ExpectedFail
        );
    }

    #[test]
    fn a_pinned_failure_is_expected_fail_and_an_unpinned_one_is_red() {
        let c = cell(json!({"id": "M-P",
            "steps": [{"calls": [{"tool": "Bash", "input": {"command": "true"}}],
                       "require": [{"kind": "ask.raised", "count": 1}]}],
            "expected_fail": [{"issue": 6674, "step": 1, "assertion": "require[0]", "observed": 2}]}));
        let at = |n: u64| {
            [Assertion {
                step: 1,
                assertion: "require[0]".into(),
                observed: Observed::Fail(n.into()),
            }]
        };
        assert_eq!(
            verdict(&c, &run(), &at(2), &[]).0,
            ReportVerdict::ExpectedFail
        );
        assert_eq!(verdict(&c, &run(), &at(3), &[]).0, ReportVerdict::Fail);
        let pass = [Assertion {
            step: 1,
            assertion: "require[0]".into(),
            observed: Observed::Pass,
        }];
        let (v, why) = verdict(&c, &run(), &pass, &[]);
        assert_eq!(
            v,
            ReportVerdict::Fail,
            "a pass under a mark is red: {why:?}"
        );
    }
}
