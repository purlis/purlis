// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **`project-skills`** (SK-21, #2784; spec #2762 decision 31, ADR 0099 as amended 2026-10-22):
//! the project's skills, moved out of `.claude/skills` into the Skill studio's `skills/`, in ONE
//! commit the person asked for and can review, and undo with `git revert`.
//!
//! Before it, the plane's `.claude/skills` reached a chat only where its program read that
//! folder: at the project's root, and in a clone through the copy the guest layer made there.
//! After it, every chat is handed the same skills by its own route ([ADR 0063] as amended), on
//! every machine whose purlis has the `project-skills` feature, the Skill studio's flag on or
//! off ([`crate::skills::reach_chats`]). What the commit holds, and nothing else:
//!
//! - Each folder of `.claude/skills/<name>/` moved to `skills/<name>/` with `git mv`, and
//!   `skills/<name>.toml` made new with `grant = "all"`. Not the default `root`: those skills
//!   reached every clone through the copy, so a narrower grant would take them away from every
//!   workspace's chats without a word.
//! - Each `charter:<skill>` reference, in a persona's definition and in the `.md` files of every
//!   skill folder of the plane, written `purlis:<skill>` (`rename_plane::skill_refs`: a reference,
//!   never prose, an image name or a line of a fenced code block).
//! - `requires = [{ feature = "project-skills", … }]` in the manifest
//!   ([`crate::skills::PROJECT_SKILLS_FEATURE`]). A build without project skills opens the
//!   project read-only instead of quietly losing them, and a build that has it copies no
//!   `.claude/skills` into a clone again ([`crate::guest::RETIRED_MIRROR`]).
//!
//! Once committed, it lets go of the project's claim and lock, then refreshes this machine's
//! clones ([`crate::guest::wire`], which takes neither), so the copies purlis made there go now; a
//! teammate's clones refresh the next time a chat starts in them. A copy somebody edited is kept,
//! and named in what the fix says.
//!
//! # Every chat keeps its skills, or nothing is written
//!
//! Before it writes, it refuses a folder that is not a whole skill (a chat would never get it), a
//! skill named as a built-in skill (a built-in name is never taken), a name the project's skills
//! already have, two folders that name one skill, a folder named as another's grant file, and a
//! folder whose name `skills/` already holds, each compared in any case: a disk that ignores case
//! holds two such folders as one. A folder or skill name that is not ASCII is refused, since
//! disks fold and normalise it each their own way. After it has moved them and before it
//! commits, it works out the skill set of every chat it can name: at the
//! project's root and in each workspace, as no persona and as each persona, on every route, and
//! in each clone with that clone's own committed skills on each program's route. Unless each
//! moved skill is in every one of them, or left out only where a workspace turned it off or a
//! narrower copy of its name stands in for it, everything is put back. A clone's own skill of the
//! same name is said, never hidden.
//!
//! # Refused, with nothing written
//!
//! From inside a chat; anywhere but a project's root (a folder inside a project, a workspace's
//! clone); while the Skill studio's preview is off on this machine; on a project this purlis may
//! not write; outside a git repository whose top is the project; with tracked changes not
//! committed; on a detached HEAD or in the middle of a merge, a rebase or a cherry-pick
//! ([`super::commit::committable`]); and where `.claude/skills` holds what a move cannot carry
//! honestly: a file git does not track, a link, a submodule, or a file beside the skill folders.
//!
//! # All or nothing
//!
//! Every change is made in the working tree and committed with exactly its own paths, through
//! purlis's git runner, which runs no hooks. When a step fails before the commit is made (the
//! commit itself included, a signer that refuses for one), the index is put back as `HEAD` has
//! it, every moved folder and rewritten file is checked out from `HEAD`, and what the fix made is
//! removed through its folder's handle: nothing else. Once the commit is made nothing is undone,
//! and `git revert` undoes it whole. Running it again on a moved project changes nothing.
//!
//! [ADR 0063]: ../../../../../docs/adr/0063-every-harness-a-chat-runs-on-is-handed-charters-skills-by-its-own-route.md

use std::path::Path;

use super::Fixed;
use super::commit::{
    CLAIM_WAIT, Work, commit_paths, committable, git, read, tracked, with_feature_named, write,
};
use super::rename_plane::{is_persona, skill_refs};
use crate::counts::counted;
use crate::guest::{RETIRED_MIRROR, under};
use crate::harness::Harness;
use crate::skills::{Grant, Origin, Reason, Route, SkillSet};

/// The commit's subject line.
pub const SUBJECT: &str = "Move this project's skills into the Skill studio (project-skills)";

/// Where the project's skills were: the plane's own `.claude/skills`, the mirror that retires.
const FROM: &str = RETIRED_MIRROR;

/// Where they go: the project's skills ([`crate::skills::PROJECT_DIR`]).
const TO: &str = crate::skills::PROJECT_DIR;

/// The mode git gives an entry that is a link.
const LINK_MODE: &str = "120000";

/// The mode git gives an entry that is a submodule.
const SUBMODULE_MODE: &str = "160000";

/// Every route a chat's skills can take: what [`kept`] works out each chat's set on.
const ROUTES: [Route; 4] = [Route::Plugin, Route::Config, Route::Briefing, Route::Mcp];

/// What a chat asking for the fix is told.
pub const FROM_A_CHAT: &str = "project-skills moves this project's committed skills for everyone \
     who works in it, so it is not run from inside a chat; nothing was written. Run `purlis \
     doctor --fix project-skills` from a terminal";

/// What a machine without the Skill studio's preview is told.
pub const STUDIO_OFF: &str = "moving the project's skills into the Skill studio is part of its \
     preview, which is off on this machine; nothing was written. Turn it on in Settings › You › \
     Previews, then run the fix again";

/// Apply `project-skills` to the project at `root`, as this process's environment and this
/// machine's Skill studio switch say, with the built-in skills shipped beside this binary.
pub fn apply(root: &Path) -> Fixed {
    let built_in = crate::skills::built_in_here();
    apply_in(
        root,
        built_in.as_deref(),
        &|name| crate::envvar::var(name),
        crate::skills::studio_on_here(),
    )
}

/// [`apply`], with the built-in skills in `built_in`, the environment read through `env` (a
/// chat's session refuses it) and the Skill studio's switch as `studio`.
pub fn apply_in(
    root: &Path,
    built_in: Option<&Path>,
    env: &dyn Fn(&str) -> Option<String>,
    studio: bool,
) -> Fixed {
    if env(crate::active::SESSION_ID_ENV).is_some_and(|id| !id.is_empty()) {
        return Fixed::Refused(FROM_A_CHAT.to_owned());
    }
    if let Err(why) = the_projects_root(root) {
        return Fixed::Refused(why);
    }
    if !studio {
        return Fixed::Refused(STUDIO_OFF.to_owned());
    }
    let Some(claim) = crate::planegit::Claim::within(root, CLAIM_WAIT) else {
        return Fixed::Refused(
            "a save is running in this project; nothing was written. Run the fix again once it \
             has finished"
                .to_owned(),
        );
    };
    let held = crate::rewrite::Lock::on(root);
    if let Err(why) = committable(root, "the move") {
        return Fixed::Refused(why);
    }
    let tracked = match tracked(root) {
        Ok(tracked) => tracked,
        Err(why) => return Fixed::Refused(why),
    };
    let project = Project::read(root);
    let plan = match plan(root, built_in, &tracked) {
        Ok(plan) => plan,
        Err(why) => return Fixed::Refused(why),
    };
    let mut work = Moving::default();
    let clashes = match make(root, &tracked, &plan, &mut work)
        .and_then(|()| kept(root, built_in, &project, &plan.folders))
    {
        Ok(clashes) => clashes,
        Err(why) => {
            let restored = work.undo(root);
            return Fixed::Refused(format!("{why}; {restored}"));
        }
    };
    let references = references(root, built_in, &project);
    if work.is_empty() {
        let mut said = vec![
            "✓ this project's skills are in the Skill studio and name purlis's skills by \
             purlis's name — nothing to do."
                .to_owned(),
        ];
        said.extend(references);
        return Fixed::Ran {
            said,
            complete: true,
        };
    }
    let before = git(root, &["rev-parse", "HEAD"]).unwrap_or_default();
    let paths = work.staged();
    let paths: Vec<&str> = paths.iter().map(String::as_str).collect();
    match commit_paths(root, &paths, &body(&plan)) {
        Ok(sha) => {
            // The commit is made, so the project is let go before its clones are refreshed:
            // a refresh is a chat's start, which never waits on the project's claim or lock.
            drop(held);
            drop(claim);
            let mut said = std::mem::take(&mut work.text.said);
            said.extend(references);
            said.extend(clashes);
            said.extend(after(root, &project, &plan, sha.as_deref()));
            Fixed::Ran {
                said,
                complete: true,
            }
        }
        Err(why) => {
            // A commit git reports failing may still have been made: a moved HEAD is a commit,
            // and a commit is never undone.
            let now = git(root, &["rev-parse", "HEAD"]).unwrap_or_default();
            if !before.is_empty() && now != before {
                return Fixed::Ran {
                    said: vec![format!(
                        "✗ {why}, but HEAD moved: the move is committed; check it with `git log \
                         -1`"
                    )],
                    complete: false,
                };
            }
            let restored = work.undo(root);
            Fixed::Refused(format!("{why}; {restored}"))
        }
    }
}

/// Why `root` is not a project's own root, or `Ok`: it has no manifest, or a project above it
/// holds it (a folder inside a project, or a workspace's clone), whose commit is not this one.
fn the_projects_root(root: &Path) -> Result<(), String> {
    if !crate::names::has_manifest(root) {
        return Err(
            "this folder is not a project's root, so there are no project skills to move; \
             nothing was written. Run the fix at the project's root"
                .to_owned(),
        );
    }
    let resolved = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    if let Some(above) = resolved
        .ancestors()
        .skip(1)
        .find(|above| crate::names::has_manifest(above))
    {
        return Err(format!(
            "this folder is inside the project at {}, so its skills are that project's to move; \
             nothing was written. Run the fix at that project's root",
            above.display()
        ));
    }
    Ok(())
}

/// The workspaces and personas of the project, read once for the whole fix.
struct Project {
    workspaces: Vec<String>,
    personas: Vec<String>,
}

impl Project {
    fn read(root: &Path) -> Project {
        Project {
            workspaces: crate::workspaces::Plane::open(root)
                .workspaces()
                .unwrap_or_default(),
            personas: crate::personaverbs::names(root),
        }
    }
}

/// One folder of `.claude/skills` and the skill it holds, by its `name`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Folder {
    folder: String,
    name: String,
}

/// One folder the fix moved, as git spells both ends.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Moved {
    from: String,
    to: String,
}

/// What the fix will move, decided before anything is written.
struct Plan {
    /// Every folder of `.claude/skills`, each a whole skill, in order.
    folders: Vec<Folder>,
    /// Whether `skills/` must be made.
    make_to: bool,
}

/// What the fix will move, or why it moves nothing.
fn plan(root: &Path, built_in: Option<&Path>, tracked: &[String]) -> Result<Plan, String> {
    let ours: Vec<&String> = tracked.iter().filter(|path| under(&[FROM], path)).collect();
    if ours.is_empty() {
        return Ok(Plan {
            folders: Vec::new(),
            make_to: false,
        });
    }
    if cfg!(not(unix)) {
        return Err(
            "the project's skills are moved only where their folders can be held open, which \
             this system does not offer; nothing was written"
                .to_owned(),
        );
    }
    carried(root)?;
    let mut names: Vec<String> = Vec::new();
    for path in &ours {
        let inside = path
            .strip_prefix(FROM)
            .unwrap_or(path)
            .trim_start_matches('/');
        match inside.split_once('/') {
            Some((folder, _)) => {
                if !names.iter().any(|f| f == folder) {
                    names.push(folder.to_owned());
                }
            }
            None => {
                return Err(format!(
                    "the project's old skills folder holds {}, a file beside the skill folders, \
                     and only a skill's folder moves into the Skill studio; nothing was written. \
                     Move it into a skill's folder or remove it, commit, then run the fix again",
                    if inside.is_empty() { FROM } else { inside }
                ));
            }
        }
    }
    names.sort();
    clash_among(&names)?;
    let make_to = match root.join(TO).symlink_metadata() {
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => true,
        Ok(meta) if meta.file_type().is_dir() => false,
        _ => {
            return Err(
                "the project's skills folder is not a folder, so no skill can move into it; \
                 nothing was written"
                    .to_owned(),
            );
        }
    };
    // What `skills/` holds, by the names its entries have: compared in any case, so a teammate
    // whose disk ignores case gets the same answer as this machine.
    let held: Vec<String> = if make_to {
        Vec::new()
    } else {
        std::fs::read_dir(root.join(TO))
            .map_err(|why| format!("could not read the project's skills folder ({why})"))?
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect()
    };
    for folder in &names {
        if let Some(taken) = taken_by(&held, folder) {
            return Err(format!(
                "the project's skills already hold {taken}, so the skill folder {folder} cannot \
                 move into its place; nothing was written. Rename one of them, commit, then run \
                 the fix again"
            ));
        }
    }
    let folders = whole(root, built_in, &names)?;
    Ok(Plan { folders, make_to })
}

/// Why the folders `names` cannot all move, among themselves, or `Ok`: a name that is not ASCII
/// (which disks fold and normalise each their own way), two that differ only in case (one folder
/// on a disk that ignores case, as macOS's and Windows' do by default), or a folder named as
/// another's grant file `<folder>.toml`, in any case.
fn clash_among(names: &[String]) -> Result<(), String> {
    if let Some(odd) = names.iter().find(|name| !name.is_ascii()) {
        return Err(format!(
            "the skill folder {odd} has a name that is not plain ASCII, which disks compare each \
             their own way; nothing was written. Rename it to letters, digits and hyphens, \
             commit, then run the fix again"
        ));
    }
    for (at, one) in names.iter().enumerate() {
        if let Some(other) = names[at + 1..].iter().find(|other| same_name(one, other)) {
            return Err(format!(
                "the skill folders {one} and {other} differ only in case, so they are one folder \
                 wherever a disk ignores case; nothing was written. Rename one of them, commit, \
                 then run the fix again"
            ));
        }
    }
    for folder in names {
        let grant = format!("{folder}.toml");
        if let Some(other) = names.iter().find(|other| same_name(other, &grant)) {
            return Err(format!(
                "the skill folder {other} has the name of the skill folder {folder}'s grant file \
                 {grant}, so one would land on the other; nothing was written. Rename one of \
                 them, commit, then run the fix again"
            ));
        }
    }
    Ok(())
}

/// The entry of `held` (what `skills/` holds) that the folder `folder` or its grant file
/// `<folder>.toml` would land on, in any case, or `None`.
fn taken_by<'a>(held: &'a [String], folder: &str) -> Option<&'a str> {
    let grant = format!("{folder}.toml");
    held.iter()
        .find(|entry| same_name(entry, folder) || same_name(entry, &grant))
        .map(String::as_str)
}

/// Whether two names are one where case is ignored: a folder's on a disk that ignores case, a
/// skill's to every program that folds it. The move takes ASCII names only ([`clash_among`],
/// [`whole`]), so ASCII's fold is every disk's.
fn same_name(one: &str, other: &str) -> bool {
    one.eq_ignore_ascii_case(other)
}

/// Why `.claude/skills` holds something a move cannot carry, or `Ok`: a file git does not track
/// (it would move without being committed, or stay behind), a link (its target would change
/// meaning in another folder) or a submodule.
fn carried(root: &Path) -> Result<(), String> {
    let staged = git(root, &["ls-files", "-s", "-z", "--", FROM])
        .map_err(|why| format!("git could not list the project's old skills folder ({why})"))?;
    for entry in staged.split('\0').filter(|e| !e.is_empty()) {
        let (meta, path) = entry.split_once('\t').unwrap_or((entry, ""));
        let what = match meta.split(' ').next().unwrap_or_default() {
            LINK_MODE => "a link",
            SUBMODULE_MODE => "a submodule",
            _ => continue,
        };
        return Err(format!(
            "{} is {what}, which would point somewhere else once its skill moved, so purlis does \
             not move it; nothing was written. Replace it with the files it points at, commit, \
             then run the fix again",
            shown(path)
        ));
    }
    let loose = git(
        root,
        &[
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
            "--ignored=matching",
            "--",
            FROM,
        ],
    )
    .map_err(|why| {
        format!("git could not say what the project's old skills folder holds ({why})")
    })?;
    let loose: Vec<&str> = loose
        .split('\0')
        .filter(|l| l.starts_with("??") || l.starts_with("!!"))
        .map(|l| l.get(3..).unwrap_or(l))
        .collect();
    if let Some(first) = loose.first() {
        let more = match loose.len() - 1 {
            0 => String::new(),
            more => format!(" and {more} more"),
        };
        return Err(format!(
            "the project's old skills folder holds files git does not track ({}{more}), which a \
             move would carry without a commit or leave behind; nothing was written. Commit them \
             or take them out, then run the fix again",
            shown(first)
        ));
    }
    Ok(())
}

/// The skill each of `names`' folders holds, or why moving one would lose it: the folder is not a
/// whole skill, or its name is not ASCII, or belongs to a built-in skill
/// ([`crate::skills::Reserved`]), or is a project skill's or another moved folder's, each in any
/// case.
fn whole(root: &Path, built_in: Option<&Path>, names: &[String]) -> Result<Vec<Folder>, String> {
    let reserved = crate::skills::Reserved::of(built_in);
    let project: Vec<String> = crate::skills::scan(root, Path::new(TO))
        .into_iter()
        .filter_map(Result::ok)
        .map(|skill| skill.name)
        .collect();
    let scanned = crate::skills::scan(root, Path::new(FROM));
    let mut folders: Vec<Folder> = Vec::new();
    for folder in names {
        let found = scanned.iter().find(|one| {
            let at = match one {
                Ok(skill) => skill.path.parent(),
                Err((folder, _)) => Some(folder.as_path()),
            };
            at.and_then(Path::file_name)
                .is_some_and(|name| name.to_string_lossy() == folder.as_str())
        });
        let name = match found {
            Some(Ok(skill)) => skill.name.clone(),
            Some(Err((_, missing))) => {
                return Err(lost_before(
                    folder,
                    &Reason::NotASkill(*missing).to_string(),
                ));
            }
            None => return Err(lost_before(folder, "it is not a folder purlis can read")),
        };
        let why = if !name.is_ascii() {
            Some(
                "the name is not plain ASCII, which programs compare each their own way; rename \
                 it to letters, digits and hyphens"
                    .to_owned(),
            )
        } else if reserved.holds(&name.to_ascii_lowercase()) {
            Some(
                "this name belongs to a built-in skill, and a built-in skill is never replaced"
                    .to_owned(),
            )
        } else if let Some(theirs) = project.iter().find(|theirs| same_name(theirs, &name)) {
            Some(format!(
                "the project's skills already have the skill {theirs}, the same name in any case"
            ))
        } else {
            folders
                .iter()
                .find(|other| same_name(&other.name, &name))
                .map(|other| format!("the skill folder {} names the same skill", other.folder))
        };
        if let Some(why) = why {
            return Err(lost_before(folder, &format!("its skill {name}: {why}")));
        }
        folders.push(Folder {
            folder: folder.clone(),
            name,
        });
    }
    Ok(folders)
}

/// The refusal of a folder whose skill no chat would get once it moved.
fn lost_before(folder: &str, why: &str) -> String {
    format!(
        "the skill folder {folder} would be left out of every chat once it moved ({why}); nothing \
         was written. Mend or rename it, commit, then run the fix again"
    )
}

/// What the fix changed so far: the rewritten files ([`Work`]), the folders moved, the grant
/// files made (by their names in `skills/`), and whether it made `skills/`.
#[derive(Default)]
struct Moving {
    text: Work,
    moved: Vec<Moved>,
    grants: Vec<String>,
    made_to: bool,
}

impl Moving {
    fn is_empty(&self) -> bool {
        self.text.touched.is_empty() && self.moved.is_empty() && self.grants.is_empty()
    }

    /// Every path the commit holds: each file rewritten or made, and each folder at both ends of
    /// its move.
    fn staged(&self) -> Vec<String> {
        let mut paths = self.text.touched.clone();
        for moved in &self.moved {
            paths.push(moved.from.clone());
            paths.push(moved.to.clone());
        }
        paths.extend(self.grants.iter().map(|grant| format!("{TO}/{grant}")));
        paths
    }

    /// Put everything back as `HEAD` has it. The project had no tracked change when the fix
    /// began ([`committable`]), so the index goes back to `HEAD` whole, then each moved folder and
    /// each rewritten file is checked out from `HEAD`, and what the fix made in `skills/` (the
    /// moved folders' new copies, the grant files, the folder itself where the fix made it) is
    /// removed through that folder's handle, by the names the fix gave it.
    fn undo(&self, root: &Path) -> String {
        if self.is_empty() && !self.made_to {
            return "nothing was written".to_owned();
        }
        let mut back = git(root, &["reset", "-q"]).is_ok();
        let mut restore: Vec<&str> = self.moved.iter().map(|m| m.from.as_str()).collect();
        restore.extend(self.text.touched.iter().map(String::as_str));
        if !restore.is_empty() {
            let mut args = vec!["checkout", "-q", "HEAD", "--"];
            args.extend(restore);
            back &= git(root, &args).is_ok();
        }
        back &= self.take_out_what_it_made(root).is_ok();
        if back {
            "every file it changed was put back as it was".to_owned()
        } else {
            "not every file could be put back — `git status` shows what is left; `git reset \
             --hard HEAD` puts the project back as its last commit has it"
                .to_owned()
        }
    }

    #[cfg(unix)]
    fn take_out_what_it_made(&self, root: &Path) -> std::io::Result<()> {
        use crate::skills::at;
        let root_dir = at::folder(root)?;
        let skills = match at::folder_in(&root_dir, TO) {
            Ok(skills) => skills,
            Err(why) if why.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(why) => return Err(why),
        };
        for moved in &self.moved {
            let leaf = moved
                .to
                .strip_prefix(&format!("{TO}/"))
                .unwrap_or(&moved.to);
            if at::entry_in(&skills, leaf)?.is_some() {
                at::remove_tree_in(&skills, leaf)?;
            }
        }
        for grant in &self.grants {
            if at::entry_in(&skills, grant)?.is_some() {
                at::remove_file_in(&skills, grant)?;
            }
        }
        if self.made_to {
            at::remove_empty_folder_in(&root_dir, TO)?;
        }
        Ok(())
    }

    #[cfg(not(unix))]
    fn take_out_what_it_made(&self, _: &Path) -> std::io::Result<()> {
        Ok(())
    }
}

/// Every change, made in the working tree and recorded in `work`.
fn make(root: &Path, tracked: &[String], plan: &Plan, work: &mut Moving) -> Result<(), String> {
    // References first, where `HEAD` has each file, so a failure puts each back by its path.
    for path in tracked {
        if !(is_persona(path) || in_a_skill(path)) {
            continue;
        }
        let text = read(root, path)?;
        let renamed = skill_refs(&text);
        if renamed != text {
            write(root, path, &renamed)?;
            work.text.touch(path);
            let line = format!("✓ {} names purlis's skills by purlis's name", whose(path));
            if !work.text.said.contains(&line) {
                work.text.said.push(line);
            }
        }
    }
    if plan.folders.is_empty() {
        return Ok(());
    }
    move_folders(root, plan, work)?;
    let manifest = crate::names::manifest_name(root);
    let text = read(root, manifest)?;
    if let Some(marked) = with_feature_named(&text, crate::skills::PROJECT_SKILLS_FEATURE)
        .map_err(|why| format!("{manifest}: {why}"))?
    {
        write(root, manifest, &marked)?;
        work.text.touch(manifest);
        work.text.said.push(
            "✓ the project now needs a purlis that has project skills: an older one opens it \
             read-only, and never starts a chat without them"
                .to_owned(),
        );
    }
    Ok(())
}

/// Each folder of the plan moved into `skills/` and granted to every workspace, `skills/` made
/// first where it is not there, all through the folders' handles.
#[cfg(unix)]
fn move_folders(root: &Path, plan: &Plan, work: &mut Moving) -> Result<(), String> {
    use crate::skills::at;
    use std::io::Write;
    let root_dir = at::folder(root).map_err(|why| format!("could not open the project ({why})"))?;
    if plan.make_to {
        at::make_folder_in(&root_dir, TO)
            .map_err(|why| format!("could not make the project's skills folder ({why})"))?;
        work.made_to = true;
    }
    let skills = at::folder_in(&root_dir, TO)
        .map_err(|why| format!("could not open the project's skills folder ({why})"))?;
    let grant = crate::skills::grant_file(&Grant::All);
    for Folder { folder, name } in &plan.folders {
        let moved = Moved {
            from: format!("{FROM}/{folder}"),
            to: format!("{TO}/{folder}"),
        };
        git(root, &["mv", "--", &moved.from, &moved.to])
            .map_err(|why| format!("could not move the skill {name} ({why})"))?;
        work.moved.push(moved);
        let file = format!("{folder}.toml");
        // Made new, never over anything: the plan proved the name free.
        let mut made = at::new_file_in(&skills, &file)
            .map_err(|why| format!("could not grant the skill {name} ({why})"))?;
        work.grants.push(file);
        made.write_all(grant.as_bytes())
            .and_then(|()| made.sync_all())
            .map_err(|why| format!("could not grant the skill {name} ({why})"))?;
        work.text.said.push(format!(
            "✓ the skill {name} is a project skill now, shared with every workspace as before"
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
fn move_folders(_: &Path, _: &Plan, _: &mut Moving) -> Result<(), String> {
    Err("the project's skills are moved only where their folders can be held open".to_owned())
}

/// What is said of each clone's own skill of a moved skill's name, or why a chat would be
/// without one of the moved skills: every chat at the project's root and in each workspace, as
/// no persona and as each persona, on every route, and in each clone with its own committed
/// skills on each program's route.
fn kept(
    root: &Path,
    built_in: Option<&Path>,
    project: &Project,
    folders: &[Folder],
) -> Result<Vec<String>, String> {
    if folders.is_empty() {
        return Ok(Vec::new());
    }
    let places: Vec<Option<&str>> = std::iter::once(None)
        .chain(project.workspaces.iter().map(|ws| Some(ws.as_str())))
        .collect();
    let personas: Vec<Option<&str>> = std::iter::once(None)
        .chain(project.personas.iter().map(|p| Some(p.as_str())))
        .collect();
    for place in &places {
        for persona in &personas {
            for route in ROUTES {
                let set = crate::skills::of_a_chat(built_in, root, *place, *persona, route);
                check(root, &set, folders, || chat_at(*place, *persona))?;
            }
        }
    }
    let mut clashes = Vec::new();
    for ws in &project.workspaces {
        let Ok(clones) = crate::repos::clones(root, ws) else {
            continue;
        };
        for clone in &clones.repos {
            for harness in Harness::ALL {
                // The clone's own skills: what it commits. A copy purlis made there is not one.
                let own: Vec<crate::skills::OutsideSkill> =
                    crate::skills::repo_skills_of_chat(harness, root, ws, &clone.path)
                        .into_iter()
                        .filter(|one| {
                            one.skill
                                .path
                                .strip_prefix(&clone.path)
                                .ok()
                                .and_then(Path::to_str)
                                .is_some_and(|rel| crate::guest::tracked(&clone.path, rel))
                        })
                        .collect();
                for one in &own {
                    if folders.iter().any(|f| f.name == one.skill.name) {
                        let line = format!(
                            "• the repo {} in the workspace {ws} has its own skill {}: its chats \
                             there get the repo's copy, as they did before",
                            clone.name, one.skill.name
                        );
                        if !clashes.contains(&line) {
                            clashes.push(line);
                        }
                    }
                }
                let set =
                    crate::skills::of_a_chat(built_in, root, Some(ws), None, harness.skills())
                        .with_outside(harness.skills(), own);
                check(root, &set, folders, || {
                    format!("in the repo {} of the workspace {ws}", clone.name)
                })?;
            }
        }
    }
    Ok(clashes)
}

/// Where a chat is, as a refusal says it.
fn chat_at(place: Option<&str>, persona: Option<&str>) -> String {
    let at = match place {
        None => "at the project's root".to_owned(),
        Some(ws) => format!("in the workspace {ws}"),
    };
    match persona {
        None => at,
        Some(persona) => format!("{at} as the persona {persona}"),
    }
}

/// Why the chat whose set is `set` (`at` says where) is without one of `folders`' skills.
fn check(
    root: &Path,
    set: &SkillSet,
    folders: &[Folder],
    at: impl Fn() -> String,
) -> Result<(), String> {
    for Folder { folder, name } in folders {
        if let Some(why) = lost(set, &root.join(TO).join(folder), name) {
            return Err(format!(
                "a chat {} would be without the skill {name} once it moved ({why}), so nothing \
                 was moved",
                at()
            ));
        }
    }
    Ok(())
}

/// Why a chat whose set is `set` lost the skill `name` moved into `folder`, or `None` where it
/// has it, or a workspace turned it off, or a narrower copy of the name stands in for it.
fn lost(set: &SkillSet, folder: &Path, name: &str) -> Option<String> {
    let here = |path: &Path| path.parent() == Some(folder);
    if set
        .effective
        .iter()
        .any(|one| one.skill.name == name && here(&one.skill.path))
    {
        return None;
    }
    let left = set
        .left_out
        .iter()
        .find(|left| left.name == name && left.folder == folder);
    match left.map(|left| &left.reason) {
        Some(Reason::TurnedOff) => None,
        Some(Reason::NameTaken { by, .. }) if *by != Origin::BuiltIn => None,
        Some(Reason::HiddenByRepo { .. }) => None,
        Some(reason) => Some(reason.to_string()),
        None => Some("it is not in the chat's skills".to_owned()),
    }
}

/// Each persona reference that names no skill a chat as that persona can get, in one line with
/// why, and one line for the personas whose references all resolve: to a built-in skill, an
/// installed one, or the persona's, a workspace's or the project's own.
fn references(root: &Path, built_in: Option<&Path>, project: &Project) -> Vec<String> {
    let mut said = Vec::new();
    let mut resolved = 0;
    let env = crate::harness_plugin::Env::of(&[]);
    for persona in &project.personas {
        let missing = crate::skills::persona::unresolved(built_in, root, persona, Some(&env));
        if missing.is_empty() {
            if !crate::skills::names_of(root, persona).is_empty() {
                resolved += 1;
            }
            continue;
        }
        for (reference, reason) in missing {
            said.push(format!(
                "• the persona {persona} names {reference}: {reason}. Its chats start without it \
                 until it is installed or the name is fixed"
            ));
        }
    }
    if resolved > 0 {
        said.push(format!(
            "✓ every skill {} names is one its chats get",
            counted(resolved, "persona", "personas")
        ));
    }
    said
}

/// What is said once the commit is made: this machine's clones refreshed, with each edited copy
/// they keep named, and what teammates get.
fn after(root: &Path, project: &Project, plan: &Plan, sha: Option<&str>) -> Vec<String> {
    let mut said = vec![match sha {
        Some(sha) => format!("✓ committed {sha}: {SUBJECT}"),
        None => {
            format!("✓ committed: {SUBJECT} (git could not name the commit; `git log -1` shows it)")
        }
    }];
    if plan.folders.is_empty() {
        return said;
    }
    said.push(format!(
        "  Every chat at the project's root, in its {} and as each persona keeps {} it had, and \
         chats that never read them before get them too.",
        counted(project.workspaces.len(), "workspace", "workspaces"),
        counted(plan.folders.len(), "skill", "skills")
    ));
    let mut refreshed = 0;
    for ws in &project.workspaces {
        let Ok(clones) = crate::repos::clones(root, ws) else {
            continue;
        };
        for clone in &clones.repos {
            crate::guest::wire(root, &clone.path);
            refreshed += 1;
            // What purlis's record still names there is a copy it kept: edited since it was
            // made. A file the repo commits is the repo's own, and said by [`kept`].
            let mut edited: Vec<String> = crate::guest::marker_at(&clone.path)
                .into_keys()
                .filter(|rel| !crate::guest::tracked(&clone.path, rel))
                .filter_map(|rel| {
                    let inside = rel.strip_prefix(FROM)?.strip_prefix('/')?;
                    Some(inside.split('/').next()?.to_owned())
                })
                .collect();
            edited.dedup();
            for skill in edited {
                said.push(format!(
                    "• the repo {} in the workspace {ws} keeps its copy of the skill folder \
                     {skill}, which was edited since purlis made it: take it out once you no \
                     longer need it",
                    clone.name
                ));
            }
        }
    }
    if refreshed > 0 {
        said.push(format!(
            "  This machine's {} refreshed: the copies purlis made there are gone.",
            counted(refreshed, "clone was", "clones were")
        ));
    }
    said.push(
        "  A teammate's clones lose their copies the next time a chat starts in each one, and a \
         copy somebody edited is kept and named there."
            .to_owned(),
    );
    said.push(
        "  A teammate's purlis needs project skills to open this project; with it, every chat \
         gets these skills, whether the Skill studio is on or off."
            .to_owned(),
    );
    said.push(match sha {
        Some(sha) => format!("  To undo it: `git revert {sha}`."),
        None => "  To undo it: `git revert` the commit.".to_owned(),
    });
    said
}

/// The commit's message.
fn body(plan: &Plan) -> String {
    let mut body = format!(
        "{SUBJECT}\n\nMade by `purlis doctor --fix project-skills`. Each skill folder of {FROM} \
         moves to {TO}/, granted to every workspace, and each charter: skill reference becomes \
         purlis:. The project now requires the `{}` feature, so a build without it opens it \
         read-only, and clones get no copy of {FROM}.\n",
        crate::skills::PROJECT_SKILLS_FEATURE
    );
    if !plan.folders.is_empty() {
        body.push('\n');
        for Folder { folder, .. } in &plan.folders {
            body.push_str(&format!("- {FROM}/{folder} -> {TO}/{folder}\n"));
        }
    }
    body
}

/// Whether `path` is a `.md` file inside a skill folder of the plane: in `.claude/skills/`, the
/// project's `skills/`, or a workspace's or a persona's `skills/`.
fn in_a_skill(path: &str) -> bool {
    path.ends_with(".md") && skill_of(path).is_some()
}

/// The skill folder `path` is inside, with the level and owner where it is a workspace's or a
/// persona's: a file in a skill's folder, never one beside the folders.
fn skill_of(path: &str) -> Option<(&str, Option<(&str, &str)>)> {
    let parts: Vec<&str> = path.split('/').collect();
    match parts.as_slice() {
        [".claude", "skills", folder, _, ..] | ["skills", folder, _, ..] => Some((folder, None)),
        [
            level @ ("workspaces" | "personas"),
            owner,
            "skills",
            folder,
            _,
            ..,
        ] => Some((folder, Some((level, owner)))),
        _ => None,
    }
}

/// Who a file whose references were rewritten is, in the window's words.
fn whose(path: &str) -> String {
    match skill_of(path) {
        Some((folder, None)) => format!("the skill {folder}"),
        Some((folder, Some(("workspaces", ws)))) => {
            format!("the skill {folder} of the workspace {ws}")
        }
        Some((folder, Some((_, persona)))) => {
            format!("the skill {folder} of the persona {persona}")
        }
        None => {
            let persona = path
                .strip_prefix("personas/")
                .and_then(|rest| rest.split('/').next())
                .map_or(path, |name| name.trim_end_matches(".md"));
            format!("the persona {persona}")
        }
    }
}

/// A path in `.claude/skills`, as a refusal names it: the skill folder and the file in it.
fn shown(path: &str) -> String {
    let inside = path
        .strip_prefix(FROM)
        .map_or(path, |rest| rest.trim_start_matches('/'));
    match inside.split_once('/') {
        Some((folder, file)) => format!("the skill folder {folder}'s {file}"),
        None => inside.to_owned(),
    }
}

#[cfg(test)]
mod tests;
