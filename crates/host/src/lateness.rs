// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What a host says in its log when it was slow** (#6437). A window gave up on a host's answer
//! to a Close after 30 s on a loaded macOS runner, and the host's log said nothing of it, so
//! nobody could tell what had held it. Two things can hold an answer, and the host now says
//! which:
//!
//! - **The call's lane, or the call itself.** A call keeps its order behind the earlier calls of
//!   its lane (a chat's commands, one after another), and then takes its own time. An answer
//!   [`SAID_FROM`] or more after its call came is said, with how much of it was each
//!   ([`answered_late`]).
//! - **The host's one thread.** A host serves on a runtime of one thread
//!   (`crate::run`), so anything that blocks it holds up every link. A look made every
//!   [`LOOKS_EVERY`] that comes [`SAID_FROM`] or more late is said ([`watch_the_thread`]).
//!
//! The lines around a held thread named nothing (queue run 43516561795: held 24,558 ms while a
//! chat started, and no answer said late), so on macOS a host asked to by [`SAMPLE_WHEN_HELD`]
//! says what its thread is doing while it is held: a thread of its own sees the looks stop, and
//! once a hold reaches [`SAMPLE_FROM`] it has `/usr/bin/sample`, Apple's own tool for this, take
//! the host's stacks for a second and says its thread's ([`sample_when_held`]). The scenario runs
//! ask it; a host on a person's machine never samples itself.
//!
//! The host's log is its standard error, which the window that started it keeps in its own log
//! (`sessionhost.rs`), and a scenario run keeps the window's log when it fails.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// How late an answer, or a look at the host's thread, is before it is said.
pub const SAID_FROM: Duration = Duration::from_secs(1);

/// How often the host's thread is looked at.
pub const LOOKS_EVERY: Duration = Duration::from_millis(250);

/// What the host says of call `word`, answered `waited` behind its lane's earlier calls and
/// `ran` carrying it out: nothing for one answered within [`SAID_FROM`] of coming.
pub fn answered_late(word: &str, waited: Duration, ran: Duration) -> Option<String> {
    let took = waited + ran;
    (took >= SAID_FROM).then(|| {
        format!(
            "a `{word}` was answered {} ms after it came: {} ms behind the earlier calls of its \
             lane, then {} ms carrying it out",
            took.as_millis(),
            waited.as_millis(),
            ran.as_millis()
        )
    })
}

/// Says [`answered_late`]'s line in this host's log, where it has one: what a host's
/// `session::Host::answered` does.
pub fn say_if_late(word: &str, waited: Duration, ran: Duration) {
    if let Some(said) = answered_late(word, waited, ran) {
        crate::say(&said);
    }
}

/// What the host says of a look at its thread that came `late` past when it was due: nothing
/// for one under [`SAID_FROM`].
pub fn thread_held(late: Duration) -> Option<String> {
    (late >= SAID_FROM).then(|| {
        format!(
            "this host's thread was held {} ms: nothing it serves was answered meanwhile",
            late.as_millis()
        )
    })
}

/// When the host's thread last looked, for a thread outside it to read: what
/// [`watch_the_thread`] writes and [`sample_when_held`] reads.
#[derive(Debug)]
pub struct Beat {
    since: Instant,
    last_ms: AtomicU64,
}

impl Beat {
    /// A beat that has just looked.
    pub fn new() -> Arc<Beat> {
        Arc::new(Beat {
            since: Instant::now(),
            last_ms: AtomicU64::new(0),
        })
    }

    fn now_ms(&self) -> u64 {
        u64::try_from(self.since.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    fn looked(&self) {
        self.last_ms.store(self.now_ms(), Ordering::Relaxed);
    }
}

/// Looks at the host's thread every [`LOOKS_EVERY`] for as long as the runtime runs, saying
/// each look that came late ([`thread_held`]), and beating `beat` at each.
pub async fn watch_the_thread(beat: Arc<Beat>) {
    loop {
        beat.looked();
        let due = Instant::now() + LOOKS_EVERY;
        tokio::time::sleep_until(due.into()).await;
        if let Some(said) = thread_held(due.elapsed()) {
            crate::say(&said);
        }
    }
}

/// The environment variable that asks a host to sample its own thread when it is held: set, to
/// anything, by the scenario runs (`app/e2e/wdio.conf.ts`), whose window passes it on.
pub const SAMPLE_WHEN_HELD: &str = "PURLIS_HOST_SAMPLES_A_HELD_THREAD";

/// How long a hold lasts before it is sampled: past [`SAID_FROM`], so a hold that is sampled is
/// also said.
pub const SAMPLE_FROM: Duration = Duration::from_secs(2);

/// How many holds one host samples at most: each costs a second of `sample`, and the first
/// few say what holds it.
pub const SAMPLES_AT_MOST: usize = 3;

/// Which holds of the host's thread to sample: one sample a hold, once it has lasted
/// [`SAMPLE_FROM`], and no more than [`SAMPLES_AT_MOST`].
#[derive(Debug, Default)]
pub struct Sampling {
    sampled_beat: Option<u64>,
    taken: usize,
}

impl Sampling {
    /// Whether to sample now, the thread having last looked at `beat_ms` and it being `now_ms`.
    pub fn now(&mut self, beat_ms: u64, now_ms: u64) -> bool {
        let held = Duration::from_millis(now_ms.saturating_sub(beat_ms));
        if held < SAMPLE_FROM || self.sampled_beat == Some(beat_ms) || self.taken >= SAMPLES_AT_MOST
        {
            return false;
        }
        self.sampled_beat = Some(beat_ms);
        self.taken += 1;
        true
    }
}

/// How many lines of the held thread's stacks are said at most.
pub const LINES_AT_MOST: usize = 200;

/// How long one said line of a stack may be: a frame's signature can run to a thousand.
const LINE_AT_MOST: usize = 240;

/// The call graph of the process's main thread, where the host's runtime runs, from what
/// `/usr/bin/sample` wrote: the lines from its heading to the next thread's, each cut to
/// [`LINE_AT_MOST`] characters, at most [`LINES_AT_MOST`] of them. Empty when it names none.
pub fn main_thread_of(sampled: &str) -> Vec<String> {
    let is_a_thread = |line: &str| {
        let words: Vec<&str> = line.split_whitespace().take(2).collect();
        words.len() == 2
            && words[0].chars().all(|c| c.is_ascii_digit())
            && words[1].starts_with("Thread_")
    };
    let mut lines = sampled
        .lines()
        .skip_while(|line| !(is_a_thread(line) && line.contains("com.apple.main-thread")));
    let Some(heading) = lines.next() else {
        return Vec::new();
    };
    std::iter::once(heading)
        .chain(lines.take_while(|line| line.starts_with("    +")))
        .take(LINES_AT_MOST)
        .map(|line| line.chars().take(LINE_AT_MOST).collect())
        .collect()
}

/// When asked by [`SAMPLE_WHEN_HELD`], on macOS, starts the thread that samples this host's
/// thread while it is held, as `beat` says: each hold [`Sampling`] picks is said with
/// `/usr/bin/sample`'s call graph of the main thread ([`main_thread_of`]). Nothing elsewhere.
pub fn sample_when_held(beat: Arc<Beat>) {
    if std::env::var_os(SAMPLE_WHEN_HELD).is_none() || !cfg!(target_os = "macos") {
        return;
    }
    let started = std::thread::Builder::new()
        .name("purlisd-held".into())
        .spawn(move || {
            let mut sampling = Sampling::default();
            loop {
                std::thread::sleep(LOOKS_EVERY);
                let beat_ms = beat.last_ms.load(Ordering::Relaxed);
                let now_ms = beat.now_ms();
                if sampling.now(beat_ms, now_ms) {
                    say_the_sample(now_ms.saturating_sub(beat_ms));
                }
            }
        });
    if let Err(why) = started {
        crate::say(&format!("a held thread will not be sampled: {why}"));
    }
}

/// Samples this host for a second, its thread having been held `held_ms`, and says its main
/// thread's call graph, a line at a time.
fn say_the_sample(held_ms: u64) {
    let into = match tempfile::Builder::new().prefix("purlisd-held").tempfile() {
        Ok(file) => file,
        Err(why) => {
            crate::say(&format!("a held thread was not sampled: {why}"));
            return;
        }
    };
    let ran = std::process::Command::new("/usr/bin/sample")
        .arg(std::process::id().to_string())
        .arg("1")
        .arg("-file")
        .arg(into.path())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .output();
    let said = match ran {
        Ok(ran) if ran.status.success() => {
            std::fs::read_to_string(into.path()).map_err(|why| why.to_string())
        }
        Ok(ran) => Err(format!(
            "/usr/bin/sample ended {}: {}",
            ran.status,
            String::from_utf8_lossy(&ran.stderr).trim()
        )),
        Err(why) => Err(format!("/usr/bin/sample did not run: {why}")),
    };
    match said.map(|sampled| main_thread_of(&sampled)) {
        Ok(lines) if !lines.is_empty() => {
            crate::say(&format!(
                "this host's thread has been held {held_ms} ms; what it is doing, by /usr/bin/sample:"
            ));
            for line in lines {
                crate::say(&format!("  {line}"));
            }
        }
        Ok(_) => crate::say("a held thread was sampled, and the sample named no main thread"),
        Err(why) => crate::say(&format!("a held thread was not sampled: {why}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_answer_that_came_late_says_how_much_its_lane_held_it_and_how_much_it_took_itself() {
        assert_eq!(
            answered_late(
                "close",
                Duration::from_millis(29_800),
                Duration::from_millis(107)
            )
            .as_deref(),
            Some(
                "a `close` was answered 29907 ms after it came: 29800 ms behind the earlier calls \
                 of its lane, then 107 ms carrying it out"
            )
        );
    }

    #[test]
    fn an_answer_within_a_second_of_its_call_is_not_said() {
        assert_eq!(
            answered_late(
                "write",
                Duration::from_millis(400),
                Duration::from_millis(599)
            ),
            None
        );
        assert!(
            answered_late(
                "write",
                Duration::from_millis(400),
                Duration::from_millis(600)
            )
            .is_some()
        );
    }

    #[test]
    fn a_hold_is_sampled_once_it_has_lasted_two_seconds_and_once_only() {
        let mut sampling = Sampling::default();
        assert!(!sampling.now(1_000, 2_999));
        assert!(sampling.now(1_000, 3_000));
        assert!(!sampling.now(1_000, 3_250));
        assert!(!sampling.now(1_000, 30_000));
    }

    #[test]
    fn no_more_than_three_holds_are_sampled() {
        let mut sampling = Sampling::default();
        let sampled: Vec<bool> = (0..5)
            .map(|hold| sampling.now(hold * 10_000, hold * 10_000 + 2_500))
            .collect();
        assert_eq!(sampled, [true, true, true, false, false]);
    }

    const SAMPLED: &str = "Analysis of sampling purlis-app (pid 16370) every 1 millisecond
Process:         purlis-app [16370]

Call graph:
    812 Thread_6380   DispatchQueue_1: com.apple.main-thread  (serial)
    + 812 start  (in dyld) + 7184  [0x197799d54]
    +   812 purlis_host::run  (in purlis-app) + 60  [0x102a26344]
    +     812 std::sync::mutex::Mutex<T>::lock  (in purlis-app) + 12  [0x102a26350]
    +       812 __psynch_mutexwait  (in libsystem_kernel.dylib) + 8  [0x1977d1234]
    812 Thread_6381   tokio-runtime-worker
    + 812 thread_start  (in libsystem_pthread.dylib) + 8  [0x197b0a0fc]

Total number in stack (recursive counted multiple, when >=5):
";

    #[test]
    fn the_main_threads_call_graph_is_read_from_a_sample_and_no_other_threads() {
        assert_eq!(
            main_thread_of(SAMPLED),
            [
                "    812 Thread_6380   DispatchQueue_1: com.apple.main-thread  (serial)",
                "    + 812 start  (in dyld) + 7184  [0x197799d54]",
                "    +   812 purlis_host::run  (in purlis-app) + 60  [0x102a26344]",
                "    +     812 std::sync::mutex::Mutex<T>::lock  (in purlis-app) + 12  [0x102a26350]",
                "    +       812 __psynch_mutexwait  (in libsystem_kernel.dylib) + 8  [0x1977d1234]",
            ]
        );
    }

    #[test]
    fn a_sample_naming_no_main_thread_gives_nothing_and_a_long_line_is_cut() {
        assert!(main_thread_of("Call graph:\n    3 Thread_1   worker\n").is_empty());
        let long = format!(
            "    1 Thread_1   DispatchQueue_1: com.apple.main-thread  (serial)\n    + 1 {}\n",
            "x".repeat(1_000)
        );
        let read = main_thread_of(&long);
        assert_eq!(read.len(), 2);
        assert_eq!(read[1].chars().count(), LINE_AT_MOST);
    }

    #[test]
    fn a_look_at_the_hosts_thread_a_second_late_is_said_and_one_sooner_is_not() {
        assert_eq!(thread_held(Duration::from_millis(999)), None);
        assert_eq!(
            thread_held(Duration::from_millis(29_907)).as_deref(),
            Some("this host's thread was held 29907 ms: nothing it serves was answered meanwhile")
        );
    }
}
