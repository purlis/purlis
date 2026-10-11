//! Tells a window when a folder its explorer has expanded changes on disk (FM-1, #1103 story
//! 57): an agent adds or removes a file, and the tree shows it without a manual refresh.
//!
//! **The same watcher as the plane's (`planewatch.rs`), on different folders.** The plane watch
//! deliberately stays out of the clones: a recursive watch there would put one on every folder
//! of every `node_modules/` and `target/`. This one watches exactly the folders a window has
//! expanded, each **non-recursively**, so its cost is what the operator has open and nothing
//! below it. A window hands its whole set each time it changes, and the set is dropped with the
//! window.
//!
//! **The window names folders, never directories.** Each one is a branch and a path inside it,
//! resolved by `purlis_core::files::Root::resolve` with every check the tree's own read makes,
//! the branch found by the bounded reader's child; one that does not resolve is not watched.
//! Each branch is found once per call, however many of its folders are open, and at most
//! `purlis_core::files::WATCHED` folders are watched for a window.
//!
//! **One event per burst** ([`crate::watchset::bursts`]), and an access is not a change: the
//! window reads the folder again when told, and on inotify that read is an `IN_OPEN`.
//!
//! **Every change is told, a short-lived file's included** (#1138). The window lists a folder
//! whenever it is told about it, so it can draw a file whose creation has not been passed on
//! yet; a debouncer that took that file's removal as cancelling its creation never told the
//! window to draw it gone. inotify reports the two separately, so an e2e spec on Linux that
//! removed a file it had just seen drawn found it drawn for ever. So the folder of every event
//! that is a change is told, whatever came after it.
//!
//! **A branch the reader gave no answer for is said, and asked again** (#1727). Finding a
//! branch's folder asks the bounded reader, which may answer that the branch is paused (its
//! reads kept running into their bounds), that every place was taken, or nothing at all within
//! its deadline. Such a branch is not watched yet, and the window is told so
//! ([`UNWATCHED`]: every folder of the window not watched now), so the explorer does not look
//! live over folders that cannot move. Its folders are asked for again once the pause is over,
//! or [`AGAIN_AFTER`] later, on a thread of their own, for as long as the window keeps the set
//! that named them; once found they are watched, and told as moved, since whatever changed in
//! them meanwhile was told by nothing. A branch the reader answered with a refusal (it is gone,
//! or the tree refuses it) is not watched and not asked again, as before: the tree's own read
//! says why.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Arc, Mutex, PoisonError, Weak};
use std::time::{Duration, Instant};

use crate::planes::{PlaneId, Planes};
use crate::watchset::Burst;

/// The event a window is sent.
pub(crate) const CHANGED: &str = "files-changed";

/// The event naming every folder of a window that is not watched now (#1727), carried as
/// [`FilesChanged`]: sent after each set a window asks for, and each time a folder of it is
/// watched late. An empty list says every folder is watched.
pub(crate) const UNWATCHED: &str = "files-unwatched";

/// How long after a read that gave no answer, and was not a pause, a branch's folders are
/// asked for again: about one reader deadline. A branch that keeps hanging is paused by the
/// reader after its second strike, and is then asked again when that pause is over.
const AGAIN_AFTER: Duration = Duration::from_secs(30);

/// How often a thread waiting to ask for a branch again looks at whether the window still
/// wants it: a window that moves on lets go of the thread within this.
const LOOKED_EVERY: Duration = Duration::from_secs(1);

/// How long a burst is folded for: an agent writing ten files is one read of their folder.
const QUIET_FOR: Duration = Duration::from_millis(250);

/// The most changed paths one burst holds. Past it the burst is taken as having moved every
/// open folder: a flood costs a fixed amount of memory, and a window reads a few folders too
/// many rather than missing one.
const MOST_PATHS: usize = 256;

/// One folder of a branch, as the window names it: the plane, the workspace, the repo, the piece
/// (none for the repo's own folder) and the folder's path inside the branch (`""` for its top).
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct BranchFolder {
    pub plane: PlaneId,
    pub workspace: String,
    pub repo: String,
    pub piece: Option<String>,
    pub folder: String,
}

/// What `files-changed` carries: the folders, of those this window asked to watch, that moved.
#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct FilesChanged {
    pub folders: Vec<BranchFolder>,
}

/// Told which window's folders moved.
pub type Told = Arc<dyn Fn(&str, Vec<BranchFolder>) + Send + Sync + 'static>;

struct Inner<W: notify::Watcher> {
    watcher: Option<W>,
    /// Each window's folders, with the directory each resolved to.
    by_window: HashMap<String, Vec<(BranchFolder, PathBuf)>>,
    /// The newest set each window asked for, by its [`FileWatch::ticket`]: a set resolved late
    /// never replaces one asked for after it.
    newest: HashMap<String, u64>,
    watched: HashSet<PathBuf>,
    /// Each window's folders of its newest set that are not watched yet (#1727).
    unwatched: HashMap<String, Vec<BranchFolder>>,
}

/// Every window's watched folders. Managed by the app; dropping it stops every watch.
pub struct FileWatch<W: notify::Watcher = notify::RecommendedWatcher> {
    inner: Arc<Mutex<Inner<W>>>,
    told: Told,
    config: notify::Config,
    tickets: std::sync::atomic::AtomicU64,
}

impl FileWatch {
    /// A watch that tells `told`, on the platform's own watcher. Nothing is watched until a
    /// window asks.
    pub fn new(told: Told) -> Self {
        Self::with_config(told, notify::Config::default())
    }
}

impl<W: notify::Watcher + Send + 'static> FileWatch<W> {
    fn with_config(told: Told, config: notify::Config) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                watcher: None,
                by_window: HashMap::new(),
                newest: HashMap::new(),
                watched: HashSet::new(),
                unwatched: HashMap::new(),
            })),
            told,
            config,
            tickets: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// A number for a set a window asks for, larger than every one handed out before: taken
    /// when the ask arrives, so the order of the asks decides and not the order their folders
    /// finish resolving in.
    pub fn ticket(&self) -> u64 {
        self.tickets
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            .saturating_add(1)
    }

    /// [`FileWatch::set`], with `unwatched` the folders of the set that could not be watched
    /// yet (#1727); unless the window has since asked for a newer set. Whether the set was
    /// taken.
    pub fn set_from(
        &self,
        window: &str,
        ticket: u64,
        folders: Vec<(BranchFolder, PathBuf)>,
        unwatched: Vec<BranchFolder>,
    ) -> Result<bool, String> {
        {
            let mut inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
            let newest = inner.newest.entry(window.to_string()).or_default();
            if ticket < *newest {
                return Ok(false);
            }
            *newest = ticket;
            if unwatched.is_empty() {
                inner.unwatched.remove(window);
            } else {
                inner.unwatched.insert(window.to_string(), unwatched);
            }
        }
        self.set(window, folders).map(|()| true)
    }

    /// Whether `ticket` is still the newest set `window` asked for.
    fn wants(&self, window: &str, ticket: u64) -> bool {
        let inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        inner.newest.get(window) == Some(&ticket)
    }

    /// Folders of `window`'s set `ticket` found late (#1727): `found` is watched now, and
    /// `settled` (found, or refused for good) is no longer said to be unwatched. `found` is
    /// told to the window as moved: whatever changed in it meanwhile was told by nothing.
    /// Answers the window's folders still not watched, or nothing when the window has since
    /// asked for a newer set, which then holds.
    fn found_late(
        &self,
        window: &str,
        ticket: u64,
        found: Vec<(BranchFolder, PathBuf)>,
        settled: &[BranchFolder],
    ) -> Option<Vec<BranchFolder>> {
        let (moved, still) = {
            let mut inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
            if inner.newest.get(window) != Some(&ticket) {
                return None;
            }
            let moved: Vec<BranchFolder> = found.iter().map(|(folder, _)| folder.clone()).collect();
            if !found.is_empty() {
                inner
                    .by_window
                    .entry(window.to_string())
                    .or_default()
                    .extend(found);
            }
            let still = match inner.unwatched.get_mut(window) {
                Some(list) => {
                    list.retain(|folder| !settled.contains(folder));
                    list.clone()
                }
                None => Vec::new(),
            };
            if still.is_empty() {
                inner.unwatched.remove(window);
            }
            if inner.watcher.is_none() && !inner.by_window.is_empty() {
                // No watcher could be made: these folders are not watched after all.
                match self.start() {
                    Ok(watcher) => inner.watcher = Some(watcher),
                    Err(_) => return Some(still),
                }
            }
            inner.follow();
            (moved, still)
        };
        // Outside the lock, as the watch's own thread tells.
        if !moved.is_empty() {
            (self.told)(window, moved);
        }
        Some(still)
    }

    /// `window`'s folders are now `folders`, each with the directory it resolved to; the ones
    /// it had before and does not name are no longer watched for it.
    pub fn set(&self, window: &str, folders: Vec<(BranchFolder, PathBuf)>) -> Result<(), String> {
        let mut inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        if folders.is_empty() {
            inner.by_window.remove(window);
        } else {
            inner.by_window.insert(window.to_string(), folders);
        }
        if inner.watcher.is_none() && !inner.by_window.is_empty() {
            inner.watcher = Some(self.start()?);
        }
        inner.follow();
        Ok(())
    }

    /// `window` is gone: none of its folders is watched for it any more.
    pub fn forget(&self, window: &str) {
        let mut inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        inner.newest.remove(window);
        inner.unwatched.remove(window);
        if inner.by_window.remove(window).is_some() {
            inner.follow();
        }
    }

    /// The platform's watcher, and a thread that folds what it reports into one tell per burst.
    /// The thread ends when the watcher is dropped, which drops the sending half it reads.
    fn start(&self) -> Result<W, String> {
        let (sent, events) = mpsc::channel();
        let handle: Weak<Mutex<Inner<W>>> = Arc::downgrade(&self.inner);
        let told = Arc::clone(&self.told);
        std::thread::Builder::new()
            .name("charter-files-watch".into())
            .spawn(move || {
                for burst in crate::watchset::bursts(events, QUIET_FOR, MOST_PATHS) {
                    let Some(inner) = handle.upgrade() else {
                        return;
                    };
                    let concerned = concerning(&burst, &inner);
                    drop(inner);
                    // Outside the lock, so a window that answers by watching again never waits
                    // on it.
                    for (window, folders) in concerned {
                        told(&window, folders);
                    }
                }
            })
            .map_err(|e| format!("purlis could not watch the branch's folders: {e}"))?;
        W::new(
            crate::watchset::sender(sent),
            // Never into a link: a folder linked from inside the branch to outside it is not
            // the branch's, as the tree's own read refuses it.
            self.config.with_follow_symlinks(false),
        )
        .map_err(|e| format!("purlis could not watch the branch's folders: {e}"))
    }
}

/// Each window that has one of `burst`'s folders open, and which of its folders moved: a
/// folder moves when a path in it changes (a file added is an event on the file) or it changes
/// itself (a watched folder removed is an event on the folder).
fn concerning<W: notify::Watcher>(
    burst: &Burst,
    inner: &Mutex<Inner<W>>,
) -> Vec<(String, Vec<BranchFolder>)> {
    let moved: HashSet<&std::path::Path> = burst
        .paths
        .iter()
        .flat_map(|path| [Some(path.as_path()), path.parent()])
        .flatten()
        .collect();
    let inner = inner.lock().unwrap_or_else(PoisonError::into_inner);
    inner
        .by_window
        .iter()
        .filter_map(|(window, folders)| {
            let hit: Vec<BranchFolder> = folders
                .iter()
                .filter(|(_, dir)| burst.everything || moved.contains(dir.as_path()))
                .map(|(folder, _)| folder.clone())
                .collect();
            (!hit.is_empty()).then(|| (window.clone(), hit))
        })
        .collect()
}

impl<W: notify::Watcher> Inner<W> {
    /// Watches every window's folders, and stops watching the ones no window names.
    fn follow(&mut self) {
        let wanted: HashSet<PathBuf> = self
            .by_window
            .values()
            .flatten()
            .map(|(_, dir)| dir.clone())
            .collect();
        if let Some(watcher) = self.watcher.as_mut() {
            // A folder the platform would not watch is tried again on the next follow, and
            // until then the explorer does not hear its changes. Saying so to the window is
            // #1160's, not this watch's yet.
            let _unwatched = crate::watchset::follow(watcher, &mut self.watched, wanted);
        }
    }
}

/// The folders of branches this window's explorer has expanded, watched until it names others
/// (FM-1). A folder that does not resolve — gone, or refused as the tree refuses it — is not
/// watched.
// Each folder is resolved by `purlis_core::files::Root::resolve`, with the tree's own checks; the
// window never names a directory. Not a doc comment, because the generated bindings carry those.
#[tauri::command]
#[specta::specta]
pub async fn files_watch(
    window: tauri::Window,
    planes: tauri::State<'_, Planes>,
    folders: Vec<BranchFolder>,
) -> Result<(), String> {
    use tauri::Manager as _;
    let ticket = window.state::<FileWatch>().ticket();
    // Each project resolved here, so the blocking half carries paths and not registry handles;
    // and each branch once, with every one of its folders, and no more than a window may watch.
    let mut by_branch: Vec<(PathBuf, Vec<BranchFolder>)> = Vec::new();
    for folder in folders.into_iter().take(purlis_core::files::WATCHED) {
        let Ok(held) = planes.held(&folder.plane) else {
            continue;
        };
        match by_branch
            .iter_mut()
            .find(|(_, of)| same_branch(&of[0], &folder))
        {
            Some((_, of)) => of.push(folder),
            None => by_branch.push((held.root().to_path_buf(), vec![folder])),
        }
    }
    // Off the thread that draws (SC-2): finding a branch's folder waits on the reader's child.
    let (resolved, later) = tauri::async_runtime::spawn_blocking(move || {
        let mut resolved = Vec::new();
        let mut later = Vec::new();
        for (root, of) in by_branch {
            match branch_found(&root, &of) {
                Found::Dirs(dirs) => resolved.extend(of_found(&of, dirs)),
                Found::Later(after) => later.push(Later {
                    root,
                    folders: of,
                    due: Instant::now() + after,
                }),
            }
        }
        (resolved, later)
    })
    .await
    .map_err(|err| format!("watching the branch's folders did not finish: {err}"))?;
    let unwatched: Vec<BranchFolder> = later
        .iter()
        .flat_map(|one| one.folders.iter().cloned())
        .collect();
    let label = window.label().to_string();
    let taken =
        window
            .state::<FileWatch>()
            .set_from(&label, ticket, resolved, unwatched.clone())?;
    if !taken {
        return Ok(());
    }
    say_unwatched(&window, unwatched);
    if !later.is_empty() {
        let window = window.clone();
        // No thread: the folders stay said as unwatched, and the window's next set asks again.
        let _ = std::thread::Builder::new()
            .name("purlis-files-later".into())
            .spawn(move || {
                let watch = window.state::<FileWatch>();
                watch_later(
                    &watch,
                    &label,
                    ticket,
                    later,
                    |one| branch_found(&one.root, &one.folders),
                    |still| say_unwatched(&window, still),
                );
            });
    }
    Ok(())
}

/// Tells `window` which of its folders are not watched now ([`UNWATCHED`]).
fn say_unwatched(window: &tauri::Window, folders: Vec<BranchFolder>) {
    use tauri::Emitter as _;
    let _ = window.emit_to(window.label(), UNWATCHED, FilesChanged { folders });
}

/// The folders `of` of one branch of the project at `root`, found by the bounded reader's
/// child, so this process starts no git for them (#1189).
fn branch_found(root: &std::path::Path, of: &[BranchFolder]) -> Found {
    let first = &of[0];
    let branch = crate::piecefiles::branch(&first.workspace, &first.repo, &first.piece);
    let named: Vec<&str> = of.iter().map(|folder| folder.folder.as_str()).collect();
    folders_found(
        || purlis_core::files::root(&crate::reader(), root, branch),
        &named,
    )
}

/// Each of `of` with the directory it resolved to; one that did not resolve is left out.
fn of_found(
    of: &[BranchFolder],
    dirs: Vec<Result<PathBuf, purlis_core::files::Refused>>,
) -> Vec<(BranchFolder, PathBuf)> {
    of.iter()
        .cloned()
        .zip(dirs)
        .filter_map(|(folder, dir)| dir.ok().map(|dir| (folder, dir)))
        .collect()
}

/// What finding a branch's folders came to.
#[derive(Debug)]
enum Found {
    /// Each folder resolved or refused, in the order they were named; none when the reader
    /// answered that the branch does not resolve.
    Dirs(Vec<Result<PathBuf, purlis_core::files::Refused>>),
    /// The reader gave no answer (#1727): ask again after this long.
    Later(Duration),
}

/// One branch's folders that are not watched yet, asked for again at `due`.
struct Later {
    root: PathBuf,
    folders: Vec<BranchFolder>,
    due: Instant,
}

/// Asks for each of `later` again once it is due, by `find`, and watches what it finds
/// ([`FileWatch::found_late`]), saying the window's folders still unwatched by `said` each time
/// that list changes; for as long as `ticket` is the newest set `window` asked for (#1727).
fn watch_later<W: notify::Watcher + Send + 'static>(
    watch: &FileWatch<W>,
    window: &str,
    ticket: u64,
    mut later: Vec<Later>,
    find: impl Fn(&Later) -> Found,
    said: impl Fn(Vec<BranchFolder>),
) {
    while !later.is_empty() && watch.wants(window, ticket) {
        let now = Instant::now();
        let Some(next) = later.iter().position(|one| one.due <= now) else {
            let first = later.iter().map(|one| one.due).min().unwrap_or(now);
            std::thread::sleep(first.saturating_duration_since(now).min(LOOKED_EVERY));
            continue;
        };
        let one = later.swap_remove(next);
        match find(&one) {
            Found::Later(after) => later.push(Later {
                due: Instant::now() + after,
                ..one
            }),
            Found::Dirs(dirs) => {
                let found = of_found(&one.folders, dirs);
                match watch.found_late(window, ticket, found, &one.folders) {
                    Some(still) => said(still),
                    None => return,
                }
            }
        }
    }
}

/// The `named` folders of the branch `find` finds, each resolved; none when the reader answered
/// that it does not resolve. A find that met every reader place taken is asked again at once
/// ([`crate::branchwatch::BUSY_TRIES`] in all, #1189). One the reader gave no answer for (a
/// pause, a gate busy every time, a read past its bounds) is [`Found::Later`] (#1727): the
/// window asks for its folders only when they change, so a branch left out for that would go
/// unwatched with nothing said.
fn folders_found(
    find: impl FnMut() -> Result<purlis_core::files::Root, purlis_core::files::Refused>,
    named: &[&str],
) -> Found {
    match crate::branchwatch::asked_past_busy(crate::branchwatch::BUSY_TRIES, find) {
        Ok(found) => Found::Dirs(found.resolve(named)),
        Err(why) => match purlis_core::files::Reader::paused_for(&why) {
            Some(left) => Found::Later(left),
            None if no_answer(&why) => Found::Later(AGAIN_AFTER),
            None => Found::Dirs(Vec::new()),
        },
    }
}

/// Whether the reader gave no answer, rather than answering with a refusal
/// ([`purlis_core::files::READ_FAILED`]).
fn no_answer(why: &purlis_core::files::Refused) -> bool {
    matches!(why, purlis_core::files::Refused::Read(said)
        if said.starts_with(purlis_core::files::READ_FAILED))
}

/// Whether two folders are of one branch of one project.
fn same_branch(a: &BranchFolder, b: &BranchFolder) -> bool {
    a.plane == b.plane && a.workspace == b.workspace && a.repo == b.repo && a.piece == b.piece
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::watchset::raw::{Raw, raw};
    use std::path::Path;
    use std::time::Instant;

    const PATIENCE: Duration = Duration::from_secs(10);

    fn busy() -> purlis_core::files::Refused {
        purlis_core::files::Refused::Read(format!(
            "{}it was busy reading other branches for 30 seconds",
            purlis_core::files::READ_FAILED
        ))
    }

    /// #1189: a branch whose folder found every reader place taken is asked again at once, as
    /// the branch watch asks; one the reader refused is asked no more, and nothing is watched.
    #[test]
    fn a_folder_that_found_the_readers_busy_is_asked_again() {
        let mut asks = 0;
        let found = folders_found(
            || {
                asks += 1;
                if asks < 3 {
                    Err(busy())
                } else {
                    Err(purlis_core::files::Refused::Read("gone".into()))
                }
            },
            &["src"],
        );
        assert!(
            matches!(found, Found::Dirs(ref dirs) if dirs.is_empty()),
            "{found:?}"
        );
        assert_eq!(asks, 3);
    }

    /// #1727: a branch the reader gave no answer for is asked for again later rather than
    /// left unwatched for good: once its pause is over when it was paused, else after about a
    /// reader deadline, a gate busy every time included.
    #[test]
    fn a_branch_the_reader_gave_no_answer_for_is_asked_for_later() {
        let paused = || {
            purlis_core::files::Refused::Read(format!(
                "{}its last 2 reads ran past their time or memory, so it is read again in 45 \
                 seconds",
                purlis_core::files::READ_FAILED
            ))
        };
        let mut asks = 0;
        let found = folders_found(
            || {
                asks += 1;
                Err(paused())
            },
            &["src"],
        );
        assert!(
            matches!(found, Found::Later(after) if after == Duration::from_secs(45)),
            "{found:?}"
        );
        assert_eq!(asks, 1, "a pause is not asked again at once");

        let mut asks = 0;
        let never = folders_found(
            || {
                asks += 1;
                Err(busy())
            },
            &["src"],
        );
        assert!(matches!(never, Found::Later(AGAIN_AFTER)), "{never:?}");
        assert_eq!(asks, crate::branchwatch::BUSY_TRIES);

        let hung = folders_found(
            || {
                Err(purlis_core::files::Refused::Read(format!(
                    "{}the read did not finish within 30 seconds",
                    purlis_core::files::READ_FAILED
                )))
            },
            &["src"],
        );
        assert!(matches!(hung, Found::Later(AGAIN_AFTER)), "{hung:?}");
    }

    /// A watch on [`Raw`] whose tells reach the receiver, nothing open yet.
    fn told_raw() -> (FileWatch<Raw>, mpsc::Receiver<(String, Vec<BranchFolder>)>) {
        let (tx, told) = mpsc::channel();
        let tx = Mutex::new(tx);
        let watch = FileWatch::<Raw>::with_config(
            Arc::new(move |window: &str, folders| {
                let _ = tx.lock().unwrap().send((window.to_string(), folders));
            }),
            notify::Config::default(),
        );
        (watch, told)
    }

    /// #1727: a branch the reader paused is said to be unwatched, asked for again once the
    /// pause is over (and again if it is paused once more), then watched and told as moved,
    /// since nothing told the window what changed in it meanwhile; and the window hears that
    /// nothing of its set is unwatched any more.
    #[test]
    fn a_paused_branch_is_watched_once_its_pause_is_over() {
        let src = PathBuf::from("/paused-branch/src");
        let (watch, told) = told_raw();
        let ticket = watch.ticket();
        assert!(
            watch
                .set_from("main", ticket, Vec::new(), vec![folder("src")])
                .unwrap()
        );
        let asks = std::cell::Cell::new(0);
        let said = std::cell::RefCell::new(Vec::new());

        watch_later(
            &watch,
            "main",
            ticket,
            vec![Later {
                root: PathBuf::from("/plane"),
                folders: vec![folder("src")],
                due: Instant::now(),
            }],
            |_| {
                asks.set(asks.get() + 1);
                if asks.get() == 1 {
                    Found::Later(Duration::from_millis(20))
                } else {
                    Found::Dirs(vec![Ok(src.clone())])
                }
            },
            |still| said.borrow_mut().push(still),
        );

        assert_eq!(asks.get(), 2);
        assert_eq!(*said.borrow(), [Vec::<BranchFolder>::new()]);
        assert_eq!(crate::watchset::raw::watched(&src), 1);
        let (window, folders) = told.recv_timeout(PATIENCE).expect("told as moved");
        assert_eq!(window, "main");
        assert_eq!(folders, [folder("src")]);
    }

    /// #1727: once the window asks for another set, a branch of the old one is not asked for
    /// again, and nothing of it is watched.
    #[test]
    fn a_window_that_asked_for_another_set_lets_go_of_the_old_ones_branches() {
        let (watch, told) = told_raw();
        let old = watch.ticket();
        watch
            .set_from("main", old, Vec::new(), vec![folder("src")])
            .unwrap();
        let newer = watch.ticket();
        watch
            .set_from("main", newer, Vec::new(), Vec::new())
            .unwrap();

        watch_later(
            &watch,
            "main",
            old,
            vec![Later {
                root: PathBuf::from("/plane"),
                folders: vec![folder("src")],
                due: Instant::now(),
            }],
            |_| panic!("asked for a branch of a set the window let go of"),
            |_| panic!("said something of a set the window let go of"),
        );

        assert!(
            watch
                .found_late("main", old, Vec::new(), &[folder("src")])
                .is_none()
        );
        assert!(told.recv_timeout(Duration::from_millis(300)).is_err());
    }

    /// The poller on macOS, where FSEvents gives no bound on when it delivers (#577); the
    /// platform's own watcher elsewhere. As `planewatch.rs`'s tests choose.
    #[cfg(target_os = "macos")]
    type Source = notify::PollWatcher;
    #[cfg(not(target_os = "macos"))]
    type Source = notify::RecommendedWatcher;

    fn folder(name: &str) -> BranchFolder {
        BranchFolder {
            plane: serde_json::from_value(serde_json::json!("/plane")).unwrap(),
            workspace: "alpha".to_string(),
            repo: "thing".to_string(),
            piece: Some("piece".to_string()),
            folder: name.to_string(),
        }
    }

    fn watching() -> (
        FileWatch<Source>,
        mpsc::Receiver<(String, Vec<BranchFolder>)>,
    ) {
        let (tx, rx) = mpsc::channel();
        let tx = Mutex::new(tx);
        let watch = FileWatch::<Source>::with_config(
            Arc::new(move |window: &str, folders| {
                let _ = tx.lock().unwrap().send((window.to_string(), folders));
            }),
            notify::Config::default().with_poll_interval(Duration::from_millis(50)),
        );
        (watch, rx)
    }

    #[test]
    fn a_file_added_to_an_expanded_folder_tells_the_window_that_expanded_it() {
        let dir = tempfile::tempdir().unwrap();
        let src = std::fs::canonicalize(dir.path()).unwrap().join("src");
        std::fs::create_dir_all(&src).unwrap();
        let (watch, told) = watching();
        watch
            .set("main", vec![(folder("src"), src.clone())])
            .unwrap();
        // The poller's first look is its baseline; give it one.
        std::thread::sleep(Duration::from_millis(200));

        std::fs::write(src.join("new.rs"), "\n").unwrap();

        let (window, folders) = told.recv_timeout(PATIENCE).expect("the window was told");
        assert_eq!(window, "main");
        assert_eq!(folders, [folder("src")]);
    }

    #[test]
    fn a_set_resolved_late_never_replaces_one_asked_for_after_it() {
        let dir = tempfile::tempdir().unwrap();
        let src = std::fs::canonicalize(dir.path()).unwrap().join("src");
        std::fs::create_dir_all(&src).unwrap();
        let (watch, told) = watching();
        let (older, newer) = (watch.ticket(), watch.ticket());
        watch
            .set_from(
                "main",
                newer,
                vec![(folder("src"), src.clone())],
                Vec::new(),
            )
            .unwrap();
        watch
            .set_from("main", older, Vec::new(), vec![folder("src")])
            .unwrap();
        std::thread::sleep(Duration::from_millis(200));

        std::fs::write(src.join("new.rs"), "\n").unwrap();

        let (_, folders) = told
            .recv_timeout(PATIENCE)
            .expect("the newer set is watched");
        assert_eq!(folders, [folder("src")]);
    }

    #[test]
    fn a_folder_no_window_names_any_more_tells_nobody() {
        let dir = tempfile::tempdir().unwrap();
        let src = std::fs::canonicalize(dir.path()).unwrap().join("src");
        std::fs::create_dir_all(&src).unwrap();
        let (watch, told) = watching();
        watch
            .set("main", vec![(folder("src"), src.clone())])
            .unwrap();
        watch.forget("main");
        std::thread::sleep(Duration::from_millis(200));

        std::fs::write(src.join("new.rs"), "\n").unwrap();

        assert!(told.recv_timeout(Duration::from_secs(2)).is_err());
    }

    /// A watch on [`Raw`], with `src` open in window `main`.
    fn watching_raw(src: &Path) -> (FileWatch<Raw>, mpsc::Receiver<(String, Vec<BranchFolder>)>) {
        let (tx, told) = mpsc::channel();
        let tx = Mutex::new(tx);
        let watch = FileWatch::<Raw>::with_config(
            Arc::new(move |window: &str, folders| {
                let _ = tx.lock().unwrap().send((window.to_string(), folders));
            }),
            notify::Config::default(),
        );
        watch
            .set("main", vec![(folder("src"), src.to_path_buf())])
            .unwrap();
        (watch, told)
    }

    #[test]
    fn a_file_removed_soon_after_it_was_made_is_still_told() {
        // The window lists a folder whenever it is told about it, and any other change can
        // tell it: so it may have drawn a file whose creation the watch has not passed on
        // yet. If the watch then took the file's removal as cancelling its creation, as a
        // debouncer that folds a create and a remove into nothing does, the window would
        // never hear of either and would draw the file for ever. inotify reports the two
        // separately, so this happened on Linux, where an e2e spec removes a file it has just
        // seen drawn (#1138).
        use notify::event::{CreateKind, EventKind, RemoveKind};
        let src = PathBuf::from("/branch/src");
        let (_watch, told) = watching_raw(&src);

        raw(EventKind::Create(CreateKind::File), &src.join("brief.md"));
        raw(EventKind::Remove(RemoveKind::File), &src.join("brief.md"));

        let (window, folders) = told
            .recv_timeout(PATIENCE)
            .expect("the window was told the folder moved");
        assert_eq!(window, "main");
        assert_eq!(folders, [folder("src")]);
    }

    #[test]
    fn a_read_of_the_folder_tells_nobody() {
        // The window reads a folder when told, and inotify reports that read: told about it,
        // the window would read again because it read, for ever.
        use notify::event::{AccessKind, AccessMode, EventKind};
        let src = PathBuf::from("/branch/src");
        let (_watch, told) = watching_raw(&src);

        raw(EventKind::Access(AccessKind::Open(AccessMode::Any)), &src);

        assert!(told.recv_timeout(QUIET_FOR * 4).is_err());
    }

    #[test]
    fn a_platform_that_lost_track_tells_every_open_folder() {
        // inotify's queue overflowed: anything may have changed, so every open folder is read
        // again rather than none.
        let src = PathBuf::from("/branch/src");
        let (_watch, told) = watching_raw(&src);

        crate::watchset::raw::event(
            notify::Event::new(notify::EventKind::Other).set_flag(notify::event::Flag::Rescan),
        );

        let (window, folders) = told.recv_timeout(PATIENCE).expect("the window was told");
        assert_eq!(window, "main");
        assert_eq!(folders, [folder("src")]);
    }

    #[test]
    fn a_sustained_flood_is_still_told_once_its_burst_is_over() {
        // A burst ends QUIET_FOR after its first event, however many more are queued: an agent
        // writing without pause must not keep the window from ever hearing of it (the burst
        // only ended on a quiet moment, and a flood of 12.3M events never had one).
        use notify::event::{CreateKind, EventKind};
        let src = PathBuf::from("/branch/src");
        let (_watch, told) = watching_raw(&src);
        let started = Instant::now();
        let mut first_tell = None;
        let mut n: u64 = 0;
        while started.elapsed() < Duration::from_secs(3) {
            raw(
                EventKind::Create(CreateKind::File),
                &src.join(format!("f{n}")),
            );
            n += 1;
            if first_tell.is_none() && told.try_recv().is_ok() {
                first_tell = Some(started.elapsed());
            }
        }
        assert!(
            first_tell.is_some_and(|t| t < Duration::from_secs(1)),
            "no tell within 1 s of a flood's start: {first_tell:?} after {n} events"
        );
    }

    #[test]
    fn a_burst_that_is_everything_tells_every_open_folder() {
        // Past MOST_PATHS a burst is everything (`watchset`'s tests hold it to that), and
        // every open folder is read again rather than none.
        let src = PathBuf::from("/branch/src");
        let watch =
            FileWatch::<Raw>::with_config(Arc::new(|_: &str, _| {}), notify::Config::default());
        watch.set("main", vec![(folder("src"), src)]).unwrap();
        let everything = Burst {
            everything: true,
            ..Burst::default()
        };
        assert_eq!(
            concerning(&everything, &watch.inner),
            [("main".to_string(), vec![folder("src")])]
        );
    }
}
