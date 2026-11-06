// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Each turn of a chat, as two git refs** (N3, ADR 0084 §1's comparison (d), #5405):
//! `refs/purlis/turns/<chat>/<n>/start` and `…/end`, the branch's state before and after the
//! turn. [`super::Comparison::Turn`] compares them with the one diff engine, and
//! `docs/plane-format.md` names the store and its tier (Clone state).
//!
//! **A state is a commit of the folder as it is**: staged in an index of its own, a copy of the
//! folder's so that a file git has already seen is not read again, so uncommitted work and
//! untracked files git does not ignore are in it, what git ignores is not, and the folder's own
//! index, branch and files do not move. A commit is minted even where nothing is uncommitted,
//! because its message says which branch the state is of and its date which turn is the latest
//! ([`ended_on`]). **Nothing is left out of it**, unlike a code link's snapshot, which leaves
//! the machine: these refs do not, and a turn's undo has to put back every file the turn changed.
//!
//! **Nothing the repo names runs**: every git call goes through `worktree::git`'s hardened
//! runner, which turns off hooks, the fsmonitor and each program the repository's config names.
//! Staging also holds every filter any other level of config names ([`hold_filters`]): the
//! folder's `.gitattributes` is an agent's to write, so it could otherwise ask for one. That is
//! done without `--attr-source`, which git only has from 2.41, because a state must record on the
//! git macOS ships. The writes are the app's: a chat's sandbox cannot write a folder's git data.

use std::path::Path;

use super::{Branch, Refused};
use crate::worktree::git;

/// Where every turn's refs live.
const NAMESPACE: &str = "refs/purlis/turns/";

/// The line of a state's message that says which branch it is of: what [`ended_on`] reads.
const BRANCH_LINE: &str = "Branch: ";

/// The longest chat id a ref is made from.
const LONGEST_CHAT: usize = 64;

/// A boundary of a turn.
#[derive(Clone, Copy)]
enum Side {
    Start,
    End,
    /// The mark an undone turn has (**Undo a turn**, #5411), at the turn's end.
    Undone,
}

impl Side {
    const fn word(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::End => "end",
            Self::Undone => "undone",
        }
    }
}

/// Whether `chat` can name a folder of refs: letters, digits, `_` and `-`, starting with a letter
/// or a digit, which is what a chat's id is. Nothing else reaches a ref's name.
fn chat_ok(chat: &str) -> bool {
    chat.len() <= LONGEST_CHAT
        && chat
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric())
        && chat
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn checked(chat: &str) -> Result<(), Refused> {
    if chat_ok(chat) {
        Ok(())
    } else {
        Err(Refused::Read(format!(
            "'{chat}' is not a chat id a turn can be recorded under"
        )))
    }
}

fn reference(chat: &str, number: u32, side: Side) -> String {
    format!("{NAMESPACE}{chat}/{number}/{}", side.word())
}

/// The ref of turn `number`'s `start` or `end`, in the bounded reader's child.
pub(super) fn sides_of(chat: &str, number: u32) -> Option<(String, String)> {
    chat_ok(chat).then(|| {
        (
            reference(chat, number, Side::Start),
            reference(chat, number, Side::End),
        )
    })
}

/// Where a chat's folder is, by the names a [`Branch`] is made of.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Place {
    pub ws: String,
    pub repo: String,
    pub piece: Option<String>,
}

/// **The repo and the branch folder `cwd` is in**, read off the path alone, because a chat is
/// removed after its folder may be: `workspaces/<ws>/<repo>/…` or
/// `workspaces/<ws>/.worktrees/<repo>/<piece>/…`, below `plane`. `None` for anywhere else: the
/// project's root, or a workspace's folder above its repos.
pub fn place_of(plane: &Path, cwd: &Path) -> Option<Place> {
    let resolved = |path: &Path| std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let (plane, cwd) = (resolved(plane), resolved(cwd));
    let below = cwd.strip_prefix(&plane).ok()?;
    let mut parts = below.iter().map(|part| part.to_str());
    if parts.next()?? != "workspaces" {
        return None;
    }
    let ws = parts.next()??.to_owned();
    let first = parts.next()??;
    if first == crate::worktree::DIR_NAME {
        let repo = parts.next()??.to_owned();
        let piece = parts.next()??.to_owned();
        return Some(Place {
            ws,
            repo,
            piece: Some(piece),
        });
    }
    // A workspace's own folders hold no branch: its memory, its todos, its dotted state.
    if first.starts_with('.') {
        return None;
    }
    Some(Place {
        ws,
        repo: first.to_owned(),
        piece: None,
    })
}

/// **A prompt started a turn of `chat` in `branch`**: its state now is the turn's start. Answers
/// the turn's number, or `None` where the chat's last turn has no end yet: a prompt sent while a
/// turn runs is that turn, as a run stays `working` through it.
pub fn began(plane: &Path, branch: Branch<'_>, chat: &str) -> Result<Option<u32>, Refused> {
    checked(chat)?;
    let folder = super::folder_of(plane, branch)?;
    let last = last_turn(&folder, chat)?;
    if let Some(Last { ended: false, .. }) = last {
        return Ok(None);
    }
    let number = last.map_or(1, |last| last.number + 1);
    record(&folder, chat, number, Side::Start)?;
    Ok(Some(number))
}

/// **A turn of `chat` in `branch` ended**, by its harness saying so or its program ending: its
/// state now is the turn's end. Answers the turn's number, or `None` where no turn is open: an end
/// heard twice, or one with no prompt before it.
pub fn ended(plane: &Path, branch: Branch<'_>, chat: &str) -> Result<Option<u32>, Refused> {
    checked(chat)?;
    let folder = super::folder_of(plane, branch)?;
    let Some(Last {
        number,
        ended: false,
    }) = last_turn(&folder, chat)?
    else {
        return Ok(None);
    };
    record(&folder, chat, number, Side::End)?;
    Ok(Some(number))
}

/// **Chat `chat` is removed**: every ref of its turns in the clone of `repo` goes. A chat with
/// none, and a clone that holds none, is no refusal.
pub fn forget(plane: &Path, ws: &str, repo: &str, chat: &str) -> Result<(), Refused> {
    checked(chat)?;
    let clone = crate::worktree::clone_dir(plane, ws, repo)?;
    let listed = refs_of(&clone, chat)?;
    let mut input = String::from("start\n");
    let mut any = false;
    for name in listed.lines() {
        input.push_str(&format!("delete {name}\n"));
        any = true;
    }
    if !any {
        return Ok(());
    }
    input.push_str("prepare\ncommit\n");
    let deleted = git::run_with_input(
        &clone,
        &["update-ref", "--stdin"],
        input.into_bytes(),
        git::READ,
    )
    .map_err(crate::worktree::Refusal::from)?;
    if deleted.code != Some(0) {
        return Err(Refused::Read(format!(
            "purlis could not remove the chat's turns: {}",
            String::from_utf8_lossy(&deleted.err).trim()
        )));
    }
    Ok(())
}

/// **Turn `number` of `chat` in `branch` is undone** (#5411): its `undone` ref is written at its
/// end, so the turn offered next is the one before it ([`Ended::offered`]). The app writes it in the
/// step that put the turn's files back, through the git runner.
pub fn undone(plane: &Path, branch: Branch<'_>, chat: &str, number: u32) -> Result<(), Refused> {
    checked(chat)?;
    let folder = super::folder_of(plane, branch)?;
    let end = reference(chat, number, Side::End);
    let name = reference(chat, number, Side::Undone);
    // The empty old value: written only where no mark is yet.
    let written = ran(&folder, &["update-ref", &name, &end, ""])?;
    if !written.ok() {
        return Err(failed("mark the turn undone", &written));
    }
    Ok(())
}

/// The last turn a chat has here.
struct Last {
    number: u32,
    ended: bool,
}

fn last_turn(folder: &Path, chat: &str) -> Result<Option<Last>, Refused> {
    let listed = refs_of(folder, chat)?;
    let mut last: Option<Last> = None;
    for name in listed.lines() {
        let Some((number, side)) = name
            .strip_prefix(NAMESPACE)
            .and_then(|rest| rest.strip_prefix(chat))
            .and_then(|rest| rest.strip_prefix('/'))
            .and_then(|rest| rest.split_once('/'))
        else {
            continue;
        };
        let Ok(number) = number.parse::<u32>() else {
            continue;
        };
        let ended = side == Side::End.word();
        match &mut last {
            Some(seen) if seen.number == number => seen.ended |= ended,
            Some(seen) if seen.number > number => {}
            _ => last = Some(Last { number, ended }),
        }
    }
    Ok(last)
}

/// Writes the state of `folder` now as `side` of turn `number`.
fn record(folder: &Path, chat: &str, number: u32, side: Side) -> Result<(), Refused> {
    let tree = staged_tree(folder)?;
    let head = ran(folder, &["symbolic-ref", "--short", "-q", "HEAD"])?;
    let message = format!(
        "purlis: turn {number} {} of chat {chat}\n\n{BRANCH_LINE}{}\n",
        side.word(),
        if head.ok() { head.line() } else { "" },
    );
    let commit = ran(
        folder,
        &[
            "-c",
            "user.name=purlis",
            "-c",
            "user.email=purlis@localhost",
            "-c",
            "commit.gpgSign=false",
            "commit-tree",
            "-p",
            "HEAD",
            "-m",
            &message,
            &tree,
        ],
    )?;
    if !commit.ok() {
        return Err(failed("record the turn's state", &commit));
    }
    let name = reference(chat, number, side);
    let written = ran(folder, &["update-ref", &name, commit.line()])?;
    if !written.ok() {
        return Err(failed("record the turn's state", &written));
    }
    Ok(())
}

/// **The tree the folder is now**, staged in an index of its own.
fn staged_tree(folder: &Path) -> Result<String, Refused> {
    let scratch = tempfile::tempdir()
        .map_err(|why| Refused::Read(format!("purlis has no room to record a turn: {why}")))?;
    let index = scratch.path().join("turn.index");
    let own = ran(
        folder,
        &["rev-parse", "--path-format=absolute", "--git-path", "index"],
    )?;
    let copied = own.ok() && std::fs::copy(own.line(), &index).is_ok();
    if !copied {
        let read = with_index(folder, &index, &["read-tree", "HEAD"])?;
        if !read.ok() {
            return Err(failed("read the branch's last commit", &read));
        }
    }
    let named = ran(
        folder,
        &["config", "--name-only", "--get-regexp", r"^filter\."],
    )?;
    let held = hold_filters(&named.out);
    let mut args: Vec<&str> = held.iter().map(String::as_str).collect();
    args.extend([
        "-c",
        "core.attributesFile=/dev/null",
        "add",
        "--all",
        "--",
        ".",
    ]);
    let added = with_index(folder, &index, &args)?;
    if !added.ok() {
        return Err(failed("stage the folder's work", &added));
    }
    let written = with_index(folder, &index, &["write-tree"])?;
    if !written.ok() {
        return Err(failed("write the folder's work", &written));
    }
    Ok(written.line().to_owned())
}

/// `-c` arguments that turn off each filter `listed` names (`git config --name-only`, one key a
/// line), so that a `.gitattributes` asking for one runs nothing and stores the file's own bytes.
fn hold_filters(listed: &str) -> Vec<String> {
    let mut names: Vec<&str> = listed
        .lines()
        .filter_map(|key| key.strip_prefix("filter.")?.rsplit_once('.'))
        .map(|(name, _)| name)
        .collect();
    names.sort_unstable();
    names.dedup();
    names
        .into_iter()
        .flat_map(|name| {
            [
                format!("filter.{name}.clean="),
                format!("filter.{name}.process="),
                format!("filter.{name}.required=false"),
            ]
        })
        .flat_map(|setting| ["-c".to_owned(), setting])
        .collect()
}

fn with_index(folder: &Path, index: &Path, args: &[&str]) -> Result<git::Run, Refused> {
    git::run_with_index(folder, index, args)
        .map_err(crate::worktree::Refusal::from)
        .map_err(Refused::from)
}

/// git in `folder`, through the hardened runner.
fn ran(folder: &Path, args: &[&str]) -> Result<git::Run, Refused> {
    git::run(folder, args, git::READ)
        .map_err(crate::worktree::Refusal::from)
        .map_err(Refused::from)
}

/// The names of the refs of `chat`'s turns in the clone `folder` belongs to.
fn refs_of(folder: &Path, chat: &str) -> Result<String, Refused> {
    let listed = ran(
        folder,
        &[
            "for-each-ref",
            "--format=%(refname)",
            &format!("{NAMESPACE}{chat}/"),
        ],
    )?;
    if !listed.ok() {
        return Err(failed("list the turns of the chat", &listed));
    }
    Ok(listed.out)
}

fn failed(doing: &str, ran: &git::Run) -> Refused {
    Refused::Read(format!(
        "purlis could not {doing}: {}",
        ran.err.trim().lines().last().unwrap_or_default()
    ))
}

/// **Each turn that ended on `branch`**, whichever chat's it was, newest first: an end ref whose
/// state's message names the branch, ordered by its end's date, ties going to the higher number.
/// Read by gitoxide in the bounded reader's child; a ref that is not one purlis wrote is passed
/// over. One walk of the refs answers every question the file diff asks of them ([`Ended`]).
pub(super) fn ended_on(repo: &gix::Repository, branch: &str) -> Ended {
    let Ok(platform) = repo.references() else {
        return Ended(Vec::new());
    };
    let Ok(prefixed) = platform.prefixed(NAMESPACE) else {
        return Ended(Vec::new());
    };
    let mut ended = Vec::new();
    for mut found in prefixed.flatten() {
        let name = found.name().as_bstr().to_string();
        let Some((chat, rest)) = name
            .strip_prefix(NAMESPACE)
            .and_then(|rest| rest.split_once('/'))
        else {
            continue;
        };
        let Some(number) = rest
            .strip_suffix("/end")
            .and_then(|number| number.parse::<u32>().ok())
        else {
            continue;
        };
        let Ok(commit) = found.peel_to_commit() else {
            continue;
        };
        let names_it = commit
            .message_raw_sloppy()
            .to_string()
            .lines()
            .any(|line| line.strip_prefix(BRANCH_LINE) == Some(branch));
        if !names_it {
            continue;
        }
        let when = commit.time().map_or(0, |time| time.seconds);
        ended.push((
            when,
            super::TurnKey {
                chat: chat.to_owned(),
                number,
            },
        ));
    }
    ended.sort_unstable_by_key(|(when, turn)| std::cmp::Reverse((*when, turn.number)));
    Ended(ended.into_iter().map(|(_, turn)| turn).collect())
}

/// The turns that ended on a branch, newest first ([`ended_on`]).
pub(super) struct Ended(Vec<super::TurnKey>);

impl Ended {
    /// **The turn Undo is offered on** (#5411; ADR 0081 as amended 2026-11-05): the latest turn
    /// that ended on the branch not yet undone, whichever chat's it was. Only that one, since
    /// undoing an older turn whose files a later one changed would need a merge.
    pub(super) fn offered(&self, repo: &gix::Repository) -> Option<&super::TurnKey> {
        self.0.iter().find(|turn| !is_undone(repo, turn))
    }

    /// **The turn the file diff opens on**: the one Undo is offered on, so that after an undo the
    /// turn before it is drawn with its Undo; and where every turn is undone, the latest.
    pub(super) fn opened(&self, repo: &gix::Repository) -> Option<&super::TurnKey> {
        self.offered(repo).or_else(|| self.0.first())
    }

    /// **The turns of `chat`, newest first** (#5408): what the file diff's turn picker lists,
    /// undone ones included, as they stay reviewable.
    pub(super) fn of(&self, chat: &str) -> Vec<u32> {
        let mut numbers: Vec<u32> = self
            .0
            .iter()
            .filter(|turn| turn.chat == chat)
            .map(|turn| turn.number)
            .collect();
        numbers.sort_unstable_by(|a, b| b.cmp(a));
        numbers
    }
}

/// Whether turn `turn` has its `undone` mark.
fn is_undone(repo: &gix::Repository, turn: &super::TurnKey) -> bool {
    chat_ok(&turn.chat)
        && repo
            .find_reference(reference(&turn.chat, turn.number, Side::Undone).as_str())
            .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_filter_any_config_names_is_held_once_and_keys_of_other_shapes_are_passed_over() {
        let held = hold_filters(
            "filter.lfs.clean\nfilter.lfs.smudge\nfilter.my.tool.process\nfilter.short\n",
        );
        assert_eq!(
            held,
            [
                "-c",
                "filter.lfs.clean=",
                "-c",
                "filter.lfs.process=",
                "-c",
                "filter.lfs.required=false",
                "-c",
                "filter.my.tool.clean=",
                "-c",
                "filter.my.tool.process=",
                "-c",
                "filter.my.tool.required=false",
            ]
        );
    }

    #[test]
    fn a_chats_folder_is_placed_by_its_path_whether_or_not_it_is_still_there() {
        let plane = Path::new("/nowhere/plane");
        let place = |path: &str| place_of(plane, &plane.join(path));
        let of = |repo: &str, piece: Option<&str>| {
            Some(Place {
                ws: "ws".into(),
                repo: repo.into(),
                piece: piece.map(str::to_owned),
            })
        };
        assert_eq!(place("workspaces/ws/app"), of("app", None));
        assert_eq!(place("workspaces/ws/app/src/deep"), of("app", None));
        assert_eq!(
            place("workspaces/ws/.worktrees/app/fix/src"),
            of("app", Some("fix"))
        );
        for nowhere in [
            "",
            "personas/x",
            "workspaces",
            "workspaces/ws",
            "workspaces/ws/.worktrees",
            "workspaces/ws/.worktrees/app",
            "workspaces/ws/.charter",
        ] {
            assert_eq!(place(nowhere), None, "{nowhere:?}");
        }
        assert_eq!(
            place_of(plane, Path::new("/elsewhere/workspaces/ws/app")),
            None
        );
    }

    #[test]
    fn a_chat_id_makes_a_ref_only_when_it_is_a_plain_name() {
        assert!(chat_ok("01J9CHAT0000000000000000TN"));
        assert!(chat_ok("a-b_c"));
        for bad in ["", "..", "a/b", "a b", "-x", "a.lock", &"x".repeat(65)] {
            assert!(!chat_ok(bad), "{bad:?}");
        }
    }
}
