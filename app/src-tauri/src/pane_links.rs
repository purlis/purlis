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

use purlis_core::files::{Answer, Ask, Branch, Reader};
use purlis_core::panelinks::{
    Asking, Asks, Candidate, ChatAt, ChatNumber, FileAt, Goes, Home, HomeIs, Hover, Kind,
    LocalFile, NOT_A_LINK, Opening, Pane, ReadFrom, Resolver, Runs, Target, UNASKABLE, WebAddress,
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
    // A link a program marked itself (OSC 8, #6218): its words, and the target it named.
    Osc8,
}

impl From<KindHint> for Kind {
    fn from(kind: KindHint) -> Self {
        match kind {
            KindHint::Url => Kind::Url,
            KindHint::File => Kind::File,
            KindHint::Chat => Kind::Chat,
            KindHint::Osc8 => Kind::Osc8,
        }
    }
}

impl From<Kind> for KindHint {
    fn from(kind: Kind) -> Self {
        match kind {
            Kind::Url => KindHint::Url,
            Kind::File => KindHint::File,
            Kind::Chat => KindHint::Chat,
            Kind::Osc8 => KindHint::Osc8,
        }
    }
}

/// **Text a pane found that may be a link**: the words as printed, and the pattern's hint. A
/// link a program marked (`osc8`) also carries `target`, the address the program named, as the
/// pane was handed it: the core reads it again, and a target the pane resolved is never sent.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, specta::Type)]
pub struct LinkCandidate {
    pub kind: KindHint,
    pub text: String,
    #[serde(default)]
    #[specta(optional)]
    pub target: Option<String>,
}

impl From<LinkCandidate> for Candidate {
    fn from(candidate: LinkCandidate) -> Self {
        Candidate {
            kind: candidate.kind.into(),
            text: candidate.text,
            target: candidate.target,
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
    /// The words of a link a program marked, where they name another target than it goes to.
    #[specta(optional)]
    pub its_text: Option<String>,
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

/// How an ended or closed chat's card is drawn from its number and where its link goes (its
/// dispatch record, or the app's record of a chat no dispatch started, #6304): none for one with
/// no card.
pub(crate) type CardOf<'a> = Box<dyn Fn(u32, &Goes) -> Option<ChatCard> + 'a>;

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
            Goes::Ended(_) | Goes::Closed(_) => {
                let card = (self.card)(at.number, &at.goes)?;
                (
                    format!("#{} · {}", at.number, card.title),
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
            Goes::Ended(_) | Goes::Closed(_) => ChatGo::Card {
                card: Box::new((self.card)(at.number, &at.goes)?),
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
    /// A link a program marked whose words read as another address or path (the prototype's
    /// S8, #6218): what its target is, and a web address's host, which the ask draws apart.
    Misleading {
        kind: KindHint,
        host: Option<String>,
    },
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
                its_text: None,
            }
        }
        Hover::Link { target, opening } => LinkSeen {
            its_text: match &*opening {
                Opening::Ask {
                    about: Asking::Misleading,
                    shown,
                    ..
                } => Some(shown.clone()),
                _ => None,
            },
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
        // A marked link that is none says the target the program named, which is what it hides.
        Hover::NotALink(reason) => {
            let named = candidate.target.as_ref().unwrap_or(&candidate.text);
            LinkSeen {
                kind: candidate.kind.into(),
                target: named.clone(),
                shown: named.clone(),
                does: LinkDoes::Refused { reason },
                its_text: None,
                chat: None,
            }
        }
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

/// What an ask for `target` in `pane`, about `about`, is about in the window's words.
fn ask_about(about: Asking, target: &Target, pane: &Pane) -> LinkAsk {
    match about {
        Asking::Link => LinkAsk::Link,
        Asking::FileOutside => LinkAsk::FileOutside {
            home: purlis_core::panelinks::home_of(pane).map(LinkHome::from),
        },
        Asking::Misleading => LinkAsk::Misleading {
            kind: target.kind().into(),
            host: match target {
                Target::Url(url) => url.host_str().map(str::to_owned),
                Target::File(_) | Target::Chat(_) => None,
            },
        },
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
            about,
        } => LinkOpened::Ask {
            about: ask_about(about, &target, pane),
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

/// **The window's resolver** (#6357): web addresses, and file paths whose branch's files are
/// asked of the bounded reader, so a file a branch doesn't offer is no link, as the light editor
/// would refuse it, and the app's process starts no git in a folder an agent can write.
///
/// Made for one command: each branch's offered list is read once for all its candidates, so
/// *Links in this pane…*, which asks a whole screen at once, reads it once too.
pub(crate) fn resolver() -> Resolver {
    let reader = crate::reader();
    let lists = std::sync::Mutex::new(Lists::new());
    Resolver::with(vec![
        Box::new(WebAddress),
        Box::new(LocalFile::here().offered_by(move |project, branch, path| {
            let key = (
                branch.ws.to_owned(),
                branch.repo.to_owned(),
                branch.piece.map(str::to_owned),
            );
            let mut lists = lists.lock().ok()?;
            let list = lists
                .entry(key)
                .or_insert_with(|| offered_list(&reader, project, branch));
            Some(
                list.as_ref()?
                    .binary_search_by(|one| one.as_str().cmp(path))
                    .is_ok(),
            )
        })),
    ])
}

/// Each branch's offered list a resolver read, by workspace, repo and piece.
type Lists = std::collections::HashMap<(String, String, Option<String>), Option<Vec<String>>>;

/// **What `branch` offers**: the list the light editor's open checks a path against (what git
/// tracks there, or doesn't ignore), sorted. `None` where the reader could not say (an index
/// gitoxide can't read, a read that ran into its bounds, a list cut short): the link stays, and
/// the editor decides.
fn offered_list(
    reader: &Reader,
    project: &std::path::Path,
    branch: Branch<'_>,
) -> Option<Vec<String>> {
    match reader.ask(project, branch, Ask::Offered { most: None }) {
        Ok(Answer::Offered(offered)) if offered.files.len() == offered.total => Some(offered.files),
        _ => None,
    }
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
    let resolver = resolver().with_kind(Box::new(ChatNumber::with_open(open.clone())));
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
        card: Box::new(move |number, goes| match goes {
            Goes::Ended(record) => crate::chatcard::card_of(held, number, record, &open),
            Goes::Closed(chat) => Some(crate::chatcard::card_of_chat(held, chat)),
            Goes::ThisChat | Goes::Open => None,
        }),
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
    // A web address opens here, in the browser; a file, or a marked link purlis refuses, is
    // answered on a thread of its own.
    if candidate.read_by() == Some(Kind::Url) {
        let asks = app.state::<PaneLinkAsks>();
        return answer(
            &resolver(),
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
mod tests;
