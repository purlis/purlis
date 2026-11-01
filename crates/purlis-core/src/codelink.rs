// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Code over the link** (#2046, ADR 0078 §7, RR-16): a repository's history moves between the
//! desktop and a runner as a git bundle, git's own file of a pack and the refs it carries, which
//! the session protocol carries on a stream of its own (`purlis_session_protocol::code`). This
//! module is every git call either end makes of one, through purlis's one git runner
//! ([`crate::worktree::git`]).
//!
//! **What moves.** The desktop sends a chat's branch and, where its work tree has uncommitted
//! work, a snapshot of that work ([`snapshot`]): a commit of the work tree as it is, every
//! untracked file that is not ignored included, on top of the branch, kept as
//! `refs/purlis/snapshots/<branch>`. The runner sends its branches back, and the desktop keeps
//! them as `refs/remotes/<pool>/<branch>` ([`take_back`]). Saves and pull requests happen on the
//! desktop, so the runner never needs a forge credential.
//!
//! **What a runner takes** ([`receive`]): exactly the refs the desktop named, each a branch or a
//! branch's snapshot whose name git accepts, each at the commit named, in a repository of the
//! project it holds at the folder the desktop named: the project's own, or
//! `workspaces/<workspace>/<repo>` in it, reached through no link. A bundle that carries any
//! other ref is refused whole, before a ref moves.
//!
//! **What the desktop takes back** ([`take_back`]): the runner's branches, under
//! `refs/remotes/<pool>/` alone. No local branch, no work tree and no index of the desktop's is
//! touched, so a runner never moves what the person works on here.
//!
//! **How git is run for a bundle**, on both ends ([`FETCHES_WITH`]): no hooks, every object
//! checked as it arrives, no submodule, no transport but the bundle's own file, no automatic
//! garbage collection; and a snapshot is staged with no attributes from the work tree, so no
//! filter a repository names runs on it. The runner's own settings ([`crate::worktree::git`]'s
//! hardening) apply under all of that.

use std::path::{Path, PathBuf};

use crate::worktree::git;

mod moving;

pub use moving::{
    MOVED, Work, bundle_work, give_work, receive_over, sent, take_over, take_work_back,
    take_work_fresh, work_of,
};

/// Where a branch's snapshot of uncommitted work is kept, in a repository either end holds.
pub const SNAPSHOTS: &str = "refs/purlis/snapshots/";

/// Where a branch is.
pub const HEADS: &str = "refs/heads/";

/// The branch a repository a runner made for the desktop starts on, which no bundle is taken
/// into: git never fetches into the branch a repository is on.
pub const UNBORN: &str = "purlis/held";

/// What every git call that reads a bundle runs with, ahead of its verb: no hook runs, every
/// object is checked as it arrives, no submodule is reached, no transport but a file (a bundle
/// is read through git's file transport), nothing is collected as garbage on the way, and no ref
/// is pruned but where the caller prunes it itself.
pub const FETCHES_WITH: [&str; 22] = [
    "-c",
    "core.hooksPath=/dev/null",
    "-c",
    "transfer.fsckObjects=true",
    "-c",
    "fetch.fsckObjects=true",
    "-c",
    "submodule.recurse=false",
    "-c",
    "protocol.allow=never",
    "-c",
    "protocol.file.allow=always",
    "-c",
    "gc.auto=0",
    "-c",
    "maintenance.auto=false",
    "-c",
    "fetch.writeCommitGraph=false",
    "-c",
    "fetch.prune=false",
    "-c",
    "fetch.pruneTags=false",
];

/// One ref and the commit it is at.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Ref {
    pub name: String,
    pub oid: String,
}

/// Why a transfer of code was not made. Each says what happened in a sentence.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Refused {
    /// A ref, a folder or a pool's name that is not one this module takes.
    #[error("{0}")]
    NotAllowed(String),
    /// git did not do what was asked.
    #[error("{0}")]
    Git(String),
}

fn not_allowed(why: impl Into<String>) -> Refused {
    Refused::NotAllowed(why.into())
}

fn git_failed(what: &str, ran: &git::Run) -> Refused {
    let said = ran.err.trim();
    Refused::Git(if said.is_empty() {
        format!("git could not {what}")
    } else {
        format!("git could not {what}: {said}")
    })
}

fn git_missing(why: git::GitUnavailable) -> Refused {
    Refused::Git(why.to_string())
}

/// **Whether `name` is a ref a transfer may carry**: a branch (`refs/heads/…`) or a branch's
/// snapshot (`refs/purlis/snapshots/…`), as git spells one (`git check-ref-format`'s rules, a
/// `..` among them), and never `HEAD` or any other ref.
pub fn carried(name: &str) -> Result<(), Refused> {
    let refused = || {
        not_allowed(format!(
            "`{name}` is not a branch or a branch's snapshot, so no code was moved under it"
        ))
    };
    let Some(branch) = name
        .strip_prefix(HEADS)
        .or_else(|| name.strip_prefix(SNAPSHOTS))
    else {
        return Err(refused());
    };
    if branch.is_empty() || branch == UNBORN || !ref_format_ok(name) {
        return Err(refused());
    }
    Ok(())
}

/// `git check-ref-format`'s rules, read here so no git is asked about a name a peer sent: no
/// part that starts with a dot or ends with `.lock`, no `..`, no `@{`, no control character, no
/// space or any of `~^:?*[\`, no `//`, and no end in `/` or `.`.
fn ref_format_ok(name: &str) -> bool {
    !name.is_empty()
        && !name.ends_with('/')
        && !name.ends_with('.')
        && !name.contains("..")
        && !name.contains("@{")
        && !name.contains("//")
        && name != "@"
        && name.chars().all(|c| {
            !c.is_control() && !matches!(c, ' ' | '~' | '^' | ':' | '?' | '*' | '[' | '\\')
        })
        && name
            .split('/')
            .all(|part| !part.is_empty() && !part.starts_with('.') && !part.ends_with(".lock"))
}

/// Whether `oid` is a full object name: forty lower-case hexadecimal digits.
fn an_oid(oid: &str) -> bool {
    oid.len() == 40 && oid.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Whether `repo` configures a remote of `name` (`remote.<name>.*`), told apart without regard
/// to case: a pool of that name would write over that remote's refs, and on a file system that
/// ignores case, over one whose name differs from it in case alone.
fn a_remote_of(repo: &Path, name: &str) -> bool {
    let Ok(listed) = run(
        repo,
        &["config", "--name-only", "--get-regexp", "^remote\\."],
    ) else {
        // What the repository configures cannot be read: it is taken to have such a remote.
        return true;
    };
    listed.out.lines().any(|key| {
        key.strip_prefix("remote.")
            .and_then(|rest| rest.rsplit_once('.'))
            .is_some_and(|(remote, _)| remote.eq_ignore_ascii_case(name))
    })
}

/// Whether `pool` is a pool's name a remote ref may be kept under, `refs/remotes/<pool>/`.
fn pool_ok(pool: &str) -> bool {
    crate::contain::workspace_name_ok(pool) && ref_format_ok(&format!("refs/remotes/{pool}/x"))
}

/// **The folder of a project's repository a transfer is for**: `repo` is empty for the
/// project's own, or `workspaces/<workspace>/<repo>` in it, each name one purlis takes, and no
/// folder from the project down to it a link. Answered whether or not it is there yet.
pub fn repo_in(project: &Path, repo: &str) -> Result<PathBuf, Refused> {
    let refused = || {
        not_allowed(format!(
            "`{repo}` is not a repository's folder in the project, so no code was moved there"
        ))
    };
    let at = match repo.split('/').collect::<Vec<_>>().as_slice() {
        [""] => project.to_path_buf(),
        ["workspaces", workspace, name]
            if crate::contain::workspace_name_ok(workspace)
                && crate::contain::repo_name_ok(name) =>
        {
            project.join("workspaces").join(workspace).join(name)
        }
        _ => return Err(refused()),
    };
    // No link from the project down: a chat could put one there to send a transfer elsewhere.
    let mut walked = project.to_path_buf();
    for part in at
        .strip_prefix(project)
        .map_err(|_| refused())?
        .components()
    {
        walked.push(part);
        match std::fs::symlink_metadata(&walked) {
            Ok(meta) if meta.file_type().is_symlink() => return Err(refused()),
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => break,
            Err(_) => return Err(refused()),
        }
    }
    Ok(at)
}

/// **The clones `project` holds in `workspace`** (1.51, #3811): the folder name of each
/// repository in the workspace's folder, sorted. Only a folder reached through no link, with a
/// name a repository's folder may have ([`repo_in`]) and not one the workspace keeps for its own
/// ([`crate::contain::workspace_keeps`]), that is a repository on a branch with a commit. None where the
/// workspace has no folder. Refused for a name no workspace has, and for a workspace reached
/// through a link.
pub fn clones_in(project: &Path, workspace: &str) -> Result<Vec<String>, Refused> {
    if !crate::contain::workspace_name_ok(workspace) {
        return Err(not_allowed(format!(
            "`{workspace}` is not a workspace's name, so no clone of it was named"
        )));
    }
    let workspaces = project.join("workspaces");
    let folder = workspaces.join(workspace);
    for walked in [&workspaces, &folder] {
        match std::fs::symlink_metadata(walked) {
            Ok(meta) if meta.is_dir() => {}
            Ok(_) => {
                return Err(not_allowed(format!(
                    "workspace {workspace} is not a folder in the project, so no clone of it was \
                     named"
                )));
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => {
                return Err(Refused::Git(format!(
                    "workspace {workspace}'s folder could not be read: {e}"
                )));
            }
        }
    }
    let entries = std::fs::read_dir(&folder).map_err(|e| {
        Refused::Git(format!(
            "workspace {workspace}'s folder could not be read: {e}"
        ))
    })?;
    let mut clones: Vec<String> = entries
        .filter_map(Result::ok)
        // `file_type` does not follow a link.
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| crate::contain::repo_name_ok(name))
        .filter(|name| !crate::contain::workspace_keeps(name))
        .filter(|name| a_repository(&folder.join(name)))
        // Only one whose work a move can give: on a branch with a commit ([`work_of`]). One
        // on no branch, or none yet, stays where it is, as before 1.51.
        .filter(|name| current_branch(&folder.join(name)).is_some())
        .collect();
    clones.sort();
    Ok(clones)
}

/// The longest `origin` URL a clone is named or given with (#3985): far past any forge's
/// address, so no runner fills a clone's configuration, or the chat's tab, with one.
const ORIGIN_MOST: usize = 2048;

/// `url`, where it is one a clone is given as its `origin` (#3985): a forge's HTTPS or SSH
/// address with no sign-in in it, held to the rules a URL pasted here is held to
/// ([`crate::getproject::ProjectUrl::parse`]: no other transport, no local path, no token and
/// nothing git or ssh would read as an option), exactly as it is, and within [`ORIGIN_MOST`].
/// Nor an `ssh://` user name ssh could read as an option, which those rules leave to git, a
/// scheme git would hand a remote helper of that spelling (`HTTPS://`), any character but ASCII
/// letters, digits and `-._~:/@+`, or a leading `~` a shell would expand: the chat's tab shows
/// the host it reaches as it is, and a shell takes the command it offers as it is.
fn forge_url(url: &str) -> Option<&str> {
    (url.len() <= ORIGIN_MOST
        && !url.starts_with('~')
        && url
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-._~:/@+".contains(&byte))
        && (url.starts_with("https://") || url.starts_with("ssh://") || !url.contains("://"))
        && !url.contains("://-")
        && crate::getproject::ProjectUrl::parse(url).is_ok_and(|parsed| parsed.as_str() == url))
    .then_some(url)
}

/// **Where each of `repos`, clones `project` holds in `workspace`, came from** (1.54, #3985):
/// the `origin` URL of each one's own configuration, by its folder name, as its git keeps it
/// (no included file read, no `insteadOf` applied), and only where it is a forge's address
/// with no sign-in in it ([`forge_url`]). Nothing for a name that is not one of its clones
/// ([`clones_in`]), nor for one whose `.git` is not a folder of its own with a config file in it.
pub fn origins_in(
    project: &Path,
    workspace: &str,
    repos: &[String],
) -> std::collections::BTreeMap<String, String> {
    let Ok(clones) = clones_in(project, workspace) else {
        return std::collections::BTreeMap::new();
    };
    let folder = project.join("workspaces").join(workspace);
    clones
        .into_iter()
        .filter(|clone| repos.contains(clone))
        // Only a clone's own `.git` folder, and its own config file in it: never one reached
        // through a link, named by a `.git` file or shared through `commondir`, which would
        // read another repository's configuration.
        .filter(|clone| {
            let git = folder.join(clone).join(".git");
            std::fs::symlink_metadata(&git).is_ok_and(|meta| meta.is_dir())
                && std::fs::symlink_metadata(git.join("config")).is_ok_and(|meta| meta.is_file())
        })
        .filter_map(|clone| {
            let ran = run(
                &folder.join(&clone),
                &[
                    "config",
                    "--file",
                    ".git/config",
                    "--no-includes",
                    "--get",
                    "remote.origin.url",
                ],
            )
            .ok()
            .filter(git::Run::ok)?;
            let url = forge_url(ran.out.strip_suffix('\n').unwrap_or(&ran.out))?.to_owned();
            Some((clone, url))
        })
        .collect()
}

/// **The `origin` the runner's clone had, as the person may give a clone made fresh from its
/// work** (#3985): never written into the clone, only offered on the chat's tab with the one
/// command that adds it, so a remote a runner named is the person's own act, never one a save
/// pushes to on its own. Only a forge's address with no sign-in in it ([`forge_url`]), in
/// characters a shell takes as they are, a runner's word being held to the rules every URL
/// here is. Refused where `pool` keeps its branches under `refs/remotes/origin/`.
pub fn offered_origin(pool: &str, url: &str) -> Result<String, Refused> {
    if forge_url(url).is_none() {
        return Err(not_allowed(
            "the runner's clone named an origin that is not a forge's HTTPS or SSH address with \
             no sign-in in it",
        ));
    }
    if pool.eq_ignore_ascii_case("origin") {
        return Err(not_allowed(format!(
            "pool {pool}'s branches are kept under that remote's name"
        )));
    }
    Ok(url.to_owned())
}

/// **A snapshot of `tree`'s uncommitted work**: a commit of the work tree as it is, its untracked
/// files that are not ignored included, whose parent is what `tree` has checked out. `None`
/// where nothing is uncommitted. Staged in an index of its own, so what is staged in `tree` is
/// left as it is, and with no attributes from the work tree, so no filter it names runs.
///
/// `leave_out` names folders and files, relative to the top of `tree`, that no snapshot carries,
/// changed or new: as they are committed, or not at all ([`plane_leaves_out`]). A snapshot of
/// nothing but those is none.
///
/// Nor does it carry a changed or new file a save to the forge would refuse, one with more than
/// one link, or one that cannot be read ([`keep_secrets_out`]).
pub fn snapshot(
    tree: &Path,
    scratch: &Path,
    leave_out: &[String],
) -> Result<Option<String>, Refused> {
    snapshot_leaving(tree, scratch, leave_out, &mut Vec::new())
}

/// [`snapshot`], telling `left` each changed or new file it left out as a save to the forge
/// would refuse it, as the window names them.
pub fn snapshot_leaving(
    tree: &Path,
    scratch: &Path,
    leave_out: &[String],
    left: &mut Vec<String>,
) -> Result<Option<String>, Refused> {
    let Some(written) = work_tree(tree, scratch, leave_out, left)? else {
        return Ok(None);
    };
    let committed = run(tree, &["rev-parse", "--verify", "--quiet", "HEAD^{tree}"])?;
    if committed.ok() && committed.line() == written {
        return Ok(None);
    }
    commit_work(tree, &written).map(Some)
}

/// A commit of the tree `written` on top of what `tree` has checked out: a snapshot.
fn commit_work(tree: &Path, written: &str) -> Result<String, Refused> {
    let commit = run(
        tree,
        &[
            "-c",
            "user.name=purlis",
            "-c",
            "user.email=purlis@localhost",
            "-c",
            "commit.gpgSign=false",
            "commit-tree",
            "-p",
            "HEAD",
            "-m",
            "purlis: uncommitted work",
            written,
        ],
    )?;
    if !commit.ok() {
        return Err(git_failed("keep the uncommitted work", &commit));
    }
    Ok(commit.line().to_owned())
}

/// **The tree `tree`'s work tree is**, as a snapshot stages it ([`snapshot_leaving`]), or `None`
/// where nothing is uncommitted.
fn work_tree(
    tree: &Path,
    scratch: &Path,
    leave_out: &[String],
    left: &mut Vec<String>,
) -> Result<Option<String>, Refused> {
    let status = run(tree, &["status", "--porcelain", "--untracked-files=all"])?;
    if !status.ok() {
        return Err(git_failed("read the work tree", &status));
    }
    if status.out.trim().is_empty() {
        return Ok(None);
    }
    let index = scratch.join("snapshot.index");
    let _ = std::fs::remove_file(&index);
    let empty = empty_tree(tree)?;
    let attr = format!("--attr-source={empty}");
    // An ignored name that git does not track is never staged, and git refuses a pathspec that
    // names one: only such a name goes unnamed. One git tracks is staged with its changes however
    // it is ignored, so it is left out by name. Where the repository ignores case, so does the
    // name: `Secrets` leaves out `secrets/`.
    let magic = if ignores_case(tree) {
        "top,exclude,literal,icase"
    } else {
        "top,exclude,literal"
    };
    let excluded: Vec<String> = leave_out
        .iter()
        .filter(|left| !ignored_and_untracked(tree, left))
        .map(|left| format!(":({magic}){left}"))
        .collect();
    let mut adding = vec![
        "-c",
        "core.attributesFile=/dev/null",
        attr.as_str(),
        "add",
        "--all",
        "--",
        ".",
    ];
    adding.extend(excluded.iter().map(String::as_str));
    let staged = [vec![attr.as_str(), "read-tree", "HEAD"], adding];
    for args in staged {
        let ran = git::run_with_index(tree, &index, &args).map_err(git_missing)?;
        if !ran.ok() {
            let _ = std::fs::remove_file(&index);
            return Err(git_failed("stage the uncommitted work", &ran));
        }
    }
    // What a save to the forge refuses is never sent (ADR 0078 §7, the operator's ruling of
    // 2026-10-25), nor a file with more than one link (it may be another file's, reached through
    // the tree), nor one that cannot be read: each goes as it is committed, or not at all.
    match keep_secrets_out(tree, &index) {
        Ok(kept_out) => left.extend(kept_out),
        Err(why) => {
            let _ = std::fs::remove_file(&index);
            return Err(why);
        }
    }
    let written = git::run_with_index(tree, &index, &["write-tree"]).map_err(git_missing)?;
    let _ = std::fs::remove_file(&index);
    if !written.ok() {
        return Err(git_failed("write the uncommitted work", &written));
    }
    Ok(Some(written.line().to_owned()))
}

/// Whether the repository at `tree` ignores case in its names (`core.ignorecase`), as git sets
/// it on a file system that does.
fn ignores_case(tree: &Path) -> bool {
    run(tree, &["config", "--type=bool", "--get", "core.ignorecase"])
        .is_ok_and(|ran| ran.ok() && ran.line() == "true")
}

/// Whether `name`, relative to the top of `tree`, is ignored there and git tracks nothing by it.
fn ignored_and_untracked(tree: &Path, name: &str) -> bool {
    let ignored =
        run(tree, &["check-ignore", "--quiet", "--no-index", "--", name]).is_ok_and(|ran| ran.ok());
    ignored
        && !run(
            tree,
            &[
                "ls-files",
                "--error-unmatch",
                "--",
                &format!(":(top,literal){name}"),
            ],
        )
        .is_ok_and(|ran| ran.ok())
}

/// **Takes out of the snapshot staged in `index` of `tree`** every changed or new file a save to
/// the forge refuses ([`crate::reposave::refused_as`]: a credential's name, a registry file
/// holding a token, a token or a private key in its text), every one with more than one link,
/// and every one that cannot be read as it was staged: each is staged as it is committed, or not
/// at all where it is not (ADR 0078 §7, the operator's ruling of 2026-10-25). Answers the paths
/// it left out.
fn keep_secrets_out(tree: &Path, index: &Path) -> Result<Vec<String>, Refused> {
    let changed = git::run_with_index(
        tree,
        index,
        &[
            "diff-index",
            "--cached",
            "--name-status",
            "-z",
            "--no-renames",
            "HEAD",
            "--",
        ],
    )
    .map_err(git_missing)?;
    if !changed.ok() {
        return Err(git_failed("read the uncommitted work", &changed));
    }
    let mut fields = changed.out.split('\0');
    let mut left = Vec::new();
    while let (Some(status), Some(path)) = (fields.next(), fields.next()) {
        // A file taken out is no file sent.
        if path.is_empty() || status == "D" {
            continue;
        }
        if leaves_out(tree, index, path) {
            left.push(path.to_owned());
        }
    }
    if left.is_empty() {
        return Ok(left);
    }
    let pathspecs: Vec<String> = left
        .iter()
        .map(|path| format!(":(top,literal){path}"))
        .collect();
    let mut args = vec!["reset", "--quiet", "HEAD", "--"];
    args.extend(pathspecs.iter().map(String::as_str));
    let ran = git::run_with_index(tree, index, &args).map_err(git_missing)?;
    if !ran.ok() {
        return Err(git_failed("leave the credential files out", &ran));
    }
    Ok(left)
}

/// Whether the changed or new file `path`, staged in `index` of `tree`, is left out of a
/// snapshot: one a save refuses, one with more than one link, or one whose staged text or whose
/// file cannot be read, which may have been one of those a moment ago.
fn leaves_out(tree: &Path, index: &Path, path: &str) -> bool {
    let staged = match git::run_with_index(tree, index, &["show", &format!(":0:{path}")]) {
        Ok(ran) if ran.ok() => ran,
        _ => return true,
    };
    std::fs::symlink_metadata(tree.join(path)).is_err()
        || more_than_one_link(&tree.join(path))
        || crate::reposave::refused_as(path, &staged.out).is_some()
}

/// Whether the file at `path` (not followed) has more than one link.
fn more_than_one_link(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        std::fs::symlink_metadata(path).is_ok_and(|meta| meta.is_file() && meta.nlink() > 1)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        false
    }
}

/// The empty tree's name in `repo`'s object format.
fn empty_tree(repo: &Path) -> Result<String, Refused> {
    let ran = run(repo, &["hash-object", "-t", "tree", "--stdin"])?;
    // `hash-object --stdin` with nothing on its input names the empty tree without writing it.
    if !ran.ok() {
        return Err(git_failed("name the empty tree", &ran));
    }
    Ok(ran.line().to_owned())
}

/// **What no snapshot of a project's own repository carries**: purlis's state folder, where its
/// vaults and this machine's sessions are, and the local settings file, under each of their
/// names. Neither belongs on another machine, ignored by the project's `.gitignore` or not.
pub fn plane_leaves_out() -> Vec<String> {
    crate::names::STATE_DIR
        .spellings()
        .chain(crate::names::LOCAL_SETTINGS.spellings())
        .map(str::to_owned)
        .collect()
}

/// What [`bundle`] made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Bundled {
    /// The bundle is at the path it was asked for.
    Wrote,
    /// The other end has every commit already: there is nothing to send.
    Nothing,
}

/// **Bundle `refs` of `repo` into `into`**, leaving out every commit `have` reaches that `repo`
/// holds: the other end's refs, which it already has. Each ref is named by its name in `repo`.
pub fn bundle(
    repo: &Path,
    refs: &[&str],
    have: &[String],
    into: &Path,
) -> Result<Bundled, Refused> {
    let into = into
        .to_str()
        .ok_or_else(|| Refused::Git("the bundle's path is not text".into()))?;
    let mut args: Vec<String> = vec![
        "bundle".into(),
        "create".into(),
        "--quiet".into(),
        into.into(),
    ];
    args.extend(refs.iter().map(|name| (*name).to_owned()));
    let known: Vec<&String> = have
        .iter()
        .filter(|oid| an_oid(oid) && holds_commit(repo, oid))
        .collect();
    if !known.is_empty() {
        args.push("--not".into());
        args.extend(known.into_iter().cloned());
    }
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let ran = git::run_untimed(repo, &args).map_err(git_missing)?;
    if ran.ok() {
        return Ok(Bundled::Wrote);
    }
    if ran.err.contains("empty bundle") {
        let _ = std::fs::remove_file(into);
        return Ok(Bundled::Nothing);
    }
    Err(git_failed("bundle the code", &ran))
}

/// What the desktop sends of one branch: the refs a `take_code` names, and whether the bundle
/// has anything in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sent {
    pub refs: Vec<Ref>,
    pub bundled: Bundled,
    /// The changed or new files the snapshot left out, as a save to the forge would refuse them
    /// ([`snapshot_leaving`]): what the window names.
    pub left_out: Vec<String>,
}

/// **The desktop bundles `branch` of `repo` for a runner**, into `into`: the branch as it is
/// committed and, where `tree` (the work tree it is checked out in) has uncommitted work, a
/// snapshot of it as `refs/purlis/snapshots/<branch>` ([`snapshot`]), leaving out what `have`
/// reaches. The snapshot's ref is kept in `repo` only while the bundle is made. The branch
/// itself, the work tree and its index are left as they are.
pub fn bundle_branch(
    repo: &Path,
    tree: Option<(&Path, &[String])>,
    branch: &str,
    have: &[String],
    into: &Path,
    scratch: &Path,
) -> Result<Sent, Refused> {
    let head = format!("{HEADS}{branch}");
    carried(&head)?;
    let at = run(
        repo,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{head}^{{commit}}"),
        ],
    )?;
    if !at.ok() {
        return Err(not_allowed(format!(
            "there is no branch {branch} here, so nothing was sent"
        )));
    }
    let mut refs = vec![Ref {
        name: head.clone(),
        oid: at.line().to_owned(),
    }];
    let mut left_out = Vec::new();
    let snapshot = match tree {
        Some((tree, leave_out)) => snapshot_leaving(tree, scratch, leave_out, &mut left_out)?,
        None => None,
    };
    let kept = format!("{SNAPSHOTS}{branch}");
    if let Some(oid) = &snapshot {
        let ran = run(repo, &["update-ref", &kept, oid])?;
        if !ran.ok() {
            return Err(git_failed("keep the snapshot for its bundle", &ran));
        }
        refs.push(Ref {
            name: kept.clone(),
            oid: oid.clone(),
        });
    }
    let names: Vec<&str> = refs.iter().map(|one| one.name.as_str()).collect();
    let bundled = bundle(repo, &names, have, into);
    if snapshot.is_some() {
        let _ = run(repo, &["update-ref", "-d", &kept]);
    }
    Ok(Sent {
        refs,
        bundled: bundled?,
        left_out,
    })
}

/// Whether `repo` holds commit `oid`.
fn holds_commit(repo: &Path, oid: &str) -> bool {
    run(repo, &["cat-file", "-e", &format!("{oid}^{{commit}}")]).is_ok_and(|ran| ran.ok())
}

/// The refs a bundle at `path` carries, as `repo`'s git reads them.
fn heads_of(repo: &Path, path: &Path) -> Result<Vec<Ref>, Refused> {
    let path = path
        .to_str()
        .ok_or_else(|| Refused::Git("the bundle's path is not text".into()))?;
    let ran = run(repo, &["bundle", "list-heads", path])?;
    if !ran.ok() {
        return Err(git_failed("read the bundle", &ran));
    }
    Ok(ran
        .out
        .lines()
        .filter_map(|line| line.split_once(' '))
        .map(|(oid, name)| Ref {
            name: name.to_owned(),
            oid: oid.to_owned(),
        })
        .collect())
}

/// **The runner takes `refs` into `repo`** (`take_code`): from the bundle at `bundle` where the
/// desktop sent one, else from the commits `repo` holds already. Every ref named must be one
/// [`carried`] takes, at a full commit name; a bundle may carry those refs and no other, each at
/// the commit named; and a branch checked out in one of `repo`'s work trees is never moved under
/// it. `repo` is made where it is not there yet, a repository with no hook and no branch of its
/// own. Answers every branch and snapshot `repo` holds after it.
///
/// **`checkout`**, a branch among `refs`, is checked out in `repo`'s own work tree where nothing
/// is checked out there yet, with the uncommitted work of the branch's snapshot where `refs`
/// carries one ([`check_out_fresh`]). A repository with something checked out is never checked
/// out again, and a branch one of its work trees has checked out is never moved, so a runner
/// chat's work there is kept.
pub fn receive(
    repo: &Path,
    bundle: Option<&Path>,
    refs: &[Ref],
    checkout: Option<&str>,
) -> Result<Vec<Ref>, Refused> {
    let checking_out = match checkout {
        Some(name) => {
            let branch = refs
                .iter()
                .find(|one| one.name == name && name.starts_with(HEADS))
                .ok_or_else(|| {
                    not_allowed(format!(
                        "`{name}` is not a branch this transfer names, so nothing was checked out"
                    ))
                })?;
            let snapshot = name
                .strip_prefix(HEADS)
                .map(|branch| format!("{SNAPSHOTS}{branch}"))
                .and_then(|kept| refs.iter().find(|one| one.name == kept));
            Some((branch.name.clone(), snapshot.map(|one| one.oid.clone())))
        }
        None => None,
    };
    for one in refs {
        carried(&one.name)?;
        if !an_oid(&one.oid) {
            return Err(not_allowed(format!(
                "`{}` is not a commit's full name, so no code was moved under {}",
                one.oid, one.name
            )));
        }
    }
    made(repo)?;
    // A branch a work tree of the runner's has checked out is the runner chat's own now: it is
    // left as it is, whatever the desktop sends of it.
    let checked_out = checked_out_in(repo)?;
    let carried_by_bundle = match bundle {
        Some(path) => {
            let heads = heads_of(repo, path)?;
            for head in &heads {
                if !refs.contains(head) {
                    return Err(not_allowed(format!(
                        "the bundle carries `{}`, which was not named, so no code was taken from it",
                        head.name
                    )));
                }
            }
            let path = path
                .to_str()
                .ok_or_else(|| Refused::Git("the bundle's path is not text".into()))?;
            let taken: Vec<&Ref> = heads
                .iter()
                .filter(|head| !checked_out.contains(&head.name))
                .collect();
            if !taken.is_empty() {
                let mut args: Vec<String> = FETCHES_WITH.iter().map(|s| (*s).to_owned()).collect();
                args.extend(
                    [
                        "fetch",
                        "--quiet",
                        "--no-tags",
                        "--no-write-fetch-head",
                        "--no-recurse-submodules",
                        path,
                    ]
                    .map(str::to_owned),
                );
                args.extend(taken.iter().map(|head| format!("+{0}:{0}", head.name)));
                let args: Vec<&str> = args.iter().map(String::as_str).collect();
                let ran = git::run_network(repo, None, &args).map_err(git_missing)?;
                if !ran.ok() {
                    return Err(git_failed("take the bundle", &ran));
                }
            }
            heads
        }
        None => Vec::new(),
    };
    for one in refs
        .iter()
        .filter(|one| !carried_by_bundle.contains(one) && !checked_out.contains(&one.name))
    {
        if !holds_commit(repo, &one.oid) {
            return Err(not_allowed(format!(
                "the runner does not hold {} and the bundle did not carry it, so {} was not moved",
                one.oid, one.name
            )));
        }
        let ran = run(repo, &["update-ref", &one.name, &one.oid])?;
        if !ran.ok() {
            return Err(git_failed(&format!("move {}", one.name), &ran));
        }
    }
    if let Some((branch, uncommitted)) = checking_out {
        check_out_fresh(repo, &branch, uncommitted.as_deref())?;
    }
    held_refs(repo)
}

/// **Check `repo`'s own work tree out on `branch`, where nothing is checked out there yet**, with
/// `uncommitted`'s files written over it as uncommitted work: changed files changed, new ones
/// untracked, deleted ones gone. No attribute from the tree is read, so no filter it names runs.
/// A repository with a commit checked out is left as it is.
pub fn check_out_fresh(
    repo: &Path,
    branch: &str,
    uncommitted: Option<&str>,
) -> Result<(), Refused> {
    let short = branch
        .strip_prefix(HEADS)
        .ok_or_else(|| not_allowed(format!("`{branch}` is not a branch to check out")))?;
    let checked_out = run(repo, &["rev-parse", "--verify", "--quiet", "HEAD"])?;
    if checked_out.ok() {
        return Ok(());
    }
    let empty = empty_tree(repo)?;
    let attr = format!("--attr-source={empty}");
    let quiet = ["-c", "core.attributesFile=/dev/null", attr.as_str()];
    let mut args: Vec<&str> = quiet.to_vec();
    args.extend(["checkout", "--quiet", "--force", "--no-guess", short, "--"]);
    let ran = run(repo, &args)?;
    if !ran.ok() {
        return Err(git_failed("check the code out", &ran));
    }
    if let Some(uncommitted) = uncommitted {
        let source = format!("--source={uncommitted}");
        let mut args: Vec<&str> = quiet.to_vec();
        args.extend(["restore", source.as_str(), "--worktree", "--", ":/"]);
        let ran = run(repo, &args)?;
        if !ran.ok() {
            return Err(git_failed("put the uncommitted work back", &ran));
        }
    }
    Ok(())
}

/// The branches checked out in `repo`'s work trees, by their full names.
fn checked_out_in(repo: &Path) -> Result<Vec<String>, Refused> {
    let ran = run(repo, &["worktree", "list", "--porcelain"])?;
    if !ran.ok() {
        return Err(git_failed("list the work trees", &ran));
    }
    let mut out: Vec<String> = ran
        .out
        .lines()
        .filter_map(|line| line.strip_prefix("branch "))
        .map(str::to_owned)
        .collect();
    // An unborn branch is not listed as checked out, yet git keeps it as the one it is on.
    let head = run(repo, &["symbolic-ref", "-q", "HEAD"])?;
    if head.ok() {
        out.push(head.line().to_owned());
    }
    Ok(out)
}

/// Make `repo` a repository where it is not one: no hooks from a template, on [`UNBORN`].
fn made(repo: &Path) -> Result<(), Refused> {
    if repo.join(".git").exists() {
        return Ok(());
    }
    std::fs::create_dir_all(repo)
        .map_err(|why| Refused::Git(format!("the repository's folder could not be made: {why}")))?;
    let initial = format!("--initial-branch={UNBORN}");
    let ran = run(repo, &["init", "--quiet", "--template=", &initial])?;
    if !ran.ok() {
        return Err(git_failed("make the repository", &ran));
    }
    Ok(())
}

/// Every branch and snapshot `repo` holds.
pub fn held_refs(repo: &Path) -> Result<Vec<Ref>, Refused> {
    refs_under(repo, &["refs/heads/", SNAPSHOTS])
}

fn refs_under(repo: &Path, under: &[&str]) -> Result<Vec<Ref>, Refused> {
    let mut args = vec!["for-each-ref", "--format=%(objectname) %(refname)"];
    args.extend_from_slice(under);
    let ran = run(repo, &args)?;
    if !ran.ok() {
        return Err(git_failed("list its refs", &ran));
    }
    Ok(ran
        .out
        .lines()
        .filter_map(|line| line.split_once(' '))
        .map(|(oid, name)| Ref {
            name: name.to_owned(),
            oid: oid.to_owned(),
        })
        .collect())
}

/// **The runner bundles its branches for the desktop** (`give_code`): every branch of `repo`,
/// leaving out what `have` reaches, into `into`. Answers the branches, and whether there was
/// anything to send.
pub fn give(repo: &Path, have: &[String], into: &Path) -> Result<(Vec<Ref>, Bundled), Refused> {
    if !repo.join(".git").exists() {
        return Err(not_allowed(
            "the runner holds no repository there, so there is no code to send",
        ));
    }
    let heads = refs_under(repo, &[HEADS])?;
    if heads.is_empty() {
        return Ok((heads, Bundled::Nothing));
    }
    let names: Vec<&str> = heads.iter().map(|head| head.name.as_str()).collect();
    let bundled = bundle(repo, &names, have, into)?;
    Ok((heads, bundled))
}

/// **The desktop takes a runner's branches back** into `repo`, as `refs/remotes/<pool>/<branch>`
/// and nowhere else: from the bundle at `bundle` where the runner sent one, else from the
/// commits `repo` holds. `heads` are the runner's branches, each one [`carried`] takes; a remote
/// ref of `pool` that the runner no longer has is removed, unless it alone holds its commit in
/// `repo`: a branch only a drained runner had stays where the person sees it (#4363). No local
/// branch, work tree or index is touched. Answers the remote refs as they are after it.
pub fn take_back(
    repo: &Path,
    pool: &str,
    bundle: Option<&Path>,
    heads: &[Ref],
) -> Result<Vec<Ref>, Refused> {
    take_back_as(repo, pool, false, bundle, heads)
}

/// [`take_back`], where `beside` leaves every branch it did not name: a draining runner's come
/// back beside those of the pool's runner now (#4248), and neither's fetch removes the other's.
fn take_back_as(
    repo: &Path,
    pool: &str,
    beside: bool,
    bundle: Option<&Path>,
    heads: &[Ref],
) -> Result<Vec<Ref>, Refused> {
    if !pool_ok(pool) {
        return Err(not_allowed(format!(
            "`{pool}` is not a pool's name refs can be kept under, so nothing was fetched"
        )));
    }
    if a_remote_of(repo, pool) {
        return Err(not_allowed(format!(
            "this repository has a remote named {pool}, whose branches a pool of that name would \
             write over, so nothing was fetched: rename the pool or the remote"
        )));
    }
    let remote = |name: &str| -> Result<String, Refused> {
        match name.strip_prefix(HEADS) {
            Some(branch) if carried(name).is_ok() => Ok(format!("refs/remotes/{pool}/{branch}")),
            _ => Err(not_allowed(format!(
                "the runner sent `{name}`, which is not a branch, so nothing was fetched"
            ))),
        }
    };
    for head in heads {
        remote(&head.name)?;
        if !an_oid(&head.oid) {
            return Err(not_allowed(format!(
                "the runner named `{}` for {}, which is not a commit's full name",
                head.oid, head.name
            )));
        }
    }
    let carried_by_bundle = match bundle {
        Some(path) => {
            let carries = heads_of(repo, path)?;
            for one in &carries {
                if !heads.contains(one) {
                    return Err(not_allowed(format!(
                        "the runner's bundle carries `{}`, which it did not name, so nothing was \
                         fetched",
                        one.name
                    )));
                }
            }
            let path = path
                .to_str()
                .ok_or_else(|| Refused::Git("the bundle's path is not text".into()))?;
            if !carries.is_empty() {
                let mut args: Vec<String> = FETCHES_WITH.iter().map(|s| (*s).to_owned()).collect();
                args.extend(
                    [
                        "fetch",
                        "--quiet",
                        "--no-tags",
                        "--no-write-fetch-head",
                        "--no-recurse-submodules",
                        path,
                    ]
                    .map(str::to_owned),
                );
                for one in &carries {
                    args.push(format!("+{}:{}", one.name, remote(&one.name)?));
                }
                let args: Vec<&str> = args.iter().map(String::as_str).collect();
                let ran = git::run_network(repo, None, &args).map_err(git_missing)?;
                if !ran.ok() {
                    return Err(git_failed("fetch the runner's branches", &ran));
                }
            }
            carries
        }
        None => Vec::new(),
    };
    for head in heads
        .iter()
        .filter(|head| !carried_by_bundle.contains(head))
    {
        if !holds_commit(repo, &head.oid) {
            return Err(not_allowed(format!(
                "this machine does not hold {} and the runner's bundle did not carry it",
                head.oid
            )));
        }
        let ran = run(repo, &["update-ref", &remote(&head.name)?, &head.oid])?;
        if !ran.ok() {
            return Err(git_failed("keep the runner's branch", &ran));
        }
    }
    let prefix = format!("refs/remotes/{pool}/");
    let wanted: Vec<String> = heads
        .iter()
        .map(|head| remote(&head.name))
        .collect::<Result<_, _>>()?;
    for kept in refs_under(repo, &[prefix.as_str()])? {
        if !beside && !wanted.contains(&kept.name) && reached_elsewhere(repo, &kept)? {
            let ran = run(repo, &["update-ref", "-d", &kept.name])?;
            if !ran.ok() {
                return Err(git_failed("remove a branch the runner no longer has", &ran));
            }
        }
    }
    refs_under(repo, &[prefix.as_str()])
}

/// Whether a ref of `repo` other than `one` holds `one`'s commit.
fn reached_elsewhere(repo: &Path, one: &Ref) -> Result<bool, Refused> {
    let ran = run(
        repo,
        &[
            "for-each-ref",
            "--format=%(refname)",
            "--contains",
            &one.oid,
        ],
    )?;
    if !ran.ok() {
        return Err(git_failed("read which branches hold a commit", &ran));
    }
    Ok(ran.out.lines().any(|name| name != one.name))
}

/// The branch `repo` is on, where it is on one with a commit.
pub fn current_branch(repo: &Path) -> Option<String> {
    let ran = run(repo, &["symbolic-ref", "--quiet", "--short", "HEAD"]).ok()?;
    let branch = ran.ok().then(|| ran.line().to_owned())?;
    let born = run(repo, &["rev-parse", "--verify", "--quiet", "HEAD"]).ok()?;
    born.ok().then_some(branch)
}

/// Whether `dir` is the top of a git repository: a clone, whose `.git` is a folder, or a work
/// tree whose `.git` names its git directory, which purlis's git runner reads only where it
/// takes one ([`crate::worktree::link`]).
pub fn a_repository(dir: &Path) -> bool {
    dir.join(".git").exists()
}

/// What one send hands the link: the refs to name, the branch to check out where nothing is
/// checked out yet, and the bundle, where there is anything to carry.
pub struct Sending<'a> {
    pub refs: Vec<Ref>,
    pub checkout: String,
    pub bundle: Option<&'a Path>,
}

/// **The desktop sends the branch `repo` is on to a runner** ([`bundle_branch`]), with its
/// uncommitted work less `leave_out`, leaving out what `have` (the runner's refs the last send
/// answered) reaches, through `through`: the link's `take_code`, which answers the refs the
/// runner holds after. A repository on no branch is refused in a sentence, and one that is not a
/// repository sends nothing. Answers those refs, and the changed or new files the snapshot left
/// out as a save to the forge would refuse them ([`Sent::left_out`]).
pub fn send(
    repo: &Path,
    leave_out: &[String],
    have: &[String],
    scratch: &Path,
    through: impl FnOnce(Sending<'_>) -> Result<Vec<Ref>, String>,
) -> Result<(Vec<Ref>, Vec<String>), Refused> {
    if !a_repository(repo) {
        return Ok((Vec::new(), Vec::new()));
    }
    let branch = current_branch(repo).ok_or_else(|| {
        not_allowed(format!(
            "{} is on no branch, so it was not sent to the runner",
            repo.display()
        ))
    })?;
    let into = scratch.join("send.bundle");
    let sent = bundle_branch(repo, Some((repo, leave_out)), &branch, have, &into, scratch)?;
    let bundle = (sent.bundled == Bundled::Wrote).then_some(into.as_path());
    let held = through(Sending {
        refs: sent.refs,
        checkout: format!("{HEADS}{branch}"),
        bundle,
    })
    .map_err(Refused::Git);
    let _ = std::fs::remove_file(&into);
    Ok((held?, sent.left_out))
}

/// **The desktop fetches a runner's branches back into `repo`** ([`take_back`]), as
/// `refs/remotes/<pool>/<branch>`, through `through`: the link's `give_code`, given what `repo`
/// already has and where to write the bundle, which answers the runner's branches and whether it
/// wrote one. A folder that is not a repository fetches nothing.
pub fn fetch_back(
    repo: &Path,
    pool: &str,
    scratch: &Path,
    through: impl FnOnce(Vec<String>, &Path) -> Result<(Vec<Ref>, bool), String>,
) -> Result<Vec<Ref>, Refused> {
    fetch_back_as(repo, pool, false, scratch, through)
}

/// [`fetch_back`] from a draining runner: what it has comes back beside what the pool's runner
/// now has, and removes none of it (#4248).
pub fn fetch_back_beside(
    repo: &Path,
    pool: &str,
    scratch: &Path,
    through: impl FnOnce(Vec<String>, &Path) -> Result<(Vec<Ref>, bool), String>,
) -> Result<Vec<Ref>, Refused> {
    fetch_back_as(repo, pool, true, scratch, through)
}

fn fetch_back_as(
    repo: &Path,
    pool: &str,
    beside: bool,
    scratch: &Path,
    through: impl FnOnce(Vec<String>, &Path) -> Result<(Vec<Ref>, bool), String>,
) -> Result<Vec<Ref>, Refused> {
    if !a_repository(repo) {
        return Ok(Vec::new());
    }
    let mut have: Vec<String> = refs_under(repo, &[HEADS, &format!("refs/remotes/{pool}/")])?
        .into_iter()
        .map(|one| one.oid)
        .collect();
    have.sort();
    have.dedup();
    let into = scratch.join("fetch.bundle");
    let (heads, wrote) = through(have, &into).map_err(Refused::Git)?;
    let bundle = wrote.then_some(into.as_path());
    let kept = take_back_as(repo, pool, beside, bundle, &heads);
    let _ = std::fs::remove_file(&into);
    kept
}

/// One branch a pool's runner has, as the desktop last fetched it ([`fetch_back`]): its name on
/// the runner, its commit, and how many commits it has that the desktop's branch of that name,
/// or what the desktop has checked out where it has none, does not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnARunner {
    pub branch: String,
    pub oid: String,
    pub ahead: u32,
}

/// **The branches of `pool`'s runner the desktop's `repo` keeps** (#2046): every
/// `refs/remotes/<pool>/<branch>` there, with what each has that this machine does not. Nothing for a
/// pool's name refs cannot be kept under, or a folder that is not a repository.
pub fn on_a_runner(repo: &Path, pool: &str) -> Vec<OnARunner> {
    if !pool_ok(pool) || !a_repository(repo) {
        return Vec::new();
    }
    let prefix = format!("refs/remotes/{pool}/");
    let Ok(kept) = refs_under(repo, &[prefix.as_str()]) else {
        return Vec::new();
    };
    kept.into_iter()
        .filter_map(|one| {
            let branch = one.name.strip_prefix(&prefix)?.to_owned();
            let here = format!("{HEADS}{branch}");
            let base = if holds_ref(repo, &here) {
                here
            } else {
                "HEAD".to_owned()
            };
            let counted = run(
                repo,
                &[
                    "rev-list",
                    "--count",
                    &format!("{base}..{}", one.name),
                    "--",
                ],
            )
            .ok()?;
            let ahead = counted.line().parse().unwrap_or_default();
            Some(OnARunner {
                branch,
                oid: one.oid,
                ahead,
            })
        })
        .collect()
}

fn holds_ref(repo: &Path, name: &str) -> bool {
    run(repo, &["rev-parse", "--verify", "--quiet", name]).is_ok_and(|ran| ran.ok())
}

/// **How a runner's session host moves code in the projects it keeps for its desktop** (#2046):
/// this module's [`receive`] and [`give`], in the repository's folder of the project the host
/// holds, which is never anywhere but the project's own folder or a workspace's repository in it
/// ([`repo_in`]). What `purlis serve` gives its host.
#[cfg(unix)]
pub struct Moves;

#[cfg(unix)]
fn code_refused(why: Refused) -> purlis_host::protocol::session::Refusal {
    use purlis_host::protocol::session;
    let code = match &why {
        Refused::NotAllowed(_) => session::NOT_ALLOWED,
        Refused::Git(_) => session::NOT_STARTED,
    };
    session::Refusal {
        code: code.into(),
        why: why.to_string(),
    }
}

#[cfg(unix)]
fn wire_refs(refs: Vec<Ref>) -> Vec<purlis_host::protocol::session::CodeRef> {
    refs.into_iter()
        .map(|one| purlis_host::protocol::session::CodeRef {
            name: one.name,
            oid: one.oid,
        })
        .collect()
}

#[cfg(unix)]
impl purlis_host::Code for Moves {
    fn take(
        &self,
        project: &Path,
        repo: &str,
        bundle: Option<&Path>,
        refs: &[purlis_host::protocol::session::CodeRef],
        checkout: Option<&str>,
    ) -> Result<Vec<purlis_host::protocol::session::CodeRef>, purlis_host::protocol::session::Refusal>
    {
        let at = repo_in(project, repo).map_err(code_refused)?;
        let refs: Vec<Ref> = refs
            .iter()
            .map(|one| Ref {
                name: one.name.clone(),
                oid: one.oid.clone(),
            })
            .collect();
        receive(&at, bundle, &refs, checkout)
            .map(wire_refs)
            .map_err(code_refused)
    }

    fn give(
        &self,
        project: &Path,
        repo: &str,
        have: &[String],
        into: &Path,
    ) -> Result<
        (Vec<purlis_host::protocol::session::CodeRef>, bool),
        purlis_host::protocol::session::Refusal,
    > {
        let at = repo_in(project, repo).map_err(code_refused)?;
        let (heads, bundled) = give(&at, have, into).map_err(code_refused)?;
        Ok((wire_refs(heads), bundled == Bundled::Wrote))
    }

    fn take_work(
        &self,
        project: &Path,
        repo: &str,
        bundle: Option<&Path>,
        refs: &[purlis_host::protocol::session::CodeRef],
        checkout: &str,
    ) -> Result<Vec<purlis_host::protocol::session::CodeRef>, purlis_host::protocol::session::Refusal>
    {
        let at = repo_in(project, repo).map_err(code_refused)?;
        let scratch = moves_scratch()?;
        let refs: Vec<Ref> = refs
            .iter()
            .map(|one| Ref {
                name: one.name.clone(),
                oid: one.oid.clone(),
            })
            .collect();
        receive_over(&at, scratch.path(), bundle, &refs, checkout)
            .map(wire_refs)
            .map_err(code_refused)
    }

    fn give_work(
        &self,
        project: &Path,
        repo: &str,
        have: &[String],
        into: &Path,
    ) -> Result<
        (Vec<purlis_host::protocol::session::CodeRef>, bool),
        purlis_host::protocol::session::Refusal,
    > {
        let at = repo_in(project, repo).map_err(code_refused)?;
        let scratch = moves_scratch()?;
        let (heads, bundled) = give_work(&at, scratch.path(), have, into).map_err(code_refused)?;
        Ok((wire_refs(heads), bundled == Bundled::Wrote))
    }

    fn take_conversation(
        &self,
        project: &Path,
        carried: &purlis_host::Carried,
        from: &Path,
    ) -> Result<(), purlis_host::protocol::session::Refusal> {
        of_a_chat(project, carried)
            .keep(&carried.file, from)
            .map_err(conversation_refused)
    }

    fn give_conversation(
        &self,
        project: &Path,
        carried: &purlis_host::Carried,
        into: &Path,
    ) -> Result<String, purlis_host::protocol::session::Refusal> {
        let found = of_a_chat(project, carried)
            .find()
            .map_err(conversation_refused)?;
        std::fs::copy(&found.path, into).map_err(|why| {
            conversation_refused(crate::conversationmove::Refused::Io(format!(
                "the conversation could not be read: {why}"
            )))
        })?;
        Ok(found.shape)
    }

    fn clones_of(
        &self,
        project: &Path,
        workspace: &str,
    ) -> Result<Vec<String>, purlis_host::protocol::session::Refusal> {
        clones_in(project, workspace).map_err(code_refused)
    }

    fn origins_of(
        &self,
        project: &Path,
        workspace: &str,
        repos: &[String],
    ) -> std::collections::BTreeMap<String, String> {
        origins_in(project, workspace, repos)
    }
}

#[cfg(unix)]
fn of_a_chat<'a>(
    project: &'a Path,
    carried: &'a purlis_host::Carried,
) -> crate::conversationmove::OfAChat<'a> {
    crate::conversationmove::OfAChat {
        root: project,
        workspace: &carried.workspace,
        profile: &carried.profile,
        conversation: &carried.conversation,
    }
}

#[cfg(unix)]
fn conversation_refused(
    why: crate::conversationmove::Refused,
) -> purlis_host::protocol::session::Refusal {
    use purlis_host::protocol::session;
    let code = match &why {
        crate::conversationmove::Refused::NotAllowed(_) => session::NOT_ALLOWED,
        _ => session::NOT_STARTED,
    };
    session::Refusal {
        code: code.into(),
        why: why.to_string(),
    }
}

#[cfg(unix)]
fn moves_scratch() -> Result<tempfile::TempDir, purlis_host::protocol::session::Refusal> {
    tempfile::Builder::new()
        .prefix("purlis-move-")
        .tempdir()
        .map_err(|why| purlis_host::protocol::session::Refusal {
            code: purlis_host::protocol::session::NOT_STARTED.into(),
            why: format!("there is no room to move the work: {why}"),
        })
}

fn run(dir: &Path, args: &[&str]) -> Result<git::Run, Refused> {
    git::run_untimed(dir, args).map_err(git_missing)
}

#[cfg(test)]
mod tests;
