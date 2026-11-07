// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The workspace's `repos/` folder (ADR 0114, decision 3; #5374): three findings, each a row
//! only where its case is on disk, so a plane with every clone in `repos/` prints what it
//! printed before.
//!
//! - **`repos outside repos/`**: clones still at a workspace's root. They keep working with
//!   nothing done, so this is a warning with a fix the operator starts, `move-into-repos`
//!   ([`super::fix::move_into_repos`]), and never a notice: the old layout is not out of date.
//! - **`repos beside repos/`**: a folder at the root beside the repo of its name in `repos/`,
//!   which is the repo ([`crate::repofolder::Holdings::shadowed`]).
//! - **`repo named repos`**: an old clone at the root named `repos`, which holds every new clone
//!   of its workspace back until it is renamed ([`crate::repofolder::Refused::RepoNamedRepos`]).
//!
//! A workspace that cannot be read is left out here; the `workspace clones` row names it.

use super::fix::{FixId, move_into_repos};
use super::{Doctor, Row};
use crate::counts::counted;
use crate::repofolder;

/// The three rows, each only where it has something to say.
pub(super) fn rows(d: &Doctor) -> Vec<Row> {
    if !d.has_plane {
        return Vec::new();
    }
    let Ok(workspaces) = crate::workspaces::Plane::open(&d.root).workspaces() else {
        return Vec::new();
    };
    let mut outside: Vec<String> = Vec::new();
    let mut beside: Vec<String> = Vec::new();
    let mut named_repos: Vec<String> = Vec::new();
    for ws in &workspaces {
        let Ok(held) = repofolder::holdings(&d.root, ws) else {
            continue;
        };
        for row in &held.repos {
            if move_into_repos::named_repos(row) {
                named_repos.push(shown(ws));
            } else if row.layout == repofolder::Layout::Root {
                outside.push(shown(&format!("{ws}/{}", row.repo.name)));
            }
        }
        beside.extend(held.shadowed.iter().map(|row| {
            let name = &row.repo.name;
            shown(&format!(
                "{ws}/{name}/ beside {ws}/repos/{name}/ (repos/{name}/ is the repo)"
            ))
        }));
    }
    let mut rows = Vec::new();
    if !outside.is_empty() {
        rows.push(
            Row::warn(
                "repos outside repos/",
                format!(
                    "{} outside repos/: {}",
                    counted(outside.len(), "repo", "repos"),
                    listed(&outside)
                ),
                "They work where they are, and nothing needs moving. `purlis doctor --fix \
                 move-into-repos` moves them into their workspace's repos/ when you choose, one \
                 workspace at a time, and repairs every branch folder cut from them; a \
                 workspace with a chat open in it is left as it is.",
            )
            .fixed_by(FixId::MoveIntoRepos),
        );
    }
    if !beside.is_empty() {
        rows.push(Row::warn(
            "repos beside repos/",
            listed(&beside),
            "purlis reads the repo in repos/ and nothing reads the other folder. Move it out of \
             the workspace, or remove it, once you no longer need what is in it.",
        ));
    }
    if !named_repos.is_empty() {
        rows.push(Row::warn(
            "repo named repos",
            format!(
                "an old clone named repos, where new repos now go, in {}",
                listed(&named_repos)
            ),
            "No repo can be added to that workspace until you rename it: move \
             workspaces/<ws>/repos to another name, then run `git worktree repair` in it if it \
             has branch folders.",
        ));
    }
    rows
}

/// A value on one line, as a row quotes it.
fn shown(value: &str) -> String {
    super::one_line(value, super::DISPLAY_LIMIT)
}

/// The first four, and `…` for the rest.
fn listed(items: &[String]) -> String {
    let mut text = items.iter().take(4).cloned().collect::<Vec<_>>().join(", ");
    if items.len() > 4 {
        text.push_str(", …");
    }
    text
}
