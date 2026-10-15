// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A project's watcher and auto-save, run in the session host (#2002): the host's
//! [`purlis_host::Save`] for the window's binary.
//!
//! **The same code as the window's.** With the start-time switch on (`crate::hostedsessions`),
//! the window asks its host to save each project it opens, and starts no auto-save of its own:
//! the host runs the plane's watch ([`crate::planewatch::Watch::in_the_host`]) and the same
//! worker ([`crate::autosave::Worker`]), in the window's own binary started again as the host,
//! with the environment the window started it with. So a save commits with the same identity,
//! scans and hooks as the window's did, and a pull request is looked at with the person's
//! sign-in where there is one, as a save pressed in the window would be.
//!
//! **What it tells the window** is what each look did ([`purlis_host::Looked`]), which the host
//! pushes over the link; the window reads its standing again, as it did when its own worker
//! told it.
//!
//! **Who is mid-turn.** A repo's auto-save waits while a chat working in its workspace is
//! mid-turn (ADR 0051). Until the board lives in the host (#2000), the host cannot tell a turn
//! from a pause, so every program running in a place that could be the workspace counts as
//! mid-turn: a repo is saved after its quiet period only while no chat runs there, and when one
//! ends, never in the middle of a turn.

use std::path::Path;
use std::sync::{Arc, Once};

use purlis_host::{Looked, Poke, Save, Saving, ToSave, Working};

use crate::autosave::{self, Saver as _};

/// The window's binary's [`Save`]: a project's watch and auto-save, in the host.
pub struct InTheHost;

impl Save for InTheHost {
    fn save(&self, saving: ToSave) -> Result<Box<dyn Saving>, String> {
        as_the_window();
        let root = saving.project_path;
        // Never fatal, as it is not in the window: the standing is read again by itself.
        let watch = crate::planewatch::Watch::in_the_host(&root)
            .map_err(|why| {
                say(&format!(
                    "{} is not watched ({why}); its auto-save reads it every few seconds",
                    root.display()
                ));
            })
            .ok();
        let told = saving.told;
        let worker = autosave::Worker::start(
            root.clone(),
            Arc::new(move |looked: autosave::Looked| {
                told(Looked {
                    saved: looked.saved,
                    moved_tree: looked.moved_tree,
                });
            }),
        );
        let working = saving.working;
        let plane = root.clone();
        worker.asks_mid_turn_of(Arc::new(move |workspace| {
            mid_turn_in(&plane, workspace, &working())
        }));
        Ok(Box::new(Kept {
            worker,
            _watch: watch,
        }))
    }
}

/// A project the host saves: its worker, and its watch. Dropping it stops both.
struct Kept {
    worker: autosave::Worker,
    _watch: Option<crate::planewatch::Watch>,
}

impl Saving for Kept {
    fn poke(&self, poke: Poke) {
        let _ = self.worker.poker().send(match poke {
            Poke::ProgramEnded => autosave::Poke::SessionEnded,
            Poke::Fetch => autosave::Poke::Fetch,
        });
    }
}

/// This process saves as the window does: it holds its own keyring items, and the person's
/// forge sign-ins are its forge calls'. Once, at the first project it saves. A test build (every
/// one is fenced) hands nothing over, as `crate::forgeaccounts::window_git` does: the machine's
/// store is outside its fence.
fn as_the_window() {
    if purlis_core::fence::FENCED {
        return;
    }
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        purlis_core::secrets::keyhold::this_process_is_the_app();
        crate::forgeaccounts::hand_over_sign_ins();
    });
}

/// The programs in `working` that could be writing a clone of `workspace` in the plane at
/// `root`, by the name a save's journal gives them: each one working in the workspace, in a
/// worktree of one of its clones, or anywhere in no other workspace, as the window counts them
/// (`crate::planes::Held::mid_turn_in`). Every one counts as mid-turn: see the module's header.
pub(crate) fn mid_turn_in(root: &Path, workspace: &str, working: &[Working]) -> Vec<String> {
    let on_disk = purlis_core::workspaces::Plane::open(root);
    working
        .iter()
        .filter(|one| {
            one.cwd
                .as_deref()
                .and_then(|cwd| crate::planes::workspace_working_in(&on_disk, cwd))
                .is_none_or(|there| there == workspace)
        })
        .map(|one| format!("chat {}", one.number))
        .collect()
}

/// One line in the host's log, which the window keeps.
fn say(line: &str) {
    use std::io::Write;
    let _ = writeln!(std::io::stderr(), "purlisd: {line}");
}

#[cfg(test)]
mod tests;
