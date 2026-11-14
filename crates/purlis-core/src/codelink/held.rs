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
    repos_held_until(projects, std::time::Instant::now() + LOOK_WITHIN)
}

/// [`repos_held`], stopping at `until` (#5312): between folders, so the listing of a repository
/// under way finishes first. What ran out is said, as it keeps the runner.
pub fn repos_held_until(projects: &Path, until: std::time::Instant) -> (Vec<Held>, Vec<String>) {
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
            if std::time::Instant::now() >= until {
                unread.push(ran_out_of_time());
                return (held, unread);
            }
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

/// The deepest a folder outside the projects is looked into, in folders from where its walk
/// starts: deeper ones are said, so the folders a walk holds open stay far under a process's
/// limit.
const MOST_DEPTH: usize = 128;

/// The most sentences of what could not be looked at outside the projects a runner gives: the
/// rest are counted.
const MOST_SAID_OUTSIDE: usize = 20;

/// How long before a host first starts a program a file still counts as changed since: a file
/// system's clock stamps a change coarsely, and nothing of a task runs before its program is
/// asked for.
pub const CHANGED_WITHIN: std::time::Duration = std::time::Duration::from_secs(2);

/// Folders of a runner's home a tool keeps by its own default, by their path in it, which hold
/// nothing a task made: Go's module and build cache under its default `GOPATH`.
const TOOLS_KEEP: &[&str] = &["go/pkg"];

/// **The file whose making marks a host's first program** (#4933), beside the projects it keeps:
/// hidden, so no project is ever named so. Its status change time is when the first program was
/// asked for, kept on the same disk as the home, so a host started again in the same pod still
/// knows it.
pub const FIRST_PROGRAM: &str = ".first-program";

/// How the folder a transfer's bundle is kept in beside the projects is named, as
/// `.code-<random>` (`purlis_host`'s code transfer): removed as the transfer ends.
pub const TRANSFER_SCRATCH: &str = ".code-";

/// Folders of the runner's file system, by their path from its root, that hold nothing a task
/// could keep (#4933): the kernel's views of itself (`proc`, `sys`, `dev`, with `/dev/shm`), and
/// what is temporary by the file system's convention (`run`, `tmp`, `var/tmp`), as a temporary
/// folder in the home is.
const NO_TASKS_WORK: &[&str] = &["proc", "sys", "dev", "run", "tmp", "var/tmp"];

/// **What purlis keeps in its data home itself** (#5029), by name at its top: every store
/// `docs/plane-format.md` names there but the projects, and no other (#5052). None holds a
/// task's work, and the walk of the data home leaves each out. A store added there and not
/// here would keep every fresh runner, so a test holds this to the plane format.
pub const PURLIS_KEEPS: &[&str] = &[
    "app-state",
    "audit",
    "browser-logins",
    crate::datahome::BROWSERS,
    "cache-homes",
    "codex-homes",
    "docker-settings",
    "engine-links",
    "engines",
    "events",
    "harnesses",
    "inbox",
    "index",
    "local-project",
    "network",
    "profiles",
    "protection",
    "reviews",
    "runners",
    "sign-ins-waiting",
    "telemetry",
];

/// **Every folder purlis keeps for itself on a runner whose home is `home`** (#5028), wherever
/// the environment `var` puts it, under each name it has had: its config home's folder (as
/// `$PURLIS_CONFIG_HOME` or `$XDG_CONFIG_HOME` places it, and as the session host finds its
/// default), its data home (as `$XDG_DATA_HOME` places it, or under the home; one
/// `$PURLIS_DATA_HOME` names is the data home in use, looked through by [`outside_with`]), the app's own folder under the data folder (its logs among them), the lock it holds
/// beside the config home's folder, and the folder of its session host's socket in
/// `$XDG_RUNTIME_DIR`. purlis keeps nothing of its own in a state or cache home. Each is left
/// out of the walks outside the projects; the data home in use is looked through by its own
/// rules ([`outside_with`]).
///
/// Only a folder purlis names is left out, never one a variable names as it is: a variable
/// naming a folder others share (`$PURLIS_LOG_DIR=/srv`) would hide all of it. So a log folder
/// moved by `$PURLIS_LOG_DIR` holds files that count, and keeps a fresh runner (`purlis serve`
/// logs to its standard error). Nor is the home, or a folder holding it, ever one.
pub fn purlis_keeps_for_itself(
    var: &dyn Fn(&str) -> Option<String>,
    home: &Path,
) -> Vec<std::path::PathBuf> {
    use std::path::PathBuf;
    let named = |name: &str| {
        var(name)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
    };
    let mut kept: Vec<PathBuf> = Vec::new();
    // The config home's folder, and the one the session host takes for its default.
    for root in [
        crate::machine::rooted(
            var(crate::machine::HOME_VAR).map(Into::into),
            var("XDG_CONFIG_HOME").map(Into::into),
            Some(home.to_path_buf()),
        ),
        crate::machine::rooted(
            None,
            var("XDG_CONFIG_HOME").map(Into::into),
            Some(home.to_path_buf()),
        ),
    ]
    .into_iter()
    .flatten()
    {
        kept.extend(crate::machine::dirs_spelled(&root));
        kept.push(root.join(crate::renamelocal::busy::LOCK));
    }
    // The data home and its log folder: each data folder the ladder can name, under each name.
    let system = [
        home.join(".local/share"),
        home.join("Library/Application Support"),
    ];
    for base in named("XDG_DATA_HOME").into_iter().chain(system) {
        for spelling in crate::names::DATA_HOME.spellings() {
            kept.push(base.join(spelling));
        }
        for app in crate::names::BUNDLE_ID.spellings() {
            kept.push(base.join(app));
        }
    }
    for app in crate::names::BUNDLE_ID.spellings() {
        kept.push(home.join("Library/Logs").join(app));
    }
    if let Some(runtime) = named("XDG_RUNTIME_DIR") {
        kept.push(runtime.join("purlis"));
    }
    kept.retain(|folder| !home.starts_with(folder));
    kept.sort();
    kept.dedup();
    kept
}

/// **A program is about to start on a host keeping its projects in `projects`** (#4933): the
/// first one's start is marked by making [`FIRST_PROGRAM`] there, which is never made again
/// while it is there. Never through a link. Where no mark could be made and nothing stands in
/// its place, why is said: the program does not start then, since a host started again would
/// know of no program and look for nothing. Anything else standing there (a folder, a link, a
/// pipe) lets it start (#5103): [`first_program_at`] says that, which keeps a fresh runner.
#[cfg(unix)]
pub fn first_program_starting(projects: &Path) -> Result<(), String> {
    match mark_first_program(projects) {
        Ok(()) => Ok(()),
        Err(why) => match rustix::fs::statat(
            rustix::fs::CWD,
            projects.join(FIRST_PROGRAM),
            rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
        ) {
            Ok(_) => Ok(()),
            Err(_) => Err(why),
        },
    }
}

/// [`first_program_starting`]'s making of the mark: why it is not the file purlis made, if not.
#[cfg(unix)]
fn mark_first_program(projects: &Path) -> Result<(), String> {
    use rustix::fs::{CWD, Mode, OFlags};
    let not_noted = |e: &dyn std::fmt::Display| {
        format!(
            "the runner could not note when its first program started, which purlis needs to \
             find what a task keeps outside its project: {e}"
        )
    };
    std::fs::create_dir_all(projects).map_err(|e| not_noted(&e))?;
    // Never waiting on a pipe left in its place, and only ever the file purlis made.
    let mark = rustix::fs::openat(
        CWD,
        projects.join(FIRST_PROGRAM),
        OFlags::WRONLY | OFlags::CREATE | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::RUSR | Mode::WUSR,
    )
    .map_err(|e| not_noted(&e))?;
    let stat = rustix::fs::fstat(&mark).map_err(|e| not_noted(&e))?;
    if rustix::fs::FileType::from_raw_mode(stat.st_mode) != rustix::fs::FileType::RegularFile {
        return Err(not_noted(&format!(
            "{FIRST_PROGRAM} beside its projects is not a file purlis made"
        )));
    }
    Ok(())
}

/// **When the first program on a host keeping its projects in `projects` started** (#4933), as
/// [`first_program_starting`] marked it: `None` where none has. Why it can't be known is said.
#[cfg(unix)]
pub fn first_program_at(projects: &Path) -> Result<Option<std::time::SystemTime>, String> {
    use rustix::fs::{AtFlags, CWD, FileType};
    let unknown = |why: String| {
        format!(
            "when the runner's first program started can't be known ({why}), so what a task kept \
             outside its project could not be looked for"
        )
    };
    let stat =
        match rustix::fs::statat(CWD, projects.join(FIRST_PROGRAM), AtFlags::SYMLINK_NOFOLLOW) {
            Ok(stat) => stat,
            Err(rustix::io::Errno::NOENT) => return Ok(None),
            Err(e) => return Err(unknown(e.to_string())),
        };
    if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile {
        return Err(unknown(format!(
            "{FIRST_PROGRAM} beside its projects is not a file purlis made"
        )));
    }
    changed_at(&stat)
        .map(Some)
        .ok_or_else(|| unknown(format!("{FIRST_PROGRAM} has no time purlis can read")))
}

/// Whether the user this process runs as may write `name` in `dir`, by its mode and whatever
/// its file system grants beyond it (an ACL). Asked by the process's real ids, which are the
/// runner's user's as its effective ones are (purlis's host is never set-id), with the call
/// every kernel and sandbox profile knows. Only a refusal by its permissions or a read-only file
/// system says no: a check that can't be made says it may, so the file counts.
#[cfg(unix)]
fn may_write<Fd: rustix::fd::AsFd, P: rustix::path::Arg>(dir: Fd, name: P) -> bool {
    !matches!(
        rustix::fs::accessat(
            dir,
            name,
            rustix::fs::Access::WRITE_OK,
            rustix::fs::AtFlags::empty(),
        ),
        Err(rustix::io::Errno::ACCESS | rustix::io::Errno::ROFS)
    )
}

/// When `stat`'s file last had its status changed, which no program can set back.
#[cfg(unix)]
fn changed_at(stat: &rustix::fs::Stat) -> Option<std::time::SystemTime> {
    u64::try_from(stat.st_ctime)
        .ok()
        .zip(u32::try_from(stat.st_ctime_nsec).ok())
        .and_then(|(secs, nanos)| {
            std::time::UNIX_EPOCH.checked_add(std::time::Duration::new(secs, nanos))
        })
}

/// **The rest of a runner's file system** (#4933), looked through as well as its home: from
/// `root`, for files `owner`, the runner's user, owns or may write (#5103).
#[derive(Debug, Clone, Copy)]
pub struct Beyond<'a> {
    pub root: &'a Path,
    pub owner: u32,
}

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
    outside_the_projects_and_beyond(home, skip, since, None)
}

/// [`outside_the_projects`], and where `beyond` names the rest of the runner's file system, each
/// file there `beyond`'s owner owns or may write changed since too (#4933, #5103): a task at the sandbox's floor may
/// write wherever the runner's user may, such as the volume its home is a folder of, or a folder
/// its image gives that user. Its home and every folder in `skip` are left out of that, as is
/// what holds no task's work ([`NO_TASKS_WORK`]); so is a hidden entry of a person's home there
/// (`/root`, `/home/<name>`), for the home's reason. A folder that user can't read is said,
/// unless it is another's, unchanged since, that the user can neither write in nor pass through:
/// that holds nothing of theirs that was not there before.
///
/// The places that user may write are found here ([`writable`]); a host that checks again keeps
/// them and calls [`outside_with`].
#[cfg(unix)]
pub fn outside_the_projects_and_beyond(
    home: &Path,
    skip: &[std::path::PathBuf],
    since: Option<std::time::SystemTime>,
    beyond: Option<Beyond<'_>>,
) -> Vec<String> {
    let Some(since) = since else {
        return Vec::new();
    };
    let found = beyond.map(|beyond| {
        let mut skip = skip.to_vec();
        skip.push(home.to_path_buf());
        writable(beyond, &skip, since)
    });
    outside_with(home, None, skip, Some(since), beyond.zip(found.as_ref()))
}

/// **purlis's data home on a runner, as the walk of it needs it** (#5029): the folder, the
/// projects folder in it (`<data>/projects`), and each repository [`repos_held`] listed there,
/// left out since it is listed on its own.
#[derive(Debug, Clone, Copy)]
pub struct DataHome<'a> {
    pub data: &'a Path,
    pub projects: &'a Path,
    pub listed: &'a [std::path::PathBuf],
}

/// **What a task keeps on its runner outside every repository** (#4781, #4933, #5029, #5030), in
/// sentences: each file changed since `since` in the runner's `home` ([`outside_the_projects`]);
/// in purlis's data home, but what purlis keeps there itself ([`PURLIS_KEEPS`]), the mark of the
/// first program and a transfer's scratch beside the projects, and each repository listed; and,
/// where `beyond` is given, in each place [`writable`] found its user may write, and each file
/// it owns or may write that [`writable`] found elsewhere, by itself or by its folder. What [`writable`] could not look through is said
/// again. `skip` is left out of every walk but where it holds the data home or is in its
/// projects. A data home that is the home itself is looked through as the home.
#[cfg(unix)]
pub fn outside_with(
    home: &Path,
    data: Option<DataHome<'_>>,
    skip: &[std::path::PathBuf],
    since: Option<std::time::SystemTime>,
    beyond: Option<(Beyond<'_>, &Writable)>,
) -> Vec<String> {
    let until = std::time::Instant::now() + LOOK_WITHIN;
    outside_until(home, data, skip, since, beyond, until)
}

/// [`outside_with`], stopping at `until` (#5312), which is said and keeps the runner.
#[cfg(unix)]
pub fn outside_until(
    home: &Path,
    data: Option<DataHome<'_>>,
    skip: &[std::path::PathBuf],
    since: Option<std::time::SystemTime>,
    beyond: Option<(Beyond<'_>, &Writable)>,
    until: std::time::Instant,
) -> Vec<String> {
    outside_bounded(home, data, skip, since, beyond, MOST_FOLDERS, until)
}

/// [`outside_with`], looking through no more than `most` folders.
#[cfg(all(unix, test))]
fn outside_within(
    home: &Path,
    data: Option<DataHome<'_>>,
    skip: &[std::path::PathBuf],
    since: Option<std::time::SystemTime>,
    beyond: Option<(Beyond<'_>, &Writable)>,
    most: usize,
) -> Vec<String> {
    let until = std::time::Instant::now() + LOOK_WITHIN;
    outside_bounded(home, data, skip, since, beyond, most, until)
}

/// [`outside_within`], stopping at `until`.
#[cfg(unix)]
fn outside_bounded(
    home: &Path,
    data: Option<DataHome<'_>>,
    skip: &[std::path::PathBuf],
    since: Option<std::time::SystemTime>,
    beyond: Option<(Beyond<'_>, &Writable)>,
    most: usize,
    until: std::time::Instant,
) -> Vec<String> {
    let Some(since) = since else {
        return Vec::new();
    };
    let mut outside = Outside::new(most, until);
    // Said once, whichever look ran out.
    outside.ran_out = beyond.is_some_and(|(_, found)| found.ran_out);
    // The data home, wherever it is, is looked through by its own rules, so never by another's.
    let data = data.filter(|data| data.data != home);
    let mut skip = skip.to_vec();
    skip.extend(data.map(|data| data.data.to_path_buf()));
    let mut walked = outside.walk(home, "", Rules::Home, &skip, &[], since);
    if walked
        && let Some(DataHome {
            data,
            projects,
            listed,
        }) = data
        && let Ok(projects_rel) = projects.strip_prefix(data)
        && let Some(projects_rel) = projects_rel.to_str()
    {
        let shown = match data.strip_prefix(home) {
            Ok(rel) => format!("~/{}", rel.display()),
            Err(_) => data.display().to_string(),
        };
        // What the other walks leave out but the data home itself and its projects, and the
        // home where it is in the data home, which its own rules look through.
        let within: Vec<std::path::PathBuf> = skip
            .iter()
            .filter(|kept| !data.starts_with(kept) && !kept.starts_with(projects))
            .cloned()
            .chain(home.starts_with(data).then(|| home.to_path_buf()))
            .collect();
        walked = outside.walk(
            data,
            "",
            Rules::Data(&shown, projects_rel),
            &within,
            listed,
            since,
        );
    }
    if walked && let Some((beyond, found)) = beyond {
        skip.push(home.to_path_buf());
        // Each place is opened from the root by its path: no folder above it is one its user
        // owns or may write in, so none can have been made a link since it was found.
        for place in &found.places {
            if !outside.walk(
                beyond.root,
                place,
                Rules::Beyond(beyond.owner),
                &skip,
                &[],
                since,
            ) {
                break;
            }
        }
        outside.look_at_recorded(beyond, found, &skip, since);
        for said in &found.said {
            outside.say(said.clone());
        }
    }
    outside.sentences()
}

/// **The longest one check of a runner looks through its files** (#5312): since #5112 what a check
/// looks at follows the size of the image, and a sweep asks again every minute, so without it
/// looks of a very large image would pile up on the runner. What ran out is said, and keeps the
/// runner as anything not looked through does.
///
/// Measured on 2026-11-05 on a CI runner running the rest of the tests, a debug build, with its
/// caches dropped: an image of 1,000,000 files in 5,000 folders took 14 to 18 seconds for the first
/// look and 20 to 29 for a check, and with them warm about 5 for either. So this holds about six
/// million such files on that disk, and a million on one six times slower. Stated, not measured:
/// no real cluster's disk was timed.
pub const LOOK_WITHIN: std::time::Duration = std::time::Duration::from_secs(300);

/// What helps where a check ran out of time (#5312): a look warms the runner's file system, and
/// what the first one finds is kept once it is whole, so a second is quicker. An image is set
/// when a runner is made, so a smaller one helps only the next.
pub const WHAT_HELPS_A_CHECK: &str = "Fetch again, as a second look is often quicker. If it keeps \
     running out, the image or what the task saved holds too many files: use a smaller image for \
     this pool's next runners, and discard this one once you no longer need what it holds";

/// What a check that ran out of time says.
fn ran_out_of_time() -> String {
    format!(
        "the runner ran out of time looking through its files, after {} minutes, the limit for \
         one check, so purlis can't tell all its work is here. {WHAT_HELPS_A_CHECK}",
        LOOK_WITHIN.as_secs() / 60
    )
}

/// `said`, a check's sentences, with that it ran out of time first and once (#5312), wherever it
/// ran out: the window names only the first few, and this says the rest may not be all there is.
#[must_use]
pub fn ran_out_first(said: Vec<String>) -> Vec<String> {
    let ran_out = ran_out_of_time();
    let (first, rest): (Vec<String>, Vec<String>) =
        said.into_iter().partition(|one| *one == ran_out);
    first.into_iter().take(1).chain(rest).collect()
}

/// The most folders a runner's file system is looked through, once for each host, to find where
/// its user may write ([`writable`]): an image's own folders count here, and never against what
/// one check looks through.
pub const MOST_IMAGE_FOLDERS: usize = 2_000_000;

/// The most files its user owns or may write outside every place it may write a runner keeps to
/// look at again ([`writable`]), a folder recorded in place of its files counting as one (#5112):
/// past it, what it found is not whole.
pub const MOST_OWN_FILES: usize = 100_000;

/// **Where on a runner, outside its home, its user may have saved anything** (#5030), as
/// [`writable`] found it: each place it may write, by its path from the root, each file it owns
/// or may write outside them, each folder recorded in place of its files, and what could not be
/// looked through, in sentences.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Writable {
    /// The cutoff it was found with: an unreadable folder unchanged since is passed over.
    pub since: Option<std::time::SystemTime>,
    /// What it was told to leave out.
    pub skip: Vec<std::path::PathBuf>,
    places: Vec<String>,
    files: Vec<String>,
    /// Folders it may not write, each holding only files it may change (#5112).
    folders: Vec<String>,
    said: Vec<String>,
    /// Whether it stopped at the check's limit of time (#5312), which a check says once.
    ran_out: bool,
}

impl Writable {
    /// Each place the user may write, by its path from the root, empty for the root itself.
    pub fn places(&self) -> &[String] {
        &self.places
    }

    /// Whether it may be kept for later checks: it looked through everything, so it says
    /// nothing, which an error that passes (`EIO`, a lack of memory) could otherwise make last
    /// as long as the host.
    pub fn whole(&self) -> bool {
        self.said.is_empty() && !self.ran_out
    }
}

/// **Every place on the runner from `beyond`'s root, but `skip`, its user may write** (#5030): a
/// folder it owns, since it could open that one again at any time, or one it may write in now,
/// and nothing within it is looked at further. Each file it owns or may write outside them is
/// kept too (#5103), since it may change that one wherever it is. Where it may change every file
/// in a folder, that folder is kept in place of them (#5112), so an image of many such files is
/// looked through whole: it may not write there, so what the folder holds stays. What can't be
/// looked through is said, by [`outside_the_projects_and_beyond`]'s rules for it.
///
/// Found once for each host and kept while it is [whole](Writable::whole), since what its user
/// may write can't grow: a runner's user is never root, holds no capability and can gain none
/// (`no_new_privileges`, every capability dropped), so a folder it neither owns nor may write in
/// stays so, and so does every folder above a place it may. So an image's own folders are looked
/// through once, not at each check.
#[cfg(unix)]
pub fn writable(
    beyond: Beyond<'_>,
    skip: &[std::path::PathBuf],
    since: std::time::SystemTime,
) -> Writable {
    writable_until(beyond, skip, since, std::time::Instant::now() + LOOK_WITHIN)
}

/// [`writable`], stopping at `until` (#5312): what it found then is not [whole](Writable::whole),
/// so it is not kept, and the next check looks again.
#[cfg(unix)]
pub fn writable_until(
    beyond: Beyond<'_>,
    skip: &[std::path::PathBuf],
    since: std::time::SystemTime,
    until: std::time::Instant,
) -> Writable {
    finds_writable(
        beyond,
        skip,
        since,
        MOST_IMAGE_FOLDERS,
        MOST_OWN_FILES,
        true,
        until,
    )
}

/// [`writable`], looking through no more than `most` folders.
#[cfg(all(unix, test))]
fn writable_within(
    beyond: Beyond<'_>,
    skip: &[std::path::PathBuf],
    since: std::time::SystemTime,
    most: usize,
) -> Writable {
    writable_bounded(beyond, skip, since, most, MOST_OWN_FILES, true)
}

/// [`writable_within`], keeping no more than `most_files` files and folders of them, with
/// `owned_is_a_place` false only for a test: there a folder the test's own user owns stands for
/// an image's, shut to the runner's user, since only root could make one another user owns.
#[cfg(all(unix, test))]
fn writable_bounded(
    beyond: Beyond<'_>,
    skip: &[std::path::PathBuf],
    since: std::time::SystemTime,
    most: usize,
    most_files: usize,
    owned_is_a_place: bool,
) -> Writable {
    let until = std::time::Instant::now() + LOOK_WITHIN;
    finds_writable(
        beyond,
        skip,
        since,
        most,
        most_files,
        owned_is_a_place,
        until,
    )
}

/// [`writable_bounded`], stopping at `until`.
#[cfg(unix)]
fn finds_writable(
    beyond: Beyond<'_>,
    skip: &[std::path::PathBuf],
    since: std::time::SystemTime,
    most: usize,
    most_files: usize,
    owned_is_a_place: bool,
    until: std::time::Instant,
) -> Writable {
    let mut outside = Outside::new(most, until);
    outside.most_files = most_files;
    outside.owned_is_a_place = owned_is_a_place;
    outside.finding = Some(Writable::default());
    outside.walk(
        beyond.root,
        "",
        Rules::Beyond(beyond.owner),
        skip,
        &[],
        since,
    );
    let mut found = outside.finding.take().unwrap_or_default();
    found.since = Some(since);
    found.skip = skip.to_vec();
    found.ran_out = std::mem::take(&mut outside.ran_out);
    found.said = outside.sentences();
    found
}

/// Which part of the runner a walk looks through, and so what it leaves out and how it names a
/// path.
#[cfg(unix)]
#[derive(Debug, Clone, Copy)]
enum Rules<'a> {
    /// The runner's home: its hidden entries and what tools keep are left out, and a path is
    /// named from `~`.
    Home,
    /// purlis's data home, named as the first, with its projects folder at the second, its path
    /// in it: what purlis keeps there itself and beside the projects is left out (#5029).
    Data(&'a str, &'a str),
    /// The rest of its file system, for files the user it names owns or may write: a path is
    /// named from `/`.
    Beyond(u32),
}

#[cfg(unix)]
impl Rules<'_> {
    fn shown(self, rel: &str) -> String {
        match (self, rel.is_empty()) {
            (Rules::Home, true) => "~".to_owned(),
            (Rules::Home, false) => format!("~/{rel}"),
            (Rules::Data(shown, _), true) => shown.to_owned(),
            (Rules::Data(shown, _), false) => format!("{shown}/{rel}"),
            (Rules::Beyond(_), _) => format!("/{rel}"),
        }
    }

    /// Whether the hidden entries of the folder at `rel` are a program's own.
    fn hidden_left_out(self, rel: &str) -> bool {
        match self {
            Rules::Home => rel.is_empty(),
            Rules::Data(..) => false,
            Rules::Beyond(_) => {
                rel == "root"
                    || rel
                        .strip_prefix("home/")
                        .is_some_and(|name| !name.is_empty() && !name.contains('/'))
            }
        }
    }

    fn left_out(self, shown: &str) -> bool {
        match self {
            Rules::Home => TOOLS_KEEP.contains(&shown),
            // What purlis keeps at the data home's top, and beside the projects the mark of the
            // first program and a transfer's scratch (`.code-<random>`), no more of what is hidden
            // there.
            Rules::Data(_, projects) => {
                PURLIS_KEEPS.contains(&shown)
                    || shown
                        .strip_prefix(projects)
                        .and_then(|rest| rest.strip_prefix('/'))
                        .is_some_and(|name| {
                            name == FIRST_PROGRAM || name.starts_with(TRANSFER_SCRATCH)
                        })
            }
            Rules::Beyond(_) => NO_TASKS_WORK.contains(&shown),
        }
    }

    /// Whether a file, folder or link found is the user's: in the home and the data home every
    /// one is; beyond them, one it owns.
    fn owns(self, stat: &rustix::fs::Stat) -> bool {
        match self {
            Rules::Home | Rules::Data(..) => true,
            Rules::Beyond(owner) => stat.st_uid == owner,
        }
    }

    /// Whether a file found, `name` in the folder `fd`, is one a task could have changed: one
    /// [the user owns](Rules::owns), or beyond the home a regular file it may write, whoever
    /// owns it (#5103).
    fn counts(
        self,
        stat: &rustix::fs::Stat,
        fd: &rustix::fd::OwnedFd,
        name: &std::ffi::CStr,
    ) -> bool {
        self.owns(stat)
            || (rustix::fs::FileType::from_raw_mode(stat.st_mode)
                == rustix::fs::FileType::RegularFile
                && may_write(fd, name))
    }
}

/// A folder a walk outside the projects has found and not yet looked through.
#[cfg(unix)]
struct Folder {
    /// Its path from where the walk's paths are named.
    rel: String,
    /// The folder it is in, held open.
    parent: std::rc::Rc<rustix::fd::OwnedFd>,
    /// Its name there; none for where the walk starts.
    name: Option<std::ffi::CString>,
    /// Whether it is said where it can't be read, whoever may write in it: always in the home and
    /// the data home, and beyond them where the walk's user owns it or it changed since.
    said_unread: bool,
}

/// What the walks outside the projects found, and what they could not look at.
#[cfg(unix)]
struct Outside {
    named: std::collections::BTreeSet<String>,
    found: usize,
    said: Vec<String>,
    unsaid: usize,
    folders: usize,
    /// The most folders it looks through.
    most: usize,
    /// The most files the user owns or may write it records outside the places ([`writable`]).
    most_files: usize,
    /// Whether a folder the user owns is a place ([`writable_bounded`]).
    owned_is_a_place: bool,
    /// Where the walk only finds the places its user may write ([`writable`]), what it found.
    finding: Option<Writable>,
    /// When the check's time is up (#5312).
    until: std::time::Instant,
    /// Whether it ran out, so nothing more is looked at.
    ran_out: bool,
}

#[cfg(unix)]
impl Outside {
    fn new(most: usize, until: std::time::Instant) -> Outside {
        Outside {
            named: std::collections::BTreeSet::new(),
            found: 0,
            said: Vec::new(),
            unsaid: 0,
            folders: 0,
            most,
            most_files: MOST_OWN_FILES,
            owned_is_a_place: true,
            finding: None,
            until,
            ran_out: false,
        }
    }

    /// Whether the check's time is up. Asked of each thing looked at: reading the clock costs far
    /// less than the file system call it comes before.
    fn out_of_time(&mut self) -> bool {
        self.ran_out = self.ran_out || std::time::Instant::now() >= self.until;
        self.ran_out
    }

    fn say(&mut self, sentence: String) {
        if self.said.len() < MOST_SAID_OUTSIDE {
            self.said.push(sentence);
        } else {
            self.unsaid += 1;
        }
    }

    fn found(&mut self, shown: String) {
        self.found += 1;
        self.named.insert(shown);
        if self.named.len() > MOST_NAMED_OUTSIDE {
            self.named.pop_last();
        }
    }

    /// Looks through the folder at `start` in `base` (`base` itself where it is empty) by
    /// `rules`, naming each path from `base`: false where it stopped at its most folders or at
    /// the check's limit of time, so nothing more is looked through.
    fn walk(
        &mut self,
        base: &Path,
        start: &str,
        rules: Rules<'_>,
        skip: &[std::path::PathBuf],
        listed: &[std::path::PathBuf],
        since: std::time::SystemTime,
    ) -> bool {
        use rustix::fs::{Access, AtFlags, CWD, Mode, OFlags};
        let folder = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC;
        let top = if start.is_empty() {
            base.to_path_buf()
        } else {
            base.join(start)
        };
        let top_fd = match rustix::fs::openat(CWD, &top, folder, Mode::empty()) {
            Ok(fd) => fd,
            Err(rustix::io::Errno::NOENT) => return true,
            Err(e) => {
                self.say(match rules {
                    Rules::Home => format!(
                        "the runner's home could not be read, so what a task kept outside its \
                         project could not be looked for: {e}"
                    ),
                    Rules::Data(..) => format!(
                        "purlis's data home on the runner could not be read, so what a task kept \
                         there outside its project could not be looked for: {e}"
                    ),
                    Rules::Beyond(_) => format!(
                        "{} on the runner could not be read, so what a task kept there could not \
                         be looked for: {e}",
                        rules.shown(start)
                    ),
                });
                return true;
            }
        };
        // Each folder is opened only as it is looked through, from the one it is in, which is held
        // open meanwhile: so the open folders are only those of the path being walked, and no
        // more than [`MOST_DEPTH`]. Each is counted as it is found, so the folders waiting are
        // bounded too.
        let mut stack: Vec<Folder> = vec![Folder {
            rel: start.to_owned(),
            parent: std::rc::Rc::new(top_fd),
            name: None,
            said_unread: true,
        }];
        self.folders += 1;
        while let Some(Folder {
            rel,
            parent,
            name,
            said_unread,
        }) = stack.pop()
        {
            if self.out_of_time() {
                return false;
            }
            let shown_dir = rules.shown(&rel);
            // A folder its user can't read is said unless it is another's, unchanged since, that
            // the user can neither write in nor pass through: that holds nothing of a task's not
            // there before. A check that can't be made is said.
            let unread = |e: rustix::io::Errno, parent: &rustix::fd::OwnedFd| {
                e != rustix::io::Errno::ACCESS
                    || said_unread
                    || name.as_deref().is_none_or(|name| {
                        // Shut to the user both ways: it can't write there, nor pass through to a
                        // folder within that it could.
                        let check =
                            |access| rustix::fs::accessat(parent, name, access, AtFlags::EACCESS);
                        !(matches!(
                            check(Access::WRITE_OK),
                            Err(rustix::io::Errno::ACCESS | rustix::io::Errno::ROFS)
                        ) && matches!(check(Access::EXEC_OK), Err(rustix::io::Errno::ACCESS)))
                    })
            };
            let fd = match &name {
                None => std::rc::Rc::clone(&parent),
                Some(name) => match rustix::fs::openat(
                    &*parent,
                    name.as_c_str(),
                    folder | OFlags::NOFOLLOW,
                    Mode::empty(),
                ) {
                    Ok(fd) => std::rc::Rc::new(fd),
                    Err(rustix::io::Errno::NOENT) => continue,
                    Err(e) => {
                        if unread(e, &parent) {
                            self.say(format!("{shown_dir} on the runner could not be read: {e}"));
                        }
                        continue;
                    }
                },
            };
            // Finding where the user may write (#5030): a folder it owns or may write in is a
            // place, looked through at each check, and nothing in it is looked at here. A check
            // that can't be made makes it a place.
            if let (Some(finding), Rules::Beyond(owner)) = (&mut self.finding, rules) {
                let owned = self.owned_is_a_place
                    && !rustix::fs::fstat(&*fd).is_ok_and(|stat| stat.st_uid != owner);
                let may_write = !matches!(
                    rustix::fs::accessat(&*fd, ".", Access::WRITE_OK, AtFlags::EACCESS),
                    Err(rustix::io::Errno::ACCESS | rustix::io::Errno::ROFS)
                );
                if owned || may_write {
                    finding.places.push(rel);
                    continue;
                }
            }
            let mut dir = match rustix::fs::Dir::read_from(&*fd) {
                Ok(dir) => dir,
                Err(e) => {
                    if unread(e, &parent) {
                        self.say(format!("{shown_dir} on the runner could not be read: {e}"));
                    }
                    continue;
                }
            };
            let hidden_left_out = rules.hidden_left_out(&rel);
            // Finding (#5112): the files of this folder its user may change, kept only while they
            // could still fit, and whether it holds a file it may not.
            let mut its_files: Vec<String> = Vec::new();
            let mut more_than_fit = false;
            let mut one_it_may_not_change = false;
            while let Some(entry) = dir.read() {
                if self.out_of_time() {
                    return false;
                }
                let entry = match entry {
                    Ok(entry) => entry,
                    Err(e) => {
                        self.say(format!(
                            "{shown_dir} on the runner could not all be read: {e}"
                        ));
                        break;
                    }
                };
                let raw = entry.file_name();
                let bytes = raw.to_bytes();
                if bytes == b"." || bytes == b".." || (hidden_left_out && bytes.starts_with(b".")) {
                    continue;
                }
                let Ok(name) = std::str::from_utf8(bytes) else {
                    self.say(format!(
                        "a file in {shown_dir} on the runner has a name that is not text, so it \
                         was not looked at"
                    ));
                    continue;
                };
                let rel_now = if rel.is_empty() {
                    name.to_owned()
                } else {
                    format!("{rel}/{name}")
                };
                if rules.left_out(&rel_now)
                    || skip.contains(&base.join(&rel_now))
                    || listed.contains(&base.join(&rel_now))
                {
                    continue;
                }
                // Never through a link: one made since counts as a file.
                let stat = match rustix::fs::statat(&*fd, raw, AtFlags::SYMLINK_NOFOLLOW) {
                    Ok(stat) => stat,
                    Err(rustix::io::Errno::NOENT) => continue,
                    Err(e) => {
                        self.say(format!(
                            "{} on the runner could not be read: {e}",
                            rules.shown(&rel_now)
                        ));
                        continue;
                    }
                };
                match rustix::fs::FileType::from_raw_mode(stat.st_mode) {
                    rustix::fs::FileType::Directory => {
                        if rel_now.split('/').count() > MOST_DEPTH {
                            self.say(format!(
                                "{} on the runner is more than {MOST_DEPTH} folders deep, so it \
                                 was not looked through",
                                rules.shown(&rel_now)
                            ));
                            continue;
                        }
                        self.folders += 1;
                        if self.folders > self.most {
                            self.say(format!(
                                "the runner holds more than {} folders outside its projects, so \
                                 not all were looked through",
                                self.most
                            ));
                            return false;
                        }
                        stack.push(Folder {
                            said_unread: !matches!(rules, Rules::Beyond(_))
                                || rules.owns(&stat)
                                || changed_at(&stat).is_none_or(|at| at >= since),
                            rel: rel_now,
                            parent: std::rc::Rc::clone(&fd),
                            name: Some(raw.to_owned()),
                        });
                    }
                    rustix::fs::FileType::RegularFile | rustix::fs::FileType::Symlink
                        if rules.counts(&stat, &fd, raw) =>
                    {
                        // Outside every place, a file of its own, or one it may write, is kept
                        // to be looked at again at each check, whatever its time now: by its
                        // folder, or by itself, as the rest of the folder decides.
                        if self.finding.is_some() {
                            if its_files.len() < self.most_files {
                                its_files.push(rel_now);
                            } else {
                                more_than_fit = true;
                            }
                            continue;
                        }
                        if changed_at(&stat).is_none_or(|at| at >= since) {
                            self.found(rules.shown(&rel_now));
                        }
                    }
                    rustix::fs::FileType::RegularFile => one_it_may_not_change = true,
                    _ => {}
                }
            }
            if let Some(finding) = &mut self.finding {
                let kept = finding.files.len() + finding.folders.len();
                let adding = match (its_files.is_empty(), one_it_may_not_change) {
                    (true, _) => 0,
                    (false, false) => 1,
                    (false, true) if more_than_fit => usize::MAX,
                    (false, true) => its_files.len(),
                };
                if kept.saturating_add(adding) > self.most_files {
                    self.say(format!(
                        "the runner's user may change more than {} files outside the places it \
                         may write, so not all were looked at",
                        self.most_files
                    ));
                    return false;
                }
                if one_it_may_not_change {
                    finding.files.append(&mut its_files);
                } else if !its_files.is_empty() {
                    finding.folders.push(rel);
                }
            }
        }
        true
    }

    /// Looks at each file and each folder of files [`writable`] recorded, until the check's time
    /// is up.
    fn look_at_recorded(
        &mut self,
        beyond: Beyond<'_>,
        found: &Writable,
        skip: &[std::path::PathBuf],
        since: std::time::SystemTime,
    ) {
        for file in &found.files {
            if self.out_of_time() {
                return;
            }
            let at = beyond.root.join(file);
            match rustix::fs::statat(rustix::fs::CWD, &at, rustix::fs::AtFlags::SYMLINK_NOFOLLOW) {
                Ok(stat) => {
                    let the_users = stat.st_uid == beyond.owner
                        || (rustix::fs::FileType::from_raw_mode(stat.st_mode)
                            == rustix::fs::FileType::RegularFile
                            && may_write(rustix::fs::CWD, &at));
                    if the_users && changed_at(&stat).is_none_or(|at| at >= since) {
                        self.found(format!("/{file}"));
                    }
                }
                Err(rustix::io::Errno::NOENT) => {}
                Err(e) => self.say(format!("/{file} on the runner could not be read: {e}")),
            }
        }
        for folder in &found.folders {
            if self.out_of_time() {
                return;
            }
            self.look_in(
                beyond.root,
                folder,
                Rules::Beyond(beyond.owner),
                skip,
                since,
            );
        }
    }

    /// Looks at the files directly in the folder at `rel` in `base`, recorded by [`writable`] in
    /// place of them (#5112), by the rules the walk that found it went by. A folder within is
    /// [`writable`]'s to have recorded on its own. It is opened from the root by its path: no
    /// folder above it, nor it, is one its user owns or may write in, so none can have been made
    /// a link since it was found.
    fn look_in(
        &mut self,
        base: &Path,
        rel: &str,
        rules: Rules<'_>,
        skip: &[std::path::PathBuf],
        since: std::time::SystemTime,
    ) {
        use rustix::fs::{AtFlags, CWD, Mode, OFlags};
        let shown_dir = rules.shown(rel);
        let at = if rel.is_empty() {
            base.to_path_buf()
        } else {
            base.join(rel)
        };
        let opened = rustix::fs::openat(
            CWD,
            &at,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        );
        let fd = match opened {
            Ok(fd) => fd,
            Err(rustix::io::Errno::NOENT) => return,
            Err(e) => {
                self.say(format!("{shown_dir} on the runner could not be read: {e}"));
                return;
            }
        };
        let mut dir = match rustix::fs::Dir::read_from(&fd) {
            Ok(dir) => dir,
            Err(e) => {
                self.say(format!("{shown_dir} on the runner could not be read: {e}"));
                return;
            }
        };
        let hidden_left_out = rules.hidden_left_out(rel);
        while let Some(entry) = dir.read() {
            if self.out_of_time() {
                return;
            }
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    self.say(format!(
                        "{shown_dir} on the runner could not all be read: {e}"
                    ));
                    return;
                }
            };
            let raw = entry.file_name();
            let bytes = raw.to_bytes();
            if bytes == b"." || bytes == b".." || (hidden_left_out && bytes.starts_with(b".")) {
                continue;
            }
            let Ok(name) = std::str::from_utf8(bytes) else {
                self.say(format!(
                    "a file in {shown_dir} on the runner has a name that is not text, so it \
                     was not looked at"
                ));
                continue;
            };
            let rel_now = if rel.is_empty() {
                name.to_owned()
            } else {
                format!("{rel}/{name}")
            };
            if rules.left_out(&rel_now) || skip.contains(&base.join(&rel_now)) {
                continue;
            }
            let stat = match rustix::fs::statat(&fd, raw, AtFlags::SYMLINK_NOFOLLOW) {
                Ok(stat) => stat,
                Err(rustix::io::Errno::NOENT) => continue,
                Err(e) => {
                    self.say(format!(
                        "{} on the runner could not be read: {e}",
                        rules.shown(&rel_now)
                    ));
                    continue;
                }
            };
            let a_file = matches!(
                rustix::fs::FileType::from_raw_mode(stat.st_mode),
                rustix::fs::FileType::RegularFile | rustix::fs::FileType::Symlink
            );
            if a_file
                && rules.counts(&stat, &fd, raw)
                && changed_at(&stat).is_none_or(|at| at >= since)
            {
                self.found(rules.shown(&rel_now));
            }
        }
    }

    fn sentences(mut self) -> Vec<String> {
        if self.unsaid > 0 {
            self.said.push(format!(
                "and {} more parts of the runner could not be looked at",
                self.unsaid
            ));
        }
        if self.found > 0 {
            let more = self.found - self.named.len();
            let named: Vec<String> = self.named.into_iter().collect();
            self.said.insert(
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
        // First, since the window names only the first few, and it says why all the rest may not
        // be all there is.
        if self.ran_out {
            self.said.insert(0, ran_out_of_time());
        }
        marked(self.said)
    }
}

/// `sentences` with each character a reader would not see in them written as its mark,
/// `⟦U+XXXX⟧` (#5103), by purlis's one rule: a name a runner reports is shown as the
/// characters it holds.
pub fn marked(sentences: Vec<String>) -> Vec<String> {
    sentences
        .into_iter()
        .map(|sentence| purlis_unicode::marked(&sentence, purlis_unicode::Breaks::Marked))
        .collect()
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
    // **Only a repository of its own** (#5029): a `.git` git can't take as one makes git look
    // for the next repository up, which would list that one's refs here and leave this folder's
    // files to no list. So git must name this folder as the repository it reads.
    let own = match kind {
        Kind::Bare => "--absolute-git-dir",
        _ => "--show-toplevel",
    };
    let named = run(dir, &["rev-parse", own]).map_err(|why| failed("read", &why.to_string()))?;
    let real = |path: &Path| std::fs::canonicalize(path).ok();
    if !named.ok()
        || real(Path::new(named.line())).is_none()
        || real(Path::new(named.line())) != real(dir)
    {
        return Err(failed(
            "read",
            format!(
                "git does not take its folder for a repository of its own {}",
                named.err.trim()
            )
            .trim_end(),
        ));
    }
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
        let most = format!("--max-count={}", MOST_STASHES + 1);
        let logged = run(
            dir,
            &["log", "--walk-reflogs", &most, "--format=%H", STASH, "--"],
        )
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
    marked(missing)
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
