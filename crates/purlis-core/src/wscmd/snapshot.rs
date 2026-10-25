// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `charter workspace snapshot` — capture a workspace's repos and branches into its
//! committed manifest.
//!
//! A port of `commands_workspace.cmd_workspace_snapshot`.
//!
//! **Enforce-push is the whole command.** A manifest branch is a promise to another engineer
//! on another machine: `charter workspace restore` will check that branch out, and a branch
//! that is not on the remote cannot be checked out there. So a repo with uncommitted work,
//! with unpushed commits, or with no upstream at all **blocks** the snapshot, and the refusal
//! names each one. `--force` records the branches as they stand and makes the promise anyway,
//! which is the operator's to make.
//!
//! A clone charter could not read blocks too (charter#917): this list being empty is what
//! lets the manifest claim to capture reality, and a `git status` that failed contributes the
//! same emptiness as one that found nothing — to somebody who has no way to know it was never
//! checked.

use std::path::Path;

use serde_json::Value;

use crate::counts::counted;
use crate::repocmd::{Say, Sink};
use crate::wscmd;

/// What `snapshot` was asked to record.
pub struct Request<'a> {
    pub root: &'a Path,
    /// The workspace, already resolved through the ladder by the caller.
    pub ws: &'a str,
    /// `--description`, which replaces the manifest's when it is given.
    pub description: Option<&'a str>,
    pub force: bool,
    /// The instant `updated_at` is stamped with, so a test can pin it.
    pub now: chrono::DateTime<chrono::Utc>,
}

/// `charter workspace snapshot [<name>] [--description …] [--force]`, and its exit code.
pub fn snapshot(request: &Request, say: Sink) -> u8 {
    let Request {
        root,
        ws,
        description,
        force,
        now,
    } = *request;

    let Ok(workspace) = crate::workspaces::Plane::open(root).workspace(ws) else {
        say(Say::Fail(
            format!("invalid workspace name '{}'", crate::personas::one_line(ws)).into(),
        ));
        return 1;
    };
    let found = match crate::repos::clones(root, ws) {
        Ok(found) => found,
        Err(why) => {
            say(Say::Fail(
                format!("could not read workspace '{ws}' — {why}").into(),
            ));
            return 1;
        }
    };
    // Said, never dropped: a clone charter refused to look at is one this snapshot will not
    // record, and the operator is the only one who can tell whether the link is theirs.
    for (name, why) in &found.refused {
        say(Say::Warn(format!("{name} is not recorded — {why}").into()));
    }
    if found.repos.is_empty() {
        say(Say::Fail(
            format!("workspace '{ws}' has no repo clones to snapshot.").into(),
        ));
        return 1;
    }
    let blockers = wscmd::restore_blockers(root, ws);
    if !blockers.is_empty() && !force {
        say(Say::Fail(
            format!(
                "Refusing to snapshot '{ws}' — push repo work first so the branch captures the \
             real state:"
            )
            .into(),
        ));
        wscmd::say_each(say, blockers);
        say(Say::Info(
            "Commit + push inside each repo, then retry (or --force to snapshot branches \
             as-is)."
                .into(),
        ));
        return 2;
    }

    let rows: Vec<Value> = found
        .repos
        .iter()
        .map(|repo| {
            let branch = match crate::repos::state_of(&repo.path) {
                Ok(state) => wscmd::branch_word(&state.head),
                // `git rev-parse --abbrev-ref HEAD` prints `HEAD` for a tree it cannot name a
                // branch in, and Python's `_repo_branch` falls back to the same literal.
                Err(_) => "HEAD".to_string(),
            };
            serde_json::json!({"name": repo.name, "branch": branch})
        })
        .collect();
    // Asked of git before the lock, so nothing else waits on it.
    let author = wscmd::git_user(root);

    // Read and written under one lock (#1292): a clone recording its repo in between would
    // otherwise be written over. The manifest on disk is the starting document, so a
    // `description` and any key an operator added keep their place: Python assigns into the
    // dict it read, and a key that is already there keeps the position it had.
    let held = match workspace.manifest_lock() {
        Ok(held) => held,
        Err(why) => {
            say(Say::Fail(format!("{why}, so nothing was recorded.").into()));
            return 1;
        }
    };
    let (doc, _owner) = workspace.manifest();
    let mut doc = match doc {
        Some(Value::Object(map)) => Value::Object(map),
        // Whatever else is in the file — a list, a number, nothing at all — is not a
        // manifest, and `snapshot` is the DELIBERATE writer: an operator who typed this is
        // asking for the file to be rewritten.
        _ => Value::Object(serde_json::Map::new()),
    };
    let map = doc.as_object_mut().expect("just built as an object");
    map.insert("name".into(), Value::String(ws.to_string()));
    if let Some(text) = description {
        map.insert("description".into(), Value::String(text.to_string()));
    }
    // `setdefault`: a manifest that never had one gets an empty string, and one that has a
    // description keeps it.
    map.entry("description")
        .or_insert_with(|| Value::String(String::new()));
    map.insert("repos".into(), Value::Array(rows.clone()));
    map.insert(
        "updated_at".into(),
        Value::String(now.format("%Y-%m-%dT%H:%M:%S+00:00").to_string()),
    );
    map.insert("updated_by".into(), Value::String(author));

    let written = workspace.write_manifest(&doc);
    drop(held);
    if let Err(why) = written {
        say(Say::Fail(
            format!(
                "could not write workspaces/{ws}/workspace.json ({why}) — nothing was recorded."
            )
            .into(),
        ));
        return 1;
    }
    say(Say::Done(
        format!(
            "Snapshot '{ws}' → workspaces/{ws}/workspace.json  ({}):",
            counted(rows.len(), "repo", "repos")
        )
        .into(),
    ));
    for row in &rows {
        say(Say::Info(
            format!(
                "  {} @ {}",
                row["name"].as_str().unwrap_or_default(),
                row["branch"].as_str().unwrap_or_default()
            )
            .into(),
        ));
    }
    say(Say::Info(
        "Share it with the team: purlis save   (commits + pushes manifest + memory)."
            .to_string()
            .into(),
    ));
    0
}

#[cfg(test)]
mod tests;
