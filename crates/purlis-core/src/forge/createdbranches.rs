// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The branches a chat made through the forge route**, the app's own record (P0b).
//!
//! A chat may move (force-push) only a branch it created: one the forge confirmed it made, in a
//! push that went through the route. That fact is kept here, in the machine config root, which no
//! chat's sandbox can write, and nowhere a chat can influence: not the piece log (a chat can
//! write a `claimed` line into it), not a clone's refs (a chat can delete or never fetch them).
//!
//! One line of JSON per creation, appended: `{"project","chat","host","repo","branch"}`, keyed by
//! the project and the creating chat's stable id. **A record is never inherited**: a chat with
//! another id (a resumed chat gets one) owns none of them, and neither does a chat of another
//! project with the same id; the route refuses every update it makes to such a branch: the person
//! pushes it. A line with no project (written before the project was a key) is owned by nobody: it
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

#[derive(Serialize, Deserialize)]
struct Line {
    /// Absent in a line written before the project was a key: no project owns it.
    #[serde(default)]
    project: Option<String>,
    chat: String,
    host: String,
    repo: String,
    branch: String,
}

/// Records that `chat` of `project` made each of `created` (`(host, repo path, branch)`, as the
/// policy keys them). One `write` per line to a file opened for append, so concurrent routes do
/// not tear it.
pub fn record(
    config_root: &Path,
    project: &Path,
    chat: &str,
    created: &[(String, String, String)],
) -> io::Result<()> {
    if created.is_empty() {
        return Ok(());
    }
    std::fs::create_dir_all(crate::machine::dir(config_root))?;
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = options.open(file_in(config_root))?;
    for (host, repo, branch) in created {
        let mut line = serde_json::to_string(&Line {
            project: Some(project.to_string_lossy().into_owned()),
            chat: chat.to_owned(),
            host: host.clone(),
            repo: repo.clone(),
            branch: branch.clone(),
        })
        .map_err(io::Error::other)?;
        line.push('\n');
        file.write_all(line.as_bytes())?;
    }
    file.flush()
}

/// The branches of `repo` on `host` that `chat` of `project` itself made. Any doubt (no file, an
/// unreadable one, a line with no project) is none.
pub fn created_by(
    config_root: &Path,
    project: &Path,
    chat: &str,
    host: &str,
    repo: &str,
) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(file_in(config_root)) else {
        return Vec::new();
    };
    let project = project.to_string_lossy();
    let mut out: Vec<String> = Vec::new();
    for line in text.lines() {
        let Ok(line) = serde_json::from_str::<Line>(line) else {
            continue;
        };
        if line.project.as_deref() == Some(project.as_ref())
            && line.chat == chat
            && line.host == host
            && line.repo == repo
            && !out.contains(&line.branch)
        {
            out.push(line.branch);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROJECT: &str = "/work/project-a";

    fn key(branch: &str) -> (String, String, String) {
        ("github.com".into(), "o/r".into(), branch.into())
    }

    fn own(dir: &Path, project: &str, chat: &str) -> Vec<String> {
        created_by(dir, Path::new(project), chat, "github.com", "o/r")
    }

    #[test]
    fn a_branch_is_the_creating_chats_own_and_no_other_chats() {
        let dir = tempfile::tempdir().unwrap();
        record(dir.path(), Path::new(PROJECT), "chat-a", &[key("work")]).unwrap();
        assert_eq!(own(dir.path(), PROJECT, "chat-a"), ["work"]);
        assert!(own(dir.path(), PROJECT, "chat-b").is_empty());
    }

    #[test]
    fn the_same_project_and_chat_owns_its_branch() {
        let dir = tempfile::tempdir().unwrap();
        record(dir.path(), Path::new(PROJECT), "chat-a", &[key("work")]).unwrap();
        record(
            dir.path(),
            Path::new("/work/project-b"),
            "chat-a",
            &[key("theirs")],
        )
        .unwrap();
        assert_eq!(own(dir.path(), PROJECT, "chat-a"), ["work"]);
        assert_eq!(own(dir.path(), "/work/project-b", "chat-a"), ["theirs"]);
    }

    #[test]
    fn another_projects_chat_with_the_same_id_owns_nothing() {
        let dir = tempfile::tempdir().unwrap();
        record(dir.path(), Path::new(PROJECT), "chat-a", &[key("work")]).unwrap();
        assert!(own(dir.path(), "/work/project-b", "chat-a").is_empty());
    }

    #[test]
    fn an_old_line_with_no_project_is_owned_by_nobody() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(crate::machine::dir(dir.path())).unwrap();
        std::fs::write(
            file_in(dir.path()),
            "{\"chat\":\"chat-a\",\"host\":\"github.com\",\"repo\":\"o/r\",\"branch\":\"old\"}\n",
        )
        .unwrap();
        assert!(own(dir.path(), PROJECT, "chat-a").is_empty());
        assert!(own(dir.path(), "", "chat-a").is_empty());
        // A new line beside it still reads.
        record(dir.path(), Path::new(PROJECT), "chat-a", &[key("new")]).unwrap();
        assert_eq!(own(dir.path(), PROJECT, "chat-a"), ["new"]);
    }

    #[test]
    fn a_branch_is_own_only_in_the_repo_and_host_it_was_made_in() {
        let dir = tempfile::tempdir().unwrap();
        record(dir.path(), Path::new(PROJECT), "chat-a", &[key("work")]).unwrap();
        let p = Path::new(PROJECT);
        assert!(created_by(dir.path(), p, "chat-a", "github.com", "o/other").is_empty());
        assert!(created_by(dir.path(), p, "chat-a", "gitlab.com", "o/r").is_empty());
    }

    #[test]
    fn a_line_that_does_not_read_grants_nothing_and_loses_no_other() {
        let dir = tempfile::tempdir().unwrap();
        record(dir.path(), Path::new(PROJECT), "chat-a", &[key("one")]).unwrap();
        let mut f = OpenOptions::new()
            .append(true)
            .open(file_in(dir.path()))
            .unwrap();
        f.write_all(b"not json\n").unwrap();
        record(dir.path(), Path::new(PROJECT), "chat-a", &[key("two")]).unwrap();
        assert_eq!(own(dir.path(), PROJECT, "chat-a"), ["one", "two"]);
    }

    #[test]
    fn no_file_is_no_branch() {
        let dir = tempfile::tempdir().unwrap();
        assert!(own(dir.path(), PROJECT, "chat-a").is_empty());
    }
}
