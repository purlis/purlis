// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A chat's skills, as purlis-core asks [`purlis_skills`] for them (ADR 0063, SK-1).
//!
//! How a chat's skill set is worked out is `purlis-skills`' alone: which skills a chat gets, by
//! which [`Route`] and from which [`Origin`], and why each left-out one is left out. This module
//! hands that crate what purlis-core knows about a chat ([`for_chat`]) and holds the
//! `skill-studio` flag ([`studio_shown`]). A harness adapter asks [`for_chat`] and turns the
//! answer into its harness's argv, env and config; the route each harness takes is its
//! declaration's fact, [`crate::harness::Harness::skills`].

pub use purlis_skills::{
    Chat, DIR_IN_BUNDLE, Effective, HarnessSkills, LISTED_ENV, LeftOut, Machine, Missing, Origin,
    Reason, Route, Skill, SkillSet, in_bundle, listed_from, listing, read,
};

use crate::harness::{Harness, Kit};

/// The skill set of a chat on `harness` armed with `kit`, asked of [`purlis_skills::skill_set`].
///
/// The built-in skills come from the bundle in `kit`. The plane is the persona's project where
/// the chat runs as one; the workspace is not known where a chat is armed, and nothing reads it
/// until the plane's levels arrive.
pub fn for_chat(kit: Kit<'_>, harness: Harness) -> SkillSet {
    purlis_skills::skill_set(
        kit.persona.map(|persona| persona.root),
        &Machine { bundle: kit.plugin },
        &Chat {
            workspace: None,
            persona: kit.persona.map(|persona| persona.persona),
            harness: HarnessSkills {
                route: harness.skills(),
            },
        },
    )
}

/// The flag the Skill studio's slices land behind until its finish line (#2762, SK-23), and is
/// then removed. It hides nothing yet.
pub const STUDIO_FLAG: &str = "skill-studio";

/// The variable that turns [`STUDIO_FLAG`] on (`1`) or off (`0`) on any channel. Any other value
/// is not read.
pub const STUDIO_ENV: &str = "PURLIS_SKILL_STUDIO";

/// Whether [`STUDIO_FLAG`] is on: off by default, on for a machine that takes the dev channel,
/// and [`STUDIO_ENV`] decides over either. The binary is the same on both channels (ADR 0092):
/// the flag follows the channel this machine chose ([`crate::machine::Store::channel`]), never
/// how the build was made.
pub fn studio_shown(
    channel: crate::updates::Channel,
    env: &dyn Fn(&str) -> Option<String>,
) -> bool {
    match crate::envvar::lookup(STUDIO_ENV, env).as_deref() {
        Some("1") => true,
        Some("0") => false,
        _ => channel == crate::updates::Channel::Dev,
    }
}

#[cfg(test)]
#[path = "skills_tests.rs"]
mod tests;
