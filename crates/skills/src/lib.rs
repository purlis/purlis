// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What a chat's skill set is, and why (SK-1, #2764; spec #2762).
//!
//! This crate is the one place purlis works skills out. Its main question is [`skill_set`]:
//! given the plane, the machine and a chat (its workspace, persona and harness), which skills
//! does the chat get, by which [`Route`] and from which [`Origin`], and which are left out, each
//! with a one-line [`Reason`]. A harness adapter asks it and turns the answer into argv, env and
//! config for its harness. It never reads a skill folder itself (ADR 0063).
//!
//! **Built-in skills only, for now.** A built-in skill ships inside purlis: one directory,
//! `skills/` inside the bundled plugin, with a folder per skill holding its `SKILL.md`. That
//! directory is the one source: every harness is handed the same files, and none gets a copy.
//! The plane's levels (project, workspace, persona and local skills), harness skills and repo
//! skills arrive in later slices behind the `skill-studio` flag; the interface already takes
//! the plane and the chat's workspace and persona, so those slices add answers and no
//! arguments.
//!
//! **Harness-agnostic.** The crate knows no harness by name. A chat's harness is described by
//! what it can do with skills ([`HarnessSkills`]), which purlis-core's harness declaration
//! says, so a new harness needs no change here.

use std::path::{Path, PathBuf};

mod read;

pub use read::{DIR_IN_BUNDLE, LISTED_ENV, frontmatter, in_bundle, listed_from, listing, read};

/// A skill, as a chat is told about it: a folder in the open SKILL.md format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skill {
    /// The frontmatter's `name`.
    pub name: String,
    /// The frontmatter's `description`: what the skill does and when to use it.
    pub description: String,
    /// Its `SKILL.md`, which the model reads when the skill applies.
    pub path: PathBuf,
}

/// How a harness's chat comes to know its skills (ADR 0063).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// The harness loads the bundled plugin, `skills/` included, for the chat alone.
    Plugin,
    /// The harness is told the skills directory for the chat alone, and discovers the skills in
    /// it as its own.
    Config,
    /// The harness has no way to take a skills directory for one chat, so the chat's
    /// `SessionStart` briefing lists each skill with the path to its `SKILL.md`.
    Briefing,
}

/// Where a skill in a chat's set comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Origin {
    /// It ships inside purlis, in the bundled plugin's `skills/`. It cannot be turned off.
    BuiltIn,
}

/// What a chat's harness can do with skills: all this crate needs to know about it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HarnessSkills {
    /// How its chats are handed their skills.
    pub route: Route,
}

/// The chat a skill set is worked out for.
#[derive(Debug, Clone, Copy)]
pub struct Chat<'a> {
    /// The workspace it works in, where purlis knows it.
    pub workspace: Option<&'a str>,
    /// The persona it runs as, or `None` for a chat on no persona.
    pub persona: Option<&'a str>,
    /// Its harness, by what it can do with skills.
    pub harness: HarnessSkills,
}

/// What this machine holds that a chat's skills can come from.
#[derive(Debug, Clone, Copy, Default)]
pub struct Machine<'a> {
    /// The bundled plugin directory the app ships, whose `skills/` holds the built-in skills,
    /// where the app found it.
    pub bundle: Option<&'a Path>,
}

/// One skill a chat gets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Effective {
    pub skill: Skill,
    /// How the chat's harness is handed it.
    pub route: Route,
    pub origin: Origin,
}

/// One skill a chat does not get, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeftOut {
    /// Its folder's name: a folder left out may name no skill.
    pub name: String,
    pub origin: Origin,
    /// Its folder.
    pub folder: PathBuf,
    pub reason: Reason,
}

/// Why a skill is left out of a chat's set. Its [`std::fmt::Display`] is the one line a person
/// is shown.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Reason {
    /// The folder is not a whole skill, so no harness would load it.
    NotASkill(Missing),
}

/// What a folder lacks to be a whole skill.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Missing {
    /// It holds no `SKILL.md`, or one that cannot be read as text.
    File,
    /// Its `SKILL.md`'s frontmatter names no `name`.
    Name,
    /// Its `SKILL.md`'s frontmatter names no `description`.
    Description,
}

impl std::fmt::Display for Reason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotASkill(Missing::File) => {
                f.write_str("not a skill: its folder has no SKILL.md")
            }
            Self::NotASkill(Missing::Name) => {
                f.write_str("not a skill: its SKILL.md names no `name`")
            }
            Self::NotASkill(Missing::Description) => {
                f.write_str("not a skill: its SKILL.md names no `description`")
            }
        }
    }
}

/// A chat's skill set: what it gets, and what it does not and why.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SkillSet {
    /// Every skill the chat gets, by name.
    pub effective: Vec<Effective>,
    /// Every skill left out, by name.
    pub left_out: Vec<LeftOut>,
}

impl SkillSet {
    /// The directories the effective skills sit in, each once, in the order of the skills: what
    /// a harness on [`Route::Config`] or [`Route::Briefing`] is told. A skill's directory is the
    /// one holding its folder.
    pub fn dirs(&self) -> Vec<&Path> {
        let mut dirs: Vec<&Path> = Vec::new();
        for effective in &self.effective {
            let Some(dir) = effective.skill.path.parent().and_then(Path::parent) else {
                continue;
            };
            if !dirs.contains(&dir) {
                dirs.push(dir);
            }
        }
        dirs
    }
}

/// What `chat`'s skill set is, and why, on `machine` in the plane at `plane` (`None` for a chat
/// purlis knows no plane for).
///
/// Today every built-in skill reaches every chat by its harness's route, and a built-in folder
/// that is not a whole skill is left out with the reason. The plane and the chat's workspace and
/// persona decide nothing yet: the plane's levels read them (SK-2 on).
pub fn skill_set(plane: Option<&Path>, machine: &Machine<'_>, chat: &Chat<'_>) -> SkillSet {
    let _read_from_the_plane_levels_on = (plane, chat.workspace, chat.persona);
    let Some(dir) = machine.bundle.and_then(in_bundle) else {
        return SkillSet::default();
    };
    let mut set = SkillSet::default();
    for folder in read::scan(&dir) {
        match folder {
            Ok(skill) => set.effective.push(Effective {
                skill,
                route: chat.harness.route,
                origin: Origin::BuiltIn,
            }),
            Err((folder, missing)) => set.left_out.push(LeftOut {
                name: folder
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                origin: Origin::BuiltIn,
                folder,
                reason: Reason::NotASkill(missing),
            }),
        }
    }
    set.effective
        .sort_by(|a, b| a.skill.name.cmp(&b.skill.name));
    set.left_out.sort_by(|a, b| a.name.cmp(&b.name));
    set
}
