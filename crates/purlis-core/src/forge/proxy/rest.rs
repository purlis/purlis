// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The forge APIs' REST routes under a chat's repo: what is refused by name, and the allowlist of
//! writes. `rel` is the path after the repo (`pulls/7/merge`), decoded once.
//!
//! **Reads** (`GET`, `HEAD`) are an allowlist too: the families `gh` and `glab` read, and under
//! pulls, issues, commits and merge requests only the routes listed, because the others follow
//! cross-references into other repos. The repo's administration (hooks, keys, secrets…) is
//! refused by name for every method. **Writes**
//! are an allowlist: a route not named below is refused, whatever it would do. The names exist so
//! the refusal says why; they are never what lets a request through.

use super::commands::{self, Body};
use super::policy::{Class, ObjectRepo, OwnsHead, PullRef, Refusal};
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

/// The reads of a repo's rules that a chat may make (#7358): the rulesets that apply, and the
/// rules that apply to a branch, which `tools/land.mjs` reads for the required checks. Every other
/// route under [`RULESET`], and every write, stays refused.
const RULE_READS: &[&[&str]] = &[
    &["rulesets"],
    &["rulesets", "#"],
    &["rules", "branches", "*"],
];

/// The same on GitLab: the rules a chat may read. `approval_settings`, `push_rules` and
/// every write stay refused under [`RULESET`].
const GITLAB_RULE_READS: &[&[&str]] = &[
    &["protected_branches"],
    &["protected_branches", "*"],
    &["approval_rules"],
];

fn rule_reads(kind: Kind) -> &'static [&'static [&'static str]] {
    match kind {
        Kind::GitHub => RULE_READS,
        Kind::GitLab => GITLAB_RULE_READS,
    }
}

/// What a chat may do to a workflow run of its own repo besides read it (#7358): rerun it, rerun
/// its failed jobs. **Never `approve`**: that runs an outsider's fork code on the repo's runners,
/// so it stays under [`APPROVAL`] and is refused: see ADR 0128's amendment of 2026-11-22.
const CI_WRITES: &[&[&str]] = &[
    &["actions", "runs", "#", "rerun"],
    &["actions", "runs", "#", "rerun-failed-jobs"],
];

/// GitLab's one write: retry a pipeline, with an empty body, as GitHub's rerun. **Never `play`**:
/// it starts a manual job, which can carry variables, and a manual job is gated code (a deploy, a
/// job on a protected ref) that is the person's to start, as GitHub's `approve` is. Never a job's
/// retry (it can enqueue a manual or delayed job at once) nor any cancel (GitHub has none, and it
/// could stop a deploy mid-apply). Never the trigger routes, which start a pipeline with
/// variables of the caller's choosing.
const GITLAB_CI_WRITES: &[&[&str]] = &[&["pipelines", "#", "retry"]];

fn is_ci_write(kind: Kind, method: &str, rel: &[String]) -> bool {
    let list = match kind {
        Kind::GitHub => CI_WRITES,
        Kind::GitLab => GITLAB_CI_WRITES,
    };
    method == "POST" && list.iter().any(|p| matches(rel, p))
}

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

// `play` is GitLab's start of a manual job: held for the person like an approval.
const APPROVAL: &[&str] = &[
    "approve",
    "unapprove",
    "approvals",
    "approval",
    "reviews",
    "play",
];

const RELEASE: &[&str] = &["releases", "tags", "tag"];

/// The words of the routes that appear only in the write rows, the read arms and the prefix of a
/// repo's routes, besides those of the lists and patterns above: with them, [`is_route_word`].
const ROUTE_WORDS: &[&str] = &[
    "repos",
    "api",
    "v3",
    "v4",
    "projects",
    "graphql",
    "user",
    "search",
    "pulls",
    "issues",
    "comments",
    "replies",
    "labels",
    "milestones",
    "update-branch",
    "requested_reviewers",
    "merge_requests",
    "rebase",
    "notes",
    "discussions",
    "uploads",
    "forks",
    "fork",
    "compare",
    "branches",
    "contents",
    "readme",
    "languages",
    "contributors",
    "check-runs",
    "check-suites",
    "statuses",
    "status",
    "git",
    "refs",
    "trees",
    "blobs",
    "commits",
    "matching-refs",
    "ref",
    "actions",
    "runs",
    "jobs",
    "artifacts",
    "workflows",
    "repository",
    "files",
    "tree",
];

/// **Whether `segment` is a word of the route vocabulary** (#7779): a word one of the forge's
/// routes above is written with, which a refusal may show as it is. Any other segment the chat
/// sent is chat-chosen text, and is never shown.
pub(super) fn is_route_word(segment: &str) -> bool {
    let segment = segment.to_ascii_lowercase();
    let word = segment.as_str();
    let listed = [ROUTE_WORDS, RULESET, ADMIN, MERGE, APPROVAL, RELEASE]
        .iter()
        .any(|list| list.contains(&word));
    let patterned = [
        RULE_READS,
        GITLAB_RULE_READS,
        CI_WRITES,
        GITLAB_CI_WRITES,
        GITHUB_READS,
        GITLAB_READS,
        DEPENDABOT_READS,
    ]
    .iter()
    .flat_map(|rows| rows.iter())
    .any(|pattern| pattern.contains(&word));
    !matches!(word, "*" | "#") && (listed || patterned)
}

fn any(names: &[String], list: &[&str]) -> bool {
    names.iter().any(|n| list.contains(&n.as_str()))
}

/// The merge routes `guards.merge` = `log` or `off` opens ([`decide`] with `merging`): GitHub's
/// pull request merge, GitLab's merge request merge and its cancel of merge-when-pipeline-succeeds.
/// Nothing else that merges is here: a branch merge (`POST merges`) and a flag in a query stay
/// refused.
fn merge_class(kind: Kind, method: &str, rel: &[String]) -> Option<Class> {
    let rows: &[(&str, &[&str])] = match kind {
        Kind::GitHub => &[("PUT", &["pulls", "#", "merge"])],
        Kind::GitLab => &[
            ("PUT", &["merge_requests", "#", "merge"]),
            (
                "POST",
                &["merge_requests", "#", "cancel_merge_when_pipeline_succeeds"],
            ),
        ],
    };
    rows.iter()
        .any(|(m, pattern)| *m == method && matches(rel, pattern))
        .then_some(Class::MergeWrite)
}

/// The refusal a route earns by its name, if it earns one. `merging`: `guards.merge` lets a merge
/// route of [`merge_class`] through.
fn named_refusal(
    kind: Kind,
    method: &str,
    rel: &[String],
    target: &Target,
    merging: bool,
) -> Option<Refusal> {
    let names = names(rel);
    if is_ci_write(kind, method, rel) {
        return None;
    }
    let rule_read = is_read(method) && rule_reads(kind).iter().any(|p| matches(rel, p));
    if any(&names, RULESET) && !rule_read {
        return Some(Refusal::Ruleset);
    }
    if any(&names, ADMIN) {
        return Some(Refusal::Admin);
    }
    if is_read(method) {
        return None;
    }
    if (any(&names, MERGE)
        || target.has_query_key("merge_when_pipeline_succeeds")
        || target.has_query_key("auto_merge"))
        && !(merging && merge_class(kind, method, rel).is_some())
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
    if is_ci_write(kind, method, rel) {
        return Some(Class::CiWrite);
    }
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
            ("POST", &["uploads"], Class::UploadWrite),
        ]),
    }
}

/// The reads under a repo that no forge limits to the repo: what `gh` and `glab` read, as
/// patterns. A route that follows cross-references (an issue's timeline, its sub-issues, the
/// pull requests of a commit, a merge request's closed issues) returns another repo's titles
/// and bodies under this token, so it is not on the list, and nothing under `pulls`, `issues`,
/// `commits` or `merge_requests` is allowed by its first word alone.
const GITHUB_READS: &[&[&str]] = &[
    &["pulls"],
    &["pulls", "#"],
    &["pulls", "#", "files"],
    &["pulls", "#", "commits"],
    // Whether it is merged: a bare status, no body.
    &["pulls", "#", "merge"],
    &["pulls", "#", "comments"],
    &["pulls", "#", "reviews"],
    &["pulls", "#", "reviews", "#"],
    &["pulls", "#", "reviews", "#", "comments"],
    &["pulls", "#", "requested_reviewers"],
    &["pulls", "comments"],
    &["pulls", "comments", "#"],
    &["issues"],
    &["issues", "#"],
    &["issues", "#", "comments"],
    &["issues", "#", "labels"],
    &["issues", "comments"],
    &["issues", "comments", "#"],
    &["commits"],
    &["commits", "*"],
    &["commits", "*", "check-runs"],
    &["commits", "*", "check-suites"],
    &["commits", "*", "status"],
    &["commits", "*", "statuses"],
];

const GITLAB_READS: &[&[&str]] = &[
    &["merge_requests"],
    &["merge_requests", "#"],
    &["merge_requests", "#", "changes"],
    &["merge_requests", "#", "diffs"],
    &["merge_requests", "#", "commits"],
    &["merge_requests", "#", "notes"],
    &["merge_requests", "#", "notes", "#"],
    &["merge_requests", "#", "discussions"],
    &["merge_requests", "#", "discussions", "*"],
    &["merge_requests", "#", "pipelines"],
    &["merge_requests", "#", "participants"],
    &["merge_requests", "#", "versions"],
    &["merge_requests", "#", "award_emoji"],
    // Whether a merge request has the approvals it needs: a read of the state, never of
    // the rules' settings.
    &["merge_requests", "#", "approval_state"],
    // Merge trains: who is in a project's train and where. Joining one stays a merge.
    &["merge_trains"],
    &["merge_trains", "merge_requests", "#"],
    &["issues"],
    &["issues", "#"],
    &["issues", "#", "notes"],
    &["issues", "#", "notes", "#"],
    &["issues", "#", "discussions"],
    &["issues", "#", "discussions", "*"],
    &["issues", "#", "award_emoji"],
    // GitLab's commit routes are under `repository`, never a bare `commits`.
    &["repository", "commits"],
    &["repository", "commits", "*"],
    &["repository", "commits", "*", "statuses"],
    &["repository", "commits", "*", "diff"],
    &["repository", "commits", "*", "comments"],
    // What `glab ci` reads: a pipeline and its jobs and test report. Not `bridges`, which names
    // the downstream projects a pipeline triggers, nor `variables`.
    &["pipelines"],
    &["pipelines", "latest"],
    &["pipelines", "#"],
    &["pipelines", "#", "jobs"],
    &["pipelines", "#", "test_report"],
    &["pipelines", "#", "test_report_summary"],
];

/// The Dependabot alert reads: the list, and one alert by its number.
const DEPENDABOT_READS: &[&[&str]] = &[&["dependabot", "alerts"], &["dependabot", "alerts", "#"]];

/// What filters or pages an alert list besides [`READ_QUERY_KEYS`].
const DEPENDABOT_QUERY_KEYS: &[&str] = &[
    "severity",
    "ecosystem",
    "package",
    "manifest",
    "epss_percentage",
    "has",
    "before",
    "after",
];

/// The query keys a read on a listed route may carry. Anything else is refused: a rendering
/// switch (`render_html`), an impersonation (`sudo`) or a key the forge reads that the proxy
/// does not know is the forge doing more than the route says.
const READ_QUERY_KEYS: &[&str] = &[
    "page",
    "per_page",
    "state",
    "status",
    "scope",
    "sort",
    "direction",
    "order",
    "order_by",
    "labels",
    "label_name",
    "milestone",
    "assignee",
    "assignee_id",
    "assignee_username",
    "author",
    "author_id",
    "author_username",
    "creator",
    "mentioned",
    "reviewer_id",
    "reviewer_username",
    "since",
    "until",
    "created_after",
    "created_before",
    "updated_after",
    "updated_before",
    "base",
    "head",
    "source_branch",
    "target_branch",
    "ref",
    "ref_name",
    "sha",
    "path",
    "wip",
    "draft",
    "search",
    "in",
    "iids[]",
    "with_stats",
    "view",
    "include_diverged_commits_count",
    "include_rebase_in_progress",
    "with_merge_status_recheck",
    "access_raw_diffs",
    "unidiff",
    "all",
    "first_parent",
    "name",
    "source",
    "event",
    "pagination",
    // GitLab's `glab mr view` sends it; it is taken out of what is sent on ([`read_allowed`]).
    "render_html",
];

fn read_query_ok(target: &Target) -> bool {
    target.query.iter().all(|(k, _)| {
        READ_QUERY_KEYS
            .iter()
            .any(|allowed| k.eq_ignore_ascii_case(allowed))
    })
}

/// Whether one side of a compare, or a pull request's head, is the recorded repo's: a bare
/// branch, or `owner:branch` (or `owner/repo:branch`) naming it. More than one colon is not a
/// shape the forge documents, so it is not read as one.
pub(super) fn side_is_own(side: &str, repo: &AllowedRepo) -> bool {
    let side = side.to_ascii_lowercase();
    let own = repo.path.to_ascii_lowercase();
    let own_owner = own.split('/').next().unwrap_or_default();
    match side.matches(':').count() {
        0 => true,
        1 => side
            .split_once(':')
            .is_some_and(|(owner, _)| owner == own_owner || owner == own),
        _ => false,
    }
}

/// Whether a pull request's head, as written (`branch`, `owner:branch`, `refs/heads/branch`), is
/// the repo's default branch: a pull request from it is the wrong way round, and nothing meant by
/// it needs it (#7494). Every other unowned head is refused where it is written to, by
/// [`head_is_owned`] (#7526), so a `release/*` head is a pull request like any other.
pub(super) fn head_is_default(side: &str, repo: &AllowedRepo) -> bool {
    let branch = side.rsplit_once(':').map_or(side, |(_, branch)| branch);
    let branch = branch.strip_prefix("refs/heads/").unwrap_or(branch);
    branch.eq_ignore_ascii_case(&repo.default_branch)
}

/// Update-branch (GitHub) and a rebase (GitLab) write to the pull request's **head**, so the head
/// is what the chat must own: a branch of this repo that the push rule's record holds a tip of
/// this chat's own on, equal to the head the forge reports now (#7526). When the forge cannot say
/// what the head is, that is a refusal too. `number` is the pull request's number in `repo`.
fn head_is_owned(
    kind: Kind,
    repo: &AllowedRepo,
    number: &str,
    objects: &dyn ObjectRepo,
    owns: OwnsHead<'_>,
) -> bool {
    let Ok(number) = number.parse::<u64>() else {
        return false;
    };
    let path = repo.path.to_ascii_lowercase();
    objects
        .pull_head(
            kind,
            &repo.host,
            PullRef::Number {
                repo: &path,
                number,
            },
        )
        .is_some_and(|head| owns(repo, &head))
}

/// A compare head or base written `owner:branch` or `owner/repo:branch` reaches that fork's
/// objects: refused unless it names the recorded repo's owner.
fn compare_is_own(kind: Kind, rel: &[String], target: &Target, repo: &AllowedRepo) -> bool {
    match kind {
        Kind::GitHub => rel.get(1).is_none_or(|spec| {
            spec.split("...")
                .flat_map(|side| side.split(".."))
                .all(|side| side_is_own(side, repo))
        }),
        Kind::GitLab => target.query.iter().all(|(k, v)| {
            !k.eq_ignore_ascii_case("from_project_id")
                || repo.project_id.is_some_and(|id| id.to_string() == *v)
        }),
    }
}

/// The reads a chat may make under its repo: an allowlist, per forge, of the families it works
/// with. Anything else (secret scanning, traffic, exports, triggers…) is not on it.
fn read_allowed(kind: Kind, rel: &[String], target: &Target, repo: &AllowedRepo) -> bool {
    // No read impersonates, on any route.
    // `render_html` is not refused on GitLab, whose own `glab mr view` and `issue view` send it: it
    // is taken out of what is sent on instead ([`super::policy`]'s `upstream`), so the forge never
    // renders references (which can name another project's titles) for a chat. GitHub has no such
    // switch on a read: one there is refused.
    if target.has_query_key("sudo")
        || (kind == Kind::GitHub && target.has_query_key("render_html"))
        || target.has_query_key("include_html_description")
    {
        return false;
    }
    let first = rel.first().map(|s| s.to_ascii_lowercase());
    let Some(first) = first else {
        // The repo itself: its name, default branch and visibility.
        return true;
    };
    let second = rel.get(1).map(|s| s.to_ascii_lowercase());
    let listed = |rows: &[&[&str]]| {
        rows.iter().any(|pattern| matches(rel, pattern)) && read_query_ok(target)
    };

    match kind {
        Kind::GitHub => match first.as_str() {
            "pulls" | "issues" | "commits" => listed(GITHUB_READS),
            "compare" => rel.len() <= 2 && compare_is_own(kind, rel, target, repo),
            "branches" | "contents" | "readme" | "languages" | "contributors" | "labels"
            | "milestones" | "tags" | "releases" | "check-runs" | "check-suites" | "statuses"
            | "status" => true,
            "git" => matches!(
                second.as_deref(),
                Some("refs" | "trees" | "blobs" | "commits" | "matching-refs" | "ref")
            ),
            "rulesets" | "rules" => listed(RULE_READS),
            // A repo's Dependabot alerts, to match one to the pull request that fixes it (#7681):
            // read only (an alert's `PATCH` dismisses it, and is not on the write list), and not
            // Dependabot's secrets, which are administration.
            "dependabot" => {
                DEPENDABOT_READS.iter().any(|pattern| matches(rel, pattern))
                    && target.query.iter().all(|(k, _)| {
                        READ_QUERY_KEYS
                            .iter()
                            .chain(DEPENDABOT_QUERY_KEYS)
                            .any(|allowed| k.eq_ignore_ascii_case(allowed))
                    })
            }
            "actions" => matches!(
                second.as_deref(),
                Some("runs" | "jobs" | "artifacts" | "workflows")
            ),
            _ => false,
        },
        Kind::GitLab => match first.as_str() {
            "merge_requests" | "issues" | "pipelines" | "merge_trains" => listed(GITLAB_READS),
            "protected_branches" | "approval_rules" => listed(GITLAB_RULE_READS),
            "jobs" | "labels" | "milestones" | "languages" | "releases" => true,
            "repository" => match second.as_deref() {
                Some("compare") => compare_is_own(kind, rel, target, repo),
                Some("commits") => listed(GITLAB_READS),
                Some("tree" | "files" | "blobs" | "branches" | "tags" | "contributors") => true,
                _ => false,
            },
            _ => false,
        },
    }
}

/// Keys that put a GitLab issue under an epic or a parent, which live outside the recorded repo.
const GITLAB_PARENT_KEYS: &[&str] = &["epic_id", "epic_iid", "parent_id", "parent_iid"];

/// The top-level keys a body names, JSON or form.
fn body_keys(body: &Body) -> impl Iterator<Item = &str> {
    body.json
        .as_ref()
        .and_then(|j| j.as_object())
        .into_iter()
        .flat_map(|o| o.keys().map(String::as_str))
        .chain(body.form.iter().map(|(k, _)| k.as_str()))
}

/// Every value a body gives `key`, as text; a value that is not a string is its JSON, which no
/// repo or branch name matches.
fn body_values(body: &Body, key: &str) -> Vec<String> {
    body.json
        .as_ref()
        .and_then(|j| j.get(key))
        .map(|v| v.as_str().map_or_else(|| v.to_string(), str::to_string))
        .into_iter()
        .chain(
            body.form
                .iter()
                .filter(|(k, _)| k == key)
                .map(|(_, v)| v.clone()),
        )
        .collect()
}

/// Whether `target`'s query is GitLab's own `merge_when_pipeline_succeeds=true|false`, **once, with
/// that exact key** and nothing else, on a merge request's merge (`PUT …/merge_requests/<iid>/merge`):
/// the form `glab` and the API's own docs send it in, which `guards.merge` = `log` or `off` takes
/// as the body's field. A repeated flag or a key in another case is a query a forge may read
/// differently from the proxy, so it stays refused.
fn is_the_merge_flag(kind: Kind, method: &str, rel: &[String], target: &Target) -> bool {
    kind == Kind::GitLab
        && method == "PUT"
        && matches(rel, &["merge_requests", "#", "merge"])
        && matches!(
            target.query.as_slice(),
            [(k, v)] if k == "merge_when_pipeline_succeeds"
                && matches!(v.as_str(), "true" | "false")
        )
}

/// What a write's request may not say: a query at all (but the merge flag of
/// [`is_the_merge_flag`], where `merging`), a merge command in any text, and, on GitLab, a merge
/// request aimed at another project.
fn check_body(
    kind: Kind,
    method: &str,
    rel: &[String],
    target: &Target,
    body: Option<&Body>,
    repo: &AllowedRepo,
    merging: bool,
) -> Result<(), Refusal> {
    // No write carries a query: an unknown parameter is the forge reading something the proxy
    // did not.
    if !target.query.is_empty() && !(merging && is_the_merge_flag(kind, method, rel, target)) {
        return Err(Refusal::MalformedRequest);
    }
    let Some(body) = body else {
        return Ok(());
    };
    if body.label_by_id {
        return Err(Refusal::LabelById);
    }
    if body
        .strings
        .iter()
        .any(|text| commands::has_merge_command(text))
        || body
            .labels
            .iter()
            .any(|label| commands::is_merge_label(label) || commands::label_has_control(label))
    {
        return Err(Refusal::MergeCommand);
    }
    if kind == Kind::GitLab {
        if body
            .strings
            .iter()
            .any(|text| commands::has_quick_action(text))
        {
            return Err(Refusal::QuickAction);
        }
        // An issue moved under an epic or a parent lives in a group the chat did not record.
        if body_keys(body).any(|k| GITLAB_PARENT_KEYS.contains(&k.to_ascii_lowercase().as_str())) {
            return Err(Refusal::OtherRepo);
        }
    }
    if kind == Kind::GitHub && method == "POST" && rel == ["pulls"] {
        let own_repo = |v: &str| v.eq_ignore_ascii_case(&repo.path);
        if body_values(body, "head")
            .iter()
            .any(|v| !side_is_own(v, repo))
            || body_values(body, "head_repo").iter().any(|v| !own_repo(v))
        {
            return Err(Refusal::OtherRepo);
        }
        // A pull request from the default branch into the chat's own makes update-branch merge
        // that branch into the default one on the forge's side (#7494).
        if body_values(body, "head")
            .iter()
            .any(|v| head_is_default(v, repo))
        {
            return Err(Refusal::ProtectedPullHead);
        }
    }
    if kind == Kind::GitLab
        && rel
            .first()
            .is_some_and(|f| f.eq_ignore_ascii_case("merge_requests"))
    {
        let mut named: Vec<String> = Vec::new();
        if let Some(v) = body.json.as_ref().and_then(|j| j.get("target_project_id")) {
            named.push(v.as_str().map_or_else(|| v.to_string(), str::to_string));
        }
        named.extend(
            body.form
                .iter()
                .filter(|(k, _)| k.eq_ignore_ascii_case("target_project_id"))
                .map(|(_, v)| v.clone()),
        );
        let own = repo.project_id.map(|id| id.to_string());
        if named.iter().any(|n| Some(n) != own.as_ref()) {
            return Err(Refusal::OtherRepo);
        }
        // A merge request from the default branch is the wrong way round (#7494).
        if method == "POST"
            && rel.len() == 1
            && body_values(body, "source_branch").iter().any(|v| {
                v.strip_prefix("refs/heads/")
                    .unwrap_or(v)
                    .eq_ignore_ascii_case(&repo.default_branch)
            })
        {
            return Err(Refusal::ProtectedPullHead);
        }
    }
    Ok(())
}

/// Whether a body is no body at all: no text, or a JSON `{}`.
fn is_empty_object(body: &Body) -> bool {
    body.form.is_empty()
        && body.strings.is_empty()
        && body
            .json
            .as_ref()
            .is_none_or(|j| j.as_object().is_some_and(|m| m.is_empty()))
}

/// Decide one REST request under a chat's repo.
#[allow(
    clippy::too_many_arguments,
    reason = "one request's parts, read together; a struct for them would only be built to be taken apart"
)]
pub(crate) fn decide(
    kind: Kind,
    method: &str,
    rel: &[String],
    target: &Target,
    body: Option<&Body>,
    repo: &AllowedRepo,
    objects: &dyn ObjectRepo,
    owns: OwnsHead<'_>,
    merging: bool,
) -> Result<Class, Refusal> {
    if let Some(refusal) = named_refusal(kind, method, rel, target, merging) {
        return Err(refusal);
    }
    if is_read(method) {
        return if read_allowed(kind, rel, target, repo) {
            Ok(Class::ApiRead)
        } else {
            Err(Refusal::NotOnList)
        };
    }
    let class = write_class(kind, method, rel)
        .or_else(|| merging.then(|| merge_class(kind, method, rel)).flatten())
        .ok_or(Refusal::NotOnList)?;
    check_body(kind, method, rel, target, body, repo, merging)?;
    let writes_head = method == "PUT"
        && match kind {
            Kind::GitHub => matches(rel, &["pulls", "#", "update-branch"]),
            Kind::GitLab => matches(rel, &["merge_requests", "#", "rebase"]),
        };
    if writes_head && !head_is_owned(kind, repo, &rel[1], objects, owns) {
        return Err(Refusal::ProtectedPullHead);
    }
    if class == Class::CiWrite && body.is_some_and(|b| !is_empty_object(b)) {
        // A rerun's body is `enable_debug_logging` and nothing else the chat may choose; GitLab's
        // retry and cancel take none.
        return Err(Refusal::MalformedRequest);
    }
    Ok(class)
}
