// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Files across a chat's browser's edge, as the window decides them** (#2020, spec #2008).
//!
//! - **A download lands in the browser's holding folder**, in the browser's own data folder
//!   under `<data>/browsers`, which no chat reads or writes. The window keeps what each answer
//!   said of the browser's downloads ([`Files::heard`]), and its row lists them with Open and
//!   Show in folder for the person ([`open_browser_download`], [`show_browser_download`]): by a
//!   second name, the page's made plain, and never Open for one that opens as a program
//!   ([`opens_as_a_program`]).
//! - **Saving one into the chat's folder asks once per origin** ([`Files::prepared`]): the
//!   first save of a download from an origin the person has not allowed for that chat saves
//!   nothing and waits as one Ask in the registry (`crate::asking::browser_saves`), with Allow
//!   for this chat and Keep out. An Allow lasts while the app holds the chat, as the chat's
//!   other browser Asks do. A save goes only to a path of the chat's folder that its rules let
//!   it write ([`purlis_core::brokered::guard`]: never what a program run later loads, never a
//!   manifest) and that purlis keeps from no chat; the host then writes it as a new file,
//!   following no link ([`purlis_core::browserfiles::bring`]).
//! - **An upload comes from the chat's folder alone, under its read rules**: each file a path
//!   relative to it, none that purlis keeps from every chat ([`purlis_core::sandbox::kept_from_reading`]).
//!   The window names the folder, its own record of where the chat runs, never the chat's word;
//!   the host reads each file following no link and hands the engine a copy
//!   ([`purlis_core::browserfiles::stage`]).
//! - **A runner chat's browser takes no files yet**: its folder is on the runner, which this
//!   window does not name (refused in words; a later change brings it).
//!
//! **Nothing a chat sends decides any of it**: its folder is the app's record of it, the origin
//! of a download is what its browser's engine said, and only the person's press answers the Ask
//! (`ui::WINDOW_ONLY`).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

use purlis_session_protocol::browsing::{Act, DownloadSaid, DownloadState};

/// What a chat is told when its save waits on the person (#2020).
pub fn save_asked_said(origin: &str) -> String {
    format!(
        "purlis asked the person whether this chat may save downloads from {origin} into its \
         folder, and saved nothing: you will be told their answer on your next turn. Call \
         browser_save_download again then; go on with what needs no download meanwhile"
    )
}

/// What a chat is told while its save still waits on the person.
pub fn save_waits_said(origin: &str) -> String {
    format!(
        "the person has not answered yet whether this chat may save downloads from {origin}, so \
         nothing was saved: go on with what needs no download, and call again once they answer"
    )
}

/// What a chat is told once the person kept downloads from an origin out of its folder.
pub fn save_kept_out_said(origin: &str) -> String {
    format!(
        "the person keeps downloads from {origin} out of this chat's folder: don't retry or work \
         around it"
    )
}

/// What a runner chat is told of an upload or a save (#2020).
pub const NOT_ON_A_RUNNER: &str = "this chat's browser runs on a pool's runner, where purlis does \
     not yet upload from or save into a chat's folder: go on without it, and tell the person if \
     the task needs it";

/// The most origins one chat's saves wait on the person for at once.
const MOST_ASKED: usize = 16;

/// **Where a chat runs, as the window knows it** (#2020): its folder and its project's root, each
/// as the kernel names it, and what purlis keeps from every chat to read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatFolder {
    pub folder: PathBuf,
    pub root: PathBuf,
    pub kept: Vec<PathBuf>,
}

/// One chat's saves (#2020): the origins allowed and kept out for it, and those that wait.
#[derive(Debug, Default)]
struct Saving {
    allowed: Vec<String>,
    kept_out: Vec<String>,
    asked: Vec<String>,
    since: u64,
}

/// One chat whose saves wait on the person (#2020): when the first began to wait, and each
/// origin, oldest first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavesWait {
    pub session: u32,
    pub since: u64,
    pub origins: Vec<String>,
}

/// What becomes of an act of a chat's that names its folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Prepared {
    /// Sent on to the host, naming the chat's folder.
    Go(Act),
    /// Refused, in words that say what to do.
    No(String),
    /// Saved nothing: the person is asked now ([`save_asked_said`]).
    Asked(String),
}

/// The window's record of its chats' browsers' files (#2020). In memory, like every Ask a chat
/// raises: a relaunch starts with none.
#[derive(Debug, Default)]
pub struct Files {
    /// What each browser's answers said of its downloads, by program, oldest first.
    downloads: Mutex<HashMap<String, Vec<DownloadSaid>>>,
    chats: Mutex<HashMap<u32, Saving>>,
}

impl Files {
    /// **Browser `program` said `downloads`** in an answer: each kept as it is now. Answers
    /// whether anything changed.
    pub fn heard(&self, program: &str, downloads: &[DownloadSaid]) -> bool {
        let mut all = lock(&self.downloads);
        let kept = all.entry(program.to_owned()).or_default();
        let mut changed = false;
        for one in downloads {
            match kept.iter_mut().find(|had| had.download == one.download) {
                Some(had) if had == one => {}
                Some(had) => {
                    *had = one.clone();
                    changed = true;
                }
                None => {
                    kept.push(one.clone());
                    changed = true;
                }
            }
        }
        changed
    }

    /// Browser `program`'s downloads, as its answers last said them.
    pub fn of(&self, program: &str) -> Vec<DownloadSaid> {
        lock(&self.downloads)
            .get(program)
            .cloned()
            .unwrap_or_default()
    }

    /// Browser `program` has ended: its downloads went with its folder.
    pub fn ended(&self, program: &str) {
        lock(&self.downloads).remove(program);
    }

    /// Chat `session` has ended: what was allowed it goes, and its Ask with it.
    pub fn chat_ended(&self, session: u32) {
        lock(&self.chats).remove(&session);
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
        now: u64,
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
                        "download {download} came from no web origin, so purlis cannot ask the \
                         person about it and saves nothing"
                    ));
                };
                if let Some(waits) = self.gate(session, &origin, now) {
                    return waits;
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

    /// Whether chat `session` may save downloads from `origin`: none where it may, else what
    /// its save comes to, asking the person where nobody has yet.
    fn gate(&self, session: u32, origin: &str, now: u64) -> Option<Prepared> {
        let mut chats = lock(&self.chats);
        let saving = chats.entry(session).or_default();
        if saving.allowed.iter().any(|one| one == origin) {
            return None;
        }
        if saving.kept_out.iter().any(|one| one == origin) {
            return Some(Prepared::No(save_kept_out_said(origin)));
        }
        if saving.asked.iter().any(|one| one == origin) {
            return Some(Prepared::No(save_waits_said(origin)));
        }
        if saving.asked.len() >= MOST_ASKED {
            return Some(Prepared::No(
                "the person has many of this chat's saves to answer already: go on with what \
                 needs no download"
                    .to_owned(),
            ));
        }
        if saving.asked.is_empty() {
            saving.since = now;
        }
        saving.asked.push(origin.to_owned());
        Some(Prepared::Asked(save_asked_said(origin)))
    }

    /// **Every chat whose saves wait on the person**, longest waiting first.
    pub fn saves_waiting(&self) -> Vec<SavesWait> {
        let mut waits: Vec<SavesWait> = lock(&self.chats)
            .iter()
            .filter(|(_, saving)| !saving.asked.is_empty())
            .map(|(session, saving)| SavesWait {
                session: *session,
                since: saving.since,
                origins: saving.asked.clone(),
            })
            .collect();
        waits.sort_by_key(|one| (one.since, one.session));
        waits
    }

    /// **The person answered chat `session`'s saves from `shown`**: Allow for this chat, or Keep
    /// out. Refused whole unless every one of them waits now: an answer is to what the person
    /// was shown.
    pub fn saves_answered(
        &self,
        session: u32,
        shown: &[String],
        allow: bool,
    ) -> Result<(), String> {
        let mut chats = lock(&self.chats);
        let waits_no_more = || "these downloads wait on you no more".to_owned();
        let saving = chats.get_mut(&session).ok_or_else(waits_no_more)?;
        if shown.is_empty() || !shown.iter().all(|one| saving.asked.contains(one)) {
            return Err(waits_no_more());
        }
        saving.asked.retain(|one| !shown.contains(one));
        let into = if allow {
            &mut saving.allowed
        } else {
            &mut saving.kept_out
        };
        for one in shown {
            if !into.contains(one) {
                into.push(one.clone());
            }
        }
        Ok(())
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

/// **Where download `download` of browser `program` is held** (#2020), on this machine, by its
/// own name, for the person's Open and Show in folder. The engine keeps it under a name of its
/// own, which says nothing of what it is; so the first press gives it a second name, the page's
/// made plain, in a folder of its own beside it (a hard link: no second copy), which is what
/// opens and what is shown. Only a download that is done, of a browser on this machine.
fn named_file(files: &Files, program: &str, download: &str) -> Result<PathBuf, String> {
    let one = files
        .of(program)
        .into_iter()
        .find(|one| one.download == download)
        .ok_or_else(|| "This download went with its browser.".to_owned())?;
    if one.state != DownloadState::Done {
        return Err("This download is not done yet: try again once it is.".to_owned());
    }
    let plain = |word: &str, more: &[u8]| {
        !word.is_empty()
            && word
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || more.contains(&b))
    };
    if !plain(&one.file, b"-") || !plain(program, b"-_") || !plain(download, b"") {
        return Err("purlis cannot tell where this download is.".to_owned());
    }
    let data = purlis_core::datahome::root()
        .ok_or_else(|| "purlis cannot tell where its data is kept.".to_owned())?;
    let held = purlis_core::browsers::browsers_in(&data)
        .join(program)
        .join(purlis_core::browsers::DOWNLOADS);
    let folder = held.join(NAMED).join(download);
    let name = file_name_of(&one.name);
    let named = folder.join(&name);
    if std::fs::symlink_metadata(&named).is_err() {
        std::fs::create_dir_all(&folder)
            .and_then(|()| std::fs::hard_link(held.join(&one.file), &named))
            .map_err(|why| format!("purlis could not name this download for you ({why})."))?;
    }
    Ok(named)
}

/// The folder in a browser's holding folder each download's own name is given in (#2020).
const NAMED: &str = "named";

/// `name`, a download's name as its browser made it plain, as one file name: anything that is
/// not one is `download`.
fn file_name_of(name: &str) -> String {
    let one = !name.is_empty()
        && !name.starts_with('.')
        && !name.contains(['/', '\\', '\0'])
        && name.chars().count() <= 255;
    if one {
        name.to_owned()
    } else {
        "download".to_owned()
    }
}

/// **Whether a file named `name` opens as a program** (#2020): an app, a script, an installer
/// or a link, which this machine would run rather than show. A page names its downloads, so Open
/// is never offered for one of these: Show in folder is.
pub fn opens_as_a_program(name: &str) -> bool {
    const PROGRAMS: &[&str] = &[
        "app",
        "action",
        "appimage",
        "applescript",
        "bash",
        "bat",
        "bin",
        "cmd",
        "com",
        "command",
        "csh",
        "deb",
        "desktop",
        "dmg",
        "exe",
        "fileloc",
        "fish",
        "inetloc",
        "jar",
        "js",
        "kext",
        "ksh",
        "lnk",
        "mobileconfig",
        "mpkg",
        "msi",
        "pkg",
        "pl",
        "plugin",
        "prefpane",
        "ps1",
        "py",
        "rb",
        "rpm",
        "run",
        "scpt",
        "scr",
        "sh",
        "terminal",
        "tool",
        "url",
        "vbs",
        "webloc",
        "workflow",
        "zsh",
    ];
    name.rsplit_once('.')
        .is_some_and(|(_, ext)| PROGRAMS.contains(&ext.trim().to_ascii_lowercase().as_str()))
}

/// **Opens download `download` of browser `program` with this machine's default for it**
/// (#2020): the person's press on its row. Never one that opens as a program
/// ([`opens_as_a_program`]): Show in folder is the way to it.
#[tauri::command]
#[specta::specta]
pub async fn open_browser_download(
    app: tauri::AppHandle,
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    program: String,
    download: String,
) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt as _;
    let held = planes.held(&plane)?;
    let file = named_file(held.browser_files(), &program, &download)?;
    if file
        .file_name()
        .is_some_and(|name| opens_as_a_program(&name.to_string_lossy()))
    {
        return Err(
            "purlis does not open a program a page sent: use Show in folder to look at it."
                .to_owned(),
        );
    }
    app.opener()
        .open_path(file.display().to_string(), None::<&str>)
        .map_err(|why| format!("The download did not open ({why}). Try Show in folder."))
}

/// **Shows download `download` of browser `program` in its folder** (#2020), in this machine's
/// file manager, by its own name.
#[tauri::command]
#[specta::specta]
pub async fn show_browser_download(
    app: tauri::AppHandle,
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    program: String,
    download: String,
) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt as _;
    let held = planes.held(&plane)?;
    let file = named_file(held.browser_files(), &program, &download)?;
    app.opener()
        .reveal_item_in_dir(&file)
        .map_err(|why| format!("The download's folder did not open ({why}). Try again."))
}

/// **The person answered chat `session`'s saves of downloads from `origins`** (#2020): Allow
/// for this chat saves them from its next call; Keep out refuses them. The window's alone.
#[tauri::command]
#[specta::specta]
pub async fn answer_browser_saves(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    session: u32,
    origins: Vec<String>,
    allow: bool,
) -> Result<(), String> {
    let held = planes.held(&plane)?;
    held.browser_files()
        .saves_answered(session, &origins, allow)?;
    for origin in &origins {
        held.chats().tell_outcome(
            session,
            purlis_core::askoutcome::Outcome {
                about: purlis_core::askoutcome::About::BrowserSave {
                    origin: origin.clone(),
                },
                answer: if allow {
                    purlis_core::askoutcome::Answer::Allowed {
                        scope: purlis_core::askoutcome::Scope::Chat,
                    }
                } else {
                    purlis_core::askoutcome::Answer::KeptBlocked { reason: None }
                },
            },
        );
    }
    held.chats().asks_may_have_moved();
    Ok(())
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests;
