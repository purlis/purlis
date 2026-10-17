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
        say(Say::Fail(format!(
            "invalid workspace name '{}' (use letters, digits, '.', '_', '-'; must not start \
             with a dot)",
            crate::personas::one_line(name)
        )));
        return Removal::just(1);
    };
    if !wscmd::workspace_dir_exists(root, name) {
        say(Say::Fail(format!("no workspace '{name}'")));
        return Removal::just(1);
    }
    // Before the guard and before anything is read below it: a `workspaces/<ws>` that is a
    // link out of the plane is a directory this command would delete somewhere else
    // entirely. Python's `shutil.rmtree` on a symlink raises; this refuses with a sentence.
    if let Err(why) = crate::contain::no_link_on_the_way(root, &dir) {
        say(Say::Fail(format!(
            "'{name}' does not resolve to a directory inside this plane, so nothing was \
             removed ({why})."
        )));
        return Removal::just(1);
    }
    if !running.is_empty() {
        say(Say::Fail(still_running(name, running)));
        return Removal::just(1);
    }

    let risky = wscmd::work_at_risk(root, name);
    if !risky.is_empty() && !force {
        let joined: Vec<String> = risky.iter().map(ToString::to_string).collect();
        say(Say::Fail(format!(
            "Refusing to remove '{name}' — this would discard work: {}. Push/commit first, \
             or pass --force.",
            joined.join("; ")
        )));
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
        say(Say::Warn(format!(
            "Discarding {} with '{name}' — nothing else holds {}.",
            counted(todos.len(), "open todo", "open todos"),
            plural(todos.len(), "it", "them")
        )));
    }

    // charter#870 and #998: purlis's lines in a repository kept elsewhere, before the delete
    // (see this module's header).
    for done in crate::guest::withdraw_for_removal(root, &dir) {
        say(withdrawn_said(name, &done));
    }
    // #998: read before the delete, cleared after it (see this module's header).
    let held = linked_worktrees(&dir);

    if let Err(why) = std::fs::remove_dir_all(&dir) {
        say(Say::Fail(format!(
            "could not remove '{name}' ({}) — some of it may still be there.",
            crate::rewrite::os_words(&why)
        )));
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
    say(Say::Done(format!(
        "Removed workspace '{name}' and its clones."
    )));
    if left > 0 {
        say(Say::Info(crate::dispatchwithin::left_behind_said(
            left, name, None,
        )));
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
            return Some(Say::Warn(format!(
                "'{ws}/{}' linked to a git directory purlis could not confirm, so its \
                 repository may still list it as a worktree; `git worktree prune` in that \
                 repository clears the record.",
                crate::personas::one_line(folder)
            )));
        }
    };
    let prune = format!("git -C {} worktree prune", main.display());
    // Something at that place again is a folder git would delete, not a record it drops.
    if std::fs::symlink_metadata(recorded).is_ok() {
        return Some(Say::Warn(format!(
            "Something is at {recorded} again, so purlis left git's record of that worktree \
             alone; once the place is clear, `{prune}` clears it."
        )));
    }
    let ran = crate::worktree::git::run(
        main,
        &["worktree", "remove", "--", recorded],
        crate::worktree::git::READ,
    );
    match ran {
        Ok(run) if run.ok() => None,
        Ok(run) => Some(Say::Warn(format!(
            "git still lists {recorded} as a worktree ({}); `{prune}` clears it.",
            crate::personas::one_line(run.err.trim())
        ))),
        Err(why) => Some(Say::Warn(format!(
            "git did not run to clear its record of {recorded} ({why}); `{prune}` clears it."
        ))),
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
        Ok(left) => vec![Say::Info(left_behind_said(ws, left))],
        Err(why) => vec![Say::Warn(format!(
            "purlis could not read the project's dispatch records here ({}), so what tasks \
             that worked in '{ws}' and their asking chats said to each other stays until it \
             expires, 30 days after each task ended.",
            crate::rewrite::os_words(&why)
        ))],
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
        Say::Warn(format!(
            "{tree} is a checkout whose git directory is outside '{ws}', so removing the \
             workspace leaves purlis's generated-layer block in {exclude} — it will name paths \
             that no longer exist. {why} Delete the block between `{}` and `{}` in that file.",
            crate::guest::EXCLUDE_BEGIN,
            crate::guest::EXCLUDE_END,
        ))
    };
    match &done.did {
        Withdrawn::Cleared => Say::Info(format!(
            "Removed purlis's generated-layer block from {exclude}, which {tree} read from \
             outside '{ws}'."
        )),
        Withdrawn::Narrowed(kept) => Say::Info(format!(
            "Took the lines only '{ws}' needed out of purlis's generated-layer block in \
             {exclude}; {kept} {} for another checkout purlis wires there.",
            if *kept == 1 {
                "line stays"
            } else {
                "lines stay"
            }
        )),
        Withdrawn::Kept(why) => left(&format!(
            "purlis left it whole because it could not account for every line in it ({}).",
            why.join("; ")
        )),
        Withdrawn::Blocked(why) => left(&format!("purlis could not rewrite it ({why}).")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn plane() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("charter.toml"), "").unwrap();
        dir
    }

    /// The command as a caller that only cares about the verdict sees it. The list it refused
    /// on has a test of its own below, where it is the claim rather than a detail.
    fn run(root: &Path, name: &str, force: bool) -> (u8, Vec<String>) {
        let (done, said) = run_fully(root, name, force);
        (done.code, said)
    }

    fn run_fully(root: &Path, name: &str, force: bool) -> (Removal, Vec<String>) {
        let mut said = Vec::new();
        let done = remove(root, name, force, &mut |line: Say| {
            said.push(line.to_string())
        });
        (done, said)
    }

    /// A real repository, because the guard's whole job is to read one.
    fn repo(at: &Path) -> PathBuf {
        std::fs::create_dir_all(at).unwrap();
        git(at, &["init", "-q", "-b", "main"]);
        git(at, &["config", "user.email", "t@example.com"]);
        git(at, &["config", "user.name", "T"]);
        std::fs::write(at.join("README.md"), "hi\n").unwrap();
        git(at, &["add", "-A"]);
        git(at, &["commit", "-qm", "first"]);
        at.to_path_buf()
    }

    fn git(at: &Path, argv: &[&str]) {
        let run = crate::testgit::run(at, argv);
        assert!(run.ok(), "git {argv:?} failed: {}", run.err);
    }

    #[test]
    fn git_is_handed_only_a_folder_of_the_workspace_spelled_plainly() {
        let going = Path::new("/var/p/workspaces/beta");
        let inside = Path::new("/private/var/p/workspaces/beta");
        for ok in [
            "/var/p/workspaces/beta/guest",
            "/private/var/p/workspaces/beta/guest",
            "/var/p/workspaces/beta/a/b",
        ] {
            assert!(plainly_inside(Path::new(ok), going, inside), "{ok}");
        }
        for refused in [
            "/var/p/workspaces/beta",
            "/var/p/workspaces/betamax/guest",
            "/var/p/workspaces/beta/../gamma/guest",
            "/var/p/workspaces/beta/guest/..",
            "/elsewhere/guest",
            "guest",
        ] {
            assert!(
                !plainly_inside(Path::new(refused), going, inside),
                "{refused}"
            );
        }
    }

    #[test]
    fn an_empty_workspace_is_removed() {
        let dir = plane();
        std::fs::create_dir_all(dir.path().join("workspaces/beta")).unwrap();
        let (code, said) = run(dir.path(), "beta", false);
        assert_eq!(code, 0, "{said:?}");
        assert!(!dir.path().join("workspaces/beta").exists());
        assert_eq!(said, vec!["✓ Removed workspace 'beta' and its clones."]);
    }

    #[test]
    fn a_workspace_a_chat_runs_in_is_refused_forced_or_not_and_kept() {
        // V27d (ADR 0076 §10): a removal must not leave a program running in a deleted folder.
        let dir = plane();
        std::fs::create_dir_all(dir.path().join("workspaces/beta")).unwrap();
        for force in [false, true] {
            let mut said = Vec::new();
            let done = remove_unless_running(
                dir.path(),
                "beta",
                force,
                &["steward 1".to_owned(), "codex 2".to_owned()],
                &mut |line: Say| said.push(line.to_string()),
            );
            assert_eq!(done.code, 1, "{said:?}");
            assert_eq!(
                said,
                vec![
                    "✗ Refusing to remove 'beta' — chats are running in it: steward 1, codex 2. \
                     Removing it would leave them running in a deleted folder; close them first."
                ]
            );
            assert!(dir.path().join("workspaces/beta").is_dir());
        }

        // Once nothing runs in it, it goes.
        let mut said = Vec::new();
        let done = remove_unless_running(dir.path(), "beta", false, &[], &mut |line: Say| {
            said.push(line.to_string())
        });
        assert_eq!(done.code, 0, "{said:?}");
        assert!(!dir.path().join("workspaces/beta").exists());
    }

    #[test]
    fn with_no_app_listening_no_chat_runs_in_any_workspace() {
        let dir = plane();
        std::fs::create_dir_all(dir.path().join("workspaces/beta")).unwrap();
        assert!(wscmd::rename::open_in_app(dir.path(), &["beta"]).is_empty());
        assert!(wscmd::rename::adopting_in_app(dir.path(), "steward").is_empty());
    }

    #[test]
    fn removing_a_workspace_forgets_what_its_tasks_said_and_says_what_it_leaves() {
        // #1520: the records of work in it stay, each with its brief and report; the words its
        // tasks and their asking chats sent each other go with the workspace.
        use crate::dispatchrecord::{self, Asker, Mode, Opening, Place, Worker};
        let dir = plane();
        std::fs::create_dir_all(dir.path().join("workspaces/beta")).unwrap();
        let now = chrono::Utc::now();
        let opened = dispatchrecord::open(
            dir.path(),
            Opening {
                mode: Mode::Task,
                asker: Asker::default(),
                persona: None,
                worker: Worker::default(),
                task: None,
                place: Place {
                    workspace: Some("beta".to_owned()),
                    ..Place::default()
                },
                brief: "Check prod.".to_owned(),
                report_owed: false,
            },
            now,
        )
        .unwrap();
        dispatchrecord::said(
            dir.path(),
            &opened.id,
            crate::dispatchtalk::Kind::Note,
            "Still up.",
            now,
        )
        .unwrap();
        dispatchrecord::close(
            dir.path(),
            &opened.id,
            dispatchrecord::Ending::default(),
            now,
        )
        .unwrap();

        let (code, said) = run(dir.path(), "beta", false);

        assert_eq!(code, 0, "{said:?}");
        assert!(
            said.iter().any(
                |line| line.contains("Kept 1 dispatch record of work in 'beta'")
                    && line.contains("forgotten now")
            ),
            "{said:?}"
        );
        let kept = dispatchrecord::read(dir.path(), &opened.id).expect("the record stays");
        assert_eq!(kept.brief, "Check prod.");
        assert_eq!(kept.talk[0].text, "");
    }

    #[test]
    fn what_a_removal_leaves_is_said_true_of_each_record() {
        use crate::dispatchrecord::LeftBehind;
        let said = |records, forgot, running| {
            left_behind_said(
                "beta",
                LeftBehind {
                    records,
                    forgot,
                    running,
                },
            )
        };
        assert_eq!(
            said(1, 1, 0),
            "Kept 1 dispatch record of work in 'beta' in the project's own state, each with its \
             brief and report, until it is collected 30 days after its task ended, or after its \
             report became owed to a chat that asked, where it is still owed. What 1 \
             task that ended and the chats that asked said to each other is forgotten now."
        );
        // Some ended and some run on: the running ones are not said to be forgotten.
        let mixed = said(3, 1, 2);
        assert!(mixed.contains("What 1 task that ended"), "{mixed}");
        assert!(
            mixed.ends_with(
                "2 tasks still running keep what their chats say until they end, and for 30 \
                 days after."
            ),
            "{mixed}"
        );
        // All running: nothing is said to be forgotten.
        let running = said(1, 0, 1);
        assert!(!running.contains("forgotten"), "{running}");
        assert!(running.contains("1 task still running keeps what its chats say"));
    }

    #[test]
    fn a_workspace_this_plane_does_not_have_is_refused() {
        let dir = plane();
        let (code, said) = run(dir.path(), "nope", false);
        assert_eq!(code, 1);
        assert_eq!(said, vec!["✗ no workspace 'nope'"]);
    }

    #[test]
    fn a_dirty_clone_stops_the_removal_and_nothing_is_deleted() {
        let dir = plane();
        let clone = repo(&dir.path().join("workspaces/beta/svc"));
        std::fs::write(clone.join("README.md"), "changed\n").unwrap();

        let (code, said) = run(dir.path(), "beta", false);
        assert_eq!(code, 2, "{said:?}");
        assert!(said[0].contains("svc: uncommitted changes"), "{said:?}");
        assert!(said[0].contains("or pass --force"), "{said:?}");
        assert!(clone.exists(), "the guard fired, so nothing was deleted");
    }

    /// **The refusal hands back the reading it was made on** (charter-app#182).
    ///
    /// Not a convenience. The surface that asks for consent draws a preview from an earlier
    /// reading of the same guard, and between the two the workspace can move; without this,
    /// the only list a caller has to name what is being discarded is that older one. The claim
    /// here is the identity: every sentence in the list is a sentence in the refusal, and
    /// nothing is in one and not the other.
    #[test]
    fn the_refusal_carries_the_list_it_refused_on() {
        // Two, so the assertion is about a LIST and not about one sentence that happens to
        // match: the failure #182 is about is a button naming one clone while the refusal
        // above it names another.
        let dir = plane();
        for name in ["svc", "lib"] {
            let clone = repo(&dir.path().join("workspaces/beta").join(name));
            std::fs::write(clone.join("README.md"), "changed\n").unwrap();
        }

        let (done, said) = run_fully(dir.path(), "beta", false);

        assert_eq!(done.code, 2, "{said:?}");
        let named: Vec<&str> = done
            .refused_over
            .iter()
            .map(|risk| risk.what.as_str())
            .collect();
        assert_eq!(named, vec!["lib", "svc"], "{said:?}");
        for risk in &done.refused_over {
            assert!(
                said[0].contains(&risk.said),
                "the refusal says {:?} and the list does not: {said:?}",
                risk.said
            );
        }
    }

    /// **And nothing else hands one back.** A caller drawing "what this discarded" from a
    /// non-empty list would be drawing it on a delete that went through, and on a name that
    /// was never a workspace.
    #[test]
    fn every_outcome_but_the_guards_refusal_refuses_over_nothing() {
        let dir = plane();
        let clone = repo(&dir.path().join("workspaces/beta/svc"));
        std::fs::write(clone.join("README.md"), "changed\n").unwrap();

        assert_eq!(run_fully(dir.path(), "nope", false).0.refused_over, vec![]);
        assert_eq!(
            run_fully(dir.path(), "../escape", false).0.refused_over,
            vec![]
        );
        // Forced, over work the guard did find: `--force` is the operator discarding it, so
        // what went is said in the lines and never as something that stopped the command.
        let (done, said) = run_fully(dir.path(), "beta", true);
        assert_eq!(done.code, 0, "{said:?}");
        assert_eq!(done.refused_over, vec![]);
    }

    #[test]
    fn an_untracked_file_is_uncommitted_work_too() {
        let dir = plane();
        let clone = repo(&dir.path().join("workspaces/beta/svc"));
        std::fs::write(clone.join("scratch.txt"), "notes\n").unwrap();

        let (code, said) = run(dir.path(), "beta", false);
        assert_eq!(code, 2, "{said:?}");
        assert!(said[0].contains("svc: uncommitted changes"), "{said:?}");
    }

    #[test]
    fn unpushed_commits_stop_the_removal() {
        let dir = plane();
        let origin = repo(&dir.path().join("origin"));
        git(&origin, &["config", "receive.denyCurrentBranch", "ignore"]);
        let clone = dir.path().join("workspaces/beta/svc");
        std::fs::create_dir_all(clone.parent().unwrap()).unwrap();
        git(
            dir.path(),
            &[
                "clone",
                "-q",
                &origin.display().to_string(),
                &clone.display().to_string(),
            ],
        );
        git(&clone, &["config", "user.email", "t@example.com"]);
        git(&clone, &["config", "user.name", "T"]);
        std::fs::write(clone.join("second.md"), "x\n").unwrap();
        git(&clone, &["add", "-A"]);
        git(&clone, &["commit", "-qm", "second"]);

        let (code, said) = run(dir.path(), "beta", false);
        assert_eq!(code, 2, "{said:?}");
        assert!(said[0].contains("svc: 1 unpushed commit"), "{said:?}");
        assert!(clone.exists());

        // …and `--force` is the way past it, which is what makes the guard a guard rather
        // than a wall.
        let (code, said) = run(dir.path(), "beta", true);
        assert_eq!(code, 0, "{said:?}");
        assert!(!dir.path().join("workspaces/beta").exists());
    }

    #[test]
    fn a_clone_with_no_upstream_at_all_is_not_work_at_risk() {
        // charter#104: a fresh local repository has no upstream from the moment it exists, so
        // "has no upstream" as the rule refuses over every workspace with nothing to lose —
        // and a guard that fires on the harmless common case teaches the `--force` habit.
        let dir = plane();
        repo(&dir.path().join("workspaces/beta/svc"));
        let (code, said) = run(dir.path(), "beta", false);
        assert_eq!(code, 0, "{said:?}");
    }

    #[test]
    fn a_detached_head_with_unpushed_work_still_stops_the_removal() {
        let dir = plane();
        let clone = repo(&dir.path().join("workspaces/beta/svc"));
        std::fs::write(clone.join("wip.md"), "x\n").unwrap();
        git(&clone, &["add", "-A"]);
        git(&clone, &["commit", "-qm", "wip"]);
        git(&clone, &["checkout", "-q", "--detach", "HEAD"]);
        // Detached, clean, and its commit is still on `main`, so there is nothing unique:
        // charter does not refuse over it, and that is the same rule as the worktree one.
        let (code, _said) = run(dir.path(), "beta", false);
        assert_eq!(code, 0);

        let clone = repo(&dir.path().join("workspaces/gamma/svc"));
        git(&clone, &["checkout", "-q", "--detach", "HEAD"]);
        std::fs::write(clone.join("only-here.md"), "x\n").unwrap();
        git(&clone, &["add", "-A"]);
        git(&clone, &["commit", "-qm", "only here"]);
        // Now the tree is dirty-free but holds a commit no branch reaches. A CLONE is judged
        // by `status`, which is clean, so this one is removable — and the same shape inside a
        // worktree is refused, because that is where the unique-commit rule applies.
        let (code, _said) = run(dir.path(), "gamma", false);
        assert_eq!(code, 0);
    }

    #[test]
    fn a_worktree_holding_commits_that_exist_nowhere_else_stops_the_removal() {
        let dir = plane();
        let clone = repo(&dir.path().join("workspaces/beta/svc"));
        let piece = dir.path().join("workspaces/beta/.worktrees/svc/task");
        git(
            &clone,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                "task",
                &piece.display().to_string(),
            ],
        );
        std::fs::write(piece.join("work.md"), "x\n").unwrap();
        git(&piece, &["add", "-A"]);
        git(&piece, &["commit", "-qm", "unique"]);

        let (code, said) = run(dir.path(), "beta", false);
        assert_eq!(code, 2, "{said:?}");
        assert!(
            said[0].contains("svc/task: 1 commit that exists nowhere else"),
            "{said:?}"
        );
        assert!(piece.exists());
    }

    #[test]
    fn a_worktree_holding_git_state_of_its_own_stops_the_removal() {
        // Clean, and nothing on it that no branch reaches: but a ref only that folder has,
        // which git deletes with it, as `wt remove` refuses over too (#1472).
        let dir = plane();
        let clone = repo(&dir.path().join("workspaces/beta/svc"));
        let piece = dir.path().join("workspaces/beta/.worktrees/svc/task");
        git(
            &clone,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                "task",
                &piece.display().to_string(),
            ],
        );
        git(&piece, &["update-ref", "refs/worktree/kept", "HEAD"]);

        let (code, said) = run(dir.path(), "beta", false);
        assert_eq!(code, 2, "{said:?}");
        assert!(
            said[0].contains("svc/task: refs/worktree/kept, a ref only this folder has"),
            "{said:?}"
        );
        assert!(piece.exists());
    }

    #[test]
    fn a_dirty_worktree_stops_the_removal() {
        let dir = plane();
        let clone = repo(&dir.path().join("workspaces/beta/svc"));
        let piece = dir.path().join("workspaces/beta/.worktrees/svc/task");
        git(
            &clone,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                "task",
                &piece.display().to_string(),
            ],
        );
        std::fs::write(piece.join("README.md"), "changed\n").unwrap();

        let (code, said) = run(dir.path(), "beta", false);
        assert_eq!(code, 2, "{said:?}");
        assert!(
            said[0].contains("svc/task: uncommitted changes"),
            "{said:?}"
        );
    }

    #[test]
    fn a_clean_worktree_whose_branch_is_reachable_elsewhere_does_not_stop_the_removal() {
        let dir = plane();
        let clone = repo(&dir.path().join("workspaces/beta/svc"));
        let piece = dir.path().join("workspaces/beta/.worktrees/svc/task");
        git(
            &clone,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                "task",
                &piece.display().to_string(),
            ],
        );
        let (code, said) = run(dir.path(), "beta", false);
        assert_eq!(code, 0, "{said:?}");
    }

    #[test]
    fn a_clone_charter_cannot_read_is_at_risk_rather_than_empty() {
        // charter#917, and the sharpest instance of it in the plane: what the list is empty of
        // is what is handed to a recursive delete.
        let dir = plane();
        let clone = dir.path().join("workspaces/beta/svc");
        std::fs::create_dir_all(clone.join(".git")).unwrap();
        // A `.git` directory with nothing in it: git answers "not a repository", which is a
        // failed `status` and not a clean one.
        let (code, said) = run(dir.path(), "beta", false);
        assert_eq!(code, 2, "{said:?}");
        assert!(said[0].contains("svc: could not be read"), "{said:?}");
        assert!(clone.exists());
    }

    #[test]
    fn a_git_dot_git_that_is_a_symlink_is_refused_and_counts_as_at_risk() {
        let dir = plane();
        let outside = tempfile::tempdir().unwrap();
        let real = repo(&outside.path().join("real"));
        let clone = dir.path().join("workspaces/beta/svc");
        std::fs::create_dir_all(&clone).unwrap();
        std::os::unix::fs::symlink(real.join(".git"), clone.join(".git")).unwrap();

        let (code, said) = run(dir.path(), "beta", false);
        assert_eq!(code, 2, "{said:?}");
        assert!(said[0].contains("svc"), "{said:?}");
        assert!(real.exists(), "nothing outside the plane was touched");
    }

    #[test]
    fn open_todos_are_reported_and_never_guard_the_removal() {
        let dir = plane();
        let ws = crate::workspaces::Plane::open(dir.path())
            .workspace("beta")
            .unwrap();
        ws.add_todo("ship the thing", "2026-05-04T11:32:17".parse().unwrap())
            .unwrap();

        let (code, said) = run(dir.path(), "beta", false);
        assert_eq!(
            code, 0,
            "a todo is a note about the future, not work: {said:?}"
        );
        assert!(
            said.iter()
                .any(|l| l.contains("Discarding 1 open todo with 'beta'")),
            "{said:?}"
        );
        assert!(!dir.path().join("workspaces/beta").exists());
    }

    #[test]
    fn a_workspace_that_is_a_link_out_of_the_plane_is_refused_and_its_target_survives() {
        let dir = plane();
        let outside = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(outside.path().join("treasure")).unwrap();
        std::fs::write(outside.path().join("treasure").join("keep.txt"), "x").unwrap();
        std::fs::create_dir_all(dir.path().join("workspaces")).unwrap();
        std::os::unix::fs::symlink(
            outside.path().join("treasure"),
            dir.path().join("workspaces").join("beta"),
        )
        .unwrap();

        let (code, said) = run(dir.path(), "beta", false);
        assert_eq!(code, 1, "{said:?}");
        assert!(
            said[0].contains("does not resolve to a directory inside this plane"),
            "{said:?}"
        );
        assert!(outside.path().join("treasure").join("keep.txt").exists());
    }

    /// A workspace `beta` holding `guest`, a linked worktree of a repository outside the
    /// plane whose exclude holds the operator's line and purlis's block listing one path; the
    /// exclude file and the operator's text.
    fn beta_reading_an_exclude_elsewhere(plane: &Path, outside: &Path) -> (PathBuf, &'static str) {
        let main = repo(&outside.join("main"));
        let piece = plane.join("workspaces/beta/guest");
        std::fs::create_dir_all(piece.parent().unwrap()).unwrap();
        git(
            &main,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                "guest",
                &piece.display().to_string(),
            ],
        );
        let exclude = main.join(".git").join("info").join("exclude");
        std::fs::create_dir_all(exclude.parent().unwrap()).unwrap();
        let theirs = "# mine\n/notes.txt\n";
        std::fs::write(
            &exclude,
            format!(
                "{theirs}{}\n/.claude/settings.json\n{}\n",
                crate::guest::EXCLUDE_BEGIN,
                crate::guest::EXCLUDE_END
            ),
        )
        .unwrap();
        (exclude, theirs)
    }

    #[test]
    fn an_exclude_block_the_delete_cannot_reach_is_cleared_and_only_it() {
        // charter#870, #998: a checkout inside the workspace whose git directory is elsewhere
        // would keep purlis's managed block after the workspace is gone. No other checkout
        // purlis wires reads that exclude, so the whole block goes, and the operator's line
        // above it stays byte for byte.
        let dir = plane();
        let outside = tempfile::tempdir().unwrap();
        let (exclude, theirs) = beta_reading_an_exclude_elsewhere(dir.path(), outside.path());

        let (code, said) = run(dir.path(), "beta", true);

        assert_eq!(code, 0, "{said:?}");
        assert_eq!(std::fs::read_to_string(&exclude).unwrap(), theirs);
        assert!(
            said.iter()
                .any(|l| l.starts_with("• Removed purlis's generated-layer block from")),
            "{said:?}"
        );
    }

    #[test]
    fn an_exclude_block_with_a_line_purlis_cannot_account_for_is_left_whole_and_named() {
        // Leaving the block is the safe direction: a worktree git cannot list (its record has
        // no `gitdir`) may be a checkout still needing every line.
        let dir = plane();
        let outside = tempfile::tempdir().unwrap();
        let (exclude, _) = beta_reading_an_exclude_elsewhere(dir.path(), outside.path());
        let before = std::fs::read_to_string(&exclude).unwrap();
        let stray = outside.path().join("main/.git/worktrees/stray");
        std::fs::create_dir_all(&stray).unwrap();

        let (code, said) = run(dir.path(), "beta", true);

        assert_eq!(code, 0, "{said:?}");
        assert_eq!(std::fs::read_to_string(&exclude).unwrap(), before);
        assert!(
            said.iter()
                .any(|l| l.contains("leaves purlis's generated-layer block")
                    && l.contains("could not account for every line")
                    && l.contains("worktree repair")),
            "{said:?}"
        );
    }
}
