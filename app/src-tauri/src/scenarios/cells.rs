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
//! A cell the runner cannot play (an unknown word, a press or a restart with no driver here, a
//! `template` or a batch's `deny_exempt_rules`) is reported `fail` with the reason, never
//! `not_run` and never `pass`: the gate must go red on it. The steps it plays are the steps it
//! validated (a dialect's and an OS's patches and the pins applied).
//!
//! **A step has one window, and every event and chat line is counted in exactly one.** Its
//! events are those the log gained from the step's start to the next step's start, its chat lines
//! those the transcript gained over the same time; the last step's end is taken before the
//! runner's own teardown presses. An event written late, at the next prompt, is counted in the
//! next step's window (`at: next_prompt` reads past the window for what the next prompt wrote,
//! `purlis_scenarios::play`), and the last step's `forbid` also reads what the run wrote after its
//! window, the teardown's lines included, so nothing is judged by no step. A step that made calls
//! and has no `hook.pretooluse` of its own in its window fails: it saw nothing to judge. A late
//! line of a middle step is judged only where the next step's own forbids see it (#7364).

use std::collections::{BTreeMap, BTreeSet};
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

use super::checks::{Check, Position, Scope, StepResult};
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
fn shows(scenario: &Scenario, query: &Query, chat: u32, hosts: &Hosts) -> bool {
    match query {
        Query::Holds(word) => holds(scenario, *word, chat),
        Query::AskFor(key) => hosts.get(key).is_some_and(|name| {
            scenario.queries().counted().asks.iter().any(|ask| {
                super::person::is_the_ask_for(
                    ask.session,
                    &super::person::shown_target(ask),
                    chat,
                    name,
                )
            })
        }),
        Query::Equals(word, n) => counted_of(scenario, *word, chat) == *n,
    }
}

/// **The number the window shows** for `word`, for `chat`.
fn counted_of(scenario: &Scenario, word: Counted, chat: u32) -> u64 {
    let queries = scenario.queries();
    match word {
        Counted::MarkerBlocked => u64::from(queries.blocks(chat, None).blocks),
        Counted::MarkerUnverified => u64::from(queries.blocks(chat, None).unverified),
        Counted::FinishRows => queries.finishes(chat).len() as u64,
        Counted::FinishBlocked => queries
            .finishes(chat)
            .last()
            .map_or(0, |finish| u64::from(finish.blocked)),
    }
}

/// **How a `window` entry came out**: a pass, or what the window showed (the number a counted
/// word gave, or whether a word held), so a mark pins what was seen.
fn window_observed(
    scenario: &Scenario,
    query: &Query,
    forbid: bool,
    chat: u32,
    hosts: &Hosts,
) -> Observed {
    match query {
        Query::Equals(word, n) => {
            let seen = counted_of(scenario, *word, chat);
            if (seen == *n) != forbid {
                Observed::Pass
            } else {
                Observed::Fail(seen.into())
            }
        }
        _ => {
            let shown = shows(scenario, query, chat, hosts);
            if shown != forbid {
                Observed::Pass
            } else {
                Observed::Fail(shown.into())
            }
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

/// A line of what the runner is doing, in a red run's log.
fn said(line: &str) {
    let _ = std::io::Write::write_fmt(&mut std::io::stderr(), format_args!("cells: {line}\n"));
}

/// The record of a cell that could not be played: a `fail` with its reasons and no events. It
/// needs no scenario, so a batch with nothing to play never starts one (whose teardown would
/// judge the log a scenario before it left).
fn unplayable(cell: &Cell, dialect: CellDialect, batch: &Batch, why: &str) {
    let record = super::report::ran(
        cell,
        dialect,
        batch.protection,
        TimingsMode::Scaled,
        ReportVerdict::Fail,
        Some(why.to_owned()),
        Vec::new(),
    );
    super::report::write(&[record.leg]).expect("the leg's report");
}

/// The hosts a batch binds, by key: each a `.test` name of its own.
fn hosts_of(batch: &Batch) -> Hosts {
    batch
        .hosts
        .keys()
        .map(|key| (key.clone(), format!("{key}.cells.test")))
        .collect()
}

/// **The cells of `batch` this run takes as `dialect`, sorted**: those a `scenario` cover names
/// ([`play::covered`]), less those a scaled run skips for their timing (reported as such by
/// `report::skipped_for_timing`, never as passed). The first list is what plays, the second what
/// cannot, with why.
fn classified<'a>(
    batch: &'a Batch,
    file: &str,
    covered: &BTreeSet<(String, String)>,
    dialect: CellDialect,
) -> (Vec<&'a Cell>, Vec<(&'a Cell, String)>) {
    let seconds = catalogue().seconds_at(SCALE);
    let pins = purlis_scenarios::validate::Validator::repo()
        .expect("the pins")
        .pins;
    let hosts = hosts_of(batch);
    let mut playable = Vec::new();
    let mut malformed = Vec::new();
    for cell in &batch.cells {
        if !cell.dialects.contains(&dialect)
            || !covered.contains(&(file.to_owned(), cell.id.clone()))
            || schedule(
                cell,
                &CoreTimings,
                TimingsMode::Scaled,
                Trigger::PullRequest,
            ) == Scheduled::SkippedForTiming
        {
            continue;
        }
        match malformed_cell(cell, batch, dialect, &hosts, &seconds, &pins) {
            Some(why) => malformed.push((cell, why)),
            None => playable.push(cell),
        }
    }
    (playable, malformed)
}

/// **Plays `playable`, cells of `batch`, as `dialect`** in a scenario of its own, and reports
/// each. The scenario's global checks are judged on its device's log, which every scenario of a
/// process shares: so the test runs each batch and dialect in a child process with homes of its
/// own ([`Homes::of_this_scenario_with`]).
pub(crate) fn play_batch(homes: &Homes, batch: &Batch, dialect: CellDialect, playable: &[&Cell]) {
    let seconds = catalogue().seconds_at(SCALE);
    let hosts = hosts_of(batch);
    // What is played is what was validated: each cell's steps with its dialect's and OS's
    // patches and the pins' shows applied (`classified` checked these), never the raw steps.
    let pins = purlis_scenarios::validate::Validator::repo()
        .expect("the pins")
        .pins;
    let resolved: Vec<Cell> = playable
        .iter()
        .map(|cell| {
            let mut one = (*cell).clone();
            one.steps =
                purlis_scenarios::validate::resolve(cell, dialect, super::report::this_os(), &pins);
            one
        })
        .collect();
    let playable: Vec<&Cell> = resolved.iter().collect();
    said(&format!(
        "{} as {}: starting its scenario for {} cells",
        batch.batch,
        dialect.word(),
        playable.len()
    ));
    let Some(scenario) = Scenario::new(
        homes,
        protection_of(batch.protection),
        Some(batch.persona.as_str()),
        host_dialect(dialect),
    ) else {
        return;
    };

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
                // Answered only by what this step wrote (an earlier step's ask or block for the
                // same target never answers a repeat), named by its cell so a mark can scope to it.
                let target = host_of_step(step, &hosts);
                scenario.refused_since(
                    id,
                    &format!("{} step {}", cell.id, n + 1),
                    target.as_deref(),
                    since,
                );
            }
            let outcome = names.first().map(|name| outcome_of(&scenario, id, name));
            let window = step
                .window
                .iter()
                .map(|q| {
                    let (query, forbid) = play::window_of(q).expect("checked before the run");
                    window_observed(&scenario, &query, forbid, id, &hosts)
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

    // The last step's window closes here, before the runner's own presses: what it does at the
    // end (keeping every ask nobody answered blocked) is not the last step's.
    let end_seq = scenario.latest_seq();
    let end_transcript = scenario.transcript().len();
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
    // Each step's window, events and chat lines, from its start to the next step's start: the
    // windows partition the run, so no event or line is in two steps.
    let ends: Vec<u64> = played
        .iter()
        .skip(1)
        .map(|p| p.since)
        .chain([end_seq])
        .collect();
    let transcript_ends: Vec<usize> = played
        .iter()
        .skip(1)
        .map(|p| p.transcript_from)
        .chain([end_transcript])
        .collect();
    // Where a global check's failure belongs to a step: the same windows, but the last one runs
    // to the log's end, since what the last step caused late is written after its quiet.
    let scope_ends: Vec<u64> = ends
        .iter()
        .take(ends.len() - 1)
        .copied()
        .chain([u64::MAX])
        .collect();
    // A cell's range in the log, or one of its steps': (after, up to and including).
    let range_of = |cell_id: &str, step: Option<usize>, scoped: bool| -> (u64, u64) {
        let ours: Vec<usize> = played
            .iter()
            .enumerate()
            .filter(|(_, p)| p.cell == cell_id && step.is_none_or(|n| p.step == n))
            .map(|(i, _)| i)
            .collect();
        match (ours.first(), ours.last()) {
            (Some(first), Some(last)) => (
                played[*first].since,
                if scoped {
                    scope_ends[*last]
                } else {
                    ends[*last]
                },
            ),
            _ => (0, 0),
        }
    };

    // The cells' own marks of a global check, each excusing only the step or refusal it names
    // and never the batch's check: a refusal by its cell's label, any other by the events of the
    // step (`"refusal": "step N"`) or of the cell. The teardown judges them once more, on the
    // same log.
    let mut registered: Vec<(String, usize, &purlis_scenarios::model::Mark)> = Vec::new();
    for cell in &playable {
        for mark in &cell.expected_fail {
            let Some(check) = mark.check.as_deref().and_then(check_of) else {
                continue;
            };
            if !applies(mark, dialect) {
                continue;
            }
            let scope = match check {
                Check::NoBlackHole => Scope::Refusal(format!(
                    "{} {}",
                    cell.id,
                    mark.refusal.as_deref().unwrap_or_default()
                )),
                _ => {
                    let step = mark
                        .refusal
                        .as_deref()
                        .and_then(|text| text.strip_prefix("step "))
                        .and_then(|n| n.parse::<usize>().ok())
                        .map(|n| n.saturating_sub(1));
                    let (after, upto) = range_of(&cell.id, step, true);
                    Scope::Events { after, upto }
                }
            };
            let at = scenario.expected_fail_in(
                check,
                u32::try_from(mark.issue).expect("an issue number"),
                mark.note.as_deref().unwrap_or("a cell's mark"),
                scope,
            );
            registered.push((cell.id.clone(), at, mark));
        }
    }
    let checks = super::checks::at_the_end(&scenario)
        .unwrap_or_else(|why| panic!("the global checks could not run: {why}"));
    said(&format!(
        "{} as {}: the global checks held {} as expected and failed {}",
        batch.batch,
        dialect.word(),
        checks.expected.len(),
        checks.failures.len()
    ));
    let run = Run {
        dialect,
        os: super::report::this_os(),
        leg: Leg::Fake,
        timings: TimingsMode::Scaled,
        seconds: seconds.clone(),
    };
    for cell in &playable {
        let mut assertions: Vec<Assertion> = Vec::new();
        for (index, ((p, end), transcript_end)) in played
            .iter()
            .zip(&ends)
            .zip(&transcript_ends)
            .enumerate()
            .filter(|(_, ((p, _), _))| p.cell == cell.id)
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
                number,
                step,
                &seen,
                &later,
                index + 1 == played.len(),
                &hosts,
                &seconds,
            ));
            for (i, observed) in p.window.iter().enumerate() {
                assertions.push(Assertion {
                    step: number,
                    assertion: format!("window[{i}]"),
                    observed: observed.clone(),
                });
            }
            for (i, line) in step.transcript.iter().enumerate() {
                let to = (*transcript_end).min(all.len());
                let from = p.transcript_from.min(to);
                let said = all[from..to].iter().any(|entry| says(entry, &line.says));
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
        // The marks of this cell that excused a failure of a check, and only those.
        let own_marks: BTreeSet<usize> = registered
            .iter()
            .filter(|(id, _, _)| *id == cell.id)
            .map(|(_, at, _)| *at)
            .collect();
        let globals: Vec<GlobalFailure> = registered
            .iter()
            .filter(|(id, at, _)| *id == cell.id && checks.fired.contains(at))
            .filter_map(|(_, _, m)| {
                Some(GlobalFailure {
                    check: m.check.clone()?,
                    refusal: m.refusal.clone().unwrap_or_default(),
                })
            })
            .collect();
        let (mut verdict, mut reason) = play::verdict(cell, &run, &assertions, &globals);
        // A global check that failed with no mark is written to the record of the cell it
        // happened in before the run panics (#7229), never only to the panic.
        let (after, upto) = range_of(&cell.id, None, true);
        let unmarked: Vec<&str> = checks
            .failures
            .iter()
            .zip(&checks.failure_at)
            .filter(|(_, at)| owns(&cell.id, (after, upto), &own_marks, at))
            .map(|(text, _)| text.as_str())
            .collect();
        if !unmarked.is_empty() {
            let said = format!("a global check failed: {}", unmarked.join("; "));
            verdict = ReportVerdict::Fail;
            reason = Some(match reason {
                Some(why) => format!("{why}; {said}"),
                None => said,
            });
        }
        if verdict == ReportVerdict::Fail {
            // What each step's window held (its events, and the chat's lines over the same time),
            // so a mark is pinned from the record and not guessed.
            let seen = played
                .iter()
                .zip(&ends)
                .zip(&transcript_ends)
                .filter(|((p, _), _)| p.cell == cell.id)
                .map(|((p, end), transcript_end)| {
                    let kinds: Vec<&str> = events
                        .iter()
                        .filter(|e| e.seq > p.since && e.seq <= *end)
                        .map(|e| e.kind.as_str())
                        .collect();
                    let to = (*transcript_end).min(all.len());
                    let said: Vec<String> = all[p.transcript_from.min(to)..to]
                        .iter()
                        .map(|line| {
                            let text = line["text"].as_str().unwrap_or_default();
                            format!(
                                "{}: {}",
                                line["kind"].as_str().unwrap_or("?"),
                                text.chars()
                                    .take(140)
                                    .collect::<String>()
                                    .replace('\n', " ")
                            )
                        })
                        .collect();
                    format!(
                        "step {} saw [{}] and the chat [{}]",
                        p.step + 1,
                        kinds.join(" "),
                        said.join(" / ")
                    )
                })
                .collect::<Vec<_>>()
                .join("; ");
            reason = reason.map(|why| format!("{why} | {seen}"));
        }
        // The record's events are the cell's own: from its first step's start to its last
        // step's end.
        let (first, last) = range_of(&cell.id, None, false);
        scenario
            .report_cell(cell, verdict, reason, first, last)
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
    window: Vec<Observed>,
    /// How many lines the chats' transcripts held when the step began.
    transcript_from: usize,
}

/// **Whether a failure of a global check is `cell`'s**: its own refusal (by label), an event in
/// its range, a mark of its own that did not fire, or a failure of the batch as a whole (an ask
/// left listed), which is every cell's.
fn owns(cell: &str, range: (u64, u64), marks: &BTreeSet<usize>, at: &Position) -> bool {
    match (at.mark, at.seq, &at.refusal) {
        (Some(mark), _, _) => marks.contains(&mark),
        (None, None, None) => true,
        _ => {
            at.seq.is_some_and(|seq| seq > range.0 && seq <= range.1)
                || at
                    .refusal
                    .as_deref()
                    .is_some_and(|label| label.starts_with(&format!("{cell} step ")))
        }
    }
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
    batch: &Batch,
    dialect: CellDialect,
    hosts: &Hosts,
    seconds: &BTreeMap<&'static str, f64>,
    pins: &purlis_scenarios::validate::Pins,
) -> Option<String> {
    let steps = purlis_scenarios::validate::resolve(cell, dialect, super::report::this_os(), pins);
    let mut why: Vec<String> = play::malformed_cell(cell, batch)
        .into_iter()
        .map(|m| m.0)
        .collect();
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
        if let Some(check) = &mark.check {
            match check_of(check) {
                None => why.push(format!(
                    "the mark's check `{check}` has no check in the suite"
                )),
                // A refusal is excused by the step that was refused, so a mark names it.
                Some(Check::NoBlackHole)
                    if mark
                        .refusal
                        .as_deref()
                        .and_then(|text| text.strip_prefix("step "))
                        .and_then(|n| n.parse::<usize>().ok())
                        .is_none_or(|n| n == 0 || n > steps.len()) =>
                {
                    why.push(format!(
                        "the mark's check `{check}` names no refusal (`\"refusal\": \"step N\"`)"
                    ));
                }
                Some(_) => {}
            }
        }
    }
    (!why.is_empty()).then(|| format!("{}: malformed: {}", cell.id, why.join("; ")))
}

/// The variable that tells a child which batch and dialect it plays (`B0:claude`).
const ONLY: &str = "PURLIS_CELLS_ONLY";

/// **Every cell a `scenario` cover names plays here, and is reported** (#7322): each batch and
/// dialect in a child process of its own, with homes and a device log of its own, so one batch's
/// refusals are never another's global checks. The legs' run-result gate then holds a row to a
/// record of this run. A cell no cover names is data only until the pull request that covers it.
///
/// The parent classifies the cells, writes the records of those it cannot play (malformed: a
/// `fail`) and of those a scaled run skips for timing (`not_run`), and starts a child for each
/// batch and dialect with something to play. A child that fails does not stop the others, and the
/// parent fails at the end naming each.
///
/// Claude on macOS is ticket 10's leg (Claude Code's own sandbox, which the fake does not play),
/// so the fake leg plays no Claude cell there and owes none.
#[test]
fn every_cell_plays_and_is_reported() {
    let validator = purlis_scenarios::validate::Validator::repo().expect("the pins");
    let batches = play::load(&purlis_scenarios::root().join("cells"), &validator)
        .unwrap_or_else(|why| panic!("the cells are malformed: {why}"));
    let covered =
        play::covered(&purlis_scenarios::coverage::Coverage::read().expect("coverage.json"));
    let only = std::env::var(ONLY).ok();
    let mut red = Vec::new();
    for (file, batch) in &batches {
        if only.is_none() {
            let skipped =
                super::report::skipped_for_timing(batch, TimingsMode::Scaled, Trigger::PullRequest);
            super::report::write(&skipped).expect("the leg's report");
        }
        for dialect in CellDialect::ALL {
            if dialect == CellDialect::Claude && cfg!(target_os = "macos") {
                continue;
            }
            let (playable, malformed) = classified(batch, file, &covered, dialect);
            let this = format!("{}:{}", batch.batch, dialect.word());
            if only.as_deref().is_some_and(|want| want != this) {
                continue;
            }
            if only.is_none() {
                for (cell, why) in &malformed {
                    unplayable(cell, dialect, batch, why);
                }
                said(&format!(
                    "{this}: {} to play, {} malformed",
                    playable.len(),
                    malformed.len()
                ));
            }
            if playable.is_empty() {
                continue;
            }
            let ran = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                if let Some(homes) = Homes::of_this_scenario_with(&[(ONLY, &this)]) {
                    play_batch(&homes, batch, dialect, &playable);
                }
            }));
            // In the child a failure is the test's own; in the parent it is one batch's, and the
            // others still run.
            if let Err(why) = ran {
                if only.is_some() {
                    std::panic::resume_unwind(why);
                }
                let said = why
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| why.downcast_ref::<&str>().map(|s| (*s).to_owned()))
                    .unwrap_or_default();
                red.push(format!("{this}: {said}"));
            }
        }
    }
    assert!(red.is_empty(), "{}", red.join("\n\n"));
}

/// **Whether a transcript line says `sentence`** in what it shows, never in how it is written: the
/// text fields of the line (`text`, `stdout`, `stderr`), so a sentence is not found in a key, a
/// step's name or the JSON's own punctuation.
fn says(entry: &serde_json::Value, sentence: &str) -> bool {
    ["text", "stdout", "stderr"]
        .iter()
        .filter_map(|field| entry.get(field).and_then(serde_json::Value::as_str))
        .any(|text| text.contains(sentence))
}

#[cfg(test)]
mod tests {
    use super::says;
    use serde_json::json;

    #[test]
    fn a_chat_line_says_what_its_text_fields_say_and_never_its_keys_or_json() {
        let line = json!({"kind": "tool_result", "step": "Allowed", "text": "Kept blocked",
                          "stdout": "out", "stderr": ""});
        assert!(says(&line, "Kept blocked"));
        assert!(says(&line, "out"));
        assert!(!says(&line, "Allowed"), "a step's name is not said");
        assert!(!says(&line, "tool_result"), "a key is not said");
        assert!(
            !says(&line, "\"text\""),
            "the JSON's punctuation is not said"
        );
    }
}
