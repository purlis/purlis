// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis artifact pin|unpin|list|search|reorder|approve-status`: the command line of
//! [`purlis_core::artifact`] (ADR 0122, #5950). A pin is committed with the workspace, so a
//! LIVE workspace's pins reach every teammate with the plane's next save.
//!
//! **There is no `approve` here** (#5959): a page's script is approved only in purlis's bar,
//! outside the page, by the person — a chat runs a command as easily as it edits a file
//! (ADR 0041). `approve-status` reads where each pin stands on this machine.

use clap::Subcommand;
use purlis_core::artifact::{self, Pin, Pinned, Request};

use crate::Here;
use crate::artifact_templates as templates;
use crate::memory::Code;
use crate::voice;

/// `purlis artifact …`.
#[derive(Subcommand)]
pub enum ArtifactCommand {
    /// Pin a folder of the workspace's artifacts/ to the workspace: its index.html gets an icon
    /// on the workspace's activity bar, for everyone in the plane once it is saved (on this
    /// machine alone while the workspace is LOCAL). Pinning a folder again with another
    /// command or working folder makes it a new pin.
    /// Run in a chat, it only proposes the pin: the person pins it in the window.
    Pin {
        /// The folder's name inside the workspace's artifacts/.
        #[arg(required_unless_present = "template", conflicts_with = "template")]
        folder: Option<String>,
        /// An extension's template to pin instead, as <extension-id>.<name> (`purlis artifact
        /// templates` lists them): copied into artifacts/<extension-id>.<name>/, then pinned as
        /// any folder is.
        #[arg(long, conflicts_with_all = ["command", "cwd"])]
        template: Option<String>,
        /// A skill that ships the page as its artifacts/<folder>/ (`purlis-skill-studio` says how):
        /// copied, without its data files, into the workspace's artifacts/<folder>/ unless the
        /// same page is there already, then pinned, or in a chat proposed, as any folder is.
        #[arg(long, conflicts_with = "template")]
        skill: Option<String>,
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
    /// The artifact templates this machine's extensions offer: each one's id (what
    /// `pin --template` takes), its title, its icon and its extension. Only an extension
    /// approved here, unchanged since, and on in the workspace offers one.
    Templates {
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
    /// Find pinned artifacts by what their pages say: each one whose name or pages hold a word
    /// starting with a word of the query, best first, as `<workspace>/<folder>  <name>`. Read
    /// from the project's search index, or from the files when there is none. A page's data
    /// files are each person's own and are never searched.
    Search {
        /// The words to find.
        #[arg(required = true)]
        query: Vec<String>,
        /// The workspace (default: the active one).
        #[arg(short = 'w', long = "workspace", conflicts_with = "all")]
        workspace: Option<String>,
        /// Every workspace's artifacts.
        #[arg(long)]
        all: bool,
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
    /// Regenerate a pinned artifact's data. With --check, say what its command resolves to on
    /// this machine (the code a person approves, its fingerprint) and whether a press runs it
    /// or asks: approved, new, changed (its code changed since you approved it), or
    /// pin-changed. The run itself is started from the artifact's bar in the window, in purlis's
    /// session host, never by this command.
    Regenerate {
        /// The pin, by id or folder.
        pin: String,
        /// Only say where the pin's command stands; run nothing.
        #[arg(long)]
        check: bool,
        /// The workspace (default: the active one).
        #[arg(short = 'w', long = "workspace")]
        workspace: Option<String>,
    },
    /// Check a page before anyone is asked to approve it: that its declaration
    /// (<script type="application/json" id="purlis">) is one purlis reads, at an API version it
    /// answers, naming only queries the catalogue has, and that every query its pages and
    /// scripts call is one it declared. Needs no plane. Exits 1 when anything would be refused.
    Check {
        /// The artifact's folder, holding its index.html.
        folder: std::path::PathBuf,
    },
    /// Print purlis-page.d.ts, the TypeScript types of a page's `purlis.port`, generated from
    /// this purlis's query catalogue: what a page asks, and each query's parameters and data.
    Types,
    /// Print purlis-page-mock.js, a mock of `purlis.port` that answers from
    /// purlis-fixtures.json beside the page, so a page can be built in any browser outside
    /// purlis. In purlis it does nothing.
    Mock,
}

impl ArtifactCommand {
    /// Whether it is one of a page author's commands, which read no plane (#5975).
    pub fn is_authors(&self) -> bool {
        matches!(
            self,
            ArtifactCommand::Check { .. } | ArtifactCommand::Types | ArtifactCommand::Mock
        )
    }
}

/// **A page author's commands** (ADR 0122 §7, #5975), answered with no plane: `check`, `types`
/// and `mock`. Any other command is not an author's and answers 2.
pub fn authors(command: &ArtifactCommand) -> Code {
    use purlis_core::artifact::{check, kit};
    match command {
        ArtifactCommand::Types => {
            print!("{}", kit::types());
            0
        }
        ArtifactCommand::Mock => {
            print!("{}", kit::MOCK);
            0
        }
        ArtifactCommand::Check { folder } => {
            let name = folder.display().to_string();
            let checked = match check::check(folder, &name) {
                Ok(checked) => checked,
                Err(why) => {
                    voice::err(format!("{name}: {why}."));
                    return 1;
                }
            };
            for line in artifact::tools::check_lines(&checked) {
                println!("{line}");
            }
            for warning in &checked.warnings {
                voice::warn(warning.clone());
            }
            for why in &checked.refused {
                voice::err(why.clone());
            }
            if !checked.refused.is_empty() {
                return 1;
            }
            voice::ok(artifact::tools::check_summary(&name, &checked));
            0
        }
        _ => 2,
    }
}

pub fn run(here: &Here, command: ArtifactCommand) -> Result<Code, String> {
    let root = here.plane.root();
    match command {
        ArtifactCommand::Pin {
            folder,
            template,
            skill,
            name,
            icon,
            command,
            cwd,
            workspace,
        } => {
            // A chat proposes; only the person pins (#5963, ADR 0122 §1). Which chat it is, and so
            // which workspace, is the app's record of it, never a flag: `-w` names nothing here.
            if !crate::skill::at_the_persons_terminal(root) {
                // A template is copied into the workspace before it is pinned: no chat does that
                // either, so it is the person's to pin from the window or their terminal.
                let Some(folder) = folder else {
                    return refused(
                        "a chat pins no extension template, so nothing was pinned: ask the person \
                         to pin it with `purlis artifact pin --template` at their terminal",
                    );
                };
                let proposed = purlis_core::artifact::proposal::Proposed {
                    folder,
                    name,
                    icon,
                    command,
                    cwd,
                    skill,
                };
                return match crate::brokered::propose_pin(proposed) {
                    Ok(code) => Ok(code),
                    Err(why) => refused(&why),
                };
            }
            let ws = here.workspace(workspace.as_deref())?;
            if let Some(template) = template {
                return Ok(templates::pin(here, &ws, &template, name, icon));
            }
            // A skill's page is placed first, from the skills a chat here would have.
            let placed = match &skill {
                Some(skill) => {
                    match templates::place_from_skill(here, &ws, skill, folder.as_deref()) {
                        Ok(placed) => Some(placed),
                        Err(why) => return refused(&why),
                    }
                }
                None => None,
            };
            let request = Request {
                folder: folder.unwrap_or_default(),
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
                Err(why) => {
                    if placed == Some(purlis_core::artifact::template::Placed::Copied) {
                        templates::remove_placed(&ws, &request.folder);
                    }
                    return refused(&why);
                }
            }
            Ok(0)
        }
        ArtifactCommand::Templates { workspace } => {
            let ws = here.workspace(workspace.as_deref())?;
            Ok(templates::list(here, &ws))
        }
        ArtifactCommand::Unpin { pin, workspace } => {
            let ws = here.workspace(workspace.as_deref())?;
            let config = purlis_core::machine::config_root();
            match artifact::unpin_here(root, ws.dir(), &pin, config.as_deref()) {
                Ok((pin, kept)) => {
                    voice::ok(format!(
                        "Unpinned {} from '{}'; its folder stays in artifacts/.",
                        said(&pin),
                        ws.name()
                    ));
                    if let Some(why) = kept {
                        voice::warn(format!("Unpinned, but {why}."));
                    }
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
                voice::info(artifact::tools::none_pinned(ws.name()));
            }
            for item in &listed {
                println!("{}", artifact::tools::list_line(item));
            }
            Ok(0)
        }
        ArtifactCommand::Search {
            query,
            workspace,
            all,
        } => {
            let query = query.join(" ");
            let ws = if all {
                None
            } else {
                Some(here.workspace(workspace.as_deref())?)
            };
            let found = artifact::search::find(root, ws.as_ref().map(|ws| ws.name()), &query).found;
            if found.is_empty() {
                let in_where = ws.map_or_else(
                    || "any workspace".to_string(),
                    |ws| format!("'{}'", ws.name()),
                );
                voice::info(format!(
                    "No artifact pinned to {in_where} says '{}'.",
                    purlis_core::personas::one_line(&query)
                ));
            }
            for one in found {
                println!(
                    "{}/{}  {}",
                    one.workspace,
                    purlis_core::personas::one_line(&one.folder),
                    purlis_core::personas::one_line(&one.name)
                );
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
            let config = purlis_core::machine::config_root();
            match artifact::tools::status_lines(config.as_deref(), ws.dir(), pin.as_deref()) {
                Ok(lines) => {
                    for line in lines {
                        println!("{line}");
                    }
                    Ok(0)
                }
                Err(why) => refused(&why),
            }
        }
        ArtifactCommand::Regenerate {
            pin,
            check,
            workspace,
        } => regenerate(here, &pin, check, workspace.as_deref()),
        authors_command @ (ArtifactCommand::Check { .. }
        | ArtifactCommand::Types
        | ArtifactCommand::Mock) => Ok(authors(&authors_command)),
    }
}

/// `purlis artifact regenerate <pin> --check`: what the pin's command resolves to and where it
/// stands. Without `--check` it refuses: Regenerate runs in the session host, from the bar.
fn regenerate(
    here: &Here,
    key: &str,
    check: bool,
    workspace: Option<&str>,
) -> Result<Code, String> {
    use artifact::regenerate::{self, Standing};
    if !check {
        return refused(
            "Regenerate runs in purlis's session host, in the sandbox a chat's commands run \
             in, and is started from the artifact's bar in the window; nothing was run. Add \
             --check to see where its command stands on this machine",
        );
    }
    let ws = here.workspace(workspace)?;
    let pins = match artifact::read(ws.dir()) {
        Ok(pins) => pins.pins,
        Err(why) => return refused(&why),
    };
    let Some(pin) = pins.iter().find(|p| p.id == key || p.folder == key) else {
        return refused(&format!(
            "no pin '{}' in this workspace (`purlis artifact list` shows them)",
            purlis_core::personas::one_line(key)
        ));
    };
    let resolved = match regenerate::resolve(here.plane.root(), ws.dir(), pin) {
        Ok(resolved) => resolved,
        Err(why) => return refused(&why),
    };
    let config = purlis_core::machine::config_root();
    let standing =
        regenerate::standing(config.as_deref(), ws.dir(), &pin.folder, &pin.id, &resolved);
    let meaning = match standing {
        Standing::Approved => "approved on this machine: a press of Regenerate runs it",
        Standing::New => "asks before it runs: never approved on this machine",
        Standing::Changed => "asks again: the code it runs changed since you approved it",
        Standing::PinChanged => {
            "asks again: its pin changed (its folder, command or working folder) since you \
             approved it"
        }
    };
    println!(
        "{}  {}  {}  {}  -  {meaning}",
        purlis_core::personas::one_line(&pin.folder),
        standing.as_str(),
        resolved.code.display(),
        resolved.fingerprint
    );
    Ok(0)
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
