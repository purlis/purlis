// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Running the git binary. Nothing here decides anything.
//!
//! Charter drives git through its binary rather than a library (ADR 0027), so this is the one
//! place that spawns it, and the hazards of doing so are handled here and nowhere else.
//!
//! # The environment is constructed, not inherited
//!
//! Subtracting a denylist does not work. `git rev-parse --local-env-vars` defines where git
//! looks for a **repository** — fifteen names on git 2.50.1. It says nothing about what git
//! **executes**, and measured on that same git:
//!
//! - `GIT_EXEC_PATH` makes `push` run an attacker's `git-remote-https`, because the https
//!   remote helper is an external binary resolved out of it.
//! - `GIT_TRACE`, and the `GIT_TRACE2*` family, append to any path given, on *every* verb
//!   including the read-only ones — a write outside the plane that charter never constructs
//!   and no containment check ever sees.
//! - `GIT_SSH_COMMAND`, `GIT_SSH`, `GIT_ASKPASS`, `SSH_ASKPASS` and `GIT_PROXY_COMMAND` each
//!   name a program git runs. `GIT_TERMINAL_PROMPT=0` closes the terminal prompt, not the
//!   askpass helper.
//! - `PATH` decides which `git` runs at all.
//!
//! None of those is repository-local, so no list git prints will ever name them, and charter
//! runs **from a hook**, where an attacker-set environment is the ordinary case. So the child
//! gets `env_clear()` and then only what git needs. A variable git grows next year is absent
//! by default rather than present until someone notices.
//!
//! # `env_clear` alone does not close it, because `HOME` has to stay
//!
//! git reads its global config from `HOME`, and that config can name programs to run. With
//! *exactly* the four variables built below and nothing else, measured:
//!
//! ```text
//! $ env -i HOME=/tmp/evilhome PATH=/usr/bin:/bin GIT_TERMINAL_PROMPT=0 LC_ALL=C \
//!     git -C repo worktree add -b probe …
//! → ran /tmp/evilhome's core.hooksPath hook
//! ```
//!
//! An attacker who can set `GIT_EXEC_PATH` can set `HOME`, and gets the same execution. But
//! `HOME` cannot simply be dropped: git's global config and every credential helper live
//! there, and `publish` needs them.
//!
//! So the execution keys are turned off **on the command line**, where `-c` beats every
//! config file — `core.hooksPath` and `core.fsmonitor` are the two that run a program on the
//! verbs this module uses (`worktree add` runs `post-checkout`; `status` runs the fsmonitor).
//! Measured: the same call with `-c core.hooksPath=/dev/null` does not run the hook.
//!
//! **What that leaves.** A repository's own hooks are disabled for charter's calls too, which
//! is deliberate — charter never commits, and a `post-checkout` charter triggers is not one
//! the operator asked for. And `HOME` remains the residual surface for anything git grows
//! that names a program through config. A call that crosses a network ([`run_network`]) takes
//! `credential.helper` out of config's hands too — it resets every configured helper and names
//! the forge CLI's own — and refuses SSH outright, so `core.sshCommand` is never reached.
//!
//! # A folder's `.git` file is read here, not by git
//!
//! A linked worktree's `.git` is a file in the folder a chat works in, and git follows it to
//! whatever git directory it names, configuration and all. So [`run`], [`run_untimed`],
//! [`run_with_input`] and [`run_network`] read it first ([`super::link`], #1055, which says
//! what is accepted): a call in a folder whose file names anything else is not started, and
//! answers git's own failure status with one sentence where git's words would be; a call in
//! one that checks is given `--git-dir` and `--work-tree`, so git reads what was checked. A
//! clone, whose `.git` is a directory, is run as before.
//!
//! [`run_as_session`] and [`run_in_hook`] are held to the same check (#1476): a call in a folder
//! whose link names something purlis will not follow is refused with the same sentence. They
//! are not handed the directory it read: they ask what the chat's own git would answer, so git
//! finds the repository as the session's git does, and a hook's own `GIT_DIR` still names it
//! there. A scratch clone that keeps its git directory beside it is read, as the check reads
//! one anywhere outside a project's workspaces.
//!
//! **No runner lets git start a git of its own inside a submodule** ([`NO_PROGRAMS`]), those
//! two included: such a child finds its repository for itself, past every check above.
//!
//! # A git that is stopped is stopped with what it started
//!
//! git works through children, and a deadline that ended git alone left them running. So git
//! leads a process group of its own and the group is what a deadline or a stop ends, asked
//! first and killed only if it will not go ([`stop`]). The `purlis` command is the exception,
//! and [`group`] says why.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

mod connecting;
mod destination;
mod group;
mod listing;
mod network;
pub mod presence;

pub use group::{each_git_leads_its_own_group, stay_in_the_callers_group};
pub(crate) use group::{lead, stop};

/// The deadline for a READ — a listing, a status, a config lookup.
///
/// **Never for `worktree add` or `merge`.** Those check out a tree: on a large repo or a cold
/// cache five seconds is routine, and a killed `worktree add` leaves the registration written
/// and the checkout half-done, so the retry meets "branch already exists". Python times out
/// its listing and nothing else (`workspace._GIT_TIMEOUT`); so does this.
///
/// **Thirty seconds, not five.** Five was Python's number for a listing, and it is wrong for
/// a `status`: measured on `main` (run 35462478106), a cold macOS CI runner blew it on a
/// three-file fixture repo, and the panel told the operator "charter could not read the
/// working tree … git did not answer within 5 seconds" about a repository that was perfectly
/// readable. The same is true of any machine under load — which is the machine charter is
/// built for, running dozens of harnesses at once. Nothing waits on this: the read is on
/// `spawn_blocking` and the cell says so until it answers. The deadline is here to stop a
/// hung git holding a thread for ever, and thirty seconds does that just as well as five
/// while no longer calling a slow answer a broken repository.
pub const READ: Duration = Duration::from_secs(30);

/// The deadline for a call that crosses a network: a chat's brokered `clone`, `fetch`, and
/// `publish` when it lands. Only ever through [`run_network`], which is what holds such a call
/// to the one-credential rule. A clone that is not a chat's has a stall limit instead
/// ([`spawn_network`], [`crate::cloning::STALL`]).
pub const NETWORK: Duration = Duration::from_secs(120);

/// Where to look for git when the inherited `PATH` is not to be trusted.
///
/// Charter runs from a hook, so `PATH` is attacker-settable and `Command::new("git")` would
/// let it choose the binary. These are searched first, in order; the inherited `PATH` is the
/// fallback so that a machine keeping git somewhere else still works, and that fallback is
/// the one part of this an attacker with the environment can still reach.
pub(crate) const GIT_DIRS: [&str; 4] = ["/usr/bin", "/usr/local/bin", "/opt/homebrew/bin", "/bin"];

/// What the child is given, and nothing else.
fn child_env(git_dirs: &str) -> Vec<(&'static str, String)> {
    child_env_from(git_dirs, &|name| std::env::var_os(name))
}

/// [`child_env`], reading this process's variables through `var`.
fn child_env_from(
    git_dirs: &str,
    var: &dyn Fn(&str) -> Option<std::ffi::OsString>,
) -> Vec<(&'static str, String)> {
    let mut env: Vec<(&'static str, String)> = Vec::new();
    // git reads the global config from HOME, and every credential helper needs it.
    if let Some(home) = var("HOME") {
        env.push(("HOME", home.to_string_lossy().into_owned()));
    }
    // And from `$XDG_CONFIG_HOME/git/`, where a person may keep their config and their global
    // excludes (`git/ignore`): without it, a read that asks what git ignores answers as if
    // they had none (#1476). It gives nothing `HOME` does not: it only says where a config
    // file is, and every key that runs a program on these verbs is off on the command line.
    // An isolated call names its global file outright, so it reads none from here.
    if let Some(config) = var("XDG_CONFIG_HOME").filter(|value| !value.is_empty()) {
        env.push(("XDG_CONFIG_HOME", config.to_string_lossy().into_owned()));
    }
    // For the credential helpers themselves (`gh`, `git-credential-osxkeychain`), which git
    // resolves as programs. Constructed, never inherited.
    env.push(("PATH", git_dirs.to_string()));
    // A prompt inside a subprocess whose output is captured is an invisible, endless wait.
    // This makes it an auth error instead. It does NOT cover the askpass helper, which is
    // why that variable is simply not passed through.
    env.push(("GIT_TERMINAL_PROMPT", "0".into()));
    // Deterministic messages for the few places charter matches its own output shape.
    env.push(("LC_ALL", "C".into()));
    // No lazy fetch, ever (git 2.44+). A repository whose config names a promisor remote
    // fetches an object it lacks the moment a read needs one — a status's rename search is
    // enough — and that fetch starts the program the remote's config names, as the operator.
    // A repository an agent can write is one whose config it can write, and charter's reads
    // run outside any sandbox; with this the read fails on the missing object instead (FM-4).
    // An explicit `fetch` or `clone` is not a lazy fetch and is unaffected.
    env.push(("GIT_NO_LAZY_FETCH", "1".into()));
    env
}

/// What one git call answered. `code` is `None` when the deadline passed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    pub code: Option<i32>,
    pub out: String,
    pub err: String,
}

impl Run {
    pub fn ok(&self) -> bool {
        self.code == Some(0)
    }

    /// stdout with the trailing newline removed — what almost every caller wants.
    pub fn line(&self) -> &str {
        self.out.trim_end_matches(['\n', '\r'])
    }
}

/// git itself could not be started.
#[derive(Debug, thiserror::Error)]
#[error("purlis could not run git: {0}. Install git, or put it on PATH")]
pub struct GitUnavailable(#[from] std::io::Error);

/// The absolute git binary, and the `PATH` the child gets.
///
/// **The fallback has to be explicit.** Returning a bare `"git"` does not work after
/// `env_clear`: Rust resolves a bare program name against the CHILD's `PATH`, which is the
/// fixed list below — so on any machine whose git lives elsewhere (NixOS, asdf/mise,
/// `~/.local/bin`) every verb would fail with "charter could not run git" while git is on
/// PATH. So the inherited `PATH` is searched here, in the parent, and what the child gets is
/// the absolute path that search found plus the directory holding it.
fn git_binary() -> (PathBuf, String) {
    git_binary_in(&GIT_DIRS, std::env::var_os("PATH").as_deref())
}

/// [`git_binary`] against `fixed` and an inherited `path`, so the search can be tested
/// without the process's own environment.
fn git_binary_in(fixed: &[&str], path: Option<&std::ffi::OsStr>) -> (PathBuf, String) {
    let mut dirs: Vec<String> = fixed.iter().map(|d| (*d).to_string()).collect();
    for dir in fixed {
        let candidate = Path::new(dir).join("git");
        if candidate.is_file() {
            return (candidate, path_value(&dirs));
        }
    }
    // Searched in the PARENT, where `PATH` is still readable, and resolved to an absolute
    // path before the child is built. An attacker who controls the parent's `PATH` still
    // chooses here — but they control charter's own binary lookup too, so this adds no
    // surface that was not already there.
    //
    // **Absolute entries only** ([`crate::programs::searchable`]). A `.` or an empty entry is
    // the working directory, and charter's working directory is routinely a repository: with
    // no git in the fixed directories, a binary called `git` committed to that repository
    // was the one this function chose — and the one handed a forge credential. That is not
    // the operator's `PATH` choosing, which the paragraph above accepts; it is whoever wrote
    // the repository choosing.
    if let Some(path) = path {
        for dir in crate::programs::searchable(path) {
            let candidate = dir.join("git");
            if candidate.is_file() {
                dirs.insert(0, dir.display().to_string());
                return (candidate, path_value(&dirs));
            }
        }
    }
    (PathBuf::from("git"), path_value(&dirs))
}

/// `dirs` as a child's `PATH`, joined the way this platform splits one — `:` here, `;` on
/// Windows (charter-app#100: a `PATH` joined with `:` arrives there as one nonsense entry).
///
/// **A directory that cannot be written into a `PATH` is left out, not split.** On unix that
/// is a directory with a `:` in it, which the joined string would hand the child as TWO
/// entries — the second one relative, resolved against whatever directory the child runs
/// in. A `$HOME` with a colon puts every user directory [`crate::programs`] searches in that
/// position. The program itself is still run by the absolute path found for it; what the
/// child loses is one directory to look for ITS programs in, which is the smaller failure and
/// the same trade `programs::chat_path_from` makes.
pub(crate) fn path_value(dirs: &[String]) -> String {
    let joinable = dirs
        .iter()
        .filter(|dir| std::env::join_paths([dir.as_str()]).is_ok());
    std::env::join_paths(joinable)
        .ok()
        .and_then(|joined| joined.into_string().ok())
        .unwrap_or_default()
}

/// Config keys that name a program git will run, turned off where no config can re-enable
/// them. `-c` on the command line beats the system, global and repository files.
///
/// Not a general hardening list: these are the keys reachable from the verbs this module
/// runs. `core.hooksPath` covers `post-checkout` on `worktree add` and `post-merge` on
/// `merge`; `core.fsmonitor` covers `status`, which `dirt` runs on every guard;
/// `log.showSignature` covers the signer a `log` or a `show` would start to check a signature.
/// The keys that name a program by a name a repository chooses are held per call instead
/// ([`programs_held`]).
///
/// **And git starts no git of its own inside a submodule** (#1055). `status` and `diff` look
/// into each populated submodule by running git there, a `fetch` or a checkout may recurse,
/// and that child finds its repository through the submodule folder's own `.git`, which this
/// module's link check never sees. So a submodule is read by its recorded commit only. What
/// that costs: uncommitted work inside a submodule does not show in purlis. Plumbing diffs do
/// not read `diff.ignoreSubmodules`, so they are given the option ([`PLUMBING_DIFFS`]).
const NO_PROGRAMS: [&str; 7] = [
    "core.hooksPath=/dev/null",
    "core.fsmonitor=false",
    "log.showSignature=false",
    "diff.ignoreSubmodules=dirty",
    "status.submoduleSummary=false",
    "submodule.recurse=false",
    "fetch.recurseSubmodules=false",
];

/// **A bare repository is used only where it is named** (D-1335-8, #1415): on every git call
/// purlis makes, brokered or not, `purlis worktree add` from your own terminal included. A bare
/// repository a chat made in a folder it writes, which no `.git` path covers, is never found by
/// discovery and read in place of the clone meant. Nothing purlis runs finds a bare repository
/// implicitly: one it means is named with `--git-dir`, or is a remote URL. A test's own fixture
/// git, which runs in the bare repositories it made as remotes, is told otherwise
/// ([`crate::testgit`]).
///
/// **"Every git call" is every git process purlis starts** (#1550). A branch the files view
/// reads with gitoxide (`files::status`) starts no git, so this key does not reach it, and it
/// needs none: the branch is opened at the folder purlis names, never found by climbing, and is
/// refused unless that folder is its repository's work tree and not its git directory, so a
/// bare repository, or a git directory made at the folder naming it as its work tree, is never
/// read in its place.
///
/// **On git 2.38 to 2.44** the key also refuses a repository's own `.git` folder as the folder
/// git runs in (git 2.45 allows that one). The session's and the hooks' guards
/// ([`run_as_session`], [`run_in_hook`]) can then fail where the session's own git, which is
/// not given the key, would not. Each fails the safe way. The project-root guard asks about the
/// project's top, never a `.git` folder. The commit guard asks only of a `revert` or a
/// `merge --ff-only`, which need a work tree, so the session's own command fails inside a
/// `.git` folder too. A hook runs at the top of its work tree, and git names `GIT_DIR` for it
/// where it is elsewhere. `purlis scan`'s `rev-parse --show-toplevel` fails inside a `.git`
/// folder on every git.
pub(crate) const BARE_ONLY_WHEN_NAMED: &str = "safe.bareRepository=explicit";

/// The diff verbs that ignore `diff.ignoreSubmodules`, and are told on their own command line.
const PLUMBING_DIFFS: [&str; 3] = ["diff-files", "diff-index", "diff-tree"];

/// What a plumbing diff is told so that it starts no git inside a submodule.
const SUBMODULES_BY_COMMIT: &str = "--ignore-submodules=dirty";

/// The verbs that write a patch, and so run an external diff (`diff.external`,
/// `diff.<driver>.command`) and a textconv (`diff.<driver>.textconv`) when no option says
/// otherwise. Each is told [`PLAIN_PATCH`] on its own command line: a config key cannot turn the
/// external diff off (an empty one is a program git fails to start, and the whole diff with
/// it), and the options hold the operator's own drivers too, so a patch purlis reads is git's
/// own text whoever configured what.
const PATCH_VERBS: [&str; 4] = ["diff", "log", "show", "whatchanged"];

/// What a patch verb is told so that it runs no external diff and no textconv.
const PLAIN_PATCH: [&str; 2] = ["--no-ext-diff", "--no-textconv"];

/// The verbs that reach no program a repository's config names, beyond what [`NO_PROGRAMS`]
/// holds on every call: they read no file through `.gitattributes`, start no editor, signer or
/// transport, and are not given the options that would (a signature format, `--textconv`).
/// Every other verb's call is first given the repository's program keys held
/// ([`programs_held`]); one of these is spared the listing that takes, and so is a `cat-file`
/// that reads no object through a driver ([`reaches_a_program`]). `log`, `show` and
/// `whatchanged` read no work tree, and the only drivers a patch reaches are held on their
/// command line ([`PLAIN_PATCH`]). `clone` and `init` make a repository whose config nobody
/// else has written yet. None of them writes a ref, so none fires a hook: a verb that does is
/// given the listing ([`writes_a_ref`]), because a hook a repository names in its config is not
/// held by [`NO_PROGRAMS`]'s `core.hooksPath`.
const NO_PROGRAM_KEYS: [&str; 15] = [
    "config",
    "log",
    "show",
    "whatchanged",
    "ls-tree",
    "diff-tree",
    "check-attr",
    "rev-parse",
    "init",
    "clone",
    "merge-base",
    "check-ref-format",
    "rev-list",
    "show-ref",
    "for-each-ref",
];

/// Whether a call whose verb starts `from` can reach a program a repository's config names:
/// every verb but [`NO_PROGRAM_KEYS`], `cat-file` only when it reads an object through a
/// driver, and `branch` and `symbolic-ref` only when they write ([`writes_a_ref`]).
fn reaches_a_program(from: &[&str]) -> bool {
    match from.first() {
        Some(&"cat-file") => from
            .iter()
            .any(|word| matches!(*word, "--textconv" | "--filters")),
        Some(&("branch" | "symbolic-ref")) => writes_a_ref(from),
        Some(verb) => !NO_PROGRAM_KEYS.contains(verb),
        None => false,
    }
}

/// The options that make `branch` write: a ref, its config, or its description in an editor.
const BRANCH_WRITES: [&str; 16] = [
    "-d",
    "-D",
    "--delete",
    "-m",
    "-M",
    "--move",
    "-c",
    "-C",
    "--copy",
    "-f",
    "--force",
    "-u",
    "--set-upstream-to",
    "--unset-upstream",
    "--track",
    "--edit-description",
];

/// The options that make `branch` list rather than make one.
const BRANCH_LISTS: [&str; 12] = [
    "--show-current",
    "--list",
    "-l",
    "-a",
    "--all",
    "-r",
    "--remotes",
    "--contains",
    "--no-contains",
    "--merged",
    "--no-merged",
    "--points-at",
];

/// Whether a `branch` or `symbolic-ref` call (`from` starts with the verb) writes a ref, and so
/// fires the reference-transaction hook; anything not read as a listing or a lookup is. A
/// `branch` writes when told to by an option ([`BRANCH_WRITES`]) or when it names a branch and
/// is not told to list ([`BRANCH_LISTS`]); a `symbolic-ref` when it deletes or is given a
/// target as well as a name.
fn writes_a_ref(from: &[&str]) -> bool {
    let Some((verb, rest)) = from.split_first() else {
        return false;
    };
    let option = |word: &&str| word.split('=').next().unwrap_or(word).to_owned();
    let (options, named) = match rest.iter().position(|word| *word == "--") {
        Some(at) => (&rest[..at], rest.len() - at - 1),
        None => (rest, 0),
    };
    let named = named + options.iter().filter(|word| !word.starts_with('-')).count();
    let flags: Vec<String> = options
        .iter()
        .filter(|word| word.starts_with('-'))
        .map(option)
        .collect();
    let has = |set: &[&str]| flags.iter().any(|flag| set.contains(&flag.as_str()));
    match *verb {
        "branch" => has(&BRANCH_WRITES) || (named > 0 && !has(&BRANCH_LISTS)),
        "symbolic-ref" => has(&["-d", "--delete"]) || named > 1,
        _ => true,
    }
}

/// The three keys `git lfs install` writes, with the values it writes: the one filter a
/// repository's own config may name and still have run ([`held_programs`]). `git-lfs` is
/// found on the runner's fixed `PATH`, never where the repository says.
const LFS_FILTER: [(&str, &str); 3] = [
    ("clean", "git-lfs clean -- %f"),
    ("smudge", "git-lfs smudge -- %f"),
    ("process", "git-lfs filter-process"),
];

/// What a call is answered when the repository's config names a program purlis cannot hold
/// for it: one named in a way a command line cannot carry, or one a remote by name starts.
const UNHELD_PROGRAM: &str = "this repository's config names a program git would run for this \
    command that purlis cannot turn off, so git was not run there. Remove that setting, or run \
    git yourself";

/// What a call that would sign is answered when the repository's own config names a signer
/// (D-1805-ruling): purlis never starts that program, and an empty one would only fail inside
/// git, so the call is refused before git runs, saying where the signer belongs instead.
const SIGNER_IN_REPO: &str = "this repo's own git config names a signing program; purlis doesn't \
    run programs a repo's config names. Set the signer in your global git config (use includeIf \
    for a per-folder signer) and purlis will use it.";

/// What a `worktree add` is answered where its checkout may read a config the runner's listing
/// did not ([`checkout_reads_another_config`]).
const ANOTHER_CONFIG: &str = "this repository's git config has an include whose answer depends on \
    where git runs, so purlis cannot tell what a new branch folder's checkout would read, and git \
    was not run there. Remove that include, or run git yourself";

/// The verbs that make a commit, and so sign it when `commit.gpgSign` or `-S` says to.
const COMMIT_VERBS: [&str; 8] = [
    "commit",
    "merge",
    "rebase",
    "cherry-pick",
    "revert",
    "am",
    "pull",
    "commit-tree",
];

/// The `(scope, key, value)` entries of `git config --list --show-scope -z`, in the order git
/// read them.
fn listed_entries(listing: &str) -> impl Iterator<Item = (&str, &str, &str)> {
    let mut fields = listing.split('\0');
    std::iter::from_fn(move || {
        let (scope, entry) = (fields.next()?, fields.next()?);
        let (key, value) = entry.split_once('\n').unwrap_or((entry, ""));
        Some((scope, key, value))
    })
}

/// Whether a config value is git's true.
fn git_true(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "" | "true" | "yes" | "on" | "1"
    )
}

/// Whether the call whose verb starts `from`, with the configuration `listing` (the command
/// line's `-c` included, as the scope `command`), would start a signer: a commit verb told
/// `-S` or `--gpg-sign`, or left to `commit.gpgSign` true, and a tag told to sign or left to
/// `tag.gpgSign` true.
fn would_sign(from: &[&str], listing: &str) -> bool {
    let last = |wanted: &str| {
        listed_entries(listing)
            .filter(|(_, key, _)| key.eq_ignore_ascii_case(wanted))
            .last()
            .is_some_and(|(_, _, value)| git_true(value))
    };
    let Some((verb, rest)) = from.split_first() else {
        return false;
    };
    if COMMIT_VERBS.contains(verb) {
        let mut sign = last("commit.gpgsign");
        for word in rest {
            if *word == "-S" || word.starts_with("--gpg-sign") {
                sign = true;
            } else if *word == "--no-gpg-sign" {
                sign = false;
            }
        }
        sign
    } else if *verb == "tag" {
        let mut sign = last("tag.gpgsign");
        for word in rest {
            if matches!(*word, "-s" | "--sign") || *word == "-u" || word.starts_with("--local-user")
            {
                sign = true;
            } else if *word == "--no-sign" {
                sign = false;
            }
        }
        sign
    } else {
        false
    }
}

/// Whether the repository's own config (any scope but the operator's and purlis's command line)
/// names a signer: `gpg.program`, `gpg.<format>.program` or `gpg.ssh.defaultKeyCommand`.
fn repo_names_a_signer(listing: &str) -> bool {
    listed_entries(listing).any(|(scope, key, _)| {
        if matches!(scope, "global" | "system" | "unknown" | "command") {
            return false;
        }
        let key = key.to_ascii_lowercase();
        key == "gpg.program"
            || key == "gpg.ssh.defaultkeycommand"
            || (key.starts_with("gpg.") && key.ends_with(".program"))
    })
}

/// The verbs that can reach a remote by name, and with it the program its config names for the
/// other end (`remote.<name>.uploadpack`, `.receivepack`); `remote` only as
/// [`REMOTE_SUBVERBS`].
const REMOTE_VERBS: [&str; 5] = ["fetch", "pull", "push", "ls-remote", "remote"];

/// What `git remote` does that asks a remote (`get-url`, `add` and the rest only read or write
/// config).
const REMOTE_SUBVERBS: [&str; 4] = ["update", "prune", "show", "set-head"];

/// Whether a call whose verb starts `from` can reach a remote ([`REMOTE_VERBS`]).
fn reaches_a_remote(from: &[&str]) -> bool {
    match from {
        ["remote", rest @ ..] => rest
            .iter()
            .find(|word| !word.starts_with('-'))
            .is_some_and(|sub| REMOTE_SUBVERBS.contains(sub)),
        [verb, ..] => REMOTE_VERBS.contains(verb),
        [] => false,
    }
}

/// The `-c` settings that hold every program a repository's own config names for the verbs
/// purlis runs, read from `git config --list --show-scope -z` (`scope\0key\nvalue\0` per
/// entry), or `None` when one cannot be held: it is named in a way a `-c` cannot carry (a `=`,
/// a newline or a NUL in its name), or the call can reach a remote (`reaches_remote`) and the
/// config names the program a remote starts at the other end, which git reads first-wins and
/// no `-c` overrides ([`REMOTE_VERBS`]).
///
/// **Held**, set by any scope but the operator's own (global, system) and purlis's command
/// line: each filter's `clean`, `smudge` and `process`, each diff `textconv` and `command`,
/// `diff.external`, each merge `driver`, the signer and what verifies a signature
/// (`gpg.program`, `gpg.<format>.program`, `gpg.ssh.defaultKeyCommand`), the editors
/// (`core.editor`, `sequence.editor`), `core.alternateRefsCommand`, and each trailer's
/// command. Each is given back empty, which git reads as no program or as a program it cannot
/// start, and a filter whose program was held is no longer `required`, so git does not refuse
/// the file it would have filtered. Where git fails to start an empty one, the call fails
/// rather than run what the repository named: the patch verbs never reach the diff ones
/// ([`PLAIN_PATCH`]), a merge that would need a held driver stops on a conflict, and a signed
/// commit in a repository that names its own signer is refused. **A hook named in config**
/// (`hook.<name>.command` and `.event`, git 2.54 and later) is not looked up through
/// `core.hooksPath`, so each name the repository gives a command or an event is turned off by
/// that name (`hook.<name>.enabled=false`); a git older than that reads no such key and runs no
/// such hook. `core.gitProxy` is not held
/// here: it is only for the `git://` transport, which every network call refuses
/// ([`NETWORK_RULE`]), and no other call connects.
///
/// **LFS's standard filter is kept** ([`LFS_FILTER`]), unless the repository's own config also
/// names a program LFS itself would run (its transfer agents and extensions, a credential
/// helper, an ssh or askpass program): then every `filter.lfs` program is held, the
/// operator's own included, so `git-lfs` is never started there. The helper purlis's own git
/// policy writes into a clone is not such a program, because a filter that is kept never gets to
/// ask one ([`lfs_filter_offline`]).
fn held_programs(reaches_remote: bool, listing: &[u8]) -> Option<Vec<String>> {
    let listing = String::from_utf8_lossy(listing);
    let mut held: Vec<String> = Vec::new();
    let mut not_required: Vec<String> = Vec::new();
    let mut lfs_runs_programs = false;
    let mut hooks_off: Vec<String> = Vec::new();
    let mut no_automatic_gc = false;
    let mut hold = |key: String, filter: Option<String>, held: &mut Vec<String>| {
        if !held.contains(&key) {
            held.push(key);
        }
        if let Some(filter) = filter
            && !not_required.contains(&filter)
        {
            not_required.push(filter);
        }
    };
    let mut fields = listing.split('\0');
    while let (Some(scope), Some(entry)) = (fields.next(), fields.next()) {
        if entry.is_empty() || matches!(scope, "global" | "system" | "unknown" | "command") {
            continue;
        }
        let (key, value) = entry.split_once('\n').unwrap_or((entry, ""));
        // git lists the section and the variable lowercased, the subsection as written.
        let Some((section, rest)) = key.split_once('.') else {
            continue;
        };
        let (sub, variable) = match rest.rsplit_once('.') {
            Some((sub, variable)) => (Some(sub), variable),
            None => (None, rest),
        };
        if lfs_names_a_program(section, sub, variable, value) {
            lfs_runs_programs = true;
        }
        let driver = match (section, sub, variable) {
            ("filter", Some(name), "clean" | "smudge" | "process") => {
                let standard = name == "lfs" && LFS_FILTER.contains(&(variable, value));
                (!standard).then(|| Some(format!("filter.{name}")))
            }
            ("diff", Some(_), "textconv" | "command")
            | ("diff", None, "external")
            | ("merge", Some(_), "driver")
            // A signer, and what verifies a signature: a commit, a tag, a merge that checks.
            | ("gpg", _, "program")
            | ("gpg", Some("ssh"), "defaultkeycommand")
            // An editor, which a commit or a merge given no message starts.
            | ("core", None, "editor")
            | ("sequence", None, "editor")
            | ("core", None, "alternaterefscommand")
            // A trailer's command, which a commit given a trailer runs.
            | ("trailer", Some(_), "cmd" | "command") => Some(None),
            _ => None,
        };
        // A hook named in config (git 2.54 and later), which `core.hooksPath` does not reach:
        // turned off by its name, which a hook of the operator's own given an event here is too.
        if section == "hook"
            && let Some(name) = sub
            && matches!(variable, "command" | "event")
        {
            if key.contains(['=', '\n', '\0']) {
                return None;
            }
            let off = format!("hook.{name}.enabled=false");
            if !hooks_off.contains(&off) {
                hooks_off.push(off);
            }
        }
        // What a remote by name starts at the other end, and the remote helper its `vcs` names
        // (#1853). git keeps the first of the first two it reads, and an empty `vcs` names a
        // helper too, so no `-c` can hold one: a call that reaches a remote is refused instead.
        if section == "remote"
            && sub.is_some()
            && matches!(variable, "uploadpack" | "receivepack" | "vcs")
            && reaches_remote
        {
            return None;
        }
        // Commands a gc runs (git 2.42 and later), a list no `-c` empties: the automatic gc and
        // maintenance that would reach them are turned off ([`NO_AUTOMATIC_GC`]), and a verb
        // that collects garbage itself is refused ([`collects_garbage_with_a_hook`]).
        if section == "gc" && sub.is_none() && variable == "recentobjectshook" {
            no_automatic_gc = true;
        }
        if let Some(filter) = driver {
            if key.contains(['=', '\n', '\0']) {
                return None;
            }
            hold(key.to_owned(), filter, &mut held);
        }
    }
    if lfs_runs_programs {
        for (variable, _) in LFS_FILTER {
            hold(
                format!("filter.lfs.{variable}"),
                Some("filter.lfs".to_owned()),
                &mut held,
            );
        }
    }
    let mut settings: Vec<String> = held.into_iter().map(|key| format!("{key}=")).collect();
    settings.extend(
        not_required
            .into_iter()
            .map(|filter| format!("{filter}.required=false")),
    );
    settings.extend(hooks_off);
    if no_automatic_gc {
        settings.extend(NO_AUTOMATIC_GC.map(str::to_owned));
    }
    Some(settings)
}

/// The verbs that collect garbage themselves, and so run `gc.recentObjectsHook` when it is set:
/// `pack-objects` only when it writes a cruft pack.
const COLLECTS_GARBAGE: [&str; 4] = ["gc", "repack", "prune", "maintenance"];

/// What turns off the gc and the maintenance a commit, a merge or a fetch starts on its own,
/// which would run `gc.recentObjectsHook` ([`held_programs`]).
const NO_AUTOMATIC_GC: [&str; 2] = ["gc.auto=0", "maintenance.auto=false"];

/// Whether the call whose verb starts `from` collects garbage itself ([`COLLECTS_GARBAGE`]) where
/// the repository's own config (`listing`, as [`listed_entries`] reads it) names a
/// `gc.recentObjectsHook`. That key is a list, which no `-c` empties, so such a call is refused.
fn collects_garbage_with_a_hook(from: &[&str], listing: &str) -> bool {
    let collects = match from {
        ["pack-objects", rest @ ..] => rest.contains(&"--cruft"),
        [verb, ..] => COLLECTS_GARBAGE.contains(verb),
        [] => false,
    };
    collects && repo_names_a_recent_objects_hook(listing)
}

/// Whether the repository's own config (any scope but the operator's and the command line's)
/// names a `gc.recentObjectsHook`.
fn repo_names_a_recent_objects_hook(listing: &str) -> bool {
    listed_entries(listing).any(|(scope, key, _)| {
        !matches!(scope, "global" | "system" | "unknown" | "command")
            && key.eq_ignore_ascii_case("gc.recentobjectshook")
    })
}

/// Whether the call whose verb starts `from` is a `worktree add` whose checkout may read a config
/// that `listing`, read in the folder the call is run in, did not (#4401).
///
/// The checkout runs in the new branch folder, on the new branch, with a git directory of its own.
/// A conditional include in the repository's own config can hold there and not here (`onbranch:`
/// and `gitdir:`, measured on git 2.50.1), and a listing here shows the include and none of what
/// it would read: a filter there would run, unheld. A path in a worktree config is read from where
/// the new folder's copy of it is. So where the repository's own scopes hold either, the runner
/// cannot tell, and the call is refused. What it can tell is left alone: a plain include in the
/// repository's config is the same file wherever git runs, a worktree config is copied to the new
/// folder, and what your global and system files include is yours, held or not. The same goes for
/// any verb that stays in its folder: a `switch` reads its config before it moves.
fn checkout_reads_another_config(from: &[&str], listing: &str) -> bool {
    let adds = match from.split_first() {
        Some((&"worktree", rest)) => {
            rest.iter().find(|word| !word.starts_with('-')) == Some(&"add")
        }
        _ => false,
    };
    adds && listed_entries(listing).any(|(scope, key, _)| {
        let key = key.to_ascii_lowercase();
        match scope {
            "local" => key.starts_with("includeif."),
            "worktree" => key.starts_with("includeif.") || key.starts_with("include."),
            _ => false,
        }
    })
}

/// Whether the repository's own config at `dir` names a program `git-lfs` itself would start, or
/// moves its store: what refuses an upload before a push ([`crate::lfsupload`], #4399). Your
/// global and system files are yours, and what the command line gives is the caller's. A listing
/// that cannot be had counts as naming one.
pub(crate) fn lfs_would_start_a_program(dir: &Path) -> Result<bool, GitUnavailable> {
    let listed = run_raw_listing(dir)?;
    let Some(listing) = listed else {
        return Ok(true);
    };
    Ok(listed_entries(&listing).any(|(scope, key, value)| {
        if matches!(scope, "global" | "system" | "unknown" | "command") {
            return false;
        }
        // Lowered whole: a sub-section keeps its case in git's listing, and git-lfs may not.
        let key = key.to_ascii_lowercase();
        let Some((section, rest)) = key.split_once('.') else {
            return false;
        };
        let (sub, variable) = match rest.rsplit_once('.') {
            Some((sub, variable)) => (Some(sub), variable),
            None => (None, rest),
        };
        // git-lfs finds some of these with loose patterns, and a URL-scoped key outranks the
        // command line ([`crate::lfsupload`]): anything that spells one counts, at any depth.
        // The exact helper purlis's git policy writes never runs: the one-credential rule empties
        // the list, and the push's own helper follows.
        match (section, sub, variable) {
            ("credential", None, "helper") => !matches!(
                value,
                "!gh auth git-credential" | "!glab auth git-credential"
            ),
            ("credential", _, "helper") | ("core", None, "sshcommand" | "askpass") => true,
            ("lfs", Some(sub), "path") if sub.starts_with("customtransfer.") => true,
            ("lfs", Some(sub), "clean" | "smudge") if sub.starts_with("extension.") => true,
            ("lfs", _, "storage") | ("http", _, "extraheader") => true,
            ("alias", None, "lfs") | ("alias", Some("lfs"), _) => true,
            _ => key.contains("customtransfer") || key.contains("standalonetransferagent"),
        }
    }))
}

fn run_raw_listing(dir: &Path) -> Result<Option<String>, GitUnavailable> {
    let ran = run(dir, &["config", "--list", "--show-scope", "-z"], READ)?;
    Ok(ran.ok().then_some(ran.out))
}

/// Whether a config key (lowercased as git lists it) names a program `git-lfs` runs when git
/// starts it as a filter: a transfer agent or an extension of LFS's own, or the credential,
/// ssh and askpass programs it asks for itself when it downloads. The exact helper purlis's git
/// policy writes is left out ([`lfs_filter_offline`]): nothing else counts as that one, neither
/// another value nor the same value kept for one URL.
fn lfs_names_a_program(section: &str, sub: Option<&str>, variable: &str, value: &str) -> bool {
    match (section, sub, variable) {
        ("credential", None, "helper") if crate::forge::is_policy_helper(value) => false,
        ("lfs", Some(sub), "path") => sub.starts_with("customtransfer."),
        ("lfs", Some(sub), "clean" | "smudge") => sub.starts_with("extension."),
        ("lfs", None, "standalonetransferagent") => true,
        // Not a program, but where git-lfs writes each file it reads, outside any sandbox.
        ("lfs", None, "storage") => true,
        ("credential", _, "helper") => true,
        ("core", None, "sshcommand" | "askpass") => true,
        _ => false,
    }
}

/// What keeps `git-lfs` off the network as a filter (#4380): told by name to leave a file as its
/// pointer rather than download it, which is what `git lfs install --skip-smudge` asks for. It
/// also leaves a pointer where the object is here, measured on git-lfs 3.8.1.
const LFS_SKIP_SMUDGE: &str = "GIT_LFS_SKIP_SMUDGE";

/// Whether the filter is kept here (#4380, #4400): the repository's own config (`listing`, as
/// [`listed_entries`] reads it) names no program LFS would run, so [`held_programs`] leaves it to
/// run. A clone needs its clean filter to read its LFS files as unchanged, and to keep them out of
/// a commit whole.
///
/// **A filter that is kept stays off the network, in any repository.** The call runs outside any
/// sandbox, and a chat can commit the `.lfsconfig` that says where a filter goes, so one that may
/// download reaches a host a chat chose, with whatever it asks. Wiring has nothing to do with it:
/// the person's own helpers, and their `.netrc`, are as much there in a repository no policy
/// wired. [`programs_held`] tells the filter never to download ([`LFS_SKIP_SMUDGE`]), so no
/// credential is asked for and no `.netrc` read, and a call off the network also resets every
/// helper on the command line, which git-lfs's own `git credential` inherits. A call on the
/// network has only the one helper [`NETWORK_RULE`] names, and still downloads nothing. So a
/// download is `git lfs pull`, in your own terminal or under that rule. A store that is not the
/// repository's own holds the filter instead ([`lfs_store_is_its_own`]).
fn lfs_filter_offline(listing: &str) -> bool {
    !listed_entries(listing).any(|(scope, key, value)| {
        if matches!(scope, "global" | "system" | "unknown" | "command") {
            return false;
        }
        let Some((section, rest)) = key.split_once('.') else {
            return false;
        };
        let (sub, variable) = match rest.rsplit_once('.') {
            Some((sub, variable)) => (Some(sub), variable),
            None => (None, rest),
        };
        lfs_names_a_program(section, sub, variable, value)
    })
}

/// What holds the whole of the LFS filter, the person's own included.
const LFS_HELD: [&str; 4] = [
    "filter.lfs.clean=",
    "filter.lfs.smudge=",
    "filter.lfs.process=",
    "filter.lfs.required=false",
];

/// Whether the store `git-lfs` would write to here is the repository's own: no symbolic link at
/// `.git/lfs`, among what it holds, or in `objects/<xx>/<yy>`, the deepest folder it writes in.
/// A chat's sandbox lets it write there, and this git runs outside that sandbox, so a link would
/// have the filter write the chat's bytes where the chat chose (#4380). Where it cannot tell
/// (git is told of another directory, a folder cannot be read, or any config moves the store
/// with `lfs.storage`) the answer is no, and the filter is held. **Not a closed race**, like the listing: a link made after this is followed.
fn lfs_store_is_its_own(
    dir: &Path,
    before_verb: &[&str],
    link: Option<&(PathBuf, PathBuf)>,
    listing: &str,
) -> bool {
    if listed_entries(listing).any(|(_, key, _)| key.eq_ignore_ascii_case("lfs.storage")) {
        return false;
    }
    if !before_verb.iter().all(|word| {
        !matches!(*word, "-C" | "--git-dir" | "--work-tree")
            && !word.starts_with("--git-dir=")
            && !word.starts_with("--work-tree=")
    }) {
        return false;
    }
    let Some(common) = common_git_dir(dir, link) else {
        return false;
    };
    let store = common.join("lfs");
    match store.symlink_metadata() {
        Err(error) => error.kind() == std::io::ErrorKind::NotFound,
        Ok(found) => found.is_dir() && holds_no_link(&store, 2),
    }
}

/// The git directory whose `lfs` folder serves `dir`: the one its top's `.git` folder is, or for
/// a linked worktree the one the checked git directory's `commondir` names.
fn common_git_dir(dir: &Path, link: Option<&(PathBuf, PathBuf)>) -> Option<PathBuf> {
    if let Some((_, git_dir)) = link {
        let Ok(named) = std::fs::read_to_string(git_dir.join("commondir")) else {
            return Some(git_dir.clone());
        };
        return Some(git_dir.join(named.trim()));
    }
    // The folder as git climbs it, links resolved. The nearest `.git` must be one git takes: it
    // climbs past a folder that is no repository, and this cannot say how far.
    std::fs::canonicalize(dir)
        .ok()?
        .ancestors()
        .map(|folder| folder.join(".git"))
        .find(|git| git.symlink_metadata().is_ok())
        .filter(|git| git_takes(git))
}

/// Whether git takes the folder `git` for a repository as it climbs to it, by what git itself
/// requires (`is_git_directory`, #4402): a `HEAD` it reads as one, and `objects` and `refs` it can
/// search. **Never more than git takes, and in places less**: a link at any of those, or a
/// `commondir` (whose repository would be another folder's, store and all), answers no, as
/// anything else this cannot read does. A no only holds the LFS filter.
fn git_takes(git: &Path) -> bool {
    let searchable = |name: &str| {
        let folder = git.join(name);
        folder.symlink_metadata().is_ok_and(|found| found.is_dir())
            && rustix::fs::access(&folder, rustix::fs::Access::EXEC_OK).is_ok()
    };
    let shared = git.join("commondir").symlink_metadata();
    git.symlink_metadata().is_ok_and(|found| found.is_dir())
        && shared.is_err_and(|why| why.kind() == std::io::ErrorKind::NotFound)
        && reads_as_a_head(&git.join("HEAD"))
        && searchable("objects")
        && searchable("refs")
}

/// Whether `head` is a plain file that says what a `HEAD` says: a ref under `refs/`, or an id. It
/// is read following no link and never waiting on a pipe, and as git reads one: its first 255
/// bytes, and past `ref:` only git's own white space, which is narrower than Unicode's.
fn reads_as_a_head(head: &Path) -> bool {
    use rustix::fs::{Mode, OFlags, open};
    let how = OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC;
    let Ok(opened) = open(head, how, Mode::empty()) else {
        return false;
    };
    let file = std::fs::File::from(opened);
    if !file.metadata().is_ok_and(|found| found.is_file()) {
        return false;
    }
    let mut read = Vec::new();
    if file.take(255).read_to_end(&mut read).is_err() {
        return false;
    }
    let Ok(text) = std::str::from_utf8(&read) else {
        return false;
    };
    let text = text.trim_end();
    match text.strip_prefix("ref:") {
        Some(named) => named
            .trim_start_matches([' ', '\t', '\r', '\n'])
            .starts_with("refs/"),
        None => matches!(text.len(), 40 | 64) && text.bytes().all(|b| b.is_ascii_hexdigit()),
    }
}

/// Whether `folder`, down `depth` folders, holds no symbolic link and can all be read.
fn holds_no_link(folder: &Path, depth: u8) -> bool {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return false;
    };
    entries.into_iter().all(|entry| {
        let Ok(entry) = entry else {
            return false;
        };
        let Ok(kind) = entry.file_type() else {
            return false;
        };
        !kind.is_symlink()
            && (!kind.is_dir() || depth == 0 || holds_no_link(&entry.path(), depth - 1))
    })
}

/// `extra` with the programs the repository's own config names held ([`held_programs`]) for a
/// call that can reach one, or what answers the call instead: git's own failure where the configuration could not
/// be listed, or [`UNHELD_PROGRAM`].
///
/// The listing is run as the call will be, with the same options before the verb, the same
/// checked link, pin and environment, so it reads the configuration the call reads. It is kept
/// for the repository until a file it was read from changes ([`listing`]), and a call on a
/// thread an isolation pins always lists again. **Not a closed race**: a configuration
/// rewritten between the listing and the call is read by the call as it is then.
fn programs_held(
    dir: &Path,
    args: &[&str],
    mut extra: Extra,
) -> Result<Result<Extra, RawRun>, GitUnavailable> {
    let Some(at) = verb_at(args) else {
        return Ok(Ok(extra));
    };
    // A verb that can reach another repository runs only through the network entry point,
    // which holds the one-credential rule and lists the call (#1853, [`connecting`]).
    if !extra.network && connecting::connects(&args[at..]) {
        return Ok(Err(refused_raw(connecting::OFF_THE_NETWORK_ENTRY)));
    }
    let programs = reaches_a_program(&args[at..]);
    // `clone` and `init` read no repository's own config ([`network`]).
    let network = extra.network && !matches!(args[at], "clone" | "init");
    if !programs && !network {
        return Ok(Ok(extra));
    }
    let lister = Extra {
        config: extra.config.clone(),
        credentials: false,
        pass: extra.pass.clone(),
        stdin: false,
        link: extra.link.clone(),
        network: false,
    };
    let listed = match config_listing(dir, &args[..at], &lister)? {
        Ok(listed) => listed,
        Err(failed) => return Ok(Err(failed)),
    };
    let said = String::from_utf8_lossy(&listed);
    if network {
        match network::held(&said, extra.credentials && proxy_in_env()) {
            Ok(settings) => extra.config.extend(settings),
            Err(kind) => return Ok(Err(refused_raw(kind.refusal()))),
        }
    }
    if !programs {
        return Ok(Ok(extra));
    }
    if repo_names_a_signer(&said) && would_sign(&args[at..], &said) {
        return Ok(Err(refused_raw(SIGNER_IN_REPO)));
    }
    if collects_garbage_with_a_hook(&args[at..], &said) {
        return Ok(Err(refused_raw(UNHELD_PROGRAM)));
    }
    if checkout_reads_another_config(&args[at..], &said) {
        return Ok(Err(refused_raw(ANOTHER_CONFIG)));
    }
    match held_programs(reaches_a_remote(&args[at..]), &listed) {
        Some(settings) => {
            extra.config.extend(settings);
            // `git lfs` itself is no filter: it is how such a clone downloads, under the rule.
            if args[at] != "lfs" && lfs_filter_offline(&said) {
                if lfs_store_is_its_own(dir, &args[..at], extra.link.as_ref(), &said) {
                    extra.pass.push((LFS_SKIP_SMUDGE.to_owned(), "1".into()));
                    if !extra.network {
                        extra.config.push("credential.helper=".to_owned());
                    }
                } else {
                    extra.config.extend(LFS_HELD.map(str::to_owned));
                }
            }
            Ok(Ok(extra))
        }
        None => Ok(Err(refused_raw(UNHELD_PROGRAM))),
    }
}

/// The repository's configuration as `git config --list --show-scope -z` gives it, for a call
/// with `before_verb` and `lister`: the one kept for it while nothing it was read from changed
/// ([`listing`]), else listed now (with `--show-origin`, which says what to keep it by) and kept.
/// Git's own failure where it could not be listed; a listing that does not read as git's is
/// refused, as one whose programs cannot be held.
fn config_listing(
    dir: &Path,
    before_verb: &[&str],
    lister: &Extra,
) -> Result<Result<Vec<u8>, RawRun>, GitUnavailable> {
    let asked = if isolation().is_none() {
        listing::Asked::new(
            dir,
            before_verb,
            &lister.config,
            &lister.pass,
            lister.link.as_ref(),
        )
    } else {
        None
    };
    if let Some(kept) = asked.as_ref().and_then(listing::kept) {
        return Ok(Ok(kept));
    }
    let mut args: Vec<&str> = before_verb.to_vec();
    args.extend(["config", "--list", "--show-origin", "--show-scope", "-z"]);
    let started = std::time::SystemTime::now();
    let listed = wait_raw(spawn_with(dir, &args, lister)?, READ)?;
    if listed.code != Some(0) {
        return Ok(Err(listed));
    }
    let Some(without) = listing::without_origins(&listed.out) else {
        return Ok(Err(refused_raw(UNHELD_PROGRAM)));
    };
    if let Some(asked) = asked {
        listing::keep(asked, &listed.out, started);
    }
    Ok(Ok(without))
}

/// Whether this process's environment names a proxy git's network call is given
/// ([`CREDENTIAL_ENV`]): an empty `http.proxy` would turn it off ([`network`]).
fn proxy_in_env() -> bool {
    CREDENTIAL_ENV
        .iter()
        .filter(|name| {
            name.to_ascii_lowercase().ends_with("_proxy") && !name.eq_ignore_ascii_case("no_proxy")
        })
        .any(|name| std::env::var_os(name).is_some_and(|value| !value.is_empty()))
}

/// The one-credential rule (charter `docs/git-policy.md`), put where no config file can
/// relax it: every call that crosses a network goes over HTTPS with ONE forge CLI's token,
/// never SSH, never a prompt, never a helper the operator's config happens to name.
///
/// - `protocol.ssh.allow=never` is the load-bearing one. A `url.git@github.com:.insteadOf
///   https://github.com/` in the operator's global config — a common way to force SSH —
///   rewrites the HTTPS URL charter built, and Python charter's clone then went over SSH
///   without a word. Here git refuses the transport instead, and the caller says why.
/// - `ext`, `git` and plain `http` are refused for the same reason: `ext::` runs a command,
///   `git://` and `http://` carry no token or carry it in the clear. `file` is left alone:
///   it reaches no network and asks for no credential.
/// - `credential.helper=` EMPTY resets every helper configured before it — the keychain, a
///   store file — so the only helper left is the forge's own, added after it. That is what
///   "one credential" means; a union of helpers is several.
/// - `core.askPass=` empty is git's own spelling of "no askpass program"; the variable is
///   already never passed.
/// - `submodule.recurse=false`: a submodule URL comes out of the cloned repo and can name
///   any host, and a nested clone does not read the policy (see `docs/git-policy.md`).
const NETWORK_RULE: [&str; 7] = [
    "protocol.ssh.allow=never",
    "protocol.ext.allow=never",
    "protocol.git.allow=never",
    "protocol.http.allow=never",
    "core.askPass=",
    "credential.helper=",
    "submodule.recurse=false",
];

/// The variables a forge CLI reads its credential, its config or its route from.
///
/// Passed through to a NETWORK call only, because git runs the forge's credential helper with
/// git's own environment — and an operator whose `gh` holds its token in `GH_TOKEN`, in a
/// keyring reached over D-Bus, or behind a corporate proxy would otherwise be refused by a
/// clone Python charter performed. Each is read out of this process's environment, never put
/// on a command line and never logged: a token in `argv` is readable by every process on the
/// machine, one in the environment only by the same user.
///
/// # What an attacker who can set each one gains
///
/// The premise throughout is an attacker who already controls this process's environment.
/// They cannot read the operator's token through any of these — every one of them flows
/// INTO the CLI — so what they buy is a credential or a route of their own, and the question
/// for each entry is whether that reaches further than the environment control they started
/// with.
///
/// - **The token names** (`GH_TOKEN`, `GITHUB_TOKEN`, `GH_ENTERPRISE_TOKEN`,
///   `GITHUB_ENTERPRISE_TOKEN`, `GITLAB_TOKEN`, `GITLAB_ACCESS_TOKEN`, `OAUTH_TOKEN`): the
///   call authenticates as the attacker's account instead of the operator's. The host is
///   still the one charter's URL names, and the fetched bytes still land on the operator's
///   disk, so this ends in a failed or an under-privileged read — not in a disclosure.
/// - **The config locations** (`GH_CONFIG_DIR`, `GLAB_CONFIG_DIR`, `XDG_CONFIG_HOME`,
///   `XDG_DATA_HOME`, `XDG_STATE_HOME`): the same thing by another road — a config file the
///   attacker wrote, holding their token. It is **not** code execution: `gh` expands an alias
///   only for a word that is not one of its own commands, and every call here is a built-in
///   (`api`, `auth`). Measured on gh 2.83.2 with a hand-written config holding
///   `aliases: {api: '!echo ALIAS_RAN', notacmd: '!echo …'}` — `gh api` ran the built-in,
///   `gh notacmd` ran the alias.
/// - **The keyring route** (`XDG_RUNTIME_DIR`, `DBUS_SESSION_BUS_ADDRESS`): the CLI asks a
///   secret service the attacker owns, which answers with their credential and learns that a
///   lookup happened. The operator's real keyring is on their own session bus and is not
///   reachable this way. Without these, a Linux desktop that keeps the token in the keyring
///   fails every call with "not logged in".
/// - **The proxies** (`HTTPS_PROXY`/`https_proxy`, `HTTP_PROXY`/`http_proxy`, `ALL_PROXY`,
///   `NO_PROXY`/`no_proxy`): the connection is routed through the attacker's proxy, which
///   sees the host and the timing. TLS is still end to end through the `CONNECT` tunnel, so
///   the token and the contents are not theirs. Without these, a network whose only route out
///   is a proxy cannot reach the forge at all.
/// - **The CA bundle** (`SSL_CERT_FILE`, `SSL_CERT_DIR`): **the sharp one.** An attacker who
///   sets it adds a certificate authority the CLI trusts, and with a proxy they also set that
///   is a full interception of the CLI's HTTPS, token included. It is here because a
///   corporate CA bundle is how many operators reach their own forge at all, and because the
///   attacker needs control of charter's own environment to use it — at which point they can
///   also choose which `charter` runs. If that trade ever stops holding, this is the first
///   entry to drop.
///
/// # Why what is left out does not break an ordinary `gh`
///
/// - `GH_HOST`, `GITLAB_HOST`: every call passes `--hostname` explicitly, so the variable
///   decides nothing — and passing it would let an environment silently move which host is
///   asked and authenticated against.
/// - `GH_EDITOR`, `EDITOR`, `PAGER`, `GH_PAGER`, `BROWSER`: each names a program. Nothing
///   here is interactive, and the output is captured rather than paged.
/// - `GH_PROMPT_DISABLED`, `NO_PROMPT`, `NO_COLOR`, `GH_NO_UPDATE_NOTIFIER`,
///   `GLAB_CHECK_UPDATE`: charter SETS these itself rather than inheriting them, so an
///   environment cannot switch a prompt back on in a process with no terminal.
/// - `GH_DEBUG`, `GIT_CURL_VERBOSE` and the trace family: they write diagnostics — headers
///   included — into output charter captures and repeats in an error. That is a token leak
///   with extra steps.
/// - Every `GIT_*`, `SSH_AUTH_SOCK`, `SSH_ASKPASS`, `GIT_ASKPASS`, `LD_PRELOAD`, `DYLD_*`:
///   the execution surface the module docs list, plus SSH, which the policy does not use.
/// - `HOME` is not in this list because it is given to every call already; `gh`'s config and
///   the keyring live under it.
pub const CREDENTIAL_ENV: [&str; 23] = [
    "GH_TOKEN",
    "GITHUB_TOKEN",
    "GH_ENTERPRISE_TOKEN",
    "GITHUB_ENTERPRISE_TOKEN",
    "GH_CONFIG_DIR",
    "GITLAB_TOKEN",
    "GITLAB_ACCESS_TOKEN",
    "OAUTH_TOKEN",
    "GLAB_CONFIG_DIR",
    "XDG_CONFIG_HOME",
    "XDG_DATA_HOME",
    "XDG_STATE_HOME",
    "XDG_RUNTIME_DIR",
    "DBUS_SESSION_BUS_ADDRESS",
    "HTTPS_PROXY",
    "https_proxy",
    "HTTP_PROXY",
    "http_proxy",
    "NO_PROXY",
    "no_proxy",
    "ALL_PROXY",
    "SSL_CERT_FILE",
    "SSL_CERT_DIR",
];

/// The variables that say WHERE git's configuration is — passed through to [`run_as_session`]
/// only.
///
/// A guard that asks git "what would this command do" has to ask the git the COMMAND will run,
/// and that git reads its global and system config through these. The one that decides a verdict
/// is an alias: `co = checkout` in `$XDG_CONFIG_HOME/git/config`, or in the file
/// `GIT_CONFIG_GLOBAL` names, or in `GIT_CONFIG_COUNT`/`GIT_CONFIG_KEY_<n>`/`GIT_CONFIG_VALUE_<n>`
/// — and a guard whose git cannot see the alias reads `git co feature` as a subcommand it has
/// never heard of and stands aside while the plane root's branch moves. Python's `_git_in`
/// inherits its whole environment, so it sees all of them; the cleared environment every other
/// call here gets would see none.
///
/// **What an attacker who can set them gains is nothing `HOME` does not already give.** Each one
/// only relocates a config FILE, or supplies config entries directly, and `HOME` — which every
/// call keeps, because the global config and every credential helper live under it — relocates
/// the global config already. The keys that run a program on the verbs a guard uses are still
/// turned off on the command line by [`NO_PROGRAMS`], and git documents that `-c` overrides the
/// environment's entries as well as every file's. `GIT_DIR`, `GIT_WORK_TREE`, `GIT_INDEX_FILE`
/// and the rest of the REPOSITORY-locating family stay out, as Python's `util.run` keeps them
/// out: they would answer for a different repository than the `-C` names.
pub const CONFIG_LOCATION_ENV: [&str; 6] = [
    "XDG_CONFIG_HOME",
    "GIT_CONFIG_GLOBAL",
    "GIT_CONFIG_SYSTEM",
    "GIT_CONFIG_NOSYSTEM",
    "GIT_CONFIG_PARAMETERS",
    "GIT_CONFIG_COUNT",
];

/// The most `GIT_CONFIG_KEY_<n>`/`GIT_CONFIG_VALUE_<n>` pairs passed through. git reads as many as
/// `GIT_CONFIG_COUNT` says; past this bound the count itself is withheld rather than truncated,
/// so git is never told to read a pair that was not passed (it refuses a missing one outright).
const MAX_CONFIG_PAIRS: usize = 64;

/// What one spawn adds to the fixed hardening.
#[derive(Default)]
struct Extra {
    /// `-c` settings after [`NO_PROGRAMS`].
    config: Vec<String>,
    /// Whether [`CREDENTIAL_ENV`] reaches the child.
    credentials: bool,
    /// Variables passed through by name, with their values as this process holds them.
    pass: Vec<(String, std::ffi::OsString)>,
    /// Whether git's standard input is a pipe the caller writes, rather than nothing.
    stdin: bool,
    /// The folder's own top and the git directory its `.git` file was checked to name
    /// ([`linked`]): the call is given both, so git reads what was checked.
    link: Option<(PathBuf, PathBuf)>,
    /// Whether the call crosses a network, and so takes its network settings from your own
    /// config only ([`network`]).
    network: bool,
}

/// The work tree and git directory a call in `dir` is held to where `dir` is in a linked
/// worktree or a submodule ([`super::link`], #1055), nothing where it is in a clone or in no
/// repository, or the sentence that refuses the call: its `.git` file names something purlis
/// will not follow.
///
/// A chat may be able to rewrite that file, and git would follow it to a configuration the
/// chat wrote. So purlis reads the file itself, and git is told the answer rather than asked
/// to find it.
fn linked(dir: &Path, args: &[&str]) -> Result<Option<(PathBuf, PathBuf)>, &'static str> {
    use super::link::{Check, Link};
    // A call the thread's own isolation pins is held to a git directory its maker checked,
    // and to that directory itself, not whatever has its name now (#1415).
    if let Some(held) = isolation()
        && let Some(pin) = held.pin_at(&real(dir))
    {
        return if pin.holds() {
            Ok(None)
        } else {
            Err(NOT_THE_CHECKED_REPOSITORY)
        };
    }
    // `worktree repair` is run in a folder that was moved, to write its new name back. Only
    // the bare form: with paths, it repairs the folders it names rather than this one (#1476).
    let repair =
        verb_at(args).is_some_and(|at| args[at] == "worktree" && args[at + 1..] == ["repair"]);
    let check = if repair { Check::Moved } else { Check::Whole };
    match super::link::of(dir, check)? {
        // And only in a folder that was moved: an entry another folder still holds is that
        // folder's, and a repair here would take its record (#1476).
        Link::Linked { top, git_dir } if repair => {
            if held_elsewhere(&top, &git_dir) {
                Err(super::link::CHANGED)
            } else {
                Ok(None)
            }
        }
        // `clone` and `init` make a repository of their own, wherever they are run.
        Link::Linked { top, git_dir } if !matches!(verb(args), Some("clone" | "init")) => {
            Ok(Some((top, git_dir)))
        }
        _ => Ok(None),
    }
}

/// Whether the worktree entry `git_dir`, which `top`'s link names, is held by another folder
/// that is there: the entry's record names that folder, and that folder's own link names the
/// entry back. Then `top` was not moved from there; it was copied, or pointed at the entry.
fn held_elsewhere(top: &Path, git_dir: &Path) -> bool {
    use super::link::{Check, Link};
    let Some(text) = super::pointer::line_of(&git_dir.join("gitdir")) else {
        return false;
    };
    let named = Path::new(text.trim_end_matches(['\n', '\r']));
    let named = if named.is_absolute() {
        named.to_path_buf()
    } else {
        git_dir.join(named)
    };
    let Some(folder) = named.parent() else {
        return false;
    };
    if real(folder) == real(top) {
        return false;
    }
    matches!(
        super::link::of(folder, Check::Whole),
        Ok(Link::Linked { top: theirs, git_dir: held })
            if held == git_dir && real(&theirs) == real(folder)
    )
}

/// What a call [`linked`] refused answers: git's own status for "not a repository", and the
/// sentence where git's words would be.
fn refused(why: &str) -> Run {
    Run {
        code: Some(128),
        out: String::new(),
        err: format!("{why}\n"),
    }
}

/// [`refused`], as bytes.
fn refused_raw(why: &str) -> RawRun {
    let run = refused(why);
    RawRun {
        code: run.code,
        out: Vec::new(),
        err: run.err.into_bytes(),
    }
}

/// [`Extra`] with the folder's checked link and the programs the repository's config names held
/// ([`programs_held`]), or the answer that refuses the call.
fn held(dir: &Path, args: &[&str], extra: Extra) -> Result<Result<Extra, RawRun>, GitUnavailable> {
    let extra = match linked(dir, args) {
        Ok(link) => Extra { link, ..extra },
        Err(why) => return Ok(Err(refused_raw(why))),
    };
    programs_held(dir, args, extra)
}

/// A [`RawRun`] as a [`Run`], its bytes read as text.
fn as_text(raw: RawRun) -> Run {
    Run {
        code: raw.code,
        out: String::from_utf8_lossy(&raw.out).into_owned(),
        err: String::from_utf8_lossy(&raw.err).into_owned(),
    }
}

/// The [`CONFIG_LOCATION_ENV`] variables `lookup` holds, with the numbered pairs
/// `GIT_CONFIG_COUNT` names — or without the count and its pairs when it is not a number up to
/// [`MAX_CONFIG_PAIRS`], or a pair it names is missing.
fn config_location_env(
    lookup: impl Fn(&str) -> Option<std::ffi::OsString>,
) -> Vec<(String, std::ffi::OsString)> {
    let mut out = Vec::new();
    for name in CONFIG_LOCATION_ENV {
        let Some(value) = lookup(name) else { continue };
        if name != "GIT_CONFIG_COUNT" {
            out.push((name.to_string(), value));
            continue;
        }
        let Some(count) = value.to_str().and_then(|v| v.trim().parse::<usize>().ok()) else {
            continue;
        };
        if count > MAX_CONFIG_PAIRS {
            continue;
        }
        let mut pairs = Vec::with_capacity(count * 2);
        for n in 0..count {
            let (key, val) = (
                format!("GIT_CONFIG_KEY_{n}"),
                format!("GIT_CONFIG_VALUE_{n}"),
            );
            match (lookup(&key), lookup(&val)) {
                (Some(k), Some(v)) => {
                    pairs.push((key, k));
                    pairs.push((val, v));
                }
                _ => {
                    pairs.clear();
                    break;
                }
            }
        }
        if pairs.len() == count * 2 {
            out.push((name.to_string(), value));
            out.extend(pairs);
        }
    }
    out
}

/// The git command `args` run: the first word past git's own options (`-c key=value`,
/// `-C dir`, every `--option`, and each of [`TAKES_THE_NEXT_WORD`] with its value).
fn verb<'a>(args: &[&'a str]) -> Option<&'a str> {
    verb_at(args).map(|at| args[at])
}

/// git's own options before the verb that, written without `=`, take the next word as their
/// value (#874): `--git-dir X status` runs `status`, not `X`.
///
/// The verb read decides how a call is run (`worktree repair`'s check, `clone` and `init`
/// making a repository of their own, `status` without optional locks), so a separated value
/// reads the same as its `--option=value` spelling, which was always skipped whole.
const TAKES_THE_NEXT_WORD: &[&str] = &[
    "-c",
    "-C",
    "--git-dir",
    "--work-tree",
    "--namespace",
    "--config-env",
    "--super-prefix",
    "--attr-source",
];

/// Where in `args` the verb is ([`verb`]).
/// Whether `args` can reach another repository, and so run only through [`run_network`]
/// ([`connecting`], #1853): for a test's own fixtures ([`crate::testgit::run`]), which clone and
/// push between folders.
#[cfg(test)]
pub(crate) fn connects(args: &[&str]) -> bool {
    verb_at(args).is_some_and(|at| connecting::connects(&args[at..]))
}

fn verb_at(args: &[&str]) -> Option<usize> {
    let mut at = 0;
    while let Some(word) = args.get(at) {
        match *word {
            option if TAKES_THE_NEXT_WORD.contains(&option) => at += 2,
            option if option.starts_with('-') => at += 1,
            _ => return Some(at),
        }
    }
    None
}

/// The configuration a **brokered** git call reads (#1335, D-1335-7): no global and no system
/// file at all, and only the keys named here on top of the repository's own.
///
/// A git action the app takes for a sandboxed chat runs outside the chat's sandbox, so nothing
/// the chat could have shaped may name a program for it to run. The operator's global and
/// system files are not the chat's, but they routinely define the very drivers a committed
/// `.gitattributes` turns on (an LFS or a custom `filter`, a `diff` textconv), and a repository
/// the chat wrote could ask for one by name. So a brokered call reads neither file
/// (`GIT_CONFIG_GLOBAL=/dev/null`, `GIT_CONFIG_NOSYSTEM=1`), and gets back only the identity
/// the operator's global file gives ([`Isolated::operators`]). What the repository's own
/// config defines is checked before the call runs (`gitbroker`).
///
/// Held per thread for the length of [`isolated`], so every git call the brokered command makes
/// on that thread reads it without each of them being told; a command that hands work to other
/// threads carries it there itself ([`isolation`]).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Isolated {
    /// `key=value`, each passed as `-c`.
    config: Vec<String>,
    /// Each work tree with the git directory a check resolved for it: a call run in that tree
    /// is given both, so git uses the repository that was checked and no other (D-1335-9). A
    /// clone, and beside it the folder of a branch cut from it (D-1453-27).
    pins: Vec<Pin>,
}

/// One pinned work tree: its git directory, and **which directory that was** when it was
/// checked (#1415), so a rename between the check and a call can't swap another clone in.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Pin {
    tree: PathBuf,
    git_dir: PathBuf,
    id: Option<DirId>,
}

/// A directory's identity, its device and inode: the same path names another directory once
/// one is renamed away and another put in its place, and this tells them apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DirId {
    dev: u64,
    ino: u64,
}

impl DirId {
    /// The identity of the directory at `path`, links followed; `None` where nothing is there.
    pub fn of(path: &Path) -> Option<Self> {
        use std::os::unix::fs::MetadataExt as _;
        std::fs::metadata(path).ok().map(|found| Self {
            dev: found.dev(),
            ino: found.ino(),
        })
    }
}

/// What a call in a pinned tree whose git directory is no longer the one checked answers.
const NOT_THE_CHECKED_REPOSITORY: &str = "the repository purlis checked for this call was \
    replaced since it was checked, so git was not run there. Run the command again";

impl Isolated {
    /// `user.name` and `user.email` as given, and nothing else.
    pub fn identity(name: Option<&str>, email: Option<&str>) -> Self {
        let mut config = Vec::new();
        for (key, value) in [("user.name", name), ("user.email", email)] {
            if let Some(value) = value.filter(|v| !v.is_empty() && !v.contains(['\n', '\0'])) {
                config.push(format!("{key}={value}"));
            }
        }
        Self {
            config,
            pins: Vec::new(),
        }
    }

    /// The identity the operator's global git config gives, read through this module's
    /// hardened runner, outside any repository.
    pub fn operators() -> Self {
        let read = |key: &str| {
            run(Path::new("/"), &["config", "--global", "--get", key], READ)
                .ok()
                .filter(Run::ok)
                .map(|run| run.line().to_owned())
        };
        Self::identity(read("user.name").as_deref(), read("user.email").as_deref())
    }

    /// The same, with every call run in `work_tree` given `--git-dir=<git_dir>` and
    /// `--work-tree=<work_tree>`, so it uses the repository a check resolved and discovers none.
    /// A tree pinned before keeps its pin, and one pinned again takes the later git directory.
    #[must_use]
    ///
    /// The git directory is held to the directory it is now ([`DirId`]): [`Self::pinned_as`]
    /// with what is there as it is pinned.
    pub fn pinned(&self, work_tree: &Path, git_dir: &Path) -> Self {
        self.pinned_as(work_tree, git_dir, DirId::of(git_dir))
    }

    /// [`Self::pinned`], held to `id`, the identity the git directory had when it was checked
    /// (#1415): a call in the tree whose git directory is not that directory any more is
    /// refused before git runs, and [`Self::holds`] answers false.
    #[must_use]
    pub fn pinned_as(&self, work_tree: &Path, git_dir: &Path, id: Option<DirId>) -> Self {
        let tree = real(work_tree);
        let mut pins = self.pins.clone();
        pins.retain(|held| held.tree != tree);
        pins.push(Pin {
            tree,
            git_dir: git_dir.to_path_buf(),
            id,
        });
        Self {
            pins,
            ..self.clone()
        }
    }

    /// Whether every pinned git directory is still the directory it was when it was checked.
    pub fn holds(&self) -> bool {
        self.pins.iter().all(Pin::holds)
    }

    /// The git directory a call run in `dir` is pinned to, if it is.
    pub fn pin_of(&self, dir: &Path) -> Option<&Path> {
        self.pin_in(dir).map(|(_, git_dir)| git_dir)
    }

    /// The pinned work tree `dir` is in, and its git directory: `dir` is that tree's top or
    /// any folder inside it (#1055), so a call run below a pinned top is held to the same
    /// git directory and finds none for itself. Of two pinned trees one inside the other, the
    /// inner one is `dir`'s.
    pub fn pin_in(&self, dir: &Path) -> Option<(&Path, &Path)> {
        let dir = real(dir);
        self.pin_at(&dir)
            .map(|pin| (pin.tree.as_path(), pin.git_dir.as_path()))
    }

    /// The pin of the tree `dir` (links resolved) is in: the innermost.
    fn pin_at(&self, dir: &Path) -> Option<&Pin> {
        self.pins
            .iter()
            .filter(|pin| dir.starts_with(&pin.tree))
            .max_by_key(|pin| pin.tree.components().count())
    }

    /// One more key. **For a test's stand-in forge only** (a `url.<base>.insteadOf` that points
    /// a clone at a local directory): the app passes the identity and nothing else.
    #[must_use]
    pub fn also(mut self, key: &str, value: &str) -> Self {
        self.config.push(format!("{key}={value}"));
        self
    }
}

impl Pin {
    /// Whether the git directory is still the one checked.
    fn holds(&self) -> bool {
        DirId::of(&self.git_dir) == self.id
    }
}

/// `path` with its links resolved, or as it is where it cannot be.
fn real(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

thread_local! {
    static ISOLATED: std::cell::RefCell<Option<Isolated>> = const { std::cell::RefCell::new(None) };
}

/// Runs `then` with every git call this thread makes reading only `isolated`'s configuration.
pub fn isolated<T>(isolated: &Isolated, then: impl FnOnce() -> T) -> T {
    struct Restore(Option<Isolated>);
    impl Drop for Restore {
        fn drop(&mut self) {
            let before = self.0.take();
            ISOLATED.with(|held| *held.borrow_mut() = before);
        }
    }
    let before = ISOLATED.with(|held| held.borrow_mut().replace(isolated.clone()));
    let _restore = Restore(before);
    then()
}

/// The isolation this thread runs under, for a command that carries it to threads of its own.
pub fn isolation() -> Option<Isolated> {
    ISOLATED.with(|held| held.borrow().clone())
}

/// [`isolated`] when `isolation` is one, else `then` as it is.
pub fn within<T>(isolation: Option<&Isolated>, then: impl FnOnce() -> T) -> T {
    match isolation {
        Some(held) => isolated(held, then),
        None => then(),
    }
}

fn spawn_with(dir: &Path, args: &[&str], extra: &Extra) -> Result<Child, GitUnavailable> {
    // Before any git is chosen: on a Mac without developer tools `/usr/bin/git` is a stub
    // that opens the system's install dialog (#1813).
    presence::require()?;
    let (binary, dirs) = git_binary();
    let mut cmd = Command::new(binary);
    for setting in NO_PROGRAMS {
        cmd.arg("-c").arg(setting);
    }
    cmd.arg("-c").arg(BARE_ONLY_WHEN_NAMED);
    for setting in &extra.config {
        cmd.arg("-c").arg(setting);
    }
    let isolated = isolation();
    let mut pinned = false;
    if let Some(held) = &isolated {
        for setting in &held.config {
            cmd.arg("-c").arg(setting);
        }
        // Anywhere in a pinned tree: a call in a folder below its top is held to the same
        // git directory, and nothing is found again from that folder's own `.git`.
        if let Some((tree, git_dir)) = held.pin_in(dir) {
            cmd.arg(format!("--git-dir={}", git_dir.display()))
                .arg(format!("--work-tree={}", tree.display()));
            pinned = true;
        }
    }
    if let (false, Some((top, git_dir))) = (pinned, &extra.link) {
        cmd.arg(format!("--git-dir={}", git_dir.display()))
            .arg(format!("--work-tree={}", top.display()));
    }
    cmd.arg("-C").arg(dir);
    match verb_at(args) {
        Some(at)
            if PLUMBING_DIFFS.contains(&args[at])
                && !args
                    .iter()
                    .any(|word| word.starts_with("--ignore-submodules")) =>
        {
            cmd.args(&args[..=at])
                .arg(SUBMODULES_BY_COMMIT)
                .args(&args[at + 1..]);
        }
        Some(at) if PATCH_VERBS.contains(&args[at]) => {
            cmd.args(&args[..=at])
                .args(PLAIN_PATCH)
                .args(&args[at + 1..]);
        }
        _ => {
            cmd.args(args);
        }
    }
    cmd.env_clear();
    for (k, v) in child_env(&dirs) {
        cmd.env(k, v);
    }
    // `status` refreshes the index through `index.lock` whenever stat data is stale, which is
    // the lock an agent's or the operator's `git add` then fails on (FD-11). Every `status`
    // charter runs is a read, so git's optional locks are held back for all of them here,
    // where no caller can forget it: `GIT_OPTIONAL_LOCKS=0` is `--no-optional-locks`.
    if verb(args) == Some("status") {
        cmd.env("GIT_OPTIONAL_LOCKS", "0");
    }
    if extra.credentials {
        for name in CREDENTIAL_ENV {
            if let Some(value) = std::env::var_os(name) {
                cmd.env(name, value);
            }
        }
    }
    for (name, value) in &extra.pass {
        cmd.env(name, value);
    }
    if isolated.is_some() {
        // After everything passed through, so no relocated config file or entry comes back
        // in. `XDG_CONFIG_HOME` stays for a forge CLI's own config: with the global file
        // named, git reads no other.
        for name in CONFIG_LOCATION_ENV
            .into_iter()
            .filter(|name| name.starts_with("GIT_"))
        {
            cmd.env_remove(name);
        }
        cmd.env("GIT_CONFIG_GLOBAL", "/dev/null");
        cmd.env("GIT_CONFIG_NOSYSTEM", "1");
        // Discovery looks at `dir` itself and never climbs to the workspace or the project
        // above it (D-1335-8): every brokered call runs at the top of the tree it means.
        if let Some(parent) = real(dir).parent() {
            cmd.env("GIT_CEILING_DIRECTORIES", parent);
        }
    }
    cmd.stdin(if extra.stdin {
        Stdio::piped()
    } else {
        Stdio::null()
    })
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
    // What a deadline or a stop ends is git and everything git started ([`group`]).
    lead(&mut cmd);
    Ok(crate::forklock::spawn(&mut cmd)?)
}

/// `core.untrackedCache=true`, for the `git add` of a save (FD-11): git remembers which
/// directories held no untracked files by their modification time, so a status in a large tree
/// stops reading every directory. The save's `add` writes the index anyway, so it is what adds
/// the cache; git's own `feature.manyFiles` turns it on the same way.
///
/// **Not for charter's reads.** Measured on git 2.50 at 300,000 files: a read-only status
/// (`--no-optional-locks`) uses a cache the index holds whether or not it is handed this `-c`,
/// never writes one back — so the cache is only as fresh as the last status that could write
/// it, the operator's own — and under `--untracked-files=all` git does not consult it at all.
/// The branch reader (gitoxide) never reads it either. What keeps an idle repo from costing a
/// status is the working-tree watch (`standings`), not this cache.
pub const UNTRACKED_CACHE: &str = "core.untrackedCache=true";

/// [`UNTRACKED_CACHE`] for `dir`, unless the operator's config already says what to do about
/// the untracked cache (`true`, `false` or `keep`): a `-c` on the command line beats every
/// config file, so it is passed only where none speaks.
pub fn untracked_cache(dir: &Path) -> Option<&'static str> {
    let asked = run(dir, &["config", "--get", "core.untrackedCache"], READ).ok()?;
    // `git config --get` exits 1 when the key is set nowhere.
    (asked.code == Some(1)).then_some(UNTRACKED_CACHE)
}

/// `args`, led by `-c` [`UNTRACKED_CACHE`] when [`untracked_cache`] says the operator's config
/// leaves it to charter: what a save's `add` runs (FD-11).
pub fn with_untracked_cache<'a>(dir: &Path, args: &[&'a str]) -> Vec<&'a str> {
    let mut out: Vec<&'a str> = untracked_cache(dir)
        .map(|cache| vec!["-c", cache])
        .unwrap_or_default();
    out.extend_from_slice(args);
    out
}

/// `run`, counted in the test build's [`tally`]: one git process in `dir` from before it is
/// spawned until it has been waited for. In the shipped build it is `run` itself.
fn counted<T>(dir: &Path, args: &[&str], run: impl FnOnce() -> T) -> T {
    #[cfg(test)]
    let _running = tally::Running::start(dir, args);
    #[cfg(not(test))]
    let _ = (dir, args);
    run()
}

/// How many git processes the runner ran in one directory in this test process, and the most
/// that ran there at once (FD-11): what shows the pollers share one standing instead of each
/// spawning its own. Test builds only; the shipped runner counts nothing.
#[cfg(test)]
pub(crate) mod tally {
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};
    use std::sync::{LazyLock, Mutex, PoisonError};

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
    pub struct Tally {
        pub spawned: u64,
        pub most_at_once: u64,
    }

    /// Each directory's tally, and how many git processes run there now.
    static TALLIES: LazyLock<Mutex<HashMap<PathBuf, (Tally, u64)>>> =
        LazyLock::new(Default::default);

    /// Every argument list the runner was handed for each directory, in order.
    static ASKED: LazyLock<Mutex<HashMap<PathBuf, Vec<Vec<String>>>>> =
        LazyLock::new(Default::default);

    /// The argument lists the runner ran git with in `dir`, oldest first.
    pub fn asked(dir: &Path) -> Vec<Vec<String>> {
        ASKED
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(dir)
            .cloned()
            .unwrap_or_default()
    }

    /// [`Tally`] for `dir`, as it was handed to the runner.
    pub fn of(dir: &Path) -> Tally {
        TALLIES
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(dir)
            .map(|(tally, _)| *tally)
            .unwrap_or_default()
    }

    pub(super) struct Running(PathBuf);

    impl Running {
        pub(super) fn start(dir: &Path, args: &[&str]) -> Self {
            ASKED
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .entry(dir.to_path_buf())
                .or_default()
                .push(args.iter().map(|arg| (*arg).to_owned()).collect());
            let mut all = TALLIES.lock().unwrap_or_else(PoisonError::into_inner);
            let (tally, now) = all.entry(dir.to_path_buf()).or_default();
            *now += 1;
            tally.spawned += 1;
            tally.most_at_once = tally.most_at_once.max(*now);
            Self(dir.to_path_buf())
        }
    }

    impl Drop for Running {
        fn drop(&mut self) {
            let mut all = TALLIES.lock().unwrap_or_else(PoisonError::into_inner);
            if let Some((_, now)) = all.get_mut(&self.0) {
                *now = now.saturating_sub(1);
            }
        }
    }
}

#[cfg(test)]
pub(crate) use tally::of as tally;

/// Run `git -C <dir> <args>` with no deadline — for a call that checks out a tree.
pub fn run_untimed(dir: &Path, args: &[&str]) -> Result<Run, GitUnavailable> {
    let extra = match held(dir, args, Extra::default())? {
        Ok(extra) => extra,
        Err(answer) => return Ok(as_text(answer)),
    };
    counted(dir, args, || {
        let child = spawn_with(dir, args, &extra)?;
        // `wait_with_output` reads both pipes as it waits, so it cannot deadlock on them.
        let out = child.wait_with_output()?;
        Ok(Run {
            code: out.status.code(),
            out: String::from_utf8_lossy(&out.stdout).into_owned(),
            err: String::from_utf8_lossy(&out.stderr).into_owned(),
        })
    })
}

/// [`run_untimed`], git reading and writing the index at `index` in place of the repository's
/// own (`GIT_INDEX_FILE`): a snapshot of a work tree's uncommitted work is staged in an index of
/// its own, so what the work tree has staged is never touched (`crate::codelink`, #2046).
pub(crate) fn run_with_index(
    dir: &Path,
    index: &Path,
    args: &[&str],
) -> Result<Run, GitUnavailable> {
    let extra = Extra {
        pass: vec![("GIT_INDEX_FILE".to_owned(), index.as_os_str().to_owned())],
        ..Extra::default()
    };
    let extra = match held(dir, args, extra)? {
        Ok(extra) => extra,
        Err(answer) => return Ok(as_text(answer)),
    };
    counted(dir, args, || {
        let out = spawn_with(dir, args, &extra)?.wait_with_output()?;
        Ok(Run {
            code: out.status.code(),
            out: String::from_utf8_lossy(&out.stdout).into_owned(),
            err: String::from_utf8_lossy(&out.stderr).into_owned(),
        })
    })
}

/// Run `git -C <dir> <args>` with a deadline.
///
/// **The pipes are drained on their own threads** ([`wait`]). Waiting on the child while its
/// output sits in an undrained pipe deadlocks the moment git writes past the buffer — about
/// 64 KiB, which `status --porcelain` in a large dirty clone passes easily — and the symptom
/// is a timeout that looks like a slow machine.
pub fn run(dir: &Path, args: &[&str], timeout: Duration) -> Result<Run, GitUnavailable> {
    let extra = match held(dir, args, Extra::default())? {
        Ok(extra) => extra,
        Err(answer) => return Ok(as_text(answer)),
    };
    counted(dir, args, || {
        Ok(wait(spawn_with(dir, args, &extra)?, timeout)?)
    })
}

/// [`run`], its output as git wrote it ([`RawRun`]): for a caller that must tell a file that is
/// not text from one that is, which [`Run`]'s lossy strings cannot (a skill's files at an
/// earlier commit, SK-57).
pub(crate) fn run_raw(
    dir: &Path,
    args: &[&str],
    timeout: Duration,
) -> Result<RawRun, GitUnavailable> {
    let extra = match held(dir, args, Extra::default())? {
        Ok(extra) => extra,
        Err(answer) => return Ok(answer),
    };
    counted(dir, args, || {
        Ok(wait_raw(spawn_with(dir, args, &extra)?, timeout)?)
    })
}

/// [`run`]'s hardening for a git the caller feeds and reads itself, a request at a time
/// (`cat-file --batch`): the child, with its three streams piped, or the sentence that refuses
/// the call. **The caller owns the deadline**: it stops the child with [`stop`] and waits for
/// it.
pub(crate) fn spawn_fed(
    dir: &Path,
    args: &[&str],
) -> Result<Result<Child, &'static str>, GitUnavailable> {
    let extra = Extra {
        stdin: true,
        ..Extra::default()
    };
    match linked(dir, args) {
        Ok(link) => match programs_held(dir, args, Extra { link, ..extra })? {
            Ok(extra) => Ok(Ok(spawn_with(dir, args, &extra)?)),
            Err(_) => Ok(Err(UNLISTED)),
        },
        Err(why) => Ok(Err(why)),
    }
}

/// What a fed call answers where the repository's configuration could not be listed, or names a
/// program purlis cannot hold ([`programs_held`]).
const UNLISTED: &str = "purlis could not read this repository's config to hold the programs \
    it names, so git was not run there";

/// [`run`], hearing `stop` as well as its deadline: the moment `stop` is raised the child is
/// ended as [`stop`] ends one, and the answer is as for a passed deadline (`code` is `None`).
/// The caller tells the two apart by its own flag. For a read a person can call off while it
/// waits on git — a search's listing of a branch, held in an ignore file that is a FIFO —
/// rather than sit out the whole deadline.
pub(crate) fn run_until(
    dir: &Path,
    args: &[&str],
    timeout: Duration,
    called_off: &AtomicBool,
) -> Result<Run, GitUnavailable> {
    let extra = match held(dir, args, Extra::default())? {
        Ok(extra) => extra,
        Err(answer) => return Ok(as_text(answer)),
    };
    counted(dir, args, || {
        let raw = wait_raw_until(spawn_with(dir, args, &extra)?, timeout, Some(called_off))?;
        Ok(Run {
            code: raw.code,
            out: String::from_utf8_lossy(&raw.out).into_owned(),
            err: String::from_utf8_lossy(&raw.err).into_owned(),
        })
    })
}

/// [`run`], bytes back, its standard output **read as it comes and held to `most_records`
/// NUL-ended records** (a `-z` listing): past them git is stopped, its group with it, and the
/// answer is `None`. A listing a small tree expands into far more than memory holds is never
/// read whole (SK-10, #2773). A read that fails, or a reader that never answers, is an error and
/// never a listing (SK-43, #3115).
pub(crate) fn run_capped(
    dir: &Path,
    args: &[&str],
    timeout: Duration,
    most_records: usize,
) -> Result<Option<RawRun>, GitUnavailable> {
    use std::sync::Arc;
    let extra = match held(dir, args, Extra::default())? {
        Ok(extra) => extra,
        Err(answer) => return Ok(Some(answer)),
    };
    counted(dir, args, || {
        let mut child = spawn_with(dir, args, &extra)?;
        let mut stdout = child.stdout.take().expect("stdout is piped");
        let over = Arc::new(AtomicBool::new(false));
        // What the reader heard: the whole listing, `None` past the cap, or the error that cut
        // its read short. A reader that sends nothing died, and is heard as nothing.
        let (out_tx, out_rx) = std::sync::mpsc::channel::<std::io::Result<Option<Vec<u8>>>>();
        #[cfg(test)]
        let before_read = capped_hook::current();
        #[cfg(not(test))]
        let before_read = || -> std::io::Result<()> { Ok(()) };
        {
            let over = Arc::clone(&over);
            std::thread::spawn(move || {
                let mut kept = Vec::new();
                let mut records = 0usize;
                let mut chunk = [0u8; 64 * 1024];
                let heard = loop {
                    match before_read().and_then(|()| stdout.read(&mut chunk)) {
                        Ok(0) => break Ok(Some(kept)),
                        Ok(n) => {
                            records += chunk[..n].iter().filter(|b| **b == 0).count();
                            if records > most_records {
                                over.store(true, Ordering::Relaxed);
                                break Ok(None);
                            }
                            kept.extend_from_slice(&chunk[..n]);
                        }
                        Err(why) if why.kind() == std::io::ErrorKind::Interrupted => {}
                        Err(why) => break Err(why),
                    }
                };
                let _ = out_tx.send(heard);
            });
        }
        let raw = wait_raw_until(child, timeout, Some(&over))?;
        // The reader's answer decides, never the flag alone: git can exit before the reader
        // has counted its last chunk, and the flag read then let a listing past the cap
        // through, cut short (SK-43, #3115). A git that exited is heard in full, as
        // [`wait_raw_until`] waits for its readers; one that was stopped gets the same grace.
        let heard = match raw.code {
            Some(_) => out_rx.recv().ok(),
            None => out_rx.recv_timeout(Duration::from_secs(2)).ok(),
        };
        match heard {
            Some(Ok(Some(out))) => Ok(Some(RawRun { out, ..raw })),
            Some(Ok(None)) => Ok(None),
            None if over.load(Ordering::Relaxed) => Ok(None),
            // git ran, and its listing could not be read whole: no code, as for a call
            // stopped at the deadline, so no caller takes it for a listing. A reader that
            // never answered is one too, whether git exited, was stopped or was ended by the
            // pipe the dead reader closed.
            Some(Err(why)) => Ok(Some(unread(
                raw,
                &format!("git's listing could not be read: {why}"),
            ))),
            None => Ok(Some(unread(raw, "git's listing was not read to its end"))),
        }
    })
}

/// A [`run_capped`] whose listing could not be read whole: no code and no output, and `why` with
/// whatever git said.
fn unread(raw: RawRun, why: &str) -> RawRun {
    let mut err = raw.err;
    if !err.is_empty() && !err.ends_with(b"\n") {
        err.push(b'\n');
    }
    err.extend_from_slice(why.as_bytes());
    RawRun {
        code: None,
        out: Vec::new(),
        err,
    }
}

/// A test's way into [`run_capped`]'s reader: `f` runs before each read, on the reader's
/// thread, and an error from it is that read's answer. Set on the calling thread, so it reaches
/// only the calls the test that set it makes: `cargo test` runs tests on a pool.
#[cfg(test)]
pub(crate) mod capped_hook {
    use std::cell::RefCell;
    use std::io;
    use std::sync::Arc;

    type Hook = Arc<dyn Fn() -> io::Result<()> + Send + Sync>;

    thread_local! {
        static BEFORE_READ: RefCell<Option<Hook>> = const { RefCell::new(None) };
    }

    /// Run `f` before every read of every [`super::run_capped`] this thread starts, until the
    /// returned guard drops.
    pub(crate) fn set(f: impl Fn() -> io::Result<()> + Send + Sync + 'static) -> Unset {
        BEFORE_READ.with(|h| *h.borrow_mut() = Some(Arc::new(f)));
        Unset
    }

    pub(crate) struct Unset;

    impl Drop for Unset {
        fn drop(&mut self) {
            BEFORE_READ.with(|h| *h.borrow_mut() = None);
        }
    }

    /// What the reader runs before each read: the hook set on this thread, or nothing.
    pub(super) fn current() -> impl Fn() -> io::Result<()> + Send + 'static {
        let hook = BEFORE_READ.with(|h| h.borrow().clone());
        move || hook.as_ref().map_or(Ok(()), |f| f())
    }
}

/// What one git call answered, as the BYTES it wrote. `code` is `None` when the deadline passed,
/// or when [`run_capped`] could not read its listing whole.
///
/// For a caller that must tell "git wrote something that is not UTF-8" from "git wrote U+FFFD",
/// which [`Run`]'s lossy strings cannot: Python decodes a child's output strictly and RAISES on
/// the first, so a port that decoded lossily would answer where its oracle had refused to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawRun {
    pub code: Option<i32>,
    pub out: Vec<u8>,
    pub err: Vec<u8>,
}

/// Run `git -C <dir> <args>` with a deadline, as the SESSION's git would read its config.
///
/// The same hardening as [`run`] — a constructed environment, no hooks, no fsmonitor — plus
/// [`CONFIG_LOCATION_ENV`], so an alias or a setting the operator keeps in a relocated global
/// config is one this call sees too. For the plane-root guards, which ask git what a command the
/// session is about to run will do; nothing that ACTS goes through here.
pub fn run_as_session(
    dir: &Path,
    args: &[&str],
    timeout: Duration,
) -> Result<RawRun, GitUnavailable> {
    if let Err(why) = linked(dir, args) {
        return Ok(refused_raw(why));
    }
    let extra = Extra {
        pass: config_location_env(|name| std::env::var_os(name)),
        ..Extra::default()
    };
    let extra = match programs_held(dir, args, extra)? {
        Ok(extra) => extra,
        Err(answer) => return Ok(answer),
    };
    counted(dir, args, || {
        Ok(wait_raw(spawn_with(dir, args, &extra)?, timeout)?)
    })
}

/// [`run`], with `input` written to git's standard input, as BYTES back: for a `--stdin` verb,
/// so paths reach git as data — never as arguments a name could turn into an option or a
/// pathspec's magic — and `-z` answers can be split exactly.
///
/// The input is written on its own thread, so git answering before it has read all of it never
/// deadlocks on a full pipe.
pub fn run_with_input(
    dir: &Path,
    args: &[&str],
    input: Vec<u8>,
    timeout: Duration,
) -> Result<RawRun, GitUnavailable> {
    run_reading(dir, args, std::io::Cursor::new(input), timeout)
}

/// [`run_with_input`], git's standard input copied from `input` as git reads it: an open file
/// reaches git as the bytes that descriptor reads, never by its name (a save staging the file
/// it checked, `crate::planegit::ondisk`).
pub(crate) fn run_reading(
    dir: &Path,
    args: &[&str],
    mut input: impl std::io::Read + Send + 'static,
    timeout: Duration,
) -> Result<RawRun, GitUnavailable> {
    let extra = Extra {
        stdin: true,
        ..Extra::default()
    };
    let extra = match held(dir, args, extra)? {
        Ok(extra) => extra,
        Err(answer) => return Ok(answer),
    };
    counted(dir, args, || {
        let mut child = spawn_with(dir, args, &extra)?;
        if let Some(mut stdin) = child.stdin.take() {
            std::thread::spawn(move || {
                // A git that stopped reading has answered or failed; its answer says which.
                let _ = std::io::copy(&mut input, &mut stdin);
            });
        }
        Ok(wait_raw(child, timeout)?)
    })
}

/// The variables git hands a hook to say WHICH repository and which index it is working on.
///
/// `git commit -a`, `git commit <paths>` and `git commit --only` stage into a temporary index
/// and name it in `GIT_INDEX_FILE`; a linked worktree or `git --git-dir` names the repository in
/// `GIT_DIR`. A hook that asked git about "the staged diff" without them would be asked about
/// a different index than the one being committed.
pub const HOOK_REPOSITORY_ENV: [&str; 6] = [
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_COMMON_DIR",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
];

/// Run `git -C <dir> <args>` with a deadline, from inside a git hook: [`run`]'s hardening plus
/// [`HOOK_REPOSITORY_ENV`] and [`CONFIG_LOCATION_ENV`], so it reads the index and the config
/// the command that ran the hook reads. Read-only callers only.
pub fn run_in_hook(dir: &Path, args: &[&str], timeout: Duration) -> Result<RawRun, GitUnavailable> {
    if let Err(why) = linked(dir, args) {
        return Ok(refused_raw(why));
    }
    let mut pass = config_location_env(|name| std::env::var_os(name));
    pass.extend(
        HOOK_REPOSITORY_ENV
            .iter()
            .filter_map(|name| std::env::var_os(name).map(|value| ((*name).to_owned(), value))),
    );
    let extra = Extra {
        pass,
        ..Extra::default()
    };
    let extra = match programs_held(dir, args, extra)? {
        Ok(extra) => extra,
        Err(answer) => return Ok(answer),
    };
    counted(dir, args, || {
        Ok(wait_raw(spawn_with(dir, args, &extra)?, timeout)?)
    })
}

/// Run `git -C <dir> <args>` across a network, under the one-credential rule.
///
/// `helper` is the credential helper the forge's own CLI provides — `!'/abs/gh' auth
/// git-credential` — and is the ONLY helper git will consult: [`NETWORK_RULE`] resets every
/// other one first. `None` gives git no credential at all, which is what a host charter does
/// not manage gets: a token for one forge is never offered to another.
///
/// The deadline is [`NETWORK`]. A clone is not run this way: every clone lands whole, with no
/// total deadline ([`spawn_network`], #4079, #4080).
///
/// **A `clone` here is told to leave its files as pointers** ([`LFS_SKIP_SMUDGE`]): it reads no
/// repository's config, so [`programs_held`] never tells its filter, and a repository it brings
/// can name where that filter goes in its own `.lfsconfig` (#4400).
pub fn run_network(dir: &Path, helper: Option<&str>, args: &[&str]) -> Result<Run, GitUnavailable> {
    let mut config: Vec<String> = NETWORK_RULE.iter().map(|s| (*s).to_string()).collect();
    if let Some(helper) = helper {
        config.push(format!("credential.helper={helper}"));
    }
    let pass = if verb(args) == Some("clone") {
        vec![(LFS_SKIP_SMUDGE.to_owned(), "1".into())]
    } else {
        Vec::new()
    };
    let extra = Extra {
        config,
        credentials: true,
        network: true,
        pass,
        ..Extra::default()
    };
    let extra = match held(dir, args, extra)? {
        Ok(extra) => extra,
        Err(answer) => return Ok(as_text(answer)),
    };
    let started = Instant::now();
    let run = counted(dir, args, || {
        Ok::<_, GitUnavailable>(wait(spawn_with(dir, args, &extra)?, NETWORK)?)
    })?;
    // Listed once git has run (#959): a call refused before it started reached nobody. Each
    // place git sent it, read as git reads it (#1853, #1828).
    for (operation, remote) in destinations(dir, args) {
        crate::netlog::git_call(operation, &remote, run.ok(), started.elapsed());
    }
    Ok(run)
}

/// The commit `name` is at in `dir`, or `None` where it names none: not there, holding garbage,
/// or an id with no commit behind it, all zeros included. **What a push hands git** (#4515): git
/// takes a local ref holding garbage or zeros for one that is not there, and a push of that name
/// for a push of nothing, which deletes the remote's branch and exits 0. And zeros given as an id
/// mean the same, so an id is only safe from here, where a commit stands behind it.
pub(crate) fn commit_at(dir: &Path, name: &str) -> Option<String> {
    let commit = format!("{name}^{{commit}}");
    run(dir, &["rev-parse", "--verify", "--quiet", &commit], READ)
        .ok()
        .filter(Run::ok)
        .map(|run| run.line().trim().to_owned())
        .filter(|id| !id.is_empty())
}

/// What a push of a `HEAD` that names no commit answers.
const HEAD_NAMES_NO_COMMIT: &str = "HEAD names no commit here, so nothing was pushed";

/// `git push <options> <url> <commit>:<to>` ([`run_network`]), of the commit `HEAD` is at by its
/// id ([`commit_at`]), so that a `HEAD` gone bad as git starts cannot read as a push of nothing.
/// `to` is a full ref: git cannot tell what an id would make a short name on a remote without it.
pub(crate) fn push_head_by_id(
    dir: &Path,
    helper: Option<&str>,
    options: &[&str],
    url: &str,
    to: &str,
) -> Result<Run, GitUnavailable> {
    let Some(commit) = commit_at(dir, "HEAD") else {
        return Ok(refused(HEAD_NAMES_NO_COMMIT));
    };
    let refspec = format!("{commit}:{to}");
    let mut args = vec!["push"];
    args.extend_from_slice(options);
    args.extend([url, refspec.as_str()]);
    run_network(dir, helper, &args)
}

/// [`run_network`] for a skill source (SK-10, #2773; ADR 0100 §5): `config` after the
/// one-credential rule, and `pass` handed to git by name, so a credential helper that reads one
/// variable of it is the only credential git has. The values in `pass` never reach a command
/// line, and git's output is the caller's to redact before it says any of it.
pub(crate) fn run_network_handing(
    dir: &Path,
    config: &[String],
    pass: Vec<(String, std::ffi::OsString)>,
    args: &[&str],
) -> Result<Run, GitUnavailable> {
    let mut all: Vec<String> = NETWORK_RULE.iter().map(|s| (*s).to_string()).collect();
    all.extend(config.iter().cloned());
    let extra = Extra {
        config: all,
        network: true,
        pass,
        ..Extra::default()
    };
    let extra = match held(dir, args, extra)? {
        Ok(extra) => extra,
        Err(answer) => return Ok(as_text(answer)),
    };
    let started = Instant::now();
    let run = counted(dir, args, || {
        Ok::<_, GitUnavailable>(wait(spawn_with(dir, args, &extra)?, NETWORK)?)
    })?;
    for (operation, remote) in destinations(dir, args) {
        crate::netlog::git_call(operation, &remote, run.ok(), started.elapsed());
    }
    Ok(run)
}

/// [`run_network`] as a child the caller reads as it runs, for its progress, and may stop with
/// [`stop`] (#4079): the child, or the sentence that refuses the call. Under the one-credential
/// rule, unlike [`spawn_persons_clone`]: the same [`NETWORK_RULE`], and `helper` the only
/// helper git consults. `config` adds `-c` settings after them, and `pass` variables by name.
/// **The caller owns the deadline**, and tells the network log how it went
/// ([`network_call_ended`]).
pub(crate) fn spawn_network(
    dir: &Path,
    helper: Option<&str>,
    config: &[String],
    pass: Vec<(String, std::ffi::OsString)>,
    args: &[&str],
) -> Result<Result<Child, String>, GitUnavailable> {
    let mut all: Vec<String> = NETWORK_RULE.iter().map(|s| (*s).to_string()).collect();
    if let Some(helper) = helper {
        all.push(format!("credential.helper={helper}"));
    }
    all.extend(config.iter().cloned());
    let extra = Extra {
        config: all,
        credentials: true,
        network: true,
        pass,
        ..Extra::default()
    };
    match held(dir, args, extra)? {
        Ok(extra) => Ok(Ok(spawn_with(dir, args, &extra)?)),
        Err(refused) => Ok(Err(as_text(refused).err)),
    }
}

/// Lists a [`spawn_network`] call once git has run (#959), as [`run_network`] lists its own.
pub(crate) fn network_call_ended(dir: &Path, args: &[&str], ok: bool, started: Instant) {
    for (operation, remote) in destinations(dir, args) {
        crate::netlog::git_call(operation, &remote, ok, started.elapsed());
    }
}

/// Each URL a push given `args` goes to, read as git reads it ([`destinations`]).
pub(crate) fn push_destinations(dir: &Path, args: &[&str]) -> Vec<String> {
    destinations(dir, args)
        .into_iter()
        .map(|(_, url)| url)
        .collect()
}

/// The verbs that reach a remote the repository names, by its name or as its default.
const BY_NAME: [&str; 4] = ["fetch", "pull", "push", "ls-remote"];

/// **The verb of a network call and each URL git sends it to**, for the network log.
///
/// - **A URL given on the command line** (#1828) is rewritten as git rewrites it
///   ([`destination::rewritten`]): a push reads `pushInsteadOf` first.
/// - **A remote given by name** (#1022): `fetch --prune` and `pull --ff-only` name no URL, so
///   the repository's own answer for the remote is read, with `ls-remote --get-url`, which
///   reaches no network and applies the rewrites as the call did. A push goes where
///   `remote get-url --push --all` says: a remote's push URLs and a `pushInsteadOf` rewrite
///   decide that, and a push goes to each.
/// - **A push that names no remote** goes to the remote git chooses for it
///   ([`destination::push_remote`]), not the fetch's default (#1853).
///
/// A name that is no remote's is read the fetch's way. Empty where git names no URL for it.
fn destinations(dir: &Path, args: &[&str]) -> Vec<(&'static str, String)> {
    let Some(at) = verb_at(args) else {
        return Vec::new();
    };
    let Some(verb) = BY_NAME
        .iter()
        .copied()
        .chain(["clone"])
        .find(|verb| *verb == args[at])
    else {
        return Vec::new();
    };
    let push = verb == "push";
    let ask = |asking: &[&str]| {
        let mut asked: Vec<&str> = args[..at].to_vec();
        asked.extend_from_slice(asking);
        run(dir, &asked, READ)
            .ok()
            .filter(Run::ok)
            .map(|ran| ran.out)
    };
    let config = |pattern: &str| {
        ask(&["config", "-z", "--get-regexp", pattern])
            .map(|listed| destination::entries(&listed))
            .unwrap_or_default()
    };
    if let Some(url) = args[at + 1..]
        .iter()
        .find(|word| !word.starts_with('-') && word.contains("://"))
    {
        // `clone` reads no repository's rewrites but your own, which this listing gives too.
        let rules = config(r"^url\..*\.(push)?insteadof$");
        return vec![(verb, destination::rewritten(url, &rules, push))];
    }
    if verb == "clone" {
        return Vec::new();
    }
    let named = args[at + 1..]
        .iter()
        .copied()
        .find(|word| !word.starts_with('-'))
        .map(str::to_owned);
    let name = match (named, push) {
        (Some(name), _) => Some(name),
        (None, false) => None,
        (None, true) => {
            let branch = ask(&["symbolic-ref", "-q", "--short", "HEAD"])
                .map(|out| out.trim().to_owned())
                .filter(|branch| !branch.is_empty());
            let rules = config(r"^(branch\..*\.(pushremote|remote)|remote\.pushdefault)$");
            Some(destination::push_remote(branch.as_deref(), &rules))
        }
    };
    let lines = |out: String| -> Vec<String> {
        out.lines()
            .map(str::trim)
            .filter(|url| !url.is_empty())
            .map(str::to_owned)
            .collect()
    };
    if push
        && let Some(name) = &name
        && let Some(urls) = ask(&["remote", "get-url", "--push", "--all", name]).map(lines)
        && !urls.is_empty()
    {
        return urls.into_iter().map(|url| (verb, url)).collect();
    }
    let fetched = match &name {
        Some(name) => ask(&["ls-remote", "--get-url", name]),
        None => ask(&["ls-remote", "--get-url"]),
    };
    fetched
        .map(lines)
        .unwrap_or_default()
        .into_iter()
        .take(1)
        .map(|url| (verb, url))
        .collect()
}

/// What the person's own clone from the window ([`spawn_persons_clone`]) is held to: what
/// reaches past their own git sign-in stays refused. `ext::` runs a command, and `git://` and
/// plain `http://` carry nothing safely; the same three [`NETWORK_RULE`] refuses. There is no
/// askpass program, and no submodule is fetched.
const PERSONS_CLONE_RULE: [&str; 5] = [
    "protocol.ext.allow=never",
    "protocol.git.allow=never",
    "protocol.http.allow=never",
    "core.askPass=",
    "submodule.recurse=false",
];

/// The person's own `clone` from the window ("Get a project", #1815), as a child the caller
/// reads as it runs, for its progress, and may stop with [`stop`]: the child, or the sentence
/// that refuses the call. **The caller owns the deadline.**
///
/// **Not under the one-credential rule** (`docs/git-policy.md`, "The person's own clone"). The
/// person pasted the URL and watches the clone, so it is their git: their credential helpers
/// and their SSH, until purlis has a credential of its own for the window's git (#752). What
/// stays held is everything else: the constructed environment, [`NO_PROGRAMS`],
/// [`PERSONS_CLONE_RULE`] and no prompt. `config` adds `-c` settings after them, and `pass`
/// names variables passed through by name ([`CREDENTIAL_ENV`] always is). No chat can reach
/// it: the only caller is a command of the window's alone.
pub(crate) fn spawn_persons_clone(
    dir: &Path,
    args: &[&str],
    config: &[String],
    pass: &[&str],
) -> Result<Result<Child, String>, GitUnavailable> {
    let mut all: Vec<String> = PERSONS_CLONE_RULE
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    all.extend(config.iter().cloned());
    let extra = Extra {
        config: all,
        credentials: true,
        pass: pass
            .iter()
            .filter_map(|name| std::env::var_os(name).map(|value| ((*name).to_owned(), value)))
            .collect(),
        network: true,
        ..Extra::default()
    };
    match held(dir, args, extra)? {
        Ok(extra) => Ok(Ok(spawn_with(dir, args, &extra)?)),
        Err(refused) => Ok(Err(as_text(refused).err)),
    }
}

/// Wait for `child` with a deadline, draining both pipes as it runs.
///
/// Shared with the forge CLI runner (`crate::forge`), which has the same two hazards: an
/// undrained pipe that deadlocks the wait, and a child that never answers.
pub(crate) fn wait(child: Child, timeout: Duration) -> std::io::Result<Run> {
    let raw = wait_raw(child, timeout)?;
    Ok(Run {
        code: raw.code,
        out: String::from_utf8_lossy(&raw.out).into_owned(),
        err: String::from_utf8_lossy(&raw.err).into_owned(),
    })
}

/// [`wait`], keeping the bytes.
fn wait_raw(child: Child, timeout: Duration) -> std::io::Result<RawRun> {
    wait_raw_until(child, timeout, None)
}

/// [`wait_raw`], ending the child early too when `called_off` is raised.
fn wait_raw_until(
    mut child: Child,
    timeout: Duration,
    called_off: Option<&AtomicBool>,
) -> std::io::Result<RawRun> {
    // A caller that reads the output as it comes ([`run_capped`]) took it already.
    let stdout = child.stdout.take();
    let mut stderr = child.stderr.take().expect("stderr is piped");
    let (out_tx, out_rx) = std::sync::mpsc::channel();
    let (err_tx, err_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut stdout) = stdout {
            let _ = stdout.read_to_end(&mut buf);
        }
        let _ = out_tx.send(buf);
    });
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stderr.read_to_end(&mut buf);
        let _ = err_tx.send(buf);
    });

    let deadline = Instant::now() + timeout;
    let code = loop {
        match child.try_wait()? {
            Some(status) => break status.code(),
            None if Instant::now() >= deadline
                || called_off.is_some_and(|off| off.load(Ordering::Relaxed)) =>
            {
                stop(&mut child);
                break None;
            }
            None => std::thread::sleep(Duration::from_millis(10)),
        }
    };

    // A child that exited is waited for in full. One that was STOPPED may have left its own
    // children holding the pipes — `clone` runs `git-remote-https`, which runs the credential
    // helper — and a read that waits for every writer would then outlive the deadline it
    // was stopped for. The stop ends its group with it, but not a child that left the group,
    // nor any under a `purlis` command ([`group`]). So a stopped call gets a short grace for
    // what was already written, and the readers are left behind rather than waited on.
    let collect = |rx: std::sync::mpsc::Receiver<Vec<u8>>| match code {
        Some(_) => rx.recv().unwrap_or_default(),
        None => rx.recv_timeout(Duration::from_secs(2)).unwrap_or_default(),
    };
    let out = collect(out_rx);
    let err = collect(err_rx);
    Ok(RawRun { code, out, err })
}

#[cfg(test)]
mod tests;
