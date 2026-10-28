// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Files across a chat's browser's edge, as the window decides them** (#2020, spec #2008).
//!
//! - **A download lands in the browser's holding folder**, in the browser's own data folder
//!   under `<data>/browsers`, which no chat reads or writes. The window keeps what each answer
//!   said of the browser's downloads ([`Files::heard`]), by the origin its engine said it came
//!   from.
//! - **Saving one into the chat's folder needs the person's yes for its origin, for that chat**
//!   ([`Files::prepared`]). Until the window's prompt for it is built (designed first, by the
//!   person's ruling of 2026-10-28), nothing answers yes: every save is refused, in words that
//!   say the download stays in the browser, so a download never lands in a chat's folder
//!   unasked. Where a yes is given, a save goes only to a path of the chat's folder that its
//!   rules let it write ([`purlis_core::brokered::guard`]: never what a program run later
//!   loads, never a manifest) and that purlis keeps from no chat; the host then writes it as a
//!   new file, following no link ([`purlis_core::browserfiles::bring`]).
//! - **An upload comes from the chat's folder alone, under its read rules**: each file a path
//!   relative to it, none that purlis keeps from every chat
//!   ([`purlis_core::sandbox::kept_from_reading`]). The window names the folder, its own record
//!   of where the chat runs, never the chat's word; the host reads each file following no link
//!   and hands the engine a copy ([`purlis_core::browserfiles::stage`]).
//! - **A runner chat's browser takes no files yet**: its folder is on the runner, which this
//!   window does not name (refused in words; #3696 brings it).
//!
//! **Nothing a chat sends decides any of it**: its folder is the app's record of it, and the
//! origin of a download is what its browser's engine said.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

use purlis_session_protocol::browsing::{Act, DownloadSaid, DownloadState};

/// What a chat is told of a save from an origin the person has not allowed it (#2020).
pub fn save_not_allowed_said(origin: &str) -> String {
    format!(
        "saving downloads from {origin} into this chat's folder needs the person's yes, which \
         this purlis does not ask for yet, so nothing was saved: the download stays in the \
         browser. Go on without it, and tell the person its name if the task needs it"
    )
}

/// What a runner chat is told of an upload or a save (#2020).
pub const NOT_ON_A_RUNNER: &str = "this chat's browser runs on a pool's runner, where purlis does \
     not yet upload from or save into a chat's folder: go on without it, and tell the person if \
     the task needs it";

/// **Where a chat runs, as the window knows it** (#2020): its folder and its project's root, each
/// as the kernel names it, and what purlis keeps from every chat to read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatFolder {
    pub folder: PathBuf,
    pub root: PathBuf,
    pub kept: Vec<PathBuf>,
}

/// What becomes of an act of a chat's that names its folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Prepared {
    /// Sent on to the host, naming the chat's folder.
    Go(Act),
    /// Refused, in words that say what to do.
    No(String),
}

/// The window's record of its chats' browsers' files (#2020). In memory: a relaunch starts with
/// none.
#[derive(Debug, Default)]
pub struct Files {
    /// What each browser's answers said of its downloads, by program, oldest first.
    downloads: Mutex<HashMap<String, Vec<DownloadSaid>>>,
    /// The origins each chat may save downloads from, by chat: the person's yes.
    allowed: Mutex<HashMap<u32, Vec<String>>>,
}

impl Files {
    /// **Browser `program` said `downloads`** in an answer: each kept as it is now.
    pub fn heard(&self, program: &str, downloads: &[DownloadSaid]) {
        if downloads.is_empty() {
            return;
        }
        let mut all = lock(&self.downloads);
        let kept = all.entry(program.to_owned()).or_default();
        for one in downloads {
            match kept.iter_mut().find(|had| had.download == one.download) {
                Some(had) => *had = one.clone(),
                None => kept.push(one.clone()),
            }
        }
    }

    /// Browser `program`'s downloads, as its answers last said them.
    fn of(&self, program: &str) -> Vec<DownloadSaid> {
        lock(&self.downloads)
            .get(program)
            .cloned()
            .unwrap_or_default()
    }

    /// Browser `program` has ended: its downloads went with its folder.
    pub fn ended(&self, program: &str) {
        lock(&self.downloads).remove(program);
    }

    /// Chat `session` has ended: what it was allowed goes with it.
    pub fn chat_ended(&self, session: u32) {
        lock(&self.allowed).remove(&session);
    }

    /// The person said yes to chat `session` saving downloads from `origin` (#2020): what the
    /// window's prompt will call once it is built. Tests only until then.
    #[cfg(test)]
    pub fn allow_saves(&self, session: u32, origin: &str) {
        lock(&self.allowed)
            .entry(session)
            .or_default()
            .push(origin.to_owned());
    }

    /// **What becomes of `act`, chat `session`'s on browser `program`** (#2020), its folder as
    /// `folder` says, on a pool's runner where `on_a_runner`. An act that names no folder goes
    /// as it is.
    pub fn prepared(
        &self,
        session: u32,
        program: &str,
        act: Act,
        folder: impl FnOnce() -> Result<ChatFolder, String>,
        on_a_runner: bool,
    ) -> Prepared {
        match act {
            Act::Upload { target, files, .. } => {
                if on_a_runner {
                    return Prepared::No(NOT_ON_A_RUNNER.to_owned());
                }
                let at = match folder() {
                    Ok(at) => at,
                    Err(why) => return Prepared::No(why),
                };
                for file in &files {
                    if let Err(why) = readable(&at, file) {
                        return Prepared::No(why);
                    }
                }
                Prepared::Go(Act::Upload {
                    target,
                    files,
                    from: Some(at.folder.display().to_string()),
                })
            }
            Act::SaveDownload { download, to, .. } => {
                if on_a_runner {
                    return Prepared::No(NOT_ON_A_RUNNER.to_owned());
                }
                let Some(one) = self
                    .of(program)
                    .into_iter()
                    .find(|one| one.download == download)
                else {
                    return Prepared::No(format!(
                        "this browser has no download {download}: list its downloads with \
                         browser_downloads"
                    ));
                };
                if one.state != DownloadState::Done {
                    return Prepared::No(format!(
                        "download {download} is not done, so it is not saved: list its \
                         downloads with browser_downloads to see how far it has come"
                    ));
                }
                let Some(origin) = one.origin else {
                    return Prepared::No(format!(
                        "download {download} came from no web origin, so purlis saves nothing \
                         of it"
                    ));
                };
                let allowed = lock(&self.allowed)
                    .get(&session)
                    .is_some_and(|all| all.contains(&origin));
                if !allowed {
                    return Prepared::No(save_not_allowed_said(&origin));
                }
                let at = match folder() {
                    Ok(at) => at,
                    Err(why) => return Prepared::No(why),
                };
                if let Err(why) = writable(&at, &to) {
                    return Prepared::No(why);
                }
                Prepared::Go(Act::SaveDownload {
                    download,
                    to,
                    into: Some(at.folder.display().to_string()),
                })
            }
            act => Prepared::Go(act),
        }
    }
}

/// Whether `written`, a path of the chat's folder, is one it may read: in the folder, and
/// nothing purlis keeps from every chat. The host opens it following no link.
fn readable(at: &ChatFolder, written: &str) -> Result<(), String> {
    let names = purlis_core::browserfiles::relative(written)?;
    let path = joined(&at.folder, &names);
    if kept(&path, &at.kept) {
        return Err(format!(
            "{written} is a file purlis keeps from every chat, so it is not uploaded"
        ));
    }
    Ok(())
}

/// Whether `written` is a path of the chat's folder a download may be saved to: in the folder,
/// nothing purlis keeps from every chat, and a place the chat's rules let it write.
fn writable(at: &ChatFolder, written: &str) -> Result<(), String> {
    let names = purlis_core::browserfiles::relative(written)?;
    let path = joined(&at.folder, &names);
    if kept(&path, &at.kept) {
        return Err(format!(
            "{written} is where purlis keeps what no chat reads, so nothing is saved there"
        ));
    }
    purlis_core::brokered::guard(&at.root, &path, None)
}

fn joined(folder: &Path, names: &[String]) -> PathBuf {
    let mut path = folder.to_path_buf();
    path.extend(names);
    path
}

/// Whether `path` is at or below one of `kept`, compared without regard to case: a disk that
/// folds names opens either spelling.
fn kept(path: &Path, kept: &[PathBuf]) -> bool {
    let folded = |path: &Path| path.to_string_lossy().to_lowercase();
    let path = folded(path);
    kept.iter().any(|one| {
        let one = folded(one);
        path == one || path.starts_with(&format!("{}/", one.trim_end_matches('/')))
    })
}

/// **Where chat `session` of `held` runs** (#2020): its folder, as the app recorded it at its
/// start, and its project's root, each as the kernel names them; and what purlis keeps from
/// every chat to read. Refused, in words, where any of it cannot be told.
pub fn chat_folder(held: &crate::planes::Held, session: u32) -> Result<ChatFolder, String> {
    let root = held.root().to_path_buf();
    let folder = held
        .chats()
        .folder_of(session)
        .unwrap_or_else(|| root.clone());
    let real = |path: &Path| {
        path.canonicalize()
            .map_err(|_| "purlis could not find this chat's folder, so no file crosses".to_owned())
    };
    let (folder, root) = (real(&folder)?, real(&root)?);
    if !folder.starts_with(&root) {
        return Err("this chat runs outside its project, so no file crosses".to_owned());
    }
    let kept =
        purlis_core::sandbox::kept_from_reading(&root, &purlis_core::sandbox::Machine::this())
            .map_err(|why| {
                format!(
                    "purlis could not tell what it keeps from chats ({why}), so no file crosses"
                )
            })?;
    Ok(ChatFolder { folder, root, kept })
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests;
