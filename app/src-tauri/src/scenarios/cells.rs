// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The runner of the cells format** (#7322): it plays `tests/scenarios/cells/*.json` in the
//! scenario host and reports each cell through [`Scenario::report_cell`], so a cell that ran
//! reaches the comparison of runs as a cell result (kind, row, `refusal_expected`), never only
//! as a JUnit row (#7229).
//!
//! What a cell says is decided in `purlis_scenarios::play`, which has no chat and is tested
//! there: the loader, the window word list, a step's expectations against its events, and the
//! verdict. What this file adds is what only the host can do, in three places and no more:
//!
//! - **One press map** ([`press_of`]): a cell's `press` label is the window's own wording
//!   (`asking::allow_said`, `asking::KEEP_SAID`) or one of a few window actions, mapped to the
//!   scripted person's driver ([`super::person`]). A label not in the map is malformed.
//! - **One window map** ([`shows`]): each word of `play::Window` and `play::Counted` is one of the
//!   window's queries ([`super::queries`]), an exhaustive match, so a new word cannot be added
//!   without its query.
//! - **One script per batch and dialect**: the batch's steps are the turns of one chat, as the
//!   batch format says (cells run in order in one chat), so a later cell sees the earlier cells'
//!   state. Each shell call runs for real under the wrap, as every scenario's does.
//!
//! A cell the runner cannot play (an unknown word, a press or a restart with no driver here) is
//! reported `fail` with the reason, never `not_run` and never `pass`: the gate must go red on
//! it. A step's events are those the log gained from the step's start to the next step's start,
//! so an event written late, at the next prompt, lands in the next step, which is what `at:
//! next_prompt` and the marks that pin it are about.

use std::collections::BTreeMap;
use std::time::Duration;

use purlis_scenarios::judge::{Assertion, GlobalFailure, Observed, Run};
use purlis_scenarios::model::{
    Batch, Button, Cell, Dialect as CellDialect, Leg, Outcome, Press as CellPress, ReportVerdict,
    Restart, Step, TimingsMode,
};
use purlis_scenarios::net::{Origin, Scheme, TestCa, Variant};
use purlis_scenarios::plan::{Scheduled, Trigger, schedule};
use purlis_scenarios::play::{self, Counted, Hosts, Malformed, Query, Seen, Window};
use purlis_scenarios::timings::{CoreTimings, catalogue};

use super::checks::{Check, StepResult};
use super::host::{Dialect, Homes, Protection, SCALE, Scenario};
use super::person::Press;
use crate::sandboxing::GrantLevel;

/// **The matrix's HTTP client**: what a cell's `"client": "get"` calls run. It reads the proxy
/// from the environment and the test CA from `SSL_CERT_FILE`, as the chat's own tools do, and
/// says the status, or the error it was given, so a refusal that wrote nothing is still seen
/// here.
const GET_PY: &str = r#"import ssl, sys, urllib.request
url, patience = sys.argv[1], float(sys.argv[2])
try:
    with urllib.request.urlopen(url, timeout=patience) as answer:
        print("status", answer.status)
except Exception as why:
    print("error", type(why).__name__, why)
    sys.exit(1)
"#;

/// Where a cell's command names the client's own file: the matrix puts it in the chat's temp
/// folder, which this runner replaces with the file it wrote in the project.
const CLIENT_PATH: &str = "$TMPDIR/purlis-matrix-<chat>/get.py";

// ---- the press map ---------------------------------------------------------------------------

/// **What a cell's `press` comes to**, in the scripted person's terms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Pressing {
    /// A button on the ask the call raised, by the words the window says.
    OnAsk(Press),
    /// Stop chat.
    StopChat,
    /// Ignore the chat's needs-you item.
    Ignore,
    /// Esc, into the pane.
    Esc,
    /// Kill the harness's process.
    KillHarness,
}

/// The one map from a press's label to what the person does. The ask buttons are the window's
/// own words, so a label the window changes makes the cell malformed rather than pressing a
/// button that is no longer there.
pub(crate) fn press_of(label: &str) -> Result<Pressing, Malformed> {
    for level in [
        GrantLevel::Once,
        GrantLevel::Chat,
        GrantLevel::You,
        GrantLevel::Machine,
        GrantLevel::Project,
    ] {
        if [true, false]
            .into_iter()
            .any(|sandbox| crate::asking::allow_said(level, sandbox) == label)
        {
            return Ok(Pressing::OnAsk(Press::Allow(level)));
        }
    }
    Ok(match label {
        crate::asking::KEEP_SAID => Pressing::OnAsk(Press::Keep(None)),
        "Merge" => Pressing::OnAsk(Press::Option(crate::asking::MERGE.to_owned())),
        "Don't merge" => Pressing::OnAsk(Press::Option(crate::asking::DONT_MERGE.to_owned())),
        // A window action is named by its label or by its command (ticket 7's list).
        "Stop chat" | "stop_chat" => Pressing::StopChat,
        "Ignore the ask" | "ignore_needs_you" => Pressing::Ignore,
        "send_input_bytes" => Pressing::Esc,
        "Kill the harness process from the person's terminal" => Pressing::KillHarness,
        other => {
            return Err(Malformed(format!(
                "press `{other}` is in no press map: add it to `press_of`, with its driver"
            )));
        }
    })
}

/// A step's press, as a pair: what to press, and the delay after the ask is listed. A button's `target` and `when` must be the ask raised by the call.
fn pressing(press: &CellPress) -> Result<(Pressing, Duration), Malformed> {
    match press {
        CellPress::Action(label) => Ok((press_of(label)?, Duration::ZERO)),
        CellPress::Button(Button {
            label,
            target,
            when,
            delay,
            then,
        }) => {
            if target != "ask" || when != "ask.raised" {
                return Err(Malformed(format!(
                    "press `{label}` on `{target}` when `{when}`: only a button of the ask a \
                     call raised (`ask`, `ask.raised`) is played"
                )));
            }
            if let Some(then) = then {
                return Err(Malformed(format!(
                    "press `{label}`: `then: {then}` is not played"
                )));
            }
            let after = delay
                .as_deref()
                .map(|text| {
                    purlis_scenarios::timings::Expr::parse(text)
                        .ok()
                        .and_then(|e| e.seconds(&catalogue().seconds_at(SCALE)))
                        .map(Duration::from_secs_f64)
                        .ok_or_else(|| Malformed(format!("press delay `{text}` is no Timings")))
                })
                .transpose()?
                .unwrap_or(Duration::ZERO);
            Ok((press_of(label)?, after))
        }
    }
}

// ---- the window map --------------------------------------------------------------------------

/// **Whether `word` holds** for `chat`, read from the window's own query.
fn holds(scenario: &Scenario, word: Window, chat: u32) -> bool {
    let queries = scenario.queries();
    match word {
        Window::AskListed => queries.counted().asks.iter().any(|ask| ask.session == chat),
        Window::NoticeStanding => queries
            .notices()
            .standing
            .iter()
            .any(|(held, _)| *held == chat),
        Window::BlockedLogLists => !queries.blocked_log().is_empty(),
        Window::ChatLogBlockRow => {
            let marker = queries.blocks(chat, None);
            marker.blocks + marker.unverified > 0
        }
        Window::NeedsYouQueued => queries.needs_you().queue.contains(&chat),
        Window::MergeRowListed => !queries.merge_row().is_empty(),
        Window::SettingsGrantListed => !queries.settings().is_empty(),
    }
}

/// **Whether `query` is what the window shows** for `chat`: a word that holds ([`holds`]), or a
/// number the window's own query gives that equals the cell's.
fn shows(scenario: &Scenario, query: Query, chat: u32) -> bool {
    match query {
        Query::Holds(word) => holds(scenario, word, chat),
        Query::Equals(word, n) => {
            let queries = scenario.queries();
            let seen = match word {
                Counted::MarkerBlocked => u64::from(queries.blocks(chat, None).blocks),
                Counted::MarkerUnverified => u64::from(queries.blocks(chat, None).unverified),
                Counted::FinishRows => queries.finishes(chat).len() as u64,
                Counted::FinishBlocked => queries
                    .finishes(chat)
                    .last()
                    .map_or(0, |finish| u64::from(finish.blocked)),
            };
            seen == n
        }
    }
}

// ---- the batch -------------------------------------------------------------------------------

/// The host protection a batch's `protection` is.
fn protection_of(word: purlis_scenarios::model::Protection) -> Protection {
    use purlis_scenarios::model::Protection as Cells;
    match word {
        Cells::None => Protection::Off,
        Cells::Basic => Protection::Basic,
        Cells::Full => Protection::Full,
    }
}

fn host_dialect(dialect: CellDialect) -> Dialect {
    match dialect {
        CellDialect::Claude => Dialect::Claude,
        CellDialect::Codex => Dialect::Codex,
        CellDialect::Opencode => Dialect::OpenCode,
    }
}

/// The global check a mark's word names, where the suite has one.
fn check_of(word: &str) -> Option<Check> {
    match word {
        "refusal_black_hole" => Some(Check::NoBlackHole),
        "ask_black_hole" => Some(Check::NoUnansweredAsk),
        "block_without_source" => Some(Check::EveryBlockHasASource),
        "one_ask_one_raise" => Some(Check::OneAskOneRaise),
        _ => None,
    }
}

/// The name a step's call is played under in the chat's script.
fn call_name(cell: &Cell, step: usize, call: usize) -> String {
    format!("{}-s{}-c{}", cell.id, step + 1, call)
}

/// **A batch's chat script**: every step of every cell the run plays is a turn, in order, its
/// calls as the dialect-neutral script names them. `client` and the hosts are bound here.
fn script_of(
    cells: &[&Cell],
    hosts: &Hosts,
    client: &std::path::Path,
    trust: &str,
    tag: &str,
) -> serde_json::Value {
    let mut turns = Vec::new();
    for cell in cells {
        for (n, step) in cell.steps.iter().enumerate() {
            let calls: Vec<serde_json::Value> = step
                .calls
                .iter()
                .enumerate()
                .map(|(i, call)| {
                    let name = call_name(cell, n, i);
                    let mut one = if call.tool.eq_ignore_ascii_case("bash") {
                        let command = call.input["command"].as_str().unwrap_or_default();
                        serde_json::json!({
                            "step": name, "tool": "bash",
                            "run": format!("{trust}{}", bound(command, hosts, client, tag)),
                        })
                    } else {
                        serde_json::json!({
                            "step": name, "tool": call.tool.to_lowercase(),
                            "input": call.input,
                        })
                    };
                    for (key, on) in [
                        ("parallel", call.parallel),
                        ("background", call.background),
                        ("subagent", call.subagent),
                    ] {
                        if on {
                            one[key] = true.into();
                        }
                    }
                    one
                })
                .collect();
            turns.push(serde_json::json!({
                "prompt": format!("{} step {}", cell.id, n + 1),
                "calls": calls,
            }));
        }
    }
    serde_json::json!({ "turns": turns })
}

/// `command` with the cell's hosts (`$h1`), the matrix's client and the chat's tag (`<chat>`,
/// `<c>`: what the matrix names a chat's own files by) bound.
fn bound(command: &str, hosts: &Hosts, client: &std::path::Path, tag: &str) -> String {
    let mut out = command
        .replace(CLIENT_PATH, &client.display().to_string())
        .replace("<chat>", tag)
        .replace("<c>", tag);
    // Longest names first, so `$h10` is never read as `$h1` and a zero.
    let mut names: Vec<_> = hosts.iter().collect();
    names.sort_by_key(|(key, _)| std::cmp::Reverse(key.len()));
    for (key, name) in names {
        out = out.replace(&format!("${key}"), name);
    }
    out
}

/// The first host a step's calls name: what an ask raised by its call is for.
fn host_of_step(step: &Step, hosts: &Hosts) -> Option<String> {
    step.calls.iter().find_map(|call| {
        let command = call.input["command"].as_str()?;
        hosts
            .iter()
            .filter(|(key, _)| command.contains(&format!("${key}")))
            .map(|(_, name)| name.clone())
            .next()
    })
}

/// What a step's first call came to, as `outcome` names it.
fn outcome_of(scenario: &Scenario, chat: u32, name: &str) -> Outcome {
    let result = scenario.result_of(chat, name);
    let result = StepResult::of(result.as_ref());
    match (result.ran, result.exit, result.hook) {
        (_, _, Some(_)) => Outcome::Denied,
        (false, _, None) => Outcome::NoPost,
        (true, Some(0), None) => Outcome::CameBack,
        (true, _, None) => Outcome::Failed,
    }
}

/// The record of a cell that could not be played.
fn unplayable(scenario: &Scenario, cell: &Cell, why: &str) {
    let _ = scenario.report_cell(cell, ReportVerdict::Fail, Some(why.to_owned()), u64::MAX);
}

/// **Plays `batch` as `dialect`** and reports each cell it takes. `Err` is a batch that is
/// malformed as a whole; a cell the runner cannot play is its own `fail` record.
pub(crate) fn play_batch(homes: &Homes, batch: &Batch, dialect: CellDialect) {
    let Some(scenario) = Scenario::new(
        homes,
        protection_of(batch.protection),
        Some(batch.persona.as_str()),
        host_dialect(dialect),
    ) else {
        return;
    };
    let seconds = catalogue().seconds_at(SCALE);
    let pins = purlis_scenarios::validate::Validator::repo()
        .expect("the pins")
        .pins;
    let hosts: Hosts = batch
        .hosts
        .keys()
        .map(|key| (key.clone(), format!("{key}.cells.test")))
        .collect();

    // The cells this run plays: this dialect's, and not those a scaled run skips for timing
    // (reported as such by `report::skipped_for_timing`, never as passed).
    let mine: Vec<&Cell> = batch
        .cells
        .iter()
        .filter(|cell| cell.dialects.contains(&dialect))
        .collect();
    let (playable, malformed): (Vec<&Cell>, Vec<(&Cell, String)>) = {
        let mut playable = Vec::new();
        let mut malformed = Vec::new();
        for cell in mine {
            if schedule(
                cell,
                &CoreTimings,
                TimingsMode::Scaled,
                Trigger::PullRequest,
            ) == Scheduled::SkippedForTiming
            {
                continue;
            }
            match malformed_cell(cell, dialect, &hosts, &seconds, &pins) {
                Some(why) => malformed.push((cell, why)),
                None => playable.push(cell),
            }
        }
        (playable, malformed)
    };
    for (cell, why) in &malformed {
        unplayable(&scenario, cell, why);
    }
    if playable.is_empty() {
        return;
    }

    // The hosts a cell names resolve to one HTTPS origin under the test CA, so a host the person
    // allows is carried and every other is held at the proxy, as in production.
    let ca = TestCa::new().expect("the test CA");
    let names: Vec<&str> = hosts.values().map(String::as_str).collect();
    let origin = Origin::start(
        Scheme::Https {
            ca: &ca,
            names: &names,
        },
        Variant::Plain,
        std::net::IpAddr::from([127, 0, 0, 1]),
    )
    .expect("the cells' origin");
    for name in &names {
        scenario.names.origin(name, 443, origin.addr());
    }
    let folder = scenario.root.join("matrix-client");
    std::fs::create_dir_all(&folder).expect("the client's folder");
    let bundle = ca.bundle(&folder).expect("the CA bundle");
    let client = folder.join("get.py");
    std::fs::write(&client, GET_PY).expect("the client");
    let trust: String = purlis_scenarios::net::ca_env(&bundle)
        .into_iter()
        .map(|(name, value)| format!("export {name}='{}'; ", value.to_string_lossy()))
        .collect();

    // The cells' own marks of a global check, which the teardown judges once for the batch.
    let mut marked = std::collections::BTreeSet::new();
    for cell in &playable {
        for mark in &cell.expected_fail {
            if let Some(check) = mark.check.as_deref().and_then(check_of)
                && applies(mark, dialect)
                && marked.insert(check)
            {
                scenario.expected_fail(
                    check,
                    u32::try_from(mark.issue).expect("an issue number"),
                    mark.note.as_deref().unwrap_or("a cell's mark"),
                );
            }
        }
    }

    let script = script_of(&playable, &hosts, &client, &trust, &batch.batch);
    let mut chat = None;
    let mut played: Vec<Played> = Vec::new();
    for cell in &playable {
        for (n, step) in cell.steps.iter().enumerate() {
            let since = scenario.latest_seq();
            let transcript_from = scenario.transcript().len();
            let id = match chat {
                None => {
                    let id = scenario.starts(&script);
                    chat = Some(id);
                    id
                }
                Some(id) => {
                    if let Some(wait) = step.wait.as_deref() {
                        std::thread::sleep(Duration::from_secs_f64(seconds_of(wait, &seconds)));
                    }
                    scenario.types(id, &format!("{} step {}", cell.id, n + 1));
                    id
                }
            };
            let names: Vec<String> = (0..step.calls.len())
                .map(|i| call_name(cell, n, i))
                .collect();
            if let Some(press) = &step.press {
                let (what, after) = pressing(press).expect("checked before the run");
                press_it(&scenario, id, &what, after, host_of_step(step, &hosts));
            }
            let wanted: Vec<&str> = names.iter().map(String::as_str).collect();
            scenario.until_played(id, &wanted);
            // The step has ended when its log has gone quiet: a hook line spooled by the wrap is
            // written into the log after the call returns, and a later step's events must not
            // be taken for it.
            super::checks::quiesce(&scenario)
                .unwrap_or_else(|why| panic!("{why}\n{}", scenario.said()));
            if step.refusal_expected {
                let target = host_of_step(step, &hosts);
                scenario.refused(id, &format!("step {}", n + 1), target.as_deref());
            }
            let outcome = names.first().map(|name| outcome_of(&scenario, id, name));
            let window = step
                .window
                .iter()
                .map(|q| {
                    let (query, forbid) = play::window_of(q).expect("checked before the run");
                    shows(&scenario, query, id) != forbid
                })
                .collect();
            played.push(Played {
                cell: cell.id.clone(),
                step: n,
                since,
                outcome,
                window,
                transcript_from,
            });
        }
    }

    // Every step's events, once the log is quiet: a step's are those from its start to the next
    // step's, the last one's to the end.
    // Whatever ask nobody answered (a client that gave up, a hold that ended) the person now
    // keeps blocked, as every scenario that leaves one does.
    // A block written late lists its ask again after the first press, so it is repeated until
    // none is left to keep (a few rounds, never for ever).
    for _ in 0..5 {
        scenario.keeps_listed_asks_blocked();
        super::checks::quiesce(&scenario)
            .unwrap_or_else(|why| panic!("{why}\n{}", scenario.said()));
        let asking = scenario.queries().counted().asks;
        if asking
            .iter()
            .all(|ask| ask.options.iter().all(|one| one.id != crate::asking::KEEP))
        {
            break;
        }
    }
    let events = scenario.events();
    let all = scenario.transcript();
    let ends: Vec<u64> = played
        .iter()
        .skip(1)
        .map(|p| p.since)
        .chain([u64::MAX])
        .collect();
    let transcript_ends: Vec<usize> = played
        .iter()
        .skip(1)
        .map(|p| p.transcript_from)
        .chain([all.len()])
        .collect();
    let checks = super::checks::at_the_end(&scenario)
        .unwrap_or_else(|why| panic!("the global checks could not run: {why}"));
    let run = Run {
        dialect,
        os: super::report::this_os(),
        leg: Leg::Fake,
        timings: TimingsMode::Scaled,
        seconds: seconds.clone(),
    };
    for cell in &playable {
        let mut assertions: Vec<Assertion> = Vec::new();
        for ((p, end), transcript_end) in played
            .iter()
            .zip(&ends)
            .zip(&transcript_ends)
            .filter(|((p, _), _)| p.cell == cell.id)
        {
            let step = &cell.steps[p.step];
            let number = u32::try_from(p.step + 1).expect("a step number");
            let as_seen = |e: &purlis_core::eventlog::Event| Seen {
                seq: e.seq,
                at_ms: purlis_core::eventlog::written_at_ms(&e.ulid).unwrap_or(0),
                kind: e.kind.clone(),
                body: e.body.clone(),
            };
            let seen: Vec<Seen> = events
                .iter()
                .filter(|e| e.seq > p.since && e.seq <= *end)
                .map(as_seen)
                .collect();
            // What came after the step: never counted for it, but how long a late event took.
            let later: Vec<Seen> = events
                .iter()
                .filter(|e| e.seq > *end)
                .map(as_seen)
                .collect();
            if let (Some(want), Some(came)) = (step.outcome, p.outcome) {
                assertions.push(play::outcome_assertion(number, want, came));
            }
            assertions.extend(play::step_assertions(
                number, step, &seen, &later, &hosts, &seconds,
            ));
            for (i, ok) in p.window.iter().enumerate() {
                assertions.push(Assertion {
                    step: number,
                    assertion: format!("window[{i}]"),
                    observed: if *ok {
                        Observed::Pass
                    } else {
                        Observed::Fail(false.into())
                    },
                });
            }
            for (i, line) in step.transcript.iter().enumerate() {
                let to = (*transcript_end).min(all.len());
                let from = p.transcript_from.min(to);
                let said = all[from..to]
                    .iter()
                    .any(|entry| entry.to_string().contains(&line.says));
                assertions.push(Assertion {
                    step: number,
                    assertion: format!("transcript[{i}]"),
                    observed: if said != line.forbid {
                        Observed::Pass
                    } else {
                        Observed::Fail(said.into())
                    },
                });
            }
        }
        let globals: Vec<GlobalFailure> = cell
            .expected_fail
            .iter()
            .filter(|m| applies(m, dialect))
            .filter_map(|m| {
                let check = m.check.as_deref()?;
                checks
                    .expected
                    .iter()
                    .any(|(fired, issue, _)| {
                        Some(*fired) == check_of(check) && u64::from(*issue) == m.issue
                    })
                    .then(|| GlobalFailure {
                        check: check.to_owned(),
                        refusal: m.refusal.clone().unwrap_or_default(),
                    })
            })
            .collect();
        let (verdict, mut reason) = play::verdict(cell, &run, &assertions, &globals);
        if verdict == ReportVerdict::Fail {
            // What each step's window held, so a mark is pinned from the record and not guessed.
            let seen = played
                .iter()
                .zip(&ends)
                .filter(|(p, _)| p.cell == cell.id)
                .map(|(p, end)| {
                    let kinds: Vec<&str> = events
                        .iter()
                        .filter(|e| e.seq > p.since && e.seq <= *end)
                        .map(|e| e.kind.as_str())
                        .collect();
                    format!("step {} saw [{}]", p.step + 1, kinds.join(" "))
                })
                .collect::<Vec<_>>()
                .join("; ");
            reason = reason.map(|why| format!("{why} | {seen}"));
        }
        let first = played
            .iter()
            .find(|p| p.cell == cell.id)
            .map_or(0, |p| p.since);
        scenario
            .report_cell(cell, verdict, reason, first)
            .unwrap_or_else(|why| panic!("{}: {why}", cell.id));
    }
    assert!(
        checks.failures.is_empty(),
        "a global check failed:\n  {}\n{}",
        checks.failures.join("\n  "),
        scenario.said()
    );
}

/// What one played step left for the judging: where it began and what the window showed.
struct Played {
    cell: String,
    step: usize,
    since: u64,
    outcome: Option<Outcome>,
    window: Vec<bool>,
    /// How many lines the chats' transcripts held when the step began.
    transcript_from: usize,
}

fn applies(mark: &purlis_scenarios::model::Mark, dialect: CellDialect) -> bool {
    purlis_scenarios::validate::applies_to(mark, dialect, super::report::this_os())
        && mark.leg.is_none_or(|leg| leg == Leg::Fake)
        && mark.timings.is_none_or(|t| t == TimingsMode::Scaled)
}

fn seconds_of(expr: &str, seconds: &BTreeMap<&'static str, f64>) -> f64 {
    purlis_scenarios::timings::Expr::parse(expr)
        .ok()
        .and_then(|e| e.seconds(seconds))
        .unwrap_or(0.0)
}

/// **The person's press**, once the step's call has raised its ask.
fn press_it(
    scenario: &Scenario,
    chat: u32,
    what: &Pressing,
    after: Duration,
    host: Option<String>,
) {
    match what {
        Pressing::OnAsk(press) => {
            let host = host.expect("a press on an ask names a host in its call, checked before");
            let (_, pressed) = scenario.person().when_listed(chat, &host, press, after);
            pressed.unwrap_or_else(|why| panic!("{why}\n{}", scenario.said()));
        }
        Pressing::StopChat => scenario
            .person()
            .stop_chat(chat, false)
            .unwrap_or_else(|why| panic!("{why}")),
        Pressing::Ignore => scenario
            .person()
            .ignores(chat)
            .unwrap_or_else(|why| panic!("{why}")),
        Pressing::Esc => scenario
            .person()
            .esc(chat)
            .unwrap_or_else(|why| panic!("{why}")),
        Pressing::KillHarness => {
            scenario
                .person()
                .kills_the_harness(chat)
                .unwrap_or_else(|why| panic!("{why}"));
        }
    }
}

/// **Why `cell` cannot be played as `dialect`**, everything at once, or `None` when it can.
fn malformed_cell(
    cell: &Cell,
    dialect: CellDialect,
    hosts: &Hosts,
    seconds: &BTreeMap<&'static str, f64>,
    pins: &purlis_scenarios::validate::Pins,
) -> Option<String> {
    let steps = purlis_scenarios::validate::resolve(cell, dialect, super::report::this_os(), pins);
    let mut why: Vec<String> = Vec::new();
    for (n, step) in steps.iter().enumerate() {
        let at = |text: String| format!("step {}: {text}", n + 1);
        why.extend(
            play::malformed_step(step, hosts, seconds)
                .into_iter()
                .map(|m| at(m.0)),
        );
        if let Some(press) = &step.press {
            match pressing(press) {
                Err(m) => why.push(at(m.0)),
                Ok((Pressing::OnAsk(_), _)) if host_of_step(step, hosts).is_none() => why.push(at(
                    "a press on an ask needs a call that names a host (`$h1`)".to_owned(),
                )),
                Ok(_) => {}
            }
        }
        for call in &step.calls {
            if call.tool.eq_ignore_ascii_case("bash") && call.input["command"].as_str().is_none() {
                why.push(at(format!(
                    "a Bash call with no `command` (client `{}`) has no script here: only a \
                     `command` is run",
                    call.client.as_deref().unwrap_or("none")
                )));
            }
        }
        if let Some(restart) = step.restart {
            why.push(at(format!(
                "restart `{}` is not played: no driver for it here yet",
                match restart {
                    Restart::Quit => "quit",
                    Restart::Window => "window",
                    Restart::HostKilled => "host_killed",
                }
            )));
        }
        if step.chat.is_some() {
            why.push(at("a second chat (`chat`) is not played".to_owned()));
        }
        if let Some(text) = &step.wait
            && purlis_scenarios::timings::Expr::parse(text)
                .ok()
                .and_then(|e| e.seconds(seconds))
                .is_none()
        {
            why.push(at(format!("wait `{text}` is no Timings")));
        }
    }
    for mark in &cell.expected_fail {
        if let Some(check) = &mark.check
            && check_of(check).is_none()
        {
            why.push(format!(
                "the mark's check `{check}` has no check in the suite"
            ));
        }
    }
    (!why.is_empty()).then(|| format!("{}: malformed: {}", cell.id, why.join("; ")))
}

/// **Every cell in `tests/scenarios/cells` plays here, and is reported** (#7322): each batch, as
/// each dialect any of its cells names, in a scenario of its own. The legs' run-result gate then
/// holds a row to a record of this run.
///
/// Claude on macOS is ticket 10's leg (Claude Code's own sandbox, which the fake does not play),
/// so the fake leg plays no Claude cell there and owes none.
#[test]
fn every_cell_plays_and_is_reported() {
    let Some(homes) = Homes::of_this_scenario() else {
        return;
    };
    let validator = purlis_scenarios::validate::Validator::repo().expect("the pins");
    let batches = play::load(&purlis_scenarios::root().join("cells"), &validator)
        .unwrap_or_else(|why| panic!("the cells are malformed: {why}"));
    // A batch's cells that the run skips for their timing are reported as such.
    for batch in &batches {
        let skipped =
            super::report::skipped_for_timing(batch, TimingsMode::Scaled, Trigger::PullRequest);
        super::report::write(&skipped).expect("the leg's report");
        for dialect in CellDialect::ALL {
            if dialect == CellDialect::Claude && cfg!(target_os = "macos") {
                continue;
            }
            if batch.cells.iter().any(|c| c.dialects.contains(&dialect)) {
                play_batch(&homes, batch, dialect);
            }
        }
    }
}
