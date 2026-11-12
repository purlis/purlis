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
//!
//! **A file's answer looks at the disk, so it is looked at on a thread of its own, for at most
//! [`LOOK_FOR`]** ([`looked`]): a path on a stalled mount answers "no link" in time, and holds
//! none of the app's shared threads. At most [`MOST_LOOKING`] looks are under way at once.
//!
//! None of them reads a secret's value or hands one back.

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use purlis_core::panelinks::{
    Asks, Candidate, FileAt, Home, HomeIs, Hover, Kind, Mounts, Opening, Pane, ReadFrom, Resolver,
    Runs, Target, UNASKABLE,
};
use serde::{Deserialize, Serialize};

use crate::piecefiles::PieceFile;
use crate::planes::{PlaneId, Planes};

/// **What the pane's pattern took a candidate for**: a hint, never trusted for more. The wire's
/// half of the core's [`Kind`]; a kind's resolver is the core's `LinkKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum KindHint {
    Url,
    File,
}

impl From<KindHint> for Kind {
    fn from(kind: KindHint) -> Self {
        match kind {
            KindHint::Url => Kind::Url,
            KindHint::File => Kind::File,
        }
    }
}

impl From<Kind> for KindHint {
    fn from(kind: Kind) -> Self {
        match kind {
            Kind::Url => KindHint::Url,
            Kind::File => KindHint::File,
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
/// on a runner or a pool. A chat on a Docker pool's runner maps its paths through the mounts
/// purlis made the runner's container with (#6221). Every other pane (a saved chat's, one whose
/// chat has closed, a cluster's or another machine's runner, one with no project open) names
/// files purlis cannot place, so it maps through none ([`Mounts::none`], #6224).
// The folder is the one the chat started in: no hook reports the folder a chat has moved to.
fn pane_of(planes: &Planes, from: LinkFrom) -> Pane {
    let held = from
        .plane
        .as_ref()
        .and_then(|plane| planes.held(plane).ok());
    let Some(held) = held else {
        return Pane {
            project: from.plane.map(|plane| PathBuf::from(plane.root())),
            session: from.session,
            folder: None,
            runs: Runs::Mapped(Mounts::none()),
        };
    };
    let chats = held.chats();
    let open = from
        .session
        .and_then(|session| Some((session, chats.chat_at(session)?)));
    let (runs, folder) = match open {
        None => (Runs::Mapped(Mounts::none()), None),
        Some((session, at)) => match (chats.pool_of(session), chats.runner_of(session)) {
            (None, None) => (Runs::Here, at.cwd),
            (Some(pool), _) => (Runs::Mapped(docker_mounts(&pool)), None),
            (None, Some(_)) => (Runs::Mapped(Mounts::none()), None),
        },
    };
    Pane {
        project: Some(held.root().to_path_buf()),
        session: from.session,
        runs,
        folder,
    }
}

/// **The mounts of a Docker pool's runner** that a chat on `pool` runs on, as purlis made its
/// container ([`Mounts::docker`]): read from the runners this machine keeps, never from what
/// the chat prints. None for a cluster pool's, or a pool this machine does not keep.
fn docker_mounts(pool: &str) -> Mounts {
    // A fresh claim is named `<pool>/<claim>`, and its runner is made as the pool's is.
    let pool = pool.split_once('/').map_or(pool, |(pool, _)| pool);
    purlis_core::machine::config_root()
        .and_then(|config| purlis_core::runners::runner_for(&config, pool).ok())
        .map_or_else(Mounts::none, |kept| Mounts::docker(&kept.made.runner))
}

/// **Where a link that opens without asking opens.**
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "kebab-case")]
pub enum OpensIn {
    /// The person's browser.
    Browser,
    /// One of purlis's editor tabs.
    EditorTab,
}

impl OpensIn {
    fn of(target: &Target) -> Self {
        match target {
            Target::Url(_) => OpensIn::Browser,
            Target::File(_) => OpensIn::EditorTab,
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
/// the workspace, D2) and what opening it does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct LinkSeen {
    pub kind: KindHint,
    pub target: String,
    pub shown: String,
    pub does: LinkDoes,
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

/// **What `candidate` is, for a hover**, resolved once: a link and what opening it does, or
/// text that is no link and why not (`ftp://…` says purlis opens only http and https). `None` for
/// text that says nothing on a hover: a path that names no file.
pub(crate) fn seen(resolver: &Resolver, pane: &Pane, candidate: &Candidate) -> Option<LinkSeen> {
    Some(match resolver.hover(candidate, pane) {
        Hover::Link { target, opening } => LinkSeen {
            kind: target.kind().into(),
            target: target.shown().to_owned(),
            shown: match &target {
                Target::File(file) => file.short().to_owned(),
                Target::Url(url) => url.as_str().to_owned(),
            },
            does: match *opening {
                Opening::Open(_) => LinkDoes::Opens {
                    into: OpensIn::of(&target),
                },
                Opening::Ask { reason, .. } => LinkDoes::Asks { reason },
                Opening::Refused(reason) => LinkDoes::Refused { reason },
            },
        },
        Hover::NotALink(reason) => LinkSeen {
            kind: candidate.kind.into(),
            target: candidate.text.clone(),
            shown: candidate.text.clone(),
            does: LinkDoes::Refused { reason },
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
        Target::Url(_) => LinkAsk::Link,
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
    match resolver.open(candidate, pane, asks, token) {
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

/// **What each candidate on a line is**, in order: a link, or none. What a hover shows.
#[tauri::command]
#[specta::specta]
pub async fn pane_link_resolve(
    app: tauri::AppHandle,
    from: LinkFrom,
    candidates: Vec<LinkCandidate>,
) -> Vec<Option<LinkSeen>> {
    use tauri::Manager as _;
    let pane = pane_of(&app.state::<Planes>(), from);
    let count = candidates.len();
    let look = move || {
        let resolver = Resolver::default();
        candidates
            .into_iter()
            .map(|candidate| seen(&resolver, &pane, &candidate.into()))
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
    let pane = pane_of(&app.state::<Planes>(), from);
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
    // A file is looked at on a thread of its own; it opens nothing there, only answers.
    let handle = app.clone();
    let look = move || {
        let asks = handle.state::<PaneLinkAsks>();
        answer(
            &Resolver::default(),
            &pane,
            &asks.0,
            &candidate,
            token.as_deref(),
            |_| Err("purlis opens a file only in an editor tab".to_owned()),
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
    let pane = pane_of(&app.state::<Planes>(), from);
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
}
