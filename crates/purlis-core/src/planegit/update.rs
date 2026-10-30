// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Update from remote** (qw348): the window's own action that brings a project's incoming
//! commits in, for the person who pressed it.
//!
//! A project behind its remote could be brought up to date only by a chat running git in the
//! project's root, which the chat's sandbox stops partway (qw346) and the guard now refuses, or
//! by the person in a terminal. [`update`] is the third way: purlis runs it itself, under no
//! chat's sandbox.
//!
//! **What it may reach is what a fetch may reach.** The network half is [`super::fetch`]'s own
//! ([`super::fetch_branch`]): one branch of the origin the project names, by a URL purlis
//! built, through the runner's network entry point, with the window's sign-in offered only to
//! the remote recorded for the project. Every git call here goes through the runner, so no hook
//! and no program the repository's config names is started.
//!
//! **Either the project is at the new commit, or it is exactly as it was.**
//! - Nothing is started on a tree with unsaved work, on a tree git stopped part-way, off the
//!   target branch, where a file is in the way, or where a file the incoming commits change may
//!   not be written ([`look`]).
//! - A move that fails anyway is undone ([`put_back`]): a rebase that started is aborted, and
//!   what a failed checkout wrote is restored to the commit the project was on, then checked.
//!   Where that check fails, the answer says so and names the files ([`Left::NotPutBack`]):
//!   it never says "as it was" of a tree that is not.
//!
//! **A request mode moves its target branch through its request** (ADR 0051). Commits here
//! that an open pull request carries are never replayed onto the remote's branch: the project
//! moves when the request merges ([`prsave::settle`]), and until then the update says what it
//! waits on. Commits no request carries yet are replayed, as in the other modes: nothing of
//! them is on the remote's target branch, and the next save pushes them to the save branch.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::{
    Claim, Rebased, WRITE, changed_paths, count, fetch_branch, git_dir_of, prsave,
    rebase_in_progress, rebase_onto, tail,
};
use crate::counts::plural;
use crate::forge;
use crate::gitstate;
use crate::planesave::{Mode, Plane};
use crate::repocmd::{Say, Sink};
use crate::worktree::git;

/// How an update ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Updated {
    /// Nothing was incoming on the branch named.
    Nothing(String),
    /// The project is at the remote's commits.
    Moved(Moved),
    /// The project is as it was, and why.
    Left(Left),
}

/// What an update brought in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Moved {
    /// The target branch.
    pub branch: String,
    /// Commits of the remote's that came in.
    pub incoming: u32,
    /// Commits of the project's own replayed on top of them; 0 for a fast-forward.
    pub replayed: u32,
    /// Whether it moved because its request merged ([`prsave::settle`]).
    pub by_request: bool,
}

/// Why an update left the project as it was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Left {
    /// A save or a fetch of the project is running.
    Busy,
    /// The project is on no branch.
    NoBranch,
    /// The fetch did not go through, in its own words: no origin on a forge purlis knows, a
    /// remote that is not the recorded one, or git's answer.
    NotFetched(String),
    /// A merge or rebase stopped part-way, or files git still calls unmerged.
    Stopped(gitstate::Stopped),
    /// Another git command holds the index.
    IndexLocked,
    /// The project is on another branch than the one it saves into.
    OffTarget { here: String, branch: String },
    /// Changed files nobody has saved: how many. Never moved, never stashed.
    Unsaved(usize),
    /// Files here that git ignores, where the incoming commits add a file of the same name.
    InTheWay(Vec<String>),
    /// Files the incoming commits change that purlis may not write. Nothing was started.
    Unwritable(Vec<String>),
    /// Files the incoming commits change that the asking chat's sandbox denies it (#3831): a
    /// project manifest, or a file a program run later loads code from. Nothing was started.
    HeldFromChats(Vec<String>),
    /// What the app will not do for a chat at the project root, in its own sentence (#3831): a
    /// move the chat's sandbox would not let it make, a move purlis could not judge, or a
    /// project whose update goes through its pull request. Nothing was started.
    NotForAChat(String),
    /// A request mode, and commits here that an open request carries: `mine` of them.
    WaitsOnRequest {
        mine: u32,
        /// The request, where it is known.
        url: Option<String>,
        /// What the look at the request said, where it said anything.
        why: Option<String>,
    },
    /// The project's own commits and the incoming ones conflict, in these files.
    Conflict(Vec<String>),
    /// A replayed commit could not be signed, in the signer's words.
    Unsigned(String),
    /// The replay did not finish within this many seconds.
    OutOfTime(u64),
    /// git did not finish, in its words. The project was put back.
    Failed(String),
    /// git did not finish, and the project could NOT be put back whole.
    NotPutBack { said: String, still: StillMoved },
}

/// What [`put_back`] could not restore.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StillMoved {
    /// Where HEAD is, when it is not where it was.
    pub head: Option<String>,
    /// The files that are not as they were.
    pub files: Vec<String>,
}

impl Left {
    /// One word for the reason, for a caller that offers a way on.
    pub fn word(&self) -> &'static str {
        match self {
            Self::Busy => "busy",
            Self::NoBranch => "no-branch",
            Self::NotFetched(_) => "not-fetched",
            Self::Stopped(_) => "stopped",
            Self::IndexLocked => "index-locked",
            Self::OffTarget { .. } => "off-target",
            Self::Unsaved(_) => "unsaved",
            Self::InTheWay(_) => "in-the-way",
            Self::Unwritable(_) => "unwritable",
            Self::HeldFromChats(_) => "held-from-chats",
            Self::NotForAChat(_) => "not-for-a-chat",
            Self::WaitsOnRequest { .. } => "waits-on-request",
            Self::Conflict(_) => "conflict",
            Self::Unsigned(_) => "unsigned",
            Self::OutOfTime(_) => "out-of-time",
            Self::Failed(_) => "failed",
            Self::NotPutBack { .. } => "not-put-back",
        }
    }

    /// The files the reason is about, where it names any.
    pub fn files(&self) -> &[String] {
        match self {
            Self::InTheWay(files)
            | Self::Unwritable(files)
            | Self::HeldFromChats(files)
            | Self::Conflict(files) => files,
            Self::NotPutBack { still, .. } => &still.files,
            Self::Stopped(stopped) => &stopped.unmerged,
            _ => &[],
        }
    }

    /// What stopped the move, as a clause: what [`Left::NotPutBack`] carries of it.
    fn stopped_by(&self) -> String {
        match self {
            Self::Failed(said) => said.clone(),
            Self::Conflict(files) => format!(
                "this project's own commits conflict with the incoming ones in {}",
                named(files)
            ),
            Self::Unsigned(signer) => format!("a replayed commit could not be signed: {signer}"),
            Self::OutOfTime(seconds) => {
                format!("the replay did not finish within {seconds} seconds")
            }
            other => other.word().replace('-', " "),
        }
    }

    /// Whether the project is exactly as it was. Everything but [`Left::NotPutBack`].
    pub fn as_it_was(&self) -> bool {
        !matches!(self, Self::NotPutBack { .. })
    }

    /// **Whether only the person can settle this reason**, where a chat asked for the update
    /// (#3877): a file the chat's sandbox denies, a move the app will not make for a chat, a
    /// conflict or a stopped merge, a project not put back whole, a branch to choose, a signer,
    /// or a file purlis may not write. Then the chat's ask raises a needs-you item that names
    /// the window's Update. A wait, a fetch that failed, the chat's own unsaved work, an ignored
    /// file in the way or a git failure the project was put back from are the chat's to settle
    /// or retry, and it is told them in the answer alone. Every reason is judged here by name,
    /// so a new one is judged when it is added.
    pub fn needs_the_person(&self) -> bool {
        match self {
            Self::HeldFromChats(_)
            | Self::NotForAChat(_)
            | Self::Conflict(_)
            | Self::Stopped(_)
            | Self::NotPutBack { .. }
            | Self::Unwritable(_)
            | Self::OffTarget { .. }
            | Self::NoBranch
            | Self::Unsigned(_) => true,
            Self::Busy
            | Self::IndexLocked
            | Self::NotFetched(_)
            | Self::Unsaved(_)
            | Self::InTheWay(_)
            | Self::WaitsOnRequest { .. }
            | Self::OutOfTime(_)
            | Self::Failed(_) => false,
        }
    }

    /// The reason, as sentences for the person. `request` is what the project's forge calls a
    /// request (`pull request`, `merge request`).
    pub fn said(&self, request: &str) -> String {
        const AS_IT_WAS: &str = "The project is exactly as it was.";
        match self {
            Self::Busy => {
                "A save of this project is running. Wait for it to finish, then update.".into()
            }
            Self::NoBranch => {
                "Nothing was updated: the project is on no branch, so there is nothing to \
                 update it from."
                    .into()
            }
            Self::NotFetched(why) => {
                format!("Nothing was updated: the remote could not be fetched: {why}.")
            }
            Self::Stopped(stopped) => format!("Nothing was updated: {}.", stopped.why()),
            Self::IndexLocked => "Nothing was updated: another git command holds the project's \
                 index. Update again once it has finished."
                .into(),
            Self::OffTarget { here, branch } => format!(
                "Nothing was updated: the project is on `{here}`, and it saves into `{branch}`."
            ),
            Self::Unsaved(n) => format!(
                "Save first, then update: {n} changed {} here {} not saved, and an update \
                 never moves unsaved work.",
                plural(*n, "file", "files"),
                plural(*n, "is", "are"),
            ),
            Self::InTheWay(files) => format!(
                "Nothing was updated: {} {} here, ignored by git, and the incoming commits add \
                 {} of the same name. Move {} away, then update.",
                named(files),
                plural(files.len(), "is", "are"),
                plural(files.len(), "a file", "files"),
                plural(files.len(), "it", "them"),
            ),
            Self::Unwritable(files) => format!(
                "Nothing was updated: the incoming commits change {}, which cannot be \
                 written here. Nothing was started. {AS_IT_WAS}",
                named(files),
            ),
            Self::HeldFromChats(files) => format!(
                "Nothing was updated: the incoming commits change {}, which a chat's sandbox \
                 does not let it write (a project manifest, or a file a program run later loads \
                 code from), so purlis does not update the project for a chat. The person \
                 updates it with Update in the window, or in their own terminal. {AS_IT_WAS}",
                named(files),
            ),
            Self::NotForAChat(why) => format!(
                "Nothing was updated: {} The person updates the project with Update in the \
                 window. {AS_IT_WAS}",
                why.trim_end()
            ),
            Self::WaitsOnRequest { mine, url, why } => {
                let on = match url {
                    Some(url) => format!("{request} {url}"),
                    None => format!("their {request}"),
                };
                let more = match why {
                    Some(why) => format!(" {}.", why.trim_end_matches('.')),
                    None => String::new(),
                };
                format!(
                    "Nothing was updated: {mine} {} here {} on {on}, and in this saving mode \
                     the project moves onto the remote's commits when it merges.{more}",
                    plural(*mine as usize, "commit", "commits"),
                    plural(*mine as usize, "waits", "wait"),
                )
            }
            Self::Conflict(files) => format!(
                "Nothing was updated: this project's own commits and the incoming ones change \
                 the same lines in {}. {AS_IT_WAS} {BY_HAND}",
                named(files)
            ),
            Self::Unsigned(signer) => format!(
                "Nothing was updated: this project's own commits could not be signed as they \
                 were replayed: {signer}. {AS_IT_WAS}"
            ),
            Self::OutOfTime(seconds) => format!(
                "Nothing was updated: replaying this project's own commits did not finish \
                 within {seconds} seconds, so purlis stopped it. {AS_IT_WAS}"
            ),
            Self::Failed(said) => {
                format!("Nothing was updated: git did not finish: {said}. {AS_IT_WAS}")
            }
            Self::NotPutBack { said, still } => format!(
                "The update stopped part-way: {said}. {} Do not save until that is settled.",
                capitalised(&said_not_put_back(still))
            ),
        }
    }
}

impl Updated {
    /// How it ended, as sentences for the person.
    pub fn said(&self, request: &str) -> String {
        match self {
            Self::Nothing(branch) => {
                format!(
                    "Nothing incoming: the project has everything `{branch}` has on the remote."
                )
            }
            Self::Moved(moved) => {
                let n = moved.incoming;
                let commits = plural(n as usize, "commit", "commits");
                if moved.by_request {
                    format!(
                        "Updated: its {request} merged, and the project is now at the remote's \
                         `{}`, {n} incoming {commits} on.",
                        moved.branch
                    )
                } else if moved.replayed > 0 {
                    format!(
                        "Updated: brought in {n} incoming {commits}, and replayed {} {} of \
                         this project's own on top.",
                        moved.replayed,
                        plural(moved.replayed as usize, "commit", "commits"),
                    )
                } else {
                    format!("Updated: brought in {n} incoming {commits}.")
                }
            }
            Self::Left(left) => left.said(request),
        }
    }
}

/// What [`put_back`] could not restore, as a clause.
pub(super) fn said_not_put_back(still: &StillMoved) -> String {
    let head = still
        .head
        .as_ref()
        .map(|head| format!("the project is at commit {head}, not the one it was on"));
    let files = (!still.files.is_empty()).then(|| {
        format!(
            "purlis could not put {} back as {} (`git restore --source=HEAD --staged \
             --worktree -- <files>` in a terminal does)",
            named(&still.files),
            plural(still.files.len(), "it was", "they were"),
        )
    });
    match (head, files) {
        (Some(head), Some(files)) => format!("{head}, and {files}"),
        (Some(one), None) | (None, Some(one)) => one,
        (None, None) => "purlis could not check that every file is as it was".into(),
    }
}

fn capitalised(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// How the person settles a conflict between the project's own commits and the remote's
/// (qw382): a chat may not rebase the project's root, so the commands are named, for a
/// terminal opened there. One sentence for the Update's answer and the Saving view's.
pub const BY_HAND: &str = "To settle it, open a terminal here and run `git fetch`, then `git \
     rebase @{u}`; edit each file git names, `git add` it and run `git rebase --continue`, then \
     Save. `git rebase --abort` puts the project back as it was.";

/// Up to five files in backticks, and how many more.
fn named(files: &[String]) -> String {
    let shown: Vec<String> = files.iter().take(5).map(|f| format!("`{f}`")).collect();
    let more = files.len().saturating_sub(shown.len());
    if more > 0 {
        format!("{} and {more} more", shown.join(", "))
    } else {
        shown.join(", ")
    }
}

/// git's last words, on one line.
fn one_line(said: &str) -> String {
    said.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Bring the incoming commits of the project at `root` in: fetch its target branch from the
/// remote recorded for it, then fast-forward, or replay the project's own commits on top.
/// `say` hears what it is doing as it goes.
///
/// For the person's own press in the window: it moves the working tree, and it holds the
/// project's [`Claim`], so no save runs beside it. A chat has it run by the app through
/// [`update_for_a_chat`], judged first.
pub fn update(root: &Path, say: Sink) -> Updated {
    update_as(root, None, say)
}

/// [`update`], run by the app for a chat at the project root (#3831, the operator's ruling of
/// 2026-10-29): only where the tree is clean, as for the window, and **judged before anything
/// moves**: where the incoming commits change a file a chat's sandbox denies it (ADR 0067 §5,
/// [`super::resolve::held_from_chats`]), nothing is started and the person updates instead. The
/// move then runs onto the commit id that was judged, never onto what the remote-tracking
/// branch names by the time git runs. `chat_dir` is where the asking chat was started: its
/// sandbox is the one the move is judged against.
pub fn update_for_a_chat(root: &Path, chat_dir: &Path, say: Sink) -> Updated {
    update_as(root, Some(chat_dir), say)
}

fn update_as(root: &Path, for_a_chat: Option<&Path>, say: Sink) -> Updated {
    let Some(_claim) = Claim::of(root) else {
        return Updated::Left(Left::Busy);
    };
    let plane = crate::planesave::Settings::read(root).plane;
    let here = git::run(root, &["rev-parse", "--abbrev-ref", "HEAD"], git::READ)
        .ok()
        .filter(git::Run::ok)
        .map(|r| r.line().trim().to_string())
        .unwrap_or_default();
    if here.is_empty() || here == "HEAD" {
        return Updated::Left(Left::NoBranch);
    }
    let branch = plane.branch.value.clone().unwrap_or_else(|| here.clone());
    // Asked before the network: a tree nobody may move is not fetched for.
    if let Some(left) = held(root, &here, &branch) {
        return Updated::Left(left);
    }
    if for_a_chat.is_some()
        && let Some(left) = request_mode_held(&plane)
    {
        return Updated::Left(left);
    }
    say(Say::Info(
        format!("Fetching `{branch}` from the remote …").into(),
    ));
    if let Err(why) = fetch_branch(root, &branch) {
        return Updated::Left(Left::NotFetched(why.trim_end_matches('.').to_string()));
    }
    if let Some(chat_dir) = for_a_chat {
        return bring_in_for_a_chat(root, &plane, &branch, chat_dir, say);
    }
    bring_in(
        root,
        &plane,
        &branch,
        &forge::Caller::window(),
        WRITE,
        true,
        say,
    )
}

/// Why nothing may move the tree at `root` now, asked of the tree as it is.
fn held(root: &Path, here: &str, branch: &str) -> Option<Left> {
    if let Some(stopped) = gitstate::stopped(root) {
        return Some(Left::Stopped(stopped));
    }
    if gitstate::find(&git_dir_of(root)).is_some() {
        return Some(Left::IndexLocked);
    }
    if here != branch {
        return Some(Left::OffTarget {
            here: here.to_string(),
            branch: branch.to_string(),
        });
    }
    let changed = changed_paths(root);
    if !changed.is_empty() {
        return Some(Left::Unsaved(changed.len()));
    }
    None
}

/// Whether commits of the project's own wait on a request rather than being replayed: a
/// request mode, a request this clone knows it opened, and commits the remote's branch lacks.
fn waits_on_its_request(mode: Option<Mode>, knows_a_request: bool, mine: u32) -> bool {
    mode.is_some_and(Mode::opens_a_pr) && knows_a_request && mine > 0
}

/// The half of [`update`] after the fetch: move the clean tree at `root`, on `branch`, onto
/// `refs/remotes/origin/<branch>` as it stands. `look` is whether the files are looked at
/// before anything starts ([`look`]); a test turns it off to drive a move that fails part-way.
///
/// A move that replayed the project's own commits leaves no push record behind (qw382): the
/// one there was is about a commit the replay rewrote, and the rebase it says stopped has gone
/// through ([`super::forget_replayed_push`]).
pub(super) fn bring_in(
    root: &Path,
    plane: &Plane,
    branch: &str,
    caller: &forge::Caller,
    deadline: Duration,
    look_first: bool,
    say: Sink,
) -> Updated {
    let theirs = format!("refs/remotes/origin/{branch}");
    bring_in_at(
        root, plane, branch, &theirs, None, caller, deadline, look_first, say,
    )
}

/// [`bring_in`] for a chat (#3831), after the fetch: the remote-tracking branch read once as
/// a commit id, the move from HEAD to it judged for the files a chat's sandbox denies, and the
/// move run onto that id.
fn bring_in_for_a_chat(
    root: &Path,
    plane: &Plane,
    branch: &str,
    chat_dir: &Path,
    say: Sink,
) -> Updated {
    // Asked again here, whatever called: no request's merge or fetch runs for a chat.
    if let Some(left) = request_mode_held(plane) {
        return Updated::Left(left);
    }
    let theirs = format!("refs/remotes/origin/{branch}");
    let (Some(head), Some(theirs)) = (head_of(root), commit_of(root, &theirs)) else {
        return Updated::Left(Left::Failed(format!(
            "purlis has no copy of the remote's `{branch}`"
        )));
    };
    if let Some(left) = held_for_a_chat(root, &head, &theirs, chat_dir) {
        return Updated::Left(left);
    }
    // No person waits on a chat's update: the forge is asked as a trigger would ask it.
    bring_in_at(
        root,
        plane,
        branch,
        &theirs,
        Some(&head),
        &forge::Caller::trigger(),
        WRITE,
        true,
        say,
    )
}

/// Why a chat's update may not move the project at `root` from `head` to `theirs`, both
/// commit ids, or `None`. **Fails closed**: where git cannot say what the move writes, it is
/// refused. Judged twice, and refused where either finds a file: against the asking chat's own
/// sandbox, as the clone guard judges a rebase for the chat started at `chat_dir`
/// ([`crate::cloneupdate::judged_rebase`]), and against the fixed floor every chat's sandbox
/// denies ([`super::resolve::held_from_chats`]).
fn held_for_a_chat(root: &Path, head: &str, theirs: &str, chat_dir: &Path) -> Option<Left> {
    let Some(changed) = differing_or_none(root, head, theirs) else {
        return Some(Left::NotForAChat(
            "git could not say which files the update would write, so purlis did not judge it \
             for this chat."
                .into(),
        ));
    };
    let changed: Vec<String> = changed.into_iter().map(|(_, path)| path).collect();
    let held = super::resolve::held_from_chats(root, &changed);
    if !held.is_empty() {
        return Some(Left::HeldFromChats(held));
    }
    let top = root.to_string_lossy();
    let place = crate::cloneupdate::Place {
        root: &top,
        chat_dir: Some(chat_dir),
        alone: true,
        sparse: true,
    };
    crate::cloneupdate::judged_rebase(&top, theirs, head, &place, &crate::cloneupdate::Asked)
        .map(Left::NotForAChat)
}

/// Why a chat may not have the project updated in its saving mode, or `None` (#3831): in a
/// mode that saves through a pull request, an update may merge or publish one, which reaches
/// the forge, and that is the person's to do from the window.
fn request_mode_held(plane: &Plane) -> Option<Left> {
    plane.mode.value.is_some_and(Mode::opens_a_pr).then(|| {
        Left::NotForAChat(
            "this project saves through a pull request, and bringing it up to date may merge \
             or publish one, which purlis does only for the person."
                .into(),
        )
    })
}

/// [`bring_in`], onto `theirs`: the remote-tracking branch, or the commit id a chat's update
/// judged, from `judged_head`, the commit it was judged from, where a chat's update judged one.
#[allow(clippy::too_many_arguments)]
fn bring_in_at(
    root: &Path,
    plane: &Plane,
    branch: &str,
    theirs: &str,
    judged_head: Option<&str>,
    caller: &forge::Caller,
    deadline: Duration,
    look_first: bool,
    say: Sink,
) -> Updated {
    let got = move_onto(
        root,
        plane,
        branch,
        theirs,
        judged_head,
        caller,
        deadline,
        look_first,
        say,
    );
    if matches!(&got, Updated::Moved(moved) if !moved.by_request) {
        super::forget_replayed_push(root);
    }
    got
}

/// [`bring_in`]'s move itself.
#[allow(clippy::too_many_arguments)]
fn move_onto(
    root: &Path,
    plane: &Plane,
    branch: &str,
    theirs: &str,
    judged_head: Option<&str>,
    caller: &forge::Caller,
    deadline: Duration,
    look_first: bool,
    say: Sink,
) -> Updated {
    let failed = |said: &str| Updated::Left(Left::Failed(said.to_string()));
    let Some(before) = head_of(root) else {
        return failed("the project has no commit to update from");
    };
    // A chat's update moves only from the commit it was judged from, and never through a
    // request.
    if let Some(judged) = judged_head {
        if before != judged {
            return failed(
                "the project's commit moved while purlis judged the update, so nothing was \
                 moved; update again",
            );
        }
        if let Some(left) = request_mode_held(plane) {
            return Updated::Left(left);
        }
    }
    let Some(behind) = count(root, &format!("HEAD..{theirs}")) else {
        return failed(&format!("purlis has no copy of the remote's `{branch}`"));
    };
    if behind == 0 {
        return Updated::Nothing(branch.to_string());
    }
    if look_first {
        let found = look(root, &before, theirs);
        if !found.in_the_way.is_empty() {
            return Updated::Left(Left::InTheWay(found.in_the_way));
        }
        if !found.unwritable.is_empty() {
            return Updated::Left(Left::Unwritable(found.unwritable));
        }
    }
    let moved = |replayed: u32, by_request: bool| {
        Updated::Moved(Moved {
            branch: branch.to_string(),
            incoming: behind,
            replayed,
            by_request,
        })
    };
    // Whatever stopped a move, the tree is put back, and the answer is the reason only where
    // that worked.
    let left = |reason: Left| match put_back(root, &before, theirs) {
        Ok(()) => Updated::Left(reason),
        Err(still) => Updated::Left(Left::NotPutBack {
            said: reason.stopped_by(),
            still,
        }),
    };

    // A request mode's own way first: a request that merged moves the project, squash or not.
    let mut why = None;
    if plane.mode.value.is_some_and(Mode::opens_a_pr) {
        match prsave::settle(root, plane, branch, plane.sign.value, None, caller, say) {
            prsave::Settled::Moved => return moved(0, true),
            prsave::Settled::Blocked(res) => why = Some(res.detail),
            prsave::Settled::Waiting(said) | prsave::Settled::Unknown(said) => why = Some(said),
            prsave::Settled::Open | prsave::Settled::Nothing => {}
        }
        // A move the settle tried and could not finish is undone like any other.
        if head_of(root).as_deref() != Some(before.as_str()) || !changed_paths(root).is_empty() {
            return left(Left::Failed(why.unwrap_or_else(|| {
                "the move onto the merged request did not finish".into()
            })));
        }
    }
    let mine = count(root, &format!("{theirs}..HEAD")).unwrap_or(0);
    if waits_on_its_request(plane.mode.value, prsave::knows_a_pr(root), mine) {
        return Updated::Left(Left::WaitsOnRequest {
            mine,
            url: prsave::known_pr_url(root),
            why,
        });
    }

    if mine == 0 {
        say(Say::Info(
            format!(
                "Bringing in {behind} incoming {} …",
                plural(behind as usize, "commit", "commits")
            )
            .into(),
        ));
        // Untimed: a merge checks out a tree, and a killed one leaves it half-written.
        return match git::run_untimed(
            root,
            &["merge", "--ff-only", "--no-overwrite-ignore", theirs],
        ) {
            Ok(run) if run.ok() => moved(0, false),
            Ok(run) => left(Left::Failed(one_line(&tail(&run)))),
            Err(unavailable) => left(Left::Failed(unavailable.to_string())),
        };
    }
    say(Say::Info(
        format!(
            "Replaying {mine} {} of this project's own onto {behind} incoming …",
            plural(mine as usize, "commit", "commits")
        )
        .into(),
    ));
    match rebase_onto(root, theirs, plane.sign.value, deadline) {
        Rebased::Replayed => moved(mine, false),
        Rebased::Conflict(files) => left(Left::Conflict(files)),
        Rebased::Unsigned(signer) => left(Left::Unsigned(signer)),
        Rebased::OutOfTime => left(Left::OutOfTime(deadline.as_secs())),
        Rebased::Failed(said) => left(Left::Failed(one_line(&said))),
        // Under a chat's sandbox, which the window never is: nothing was started.
        Rebased::Held(files) => Updated::Left(Left::Unwritable(files)),
    }
}

/// The commit `name` names in `root`, or `None`.
fn commit_of(root: &Path, name: &str) -> Option<String> {
    let spec = format!("{name}^{{commit}}");
    git::run(root, &["rev-parse", "--verify", "-q", &spec], git::READ)
        .ok()
        .filter(git::Run::ok)
        .map(|r| r.line().trim().to_string())
        .filter(|id| !id.is_empty())
}

fn head_of(root: &Path) -> Option<String> {
    git::run(root, &["rev-parse", "--verify", "-q", "HEAD"], git::READ)
        .ok()
        .filter(git::Run::ok)
        .map(|r| r.line().trim().to_string())
        .filter(|head| !head.is_empty())
}

/// [`differing_or_none`], read as nothing differing where git cannot answer: for the window,
/// whose move then fails on its own before a file is written.
fn differing(root: &Path, from: &str, to: &str) -> Vec<(bool, String)> {
    differing_or_none(root, from, to).unwrap_or_default()
}

/// The paths two commits' trees differ in, each with whether `to` adds it, or `None` where git
/// could not answer.
fn differing_or_none(root: &Path, from: &str, to: &str) -> Option<Vec<(bool, String)>> {
    let run = git::run(
        root,
        &[
            "diff-tree",
            "-r",
            "--no-renames",
            "--name-status",
            "-z",
            from,
            to,
        ],
        git::READ,
    )
    .ok()
    .filter(git::Run::ok)?;
    let mut out = Vec::new();
    let mut fields = run.out.split('\0');
    while let (Some(status), Some(path)) = (fields.next(), fields.next()) {
        if !path.is_empty() {
            out.push((status.starts_with('A'), path.to_string()));
        }
    }
    Some(out)
}

/// What [`look`] found that would stop a move part-way.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Found {
    pub in_the_way: Vec<String>,
    pub unwritable: Vec<String>,
}

/// **A move that cannot finish does not start.** git checks a tree out file by file and skips
/// what it cannot write, so the files a move from `before` to `to` would write are looked at
/// first: a file already there where the move adds one (the tree is clean, so git ignores it,
/// and a replay would write over it), and a file purlis may not replace.
pub(super) fn look(root: &Path, before: &str, to: &str) -> Found {
    let mut found = Found::default();
    let mut dirs: HashMap<PathBuf, bool> = HashMap::new();
    for (added, path) in differing(root, before, to) {
        let file = root.join(&path);
        if added && std::fs::symlink_metadata(&file).is_ok() {
            found.in_the_way.push(path);
        } else if !may_replace(root, &file, &mut dirs) {
            found.unwritable.push(path);
        }
    }
    found
}

/// Whether purlis may replace `file`, as git does it: unlink the old one and create the new, in
/// the nearest folder that is there. `dirs` remembers each folder's answer.
fn may_replace(root: &Path, file: &Path, dirs: &mut HashMap<PathBuf, bool>) -> bool {
    if std::fs::symlink_metadata(file).is_ok_and(|meta| locked(&meta)) {
        return false;
    }
    let mut dir = file.parent();
    while let Some(at) = dir {
        if let Ok(meta) = std::fs::symlink_metadata(at) {
            return *dirs
                .entry(at.to_path_buf())
                .or_insert_with(|| meta.is_dir() && !locked(&meta) && writable(at));
        }
        if at == root {
            break;
        }
        dir = at.parent();
    }
    true
}

/// Whether this process may make and remove entries in the folder `dir`.
#[cfg(unix)]
fn writable(dir: &Path) -> bool {
    rustix::fs::access(
        dir,
        rustix::fs::Access::WRITE_OK | rustix::fs::Access::EXEC_OK,
    )
    .is_ok()
}

#[cfg(not(unix))]
fn writable(dir: &Path) -> bool {
    std::fs::metadata(dir).is_ok_and(|meta| !meta.permissions().readonly())
}

/// Whether the file system holds the entry against every write: macOS's immutable flags.
#[cfg(target_os = "macos")]
fn locked(meta: &std::fs::Metadata) -> bool {
    use std::os::macos::fs::MetadataExt;
    const UF_IMMUTABLE: u32 = 0x0000_0002;
    const SF_IMMUTABLE: u32 = 0x0002_0000;
    meta.st_flags() & (UF_IMMUTABLE | SF_IMMUTABLE) != 0
}

#[cfg(not(target_os = "macos"))]
fn locked(_meta: &std::fs::Metadata) -> bool {
    false
}

/// How many paths one git call is handed.
const AT_ONCE: usize = 200;

/// **Put the tree at `root` back exactly as it was at `before`**, after a move towards
/// `towards` that did not finish. `Ok` only when it is: HEAD at `before`, and none of the paths
/// the move could have written differing from it.
///
/// - A rebase that started is aborted; one that never started is not asked to abort.
/// - What a failed checkout wrote is restored from `before`, path by path, over the paths the
///   two trees differ in and no others: a file somebody else changed meanwhile is not purlis's
///   to touch.
/// - A file the move added is removed only where its content is the one `towards` has, so it
///   is the checkout's own and nobody's work.
pub(super) fn put_back(root: &Path, before: &str, towards: &str) -> Result<(), StillMoved> {
    if rebase_in_progress(root) {
        let _ = git::run_untimed(root, &["rebase", "--abort"]);
    }
    let head = head_of(root);
    if head.as_deref() != Some(before) {
        return Err(StillMoved {
            head: Some(head.unwrap_or_else(|| "unknown".into())),
            files: changed_paths(root),
        });
    }
    let dirty: HashSet<String> = changed_paths(root).into_iter().collect();
    if dirty.is_empty() {
        return Ok(());
    }
    let touched = differing(root, before, towards);
    let ours = |added: bool| -> Vec<&str> {
        touched
            .iter()
            .filter(|(is_added, path)| *is_added == added && dirty.contains(path))
            .map(|(_, path)| path.as_str())
            .collect()
    };
    for paths in ours(false).chunks(AT_ONCE) {
        let mut args = vec![
            "--literal-pathspecs",
            "restore",
            "--source",
            before,
            "--staged",
            "--worktree",
            "--",
        ];
        args.extend_from_slice(paths);
        let _ = git::run_untimed(root, &args);
    }
    for paths in ours(true).chunks(AT_ONCE) {
        let mut args = vec![
            "--literal-pathspecs",
            "rm",
            "--cached",
            "-q",
            "--ignore-unmatch",
            "--",
        ];
        args.extend_from_slice(paths);
        let _ = git::run(root, &args, WRITE);
        let there: Vec<&str> = paths
            .iter()
            .copied()
            .filter(|path| {
                std::fs::symlink_metadata(root.join(path)).is_ok_and(|meta| meta.is_file())
            })
            .collect();
        let theirs = blobs_in(root, towards, &there);
        for (path, blob) in there.iter().zip(hashes_of(root, &there)) {
            let file = root.join(path);
            if theirs.get(*path) == Some(&blob) && std::fs::remove_file(&file).is_ok() {
                remove_empty_folders(root, &file);
            }
        }
    }
    let touched: HashSet<&str> = touched.iter().map(|(_, path)| path.as_str()).collect();
    let still: Vec<String> = changed_paths(root)
        .into_iter()
        .filter(|path| touched.contains(path.as_str()))
        .collect();
    if still.is_empty() {
        Ok(())
    } else {
        Err(StillMoved {
            head: None,
            files: still,
        })
    }
}

/// The blob `commit` has at each of `paths`.
fn blobs_in(root: &Path, commit: &str, paths: &[&str]) -> HashMap<String, String> {
    let mut args = vec!["--literal-pathspecs", "ls-tree", "-r", "-z", commit, "--"];
    args.extend_from_slice(paths);
    let Some(run) = git::run(root, &args, git::READ).ok().filter(git::Run::ok) else {
        return HashMap::new();
    };
    run.out
        .split('\0')
        .filter_map(|entry| {
            // `<mode> blob <object>\t<path>`
            let (meta, path) = entry.split_once('\t')?;
            let mut fields = meta.split(' ');
            let (_mode, kind, object) = (fields.next()?, fields.next()?, fields.next()?);
            (kind == "blob").then(|| (path.to_string(), object.to_string()))
        })
        .collect()
}

/// The blob each file of `paths` would be stored as, in their order; none where git could not
/// say for all of them, so nothing is then taken for the checkout's own.
fn hashes_of(root: &Path, paths: &[&str]) -> Vec<String> {
    if paths.is_empty() {
        return Vec::new();
    }
    let mut args = vec!["hash-object", "--"];
    args.extend_from_slice(paths);
    let hashes: Vec<String> = git::run(root, &args, git::READ)
        .ok()
        .filter(git::Run::ok)
        .map(|run| {
            run.out
                .lines()
                .map(|line| line.trim().to_string())
                .collect()
        })
        .unwrap_or_default();
    if hashes.len() == paths.len() {
        hashes
    } else {
        Vec::new()
    }
}

/// Remove the folders above `file` that its removal left empty, never `root` itself.
fn remove_empty_folders(root: &Path, file: &Path) {
    let mut dir = file.parent();
    while let Some(at) = dir {
        if at == root || !at.starts_with(root) || std::fs::remove_dir(at).is_err() {
            return;
        }
        dir = at.parent();
    }
}

#[cfg(test)]
mod tests;
