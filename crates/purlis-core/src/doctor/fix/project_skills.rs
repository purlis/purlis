// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **`project-skills`** (SK-21, #2784; spec #2762 decision 31, ADR 0099): the project's
//! skills, moved out of `.claude/skills` into the Skill studio's `skills/`, in ONE commit the
//! person asked for and can review, and undo with `git revert`.
//!
//! Before it, the plane's `.claude/skills` reached a chat only where Claude Code or opencode
//! read it: at the project's root, and in a clone through the copy the guest layer made there.
//! After it, every chat is handed the same skills by its own route ([ADR 0063] as amended), on
//! Claude Code, Codex and opencode alike. What the commit holds, and nothing else:
//!
//! - Each folder of `.claude/skills/<name>/` moved to `skills/<name>/` with `git mv`, and
//!   `skills/<name>.toml` written with `grant = "all"`. Not the default `root`: those skills
//!   reached every clone through the copy, so a narrower grant would take them away from every
//!   workspace's chats without a word.
//! - Each `charter:<skill>` reference, in a persona's definition and in the `.md` files of every
//!   skill folder of the plane, written `purlis:<skill>` (the rule `rename-plane` rewrites
//!   personas by). SK-5 already reads the old prefix as the new one; this makes the files say it.
//! - `requires = [{ feature = "project-skills", … }]` in the manifest
//!   ([`crate::skills::PROJECT_SKILLS_FEATURE`]). A build that hands a chat no project skill opens
//!   the project read-only instead of quietly losing them, and a build that has it copies no
//!   `.claude/skills` into a clone again: the guest layer withdraws the copies it made at the
//!   next wire (`guest::RETIRED_MIRROR`).
//!
//! It says what it changed, line by line: each skill moved and its grant, each file whose
//! references it rewrote, every persona reference that names no skill a chat can get (with the
//! one-line reason), and how to undo the commit.
//!
//! # Every chat keeps its skills, or nothing is written
//!
//! Before it writes, it refuses a move that would leave a chat without a skill it had: a skill
//! named as a built-in skill (a built-in name is never taken), a name the project's skills
//! already have, and two folders that name one skill. After it has moved them and before it
//! commits, it works out the skill set of a chat at the project's root and in each workspace
//! ([`crate::skills::of_a_chat`]) and puts everything back unless each moved skill is there, or
//! left out only where the workspace turned it off or a narrower copy of its name stands.
//!
//! # Refused, with nothing written
//!
//! From inside a chat, and where the Skill studio's preview is off on this machine (a chat would
//! then get none of the project's skills, so the move would lose them). As `rename-plane` is: on
//! a project this purlis may not write, outside a git repository whose top is the project, and
//! with tracked changes not committed. And where `.claude/skills` holds what a move cannot carry
//! honestly: a file git does not track, a link, a submodule, or a file beside the skill folders.
//!
//! # All or nothing
//!
//! Every change is made in the working tree and committed with exactly its own paths, through
//! purlis's own git runner, which runs no hooks. When a step fails before the commit is made,
//! every folder is moved back, every rewritten file is put back from `HEAD`, the files it made
//! are removed, and the fix says why. Once the commit is made nothing is undone, and `git revert`
//! undoes it whole. Running it again on a moved project changes nothing.
//!
//! [ADR 0063]: ../../../../../docs/adr/0063-every-harness-a-chat-runs-on-is-handed-charters-skills-by-its-own-route.md

use std::path::Path;

use super::Fixed;
use super::rename_plane::{
    CLAIM_WAIT, Work, commit_paths, committable, git, is_persona, skill_refs, tracked,
    with_feature_named,
};
use crate::counts::counted;
use crate::skills::{Grant, Origin, Reason, SkillSet};

/// The commit's subject line.
pub const SUBJECT: &str = "Move this project's skills into the Skill studio (project-skills)";

/// Where the project's skills were: the plane's own Claude Code skills folder.
pub const FROM: &str = ".claude/skills";

/// Where they go: the project's skills ([`crate::skills::PROJECT_DIR`]).
const TO: &str = crate::skills::PROJECT_DIR;

/// What a chat asking for the fix is told.
pub const FROM_A_CHAT: &str = "project-skills moves this project's committed skills for everyone \
     who works in it, so it is not run from inside a chat; nothing was written. Run `purlis \
     doctor --fix project-skills` from a terminal";

/// What a machine without the Skill studio's preview is told.
pub const STUDIO_OFF: &str = "the Skill studio is a preview, and while it is off on this machine \
     a chat gets none of the project's skills, so moving them would lose them; nothing was \
     written. Turn it on in Settings › You › Previews, then run the fix again";

/// Apply `project-skills` to the project at `root`, as this process's environment and this
/// machine's Skill studio switch say, with the built-in skills shipped beside this binary.
/// The caller has already refused a project this purlis may not write.
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
    if !studio {
        return Fixed::Refused(STUDIO_OFF.to_owned());
    }
    let Some(_claim) = crate::planegit::Claim::within(root, CLAIM_WAIT) else {
        return Fixed::Refused(
            "a save is running in this project; nothing was written. Run the fix again once it \
             has finished"
                .to_owned(),
        );
    };
    let _held = crate::rewrite::Lock::on(root);
    if let Err(why) = committable(root, "the move") {
        return Fixed::Refused(why);
    }
    let tracked = match tracked(root) {
        Ok(tracked) => tracked,
        Err(why) => return Fixed::Refused(why),
    };
    let plan = match plan(root, built_in, &tracked) {
        Ok(plan) => plan,
        Err(why) => return Fixed::Refused(why),
    };
    let mut work = Moving::default();
    if let Err(why) = make(root, built_in, &tracked, &plan, &mut work) {
        let restored = work.undo(root);
        return Fixed::Refused(format!("{why}; {restored}"));
    }
    let references = references(root, built_in);
    if work.is_empty() {
        let mut said = vec![
            "✓ this project's skills are in skills/ and name purlis's skills by purlis's name — \
             nothing to do."
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
            let mut said = std::mem::take(&mut work.text.said);
            said.extend(references);
            said.extend(after(&plan, sha.as_deref()));
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

/// What the fix will move, decided before anything is written.
struct Plan {
    /// The folders of `.claude/skills`, by name, in order.
    folders: Vec<String>,
    /// The skill each folder holds, by its `name`, where it is a whole skill: what a chat must
    /// still get once it moved.
    skills: Vec<(String, String)>,
    /// Whether `skills/` must be made.
    make_to: bool,
    /// How many workspaces the project has, for the report.
    workspaces: usize,
}

/// What the fix will move, or why it moves nothing.
fn plan(root: &Path, built_in: Option<&Path>, tracked: &[String]) -> Result<Plan, String> {
    let ours: Vec<&String> = tracked
        .iter()
        .filter(|path| under(path, FROM) || path.as_str() == FROM)
        .collect();
    let plan = Plan {
        folders: Vec::new(),
        skills: Vec::new(),
        make_to: false,
        workspaces: crate::workspaces::Plane::open(root)
            .workspaces()
            .unwrap_or_default()
            .len(),
    };
    if ours.is_empty() {
        return Ok(plan);
    }
    carried(root)?;
    let mut folders: Vec<String> = Vec::new();
    for path in &ours {
        let inside = path
            .strip_prefix(FROM)
            .unwrap_or(path)
            .trim_start_matches('/');
        match inside.split_once('/') {
            Some((folder, _)) => {
                if !folders.iter().any(|f| f == folder) {
                    folders.push(folder.to_owned());
                }
            }
            None => {
                return Err(format!(
                    "{path} is a file beside the skill folders in {FROM}, and only a skill's \
                     folder moves into the Skill studio; nothing was written. Move it into a \
                     skill's folder or remove it, commit, then run the fix again"
                ));
            }
        }
    }
    folders.sort();
    let to = root.join(TO);
    let make_to = match to.symlink_metadata() {
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => true,
        Ok(meta) if meta.file_type().is_dir() => false,
        _ => {
            return Err(format!(
                "{TO} at the project's root is not a folder, so no skill can move into it; \
                 nothing was written"
            ));
        }
    };
    for folder in &folders {
        for taken in [format!("{TO}/{folder}"), format!("{TO}/{folder}.toml")] {
            if root.join(&taken).symlink_metadata().is_ok() {
                return Err(format!(
                    "{taken} is already there, so {FROM}/{folder} cannot move into its place; \
                     nothing was written. Rename one of them, commit, then run the fix again"
                ));
            }
        }
    }
    let skills = clashes(root, built_in, &folders)?;
    Ok(Plan {
        folders,
        skills,
        make_to,
        ..plan
    })
}

/// Why `.claude/skills` holds something a move cannot carry, or `Ok`: a file git does not track
/// (it would move without being committed, or stay behind), a link (its target would change
/// meaning in another folder) or a submodule.
fn carried(root: &Path) -> Result<(), String> {
    let staged = git(root, &["ls-files", "-s", "-z", "--", FROM])
        .map_err(|why| format!("git could not list {FROM} ({why})"))?;
    for entry in staged.split('\0').filter(|e| !e.is_empty()) {
        let (meta, path) = entry.split_once('\t').unwrap_or((entry, ""));
        let mode = meta.split(' ').next().unwrap_or_default();
        let what = match mode {
            "120000" => "a link",
            "160000" => "a submodule",
            _ => continue,
        };
        return Err(format!(
            "{path} is {what}, which would point somewhere else once its skill moved, so \
             purlis does not move it; nothing was written. Replace it with the files it points \
             at, commit, then run the fix again"
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
    .map_err(|why| format!("git could not say what {FROM} holds ({why})"))?;
    let loose: Vec<&str> = loose
        .split('\0')
        .filter(|l| l.starts_with("??") || l.starts_with("!!"))
        .map(|l| l.get(3..).unwrap_or(l))
        .collect();
    if let Some(first) = loose.first() {
        let more = loose.len() - 1;
        let more = if more > 0 {
            format!(" and {more} more")
        } else {
            String::new()
        };
        return Err(format!(
            "{FROM} holds files git does not track ({first}{more}), which a move would carry \
             without a commit or leave behind; nothing was written. Commit them or take them \
             out, then run the fix again"
        ));
    }
    Ok(())
}

/// The skill each of `folders` holds, by name, or why moving one would lose it: its name is a
/// built-in skill's, a project skill's, or another moved folder's.
fn clashes(
    root: &Path,
    built_in: Option<&Path>,
    folders: &[String],
) -> Result<Vec<(String, String)>, String> {
    let built_in = crate::skills::built_in_names(built_in);
    let project: Vec<String> = crate::skills::read(&root.join(TO))
        .into_iter()
        .map(|skill| skill.name)
        .collect();
    let mut skills: Vec<(String, String)> = Vec::new();
    for skill in crate::skills::read(&root.join(FROM)) {
        let Some(folder) = skill
            .path
            .parent()
            .and_then(Path::file_name)
            .map(|name| name.to_string_lossy().into_owned())
            .filter(|folder| folders.contains(folder))
        else {
            continue;
        };
        let name = skill.name;
        let why = if built_in.contains(&name) {
            Some(
                "a built-in skill has this name, and a built-in skill is never replaced".to_owned(),
            )
        } else if project.contains(&name) {
            Some(format!(
                "the project's skills in {TO}/ already have a skill of this name"
            ))
        } else {
            skills
                .iter()
                .find(|(_, other)| *other == name)
                .map(|(other, _)| format!("{FROM}/{other} names the same skill"))
        };
        if let Some(why) = why {
            return Err(format!(
                "the skill {name} in {FROM}/{folder} would be left out of every chat once it \
                 moved: {why}; nothing was written. Rename it, commit, then run the fix again"
            ));
        }
        skills.push((folder, name));
    }
    Ok(skills)
}

/// What the fix changed so far: the rewritten files ([`Work`]), the folders moved, the grant
/// files made, and whether it made `skills/`.
#[derive(Default)]
struct Moving {
    text: Work,
    moved: Vec<(String, String)>,
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
        for (from, to) in &self.moved {
            paths.push(from.clone());
            paths.push(to.clone());
        }
        paths.extend(self.grants.iter().cloned());
        paths
    }

    /// Put everything back as `HEAD` has it: each folder moved back, then each rewritten file
    /// restored and each made one removed, then `skills/` where the fix made it and it is empty.
    fn undo(&self, root: &Path) -> String {
        let mut back = true;
        for grant in &self.grants {
            // A file the fix made where the plan proved nothing was: removed as itself.
            back &= match std::fs::remove_file(root.join(grant)) {
                Ok(()) => true,
                Err(why) => why.kind() == std::io::ErrorKind::NotFound,
            };
        }
        if !self.moved.is_empty() {
            // git may have taken the folder the last move emptied.
            back &= std::fs::create_dir_all(root.join(FROM)).is_ok();
        }
        for (from, to) in self.moved.iter().rev() {
            back &= git(root, &["mv", "--", to, from]).is_ok();
        }
        if self.moved.is_empty() && self.grants.is_empty() && self.text.touched.is_empty() {
            return "nothing was written".to_owned();
        }
        let restored = if self.text.touched.is_empty() {
            "every file it changed was put back as it was".to_owned()
        } else {
            self.text.undo(root)
        };
        if self.made_to {
            // Only the folder the fix made, and only empty: `remove_dir` never takes a file.
            let _ = std::fs::remove_dir(root.join(TO));
        }
        if back {
            restored
        } else {
            format!(
                "not every folder could be moved back — `git status` shows what is left; `git \
                 reset --hard HEAD` puts the project back as its last commit has it ({restored})"
            )
        }
    }
}

/// Every change, made in the working tree and recorded in `work`, then checked against the skill
/// set of every chat.
fn make(
    root: &Path,
    built_in: Option<&Path>,
    tracked: &[String],
    plan: &Plan,
    work: &mut Moving,
) -> Result<(), String> {
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
            work.text
                .said
                .push(format!("✓ {path}: skill references under purlis's name"));
        }
    }
    if plan.folders.is_empty() {
        return Ok(());
    }
    if plan.make_to {
        std::fs::create_dir(root.join(TO))
            .map_err(|why| format!("could not make {TO}/ ({why})"))?;
        work.made_to = true;
    }
    for folder in &plan.folders {
        let (from, to) = (format!("{FROM}/{folder}"), format!("{TO}/{folder}"));
        git(root, &["mv", "--", &from, &to])
            .map_err(|why| format!("could not move {from} ({why})"))?;
        work.moved.push((from.clone(), to.clone()));
        let grant = format!("{to}.toml");
        // Proved not there by the plan, so the file is the fix's own from here on.
        work.grants.push(grant.clone());
        crate::skills::set_grant(&root.join(&to), &Grant::All)?;
        work.text.said.push(format!(
            "✓ moved {from} to {to}, granted to every workspace ({grant})"
        ));
    }
    let manifest = crate::names::manifest_name(root);
    let text = read(root, manifest)?;
    let feature = crate::skills::PROJECT_SKILLS_FEATURE;
    if let Some(marked) =
        with_feature_named(&text, feature).map_err(|why| format!("{manifest}: {why}"))?
    {
        write(root, manifest, &marked)?;
        work.text.touch(manifest);
        work.text
            .said
            .push(format!("✓ {manifest} requires `{feature}`"));
    }
    kept(root, built_in, plan)
}

/// Why a chat would be without one of the moved skills, or `Ok` where every chat at the
/// project's root and in each workspace still gets each of them.
fn kept(root: &Path, built_in: Option<&Path>, plan: &Plan) -> Result<(), String> {
    let mut places: Vec<Option<String>> = vec![None];
    places.extend(
        crate::workspaces::Plane::open(root)
            .workspaces()
            .unwrap_or_default()
            .into_iter()
            .map(Some),
    );
    for place in &places {
        let set = crate::skills::of_a_chat(
            built_in,
            root,
            place.as_deref(),
            None,
            crate::skills::Route::Plugin,
        );
        for (folder, name) in &plan.skills {
            if let Some(why) = lost(&set, &root.join(TO).join(folder), name) {
                let at = match place {
                    None => "at the project's root".to_owned(),
                    Some(ws) => format!("in the workspace {ws}"),
                };
                return Err(format!(
                    "a chat {at} would be without the skill {name} once it moved ({why}), so \
                     nothing was moved"
                ));
            }
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
        Some(reason) => Some(reason.to_string()),
        None => Some("it is not in the chat's skills".to_owned()),
    }
}

/// Each persona reference that names no skill a chat as that persona can get, in one line with
/// why, and one line for the personas whose references all resolve: to a built-in skill, an
/// installed one, or the persona's, a workspace's or the project's own.
fn references(root: &Path, built_in: Option<&Path>) -> Vec<String> {
    let mut said = Vec::new();
    let mut resolved = 0;
    for persona in crate::personaverbs::names(root) {
        let names = crate::skills::names_of(root, &persona);
        if names.is_empty() {
            continue;
        }
        let set = crate::skills::in_the_studio(built_in, root, None, Some(&persona));
        let mut all = true;
        for reference in &names {
            // A reference the set leaves out is listed as its `skills:` list writes it.
            let written = reference.to_string();
            let missing = set.left_out.iter().find(|left| {
                left.name == written
                    && matches!(
                        left.reason,
                        Reason::NotOnThisMachine | Reason::NoSuchBuiltIn
                    )
            });
            if let Some(left) = missing {
                all = false;
                said.push(format!(
                    "• the persona {persona} names {reference}: {}. Its chats start without it \
                     until it is installed or the name is fixed",
                    left.reason
                ));
            }
        }
        if all {
            resolved += 1;
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

/// What is said once the commit is made.
fn after(plan: &Plan, sha: Option<&str>) -> Vec<String> {
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
        "  Every chat at the project's root and in its {} gets {} it had, on Claude Code, Codex \
         and opencode. Codex never read {FROM}, so its chats get them now too.",
        counted(plan.workspaces, "workspace", "workspaces"),
        match plan.skills.len() {
            1 => "the skill".to_owned(),
            n => format!("the {n} skills"),
        }
    ));
    said.push(format!(
        "  Clones get no copy of {FROM} from now on, and the copies made there are taken \
         out the next time a chat starts in each one."
    ));
    said.push(format!(
        "  A teammate gets these skills with a purlis that has the `{}` feature and the Skill \
         studio's preview on; a build without the feature opens this project read-only.",
        crate::skills::PROJECT_SKILLS_FEATURE
    ));
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
        for folder in &plan.folders {
            body.push_str(&format!("- {FROM}/{folder} -> {TO}/{folder}\n"));
        }
    }
    body
}

/// Whether `path` is `root/…`.
fn under(path: &str, root: &str) -> bool {
    path.strip_prefix(root)
        .is_some_and(|rest| rest.starts_with('/'))
}

/// Whether `path` is a `.md` file inside a skill folder of the plane: in `.claude/skills/`, the
/// project's `skills/`, or a workspace's or a persona's `skills/`.
fn in_a_skill(path: &str) -> bool {
    if !path.ends_with(".md") {
        return false;
    }
    let parts: Vec<&str> = path.split('/').collect();
    let inside = match parts.as_slice() {
        [".claude", "skills", rest @ ..] | ["skills", rest @ ..] => rest,
        ["workspaces" | "personas", _, "skills", rest @ ..] => rest,
        _ => return false,
    };
    // A file in a skill's folder, never one beside the folders.
    inside.len() >= 2
}

/// The text of `rel`, read with no link followed on the way to it or at it.
fn read(root: &Path, rel: &str) -> Result<String, String> {
    crate::contain::read_text_no_link(root, &root.join(rel))
        .map_err(|e| format!("could not read {rel} ({e})"))
}

/// `rel` replaced whole with `text`, refused where it or a folder on the way is a link.
fn write(root: &Path, rel: &str, text: &str) -> Result<(), String> {
    super::rename_plane::write(root, rel, text)
}

#[cfg(test)]
mod tests;
