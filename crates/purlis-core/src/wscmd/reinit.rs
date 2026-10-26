// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `charter workspace reinit` — bring a workspace's on-disk structure and charter's layer up
//! to what this version writes.
//!
//! Idempotent and additive: existing content is never touched. `--all` repairs every
//! workspace at once, which is what an operator runs after a charter upgrade.
//!
//! # Why the layer is its own set of lines
//!
//! A workspace whose LAYOUT is current can still hold a `.claude/settings.json` generated
//! before the plane's own settings moved. Two facts, so two reports — and the layer is the
//! one this command silently repaired while printing "nothing to do" until it was said out
//! loud.
//!
//! # The checkouts inside it, too
//!
//! A clone or a linked worktree under the workspace is a git root of its own: a chat started
//! there reads project settings from that directory and the walk for agents and skills stops
//! at the git root, so without charter's layer it runs with none of the plane's ask/deny
//! rules, no persona agents and no `$CHARTER_HARNESS`. [`crate::wslayer::wire`] wires each
//! one and its rows arrive here labelled `<checkout>/<path>`, so this command is the repair
//! for a clone an older charter made as well as for the directory around it.
//!
//! **A checkout's file of the operator's is worded differently, and that is not decoration.**
//! Only a checkout is somebody else's repository, where charter's line in `info/exclude`
//! keeps their file hidden while it is there — so the one true piece of advice is `git add
//! -f`, and "move it aside", which the workspace directory's own row says, would be wrong.
//!
//! # Two counters, because they are two units
//!
//! A workspace can need several repairs — the layer is a row per file, the structure bump is
//! another — so a single counter printed against the number of workspaces said *"Healed 32 of
//! 17 workspace(s); the rest were current."* on a 17-workspace plane. `repaired` is a SET OF
//! NAMES drawn from the names walked, so no arithmetic below is in a position to state a count
//! greater than the total.

use std::collections::BTreeSet;
use std::path::Path;

use crate::counts::counted;
use crate::repocmd::{Say, Sink};
use crate::wscmd::{self, ensure};
use crate::wslayer::{self, Did, Why};

/// The baseline files a LIVE workspace actually SHARES. Healing only `refs/` or the stamp
/// leaves nothing to commit, and advising `save` then sends the operator to a command that
/// prints "Nothing to save".
const LIVE_SHARED: [&str; 3] = ["workspace.md", "workspace.json", "memory/MEMORY.md"];

/// Which workspaces to repair.
pub enum Scope<'a> {
    /// One, by name.
    One(&'a str),
    /// Every workspace this plane has.
    All,
}

/// Run it, and give back the exit code.
pub fn reinit(root: &Path, scope: Scope, now: chrono::DateTime<chrono::Utc>, say: Sink) -> u8 {
    let plane = crate::workspaces::Plane::open(root);
    let author = ensure::author();
    // Workspaces `--all` could not look at, named here and counted in the closing line. A
    // listing that leaves such a directory out is a walk that never met it, and the line
    // under it said "Up to date — nothing to do" about the whole plane.
    let mut unseen = 0usize;
    let names: Vec<String> = match scope {
        Scope::All => {
            let (names, unread) = plane.read_workspaces().unwrap_or_default();
            for (path, code) in &unread {
                unseen += 1;
                say(Say::Fail(wscmd::cannot_check_workspace(path, *code).into()));
            }
            names
        }
        Scope::One(name) => {
            let Ok(workspace) = plane.workspace(name) else {
                say(Say::Fail(
                    format!(
                        "invalid workspace name '{}'",
                        crate::personas::one_line(name)
                    )
                    .into(),
                ));
                return 1;
            };
            // Through the link, and the errno kept: a directory that is a symlink loop is not
            // "no workspace", and telling its operator to restore read access clears no loop.
            match workspace.dir().metadata() {
                Ok(_) => vec![name.to_string()],
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
                    ) =>
                {
                    say(Say::Fail(format!("no workspace '{name}'").into()));
                    return 1;
                }
                Err(e) => {
                    say(Say::Fail(
                        wscmd::cannot_check_workspace(workspace.dir(), e.raw_os_error()).into(),
                    ));
                    return 1;
                }
            }
        }
    };
    if names.is_empty() && unseen == 0 {
        say(Say::Info(
            "No workspaces to reinitialize.".to_string().into(),
        ));
        return 0;
    }

    let mut repairs = 0usize;
    let mut repaired: BTreeSet<String> = BTreeSet::new();
    // Workspaces whose manifest `reinit` was asked for and could not write. Kept apart from
    // `repaired` so the closing line does not say "nothing to do" — or, worse, "the rest were
    // current" — over an error two lines above it.
    let mut blocked: BTreeSet<String> = BTreeSet::new();
    // Workspaces left holding a state a row names and `reinit` cannot clear: a file charter
    // did not write, one it cannot read or write. "Up to date — nothing to do" printed beside
    // one of them told the operator the plane's rules were in force where they were not.
    let mut unresolved: BTreeSet<String> = BTreeSet::new();

    for name in &names {
        let Ok(workspace) = plane.workspace(name) else {
            continue;
        };
        let dir = workspace.dir().to_path_buf();
        let mut before = wslayer::structure_status(&dir);

        // The layer, written HERE rather than left to the scaffold's own call, because the
        // repair has to report what it DID and only the writer knows that: a `.claude` that
        // cannot be made comes back `blocked`, and reading the pre-state instead would print
        // "wrote it" over a write that never happened. The scaffold below re-runs it and gets
        // `present` for everything, which is what idempotent means.
        let layer: Vec<_> = wslayer::wire(root, &dir)
            .into_iter()
            .filter(|row| row.did != Did::Present)
            .collect();
        wslayer::scaffold(&plane, name, now, &author);
        // Structure is not only what lives inside the workspace directory: which of its paths
        // are SHARED is part of the layout too, and that lives in the managed `.gitignore`
        // block. A plane made LIVE before `todos/` existed lists four paths per workspace and
        // nothing re-runs `set_live` unprompted, so the upgrade command is where it is
        // repaired.
        let _ = wscmd::refresh_live_block(root);

        for row in &layer {
            let rel = &row.rel;
            // Counted off the ROW's own predicates, never off the arm that happens to print:
            // "is this a repair" and "what does this row say" must not be able to disagree,
            // which is what a per-arm `repairs += 1` invites the next arm to get wrong.
            if row.did.is_unresolved() {
                unresolved.insert(name.clone());
            }
            if row.did.is_repair() {
                repairs += 1;
                repaired.insert(name.clone());
            }
            match row.did {
                // Never "remove it" in a checkout: the file is somebody's own in their own
                // repository and stays hidden while it is there, so the one thing to say is
                // how to commit it on purpose. Everywhere else `doctor` says a foreign file
                // stays exactly as it is, and one command may not advise what the other
                // rules out.
                Did::Foreign => match wslayer::checkout_row(&dir, rel) {
                    Some((_, inside)) => say(Say::Warn(
                        format!(
                            "'{name}': {rel} holds content purlis did not write — left completely \
                         untouched, and hidden in that checkout while it is there; if this is \
                         your own file and you mean to commit it: git add -f {inside}"
                        )
                        .into(),
                    )),
                    None => say(Say::Warn(
                        format!(
                            "'{name}': {rel} was not written by purlis — left completely \
                         untouched; purlis never overwrites it."
                        )
                        .into(),
                    )),
                },
                // Not `foreign` and never "remove it": the approvals in that file are the
                // harness's own, and deleting it to get charter's copy back destroys them.
                Did::HarnessBehind => say(Say::Warn(
                    format!(
                        "'{name}': {rel} is a file purlis cannot vouch for as its own write, and \
                     the harness saves its approvals into that file, so purlis does not \
                     rewrite it: the plane's machine-local rules it lacks are not in force \
                     there."
                    )
                    .into(),
                )),
                Did::Blocked => say(Say::Fail(
                    format!(
                        "'{name}': {rel} could not be written — something is in the way at that \
                     path. purlis never deletes or renames existing content."
                    )
                    .into(),
                )),
                Did::Unreadable => say(Say::Warn(
                    format!(
                        "'{name}': {rel} cannot be read — left exactly as it is; purlis writes \
                     there again only if that file turns out to be exactly what purlis last \
                     wrote."
                    )
                    .into(),
                )),
                // A sentence of its own: nothing is in the way at that path, so `blocked`'s
                // wording would send the operator looking for an obstruction that is not
                // there. What stopped the write is that the file could not be hidden.
                Did::Withheld => say(Say::Fail(
                    format!(
                        "'{name}': {rel} was not written — purlis could not hide it in that \
                     checkout's .git/info/exclude, and a machine-local file it cannot hide \
                     would be committable there."
                    )
                    .into(),
                )),
                // #1583: the record names it, the project's own note of what it offered does
                // not. Its own sentence, never `foreign`'s: it may be purlis's older copy. Not
                // a repair `reinit` can make — the same gate stands here — so the row names
                // what does: the file moved aside, this command writes the current text.
                Did::Unconfirmed => say(Say::Warn(
                    format!(
                        "'{name}': purlis has newer text for {rel} but cannot confirm it wrote the \
                     file there now, so it is left as is. If it is purlis's own older copy, move \
                     it aside and run `purlis workspace reinit {name}` to write the current one."
                    )
                    .into(),
                )),
                // charter#1072: every worktree of that clone went unchecked, and "nothing to
                // do" printed over them. Not a repair either, and not one `reinit` can make.
                Did::Unlisted => {
                    let tree = wslayer::checkout_row(&dir, rel).map(|(tree, _)| tree);
                    let (label, fix) = match &tree {
                        Some(tree) => (
                            wslayer::checkout_label(&dir, tree),
                            wslayer::unlisted_fix(tree),
                        ),
                        None => (rel.clone(), String::new()),
                    };
                    say(Say::Warn(format!(
                        "'{name}': git could not list the worktrees of {label}, so none of them \
                         was checked or given purlis's layer; {fix}."
                    ).into()));
                }
                // charter#1072: a line left out of a shared exclude because it would hide a
                // file of yours in another checkout reading it. Not a repair — the
                // `wrote`/`refreshed` pair below would call it "wrote .git/info/exclude" —
                // and not one `reinit` can make: the file is yours to commit or move.
                Did::Unhidden => {
                    let why = wslayer::checkout_row(&dir, rel)
                        .map(|(tree, _)| crate::guest::unhidden(root, &tree))
                        .unwrap_or_default();
                    say(Say::Warn(
                        format!(
                            "'{name}': {rel} does not hide all of purlis's files in that checkout \
                         — {}.",
                            why.join("; ")
                        )
                        .into(),
                    ));
                }
                // charter's ruling H: a write charter could not record first is not made, and
                // the lines stay. Not `blocked`: nothing is in the way at that path. With the
                // errno the publish failed with.
                Did::Unrecorded => {
                    let refused = wslayer::checkout_row(&dir, rel)
                        .map(|(tree, _)| crate::guest::unrecorded_fix(root, &tree, "that checkout"))
                        .unwrap_or_default();
                    let tail = if refused.is_empty() {
                        String::new()
                    } else {
                        format!("; {refused}")
                    };
                    say(Say::Warn(
                        format!(
                            "'{name}': {rel} — purlis could not publish its record there first, so \
                         it wrote nothing and kept every exclude line it had{tail}."
                        )
                        .into(),
                    ));
                }
                Did::Removed => {
                    // Its own sentence rather than the `wrote`/`refreshed` pair below, which
                    // would call a deletion "refreshed". A removal is the one repair here
                    // whose CAUSE the operator cannot see in the workspace — the plane stopped
                    // declaring it — so the row has to carry it.
                    say(Say::Done(
                        format!(
                            "Reinitialized '{name}' → removed {rel} — the plane no longer declares \
                         it (purlis's harness layer)."
                        )
                        .into(),
                    ));
                }
                Did::Created | Did::Refreshed => {
                    let what = if row.did == Did::Created {
                        "wrote"
                    } else {
                        "refreshed"
                    };
                    say(Say::Done(
                        format!("Reinitialized '{name}' → {what} {rel} (purlis's harness layer).")
                            .into(),
                    ));
                }
                Did::Present => {}
            }
        }

        for found in &before.unreadable {
            // A baseline file charter cannot check is named with what clears it, and the
            // scaffold writes nothing over it. What clears it is the errno's, not one sentence
            // for every cause: "restore read access" for a symlink loop is a repair no
            // permission bit is in the way of.
            unresolved.insert(name.clone());
            // The path is where the errno was MET, which is the rel's own unless a directory
            // above it is what fails — `doctor` names `refs/` where the loop is, and this must
            // not name `refs/README.md`, which is no link at all.
            let stopped = wslayer::stopped_at(&found.path, found.code);
            let shown =
                crate::shown::readable(&stopped.to_string_lossy(), crate::memstore::PATH_LIMIT);
            say(Say::Warn(
                format!(
                    "'{name}': {} cannot be checked — purlis writes nothing there it cannot see; \
                 {}.",
                    found.rel,
                    crate::memstore::uncheckable_fix(found.code, &shown, "that path")
                )
                .into(),
            ));
        }

        for (rel, blocker) in &before.in_the_way {
            // The shapes a path of the layout takes when it is not what the layout needs.
            // "added" would be as wrong as a traceback: the scaffold creates nothing beneath
            // either, and removes neither, so the row says which one was found and the repair
            // that is the operator's to make.
            unresolved.insert(name.clone());
            let fix = match blocker.why {
                Why::Dangling => "is a symlink whose target is not there, and purlis writes \
                                  nothing through it; removing or repointing that link clears \
                                  this"
                    .to_string(),
                Why::NotADirectory => "is not a directory, and purlis never moves existing \
                                       content; moving it out of the way clears this"
                    .to_string(),
                Why::IsALink => {
                    // Never "repoint it" for a FILE: no target makes a file link one charter
                    // writes through. What it becomes is what the layout has at that path.
                    let real = if blocker.path == dir.join(rel) {
                        "file"
                    } else {
                        "directory"
                    };
                    format!(
                        "is a symlink, and purlis writes nothing through one; replacing it \
                         with a real {real} clears this"
                    )
                }
                Why::IsADirectory | Why::NotARegularFile => {
                    "is not a regular file, and purlis never moves existing content; moving \
                     it out of the way clears this"
                        .to_string()
                }
            };
            // The stamp is written over, not created, when it is there: "could not be written"
            // is what happened to it, and it is the sentence the version line below gives way
            // to.
            let did = if rel.as_str() == wslayer::STRUCTURE_MARKER {
                "could not be written"
            } else {
                "cannot be created"
            };
            let shown = crate::shown::readable(
                &blocker.path.to_string_lossy(),
                crate::memstore::PATH_LIMIT,
            );
            say(Say::Warn(
                format!("'{name}': {rel} {did} — {shown} {fix}.").into(),
            ));
        }

        // The BACKFILL half, and the reason it is checked AFTER rather than read off the
        // pre-state: `scaffold_manifest` swallows its own failure, because it runs on a launch
        // path where raising would cost the operator their tab. That is the right trade there
        // and it costs this command its honesty unless the file is looked at again — "added
        // workspace.json" printed over a manifest that is not there is exactly the tick that
        // stops somebody checking.
        let manifest = dir.join("workspace.json");
        let named_in_the_way = before
            .in_the_way
            .iter()
            .any(|(rel, _)| rel.as_str() == "workspace.json");
        // A manifest a row above already NAMES is not written by the scaffold, and that is not
        // a write that failed: "could not be written" beside that row said one link twice, and
        // counted the workspace as a repair that went wrong.
        if !named_in_the_way {
            match manifest.metadata() {
                Ok(_) => {}
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
                    ) =>
                {
                    blocked.insert(name.clone());
                    before.missing.retain(|rel| *rel != "workspace.json");
                    before.ok = before.missing.is_empty() && before.version >= before.target;
                    say(Say::Fail(format!(
                        "'{name}': workspace.json could not be written — something is in the way \
                         at that path. purlis never deletes or renames existing content."
                    ).into()));
                }
                Err(_) => {
                    // An `lstat` that fails proves nothing about the file, and "could not be
                    // written" sends somebody to fix a write.
                    say(Say::Warn(format!(
                        "'{name}': workspace.json cannot be checked — purlis cannot say whether \
                         it is there; restoring read access to it clears this."
                    ).into()));
                }
            }
        }

        // The stamp, looked at again for the manifest's reason: a refused stamp write is
        // swallowed, and "added structure v0 → v5" printed over one that was not written — then
        // again on the next run, since the workspace still read as stale.
        let after = wslayer::stamp(&dir).0;
        if after < before.target {
            unresolved.insert(name.clone());
            if !before
                .in_the_way
                .iter()
                .any(|(rel, _)| rel.as_str() == wslayer::STRUCTURE_MARKER)
            {
                say(Say::Warn(
                    format!(
                        "'{name}': {} could not be written, so this workspace still reads as \
                     structure v{after} and stays flagged for reinit.",
                        wslayer::STRUCTURE_MARKER
                    )
                    .into(),
                ));
            }
            // Files it did add are still reported.
            before.ok = before.missing.is_empty();
        }
        if before.ok {
            continue;
        }
        repairs += 1;
        repaired.insert(name.clone());
        let what = if before.missing.is_empty() {
            format!("structure v{} → v{}", before.version, before.target)
        } else {
            before.missing.join(", ")
        };
        say(Say::Done(
            format!("Reinitialized '{name}' → added {what}.").into(),
        ));
        if plane.is_live(name) && before.missing.iter().any(|rel| LIVE_SHARED.contains(rel)) {
            say(Say::Info(
                format!("  '{name}' is LIVE — commit the restored files: purlis save").into(),
            ));
        }
    }

    if repaired.is_empty() && blocked.is_empty() && unresolved.is_empty() && unseen == 0 {
        say(Say::Done(
            format!(
                "Up to date (structure v{}) — nothing to do.",
                wslayer::STRUCTURE_VERSION
            )
            .into(),
        ));
    } else if names.len() + unseen > 1 {
        // Two units, named as two. "Applied N repair(s)" is the number the rows above add up
        // to; "across M of T workspace(s)" is the number an operator checks against the plane
        // they know the size of.
        let stuck = if blocked.is_empty() {
            String::new()
        } else {
            format!("{} could not be repaired; ", blocked.len())
        };
        let left = unresolved.len()
            - unresolved
                .iter()
                .filter(|n| repaired.contains(*n) || blocked.contains(*n))
                .count();
        let kept = if left == 0 {
            String::new()
        } else {
            format!("{left} still hold what the rows above name; ")
        };
        let unchecked = if unseen == 0 {
            String::new()
        } else {
            format!("{unseen} could not be checked; ")
        };
        say(Say::Info(
            format!(
                "Applied {} across {} of {}; {stuck}{kept}{unchecked}\
             the rest were current.",
                counted(repairs, "repair", "repairs"),
                repaired.len(),
                counted(names.len() + unseen, "workspace", "workspaces")
            )
            .into(),
        ));
    }
    0
}

#[cfg(test)]
mod tests;
