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
    let leave_out = leave_out_of(root, rel)?;
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
            "purlis: {} of {} not sent to the runner, as a save to the forge would refuse them",
            purlis_core::counts::counted(left_out.len(), "file", "files"),
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

/// What a send of the repository at `rel` leaves out: purlis's state for the project's own; for
/// a workspace's clone, what the person named in the workspace's settings
/// (`settings.runners.leave_out`).
fn leave_out_of(root: &Path, rel: &str) -> Result<Vec<String>, String> {
    if rel.is_empty() {
        return Ok(codelink::plane_leaves_out());
    }
    match rel
        .strip_prefix("workspaces/")
        .and_then(|rest| rest.split('/').next())
    {
        // A list that cannot be read is never taken as one that names nothing: the send is
        // refused in its words, and what the person listed stays here.
        Some(workspace) => Ok(
            purlis_core::settings::workspace::send_leaves_out(root, workspace)
                .map_err(|why| {
                    format!(
                        "the code of {} was not sent to the runner: {why}. Mend the list in the \
                     workspace's settings, Sent to runners",
                        shown(rel)
                    )
                })?
                .into_iter()
                // A folder is named by its path: git's pathspec takes it without the slash.
                .map(|entry| entry.trim_end_matches('/').to_owned())
                .collect(),
        ),
        None => Ok(Vec::new()),
    }
}

/// **Send every clone of `workspace` ahead of a move** (#2048): each clone's branch, committed,
/// with nothing checked out on the runner, so the move itself carries only what changed since.
/// A send ahead that fails moves nothing and is said: the move carries it all then.
pub(crate) fn send_ahead(
    link: &HostLink,
    project: &str,
    root: &Path,
    workspace: &str,
    sent: &Sent,
) -> Result<(), String> {
    for rel in clones_of(root, workspace) {
        let repo = root.join(&rel);
        let Some(branch) = codelink::current_branch(&repo) else {
            continue;
        };
        let have = lock(sent).get(&rel).cloned().unwrap_or_default();
        let scratch = scratch()?;
        let into = scratch.path().join("ahead.bundle");
        let ahead = codelink::bundle_branch(&repo, None, &branch, &have, &into, scratch.path())
            .map_err(|why| why.to_string())
            .and_then(|bundled| {
                let bundle =
                    (bundled.bundled == codelink::Bundled::Wrote).then_some(into.as_path());
                link.send_code(project, &rel, code_refs(bundled.refs), None, bundle)
            });
        match ahead {
            Ok(held) => {
                lock(sent).insert(rel, held.into_iter().map(|one| one.oid).collect());
            }
            Err(why) => {
                lock(sent).remove(&rel);
                return Err(format!(
                    "the code of {} did not reach the runner: {why}",
                    shown(&rel)
                ));
            }
        }
    }
    Ok(())
}

/// **Moves the work tree of every clone of `workspace` to the runner** (#2048), as its chat
/// moves there: each clone's branch, its commits and its uncommitted work, less what its sends
/// leave out, taken over onto the runner's work tree where it is still what both ends last
/// agreed. The first clone the runner refuses stops the move, in its words.
pub(crate) fn move_workspace_to(
    link: &HostLink,
    project: &str,
    root: &Path,
    workspace: &str,
    sent: &Sent,
) -> Result<(), String> {
    for rel in clones_of(root, workspace) {
        let repo = root.join(&rel);
        let leave_out = leave_out_of(root, &rel)?;
        let have = lock(sent).get(&rel).cloned().unwrap_or_default();
        let scratch = scratch()?;
        let into = scratch.path().join("work.bundle");
        let moved = codelink::work_of(&repo, scratch.path(), &leave_out)
            .and_then(|work| {
                codelink::bundle_work(&repo, &work, &have, &into).map(|bundled| (work, bundled))
            })
            .map_err(|why| why.to_string())
            .and_then(|(work, (refs, bundled))| {
                let bundle = (bundled == codelink::Bundled::Wrote).then_some(into.as_path());
                let checkout = format!("{}{}", codelink::HEADS, work.branch);
                let held = link.send_work(project, &rel, code_refs(refs), checkout, bundle)?;
                codelink::sent(&repo, &work).map_err(|why| why.to_string())?;
                Ok(held)
            });
        match moved {
            Ok(held) => {
                lock(sent).insert(rel, held.into_iter().map(|one| one.oid).collect());
            }
            Err(why) => {
                lock(sent).remove(&rel);
                return Err(format!(
                    "the work in {} did not move to the runner: {why}",
                    shown(&rel)
                ));
            }
        }
    }
    Ok(())
}

/// **Moves the work tree of every clone of `workspace` back from the runner** (#2048), as its
/// chat moves here: the runner's branches into `refs/remotes/<pool>/*`, and this clone's work
/// tree taken over onto the runner's, its branch, commits and uncommitted work, where it is
/// still what both ends last agreed. The first clone that is not stops the move, in its words.
/// A clone the runner chat made there, which this machine has none of, is made here fresh from
/// the runner's work (#3811, [`codelink::take_work_fresh`]), where the runner names its clones
/// (session protocol 1.51); a runner before it cannot, and such a clone stays there. Answers
/// what came back that the person is to look at, by its path in the project
/// ([`codelink::take_work_back`]), and each clone made fresh.
pub(crate) fn move_workspace_back(
    link: &HostLink,
    project: &str,
    root: &Path,
    workspace: &str,
    pool: &str,
) -> Result<crate::host::CameBack, String> {
    let here = clones_of(root, workspace);
    // Asked first, so a runner that cannot say changes nothing here.
    let only_there = made_there(link, project, workspace, &here)?;
    // And each fresh one is checked to have room here before any clone here is taken over, so
    // one refused changes nothing.
    for rel in only_there.keys() {
        room_for(root, rel)?;
    }
    let mut came_back = crate::host::CameBack::default();
    for rel in here {
        let repo = root.join(&rel);
        let leave_out = leave_out_of(root, &rel)?;
        let scratch = scratch()?;
        let into = scratch.path().join("work.bundle");
        let have: Vec<String> = codelink::held_refs(&repo)
            .map(|refs| refs.into_iter().map(|one| one.oid).collect())
            .unwrap_or_default();
        let moved = link
            .fetch_work(project, &rel, have, &into)
            .and_then(|(heads, wrote)| {
                let bundle = wrote.then_some(into.as_path());
                codelink::take_work_back(
                    &repo,
                    scratch.path(),
                    &leave_out,
                    pool,
                    bundle,
                    &core_refs(heads),
                )
                .map_err(|why| why.to_string())
            });
        match moved {
            Ok(came) => came_back
                .watch
                .extend(came.into_iter().map(|path| in_clone(&rel, &path))),
            Err(why) => {
                return Err(format!(
                    "the work in {} did not come back from the runner: {why}",
                    shown(&rel)
                ));
            }
        }
    }
    for (rel, origin) in only_there {
        let made = codelink::repo_in(root, &rel)
            .map_err(|why| why.to_string())
            .and_then(|repo| {
                let scratch = scratch()?;
                let into = scratch.path().join("work.bundle");
                let (heads, wrote) = link.fetch_work(project, &rel, Vec::new(), &into)?;
                let bundle = wrote.then_some(into.as_path());
                let came = codelink::take_work_fresh(&repo, pool, bundle, &core_refs(heads))
                    .map_err(|why| why.to_string())?;
                // The clone is made: a remote not given leaves it here, without one, and says
                // why on the chat's tab.
                let remote = match origin {
                    Some(url) => codelink::give_origin(&repo, pool, &url)
                        .map(|()| url)
                        .map_err(|why| why.to_string()),
                    None => Err(NO_ORIGIN_NAMED.to_owned()),
                };
                Ok((came, remote))
            });
        match made {
            Ok((came, remote)) => {
                came_back
                    .watch
                    .extend(came.into_iter().map(|path| in_clone(&rel, &path)));
                came_back
                    .made
                    .push(crate::host::MadeHere { clone: rel, remote });
            }
            Err(why) => {
                return Err(format!(
                    "{rel}, which the runner's chat cloned, did not come back from the runner: \
                     {why}"
                ));
            }
        }
    }
    Ok(came_back)
}

/// **Whether a fresh clone at `rel` can be made here** (#3811): its folder in the project is
/// reached through no link and holds nothing of that name yet. [`codelink::take_work_fresh`]
/// holds it to that again as it makes it.
fn room_for(root: &Path, rel: &str) -> Result<(), String> {
    let refused = |why: &str| {
        format!("{rel}, which the runner's chat cloned, did not come back from the runner: {why}")
    };
    let repo = codelink::repo_in(root, rel).map_err(|why| refused(&why.to_string()))?;
    if std::fs::symlink_metadata(&repo).is_ok() {
        return Err(refused("something of that name is here already"));
    }
    if !repo
        .parent()
        .is_some_and(|parent| std::fs::symlink_metadata(parent).is_ok_and(|meta| meta.is_dir()))
    {
        return Err(refused("its workspace's folder is not here"));
    }
    Ok(())
}

/// Why a clone made fresh has no remote where the runner named none it could (#3985): a runner
/// before session protocol 1.54 names none, and one of it names none that is not a forge's
/// address with no sign-in in it.
const NO_ORIGIN_NAMED: &str = "the runner named no origin a forge reaches for it";

/// **The clones of `workspace` the runner holds and this machine has none of** (#3811), by
/// their folders in the project: what its chat cloned there, each with the `origin` URL the
/// runner names for it (1.54, #3985), its word alone, which [`codelink::give_origin`] holds to
/// this machine's rules. None from a runner before session protocol 1.51, which cannot say. A
/// name no repository's folder may have is refused, in words; one the workspace keeps for its
/// own is never a clone, and stays there. A clone here is the runner's of that name however
/// either is cased, as this machine's disk may not tell them apart.
fn made_there(
    link: &HostLink,
    project: &str,
    workspace: &str,
    here: &[String],
) -> Result<std::collections::BTreeMap<String, Option<String>>, String> {
    let Some((there, mut origins)) = link
        .clones_of(project, workspace)
        .map_err(|why| format!("the runner did not say which clones it holds: {why}"))?
    else {
        return Ok(std::collections::BTreeMap::new());
    };
    let mut only_there = std::collections::BTreeMap::new();
    for name in there {
        if !purlis_core::contain::repo_name_ok(&name) {
            return Err(format!(
                "the runner named a clone `{name}`, which is not a repository's folder name, so \
                 nothing came back"
            ));
        }
        if purlis_core::contain::workspace_keeps(&name) {
            continue;
        }
        let rel = format!("workspaces/{workspace}/{name}");
        if !here.iter().any(|one| one.eq_ignore_ascii_case(&rel)) {
            let origin = origins.remove(&name);
            only_there.insert(rel, origin);
        }
    }
    Ok(only_there)
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

/// `path` in the clone at `rel`, by its path in the project: the clone itself for an empty one,
/// what [`codelink::take_work_back`] names where what came could not be listed.
fn in_clone(rel: &str, path: &str) -> String {
    if path.is_empty() {
        rel.to_owned()
    } else {
        format!("{rel}/{path}")
    }
}

fn shown(rel: &str) -> &str {
    if rel.is_empty() { "the project" } else { rel }
}

#[cfg(test)]
mod tests;
