// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The kill switch as the app holds it (OV-1, ADR 0071): one switch for every plane and every
//! window this process has, and the watch that hears `charter stop --all` throw it.
//!
//! **While the app runs, the switch in its memory is the authority.** The files on disk
//! (`purlis_core::halt`) are how another process throws it and how the next launch learns it
//! was thrown. They can stop this process but never re-arm it: a stop on disk is in force the
//! moment it is written, and a marker that goes away while agents are stopped is put back and
//! journaled as a tamper. Only [`KillSwitch::rearm`], which the window's control alone calls,
//! lets chats start again.
//!
//! **The residual.** A process running as the operator's user can rewrite both files, and
//! one that forges a re-arm in the journal and removes the marker while the app is not running
//! is believed at the next launch. ADR 0071 says what closes that and where.
//!
//! **When chats run in the session host, the host holds the switch too** (#2001, ADR 0071 as
//! amended 2026-10-14): it is the authority over every program it runs, and keeps the stop
//! across its own restart in a folder no chat can reach. This switch is then its client
//! ([`Authority`]): every stop it throws or hears is the host's too, a re-arm is the host's
//! first and the files' after, so one the host did not take leaves every agent stopped, and an
//! app that connects to a host already stopped holds every agent stopped with it
//! ([`KillSwitch::agree_with_its_host`]). With chats in the window, there is no host to hold it,
//! and nothing here changes.
//!
//! **Every journal line is recorded in the host's event log** (#1375, ADR 0075's "ADR 0071,
//! amended"), once the switch is told where ([`KillSwitch::record_into`]): what the window and
//! the app write themselves, from what they wrote, and every other line as the watch reads it.
//! **Recording never holds the switch up**: it happens after the switch has acted, on a thread
//! of its own, and nothing the switch does waits on it or learns whether it worked.
//!
//! **What it stops is every session charter started**, shell tabs included: a harness can be
//! started by hand in a shell (ADR 0062), and leaving those running would stop only the agents
//! charter happened to know about. **What it refuses until re-armed is every chat and harness
//! start**, from whatever asks — the operator's new chat, a relaunch putting a record back, a
//! handoff, a curation action — at [`crate::host::SessionHost::open`]. **A shell the operator
//! opens from the window is let through**: looking at what the agents did is a human act, and
//! the switch is there to stop agents, not to lock the operator out of their own machine.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, mpsc};
use std::time::Duration;

use notify::RecursiveMode;
use purlis_core::halt::{self, Actor};

/// The event every window is sent when the switch moves, carrying whether agents are stopped.
pub(crate) const CHANGED: &str = "kill-switch";

/// How long a burst of writes to charter's directory is folded for. A stop is a marker and a
/// journal line, two renames; this makes them one look at the switch.
const FOLDED_FOR: Duration = Duration::from_millis(100);

/// The most changed paths one burst holds; past it the burst is a look at the switch whatever
/// it named.
const MOST_PATHS: usize = 256;

/// How often the watch looks at the switch's files when nothing has told it to (D-88l): a watch
/// the platform has quietly lost then makes a stop slower to arrive, never lost.
const LOOK_EVERY: Duration = Duration::from_secs(3);

/// How long a test waits for the watch to say a stop was heard before calling it lost (#1006).
///
/// Derived from [`LOOK_EVERY`], not picked: whatever the platform's file events do — macOS's
/// arrive late on a busy machine — the watch looks at the files once per period of quiet, so a
/// stop is heard within a period of being written. Ten periods is a stop the watch missed, never
/// a machine that was only slow; a test that passes still passes as soon as it hears.
#[cfg(test)]
pub(crate) const HEARD_WITHIN: Duration = LOOK_EVERY.saturating_mul(10);

/// What a look at the files did to the switch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Moved {
    /// Nothing: the files agree with the switch.
    Nothing,
    /// Another process stopped every agent; this one now has too.
    Stopped,
    /// The marker went away while agents were stopped. It was put back and journaled.
    Tampered,
}

/// The session host's hold on the switch, when chats run in it (#2001): it stops every program
/// it runs, keeps the stop across its restart, and re-arms only for the window.
pub trait Authority: Send + Sync {
    /// Every agent stops: every program the host runs ends, and none starts. `Ok` once the
    /// host holds the stop, with what it had to say of it (a stop it could not keep across its
    /// restart); `Err` when it was not told, or does not hold it.
    fn stop(&self) -> Result<Option<String>, String>;
    /// Programs may start again: the window's link alone may ask.
    fn rearm(&self) -> Result<(), String>;
    /// Whether the host holds every agent stopped.
    fn stopped(&self) -> Result<bool, String>;
    /// **The runners the host's stop stopped, which its re-arm re-arms** (#2051), each
    /// `<pool>/<runner>`: none for a host that reaches no runner.
    fn runners(&self) -> Result<Vec<String>, String> {
        Ok(Vec::new())
    }
}

/// Whether this machine's agents are stopped, as this process holds it.
pub struct KillSwitch {
    /// The machine's config home, where the files are. None on a machine with none: the switch
    /// still works, for as long as this process runs.
    config: Option<PathBuf>,
    /// The authority while this process runs. Held across every change so a re-arm and a look
    /// at the files can never interleave — the re-arm's own removal of the marker would
    /// otherwise read as a tamper.
    stopped: Mutex<bool>,
    /// Where each journal line goes to be recorded, once [`Self::record_into`] said.
    recording: Mutex<Option<mpsc::Sender<Noted>>>,
    /// The session host that holds the switch too, when chats run in it.
    host: std::sync::OnceLock<Arc<dyn Authority>>,
    /// A stop the session host has not taken yet: sent again at every look and at the next
    /// [`Self::agree_with_its_host`], until it is.
    owes_the_host: std::sync::atomic::AtomicBool,
}

/// What the recording thread is told: read the journal, or record a line the switch wrote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Noted {
    Read,
    Wrote(halt::Entry),
}

impl KillSwitch {
    /// The switch kept under `config`: stopped already if the files say so.
    pub fn kept_in(config: Option<PathBuf>) -> Arc<Self> {
        let stopped = config.as_deref().is_some_and(halt::stopped_on_disk);
        Arc::new(Self {
            config,
            stopped: Mutex::new(stopped),
            recording: Mutex::new(None),
            host: std::sync::OnceLock::new(),
            owes_the_host: std::sync::atomic::AtomicBool::new(false),
        })
    }

    /// A switch nobody has thrown, kept nowhere: what a `Sessions` a test builds is given.
    pub fn unthrown() -> Arc<Self> {
        Self::kept_in(None)
    }

    /// Whether agents are stopped: by this process, or by the files. A stop on disk counts the
    /// moment it is written, before the watch has seen it; files saying otherwise never count.
    pub fn is_stopped(&self) -> bool {
        *self.held() || self.config.as_deref().is_some_and(halt::stopped_on_disk)
    }

    /// Records each journal line into `events` from now on (#1375), on a thread of its own,
    /// starting with the journal as it is now. What the switch writes itself is recorded from
    /// what it wrote, whatever the file says after; the rest as the watch reads it. Nothing
    /// where there is no config home, which has no journal.
    pub fn record_into(&self, events: crate::hooks::Events) {
        let Some(config) = self.config.clone() else {
            return;
        };
        let (tell, told) = mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("purlis-kill-switch-record".into())
            .spawn(move || record(&events, &config, &told));
        match spawned {
            Ok(_) => {
                let _ = tell.send(Noted::Read);
                *self
                    .recording
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner) = Some(tell);
            }
            Err(why) => tracing::warn!(
                "purlis: the kill switch's journal will not be recorded in the event log ({why})"
            ),
        }
    }

    /// Tells the recording thread `noted`, once the switch has acted. Never waits: the channel
    /// is unbounded, and a thread that is gone is a record lost, never a switch held.
    fn noted(&self, noted: Noted) {
        if let Some(tell) = self
            .recording
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
        {
            let _ = tell.send(noted);
        }
    }

    /// The session host holds the switch too from now on: chats run in it (#2001). Set once,
    /// as the app starts.
    pub fn held_by(&self, host: Arc<dyn Authority>) {
        let _ = self.host.set(host);
    }

    /// Stops every agent. Stopped in this process whatever happens on disk or in the session
    /// host; the error is the sentence saying the stop was not kept, for the operator to see.
    pub fn stop(&self, by: Actor) -> Result<(), String> {
        let kept = self.stop_here(by);
        let told = self.tell_the_host();
        kept.and(told)
    }

    fn stop_here(&self, by: Actor) -> Result<(), String> {
        let mut stopped = self.held();
        *stopped = true;
        let Some(config) = &self.config else {
            return Err(
                "every agent is stopped in this app, but there is no config home to keep the \
                 stop in: a relaunch will start chats again"
                    .to_owned(),
            );
        };
        let at = halt::now();
        let kept = halt::stop(config, by, at)
            .map_err(|not_kept| format!("every agent is stopped in this app, but {not_kept}"));
        self.noted(Noted::Wrote(halt::Entry {
            at,
            event: halt::Event::Stop,
            by,
        }));
        kept
    }

    /// The session host stops every agent too, when chats run in it. Never under the switch's
    /// lock: the host's answer can take its time.
    fn tell_the_host(&self) -> Result<(), String> {
        let Some(host) = self.host.get() else {
            return Ok(());
        };
        let told = host.stop();
        self.owes_the_host
            .store(told.is_err(), std::sync::atomic::Ordering::SeqCst);
        if let Ok(Some(said)) = &told {
            return Err(format!(
                "every agent is stopped, here and in the session host, but the session host \
                 said: {said}"
            ));
        }
        told.map(|_| ()).map_err(|why| {
            tracing::warn!(
                "purlis: the session host's stop is not as it should be, and is sent again at the \
                 next look ({why})"
            );
            format!(
                "every agent is stopped in this app, but the session host's stop is not as it \
                 should be: {why}"
            )
        })
    }

    /// The runners the session host's stop stopped, which a re-arm re-arms with it (#2051).
    pub fn runners_a_rearm_reaches(&self) -> Vec<String> {
        self.host
            .get()
            .and_then(|host| host.runners().ok())
            .unwrap_or_default()
    }

    /// Sends the session host again a stop it did not take.
    fn pay_what_is_owed(&self) {
        if self.owes_the_host.load(std::sync::atomic::Ordering::SeqCst) && self.is_stopped() {
            let _ = self.tell_the_host();
        }
    }

    /// Brings this switch and the session host's into line once the app reaches its host, in
    /// the one direction either may move: towards stopped. A stop this app holds is the host's
    /// too; a host that holds every agent stopped stops this app. Answers whether this switch
    /// moved, for the windows to be told. Nothing where chats do not run in a host.
    pub fn agree_with_its_host(&self) -> bool {
        let Some(host) = self.host.get() else {
            return false;
        };
        if self.is_stopped() {
            let _ = self.tell_the_host();
            return false;
        }
        match host.stopped() {
            Ok(true) => {
                *self.held() = true;
                true
            }
            Ok(false) => false,
            Err(why) => {
                tracing::warn!(
                    "purlis: the session host did not say whether agents are stopped ({why})"
                );
                false
            }
        }
    }

    /// Lets chats start again: the window's alone. Answers whether agents had been stopped. A
    /// re-arm the session host or the files could not take is refused, and agents stay stopped:
    /// the host's goes first, so the files never say re-armed while the host holds the stop.
    pub fn rearm(&self) -> Result<bool, String> {
        if let Some(host) = self.host.get() {
            host.rearm().map_err(|why| {
                format!("agents are still stopped: the session host did not re-arm ({why})")
            })?;
        }
        let mut stopped = self.held();
        let was = *stopped || self.config.as_deref().is_some_and(halt::stopped_on_disk);
        if let Some(config) = &self.config {
            let at = halt::now();
            if let Err(why) = halt::rearm(config, at) {
                drop(stopped);
                // The host took the re-arm and the files did not: the host stops again, so the
                // two never disagree towards running; a stop it does not take now is owed, and
                // sent again at the next look.
                let not_kept = format!("agents are still stopped: the re-arm was not kept ({why})");
                return Err(match self.tell_the_host() {
                    Ok(()) => not_kept,
                    Err(told) => format!("{not_kept}; {told}"),
                });
            }
            self.noted(Noted::Wrote(halt::Entry {
                at,
                event: halt::Event::Rearm,
                by: Actor::Window,
            }));
        }
        *stopped = false;
        Ok(was)
    }

    /// Looks at the files and brings the switch in line with them, in the one direction it may
    /// move: towards stopped.
    pub fn look(&self) -> Moved {
        self.pay_what_is_owed();
        let Some(config) = &self.config else {
            return Moved::Nothing;
        };
        let mut stopped = self.held();
        if *stopped {
            if halt::marker_present(config) {
                return Moved::Nothing;
            }
            let at = halt::now();
            if let Err(why) = halt::restore(config, at) {
                tracing::warn!(
                    "purlis: the kill switch's marker was removed and could not be put back ({why})"
                );
            }
            // Recorded whether or not either file took it (#1375).
            self.noted(Noted::Wrote(halt::Entry {
                at,
                event: halt::Event::Tamper,
                by: Actor::App,
            }));
            return Moved::Tampered;
        }
        if halt::stopped_on_disk(config) {
            *stopped = true;
            drop(stopped);
            self.noted(Noted::Read);
            // `purlis stop --all` is heard here, and reaches the session host's programs too.
            let _ = self.tell_the_host();
            return Moved::Stopped;
        }
        Moved::Nothing
    }

    /// The directory the files are in, for the watch.
    fn directory(&self) -> Option<PathBuf> {
        self.config
            .as_deref()
            .and_then(|config| halt::directory(config).ok())
    }

    fn held(&self) -> MutexGuard<'_, bool> {
        self.stopped.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// The recording thread: each line the switch wrote, and the journal each time it is to be
/// read, into `events`, until the switch is gone. Reads asked for together are one read.
fn record(events: &crate::hooks::Events, config: &Path, told: &mpsc::Receiver<Noted>) {
    while let Ok(first) = told.recv() {
        let mut wrote = Vec::new();
        let mut read = false;
        for noted in std::iter::once(first).chain(told.try_iter()) {
            match noted {
                Noted::Read => read = true,
                Noted::Wrote(entry) => wrote.push(entry),
            }
        }
        // The session host writes the log while chats run in it (#2003): each line and each
        // read goes to it, and it reads the journal from the same machine store.
        let Some(log) = events.held() else {
            for line in wrote {
                let line = purlis_core::eventlog::Entry::KillSwitchWrote { line, device: None };
                if let Err(why) = events.record(line) {
                    tracing::warn!(
                        "purlis: a kill-switch line was not recorded in the event log ({why})"
                    );
                }
            }
            if read && let Err(why) = events.record(purlis_core::eventlog::Entry::KillSwitchRead) {
                tracing::warn!(
                    "purlis: the kill switch's journal was not recorded in the event log ({why})"
                );
            }
            continue;
        };
        let mut recorder = log.lock().unwrap_or_else(PoisonError::into_inner);
        // Each on its own, so one that fails never keeps another from being recorded.
        for entry in wrote {
            if let Err(why) = recorder.kill_switch_wrote(entry, config) {
                tracing::warn!(
                    "purlis: a kill-switch line was not recorded in the event log ({why})"
                );
            }
        }
        if read && let Err(why) = recorder.kill_switch_read(config) {
            tracing::warn!(
                "purlis: the kill switch's journal was not recorded in the event log ({why})"
            );
        }
    }
}

/// A watch on the switch's files. Dropping it stops it.
pub struct Watch<W: notify::Watcher = notify::RecommendedWatcher> {
    /// Shared with the watch's thread only weakly, so it can watch the directory again; dropped
    /// here, it ends that thread.
    _watcher: Arc<Mutex<W>>,
}

/// Watches charter's directory, non-recursively, and calls `moved` after every batch of writes
/// to the switch's files with what [`KillSwitch::look`] made of them — and once at the start,
/// for a stop written while the app was coming up. None where there is no directory to watch.
pub fn watch(
    switch: Arc<KillSwitch>,
    moved: impl Fn(Moved) + Send + 'static,
) -> Option<notify::Result<Watch>> {
    let dir = switch.directory()?;
    Some(watch_in(&dir, switch, moved))
}

/// [`watch`], in `dir`, on a watcher of the caller's choosing.
///
/// **A marker made and taken away inside one burst is still looked at** (#1139): every burst
/// that names one of the switch's files is a look, whatever its events added up to. The
/// debouncer this used to fold through took a removal that arrived while the creation was
/// still queued as the file never having been there.
fn watch_in<W: notify::Watcher + Send + 'static>(
    dir: &Path,
    switch: Arc<KillSwitch>,
    moved: impl Fn(Moved) + Send + 'static,
) -> notify::Result<Watch<W>> {
    watch_in_every(dir, switch, moved, LOOK_EVERY)
}

/// [`watch_in`], looking at the files every `every` that passes with no event (D-88l, ADR 0071).
///
/// **A dead watch is a slower stop, not none.** A watcher can stop hearing without saying so:
/// charter's directory removed and made again takes its inotify watch with it. So when nothing
/// arrives for `every`, the switch is looked at anyway, and the directory is watched again
/// whenever the platform said it was removed or renamed, a burst was everything, or it is no
/// longer the directory that was watched.
fn watch_in_every<W: notify::Watcher + Send + 'static>(
    dir: &Path,
    switch: Arc<KillSwitch>,
    moved: impl Fn(Moved) + Send + 'static,
    every: Duration,
) -> notify::Result<Watch<W>> {
    let (sent, events) = mpsc::channel();
    let mut watcher = W::new(crate::watchset::sender(sent), notify::Config::default())?;
    let mut watched_as = identity(dir);
    watcher.watch(dir, RecursiveMode::NonRecursive)?;
    let watcher = Arc::new(Mutex::new(watcher));
    let rewatching = Arc::downgrade(&watcher);
    let looking = Arc::clone(&switch);
    let dir = dir.to_path_buf();
    std::thread::Builder::new()
        .name("charter-kill-switch-watch".into())
        .spawn(move || {
            // Set when the watch can no longer be trusted, and watched again at the end of the
            // step: cleared only by a watch that took.
            let mut lost = false;
            for step in crate::watchset::bursts_or_idle(events, FOLDED_FOR, MOST_PATHS, every) {
                match step {
                    Some(burst) => {
                        // An event on the directory itself is its removal or its rename (the
                        // platform says so as it drops the watch: inotify's IN_DELETE_SELF and
                        // IN_MOVE_SELF), and a burst that is everything may hide one. A directory
                        // made again at once can have the same device and inode on Linux, so
                        // its identity alone does not show the watch went with the old one.
                        lost |= burst.everything || burst.paths.contains(&dir);
                        let ours = burst.everything
                            || burst
                                .paths
                                .iter()
                                .filter_map(|path| path.file_name())
                                .any(halt::concerns);
                        if ours {
                            moved(looking.look());
                            // After the switch acted on it: a line another process wrote.
                            looking.noted(Noted::Read);
                        }
                    }
                    // Told only when the look moved the switch: a window is not sent the same
                    // standing every few seconds.
                    None => {
                        match looking.look() {
                            Moved::Nothing => {}
                            moved_to => moved(moved_to),
                        }
                        // A watch the platform lost still has its lines recorded; a journal
                        // that has not changed is not read again.
                        looking.noted(Noted::Read);
                    }
                }
                let now = identity(&dir);
                if !lost && now == watched_as {
                    continue;
                }
                let Some(watcher) = rewatching.upgrade() else {
                    return;
                };
                let mut watcher = watcher.lock().unwrap_or_else(PoisonError::into_inner);
                // The old watch is gone with its directory, or about to be: an error is that.
                let _ = watcher.unwatch(&dir);
                // Not watched yet when this fails, or when there is no directory: the next step
                // tries again.
                if now.is_none() || watcher.watch(&dir, RecursiveMode::NonRecursive).is_ok() {
                    watched_as = now;
                    lost = false;
                }
            }
        })
        .map_err(notify::Error::io)?;
    // Anything written before the watch began is looked at once, here.
    let _ = switch.look();
    Ok(Watch { _watcher: watcher })
}

/// Which directory `dir` is now, by its device and inode, or `None` when there is none: a
/// directory made again at the same path is another one, and its watch went with the first.
fn identity(dir: &Path) -> Option<(u64, u64)> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        std::fs::metadata(dir)
            .ok()
            .map(|found| (found.dev(), found.ino()))
    }
    #[cfg(not(unix))]
    {
        std::fs::metadata(dir).ok().map(|_| (0, 0))
    }
}

/// **The runners the re-arm re-arms beside this machine** (#2051, ADR 0071 as amended: *"The
/// window's re-arm lists the runners it will re-arm"*): what the session host's stop stopped,
/// read from the host as the title bar draws a stopped switch. None where chats do not run in a
/// host, or it cannot say.
#[tauri::command]
#[specta::specta]
pub async fn runners_a_rearm_reaches(
    planes: tauri::State<'_, crate::planes::Planes>,
) -> Result<Vec<String>, String> {
    let switch = planes.kill_switch();
    tauri::async_runtime::spawn_blocking(move || switch.runners_a_rearm_reaches())
        .await
        .map_err(|why| format!("the runners could not be read: {why}"))
}

/// Whether every agent is stopped, for a window drawing its title bar.
#[tauri::command]
#[specta::specta]
pub fn agents_stopped(planes: tauri::State<'_, crate::planes::Planes>) -> bool {
    planes.is_stopped()
}

/// The title bar's stop: every chat and shell charter started, in every project and window,
/// ended, and no chat started until re-armed. Answers how many it stopped, or the sentence
/// saying the stop holds here but was not kept on disk.
///
/// On a blocking thread, because ending takes about a second and the window must go on drawing
/// while it does. Every window is told the switch's state once it is done, whichever way.
#[tauri::command]
#[specta::specta]
pub async fn stop_every_agent(app: tauri::AppHandle) -> Result<u32, String> {
    use tauri::{Emitter, Manager};
    tauri::async_runtime::spawn_blocking(move || {
        let planes = app.state::<crate::planes::Planes>();
        let stopped = planes.stop_every_agent(Actor::Window);
        // Every browser too (#2012): its engine's whole process group ends.
        crate::browsers::stop_every_browser(&app);
        let _ = app.emit(CHANGED, planes.is_stopped());
        stopped.map(|count| u32::try_from(count).unwrap_or(u32::MAX))
    })
    .await
    .map_err(|err| format!("the stop did not finish: {err}"))?
}

/// The title bar's re-arm: chats may start again. Nothing that was stopped is restarted.
#[tauri::command]
#[specta::specta]
pub fn rearm_agents(
    app: tauri::AppHandle,
    planes: tauri::State<'_, crate::planes::Planes>,
) -> Result<(), String> {
    use tauri::Emitter;
    let rearmed = planes.rearm();
    let _ = app.emit(CHANGED, planes.is_stopped());
    rearmed.map(|_| ())
}

/// Starts the watch for this app and keeps it for the app's life: a stop another process wrote
/// ends every chat here, and every window is told how the switch stands.
pub fn hear(app: &tauri::AppHandle) {
    use tauri::{Emitter, Manager};
    let planes = app.state::<crate::planes::Planes>();
    let told = app.clone();
    match watch(planes.kill_switch(), move |moved| {
        let planes = told.state::<crate::planes::Planes>();
        if moved == Moved::Stopped {
            planes.end_every_chat();
            crate::browsers::stop_every_browser(&told);
        }
        let _ = told.emit(CHANGED, planes.is_stopped());
    }) {
        Some(Ok(watch)) => {
            app.manage(watch);
        }
        Some(Err(why)) => tracing::warn!(
            "purlis: `purlis stop --all` will not be heard by this app until it restarts ({why})"
        ),
        None => {}
    }
    // A stop written while the app was coming up, which the watch's first look caught.
    if planes.is_stopped() {
        planes.end_every_chat();
        crate::browsers::stop_every_browser(app);
    }
}

#[cfg(test)]
mod tests;
