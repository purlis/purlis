// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A project's Claude Code settings, screened again when they change** (ADR 0067 as amended
//! 2026-10-16).
//!
//! A sandboxed Claude Code chat loads its project's own settings files only where they hold
//! what a chat keeps (`purlis_core::harness::claude::project`). That is decided at its start,
//! and the files can change while it runs: a pull or a save of the window's, or the person's
//! own editor. So each such chat's files are watched from before it starts, and screened again
//! on every change. One that no longer passes is told ([`Told`]): the chat is owed a restart
//! on its conversation, which starts it on the person's settings and purlis's, with the same
//! Notice a start gives. One that still passes is left as it is.
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
const SETTLED_AT_MOST: std::time::Duration = std::time::Duration::from_secs(2);

/// The event the window hears a chat's project settings changed on.
pub const EVENT: &str = "chat-project-settings-changed";

/// **A chat whose project's Claude Code settings changed so that it no longer loads them**, as
/// the window hears it: it is owed a restart, which the window drives at once.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct Rescreened {
    pub plane: PlaneId,
    pub session: u32,
}

/// Told each [`Rescreened`], as the window shows it.
pub type Teller = Arc<dyn Fn(Rescreened) + Send + Sync + 'static>;

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
    /// Each folder listened to.
    listened: BTreeSet<PathBuf>,
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
                    notify::recommended_watcher(move |_: notify::Result<notify::Event>| {
                        let _ = changed.send(());
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
            if inner.listened.contains(&folder) || !folder.is_dir() {
                continue;
            }
            let Some(watcher) = inner.watcher.as_mut() else {
                return Err("no watcher could be made".to_owned());
            };
            watcher
                .watch(&folder, RecursiveMode::NonRecursive)
                .map_err(|why| format!("{} cannot be watched ({why})", folder.display()))?;
            inner.listened.insert(folder);
        }
        Ok(())
    }

    /// Chat `session`, started in `cwd` on its project's files, is watched from now: and
    /// screened at once, so a change since [`Self::prepare`] is not missed.
    pub fn chat(&self, session: u32, cwd: &Path) {
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
        let gone: Vec<PathBuf> = inner.listened.difference(&needed).cloned().collect();
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
