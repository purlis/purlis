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

/// Where a branch's snapshot of uncommitted work is kept, in a repository either end holds.
pub const SNAPSHOTS: &str = "refs/purlis/snapshots/";

/// Where a branch is.
pub const HEADS: &str = "refs/heads/";

/// The branch a repository a runner made for the desktop starts on, which no bundle is taken
/// into: git never fetches into the branch a repository is on.
pub const UNBORN: &str = "purlis/held";

/// What every git call that reads a bundle runs with, ahead of its verb: no hook runs, every
/// object is checked as it arrives, no submodule is reached, no transport but a file (a bundle
/// is read through git's file transport), and nothing is collected as garbage on the way.
pub const FETCHES_WITH: [&str; 18] = [
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

/// Whether `name` is a pool's name a remote ref may be kept under, `refs/remotes/<pool>/`.
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

/// **A snapshot of `tree`'s uncommitted work**: a commit of the work tree as it is, its untracked
/// files that are not ignored included, whose parent is what `tree` has checked out. `None`
/// where nothing is uncommitted. Staged in an index of its own, so what is staged in `tree` is
/// left as it is, and with no attributes from the work tree, so no filter it names runs.
///
/// `leave_out` names folders and files, relative to the top of `tree`, that no snapshot carries,
/// changed or new: as they are committed, or not at all ([`plane_leaves_out`]). A snapshot of
/// nothing but those is none.
pub fn snapshot(
    tree: &Path,
    scratch: &Path,
    leave_out: &[String],
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
    let excluded: Vec<String> = leave_out
        .iter()
        .map(|left| format!(":(top,exclude,literal){left}"))
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
    let written = git::run_with_index(tree, &index, &["write-tree"]).map_err(git_missing)?;
    let _ = std::fs::remove_file(&index);
    if !written.ok() {
        return Err(git_failed("write the uncommitted work", &written));
    }
    let committed = run(tree, &["rev-parse", "--verify", "--quiet", "HEAD^{tree}"])?;
    if committed.ok() && committed.line() == written.line() {
        return Ok(None);
    }
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
            written.line(),
        ],
    )?;
    if !commit.ok() {
        return Err(git_failed("keep the uncommitted work", &commit));
    }
    Ok(Some(commit.line().to_owned()))
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
    let snapshot = match tree {
        Some((tree, leave_out)) => snapshot(tree, scratch, leave_out)?,
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
/// ref of `pool` that the runner no longer has is removed. No local branch, work tree or index is
/// touched. Answers the remote refs as they are after it.
pub fn take_back(
    repo: &Path,
    pool: &str,
    bundle: Option<&Path>,
    heads: &[Ref],
) -> Result<Vec<Ref>, Refused> {
    if !pool_ok(pool) {
        return Err(not_allowed(format!(
            "`{pool}` is not a pool's name refs can be kept under, so nothing was fetched"
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
        if !wanted.contains(&kept.name) {
            let ran = run(repo, &["update-ref", "-d", &kept.name])?;
            if !ran.ok() {
                return Err(git_failed("remove a branch the runner no longer has", &ran));
            }
        }
    }
    refs_under(repo, &[prefix.as_str()])
}

/// The branch `repo` is on, where it is on one with a commit.
pub fn current_branch(repo: &Path) -> Option<String> {
    let ran = run(repo, &["symbolic-ref", "--quiet", "--short", "HEAD"]).ok()?;
    let branch = ran.ok().then(|| ran.line().to_owned())?;
    let born = run(repo, &["rev-parse", "--verify", "--quiet", "HEAD"]).ok()?;
    born.ok().then_some(branch)
}

/// Whether `dir` is the top of a git repository of its own: a clone, whose `.git` is a folder.
pub fn a_repository(dir: &Path) -> bool {
    dir.join(".git").is_dir()
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
/// repository sends nothing.
pub fn send(
    repo: &Path,
    leave_out: &[String],
    have: &[String],
    scratch: &Path,
    through: impl FnOnce(Sending<'_>) -> Result<Vec<Ref>, String>,
) -> Result<Vec<Ref>, Refused> {
    if !a_repository(repo) {
        return Ok(Vec::new());
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
    held
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
    let kept = take_back(repo, pool, bundle, &heads);
    let _ = std::fs::remove_file(&into);
    kept
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
}

fn run(dir: &Path, args: &[&str]) -> Result<git::Run, Refused> {
    git::run_untimed(dir, args).map_err(git_missing)
}

#[cfg(test)]
mod tests;
