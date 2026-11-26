// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **T1: the event log keeps 1,000 events a second, sustained, with L5 inside its budget**
//! (`docs/spec.md`, the budgets of record; FD-9, ADR 0075 §7; #7742).
//!
//! **The load** is what a session host's hook threads ask of the log at the top class: 50 hot
//! chats ([`CHATS`]), each sending its tool hooks on a thread of its own, through the host's own
//! write ([`EventLog::write`]): the line applied under the log's lock, then made durable with the
//! `fsync` outside it, which lines written at once share (group commit, FD-30). Between them
//! they offer [`OFFERED`] events a second for [`SECONDS`] seconds, each chat's lines on a fixed
//! schedule, so a write that falls behind makes the next one late rather than the load lighter.
//! The load runs [`ROUNDS`] times, each into a log of its own, and each budget is held to the
//! median of the rounds (ADR 0086 §1: a CI absolute timing is a median, never one sample).
//!
//! **What is held** ([`judged`]):
//! - **T1**: the events made durable in a round, over the time from its first line's slot to its
//!   last line's durability, are at least [`EVENTS_A_SECOND`] a second. The load offers a quarter more, so a
//!   log that keeps up passes with room and one that falls behind is measured at its own pace.
//! - **L5 inside its budget**: a round's p95 of its writes, from the slot its line was due in to the moment it
//!   is durable, is within [`HOOK_P95`]. A hook is told its line is taken only then (FD-30), so
//!   this is the log's share of every tool hook's time, and L5 can be inside 50 ms only if it is.
//!   The rest of L5, the hook's own command at the scale target, is `scale`'s (`scale.yml`).
//!
//! It is ignored by default, because it runs for half a minute. `stress.yml`'s `event log
//! throughput` step runs it on Linux and fails for a miss (CI absolute: the budget is many times
//! the median, ADR 0086 §1), in the dev profile the `rust` job caches:
//!
//! ```text
//! cargo test -p purlis-core --test it -- \
//!   the_event_log_keeps_a_thousand_events_a_second:: --ignored --nocapture
//! ```
//!
//! **T2 is not measured here.** Its subject is the audit's writer and its group commit (ADR 0075
//! §7), which AU-3 adds and which is not written yet: there is no audit entry to count and no
//! audit commit to time. AU-3 holds that test (ADR 0075 §7), and the budgets table says so.

use std::path::Path;
use std::time::{Duration, Instant};

use purlis_core::eventlog::{ArgsKey, Entry, Log, Recorder};
use purlis_core::hookchannel::EventLog;
use purlis_core::hookwire::{Decision, ToolCall};

/// T1's budget: events made durable a second, sustained.
const EVENTS_A_SECOND: f64 = 1_000.0;

/// L5's budget, the most a hook call may take at p95: the most the log's share of it may.
const HOOK_P95: Duration = Duration::from_millis(50);

/// The hot chats of the top class (ADR 0082 §3), each a hook thread of its own.
const CHATS: u32 = 50;

/// What the load offers, events a second across every chat: a quarter over T1, so a log that
/// keeps up is measured above it.
const OFFERED: f64 = EVENTS_A_SECOND * 1.25;

/// How long the load is sustained in each round.
const SECONDS: u64 = 10;

/// How many times the load runs: each budget is held to the rounds' median.
const ROUNDS: usize = 3;

/// The device the logs are written for.
const DEVICE: &str = "01J9ZZDEVICE0000000000000";

/// What one round of the load measured.
#[derive(Debug, Clone)]
struct Measured {
    /// Events made durable.
    events: usize,
    /// From the first line's slot to the last line's durability.
    elapsed: Duration,
    /// Each write, from its line's slot to its durability.
    writes: Vec<Duration>,
}

impl Measured {
    /// Events made durable a second: 0 for a round that took no time.
    fn events_a_second(&self) -> f64 {
        if self.elapsed.is_zero() {
            0.0
        } else {
            self.events as f64 / self.elapsed.as_secs_f64()
        }
    }

    /// The p95 of its writes: `None` for a round that wrote nothing.
    fn p95(&self) -> Option<Duration> {
        percentile(&self.writes, 0.95)
    }

    /// The round in one line, for the job's log and summary.
    fn said(&self) -> String {
        let ms = |at: Option<Duration>| at.map_or(f64::NAN, |at| at.as_secs_f64() * 1000.0);
        format!(
            "T1 | {} events in {:.2} s | {:.0} events/s | write p95 {:.1} ms | worst {:.1} ms",
            self.events,
            self.elapsed.as_secs_f64(),
            self.events_a_second(),
            ms(self.p95()),
            ms(self.writes.iter().max().copied()),
        )
    }
}

/// The value at `p` (0 to 1) of `all`, by nearest rank: `None` for none.
fn percentile(all: &[Duration], p: f64) -> Option<Duration> {
    let mut sorted = all.to_vec();
    sorted.sort();
    let rank = (p * sorted.len() as f64).ceil() as usize;
    sorted.get(rank.max(1) - 1).copied()
}

/// The middle of `all`, the lower of the two for an even count: `None` for none.
fn median<T: PartialOrd + Copy>(mut all: Vec<T>) -> Option<T> {
    all.sort_by(|a, b| a.partial_cmp(b).expect("no NaN is measured"));
    all.get(all.len().saturating_sub(1) / 2).copied()
}

/// **Each budget the rounds missed at their median, as a line that says by how much**: none for
/// rounds inside both. Rounds that wrote nothing miss both.
fn judged(rounds: &[Measured]) -> Vec<String> {
    let mut missed = Vec::new();
    let rate = median(rounds.iter().map(Measured::events_a_second).collect()).unwrap_or(0.0);
    if rate < EVENTS_A_SECOND {
        missed.push(format!(
            "T1: the event log kept {rate:.0} events/s at the median of {} rounds, under its \
             {EVENTS_A_SECOND:.0}",
            rounds.len()
        ));
    }
    match median(rounds.iter().filter_map(Measured::p95).collect()) {
        Some(at) if at <= HOOK_P95 => {}
        Some(at) => missed.push(format!(
            "L5 at T1's load: a write's p95 was {:.1} ms at the median of {} rounds, over L5's \
             {} ms",
            at.as_secs_f64() * 1000.0,
            rounds.len(),
            HOOK_P95.as_millis()
        )),
        None => missed.push("L5 at T1's load: no write was measured".to_owned()),
    }
    missed
}

/// An event log in `dir`, as a session host opens it.
fn a_log_in(dir: &Path) -> EventLog {
    let dir = dir.to_path_buf();
    EventLog::opening(move || {
        Ok(Recorder::new(
            Log::open(&dir, DEVICE)?,
            ArgsKey::open(&dir)?,
        ))
    })
}

/// One tool hook's line from chat `chat`, its `n`th: a pre hook and its post hook in turn, as a
/// harness sends them, so the writer pairs each call into a duration as it does in a chat.
fn a_tool_line(plane: &Path, chat: u32, n: u64) -> Entry {
    Entry::Tool {
        plane: plane.to_path_buf(),
        call: ToolCall {
            chat,
            tool_hook: if n.is_multiple_of(2) {
                "pretooluse"
            } else {
                "posttooluse"
            }
            .into(),
            tool: Some("Bash".into()),
            call: Some(format!("toolu_{chat}_{}", n / 2)),
            args: Some(format!("{:064x}", u64::from(chat) << 32 | n)),
            decision: Decision::None,
            rule: None,
            arm: None,
            hook_ms: 3,
            agent: None,
            at_ms: 0,
            skill: None,
        },
    }
}

/// Runs one round of the load against a new log in `dir`.
fn a_round(dir: &Path) -> Measured {
    let log = a_log_in(&dir.join("events"));
    let plane = dir.join("plane");
    // The time between one chat's slots: every chat sends at the same pace.
    let slot_gap = Duration::from_secs_f64(f64::from(CHATS) / OFFERED);
    let lines = (OFFERED * SECONDS as f64 / f64::from(CHATS)) as u64;
    // Every chat's first line is written before the clock starts, so a run's `run.started` is
    // not in what is timed: the load is the tool hooks of chats already running.
    for chat in 1..=CHATS {
        log.write(a_tool_line(&plane, chat, 0))
            .expect("the log takes a chat's first line");
    }
    let start = Instant::now();
    let threads: Vec<_> = (1..=CHATS)
        .map(|chat| {
            let log = log.clone();
            let plane = plane.clone();
            // The chats' slots are spread across `slot_gap`, as hooks arrive, not in one burst.
            let offset = slot_gap.mul_f64(f64::from(chat - 1) / f64::from(CHATS));
            std::thread::spawn(move || {
                let mut writes = Vec::with_capacity(lines as usize);
                for n in 1..=lines {
                    let due = start + offset + slot_gap * (n - 1) as u32;
                    if let Some(wait) = due.checked_duration_since(Instant::now()) {
                        std::thread::sleep(wait);
                    }
                    log.write(a_tool_line(&plane, chat, n))
                        .expect("the log takes a tool line");
                    writes.push(due.elapsed());
                }
                writes
            })
        })
        .collect();
    let writes: Vec<Duration> = threads
        .into_iter()
        .flat_map(|thread| thread.join().expect("a chat's hook thread"))
        .collect();
    Measured {
        events: writes.len(),
        elapsed: start.elapsed(),
        writes,
    }
}

#[test]
#[ignore = "runs for half a minute: stress.yml's `event log throughput` step runs it"]
fn the_log_keeps_a_thousand_events_a_second_with_each_write_inside_l5() {
    let rounds: Vec<Measured> = (0..ROUNDS)
        .map(|_| {
            let dir = tempfile::tempdir().unwrap();
            let round = a_round(dir.path());
            println!("{}", round.said());
            round
        })
        .collect();
    let missed = judged(&rounds);
    assert!(missed.is_empty(), "{}", missed.join("\n"));
}

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

/// A round of `events` in ten seconds, its writes each `writes`.
fn a_round_of(events: usize, writes: Vec<Duration>) -> Measured {
    Measured {
        events,
        elapsed: Duration::from_secs(10),
        writes,
    }
}

#[test]
fn rounds_inside_both_budgets_miss_nothing() {
    let rounds = vec![a_round_of(12_500, vec![ms(2); 100]); 3];
    assert_eq!(judged(&rounds), Vec::<String>::new());
}

#[test]
fn a_log_that_falls_behind_misses_t1_by_its_median_rate() {
    let rounds = [
        a_round_of(12_500, vec![ms(2); 100]),
        a_round_of(8_000, vec![ms(2); 100]),
        a_round_of(9_000, vec![ms(2); 100]),
    ];
    assert_eq!(
        judged(&rounds),
        ["T1: the event log kept 900 events/s at the median of 3 rounds, under its 1000"]
    );
}

#[test]
fn one_slow_round_of_three_misses_nothing() {
    let rounds = [
        a_round_of(12_500, vec![ms(2); 100]),
        a_round_of(4_000, vec![ms(90); 100]),
        a_round_of(12_400, vec![ms(3); 100]),
    ];
    assert_eq!(judged(&rounds), Vec::<String>::new());
}

#[test]
fn six_slow_writes_in_a_hundred_miss_l5_and_five_do_not() {
    // Six slow writes in a hundred: the 95th is one of them.
    let mut six = vec![ms(2); 94];
    six.extend([ms(80); 6]);
    assert_eq!(
        judged(&vec![a_round_of(12_500, six); 3]),
        ["L5 at T1's load: a write's p95 was 80.0 ms at the median of 3 rounds, over L5's 50 ms"]
    );
    // Five in a hundred: the 95th is the last fast one.
    let mut five = vec![ms(2); 95];
    five.extend([ms(80); 5]);
    assert_eq!(
        judged(&vec![a_round_of(12_500, five); 3]),
        Vec::<String>::new()
    );
}

#[test]
fn rounds_that_wrote_nothing_miss_both() {
    let nothing = Measured {
        events: 0,
        elapsed: Duration::ZERO,
        writes: Vec::new(),
    };
    assert_eq!(
        judged(&vec![nothing; 3]),
        [
            "T1: the event log kept 0 events/s at the median of 3 rounds, under its 1000",
            "L5 at T1's load: no write was measured",
        ]
    );
}
