// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis artifact templates` and `purlis artifact pin --template`: an extension's artifact
//! templates (ADR 0122 §9, #5973), the command line of [`purlis_core::extension::artifacts`];
//! and `purlis artifact pin --skill`'s placing of a page a skill ships (#5974).

use purlis_core::artifact::Pinned;
use purlis_core::extension::{BuiltIn, artifacts, project::Choices};
use purlis_core::workspaces::Workspace;

use crate::Here;
use crate::memory::Code;
use crate::voice;

/// What this machine's extensions offer the workspace, one line each.
pub fn list(here: &Here, ws: &Workspace) -> Code {
    let Some(config) = purlis_core::machine::config_root() else {
        voice::info("No extension offers an artifact template: purlis has no config folder here.");
        return 0;
    };
    let offered = artifacts::offered(
        &config,
        &BuiltIn::of_this_program(),
        &Choices::read_in(here.plane.root(), Some(ws.name())),
    );
    if offered.is_empty() {
        voice::info(format!(
            "No extension offers an artifact template to '{}'.",
            ws.name()
        ));
    }
    for one in offered {
        println!(
            "{}  {}  {}  {}",
            one.id,
            purlis_core::personas::one_line(&one.title),
            one.icon,
            purlis_core::personas::one_line(&one.extension)
        );
    }
    0
}

/// Copy extension template `id` into the workspace and pin it.
pub fn pin(
    here: &Here,
    ws: &Workspace,
    id: &str,
    name: Option<String>,
    icon: Option<String>,
) -> Code {
    let root = here.plane.root();
    let Some(config) = purlis_core::machine::config_root() else {
        voice::err("purlis has no config folder here, so no extension offers a template.");
        return 1;
    };
    let pinned = artifacts::pin(
        &config,
        &BuiltIn::of_this_program(),
        &Choices::read_in(root, Some(ws.name())),
        root,
        ws.dir(),
        &artifacts::Wanted { id, name, icon },
    );
    let done = match pinned {
        Ok(done) => done,
        Err(why) => {
            voice::err(why);
            return 1;
        }
    };
    let pin = match &done.pinned {
        Pinned::New(pin) | Pinned::Updated(pin) | Pinned::Replaced { new: pin, .. } => pin,
    };
    let reach = if purlis_core::wscmd::live_workspaces(root).contains(ws.name()) {
        "everyone in the plane sees it once the plane is saved"
    } else {
        "the workspace is LOCAL, so it stays on this machine"
    };
    voice::ok(format!(
        "Copied {id} into artifacts/{} and pinned '{}' ({}) to '{}' — {reach}. Its script, if \
         it has any, asks each person before it runs.",
        done.folder,
        purlis_core::personas::one_line(&pin.name),
        pin.id,
        ws.name()
    ));
    0
}

/// **Place page `folder` that skill `skill` ships in the workspace** (#5974), at the person's
/// terminal: the skill found among the skills a chat in the workspace would have.
pub fn place_from_skill(
    here: &Here,
    ws: &Workspace,
    skill: &str,
    folder: Option<&str>,
) -> Result<purlis_core::artifact::template::Placed, String> {
    let root = here.plane.root();
    let set = purlis_core::skills::of_a_chat(
        purlis_core::skills::built_in_here().as_deref(),
        root,
        Some(ws.name()),
        None,
        purlis_core::skills::Route::Mcp,
    );
    purlis_core::artifact::template::place_from_skill(
        root,
        ws.dir(),
        &set,
        skill,
        folder.unwrap_or_default(),
    )
}

/// Remove a copy [`place_from_skill`] made, once its pin was refused.
pub fn remove_placed(ws: &Workspace, folder: &str) {
    let _ = std::fs::remove_dir_all(ws.dir().join(purlis_core::artifact::DIR).join(folder));
}
