// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The repository configuration a declared tool's run reads** (#3868): only the person's.
//!
//! A run starts in the chat's folder with an environment of its own (#3419), but git, and the
//! forge command lines through it, still read the repository that folder is in: its config, the
//! files that config includes, its hooks, and glab's own settings kept in its git folder. A
//! chat that could write any of them could have the run start its code beside the tool's value.
//! So a run goes ahead only where each of them is out of a chat's reach: outside every folder a
//! chat may write, or in the project, where class 5 holds them from every chat (ADR 0067 §5),
//! and never in a temporary folder, where nothing holds them. And git is then held to the
//! repository judged ([`Pin`]): a folder a chat swaps, or a `.git` it plants, after the
//! judgement is never the one git reads. Judged before any value is read, as a run's arguments
//! are ([`super::allows`]).

use std::path::{Component, Path, PathBuf};

/// The deepest a config's includes are followed, as git's own limit.
const DEEPEST: usize = 10;

/// The files of a git folder git reads settings or code from, which class 5 holds.
const SETTINGS: &[&str] = &[
    "config",
    "config.worktree",
    "hooks",
    "commondir",
    "info/attributes",
    "objects/info/alternates",
];

/// **The repository a run's git is held to** ([`held`]): the git folder judged and its work
/// tree, or none where the run's folder is in no repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pin {
    git: Option<PathBuf>,
    work_tree: Option<PathBuf>,
}

impl Pin {
    /// **The variables the run starts with**, so git reads the repository judged and no other:
    /// `GIT_DIR` and `GIT_WORK_TREE` naming it; or `GIT_DIR` naming nothing where there is no
    /// repository, or where the run `clones` (git would make the new repository in the judged
    /// one's place), so git finds none.
    pub fn env(&self, clones: bool) -> Vec<(String, String)> {
        let nothing = if cfg!(windows) { "NUL" } else { "/dev/null" };
        match (&self.git, clones) {
            (Some(git), false) => {
                let mut env = vec![("GIT_DIR".to_owned(), git.display().to_string())];
                if let Some(top) = &self.work_tree {
                    env.push(("GIT_WORK_TREE".to_owned(), top.display().to_string()));
                }
                env
            }
            _ => vec![("GIT_DIR".to_owned(), nothing.to_owned())],
        }
    }
}

/// **Whether a run in `folder` reads only repository configuration the person controls**, for
/// a project at `root` whose chats may write `writable` (and the temporary folders): the
/// repository its git is then held to, or the sentence naming what a chat could have written.
/// `glab` says the run's program is glab, which also reads its own settings from the git
/// folder. The folder is judged as the kernel names it, its links followed.
pub fn held(folder: &Path, root: &Path, writable: &[PathBuf], glab: bool) -> Result<Pin, String> {
    let reach = Reach { root, writable };
    let folder = folder
        .canonicalize()
        .unwrap_or_else(|_| folder.to_path_buf());
    let Some(found) = repository_of(&folder)? else {
        return Ok(Pin {
            git: None,
            work_tree: None,
        });
    };
    let mut git_folders = vec![found.git.clone()];
    if let Some(common) = common_of(&found.git) {
        git_folders.push(common);
    }
    for git in &git_folders {
        reach.held(git, "its git folder", true)?;
        for file in SETTINGS {
            reach.held(&git.join(file), "the repository's settings", false)?;
        }
        if glab && git.join("glab-cli").exists() {
            return Err(refused(
                &git.join("glab-cli"),
                "holds glab's own settings for the repository, which a chat can write; move \
                 them to the person's own glab settings",
            ));
        }
    }
    let common = git_folders.last().cloned().unwrap_or(found.git.clone());
    for config in [common.join("config"), found.git.join("config.worktree")] {
        reach.config(&config, &found.top, 0)?;
    }
    Ok(Pin {
        work_tree: (!found.bare).then(|| found.top.clone()),
        git: Some(found.git),
    })
}

/// The folders a chat may write, as a run judges a repository's files.
struct Reach<'a> {
    root: &'a Path,
    writable: &'a [PathBuf],
}

impl Reach<'_> {
    /// Whether `path` is out of a chat's reach: outside every folder it may write, or in the
    /// project, out of the temporary folders, where class 5 holds it. A git folder (`folder`)
    /// in the project is held where it is held itself or its settings are ([`SETTINGS`], each
    /// judged in turn by the caller).
    fn held(&self, path: &Path, what: &str, folder: bool) -> Result<(), String> {
        let path = lexical(path);
        let temps = super::temp_folders();
        let mut written: Vec<PathBuf> = vec![self.root.to_path_buf()];
        written.extend(self.writable.iter().cloned());
        written.extend(temps.iter().cloned());
        if !super::inside_any(&path, &written) {
            return Ok(());
        }
        let in_project = super::inside_any(&path, &[self.root.to_path_buf()])
            && !super::inside_any(&path, &temps);
        if in_project
            && (held_by_class_5(&path) || (folder && held_by_class_5(&path.join("config"))))
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
            let mut line = line.trim();
            if let Some(header) = line.strip_prefix('[') {
                section = header
                    .split([']', ' ', '\t', '"', '.'])
                    .next()
                    .unwrap_or_default()
                    .to_ascii_lowercase();
                // A key may follow its section's header on the same line.
                line = header.split_once(']').map_or("", |(_, rest)| rest.trim());
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
                self.held(&included, "a config the repository's includes", false)?;
                self.config(&included, top, depth + 1)?;
            } else if section == "core" && key == "hookspath" {
                self.held(&top.join(&value), "the repository's hooks folder", false)?;
            }
        }
        Ok(())
    }
}

/// Whether `path` is where class 5 holds it from every chat in the project
/// ([`crate::sandbox::PLANTED`], by name: a `.git` folder's settings, `.husky`, `.githooks`,
/// a `.gitconfig`…), or, for a submodule's git folder at any depth under `.git/modules`, one of
/// its [`SETTINGS`] or below its hooks.
fn held_by_class_5(path: &Path) -> bool {
    if crate::sandboxblock::is_planted(path) {
        return true;
    }
    let parts: Vec<&str> = path
        .components()
        .filter_map(|part| match part {
            Component::Normal(name) => name.to_str(),
            _ => None,
        })
        .collect();
    let Some(at) = parts
        .windows(2)
        .position(|pair| pair == [".git", "modules"])
    else {
        return false;
    };
    let below = &parts[at + 2..];
    let joined = below.join("/");
    below.len() > 1
        && (SETTINGS
            .iter()
            .any(|file| joined.ends_with(&format!("/{file}")))
            || below.contains(&"hooks"))
}

/// A repository found from a folder: its git folder, the folder a relative hooks folder is
/// read from (its work tree's top, or the git folder of a bare one), and whether it is bare.
struct Found {
    git: PathBuf,
    top: PathBuf,
    bare: bool,
}

/// **The repository `folder` is in, as git finds it**: the nearest folder at or above it that
/// holds a `.git` folder, or a `.git` file naming one (read as git reads it: `gitdir: ` and the
/// rest, its line end aside), or that is a git folder itself (a bare repository, or inside a
/// `.git`), whatever git's `safe.bareRepository` says.
fn repository_of(folder: &Path) -> Result<Option<Found>, String> {
    for at in folder.ancestors() {
        let dot = at.join(".git");
        if std::fs::symlink_metadata(&dot).is_ok() {
            if dot.is_dir() {
                return Ok(Some(Found {
                    git: dot.canonicalize().unwrap_or(dot),
                    top: at.to_path_buf(),
                    bare: false,
                }));
            }
            let text = std::fs::read_to_string(&dot)
                .map_err(|_| refused(&dot, "is a `.git` that purlis cannot read"))?;
            let named = text
                .strip_prefix("gitdir: ")
                .map(|rest| rest.trim_end_matches(['\n', '\r']))
                .filter(|named| !named.is_empty() && !named.contains(['\n', '\r']));
            let Some(named) = named else {
                return Err(refused(&dot, "is a `.git` file that names no git folder"));
            };
            return Ok(Some(Found {
                git: lexical(&at.join(named)),
                top: at.to_path_buf(),
                bare: false,
            }));
        }
        if is_git_folder(at) {
            return Ok(Some(Found {
                git: at.to_path_buf(),
                top: at.to_path_buf(),
                bare: true,
            }));
        }
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
    Some(lexical(&git.join(named.trim_end_matches(['\n', '\r']))))
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

/// The options with which git reads a config, a git folder or a work tree other than the
/// repository judged, or runs a program the run names: a declared git's run never takes one.
const GIT_ELSEWHERE: &[&str] = &[
    "-c",
    "-C",
    "--config-env",
    "--git-dir",
    "--work-tree",
    "--exec-path",
    "--namespace",
    "--super-prefix",
];

/// **Why a run of git with `args` never runs with a vault's value**, if it does not: an option
/// of [`GIT_ELSEWHERE`], written alone, joined to its value (`-ckey=value`, `--git-dir=x`), or
/// anywhere among the arguments (git reads its own before the command, and a command's own
/// `-c` is refused with them).
pub fn refused_git(args: &[String]) -> Option<String> {
    let odd = args.iter().find(|arg| {
        GIT_ELSEWHERE.iter().any(|option| {
            arg.as_str() == *option
                || arg.starts_with(&format!("{option}="))
                || (option.len() == 2 && arg.starts_with(option))
        })
    })?;
    Some(format!(
        "git runs with a vault's value only in the repository purlis judged, and {odd:?} names \
         a config, a git folder or a program of its own, so nothing ran; run it in your own \
         terminal, where purlis limits nothing."
    ))
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
