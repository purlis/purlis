// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A project's code on the runner its chats run on** (#2046, ADR 0078 §7): what the window
//! sends a pool's runner over the link it holds the project on, and what it fetches back.
//!
//! - **As it holds the project there**, the window sends the project's own repository: the
//!   branch it is on and its uncommitted work, less purlis's state folder and the local settings
//!   file ([`purlis_core::codelink::plane_leaves_out`]). The runner checks it out once, in
//!   `<data>/projects/<project id>`, so its chats find the project's personas, workspaces and
//!   memory as here.
//! - **Before a chat starts in a workspace there**, the window sends each of that workspace's
//!   clones the same way, so the chat works on the code as it is here, uncommitted work and all.
//! - **It fetches each clone's branches back** into `refs/remotes/<pool>/*`, and nowhere else,
//!   where Changes shows them ready to push or open as a pull request here.
//!
//! What the runner held after each send is kept for the next, so a send carries only what the
//! runner lacks. It is forgotten after a send that failed, and whenever the project is held
//! again, so the next send is whole. The git calls are the core's ([`purlis_core::codelink`]).

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use purlis_core::codelink::{self, Ref};
use purlis_session_protocol::session::CodeRef;

use crate::hostedsessions::HostLink;

/// What a runner held of each repository after the last send, by its folder in the project
/// (empty for the project's own): the commits a next send leaves out.
pub(crate) type Sent = Mutex<HashMap<String, Vec<String>>>;

fn code_refs(refs: Vec<Ref>) -> Vec<CodeRef> {
    refs.into_iter()
        .map(|one| CodeRef {
            name: one.name,
            oid: one.oid,
        })
        .collect()
}

fn core_refs(refs: Vec<CodeRef>) -> Vec<Ref> {
    refs.into_iter()
        .map(|one| Ref {
            name: one.name,
            oid: one.oid,
        })
        .collect()
}

fn lock(sent: &Sent) -> std::sync::MutexGuard<'_, HashMap<String, Vec<String>>> {
    sent.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// **Forget what the runner held of every repository**, so the next send of each is whole: the
/// project is held again, and the runner may hold none of it.
pub(crate) fn forget(sent: &Sent) {
    lock(sent).clear();
}

fn scratch() -> Result<tempfile::TempDir, String> {
    tempfile::Builder::new()
        .prefix("purlis-code-")
        .tempdir()
        .map_err(|why| format!("there is no room on this machine to bundle the code: {why}"))
}

/// **Send the repository at `rel` in the project at `root`** to the runner `link` holds
/// `project` on: the project's own for an empty `rel`, else `workspaces/<workspace>/<repo>`.
pub(crate) fn send(
    link: &HostLink,
    project: &str,
    root: &Path,
    rel: &str,
    sent: &Sent,
) -> Result<(), String> {
    let repo = if rel.is_empty() {
        root.to_path_buf()
    } else {
        root.join(rel)
    };
    // The project's own leaves out purlis's state; a workspace's clone, what the person named
    // in the workspace's settings (`settings.runners.leave_out`).
    let leave_out = if rel.is_empty() {
        codelink::plane_leaves_out()
    } else {
        rel.strip_prefix("workspaces/")
            .and_then(|rest| rest.split('/').next())
            .map(|workspace| {
                // A folder is named by its path: git's pathspec takes it without the slash.
                purlis_core::settings::workspace::send_leaves_out(root, workspace)
                    .into_iter()
                    .map(|entry| entry.trim_end_matches('/').to_owned())
                    .collect()
            })
            .unwrap_or_default()
    };
    let have = lock(sent).get(rel).cloned().unwrap_or_default();
    let once = |have: &[String]| -> Result<(Vec<Ref>, Vec<String>), String> {
        let scratch = scratch()?;
        codelink::send(&repo, &leave_out, have, scratch.path(), |sending| {
            link.send_code(
                project,
                rel,
                code_refs(sending.refs),
                Some(sending.checkout),
                sending.bundle,
            )
            .map(core_refs)
        })
        .map_err(|why| why.to_string())
    };
    // A send that leaves out what the runner held last may fail where the runner no longer holds
    // it: that send is made again whole, and what the runner held is not known after a send that
    // failed, so the next one is whole too.
    let (held, left_out) = once(&have)
        .or_else(|why| if have.is_empty() { Err(why) } else { once(&[]) })
        .map_err(|why| {
            lock(sent).remove(rel);
            format!("the code of {} did not reach the runner: {why}", shown(rel))
        })?;
    // Named where the person looks: Changes, under the clone (`panels::left_out_of_send`).
    if !left_out.is_empty() {
        tracing::info!(
            "purlis: {} file(s) of {} were not sent to the runner, as a save to the forge would \
             refuse them",
            left_out.len(),
            shown(rel)
        );
    }
    crate::panels::sent_leaving(&repo, left_out);
    lock(sent).insert(
        rel.to_owned(),
        held.into_iter().map(|one| one.oid).collect(),
    );
    Ok(())
}

/// The repositories of `workspace` in the project at `root`, by their folders in it.
fn clones_of(root: &Path, workspace: &str) -> Vec<String> {
    if !purlis_core::contain::workspace_name_ok(workspace) {
        return Vec::new();
    }
    let Ok(entries) = std::fs::read_dir(root.join("workspaces").join(workspace)) else {
        return Vec::new();
    };
    let mut clones: Vec<String> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| purlis_core::contain::repo_name_ok(name))
        .filter(|name| codelink::a_repository(&root.join("workspaces").join(workspace).join(name)))
        .map(|name| format!("workspaces/{workspace}/{name}"))
        .collect();
    clones.sort();
    clones
}

/// **Send every clone of `workspace`** to the runner, before a chat starts there.
pub(crate) fn send_workspace(
    link: &HostLink,
    project: &str,
    root: &Path,
    workspace: &str,
    sent: &Sent,
) -> Result<(), String> {
    for rel in clones_of(root, workspace) {
        send(link, project, root, &rel, sent)?;
    }
    Ok(())
}

/// **Fetch the branches of every clone of `workspace` back from the runner**, into each clone's
/// `refs/remotes/<pool>/*`. Each clone fetches on its own: one that fails is said, and the rest
/// still fetch.
pub(crate) fn fetch_workspace(
    link: &HostLink,
    project: &str,
    root: &Path,
    workspace: &str,
    pool: &str,
) -> Vec<String> {
    let mut failed = Vec::new();
    for rel in clones_of(root, workspace) {
        let fetched = scratch().and_then(|scratch| {
            codelink::fetch_back(&root.join(&rel), pool, scratch.path(), |have, into| {
                link.fetch_code(project, &rel, have, into)
                    .map(|(heads, wrote)| (core_refs(heads), wrote))
            })
            .map_err(|why| why.to_string())
        });
        if let Err(why) = fetched {
            failed.push(format!(
                "the runner's branches of {} were not fetched: {why}",
                shown(&rel)
            ));
        }
    }
    failed
}

fn shown(rel: &str) -> &str {
    if rel.is_empty() { "the project" } else { rel }
}

#[cfg(test)]
mod tests;
