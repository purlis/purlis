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
//! **One exception: the chat that asked for the work** (ADR 0128, amended 2026-11-23). A branch a
//! *task* made is the dispatching chat's to follow up after the task ends, so an update is also
//! let through when the branch's recorded owner is a chat the pusher dispatched, directly or up
//! the chain ([`tip_for`]), from the same tip rule: the old id is the recorded tip. **The link is
//! the app's own dispatch record** ([`crate::dispatchrecord`], which no chat can write), never a
//! session record or any file a chat writes; where the record shows no such link the update is
//! refused. **The confirmed push is then recorded under the pusher** ([`record`]'s one line,
//! which makes the last line the dispatcher's): the task, if it still runs, is no longer the
//! owner and its next update is refused, so the task and the dispatcher cannot both keep moving
//! the branch. The link runs one way: a task never moves a branch of the chat that dispatched it.
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
    let lines: Vec<Line> = created
        .iter()
        .filter(|c| c.repo_id.is_some())
        .map(|c| Line {
            project_id: Some(project_id.to_owned()),
            chat: chat.to_owned(),
            host: c.host.clone(),
            repo: c.repo.clone(),
            repo_id: c.repo_id,
            branch: c.branch.clone(),
            tip: c.tip.clone(),
            gone: false,
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
    (!line.chat.is_empty()).then(|| (project, line.chat, line.tip.to_ascii_lowercase()))
}

/// [`tip_of`], or, where the branch is the record of a chat of the same project that `chat`
/// dispatched, that chat's tip. `dispatched(owner)` says whether `chat` dispatched the chat `owner`,
/// from the app's dispatch record (asked only when the branch is another chat's).
pub fn tip_for(
    config_root: &Path,
    project_id: &str,
    chat: &str,
    host: &str,
    repo_id: u64,
    branch: &str,
    dispatched: &dyn Fn(&str) -> bool,
) -> Option<String> {
    if let Some(tip) = tip_of(config_root, project_id, chat, host, repo_id, branch) {
        return Some(tip);
    }
    let (project, owner, tip) = owner_of(config_root, host, repo_id, branch)?;
    (project == project_id && owner != chat && dispatched(&owner)).then_some(tip)
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
        }
    }

    fn own(dir: &Path, project: &str, chat: &str) -> Vec<String> {
        created_by(dir, project, chat, "github.com", REPO_ID)
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
        record(dir.path(), PROJECT, "task", &[made("work")]).unwrap();
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
