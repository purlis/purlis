// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A project's Claude Code settings, screened again when they change** (ADR 0067 as amended
//! 2026-10-16).
//!
//! A sandboxed Claude Code chat loads its project's own settings files only where they hold
//! what a chat keeps (`purlis_core::harness::claude::project`). That is decided at its start,
//! and the files can change while it runs: a pull or a save of the window's, or the person's
//! own editor. So each such chat's files are watched from before it starts, and screened again
//! on every change. One that no longer passes is told ([`Told`]): the core starts the chat again
//! on its conversation at once, on the person's settings and purlis's, with the same Notice a
//! start gives, or ends it where it cannot be started again; the window only redraws. One that
//! still passes is left as it is.
//!
//! **Fail closed.** Where the watch cannot be set up, the chat is not started on its project's
//! files at all ([`Watch::prepare`] refuses, and the start limits it).
//!
//! The folders watched are the ones holding each screened file and the folders holding those,
//! one level each and never below, so a `.claude` made later is seen and then watched itself.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use notify::{RecursiveMode, Watcher};
use purlis_core::harness::claude::project;

use crate::planes::PlaneId;

/// How long the files are left quiet before they are screened again after a change.
const SETTLING: std::time::Duration = std::time::Duration::from_millis(150);

/// The longest a run of changes is waited out before the files are screened anyway.
const SETTLED_AT_MOST: std::time::Duration = std::time::Duration::from_millis(500);

/// The names whose change is screened: a settings folder and the files in it, in any case.
const NAMES: [&str; 3] = [".claude", "settings.json", "settings.local.json"];

/// Whether `event` is one the files are screened again for: a change to a name of [`NAMES`],
/// never a read or an open. An error says nothing of what changed, so it is screened for.
fn screened_for(event: &notify::Result<notify::Event>) -> bool {
    let Ok(event) = event else {
        return true;
    };
    if matches!(event.kind, notify::EventKind::Access(_)) {
        return false;
    }
    event.paths.is_empty()
        || event.paths.iter().any(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| NAMES.iter().any(|one| one.eq_ignore_ascii_case(name)))
        })
}

/// What a folder listened to is on disk: its device and inode where the system says, so a folder
/// taken away and made again is listened to again.
fn identity(folder: &Path) -> Option<(u64, u64)> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        std::fs::metadata(folder)
            .ok()
            .map(|meta| (meta.dev(), meta.ino()))
    }
    #[cfg(not(unix))]
    {
        std::fs::metadata(folder).ok().map(|_| (0, 0))
    }
}

/// The event the window hears a chat's project settings changed on.
pub const EVENT: &str = "chat-project-settings-changed";

/// **A chat the core started again, or ended, because its project's Claude Code settings no
/// longer pass**, as the window redraws it.
#[derive(serde::Serialize, specta::Type)]
pub(crate) struct Rescreened {
    pub(crate) plane: PlaneId,
    /// The chat as it was.
    pub(crate) session: u32,
    /// The chat started again in its place, as the window draws it; none where it was ended.
    pub(crate) chat: Option<crate::OpenChat>,
    /// What the new run's start found to say on its tab.
    pub(crate) notices: Vec<String>,
    /// Why it could not be started again, and was ended: said on its tab.
    pub(crate) ended: Option<String>,
}

/// Told each [`Rescreened`], as the window shows it.
pub(crate) type Teller = Arc<dyn Fn(Rescreened) + Send + Sync + 'static>;

/// Told the number of each chat whose project's settings no longer pass.
pub type Told = Arc<dyn Fn(u32) + Send + Sync + 'static>;

/// One chat watched: the folder it runs in.
struct Watched {
    cwd: PathBuf,
}

#[derive(Default)]
struct Inner {
    watcher: Option<Box<dyn Watcher + Send>>,
    /// Whether a watcher could not be made: every prepare is refused then.
    unmade: bool,
    chats: HashMap<u32, Watched>,
    /// Each folder listened to, and what it was on disk when it was.
    listened: HashMap<PathBuf, Option<(u64, u64)>>,
    /// Run as a chat is watched, before it is screened: for a test to change its files then.
    #[cfg(test)]
    before_chat: Option<Box<dyn Fn() + Send>>,
}

/// The watch of every open sandboxed Claude Code chat's project settings files.
#[derive(Clone)]
pub struct Watch {
    inner: Arc<Mutex<Inner>>,
    told: Arc<Mutex<Option<Told>>>,
}

impl Default for Watch {
    fn default() -> Self {
        Self::new()
    }
}

impl Watch {
    /// A watch on the platform's own watcher, made at the first chat it is asked for.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner::default())),
            told: Arc::new(Mutex::new(None)),
        }
    }

    /// A watch that cannot watch anything: what a machine whose watcher cannot be made has.
    #[cfg(test)]
    pub fn unable() -> Self {
        let watch = Self::new();
        watch.lock().unmade = true;
        watch
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Runs `before` once, as the next chat is watched and before it is screened.
    #[cfg(test)]
    pub fn changing_before_the_next_chat(&self, before: impl Fn() + Send + 'static) {
        self.lock().before_chat = Some(Box::new(before));
    }

    /// Who is told, from now on, each chat whose project settings no longer pass.
    pub fn telling(&self, told: Told) {
        *self.told.lock().unwrap_or_else(PoisonError::into_inner) = Some(told);
    }

    /// Listens to the folders the settings of a chat in `cwd` are in, before it starts.
    /// Refused where no watch can be set up, and the chat is then started without them.
    pub fn prepare(&self, cwd: &Path) -> Result<(), String> {
        let mut inner = self.lock();
        if inner.watcher.is_none() && !inner.unmade {
            // The watcher's own thread only says something changed: a watch added from it would
            // wait on that same thread. Another thread screens, holding the watch weakly, so the
            // watcher the watch holds does not hold the watch. Every chat is screened again on
            // any change: a few small files each, and an event may name a folder by another
            // spelling than the chat's.
            let (changed, heard) = std::sync::mpsc::channel::<()>();
            let (inner_of, told) = (Arc::downgrade(&self.inner), Arc::clone(&self.told));
            let screening = std::thread::Builder::new()
                .name("purlis-project-settings".into())
                .spawn(move || {
                    while heard.recv().is_ok() {
                        // Settled first: a file being saved is often seen empty or half
                        // written for a moment, which is not what it holds.
                        let until = std::time::Instant::now() + SETTLED_AT_MOST;
                        loop {
                            std::thread::sleep(SETTLING);
                            let mut more = false;
                            while heard.try_recv().is_ok() {
                                more = true;
                            }
                            if !more || std::time::Instant::now() >= until {
                                break;
                            }
                        }
                        let Some(inner) = inner_of.upgrade() else {
                            return;
                        };
                        Watch {
                            inner,
                            told: Arc::clone(&told),
                        }
                        .screen_all();
                    }
                });
            let made = screening
                .map_err(|why| notify::Error::generic(&why.to_string()))
                .and_then(|_| {
                    notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                        if screened_for(&event) {
                            let _ = changed.send(());
                        }
                    })
                });
            match made {
                Ok(watcher) => inner.watcher = Some(Box::new(watcher)),
                Err(why) => {
                    tracing::warn!("purlis: project settings cannot be watched ({why})");
                    inner.unmade = true;
                }
            }
        }
        if inner.unmade {
            return Err("no watcher could be made".to_owned());
        }
        Self::listen(&mut inner, cwd)
    }

    /// Listens to every folder `cwd`'s settings files are in, and the one above each.
    fn listen(inner: &mut Inner, cwd: &Path) -> Result<(), String> {
        for folder in folders(cwd) {
            let now = identity(&folder);
            if now.is_none() {
                // Gone: listened to afresh when it is made again.
                inner.listened.remove(&folder);
                continue;
            }
            // inotify lets go of a folder's watch when the folder goes, and a folder made again
            // may take the same inode, so on Linux each is listened to again at every screen,
            // which inotify takes as a no-op while its watch stands. Elsewhere, a folder is
            // listened to again when it is not what it was.
            let again = cfg!(target_os = "linux");
            match inner.listened.get(&folder) {
                Some(was) if *was == now && !again => continue,
                Some(was) if *was != now => {
                    if let Some(watcher) = inner.watcher.as_mut() {
                        let _ = watcher.unwatch(&folder);
                    }
                }
                _ => {}
            }
            let Some(watcher) = inner.watcher.as_mut() else {
                return Err("no watcher could be made".to_owned());
            };
            watcher
                .watch(&folder, RecursiveMode::NonRecursive)
                .map_err(|why| format!("{} cannot be watched ({why})", folder.display()))?;
            inner.listened.insert(folder, now);
        }
        Ok(())
    }

    /// Chat `session`, started in `cwd` on its project's files, is watched from now: and
    /// screened at once, so a change since [`Self::prepare`] is not missed.
    pub fn chat(&self, session: u32, cwd: &Path) {
        #[cfg(test)]
        {
            let before = self.lock().before_chat.take();
            if let Some(before) = before {
                before();
            }
        }
        self.lock().chats.insert(
            session,
            Watched {
                cwd: cwd.to_path_buf(),
            },
        );
        self.screen(&[session]);
    }

    /// Chat `session` is watched no more, and folders no other chat needs are let go of.
    pub fn forget(&self, session: u32) {
        let mut inner = self.lock();
        if inner.chats.remove(&session).is_none() {
            return;
        }
        let needed: BTreeSet<PathBuf> = inner
            .chats
            .values()
            .flat_map(|chat| folders(&chat.cwd))
            .collect();
        let gone: Vec<PathBuf> = inner
            .listened
            .keys()
            .filter(|folder| !needed.contains(*folder))
            .cloned()
            .collect();
        for folder in gone {
            if let Some(watcher) = inner.watcher.as_mut() {
                let _ = watcher.unwatch(&folder);
            }
            inner.listened.remove(&folder);
        }
    }

    /// The chats watched, by number.
    #[cfg(test)]
    pub fn watched(&self) -> Vec<u32> {
        let mut out: Vec<u32> = self.lock().chats.keys().copied().collect();
        out.sort_unstable();
        out
    }

    /// Screens every chat watched again, as a change anywhere would.
    pub fn screen_all(&self) {
        let sessions: Vec<u32> = self.lock().chats.keys().copied().collect();
        self.screen(&sessions);
    }

    /// Screens `sessions` again: a `.claude` made since is listened to first, then each chat's
    /// files are read. One that no longer passes is watched no more, and told.
    fn screen(&self, sessions: &[u32]) {
        let mut failed = Vec::new();
        for &session in sessions {
            let Some(cwd) = self.lock().chats.get(&session).map(|chat| chat.cwd.clone()) else {
                continue;
            };
            // Listened to before it is read, so what is written after the read is heard.
            let listened = Self::listen(&mut self.lock(), &cwd);
            if listened.is_err() || !project::screen(Some(&cwd)).loads_the_project() {
                failed.push(session);
            }
        }
        if failed.is_empty() {
            return;
        }
        let told = self
            .told
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        for session in failed {
            self.forget(session);
            if let Some(told) = &told {
                told(session);
            }
        }
    }
}

/// The folders a chat in `cwd` has its project settings files in, and the one above each.
fn folders(cwd: &Path) -> BTreeSet<PathBuf> {
    project::files(cwd)
        .into_iter()
        .flat_map(|(file, _)| {
            let claude = file.parent().map(Path::to_path_buf);
            let above = claude
                .as_deref()
                .and_then(Path::parent)
                .map(Path::to_path_buf);
            [claude, above]
        })
        .flatten()
        .collect()
}

#[cfg(test)]
mod tests;
