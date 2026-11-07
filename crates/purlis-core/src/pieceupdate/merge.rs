// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis worktree update --merge` (#5253): a branch folder's base merged into its branch by
//! the app, for a chat whose branch is already pushed. No commit is rewritten, so a plain push
//! follows. It stops on a conflict and names the files, and `--continue` and `--abort` go on and
//! back as they do for a rebase, only for a merge purlis started.
//!
//! **The app runs this outside the chat's sandbox, in a folder the chat writes.** So it starts
//! nothing the chat could have written, and that is held in three places:
//!
//! - **The runner** ([`git`]) turns off every hook and every program a repository's config names,
//!   on every call: merge drivers, filters, editors and signers, and a branch's `mergeOptions`,
//!   read from the config as git would read it at that call, whichever file it came from.
//! - **[`MERGE`] and [`MERGE_OPTIONS`]** pin what this merge asks of git, past anything in a
//!   config file, a strategy above all: `-s` is given here, so no `pull.twohead` names one.
//! - **`.gitattributes`** can only choose among what config defines, so it reaches nothing once
//!   the runner has held that. A file a filter is marked for is staged by `--continue` as a
//!   rebase's is, which refuses it.
//!
//! **Which files take the base's copy is one fixed rule, never a list the chat writes**
//! ([`is_baseline`]): a `.png` under a `__screenshots__` folder, which CI redraws. The chat says
//! only whether to, with `--take-base-baselines`, so no flag can make it lose a source file the
//! way a general `-X theirs` could, which also settles each hunk of a file that merged in part.

use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use super::{
    Brought, Here, MOVED, NewBase, Piece, Reach, brought_up, commit_of, first_line, is_ancestor,
    note, rerecord, stage_resolved, stopped_in, unstopped_and_clean, untracked_kept,
};
use crate::gitstate::{self, Operation};
use crate::repocmd::Say;
use crate::worktree::git;

/// What every merge here runs with, whatever the repository's config says: no editor and no
/// signer, nothing checked against one, no upkeep after it, a resolution recorded by nobody, and
/// no file written where only a rename seen across a whole folder would put it, which the judging
/// of a sandboxed chat's merge could not read in advance (#1948).
const MERGE: [&str; 14] = [
    "-c",
    "core.editor=:",
    "-c",
    "commit.gpgSign=false",
    "-c",
    "merge.verifySignatures=false",
    "-c",
    "merge.directoryRenames=false",
    "-c",
    "rerere.enabled=false",
    "-c",
    "gc.auto=0",
    "-c",
    "maintenance.auto=false",
];

/// What the merge is told outright, ahead of any option a config could give it.
const MERGE_OPTIONS: [&str; 11] = [
    "merge",
    "-s",
    "ort",
    "--ff",
    "--commit",
    "--no-edit",
    "--no-verify",
    "--no-gpg-sign",
    "--no-verify-signatures",
    "--no-autostash",
    "--no-stat",
];

/// The folder CI's screenshot baselines live in.
const BASELINES: &str = "__screenshots__";

/// How long a merge's base copies may take: thousands of pictures, written once. A deadline only
/// so a hung git does not hold the folder's lock for ever.
const TAKING: Duration = Duration::from_secs(600);

/// The file [`Note`] is kept in while a merge is stopped, with the update's others
/// ([`Here::notes`]).
const NOTE: &str = "purlis-update-merge";

/// What a merge purlis started notes: the branch, the commit it was at, what it merged and the
/// name it merged that under. `--continue` and `--abort` act only on the merge this describes.
struct Note {
    branch: String,
    head: String,
    onto: String,
    base: String,
}

impl Note {
    fn write(&self, here: &Here) -> Result<(), String> {
        let text = format!(
            "{}\n{}\n{}\n{}\n",
            self.branch, self.head, self.onto, self.base
        );
        note(&here.root, &here.notes, NOTE, &text)
            .map_err(|why| format!("purlis could not note the merge it starts: {why}"))
    }

    fn read(notes: &Path) -> Option<Self> {
        let text = std::fs::read_to_string(notes.join(NOTE)).ok()?;
        let mut lines = text.lines().map(str::to_owned);
        Some(Self {
            branch: lines.next()?,
            head: lines.next()?,
            onto: lines.next()?,
            base: lines.next()?,
        })
    }

    fn remove(notes: &Path) {
        let _ = std::fs::remove_file(notes.join(NOTE));
    }

    /// The note of the merge stopped in the folder now, only where that merge is the one purlis
    /// started: on the same branch at the same commit, merging the same commit. A note left from
    /// an earlier merge never vouches for one the person started since.
    fn of_the_stopped_merge(here: &Here) -> Option<Self> {
        let note = Self::read(&here.notes)?;
        // This merge is run with `--no-autostash`, so it has nothing to restore.
        if here
            .git_dir
            .join("MERGE_AUTOSTASH")
            .symlink_metadata()
            .is_ok()
        {
            return None;
        }
        let merging = std::fs::read_to_string(here.git_dir.join("MERGE_HEAD")).ok()?;
        let at = commit_of(&here.folder, "HEAD")?;
        (merging.trim() == note.onto && at == note.head && here.branch == note.branch)
            .then_some(note)
    }

    fn message(&self) -> String {
        format!("Merge {} into {}", self.base, self.branch)
    }
}

/// Whether git has a merge stopped in the folder, whoever started it.
pub(super) fn is_stopped(here: &Here) -> bool {
    stopped_in(here).is_some_and(|stopped| stopped.operation == Some(Operation::Merge))
}

/// Whether `path` is a screenshot baseline: a `.png` with a [`BASELINES`] folder above it.
fn is_baseline(path: &str) -> bool {
    let mut parts = path.split('/');
    let file = parts.next_back().unwrap_or_default();
    file.ends_with(".png") && parts.any(|folder| folder == BASELINES)
}

/// The base brought in: judged first for a sandboxed chat, refused where it would write over a
/// file on disk that the current commit does not track, and run on the ids read once, whatever a
/// ref names by the time git runs.
pub(super) fn start(
    piece: &Piece,
    here: &Here,
    base: Option<&str>,
    take_base_baselines: bool,
    reach: &Reach,
    say: &mut dyn FnMut(Say),
) -> Result<u8, String> {
    unstopped_and_clean(here)?;
    let brought = brought_up(piece, here, base, reach, say)?;
    let label = &here.label;
    let onto_id = commit_of(&here.folder, &brought.onto).ok_or_else(|| {
        format!(
            "{} names no commit here, so nothing was moved",
            brought.onto
        )
    })?;
    let head_id = commit_of(&here.folder, "HEAD")
        .ok_or_else(|| format!("{label} has no commit, so nothing was moved"))?;
    if is_ancestor(&here.folder, &onto_id, &head_id)? {
        if let Some(base) = &brought.new_base {
            rerecord(here, &here.branch, base, say);
        }
        say(Say::Done(
            format!("{label} already has everything on {}", brought.base).into(),
        ));
        return Ok(0);
    }
    if let Some(chat_dir) = reach.sandboxed_at {
        let root = piece.root.to_string_lossy();
        let place = crate::cloneupdate::Place {
            root: &root,
            chat_dir: Some(chat_dir),
            alone: true,
            // The app runs exactly this merge, with nothing beside it.
            sparse: true,
        };
        let folder = here.folder.to_string_lossy();
        if let Some(why) = crate::cloneupdate::judged_merge(
            &folder,
            &onto_id,
            &head_id,
            &place,
            &crate::cloneupdate::Asked,
        ) {
            return Err(why);
        }
    }
    untracked_kept(here, &head_id, &onto_id)?;
    // The merge judged is that of the head read above: a branch moved since is not merged.
    if commit_of(&here.folder, "HEAD").as_deref() != Some(head_id.as_str()) {
        return Err(MOVED.to_owned());
    }
    let note = Note {
        branch: here.branch.clone(),
        head: head_id,
        onto: onto_id,
        base: brought.base.clone(),
    };
    note.write(here)?;
    NewBase::write(here, brought.new_base.as_deref())?;
    let message = note.message();
    let mut args: Vec<&str> = MERGE.to_vec();
    args.extend(MERGE_OPTIONS);
    args.extend(["-m", &message, &note.onto]);
    // Untimed: a merge checks trees out, and one killed halfway is left half done.
    let went = git::run_untimed(&here.folder, &args).map_err(|e| e.to_string())?;
    if went.ok() {
        return Ok(finished(here, &note, &brought, say));
    }
    if !is_stopped(here) {
        Note::remove(&here.notes);
        let _ = NewBase::write(here, None);
        let said = if went.err.trim().is_empty() {
            &went.out
        } else {
            &went.err
        };
        return Err(format!(
            "git did not merge {} into {label}: {}. Nothing was moved",
            brought.base,
            first_line(said).trim_end_matches('.')
        ));
    }
    let conflicted = gitstate::unmerged(&here.folder);
    if take_base_baselines {
        take_the_bases_copy(here, &note.onto, &conflicted)?;
    }
    let left = gitstate::unmerged(&here.folder);
    if left.is_empty() {
        committed(here, &note)?;
        return Ok(finished(here, &note, &brought, say));
    }
    let mut stopped = format!(
        "the merge of {} into {label} stopped on a conflict in {}. Resolve them in the folder, \
         then `purlis worktree update --continue`; or `purlis worktree update --abort` puts the \
         branch back where it was",
        brought.base,
        left.join(", ")
    );
    if !take_base_baselines && left.iter().any(|path| is_baseline(path)) {
        stopped.push_str(
            ". For screenshot baselines, which CI redraws, abort and run `purlis worktree update \
             --merge --take-base-baselines`, and each takes the base's copy",
        );
    }
    say(Say::Fail(stopped.into()));
    Ok(1)
}

/// Each of `conflicted` that is a baseline ([`is_baseline`]) set to what `onto` has, which may be
/// nothing. What `onto` has that is not a plain file is left in conflict, for the chat to read.
fn take_the_bases_copy(here: &Here, onto: &str, conflicted: &[String]) -> Result<(), String> {
    let baselines: Vec<&String> = conflicted.iter().filter(|path| is_baseline(path)).collect();
    if baselines.is_empty() {
        return Ok(());
    }
    let listed = git::run(
        &here.folder,
        &["ls-tree", "-r", "-z", "--full-tree", onto],
        git::READ,
    )
    .map_err(|e| e.to_string())?;
    if !listed.ok() {
        return Err(format!(
            "git could not say what the base has: {}. Finish with `purlis worktree update --abort`",
            first_line(&listed.err).trim_end_matches('.')
        ));
    }
    let modes: HashMap<&str, &str> = listed
        .out
        .split('\0')
        .filter_map(|entry| {
            let (meta, path) = entry.split_once('\t')?;
            Some((path, meta.split(' ').next()?))
        })
        .collect();
    let (mut copied, mut gone) = (Vec::new(), Vec::new());
    for path in baselines {
        match modes.get(path.as_str()) {
            Some(&("100644" | "100755")) => copied.push(path),
            Some(_) => {}
            None => gone.push(path),
        }
    }
    each_of(
        here,
        &[
            "checkout",
            onto,
            "--pathspec-from-file=-",
            "--pathspec-file-nul",
        ],
        &copied,
    )?;
    each_of(
        here,
        &[
            "rm",
            "-q",
            "-f",
            "--pathspec-from-file=-",
            "--pathspec-file-nul",
        ],
        &gone,
    )
}

/// `args` given each of `paths`, read literally, on its standard input: no name is an argument,
/// and however many there are they fit.
fn each_of(here: &Here, args: &[&str], paths: &[&String]) -> Result<(), String> {
    if paths.is_empty() {
        return Ok(());
    }
    let mut named = Vec::new();
    for path in paths {
        named.extend_from_slice(b":(literal)");
        named.extend_from_slice(path.as_bytes());
        named.push(0);
    }
    let ran = git::run_with_input(&here.folder, args, named, TAKING).map_err(|e| e.to_string())?;
    if ran.code != Some(0) {
        return Err(format!(
            "git did not take the base's copy of the baselines: {}. Finish with `purlis worktree \
             update --abort`",
            first_line(&String::from_utf8_lossy(&ran.err)).trim_end_matches('.')
        ));
    }
    Ok(())
}

/// The stopped merge committed, as what is staged and nothing else, with the message purlis wrote
/// and no text from git's own notes of the conflict.
fn committed(here: &Here, note: &Note) -> Result<(), String> {
    let message = note.message();
    let mut args: Vec<&str> = MERGE.to_vec();
    args.extend([
        "commit",
        "--no-verify",
        "--no-gpg-sign",
        "--no-status",
        "-m",
        &message,
    ]);
    let went = git::run_untimed(&here.folder, &args).map_err(|e| e.to_string())?;
    if went.ok() {
        return Ok(());
    }
    let said = if went.err.trim().is_empty() {
        &went.out
    } else {
        &went.err
    };
    Err(format!(
        "git did not commit the merge in {}: {}. It is still stopped: `purlis worktree update \
         --abort` puts the branch back",
        here.label,
        first_line(said).trim_end_matches('.')
    ))
}

/// **The merge commit signed** where the app runs this for a chat and holds a key (#5480): the
/// commits made since the branch's head and the base merged, which is the merge commit alone, or
/// none for a fast-forward. The branch is moved to the signed copy only from where git left it.
fn signed(here: &Here, note: &Note, say: &mut dyn FnMut(Say)) {
    let Some(head) = commit_of(&here.folder, "HEAD") else {
        return;
    };
    let signed = super::signed_since(&here.folder, head.clone(), &[&note.head, &note.onto], say);
    if signed == head {
        return;
    }
    let branch = crate::worktree::name::as_ref(&note.branch);
    if let Err(why) = crate::commitsign::moved(&here.folder, &branch, &signed, &head) {
        say(Say::Warn(
            format!("the merge commit is made and not signed: {why}").into(),
        ));
    }
}

/// The merge is done: its notes spent, a new base recorded, and said what follows.
fn finished(here: &Here, note: &Note, brought: &Brought, say: &mut dyn FnMut(Say)) -> u8 {
    signed(here, note, say);
    Note::remove(&here.notes);
    NewBase::recorded(here, &note.branch, say);
    say(Say::Done(
        format!(
            "{} has {} merged in{}. No commit was rewritten, so `purlis worktree push` publishes it",
            here.label,
            brought.base,
            brought.said_from()
        )
        .into(),
    ));
    0
}

/// `--continue`: the files git named staged, once resolved, and the merge committed.
pub(super) fn go_on(here: &Here, say: &mut dyn FnMut(Say)) -> Result<u8, String> {
    let label = &here.label;
    let note = Note::of_the_stopped_merge(here).ok_or_else(|| {
        format!(
            "the merge stopped in {label} is not one purlis started: finish it from your own \
             terminal"
        )
    })?;
    let files = gitstate::unmerged(&here.folder);
    if !files.is_empty() {
        stage_resolved(here, &files)?;
    }
    committed(here, &note)?;
    signed(here, &note, say);
    Note::remove(&here.notes);
    NewBase::recorded(here, &note.branch, say);
    say(Say::Done(
        format!(
            "{label} has {} merged in. No commit was rewritten, so `purlis worktree push` \
             publishes it",
            note.base
        )
        .into(),
    ));
    Ok(0)
}

/// `--abort`: the branch and its files back where they were, for a merge purlis started. It
/// writes files as the merge did, so for a sandboxed chat it is judged as the merge was, on what
/// it would write now (`sandboxed` is the root and the chat's folder). Fails closed.
pub(super) fn abort(
    here: &Here,
    sandboxed: Option<(&Path, &Path)>,
    say: &mut dyn FnMut(Say),
) -> Result<u8, String> {
    let label = &here.label;
    if Note::of_the_stopped_merge(here).is_none() {
        return Err(format!(
            "the merge stopped in {label} is not one purlis started: finish or abort it from \
             your own terminal"
        ));
    }
    if let Some((root, chat_dir)) = sandboxed {
        let staged = staged_apart_from_head(here)?;
        if let Some(why) = crate::cloneupdate::judged_merge_abort(
            &here.folder.to_string_lossy(),
            staged,
            &root.to_string_lossy(),
            chat_dir,
            &crate::cloneupdate::Asked,
        ) {
            return Err(why);
        }
    }
    let mut args: Vec<&str> = MERGE.to_vec();
    args.extend(["merge", "--abort"]);
    let back = git::run_untimed(&here.folder, &args).map_err(|e| e.to_string())?;
    if !back.ok() {
        return Err(format!(
            "git did not put {label} back: {}",
            first_line(&back.err)
        ));
    }
    Note::remove(&here.notes);
    let _ = NewBase::write(here, None);
    say(Say::Done(
        format!("{label} is back where it was before the merge").into(),
    ));
    Ok(0)
}

/// The paths where what is staged is not what the commit has, which an abort writes back.
fn staged_apart_from_head(here: &Here) -> Result<Vec<String>, String> {
    let read = git::run(
        &here.folder,
        &["diff-index", "--cached", "--name-only", "-z", "HEAD", "--"],
        git::READ,
    )
    .map_err(|e| e.to_string())?;
    if !read.ok() {
        return Err(format!(
            "git could not say what an abort of the merge in {} would write, so it was not run: {}",
            here.label,
            first_line(&read.err).trim_end_matches('.')
        ));
    }
    Ok(read
        .out
        .split('\0')
        .filter(|path| !path.is_empty())
        .map(str::to_owned)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::is_baseline;

    #[test]
    fn a_baseline_is_a_png_with_a_screenshots_folder_above_it() {
        for yes in [
            "app/src/__screenshots__/Card.stories.tsx/card-dark-webkit-linux.png",
            "__screenshots__/a.png",
        ] {
            assert!(is_baseline(yes), "{yes}");
        }
        for no in [
            "app/src/assets/logo.png",
            "app/src/__screenshots__/notes.bin",
            "app/src/__screenshots__.png",
            "app/not__screenshots__/a.png",
            "__screenshots__",
            "app/src/__screenshots__/a.png.txt",
            "",
        ] {
            assert!(!is_baseline(no), "{no}");
        }
    }
}
