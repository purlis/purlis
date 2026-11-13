// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The Bash guard's arm for `git add` and `git commit` in a branch folder purlis made, from a
//! sandboxed chat (#3830): refused before it runs, with the command that commits there.
//!
//! A branch folder is a linked worktree, and git keeps its index in the clone's git directory,
//! under `.git/worktrees/<name>`. Every sandboxed chat's sandbox denies writing there, at any
//! depth ([`crate::sandbox::PLANTED`]), so `git add` and `git commit` in a branch folder stop
//! on `Operation not permitted`, and the block raised a sandbox Notice for the person, though
//! the fix is the chat's own: `purlis worktree commit`, which the app runs outside the sandbox
//! on the folder's branch (#1476). **When an agent can fix its own call, it is refused with the
//! shape to retry, and never stops on the person** (the operator's ruling of 2026-10-29).
//!
//! **A mistake guard, not the boundary**: the sandbox refuses the write either way. So it
//! speaks only where it is sure:
//!
//! - the chat was given a sandbox (the caller asks);
//! - the one repository git will use, read as [`crate::projectgit`] reads it, is the record of a
//!   branch folder **purlis made**: git's own record of the folder (its `gitdir` file) names a
//!   folder at `workspaces/<ws>/.worktrees/<repo>/<piece>` in the project. A worktree the person
//!   made with `git worktree add` anywhere else is left to the sandbox, since `purlis worktree
//!   commit` would not commit there (the review of #3841);
//! - and the line can be placed: a `cd` to a literal folder is followed only on a line whose
//!   commands all run in one shell, one after another (`&&`, `;`). A `cd` can fail, so after a
//!   `;` or a new line the git is judged in the folder the `cd` left as well as the one it
//!   names; only `&&` from the `cd` to the git says the `cd` was made. A `cd` in a subshell, a
//!   pipeline or a substitution, or any move it cannot place (an expansion, `cd -`, `popd`,
//!   `pushd +1`, a `cd` that `CDPATH` may send elsewhere), leaves the line to the sandbox. A
//!   wrapper's own move to a literal folder (`env -C <folder> git …`, #3931) places that one
//!   command; one it cannot place leaves that command to the sandbox.
//!
//! **A chat with no sandbox has no sandbox to leave a line to.** Its guard holds every git
//! that may write a branch folder (its branch, HEAD, index or working tree) to the folder's one
//! live writer ([`branch_write`]), and reads the same line
//! the same way, but where it cannot place a git that may write a branch (after a move it
//! cannot follow, past [`MOST_PLACES`], as an alias it cannot read, behind a wrapper that may
//! change where git runs or what it reads, in a script handed to a shell, or with words it
//! cannot see) it says so ([`BranchWrites::unplaced`]), and the guard refuses that git with
//! the shape that places it. git's own commands that write no branch are let through wherever
//! they run. For it a bare relative `cd` is a move it cannot follow where its shell may have a
//! `CDPATH` ([`shell_may_have_cdpath`]).

use std::path::{Component, Path, PathBuf};

use crate::shellwrap::{self, base_lower};
use crate::{projectgit, shellseg};

/// The trace reason this arm is tallied under.
pub const REASON: &str = "branch-folder-git";

/// The folder, inside a workspace, that holds the branch folders purlis makes
/// (`workspaces/<ws>/.worktrees/<repo>/<piece>`), as [`crate::active::workspace_of_tree`]
/// reads it.
const WORKTREES: &str = ".worktrees";

/// The most folders [`found`] keeps the shell possibly in, after `cd`s that may each have
/// failed. Each `cd` before a `;` can add one, so past this many the shell's folder is taken as
/// unknown rather than followed, and no move joins more than this many paths. A chain joined by
/// `&&` alone adds none: whatever runs after it, runs where it went.
const MOST_PLACES: usize = 8;

/// git's own commands, as `git --list-cmds=main` lists them (git 2.50): git runs its own
/// command by such a name whatever an alias of it says, so an unsandboxed chat's guard reads no
/// alias for them ([`alias_of`]).
#[rustfmt::skip]
const GITS_OWN: &[&str] = &[
    "add", "am", "annotate", "apply", "archive", "backfill", "bisect", "blame", "branch",
    "bugreport", "bundle", "cat-file", "check-attr", "check-ignore", "check-mailmap",
    "check-ref-format", "checkout", "checkout-index", "cherry", "cherry-pick", "clean", "clone",
    "column", "commit", "commit-graph", "commit-tree", "config", "count-objects", "credential",
    "credential-cache", "credential-osxkeychain", "credential-store", "daemon", "describe",
    "diagnose", "diff", "diff-files", "diff-index", "diff-pairs", "diff-tree", "difftool",
    "fast-export", "fast-import", "fetch", "fetch-pack", "filter-branch", "fmt-merge-msg",
    "for-each-ref", "for-each-repo", "format-patch", "fsck", "fsck-objects", "gc",
    "get-tar-commit-id", "grep", "hash-object", "help", "hook", "http-backend", "http-fetch",
    "http-push", "imap-send", "index-pack", "init", "init-db", "interpret-trailers", "log",
    "ls-files", "ls-remote", "ls-tree", "mailinfo", "mailsplit", "maintenance", "merge",
    "merge-base", "merge-file", "merge-index", "merge-octopus", "merge-one-file", "merge-ours",
    "merge-recursive", "merge-recursive-ours", "merge-recursive-theirs", "merge-resolve",
    "merge-subtree", "merge-tree", "mergetool", "mktag", "mktree", "multi-pack-index", "mv",
    "name-rev", "notes", "p4", "pack-objects", "pack-redundant", "pack-refs", "patch-id", "pickaxe",
    "prune", "prune-packed", "pull", "push", "quiltimport", "range-diff", "read-tree", "rebase",
    "receive-pack", "reflog", "refs", "remote", "remote-ext", "remote-fd", "remote-ftp",
    "remote-ftps", "remote-http", "remote-https", "repack", "replace", "replay", "request-pull",
    "rerere", "reset", "restore", "rev-list", "rev-parse", "revert", "rm", "send-email",
    "send-pack", "shell", "shortlog", "show", "show-branch", "show-index", "show-ref",
    "sparse-checkout", "stage", "stash", "status", "stripspace", "submodule", "subtree", "switch",
    "symbolic-ref", "tag", "unpack-file", "unpack-objects", "update-index", "update-ref",
    "update-server-info", "upload-archive", "upload-pack", "var", "verify-commit", "verify-pack",
    "verify-tag", "version", "whatchanged", "worktree", "write-tree",
];

/// What [`found`] does at a git it cannot place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Unplaced {
    /// Stop reading the line: what follows is left to the sandbox.
    Stop,
    /// Read on, and name the first such git: there is no sandbox to leave it to.
    Name,
}

/// Every git on a line that writes a branch folder's branch ([`branch_write`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchWrites {
    /// Each one placed, in the line's order.
    pub found: Vec<InAFolder<crate::piececlaim::Act>>,
    /// The subcommand of the first git that may write a branch where the line could not be
    /// followed to the folder it runs in: after a move that cannot be placed, past
    /// [`MOST_PLACES`], or an alias whose command cannot be read. `None` where there is none.
    pub unplaced: Option<String>,
}

/// What the chat is told, for `git <sub>`: the shape to retry, in a call of its own.
pub fn said(sub: &str) -> String {
    format!(
        "`git {sub}` in a branch folder writes the folder's git record in its clone's .git \
         (`.git/worktrees/<name>`), which this chat's sandbox does not let it write, so it \
         would stop on Operation not permitted. Commit through the app, from inside the \
         folder and in a call of its own: `purlis worktree commit -m \"<message>\" <path>…`, \
         naming the files to commit, or `--all` in place of the paths for every change to a \
         file git already tracks. The app makes the commit on the folder's branch, outside the \
         sandbox, and asks nobody. Read-only git (`git status`, `git diff`, `git log`) works \
         here."
    )
}

/// What an unsandboxed chat is told of `git <sub>` on a line that cannot be followed to the
/// folder it runs in ([`BranchWrites::unplaced`]): the shape that places it.
pub fn unplaced_said(sub: &str) -> String {
    format!(
        "purlis cannot tell which folder `git {sub}` on this line runs in, so it cannot check \
         that no other live chat is working in that branch folder. Run it in a call of its own, \
         from the folder or after a `cd` to the folder's whole path joined by `&&` (`cd \
         <folder> && git {sub} …`), as the git command itself rather than an alias, a wrapper \
         or a script handed to a shell."
    )
}

/// The refusal for `cmd`, run in `cwd` by a chat the app gave a sandbox, in the project at
/// `root`: `(subcommand, sentence)`, or `None`.
pub fn refusal(cmd: &str, cwd: &str, root: &str) -> Option<(String, String)> {
    let cdpath = || std::env::var_os("CDPATH").is_some_and(|it| !it.is_empty());
    let first = found(cmd, cwd, root, Unplaced::Stop, &cdpath, |sub, post| {
        writes_the_index(sub, post).then_some(())
    })
    .0
    .into_iter()
    .next()?;
    let why = said(&first.sub);
    Some((first.sub, why))
}

/// One `git` on a line, in a branch folder purlis made: what [`found`] reads a line into.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InAFolder<T> {
    /// The git subcommand, as the line spells it.
    pub sub: String,
    /// What the reader asking made of it: for [`branch_write`], what it does to the branch.
    pub what: T,
    /// The branch folder's top.
    pub folder: PathBuf,
}

/// **Plain git that may write a branch folder** (#3931): every git on the line `cmd`, run in
/// `cwd`, that may move a branch or HEAD, or write the index or the working tree, of a branch
/// folder purlis made in the project at `root`, in the line's order, read and placed as
/// [`refusal`] reads and places a line. What an unsandboxed chat's guard holds to each folder's
/// one live writer ([`crate::piececlaim`]). A `git commit`, `merge`, `cherry-pick`, `revert` or
/// `am` is a commit, a `push` a push, and every other git an update, but for the git that only
/// reads ([`reads_only`]): an allow-list, so a subcommand it does not know counts as a write. A
/// dry run writes nothing and is let through.
///
/// Its folder is the repository git will use, from `-C`, `--git-dir` and `GIT_DIR` as git
/// reads them, and the working tree it is told to write (`--work-tree`, `GIT_WORK_TREE`, or the
/// folder it runs in where only a git directory is given).
///
/// A git that may write but cannot be placed is named apart ([`BranchWrites::unplaced`]): this
/// reader has no sandbox to leave it to. That includes a `-C`, `--git-dir`, `--work-tree`,
/// `GIT_DIR` or `GIT_WORK_TREE` the line does not spell out. An alias is read from the
/// configuration of each folder it may run in; one that runs a shell command, or that the line
/// itself or git's environment defines, cannot be.
pub fn branch_write(cmd: &str, cwd: &str, root: &str) -> BranchWrites {
    branch_write_where(cmd, cwd, root, &|| {
        shell_may_have_cdpath(&|name| std::env::var_os(name), Path::new("/etc"))
    })
}

/// [`branch_write`], with `cdpath` saying whether the chat's shell may have a `CDPATH`: asked
/// only of a bare relative `cd`, and once a line.
fn branch_write_where(cmd: &str, cwd: &str, root: &str, cdpath: &dyn Fn() -> bool) -> BranchWrites {
    let (found, unplaced) = found(cmd, cwd, root, Unplaced::Name, cdpath, writes_the_branch);
    BranchWrites { found, unplaced }
}

/// The start files a chat's shell may read before it runs a line, each in a folder of
/// [`StartFolder`]: zsh's, which it reads from `ZDOTDIR` where that is set, and the
/// Bourne shells' and bash's. An interactive shell's files are among them: a login shell's
/// profile often reads them.
const START_FILES: [(StartFolder, &str); 21] = [
    (StartFolder::Home, ".zshenv"),
    (StartFolder::Zdot, ".zshenv"),
    (StartFolder::Zdot, ".zprofile"),
    (StartFolder::Zdot, ".zshrc"),
    (StartFolder::Zdot, ".zlogin"),
    (StartFolder::Etc, "zshenv"),
    (StartFolder::Etc, "zprofile"),
    (StartFolder::Etc, "zshrc"),
    (StartFolder::Etc, "zlogin"),
    (StartFolder::Etc, "zsh/zshenv"),
    (StartFolder::Etc, "zsh/zprofile"),
    (StartFolder::Etc, "zsh/zshrc"),
    (StartFolder::Etc, "zsh/zlogin"),
    (StartFolder::Home, ".bash_profile"),
    (StartFolder::Home, ".bash_login"),
    (StartFolder::Home, ".profile"),
    (StartFolder::Home, ".bashrc"),
    (StartFolder::Etc, "profile"),
    (StartFolder::Etc, "bashrc"),
    (StartFolder::Etc, "bash.bashrc"),
    (StartFolder::Etc, "environment"),
];

/// Where one of [`START_FILES`] is.
#[derive(Debug, Clone, Copy)]
enum StartFolder {
    /// The home.
    Home,
    /// `ZDOTDIR`, or the home where it is not set.
    Zdot,
    /// The system's folder of settings.
    Etc,
}

/// The variables that name a file a shell reads before the line it runs: bash's, for a shell
/// that runs a script, and a POSIX shell's, for an interactive one.
const START_FILE_VARS: [&str; 2] = ["BASH_ENV", "ENV"];

/// The most of one start file read for a `CDPATH`.
const START_FILE_READ: u64 = 1 << 20;

/// **Whether a chat's shell may have a `CDPATH`**, along which a relative `cd` is looked for
/// first, **or `cdable_vars`**, with which a relative `cd` may go to a variable's folder: one
/// in its environment (`var`; `BASHOPTS` for bash's option), or a start file one of its shells
/// reads ([`START_FILES`] in the home, `ZDOTDIR` and `etc`, and the files [`START_FILE_VARS`]
/// name) that names either anywhere, in any case ([`may_move_a_cd`]). A start file that is
/// there but cannot be read, or that reads another, may set one too. Each file read is a small
/// one, and only for a line with a bare relative `cd`.
fn shell_may_have_cdpath(var: &dyn Fn(&str) -> Option<std::ffi::OsString>, etc: &Path) -> bool {
    if var("CDPATH").is_some_and(|it| !it.is_empty()) {
        return true;
    }
    if var("BASHOPTS").is_some_and(|it| it.to_string_lossy().contains("cdable_vars")) {
        return true;
    }
    let set = |name: &str| var(name).filter(|it| !it.is_empty()).map(PathBuf::from);
    let home = set("HOME");
    let zdot = set("ZDOTDIR").or_else(|| home.clone());
    let files = START_FILES
        .iter()
        .filter_map(|(folder, name)| {
            let folder = match folder {
                StartFolder::Home => home.as_deref()?,
                StartFolder::Zdot => zdot.as_deref()?,
                StartFolder::Etc => etc,
            };
            Some(folder.join(name))
        })
        .chain(START_FILE_VARS.iter().filter_map(|name| set(name)));
    files.into_iter().any(|file| {
        // A start file is often a link into a folder of dotfiles: it is read where it leads.
        let Ok(real) = std::fs::canonicalize(&file) else {
            return false;
        };
        crate::sandbox::arrived::read_regular(&real, START_FILE_READ)
            .is_none_or(|text| may_move_a_cd(&text))
    })
}

/// Whether a start file's `text` may change where a relative `cd` goes: it names `cdpath`, or
/// `cdable_vars` as zsh spells it too (any case, `_` anywhere), anywhere; or it reads another
/// file (`source`, or `.` as a command), which is not followed and may set either.
fn may_move_a_cd(text: &[u8]) -> bool {
    let folded: Vec<u8> = text
        .iter()
        .filter(|b| **b != b'_')
        .map(u8::to_ascii_lowercase)
        .collect();
    let names = |word: &[u8]| folded.windows(word.len()).any(|it| it == word);
    if names(b"cdpath") || names(b"cdablevars") || names(b"source") {
        return true;
    }
    // `.` as a command: at the start of a command, followed by a blank.
    text.windows(3).any(|it| {
        matches!(
            it[0],
            b' ' | b'\t' | b'\n' | b';' | b'&' | b'|' | b'(' | b'{'
        ) && it[1] == b'.'
            && matches!(it[2], b' ' | b'\t')
    }) || text.starts_with(b". ")
        || text.starts_with(b".\t")
}

/// Whether the project at `root` has a branch folder purlis made, in any workspace: where it
/// has none, no git can write one.
pub fn any_branch_folder(root: &Path) -> bool {
    let Ok(workspaces) = std::fs::read_dir(root.join("workspaces")) else {
        return false;
    };
    workspaces.flatten().any(|ws| {
        std::fs::read_dir(ws.path().join(WORKTREES)).is_ok_and(|repos| {
            repos.flatten().any(|repo| {
                std::fs::read_dir(repo.path()).is_ok_and(|mut pieces| pieces.next().is_some())
            })
        })
    })
}

/// What `git <sub> <post…>` does to the branch folder it runs in, as a branch folder's claim
/// names it ([`branch_write`]), or `None` for git that only reads: one of [`reads_only`].
///
/// A commit and its kin are a commit, a rebase or a pull an update, and a push a push. Every
/// other git counts as an update, git's own or not: one that may move a branch or HEAD, or
/// write the index or the working tree, and one this does not know. A dry run, and what only
/// shows or leaves a sequence under way (`--quit`, `--show-current-patch`), writes nothing; an
/// `--abort` puts HEAD, the index and the tree back, so it writes.
fn writes_the_branch(sub: &str, post: &[String]) -> Option<crate::piececlaim::Act> {
    use crate::piececlaim::Act;
    let has = |flag: &str| post.iter().any(|arg| arg == flag);
    if reads_only(sub, post) {
        return None;
    }
    let only_shows = has("--quit") || has("--show-current-patch");
    match sub {
        "commit" if has("--dry-run") => None,
        "merge" | "cherry-pick" | "revert" | "am" | "rebase" if only_shows => None,
        "commit" | "merge" | "cherry-pick" | "revert" | "am" if !has("--abort") => {
            Some(Act::Commit)
        }
        "pull" if has("--dry-run") => None,
        "push" if has("-n") || has("--dry-run") => None,
        "push" => Some(Act::Push),
        _ => Some(Act::Update),
    }
}

/// git's own commands that never move a branch or HEAD, nor write the index or the working
/// tree, however they are called. Some write objects, which change no branch.
#[rustfmt::skip]
const ONLY_READ: &[&str] = &[
    "annotate", "archive", "blame", "bugreport", "cat-file", "check-attr", "check-ignore",
    "check-mailmap", "check-ref-format", "cherry", "column", "commit-tree", "count-objects",
    "describe", "diagnose", "diff", "diff-files", "diff-index", "diff-pairs", "diff-tree",
    "fmt-merge-msg", "for-each-ref", "format-patch", "fsck", "fsck-objects", "get-tar-commit-id",
    "grep", "hash-object", "help", "log", "ls-files", "ls-remote", "ls-tree", "mailinfo",
    "mailsplit", "merge-base", "merge-tree", "mktag", "mktree", "name-rev", "patch-id",
    "range-diff", "rev-list", "rev-parse", "shortlog", "show", "show-branch", "show-index",
    "show-ref", "status", "stripspace", "var", "verify-commit", "verify-pack", "verify-tag",
    "version", "whatchanged", "write-tree",
];

/// Whether `git <sub> <post…>` only reads: one of [`ONLY_READ`], or the reading forms of a
/// command that may also write (`branch` listing, `config --get`, `stash list`, a `fetch` that
/// names no local ref to update). Anything else may write, a subcommand this does not know
/// included. Each word is looked at a bounded number of times.
fn reads_only(sub: &str, post: &[String]) -> bool {
    if ONLY_READ.contains(&sub) {
        return true;
    }
    let has = |flags: &[&str]| {
        post.iter().any(|arg| {
            flags.iter().any(|flag| {
                arg == flag
                    || (flag.starts_with("--")
                        && arg
                            .strip_prefix(flag)
                            .is_some_and(|rest| rest.starts_with('=')))
            })
        })
    };
    let operands = || post.iter().filter(|arg| !arg.starts_with('-'));
    let first = || operands().next().map(String::as_str);
    match sub {
        // A local ref updated only by a refspec that names one (`<src>:<dst>`).
        "fetch" => !operands().any(|arg| arg.contains(':')),
        "branch" => {
            !has(&[
                "-d",
                "-D",
                "-m",
                "-M",
                "-c",
                "-C",
                "-f",
                "-u",
                "--delete",
                "--move",
                "--copy",
                "--force",
                "--set-upstream-to",
                "--unset-upstream",
                "--edit-description",
                "--create-reflog",
                "--track",
                "--no-track",
            ]) && (first().is_none()
                || has(&[
                    "-l",
                    "--list",
                    "--contains",
                    "--no-contains",
                    "--merged",
                    "--no-merged",
                    "--points-at",
                    "--format",
                    "--sort",
                ]))
        }
        "tag" => {
            !has(&[
                "-d",
                "-a",
                "-s",
                "-u",
                "-f",
                "-m",
                "-F",
                "--delete",
                "--annotate",
                "--sign",
                "--local-user",
                "--force",
                "--message",
                "--file",
                "--edit",
                "-e",
            ]) && (first().is_none()
                || has(&[
                    "-l",
                    "--list",
                    "-n",
                    "--contains",
                    "--no-contains",
                    "--merged",
                    "--no-merged",
                    "--points-at",
                    "-v",
                    "--verify",
                ]))
        }
        "config" => config_reads(post),
        "symbolic-ref" => !has(&["-d", "--delete"]) && operands().nth(1).is_none(),
        "reflog" => !matches!(first(), Some("expire" | "delete" | "drop" | "write")),
        "remote" => post.is_empty() || matches!(first(), None | Some("show" | "get-url")),
        "stash" => matches!(first(), Some("list" | "show")),
        "worktree" => first() == Some("list"),
        "notes" => matches!(first(), Some("list" | "show")),
        "submodule" => matches!(first(), None | Some("status" | "summary")),
        "sparse-checkout" => first() == Some("list"),
        "bisect" => matches!(first(), Some("log" | "view" | "visualize")),
        "rerere" => matches!(first(), Some("status" | "diff" | "remaining")),
        "bundle" => matches!(first(), Some("verify" | "list-heads")),
        "apply" => {
            has(&["--check", "--stat", "--numstat", "--summary"]) && !has(&["--apply", "--index"])
        }
        _ => false,
    }
}

/// Whether `git config <post…>` only reads: a `--get…` or `--list`, the `get` and `list`
/// commands, or a key alone, and nothing that sets, unsets, renames or edits.
fn config_reads(post: &[String]) -> bool {
    const SETS: &[&str] = &[
        "--add",
        "--replace-all",
        "--unset",
        "--unset-all",
        "--rename-section",
        "--remove-section",
        "-e",
        "--edit",
    ];
    const GETS: &[&str] = &[
        "--get",
        "--get-all",
        "--get-regexp",
        "--get-urlmatch",
        "--get-color",
        "--get-colorbool",
        "-l",
        "--list",
    ];
    // The options that take the next word as their value.
    const VALUED: &[&str] = &["-f", "--file", "--blob", "--type", "--default", "--comment"];
    let mut operands = 0usize;
    let mut first: Option<&str> = None;
    let mut words = post.iter();
    let mut gets = false;
    while let Some(word) = words.next() {
        let word = word.as_str();
        if SETS.contains(&word) {
            return false;
        }
        if GETS.contains(&word) {
            gets = true;
        } else if VALUED.contains(&word) {
            words.next();
        } else if !word.starts_with('-') {
            operands += 1;
            first.get_or_insert(word);
        }
    }
    if gets {
        return true;
    }
    match first {
        Some("get" | "list") => true,
        Some("set" | "unset" | "rename-section" | "remove-section" | "edit") => false,
        _ => operands <= 1,
    }
}

/// Every `git` on the line `cmd`, run in `cwd`, that `wanted` names a `T` for and whose
/// repository is the record of a branch folder purlis made in the project at `root`, in the
/// line's order. A wrapper's own move (`env -C <folder> git …`) places that one command where
/// it is literal.
///
/// Where the line makes a move that cannot be placed, `unplaced` says what follows: with
/// [`Unplaced::Stop`], reading stops there and what it found before that move is kept; with
/// [`Unplaced::Name`], it reads on and names the first git after it that `wanted` names a `T`
/// for, or that may be an alias, which then runs in a folder it does not know. With
/// [`Unplaced::Name`], an alias is read too ([`alias_of`]), and one it cannot read is named so.
fn found<T: Clone>(
    cmd: &str,
    cwd: &str,
    root: &str,
    unplaced: Unplaced,
    cdpath: &dyn Fn() -> bool,
    wanted: impl Fn(&str, &[String]) -> Option<T>,
) -> (Vec<InAFolder<T>>, Option<String>) {
    let mut seen = Vec::new();
    let mut lost: Option<String> = None;
    if !shellwrap::prefilter_text(cmd).contains("git") {
        return (seen, lost);
    }
    // A heredoc body no program that runs text is handed is data, and only the substitutions
    // of one the shell expands run (#6293).
    let cmd = &crate::heredoc::commands_a_shell_runs(cmd);
    let Some(segments) = shellseg::joined_argv(cmd) else {
        // A line that does not tokenize cannot be followed, and a git in it may write.
        if unplaced == Unplaced::Name {
            lost = unreadable_may_write(cmd, &wanted);
        }
        return (seen, lost);
    };
    // A `cd` carries over to the next command only where both run in this shell, in turn.
    let in_turn = |op: &Option<String>| {
        op.as_deref()
            .is_none_or(|op| matches!(op, "&&" | ";" | "\n"))
    };
    let one_shell = segments
        .iter()
        .all(|segment| in_turn(&segment.before) && in_turn(&segment.after));
    let argvs: Vec<Vec<String>> = segments.iter().map(|s| s.argv.clone()).collect();
    // Whether the line's own words were read for a shell fed on its standard input: once is
    // enough, however many such shells the line has.
    let mut line_read = false;
    let exported = shellwrap::exported_env(&argvs);
    // A relative `cd` is looked for along `CDPATH` first, where the shell has one: one the
    // line sets, or one `cdpath` says the shell may have. Asked once, and only of a bare `cd`.
    let cdpath_here = std::cell::OnceCell::new();
    let cdpath_here = || *cdpath_here.get_or_init(cdpath);
    let sets_cdpath = |env: &[String]| {
        env.iter().any(|pair| {
            pair.strip_prefix("CDPATH=")
                .is_some_and(|it| !it.is_empty())
        })
    };
    // Every folder the shell may be in: a `cd` that fails leaves it where it was, and only
    // `&&` stops what follows. So a `cd` adds the folder it names, and the folders it left come
    // back at the first `;` or new line after it. Empty once it cannot be known.
    let mut here = vec![PathBuf::from(cwd)];
    let mut if_a_move_failed: Vec<PathBuf> = Vec::new();
    // More folders a failed move may have left than [`MOST_PLACES`]: they are no longer kept,
    // and from the first `;` or new line after them the shell's folder is not known.
    let mut too_many_if_a_move_failed = false;
    let add = |places: &mut Vec<PathBuf>, place: PathBuf| {
        if !places.contains(&place) {
            places.push(place);
        }
    };
    // Where the shell is can no longer be told: stop, or read on knowing no folder.
    macro_rules! out_of_sight {
        () => {{
            if unplaced == Unplaced::Stop {
                return (seen, lost);
            }
            here.clear();
            if_a_move_failed.clear();
            too_many_if_a_move_failed = false;
        }};
    }
    for (segment, before) in segments.iter().zip(exported.iter()) {
        if segment.before.as_deref().is_some_and(|op| op != "&&") {
            if too_many_if_a_move_failed {
                out_of_sight!();
            }
            for left in if_a_move_failed.drain(..) {
                add(&mut here, left);
            }
            // More places than a line of a few moves has: out of sight.
            if here.len() > MOST_PLACES {
                out_of_sight!();
            }
        }
        let call = shellwrap::split_env_chdir(&as_run(&segment.argv));
        let program = base_lower(&call.prog);
        match program.as_str() {
            // A move to a literal folder, on a line run in one shell, is followed; anything
            // else is out of sight.
            "cd" | "pushd" => match call.argv.get(1) {
                // A `cd` behind `builtin` or `command` is left unplaced like any wrapped move,
                // and one with more than one operand (zsh's `cd old new` rewrites the folder
                // the shell is in) goes nowhere its words name.
                Some(to)
                    if one_shell
                        && call.argv.len() == 2
                        && call.layers.is_empty()
                        && !here.is_empty()
                        && !projectgit::unplaceable(to)
                        && to != "-"
                        && !to.starts_with(['-', '+'])
                        && (beside_or_absolute(to)
                            || !(sets_cdpath(before)
                                || sets_cdpath(&call.env)
                                || cdpath_here())) =>
                {
                    let mut moved = Vec::new();
                    for from in &here {
                        add(&mut moved, from.join(to));
                    }
                    for left in std::mem::replace(&mut here, moved) {
                        if !too_many_if_a_move_failed {
                            add(&mut if_a_move_failed, left);
                        }
                    }
                    if if_a_move_failed.len() > MOST_PLACES {
                        too_many_if_a_move_failed = true;
                        if_a_move_failed.clear();
                    }
                    continue;
                }
                _ => {
                    out_of_sight!();
                    continue;
                }
            },
            "popd" => {
                out_of_sight!();
                continue;
            }
            _ => {}
        }
        if unplaced == Unplaced::Name && program != "git" {
            // git handed to a shell as a string, or fed to one on its standard input, runs
            // where and how that shell says: one that may write a branch is named.
            if lost.is_none()
                && let Some(sub) = in_a_nested_shell(&call.argv, &argvs, &mut line_read, &wanted, 0)
            {
                lost = Some(sub);
            }
            continue;
        }
        if program != "git" {
            continue;
        }
        let (pre, rest) = shellwrap::git_globals(&call.argv);
        // A wrapper that adds words of its own (`xargs git`) runs git with words the line does
        // not show.
        let words_added = adds_words(&call);
        if unplaced == Unplaced::Name && words_added {
            if lost.is_none() {
                lost = may_write_unseen(&call.argv, &wanted, true);
            }
            continue;
        }
        let Some((sub, post)) = rest.split_first() else {
            continue;
        };
        let mut env = before.clone();
        env.extend(call.env.iter().cloned());
        let direct = wanted(sub, post);
        // git runs its own command by its name whatever an alias says; any other name is an
        // alias where one is set, and otherwise a program of its own.
        let may_be_an_alias = unplaced == Unplaced::Name && !GITS_OWN.contains(&sub.as_str());
        // An alias the line defines or includes itself, in any case, or one read from
        // configuration the line moves, is read from no configuration this can see.
        let defined_here = config_given(&pre) || env.iter().any(|pair| config_moved(pair));
        // A wrapper that may change where git runs or what it reads: what it does to git is not
        // read here.
        let wrapped = unplaced == Unplaced::Name && !read_through(&call);
        let placeable = !here.is_empty()
            && !wrapped
            && (call.chdir.is_empty() || !projectgit::unplaceable(&call.chdir))
            && !points_out_of_sight(&pre, &env);
        if !placeable || (may_be_an_alias && defined_here) {
            if unplaced == Unplaced::Name && (direct.is_some() || may_be_an_alias) && lost.is_none()
            {
                lost = Some(sub.clone());
            }
            continue;
        }
        // Every branch folder the git may run in, each once. `env -C <folder>` moves this one
        // command, never the shell.
        let mut folders: Vec<(PathBuf, T)> = Vec::new();
        let mut placed: Vec<PathBuf> = Vec::new();
        for from in &here {
            let at = if call.chdir.is_empty() {
                from.clone()
            } else {
                from.join(&call.chdir)
            };
            let repository = projectgit::repository(&at, &pre, &env)
                .and_then(|repo| purlis_made(&repo, Path::new(root)));
            // A git with no sandbox behind it writes the tree it is given, too, whichever
            // repository it uses.
            let tree = (unplaced == Unplaced::Name)
                .then(|| work_tree(&at, &pre, &env))
                .flatten()
                .and_then(|tree| projectgit::repository(&tree, &[], &[]))
                .and_then(|repo| purlis_made(&repo, Path::new(root)));
            for folder in repository.into_iter().chain(tree) {
                if !placed.contains(&folder) {
                    placed.push(folder);
                }
            }
        }
        for folder in placed {
            let what = match &direct {
                _ if may_be_an_alias => match alias_of(&folder, sub) {
                    Alias::None => direct.clone(),
                    Alias::Runs(words) => {
                        // Read as git reads the words it runs: a global option among them, or
                        // another alias, is not read further.
                        let argv: Vec<String> =
                            std::iter::once("git".to_owned()).chain(words).collect();
                        let (globals, runs) = shellwrap::git_globals(&argv);
                        let read = match runs.split_first() {
                            Some((then, more))
                                if globals.is_empty() && GITS_OWN.contains(&then.as_str()) =>
                            {
                                let post: Vec<String> = more.iter().chain(post).cloned().collect();
                                Ok(wanted(then, &post))
                            }
                            _ => Err(()),
                        };
                        read.unwrap_or_else(|()| {
                            if lost.is_none() {
                                lost = Some(sub.clone());
                            }
                            None
                        })
                    }
                    Alias::Unread => {
                        if lost.is_none() {
                            lost = Some(sub.clone());
                        }
                        None
                    }
                },
                Some(what) => Some(what.clone()),
                None => None,
            };
            if let Some(what) = what {
                folders.push((folder, what));
            }
        }
        for (folder, what) in folders {
            seen.push(InAFolder {
                sub: sub.clone(),
                what,
                folder,
            });
        }
    }
    (seen, lost)
}

/// Whether git's global options `pre` or the variables `env` (`NAME=value`) point it at a place
/// the line does not show: a `-C`, `--git-dir` or `--work-tree`, or a `GIT_DIR` or
/// `GIT_WORK_TREE`, that is a shell expansion, or an option whose value the line leaves out.
fn points_out_of_sight(pre: &[String], env: &[String]) -> bool {
    let mut words = pre.iter();
    while let Some(word) = words.next() {
        let value = match word.as_str() {
            "-C" | "--git-dir" | "--work-tree" => match words.next() {
                Some(value) => value,
                None => return true,
            },
            other => match other
                .strip_prefix("--git-dir=")
                .or_else(|| other.strip_prefix("--work-tree="))
            {
                Some(value) => value,
                None => {
                    if shellwrap::GIT_VALUE_OPTS.contains(&other) {
                        words.next();
                    }
                    continue;
                }
            },
        };
        if projectgit::unplaceable(value) {
            return true;
        }
    }
    env.iter().any(|pair| {
        pair.strip_prefix("GIT_DIR=")
            .or_else(|| pair.strip_prefix("GIT_WORK_TREE="))
            .is_some_and(projectgit::unplaceable)
    })
}

/// The working tree a git run in `cwd` with the global options `pre` and the variables `env`
/// is told to write, where it is told one: the last `--work-tree`, else `GIT_WORK_TREE`, else,
/// where a git directory is given (`--git-dir`, `GIT_DIR`), the folder it runs in, which git
/// then takes for its tree. Relative to where the last `-C` leaves it. `None` where it is
/// told none: it then writes the tree of the repository it finds.
fn work_tree(cwd: &Path, pre: &[String], env: &[String]) -> Option<PathBuf> {
    let last_set = |name: &str| {
        env.iter()
            .rev()
            .find_map(|pair| pair.strip_prefix(name))
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
    };
    let mut tree = last_set("GIT_WORK_TREE=");
    let mut git_dir = last_set("GIT_DIR=").is_some();
    let mut here = cwd.to_path_buf();
    let mut words = pre.iter();
    while let Some(word) = words.next() {
        match word.as_str() {
            "-C" => here = here.join(words.next()?),
            "--work-tree" => tree = Some(words.next()?.clone()),
            "--git-dir" => {
                words.next();
                git_dir = true;
            }
            other if other.starts_with("--git-dir=") => git_dir = true,
            other => {
                if let Some(value) = other.strip_prefix("--work-tree=") {
                    tree = Some(value.to_owned());
                } else if shellwrap::GIT_VALUE_OPTS.contains(&other) {
                    words.next();
                }
            }
        }
    }
    match tree {
        Some(tree) => Some(here.join(tree)),
        None => git_dir.then_some(here),
    }
}

/// What an unplaced git is named by where its subcommand cannot be read.
const SOME_SUBCOMMAND: &str = "<subcommand>";

/// The wrappers that add words of their own to the command they run.
const ADDS_WORDS: &[&str] = &["xargs", "parallel"];

/// How deeply [`in_a_nested_shell`] reads a shell's script inside another's.
const MOST_NESTED: usize = 4;

/// The subcommand of the first git among `argvs`, each read through its wrappers, or
/// [`SOME_SUBCOMMAND`].
fn first_git_sub(argvs: &[Vec<String>]) -> String {
    argvs
        .iter()
        .map(|argv| shellwrap::split_env_chdir(&as_run(argv)).argv)
        .filter(|argv| argv.first().is_some_and(|prog| base_lower(prog) == "git"))
        .find_map(|argv| shellwrap::git_globals(&argv).1.into_iter().next())
        .unwrap_or_else(|| SOME_SUBCOMMAND.to_owned())
}

/// The words of a command as it runs them: each redirection the shell takes out, wherever it
/// stands (its operator, the word it takes, and a descriptor's number in front of it). A
/// here-string is kept: it is a script a shell may be fed.
fn as_run(argv: &[String]) -> Vec<String> {
    let mut words: Vec<String> = Vec::with_capacity(argv.len());
    let mut it = argv.iter();
    while let Some(word) = it.next() {
        if word != "<<<" && shellwrap::is_redirect_token(word) {
            if words
                .last()
                .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
            {
                words.pop();
            }
            it.next();
        } else {
            words.push(word.clone());
        }
    }
    words
}

/// The subcommand of `git …` (`argv`) that may write a branch where not all of its words are in
/// sight: one `wanted` names, one that is not git's own and so may be an alias, or, where
/// `words_unseen` says words may be added to it, none at all ([`SOME_SUBCOMMAND`]), or one
/// `wanted` names with none of its flags: a flag added out of sight may undo one in sight that
/// keeps it from writing (`--dry-run`), so only what writes with no flag at all is asked of
/// `wanted`. `None` for git's own commands that `wanted` does not name.
fn may_write_unseen<T>(
    argv: &[String],
    wanted: &impl Fn(&str, &[String]) -> Option<T>,
    words_unseen: bool,
) -> Option<String> {
    let (_, rest) = shellwrap::git_globals(argv);
    match rest.split_first() {
        None => words_unseen.then(|| SOME_SUBCOMMAND.to_owned()),
        Some((sub, post)) => (wanted(sub, post).is_some()
            || (words_unseen && wanted(sub, &[]).is_some())
            || !GITS_OWN.contains(&sub.as_str()))
        .then(|| sub.clone()),
    }
}

/// What may write a branch on the line `cmd`, which does not tokenize, read as words: the first
/// git on it [`may_write_unseen`] names, or the first git's subcommand where a word that
/// mentions git is not git itself (a shell's script, say). `None` where every git on it is
/// git's own and writes no branch.
fn unreadable_may_write<T>(
    cmd: &str,
    wanted: &impl Fn(&str, &[String]) -> Option<T>,
) -> Option<String> {
    shellseg::segment_argv(cmd).iter().find_map(|words| {
        if !shellwrap::prefilter_text(&words.join(" ")).contains("git") {
            return None;
        }
        let call = shellwrap::split_env_chdir(&as_run(words));
        if base_lower(&call.prog) != "git"
            || call.argv[1..]
                .iter()
                .any(|word| shellwrap::prefilter_text(word).contains("git"))
        {
            return Some(first_git_sub(std::slice::from_ref(words)));
        }
        may_write_unseen(&call.argv, wanted, true)
    })
}

/// The subcommand of a git that may write a branch, or may be an alias, in what the command
/// `argv` (its wrappers taken off) hands a shell to run: `eval`'s words, a shell's `-c` script
/// (any shell's, fish's `--command` too), the string a runner hands one (`watch`, `flock -c`,
/// `script -c`, `parallel`), a here-string, or, for a shell that reads its standard input, any
/// word of the line (`argvs`) that holds one, read once a line (`line_read`). `None` where there
/// is none, or `argv` is no such command.
fn in_a_nested_shell<T>(
    argv: &[String],
    argvs: &[Vec<String>],
    line_read: &mut bool,
    wanted: &impl Fn(&str, &[String]) -> Option<T>,
    depth: usize,
) -> Option<String> {
    let program = argv.first()?;
    let mut scripts = shellwrap::shell_scripts_of(program, argv);
    if scripts.is_empty() {
        if let Some(script) = shellwrap::here_string_script_of(program, argv) {
            scripts.push(script);
        } else if shellwrap::takes_its_script_from_stdin(program, argv) {
            if std::mem::replace(line_read, true) {
                return None;
            }
            // Its standard input: what the line feeds it is in the line's own words.
            scripts = argvs.iter().flatten().cloned().collect();
        } else {
            // A script file, or no shell: as any program, not read.
            return None;
        }
    }
    let scripts: Vec<String> = scripts
        .into_iter()
        .filter(|word| shellwrap::prefilter_text(word).contains("git"))
        .collect();
    if scripts.is_empty() {
        return None;
    }
    if depth >= MOST_NESTED {
        return Some(SOME_SUBCOMMAND.to_owned());
    }
    scripts.iter().find_map(|script| {
        let inner = shellseg::segment_argv(script);
        let mut inner_read = false;
        inner.iter().find_map(|words| {
            let call = shellwrap::split_env_chdir(&as_run(words));
            let first = call.argv.first()?;
            if base_lower(first) != "git" {
                return in_a_nested_shell(&call.argv, &inner, &mut inner_read, wanted, depth + 1);
            }
            // `git` alone runs its help, unless a wrapper adds the words it runs.
            may_write_unseen(&call.argv, wanted, adds_words(&call))
        })
    })
}

/// Whether a wrapper in front of the git `call` runs adds words of its own (`xargs git`).
fn adds_words(call: &shellwrap::Invocation) -> bool {
    call.layers
        .iter()
        .any(|at| ADDS_WORDS.contains(&base_lower(&call.taken[*at]).as_str()))
}

/// The wrappers that run git in the folder the line is in, with the environment and the
/// configuration it would have run with: they time it, bound it, renice it, keep the machine
/// awake for it or buffer its output, whatever their options say.
const READ_THROUGH: [&str; 8] = [
    "time",
    "command",
    "nohup",
    "timeout",
    "gtimeout",
    "nice",
    "caffeinate",
    "stdbuf",
];

/// Whether the git `call` runs is reached through no wrapper but [`READ_THROUGH`]'s, and `env`
/// only setting variables and moving to a folder ([`shellwrap::split_env_chdir`] reads both).
fn read_through(call: &shellwrap::Invocation) -> bool {
    let mut starts = call.layers.clone();
    starts.push(call.taken.len());
    starts.windows(2).all(|two| {
        let layer = &call.taken[two[0]..two[1]];
        let Some((wrapper, words)) = layer.split_first() else {
            return true;
        };
        let wrapper = base_lower(wrapper);
        if READ_THROUGH.contains(&wrapper.as_str()) {
            return true;
        }
        if wrapper != "env" {
            return false;
        }
        let mut words = words.iter();
        while let Some(word) = words.next() {
            match word.as_str() {
                "-C" | "--chdir" => {
                    words.next();
                }
                moved if moved.starts_with("--chdir=") => {}
                moved if moved.starts_with("-C") && moved.len() > 2 => {}
                set if !set.starts_with('-') && set.contains('=') => {}
                _ => return false,
            }
        }
        true
    })
}

/// Whether git's global options `pre` give it configuration that may define an alias: `-c`
/// with an `alias.` or `include` key in any case (git's keys are), or `--config-env`.
fn config_given(pre: &[String]) -> bool {
    pre.iter()
        .any(|word| word == "--config-env" || word.starts_with("--config-env="))
        || pre.windows(2).any(|two| {
            let key = two[1].split('=').next().unwrap_or("").to_ascii_lowercase();
            two[0] == "-c" && (key.starts_with("alias.") || key.starts_with("include"))
        })
}

/// Whether the variable `pair` (`NAME=value`) moves where git reads its configuration: `HOME`,
/// `XDG_CONFIG_HOME`, or any of git's own `GIT_CONFIG…`.
fn config_moved(pair: &str) -> bool {
    pair.starts_with("HOME=")
        || pair.starts_with("XDG_CONFIG_HOME=")
        || pair.starts_with("GIT_CONFIG")
}

/// Whether a `cd` to `to` goes where it says whatever `CDPATH` holds: an absolute folder, or one
/// named from the shell's own (`.`, `..`, `./…`, `../…`), which the shell never looks for along
/// `CDPATH`.
fn beside_or_absolute(to: &str) -> bool {
    Path::new(to).is_absolute()
        || matches!(to, "." | "..")
        || to.starts_with("./")
        || to.starts_with("../")
}

/// What a git subcommand stands for in a branch folder, as an alias.
enum Alias {
    /// No alias of that name.
    None,
    /// The git command it stands for, as words.
    Runs(Vec<String>),
    /// One that runs a shell command, or whose configuration could not be read.
    Unread,
}

/// The alias `sub` is in the branch folder `folder`, as the chat's git there reads it: with the
/// configuration its environment names too (`GIT_CONFIG_GLOBAL`, `GIT_CONFIG_COUNT` and its
/// pairs). A subcommand that is not an alias's name as git spells one (letters, digits and
/// `-`) is an expansion or a pattern the shell has yet to turn into one: unread.
fn alias_of(folder: &Path, sub: &str) -> Alias {
    use crate::worktree::git;
    if sub.is_empty() || !sub.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
        return Alias::Unread;
    }
    let key = format!("alias.{sub}");
    let Ok(run) = git::run_as_session(folder, &["config", "--get", &key], git::READ) else {
        return Alias::Unread;
    };
    match run.code {
        // git's own answer for a key that is not set.
        Some(1) => Alias::None,
        Some(0) => {
            let said = String::from_utf8_lossy(&run.out);
            let words = said.lines().next().unwrap_or_default().trim();
            if words.starts_with('!') {
                return Alias::Unread;
            }
            match shellseg::joined_argv(words) {
                Some(segments) if segments.len() == 1 => Alias::Runs(segments[0].argv.clone()),
                _ => Alias::Unread,
            }
        }
        _ => Alias::Unread,
    }
}

/// Whether `git <sub> <post…>` writes the index: `add` and `commit`, but for their dry runs
/// (`add -n`, `add --dry-run`, `commit --dry-run`; a commit's `-n` is `--no-verify`, which
/// writes).
fn writes_the_index(sub: &str, post: &[String]) -> bool {
    let has = |flag: &str| post.iter().any(|arg| arg == flag);
    match sub {
        "add" => !(has("-n") || has("--dry-run")),
        "commit" => !has("--dry-run"),
        _ => false,
    }
}

/// Whether the git directory `repo` is the record of a branch folder purlis made in the
/// project at `root`: a linked worktree's record (`.git/worktrees/<name>`), whose `gitdir`
/// file (git's own pointer back to the folder's `.git`) names a folder at
/// `workspaces/<ws>/.worktrees/<repo>/<piece>`: that folder, or `None`.
fn purlis_made(repo: &Path, root: &Path) -> Option<PathBuf> {
    if !a_worktree_record(repo) {
        return None;
    }
    let pointer = std::fs::read_to_string(repo.join("gitdir")).ok()?;
    let folder = std::fs::canonicalize(Path::new(pointer.trim()).parent()?).ok()?;
    let workspaces = std::fs::canonicalize(root.join("workspaces")).ok()?;
    let within = folder.strip_prefix(&workspaces).ok()?;
    let parts: Vec<Component<'_>> = within.components().collect();
    matches!(
        parts.as_slice(),
        [Component::Normal(_), Component::Normal(worktrees), Component::Normal(_), Component::Normal(_)]
            if *worktrees == WORKTREES
    )
    .then_some(folder)
}

/// Whether the git directory `repo` is a linked worktree's record in its clone's git directory
/// (`.git/worktrees/<name>`).
fn a_worktree_record(repo: &Path) -> bool {
    let parts: Vec<&std::ffi::OsStr> = repo
        .components()
        .filter_map(|part| match part {
            Component::Normal(name) => Some(name),
            _ => None,
        })
        .collect();
    parts
        .windows(3)
        .any(|three| three[0] == ".git" && three[1] == "worktrees")
}

#[cfg(test)]
mod tests;
