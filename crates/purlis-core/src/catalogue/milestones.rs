// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **`purlis.forge_milestones`** (#6233, ADR 0122 as amended 2026-11-11): the milestones of a
//! repo of the asking workspace, each with its progress, read from GitHub.
//!
//! - **Only a repo of the asking workspace**, one of its clones whose `origin` is on GitHub,
//!   named by `repo` or, with none named, the workspace's only clone. Never an owner/repo the
//!   caller writes: **the repo is the one the project's inventory records for that clone**
//!   ([`recorded`], the operator's ruling of 2026-11-12), and a clone whose live origin
//!   ([`crate::forge::pr::Repo::of_clone`]) names another, or that has no record, is refused.
//! - **As whoever asks** ([`Reach::caller`]): a page as the person in the window, in the
//!   background (`Caller::window().background()`), which [`crate::forge::route`] sends on their
//!   sign-in, else their `gh` login; a chat's MCP call as that chat (`Caller::mcp`), on its own
//!   `gh` login. No token is read here, and none is in the answer.
//! - **Read-only**, and its answer is data. It reads the network for the page, which still has
//!   none of its own.
//! - **No login, no network, a spent limit** each answer `failed` with a sentence that says so
//!   ([`Unanswered::message`]), and a `repo` or `state` it does not take is `refused` with one.

use std::path::Path;

use serde_json::{Map, Value, json};

use super::ErrorKind;
use crate::forge::backend::{Caller, MilestoneProgress};
use crate::forge::pr::Repo;
use crate::forge::{Failure, ForgeBackend, ForgeError, Kind};

/// The query's MCP tool, and its name after `purlis.`.
pub const NAME: &str = "forge_milestones";

/// How long an answer stays good: a page subscribed to it is asked again after this, and
/// never sooner, so a live page asks GitHub about once a minute.
pub const FRESH_SECONDS: u64 = 60;

/// Who asks the forge, and how a repo's backend is made.
pub struct Reach<'a> {
    pub caller: Caller,
    pub backend_of: &'a dyn Fn(&Repo) -> Box<dyn ForgeBackend>,
}

/// Why there are no milestones: the catalogue's kind, and a sentence for the reader.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unanswered {
    pub kind: ErrorKind,
    pub message: String,
}

impl Unanswered {
    fn refused(message: String) -> Self {
        Unanswered {
            kind: ErrorKind::Refused,
            message,
        }
    }

    fn failed(message: String) -> Self {
        Unanswered {
            kind: ErrorKind::Failed,
            message,
        }
    }
}

/// The JSON Schema of the MCP tool's parameters, which are the query's.
pub fn parameters() -> Value {
    json!({
        "type": "object",
        "properties": {
            "repo": {
                "type": "string",
                "description": "The repo of this workspace, by its folder's name. Leave it out \
                                when the workspace has one repo.",
            },
            "state": {
                "type": "string",
                "enum": ["open", "all"],
                "description": "open (the default) for the open milestones, or all for the \
                                closed ones too.",
            },
        },
        "additionalProperties": false,
    })
}

/// **The milestones `args` ask for**, of a repo of workspace `workspace` in the plane at `root`.
pub fn read(
    root: &Path,
    workspace: &str,
    args: &Map<String, Value>,
    reach: &Reach,
) -> Result<Value, Unanswered> {
    let all = match args.get("state").and_then(Value::as_str) {
        None | Some("open") => false,
        Some("all") => true,
        Some(other) => {
            return Err(Unanswered::refused(format!(
                "purlis does not list milestones by the state {}: it is open or all.",
                crate::shown::short(other)
            )));
        }
    };
    // Its words name folders on this machine, which a page has no business showing.
    let clones = crate::repos::clones(root, workspace).map_err(|_| {
        Unanswered::failed("purlis could not read the repos of this workspace.".into())
    })?;
    let names: Vec<&str> = clones.repos.iter().map(|r| r.name.as_str()).collect();
    let clone = match args.get("repo").and_then(Value::as_str) {
        Some(name) => clones
            .repos
            .iter()
            .find(|r| r.name == name)
            .ok_or_else(|| {
                Unanswered::refused(format!(
                    "{} is not a repo of this workspace. Name one of {}.",
                    crate::shown::short(name),
                    names.join(", ")
                ))
            })?,
        None => match clones.repos.as_slice() {
            [one] => one,
            [] => {
                return Err(Unanswered::refused(
                    "This workspace has no repo yet, so it has no milestones to show.".into(),
                ));
            }
            _ => {
                return Err(Unanswered::refused(format!(
                    "This workspace has {} repos. Name the one to show with repo: {}.",
                    names.len(),
                    names.join(", ")
                )));
            }
        },
    };
    // The repo is the one the project records for the clone, never its live origin alone,
    // which anything that writes the clone's `.git/config` can point elsewhere (the operator's
    // ruling of 2026-11-12). The two must agree, and neither is named to the page.
    let repo = match (
        Repo::of_clone(root, &clone.path),
        recorded(root, &clone.name),
    ) {
        (Ok(live), Some(recorded)) if same(&live, &recorded) => recorded,
        (Ok(_), None) => {
            return Err(Unanswered::refused(format!(
                "This project records no repo for {}, so purlis does not read its milestones. \
                 Record it with `purlis discover`, or clone it from the window.",
                clone.name
            )));
        }
        (Err(_), None) => {
            return Err(Unanswered::refused(format!(
                "{} has no origin on a GitHub host this project declares, so it has no \
                 milestones purlis can read.",
                clone.name
            )));
        }
        _ => {
            return Err(Unanswered::refused(format!(
                "The origin of {} is not the repo this project records for it, so purlis does \
                 not read its milestones. Set its origin back, or record the repo again with \
                 `purlis discover`.",
                clone.name
            )));
        }
    };
    if repo.forge.kind != Kind::GitHub {
        return Err(Unanswered::refused(format!(
            "{} is on GitLab, and purlis reads milestones from GitHub only.",
            clone.name
        )));
    }
    let read = (reach.backend_of)(&repo)
        .milestone_progress(&reach.caller, &repo.path, all)
        .map_err(|error| unanswered(&repo.path, &error))?;
    Ok(json!({
        "repo": clone.name,
        "path": repo.path,
        "state": if all { "all" } else { "open" },
        "cut": read.cut,
        "milestones": read.listed.iter().map(milestone).collect::<Vec<_>>(),
    }))
}

/// The repo the project's inventory records for the clone `name` (`inventory/repos.json`, a
/// Plane-tier record no chat writes), on a forge this project declares; `None` when it records
/// none there.
fn recorded(root: &Path, name: &str) -> Option<Repo> {
    let doc = crate::inventory::load(root, "").ok()?;
    let row = crate::inventory::listed(&doc)
        .into_iter()
        .find(|row| row.get("name").and_then(Value::as_str) == Some(name))?;
    let record = crate::inventory::read(&row)?;
    let forge = crate::forge::resolve_host(&record.ssh_url, root)?;
    (forge.kind == record.forge && !record.path_with_namespace.is_empty()).then(|| Repo {
        forge,
        path: record.path_with_namespace,
    })
}

/// Whether the clone's live origin names the recorded repo: one forge, one host and one path,
/// in any case, as GitHub and GitLab read them.
fn same(live: &Repo, recorded: &Repo) -> bool {
    live.forge.kind == recorded.forge.kind
        && live.forge.host.eq_ignore_ascii_case(&recorded.forge.host)
        && live.path.eq_ignore_ascii_case(&recorded.path)
}

/// One milestone as data.
fn milestone(m: &MilestoneProgress) -> Value {
    json!({
        "number": m.number,
        "title": m.title,
        "description": m.description,
        "due_on": m.due_on,
        "state": m.state,
        "open_issues": m.open_issues,
        "closed_issues": m.closed_issues,
        "url": m.url,
        "updated_at": m.updated_at,
    })
}

/// What a page is told when the forge did not answer. The forge's own words are left out: a
/// page shows this sentence, and only these causes are worth the reader's knowing.
fn unanswered(path: &str, error: &ForgeError) -> Unanswered {
    Unanswered::failed(match error.failure() {
        Failure::Auth => format!(
            "purlis has no GitHub login to read the milestones of {path}. Sign in to GitHub in \
             purlis, or run `gh auth login`."
        ),
        Failure::Transport => format!(
            "purlis could not reach GitHub for the milestones of {path}. It asks again in a \
             minute."
        ),
        Failure::RateLimited { .. } => format!(
            "GitHub's request limit for this hour is spent, so purlis waits to read the \
             milestones of {path}."
        ),
        Failure::NotFound => format!(
            "GitHub does not show {path} to this login, so purlis cannot read its milestones."
        ),
        // A 403, or a caller `route` gave no credential: a chat whose CLI login is the person's.
        Failure::Forbidden => {
            format!("purlis may not read the milestones of {path} with this login.")
        }
        _ => format!("GitHub answered the milestones of {path} in a way purlis could not read."),
    })
}

/// The JSON Schema of the answer's `data`.
pub fn result() -> Value {
    let string = |d: &str| json!({ "type": "string", "description": d });
    let integer = |d: &str| json!({ "type": "integer", "description": d });
    super::object(json!({
        "repo": string("The repo of this workspace, by its folder's name."),
        "path": string("Its owner/name on GitHub."),
        "state": string("open or all, as asked."),
        "cut": {
            "type": "boolean",
            "description": "Whether GitHub may list more milestones than these: purlis reads \
                            the first 300 at most.",
        },
        "milestones": super::list_of(super::object(json!({
            "number": integer("Its number on GitHub."),
            "title": string("Its title."),
            "description": string("Its description, or empty."),
            "due_on": {
                "type": ["string", "null"],
                "description": "When it is due (RFC 3339), or null.",
            },
            "state": string("open or closed."),
            "open_issues": integer("How many of its issues are open."),
            "closed_issues": integer("How many of its issues are closed."),
            "url": string("Its page on GitHub."),
            "updated_at": string("When it last changed (RFC 3339)."),
        }))),
    }))
}

#[cfg(test)]
mod tests;
