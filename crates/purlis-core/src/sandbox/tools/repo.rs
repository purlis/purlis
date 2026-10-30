// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The repository configuration a declared tool's run reads** (#3868): only the person's.
//!
//! A run starts in the chat's folder with an environment of its own (#3419), but git, and the
//! forge command lines through it, still read the repository that folder is in: its config, the
//! files that config includes, its hooks, and glab's own settings kept in its git folder. A
//! chat that could write any of them could have the run start its code beside the tool's value.
//! So a run goes ahead only where each of them is out of a chat's reach: outside every folder a
//! chat may write, or in the project, where class 5 holds a `.git` folder's config and hooks
//! from every chat (ADR 0067 §5), and never in a temporary folder, where nothing holds them.
//! Judged before any value is read, as a run's arguments are ([`super::allows`]).

use std::path::{Component, Path, PathBuf};

/// The deepest a config's includes are followed, as git's own limit.
const DEEPEST: usize = 10;

/// **Whether a run in `folder` reads only repository configuration the person controls**, for
/// a project at `root` whose chats may write `writable` (and the temporary folders), or the
/// sentence naming what a chat could have written. `glab` says the run's program is glab,
/// which also reads its own settings from the git folder.
pub fn held(folder: &Path, root: &Path, writable: &[PathBuf], glab: bool) -> Result<(), String> {
    let reach = Reach { root, writable };
    let Some(found) = repository_of(folder)? else {
        return Ok(());
    };
    let mut git_folders = vec![found.git.clone()];
    if let Some(common) = common_of(&found.git) {
        git_folders.push(common);
    }
    for git in &git_folders {
        reach.held(git, "its git folder")?;
        if glab && git.join("glab-cli").exists() {
            return Err(refused(
                &git.join("glab-cli"),
                "holds glab's own settings for the repository, which a chat can write",
            ));
        }
    }
    let common = git_folders.last().cloned().unwrap_or(found.git.clone());
    for config in [common.join("config"), found.git.join("config.worktree")] {
        reach.config(&config, &found.top, 0)?;
    }
    Ok(())
}

/// The folders a chat may write, as a run judges a repository's files.
struct Reach<'a> {
    root: &'a Path,
    writable: &'a [PathBuf],
}

impl Reach<'_> {
    /// Whether `path` is out of a chat's reach: outside every folder it may write, or in the
    /// project, out of the temporary folders, where class 5 holds it.
    fn held(&self, path: &Path, what: &str) -> Result<(), String> {
        let path = lexical(path);
        let temps = super::temp_folders();
        let mut written: Vec<PathBuf> = vec![self.root.to_path_buf()];
        written.extend(self.writable.iter().cloned());
        written.extend(temps.iter().cloned());
        if !super::inside_any(&path, &written) {
            return Ok(());
        }
        if super::inside_any(&path, &[self.root.to_path_buf()])
            && !super::inside_any(&path, &temps)
            && held_by_class_5(&path)
        {
            return Ok(());
        }
        Err(refused(
            &path,
            &format!("is {what}, in a folder a chat may write"),
        ))
    }

    /// Each file `config` includes, and the hooks folder it names, held ([`Self::held`]), and
    /// the files it includes judged in turn. `top` is the folder a relative hooks folder is
    /// read from.
    fn config(&self, config: &Path, top: &Path, depth: usize) -> Result<(), String> {
        let Ok(text) = std::fs::read_to_string(config) else {
            return Ok(());
        };
        if depth > DEEPEST {
            return Err(refused(
                config,
                "includes more configs in turn than git reads",
            ));
        }
        let beside = config.parent().unwrap_or(Path::new("/"));
        let mut section = String::new();
        for line in text.lines() {
            let line = line.trim();
            if let Some(header) = line.strip_prefix('[') {
                section = header
                    .split([']', ' ', '\t', '"', '.'])
                    .next()
                    .unwrap_or_default()
                    .to_ascii_lowercase();
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim().to_ascii_lowercase();
            let value = unquoted(value);
            // `~` is the run's own home, which holds nothing of the chat's.
            if value.is_empty() || value.starts_with('~') {
                continue;
            }
            if (section == "include" || section == "includeif") && key == "path" {
                let included = beside.join(&value);
                self.held(&included, "a config the repository's includes")?;
                self.config(&included, top, depth + 1)?;
            } else if section == "core" && key == "hookspath" {
                self.held(&top.join(&value), "the repository's hooks folder")?;
            }
        }
        Ok(())
    }
}

/// Whether `path` is where class 5 holds a repository's settings from every chat in the
/// project: a `.git` folder itself, its `config` or `config.worktree`, its hooks, or a branch
/// folder's git folder under its `worktrees`.
fn held_by_class_5(path: &Path) -> bool {
    let parts: Vec<&std::ffi::OsStr> = path
        .components()
        .filter_map(|part| match part {
            Component::Normal(name) => Some(name),
            _ => None,
        })
        .collect();
    let Some(at) = parts.iter().rposition(|part| *part == ".git") else {
        return false;
    };
    match parts.get(at + 1).and_then(|part| part.to_str()) {
        None => true,
        Some("config" | "config.worktree") => parts.len() == at + 2,
        Some("hooks" | "worktrees") => true,
        Some(_) => false,
    }
}

/// A repository found from a folder: its git folder, and the folder its relative hooks folder
/// is read from (its work tree's top, or the git folder of a bare one).
struct Found {
    git: PathBuf,
    top: PathBuf,
}

/// **The repository `folder` is in, as git finds it**: the nearest folder at or above it that
/// is a git folder itself (a bare repository, or inside a `.git`), or holds a `.git` folder, or
/// a `.git` file naming one. A git folder found bare is judged as one whatever git's
/// `safe.bareRepository` says.
fn repository_of(folder: &Path) -> Result<Option<Found>, String> {
    for at in folder.ancestors() {
        if is_git_folder(at) {
            return Ok(Some(Found {
                git: at.to_path_buf(),
                top: at.to_path_buf(),
            }));
        }
        let dot = at.join(".git");
        let Ok(meta) = std::fs::symlink_metadata(&dot) else {
            continue;
        };
        if meta.is_dir() {
            return Ok(Some(Found {
                git: dot,
                top: at.to_path_buf(),
            }));
        }
        let text = std::fs::read_to_string(&dot)
            .map_err(|_| refused(&dot, "is a `.git` that purlis cannot read"))?;
        let Some(named) = text
            .lines()
            .next()
            .and_then(|line| line.strip_prefix("gitdir:"))
        else {
            return Err(refused(&dot, "is a `.git` file that names no git folder"));
        };
        return Ok(Some(Found {
            git: lexical(&at.join(named.trim())),
            top: at.to_path_buf(),
        }));
    }
    Ok(None)
}

/// Whether `folder` is a git folder: it holds `HEAD`, `objects` and `refs`.
fn is_git_folder(folder: &Path) -> bool {
    folder.join("HEAD").is_file() && folder.join("objects").is_dir() && folder.join("refs").is_dir()
}

/// The common git folder a branch folder's git folder `git` names in its `commondir`.
fn common_of(git: &Path) -> Option<PathBuf> {
    let named = std::fs::read_to_string(git.join("commondir")).ok()?;
    Some(lexical(&git.join(named.trim())))
}

/// `path` with its `.` and `..` resolved by name, as git reads a path it is given.
fn lexical(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

/// A config value without its quotes and any comment after it.
fn unquoted(value: &str) -> String {
    let value = value.trim();
    if let Some(inner) = value.strip_prefix('"') {
        return inner.split('"').next().unwrap_or_default().to_owned();
    }
    value
        .split([';', '#'])
        .next()
        .unwrap_or_default()
        .trim()
        .to_owned()
}

/// The sentence a refused run is answered with.
fn refused(path: &Path, why: &str) -> String {
    format!(
        "{} {why}, so a run with a vault's value never starts here, where a chat could have \
         it run code of its own beside the value. Run it in the project's own clone, in a folder \
         outside any repository, or in your own terminal, where purlis limits nothing.",
        path.display()
    )
}
