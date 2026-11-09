// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis artifact pin|unpin|list|reorder`: the command line of [`purlis_core::artifact`]
//! (ADR 0120, #5950). A pin is committed with the workspace, so a LIVE workspace's pins reach
//! every teammate with the plane's next save.

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
    /// cannot be opened here (not pulled yet, or removed) said so.
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
                Ok(Pinned::Replaced { new, old }) => {
                    voice::ok(format!(
                        "Pinned {} to '{}' as a new pin in the place of {}, since its command \
                         or working folder changed — {reach}.",
                        said(&new),
                        ws.name(),
                        old.id
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
    }
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
