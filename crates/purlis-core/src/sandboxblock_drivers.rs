// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Whether a repository's own config names a program a reading git command runs** (#2212).
//!
//! A line of readers merely printed what it handed back (#1972), and `git log`, `git status`
//! and the other subcommands in [`super::GIT_READERS`] are readers. A repository's own config
//! can make one of them run a program: an fsmonitor hook (`core.fsmonitor`), an external diff
//! (`diff.external`), a diff driver's `command` or `textconv`, a filter's `clean`, `smudge` or
//! `process`, a signature program (`gpg.program`, `gpg.<format>.program`,
//! `log.showSignature`), a `post-index-change` hook, which runs when a reader writes the index
//! it refreshed (in the hooks folder, the one `core.hooksPath` names, or a `hook.<name>` whose
//! `event` it is), and a promisor remote (`extensions.partialClone`,
//! `remote.<name>.promisor`), which fetches an object a reader asks for. A git reader in such
//! a repository is read as one that runs a program ([`super::may_run_a_program`]).
//!
//! **What is read.** The repository the folder stands in, found the way git finds it
//! ([`crate::gitconfig::discover`]: up from the folder to a `.git` folder or file); its
//! `config` and `config.worktree`, and its common git folder's `config` and `hooks` for a
//! linked work tree (`commondir`); and the files they include (`include.path`,
//! `includeIf.<condition>.path`, whatever the condition), [`MOST_INCLUDES_DEEP`] deep as git
//! itself. Each file is read only when it is a regular file of at most
//! [`crate::gitconfig::MAX_CONFIG_BYTES`], and at most [`MOST_FILES`] files are read for one
//! answer.
//!
//! **What cannot be read counts as naming a program**: the answer decides whether a refusal
//! is raised, and one that could not be told is raised. A walk to the repository that stops on
//! an error, a `.git` file that names no folder, a config or an include that is there but
//! cannot be read, is not a regular file or is larger than the bound, an include past git's
//! depth or past [`MOST_FILES`], and a `~/` include or hooks folder with no home known each
//! count. Only a file that is not there (git reads none either) and no `.git` up to the root
//! name none.

use std::io::Read;
use std::path::{Path, PathBuf};

use crate::gitconfig::{Discovered, MAX_CONFIG_BYTES, discover, pairs};

/// How many includes deep a config is read: git's own limit.
pub const MOST_INCLUDES_DEEP: usize = 10;

/// The most files read for one answer, includes and pointers among them.
pub const MOST_FILES: usize = 32;

/// The most of a pointer file (`commondir`) that is read.
const MAX_POINTER_BYTES: u64 = 4096;

/// The hook git runs when a reading command writes the index it refreshed.
const INDEX_HOOK: &str = "post-index-change";

/// **Whether the repository `folder` stands in names a program a git reader runs**, as the
/// module says. `home` is where a `~/` path resolves. A folder in no repository names none.
pub fn names_a_program(folder: &Path, home: Option<&Path>) -> bool {
    let Some(folder) = folder.to_str() else {
        return true;
    };
    match discover(folder) {
        Discovered::Repository { git_dir, top } => {
            git_dir_names_a_program(Path::new(&git_dir), Some(Path::new(&top)), home)
        }
        Discovered::Nothing => false,
        Discovered::Unknown => true,
    }
}

/// Whether the repository whose git folder is `git_dir`, found at the work tree's `top`
/// where that is known, names a program a git reader runs.
pub(crate) fn git_dir_names_a_program(
    git_dir: &Path,
    top: Option<&Path>,
    home: Option<&Path>,
) -> bool {
    let mut reading = Reading {
        home,
        files_left: MOST_FILES,
        entries: Vec::new(),
    };
    reading.program_named(git_dir, top).unwrap_or(true)
}

/// One answer's reading: what is left of its budget, and the config entries read so far.
struct Reading<'a> {
    home: Option<&'a Path>,
    files_left: usize,
    entries: Vec<Entry>,
}

/// One config entry: its section, subsection, key and value.
type Entry = (String, String, String, String);

/// What reading one file found.
enum Found {
    /// The file is not there.
    Missing,
    /// Its text.
    Text(String),
}

impl Reading<'_> {
    /// `Some(answer)`, or `None` where something could not be read.
    fn program_named(&mut self, git_dir: &Path, top: Option<&Path>) -> Option<bool> {
        let common = match self.read(&git_dir.join("commondir"), MAX_POINTER_BYTES)? {
            Found::Missing => git_dir.to_path_buf(),
            Found::Text(text) => match text.lines().next().unwrap_or_default().trim() {
                "" => git_dir.to_path_buf(),
                named => git_dir.join(named),
            },
        };
        let mut configs = vec![git_dir.join("config"), git_dir.join("config.worktree")];
        if common != git_dir {
            configs.push(common.join("config"));
        }
        for config in &configs {
            self.read_entries(config, 0)?;
        }
        let mut hooks = vec![common.join("hooks")];
        for (section, _, key, value) in &self.entries {
            if (section.as_str(), key.as_str()) == ("core", "hookspath") {
                hooks.push(self.hooks_folder(value, top)?);
            }
        }
        Some(
            hooks.iter().any(|folder| folder.join(INDEX_HOOK).is_file())
                || self
                    .entries
                    .iter()
                    .any(|(section, subsection, key, value)| {
                        names_a_program_by(section, subsection, key, value)
                    }),
        )
    }

    /// The folder `core.hooksPath` names, as git reads it: absolute, after `~/` in the home,
    /// or relative to the work tree's top; `None` where the home or the top is not known.
    fn hooks_folder(&self, value: &str, top: Option<&Path>) -> Option<PathBuf> {
        if let Some(rest) = value.strip_prefix("~/") {
            return self.home.map(|home| home.join(rest));
        }
        let named = Path::new(value);
        if named.is_absolute() {
            return Some(named.to_path_buf());
        }
        top.map(|top| top.join(named))
    }

    /// The entries of the config file at `path` and of the files it includes, `depth`
    /// includes deep; `None` where one could not be read.
    fn read_entries(&mut self, path: &Path, depth: usize) -> Option<()> {
        let Found::Text(text) = self.read(path, MAX_CONFIG_BYTES)? else {
            return Some(());
        };
        for (section, subsection, key, value) in pairs(&text) {
            // `[diff.pdf]` is git's older spelling of `[diff "pdf"]`.
            let (section, subsection) = match section.split_once('.') {
                Some((section, sub)) if subsection.is_empty() => {
                    (section.to_owned(), sub.to_owned())
                }
                _ => (section, subsection),
            };
            if key == "path" && (section == "include" || section == "includeif") {
                if value.is_empty() {
                    continue;
                }
                if depth + 1 > MOST_INCLUDES_DEEP {
                    return None;
                }
                let file = match value.strip_prefix("~/") {
                    Some(rest) => self.home?.join(rest),
                    None => path.parent()?.join(&value),
                };
                self.read_entries(&file, depth + 1)?;
                continue;
            }
            self.entries.push((section, subsection, key, value));
        }
        Some(())
    }

    /// The file at `path`, at most `limit` bytes of it; `None` where it is there but could not
    /// be read whole, is not a regular file, or the budget is spent.
    fn read(&mut self, path: &Path, limit: u64) -> Option<Found> {
        self.files_left = self.files_left.checked_sub(1)?;
        let meta = match std::fs::metadata(path) {
            Ok(meta) => meta,
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
                ) =>
            {
                return Some(Found::Missing);
            }
            Err(_) => return None,
        };
        if !meta.is_file() || meta.len() > limit {
            return None;
        }
        let mut raw = Vec::new();
        std::fs::File::open(path)
            .ok()?
            .take(limit + 1)
            .read_to_end(&mut raw)
            .ok()?;
        if raw.len() as u64 > limit {
            return None;
        }
        Some(Found::Text(String::from_utf8_lossy(&raw).into_owned()))
    }
}

/// Whether one config entry names a program a git reader runs, or a remote it fetches from.
fn names_a_program_by(section: &str, subsection: &str, key: &str, value: &str) -> bool {
    let sub = !subsection.is_empty();
    match (section, key) {
        // `true` is git's own fsmonitor daemon, and `false` none.
        ("core", "fsmonitor") => boolean(value).is_none(),
        ("hook", "event") => sub && value == INDEX_HOOK,
        ("diff", "external") => !sub,
        ("diff", "command" | "textconv") => sub,
        ("filter", "clean" | "smudge" | "process") => sub,
        ("gpg", "program") => true,
        ("log", "showsignature") => boolean(value) != Some(false),
        ("extensions", "partialclone") => true,
        ("remote", "promisor") => sub && boolean(value) != Some(false),
        _ => false,
    }
}

/// A config value as git's boolean, or `None` for one that is not.
fn boolean(value: &str) -> Option<bool> {
    match value.to_ascii_lowercase().as_str() {
        "true" | "yes" | "on" | "1" => Some(true),
        "false" | "no" | "off" | "0" | "" => Some(false),
        _ => None,
    }
}
