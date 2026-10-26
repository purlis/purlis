// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `charter workspace restore` — rebuild a workspace from its committed manifest.
//!
//! A port of `commands_workspace.cmd_workspace_restore`. M2.24 named this as a gap with one
//! half missing: the clone per absent repo is [`crate::repocmd::clone`], and **the
//! credentialed `git pull` per recorded branch had no port**, which is why
//! `workspace fork --restore` took the flag and then said what it had not done. This is that
//! half, and the command around it.
//!
//! # The manifest is a committed file, so every field in it is untrusted
//!
//! `workspace.json` is committed *precisely so a teammate can restore somebody else's
//! workspace*. That is the feature, and it is what makes the document an input from another
//! machine. Two guards come straight out of charter's own incidents and both are here:
//!
//! - **The name is a path segment, gated before it is joined** (charter#325/#334/#328). The
//!   existence check below is an existence check and never a containment one, so without
//!   this a row named `../../elsewhere` selected a repository the operator never named — and
//!   `git checkout` plus a **credentialed** `git pull` then ran inside it, confirmed on
//!   0.47.2 by the target repository's own reflog.
//! - **A branch may not begin with `-`** (charter#334's second half). A branch is a REF and
//!   not a path segment — `feature/x` is what most teams write — so no name rule applies and
//!   the treatment is argv position instead: `git checkout` reads a leading dash as an
//!   option, and it has options that WRITE (`-b`, `-B`, `--orphan`,
//!   `--pathspec-from-file`). `git check-ref-format` is deliberately not used: measured on
//!   git 2.50.1, `check-ref-format refs/heads/-b` ACCEPTS `-b`, because a leading dash is
//!   legal inside a ref. Ref grammar answers a different question than argv safety.
//!
//! A refusal is **per entry, never per document**: a manifest is shared, so rejecting the
//! whole thing would let one bad row deny the other eight repos to the whole team — an
//! attack in its own right.
//!
//! # Where the credential comes from, and where it does not
//!
//! The pull goes through [`crate::worktree::git::run_network`], which is the one-credential
//! rule: every configured helper is reset and the only one left is the forge CLI's own,
//! resolved from **this clone's own origin** ([`crate::gitpolicy::forge_for`]) and never from
//! a hardcoded forge. A host charter does not manage gets no credential and no pull — a token
//! for one forge is never offered to another.
//!
//! That runner is stricter than Python's `_cred_flag` in one way, and it is the same
//! hardening `charter clone` already carries: `protocol.ssh.allow=never`. A clone whose
//! origin is an SSH URL is refused the transport here, where Python would pull over SSH with
//! whatever key the agent holds. `docs/git-policy.md` is where that rule is written down; it
//! is not this command's to relax.
//!
//! # What a differential test of this can and cannot see
//!
//! Everything but the transfer. A scenario cannot reach a real forge, so it stands a bare
//! repository beside the plane and rewrites the forge's URL onto it with
//! `url.<local>.insteadOf` — and **`git remote get-url` applies that rewrite** (measured, git
//! 2.50.1: `config remote.origin.url` gives the HTTPS URL, `get-url` gives `file:///…`).
//! [`crate::gitpolicy::forge_for`] reads `get-url`, so the same rewrite that makes the fetch
//! local makes the origin a host charter cannot place, and the row is skipped on both sides.
//! The two are mutually exclusive by construction — the same trap the differential's
//! `_forge_trap` existed to catch one level up, for the plane's own push (see
//! `planegit::origin_https`). So the scenarios cover the decisions (which rows are refused, which are skipped,
//! which branch is checked out, what is said and in what order) and the pull's own wiring is
//! pinned by the unit tests at the bottom of this file, against a local bare remote.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::counts::counted;
use crate::forge::py_str;
use crate::repocmd::{Say, Sink, clone};
use crate::wscmd::ensure;

/// What `restore` was asked for.
pub struct Request<'a> {
    pub root: &'a Path,
    /// The workspace to rebuild.
    pub ws: &'a str,
    /// List what would be cloned and clone nothing — Python's `--on-demand`.
    pub on_demand: bool,
    /// The instant a manifest this writes would be stamped with (the clone's membership
    /// record).
    pub now: chrono::DateTime<chrono::Utc>,
}

/// One manifest row that survived the containment gate.
struct Row {
    /// The row itself, for the fields that are printed back.
    record: Value,
    /// Where it would be cloned to.
    dir: PathBuf,
}

impl Row {
    /// The row's `name`, as Python prints it — `str(r["name"])`.
    fn name(&self) -> String {
        name_of(&self.record)
    }

    /// The branch this row pins — [`pin_of`].
    fn pin(&self) -> String {
        pin_of(&self.record)
    }

    /// The row's `branch` as the report repeats it — `r["branch"]`, unstripped.
    ///
    /// Not [`pin_of`]: charter prints the field and hands git the stripped value, so a row
    /// whose branch is `" main "` says `" main "` in the report. Only reached where
    /// [`pin_of`] is non-empty, so the key is there.
    fn branch_said(&self) -> String {
        py_str(self.record.get("branch").unwrap_or(&Value::Null))
    }
}

/// A manifest row's `name`, as Python prints it — `str(r["name"])`.
fn name_of(record: &Value) -> String {
    py_str(record.get("name").unwrap_or(&Value::Null))
}

/// The branch a manifest row pins, or `""` for one that pins none — Python's `_pin`.
///
/// Two shapes reach this and both are ordinary. `snapshot` writes `{"name", "branch"}`,
/// because an operator asked for the branches to be captured; every manifest charter writes
/// on its own writes `{"name"}` alone (charter#884), and reading `row["branch"]` was a
/// `KeyError` waiting for the first of them.
///
/// **Stripped**, so the value the leading-dash guard inspects is the value git is handed:
/// `" -b"` is not a ref anybody writes and a `starts_with` over the unstripped string says
/// nothing about it, while `git checkout` reads the argument it is actually given.
///
/// **Python's truth test, not a null check**: `str(row.get("branch") or "")`. A row whose
/// branch is `0`, `false`, `[]` or `{}` pins nothing, exactly as one whose branch is absent
/// does — a manifest is a committed document and every JSON shape reaches here.
fn pin_of(record: &Value) -> String {
    match record.get("branch") {
        Some(value) if crate::forge::truthy(value) => py_str(value).trim().to_string(),
        _ => String::new(),
    }
}

/// Run it, and give back the exit code.
pub fn restore(request: &Request, say: Sink) -> u8 {
    let Request {
        root,
        ws,
        on_demand,
        now,
    } = *request;
    let plane = crate::workspaces::Plane::open(root);
    // A manifest charter cannot reach is a manifest charter does not have: Python's
    // `read_manifest` is gated, and a name that is a path has no manifest rather than
    // somebody else's.
    let workspace = plane.workspace(ws).ok();
    let manifest = match workspace
        .as_ref()
        .map(crate::workspaces::Workspace::manifest)
    {
        Some((Some(Value::Object(map)), _)) => map,
        _ => serde_json::Map::new(),
    };
    let repos: Vec<Value> = manifest
        .get("repos")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if repos.is_empty() {
        say(Say::Fail(
            format!(
                "no manifest for '{ws}' (workspaces/{ws}/workspace.json). Pull the plane's latest \
             first: git pull"
            )
            .into(),
        ));
        return 1;
    }
    // The local structure, before anything is cloned into it: `refs/`, the memory store, the
    // charter, the layer and the version stamp. A path below the folder it could not write is
    // `purlis workspace reinit`'s to report; a folder it could not make is the end of this.
    let author = ensure::author();
    if let Err(why) = ensure::ensure(root, ws, now, &author) {
        say(Say::Fail(why.into()));
        return 1;
    }

    let said = |key: &str| match manifest.get(key) {
        None => "?".to_string(),
        Some(value) => py_str(value),
    };
    say(Say::Info(
        format!(
            "Restoring '{ws}' — {} from manifest (updated {} by {}).",
            counted(repos.len(), "repo", "repos"),
            said("updated_at"),
            said("updated_by"),
        )
        .into(),
    ));
    if on_demand {
        for record in &repos {
            let pin = pin_of(record);
            let pin = if pin.is_empty() {
                "no branch recorded".to_string()
            } else {
                pin
            };
            say(Say::Info(
                format!(
                    "  on-demand: {} @ {pin} (clone when you enter it)",
                    name_of(record),
                )
                .into(),
            ));
        }
        say(Say::Info(
            format!("Enter the workspace and clone as you go: purlis clone <repo> -w {ws}").into(),
        ));
        return 0;
    }

    let Some(ws_dir) = workspace.as_ref().map(|w| w.dir().to_path_buf()) else {
        // Unreachable in practice — a workspace whose name cannot be joined has no manifest
        // and returned above — and a refusal rather than a panic if it ever stops being.
        say(Say::Fail(
            format!("No usable repos in '{ws}'s manifest.").into(),
        ));
        return 1;
    };

    let mut rows: Vec<Row> = Vec::new();
    let mut refused: Vec<Value> = Vec::new();
    for record in &repos {
        let name = name_of(record);
        match child(root, ws, &ws_dir, &name) {
            Some(dir) => rows.push(Row {
                record: record.clone(),
                dir,
            }),
            None => refused.push(record.clone()),
        }
    }
    for record in &refused {
        // `str(r.get("name"))!r` — the string charter joined, repeated back as Python repeats
        // a value read from a committed file.
        let name = name_of(record);
        say(Say::Fail(
            format!(
                "  {}: refused — {}. Fix workspaces/{ws}/workspace.json.",
                crate::pyrepr::repr_str(&name),
                clone::not_a_segment(&name),
            )
            .into(),
        ));
    }
    if rows.is_empty() {
        say(Say::Fail(
            format!("No usable repos in '{ws}'s manifest.").into(),
        ));
        return 1;
    }

    let missing: Vec<String> = rows
        .iter()
        .filter(|row| !is_git_repo(&row.dir))
        .map(Row::name)
        .collect();
    if !missing.is_empty() {
        // Its exit status is deliberately dropped, which is Python's: a repo this could not
        // clone is reported one line down as "not cloned (no access?) — skipped", and
        // `restore` answers with the count it managed rather than with the clone's failure.
        let _ = clone::clone(
            &clone::Request {
                root,
                ws,
                repos: &missing,
                now,
                author: &author,
                hosts: None,
            },
            say,
        );
    }

    let mut done = 0usize;
    for row in &rows {
        let name = row.name();
        if !is_git_repo(&row.dir) {
            say(Say::Warn(
                format!("  {name}: not cloned (no access?) — skipped.").into(),
            ));
            continue;
        }
        // An UNPINNED row is restored by existing (charter#884). Every manifest charter
        // writes itself records membership and no branch — a branch carries `snapshot`'s
        // promise that it is on the remote, and the writers nobody asked for cannot make it —
        // so this has to read a row that has none. **Before the forge lookup**, because there
        // is nothing to check out and nothing to pull: the clone above is already on whatever
        // the remote calls default, which is what "unpinned" means.
        let branch = row.pin();
        if branch.is_empty() {
            say(Say::Done(
                format!(
                    "  {name} @ default branch (no branch recorded — `purlis workspace snapshot \
                 {ws}` pins one)"
                )
                .into(),
            ));
            done += 1;
            continue;
        }
        // THIS clone's own forge, never a hardcoded one. An unrecognised host — not a default
        // forge, not declared in `charter.toml` — gets no guessed credential helper: skipped
        // rather than mis-authenticated.
        let Some(forge) = crate::gitpolicy::forge_for(&row.dir, root) else {
            say(Say::Warn(
                format!("  {name}: origin host isn't a known/declared forge — skipped.").into(),
            ));
            continue;
        };
        if branch.starts_with('-') {
            say(Say::Fail(
                format!(
                    "  {name}: refused branch {} — a branch read from a committed manifest may not \
                 begin with '-', which git would read as an option rather than a ref. Fix \
                 workspaces/{ws}/workspace.json.",
                    crate::pyrepr::repr_str(&branch),
                )
                .into(),
            ));
            continue;
        }
        if checkout(&row.dir, &branch) {
            // The window's own sign-in goes only to the remote recorded for the clone, as a
            // save's push does (#1851): a clone whose remote changed is not pulled.
            match crate::forge::gitcredential::helper_for_fetch(&forge, &row.dir) {
                Ok(helper) => pull(&row.dir, &helper),
                Err(why) => say(Say::Warn(format!("  {name}: {why}; not pulled.").into())),
            }
            say(Say::Done(
                format!("  {name} @ {}", row.branch_said()).into(),
            ));
            done += 1;
        } else {
            say(Say::Warn(
                format!("  {name}: couldn't checkout '{}'.", row.branch_said()).into(),
            ));
        }
    }
    say(Say::Done(
        format!(
            "Restored {done}/{} into '{ws}'.",
            counted(rows.len(), "repo", "repos")
        )
        .into(),
    ));
    0
}

/// `ws_dir / name` when that is a direct child charter may write, else `None` — Python's
/// `contain.child`, with this port's own containment gate behind it.
///
/// The gate is on the directory charter is about to run git **in**, which is the path it
/// opens: `git -C <dir>` reads and writes the tree at `dir`, so a `workspaces/<ws>/<repo>`
/// that is a symlink out of the plane is a checkout this command may not touch. Never the
/// parent — `workspaces/<ws>` being fine says nothing about the child a committed manifest
/// names.
fn child(root: &Path, ws: &str, ws_dir: &Path, name: &str) -> Option<PathBuf> {
    if !crate::contain::segment_ok(name) {
        return None;
    }
    let dir = crate::worktree::confine::within_workspace(root, ws, &ws_dir.join(name)).ok()?;
    crate::contain::readable(root, &dir).ok()?;
    Some(dir)
}

/// Whether there is a `.git` at `path` — Python's `is_git_repo`, through the link rather than
/// at it, and "I am not allowed to look" answered as "no".
fn is_git_repo(path: &Path) -> bool {
    std::fs::metadata(path.join(".git")).is_ok()
}

/// `git checkout <branch> --` in `dir`. Whether it landed.
///
/// **`--` after the ref**, so git cannot reinterpret the branch as a pathspec even if the
/// leading-dash guard above is ever loosened. It goes after the branch and not before:
/// `checkout -- <x>` means the opposite — it forces the PATHSPEC reading.
///
/// Untimed, like every call that checks out a tree ([`crate::worktree::git::run_untimed`]'s
/// contract): a killed checkout leaves a half-written working tree, and on a large repo or a
/// cold cache thirty seconds is routine.
fn checkout(dir: &Path, branch: &str) -> bool {
    crate::worktree::git::run_untimed(dir, &["checkout", branch, "--"]).is_ok_and(|r| r.ok())
}

/// `git pull --ff-only` in `dir`, over `helper`'s credential — the latest of the recorded
/// branch.
///
/// **The outcome is deliberately not reported**, which is Python's behaviour: the row is
/// about the branch being checked out, and a pull that could not reach the forge leaves the
/// operator with the branch they asked for at the revision they already had. Reporting it
/// would be a second, louder failure for every machine that restores offline.
fn pull(dir: &Path, helper: &str) {
    let _ = crate::worktree::git::run_network(dir, Some(helper), &["pull", "--ff-only"]);
}

/// Where `workspace fork --restore` ends: the restore of the fork it has just written.
///
/// A function of its own so `fork` names one thing, and so the count it prints first is the
/// count this then restores.
pub fn after_fork(root: &Path, ws: &str, now: chrono::DateTime<chrono::Utc>, say: Sink) -> u8 {
    restore(
        &Request {
            root,
            ws,
            on_demand: false,
            now,
        },
        say,
    )
}

#[cfg(test)]
mod tests;
