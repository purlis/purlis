// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `work links`: the work link log's lines charter skipped, alias cycles a merge made, and todos
//! a promote aliased to an issue but did not get to close (ADR 0088 §2, §3, §5).
//!
//! **It reports and changes nothing.** The close a crash cut short is finished by the next
//! `charter ws todo` in that workspace, which is what the row says to run.
//!
//! **Shown only when there is something to say.** A project nothing has linked or promoted has
//! no log, and a healthy one has nothing to report; a row on every run would be furniture.

use super::{Doctor, Row};
use crate::counts::counted;
use crate::work::log;

const NAME: &str = "work links";

pub(super) fn work_links(d: &Doctor) -> Option<Row> {
    if !d.has_plane {
        return None;
    }
    let folded = log::fold(&d.root);
    let cycles = folded.cycles();
    if !cycles.is_empty() {
        let keys: Vec<String> = cycles.iter().map(ToString::to_string).collect();
        return Some(Row::fail(
            NAME,
            format!("an alias cycle: {}", keys.join(", ")),
            "Two devices' logs, merged, say each key stands for the other. Readers stop before \
             the first key they would repeat. The newest alias is the one to undo: remove its \
             line from workspaces/<ws>/work/<device>.jsonl and save.",
        ));
    }
    let unclosed = promoted_but_open(d, &folded);
    if let Some(ws) = unclosed.first().map(|(ws, _, _)| ws.clone()) {
        let named: Vec<String> = unclosed
            .iter()
            .map(|(ws, title, to)| format!("{ws}: '{title}' → {to}"))
            .collect();
        return Some(Row::warn(
            NAME,
            format!("promoted but still open: {}", named.join("; ")),
            format!(
                "A promote stopped after its alias, so every reader already treats the todo as \
                 the issue. `purlis ws todo -w {ws}` closes it, journalled as promote would have."
            ),
        ));
    }
    let skipped = folded.skipped();
    (skipped > 0).then(|| {
        let files: Vec<String> = folded
            .skipped_in()
            .iter()
            .map(|(file, n)| format!("{n} in workspaces/{}", file.replacen('/', "/work/", 1)))
            .collect();
        Row::warn(
            NAME,
            format!(
                "{} skipped, not one of the four closed key sets: {}",
                counted(skipped, "line", "lines"),
                files.join(", ")
            ),
            "Lines purlis did not write, or a newer purlis wrote. Each is read as absent; \
             nothing else in the log is affected.",
        )
    })
}

/// Every open todo whose key already has an alias: `(workspace, title, where it went)`.
fn promoted_but_open(d: &Doctor, folded: &log::Fold) -> Vec<(String, String, String)> {
    let plane = crate::workspaces::Plane::open(&d.root);
    let Ok(names) = plane.workspaces() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for name in names {
        let Ok(ws) = plane.workspace(&name) else {
            continue;
        };
        for todo in ws.todos().unwrap_or_default() {
            let Ok(key) = crate::work::TrackerKey::todo(&name, &todo.slug) else {
                continue;
            };
            if folded.is_aliased(&key) {
                out.push((name.clone(), todo.title, folded.resolve(&key).to_string()));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests;
