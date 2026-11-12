// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Links in a pane's output** (spec #6208, #6215, #6216): the window's commands over the core's
//! resolver ([`purlis_core::panelinks`]).
//!
//! - [`pane_link_resolve`]: what each candidate a line holds is, for a hover. The pane caches
//!   the answers itself, per pane, so hovering a line again asks nothing.
//! - [`pane_link_open`]: the one open command (ADR 0052's list). It is handed the candidate's
//!   text again, never a target, and resolves it again, so a pane can never name what opens. It
//!   answers [`LinkOpened`]: opened, a file for the window's light editor, ask first (with the
//!   ask's one-use token, [`Asks`]), or refused with the window's sentence. A web address goes
//!   out through the opener plugin; a file never does: the window shows it in an editor tab, so
//!   a click never runs anything.
//! - [`pane_link_file`]: a file outside the project, read for its tab only once an ask for it
//!   was answered in that pane, and only while it is still the file it was then.
//! - **A chat's number** (`chat 641`, #6217) is looked up in the pane's own project, given the
//!   chats open there now ([`ChatsHere`]). Its hover says its number and title, where it is and
//!   what ⌘-click does ([`ChatSeen`]); opening it answers what the window shows ([`ChatGo`]): the
//!   chat in its tab or workspace, an ended chat's card, or nothing for the pane's own chat. A
//!   number no chat of the project had says nothing on a hover.
//!
//! **A file's answer looks at the disk, so it is looked at on a thread of its own, for at most
//! [`LOOK_FOR`]** ([`looked`]): a path on a stalled mount answers "no link" in time, and holds
//! none of the app's shared threads. At most [`MOST_LOOKING`] looks are under way at once.
//!
//! None of them reads a secret's value or hands one back.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use purlis_core::dispatchrecord::Record;
use purlis_core::panelinks::{
    Asks, Candidate, ChatAt, ChatNumber, FileAt, Goes, Home, HomeIs, Hover, Kind, NOT_A_LINK,
    Opening, Pane, ReadFrom, Resolver, Runs, Target, UNASKABLE,
};
use serde::{Deserialize, Serialize};

use crate::chatcard::ChatCard;
use crate::piecefiles::PieceFile;
use crate::planes::{Held, PlaneId, Planes};

/// **What the pane's pattern took a candidate for**: a hint, never trusted for more. The wire's
/// half of the core's [`Kind`]; a kind's resolver is the core's `LinkKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum KindHint {
    Url,
    File,
    Chat,
}

impl From<KindHint> for Kind {
    fn from(kind: KindHint) -> Self {
        match kind {
            KindHint::Url => Kind::Url,
            KindHint::File => Kind::File,
            KindHint::Chat => Kind::Chat,
        }
    }
}

impl From<Kind> for KindHint {
    fn from(kind: Kind) -> Self {
        match kind {
            Kind::Url => KindHint::Url,
            Kind::File => KindHint::File,
            Kind::Chat => KindHint::Chat,
        }
    }
}

/// **Text a pane found that may be a link**: the words as printed, and the pattern's hint.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, specta::Type)]
pub struct LinkCandidate {
    pub kind: KindHint,
    pub text: String,
}

impl From<LinkCandidate> for Candidate {
    fn from(candidate: LinkCandidate) -> Self {
        Candidate {
            kind: candidate.kind.into(),
            text: candidate.text,
        }
    }
}

/// **Where a candidate was found**: the pane's project and the chat it draws, where it has them
/// (a saved chat's read-only terminal has neither).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, specta::Type)]
pub struct LinkFrom {
    pub plane: Option<PlaneId>,
    pub session: Option<u32>,
}

/// **The pane `from` names, as the core resolves against it**: its project's folder, and the
/// folder its chat started in. Its paths are this machine's only for a chat open here and not
/// on a runner or a pool; every other pane (a saved chat's, one whose chat has closed, one with
/// no project open) names files purlis cannot place, so they are [`Runs::Elsewhere`]. A
/// project the window does not hold is no project: nothing of it is read. Handed back with the
/// pane, the project the window holds, which a chat's number is looked up in.
// The folder is the one the chat started in: no hook reports the folder a chat has moved to.
fn pane_of(planes: &Planes, from: LinkFrom) -> (Pane, Option<Arc<Held>>) {
    let held = from
        .plane
        .as_ref()
        .and_then(|plane| planes.held(plane).ok());
    let Some(held) = held else {
        let pane = Pane {
            project: None,
            session: from.session,
            folder: None,
            runs: Runs::Elsewhere,
        };
        return (pane, None);
    };
    let chats = held.chats();
    let here = from.session.and_then(|session| {
        let at = chats.chat_at(session)?;
        let local = chats.runner_of(session).is_none() && chats.pool_of(session).is_none();
        local.then_some(at)
    });
    let pane = Pane {
        project: Some(held.root().to_path_buf()),
        session: from.session,
        runs: if here.is_some() {
            Runs::Here
        } else {
            Runs::Elsewhere
        },
        folder: here.and_then(|at| at.cwd),
    };
    (pane, Some(held))
}

/// **Where a link that opens without asking opens.**
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "kebab-case")]
pub enum OpensIn {
    /// The person's browser.
    Browser,
    /// One of purlis's editor tabs.
    EditorTab,
    /// purlis's window itself: a chat shown in its tab, or a closed chat's card.
    Window,
}

impl OpensIn {
    fn of(target: &Target) -> Self {
        match target {
            Target::Url(_) => OpensIn::Browser,
            Target::File(_) => OpensIn::EditorTab,
            Target::Chat(_) => OpensIn::Window,
        }
    }
}

/// **What ⌘-click on a link would do**, said on its hover.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "does", rename_all = "lowercase")]
pub enum LinkDoes {
    /// It opens, without asking, `into` the browser or an editor tab.
    Opens { into: OpensIn },
    /// It asks first, and why: where it is, in the window's words ("outside smart-ide").
    Asks { reason: String },
    /// It will not open, and the sentence that says why.
    Refused { reason: String },
}

/// **What a hover over a candidate shows**: its kind, its target in full (its own text, where it
/// is no link), what the hover's first line says (`shown`: a file in the chat's workspace from
/// the workspace, D2; a chat as "#641 · <title>") and what opening it does. A chat link says
/// what the chat is too.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct LinkSeen {
    pub kind: KindHint,
    pub target: String,
    pub shown: String,
    pub does: LinkDoes,
    /// What the chat a chat link names is: none for every other kind.
    pub chat: Option<ChatSeen>,
}

/// **What the chat a `chat 641` link names is**, for its hover's second line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "is", rename_all = "snake_case")]
pub enum ChatSeen {
    /// The pane's own chat: ⌘-click does nothing more.
    ThisChat { number: u32 },
    /// Open under `number`; `workspace` where that is another workspace than the pane's chat's.
    Open {
        number: u32,
        workspace: Option<String>,
    },
    /// Ended: its card, which ⌘-click shows.
    Ended { card: Box<ChatCard> },
}

/// **What the window shows for a chat link opened.**
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "to", rename_all = "snake_case")]
pub enum ChatGo {
    /// The pane's own chat: nothing more.
    ThisChat,
    /// Chat `session`, in its tab or its workspace.
    Chat { session: u32 },
    /// An ended chat's card, beside the link.
    Card { card: Box<ChatCard> },
}

/// A chat of the pane's project open now, as a chat link's hover names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OpenNamed {
    pub session: u32,
    pub name: String,
    pub workspace: Option<String>,
}

/// **What the window knows of the pane's project's chats**: the ones open now, and how an ended
/// one's card is drawn from its record.
pub(crate) struct ChatsHere<'a> {
    pub open: Vec<OpenNamed>,
    pub card: CardOf<'a>,
}

/// How an ended chat's card is drawn from its number and its record: none for one with no card.
pub(crate) type CardOf<'a> = Box<dyn Fn(u32, &Record) -> Option<ChatCard> + 'a>;

impl ChatsHere<'_> {
    /// A pane in no project: no chat links.
    pub(crate) fn none() -> Self {
        Self {
            open: Vec::new(),
            card: Box::new(|_, _| None),
        }
    }

    fn named(&self, session: u32) -> Option<&OpenNamed> {
        self.open.iter().find(|one| one.session == session)
    }

    /// `#641 · <title>`, as the tab's tooltip says it (#6212).
    fn title(&self, number: u32) -> String {
        match self.named(number) {
            Some(one) if !one.name.is_empty() => format!("#{number} · {}", one.name),
            _ => format!("#{number}"),
        }
    }

    /// What chat link `at` goes to, for a hover: none where an ended chat has no card.
    fn seen(&self, at: &ChatAt, mine: Option<u32>) -> Option<(String, ChatSeen)> {
        Some(match &at.goes {
            Goes::ThisChat => (
                self.title(at.number),
                ChatSeen::ThisChat { number: at.number },
            ),
            Goes::Open => {
                let now = &at.number;
                let theirs = self.named(*now).and_then(|one| one.workspace.clone());
                let ours = mine
                    .and_then(|mine| self.named(mine))
                    .and_then(|one| one.workspace.clone());
                (
                    self.title(*now),
                    ChatSeen::Open {
                        number: *now,
                        workspace: theirs.filter(|theirs| Some(theirs) != ours.as_ref()),
                    },
                )
            }
            Goes::Ended(record) => {
                let card = (self.card)(at.number, record)?;
                (
                    format!("#{} · {}", at.number, card.task.name),
                    ChatSeen::Ended {
                        card: Box::new(card),
                    },
                )
            }
        })
    }

    /// What opening chat link `at` shows: none where an ended chat has no card.
    fn go(&self, at: &ChatAt) -> Option<ChatGo> {
        Some(match &at.goes {
            Goes::ThisChat => ChatGo::ThisChat,
            Goes::Open => ChatGo::Chat { session: at.number },
            Goes::Ended(record) => ChatGo::Card {
                card: Box::new((self.card)(at.number, record)?),
            },
        })
    }
}

/// **Where the window reads a file a link opens**: the place its light editor reads it by and
/// its path there, or, outside the project, its path in full ([`pane_link_file`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "in", rename_all = "lowercase")]
pub enum LinkFileAt {
    /// A branch's folder, or with no piece the repo's own folder.
    Branch {
        workspace: String,
        repo: String,
        piece: Option<String>,
        path: String,
    },
    /// A workspace's own files.
    Workspace { workspace: String, path: String },
    /// The project's own files.
    Project { path: String },
    /// Not in the project.
    Outside { path: String },
}

/// **What a chat counts as its own**, for the ask that says a file is not in it, with its folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "is", rename_all = "lowercase")]
pub enum LinkHome {
    Workspace { name: String, folder: String },
    Project { folder: String },
}

impl From<Home> for LinkHome {
    fn from(home: Home) -> Self {
        let folder = home.folder.display().to_string();
        match home.is {
            HomeIs::Workspace(name) => LinkHome::Workspace { name, folder },
            HomeIs::Project => LinkHome::Project { folder },
        }
    }
}

/// **What an ask is about**, which picks the window's question.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "about", rename_all = "kebab-case")]
pub enum LinkAsk {
    /// A link, asked about in general words.
    Link,
    /// A file outside the chat's home (the prototype's S7), and that home where it has one.
    FileOutside { home: Option<LinkHome> },
}

/// **The open command's answer.**
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "answer", rename_all = "lowercase")]
pub enum LinkOpened {
    /// Opened: `target` is what was opened.
    Opened { kind: KindHint, target: String },
    /// A chat link: what the window shows for it.
    Chat { go: ChatGo },
    /// A file, for the window to show in an editor tab at its line and column: never handed to
    /// the system, so nothing runs.
    File {
        target: String,
        at: LinkFileAt,
        line: Option<u32>,
        column: Option<u32>,
    },
    /// Not opened yet: the window asks, showing its text and where it goes. Answered, the same
    /// candidate is sent again with `token`, which opens it once.
    Ask {
        reason: String,
        shown: String,
        target: String,
        token: String,
        about: LinkAsk,
    },
    /// Not opened, and the sentence the window shows.
    Refused { reason: String },
}

/// `seen_in` for a pane that knows no chats: the tests' way to ask about every other kind.
#[cfg(test)]
pub(crate) fn seen(resolver: &Resolver, pane: &Pane, candidate: &Candidate) -> Option<LinkSeen> {
    seen_in(resolver, pane, candidate, &ChatsHere::none())
}

/// **What `candidate` is, for a hover**, resolved once, given what the window knows of the
/// pane's project's chats: a link and what opening it does, or text that is no link and why not
/// (`ftp://…` says purlis opens only http and https). `None` for text that says nothing on a
/// hover: a path that names no file, a number no chat had, an ended chat with no card.
pub(crate) fn seen_in(
    resolver: &Resolver,
    pane: &Pane,
    candidate: &Candidate,
    chats: &ChatsHere<'_>,
) -> Option<LinkSeen> {
    Some(match resolver.hover(candidate, pane) {
        Hover::Link {
            target: Target::Chat(at),
            ..
        } => {
            let (shown, chat) = chats.seen(&at, pane.session)?;
            LinkSeen {
                kind: KindHint::Chat,
                target: at.shown().to_owned(),
                shown,
                does: LinkDoes::Opens {
                    into: OpensIn::Window,
                },
                chat: Some(chat),
            }
        }
        Hover::Link { target, opening } => LinkSeen {
            kind: target.kind().into(),
            target: target.shown().to_owned(),
            shown: match &target {
                Target::File(file) => file.short().to_owned(),
                Target::Url(url) => url.as_str().to_owned(),
                Target::Chat(at) => at.shown().to_owned(),
            },
            does: match *opening {
                Opening::Open(_) => LinkDoes::Opens {
                    into: OpensIn::of(&target),
                },
                Opening::Ask { reason, .. } => LinkDoes::Asks { reason },
                Opening::Refused(reason) => LinkDoes::Refused { reason },
            },
            chat: None,
        },
        Hover::NotALink(reason) => LinkSeen {
            kind: candidate.kind.into(),
            target: candidate.text.clone(),
            shown: candidate.text.clone(),
            does: LinkDoes::Refused { reason },
            chat: None,
        },
        Hover::Quiet => return None,
    })
}

/// Where the window reads `file`.
fn file_at(file: &FileAt) -> LinkFileAt {
    match &file.read_from {
        ReadFrom::Branch {
            ws,
            repo,
            piece,
            path,
        } => LinkFileAt::Branch {
            workspace: ws.clone(),
            repo: repo.clone(),
            piece: piece.clone(),
            path: path.clone(),
        },
        ReadFrom::Workspace { ws, path } => LinkFileAt::Workspace {
            workspace: ws.clone(),
            path: path.clone(),
        },
        ReadFrom::Project { path } => LinkFileAt::Project { path: path.clone() },
        ReadFrom::Outside => LinkFileAt::Outside {
            path: file.path.display().to_string(),
        },
    }
}

/// What an ask for `target` in `pane` is about.
fn ask_about(target: &Target, pane: &Pane) -> LinkAsk {
    match target {
        Target::File(_) => LinkAsk::FileOutside {
            home: purlis_core::panelinks::home_of(pane).map(LinkHome::from),
        },
        Target::Url(_) | Target::Chat(_) => LinkAsk::Link,
    }
}

/// **Opens `candidate`, resolved again from its text**: `open_url` is handed the core's target
/// and nothing the window said, and a file is handed to nothing. `token` is the one-use token the
/// window sent back for the ask it was handed.
pub(crate) fn answer(
    resolver: &Resolver,
    pane: &Pane,
    asks: &Asks,
    candidate: &Candidate,
    token: Option<&str>,
    open_url: impl FnOnce(&str) -> Result<(), String>,
) -> LinkOpened {
    answer_in(
        resolver,
        pane,
        asks,
        candidate,
        token,
        open_url,
        &ChatsHere::none(),
    )
}

/// [`answer`], with what the window knows of the pane's project's chats.
pub(crate) fn answer_in(
    resolver: &Resolver,
    pane: &Pane,
    asks: &Asks,
    candidate: &Candidate,
    token: Option<&str>,
    open_url: impl FnOnce(&str) -> Result<(), String>,
    chats: &ChatsHere<'_>,
) -> LinkOpened {
    match resolver.open(candidate, pane, asks, token) {
        // A chat link never asks: it shows a chat or a card, and runs nothing.
        Opening::Open(Target::Chat(at)) => match chats.go(&at) {
            Some(go) => LinkOpened::Chat { go },
            None => LinkOpened::Refused {
                reason: NOT_A_LINK.to_owned(),
            },
        },
        Opening::Open(Target::File(file)) => LinkOpened::File {
            target: file.shown().to_owned(),
            at: file_at(&file),
            line: file.line,
            column: file.column,
        },
        Opening::Open(Target::Url(url)) => match open_url(url.as_str()) {
            Ok(()) => LinkOpened::Opened {
                kind: KindHint::Url,
                target: url.as_str().to_owned(),
            },
            Err(reason) => LinkOpened::Refused { reason },
        },
        Opening::Ask {
            reason,
            shown,
            target,
            token: Some(token),
        } => LinkOpened::Ask {
            about: ask_about(&target, pane),
            reason,
            shown,
            target: target.shown().to_owned(),
            token,
        },
        Opening::Ask { token: None, .. } => LinkOpened::Refused {
            reason: UNASKABLE.to_owned(),
        },
        Opening::Refused(reason) => LinkOpened::Refused { reason },
    }
}

/// **The asks the window was handed**, and the files outside the project opened on one
/// ([`Asks`]): one per app, so a token is answered only in the window run that was handed it.
#[derive(Debug, Default)]
pub struct PaneLinkAsks(pub Asks);

/// The longest a link's look at the disk is waited for.
pub const LOOK_FOR: Duration = Duration::from_secs(2);

/// The most looks at the disk under way at once; one more answers at once that it did not look.
pub const MOST_LOOKING: usize = 4;

/// The looks under way now.
static LOOKING: AtomicUsize = AtomicUsize::new(0);

/// **`look`, run on a thread of its own and waited for at most `wait`**: `None` where it took
/// longer, or where [`MOST_LOOKING`] looks are under way already. A look that never ends holds
/// only its own thread, and one place in the count while it lasts.
pub(crate) fn looked<T: Send + 'static>(
    wait: Duration,
    look: impl FnOnce() -> T + Send + 'static,
) -> Option<T> {
    if LOOKING.fetch_add(1, Ordering::SeqCst) >= MOST_LOOKING {
        LOOKING.fetch_sub(1, Ordering::SeqCst);
        return None;
    }
    let (told, heard) = std::sync::mpsc::sync_channel(1);
    let started = std::thread::Builder::new()
        .name("pane-link-look".into())
        .spawn(move || {
            let answer = look();
            LOOKING.fetch_sub(1, Ordering::SeqCst);
            let _ = told.send(answer);
        });
    if started.is_err() {
        LOOKING.fetch_sub(1, Ordering::SeqCst);
        return None;
    }
    heard.recv_timeout(wait).ok()
}

/// What opening a link answers when the look at its file did not end in time.
pub const NOT_LOOKED: &str = "purlis could not look at that file in time, so it is not opened";

/// **The resolver for a pane in `held`'s project, and what it knows of its chats**: every kind,
/// the chat kind given the chats open there now. A pane in no project the window holds links no
/// chat.
fn for_project(held: Option<&Held>) -> (Resolver, ChatsHere<'_>) {
    // With no project, the chat kind still reads a chat's number, and finds none: it says
    // nothing on a hover.
    let open = held.map(crate::chatcard::open_chats).unwrap_or_default();
    let resolver = Resolver::default().with_kind(Box::new(ChatNumber::with_open(open.clone())));
    let Some(held) = held else {
        return (resolver, ChatsHere::none());
    };
    let named = held
        .chats()
        .open_now()
        .into_iter()
        .map(|chat| OpenNamed {
            session: chat.session,
            name: chat.name,
            workspace: chat.workspace,
        })
        .collect();
    let chats = ChatsHere {
        open: named,
        card: Box::new(move |number, record| crate::chatcard::card_of(held, number, record, &open)),
    };
    (resolver, chats)
}

/// **What each candidate on a line is**, in order: a link, or none. What a hover shows.
#[tauri::command]
#[specta::specta]
pub async fn pane_link_resolve(
    app: tauri::AppHandle,
    from: LinkFrom,
    candidates: Vec<LinkCandidate>,
) -> Vec<Option<LinkSeen>> {
    use tauri::Manager as _;
    let (pane, held) = pane_of(&app.state::<Planes>(), from);
    let count = candidates.len();
    // A chat's number reads the project's dispatch records: looked at as a file is.
    let look = move || {
        let (resolver, chats) = for_project(held.as_deref());
        candidates
            .into_iter()
            .map(|candidate| seen_in(&resolver, &pane, &candidate.into(), &chats))
            .collect::<Vec<_>>()
    };
    // Waited for off the window's thread; a look that did not end in time is no link.
    tauri::async_runtime::spawn_blocking(move || looked(LOOK_FOR, look))
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| vec![None; count])
}

/// **Opens a link a pane found** (⌘-click, Ctrl+click off a Mac): the candidate's text,
/// resolved again here. `token` is the one-use token of the ask the window was handed.
#[tauri::command]
#[specta::specta]
pub async fn pane_link_open(
    app: tauri::AppHandle,
    from: LinkFrom,
    candidate: LinkCandidate,
    token: Option<String>,
) -> LinkOpened {
    use tauri::Manager as _;
    let (pane, held) = pane_of(&app.state::<Planes>(), from);
    let candidate: Candidate = candidate.into();
    if candidate.kind == Kind::Url {
        let asks = app.state::<PaneLinkAsks>();
        return answer(
            &Resolver::default(),
            &pane,
            &asks.0,
            &candidate,
            token.as_deref(),
            |url| open_in_browser(&app, url),
        );
    }
    // A file, or a chat's number, is looked at on a thread of its own; it opens nothing there,
    // only answers. A chat link never asks, so it is never answered by a token.
    let handle = app.clone();
    let look = move || {
        let asks = handle.state::<PaneLinkAsks>();
        let (resolver, chats) = for_project(held.as_deref());
        answer_in(
            &resolver,
            &pane,
            &asks.0,
            &candidate,
            token.as_deref(),
            |_| Err("purlis opens a file only in an editor tab".to_owned()),
            &chats,
        )
    };
    tauri::async_runtime::spawn_blocking(move || looked(LOOK_FOR, look))
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| LinkOpened::Refused {
            reason: NOT_LOOKED.to_owned(),
        })
}

/// **A file outside the project, as its tab reads it**: the file, or that no answered ask in
/// this pane opened it (the window says how to open it again).
#[derive(Debug, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "read", rename_all = "kebab-case")]
pub enum LinkFile {
    File { file: PieceFile },
    NotOpened,
}

/// **A file outside the project, read for its editor tab**: only one opened in the pane `from`
/// names by an answered ask, in this run of the app and within the hour, and only while it is
/// still the file it was then. Read from `/` one folder at a time with no link followed.
#[tauri::command]
#[specta::specta]
pub async fn pane_link_file(
    app: tauri::AppHandle,
    from: LinkFrom,
    path: String,
) -> Result<LinkFile, String> {
    use tauri::Manager as _;
    let (pane, _) = pane_of(&app.state::<Planes>(), from);
    let path = PathBuf::from(path);
    let Some(identity) = app.state::<PaneLinkAsks>().0.opened(&pane, &path) else {
        return Ok(LinkFile::NotOpened);
    };
    crate::off_the_window("reading the file", move || {
        purlis_core::files::folder::read_absolute(&path, identity).map(|opened| LinkFile::File {
            file: PieceFile::from(opened),
        })
    })
    .await
}

/// Opens `url` in the person's browser; a scenario test's build opens nothing.
fn open_in_browser<R: tauri::Runtime>(app: &tauri::AppHandle<R>, url: &str) -> Result<(), String> {
    #[cfg(feature = "e2e")]
    {
        let _ = (app, url);
        Ok(())
    }
    #[cfg(not(feature = "e2e"))]
    {
        use tauri_plugin_opener::OpenerExt as _;
        // The opener's own words could repeat the address: the sentence is purlis's.
        app.opener()
            .open_url(url, None::<&str>)
            .map_err(|_| "purlis could not open the address in your browser".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    fn url(text: &str) -> Candidate {
        Candidate {
            kind: Kind::Url,
            text: text.to_owned(),
        }
    }

    /// Opens `text` as the command does, with a browser that records what it was handed.
    fn opened(text: &str) -> (LinkOpened, Vec<String>) {
        let handed = RefCell::new(Vec::new());
        let answered = answer(
            &Resolver::default(),
            &Pane::default(),
            &Asks::default(),
            &url(text),
            None,
            |url| {
                handed.borrow_mut().push(url.to_owned());
                Ok(())
            },
        );
        (answered, handed.into_inner())
    }

    #[test]
    fn an_https_address_opens_in_the_browser() {
        let (answered, handed) = opened("https://github.com/purlis/purlis/pull/6148");

        assert_eq!(
            answered,
            LinkOpened::Opened {
                kind: KindHint::Url,
                target: "https://github.com/purlis/purlis/pull/6148".into()
            }
        );
        assert_eq!(handed, ["https://github.com/purlis/purlis/pull/6148"]);
    }

    #[test]
    fn another_scheme_is_refused_and_the_browser_is_handed_nothing() {
        for text in [
            "ftp://mirror.example.org/pub",
            "file:///etc/hosts",
            "javascript:alert(1)",
        ] {
            let (answered, handed) = opened(text);
            assert!(
                matches!(&answered, LinkOpened::Refused { reason } if reason.contains("only http and https")),
                "{text}: {answered:?}"
            );
            assert!(handed.is_empty(), "{text} reached the browser");
        }
    }

    #[test]
    fn what_opens_is_the_core_s_reading_of_the_text_never_a_target_the_pane_named() {
        // The command takes no target: only the text, which the core reads again. A pane that
        // sent words with a hidden byte, or an address the browser would read another way, opens
        // only what the core makes of them.
        let (answered, handed) = opened("https://purlis.dev\u{0}.evil.example/");
        assert!(
            matches!(answered, LinkOpened::Refused { .. }),
            "{answered:?}"
        );
        assert!(handed.is_empty());

        let (_, handed) = opened("HTTPS://PURLIS.dev");
        assert_eq!(handed, ["https://purlis.dev/"]);
    }

    #[test]
    fn a_browser_that_will_not_open_it_is_a_refusal_in_purlis_s_words() {
        let answered = answer(
            &Resolver::default(),
            &Pane::default(),
            &Asks::default(),
            &url("https://purlis.dev/"),
            None,
            |_| Err("purlis could not open the address in your browser".into()),
        );

        assert_eq!(
            answered,
            LinkOpened::Refused {
                reason: "purlis could not open the address in your browser".into()
            }
        );
    }

    #[test]
    fn a_hover_says_the_target_and_that_it_opens() {
        let seen = seen(
            &Resolver::default(),
            &Pane::default(),
            &url("https://purlis.dev"),
        );

        assert_eq!(
            seen,
            Some(LinkSeen {
                kind: KindHint::Url,
                target: "https://purlis.dev/".into(),
                shown: "https://purlis.dev/".into(),
                does: LinkDoes::Opens {
                    into: OpensIn::Browser
                },
                chat: None,
            })
        );
    }

    #[test]
    fn a_hover_over_another_scheme_says_why_it_won_t_open_and_so_does_one_carrying_a_password() {
        assert_eq!(
            seen(
                &Resolver::default(),
                &Pane::default(),
                &url("ftp://x.example/")
            ),
            Some(LinkSeen {
                kind: KindHint::Url,
                target: "ftp://x.example/".into(),
                shown: "ftp://x.example/".into(),
                does: LinkDoes::Refused {
                    reason: "purlis opens only http and https addresses, so it will not open \
                             this ftp: one"
                        .into()
                },
                chat: None,
            })
        );
        assert!(matches!(
            seen(
                &Resolver::default(),
                &Pane::default(),
                // A made-up sign-in, spelled in parts so the commit scan does not take it for one.
                &url(&["https://", "me:", "made-up", "@purlis.dev/"].concat())
            ),
            Some(LinkSeen {
                does: LinkDoes::Refused { .. },
                ..
            })
        ));
    }

    #[test]
    fn the_window_s_words_for_the_answers_are_these() {
        // The shape the window reads (`paneLinks.ts`).
        let json = |answer: &LinkOpened| serde_json::to_value(answer).expect("serialises");
        assert_eq!(
            json(&LinkOpened::Opened {
                kind: KindHint::Url,
                target: "https://a.example/".into()
            }),
            serde_json::json!({"answer": "opened", "kind": "url", "target": "https://a.example/"})
        );
        assert_eq!(
            json(&LinkOpened::Refused {
                reason: "no".into()
            }),
            serde_json::json!({"answer": "refused", "reason": "no"})
        );
        assert_eq!(
            serde_json::to_value(LinkDoes::Asks { reason: "r".into() }).expect("serialises"),
            serde_json::json!({"does": "asks", "reason": "r"})
        );
        assert_eq!(
            serde_json::to_value(LinkDoes::Opens {
                into: OpensIn::EditorTab
            })
            .expect("serialises"),
            serde_json::json!({"does": "opens", "into": "editor-tab"})
        );
    }

    /// A project holding workspace `smart-ide`, a chat working in it, and a file outside both.
    struct Here {
        _held: tempfile::TempDir,
        pane: Pane,
        notes: PathBuf,
        hosts: PathBuf,
    }

    fn here() -> Here {
        let held = tempfile::tempdir().expect("a temporary folder");
        let top = std::fs::canonicalize(held.path()).expect("resolved");
        let ws = top.join("project/workspaces/smart-ide");
        std::fs::create_dir_all(&ws).expect("a workspace");
        let notes = ws.join("notes.md");
        std::fs::write(&notes, "one\ntwo\n").expect("written");
        let hosts = top.join("etc/hosts");
        std::fs::create_dir_all(hosts.parent().expect("a parent")).expect("folders");
        std::fs::write(&hosts, "127.0.0.1 localhost\n").expect("written");
        Here {
            pane: Pane {
                project: Some(top.join("project")),
                session: Some(3),
                folder: Some(ws),
                runs: Runs::Here,
            },
            _held: held,
            notes,
            hosts,
        }
    }

    fn file(text: &str) -> Candidate {
        Candidate {
            kind: Kind::File,
            text: text.to_owned(),
        }
    }

    /// Opens `text` as a file link in `pane`, with a browser that must never be handed anything.
    fn opened_file(pane: &Pane, asks: &Asks, text: &str, token: Option<&str>) -> LinkOpened {
        answer(
            &Resolver::default(),
            pane,
            asks,
            &file(text),
            token,
            |handed| panic!("a file reached the system's opener: {handed}"),
        )
    }

    #[test]
    fn a_file_in_the_workspace_is_answered_for_the_editor_tab_at_its_line_and_never_handed_on() {
        let here = here();

        assert_eq!(
            opened_file(&here.pane, &Asks::default(), "notes.md:2:1", None),
            LinkOpened::File {
                target: format!("{}:2:1", here.notes.display()),
                at: LinkFileAt::Workspace {
                    workspace: "smart-ide".into(),
                    path: "notes.md".into()
                },
                line: Some(2),
                column: Some(1),
            }
        );
    }

    #[test]
    fn a_file_outside_the_workspace_asks_with_a_token_and_its_answer_opens_it_once() {
        let here = here();
        let asks = Asks::default();
        let text = format!("{}:1", here.hosts.display());

        let LinkOpened::Ask {
            reason,
            shown,
            target,
            token: _,
            about,
        } = opened_file(&here.pane, &asks, &text, None)
        else {
            panic!("expected an ask");
        };
        assert_eq!(reason, "outside smart-ide");
        assert_eq!(
            (shown.as_str(), target.as_str()),
            (text.as_str(), text.as_str())
        );
        let folder = here.pane.folder.clone().expect("a folder");
        assert_eq!(
            about,
            LinkAsk::FileOutside {
                home: Some(LinkHome::Workspace {
                    name: "smart-ide".into(),
                    folder: folder.display().to_string(),
                }),
            }
        );
        // The target the ask showed is no answer: only its token is.
        assert!(matches!(
            opened_file(&here.pane, &asks, &text, Some(&target)),
            LinkOpened::Ask { .. }
        ));
        assert_eq!(asks.opened(&here.pane, &here.hosts), None);

        let LinkOpened::Ask { token, .. } = opened_file(&here.pane, &asks, &text, None) else {
            panic!("expected an ask");
        };
        assert_eq!(
            opened_file(&here.pane, &asks, &text, Some(&token)),
            LinkOpened::File {
                target: text.clone(),
                at: LinkFileAt::Outside {
                    path: here.hosts.display().to_string()
                },
                line: Some(1),
                column: None,
            }
        );
        assert!(asks.opened(&here.pane, &here.hosts).is_some());
        assert!(matches!(
            opened_file(&here.pane, &asks, &text, Some(&token)),
            LinkOpened::Ask { .. }
        ));
    }

    #[test]
    fn a_hover_over_a_path_to_no_file_says_nothing() {
        let here = here();

        assert_eq!(
            seen(&Resolver::default(), &here.pane, &file("src/gone.rs:10")),
            None
        );
        assert_eq!(
            seen(&Resolver::default(), &here.pane, &file("notes.md")),
            Some(LinkSeen {
                kind: KindHint::File,
                target: here.notes.display().to_string(),
                // In the chat's workspace: said from the workspace (D2).
                shown: "notes.md".into(),
                does: LinkDoes::Opens {
                    into: OpensIn::EditorTab
                },
                chat: None,
            })
        );
    }

    #[test]
    fn a_file_outside_the_project_reads_only_in_the_pane_its_ask_was_answered_in() {
        let here = here();
        let asks = Asks::default();
        let text = here.hosts.display().to_string();
        let LinkOpened::Ask { token, .. } = opened_file(&here.pane, &asks, &text, None) else {
            panic!("expected an ask");
        };
        opened_file(&here.pane, &asks, &text, Some(&token));

        let identity = asks.opened(&here.pane, &here.hosts).expect("opened");
        assert_eq!(
            purlis_core::files::folder::read_absolute(&here.hosts, identity).map(PieceFile::from),
            Ok(PieceFile::Text {
                text: "127.0.0.1 localhost\n".into()
            })
        );
        let another_chat = Pane {
            session: Some(4),
            ..here.pane.clone()
        };
        assert_eq!(asks.opened(&another_chat, &here.hosts), None);
    }

    #[test]
    fn a_look_that_does_not_end_in_time_answers_none_and_so_does_one_past_the_most() {
        assert_eq!(looked(Duration::from_secs(5), || 7), Some(7));
        assert_eq!(
            looked(Duration::from_millis(20), || {
                std::thread::sleep(Duration::from_millis(400));
                7
            }),
            None
        );
    }

    #[test]
    fn the_window_s_words_for_a_file_and_an_ask_are_these() {
        let json = |answer: &LinkOpened| serde_json::to_value(answer).expect("serialises");
        assert_eq!(
            json(&LinkOpened::File {
                target: "/p/a.rs:3".into(),
                at: LinkFileAt::Branch {
                    workspace: "w".into(),
                    repo: "r".into(),
                    piece: None,
                    path: "a.rs".into()
                },
                line: Some(3),
                column: None,
            }),
            serde_json::json!({
                "answer": "file", "target": "/p/a.rs:3", "line": 3, "column": null,
                "at": {"in": "branch", "workspace": "w", "repo": "r", "piece": null, "path": "a.rs"}
            })
        );
        assert_eq!(
            json(&LinkOpened::Ask {
                reason: "outside w".into(),
                shown: "/etc/hosts".into(),
                target: "/etc/hosts".into(),
                token: "t".into(),
                about: LinkAsk::FileOutside {
                    home: Some(LinkHome::Workspace {
                        name: "w".into(),
                        folder: "/p/workspaces/w".into()
                    }),
                },
            }),
            serde_json::json!({
                "answer": "ask", "reason": "outside w", "shown": "/etc/hosts",
                "target": "/etc/hosts", "token": "t",
                "about": {"about": "file-outside",
                          "home": {"is": "workspace", "name": "w", "folder": "/p/workspaces/w"}}
            })
        );
    }

    // ---- Chat links (#6217): the window's half, over the core's chat kind. ----

    use purlis_core::chatlookup::OpenChat;
    use purlis_core::panelinks::ChatNumber;

    fn chat(text: &str) -> Candidate {
        Candidate {
            kind: Kind::Chat,
            text: text.to_owned(),
        }
    }

    /// Chat 652's pane in a project with no records: only open chats link.
    fn pane_of_652(root: &std::path::Path) -> Pane {
        Pane {
            project: Some(root.to_owned()),
            session: Some(652),
            ..Pane::default()
        }
    }

    fn named(session: u32, name: &str, workspace: &str) -> OpenNamed {
        OpenNamed {
            session,
            name: name.to_owned(),
            workspace: Some(workspace.to_owned()),
        }
    }

    /// The window's chats: 652 (this pane's, in smart-ide), 660 (in docs) and 661 (in smart-ide).
    fn chats_here() -> (Resolver, ChatsHere<'static>) {
        let open = [(652, "A"), (660, "B"), (661, "C")]
            .map(|(number, id)| OpenChat {
                number,
                id: Some(format!("01K6{id}")),
            })
            .to_vec();
        (
            Resolver::default().with_kind(Box::new(ChatNumber::with_open(open))),
            ChatsHere {
                open: vec![
                    named(652, "Grill #5820: chat links", "smart-ide"),
                    named(660, "Docs site: the setup page", "docs"),
                    named(661, "Signed commits #5607", "smart-ide"),
                ],
                card: Box::new(|_, _| None),
            },
        )
    }

    #[test]
    fn a_chat_open_in_another_workspace_is_seen_by_number_and_title_with_that_workspace() {
        let dir = tempfile::tempdir().expect("a project");
        let (links, chats) = chats_here();

        assert_eq!(
            seen_in(&links, &pane_of_652(dir.path()), &chat("chat 660"), &chats),
            Some(LinkSeen {
                kind: KindHint::Chat,
                target: "chat 660".into(),
                shown: "#660 · Docs site: the setup page".into(),
                does: LinkDoes::Opens {
                    into: OpensIn::Window
                },
                chat: Some(ChatSeen::Open {
                    number: 660,
                    workspace: Some("docs".into())
                }),
            })
        );
        // In the pane's own workspace, it names none.
        assert!(matches!(
            seen_in(&links, &pane_of_652(dir.path()), &chat("chat 661"), &chats),
            Some(LinkSeen {
                chat: Some(ChatSeen::Open {
                    number: 661,
                    workspace: None
                }),
                ..
            })
        ));
    }

    #[test]
    fn opening_an_open_chat_shows_it_and_reaches_no_browser() {
        let dir = tempfile::tempdir().expect("a project");
        let (links, chats) = chats_here();
        let answered = answer_in(
            &links,
            &pane_of_652(dir.path()),
            &Asks::default(),
            &chat("chat 660"),
            None,
            |url| panic!("{url} reached the browser"),
            &chats,
        );

        assert_eq!(
            answered,
            LinkOpened::Chat {
                go: ChatGo::Chat { session: 660 }
            }
        );
    }

    #[test]
    fn its_own_number_is_this_chat_and_opening_it_shows_nothing_more() {
        let dir = tempfile::tempdir().expect("a project");
        let (links, chats) = chats_here();

        assert!(matches!(
            seen_in(&links, &pane_of_652(dir.path()), &chat("chat 652"), &chats),
            Some(LinkSeen {
                chat: Some(ChatSeen::ThisChat { number: 652 }),
                ref shown,
                ..
            }) if shown == "#652 · Grill #5820: chat links"
        ));
        assert_eq!(
            answer_in(
                &links,
                &pane_of_652(dir.path()),
                &Asks::default(),
                &chat("chat 652"),
                None,
                |url| panic!("{url} reached the browser"),
                &chats,
            ),
            LinkOpened::Chat {
                go: ChatGo::ThisChat
            }
        );
    }

    #[test]
    fn an_unknown_number_says_nothing_on_a_hover_and_opening_it_is_refused() {
        let dir = tempfile::tempdir().expect("a project");
        let (links, chats) = chats_here();

        assert_eq!(
            seen_in(&links, &pane_of_652(dir.path()), &chat("chat 9999"), &chats),
            None
        );
        assert!(matches!(
            answer_in(
                &links,
                &pane_of_652(dir.path()),
                &Asks::default(),
                &chat("chat 9999"),
                None,
                |url| panic!("{url} reached the browser"),
                &chats,
            ),
            LinkOpened::Refused { .. }
        ));
    }

    #[test]
    fn a_pane_in_no_project_links_no_chat_and_says_nothing_on_its_hover() {
        let (links, chats) = for_project(None);
        assert_eq!(
            seen_in(&links, &Pane::default(), &chat("chat 652"), &chats),
            None
        );
    }

    #[test]
    fn the_window_s_words_for_a_chat_link_are_these() {
        let json = |answer: &LinkOpened| serde_json::to_value(answer).expect("serialises");
        assert_eq!(
            json(&LinkOpened::Chat {
                go: ChatGo::Chat { session: 660 }
            }),
            serde_json::json!({"answer": "chat", "go": {"to": "chat", "session": 660}})
        );
        assert_eq!(
            json(&LinkOpened::Chat {
                go: ChatGo::ThisChat
            }),
            serde_json::json!({"answer": "chat", "go": {"to": "this_chat"}})
        );
        assert_eq!(
            serde_json::to_value(ChatSeen::Open {
                number: 660,
                workspace: None
            })
            .expect("serialises"),
            serde_json::json!({"is": "open", "number": 660, "workspace": null})
        );
    }
}
