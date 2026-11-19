// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The `known gaps` rows (M75 slice 37, #7046; spec §14.8; Q319 C1): for each harness purlis
//! starts, every gap that holds on this system and leaves unenforced a family whose rule is in
//! force. A gap is a declared fact about a harness, not a fault: the row is green, and the chat
//! it concerns still starts. A project that wants it refused writes a forbid on `start`.
//!
//! **Wherever a family is not `off`**: the default `watch` template enforces `secrets`, `vaults`
//! and `guards.credential`, so a project that wrote no `[policy]` has gaps to declare too, and
//! what doctor declares is what Settings declares for every project.

use super::{Doctor, Row};
use crate::enforcement::Resolved;
use crate::harness::Harness;
use crate::knowngaps::Declared;
use crate::sandbox::Os;

/// The rows for the plane `d` reads, on this system, under the modes the plane runs at.
pub(super) fn known_gaps(d: &Doctor) -> Vec<Row> {
    if !d.has_plane {
        return Vec::new();
    }
    let floor = crate::enforcement::at_start(&d.root);
    // Every family off (the `open` template): nothing is asked of any harness, so no gap.
    if crate::enforcement::Family::ALL
        .into_iter()
        .all(|family| floor.mode(family) == crate::enforcement::Mode::Off)
    {
        return Vec::new();
    }
    rows(&floor, Os::this())
}

/// One row per harness, each naming every gap declared for it on `os` under `floor`.
pub fn rows(floor: &Resolved, os: Os) -> Vec<Row> {
    Harness::ALL
        .into_iter()
        .map(|harness| {
            let declared = Declared::on(harness, os, floor);
            let name = format!("known gaps: {}", harness.name());
            if declared.is_empty() {
                return Row::ok(&name, "none declared");
            }
            let said = declared
                .entries()
                .iter()
                .map(|entry| format!("{}: {}", entry.gap.id, entry.gap.said))
                .collect::<Vec<_>>()
                .join(" ");
            Row::ok(
                &name,
                format!(
                    "{} declared, and the chat still starts. {said}",
                    declared.entries().len()
                ),
            )
        })
        .collect()
}
