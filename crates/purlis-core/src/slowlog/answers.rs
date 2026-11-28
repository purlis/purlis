// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The core's answers, from their spans** (FS-M1a, #7893): a `tracing` [`Layer`] that reads
//! the app's `answer` spans and nothing else, and writes two kinds of line to the slow log.
//!
//! - **An answer span** ([`Slow::AnswerSpan`]): one answer that took its kind's threshold or
//!   longer in all ([`Kind::AnswerSpan`]), with its command, class, time in line and work.
//! - **An answer summary** ([`Slow::AnswerSummary`]): every answer, the quick ones too, by
//!   command, as a median and a 99th percentile, from a histogram held in memory. Written by the
//!   first answer to close an hour or more after the last summary, and as the app quits
//!   ([`at_exit`]); an hour with no answer writes none.
//!
//! The app opens the span (`answer{command, class}`) where an answer is worked: around a
//! synchronous command on the main thread, and on the blocking thread of the one door for the
//! rest, which records how long the work waited there (`queued_ms`) and how long it took
//! (`work_ms`).
//!
//! **Never an argument** (ADR 0075). The layer reads four fields of the span by name and
//! writes those alone: whatever else a span carries never reaches the log. The window's own
//! moments keep [`super::record`].

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, OnceLock, mpsc};

use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;

use super::{Kind, Slow, Spread};

/// The span's name: what the app opens around each answer.
pub const SPAN: &str = "answer";

/// What the summary holds a command's answers to: how many it is told apart in a millisecond
/// count, two significant digits, so a 99th percentile reads within 1 %.
const DIGITS: u8 = 2;

/// The longest answer the histogram tells apart, an hour; a longer one counts as an hour.
const LONGEST_MS: u64 = 3_600_000;

/// The most commands one summary line holds: the ones that answered most, so 200 summaries
/// of about 100 bytes a command stay near a megabyte in all, however many commands the app has.
pub const MOST_COMMANDS: usize = 50;

/// How long a summary covers before the next answer writes it.
pub const HOUR_MS: u64 = 3_600_000;

/// **Every answer since the last summary**, a histogram of milliseconds per command and class.
pub struct Spreads {
    since: u64,
    /// By command, then class: looked up by the names alone, so an answer to a command seen
    /// before allocates nothing under the lock.
    each: BTreeMap<String, BTreeMap<String, hdrhistogram::Histogram<u64>>>,
}

impl Spreads {
    /// None yet, counted from `since` (milliseconds since 1970).
    pub fn since(since: u64) -> Self {
        Spreads {
            since,
            each: BTreeMap::new(),
        }
    }

    /// One answer to `command` of class `class` that took `ms` in all.
    pub fn add(&mut self, command: &str, class: &str, ms: u32) {
        if !self.each.contains_key(command) {
            self.each.insert(command.to_owned(), BTreeMap::new());
        }
        let classes = self.each.get_mut(command).expect("inserted above");
        if !classes.contains_key(class) {
            let fresh = hdrhistogram::Histogram::new_with_bounds(1, LONGEST_MS, DIGITS)
                .expect("the bounds are fixed and valid");
            classes.insert(class.to_owned(), fresh);
        }
        let held = classes.get_mut(class).expect("inserted above");
        held.saturating_record(u64::from(ms).min(LONGEST_MS));
    }

    /// Whether the summary is due at `now`: an hour or more since the last.
    pub fn due(&self, now: u64) -> bool {
        now.saturating_sub(self.since) >= HOUR_MS
    }

    /// **The summary from `since` to `now`**, and the count started again from `now`; `None`
    /// when no answer closed in it.
    pub fn take(&mut self, now: u64) -> Option<Slow> {
        let since = std::mem::replace(&mut self.since, now);
        let each = std::mem::take(&mut self.each);
        if each.is_empty() {
            return None;
        }
        let ms = |held: &hdrhistogram::Histogram<u64>, at: f64| {
            u32::try_from(held.value_at_quantile(at)).unwrap_or(u32::MAX)
        };
        let mut commands: Vec<Spread> = each
            .into_iter()
            .flat_map(|(command, classes)| {
                classes.into_iter().map(move |(class, held)| Spread {
                    command: command.clone(),
                    class,
                    n: u32::try_from(held.len()).unwrap_or(u32::MAX),
                    p50_ms: ms(&held, 0.5),
                    p99_ms: ms(&held, 0.99),
                })
            })
            .collect();
        if commands.len() > MOST_COMMANDS {
            commands.sort_by_key(|spread| std::cmp::Reverse(spread.n));
            commands.truncate(MOST_COMMANDS);
            commands.sort_by(|a, b| (&a.command, &a.class).cmp(&(&b.command, &b.class)));
        }
        Some(Slow::AnswerSummary {
            at: now,
            since,
            commands,
        })
    }
}

/// Where the layer's lines go, and its clock: the app's writer thread and this machine's time;
/// a test's own.
type Write = Arc<dyn Fn(Slow) + Send + Sync>;
type Clock = Arc<dyn Fn() -> u64 + Send + Sync>;

/// **The layer that reads the `answer` spans** ([module docs](self)).
#[derive(Clone)]
pub struct AnswerSpans {
    spreads: Arc<Mutex<Spreads>>,
    write: Write,
    now: Clock,
}

impl AnswerSpans {
    /// A layer that hands each line to `write`, and reads the time from `now`.
    pub fn writing_to(
        write: impl Fn(Slow) + Send + Sync + 'static,
        now: impl Fn() -> u64 + Send + Sync + 'static,
    ) -> Self {
        let now: Clock = Arc::new(now);
        AnswerSpans {
            spreads: Arc::new(Mutex::new(Spreads::since(now()))),
            write: Arc::new(write),
            now,
        }
    }

    /// **The summary now**, whether or not an hour has passed: what [`at_exit`] writes.
    pub fn summary_now(&self) -> Option<Slow> {
        lock(&self.spreads).take((self.now)())
    }
}

/// The layer the app's log installs: its lines written to [`super::path`] on a thread of its
/// own, never on the thread whose answer closed. Kept, so [`at_exit`] can write the last
/// summary. A second call hands back the same one.
pub fn installed() -> AnswerSpans {
    static INSTALLED: OnceLock<AnswerSpans> = OnceLock::new();
    INSTALLED
        .get_or_init(|| {
            let (lines, written) = mpsc::channel::<Slow>();
            let spawned = std::thread::Builder::new()
                .name("answer log writer".into())
                .spawn(move || {
                    for slow in written {
                        if let Err(err) = super::record(&slow) {
                            tracing::debug!("the slow-answer log was not written: {err}");
                        }
                    }
                });
            // Kept, not logged: this runs before the log is installed, which says it after.
            if let Err(err) = spawned {
                let _ = UNSTARTED.set(err.to_string());
            }
            let layer = AnswerSpans::writing_to(
                move |slow| {
                    let _ = lines.send(slow);
                },
                super::now_ms,
            );
            let _ = KEPT.set(layer.clone());
            layer
        })
        .clone()
}

/// The layer [`installed`] made, once it has.
static KEPT: OnceLock<AnswerSpans> = OnceLock::new();

/// Why [`installed`]'s writer thread did not start, if it did not.
static UNSTARTED: OnceLock<String> = OnceLock::new();

/// **Why the answer log's writer did not start**, when it did not: for the log to say once it
/// is installed, since [`installed`] runs before it is.
pub fn writer_unstarted() -> Option<&'static str> {
    UNSTARTED.get().map(String::as_str)
}

/// **The last summary, written as the app quits**, on the thread that quits: the writer thread
/// may not get to it before the process ends. Nothing when no layer is installed or no answer
/// closed since the last.
///
/// The app calls it from its exit event and from the Windows updater's exit, the two ways out
/// that come after an answer; every `std::process::exit` of the app is at launch, before any.
/// **What is lost:** an answer span's line still queued for the writer thread when the process
/// ends, and the whole last summary on a crash or a force-quit, which run no exit code. A
/// summary is written only by an answer or a quit, so an app left idle or in the background
/// writes none until its next answer.
pub fn at_exit() {
    let Some(layer) = KEPT.get() else {
        return;
    };
    if let Some(summary) = layer.summary_now()
        && let Err(err) = super::record(&summary)
    {
        tracing::debug!("the last answer summary was not written: {err}");
    }
}

/// The four fields the layer reads of an `answer` span, as they were recorded.
#[derive(Debug, Default)]
struct Fields {
    command: Option<String>,
    class: Option<String>,
    queued_ms: Option<u64>,
    work_ms: Option<u64>,
}

impl Visit for Fields {
    fn record_str(&mut self, field: &Field, value: &str) {
        match field.name() {
            "command" => self.command = Some(value.to_owned()),
            "class" => self.class = Some(value.to_owned()),
            _ => {}
        }
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        match field.name() {
            "queued_ms" => self.queued_ms = Some(value),
            "work_ms" => self.work_ms = Some(value),
            _ => {}
        }
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        if let Ok(value) = u64::try_from(value) {
            self.record_u64(field, value);
        }
    }

    // Every other field is passed over: an argument never reaches the log.
    fn record_debug(&mut self, _field: &Field, _value: &dyn std::fmt::Debug) {}
}

impl<S> tracing_subscriber::Layer<S> for AnswerSpans
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        if attrs.metadata().name() != SPAN {
            return;
        }
        let Some(span) = ctx.span(id) else {
            return;
        };
        let mut fields = Fields::default();
        attrs.record(&mut fields);
        span.extensions_mut().insert(fields);
    }

    fn on_record(&self, id: &Id, values: &Record<'_>, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(id) else {
            return;
        };
        if let Some(fields) = span.extensions_mut().get_mut::<Fields>() {
            values.record(fields);
        }
    }

    fn on_close(&self, id: Id, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(&id) else {
            return;
        };
        let Some(fields) = span.extensions_mut().remove::<Fields>() else {
            return;
        };
        // An answer whose work never ended (its thread panicked) has no time to give.
        let (Some(command), Some(class), Some(work_ms)) =
            (fields.command, fields.class, fields.work_ms)
        else {
            return;
        };
        let ms = |ms: u64| u32::try_from(ms).unwrap_or(u32::MAX);
        let (queued_ms, work_ms) = (ms(fields.queued_ms.unwrap_or(0)), ms(work_ms));
        let now = (self.now)();
        let summary = {
            let mut spreads = lock(&self.spreads);
            spreads.add(&command, &class, queued_ms.saturating_add(work_ms));
            spreads.due(now).then(|| spreads.take(now)).flatten()
        };
        if queued_ms.saturating_add(work_ms) >= Kind::AnswerSpan.keeping().from_ms {
            (self.write)(Slow::AnswerSpan {
                command,
                class,
                queued_ms,
                work_ms,
                at: now.saturating_sub(u64::from(queued_ms) + u64::from(work_ms)),
            });
        }
        if let Some(summary) = summary {
            (self.write)(summary);
        }
    }
}

fn lock<T>(held: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    held.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
mod tests;
