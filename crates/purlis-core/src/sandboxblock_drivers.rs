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
//! `event` it is), and a promisor remote (`extensions.partialClone`, `remote.<name>.promisor`), which
//! fetches an object a reader asks for. A git reader in such a repository is read as one that
//! runs a program ([`super::may_run_a_program`]).
//!
//! **What is read.** The repository the folder stands in, found the way git finds it
//! ([`crate::gitconfig::git_dir_at`]: up from the folder to a `.git` folder or file); its
//! `config` and `config.worktree`, and its common git folder's `config` and `hooks` for a
//! linked work tree (`commondir`); and the files they include (`include.path`,
//! `includeIf.<condition>.path`, whatever the condition), to [`MOST_INCLUDES_DEEP`]. Each file
//! is read only when it is a regular file, and at most [`crate::gitconfig::MAX_CONFIG_BYTES`]
//! of it.
//!
//! A handful of small files, read only for a line that runs a git reader: cheap enough for
//! the hook that runs after every command.

use std::path::{Path, PathBuf};

use crate::gitconfig::{MAX_CONFIG_BYTES, git_dir_at, pairs, read_bounded};

/// How many includes deep a config is read for a driver.
pub const MOST_INCLUDES_DEEP: usize = 4;

/// The hook git runs when a reading command writes the index it refreshed.
const INDEX_HOOK: &str = "post-index-change";

/// **Whether the repository `folder` stands in names a program a git reader runs**, as the
/// module says. `home` is where a `~/` include resolves. A folder in no repository names none.
pub fn names_a_program(folder: &Path, home: Option<&Path>) -> bool {
    folder
        .to_str()
        .and_then(git_dir_at)
        .is_some_and(|git_dir| git_dir_names_a_program(Path::new(&git_dir), home))
}

/// Whether the repository whose git folder is `git_dir` names a program a git reader runs.
pub(crate) fn git_dir_names_a_program(git_dir: &Path, home: Option<&Path>) -> bool {
    let git_dir = git_dir.to_path_buf();
    let common = common_dir(&git_dir);
    let mut configs = vec![git_dir.join("config"), git_dir.join("config.worktree")];
    if common != git_dir {
        configs.push(common.join("config"));
    }
    let mut entries = Vec::new();
    for config in &configs {
        read_entries(config, home, 0, &mut entries);
    }
    let mut hooks = vec![common.join("hooks")];
    for (section, _, key, value) in &entries {
        if (section.as_str(), key.as_str()) == ("core", "hookspath") {
            match hooks_folder(&git_dir, &common, value, home) {
                Some(folder) => hooks.push(folder),
                None => return true,
            }
        }
    }
    hooks.iter().any(|folder| folder.join(INDEX_HOOK).is_file())
        || entries.iter().any(|(section, subsection, key, value)| {
            names_a_program_by(section, subsection, key, value)
        })
}

/// The folder `core.hooksPath` names, as git reads it: absolute, after `~/` in `home`, or
/// relative to the work tree's top; `None` where that top is not known.
fn hooks_folder(
    git_dir: &Path,
    common: &Path,
    value: &str,
    home: Option<&Path>,
) -> Option<PathBuf> {
    if let Some(rest) = value.strip_prefix("~/") {
        return home.map(|home| home.join(rest));
    }
    let named = Path::new(value);
    if named.is_absolute() {
        return Some(named.to_path_buf());
    }
    // A linked work tree's git folder names the `.git` file at its top in `gitdir`.
    let top = if git_dir == common {
        (git_dir.file_name()? == ".git")
            .then(|| git_dir.parent())??
            .to_path_buf()
    } else {
        let raw = read_bounded(&git_dir.join("gitdir").to_string_lossy(), 4096)?;
        let dot = String::from_utf8_lossy(&raw)
            .lines()
            .next()?
            .trim()
            .to_owned();
        Path::new(&dot).parent()?.to_path_buf()
    };
    Some(top.join(named))
}

/// The common git folder a linked work tree's git folder names in its `commondir`, or the git
/// folder itself.
fn common_dir(git_dir: &Path) -> PathBuf {
    let Some(raw) = read_bounded(&git_dir.join("commondir").to_string_lossy(), 4096) else {
        return git_dir.to_path_buf();
    };
    let named = String::from_utf8_lossy(&raw);
    let named = named.lines().next().unwrap_or_default().trim();
    if named.is_empty() {
        git_dir.to_path_buf()
    } else {
        git_dir.join(named)
    }
}

/// One config entry: its section, subsection, key and value.
type Entry = (String, String, String, String);

/// The entries of the config file at `path` and of the files it includes, into `out`.
fn read_entries(path: &Path, home: Option<&Path>, depth: usize, out: &mut Vec<Entry>) {
    let Some(raw) = read_bounded(&path.to_string_lossy(), MAX_CONFIG_BYTES) else {
        return;
    };
    let text = String::from_utf8_lossy(&raw);
    for (section, subsection, key, value) in pairs(&text) {
        // `[diff.pdf]` is git's older spelling of `[diff "pdf"]`.
        let (section, subsection) = match section.split_once('.') {
            Some((section, sub)) if subsection.is_empty() => (section.to_owned(), sub.to_owned()),
            _ => (section, subsection),
        };
        if key == "path" && (section == "include" || section == "includeif") {
            if depth < MOST_INCLUDES_DEEP
                && let Some(file) = included(path, &value, home)
            {
                read_entries(&file, home, depth + 1, out);
            }
            continue;
        }
        out.push((section, subsection, key, value));
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

/// The file an include of `value` in the config at `config` names: relative to that config's
/// folder, or to `home` after `~/`.
fn included(config: &Path, value: &str, home: Option<&Path>) -> Option<PathBuf> {
    if value.is_empty() {
        return None;
    }
    if let Some(rest) = value.strip_prefix("~/") {
        return home.map(|home| home.join(rest));
    }
    Some(config.parent()?.join(value))
}
