// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The forge APIs' REST routes under a chat's repo: what is refused by name, and the allowlist of
//! writes. `rel` is the path after the repo (`pulls/7/merge`), decoded once.
//!
//! **Reads** (`GET`, `HEAD`) are answered for anything under the repo, except the families that
//! hold the repo's administration (hooks, keys, secrets, protection, collaborators…). **Writes**
//! are an allowlist: a route not named below is refused, whatever it would do. The names exist so
//! the refusal says why; they are never what lets a request through.

use super::commands;
use super::policy::{Class, Refusal};
use super::repo::{AllowedRepo, Kind};
use super::request::{Target, is_read};

/// Whether `rel` matches `pattern`: `*` is any one segment, `#` one made of digits.
fn matches(rel: &[String], pattern: &[&str]) -> bool {
    rel.len() == pattern.len()
        && rel.iter().zip(pattern).all(|(seg, pat)| match *pat {
            "*" => true,
            "#" => !seg.is_empty() && seg.bytes().all(|b| b.is_ascii_digit()),
            fixed => seg.eq_ignore_ascii_case(fixed),
        })
}

/// Segments after which what follows is a file path or a ref name, not the forge's own words.
fn free_form_from(rel: &[String]) -> usize {
    match rel {
        [first, ..] if first.eq_ignore_ascii_case("contents") => 1,
        [first, second, ..]
            if first.eq_ignore_ascii_case("repository")
                && (second.eq_ignore_ascii_case("files")
                    || second.eq_ignore_ascii_case("blobs")
                    || second.eq_ignore_ascii_case("tree")) =>
        {
            2
        }
        _ => rel.len(),
    }
}

fn names(rel: &[String]) -> Vec<String> {
    // A decoded `/` is read as a break, so `hooks%2Fx` is `hooks` and `x` for the names it earns.
    rel[..free_form_from(rel)]
        .iter()
        .flat_map(|s| s.split('/'))
        .map(str::to_ascii_lowercase)
        .collect()
}

/// A rule that stands over the repo: refused for every method.
const RULESET: &[&str] = &[
    "rulesets",
    "rules",
    "protection",
    "protected_branches",
    "protected_tags",
    "approval_rules",
    "approval_settings",
    "push_rule",
    "push_rules",
    "deployment_protection_rules",
    "deployment-protection-rules",
];

/// The repo's administration: refused for every method, reads included, because what a hook, a
/// key or a variable holds is the point.
const ADMIN: &[&str] = &[
    "hooks",
    "keys",
    "deploy_keys",
    "deploy_tokens",
    "access_tokens",
    "secrets",
    "variables",
    "collaborators",
    "invitations",
    "members",
    "settings",
    "transfer",
    "teams",
    "pages",
    "autolinks",
    "vulnerability-alerts",
    "security-and-analysis",
    "runners",
    "permissions",
    "environments",
    "integrations",
    "services",
    "import",
    "rename",
    "dispatches",
    "installation",
];

const MERGE: &[&str] = &[
    "merge",
    "merges",
    "merge_when_pipeline_succeeds",
    "cancel_merge_when_pipeline_succeeds",
    "auto-merge",
    "auto_merge",
];

const APPROVAL: &[&str] = &["approve", "unapprove", "approvals", "approval", "reviews"];

const RELEASE: &[&str] = &["releases", "tags", "tag"];

fn any(names: &[String], list: &[&str]) -> bool {
    names.iter().any(|n| list.contains(&n.as_str()))
}

/// The refusal a route earns by its name, if it earns one.
fn named_refusal(method: &str, rel: &[String], target: &Target) -> Option<Refusal> {
    let names = names(rel);
    if any(&names, RULESET) {
        return Some(Refusal::Ruleset);
    }
    if any(&names, ADMIN) {
        return Some(Refusal::Admin);
    }
    if is_read(method) {
        return None;
    }
    if any(&names, MERGE)
        || target.has_query_key("merge_when_pipeline_succeeds")
        || target.has_query_key("auto_merge")
    {
        return Some(Refusal::Merge);
    }
    if any(&names, APPROVAL) {
        return Some(Refusal::Approval);
    }
    if any(&names, RELEASE) {
        return Some(Refusal::ReleaseOrTag);
    }
    if rel.is_empty() {
        // `PATCH` or `DELETE` on the repo itself: its name, visibility, archive, deletion.
        return Some(Refusal::Admin);
    }
    None
}

/// The writes a chat may make on its repo.
fn write_class(kind: Kind, method: &str, rel: &[String]) -> Option<Class> {
    let allow = |rows: &[(&str, &[&str], Class)]| {
        rows.iter()
            .find(|(m, pattern, _)| *m == method && matches(rel, pattern))
            .map(|(_, _, class)| class.clone())
    };
    match kind {
        Kind::GitHub => allow(&[
            ("POST", &["pulls"], Class::PullRequestWrite),
            ("PATCH", &["pulls", "#"], Class::PullRequestWrite),
            (
                "PUT",
                &["pulls", "#", "update-branch"],
                Class::PullRequestWrite,
            ),
            (
                "POST",
                &["pulls", "#", "requested_reviewers"],
                Class::PullRequestWrite,
            ),
            ("POST", &["pulls", "#", "comments"], Class::CommentWrite),
            (
                "POST",
                &["pulls", "#", "comments", "#", "replies"],
                Class::CommentWrite,
            ),
            ("PATCH", &["pulls", "comments", "#"], Class::CommentWrite),
            ("POST", &["issues"], Class::IssueWrite),
            ("PATCH", &["issues", "#"], Class::IssueWrite),
            ("POST", &["issues", "#", "comments"], Class::CommentWrite),
            ("PATCH", &["issues", "comments", "#"], Class::CommentWrite),
            ("POST", &["issues", "#", "labels"], Class::LabelWrite),
            ("DELETE", &["issues", "#", "labels", "*"], Class::LabelWrite),
        ]),
        Kind::GitLab => allow(&[
            ("POST", &["merge_requests"], Class::PullRequestWrite),
            ("PUT", &["merge_requests", "#"], Class::PullRequestWrite),
            (
                "PUT",
                &["merge_requests", "#", "rebase"],
                Class::PullRequestWrite,
            ),
            (
                "POST",
                &["merge_requests", "#", "notes"],
                Class::CommentWrite,
            ),
            (
                "PUT",
                &["merge_requests", "#", "notes", "#"],
                Class::CommentWrite,
            ),
            (
                "POST",
                &["merge_requests", "#", "discussions"],
                Class::CommentWrite,
            ),
            (
                "POST",
                &["merge_requests", "#", "discussions", "*", "notes"],
                Class::CommentWrite,
            ),
            (
                "PUT",
                &["merge_requests", "#", "discussions", "*"],
                Class::CommentWrite,
            ),
            ("POST", &["issues"], Class::IssueWrite),
            ("PUT", &["issues", "#"], Class::IssueWrite),
            ("POST", &["issues", "#", "notes"], Class::CommentWrite),
            ("PUT", &["issues", "#", "notes", "#"], Class::CommentWrite),
        ]),
    }
}

/// What a write's body may not say: a merge command in any text, and, on GitLab, a merge request
/// aimed at another project.
fn check_body(
    kind: Kind,
    rel: &[String],
    target: &Target,
    body: &[u8],
    repo: &AllowedRepo,
) -> Result<(), Refusal> {
    if commands::body_strings(body)
        .iter()
        .any(|text| commands::has_merge_command(text))
    {
        return Err(Refusal::MergeCommand);
    }
    if kind == Kind::GitLab && rel.first().is_some_and(|f| f == "merge_requests") {
        let mut named: Vec<String> = target
            .query
            .iter()
            .filter(|(k, _)| k.eq_ignore_ascii_case("target_project_id"))
            .map(|(_, v)| v.clone())
            .collect();
        match serde_json::from_slice::<serde_json::Value>(body) {
            Ok(serde_json::Value::Object(map)) => {
                if let Some(v) = map.get("target_project_id") {
                    named.push(v.as_str().map_or_else(|| v.to_string(), str::to_string));
                }
            }
            Ok(_) => {}
            Err(_) => named.extend(
                url::form_urlencoded::parse(body)
                    .filter(|(k, _)| k == "target_project_id")
                    .map(|(_, v)| v.into_owned()),
            ),
        }
        let own = repo.project_id.map(|id| id.to_string());
        if named.iter().any(|n| Some(n) != own.as_ref()) {
            return Err(Refusal::OtherRepo);
        }
    }
    Ok(())
}

/// Decide one REST request under a chat's repo.
pub(crate) fn decide(
    kind: Kind,
    method: &str,
    rel: &[String],
    target: &Target,
    body: &[u8],
    repo: &AllowedRepo,
) -> Result<Class, Refusal> {
    if let Some(refusal) = named_refusal(method, rel, target) {
        return Err(refusal);
    }
    if is_read(method) {
        return Ok(Class::ApiRead);
    }
    let class = write_class(kind, method, rel).ok_or(Refusal::NotOnList)?;
    check_body(kind, rel, target, body, repo)?;
    Ok(class)
}
