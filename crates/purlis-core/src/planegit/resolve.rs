// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Resolve a save's conflict from the window** (qw382): the person's own press runs the
//! rebase a save's push retry could not finish, and git's part of settling it, while a chat
//! may edit the files it stopped in.
//!
//! A chat may not run git that rewrites the project root's tree (the A3c guard,
//! [`crate::rootupdate`]), and that stays so. Here the window runs every git command, under no
//! chat's sandbox, each one the person's press:
//!
//! - [`start`] fetches the target branch from the remote recorded for the project and replays
//!   the project's own commits onto it. Where git stops on a conflict, the rebase is left
//!   stopped, and the answer names the files.
//! - [`carry_on`] is *Continue*: it checks that no conflict marker is left in those files,
//!   stages exactly them, and runs `git rebase --continue`. Where git stops again, the answer
//!   names the next files.
//! - [`abort`] is *Abort*: `git rebase --abort`, which puts the project back where it was.
//!
//! **What the window stages is decided by git and the window, never by what a chat wrote.**
//! The files staged are those git still calls unmerged and those the window was shown, and
//! every one of them must be a file the commit being replayed changes: anything else is
//! refused, as is anything already staged that is not one of those. The commands are fixed
//! here, and file names reach git after `--` with literal pathspecs.

use std::collections::BTreeSet;
use std::path::Path;

use super::{
    Claim, Rebased, WRITE, changed_paths, count, fetch_branch, forget_replayed_push, gpgsign,
    rebase_in_progress, rebase_onto, tail,
};
use crate::gitstate;
use crate::repocmd::{Say, Sink};
use crate::worktree::git;

/// How a press of Resolve, Continue or Abort ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Settled {
    /// The project's own commits are on the remote's now. What is left is the save's push.
    Replayed,
    /// git stopped on a conflict in these files. The rebase is left stopped for the person.
    Stopped(Vec<String>),
    /// The rebase was undone: the project is where it was before Resolve.
    Aborted,
    /// Nothing was changed, and why, as a sentence for the person.
    Refused(String),
}

/// The most of a file read for conflict markers.
const MOST_READ: u64 = 16 * 1024 * 1024;

/// What a press says while a save or a fetch holds the project.
const BUSY: &str = "A save of this project is running. Wait for it to finish, then try again.";

/// **Resolve**: fetch the target branch and replay the project's own commits onto it, stopping
/// where git stops. For the person's own press in the window, and nothing else.
pub fn start(root: &Path, say: Sink) -> Settled {
    let Some(_claim) = Claim::of(root) else {
        return Settled::Refused(BUSY.into());
    };
    let plane = crate::planesave::Settings::read(root).plane;
    if let Some(stopped) = gitstate::stopped(root) {
        return Settled::Refused(format!(
            "Nothing was started: {}. Continue or Abort it first.",
            stopped.why()
        ));
    }
    let here = git::run(root, &["rev-parse", "--abbrev-ref", "HEAD"], git::READ)
        .ok()
        .filter(git::Run::ok)
        .map(|r| r.line().trim().to_string())
        .unwrap_or_default();
    if here.is_empty() || here == "HEAD" {
        return Settled::Refused("Nothing was started: the project is on no branch.".into());
    }
    let branch = plane.branch.value.clone().unwrap_or_else(|| here.clone());
    if here != branch {
        return Settled::Refused(format!(
            "Nothing was started: the project is on `{here}`, and it saves into `{branch}`."
        ));
    }
    let changed = changed_paths(root);
    if !changed.is_empty() {
        return Settled::Refused(format!(
            "Save first: {} changed file(s) here are not saved, and a rebase never moves \
             unsaved work.",
            changed.len()
        ));
    }
    say(Say::Info(format!("Fetching `{branch}` from the remote …")));
    if let Err(why) = fetch_branch(root, &branch) {
        return Settled::Refused(format!(
            "Nothing was started: the remote could not be fetched: {}.",
            why.trim_end_matches('.')
        ));
    }
    let theirs = format!("refs/remotes/origin/{branch}");
    let Some(before) = head_of(root) else {
        return Settled::Refused("Nothing was started: the project has no commit.".into());
    };
    if count(root, &format!("HEAD..{theirs}")) == Some(0) {
        forget_replayed_push(root);
        return Settled::Replayed;
    }
    say(Say::Info("Replaying this project's own commits …".into()));
    match rebase_onto(root, &theirs, plane.sign.value, WRITE) {
        Rebased::Replayed => {
            forget_replayed_push(root);
            Settled::Replayed
        }
        Rebased::Conflict(files) => Settled::Stopped(files),
        Rebased::Held(files) => Settled::Refused(format!(
            "Nothing was started: purlis may not write {}.",
            files.join(", ")
        )),
        other => {
            let said = match other {
                Rebased::Unsigned(signer) => {
                    format!("a replayed commit could not be signed: {signer}")
                }
                Rebased::OutOfTime => "the replay did not finish in time".into(),
                Rebased::Failed(said) => said,
                _ => String::new(),
            };
            match super::update::put_back(root, &before, &theirs) {
                Ok(()) => Settled::Refused(format!(
                    "The replay did not go through: {}. The project is exactly as it was.",
                    one_line(&said)
                )),
                Err(still) => Settled::Refused(format!(
                    "The replay stopped part-way: {}. {}",
                    one_line(&said),
                    super::update::said_not_put_back(&still)
                )),
            }
        }
    }
}

/// **Continue**: stage the files the conflict is in, once no conflict marker is left in them,
/// and let git carry on. `shown` are the files the window showed the person; git's own
/// unmerged files are added to them.
pub fn carry_on(root: &Path, shown: &[String], say: Sink) -> Settled {
    let Some(_claim) = Claim::of(root) else {
        return Settled::Refused(BUSY.into());
    };
    if !rebase_in_progress(root) {
        return Settled::Refused(
            "No rebase is stopped in this project, so there is nothing to continue.".into(),
        );
    }
    let Some(replayed) = replayed_paths(root) else {
        return Settled::Refused(
            "purlis cannot tell which commit git stopped on, so it stages nothing. Continue \
             it in a terminal, or Abort."
                .into(),
        );
    };
    let stage: BTreeSet<String> = shown
        .iter()
        .cloned()
        .chain(gitstate::unmerged(root))
        .collect();
    if let Some(error) = refuse_staging(root, &stage, &replayed) {
        return Settled::Refused(error);
    }
    let staged = staged_paths(root);
    let extra: Vec<&String> = staged
        .iter()
        .filter(|path| !replayed.contains(*path))
        .collect();
    if !extra.is_empty() {
        return Settled::Refused(format!(
            "{} {} staged, and the commit being replayed does not change it. Unstage it \
             (`git restore --staged -- <file>`), then Continue.",
            join(&extra),
            if extra.len() == 1 { "is" } else { "are" },
        ));
    }
    if !stage.is_empty() {
        let mut add: Vec<&str> = vec!["--literal-pathspecs", "add", "-A", "--"];
        add.extend(stage.iter().map(String::as_str));
        match git::run(root, &add, WRITE) {
            Ok(run) if run.ok() => {}
            Ok(run) => {
                return Settled::Refused(format!(
                    "git would not stage them: {}",
                    one_line(&tail(&run))
                ));
            }
            Err(unavailable) => return Settled::Refused(unavailable.to_string()),
        }
    }
    say(Say::Info("Continuing the replay …".into()));
    let plane = crate::planesave::Settings::read(root).plane;
    let continued = git::run(
        root,
        &[
            "-c",
            "core.editor=true",
            "-c",
            gpgsign(plane.sign.value),
            "rebase",
            "--continue",
        ],
        WRITE,
    );
    if !rebase_in_progress(root) {
        forget_replayed_push(root);
        return Settled::Replayed;
    }
    let unmerged = gitstate::unmerged(root);
    if !unmerged.is_empty() {
        return Settled::Stopped(unmerged);
    }
    Settled::Refused(format!(
        "git did not carry on: {}. The rebase is still stopped: Continue again, or Abort.",
        match continued {
            Ok(run) => one_line(&tail(&run)),
            Err(unavailable) => unavailable.to_string(),
        }
    ))
}

/// **Abort**: put the project back where it was before the rebase started.
pub fn abort(root: &Path) -> Settled {
    let Some(_claim) = Claim::of(root) else {
        return Settled::Refused(BUSY.into());
    };
    if !rebase_in_progress(root) {
        return Settled::Refused(
            "No rebase is stopped in this project, so there is nothing to abort.".into(),
        );
    }
    let aborted = git::run_untimed(root, &["rebase", "--abort"]);
    if rebase_in_progress(root) {
        return Settled::Refused(format!(
            "git did not abort the rebase: {}",
            match aborted {
                Ok(run) => one_line(&tail(&run)),
                Err(unavailable) => unavailable.to_string(),
            }
        ));
    }
    super::touch(root);
    Settled::Aborted
}

/// The two sides of a stopped rebase, by their commit subjects: the remote's (`HEAD` while a
/// rebase is stopped) and the project's own being replayed (`REBASE_HEAD`).
pub fn sides(root: &Path) -> Option<(String, String)> {
    let subject = |rev: &str| {
        git::run(root, &["log", "-1", "--format=%s", rev, "--"], git::READ)
            .ok()
            .filter(git::Run::ok)
            .map(|run| run.line().trim().to_string())
    };
    Some((subject("HEAD")?, subject("REBASE_HEAD")?))
}

/// Why `stage` may not be staged: a file the commit being replayed does not change, a file
/// that is not a plain one, or one with a conflict marker left in it. `None` when it may.
fn refuse_staging(
    root: &Path,
    stage: &BTreeSet<String>,
    replayed: &BTreeSet<String>,
) -> Option<String> {
    let outside: Vec<&String> = stage.iter().filter(|p| !replayed.contains(*p)).collect();
    if !outside.is_empty() {
        return Some(format!(
            "purlis stages only the files the conflict is in, and the commit being replayed \
             does not change {}. Nothing was staged.",
            join(&outside)
        ));
    }
    for path in stage {
        let file = root.join(path);
        match std::fs::symlink_metadata(&file) {
            Err(why) if why.kind() == std::io::ErrorKind::NotFound => {}
            Err(why) => return Some(format!("`{path}` cannot be read: {why}.")),
            Ok(meta) if !meta.is_file() => {
                return Some(format!(
                    "`{path}` is not a plain file, so purlis will not stage it. Settle it in a \
                     terminal, or Abort."
                ));
            }
            Ok(_) => {
                if has_markers(&file) {
                    return Some(format!(
                        "`{path}` still has conflict markers (`<<<<<<<`, `>>>>>>>`). Settle \
                         them, then Continue."
                    ));
                }
            }
        }
    }
    None
}

/// Whether a file still holds a line git opens or closes a conflict with.
fn has_markers(file: &Path) -> bool {
    use std::io::Read;
    let Ok(open) = std::fs::File::open(file) else {
        return true;
    };
    let mut bytes = Vec::new();
    if open.take(MOST_READ).read_to_end(&mut bytes).is_err() {
        return true;
    }
    markers_in(&bytes)
}

/// Whether `text` has a line that starts with git's conflict opener or closer.
pub fn markers_in(text: &[u8]) -> bool {
    text.split(|b| *b == b'\n')
        .any(|line| line.starts_with(b"<<<<<<<") || line.starts_with(b">>>>>>>"))
}

/// The paths the commit being replayed changes, or `None` when git cannot say.
fn replayed_paths(root: &Path) -> Option<BTreeSet<String>> {
    let run = git::run(
        root,
        &[
            "diff-tree",
            "--root",
            "--no-commit-id",
            "--no-renames",
            "-r",
            "--name-only",
            "-z",
            "REBASE_HEAD",
        ],
        git::READ,
    )
    .ok()
    .filter(git::Run::ok)?;
    let paths: BTreeSet<String> = run
        .out
        .split('\0')
        .filter(|p| !p.is_empty())
        .map(str::to_string)
        .collect();
    (!paths.is_empty()).then_some(paths)
}

/// The paths staged against HEAD now.
fn staged_paths(root: &Path) -> Vec<String> {
    git::run(
        root,
        &[
            "diff",
            "--cached",
            "--no-renames",
            "--name-only",
            "-z",
            "HEAD",
        ],
        git::READ,
    )
    .ok()
    .filter(git::Run::ok)
    .map(|run| {
        run.out
            .split('\0')
            .filter(|p| !p.is_empty())
            .map(str::to_string)
            .collect()
    })
    .unwrap_or_default()
}

fn head_of(root: &Path) -> Option<String> {
    git::run(root, &["rev-parse", "--verify", "-q", "HEAD"], git::READ)
        .ok()
        .filter(git::Run::ok)
        .map(|r| r.line().trim().to_string())
        .filter(|head| !head.is_empty())
}

fn join(paths: &[&String]) -> String {
    paths
        .iter()
        .map(|p| format!("`{p}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn one_line(said: &str) -> String {
    said.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::markers_in;

    #[test]
    fn a_conflicts_opener_or_closer_is_a_marker_and_a_heading_rule_is_not() {
        assert!(markers_in(
            b"a\n<<<<<<< HEAD\nb\n=======\nc\n>>>>>>> mine\n"
        ));
        assert!(markers_in(b">>>>>>> mine"));
        assert!(!markers_in(
            b"Title\n=======\n\ntext <<<<<<< inside a line\n"
        ));
    }
}
