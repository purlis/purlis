// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat's browser timeline** (#2017, spec #2008 story 56): what the chat's agent did in its
//! browsers, with the screenshots its tools returned, kept on this device and bounded, as proof
//! of the work. `app/browser-timelines/<chat>/` in the project's state folder.
//!
//! - **What it holds is what the agent was given, never more.** A moment is recorded from the
//!   answer the chat's browser call was given ([`acted`]): its act in words, the page it left
//!   cut to its scheme, host, port and path, and, for a screenshot, the very picture the agent
//!   got, already blanked by the browser. The timeline takes no screenshot of its own, so it
//!   holds nothing a blanked field showed, and nothing the agent never saw.
//! - **What the agent typed is not kept**: a typing is "Typed into 2 fields", never its text.
//!   Reading the page (a snapshot, a find, the changes, the pages, a wait for text) is no
//!   moment, and neither is a retry, which acted on nothing.
//! - **Nothing of a moment the person or purlis drove.** Their taking the browser over, handing
//!   it back, and purlis signing in are moments of their own with no screenshot
//!   ([`took_over`], [`handed_back`], [`signed_in`]). A screenshot whose answer comes back while
//!   the person or purlis drives the browser is not kept ([`Unkept::Driven`]): the browser
//!   refuses the agent's calls then, so it can only be one taken just before, and it is let go.
//! - **Bounded**: the newest [`MOST_MOMENTS`] moments, the newest [`MOST_SHOTS`] screenshots of
//!   them, none larger than [`MOST_SHOT_BYTES`], and the timelines of the newest [`MOST_CHATS`]
//!   chats of the project. What is let go is removed from disk at once.
//! - **This device's, never committed, and no chat's to read**: under `app/`, which no
//!   sandboxed chat writes, in a folder none reads (`crate::sandbox`), since a screenshot of one
//!   persona's chat is not for a chat running as another. The app removes a chat's timeline
//!   when the chat closes.

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
pub const MOST_SHOT_BYTES: usize = 8 * 1024 * 1024;

/// The most chats of a project that keep a timeline: those written to last.
pub const MOST_CHATS: usize = 16;

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
    /// Let go to keep the timeline within [`MOST_SHOTS`].
    Bound,
    /// Larger than [`MOST_SHOT_BYTES`].
    TooLarge,
    /// Its answer came back while the person or purlis drove the browser.
    Driven,
}

/// One moment of a chat's timeline, as kept.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Moment {
    /// When, in milliseconds since 1970.
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

/// **The moment the agent's act `act` on browser `browser` makes**, answered `browsed`, at
/// `at`: none for a retry, or for a call that only reads the page. Where `driven_by` says the
/// person or purlis drives the browser as the answer comes back, its screenshot is not kept.
pub fn acted(
    browser: &str,
    act: &Act,
    browsed: &Browsed,
    at: u64,
    driven_by: Option<By>,
) -> Option<Happened> {
    if browsed.repeated {
        return None;
    }
    let did = did(act)?;
    let (png, unkept) = match (&browsed.png, driven_by) {
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
            page: page_of(browsed),
            shot: None,
            unkept,
        },
        png,
    })
}

/// The person took browser `browser` over, at `at`.
pub fn took_over(browser: &str, at: u64) -> Happened {
    plain(browser, By::Person, "You took the browser over", None, at)
}

/// The person handed browser `browser` back to the agent, at `at`.
pub fn handed_back(browser: &str, at: u64) -> Happened {
    plain(browser, By::Person, "You handed the browser back", None, at)
}

/// purlis performed the chat's browser login `login` in browser `browser`, leaving `browsed`.
pub fn signed_in(browser: &str, login: &str, browsed: &Browsed, at: u64) -> Happened {
    plain(
        browser,
        By::Purlis,
        &format!("purlis signed in with {login}"),
        page_of(browsed),
        at,
    )
}

fn plain(browser: &str, by: By, did: &str, page: Option<String>, at: u64) -> Happened {
    Happened {
        moment: Moment {
            at,
            browser: browser.to_owned(),
            by,
            did: did.to_owned(),
            page,
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

fn address(written: &str) -> String {
    crate::browsersatquit::cut(written).unwrap_or_else(|| "an address".to_owned())
}

/// An act in words that hold nothing typed; `None` for one that only reads the page.
fn did(act: &Act) -> Option<String> {
    let plural = |n: usize, one: &str, many: &str| {
        if n == 1 {
            format!("1 {one}")
        } else {
            format!("{n} {many}")
        }
    };
    Some(match act {
        Act::Snapshot { .. }
        | Act::Find { .. }
        | Act::Changes { .. }
        | Act::Pages
        | Act::WaitForText { .. } => return None,
        Act::Open { address: to } => format!("Opened {}", address(to)),
        Act::Navigate { address: to, .. } => format!("Went to {}", address(to)),
        Act::Click { target } => format!("Clicked {target}"),
        Act::DoubleClick { target } => format!("Double-clicked {target}"),
        Act::Hover { target } => format!("Hovered over {target}"),
        Act::Type { fields, .. } => {
            format!("Typed into {}", plural(fields.len(), "field", "fields"))
        }
        Act::Press { keys, .. } => format!("Pressed {}", plural(keys.len(), "key", "keys")),
        Act::Select { target, .. } => format!("Chose in {target}"),
        Act::Drag { from, .. } => format!("Dragged {from}"),
        Act::Scroll { .. } => "Scrolled".to_owned(),
        Act::Back { .. } => "Went back".to_owned(),
        Act::SwitchPage { page } => format!("Switched to page {page}"),
        Act::Screenshot { .. } => "Took a screenshot".to_owned(),
    })
}

/// A folder or file that is there as itself: never through a link.
fn is_dir(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|there| there.is_dir())
}

fn is_file(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|there| there.is_file())
}

fn kept(folder: &Path) -> Kept {
    let file = folder.join(FILE);
    if !is_file(&file) {
        return Kept::default();
    }
    std::fs::read_to_string(&file)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
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

/// **Records `happened` in chat `chat`'s timeline**, letting go of what falls outside its
/// bound: its oldest moments, the screenshots of its oldest, and the timelines of the project's
/// chats written to longest ago.
pub fn record(root: &Path, chat: &str, happened: Happened) -> io::Result<()> {
    let Some(chat) = named(chat) else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a chat's timeline is named by the chat's id",
        ));
    };
    let _writing = WRITING.lock().unwrap_or_else(|held| held.into_inner());
    let folder = folder(root, chat);
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
    let mut kept = kept(&folder);
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
    kept.moments.push(moment);
    while kept.moments.len() > MOST_MOMENTS {
        let old = kept.moments.remove(0);
        if let Some(n) = old.shot {
            gone(&png(&folder, n))?;
        }
    }
    let mut shots = kept.moments.iter().filter(|one| one.shot.is_some()).count();
    for old in &mut kept.moments {
        if shots <= MOST_SHOTS {
            break;
        }
        if let Some(n) = old.shot.take() {
            gone(&png(&folder, n))?;
            old.unkept = Some(Unkept::Bound);
            shots -= 1;
        }
    }
    let text = serde_json::to_string_pretty(&kept).map_err(io::Error::other)?;
    crate::rewrite::replace(
        root,
        &folder.join(FILE),
        format!("{text}\n").as_bytes(),
        crate::rewrite::Mode::Private,
    )
}

/// Leaves room for one more chat's timeline: the oldest written go, past [`MOST_CHATS`] - 1.
fn make_room(root: &Path) -> io::Result<()> {
    let dir = dir(root);
    if !is_dir(&dir) {
        return Ok(());
    }
    let mut chats: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(&dir)?
        .filter_map(Result::ok)
        .filter(|one| one.file_name().to_str().and_then(named).is_some())
        .filter(|one| is_dir(&one.path()))
        .map(|one| {
            let written = std::fs::symlink_metadata(one.path().join(FILE))
                .or_else(|_| std::fs::symlink_metadata(one.path()))
                .and_then(|there| there.modified())
                .unwrap_or(std::time::UNIX_EPOCH);
            (written, one.path())
        })
        .collect();
    chats.sort();
    while chats.len() >= MOST_CHATS {
        let (_, oldest) = chats.remove(0);
        std::fs::remove_dir_all(oldest)?;
    }
    Ok(())
}

/// **Chat `chat`'s timeline**, oldest first: none for a chat that has none, or one that does
/// not read.
pub fn read(root: &Path, chat: &str) -> Vec<Moment> {
    let Some(chat) = named(chat) else {
        return Vec::new();
    };
    let folder = folder(root, chat);
    if !is_dir(&folder) {
        return Vec::new();
    }
    kept(&folder).moments
}

/// **Screenshot `shot` of chat `chat`'s timeline**, PNG: only one a moment of it lists.
pub fn shot_of(root: &Path, chat: &str, shot: u64) -> Option<Vec<u8>> {
    if !read(root, chat).iter().any(|one| one.shot == Some(shot)) {
        return None;
    }
    let path = png(&folder(root, chat), shot);
    if !is_file(&path) {
        return None;
    }
    std::fs::read(path).ok()
}

/// **Removes chat `chat`'s timeline**, with its screenshots. None there is no error.
pub fn remove(root: &Path, chat: &str) -> io::Result<()> {
    let Some(chat) = named(chat) else {
        return Ok(());
    };
    let _writing = WRITING.lock().unwrap_or_else(|held| held.into_inner());
    let folder = folder(root, chat);
    match std::fs::symlink_metadata(&folder) {
        Err(none) if none.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(why) => Err(why),
        Ok(there) if there.is_dir() => std::fs::remove_dir_all(&folder),
        // A link in its place goes, never what it points to.
        Ok(_) => std::fs::remove_file(&folder),
    }
}

#[cfg(test)]
mod tests;
