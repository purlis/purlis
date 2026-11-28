// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The core's main thread, watched** (felt speed round 2, H6b). Every command the window
//! sends enters on it, and a synchronous one runs there (`off_the_main_thread`'s costs list),
//! so while anything holds it every other answer waits in line, typing included.
//!
//! Two things go to the slow-answer and stall log (`purlis_core::slowlog`), written on a thread
//! of their own and never on the main thread:
//!
//! - **a core stall**: a ping asked of the main thread every [`EVERY`] that it ran
//!   [`STALLED`] or more late, with the window command it was running when the wait passed
//!   [`STALLED`] (`None` when what held it was not a command: an event, a terminal's output);
//! - **a core command**: a window command that held the main thread [`HELD_LONG`] or more,
//!   with when it was received there. Beside the window's own entry for its answer, asked at
//!   `at` and answered `ms` later, that tells its time in line from its work.

use std::sync::{Mutex, OnceLock, mpsc};
use std::thread::ThreadId;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use purlis_core::slowlog::{Kind, Slow};

/// How often the main thread is pinged.
pub(crate) const EVERY: Duration = Duration::from_millis(100);

/// A ping this late, or later, is a core stall: the window's own budget for a freeze, and the
/// slow log's threshold for a core stall.
pub(crate) const STALLED: Duration =
    Duration::from_millis(Kind::CoreStall.keeping().from_ms as u64);

/// A command that holds the main thread this long, or longer, is written down: the slow log's
/// threshold for a core command.
pub(crate) const HELD_LONG: Duration =
    Duration::from_millis(Kind::CoreCommand.keeping().from_ms as u64);

/// What the main thread is running, outermost first: a window command, a window event, a menu
/// click, and, inside it, the lock it waits on. Empty when nothing of purlis's is running there.
#[derive(Default)]
pub(crate) struct Running(Mutex<Vec<String>>);

impl Running {
    const fn new() -> Self {
        Running(Mutex::new(Vec::new()))
    }

    fn enter(&self, what: String) {
        self.0
            .lock()
            .unwrap_or_else(|held| held.into_inner())
            .push(what);
    }

    fn leave(&self) {
        self.0.lock().unwrap_or_else(|held| held.into_inner()).pop();
    }

    /// What it is running, as one line: `window event: focused > waiting on a lock at
    /// app/src-tauri/src/chats.rs:6963`. A source location, never a path of the person's.
    pub(crate) fn now(&self) -> Option<String> {
        let held = self.0.lock().unwrap_or_else(|held| held.into_inner());
        if held.is_empty() {
            None
        } else {
            Some(held.join(" > "))
        }
    }
}

/// The app's main thread, as its setup found it.
static MAIN: OnceLock<ThreadId> = OnceLock::new();

/// What the app's main thread is running.
static RUNNING: Running = Running::new();

/// Where the main thread's slow moments go to be written.
static SINK: OnceLock<mpsc::Sender<Slow>> = OnceLock::new();

/// **Starts watching the main thread**, from the app's setup, which runs on it: a thread that
/// writes what is seen to the log, and one that pings.
pub(crate) fn start<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    if MAIN.set(std::thread::current().id()).is_err() {
        return;
    }
    let (sink, seen) = mpsc::channel::<Slow>();
    let _ = SINK.set(sink.clone());
    let spawned = std::thread::Builder::new()
        .name("slow log writer".into())
        .spawn(move || {
            for slow in seen {
                if let Err(err) = purlis_core::slowlog::record(&slow) {
                    tracing::debug!("the slow-answer log was not written: {err}");
                }
            }
        });
    if let Err(err) = spawned {
        tracing::warn!("the slow-answer log's writer did not start: {err}");
        return;
    }
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("main thread watch".into())
        .spawn(move || {
            watch(
                |ping| app.run_on_main_thread(ping).is_ok(),
                &RUNNING,
                &sink,
                EVERY,
                STALLED,
            );
        });
    if let Err(err) = spawned {
        tracing::warn!("the main thread's watch did not start: {err}");
    }
}

/// **The app's invoke handler, each command run through [`command`]**: named and timed while
/// it holds the main thread.
pub(crate) fn timed<R: tauri::Runtime>(
    handler: impl Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static,
) -> impl Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static {
    move |invoke| {
        let name = invoke.message.command().to_owned();
        command(&name, || handler(invoke))
    }
}

/// **Runs the window command `command`**, timed and named while it runs when this is the main
/// thread; anywhere else (a test's thread), just runs it.
pub(crate) fn command<T>(command: &str, run: impl FnOnce() -> T) -> T {
    if MAIN.get() != Some(&std::thread::current().id()) {
        return run();
    }
    held(command, &RUNNING, SINK.get(), HELD_LONG, run)
}

/// **A lock taken on the main thread, named when it has to wait**: the stall it causes says
/// `waiting on a lock at <file>:<line>`, the line that asked for it, so the holder is the other
/// place that locks the same mutex. Off the main thread, and when the lock is free, it is a
/// plain `lock`. A poisoned lock is taken over, as everywhere in the app.
#[track_caller]
pub(crate) fn lock<T: ?Sized>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    if MAIN.get() != Some(&std::thread::current().id()) {
        return mutex.lock().unwrap_or_else(|held| held.into_inner());
    }
    lock_in(mutex, std::panic::Location::caller(), &RUNNING, SINK.get())
}

/// [`lock`], naming the wait in `running` and writing it to `sink` as a core command when it
/// took [`HELD_LONG`] or more.
fn lock_in<'a, T: ?Sized>(
    mutex: &'a Mutex<T>,
    at: &std::panic::Location<'_>,
    running: &Running,
    sink: Option<&mpsc::Sender<Slow>>,
) -> std::sync::MutexGuard<'a, T> {
    use std::sync::TryLockError;
    match mutex.try_lock() {
        Ok(guard) => guard,
        Err(TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
        Err(TryLockError::WouldBlock) => held(
            &format!("waiting on a lock at {}:{}", at.file(), at.line()),
            running,
            sink,
            HELD_LONG,
            || mutex.lock().unwrap_or_else(|held| held.into_inner()),
        ),
    }
}

/// The name a window event goes by in a core stall.
pub(crate) fn window_event_name(event: &tauri::WindowEvent) -> &'static str {
    use tauri::WindowEvent as E;
    match event {
        E::Resized(_) => "window event: resized",
        E::Moved(_) => "window event: moved",
        E::CloseRequested { .. } => "window event: close requested",
        E::Destroyed => "window event: destroyed",
        E::Focused(_) => "window event: focused",
        E::ScaleFactorChanged { .. } => "window event: scale factor changed",
        E::DragDrop(_) => "window event: drag drop",
        E::ThemeChanged(_) => "window event: theme changed",
        _ => "window event: other",
    }
}

/// Runs `run` as `command`, named in `running` while it runs, and sends a core command to
/// `sink` when it took `long` or more.
fn held<T>(
    command: &str,
    running: &Running,
    sink: Option<&mpsc::Sender<Slow>>,
    long: Duration,
    run: impl FnOnce() -> T,
) -> T {
    /// Takes the name off again, even when the command panics.
    struct Named<'a>(&'a Running);
    impl Drop for Named<'_> {
        fn drop(&mut self) {
            self.0.leave();
        }
    }

    let at = now_ms();
    let began = Instant::now();
    running.enter(command.to_owned());
    let named = Named(running);
    let answer = run();
    drop(named);
    let took = began.elapsed();
    if took >= long
        && let Some(sink) = sink
    {
        let _ = sink.send(Slow::CoreCommand {
            command: command.to_owned(),
            ms: millis(took),
            at,
        });
    }
    answer
}

/// **Pings the main thread every `every`** through `ping`, which hands it what to run and says
/// whether it could, and sends a core stall to `sink` for each ping that ran `stalled` or
/// more late. Returns once `ping` cannot hand a ping over, or nothing is left to send to.
pub(crate) fn watch(
    ping: impl Fn(Box<dyn FnOnce() + Send>) -> bool,
    running: &Running,
    sink: &mpsc::Sender<Slow>,
    every: Duration,
    stalled: Duration,
) {
    loop {
        let (ran, landed) = mpsc::sync_channel::<()>(1);
        let at = now_ms();
        let asked = Instant::now();
        if !ping(Box::new(move || {
            let _ = ran.send(());
        })) {
            return;
        }
        let mut seen_running = None;
        match landed.recv_timeout(stalled) {
            Ok(()) => {}
            Err(mpsc::RecvTimeoutError::Timeout) => {
                seen_running = running.now();
                if landed.recv().is_err() {
                    return;
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        }
        let late = asked.elapsed();
        if late >= stalled
            && sink
                .send(Slow::CoreStall {
                    ms: millis(late),
                    at,
                    running: seen_running,
                })
                .is_err()
        {
            return;
        }
        std::thread::sleep(every);
    }
}

/// Milliseconds since 1970 by this machine's clock; 0 before then.
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| {
            u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
        })
}

/// `took` in whole milliseconds, at most `u32::MAX` (49 days).
fn millis(took: Duration) -> u32 {
    u32::try_from(took.as_millis()).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests;
