// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Tells a window when a theme folder it draws from changes on disk** (#2324, spec #2313):
//! saving any file in a theme folder that is in use repaints the window, with no new window.
//!
//! The theme engine decides everything here ([`purlis_core::theme::reload`]): which folders a
//! window's theme is read from, whether a burst of changes is one of them, and how a run of
//! bursts is paced into redraws. This is the wire: the platform's watcher, a thread, and the
//! event.
//!
//! - **What a window draws from is what it last asked about** ([`follow`]): the levels of the
//!   project and workspace `theme_in_force` was asked for, from the roots it already holds —
//!   the config home, and the plane root `Planes` holds the project at — and the extensions that
//!   contribute only themes. A window gone is let go of ([`forget`]).
//! - **A change only asks.** The window is told [`CHANGED`] and asks `theme_in_force` again,
//!   which reads the theme the one way a theme is ever read. No path a change names is read or
//!   sent.
//! - **One event per burst, at most one redraw per gap** ([`purlis_core::theme::reload::Pace`]),
//!   whatever a chat writes.
//! - **Each folder non-recursively, never through a link**, at most
//!   [`purlis_core::theme::reload::MOST_WATCHED`] for a window.
//! - **When the platform will not watch, it looks instead**: every
//!   [`purlis_core::theme::reload::LOOK_EVERY`], at the entries of the folders it would have
//!   watched, and never more often.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::{Duration, Instant};

use purlis_core::counts::counted;
use purlis_core::theme::reload::{LOOK_EVERY, Look, Pace, QUIET_FOR, Reload, ThemesOnly, Watched};

/// The event a window is sent when its theme is to be read again.
pub(crate) const CHANGED: &str = "theme-folders-changed";

/// What [`CHANGED`] carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ThemeFoldersChanged {
    /// Whether an extension's themes changed too, so the survey of them is taken again.
    pub survey: bool,
}

/// The most paths a burst holds before it counts as everything.
const MOST_PATHS: usize = 512;

/// How long the thread sleeps when nothing is owed and nothing is looked at: it wakes for an
/// event, and at least this often to notice a window that came to need a look (a folder the
/// platform refused) or that nothing holds the watch any more. One wake, never a spin.
const IDLE: Duration = LOOK_EVERY;

/// **Where one window's theme is read from**: the config home, and the plane root and the
/// workspace in front, as `theme_in_force` was asked for them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Target {
    pub config: PathBuf,
    pub plane: Option<PathBuf>,
    pub workspace: Option<String>,
}

impl Target {
    /// `path` relative to the root of this target it is under, for a log line that names no
    /// folder of the person's: `config:…`, `plane:…`, or `elsewhere:` and its name alone.
    fn relative(&self, path: &Path) -> String {
        let below = |root: &Path| {
            [Some(root.to_path_buf()), root.canonicalize().ok()]
                .into_iter()
                .flatten()
                .find_map(|one| path.strip_prefix(&one).ok().map(Path::to_path_buf))
        };
        if let Some(inside) = below(&self.config) {
            return format!("config:{}", inside.display());
        }
        if let Some(inside) = self.plane.as_deref().and_then(below) {
            return format!("plane:{}", inside.display());
        }
        format!(
            "elsewhere:{}",
            path.file_name()
                .map(|one| one.to_string_lossy())
                .unwrap_or_default()
        )
    }

    /// What to watch for it now.
    fn watched(&self) -> Watched {
        let folders = purlis_core::theme::Folders::of(
            Some(&self.config),
            self.plane.as_deref(),
            self.workspace.as_deref(),
        );
        let extensions = ThemesOnly::installed(&self.config, &crate::extensions::built_in());
        Watched::of(&folders, &extensions)
    }
}

/// Tells the window `label` that its theme is to be read again.
type Tell = Arc<dyn Fn(&str, ThemeFoldersChanged) + Send + Sync>;

struct Inner<W: notify::Watcher> {
    /// The platform's watcher, or `None` when it could not be made: every folder is looked at.
    watcher: Option<W>,
    /// What is watched now, of every window.
    watching: HashSet<PathBuf>,
    /// The folders the platform would not watch: looked at instead.
    refused: HashSet<PathBuf>,
    /// Each window's target, and what it watches.
    windows: HashMap<String, (Target, Watched)>,
    /// Each window's last look, while anything of it is looked at.
    looks: HashMap<String, Look>,
    /// Each window's pace.
    paces: HashMap<String, Pace>,
    /// Kept so the channel stays open when there is no platform watcher to hold its sender.
    _sender: Sender<notify::Event>,
}

impl<W: notify::Watcher> Inner<W> {
    /// The platform watches the union of every window's folders.
    fn follow(&mut self) {
        let wanted: HashSet<PathBuf> = self
            .windows
            .values()
            .flat_map(|(_, watched)| watched.folders().map(Path::to_path_buf))
            .collect();
        self.refused = match self.watcher.as_mut() {
            Some(watcher) => crate::watchset::follow(watcher, &mut self.watching, wanted),
            None => wanted,
        };
        tracing::info!(
            "purlis: theme folders: {} watched, {} looked at instead, for {}",
            self.watching.len(),
            self.refused.len(),
            counted(self.windows.len(), "window", "windows")
        );
    }

    /// Whether anything of `watched` is looked at rather than watched.
    fn looks_at(&self, watched: &Watched) -> bool {
        watched
            .folders()
            .any(|folder| self.refused.contains(folder))
    }

    /// Each window's folders found again, after a change that may have made or taken one.
    fn find_again(&mut self) {
        for (target, watched) in self.windows.values_mut() {
            *watched = target.watched();
        }
        self.follow();
    }

    fn heard(&mut self, paths: &HashSet<PathBuf>, everything: bool) -> bool {
        let mut again = everything;
        for (label, (target, watched)) in &self.windows {
            let asked = watched.decide(paths.iter().map(PathBuf::as_path), everything);
            let why = match asked {
                Reload::Nothing => ": none of them is inside a themes/ it holds",
                Reload::Redraw { .. } => "",
            };
            let named: Vec<String> = paths
                .iter()
                .take(3)
                .map(|path| target.relative(path))
                .collect();
            tracing::info!(
                "purlis: theme folders: a burst of {}{} asks {asked:?} of window {label}{why}; \
                 among them {}",
                counted(paths.len(), "change", "changes"),
                if everything {
                    " (anything may have moved)"
                } else {
                    ""
                },
                named.join(", ")
            );
            again |= asked != Reload::Nothing;
            self.paces.entry(label.clone()).or_default().heard(asked);
        }
        again
    }
}

/// **The theme watch**: one for the app, its windows' targets in it.
pub(crate) struct ThemeWatch<W: notify::Watcher = notify::RecommendedWatcher> {
    inner: Arc<Mutex<Inner<W>>>,
}

impl<W: notify::Watcher + Send + 'static> ThemeWatch<W> {
    /// Starts the watch, telling each window through `tell`. A platform watcher that cannot be
    /// made leaves the watch looking instead.
    fn start(tell: Tell) -> Self {
        let (sent, events) = mpsc::channel();
        let config = notify::Config::default().with_follow_symlinks(false);
        // A poller in a test looks often; the platform's own watchers do not poll.
        #[cfg(test)]
        let config = config.with_poll_interval(Duration::from_millis(50));
        let watcher = match W::new(crate::watchset::sender(sent.clone()), config) {
            Ok(watcher) => Some(watcher),
            Err(why) => {
                tracing::warn!(
                    "purlis: theme folders are looked at every {}s instead of watched ({why})",
                    LOOK_EVERY.as_secs()
                );
                None
            }
        };
        let inner = Arc::new(Mutex::new(Inner {
            watcher,
            watching: HashSet::new(),
            refused: HashSet::new(),
            windows: HashMap::new(),
            looks: HashMap::new(),
            paces: HashMap::new(),
            _sender: sent,
        }));
        let running = Arc::clone(&inner);
        let started = std::thread::Builder::new()
            .name("purlis-theme-watch".into())
            .spawn(move || run(&running, &events, &tell));
        if let Err(why) = started {
            tracing::warn!("purlis: theme folders are not watched ({why})");
        }
        Self { inner }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner<W>> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The window `label` draws its theme from `target` now.
    /// Its folders are found again at every ask, so a watch that went stale (a folder made or
    /// taken that no change told) is put right by the window's next draw.
    fn set(&self, label: &str, target: Target) {
        let watched = target.watched();
        let mut inner = self.lock();
        if inner
            .windows
            .get(label)
            .is_some_and(|(was, now)| *was == target && *now == watched)
        {
            return;
        }
        if watched.left_out() > 0 {
            tracing::warn!(
                "purlis: {} theme folders are not watched for this window, past the {} watched",
                watched.left_out(),
                purlis_core::theme::reload::MOST_WATCHED
            );
        }
        let held: Vec<String> = watched.levels().map(|one| target.relative(one)).collect();
        tracing::info!(
            "purlis: theme folders: window {label} watches {}, and redraws for a change inside {}",
            counted(watched.folders().count(), "folder", "folders"),
            held.join(", ")
        );
        inner.windows.insert(label.to_owned(), (target, watched));
        inner.looks.remove(label);
        inner.follow();
    }

    /// The plane at `root` is let go of: every window drawing from it is watched for the
    /// person's own folders alone, until it asks for its theme again.
    fn let_go_of_plane(&self, root: &Path) {
        let mut inner = self.lock();
        let mut moved = false;
        for (target, watched) in inner.windows.values_mut() {
            if target.plane.as_deref() == Some(root) {
                target.plane = None;
                target.workspace = None;
                *watched = target.watched();
                moved = true;
            }
        }
        if moved {
            inner.follow();
        }
    }

    /// The window `label` is gone.
    fn forget(&self, label: &str) {
        let mut inner = self.lock();
        inner.windows.remove(label);
        inner.looks.remove(label);
        inner.paces.remove(label);
        inner.follow();
    }
}

/// The watch's thread: bursts in, redraws out, until nothing holds the watch.
fn run<W: notify::Watcher>(
    inner: &Arc<Mutex<Inner<W>>>,
    events: &Receiver<notify::Event>,
    tell: &Tell,
) {
    let lock = || inner.lock().unwrap_or_else(PoisonError::into_inner);
    let mut looked = Instant::now();
    loop {
        let now = Instant::now();
        let wait = {
            let inner = lock();
            let owed = inner.paces.values().filter_map(|pace| pace.wait(now)).min();
            let looking = inner
                .windows
                .values()
                .any(|(_, watched)| inner.looks_at(watched));
            let look = looking.then(|| (looked + LOOK_EVERY).saturating_duration_since(now));
            [owed, look].into_iter().flatten().min().unwrap_or(IDLE)
        };
        match events.recv_timeout(wait) {
            Ok(first) => {
                let Some(burst) =
                    crate::watchset::rest_of_burst(events, first, QUIET_FOR, MOST_PATHS)
                else {
                    return;
                };
                let mut inner = lock();
                if inner.heard(&burst.paths, burst.everything) || burst.lost {
                    inner.find_again();
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        let now = Instant::now();
        if now.saturating_duration_since(looked) >= LOOK_EVERY {
            looked = now;
            look(&mut lock());
        }
        let due: Vec<(String, Reload)> = {
            let mut inner = lock();
            inner
                .paces
                .iter_mut()
                .filter_map(|(label, pace)| pace.due(now).map(|asked| (label.clone(), asked)))
                .collect()
        };
        for (label, asked) in due {
            if let Reload::Redraw { survey } = asked {
                tracing::info!(
                    "purlis: theme folders changed: window {label} reads its theme again{}",
                    if survey {
                        ", and the extensions' themes"
                    } else {
                        ""
                    }
                );
                tell(&label, ThemeFoldersChanged { survey });
            }
        }
    }
}

/// Each window any of whose folders the platform would not watch is looked at, and what the
/// look finds changed is heard as a burst.
fn look<W: notify::Watcher>(inner: &mut Inner<W>) {
    let mut changed: Vec<(String, Vec<PathBuf>)> = Vec::new();
    let labels: Vec<String> = inner
        .windows
        .iter()
        .filter(|(_, (_, watched))| inner.looks_at(watched))
        .map(|(label, _)| label.clone())
        .collect();
    for label in labels {
        let Some((_, watched)) = inner.windows.get(&label) else {
            continue;
        };
        let now = Look::of(watched);
        // The first look is what the next is compared with.
        if let Some(before) = inner.looks.insert(label.clone(), now.clone()) {
            changed.push((label, before.changed(&now)));
        }
    }
    let mut again = false;
    for (label, paths) in changed {
        let Some((_, watched)) = inner.windows.get(&label) else {
            continue;
        };
        let asked = watched.decide(paths.iter().map(PathBuf::as_path), false);
        again |= asked != Reload::Nothing;
        inner.paces.entry(label).or_default().heard(asked);
    }
    if again {
        inner.find_again();
    }
}

/// The app's one theme watch, started by the first window that asks for its theme.
static WATCH: OnceLock<ThemeWatch> = OnceLock::new();

/// **The window draws its theme from `target` now**: watch its theme folders, and tell it
/// [`CHANGED`] when one changes.
pub(crate) fn follow(window: &tauri::WebviewWindow, target: Target) {
    use tauri::{Emitter as _, Manager as _};
    let app = window.app_handle().clone();
    let watch = WATCH.get_or_init(|| {
        ThemeWatch::start(Arc::new(move |label: &str, said: ThemeFoldersChanged| {
            // A window gone between the change and here is told nothing.
            let _ = app.emit_to(label, CHANGED, said);
        }))
    });
    watch.set(window.label(), target);
}

/// The plane at `root` is closed: its theme folders are no longer watched for any window.
pub(crate) fn let_go_of_plane(root: &Path) {
    if let Some(watch) = WATCH.get() {
        watch.let_go_of_plane(root);
    }
}

/// The window `label` is gone: its theme folders are no longer watched for it.
pub(crate) fn forget(label: &str) {
    if let Some(watch) = WATCH.get() {
        watch.forget(label);
    }
}

/// Every folder watched now, for a test.
#[cfg(test)]
fn watching<W: notify::Watcher + Send + 'static>(
    watch: &ThemeWatch<W>,
) -> std::collections::BTreeSet<PathBuf> {
    watch.lock().watching.iter().cloned().collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use purlis_core::theme::reload::GAP;

    use super::*;

    /// What the watch told, in order.
    #[derive(Default)]
    struct Told(Mutex<Vec<(String, ThemeFoldersChanged)>>);

    impl Told {
        fn tell(self: &Arc<Self>) -> Tell {
            let told = Arc::clone(self);
            Arc::new(move |label: &str, said| {
                told.0
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .push((label.to_owned(), said));
            })
        }

        fn count(&self) -> usize {
            self.0.lock().unwrap_or_else(PoisonError::into_inner).len()
        }

        fn until(&self, at_least: usize, within: Duration) -> bool {
            let until = Instant::now() + within;
            while Instant::now() < until {
                if self.count() >= at_least {
                    return true;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            false
        }
    }

    /// A config home and a plane with a project theme folder `team`.
    fn places() -> (tempfile::TempDir, Target) {
        let home = tempfile::tempdir().unwrap();
        let config = home.path().join("config");
        let plane = home.path().join("plane");
        std::fs::create_dir_all(purlis_core::machine::dir(&config)).unwrap();
        std::fs::create_dir_all(plane.join("themes/team")).unwrap();
        std::fs::write(plane.join("themes/team/theme.json"), "{}").unwrap();
        (
            home,
            Target {
                config,
                plane: Some(plane),
                workspace: None,
            },
        )
    }

    /// A platform watcher that refuses to be made: the watch looks instead.
    struct Refusing;

    impl notify::Watcher for Refusing {
        fn new<F: notify::EventHandler>(_: F, _: notify::Config) -> notify::Result<Self> {
            Err(notify::Error::generic("refused by the test"))
        }
        fn watch(&mut self, _: &Path, _: notify::RecursiveMode) -> notify::Result<()> {
            unreachable!()
        }
        fn unwatch(&mut self, _: &Path) -> notify::Result<()> {
            unreachable!()
        }
        fn kind() -> notify::WatcherKind {
            notify::WatcherKind::NullWatcher
        }
    }

    #[test]
    fn a_window_s_theme_folders_are_watched_and_let_go_of_with_it() {
        let (_home, target) = places();
        let told = Arc::new(Told::default());
        let watch = ThemeWatch::<notify::PollWatcher>::start(told.tell());
        let plane = target.plane.clone().unwrap();
        watch.set("main", target);
        let got = watching(&watch);
        assert!(got.contains(&plane.join("themes")), "{got:?}");
        assert!(got.contains(&plane.join("themes/team")), "{got:?}");
        watch.forget("main");
        assert_eq!(watching(&watch), BTreeSet::new());
    }

    #[test]
    fn a_plane_let_go_of_is_no_longer_watched_and_the_person_s_folders_still_are() {
        let (home, target) = places();
        let mine = purlis_core::machine::dir(&target.config).join("themes/mine");
        std::fs::create_dir_all(&mine).unwrap();
        let told = Arc::new(Told::default());
        let watch = ThemeWatch::<notify::PollWatcher>::start(told.tell());
        let plane = target.plane.clone().unwrap();
        watch.set("main", target.clone());
        // Another window on another project keeps its own.
        let other = home.path().join("other");
        std::fs::create_dir_all(other.join("themes/b")).unwrap();
        watch.set(
            "split",
            Target {
                plane: Some(other.clone()),
                ..target
            },
        );
        watch.let_go_of_plane(&plane);
        let got = watching(&watch);
        assert!(got.iter().all(|one| !one.starts_with(&plane)), "{got:?}");
        assert!(got.contains(&mine), "{got:?}");
        assert!(got.contains(&other.join("themes/b")), "{got:?}");
    }

    /// The handler each [`Injected`] watcher was made with, by the test that made it: what a
    /// test tells the watch, in the shapes the platform reports, with no platform's timing.
    static INJECT: Mutex<Option<Box<dyn notify::EventHandler>>> = Mutex::new(None);

    /// A platform watcher that watches every folder it is asked to and reports only what a test
    /// injects ([`inject`]).
    struct Injected;

    impl notify::Watcher for Injected {
        fn new<F: notify::EventHandler>(handler: F, _: notify::Config) -> notify::Result<Self> {
            *INJECT.lock().unwrap_or_else(PoisonError::into_inner) = Some(Box::new(handler));
            Ok(Self)
        }
        fn watch(&mut self, _: &Path, _: notify::RecursiveMode) -> notify::Result<()> {
            Ok(())
        }
        fn unwatch(&mut self, _: &Path) -> notify::Result<()> {
            Ok(())
        }
        fn kind() -> notify::WatcherKind {
            notify::WatcherKind::NullWatcher
        }
    }

    /// Reports a change of `kind` to `paths`, as the platform would.
    fn inject(kind: notify::EventKind, paths: &[PathBuf]) {
        let mut event = notify::Event::new(kind);
        for path in paths {
            event = event.add_path(path.clone());
        }
        if let Some(handler) = INJECT
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_mut()
        {
            handler.handle_event(Ok(event));
        }
    }

    #[test]
    fn a_theme_folder_made_where_a_level_had_none_is_heard_and_every_save_after_it() {
        use notify::event::{CreateKind, DataChange, EventKind, ModifyKind, RenameMode};
        let home = tempfile::tempdir().unwrap();
        let config = home.path().join("config");
        let machine = purlis_core::machine::dir(&config);
        std::fs::create_dir_all(&machine).unwrap();
        let told = Arc::new(Told::default());
        let watch = ThemeWatch::<Injected>::start(told.tell());
        watch.set(
            "main",
            Target {
                config,
                plane: None,
                workspace: None,
            },
        );
        let folder = machine.join("themes/live");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("theme.json"), "{}").unwrap();
        let file = folder.join("theme.json");
        // The folder made: the platform names `themes` in the folder that was watched.
        inject(
            EventKind::Create(CreateKind::Folder),
            &[machine.join("themes")],
        );
        assert!(
            told.until(1, Duration::from_secs(5)),
            "the folder made was never told"
        );
        assert!(watching(&watch).contains(&folder), "{:?}", watching(&watch));
        // Each later save, in each shape the platform reports one, is told once more.
        let saves = [
            (
                EventKind::Modify(ModifyKind::Data(DataChange::Any)),
                vec![file.clone()],
            ),
            (
                EventKind::Modify(ModifyKind::Name(RenameMode::Both)),
                vec![folder.join("theme.json.tmp-1"), file.clone()],
            ),
            (EventKind::Create(CreateKind::File), vec![file.clone()]),
            (
                EventKind::Modify(ModifyKind::Metadata(notify::event::MetadataKind::WriteTime)),
                vec![file.clone()],
            ),
        ];
        for (n, (kind, paths)) in saves.into_iter().enumerate() {
            std::thread::sleep(GAP);
            inject(kind, &paths);
            assert!(
                told.until(n + 2, Duration::from_secs(5)),
                "save {n} was never told"
            );
        }
        drop(watch);
        *INJECT.lock().unwrap_or_else(PoisonError::into_inner) = None;
    }

    /// What the window would draw for `config` now, as `theme_in_force` answers it.
    fn surface_drawn(config: &Path) -> Option<String> {
        use purlis_core::theme;
        let levels = theme::Levels::read(Some(config), None, None);
        let folders = theme::Folders::of(Some(config), None, None);
        let drawn = crate::extensions::theme_in_force_of(
            &levels,
            &folders,
            None,
            None,
            false,
            theme::Contrast::Standard,
            || Arc::from(Vec::new()),
            |_| None,
        );
        drawn
            .theme
            .and_then(|one| one.values.get("surface.base").cloned())
    }

    fn put_surface(folder: &Path, surface: &str) {
        std::fs::create_dir_all(folder).unwrap();
        std::fs::write(
            folder.join("theme.json"),
            format!(
                r#"{{"name":"Live","appearances":[{{"id":"dark","appearance":"dark","tokens":{{"surface.base":"{surface}"}}}}]}}"#
            ),
        )
        .unwrap();
    }

    #[test]
    fn a_machine_theme_folder_chosen_and_saved_is_drawn_again_as_saved() {
        use purlis_core::theme::{Choice, choices};
        // The chain the scenario runs, without the window: a machine theme folder chosen, the
        // watch on it, a save, the window told, and what `theme_in_force` answers then. Each
        // surface is one Classic's text and marks clear, so no consent floor stands in for it.
        let home = tempfile::tempdir().unwrap();
        let config = home.path().join("config");
        let machine = purlis_core::machine::dir(&config);
        let folder = machine.join("themes/live");
        put_surface(&folder, "#171710");
        choices::set(&config, choices::Slot::Machine, Choice::new("live", None)).unwrap();
        assert_eq!(surface_drawn(&config).as_deref(), Some("#171710"));
        let told = Arc::new(Told::default());
        let watch = ThemeWatch::<notify::PollWatcher>::start(told.tell());
        watch.set(
            "main",
            Target {
                config: config.clone(),
                plane: None,
                workspace: None,
            },
        );
        assert!(watching(&watch).contains(&folder), "{:?}", watching(&watch));
        for (n, surface) in ["#181810", "#191910"].into_iter().enumerate() {
            // The poller tells a file changed by its time to the second, and these saves are of
            // one size: each comes in a second of its own.
            std::thread::sleep(Duration::from_millis(1100));
            put_surface(&folder, surface);
            assert!(
                told.until(n + 1, Duration::from_secs(10)),
                "the save of {surface} was never told"
            );
            assert_eq!(surface_drawn(&config).as_deref(), Some(surface));
        }
    }

    #[test]
    fn a_platform_that_will_not_watch_is_looked_at_instead_and_a_save_is_told_once() {
        let (_home, target) = places();
        let told = Arc::new(Told::default());
        let watch = ThemeWatch::<Refusing>::start(told.tell());
        let plane = target.plane.clone().unwrap();
        watch.set("main", target);
        // The first look is what the next is compared with: the save comes after it.
        let looked = Instant::now() + LOOK_EVERY * 5;
        while !watch.lock().looks.contains_key("main") {
            assert!(Instant::now() < looked, "the watch never looked");
            std::thread::sleep(Duration::from_millis(50));
        }
        std::fs::write(plane.join("themes/team/theme.json"), "{\"saved\":true}").unwrap();
        assert!(
            told.until(1, LOOK_EVERY * 5),
            "the look never told the save"
        );
        // It looks no more often than its interval, and a save is told once.
        std::thread::sleep(LOOK_EVERY * 2 + GAP);
        assert_eq!(told.count(), 1);
        drop(watch);
    }
}
