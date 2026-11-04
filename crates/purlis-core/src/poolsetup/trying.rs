// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Which profile Try it starts its chat on** (#4483, Q32): one whose harness reports its
//! session's start by a hook as it launches, since that hook is what Try it waits for (ADR 0076),
//! and one the pool's runner has signed in where the runner said.
//!
//! The project's default comes first, then the profiles in the picker's order. Where the runner
//! wasn't asked, or couldn't say, the start itself answers whether the profile is signed in.
//! Nothing here reads a file: the window hands in the profiles and how the runner stands.

use crate::profilestore::sign_in::Standing;

use super::{Profile, SignIns, StepId};

/// One of the project's profiles, as Try it weighs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub name: String,
    /// Its harness reports `SessionStart` by a hook as it launches, not only with its first
    /// message.
    pub reports_its_start: bool,
}

/// Why Try it starts no chat, and the step that mends it, where one does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotTried {
    pub why: String,
    pub step: Option<StepId>,
}

/// **The profile Try it starts its chat on pool `pool`**, or why there is none: the default
/// first, then each of `profiles` in order, among those whose harness reports its start.
pub fn profile_to_try(
    pool: &str,
    sign_ins: &SignIns,
    default: Option<&str>,
    profiles: &[Candidate],
) -> Result<String, NotTried> {
    let mut usable: Vec<&str> = profiles
        .iter()
        .filter(|one| one.reports_its_start)
        .map(|one| one.name.as_str())
        .collect();
    if let Some(at) = default.and_then(|name| usable.iter().position(|one| *one == name)) {
        let first = usable.remove(at);
        usable.insert(0, first);
    }
    if usable.is_empty() {
        return Err(NotTried {
            why: "None of this project's profiles runs a harness that says when its chat has \
                  started, so Try it can't see one start. Add a Claude Code profile in Settings, \
                  then try again."
                .to_owned(),
            step: None,
        });
    }
    let SignIns::Read(read) = sign_ins else {
        // Not asked, or it couldn't say: the start answers for the sign-in.
        return Ok(usable[0].to_owned());
    };
    let standing = |name: &str| {
        read.iter()
            .find(|(one, _)| one == name)
            .map(|(_, profile)| *profile)
    };
    let signed_in = |name: &str| standing(name) == Some(Profile::Standing(Standing::SignedIn));
    // A harness that couldn't say, or is still being fetched, starts a chat all the same.
    let may_start = |name: &str| {
        matches!(
            standing(name),
            Some(Profile::Standing(Standing::CannotTell | Standing::Fetching))
        )
    };
    if let Some(name) = usable
        .iter()
        .find(|name| signed_in(name))
        .or_else(|| usable.iter().find(|name| may_start(name)))
    {
        return Ok((*name).to_owned());
    }
    let any_signed_in = read
        .iter()
        .any(|(_, profile)| *profile == Profile::Standing(Standing::SignedIn));
    Err(NotTried {
        why: if any_signed_in {
            format!(
                "The profiles signed in on {pool} don't say when their chat has started, so Try \
                 it can't see one start. Sign in a Claude Code profile on the Sign-ins step, \
                 then try again."
            )
        } else {
            format!(
                "No profile is signed in on {pool} yet, so a chat there can't start. Sign one in \
                 on the Sign-ins step, then try again."
            )
        },
        step: Some(StepId::Signins),
    })
}

#[cfg(test)]
mod tests;
