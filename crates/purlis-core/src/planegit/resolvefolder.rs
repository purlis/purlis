// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A save's conflict, settled in a resolve folder off to the side** (#5824 F7, D2).
//!
//! Resolve used to leave a real merge stopped at the plane root, so chats wrote into a stopped
//! tree and a quit left the root mid-merge. Now nothing multi-step ever runs at the root:
//!
//! - [`cut`] makes the **resolve folder**, a linked worktree of the plane at the state folder's
//!   `resolve/` ([`folder`]), on the temporary branch [`BRANCH`]. Its files are the merge of the
//!   plane's commits and the remote's target as `git merge-tree` writes it, what purlis
//!   generates already rebuilt ([`super::bringin::plan`]); each file a person wrote that
//!   conflicts holds git's markers, and the folder's index holds its three sides as unmerged
//!   entries, so `git ls-files -u`, `git checkout --ours` and `--theirs` work there as in any
//!   stopped merge. What purlis holds about it is [`RECORD`], in the plane's git directory.
//! - The person, or a Fix chat started from the window, settles the marked files **in the
//!   folder**. Chats go on writing at the root meanwhile.
//! - [`carry_on`] (*Continue*) checks that no file holds a marker and nothing is unmerged (the
//!   #433 check), stages the settled files from the bytes it checked, commits the merge
//!   (`commit-tree`, signed as the plane's saves are), and moves the root onto it with one
//!   `reset --keep`, which refuses rather than overwrite a file a chat changed, and says which.
//!   Where the root was saved since the folder was cut, those saves are replayed onto the merge
//!   off to the side first ([`super::replay`]). The folder goes.
//! - [`abort`] removes the folder and changes nothing at the root. [`skip`] keeps its meaning:
//!   where what is settled brings nothing the remote lacks, it drops this machine's side and
//!   moves the root onto the remote's.
//!
//! **A quit at any point leaves the root untouched**: before the record is written there is no
//! resolve, and a folder without one is removed by the next [`cut`]; after it, the folder is
//! found again by the next look ([`resolving`]).
//!
//! The window runs every git command here, under no chat's sandbox, each one the person's press.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{
    Claim, WRITE, bringin, forget_replayed_push, git, git_dir_of, is_object_name, line_of, ondisk,
    replay, sandbox_would_stop, tail,
};
use crate::repocmd::{Say, Sink};

/// One side of a conflicted path, as git's index holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Stage {
    /// 1 the merge base, 2 ours, 3 theirs.
    number: u8,
    mode: String,
    object: String,
}

/// Each conflicted path's sides.
type Sides = BTreeMap<String, Vec<Stage>>;

/// How a press of Resolve, Continue, Skip or Abort ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Settled {
    /// This machine's commits and the remote's are one line now. What is left is the push
    /// ([`super::resolve::push_after`]).
    Replayed,
    /// These files conflict, and a resolve folder holds them for the person or a chat.
    Stopped(Vec<String>),
    /// What is settled brings nothing the remote lacks: Skip drops this machine's side, as a
    /// replay drops a commit left empty.
    Empty,
    /// The resolve folder, or a stop an older build left at the root, is gone, and nothing else
    /// changed.
    Aborted,
    /// Nothing was changed, and why, as a sentence for the person.
    Refused(String),
}

/// The files of a conflict, by who may settle them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Conflicts {
    /// Files git marked the conflict in, still unsettled: a person's or a chat's to edit.
    pub marked: Vec<String>,
    /// Files git could not mark the conflict in, still unsettled: a binary file, a file one
    /// side deleted, a link or a submodule. The person settles each in a terminal, by choosing
    /// a side and staging it.
    pub by_hand: Vec<String>,
    /// The settings file, where both sides changed keys each their own way (#5852): the person
    /// chooses each key's value in the window, and no marker is ever written into it.
    pub keyed: Vec<String>,
}

/// Whether `text` has a line that starts with one of git's conflict markers: the opener, the
/// closer, or the common ancestor's (`diff3`, `zdiff3`). The separator alone is not one, since
/// a Markdown heading is underlined the same way.
pub fn markers_in(text: &[u8]) -> bool {
    text.split(|b| *b == b'\n').any(|line| {
        line.starts_with(b"<<<<<<<") || line.starts_with(b">>>>>>>") || line.starts_with(b"|||||||")
    })
}

/// The file in the plane's git directory that holds what the window knows of an open resolve.
pub const RECORD: &str = "purlis-resolve.json";

/// The temporary branch the resolve folder is on.
pub const BRANCH: &str = "purlis-resolve";

/// The folder's name in the state folder.
const FOLDER: &str = "resolve";

/// The most of a file read for conflict markers.
const MOST_READ: u64 = 16 * 1024 * 1024;

/// What a press says while a save or a fetch holds the project.
pub(super) const BUSY: &str =
    "A save of this project is running. Wait for it to finish, then try again.";

/// What a press says where no resolve is open.
const NONE_OPEN: &str =
    "No resolve is open in this project, so there is nothing to carry on. Press Resolve again.";

/// **Where the resolve folder of the plane at `root` is**: `resolve/` in its state folder, never
/// under `$PURLIS_HOME`, since it is a checkout of this plane.
pub fn folder(root: &Path) -> PathBuf {
    crate::names::state(root).join(FOLDER)
}

/// What the window holds about an open resolve ([`RECORD`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resolving {
    /// The branch the plane saves into.
    pub target: String,
    /// The plane's HEAD when Resolve was pressed.
    pub ours: String,
    /// The remote's target, merged in.
    pub theirs: String,
    /// The tree the folder was cut with, markers included.
    pub tree: String,
    /// Every path left in conflict, sorted.
    pub conflicted: Vec<String>,
    /// Of those, the ones git could not mark.
    pub by_hand: Vec<String>,
    /// The settings file's keys both sides changed, each its own way (#5852): the person's to
    /// choose in the window. Empty where none.
    #[serde(default)]
    pub keys: Vec<ResolvingKey>,
    /// When it was cut, in seconds since the epoch.
    pub at: u64,
}

/// A settings key both sides changed, each its own way, with both values as TOML writes them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvingKey {
    /// The settings file it is in.
    pub file: String,
    /// The key's dotted path.
    pub key: String,
    /// This machine's value; `None` where it removed the key.
    pub ours: Option<String>,
    /// The remote's value; `None` where it removed the key.
    pub theirs: Option<String>,
}

/// **The person's choice for one settings key in conflict** (#5852): the remote's value, or this
/// machine's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chosen {
    pub file: String,
    pub key: String,
    pub side: Side,
}

/// Which side of a conflict a choice keeps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// This machine's.
    Ours,
    /// The remote's.
    Theirs,
}

impl Resolving {
    /// Whether every value is in the shape this module writes: each ends up in git's argv.
    fn holds(&self) -> bool {
        let path_ok = |path: &String| {
            !path.is_empty()
                && !path.starts_with('/')
                && !path.split('/').any(|part| part.is_empty() || part == "..")
        };
        crate::planesave::branch_ok(&self.target)
            && [&self.ours, &self.theirs, &self.tree]
                .iter()
                .all(|id| is_object_name(id))
            && self.conflicted.iter().all(path_ok)
            && self
                .by_hand
                .iter()
                .all(|path| self.conflicted.contains(path))
            && self
                .keys
                .iter()
                .all(|key| self.conflicted.contains(&key.file))
    }

    /// Whether `path` is a settings file whose keys the person chooses.
    fn keyed(&self, path: &str) -> bool {
        self.keys.iter().any(|key| key.file == path)
    }
}

/// **The resolve open in the plane at `root`**, or `None`: its record, while the folder is there.
pub fn resolving(root: &Path) -> Option<Resolving> {
    match open(root) {
        Open::Resolving(resolving) => Some(resolving),
        Open::None | Open::NotOurs(_) => None,
    }
}

/// What the plane at `root` has of a resolve.
enum Open {
    /// No resolve, or a record with no folder.
    None,
    /// A resolve, its folder the one cut for it.
    Resolving(Resolving),
    /// A record and a folder whose gitfile is not the one cut for it, and why: nothing runs git
    /// in it.
    NotOurs(String),
}

/// The resolve open at `root`, checked: its record, and its folder's gitfile the one cut for it.
fn open(root: &Path) -> Open {
    let Some(resolving) = std::fs::read_to_string(git_dir_of(root).join(RECORD))
        .ok()
        .and_then(|text| serde_json::from_str::<Resolving>(&text).ok())
        .filter(Resolving::holds)
    else {
        return Open::None;
    };
    if std::fs::symlink_metadata(folder(root).join(".git")).is_err() {
        return Open::None;
    }
    match folder_is_ours(root) {
        Ok(()) => Open::Resolving(resolving),
        Err(why) => Open::NotOurs(why),
    }
}

/// What a press says of a resolve folder whose gitfile is not the one cut for it.
const NOT_OURS: &str = "The resolve folder's link to this project's git is not the one purlis \
     made, so purlis runs nothing in it. Abort removes the folder, and nothing here changes.";

/// **Whether the resolve folder is the one cut for this plane**, or why not: its `.git` is a
/// plain file naming this plane's own `worktrees/<entry>` in its common git directory, and that
/// entry's `gitdir` names the folder's `.git` back. Asked before git runs in the folder, which a
/// chat can write: git follows that file to whatever it names.
fn folder_is_ours(root: &Path) -> Result<(), String> {
    use std::io::Read;
    let not_ours = || NOT_OURS.to_owned();
    let gitfile = folder(root).join(".git");
    if !std::fs::symlink_metadata(&gitfile).is_ok_and(|meta| meta.is_file()) {
        return Err(not_ours());
    }
    let mut text = String::new();
    std::fs::File::open(&gitfile)
        .and_then(|file| file.take(4096).read_to_string(&mut text))
        .map_err(|_| not_ours())?;
    let named = text
        .trim_end_matches(['\n', '\r'])
        .strip_prefix("gitdir: ")
        .ok_or_else(not_ours)?;
    let named = folder(root).join(named);
    let real = |path: &Path| std::fs::canonicalize(path).map_err(|_| not_ours());
    let entry = real(&named)?;
    let common = line_of(
        root,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )
    .ok_or_else(not_ours)?;
    let worktrees = real(&Path::new(&common).join("worktrees"))?;
    if entry.parent() != Some(worktrees.as_path()) || !entry.is_dir() {
        return Err(not_ours());
    }
    let mut back = String::new();
    std::fs::File::open(entry.join("gitdir"))
        .and_then(|file| file.take(4096).read_to_string(&mut back))
        .map_err(|_| not_ours())?;
    let back = Path::new(back.trim_end_matches(['\n', '\r']));
    if real(back)? != real(&gitfile)? {
        return Err(not_ours());
    }
    Ok(())
}

/// The files of the open resolve, by who may settle them, as they stand in the folder now.
pub fn conflicts(root: &Path) -> Option<Conflicts> {
    match open(root) {
        Open::Resolving(resolving) => Some(sorted(&folder(root), &resolving)),
        // Still open, so the window offers Abort; nothing is read from the folder.
        Open::NotOurs(_) => Some(Conflicts::default()),
        Open::None => None,
    }
}

/// **Cut the resolve folder** for a merge of HEAD and the remote-tracking `target` whose files a
/// person wrote conflict. Answers the files in conflict. On any failure nothing is left: no
/// folder, branch or record. The root is never touched.
pub(super) fn cut(root: &Path, target: &str) -> Result<Vec<String>, String> {
    clear(root);
    let theirs_ref = format!("refs/remotes/origin/{target}");
    let ours = line_of(root, &["rev-parse", "--verify", "-q", "HEAD^{commit}"])
        .filter(|id| is_object_name(id))
        .ok_or("the project has no commit")?;
    let theirs = line_of(
        root,
        &[
            "rev-parse",
            "--verify",
            "-q",
            &format!("{theirs_ref}^{{commit}}"),
        ],
    )
    .filter(|id| is_object_name(id))
    .ok_or_else(|| format!("{target} has not been fetched"))?;
    let plan = bringin::plan_between(root, &ours, &theirs, None)?;
    if plan.person.is_empty() {
        return Err("nothing a person wrote is in conflict".into());
    }
    let tree = bringin::tree_of(root, &plan)?;
    let stages = unmerged_sides(root, &ours, &theirs)?;
    let folder = folder(root);
    let made = (|| {
        std::fs::create_dir_all(folder.parent().unwrap_or(root)).map_err(|e| e.to_string())?;
        let folder_arg = folder.to_string_lossy();
        succeeded(git::run_untimed(
            root,
            &[
                "worktree",
                "add",
                "-q",
                "--no-checkout",
                "-B",
                BRANCH,
                &folder_arg,
                &ours,
            ],
        ))?;
        // Untimed: it checks out a tree.
        succeeded(git::run_untimed(
            &folder,
            &["read-tree", "--reset", "-u", &tree],
        ))?;
        // What a person wrote, with purlis's part already settled and markers around theirs.
        for (path, text) in &plan.marked {
            written_in(&folder, path, text)?;
        }
        // A settings file whose keys conflict is written as the structured merge leaves it,
        // this machine's value standing in for each such key: no marker, ever (#5852).
        let mut keys = Vec::new();
        for file in plan.keys.keys() {
            let around = settings_around(root, &ours, &theirs, file)?;
            written_in(&folder, file, &around.text)?;
            keys.extend(around.keys.into_iter().map(|key| ResolvingKey {
                file: file.clone(),
                key: key.key,
                ours: key.ours,
                theirs: key.theirs,
            }));
        }
        let mut info = String::new();
        let zero = "0".repeat(ours.len());
        for path in &plan.person {
            let Some(sides) = stages.get(path) else {
                continue;
            };
            info.push_str(&format!("0 {zero}\t{path}\0"));
            for Stage {
                number,
                mode,
                object,
            } in sides
            {
                info.push_str(&format!("{mode} {object} {number}\t{path}\0"));
            }
        }
        let indexed = git::run_with_input(
            &folder,
            &["update-index", "-z", "--index-info"],
            info.into_bytes(),
            WRITE,
        )
        .map_err(|unavailable| unavailable.to_string())?;
        if indexed.code != Some(0) {
            return Err(String::from_utf8_lossy(&indexed.err).trim().to_owned());
        }
        // A file git could not mark, or one its attributes merged with no marker in it (a
        // `-merge` or `binary` rule leaves one side's copy), is settled by hand.
        let by_hand: Vec<String> = plan
            .person
            .iter()
            .filter(|path| {
                !plan.keys.contains_key(*path)
                    && (stages
                        .get(*path)
                        .is_none_or(|sides| marks_nothing(&folder, path, sides))
                        || !std::fs::read(folder.join(path)).is_ok_and(|bytes| markers_in(&bytes)))
            })
            .cloned()
            .collect();
        let resolving = Resolving {
            target: target.to_owned(),
            ours: ours.clone(),
            theirs: theirs.clone(),
            tree: tree.clone(),
            conflicted: plan.person.clone(),
            by_hand,
            keys,
            at: super::now() as u64,
        };
        write_record(root, &resolving)
    })();
    match made {
        Ok(()) => Ok(plan.person),
        Err(why) => {
            clear(root);
            Err(why)
        }
    }
}

/// **Resolve**: fetch the target and bring it in, as far as it goes by itself, and where what a
/// person wrote conflicts, cut the resolve folder for it. For the person's own press in the
/// window, and nothing else. A resolve already open is found again, not cut afresh.
pub fn start(root: &Path, say: Sink) -> Settled {
    let branch = match target_of(root) {
        Ok(branch) => branch,
        Err(refused) => return refused,
    };
    if let Some(resolving) = resolving(root) {
        return Settled::Stopped(resolving.conflicted);
    }
    say(Say::Info(
        format!("Fetching `{branch}` from the remote …").into(),
    ));
    {
        let Some(_claim) = Claim::of(root) else {
            return Settled::Refused(BUSY.into());
        };
        if let Err(why) = super::fetch_branch(root, &branch) {
            return Settled::Refused(format!(
                "Nothing was started: the remote could not be fetched: {}.",
                why.trim_end_matches('.')
            ));
        }
    }
    start_fetched(root, say)
}

/// [`start`], once the target is fetched into its remote-tracking branch.
pub(super) fn start_fetched(root: &Path, say: Sink) -> Settled {
    let Some(_claim) = Claim::of(root) else {
        return Settled::Refused(BUSY.into());
    };
    let branch = match target_of(root) {
        Ok(branch) => branch,
        Err(refused) => return refused,
    };
    if let Some(resolving) = resolving(root) {
        return Settled::Stopped(resolving.conflicted);
    }
    let theirs = format!("refs/remotes/origin/{branch}");
    if super::count(root, &format!("HEAD..{theirs}")) == Some(0) {
        forget_replayed_push(root);
        return Settled::Replayed;
    }
    let plane = crate::planesave::Settings::read(root).plane;
    let requests = matches!(
        plane.mode.value,
        Some(crate::planesave::Mode::Pr | crate::planesave::Mode::PrMerge)
    );
    if requests {
        say(Say::Info(format!("Merging `{branch}` in …").into()));
        // The person asked for it: brought in whenever it moved, needed or not.
        match bringin::bring_in(root, &branch, plane.sign.value, bringin::When::Moved) {
            bringin::BroughtIn::Nothing | bringin::BroughtIn::Moved(_) => {
                forget_replayed_push(root);
                return Settled::Replayed;
            }
            bringin::BroughtIn::Waiting(why) => {
                return Settled::Refused(format!("Nothing was started: {why}."));
            }
            bringin::BroughtIn::Failed(why) => {
                return Settled::Refused(format!(
                    "Nothing was started: git could not merge them: {}.",
                    crate::personas::one_line(&why)
                ));
            }
            bringin::BroughtIn::Conflict(_) => {}
        }
    } else {
        say(Say::Info("Replaying this project's own commits …".into()));
        match replay::replay(root, "HEAD", &theirs, plane.sign.value, WRITE) {
            Ok(tip) => {
                return match move_root(root, &tip) {
                    Ok(()) => {
                        forget_replayed_push(root);
                        super::touch(root);
                        Settled::Replayed
                    }
                    Err(why) => Settled::Refused(why),
                };
            }
            // What a person wrote conflicts: the folder is cut for it, below.
            Err(replay::Unreplayed::Conflict(_)) => {}
            Err(stopped) => return Settled::Refused(said_unreplayed(stopped, When::Starting)),
        }
    }
    say(Say::Info(
        "Cutting a resolve folder for the conflict …".into(),
    ));
    match cut(root, &branch) {
        Ok(files) => {
            super::touch(root);
            Settled::Stopped(files)
        }
        Err(why) => Settled::Refused(format!(
            "Nothing was started: the resolve folder could not be made: {}.",
            crate::personas::one_line(&why)
        )),
    }
}

/// The branch the plane saves into, which it must be on, or the refusal.
fn target_of(root: &Path) -> Result<String, Settled> {
    if let Err(why) = super::git_files_refused(root) {
        return Err(Settled::Refused(format!("Nothing was started: {why}.")));
    }
    if let Some(stopped) = crate::gitstate::stopped(root) {
        return Err(Settled::Refused(format!(
            "Nothing was started: {}. Continue or Abort it first.",
            stopped.why()
        )));
    }
    let here = line_of(root, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_default();
    if here.is_empty() || here == "HEAD" {
        return Err(Settled::Refused(
            "Nothing was started: the project is on no branch.".into(),
        ));
    }
    let plane = crate::planesave::Settings::read(root).plane;
    let branch = plane.branch.value.clone().unwrap_or_else(|| here.clone());
    if here != branch {
        return Err(Settled::Refused(format!(
            "Nothing was started: the project is on `{here}`, and it saves into `{branch}`."
        )));
    }
    Ok(branch)
}

/// **Continue**: commit what is settled in the folder and move the root onto it.
///
/// `shown` are the files the window showed the person; each must be one in conflict. Refused,
/// with nothing moved, while a file git could not mark is unsettled, a marked file still holds a
/// marker or is not a plain file, anything unmerged is left, or anything outside the files in
/// conflict differs from how the folder was cut.
pub fn carry_on(root: &Path, shown: &[String], keys: &[Chosen], say: Sink) -> Settled {
    let Some(_claim) = Claim::of(root) else {
        return Settled::Refused(BUSY.into());
    };
    if let Err(why) = super::git_files_refused(root) {
        return Settled::Refused(format!("Nothing was carried on: {why}."));
    }
    let resolving = match open(root) {
        Open::Resolving(resolving) => resolving,
        Open::NotOurs(why) => return Settled::Refused(why),
        Open::None => return Settled::Refused(NONE_OPEN.into()),
    };
    let folder = folder(root);
    let outside: Vec<&String> = shown
        .iter()
        .filter(|path| !resolving.conflicted.contains(*path))
        .collect();
    if !outside.is_empty() {
        return Settled::Refused(format!(
            "purlis stages only the files in conflict, and {} {} not one. Nothing was staged.",
            join(&outside),
            if outside.len() == 1 { "is" } else { "are" },
        ));
    }
    let tree = match settled_tree(root, &folder, &resolving, keys) {
        Ok(tree) => tree,
        Err(refused) => return refused,
    };
    // Settled as the remote has it: nothing of this machine's is left, and Skip is the way on.
    if line_of(
        root,
        &["rev-parse", &format!("{}^{{tree}}", resolving.theirs)],
    )
    .as_deref()
        == Some(tree.as_str())
    {
        return Settled::Empty;
    }
    say(Say::Info("Committing the merge …".into()));
    let plane = crate::planesave::Settings::read(root).plane;
    let subject = format!("Bring {} into this machine's saves", resolving.target);
    let merge = match bringin::commit_tree(
        root,
        &tree,
        &[&resolving.ours, &resolving.theirs],
        &subject,
        plane.sign.value,
    ) {
        Ok(merge) => merge,
        Err(why) => {
            return Settled::Refused(format!(
                "The merge could not be committed: {}. Nothing moved; the resolve folder is kept.",
                crate::personas::one_line(&why)
            ));
        }
    };
    match moved_onto(root, &resolving, &merge, plane.sign.value) {
        Ok(()) => {
            clear(root);
            forget_replayed_push(root);
            super::touch(root);
            Settled::Replayed
        }
        Err(why) => Settled::Refused(why),
    }
}

/// **Skip**: where what is settled brings nothing the remote lacks, drop this machine's side and
/// move the root onto the remote's, as a rebase drops a commit left empty.
pub fn skip(root: &Path) -> Settled {
    let Some(_claim) = Claim::of(root) else {
        return Settled::Refused(BUSY.into());
    };
    let resolving = match open(root) {
        Open::Resolving(resolving) => resolving,
        Open::NotOurs(why) => return Settled::Refused(why),
        Open::None => return Settled::Refused(NONE_OPEN.into()),
    };
    let folder = folder(root);
    // Skip keeps this machine's value for no key: each stands as the remote's.
    let keys: Vec<Chosen> = resolving
        .keys
        .iter()
        .map(|key| Chosen {
            file: key.file.clone(),
            key: key.key.clone(),
            side: Side::Theirs,
        })
        .collect();
    let tree = match settled_tree(root, &folder, &resolving, &keys) {
        Ok(tree) => tree,
        Err(refused) => return refused,
    };
    let theirs_tree = line_of(
        root,
        &["rev-parse", &format!("{}^{{tree}}", resolving.theirs)],
    );
    if theirs_tree.as_deref() != Some(tree.as_str()) {
        return Settled::Refused(
            "Skip drops only a side that has nothing left to bring, and what is settled still \
             changes files. Continue it, or Abort."
                .into(),
        );
    }
    let plane = crate::planesave::Settings::read(root).plane;
    match moved_onto(root, &resolving, &resolving.theirs, plane.sign.value) {
        Ok(()) => {
            clear(root);
            forget_replayed_push(root);
            super::touch(root);
            Settled::Replayed
        }
        Err(why) => Settled::Refused(why),
    }
}

/// **Abort**: remove the resolve folder. Nothing at the root changes.
pub fn abort(root: &Path) -> Settled {
    let Some(_claim) = Claim::of(root) else {
        return Settled::Refused(BUSY.into());
    };
    if resolving(root).is_none() && !folder(root).exists() {
        return Settled::Refused(
            "No resolve is open in this project, so there is nothing to abort.".into(),
        );
    }
    clear(root);
    super::touch(root);
    Settled::Aborted
}

/// The two sides of the open resolve, by their commit subjects: the remote's, then this
/// machine's.
pub fn sides(root: &Path) -> Option<(String, String)> {
    let resolving = resolving(root)?;
    let subject = |id: &str| {
        git::run(root, &["log", "-1", "--format=%s", id, "--"], git::READ)
            .ok()
            .filter(git::Run::ok)
            .map(|run| run.line().trim().to_string())
    };
    Some((subject(&resolving.theirs)?, subject(&resolving.ours)?))
}

/// The tree the folder comes to once every file in conflict is settled and staged, or the
/// refusal that leaves everything as it was.
fn settled_tree(
    root: &Path,
    folder: &Path,
    resolving: &Resolving,
    chosen: &[Chosen],
) -> Result<String, Settled> {
    let left = sorted(folder, resolving);
    if !left.by_hand.is_empty() {
        return Err(Settled::Refused(said_by_hand(folder, &left.by_hand)));
    }
    // Every key in conflict is chosen before anything is staged.
    let unchosen: Vec<String> = resolving
        .keys
        .iter()
        .filter(|key| left.keyed.contains(&key.file))
        .filter(|key| {
            !chosen
                .iter()
                .any(|c| c.file == key.file && c.key == key.key)
        })
        .map(|key| format!("`{}` in `{}`", key.key, key.file))
        .collect();
    if !unchosen.is_empty() {
        return Err(Settled::Refused(format!(
            "Choose this machine's value or the remote's for {}, then Continue.",
            unchosen.join(", ")
        )));
    }
    // Settled already, and staged: by the person in a terminal there.
    for path in &resolving.conflicted {
        if left.marked.contains(path) || resolving.by_hand.contains(path) || resolving.keyed(path) {
            continue;
        }
        if staged_text(folder, path).is_some_and(|text| markers_in(&text)) {
            return Err(Settled::Refused(said_markers(path, "is staged with")));
        }
    }
    // Only the files in conflict change: everything else is as the folder was cut.
    let changed = paths_listed(
        folder,
        &[
            "diff-index",
            "--cached",
            "--no-renames",
            "--name-only",
            "-z",
            &resolving.tree,
            "--",
        ],
    );
    let elsewhere: Vec<&String> = changed
        .iter()
        .filter(|path| !resolving.conflicted.contains(*path))
        .collect();
    if !elsewhere.is_empty() {
        return Err(Settled::Refused(format!(
            "{} {} not as the resolve folder was cut, and purlis carries on only the files in \
             conflict. Put {} back, or Abort and Resolve again.",
            join(&elsewhere),
            if elsewhere.len() == 1 { "is" } else { "are" },
            if elsewhere.len() == 1 { "it" } else { "them" },
        )));
    }
    let mut rows = Vec::new();
    for path in &left.marked {
        let (file, runs, bytes) = read_plain(folder, path).map_err(Settled::Refused)?;
        if markers_in(&bytes) {
            return Err(Settled::Refused(said_markers(path, "still has")));
        }
        rows.push(ondisk::Row {
            path: path.clone(),
            from: ondisk::Source::File {
                file,
                runs,
                checked: Some(bytes),
            },
        });
    }
    let named: Vec<String> = left.marked.clone();
    if !named.is_empty() {
        staged_checked(folder, rows, &named)?;
    }
    // Each settings file in conflict, written from the three sides and the person's choices,
    // never from what is on disk in the folder, which a chat can write (#5852).
    for file in &left.keyed {
        let theirs_for: Vec<String> = chosen
            .iter()
            .filter(|c| &c.file == file && c.side == Side::Theirs)
            .map(|c| c.key.clone())
            .collect();
        let text = settings_chosen(root, &resolving.ours, &resolving.theirs, file, &theirs_for)
            .map_err(|why| Settled::Refused(format!("`{file}` could not be settled: {why}.")))?;
        staged_as(folder, file, &text).map_err(|why| {
            Settled::Refused(format!(
                "`{file}` could not be staged: {}",
                crate::personas::one_line(&why)
            ))
        })?;
    }
    // Nothing is left unmerged (#433), whatever the window last drew.
    let unmerged = unmerged(folder);
    if !unmerged.is_empty() {
        let named: Vec<&String> = unmerged.keys().collect();
        return Err(Settled::Refused(format!(
            "{} {} still unmerged in the resolve folder. Settle {}, then Continue.",
            join(&named),
            if named.len() == 1 { "is" } else { "are" },
            if named.len() == 1 { "it" } else { "them" },
        )));
    }
    let tree = written_tree(folder)?;
    // The memory indexes, written again from the tree the person settled (#5824 F4), so no line
    // names a memory they deleted, and one they kept is listed.
    let indexes = bringin::indexes_settled_between(root, &tree, &resolving.ours, &resolving.theirs)
        .map_err(|why| {
            Settled::Refused(format!(
                "Nothing was carried on: purlis could not write a memory index: {}.",
                crate::personas::one_line(&why)
            ))
        })?;
    if indexes.is_empty() {
        return Ok(tree);
    }
    for (index, text) in &indexes {
        staged_as(folder, index, text).map_err(|why| {
            Settled::Refused(format!(
                "Nothing was carried on: purlis could not write {index}: {}.",
                crate::personas::one_line(&why)
            ))
        })?;
    }
    written_tree(folder)
}

/// **The settled files staged as the bytes that were checked**, as a save stages ([`ondisk`]):
/// each from the descriptor it was read through, asked again just before that it has one name.
/// Once staged, a file that is not the file on disk now (written while purlis was checking it,
/// or given a second name) is put back in conflict and its blob taken out of the object store,
/// so a chat's late write is never left behind in a folder Continue then removes.
fn staged_checked(folder: &Path, rows: Vec<ondisk::Row>, named: &[String]) -> Result<(), Settled> {
    #[cfg(test)]
    super::tests::before_the_add(folder);
    let held_back = ondisk::stage_rows(folder, rows).map_err(|said| {
        Settled::Refused(format!(
            "git would not stage them: {}",
            crate::personas::one_line(&said)
        ))
    })?;
    if let Some(refusal) = ondisk::refused_unstaged(&held_back, ondisk::PutBack::Conflicted(named))
    {
        return Err(not_the_file(&refusal));
    }
    if let Err(said) = ondisk::refresh(folder, false)
        && let Some(refusal) =
            ondisk::refused_into(folder, &Err(said), ondisk::PutBack::Conflicted(named))
    {
        return Err(not_the_file(&refusal));
    }
    let found: Result<Vec<ondisk::Held>, String> = ondisk::not_as_staged(folder).map(|held| {
        held.into_iter()
            .filter(|one| named.contains(&one.path))
            .collect()
    });
    if let Ok(held) = &found
        && !held.is_empty()
        && held.iter().all(|one| one.why == ondisk::Why::Changed)
    {
        let moved: Vec<&String> = named
            .iter()
            .filter(|path| held.iter().any(|one| one.path == **path))
            .collect();
        let all: Vec<ondisk::Held> = named
            .iter()
            .map(|path| ondisk::Held {
                path: path.clone(),
                why: ondisk::Why::Changed,
            })
            .collect();
        let _ = ondisk::refused_into(folder, &Ok(all), ondisk::PutBack::Conflicted(named));
        return Err(Settled::Refused(format!(
            "{} changed while purlis was checking {}, so nothing was carried on. Continue again \
             once it is settled.",
            join(&moved),
            if moved.len() == 1 { "it" } else { "them" },
        )));
    }
    if let Some(refusal) = ondisk::refused_into(folder, &found, ondisk::PutBack::Conflicted(named))
    {
        return Err(not_the_file(&refusal));
    }
    Ok(())
}

/// What Continue refused by [`ondisk`] says: a settled file is not the one file on disk.
fn not_the_file(refusal: &ondisk::Refusal) -> Settled {
    Settled::Refused(format!(
        "A settled file is not the one file on disk: {} Settle it again as one file, then \
         Continue.",
        refusal.sentence()
    ))
}

/// `text` written at `path` in the folder, never through a link a side of the merge committed on
/// the way to it, the file's mode kept. `Err` names the file.
fn written_in(folder: &Path, path: &str, text: &str) -> Result<(), String> {
    crate::contain::no_link_on_the_way(folder, &folder.join(path))
        .and_then(|()| {
            crate::rewrite::replace(
                folder,
                &folder.join(path),
                text.as_bytes(),
                crate::rewrite::Mode::Kept,
            )
        })
        .map_err(|why| format!("{path}: {why}"))
}

/// The tree the folder's index holds now.
fn written_tree(folder: &Path) -> Result<String, Settled> {
    git::run(folder, &["write-tree"], WRITE)
        .ok()
        .filter(git::Run::ok)
        .map(|run| run.line().trim().to_owned())
        .filter(|tree| is_object_name(tree))
        .ok_or_else(|| Settled::Refused("git could not write what is settled.".into()))
}

/// `text` written at `path` in the folder and staged as those bytes, whatever the file held and
/// whatever sides the index had for it.
fn staged_as(folder: &Path, path: &str, text: &str) -> Result<(), String> {
    written_in(folder, path, text)?;
    let hashed = git::run_with_input(
        folder,
        &["hash-object", "-w", "--no-filters", "--stdin"],
        text.as_bytes().to_vec(),
        WRITE,
    )
    .map_err(|unavailable| unavailable.to_string())?;
    let object = String::from_utf8_lossy(&hashed.out).trim().to_owned();
    if hashed.code != Some(0) || !is_object_name(&object) {
        return Err(String::from_utf8_lossy(&hashed.err).trim().to_owned());
    }
    let zero = "0".repeat(object.len());
    let info = format!("0 {zero}\t{path}\0100644 {object} 0\t{path}\0");
    let indexed = git::run_with_input(
        folder,
        &["update-index", "-z", "--index-info"],
        info.into_bytes(),
        WRITE,
    )
    .map_err(|unavailable| unavailable.to_string())?;
    if indexed.code != Some(0) {
        return Err(String::from_utf8_lossy(&indexed.err).trim().to_owned());
    }
    Ok(())
}

/// The three sides of the settings file at `file` for a merge of `ours` and `theirs`: from where
/// they parted, this machine's and the remote's.
fn settings_sides(
    root: &Path,
    ours: &str,
    theirs: &str,
    file: &str,
) -> Result<(String, String, String), String> {
    let base = line_of(root, &["merge-base", ours, theirs])
        .ok_or("the two histories have no commit in common")?;
    let blob = |rev: &str| {
        git::run(
            root,
            &["cat-file", "blob", &format!("{rev}:{file}")],
            git::READ,
        )
        .ok()
        .filter(git::Run::ok)
        .map(|run| run.out)
        .ok_or_else(|| format!("{file} is not in {rev}"))
    };
    Ok((blob(&base)?, blob(ours)?, blob(theirs)?))
}

/// The settings file merged around its keys in conflict ([`crate::tomlmerge::around`]).
fn settings_around(
    root: &Path,
    ours: &str,
    theirs: &str,
    file: &str,
) -> Result<crate::tomlmerge::Around, String> {
    let (base, mine, remote) = settings_sides(root, ours, theirs, file)?;
    crate::tomlmerge::around(&base, &mine, &remote)
}

/// The settings file once the person chose ([`crate::tomlmerge::chosen`]).
fn settings_chosen(
    root: &Path,
    ours: &str,
    theirs: &str,
    file: &str,
    theirs_for: &[String],
) -> Result<String, String> {
    let (base, mine, remote) = settings_sides(root, ours, theirs, file)?;
    crate::tomlmerge::chosen(&base, &mine, &remote, theirs_for)
}

/// Move the root onto `onto` with one `reset --keep`: first replaying onto it, off to the side,
/// what was saved at the root since the folder was cut. `Err` is the sentence for the person;
/// nothing moved, and the folder is kept.
fn moved_onto(root: &Path, resolving: &Resolving, onto: &str, sign: bool) -> Result<(), String> {
    let here = line_of(root, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_default();
    if here != resolving.target {
        return Err(format!(
            "The project is on `{here}` now, and the resolve was for `{}`. Nothing moved; the \
             resolve folder is kept.",
            resolving.target
        ));
    }
    let head = line_of(root, &["rev-parse", "--verify", "-q", "HEAD"]).unwrap_or_default();
    let onto = if head == resolving.ours {
        onto.to_owned()
    } else {
        replay::replay(root, "HEAD", onto, sign, WRITE)
            .map_err(|stopped| said_unreplayed(stopped, When::Moving))?
    };
    move_root(root, &onto)
}

/// When a replay ran, for what its stop is said as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum When {
    /// Resolve, replaying this machine's commits onto the fetched target.
    Starting,
    /// Continue, carrying what was saved since Resolve began onto the settled merge.
    Moving,
}

/// **The one mapping of a replay that stopped**, for the person (#5886): what happened, and that
/// nothing moved.
fn said_unreplayed(stopped: replay::Unreplayed, when: When) -> String {
    use replay::Unreplayed;
    match (when, stopped) {
        // Resolve cuts a resolve folder for a conflict before it says any stop: the
        // `Err(replay::Unreplayed::Conflict(_)) => {}` arm of [`start_fetched`] comes first.
        (When::Starting, Unreplayed::Conflict(_)) => {
            unreachable!("Resolve cuts a folder for a replay's conflict and never says it")
        }
        (When::Starting, Unreplayed::Unsigned(signer)) => {
            format!("Nothing was started: a replayed commit could not be signed: {signer}.")
        }
        (When::Starting, Unreplayed::OutOfTime) => "Nothing was started: the replay did not \
             finish in time. The project is exactly as it was."
            .into(),
        (When::Starting, Unreplayed::Failed(said)) => format!(
            "Nothing was started: git could not replay them: {}.",
            crate::personas::one_line(&said)
        ),
        (When::Moving, Unreplayed::Conflict(files)) => format!(
            "This machine saved since Resolve began, and that save changes {} the way the \
             settled merge does not. Nothing moved: Abort, then Resolve again.",
            files.join(", ")
        ),
        (When::Moving, Unreplayed::Unsigned(signer)) => format!(
            "A save made since Resolve began could not be signed again: {signer}. Nothing \
             moved; the resolve folder is kept."
        ),
        (When::Moving, Unreplayed::OutOfTime) => "Carrying this machine's newer saves onto the \
             merge did not finish in time. Nothing moved; the resolve folder is kept."
            .into(),
        (When::Moving, Unreplayed::Failed(said)) => format!(
            "This machine's newer saves could not be carried onto the merge: {}. Nothing \
             moved; the resolve folder is kept.",
            crate::personas::one_line(&said)
        ),
    }
}

/// **The one move of the root**: `reset --keep` onto `onto`, from the window only. `Err` is the
/// sentence for the person, naming any file a chat changed that the move would overwrite; nothing
/// moved.
fn move_root(root: &Path, onto: &str) -> Result<(), String> {
    let stops = sandbox_would_stop(root, onto);
    if !stops.is_empty() {
        return Err(format!(
            "Resolve moves the project from the window, not from a chat: {}.",
            super::said_sandbox_stops(&stops)
        ));
    }
    // Untimed: a reset checks out a tree, and a killed one leaves it half-written.
    match git::run_untimed(root, &["reset", "-q", "--keep", onto]) {
        Ok(run) if run.ok() => Ok(()),
        Ok(run) => {
            let in_the_way = in_the_way(&run.err);
            Err(if in_the_way.is_empty() {
                format!(
                    "The project did not move onto the merge: {}. Nothing moved; the resolve \
                     folder is kept.",
                    crate::personas::one_line(&tail(&run))
                )
            } else {
                let named: Vec<&String> = in_the_way.iter().collect();
                format!(
                    "{} changed here since Resolve began, and the merge changes {} too, so \
                     nothing moved. Save {}, or undo the change, then Continue again; the \
                     resolve folder is kept.",
                    join(&named),
                    if named.len() == 1 { "it" } else { "them" },
                    if named.len() == 1 { "it" } else { "them" },
                )
            })
        }
        Err(unavailable) => Err(format!(
            "The project did not move onto the merge: {unavailable}. The resolve folder is kept."
        )),
    }
}

/// The files `git reset --keep` said are in its way: changed and not committed, or untracked
/// where the merge writes a file.
///
/// Read by git's English words, which every call has: the runner gives git `LC_ALL=C` and no
/// other locale (`worktree::git`, #5886).
fn in_the_way(err: &str) -> Vec<String> {
    let mut out = BTreeSet::new();
    let mut listing = false;
    for line in err.lines() {
        if let Some(rest) = ["Entry '", "file '"]
            .iter()
            .find_map(|opener| line.split_once(opener).map(|(_, rest)| rest))
            && let Some((path, _)) = rest.split_once('\'')
        {
            out.insert(path.to_owned());
            continue;
        }
        if line.contains("would be overwritten") {
            listing = true;
            continue;
        }
        if listing {
            if let Some(path) = line.strip_prefix('\t') {
                out.insert(path.trim().to_owned());
            } else {
                listing = false;
            }
        }
    }
    out.into_iter().collect()
}

/// Remove whatever an open or half-cut resolve left: the folder, its branch and the record.
fn clear(root: &Path) {
    let folder = folder(root);
    // git runs on the folder only while its gitfile is the one cut for it.
    if folder.exists() && folder_is_ours(root).is_ok() {
        let folder_arg = folder.to_string_lossy();
        let _ = git::run_untimed(
            root,
            &["worktree", "remove", "--force", "--force", &folder_arg],
        );
    }
    // Whatever is left, removed by the file system: never through a link, and only a folder of
    // this plane's state folder.
    if std::fs::symlink_metadata(&folder).is_ok_and(|meta| meta.is_dir()) {
        let _ = std::fs::remove_dir_all(&folder);
    }
    let _ = git::run(root, &["worktree", "prune"], WRITE);
    let branch = format!("refs/heads/{BRANCH}");
    if line_of(root, &["rev-parse", "--verify", "-q", &branch]).is_some() {
        let _ = git::run(root, &["update-ref", "-d", &branch], WRITE);
    }
    let _ = std::fs::remove_file(git_dir_of(root).join(RECORD));
}

fn write_record(root: &Path, resolving: &Resolving) -> Result<(), String> {
    let path = git_dir_of(root).join(RECORD);
    let text = serde_json::to_string_pretty(resolving).map_err(|e| e.to_string())?;
    let dir = path.parent().unwrap_or(root);
    crate::rewrite::replace(dir, &path, text.as_bytes(), crate::rewrite::Mode::Private)
        .map_err(|why| why.to_string())
}

/// Each conflicted path's sides as `git merge-tree` left them: stage, mode and object.
fn unmerged_sides(root: &Path, ours: &str, theirs: &str) -> Result<Sides, String> {
    let run = git::run(
        root,
        &[
            "merge-tree",
            "--write-tree",
            "-z",
            "--no-messages",
            ours,
            theirs,
        ],
        WRITE,
    )
    .map_err(|unavailable| unavailable.to_string())?;
    if !matches!(run.code, Some(0 | 1)) {
        return Err(tail(&run));
    }
    Ok(stages_of(run.out.split('\0').skip(1)))
}

/// `<mode> <object> <stage>\t<path>` entries, by path.
fn stages_of<'a>(entries: impl Iterator<Item = &'a str>) -> Sides {
    let mut out: Sides = BTreeMap::new();
    for entry in entries.filter(|entry| !entry.is_empty()) {
        let Some((meta, path)) = entry.split_once('\t') else {
            continue;
        };
        if let [mode, object, stage] = meta.split(' ').collect::<Vec<_>>()[..]
            && let Ok(stage) = stage.parse::<u8>()
        {
            out.entry(path.to_owned()).or_default().push(Stage {
                number: stage,
                mode: mode.to_owned(),
                object: object.to_owned(),
            });
        }
    }
    out
}

/// The files git calls unmerged in `folder`, each with its sides.
fn unmerged(folder: &Path) -> Sides {
    git::run(folder, &["ls-files", "-u", "-z"], git::READ)
        .ok()
        .filter(git::Run::ok)
        .map(|run| stages_of(run.out.split('\0')))
        .unwrap_or_default()
}

/// The files still unmerged in the folder, by who may settle them.
fn sorted(folder: &Path, resolving: &Resolving) -> Conflicts {
    let mut out = Conflicts::default();
    for (path, sides) in unmerged(folder) {
        if resolving.keyed(&path) {
            out.keyed.push(path);
        } else if resolving.by_hand.contains(&path) || marks_nothing(folder, &path, &sides) {
            out.by_hand.push(path);
        } else {
            out.marked.push(path);
        }
    }
    out
}

/// Whether git could not mark the conflict in `path`: a side is missing, an entry is a link or a
/// submodule, or a side is binary as git judges it (a NUL in its first 8000 bytes).
fn marks_nothing(folder: &Path, path: &str, sides: &[Stage]) -> bool {
    let _ = path;
    let side = |n: u8| sides.iter().find(|stage| stage.number == n);
    let (Some(ours), Some(theirs)) = (side(2), side(3)) else {
        return true;
    };
    if sides
        .iter()
        .any(|stage| stage.mode == "120000" || stage.mode == "160000")
    {
        return true;
    }
    let binary = |object: &str| {
        git::run(folder, &["cat-file", "blob", object], git::READ)
            .ok()
            .filter(git::Run::ok)
            .is_none_or(|run| run.out.bytes().take(8000).any(|b| b == 0))
    };
    binary(&ours.object) || binary(&theirs.object)
}

/// What is staged at `path` (stage 0), or `None`.
fn staged_text(folder: &Path, path: &str) -> Option<Vec<u8>> {
    git::run_raw(
        folder,
        &[
            "--literal-pathspecs",
            "cat-file",
            "blob",
            &format!(":0:{path}"),
        ],
        git::READ,
    )
    .ok()
    .filter(|run| run.code == Some(0))
    .map(|run| run.out)
}

/// A plain file in the folder read, as a save reads one: opened one folder at a time and never
/// through a link, with one name, and its bytes read from that descriptor.
fn read_plain(folder: &Path, path: &str) -> Result<(std::fs::File, bool, Vec<u8>), String> {
    use std::io::Read;
    let (file, runs) = match ondisk::opened(folder, path) {
        ondisk::OnDisk::File(file, runs) => (file, runs),
        ondisk::OnDisk::SecondName => {
            let resolving = Ok(vec![ondisk::Held {
                path: path.to_owned(),
                why: ondisk::Why::SecondName,
            }]);
            return Err(ondisk::refused_ahead(&resolving)
                .map(|refusal| {
                    format!(
                        "A settled file is not the one file on disk: {} Settle it again as one \
                         file, then Continue.",
                        refusal.sentence()
                    )
                })
                .unwrap_or_default());
        }
        ondisk::OnDisk::Gone => {
            return Err(format!(
                "`{path}` is gone from the resolve folder. Settle it there (`git add` or `git \
                 rm` it), then Continue."
            ));
        }
        #[cfg(not(unix))]
        ondisk::OnDisk::ByGit => {
            let file = std::fs::File::open(folder.join(path))
                .map_err(|why| format!("`{path}` cannot be read: {why}."))?;
            (file, false)
        }
        _ => {
            return Err(format!(
                "`{path}` is not a plain file, so purlis will not stage it. Settle it in a \
                 terminal, or Abort."
            ));
        }
    };
    let mut bytes = Vec::new();
    (&file)
        .take(MOST_READ + 1)
        .read_to_end(&mut bytes)
        .map_err(|why| format!("`{path}` cannot be read: {why}."))?;
    if bytes.len() as u64 > MOST_READ {
        return Err(format!(
            "`{path}` is too large for purlis to check. Settle it in a terminal, or Abort."
        ));
    }
    Ok((file, runs, bytes))
}

/// The sentence for files git could not mark a conflict in: settled in a terminal in the folder.
pub fn said_by_hand(folder: &Path, files: &[String]) -> String {
    let named: Vec<&String> = files.iter().collect();
    format!(
        "git could not mark the conflict in {} (a binary file, a file one side deleted, a link \
         or a submodule), so {} settled in a terminal, in the resolve folder `{}`: keep one \
         side with `git checkout --ours -- <file>` (this machine's) or `git checkout --theirs \
         -- <file>` (the remote's), or delete it, then `git add -- <file>` or `git rm -- \
         <file>`. Then Continue.",
        join(&named),
        if files.len() == 1 {
            "it is"
        } else {
            "they are"
        },
        folder.display(),
    )
}

fn said_markers(path: &str, how: &str) -> String {
    format!(
        "`{path}` {how} conflict markers (`<<<<<<<`, `|||||||`, `>>>>>>>`) in the resolve \
         folder. Settle them, then Continue."
    )
}

/// The paths a git call lists, NUL-separated; none where it failed.
fn paths_listed(dir: &Path, args: &[&str]) -> Vec<String> {
    git::run(dir, args, git::READ)
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

/// A git call that ran and exited 0, or what went wrong in git's words.
fn succeeded(ran: Result<git::Run, git::GitUnavailable>) -> Result<(), String> {
    match ran {
        Ok(run) if run.ok() => Ok(()),
        Ok(run) => Err(tail(&run)),
        Err(unavailable) => Err(unavailable.to_string()),
    }
}

fn join(paths: &[&String]) -> String {
    paths
        .iter()
        .map(|p| format!("`{p}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests;
