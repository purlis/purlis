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
    Chat, DIR_IN_BUNDLE, Effective, HarnessSkills, LeftOut, Machine, Missing, Origin, Reason,
    Route, Skill, SkillSet, in_bundle, listing, read,
};

use crate::harness::{Harness, Kit};

/// The variable a chat whose harness cannot load its skills is started with: the directory of
/// the skills its set holds ([`SkillSet::dir`]), whose skills its `SessionStart` briefing then
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
    /// Where a set's folder of links is laid out ([`lay_out`]): `<app data>/skill-sets`, where
    /// purlis has a data folder. Without it, a harness that needs the folder is handed the
    /// built-in skills alone.
    pub sets: Option<&'a Path>,
}

/// The skill set of a chat on `harness` armed with `kit`, asked of [`purlis_skills::skill_set`].
///
/// The built-in skills come from the bundle in `kit`, and the plane's from `kit.skills`, where
/// the `skill-studio` flag let them in.
pub fn for_chat(kit: Kit<'_>, harness: Harness) -> SkillSet {
    let built_in = kit.plugin.and_then(in_bundle);
    ask(
        built_in.as_deref(),
        kit.skills.map(|skills| (skills.plane, skills.workspace)),
        kit.persona.map(|persona| persona.persona),
        harness.skills(),
    )
}

/// The folder of links a harness on [`Route::Plugin`] or [`Route::Config`] is handed the plane's
/// skills of `set` by, laid out under `kit.skills`' `sets`, or `None` where the set holds none or
/// the folder could not be laid out (said in the log: the chat then has the built-in skills
/// alone).
pub fn linked(set: &SkillSet, kit: Kit<'_>) -> Option<PathBuf> {
    let sets = kit.skills?.sets?;
    match lay_out(set, sets) {
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
        plane.then_some((root, workspace)),
        persona.as_deref(),
        Route::Briefing,
    );
    let skills: Vec<Skill> = set.effective.into_iter().map(|e| e.skill).collect();
    listing(&skills)
}

/// One ask of purlis-skills, from a directory of built-in skills, the plane with the workspace
/// the chat works in (where the plane's skills are let in), the persona a chat runs as, and the
/// chat's route.
fn ask(
    built_in: Option<&Path>,
    plane: Option<(&Path, Option<&str>)>,
    persona: Option<&str>,
    route: Route,
) -> SkillSet {
    purlis_skills::skill_set(
        plane.map(|(root, _)| root),
        &Machine { built_in },
        &Chat {
            workspace: plane.and_then(|(_, workspace)| workspace),
            persona,
            harness: HarnessSkills { route },
        },
    )
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
    ask(built_in, Some((plane, workspace)), None, Route::Plugin)
}

/// The built-in skills' directory of the plugin installed beside this binary, where there is one
/// ([`crate::plugin_install::bundle_beside`]).
pub fn built_in_here() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    crate::plugin_install::bundle_beside(&exe).and_then(|plugin| in_bundle(&plugin))
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
