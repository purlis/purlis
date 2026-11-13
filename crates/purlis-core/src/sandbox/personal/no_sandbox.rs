// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **No sandbox at setup** (T1-25; Q156, Q163): what choosing it in setup's Protection row
//! does beyond writing the level, said before the press and done in the same press.
//!
//! - **The report** ([`report`], R-066): the plain-file vaults this machine registers for the
//!   project, which move into the system keychain before the level is written, and the key of
//!   the warning every No sandbox chat carries (Q141). The window draws both in the row.
//! - **The move, in the step that writes the level** ([`moved_before_the_level`], R-293): each
//!   vault moves whole or not at all ([`crate::secrets::tokeyring`]), and a move that stops
//!   leaves the level unwritten.
//! - **The refusal** ([`TeamVault`], R-298): a plain-file vault the team's committed
//!   `vaults.json` registers is the team's to move, and no machine may turn its sandbox off over
//!   a file every chat could then read. What refuses is that the committed file registers it;
//!   the owner it names (the author of the commit that added its entry, Q250) is only said, and
//!   falls back to "your team" wherever git cannot say.

use std::time::Duration;

use crate::secrets::{Ctx, tokeyring};

/// The key of the warning a No sandbox chat carries (Q62, Q141): the window's words for it.
pub const WARNING: &str = "setup.no-sandbox.runs-as-you";

/// Who the refusal names where git cannot say who added the vault's entry.
pub const YOUR_TEAM: &str = "your team";

/// What choosing No sandbox at setup would do for a project.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Report {
    /// The plain-file vaults that move into the keychain in the press that writes the level.
    pub moves: Vec<String>,
    /// The key of the warning to show beside them ([`WARNING`]).
    pub warning: &'static str,
    /// The team's vault that refuses No sandbox here, where there is one.
    pub refused: Option<TeamVault>,
}

/// A plain-file vault the team's committed file registers, which refuses No sandbox.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TeamVault {
    /// The vault's name.
    pub vault: String,
    /// Who added it, as inert text: a name from git, or [`YOUR_TEAM`].
    pub owner: String,
}

impl TeamVault {
    /// The refusal, as the window says it: the vault, who registered it, and Basic sandbox.
    pub fn said(&self) -> String {
        let TeamVault { vault, owner } = self;
        format!(
            "No sandbox is refused here: the vault {vault}, which {owner} registered in the \
             project's committed vaults.json, is a plain file a chat with no sandbox could \
             read. Choose Basic sandbox, or ask {owner} to move it into the keychain."
        )
    }
}

/// What choosing No sandbox would do for the project of `ctx`, read without writing anything.
pub fn report(ctx: &Ctx) -> Result<Report, String> {
    let (mine, team): (Vec<_>, Vec<_>) = tokeyring::plain_file_vaults(ctx)
        .map_err(|why| why.message.to_string())?
        .into_iter()
        .partition(|one| one.movable);
    Ok(Report {
        moves: mine.into_iter().map(|one| one.name).collect(),
        warning: WARNING,
        refused: team.first().map(|one| TeamVault {
            vault: one.name.clone(),
            owner: owner_of(ctx, &one.name),
        }),
    })
}

/// How long git is given to say who added a vault's entry, before the refusal says "your team".
const BLAME_WITHIN: Duration = Duration::from_secs(3);

/// The longest owner's name said: past it, the name is cut.
const OWNER_AT_MOST: usize = 80;

/// **The author of the commit that added vault `name`'s own entry** to the committed
/// `vaults.json` (Q250): `git blame` of the line its entry starts on, so whoever touched the file
/// later for another vault is not named. [`YOUR_TEAM`] where the file is not in git, the line
/// is not found or not committed, or git fails or takes longer than [`BLAME_WITHIN`].
fn owner_of(ctx: &Ctx, name: &str) -> String {
    blamed(ctx, name).unwrap_or_else(|| YOUR_TEAM.to_owned())
}

fn blamed(ctx: &Ctx, name: &str) -> Option<String> {
    let file = ctx.shared_registry();
    let text = std::fs::read_to_string(&file).ok()?;
    let key = format!("\"{name}\"");
    let line = 1 + text.lines().position(|line| {
        line.match_indices(&key)
            .any(|(at, _)| line[at + key.len()..].trim_start().starts_with(':'))
    })?;
    let range = format!("{line},{line}");
    let dir = file.parent()?;
    let file_name = file.file_name()?.to_str()?;
    let ran = crate::worktree::git::run(
        dir,
        &["blame", "--porcelain", "-L", &range, "--", file_name],
        BLAME_WITHIN,
    )
    .ok()?;
    if !ran.ok() {
        return None;
    }
    let mut lines = ran.out.lines();
    // A line not committed yet is blamed on the all-zero commit, which no one wrote.
    if lines.next()?.split(' ').next()?.bytes().all(|b| b == b'0') {
        return None;
    }
    let author: String = lines
        .find_map(|line| line.strip_prefix("author "))?
        .chars()
        .filter(|c| !c.is_control())
        .take(OWNER_AT_MOST)
        .collect();
    let author = author.trim();
    (!author.is_empty()).then(|| author.to_owned())
}

/// **Every vault [`report`] lists moved, before the level is written** (R-293): refused with the
/// team's vault's sentence, or stopped with the move's own, and then nothing else moves and the
/// caller writes no level. Each vault moves whole or not at all
/// ([`tokeyring::move_to_keyring`]); one moved before another stopped stays in the keychain.
pub(super) fn moved_before_the_level(ctx: &Ctx) -> Result<(), String> {
    let said = report(ctx)?;
    if let Some(team) = said.refused {
        return Err(team.said());
    }
    for name in &said.moves {
        tokeyring::move_to_keyring(ctx, name)
            .map_err(|why| format!("purlis could not move the vault {name}: {}", why.message))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
