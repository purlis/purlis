// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The Bash guard's arm for git that rewrites the PROJECT ROOT's working tree from another
//! commit, from a sandboxed chat that may write that folder (qw346).
//!
//! A chat's sandbox never lets it write the project's manifest or what a program run later
//! loads code from ([`crate::sandbox::MANIFESTS`], [`crate::sandbox::PLANTED`], ADR 0067 §5).
//! git checks a tree out file by file: it writes each file it may, skips each it may not, and
//! then gives up. Measured on git 2.50 with a file the kernel refuses writing: `git rebase
//! <upstream>` wrote every other file of the upstream's tree, left HEAD and the index where they
//! were, and left no `rebase-merge`, `REBASE_HEAD` or `ORIG_HEAD` behind. The project is then
//! half-updated, with nothing to say an update was tried, and the next save would commit the
//! mix.
//!
//! **A command that cannot finish does not start.** So where the root folder holds a file the
//! sandbox denies, this arm refuses the commands that rewrite the tree from another commit
//! before they run, and says who can update the project instead.
//!
//! **And the way back stays open.** Putting files back as the current commit has them writes
//! only towards it, so it is allowed: `git restore`, `git checkout -- <paths>`, `git checkout
//! HEAD -- <paths>`, `git reset --hard` with no commit or with `HEAD`, and `--abort` or
//! `--quit` of an operation. One that names a file the sandbox denies is refused up front, with
//! the command the person runs for that file.
//!
//! **A mistake guard, not the boundary**, as [`crate::projectgit`] is: the sandbox already
//! refuses the writes. It speaks only for a chat the app says it sandboxed, and only where that
//! chat may write the root folder: a chat in a folder below the root stops on git's lock file
//! before anything is written, which is [`crate::projectgit`]'s to explain.
//!
//! It refuses every such command there, whatever the other commit changes. Asking git what an
//! update would write, so one that touches no denied file runs, needs the commit the update
//! lands on, which a `pull` does not have before it fetches: left as a follow-up.

use std::path::Path;

use crate::planeroot::{self, AliasEnd, OperandKind, OptKind};
use crate::sandbox::{self, Reach};
use crate::shellwrap;

/// The trace reason this arm is tallied under.
pub const REASON: &str = "project-root-half-update";

/// The options that end an operation git stopped in the middle of without rewriting the tree
/// from another commit: `--abort` puts it back as it was, `--quit` only forgets the operation.
const WAYS_BACK: [&str; 2] = ["--abort", "--quit"];

/// `git bisect` subcommands that only read.
const BISECT_READS: [&str; 5] = ["log", "view", "visualize", "terms", "help"];

/// `git stash` subcommands that write a stash's tree into the working tree.
const STASH_WRITES: [&str; 3] = ["pop", "apply", "branch"];

/// The words for the commit the working tree is already at.
fn is_current(word: &str) -> bool {
    matches!(word, "HEAD" | "@")
}

/// Where a restore's files come from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// The index: `git restore <paths>`, `git checkout -- <paths>`.
    Index,
    /// The current commit, spelled `HEAD` or `@`.
    Head,
    /// Any other commit.
    Other(String),
}

/// What one `git <sub> <post…>` writes into the working tree, as far as this arm judges it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Writes {
    /// Nothing from another commit.
    Nothing,
    /// The tree, from another commit.
    Tree,
    /// The tree, unless HEAD is already at this branch or commit: `git checkout <default>`.
    MovesTo(String),
    /// The paths named, from `source`. `listed` is whether every path is on the line: a list
    /// in a file (`--pathspec-from-file`) or on standard input is out of sight.
    Paths {
        source: Source,
        paths: Vec<String>,
        listed: bool,
    },
    /// `git checkout <a> <b…>` with no `--`: git decides whether `<a>` is a commit or a path.
    Operands(Vec<String>),
}

/// What `git <sub> <post…>` writes into the working tree. Read from the words alone; what
/// only git can say ([`Writes::MovesTo`], [`Writes::Operands`]) is left to the caller.
pub fn classify(sub: &str, post: &[String]) -> Writes {
    let has = |flag: &str| post.iter().any(|arg| arg == flag);
    let way_back = WAYS_BACK.iter().any(|flag| has(flag));
    let first_word = post.iter().find(|arg| !arg.starts_with('-'));
    let tree = |writes: bool| {
        if writes {
            Writes::Tree
        } else {
            Writes::Nothing
        }
    };
    match sub {
        "pull" => Writes::Tree,
        "rebase" => tree(!(way_back || has("--show-current-patch") || has("--edit-todo"))),
        // `--continue` only records the merge the person settled.
        "merge" => tree(!(way_back || has("--continue"))),
        "cherry-pick" | "revert" => tree(!way_back),
        "am" => tree(!(way_back || has("--show-current-patch"))),
        "stash" => tree(first_word.is_some_and(|word| STASH_WRITES.contains(&word.as_str()))),
        "bisect" => tree(first_word.is_some_and(|word| !BISECT_READS.contains(&word.as_str()))),
        // `-u` is what makes `read-tree` write the working tree; alone or in a cluster.
        "read-tree" => tree(
            post.iter()
                .any(|arg| arg.starts_with('-') && !arg.starts_with("--") && arg.contains('u')),
        ),
        "reset" => reset(post),
        "checkout" | "switch" => checkout(sub, post),
        "restore" => restore(post),
        _ => Writes::Nothing,
    }
}

/// `git reset`: only a mode that overwrites the working tree, and only to another commit.
fn reset(post: &[String]) -> Writes {
    if !post
        .iter()
        .any(|arg| planeroot::RESET_TREE_MODES.contains(&arg.as_str()))
    {
        return Writes::Nothing;
    }
    let before = post
        .iter()
        .position(|arg| arg == "--")
        .map_or(post, |cut| &post[..cut]);
    match before.iter().find(|arg| !arg.starts_with('-')) {
        Some(target) if !is_current(target) => Writes::Tree,
        _ => Writes::Nothing,
    }
}

/// Whether a list of paths is handed to git off the line.
fn paths_from_a_file(post: &[String]) -> bool {
    post.iter()
        .any(|arg| arg.starts_with("--pathspec-from-file"))
}

/// `git checkout` and `git switch`, as the branch guard lets them through
/// ([`planeroot::plane_root_branch_reason`]): a restore, or the root put back on its default
/// branch. One with an option that creates, detaches or cannot be placed is that guard's to
/// refuse, and is not judged here.
fn checkout(sub: &str, post: &[String]) -> Writes {
    let restore_shaped = post
        .iter()
        .filter(|arg| arg.starts_with('-') && *arg != "-" && *arg != "--")
        .all(|opt| planeroot::checkout_opt_kind(opt) == OptKind::Restore);
    if !restore_shaped {
        return Writes::Nothing;
    }
    let operand = |arg: &&String| *arg == "-" || !arg.starts_with('-');
    let mut words = post;
    if let (true, Some(cut)) = (sub == "checkout", post.iter().position(|arg| arg == "--")) {
        if cut + 1 < post.len() {
            let before: Vec<&String> = post[..cut].iter().filter(operand).collect();
            let source = match before.as_slice() {
                [] => Source::Index,
                [one] if is_current(one) => Source::Head,
                [one, ..] => Source::Other((*one).clone()),
            };
            return Writes::Paths {
                source,
                paths: post[cut + 1..].to_vec(),
                listed: true,
            };
        }
        words = &post[..cut];
    }
    let wants: Vec<String> = words.iter().filter(operand).cloned().collect();
    match wants.as_slice() {
        // Bare, or paths from a file out of the index: nothing from another commit.
        [] => Writes::Nothing,
        [target, ..] if sub == "switch" => Writes::MovesTo(target.clone()),
        _ => Writes::Operands(wants),
    }
}

/// `git restore`: its source, and the paths it names.
fn restore(post: &[String]) -> Writes {
    let mut source = Source::Index;
    let set = |value: &str| {
        if is_current(value) {
            Source::Head
        } else {
            Source::Other(value.to_string())
        }
    };
    let (mut staged, mut worktree) = (false, false);
    let mut paths: Vec<String> = Vec::new();
    let mut i = 0;
    while i < post.len() {
        let arg = post[i].as_str();
        i += 1;
        if arg == "--" {
            paths.extend(post[i..].iter().cloned());
            break;
        }
        if let Some(value) = arg.strip_prefix("--source=") {
            source = set(value);
        } else if arg == "--source" {
            if let Some(value) = post.get(i) {
                source = set(value);
            }
            i += 1;
        } else if matches!(
            arg,
            "--pathspec-from-file" | "--conflict" | "--inter-hunk-context" | "--unified"
        ) {
            i += 1; // its value is the next word
        } else if arg == "--staged" {
            staged = true;
        } else if arg == "--worktree" {
            worktree = true;
        } else if arg.starts_with("--") {
            // Any other long option names no source and no path.
        } else if arg.starts_with('-') && arg.len() > 1 {
            // A cluster of short options: `-s` takes the rest of the word, or the next one.
            for (at, letter) in arg.char_indices().skip(1) {
                match letter {
                    'S' => staged = true,
                    'W' => worktree = true,
                    's' | 'U' => {
                        let attached = &arg[at + letter.len_utf8()..];
                        let value = if attached.is_empty() {
                            i += 1;
                            post.get(i - 1).map(String::as_str)
                        } else {
                            Some(attached)
                        };
                        if let (true, Some(value)) = (letter == 's', value) {
                            source = set(value);
                        }
                        break;
                    }
                    _ => {}
                }
            }
        } else {
            paths.push(arg.to_string());
        }
    }
    // `--staged` alone writes the index and no file.
    if staged && !worktree {
        return Writes::Nothing;
    }
    // With `--staged`, git takes the files from HEAD where no source is named.
    if staged && source == Source::Index {
        source = Source::Head;
    }
    Writes::Paths {
        source,
        listed: !paths.is_empty() && !paths_from_a_file(post),
        paths,
    }
}

/// The names in the project's root folder that a chat's sandbox denies it writing, as found
/// there now: the project's manifests, and what a program run later loads code from. What is
/// inside git's own folder is no part of the working tree and is not listed.
pub fn denied_in(root: &Path) -> Vec<String> {
    found_in(root, &denied_names())
}

/// The names [`denied_in`] looks for in the root folder.
fn denied_names() -> Vec<&'static str> {
    let manifests = sandbox::MANIFESTS.iter().copied();
    let later_code = sandbox::PLANTED
        .iter()
        .filter(|planted| planted.reach == Reach::AndBelow)
        .map(|planted| planted.path)
        .filter(|path| !path.contains("**") && !path.starts_with(".git/"));
    manifests.chain(later_code).collect()
}

/// Which of `names` are in `root` now, whatever each is: a file, a folder or a link.
fn found_in(root: &Path, names: &[&str]) -> Vec<String> {
    names
        .iter()
        .filter(|name| std::fs::symlink_metadata(root.join(name)).is_ok())
        .map(|name| (*name).to_string())
        .collect()
}

/// Whether `path`, as a git command in the project names it, is a file a chat's sandbox denies
/// it writing: under a later-code name at any depth, or a manifest in the root folder. A path
/// that climbs (`..`) or starts at the top (`:/`, or absolute and inside `root`) is read as
/// being there.
pub fn path_is_denied(path: &str, root: &Path) -> bool {
    let from_top = path.strip_prefix(":/");
    let spelled = Path::new(from_top.unwrap_or(path));
    let inside = spelled.strip_prefix(root).ok();
    let rel = inside.unwrap_or(spelled);
    if crate::sandboxblock::is_planted(rel) {
        return true;
    }
    let names: Vec<&str> = rel
        .components()
        .filter_map(|part| match part {
            std::path::Component::Normal(name) => name.to_str(),
            _ => None,
        })
        .collect();
    let climbs = rel
        .components()
        .any(|part| part == std::path::Component::ParentDir);
    let Some(last) = names.last() else {
        return false;
    };
    let manifest = sandbox::MANIFESTS
        .iter()
        .any(|name| name.eq_ignore_ascii_case(last));
    manifest && (names.len() == 1 || climbs) && (inside.is_some() || !spelled.is_absolute())
}

/// Whether the shell, or git's own pathspec magic, decides what this path names.
fn out_of_sight(path: &str) -> bool {
    path.is_empty() || path.starts_with(':') || path.contains(['$', '`', '*', '?', '[', '{'])
}

/// What the chat is told when a command would rewrite the tree from another commit. `held`
/// is what the root folder holds that the sandbox denies ([`denied_in`]).
pub fn said_update(sub: &str, held: &[String]) -> String {
    format!(
        "`git {sub}` would rewrite the project's working tree from another commit, and this \
         chat's sandbox does not let it write every file in the project's root folder ({}). git \
         writes each file it may and skips each it may not, so the sandbox stops this partway \
         and would leave the project half-updated: most files as the other commit has them, \
         the rest as they were, the current commit unmoved, and nothing to say an update was \
         tried. Nothing was run. Run `purlis worktree update` here instead (#3831): the app \
         runs the window's own Update for this chat, outside this sandbox. It brings the \
         remote's commits in, replays the project's own commits on top, and leaves the \
         project exactly as it was where it cannot. It refuses while anything here is \
         uncommitted, and where the incoming commits change a file this sandbox denies; then \
         the person updates with Update in the window. Anything else this command was for, \
         the person runs in their own terminal. Read-only git works here (`git status`, `git \
         log`, `git diff`), and so does putting files back as the current commit has them: \
         `git restore --source=HEAD --worktree -- <paths>`.",
        named(held)
    )
}

/// What the chat is told when a restore names files the sandbox denies it writing: which, and
/// the command the person runs for them.
pub fn said_denied_paths(sub: &str, source: &Source, denied: &[String], root: &str) -> String {
    let quoted: Vec<String> = denied.iter().map(|p| crate::handoff::quote(p)).collect();
    let from = match source {
        Source::Index => String::new(),
        Source::Head => " --source=HEAD".to_string(),
        Source::Other(commit) => format!(" --source={}", crate::handoff::quote(commit)),
    };
    format!(
        "`git {sub}` would put back {}, which this chat's sandbox does not let it write: a \
         project manifest, or a file a program run later loads code from. git would restore \
         the other paths and fail on {}. Nothing was run. Restore the other paths without {}, \
         and ask the person to restore {} in their own terminal: `git -C {} restore{from} \
         --worktree -- {}`.",
        named(denied),
        those(denied),
        those(denied),
        those(denied),
        crate::handoff::quote(root),
        quoted.join(" ")
    )
}

/// What the chat is told when a restore from another commit may reach a file the sandbox
/// denies: purlis allows one only where every path is a file it can see.
pub fn said_from_another(sub: &str, commit: &str, held: &[String]) -> String {
    format!(
        "`git {sub}` would write files from another commit ('{commit}') into the project's \
         root folder, and purlis cannot tell that each is a file this chat's sandbox lets it \
         write ({} are not): a path here is a folder, a pattern, a list in a file, a file \
         that is not there yet, or one the shell names. A restore the sandbox stops partway \
         leaves a mix of two commits. Nothing was run. Name the files one by one, each a file \
         that is there now, and it runs; or ask the person to run this command in their own \
         terminal. Putting files back as the current commit has them is allowed as it is: \
         `git restore --source=HEAD --worktree -- <paths>`.",
        named(held)
    )
}

/// What an invocation purlis cannot place is told first: it may act on the root.
const MAY_BE_THE_ROOT: &str = "purlis cannot tell which repository this command acts on: a \
     directory it is sent to is named only when the shell runs it (a variable, a command \
     substitution, a glob, `~user`, `cd -`), so it may be the project's root folder. Spell the \
     path out and purlis checks it: `git -C <path> …` and `cd <path> && git …` are both read. \
     There, ";

/// A few names, as a sentence lists them.
fn named(names: &[String]) -> String {
    let shown: Vec<String> = names.iter().take(4).map(|n| format!("`{n}`")).collect();
    let more = names.len().saturating_sub(shown.len());
    match (shown.len(), more) {
        (0, _) => "the project's manifest".to_string(),
        (_, 0) => shown.join(", "),
        (_, more) => format!("{} and {more} more", shown.join(", ")),
    }
}

fn those(names: &[String]) -> &'static str {
    if names.len() == 1 {
        "that one"
    } else {
        "those"
    }
}

/// What only git can say about a command, asked of the project's repository.
pub trait Asks {
    /// Whether HEAD is already at `target`, so moving to it rewrites nothing.
    fn head_is_at(&self, target: &str) -> bool;
    /// What `git checkout <operand>` would make of the operand.
    fn operand_kind(&self, operand: &str) -> OperandKind;
    /// Whether `path`, as the command names it, is a plain file that is there now.
    fn is_a_file(&self, path: &str) -> bool;
}

/// [`Asks`], answered by git in `root` and by the folders the command may run in.
struct Asked<'a> {
    root: &'a str,
    cwd: &'a str,
}

impl Asks for Asked<'_> {
    fn head_is_at(&self, target: &str) -> bool {
        planeroot::head_is_at(self.root, target)
    }

    fn operand_kind(&self, operand: &str) -> OperandKind {
        planeroot::checkout_operand_kind(self.root, operand)
    }

    fn is_a_file(&self, path: &str) -> bool {
        // Wherever git may run it from, the folder the line starts in or the root: a plain
        // file in one, and nothing else of that name in the other.
        let found: Vec<bool> = [self.cwd, self.root]
            .iter()
            .filter(|dir| !dir.is_empty())
            .filter_map(|dir| std::fs::symlink_metadata(Path::new(dir).join(path)).ok())
            .map(|meta| meta.is_file())
            .collect();
        !found.is_empty() && found.iter().all(|file| *file)
    }
}

/// [`Asks`] where an earlier command on the line may have changed what git would say: HEAD is
/// at nothing, an operand is unknown, and no path is a file in sight.
struct Unmeasured;

impl Asks for Unmeasured {
    fn head_is_at(&self, _: &str) -> bool {
        false
    }

    fn operand_kind(&self, _: &str) -> OperandKind {
        OperandKind::Unknown
    }

    fn is_a_file(&self, _: &str) -> bool {
        false
    }
}

/// The refusal one `git <sub> <post…>` in the project's root folder earns, or `None`. `held`
/// is what that folder holds that the sandbox denies, and is not empty.
pub fn judged(
    sub: &str,
    post: &[String],
    held: &[String],
    root: &str,
    asks: &dyn Asks,
) -> Option<String> {
    let paths = |source: Source, paths: Vec<String>, listed: bool| {
        let denied: Vec<String> = paths
            .iter()
            .filter(|path| path_is_denied(path, Path::new(root)))
            .cloned()
            .collect();
        if !denied.is_empty() {
            return Some(said_denied_paths(sub, &source, &denied, root));
        }
        let Source::Other(commit) = source else {
            // Towards the commit the tree is at: git names any file it could not write, and
            // what it did write is as that commit has it.
            return None;
        };
        let seen = listed
            && paths
                .iter()
                .all(|path| !out_of_sight(path) && asks.is_a_file(path));
        (!seen).then(|| said_from_another(sub, &commit, held))
    };
    match classify(sub, post) {
        Writes::Nothing => None,
        Writes::Tree => Some(said_update(sub, held)),
        Writes::MovesTo(target) => (!asks.head_is_at(&target)).then(|| said_update(sub, held)),
        Writes::Paths {
            source,
            paths: named,
            listed,
        } => paths(source, named, listed),
        Writes::Operands(wants) => match wants.as_slice() {
            [] => None,
            // The branch guard let it through: a tracked path, or the default branch.
            [one] => match asks.operand_kind(one) {
                OperandKind::Path => paths(Source::Index, wants.clone(), true),
                _ => (!asks.head_is_at(one)).then(|| said_update(sub, held)),
            },
            [first, rest @ ..] => match asks.operand_kind(first) {
                OperandKind::Path => paths(Source::Index, wants.clone(), true),
                _ if is_current(first) => paths(Source::Head, rest.to_vec(), true),
                _ => paths(Source::Other(first.clone()), rest.to_vec(), true),
            },
        },
    }
}

/// Whether a chat started in `chat_dir` may write the project's root folder: one started
/// there, or in a folder that holds it. A chat in a folder below the root, or outside the
/// project, may not, and git stops on its lock file before it writes ([`crate::projectgit`]).
/// Where the folder is not known, or does not resolve, it may.
fn writes_the_root(root: &str, chat_dir: Option<&Path>) -> bool {
    let Some(chat_dir) = chat_dir else {
        return true;
    };
    match (std::fs::canonicalize(root), std::fs::canonicalize(chat_dir)) {
        (Ok(root), Ok(chat)) => root.starts_with(&chat),
        _ => true,
    }
}

/// The refusal for `cmd`, run in `cwd` by a sandboxed chat started in `chat_dir`, in the
/// project at `root`: `(subcommand, sentence)`, or `None`.
///
/// The caller has already asked whether the chat was given a sandbox. The line is read by the
/// walk the project-root guards share ([`planeroot::plane_root_git`]), so a `cd`, an alias, a
/// string a shell is handed and a directory the guard cannot name are read as they are there.
pub fn refusal(
    cmd: &str,
    cwd: &str,
    root: &str,
    chat_dir: Option<&Path>,
) -> Option<(String, String)> {
    refusal_where(cmd, cwd, root, chat_dir, &denied_in)
}

/// [`refusal`], with what the root folder holds that the sandbox denies asked of `denied`
/// in place of [`denied_in`]: for a caller that cannot write those names to look for them, as
/// a test run inside a chat's sandbox cannot.
pub fn refusal_where(
    cmd: &str,
    cwd: &str,
    root: &str,
    chat_dir: Option<&Path>,
    denied: &dyn Fn(&Path) -> Vec<String>,
) -> Option<(String, String)> {
    if !shellwrap::may_name(cmd, "git") {
        return None;
    }
    if !writes_the_root(root, chat_dir) {
        return None;
    }
    let root = planeroot::plane_root(root);
    refusal_of(
        &planeroot::plane_root_git(cmd, cwd, &root),
        cwd,
        &root,
        chat_dir,
        denied,
    )
}

/// [`refusal_where`], for a line the walk has already read: `line` is
/// [`planeroot::plane_root_git`]'s answer for it, against `root` as [`planeroot::plane_root`]
/// spells it. For a caller that asks more than one arm of a line and reads it once.
pub fn refusal_of(
    line: &[planeroot::RootInvocation],
    cwd: &str,
    root: &str,
    chat_dir: Option<&Path>,
    denied: &dyn Fn(&Path) -> Vec<String>,
) -> Option<(String, String)> {
    if !writes_the_root(root, chat_dir) {
        return None;
    }
    let mut held: Option<Vec<String>> = None;
    for inv in line {
        if inv.unread {
            continue; // the branch guard refuses what it could not read
        }
        let (sub, post, end) = inv.resolved(root);
        if end != AliasEnd::Resolved && end != AliasEnd::Unasked {
            continue; // refused by the branch guard, in its own words
        }
        if classify(&sub, &post) == Writes::Nothing {
            continue;
        }
        // Looked at only for a command this arm judges, and once for the line.
        let held = held.get_or_insert_with(|| denied(Path::new(root)));
        if held.is_empty() {
            return None;
        }
        let asked = Asked { root, cwd };
        // After a command that may write, what git said before the line ran may no longer
        // hold: nothing it would say is taken.
        let unmeasured = Unmeasured;
        let asks: &dyn Asks = if inv.after_a_writer {
            &unmeasured
        } else {
            &asked
        };
        let Some(why) = judged(&sub, &post, held, root, asks) else {
            continue;
        };
        let why = if inv.unnamed {
            format!("{MAY_BE_THE_ROOT}{why}")
        } else if inv.after_loose_cd {
            format!("{why} {}", planeroot::LOOSE_CD_HINT.trim_end())
        } else {
            why
        };
        return Some((sub, why));
    }
    None
}

#[cfg(test)]
mod tests;
