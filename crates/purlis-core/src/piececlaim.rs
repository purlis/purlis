// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A branch folder has one live writer** (#3901, from #3897 concern 2).
//!
//! After an app restart a coordinator took a live task for a dead one and dispatched a resume
//! chat; both rebased, committed and pushed in one branch folder before either noticed. So the
//! brokered writes of a branch folder (`purlis worktree commit`, `update` and `push`) are held
//! to **purlis's piece claim** ([`crate::pieces`]): the latest `claimed` line for the folder
//! names the chat that holds it, by its stable id ([`crate::pieces::Who::chat`]).
//!
//! - **A chat that writes in a folder no live chat holds takes it**: its claim is logged, and
//!   from then it is the holder. A folder cut for the person, or by the window, or claimed
//!   before chats were named in the log, is held by no chat.
//! - **A second chat's write in a folder a live chat holds is refused**, naming the holder and
//!   how to hand it over: the holder's `purlis worktree done` or `abandon`, or the person.
//! - **A removal is held to the claim too** (#3879): a folder a live chat holds is not removed
//!   for another, and a removal ends the folder's claim, whoever held it.
//! - **A claim whose chat has ended is released**: the chat is not open in this app, or its
//!   program has ended. So is one its holder, or the person, declared `done` or `abandoned`; a
//!   declaration by some other chat releases nothing.
//! - **Where the holder's liveness cannot be read, the write is refused** (fail closed): its
//!   chat has not come back since a restart (restoring, or waiting to start again), or no app
//!   answered for this chat at all.
//!
//! **Plain git is held to it too** (#3931). A chat started without the sandbox commits, rebases
//! and pushes with git itself, which purlis does not broker. So its Bash guard reads each line
//! for git that writes a branch folder's branch ([`crate::branchgit::branch_write`]) and asks
//! the app this same check before it runs ([`crate::hookwire::GitWork::WorktreeClaim`]): a
//! folder another live chat holds is refused in the words above, and a free one is taken.
//! **Where no app answers in time, the git runs**: unlike a brokered write, which fails closed
//! with no app, a guard on the chat's own git with no answer lets the call through (the review
//! of #4129). The person's own terminal runs no guard, and a shell tab is never held to a
//! claim.
//!
//! The claim and the check are made under one lock, an advisory lock on a file beside the
//! workspace's piece log taken by the app and by the command line alike, so of two chats
//! writing at once in a free folder one takes it and the other is refused, whichever process
//! each runs in. The lock covers the check and the line only, never the work: a removal is
//! judged and its free folder claimed under it, and removed outside it (#8024). The file names
//! its holder while it is held, so a taker that waited too long says who kept it, and a removal
//! that loses the race once tries once more. The log lives in the workspace, which chats
//! may write: this keeps two chats from writing one folder by accident, which is what happened,
//! and is not a wall against a chat set on going through it.

use std::path::{Path, PathBuf};
use std::time::Duration;

use jiff::Timestamp;
use serde_json::Value;

use crate::pieces::{self, Event, Who};
use crate::worktree::Located;

/// The trace reason the Bash guard tallies a refusal of plain git under: a chat's own `git
/// commit`, `push` or update in a branch folder another live chat holds (#3931), refused in the
/// words a brokered write is refused in.
pub const REASON: &str = "branch-folder-held";

/// A chat the app has, as a claim's holder is looked for among them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Live {
    /// Its stable id (ADR 0066).
    pub id: String,
    /// The app's number for it now: what the refusal names it by. None for a chat purlis is
    /// still bringing back that has no number yet.
    pub chat: Option<u32>,
    /// Whether its program runs here now. Not yet: it is restoring after a restart, or waiting
    /// to start again, and whether it still works cannot be told.
    pub back: bool,
}

/// What a chat would do in a branch folder, as a refusal names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Act {
    Commit,
    Update,
    Push,
    /// The folder's removal (#3879).
    Remove,
    /// Any of the three: what a dispatch into a held folder is warned of.
    Write,
}

impl Act {
    fn word(self) -> &'static str {
        match self {
            Self::Commit => "commit",
            Self::Update => "update",
            Self::Push => "push",
            Self::Remove => "remove",
            Self::Write => "commit, update or push",
        }
    }
}

/// Whether a chat may write in a branch folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Writer {
    /// No live chat holds it: the chat that writes takes it.
    Free,
    /// The chat that writes holds it.
    Yours,
    /// Another chat holds it, or may: the refusal, naming the holder and how to hand it over.
    Refused(String),
}

/// The chat a folder's claim names, where one does.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Holder {
    id: String,
    /// The number its claim was logged under, where it reads as one.
    number: Option<u32>,
    persona: Option<String>,
}

/// Who holds the branch folder `at` by its workspace's piece log, read in order:
///
/// - a `claimed` line with a `chat` makes that chat the holder;
/// - a `claimed` line with no `chat` (the window's, the person's, or one from before chats
///   were named) changes nothing: it holds nothing, and it lets no holder go;
/// - a `done` or `abandoned` line releases the holder only where it is the holder's own (its
///   `chat` is the holder's) or the person's (it has neither `chat` nor `session`). One from
///   any other chat, or with a `session` and no `chat`, releases nothing.
fn holder(plane: &Path, at: &Located) -> Option<Holder> {
    let text = |e: &Value, key: &str| {
        e.get(key)
            .and_then(Value::as_str)
            .filter(|v| !v.is_empty())
            .map(str::to_owned)
    };
    let mut held: Option<Holder> = None;
    let gone = crate::worktree::path_for(plane, &at.workspace, &at.repo, &at.piece)
        .is_ok_and(|folder| folder.symlink_metadata().is_err());
    for e in pieces::events(plane, &at.workspace) {
        if text(&e, "piece").as_deref() != Some(at.piece.as_str())
            || text(&e, "repo").unwrap_or_default() != at.repo
        {
            continue;
        }
        let chat = text(&e, "chat");
        match text(&e, "event").as_deref() {
            Some("claimed") => {
                if let Some(id) = chat {
                    held = Some(Holder {
                        id,
                        number: text(&e, "session").and_then(|n| n.parse().ok()),
                        persona: text(&e, "persona"),
                    });
                }
            }
            // The folder is gone, and its claim with it (#3879): only while it is gone, so a
            // line written into the log by hand lets no live folder's holder go.
            Some("removed") if gone => held = None,
            Some("done" | "abandoned") => {
                let the_persons = chat.is_none() && text(&e, "session").is_none();
                let the_holders =
                    chat.is_some() && chat.as_deref() == held.as_ref().map(|h| h.id.as_str());
                if the_persons || the_holders {
                    held = None;
                }
            }
            _ => {}
        }
    }
    held
}

/// Whether the chat `me` (its stable id, where it has one) may `act` in the branch folder `at`,
/// by the piece log and `live`: the chats the app has, or `None` where no app said.
pub fn writer(
    plane: &Path,
    at: &Located,
    me: Option<&str>,
    live: Option<&[Live]>,
    act: Act,
) -> Writer {
    let Some(holder) = holder(plane, at) else {
        return Writer::Free;
    };
    if me == Some(holder.id.as_str()) {
        return Writer::Yours;
    }
    let act = act.word();
    let found = live.map(|live| live.iter().find(|one| one.id == holder.id));
    let named = |chat: String| match &holder.persona {
        Some(persona) => format!("{chat} ({})", crate::personas::one_line(persona)),
        None => chat,
    };
    let by_number = |number: Option<u32>| match number {
        Some(number) => format!("chat {number}"),
        None => "another chat".to_owned(),
    };
    let folder = format!("{}'s branch folder '{}'", at.repo, at.piece);
    let hand_over = |who: &str| {
        format!(
            "To hand it over, {who} runs `purlis worktree done`, or `purlis worktree abandon \
             \"<reason>\"`, in it; or the person closes {who}, or runs one of those in the \
             folder from their own terminal. Until then, work in a branch folder of your own \
             (`purlis worktree add`). Nothing was done"
        )
    };
    match found {
        // Not open here: it has ended, and its claim with it.
        Some(None) => Writer::Free,
        Some(Some(one)) if one.back => {
            let who = named(by_number(one.chat.or(holder.number)));
            Writer::Refused(format!(
                "{folder} is claimed by {who}, which is still at work there, so purlis will not \
                 {act} in it for a second chat: two chats writing one folder overwrite each \
                 other's work. {}",
                hand_over(&who)
            ))
        }
        Some(Some(one)) => {
            // A chat still coming back has no number of its own yet: its old one may be dealt.
            let who = named(match one.chat {
                Some(number) => format!("chat {number}"),
                None => "a chat purlis is still bringing back".to_owned(),
            });
            Writer::Refused(format!(
                "{folder} is claimed by {who}, which has not come back since purlis restarted, \
                 so whether it still works there cannot be told, and purlis will not {act} in \
                 it for a second chat. {}",
                hand_over(&who)
            ))
        }
        None => {
            let who = named(by_number(holder.number));
            Writer::Refused(format!(
                "{folder} is claimed by {who}, and whether that chat still works there cannot \
                 be told without the purlis app, which did not answer, so purlis will not {act} \
                 in it for a second chat. {}",
                hand_over(&who)
            ))
        }
    }
}

/// **Why an open chat keeps the branch folder `at` from a prune** (ADR 0110), or `None`: its
/// claim names a chat that has declared neither `done` nor `abandoned` and is open among
/// `live`, the asking chat (`me`) included; or `live` is `None`, no app having said which chats
/// are open, and a held folder is kept (fail closed). A chat not open here holds nothing.
pub fn held_open(
    plane: &Path,
    at: &Located,
    me: Option<&str>,
    live: Option<&[Live]>,
) -> Option<String> {
    let holder = holder(plane, at)?;
    let chat = |number: Option<u32>| match number {
        Some(number) => format!("chat {number}"),
        None => "a chat".to_owned(),
    };
    if me == Some(holder.id.as_str()) {
        return Some(
            "this chat holds it and has declared neither done nor abandoned: run `purlis \
             worktree done` in it first"
                .to_owned(),
        );
    }
    match live {
        None => Some(format!(
            "{} holds it, and whether that chat is still open cannot be told without the purlis \
             app: run this from a chat, or `purlis worktree done` in the folder",
            chat(holder.number)
        )),
        Some(live) => live.iter().find(|one| one.id == holder.id).map(|one| {
            format!(
                "{} holds it and has declared neither done nor abandoned",
                chat(one.chat.or(holder.number))
            )
        }),
    }
}

/// The file whose advisory lock every check-and-claim in a workspace holds: beside its piece
/// log, so the app and a command line in a chat take the same one.
fn lock_path(plane: &Path, ws: &str) -> PathBuf {
    pieces::dir_for(plane, ws).join(".claims.lock")
}

/// How long a check-and-claim waits on another's: a check and one appended line.
const A_CLAIM_WAITS_AT_MOST: Duration = Duration::from_secs(5);

/// Workspace `ws`'s claim lock, held. The line naming its holder is emptied before the lock is
/// let go of (the field is dropped after this guard's own `drop`).
#[derive(Debug)]
pub(crate) struct Claims(crate::filelock::Held);

impl Drop for Claims {
    fn drop(&mut self) {
        // Best-effort: a line left behind names a holder that has gone, until the next taker
        // writes its own over it.
        let _ = self.0.set_len(0);
    }
}

/// Takes workspace `ws`'s claim lock, or says why not. The file is opened through no link.
pub(crate) fn locked(plane: &Path, ws: &str) -> Result<Claims, String> {
    hold(plane, ws, None, A_CLAIM_WAITS_AT_MOST).map_err(not_held)
}

/// The sentence a lock that could not be held is refused in.
fn not_held(why: std::io::Error) -> String {
    format!("purlis could not hold the branch folder claims ({why})")
}

/// Takes workspace `ws`'s claim lock for `by` (where a chat asks), waiting `wait` at most, and
/// writes in it who holds it (#8024); a wait that ends names who held it, and since when.
fn hold(plane: &Path, ws: &str, by: Option<&Who>, wait: Duration) -> std::io::Result<Claims> {
    use std::io::{Read, Seek, SeekFrom, Write};
    let path = lock_path(plane, ws);
    crate::contain::writable(plane, &path).map_err(std::io::Error::other)?;
    std::fs::create_dir_all(pieces::dir_for(plane, ws))?;
    let mut open = std::fs::OpenOptions::new();
    open.create(true).truncate(false).read(true).write(true);
    // Never waits to open: a FIFO a chat planted at the path opens at once, rather than
    // holding the broker until a reader comes, and is refused below. `flock` takes no
    // notice of the flag, so it stays.
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        open.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK);
    }
    let file = open.open(&path)?;
    if !file.metadata()?.file_type().is_file() {
        return Err(std::io::Error::other(
            "what is at its lock's place is not a plain file",
        ));
    }
    let reader = file.try_clone()?;
    match crate::filelock::lock_within(file, wait, "the branch folder claims") {
        Ok(held) => {
            let mut line = serde_json::json!({
                "pid": std::process::id(),
                "since": Timestamp::now().to_string(),
            });
            if let Some(chat) = by.and_then(|by| by.session.as_deref()?.parse::<u32>().ok()) {
                line["chat"] = chat.into();
            }
            // Best-effort: who holds it is only ever said, never judged by.
            let mut on: &std::fs::File = &held;
            let _ = on
                .set_len(0)
                .and_then(|()| on.seek(SeekFrom::Start(0)))
                .and_then(|_| on.write_all(line.to_string().as_bytes()));
            Ok(Claims(held))
        }
        Err(why) if why.kind() == std::io::ErrorKind::TimedOut => {
            let mut said = Vec::new();
            let mut from: &std::fs::File = &reader;
            let read = from
                .seek(SeekFrom::Start(0))
                .and_then(|_| from.take(512).read_to_end(&mut said));
            let holder = read
                .ok()
                .and_then(|_| holder_said(&said, Timestamp::now()))
                .map(|holder| format!("; {holder}"))
                .unwrap_or_default();
            Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                format!("{why}{holder}"),
            ))
        }
        Err(why) => Err(why),
    }
}

/// Who the line in the claim lock says holds it, and for how long by `now`, or `None` where it
/// names nobody. The file is in the workspace, which chats may write, so only what reads as a
/// number or a timestamp is said: no text from it reaches the person.
fn holder_said(line: &[u8], now: Timestamp) -> Option<String> {
    let line: Value = serde_json::from_slice(line).ok()?;
    let number = |key: &str| {
        line.get(key)
            .and_then(Value::as_u64)
            .and_then(|n| u32::try_from(n).ok())
    };
    let who = match (number("chat"), number("pid")) {
        (Some(chat), Some(pid)) => format!("chat {chat} (process {pid})"),
        (Some(chat), None) => format!("chat {chat}"),
        (None, Some(pid)) => format!("process {pid}"),
        (None, None) => return None,
    };
    let held = line
        .get("since")
        .and_then(Value::as_str)
        .and_then(|since| since.parse::<Timestamp>().ok())
        .and_then(|since| now.duration_since(since).try_into().ok())
        .map(|held: Duration| {
            if held < Duration::from_secs(1) {
                format!(", which has held them for {} ms", held.as_millis())
            } else {
                format!(", which has held them for {} s", held.as_secs())
            }
        })
        .unwrap_or_default();
    Some(format!("they are held by {who}{held}"))
}

/// **The check every brokered write of a branch folder makes first** (#3901): `Ok` where `who`
/// may `act` in the branch folder `at`, having taken it where no live chat held it; else the
/// refusal. A chat with no stable id cannot be named as a holder, so it takes nothing, and
/// writes only where no live chat holds the folder. A claim that cannot be written, or a lock
/// that cannot be held, is a refusal: a write nobody holds the folder for is what this guards
/// against.
pub fn take(
    plane: &Path,
    at: &Located,
    who: &Who,
    live: Option<&[Live]>,
    act: Act,
    now: Timestamp,
) -> Result<(), String> {
    take_telling(plane, at, who, live, act, now).map_err(Refusal::why)
}

/// Why [`take_telling`] refused: the one case a guard on a chat's own git refuses for
/// (#3931, the review of #4129), apart from every other.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// Another chat holds the folder, and its program runs here now: the sentence names it.
    HeldByALiveChat(String),
    /// Anything else: a holder that has not come back since a restart, no app to say who
    /// lives, or a lock or a claim that could not be held or written.
    Otherwise(String),
}

impl Refusal {
    /// The sentence, whichever it is.
    pub fn why(self) -> String {
        match self {
            Self::HeldByALiveChat(why) | Self::Otherwise(why) => why,
        }
    }
}

/// [`take`], saying whether a refusal is because another live chat holds the folder.
pub fn take_telling(
    plane: &Path,
    at: &Located,
    who: &Who,
    live: Option<&[Live]>,
    act: Act,
    now: Timestamp,
) -> Result<(), Refusal> {
    let refused = |why: String| {
        Refusal::Otherwise(format!(
            "{why}, so a second chat could not be kept out of {}'s branch folder '{}', and \
             purlis will not {} there. Nothing was done",
            at.repo,
            at.piece,
            act.word()
        ))
    };
    let _one_at_a_time = hold(plane, &at.workspace, Some(who), A_CLAIM_WAITS_AT_MOST)
        .map_err(|why| refused(not_held(why)))?;
    match writer(plane, at, who.chat.as_deref(), live, act) {
        Writer::Yours => Ok(()),
        Writer::Refused(why) => {
            // Under the lock, so the holder read is the one `writer` judged.
            let by_a_live_chat = holder(plane, at).is_some_and(|holder| {
                live.into_iter()
                    .flatten()
                    .any(|one| one.id == holder.id && one.back)
            });
            Err(if by_a_live_chat {
                Refusal::HeldByALiveChat(why)
            } else {
                Refusal::Otherwise(why)
            })
        }
        Writer::Free if who.chat.is_none() => Ok(()),
        Writer::Free => pieces::record(
            plane,
            &at.workspace,
            Event::Claimed,
            &at.repo,
            &at.piece,
            None,
            who,
            now,
        )
        .map(|_| ())
        .ok_or_else(|| refused("purlis could not record that this chat works there".to_owned())),
    }
}

/// Whose word a removal is held to (#3879).
#[derive(Debug, Clone, Copy)]
pub enum Judged<'a> {
    /// The person's own terminal, which hands any folder over: nothing is checked.
    ByThePerson,
    /// A chat: refused where another live chat holds the folder, by `live` (the chats the app
    /// has, or `None` where no app said, which refuses a held folder).
    ByAChat(Option<&'a [Live]>),
}

/// **A branch folder's removal, held to its claim** (#3879): `remove` is run only where
/// `judged` lets `who` have the folder, judged under the workspace's claim lock and, for a chat
/// in a folder no live chat holds, claimed for it there; `remove` itself runs outside the lock
/// (#8024). A removal that answers `0` ends the folder's claim, whoever held it, by a `removed`
/// line. Else the refusal, in the words a commit, an update or a push is refused in.
pub fn removing(
    plane: &Path,
    at: &Located,
    who: &Who,
    judged: Judged<'_>,
    now: Timestamp,
    remove: impl FnOnce() -> u8,
) -> Result<u8, String> {
    removing_within(plane, at, who, judged, now, A_CLAIM_WAITS_AT_MOST, remove)
}

/// [`removing`], each take of the claim lock waiting `wait` at most.
fn removing_within(
    plane: &Path,
    at: &Located,
    who: &Who,
    judged: Judged<'_>,
    now: Timestamp,
    wait: Duration,
    remove: impl FnOnce() -> u8,
) -> Result<u8, String> {
    let refused = |why: String| {
        format!(
            "{why}, so a second chat could not be kept out of {}'s branch folder '{}', and \
             purlis will not remove it. Nothing was removed",
            at.repo, at.piece
        )
    };
    // A take that loses the race once tries once more before refusing (#8024).
    let take_the_lock = || {
        hold(plane, &at.workspace, Some(who), wait)
            .or_else(|why| match why.kind() {
                std::io::ErrorKind::TimedOut => hold(plane, &at.workspace, Some(who), wait),
                _ => Err(why),
            })
            .map_err(not_held)
    };
    // Under the lock only the judging and the claim (#8024): the removal itself, with its git
    // and its forge's answer, runs outside it. A chat that removes a folder no live chat holds
    // takes it first, as any write does, so no second chat writes there while it goes.
    {
        let _one_at_a_time = take_the_lock().map_err(refused)?;
        if let Judged::ByAChat(live) = judged {
            match writer(plane, at, who.chat.as_deref(), live, Act::Remove) {
                Writer::Refused(why) => return Err(why),
                Writer::Free if who.chat.is_some() => {
                    pieces::record(
                        plane,
                        &at.workspace,
                        Event::Claimed,
                        &at.repo,
                        &at.piece,
                        None,
                        who,
                        now,
                    )
                    .ok_or_else(|| {
                        refused("purlis could not record that this chat works there".to_owned())
                    })?;
                }
                Writer::Free | Writer::Yours => {}
            }
        }
    }
    let code = remove();
    if code == 0 {
        // Best-effort: the folder is gone either way, and a claim on a folder that is not
        // there holds nothing a later cut of the same name could not take back. Under the lock
        // where it can be had; a `removed` line only ever lets a holder go while the folder is
        // gone, so one appended amid another's check makes that check no less careful.
        let _one_at_a_time = take_the_lock().ok();
        let _ = pieces::record(
            plane,
            &at.workspace,
            Event::Removed,
            &at.repo,
            &at.piece,
            None,
            who,
            now,
        );
    }
    Ok(code)
}

#[cfg(test)]
mod tests;
