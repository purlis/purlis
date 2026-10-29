// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The Bash guard's arm for git that rewrites a working tree OTHER than the project's root
//! from another commit, from a sandboxed chat: a workspace's clone, a branch folder, a scratch
//! checkout (qw349). [`crate::rootupdate`] is the same arm for the root.
//!
//! A chat's sandbox never lets it write what a program run later loads code from, in any
//! folder ([`crate::sandbox::PLANTED`], ADR 0067 §5): an editor's tasks, a hook manager's
//! folder, an agent's settings. Plenty of repositories track such files. git checks a tree out
//! file by file, writing each it may and skipping each it may not. Measured on git 2.50 with a
//! file the kernel refuses writing, in a clone:
//!
//! - `git pull`, `git merge`, `git rebase` and `git reset --hard` wrote every other file of
//!   the other commit and failed, with the current commit unmoved;
//! - `git checkout <branch>` and `git switch` did the same and **reported success**, at the
//!   new commit, with the files they could not write left behind as if the chat had changed
//!   them;
//! - `git clone` wrote the files it could, left the index empty and said the checkout failed.
//!
//! **A command that cannot finish does not start**, and here the arm can say which those are.
//! Before the command runs it asks git which paths differ between the folder's commit and the
//! commit the command moves it to, and refuses only where one of them is a path the sandbox
//! denies. The refusal names those paths. A move that touches none of them runs.
//!
//! **A path git will not write is not counted.** Where a sparse checkout is on in the folder
//! (`core.sparseCheckout`, as git reads it there), git writes no path its patterns leave out,
//! whatever the move changes, so the arm asks git which of the denied paths those are
//! (`git sparse-checkout check-rules`) and drops them: only where nothing on the line can
//! change that checkout ([`Place::sparse`]). Where git does not answer, every one counts.
//!
//! **Where the other commit cannot be named before the command runs**, the arm falls back to
//! what [`crate::rootupdate`] does: it refuses where the repository tracks a denied path at
//! all, and says how to make the move one it can check. That is a `pull`, which fetches its
//! commit itself (fetch first, then merge or rebase what was fetched); a command with another
//! git before it on the same line, which may move the name it was read from; and the forms
//! whose commits are not on the line (`rebase -i`, `cherry-pick` of a range, `am`, `bisect`).
//!
//! **A mistake guard, not the boundary**: the sandbox already refuses the writes. So it reads
//! only what is tracked, by name, and never asks the kernel. Every question is put to git
//! through the one hardened runner ([`crate::worktree::git`]), reading no global or system
//! configuration, and only plumbing that compares trees or reads the sparse patterns: nothing
//! here reads a file's content
//! through a filter or starts a program a repository's configuration names.

use std::path::{Path, PathBuf};

use crate::planeroot::{self, AliasEnd, OptKind, RootInvocation};
use crate::rootupdate::{self, Source, Writes};
use crate::sandbox::{self, Reach};
use crate::shellwrap;
use crate::worktree::git;

/// The trace reason this arm is tallied under.
pub const REASON: &str = "clone-half-update";

/// Options of `git merge` that take no value and name no commit.
const MERGE_FLAGS: [&str; 26] = [
    "--ff",
    "--no-ff",
    "--ff-only",
    "--edit",
    "-e",
    "--no-edit",
    "--commit",
    "--no-commit",
    "--squash",
    "--no-squash",
    "--stat",
    "--no-stat",
    "-n",
    "-q",
    "--quiet",
    "-v",
    "--verbose",
    "--verify",
    "--no-verify",
    "--signoff",
    "--no-signoff",
    "--no-gpg-sign",
    "--progress",
    "--no-progress",
    "--autostash",
    "--no-autostash",
];

/// Options of `git rebase` that take no value, name no commit, and leave the commits it
/// replays those of the current branch that the upstream lacks. Never `-i`, `--exec`, `--onto`
/// or `--root`.
const REBASE_FLAGS: [&str; 27] = [
    "-r",
    "-q",
    "--quiet",
    "-v",
    "--verbose",
    "--stat",
    "--no-stat",
    "-n",
    "--verify",
    "--no-verify",
    "--autostash",
    "--no-autostash",
    "--no-ff",
    "--force-rebase",
    "-f",
    "--fork-point",
    "--no-fork-point",
    "--keep-empty",
    "--no-keep-empty",
    "--reapply-cherry-picks",
    "--no-reapply-cherry-picks",
    "--rebase-merges",
    "--no-rebase-merges",
    "--signoff",
    "--no-gpg-sign",
    "--update-refs",
    "--no-update-refs",
];

/// Options of `git cherry-pick` and `git revert` that take no value and name no commit.
const PICK_FLAGS: [&str; 12] = [
    "-x",
    "-e",
    "--edit",
    "--no-edit",
    "-n",
    "--no-commit",
    "-s",
    "--signoff",
    "--ff",
    "--allow-empty",
    "--allow-empty-message",
    "--no-gpg-sign",
];

/// Options that go on with an operation git stopped in the middle of. Its start was judged
/// with every commit it replays, so going on writes nothing the start did not answer for.
const GOES_ON: [&str; 3] = ["--continue", "--skip", "--resolved"];

/// The subcommands [`GOES_ON`] is read for.
const STOPPABLE: [&str; 4] = ["rebase", "cherry-pick", "revert", "am"];

/// Options of `git checkout` and `git switch` that neither create, detach nor restore, and
/// are harmless to a move: read here, where the root's branch guard refuses them outright.
const BRANCHING_FLAGS: [&str; 5] = ["-t", "--track", "--guess", "--discard-changes", "-l"];

/// Options of `git clone` that take the next word as their value.
const CLONE_VALUE_OPTS: [&str; 22] = [
    "--revision",
    "--bundle-uri",
    "--server-option",
    "--ref-format",
    "-b",
    "--branch",
    "-o",
    "--origin",
    "--depth",
    "--reference",
    "--reference-if-able",
    "--separate-git-dir",
    "-c",
    "--config",
    "--template",
    "-u",
    "--upload-pack",
    "--shallow-since",
    "--shallow-exclude",
    "--filter",
    "-j",
    "--jobs",
];

/// Options of `git clone` that write no working tree.
const CLONE_NO_TREE: [&str; 4] = ["-n", "--no-checkout", "--bare", "--mirror"];

/// The most tracked paths asked of git one by one ([`Asks::changed`]).
pub const MAX_ASKED_PATHS: usize = 64;

/// Why the commit a command moves the folder to is not one the arm can name before it runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unnamed {
    /// The command fetches it itself: `git pull`.
    Fetched,
    /// Its commits are not on the line, or are more than one.
    Elsewhere,
}

/// Where one `git <sub> <post…>` moves the working tree, as this arm judges it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Move {
    /// Nowhere: nothing is written from another commit.
    Nothing,
    /// To `target`, as a checkout does: what differs between the two commits is written.
    /// `forced` is whether what was changed in the folder is overwritten too (`reset --hard`,
    /// `checkout -f`, a restore from another commit).
    To { target: String, forced: bool },
    /// `git merge <target>`: what differs between the two commits, at most.
    Merges(String),
    /// `git rebase <upstream>`: the upstream's tree, then each commit it lacks, replayed.
    Replays(String),
    /// `git cherry-pick <commit>` or `git revert <commit>`: what that commit changed.
    Picks(String),
    /// `git stash pop` or `apply`: what the stash changed.
    Unstashes(String),
    /// `git checkout <first> <rest…>` with no `--`: git decides whether `first` is a commit.
    Word { first: String, rest: Vec<String> },
    /// A restore of the paths named.
    Restores { source: Source, paths: Vec<String> },
    /// The tree, from a commit the arm cannot name.
    Unnamed(Unnamed),
}

/// Where `git <sub> <post…>` moves the working tree. Read from the words alone.
pub fn moves(sub: &str, post: &[String]) -> Move {
    let has = |flag: &str| post.iter().any(|arg| arg == flag);
    if STOPPABLE.contains(&sub) && GOES_ON.iter().any(|flag| has(flag)) {
        return Move::Nothing;
    }
    if matches!(sub, "checkout" | "switch") && !restore_shaped(post) {
        return branching(post);
    }
    let unnamed = Move::Unnamed(Unnamed::Elsewhere);
    match rootupdate::classify(sub, post) {
        Writes::Nothing => Move::Nothing,
        Writes::Tree => match sub {
            "pull" => Move::Unnamed(Unnamed::Fetched),
            "merge" => match one_operand(post, &MERGE_FLAGS) {
                Some(target) => Move::Merges(target.unwrap_or_else(upstream)),
                None => unnamed,
            },
            "rebase" => match one_operand(post, &REBASE_FLAGS) {
                Some(target) => Move::Replays(target.unwrap_or_else(upstream)),
                None => unnamed,
            },
            "cherry-pick" | "revert" => match one_operand(post, &PICK_FLAGS) {
                Some(Some(commit)) if !commit.contains("..") && !commit.starts_with('^') => {
                    Move::Picks(commit)
                }
                _ => unnamed,
            },
            "reset" => match post.iter().find(|arg| !arg.starts_with('-')) {
                Some(target) => Move::To {
                    target: target.clone(),
                    forced: true,
                },
                None => unnamed,
            },
            "stash" => unstashes(post),
            _ => unnamed,
        },
        Writes::MovesTo(target) => Move::To {
            target: previous(&target),
            forced: forces(post),
        },
        Writes::Paths { source, paths, .. } => Move::Restores { source, paths },
        Writes::Operands(wants) => match wants.split_first() {
            Some((first, rest)) => Move::Word {
                first: previous(first),
                rest: rest.to_vec(),
            },
            None => Move::Nothing,
        },
    }
}

/// Whether `git <sub> <post…>` writes files from another commit than the one the tree is at:
/// every move but a restore towards that commit, which is the way back and is judged only by
/// the paths it names.
fn from_another(sub: &str, post: &[String]) -> bool {
    !matches!(
        moves(sub, post),
        Move::Nothing
            | Move::Restores {
                source: Source::Index | Source::Head,
                ..
            }
    )
}

/// The name git reads for the branch a bare `merge` or `rebase` takes.
fn upstream() -> String {
    "@{upstream}".to_string()
}

/// `-` is the branch the folder was on before.
fn previous(word: &str) -> String {
    if word == "-" {
        "@{-1}".to_string()
    } else {
        word.to_string()
    }
}

/// Whether every option of a `checkout` or `switch` is one a restore takes
/// ([`planeroot::checkout_opt_kind`]): the shapes [`rootupdate::classify`] reads.
fn restore_shaped(post: &[String]) -> bool {
    options(post).all(|opt| planeroot::checkout_opt_kind(opt) == OptKind::Restore)
}

/// The option words of a command: every word that starts with `-` but `-` and `--`.
fn options(post: &[String]) -> impl Iterator<Item = &String> {
    post.iter()
        .filter(|arg| arg.starts_with('-') && *arg != "-" && *arg != "--")
}

/// Whether a `checkout` or `switch` throws away what was changed in the folder.
fn forces(post: &[String]) -> bool {
    options(post).any(|opt| {
        matches!(opt.as_str(), "--force" | "--discard-changes")
            || (!opt.starts_with("--") && opt.contains('f'))
    })
}

/// The one commit a command names, where every option is in `flags`: `Some(None)` where it
/// names none, `None` where an option is not in `flags` (it may take a value, or change what
/// the command does) or more than one word is left.
fn one_operand(post: &[String], flags: &[&str]) -> Option<Option<String>> {
    let mut found: Option<String> = None;
    for arg in post {
        if arg.starts_with('-') {
            // A signing key may be attached (`-S<key>`, `--gpg-sign=<key>`), and names no commit.
            let signs = arg.starts_with("-S") || arg.starts_with("--gpg-sign");
            if !(signs || flags.contains(&arg.as_str())) {
                return None;
            }
        } else if found.replace(arg.clone()).is_some() {
            return None;
        }
    }
    Some(found)
}

/// `git stash pop|apply [<stash>]`, with no option but `-q` and `--index`.
fn unstashes(post: &[String]) -> Move {
    let unnamed = Move::Unnamed(Unnamed::Elsewhere);
    let mut words = post
        .iter()
        .filter(|arg| !matches!(arg.as_str(), "-q" | "--quiet" | "--index" | "--no-index"));
    match (words.next().map(String::as_str), words.next(), words.next()) {
        (Some("pop" | "apply"), stash, None) => match stash {
            Some(stash) if stash.starts_with('-') => unnamed,
            Some(stash) => Move::Unstashes(stash.clone()),
            None => Move::Unstashes("stash@{0}".to_string()),
        },
        _ => unnamed,
    }
}

/// A `checkout` or `switch` with an option that creates a branch or detaches: where it moves
/// the tree is the commit it starts from, where it names one.
fn branching(post: &[String]) -> Move {
    let unnamed = Move::Unnamed(Unnamed::Elsewhere);
    if post.iter().any(|arg| arg == "--") {
        return unnamed;
    }
    let mut creates = false;
    // The new branch's name is the first word after the options, unless it is attached.
    let mut name_follows = false;
    for opt in options(post) {
        let long = opt.split('=').next().unwrap_or(opt);
        match planeroot::checkout_opt_kind(opt) {
            // An orphan branch starts from no commit: `switch --orphan` empties the tree.
            OptKind::Create if long == "--orphan" => return unnamed,
            OptKind::Create => {
                creates = true;
                let attached = if opt.starts_with("--") {
                    opt.contains('=')
                } else {
                    let at = opt.find(['b', 'B', 'c', 'C']).unwrap_or(opt.len());
                    at + 1 < opt.len()
                };
                name_follows |= !attached;
            }
            OptKind::Detach | OptKind::Restore => {}
            OptKind::Unknown => {
                let flag = long.strip_prefix("--no-").map(|rest| format!("--{rest}"));
                let flag = flag.as_deref().unwrap_or(long);
                if !BRANCHING_FLAGS.contains(&flag) {
                    return unnamed;
                }
            }
        }
    }
    let mut words = post
        .iter()
        .filter(|arg| *arg == "-" || !arg.starts_with('-'));
    if creates && name_follows {
        words.next();
    }
    match (words.next(), words.next()) {
        // No start named: the branch is made, or HEAD detached, at the commit the tree is at.
        (None, _) => Move::Nothing,
        (Some(start), None) => Move::To {
            target: previous(start),
            forced: forces(post),
        },
        _ => unnamed,
    }
}

/// The working tree git acts on from a folder, as far as purlis could read it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Top {
    /// Its top.
    At(String),
    /// No repository at the folder or above it: git says so itself, and writes nothing.
    NoRepository,
    /// A repository is there and purlis could not read it, for the reason given: its `.git`
    /// names a git directory purlis does not follow ([`crate::worktree::link`]), or git did
    /// not answer. The chat's own git may read it all the same, so a move there is refused.
    Unread(&'static str),
}

/// What [`Top::Unread`] says where git was asked and did not answer.
const NO_ANSWER: &str = "git did not answer for the repository in this folder";

/// The commit a working tree is at, as far as git said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Head {
    /// A commit.
    At(String),
    /// None yet: a branch that has no commit (after `git init`, before the first checkout of
    /// a fetched commit). git writes the whole tree of whatever it lands on.
    Unborn,
    /// git did not say: not the same as none, and a move there does not start.
    Unasked,
}

/// What only git, and the folder, can say about a command.
pub trait Asks {
    /// The working tree git acts on from `dir`.
    fn top(&self, dir: &str) -> Top;
    /// The commit the working tree at `top` is at.
    fn head(&self, top: &str) -> Head;
    /// Whether the working tree at `top` has an index: one that was never checked out (a
    /// clone made with `--no-checkout`) has none, and the first command that checks anything
    /// out there writes the whole tree, whatever commit it is already at.
    fn has_index(&self, top: &str) -> bool;
    /// The paths `commit` tracks that `keep` keeps, or `None` where git could not say.
    /// [`Asks::tracked`], filtered; apart so that one line asks once for a repository.
    fn kept(
        &self,
        top: &str,
        commit: &str,
        keep: &dyn Fn(&str) -> bool,
    ) -> Option<std::rc::Rc<Vec<String>>> {
        let mut tracked = self.tracked(top, commit)?;
        tracked.retain(|path| keep(path));
        Some(std::rc::Rc::new(tracked))
    }
    /// The commit `name` names in the repository at `top`, or `None`.
    fn commit(&self, top: &str, name: &str) -> Option<String>;
    /// The one remote branch git would take a bare `word` for (`git checkout <word>` where no
    /// local branch has the name and exactly one remote does), or `None`.
    fn guessed(&self, top: &str, word: &str) -> Option<String>;
    /// Every path that differs between the commits `from` and `to`, or `None` where git could
    /// not say.
    fn differ(&self, top: &str, from: &str, to: &str) -> Option<Vec<String>>;
    /// Every path changed by a commit the current branch has and `onto` lacks: what a rebase
    /// onto it replays. `None` where git could not say.
    fn replayed(&self, top: &str, onto: &str) -> Option<Vec<String>>;
    /// [`Asks::replayed`], for the commits `head` has and `onto` lacks, whatever HEAD is:
    /// what a rebase of exactly `head` replays.
    fn replayed_from(&self, top: &str, onto: &str, head: &str) -> Option<Vec<String>>;
    /// Every path `commit` tracks, or `None` where git could not say.
    fn tracked(&self, top: &str, commit: &str) -> Option<Vec<String>>;
    /// Which of `paths`, each tracked by the current commit, are not in the folder as that
    /// commit has them. `None` where git could not say.
    fn changed(&self, top: &str, paths: &[String]) -> Option<Vec<String>>;
    /// Whether something is at `path`.
    fn is_there(&self, path: &Path) -> bool;
    /// Which of `paths` git leaves out of the working tree at `top` because a sparse checkout
    /// is on there and its patterns leave them out: git writes none of them, whatever a move
    /// changes. Empty where no sparse checkout is on, and `None` where git could not say.
    fn left_out(&self, top: &str, paths: &[String]) -> Option<Vec<String>>;
}

/// [`Asks`], answered by git through the hardened runner, with no global or system
/// configuration read, and by the folder itself.
pub struct Asked;

impl Asked {
    /// The isolation each question is asked under: the one the thread holds where purlis runs
    /// git for a chat (pinned to the repository it checked), and otherwise none of the global
    /// or system configuration.
    fn held() -> git::Isolated {
        git::isolation().unwrap_or_default()
    }

    /// One question's answer, where git answered it.
    fn git(dir: &str, args: &[&str]) -> Option<String> {
        let asked = git::isolated(&Self::held(), || {
            git::run(Path::new(dir), args, crate::doctor::CHECK_TIMEOUT)
        })
        .ok()?;
        asked.ok().then_some(asked.out)
    }

    /// A `-z` answer's paths.
    fn paths(answer: &str) -> Vec<String> {
        answer
            .split('\0')
            .filter(|path| !path.is_empty())
            .map(str::to_string)
            .collect()
    }
}

impl Asks for Asked {
    fn top(&self, dir: &str) -> Top {
        use crate::worktree::link::{self, Check, Link};
        // The folder that holds the `.git`, found as git finds it, by climbing: the runner
        // asks git about the folder it is given and never one above it, so a command run
        // below a clone's top would otherwise be asked about no repository at all. The same
        // read the runner makes of a `.git` file, so a link it will not follow is known here.
        let holds = match link::of(Path::new(dir), Check::Whole) {
            Ok(Link::None) => return Top::NoRepository,
            Ok(Link::Clone { top } | Link::Linked { top, .. }) => top,
            Err(why) => return Top::Unread(why),
        };
        let holds = holds.to_string_lossy();
        let top = Self::git(&holds, &["rev-parse", "--show-toplevel"]);
        match top.as_deref().map(|top| top.trim_end_matches(['\n', '\r'])) {
            Some(top) if !top.is_empty() => Top::At(top.to_string()),
            _ => Top::Unread(NO_ANSWER),
        }
    }

    fn head(&self, top: &str) -> Head {
        if let Some(oid) = self.commit(top, "HEAD") {
            return Head::At(oid);
        }
        // No commit, or no answer? A branch git names for HEAD, with no commit behind it, is
        // the first. Anything else (a detached HEAD at nothing, a repository git cannot read,
        // a call that did not return) is the second.
        match Self::git(top, &["symbolic-ref", "--quiet", "HEAD"]) {
            Some(branch) if !branch.trim().is_empty() => Head::Unborn,
            _ => Head::Unasked,
        }
    }

    fn has_index(&self, top: &str) -> bool {
        let Some(named) = Self::git(top, &["rev-parse", "--git-path", "index"]) else {
            // Not known: read as none, which only adds to what a move answers for.
            return false;
        };
        let named = Path::new(named.trim_end_matches(['\n', '\r']));
        if named.as_os_str().is_empty() {
            return false;
        }
        std::fs::symlink_metadata(Path::new(top).join(named)).is_ok()
    }

    fn commit(&self, top: &str, name: &str) -> Option<String> {
        // Never handed to git where it would read as an option.
        if name.is_empty() || name.starts_with('-') {
            return None;
        }
        let spec = format!("{name}^{{commit}}");
        let oid = Self::git(top, &["rev-parse", "--verify", "--quiet", &spec])?;
        let oid = oid.trim();
        (!oid.is_empty()).then(|| oid.to_string())
    }

    fn guessed(&self, top: &str, word: &str) -> Option<String> {
        if word.is_empty() || word.starts_with('-') || word.contains(['*', '?', '[', '\\']) {
            return None;
        }
        let pattern = format!("refs/remotes/*/{word}");
        let refs = Self::git(top, &["for-each-ref", "--format=%(refname)", &pattern])?;
        let mut names = refs.lines().filter(|name| !name.is_empty());
        match (names.next(), names.next()) {
            (Some(one), None) => Some(one.to_string()),
            _ => None,
        }
    }

    fn differ(&self, top: &str, from: &str, to: &str) -> Option<Vec<String>> {
        // The plumbing: two trees compared, the index and the files neither read nor refreshed.
        let args = [
            "diff-tree",
            "-r",
            "--name-only",
            "--no-renames",
            "-z",
            from,
            to,
            "--",
        ];
        Self::git(top, &args).map(|answer| Self::paths(&answer))
    }

    fn replayed(&self, top: &str, onto: &str) -> Option<Vec<String>> {
        self.replayed_from(top, onto, "HEAD")
    }

    fn replayed_from(&self, top: &str, onto: &str, head: &str) -> Option<Vec<String>> {
        if onto.starts_with('-') || head.starts_with('-') {
            return None;
        }
        let range = format!("{onto}..{head}");
        let commits = Self::git(top, &["rev-list", &range, "--"])?;
        if commits.trim().is_empty() {
            return Some(Vec::new());
        }
        // Each commit against each of its parents, a merge's included, read from standard
        // input so no commit is an argument.
        let args = [
            "diff-tree",
            "--stdin",
            "-r",
            "-m",
            "--root",
            "--no-commit-id",
            "--name-only",
            "--no-renames",
            "-z",
        ];
        let asked = git::isolated(&Self::held(), || {
            git::run_with_input(
                Path::new(top),
                &args,
                commits.into_bytes(),
                crate::doctor::CHECK_TIMEOUT,
            )
        })
        .ok()?;
        (asked.code == Some(0)).then(|| Self::paths(&String::from_utf8_lossy(&asked.out)))
    }

    fn tracked(&self, top: &str, commit: &str) -> Option<Vec<String>> {
        if commit.is_empty() || commit.starts_with('-') {
            return None;
        }
        let args = [
            "ls-tree",
            "-r",
            "--name-only",
            "--full-tree",
            "-z",
            commit,
            "--",
        ];
        Self::git(top, &args).map(|answer| Self::paths(&answer))
    }

    fn changed(&self, top: &str, paths: &[String]) -> Option<Vec<String>> {
        if paths.len() > MAX_ASKED_PATHS {
            return None;
        }
        let mut changed = Vec::new();
        let mut plain: Vec<&String> = Vec::new();
        for path in paths {
            let file = Path::new(top).join(path);
            // Gone, a folder or a link: not as the commit has it, as far as this asks. So is
            // a name git would not read off a line as it is written.
            let asked = std::fs::symlink_metadata(&file).is_ok_and(|meta| meta.is_file())
                && !path.contains(['\n', '\r'])
                && !path.starts_with('"');
            if asked {
                plain.push(path);
            } else {
                changed.push(path.clone());
            }
        }
        if plain.is_empty() {
            return Some(changed);
        }
        // Two questions for all of them, not two for each: what the commit holds at every
        // path, and the files' bytes as they are, through no filter a repository could name.
        // The paths reach git on its standard input, never as arguments.
        let listing = Self::git(top, &["ls-tree", "-r", "-z", "--full-tree", "HEAD", "--"])?;
        let committed: std::collections::HashMap<&str, &str> = listing
            .split('\0')
            .filter_map(|entry| {
                let (meta, path) = entry.split_once('\t')?;
                Some((path, meta.split(' ').nth(2)?))
            })
            .collect();
        let mut input = String::new();
        for path in &plain {
            input.push_str(path);
            input.push('\n');
        }
        let args = ["hash-object", "--no-filters", "--stdin-paths"];
        let asked = git::isolated(&Self::held(), || {
            git::run_with_input(
                Path::new(top),
                &args,
                input.into_bytes(),
                crate::doctor::CHECK_TIMEOUT,
            )
        })
        .ok()?;
        let now = String::from_utf8_lossy(&asked.out);
        let now: Vec<&str> = now.lines().collect();
        if asked.code != Some(0) || now.len() != plain.len() {
            return None;
        }
        for (path, now) in plain.into_iter().zip(now) {
            if committed.get(path.as_str()) != Some(&now.trim()) {
                changed.push(path.clone());
            }
        }
        Some(changed)
    }

    fn is_there(&self, path: &Path) -> bool {
        std::fs::symlink_metadata(path).is_ok()
    }

    fn left_out(&self, top: &str, paths: &[String]) -> Option<Vec<String>> {
        if paths.is_empty() {
            return Some(Vec::new());
        }
        // Only where git applies the patterns: with no sparse checkout on, a bit set by hand
        // keeps no file a move changes from being written.
        let on = Self::git(
            top,
            &["config", "--type=bool", "--get", "core.sparseCheckout"],
        )?;
        if on.trim() != "true" {
            return Some(Vec::new());
        }
        // git's own reading of the patterns, the paths on standard input and never arguments.
        // It answers the paths the patterns keep; the rest are left out.
        let mut input = Vec::new();
        for path in paths {
            input.extend_from_slice(path.as_bytes());
            input.push(0);
        }
        let asked = git::isolated(&Self::held(), || {
            git::run_with_input(
                Path::new(top),
                &["sparse-checkout", "check-rules", "-z"],
                input,
                crate::doctor::CHECK_TIMEOUT,
            )
        })
        .ok()?;
        if asked.code != Some(0) {
            return None;
        }
        let kept = String::from_utf8_lossy(&asked.out);
        let kept: std::collections::HashSet<&str> =
            kept.split('\0').filter(|path| !path.is_empty()).collect();
        Some(
            paths
                .iter()
                .filter(|path| !kept.contains(path.as_str()))
                .cloned()
                .collect(),
        )
    }
}

/// What a chat's sandbox denies it writing in one working tree, judged by name.
pub struct Denies {
    top: PathBuf,
    /// The manifests held for this chat ([`sandbox::manifests_held`]), lower-cased.
    manifests: Vec<String>,
}

impl Denies {
    /// For the working tree at `top`, for a chat started in `chat_dir` in the project at
    /// `root`.
    pub fn of(top: &str, root: &str, chat_dir: Option<&Path>) -> Self {
        let manifests = sandbox::manifests_held(Path::new(root), chat_dir)
            .into_iter()
            .map(|held| held.path.to_string_lossy().to_lowercase())
            .collect();
        Self {
            top: PathBuf::from(top),
            manifests,
        }
    }

    /// Whether the sandbox denies writing `rel`, a path git names from the top of the tree: a
    /// later-code name at any depth, whatever its case; a file below a name the sandbox never
    /// lets a chat create (`.claude`), where that folder is not there yet; or a manifest the
    /// chat's sandbox holds.
    pub fn path(&self, rel: &str, asks: &dyn Asks) -> bool {
        let lower = rel.to_lowercase();
        if crate::sandboxblock::is_planted(Path::new(&lower)) {
            return true;
        }
        let parts: Vec<&str> = rel.split('/').collect();
        let folders = &parts[..parts.len().saturating_sub(1)];
        let never_made = sandbox::PLANTED.iter().filter(|planted| {
            planted.reach == Reach::Itself && !planted.path.contains('/') && planted.path != ".git"
        });
        for planted in never_made {
            for (at, part) in folders.iter().enumerate() {
                if part.eq_ignore_ascii_case(planted.path)
                    && !asks.is_there(&self.top.join(parts[..=at].join("/")))
                {
                    return true;
                }
            }
        }
        let whole = self.top.join(rel).to_string_lossy().to_lowercase();
        self.manifests.contains(&whole)
    }

    /// Whether a path as a command names it, from `dir`, is one the sandbox denies.
    fn named(&self, path: &str, dir: &str, asks: &dyn Asks) -> bool {
        let from_top = path.strip_prefix(":/");
        let spelled = Path::new(from_top.unwrap_or(path));
        let whole = match (from_top, spelled.is_absolute()) {
            (Some(_), _) => self.top.join(spelled),
            (None, true) => spelled.to_path_buf(),
            (None, false) => Path::new(dir).join(spelled),
        };
        let rel = whole.strip_prefix(&self.top).unwrap_or(spelled);
        self.path(&rel.to_string_lossy(), asks)
    }
}

/// Whether a path is one a sentence repeats: short, and of the characters a file name is
/// ordinarily made of. A path comes from a repository's tree or a command's output, which
/// anyone can have written, and a sentence of purlis's own never carries their prose.
fn repeatable(path: &str) -> bool {
    path.len() <= 160
        && path
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/' | '@' | '+'))
}

/// A few paths, as a sentence lists them: those it repeats ([`repeatable`]), and how many
/// more there are.
fn listed(paths: &[String]) -> String {
    let shown: Vec<String> = paths
        .iter()
        .filter(|path| repeatable(path))
        .take(4)
        .map(|path| format!("`{path}`"))
        .collect();
    let more = paths.len().saturating_sub(shown.len());
    match (shown.is_empty(), more) {
        (true, 1) => "1 file".to_string(),
        (true, more) => format!("{more} files"),
        (false, 0) => shown.join(", "),
        (false, more) => format!("{} and {more} more", shown.join(", ")),
    }
}

/// What every refusal says the denied files are, and why git cannot be left to it.
const WHY: &str = "this chat's sandbox does not let it write them: they are files a program \
     run later loads code from (an editor's tasks, a hook, an agent's settings), or a project \
     manifest. git writes each file it may and skips each it may not, so the sandbox would \
     stop this partway and leave the folder half-updated: most files as the other commit has \
     them, and those as they were. A `checkout` or `switch` even reports success then, with \
     those files left behind as if you had changed them.";

/// A folder, as a sentence names it: shell-quoted where its name is short and made of a
/// name's characters ([`repeatable`]), and "that folder" where it is not. A folder's name is
/// data from whoever made the tree (a submodule's path, an unpacked archive).
fn folder(top: &str) -> String {
    if repeatable(top) {
        crate::handoff::quote(top)
    } else {
        "that folder".to_string()
    }
}

/// A folder, in a command a sentence gives: as [`folder`], with a placeholder where the name
/// is not repeated.
fn folder_word(top: &str) -> String {
    if repeatable(top) {
        top.to_string()
    } else {
        "<that folder>".to_string()
    }
}

/// The most of a word git is run with that a sentence repeats.
const MAX_WORD: usize = 40;

/// A word git was run with, as a sentence names it: the word where it is short and made of
/// a name's characters, and no word where it is not (an alias's word is the line's own).
fn as_typed(sub: &str) -> String {
    if sub.len() <= MAX_WORD && repeatable(sub) && !sub.contains('/') {
        format!("`git {sub}`")
    } else {
        "This git command".to_string()
    }
}

/// What the chat is told when the move would write files the sandbox denies: which, and who
/// can run it.
pub fn said_writes(sub: &str, denied: &[String], top: &str) -> String {
    format!(
        "{} would write {} in {}, and {WHY} Nothing was run. Ask the person to run this \
         command in their own terminal, in that folder, where no sandbox stops it. A move \
         that touches none of those files runs from this chat as it is, since purlis checks \
         the two commits before each one. Read-only git works here (`git status`, `git log`, \
         `git diff`).",
        as_typed(sub),
        listed(denied),
        folder(top)
    )
}

/// What the chat is told when the commit cannot be named before the command runs, in a
/// repository that tracks files the sandbox denies: why not, and how to make it checkable.
pub fn said_unnamed(sub: &str, tracked: &[String], top: &str, why: Unknown) -> String {
    let (cannot, instead) = match why {
        Unknown::Fetched => (
            "a commit it fetches itself, which purlis cannot name before it runs",
            "Where that folder is a workspace's clone or one of its branch folders, run \
             `purlis worktree update` in it instead: the app fetches and fast-forwards or \
             rebases it for this chat, and refuses only where the move would write one of \
             these files or overwrite uncommitted work. Anywhere else, fetch first, as a \
             command of its own (`git fetch`). Then run `git merge --ff-only @{u}` or `git \
             rebase @{u}`, each as a command of its own. Those are checked against what was \
             fetched, and they run where they touch none of these files. Where they do, ask \
             the person to run `git pull` in their own terminal, in that folder.",
        ),
        Unknown::NotAlone => (
            "a commit purlis cannot tell before the line runs: a command before it on the same \
             line may write, and so may move the name it is read from or the commit the \
             folder is at",
            "Run it as a command of its own, with nothing before it on the line but what only \
             reads (a `cd`, `ls`, `git status`, `git log` and `git diff` may stay). Then \
             purlis checks exactly which files it would write, and it runs where it touches \
             none of these.",
        ),
        Unknown::Elsewhere => (
            "a commit purlis could not name before it runs: it is not on the line, there is \
             more than one, an option changes what is written, or the name did not resolve \
             here",
            "Where the command can name one commit plainly (`git merge <commit>`, `git rebase \
             <upstream>`, `git cherry-pick <commit>`, `git checkout <branch>`, `git reset \
             --hard <commit>`), purlis checks which files it would write and it runs where it \
             touches none of these. To bring a workspace's clone or branch folder up to its \
             remote, run `purlis worktree update` in it: the app does that for this chat. \
             Otherwise ask the person to run this command in their own terminal, in that \
             folder.",
        ),
    };
    format!(
        "{} would rewrite the working tree of {} from {cannot}. The repository tracks {}, and \
         {WHY} Nothing was run. {instead}",
        as_typed(sub),
        folder(top),
        listed(tracked)
    )
}

/// What the chat is told when a clone's source tracks files the sandbox denies. The ways
/// through are the two that end with a whole folder: a clone the app makes, and the person's
/// own. A clone without its files is not one: its checkout stops at the same files.
pub fn said_clone(denied: &[String]) -> String {
    format!(
        "`git clone` would check out a repository that tracks {}, and {WHY} Nothing was run. \
         For a repository in this project's inventory, run `purlis clone <repo>`: the app \
         makes the clone for this chat, whole. Otherwise ask the person to clone it in their \
         own terminal, where no sandbox stops the checkout.",
        listed(denied)
    )
}

/// The refusal a rebase of exactly `head` onto exactly `onto`, both commit ids, earns in the
/// working tree at `top` (qw387), or `None`: what it writes is what differs between the two
/// and what each commit `head` has and `onto` lacks changed, read for those ids and never for
/// whatever HEAD names by then. What purlis runs for a chat is judged as it will run.
pub fn judged_rebase(
    top: &str,
    onto: &str,
    head: &str,
    place: &Place<'_>,
    asks: &dyn Asks,
) -> Option<String> {
    judged_rebase_from(top, onto, onto, head, place, asks)
}

/// [`judged_rebase`], for `git rebase --onto <onto> <upstream> <head>` (#3836): what it replays
/// is each commit `head` has and `upstream` lacks.
pub fn judged_rebase_from(
    top: &str,
    onto: &str,
    upstream: &str,
    head: &str,
    place: &Place<'_>,
    asks: &dyn Asks,
) -> Option<String> {
    let (Some(mut paths), Some(replayed)) = (
        asks.differ(top, head, onto),
        asks.replayed_from(top, upstream, head),
    ) else {
        return Some(said_unread("rebase", NO_ANSWER));
    };
    paths.extend(replayed);
    let denies = Denies::of(top, place.root, place.chat_dir);
    let mut denied = each_once(paths, &|path: &str| denies.path(path, asks));
    if place.sparse
        && let Some(out) = asks.left_out(top, &denied)
        && !out.is_empty()
    {
        let out: std::collections::HashSet<String> = out.into_iter().collect();
        denied.retain(|path| !out.contains(path));
    }
    (!denied.is_empty()).then(|| said_writes("rebase", &denied, top))
}

/// The refusal an app-run cut earns for a sandboxed chat (qw387), or `None`: a new branch
/// folder at `folder` checked out from `start` in the clone at `clone`, judged as the chat's
/// own checkout of it would be. Every file of `start` is written there, so a denied path it
/// tracks is refused where its content is not what the clone already has checked out at that
/// path, or where the clone has nothing there. `root` and `chat_dir` say which manifests the
/// chat's sandbox holds. Fails closed: where git does not answer, the cut is refused.
pub fn judged_cut(
    clone: &Path,
    start: &str,
    folder: &Path,
    root: &Path,
    chat_dir: Option<&Path>,
) -> Option<String> {
    let unread = || {
        Some(format!(
            "purlis could not read what the new branch folder would hold ({}), so the app did \
             not cut it for this chat. Ask the person to run the command in their own terminal",
            crate::shown::short(start)
        ))
    };
    if start.is_empty() || start.starts_with('-') {
        return unread();
    }
    let clone_s = clone.to_string_lossy();
    let Some(listing) = Asked::git(
        &clone_s,
        &["ls-tree", "-r", "-z", "--full-tree", start, "--"],
    ) else {
        return unread();
    };
    let denies = Denies::of(&folder.to_string_lossy(), &root.to_string_lossy(), chat_dir);
    let nothing_there = NothingThere(&Asked);
    let mut wanted: Vec<(String, String)> = Vec::new();
    for entry in listing.split('\0').filter(|entry| !entry.is_empty()) {
        let Some((meta, path)) = entry.split_once('\t') else {
            return unread();
        };
        if !denies.path(path, &nothing_there) {
            continue;
        }
        let mut meta = meta.split(' ');
        let (mode, oid) = (meta.next().unwrap_or(""), meta.nth(1).unwrap_or(""));
        wanted.push((path.to_owned(), format!("{mode} {oid}")));
    }
    if wanted.is_empty() {
        return None;
    }
    // What the clone holds at each path now: a plain file's bytes as git would store them,
    // through no filter; anything else (gone, a folder, a link) is not the same.
    let mut differ: Vec<String> = Vec::new();
    let mut plain: Vec<&(String, String)> = Vec::new();
    for want in &wanted {
        let file = clone.join(&want.0);
        let is_file = std::fs::symlink_metadata(&file).is_ok_and(|meta| meta.is_file());
        if is_file && want.1.starts_with("100") && !want.0.contains(['\n', '\r']) {
            plain.push(want);
        } else {
            differ.push(want.0.clone());
        }
    }
    if !plain.is_empty() {
        let mut input = String::new();
        for (path, _) in &plain {
            input.push_str(path);
            input.push('\n');
        }
        let asked = git::isolated(&Asked::held(), || {
            git::run_with_input(
                clone,
                &["hash-object", "--no-filters", "--stdin-paths"],
                input.into_bytes(),
                crate::doctor::CHECK_TIMEOUT,
            )
        });
        let Ok(asked) = asked else {
            return unread();
        };
        let now = String::from_utf8_lossy(&asked.out).into_owned();
        let now: Vec<&str> = now.lines().collect();
        if asked.code != Some(0) || now.len() != plain.len() {
            return unread();
        }
        for ((path, meta), oid) in plain.into_iter().zip(now) {
            // The mode as git would record the file there: executable or not.
            let mode = {
                use std::os::unix::fs::PermissionsExt as _;
                let exec = std::fs::metadata(clone.join(path))
                    .is_ok_and(|found| found.permissions().mode() & 0o111 != 0);
                if exec { "100755" } else { "100644" }
            };
            if *meta != format!("{mode} {}", oid.trim()) {
                differ.push(path.clone());
            }
        }
    }
    (!differ.is_empty()).then(|| {
        format!(
            "the new branch folder would be checked out from {} with {}, which differ from what \
             the clone has checked out, and {WHY} The app checks a folder out for this chat as \
             the chat's own checkout would be judged, so nothing was cut. Ask the person to run \
             the command in their own terminal",
            crate::shown::short(start),
            listed(&differ)
        )
    })
}

/// What the chat is told of a command that would write the files of a folder that has none
/// of them yet (cloned without a checkout): the whole tree is written, whatever commit the
/// folder is already at.
pub fn said_unchecked(sub: &str, denied: &[String], top: &str) -> String {
    format!(
        "{} would write the files of a folder that has none of them yet: {} holds a \
         repository whose files were never checked out (a clone made with `--no-checkout`). \
         That writes its whole tree, {} with it, which this chat's sandbox does not let it \
         write (files a program run later loads code from, or a project manifest). git would \
         write every other file and leave the folder without those, shown as deleted. Nothing \
         was run. Ask the person to finish the checkout in their own terminal: `git -C {} \
         restore --source=HEAD --staged --worktree -- :/`. For a repository in this project's \
         inventory, `purlis clone <repo>` has the app make the clone whole instead.",
        as_typed(sub),
        folder(top),
        listed(denied),
        folder_word(top)
    )
}

/// What the chat is told when a repository is there and purlis could not read it
/// ([`Top::Unread`], [`Head::Unasked`]): the chat's own git may read it all the same, so the
/// move does not start. `why` is one of purlis's own sentences, never git's words.
pub fn said_unread(sub: &str, why: &str) -> String {
    format!(
        "{} would rewrite a working tree from another commit, in a repository purlis could not \
         read to check it: {why}. Where the repository tracks a file this chat's sandbox does \
         not let it write (an editor's tasks, a hook, an agent's settings), the sandbox stops \
         such a move partway and leaves the folder half-updated. Nothing was run. Ask the \
         person to run this command in their own terminal, in that folder. Read-only git \
         works here (`git status`, `git log`, `git diff`).",
        as_typed(sub)
    )
}

/// What the chat is told of a word git may take for an alias the guard could not read
/// ([`AliasEnd::Unread`], [`AliasEnd::TooDeep`]), in a repository that tracks files the sandbox
/// denies: it may be a move.
pub fn said_alias(sub: &str, tracked: &[String], top: &str) -> String {
    format!(
        "{} may be an alias, and purlis could not read where it leads: this line sets git \
         configuration purlis does not read (a config file it names, a value only the shell \
         fills in, or one an earlier command on the line sets), or the alias leads to another \
         more times than purlis follows. It may rewrite the working tree of {} from another \
         commit. The repository tracks {}, and {WHY} Nothing was run. Run the command the \
         alias stands for, as a command of its own, and purlis checks which files it would \
         write.",
        as_typed(sub),
        folder(top),
        listed(tracked)
    )
}

/// What an invocation purlis cannot place is told: it may act on such a folder.
const CANNOT_PLACE: &str = "purlis cannot tell which folder this command acts on: the \
     directory it is sent to is named only when the shell runs it (a variable, a command \
     substitution, a glob, `~user`, `cd -`), git is pointed at a repository or a work tree by \
     name (`--git-dir`, `--work-tree`, `GIT_DIR`, `GIT_WORK_TREE`), or the line leaves more \
     folders in play than purlis reads. It rewrites a working tree from another commit, or \
     may, and where the repository tracks a file this chat's sandbox does not let it write \
     (an editor's tasks, a hook, an agent's settings) the sandbox stops that partway and \
     leaves the folder half-updated. Nothing was run. Spell the folder out and run git inside it, \
     and purlis checks which files the command would write: `git -C <path> …` and `cd <path> \
     && git …` are both read.";

/// How many commands on one line git is asked about ([`refusal`]).
pub const MAX_JUDGED: usize = 4;

/// What a line with more of them is told.
const TOO_MANY: &str = "this line runs more git commands that rewrite a working tree from \
     another commit than purlis checks on one line. Each is checked against the files this \
     chat's sandbox does not let it write (an editor's tasks, a hook, an agent's settings), \
     since a move that would write one stops partway and leaves the folder half-updated. \
     Nothing was run. Run them a few at a time, each line holding four at most.";

/// Why a move's commit is not one the arm named.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unknown {
    /// A `pull`.
    Fetched,
    /// Another git that writes is on the line.
    NotAlone,
    /// Anything else.
    Elsewhere,
}

/// Where a judged command runs, and for whom.
pub struct Place<'a> {
    /// The project's root folder.
    pub root: &'a str,
    /// The folder the chat was started in, where known.
    pub chat_dir: Option<&'a Path>,
    /// Whether no other git that writes is on the line, and nothing on it went unread: the
    /// commit a name stands for is then the one git will read too.
    pub alone: bool,
    /// Whether the folder's own sparse checkout decides what git writes: the command is
    /// [`Place::alone`], and nothing on the line changes its sparsity (configuration it sets,
    /// an option that ignores the patterns, a `git sparse-checkout` beside it). Only then are
    /// the paths the patterns leave out dropped.
    pub sparse: bool,
}

/// Whether `git <sub> <post…>` writes tracked files as the commit the folder is at has them:
/// a restore, a `checkout` or `switch` in any form, a `reset` that overwrites the tree. In a
/// folder that was checked out these write only what was changed there, and are the way
/// back. In one that never was ([`Asks::has_index`]) they write the whole tree.
fn towards_the_commit(sub: &str, post: &[String]) -> bool {
    match sub {
        "checkout" | "switch" => true,
        "restore" => {
            let has = |flag: &str| post.iter().any(|arg| arg == flag);
            let short = |letter: char| {
                post.iter().any(|arg| {
                    arg.starts_with('-') && !arg.starts_with("--") && arg.contains(letter)
                })
            };
            let staged = has("--staged") || short('S');
            let worktree = has("--worktree") || short('W');
            !staged || worktree
        }
        "reset" => post
            .iter()
            .any(|arg| planeroot::RESET_TREE_MODES.contains(&arg.as_str())),
        _ => false,
    }
}

/// Whether git guesses a remote branch for the one word of a `checkout` or `switch`: no
/// option creates or detaches, none says not to, and one word names where it goes.
fn guesses(sub: &str, post: &[String]) -> bool {
    if !matches!(sub, "checkout" | "switch") || post.iter().any(|arg| arg == "--") {
        return false;
    }
    let plain = options(post).all(|opt| {
        opt != "--no-guess"
            && !matches!(
                planeroot::checkout_opt_kind(opt),
                OptKind::Create | OptKind::Detach
            )
    });
    let words = post
        .iter()
        .filter(|arg| *arg == "-" || !arg.starts_with('-'))
        .count();
    plain && words == 1
}

/// `paths` that `keep` keeps, each once, in order.
fn each_once(paths: Vec<String>, keep: &dyn Fn(&str) -> bool) -> Vec<String> {
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut out: Vec<String> = Vec::new();
    for path in paths {
        if keep(&path) && seen.insert(path.clone()) {
            out.push(path);
        }
    }
    out
}

/// The refusal one `git <sub> <post…>`, run in `dir`, earns, or `None`.
pub fn judged(
    sub: &str,
    post: &[String],
    dir: &str,
    place: &Place<'_>,
    asks: &dyn Asks,
) -> Option<String> {
    let moved = moves(sub, post);
    let towards = towards_the_commit(sub, post);
    if moved == Move::Nothing && !towards {
        return None;
    }
    let another = from_another(sub, post);
    let top = match asks.top(dir) {
        Top::At(top) => top,
        // No repository there: git says so itself, and writes nothing.
        Top::NoRepository => return None,
        // One purlis could not read: the way back stays open, a move does not start.
        Top::Unread(why) => return another.then(|| said_unread(sub, why)),
    };
    let denies = Denies::of(&top, place.root, place.chat_dir);
    let denied_path = |path: &str| denies.path(path, asks);
    // What git will write of them: a sparse checkout leaves some out, and git writes none of
    // those whatever the move changes. Where git could not say, every one counts.
    let written_of =
        |paths: Vec<String>| match place.sparse.then(|| asks.left_out(&top, &paths)).flatten() {
            Some(out) if !out.is_empty() => {
                let out: std::collections::HashSet<String> = out.into_iter().collect();
                paths
                    .into_iter()
                    .filter(|path| !out.contains(path))
                    .collect()
            }
            _ => paths,
        };
    let denied = |paths: Vec<String>| written_of(each_once(paths, &denied_path));
    // The commit the folder is at. Where git did not say, nothing is known of it.
    let head = match asks.head(&top) {
        Head::At(oid) => Some(oid),
        Head::Unborn => None,
        Head::Unasked => return another.then(|| said_unread(sub, NO_ANSWER)),
    };
    // The denied files the current commit tracks: what a forced move may overwrite whatever
    // the two commits say, what an unnamed move is judged by, and what a folder that was
    // never checked out lacks. `None` where git did not list them.
    let held = || match &head {
        Some(_) => asks.kept(&top, "HEAD", &denied_path),
        None => Some(std::rc::Rc::new(Vec::new())),
    };
    // The tree the folder holds: its commit's, or none where it has no commit yet or was
    // never checked out. git then writes the whole tree of the commit it lands on.
    let holds = head.as_deref().filter(|_| asks.has_index(&top));
    if !another {
        // Towards the commit the folder is at. A restore that names a denied file is told so.
        if let Move::Restores { source, paths } = &moved {
            let named: Vec<String> = paths
                .iter()
                .filter(|path| denies.named(path, dir, asks))
                .cloned()
                .collect();
            if !named.is_empty() {
                return Some(rootupdate::said_denied_paths(
                    sub,
                    source,
                    &named,
                    &folder_word(&top),
                ));
            }
        }
        // In a folder that was checked out it writes only what was changed there, and git
        // names any file it could not write. In one that never was, it writes everything.
        if head.is_some() && holds.is_none() {
            return match held().map(|lacks| written_of(lacks.to_vec())) {
                Some(lacks) => (!lacks.is_empty()).then(|| said_unchecked(sub, &lacks, &top)),
                None => Some(said_unread(sub, NO_ANSWER)),
            };
        }
        return None;
    }
    let between = |from: &str, to: &str| {
        let (from, to) = (asks.commit(&top, from)?, asks.commit(&top, to)?);
        asks.differ(&top, &from, &to)
    };
    // The commit a word names for this command, as git will read it there. A `checkout` and
    // a `switch` take a branch of that name before anything else of it (a tag, say), where
    // git's other commands read the tag first; and where nothing has the name and one word
    // says where to go, they take the one remote branch of it.
    let branch_first = matches!(sub, "checkout" | "switch");
    let guessing = guesses(sub, post);
    let named = |name: &str| {
        branch_first
            .then(|| asks.commit(&top, &format!("refs/heads/{name}")))
            .flatten()
            .or_else(|| asks.commit(&top, name))
            .or_else(|| {
                let remote = guessing.then(|| asks.guessed(&top, name)).flatten()?;
                asks.commit(&top, &remote)
            })
    };
    let to = |target: &str, forced: bool| -> Option<Vec<String>> {
        let to = named(target)?;
        let Some(from) = holds else {
            return asks.tracked(&top, &to);
        };
        let mut paths = asks.differ(&top, from, &to)?;
        if forced {
            let held = held()?;
            if !held.is_empty() {
                paths.extend(asks.changed(&top, &held)?);
            }
        }
        Some(paths)
    };
    let mut why = if place.alone {
        Unknown::Elsewhere
    } else {
        Unknown::NotAlone
    };
    // Every path the move may write, or `None` where its commit cannot be named.
    let written: Option<Vec<String>> = match &moved {
        Move::Nothing => return None,
        Move::Unnamed(Unnamed::Fetched) => {
            why = Unknown::Fetched;
            None
        }
        Move::Unnamed(Unnamed::Elsewhere) => {
            why = Unknown::Elsewhere;
            None
        }
        Move::Restores { source, paths } => {
            let named: Vec<String> = paths
                .iter()
                .filter(|path| denies.named(path, dir, asks))
                .cloned()
                .collect();
            if !named.is_empty() {
                return Some(rootupdate::said_denied_paths(
                    sub,
                    source,
                    &named,
                    &folder_word(&top),
                ));
            }
            match source {
                Source::Other(commit) if place.alone => to(commit, true),
                _ => None,
            }
        }
        _ if !place.alone => None,
        Move::To { target, forced } => to(target, *forced),
        Move::Merges(target) => to(target, false),
        Move::Replays(target) => to(target, false).and_then(|mut paths| {
            let onto = asks.commit(&top, target)?;
            paths.extend(asks.replayed(&top, &onto)?);
            Some(paths)
        }),
        Move::Picks(commit) => between(&format!("{commit}^"), commit),
        Move::Unstashes(stash) => {
            // A stash that holds untracked files writes those too: not named here.
            if asks.commit(&top, &format!("{stash}^3")).is_some() {
                None
            } else {
                between(&format!("{stash}^1"), stash)
            }
        }
        Move::Word { first, rest } => {
            if named(first).is_some() {
                to(first, forces(post) || !rest.is_empty())
            } else {
                // Paths, out of the index.
                let named: Vec<String> = std::iter::once(first)
                    .chain(rest)
                    .filter(|path| denies.named(path, dir, asks))
                    .cloned()
                    .collect();
                if !named.is_empty() {
                    return Some(rootupdate::said_denied_paths(
                        sub,
                        &Source::Index,
                        &named,
                        &folder_word(&top),
                    ));
                }
                // Paths out of an index the folder does not have yet: nothing to write.
                return None;
            }
        }
    };
    match written {
        Some(paths) => {
            let denied = denied(paths);
            (!denied.is_empty()).then(|| said_writes(sub, &denied, &top))
        }
        None => match held().map(|held| written_of(held.to_vec())) {
            Some(held) => (!held.is_empty()).then(|| said_unnamed(sub, &held, &top, why)),
            // A commit git did not list: not known, so not started.
            None => Some(said_unread(sub, NO_ANSWER)),
        },
    }
}

/// The refusal a word git may take for an alias the guard could not read earns, run in `dir`:
/// it may be a move the arm cannot name, so it is refused where the repository tracks a file
/// the sandbox denies, as any such move is.
fn judged_unread_alias(sub: &str, dir: &str, place: &Place<'_>, asks: &dyn Asks) -> Option<String> {
    let top = match asks.top(dir) {
        Top::At(top) => top,
        Top::NoRepository => return None,
        Top::Unread(why) => return Some(said_unread(sub, why)),
    };
    let denies = Denies::of(&top, place.root, place.chat_dir);
    match asks.head(&top) {
        Head::Unborn => None,
        Head::Unasked => Some(said_unread(sub, NO_ANSWER)),
        Head::At(_) => match asks.kept(&top, "HEAD", &|path| denies.path(path, asks)) {
            Some(held) => (!held.is_empty()).then(|| said_alias(sub, &held, &top)),
            None => Some(said_unread(sub, NO_ANSWER)),
        },
    }
}

/// The refusal a `git clone <post…>` run in `dir` earns, or `None`: only where its source is a
/// repository on this machine, which can be asked what it tracks before anything is fetched.
/// A clone from a URL cannot be asked, and is left to run.
pub fn judged_clone(
    post: &[String],
    dir: &str,
    place: &Place<'_>,
    asks: &dyn Asks,
) -> Option<String> {
    // The commit checked out: the branch or the revision asked for, however it is attached.
    let mut commit: Option<String> = None;
    let mut words: Vec<&String> = Vec::new();
    let mut i = 0;
    while i < post.len() {
        let arg = &post[i];
        i += 1;
        if arg == "--" {
            words.extend(&post[i..]);
            break;
        }
        if CLONE_NO_TREE.contains(&arg.as_str()) {
            return None;
        }
        let attached = ["--branch=", "--revision="]
            .iter()
            .find_map(|opt| arg.strip_prefix(opt))
            .or_else(|| {
                arg.strip_prefix("-b")
                    .filter(|rest| !rest.is_empty() && !arg.starts_with("--"))
            });
        if let Some(named) = attached {
            commit = Some(named.to_string());
        } else if CLONE_VALUE_OPTS.contains(&arg.as_str()) {
            if matches!(arg.as_str(), "-b" | "--branch" | "--revision") {
                commit = post.get(i).cloned();
            }
            i += 1;
        } else if !arg.starts_with('-') {
            words.push(arg);
        }
    }
    let source = words.first()?;
    // A repository on this machine named as a URL is one all the same.
    let path = source.strip_prefix("file://").unwrap_or(source);
    // A URL, or a path only the shell can name: nothing to ask before it is fetched.
    if path.contains("://") || path.contains(['$', '`', '*', '?', '~']) {
        return None;
    }
    let from = Path::new(dir).join(path);
    if !asks.is_there(&from) {
        return None;
    }
    let from = from.to_string_lossy().into_owned();
    let top = match asks.top(&from) {
        Top::At(top) => top,
        Top::NoRepository | Top::Unread(_) => from,
    };
    let commit = commit.as_deref().unwrap_or("HEAD");
    // Where the clone lands, so a manifest it would put in a folder the chat's sandbox holds
    // is counted. Nothing is there yet, so a folder the sandbox never lets a chat create is
    // one too.
    let name = words
        .get(1)
        .map(|word| word.to_string())
        .or_else(|| {
            let last = Path::new(path.trim_end_matches('/')).file_name()?;
            let last = last.to_string_lossy();
            Some(last.strip_suffix(".git").unwrap_or(&last).to_string())
        })
        .unwrap_or_default();
    let lands = Path::new(dir).join(name).to_string_lossy().into_owned();
    let denies = Denies::of(&lands, place.root, place.chat_dir);
    let nothing_there = NothingThere(asks);
    let mut denied = asks.tracked(&top, commit)?;
    denied.retain(|path| denies.path(path, &nothing_there));
    (!denied.is_empty()).then(|| said_clone(&denied))
}

/// [`Asks`] for a folder a clone has not made yet: nothing is in it.
struct NothingThere<'a>(&'a dyn Asks);

impl Asks for NothingThere<'_> {
    fn top(&self, dir: &str) -> Top {
        self.0.top(dir)
    }
    fn head(&self, top: &str) -> Head {
        self.0.head(top)
    }
    fn has_index(&self, top: &str) -> bool {
        self.0.has_index(top)
    }
    fn commit(&self, top: &str, name: &str) -> Option<String> {
        self.0.commit(top, name)
    }
    fn guessed(&self, top: &str, word: &str) -> Option<String> {
        self.0.guessed(top, word)
    }
    fn differ(&self, top: &str, from: &str, to: &str) -> Option<Vec<String>> {
        self.0.differ(top, from, to)
    }
    fn replayed(&self, top: &str, onto: &str) -> Option<Vec<String>> {
        self.0.replayed(top, onto)
    }
    fn replayed_from(&self, top: &str, onto: &str, head: &str) -> Option<Vec<String>> {
        self.0.replayed_from(top, onto, head)
    }
    fn tracked(&self, top: &str, commit: &str) -> Option<Vec<String>> {
        self.0.tracked(top, commit)
    }
    fn changed(&self, top: &str, paths: &[String]) -> Option<Vec<String>> {
        self.0.changed(top, paths)
    }
    fn is_there(&self, _: &Path) -> bool {
        false
    }
    fn left_out(&self, top: &str, paths: &[String]) -> Option<Vec<String>> {
        self.0.left_out(top, paths)
    }
}

/// [`Asks`], each question put once for a line: what a folder's repository is, the commit it
/// is at, whether it was checked out, what a name names and which denied files a commit
/// tracks. A line's commands mostly act on one repository, and the way back is not counted
/// against what a line is asked about ([`MAX_JUDGED`]), so without this a line of restores
/// would start git for each.
struct Once<'a> {
    asks: &'a dyn Asks,
    tops: std::cell::RefCell<std::collections::HashMap<String, Top>>,
    heads: std::cell::RefCell<std::collections::HashMap<String, Head>>,
    indexes: std::cell::RefCell<std::collections::HashMap<String, bool>>,
    commits: std::cell::RefCell<std::collections::HashMap<(String, String), Option<String>>>,
    kept: std::cell::RefCell<KeptPaths>,
}

/// The denied files a commit tracks, by the repository's top and the commit's name.
type KeptPaths = std::collections::HashMap<(String, String), Option<std::rc::Rc<Vec<String>>>>;

impl<'a> Once<'a> {
    fn of(asks: &'a dyn Asks) -> Self {
        Once {
            asks,
            tops: std::cell::RefCell::default(),
            heads: std::cell::RefCell::default(),
            indexes: std::cell::RefCell::default(),
            commits: std::cell::RefCell::default(),
            kept: std::cell::RefCell::default(),
        }
    }
}

impl Asks for Once<'_> {
    // Each answer is kept under its own type, spelt out: one helper generic over the answer
    // would do, and reads to a code scanner as a copy of any value in the crate.
    fn top(&self, dir: &str) -> Top {
        if let Some(known) = self.tops.borrow().get(dir) {
            return Top::clone(known);
        }
        let found = self.asks.top(dir);
        self.tops
            .borrow_mut()
            .insert(dir.to_string(), Top::clone(&found));
        found
    }
    fn head(&self, top: &str) -> Head {
        if let Some(known) = self.heads.borrow().get(top) {
            return Head::clone(known);
        }
        let found = self.asks.head(top);
        self.heads
            .borrow_mut()
            .insert(top.to_string(), Head::clone(&found));
        found
    }
    fn has_index(&self, top: &str) -> bool {
        if let Some(known) = self.indexes.borrow().get(top) {
            return *known;
        }
        let found = self.asks.has_index(top);
        self.indexes.borrow_mut().insert(top.to_string(), found);
        found
    }
    fn kept(
        &self,
        top: &str,
        commit: &str,
        keep: &dyn Fn(&str) -> bool,
    ) -> Option<std::rc::Rc<Vec<String>>> {
        // One filter for a repository on one line: what the chat's sandbox denies there.
        let key = (top.to_string(), commit.to_string());
        if let Some(known) = self.kept.borrow().get(&key) {
            return known.as_ref().map(std::rc::Rc::clone);
        }
        let found = self.asks.kept(top, commit, keep);
        self.kept
            .borrow_mut()
            .insert(key, found.as_ref().map(std::rc::Rc::clone));
        found
    }
    fn commit(&self, top: &str, name: &str) -> Option<String> {
        let key = (top.to_string(), name.to_string());
        if let Some(known) = self.commits.borrow().get(&key) {
            return known.as_deref().map(str::to_string);
        }
        let found = self.asks.commit(top, name);
        self.commits
            .borrow_mut()
            .insert(key, found.as_deref().map(str::to_string));
        found
    }
    fn guessed(&self, top: &str, word: &str) -> Option<String> {
        self.asks.guessed(top, word)
    }
    fn differ(&self, top: &str, from: &str, to: &str) -> Option<Vec<String>> {
        self.asks.differ(top, from, to)
    }
    fn replayed(&self, top: &str, onto: &str) -> Option<Vec<String>> {
        self.asks.replayed(top, onto)
    }
    fn replayed_from(&self, top: &str, onto: &str, head: &str) -> Option<Vec<String>> {
        self.asks.replayed_from(top, onto, head)
    }
    fn tracked(&self, top: &str, commit: &str) -> Option<Vec<String>> {
        self.asks.tracked(top, commit)
    }
    fn changed(&self, top: &str, paths: &[String]) -> Option<Vec<String>> {
        self.asks.changed(top, paths)
    }
    fn is_there(&self, path: &Path) -> bool {
        self.asks.is_there(path)
    }
    fn left_out(&self, top: &str, paths: &[String]) -> Option<Vec<String>> {
        self.asks.left_out(top, paths)
    }
}

/// The refusal for `cmd`, run in `cwd` by a sandboxed chat started in `chat_dir`, in the
/// project at `root`: `(subcommand, sentence)`, or `None`.
///
/// The caller has already asked whether the chat was given a sandbox. The line is read by the
/// walk the project-root guards share ([`planeroot::line_git`]), so a `cd`, an alias, a string
/// a shell is handed and a directory the guard cannot name are read as they are there. What
/// acts on the project's root is left to those guards.
pub fn refusal(
    cmd: &str,
    cwd: &str,
    root: &str,
    chat_dir: Option<&Path>,
) -> Option<(String, String)> {
    refusal_with(cmd, cwd, root, chat_dir, &Asked)
}

/// [`refusal`], with git and the folder asked through `asks`.
pub fn refusal_with(
    cmd: &str,
    cwd: &str,
    root: &str,
    chat_dir: Option<&Path>,
    asks: &dyn Asks,
) -> Option<(String, String)> {
    if !shellwrap::may_name(cmd, "git") {
        return None;
    }
    let root = planeroot::plane_root(root);
    let line = planeroot::line_git(cmd, cwd, &root);
    refusal_of(&line, cwd, &root, chat_dir, asks)
}

/// The options any move may carry and still be judged by its folder's sparse checkout.
const SPARSE_KEEPS_ANY: [&str; 4] = ["-q", "--quiet", "-v", "--verbose"];

/// Those of a `checkout` or `switch`, beside [`BRANCHING_FLAGS`].
const SPARSE_KEEPS_CHECKOUT: [&str; 9] = [
    "-f", "--force", "--detach", "-b", "-B", "-c", "-C", "-m", "--merge",
];

/// Those of a `reset`.
const SPARSE_KEEPS_RESET: [&str; 5] = ["--hard", "--keep", "--merge", "--soft", "--mixed"];

/// Those of a `rebase`, beside [`REBASE_FLAGS`]: signing only.
const SPARSE_KEEPS_REBASE: [&str; 2] = ["-S", "--gpg-sign"];

/// Those of a `cherry-pick` or `revert`, beside [`PICK_FLAGS`]: signing only.
const SPARSE_KEEPS_PICK: [&str; 2] = ["-S", "--gpg-sign"];

/// Those of a `restore`.
const SPARSE_KEEPS_RESTORE: [&str; 5] = ["-s", "--staged", "-S", "--worktree", "-W"];

/// Whether every option of `git <sub> <post…>` is one its own subcommand's list holds, spelled
/// out whole, each one that changes nothing about which paths git writes. Any other option, an
/// abbreviation or a bundle of short options included, and the paths the folder's sparse
/// checkout leaves out count again ([`Place::sparse`]). A flag one subcommand reads as
/// harmless may be another's command to run, so no list is shared between them.
fn sparse_keeps(sub: &str, post: &[String]) -> bool {
    let own: &[&str] = match sub {
        "checkout" | "switch" => &[&SPARSE_KEEPS_CHECKOUT[..], &BRANCHING_FLAGS[..]].concat(),
        "reset" => &SPARSE_KEEPS_RESET,
        "merge" | "pull" => &MERGE_FLAGS,
        "rebase" => &[&REBASE_FLAGS[..], &SPARSE_KEEPS_REBASE[..]].concat(),
        "cherry-pick" | "revert" => &[&PICK_FLAGS[..], &SPARSE_KEEPS_PICK[..]].concat(),
        "restore" => &SPARSE_KEEPS_RESTORE,
        _ => &[],
    };
    post.iter()
        .take_while(|word| word.as_str() != "--")
        .filter(|word| word.starts_with('-'))
        .all(|word| {
            let word = word.as_str();
            SPARSE_KEEPS_ANY.contains(&word)
                || own.contains(&word)
                || (sub == "restore" && word.starts_with("--source="))
        })
}

/// What, set on the line, may run a program or change a setting in the middle of a move: the
/// editors git asks for a message or a todo list.
const EDITOR_ENV: [&str; 4] = ["GIT_EDITOR=", "GIT_SEQUENCE_EDITOR=", "EDITOR=", "VISUAL="];

/// Whether `inv` sets git configuration on the line: `-c`, `--config-env`, or any `GIT_*`
/// variable or an editor among the assignments and exports it runs with.
fn configures(inv: &RootInvocation) -> bool {
    inv.pre
        .iter()
        .any(|word| word.starts_with("-c") || word.starts_with("--config-env"))
        || inv.env.newest_first().chain(&inv.own_env).any(|assign| {
            // Any of git's own variables may change what it does mid-move: its config, its
            // directories, a program it starts (an askpass, a proxy, an ssh).
            let assign = assign.trim_start_matches("export ");
            assign.starts_with("GIT_") || EDITOR_ENV.iter().any(|editor| assign.starts_with(editor))
        })
}

/// How many folders one line's ways back are asked about ([`refusal_of`]). The way back is
/// never counted as a move; this only bounds how many different repositories one line can
/// make the guard read.
pub const MAX_FOLDERS: usize = 32;

/// What a line whose ways back reach more folders than that is told.
const TOO_MANY_FOLDERS: &str = "this line runs git that writes working trees in more folders \
     than purlis reads on one line. Each is read for files this chat's sandbox does not let \
     it write (an editor's tasks, a hook, an agent's settings), since a checkout that would \
     write one stops partway. Nothing was run. Run them a few folders at a time.";

/// [`refusal_with`], for a line the walk has already read: `line` is [`planeroot::line_git`]'s
/// answer for it, against `root` as [`planeroot::plane_root`] spells it. For a caller that
/// asks more than one arm of a line and reads it once.
pub fn refusal_of(
    line: &planeroot::LineGit,
    cwd: &str,
    root: &str,
    chat_dir: Option<&Path>,
    asks: &dyn Asks,
) -> Option<(String, String)> {
    let asks = &Once::of(asks);
    // The subcommand git runs and its words, where the guard read them; and whether the word
    // may be an alias it could not read, which may then be any command (the word is kept as
    // written). One whose options hide the word itself is the root guards' to refuse.
    let read = |inv: &RootInvocation| {
        if inv.unread {
            return None;
        }
        let (sub, post, end) = inv.resolved(root);
        // The shell's redirections are its own, never git's words: `git rebase <upstream>
        // 2>&1` names one commit, as `git rebase <upstream>` does.
        // A quoted word spelled like one is an operand: then nothing is dropped.
        let words = |post: &[String]| -> Vec<String> {
            if line.quoted_redirection {
                return post.to_vec();
            }
            shellwrap::without_redirections(post)
                .into_iter()
                .cloned()
                .collect()
        };
        Some(match end {
            AliasEnd::Resolved | AliasEnd::Unasked => (sub, words(&post), false),
            AliasEnd::Unread | AliasEnd::TooDeep => (inv.sub.clone(), words(&inv.post), true),
        })
    };
    let hinted = |inv: &RootInvocation, why: String| {
        if inv.after_loose_cd {
            format!("{why} {}", planeroot::LOOSE_CD_HINT.trim_end())
        } else {
            why
        }
    };
    // The word a verdict is tallied under: git's own, or `alias` for a word that is the
    // line's and may be anything.
    let shape = |sub: String, unread_alias: bool| {
        if unread_alias && !(sub.len() <= MAX_WORD && repeatable(&sub)) {
            "alias".to_string()
        } else {
            sub
        }
    };
    // How many MOVES git has been asked about: each costs a few git processes for each
    // folder it may run in, so a line is judged up to [`MAX_JUDGED`] of them and refused past
    // that, which keeps the arm's time bounded however long the line is. The way back (a
    // restore to the commit the folder is at, an `--abort`) is never counted: its questions
    // are put once for a folder ([`Once`]), and only the folders are bounded.
    let mut moves_asked = 0usize;
    let mut folders: std::collections::HashSet<&str> = std::collections::HashSet::new();
    // The folder's own sparse checkout decides only where nothing on the line can change it:
    // no configuration set anywhere on it, no alias the guard did not read, and no git that
    // is `git sparse-checkout` once its alias is read.
    let line_leaves_sparsity = line
        .elsewhere
        .iter()
        .map(|each| &each.inv)
        .chain(&line.root)
        .all(|inv| {
            !configures(inv)
                && read(inv).is_some_and(|(sub, _, unread)| !unread && sub != "sparse-checkout")
        });
    for each in &line.elsewhere {
        let Some((sub, post, unread_alias)) = read(&each.inv) else {
            continue;
        };
        let clone = !unread_alias && sub == "clone";
        let moves_from_another = unread_alias || clone || from_another(&sub, &post);
        if !moves_from_another && !towards_the_commit(&sub, &post) {
            continue;
        }
        if !clone && (each.pointed || each.more_dirs) && moves_from_another {
            return Some((shape(sub, unread_alias), CANNOT_PLACE.to_string()));
        }
        if moves_from_another {
            moves_asked += 1;
            if moves_asked > MAX_JUDGED {
                return Some((shape(sub, unread_alias), TOO_MANY.to_string()));
            }
        }
        // What comes before it on the line may move what a name stands for.
        let alone = !each.after_a_writer && !line.too_deep;
        let place = Place {
            root,
            chat_dir,
            alone,
            sparse: alone && line_leaves_sparsity && sparse_keeps(&sub, &post),
        };
        for dir in &each.dirs {
            if !moves_from_another && folders.insert(dir.as_str()) && folders.len() > MAX_FOLDERS {
                return Some((sub, TOO_MANY_FOLDERS.to_string()));
            }
            let found = if clone {
                judged_clone(&post, dir, &place, asks)
            } else if unread_alias {
                judged_unread_alias(&sub, dir, &place, asks)
            } else {
                judged(&sub, &post, dir, &place, asks).map(|why| hinted(&each.inv, why))
            };
            if let Some(why) = found {
                return Some((shape(sub, unread_alias), why));
            }
        }
    }
    let place = Place {
        root,
        chat_dir,
        alone: false,
        sparse: false,
    };
    for inv in &line.root {
        let Some((sub, post, unread_alias)) = read(inv) else {
            continue;
        };
        // An alias out of sight, in the root or where the guard cannot name: the root's
        // branch guard refuses it for every chat, before this arm is asked.
        if unread_alias {
            continue;
        }
        // A clone run inside the project's own tree makes a repository of its own there.
        if sub == "clone" {
            if inv.unnamed {
                continue;
            }
            moves_asked += 1;
            if moves_asked > MAX_JUDGED {
                return Some((sub, TOO_MANY.to_string()));
            }
            // Where git runs it: its own `-C`s from where the line starts.
            let dir = planeroot::git_target(cwd, &inv.pre, &[])
                .into_iter()
                .next()
                .unwrap_or_else(|| cwd.to_string());
            if let Some(why) = judged_clone(&post, &dir, &place, asks) {
                return Some((sub, why));
            }
            continue;
        }
        // Sent where the guard cannot name: it may be a folder this arm answers for.
        if inv.unnamed && from_another(&sub, &post) {
            return Some((sub, hinted(inv, CANNOT_PLACE.to_string())));
        }
    }
    None
}

/// The paths git said it could not write, in `text` (a command's standard error): each line
/// of git's own for a file it could not create or replace because the system refused.
fn unwritten(text: &str) -> Vec<String> {
    // What each sandbox answers a write it denies with (`sandbox::held`): Seatbelt's, a
    // read-only mount's under bubblewrap, and Landlock's.
    const REFUSED: [&str; 3] = [
        ": Operation not permitted",
        ": Read-only file system",
        ": Permission denied",
    ];
    let mut out: Vec<String> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        let Some(said) = REFUSED
            .iter()
            .find_map(|refused| line.strip_suffix(refused))
        else {
            continue;
        };
        let path = [
            "error: unable to create file ",
            "error: unable to unlink old ",
            "error: unable to create symlink ",
            "fatal: cannot create directory at ",
        ]
        .iter()
        .find_map(|opening| said.strip_prefix(opening));
        let Some(path) = path else {
            continue;
        };
        let path = path.trim_matches('\'').to_string();
        if !path.is_empty() && !out.contains(&path) && out.len() < MAX_ASKED_PATHS {
            out.push(path);
        }
    }
    out
}

/// The folder a `git clone` said it was cloning into, in `text`.
fn cloned_into(text: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let named = line.trim().strip_prefix("Cloning into '")?;
        let folder = named.strip_suffix("'...")?;
        (!folder.is_empty()).then(|| folder.to_string())
    })
}

/// **What a chat is told after a git command its sandbox stopped partway** (qw349): the net
/// under the guard, for a move whose commit could not be named before it ran. Read from the
/// command's own words: git names each file it could not write. `None` unless the line names
/// git and one of those files is a later-code name or a manifest's.
///
/// The words are the command's, and go back only to the chat that ran it.
pub fn stopped(cmd: &str, text: &str) -> Option<String> {
    if !shellwrap::may_name(cmd, "git") {
        return None;
    }
    let by_name = |path: &String| {
        let lower = path.to_lowercase();
        let manifest = Path::new(&lower)
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| sandbox::MANIFESTS.contains(&name));
        manifest
            || crate::sandboxblock::is_planted(Path::new(&lower))
            || lower.split('/').any(|part| part == ".claude")
    };
    let denied: Vec<String> = unwritten(text).into_iter().filter(by_name).collect();
    if denied.is_empty() {
        return None;
    }
    let named: Vec<&str> = denied
        .iter()
        .filter(|path| repeatable(path))
        .map(String::as_str)
        .collect();
    let those = if named.len() == denied.len() {
        named.join(" ")
    } else {
        "<those files>".to_string()
    };
    if text.contains("Clone succeeded, but checkout failed") {
        let folder = cloned_into(text)
            .filter(|folder| repeatable(folder))
            .unwrap_or_else(|| "<the clone's folder>".to_string());
        return Some(format!(
            "⬢ git stopped this clone partway: this chat's sandbox does not let it write {} \
             (files a program run later loads code from). The folder holds the repository and \
             its commit, but not all of its files, and its index is empty. Do not commit from \
             it as it is: every file it lacks would be recorded as deleted. To finish it, ask \
             the person to run this in their own terminal: `git -C {folder} restore \
             --source=HEAD --staged --worktree -- :/`. Run from this chat, the same command \
             writes every file but those. For a repository in this project's inventory, \
             `purlis clone <repo>` has the app make the clone whole instead.",
            listed(&denied)
        ));
    }
    Some(format!(
        "⬢ git stopped partway: this chat's sandbox does not let it write {} (files a program \
         run later loads code from). The folder is now half-updated: the other files are as \
         the other commit has them, and those are as they were. After a `checkout` or a \
         `switch` git moved to the new commit all the same, and shows those files as changed; \
         after a `pull`, `merge`, `rebase` or `reset` the commit did not move. Do not commit, \
         save or `git add -A` from it as it is. Where the commit did not move, `git restore \
         --source=HEAD --worktree -- :/` puts the folder back, and files only the other \
         commit has are left untracked (`git status` lists them). Where it did move, ask the \
         person to run this in their own terminal, in that folder: `git restore --source=HEAD \
         --worktree -- {those}`.",
        listed(&denied)
    ))
}

#[cfg(test)]
mod tests;
