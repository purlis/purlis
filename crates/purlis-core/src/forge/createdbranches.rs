// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The branches a chat made through the forge route**, the app's own record (P0b).
//!
//! A chat may move (force-push) only a branch it created: one the forge confirmed it made, in a
//! push that went through the route. That fact is kept here, in the machine config root, which no
//! chat's sandbox can write, and nowhere a chat can influence: not the piece log (a chat can
//! write a `claimed` line into it), not a clone's refs (a chat can delete or never fetch them).
//!
//! One line of JSON per push the forge confirmed, appended (the creation first, then each later
//! push): `{"project_id","chat","host","repo","repo_id","branch","tip"}`. **Every part of the key
//! is a stable identity, none a name a person can reuse:** the plane's project id (a path can later
//! hold another project), the chat's id, and the forge's numeric id of the repo (a repo deleted and
//! made again at the same path has another one). `tip` is the commit that push left the branch at.
//!
//! **A chat may move a branch only from the tip it last put there.** For one `(host, repo id,
//! branch)` **the last line decides** ([`tip_of`]): a push line makes its project and chat the
//! owner and its `tip` the branch's tip; a `gone` line makes nobody. The route reads this at every
//! update it checks, not once at a run's start, and lets an update through only when its old id
//! *is* that tip. So a branch deleted and made again (by a colleague, or by another chat through
//! the route, whose push line becomes the last), or pushed to by anyone else, has another tip and
//! the update is refused, whether or not a `gone` line was written and whether or not the chat's
//! run read an advertisement.
//!
//! **A record is never inherited**: a chat with another id (a resumed chat gets one) owns none of
//! them, and neither does a chat of another project with the same id; the route refuses every
//! update it makes to such a branch: the person pushes it.
//!
//! **One exception: the parent of an ended task, on the branch purlis cut for it** (ADR 0128,
//! amended 2026-11-24). The parent may take up the branch of its *direct* child, in the same
//! workspace, run by the same persona, whose task has ended, and only the branch purlis cut for
//! that task, where the last line is a route-confirmed push ([`Via::Route`], never the person's
//! Allow), from the same tip rule ([`tip_for`]). **The link is the app's own dispatch record**
//! ([`crate::dispatchrecord::lineage_allows`], which no sandboxed chat can write and which obeys
//! `[policy]`'s `lineage`, an allowance on by default whose strict value is `deny`), never a
//! session record or any file a chat writes. It never
//! reaches a protected branch or a delete: the route refuses those before it reads an owner.
//! **The confirmed push is then recorded under the pusher**, so the task can no longer move the
//! branch. The link runs one way: a task never moves a branch of the chat that dispatched it.
//!
//! **A `gone` line is an earlier end of the record**, not the guard: the route reads the forge's
//! advertisement of the repo before a push, and a recorded branch it does not list gets a `gone`
//! line ([`forget`]). A branch made again, by this chat or another through the route, is a new
//! push line.
//!
//! **Fail closed.** A line with no project id or no repo id (written before they were keys) is
//! owned by nobody. A line whose `tip` is not a full commit id (missing, empty or garbled) leaves
//! the branch with no tip to match, so every update to it is refused. A line that does not parse
//! as JSON is skipped; what an update needs is a tip that line would have had to supply, so the
//! worst a skipped line does is leave an older tip that the forge itself no longer holds.

use std::fs::OpenOptions;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The file, in purlis's directory of the machine config root (`config_root` is the parent,
/// `~/.config`): the folder every sandbox denies a chat to write ([`crate::machine::dirs_spelled`]).
pub fn file_in(config_root: &Path) -> PathBuf {
    crate::machine::dir(config_root).join("forge-created-branches.jsonl")
}

/// One branch the forge confirmed a push made or moved, as the policy read it from the push's
/// command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatedBranch {
    pub host: String,
    /// The lower-case repo path, kept for a person reading the file; never the key.
    pub repo: String,
    /// The forge's numeric id of the repo; `None` where the route does not know it, and then
    /// nothing is recorded.
    pub repo_id: Option<u64>,
    pub branch: String,
    /// The commit the push left the branch at.
    pub tip: String,
    /// **The push was checked as one that may only add to the branch** (the chat held it by the
    /// person's Allow, [`OwnedTip::fast_forward_only`]): its route line is recorded
    /// [`Via::Allow`] whatever the file says when the forge confirms it, so a line written
    /// between the check and the confirm can never drop the marker (#7841). Read only by a
    /// [`Via::Route`] record.
    pub fast_forward_only: bool,
}

#[derive(Serialize, Deserialize)]
struct Line {
    /// Absent in a line written before the plane's project id was a key: no project owns it.
    #[serde(default)]
    project_id: Option<String>,
    #[serde(default)]
    chat: String,
    host: String,
    repo: String,
    /// Absent in a line written before the forge's repo id was a key: no repo owns it.
    #[serde(default)]
    repo_id: Option<u64>,
    branch: String,
    #[serde(default)]
    tip: String,
    /// The branch was seen gone from the forge: nobody owns it from here.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    gone: bool,
    /// How the line came to be written; absent before the key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    via: Option<Via>,
}

/// **How a push line came to be written**: what lets a reader tell a push the forge confirmed
/// through the route from the person's **Allow** (`docs/plane-format.md`). A line without it
/// (written before the key, or by [`record`]) is neither, and is never taken for a route push:
/// like an Allow, a branch it holds moves only forward ([`OwnedTip::fast_forward_only`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Via {
    /// The forge confirmed a push that went through the route.
    Route,
    /// The person pressed Allow on *Let this chat push it* ([`crate::windowonly::let_chat_push`]).
    Allow,
}

fn append(config_root: &Path, lines: &[Line]) -> io::Result<()> {
    if lines.is_empty() {
        return Ok(());
    }
    std::fs::create_dir_all(crate::machine::dir(config_root))?;
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = options.open(file_in(config_root))?;
    // One `write` per line to a file opened for append, so concurrent routes do not tear it.
    for line in lines {
        let mut text = serde_json::to_string(line).map_err(io::Error::other)?;
        text.push('\n');
        file.write_all(text.as_bytes())?;
    }
    file.flush()
}

/// Records that `chat` of the project with the stable id `project_id` made, or moved to a new tip,
/// each of `created`. A
/// branch whose repo id is not known is not recorded: it could never be matched.
pub fn record(
    config_root: &Path,
    project_id: &str,
    chat: &str,
    created: &[CreatedBranch],
) -> io::Result<()> {
    record_via(config_root, project_id, chat, created, None)
}

/// [`record`], saying how the lines came to be written ([`Via`]): the route's confirmed push
/// is [`Via::Route`] (`forgeroute`'s `record_created`), the person's Allow [`Via::Allow`].
///
/// **A branch a chat owns by the person's Allow, or by a line with no `via`, stays non-lineage
/// when the chat pushes it again through the route**: the later line keeps the earlier one's
/// marker, so the Allow never becomes a route push one push later and so never chains to the
/// chat that asked for the child ([`tip_for`]). Only a branch the chat made through the route
/// itself, or took up by lineage from another chat, is a route line. **A push the route checked as
/// one that may only add** ([`CreatedBranch::fast_forward_only`]) is recorded [`Via::Allow`] from
/// that check, not from the file read again at the confirm, where another chat's line may stand
/// by then (#7841).
pub fn record_via(
    config_root: &Path,
    project_id: &str,
    chat: &str,
    created: &[CreatedBranch],
    via: Option<Via>,
) -> io::Result<()> {
    let lines: Vec<Line> = created
        .iter()
        .filter(|c| c.repo_id.is_some())
        .map(|c| {
            let via = match (via, c.repo_id) {
                // The check read the chat's Allow: the marker it read is the one recorded.
                (Some(Via::Route), Some(_)) if c.fast_forward_only => Some(Via::Allow),
                (Some(Via::Route), Some(repo_id)) => {
                    match owner_line(config_root, &c.host, repo_id, &c.branch) {
                        Some(earlier)
                            if earlier.project == project_id
                                && earlier.chat == chat
                                && earlier.via != Some(Via::Route) =>
                        {
                            earlier.via
                        }
                        _ => via,
                    }
                }
                _ => via,
            };
            Line {
                project_id: Some(project_id.to_owned()),
                chat: chat.to_owned(),
                host: c.host.clone(),
                repo: c.repo.clone(),
                repo_id: c.repo_id,
                branch: c.branch.clone(),
                tip: c.tip.clone(),
                gone: false,
                via,
            }
        })
        .collect();
    append(config_root, &lines)
}

/// Ends the record of each of `gone`: the forge's advertisement no longer lists it.
pub fn forget(config_root: &Path, gone: &[CreatedBranch]) -> io::Result<()> {
    let lines: Vec<Line> = gone
        .iter()
        .filter(|c| c.repo_id.is_some())
        .map(|c| Line {
            project_id: None,
            chat: String::new(),
            host: c.host.clone(),
            repo: c.repo.clone(),
            repo_id: c.repo_id,
            branch: c.branch.clone(),
            tip: String::new(),
            gone: true,
            via: None,
        })
        .collect();
    append(config_root, &lines)
}

/// A full commit id: 40 (SHA-1) or 64 (SHA-256) hex digits.
pub fn is_commit_id(id: &str) -> bool {
    matches!(id.len(), 40 | 64) && id.bytes().all(|b| b.is_ascii_hexdigit())
}

/// For each branch of the repo `repo_id` on `host`, in the order first seen: the tip `chat` of
/// `project_id` last put on it, or `None` where the last line is nobody's, someone else's, gone
/// or has no tip that reads. Any doubt (no file, an unreadable one) is no branch.
fn decided(
    config_root: &Path,
    project_id: &str,
    chat: &str,
    host: &str,
    repo_id: u64,
) -> Vec<(String, Option<String>)> {
    let Ok(text) = std::fs::read_to_string(file_in(config_root)) else {
        return Vec::new();
    };
    let mut decided: Vec<(String, Option<String>)> = Vec::new();
    for line in text.lines() {
        let Ok(line) = serde_json::from_str::<Line>(line) else {
            continue;
        };
        if line.host != host || line.repo_id != Some(repo_id) {
            continue;
        }
        let mine = !line.gone
            && line.project_id.as_deref() == Some(project_id)
            && line.chat == chat
            && is_commit_id(&line.tip);
        let tip = mine.then(|| line.tip.to_ascii_lowercase());
        match decided.iter_mut().find(|(b, _)| *b == line.branch) {
            Some(entry) => entry.1 = tip,
            None => decided.push((line.branch, tip)),
        }
    }
    decided
}

/// Who the last line for `branch` of the repo `repo_id` on `host` makes its owner, and the tip it
/// left: `(project id, chat, tip)`, **read from the file now**. `None` unless that line is a
/// push with a project, a chat and a tip that reads.
pub fn owner_of(
    config_root: &Path,
    host: &str,
    repo_id: u64,
    branch: &str,
) -> Option<(String, String, String)> {
    owner_line(config_root, host, repo_id, branch).map(|o| (o.project, o.chat, o.tip))
}

/// The last line for a branch, as [`owner_of`] reads it, with how it was written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Owner {
    pub project: String,
    pub chat: String,
    /// Lower-case.
    pub tip: String,
    pub via: Option<Via>,
}

/// [`owner_of`], with the line's [`Via`].
pub fn owner_line(config_root: &Path, host: &str, repo_id: u64, branch: &str) -> Option<Owner> {
    let text = std::fs::read_to_string(file_in(config_root)).ok()?;
    let mut last: Option<Line> = None;
    for line in text.lines() {
        let Ok(line) = serde_json::from_str::<Line>(line) else {
            continue;
        };
        if line.host == host && line.repo_id == Some(repo_id) && line.branch == branch {
            last = Some(line);
        }
    }
    let line = last.filter(|l| !l.gone && is_commit_id(&l.tip))?;
    let project = line.project_id.filter(|p| !p.is_empty())?;
    (!line.chat.is_empty()).then(|| Owner {
        project,
        chat: line.chat,
        tip: line.tip.to_ascii_lowercase(),
        via: line.via,
    })
}

/// [`tip_of`], or, where the branch's last line is a **route-confirmed push** ([`Via::Route`])
/// of a chat of the same project and `lineage(owner)` says `chat` may take it up (a parent and
/// its ended child's cut branch: [`crate::dispatchrecord::lineage_allows`], from the app's own
/// dispatch records, asked only when the branch is another chat's), that line's tip. A line the
/// person's Allow wrote, or one with no `via`, is never lineage.
pub fn tip_for(
    config_root: &Path,
    project_id: &str,
    chat: &str,
    host: &str,
    repo_id: u64,
    branch: &str,
    lineage: &dyn Fn(&str) -> bool,
) -> Option<String> {
    owned_tip_for(
        config_root,
        project_id,
        chat,
        host,
        repo_id,
        branch,
        lineage,
    )
    .map(|t| t.tip)
}

/// The tip a chat may move a branch from, and whether it may move it only forward.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedTip {
    /// Lower-case.
    pub tip: String,
    /// **The chat owns the branch by anything but a route push** ([`Via::Allow`], which a later
    /// push keeps, or a line with no `via`, which an Allow of #7415's time to #7552's is): the
    /// branch may have been made by someone else, and floor item 6 of ADR 0125's 2026-11-27
    /// amendment holds, so its new tip must descend from the forge's (#7841). Only a
    /// [`Via::Route`] line, a branch the chat made through the route, may be rewound.
    pub fast_forward_only: bool,
}

/// [`tip_for`], from **one read of the file**, with whether the update must be a fast-forward
/// ([`OwnedTip::fast_forward_only`]): the last line for the branch says both, so a line written
/// between two reads can never pair one line's tip with another's marker.
pub fn owned_tip_for(
    config_root: &Path,
    project_id: &str,
    chat: &str,
    host: &str,
    repo_id: u64,
    branch: &str,
    lineage: &dyn Fn(&str) -> bool,
) -> Option<OwnedTip> {
    let owner = owner_line(config_root, host, repo_id, branch)?;
    if owner.project != project_id {
        return None;
    }
    if owner.chat == chat {
        return Some(OwnedTip {
            fast_forward_only: owner.via != Some(Via::Route),
            tip: owner.tip,
        });
    }
    (owner.via == Some(Via::Route) && lineage(&owner.chat)).then_some(OwnedTip {
        tip: owner.tip,
        fast_forward_only: false,
    })
}

/// The tip `chat` of the project `project_id` last put on `branch` of the repo with forge id
/// `repo_id` on `host`, **read from the file now**: `None` unless the last line for the branch is
/// this chat's push with a tip that reads. An update to the branch is allowed only from this id.
pub fn tip_of(
    config_root: &Path,
    project_id: &str,
    chat: &str,
    host: &str,
    repo_id: u64,
    branch: &str,
) -> Option<String> {
    decided(config_root, project_id, chat, host, repo_id)
        .into_iter()
        .find(|(b, _)| b == branch)
        .and_then(|(_, tip)| tip)
}

/// The branches of the repo with forge id `repo_id` on `host` that `chat` of the project
/// `project_id` itself made or last pushed, and that have not been seen gone since.
pub fn created_by(
    config_root: &Path,
    project_id: &str,
    chat: &str,
    host: &str,
    repo_id: u64,
) -> Vec<String> {
    decided(config_root, project_id, chat, host, repo_id)
        .into_iter()
        .filter_map(|(branch, tip)| tip.map(|_| branch))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROJECT: &str = "01J0000000000000000000000A";
    const OTHER_PROJECT: &str = "01J0000000000000000000000B";
    const REPO_ID: u64 = 7;

    fn made(branch: &str) -> CreatedBranch {
        CreatedBranch {
            host: "github.com".into(),
            repo: "o/r".into(),
            repo_id: Some(REPO_ID),
            branch: branch.into(),
            tip: "a".repeat(40),
            fast_forward_only: false,
        }
    }

    fn own(dir: &Path, project: &str, chat: &str) -> Vec<String> {
        created_by(dir, project, chat, "github.com", REPO_ID)
    }

    #[test]
    fn lineage_is_asked_only_of_a_route_push_line_never_the_persons_allow_or_an_unmarked_one() {
        let dir = tempfile::tempdir().unwrap();
        let yes = |_: &str| true;
        for (via, lets) in [
            (Some(Via::Route), true),
            (Some(Via::Allow), false),
            (None, false),
        ] {
            record_via(dir.path(), PROJECT, "child", &[made("cut")], via).unwrap();
            let tip = tip_for(
                dir.path(),
                PROJECT,
                "parent",
                "github.com",
                REPO_ID,
                "cut",
                &yes,
            );
            assert_eq!(tip.is_some(), lets, "{via:?}");
        }
    }

    #[test]
    fn a_later_route_push_of_a_branch_the_person_allowed_stays_non_lineage() {
        let dir = tempfile::tempdir().unwrap();
        let yes = |_: &str| true;
        let lineage = || {
            tip_for(
                dir.path(),
                PROJECT,
                "parent",
                "github.com",
                REPO_ID,
                "cut",
                &yes,
            )
        };
        record_via(
            dir.path(),
            PROJECT,
            "child",
            &[made("cut")],
            Some(Via::Allow),
        )
        .unwrap();
        record_via(
            dir.path(),
            PROJECT,
            "child",
            &[made("cut")],
            Some(Via::Route),
        )
        .unwrap();
        record_via(
            dir.path(),
            PROJECT,
            "child",
            &[made("cut")],
            Some(Via::Route),
        )
        .unwrap();
        assert_eq!(
            lineage(),
            None,
            "the Allow is not a route push, however often it moves"
        );
        let owner = owner_line(dir.path(), "github.com", REPO_ID, "cut").unwrap();
        assert_eq!(owner.via, Some(Via::Allow));
        // An unmarked line stays unmarked.
        record(dir.path(), PROJECT, "child", &[made("old")]).unwrap();
        record_via(
            dir.path(),
            PROJECT,
            "child",
            &[made("old")],
            Some(Via::Route),
        )
        .unwrap();
        let owner = owner_line(dir.path(), "github.com", REPO_ID, "old").unwrap();
        assert_eq!(owner.via, None);
    }

    #[test]
    fn a_branch_a_chat_made_through_the_route_stays_a_route_line_and_a_new_owner_starts_one() {
        let dir = tempfile::tempdir().unwrap();
        record_via(
            dir.path(),
            PROJECT,
            "child",
            &[made("cut")],
            Some(Via::Route),
        )
        .unwrap();
        record_via(
            dir.path(),
            PROJECT,
            "child",
            &[made("cut")],
            Some(Via::Route),
        )
        .unwrap();
        let via = |branch: &str| {
            owner_line(dir.path(), "github.com", REPO_ID, branch)
                .unwrap()
                .via
        };
        assert_eq!(via("cut"), Some(Via::Route));
        // The person's Allow of a branch another chat owned does not make the new owner's later
        // route push a route line; a route push of another chat's branch (lineage) does.
        record_via(
            dir.path(),
            PROJECT,
            "other",
            &[made("cut")],
            Some(Via::Allow),
        )
        .unwrap();
        record_via(
            dir.path(),
            PROJECT,
            "other",
            &[made("cut")],
            Some(Via::Route),
        )
        .unwrap();
        assert_eq!(via("cut"), Some(Via::Allow));
        record_via(
            dir.path(),
            PROJECT,
            "third",
            &[made("cut")],
            Some(Via::Route),
        )
        .unwrap();
        assert_eq!(via("cut"), Some(Via::Route));
    }

    #[test]
    fn lineage_is_not_asked_for_a_chat_of_another_project() {
        let dir = tempfile::tempdir().unwrap();
        record_via(
            dir.path(),
            OTHER_PROJECT,
            "child",
            &[made("cut")],
            Some(Via::Route),
        )
        .unwrap();
        let asked = std::cell::Cell::new(false);
        let spy = |_: &str| {
            asked.set(true);
            true
        };
        let tip = tip_for(
            dir.path(),
            PROJECT,
            "parent",
            "github.com",
            REPO_ID,
            "cut",
            &spy,
        );
        assert_eq!(tip, None);
        assert!(!asked.get(), "another project's chat is never asked about");
    }

    #[test]
    fn a_line_without_via_still_reads_and_a_marked_one_reads_back_its_marker() {
        let dir = tempfile::tempdir().unwrap();
        record(dir.path(), PROJECT, "a", &[made("old")]).unwrap();
        record_via(dir.path(), PROJECT, "a", &[made("new")], Some(Via::Allow)).unwrap();
        let via =
            |branch: &str| owner_line(dir.path(), "github.com", REPO_ID, branch).map(|o| o.via);
        assert_eq!(via("old"), Some(None));
        assert_eq!(via("new"), Some(Some(Via::Allow)));
        let text = std::fs::read_to_string(file_in(dir.path())).unwrap();
        assert!(text.lines().next().unwrap().find("via").is_none(), "{text}");
        assert!(text.contains(r#""via":"allow""#), "{text}");
    }

    #[test]
    fn a_branch_is_the_creating_chats_own_and_no_other_chats() {
        let dir = tempfile::tempdir().unwrap();
        record(dir.path(), PROJECT, "chat-a", &[made("work")]).unwrap();
        assert_eq!(own(dir.path(), PROJECT, "chat-a"), ["work"]);
        assert!(own(dir.path(), PROJECT, "chat-b").is_empty());
    }

    #[test]
    fn another_projects_chat_with_the_same_id_owns_nothing() {
        let dir = tempfile::tempdir().unwrap();
        record(dir.path(), PROJECT, "chat-a", &[made("work")]).unwrap();
        record(dir.path(), OTHER_PROJECT, "chat-a", &[made("theirs")]).unwrap();
        assert_eq!(own(dir.path(), PROJECT, "chat-a"), ["work"]);
        assert_eq!(own(dir.path(), OTHER_PROJECT, "chat-a"), ["theirs"]);
    }

    #[test]
    fn a_repo_made_again_at_the_same_path_owns_nothing() {
        let dir = tempfile::tempdir().unwrap();
        record(dir.path(), PROJECT, "chat-a", &[made("work")]).unwrap();
        // The same host and path, another forge id: a repo deleted and created again.
        assert!(created_by(dir.path(), PROJECT, "chat-a", "github.com", REPO_ID + 1).is_empty());
    }

    #[test]
    fn a_branch_is_own_only_in_the_repo_and_host_it_was_made_in() {
        let dir = tempfile::tempdir().unwrap();
        record(dir.path(), PROJECT, "chat-a", &[made("work")]).unwrap();
        assert!(created_by(dir.path(), PROJECT, "chat-a", "gitlab.com", REPO_ID).is_empty());
        assert!(created_by(dir.path(), PROJECT, "chat-a", "github.com", 8).is_empty());
    }

    #[test]
    fn a_branch_seen_gone_is_nobodys_until_made_again() {
        let dir = tempfile::tempdir().unwrap();
        record(dir.path(), PROJECT, "chat-a", &[made("work"), made("keep")]).unwrap();
        forget(dir.path(), &[made("work")]).unwrap();
        assert_eq!(own(dir.path(), PROJECT, "chat-a"), ["keep"]);
        record(dir.path(), PROJECT, "chat-a", &[made("work")]).unwrap();
        assert_eq!(own(dir.path(), PROJECT, "chat-a"), ["work", "keep"]);
    }

    #[test]
    fn a_branch_made_again_by_another_chat_is_theirs_and_not_the_first_chats() {
        let dir = tempfile::tempdir().unwrap();
        record(dir.path(), PROJECT, "chat-a", &[made("work")]).unwrap();
        record(dir.path(), PROJECT, "chat-b", &[made("work")]).unwrap();
        assert!(own(dir.path(), PROJECT, "chat-a").is_empty());
        assert_eq!(own(dir.path(), PROJECT, "chat-b"), ["work"]);
    }

    #[test]
    fn an_old_line_with_no_project_id_or_repo_id_is_owned_by_nobody() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(crate::machine::dir(dir.path())).unwrap();
        std::fs::write(
            file_in(dir.path()),
            concat!(
                "{\"project\":\"/work/p\",\"chat\":\"chat-a\",\"host\":\"github.com\",",
                "\"repo\":\"o/r\",\"branch\":\"old\"}\n",
                "{\"project_id\":\"01J0000000000000000000000A\",\"chat\":\"chat-a\",",
                "\"host\":\"github.com\",\"repo\":\"o/r\",\"branch\":\"no-repo-id\"}\n",
            ),
        )
        .unwrap();
        assert!(own(dir.path(), PROJECT, "chat-a").is_empty());
        assert!(own(dir.path(), "", "chat-a").is_empty());
        // A new line beside them still reads.
        record(dir.path(), PROJECT, "chat-a", &[made("new")]).unwrap();
        assert_eq!(own(dir.path(), PROJECT, "chat-a"), ["new"]);
    }

    #[test]
    fn a_branch_with_no_known_repo_id_is_not_recorded() {
        let dir = tempfile::tempdir().unwrap();
        let mut unknown = made("work");
        unknown.repo_id = None;
        record(dir.path(), PROJECT, "chat-a", &[unknown]).unwrap();
        assert!(!file_in(dir.path()).exists());
    }

    #[test]
    fn a_line_that_does_not_read_grants_nothing_and_loses_no_other() {
        let dir = tempfile::tempdir().unwrap();
        record(dir.path(), PROJECT, "chat-a", &[made("one")]).unwrap();
        let mut f = OpenOptions::new()
            .append(true)
            .open(file_in(dir.path()))
            .unwrap();
        f.write_all(b"not json\n").unwrap();
        record(dir.path(), PROJECT, "chat-a", &[made("two")]).unwrap();
        assert_eq!(own(dir.path(), PROJECT, "chat-a"), ["one", "two"]);
    }

    fn owned(dir: &Path, chat: &str, branch: &str) -> Option<OwnedTip> {
        owned_tip_for(dir, PROJECT, chat, "github.com", REPO_ID, branch, &|_| {
            false
        })
    }

    #[test]
    fn a_push_checked_as_only_adding_is_recorded_allow_whatever_line_is_written_before_its_confirm()
    {
        let dir = tempfile::tempdir().unwrap();
        record_via(
            dir.path(),
            PROJECT,
            "a",
            &[made("theirs")],
            Some(Via::Allow),
        )
        .unwrap();
        // The check reads the chat's Allow: the push may only add to the branch.
        let checked = owned(dir.path(), "a", "theirs").expect("a's by the Allow");
        assert!(checked.fast_forward_only);
        // Between the check and the forge's confirm, the person lets another chat push it.
        record_via(
            dir.path(),
            PROJECT,
            "b",
            &[made("theirs")],
            Some(Via::Allow),
        )
        .unwrap();
        let mut pushed = made("theirs");
        pushed.tip = "b".repeat(40);
        pushed.fast_forward_only = checked.fast_forward_only;
        record_via(dir.path(), PROJECT, "a", &[pushed], Some(Via::Route)).unwrap();

        let owner = owner_line(dir.path(), "github.com", REPO_ID, "theirs").unwrap();
        assert_eq!(
            owner.via,
            Some(Via::Allow),
            "the marker the check read is kept"
        );
        assert!(owned(dir.path(), "a", "theirs").unwrap().fast_forward_only);
    }

    #[test]
    fn a_branch_held_by_a_line_with_no_via_moves_only_forward_and_a_route_line_does_not() {
        let dir = tempfile::tempdir().unwrap();
        // A line written before `via` was a key (an Allow of #7415's time reads the same).
        record(dir.path(), PROJECT, "a", &[made("old")]).unwrap();
        record_via(dir.path(), PROJECT, "a", &[made("cut")], Some(Via::Route)).unwrap();

        assert!(owned(dir.path(), "a", "old").unwrap().fast_forward_only);
        assert!(!owned(dir.path(), "a", "cut").unwrap().fast_forward_only);
    }

    #[test]
    fn the_last_owner_is_who_pushed_last_with_the_tip_it_left() {
        let dir = tempfile::tempdir().unwrap();
        assert!(owner_of(dir.path(), "github.com", REPO_ID, "work").is_none());
        record(dir.path(), PROJECT, "task", &[made("work")]).unwrap();
        let mut moved = made("work");
        moved.tip = "b".repeat(40);
        record(dir.path(), PROJECT, "asker", &[moved]).unwrap();
        assert_eq!(
            owner_of(dir.path(), "github.com", REPO_ID, "work"),
            Some((PROJECT.to_owned(), "asker".to_owned(), "b".repeat(40)))
        );
        forget(dir.path(), &[made("work")]).unwrap();
        assert!(owner_of(dir.path(), "github.com", REPO_ID, "work").is_none());
    }

    #[test]
    fn a_dispatcher_takes_up_its_tasks_branch_and_the_task_then_loses_it() {
        let dir = tempfile::tempdir().unwrap();
        let tip = |chat: &str, project: &str, repo_id: u64| {
            tip_for(
                dir.path(),
                project,
                chat,
                "github.com",
                repo_id,
                "work",
                &|owner| owner == "task" && chat == "asker",
            )
        };
        record_via(
            dir.path(),
            PROJECT,
            "task",
            &[made("work")],
            Some(Via::Route),
        )
        .unwrap();
        // The task is the owner; its dispatcher may move the branch from the recorded tip.
        assert_eq!(tip("task", PROJECT, REPO_ID), Some("a".repeat(40)));
        assert_eq!(tip("asker", PROJECT, REPO_ID), Some("a".repeat(40)));
        // Not another chat, another project's chat of the same id, or another repo.
        assert_eq!(tip("other", PROJECT, REPO_ID), None);
        assert_eq!(tip("asker", OTHER_PROJECT, REPO_ID), None);
        assert_eq!(tip("asker", PROJECT, REPO_ID + 1), None);
        // The dispatcher's confirmed push is recorded under it: now the task cannot move it.
        let mut moved = made("work");
        moved.tip = "c".repeat(40);
        record(dir.path(), PROJECT, "asker", &[moved]).unwrap();
        assert_eq!(tip("asker", PROJECT, REPO_ID), Some("c".repeat(40)));
        assert_eq!(tip("task", PROJECT, REPO_ID), None);
    }

    #[test]
    fn a_branch_nobody_dispatched_from_is_not_taken_up() {
        let dir = tempfile::tempdir().unwrap();
        record(dir.path(), PROJECT, "task", &[made("work")]).unwrap();
        let never = |_: &str| false;
        assert_eq!(
            tip_for(
                dir.path(),
                PROJECT,
                "asker",
                "github.com",
                REPO_ID,
                "work",
                &never
            ),
            None
        );
    }

    #[test]
    fn no_file_is_no_branch() {
        let dir = tempfile::tempdir().unwrap();
        assert!(own(dir.path(), PROJECT, "chat-a").is_empty());
    }
}
