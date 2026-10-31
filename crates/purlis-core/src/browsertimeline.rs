// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat's browser timeline** (#2017, spec #2008 story 56): what the chat's agent did in its
//! browsers, with the screenshots its tools returned, kept on this device and bounded, as proof
//! of the work. `app/browser-timelines/<chat>/` in the project's state folder. The app keeps it,
//! not the session host (ADR 0068, amended for #2017).
//!
//! - **What it holds is what the agent was given, never more.** A moment is recorded from the
//!   answer the chat's browser call was given ([`acted`]): its act in words, the page it left
//!   cut to its scheme, host, port and path, and, for a screenshot, the very picture the agent
//!   got, already blanked by the browser. The timeline takes no screenshot of its own, so it
//!   holds nothing a blanked field showed, and nothing the agent never saw.
//! - **What the agent typed is not kept**: a typing is "Typed into 2 fields", never its text,
//!   and a reference the agent wrote is cut to [`MOST_TARGET_CHARS`]. Reading the page (a
//!   snapshot, a find, the changes, the pages, a wait for text) is no moment, and neither is a
//!   retry, which acted on nothing.
//! - **No address of a browser purlis signed in** ([`Seen::signed_in`]): a signed-in site's
//!   paths can carry what lets someone in, so its moments keep no page, as the browsers kept at
//!   a quit keep none of one (#2022).
//! - **Nothing of a moment the person or purlis drove.** Their taking the browser over, handing
//!   it back, and purlis signing in are moments of their own with no screenshot
//!   ([`took_over`], [`handed_back`], [`signed_in`]). A screenshot whose answer comes back while
//!   the person or purlis drives the browser is not kept ([`Unkept::Driven`]): the browser
//!   refuses the agent's calls then, so it can only be one taken just before, and it is let go.
//! - **Bounded**: the newest [`MOST_MOMENTS`] moments, the newest [`MOST_SHOTS`] screenshots of
//!   them, none larger than [`MOST_SHOT_BYTES`], [`MOST_PROJECT_SHOT_BYTES`] of screenshots in
//!   the whole project (the oldest go first, whichever chat's), and the timelines of the newest
//!   [`MOST_CHATS`] chats of the project. What is let go is removed from disk at once, after
//!   the list that named it no longer does.
//! - **This device's, never committed, and no chat's to read**: under `app/`, which no
//!   sandboxed chat writes, in a folder none reads (`crate::sandbox`), since a screenshot of one
//!   persona's chat is not for a chat running as another. Nothing here is reached through a
//!   link ([`crate::contain::no_link_on_the_way`]). The app removes a chat's timeline when the
//!   chat closes, and [`sweep`]s what a crash or a quit left.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use purlis_session_protocol::browsing::{Act, Browsed};
use serde::{Deserialize, Serialize};

/// The folder of every chat's timeline, relative to the project's state folder.
pub const DIR: &str = "app/browser-timelines";

/// The file in a chat's folder that lists its moments.
pub const FILE: &str = "timeline.json";

/// The most moments a chat's timeline keeps: the newest.
pub const MOST_MOMENTS: usize = 200;

/// The most screenshots a chat's timeline keeps: those of its newest moments that have one.
pub const MOST_SHOTS: usize = 24;

/// The largest screenshot kept, in bytes.
pub const MOST_SHOT_BYTES: usize = 4 * 1024 * 1024;

/// The most bytes of screenshots the project's timelines keep together: past it, the oldest
/// go, whichever chat's.
pub const MOST_PROJECT_SHOT_BYTES: u64 = 96 * 1024 * 1024;

/// The most chats of a project that keep a timeline: those written to last.
pub const MOST_CHATS: usize = 16;

/// The most characters of a reference the agent wrote that a moment keeps.
pub const MOST_TARGET_CHARS: usize = 200;

/// How young a chat's timeline is spared by a [`sweep`], whatever the record of open chats
/// says: a chat that has only just started may not be in it yet.
pub const SWEEP_GRACE: std::time::Duration = std::time::Duration::from_secs(120);

/// Who drove the browser at a moment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum By {
    Agent,
    Person,
    Purlis,
}

/// Why a moment that had a screenshot keeps none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Unkept {
    /// Let go to keep the timeline within its bounds.
    Bound,
    /// Larger than [`MOST_SHOT_BYTES`].
    TooLarge,
    /// Its answer came back while the person or purlis drove the browser.
    Driven,
}

/// One moment of a chat's timeline, as kept.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Moment {
    /// When, in seconds since 1970.
    pub at: u64,
    /// Which of the chat's browsers, as the chat names it (`b1`).
    pub browser: String,
    pub by: By,
    /// What was done, in words that hold nothing typed.
    pub did: String,
    /// The page it left, cut to its scheme, host, port and path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<String>,
    /// Its screenshot's number, where one is kept ([`shot_of`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shot: Option<u64>,
    /// Why a screenshot it had is not kept.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unkept: Option<Unkept>,
}

/// A moment to record: what happened, with the screenshot the agent was given, if any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Happened {
    pub moment: Moment,
    pub png: Option<Vec<u8>>,
}

/// What the app knows of a browser as an answer to its agent comes back.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Seen {
    /// Who drives it now, where the agent does not.
    pub driven_by: Option<By>,
    /// Whether it holds a login purlis performed.
    pub signed_in: bool,
}

/// What a chat's folder holds in [`FILE`].
#[derive(Debug, Default, Serialize, Deserialize)]
struct Kept {
    #[serde(default)]
    moments: Vec<Moment>,
    /// The next screenshot's number.
    #[serde(default)]
    next: u64,
}

/// One write at a time, so two calls of a chat's never lose each other's moment.
static WRITING: Mutex<()> = Mutex::new(());

/// The folder of every chat's timeline in the project at `root`.
pub fn dir(root: &Path) -> PathBuf {
    crate::names::state(root).join(DIR)
}

/// The folder of chat `chat`'s timeline.
pub fn folder(root: &Path, chat: &str) -> PathBuf {
    dir(root).join(chat)
}

/// `chat` where it is a ULID in its canonical spelling: the only names a folder here has.
fn named(chat: &str) -> Option<&str> {
    (crate::reopen::a_ulid(chat).as_deref() == Some(chat)).then_some(chat)
}

/// Chat `chat`'s folder, where its name is a chat's and nothing on the way to it is a link.
fn reached(root: &Path, chat: &str) -> io::Result<PathBuf> {
    let Some(chat) = named(chat) else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a chat's timeline is named by the chat's id",
        ));
    };
    let folder = folder(root, chat);
    crate::contain::no_link_on_the_way(root, &folder)?;
    Ok(folder)
}

/// **The moment the agent's act `act` on browser `browser` makes**, answered `browsed`, at
/// `at` (seconds since 1970), as the app `seen` the browser then: none for a retry, or for a
/// call that only reads the page.
pub fn acted(browser: &str, act: &Act, browsed: &Browsed, at: u64, seen: Seen) -> Option<Happened> {
    if browsed.repeated {
        return None;
    }
    let did = did(act, seen.signed_in)?;
    let (png, unkept) = match (&browsed.png, seen.driven_by) {
        (Some(_), Some(By::Person | By::Purlis)) => (None, Some(Unkept::Driven)),
        (Some(png), _) => (Some(png.clone()), None),
        (None, _) => (None, None),
    };
    Some(Happened {
        moment: Moment {
            at,
            browser: browser.to_owned(),
            by: By::Agent,
            did,
            page: if seen.signed_in {
                None
            } else {
                page_of(browsed)
            },
            shot: None,
            unkept,
        },
        png,
    })
}

/// The person took browser `browser` over, at `at`.
pub fn took_over(browser: &str, at: u64) -> Happened {
    plain(browser, By::Person, "You took the browser over", at)
}

/// The person handed browser `browser` back to the agent, at `at`.
pub fn handed_back(browser: &str, at: u64) -> Happened {
    plain(browser, By::Person, "You handed the browser back", at)
}

/// purlis performed the chat's browser login `login` in browser `browser`, at `at`: with no
/// page, as no moment of a signed-in browser has.
pub fn signed_in(browser: &str, login: &str, at: u64) -> Happened {
    plain(
        browser,
        By::Purlis,
        &format!("purlis signed in with {}", cut_to(login, MOST_TARGET_CHARS)),
        at,
    )
}

fn plain(browser: &str, by: By, did: &str, at: u64) -> Happened {
    Happened {
        moment: Moment {
            at,
            browser: browser.to_owned(),
            by,
            did: did.to_owned(),
            page: None,
            shot: None,
            unkept: None,
        },
        png: None,
    }
}

fn page_of(browsed: &Browsed) -> Option<String> {
    browsed
        .page
        .as_ref()
        .and_then(|page| crate::browsersatquit::cut(&page.address))
}

/// `text` to its first `most` characters, with an ellipsis where it was longer.
fn cut_to(text: &str, most: usize) -> String {
    if text.chars().count() <= most {
        return text.to_owned();
    }
    let mut cut: String = text.chars().take(most).collect();
    cut.push('…');
    cut
}

/// An act in words that hold nothing typed; `None` for one that only reads the page. A
/// browser purlis signed in has no address said.
fn did(act: &Act, signed_in: bool) -> Option<String> {
    let plural = |n: usize, one: &str, many: &str| {
        if n == 1 {
            format!("1 {one}")
        } else {
            format!("{n} {many}")
        }
    };
    let address = |written: &str| match crate::browsersatquit::cut(written) {
        // Already no longer than the longest address kept at a quit.
        Some(cut) if !signed_in => cut,
        _ => "a page".to_owned(),
    };
    let target = |written: &str| cut_to(written, MOST_TARGET_CHARS);
    Some(match act {
        Act::Snapshot { .. }
        | Act::Find { .. }
        | Act::Changes { .. }
        | Act::Pages
        | Act::Downloads
        | Act::WaitForText { .. } => return None,
        Act::Open { address: to } => format!("Opened {}", address(to)),
        Act::Navigate { address: to, .. } => format!("Went to {}", address(to)),
        Act::Click { target: on } => format!("Clicked {}", target(on)),
        Act::DoubleClick { target: on } => format!("Double-clicked {}", target(on)),
        Act::Hover { target: on } => format!("Hovered over {}", target(on)),
        Act::Type { fields, .. } => {
            format!("Typed into {}", plural(fields.len(), "field", "fields"))
        }
        Act::Press { keys, .. } => format!("Pressed {}", plural(keys.len(), "key", "keys")),
        Act::Select { target: on, .. } => format!("Chose in {}", target(on)),
        Act::Drag { from, .. } => format!("Dragged {}", target(from)),
        Act::Scroll { .. } => "Scrolled".to_owned(),
        Act::Back { .. } => "Went back".to_owned(),
        Act::SwitchPage { page } => format!("Switched to page {}", target(page)),
        Act::Screenshot { .. } => "Took a screenshot".to_owned(),
        Act::Upload { files, .. } => {
            format!("Uploaded {}", plural(files.len(), "file", "files"))
        }
        Act::SaveDownload { download, .. } => format!("Saved download {}", target(download)),
    })
}

/// A folder or file that is there as itself: never through a link.
fn is_dir(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|there| there.is_dir())
}

fn is_file(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|there| there.is_file())
}

/// What `folder` lists. A list that is there and does not read is warned of and the whole
/// folder goes, so no screenshot is left that no list names; the timeline starts again.
fn load(folder: &Path) -> Kept {
    let file = folder.join(FILE);
    if !is_file(&file) {
        return Kept::default();
    }
    let read = std::fs::read_to_string(&file)
        .map_err(|why| why.to_string())
        .and_then(|text| serde_json::from_str::<Kept>(&text).map_err(|why| why.to_string()));
    match read {
        Ok(kept) => kept,
        Err(why) => {
            tracing::warn!(
                "purlis: a chat's browser timeline did not read ({why}), so it was removed with \
                 its screenshots"
            );
            if let Err(why) = std::fs::remove_dir_all(folder) {
                tracing::warn!("purlis: the browser timeline that did not read stays: {why}");
            }
            Kept::default()
        }
    }
}

fn png(folder: &Path, shot: u64) -> PathBuf {
    folder.join(format!("{shot}.png"))
}

fn gone(path: &Path) -> io::Result<()> {
    match std::fs::remove_file(path) {
        Err(none) if none.kind() == io::ErrorKind::NotFound => Ok(()),
        done => done,
    }
}

fn write(root: &Path, folder: &Path, kept: &Kept) -> io::Result<()> {
    let text = serde_json::to_string_pretty(kept).map_err(io::Error::other)?;
    crate::rewrite::replace(
        root,
        &folder.join(FILE),
        format!("{text}\n").as_bytes(),
        crate::rewrite::Mode::Private,
    )
}

/// **Records `happened` in chat `chat`'s timeline**, letting go of what falls outside its
/// bounds: its oldest moments, the screenshots of its oldest, the oldest screenshots of the
/// project past its byte budget, and the timelines of the project's chats written to longest
/// ago. The list is written before a screenshot it no longer names is removed.
pub fn record(root: &Path, chat: &str, happened: Happened) -> io::Result<()> {
    let folder = reached(root, chat)?;
    let _writing = WRITING.lock().unwrap_or_else(|held| held.into_inner());
    let mut kept = load(&folder);
    if !is_dir(&folder) {
        if std::fs::symlink_metadata(&folder).is_ok() {
            return Err(io::Error::other(
                "a chat's timeline folder is not a folder of its own",
            ));
        }
        make_room(root)?;
        std::fs::create_dir_all(&folder)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o700))?;
        }
    }
    let Happened {
        mut moment,
        png: picture,
    } = happened;
    if let Some(picture) = picture {
        if picture.len() > MOST_SHOT_BYTES {
            moment.unkept = Some(Unkept::TooLarge);
        } else {
            let n = kept.next;
            kept.next += 1;
            crate::rewrite::replace(
                root,
                &png(&folder, n),
                &picture,
                crate::rewrite::Mode::Private,
            )?;
            moment.shot = Some(n);
        }
    }
    let had_a_shot = moment.shot.is_some();
    kept.moments.push(moment);
    let mut let_go: Vec<u64> = Vec::new();
    while kept.moments.len() > MOST_MOMENTS {
        let old = kept.moments.remove(0);
        let_go.extend(old.shot);
    }
    let mut shots = kept.moments.iter().filter(|one| one.shot.is_some()).count();
    for old in &mut kept.moments {
        if shots <= MOST_SHOTS {
            break;
        }
        if let Some(n) = old.shot.take() {
            let_go.push(n);
            old.unkept = Some(Unkept::Bound);
            shots -= 1;
        }
    }
    write(root, &folder, &kept)?;
    for n in let_go {
        gone(&png(&folder, n))?;
    }
    if had_a_shot {
        fit_the_budget(root)?;
    }
    Ok(())
}

/// Every chat's folder of the project, by its id: those named as a chat's, as folders of their
/// own.
fn chats(root: &Path) -> io::Result<Vec<(String, PathBuf)>> {
    let dir = dir(root);
    crate::contain::no_link_on_the_way(root, &dir)?;
    if !is_dir(&dir) {
        return Ok(Vec::new());
    }
    Ok(std::fs::read_dir(&dir)?
        .filter_map(Result::ok)
        .filter_map(|one| {
            let name = one.file_name().to_str().and_then(named)?.to_owned();
            is_dir(&one.path()).then(|| (name, one.path()))
        })
        .collect())
}

/// Lets go of the project's oldest screenshots, whichever chat's, until they fit
/// [`MOST_PROJECT_SHOT_BYTES`].
fn fit_the_budget(root: &Path) -> io::Result<()> {
    let mut all: Vec<(u64, u64, usize, u64)> = Vec::new();
    let mut lists: Vec<(PathBuf, Kept)> = Vec::new();
    for (_, folder) in chats(root)? {
        let kept = load(&folder);
        let at = lists.len();
        for moment in &kept.moments {
            if let Some(n) = moment.shot {
                let size = std::fs::symlink_metadata(png(&folder, n)).map_or(0, |one| one.len());
                all.push((moment.at, size, at, n));
            }
        }
        lists.push((folder, kept));
    }
    let mut total: u64 = all.iter().map(|one| one.1).sum();
    if total <= MOST_PROJECT_SHOT_BYTES {
        return Ok(());
    }
    all.sort_by_key(|&(at, _, list, n)| (at, list, n));
    let mut let_go: Vec<(usize, u64)> = Vec::new();
    for (_, size, list, n) in all {
        if total <= MOST_PROJECT_SHOT_BYTES {
            break;
        }
        total = total.saturating_sub(size);
        let_go.push((list, n));
    }
    for (at, (folder, kept)) in lists.iter_mut().enumerate() {
        let mine: Vec<u64> = let_go
            .iter()
            .filter(|(list, _)| *list == at)
            .map(|(_, n)| *n)
            .collect();
        if mine.is_empty() {
            continue;
        }
        for moment in &mut kept.moments {
            if moment.shot.is_some_and(|n| mine.contains(&n)) {
                moment.shot = None;
                moment.unkept = Some(Unkept::Bound);
            }
        }
        write(root, folder, kept)?;
        for n in mine {
            gone(&png(folder, n))?;
        }
    }
    Ok(())
}

/// Leaves room for one more chat's timeline: the oldest written go, past [`MOST_CHATS`] - 1.
fn make_room(root: &Path) -> io::Result<()> {
    let mut chats: Vec<(std::time::SystemTime, PathBuf)> = chats(root)?
        .into_iter()
        .map(|(_, folder)| (written(&folder), folder))
        .collect();
    chats.sort();
    while chats.len() >= MOST_CHATS {
        let (_, oldest) = chats.remove(0);
        std::fs::remove_dir_all(oldest)?;
    }
    Ok(())
}

/// When a chat's timeline was last written.
fn written(folder: &Path) -> std::time::SystemTime {
    std::fs::symlink_metadata(folder.join(FILE))
        .or_else(|_| std::fs::symlink_metadata(folder))
        .and_then(|there| there.modified())
        .unwrap_or(std::time::UNIX_EPOCH)
}

/// **Chat `chat`'s timeline**, oldest first: none for a chat that has none, one reached
/// through a link, or one that does not read (which goes).
pub fn read(root: &Path, chat: &str) -> Vec<Moment> {
    let Ok(folder) = reached(root, chat) else {
        return Vec::new();
    };
    if !is_dir(&folder) {
        return Vec::new();
    }
    let _writing = WRITING.lock().unwrap_or_else(|held| held.into_inner());
    load(&folder).moments
}

/// **Screenshot `shot` of chat `chat`'s timeline**, PNG: only one a moment of it lists.
pub fn shot_of(root: &Path, chat: &str, shot: u64) -> Option<Vec<u8>> {
    if !read(root, chat).iter().any(|one| one.shot == Some(shot)) {
        return None;
    }
    let path = png(&reached(root, chat).ok()?, shot);
    if !is_file(&path) {
        return None;
    }
    std::fs::read(path).ok()
}

/// **Removes chat `chat`'s timeline**, with its screenshots. None there is no error; one reached
/// through a link is refused, and nothing it points to is removed.
pub fn remove(root: &Path, chat: &str) -> io::Result<()> {
    let folder = reached(root, chat)?;
    let _writing = WRITING.lock().unwrap_or_else(|held| held.into_inner());
    match std::fs::symlink_metadata(&folder) {
        Err(none) if none.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(why) => Err(why),
        Ok(there) if there.is_dir() => std::fs::remove_dir_all(&folder),
        Ok(_) => std::fs::remove_file(&folder),
    }
}

/// **What a crash or a quit left goes** (#2017), each time the app writes its record of the
/// project's open chats, `open` their ids: the timeline of every chat not in it (spared
/// [`SWEEP_GRACE`] after its last write), and in every timeline kept, each screenshot its list
/// does not name.
pub fn sweep(root: &Path, open: &[&str]) {
    sweep_sparing(root, open, SWEEP_GRACE);
}

fn sweep_sparing(root: &Path, open: &[&str], grace: std::time::Duration) {
    let _writing = WRITING.lock().unwrap_or_else(|held| held.into_inner());
    let Ok(chats) = chats(root) else {
        return;
    };
    for (chat, folder) in chats {
        let young = written(&folder).elapsed().is_ok_and(|age| age < grace);
        if !open.contains(&chat.as_str()) && !young {
            if let Err(why) = std::fs::remove_dir_all(&folder) {
                tracing::warn!("purlis: a closed chat's browser timeline stays on disk: {why}");
            }
            continue;
        }
        let kept = load(&folder);
        let Ok(entries) = std::fs::read_dir(&folder) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            let name = entry.file_name();
            let Some(n) = name
                .to_str()
                .and_then(|name| name.strip_suffix(".png"))
                .and_then(|n| n.parse::<u64>().ok())
            else {
                continue;
            };
            if !kept.moments.iter().any(|one| one.shot == Some(n)) {
                let _ = gone(&entry.path());
            }
        }
    }
}

#[cfg(test)]
mod tests;
