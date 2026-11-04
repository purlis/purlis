// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Why a chat runs at its level** (#5207, ADR 0096 as amended 2026-11-05): the one answer the
//! window says beside the level and on a block's Notice, so neither the person nor the chat has
//! to guess. It decides nothing. The level is [`super::your_level`]'s, and each reason is read
//! by comparing what that seam answers with what its inputs say, so the rule is written once.

use std::path::Path;

use super::personal::{self, Choice};
use super::{Made, policy, team, your_level};

/// Where a person changes their level, as its labels read.
pub const WHERE: &str = "Settings › Project › Sandbox › Your level";

/// Why a start runs at the level it does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Why {
    /// The person's own level for the project.
    Chosen,
    /// The person set no level, so the project's own stands.
    NoneChosen,
    /// The person's row for this persona, which wins over the project's level.
    Persona(String),
    /// The file says this lower level, and it does not count until the person confirms it.
    WaitsForConfirm(Choice),
    /// A team's or an administrator's policy holds the full sandbox: its sentence, naming whose.
    Policy(String),
    /// A chat in the chain that asked for this one runs at a stricter level.
    AskedByStricter,
    /// Another chat asked for this one, and the record cannot name the whole chain.
    ChainUnread,
    /// A peer started it on the host, where no level of the person's is read.
    PeerStart,
}

impl Why {
    /// The reason in plain words, with where to change it, or whose it is where it is not the
    /// person's to change.
    pub fn said(&self) -> String {
        match self {
            Self::Chosen => {
                format!("This is the level you chose for this project. Change it in {WHERE}.")
            }
            Self::NoneChosen => format!(
                "You have not chosen a level here, so this chat takes the project's own. Choose \
                 one in {WHERE}."
            ),
            Self::Persona(name) => format!(
                "Your As {name} row sets this chat's level, over the project's. Change it in \
                 {WHERE}."
            ),
            Self::WaitsForConfirm(level) => format!(
                "Your level of {} waits for your Confirm, so it does not count yet. Confirm it \
                 in {WHERE}.",
                level.said()
            ),
            Self::Policy(said) => said.clone(),
            Self::AskedByStricter => format!(
                "A chat that asked for this one runs at a stricter level, and no chat goes below \
                 the chats that asked for it. Change their level in {WHERE}."
            ),
            Self::ChainUnread => "Another chat asked for this one and purlis cannot read the \
                                  whole chain of chats that asked, so it stays at the project's \
                                  own level or stricter. A chat you start in the window takes \
                                  your level."
                .to_owned(),
            Self::PeerStart => "A peer started this chat on its host, where your level is not \
                                read, so it takes the project's own. A chat you start in the \
                                window takes your level."
                .to_owned(),
        }
    }
}

/// The level a start would run at now, and why ([`next_start`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NextStart {
    pub level: Choice,
    pub why: Why,
}

impl NextStart {
    /// What is said of a chat running at `running`: why, and where its next start would differ,
    /// that a restart applies it.
    pub fn said(&self, running: Choice) -> String {
        let why = self.why.said();
        if running == self.level {
            return why;
        }
        format!(
            "This chat started with {}, and its next start has {}. Restart chat applies it. {why}",
            with(running),
            with(self.level)
        )
    }
}

/// `level` as the words after "with" and "has".
fn with(level: Choice) -> &'static str {
    match level {
        Choice::On => "the full sandbox",
        Choice::Floor => "the floor",
        Choice::Off => "no sandbox",
    }
}

/// **The level a start of a chat on `persona`, made `made`, would run at now, and why** (#5207).
/// In order: a policy that holds the level, how the start was made where that moved it from
/// what a start in the window takes, a lower level still to confirm, the persona's row, then
/// the project's level or none.
pub fn next_start(
    root: &Path,
    persona: Option<&str>,
    locks: &policy::Locks,
    made: &Made,
) -> NextStart {
    let base = personal::base(root);
    let read = |made: &Made| your_level(root, persona, locks, made).unwrap_or(base);
    // A start that names no way it was made takes the project's own, as `decide` takes it.
    let level = if made.via().is_some() {
        read(made)
    } else {
        base
    };
    let why = why_of(root, persona, locks, made, level, read(&Made::Window));
    NextStart { level, why }
}

fn why_of(
    root: &Path,
    persona: Option<&str>,
    locks: &policy::Locks,
    made: &Made,
    level: Choice,
    in_the_window: Choice,
) -> Why {
    if let Some(said) = team::allowed_levels_under(root, locks).why {
        return Why::Policy(said);
    }
    if level != in_the_window {
        match made {
            Made::Unnamed => return Why::PeerStart,
            Made::Asked {
                above: Some(above), ..
            } if !above.is_empty() => return Why::AskedByStricter,
            Made::Asked { .. } => return Why::ChainUnread,
            Made::Window | Made::Resume | Made::Relaunch => {}
        }
    }
    let honoured = personal::in_force(root);
    if let Some(said) = personal::of(root).for_persona(persona)
        && Some(said) != honoured.for_persona(persona)
    {
        return Why::WaitsForConfirm(said);
    }
    match persona.filter(|_| honoured.at(persona).is_some()) {
        Some(name) => Why::Persona(name.to_owned()),
        None if honoured.level.is_some() => Why::Chosen,
        None => Why::NoneChosen,
    }
}

#[cfg(test)]
mod tests;
