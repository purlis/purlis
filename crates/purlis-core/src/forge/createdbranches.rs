// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The branches a chat made through the forge route**, the app's own record (P0b).
//!
//! A chat may move (force-push) only a branch it created: one the forge confirmed it made, in a
//! push that went through the route. That fact is kept here, in the machine config root, which no
//! chat's sandbox can write, and nowhere a chat can influence: not the piece log (a chat can
//! write a `claimed` line into it), not a clone's refs (a chat can delete or never fetch them).
//!
//! One line of JSON per creation, appended: `{"chat","host","repo","branch"}`, keyed by the
//! creating chat's stable id. **A record is never inherited**: a chat with another id (a resumed
//! chat gets one) owns none of them, and the route refuses every update it makes to such a branch:
//! the person pushes it. A line that does not read is skipped, which only ever loses ownership,
//! never grants it.

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
    chat: String,
    host: String,
    repo: String,
    branch: String,
}

/// Records that `chat` made each of `created` (`(host, repo path, branch)`, as the policy keys
/// them). One `write` per line to a file opened for append, so concurrent routes do not tear it.
pub fn record(
    config_root: &Path,
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

/// The branches of `repo` on `host` that `chat` itself made. Any doubt (no file, an unreadable
/// one) is none.
pub fn created_by(config_root: &Path, chat: &str, host: &str, repo: &str) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(file_in(config_root)) else {
        return Vec::new();
    };
    let mut out: Vec<String> = Vec::new();
    for line in text.lines() {
        let Ok(line) = serde_json::from_str::<Line>(line) else {
            continue;
        };
        if line.chat == chat
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

    fn key(branch: &str) -> (String, String, String) {
        ("github.com".into(), "o/r".into(), branch.into())
    }

    #[test]
    fn a_branch_is_the_creating_chats_own_and_no_other_chats() {
        let dir = tempfile::tempdir().unwrap();
        record(dir.path(), "chat-a", &[key("work")]).unwrap();
        assert_eq!(
            created_by(dir.path(), "chat-a", "github.com", "o/r"),
            ["work"]
        );
        assert!(created_by(dir.path(), "chat-b", "github.com", "o/r").is_empty());
    }

    #[test]
    fn a_branch_is_own_only_in_the_repo_and_host_it_was_made_in() {
        let dir = tempfile::tempdir().unwrap();
        record(dir.path(), "chat-a", &[key("work")]).unwrap();
        assert!(created_by(dir.path(), "chat-a", "github.com", "o/other").is_empty());
        assert!(created_by(dir.path(), "chat-a", "gitlab.com", "o/r").is_empty());
    }

    #[test]
    fn a_line_that_does_not_read_grants_nothing_and_loses_no_other() {
        let dir = tempfile::tempdir().unwrap();
        record(dir.path(), "chat-a", &[key("one")]).unwrap();
        let mut f = OpenOptions::new()
            .append(true)
            .open(file_in(dir.path()))
            .unwrap();
        f.write_all(b"not json\n").unwrap();
        record(dir.path(), "chat-a", &[key("two")]).unwrap();
        assert_eq!(
            created_by(dir.path(), "chat-a", "github.com", "o/r"),
            ["one", "two"]
        );
    }

    #[test]
    fn no_file_is_no_branch() {
        let dir = tempfile::tempdir().unwrap();
        assert!(created_by(dir.path(), "chat-a", "github.com", "o/r").is_empty());
    }
}
