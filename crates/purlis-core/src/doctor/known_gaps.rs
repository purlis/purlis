// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The `known gaps` rows (M75 slice 37, #7046; spec §14.8; Q319 C1): for each harness purlis
//! starts, every gap that holds on this system and leaves unenforced a family whose rule is in
//! force. A gap is a declared fact about a harness, not a fault: the row is green, and the chat
//! it concerns still starts. A project that wants it refused writes a forbid on `start`.
//!
//! **Only where the project wrote a `[policy]`** (a template or a family's mode): the table doctor
//! prints for a project that did not is the one the Python charter recorded, row for row
//! (`every_row_python_prints_is_printed_in_pythons_order`, `recorded_behaviour`), and what
//! it declares there is what Settings declares for every project.

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
    let said = crate::enforcement::Settings::read(&d.root);
    if !said.template_said && said.overrides.is_empty() {
        return Vec::new();
    }
    rows(&crate::enforcement::at_start(&d.root), Os::this())
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
