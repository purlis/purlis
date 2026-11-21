// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The deny state, acted on** (M75 slice 07b, #7016; spec §4.6, §6.1; Q320b R2-3; review
//! rounds 3 to 5).
//!
//! Slice 07a decides which state a plane is in ([`crate::secrets::answers::standing`]); this is
//! what the deny state does:
//!
//! - **Every action but `start` is refused** ([`refused`]), naming the deny state, before any rule
//!   is read: the steps after it in spec §6 are not reached. The chat still starts, and **`start`
//!   is still judged** by the rules, so a forbid on it still refuses the chat: the deny state only
//!   adds refusals ([`decide::in_plane`]).
//! - **The chat runs at `full` with only the minimum permits** its harness needs
//!   ([`MINIMUM_PERMITS`], [`crate::sandbox::for_start_in_deny_state`]).
//! - **A re-press is raised** ([`RePress`]): nothing is allowed until the person presses. After a
//!   lost store it diffs against the shipped default, which holds no nevers and no accepted floor,
//!   and says how many of each were lost, the Everywhere nevers counted apart.
//!
//! **A chat never clears a deny state or presses for the person.** The press is a value only the
//! window's command layer makes ([`crate::secrets::answers::Press`]); nothing here takes one.

use crate::secrets::answers::{Deny, Standing, State};

use super::decide::{Deciding, Decision, Outcome};
use super::{Level, family_of};
use crate::enforcement::Mode;

/// The rule a deny-state refusal names, in place of a rule of any file: the refusal is the
/// plane's state, not a policy's.
pub const RULE_ID: &str = "answers-deny-state";

/// What the harness needs to run in a deny state, and nothing else (review round 3, SEC3-SF4;
/// **proposed**, until slice 22 compiles the sandbox from the policy). Everything else a project,
/// a persona or the person granted is left out: the chat is compiled at `full` with these.
pub const MINIMUM_PERMITS: [Permit; 3] = [
    Permit::ModelProviders,
    Permit::HarnessOwnFolders,
    Permit::ChatScratch,
];

/// One permit of the deny state's sandbox.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permit {
    /// The model providers' hosts ([`crate::sandbox::Preset::ModelProviders`]): the harness cannot
    /// answer without them. Not the forge, not the toolchains, no host of the project's own.
    ModelProviders,
    /// The harness's own program and config folders, which the full sandbox already lets it read.
    HarnessOwnFolders,
    /// The chat's own scratch folder, which the full sandbox already lets it write.
    ChatScratch,
}

/// **What a chat in the deny state is told**, and what its decision carries.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DenyState {
    /// Each cause, in words.
    pub causes: Vec<String>,
    /// The re-press, as the window shows it ([`RePress::said`]).
    pub re_press: String,
}

/// **The re-press** (review round 3, SEC3-SF1; review round 4, SEC4 nit; review round 5, SEC5
/// nit): raised while a plane is in the deny state, and the one way out. It diffs against the
/// shipped default, which holds no nevers and no accepted floor, so what was lost is what the
/// anchors counted that the store no longer holds.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RePress {
    /// This plane's nevers lost, from the anchor's count.
    pub nevers: u64,
    /// This plane's accepted-floor versions lost.
    pub floors: u64,
    /// The Everywhere nevers lost, from the Everywhere anchor's count (review round 5).
    pub everywhere_nevers: u64,
    /// Whether a count could be made at all: false where the cause does not say what was lost
    /// (a locked key, a moved plane), so the window never invents a number.
    pub counted: bool,
}

impl RePress {
    /// **The re-press a plane's standing raises**, none where it is not in the deny state.
    pub fn of(standing: &Standing) -> Option<Self> {
        let State::Deny(causes) = &standing.state else {
            return None;
        };
        let mut raised = Self::default();
        for cause in causes {
            match cause {
                Deny::ManifestMissing(lost) | Deny::ManifestOlder(lost) => {
                    raised.nevers = raised.nevers.max(lost.nevers);
                    raised.floors = raised.floors.max(lost.floors);
                    raised.counted = true;
                }
                Deny::EverywhereOlder(lost) => {
                    raised.everywhere_nevers = raised.everywhere_nevers.max(lost.nevers);
                    raised.counted = true;
                }
                _ => {}
            }
        }
        Some(raised)
    }

    /// **The diff against the shipped default, in the window's words**: what was lost, how many,
    /// and what the press does. Sentence case, what happened and what to do (`docs/ui-copy.md`).
    pub fn said(&self) -> String {
        let mut lost = String::new();
        if self.counted {
            let nevers = plural(self.nevers, "never");
            let floors = plural(self.floors, "accepted version");
            lost = format!(" Lost: {nevers} and {floors} in this project");
            if self.everywhere_nevers > 0 {
                lost.push_str(&format!(
                    ", and {} that apply everywhere",
                    plural(self.everywhere_nevers, "never")
                ));
            }
            lost.push('.');
        }
        format!(
            "Purlis can no longer vouch for what you answered here, so nothing is allowed in this \
             project until you press to start again.{lost} The shipped default holds no nevers and \
             no accepted floor: after the press, only the shipped default and the project's own \
             files apply, and what you answered before has to be answered again."
        )
    }
}

fn plural(n: u64, word: &str) -> String {
    if n == 1 {
        format!("1 {word}")
    } else {
        format!("{n} {word}s")
    }
}

impl DenyState {
    /// **What `standing` says**, none where the plane is not in the deny state.
    pub fn of(standing: &Standing) -> Option<Self> {
        let re_press = RePress::of(standing)?;
        Some(Self {
            causes: standing.causes().iter().map(Deny::said).collect(),
            re_press: re_press.said(),
        })
    }

    /// **The refusal's sentence**: the deny state, named, and what ends it.
    pub fn said(&self) -> String {
        format!(
            "This project's sealed answers are in the deny state ({}), so purlis refuses everything \
             but starting a chat. {}",
            self.causes.join("; "),
            self.re_press
        )
    }
}

/// **The refusal of `action` in the deny state**: a deny naming the deny state, in place of every
/// rule, and `None` for `start`, which the rules still judge (review round 5, SEC5-SF2) and which
/// a forbid on still refuses. Where the plane is not in the deny state, none.
pub fn refused(action: &str, standing: &Standing) -> Option<Decision> {
    if action == "start" {
        return None;
    }
    let state = DenyState::of(standing)?;
    let family = family_of(action).unwrap_or("").to_owned();
    Some(Decision {
        outcome: Outcome::Deny,
        deciding: vec![Deciding {
            id: RULE_ID.to_owned(),
            level: Level::You.word().to_owned(),
            owner: "your sealed answers".to_owned(),
            file: String::new(),
            line: 0,
            off: false,
        }],
        mode_family: family.clone(),
        family,
        // No mode lifts it, and no lock is needed to hold it.
        mode: Mode::Enforce,
        after_mode: Outcome::Deny,
        lifted: Vec::new(),
        asked: Vec::new(),
        lock: None,
        grant: None,
        unasked: None,
        keep: None,
        timed: None,
        deny_state: Some(state),
    })
}

#[cfg(test)]
#[path = "deny_state_tests.rs"]
mod tests;
