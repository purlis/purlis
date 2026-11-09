// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis artifact pin|unpin|list|reorder|approve-status`: the command line of
//! [`purlis_core::artifact`] (ADR 0120, #5950). A pin is committed with the workspace, so a
//! LIVE workspace's pins reach every teammate with the plane's next save.
//!
//! **There is no `approve` here** (#5959): a page's script is approved only in purlis's bar,
//! outside the page, by the person — a chat runs a command as easily as it edits a file
//! (ADR 0041). `approve-status` reads where each pin stands on this machine.

use clap::Subcommand;
use purlis_core::artifact::{self, Pin, Pinned, Request};

use crate::Here;
use crate::memory::Code;
use crate::voice;

/// `purlis artifact …`.
#[derive(Subcommand)]
pub enum ArtifactCommand {
    /// Pin a folder of the workspace's artifacts/ to the workspace: its index.html gets an icon
    /// on the workspace's activity bar, for everyone in the plane once it is saved (on this
    /// machine alone while the workspace is LOCAL). Pinning a folder again with another
    /// command or working folder makes it a new pin.
    Pin {
        /// The folder's name inside the workspace's artifacts/.
        folder: String,
        /// The icon's name (default: the page's <title>, else the folder's name).
        #[arg(long)]
        name: Option<String>,
        /// One of purlis's icons (default: chart).
        #[arg(long)]
        icon: Option<String>,
        /// The command that writes the artifact's data, run by Regenerate.
        #[arg(long)]
        command: Option<String>,
        /// The command's working folder, relative to the workspace.
        #[arg(long, requires = "command")]
        cwd: Option<String>,
        /// The workspace (default: the active one).
        #[arg(short = 'w', long = "workspace")]
        workspace: Option<String>,
    },
    /// Unpin an artifact from the workspace, by its id or its folder. Its files stay.
    Unpin {
        /// The pin's id or folder.
        pin: String,
        /// The workspace (default: the active one).
        #[arg(short = 'w', long = "workspace")]
        workspace: Option<String>,
    },
    /// The workspace's pinned artifacts in their order, each with its id, and any whose folder
    /// cannot be opened here (not pulled yet, removed, or over the size cap) said so.
    List {
        /// The workspace (default: the active one).
        #[arg(short = 'w', long = "workspace")]
        workspace: Option<String>,
    },
    /// Put the named pins first, in the order given; every other pin follows in its order.
    Reorder {
        /// Pins by id or folder, first to last.
        #[arg(required = true)]
        pins: Vec<String>,
        /// The workspace (default: the active one).
        #[arg(short = 'w', long = "workspace")]
        workspace: Option<String>,
    },
    /// Where each pinned artifact stands on this machine: script-free (opens without asking),
    /// approved (its script runs), or asking — new, its template changed since you approved
    /// it, or its pin changed (and by whom). One line each: folder, standing, the template's
    /// fingerprint, and what it means. Approving is done in the artifact's bar in the window.
    ApproveStatus {
        /// Only this pin, by id or folder.
        pin: Option<String>,
        /// The workspace (default: the active one).
        #[arg(short = 'w', long = "workspace")]
        workspace: Option<String>,
    },
}

pub fn run(here: &Here, command: ArtifactCommand) -> Result<Code, String> {
    let root = here.plane.root();
    match command {
        ArtifactCommand::Pin {
            folder,
            name,
            icon,
            command,
            cwd,
            workspace,
        } => {
            let ws = here.workspace(workspace.as_deref())?;
            let request = Request {
                folder,
                name,
                icon,
                command,
                cwd,
            };
            let reach = if purlis_core::wscmd::live_workspaces(root).contains(ws.name()) {
                "everyone in the plane sees it once the plane is saved"
            } else {
                "the workspace is LOCAL, so it stays on this machine"
            };
            match artifact::pin(root, ws.dir(), &request) {
                Ok(Pinned::New(pin)) => {
                    voice::ok(format!(
                        "Pinned {} to '{}' — {reach}.",
                        said(&pin),
                        ws.name()
                    ));
                }
                Ok(Pinned::Updated(pin)) => {
                    voice::ok(format!(
                        "{} is pinned to '{}' — {reach}.",
                        said(&pin),
                        ws.name()
                    ));
                }
                Ok(Pinned::Replaced { new, old_id }) => {
                    voice::ok(format!(
                        "Pinned {} to '{}' as a new pin in the place of {}, since its command \
                         or working folder changed — {reach}.",
                        said(&new),
                        ws.name(),
                        old_id
                    ));
                }
                Err(why) => return refused(&why),
            }
            Ok(0)
        }
        ArtifactCommand::Unpin { pin, workspace } => {
            let ws = here.workspace(workspace.as_deref())?;
            match artifact::unpin(root, ws.dir(), &pin) {
                Ok(pin) => {
                    voice::ok(format!(
                        "Unpinned {} from '{}'; its folder stays in artifacts/.",
                        said(&pin),
                        ws.name()
                    ));
                    Ok(0)
                }
                Err(why) => refused(&why),
            }
        }
        ArtifactCommand::List { workspace } => {
            let ws = here.workspace(workspace.as_deref())?;
            let listed = match artifact::list(ws.dir()) {
                Ok(listed) => listed,
                Err(why) => return refused(&why),
            };
            if listed.is_empty() {
                voice::info(format!(
                    "No artifact is pinned to '{}' (pin one: purlis artifact pin <folder>).",
                    ws.name()
                ));
            }
            for item in listed {
                let pin = &item.pin;
                let mut line = format!(
                    "{}  {}  {}  {}  {}",
                    pin.order,
                    pin.id,
                    purlis_core::personas::one_line(&pin.folder),
                    pin.icon,
                    purlis_core::personas::one_line(&pin.name)
                );
                if let Some(why) = item.missing {
                    line.push_str(&format!("  (missing: {why})"));
                }
                if let Some(why) = item.too_large {
                    line.push_str(&format!("  (too large: {why})"));
                }
                println!("{line}");
            }
            Ok(0)
        }
        ArtifactCommand::Reorder { pins, workspace } => {
            let ws = here.workspace(workspace.as_deref())?;
            match artifact::reorder(root, ws.dir(), &pins) {
                Ok(now) => {
                    let order: Vec<String> = now
                        .iter()
                        .map(|pin| purlis_core::personas::one_line(&pin.folder))
                        .collect();
                    voice::ok(format!(
                        "Reordered '{}'s artifacts: {}.",
                        ws.name(),
                        order.join(", ")
                    ));
                    Ok(0)
                }
                Err(why) => refused(&why),
            }
        }
        ArtifactCommand::ApproveStatus { pin, workspace } => {
            let ws = here.workspace(workspace.as_deref())?;
            let listed = match artifact::list(ws.dir()) {
                Ok(listed) => listed,
                Err(why) => return refused(&why),
            };
            let config = purlis_core::machine::config_root();
            let mut shown = 0;
            for item in listed {
                let one = &item.pin;
                if pin
                    .as_deref()
                    .is_some_and(|key| key != one.id && key != one.folder)
                {
                    continue;
                }
                shown += 1;
                println!("{}", standing_line(config.as_deref(), ws.dir(), &item));
            }
            if let Some(key) = pin.filter(|_| shown == 0) {
                return refused(&format!(
                    "no pin '{}' in this workspace (`purlis artifact list` shows them)",
                    purlis_core::personas::one_line(&key)
                ));
            }
            Ok(0)
        }
    }
}

/// One pin's line of `approve-status`: folder, standing, fingerprint, and what it means.
fn standing_line(
    config: Option<&std::path::Path>,
    workspace: &std::path::Path,
    item: &artifact::Listed,
) -> String {
    use artifact::approval::{self, Standing};
    let pin = &item.pin;
    let folder = purlis_core::personas::one_line(&pin.folder);
    let read = match &item.missing {
        Some(why) => Err(why.clone()),
        None => artifact::resolve(workspace, &pin.folder)
            .and_then(|real| artifact::serve::Snapshot::read(&real)),
    };
    let snapshot = match read {
        Ok(snapshot) => snapshot,
        Err(why) => return format!("{folder}  missing  -  {why}"),
    };
    let standing = approval::standing(config, workspace, &pin.folder, &pin.id, &snapshot);
    let meaning = match standing {
        Standing::ScriptFree => "opens without asking: no script".to_owned(),
        Standing::Approved => "approved on this machine: its script runs".to_owned(),
        Standing::New => "asks before its script runs: never approved on this machine".to_owned(),
        Standing::Changed => "asks again: its template changed since you approved it".to_owned(),
        Standing::PinChanged => match artifact::pinned_by(workspace, &pin.id) {
            Some(who) => format!(
                "asks again: pin changed by {who} (its folder or command) since you approved it"
            ),
            None => "asks again: pin changed (its folder or command) since you approved it, \
                     not saved yet"
                .to_owned(),
        },
        Standing::NotHere => {
            "renders without its script: scripted artifacts do not run on this platform yet"
                .to_owned()
        }
    };
    format!(
        "{folder}  {}  {}  {meaning}",
        standing.as_str(),
        snapshot.fingerprint()
    )
}

/// A pin as a sentence names it: its name, folder and id.
fn said(pin: &Pin) -> String {
    format!(
        "'{}' ({}, {})",
        purlis_core::personas::one_line(&pin.name),
        purlis_core::personas::one_line(&pin.folder),
        pin.id
    )
}

fn refused(why: &str) -> Result<Code, String> {
    voice::err(why.to_string());
    Ok(1)
}
