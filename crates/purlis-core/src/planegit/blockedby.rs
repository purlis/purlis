// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Where the next save is blocked by what looks like a credential, as a window says it
//! (ADR 0051).
//!
//! The standing said only [`super::SECRET_REFUSED`], "a secret-shaped value in a staged file",
//! so the title bar and the Inbox could say no more, and the person had to ask a chat which
//! file it was. This names each changed file the save would refuse: its path, the line, the
//! kind of value, and what the scan matched with the value hidden. **Never the value**: what is
//! here is drawn in a window and sent to every reader of the standing.
//!
//! The question is the standing's own ([`super::secret_on_disk`]), so a file is named here
//! exactly when it blocks. The file and its name are whatever a chat wrote: a path is said
//! through [`shown::readable`], and [`file_of`] opens only a file this reading names.

use std::path::{Path, PathBuf};

use crate::{secretshape, shown};

/// One changed file the next save would refuse, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretAt {
    /// From the top of the project, as git names it.
    pub path: String,
    /// The 1-based line the value starts on, when the text as written spells it; `None` for
    /// one read only through the file's escapes.
    pub line: Option<usize>,
    /// The kind of value, in the scan's words (`credential assignment`, `JWT`).
    pub kind: &'static str,
    /// What the scan matched, with the value hidden ([`secretshape::found_and_shown`]).
    pub shown: Option<String>,
}

/// The kind said for a file that blocked a moment ago and reads otherwise now: it changed
/// between the two reads, and the next standing says which it is.
const CHANGING: &str = "a secret-shaped value";

/// Each of `changed` the next save would refuse, in their order. A file is listed whenever
/// the standing's own question says it blocks, whatever the second read finds, so this never
/// names fewer files than block.
pub(super) fn among(root: &Path, changed: &[String]) -> Vec<SecretAt> {
    changed
        .iter()
        .filter(|path| super::secret_on_disk(root, path))
        .map(|path| {
            let read = read(root, path);
            SecretAt {
                path: path.clone(),
                line: read.as_ref().and_then(|read| read.line),
                kind: read.as_ref().map_or(CHANGING, |read| read.kind),
                shown: read.and_then(|read| read.shown),
            }
        })
        .collect()
}

/// What the scan reads in the file at `path` now, as [`super::secret_on_disk`] reads it.
fn read(root: &Path, path: &str) -> Option<secretshape::Read> {
    let bytes = std::fs::read(root.join(path)).ok()?;
    if super::unscanned(path, &bytes).is_some() {
        return None;
    }
    secretshape::read_and_shown(
        secretshape::Structured::of(path),
        &String::from_utf8_lossy(&bytes),
    )
}

/// The most characters of a file's name said in a sentence; a longer one keeps its head and
/// its tail, where its extension is.
const NAME_SHOWN: usize = 40;

/// `path` for a sentence in a title bar: its first folder and its file name, with what is
/// between them left out (`personas/…/note.md`), and a long name cut in its middle.
pub fn short_path(path: &str) -> String {
    let parts: Vec<&str> = path.split('/').collect();
    let name = parts.last().copied().unwrap_or_default();
    let chars: Vec<char> = name.chars().collect();
    let name = if chars.len() > NAME_SHOWN {
        let head: String = chars[..NAME_SHOWN - 13].iter().collect();
        let tail: String = chars[chars.len() - 12..].iter().collect();
        format!("{}…{}", shown::short(&head), shown::short(&tail))
    } else {
        shown::short(name)
    };
    match parts.as_slice() {
        [] | [_] => name,
        [folder, _] => format!("{}/{name}", shown::short(folder)),
        [folder, ..] => format!("{}/…/{name}", shown::short(folder)),
    }
}

/// Why the save is blocked, naming the first file: "a secret-shaped value in
/// personas/…/note.md, line 5", and how many more files there are. `None` for no finding.
pub fn said(secrets: &[SecretAt]) -> Option<String> {
    let first = secrets.first()?;
    let line = first
        .line
        .map(|line| format!(", line {line}"))
        .unwrap_or_default();
    let more = match secrets.len() - 1 {
        0 => String::new(),
        1 => " and 1 more file".to_owned(),
        n => format!(" and {n} more files"),
    };
    Some(format!(
        "a secret-shaped value in {}{line}{more}",
        short_path(&first.path)
    ))
}

/// The file `path` names among `secrets`, and the line to open it at: the finding's own, or
/// the first. Refused for a path no finding names, so a window opens only what this reading
/// of the project found; and for a finding that is no longer a file of its own inside the
/// project (a link put in its place since).
pub fn file_of(root: &Path, secrets: &[SecretAt], path: &str) -> Result<(PathBuf, u32), String> {
    let Some(found) = secrets.iter().find(|one| one.path == path) else {
        return Err(format!(
            "purlis opens only a file the save is blocked in, and {} is not one now",
            shown::short(path)
        ));
    };
    let file = root.join(&found.path);
    let a_file = std::fs::symlink_metadata(&file).is_ok_and(|meta| meta.is_file());
    if !a_file || !crate::contain::within_plane(root, &file) {
        return Err(format!(
            "{} is not a file of this project now",
            shown::short(path)
        ));
    }
    let line = found
        .line
        .and_then(|line| u32::try_from(line).ok())
        .unwrap_or(1);
    Ok((file, line))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("a directory");
        for (path, text) in files {
            let file = dir.path().join(path);
            std::fs::create_dir_all(file.parent().expect("a folder")).expect("a folder");
            std::fs::write(file, text).expect("written");
        }
        dir
    }

    fn paths(all: &[&str]) -> Vec<String> {
        all.iter().map(|path| (*path).to_owned()).collect()
    }

    #[test]
    fn each_file_the_save_would_refuse_is_named_with_its_line_and_kind_and_no_value() {
        let dir = project(&[
            ("notes/clean.md", "# a note\n\nnothing here\n"),
            (
                "personas/devops/memory/login.md",
                "# login\n\nline three\n\npassword: hunter2is\n",
            ),
            // The shape a persona's note held when its project could not be saved: names only.
            (
                "personas/devops/memory/names.md",
                "the client reads `token = VOLATICLOUD_MCP_TOKEN` from the vault\n",
            ),
        ]);
        let found = among(
            dir.path(),
            &paths(&[
                "notes/clean.md",
                "personas/devops/memory/login.md",
                "personas/devops/memory/names.md",
            ]),
        );
        assert_eq!(
            found,
            vec![SecretAt {
                path: "personas/devops/memory/login.md".into(),
                line: Some(5),
                kind: "credential assignment",
                shown: Some("password = ********".into()),
            }]
        );
        assert!(!format!("{found:?}").contains("hunter2is"));
    }

    #[test]
    fn a_key_is_shown_masked_as_a_refused_commit_shows_one() {
        let key = ["deploy with ghp", "_0123456789abcdefABCDEF\n"].concat();
        let dir = project(&[("notes/deploy.md", &key)]);
        let found = among(dir.path(), &paths(&["notes/deploy.md"]));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].line, Some(1));
        let shown = found[0].shown.as_deref().expect("shown");
        assert!(shown.starts_with("ghp_*"), "{shown}");
        assert!(!shown.contains("abcdefABCDEF"), "{shown}");
    }

    #[test]
    fn the_sentence_names_the_first_file_shortly_and_counts_the_rest() {
        let at = |path: &str, line: Option<usize>| SecretAt {
            path: path.to_owned(),
            line,
            kind: "credential assignment",
            shown: None,
        };
        assert_eq!(said(&[]), None);
        assert_eq!(
            said(&[at("personas/devops/memory/note.md", Some(5))]).as_deref(),
            Some("a secret-shaped value in personas/…/note.md, line 5")
        );
        assert_eq!(
            said(&[at("charter.json", None), at("b.md", Some(1))]).as_deref(),
            Some("a secret-shaped value in charter.json and 1 more file")
        );
        assert_eq!(
            said(&[at("notes/a.md", Some(2)), at("b", None), at("c", None)]).as_deref(),
            Some("a secret-shaped value in notes/a.md, line 2 and 2 more files")
        );
    }

    #[test]
    fn a_long_name_keeps_its_head_and_its_extension_and_a_strange_one_is_escaped() {
        let long = "personas/devops/memory/the-volaticloud-mcp-keycloak-client-is-now-tofu-.md";
        assert_eq!(
            short_path(long),
            "personas/…/the-volaticloud-mcp-keycloa…now-tofu-.md"
        );
        // A name is whatever a chat wrote: a control character is said, never drawn.
        let strange = short_path("notes/a\u{1b}[31m.md");
        assert!(!strange.contains('\u{1b}'), "{strange:?}");
    }

    #[test]
    fn only_a_file_a_finding_names_is_opened_and_at_its_line() {
        let dir = project(&[
            ("notes/login.md", "\n\npassword: hunter2is\n"),
            ("notes/other.md", "nothing\n"),
            ("escaped.json", "{}\n"),
        ]);
        let mut secrets = among(dir.path(), &paths(&["notes/login.md", "notes/other.md"]));
        secrets.push(SecretAt {
            path: "escaped.json".into(),
            line: None,
            kind: "JWT",
            shown: None,
        });
        assert_eq!(
            file_of(dir.path(), &secrets, "notes/login.md"),
            Ok((dir.path().join("notes/login.md"), 3))
        );
        // A finding read through its escapes opens at the top.
        assert_eq!(
            file_of(dir.path(), &secrets, "escaped.json"),
            Ok((dir.path().join("escaped.json"), 1))
        );
        for not_named in ["notes/other.md", "../outside.md", "/etc/hosts", ""] {
            let refused = file_of(dir.path(), &secrets, not_named).unwrap_err();
            assert!(
                refused.contains("only a file the save is blocked in"),
                "{refused}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_finding_that_became_a_link_or_went_is_not_opened() {
        let dir = project(&[("notes/login.md", "password: hunter2is\n")]);
        let secrets = among(dir.path(), &paths(&["notes/login.md"]));
        let file = dir.path().join("notes/login.md");
        std::fs::remove_file(&file).expect("removed");
        assert!(file_of(dir.path(), &secrets, "notes/login.md").is_err());
        std::os::unix::fs::symlink("/etc/hosts", &file).expect("a link");
        let refused = file_of(dir.path(), &secrets, "notes/login.md").unwrap_err();
        assert!(refused.contains("not a file of this project"), "{refused}");
    }
}
