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
    let repo = folder_of(root, rel)?;
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
    let (first, hit_by_it) = purlis_core::poollimits::attempt(|| once(&have));
    let answered = match first {
        // Sent again, what the first send hit refused nothing: only the send that answers offers.
        Err(_) if !have.is_empty() => once(&[]),
        first => {
            if let Some(hit) = hit_by_it {
                purlis_core::poollimits::hits_on(&hit.pool, &hit.hits);
            }
            first
        }
    };
    let (held, left_out) = answered.map_err(|why| {
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
        let repo = folder_of(root, &rel)?;
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
        let repo = folder_of(root, &rel)?;
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
    let (mut only_there, stayed) = made_there(link, project, root, workspace)?;
    let mut came_back = crate::host::CameBack {
        stayed,
        ..Default::default()
    };
    // An old clone named `repos` holds back only what would go in `repos/` (#5601).
    only_there.retain(|rel, _| match held_back_by_repos(root, rel) {
        Some(why) => {
            came_back.stayed.push(format!(
                "{}, which the runner's chat cloned, stayed on the runner: {why}",
                shown(rel)
            ));
            false
        }
        None => true,
    });
    // And each fresh one is checked to have room here before any clone here is taken over, so
    // one refused changes nothing.
    for rel in only_there.keys() {
        room_for(root, rel)?;
    }
    for rel in here {
        let repo = folder_of(root, &rel)?;
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
                .extend(came.into_iter().map(|path| in_clone(root, &repo, &path))),
            Err(why) => {
                return Err(format!(
                    "the work in {} did not come back from the runner: {why}",
                    shown(&rel)
                ));
            }
        }
    }
    for (rel, origin) in only_there {
        let made = codelink::new_repo_in(root, &rel)
            .map_err(|why| why.to_string())
            .and_then(|repo| {
                let scratch = scratch()?;
                let into = scratch.path().join("work.bundle");
                let (heads, wrote) = link.fetch_work(project, &rel, Vec::new(), &into)?;
                let bundle = wrote.then_some(into.as_path());
                let came = codelink::take_work_fresh(&repo, pool, bundle, &core_refs(heads))
                    .map_err(|why| why.to_string())?;
                // The clone is made with no remote: the runner's is only offered on the chat's
                // tab, or why there is none said.
                let remote = match origin {
                    Some(url) => {
                        codelink::offered_origin(pool, &url).map_err(|why| why.to_string())
                    }
                    None if link.names_clone_origins() => Err(NO_ORIGIN_NAMED.to_owned()),
                    None => Err(NO_ORIGIN_SAID.to_owned()),
                };
                Ok((repo, came, remote))
            });
        match made {
            Ok((repo, came, remote)) => {
                came_back
                    .watch
                    .extend(came.into_iter().map(|path| in_clone(root, &repo, &path)));
                came_back.made.push(crate::host::MadeHere {
                    clone: in_clone(root, &repo, ""),
                    remote,
                });
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

/// **Whether a fresh clone at `rel` can be made here** (#3811): where a new clone goes
/// ([`codelink::new_repo_in`], `repos/<repo>`, #5534) is reached through no link and holds
/// nothing of that name yet, in a workspace whose folder is here. [`codelink::take_work_fresh`]
/// holds it to that again as it makes it, and makes `repos/` where it is not there yet.
fn room_for(root: &Path, rel: &str) -> Result<(), String> {
    let refused = |why: &str| {
        format!("{rel}, which the runner's chat cloned, did not come back from the runner: {why}")
    };
    if let Some(why) = held_back_by_repos(root, rel) {
        return Err(refused(&why));
    }
    let repo = codelink::new_repo_in(root, rel).map_err(|why| refused(&why.to_string()))?;
    if std::fs::symlink_metadata(&repo).is_ok() {
        return Err(refused("something of that name is here already"));
    }
    let folder = |path: &Path| std::fs::symlink_metadata(path).is_ok_and(|meta| meta.is_dir());
    let repos = repo.parent();
    if !repos.is_some_and(|repos| folder(repos) || repos.parent().is_some_and(folder)) {
        return Err(refused("its workspace's folder is not here"));
    }
    Ok(())
}

/// **Why a runner's clone named `name` cannot come back into `workspace`** (#5601), where a
/// folder here holds that name ([`purlis_core::repofolder::held`]) that is not one of the clones
/// work moves for ([`clones_of`]): a second copy in `repos/` would hide it, and nothing moves
/// into it, so the runner's work is said to stay rather than left there in silence. `None` where
/// a clone here has the name, or nothing does.
fn held_back_here(root: &Path, workspace: &str, name: &str) -> Option<String> {
    let row = purlis_core::repofolder::held(root, workspace, name)?;
    if clones_of(root, workspace).contains(&format!("workspaces/{workspace}/{}", row.repo.name)) {
        return None;
    }
    let what = match row.git {
        purlis_core::repofolder::Git::Link => "its .git is a link",
        purlis_core::repofolder::Git::File => "its .git is a file",
        purlis_core::repofolder::Git::Directory => "purlis does not move work for it",
    };
    let at = row.repo.path.strip_prefix(root).unwrap_or(&row.repo.path);
    Some(format!(
        "{} here holds that name and is not a clone work comes back into ({what}). Move it to \
         another name, and the runner's clone can come back",
        at.display()
    ))
}

/// **Why no fresh clone at `rel` can be made here**, while an old clone named `repos` stands where
/// new clones go in its workspace (#5601), in words that fit work coming back; `None` otherwise.
fn held_back_by_repos(root: &Path, rel: &str) -> Option<String> {
    let ["workspaces", workspace, name] = rel.split('/').collect::<Vec<_>>()[..] else {
        return None;
    };
    matches!(
        purlis_core::repofolder::new_folder(root, workspace, name),
        Err(purlis_core::repofolder::Refused::RepoNamedRepos { .. })
    )
    .then(|| {
        format!(
            "workspace '{workspace}' has an old clone named repos where new clones now go. \
             Rename it (move workspaces/{workspace}/repos to another name, then run `git \
             worktree repair` in it if it has branch folders), and the runner's clone can come \
             back"
        )
    })
}

/// Why a clone made fresh has no remote where the runner named none (#3985): it names none
/// that is not a forge's address with no sign-in in it.
const NO_ORIGIN_NAMED: &str = "the runner named no origin a forge reaches for it";

/// Why a clone made fresh has no remote from a runner before session protocol 1.54, which names
/// none (#3985): one of a later purlis would.
const NO_ORIGIN_SAID: &str =
    "the runner's purlis is too old to say where its clone came from; a newer one would";

/// Each clone [`made_there`] finds, by its folder in the project, with the `origin` the runner
/// names for it.
type MadeThere = std::collections::BTreeMap<String, Option<String>>;

/// **The clones of `workspace` the runner holds and this machine has none of** (#3811), by
/// their folders in the project: what its chat cloned there, each with the `origin` URL the
/// runner names for it (1.54, #3985), its word alone, which [`codelink::offered_origin`] holds to
/// this machine's rules. None from a runner before session protocol 1.51, which cannot say. A
/// name no repository's folder may have is refused, in words; one the workspace keeps for its
/// own is never a clone, and stays there. A clone here is the runner's of that name however
/// either is cased, as this machine's disk may not tell them apart, and whatever its `.git` is
/// ([`purlis_core::repofolder::holds`], #5601): a second copy in `repos/` would hide it. Where
/// what holds the name is no clone work comes back into, the runner's clone is said to stay, in
/// the sentences answered beside ([`held_back_here`]).
fn made_there(
    link: &HostLink,
    project: &str,
    root: &Path,
    workspace: &str,
) -> Result<(MadeThere, Vec<String>), String> {
    let Some((there, mut origins)) = link
        .clones_of(project, workspace)
        .map_err(|why| format!("the runner did not say which clones it holds: {why}"))?
    else {
        return Ok(Default::default());
    };
    let mut only_there = std::collections::BTreeMap::new();
    let mut stayed = Vec::new();
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
        if let Some(why) = held_back_here(root, workspace, &name) {
            stayed.push(format!(
                "{rel}, which the runner's chat cloned, stayed on the runner: {why}"
            ));
        } else if !purlis_core::repofolder::holds(root, workspace, &name) {
            let origin = origins.remove(&name);
            only_there.insert(rel, origin);
        }
    }
    Ok((only_there, stayed))
}

/// The repositories of `workspace` in the project at `root`, in `repos/` or at its root
/// ([`purlis_core::repofolder::holdings`], #5371), each as `workspaces/<workspace>/<repo>`: the
/// name a transfer carries ([`codelink::repo_in`]), which each end finds on its own disk
/// ([`folder_of`]). Every repo, and at the root, as before, a linked worktree or a repository in
/// a name the workspace keeps.
fn clones_of(root: &Path, workspace: &str) -> Vec<String> {
    if !purlis_core::contain::workspace_name_ok(workspace) {
        return Vec::new();
    }
    let Ok(walked) = purlis_core::repofolder::holdings(root, workspace) else {
        return Vec::new();
    };
    let at_the_root = walked.others.iter().filter(|row| {
        row.layout == purlis_core::repofolder::Layout::Root
            && row.git != purlis_core::repofolder::Git::Link
    });
    let mut clones: Vec<String> = walked
        .repos
        .iter()
        .chain(at_the_root)
        .map(|row| format!("workspaces/{workspace}/{}", row.repo.name))
        .collect();
    clones.sort();
    clones
}

/// **The folder of the repository at `rel`** in the project at `root`: the project's own for an
/// empty `rel`, else, for `workspaces/<workspace>/<repo>`, where that repo is in either layout
/// ([`purlis_core::repofolder::folder`]), refused where that is refused (a workspace reached
/// through a link, a name no repo has) rather than joined by hand around it. Anything else is
/// joined as it stands, as before.
fn folder_of(root: &Path, rel: &str) -> Result<std::path::PathBuf, String> {
    if rel.is_empty() {
        return Ok(root.to_path_buf());
    }
    match rel.split('/').collect::<Vec<_>>().as_slice() {
        ["workspaces", workspace, repo] => {
            purlis_core::repofolder::folder(root, workspace, repo).map_err(|why| why.to_string())
        }
        _ => Ok(root.join(rel)),
    }
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
/// still fetch. `left` is a pool the workspace no longer runs on (#4590).
pub(crate) fn fetch_workspace(
    link: &HostLink,
    project: &str,
    root: &Path,
    workspace: &str,
    pool: &str,
    beside: bool,
    left: bool,
) -> Vec<String> {
    fetch_clones(
        link,
        project,
        root,
        &clones_of(root, workspace),
        pool,
        beside,
        left,
    )
}

/// [`fetch_workspace`], of the clones `clones`.
fn fetch_clones(
    link: &HostLink,
    project: &str,
    root: &Path,
    clones: &[String],
    pool: &str,
    beside: bool,
    left: bool,
) -> Vec<String> {
    let mut failed = Vec::new();
    for rel in clones {
        let fetched = scratch().and_then(|scratch| {
            let through = |have, into: &Path| {
                link.fetch_code(project, rel, have, into)
                    .map(|(heads, wrote)| (core_refs(heads), wrote))
            };
            let repo = folder_of(root, rel)?;
            if left {
                codelink::fetch_back_left(&repo, pool, beside, scratch.path(), through)
            } else if beside {
                codelink::fetch_back_beside(&repo, pool, scratch.path(), through)
            } else {
                codelink::fetch_back(&repo, pool, scratch.path(), through)
            }
            .map_err(|why| why.to_string())
        });
        if let Err(why) = fetched {
            failed.push(format!(
                "the runner's branches of {} were not fetched: {why}",
                shown(rel)
            ));
        }
    }
    failed
}

/// **What a chat's fresh runner holds comes back to stay** (#4569): the branches of the project's
/// own repository and of each clone of `workspace` under `refs/remotes/<runner>/`, and a clone made
/// there as a clone here, since nothing of it would be left anywhere once the runner goes. Every
/// clone here is asked for, whatever state it is in there: only the runner's word that it holds
/// no repository there, as for a clone made here since, is nothing to fetch. A runner that can't
/// say what it holds is a failure, since what it made would be lost. Answers what did not come:
/// every clone is tried. What a clone here already holds is not kept twice
/// ([`codelink::keep_only_whats_new`]).
pub(crate) fn fetch_whole(
    link: &HostLink,
    project: &str,
    root: &Path,
    workspace: &str,
    runner: &str,
) -> Vec<String> {
    // Each sentence holds what a runner said, so each is marked by purlis's one rule (#5103).
    codelink::held::marked(fetch_whole_said(link, project, root, workspace, runner))
}

/// [`fetch_whole`]'s sentences, as they are made.
fn fetch_whole_said(
    link: &HostLink,
    project: &str,
    root: &Path,
    workspace: &str,
    runner: &str,
) -> Vec<String> {
    // **What still runs there may yet commit** (#4569): a close is answered before its program
    // ends, so nothing is fetched until every program of the project there has ended.
    if let Err(why) = programs_ended(link, project, PROGRAMS_END_WITHIN) {
        return vec![why];
    }
    let there = match link.clones_of(project, workspace) {
        Ok(Some((there, _))) => there,
        Ok(None) => return vec![format!("runner {runner} is too old to say what it holds")],
        Err(why) => {
            return vec![format!(
                "the runner did not say which clones it holds: {why}"
            )];
        }
    };
    let mut here = clones_of(root, workspace);
    let mut failed = Vec::new();
    for name in there {
        if !purlis_core::contain::repo_name_ok(&name) {
            failed.push(format!(
                "the runner holds a clone named `{name}`, which can't come here"
            ));
            continue;
        }
        let rel = format!("workspaces/{workspace}/{name}");
        if purlis_core::contain::workspace_keeps(&name) {
            continue;
        }
        // Held here in any case and by any `.git`, it is not made again beside (#5601), and
        // where no work comes back into what holds it, that is said.
        if let Some(why) = held_back_here(root, workspace, &name) {
            failed.push(format!(
                "the clone {rel} did not come back from its runner: {why}"
            ));
            continue;
        }
        if purlis_core::repofolder::holds(root, workspace, &name) {
            continue;
        }
        if let Err(why) = comes_here_whole(link, project, root, &rel, runner) {
            failed.push(format!(
                "the clone {} did not come back from its runner: {why}",
                shown(&rel)
            ));
        }
    }
    if codelink::a_repository(root) {
        here.push(String::new());
    }
    failed.extend(
        fetch_clones(link, project, root, &here, runner, true, false)
            .into_iter()
            .filter(|why| !why.contains(codelink::NO_REPOSITORY_THERE)),
    );
    if failed.is_empty() {
        for rel in &here {
            if let Err(why) = folder_of(root, rel).and_then(|repo| {
                codelink::keep_only_whats_new(&repo, runner).map_err(|why| why.to_string())
            }) {
                // Only tidiness: every branch is here.
                tracing::warn!("purlis: in {}: {why}", shown(rel));
            }
        }
    }
    failed
}

/// **Only proof lets a fresh runner go** (#4569): what it holds, listed by the runner itself and
/// not the clones this machine expected, each commit reached from a ref here. Answers what is
/// missing, in sentences. Runs after [`fetch_whole`], outside the lock on fetches (#5393).
///
/// **A look that took long is taken again** (#5393): where the first took longer than the wait for
/// any answer, the runner goes only on a second that passes within that wait, so a proof is as
/// recent as when a look had only that wait. A look warms the runner's file system and what its
/// first look found of the image is kept, so a second is far quicker.
pub(crate) fn proof_of(link: &HostLink, project: &str, root: &Path, runner: &str) -> Vec<String> {
    let again_within = link.answer_within();
    codelink::held::marked(proved_recently(
        again_within,
        || not_home(link, project, root, runner),
        || not_home_within(link, project, root, runner, again_within),
    ))
}

/// [`proof_of`]'s rule: what `look` finds, and where it passed but took longer than `long`, what
/// `again` finds.
fn proved_recently(
    long: std::time::Duration,
    look: impl FnOnce() -> Vec<String>,
    again: impl FnOnce() -> Vec<String>,
) -> Vec<String> {
    let began = std::time::Instant::now();
    let missing = look();
    if missing.is_empty() && began.elapsed() > long {
        again()
    } else {
        missing
    }
}

/// **What runner `runner` holds that the project at `root` does not** (#4569), in sentences:
/// none only where the runner listed every repository it holds and this project has each one's
/// every commit ([`codelink::held::not_home`]). A runner that cannot list them proves nothing.
pub(crate) fn not_home(link: &HostLink, project: &str, root: &Path, runner: &str) -> Vec<String> {
    not_home_within(link, project, root, runner, link.proof_within())
}

/// [`not_home`], waiting at most `within` for the runner's list.
pub(crate) fn not_home_within(
    link: &HostLink,
    project: &str,
    root: &Path,
    runner: &str,
    within: std::time::Duration,
) -> Vec<String> {
    let (repos, unread) = match link.repos_held_within(within) {
        Ok(Some(held)) => held,
        Ok(None) => {
            return vec![format!(
                "runner {runner} is too old to list what it holds, so purlis can't tell its work is here"
            )];
        }
        // What a runner said is marked as the proof's sentences are (#5103).
        Err(why) => {
            return codelink::held::marked(vec![format!(
                "the runner did not list what it holds: {why}"
            )]);
        }
    };
    let held: Vec<codelink::held::Held> = repos
        .into_iter()
        .map(|one| codelink::held::Held {
            project: one.project,
            path: one.path,
            refs: core_refs(one.refs),
            changed: one
                .changed
                .into_iter()
                .map(|change| codelink::held::Changed {
                    path: change.path,
                    now: change.now,
                })
                .collect(),
        })
        .collect();
    codelink::held::not_home(root, project, &held, &unread)
}

/// How long a fresh runner's end waits for the programs of its project there to end (#4569).
pub(crate) const PROGRAMS_END_WITHIN: std::time::Duration = std::time::Duration::from_secs(60);

/// **Whether every program of `project` on the host `link` reaches has ended**, waiting at most
/// `within` (#4569): one still running may commit after a fetch, so its runner is not let go of
/// meanwhile. The host answers this window's programs alone: another checkout's task is kept
/// from this window by its row's checkout ([`purlis_core::runners::FreshOf::is_of`]).
pub(crate) fn programs_ended(
    link: &HostLink,
    project: &str,
    within: std::time::Duration,
) -> Result<(), String> {
    use purlis_session_protocol::session::{Answer, Command};
    let until = std::time::Instant::now() + within;
    loop {
        let running = match link.call(Command::ProgramsOf {
            project: project.to_owned(),
        }) {
            Ok(Answer::Programs(programs)) => still_running(&programs),
            Ok(_) => return Err("the runner did not say what still runs there".to_owned()),
            Err(why) => {
                return Err(format!(
                    "the runner did not say what still runs there: {why}"
                ));
            }
        };
        if running == 0 {
            return Ok(());
        }
        if std::time::Instant::now() >= until {
            return Err(running_said(running));
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

/// How many of `programs` have not ended.
fn still_running(programs: &[purlis_session_protocol::session::HeldProgram]) -> usize {
    programs
        .iter()
        .filter(|program| program.exit.is_none())
        .count()
}

/// Why a runner whose `running` programs have not ended is not let go of.
fn running_said(running: usize) -> String {
    format!(
        "{} of this project {} on the runner, and could still commit",
        purlis_core::counts::counted(running, "program", "programs"),
        purlis_core::counts::plural(running, "still runs", "still run"),
    )
}

/// The clone `rel`, made on the runner, as a clone here.
fn comes_here_whole(
    link: &HostLink,
    project: &str,
    root: &Path,
    rel: &str,
    runner: &str,
) -> Result<(), String> {
    room_for(root, rel)?;
    let repo = codelink::new_repo_in(root, rel).map_err(|why| why.to_string())?;
    let scratch = scratch()?;
    let into = scratch.path().join("work.bundle");
    let (heads, wrote) = link.fetch_work(project, rel, Vec::new(), &into)?;
    codelink::take_work_fresh(
        &repo,
        runner,
        wrote.then_some(into.as_path()),
        &core_refs(heads),
    )
    .map(|_| ())
    .map_err(|why| why.to_string())
}

/// `path` in the clone at `repo`, by its path in the project, wherever this machine holds the
/// clone (#5534): the clone itself for an empty one, what [`codelink::take_work_back`] names
/// where what came could not be listed.
fn in_clone(root: &Path, repo: &Path, path: &str) -> String {
    let rel = repo.strip_prefix(root).unwrap_or(repo).to_string_lossy();
    match (rel.is_empty(), path.is_empty()) {
        (_, true) => rel.into_owned(),
        (true, false) => path.to_owned(),
        (false, false) => format!("{rel}/{path}"),
    }
}

fn shown(rel: &str) -> &str {
    if rel.is_empty() { "the project" } else { rel }
}

#[cfg(test)]
mod tests;
