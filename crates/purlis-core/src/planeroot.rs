// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Arms A3 and A3b: the plane root is one shared working tree — refuse a branch move in it
//! (#157), and refuse a `git reset` there that would destroy commits no remote has (#401).
//!
//! A port of `charter/hooks.py`'s `_plane_root_branch_reason`, `_plane_root_reset_reason` and
//! everything they close over that no earlier stage shipped: `_plane_root_git` (the walk both
//! share), `_git_target` (the directories one invocation acts on), `_checkout_opt_kind`,
//! `_checkout_operand_kind`, `_created_branch`, `_inline_aliases`, `_resolve_git_alias`,
//! `_unpushed_at_risk`, `doctor._plane_default_branch`, and their tables. The line is read by
//! [`crate::shellseg`], each segment's program and environment by [`crate::shellwrap`] (whose
//! `exported_env` answers "what did an earlier segment export", #496), and a repository's own
//! `core.worktree` by [`crate::gitconfig`].
//!
//! # Where this is called from
//!
//! [`crate::toolgate`] — `charter/hooks.py:pretooluse`'s eight refusals, in its order — and
//! through it `charter hook pretooluse` (M3.1 stage 6).
//!
//! Until that stage the switch was closed: `main.rs`'s `is_a_tool_hook` answered every word in
//! the `pretooluse`/`posttooluse` namespace with exit 2 — *block* — because a PARTIAL guard on
//! the switch turns fail-closed into allow-everything-except-the-arm-that-is-ported. It moved
//! once all eight arms were standing, and never earlier. The other eight words in that
//! namespace still block, for the same reason they always did.
//! # These guards ask git, and how they ask is part of the answer
//!
//! Four questions are put to a real git: does an operand resolve to a commit, does git track it
//! as a path, what is an alias's body, and how many commits would a reset take off the branch
//! that no upstream has. Python asks through `doctor._git_in`, which inherits the hook's whole
//! environment; this asks through [`crate::worktree::git::run_as_session`], the hardened runner
//! (a constructed environment, no hooks, no fsmonitor, git found in a fixed list of directories)
//! plus the variables that say where git's CONFIG is. The second half matters: an alias in a
//! relocated global config is exactly what the command about to run will expand, and a guard
//! whose git could not see it would stand aside while the root's branch moved.
//!
//! The deadline is Python's, five seconds (`doctor.CHECK_TIMEOUT`), and what a question that
//! could not be asked answers is Python's per call site — which is not one rule:
//!
//! * an operand that could not be classified is `unknown`, and that **keeps the refusal**;
//! * an alias that could not be read is "not an alias", which **stands aside** — charter has no
//!   reason yet to believe the command is a branch move;
//! * a reset whose unpushed count could not be read is **allowed** — the guard only speaks when
//!   it has measured that something would be lost, the same silence `doctor` keeps;
//! * a default branch that could not be read is "none", which withholds the remedy's carve-out.
//!
//! "Could not be asked" is also Python's: the process could not start, the deadline passed, an
//! argument held a NUL, or git wrote bytes that are not UTF-8 — Python decodes a child's output
//! strictly and raises. [`crate::worktree::git::RawRun`] keeps the bytes so that last case is
//! told apart from a U+FFFD git really wrote.
//!
//! # Where the Python raises and this answers
//!
//! The Python raises out of these guards on three inputs, and a raise out of `pretooluse` is an
//! ALLOW (charter#1166, and charter#1178 for these sites): a symlink loop among an invocation's
//! subjects on CPython 3.11/3.12, a NUL in `-C`/`--work-tree`/`--git-dir`, and a default branch
//! whose name is not UTF-8. This answers all three — the loop is resolved as CPython 3.14 does,
//! the NUL is a path that is not the root, and the undecodable default is no default — and the
//! differential keeps those inputs out of its alphabet, because it cannot arbitrate an input one
//! side has no answer for. Unit tests below pin what this does there.
//!
//! # Where this parts from the frozen Python, on purpose
//!
//! The Python oracle recognised git and the root by SPELLING and modelled every `cd` as
//! succeeding (charter#1176, charter#1177). Both were fail-opens, fixed here and recorded as
//! changed corpus rows (ADR 0046):
//!
//! * **#346** — `git` is recognised folded (`GIT` runs git on APFS and NTFS), inline aliases are
//!   keyed folded as git keys them, and a subject is compared to the root by identity and by the
//!   repository git would discover from it ([`TheRoot`]), not as a resolved string.
//! * **#345** — a `cd` only moves the later segments for certain when its failure would stop them
//!   (the shell's own `cd`, joined by `&&`); any other `cd` adds a directory the shell may be in
//!   and keeps the one it was in ([`Whereabouts`]).
//! * **#1323** — a directory git is sent to (`-C`, `--git-dir`, `--work-tree`, `GIT_DIR`,
//!   `GIT_WORK_TREE`, a wrapper's chdir flag) is read the way the shell hands it over: `~` is
//!   `$HOME`, and a word the guard cannot name (`$VAR`, a glob, `~user`) may be the root
//!   ([`invocation_readings`]), refused with a sentence of its own. The Python joined the word
//!   to the cwd as written. And a subject that IS the root's git directory reaches the root,
//!   wherever that directory lives.

//! * **qw346** — `git checkout HEAD` (or `@`) with no path after a `--` on the line is refused as
//!   before, in a sentence of its own that names the restore that cannot move HEAD. The verdict
//!   is the Python's; the words are not.

use crate::gitconfig;
use crate::guardcaps;
use crate::memstore::py_strip;
use crate::pypath::{is_abs, normpath, path_div, pure_path, realpath};
use crate::shellseg;
use crate::shellwrap::{self, GIT_VALUE_OPTS};
use crate::worktree::git as runner;

mod lineenv;
use lineenv::Chain;
pub use lineenv::LineEnv;

/// git subcommands that move HEAD from one branch to another — `_BRANCH_MOVERS`.
///
/// Deliberately short: the evidence in #157 is about SWITCHING. `reset` has its own guard with
/// its own subject and remedy, because "HEAD moved between branches" and "commits were destroyed"
/// are two findings and only one sentence can be the denial.
pub const BRANCH_MOVERS: [&str; 2] = ["checkout", "switch"];

/// Options that make a `checkout`/`switch` CREATE a branch — `_BRANCH_CREATOR_OPTS`. The operand
/// of one of these is a name to create, never a path to restore. `--orphan` was the round-one
/// bypass: `git checkout --orphan README` read as a restore of `README`.
pub const BRANCH_CREATOR_OPTS: [&str; 7] = [
    "-b",
    "-B",
    "-c",
    "-C",
    "--orphan",
    "--create",
    "--force-create",
];

/// Options that take HEAD off its branch — `_DETACH_OPTS`. `--detach` needs no operand at all.
pub const DETACH_OPTS: [&str; 2] = ["--detach", "-d"];

/// Options a RESTORE accepts, and that cannot move HEAD — `_RESTORE_OPTS`.
///
/// An ALLOWLIST, and the direction is the point: "is every option here one I can show a restore
/// accepts?" keeps the guard shut for an option git adds next year, where "is it one of the four
/// I know move HEAD?" opened it for `--orphan`, `--track` and `--guess` the day it was written.
pub const RESTORE_OPTS: [&str; 19] = [
    "--ours",
    "--theirs",
    "--force",
    "--merge",
    "--patch",
    "--quiet",
    "--progress",
    "--conflict",
    "--overlay",
    "--ignore-skip-worktree-bits",
    "--pathspec-from-file",
    "--pathspec-file-nul",
    "--recurse-submodules",
    "--overwrite-ignore",
    "--ignore-other-worktrees",
    "--source",
    "--staged",
    "--worktree",
    "--ignore-unmerged",
];

/// The same list in git's short spelling, as LETTERS — `_RESTORE_SHORTS`. `-fq` is one token and
/// `-bREADME` is one token; `-2`/`-3` are `--ours`/`--theirs`.
pub const RESTORE_SHORTS: &str = "fmpq23";

/// How many trailing operands of a `git checkout <tree-ish> <paths…>` are resolved before the
/// guard stops and keeps refusing — `_MAX_CHECKOUT_OPERANDS`. Two local git questions each, on
/// the `PreToolUse` path, so the work an agent can ask for here is bounded.
pub const MAX_CHECKOUT_OPERANDS: usize = 32;

/// `git reset` modes that overwrite the WORKING TREE as they move HEAD — `_RESET_TREE_MODES`.
/// `--soft` and `--mixed` leave every byte on disk, but take commits off the branch all the
/// same, so [`plane_root_reset_reason`] judges every mode and only the denial's words differ.
pub const RESET_TREE_MODES: [&str; 3] = ["--hard", "--merge", "--keep"];

/// The environment spellings of the two global options that NAME A REPOSITORY — `_GIT_DIR_ENV`.
/// `GIT_DIR=<plane>/.git git checkout feature` moves the root's HEAD from anywhere.
pub const GIT_DIR_ENV: [(&str, &str); 2] = [("GIT_DIR", "git_dir"), ("GIT_WORK_TREE", "work_tree")];

/// Subcommands taken to be git's own, so they are not asked about as aliases —
/// `_GIT_KNOWN_SUBCOMMANDS`.
///
/// **A cost list, never a safety list**: a name missing from it costs one `git config --get`.
/// Everything that MOVES HEAD is deliberately absent, so the resolution still runs for
/// `checkout` and `switch`.
pub const GIT_KNOWN_SUBCOMMANDS: [&str; 67] = [
    "add",
    "am",
    "annotate",
    "apply",
    "archive",
    "bisect",
    "blame",
    "branch",
    "bundle",
    "cat-file",
    "check-ignore",
    "cherry",
    "cherry-pick",
    "clean",
    "clone",
    "commit",
    "config",
    "describe",
    "diff",
    "difftool",
    "fetch",
    "for-each-ref",
    "format-patch",
    "fsck",
    "gc",
    "grep",
    "help",
    "init",
    "log",
    "ls-files",
    "ls-remote",
    "ls-tree",
    "merge",
    "merge-base",
    "mergetool",
    "mv",
    "notes",
    "pull",
    "push",
    "range-diff",
    "rebase",
    "reflog",
    "remote",
    "repack",
    "replace",
    "rerere",
    "reset",
    "restore",
    "rev-list",
    "rev-parse",
    "revert",
    "rm",
    "shortlog",
    "show",
    "show-ref",
    "sparse-checkout",
    "stash",
    "status",
    "submodule",
    "symbolic-ref",
    "tag",
    "update-index",
    "update-ref",
    "verify-commit",
    "version",
    "whatchanged",
    "worktree",
];

/// How many alias hops are followed — `_MAX_ALIAS_HOPS`. git resolves an alias to an alias
/// (`ck = co`, `co = checkout` switches branches), so one hop is not enough and unbounded is a
/// loop.
pub const MAX_ALIAS_HOPS: usize = 4;

/// What one `checkout`/`switch` option does — `_checkout_opt_kind`'s four answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OptKind {
    Create,
    Detach,
    Restore,
    /// Anything this cannot place — treated as able to move HEAD, so the fail-closed default
    /// belongs to the option charter has never seen.
    Unknown,
}

impl OptKind {
    /// The Python's string for it.
    pub fn as_str(self) -> &'static str {
        match self {
            OptKind::Create => "create",
            OptKind::Detach => "detach",
            OptKind::Restore => "restore",
            OptKind::Unknown => "unknown",
        }
    }
}

/// Classify one option of a `git checkout`/`git switch` — `_checkout_opt_kind`.
///
/// git takes an option's value ATTACHED as readily as separated (`-bREADME`, `--orphan=README`)
/// and clusters short options (`-fq`), so this normalises to the option NAME before answering. A
/// short cluster is read left to right and stops at the first letter that decides: `-b` swallows
/// the rest of the token as the new branch's name. `--no-x` is normalised to `--x` and stays on
/// the same side of the fence.
pub fn checkout_opt_kind(tok: &str) -> OptKind {
    if tok.starts_with("--") {
        let mut name = tok.split('=').next().unwrap_or(tok).to_string();
        if let Some(rest) = name.strip_prefix("--no-") {
            name = format!("--{rest}");
        }
        if BRANCH_CREATOR_OPTS.contains(&name.as_str()) {
            return OptKind::Create;
        }
        if DETACH_OPTS.contains(&name.as_str()) {
            return OptKind::Detach;
        }
        return if RESTORE_OPTS.contains(&name.as_str()) {
            OptKind::Restore
        } else {
            OptKind::Unknown
        };
    }
    for ch in tok.chars().skip(1) {
        let letter = format!("-{ch}");
        if BRANCH_CREATOR_OPTS.contains(&letter.as_str()) {
            return OptKind::Create;
        }
        if DETACH_OPTS.contains(&letter.as_str()) {
            return OptKind::Detach;
        }
        if !RESTORE_SHORTS.contains(ch) {
            return OptKind::Unknown;
        }
    }
    OptKind::Restore
}

/// Every directory a git invocation's GLOBAL options aim it at — `_git_target`.
///
/// Exactly: the cwd, always (after any `-C`s, each joined onto the last, as git does); the
/// `--work-tree`/`GIT_WORK_TREE` if one is named; the `--git-dir`/`GIT_DIR` and its `..` if one
/// is named (the `..` is appended rather than taken lexically, so the caller's `realpath`
/// collapses `.git/refs/..` the way the filesystem does); and the `core.worktree` the repository
/// this invocation is aimed at writes in its own config (#504).
///
/// **It only ever GROWS**: a subject list that got smaller as the command line got longer would
/// be a flag-shaped bypass by construction, so the cwd is kept even when `--work-tree` is named
/// (git still discovers the REFS from the cwd).
///
/// `pre` is `git_globals`' first half — git's own options ONLY — and `env` is the invocation's
/// environment, earlier exports first: an option on the line overrides the environment.
///
/// Every path is the string `str(Path(...))` would be.
pub fn git_target(cwd: &str, pre: &[String], env: &[String]) -> Vec<String> {
    git_target_with(cwd, pre, env, &gitconfig::configured_work_tree)
}

/// [`git_target`], with the repository's own `core.worktree` asked through `configured`: the
/// walk asks the disk once for each directory on a line ([`Disk`]), not once for each git.
fn git_target_with(
    cwd: &str,
    pre: &[String],
    env: &[String],
    configured: &dyn Fn(&str, Option<&str>) -> Option<String>,
) -> Vec<String> {
    let mut here = pure_path(if cwd.is_empty() { "." } else { cwd });
    let mut git_dir: Option<String> = None;
    let mut work_tree: Option<String> = None;
    for assign in env {
        let (name, val) = assign.split_once('=').unwrap_or((assign.as_str(), ""));
        match GIT_DIR_ENV
            .iter()
            .find(|(k, _)| *k == name)
            .map(|(_, w)| *w)
        {
            Some("git_dir") => git_dir = Some(val.to_string()),
            Some("work_tree") => work_tree = Some(val.to_string()),
            _ => {}
        }
    }
    let mut i = 0usize;
    while i < pre.len() {
        let tok = pre[i].as_str();
        let (name, attached) = match tok.split_once('=') {
            Some((n, a)) => (n, Some(a)),
            None => (tok, None),
        };
        // Only the separated `-C`: git rejects the attached one outright.
        if tok == "-C" && i + 1 < pre.len() {
            let val = pre[i + 1].as_str();
            if !val.is_empty() {
                here = path_div(&here, val); // `-C ""` leaves the directory unchanged
            }
            i += 2;
            continue;
        }
        if name == "--git-dir" || name == "--work-tree" {
            let (val, step) = match attached {
                Some(a) => (a.to_string(), 1),
                None => (pre.get(i + 1).cloned().unwrap_or_default(), 2),
            };
            if !val.is_empty() {
                if name == "--git-dir" {
                    git_dir = Some(val);
                } else {
                    work_tree = Some(val);
                }
            }
            i += step;
            continue;
        }
        i += if GIT_VALUE_OPTS.contains(&tok) { 2 } else { 1 };
    }
    let at = |p: &str| path_div(&here, p);
    let mut out = vec![here.clone()];
    if let Some(wt) = &work_tree {
        out.push(at(wt));
    }
    let named = git_dir.as_deref().map(at);
    if let Some(gd) = &named {
        out.push(gd.clone());
        out.push(path_div(gd, ".."));
    }
    if let Some(configured) = configured(&here, named.as_deref()) {
        out.push(configured);
    }
    out
}

/// `Path(p).resolve()` as a string — CPython's non-strict `realpath` of the `Path`'s string.
fn resolved(p: &str) -> String {
    pure_path(&realpath(&pure_path(p)))
}

/// One git invocation that acts on the plane root: its subcommand as WRITTEN, the arguments
/// after it, and git's own options before it (one of which can define the subcommand: `git -c
/// alias.co=checkout co feature`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RootInvocation {
    pub sub: String,
    pub post: Vec<String>,
    pub pre: Vec<String>,
    /// It reaches the root only through a directory the guard cannot name before the shell runs
    /// (`cd "$X"`, `git -C "$X"`, a glob, `cd -`): it MAY act on the root, so it is refused, and
    /// the denial says that rather than claiming it does (#1323).
    pub unnamed: bool,
    /// Its options held a substitution, and which word after it git runs could not be worked
    /// out — git could not be asked about its aliases, or there were more alias words than the
    /// line's budget: it is refused (#1354, as #438 rules for an operand git could not be asked
    /// about).
    pub unread: bool,
    /// Its program is a word only the shell can name (`$G`, `"$GIT"`), so it may not be git at
    /// all: what an unread denial says it could not read (#1326).
    pub program_unnamed: bool,
    /// The environment it may run with as the line sets it — earlier bare assignments and
    /// exports, then its own assignments — which can define aliases (#1358). Shared with the
    /// line's other gits rather than copied into each (#2648).
    pub env: LineEnv,
    /// Its own assignments alone: what reaches git whatever an earlier segment's fate (`unset`,
    /// `env -u`, a subshell, `false &&`). An alias the two disagree on is unread ([`LineConfig`]),
    /// so a line value can only add refusals.
    pub own_env: Vec<String>,
    /// A shell `cd` earlier on the line was joined to what follows by `;`, `||`, `&` or a
    /// newline rather than `&&`, so the directory the shell was in stayed in play (#345). The
    /// denial says to join it with `&&` ([`LOOSE_CD_HINT`], #1326).
    pub after_loose_cd: bool,
    /// A command that may write comes before it on the line ([`Elsewhere::after_a_writer`]): an
    /// alias, what a word names, the commit HEAD is at and the commits no upstream has may all
    /// have changed by the time it runs, so the guards judge none of them from what they read
    /// before the line ran.
    pub after_a_writer: bool,
    /// An unquoted command substitution stands among git's own options: what it prints is
    /// split into words git reads as options and as its command. Refused, with the quoted
    /// spelling the guard reads as one word.
    pub unquoted_substitution: bool,
    /// The shell fills in its command word (`git $S …`, `git "$@"`), so a word after it that
    /// reads as a command may be an option or an operand of another: refused as unknown unless
    /// a more specific reason applies.
    pub command_filled: bool,
}

impl RootInvocation {
    /// The subcommand git runs and its words, its aliases followed
    /// ([`resolve_git_alias_to_its_end`]). After a command that may write, a word that is not
    /// git's own may be an alias that command defined: it is [`AliasEnd::Unread`].
    pub fn resolved(&self, root: &str) -> (String, Vec<String>, AliasEnd) {
        let own = |sub: &str| BRANCH_MOVERS.contains(&sub) || GIT_KNOWN_SUBCOMMANDS.contains(&sub);
        if self.after_a_writer && !own(&self.sub) {
            return (self.sub.clone(), self.post.clone(), AliasEnd::Unread);
        }
        resolve_git_alias_to_its_end(
            root,
            &self.sub,
            &self.post,
            &self.pre,
            &self.env,
            &self.own_env,
        )
    }
}

/// What a named denial adds when a `cd` earlier on the line was not joined by `&&`: agents met
/// that answer as "the guard ignores cd" (#1326).
pub const LOOSE_CD_HINT: &str = "A `cd` joined to the next command by `;`, `||` or a newline \
     leaves the shell where it was if it fails, and one sent to the background by `&` never \
     moves the shell at all, so purlis still counts the directory the line started in: join it \
     with `&&` (`cd <path> && git …`) and purlis reads where git runs. ";

/// The plane root as the walk recognises it: by IDENTITY, never by spelling (#346).
///
/// A re-cased path on a case-insensitive filesystem and macOS's `/System/Volumes/Data` firmlink
/// both reach the root while [`resolved`] spells them differently, so a subject is compared by
/// `(st_dev, st_ino)`, falling back to the resolved string only where it cannot be stat-ed. And
/// git does not act on the directory it runs in but on the repository it DISCOVERS upward from
/// it, so a subject inside the root's own repository — `docs/`, or a directory the same command
/// is about to make — reaches the root too, while a repository of its own inside it (a workspace
/// clone) stops the ascent and does not.
struct TheRoot {
    path: String,
    id: Option<(u64, u64)>,
    /// The root's own git directory and its identity, when the root has a `.git` of its own.
    git_dir: Option<(String, Option<(u64, u64)>)>,
}

impl TheRoot {
    fn of(root: &str) -> Self {
        let own = std::fs::symlink_metadata(path_div(root, ".git")).is_ok();
        let git_dir = own
            .then(|| gitconfig::git_dir_at(root))
            .flatten()
            .map(|gd| {
                let gd = resolved(&gd);
                let id = identity(&gd);
                (gd, id)
            });
        TheRoot {
            path: root.to_string(),
            id: identity(root),
            git_dir,
        }
    }

    /// Whether git, pointed at the subject `t`, acts on the root's repository.
    fn is_reached_from(&self, t: &str) -> bool {
        let t = resolved(t);
        if same_dir(&t, &self.path, self.id) {
            return true;
        }
        let Some((root_gd, root_gd_id)) = &self.git_dir else {
            return false;
        };
        // The root's git directory itself, wherever it lives: `--git-dir=<it>` moves the root's
        // HEAD as surely as standing in the root does.
        if same_dir(&t, root_gd, *root_gd_id) {
            return true;
        }
        gitconfig::git_dir_at(&t).is_some_and(|gd| same_dir(&resolved(&gd), root_gd, *root_gd_id))
    }
}

/// Whether the resolved `path` and `other` (whose identity is `other_id`) are one directory: by
/// identity where both can be stat-ed, by the resolved string where either cannot.
fn same_dir(path: &str, other: &str, other_id: Option<(u64, u64)>) -> bool {
    match (identity(path), other_id) {
        (Some(a), Some(b)) => a == b,
        _ => path == other,
    }
}

/// `str(Path(dir) / rel)`, with an empty `dir` read as the current directory.
fn join_dir(dir: &str, rel: &str) -> String {
    path_div(&pure_path(if dir.is_empty() { "." } else { dir }), rel)
}

/// [`crate::pypath::file_identity`] of a path string.
fn identity(path: &str) -> Option<(u64, u64)> {
    crate::pypath::file_identity(std::path::Path::new(path))
}

/// The most directories [`Whereabouts`] spells; past it the shell may be anywhere.
const MAX_WHEREABOUTS: usize = 64;

/// The most bytes of directory names [`Whereabouts`] spells: [`MAX_WHEREABOUTS`] names of 128
/// bytes. Every git on a line is read against each directory the shell may be in, so what is
/// spelt is what one git costs; a `cd` to a relative name makes each name longer, and a line
/// of them would otherwise make each git cost the line's length (qw367).
const MAX_SPELT_BYTES: usize = 8 * 1024;

/// Where the later segments of one command line may be running: every directory a `cd` so far
/// could have left the shell in, and whether one of them is a directory this cannot name
/// (`cd "$DIR"`, `cd -`, `popd`), which may be the root.
///
/// **It only ever grows while the shell might still be elsewhere**, as [`git_target`]'s list does
/// and for the same reason: a `cd` is only taken to REPLACE where the shell is when its failure
/// would stop what follows, and a `cd` that fails leaves the shell where it was (#345).
#[derive(Clone, Debug, Default)]
struct Whereabouts {
    dirs: Vec<String>,
    anywhere: bool,
    /// More directories were in play than are spelt ([`MAX_WHEREABOUTS`],
    /// [`MAX_SPELT_BYTES`]): none is spelt, the shell may be anywhere, and it stays so until a
    /// `cd` that cannot fail unnoticed names an absolute directory.
    lost: bool,
    /// The bytes of `dirs`.
    bytes: usize,
}

impl Whereabouts {
    fn at(dir: &str) -> Self {
        Whereabouts {
            dirs: vec![dir.to_string()],
            bytes: dir.len(),
            ..Whereabouts::default()
        }
    }

    /// A directory no word spells: the shell may be anywhere.
    fn unnamed() -> Self {
        Whereabouts {
            anywhere: true,
            ..Whereabouts::default()
        }
    }

    /// `other` taken in. Past [`MAX_WHEREABOUTS`] directories, or [`MAX_SPELT_BYTES`] of
    /// their names, the shell may be anywhere and none is spelt any more: each loose `cd` to
    /// a relative name may double them, each `cd <name> && …;` adds one a level deeper, and
    /// spelling every one would cost the line the power of its `cd`s (#1325, qw367). A git
    /// read there is one the guard cannot name, which every guard refuses as it would in the
    /// root. What a later segment makes by a relative name is then a name no directory word
    /// can be told apart from ([`Made::spelt`]).
    fn merge(&mut self, other: Whereabouts) {
        self.anywhere |= other.anywhere;
        if self.lost || other.lost {
            self.lose();
            return;
        }
        for d in other.dirs {
            if self.dirs.contains(&d) {
                continue;
            }
            if self.dirs.len() >= MAX_WHEREABOUTS || self.bytes + d.len() > MAX_SPELT_BYTES {
                self.lose();
                return;
            }
            self.bytes += d.len();
            self.dirs.push(d);
        }
    }

    /// More directories than are spelt: see [`Whereabouts::lost`].
    fn lose(&mut self) {
        *self = Whereabouts {
            anywhere: true,
            lost: true,
            ..Whereabouts::default()
        };
    }
}

/// Every path an earlier segment of the line makes or re-points before a later one runs: the
/// target of an `ln`, `mv` or `cp`, and a path a redirection writes (#1326). The guard reads
/// directories before the line runs, so `ln -s <root> foo && git -C foo …` would be read as a
/// `foo` that does not exist yet. A directory word that reaches one of these is one the guard
/// cannot name ([`Made::reaches`]). Read off the line, never the filesystem; each path is held
/// absolute, as every directory the shell may be in would spell it.
#[derive(Debug, Default)]
struct Made {
    paths: Vec<String>,
    /// More was made than is held ([`MAX_MADE`]), or a relative path was made while the shell
    /// may be in more directories than are spelt ([`MAX_MADE_DIRS`]) or in one no word spells:
    /// every directory word reaches it. Keeps the reading linear in the line, and fails closed.
    overflowed: bool,
}

/// The most made paths held for one line; past it every directory word is unnameable.
const MAX_MADE: usize = 64;

/// The most directories a relative made path is spelt from; past it every directory word is
/// unnameable.
const MAX_MADE_DIRS: usize = 32;

/// The programs whose last operand (or `-t` directory) is a path they make or re-point.
/// GNU coreutils' own names for them where they sit beside the system's (`gln`), included.
const PATH_MAKERS: [&str; 6] = ["ln", "mv", "cp", "gln", "gmv", "gcp"];

/// Programs that make paths, links among them, named by what they unpack or copy rather than
/// on the line (train 77 review): what they make cannot be spelt, so the guard cannot name any
/// directory after them on the line.
const UNSPELLABLE_MAKERS: [&str; 10] = [
    "tar", "gtar", "bsdtar", "unzip", "7z", "7za", "cpio", "rsync", "ditto", "pax",
];

/// The redirections that write the word after them.
const WRITING_REDIRECTIONS: [&str; 6] = [">", ">>", ">|", "<>", "&>", "&>>"];

/// What a file written inside a directory makes of it for git: a `.git` file or directory, or a
/// git directory's own `HEAD`, `commondir` or `gitdir`. Any other file inside it does not move
/// where git acts.
const GIT_CONTROL_NAMES: [&str; 4] = [".git", "HEAD", "commondir", "gitdir"];

impl Made {
    /// `word` as each directory in `here` spells it, absolute and normalised, or `None` when the
    /// shell may be in more directories than are spelt, or in one no word spells (after a `cd`
    /// the guard cannot name): a relative word there may name any directory.
    fn spelt(word: &str, here: &Whereabouts) -> Option<Vec<String>> {
        let word = word.trim_end_matches('/');
        if word.is_empty() {
            return Some(Vec::new());
        }
        if is_abs(word) {
            return Some(vec![normpath(word)]);
        }
        if here.anywhere || here.dirs.len() > MAX_MADE_DIRS {
            return None;
        }
        Some(
            here.dirs
                .iter()
                .map(|dir| normpath(&join_dir(dir, word)))
                .collect(),
        )
    }

    /// What the segment `argv` (program `prog`, its words `args`) makes, the shell being in
    /// `here`: taken in with [`Made::take`] once the segment is read, so it reaches only the
    /// segments after it.
    fn of(argv: &[String], prog: &str, args: &[String], here: &Whereabouts) -> Option<Vec<String>> {
        let mut words: Vec<&str> = Vec::new();
        for pair in argv.windows(2) {
            if WRITING_REDIRECTIONS.contains(&pair[0].as_str()) {
                words.push(&pair[1]);
            }
        }
        // `busybox ln …` is `ln …`: busybox is a wrapper [`shellwrap`] reads through (#1325).
        let name = shellwrap::base_lower(prog);
        if UNSPELLABLE_MAKERS.contains(&name.as_str()) {
            return None;
        }
        let made_basename;
        if PATH_MAKERS.contains(&name.as_str()) {
            let mut operands: Vec<&str> = Vec::new();
            let mut options_end = false;
            let mut k = 1;
            while k < args.len() {
                let w = args[k].as_str();
                if WRITING_REDIRECTIONS.contains(&w) {
                    k += 2;
                    continue;
                }
                if options_end || !w.starts_with('-') || w == "-" {
                    operands.push(w);
                } else if w == "--" {
                    options_end = true;
                } else if let Some(dir) = w.strip_prefix("--target-directory=") {
                    words.push(dir);
                } else if matches!(w, "-t" | "--target-directory" | "-S" | "--suffix") {
                    if matches!(w, "-t" | "--target-directory")
                        && let Some(dir) = args.get(k + 1)
                    {
                        words.push(dir);
                    }
                    k += 1;
                } else if !w.starts_with("--")
                    && let Some(at) = w.find('t')
                    && at + 1 < w.len()
                {
                    // `-tDIR`, or `-st DIR` read as above where it ends in `t`.
                    words.push(&w[at + 1..]);
                } else if !w.starts_with("--") && w.ends_with('t') {
                    if let Some(dir) = args.get(k + 1) {
                        words.push(dir);
                    }
                    k += 1;
                }
                k += 1;
            }
            match operands.as_slice() {
                [] => {}
                // `ln -s <target>` makes the target's own name where the shell is.
                [only] if name == "ln" => {
                    made_basename = shellwrap::basename(only.trim_end_matches('/')).to_owned();
                    words.push(&made_basename);
                }
                [.., last] => words.push(last),
            }
        }
        let mut out = Vec::new();
        for word in words {
            out.extend(Self::spelt(word, here)?);
        }
        Some(out)
    }

    /// Takes in what a segment made ([`Made::of`]; `None` for what could not be spelt).
    fn take(&mut self, made: Option<Vec<String>>) {
        let Some(made) = made else {
            self.overflowed = true;
            return;
        };
        for path in made {
            if self.overflowed || self.paths.len() >= MAX_MADE {
                self.overflowed = true;
                return;
            }
            if !self.paths.contains(&path) {
                self.paths.push(path);
            }
        }
    }

    /// Whether git sent to the directory `word`, the shell being in `here`, may act somewhere the
    /// line made or re-pointed: the word is a made path or inside one, or a made path is the
    /// word's own `.git` or one of its git directory's files ([`GIT_CONTROL_NAMES`]).
    fn reaches(&self, word: &str, here: &Whereabouts) -> bool {
        if self.overflowed {
            return true;
        }
        if self.paths.is_empty() {
            return false;
        }
        let Some(dirs) = Self::spelt(word, here) else {
            return true;
        };
        dirs.iter().any(|dir| {
            self.paths.iter().any(|made| {
                made == dir
                    || inside(made, dir).is_some()
                    || inside(dir, made).is_some_and(|rest| {
                        rest.split('/').next() == Some(".git") || GIT_CONTROL_NAMES.contains(&rest)
                    })
            })
        })
    }
}

/// What follows `outer` in `inner` when `inner` is a path inside it, or `None`.
fn inside<'a>(outer: &str, inner: &'a str) -> Option<&'a str> {
    inner.strip_prefix(outer)?.strip_prefix('/')
}

/// The shell's own directory-changing builtins, by the word the shell looks up: the lookup is
/// case-SENSITIVE (a builtin is found before `PATH`), so `CD` is not one of them — on a
/// case-insensitive filesystem it runs `/usr/bin/cd`, a program that cannot move the shell.
const CD_BUILTINS: [&str; 3] = ["cd", "pushd", "popd"];

/// One `cd`, `pushd` or `popd` in a segment.
struct CdCall {
    builtin: String,
    args: Vec<String>,
    /// CERTAINLY the shell's own builtin run in this shell: the bare word at the front, after
    /// assignments and an optional `builtin` or `command`. Anything else that names one (`if cd
    /// x`, `! cd x`, `time cd x`, `env cd x`, `/usr/bin/cd x`) may or may not move the shell, and
    /// a guard that cannot tell takes both directories.
    certain: bool,
}

/// The `cd`-shaped call in one segment, if it is one — see [`CdCall`].
fn cd_call(toks: &[String], prog: &str, args: &[String]) -> Option<CdCall> {
    let mut i = 0;
    while i < toks.len() && shellwrap::is_env_assignment(&toks[i]) {
        i += 1;
    }
    if matches!(toks.get(i).map(String::as_str), Some("builtin" | "command")) {
        i += 1;
    }
    if let Some(word) = toks.get(i)
        && CD_BUILTINS.contains(&word.as_str())
    {
        return Some(CdCall {
            builtin: word.clone(),
            args: toks[i + 1..].to_vec(),
            certain: true,
        });
    }
    let base = shellwrap::basename(prog);
    CD_BUILTINS.contains(&base).then(|| CdCall {
        builtin: base.to_string(),
        args: args.get(1..).unwrap_or(&[]).to_vec(),
        certain: false,
    })
}

/// What the whole command line does to the meaning of a `cd` in it — read once, from its text.
///
/// Each is a fail-closed reading of a substring: a line that merely mentions `HOME` loses the
/// `~` shortcut, which costs nothing but a refusal the denial's own remedy answers.
#[derive(Clone)]
struct CdContext {
    /// The line may redefine `cd` — a function (`cd() …`, `function cd`), an `alias`, or an
    /// `enable -n` — so no `cd` in it is certainly the builtin.
    redefined: bool,
    /// The line may set `HOME`, so `~` and a bare `cd` go where this cannot tell.
    sets_home: bool,
    /// `CDPATH` is in play — set on the line, or in the environment the shell inherits — so a
    /// relative name may resolve somewhere else entirely.
    cdpath: bool,
    /// The line quotes or escapes a `~` (`'~`, `"~`, `\~`, or a quote right after one), so a
    /// separated `~` word the reader hands over unquoted may be one the shell leaves alone: a
    /// directory named `~`, not `$HOME` (#1323). Read from the text, before anything runs, so a
    /// `~` the same line creates is covered too.
    quotes_tilde: bool,
}

impl CdContext {
    /// This context with the line's around it: a string a shell is handed inherits what its
    /// line does to a `cd`, so each reading is the more careful of the two.
    fn with(self, outer: Option<&CdContext>) -> Self {
        let Some(o) = outer else {
            return self;
        };
        CdContext {
            redefined: self.redefined || o.redefined,
            sets_home: self.sets_home || o.sets_home,
            cdpath: self.cdpath || o.cdpath,
            quotes_tilde: self.quotes_tilde || o.quotes_tilde,
        }
    }

    fn of(cmd: &str) -> Self {
        static REDEFINES: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
        let redefines = REDEFINES.get_or_init(|| {
            regex::Regex::new(
                r"(?:^|[^A-Za-z0-9_])(?:(?:cd|pushd|popd)\s*\(|function\s+(?:cd|pushd|popd)\b)|(?:^|[;&|({\n]|\bthen|\bdo)\s*(?:builtin\s+|command\s+)?(?:alias|enable)\b",
            )
            .expect("compiles")
        });
        CdContext {
            redefined: redefines.is_match(cmd),
            sets_home: cmd.contains("HOME"),
            cdpath: cmd.contains("CDPATH")
                || std::env::var_os("CDPATH").is_some_and(|v| !v.is_empty()),
            // `$'…'` too: ANSI-C quoting spells a `~` as `$'\x7e'` or `$'\176'`, which the
            // reader decodes and the raw text never shows.
            quotes_tilde: ["'~", "\"~", "\\~", "~'", "~\"", "~\\", "$'"]
                .iter()
                .any(|quoted| cmd.contains(quoted)),
        }
    }
}

/// Where one `cd`/`pushd`/`popd` sends the shell from `here`, read as the shell reads it.
///
/// A destination this cannot name is `anywhere`: `popd`, `cd -` (`$OLDPWD`), `pushd` with no
/// directory or a stack rotation, a word the shell will still expand (`$`, a glob, a brace), a
/// `~user`, a `~` when the line may set `HOME`, a second operand (zsh's `cd old new`), and — when
/// `CDPATH` is in play — a relative name `CDPATH` may resolve elsewhere. `pushd -n` moves nothing.
/// `~` and a bare `cd` are `$HOME`. A `..` is read both ways, because `cd` takes `a/link/..`
/// LOGICALLY (back in `a`) where [`resolved`] takes it physically; both readings are kept.
fn cd_destinations(
    builtin: &str,
    args: &[String],
    here: &Whereabouts,
    line: &CdContext,
) -> Whereabouts {
    let anywhere = Whereabouts::unnamed;
    if builtin == "popd" {
        return anywhere();
    }
    let mut operands = args.iter().filter(|a| *a == "-" || !a.starts_with('-'));
    let dest = operands.next();
    if operands.next().is_some() {
        return anywhere(); // zsh: `cd old new` substitutes in the current path
    }
    let no_change = args
        .iter()
        .any(|a| a.starts_with('-') && !a.starts_with("--") && a.contains('n'));
    if builtin == "pushd" && no_change {
        return here.clone(); // `pushd -n` edits the stack and stays put
    }
    let dest = match (builtin, dest) {
        (_, Some(d)) if d == "-" || d.starts_with('+') => return anywhere(),
        ("pushd", None) => return anywhere(),
        (_, None) => "~".to_string(),
        (_, Some(d)) if d.is_empty() => return here.clone(), // `cd ""` stays put
        (_, Some(d)) => d.clone(),
    };
    if dest.contains(['$', '*', '?', '[', '{', '`']) {
        return anywhere();
    }
    if dest == "~" || dest.starts_with("~/") {
        if line.sets_home {
            return anywhere();
        }
        let Some(home) = std::env::var("HOME").ok().filter(|h| is_abs(h)) else {
            return anywhere();
        };
        let mut out = cd_to(format!("{home}{}", &dest[1..]), here, line);
        // A quoted or escaped `~` stays a directory named `~`, and the reader has taken the
        // quoting off: on a line that shows one, both are where the shell may go (#1323).
        if line.quotes_tilde {
            out.merge(cd_to(dest, here, line));
        }
        return out;
    }
    if dest.starts_with('~') {
        return anywhere();
    }
    cd_to(dest, here, line)
}

/// Where a `cd` to the named `dest` sends the shell from `here` — [`cd_destinations`] once the
/// word is read: `CDPATH`, and both readings of a `..`.
fn cd_to(dest: String, here: &Whereabouts, line: &CdContext) -> Whereabouts {
    if !is_abs(&dest) {
        let walks_cdpath =
            !(dest == "." || dest == ".." || dest.starts_with("./") || dest.starts_with("../"));
        if walks_cdpath && line.cdpath {
            return Whereabouts::unnamed();
        }
    }
    let mut out = Whereabouts {
        anywhere: here.anywhere && !is_abs(&dest),
        lost: here.lost && !is_abs(&dest),
        ..Whereabouts::default()
    };
    let from: Vec<&str> = if is_abs(&dest) {
        vec![""]
    } else {
        here.dirs.iter().map(String::as_str).collect()
    };
    for h in from {
        let physical = if is_abs(&dest) {
            dest.clone()
        } else {
            join_dir(h, &dest)
        };
        let logical = normpath(&physical);
        out.merge(Whereabouts::at(&physical));
        out.merge(Whereabouts::at(&logical));
    }
    out
}

/// The most `~` words one invocation's directories are read both ways for. Each doubles the
/// readings; past this many the invocation is one the guard cannot name.
const MAX_TILDE_WORDS: usize = 4;

/// How the shell hands git one directory word: the readings it may become, or `None` when the
/// guard cannot name it (#1323).
///
/// `None` is a word the shell expands later — `$`, a command substitution, a glob, a brace — or a
/// `~user`, or a `~` when the line may set `HOME` or `HOME` is not absolute. These are the
/// destinations [`cd_destinations`] cannot name either. A `~` or `~/…` is `$HOME`, as the shell
/// expands it unquoted in a word of its own. The reader has taken the quoting off, though, so a
/// `'~'` or `\~` arrives looking the same and stays a directory named `~`: `literal` keeps that
/// reading as well, and the caller sets it wherever the shell may leave the `~` alone.
fn directory_readings(word: &str, literal: bool, line: &CdContext) -> Option<Vec<String>> {
    if word.contains(['$', '`', '*', '?', '[', '{']) {
        return None;
    }
    if !word.starts_with('~') {
        return Some(vec![word.to_string()]);
    }
    if !(word == "~" || word.starts_with("~/")) || line.sets_home {
        return None;
    }
    let home = std::env::var("HOME").ok().filter(|h| is_abs(h))?;
    let expanded = format!("{home}{}", &word[1..]);
    Some(if literal {
        vec![word.to_string(), expanded]
    } else {
        vec![expanded]
    })
}

/// One reading of an invocation's directory words: git's own options, its environment, and a
/// wrapper's chdir flag, as [`git_target`] and the walk take them.
type Reading = (Vec<String>, Vec<String>, String);

/// Every way one invocation's directory words may reach git — `(pre, env, chdir)` with each word
/// replaced by one of its [`directory_readings`] — or `None` when one of them cannot be named.
///
/// The words are git's own `-C`, `--git-dir` and `--work-tree` (separated or attached), the
/// `GIT_DIR`/`GIT_WORK_TREE` it inherits, and a wrapper's chdir flag. Nothing else in `pre` or
/// `env` is read: a `$` in `-c x.y=$V` names no directory. A word `made` says an earlier
/// segment of the line makes or re-points cannot be named either ([`Made`], #1326).
fn invocation_readings(
    pre: &[String],
    env: &[String],
    chdir: &str,
    chdir_separated: bool,
    line: &CdContext,
    made: &dyn Fn(&str) -> bool,
) -> Option<Vec<Reading>> {
    let directory_readings = |word: &str, literal: bool, line: &CdContext| {
        if made(word) {
            return None;
        }
        directory_readings(word, literal, line)
    };
    // A separated word's `~` stays literal only when quoted or escaped, which the reader cannot
    // see in the word but the line's text shows ([`CdContext::quotes_tilde`]). An attached `--git-dir=~`, an
    // assignment's value (zsh leaves `GIT_DIR=~/x` alone after `env`) and an attached wrapper
    // chdir flag (`env --chdir=~` and `env -C~` leave it alone everywhere) keep both always.
    // Every word that names a directory: where it is, and its readings.
    enum At {
        Pre(usize, &'static str),
        Env(usize, &'static str),
        Chdir,
    }
    let mut words: Vec<(At, Vec<String>)> = Vec::new();
    let mut i = 0usize;
    while i < pre.len() {
        let tok = pre[i].as_str();
        if matches!(tok, "-C" | "--git-dir" | "--work-tree") {
            if let Some(val) = pre.get(i + 1) {
                let literal = line.quotes_tilde;
                words.push((At::Pre(i + 1, ""), directory_readings(val, literal, line)?));
            }
            i += 2;
            continue;
        }
        for prefix in ["--git-dir=", "--work-tree="] {
            if let Some(val) = tok.strip_prefix(prefix) {
                words.push((At::Pre(i, prefix), directory_readings(val, true, line)?));
            }
        }
        i += if GIT_VALUE_OPTS.contains(&tok) { 2 } else { 1 };
    }
    for (j, assign) in env.iter().enumerate() {
        for prefix in ["GIT_DIR=", "GIT_WORK_TREE="] {
            if let Some(val) = assign.strip_prefix(prefix) {
                words.push((At::Env(j, prefix), directory_readings(val, true, line)?));
            }
        }
    }
    if !chdir.is_empty() {
        let literal = !chdir_separated || line.quotes_tilde;
        words.push((At::Chdir, directory_readings(chdir, literal, line)?));
    }
    if words.iter().filter(|(_, r)| r.len() > 1).count() > MAX_TILDE_WORDS {
        return None;
    }
    let mut out = vec![(pre.to_vec(), env.to_vec(), chdir.to_string())];
    for (at, readings) in &words {
        let mut next = Vec::new();
        for (p, e, c) in &out {
            for reading in readings {
                let (mut p, mut e, mut c) = (p.clone(), e.clone(), c.clone());
                match at {
                    At::Pre(k, prefix) => p[*k] = format!("{prefix}{reading}"),
                    At::Env(k, prefix) => e[*k] = format!("{prefix}{reading}"),
                    At::Chdir => c = reading.clone(),
                }
                next.push((p, e, c));
            }
        }
        out = next;
    }
    Some(out)
}

/// Every git invocation in `cmd` that acts on the PLANE ROOT — `_plane_root_git`, the walk both
/// guards share, so the two of them share one pair of eyes.
///
/// `root` is the plane root as `Path(config.ROOT).resolve()` makes it; [`plane_root`] makes it.
///
/// * **Fails OPEN on a command that would not tokenize**, in one place: the re-segmented guess
///   is made without quoting, and a phantom `git checkout` out of a broken quote must not stop a
///   turn. The leak and one-credential guards, which may not miss, keep scanning it.
/// * **A `cd` earlier in the SAME command moves where later segments run** (#183) — but only a
///   `cd` whose failure stops them is taken to have moved them for certain: the shell's own `cd`,
///   joined to what follows by `&&`, and only until that `&&` chain ends. Every other `cd` — one
///   followed by `;`, `||`, `&` or a newline, one in a pipeline or a subshell, one run as a
///   program — ADDS its destination and keeps the directory the shell was in (#345).
/// * **A wrapper's own chdir flag** (`env -C`, `sudo --chdir`) moves git as a `cd` would.
/// * **Any** subject being the root is enough ([`git_target`]), from any directory the shell may
///   be in, and under every reading of the directory words git is sent to; a word the guard
///   cannot name puts the invocation on the root (#1323).
/// * **A string a shell is handed to run is read as a line of its own** (#1325): a shell's
///   `-c` ([`shellwrap::shell_scripts_of`], every shell that runs one), `eval`'s words, a
///   here-string, what a pipe feeds a shell reading its stdin
///   ([`crate::consentspelling::fed_on_its_stdin`]), the command a `find -exec` runs, and a
///   heredoc body fed to fish, read as fish reads it. It starts where that shell may be, with
///   what the line exported to it and what it made; a `cd` inside it moves only that shell,
///   except under `eval`, which runs in this one, and whose exports the line keeps. A string handed on deeper than [`guardcaps::MAX_NESTING`] is not read: where it
///   may name git, the line is refused unread ([`plane_root_branch_reason`]).
pub fn plane_root_git(cmd: &str, cwd: &str, root: &str) -> Vec<RootInvocation> {
    read_line(cmd, cwd, root).0
}

/// [`plane_root_git`]'s invocations, and whether a string a shell is handed was nested past
/// what the walk reads while it may name git.
fn read_line(cmd: &str, cwd: &str, root: &str) -> (Vec<RootInvocation>, bool) {
    let walk = walked(cmd, cwd, root, None);
    (walk.out, walk.too_deep)
}

/// The walk of `cmd`, gathering the git invocations that act elsewhere too where `elsewhere`
/// is a list to put them in.
fn walked(cmd: &str, cwd: &str, root: &str, elsewhere: Option<Vec<Elsewhere>>) -> Walk {
    // What the walk asks git — the line's aliases, whether a word is a tracked path — is asked
    // once for the line, whichever guard reads it ([`remembering`]).
    remembering(|| walked_once(cmd, cwd, root, elsewhere))
}

/// [`walked`], inside its memory.
fn walked_once(cmd: &str, cwd: &str, root: &str, elsewhere: Option<Vec<Elsewhere>>) -> Walk {
    let shared = Shared {
        root,
        the_root: std::cell::OnceCell::new(),
        aliases: AliasBook::of(root),
        disk: std::cell::RefCell::default(),
    };
    let mut walk = Walk {
        made: Made::default(),
        out: Vec::new(),
        elsewhere,
        wrote: false,
        changed: false,
        git_wrote: false,
        configures_git: false,
        too_deep: false,
        budget: STRING_BUDGET_PER_BYTE
            .saturating_mul(cmd.len())
            .saturating_add(STRING_BUDGET_FLOOR),
    };
    walk_line(
        cmd,
        Whereabouts::at(cwd),
        &Outer::default(),
        0,
        &shared,
        &mut walk,
    );
    walk
}

/// One git invocation that acts on a repository other than the project's own: a workspace's
/// clone, a branch folder, a scratch checkout (qw349, [`crate::cloneupdate`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Elsewhere {
    /// The invocation, as the root's are read. Its `unnamed` is never set: one sent where the
    /// guard cannot name is listed for the root ([`RootInvocation::unnamed`]).
    pub inv: RootInvocation,
    /// Every directory git may be run in, after its `-C`s and a wrapper's chdir flag: one for
    /// each directory the shell may be in and each reading of a word.
    pub dirs: Vec<String>,
    /// More directories than [`MAX_ELSEWHERE_DIRS`]: the rest are not listed.
    pub more_dirs: bool,
    /// git is pointed at a repository or a work tree by name (`--git-dir`, `--work-tree`,
    /// `GIT_DIR`, `GIT_WORK_TREE`), so the folder it runs in is not the whole answer.
    pub pointed: bool,
    /// A command that may write comes before it on the line, in a string a shell is handed
    /// too: any program but one that only reads or prints ([`may_write`]), a redirection
    /// that writes a file, git other than a [`GIT_READS`] subcommand. Such a command may move
    /// a name, or the commit a folder is at, after the guard has read them.
    pub after_a_writer: bool,
}

/// The most directories one [`Elsewhere`] lists.
pub const MAX_ELSEWHERE_DIRS: usize = 8;

/// git subcommands that write nothing a later git on the same line reads a commit from: what
/// may stand beside a judged command without making its target one the guard read too early.
/// None of them can be an alias, since git never lets an alias shadow a command of its own.
pub const GIT_READS: [&str; 16] = [
    "blame",
    "cat-file",
    "describe",
    "diff",
    "for-each-ref",
    "grep",
    "help",
    "log",
    "ls-files",
    "ls-tree",
    "merge-base",
    "rev-list",
    "rev-parse",
    "show",
    "status",
    "version",
];

/// Every git invocation on a line, sorted by where it acts (qw349).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LineGit {
    /// On the project's root, or where the guard cannot name ([`plane_root_git`]).
    pub root: Vec<RootInvocation>,
    /// On another repository.
    pub elsewhere: Vec<Elsewhere>,
    /// A string a shell is handed was not read, and may name git.
    pub too_deep: bool,
    /// A word on the line is spelled like a redirection but quoted or escaped (`'>'`), so it is
    /// an operand, and a reader that drops redirections by their spelling would drop it too.
    pub quoted_redirection: bool,
}

/// [`plane_root_git`]'s walk, answering the invocations that act elsewhere too.
pub fn line_git(cmd: &str, cwd: &str, root: &str) -> LineGit {
    let walk = walked(cmd, cwd, root, Some(Vec::new()));
    // Read off the source, where quoting is still known; a line the lexer refuses counts.
    let quoted_redirection = crate::shellseg::lex(cmd).map_or(true, |toks| {
        toks.iter()
            .any(|tok| !tok.bare && shellwrap::is_redirect_token(&tok.text))
    });
    LineGit {
        root: walk.out,
        elsewhere: walk.elsewhere.unwrap_or_default(),
        too_deep: walk.too_deep,
        quoted_redirection,
    }
}

/// What one command line shares with every string its shells are handed: the root, stat-ed on
/// the first git invocation so a line with none pays nothing for it, and the root's configured
/// aliases, read once for the whole line and only if it needs them.
struct Shared<'a> {
    root: &'a str,
    the_root: std::cell::OnceCell<TheRoot>,
    aliases: AliasBook<'a>,
    disk: std::cell::RefCell<Disk>,
}

/// What the disk said of the directories the gits on one line may run in: asked once for the
/// line, not once for each git (qw367). Every git on a line is read against every directory
/// the shell may be in, and most of a line's gits are sent to the same few, so asking again
/// each time made a line of `cd <name> && git …;` cost the cube of its length.
///
/// Nothing a line does is read back off the disk as it is read: what a segment makes is
/// followed on the line itself ([`Made`]), so an answer holds for the whole of it.
#[derive(Default)]
struct Disk {
    /// Whether git, pointed at a directory, acts on the root's repository
    /// ([`TheRoot::is_reached_from`]).
    reaches: std::collections::HashMap<String, bool>,
    /// The `core.worktree` of the repository a directory is in, or of a named git directory
    /// ([`gitconfig::configured_work_tree`]).
    work_trees: std::collections::HashMap<(String, Option<String>), Option<String>>,
}

/// The most answers [`Disk`] keeps of each kind. Past it the disk is asked again, so a line
/// that sends git to thousands of different directories costs time and not memory.
const MAX_DISK_ANSWERS: usize = 1024;

impl Shared<'_> {
    /// [`gitconfig::configured_work_tree`], asked once for the line.
    fn configured_work_tree(&self, here: &str, git_dir: Option<&str>) -> Option<String> {
        let key = (here.to_string(), git_dir.map(str::to_string));
        if let Some(known) = self.disk.borrow().work_trees.get(&key) {
            return known.clone();
        }
        let found = gitconfig::configured_work_tree(here, git_dir);
        let mut disk = self.disk.borrow_mut();
        if disk.work_trees.len() < MAX_DISK_ANSWERS {
            disk.work_trees.insert(key, found.clone());
        }
        found
    }

    /// Whether git, pointed at `target`, acts on the root's repository, asked once for the
    /// line.
    fn reaches_the_root(&self, target: &str) -> bool {
        if let Some(known) = self.disk.borrow().reaches.get(target) {
            return *known;
        }
        let found = self
            .the_root
            .get_or_init(|| TheRoot::of(self.root))
            .is_reached_from(target);
        let mut disk = self.disk.borrow_mut();
        if disk.reaches.len() < MAX_DISK_ANSWERS {
            disk.reaches.insert(target.to_string(), found);
        }
        found
    }

    /// Where one git may run, under each reading of its directory words and from each
    /// directory the shell may be in: whether one of them acts on the root's repository, and
    /// otherwise, where `listed`, every directory it runs in, [`MAX_ELSEWHERE_DIRS`] at most,
    /// and whether there were more. One pass over the directories, for both answers.
    fn placed(
        &self,
        readings: &[Reading],
        here: &[String],
        listed: bool,
    ) -> (bool, Vec<String>, bool) {
        let configured = |dir: &str, git_dir: Option<&str>| self.configured_work_tree(dir, git_dir);
        let (mut dirs, mut more): (Vec<String>, bool) = (Vec::new(), false);
        for (pre, inherited, chdir) in readings {
            for dir in here {
                let dir = if chdir.is_empty() {
                    dir.clone()
                } else {
                    join_dir(dir, chdir) // `env -C`, `sudo --chdir`
                };
                let targets = git_target_with(&dir, pre, inherited, &configured);
                if targets.iter().any(|target| self.reaches_the_root(target)) {
                    return (true, Vec::new(), false);
                }
                // The directory git runs in is the first of its subjects ([`git_target`]).
                let Some(runs_in) = targets.into_iter().next().filter(|_| listed && !more) else {
                    continue;
                };
                if dirs.contains(&runs_in) {
                    continue;
                }
                // Past the cap nothing more is listed: the caller takes the rest as unnamed.
                if dirs.len() >= MAX_ELSEWHERE_DIRS {
                    more = true;
                    continue;
                }
                dirs.push(runs_in);
            }
        }
        (false, dirs, more)
    }
}

/// What the walk gathers across a line and the strings in it.
struct Walk {
    /// What earlier segments make or re-point: a directory word reaching it is unnameable
    /// (#1326). One for the line and its strings, since a string's `ln` makes a path for the
    /// line after it as surely as the line's own.
    made: Made,
    out: Vec<RootInvocation>,
    /// The invocations that act on another repository, where the caller asked for them
    /// ([`line_git`]).
    elsewhere: Option<Vec<Elsewhere>>,
    /// A command that may write has been met ([`Elsewhere::after_a_writer`]).
    wrote: bool,
    /// A command that may change what the guards read of the repository — a ref, a commit,
    /// configuration — has been met ([`RootInvocation::after_a_writer`]): [`Self::wrote`] less
    /// the gits in the root that only put files back ([`touches_only_files`]).
    changed: bool,
    /// A git that may write has been met, or a program that may be git told to `config`: a
    /// word after a program only the shell can name may then be an alias it defined.
    git_wrote: bool,
    /// A segment so far sets or exports a variable that points git at configuration (a
    /// `GIT_*` variable, `HOME`, `XDG_CONFIG_HOME`): no git after it is taken to only put files
    /// back ([`touches_only_files`]).
    configures_git: bool,
    /// A string was handed on past [`guardcaps::MAX_NESTING`], or past the budget, where it
    /// may name git.
    too_deep: bool,
    /// The bytes of handed-on strings the walk may still read ([`STRING_BUDGET_PER_BYTE`]).
    budget: usize,
}

/// How many bytes of strings handed on the walk reads for each byte of the line, plus
/// [`STRING_BUDGET_FLOOR`]. A string a shell is handed is read from the text around it, but one
/// a pipe feeds is read once for each shell along the pipeline and each way a feeder may print
/// it, and those readings nest: without a budget a line could cost the readings' product,
/// level on level. Past it, what is left is refused unread where it may name git (#1325).
const STRING_BUDGET_PER_BYTE: usize = 128;

/// The bytes of handed-on strings any line may have read, however short.
const STRING_BUDGET_FLOOR: usize = 64 * 1024;

/// What the segment that hands a shell a string gives it: the environment exported to it,
/// every bare assignment before it, its own assignments, whether a loose `cd` came before it,
/// what the line around it does to a `cd`, and whether `set -a` is on where an `eval` runs the
/// string in this shell. Empty for the line itself.
///
/// The lists are kept in parts, one or two for each level the string is in, each a prefix of a
/// chain the line shares ([`LineEnv`]): a line of many assignments handing many strings on
/// would otherwise copy the one into the other, its square.
#[derive(Default)]
struct Outer<'a> {
    exported: Vec<Chain>,
    assigned: Vec<Chain>,
    own_env: Vec<&'a [String]>,
    loose_cd: bool,
    context: Option<CdContext>,
    allexport: bool,
}

/// The parts of a list, in order, as one.
fn joined(parts: &[&[String]]) -> Vec<String> {
    parts.iter().flat_map(|part| part.iter().cloned()).collect()
}

/// What a line leaves the shell that read it: every directory it may be in, what it exported
/// and assigned there, and whether `set -a` is on after it. Where an `eval` leaves the shell
/// that ran it (#1325).
struct Left {
    here: Whereabouts,
    exported: Vec<String>,
    assigned: Vec<String>,
    allexport: bool,
}

impl Left {
    fn at(here: Whereabouts) -> Self {
        Left {
            here,
            exported: Vec::new(),
            assigned: Vec::new(),
            allexport: false,
        }
    }
}

/// How many heredoc bodies fed to fish one line's walk reads again as fish reads them; past it
/// the line is refused unread where it may name git, as a string nested too deep is.
const MAX_FED_BODIES: usize = 64;

/// One command line, or a string a shell runs `depth` levels in, read from `start` — the walk
/// [`plane_root_git`] documents. Answers what the line leaves its shell ([`Left`]).
fn walk_line(
    cmd: &str,
    start: Whereabouts,
    outer: &Outer<'_>,
    depth: usize,
    shared: &Shared<'_>,
    walk: &mut Walk,
) -> Left {
    if depth > guardcaps::MAX_NESTING {
        // A git the walk cannot read may move the root: refused unread where one may be named,
        // a program in a variable among them (#1354).
        walk.too_deep |= shellwrap::may_name(cmd, "git") || cmd.contains('$');
        return Left::at(start);
    }
    if depth > 0 {
        let Some(left) = walk.budget.checked_sub(cmd.len()) else {
            walk.budget = 0;
            walk.too_deep |= shellwrap::may_name(cmd, "git") || cmd.contains('$');
            return Left::at(start);
        };
        walk.budget = left;
    }
    let Some(segments) = shellseg::joined_argv(cmd) else {
        return Left::at(start);
    };
    let argvs: Vec<Vec<String>> = segments.iter().map(|s| s.argv.clone()).collect();
    let carried = shellwrap::exported_env_shared(&argvs);
    let aliases = &shared.aliases;
    let mut here = start;
    // Every directory the shell is in at some point of the line: where a heredoc body it feeds
    // fish may run, since the body is not tied to the segment that reads it. Kept only where
    // one may be read.
    let fish_heredocs = cmd.contains("<<") && shellwrap::may_name(cmd, "fish");
    let mut ever = here.clone();
    // The bare assignments an `eval` made: shell variables a later `export NAME` exports.
    let mut evals_set: Vec<String> = Vec::new();
    // What the line has exported so far, in order: its own exports, read ahead by `carried`,
    // and those an `eval` made, as each is met (#1325). Grows by each once, so it stays linear.
    let mut exported: Vec<String> = Vec::new();
    let mut exports_taken = 0usize;
    // `exported` and `assigned` (below) as chains each git and string shares a prefix of, each
    // entry copied in once: a git's environment is then a few pointers, not a copy of all the
    // line set before it, which made a line alternating exports and gits cost its square (#2648).
    let mut exported_chain = Chain::default();
    let mut assigned_chain = Chain::default();
    // Where the shell stays if a `cd … &&` failed: back in play once the `&&` chain ends.
    let mut left_behind = Whereabouts::default();
    // A certain `cd` not joined by `&&` came before: what the denial's hint is for (#1326).
    let mut loose_cd = outer.loose_cd;
    let context = CdContext::of(cmd).with(outer.context.as_ref());
    // Every assignment a bare assignment segment (`A=1;`) has made so far: each distinct one
    // once, in order, so a line repeating one stays short and every value a variable was given
    // is still there to be read (D-1358f). A string's line's come before them.
    let mut assigned: Vec<String> = Vec::new();
    let mut assigned_seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    // `set -a`: on from a segment of this line, which `carried` reads too, or on in the shell
    // before the line began or from an `eval` in it, which `carried` cannot know. Under the
    // second, this walk exports each bare assignment itself.
    let mut allexport_own = false;
    let mut allexport_given = outer.allexport;
    for (i, seg) in segments.iter().enumerate() {
        // Taken apart each segment, so a string handed on can be walked with the whole of it.
        let own = carried.before(i);
        exported.extend_from_slice(&own[exports_taken..]);
        exports_taken = own.len();
        if fish_heredocs {
            ever.merge(here.clone());
        }
        // A function's body runs wherever, and with whatever arguments, the shell calls it:
        // read as a line of its own from a directory the guard cannot name.
        if let Some(body) = function_body(&seg.argv) {
            let line: Vec<String> = body.iter().map(|w| crate::handoff::quote(w)).collect();
            walk_line(
                &line.join(" "),
                Whereabouts::unnamed(),
                &Outer::default(),
                depth + 1,
                shared,
                walk,
            );
        }
        let before = exported_chain.freeze(&exported);
        let assigned_before = assigned_chain.freeze(&assigned);
        walk.configures_git |= seg.argv.iter().any(|w| sets_git_configuration(w));
        let (out, made) = (&mut walk.out, &mut walk.made);
        let (elsewhere, wrote, changed, git_wrote, configures_git) = (
            &mut walk.elsewhere,
            &mut walk.wrote,
            &mut walk.changed,
            &mut walk.git_wrote,
            &walk.configures_git,
        );
        // Whether the segment is one of git's, which answers for itself below.
        let mut gits = false;
        let call = shellwrap::split_env_chdir(&seg.argv);
        let (prog, env, args, chdir) = (call.prog, call.env, call.argv, call.chdir);
        // What this segment makes, read where it runs and taken in once it is read.
        let makes = Made::of(&seg.argv, &prog, &args, &here);
        // The segment's own assignments: the only line-set configuration git surely sees
        // (#1358), after those of the segments that handed this string on. Built only for a
        // segment that may be git.
        let own_env_of = || {
            let mut out = joined(&outer.own_env);
            out.extend(env.iter().cloned());
            out
        };
        // `before + env`, built only for a segment that may be git, of the chains the line
        // shares: copying every earlier export into every segment made a line of exports cost
        // its square. A string starts with what its line exported to it.
        let inherited_parts = || {
            let mut parts = outer.exported.clone();
            parts.push(before.clone());
            parts.push(Chain::default().freeze(&own_env_of()));
            parts
        };
        let inherited = || LineEnv::of(inherited_parts());
        // What may set git's configuration for this segment: every bare assignment the line
        // made before it, exported or not — HOME and the session's own GIT_CONFIG_* are already
        // exported, so a bare `HOME=/x;` reaches git — then the exports and the segment's own.
        // Built only for a segment that may be git, so a long line of assignments costs what it
        // is long.
        let config_env = || {
            let mut parts = outer.assigned.clone();
            parts.push(assigned_before.clone());
            parts.extend(inherited_parts());
            LineEnv::of(parts)
        };
        if let Some(cd) = cd_call(&seg.argv, &prog, &args) {
            let moved = if cd.args.iter().any(|arg| made.reaches(arg, &here)) {
                Whereabouts::unnamed()
            } else {
                cd_destinations(&cd.builtin, &cd.args, &here, &context)
            };
            let piped = matches!(seg.before.as_deref(), Some("|" | "|&"));
            if cd.certain && !context.redefined && !piped && seg.after.as_deref() == Some("&&") {
                left_behind.merge(std::mem::replace(&mut here, moved));
            } else {
                // Only a `cd` that `&&` would have made certain gets the hint: one the line
                // redefined, or one in a pipeline, stays uncertain whatever joins it.
                let pipes = matches!(seg.after.as_deref(), Some("|" | "|&"));
                loose_cd |= cd.certain && !context.redefined && !piped && !pipes;
                here.merge(moved);
            }
        } else if prog.contains('$') {
            // The program is a word only the shell can name (`$G checkout -b x` after `G=git`,
            // `$(which git) …`): it may be git, sent anywhere (#1354). Answered for when a
            // branch move or reset follows it, or an alias of one, read through the line's one
            // alias book as after a substitution among git's options (#1326). Only a word an
            // alias book the guard reads names is followed: a program that may not be git is
            // not refused for every word because the line points git at a config file.
            // A program with no words after it runs nothing the guards judge, and is left
            // before the line's configuration is gathered for it.
            let words = args.get(1..).unwrap_or(&[]);
            let (env, own_env) = if words.is_empty() {
                (LineEnv::default(), Vec::new())
            } else {
                (config_env(), own_env_of())
            };
            // After a git that may write, any word may be an alias it defined.
            let after_a_writer = *changed;
            let configured = *git_wrote;
            // Its first word after git's own options, filled in by the shell: a later word that
            // reads as a command is not taken to be the one git runs.
            let command_filled = {
                let (pre, rest) = shellwrap::git_globals(&args);
                shell_fills_the_command(&pre, &rest)
            };
            let found = match literal_mover_after(words) {
                Some((sub, post)) => Some((sub, post, Vec::new(), false)),
                None if words.is_empty() => None,
                None => {
                    let (pre, rest) = shellwrap::git_globals(&args);
                    subcommand_after_splice(
                        &pre,
                        &rest,
                        &env.to_vec(),
                        &own_env,
                        aliases,
                        configured,
                        configured,
                    )
                    .map(|(sub, post, unread)| (sub, post, pre, unread))
                }
            };
            gits = true;
            *wrote |= !words.is_empty();
            *changed |= !words.is_empty();
            // It may be no git at all: only a word that sets configuration makes the words
            // after the next such program read as aliases.
            *git_wrote |= words.iter().any(|w| w == "config");
            if let Some((sub, post, pre, unread)) = found {
                out.push(RootInvocation {
                    sub,
                    post,
                    pre,
                    unnamed: true,
                    unread,
                    program_unnamed: true,
                    env,
                    own_env,
                    after_loose_cd: loose_cd,
                    after_a_writer,
                    unquoted_substitution: false,
                    command_filled,
                });
            }
        } else if let Some(at) = spliced_before_program(&seg.argv, &prog) {
            // The program itself is lost in a substitution's words (#1354): `GIT_DIR=$(…) git …`
            // and `env -C $(…) git …` read as running the substitution's first word. A git
            // among what follows may be the one that runs, sent where the guard cannot name.
            let (pre, rest) = shellwrap::git_globals(&seg.argv[at..]);
            let (env, own_env) = (config_env(), own_env_of());
            gits = true;
            let after_a_writer = *changed;
            *wrote = true;
            *changed = true;
            *git_wrote = true;
            if let Some((sub, post, unread)) = subcommand_after_splice(
                &pre,
                &rest,
                &env.to_vec(),
                &own_env,
                aliases,
                true,
                after_a_writer,
            ) {
                out.push(RootInvocation {
                    sub,
                    post,
                    pre,
                    unnamed: true,
                    unread,
                    program_unnamed: false,
                    env,
                    own_env,
                    after_loose_cd: loose_cd,
                    after_a_writer,
                    unquoted_substitution: false,
                    command_filled: false,
                });
            }
        } else if shellwrap::base_lower(&prog) == "git" {
            gits = true;
            // Folded: on APFS and NTFS `GIT` runs git (#346).
            let (pre, rest) = shellwrap::git_globals(&args);
            // A substitution among git's own options (#1354): its words, and every option after
            // it, are spread through what follows, so neither the directory nor the subcommand
            // can be read from their places.
            // And a command word, or an option of git's own, that the shell fills in: which
            // command git runs, and where, is the shell's to say. A branch move or reset among
            // the words after it is judged as written; else the command is unread.
            let filled = shell_fills_the_command(&pre, &rest);
            let unquoted = pre.iter().any(|t| t.ends_with('$'));
            let spliced = filled || unquoted;
            let (env, own_env) = (config_env(), own_env_of());
            let after_wrote = *wrote;
            let after_a_writer = *changed;
            let first = || {
                rest.split_first()
                    .map(|(sub, post)| (sub.clone(), post.to_vec(), false))
            };
            let words = if spliced {
                // A branch move or reset among the words after it is judged as written; else
                // a command word the shell fills in is unread, and an unquoted substitution is
                // kept for the branch guard to refuse ([`RootInvocation::unquoted_substitution`]).
                let found = subcommand_after_splice(
                    &pre,
                    &rest,
                    &env.to_vec(),
                    &own_env,
                    aliases,
                    true,
                    after_a_writer,
                );
                match found {
                    Some(found) => Some(found),
                    None if filled || unquoted => {
                        let (sub, post, _) = first().unwrap_or_default();
                        Some((sub, post, filled))
                    }
                    None => None,
                }
            } else {
                first()
            };
            if let Some((sub, post, unread)) = words {
                let writes = spliced || !GIT_READS.contains(&sub.as_str());
                *wrote |= writes;
                // `before + env`, in that order: an assignment on THIS invocation overrides an
                // export.
                // `env -C ~/x` hands the shell a word of its own; `env --chdir=~/x` does not.
                let chdir_separated = seg
                    .argv
                    .windows(2)
                    .any(|w| matches!(w[0].as_str(), "-C" | "-D" | "--chdir") && w[1] == chdir);
                // Only what the walk reads of it ([`LineEnv::read_by_the_walk`]): the directory
                // variables and a `core.worktree`, as long as those are and not as the line.
                let inherited = inherited().read_by_the_walk();
                let readings = invocation_readings(
                    &pre,
                    &inherited,
                    &chdir,
                    chdir_separated,
                    &context,
                    &|word| made.reaches(word, &here),
                );
                // Asked of the disk in one pass, and once for the line ([`Disk`]): whether it
                // acts on the root, and where it runs for one that acts on another repository
                // (qw349), listed only for a caller that asked and only where the shell is
                // somewhere the guard can name.
                let (named, dirs, more_dirs) = match &readings {
                    Some(readings) => {
                        shared.placed(readings, &here.dirs, elsewhere.is_some() && !here.anywhere)
                    }
                    None => (false, Vec::new(), false),
                };
                // Fail closed: a directory the guard cannot name may be the root (#1323).
                let unnamed = spliced || (!named && (here.anywhere || readings.is_none()));
                let acts_in = match (&readings, elsewhere.is_some() && !named && !unnamed) {
                    (Some(readings), true) => Some((dirs, more_dirs, pointed_by_name(readings))),
                    _ => None,
                };
                // One in the root that only puts files back changes nothing the guards read of
                // the repository after it.
                // Once the line has changed it, nothing more is asked of git for it.
                // Not where the line sets git configuration, which can run a program
                // (`core.fsmonitor`, a clean filter) as files are put back.
                let changes = writes
                    && !(!*changed
                        && !*configures_git
                        && !carries_config(&pre)
                        && (named || unnamed)
                        && !spliced
                        && touches_only_files(&sub, &post, shared.root));
                *changed |= changes;
                *git_wrote |= changes;
                let inv = RootInvocation {
                    sub,
                    post,
                    pre,
                    unnamed,
                    unread,
                    program_unnamed: false,
                    env,
                    own_env,
                    after_loose_cd: loose_cd,
                    after_a_writer,
                    unquoted_substitution: unquoted,
                    command_filled: filled,
                };
                if named || unnamed {
                    out.push(inv);
                } else if let (Some(list), Some((dirs, more_dirs, pointed))) = (elsewhere, acts_in)
                {
                    list.push(Elsewhere {
                        inv,
                        dirs,
                        more_dirs,
                        pointed,
                        after_a_writer: after_wrote,
                    });
                }
            }
        }
        // A string this segment hands a shell runs where that shell runs, with what the line
        // gives it (#1325).
        // So does what a pipe feeds a shell that reads its script from stdin, and what a `find`
        // runs (read as a line of its own, from where `find` runs, or anywhere for `-execdir`).
        let scripts: Vec<String> = shellwrap::shell_scripts_of(&prog, &args)
            .into_iter()
            .chain(shellwrap::here_string_script_of(&prog, &args))
            .chain(crate::consentspelling::fed_on_its_stdin(&segments, i))
            .collect();
        let found = shellwrap::commands_a_find_runs(&prog, &args);
        let evals = shellwrap::basename(&prog) == "eval";
        let handed = (!scripts.is_empty() || !found.is_empty()).then(|| {
            let gives = Outer {
                exported: outer
                    .exported
                    .iter()
                    .cloned()
                    .chain([before.clone()])
                    .collect(),
                assigned: outer
                    .assigned
                    .iter()
                    .cloned()
                    .chain([assigned_before.clone()])
                    .collect(),
                own_env: outer.own_env.iter().copied().chain([&env[..]]).collect(),
                loose_cd,
                context: Some(context.clone()),
                // An `eval` runs its string in this shell, under its options; any other shell
                // starts with its own.
                allexport: evals && (allexport_own || allexport_given),
            };
            let runs_at = where_a_shell_runs(&here, &chdir, &seg.argv, &context, made);
            (runs_at, gives)
        });
        made.take(makes);
        // Any other program that may write, for the gits after it ([`Elsewhere::after_a_writer`]).
        // A shell handed a string, a `find` that runs commands and an `eval` are read through:
        // what they run answers for itself.
        let moves_the_shell = cd_call(&seg.argv, &prog, &args).is_some();
        let read_through = gits || moves_the_shell || handed.is_some() || evals;
        let writes =
            writes_a_file(&seg.argv) || (!read_through && may_write(&seg.argv, &prog, &args));
        walk.wrote |= writes;
        walk.changed |= writes;
        // Each string is read now, so what it makes reaches the segments after it, and where an
        // `eval` leaves this shell is where they run.
        let mut evaled = Left::at(Whereabouts::default());
        if let Some((runs_at, gives)) = handed {
            for script in &scripts {
                let left = walk_line(script, runs_at.clone(), &gives, depth + 1, shared, walk);
                if evals {
                    evaled.here.merge(left.here);
                    evaled.exported.extend(left.exported);
                    evaled.assigned.extend(left.assigned);
                    evaled.allexport |= left.allexport;
                }
            }
            if !found.is_empty() {
                let in_each_dir = args
                    .iter()
                    .any(|w| matches!(w.as_str(), "-execdir" | "-okdir"));
                let find_runs_at = if in_each_dir {
                    Whereabouts::unnamed()
                } else {
                    runs_at
                };
                for run in &found {
                    let line: Vec<String> = run.iter().map(|w| crate::handoff::quote(w)).collect();
                    walk_line(
                        &line.join(" "),
                        find_runs_at.clone(),
                        &gives,
                        depth + 1,
                        shared,
                        walk,
                    );
                }
            }
        }
        // What an `eval` exported and assigned is this shell's from here on (#1325), and a
        // later `export NAME` exports a variable it set.
        here.merge(evaled.here);
        exported.extend(evaled.exported);
        allexport_given |= evaled.allexport;
        for assign in evaled.assigned {
            if assigned_seen.insert(assign.clone()) {
                assigned.push(assign.clone());
                evals_set.push(assign);
            }
        }
        if !evals_set.is_empty() && shellwrap::exports(&seg.argv) {
            for name in seg.argv.iter().skip(1).filter(|w| !w.contains('=')) {
                let set = evals_set
                    .iter()
                    .filter(|a| a.split_once('=').is_some_and(|(n, _)| n == name));
                exported.extend(set.cloned());
            }
        }
        if !matches!(seg.after.as_deref(), Some("&&" | "|" | "|&")) {
            here.merge(std::mem::take(&mut left_behind));
        }
        if !seg.argv.is_empty() && seg.argv.iter().all(|t| shellwrap::is_env_assignment(t)) {
            for assign in &seg.argv {
                if assigned_seen.insert(assign.clone()) {
                    assigned.push(assign.clone());
                }
            }
            if allexport_given && !allexport_own {
                exported.extend(seg.argv.iter().cloned());
            }
        }
        allexport_own |= shellwrap::sets_allexport(&seg.argv);
    }
    here.merge(left_behind);
    exported.extend_from_slice(&carried.before(segments.len())[exports_taken..]);
    ever.merge(here.clone());
    // The walk reads a heredoc's body as lines of this one, as the lexer hands them over, so a
    // body fed to a shell is read already. One fed to fish is read again as fish reads it, its
    // keywords read past, from every directory the shell was in (#1325).
    if fish_heredocs {
        let mut bodies: Vec<String> = crate::heredoc::Heredocs::of(cmd)
            .bodies_fed_to_a_shell()
            .iter()
            .map(|body| shellwrap::as_sh_reads_fish(body))
            .collect();
        if bodies.len() > MAX_FED_BODIES {
            walk.too_deep |= shellwrap::may_name(cmd, "git") || cmd.contains('$');
            bodies.truncate(MAX_FED_BODIES);
        }
        let gives = Outer {
            exported: outer
                .exported
                .iter()
                .cloned()
                .chain([exported_chain.freeze(&exported)])
                .collect(),
            assigned: outer
                .assigned
                .iter()
                .cloned()
                .chain([assigned_chain.freeze(&assigned)])
                .collect(),
            own_env: outer.own_env.clone(),
            loose_cd: false,
            context: Some(context.clone()),
            allexport: false,
        };
        for body in &bodies {
            walk_line(body, ever.clone(), &gives, depth + 1, shared, walk);
        }
    }
    Left {
        here,
        exported,
        assigned,
        allexport: allexport_own || allexport_given,
    }
}

/// Whether a word sets or exports a variable that points git at configuration: any `GIT_*`
/// variable, `HOME` or `XDG_CONFIG_HOME`, read wide, since it can only add a refusal.
fn sets_git_configuration(word: &str) -> bool {
    let name = word.split_once('=').map_or(word, |(name, _)| name);
    name.starts_with("GIT_") || matches!(name, "HOME" | "XDG_CONFIG_HOME")
}

/// Whether git's own options set configuration for this one invocation (`-c`, `--config-env`).
fn carries_config(pre: &[String]) -> bool {
    pre.iter().any(|w| {
        w == "-c"
            || w.starts_with("--config-env")
            || (w.starts_with("-c") && w.len() > 2 && !w.starts_with("--"))
    })
}

/// Whether `git <sub> <post…>` run in the root only puts files back in the index or the working
/// tree, changing no ref, commit or configuration: `add`, `rm`, `mv`, `restore`, a `reset` to
/// the commit HEAD is at, and a `checkout` the branch guard reads as a file restore (every
/// option restore-only, and paths after a `--` or every operand a tracked path git cannot
/// read as a commit). Asked of git in `root` as the branch guard asks it.
fn touches_only_files(sub: &str, post: &[String], root: &str) -> bool {
    let cut = post.iter().position(|a| a == "--");
    let before = &post[..cut.unwrap_or(post.len())];
    let operands: Vec<&String> = before.iter().filter(|a| !a.starts_with('-')).collect();
    match sub {
        "add" | "rm" | "mv" | "restore" => true,
        "reset" => operands
            .first()
            .is_none_or(|target| matches!(target.as_str(), "HEAD" | "@")),
        "checkout" => {
            let restore_only = before
                .iter()
                .filter(|a| a.starts_with('-') && *a != "-")
                .all(|a| checkout_opt_kind(a) == OptKind::Restore);
            let paths_follow = cut.is_some_and(|at| at + 1 < post.len());
            restore_only
                && (paths_follow
                    || (!operands.is_empty()
                        && operands.len() <= MAX_CHECKOUT_OPERANDS
                        && operands
                            .iter()
                            .all(|w| checkout_operand_kind(root, w) == OperandKind::Path)))
        }
        _ => false,
    }
}

/// The first command of a function's body, where `argv` defines one (`f() { …`, `f() ( …`,
/// `function f { …`): the reader hands the definition over as one segment whose words begin
/// with the name. `None` for any other segment.
fn function_body(argv: &[String]) -> Option<&[String]> {
    let opens = |w: &String| matches!(w.as_str(), "{" | "(");
    let after_name = match argv {
        [kw, _, rest @ ..] if kw == "function" => match rest {
            [open, close, rest @ ..] if open == "(" && close == ")" => rest,
            _ => rest,
        },
        [_, open, close, rest @ ..] if open == "(" && close == ")" => rest,
        _ => return None,
    };
    let body = match after_name {
        [open, rest @ ..] if opens(open) => rest,
        rest => rest,
    };
    (!body.is_empty()).then_some(body)
}

/// Whether a segment redirects into a file: any writing redirection but one to the null
/// device or to a descriptor.
fn writes_a_file(argv: &[String]) -> bool {
    argv.windows(2).any(|pair| {
        WRITING_REDIRECTIONS.contains(&pair[0].as_str())
            && pair[1] != "/dev/null"
            && !pair[1].starts_with('&')
    })
}

/// Whether a segment's program may write: any program but one that only reads or prints.
/// The list is the one the sandbox-block reader keeps ([`crate::sandboxblock::READERS`]),
/// less the three in it that write a file when told to: `tee`, `sed -i` and a `find` that
/// deletes or prints to a file. A segment of assignments alone runs nothing.
fn may_write(argv: &[String], prog: &str, args: &[String]) -> bool {
    if writes_a_file(argv) {
        return true;
    }
    if prog.is_empty() || argv.iter().all(|word| shellwrap::is_env_assignment(word)) {
        return false;
    }
    let base = shellwrap::base_lower(prog);
    if !crate::sandboxblock::READERS.contains(&base.as_str()) {
        return true;
    }
    let told = |starts: &[&str]| {
        args.iter()
            .any(|arg| starts.iter().any(|start| arg.starts_with(start)))
    };
    match base.as_str() {
        "tee" => true,
        "sed" => told(&["-i", "--in-place"]),
        "find" => told(&["-delete", "-fprint", "-fls"]),
        _ => false,
    }
}

/// Whether a word points one git invocation at a repository or a work tree by name
/// ([`Elsewhere::pointed`]), under any reading of its directory words: git's own options, the
/// variables it inherits, and a `core.worktree` the line itself configures, however it is
/// handed over (`-c`, `--config-env`, a `GIT_CONFIG_*` variable). The last is read wide, as
/// any word that spells the key: it can only add a refusal.
fn pointed_by_name(readings: &[Reading]) -> bool {
    let sets_work_tree = |word: &String| word.to_ascii_lowercase().contains("core.worktree");
    readings.iter().any(|(pre, inherited, _)| {
        pre.iter().any(|word| {
            let name = word.split_once('=').map_or(word.as_str(), |(name, _)| name);
            matches!(name, "--git-dir" | "--work-tree") || sets_work_tree(word)
        }) || inherited.iter().any(|assign| {
            sets_work_tree(assign)
                || GIT_DIR_ENV
                    .iter()
                    .any(|(name, _)| assign.split_once('=').is_some_and(|(n, _)| n == *name))
        })
    })
}

/// Every directory a shell handed a string may run it in: where this one may be, moved by a
/// wrapper's chdir flag (`env -C <dir> sh -c …`). A flag the guard cannot name may be anywhere.
fn where_a_shell_runs(
    here: &Whereabouts,
    chdir: &str,
    argv: &[String],
    line: &CdContext,
    made: &Made,
) -> Whereabouts {
    if chdir.is_empty() {
        return here.clone();
    }
    let separated = argv
        .windows(2)
        .any(|w| matches!(w[0].as_str(), "-C" | "-D" | "--chdir") && w[1] == chdir);
    let readings = (!made.reaches(chdir, here))
        .then(|| directory_readings(chdir, !separated || line.quotes_tilde, line))
        .flatten();
    let Some(readings) = readings else {
        return Whereabouts::unnamed();
    };
    let mut out = Whereabouts {
        anywhere: here.anywhere,
        lost: here.lost,
        ..Whereabouts::default()
    };
    for dir in &here.dirs {
        for reading in &readings {
            out.merge(Whereabouts::at(&join_dir(dir, reading)));
        }
    }
    out
}

/// Where a git invocation may begin in a segment whose PROGRAM was taken from a substitution's
/// words, or `None`.
///
/// The reader spreads an unquoted `$(…)` or backtick substitution through the argv it stands in:
/// the word it began in ends in `$`, then come its own words, then the rest (#1354). When that
/// word stands before the program — an assignment, a wrapper's option — the program read is the
/// substitution's first word; the first `git` after it is answered for instead.
fn spliced_before_program(argv: &[String], prog: &str) -> Option<usize> {
    let mark = argv.iter().position(|t| t.ends_with('$'))?;
    let prog_at = argv.iter().position(|t| t == prog)?;
    if mark >= prog_at || shellwrap::base_lower(prog) == "git" {
        return None;
    }
    argv.iter()
        .skip(mark + 1)
        .position(|t| shellwrap::base_lower(t) == "git")
        .map(|i| mark + 1 + i)
}

/// The subcommand of a git invocation whose options held a substitution, its arguments, and
/// whether it could not be read (#1354).
///
/// Which word git really sees first cannot be read, so the guard looks for the words it judges:
/// the first literal branch mover or `reset` among the words that follow; else the first word
/// named as an alias — inline (`-c alias.co=checkout`, before the word) or in the root's config
/// — that resolves to one, taken as what it resolves to; else nothing, since no word there is
/// one the guards judge. Where git could not be asked about its aliases, or more than
/// [`MAX_CHECKOUT_OPERANDS`] different aliases would have to be followed, the answer is UNREAD, and
/// refused. Every alias is read from the line's one [`AliasBook`], so the cost stays flat.
///
/// `any_word` is whether a config source the guard does not read may make ANY word an alias:
/// so after git itself, but not after a program only the shell can name, which may be no git
/// at all and whose every argument would be refused on a line that sets `HOME` (#1326) —
/// unless a git that may write came before it on the line.
///
/// `configured` is whether an earlier command on the line may have changed git's
/// configuration: then any word that is not git's own may be an alias, and is unread.
fn subcommand_after_splice(
    pre: &[String],
    rest: &[String],
    env: &[String],
    own_env: &[String],
    aliases: &AliasBook<'_>,
    any_word: bool,
    configured: bool,
) -> Option<(String, Vec<String>, bool)> {
    if let Some((sub, post)) = literal_mover_after(rest) {
        return Some((sub, post, false));
    }
    let first = || {
        rest.split_first()
            .map(|(sub, post)| (sub.clone(), post.to_vec()))
    };
    let moves = |sub: &str| BRANCH_MOVERS.contains(&sub) || sub == "reset";
    // The aliases the line itself defines, kept as the walk goes: a `-c alias.x=…` or
    // `--config-env` pair joins them once both its words are behind (built once rather than
    // per word), over what the environment defines (#1358).
    let mut inline = LineConfig::of(pre, env, own_env);
    inline.unsure_all |= configured;
    let mut prev: Option<&String> = pre.last();
    // What an alias is now: the line's body first, then the config's.
    let body_now = |inline: &LineConfig, name: &str| -> Result<String, AliasEnd> {
        let folded = name.to_lowercase();
        match inline.body(&folded) {
            Some(found) => found,
            None => aliases.body(&folded).ok_or(AliasEnd::Unasked),
        }
    };
    // Every alias chain already followed to no branch move, as the bodies it read: a word
    // whose chain reads the same bodies now answers the same again, so `st st st …` costs one
    // following, and redefining any alias on the chain makes it a new one.
    let mut cleared: Vec<Vec<(String, String)>> = Vec::new();
    for (k, word) in rest.iter().enumerate() {
        let folded = word.to_lowercase();
        let named = if any_word {
            inline.names(&folded, word)
        } else {
            inline.names_exactly(&folded)
        } || {
            let Some(book) = aliases.bodies() else {
                let (sub, post) = first().unwrap_or_default();
                return Some((sub, post, true));
            };
            book.iter().any(|(name, _)| *name == folded)
        };
        let known = || {
            cleared.iter().any(|chain| {
                chain.first().is_some_and(|(n, _)| *n == folded)
                    && chain
                        .iter()
                        .all(|(n, b)| body_now(&inline, n).as_ref() == Ok(b))
            })
        };
        if named && !known() {
            if cleared.len() >= MAX_CHECKOUT_OPERANDS {
                // Past the budget the rest is unread, never allowed.
                let (sub, post) = first().unwrap_or_default();
                return Some((sub, post, true));
            }
            let mut chain: Vec<(String, String)> = Vec::new();
            let (sub, post, end) = expand_alias(word, &rest[k + 1..], &mut |name| {
                let body = body_now(&inline, name);
                if let Ok(b) = &body {
                    chain.push((name.to_lowercase(), b.clone()));
                }
                body
            });
            if end != AliasEnd::Resolved {
                return Some((sub, post, true));
            }
            if moves(&sub) {
                return Some((sub, post, false));
            }
            cleared.push(chain);
        }
        inline.read_option(prev.map(String::as_str), word, own_env, env);
        prev = Some(word);
    }
    // No word after the substitution is one the guards judge, or an alias of one.
    None
}

/// The root's configured aliases for one command line: every `alias.*` read with ONE git
/// question, the first time a line needs one, so a line that names an alias a thousand times
/// asks git once (#1354). `None` inside is a question that could not be put.
struct AliasBook<'a> {
    root: &'a str,
    bodies: std::cell::OnceCell<Option<Vec<(String, String)>>>,
}

impl<'a> AliasBook<'a> {
    fn of(root: &'a str) -> Self {
        AliasBook {
            root,
            bodies: std::cell::OnceCell::new(),
        }
    }

    /// Every alias as `(name, body)`, names folded as git keys them, or `None` when git could
    /// not be asked.
    fn bodies(&self) -> Option<&[(String, String)]> {
        self.bodies
            .get_or_init(|| configured_aliases(self.root))
            .as_deref()
    }

    /// The body `git config --get alias.<name>` would answer — the last one set, `""` for none
    /// — or `None` when git could not be asked.
    fn body(&self, name: &str) -> Option<String> {
        let folded = name.to_lowercase();
        let book = self.bodies()?;
        Some(
            book.iter()
                .rev()
                .find(|(k, _)| *k == folded)
                .map(|(_, b)| py_strip(b).to_string())
                .unwrap_or_default(),
        )
    }
}

/// Every `alias.*` in the root's config, in order, as `(folded name, body)`: `git config -z
/// --get-regexp`, whose NUL-separated records keep a body's own newlines. `None` when git could
/// not be asked; a config with no alias is an empty list.
fn configured_aliases(root: &str) -> Option<Vec<(String, String)>> {
    let a = git_in(root, &["config", "-z", "--get-regexp", r"^alias\."]).ok()?;
    if !a.ok {
        return Some(Vec::new()); // exit 1: no key matched
    }
    Some(
        a.out
            .split('\0')
            .filter_map(|record| {
                let (key, body) = record.split_once('\n').unwrap_or((record, ""));
                let name = key.strip_prefix("alias.")?;
                Some((name.to_lowercase(), body.to_string()))
            })
            .collect(),
    )
}

/// The first literal branch mover or `reset` in `words`, with the words after it.
fn literal_mover_after(words: &[String]) -> Option<(String, Vec<String>)> {
    let at = words
        .iter()
        .position(|t| BRANCH_MOVERS.contains(&t.as_str()) || t == "reset")?;
    Some((words[at].clone(), words[at + 1..].to_vec()))
}

/// Whether the shell fills in git's command word or the name of one of git's own options
/// (`git $A …`, `git "$@"`, `git --$X …`): then which command git runs, and where, cannot be
/// read. An option's value is read where it stands (a directory, a configuration entry).
fn shell_fills_the_command(pre: &[String], rest: &[String]) -> bool {
    let mut i = 0usize;
    while i < pre.len() {
        let word = pre[i].as_str();
        let name = word.split_once('=').map_or(word, |(name, _)| name);
        if shell_fills(name) {
            return true;
        }
        i += if GIT_VALUE_OPTS.contains(&word) { 2 } else { 1 };
    }
    rest.first().is_some_and(|word| shell_fills(word))
}

/// `Path(config.ROOT).resolve()` — the string every subject is compared to.
pub fn plane_root(root: &str) -> String {
    resolved(root)
}

/// One git question could not be put: the process did not start, the deadline passed, an
/// argument held a NUL, or git wrote bytes that are not UTF-8 — Python's `ProcTimeout`,
/// `OSError` and `ValueError`.
#[derive(Debug)]
struct Unasked;

/// What one git question answered, as Python's `CompletedProcess` would hold it.
#[derive(Clone)]
struct Answer {
    ok: bool,
    out: String,
}

/// The most answers one guard's call keeps ([`Remembered`]). Past it git is asked again, so a
/// line that puts thousands of different questions costs time and not memory.
const MAX_REMEMBERED_ANSWERS: usize = 1024;

/// What git answered, by the root it was asked in and the arguments it was asked with.
type Answers = std::collections::HashMap<(String, Vec<String>), Answer>;

thread_local! {
    /// What git answered during the guard call running on this thread: `None` outside one
    /// ([`remembering`]).
    static REMEMBERED: std::cell::RefCell<Option<Answers>> =
        const { std::cell::RefCell::new(None) };
}

/// Ends the memory [`remembering`] began, however the call it wraps ends.
struct Remembered {
    began: bool,
}

impl Drop for Remembered {
    fn drop(&mut self) {
        if self.began {
            REMEMBERED.with(|r| *r.borrow_mut() = None);
        }
    }
}

/// `then`, with every git question it puts asked of git once (#2619).
///
/// A line repeats the same few questions — whether `HEAD` would lose commits, what the default
/// branch is, whether a word is a tracked path — once for each git on it, and each is a git
/// process: tens of milliseconds on a macOS runner, so 124 KB of the restores and ended
/// operations the linear-time test reads took the branch and reset guards 40 s there, in a
/// release build as in a debug one. Nothing runs between two questions of one guard call, so the
/// second is answered what the first was; that is all that changes. Only an answer git gave is
/// kept: a question that could not be asked is asked again, as before, so no refusal a later
/// attempt would have made is lost. The memory ends with the call, so the next command is read
/// against the repository as it is then.
fn remembering<T>(then: impl FnOnce() -> T) -> T {
    let began = REMEMBERED.with(|r| {
        let mut r = r.borrow_mut();
        let began = r.is_none();
        if began {
            *r = Some(std::collections::HashMap::new());
        }
        began
    });
    let _ends = Remembered { began };
    then()
}

/// One read-only git question about `root` — `doctor._git_in`, through the session-aware runner,
/// answered from what the guard call already asked where it can ([`remembering`]).
fn git_in(root: &str, args: &[&str]) -> Result<Answer, Unasked> {
    let key = (
        root.to_string(),
        args.iter().map(|a| (*a).to_string()).collect::<Vec<_>>(),
    );
    let known = REMEMBERED.with(|r| r.borrow().as_ref().and_then(|m| m.get(&key).cloned()));
    if let Some(known) = known {
        return Ok(known);
    }
    let answer = git_asked(root, args)?;
    REMEMBERED.with(|r| {
        if let Some(m) = r.borrow_mut().as_mut()
            && m.len() < MAX_REMEMBERED_ANSWERS
        {
            m.insert(key, answer.clone());
        }
    });
    Ok(answer)
}

/// [`git_in`]'s question, put to git.
fn git_asked(root: &str, args: &[&str]) -> Result<Answer, Unasked> {
    let raw = runner::run_as_session(
        std::path::Path::new(root),
        args,
        crate::doctor::CHECK_TIMEOUT,
    )
    .map_err(|_| Unasked)?;
    let code = raw.code.ok_or(Unasked)?;
    // `text=True` decodes BOTH streams strictly; either failing is the `ValueError`.
    let out = String::from_utf8(raw.out).map_err(|_| Unasked)?;
    String::from_utf8(raw.err).map_err(|_| Unasked)?;
    // ...and translates newlines universally.
    let out = out.replace("\r\n", "\n").replace('\r', "\n");
    Ok(Answer { ok: code == 0, out })
}

/// What `git checkout <op>` would make of an operand — `_checkout_operand_kind`'s five answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperandKind {
    Rev,
    Path,
    Both,
    Neither,
    /// git could not be asked — kept denied: a guard that opened because it failed to ask is the
    /// fail-open shape #438 is about.
    Unknown,
}

impl OperandKind {
    /// The Python's string for it.
    pub fn as_str(self) -> &'static str {
        match self {
            OperandKind::Rev => "rev",
            OperandKind::Path => "path",
            OperandKind::Both => "both",
            OperandKind::Neither => "neither",
            OperandKind::Unknown => "unknown",
        }
    }
}

/// What `git checkout <op>` would make of `op` in `root` — `_checkout_operand_kind` — **asked of
/// git**, never inferred from the spelling.
///
/// `rev-parse --verify --quiet <op>^{commit}` (does it resolve to a commit?) and `ls-files
/// --error-unmatch -- <op>` (does git TRACK it?). Only `Path` — git cannot read it as a commit
/// and can read it as a tracked path — lets a command through; `Both` is refused (git breaks the
/// tie for the ref), `Neither` is refused (a remote-only branch is DWIM-created), and `Unknown` is
/// refused. `-` is `@{-1}`, a ref, and is never handed to git, where it would read as an option.
pub fn checkout_operand_kind(root: &str, op: &str) -> OperandKind {
    if op == "-" {
        return OperandKind::Rev;
    }
    let spec = format!("{op}^{{commit}}");
    let rev = match git_in(root, &["rev-parse", "--verify", "--quiet", &spec]) {
        Ok(a) => a.ok,
        Err(_) => return OperandKind::Unknown,
    };
    let path = match git_in(root, &["ls-files", "--error-unmatch", "--", op]) {
        Ok(a) => a.ok,
        Err(_) => return OperandKind::Unknown,
    };
    match (rev, path) {
        (true, true) => OperandKind::Both,
        (true, false) => OperandKind::Rev,
        (false, true) => OperandKind::Path,
        (false, false) => OperandKind::Neither,
    }
}

/// Whether HEAD in `root` is already at the commit `target` names, so a checkout of it rewrites
/// no file (qw346, [`crate::rootupdate`]). `false` wherever git could not be asked or either
/// name resolves to no commit: a caller that cannot tell takes it that the tree would move.
pub fn head_is_at(root: &str, target: &str) -> bool {
    let commit = |name: &str| {
        let spec = format!("{name}^{{commit}}");
        git_in(root, &["rev-parse", "--verify", "--quiet", &spec])
            .ok()
            .filter(|a| a.ok)
            .map(|a| py_strip(&a.out).to_string())
            .filter(|oid| !oid.is_empty())
    };
    // `-` is the previous branch, and never handed to git, where it would read as an option.
    let target = if target == "-" { "@{-1}" } else { target };
    if target.starts_with('-') {
        return false;
    }
    match (commit("HEAD"), commit(target)) {
        (Some(head), Some(there)) => head == there,
        _ => false,
    }
}

/// Aliases defined ON THE COMMAND LINE: `git -c alias.co=checkout co feature` —
/// `_inline_aliases`. Only the separated `-c <name>=<value>`: git rejects the attached form.
/// Later definitions win, as a dict's later assignment does. The NAME is kept LOWER-CASED, git's
/// own rule for the last component of a config key, so `-c alias.CO=checkout co` is found (#346).
pub fn inline_aliases(pre: &[String]) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for j in 0..pre.len().saturating_sub(1) {
        if pre[j] != "-c" {
            continue;
        }
        let Some((name, body)) = pre[j + 1].split_once('=') else {
            continue;
        };
        if name.to_lowercase().starts_with("alias.") {
            let alias: String = name
                .chars()
                .skip("alias.".len())
                .collect::<String>()
                .to_lowercase();
            match out.iter_mut().find(|(k, _)| *k == alias) {
                Some(row) => row.1 = body.to_string(),
                None => out.push((alias, body.to_string())),
            }
        }
    }
    out
}

/// Follow git ALIASES to the subcommand that will really run — `_resolve_git_alias`.
///
/// Asked of git, for this repository, and only for a subcommand not already taken to be git's
/// own. The caller's arguments are appended to the expansion, exactly as git appends them, so
/// `sw = switch -c` plus `git sw neu` is judged as `switch -c neu`. A `!git …` body is read as
/// the git command it is; any other `!` body is a shell charter does not read, and stands aside.
/// An alias git would not answer for is "not an alias" — out of scope, not allowed.
pub fn resolve_git_alias(
    root: &str,
    sub: &str,
    post: &[String],
    pre: &[String],
) -> (String, Vec<String>) {
    let (sub, post, _) =
        resolve_git_alias_to_its_end(root, sub, post, pre, &LineEnv::default(), &[]);
    (sub, post)
}

/// [`resolve_git_alias`], and how the walk ended: a chain still on an alias when
/// [`MAX_ALIAS_HOPS`] ran out is [`AliasEnd::TooDeep`], which the branch guard refuses — the
/// end it cannot see may move HEAD.
///
/// `env` is the invocation's environment as the line sets it ([`RootInvocation::env`]) and
/// `own_env` its own assignments ([`RootInvocation::own_env`]): an alias they define, or one a
/// source they name could define, is read from them first (#1358, [`LineConfig`]).
pub fn resolve_git_alias_to_its_end(
    root: &str,
    sub: &str,
    post: &[String],
    pre: &[String],
    env: &LineEnv,
    own_env: &[String],
) -> (String, Vec<String>, AliasEnd) {
    // Read only for a word that may be an alias: a line of exports hands every git on it the
    // whole of them, and reading them for each `git status` cost the line its square (qw367).
    let mut line: Option<LineConfig> = None;
    expand_alias(sub, post, &mut |name| {
        let line = line.get_or_insert_with(|| LineConfig::of(pre, &env.to_vec(), own_env));
        if let Some(found) = line.body(&name.to_lowercase()) {
            return found;
        }
        let key = format!("alias.{name}");
        match git_in(root, &["config", "--get", &key]) {
            Ok(a) if a.ok => Ok(py_strip(&a.out).to_string()),
            Ok(_) => Ok(String::new()),
            Err(_) => Err(AliasEnd::Unasked),
        }
    })
}

/// The git configuration one invocation takes from its own command line and environment, as
/// far as aliases go (#1358): `-c`, `--config-env`, `GIT_CONFIG_COUNT` with its numbered pairs,
/// and `GIT_CONFIG_PARAMETERS`, each as the line sets it — the config FILES are git's to read,
/// and the guard asks git about them.
///
/// A source the guard does not read makes the line OPAQUE: any alias may be defined there. That
/// is a config file the line points git at (`GIT_CONFIG_GLOBAL`, `GIT_CONFIG_SYSTEM`,
/// `GIT_CONFIG`, `HOME`, `XDG_CONFIG_HOME`, an `include.path` or `includeIf.*.path`) — never
/// opened, because the same line may write it before git reads it — and a key, count or list
/// only the shell can name. An alias whose VALUE only the shell can name is unread on its own.
///
/// **Only the invocation's own assignments and git's own options surely reach git** (D-1358f).
/// A value an earlier segment sets may not — `unset`, `env -u`, `env -i`, a subshell, `false &&`
/// — and any SUBSET of them may be the one that does, so no single reading is safe to trust. The
/// guard reads the line twice, with every value and with the invocation's own alone, and an
/// alias the two readings disagree on — defined, redefined or shadowed by a value that may not
/// reach git — is UNREAD, as is every alias when only the fuller reading is opaque. A line value
/// can therefore only add refusals.
#[derive(Clone, Debug, Default)]
struct LineConfig {
    /// Folded alias name → body, or `None` for a body only the shell can name, as the
    /// invocation's own assignments and git's options set them. Later sources were inserted
    /// later, so the map holds the one git would use.
    aliases: std::collections::HashMap<String, Option<String>>,
    opaque: bool,
    /// Aliases an earlier segment's value defines or shadows: unread.
    unsure: std::collections::HashSet<String>,
    /// An earlier segment's value may define ANY alias: every alias is unread.
    unsure_all: bool,
    /// Aliases git's own options on the command define: the last word, whatever came before.
    argv: std::collections::HashSet<String>,
}

/// The environment variables that point git at a config file of their own choosing.
const CONFIG_FILE_ENV: [&str; 5] = [
    "GIT_CONFIG_GLOBAL",
    "GIT_CONFIG_SYSTEM",
    "GIT_CONFIG",
    "HOME",
    "XDG_CONFIG_HOME",
];

/// A value the line sets for an environment variable: the last assignment, `Some(None)` when
/// only the shell can name it, `None` when the line does not set it.
fn line_value(env: &[String], name: &str) -> Option<Option<String>> {
    env.iter().rev().find_map(|assign| {
        let (n, v) = assign.split_once('=')?;
        (n == name).then(|| (!shell_fills(v)).then(|| v.to_string()))
    })
}

/// Whether the shell, not the guard, decides what this word is: a parameter, a substitution.
fn shell_fills(word: &str) -> bool {
    word.contains(['$', '`'])
}

impl LineConfig {
    /// The configuration of an invocation whose environment may be `env` — everything the line
    /// set — and is surely `own_env`, with git's own options `pre`.
    fn of(pre: &[String], env: &[String], own_env: &[String]) -> Self {
        let mut own = Self::read(pre, own_env, env);
        let before: Vec<&String> = env.iter().filter(|a| !own_env.contains(a)).collect();
        own.mark_unsure(&before, env, own_env);
        own
    }

    /// Every alias a value from an earlier segment may define or shadow, read leniently: EVERY
    /// value each variable is given, not the last, because any subset of them may be the one
    /// that reaches git (D-1358f). A config file, an include, or a key, count or list only the
    /// shell can name may define any alias.
    fn mark_unsure(&mut self, before: &[&String], env: &[String], own_env: &[String]) {
        let mut all = false;
        let mut names: Vec<String> = Vec::new();
        let mut key = |key: &str, all: &mut bool| {
            if shell_fills(key) {
                *all = true;
                return;
            }
            let folded = key.to_lowercase();
            if folded == "include.path"
                || (folded.starts_with("includeif.") && folded.ends_with(".path"))
            {
                *all = true;
            } else if let Some(name) = folded.strip_prefix("alias.") {
                names.push(name.to_string());
            }
        };
        let own_sets = |prefix: &str| {
            own_env.iter().any(|a| {
                a.split_once('=')
                    .is_some_and(|(n, _)| n.starts_with(prefix))
            })
        };
        for assign in before {
            let Some((name, value)) = assign.split_once('=') else {
                continue;
            };
            if CONFIG_FILE_ENV.contains(&name) {
                all = true;
            } else if name == "GIT_CONFIG_PARAMETERS" {
                match (shell_fills(value), shellseg::posix_split(value)) {
                    (false, Ok(words)) => {
                        for word in words {
                            key(
                                word.split_once('=').map_or(word.as_str(), |(k, _)| k),
                                &mut all,
                            );
                        }
                    }
                    _ => all = true,
                }
            } else if name == "GIT_CONFIG_COUNT" {
                // A count the shell fills in, or one that turns the invocation's own pairs on
                // or off, may make any of them git's.
                if shell_fills(value)
                    || value.trim().parse::<usize>().is_err()
                    || (!own_sets("GIT_CONFIG_COUNT")
                        && (own_sets("GIT_CONFIG_KEY_") || own_sets("GIT_CONFIG_VALUE_")))
                {
                    all = true;
                }
            } else if name.starts_with("GIT_CONFIG_KEY_") {
                key(value, &mut all);
            } else if let Some(n) = name.strip_prefix("GIT_CONFIG_VALUE_") {
                // The value is an alias's body if any key at its index names one.
                let key_name = format!("GIT_CONFIG_KEY_{n}");
                let mut keys: Vec<String> = env
                    .iter()
                    .filter_map(|a| a.split_once('='))
                    .filter(|(k, _)| *k == key_name)
                    .map(|(_, v)| v.to_string())
                    .collect();
                keys.extend(std::env::var(&key_name).ok());
                if keys.is_empty() {
                    all = true;
                }
                for k in keys {
                    key(&k, &mut all);
                }
            }
        }
        for name in names {
            if !self.argv.contains(&name) {
                self.unsure.insert(name);
            }
        }
        self.unsure_all |= all;
    }

    /// The configuration `env` sets, with git's own options `pre`; `full` is the whole line's
    /// environment, which a `--config-env` variable is looked for in too.
    fn read(pre: &[String], env: &[String], full: &[String]) -> Self {
        let mut line = LineConfig::default();
        let process = |name: &str| std::env::var(name).ok();
        // The value an invocation sees: the line's, else the one this session started with.
        let value = |name: &str| match line_value(env, name) {
            Some(v) => v,
            None => process(name),
        };
        let sets = |prefix: &str| {
            env.iter().any(|a| {
                a.split_once('=')
                    .is_some_and(|(n, _)| n.starts_with(prefix))
            })
        };
        if CONFIG_FILE_ENV
            .iter()
            .any(|name| line_value(env, name).is_some())
        {
            line.opaque = true;
        }
        // The numbered pairs, read where the line touches them: the session's own are git's to
        // read, and the guard's git reads them.
        if sets("GIT_CONFIG_COUNT") || sets("GIT_CONFIG_KEY_") || sets("GIT_CONFIG_VALUE_") {
            match value("GIT_CONFIG_COUNT").map(|c| c.trim().parse::<usize>()) {
                Some(Ok(count)) if count <= MAX_CHECKOUT_OPERANDS * 4 => {
                    for n in 0..count {
                        match value(&format!("GIT_CONFIG_KEY_{n}")) {
                            Some(key) => {
                                // A value set nowhere the guard can see is unread, not empty.
                                let body = match line_value(env, &format!("GIT_CONFIG_VALUE_{n}")) {
                                    Some(v) => v,
                                    None => process(&format!("GIT_CONFIG_VALUE_{n}")),
                                };
                                line.entry(&key, body);
                            }
                            None => line.opaque = true,
                        }
                    }
                }
                Some(Ok(_)) | Some(Err(_)) => line.opaque = true,
                None => {
                    // A count only the shell can name, or pairs with no count to read them by.
                    if line_value(env, "GIT_CONFIG_COUNT").is_some() {
                        line.opaque = true;
                    }
                }
            }
        }
        match line_value(env, "GIT_CONFIG_PARAMETERS") {
            Some(Some(list)) => match shellseg::posix_split(&list) {
                Ok(words) => {
                    for word in words {
                        let (key, body) = match word.split_once('=') {
                            Some((k, v)) => (k.to_string(), v.to_string()),
                            None => (word.clone(), String::new()),
                        };
                        line.entry(&key, Some(body));
                    }
                }
                Err(_) => line.opaque = true,
            },
            Some(None) => line.opaque = true,
            None => {}
        }
        let mut prev: Option<&str> = None;
        for word in pre {
            line.read_option(prev, word, env, full);
            prev = Some(word);
        }
        line
    }

    /// Take in one of git's own options, given the word before it: `-c key=value`,
    /// `--config-env key=VAR` and `--config-env=key=VAR`.
    ///
    /// These are git's own words, the last say whatever the environment holds, so an alias they
    /// define is no longer unsure — unless its `--config-env` variable comes from an earlier
    /// segment, which may not reach git.
    fn read_option(&mut self, prev: Option<&str>, word: &str, env: &[String], full: &[String]) {
        let config_env = |this: &mut Self, spec: &str| {
            let Some((key, var)) = spec.split_once('=') else {
                return;
            };
            // git reads VAR from the environment it is given; a VAR the invocation does not set
            // is the session's, unless an earlier segment sets it — then it may not reach git —
            // and one the session does not hold either is one the guard cannot see.
            let body = match line_value(env, var) {
                Some(v) => v,
                None if line_value(full, var).is_some() => None,
                None => std::env::var(var).ok(),
            };
            this.option_entry(key, body);
        };
        match prev {
            Some("-c") => {
                let (key, body) = match word.split_once('=') {
                    Some((k, v)) => (k, Some(v.to_string())),
                    None => (word, Some(String::new())),
                };
                let body = body.filter(|b| !shell_fills(b));
                self.option_entry(key, body);
            }
            Some("--config-env") => config_env(self, word),
            _ => {
                if let Some(spec) = word.strip_prefix("--config-env=") {
                    config_env(self, spec);
                }
            }
        }
    }

    /// An entry git's own options make: the last word on that alias.
    fn option_entry(&mut self, key: &str, body: Option<String>) {
        self.entry(key, body);
        if let Some(name) = key.to_lowercase().strip_prefix("alias.") {
            self.argv.insert(name.to_string());
            self.unsure.remove(name);
        }
    }

    /// One configuration entry: an alias, a file to include, or nothing the guard reads.
    fn entry(&mut self, key: &str, body: Option<String>) {
        if shell_fills(key) {
            self.opaque = true;
            return;
        }
        let folded = key.to_lowercase();
        if folded == "include.path"
            || (folded.starts_with("includeif.") && folded.ends_with(".path"))
        {
            self.opaque = true;
        } else if let Some(name) = folded.strip_prefix("alias.") {
            self.aliases.insert(name.to_string(), body);
        }
    }

    /// The body the line gives the alias `folded`: `Some(Ok(body))`, `Some(Err(Unread))` when
    /// only the shell or an unread file can say, `None` when the config files are git's to ask.
    fn body(&self, folded: &str) -> Option<Result<String, AliasEnd>> {
        if !self.argv.contains(folded) && (self.unsure_all || self.unsure.contains(folded)) {
            return Some(Err(AliasEnd::Unread));
        }
        match self.aliases.get(folded) {
            Some(Some(body)) => Some(Ok(body.clone())),
            Some(None) => Some(Err(AliasEnd::Unread)),
            None if self.opaque => Some(Err(AliasEnd::Unread)),
            None => None,
        }
    }

    /// Whether the line names `folded` as an alias, or a value it sets may: never a word only
    /// because a config file the line points git at may define any alias.
    fn names_exactly(&self, folded: &str) -> bool {
        self.aliases.contains_key(folded) || self.unsure.contains(folded)
    }

    /// Whether `word` may be an alias the line defines: one it names, or, on an opaque line, any
    /// word that is not git's own.
    fn names(&self, folded: &str, word: &str) -> bool {
        self.aliases.contains_key(folded)
            || self.unsure.contains(folded)
            || ((self.opaque || self.unsure_all)
                && !BRANCH_MOVERS.contains(&word)
                && !GIT_KNOWN_SUBCOMMANDS.contains(&word))
    }
}

/// How following an alias ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AliasEnd {
    /// At a word that is no alias, a loop, or a body charter does not read.
    Resolved,
    /// A body git could not be asked for.
    Unasked,
    /// Still on an alias when [`MAX_ALIAS_HOPS`] ran out.
    TooDeep,
    /// Defined, or possibly defined, where the guard does not read: a config file the command
    /// names, or a value only the shell can name (#1358).
    Unread,
}

/// [`resolve_git_alias`]'s hops, with each alias's body asked of `body`: `Ok(body)` (`""` for
/// none), or the end a body that could not be read makes — [`AliasEnd::Unasked`] or
/// [`AliasEnd::Unread`] — which stops the walk where it stands.
fn expand_alias(
    sub: &str,
    post: &[String],
    body: &mut dyn FnMut(&str) -> Result<String, AliasEnd>,
) -> (String, Vec<String>, AliasEnd) {
    let mut sub = sub.to_string();
    let mut post = post.to_vec();
    let mut seen: Vec<String> = Vec::new();
    let settled = |sub: &str, seen: &[String]| {
        BRANCH_MOVERS.contains(&sub)
            || GIT_KNOWN_SUBCOMMANDS.contains(&sub)
            || seen.iter().any(|s| s == sub)
    };
    for _ in 0..MAX_ALIAS_HOPS {
        if settled(&sub, &seen) {
            return (sub, post, AliasEnd::Resolved);
        }
        seen.push(sub.clone());
        let text = match body(&sub) {
            Ok(text) => text,
            Err(end) => return (sub, post, end),
        };
        let Some((first, tail)) = alias_words(&text) else {
            return (sub, post, AliasEnd::Resolved);
        };
        let mut next = tail;
        next.extend(post);
        sub = first;
        post = next;
    }
    // The hops ran out: still on an alias is a chain whose end is out of sight.
    let end = if settled(&sub, &seen) {
        AliasEnd::Resolved
    } else {
        match body(&sub) {
            Err(end) => end,
            Ok(text) if alias_words(&text).is_some() => AliasEnd::TooDeep,
            Ok(_) => AliasEnd::Resolved,
        }
    };
    (sub, post, end)
}

/// The git words an alias body expands to — its first word and the rest — or `None` for an
/// empty body, one that does not split, or a `!` body that runs something other than git.
fn alias_words(body: &str) -> Option<(String, Vec<String>)> {
    if body.is_empty() {
        return None;
    }
    let shell = body.starts_with('!');
    let text = if shell { &body[1..] } else { body };
    // A `!` alias is run by `sh -c`, so it is read as the shell reads it.
    let split = if shell {
        shellseg::shell_split(text)
    } else {
        shellseg::posix_split(text)
    };
    let mut toks = split.ok()?;
    if shell {
        if toks.first().map(|t| shellwrap::base_lower(t)).as_deref() != Some("git") {
            return None;
        }
        toks.remove(0);
    }
    let (first, tail) = toks.split_first()?;
    Some((first.clone(), tail.to_vec()))
}

/// The name a branch-creating `checkout`/`switch` would create, for the DENIAL TEXT —
/// `_created_branch`. The name can be inside the option token (`-bREADME`, `--orphan=README`),
/// where the operand list is empty.
pub fn created_branch(opts: &[String], classes: &[OptKind], wants: &[String]) -> Option<String> {
    let first_want = || wants.first().cloned();
    let Some(first) = opts
        .iter()
        .zip(classes)
        .find(|(_, c)| **c == OptKind::Create)
        .map(|(o, _)| o)
    else {
        return first_want();
    };
    if first.starts_with("--") {
        if let Some((_, value)) = first.split_once('=') {
            return if value.is_empty() {
                None
            } else {
                Some(value.to_string())
            };
        }
    } else {
        let chars: Vec<char> = first.chars().collect();
        for (i, ch) in chars.iter().enumerate().skip(1) {
            if BRANCH_CREATOR_OPTS.contains(&format!("-{ch}").as_str()) {
                let attached: String = chars[i + 1..].iter().collect();
                return if attached.is_empty() {
                    first_want()
                } else {
                    Some(attached)
                };
            }
        }
    }
    first_want()
}

/// This repository's default branch, or `None` — `doctor._plane_default_branch`: the remote's
/// own `origin/HEAD` first, then a local `main` or `master`. `Err` is a question that could not
/// be asked, which the caller treats as no default.
fn plane_default_branch(root: &str) -> Result<Option<String>, Unasked> {
    let head = git_in(
        root,
        &[
            "symbolic-ref",
            "--quiet",
            "--short",
            "refs/remotes/origin/HEAD",
        ],
    )?;
    let remote_head = py_strip(&head.out);
    if head.ok && !remote_head.is_empty() {
        // `--short` renders it as `origin/main`, not `main`.
        return Ok(Some(match remote_head.strip_prefix("origin/") {
            Some(rest) => rest.to_string(),
            None => remote_head.to_string(),
        }));
    }
    for guess in ["main", "master"] {
        let refname = format!("refs/heads/{guess}");
        if git_in(root, &["rev-parse", "--verify", "--quiet", &refname])?.ok {
            return Ok(Some(guess.to_string()));
        }
    }
    Ok(None)
}

/// [`plane_default_branch`] as the branch guard reads it: a question that could not be asked is
/// no default. Python catches `ProcTimeout`/`OSError` there and RAISES on a name that is not
/// UTF-8 (charter#1178); both are no default here, which withholds the remedy's carve-out — the
/// refusing direction.
pub fn default_branch(root: &str) -> Option<String> {
    plane_default_branch(root).unwrap_or(None)
}

/// Deny a git command that would move the PLANE ROOT between branches (#157) —
/// `_plane_root_branch_reason`. `root` is the plane root as charter's config holds it.
///
/// Four things keep it a guard rather than a cage: only branch moves; only the root; the
/// documented remedy (`git checkout <default>`, leaving HEAD ATTACHED to the default branch)
/// stays runnable; and a file restore is not a branch move (#461) — resolved by asking git, and
/// only an affirmative "a tracked path and not a revision", with every option one charter can
/// place as restore-only, opens the gate.
pub fn plane_root_branch_reason(cmd: &str, cwd: &str, root: &str) -> Option<String> {
    remembering(|| branch_reason(cmd, cwd, root))
}

/// [`plane_root_branch_reason`], each git question asked once.
fn branch_reason(cmd: &str, cwd: &str, root: &str) -> Option<String> {
    let root = plane_root(root);
    let (invocations, too_deep) = read_line(cmd, cwd, &root);
    if too_deep {
        // A git in a string handed on past what the walk reads may move the root (#1325).
        return Some(guardcaps::too_deep_refusal());
    }
    for inv in invocations {
        if let Some(why) = branch_move_reason(&inv, &root) {
            return Some(why);
        }
        // Read as nothing the guard refuses, but the words git reads are not the ones on the
        // line: refused all the same, with the spelling purlis reads.
        if inv.command_filled {
            return Some(format!(
                "cannot tell what this `git` command does: the shell fills in its command word \
                 (a variable, a function's arguments), so a word after it that reads as a \
                 command may be an option or an operand of another. Spell the command out and \
                 purlis checks it. {}",
                root_tail(default_branch(&root).as_deref())
            ));
        }
        if inv.unquoted_substitution {
            return Some(format!(
                "cannot tell which repository this `git` command acts on, or which command it \
                 runs: a command substitution among git's own options is not quoted, so what it \
                 prints is split into words git reads as options and as its command. Quote it \
                 and purlis reads it as one word: `git -C \"$(…)\" …`. {}",
                root_tail(default_branch(&root).as_deref())
            ));
        }
    }
    None
}

/// What [`plane_root_branch_reason`] says of one invocation, or `None`.
fn branch_move_reason(inv: &RootInvocation, root: &str) -> Option<String> {
    let root = root.to_string();
    if inv.unread && inv.program_unnamed {
        return Some(format!(
            "cannot tell what this command does: its program is named only when the shell \
                 runs it (a variable, a command substitution), so it may be git, and purlis \
                 could not work out which of the words after it git would run as its command — \
                 a guard that opened because it could not tell is no guard. Spell the program \
                 out and purlis checks it: `git -C <path> …` and `cd <path> && git …` are both \
                 read. {}",
            root_tail(default_branch(&root).as_deref())
        ));
    }
    if inv.unread {
        return Some(format!(
            "cannot tell what this `git` command does: a variable or command substitution \
                 in its command's place or among its options hides which repository it acts \
                 on, and purlis could not work out which of the words after it git would run \
                 as its command — a guard that opened because it could not tell is no guard. \
                 Spell the command and the path out and purlis checks them: `git -C <path> …` \
                 and `cd <path> && git …` are both read. {}",
            root_tail(default_branch(&root).as_deref())
        ));
    }
    let (mut sub, mut post) = (inv.sub.clone(), inv.post.clone());
    if !BRANCH_MOVERS.contains(&sub.as_str()) {
        // `co = checkout` moves the root's HEAD exactly as far (#461, round two).
        let end;
        (sub, post, end) = inv.resolved(&root);
        if end == AliasEnd::Unread && inv.after_a_writer {
            return Some(format!(
                "cannot tell what `git {}` does: it may be an alias, and an earlier command \
                     on the line may change git's configuration before it runs, so where the \
                     alias leads is out of sight. Run it as a command of its own and purlis \
                     reads it, or run the command the alias stands for. {}",
                inv.sub,
                root_tail(default_branch(&root).as_deref())
            ));
        }
        if end == AliasEnd::Unread {
            return Some(format!(
                "cannot tell what `git {}` does: it may be an alias, and this command sets \
                     git configuration purlis does not read — a config file it names, a value \
                     only the shell fills in, or one an earlier command on the line sets, which \
                     may not reach git — so where the alias leads is out of sight. \
                     Run the command the alias stands for, or leave that setting off. {}",
                inv.sub,
                root_tail(default_branch(&root).as_deref())
            ));
        }
        if end == AliasEnd::TooDeep {
            // The same confusion as any named denial's (#1093): a loose `cd` before it.
            let hint = if inv.after_loose_cd && !inv.unnamed {
                LOOSE_CD_HINT
            } else {
                ""
            };
            return Some(format!(
                "cannot tell what `git {}` does in the PLANE ROOT: it is an alias that leads \
                     to another alias more than {MAX_ALIAS_HOPS} times, past where purlis \
                     follows them, and the end of a chain it cannot see may move HEAD. Run the \
                     command it stands for instead. {hint}{}",
                inv.sub,
                root_tail(default_branch(&root).as_deref())
            ));
        }
        if sub == "symbolic-ref" {
            if let Some(to) = head_pointed_at(&post) {
                let hint = if inv.unnamed {
                    "It may be the PLANE ROOT: a directory it is sent to is named only when \
                         the shell runs it. "
                } else if inv.after_loose_cd {
                    LOOSE_CD_HINT
                } else {
                    ""
                };
                return Some(format!(
                    "would point HEAD in the PLANE ROOT at '{to}': `git symbolic-ref HEAD \
                         <ref>` moves the root to another branch without touching a file, so \
                         the working tree no longer matches the branch it is on. {hint}{}",
                    root_tail(default_branch(&root).as_deref())
                ));
            }
            return None;
        }
        if !BRANCH_MOVERS.contains(&sub.as_str()) {
            return None;
        }
    }
    // Every option classified before anything reads an operand: what an operand MEANS
    // depends on the options beside it.
    let opts: Vec<String> = post
        .iter()
        .filter(|a| a.starts_with('-') && *a != "-" && *a != "--")
        .cloned()
        .collect();
    let classes: Vec<OptKind> = opts.iter().map(|o| checkout_opt_kind(o)).collect();
    let creating = classes.contains(&OptKind::Create);
    let detaching = classes.contains(&OptKind::Detach);
    let restore_shaped = classes.iter().all(|c| *c == OptKind::Restore);
    let unplaced = opts
        .iter()
        .zip(&classes)
        .find(|(_, c)| **c == OptKind::Unknown)
        .map(|(o, _)| o.clone());

    // What FOLLOWS `--` makes a path form, not its presence; and `git switch` has no path
    // half for a `--` to introduce, so only `checkout` reads it this way.
    let separator = post.iter().position(|a| a == "--");
    if let (true, Some(cut)) = (sub == "checkout", separator) {
        if cut + 1 < post.len() {
            if restore_shaped {
                return None; // paths follow: a restore, HEAD does not move
            }
        } else {
            post.truncate(cut); // a trailing bare `--` still switches
        }
    }
    // A bare `-` is a REF (the previous branch), not a flag.
    let wants: Vec<String> = post
        .iter()
        .filter(|a| *a == "-" || !a.starts_with('-'))
        .cloned()
        .collect();
    if wants.is_empty() && !(creating || detaching) && restore_shaped {
        return None; // bare `git checkout` moves nothing
    }

    let default = default_branch(&root);
    // The documented remedy stays runnable — and what makes a command the remedy is that it
    // leaves HEAD ATTACHED to the default branch, not that the name is beside it.
    if restore_shaped
        && default
            .as_deref()
            .is_some_and(|d| wants.first().map(String::as_str) == Some(d))
    {
        return None;
    }

    let mut kind = OperandKind::Rev;
    // A file restore read off what the repository holds before the line ran: an earlier
    // command on it that may write may make the word a branch first.
    let mut unmeasured = false;
    if let (true, Some(first)) = (sub == "checkout" && restore_shaped, wants.first()) {
        kind = checkout_operand_kind(&root, first);
        let restore = if wants.len() == 1 {
            kind == OperandKind::Path
        } else {
            // Every trailing operand RESOLVED, not assumed a path: the tokeniser flattens
            // `git checkout $(echo feature)` into several tokens.
            let rest = &wants[1..];
            rest.len() <= MAX_CHECKOUT_OPERANDS
                && rest
                    .iter()
                    .all(|w| checkout_operand_kind(&root, w) == OperandKind::Path)
        };
        if restore && !inv.after_a_writer {
            return None; // `git restore <path>`, spelled the old way
        }
        unmeasured = restore;
    }

    let want0 = wants.first().cloned().unwrap_or_default();
    let opening = if inv.unnamed {
        format!(
            "cannot tell which repository this `git {sub}` acts on: a directory it is sent \
                 to is named only when the shell runs it (a variable, a command substitution, a \
                 glob, `~user`, `cd -`), so it may be the PLANE ROOT. Spell the path out and \
                 purlis checks it: `git -C <path>` and `cd <path> && git …` are both read. "
        )
    } else if unmeasured {
        format!(
            "cannot tell whether this `git checkout` restores a file or switches branches \
                 in the PLANE ROOT: an earlier command on the line may change the repository \
                 before it runs, and what '{want0}' names with it. Run the restore as a command \
                 of its own and purlis reads it, or spell it `git restore {want0}` or \
                 `git checkout -- {want0}`, which purlis allows here. "
        )
    } else {
        match kind {
            OperandKind::Both => format!(
                "cannot tell what `git checkout {want0}` does in the PLANE ROOT — it is \
                 AMBIGUOUS: '{want0}' is both a tracked path here and a name git \
                 resolves to a commit, so it could be a file restore or a ref move, and \
                 git breaks that tie in favour of the REF — this would switch the root. \
                 Say which you meant and it runs: `git restore {want0}` (or \
                 `git checkout -- {want0}`) restores the file, and purlis allows that \
                 here in either spelling. "
            ),
            OperandKind::Neither => format!(
                "would move HEAD in the PLANE ROOT: '{want0}' is not a path this tree \
                 tracks, so `git checkout` reads it as a revision — and a branch of that \
                 name on a remote is checked out here as a new local branch. (Meant the \
                 file? `git restore {want0}` is allowed, and would tell you git has \
                 never heard of that path either.) "
            ),
            OperandKind::Unknown => format!(
                "would move HEAD in the PLANE ROOT — purlis could not ask git whether \
                 '{want0}' is a path or a revision here, and a guard that opened \
                 because it failed to ask is no guard. "
            ),
            _ => match &unplaced {
                Some(unplaced) if !creating && !detaching => {
                    let hint = if wants.is_empty() {
                        String::new()
                    } else {
                        format!(
                            "(A restore needs no options here: `git restore {want0}` and \
                             `git checkout -- {want0}` are both allowed.) "
                        )
                    };
                    format!(
                        "cannot read this `git {sub}` as a file restore in the PLANE ROOT: it does \
                         not recognise the option '{unplaced}', and an option purlis cannot place \
                         is one that may move HEAD — `git checkout --orphan <file>` reads exactly \
                         like a restore and creates a branch. Only a form purlis can show is a \
                         restore opens that gate. {hint}"
                    )
                }
                // `git checkout HEAD` with its paths out of sight: still read as a move,
                // and told the spelling that cannot move HEAD (qw346).
                _ if sub == "checkout"
                    && !creating
                    && !detaching
                    && wants.len() == 1
                    && matches!(want0.as_str(), "HEAD" | "@") =>
                {
                    format!(
                        "cannot read this `git checkout {want0}` as a file restore in the \
                         PLANE ROOT: no path follows a `--` on the line, and paths git is handed \
                         from a file or by `xargs` are out of purlis's sight, so '{want0}' reads \
                         as a ref to move to. To put files back as the current commit has them, \
                         use the spelling that cannot move HEAD, which purlis allows here: \
                         `git restore --source=HEAD --worktree -- <paths>`, or with \
                         `--pathspec-from-file=<file>` in place of the paths. "
                    )
                }
                _ => {
                    let created = if creating {
                        created_branch(&opts, &classes, &wants).filter(|c| !c.is_empty())
                    } else {
                        None
                    };
                    let moving = if let Some(created) = created {
                        format!("create '{created}'")
                    } else if creating {
                        "create a branch".to_string()
                    } else if detaching && !wants.is_empty() {
                        format!("detach HEAD at '{want0}'")
                    } else if detaching {
                        "detach HEAD".to_string()
                    } else if !wants.is_empty() {
                        format!("switch to '{want0}'")
                    } else {
                        "switch branches".to_string()
                    };
                    format!("would {moving} in the PLANE ROOT. ")
                }
            },
        }
    };
    let hint = if inv.after_loose_cd && !inv.unnamed {
        LOOSE_CD_HINT
    } else {
        ""
    };
    Some(format!("{opening}{hint}{}", root_tail(default.as_deref())))
}

/// The ref a `git symbolic-ref <post…>` points HEAD at, or `None` for one that reads a symbolic
/// ref, deletes one, or sets one other than HEAD. A name only the shell can name may be HEAD.
fn head_pointed_at(post: &[String]) -> Option<String> {
    let mut words: Vec<&String> = Vec::new();
    let mut i = 0usize;
    while i < post.len() {
        match post[i].as_str() {
            "-d" | "--delete" => return None,
            "-m" => i += 1, // its reason
            w if w.starts_with('-') => {}
            _ => words.push(&post[i]),
        }
        i += 1;
    }
    match words.as_slice() {
        // Folded, as a volume that folds case reads `head` as `HEAD`.
        [name, to, ..] if name.eq_ignore_ascii_case("HEAD") || shell_fills(name) => {
            Some(to.to_string())
        }
        _ => None,
    }
}

/// Where branch work goes, as every branch-guard denial says it (#3798).
const BRANCH_WORK: &str = "Branch work belongs in a branch folder of the workspace's clone: \
     `purlis worktree add <repo> <piece>`, then commit and push from it with `purlis worktree \
     commit` and `purlis worktree push`. A new task gets its own workspace: `purlis workspace \
     create <task>`, then `purlis clone <repo>`.";

/// What every branch-guard denial ends with: why the root is guarded, where branch work goes,
/// and the remedy that stays runnable — naming the default branch when there is one. Branch
/// work on a repo the workspace has already cloned goes to a branch folder purlis cuts, which a
/// sandboxed chat commits and pushes through purlis too; a new workspace is for a new task
/// (#3798).
fn root_tail(default: Option<&str>) -> String {
    let back = match default.filter(|d| !d.is_empty()) {
        Some(d) => format!(
            "`git checkout {d}` — putting the root back on its default branch — is always \
             allowed."
        ),
        None => "Putting the root back on its default branch is always allowed.".to_string(),
    };
    format!(
        "The plane root is one working tree every session \
         shares — two agents here silently clobber each other's branches, and the \
         symptom looks like an unrelated bug. {BRANCH_WORK} {back}"
    )
}

/// `(commits destroyed, upstream ref)` if resetting `root` to `target` would take commits off the
/// branch that exist NOWHERE ELSE — `_unpushed_at_risk`.
///
/// One question: `git rev-list --count HEAD --not <target> @{upstream} --`. `git reset --hard
/// HEAD` counts 0; a synced root counts 0; a path operand fails the whole call (the `--` marks
/// everything before it as revisions, and `@{upstream}` cannot be a path). `None` on any
/// non-zero exit — also the honest answer for a root with no tracking branch — and on any
/// question that could not be asked. Never panics.
pub fn unpushed_at_risk(root: &str, target: &str) -> Option<(u64, String)> {
    let r = git_in(
        root,
        &[
            "rev-list",
            "--count",
            "HEAD",
            "--not",
            target,
            "@{upstream}",
            "--",
        ],
    )
    .ok()?;
    if !r.ok {
        return None;
    }
    // `int(r.stdout.strip() or 0)` — a count git did not write is `ValueError`, i.e. `None`.
    let text = py_strip(&r.out);
    let n: i128 = if text.is_empty() { 0 } else { py_int(text)? };
    if n <= 0 {
        return None;
    }
    let up = git_in(root, &["rev-parse", "--abbrev-ref", "@{upstream}"]).ok()?;
    let name = if up.ok {
        py_strip(&up.out).to_string()
    } else {
        String::new()
    };
    let name = if name.is_empty() {
        "its upstream".to_string()
    } else {
        name
    };
    Some((u64::try_from(n).unwrap_or(u64::MAX), name))
}

/// `int(text)` for the digits `git rev-list --count` writes: an optional sign, then ASCII digits
/// with `_` allowed BETWEEN them, as Python's `int` reads a string. Anything else is `None` — the
/// `ValueError` the caller turns into "nothing at risk".
fn py_int(text: &str) -> Option<i128> {
    let (neg, digits) = match text.strip_prefix('-') {
        Some(d) => (true, d),
        None => (false, text.strip_prefix('+').unwrap_or(text)),
    };
    if digits.is_empty()
        || digits.starts_with('_')
        || digits.ends_with('_')
        || digits.contains("__")
        || !digits.chars().all(|c| c.is_ascii_digit() || c == '_')
    {
        return None;
    }
    let n: i128 = digits.replace('_', "").parse().ok()?;
    Some(if neg { -n } else { n })
}

/// Deny a `git reset` or `git update-ref` that would take unpushed commits off the plane root's
/// branch (#401) — `_plane_root_reset_reason`. `root` is the plane root as charter's config
/// holds it.
///
/// A reset in any mode that moves HEAD, and a ref update of HEAD or the branch the root is on:
/// `--soft` and `--mixed` keep the files, but the commits are left with the reflog as their
/// only copy, and the next reset loses what the working tree kept. Only a command that
/// measurably drops something unpublished ([`unpushed_at_risk`] is the condition), and the
/// denial clears itself the moment `charter save` pushes the commits. A command whose answer
/// cannot be measured — an earlier command on the line that may write changes what it is
/// measured against, a commit only the shell can name on a branch with unpushed commits, refs
/// read from standard input — is refused as unknown. It follows ALIASES as the branch guard
/// does (#467), so its hot-path filter is `git`, never `reset`: `wipe = reset --hard` destroys
/// commits without the word. The filter folds and reads through quoting
/// ([`shellwrap::may_name`]), because the walk behind it does: `GIT` and `g''it` are git.
pub fn plane_root_reset_reason(cmd: &str, cwd: &str, root: &str) -> Option<String> {
    if !shellwrap::may_name(cmd, "git") {
        return None;
    }
    remembering(|| reset_reason(cmd, cwd, root))
}

/// [`plane_root_reset_reason`], each git question asked once.
fn reset_reason(cmd: &str, cwd: &str, root: &str) -> Option<String> {
    let root = plane_root(root);
    let on = std::cell::OnceCell::new();
    let branch_on = || on.get_or_init(|| current_branch(&root)).clone();
    for inv in plane_root_git(cmd, cwd, &root) {
        let (sub, post) = if matches!(inv.sub.as_str(), "reset" | "update-ref") {
            (inv.sub.clone(), inv.post.clone())
        } else {
            // Any reading that is a reset is the one measured (#1358).
            let (s, p, _) = inv.resolved(&root);
            (s, p)
        };
        let lands = match sub.as_str() {
            "reset" => reset_lands_on(&post),
            "update-ref" => ref_update_lands_on(&post, inv.after_a_writer, &branch_on),
            _ => None,
        };
        let (target, tree) = match lands {
            None => continue,
            Some(Lands::Unread) => return Some(REFS_FROM_INPUT.to_string()),
            Some(Lands::On(target, tree)) => (target, tree),
        };
        let hint = if inv.after_loose_cd && !inv.unnamed {
            format!(" {}", LOOSE_CD_HINT.trim_end())
        } else {
            String::new()
        };
        let filled = shell_fills(&target);
        // A commit only the shell can name may be any of them: measured as the furthest back.
        let measured = if filled {
            "@{upstream}"
        } else {
            target.as_str()
        };
        // What is at risk as the repository stands is said as measured; where nothing is, an
        // earlier command on the line that may write may still put something at risk.
        let Some((n, upstream)) = unpushed_at_risk(&root, measured) else {
            if inv.after_a_writer && !matches!(target.as_str(), "HEAD" | "@") {
                return Some(format!(
                    "cannot tell what this `git {sub}` would take off the PLANE ROOT's branch: an \
                     earlier command on the line may change the repository before it runs — the \
                     commit HEAD is at, the commits no upstream has, what '{target}' names — and \
                     purlis measures it against the repository as it stands. Run it as a command of \
                     its own and purlis measures it.{hint}"
                ));
            }
            continue;
        };
        let (commits, are) = if n == 1 {
            ("commit", "is")
        } else {
            ("commits", "are")
        };
        let loses = if tree {
            "overwriting the working tree"
        } else {
            "leaving them with the reflog as their only copy"
        };
        if inv.unnamed {
            return Some(format!(
                "cannot tell which repository this `git {sub}` acts on: a directory it is sent \
                 to is named only when the shell runs it (a variable, a command substitution, a \
                 glob, `~user`, `cd -`), so it may be the PLANE ROOT — and there it may take \
                 {n} {commits} that {are} not on {upstream} off the branch, {loses}. Spell \
                 the path out and purlis checks it: `git -C <path>` and `cd <path> && git …` are \
                 both read."
            ));
        }
        let keep = format!(
            "See exactly what would go: `git -C {root} log --oneline '@{{upstream}}..HEAD'`. \
             Keep it: `purlis save` pushes it, and this {sub} stops being refused the moment it \
             lands.{hint}"
        );
        if filled {
            return Some(format!(
                "cannot tell what this `git {sub}` would take off the PLANE ROOT's branch: \
                 '{target}' is named only when the shell runs it, and the branch holds {n} \
                 {commits} that {are} not on {upstream}. Spell the commit out and purlis \
                 measures it. {keep}"
            ));
        }
        if !tree {
            return Some(format!(
                "would take {n} {commits} that {are} not on {upstream} off the PLANE ROOT's \
                 branch: the working tree keeps the files, but the commits are left with the \
                 reflog as their only copy, and the next reset or checkout loses them. An \
                 unpushed commit here is usually a memory commit whose push a protected branch \
                 refused. {keep}"
            ));
        }
        return Some(format!(
            "would delete {n} {commits} from the PLANE ROOT that {are} \
             not on {upstream}, and this reset overwrites the working tree — their content \
             leaves the disk with the reflog as the only copy. An unpushed commit here is \
             usually a memory commit whose push a protected branch refused, which is how \
             eleven of them were lost. See exactly what would go: \
             `git -C {root} log --oneline '@{{upstream}}..HEAD'`. Keep it: `purlis save` \
             pushes it, and this reset stops being refused the moment it lands.{hint}"
        ));
    }
    None
}

/// Where one judged command leaves the root's branch.
enum Lands {
    /// At this commit, and whether it overwrites the working tree as it goes.
    On(String, bool),
    /// Where refs read from standard input say.
    Unread,
}

/// What a `git update-ref` reading its refs from standard input is told.
const REFS_FROM_INPUT: &str = "cannot tell what this `git update-ref --stdin` does in the PLANE \
     ROOT: the refs it moves are read from its input, out of purlis's sight, and moving the \
     root's branch may take commits no upstream has off it. Name the ref and the commit on the \
     line and purlis measures it.";

/// The commit a `git reset <post…>` moves HEAD to, and whether it overwrites the working tree:
/// `None` for one that moves no HEAD — no commit named, or a path form (`<commit> -- <paths>`,
/// `<commit> <paths>`, `--patch`, `--pathspec-from-file`).
fn reset_lands_on(post: &[String]) -> Option<Lands> {
    let tree = post.iter().any(|a| RESET_TREE_MODES.contains(&a.as_str()));
    let mut post = post.to_vec();
    // What FOLLOWS `--` makes it a path form (`git reset <ref> -- <paths>` moves no HEAD).
    if let Some(cut) = post.iter().position(|a| a == "--") {
        if cut + 1 < post.len() {
            return None;
        }
        post.truncate(cut);
    }
    // A mode that keeps the tree has path forms of its own; a tree mode is judged as it was.
    if !tree
        && post
            .iter()
            .any(|a| a == "-p" || a == "--patch" || a.starts_with("--pathspec-from-file"))
    {
        return None;
    }
    let operands: Vec<&String> = post.iter().filter(|a| !a.starts_with('-')).collect();
    // `git reset --hard` with no ref moves HEAD nowhere: no commit dies.
    let target = operands.first()?;
    if !tree && operands.len() > 1 {
        return None; // `git reset <commit> <paths…>` resets the index only
    }
    Some(Lands::On(target.to_string(), tree))
}

/// The commit a `git update-ref <post…>` moves HEAD or the root's branch to — `@{upstream}`
/// for one that deletes it, which takes every unpushed commit — or `None` for one that moves
/// another ref. `branch_on` is the branch the root is on (`refs/heads/<name>`). After a command
/// that may write, which branch that is may have changed: any branch counts.
fn ref_update_lands_on(
    post: &[String],
    after_a_writer: bool,
    branch_on: &dyn Fn() -> Option<String>,
) -> Option<Lands> {
    let mut words: Vec<&String> = Vec::new();
    let mut delete = false;
    let mut i = 0usize;
    while i < post.len() {
        match post[i].as_str() {
            "--stdin" => return Some(Lands::Unread),
            "-d" => delete = true,
            "-m" => i += 1, // its reason
            w if w.starts_with('-') => {}
            _ => words.push(&post[i]),
        }
        i += 1;
    }
    let name = words.first()?;
    // Folded: on a volume that folds case (macOS's, by default) `refs/heads/MAIN` is the file
    // `refs/heads/main`; where it does not, folding only adds a refusal.
    let ours = name.eq_ignore_ascii_case("HEAD")
        || name.as_str() == "@"
        || shell_fills(name)
        || (after_a_writer && name.to_ascii_lowercase().starts_with("refs/heads/"))
        || branch_on().is_some_and(|on| on.eq_ignore_ascii_case(name));
    if !ours {
        return None;
    }
    let target = if delete {
        "@{upstream}".to_string()
    } else {
        words.get(1)?.to_string()
    };
    Some(Lands::On(target, false))
}

/// The branch the root is on, as `refs/heads/<name>`, or `None` for a detached HEAD or a
/// question that could not be asked.
fn current_branch(root: &str) -> Option<String> {
    let a = git_in(root, &["symbolic-ref", "--quiet", "HEAD"]).ok()?;
    let name = py_strip(&a.out);
    (a.ok && !name.is_empty()).then(|| name.to_string())
}

#[cfg(test)]
mod tests;
