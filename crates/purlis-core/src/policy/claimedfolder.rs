// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A branch folder another chat claimed, as the policy decides it** (M75-25, #7034; ADR 0125,
//! amended 2026-11-27, §2 and §5; spec §6 step 1).
//!
//! Two things guard a chat's write of a branch folder, and they are kept apart:
//!
//! - **The one-writer mutex** ([`crate::piececlaim`]): a second live chat is refused while
//!   another holds the folder. It is **protocol**, not a chat decision (spec §6 step 1): it has no
//!   action and no rule, and it holds in every mode, `guards.git = "off"` included.
//! - **`guards-git-claimed-folder`** ([`RULE`]): a rule of the shipped default, decided by the one
//!   decision function under `guards.git`'s mode. It forbids `commit`, `update` and `discard`
//!   where `context.owner` is `task`, `asker` or `other`: the folder's **first claim** is another
//!   chat's, by an owner record only the app seals ([`super::owner`]). Where no record names an
//!   owner (`"none"`) the act is decided as it was before this rule.
//!
//! [`judge`] decides first, and the mutex is taken only once the policy lets the act go, so a
//! refused chat never comes to hold a folder it may not write. The first chat whose act goes
//! there is recorded as the folder's owner ([`Cleared::record`]), after it has taken the mutex.
//!
//! **Only the session host decides**: every caller holds the app's record of the asking chat
//! ([`decide::ChatFacts`]). A process that is not the app has none and asks nothing here, and it
//! could not verify an owner record anyway ([`super::owner`]).

use std::path::{Path, PathBuf};

use crate::enforcement::{self, Family, Mode};
use crate::piececlaim::Act;
use crate::worktree::Located;

use super::decide::{self, ChatFacts, Decision, GitFacts, Outcome, Sealed};
use super::owner::{self, Owned, Owner, Touched};

/// The rule's `@id`, as the shipped default writes it.
pub const RULE: &str = "guards-git-claimed-folder";

/// **The chat that asks**, as the session host records it.
#[derive(Debug, Clone, Copy)]
pub struct Asking<'a> {
    /// The app's number for the chat.
    pub chat: u32,
    /// Its stable id, which an owner record names it by. `None`: it owns nothing, and every folder
    /// another chat owns reads `"none"` to it ([`owner::of_request`]).
    pub id: Option<&'a str>,
    /// What the app records of it.
    pub facts: &'a ChatFacts,
}

/// **The action a branch folder's act is** in the schema, or `None` for one this rule does not
/// decide: a push reads its forge branch's owner, not its folder's (`guards-git-push-not-own`),
/// and a plain write of a file is no git act.
pub fn action_of(act: Act) -> Option<&'static str> {
    match act {
        Act::Commit => Some("commit"),
        Act::Update => Some("update"),
        Act::Remove => Some("discard"),
        Act::Push | Act::Write => None,
    }
}

/// **The folder an owner record keys**: the plane's real root, then the folder's place by its
/// names. Never the folder's own path resolved, so a link put where the folder was keys the same
/// record and reads the same owner.
pub fn folder_of(root: &Path, at: &Located) -> PathBuf {
    std::fs::canonicalize(root)
        .unwrap_or_else(|_| root.to_path_buf())
        .join("workspaces")
        .join(&at.workspace)
        .join(crate::worktree::DIR_NAME)
        .join(&at.repo)
        .join(&at.piece)
}

/// The `Branch` the act is judged on: the folder's branch, named by its place
/// (`<workspace>/<repo>#<folder>`), since its forge repo is not read here.
fn resource_of(at: &Located) -> String {
    format!("{}/{}#{}", at.workspace, at.repo, at.piece)
}

/// **What the policy let go**: the record to make once the mutex is taken.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use = "the first claim is recorded only once the mutex is taken"]
pub struct Cleared {
    /// The folder and the chat to record as its owner, where no record names one yet.
    first: Option<(PathBuf, String)>,
}

impl Cleared {
    /// Nothing decided and nothing to record: an act this rule does not decide, or a process
    /// that is not the session host.
    pub fn nothing() -> Self {
        Self { first: None }
    }

    /// **Records the asking chat as the folder's owner**, where no record named one: after the
    /// mutex was taken, so only the chat that holds the folder is recorded. A record that cannot be
    /// made (this process cannot seal, the plane is in the deny state, or an owner record is lost)
    /// leaves the folder no one's, which is how it was decided: nothing is loosened by it.
    pub fn record(self, root: &Path) {
        if let Some((folder, chat)) = self.first
            && let Err(why) = owner::record_owner(root, &Owned::Folder(&folder), &chat)
        {
            tracing::debug!("purlis: a branch folder's first claim was not recorded: {why}");
        }
    }
}

/// **Whether `asking` may `act` in the branch folder `at`**, by the policy in force (the project
/// at `HEAD`, what the person accepted, the machine's locks): `Ok` where it goes, with the first
/// claim to record; else the refusal, naming each deciding rule's `@id`, level and owner. An ask
/// is refused too: nothing holds a branch folder's act for the person's answer, so it never runs
/// unasked. A rule `log` lifted is written to the event log. Every refusal of the decision, the
/// deny state and a request that does not fit the schema included, is a refusal here.
pub fn judge(
    root: &Path,
    asking: &Asking<'_>,
    at: &Located,
    act: Act,
    git: GitFacts,
) -> Result<Cleared, String> {
    let Some(action) = action_of(act) else {
        return Ok(Cleared::nothing());
    };
    let folder = folder_of(root, at);
    let sealed = Sealed {
        chat: asking.id.map(str::to_owned),
        touched: Some(Touched::from(Owned::Folder(&folder))),
        ..Sealed::of(root)
    };
    let owner = sealed.owner();
    let decision = decide::for_chats_git(
        root,
        asking.chat,
        asking.facts,
        git,
        &enforcement::in_force(root),
        &sealed,
        action,
        ("Branch", &resource_of(at)),
    );
    match decision.after_mode {
        Outcome::Allow => {
            // Under `log`, each forbid the mode lifted is a would-have-blocked line; `off` writes
            // none (spec §6.4).
            if decision.would_have_blocked() && decision.mode == Mode::Log {
                let chat = asking.id.unwrap_or(enforcement::UNKNOWN_CHAT);
                for rule in &decision.deciding {
                    enforcement::record(root, chat, Family::GuardsGit, &rule.id, None);
                }
            }
            let first = asking
                .id
                .filter(|id| !id.is_empty() && owner == Owner::Nobody)
                .map(|id| (folder, id.to_owned()));
            Ok(Cleared { first })
        }
        Outcome::Ask | Outcome::Deny => Err(refusal(at, action, &decision)),
    }
}

/// What the chat is told of a refused act.
fn refusal(at: &Located, action: &str, decision: &Decision) -> String {
    let folder = format!(
        "{}'s branch folder '{}'",
        crate::shown::readable(&at.repo, 60),
        crate::shown::readable(&at.piece, 60)
    );
    let verb = match action {
        "commit" => "commit in",
        "update" => "update",
        _ => "discard work in",
    };
    let decided = if decision.outcome == Outcome::Ask && decision.after_mode == Outcome::Ask {
        "asks the person first, and nothing holds this for an answer, so it is refused"
    } else {
        "refuses it"
    };
    let what_to_do = if decision.ids().contains(&RULE) {
        "Another chat claimed this folder first, so it is that chat's to write. Work in a branch \
         folder of your own (`purlis worktree add`), or ask the person to hand this one over."
    } else {
        "Ask the person whether it should be done."
    };
    format!(
        "purlis's project policy will not {verb} {folder}: {} {decided}. Nothing was changed. \
         {what_to_do}",
        decision.rules_said()
    )
}

#[cfg(test)]
#[path = "claimedfolder_tests.rs"]
mod tests;
