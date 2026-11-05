// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis check report <branch> <file.sarif>` (#5407, ADR 0114): how a branch's check results
//! arrive, from a script, an agent's hook or CI. purlis runs no check here and reads one format:
//! the core (`purlis_core::checks`) reads the file, keeps it and places it on the diff.
//!
//! **In a chat the app started, the app makes the write** (ADR 0067 §2): a chat's sandbox does not
//! write the workspace's `pieces/`. The file is checked here first, so a bad one is refused in the
//! same words either way, and it must lie inside the branch's folder for the app to read it.
//! **The exit is 0 when it was recorded, 1 when it was refused**, nothing recorded.

use std::path::Path;

use clap::Subcommand;
use purlis_core::brokered::Write;
use purlis_core::checks;
use purlis_core::files::Branch;

use crate::brokered::{Forwarded, Named};
use crate::{Here, voice};

#[derive(Subcommand)]
pub enum CheckCommand {
    /// Hand purlis a branch's check results, in SARIF 2.1.0, to draw on its diff. A tool that
    /// reported before keeps its place until it reports again; the results are marked out of date
    /// once the branch's head moves.
    Report {
        /// The branch: one a folder of this workspace has checked out.
        branch: String,
        /// The SARIF 2.1.0 file. In a sandboxed chat it must be inside the branch's folder.
        file: std::path::PathBuf,
        /// The repo, where two of the workspace's have the branch. Default: the one you stand in.
        #[arg(long)]
        repo: Option<String>,
        /// The workspace. Default: the active one.
        #[arg(long, short = 'w')]
        workspace: Option<String>,
    },
}

pub fn run(here: &Here, command: CheckCommand) -> Result<u8, String> {
    let CheckCommand::Report {
        branch,
        file,
        repo,
        workspace,
    } = command;
    let root = here.plane.root();
    let named = crate::brokered::named_here(here, workspace.as_deref());
    if let Named::Refused(why) = &named {
        voice::err(why.clone());
        return Ok(1);
    }
    let ws = here.active_workspace(workspace.as_deref())?;
    let repo = repo.or_else(|| repo_here(root, &here.cwd));
    let (repo, piece) = match checks::branch_named(root, &ws, repo.as_deref(), &branch) {
        Ok(found) => found,
        Err(why) => {
            voice::err(why);
            return Ok(1);
        }
    };
    let place = match &piece {
        Some(piece) => Branch::piece(&ws, &repo, piece),
        None => Branch::repo(&ws, &repo),
    };
    let folder = purlis_core::files::folder(root, place, "").map_err(|e| e.to_string())?;
    let file = here.cwd.join(file);
    let tools = match checks::results_in(&file, &folder) {
        Ok(results) => said_tools(&results),
        Err(why) => {
            voice::err(checks::not_recorded(&file, &why));
            return Ok(1);
        }
    };
    if matches!(named, Named::Own)
        && let Some(code) = by_the_app(&file, &folder, &repo, piece.as_deref(), &tools, &branch)
    {
        return Ok(code);
    }
    match checks::report(root, place, &file, chrono::Utc::now()) {
        Ok(_) => {
            voice::ok(format!("{tools} recorded for {branch}"));
            Ok(0)
        }
        Err(why) => {
            voice::err(why);
            Ok(1)
        }
    }
}

/// The write handed to the app, and the code to exit with; `None` where no app took it.
fn by_the_app(
    file: &Path,
    folder: &Path,
    repo: &str,
    piece: Option<&str>,
    tools: &str,
    branch: &str,
) -> Option<u8> {
    let Some(inside) = inside(file, folder) else {
        if !crate::brokered::sandboxed() {
            return None;
        }
        voice::err(format!(
            "{} is outside {}, the branch's folder, and a sandboxed chat hands the app only a file \
             there. Nothing was recorded: write the SARIF file inside that folder, then report it \
             again",
            file.display(),
            folder.display()
        ));
        return Some(1);
    };
    let write = Write::CheckReport {
        repo: repo.to_owned(),
        piece: piece.map(str::to_owned),
        file: inside,
    };
    match crate::brokered::forwarded(write) {
        Forwarded::Written { .. } => {
            voice::ok(format!("{tools} recorded for {branch}"));
            Some(0)
        }
        Forwarded::Refused(why) | Forwarded::Unsure(why) => {
            voice::err(why);
            Some(1)
        }
        Forwarded::NotTaken(_) => None,
    }
}

/// `file`'s path inside `folder`, with `/` between its parts, or `None` if it is not there.
fn inside(file: &Path, folder: &Path) -> Option<String> {
    let file = std::fs::canonicalize(file).ok()?;
    let folder = std::fs::canonicalize(folder).ok()?;
    let rest = file.strip_prefix(folder).ok()?;
    Some(rest.to_str()?.replace(std::path::MAIN_SEPARATOR, "/"))
}

/// The repo of the piece or clone the command stands in.
fn repo_here(root: &Path, cwd: &Path) -> Option<String> {
    purlis_core::worktree::locate(root, cwd)
        .map(|at| at.repo)
        .or_else(|| purlis_core::worktree::locate_clone(root, cwd).map(|(_, repo)| repo))
}

/// `3 results from CodeQL, none from eslint`.
fn said_tools(results: &[checks::ToolResults]) -> String {
    results
        .iter()
        .map(|one| match one.diagnostics.len() {
            0 => format!("no results from {}", one.tool),
            1 => format!("1 result from {}", one.tool),
            many => format!("{many} results from {}", one.tool),
        })
        .collect::<Vec<_>>()
        .join(", ")
}
