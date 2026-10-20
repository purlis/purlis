// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A chat's skills, as purlis-core asks [`purlis_skills`] for them (ADR 0063, SK-1).
//!
//! How a chat's skill set is worked out is `purlis-skills`' alone: which skills a chat gets, by
//! which [`Route`] and from which [`Origin`], and why each left-out one is left out. This module
//! hands that crate what purlis-core knows about a chat, where the chat is armed
//! ([`for_chat`]) and where a briefed chat's `SessionStart` lists its skills ([`briefed`]), and
//! it holds the `skill-studio` flag ([`studio_on`]). A harness adapter asks [`for_chat`] and
//! turns the answer into its harness's argv, env and config; the route each harness takes is its
//! declaration's fact, [`crate::harness::Harness::skills`].

use std::path::{Path, PathBuf};

pub use purlis_skills::{
    Chat, DIR_IN_BUNDLE, Effective, Grant, HarnessSkills, LOCAL_DIR, LeftOut, Machine, Missing,
    Origin, PROJECT_DIR, Reason, Route, Skill, SkillSet, grant_of, in_bundle, level_dir,
    level_shown, listing, named, read, set_grant, share,
};

/// Skill runs: the span from a skill's invocation to the end of the turn (SK-3).
pub use purlis_skills::runs;

use crate::harness::{Harness, Kit};

/// The variable a chat whose harness cannot load its skills is started with: the directory of
/// the built-in skills its set holds ([`SkillSet::built_in_dir`]), whose skills its `SessionStart` briefing then
/// lists ([`briefed`]). Set by the app for such a chat alone, so a harness that loads the skills
/// itself is never briefed on them twice.
pub const LISTED_ENV: &str = "PURLIS_SKILLS_DIR";

/// The variable a briefed chat is started with when its set holds the plane's skills (ADR 0099):
/// `1`, and its `SessionStart` briefing then lists them with the built-in ones. Set only where the
/// `skill-studio` flag let the plane's skills into the chat's set, so a briefing never reads the
/// plane where arming did not.
pub const PLANE_LISTED_ENV: &str = "PURLIS_SKILLS_PLANE";

/// The plane a chat's skills come from beside the built-in ones (ADR 0099), as a chat is armed.
#[derive(Debug, Clone, Copy)]
pub struct PlaneSkills<'a> {
    /// The plane's root.
    pub plane: &'a Path,
    /// The workspace the chat works in, or `None` for a chat at the plane root.
    pub workspace: Option<&'a str>,
    /// The folder of links to the plane's skills of the chat's set, which the host laid out
    /// before arming it ([`lay_out_for_chat`]), or `None` where it laid out none. A harness that
    /// needs the folder is then handed the built-in skills alone.
    pub linked: Option<&'a Path>,
}

/// The skill set of a chat on `harness` armed with `kit`, asked of [`purlis_skills::skill_set`].
///
/// The built-in skills come from the bundle in `kit`, and the plane's from `kit.skills`, where
/// the `skill-studio` flag let them in.
pub fn for_chat(kit: Kit<'_>, harness: Harness) -> SkillSet {
    let built_in = kit.plugin.and_then(in_bundle);
    ask(
        built_in.as_deref(),
        kit.skills,
        kit.persona.map(|persona| persona.persona),
        harness.skills(),
    )
}

/// The folder of links a harness on [`Route::Plugin`] or [`Route::Config`] is handed the plane's
/// skills of `set` by: the one the host laid out (`kit.skills`' `linked`), where the set holds a
/// plane skill. Nothing is read or written here: an adapter only turns the set into argv, env and
/// config (ADR 0063 as amended 2026-10-20).
pub fn linked<'a>(set: &SkillSet, kit: Kit<'a>) -> Option<&'a Path> {
    set.from_the_plane().next()?;
    kit.skills?.linked
}

/// **The host's part of arming a chat with the plane's skills**: the folder of links for the
/// plane's skills of the set of a chat in `plane`, at its root or in `workspace`, running as
/// `persona`, with the built-in skills in `built_in`, laid out under `sets`. `None` where the set
/// holds no plane skill, or the folder could not be laid out (said in the log: the chat then has
/// the built-in skills alone). The set is the one [`for_chat`] works out for that chat, whatever
/// its harness: the route changes how a skill is handed over, never which.
pub fn lay_out_for_chat(
    plane: &Path,
    workspace: Option<&str>,
    persona: Option<&str>,
    built_in: Option<&Path>,
    sets: &Path,
) -> Option<PathBuf> {
    let at = PlaneSkills {
        plane,
        workspace,
        linked: None,
    };
    let set = ask(built_in, Some(at), persona, Route::Plugin);
    match lay_out(&set, sets) {
        Ok(folder) => folder,
        Err(why) => {
            tracing::warn!(
                "purlis: the project's skills could not be laid out for this chat under {} \
                 ({why}); it has the built-in skills alone",
                sets.display()
            );
            None
        }
    }
}

/// The name of the plugin a set's folder is, in Claude Code: its skills are `skills:<name>`.
pub const LINKED_PLUGIN: &str = "skills";

/// **The folder of links for the plane's skills of `set`**, under `sets`, laid out if it is not
/// there yet, or `None` for a set with no plane skill (ADR 0063 as amended 2026-10-20).
///
/// `<sets>/<digest>/` holds `.claude-plugin/plugin.json`, naming the plugin [`LINKED_PLUGIN`],
/// and `skills/<name>`, a link to each skill's folder in the plane, so Claude Code loads it as a
/// plugin and opencode scans its `skills/`. `<digest>` is a SHA-256 over every name and the
/// folder it links to, so the folder of one set is never another's, and it is complete before
/// it takes that name: a running chat's folder never changes. One found complete is used as it
/// is; one that is not is made again.
pub fn lay_out(set: &SkillSet, sets: &Path) -> std::io::Result<Option<PathBuf>> {
    let links: Vec<(&str, &Path)> = set
        .from_the_plane()
        .filter_map(|effective| {
            let folder = effective.skill.path.parent()?;
            plain_name(&effective.skill.name).then_some((effective.skill.name.as_str(), folder))
        })
        .collect();
    if links.is_empty() {
        return Ok(None);
    }
    let digest = {
        use sha2::Digest;
        let mut hash = sha2::Sha256::new();
        for (name, folder) in &links {
            hash.update(name.as_bytes());
            hash.update([0]);
            hash.update(folder.as_os_str().as_encoded_bytes());
            hash.update([0]);
        }
        hash.finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    };
    let folder = sets.join(&digest);
    if laid_out(&folder, &links) {
        return Ok(Some(folder));
    }
    std::fs::create_dir_all(sets)?;
    let building = tempfile::Builder::new()
        .prefix(&format!(".{digest}."))
        .tempdir_in(sets)?;
    let manifest = building.path().join(".claude-plugin");
    std::fs::create_dir(&manifest)?;
    std::fs::write(manifest.join("plugin.json"), manifest_text())?;
    let skills = building.path().join(DIR_IN_BUNDLE);
    std::fs::create_dir(&skills)?;
    for (name, target) in &links {
        link(target, &skills.join(name))?;
    }
    if folder.exists() {
        // Not complete, so not a set's folder any chat was started with: made again.
        std::fs::remove_dir_all(&folder)?;
    }
    let built = building.keep();
    match std::fs::rename(&built, &folder) {
        Ok(()) => Ok(Some(folder)),
        // Another chat's start laid the same set out first.
        Err(_) if laid_out(&folder, &links) => {
            let _ = std::fs::remove_dir_all(&built);
            Ok(Some(folder))
        }
        Err(why) => {
            let _ = std::fs::remove_dir_all(&built);
            Err(why)
        }
    }
}

/// The plugin manifest a set's folder holds.
fn manifest_text() -> String {
    serde_json::json!({
        "name": LINKED_PLUGIN,
        "description": "This project's skills, as purlis hands them to this chat",
    })
    .to_string()
}

/// Whether `folder` is the complete folder of `links`: its manifest, and exactly one link per
/// skill, to its folder.
fn laid_out(folder: &Path, links: &[(&str, &Path)]) -> bool {
    let manifest = std::fs::read_to_string(folder.join(".claude-plugin/plugin.json"));
    if manifest.ok().as_deref() != Some(manifest_text().as_str()) {
        return false;
    }
    let skills = folder.join(DIR_IN_BUNDLE);
    let count = std::fs::read_dir(&skills).map(Iterator::count).unwrap_or(0);
    count == links.len()
        && links.iter().all(|(name, target)| {
            std::fs::read_link(skills.join(name)).ok().as_deref() == Some(*target)
        })
}

/// Whether `name` can name a link in a set's folder: one plain path component.
fn plain_name(name: &str) -> bool {
    let mut parts = Path::new(name).components();
    matches!(
        (parts.next(), parts.next()),
        (Some(std::path::Component::Normal(_)), None)
    ) && !name.contains(['/', '\\'])
}

#[cfg(unix)]
fn link(target: &Path, at: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, at)
}

#[cfg(windows)]
fn link(target: &Path, at: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_dir(target, at)
}

/// The briefing section that lists the skills of a chat started with [`LISTED_ENV`], or none
/// where it is unset or the set is empty.
///
/// The set is asked of [`purlis_skills::skill_set`] with what the chat was armed with — the
/// directory, the persona the chat runs as in the plane at `root`, and the briefing route,
/// the only one the variable is set for — so it lists what the chat's set holds, never every
/// folder in the directory.
pub fn briefed(
    root: &Path,
    workspace: Option<&str>,
    env: &dyn Fn(&str) -> Option<String>,
) -> Option<String> {
    let dir = env(LISTED_ENV).filter(|dir| !dir.is_empty())?;
    let persona = crate::envvar::lookup(crate::active::PERSONA_ENV, env).filter(|p| !p.is_empty());
    let plane = env(PLANE_LISTED_ENV).is_some_and(|on| on == "1");
    let set = ask(
        Some(Path::new(&dir)),
        plane.then_some(PlaneSkills {
            plane: root,
            workspace,
            linked: None,
        }),
        persona.as_deref(),
        Route::Briefing,
    );
    let skills: Vec<Skill> = set.effective.into_iter().map(|e| e.skill).collect();
    listing(&skills)
}

/// **The skills a briefed chat may invoke by reading their `SKILL.md`** (SK-3): every skill in
/// the directory it was started with in [`LISTED_ENV`], and the plane's at `root` where
/// [`PLANE_LISTED_ENV`] says its set holds them. Empty for a chat that was not briefed: a
/// harness that loads its skills itself invokes them by its own tool.
pub fn readable_by_a_briefed_chat(root: &Path, env: &dyn Fn(&str) -> Option<String>) -> Vec<Skill> {
    let Some(dir) = env(LISTED_ENV).filter(|dir| !dir.is_empty()) else {
        return Vec::new();
    };
    let mut skills = read(Path::new(&dir));
    if env(PLANE_LISTED_ENV).is_some_and(|on| on == "1") {
        skills.extend(read(&root.join(purlis_skills::PROJECT_DIR)));
    }
    skills
}

/// One ask of purlis-skills, from a directory of built-in skills, the plane with the workspace
/// the chat works in (where the plane's skills are let in), the persona a chat runs as, and the
/// chat's route.
fn ask(
    built_in: Option<&Path>,
    plane: Option<PlaneSkills<'_>>,
    persona: Option<&str>,
    route: Route,
) -> SkillSet {
    let kept = |folder: &Path| match plane {
        Some(at) => crate::profiles::kept_local(at.plane, folder),
        None => Err("there is no plane to ask git about".to_owned()),
    };
    let off = plane
        .and_then(|at| Some(turned_off(at.plane, at.workspace?)))
        .unwrap_or_default();
    purlis_skills::skill_set(
        plane.map(|at| at.plane),
        &Machine {
            built_in,
            kept_local: Some(&kept),
        },
        &Chat {
            workspace: plane.and_then(|at| at.workspace),
            persona,
            turned_off: &off,
            harness: HarnessSkills { route },
        },
    )
}

/// **The project skills the workspace `workspace` turned off** (#2767): `settings.skills.off`,
/// as the Workspace settings layer reads its `workspace.json`. Empty where it names none, or
/// holds something other than a list of names, which that layer's save refuses to write.
pub fn turned_off(plane: &Path, workspace: &str) -> Vec<String> {
    use crate::settings::workspace::{SKILLS, SKILLS_OFF};
    crate::settings::workspace::read(plane, workspace)
        .and_then(|table| {
            let names = table.get(SKILLS)?.get(SKILLS_OFF)?.as_array()?;
            names
                .iter()
                .map(|name| name.as_str().map(str::to_owned))
                .collect::<Option<Vec<_>>>()
        })
        .unwrap_or_default()
}

/// The folder of the project skill named `name` in the plane at `plane`: in its `skills/`, else
/// in its `skills.local/`, or `None` where neither holds one.
pub fn project_folder(plane: &Path, name: &str) -> Option<PathBuf> {
    [false, true]
        .into_iter()
        .filter_map(|local| level_dir(plane, None, local))
        .find_map(|dir| named(&dir, name))
}

/// **`purlis skill grant <name> <grant>`** (#2767): writes `grant` as where the project skill
/// `name` reaches, and says its folder. Refused, writing nothing, on a project purlis opens
/// read-only, for a name no project skill has, and for a workspace the plane does not have, so a
/// typo never grants a name nobody has.
pub fn grant(plane: &Path, name: &str, grant: &Grant) -> Result<PathBuf, String> {
    crate::settings::writable(plane)?;
    if let Grant::Only(names) = grant {
        let held = crate::workspaces::Plane::open(plane);
        for ws in names {
            if !held.workspace(ws).is_ok_and(|one| one.dir().is_dir()) {
                return Err(format!(
                    "this project has no workspace {ws:?}, so nothing was granted: \
                     `purlis workspace list` lists them"
                ));
            }
        }
    }
    let folder = project_folder(plane, name).ok_or_else(|| no_project_skill(name))?;
    set_grant(&folder, grant)?;
    Ok(folder)
}

/// **Turns the project skill `name` off (`on` false) or back on in `workspace`** (#2767): its
/// `settings.skills.off`, through the Workspace level's own save. A workspace can only take
/// away what a grant gave, so a skill whose grant does not reach `workspace` is refused either
/// way, saying the grant that would. `Ok(false)` where it already was so, and nothing is written.
pub fn switch(plane: &Path, workspace: &str, name: &str, on: bool) -> Result<bool, String> {
    let held = crate::workspaces::Plane::open(plane);
    if !held
        .workspace(workspace)
        .is_ok_and(|one| one.dir().is_dir())
    {
        return Err(format!(
            "this project has no workspace {workspace:?}: `purlis workspace list` lists them"
        ));
    }
    let folder = project_folder(plane, name).ok_or_else(|| no_project_skill(name))?;
    let reaches = grant_of(&folder).is_ok_and(|grant| grant.reaches(workspace));
    if !reaches {
        let what = if on {
            "cannot be turned on there: a workspace only turns off what the project granted it"
        } else {
            "has nothing to turn off there"
        };
        return Err(format!(
            "{name} is not granted to {workspace}, so it {what}. The project grants it at its \
             root: purlis skill grant {name} {workspace}"
        ));
    }
    let mut off = turned_off(plane, workspace);
    let was = off.iter().any(|one| one == name);
    if was != on {
        return Ok(false);
    }
    if on {
        off.retain(|one| one != name);
    } else {
        off.push(name.to_owned());
    }
    let read = crate::settings::workspace::read_file(plane, workspace)?;
    let base = read.exists.then_some(read.text.as_str());
    let path = vec![
        crate::settings::Step::Key(crate::settings::workspace::SKILLS.to_owned()),
        crate::settings::Step::Key(crate::settings::workspace::SKILLS_OFF.to_owned()),
    ];
    let value = (!off.is_empty()).then_some(crate::settings::Value::List(off));
    crate::settings::workspace::save(
        plane,
        workspace,
        base,
        &[crate::settings::Edit { path, value }],
    )
    .map_err(|why| why.join(" "))?;
    Ok(true)
}

fn no_project_skill(name: &str) -> String {
    format!(
        "no project skill is named {name:?}: a project skill is a folder in skills/ or \
         skills.local/ at the project's root, and `purlis skill list` lists them"
    )
}

/// **Share** (#2767): moves the local skill `name` of the plane root (`workspace` `None`) or of
/// `workspace` into that level's shared `skills/`, so the plane's save commits it. Refused on a
/// project purlis opens read-only, whose committed files it does not write.
pub fn share_here(plane: &Path, workspace: Option<&str>, name: &str) -> Result<PathBuf, String> {
    crate::settings::writable(plane)?;
    share(plane, workspace, name)
}

/// The flag the Skill studio's slices land behind until its finish line (#2762, SK-23), and is
/// then removed. It hides nothing yet.
pub const STUDIO_FLAG: &str = "skill-studio";

/// The variable that turns [`STUDIO_FLAG`] on (`1`) or off (`0`), over what this machine says.
/// Any other value is not read.
pub const STUDIO_ENV: &str = "PURLIS_SKILL_STUDIO";

/// Whether [`STUDIO_FLAG`] is on: what this machine's store says
/// ([`crate::machine::Flags::skill_studio`], off unless turned on, on every channel), and
/// [`STUDIO_ENV`] decides over it.
pub fn studio_on(flags: &crate::machine::Flags, env: &dyn Fn(&str) -> Option<String>) -> bool {
    match crate::envvar::lookup(STUDIO_ENV, env).as_deref() {
        Some("1") => true,
        Some("0") => false,
        _ => flags.skill_studio,
    }
}

/// **What the Skill studio and `purlis skill list` show** (SK-2): the skill set of a chat in the
/// plane at `plane`, at its root (`workspace` `None`) or in `workspace`, with the built-in skills
/// from `built_in`. Every skill with its level, and every one left out with its reason. Read as a
/// Claude Code chat's: the route is not what they show.
pub fn in_the_studio(built_in: Option<&Path>, plane: &Path, workspace: Option<&str>) -> SkillSet {
    let at = PlaneSkills {
        plane,
        workspace,
        linked: None,
    };
    ask(built_in, Some(at), None, Route::Plugin)
}

/// One skill as the Skill studio and `purlis skill list` and `show` show it: every copy of a name,
/// the one a chat gets and each left out with why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StudioRow {
    /// Its name, or its folder's where it names none.
    pub name: String,
    pub origin: Origin,
    /// Whether it is a local skill, kept on this machine in its level's `skills.local/` (#2767).
    pub local: bool,
    /// Where a project skill reaches (`root`, `all`, or the workspaces' names), or `None` for a
    /// skill of another level. A grant that cannot be read reaches the root alone, and says so
    /// in the left-out line of a workspace's chat.
    pub grant: Option<String>,
    /// Why a chat does not get it, in one line, or `None` where it does.
    pub left_out: Option<String>,
    /// What it does: its `description`, empty for a copy left out.
    pub description: String,
    /// Its folder, relative to the plane where it is inside it.
    pub folder: String,
}

/// **The studio's rows of `set`**, a set of a chat in the plane at `plane`: every skill it gets
/// and every one left out, by name, each folder relative to the plane. Of the copies of one name,
/// the one the chat gets comes first, then each copy left out from the widest level to the
/// narrowest, as the set lists them (#2767). The one list the window and the command line draw.
pub fn studio_rows(set: &SkillSet, plane: &Path) -> Vec<StudioRow> {
    let relative = |path: &Path| {
        path.strip_prefix(plane)
            .unwrap_or(path)
            .to_string_lossy()
            .into_owned()
    };
    let grant = |origin: Origin, folder: &Path| {
        (origin == Origin::Project).then(|| grant_of(folder).unwrap_or(Grant::Root).to_string())
    };
    let mut rows: Vec<StudioRow> = set
        .effective
        .iter()
        .map(|e| {
            let folder = e.skill.path.parent().unwrap_or(&e.skill.path);
            StudioRow {
                name: e.skill.name.clone(),
                origin: e.origin,
                local: e.local,
                grant: grant(e.origin, folder),
                left_out: None,
                description: e.skill.description.clone(),
                folder: relative(folder),
            }
        })
        .chain(set.left_out.iter().map(|l| StudioRow {
            name: l.name.clone(),
            origin: l.origin,
            local: l.local,
            grant: grant(l.origin, &l.folder),
            left_out: Some(l.reason.to_string()),
            description: String::new(),
            folder: relative(&l.folder),
        }))
        .collect();
    // Stable: the set already lists each name's left-out copies widest first.
    rows.sort_by(|a, b| {
        a.name
            .cmp(&b.name)
            .then(a.left_out.is_some().cmp(&b.left_out.is_some()))
    });
    rows
}

/// **The studio's runs** (SK-3): every run the plane at `plane`'s skill log holds, newest first,
/// of skill `skill` where one is named (by its name, with or without the namespace a harness
/// used it by), at most `limit`. The one list a skill's Runs panel and `purlis skill runs` draw.
pub fn studio_runs(plane: &Path, skill: Option<&str>, limit: usize) -> Vec<crate::skilluse::Run> {
    crate::skilluse::runs(plane)
        .into_iter()
        .filter(|run| skill.is_none_or(|name| run.is_of(name)))
        .take(limit)
        .collect()
}

/// The built-in skills' directory of the plugin installed beside this binary, where there is one
/// ([`crate::plugin_install::bundle_beside`]): what the Skill studio and `purlis skill` list,
/// found the one way in the window and on the command line.
pub fn built_in_here() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    crate::plugin_install::bundle_beside(&exe).and_then(|plugin| in_bundle(&plugin))
}

/// **Turns the Skill studio's flag on or off on this machine** (`flags.skill-studio` in
/// `machine.json`), as Settings › You › Previews does. [`STUDIO_ENV`] still decides over it
/// where it is set.
pub fn set_studio_on(config_root: &Path, on: bool) -> Result<(), String> {
    crate::machine::update(config_root, |store| store.flags.skill_studio = on)
        .map(|_| ())
        .map_err(|why| format!("purlis could not keep it, so nothing was changed: {why}"))
}

/// [`studio_on`] for this process: this machine's store, and this process's environment. Asked
/// at every chat's start, so it never holds the fence: a fenced test build reads a store outside
/// its fence as holding no flag, as [`crate::profilestore::listed_here`] does.
pub fn studio_on_here() -> bool {
    let flags = crate::machine::config_root_unheld()
        .filter(|config| {
            !crate::fence::FENCED || crate::fence::inside(config, &crate::fence::fence())
        })
        .map(|config| crate::machine::read(&config).store.flags)
        .unwrap_or_default();
    studio_on(&flags, &|name| std::env::var(name).ok())
}

#[cfg(test)]
mod tests;
