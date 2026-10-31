// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Where the later-code class (ADR 0067 §5 class 5, rulings V73b and V73d) is found in a plane
//! when a chat starts. Names that are protected wherever they are ([`super::PLANTED`]) need no
//! search. What a protected config points at elsewhere does: the directory a `core.hooksPath`
//! names, and every script a harness's project config runs. Each is denied writing from the
//! chat's start, resolved then.
//!
//! The search follows no link. The places purlis itself runs git are read whole
//! ([`clones_purlis_works_in`]); the rest of the plane is searched within a bound ([`DEPTH`],
//! [`LOOKED_AT`]). Something added after the start, or beyond the bound, is not resolved: the config that would name it is itself
//! never written by the chat, so only a config a person writes later can name something new.

use std::path::{Path, PathBuf};

/// How many directories below the plane the search goes.
pub const DEPTH: usize = 4;

/// How many directory entries the search looks at in all.
pub const LOOKED_AT: usize = 20_000;

/// Directories the search never enters: a repository's own internals, and package trees.
const SKIPPED: [&str; 3] = [".git", "node_modules", "target"];

/// How many directories below a folder the search for repositories goes ([`git_folder_pins`]):
/// deeper than [`DEPTH`], as a workspace keeps each branch folder five below the plane
/// (`workspaces/<name>/.worktrees/<repo>/<branch>`).
pub const CLONE_DEPTH: usize = 6;

/// `root` and the directories below it, to [`DEPTH`], without links or [`SKIPPED`] ones.
pub fn directories(root: &Path) -> Vec<PathBuf> {
    directories_to(root, DEPTH)
}

/// [`directories`], to `depth` below `root`.
pub fn directories_to(root: &Path, depth: usize) -> Vec<PathBuf> {
    let mut found = vec![root.to_path_buf()];
    let mut looked = 0;
    let mut level = vec![root.to_path_buf()];
    for _ in 0..depth {
        let mut next = Vec::new();
        for dir in level {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                looked += 1;
                if looked > LOOKED_AT {
                    return found;
                }
                let is_dir = entry.file_type().is_ok_and(|kind| kind.is_dir());
                let name = entry.file_name();
                if !is_dir || SKIPPED.iter().any(|skipped| name == *skipped) {
                    continue;
                }
                found.push(entry.path());
                next.push(entry.path());
            }
        }
        level = next;
    }
    found
}

/// The directories of `dirs` that hold a `.git`: each a clone or a worktree.
pub fn clones(dirs: &[PathBuf]) -> Vec<PathBuf> {
    dirs.iter()
        .filter(|dir| dir.join(".git").exists())
        .cloned()
        .collect()
}

/// The git directory of the clone at `clone`, and the common one its config lives in: the
/// `.git` directory, or for a worktree the directory its `.git` file names and that one's
/// `commondir`.
pub fn git_dirs(clone: &Path) -> Vec<PathBuf> {
    let dot_git = clone.join(".git");
    if dot_git.is_dir() {
        return vec![dot_git];
    }
    let Ok(text) = std::fs::read_to_string(&dot_git) else {
        return Vec::new();
    };
    let Some(named) = text.lines().find_map(|line| line.strip_prefix("gitdir:")) else {
        return Vec::new();
    };
    let git_dir = clone.join(named.trim());
    let mut dirs = vec![git_dir.clone()];
    if let Ok(common) = std::fs::read_to_string(git_dir.join("commondir")) {
        dirs.push(git_dir.join(common.trim()));
    }
    dirs
}

/// **The paths that hold each repository's git folder as a whole** below `folders`, each
/// searched to [`CLONE_DEPTH`] and bounded as [`directories`] is: one path in or at each git
/// folder that is there now, which a harness that pins every folder above a denied path
/// ([`super::claude`]) denies writing, so the git folder stays where it is while git writes the
/// rest of it.
///
/// - A `.git` directory: its `config`, and each submodule's in it ([`module_git_dirs`]).
/// - A `.git` file (a linked worktree's or a submodule's): the file itself, and the `config` of
///   the git directory it names, or of the common one that directory names (`commondir`), with
///   each submodule's there. Only a pointer to a git directory, one that holds a `HEAD`, is
///   followed; a `.git` file that names anything else is no repository, and nothing of it is
///   held.
/// - A `.git` that is a link: nothing. Where it leads is not a path in this folder, and a rule
///   on it could hold what the link names instead.
///
/// Git writes none of these in ordinary work, and each `config` is held by name already
/// ([`super::PLANTED`]). A path that is a directory or a link is never one: a rule on it would
/// hold what is below it, or what it leads to. Each path once, as written with `..` taken off
/// ([`lexical`]), in the order found.
pub fn git_folder_pins(folders: &[PathBuf]) -> Vec<PathBuf> {
    git_folders_kept(folders)
        .into_iter()
        .map(|(pin, _)| pin)
        .collect()
}

/// The folders below `folder` that purlis itself runs git in, read without a bound: `folder`
/// itself, each clone of each workspace (`workspaces/<ws>/<clone>`, as [`crate::repos::clones`]
/// lists them) and each branch folder (`workspaces/<ws>/.worktrees/<repo>/<piece>`). Each
/// level is read whole and no link is followed, so nothing else in the plane, however much it
/// holds, keeps one of these from being found.
pub fn clones_purlis_works_in(folder: &Path) -> Vec<PathBuf> {
    // The directories in `dir`, never a link to one; none where `dir` is itself a link.
    let folders_in = |dir: &Path| -> Vec<PathBuf> {
        if !std::fs::symlink_metadata(dir).is_ok_and(|meta| meta.is_dir()) {
            return Vec::new();
        }
        let Ok(entries) = std::fs::read_dir(dir) else {
            return Vec::new();
        };
        let mut found: Vec<PathBuf> = entries
            .flatten()
            .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
            .map(|entry| entry.path())
            .collect();
        found.sort();
        found
    };
    let mut found = vec![folder.to_path_buf()];
    for workspace in folders_in(&folder.join(super::WORKSPACES)) {
        found.extend(folders_in(&workspace));
        for repo in folders_in(&workspace.join(".worktrees")) {
            found.extend(folders_in(&repo));
        }
    }
    clones(&found)
}

/// [`git_folder_pins`], each with the folder searched that it was found in.
pub fn git_folders_kept(folders: &[PathBuf]) -> Vec<(PathBuf, PathBuf)> {
    let mut pins: Vec<(PathBuf, PathBuf)> = Vec::new();
    for folder in folders {
        let mut pin = |path: PathBuf| {
            let path = lexical(&path);
            let is_entry = std::fs::symlink_metadata(&path)
                .is_ok_and(|kind| kind.is_dir() || kind.file_type().is_symlink());
            if !is_entry && !pins.iter().any(|(pinned, _)| *pinned == path) {
                pins.push((path, folder.clone()));
            }
        };
        // The clones purlis works in first, found whatever the bounded walk reaches; then the
        // rest the walk finds.
        let mut found = clones_purlis_works_in(folder);
        for clone in clones(&directories_to(folder, CLONE_DEPTH)) {
            if !found.contains(&clone) {
                found.push(clone);
            }
        }
        for clone in found {
            let dot_git = clone.join(".git");
            let Ok(kind) = std::fs::symlink_metadata(&dot_git) else {
                continue;
            };
            let git_dir = if kind.is_dir() {
                dot_git
            } else if kind.is_file() {
                let Some(named) = pointed_git_dir(&clone) else {
                    continue;
                };
                let common = named.join("commondir");
                let git_dir = if common.exists() {
                    let Some(common) = std::fs::read_to_string(&common)
                        .ok()
                        .map(|text| lexical(&named.join(text.trim())))
                        .filter(|dir| is_git_dir(dir))
                    else {
                        pin(dot_git);
                        continue;
                    };
                    common
                } else {
                    named
                };
                pin(dot_git);
                git_dir
            } else {
                continue;
            };
            pin(git_dir.join("config"));
            for module in module_git_dirs(&git_dir) {
                pin(module.join("config"));
            }
        }
    }
    pins
}

/// Whether `dir` is a git directory: a directory, not a link, that holds a `HEAD`.
fn is_git_dir(dir: &Path) -> bool {
    std::fs::symlink_metadata(dir).is_ok_and(|kind| kind.is_dir()) && dir.join("HEAD").is_file()
}

/// The git directory the `.git` file of `clone` names, where it names one ([`is_git_dir`]).
fn pointed_git_dir(clone: &Path) -> Option<PathBuf> {
    let text = std::fs::read_to_string(clone.join(".git")).ok()?;
    let named = text.lines().find_map(|line| line.strip_prefix("gitdir:"))?;
    let dir = lexical(&clone.join(named.trim()));
    is_git_dir(&dir).then_some(dir)
}

/// The submodules' git directories below a clone's `git_dir` (`modules/…`, nested), each one
/// a directory holding a `HEAD`.
pub fn module_git_dirs(git_dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut looked = 0;
    let mut level = vec![git_dir.join("modules")];
    while !level.is_empty() && looked < LOOKED_AT {
        let mut next = Vec::new();
        for dir in level {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                looked += 1;
                if !entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                    continue;
                }
                let path = entry.path();
                if path.join("HEAD").is_file() {
                    found.push(path.clone());
                    next.push(path.join("modules"));
                } else {
                    next.push(path);
                }
            }
        }
        level = next;
    }
    found
}

/// `core.hooksPath` in the git config `text`, as written.
pub fn hooks_path_in(text: &str) -> Option<String> {
    let mut in_core = false;
    let mut found = None;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            let section = line.trim_start_matches('[').split([']', ' ', '"']).next();
            in_core = section.is_some_and(|it| it.eq_ignore_ascii_case("core"));
            continue;
        }
        if !in_core {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim().eq_ignore_ascii_case("hookspath") {
            let value = value.trim();
            let value = value
                .strip_prefix('"')
                .and_then(|it| it.strip_suffix('"'))
                .unwrap_or(value);
            found = Some(value.to_owned());
        }
    }
    found
}

/// `written`, a path a config names, made absolute against `base` with `~` as `home`.
fn absolute(written: &str, base: &Path, home: Option<&Path>) -> Option<PathBuf> {
    if written.is_empty() || written.contains('$') {
        return None;
    }
    if let Some(rest) = written.strip_prefix("~/") {
        return home.map(|home| home.join(rest));
    }
    let path = Path::new(written);
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    };
    // `./` parts said as written, not as a sandbox matches a path.
    Some(
        joined
            .components()
            .filter(|part| !matches!(part, std::path::Component::CurDir))
            .collect(),
    )
}

/// One path resolved for the later-code class, with what named it: the config file and the
/// word in it, as written, so a refusal can say where to look.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    pub path: PathBuf,
    /// The file that names it: a git config, or a harness's project config.
    pub file: PathBuf,
    /// The word in that file that names it.
    pub word: String,
}

/// `path` with each `..` taking off the part before it, as written: what a sandbox makes of
/// `repo/../..` where those folders need not exist to be asked about.
pub(crate) fn lexical(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            part => out.push(part),
        }
    }
    out
}

/// The directories every `core.hooksPath` that git would use names: each clone's own, and the
/// user's global one, resolved against each clone. Those directories hold the hooks git runs.
pub fn hooks_paths(
    clones: &[PathBuf],
    home: Option<&Path>,
    xdg_config: Option<&Path>,
) -> Vec<Resolved> {
    let read = |file: PathBuf| {
        let text = std::fs::read_to_string(&file).ok()?;
        Some((file, hooks_path_in(&text)?))
    };
    let global: Vec<(PathBuf, String)> = [
        home.map(|home| home.join(".gitconfig")),
        xdg_config.map(|config| config.join("git/config")),
    ]
    .into_iter()
    .flatten()
    .filter_map(read)
    .collect();
    let mut found = Vec::new();
    for clone in clones {
        let own = git_dirs(clone)
            .into_iter()
            .filter_map(|dir| read(dir.join("config")));
        for (file, written) in own.chain(global.iter().cloned()) {
            if let Some(path) = absolute(&written, clone, home) {
                found.push(Resolved {
                    path,
                    file,
                    word: written,
                });
            }
        }
    }
    found
}

/// The project config files whose commands a harness runs, relative to the directory they
/// configure (ruling V73d).
pub const COMMAND_CONFIGS: [&str; 6] = [
    ".claude/settings.json",
    ".claude/settings.local.json",
    ".mcp.json",
    "opencode.json",
    "opencode.jsonc",
    ".codex/config.toml",
];

/// The keys whose values are a command, or a command's words, or a plugin, in those files.
const COMMAND_KEYS: [&str; 4] = ["command", "args", "plugin", "plugins"];

/// What a [`COMMAND_KEYS`] key holds: a line a shell reads, or words handed over as they are.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Command {
    Line(String),
    Words(Vec<String>),
}

/// Every script the project config files in `dir` name for a harness to run: each word of a
/// command, its arguments or a plugin entry that reads as a path ([`reads_as_a_path`]), and
/// each word a program opens as a file of code, `/` or not ([`files_of`], D-1418-1), made
/// absolute against `dir`.
///
/// **Fail closed** for what is a path (ADR 0067 §5, the later-code class). A word is left out
/// only where it cannot name a script a harness runs: jq's program text, which runs nothing; an
/// operator; where a command's output goes; an option; a URL; a word made only of `/`; and a
/// folder that holds the config, which could not be replaced without the config going with it
/// (#1327, D-1327-8), though a runner pointed at one has the files it loads there named
/// ([`ENTRY_FILES`], D-1327-9). What a shell is handed to run (`-c`), what it reads its commands from (`<`, `<<<`,
/// `<(…)`) and the program text of `awk` and `sed`, which can run commands, are read as
/// commands in turn (D-1327-7).
///
/// A package a runner loads from `node_modules`, and a folder not there yet that a name Node
/// loads first starts with, are named once, as the folder, which holds every file named in it
/// before; a path a folder named already holds is not named again (#1425). The same paths are
/// protected by fewer entries, so a config stays further below [`MOST_NAMED`].
///
/// A config that names more than purlis follows ([`MOST_MOVES`], [`MOST_NAMED`]) is [`Unread`]:
/// the chat is refused rather than started with what was read so far.
pub fn scripts_named(
    project: &Path,
    dir: &Path,
    home: Option<&Path>,
    writable: &[PathBuf],
) -> Result<Vec<Resolved>, Unread> {
    let mut found = Vec::new();
    for name in COMMAND_CONFIGS {
        if let Ok(text) = std::fs::read_to_string(dir.join(name)) {
            found.extend(scripts_in(project, dir, name, &text, home, writable)?);
        }
    }
    Ok(found)
}

/// A config purlis stopped reading, and the word it stopped at.
pub type Unread = super::Named;

/// The most folder moves (`cd`, `pushd`) followed in one config.
pub const MOST_MOVES: usize = 64;

/// The most paths named from one config.
pub const MOST_NAMED: usize = 1024;

/// The files a runner loads when it is pointed at a folder (D-1327-9): denied in that folder,
/// not below it, where the folder itself is never denied (D-1327-8).
pub const ENTRY_FILES: [&str; 11] = [
    "package.json",
    "index.js",
    "index.mjs",
    "index.cjs",
    "index.ts",
    "main.go",
    "go.mod",
    "__main__.py",
    "main.py",
    "index.php",
    "main.rb",
];

/// Programs that run what a folder holds when pointed at one ([`ENTRY_FILES`]); `go` and `uv`
/// with `run`.
const RUNNERS: [&str; 10] = [
    "node", "deno", "bun", "tsx", "ts-node", "npx", "python", "python3", "ruby", "php",
];

/// [`scripts_named`] for one of the [`COMMAND_CONFIGS`], `name`, in `dir`, that holds `text`,
/// in the project at `project`.
pub fn scripts_in(
    project: &Path,
    dir: &Path,
    name: &str,
    text: &str,
    home: Option<&Path>,
    writable: &[PathBuf],
) -> Result<Vec<Resolved>, Unread> {
    let value: Option<serde_json::Value> = if name.ends_with(".toml") {
        text.parse::<toml::Table>()
            .ok()
            .and_then(|table| serde_json::to_value(table).ok())
    } else {
        serde_json::from_str(&without_comments(text)).ok()
    };
    let mut commands = Vec::new();
    if let Some(value) = value {
        commands_in(&value, false, &mut commands);
    }
    let mut scan = Scan {
        project,
        dir,
        home,
        writable,
        file: dir.join(name),
        dir_text: dir.display().to_string(),
        found: Vec::new(),
        seen: std::collections::HashSet::new(),
        named: std::collections::HashSet::new(),
        moves: 0,
        unread: None,
        roots: std::cell::OnceCell::new(),
    };
    for command in commands {
        let mut bases = Bases::at(dir);
        scan.moves = 0;
        match command {
            Command::Line(line) => scan.line(&line, &mut bases, NESTING),
            Command::Words(words) => scan.segment(&words, &mut bases, NESTING),
        }
    }
    match scan.unread {
        Some(unread) => Err(unread),
        None => Ok(scan.found),
    }
}

/// The folders one command's relative words are found from: every folder it went to, from
/// the one its config is in on, so a script after the shell went back to an earlier folder
/// (`cd -`, `popd`, a closed subshell) is still found where it is. A move is taken from the
/// folder the shell is in, so a chain of them costs one step each, and [`MOST_MOVES`] bounds
/// how many there are.
struct Bases {
    visited: Vec<PathBuf>,
    /// The folder the shell is in now, the one `cd -` goes back to, and `pushd`'s stack.
    now: PathBuf,
    before: PathBuf,
    stack: Vec<PathBuf>,
    /// What each open subshell will go back to.
    scopes: Vec<(PathBuf, PathBuf, Vec<PathBuf>)>,
}

impl Bases {
    fn at(start: &Path) -> Self {
        Self {
            visited: vec![start.to_path_buf()],
            now: start.to_path_buf(),
            before: start.to_path_buf(),
            stack: Vec::new(),
            scopes: Vec::new(),
        }
    }

    /// The shell is now in `folder`.
    fn go(&mut self, folder: PathBuf) {
        self.before = std::mem::replace(&mut self.now, folder);
        if !self.visited.contains(&self.now) {
            self.visited.push(self.now.clone());
        }
    }
}

/// How deep a command handed to another as text (`bash -c '…'`, a quoted word) is read.
const NESTING: usize = 4;

/// Programs whose program text runs nothing, so it names no script: their first operand,
/// unless a `-f` names the file the program is read from (D-1327-7).
const INERT_TEXT: [&str; 2] = ["jq", "gojq"];

/// Shells, whose `-c` operand is commands.
const SHELLS: [&str; 7] = ["sh", "bash", "zsh", "dash", "ksh", "mksh", "fish"];

/// Where [`scripts_in`] gathers what one config names.
struct Scan<'a> {
    /// The project the config is in, whose folders a value may name ([`Scan::values`]).
    project: &'a Path,
    dir: &'a Path,
    home: Option<&'a Path>,
    /// The folders outside the project and the home folder a chat could still write: those
    /// you list as ones chats may be granted, and the project's cache home (D-T56-1).
    writable: &'a [PathBuf],
    file: PathBuf,
    dir_text: String,
    found: Vec<Resolved>,
    seen: std::collections::HashSet<PathBuf>,
    /// The paths in `found`, so one a folder named already holds is not named again.
    named: std::collections::HashSet<PathBuf>,
    /// The folder moves followed so far in the command being read.
    moves: usize,
    /// Where reading stopped, past [`MOST_MOVES`] or [`MOST_NAMED`].
    unread: Option<Unread>,
    /// Every spelling of each folder [`Scan::holds`] counts, and whether its volume folds
    /// case: worked out at the first value that asks.
    roots: std::cell::OnceCell<Vec<(PathBuf, bool)>>,
}

impl Scan<'_> {
    /// A line a shell reads, with `bases` the folders its relative words are found from.
    fn line(&mut self, line: &str, bases: &mut Bases, depth: usize) {
        for segment in shell_segments(line) {
            match segment {
                Segment::Words(words) => self.segment(&words, bases, depth),
                Segment::Open => bases.scopes.push((
                    bases.now.clone(),
                    bases.before.clone(),
                    bases.stack.clone(),
                )),
                Segment::Close => {
                    if let Some((now, before, stack)) = bases.scopes.pop() {
                        bases.now = now;
                        bases.before = before;
                        bases.stack = stack;
                    }
                }
            }
        }
    }

    /// One command's words.
    fn segment(&mut self, segment: &[String], bases: &mut Bases, depth: usize) {
        if self.unread.is_some() {
            return;
        }
        let read = read_segment(segment);
        for one in read.moves {
            self.moves += 1;
            if self.moves > MOST_MOVES {
                self.stop(&one.word);
                return;
            }
            match one.kind {
                MoveKind::To | MoveKind::Push => {
                    let written = self.substituted(&one.word);
                    if let Some(path) = absolute(&written, &bases.now, self.home) {
                        if one.kind == MoveKind::Push {
                            bases.stack.push(bases.now.clone());
                        }
                        bases.go(path);
                    }
                }
                MoveKind::Back => {
                    let before = bases.before.clone();
                    bases.go(before);
                }
                MoveKind::Pop => {
                    if let Some(folder) = bases.stack.pop() {
                        bases.go(folder);
                    }
                }
                MoveKind::Visit => {
                    let written = self.substituted(&one.word);
                    if let Some(path) = absolute(&written, &bases.now, self.home)
                        && !bases.visited.contains(&path)
                    {
                        bases.visited.push(path);
                    }
                }
            }
        }
        let mut folders = read
            .search
            .map(|search| search_folders(search, &read.words))
            .unwrap_or_default();
        if read.search == Some(Search::Node) {
            let stops: Vec<&Path> = std::iter::once(self.project)
                .chain(self.home)
                .chain(self.writable.iter().map(PathBuf::as_path))
                .collect();
            folders.extend(node_modules_up(&bases.visited, &stops, self.home));
        }
        for word in read.words {
            self.word(&word, bases, depth, read.runner);
        }
        for file in read.files {
            self.file_word(&file, bases, read.runner);
            if !file.searched || !looked_up(&file.value) {
                continue;
            }
            // Where the program also looks a bare name up (#1425): named as a file in each
            // folder of its search path, as it is in the folders the command runs in. An
            // absolute folder is held to where a chat could write, as a value after `=` is.
            for folder in &folders {
                let folder = self.substituted(folder);
                if folder.starts_with('/') && !self.holds(&folder) {
                    continue;
                }
                let along = FileWord {
                    value: format!("{}/{}", folder.trim_end_matches('/'), file.value),
                    ..file.clone()
                };
                self.file_word(&along, bases, read.runner);
            }
        }
        if let Some(depth) = depth.checked_sub(1) {
            for text in read.commands {
                self.line(&text, bases, depth);
            }
        }
    }

    /// One word: named where it reads as a path, and read again as a line where a shell would
    /// split it, as a quoted operand or an argument handed over whole may be.
    /// One word of a command whose program is a runner (`runner`) or not.
    fn word(&mut self, word: &str, bases: &mut Bases, depth: usize, runner: bool) {
        if self.unread.is_some() {
            return;
        }
        for (written, file_only) in self.values(word) {
            self.named_as(&written, word, bases, runner, file_only);
        }
        if let Some((name, value)) = word.split_once('=')
            && let Some(carries) = runs_code(name)
        {
            self.code_value(carries, value, bases, depth, runner);
        }
        let split_again = word.contains(|c: char| {
            c.is_whitespace()
                || matches!(
                    c,
                    '|' | ';' | '&' | '<' | '>' | '(' | ')' | '"' | '\'' | '`'
                )
        });
        if split_again && let Some(depth) = depth.checked_sub(1) {
            self.line(word, bases, depth);
        }
    }

    /// `value`, of a variable or git setting that loads code as `carries` says: named whatever
    /// is at the path, as a chat can make a folder where a file will be (D-1418-10).
    fn code_value(
        &mut self,
        carries: Carries,
        value: &str,
        bases: &mut Bases,
        depth: usize,
        runner: bool,
    ) {
        let file = |value: &str| FileWord {
            value: value.to_owned(),
            finds: Finds::Exact,
            searched: false,
        };
        match carries {
            Carries::Path => self.file_word(&file(value), bases, runner),
            Carries::Paths => {
                for one in value.split([':', ' ']).filter(|it| !it.is_empty()) {
                    self.file_word(&file(one), bases, runner);
                }
            }
            Carries::Command => {
                if let Some(depth) = depth.checked_sub(1) {
                    self.line(value, bases, depth);
                }
            }
            Carries::Options(program) => {
                if let Some(depth) = depth.checked_sub(1) {
                    self.line(&format!("{program} {value}"), bases, depth);
                }
            }
            Carries::Perl => {
                let mut words = value.split_whitespace();
                let mut dirs = Vec::new();
                let mut modules = Vec::new();
                while let Some(word) = words.next() {
                    let Some(option) = word.strip_prefix('-') else {
                        continue;
                    };
                    let mut chars = option.chars();
                    let Some(letter) = chars.next() else {
                        continue;
                    };
                    let rest = chars.as_str();
                    let given = if rest.is_empty() {
                        words.next().unwrap_or_default()
                    } else {
                        rest
                    };
                    match letter {
                        'I' => dirs.push(given.to_owned()),
                        'M' | 'm' => modules.push(perl_module_file(given)),
                        _ => {}
                    }
                }
                for dir in &dirs {
                    self.file_word(&file(dir), bases, runner);
                }
                for module in &modules {
                    self.file_word(&file(module), bases, runner);
                    for dir in &dirs {
                        self.file_word(&file(&format!("{dir}/{module}")), bases, runner);
                    }
                }
            }
        }
    }

    /// `written`, one value of `word`, named where it reads as a path; only as a file where
    /// `file_only` ([`Self::values`]).
    fn named_as(
        &mut self,
        written: &str,
        word: &str,
        bases: &Bases,
        runner: bool,
        file_only: bool,
    ) {
        let folder_word = runner && matches!(written, "." | "..");
        if reads_as_a_path(written) || folder_word {
            self.named_at(written, word, bases, runner, &[], file_only);
        }
    }

    /// `written`, named by `word`, from each folder the command went to; and, where its name
    /// carries none of `extensions`, with each of them, as a runner tries it (D-1418-2). A
    /// folder that is there is, to such a runner, the files it loads from it, not the folder
    /// (D-1327-9). Where `file_only`, a path is named only as a file: one that is there, or
    /// one not there yet with an extension (D-1418-10).
    fn named_at(
        &mut self,
        written: &str,
        word: &str,
        bases: &Bases,
        runner: bool,
        extensions: &[&str],
        file_only: bool,
    ) {
        for base in bases.visited.clone() {
            let Some(path) = absolute(written, &base, self.home) else {
                continue;
            };
            // A folder that holds the config is no script: replaced, it would take the
            // config naming it along (D-1327-8). What a runner loads from it is (D-1327-9).
            if self.dir.starts_with(lexical(&path)) {
                if runner {
                    for entry in ENTRY_FILES {
                        self.name(path.join(entry), word);
                    }
                }
                continue;
            }
            // A package in `node_modules`: its folder once, which holds every file a runner
            // could load from it (#1425). Not one written with `..`, which is named as written
            // and with the `..` taken off, below.
            if !file_only
                && has_no_parent(&path)
                && let Some(package) = self.package_of(&path)
            {
                self.name(package.clone(), word);
                if lexical(&path) == package {
                    self.beside(&package, word, extensions);
                    self.main_of(&package, word, extensions);
                }
                continue;
            }
            if !extensions.is_empty() && path.is_dir() {
                for entry in ENTRY_FILES {
                    self.name(path.join(entry), word);
                }
                // A runner tries the file beside the folder before the folder (Node's
                // `LOAD_AS_FILE` before `LOAD_AS_DIRECTORY`).
                self.beside(&path, word, extensions);
                self.main_of(&path, word, extensions);
                continue;
            }
            let tried: Vec<PathBuf> = match path.file_name().and_then(|name| name.to_str()) {
                Some(name)
                    if !written.ends_with('/')
                        && !extensions.iter().any(|it| name.ends_with(it)) =>
                {
                    extensions
                        .iter()
                        .map(|it| path.with_file_name(format!("{name}{it}")))
                        .collect()
                }
                _ => Vec::new(),
            };
            for path in std::iter::once(path).chain(tried) {
                if file_only && !(path.is_file() || (!path.exists() && path.extension().is_some()))
                {
                    continue;
                }
                // As written and with `..` taken off, which a sandbox matches when the folder
                // before the `..` is missing.
                let plain = lexical(&path);
                self.name(path, word);
                self.name(plain, word);
            }
        }
    }

    /// The package folder `path` is in, where it is to be named whole (#1425): the folder right
    /// below the nearest `node_modules` above `path` (`node_modules/<name>`, or
    /// `node_modules/@<scope>/<name>`). None where `path` is in no package, where that folder
    /// holds the config or the project (a project that is itself below a `node_modules`), and
    /// where it is a link to anything but another `node_modules`'s package: a workspace's
    /// package linked there is the project's own code, which the chat must still be able to
    /// work on, so only the files a runner loads from it are named, as anywhere else.
    fn package_of(&self, path: &Path) -> Option<PathBuf> {
        let path = lexical(path);
        let package = package_folder(&path)?;
        if self.dir.starts_with(&package) || self.project.starts_with(&package) {
            return None;
        }
        match std::fs::symlink_metadata(&package) {
            Ok(meta) if meta.file_type().is_symlink() => {
                let target = std::fs::canonicalize(&package).ok()?;
                package_folder(&target)?;
            }
            Ok(_) => {}
            // Not there yet: a folder named whole as nothing is in it.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return None,
        }
        Some(package)
    }

    /// Each file beside the folder `folder` a runner tries, with each of `extensions`, before
    /// the folder.
    fn beside(&mut self, folder: &Path, word: &str, extensions: &[&str]) {
        let Some(name) = folder.file_name().and_then(|name| name.to_str()) else {
            return;
        };
        for it in extensions {
            self.name(folder.with_file_name(format!("{name}{it}")), word);
        }
    }

    /// The files the manifest of `dir`, a package Node found in `node_modules`, says Node or a
    /// bundler loads ([`package_entries`]: its `main`, its `module` and what its `exports` maps
    /// to), each held to where a chat could write (#1425); nothing where `dir` is no such
    /// package. Below a package folder named whole each is held already ([`Self::name`]).
    fn main_of(&mut self, dir: &Path, word: &str, extensions: &[&str]) {
        for main in package_entries(dir) {
            if !self.holds(&main.display().to_string()) {
                continue;
            }
            // As a name written with its extension is taken as it is, an entry with one names
            // that file alone; one without is tried as Node tries a `main`, with each extension
            // and as a folder. Kept narrow, as each package costs its share of [`MOST_NAMED`].
            let bare = main
                .file_name()
                .and_then(|name| name.to_str())
                .filter(|name| !extensions.iter().any(|it| name.ends_with(it)));
            let mut tried: Vec<PathBuf> = Vec::new();
            if let Some(name) = bare {
                tried.extend(
                    extensions
                        .iter()
                        .map(|it| main.with_file_name(format!("{name}{it}"))),
                );
                tried.extend(ENTRY_FILES.iter().map(|entry| main.join(entry)));
            }
            for one in std::iter::once(main.clone()).chain(tried) {
                self.name(one, word);
            }
        }
    }

    /// A word a program opens as a file of code (D-1418-1): named from each folder the command
    /// went to, with or without a `/`, and as the runner looks it up ([`Finds`]).
    fn file_word(&mut self, file: &FileWord, bases: &Bases, runner: bool) {
        if self.unread.is_some() {
            return;
        }
        let written = self.substituted(&file.value);
        if written.is_empty() || written == "-" || written.contains("://") {
            return;
        }
        match file.finds {
            // A Perl module is turned into its file where it is read ([`files_of`]).
            Finds::Exact | Finds::PerlModule => {
                self.named_at(&written, &file.value, bases, runner, &[], false);
            }
            Finds::Extensions(extensions) if file.searched && extensions == JS => {
                self.package_word(&written, &file.value, bases, runner, extensions);
            }
            Finds::Extensions(extensions) => {
                self.named_at(&written, &file.value, bases, runner, extensions, false);
            }
            Finds::Module => {
                for path in module_files(&written) {
                    self.named_at(&path, &file.value, bases, runner, &[], false);
                }
            }
        }
    }

    /// A name Node is asked to load first (`-r`, `--import`), `written`, from each folder the
    /// command went to (#1425). Written as a package and a file in it (`dotenv/config`,
    /// `@scope/name/x`), where the folder it starts with is not there, that folder is named
    /// once: it holds every file a runner could try below it, which a chat could only plant
    /// by making the folder. Where it is there, each file is named as [`Self::named_at`] names
    /// it.
    fn package_word(
        &mut self,
        written: &str,
        word: &str,
        bases: &Bases,
        runner: bool,
        extensions: &[&str],
    ) {
        // Not one written with `..`, whose files may be outside the folder it starts with.
        let first = written
            .split_once('/')
            .map(|(first, _)| first)
            .filter(|first| {
                looked_up(written) && !first.is_empty() && !written.split('/').any(|it| it == "..")
            });
        let Some(first) = first else {
            self.named_at(written, word, bases, runner, extensions, false);
            return;
        };
        for base in &bases.visited {
            let folder = base.join(first);
            let missing = matches!(
                std::fs::symlink_metadata(&folder),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound
            );
            if missing && !self.dir.starts_with(lexical(&folder)) {
                // As written and with `..` taken off (from the folder the command went to), as
                // [`Self::named_at`] names a path.
                let plain = lexical(&folder);
                self.name(folder, word);
                self.name(plain, word);
            } else {
                self.named_at(written, word, &Bases::at(base), runner, extensions, false);
            }
        }
    }

    /// `path`, named by `word`, once; past [`MOST_NAMED`] the config is unread.
    ///
    /// A path a folder named already holds is not named again, and a folder named takes the
    /// place of the paths named below it: a rule on a folder holds everything below it, so the
    /// same paths are protected by fewer (#1425). A path written with `..` is neither, as what
    /// it is below is not what it reads as.
    fn name(&mut self, path: PathBuf, word: &str) {
        if !self.seen.insert(path.clone()) {
            return;
        }
        let plain = has_no_parent(&path);
        if plain
            && path
                .ancestors()
                .skip(1)
                .any(|folder| self.named.contains(folder))
        {
            return;
        }
        if plain {
            let below: Vec<PathBuf> = self
                .named
                .iter()
                .filter(|one| one.starts_with(&path) && has_no_parent(one))
                .cloned()
                .collect();
            if !below.is_empty() {
                self.found.retain(|one| !below.contains(&one.path));
                for one in &below {
                    self.named.remove(one);
                }
            }
        }
        if self.found.len() >= MOST_NAMED {
            self.stop(word);
            return;
        }
        self.named.insert(path.clone());
        self.found.push(Resolved {
            path,
            file: self.file.clone(),
            word: word.to_owned(),
        });
    }

    /// Stops reading at `word`.
    fn stop(&mut self, word: &str) {
        self.unread.get_or_insert_with(|| Unread {
            file: self.file.clone(),
            word: word.to_owned(),
        });
    }

    /// `word` as the harness would see it: a `file://` plugin as its path, and the project
    /// folder's variable as the folder.
    fn substituted(&self, word: &str) -> String {
        word.trim_start_matches("file://")
            .replace("${CLAUDE_PROJECT_DIR}", &self.dir_text)
            .replace("$CLAUDE_PROJECT_DIR", &self.dir_text)
    }

    /// What `word` may hand its program as a file ([`Self::substituted`]): an option's value
    /// after `=` (`--flag=value`) or after a JVM agent's colon, or each value it may have
    /// written against a short option ([`attached_values`]), as a separated value would be;
    /// any other word as it is (#1356).
    ///
    /// A value taken out of a word that starts with `~` is named as written too, below the
    /// folder the command runs in: a shell expands `~` only at a word's start, and bash after
    /// an option's `=` as well, so the program may open either (D-1356-8).
    ///
    /// Each comes with whether it is named only as a file (D-1418-10): a value after `=`, an
    /// assignment's or an option's, is a search path or an output folder as often as code, so
    /// a folder named only that way is never denied.
    fn values(&self, word: &str) -> Vec<(String, bool)> {
        let written = self.substituted(word);
        let mut whole = false;
        let mut taken: Vec<(String, bool)> = Vec::new();
        if let Some(list) = comma_list(&written) {
            // Each value a compiler hands on (`-Wl,-rpath,./lib`), a word of its own (D-1418-5).
            for value in list.split(',') {
                if value.starts_with('-') {
                    taken.extend(self.values(value));
                } else {
                    taken.push((value.to_owned(), false));
                }
            }
        } else if let Some(value) = COLON_OPTIONS
            .iter()
            .find_map(|option| written.strip_prefix(option))
        {
            // A JVM agent's file, and its options after `=`, which can name files.
            let mut parts = value.splitn(2, '=');
            taken.extend(parts.next().map(|it| (it.to_owned(), false)));
            taken.extend(parts.next().map(|it| (it.to_owned(), true)));
        } else {
            if let Some((flag, value)) = written.split_once('=') {
                if flag.starts_with('-') {
                    taken.push((value.to_owned(), true));
                } else if is_assigned(flag) {
                    // `BASH_ENV=./x.sh`: the value, as a guess held to where a chat could
                    // write, and the whole word (D-1418-3).
                    whole = true;
                    if !value.starts_with('/') || self.holds(value) {
                        taken.push((value.to_owned(), true));
                    }
                    // A colon list (`PYTHONPATH=./a:./b`, `NODE_PATH=lib:tools`): each part as a
                    // value of its own, held the same way, beside the value whole (#1425).
                    if value.contains(':') && !value.contains("://") {
                        for part in value.split(':').filter(|part| !part.is_empty()) {
                            if !part.starts_with('/') || self.holds(part) {
                                taken.push((part.to_owned(), true));
                            }
                        }
                    }
                }
            }
            // And what follows a short option's letter, `=` and all (D-1418-4).
            taken.extend(
                attached_values(&written)
                    .into_iter()
                    .filter(|(value, first)| *first || !value.starts_with('/') || self.holds(value))
                    .map(|(value, _)| (value, false)),
            );
        }
        if taken.is_empty() {
            return vec![(written, false)];
        }
        let mut out = Vec::new();
        if whole {
            out.push((written.clone(), true));
        }
        for (value, file_only) in taken {
            if value.starts_with('~') {
                out.push((format!("./{value}"), file_only));
            }
            out.push((value, file_only));
        }
        out
    }

    /// Whether the absolute `path` is where a chat could write it: in the project, the home
    /// folder, a folder you list as one chats may be granted, or the project's cache home
    /// (D-1356-7, D-T56-1). Counting too many only names more files, which fails closed.
    ///
    /// Each by every name it and `path` have ([`super::spellings`]): through a link, on the
    /// data volume's other side, and case-folded where the folder's volume folds case (#1418).
    fn holds(&self, path: &str) -> bool {
        let roots = self.roots.get_or_init(|| {
            std::iter::once(self.project)
                .chain(self.home)
                .chain(self.writable.iter().map(PathBuf::as_path))
                .flat_map(|root| {
                    let folds = super::folds_case(root);
                    super::spellings(root)
                        .into_iter()
                        .map(move |name| (name, folds))
                })
                .collect()
        });
        let names = super::spellings(Path::new(path));
        names.iter().any(|name| {
            roots.iter().any(|(root, folds)| {
                name.starts_with(root)
                    || (*folds && super::folded(name).starts_with(super::folded(root)))
            })
        })
    }
}

/// Options whose value is written after a colon: a JVM's agents (`-javaagent:<jar>[=<options>]`).
const COLON_OPTIONS: [&str; 3] = ["-javaagent:", "-agentpath:", "-agentlib:"];

/// The values after a compiler's `-W<letter>,` (`-Wl,`, `-Wa,`, `-Wp,`), which it hands on
/// split at each comma.
fn comma_list(word: &str) -> Option<&str> {
    let rest = word.strip_prefix("-W")?;
    let mut chars = rest.chars();
    let letter = chars.next()?;
    (letter.is_ascii_alphabetic() && chars.next() == Some(',')).then(|| &rest[2..])
}

/// The files Python runs for `-m <module>` (#1418 review M2): `a/b.py`, or the package
/// `a/b`'s `__main__.py` and `__init__.py`, and each parent package's `__init__.py`. Never the
/// package folder, which holds the code a chat works on.
fn module_files(module: &str) -> Vec<String> {
    let path = module.replace('.', "/");
    let mut out = vec![
        format!("{path}.py"),
        format!("{path}/__main__.py"),
        format!("{path}/__init__.py"),
    ];
    let mut parent = path.as_str();
    while let Some((up, _)) = parent.rsplit_once('/') {
        out.push(format!("{up}/__init__.py"));
        parent = up;
    }
    out
}

/// Whether `name`, before a word's first `=`, reads as what is assigned: a variable
/// (`BASH_ENV`) or a git setting (`core.hooksPath`).
fn is_assigned(name: &str) -> bool {
    name.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
}

/// The values a cluster of short options may have written against one of them, `-L./lib` or
/// `-bf./x.awk`: what follows each of its leading letters, where that holds a `/`, and whether
/// it follows the first letter.
///
/// **Fail closed** (#1356, D-1356-1). Which letter takes a value is the program's to say, so
/// every split is read. A relative split that is no value names a file below the config's
/// folder that nobody runs, and denies a chat nothing it needs. An absolute split after a
/// later letter could name a real folder elsewhere (`-Ivendor/tmp` names `/tmp`), so
/// [`Scan::values`] keeps one only where a chat could write it, in the project, the home
/// folder, a folder chats may be granted or the project's cache home (D-1356-7, D-T56-1).
fn attached_values(word: &str) -> Vec<(String, bool)> {
    let Some(short) = word
        .strip_prefix('-')
        .filter(|short| !short.starts_with('-'))
    else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (at, letter) in short.char_indices() {
        if !letter.is_ascii_alphanumeric() {
            break;
        }
        let value = &short[at + letter.len_utf8()..];
        if !value.contains('/') {
            break;
        }
        out.push((value.to_owned(), at == 0));
    }
    out
}

/// Whether `word` is a cluster of short options whose leading letters hold `letter`, with or
/// without a value written against the last of them (`-rf`, `-rf./x.jq`).
fn short_leads_with(word: &str, letter: char) -> bool {
    word.strip_prefix('-')
        .filter(|short| !short.starts_with('-'))
        .is_some_and(|short| {
            short
                .chars()
                .take_while(char::is_ascii_alphabetic)
                .any(|it| it == letter)
        })
}

/// One command's words, sorted by what they are to a script search.
#[derive(Debug, Default)]
struct Read {
    /// Words that may name a script.
    words: Vec<String>,
    /// Text handed to a shell to run (`-c`), read as commands.
    commands: Vec<String>,
    /// Where the commands after this one run (`cd`, `pushd`, `popd`).
    moves: Vec<Move>,
    /// Whether the program runs what a folder holds when pointed at one ([`RUNNERS`]).
    runner: bool,
    /// Words the program opens as a file of code ([`LOADS`]).
    files: Vec<FileWord>,
    /// The search path the program also looks those up along ([`Search`]).
    search: Option<Search>,
}

/// A change of the folder later commands run in.
#[derive(Debug)]
struct Move {
    kind: MoveKind,
    /// The word that made it: the folder, or `-` or `popd`.
    word: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MoveKind {
    /// `cd <folder>`.
    To,
    /// `pushd <folder>`.
    Push,
    /// `cd -`, to the folder before.
    Back,
    /// `popd`.
    Pop,
    /// A folder this command alone runs in (`uv --directory D`, `env -C D`): its words are
    /// found from it too, and the shell stays where it is.
    Visit,
}

/// Whether `word` is a cluster of short options (`-rf`) that holds `letter`; one with a value
/// written against it (`-L./lib`) is not.
fn short_holds(word: &str, letter: char) -> bool {
    word.strip_prefix('-').is_some_and(|short| {
        !short.is_empty()
            && short.chars().all(|c| c.is_ascii_alphabetic())
            && short.contains(letter)
    })
}

/// What the value of a variable or a git setting that loads code holds (D-1418-10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Carries {
    /// A file or folder it loads code from.
    Path,
    /// Several, apart at `:` or a space.
    Paths,
    /// A command it runs.
    Command,
    /// Options for this program, which may load files.
    Options(&'static str),
    /// Perl's options: `-I<dir>` and `-M<Module>`.
    Perl,
}

/// The variables, and the git settings (`-c <key>=<value>`, any case), whose value loads code
/// (D-1418-10): named whatever is at the path, never only as a file.
const RUNS_CODE: [(&str, Carries); 36] = [
    ("BASH_ENV", Carries::Path),
    ("ENV", Carries::Path),
    ("ZDOTDIR", Carries::Path),
    ("PROMPT_COMMAND", Carries::Command),
    ("PYTHONSTARTUP", Carries::Path),
    ("NODE_OPTIONS", Carries::Options("node")),
    ("RUBYOPT", Carries::Options("ruby")),
    ("PERL5OPT", Carries::Perl),
    ("JAVA_TOOL_OPTIONS", Carries::Options("java")),
    ("JDK_JAVA_OPTIONS", Carries::Options("java")),
    ("_JAVA_OPTIONS", Carries::Options("java")),
    ("LD_PRELOAD", Carries::Paths),
    ("LD_AUDIT", Carries::Paths),
    ("DYLD_INSERT_LIBRARIES", Carries::Paths),
    ("GIT_SSH", Carries::Path),
    ("GIT_SSH_COMMAND", Carries::Command),
    ("GIT_EXTERNAL_DIFF", Carries::Command),
    ("GIT_PAGER", Carries::Command),
    ("GIT_EDITOR", Carries::Command),
    ("GIT_SEQUENCE_EDITOR", Carries::Command),
    ("GIT_ASKPASS", Carries::Path),
    ("GIT_PROXY_COMMAND", Carries::Command),
    ("SSH_ASKPASS", Carries::Path),
    ("EDITOR", Carries::Command),
    ("VISUAL", Carries::Command),
    ("PAGER", Carries::Command),
    ("core.hooksPath", Carries::Path),
    ("core.fsmonitor", Carries::Path),
    ("core.sshCommand", Carries::Command),
    ("core.pager", Carries::Command),
    ("core.editor", Carries::Command),
    ("core.askPass", Carries::Path),
    ("sequence.editor", Carries::Command),
    ("diff.external", Carries::Command),
    ("gpg.program", Carries::Path),
    ("credential.helper", Carries::Command),
];

/// What `name`'s value holds where it loads code ([`RUNS_CODE`]): a variable as written, a git
/// setting in any case.
fn runs_code(name: &str) -> Option<Carries> {
    RUNS_CODE.iter().find_map(|(it, carries)| {
        let same = if it.contains('.') {
            it.eq_ignore_ascii_case(name)
        } else {
            *it == name
        };
        same.then_some(*carries)
    })
}

/// `name` without a version after it where it is a program purlis reads (`python3.12`,
/// `node22`, `bash5`); any other name as it is.
fn plain_name(name: &str) -> &str {
    let plain = name.trim_end_matches(|c: char| c.is_ascii_digit() || c == '.');
    let known = LOADS.iter().any(|it| it.programs.contains(&plain)) || RUNNERS.contains(&plain);
    if plain != name && !plain.is_empty() && known {
        plain
    } else {
        name
    }
}

/// A command a launcher runs ([`launched`]).
#[derive(Debug, Default)]
struct Launched {
    /// The command, its program first.
    command: Vec<String>,
    /// The launcher's own words that are still words: variables `env` sets.
    words: Vec<String>,
    /// Folders the command runs in (`uv --directory D`).
    folders: Vec<String>,
}

/// The command the launcher `name` runs with `rest` (D-1418-9): `env`, `exec`, `nohup`,
/// `npx`/`bunx`, `pnpm exec`/`dlx`, `yarn`, `uv run`, `poetry run` and `pipenv run`. A
/// package runner's command is read only where it is a program purlis reads ([`LOADS`]);
/// anything else is a package's own tool, whose words are read as they were.
fn launched(name: &str, rest: &[String]) -> Option<Launched> {
    let known = |word: &String| {
        let word = unversioned(word);
        let plain = plain_name(word.rsplit('/').next().unwrap_or(word));
        // A shell's own `.` and `source` are no package's program.
        !matches!(plain, "." | "source") && LOADS.iter().any(|it| it.programs.contains(&plain))
    };
    let mut out = Launched::default();
    let mut words = rest.iter().peekable();
    match name {
        "env" => {
            while let Some(word) = words.peek() {
                match word.as_str() {
                    "-u" | "--unset" => {
                        words.next();
                        words.next();
                    }
                    "-C" | "--chdir" => {
                        words.next();
                        out.folders.extend(words.next().cloned());
                    }
                    // BSD's: where the program is looked up, not a folder anything runs in.
                    "-P" | "-S" => {
                        words.next();
                        words.next();
                    }
                    "--" => {
                        words.next();
                    }
                    _ if word.starts_with("--chdir=") || word.starts_with("-C") => {
                        let folder = word
                            .strip_prefix("--chdir=")
                            .or_else(|| word.strip_prefix("-C"))
                            .unwrap_or_default();
                        out.folders.push(folder.to_owned());
                        words.next();
                    }
                    _ if word.starts_with('-') => {
                        words.next();
                    }
                    _ if word.contains('=') => {
                        out.words.extend(words.next().cloned());
                    }
                    _ => break,
                }
            }
        }
        "exec" => {
            while let Some(word) = words.peek() {
                if *word == "-a" {
                    words.next();
                    words.next();
                } else if word.starts_with('-') {
                    words.next();
                } else {
                    break;
                }
            }
        }
        "nohup" => {}
        "npx" | "bunx" | "pnpx" => {
            skip_options(&mut words, &["-p", "--package", "-c", "--call"]);
            if !words.peek().is_some_and(|word| known(word)) {
                return None;
            }
        }
        "pnpm" | "yarn" => {
            if words
                .peek()
                .is_some_and(|word| matches!(word.as_str(), "exec" | "dlx"))
            {
                words.next();
                skip_options(&mut words, &["-p", "--package"]);
            }
            if !words.peek().is_some_and(|word| known(word)) {
                return None;
            }
        }
        "uv" => {
            loop {
                let word = words.next()?;
                match word.as_str() {
                    "--directory" | "--project" => out.folders.extend(words.next().cloned()),
                    "--cache-dir" | "--config-file" | "--color" => {
                        words.next();
                    }
                    "run" => break,
                    _ if let Some(folder) = uv_folder(word) => out.folders.push(folder),
                    _ if word.starts_with('-') => {}
                    _ => return None,
                }
            }
            loop {
                let word = words.peek()?;
                match word.as_str() {
                    "--directory" | "--project" => {
                        words.next();
                        out.folders.extend(words.next().cloned());
                    }
                    "--with"
                    | "--with-requirements"
                    | "--with-editable"
                    | "--python"
                    | "-p"
                    | "--extra"
                    | "--group"
                    | "--package"
                    | "--env-file"
                    | "--index" => {
                        words.next();
                        words.next();
                    }
                    "--" => {
                        words.next();
                    }
                    // `uv run -m mod` runs the module with Python.
                    "-m" | "--module" => {
                        words.next();
                        out.command.extend(["python".to_owned(), "-m".to_owned()]);
                        break;
                    }
                    _ if let Some(folder) = uv_folder(word) => {
                        out.folders.push(folder);
                        words.next();
                    }
                    _ if word.starts_with('-') => {
                        words.next();
                    }
                    // A folder: what a runner loads from it, read as uv's own words (D-1327-9).
                    _ if matches!(word.as_str(), "." | "..") || word.ends_with('/') => {
                        return None;
                    }
                    // `uv run x.py` runs it with Python.
                    _ if word.ends_with(".py") || word.ends_with(".pyw") => {
                        out.command.push("python".to_owned());
                        break;
                    }
                    _ => break,
                }
            }
        }
        "poetry" | "pipenv" => {
            if words.next().is_none_or(|word| word != "run") {
                return None;
            }
        }
        _ => return None,
    }
    out.command.extend(words.cloned());
    // A package runner's `tsx@4` is the program `tsx`.
    if let Some(program) = out.command.first_mut() {
        *program = unversioned(program).to_owned();
    }
    (!out.command.is_empty()).then_some(out)
}

/// The folder of uv's `--directory=<dir>` or `--project=<dir>`.
fn uv_folder(word: &str) -> Option<String> {
    word.strip_prefix("--directory=")
        .or_else(|| word.strip_prefix("--project="))
        .map(str::to_owned)
}

/// A package's name without the version a package runner is given (`tsx@4`); a scoped name's
/// own `@` (`@scope/x`) is kept.
fn unversioned(word: &str) -> &str {
    match word.rfind('@') {
        Some(at) if at > 0 => &word[..at],
        _ => word,
    }
}

/// Takes the options at the front of `words`: those in `valued` with the word after them.
fn skip_options(words: &mut std::iter::Peekable<std::slice::Iter<'_, String>>, valued: &[&str]) {
    while let Some(word) = words.peek() {
        if !word.starts_with('-') {
            break;
        }
        let takes = valued.contains(&word.as_str());
        words.next();
        if takes {
            words.next();
        }
    }
}

/// `segment`'s words, sorted ([`Read`]).
fn read_segment(segment: &[String]) -> Read {
    let mut read = Read::default();
    // Variables set for the command come first, and are words like any other.
    let assigning = |word: &String| {
        word.split_once('=').is_some_and(|(name, _)| {
            !name.is_empty()
                && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                && !name.starts_with(|c: char| c.is_ascii_digit())
        })
    };
    let at = segment.iter().take_while(|word| assigning(word)).count();
    read.words.extend(segment[..at].iter().cloned());
    let Some(program) = segment.get(at) else {
        return read;
    };
    let rest = &segment[at + 1..];
    let name = plain_name(program.rsplit('/').next().unwrap_or(program));
    read.words.push(program.clone());
    // A launcher's command, read as a command of its own (D-1418-9).
    if let Some(launched) = launched(name, rest) {
        let mut inner = read_segment(&launched.command);
        read.words.extend(launched.words);
        read.words.append(&mut inner.words);
        inner.words = read.words;
        let mut moves: Vec<Move> = launched
            .folders
            .into_iter()
            .map(|word| Move {
                kind: MoveKind::Visit,
                word,
            })
            .collect();
        moves.append(&mut inner.moves);
        inner.moves = moves;
        return inner;
    }
    read.runner = RUNNERS.contains(&name)
        || (matches!(name, "go" | "uv") && rest.first().is_some_and(|word| word == "run"));
    read.files = files_of(name, rest);
    read.search = search_of(name);
    let mut words = rest.iter();
    if name == "popd" {
        read.moves.push(Move {
            kind: MoveKind::Pop,
            word: program.clone(),
        });
        return read;
    }
    if matches!(name, "cd" | "pushd") {
        for word in words {
            if word == "-" && name == "cd" {
                read.moves.push(Move {
                    kind: MoveKind::Back,
                    word: word.clone(),
                });
                break;
            }
            if word.starts_with('-') {
                continue;
            }
            read.moves.push(Move {
                kind: if name == "cd" {
                    MoveKind::To
                } else {
                    MoveKind::Push
                },
                word: word.clone(),
            });
            break;
        }
        return read;
    }
    if name == "git" {
        while let Some(word) = words.next() {
            if word == "-C" {
                // The folder git runs in, which no later command does.
                words.next();
            } else if word == "-c" {
                // A setting, read as an assignment is: one that names code git runs is named
                // whatever it is ([`RUNS_CODE`]).
                read.words.extend(words.next().cloned());
            } else {
                read.words.push(word.clone());
            }
        }
        return read;
    }
    if SHELLS.contains(&name) {
        while let Some(word) = words.next() {
            if short_holds(word, 'c') {
                read.commands.extend(words.next().cloned());
            } else {
                read.words.push(word.clone());
            }
        }
        return read;
    }
    if INERT_TEXT.contains(&name) {
        let from_file = rest.iter().any(|word| {
            word == "--from-file" || word.starts_with("--from-file=") || short_leads_with(word, 'f')
        });
        let mut text_given = from_file;
        while let Some(word) = words.next() {
            match word.as_str() {
                "--arg" | "--argjson" | "--slurpfile" | "--rawfile" => {
                    read.words.extend(words.by_ref().take(2).cloned());
                }
                "--from-file" | "--indent" | "-L" => read.words.extend(words.next().cloned()),
                _ if short_holds(word, 'f') || short_holds(word, 'L') => {
                    read.words.push(word.clone());
                    read.words.extend(words.next().cloned());
                }
                _ if word.starts_with('-') => read.words.push(word.clone()),
                // jq's program, which runs nothing.
                _ if !text_given => text_given = true,
                _ => read.words.push(word.clone()),
            }
        }
        return read;
    }
    read.words.extend(rest.iter().cloned());
    read
}

/// How a program finds a file of code it is given by name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Finds {
    /// As named.
    Exact,
    /// As named, or with one of these after it where it has none of them.
    Extensions(&'static [&'static str]),
    /// A Python module, `a.b`: `a/b.py` or the package folder `a/b`.
    Module,
    /// A Perl module, `A::B`, which Perl loads from `A/B.pm` ([`perl_module_file`]).
    PerlModule,
}

/// A word a program opens as a file of code, and how it finds it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct FileWord {
    value: String,
    finds: Finds,
    /// An option's value, which the program also looks for along its search path where it is
    /// a bare name ([`Search`]); never the script operand.
    searched: bool,
}

/// The search path a program also looks a bare name up along, past the folder it runs in
/// (#1425): Node's `NODE_PATH` and `node_modules` for what it is asked to load first (`-r`,
/// `--import`), gawk's `AWKPATH` for `-f` and `-i`, Ruby's load path for `-r`, and Perl's
/// `@INC` for `-M`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Search {
    /// `NODE_PATH`, a colon list, and the `node_modules` in each folder up from where the
    /// command runs ([`node_modules_up`]).
    Node,
    /// `AWKPATH`, a colon list.
    Awk,
    /// `RUBYLIB`, a colon list, and Ruby's own `-I<folder>` options, each one with a `/` (a
    /// bare `-I lib` stays unnamed, D-1418-1).
    Ruby,
    /// `PERL5LIB`, a colon list, and Perl's own `-I<folder>` options with a `/`, as Ruby's.
    Perl,
}

/// The search path the program `name` follows, if it is one purlis reads ([`Search`]).
fn search_of(name: &str) -> Option<Search> {
    match name {
        "node" | "ts-node" | "tsx" | "bun" => Some(Search::Node),
        "awk" | "gawk" | "mawk" | "nawk" => Some(Search::Awk),
        "ruby" => Some(Search::Ruby),
        "perl" => Some(Search::Perl),
        _ => None,
    }
}

/// The folders, as written, `search` reads off a command's `words`: the variable's colon list,
/// set in front of the command or by a launcher (`env NODE_PATH=./lib node …`), and Ruby's or
/// Perl's `-I` options, attached (`-I./lib`) or apart (`-I ./lib`).
fn search_folders(search: Search, words: &[String]) -> Vec<String> {
    let (variable, includes) = match search {
        Search::Node => ("NODE_PATH", false),
        Search::Awk => ("AWKPATH", false),
        Search::Ruby => ("RUBYLIB", true),
        Search::Perl => ("PERL5LIB", true),
    };
    let mut out: Vec<String> = words
        .iter()
        .filter_map(|word| word.strip_prefix(variable)?.strip_prefix('='))
        .flat_map(|list| list.split(':'))
        .filter(|folder| !folder.is_empty())
        .map(str::to_owned)
        .collect();
    if includes {
        let mut words = words.iter();
        while let Some(word) = words.next() {
            let folder = match word.strip_prefix("-I") {
                Some("") => words.next().cloned(),
                Some(folder) => Some(folder.to_owned()),
                None => None,
            };
            out.extend(folder.filter(|folder| folder.contains('/')));
        }
    }
    out
}

/// The `node_modules` folder in each of `bases` and in every folder above it, up to the
/// nearest of `stops` (the project, the home folder, a folder chats may be granted, the cache
/// home), where Node looks a bare name up (#1425); and the home folder's own. Each is absolute,
/// so [`Scan::segment`] keeps only those a chat could write.
///
/// Bounded so an ordinary project stays well under [`MOST_NAMED`], past which its chat is
/// refused: a hook run in the project is not followed up through every folder between the
/// project and the home folder, which a chat writes only once you grant it the home folder.
fn node_modules_up(bases: &[PathBuf], stops: &[&Path], home: Option<&Path>) -> Vec<String> {
    let stops: Vec<PathBuf> = stops.iter().map(|stop| lexical(stop)).collect();
    let mut out: Vec<String> = Vec::new();
    let mut push = |folder: &Path| {
        let one = folder.join("node_modules").display().to_string();
        if !out.contains(&one) {
            out.push(one);
        }
    };
    for base in bases {
        for folder in lexical(base).ancestors() {
            push(folder);
            if stops.iter().any(|stop| stop == folder) {
                break;
            }
        }
    }
    if let Some(home) = home {
        push(&lexical(home));
    }
    out
}

/// The folder of the package `path` is in or is, below the nearest `node_modules` above it:
/// `node_modules/<name>`, or `node_modules/@<scope>/<name>` (#1425). None for a path in no
/// `node_modules`, for the `node_modules` folder itself, and for a scope's folder.
fn package_folder(path: &Path) -> Option<PathBuf> {
    let parts: Vec<std::path::Component<'_>> = path.components().collect();
    let at = parts
        .iter()
        .rposition(|part| part.as_os_str() == "node_modules")?;
    let first = parts.get(at + 1)?.as_os_str().to_str()?;
    let deep = at + if first.starts_with('@') { 3 } else { 2 };
    if parts.len() < deep
        || !parts[at + 1..deep]
            .iter()
            .all(|part| matches!(part, std::path::Component::Normal(_)))
    {
        return None;
    }
    Some(parts[..deep].iter().collect())
}

/// Whether `path` is written without `..`, so a folder it starts with is one it is in.
fn has_no_parent(path: &Path) -> bool {
    !path
        .components()
        .any(|part| matches!(part, std::path::Component::ParentDir))
}

/// The most of a package's `package.json` read for what it loads ([`package_entries`]).
#[cfg(unix)]
const MOST_MANIFEST: u64 = 256 * 1024;

/// The most entries one package's manifest is followed for ([`package_entries`]). A package
/// below `node_modules` is named whole, which holds every file Node maps a subpath to, so this
/// bounds only what a manifest a chat may have written costs to read.
const MOST_ENTRIES: usize = 64;

/// How deep an `exports` map's nested conditions are followed.
const MOST_EXPORTS_DEPTH: usize = 8;

/// The files a package Node found in `node_modules` at `dir` says are loaded from it, where
/// `dir` is in a `node_modules` and its `package.json` says any: its `main`, its `module` (a
/// bundler's entry), and each file its `exports` maps to, under every subpath and condition
/// (#1425). None for any other folder, whose manifest names a project's own code. A target
/// written from `/` is left out, as one is for `main` (#1425), and so is an `exports` pattern.
///
/// A chat may have written the package, so its manifest is read as untrusted: the folder and
/// the file are no link, the file is opened non-blocking and read only if it is a regular file
/// of at most [`MOST_MANIFEST`] bytes, so nothing planted there holds a chat's start; and at
/// most [`MOST_ENTRIES`] targets, [`MOST_EXPORTS_DEPTH`] conditions deep, are followed.
fn package_entries(dir: &Path) -> Vec<PathBuf> {
    if !dir
        .components()
        .any(|part| part.as_os_str() == "node_modules")
        || !std::fs::symlink_metadata(dir).is_ok_and(|meta| meta.is_dir())
    {
        return Vec::new();
    }
    let Some(value) = small_file(&dir.join("package.json"))
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
    else {
        return Vec::new();
    };
    // `main` is Node's to resolve, `..` and all, as it always was here. `module` and every
    // `exports` target name a file of the package itself: Node refuses an `exports` target that
    // does not start `./` or that has a `..` or `node_modules` part (train 79 review), so such a
    // target names nothing it loads, and a manifest never makes this name a path outside.
    let mut targets: Vec<&str> = value
        .get("main")
        .and_then(|v| v.as_str())
        .into_iter()
        .collect();
    // A bundler's `module` is often written without the `./`, which it means all the same.
    targets.extend(
        value
            .get("module")
            .and_then(|v| v.as_str())
            .filter(|module| {
                within_the_package(&format!("./{}", module.trim().trim_start_matches("./")))
            }),
    );
    let mut exported: Vec<&str> = Vec::new();
    if let Some(exports) = value.get("exports") {
        export_targets(exports, 0, &mut exported);
    }
    targets.extend(
        exported
            .into_iter()
            .filter(|target| within_the_package(target.trim())),
    );
    let mut out: Vec<PathBuf> = Vec::new();
    for target in targets {
        let target = target.trim();
        if target.is_empty() || target.starts_with('/') {
            continue;
        }
        let path = lexical(&dir.join(target));
        if !out.contains(&path) {
            out.push(path);
        }
    }
    out
}

/// Whether `target`, a `module` or an `exports` target, is one Node would take as a file of the
/// package: `./` first, and no `..`, `.` or `node_modules` part after it.
fn within_the_package(target: &str) -> bool {
    target.strip_prefix("./").is_some_and(|rest| {
        !rest.is_empty()
            && rest.split(['/', '\\']).all(|part| {
                !part.is_empty() && part != ".." && part != "." && part != "node_modules"
            })
    })
}

/// Each file `exports` (or a part of it, `depth` conditions down) maps to, into `into`: a
/// string is one, a fallback list and a map of subpaths or conditions each of theirs.
fn export_targets<'a>(exports: &'a serde_json::Value, depth: usize, into: &mut Vec<&'a str>) {
    if depth > MOST_EXPORTS_DEPTH || into.len() >= MOST_ENTRIES {
        return;
    }
    match exports {
        // A pattern (`./features/*.js`) names no one file: the package's folder, named whole,
        // holds what it maps to.
        serde_json::Value::String(target) if target.contains('*') => {}
        serde_json::Value::String(target) => into.push(target),
        serde_json::Value::Array(items) => {
            for item in items {
                export_targets(item, depth + 1, into);
            }
        }
        serde_json::Value::Object(map) => {
            for item in map.values() {
                export_targets(item, depth + 1, into);
            }
        }
        _ => {}
    }
}

/// `path`'s text, opened following no link and without waiting on a FIFO, where it is a
/// regular file of at most [`MOST_MANIFEST`] bytes.
#[cfg(unix)]
fn small_file(path: &Path) -> Option<String> {
    use rustix::fs::{CWD, Mode, OFlags};
    use std::io::Read as _;
    let flags = OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC;
    let file = std::fs::File::from(rustix::fs::openat(CWD, path, flags, Mode::empty()).ok()?);
    let meta = file.metadata().ok()?;
    if !meta.is_file() || meta.len() > MOST_MANIFEST {
        return None;
    }
    let mut text = String::new();
    file.take(MOST_MANIFEST).read_to_string(&mut text).ok()?;
    Some(text)
}

/// Where no link can be refused at the open, nothing is read.
#[cfg(not(unix))]
fn small_file(_: &Path) -> Option<String> {
    None
}

/// Whether a program looks `value` up along its search path: a bare name or a package path
/// (`hook`, `pkg/register`), not one written from a folder (`./hook`, `/opt/hook`, `~/hook`).
fn looked_up(value: &str) -> bool {
    !value.is_empty() && !value.starts_with(['.', '/', '~', '$'])
}

/// The file Perl loads for `-M<module>` or `-m<module>` (`-MA::B=x` and `-M-A`, `no A`, too):
/// `A/B.pm`, which it looks up along `@INC`.
fn perl_module_file(module: &str) -> String {
    let module = module.trim_start_matches('-');
    let module = module
        .split(|c: char| c == '=' || c.is_whitespace())
        .next()
        .unwrap_or_default();
    format!("{}.pm", module.replace("::", "/"))
}

/// What Node, and the runners that load as it does, try after a name given without its
/// extension.
const JS: &[&str] = &[
    ".js", ".mjs", ".cjs", ".json", ".node", ".ts", ".mts", ".cts", ".tsx", ".jsx",
];

/// Node's long options whose value is a file of code it loads first.
const NODE_FILES: &[(&str, Finds)] = &[
    ("--require", Finds::Extensions(JS)),
    ("--import", Finds::Extensions(JS)),
    ("--loader", Finds::Extensions(JS)),
    ("--experimental-loader", Finds::Extensions(JS)),
];

/// Node's long options whose value is anything else (#1418 review F1): never its script.
const NODE_VALUED: &[&str] = &[
    "--conditions",
    "--title",
    "--env-file",
    "--env-file-if-exists",
    "--input-type",
    "--inspect-port",
    "--stack-size",
    "--watch-path",
    "--experimental-default-type",
    "--diagnostic-dir",
    "--report-dir",
    "--redirect-warnings",
];

/// The programs that open a file of code they are given by name, and where on their command
/// line it is (D-1418-1).
struct Loads {
    programs: &'static [&'static str],
    /// The script: the first operand, how it is found. `None` where the first operand is
    /// program text or data.
    script: Option<Finds>,
    /// Short options whose value is a file of code, how it is found, and whether that file is
    /// the script, so no operand is.
    letters: &'static [(char, Finds, bool)],
    /// Long options whose value is a file of code.
    long: &'static [(&'static str, Finds)],
    /// Short options, and long ones, whose value is anything else.
    valued: &'static [char],
    valued_long: &'static [&'static str],
    /// Options after which there is no script: code given on the line (`-c`, `-e`).
    ends: &'static [char],
    ends_long: &'static [&'static str],
    /// How its own subcommands come before the script ([`Sub`]).
    sub: Sub,
}

/// How a program's own subcommands stand before its script (#1418 review M1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sub {
    /// It has none: its first operand is the script.
    None,
    /// deno: a script only after `run`; any other subcommand (`test`, `task`, `fmt`) ends it.
    Deno,
    /// bun: after `run`, or alone, a word is a file only with a `/` or a JS extension, and
    /// otherwise a package script's name or one of bun's own subcommands (`test`, `install`,
    /// `x`), none of which carries either.
    Bun,
    /// tsx: `watch` comes before the script.
    Tsx,
}

/// [`Loads`] for each program (D-1418-1, D-1418-2).
const LOADS: [Loads; 14] = [
    Loads {
        programs: &SHELLS,
        script: Some(Finds::Exact),
        letters: &[],
        long: &[("--rcfile", Finds::Exact), ("--init-file", Finds::Exact)],
        valued: &['o', 'O'],
        valued_long: &[],
        ends: &['c'],
        ends_long: &[],
        sub: Sub::None,
    },
    Loads {
        programs: &["source", "."],
        script: Some(Finds::Exact),
        letters: &[],
        long: &[],
        valued: &[],
        valued_long: &[],
        ends: &[],
        ends_long: &[],
        sub: Sub::None,
    },
    Loads {
        programs: &["node", "ts-node"],
        script: Some(Finds::Extensions(JS)),
        letters: &[('r', Finds::Extensions(JS), false)],
        long: NODE_FILES,
        valued: &['C'],
        valued_long: NODE_VALUED,
        ends: &['e', 'p'],
        ends_long: &["--eval", "--print", "--run"],
        sub: Sub::None,
    },
    Loads {
        programs: &["tsx"],
        script: Some(Finds::Extensions(JS)),
        letters: &[('r', Finds::Extensions(JS), false)],
        long: NODE_FILES,
        valued: &['C'],
        valued_long: NODE_VALUED,
        ends: &['e', 'p'],
        ends_long: &["--eval", "--print", "--run"],
        sub: Sub::Tsx,
    },
    Loads {
        programs: &["bun"],
        script: Some(Finds::Extensions(JS)),
        letters: &[('r', Finds::Extensions(JS), false)],
        long: &[
            ("--preload", Finds::Extensions(JS)),
            ("--require", Finds::Extensions(JS)),
            ("--import", Finds::Extensions(JS)),
        ],
        valued: &['c'],
        valued_long: &[
            "--config",
            "--cwd",
            "--env-file",
            "--tsconfig-override",
            "--filter",
        ],
        ends: &['e', 'p'],
        ends_long: &["--eval", "--print"],
        sub: Sub::Bun,
    },
    Loads {
        programs: &["deno"],
        script: Some(Finds::Extensions(JS)),
        letters: &[],
        long: &[("--preload", Finds::Extensions(JS))],
        valued: &['c', 'L'],
        valued_long: &[
            "--config",
            "--import-map",
            "--cert",
            "--location",
            "--seed",
            "--log-level",
        ],
        ends: &[],
        ends_long: &["--eval"],
        sub: Sub::Deno,
    },
    Loads {
        programs: &["make", "gmake"],
        script: None,
        letters: &[('f', Finds::Exact, false)],
        long: &[("--file", Finds::Exact), ("--makefile", Finds::Exact)],
        valued: &['C', 'I', 'o', 'W'],
        valued_long: &["--directory", "--include-dir"],
        ends: &[],
        ends_long: &[],
        sub: Sub::None,
    },
    Loads {
        programs: &["python", "python3"],
        script: Some(Finds::Exact),
        letters: &[('m', Finds::Module, true)],
        long: &[],
        valued: &['W', 'X', 'Q'],
        valued_long: &[],
        ends: &['c'],
        ends_long: &[],
        sub: Sub::None,
    },
    Loads {
        programs: &["ruby"],
        script: Some(Finds::Exact),
        letters: &[('r', Finds::Extensions(&[".rb", ".so", ".bundle"]), false)],
        long: &[("--require", Finds::Extensions(&[".rb", ".so", ".bundle"]))],
        valued: &['I', 'C', 'E'],
        valued_long: &[],
        ends: &['e'],
        ends_long: &[],
        sub: Sub::None,
    },
    Loads {
        programs: &["perl"],
        script: Some(Finds::Exact),
        letters: &[
            ('M', Finds::PerlModule, false),
            ('m', Finds::PerlModule, false),
        ],
        long: &[],
        valued: &['I', 'x'],
        valued_long: &[],
        ends: &['e', 'E'],
        ends_long: &[],
        sub: Sub::None,
    },
    Loads {
        programs: &["php"],
        script: Some(Finds::Exact),
        letters: &[('f', Finds::Exact, true)],
        long: &[],
        valued: &['c', 'd', 'z'],
        valued_long: &[],
        ends: &['r'],
        ends_long: &[],
        sub: Sub::None,
    },
    Loads {
        programs: &["awk", "gawk", "mawk", "nawk"],
        script: None,
        letters: &[
            ('f', Finds::Extensions(&[".awk"]), false),
            ('E', Finds::Extensions(&[".awk"]), false),
            ('i', Finds::Extensions(&[".awk"]), false),
        ],
        long: &[
            ("--file", Finds::Extensions(&[".awk"])),
            ("--exec", Finds::Extensions(&[".awk"])),
            ("--include", Finds::Extensions(&[".awk"])),
        ],
        valued: &['F', 'v'],
        valued_long: &["--field-separator", "--assign"],
        ends: &[],
        ends_long: &[],
        sub: Sub::None,
    },
    Loads {
        programs: &["sed", "gsed"],
        script: None,
        letters: &[('f', Finds::Exact, false)],
        long: &[("--file", Finds::Exact)],
        valued: &['e', 'l'],
        valued_long: &["--expression"],
        ends: &[],
        ends_long: &[],
        sub: Sub::None,
    },
    Loads {
        programs: &INERT_TEXT,
        script: None,
        letters: &[('f', Finds::Exact, false)],
        long: &[("--from-file", Finds::Exact)],
        valued: &['L'],
        valued_long: &["--indent"],
        ends: &[],
        ends_long: &[],
        sub: Sub::None,
    },
];

/// The words of `rest`, the words after the program `name`, that it opens as a file of code
/// ([`LOADS`]): the script operand, and each value of an option that names such a file,
/// attached (`-fsum.awk`, `--file=sum.awk`) or given apart.
///
/// Each is read by the program's own options, not guessed at, so one that is absolute is named
/// wherever it is, as a separate word is: unlike a later letter's split of a cluster whose
/// letters purlis does not know (D-1356-7).
fn files_of(name: &str, rest: &[String]) -> Vec<FileWord> {
    let Some(loads) = LOADS.iter().find(|it| it.programs.contains(&name)) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut words = rest.iter();
    let mut first_operand = true;
    while let Some(word) = words.next() {
        if word == "--" {
            if let (Some(finds), Some(script)) = (loads.script, words.next()) {
                out.push(FileWord {
                    value: script.clone(),
                    finds,
                    searched: false,
                });
            }
            break;
        }
        if let Some(long) = word.strip_prefix("--") {
            let (flag, value) = match long.split_once('=') {
                Some((flag, value)) => (format!("--{flag}"), Some(value.to_owned())),
                None => (word.clone(), None),
            };
            if let Some((_, finds)) = loads.long.iter().find(|(it, _)| *it == flag) {
                if let Some(value) = value.or_else(|| words.next().cloned()) {
                    out.push(FileWord {
                        value,
                        finds: *finds,
                        searched: true,
                    });
                }
            } else if loads.ends_long.contains(&flag.as_str()) {
                return out;
            } else if value.is_none() && loads.valued_long.contains(&flag.as_str()) {
                words.next();
            }
            continue;
        }
        if let Some(short) = word
            .strip_prefix('-')
            .or_else(|| word.strip_prefix('+'))
            .filter(|short| !short.is_empty())
        {
            for (at, letter) in short.char_indices() {
                let value = &short[at + letter.len_utf8()..];
                if let Some((_, finds, script)) =
                    loads.letters.iter().find(|(it, ..)| *it == letter)
                {
                    let value = if value.is_empty() {
                        words.next().cloned()
                    } else {
                        Some(value.to_owned())
                    };
                    out.extend(value.map(|value| match finds {
                        Finds::PerlModule => FileWord {
                            value: perl_module_file(&value),
                            finds: Finds::Exact,
                            searched: true,
                        },
                        _ => FileWord {
                            value,
                            finds: *finds,
                            searched: true,
                        },
                    }));
                    if *script {
                        return out;
                    }
                    break;
                }
                if loads.valued.contains(&letter) {
                    if value.is_empty() {
                        words.next();
                    }
                    break;
                }
                if loads.ends.contains(&letter) {
                    return out;
                }
                if !letter.is_ascii_alphanumeric() {
                    break;
                }
            }
            continue;
        }
        let first = std::mem::replace(&mut first_operand, false);
        match loads.sub {
            Sub::Deno if first && word == "run" => continue,
            Sub::Deno if first => return out,
            Sub::Tsx if first && word == "watch" => continue,
            Sub::Bun if first && word == "run" => continue,
            // A package script's name, not a file (`bun run lint`, `bun dev`).
            Sub::Bun if !word.contains('/') && !JS.iter().any(|it| word.ends_with(it)) => {
                return out;
            }
            _ => {}
        }
        if let Some(finds) = loads.script {
            out.push(FileWord {
                value: word.clone(),
                finds,
                searched: false,
            });
            break;
        }
    }
    out
}

/// Whether `word`, as a shell left it, reads as a path a harness could run: it holds a `/`, and
/// is not only `/`s, a URL, an option or a device. A space is a path's only where no part of it
/// between `/`s starts or ends with one, as `.a // .b` does and `tools/my hook.sh` does not.
///
/// Any other word is kept, odd characters and all: one that is not a path names a file nobody
/// runs, which costs nothing, while one left out could be a script a chat may then write.
fn reads_as_a_path(word: &str) -> bool {
    word.contains('/')
        && !word.chars().all(|c| c == '/')
        && !word.contains("://")
        && !word.starts_with('-')
        && !word.starts_with("/dev/")
        && word.split('/').all(|part| {
            part.trim() == part && !(part.is_empty() && word.contains(char::is_whitespace))
        })
}

/// `line` as a POSIX shell splits it: each simple command's words, with quotes taken off, and
/// without its operators (`|`, `&&`, `;`, …) or its redirections and their targets.
fn shell_segments(line: &str) -> Vec<Segment> {
    let mut segments = Vec::new();
    let mut words: Vec<String> = Vec::new();
    let mut word = String::new();
    let mut in_word = false;
    let mut redirected = false;
    let mut chars = line.chars().peekable();
    let finish =
        |word: &mut String, in_word: &mut bool, redirected: &mut bool, words: &mut Vec<String>| {
            if *in_word {
                if *redirected {
                    *redirected = false;
                } else {
                    words.push(std::mem::take(word));
                }
            }
            word.clear();
            *in_word = false;
        };
    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                in_word = true;
                for c in chars.by_ref() {
                    if c == '\'' {
                        break;
                    }
                    word.push(c);
                }
            }
            '"' => {
                in_word = true;
                while let Some(c) = chars.next() {
                    match c {
                        '"' => break,
                        '\\' => {
                            if let Some(next) = chars.next() {
                                word.push(next);
                            }
                        }
                        _ => word.push(c),
                    }
                }
            }
            '\\' => {
                in_word = true;
                if let Some(next) = chars.next() {
                    word.push(next);
                }
            }
            '>' | '<' => {
                // A file descriptor's number before the operator is part of it.
                if in_word && word.chars().all(|c| c.is_ascii_digit()) {
                    word.clear();
                    in_word = false;
                }
                finish(&mut word, &mut in_word, &mut redirected, &mut words);
                if chars.peek() == Some(&'(') {
                    // `<(…)` and `>(…)`: commands, read as such.
                    continue;
                }
                let mut operator = c.to_string();
                while let Some(next) = chars.next_if(|c| matches!(c, '>' | '<' | '&' | '|' | '-')) {
                    operator.push(next);
                }
                // Where output goes, a descriptor, and a here-document's delimiter name nothing
                // that runs. What a command reads (`<`, `<<<`, `<>`) may be what a shell runs.
                redirected =
                    c == '>' || operator.contains('&') || matches!(operator.as_str(), "<<" | "<<-");
            }
            '|' | ';' | '&' | '\n' | '(' | ')' => {
                finish(&mut word, &mut in_word, &mut redirected, &mut words);
                redirected = false;
                if c == '&' && chars.peek() == Some(&'>') {
                    continue;
                }
                if !words.is_empty() {
                    segments.push(Segment::Words(std::mem::take(&mut words)));
                }
                match c {
                    '(' => segments.push(Segment::Open),
                    ')' => segments.push(Segment::Close),
                    _ => {}
                }
            }
            c if c.is_whitespace() => {
                finish(&mut word, &mut in_word, &mut redirected, &mut words);
            }
            _ => {
                in_word = true;
                word.push(c);
            }
        }
    }
    finish(&mut word, &mut in_word, &mut redirected, &mut words);
    if !words.is_empty() {
        segments.push(Segment::Words(words));
    }
    segments
}

/// One piece of a line a shell reads: a simple command's words, or where a subshell opens or
/// closes, after which the folder is again the one it was before.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Segment {
    Words(Vec<String>),
    Open,
    Close,
}

/// The commands under a [`COMMAND_KEYS`] key anywhere in `value`: a string as a line a shell
/// reads, a list of strings as words handed over as they are.
fn commands_in(value: &serde_json::Value, under: bool, into: &mut Vec<Command>) {
    match value {
        serde_json::Value::String(text) if under => into.push(Command::Line(text.clone())),
        serde_json::Value::Array(items)
            if under && !items.is_empty() && items.iter().all(serde_json::Value::is_string) =>
        {
            into.push(Command::Words(
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(str::to_owned))
                    .collect(),
            ));
        }
        serde_json::Value::Array(items) => {
            for item in items {
                commands_in(item, under, into);
            }
        }
        serde_json::Value::Object(map) => {
            // An MCP server's program and its arguments are one command: a shell's `-c` is
            // among the arguments.
            if let (Some(serde_json::Value::String(program)), Some(serde_json::Value::Array(args))) =
                (map.get("command"), map.get("args"))
                && args.iter().all(serde_json::Value::is_string)
            {
                let mut words = vec![program.clone()];
                words.extend(
                    args.iter()
                        .filter_map(|arg| arg.as_str().map(str::to_owned)),
                );
                into.push(Command::Words(words));
                for (key, item) in map {
                    if key != "command" && key != "args" {
                        commands_in(item, under || COMMAND_KEYS.contains(&key.as_str()), into);
                    }
                }
                return;
            }
            for (key, item) in map {
                commands_in(item, under || COMMAND_KEYS.contains(&key.as_str()), into);
            }
        }
        _ => {}
    }
}

/// `text` without `//` line comments, which `opencode.jsonc` may hold. A `//` inside a string,
/// as in a URL, is kept.
fn without_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        let mut in_string = false;
        let mut escaped = false;
        let mut cut = line.len();
        let bytes = line.as_bytes();
        for (at, byte) in bytes.iter().enumerate() {
            match byte {
                _ if escaped => escaped = false,
                b'\\' if in_string => escaped = true,
                b'"' => in_string = !in_string,
                b'/' if !in_string && bytes.get(at + 1) == Some(&b'/') => {
                    cut = at;
                    break;
                }
                _ => {}
            }
        }
        out.push_str(&line[..cut]);
        out.push('\n');
    }
    out
}

/// Everything resolved for the plane at `root` at a chat's start: what `core.hooksPath` names
/// and every script a harness's project config runs, in the plane and the clones in it.
/// `writable` is where a chat could write outside the project and the home folder
/// ([`Scan::holds`]).
pub fn resolved(
    root: &Path,
    home: Option<&Path>,
    xdg_config: Option<&Path>,
    writable: &[PathBuf],
) -> Result<Vec<Resolved>, Unread> {
    let dirs = directories(root);
    let clones = clones(&dirs);
    let mut found = hooks_paths(&clones, home, xdg_config);
    for dir in &dirs {
        found.extend(scripts_named(root, dir, home, writable)?);
    }
    Ok(once_each(found))
}

/// `found` with each path once, and without a path a folder named in it holds already, as a
/// rule on a folder holds everything below it (#1425); in order. A path written with `..` is
/// kept, as what it is below is not what it reads as.
fn once_each(mut found: Vec<Resolved>) -> Vec<Resolved> {
    let mut seen = std::collections::HashSet::new();
    found.retain(|it| seen.insert(it.path.clone()));
    found.retain(|it| {
        !has_no_parent(&it.path)
            || !it
                .path
                .ancestors()
                .skip(1)
                .any(|folder| seen.contains(folder))
    });
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hooks_path_is_read_from_the_core_section_in_any_case_and_quoting() {
        assert_eq!(
            hooks_path_in(
                "[user]\n\thooksPath = no\n[core]\n\tbare = false\n\tHooksPath = \"tools/hooks\"\n"
            ),
            Some("tools/hooks".to_owned())
        );
        assert_eq!(hooks_path_in("[core]\n\tbare = false\n"), None);
        assert_eq!(
            hooks_path_in("[core \"x\"]\nhookspath=h\n"),
            Some("h".to_owned())
        );
    }

    #[test]
    fn every_hooks_path_git_would_use_is_resolved_against_its_clone() {
        let plane = tempfile::tempdir().expect("a plane");
        let clone = plane.path().join("ws/repo");
        std::fs::create_dir_all(clone.join(".git")).expect("a clone");
        std::fs::write(
            clone.join(".git/config"),
            "[core]\n\thooksPath = scripts/hooks\n",
        )
        .expect("its config");
        let home = tempfile::tempdir().expect("a home");
        std::fs::write(
            home.path().join(".gitconfig"),
            "[core]\n\thooksPath = ~/my-hooks\n",
        )
        .expect("a global config");
        let found: Vec<PathBuf> = resolved(plane.path(), Some(home.path()), None, &[])
            .expect("read")
            .into_iter()
            .map(|it| it.path)
            .collect();
        assert!(found.contains(&clone.join("scripts/hooks")), "{found:?}");
        assert!(found.contains(&home.path().join("my-hooks")), "{found:?}");
    }

    #[test]
    fn a_worktree_s_hooks_path_is_read_from_its_common_git_directory() {
        let plane = tempfile::tempdir().expect("a plane");
        let main = plane.path().join("main");
        std::fs::create_dir_all(main.join(".git/worktrees/w")).expect("a main clone");
        std::fs::write(main.join(".git/config"), "[core]\nhooksPath = /abs/hooks\n")
            .expect("its config");
        std::fs::write(main.join(".git/worktrees/w/commondir"), "../..\n").expect("commondir");
        let worktree = plane.path().join("w");
        std::fs::create_dir_all(&worktree).expect("a worktree");
        std::fs::write(
            worktree.join(".git"),
            format!("gitdir: {}\n", main.join(".git/worktrees/w").display()),
        )
        .expect("its .git file");
        let found = hooks_paths(&[worktree], None, None);
        assert_eq!(
            found,
            [Resolved {
                path: PathBuf::from("/abs/hooks"),
                file: main.join(".git/worktrees/w/../../config"),
                word: "/abs/hooks".to_owned(),
            }]
        );
    }

    /// However much else the plane holds, each clone and branch folder purlis works in is held:
    /// a folder of more entries than the walk looks at does not keep them from being found.
    #[test]
    fn the_clones_purlis_works_in_are_held_whatever_the_walk_reaches() {
        let plane = tempfile::tempdir().expect("a plane");
        let at = |rel: &str| plane.path().join(rel);
        let git_dir = |rel: &str| {
            std::fs::create_dir_all(at(rel)).expect("a git folder");
            std::fs::write(at(rel).join("HEAD"), "ref: refs/heads/main\n").expect("HEAD");
        };
        git_dir("workspaces/w/repo/.git");
        git_dir("workspaces/w/repo/.git/worktrees/piece");
        std::fs::create_dir_all(at("workspaces/w/.worktrees/repo/piece")).expect("a piece");
        std::fs::write(
            at("workspaces/w/.worktrees/repo/piece/.git"),
            format!(
                "gitdir: {}\n",
                at("workspaces/w/repo/.git/worktrees/piece").display()
            ),
        )
        .expect("its .git file");
        // More entries than the walk looks at, in a folder beside the workspaces.
        std::fs::create_dir_all(at("aaa")).expect("a full folder");
        for n in 0..=LOOKED_AT {
            std::fs::write(at("aaa").join(n.to_string()), "").expect("an entry");
        }
        let pins = git_folder_pins(&[plane.path().to_path_buf()]);
        for pin in [
            "workspaces/w/repo/.git/config",
            "workspaces/w/.worktrees/repo/piece/.git",
        ] {
            assert!(pins.contains(&at(pin)), "{pin}: {pins:?}");
        }
        // A link in their place is never followed.
        let elsewhere = tempfile::tempdir().expect("elsewhere");
        git_dir_at(elsewhere.path());
        #[cfg(unix)]
        std::os::unix::fs::symlink(elsewhere.path(), at("workspaces/w/linked")).expect("a link");
        let pins = git_folder_pins(&[plane.path().to_path_buf()]);
        assert!(
            !pins
                .iter()
                .any(|pin| pin.starts_with(at("workspaces/w/linked"))),
            "{pins:?}"
        );
    }

    /// A clone at `dir`: a `.git` directory holding a `HEAD`.
    fn git_dir_at(dir: &Path) {
        std::fs::create_dir_all(dir.join(".git")).expect("a git folder");
        std::fs::write(dir.join(".git/HEAD"), "ref: refs/heads/main\n").expect("HEAD");
    }

    #[test]
    fn each_repository_s_git_folder_is_held_by_a_path_in_or_at_it() {
        let plane = tempfile::tempdir().expect("a plane");
        let at = |rel: &str| plane.path().join(rel);
        // A clone with a `.git` directory and a submodule.
        std::fs::create_dir_all(at("ws/repo/.git/modules/sub")).expect("a clone");
        std::fs::write(at("ws/repo/.git/HEAD"), "ref: refs/heads/main\n").expect("HEAD");
        std::fs::write(
            at("ws/repo/.git/modules/sub/HEAD"),
            "ref: refs/heads/main\n",
        )
        .expect("the submodule's HEAD");
        std::fs::create_dir_all(at("ws/repo/sub")).expect("the submodule");
        std::fs::write(at("ws/repo/sub/.git"), "gitdir: ../.git/modules/sub\n")
            .expect("the submodule's .git file");
        // A linked worktree of it, five folders down, as a workspace keeps its branch folders.
        std::fs::create_dir_all(at("ws/repo/.git/worktrees/b")).expect("its worktree record");
        std::fs::write(at("ws/repo/.git/worktrees/b/commondir"), "../..\n").expect("commondir");
        std::fs::write(at("ws/repo/.git/worktrees/b/HEAD"), "ref: refs/heads/b\n").expect("HEAD");
        std::fs::create_dir_all(at("workspaces/w/.worktrees/repo/b")).expect("a branch folder");
        std::fs::write(
            at("workspaces/w/.worktrees/repo/b/.git"),
            format!("gitdir: {}\n", at("ws/repo/.git/worktrees/b").display()),
        )
        .expect("its .git file");
        // A clone in a folder outside the plane.
        let elsewhere = tempfile::tempdir().expect("a folder elsewhere");
        std::fs::create_dir_all(elsewhere.path().join("x/.git")).expect("a clone there");

        let pins = git_folder_pins(&[plane.path().to_path_buf(), elsewhere.path().to_path_buf()]);
        for want in [
            at("ws/repo/.git/config"),
            at("ws/repo/.git/modules/sub/config"),
            at("ws/repo/sub/.git"),
            at("workspaces/w/.worktrees/repo/b/.git"),
            elsewhere.path().join("x/.git/config"),
        ] {
            assert!(pins.contains(&want), "{want:?} not in {pins:?}");
        }
        // Never the git folder itself where it is a directory, nor `..` in a path, nor a linked
        // worktree's own record, which `.git/worktrees` holds by name.
        assert!(!pins.contains(&at("ws/repo/.git")), "{pins:?}");
        assert!(
            !pins.contains(&at("ws/repo/.git/worktrees/b/config")),
            "{pins:?}"
        );
        assert!(
            pins.iter().all(|pin| !pin
                .components()
                .any(|c| c == std::path::Component::ParentDir)),
            "{pins:?}"
        );
        // Each once, though the clone, its submodule and its worktree all name it.
        let main = at("ws/repo/.git/config");
        assert_eq!(
            pins.iter().filter(|pin| **pin == main).count(),
            1,
            "{pins:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_git_link_is_never_followed_to_what_it_names() {
        let plane = tempfile::tempdir().expect("a plane");
        let root = plane.path();
        std::fs::write(root.join("config"), "").expect("a file of the plane's");
        std::fs::create_dir_all(root.join("ws/repo")).expect("a folder");
        std::os::unix::fs::symlink(root, root.join("ws/repo/.git")).expect("a .git link");
        let pins = git_folder_pins(&[root.to_path_buf()]);
        assert_eq!(pins, Vec::<PathBuf>::new());
    }

    #[test]
    fn a_git_file_naming_no_git_directory_holds_nothing() {
        let plane = tempfile::tempdir().expect("a plane");
        let at = |rel: &str| plane.path().join(rel);
        // A folder of the project's own, with a `config` and a `modules` in it, named as a git
        // directory by a `.git` file elsewhere.
        std::fs::create_dir_all(at("docs/modules/x")).expect("a project folder");
        std::fs::write(at("docs/config"), "").expect("a file in it");
        std::fs::write(at("docs/modules/x/HEAD"), "").expect("a HEAD below it");
        std::fs::create_dir_all(at("ws/repo")).expect("a folder");
        std::fs::write(
            at("ws/repo/.git"),
            format!("gitdir: {}\n", at("docs").display()),
        )
        .expect("a .git file");
        // And one naming a git directory whose `commondir` names the project's folder.
        std::fs::create_dir_all(at("ws/real/.git")).expect("a git directory");
        std::fs::write(at("ws/real/.git/HEAD"), "ref: refs/heads/main\n").expect("HEAD");
        std::fs::write(
            at("ws/real/.git/commondir"),
            at("docs").display().to_string(),
        )
        .expect("commondir");
        std::fs::create_dir_all(at("ws/other")).expect("a folder");
        std::fs::write(
            at("ws/other/.git"),
            format!("gitdir: {}\n", at("ws/real/.git").display()),
        )
        .expect("a .git file");
        let pins = git_folder_pins(&[plane.path().to_path_buf()]);
        assert!(
            pins.iter().all(|pin| !pin.starts_with(at("docs"))),
            "{pins:?}"
        );
        assert!(!pins.contains(&at("ws/repo/.git")), "{pins:?}");
        // The real git directory is held as itself; what its `commondir` names is not.
        assert!(pins.contains(&at("ws/other/.git")), "{pins:?}");
        assert!(pins.contains(&at("ws/real/.git/config")), "{pins:?}");
    }

    #[test]
    fn every_script_a_harness_s_project_config_runs_is_named() {
        let dir = tempfile::tempdir().expect("a project");
        let at = |rel: &str| dir.path().join(rel);
        std::fs::create_dir_all(at(".claude")).expect(".claude");
        std::fs::write(
            at(".claude/settings.json"),
            r#"{"hooks": {"Stop": [{"hooks": [{"type": "command", "command": "\"$CLAUDE_PROJECT_DIR\"/.claude/hooks/stop.sh --x"}]}]},
                "statusLine": {"command": "./bin/status.sh"}}"#,
        )
        .expect("settings");
        std::fs::write(
            at(".mcp.json"),
            r#"{"mcpServers": {"x": {"command": "node", "args": ["tools/server.js", "--port=3"]}}}"#,
        )
        .expect(".mcp.json");
        std::fs::write(
            at("opencode.jsonc"),
            "{\n // a comment\n \"plugin\": [\"file://./plugins/p.ts\", \"some-npm-plugin\"],\n \"mcp\": {\"y\": {\"command\": [\"bun\", \"run\", \"mcp/y.ts\"], \"url\": \"https://x.test/a\"}}\n}",
        )
        .expect("opencode.jsonc");
        std::fs::create_dir_all(at(".codex")).expect(".codex");
        std::fs::write(
            at(".codex/config.toml"),
            "[mcp_servers.z]\ncommand = \"/opt/z/run\"\nargs = [\"-c\", \"scripts/z.py\"]\n",
        )
        .expect("config.toml");
        let found: Vec<PathBuf> = scripts_named(dir.path(), dir.path(), None, &[])
            .expect("read")
            .into_iter()
            .map(|it| it.path)
            .collect();
        for want in [
            at(".claude/hooks/stop.sh"),
            at("bin/status.sh"),
            at("tools/server.js"),
            at("plugins/p.ts"),
            at("mcp/y.ts"),
            PathBuf::from("/opt/z/run"),
            at("scripts/z.py"),
        ] {
            assert!(found.contains(&want), "{want:?} not in {found:?}");
        }
        assert!(
            !found.iter().any(|it| it.ends_with("some-npm-plugin")),
            "{found:?}"
        );
    }

    /// What `scripts_named` finds for a project whose `.claude/settings.json` hook runs
    /// `command`.
    fn named_by_hook(command: &str) -> (PathBuf, Vec<PathBuf>) {
        let dir = PathBuf::from("/plane/ws/repo");
        let settings = serde_json::json!({"hooks": {"PostToolUse": [{"hooks": [
            {"type": "command", "command": command}
        ]}]}});
        let found = scripts_in(
            Path::new("/plane"),
            &dir,
            ".claude/settings.json",
            &settings.to_string(),
            None,
            &[],
        )
        .expect("read")
        .into_iter()
        .map(|named| named.path)
        .collect();
        (dir, found)
    }

    /// What `scripts_in` names for the `.mcp.json` server `server`, as JSON.
    fn named_by_server(server: &str) -> Vec<PathBuf> {
        let text = format!(r#"{{"mcpServers": {{"x": {server}}}}}"#);
        scripts_in(
            Path::new("/plane"),
            Path::new("/plane/ws/repo"),
            ".mcp.json",
            &text,
            None,
            &[],
        )
        .expect("read")
        .into_iter()
        .map(|named| named.path)
        .collect()
    }

    /// `found` with each path once, in order.
    fn once(found: Vec<PathBuf>) -> Vec<PathBuf> {
        let mut seen = std::collections::BTreeSet::new();
        found
            .into_iter()
            .filter(|it| seen.insert(it.clone()))
            .collect()
    }

    #[test]
    fn jq_s_alternative_operator_is_not_a_script_path() {
        // #1327: seen on a real project, whose every chat it made read-only.
        let (_dir, found) = named_by_hook(
            "jq -r '.tool_response.filePath // .tool_input.file_path' | xargs -r prettier --write",
        );
        assert_eq!(found, Vec::<PathBuf>::new());
        let (_dir, found) = named_by_hook("jq -r .a // .b");
        assert_eq!(found, Vec::<PathBuf>::new());
        let (_dir, found) = named_by_hook("bash -c \"jq -r '.a // .b' | xargs ls\"");
        assert_eq!(found, Vec::<PathBuf>::new());
    }

    #[test]
    fn jq_program_text_operators_and_output_redirections_are_not_script_paths() {
        for command in [
            "awk -F/ '{print $NF}' x",
            "jq '.a / 2'",
            "jq -L ./jqlib --arg n v '.a // .b'",
            "awk '{ print a \" // \" b }'",
            "grep -q x 2>/dev/null || true",
            "cat > /dev/null",
            "echo hi >/tmp/out",
            "echo hi >> ./log/out.txt 2>&1",
            "test -f x && exit 0",
        ] {
            let (_dir, found) = named_by_hook(command);
            let found: Vec<_> = found
                .into_iter()
                .filter(|it| !it.ends_with("jqlib"))
                .collect();
            assert_eq!(found, Vec::<PathBuf>::new(), "{command}");
        }
    }

    #[test]
    fn the_folder_a_command_works_in_is_not_a_script_path() {
        for command in [
            "cd \"$CLAUDE_PROJECT_DIR\" && npm run lint",
            "pushd \"$CLAUDE_PROJECT_DIR\"",
            "git -C \"$CLAUDE_PROJECT_DIR\" diff",
            // A folder that holds the config is never one a harness runs: were it replaced, the
            // config naming it would go with it.
            "prettier --write ./",
            "eslint \"$CLAUDE_PROJECT_DIR\"",
            "ls ../..",
        ] {
            let (_dir, found) = named_by_hook(command);
            assert_eq!(found, Vec::<PathBuf>::new(), "{command}");
        }
        // A script after a `cd` is named where it is found from either folder.
        let (dir, found) = named_by_hook("cd ./tools && ./fmt.sh");
        assert_eq!(found, [dir.join("fmt.sh"), dir.join("tools/fmt.sh")]);
    }

    #[test]
    fn a_real_script_path_in_a_hook_is_still_named() {
        // ADR 0067 §5, the later-code class: a script a hook runs is never the chat's to write.
        for (command, want) in [
            ("./scripts/x.sh", "scripts/x.sh"),
            (
                "\"$CLAUDE_PROJECT_DIR\"/.claude/hooks/x.sh --flag",
                ".claude/hooks/x.sh",
            ),
            ("bash 'tools/my hook.sh'", "tools/my hook.sh"),
            ("jq -r '.a' | ./bin/fmt.sh", "bin/fmt.sh"),
            ("sed -f ./scripts/fix.sed x", "scripts/fix.sed"),
            ("node tools/server.js 2>/dev/null", "tools/server.js"),
            ("jq -rf ./filters/x.jq", "filters/x.jq"),
            ("jq --from-file ./filters/y.jq", "filters/y.jq"),
            ("awk -f ./tools/sum.awk data", "tools/sum.awk"),
            ("'./scripts/run [1].sh'", "scripts/run [1].sh"),
            ("./scripts/a&b.sh", "scripts/a"),
            // What a shell reads its commands from is a script, however it is handed over.
            ("bash < ./scripts/x.sh", "scripts/x.sh"),
            (
                "sh < \"$CLAUDE_PROJECT_DIR\"/.claude/hooks/y.sh",
                ".claude/hooks/y.sh",
            ),
            ("python3 - < tools/hook.py", "tools/hook.py"),
            ("bash <<< ./scripts/here.sh", "scripts/here.sh"),
            ("bash <(./scripts/gen.sh)", "scripts/gen.sh"),
            ("diff <(./scripts/a.sh) - > /dev/null", "scripts/a.sh"),
            // An option's value that names a file the program loads.
            ("gawk -E ./tools/x.awk", "tools/x.awk"),
            ("gawk -i ./tools/lib.awk '{ print }'", "tools/lib.awk"),
            ("gawk -l ./tools/ext '{ print }'", "tools/ext"),
            // awk and sed run commands from their program text (D-1327-7).
            ("awk '{ system(\"./scripts/a.sh\") }'", "scripts/a.sh"),
            (
                "awk 'BEGIN { \"./scripts/b.sh --x\" | getline v }'",
                "scripts/b.sh",
            ),
            ("awk '{ print | \"./scripts/c.sh\" }' f", "scripts/c.sh"),
            ("sed -n 'e ./scripts/d.sh' f", "scripts/d.sh"),
        ] {
            let (dir, found) = named_by_hook(command);
            assert!(found.contains(&dir.join(want)), "{command}: {found:?}");
        }
        let (_dir, found) = named_by_hook("/opt/hooks/check && /usr/local/bin/lint.sh");
        assert_eq!(
            found,
            [
                PathBuf::from("/opt/hooks/check"),
                PathBuf::from("/usr/local/bin/lint.sh")
            ]
        );
    }

    #[test]
    fn what_a_shell_is_handed_with_c_is_read_as_commands() {
        let dir = Path::new("/plane/ws/repo");
        assert_eq!(
            once(named_by_server(
                r#"{"command": "bash", "args": ["-c", "node ./tools/server.js"]}"#
            )),
            [dir.join("tools/server.js")]
        );
        assert_eq!(
            once(named_by_server(
                r#"{"command": "sh", "args": ["-lc", "./a.sh && ./b.sh"]}"#
            )),
            [dir.join("a.sh"), dir.join("b.sh")]
        );
        let opencode = scripts_in(
            Path::new("/plane"),
            dir,
            "opencode.json",
            r#"{"mcp": {"y": {"command": ["bash", "-c", "./scripts/oc.sh --x"]}}}"#,
            None,
            &[],
        )
        .expect("read");
        assert_eq!(
            once(opencode.into_iter().map(|it| it.path).collect()),
            [dir.join("scripts/oc.sh")]
        );
        let (_, found) = named_by_hook("bash -c \"./scripts/a.sh && ./scripts/b.sh\"");
        assert_eq!(
            once(found),
            [dir.join("scripts/a.sh"), dir.join("scripts/b.sh")]
        );
        let (_, found) = named_by_hook("zsh -ic './scripts/z.sh'");
        assert_eq!(once(found), [dir.join("scripts/z.sh")]);
    }

    #[test]
    fn each_script_is_named_with_the_file_and_the_word_that_name_it() {
        let dir = Path::new("/plane/ws/repo");
        let named = scripts_in(
            Path::new("/plane"),
            dir,
            ".mcp.json",
            r#"{"mcpServers": {"x": {"command": "node", "args": ["--inspect", "tools/my server.js"]}}}"#,
            None, &[]
        )
        .expect("read");
        let want = Resolved {
            path: dir.join("tools/my server.js"),
            file: dir.join(".mcp.json"),
            word: "tools/my server.js".to_owned(),
        };
        assert!(named.contains(&want), "{named:?}");
    }

    #[test]
    fn a_long_chain_of_folder_moves_is_read_at_once_and_names_a_bounded_set() {
        let dir = Path::new("/plane/ws/repo");
        let moves: Vec<String> = (0..30).map(|at| format!("cd ./d{at}")).collect();
        let command = format!("{} && ./x.sh", moves.join(" && "));
        let started = std::time::Instant::now();
        let (_, found) = named_by_hook(&command);
        assert!(
            started.elapsed() < std::time::Duration::from_millis(100),
            "{:?}",
            started.elapsed()
        );
        let deepest: PathBuf = (0..30).fold(dir.to_path_buf(), |at, n| at.join(format!("d{n}")));
        // Found from every folder the command went through: one path each, no more.
        assert_eq!(found.len(), 31, "{found:?}");
        assert!(found.contains(&deepest.join("x.sh")), "{found:?}");
    }

    #[test]
    fn a_script_after_the_shell_returns_to_an_earlier_folder_is_named_where_it_is() {
        let dir = Path::new("/plane/ws/repo");
        for (command, want) in [
            ("cd sub && (cd tools && make) && ./run.sh", "sub/run.sh"),
            ("(cd a && cd b); cd c; ./x.sh", "c/x.sh"),
            ("cd a && cd b && cd - && ./x.sh", "a/x.sh"),
            ("pushd a && pushd b && popd && ./x.sh", "a/x.sh"),
            ("cd a && cd ../b && ./x.sh", "b/x.sh"),
        ] {
            let (_, found) = named_by_hook(command);
            assert!(found.contains(&dir.join(want)), "{command}: {found:?}");
        }
    }

    #[test]
    fn folder_moves_are_counted_per_command() {
        let dir = Path::new("/plane/ws/repo");
        let hooks: Vec<serde_json::Value> = (0..100)
            .map(|at| {
                serde_json::json!({"type": "command",
                    "command": format!("cd \"$CLAUDE_PROJECT_DIR\" && ./scripts/h{at}.sh")})
            })
            .collect();
        let settings = serde_json::json!({"hooks": {"Stop": [{"hooks": hooks}]}});
        let found = scripts_in(
            Path::new("/plane"),
            dir,
            ".claude/settings.json",
            &settings.to_string(),
            None,
            &[],
        )
        .expect("every hook read");
        assert_eq!(found.len(), 100);
        assert!(found.iter().any(|it| it.path == dir.join("scripts/h99.sh")));
    }

    #[test]
    fn past_the_most_folder_moves_a_config_is_unread_rather_than_half_read() {
        let dir = Path::new("/plane/ws/repo");
        let moves: Vec<String> = (0..=MOST_MOVES).map(|at| format!("cd ./d{at}")).collect();
        let settings = serde_json::json!({"hooks": {"Stop": [{"hooks": [
            {"type": "command", "command": format!("{} && ./x.sh", moves.join(" && "))}
        ]}]}});
        assert_eq!(
            scripts_in(
                Path::new("/plane"),
                dir,
                ".claude/settings.json",
                &settings.to_string(),
                None,
                &[]
            ),
            Err(Unread {
                file: dir.join(".claude/settings.json"),
                word: format!("./d{MOST_MOVES}"),
            })
        );
    }

    #[test]
    fn a_runner_pointed_at_the_config_s_folder_has_that_folder_s_entry_files_named() {
        // D-1327-9: the folder itself is never denied (D-1327-8), what a runner loads from it is.
        for command in [
            "node ./",
            "node .",
            "deno run .",
            "bun \"$CLAUDE_PROJECT_DIR\"",
            "tsx .",
            "ts-node ./",
            "npx .",
            "go run .",
            "python .",
            "python3 \"$CLAUDE_PROJECT_DIR\"",
            "uv run .",
            "ruby .",
            "php ./",
        ] {
            let (dir, found) = named_by_hook(command);
            let want: Vec<PathBuf> = ENTRY_FILES.iter().map(|name| dir.join(name)).collect();
            assert_eq!(
                found.iter().map(|it| lexical(it)).collect::<Vec<_>>(),
                want,
                "{command}"
            );
        }
        let (dir, found) = named_by_hook("node ../..");
        assert!(found.contains(&dir.join("../../package.json")), "{found:?}");
        // Anything else pointed at the folder names nothing.
        let (_, found) = named_by_hook("prettier --write .");
        assert_eq!(found, Vec::<PathBuf>::new());
    }

    #[test]
    fn an_option_s_value_written_against_it_is_read_as_its_value() {
        let (dir, found) = named_by_hook("jq -L./jqlib '.a // .b'");
        assert_eq!(found, [dir.join("jqlib")]);
        let (dir, found) = named_by_hook("awk -f./tools/sum.awk data");
        assert_eq!(found, [dir.join("tools/sum.awk")]);
    }

    /// What a hook running `command` in `/plane/ws/repo` names, for an operator whose home is
    /// `/home/op`.
    fn named_at_home(command: &str) -> Vec<PathBuf> {
        named_writing(command, &[])
    }

    /// [`named_at_home`] for a chat that could also write `writable`.
    fn named_writing(command: &str, writable: &[PathBuf]) -> Vec<PathBuf> {
        let settings = serde_json::json!({"hooks": {"PostToolUse": [{"hooks": [
            {"type": "command", "command": command}
        ]}]}});
        scripts_in(
            Path::new("/plane"),
            Path::new("/plane/ws/repo"),
            ".claude/settings.json",
            &settings.to_string(),
            Some(Path::new("/home/op")),
            writable,
        )
        .expect("read")
        .into_iter()
        .map(|named| named.path)
        .collect()
    }

    #[test]
    fn an_absolute_split_of_a_cluster_is_named_in_a_folder_chats_may_be_granted() {
        // D-T56-1: a folder you list as one chats may be granted, outside the project and the
        // home folder, is one a chat could write, so a later letter's value there is named.
        let granted = [PathBuf::from("/opt/granted")];
        let command = "cc -xI/opt/granted/inc/x.h a.c";
        assert!(!named_at_home(command).contains(&PathBuf::from("/opt/granted/inc/x.h")));
        let found = named_writing(command, &granted);
        assert!(
            found.contains(&PathBuf::from("/opt/granted/inc/x.h")),
            "{found:?}"
        );
        // Only below it: a folder beside it is still no value.
        let found = named_writing("cc -xI/opt/grantedx/y a.c", &granted);
        assert!(
            !found.contains(&PathBuf::from("/opt/grantedx/y")),
            "{found:?}"
        );
    }

    #[test]
    fn an_absolute_split_of_a_cluster_is_named_in_the_project_s_cache_home() {
        // D-T56-1: the project's cache home (#1337) can sit outside the home folder, where
        // `XDG_DATA_HOME` moves purlis's data home; a chat writes it, so a value there is named.
        let cache = PathBuf::from("/srv/data/purlis/cache-homes/0123abcd");
        // A compiler's cluster, whose letters purlis does not know: gawk's `-bf` is read by
        // gawk's own options (D-1418-1), which name its `-f` file wherever it is.
        let command = "cc -xI/srv/data/purlis/cache-homes/0123abcd/npm/x.h a.c";
        let want = cache.join("npm/x.h");
        assert!(!named_at_home(command).contains(&want));
        let found = named_writing(command, std::slice::from_ref(&cache));
        assert!(found.contains(&want), "{found:?}");
    }

    #[test]
    fn an_absolute_split_of_a_cluster_is_named_where_a_chat_could_write_it() {
        // #1356, D-1356-7: an absolute value after any letter, in the project or the home
        // folder, is named; one after a later letter elsewhere names no `/tmp` for `-Ivendor/tmp`.
        for (command, want) in [
            ("cc -I/opt/include/x.h -c a.c", "/opt/include/x.h"),
            ("gawk -bf/plane/tools/x.awk data", "/plane/tools/x.awk"),
            ("sed -nf/plane/ws/repo/fix.sed x", "/plane/ws/repo/fix.sed"),
            ("jq -rf/plane/filters/x.jq ./in.json", "/plane/filters/x.jq"),
            ("cc -xI/plane/inc/x.h a.c", "/plane/inc/x.h"),
            ("cc -xI/home/op/inc/x.h a.c", "/home/op/inc/x.h"),
        ] {
            let found = named_at_home(command);
            assert!(found.contains(&PathBuf::from(want)), "{command}: {found:?}");
        }
        for (command, not) in [
            ("cc -Ivendor/tmp -c a.c", "/tmp"),
            ("cc -Isrc/usr/local x.c", "/usr/local"),
            ("cc -xI/opt/elsewhere x.c", "/opt/elsewhere"),
        ] {
            let found = named_at_home(command);
            assert!(!found.contains(&PathBuf::from(not)), "{command}: {found:?}");
        }
    }

    #[test]
    fn a_tilde_inside_a_word_is_named_as_written_and_as_the_home_folder() {
        // #1356, D-1356-8: a shell expands `~` only at a word's start (and after `=` in bash),
        // so a value taken out of a word may be either.
        for (command, tail) in [
            ("cc -I~/lib/x.h a.c", "lib/x.h"),
            ("cc --include=~/lib/x.h a.c", "lib/x.h"),
            ("java -javaagent:~/tools/a.jar -jar x", "tools/a.jar"),
            ("awk -bf~/tools/x.awk data", "tools/x.awk"),
        ] {
            let found = named_at_home(command);
            for want in [
                PathBuf::from("/plane/ws/repo/~").join(tail),
                PathBuf::from("/home/op").join(tail),
            ] {
                assert!(
                    found.contains(&want),
                    "{command}: {want:?} not in {found:?}"
                );
            }
        }
    }

    #[test]
    fn a_java_agent_s_file_written_after_a_colon_is_named() {
        // #1356 review: `-javaagent:<jar>[=options]`, `-agentpath:<lib>[=options]`, `-agentlib:`.
        for (command, want) in [
            (
                "java -javaagent:./tools/agent.jar -jar app.jar",
                "tools/agent.jar",
            ),
            (
                "java -javaagent:./tools/agent.jar=./conf/a.yml -jar x",
                "tools/agent.jar",
            ),
            (
                "java -javaagent:./tools/agent.jar=./conf/a.yml -jar x",
                "conf/a.yml",
            ),
            (
                "java -agentpath:./lib/prof.so=depth=3 -jar x",
                "lib/prof.so",
            ),
            ("java -agentlib:./lib/hprof=cpu=times -jar x", "lib/hprof"),
        ] {
            let (dir, found) = named_by_hook(command);
            assert!(found.contains(&dir.join(want)), "{command}: {found:?}");
        }
    }

    #[test]
    fn an_option_s_value_attached_to_it_names_what_it_names_given_apart() {
        // #1356: written against its option, alone or after other short options, or after `=`,
        // a value is the operand it is when it is a word of its own.
        for (apart, attached) in [
            (
                "gawk -b -f ./tools/sum.awk data",
                "gawk -bf./tools/sum.awk data",
            ),
            (
                "sed -n -f ./scripts/fix.sed x",
                "sed -nf./scripts/fix.sed x",
            ),
            (
                "jq -f ./filters/x.jq ./data/in.json",
                "jq -f./filters/x.jq ./data/in.json",
            ),
            (
                "jq -r -f ./filters/x.jq ./data/in.json",
                "jq -rf./filters/x.jq ./data/in.json",
            ),
            (
                "jq --from-file ./filters/x.jq ./data/in.json",
                "jq --from-file=./filters/x.jq ./data/in.json",
            ),
            (
                "node --require ./tools/hook.js",
                "node --require=./tools/hook.js",
            ),
        ] {
            let (_, want) = named_by_hook(apart);
            let (_, found) = named_by_hook(attached);
            assert!(!want.is_empty(), "{apart}");
            for path in &want {
                assert!(
                    found.contains(path),
                    "{attached}: {path:?} not in {found:?}"
                );
            }
        }
    }

    #[test]
    fn a_script_named_without_a_slash_is_named_where_its_program_reads_it() {
        // #1418, D-1418-1: a word a program opens as its script or program file is a path
        // from the folder the command runs in, `/` or not.
        for (command, want) in [
            ("bash hook.sh", "hook.sh"),
            ("sh -e -o pipefail hook.sh arg", "hook.sh"),
            ("bash --rcfile rc.sh -i", "rc.sh"),
            ("source env.sh", "env.sh"),
            (". env.sh", "env.sh"),
            ("awk -f sum.awk data", "sum.awk"),
            ("awk -fsum.awk data", "sum.awk"),
            ("gawk --file=sum.awk data", "sum.awk"),
            ("gawk -v x=1 -f sum.awk data", "sum.awk"),
            ("sed -n -f fix.sed x", "fix.sed"),
            ("jq -rf filter.jq in.json", "filter.jq"),
            ("python3 hook.py", "hook.py"),
            ("python3 -u -W ignore hook.py", "hook.py"),
            ("python3 -mtools.x", "tools/x.py"),
            ("python3 -m tools.x --flag", "tools/x/__main__.py"),
            ("node server.js", "server.js"),
            ("deno run main.ts", "main.ts"),
            ("ruby hook.rb", "hook.rb"),
            ("perl -w hook.pl", "hook.pl"),
            ("php -f hook.php", "hook.php"),
            // Its letters read as gawk reads them: `-f`'s file, wherever it is.
            ("gawk -bf/opt/tools/x.awk data", "/opt/tools/x.awk"),
        ] {
            let (dir, found) = named_by_hook(command);
            assert!(found.contains(&dir.join(want)), "{command}: {found:?}");
        }
        let found = named_by_server(r#"{"command": "node", "args": ["server.js"]}"#);
        assert!(
            found.contains(&PathBuf::from("/plane/ws/repo/server.js")),
            "{found:?}"
        );
        // Found from every folder the command went to, as a word with a `/` is.
        let (dir, found) = named_by_hook("cd tools && bash fmt.sh");
        assert!(found.contains(&dir.join("tools/fmt.sh")), "{found:?}");
    }

    #[test]
    fn a_word_no_program_opens_as_its_script_is_still_no_path() {
        // D-1418-1: only the script operand and a program-file option's value; the script's
        // own arguments, code handed with `-c` or `-e`, and a module's arguments name nothing.
        for command in [
            "bash -c 'echo hi' arg0",
            "python3 -c 'print(1)' data.txt",
            "node -e 'x()' data.txt",
            "python3 -m pytest tests",
            "awk '{ print }' data.txt",
            "sed -n p data.txt",
            "npx prettier --write src",
            "jq -r .a in.json",
        ] {
            let (dir, found) = named_by_hook(command);
            for not in ["data.txt", "arg0", "tests", "src", "in.json", "prettier"] {
                assert!(!found.contains(&dir.join(not)), "{command}: {found:?}");
            }
        }
    }

    #[test]
    fn a_file_named_without_its_extension_is_named_with_each_one_its_runner_tries() {
        // #1418, D-1418-2.
        for (command, want) in [
            ("node -r ./hook x.js", "hook.js"),
            ("node -r ./hook x.js", "hook.cjs"),
            ("node --require=./hook x.js", "hook.json"),
            ("node --import ./tools/reg x.js", "tools/reg.mjs"),
            ("tsx ./tools/run", "tools/run.ts"),
            ("node ./tools/run", "tools/run.js"),
            ("ruby -r./x hook.rb", "x.rb"),
            ("ruby -r ./x hook.rb", "x.so"),
            ("gawk -i ./lib/util '{ print }'", "lib/util.awk"),
            ("gawk -f ./lib/sum data", "lib/sum.awk"),
        ] {
            let (dir, found) = named_by_hook(command);
            assert!(found.contains(&dir.join(want)), "{command}: {found:?}");
        }
        // A name that already carries one of them is the file itself.
        let (dir, found) = named_by_hook("node ./tools/server.js");
        assert_eq!(once(found), [dir.join("tools/server.js")]);
    }

    #[test]
    fn an_assignment_s_value_is_named_as_well_as_the_whole_word() {
        // #1418, D-1418-3: `BASH_ENV=./x.sh`, git's `-c <key>=<value>`.
        for (command, want) in [
            ("BASH_ENV=./x.sh bash -c 'true'", "x.sh"),
            // The folder of git's hooks, which is code, whatever it is (D-1418-10).
            ("git -c core.hooksPath=./h commit", "h"),
            ("git -c core.fsmonitor=./tools/fsm commit", "tools/fsm"),
            ("make CC=./tools/cc.sh", "tools/cc.sh"),
        ] {
            let (dir, found) = named_by_hook(command);
            assert!(found.contains(&dir.join(want)), "{command}: {found:?}");
        }
        // As written too: a file may hold an `=` in its name.
        let (dir, found) = named_by_hook("./run x=tools/y.sh");
        assert!(found.contains(&dir.join("x=tools/y.sh")), "{found:?}");
        // An absolute value is a guess, held to where a chat could write (D-1356-7).
        let found = named_at_home("PREFIX=/usr/local make && HOOKS=/opt/h.sh x");
        for not in ["/usr/local", "/opt/h.sh"] {
            assert!(!found.contains(&PathBuf::from(not)), "{found:?}");
        }
        let found = named_at_home("git -c core.hooksPath=/plane/h x && X=/home/op/y.sh z");
        for want in ["/plane/h", "/home/op/y.sh"] {
            assert!(found.contains(&PathBuf::from(want)), "{found:?}");
        }
    }

    #[test]
    fn a_folder_named_only_after_an_equals_sign_is_never_denied() {
        // #1418, D-1418-10: a search path or an output folder, not code.
        for (command, not) in [
            ("CARGO_TARGET_DIR=./target cargo clippy", "target"),
            (
                "CARGO_TARGET_DIR=$CLAUDE_PROJECT_DIR/target cargo build",
                "target",
            ),
            ("OUT=./dist npm run build", "dist"),
            ("PYTHONPATH=./src python3 -c 'import x'", "src"),
            ("env NODE_PATH=./lib node", "lib"),
            ("cargo test --target-dir=./target", "target"),
            ("make CC=./tools/cc", "tools/cc"),
        ] {
            let (dir, found) = named_by_hook(command);
            assert!(!found.contains(&dir.join(not)), "{command}: {found:?}");
        }
        // A file that is there is named, extension or not.
        let base = tempfile::tempdir().expect("a base");
        let project = base.path().canonicalize().expect("the base");
        let repo = project.join("ws/repo");
        std::fs::create_dir_all(repo.join("target")).expect("a folder");
        std::fs::write(repo.join("hook"), "#!/bin/sh\n").expect("a file");
        let found = named_in(&project, &project, "X=./hook Y=./target z --t=./target");
        assert!(found.contains(&repo.join("hook")), "{found:?}");
        assert!(!found.contains(&repo.join("target")), "{found:?}");
    }

    #[test]
    fn a_colon_list_is_named_part_by_part() {
        // #1425: each part of `PYTHONPATH=./a:./b` is a value of its own, held as one is.
        for (command, want) in [
            ("PYTHONPATH=./a.zip:./b.zip python3 x.py", "a.zip"),
            ("PYTHONPATH=./a.zip:./b.zip python3 x.py", "b.zip"),
            ("X=./tools/h.sh:./tools/g.sh y", "tools/g.sh"),
            ("env NODE_PATH=lib:./tools/reg.js node s.js", "tools/reg.js"),
        ] {
            let (dir, found) = named_by_hook(command);
            assert!(found.contains(&dir.join(want)), "{command}: {found:?}");
        }
        // A folder in one is a search path, never denied, as a single one is (D-1418-10).
        let (dir, found) = named_by_hook("PYTHONPATH=./src:./lib python3 -c 'import x'");
        for not in ["src", "lib"] {
            assert!(!found.contains(&dir.join(not)), "{found:?}");
        }
        // An absolute part is held to where a chat could write, as a whole value is.
        let found = named_at_home("X=/opt/a.sh:/home/op/b.sh y");
        assert!(!found.contains(&PathBuf::from("/opt/a.sh")), "{found:?}");
        assert!(found.contains(&PathBuf::from("/home/op/b.sh")), "{found:?}");
    }

    #[test]
    fn a_bare_name_is_also_named_along_the_search_path_its_runner_follows() {
        // #1425: `NODE_PATH`, `AWKPATH` and Ruby's `-I` with a `/`.
        for (command, want) in [
            ("NODE_PATH=./lib node -r hook s.js", "lib/hook.js"),
            ("NODE_PATH=./lib node --require=hook s.js", "lib/hook.cjs"),
            ("env NODE_PATH=./a:./b node --import reg s.js", "b/reg.mjs"),
            (
                "NODE_PATH=lib tsx -r pkg/register s.ts",
                "lib/pkg/register.ts",
            ),
            ("AWKPATH=./awk gawk -f sum data", "awk/sum.awk"),
            (
                "AWKPATH=./awk:./more gawk -i util -f x.awk d",
                "more/util.awk",
            ),
            ("ruby -I./rlib -r x y.rb", "rlib/x.rb"),
            ("ruby -I ./rlib -rx y.rb", "rlib/x.so"),
        ] {
            let (dir, found) = named_by_hook(command);
            assert!(covered(&found, &dir.join(want)), "{command}: {found:?}");
        }
        // A name written from a folder is not looked up, and a bare `-I lib` stays unread
        // (D-1418-1); a script operand is never looked up along it.
        for (command, not) in [
            ("NODE_PATH=./lib node -r ./hook s.js", "lib/hook.js"),
            ("ruby -Ilib -r x y.rb", "lib/x.rb"),
            ("NODE_PATH=./lib node s.js", "lib/s.js"),
        ] {
            let (dir, found) = named_by_hook(command);
            assert!(!covered(&found, &dir.join(not)), "{command}: {found:?}");
        }
    }

    #[test]
    fn a_bare_name_node_loads_first_is_named_in_each_node_modules_up_from_where_it_runs() {
        // #1425: Node looks a bare `-r`/`--import` name up in `node_modules`, in the folder the
        // command runs in and each one above it, as far as a chat could write.
        for (command, want) in [
            ("node -r hook s.js", "/plane/ws/repo/node_modules/hook.js"),
            ("node --import reg s.js", "/plane/ws/node_modules/reg.mjs"),
            (
                "tsx -r pkg/register s.ts",
                "/plane/node_modules/pkg/register.ts",
            ),
            ("bun --preload hook s.ts", "/plane/node_modules/hook.ts"),
            (
                "cd tools && node -r hook s.js",
                "/plane/ws/repo/tools/node_modules/hook.cjs",
            ),
        ] {
            let (_, found) = named_by_hook(command);
            assert!(covered(&found, Path::new(want)), "{command}: {found:?}");
        }
        // Not above the project, where no chat writes, nor for a name written from a folder or
        // for the script operand.
        for (command, not) in [
            ("node -r hook s.js", "/node_modules/hook.js"),
            ("node -r ./hook s.js", "/plane/ws/repo/node_modules/hook.js"),
            ("node s.js", "/plane/ws/repo/node_modules/s.js"),
        ] {
            let (_, found) = named_by_hook(command);
            assert!(!covered(&found, Path::new(not)), "{command}: {found:?}");
        }
    }

    #[test]
    fn ruby_s_load_path_and_perl_s_inc_are_followed_for_a_bare_name() {
        // #1425: `RUBYLIB` beside Ruby's `-I`; Perl's `-M`/`-m` module, from where the command
        // runs and along `PERL5LIB` and `-I` with a `/`.
        for (command, want) in [
            ("RUBYLIB=./rlib ruby -r x y.rb", "rlib/x.rb"),
            ("env RUBYLIB=lib:./more ruby -rx y.rb", "more/x.so"),
            ("perl -MMy::Hook s.pl", "My/Hook.pm"),
            ("perl -mMy::Hook=go s.pl", "My/Hook.pm"),
            ("perl -M-Strictish s.pl", "Strictish.pm"),
            ("PERL5LIB=./plib perl -MMy::Hook s.pl", "plib/My/Hook.pm"),
            ("PERL5LIB=a:./b perl -MHook s.pl", "b/Hook.pm"),
            ("perl -I./plib -MHook s.pl", "plib/Hook.pm"),
            ("perl -I ./plib -MHook s.pl", "plib/Hook.pm"),
        ] {
            let (dir, found) = named_by_hook(command);
            assert!(covered(&found, &dir.join(want)), "{command}: {found:?}");
        }
        // A bare `-Ilib` stays unread (D-1418-1), and Perl's script is still its first operand.
        for (command, not) in [
            ("perl -Ilib -MHook s.pl", "lib/Hook.pm"),
            ("RUBYLIB=./rlib ruby y.rb", "rlib/y.rb"),
        ] {
            let (dir, found) = named_by_hook(command);
            assert!(!covered(&found, &dir.join(not)), "{command}: {found:?}");
        }
        let (dir, found) = named_by_hook("perl -MHook s.pl");
        assert!(found.contains(&dir.join("s.pl")), "{found:?}");
    }

    #[test]
    fn a_package_in_node_modules_names_the_file_its_manifest_says_node_loads() {
        let project = tempfile::tempdir().expect("temp");
        let package = project.path().join("node_modules/hook");
        std::fs::create_dir_all(&package).expect("folder");
        std::fs::write(
            package.join("package.json"),
            r#"{"name": "hook", "main": "lib/start"}"#,
        )
        .expect("manifest");
        let settings = serde_json::json!({"hooks": {"PostToolUse": [{"hooks": [
            {"type": "command", "command": "node -r hook s.js"}
        ]}]}});
        let found: Vec<PathBuf> = scripts_in(
            project.path(),
            project.path(),
            ".claude/settings.json",
            &settings.to_string(),
            None,
            &[],
        )
        .expect("read")
        .into_iter()
        .map(|named| named.path)
        .collect();
        for want in ["package.json", "index.js", "lib/start", "lib/start.js"] {
            assert!(covered(&found, &package.join(want)), "{want}: {found:?}");
        }
        // A main that leaves the package for where no chat writes is not followed.
        std::fs::write(
            package.join("package.json"),
            r#"{"main": "../../../../../../../../../../opt/x.js"}"#,
        )
        .expect("manifest");
        let found: Vec<PathBuf> = scripts_in(
            project.path(),
            project.path(),
            ".claude/settings.json",
            &settings.to_string(),
            None,
            &[],
        )
        .expect("read")
        .into_iter()
        .map(|named| named.path)
        .collect();
        assert!(
            !found.iter().any(|path| path.ends_with("opt/x.js")),
            "{found:?}"
        );
    }

    /// #1425: past [`MOST_NAMED`] a chat is refused, so following `node_modules` must leave an
    /// ordinary project below it: a monorepo whose hooks go into its packages and load
    /// their usual hooks first, with those packages installed. The walk stops at the project
    /// and takes the home folder's own `node_modules`, not every folder between.
    #[test]
    fn a_monorepo_s_node_hooks_stay_well_under_the_most_named() {
        let home = tempfile::tempdir().expect("temp");
        let project = home.path().join("code/org/workspaces/ws");
        let repo = project.join("repo");
        let package = |at: &Path, name: &str| {
            let folder = at.join("node_modules").join(name);
            std::fs::create_dir_all(&folder).expect("folder");
            std::fs::write(folder.join("package.json"), r#"{"main": "dist/index.js"}"#)
                .expect("manifest");
        };
        for name in [
            "dotenv",
            "ts-node",
            "tsx",
            "tsconfig-paths",
            "source-map-support",
        ] {
            package(&repo, name);
        }
        let packages = [
            "packages/api",
            "packages/web",
            "apps/cli",
            "packages/shared",
        ];
        for one in packages {
            package(&repo.join(one), "dotenv");
            package(&repo.join(one), "tsx");
        }
        let mut commands: Vec<serde_json::Value> = packages
            .iter()
            .map(|one| {
                serde_json::json!({"type": "command", "command": format!(
                    "cd {one} && node -r dotenv/config -r ts-node/register --import tsx \
                     src/main.ts"
                )})
            })
            .collect();
        commands.push(serde_json::json!({"type": "command", "command":
            "node -r tsconfig-paths/register -r source-map-support/register scripts/check.js"}));
        let settings = serde_json::json!({"hooks": {"PostToolUse": [{"hooks": commands}]}});

        let found: Vec<PathBuf> = scripts_in(
            &project,
            &repo,
            ".claude/settings.json",
            &settings.to_string(),
            Some(home.path()),
            &[],
        )
        .expect("read")
        .into_iter()
        .map(|named| named.path)
        .collect();

        // 196 when this was written (561 before a package was named once as its folder, #1425),
        // of which 193 named before `node_modules` was followed.
        assert!(found.len() < MOST_NAMED / 4, "{}", found.len());
        let in_home = home.path().join("node_modules/tsx.mjs");
        let between = home.path().join("code/node_modules/tsx.mjs");
        assert!(found.contains(&in_home), "{found:?}");
        assert!(!found.contains(&between), "{found:?}");
        assert!(
            found.contains(&repo.join("packages/node_modules/tsx.mjs")),
            "{found:?}"
        );
    }

    /// Whether `path` is denied by one of `found`: itself, or a folder it is in, as a rule on a
    /// folder holds everything below it.
    fn covered(found: &[PathBuf], path: &Path) -> bool {
        found.iter().any(|one| path.starts_with(one))
    }

    /// What a `.claude/settings.json` hook running `command` names in `project`, which is also
    /// the folder the config is in.
    fn named_in_project(project: &Path, command: &str) -> Vec<PathBuf> {
        let settings = serde_json::json!({"hooks": {"PostToolUse": [{"hooks": [
            {"type": "command", "command": command}
        ]}]}});
        scripts_in(
            project,
            project,
            ".claude/settings.json",
            &settings.to_string(),
            None,
            &[],
        )
        .expect("read")
        .into_iter()
        .map(|named| named.path)
        .collect()
    }

    /// #1425: a package Node loads from `node_modules` is named once, as its folder, which holds
    /// every file named in it before (its manifest, its entry files, what its manifest's `main`
    /// says, a file below it asked for by name), and each file beside it Node tries first.
    #[test]
    fn a_package_in_node_modules_is_named_once_as_its_folder() {
        let project = tempfile::tempdir().expect("temp");
        let package = project.path().join("node_modules/hook");
        std::fs::create_dir_all(&package).expect("folder");
        std::fs::write(
            package.join("package.json"),
            r#"{"name": "hook", "main": "lib/start"}"#,
        )
        .expect("manifest");
        let modules = project.path().join("node_modules");
        let found = named_in_project(project.path(), "node -r hook -r hook/register s.js");
        assert!(found.contains(&package), "{found:?}");
        // Each path named before this, held by the folder or named itself.
        for before in [
            "hook/package.json",
            "hook/index.js",
            "hook/main.py",
            "hook/lib/start",
            "hook/lib/start.js",
            "hook/lib/start/index.js",
            "hook/register",
            "hook/register.js",
            "hook/register.ts",
        ] {
            assert!(
                covered(&found, &modules.join(before)),
                "{before}: {found:?}"
            );
        }
        // Node tries a file beside the folder before the folder itself.
        for beside in ["hook.js", "hook.cjs", "hook.mjs", "hook.json"] {
            assert!(found.contains(&modules.join(beside)), "{beside}: {found:?}");
        }
        assert!(
            !found
                .iter()
                .any(|one| one.starts_with(&package) && *one != package),
            "nothing below the folder is named twice: {found:?}"
        );
    }

    /// #1425: a package folder that is not there yet is named once too, for every file below it
    /// a hook asks for, and a scoped package by its own folder, never its scope's.
    #[test]
    fn a_package_not_installed_yet_is_named_once_for_every_file_asked_of_it() {
        let project = tempfile::tempdir().expect("temp");
        let modules = project.path().join("node_modules");
        let found = named_in_project(
            project.path(),
            "node -r dotenv/config -r dotenv/register -r @swc/register/x s.js",
        );
        assert!(found.contains(&modules.join("dotenv")), "{found:?}");
        assert!(found.contains(&modules.join("@swc/register")), "{found:?}");
        assert!(!covered(&found, &modules.join("@swc/other")), "{found:?}");
        for before in [
            "dotenv/config",
            "dotenv/config.js",
            "dotenv/register.cjs",
            "@swc/register/x.ts",
        ] {
            assert!(
                covered(&found, &modules.join(before)),
                "{before}: {found:?}"
            );
        }
        let below = found.iter().filter(|one| one.starts_with(&modules)).count();
        assert_eq!(below, 2, "{found:?}");
    }

    /// #1425: a package and a file in it that Node is asked to load first is also named from
    /// the folder the command runs in: there, a folder it starts with that is not there is
    /// named once, as it holds every file tried below it; one that is there, the project's own,
    /// is never named whole.
    #[test]
    fn a_package_s_file_asked_of_the_folder_a_command_runs_in_names_a_missing_folder_once() {
        let project = tempfile::tempdir().expect("temp");
        let at = |rel: &str| project.path().join(rel);
        std::fs::create_dir_all(at("lib")).expect("lib");
        let found = named_in_project(project.path(), "node -r dotenv/config -r lib/x s.js");
        assert!(found.contains(&at("dotenv")), "{found:?}");
        for before in ["dotenv/config", "dotenv/config.js", "dotenv/config.tsx"] {
            assert!(covered(&found, &at(before)), "{before}: {found:?}");
        }
        assert!(!found.contains(&at("lib")), "{found:?}");
        assert!(found.contains(&at("lib/x.js")), "{found:?}");
        assert!(!covered(&found, &at("lib/y.js")), "{found:?}");
    }

    #[test]
    fn a_path_named_twice_or_below_a_folder_named_is_kept_once() {
        let one = |path: &str| Resolved {
            path: PathBuf::from(path),
            file: PathBuf::from("/p/.mcp.json"),
            word: "w".to_owned(),
        };
        let found = once_each(vec![
            one("/p/a/x.js"),
            one("/p/b"),
            one("/p/a"),
            one("/p/b/y.js"),
            one("/p/b"),
            one("/p/bb.js"),
            one("/p/b/../c.js"),
        ]);
        let paths: Vec<&Path> = found.iter().map(|it| it.path.as_path()).collect();
        assert_eq!(
            paths,
            [
                Path::new("/p/b"),
                Path::new("/p/a"),
                Path::new("/p/bb.js"),
                Path::new("/p/b/../c.js")
            ]
        );
    }

    /// #1425: a path one config names inside a folder another config names is held by that
    /// folder, so the plane's list names it once.
    #[test]
    fn a_path_a_folder_named_by_another_config_holds_is_named_once() {
        let root = tempfile::tempdir().expect("temp");
        let root = root.path().canonicalize().expect("real");
        std::fs::create_dir_all(root.join("tool")).expect("tool");
        std::fs::write(
            root.join(".mcp.json"),
            r#"{"mcpServers": {"x": {"command": "node", "args": ["-r", "dotenv/config", "s.js"]}}}"#,
        )
        .expect(".mcp.json");
        let inside = root.join("dotenv/extra.js");
        std::fs::write(
            root.join("tool/.mcp.json"),
            serde_json::json!({"mcpServers": {"y": {"command": "python3", "args": [inside]}}})
                .to_string(),
        )
        .expect("tool/.mcp.json");
        let found: Vec<PathBuf> = resolved(&root, None, None, &[])
            .expect("read")
            .into_iter()
            .map(|it| it.path)
            .collect();
        assert!(found.contains(&root.join("dotenv")), "{found:?}");
        assert!(!found.contains(&inside), "{found:?}");
        assert!(found.contains(&root.join("s.js")), "{found:?}");
    }

    /// #1425: a path written with `..` is named as written and with the `..` taken off, as
    /// before, never folded into a folder it only reads as being in.
    #[test]
    fn a_path_written_with_dot_dot_is_never_taken_for_the_folder_it_reads_as_in() {
        let project = tempfile::tempdir().expect("temp");
        let at = |rel: &str| project.path().join(rel);
        let found = named_in_project(
            project.path(),
            "node -r ./node_modules/a/../b/x.js -r up/../../y s.js",
        );
        assert!(found.contains(&at("node_modules/a/../b/x.js")), "{found:?}");
        assert!(found.contains(&at("node_modules/b/x.js")), "{found:?}");
        assert!(!found.contains(&at("node_modules/b")), "{found:?}");
        assert!(!found.contains(&at("node_modules/a")), "{found:?}");
        assert!(!found.contains(&at("up")), "{found:?}");
    }

    /// #1425: a package that is a link to the project's own code (a workspace's package, as
    /// npm, yarn and pnpm link them) is not named whole, so the chat can still work on that
    /// code: only the files Node loads from it are, as before.
    #[cfg(unix)]
    #[test]
    fn a_workspace_package_linked_into_node_modules_is_not_named_whole() {
        let project = tempfile::tempdir().expect("temp");
        let project = project.path().canonicalize().expect("real");
        let shared = project.join("packages/shared");
        std::fs::create_dir_all(&shared).expect("package");
        std::fs::write(shared.join("package.json"), r#"{"main": "dist/index.js"}"#)
            .expect("manifest");
        std::fs::create_dir_all(project.join("node_modules/@org")).expect("scope");
        let link = project.join("node_modules/@org/shared");
        std::os::unix::fs::symlink(&shared, &link).expect("link");
        let found = named_in_project(&project, "node -r @org/shared s.js");
        assert!(!found.contains(&link), "{found:?}");
        assert!(!covered(&found, &shared.join("src/a.ts")), "{found:?}");
        assert!(found.contains(&link.join("package.json")), "{found:?}");
        assert!(found.contains(&link.join("index.js")), "{found:?}");
    }

    /// #1425: a project that is itself below a `node_modules` folder is never named whole as a
    /// package, nor is a folder above the config.
    #[test]
    fn a_project_inside_a_node_modules_folder_is_not_taken_for_a_package() {
        let outer = tempfile::tempdir().expect("temp");
        let project = outer.path().join("node_modules/proj");
        std::fs::create_dir_all(&project).expect("project");
        let found = named_in_project(&project, "node scripts/check.js");
        assert!(
            found.contains(&project.join("scripts/check.js")),
            "{found:?}"
        );
        assert!(!covered(&found, &project.join("src/main.js")), "{found:?}");
    }

    /// A package's manifest is written by whoever wrote the package, a chat included: a FIFO
    /// there is not waited on and a link there is not followed.
    #[cfg(unix)]
    #[test]
    fn a_package_manifest_that_is_no_plain_file_is_not_read() {
        let project = tempfile::tempdir().expect("temp");
        let fifo = project.path().join("node_modules/fifo");
        let linked = project.path().join("node_modules/linked");
        std::fs::create_dir_all(&fifo).expect("folder");
        std::fs::create_dir_all(&linked).expect("folder");
        let made = crate::forklock::status(
            std::process::Command::new("mkfifo").arg(fifo.join("package.json")),
        )
        .is_ok_and(|status| status.success());
        assert!(made, "mkfifo makes a FIFO");
        let elsewhere = project.path().join("elsewhere.json");
        std::fs::write(&elsewhere, r#"{"main": "lib/start.js"}"#).expect("manifest");
        std::os::unix::fs::symlink(&elsewhere, linked.join("package.json")).expect("link");
        assert_eq!(package_entries(&fifo), Vec::<PathBuf>::new());
        assert_eq!(package_entries(&linked), Vec::<PathBuf>::new());
        let plain = project.path().join("node_modules/plain");
        std::fs::create_dir_all(&plain).expect("folder");
        std::fs::write(plain.join("package.json"), r#"{"main": "lib/start.js"}"#).expect("it");
        assert_eq!(package_entries(&plain), vec![plain.join("lib/start.js")]);
    }

    /// #1425: what a package's manifest says Node or a bundler loads besides `main` (its
    /// `module`, and every file its `exports` maps to, under each condition) is named as its
    /// `main` is, where it leaves the package's folder for one a chat writes.
    #[test]
    fn a_package_s_exports_and_module_are_named_as_its_main_is() {
        let project = tempfile::tempdir().expect("temp");
        let package = project.path().join("node_modules/hook");
        std::fs::create_dir_all(&package).expect("folder");
        std::fs::write(
            package.join("package.json"),
            r#"{
                "main": "../a/main.js",
                "module": "b/esm.mjs",
                "exports": {
                    ".": {
                        "import": {"node": "./c/node.mjs", "default": "./c/any.mjs"},
                        "require": ["./d/first.cjs", "./d/second.cjs"]
                    },
                    "./register": "./e/register",
                    "./features/*": "./f/*.js",
                    "./package.json": null,
                    "./out": "../../g/out.js",
                    "./deep": "./h/../../i/deep.js",
                    "./dep": "./node_modules/j/k.js"
                }
            }"#,
        )
        .expect("manifest");
        let found = named_in_project(project.path(), "node -r hook s.js");
        let modules = project.path().join("node_modules");
        assert!(covered(&found, &modules.join("a/main.js")), "{found:?}");
        for want in [
            "b/esm.mjs",
            "c/node.mjs",
            "c/any.mjs",
            "d/first.cjs",
            "d/second.cjs",
            "e/register",
        ] {
            assert!(covered(&found, &package.join(want)), "{want}: {found:?}");
        }
        // Node loads no `exports` target outside the package, so none is named (train 79
        // review): what a manifest says never reaches past its package but through `main`.
        let entries = package_entries(&package);
        for gone in ["g/out.js", "i/deep.js", "j/k.js"] {
            assert!(
                !entries.iter().any(|it| it.ends_with(gone)),
                "{gone}: {entries:?}"
            );
        }
        // A pattern names no one file, and a target that leaves for where no chat writes is not
        // followed.
        assert!(
            !found.iter().any(|it| it.to_string_lossy().contains('*')),
            "{found:?}"
        );
        std::fs::write(
            package.join("package.json"),
            r#"{"exports": "../../../../../../../../../../../opt/x.js"}"#,
        )
        .expect("manifest");
        let found = named_in_project(project.path(), "node -r hook s.js");
        assert!(
            !found.iter().any(|it| it.ends_with("opt/x.js")),
            "{found:?}"
        );
    }

    /// The exports' shorthand, a string alone, and a conditions object without subpaths.
    #[test]
    fn every_form_of_exports_gives_its_files() {
        let project = tempfile::tempdir().expect("temp");
        let package = project.path().join("node_modules/p");
        std::fs::create_dir_all(&package).expect("folder");
        for (manifest, want) in [
            (r#"{"exports": "./x.js"}"#, vec!["x.js"]),
            (
                r#"{"exports": {"require": "./r.cjs", "import": "./i.mjs"}}"#,
                vec!["r.cjs", "i.mjs"],
            ),
            (
                r#"{"main": "m.js", "module": "./esm/m.mjs"}"#,
                vec!["m.js", "esm/m.mjs"],
            ),
            (r#"{"exports": {"./a": "/abs/a.js"}}"#, vec![]),
            (r#"{"exports": "../x.js", "module": "../m.mjs"}"#, vec![]),
            (r#"{"exports": {"./a": "./b/../../c.js"}}"#, vec![]),
            (r#"{"module": "./esm/../m.mjs"}"#, vec![]),
        ] {
            std::fs::write(package.join("package.json"), manifest).expect("manifest");
            let mut want: Vec<PathBuf> = want.iter().map(|it| package.join(it)).collect();
            let mut found = package_entries(&package);
            want.sort();
            found.sort();
            assert_eq!(found, want, "{manifest}");
        }
    }

    /// A manifest a chat may have written is followed within its bounds (train 79 review): at
    /// most [`MOST_ENTRIES`] targets, and none more than [`MOST_EXPORTS_DEPTH`] levels down.
    #[test]
    fn a_hostile_exports_map_is_followed_only_within_its_bounds() {
        let project = tempfile::tempdir().expect("temp");
        let package = project.path().join("node_modules/p");
        std::fs::create_dir_all(&package).expect("folder");
        let many: Vec<String> = (0..1000).map(|n| format!("./f{n}.js")).collect();
        std::fs::write(
            package.join("package.json"),
            serde_json::json!({ "exports": many }).to_string(),
        )
        .expect("manifest");
        assert_eq!(package_entries(&package).len(), MOST_ENTRIES);
        let mut deep = serde_json::json!("./deep.js");
        for _ in 0..50 {
            deep = serde_json::json!({ "node": deep });
        }
        std::fs::write(
            package.join("package.json"),
            serde_json::json!({ "exports": deep }).to_string(),
        )
        .expect("manifest");
        assert!(package_entries(&package).is_empty());
    }

    #[test]
    fn a_variable_that_loads_code_names_its_value_whatever_is_there() {
        // #1418 round 2, D-1418-10 as amended: what a chat can make at the path never decides.
        let base = tempfile::tempdir().expect("a base");
        let project = base.path().canonicalize().expect("the base");
        let repo = project.join("ws/repo");
        for folder in ["envsh", "envsh.sh"] {
            std::fs::create_dir_all(repo.join(folder)).expect("a folder");
        }
        for (command, want) in [
            ("BASH_ENV=./.bashenv bash -c true", ".bashenv"),
            ("ENV=./rc sh -c true", "rc"),
            ("BASH_ENV=./envsh bash -c true", "envsh"),
            ("BASH_ENV=./envsh.sh bash -c true", "envsh.sh"),
            ("env PYTHONSTARTUP=./st python3", "st"),
            ("ZDOTDIR=./zd zsh -c x", "zd"),
            ("LD_PRELOAD=./lib/a.so:./lib/b x", "lib/b"),
            ("DYLD_INSERT_LIBRARIES=./i x", "i"),
            ("NODE_OPTIONS='--require ./hook' node s.js", "hook"),
            ("NODE_OPTIONS='--import=./reg' node s.js", "reg.mjs"),
            ("RUBYOPT=-r./rx ruby y.rb", "rx.rb"),
            ("PERL5OPT='-I./plib -MMy::Mod' perl x.pl", "plib"),
            ("PERL5OPT='-I./plib -MMy::Mod' perl x.pl", "My/Mod.pm"),
            ("GIT_SSH=./ssh git fetch", "ssh"),
            ("GIT_SSH_COMMAND='./ssh2 -i k' git fetch", "ssh2"),
            ("GIT_EXTERNAL_DIFF=./d git diff", "d"),
            ("GIT_PAGER=./p git log", "p"),
            ("git -c core.sshCommand=./s3 fetch", "s3"),
            ("JAVA_TOOL_OPTIONS=-javaagent:./ag java x", "ag"),
        ] {
            let found = named_in(&project, &project, command);
            assert!(found.contains(&repo.join(want)), "{command}: {found:?}");
        }
    }

    #[test]
    fn a_launcher_s_folder_and_value_written_against_its_option_are_read() {
        // #1418 round 2: `--chdir=`, `-C<dir>`, `--directory=`, `--project=`, BSD `env -P`,
        // a versioned package and `uv run -m`.
        for (command, want) in [
            ("env --chdir=/elsewhere node x.js", "/elsewhere/x.js"),
            ("env -C/elsewhere node x.js", "/elsewhere/x.js"),
            ("env -P /opt/bin node x.js", "/plane/ws/repo/x.js"),
            ("uv --directory=/x run server.py", "/x/server.py"),
            ("uv run --project=/y server.py", "/y/server.py"),
            ("npx tsx@4 server.ts", "/plane/ws/repo/server.ts"),
            ("npx -y tsx@4.7.0 s2.ts", "/plane/ws/repo/s2.ts"),
            ("uv run -m mod", "/plane/ws/repo/mod/__main__.py"),
        ] {
            let (_, found) = named_by_hook(command);
            assert!(found.contains(&PathBuf::from(want)), "{command}: {found:?}");
        }
    }

    #[test]
    fn a_runner_s_own_subcommand_is_no_script() {
        // #1418 review M1: `bun test`, `deno task`, a package script, never a folder.
        for command in [
            "bun test",
            "deno test",
            "bun run lint",
            "bun lint",
            "bun install",
            "bun x prettier",
            "node --run build",
            "deno task dev",
            "deno fmt",
            "tsx watch src/index.ts",
        ] {
            let (_, found) = named_by_hook(command);
            for path in &found {
                let name = path.file_name().and_then(|it| it.to_str()).unwrap_or("");
                assert!(
                    ![
                        "test", "lint", "install", "x", "prettier", "build", "task", "dev", "fmt",
                        "watch"
                    ]
                    .contains(&name.split('.').next().unwrap_or("")),
                    "{command}: {found:?}"
                );
            }
        }
        for (command, want) in [
            ("tsx watch src/index.ts", "src/index.ts"),
            ("bun run ./tools/x", "tools/x.ts"),
            ("bun run tools/y.ts", "tools/y.ts"),
            ("bun ./tools/z", "tools/z.js"),
            ("deno run -A main.ts", "main.ts"),
        ] {
            let (dir, found) = named_by_hook(command);
            assert!(found.contains(&dir.join(want)), "{command}: {found:?}");
        }
    }

    #[test]
    fn a_python_module_names_the_files_python_runs_never_its_package_folder() {
        // #1418 review M2.
        let (dir, found) = named_by_hook("python3 -m app.lint --fix");
        for want in [
            "app/lint.py",
            "app/lint/__main__.py",
            "app/lint/__init__.py",
            "app/__init__.py",
        ] {
            assert!(found.contains(&dir.join(want)), "{want}: {found:?}");
        }
        for not in ["app", "app/lint"] {
            assert!(!found.contains(&dir.join(not)), "{not}: {found:?}");
        }
        let found = named_by_server(r#"{"command": "python", "args": ["-m", "myserver"]}"#);
        let dir = Path::new("/plane/ws/repo");
        for want in [
            "myserver.py",
            "myserver/__main__.py",
            "myserver/__init__.py",
        ] {
            assert!(found.contains(&dir.join(want)), "{want}: {found:?}");
        }
        assert!(!found.contains(&dir.join("myserver")), "{found:?}");
    }

    #[test]
    fn an_option_s_value_is_not_its_runner_s_script() {
        // #1418 review F1.
        for (command, want, not) in [
            ("node --env-file .env server.js", "server.js", ".env"),
            (
                "node --env-file-if-exists .env.local s.js",
                "s.js",
                ".env.local",
            ),
            ("node --input-type module s.js", "s.js", "module"),
            ("node -C dev s.js", "s.js", "dev"),
            ("node --title t s.js", "s.js", "t"),
            (
                "deno run --config deno.json main.ts",
                "main.ts",
                "deno.json",
            ),
            ("deno run -c deno.json main.ts", "main.ts", "deno.json"),
            (
                "deno run --import-map map.json main.ts",
                "main.ts",
                "map.json",
            ),
            ("deno run --cert ca.pem main.ts", "main.ts", "ca.pem"),
            (
                "deno run --location https://x.test main.ts",
                "main.ts",
                "https:",
            ),
        ] {
            let (dir, found) = named_by_hook(command);
            assert!(found.contains(&dir.join(want)), "{command}: {found:?}");
            assert!(!found.contains(&dir.join(not)), "{command}: {found:?}");
        }
    }

    #[test]
    fn a_command_a_launcher_runs_is_read_as_a_command() {
        // #1418, D-1418-9: the rest of the line after a launcher is a command of its own.
        for (command, want) in [
            ("uv run server.py", "server.py"),
            ("uv run --with x python server.py", "server.py"),
            ("uv --directory tools run server.py", "tools/server.py"),
            ("uv run tsx s.ts", "s.ts"),
            ("npx tsx server.ts", "server.ts"),
            ("npx -y tsx server.ts", "server.ts"),
            ("pnpm exec tsx x.ts", "x.ts"),
            ("pnpm dlx tsx y.ts", "y.ts"),
            ("yarn tsx z.ts", "z.ts"),
            ("bunx tsx w.ts", "w.ts"),
            ("poetry run python p.py", "p.py"),
            ("env node server.js", "server.js"),
            ("env -i FOO=1 -u BAR node a.js", "a.js"),
            ("/usr/bin/env python3 hook.py", "hook.py"),
            ("exec node server.js", "server.js"),
            ("nohup bash run.sh", "run.sh"),
            ("python3.12 hook.py", "hook.py"),
            ("node22 s.js", "s.js"),
            ("make -f build.mk all", "build.mk"),
            ("make --file=rules.mk", "rules.mk"),
        ] {
            let (dir, found) = named_by_hook(command);
            assert!(found.contains(&dir.join(want)), "{command}: {found:?}");
        }
        let dir = Path::new("/plane/ws/repo");
        for (server, want) in [
            (
                r#"{"command": "uv", "args": ["run", "server.py"]}"#,
                "server.py",
            ),
            (
                r#"{"command": "npx", "args": ["tsx", "server.ts"]}"#,
                "server.ts",
            ),
            (
                r#"{"command": "env", "args": ["node", "server.js"]}"#,
                "server.js",
            ),
        ] {
            let found = named_by_server(server);
            assert!(found.contains(&dir.join(want)), "{server}: {found:?}");
        }
        // A package's own tool is no file.
        let (dir, found) = named_by_hook("npx prettier --write src");
        assert!(!found.contains(&dir.join("prettier")), "{found:?}");
    }

    #[test]
    fn a_value_attached_to_a_short_option_and_one_after_its_equals_sign_are_both_named() {
        // #1418, D-1418-4: `-f./a=b.awk` may be the file `./a=b.awk`.
        let (dir, found) = named_by_hook("awk -f./a=tools/b.awk data");
        for want in ["a=tools/b.awk", "tools/b.awk"] {
            assert!(found.contains(&dir.join(want)), "{want}: {found:?}");
        }
    }

    #[test]
    fn each_comma_separated_value_a_compiler_passes_on_is_named() {
        // #1418, D-1418-5: `-Wl,<a>,<b>` hands the linker each as a word of its own.
        for (command, want) in [
            ("gcc -Wl,-rpath,./lib a.c", "lib"),
            ("gcc -Wl,-T,./link/x.ld a.c", "link/x.ld"),
            ("gcc -Wl,--script=./link/y.ld a.c", "link/y.ld"),
            ("gcc -Wp,-include,./inc/z.h a.c", "inc/z.h"),
        ] {
            let (dir, found) = named_by_hook(command);
            assert!(found.contains(&dir.join(want)), "{command}: {found:?}");
        }
    }

    /// What `scripts_in` names for a hook running `command` in `project/ws/repo`, with
    /// `home` the home folder.
    fn named_in(project: &Path, home: &Path, command: &str) -> Vec<PathBuf> {
        let settings = serde_json::json!({"hooks": {"PostToolUse": [{"hooks": [
            {"type": "command", "command": command}
        ]}]}});
        scripts_in(
            project,
            &project.join("ws/repo"),
            ".claude/settings.json",
            &settings.to_string(),
            Some(home),
            &[],
        )
        .expect("read")
        .into_iter()
        .map(|named| named.path)
        .collect()
    }

    #[cfg(unix)]
    #[test]
    fn a_later_letter_s_absolute_value_is_held_to_the_project_by_any_name() {
        // #1418, D-1418-6: through a link to the project, and in another case where the volume
        // folds it.
        let base = tempfile::tempdir().expect("a base");
        let base = base.path().canonicalize().expect("the base");
        let project = base.join("plane");
        std::fs::create_dir_all(project.join("ws/repo")).expect("the project");
        let home = base.join("home");
        std::fs::create_dir_all(&home).expect("a home");
        std::os::unix::fs::symlink(&project, base.join("alias")).expect("a link");
        let linked = base.join("alias/inc/x.h");
        let found = named_in(&project, &home, &format!("cc -xI{} a.c", linked.display()));
        assert!(found.contains(&linked), "{found:?}");
        let folds = std::fs::metadata(base.join("PLANE")).is_ok();
        let cased = base.join("PLANE/inc/y.h");
        let found = named_in(&project, &home, &format!("cc -xI{} a.c", cased.display()));
        assert_eq!(found.contains(&cased), folds, "{found:?}");
        // Beside it, by any name, is still beside it.
        let beside = base.join("elsewhere/z.h");
        let found = named_in(&project, &home, &format!("cc -xI{} a.c", beside.display()));
        assert!(!found.contains(&beside), "{found:?}");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn a_later_letter_s_absolute_value_is_held_to_the_project_under_its_data_volume_name() {
        // #1418, D-1418-6.
        let base = tempfile::tempdir().expect("a base");
        let base = base.path().canonicalize().expect("the base");
        let project = base.join("plane");
        std::fs::create_dir_all(project.join("ws/repo")).expect("the project");
        let spelled = Path::new("/System/Volumes/Data")
            .join(project.strip_prefix("/").expect("absolute"))
            .join("inc/x.h");
        let found = named_in(&project, &base, &format!("cc -xI{} a.c", spelled.display()));
        assert!(found.contains(&spelled), "{found:?}");
    }

    #[test]
    fn a_submodule_s_git_directory_is_found_at_any_depth() {
        let clone = tempfile::tempdir().expect("a clone");
        let git = clone.path().join(".git");
        for dir in ["modules/a", "modules/a/modules/b", "modules/group/c"] {
            std::fs::create_dir_all(git.join(dir)).expect("a module");
            std::fs::write(git.join(dir).join("HEAD"), "ref: x\n").expect("HEAD");
        }
        let mut found = module_git_dirs(&git);
        found.sort();
        assert_eq!(
            found,
            [
                git.join("modules/a"),
                git.join("modules/a/modules/b"),
                git.join("modules/group/c")
            ]
        );
    }
}
