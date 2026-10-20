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

use std::path::Path;

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

/// The skill set of a chat on `harness` armed with `kit`, asked of [`purlis_skills::skill_set`].
///
/// The built-in skills come from the bundle in `kit`. The plane is the persona's project where
/// the chat runs as one; the workspace is not known where a chat is armed, and nothing reads it
/// until the plane's levels arrive (SK-2, which widens `Kit` for it).
pub fn for_chat(kit: Kit<'_>, harness: Harness) -> SkillSet {
    let built_in = kit.plugin.and_then(in_bundle);
    ask(
        built_in.as_deref(),
        kit.persona.map(|persona| (persona.root, persona.persona)),
        harness.skills(),
    )
}

/// The briefing section that lists the skills of a chat started with [`LISTED_ENV`], or none
/// where it is unset or the set is empty.
///
/// The set is asked of [`purlis_skills::skill_set`] with what the chat was armed with — the
/// directory, the persona the chat runs as in the plane at `root`, and the briefing route,
/// the only one the variable is set for — so it lists what the chat's set holds, never every
/// folder in the directory.
pub fn briefed(root: &Path, env: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    let dir = env(LISTED_ENV).filter(|dir| !dir.is_empty())?;
    let persona = crate::envvar::lookup(crate::active::PERSONA_ENV, env).filter(|p| !p.is_empty());
    let set = ask(
        Some(Path::new(&dir)),
        persona.as_deref().map(|persona| (root, persona)),
        Route::Briefing,
    );
    let skills: Vec<Skill> = set.effective.into_iter().map(|e| e.skill).collect();
    listing(&skills)
}

/// One ask of purlis-skills, from a directory of built-in skills, the persona a chat runs as
/// with the plane that defines it, and the chat's route.
fn ask(built_in: Option<&Path>, persona: Option<(&Path, &str)>, route: Route) -> SkillSet {
    purlis_skills::skill_set(
        persona.map(|(root, _)| root),
        &Machine { built_in },
        &Chat {
            workspace: None,
            persona: persona.map(|(_, name)| name),
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

#[cfg(test)]
mod tests;
