// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `charter workspace remove` — delete a workspace and its clones.
//!
//! A port of `commands_workspace.cmd_workspace_remove`.
//!
//! **This command is a recursive delete, and the guard in front of it is the difference
//! between a command and a disaster.** [`crate::wscmd::work_at_risk`] is that guard, and what
//! it is empty of is what this hands to `remove_dir_all`. Three properties of it are
//! load-bearing and each was a bug first:
//!
//! - **A clone charter could not read is at risk** (charter#917). A `git status` that FAILED
//!   writes nothing to stdout, and `""` read as "clean" makes a clone charter could not look
//!   at indistinguishable from one it looked at and found empty.
//! - **Worktrees are checked, and on a different rule** (charter#91). The clone walk cannot
//!   see them — a clone's `.git` is a directory, a linked worktree's is a file — and a
//!   worktree is at risk when it holds commits reachable from no other ref, which is exactly
//!   what `charter wt remove` refuses over. The two guards refuse on identical grounds on
//!   purpose: a workspace that removed what `wt remove` would not have is charter#91 again.
//! - **Open todos are reported and never guarded on.** A todo is a note about the future, not
//!   work that ceases to exist, and a workspace whose todos were all abandoned is precisely
//!   the one worth deleting. Making that case demand `--force` teaches the habit of reaching
//!   for `--force`, which is how a guard stops protecting the commits it exists for.
//!
//! # The exclude block a linked worktree reads elsewhere (#998)
//!
//! Python runs `workspace.unwire_guests(name)` before the delete: purlis's generated files go
//! with the directory, but a **linked worktree's** `info/exclude` is the main repository's and
//! lives somewhere else on disk entirely, so the delete alone would leave purlis's managed
//! block behind in a repository that is still there, naming paths that no longer exist
//! (charter#870).
//!
//! So before the delete, [`crate::guest::withdraw_for_removal`] takes out of each such block
//! the lines only this workspace's checkouts needed, content-addressed the way a wire decides
//! a line: a line stays while another checkout purlis wires there still has its file. Only
//! purlis's own block is rewritten, and when any line in it cannot be accounted for the block
//! is left whole and **named** instead, with what clears it. The operator then loses a tidy-up,
//! never a line of their own.
//!
//! # git's record of a linked worktree kept elsewhere (#998)
//!
//! The same checkout leaves a second thing behind in that repository: git's own record of the
//! worktree (`.git/worktrees/<id>`), which lists the deleted folder as prunable until somebody
//! prunes it. So before the delete, each direct child whose `.git` file names a worktree entry
//! of a repository outside the workspace, and whose entry names that file back
//! ([`crate::worktree::link::of`], the runner's own check), is noted with the folder as git
//! recorded it. **After** the delete, `git worktree remove -- <that folder>` is run in that
//! repository once per folder: with the folder gone git only drops its entry, and before the
//! delete it would resolve links and refuse over untracked files. Never a blanket
//! `git worktree prune`, which would clear records of folders this command never held. Where
//! the check fails or git refuses, one `Warn` line names the prune that clears it.

use std::path::{Path, PathBuf};

use crate::counts::{counted, plural};
use crate::repocmd::{Say, Sink};
use crate::wscmd;

/// What [`remove`] did, and — when the guard refused — **the reading it refused on**.
///
/// **The list travels with the refusal because the refusal's words are about it**
/// (charter-app#182). The window asks `work_at_risk` a second time, when the dialog opens, to
/// draw a preview; that read is older than this one by however long somebody spent reading the
/// dialog. If the workspace moved in between — a clone goes dirty, a commit is made, a
/// worktree appears — the surface then names one clone in the sentence it quotes and a
/// different one on the button that discards it, at exactly the moment consent is being given.
/// Nothing was ever deleted wrongly: this read is the one that decides, and it always was.
/// What was wrong was that the window had no way to draw the list this read was made of.
///
/// **Empty on every outcome but the guard's refusal**, a forced delete included: `--force` is
/// the operator choosing to discard what is here, so what it discarded is reported by the
/// `Warn` lines and by the delete itself, and a caller must never read a non-empty list as
/// "something stopped".
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Removal {
    /// The exit code, unchanged: 0 removed, 1 could not, 2 the guard refused.
    pub code: u8,
    /// `work_at_risk`'s answer at the instant of the delete, when that answer is what stopped
    /// it. The sentences in it are the same sentences the refusal above joined together.
    pub refused_over: Vec<wscmd::AtRisk>,
}

impl Removal {
    /// An outcome with nothing refused over — every path but the guard's.
    fn just(code: u8) -> Self {
        Self {
            code,
            refused_over: Vec::new(),
        }
    }
}

/// `charter workspace remove <name> [--force]`, its exit code, and what it refused on.
///
/// Exit 2 for the guard, as Python does: a refusal that protected work is not the same
/// failure as a name that is not a workspace, and a script can tell them apart.
///
/// **Refused while a chat in it has a live run** (V27d, ADR 0076 §10), `--force` or not: a
/// removal must not leave a program running in a deleted directory. The chats are the ones the
/// app has open in it ([`wscmd::rename::open_in_app`]), as a rename is refused over them.
pub fn remove(root: &Path, name: &str, force: bool, say: Sink) -> Removal {
    let running = match wscmd::workspace_dir(root, name) {
        Some(_) => wscmd::rename::open_in_app(root, &[name]),
        None => Vec::new(),
    };
    remove_unless_running(root, name, force, &running, say)
}

/// [`remove`], with the chats running in the workspace already asked: `running`, by the name
/// each is shown under.
pub(crate) fn remove_unless_running(
    root: &Path,
    name: &str,
    force: bool,
    running: &[String],
    say: Sink,
) -> Removal {
    let Some(dir) = wscmd::workspace_dir(root, name) else {
        say(Say::Fail(
            format!(
                "invalid workspace name '{}' (use letters, digits, '.', '_', '-'; must not start \
             with a dot)",
                crate::personas::one_line(name)
            )
            .into(),
        ));
        return Removal::just(1);
    };
    if !wscmd::workspace_dir_exists(root, name) {
        say(Say::Fail(format!("no workspace '{name}'").into()));
        return Removal::just(1);
    }
    // Before the guard and before anything is read below it: a `workspaces/<ws>` that is a
    // link out of the plane is a directory this command would delete somewhere else
    // entirely. Python's `shutil.rmtree` on a symlink raises; this refuses with a sentence.
    if let Err(why) = crate::contain::no_link_on_the_way(root, &dir) {
        say(Say::Fail(
            format!(
                "'{name}' does not resolve to a directory inside this plane, so nothing was \
             removed ({why})."
            )
            .into(),
        ));
        return Removal::just(1);
    }
    if !running.is_empty() {
        say(Say::Fail(still_running(name, running).into()));
        return Removal::just(1);
    }

    let risky = wscmd::work_at_risk(root, name);
    if !risky.is_empty() && !force {
        let joined: Vec<String> = risky.iter().map(ToString::to_string).collect();
        say(Say::Fail(
            format!(
                "Refusing to remove '{name}' — this would discard work: {}. Push/commit first, \
             or pass --force.",
                joined.join("; ")
            )
            .into(),
        ));
        // The sentence and the list are made from one reading, here, and handed back together.
        // A caller that wanted to name what was refused over and read the guard again would be
        // naming a second, later answer — which is charter-app#182 with the clock the other
        // way round.
        return Removal {
            code: 2,
            refused_over: risky,
        };
    }

    // Reported, never guarded on — see this module's header. Said HERE rather than above the
    // guard so the count describes what is actually about to happen, and only when there is
    // something to say: "0 open todos" on every removal is how a line stops being read at
    // all, including on the removal where it mattered.
    if let Ok(ws) = crate::workspaces::Plane::open(root).workspace(name)
        && let Ok(todos) = ws.todos()
        && !todos.is_empty()
    {
        say(Say::Warn(
            format!(
                "Discarding {} with '{name}' — nothing else holds {}.",
                counted(todos.len(), "open todo", "open todos"),
                plural(todos.len(), "it", "them")
            )
            .into(),
        ));
    }

    // charter#870 and #998: purlis's lines in a repository kept elsewhere, before the delete
    // (see this module's header).
    for done in crate::guest::withdraw_for_removal(root, &dir) {
        say(withdrawn_said(name, &done));
    }
    // #998: read before the delete, cleared after it (see this module's header).
    let held = linked_worktrees(&dir);

    if let Err(why) = std::fs::remove_dir_all(&dir) {
        say(Say::Fail(
            format!(
                "could not remove '{name}' ({}) — some of it may still be there.",
                crate::rewrite::os_words(&why)
            )
            .into(),
        ));
        return Removal::just(1);
    }
    for tree in &held {
        if let Some(said) = forget(name, tree) {
            say(said);
        }
    }
    // The dispatch grants limited to this workspace cover nothing from here on, and none of
    // them is for a workspace made under the name later (#1505).
    let left = crate::dispatchwithin::naming(root, name);
    crate::dispatchwithin::noticed(root);
    for said in dispatches_left_behind(root, name) {
        say(said);
    }
    say(Say::Done(
        format!("Removed workspace '{name}' and its clones.").into(),
    ));
    if left > 0 {
        say(Say::Info(
            crate::dispatchwithin::left_behind_said(left, name, None).into(),
        ));
    }
    Removal::just(0)
}

/// A direct child of the workspace that held a `.git` file, read before the delete.
#[derive(Debug)]
enum Held {
    /// A linked worktree of the repository whose top is `main`, outside the workspace, checked
    /// both ways; `recorded` is its folder as git's entry spells it, the name
    /// `git worktree remove` finds it by once the folder is gone.
    Tree { recorded: String, main: PathBuf },
    /// A `.git` file naming something the runner's check refused: nothing is run for it.
    Unchecked { folder: String },
}

/// The linked worktrees of repositories kept outside `going` that its direct children are.
/// A clone inside `going` takes its worktrees' records with it, and is left out.
fn linked_worktrees(going: &Path) -> Vec<Held> {
    use crate::worktree::link::{self, Check, Link};
    let Ok(reader) = std::fs::read_dir(going) else {
        return Vec::new();
    };
    let mut children: Vec<PathBuf> = reader.filter_map(Result::ok).map(|e| e.path()).collect();
    children.sort();
    let inside = std::fs::canonicalize(going).unwrap_or_else(|_| going.to_path_buf());
    let mut out = Vec::new();
    for child in children {
        let is_file = std::fs::symlink_metadata(child.join(".git"))
            .is_ok_and(|meta| meta.file_type().is_file());
        if !is_file {
            continue;
        }
        let folder = || {
            child
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        };
        let (top, git_dir) = match link::of(&child, Check::Whole) {
            Ok(Link::Linked { top, git_dir }) => (top, git_dir),
            // A submodule's link or a clone: no worktree record anywhere to clear.
            Ok(_) => continue,
            Err(_) => {
                out.push(Held::Unchecked { folder: folder() });
                continue;
            }
        };
        let Some(common) = link::worktree_of(&git_dir) else {
            continue;
        };
        if common.starts_with(&inside) {
            continue;
        }
        let Some(main) = common.parent().map(Path::to_path_buf) else {
            continue;
        };
        // `of` checked that the entry's `gitdir` names this folder's `.git`; git finds the
        // entry again by the folder that line spells, so that spelling is what it is handed.
        // A relative line is read as git reads it, from the folder as the kernel names it.
        let recorded = std::fs::read_to_string(git_dir.join("gitdir"))
            .ok()
            .map(|text| text.trim_end_matches(['\n', '\r']).to_owned())
            .and_then(|line| {
                let line = line.strip_suffix("/.git")?.to_owned();
                Some(if Path::new(&line).is_absolute() {
                    line
                } else {
                    top.display().to_string()
                })
            })
            // Only a folder of this workspace, spelled plainly, is handed to git.
            .filter(|line| plainly_inside(Path::new(line), going, &inside));
        match recorded {
            Some(recorded) => out.push(Held::Tree { recorded, main }),
            None => out.push(Held::Unchecked { folder: folder() }),
        }
    }
    out
}

/// Whether `path` names a folder below `going` (as given, or as the kernel names it) with no
/// `..` on the way: git is only ever handed a place this command just deleted.
fn plainly_inside(path: &Path, going: &Path, inside: &Path) -> bool {
    use std::path::Component;
    let below = |top: &Path| path.starts_with(top) && path != top;
    (below(going) || below(inside))
        && path
            .components()
            .all(|part| !matches!(part, Component::ParentDir | Component::CurDir))
}

/// git's record of `held` cleared, once its folder is gone, or the one line that says how.
fn forget(ws: &str, held: &Held) -> Option<Say> {
    let (recorded, main) = match held {
        Held::Tree { recorded, main } => (recorded, main),
        Held::Unchecked { folder } => {
            return Some(Say::Warn(
                format!(
                    "'{ws}/{}' linked to a git directory purlis could not confirm, so its \
                 repository may still list it as a worktree; `git worktree prune` in that \
                 repository clears the record.",
                    crate::personas::one_line(folder)
                )
                .into(),
            ));
        }
    };
    let prune = format!("git -C {} worktree prune", main.display());
    // Something at that place again is a folder git would delete, not a record it drops.
    if std::fs::symlink_metadata(recorded).is_ok() {
        return Some(Say::Warn(
            format!(
                "Something is at {recorded} again, so purlis left git's record of that worktree \
             alone; once the place is clear, `{prune}` clears it."
            )
            .into(),
        ));
    }
    let ran = crate::worktree::git::run(
        main,
        &["worktree", "remove", "--", recorded],
        crate::worktree::git::READ,
    );
    match ran {
        Ok(run) if run.ok() => None,
        Ok(run) => Some(Say::Warn(
            format!(
                "git still lists {recorded} as a worktree ({}); `{prune}` clears it.",
                crate::personas::one_line(run.err.trim())
            )
            .into(),
        )),
        Err(why) => Some(Say::Warn(
            format!(
                "git did not run to clear its record of {recorded} ({why}); `{prune}` clears it."
            )
            .into(),
        )),
    }
}

/// The refusal for a workspace with chats running in it: which, and what to do first.
fn still_running(name: &str, running: &[String]) -> String {
    let (are, them) = if running.len() == 1 {
        ("a chat is", "it")
    } else {
        ("chats are", "them")
    };
    format!(
        "Refusing to remove '{name}' — {are} running in it: {}. Removing it would leave {them} \
         running in a deleted folder; close {them} first.",
        running.join(", ")
    )
}

/// What the removal leaves in the project's dispatch records (#1520), said, and only where
/// there is something to say: what the tasks that worked in `ws` and their asking chats said to
/// each other is forgotten, and the records themselves, with each brief and report, stay until
/// they are collected. A store that cannot be read here is said so: its words stay until they
/// expire.
fn dispatches_left_behind(root: &Path, ws: &str) -> Vec<Say> {
    match crate::dispatchrecord::workspace_removed(root, ws) {
        Ok(left) if left.records == 0 => Vec::new(),
        Ok(left) => vec![Say::Info(left_behind_said(ws, left).into())],
        Err(why) => vec![Say::Warn(
            format!(
                "purlis could not read the project's dispatch records here ({}), so what tasks \
             that worked in '{ws}' and their asking chats said to each other stays until it \
             expires, 30 days after each task ended.",
                crate::rewrite::os_words(&why)
            )
            .into(),
        )],
    }
}

/// The sentence for `left`, true of each part of it: the records that stay, the words that
/// were forgotten, and the running tasks that keep theirs.
fn left_behind_said(ws: &str, left: crate::dispatchrecord::LeftBehind) -> String {
    let many = |n: usize, one: &str, more: &str| {
        if n == 1 {
            format!("1 {one}")
        } else {
            format!("{n} {more}")
        }
    };
    let mut said = format!(
        "Kept {} of work in '{ws}' in the project's own state, each with its brief and report, \
         until it is collected 30 days after its task ended, or after its report became owed \
         to a chat that asked, where it is still owed.",
        many(left.records, "dispatch record", "dispatch records")
    );
    if left.forgot > 0 {
        said.push_str(&format!(
            " What {} and the chats that asked said to each other is forgotten now.",
            many(left.forgot, "task that ended", "tasks that ended")
        ));
    }
    if left.running > 0 {
        said.push_str(&format!(
            " {} still running {} what {} chats say until {} end, and for 30 days after.",
            many(left.running, "task", "tasks"),
            if left.running == 1 { "keeps" } else { "keep" },
            if left.running == 1 { "its" } else { "their" },
            if left.running == 1 { "it" } else { "they" },
        ));
    }
    said
}

/// What became of purlis's block in an exclude file outside workspace `ws`, said.
fn withdrawn_said(ws: &str, done: &crate::guest::Withdrawal) -> Say {
    use crate::guest::Withdrawn;
    let (tree, exclude) = (done.tree.display(), done.exclude.display());
    let left = |why: &str| {
        Say::Warn(
            format!(
                "{tree} is a checkout whose git directory is outside '{ws}', so removing the \
             workspace leaves purlis's generated-layer block in {exclude} — it will name paths \
             that no longer exist. {why} Delete the block between `{}` and `{}` in that file.",
                crate::guest::EXCLUDE_BEGIN,
                crate::guest::EXCLUDE_END,
            )
            .into(),
        )
    };
    match &done.did {
        Withdrawn::Cleared => Say::Info(
            format!(
                "Removed purlis's generated-layer block from {exclude}, which {tree} read from \
             outside '{ws}'."
            )
            .into(),
        ),
        Withdrawn::Narrowed(kept) => Say::Info(
            format!(
                "Took the lines only '{ws}' needed out of purlis's generated-layer block in \
             {exclude}; {kept} {} for another checkout purlis wires there.",
                if *kept == 1 {
                    "line stays"
                } else {
                    "lines stay"
                }
            )
            .into(),
        ),
        Withdrawn::Kept(why) => left(&format!(
            "purlis left it whole because it could not account for every line in it ({}).",
            why.join("; ")
        )),
        Withdrawn::Blocked(why) => left(&format!("purlis could not rewrite it ({why}).")),
    }
}

#[cfg(test)]
mod tests;
