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
                let line = purlis_core::eventlog::Entry::KillSwitchWrote { line };
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
mod tests {
    use std::sync::mpsc;

    use super::*;
    use purlis_core::halt::{Entry, Event};

    fn a_config_home() -> tempfile::TempDir {
        tempfile::tempdir().expect("a config home")
    }

    fn switch_in(config: &Path) -> Arc<KillSwitch> {
        KillSwitch::kept_in(Some(config.to_path_buf()))
    }

    fn events(config: &Path) -> Vec<(Event, Actor)> {
        halt::journal(config)
            .into_iter()
            .map(|Entry { event, by, .. }| (event, by))
            .collect()
    }

    #[test]
    fn a_stop_another_process_wrote_is_in_force_before_anything_looked_at_it() {
        let config = a_config_home();
        let switch = switch_in(config.path());

        halt::stop(config.path(), Actor::Cli, 1).expect("stopped from a terminal");

        assert!(switch.is_stopped());
    }

    #[test]
    fn an_app_started_after_a_stop_starts_stopped() {
        let config = a_config_home();
        halt::stop(config.path(), Actor::Cli, 1).expect("stopped from a terminal");

        assert!(switch_in(config.path()).is_stopped());
    }

    #[test]
    fn an_app_started_after_a_stop_whose_marker_was_removed_starts_stopped() {
        let config = a_config_home();
        halt::stop(config.path(), Actor::Cli, 1).expect("stopped from a terminal");
        std::fs::remove_file(halt::marker(config.path())).expect("removed by an agent");

        assert!(switch_in(config.path()).is_stopped());
    }

    #[test]
    fn a_marker_removed_while_stopped_does_not_re_arm_and_is_put_back_as_a_tamper() {
        let config = a_config_home();
        let switch = switch_in(config.path());
        switch.stop(Actor::Window).expect("stopped");

        std::fs::remove_file(halt::marker(config.path())).expect("removed by an agent");
        // Even with a forged re-arm in the journal: the switch in memory is the authority.
        let forged = serde_json::to_string(&Entry {
            at: 2,
            event: Event::Rearm,
            by: Actor::Window,
        })
        .unwrap();
        let journal = halt::journal_path(config.path());
        let mut text = std::fs::read_to_string(&journal).unwrap();
        text.push_str(&forged);
        text.push('\n');
        std::fs::write(&journal, text).unwrap();

        assert_eq!(switch.look(), Moved::Tampered);
        assert!(switch.is_stopped());
        assert!(
            halt::marker_present(config.path()),
            "the marker was not put back"
        );
        assert_eq!(
            events(config.path()).last(),
            Some(&(Event::Tamper, Actor::App))
        );
    }

    #[test]
    fn a_look_after_a_stop_on_disk_stops_this_process_and_the_marker_then_cannot_undo_it() {
        let config = a_config_home();
        let switch = switch_in(config.path());
        halt::stop(config.path(), Actor::Cli, 1).expect("stopped from a terminal");

        assert_eq!(switch.look(), Moved::Stopped);
        assert_eq!(switch.look(), Moved::Nothing, "a second look moves nothing");
    }

    #[test]
    fn only_a_re_arm_lets_chats_start_again_and_its_own_removal_is_no_tamper() {
        let config = a_config_home();
        let switch = switch_in(config.path());
        switch.stop(Actor::Window).expect("stopped");

        assert!(switch.rearm().expect("re-armed"));

        assert!(!switch.is_stopped());
        assert_eq!(switch.look(), Moved::Nothing);
        assert_eq!(
            events(config.path()),
            [(Event::Stop, Actor::Window), (Event::Rearm, Actor::Window)]
        );
    }

    #[test]
    fn the_window_s_stop_holds_with_no_config_home_and_says_it_will_not_outlive_the_app() {
        let switch = KillSwitch::unthrown();
        let said = switch
            .stop(Actor::Window)
            .expect_err("nothing to keep it in");
        assert!(said.contains("a relaunch will start chats again"), "{said}");
        assert!(switch.is_stopped());
        assert!(switch.rearm().expect("nothing to remove"));
        assert!(!switch.is_stopped());
    }

    #[cfg(unix)]
    #[test]
    fn a_stop_defeated_in_advance_by_a_chmod_still_stops_this_app_and_says_so() {
        use std::os::unix::fs::PermissionsExt;
        let config = a_config_home();
        let dir = purlis_core::machine::dir(config.path());
        std::fs::create_dir_all(&dir).expect("charter's directory");
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o500)).expect("chmod");
        let switch = switch_in(config.path());

        let said = switch.stop(Actor::Window);

        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).expect("chmod");
        let said = said.expect_err("nothing could be written");
        assert!(
            said.contains("every agent is stopped in this app"),
            "{said}"
        );
        assert!(switch.is_stopped());
    }

    /// An event log in `data`, for a switch to record into.
    fn a_log_in(data: &Path) -> crate::hooks::Events {
        use purlis_core::eventlog::{ArgsKey, Log, Recorder};
        crate::hooks::Events::here(Recorder::new(
            Log::open(data, "01J9ZZDEVICE0000000000000").expect("a log"),
            ArgsKey::open(data).expect("a key"),
        ))
    }

    /// The kill-switch events in the log in `data` once there are `n` of them, as (kind,
    /// actor_kind), or what there were after the wait.
    fn logged(data: &Path, n: usize) -> Vec<(String, String)> {
        let until = std::time::Instant::now() + Duration::from_secs(10);
        loop {
            let found: Vec<(String, String)> = purlis_core::eventlog::read(data)
                .expect("a log")
                .into_iter()
                .filter(|event| event.kind.starts_with("killswitch."))
                .map(|event| {
                    let kind = event.body["actor_kind"].as_str().unwrap_or("").to_owned();
                    (event.kind, kind)
                })
                .collect();
            if found.len() >= n || std::time::Instant::now() > until {
                return found;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn said(kind: &str, actor_kind: &str) -> (String, String) {
        (kind.to_owned(), actor_kind.to_owned())
    }

    #[test]
    fn the_window_s_stop_and_re_arm_are_recorded_as_the_operator_s() {
        // #1375: after the switch has acted, never before it.
        let config = a_config_home();
        let data = tempfile::tempdir().expect("a data home");
        let switch = switch_in(config.path());
        switch.record_into(a_log_in(data.path()));

        switch.stop(Actor::Window).expect("stopped");
        switch.rearm().expect("re-armed");

        assert_eq!(
            logged(data.path(), 2),
            [
                said("killswitch.stop", "human"),
                said("killswitch.rearm", "human")
            ]
        );
    }

    #[test]
    fn a_stop_another_process_wrote_is_recorded_as_unattributed_once() {
        let config = a_config_home();
        let data = tempfile::tempdir().expect("a data home");
        let switch = switch_in(config.path());
        switch.record_into(a_log_in(data.path()));
        halt::stop(config.path(), Actor::Cli, halt::now()).expect("stopped from a terminal");

        assert_eq!(switch.look(), Moved::Stopped);
        switch.noted(Noted::Read);

        assert_eq!(
            logged(data.path(), 1),
            [said("killswitch.stop", "unattributed")]
        );
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(logged(data.path(), 2).len(), 1, "read twice, one event");
    }

    #[cfg(unix)]
    #[test]
    fn a_tamper_is_recorded_even_when_neither_file_could_be_written() {
        use std::os::unix::fs::PermissionsExt;
        let config = a_config_home();
        let data = tempfile::tempdir().expect("a data home");
        let switch = switch_in(config.path());
        switch.record_into(a_log_in(data.path()));
        switch.stop(Actor::Window).expect("stopped");
        std::fs::remove_file(halt::marker(config.path())).expect("removed by an agent");
        let dir = purlis_core::machine::dir(config.path());
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o500)).expect("chmod");

        let moved = switch.look();

        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).expect("chmod");
        assert_eq!(moved, Moved::Tampered);
        assert!(switch.is_stopped());
        assert_eq!(
            logged(data.path(), 2),
            [
                said("killswitch.stop", "human"),
                said("killswitch.tamper", "host")
            ]
        );
    }

    #[test]
    fn a_marker_made_and_taken_away_inside_one_burst_is_still_looked_at() {
        use crate::watchset::raw::{Raw, raw};
        use notify::event::{CreateKind, EventKind, RemoveKind};
        let config = a_config_home();
        let switch = switch_in(config.path());
        let dir = switch.directory().expect("charter's directory");
        let (tell, heard) = mpsc::channel();
        let _watch = watch_in::<Raw>(&dir, Arc::clone(&switch), move |moved| {
            let _ = tell.send(moved);
        })
        .expect("watching");
        halt::stop(config.path(), Actor::Cli, 1).expect("stopped from a terminal");

        // What the platform reports of it is only the marker, made and at once taken away.
        let marker = halt::marker(config.path());
        raw(EventKind::Create(CreateKind::File), &marker);
        raw(EventKind::Remove(RemoveKind::File), &marker);

        assert_eq!(
            heard.recv_timeout(HEARD_WITHIN),
            Ok(Moved::Stopped),
            "the watch never looked"
        );
    }

    #[test]
    fn a_platform_that_lost_track_is_a_look_at_the_switch() {
        use crate::watchset::raw::{Raw, event};
        let config = a_config_home();
        let switch = switch_in(config.path());
        let dir = switch.directory().expect("charter's directory");
        let (tell, heard) = mpsc::channel();
        let _watch = watch_in::<Raw>(&dir, Arc::clone(&switch), move |moved| {
            let _ = tell.send(moved);
        })
        .expect("watching");
        halt::stop(config.path(), Actor::Cli, 1).expect("stopped from a terminal");

        event(notify::Event::new(notify::EventKind::Other).set_flag(notify::event::Flag::Rescan));

        assert_eq!(
            heard.recv_timeout(HEARD_WITHIN),
            Ok(Moved::Stopped),
            "the watch never looked"
        );
    }

    #[test]
    fn a_watch_that_hears_nothing_still_sees_a_stop_within_its_period() {
        // D-88l: the watcher's events are cut off entirely (Raw is told nothing), as a watch
        // the platform has lost would be; the stop is still seen, a period late at most.
        use crate::watchset::raw::Raw;
        let config = a_config_home();
        let switch = switch_in(config.path());
        let dir = switch.directory().expect("charter's directory");
        let (tell, heard) = mpsc::channel();
        let every = Duration::from_millis(200);
        let _watch = watch_in_every::<Raw>(
            &dir,
            Arc::clone(&switch),
            move |moved| {
                let _ = tell.send(moved);
            },
            every,
        )
        .expect("watching");

        halt::stop(config.path(), Actor::Cli, 1).expect("stopped from a terminal");

        assert_eq!(
            heard.recv_timeout(HEARD_WITHIN),
            Ok(Moved::Stopped),
            "a watch that heard nothing never looked"
        );
        assert!(switch.is_stopped());
    }

    /// [`watch_in_every`] on [`Raw`](crate::watchset::raw::Raw), looking every 50 ms, and how
    /// many times `dir` has been watched so far.
    fn watching_counted(
        config: &Path,
    ) -> (
        Watch<crate::watchset::raw::Raw>,
        PathBuf,
        impl Fn() -> usize,
    ) {
        let switch = switch_in(config);
        let dir = switch.directory().expect("charter's directory");
        let watch = watch_in_every::<crate::watchset::raw::Raw>(
            &dir,
            switch,
            |_| {},
            Duration::from_millis(50),
        )
        .expect("watching");
        let counted = dir.clone();
        (watch, dir, move || crate::watchset::raw::watched(&counted))
    }

    /// Waits until `count` says `want`, or ten seconds have passed, and says what it said.
    fn reaches(count: impl Fn() -> usize, want: usize) -> usize {
        let until = std::time::Instant::now() + Duration::from_secs(10);
        while count() < want && std::time::Instant::now() < until {
            std::thread::sleep(Duration::from_millis(20));
        }
        count()
    }

    #[test]
    fn a_directory_the_platform_says_was_removed_is_watched_again_even_as_the_same_inode() {
        // Linux: removed and made again at once, the directory can have the same device and
        // inode, and its watch is gone all the same. What says so is the platform's removal
        // of the directory itself; here nothing on disk changes at all.
        use notify::event::{EventKind, RemoveKind};
        let config = a_config_home();
        let (_watch, dir, watches) = watching_counted(config.path());
        assert_eq!(watches(), 1);

        crate::watchset::raw::raw(EventKind::Remove(RemoveKind::Folder), &dir);

        assert_eq!(
            reaches(&watches, 2),
            2,
            "the directory the platform removed was not watched again"
        );
    }

    #[test]
    fn a_directory_made_again_as_another_is_watched_again_with_nothing_said() {
        // A watch that dies without a word: the directory is another one, and the idle look
        // sees it. The old one is kept aside, so the new one cannot have its inode.
        let config = a_config_home();
        let (_watch, dir, watches) = watching_counted(config.path());
        assert_eq!(watches(), 1);

        std::fs::rename(&dir, dir.with_extension("old")).expect("moved aside");
        std::fs::create_dir_all(&dir).expect("made again");

        assert_eq!(
            reaches(&watches, 2),
            2,
            "the directory made again was not watched again"
        );
    }

    #[test]
    fn the_watch_hears_charter_stop_all_and_puts_back_a_marker_taken_away() {
        let config = a_config_home();
        let switch = switch_in(config.path());
        let (tell, heard) = mpsc::channel();
        let _watch = watch(Arc::clone(&switch), move |moved| {
            let _ = tell.send(moved);
        })
        .expect("a directory to watch")
        .expect("watching");
        // The real watcher, at the real period: on a busy macOS machine its events can arrive
        // late or not at all, and the idle look is what still hears the stop (#1006).
        let next = |wanted: Moved| loop {
            let moved = heard
                .recv_timeout(HEARD_WITHIN)
                .expect("the watch said nothing");
            if moved == wanted {
                break;
            }
        };

        halt::stop(config.path(), Actor::Cli, 1).expect("stopped from a terminal");
        next(Moved::Stopped);
        assert!(switch.is_stopped());

        std::fs::remove_file(halt::marker(config.path())).expect("removed by an agent");
        next(Moved::Tampered);
        assert!(halt::marker_present(config.path()));
        assert!(switch.is_stopped());
    }

    /// A session host's hold on the switch, as the app's switch reaches it (#2001).
    #[derive(Default)]
    struct AHost {
        stopped: Mutex<bool>,
        refuses: bool,
        /// Its stops fail, for as long as this says so.
        stops_fail: Mutex<bool>,
        /// It holds each stop but cannot keep it across its restart.
        cannot_keep: bool,
        heard: Mutex<Vec<&'static str>>,
    }

    impl AHost {
        fn stopped() -> Arc<AHost> {
            let host = AHost::default();
            *host.stopped.lock().unwrap() = true;
            Arc::new(host)
        }

        fn refusing() -> Arc<AHost> {
            Arc::new(AHost {
                refuses: true,
                ..AHost::default()
            })
        }

        fn is_stopped(&self) -> bool {
            *self.stopped.lock().unwrap()
        }

        fn heard(&self) -> Vec<&'static str> {
            self.heard.lock().unwrap().clone()
        }
    }

    impl Authority for AHost {
        fn stop(&self) -> Result<Option<String>, String> {
            self.heard.lock().unwrap().push("stop");
            if self.refuses || *self.stops_fail.lock().unwrap() {
                return Err("the session host is not running".into());
            }
            *self.stopped.lock().unwrap() = true;
            Ok(self
                .cannot_keep
                .then(|| "the stop could not be kept across its restart".to_owned()))
        }

        fn rearm(&self) -> Result<(), String> {
            self.heard.lock().unwrap().push("rearm");
            if self.refuses {
                return Err("the session host is not running".into());
            }
            *self.stopped.lock().unwrap() = false;
            Ok(())
        }

        fn stopped(&self) -> Result<bool, String> {
            if self.refuses {
                return Err("the session host is not running".into());
            }
            Ok(self.is_stopped())
        }
    }

    fn held_by(config: &Path, host: &Arc<AHost>) -> Arc<KillSwitch> {
        let switch = switch_in(config);
        switch.held_by(Arc::clone(host) as Arc<dyn Authority>);
        switch
    }

    #[test]
    fn the_window_s_stop_is_the_session_host_s_too() {
        let config = a_config_home();
        let host = Arc::new(AHost::default());
        let switch = held_by(config.path(), &host);

        switch.stop(Actor::Window).expect("stopped");

        assert!(host.is_stopped());
        assert!(halt::stopped_on_disk(config.path()));
    }

    #[test]
    fn a_stop_from_a_terminal_the_watch_hears_reaches_the_session_host() {
        let config = a_config_home();
        let host = Arc::new(AHost::default());
        let switch = held_by(config.path(), &host);
        halt::stop(config.path(), Actor::Cli, 1).expect("stopped from a terminal");

        assert_eq!(switch.look(), Moved::Stopped);

        assert!(host.is_stopped());
    }

    #[test]
    fn a_re_arm_is_the_session_host_s_first_and_then_the_files() {
        let config = a_config_home();
        let host = Arc::new(AHost::default());
        let switch = held_by(config.path(), &host);
        switch.stop(Actor::Window).expect("stopped");

        assert!(switch.rearm().expect("re-armed"));

        assert_eq!(host.heard(), ["stop", "rearm"]);
        assert!(!host.is_stopped());
        assert!(!switch.is_stopped());
        assert!(!halt::stopped_on_disk(config.path()));
    }

    #[test]
    fn a_re_arm_the_session_host_did_not_take_leaves_every_agent_stopped() {
        let config = a_config_home();
        let host = AHost::refusing();
        let switch = held_by(config.path(), &host);
        let _ = switch.stop(Actor::Window);

        let refused = switch.rearm().expect_err("not re-armed");

        assert!(refused.contains("still stopped"), "{refused}");
        assert!(switch.is_stopped());
        assert!(
            halt::stopped_on_disk(config.path()),
            "the files still say stopped"
        );
    }

    #[test]
    fn a_stop_the_session_host_was_not_told_of_still_stops_this_app_and_says_so() {
        let config = a_config_home();
        let switch = held_by(config.path(), &AHost::refusing());

        let said = switch
            .stop(Actor::Window)
            .expect_err("the host was not told");

        assert!(switch.is_stopped());
        assert!(said.contains("session host"), "{said}");
    }

    #[test]
    fn an_app_that_finds_its_session_host_stopped_holds_every_agent_stopped() {
        let config = a_config_home();
        let switch = held_by(config.path(), &AHost::stopped());
        assert!(!switch.is_stopped(), "the files say nothing");

        assert!(switch.agree_with_its_host(), "the switch moved");

        assert!(switch.is_stopped());
    }

    #[test]
    fn an_app_that_starts_stopped_tells_its_session_host() {
        let config = a_config_home();
        halt::stop(config.path(), Actor::Cli, 1).expect("stopped while no host ran");
        let host = Arc::new(AHost::default());
        let switch = held_by(config.path(), &host);

        assert!(!switch.agree_with_its_host(), "it was stopped already");

        assert!(host.is_stopped());
    }

    #[test]
    fn a_switch_with_no_session_host_is_as_it_was() {
        let config = a_config_home();
        let switch = switch_in(config.path());

        assert!(!switch.agree_with_its_host());
        switch.stop(Actor::Window).expect("stopped");
        assert!(switch.rearm().expect("re-armed"));
    }

    #[cfg(unix)]
    #[test]
    fn a_re_arm_the_files_did_not_take_stops_the_host_again_or_owes_it_the_stop_until_it_does() {
        use std::os::unix::fs::PermissionsExt;
        let config = a_config_home();
        let host = Arc::new(AHost::default());
        let switch = held_by(config.path(), &host);
        switch.stop(Actor::Window).expect("stopped");
        // The files cannot take a re-arm, and the host cannot take its stop back.
        let dir = halt::directory(config.path()).expect("the folder");
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o500)).expect("chmod");
        *host.stops_fail.lock().unwrap() = true;

        let refused = switch.rearm().expect_err("not re-armed");

        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).expect("chmod");
        assert!(refused.contains("still stopped"), "{refused}");
        assert!(
            refused.contains("session host"),
            "the host's part is said: {refused}"
        );
        assert!(switch.is_stopped());
        assert!(!host.is_stopped(), "the host took the re-arm");

        // The host takes stops again: the next look sends it the one it is owed.
        *host.stops_fail.lock().unwrap() = false;
        let _ = switch.look();

        assert!(
            host.is_stopped(),
            "the two never stay apart towards running"
        );
    }

    #[test]
    fn a_stop_the_session_host_did_not_take_is_sent_again_when_the_app_next_agrees_with_it() {
        let config = a_config_home();
        let host = Arc::new(AHost::default());
        *host.stops_fail.lock().unwrap() = true;
        let switch = held_by(config.path(), &host);
        let _ = switch.stop(Actor::Window);
        assert!(!host.is_stopped());

        *host.stops_fail.lock().unwrap() = false;
        switch.agree_with_its_host();

        assert!(host.is_stopped());
    }

    #[test]
    fn a_stop_the_session_host_holds_but_cannot_keep_is_said_as_that_and_not_sent_again() {
        let config = a_config_home();
        let host = Arc::new(AHost {
            cannot_keep: true,
            ..AHost::default()
        });
        let switch = held_by(config.path(), &host);

        let said = switch
            .stop(Actor::Window)
            .expect_err("not kept by the host");
        let _ = switch.look();

        assert!(said.contains("session host said"), "{said}");
        assert!(!said.contains("not told"), "{said}");
        assert_eq!(
            host.heard(),
            ["stop"],
            "a stop the host holds is not owed to it"
        );
    }
}
