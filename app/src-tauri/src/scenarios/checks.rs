// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The global checks** (T1.1-07, #6854): six checks every scenario is held to at its end, that
//! no scenario switches off. A check a scenario's cell knowingly fails is `expected_fail`ed, per
//! scenario and check, with its issue ([`Scenario::expected_fail`]): the failure is then said,
//! reported as an expected fail, and a mark whose check did not fail is itself a failure, so
//! closing the issue removes the mark.
//!
//! | Check | Fires when |
//! |---|---|
//! | [`Check::NoBlackHole`] | a refusal (one the scenario declared, or a hook's `deny`) has no ask, no block and no hold written for its chat (R-004) |
//! | [`Check::NoUnansweredAsk`] | the window still lists an ask at the end |
//! | [`Check::NoLeftoverGrant`] | Settings lists a grant nobody made, or a once-grant that outlived its use |
//! | [`Check::HookWithinBudget`] | a hook event's `hook_ms` is over `hold` + `patience` of the scenario's timings |
//! | [`Check::EveryBlockHasASource`] | a `stop.block` does not say who refused it |
//! | [`Check::OneAskOneRaise`] | a chat's `ask.raised` is written twice with no new call, prompt or answer between (Q262) |
//!
//! **"At the end" is quiesced** ([`quiesce`]): the log has had no event for the scenario's
//! `group` and `tally_tick` together, judged by the events' own times, never by a sleep. What
//! the Linux supervisor tells and a hook line in flight reach the log as an event, which
//! restarts that wait. **The limit:** quiet is read from the log alone, so a hook or a hold that
//! is still pending and has written nothing yet is not seen: a scenario that ends with one
//! pending must wait for it itself (`until_played`), or its check may be judged too soon.
//!
//! **The log is read, not assumed** ([`log_verdict`]): a log that cannot be read is an `Err`, and
//! an empty one once a chat has started is a failure, never a quiet scenario.
//!
//! The checks themselves are a pure function of what was observed ([`judge`]): each has a
//! planted-fault test.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use purlis_core::eventlog::{Event, stops};

use super::host::Scenario;
use super::queries::QuietView;
use crate::sandboxing::GrantLevel;
use crate::taskblocks::BlockShown;

/// One of the six checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Check {
    NoBlackHole,
    NoUnansweredAsk,
    NoLeftoverGrant,
    HookWithinBudget,
    EveryBlockHasASource,
    OneAskOneRaise,
}

impl Check {
    /// Every check: none is left out of a scenario.
    pub(crate) const ALL: [Check; 6] = [
        Check::NoBlackHole,
        Check::NoUnansweredAsk,
        Check::NoLeftoverGrant,
        Check::HookWithinBudget,
        Check::EveryBlockHasASource,
        Check::OneAskOneRaise,
    ];

    pub(crate) fn word(self) -> &'static str {
        match self {
            Check::NoBlackHole => "no black hole",
            Check::NoUnansweredAsk => "no unanswered ask",
            Check::NoLeftoverGrant => "no leftover grant",
            Check::HookWithinBudget => "no hook over budget",
            Check::EveryBlockHasASource => "every block has a source",
            Check::OneAskOneRaise => "one ask, one raise",
        }
    }
}

/// A refusal a scenario knows its chat was given, whose ask, block or hold must be written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Refusal {
    /// The chat, by the id the event log knows it by.
    pub chat: String,
    /// The step that was refused.
    pub step: String,
    /// What it was aimed at (a host or a folder), where the scenario says.
    pub target: Option<String>,
    /// The ask, block or hold must be an event after this one's number: the log's latest when
    /// the scenario declared the refusal before the call that is refused, 0 for any.
    pub after: u64,
}

/// **What a scenario observed at its end**: [`judge`]'s whole input.
#[derive(Debug, Clone)]
pub(crate) struct Observed<'a> {
    pub events: &'a [Event],
    pub refusals: &'a [Refusal],
    /// The asks the window lists at the end: "chat N `key` says".
    pub asks_left: &'a [String],
    /// Every ask the window was seen to list: (chat by the event log's id, the target it asked
    /// about, the log's latest event number when it was seen listed). A raised ask carries no
    /// target in the log, so this is what ties one to a refusal: an ask answers a refusal only
    /// if it was written after it and was listed, for that target, at or after being written.
    pub asked_for: &'a [(String, String, u64)],
    /// What Settings lists beyond what it listed before the scenario began: (kind, target, level).
    pub grants_listed: &'a [(String, String, String)],
    /// What the person granted in the scenario and has not revoked: (kind, target, level).
    pub grants_made: &'a [(String, String, String)],
    /// The most a hook may take.
    pub hook_budget: Duration,
}

/// **Where a failure is**, so a mark can excuse it and nothing else: the log's event number, the
/// refusal's label, or neither for a failure of the scenario as a whole (an ask left listed).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Position {
    pub seq: Option<u64>,
    pub refusal: Option<String>,
    /// The mark that did not fire, by its place in the ledger.
    pub mark: Option<usize>,
}

impl Position {
    fn seq(seq: u64) -> Self {
        Position {
            seq: Some(seq),
            ..Position::default()
        }
    }
}

/// One check's failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Violation {
    pub check: Check,
    pub what: String,
    pub at: Position,
}

fn kind_is_a_stop(kind: &str) -> bool {
    matches!(kind, stops::BLOCK | stops::HOLD | stops::ASK_RAISED)
}

/// The text a block's or hold's body says its target is: the one value inside `target`.
fn target_in(body: &serde_json::Value) -> Option<String> {
    body.get("target")?
        .as_object()?
        .values()
        .next()?
        .as_str()
        .map(str::to_owned)
}

/// **Every check, on what was observed.** Pure: the checks' planted-fault tests call it with
/// events written by hand.
pub(crate) fn judge(observed: &Observed<'_>) -> Vec<Violation> {
    let mut found = Vec::new();
    let mut fail = |check: Check, what: String, at: Position| {
        found.push(Violation { check, what, at });
    };

    // No black hole (R-004): every refusal has an ask, a block or a hold. The refusals are the
    // ones the scenario declared and every hook `deny`.
    let mut refusals: Vec<Refusal> = observed.refusals.to_vec();
    for event in observed.events {
        if event.kind.starts_with("hook.")
            && event
                .body
                .get("decision")
                .and_then(serde_json::Value::as_str)
                == Some("deny")
            && let Some(chat) = event.chat.clone()
        {
            refusals.push(Refusal {
                chat,
                step: format!("a hook's deny, event {}", event.seq),
                target: None,
                after: event.seq,
            });
        }
    }
    // Each stop written answers one refusal, never two: an ask raised for one target is no answer
    // to a refusal of another. A block or a hold answers the refusal of its target; an ask (which
    // names no target in the log) answers the refusal of a target the window listed an ask for.
    let mut used: BTreeSet<u64> = BTreeSet::new();
    for refusal in &refusals {
        let answer = observed.events.iter().find(|event| {
            kind_is_a_stop(&event.kind)
                && !used.contains(&event.seq)
                && event.seq > refusal.after
                && event.chat.as_deref() == Some(refusal.chat.as_str())
                && match (&refusal.target, event.kind.as_str()) {
                    (None, _) => true,
                    (Some(wanted), stops::ASK_RAISED) => {
                        observed.asked_for.iter().any(|(chat, target, seen)| {
                            chat == &refusal.chat && target == wanted && *seen >= event.seq
                        })
                    }
                    (Some(wanted), _) => target_in(&event.body).is_some_and(|one| &one == wanted),
                }
        });
        if let Some(answer) = answer {
            used.insert(answer.seq);
        } else {
            fail(
                Check::NoBlackHole,
                format!(
                    "chat {} was refused `{}`{}, and nothing was written: no ask, no block, no hold",
                    refusal.chat,
                    refusal.step,
                    refusal
                        .target
                        .as_ref()
                        .map(|target| format!(" (aimed at {target})"))
                        .unwrap_or_default()
                ),
                Position {
                    seq: refusal
                        .step
                        .strip_prefix("a hook's deny, event ")
                        .and_then(|n| n.parse().ok()),
                    refusal: Some(refusal.step.clone()),
                    mark: None,
                },
            );
        }
    }

    // No unanswered ask at the end.
    for ask in observed.asks_left {
        fail(
            Check::NoUnansweredAsk,
            format!("the window still lists {ask}"),
            Position::default(),
        );
    }

    // No leftover grant: Settings lists only what the person granted and kept.
    for (kind, target, level) in observed.grants_listed {
        let made = observed
            .grants_made
            .iter()
            .find(|(k, t, _)| k == kind && t == target);
        match made {
            None => fail(
                Check::NoLeftoverGrant,
                format!("Settings lists a {kind} grant for {target} ({level}) nobody made"),
                Position::default(),
            ),
            Some((_, _, made_at)) if made_at == "once" => fail(
                Check::NoLeftoverGrant,
                format!("a once-grant of {kind} {target} outlived its use: Settings lists it"),
                Position::default(),
            ),
            Some(_) => {}
        }
    }

    // No hook over budget. A `hook_ms` that is not a whole number measures nothing: it fails.
    let budget_ms = u64::try_from(observed.hook_budget.as_millis()).unwrap_or(u64::MAX);
    for event in observed.events {
        let Some(measured) = event.body.get("hook_ms") else {
            continue;
        };
        match measured.as_u64() {
            Some(took) if took > budget_ms => fail(
                Check::HookWithinBudget,
                format!(
                    "event {} ({}) took a hook {took} ms, over its {budget_ms} ms budget",
                    event.seq, event.kind
                ),
                Position::seq(event.seq),
            ),
            Some(_) => {}
            None => fail(
                Check::HookWithinBudget,
                format!(
                    "event {} ({}) says `hook_ms` {measured}, which is no whole number of \
                     milliseconds: the hook's time is not measured",
                    event.seq, event.kind
                ),
                Position::seq(event.seq),
            ),
        }
    }

    // Every block has a source.
    for event in observed.events.iter().filter(|e| e.kind == stops::BLOCK) {
        let source = event.body.get("source").and_then(serde_json::Value::as_str);
        let known = [
            "guard",
            "harness_check",
            "sandbox",
            "policy",
            "project_setting",
        ];
        if !source.is_some_and(|word| known.contains(&word)) {
            fail(
                Check::EveryBlockHasASource,
                format!(
                    "block event {} says no source (`{}`): who refused it is not written",
                    event.seq,
                    source.unwrap_or("none")
                ),
                Position::seq(event.seq),
            );
        }
    }

    // One ask, one raise (Q262): a chat's `ask.raised` is written once for an ask. Two of its raises
    // with no new call (`hook.pretooluse`), prompt or answer between them are one ask raised twice,
    // by two writers (#6674); a second call that is refused raises an ask of its own.
    let mut open: BTreeMap<&str, usize> = BTreeMap::new();
    for event in observed.events {
        let Some(chat) = event.chat.as_deref() else {
            continue;
        };
        let kind = event.kind.as_str();
        if kind == stops::ASK_RAISED {
            let raised = open.entry(chat).or_default();
            *raised += 1;
            if *raised == 2 {
                fail(
                    Check::OneAskOneRaise,
                    format!(
                        "chat {chat}: event {} is a second ask.raised with no new call, prompt or \
                         answer since the first",
                        event.seq
                    ),
                    Position::seq(event.seq),
                );
            }
        } else if kind.starts_with("hook.pretool")
            || kind == "hook.userpromptsubmit"
            || kind == stops::ASK_ANSWERED
            || kind == "run.started"
        {
            open.remove(chat);
        }
    }
    found
}

/// **Where a mark excuses its check**: never for the whole suite. A scenario's own mark holds for
/// the scenario (`Anywhere`); a cell's holds for the events it wrote (`Events`: after the first
/// number, up to and including the second) or for one refusal it declared (`Refusal`, by label).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) enum Scope {
    #[default]
    Anywhere,
    Events {
        after: u64,
        upto: u64,
    },
    Refusal(String),
}

impl Scope {
    fn holds(&self, at: &Position) -> bool {
        match self {
            Scope::Anywhere => true,
            Scope::Events { after, upto } => at.seq.is_some_and(|seq| seq > *after && seq <= *upto),
            Scope::Refusal(label) => at.refusal.as_deref() == Some(label.as_str()),
        }
    }
}

/// A check a scenario knowingly fails, with the issue that says why.
#[derive(Debug, Clone)]
pub(crate) struct Mark {
    pub check: Check,
    pub issue: u32,
    pub why: String,
    pub scope: Scope,
}

/// **What came of a scenario's checks**: each failure that is not marked, and each marked check
/// (said, and reported as an expected fail) or mark that did not fire.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Outcome {
    pub failures: Vec<String>,
    /// Where each of `failures` is, in the same order.
    pub failure_at: Vec<Position>,
    pub expected: Vec<(Check, u32, String)>,
    /// The marks that excused a failure, by their place in the ledger.
    pub fired: std::collections::BTreeSet<usize>,
}

/// **Applies `marks` to what [`judge`] found**: a failure of a marked check is an expected fail;
/// any other is a failure; a mark whose check passed is an unexpected pass, a failure too.
pub(crate) fn outcome(found: Vec<Violation>, marks: &[Mark]) -> Outcome {
    let mut out = Outcome::default();
    for violation in &found {
        let mark = marks
            .iter()
            .enumerate()
            .find(|(_, mark)| mark.check == violation.check && mark.scope.holds(&violation.at));
        match mark {
            Some((i, mark)) => {
                out.fired.insert(i);
                out.expected
                    .push((violation.check, mark.issue, violation.what.clone()));
            }
            None => {
                out.failures
                    .push(format!("{}: {}", violation.check.word(), violation.what));
                out.failure_at.push(violation.at.clone());
            }
        }
    }
    for (i, mark) in marks.iter().enumerate() {
        if !out.fired.contains(&i) {
            out.failures.push(format!(
                "{}: unexpected pass: #{} ({}) is marked as failing here, and the check passed: \
                 remove the mark",
                mark.check.word(),
                mark.issue,
                mark.why
            ));
            out.failure_at.push(Position {
                mark: Some(i),
                ..Position::default()
            });
        }
    }
    out
}

/// What a scenario's person granted and has not revoked, for every scenario of this process:
/// (kind, target, level). Machine and You grants outlive a scenario in the homes its test
/// shares, so a later scenario may find an earlier one's in Settings.
static MADE: Mutex<Vec<(String, String, String)>> = Mutex::new(Vec::new());

fn grant_kind(what: impl std::fmt::Debug) -> String {
    format!("{what:?}")
}

fn level_word(level: GrantLevel) -> String {
    crate::asking::level_id(level).to_owned()
}

/// **Quiet, as the scripted person turns it on and off**: the notifier's own `QuietSources` and
/// `Rules`, so what Quiet's bucket held is told as the window is told it.
#[derive(Default)]
pub(crate) struct QuietState {
    inner: Mutex<(
        crate::asknotify::QuietSources,
        crate::asknotify::Rules,
        Option<crate::asknotify::QuietEnded>,
    )>,
}

impl QuietState {
    /// "Quiet now" on or off: Quiet starts or ends as the notifier finds it before each read.
    pub(crate) fn set(&self, on: bool) {
        let mut inner = self.inner.lock().expect("quiet");
        inner.0.set_now(on);
        let quiet = inner.0.is_quiet(false, Instant::now());
        let at = u32::try_from(crate::sandboxing::now_secs()).unwrap_or(u32::MAX);
        let (_, ended) = inner.1.quiet(quiet, at, false, &|_| false);
        if ended.is_some() {
            inner.2 = ended;
        }
    }

    /// Whether Quiet holds now and what it held when it last ended.
    pub(crate) fn view(&self) -> QuietView {
        let inner = self.inner.lock().expect("quiet");
        QuietView {
            on: inner.0.is_quiet(false, Instant::now()),
            ended: inner.2.clone(),
        }
    }

    /// The asks the window lists now come in: while Quiet holds, they are its bucket's.
    pub(crate) fn hears(&self, plane: &crate::planes::PlaneId, asks: &[crate::asking::Shown]) {
        let mut inner = self.inner.lock().expect("quiet");
        let quiet = inner.0.is_quiet(false, Instant::now());
        inner.1.heard(&crate::asknotify::Heard {
            plane,
            asks,
            looking: &|_| false,
            quiet: &|| quiet,
            now: Instant::now(),
        });
    }
}

/// **What a scenario's person, queries and checks keep**: the commands reached, the asks seen,
/// the refusals declared, the marks, and Quiet.
#[derive(Default)]
pub(crate) struct Ledger {
    pub reached: super::person::Reached,
    pub quiet: QuietState,
    refusals: Mutex<Vec<Refusal>>,
    marks: Mutex<Vec<Mark>>,
    /// What Settings listed when the scenario began: the project's own declarations.
    baseline: Mutex<Vec<(String, String)>>,
    /// Every ask the window was seen to list: (chat, the target it asks about, the log's latest
    /// event number when it was seen).
    listed: Mutex<BTreeSet<(u32, String, u64)>>,
    /// Every ask the window drew a target of that a person could be misled by.
    drawn_wrong: Mutex<BTreeSet<String>>,
    /// Every block a chat was seen held on (chat, target), as Notices raise them, kept after the
    /// block is answered and so retracted.
    standing: Mutex<BTreeSet<(u32, String)>>,
}

impl Ledger {
    /// The window listed chat `session`'s ask about `target`, and drew it as `fault` says, if it
    /// drew it wrong.
    pub(crate) fn listed(&self, session: u32, target: &str, seen_at: u64, fault: Option<String>) {
        if !target.is_empty() {
            self.listed
                .lock()
                .expect("listed")
                .insert((session, target.to_owned(), seen_at));
        }
        if let Some(fault) = fault {
            self.drawn_wrong.lock().expect("drawn wrong").insert(fault);
        }
    }

    /// A chat was seen held on the block for `target`.
    pub(crate) fn stood(&self, session: u32, target: &str) {
        self.standing
            .lock()
            .expect("standing")
            .insert((session, target.to_owned()));
    }

    /// Every block a chat was seen held on: (chat, target).
    pub(crate) fn ever_standing(&self) -> Vec<(u32, String)> {
        self.standing
            .lock()
            .expect("standing")
            .iter()
            .cloned()
            .collect()
    }

    /// The person granted `shown` at `level`.
    pub(crate) fn granted(&self, shown: &BlockShown, level: GrantLevel) {
        let one = (
            grant_kind(shown.what),
            shown.target.clone(),
            level_word(level),
        );
        let mut made = MADE.lock().expect("made");
        made.retain(|(k, t, _)| !(k == &one.0 && t == &one.1));
        made.push(one);
    }

    /// The person revoked `grant` in Settings.
    pub(crate) fn revoked(&self, grant: &crate::sandboxing::SandboxGrant) {
        let (kind, target) = (grant_kind(grant.what), grant.target.clone());
        MADE.lock()
            .expect("made")
            .retain(|(k, t, _)| !(k == &kind && t == &target));
    }

    fn settings_now(scenario: &Scenario) -> Vec<(String, String, String)> {
        scenario
            .queries()
            .settings()
            .into_iter()
            .map(|one| (grant_kind(one.what), one.target, level_word(one.level)))
            .collect()
    }

    /// Settings as the scenario begins: what is listed then is the project's own.
    pub(crate) fn begins(&self, scenario: &Scenario) {
        *self.baseline.lock().expect("baseline") = Self::settings_now(scenario)
            .into_iter()
            .map(|(k, t, _)| (k, t))
            .collect();
    }
}

impl Scenario {
    /// **Declares a refusal** a scenario's chat was given at `step`, aimed at `target`: its
    /// ask, block or hold must be written ([`Check::NoBlackHole`]).
    pub(crate) fn refused(&self, session: u32, step: &str, target: Option<&str>) {
        self.declares(session, step, target, 0);
    }

    /// **[`Self::refused`], for a step that began after the log's event `since`**: only what is
    /// written after it answers the refusal, so an ask or a block an earlier step wrote for the
    /// same target never does (a repeat's refusal is held to its own answer).
    pub(crate) fn refused_since(&self, session: u32, step: &str, target: Option<&str>, since: u64) {
        self.declares(session, step, target, since);
    }

    /// **[`Self::refused`], for a refusal that has not happened yet**: what answers it must be
    /// written after the log's latest event now, so an ask an earlier call raised answers nothing.
    pub(crate) fn refused_after_now(&self, session: u32, step: &str, target: Option<&str>) {
        let after = self.events().last().map_or(0, |event| event.seq);
        self.declares(session, step, target, after);
    }

    fn declares(&self, session: u32, step: &str, target: Option<&str>, after: u64) {
        let chat = self
            .queries()
            .chat_id(session)
            .unwrap_or_else(|| panic!("the event log knows no chat by session {session}"));
        self.ledger
            .refusals
            .lock()
            .expect("refusals")
            .push(Refusal {
                chat,
                step: step.to_owned(),
                target: target.map(str::to_owned),
                after,
            });
    }

    /// **Marks `check` as failing here, for `issue`**: per scenario and check, never for the
    /// suite. A mark whose check passes is a failure at the end.
    pub(crate) fn expected_fail(&self, check: Check, issue: u32, why: &str) {
        self.expected_fail_in(check, issue, why, Scope::Anywhere);
    }

    /// **[`Self::expected_fail`], excusing only what `scope` holds** (a cell's step, a cell's
    /// refusal). Answers the mark's place in the ledger, which [`Outcome::fired`] names.
    pub(crate) fn expected_fail_in(
        &self,
        check: Check,
        issue: u32,
        why: &str,
        scope: Scope,
    ) -> usize {
        assert!(issue > 0, "an expected fail names its issue");
        let mut marks = self.ledger.marks.lock().expect("marks");
        marks.push(Mark {
            check,
            issue,
            why: why.to_owned(),
            scope,
        });
        marks.len() - 1
    }
}

/// **Until the scenario is quiet**: its log has had no event for `group` and `tally_tick`
/// together, by the events' own times. A condition polled, never a sleep that hopes.
pub(crate) fn quiesce(scenario: &Scenario) -> Result<(), String> {
    let settle = scenario.timings.group() + scenario.timings.tally_tick();
    let deadline = Instant::now() + super::PLAYS_WITHIN;
    loop {
        let events = scenario.try_events()?;
        // A chat has started and the log holds nothing yet: not quiet, and not judged.
        let written = !events.is_empty() || scenario.chats_started() == 0;
        let newest = events
            .iter()
            .filter_map(|event| purlis_core::eventlog::written_at_ms(&event.ulid))
            .max();
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| {
                u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
            });
        let quiet_for = newest.map_or(u64::MAX, |at| now_ms.saturating_sub(at));
        if written && u128::from(quiet_for) >= settle.as_millis() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            log_verdict(scenario.chats_started(), Ok(events))?;
            return Err(format!(
                "the log never went quiet: its newest event is {quiet_for} ms old, and quiet \
                 is {} ms",
                settle.as_millis()
            ));
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

/// **The log as the checks read it**: `read` is what the log answered, `chats` how many chats
/// the scenario started. A log that could not be read is an `Err`, and so is an empty one once a
/// chat has started: a chat writes its run to the log, so none is a log read from the wrong place.
pub(crate) fn log_verdict(
    chats: usize,
    read: Result<Vec<Event>, String>,
) -> Result<Vec<Event>, String> {
    let events = read.map_err(|why| format!("the event log could not be read: {why}"))?;
    if events.is_empty() && chats > 0 {
        return Err(format!(
            "the event log holds no event, though chats started (count {chats}): it is read from the \
             wrong place or was never written, and a check judged on it would pass on nothing"
        ));
    }
    Ok(events)
}

/// **The checks need the window**: they read the asks, Settings and Notices through it. A scenario
/// with no window at its end (its restart did not finish) is not checked, and so is not green.
pub(crate) fn window_for_the_checks(up: bool) -> Result<(), String> {
    if up {
        Ok(())
    } else {
        Err(
            "the scenario has no window at its end (a restart did not finish): its global \
             checks were not run, and a scenario that was not checked has not passed"
                .to_owned(),
        )
    }
}

/// **The scenario's checks at its end**: quiesced, observed through the window's own queries,
/// judged, and the marks applied. `Err` is a report for a red run.
pub(crate) fn at_the_end(scenario: &Scenario) -> Result<Outcome, String> {
    quiesce(scenario)?;
    let queries = scenario.queries();
    let asks = queries.counted();
    // The asks nobody answered: a chat's own wait for the person's reply is no ask raised. An ask
    // the window still lists is unanswered, whatever the person pressed on it.
    let asks_left: Vec<String> = asks
        .asks
        .iter()
        .filter(|ask| !crate::asknotify::own_wait(ask))
        .map(|ask| format!("chat {} `{}` ({:?})", ask.session, ask.ask, ask.says))
        .collect();
    let _ = queries.notices();
    scenario.ledger.quiet.hears(&scenario.plane, &asks.asks);
    let baseline = scenario.ledger.baseline.lock().expect("baseline").clone();
    let grants_listed: Vec<(String, String, String)> = Ledger::settings_now(scenario)
        .into_iter()
        .filter(|(k, t, _)| !baseline.iter().any(|(bk, bt)| bk == k && bt == t))
        .collect();
    let grants_made = MADE.lock().expect("made").clone();
    let events = log_verdict(scenario.chats_started(), scenario.try_events())?;
    let refusals = scenario.ledger.refusals.lock().expect("refusals").clone();
    let drawn_wrong: Vec<String> = scenario
        .ledger
        .drawn_wrong
        .lock()
        .expect("drawn wrong")
        .iter()
        .cloned()
        .collect();
    if !drawn_wrong.is_empty() {
        return Err(format!(
            "the window drew an ask's target other than what is decided:\n  {}",
            drawn_wrong.join("\n  ")
        ));
    }
    let asked_for: Vec<(String, String, u64)> = scenario
        .ledger
        .listed
        .lock()
        .expect("listed")
        .iter()
        .filter_map(|(session, target, seen)| {
            Some((queries.chat_id(*session)?, target.clone(), *seen))
        })
        .collect();
    let found = judge(&Observed {
        events: &events,
        refusals: &refusals,
        asks_left: &asks_left,
        asked_for: &asked_for,
        grants_listed: &grants_listed,
        grants_made: &grants_made,
        hook_budget: scenario.timings.hold() + scenario.timings.patience(),
    });
    let marks = scenario.ledger.marks.lock().expect("marks").clone();
    Ok(outcome(found, &marks))
}

/// **The report of a failing scenario**: the step that failed, what was expected and what was
/// seen, the events the log holds, what the window's queries show, and the transcript's last
/// lines.
pub(crate) fn failure_report(
    step: &str,
    expected: &str,
    seen: &str,
    events: &[Event],
    queries: &str,
    transcript: &[serde_json::Value],
) -> String {
    let events: Vec<String> = events
        .iter()
        .map(|event| {
            format!(
                "#{} {} chat {:?} {}",
                event.seq,
                event.kind,
                event.chat,
                event.body.to_string().chars().take(200).collect::<String>()
            )
        })
        .collect();
    let excerpt: Vec<String> = transcript
        .iter()
        .rev()
        .take(20)
        .rev()
        .map(|line| line.to_string().chars().take(300).collect())
        .collect();
    format!(
        "step: {step}\nexpected: {expected}\nseen: {seen}\nevents: {events:#?}\n\
         the window's queries:\n{queries}\ntranscript (last {}): {excerpt:#?}",
        excerpt.len()
    )
}

impl Scenario {
    /// **Fails the scenario at `step`** with [`failure_report`].
    pub(crate) fn fails_at(&self, step: &str, expected: &str, seen: &str) -> ! {
        let queries =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.queries().said()))
                .unwrap_or_else(|_| "the window's queries could not be read".to_owned());
        panic!(
            "{}",
            failure_report(
                step,
                expected,
                seen,
                &self.try_events().unwrap_or_default(),
                &queries,
                &self.transcript()
            )
        )
    }
}

/// What one of the canary's steps came to, as the chat's transcript records its tool result.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct StepResult {
    /// The command ran: its result has an exit status. One a hook denied, or that never ran,
    /// has none.
    pub ran: bool,
    pub exit: Option<i64>,
    /// What it said, its stderr and the result's own text.
    pub said: String,
    /// The hook that refused it, where one did.
    pub hook: Option<String>,
}

impl StepResult {
    /// The step as a `tool_result` line of the transcript records it; none for a step with no
    /// result.
    pub(crate) fn of(line: Option<&serde_json::Value>) -> StepResult {
        let Some(line) = line else {
            return StepResult::default();
        };
        let text = |key: &str| {
            line.get(key)
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
        };
        StepResult {
            ran: line.get("exit").is_some(),
            exit: line.get("exit").and_then(serde_json::Value::as_i64),
            said: format!("{} {}", text("stderr"), text("text")).to_lowercase(),
            hook: line
                .get("hook")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned),
        }
    }

    fn described(&self) -> String {
        format!(
            "ran: {}, exit: {:?}, hook: {:?}, said: {:?}",
            self.ran,
            self.exit,
            self.hook,
            self.said.chars().take(200).collect::<String>()
        )
    }
}

/// What got through a scenario's wrap, as the canary reads it: the file one write left, the
/// connections its listener was dialled by, and what each step's tool result says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Canary {
    pub wrote: bool,
    pub dialled: usize,
    pub write: StepResult,
    pub connect: StepResult,
}

/// **Whether the wrap was in force**: a write outside the project and a direct dial both
/// refused, **and each step's own result says so**. No file and no dial is not enough: a command
/// that is not installed, one a hook denied and one that never ran leave neither. `Err` says
/// which got through, or which refusal was not seen.
pub(crate) fn judge_canary(seen: &Canary) -> Result<(), String> {
    let mut through = Vec::new();
    if seen.wrote {
        through.push("a write outside the project".to_owned());
    }
    if seen.dialled > 0 {
        through.push(format!("direct dials ({})", seen.dialled));
    }
    if !through.is_empty() {
        return Err(format!(
            "this scenario's wrap is not in force: {} got through",
            through.join(" and ")
        ));
    }
    // The write: the shell ran it and the OS said no (the shell's own words for EPERM, EACCES
    // and EROFS), not "not found" (127) or a command that never ran.
    let write_refused = seen.write.ran
        && seen.write.exit.is_some_and(|code| code != 0 && code != 127)
        && [
            "not permitted",
            "permission denied",
            "read-only file system",
        ]
        .iter()
        .any(|words| seen.write.said.contains(words));
    if !write_refused {
        return Err(format!(
            "the canary's write was not seen refused by the wrap (it may not have run, or a hook \
             refused it): {}",
            seen.write.described()
        ));
    }
    // The connect: curl ran and could not connect (7). 127 is curl missing, 28 a hung dial.
    let connect_refused = seen.connect.ran && seen.connect.exit == Some(7);
    if !connect_refused {
        return Err(format!(
            "the canary's connect was not seen refused by the wrap (curl may be missing, or a \
             hook refused it): {}",
            seen.connect.described()
        ));
    }
    Ok(())
}

/// The canary's two commands, as a chat of the scenario runs them: one write outside the
/// project and one connect that does not go through the proxy, to `port` on this machine.
pub(crate) fn canary_script(file: &std::path::Path, port: u16) -> serde_json::Value {
    serde_json::json!({
        "turns": [{"prompt": "canary", "calls": [
            {"step": "canary-write", "tool": "bash",
             "run": format!("echo canary > '{}'", file.display())},
            {"step": "canary-connect", "tool": "bash",
             "run": format!("curl -sS --noproxy '*' --max-time 5 -o /dev/null http://127.0.0.1:{port}/")}
        ]}]
    })
}

/// A listener and the connections it has accepted: one per scenario, so two scenarios running at
/// once never count each other's dials.
struct Dials {
    listener: std::net::TcpListener,
    accepted: usize,
}

impl Dials {
    /// How many connections it has accepted so far.
    fn count(&mut self) -> usize {
        while self.listener.accept().is_ok() {
            self.accepted += 1;
        }
        self.accepted
    }
}

impl Scenario {
    /// **The scenario's canary**: before anything else it proves its own wrap is in force, with
    /// one refused write and one refused connect, each read from outside the chat (the file a
    /// write left, a listener's connections) **and from its own tool result** (the OS said no),
    /// and each with a positive control, so a canary that cannot see is red, not green. The
    /// person then keeps both refusals blocked, and the chat is closed: what follows starts as
    /// if it had never run.
    pub(crate) fn canary(&self) {
        use std::net::{TcpListener, TcpStream};
        let outside = self.a_folder_outside("canary");
        let file = outside.join("written-by-the-chat");
        let listener = TcpListener::bind("127.0.0.1:0").expect("the canary's listener");
        listener.set_nonblocking(true).expect("non-blocking");
        let port = listener.local_addr().expect("its address").port();
        let mut dials = Dials {
            listener,
            accepted: 0,
        };

        // The positive controls, from this process, outside any wrap: it can write there and
        // dial there, so a refusal below is the wrap's.
        let control = outside.join("written-by-the-test");
        std::fs::write(&control, "control").expect("the control write is allowed");
        TcpStream::connect(("127.0.0.1", port)).expect("the control dial is allowed");
        let deadline = Instant::now() + Duration::from_secs(10);
        while dials.count() == 0 {
            assert!(
                Instant::now() < deadline,
                "the canary's listener never saw its own control dial"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        let controls = dials.count();

        let chat = self.starts(&canary_script(&file, port));
        self.until_played(chat, &["canary-write", "canary-connect"]);
        let seen = Canary {
            wrote: file.exists(),
            dialled: dials.count() - controls,
            write: StepResult::of(self.result_of(chat, "canary-write").as_ref()),
            connect: StepResult::of(self.result_of(chat, "canary-connect").as_ref()),
        };
        if let Err(why) = judge_canary(&seen) {
            self.fails_at("canary", "one refused write and one refused connect", &why);
        }
        // The two refusals may have raised asks: the person keeps them blocked.
        self.keeps_listed_asks_blocked();
        self.forget_the_canary_chat(chat);
    }

    /// The person presses Keep blocked on every ask the window lists, once each is listed (a
    /// block is told when the supervisor's report reaches the window).
    pub(crate) fn keeps_listed_asks_blocked(&self) {
        let _ = quiesce(self);
        for ask in self.queries().counted().asks {
            if ask.options.iter().any(|one| one.id == crate::asking::KEEP) {
                let _ = self
                    .person()
                    .presses_on(&ask, &super::person::Press::Keep(None));
            }
        }
    }
}

/// **The urllib step of #6675 was refused by the proxy**: Python ran, its request failed, and what
/// it says is the proxy's refusal: Python's `Tunnel connection failed: 403 Forbidden` (the proxy
/// answered the CONNECT with a 403) or a `ProxyError`. A bare "403" or "forbidden" is no proof: a
/// traceback line can say either. A `python3` that is not
/// installed (127), a step a hook denied or one that never ran is no refusal, and so no black
/// hole: the pinned scenario fails on it, and so cannot pass for the wrong reason. `Err` says what
/// the step came to.
pub(crate) fn urllib_refused_by_the_proxy(result: &StepResult) -> Result<(), String> {
    let refused = result.ran
        && result.exit.is_some_and(|code| code != 0 && code != 127)
        && result.hook.is_none()
        && ["tunnel connection failed", "proxyerror"]
            .iter()
            .any(|words| result.said.contains(words))
        && !result.said.contains("no such file")
        && !result.said.contains("not found");
    if refused {
        Ok(())
    } else {
        Err(format!(
            "the urllib step was not seen refused by the proxy (python3 may be missing, or the \
             step never ran): {}",
            result.described()
        ))
    }
}

/// **Whether an ask shows exactly the target that is decided** (T1.1-07, item 5): what the
/// person approves is what is decided, and what is granted is the string shown.
///
/// - **Escaped**: no bidi control, control or ANSI character is shown raw.
/// - **Punycode shown**: a host that is not ASCII is shown as its `xn--` form, never as the
///   look-alike its characters draw.
/// - **Never truncated to a different host**: what is shown is the whole of what is decided.
/// - **The grant written is the exact string shown**: `decided` is the string a grant is kept
///   by, or the name the proxy looks up, and it is the string shown.
pub(crate) fn shows_the_decided_target(shown: &BlockShown, decided: &str) -> Result<(), String> {
    shows_safely(shown)?;
    let text = &shown.target;
    if text != decided {
        let cut = text.ends_with('…') || text.ends_with("...");
        return Err(format!(
            "the ask shows {text:?}, but {decided:?} is decided{}",
            if cut {
                ": the shown target is cut short"
            } else {
                ""
            }
        ));
    }
    Ok(())
}

/// **The half of [`shows_the_decided_target`] that needs no decided string**, so every ask the
/// window lists is held to it: escaped (no bidi, control or ANSI character raw) and a host shown
/// in punycode.
pub(crate) fn shows_safely(shown: &BlockShown) -> Result<(), String> {
    let text = &shown.target;
    if let Some(bad) = text
        .chars()
        .find(|c| c.is_control() || purlis_unicode::bidi_control(*c))
    {
        return Err(format!(
            "the ask shows {text:?} with {bad:?} raw: a control, bidi or ANSI character must be \
             escaped"
        ));
    }
    if shown.what == crate::sandboxing::GrantWhat::Host && !text.is_ascii() {
        return Err(format!(
            "the ask shows the host {text:?} as it is written, not in punycode"
        ));
    }
    Ok(())
}
