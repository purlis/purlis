// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The `harness` row: every harness the project has, and the most each one offers (OB-8, #994).
//!
//! **The registry is the declarations** (ADR 0073): the three purlis ships and each
//! `harnesses/<name>.toml` the project commits, read by [`harness_declaration::read`], the
//! same reader a chat's start asks. A harness's ceiling is the highest level its declaration
//! offers: level 3 where it names an ACP agent, level 2 where purlis ships the adapter that
//! arms its hooks, and level 1, its terminal alone, otherwise.
//!
//! **A warning for a declaration purlis refused**, with the reader's own sentence: a refused
//! file declares nothing, so a profile naming it starts no chat, and nothing else on the
//! project's surfaces says so before someone tries. Reading runs nothing and asks nothing, so
//! the preflight runs this row too.

use super::{Doctor, Row};
use crate::counts::counted;
use crate::harness_declaration::{self, Declaration, Declarations};

/// The row's name, as the Python charter printed it.
pub(super) const NAME: &str = "harness";

/// The row for the project `d` answers for. Without a project, the built-ins alone.
pub(super) fn harness(d: &Doctor) -> Row {
    let declared = if d.has_plane {
        harness_declaration::read(&d.root)
    } else {
        Declarations {
            declared: harness_declaration::builtins().to_vec(),
            refused: Vec::new(),
        }
    };
    row_of(&declared)
}

/// The row for `declared`.
pub(super) fn row_of(declared: &Declarations) -> Row {
    let ceilings = declared
        .declared
        .iter()
        .map(ceiling)
        .collect::<Vec<_>>()
        .join("; ");
    let count = declared.declared.len();
    if declared.refused.is_empty() {
        return Row::ok(
            NAME,
            format!("{} — {ceilings}", counted(count, "harness", "harnesses")),
        );
    }
    let refused = declared
        .refused
        .iter()
        .map(|r| super::one_line(&r.reason, super::PATH_DISPLAY_LIMIT))
        .collect::<Vec<_>>()
        .join("; ");
    Row::warn(
        NAME,
        format!(
            "{} refused: {refused} ({} read — {ceilings})",
            counted(declared.refused.len(), "declaration", "declarations"),
            counted(count, "harness", "harnesses")
        ),
        "A refused declaration declares nothing, so a profile that names its harness starts \
         no chat. Fix the file as the reason says, or remove it from harnesses/; `purlis \
         harness list` shows what is read.",
    )
}

/// One harness and the highest level its declaration offers.
fn ceiling(d: &Declaration) -> String {
    let level = if d.levels.acp.is_some() {
        "level 3 (ACP)"
    } else if d.has_adapter() {
        "level 2 (hooks)"
    } else {
        "level 1 (terminal only)"
    };
    format!(
        "{}: {level}",
        super::one_line(&d.name, super::DISPLAY_LIMIT)
    )
}

#[cfg(test)]
mod tests;
