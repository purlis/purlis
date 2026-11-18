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
//! | [`Check::OneAskOneRaise`] | more `ask.raised` were written for a chat than asks it was seen to have (Q262) |
//!
//! **"At the end" is quiesced** ([`quiesce`]): the log has had no event for the scenario's
//! `group` and `tally_tick` together, judged by the events' own times, never by a sleep. What
//! the Linux supervisor tells and a hook line in flight each reach the log as an event, which
//! restarts that wait, so neither is judged before it is written.
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
}

/// **What a scenario observed at its end**: [`judge`]'s whole input.
#[derive(Debug, Clone)]
pub(crate) struct Observed<'a> {
    pub events: &'a [Event],
    pub refusals: &'a [Refusal],
    /// The asks the window lists at the end: "chat N `key` says".
    pub asks_left: &'a [String],
    /// Each chat's distinct asks the scenario saw, by the chat's logged id.
    pub asks_seen: &'a BTreeMap<String, usize>,
    /// What Settings lists beyond what it listed before the scenario began: (kind, target, level).
    pub grants_listed: &'a [(String, String, String)],
    /// What the person granted in the scenario and has not revoked: (kind, target, level).
    pub grants_made: &'a [(String, String, String)],
    /// The most a hook may take.
    pub hook_budget: Duration,
}

/// One check's failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Violation {
    pub check: Check,
    pub what: String,
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
    let mut fail = |check: Check, what: String| found.push(Violation { check, what });

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
            });
        }
    }
    for refusal in &refusals {
        let written = observed.events.iter().any(|event| {
            kind_is_a_stop(&event.kind)
                && event.chat.as_deref() == Some(refusal.chat.as_str())
                && match (&refusal.target, event.kind.as_str()) {
                    // An ask names no target in the log: it answers any refusal of its chat.
                    (_, stops::ASK_RAISED) | (None, _) => true,
                    (Some(wanted), _) => target_in(&event.body).is_some_and(|one| &one == wanted),
                }
        });
        if !written {
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
            );
        }
    }

    // No unanswered ask at the end.
    for ask in observed.asks_left {
        fail(
            Check::NoUnansweredAsk,
            format!("the window still lists {ask}"),
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
            ),
            Some((_, _, made_at)) if made_at == "once" => fail(
                Check::NoLeftoverGrant,
                format!("a once-grant of {kind} {target} outlived its use: Settings lists it"),
            ),
            Some(_) => {}
        }
    }

    // No hook over budget.
    let budget_ms = u64::try_from(observed.hook_budget.as_millis()).unwrap_or(u64::MAX);
    for event in observed.events {
        if let Some(took) = event
            .body
            .get("hook_ms")
            .and_then(serde_json::Value::as_u64)
            && took > budget_ms
        {
            fail(
                Check::HookWithinBudget,
                format!(
                    "event {} ({}) took a hook {took} ms, over its {budget_ms} ms budget",
                    event.seq, event.kind
                ),
            );
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
            );
        }
    }

    // One ask, one raise (Q262): a chat's raises are no more than the asks it was seen to
    // have, or the answers the log holds, whichever is more.
    let mut raised: BTreeMap<&str, usize> = BTreeMap::new();
    let mut answered: BTreeMap<&str, usize> = BTreeMap::new();
    for event in observed.events {
        let Some(chat) = event.chat.as_deref() else {
            continue;
        };
        match event.kind.as_str() {
            stops::ASK_RAISED => *raised.entry(chat).or_default() += 1,
            stops::ASK_ANSWERED => *answered.entry(chat).or_default() += 1,
            _ => {}
        }
    }
    for (chat, raises) in raised {
        let asks = observed
            .asks_seen
            .get(chat)
            .copied()
            .unwrap_or(0)
            .max(answered.get(chat).copied().unwrap_or(0));
        if raises > asks {
            fail(
                Check::OneAskOneRaise,
                format!("chat {chat}: {raises} ask.raised written for {asks} ask(s) seen"),
            );
        }
    }
    found
}

/// A check a scenario knowingly fails, with the issue that says why.
#[derive(Debug, Clone)]
pub(crate) struct Mark {
    pub check: Check,
    pub issue: u32,
    pub why: String,
}

/// **What came of a scenario's checks**: each failure that is not marked, and each marked check
/// (said, and reported as an expected fail) or mark that did not fire.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Outcome {
    pub failures: Vec<String>,
    pub expected: Vec<(Check, u32, String)>,
}

/// **Applies `marks` to what [`judge`] found**: a failure of a marked check is an expected fail;
/// any other is a failure; a mark whose check passed is an unexpected pass, a failure too.
pub(crate) fn outcome(found: Vec<Violation>, marks: &[Mark]) -> Outcome {
    let mut out = Outcome::default();
    for violation in &found {
        match marks.iter().find(|mark| mark.check == violation.check) {
            Some(mark) => out
                .expected
                .push((violation.check, mark.issue, violation.what.clone())),
            None => out
                .failures
                .push(format!("{}: {}", violation.check.word(), violation.what)),
        }
    }
    for mark in marks {
        if !found.iter().any(|one| one.check == mark.check) {
            out.failures.push(format!(
                "{}: unexpected pass: #{} ({}) is marked as failing here, and the check passed: \
                 remove the mark",
                mark.check.word(),
                mark.issue,
                mark.why
            ));
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
    seen: Mutex<BTreeMap<String, BTreeSet<String>>>,
    refusals: Mutex<Vec<Refusal>>,
    marks: Mutex<Vec<Mark>>,
    /// What Settings listed when the scenario began: the project's own declarations.
    baseline: Mutex<Vec<(String, String)>>,
}

impl Ledger {
    /// A chat was seen to have ask `key`.
    pub(crate) fn saw(&self, chat: &Option<String>, key: &str) {
        if let Some(chat) = chat {
            self.seen
                .lock()
                .expect("seen")
                .entry(chat.clone())
                .or_default()
                .insert(key.to_owned());
        }
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
            });
    }

    /// **Marks `check` as failing here, for `issue`**: per scenario and check, never for the
    /// suite. A mark whose check passes is a failure at the end.
    pub(crate) fn expected_fail(&self, check: Check, issue: u32, why: &str) {
        assert!(issue > 0, "an expected fail names its issue");
        self.ledger.marks.lock().expect("marks").push(Mark {
            check,
            issue,
            why: why.to_owned(),
        });
    }
}

/// **Until the scenario is quiet**: its log has had no event for `group` and `tally_tick`
/// together, by the events' own times. A condition polled, never a sleep that hopes.
pub(crate) fn quiesce(scenario: &Scenario) -> Result<(), String> {
    let settle = scenario.timings.group() + scenario.timings.tally_tick();
    let deadline = Instant::now() + super::PLAYS_WITHIN;
    loop {
        let newest = scenario
            .events()
            .iter()
            .filter_map(|event| purlis_core::eventlog::written_at_ms(&event.ulid))
            .max();
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| {
                u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
            });
        let quiet_for = newest.map_or(u64::MAX, |at| now_ms.saturating_sub(at));
        if u128::from(quiet_for) >= settle.as_millis() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "the log never went quiet: its newest event is {quiet_for} ms old, and quiet \
                 is {} ms",
                settle.as_millis()
            ));
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

/// **The scenario's checks at its end**: quiesced, observed through the window's own queries,
/// judged, and the marks applied. `Err` is a report for a red run.
pub(crate) fn at_the_end(scenario: &Scenario) -> Result<Outcome, String> {
    quiesce(scenario)?;
    let queries = scenario.queries();
    let asks = queries.counted();
    let asks_left: Vec<String> = asks
        .asks
        .iter()
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
    let asks_seen: BTreeMap<String, usize> = scenario
        .ledger
        .seen
        .lock()
        .expect("seen")
        .iter()
        .map(|(chat, keys)| (chat.clone(), keys.len()))
        .collect();
    let events = scenario.events();
    let refusals = scenario.ledger.refusals.lock().expect("refusals").clone();
    let found = judge(&Observed {
        events: &events,
        refusals: &refusals,
        asks_left: &asks_left,
        asks_seen: &asks_seen,
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
                &self.events(),
                &queries,
                &self.transcript()
            )
        )
    }
}

/// What got through a scenario's wrap, as the canary reads it: the file one write left, and
/// the connections its listener was dialled by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Canary {
    pub wrote: bool,
    pub dialled: usize,
}

/// **Whether the wrap was in force**: a write outside the project and a direct dial both
/// refused. `Err` says which got through.
pub(crate) fn judge_canary(seen: Canary) -> Result<(), String> {
    let mut through = Vec::new();
    if seen.wrote {
        through.push("a write outside the project".to_owned());
    }
    if seen.dialled > 0 {
        through.push(format!("{} direct dial(s)", seen.dialled));
    }
    if through.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "this scenario's wrap is not in force: {} got through",
            through.join(" and ")
        ))
    }
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

impl Scenario {
    /// **The scenario's canary**: before anything else it proves its own wrap is in force, with
    /// one refused write and one refused connect, each read from outside the chat (the file a
    /// write left, a listener's connections), and each with a positive control, so a canary that
    /// cannot see is red, not green. The person then keeps both refusals blocked.
    pub(crate) fn canary(&self) {
        use std::net::{TcpListener, TcpStream};
        let outside = self.a_folder_outside("canary");
        let file = outside.join("written-by-the-chat");
        let listener = TcpListener::bind("127.0.0.1:0").expect("the canary's listener");
        listener.set_nonblocking(true).expect("non-blocking");
        let port = listener.local_addr().expect("its address").port();

        // The positive controls, from this process, outside any wrap: it can write there and
        // dial there, so a refusal below is the wrap's.
        let control = outside.join("written-by-the-test");
        std::fs::write(&control, "control").expect("the control write is allowed");
        let before = accepted(&listener);
        TcpStream::connect(("127.0.0.1", port)).expect("the control dial is allowed");
        let mut dialled_by_control = 0;
        let deadline = Instant::now() + Duration::from_secs(10);
        while dialled_by_control == 0 {
            dialled_by_control = accepted(&listener) - before;
            assert!(
                Instant::now() < deadline,
                "the canary's listener never saw its own control dial"
            );
            std::thread::sleep(Duration::from_millis(10));
        }

        let chat = self.starts(&canary_script(&file, port));
        self.until_played(chat, &["canary-write", "canary-connect"]);
        let seen = Canary {
            wrote: file.exists(),
            dialled: accepted(&listener) - before - dialled_by_control,
        };
        if let Err(why) = judge_canary(seen) {
            self.fails_at("canary", "one refused write and one refused connect", &why);
        }
        // The two refusals may have raised asks: the person keeps them blocked.
        self.keeps_listed_asks_blocked();
    }

    /// The person presses Keep blocked on every ask the window lists, once each is listed (a
    /// block is told when the supervisor's report reaches the window).
    pub(crate) fn keeps_listed_asks_blocked(&self) {
        let _ = quiesce(self);
        for ask in self.queries().counted().asks {
            if ask.options.iter().any(|one| one.id == crate::asking::KEEP) {
                let _ = self.person().press(&ask, &super::person::Press::Keep(None));
            }
        }
    }
}

/// How many connections `listener` has accepted so far (a counter, kept apart).
fn accepted(listener: &std::net::TcpListener) -> usize {
    static COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    while listener.accept().is_ok() {
        COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
    COUNT.load(std::sync::atomic::Ordering::SeqCst)
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
