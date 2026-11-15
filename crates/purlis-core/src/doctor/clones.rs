// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `workspace clones`: clones behind their upstream, in EVERY workspace (#156).
//!
//! `sync` defaults to the active workspace, so a clone seven commits behind in another one
//! was reported by nothing — and a stale clone is not inert, it is what a session reads if it
//! happens to work there.
//!
//! **Read from remote-tracking refs; never fetched.** This runs from the SessionStart hook
//! and must not reach the network. It can therefore under-report — origin moving is
//! invisible until something fetches — but it can never invent staleness.

use super::fsx::{self, Unread};
use super::git::git_in;
use super::{Doctor, Row, Status};
use crate::counts::counted;
use crate::memstore::py_strip;

pub(super) fn workspace_clones(d: &Doctor) -> Row {
    const NAME: &str = "workspace clones";
    if !d.has_plane {
        return Row::ok(NAME, "no control plane found");
    }
    let root = d.root.as_path();
    let mut behind: Vec<String> = Vec::new();
    let mut total = 0usize;
    let (workspaces, mut unseen): (Vec<String>, Vec<Unread>) = match fsx::read_workspaces(root) {
        Ok(found) => found,
        Err(e) => {
            return Row::not_checked(NAME, fsx::py_os_error(&e, &root.join("workspaces")));
        }
    };
    let looked_for = workspaces.len() + unseen.len();
    let mut unread_ws = unseen.len();
    for ws in &workspaces {
        // One workspace charter cannot list is named, and the others are still read (#1014):
        // costing the whole row would hide a stale clone one workspace over.
        let (found, unstatted) = match fsx::read_clones(root, ws) {
            Ok(found) => found,
            Err(e) => {
                unseen.push((root.join("workspaces").join(ws), e.raw_os_error()));
                unread_ws += 1;
                continue;
            }
        };
        unseen.extend(unstatted);
        for clone in found {
            total += 1;
            // `@{upstream}` fails cleanly where there is no tracking branch, which is not a
            // fault.
            let run = match git_in(&clone, &["rev-list", "--count", "HEAD..@{upstream}"]) {
                Ok(run) => run,
                Err(why) => return Row::not_checked(NAME, why),
            };
            if !run.ok() {
                continue;
            }
            let n: u64 = match py_strip(&run.out) {
                "" => 0,
                text => match text.parse() {
                    Ok(n) => n,
                    Err(_) => {
                        return Row::not_checked(
                            NAME,
                            format!("invalid literal for int() with base 10: '{text}'"),
                        );
                    }
                },
            };
            if n > 0 {
                let name = clone
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                // A chat can name a directory anything, and a newline in it must not print a
                // row of its own (#353).
                let at = super::one_line(&format!("{ws}/{name}"), super::DISPLAY_LIMIT);
                behind.push(format!("{at} ({n} behind)"));
            }
        }
    }

    if behind.is_empty() {
        if !unseen.is_empty() {
            let row = Row::warn(
                NAME,
                format!(
                    "{} across {} of {}, none behind",
                    counted(total, "clone", "clones"),
                    looked_for - unread_ws,
                    counted(looked_for, "workspace", "workspaces")
                ),
                "",
            );
            return fsx::beside_unread(root, row, &unseen);
        }
        if total == 0 {
            return Row::ok(NAME, "no clones in any workspace — nothing to check");
        }
        return Row::ok(
            NAME,
            format!(
                "{} across all workspaces, none behind",
                counted(total, "clone", "clones")
            ),
        );
    }
    let mut detail = behind
        .iter()
        .take(4)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    if behind.len() > 4 {
        detail.push_str(", …");
    }
    let row = Row {
        name: NAME.to_owned(),
        status: Status::Warn,
        detail,
        hint: "→ purlis sync --all  (plain `sync` only touches the ACTIVE workspace, which is \
               how this stays hidden)  Counted from what the last fetch recorded, so it can \
               under-report — never a live query, this runs at SessionStart."
            .to_owned(),
        settings: None,
        fix: None,
    };
    fsx::beside_unread(root, row, &unseen)
}

/// V35: every `AGENTS.md` that charter's exclude line hides and charter did not write, in
/// every workspace's repositories ([`crate::guest::hidden_agents_md`]). No row at all where
/// there is none: the line exists only where a chat's own worktree was given an `AGENTS.md`
/// (ADR 0085), and a green row for a feature most planes never meet is furniture.
pub(super) fn hidden_agents_md(d: &Doctor) -> Option<Row> {
    const NAME: &str = "hidden AGENTS.md";
    if !d.has_plane {
        return None;
    }
    let root = d.root.as_path();
    let base = root.join("workspaces");
    let mut hidden = crate::guest::HiddenAgentsMd::default();
    // A workspace or clone this cannot list is a doubt, never an absent row: silence here
    // would read as "nothing hidden".
    let (workspaces, unseen) = match fsx::read_workspaces(root) {
        Ok(found) => found,
        Err(e) => {
            return Some(Row::not_checked(NAME, fsx::py_os_error(&e, &base)));
        }
    };
    let cannot = |(path, code): &Unread| {
        format!(
            "{} cannot be checked{} — restoring read access clears this",
            path.display(),
            code.map(|c| format!(" (errno {c})")).unwrap_or_default()
        )
    };
    hidden.unsure.extend(unseen.iter().map(cannot));
    for ws in &workspaces {
        let (clones, unstatted) = match fsx::read_clones(root, ws) {
            Ok(found) => found,
            Err(e) => {
                hidden.unsure.push(fsx::py_os_error(&e, &base.join(ws)));
                continue;
            }
        };
        hidden.unsure.extend(unstatted.iter().map(cannot));
        for clone in clones {
            hidden.extend(crate::guest::hidden_agents_md(&clone));
        }
    }
    let said = hidden.said(|path| {
        super::one_line(
            &path
                .strip_prefix(&base)
                .unwrap_or(path)
                .display()
                .to_string(),
            super::DISPLAY_LIMIT,
        )
    })?;
    Some(Row::warn(
        NAME,
        said,
        "Commit or move aside each file named, and restore read access where a check could not \
         run (ADR 0085).",
    ))
}

/// #4775: every clone whose own config git stops on, named with how to take the section out.
/// A base recorded under no branch, `[branch ""]`, is read by `git config` and refused by every
/// git that reads a remote, so a fetch, a push and the upstream `workspace clones` counts all
/// fail on it, and that row reads the clone as one with no upstream. purlis's next cut, update
/// or push in the clone removes it ([`crate::worktree::mend_nameless_base`]); this says so
/// before then. A config `git config` cannot read at all is named too, with git's own words.
/// No row at all where every config reads: one green row per clone would be furniture.
pub(super) fn clone_git_config(d: &Doctor) -> Option<Row> {
    const NAME: &str = "clone git config";
    if !d.has_plane {
        return None;
    }
    let root = d.root.as_path();
    // A workspace or clone that cannot be listed is `workspace clones`' to name.
    let (workspaces, _) = fsx::read_workspaces(root).ok()?;
    let mut nameless: Vec<(String, String)> = Vec::new();
    let mut unread: Vec<(String, String)> = Vec::new();
    for ws in &workspaces {
        let Ok((clones, _)) = fsx::read_clones(root, ws) else {
            continue;
        };
        for clone in clones {
            let name = clone
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let at = super::one_line(&format!("{ws}/{name}"), super::DISPLAY_LIMIT);
            // A word of the command the hint suggests, so quoted and never shortened (#4830).
            let shown = crate::shown::shell_path(&clone);
            // git answers 0 where a key matches and 1 where none does; anything else is a
            // config it could not read.
            match git_in(
                &clone,
                &["config", "--local", "--get-regexp", r"^branch\.\."],
            ) {
                Ok(run) if run.code == Some(0) => nameless.push((at, shown)),
                Ok(run) if run.code == Some(1) => {}
                Ok(run) => unread.push((at, super::one_line(&run.err, super::DISPLAY_LIMIT))),
                Err(why) => unread.push((at, why)),
            }
        }
    }
    if nameless.is_empty() && unread.is_empty() {
        return None;
    }
    let mut detail = Vec::new();
    let mut hint = Vec::new();
    if !nameless.is_empty() {
        let names: Vec<&str> = nameless.iter().map(|(at, _)| at.as_str()).collect();
        detail.push(format!(
            "{} holds [branch \"\"], a section git stops on: every fetch and push in it fails",
            names.join(", ")
        ));
        let commands: Vec<String> = nameless
            .iter()
            .map(|(_, clone)| format!("git -C {clone} config --remove-section branch."))
            .collect();
        hint.push(format!(
            "Remove it from your own terminal: {}. purlis's next cut, update or push in the \
             clone removes it too.",
            commands.join(" ; ")
        ));
    }
    if !unread.is_empty() {
        let said: Vec<String> = unread
            .iter()
            .map(|(at, why)| format!("{at} ({why})"))
            .collect();
        detail.push(format!("git cannot read the config of {}", said.join(", ")));
        hint.push("Open the clone's .git/config and fix or remove the line git names.".to_owned());
    }
    Some(Row::warn(NAME, detail.join("; "), hint.join(" ")))
}
/// **`unrecorded clones`** (R-338, Q184): every clone of the project's workspaces that purlis did
/// not clone and the person has not trusted, each with the address its remote names: the clones
/// the window's one Trust these clones lists
/// ([`crate::forge::gitcredential::vouch::unvouched_clones`], [`crate::windowonly`]). A clone
/// whose origin names several addresses is not in that ask, since no press can trust it, so the
/// row names it with the fix: give origin one address (Q257). It reads the record of clones and
/// raises nothing: a clone is trusted only on the person's answer. No row at all where there is
/// none, or no config home to read the record in.
pub(super) fn unrecorded_clones(d: &Doctor) -> Option<Row> {
    const NAME: &str = "unrecorded clones";
    if !d.has_plane {
        return None;
    }
    // The config home: the machine store the record of clones is kept in, as the budgets are.
    let config_root = d.budgets.as_deref()?;
    let clones = crate::forge::gitcredential::vouch::unvouched_clones(&d.root, config_root);
    if clones.is_empty() {
        return None;
    }
    // A chat can name a folder anything, and a newline in it must not print a row of its own
    // (#353).
    let said = |text: String| super::one_line(&text, super::DISPLAY_LIMIT);
    let (one, several): (Vec<_>, Vec<_>) = clones.iter().partition(|clone| clone.place.is_some());
    let mut detail = Vec::new();
    let mut hint = Vec::new();
    if !one.is_empty() {
        let named: Vec<String> = one
            .iter()
            .map(|clone| said(format!("{} ({})", clone.folder, clone.remote_said())))
            .collect();
        let them = if one.len() == 1 { "it" } else { "them" };
        detail.push(format!(
            "purlis did not clone {} and you have not trusted {them}: {}",
            if one.len() == 1 { "this" } else { "these" },
            named.join(", ")
        ));
        hint.push(format!(
            "Trust {them} only if you made {them} yourself: with Trust these clones in the \
             window's Asks, or, for a clone at a workspace's root, with `purlis doctor --fix \
             move-into-repos` in your own terminal, which asks. Until then no chat gets a \
             GitHub App token for {them}, and a save does not push {them} with your sign-in."
        ));
    }
    for clone in &several {
        detail.push(said(format!(
            "{} names more than one address ({}), so purlis cannot trust it",
            clone.folder,
            clone.remote_said()
        )));
        hint.push(said(format!(
            "To trust it, give origin of {} one address, and it joins Trust these clones.",
            clone.folder
        )));
    }
    Some(Row::warn(NAME, detail.join("; "), hint.join(" ")))
}
