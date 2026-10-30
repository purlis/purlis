// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat's work tree, moved with the chat** (#2048): where a chat moves between the desktop
//! and a runner, the work tree of each clone it works in goes with it, its branch, its commits
//! and its uncommitted work, and the device it leaves keeps its copy as it was.
//!
//! **What both ends agreed on is kept** as `refs/purlis/moved/<branch>` ([`MOVED`]) in each
//! repository: a snapshot commit ([`work_of`]) of the work tree as one end sent it and the other
//! took it. A work tree is taken over only where nothing was done in it since ([`take_over`]):
//! it is still on that commit, with that work. One that has no such record is taken over only
//! where it has nothing uncommitted and every commit it is on is in what comes. Anything else is
//! refused whole, in a sentence, and nothing of the repository is changed: the chat stays where
//! it was, and the work the other device would have written over is kept.
//!
//! **How the work tree is moved**: git's own two-tree `read-tree -m -u`, from what was agreed to
//! what comes, with no attribute of the work tree read (so no filter it names runs) and no
//! submodule reached; then the branch is moved to the commit that comes, and the index to that
//! commit, so the work that came is uncommitted work again, as it was where it came from.

use std::path::Path;

use super::{
    HEADS, Ref, Refused, SNAPSHOTS, an_oid, carried, commit_work, empty_tree, git_failed,
    not_allowed, run, work_tree,
};

/// Where what both ends agreed of a branch's work tree is kept, in a repository either end holds.
pub const MOVED: &str = "refs/purlis/moved/";

/// Where a transfer's refs wait while a work tree is taken over (never carried).
const INCOMING: &str = "refs/purlis/incoming/";

/// **A work tree as it is now**: the branch it is on, the commit, and a snapshot commit of its
/// work, made whether or not anything is uncommitted, whose parent is that commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Work {
    /// The branch's short name.
    pub branch: String,
    pub head: String,
    pub snapshot: String,
}

/// **`tree`'s work as it is now**, less `leave_out` and what a snapshot never carries
/// ([`super::snapshot_leaving`]). A work tree on no branch is refused in a sentence.
pub fn work_of(tree: &Path, scratch: &Path, leave_out: &[String]) -> Result<Work, Refused> {
    let branch = super::current_branch(tree).ok_or_else(|| {
        not_allowed(format!(
            "{} is on no branch, so its work was not moved",
            tree.display()
        ))
    })?;
    carried(&format!("{HEADS}{branch}"))?;
    let head = commit_of(tree, "HEAD")?;
    let written = match work_tree(tree, scratch, leave_out, &mut Vec::new())? {
        Some(written) => written,
        None => tree_of(tree, "HEAD")?,
    };
    let snapshot = commit_work(tree, &written)?;
    Ok(Work {
        branch,
        head,
        snapshot,
    })
}

fn commit_of(repo: &Path, name: &str) -> Result<String, Refused> {
    let ran = run(
        repo,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{name}^{{commit}}"),
        ],
    )?;
    if !ran.ok() {
        return Err(git_failed(&format!("find {name}"), &ran));
    }
    Ok(ran.line().to_owned())
}

fn tree_of(repo: &Path, name: &str) -> Result<String, Refused> {
    let ran = run(
        repo,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{name}^{{tree}}"),
        ],
    )?;
    if !ran.ok() {
        return Err(git_failed(&format!("read {name}"), &ran));
    }
    Ok(ran.line().to_owned())
}

fn held(repo: &Path, name: &str) -> Result<Option<String>, Refused> {
    let ran = run(repo, &["rev-parse", "--verify", "--quiet", name])?;
    Ok(ran.ok().then(|| ran.line().to_owned()))
}

fn ancestor(repo: &Path, older: &str, newer: &str) -> bool {
    run(repo, &["merge-base", "--is-ancestor", older, newer]).is_ok_and(|ran| ran.ok())
}

/// The tree `repo`'s work tree is agreed to be at: the work tree is still what the last move
/// left it, or has nothing uncommitted and only commits in `to_head`. Refused in a sentence
/// otherwise.
fn agreed_tree(
    repo: &Path,
    scratch: &Path,
    leave_out: &[String],
    to_head: &str,
) -> Result<String, Refused> {
    let changed = || {
        not_allowed(format!(
            "{} has work the other device has not got: it changed after the chat's work last \
             moved, so it was left as it is",
            repo.display()
        ))
    };
    let branch = super::current_branch(repo).ok_or_else(changed)?;
    let head = commit_of(repo, "HEAD")?;
    let now = match work_tree(repo, scratch, leave_out, &mut Vec::new())? {
        Some(written) => written,
        None => tree_of(repo, "HEAD")?,
    };
    match held(repo, &format!("{MOVED}{branch}"))? {
        Some(moved) => {
            let base = commit_of(repo, &format!("{moved}^1")).map_err(|_| changed())?;
            if base == head && tree_of(repo, &moved)? == now {
                Ok(now)
            } else {
                Err(changed())
            }
        }
        None if now == tree_of(repo, "HEAD")? && ancestor(repo, &head, to_head) => Ok(now),
        None => Err(changed()),
    }
}

/// A copy of `repo`'s index, put back where a move does not finish.
struct IndexKept {
    at: std::path::PathBuf,
    copy: Option<std::path::PathBuf>,
}

impl IndexKept {
    fn of(repo: &Path, scratch: &Path) -> Result<IndexKept, Refused> {
        let ran = run(
            repo,
            &["rev-parse", "--path-format=absolute", "--git-path", "index"],
        )?;
        if !ran.ok() {
            return Err(git_failed("find the index", &ran));
        }
        let at = std::path::PathBuf::from(ran.line());
        let copy = scratch.join("index.kept");
        let copy = match std::fs::copy(&at, &copy) {
            Ok(_) => Some(copy),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => {
                return Err(Refused::Git(format!("the index could not be kept: {e}")));
            }
        };
        Ok(IndexKept { at, copy })
    }

    fn put_back(&self) {
        match &self.copy {
            Some(copy) => {
                let _ = std::fs::copy(copy, &self.at);
            }
            None => {
                let _ = std::fs::remove_file(&self.at);
            }
        }
    }
}

/// **Takes `repo`'s work tree over** to branch `to_branch` at `to_head`, with the uncommitted
/// work of `to_snapshot` (a snapshot whose parent is `to_head`), where it is still what both
/// ends last agreed ([`agreed_tree`], `leave_out` as its snapshots leave out). The objects of both
/// must be in `repo`. Afterwards the work tree is `to_snapshot`'s, the branch is checked out at
/// `to_head` with its index at it, and `to_snapshot` is what both ends agreed of `to_branch`.
/// Refused whole where the work tree is not as agreed, or where git would write over anything
/// it is not: nothing is changed then.
pub fn take_over(
    repo: &Path,
    scratch: &Path,
    leave_out: &[String],
    to_branch: &str,
    to_head: &str,
    to_snapshot: &str,
) -> Result<Vec<String>, Refused> {
    let to = format!("{HEADS}{to_branch}");
    carried(&to)?;
    if !an_oid(to_head) || !an_oid(to_snapshot) {
        return Err(not_allowed("a move names a commit by its full name"));
    }
    if commit_of(repo, &format!("{to_snapshot}^1"))? != to_head {
        return Err(not_allowed(format!(
            "the work that came for {to_branch} is not on its branch's commit, so it was not \
             taken"
        )));
    }
    let was = super::current_branch(repo);
    let from = agreed_tree(repo, scratch, leave_out, to_head)?;
    let to_tree = tree_of(repo, to_snapshot)?;
    let kept = IndexKept::of(repo, scratch)?;
    move_tree(repo, &from, &to_tree, &to, to_head, || kept.put_back())?;
    agree(repo, to_branch, to_snapshot, was.as_deref())?;
    Ok(to_watch(repo, &from, &to_tree))
}

/// **Moves `repo`'s work tree from tree `from` to tree `to_tree`**, then branch `to` to `to_head`
/// with the index at it, so what `to_tree` has beyond `to_head` is uncommitted work: git's own
/// two-tree `read-tree -m -u`, with no attribute of the work tree read (so no filter it names
/// runs), no submodule reached and no hook run. `undo` is called where the work tree did not move.
fn move_tree(
    repo: &Path,
    from: &str,
    to_tree: &str,
    to: &str,
    to_head: &str,
    undo: impl Fn(),
) -> Result<(), Refused> {
    let empty = empty_tree(repo)?;
    let attr = format!("--attr-source={empty}");
    let quiet = [
        "-c",
        "core.attributesFile=/dev/null",
        "-c",
        "submodule.recurse=false",
        "-c",
        "core.hooksPath=/dev/null",
        attr.as_str(),
    ];
    let steps: [Vec<&str>; 2] = [
        vec!["read-tree", from],
        vec!["read-tree", "-m", "-u", from, to_tree],
    ];
    for (at, step) in steps.into_iter().enumerate() {
        let mut args: Vec<&str> = quiet.to_vec();
        args.extend(step);
        let ran = run(repo, &args)?;
        if !ran.ok() {
            undo();
            return Err(git_failed("move the work tree", &ran));
        }
        if at == 0 {
            // The index read from a tree knows nothing of the files' times: git learns them
            // here, where every file is as that tree has it, so the move compares no content.
            let _ = run(repo, &["update-index", "-q", "--refresh"]);
        }
    }
    let message = "purlis: the chat's work moved here";
    let moved = [
        vec!["update-ref", "-m", message, to, to_head],
        vec!["symbolic-ref", "-m", message, "HEAD", to],
        vec!["read-tree", to_head],
    ];
    for step in moved {
        let ran = run(repo, &step)?;
        if !ran.ok() {
            return Err(git_failed("move the branch with its work", &ran));
        }
    }
    Ok(())
}

/// **Files a tool on this machine runs or reads as its settings**, by their place in a clone:
/// what a person looks at before running anything where a chat's work came back from another
/// machine (the person's ruling of 2026-10-27). Direnv's `.envrc`, the harnesses' own folders and
/// settings (`.claude/`, `.codex/`, `.cursor/`, `.mcp.json`, `opencode.json`), editors' workspace
/// settings (`.vscode/`, `.idea/`), git hook managers (`.husky/`, `lefthook.yml`,
/// `.pre-commit-config.yaml`), dev containers, and the files a build runs: `package.json` (its
/// scripts), `Makefile`, `justfile`, `Taskfile.yml`, `build.rs`.
pub fn runs_here(path: &str) -> bool {
    const FOLDERS: [&str; 8] = [
        ".claude/",
        ".codex/",
        ".cursor/",
        ".vscode/",
        ".idea/",
        ".husky/",
        ".devcontainer/",
        ".githooks/",
    ];
    const NAMES: [&str; 14] = [
        ".envrc",
        ".mcp.json",
        "opencode.json",
        "package.json",
        "makefile",
        "gnumakefile",
        "justfile",
        "taskfile.yml",
        "taskfile.yaml",
        "lefthook.yml",
        ".pre-commit-config.yaml",
        "build.rs",
        ".npmrc",
        ".tool-versions",
    ];
    let lower = path.to_ascii_lowercase();
    let name = lower.rsplit('/').next().unwrap_or(&lower);
    FOLDERS
        .iter()
        .any(|folder| lower.starts_with(folder) || lower.contains(&format!("/{folder}")))
        || NAMES.contains(&name)
}

/// **What came onto `repo`'s work tree, moving it from tree `from` to tree `to`, that the person
/// is to look at** (the person's ruling of 2026-10-27): every changed or new file a local tool
/// runs ([`runs_here`]), every one a save to the forge refuses
/// ([`crate::reposave::refused_as`]), and every link, which may lead out of the clone. By path.
fn to_watch(repo: &Path, from: &str, to: &str) -> Vec<String> {
    let Ok(changed) = run(
        repo,
        &[
            "diff-tree",
            "-r",
            "-z",
            "--no-renames",
            "--diff-filter=AMT",
            "--raw",
            from,
            to,
        ],
    ) else {
        return Vec::new();
    };
    let mut watch = Vec::new();
    let mut fields = changed.out.split('\0');
    while let (Some(meta), Some(path)) = (fields.next(), fields.next()) {
        if path.is_empty() {
            continue;
        }
        // `:<old mode> <new mode> <old oid> <new oid> <status>`
        let parts: Vec<&str> = meta.trim_start_matches(':').split(' ').collect();
        let (mode, oid) = (parts.get(1).copied(), parts.get(3).copied());
        let link = mode == Some("120000");
        let refused = oid.is_some_and(|oid| {
            run(repo, &["cat-file", "blob", oid])
                .ok()
                .is_some_and(|blob| crate::reposave::refused_as(path, &blob.out).is_some())
        });
        if link || refused || runs_here(path) {
            watch.push(path.to_owned());
        }
    }
    watch
}

/// Keeps `snapshot` as what both ends agreed of `branch`, and forgets what was agreed of `was`,
/// the branch the work tree was on before, where it is another.
fn agree(repo: &Path, branch: &str, snapshot: &str, was: Option<&str>) -> Result<(), Refused> {
    let ran = run(repo, &["update-ref", &format!("{MOVED}{branch}"), snapshot])?;
    if !ran.ok() {
        return Err(git_failed("keep what was moved", &ran));
    }
    if let Some(was) = was.filter(|was| *was != branch) {
        let _ = run(repo, &["update-ref", "-d", &format!("{MOVED}{was}")]);
    }
    Ok(())
}

/// **Agrees `work` as moved from `repo`**: the desktop's record once the runner took its work
/// tree over, so the chat's work coming back is taken only onto this work tree as it was then.
pub fn sent(repo: &Path, work: &Work) -> Result<(), Refused> {
    agree(repo, &work.branch, &work.snapshot, None)
}

/// **What a runner gives of its work tree with its branches** (`give_code` with its work,
/// #2048): every branch, and the snapshot of the work tree's own branch as
/// `refs/purlis/snapshots/<branch>`, which both ends then agree on, bundled into `into` leaving
/// out what `have` reaches. Answers the refs, and whether there was anything to send. Where the
/// runner holds no repository there, a clone made on the desktop's side only (#3454), it answers
/// no refs and nothing to send: the desktop's clone stays as it is ([`take_work_back`]).
pub fn give_work(
    repo: &Path,
    scratch: &Path,
    have: &[String],
    into: &Path,
) -> Result<(Vec<Ref>, super::Bundled), Refused> {
    if !repo.join(".git").exists() {
        return Ok((Vec::new(), super::Bundled::Nothing));
    }
    let work = work_of(repo, scratch, &[])?;
    let kept = format!("{SNAPSHOTS}{}", work.branch);
    let ran = run(repo, &["update-ref", &kept, &work.snapshot])?;
    if !ran.ok() {
        return Err(git_failed("keep the work for its bundle", &ran));
    }
    let mut refs = super::refs_under(repo, &[HEADS])?;
    refs.push(Ref {
        name: kept,
        oid: work.snapshot.clone(),
    });
    let names: Vec<&str> = refs.iter().map(|one| one.name.as_str()).collect();
    let bundled = super::bundle(repo, &names, have, into)?;
    agree(repo, &work.branch, &work.snapshot, None)?;
    Ok((refs, bundled))
}

/// **The refs a bundle at `bundle` carries, fetched under [`INCOMING`]** in `repo`, where no
/// branch, work tree or index is touched: only what `named` names, each where it says, or the
/// bundle is refused whole. Without a bundle, every named commit must be in `repo` already.
fn fetch_incoming(repo: &Path, bundle: Option<&Path>, named: &[Ref]) -> Result<(), Refused> {
    let Some(path) = bundle else {
        for one in named {
            if commit_of(repo, &one.oid).is_err() {
                return Err(not_allowed(format!(
                    "{} is not here and nothing carried it, so the work was not moved",
                    one.oid
                )));
            }
        }
        return Ok(());
    };
    let carries = super::heads_of(repo, path)?;
    for one in &carries {
        if !named.contains(one) {
            return Err(not_allowed(format!(
                "the bundle carries `{}`, which was not named, so no work was taken from it",
                one.name
            )));
        }
    }
    let path = path
        .to_str()
        .ok_or_else(|| Refused::Git("the bundle's path is not text".into()))?;
    let mut args: Vec<String> = super::FETCHES_WITH
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
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
        let rest = one
            .name
            .strip_prefix("refs/")
            .ok_or_else(|| not_allowed(format!("`{}` is not a ref", one.name)))?;
        args.push(format!("+{}:{INCOMING}{rest}", one.name));
    }
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let ran = super::git::run_network(repo, None, &args).map_err(super::git_missing)?;
    if !ran.ok() {
        return Err(git_failed("take the work's bundle", &ran));
    }
    Ok(())
}

/// Forgets every ref [`fetch_incoming`] fetched.
fn drop_incoming(repo: &Path) {
    if let Ok(refs) = super::refs_under(repo, &[INCOMING]) {
        for one in refs {
            let _ = run(repo, &["update-ref", "-d", &one.name]);
        }
    }
}

/// **The runner takes the desktop's work over** (`take_code` taking the work tree over, #2048):
/// `refs` must be the branch `checkout` and its snapshot, from the bundle at `bundle` or from
/// what `repo` holds. A repository with nothing checked out is checked out on them fresh
/// ([`super::check_out_fresh`]); one with a work tree is taken over ([`take_over`]). Either way
/// what came is what both ends agree on after. Answers every branch and snapshot `repo` holds.
pub fn receive_over(
    repo: &Path,
    scratch: &Path,
    bundle: Option<&Path>,
    refs: &[Ref],
    checkout: &str,
) -> Result<Vec<Ref>, Refused> {
    let branch = checkout
        .strip_prefix(HEADS)
        .ok_or_else(|| not_allowed(format!("`{checkout}` is not a branch to move work onto")))?;
    let snapshot_name = format!("{SNAPSHOTS}{branch}");
    let head = refs
        .iter()
        .find(|one| one.name == checkout)
        .ok_or_else(|| not_allowed(format!("the move names no commit for {checkout}")))?;
    let snapshot = refs
        .iter()
        .find(|one| one.name == snapshot_name)
        .ok_or_else(|| not_allowed(format!("the move names no work for {checkout}")))?;
    if refs.len() != 2 {
        return Err(not_allowed(
            "a move carries one branch and its work, and nothing else",
        ));
    }
    for one in refs {
        carried(&one.name)?;
        if !an_oid(&one.oid) {
            return Err(not_allowed(format!(
                "`{}` is not a commit's full name, so no work was moved",
                one.oid
            )));
        }
    }
    super::made(repo)?;
    let fresh = !run(repo, &["rev-parse", "--verify", "--quiet", "HEAD"])?.ok();
    let moved = fetch_incoming(repo, bundle, refs).and_then(|()| {
        if fresh {
            for one in refs {
                let ran = run(repo, &["update-ref", &one.name, &one.oid])?;
                if !ran.ok() {
                    return Err(git_failed(&format!("move {}", one.name), &ran));
                }
            }
            super::check_out_fresh(repo, checkout, Some(&snapshot.oid))?;
            agree(repo, branch, &snapshot.oid, None)
        } else {
            take_over(repo, scratch, &[], branch, &head.oid, &snapshot.oid)?;
            let ran = run(repo, &["update-ref", &snapshot_name, &snapshot.oid])?;
            if !ran.ok() {
                return Err(git_failed("keep the work that came", &ran));
            }
            Ok(())
        }
    });
    drop_incoming(repo);
    moved?;
    super::held_refs(repo)
}

/// **The desktop takes a runner chat's work back** (#2048): the runner's branches under
/// `refs/remotes/<pool>/` as a fetch does ([`super::take_back`]), then this clone's work tree
/// taken over onto the runner's work tree's branch, `heads`' snapshot (`refs/purlis/snapshots/
/// <branch>`, exactly one), with the work in it ([`take_over`], `leave_out` as this clone's
/// sends leave out). Refused whole, nothing of the work tree changed, where this clone changed
/// since its work went to the runner. Where the runner holds no copy of the clone (no refs at
/// all, [`give_work`]), it was made on this side only, and stays as it is (#3454).
pub fn take_work_back(
    repo: &Path,
    scratch: &Path,
    leave_out: &[String],
    pool: &str,
    bundle: Option<&Path>,
    heads: &[Ref],
) -> Result<Vec<String>, Refused> {
    if heads.is_empty() && bundle.is_none() {
        return Ok(Vec::new());
    }
    let (branch, head, snapshot, branches) = work_named(heads)?;
    let moved = fetch_incoming(repo, bundle, heads).and_then(|()| {
        // The runner's branches, as a fetch keeps them, from what came.
        super::take_back(repo, pool, None, &branches)?;
        take_over(repo, scratch, leave_out, &branch, &head.oid, &snapshot.oid)
    });
    drop_incoming(repo);
    moved
}

/// **What a runner gave of its work tree, as [`give_work`] names it**: exactly one snapshot,
/// `refs/purlis/snapshots/<branch>`, with its branch among `heads`, every name one a move
/// carries and every commit named in full. Answers the branch's short name, its head, its
/// snapshot, and every branch.
fn work_named(heads: &[Ref]) -> Result<(String, Ref, Ref, Vec<Ref>), Refused> {
    let snapshots: Vec<&Ref> = heads
        .iter()
        .filter(|one| one.name.starts_with(SNAPSHOTS))
        .collect();
    let [snapshot] = snapshots.as_slice() else {
        return Err(not_allowed(
            "the runner sent no work, or more than one, so none was taken",
        ));
    };
    let branch = snapshot
        .name
        .strip_prefix(SNAPSHOTS)
        .unwrap_or_default()
        .to_owned();
    let head = heads
        .iter()
        .find(|one| one.name == format!("{HEADS}{branch}"))
        .ok_or_else(|| {
            not_allowed(format!(
                "the runner sent work for {branch} without the branch, so none was taken"
            ))
        })?;
    for one in heads {
        carried(&one.name)?;
        if !an_oid(&one.oid) {
            return Err(not_allowed(format!(
                "the runner named `{}` for {}, which is not a commit's full name",
                one.oid, one.name
            )));
        }
    }
    let branches: Vec<Ref> = heads
        .iter()
        .filter(|one| one.name.starts_with(HEADS))
        .cloned()
        .collect();
    Ok((branch, head.clone(), (*snapshot).clone(), branches))
}

/// **The desktop makes a clone the runner chat made, fresh** (#3811): where this machine has
/// nothing at `repo`, a repository is made there, the runner's branches kept under
/// `refs/remotes/<pool>/` as a fetch keeps them, and its work tree checked out on the runner's
/// work tree's branch with the work in it, as [`take_over`] moves one from nothing: no
/// attribute read, no submodule reached, no hook run, and git writes no path through a link.
/// What came is what both ends agree on after. Answers what came that the person is to look
/// at, every file of it a local tool runs and every link ([`to_watch`] from the empty tree).
/// Refused where anything is at `repo` already, which is never written into; a clone that does
/// not come leaves nothing at `repo`. No remote of the runner's comes with it.
pub fn take_work_fresh(
    repo: &Path,
    pool: &str,
    bundle: Option<&Path>,
    heads: &[Ref],
) -> Result<Vec<String>, Refused> {
    let (branch, head, snapshot, branches) = work_named(heads)?;
    if std::fs::symlink_metadata(repo).is_ok() {
        return Err(not_allowed(format!(
            "{} is here already, so the runner's clone of it was not made here",
            repo.display()
        )));
    }
    let made = super::made(repo).and_then(|()| {
        fetch_incoming(repo, bundle, heads)?;
        if commit_of(repo, &format!("{}^1", snapshot.oid))? != head.oid {
            return Err(not_allowed(format!(
                "the work that came for {branch} is not on its branch's commit, so it was not \
                 taken"
            )));
        }
        super::take_back(repo, pool, None, &branches)?;
        let empty = empty_tree(repo)?;
        let to_tree = tree_of(repo, &snapshot.oid)?;
        move_tree(repo, &empty, &to_tree, &head.name, &head.oid, || {})?;
        agree(repo, &branch, &snapshot.oid, None)?;
        Ok(to_watch(repo, &empty, &to_tree))
    });
    if made.is_err() {
        // Only what this made: nothing was at `repo` before it.
        let _ = std::fs::remove_dir_all(repo);
    } else {
        drop_incoming(repo);
    }
    made
}

/// **The desktop bundles `work` of `repo` for a move** into `into`: the branch at its commit and
/// the work as `refs/purlis/snapshots/<branch>`, leaving out what `have` reaches. The snapshot's
/// ref is kept only while the bundle is made. Answers the refs a `take_code` names.
pub fn bundle_work(
    repo: &Path,
    work: &Work,
    have: &[String],
    into: &Path,
) -> Result<(Vec<Ref>, super::Bundled), Refused> {
    let refs = vec![
        Ref {
            name: format!("{HEADS}{}", work.branch),
            oid: work.head.clone(),
        },
        Ref {
            name: format!("{SNAPSHOTS}{}", work.branch),
            oid: work.snapshot.clone(),
        },
    ];
    let kept = &refs[1].name;
    let ran = run(repo, &["update-ref", kept, &work.snapshot])?;
    if !ran.ok() {
        return Err(git_failed("keep the work for its bundle", &ran));
    }
    let names: Vec<&str> = refs.iter().map(|one| one.name.as_str()).collect();
    let bundled = super::bundle(repo, &names, have, into);
    let _ = run(repo, &["update-ref", "-d", kept]);
    Ok((refs, bundled?))
}

#[cfg(test)]
mod tests;
