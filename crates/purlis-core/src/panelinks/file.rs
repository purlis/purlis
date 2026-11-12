// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A file path in a pane's output** (#6216, spec #6208 decision 7): `src/main.rs:42:7`,
//! `~/notes.md`, `/etc/hosts:3` and a `file:///` address, read into the file on this machine it
//! names, at its line and column.
//!
//! - **Only a file that is there links.** The text is joined to the chat's working folder (a
//!   relative path) or the home folder (`~/`), and every link on the way is resolved. What is
//!   left must be a regular file. Anything else is no link and says nothing on its hover
//!   ([`Resolved::Quiet`]): path-shaped words are everywhere in a harness's output.
//! - **Where it is read from** ([`Within`]): a branch's folder, a repo's own folder, a
//!   workspace's own files or the project's, by the names the light editor reads them by, or
//!   outside the project.
//! - **A file outside the chat's workspace asks first** ([`Opening::Ask`]): the project, for a
//!   chat at the project's root. Opening one only shows it, so a click never runs anything.
//! - **A `file://` address is a file link** under these same rules when it names this machine
//!   (`file:///…`, `file://localhost/…`, spec Q2). One naming another host is not, and says why.

use std::path::{Component, Path, PathBuf};

use super::{Kind, LinkKind, Opening, Pane, Resolved, Runs, Target};

/// **A file a path names**, as the core resolved it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileAt {
    /// The file, absolute, with every link on the way resolved.
    pub path: PathBuf,
    /// The line the text named, from 1.
    pub line: Option<u32>,
    /// The column the text named, from 1; only with a line.
    pub column: Option<u32>,
    /// Where the window reads it from.
    pub within: Within,
    /// Whether it is in the chat's workspace (the project, for a chat at its root): one that is
    /// not asks first.
    pub home: bool,
    /// The path in full with its `:line:col`, as a hover shows it and *Copy link* copies it.
    shown: String,
}

impl FileAt {
    /// The file in full with its `:line:col`.
    pub fn shown(&self) -> &str {
        &self.shown
    }
}

/// **Where a file is read from**: the place the light editor reads it by, and its path there
/// (`/` between its parts), or outside the project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Within {
    /// A branch's folder, or with no piece the repo's own folder (its clone).
    Branch {
        ws: String,
        repo: String,
        piece: Option<String>,
        path: String,
    },
    /// A workspace's own files: its folder, but its repos and their branches' folders.
    Workspace { ws: String, path: String },
    /// The project's own files: its folder, but its workspaces.
    Project { path: String },
    /// Not in the project: read only once the person said to open it.
    Outside,
}

/// **The folder a chat counts as its own**: its workspace's, or the project's for a chat at the
/// project's root. A file anywhere else asks first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Home {
    /// What it is called in a sentence: the workspace's name, or "the project".
    pub name: String,
    /// Its folder, links resolved.
    pub folder: PathBuf,
}

/// **The home of `pane`'s chat**: its workspace, or the project when it works at the project's
/// root or in no workspace of it. `None` for a pane in no project.
pub fn home_of(pane: &Pane) -> Option<Home> {
    let project = std::fs::canonicalize(pane.project.as_ref()?).ok()?;
    let in_a_workspace = pane
        .folder
        .as_deref()
        .and_then(|folder| crate::active::workspace_of_tree(&project, folder));
    Some(match in_a_workspace {
        Some(ws) => Home {
            folder: project.join("workspaces").join(&ws),
            name: ws,
        },
        None => Home {
            name: "the project".into(),
            folder: project,
        },
    })
}

/// **A file path** (#6216): opened in the light editor, at its line, never by the system.
pub struct LocalFile {
    /// The home folder a `~/` path starts from; `None` where the environment names none.
    home: Option<PathBuf>,
}

impl LocalFile {
    /// Paths read with `home` as the home folder.
    pub fn with_home(home: Option<PathBuf>) -> Self {
        Self { home }
    }

    /// Paths read with the person's own home folder.
    pub fn here() -> Self {
        Self::with_home(crate::profiles::home())
    }
}

impl LinkKind for LocalFile {
    fn kind(&self) -> Kind {
        Kind::File
    }

    fn resolve(&self, text: &str, pane: &Pane) -> Resolved {
        if text.is_empty() || text.chars().any(|c| c.is_whitespace() || c.is_control()) {
            return Resolved::Quiet;
        }
        let (words, line, column) = at_line(text);
        let named = match words.strip_prefix("file:") {
            Some(_) => match on_this_machine(words) {
                Ok(path) => path,
                Err(why) => return why,
            },
            None => match self.joined(words, pane) {
                Some(path) => path,
                None => return Resolved::Quiet,
            },
        };
        // A chat on a runner names its own machine's files: mapped through its mounts (#6221),
        // never read as this one's.
        if pane.runs != Runs::Here {
            return Resolved::Quiet;
        }
        let Ok(path) = std::fs::canonicalize(&named) else {
            return Resolved::Quiet;
        };
        if !std::fs::metadata(&path).is_ok_and(|meta| meta.is_file()) {
            return Resolved::Quiet;
        }
        let Some(within) = within(pane, &path) else {
            return Resolved::Quiet;
        };
        let home = home_of(pane).is_some_and(|home| path.starts_with(&home.folder));
        let mut shown = path.display().to_string();
        if let Some(line) = line {
            shown.push_str(&format!(":{line}"));
        }
        if let Some(column) = column {
            shown.push_str(&format!(":{column}"));
        }
        Resolved::Link(Target::File(Box::new(FileAt {
            path,
            line,
            column,
            within,
            home,
            shown,
        })))
    }

    fn opening(&self, target: Target, text: &str, pane: &Pane) -> Opening {
        let Target::File(file) = &target else {
            return Opening::Refused("purlis does not open that as a file".into());
        };
        if file.home {
            return Opening::Open(target);
        }
        let name = home_of(pane).map_or_else(|| "the project".to_owned(), |home| home.name);
        Opening::Ask {
            reason: format!("outside {name}"),
            shown: text.to_owned(),
            target,
        }
    }
}

impl LocalFile {
    /// `words` as a path on this machine: from `/`, from the home folder for `~/`, and from the
    /// chat's working folder otherwise. `None` where there is no folder to start from.
    fn joined(&self, words: &str, pane: &Pane) -> Option<PathBuf> {
        if let Some(rest) = words.strip_prefix("~/") {
            return Some(self.home.as_ref()?.join(rest));
        }
        // `~name/` is another person's home, which purlis does not look up.
        if words.starts_with('~') {
            return None;
        }
        let path = Path::new(words);
        if path.is_absolute() {
            return Some(path.to_path_buf());
        }
        Some(pane.folder.as_ref()?.join(path))
    }
}

/// `text` without its `:line[:col]`, and the line and column it named. A line of 0 is none, and
/// so is a column without one.
fn at_line(text: &str) -> (&str, Option<u32>, Option<u32>) {
    let number = |part: &str| {
        (!part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
            .then(|| part.parse::<u32>().ok())
            .flatten()
            .filter(|n| *n > 0)
    };
    let Some((before, last)) = text.rsplit_once(':') else {
        return (text, None, None);
    };
    let Some(last) = number(last) else {
        return (text, None, None);
    };
    if let Some((path, line)) = before.rsplit_once(':')
        && let Some(line) = number(line)
        && !path.is_empty()
    {
        return (path, Some(line), Some(last));
    }
    if before.is_empty() {
        return (text, None, None);
    }
    (before, Some(last), None)
}

/// A `file:` address as the path it names on this machine (spec Q2): `file:///…` and
/// `file://localhost/…`. One naming another host is no link, and says so.
fn on_this_machine(words: &str) -> Result<PathBuf, Resolved> {
    let Ok(url) = url::Url::parse(words) else {
        return Err(Resolved::Quiet);
    };
    match url.host_str() {
        None | Some("") | Some("localhost") => {}
        Some(host) => {
            return Err(Resolved::NotALink(format!(
                "purlis opens only this machine's files, and this file: address names {host}"
            )));
        }
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err(Resolved::Quiet);
    }
    // `to_file_path` refuses a host it would hand on; `localhost` is this machine.
    let mut local = url.clone();
    if local.set_host(None).is_err() {
        return Err(Resolved::Quiet);
    }
    local.to_file_path().map_err(|()| Resolved::Quiet)
}

/// **Where `path` (resolved) is read from**, in `pane`'s project. `None` for a path the window
/// could not name: one whose parts are not text.
fn within(pane: &Pane, path: &Path) -> Option<Within> {
    let Some(project) = pane
        .project
        .as_ref()
        .and_then(|project| std::fs::canonicalize(project).ok())
    else {
        return Some(Within::Outside);
    };
    let Ok(in_project) = path.strip_prefix(&project) else {
        return Some(Within::Outside);
    };
    let under = |folder: &Path| -> Option<String> {
        let folder = std::fs::canonicalize(folder).ok()?;
        slashed(path.strip_prefix(folder).ok()?)
    };
    if let Some(at) = crate::worktree::locate(&project, path) {
        let folder =
            crate::worktree::path_for(&project, &at.workspace, &at.repo, &at.piece).ok()?;
        return Some(Within::Branch {
            path: under(&folder)?,
            ws: at.workspace,
            repo: at.repo,
            piece: Some(at.piece),
        });
    }
    if let Some((ws, repo)) = crate::worktree::locate_clone(&project, path) {
        let folder = crate::repofolder::folder(&project, &ws, &repo).ok()?;
        return Some(Within::Branch {
            path: under(&folder)?,
            ws,
            repo,
            piece: None,
        });
    }
    if let Some(ws) = crate::active::workspace_of_tree(&project, path) {
        return Some(Within::Workspace {
            path: under(&project.join("workspaces").join(&ws))?,
            ws,
        });
    }
    Some(Within::Project {
        path: slashed(in_project)?,
    })
}

/// `path`'s parts with `/` between them, or `None` where one is not text or it is empty.
fn slashed(path: &Path) -> Option<String> {
    let parts: Option<Vec<&str>> = path
        .components()
        .map(|part| match part {
            Component::Normal(name) => name.to_str(),
            _ => None,
        })
        .collect();
    let parts = parts?;
    (!parts.is_empty()).then(|| parts.join("/"))
}

#[cfg(test)]
#[path = "file_tests.rs"]
mod tests;
