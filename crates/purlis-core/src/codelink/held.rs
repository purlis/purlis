// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What a runner holds, and the proof that the desktop has it** (1.69, #4569): a chat's fresh
//! runner goes only once its desktop has every commit it holds.
//!
//! **The runner lists everything** ([`repos_held`]): every repository in every project it keeps,
//! wherever it is in that project and whatever state it is in (on no branch, mid-rebase, nested
//! in another, a submodule's work tree, a bare one, one named like a folder a workspace keeps),
//! with each ref but another repository's branches as last fetched (`refs/remotes/…`) and
//! purlis's own record of what a desktop sent it (`refs/purlis/…`), `HEAD`, and each file of its
//! work tree that no commit has, with what that file holds. What it could not look through it
//! says, so an answer is whole only where that is empty.
//!
//! **The desktop proves it has each of them** ([`not_home`]): the repository at the same path in
//! its own project reaches every one of those commits from one of its refs, and holds each of
//! those files as the runner does: the desktop's own uncommitted work, sent to the runner as a
//! chat started, is no loss. Anything else — a repository of another project, one with no
//! counterpart here, a commit no ref here reaches, a file the runner changed that differs here, a
//! list that is not whole — is said, in a sentence, and the runner stays.
//! That is what turns every way a task's work could be missed into a runner that is kept.

use std::path::Path;

use super::{Ref, run};

/// One repository a runner holds: its project, its path in it, its refs and whether its work tree
/// holds a change no commit has.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Held {
    pub project: String,
    /// `/`-separated, empty for the project's own repository.
    pub path: String,
    pub refs: Vec<Ref>,
    /// Each file its work tree holds that differs from what it has checked out.
    pub changed: Vec<Changed>,
}

/// The most repositories a runner lists: past it, the list is not whole.
pub const MOST_REPOS: usize = 1000;

/// The most folders a runner looks through: past it, the list is not whole.
pub const MOST_FOLDERS: usize = 200_000;

/// The ref under which `HEAD` is listed: the commit checked out, on a branch or on none.
pub const HEAD: &str = "HEAD";

/// The ref a repository's newest stash is under.
const STASH: &str = "refs/stash";

/// The most stashes a repository lists: past it, its list is not whole.
const MOST_STASHES: usize = 1000;

/// **Every repository in the projects kept in `projects`** (1.69, #4569), each project a folder
/// named by its id, and what could not be looked through, in sentences. No link is followed, and
/// nothing inside a repository's own git folder is looked through.
pub fn repos_held(projects: &Path) -> (Vec<Held>, Vec<String>) {
    let mut held = Vec::new();
    let mut unread = Vec::new();
    let entries = match std::fs::read_dir(projects) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (held, unread),
        Err(e) => {
            unread.push(format!("the runner's projects could not be read: {e}"));
            return (held, unread);
        }
    };
    let mut folders = 0usize;
    let mut ids: Vec<String> = Vec::new();
    for entry in entries {
        // What can't be read or named is said, never passed over (D-4569-22).
        match entry.map(|entry| entry.file_name().into_string()) {
            // A transfer's scratch, which a host keeps beside its projects and removes as it ends,
            // is named hidden; no project's folder is.
            Ok(Ok(id)) if id.starts_with('.') => {}
            Ok(Ok(id)) => ids.push(id),
            Ok(Err(_)) => unread.push(
                "a project folder on the runner has a name that is not text, so it was not looked \
                 through"
                    .to_owned(),
            ),
            Err(e) => unread.push(format!("the runner's projects could not all be read: {e}")),
        }
    }
    ids.sort();
    for id in ids {
        let top = projects.join(&id);
        if !std::fs::symlink_metadata(&top).is_ok_and(|meta| meta.is_dir()) {
            continue;
        }
        let mut stack = vec![String::new()];
        while let Some(rel) = stack.pop() {
            folders += 1;
            if folders > MOST_FOLDERS {
                unread.push(format!(
                    "the runner holds more than {MOST_FOLDERS} folders, so not all were looked \
                     through"
                ));
                return (held, unread);
            }
            let dir = if rel.is_empty() {
                top.clone()
            } else {
                top.join(&rel)
            };
            let kind = repo_kind(&dir);
            if kind != Kind::None {
                if held.len() >= MOST_REPOS {
                    unread.push(format!(
                        "the runner holds more than {MOST_REPOS} repositories, so not all were \
                         listed"
                    ));
                    return (held, unread);
                }
                match one(&dir, &id, &rel, kind) {
                    Ok(one) => held.push(one),
                    Err(why) => unread.push(why),
                }
                // A bare repository's folders are all its own git folder.
                if kind == Kind::Bare {
                    continue;
                }
            }
            let entries = match std::fs::read_dir(&dir) {
                Ok(entries) => entries,
                // Gone while it was looked through: nothing is there to hold.
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => {
                    unread.push(format!("{} could not be read: {e}", shown(&id, &rel)));
                    continue;
                }
            };
            for entry in entries {
                let entry = match entry {
                    Ok(entry) => entry,
                    Err(e) => {
                        unread.push(format!("{} could not all be read: {e}", shown(&id, &rel)));
                        continue;
                    }
                };
                // `file_type` does not follow a link.
                match entry.file_type() {
                    Ok(kind) if kind.is_dir() => {}
                    Ok(_) => continue,
                    Err(e) => {
                        unread.push(format!("{} could not all be read: {e}", shown(&id, &rel)));
                        continue;
                    }
                }
                let Ok(name) = entry.file_name().into_string() else {
                    unread.push(format!(
                        "a folder in {} has a name that is not text, so it was not looked through",
                        shown(&id, &rel)
                    ));
                    continue;
                };
                if name == ".git" {
                    continue;
                }
                stack.push(if rel.is_empty() {
                    name
                } else {
                    format!("{rel}/{name}")
                });
            }
        }
    }
    held.sort_by(|a, b| (&a.project, &a.path).cmp(&(&b.project, &b.path)));
    (held, unread)
}

/// The most files outside the projects a runner names: the rest are counted.
const MOST_NAMED_OUTSIDE: usize = 5;

/// The most sentences of what could not be looked at a runner's home gives: the rest are counted.
const MOST_SAID_OUTSIDE: usize = 20;

/// How long before a host first starts a program a file still counts as changed since: a file
/// system's clock stamps a change coarsely, and nothing of a task runs before its program is
/// asked for.
pub const CHANGED_WITHIN: std::time::Duration = std::time::Duration::from_secs(2);

/// Folders of a runner's home a tool keeps by its own default, by their path in it, which hold
/// nothing a task made: Go's module and build cache under its default `GOPATH`.
const TOOLS_KEEP: &[&str] = &["go/pkg"];

/// **What a task keeps outside its project's folder on its runner** (#4781), in sentences: each
/// file in the runner's `home` changed since `since`, when the host first started a program, so
/// in no repository and unable to come home. Each keeps the runner, and the window's Notice names
/// it. None where no program has started yet: nothing of a task can be there.
///
/// Only what a person would look for is looked through: a hidden entry of the home (`.config`,
/// `.cache`, `.local` and the like) is a program's own settings, cache or record by the home's
/// convention, purlis's own data home among them, and is left out, as is every folder in `skip`
/// and each one a tool keeps by its own default ([`TOOLS_KEEP`]). A temporary folder is outside
/// the home and temporary on any machine. Only a file or a link counts, never a socket or a pipe a
/// program left. Each folder is opened from the one it is in, never through a link, so nothing
/// outside the home is looked through however its folders change meanwhile. A file is known
/// changed by its status change time, which no program can set back, so a copy that keeps its
/// times counts too.
#[cfg(unix)]
pub fn outside_the_projects(
    home: &Path,
    skip: &[std::path::PathBuf],
    since: Option<std::time::SystemTime>,
) -> Vec<String> {
    use rustix::fs::{AtFlags, CWD, Mode, OFlags};
    let Some(since) = since else {
        return Vec::new();
    };
    let folder = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC;
    let mut named = std::collections::BTreeSet::new();
    let mut found = 0usize;
    let mut said: Vec<String> = Vec::new();
    let mut unsaid = 0usize;
    let mut say = |sentence: String, said: &mut Vec<String>| {
        if said.len() < MOST_SAID_OUTSIDE {
            said.push(sentence);
        } else {
            unsaid += 1;
        }
    };
    let top = match rustix::fs::openat(CWD, home, folder, Mode::empty()) {
        Ok(top) => top,
        Err(rustix::io::Errno::NOENT) => return Vec::new(),
        Err(e) => {
            return vec![format!(
                "the runner's home could not be read, so what a task kept outside its project \
                 could not be looked for: {e}"
            )];
        }
    };
    let mut folders = 0usize;
    let mut stack = vec![(String::new(), top)];
    while let Some((rel, fd)) = stack.pop() {
        folders += 1;
        if folders > MOST_FOLDERS {
            say(
                format!(
                    "the runner's home holds more than {MOST_FOLDERS} folders, so not all were \
                     looked through"
                ),
                &mut said,
            );
            break;
        }
        let shown_dir = if rel.is_empty() {
            "~".to_owned()
        } else {
            format!("~/{rel}")
        };
        let mut dir = match rustix::fs::Dir::read_from(&fd) {
            Ok(dir) => dir,
            Err(e) => {
                say(
                    format!("{shown_dir} on the runner could not be read: {e}"),
                    &mut said,
                );
                continue;
            }
        };
        while let Some(entry) = dir.read() {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    say(
                        format!("{shown_dir} on the runner could not all be read: {e}"),
                        &mut said,
                    );
                    break;
                }
            };
            let raw = entry.file_name();
            let bytes = raw.to_bytes();
            if bytes == b"." || bytes == b".." || (rel.is_empty() && bytes.starts_with(b".")) {
                continue;
            }
            let Ok(name) = std::str::from_utf8(bytes) else {
                say(
                    format!(
                        "a file in {shown_dir} on the runner has a name that is not text, so it \
                         was not looked at"
                    ),
                    &mut said,
                );
                continue;
            };
            let shown = if rel.is_empty() {
                name.to_owned()
            } else {
                format!("{rel}/{name}")
            };
            if TOOLS_KEEP.contains(&shown.as_str()) || skip.contains(&home.join(&shown)) {
                continue;
            }
            // Never through a link: one made since counts as a file.
            let stat = match rustix::fs::statat(&fd, raw, AtFlags::SYMLINK_NOFOLLOW) {
                Ok(stat) => stat,
                Err(rustix::io::Errno::NOENT) => continue,
                Err(e) => {
                    say(
                        format!("~/{shown} on the runner could not be read: {e}"),
                        &mut said,
                    );
                    continue;
                }
            };
            match rustix::fs::FileType::from_raw_mode(stat.st_mode) {
                rustix::fs::FileType::Directory => {
                    match rustix::fs::openat(&fd, raw, folder | OFlags::NOFOLLOW, Mode::empty()) {
                        Ok(inner) => stack.push((shown, inner)),
                        Err(rustix::io::Errno::NOENT) => {}
                        Err(e) => say(
                            format!("~/{shown} on the runner could not be read: {e}"),
                            &mut said,
                        ),
                    }
                }
                rustix::fs::FileType::RegularFile | rustix::fs::FileType::Symlink => {
                    let changed = u64::try_from(stat.st_ctime)
                        .ok()
                        .zip(u32::try_from(stat.st_ctime_nsec).ok())
                        .and_then(|(secs, nanos)| {
                            std::time::UNIX_EPOCH.checked_add(std::time::Duration::new(secs, nanos))
                        });
                    if changed.is_none_or(|at| at >= since) {
                        found += 1;
                        named.insert(shown);
                        if named.len() > MOST_NAMED_OUTSIDE {
                            named.pop_last();
                        }
                    }
                }
                _ => {}
            }
        }
    }
    if unsaid > 0 {
        said.push(format!(
            "and {unsaid} more parts of the runner's home could not be looked at"
        ));
    }
    if found > 0 {
        let more = found - named.len();
        let named: Vec<String> = named.iter().map(|one| format!("~/{one}")).collect();
        said.insert(
            0,
            format!(
                "the task saved files on the runner outside its project, which purlis can't \
                 bring here: {}{}. Fetch again brings back only what is in the project, so \
                 discard the runner once you no longer need them",
                named.join(", "),
                if more > 0 {
                    format!(" and {more} more")
                } else {
                    String::new()
                }
            ),
        );
    }
    said
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    None,
    /// A work tree: its `.git` is a folder, or a file naming its git folder.
    WorkTree,
    /// A repository with no work tree.
    Bare,
}

fn repo_kind(dir: &Path) -> Kind {
    if std::fs::symlink_metadata(dir.join(".git")).is_ok() {
        return Kind::WorkTree;
    }
    let is = |name: &str, dir_wanted: bool| {
        std::fs::symlink_metadata(dir.join(name)).is_ok_and(|meta| {
            if dir_wanted {
                meta.is_dir()
            } else {
                meta.is_file()
            }
        })
    };
    if is("HEAD", false) && is("objects", true) && is("refs", true) {
        return Kind::Bare;
    }
    Kind::None
}

fn shown(project: &str, rel: &str) -> String {
    if rel.is_empty() {
        format!("project {project}'s own folder")
    } else {
        format!("{rel} in project {project}")
    }
}

/// The repository at `dir`, `rel` in project `project`.
fn one(dir: &Path, project: &str, rel: &str, kind: Kind) -> Result<Held, String> {
    let failed = |what: &str, said: &str| {
        let said = said.trim();
        if said.is_empty() {
            format!("the refs of {} could not be {what}", shown(project, rel))
        } else {
            format!(
                "the refs of {} could not be {what}: {said}",
                shown(project, rel)
            )
        }
    };
    let listed = run(
        dir,
        &[
            "for-each-ref",
            "--format=%(objectname) %(*objectname) %(objecttype) %(*objecttype) %(refname)",
        ],
    )
    .map_err(|why| failed("read", &why.to_string()))?;
    if !listed.ok() {
        return Err(failed("read", &listed.err));
    }
    let mut refs = Vec::new();
    for line in listed.out.lines() {
        let mut parts = line.splitn(5, ' ');
        let (Some(oid), Some(peeled), Some(kind), Some(peeled_kind), Some(name)) = (
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
        ) else {
            return Err(failed("read", &format!("git listed `{line}`")));
        };
        // Another repository's branches, and what a desktop sent (its snapshots among them),
        // whose desktop deletes its own copy once sent: neither is work made here.
        if name.starts_with("refs/remotes/") || name.starts_with("refs/purlis/") {
            continue;
        }
        // A tag is the commit it is of; a ref to no commit holds no work.
        let commit = match (kind, peeled_kind) {
            ("commit", _) => oid,
            ("tag", "commit") => peeled,
            _ => continue,
        };
        refs.push(Ref {
            name: name.to_owned(),
            oid: commit.to_owned(),
        });
    }
    // **Every stash, not only the newest** (#4781): `refs/stash` names the last one, and those
    // under it live only in its log, which no fetch carries.
    // Asked of the ref itself, so a stash that is no commit, which for-each-ref passes over,
    // still has its log read, or the list is not whole.
    let stashed = run(dir, &["rev-parse", "--verify", "--quiet", STASH])
        .map_err(|why| failed("read", &why.to_string()))?;
    if stashed.ok() && !stashed.line().is_empty() {
        let logged = run(dir, &["log", "--walk-reflogs", "--format=%H", STASH, "--"])
            .map_err(|why| failed("read", &why.to_string()))?;
        if !logged.ok() {
            return Err(failed("read", &logged.err));
        }
        if logged.out.lines().count() > MOST_STASHES {
            return Err(failed(
                "all listed",
                &format!("it holds more than {MOST_STASHES} stashes"),
            ));
        }
        for (n, oid) in logged.out.lines().enumerate().skip(1) {
            refs.push(Ref {
                name: format!("{STASH}@{{{n}}}"),
                oid: oid.trim().to_owned(),
            });
        }
    }
    let head = run(dir, &["rev-parse", "--verify", "--quiet", "HEAD^{commit}"])
        .map_err(|why| failed("read", &why.to_string()))?;
    if head.ok() && !head.line().is_empty() {
        refs.push(Ref {
            name: HEAD.to_owned(),
            oid: head.line().to_owned(),
        });
    }
    let changed = if kind == Kind::WorkTree {
        uncommitted(dir, project, rel)?
    } else {
        Vec::new()
    };
    Ok(Held {
        project: project.to_owned(),
        path: rel.to_owned(),
        refs,
        changed,
    })
}

/// The most uncommitted files a repository lists: past it, its list is not whole.
pub const MOST_CHANGED: usize = 20_000;

/// **Each file of the work tree at `dir` that differs from what it has checked out**, with what
/// it holds now ([`file_now`]): a tracked file changed or gone, or a file that is not tracked:
/// in a clone, only one no ignore rule covers; in the project's own repository, any, since what
/// it ignores (a workspace's folder above all) is in no repository at all. A repository inside it is listed on its own, so its folder counts for
/// nothing here (each untracked file is named, so such a folder is named alone), and neither does
/// what no snapshot of a project's own repository carries ([`super::plane_leaves_out`]).
fn uncommitted(dir: &Path, project: &str, rel: &str) -> Result<Vec<Changed>, String> {
    let unread = |why: String| {
        format!(
            "what {} holds that no commit has could not be read: {why}",
            shown(project, rel)
        )
    };
    // In the project's own repository, what it ignores counts too: a workspace's folder and
    // what a task writes there are ignored by it, and are in no clone (the person's ruling of
    // 2026-11-02). In a clone, what git ignores is left out, as git leaves it out.
    let mut args = vec![
        "status",
        "--porcelain=v1",
        "-z",
        "--no-renames",
        "--untracked-files=all",
        "--ignore-submodules=all",
    ];
    if rel.is_empty() {
        args.push("--ignored=traditional");
    }
    let status = run(dir, &args).map_err(|why| unread(why.to_string()))?;
    if !status.ok() {
        return Err(unread(status.err.trim().to_owned()));
    }
    let left_out = if rel.is_empty() {
        super::plane_leaves_out()
    } else {
        Vec::new()
    };
    let mut paths = Vec::new();
    for entry in status.out.split('\0').filter(|entry| entry.len() > 3) {
        let path = entry[3..].trim_end_matches('/');
        // A name that is not text can't be found again, here or there (D-4569-22).
        if path.contains(char::REPLACEMENT_CHARACTER) {
            return Err(unread(format!(
                "a file whose name is not text, `{path}`, can't be compared"
            )));
        }
        let untracked = entry.starts_with("?? ") || entry.starts_with("!! ");
        if untracked && repo_kind(&dir.join(path)) != Kind::None {
            continue;
        }
        let top = path.split('/').next().unwrap_or(path);
        if left_out.iter().any(|out| out == top) {
            continue;
        }
        paths.push(path.to_owned());
    }
    // **What git is told not to look at** (#4781): `status` shows no change to a file marked
    // unchanged (`assume-unchanged`, a lower-case tag) or to skip the work tree (`skip-worktree`,
    // `S`, or `s` with both), so each one is listed as it is, and is no loss only where it is the
    // same here. One marked unchanged counts even where it is gone, since `status` does not see
    // that either. One marked to skip the work tree that is not there holds nothing: a sparse
    // checkout leaves its files out so.
    let flagged = run(dir, &["ls-files", "-z", "-v"]).map_err(|why| unread(why.to_string()))?;
    if !flagged.ok() {
        return Err(unread(flagged.err.trim().to_owned()));
    }
    for entry in flagged.out.split('\0').filter(|entry| entry.len() > 2) {
        let (tag, path) = entry.split_at(2);
        let tag = tag.chars().next().unwrap_or(' ');
        let skips_the_work_tree = tag == 'S' || tag == 's';
        if !(tag.is_ascii_lowercase() || skips_the_work_tree) {
            continue;
        }
        let top = path.split('/').next().unwrap_or(path);
        if left_out.iter().any(|out| out == top) {
            continue;
        }
        if skips_the_work_tree && std::fs::symlink_metadata(dir.join(path)).is_err() {
            continue;
        }
        if path.contains(char::REPLACEMENT_CHARACTER) {
            return Err(unread(format!(
                "a file whose name is not text, `{path}`, can't be compared"
            )));
        }
        paths.push(path.to_owned());
    }
    paths.sort();
    paths.dedup();
    if paths.len() > MOST_CHANGED {
        return Err(unread(format!(
            "it has more than {MOST_CHANGED} changed files"
        )));
    }
    paths
        .into_iter()
        .map(|path| {
            file_now(dir, &path)
                .map(|now| Changed {
                    path: path.clone(),
                    now,
                })
                .map_err(unread)
        })
        .collect()
}

/// One file of a work tree that differs from what it has checked out, and what it holds now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Changed {
    /// `/`-separated, in its repository.
    pub path: String,
    /// What [`file_now`] says of it.
    pub now: Option<String>,
}

/// **What the file at `path` in the work tree at `repo` holds**, so two ends can tell they hold
/// the same: the blob a file's bytes are, as git names it with no filter run (`blob:<oid>`), where
/// a link points (`link:<target>`), or `None` where there is no file there.
fn file_now(repo: &Path, path: &str) -> Result<Option<String>, String> {
    let at = repo.join(path);
    let meta = match std::fs::symlink_metadata(&at) {
        Ok(meta) => meta,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("{path} could not be read: {e}")),
    };
    if meta.file_type().is_symlink() {
        let target =
            std::fs::read_link(&at).map_err(|e| format!("{path} could not be read: {e}"))?;
        let Some(target) = target.to_str() else {
            return Err(format!("{path} is a link whose target is not text"));
        };
        return Ok(Some(format!("link:{target}")));
    }
    if !meta.is_file() {
        return Ok(Some("folder".to_owned()));
    }
    let hashed = run(repo, &["hash-object", "--no-filters", "--", path])
        .map_err(|why| format!("{path} could not be read: {why}"))?;
    if !hashed.ok() {
        return Err(format!("{path} could not be read: {}", hashed.err.trim()));
    }
    Ok(Some(format!("blob:{}", hashed.line())))
}

/// **What the desktop's project at `root` does not have of what a runner holds**, in sentences
/// (#4569): none only where `unread` is empty and every repository in `held` is of `project`, has
/// a repository at the same path here that holds each file the runner's holds uncommitted as it
/// holds it, and has each of its commits reached from a ref here. Only then may the runner go.
pub fn not_home(root: &Path, project: &str, held: &[Held], unread: &[String]) -> Vec<String> {
    let mut missing: Vec<String> = unread.to_vec();
    // A runner is sent the project's own repository as a task starts there (D-4569-21): one that
    // no longer lists it may hold, outside every clone, what no list shows.
    if !held
        .iter()
        .any(|one| one.project == project && one.path.is_empty())
    {
        missing.push(
            "the runner no longer holds the project's own repository, so what it holds outside \
             the clones can't be listed"
                .to_owned(),
        );
    }
    for one in held {
        let at = if one.path.is_empty() {
            "the project's own repository".to_owned()
        } else {
            format!("the repository {}", one.path)
        };
        if one.project != project {
            if !one.refs.is_empty() || !one.changed.is_empty() {
                missing.push(format!(
                    "the runner holds {at} of a project kept under another id ({}), which can't \
                     come here",
                    one.project
                ));
            }
            continue;
        }
        if one.refs.is_empty() && one.changed.is_empty() {
            continue;
        }
        let Some(here) = path_here(root, &one.path) else {
            missing.push(format!(
                "the runner holds {at}, at a path that can't be one here"
            ));
            continue;
        };
        if !super::a_repository(&here) {
            missing.push(format!("{at} on the runner has no repository here"));
            continue;
        }
        let differ: Vec<&str> = one
            .changed
            .iter()
            .filter(|change| {
                path_here(&here, &change.path).is_none_or(|_| {
                    file_now(&here, &change.path).ok().as_ref() != Some(&change.now)
                })
            })
            .map(|change| change.path.as_str())
            .collect();
        if !differ.is_empty() {
            let shown: Vec<&str> = differ.iter().take(5).copied().collect();
            let more = differ.len() - shown.len();
            missing.push(format!(
                "{at} on the runner holds changes no commit has, which differ here: {}{}",
                shown.join(", "),
                if more > 0 {
                    format!(" and {more} more")
                } else {
                    String::new()
                }
            ));
        }
        // Stashes are said as one, in words: no fetch brings one back (#4781).
        let mut stashes = Vec::new();
        for kept in &one.refs {
            if reached_here(&here, &kept.oid) {
                continue;
            }
            if kept.name == STASH || kept.name.starts_with(&format!("{STASH}@{{")) {
                stashes.push(kept.name.as_str());
                continue;
            }
            missing.push(format!(
                "{} of {at} on the runner is at a commit no ref here reaches",
                kept.name
            ));
        }
        if !stashes.is_empty() {
            missing.push(format!(
                "{at} on the runner holds {} ({}), which Fetch again does not bring back",
                crate::counts::counted(stashes.len(), "stashed change", "stashed changes"),
                stashes
                    .iter()
                    .take(3)
                    .copied()
                    .chain((stashes.len() > 3).then_some("…"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }
    missing
}

/// `path`, `/`-separated in a project, as a folder in the project at `root`: only plain names,
/// no `.` or `..`, none empty.
fn path_here(root: &Path, path: &str) -> Option<std::path::PathBuf> {
    if path.is_empty() {
        return Some(root.to_path_buf());
    }
    let mut at = root.to_path_buf();
    for part in path.split('/') {
        if part.is_empty()
            || part == "."
            || part == ".."
            || part.contains('\\')
            || part.contains('\0')
        {
            return None;
        }
        at.push(part);
    }
    Some(at)
}

/// Whether `oid`, a full object name, is a commit `repo` reaches from one of its refs or `HEAD`.
fn reached_here(repo: &Path, oid: &str) -> bool {
    if !super::an_oid(oid) {
        return false;
    }
    let is_commit =
        run(repo, &["cat-file", "-e", &format!("{oid}^{{commit}}")]).is_ok_and(|ran| ran.ok());
    if !is_commit {
        return false;
    }
    run(
        repo,
        &["rev-list", "--max-count=1", oid, "--not", "--all", "HEAD"],
    )
    .is_ok_and(|ran| ran.ok() && ran.out.trim().is_empty())
}

#[cfg(test)]
mod tests;
