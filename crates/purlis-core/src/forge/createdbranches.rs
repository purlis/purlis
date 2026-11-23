// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The branches a chat made through the forge route**, the app's own record (P0b).
//!
//! A chat may move (force-push) only a branch it created: one the forge confirmed it made, in a
//! push that went through the route. That fact is kept here, in the machine config root, which no
//! chat's sandbox can write, and nowhere a chat can influence: not the piece log (a chat can
//! write a `claimed` line into it), not a clone's refs (a chat can delete or never fetch them).
//!
//! One line of JSON per creation, appended: `{"project_id","chat","host","repo","repo_id",
//! "branch","tip"}`. **Every part of the key is a stable identity, none a name a person can
//! reuse:** the plane's project id (a path can later hold another project), the chat's id, and
//! the forge's numeric id of the repo (a repo deleted and made again at the same path has
//! another one). `tip` is the commit the branch was made at.
//!
//! **A record is never inherited**: a chat with another id (a resumed chat gets one) owns none of
//! them, and neither does a chat of another project with the same id; the route refuses every
//! update it makes to such a branch: the person pushes it.
//!
//! **A record dies with the branch it names.** The route reads the forge's advertisement of the
//! repo before every push; a recorded branch that the advertisement does not list is gone, and
//! a `gone` line is appended for it ([`forget`]). A branch made again, by this chat or another
//! through the route, is a new creation line. For one `(host, repo id, branch)` **the last line
//! decides**: a creation line makes its chat the owner, a `gone` line makes nobody. So a name
//! someone re-creates after the forge deleted it is not the chat's, unless this chat made it again.
//!
//! A line with no project id or no repo id (written before they were keys) is owned by nobody: it
//! fails closed. A line that does not read is skipped, which only ever loses ownership, never
//! grants it.

use std::fs::OpenOptions;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The file, in purlis's directory of the machine config root (`config_root` is the parent,
/// `~/.config`): the folder every sandbox denies a chat to write ([`crate::machine::dirs_spelled`]).
pub fn file_in(config_root: &Path) -> PathBuf {
    crate::machine::dir(config_root).join("forge-created-branches.jsonl")
}

/// One branch the forge confirmed a push made, as the policy read it from the push's command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatedBranch {
    pub host: String,
    /// The lower-case repo path, kept for a person reading the file; never the key.
    pub repo: String,
    /// The forge's numeric id of the repo; `None` where the route does not know it, and then
    /// nothing is recorded.
    pub repo_id: Option<u64>,
    pub branch: String,
    /// The commit the push made the branch at.
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

/// Records that `chat` of the project with the stable id `project_id` made each of `created`. A
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

/// The branches of the repo with forge id `repo_id` on `host` that `chat` of the project
/// `project_id` itself made and that have not been seen gone since. Any doubt (no file, an
/// unreadable one, a line with no project id or no repo id) is none.
pub fn created_by(
    config_root: &Path,
    project_id: &str,
    chat: &str,
    host: &str,
    repo_id: u64,
) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(file_in(config_root)) else {
        return Vec::new();
    };
    // The last line for a branch decides: `Some(true)` is this chat's, `Some(false)` nobody's or
    // someone else's.
    let mut decided: Vec<(String, bool)> = Vec::new();
    for line in text.lines() {
        let Ok(line) = serde_json::from_str::<Line>(line) else {
            continue;
        };
        if line.host != host || line.repo_id != Some(repo_id) {
            continue;
        }
        let mine = !line.gone
            && line.project_id.as_deref() == Some(project_id)
            && line.chat == chat;
        match decided.iter_mut().find(|(b, _)| *b == line.branch) {
            Some(entry) => entry.1 = mine,
            None => decided.push((line.branch, mine)),
        }
    }
    decided
        .into_iter()
        .filter_map(|(branch, mine)| mine.then_some(branch))
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
    fn no_file_is_no_branch() {
        let dir = tempfile::tempdir().unwrap();
        assert!(own(dir.path(), PROJECT, "chat-a").is_empty());
    }
}
