// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Opening or updating a pull request — a merge request on GitLab — asking the forge to merge
//! it once it may, and, for `charter change land` alone, merging it. The forge adapters' first
//! write that is not a push (ADR 0051).
//!
//! The calls, the same on both forges:
//!
//! - `open_or_update`: the open PR from `head` into `base`, updated with a new title and body,
//!   or a new one when there is none. Never a second one for the same pair.
//! - `request_auto_merge`: the forge queues it to merge when its checks allow, by the
//!   repo's own method, preferring rebase, then a merge commit, then squash — and only at the
//!   commit this save pushed.
//! - `merge_when_ready`: purlis merges a `pr-merge` save's request itself, at the commit the
//!   save pushed, once the forge says it may merge and purlis has read every check at that
//!   commit as passed. Never past a failing check, required or not, a pending one or a
//!   required review; and with no check at all, only once the commit has been on the forge
//!   long enough for checks to have started and nothing says any will ([`Gate`], ADR 0051,
//!   amended 2026-10-13). A branch that lands through a merge queue or train is put in it then,
//!   and never earlier.
//! - `merge_at` and `enqueue_at`: `charter change land`'s merge, now or through the target
//!   branch's merge queue or merge train, each pinned to the head commit whose checks charter
//!   read (ADR 0060 D3, ruling Q16). `lands_through_queue` says which.
//!
//! # The base is always named
//!
//! A PR is opened into the `base` the caller passes: GitHub's `base`, GitLab's
//! `target_branch`. The compare link this replaces left it out, and GitHub then resolved it
//! against the repo's default branch, so a plane with `[plane] branch = "release"` was offered
//! a PR into `main` (#298).
//!
//! # Credentials
//!
//! The same as the rest of [`crate::forge`]: every question goes through a backend's
//! [`super::transport::Transport`], today the forge's own CLI, so the token stays in that CLI
//! and is never read, passed or printed here. Every value that came from a repo or a person —
//! a branch, a title, a body — is a literal field, which the CLI transport sends with `-f`;
//! `-F` would read a leading `@` as a file to upload (charter #323). A typed field carries
//! only values charter writes itself: a number and `true`/`false`.
//!
//! The operations themselves are [`super::backend::Requests`]' methods, with a body per forge
//! in [`super::github`] and [`super::gitlab`]. This module holds their neutral types and the
//! rules both forges share.
//!
//! # Only the app asks for auto-merge, and only `charter change land` merges
//!
//! An agent never merges: `floorguard` refuses `gh pr merge` and `charter change land` from a
//! session nobody is watching, and keeps doing so. **A save no longer asks the forge to
//! auto-merge** (ADR 0051, amended 2026-10-13): a forge's own auto-merge waits for the checks
//! the branch *requires* and merges past a failing one that is not, so a request handed to it
//! can merge red. Every `pr-merge` save takes what was [`Capability::AutoMerge`]'s fallback,
//! [`Fallback::MergeWhenReady`]: `merge_when_ready`, behind purlis's own reading of every
//! check. `request_auto_merge` stays on the seam, with its contract cases, and no save calls
//! it: when the forge will not queue a merge because there is nothing to wait for, its answer
//! is [`AutoMerge::NotQueued`] with the forge's reason, and when the repo or its plan has no
//! auto-merge at all, [`AutoMerge::Unavailable`].
//!
//! [`Capability::AutoMerge`]: super::backend::Capability::AutoMerge
//! [`Fallback::MergeWhenReady`]: super::backend::Fallback::MergeWhenReady
//!
//! `merge_at` and `enqueue_at` do merge, and their one caller is `charter change land`, after
//! its gates (ADR 0060 D3, which made this module's earlier "nothing here merges" false). They
//! never ask the forge to merge *later*: neither sets auto-merge (GitLab's
//! `merge_when_pipeline_succeeds` or `auto_merge`), which would merge whatever head the branch
//! has when a pipeline passes rather than the one charter read.

use std::path::Path;

use serde_json::Value;

use super::checks::{Checks, Ci, FailedCheck};
use super::{Forge, ForgeError, Kind};
use crate::worktree::git;

/// A repository on a forge: which forge, and its path there (`owner/name` on GitHub, the full
/// namespace on GitLab).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repo {
    pub forge: Forge,
    pub path: String,
}

/// An open pull or merge request: the number the forge shows (a GitLab MR's `iid`, not its
/// global `id`) and its web page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pr {
    pub number: u64,
    pub url: String,
}

/// A picture to show on a request's page, as browser proof (#2026): its file name, which purlis
/// makes (`browser-proof-<n>.png`, never anything an agent wrote), and its PNG bytes, which the
/// person picked and saw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub name: String,
    pub png: Vec<u8>,
}

/// The branch a GitHub repo keeps request `number`'s browser proof on (#2026): a branch of
/// purlis's own whose commits hold only those pictures, none with a parent outside it, so no
/// workflow file is on it and nothing runs when it moves. A save's branch ends in a commit's
/// short hash, never `pr-<n>`, so the two never meet.
pub fn proof_branch(number: u64) -> String {
    format!(
        "{}browser-proof/pr-{number}",
        crate::names::BRANCH_PREFIX.write
    )
}

/// Where a request's browser proof is kept on its forge, and who can open it there, in the
/// words the person is shown before anything is sent (#2026). `uploads_need_sign_in` is a
/// GitLab project's `enforce_auth_checks_on_uploads`, where it was read: without it, an upload is
/// opened by anyone who has its link.
pub fn proof_kept_at(
    kind: Kind,
    path: &str,
    number: u64,
    uploads_need_sign_in: Option<bool>,
) -> String {
    match (kind, uploads_need_sign_in) {
        (Kind::GitHub, _) => format!(
            "on the branch {} of {path}, in its history for good",
            proof_branch(number)
        ),
        (Kind::GitLab, Some(true)) => format!(
            "as uploads of {path}, which only those who can read {path} can open; they stay \
             after the comment is deleted"
        ),
        (Kind::GitLab, _) => format!(
            "as uploads of {path}, which anyone with their link can open, signed in or not; \
             they stay after the comment is deleted"
        ),
    }
}

/// Whether anyone with a picture's link can open browser proof kept on `kind`, signed in or not,
/// whoever can read the repo: a GitLab upload, unless the project checks who opens its uploads
/// (`uploads_need_sign_in`). GitHub draws a picture only for whoever can read the repo.
pub fn proof_open_by_link(kind: Kind, uploads_need_sign_in: Option<bool>) -> bool {
    matches!(kind, Kind::GitLab) && uploads_need_sign_in != Some(true)
}

/// Whether attaching proof on `kind` writes to the repo itself, so it needs push access there:
/// GitHub's proof branch does; GitLab's uploads need only the right to comment (#2026).
pub fn proof_needs_push(kind: Kind) -> bool {
    matches!(kind, Kind::GitHub)
}

/// The mark a request's number takes on `kind`: `#12` on GitHub, `!12` on GitLab.
pub fn request_mark(kind: Kind) -> char {
    match kind {
        Kind::GitHub => '#',
        Kind::GitLab => '!',
    }
}

/// What comes between a repo's page and a request's number in the request's page on `kind`.
pub fn request_page_between(kind: Kind) -> &'static str {
    match kind {
        Kind::GitHub => "/pull/",
        Kind::GitLab => "/-/merge_requests/",
    }
}

/// Whether `name` is a picture name browser proof writes: `browser-proof-<digits>.png`.
pub fn is_proof_name(name: &str) -> bool {
    name.strip_prefix("browser-proof-")
        .and_then(|rest| rest.strip_suffix(".png"))
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// Whether `sha` is a whole git object name, as a forge answers one: forty or sixty-four
/// lower-case hex digits.
pub(super) fn is_object_name(sha: &str) -> bool {
    matches!(sha.len(), 40 | 64) && sha.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// How `charter change land` has its own landing commit made: a merge commit or a squash, with
/// the subject and body charter writes, which carry the `Charter-Change:` trailer where the
/// forge lets charter write the message (ADR 0060 §4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeAs {
    pub squash: bool,
    pub title: String,
    pub message: String,
}

/// What `merge_at` got the forge to do, when it did not refuse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergedAt {
    /// Merged now, at the head charter named.
    Now,
    /// The forge set the request to merge later instead, and charter could not undo that, in
    /// these words. Whatever head the branch has when a later pipeline passes is what merges.
    Later(String),
}

/// What `request_auto_merge` got the forge to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutoMerge {
    /// The forge will merge the PR, at the pushed commit, once its checks pass.
    Queued,
    /// The forge added the request to its target branch's merge train (#805): GitLab merges it
    /// in its turn, once the train's pipeline for it passes.
    JoinedTrain,
    /// The forge would not queue it because there is nothing to wait for — no checks, or a
    /// pipeline already finished — and nothing was merged. Carries the forge's reason.
    NotQueued(String),
    /// The forge has no auto-merge for this repo: its setting is off, or its plan has none
    /// (GitHub offers none for a private repo on the Free plan). Nothing was asked of the PR.
    Unavailable(super::backend::Unavailable),
}

/// What `merge_when_ready` found, and did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ready {
    /// Merged now, at the commit the save pushed, by the repo's own method — or merged
    /// already, by somebody else.
    Merged,
    /// Not yet: a check at the commit is still running, none has had time to start, none has
    /// started where the repository's other requests have them, or the forge is still working
    /// out whether it can merge. Nothing was merged; asking again later may. Carries why.
    Waiting(String),
    /// Not yet, and nothing purlis reads says when: no check has started where the
    /// repository's other requests have them (a workflow its paths filtered out looks the
    /// same as one that has not started), its checks cannot be read at this commit, or a
    /// check its branch requires has not reported. Nothing was merged, and purlis looks again
    /// as for [`Ready::Waiting`]; it is the person's own merge that ends a wait like this one
    /// (the operator's ruling, 2026-10-14). Carries why.
    Stalled(String),
    /// A check at the commit failed, required or not, and nothing was merged. It stays open;
    /// a later save pushes onto the same request. Carries the checks that failed.
    Red(Vec<FailedCheck>),
    /// Every check passed, and its branch lands through a merge queue or a merge train: purlis
    /// put it there, at the commit the save pushed, and the forge merges it in its turn.
    /// Nothing more is asked of the forge about merging it. Carries where it went, in words.
    Queued(String),
    /// It cannot merge as it stands — it conflicts, a review or another rule the branch has is
    /// not met, or its head moved — and nothing was merged. It stays open for a person.
    /// Carries why, in words.
    Refused(String),
}

/// The one request `merge_when_ready` may merge, as the save that opened it defines it: the
/// commit it pushed, the branch it pushed that commit to, and the branch the request goes
/// into. The forge's own answer must match all three ([`not_this_saves`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pinned<'a> {
    /// The commit the save pushed.
    pub sha: &'a str,
    /// The save's own branch on the forge: the plane's save branch for this clone, or the
    /// branch a repo's save pushed.
    pub branch: &'a str,
    /// The configured target: `[plane] branch`, or a repo's `[repos.<name>] branch` or its
    /// default branch.
    pub base: &'a str,
}

/// What stands between a save's request and `merge_when_ready` merging it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gate {
    /// purlis's own reading of the checks at the head: every one passed, or none exists and
    /// none will. `settled` is whether the head has been on the forge long enough for its
    /// checks to have started ([`SETTLE`]): until it has, no check is "not yet", never "none".
    Checks { settled: bool },
    /// The person in the window said to merge it as it stands, failed and missing checks
    /// included (the operator's ruling P1, 2026-10-13). Only the checks are passed over: a
    /// conflict, a required review and every rule the forge itself enforces still refuse it.
    Person,
}

/// How long a pushed commit is given for its checks to start before none at all is read as
/// "none ran": a forge registers a workflow's checks seconds after the push, not at it.
pub const SETTLE: std::time::Duration = std::time::Duration::from_secs(60);

impl Repo {
    /// Its forge's backend, over the transport every call takes today.
    pub fn backend(&self) -> Box<dyn super::backend::ForgeBackend> {
        self.forge.backend()
    }

    /// The repo the plane's own `origin` names: [`Repo::of_clone`] of the plane itself.
    pub fn of_plane(plane: &Path) -> Result<Repo, String> {
        Repo::of_clone(plane, plane)
    }

    /// The repo a clone's `origin` names, on a forge `plane`'s `charter.toml` declares or a
    /// kind's default host. Refused for any other host: a request mode there is a config error
    /// (ADR 0051), and charter does not guess which API an unknown host speaks.
    pub fn of_clone(plane: &Path, clone: &Path) -> Result<Repo, String> {
        let url = git::run(clone, &["remote", "get-url", "origin"], git::READ)
            .map(|run| run.out.trim().to_string())
            .unwrap_or_default();
        if url.is_empty() {
            return Err(format!(
                "{} has no origin, so there is nowhere to open a pull request",
                clone.display()
            ));
        }
        let Some(forge) = super::resolve_host(&url, plane) else {
            return Err(format!(
                "origin {url} is not on a GitHub or GitLab host this plane declares, so purlis \
                 cannot open a pull request there. Add it as a [[forge]] in charter.toml, or \
                 choose a mode without a pull request"
            ));
        };
        let Some(path) = super::namespace_of(&url) else {
            return Err(format!("origin {url} names no repository"));
        };
        Ok(Repo { forge, path })
    }
}

/// The request's number and page out of a forge's record of it.
pub(super) fn pr_of(
    record: &Value,
    number_key: &str,
    url_key: &str,
    doing: &str,
) -> Result<Pr, ForgeError> {
    let number = record.get(number_key).and_then(Value::as_u64);
    let url = record.get(url_key).and_then(Value::as_str);
    match (number, url) {
        (Some(number), Some(url)) => Ok(Pr {
            number,
            url: url.to_string(),
        }),
        _ => Err(ForgeError::new(format!(
            "{doing}: the answer named no {number_key} and {url_key}"
        ))),
    }
}

/// The first record of a listing, or `None` for an empty one. GitHub's lookup names the head
/// by `owner:branch`, so a fork's PR never matches it.
pub(super) fn first(listing: &Value) -> Option<&Value> {
    listing.as_array().and_then(|items| items.first())
}

/// The first MR in a listing whose source branch is in the project itself. GitLab's lookup
/// matches `source_branch` by name only and lists MRs from forks too, so a stranger's fork
/// MR from a branch of the same name would otherwise be rewritten and set to merge.
pub(super) fn own_mr(listing: &Value) -> Option<&Value> {
    listing.as_array()?.iter().find(|mr| {
        let source = mr.get("source_project_id").and_then(Value::as_u64);
        source.is_some() && source == mr.get("target_project_id").and_then(Value::as_u64)
    })
}

/// The line charter puts in the body of every pull request it writes, so it can tell its own
/// from one a person opened from the same branch. Bodies already on a forge carry the old
/// spelling, which [`is_ours`] recognises forever ([`crate::names::SAVE_PR_MARKER`]).
pub const MARKER: &str = crate::names::SAVE_PR_MARKER.write;

/// What `open_or_update` found or made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opened {
    pub pr: Pr,
    /// Whether the PR is charter's: one it opened now, or an open one whose head branch is
    /// under the product's branch prefix or whose body carries [`MARKER`], either under any
    /// spelling it has had ([`crate::names`]). A PR that is not charter's was left
    /// exactly as it was, and nothing may be asked of the forge about it — auto-merge above all.
    pub ours: bool,
}

/// Whether an open PR is charter's to rewrite: its head branch is one charter names, or its
/// body carries [`MARKER`] — the purlis names, or the charter ones a PR opened before the
/// rename still has (V93j).
pub(super) fn is_ours(head: &str, body: Option<&str>) -> bool {
    use crate::names::{BRANCH_PREFIX, SAVE_PR_MARKER};
    BRANCH_PREFIX.strip(head).is_some()
        || body.is_some_and(|b| SAVE_PR_MARKER.spellings().any(|m| b.contains(m)))
}

/// Where a pull or merge request stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    Open,
    /// Merged, as `commit` on the target branch: the merge commit, the squash commit, or the
    /// last commit a rebase merge wrote. `None` when the forge names none.
    Merged {
        commit: Option<String>,
    },
    /// Closed without merging.
    Closed,
}

/// A commit a request record names under `key`, if it names a non-empty one.
pub(super) fn commit_named(record: &Value, key: &str) -> Option<String> {
    record[key]
        .as_str()
        .filter(|sha| !sha.is_empty())
        .map(str::to_string)
}

/// A request record whose `state` is none a backend knows.
pub(super) fn unknown_state(record: &Value, doing: &str) -> ForgeError {
    let word = record["state"].as_str().unwrap_or("");
    ForgeError::new(format!("{doing}: the forge answered the state {word:?}"))
}

/// A pull or merge request found by its head branch: what `charter change show` reads of each
/// member (ADR 0060).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub number: u64,
    pub url: String,
    pub state: State,
    /// The commit the request's branch is at, as the forge says. Checks are read at exactly
    /// this commit and at no other.
    pub head: String,
}

/// GitHub's methods in charter's order of preference: `(setting, GraphQL enum)`. Rebase keeps
/// each save's commit as it was made; a merge commit keeps them too; squash rewrites them into
/// one, which is why it comes last (ADR 0051).
pub(super) const GITHUB_METHODS: [(&str, &str); 3] = [
    ("rebaseMergeAllowed", "REBASE"),
    ("mergeCommitAllowed", "MERGE"),
    ("squashMergeAllowed", "SQUASH"),
];

/// GitHub's refusals that mean "nothing to wait for": a PR that could merge now (`clean`), or
/// one whose only checks are not required (`unstable`), cannot be put on auto-merge.
pub(super) const GITHUB_NOTHING_TO_WAIT_FOR: [&str; 2] = ["clean status", "unstable status"];

/// GitHub's refusal of `enablePullRequestAutoMerge` in a repo that has no auto-merge: its
/// setting is off, or its plan has none.
pub(super) const GITHUB_AUTO_MERGE_NOT_ALLOWED: &str = "Auto merge is not allowed";

/// A GitLab pipeline still to finish, so there is something to wait for. GitLab 19.4 lists
/// thirteen statuses (`doc/api/pipelines.md`); these are the ones that have not started or
/// are still going, `waiting_for_callback` among them.
pub(super) const GITLAB_ACTIVE: [&str; 7] = [
    "created",
    "waiting_for_resource",
    "preparing",
    "waiting_for_callback",
    "pending",
    "running",
    "scheduled",
];

/// GitLab's refusals of the merge call that mean "this MR cannot be merged now": 405 (not
/// mergeable) and 422 (it cannot be set to merge). Any other refusal — a 409 for a head that
/// moved past `sha`, a 401 — is an error.
pub(super) const GITLAB_NOT_MERGEABLE: [&str; 2] = ["(HTTP 405)", "(HTTP 422)"];

/// `Queued` for a call that succeeded, `NotQueued` with the forge's words for a refusal that
/// names one of `nothing_to_wait_for`, and an error for every other failure.
pub(super) fn not_queued_when(
    said: Result<Result<(), String>, ForgeError>,
    nothing_to_wait_for: &[&str],
    doing: &str,
) -> Result<AutoMerge, ForgeError> {
    match said? {
        Ok(()) => Ok(AutoMerge::Queued),
        Err(why) if nothing_to_wait_for.iter().any(|w| why.contains(w)) => {
            Ok(AutoMerge::NotQueued(why))
        }
        Err(why) => Err(ForgeError::new(format!("{doing} failed: {why}"))),
    }
}

/// What the checks at the head say before a merge purlis makes itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Gated {
    /// Every check there passed, or the person said to merge it as it stands.
    Go,
    /// No check exists at the head, and it has had time to: the backend now asks whether the
    /// repository shows any sign that one will run ([`none_will_run`]).
    NoneRan,
    /// It waits, or it is red.
    Stop(Ready),
}

/// What [`Gate::Checks`] waits with while the head is too new for "no check" to mean none.
pub const NOT_STARTED_YET: &str =
    "it was pushed a moment ago, and its checks have not had time to start";

/// What [`Gate::Checks`] waits with while the head is too new for the checks that are there
/// to be all of them.
pub const SOME_NOT_STARTED_YET: &str =
    "it was pushed a moment ago, and more of its checks may not have started yet";

/// [`Gated`] for `checks` under `gate`. Silence is never a pass: a forge that gave no answer
/// waits, a head with no check waits until it has had time to get one, and a failed check of
/// any kind stops it, whatever the forge's own rules require.
pub(super) fn check_gate(checks: &Checks, gate: Gate) -> Gated {
    let Gate::Checks { settled } = gate else {
        return Gated::Go;
    };
    match checks.ci {
        Ci::Passed if settled => Gated::Go,
        // The checks that are there passed, and the head is too new for them to be all of
        // them: a quick status lands seconds before a workflow registers its runs.
        Ci::Passed => Gated::Stop(Ready::Waiting(SOME_NOT_STARTED_YET.into())),
        Ci::NotRun if settled => Gated::NoneRan,
        Ci::NotRun => Gated::Stop(Ready::Waiting(NOT_STARTED_YET.into())),
        Ci::Running => Gated::Stop(Ready::Waiting("its checks are still running".into())),
        Ci::Failed => Gated::Stop(Ready::Red(checks.failed.clone())),
        // Not read is not passed, and nothing says when it will be read.
        Ci::Unknown => Gated::Stop(Ready::Stalled(format!(
            "purlis could not read its checks: {}",
            checks.why.as_deref().unwrap_or("no answer")
        ))),
    }
}

/// `gate`, held to the forge's own clock as well as this machine's: a head is settled only
/// when the forge says the request was last touched at least [`SETTLE`] ago. A push touches
/// it, and so does anything later, so this can only wait longer than the push asks, never
/// less. `touched` is the forge's timestamp (RFC 3339); one that is missing, unreadable or
/// in the future does not settle anything.
///
/// The forge is the truth; what is kept on disk about the push is a hint. Both must say the
/// head has been there for the wait.
pub(super) fn held_to_the_forges_clock(gate: Gate, touched: Option<&str>) -> Gate {
    let Gate::Checks { settled: true } = gate else {
        return gate;
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default();
    let settled = touched
        .and_then(|at| chrono::DateTime::parse_from_rfc3339(at).ok())
        .is_some_and(|at| now - at.timestamp() >= SETTLE.as_secs() as i64);
    Gate::Checks { settled }
}

/// Why a request the forge answered is not the one this save may merge, or `None` when it
/// is. Asked of the forge at every look and for the person's merge; all of these must hold,
/// by the forge's own answer:
///
/// - it is from this repository, not a fork;
/// - its head branch is the branch this save pushes ([`Pinned::branch`]): the plane's save
///   branch for this clone, or the branch a repo's save pushed. A branch purlis names is not
///   enough; another machine's save and a `purlis change` member's request are not this
///   save's;
/// - it goes into the configured target ([`Pinned::base`]);
/// - it is one purlis opened ([`is_ours`]): a person's own request from the same branch,
///   which a save leaves as it is, is theirs to merge.
///
/// The forge is the truth; what is kept on disk about the request is a hint.
pub(super) fn not_this_saves(
    request: &str,
    at: &Pinned,
    head: Option<&str>,
    base: Option<&str>,
    body: Option<&str>,
    from_this_repo: bool,
) -> Option<Ready> {
    let refused = |why: String| {
        Some(Ready::Refused(format!(
            "{request} {why}, so it is not this save's request, and purlis will not merge it"
        )))
    };
    let shown = |name: Option<&str>| {
        crate::shown::one_line(name.unwrap_or("no branch"), crate::shown::DISPLAY_LIMIT)
    };
    if !from_this_repo {
        return refused("is from another repository".into());
    }
    if head != Some(at.branch) {
        return refused(format!(
            "is from {}, and this save pushes {}",
            shown(head),
            at.branch
        ));
    }
    if base != Some(at.base) {
        return refused(format!(
            "goes into {}, and this save's target is {}",
            shown(base),
            at.base
        ));
    }
    if !is_ours(at.branch, body) {
        return refused("is not a request purlis opened".into());
    }
    None
}

/// [`Gated::NoneRan`], settled by what the repository shows: `Ok(None)` when nothing says a
/// check will run on this head, so it may merge with none; `Ok(Some(why))` when something does,
/// and it waits; and a read that failed waits too, because not knowing is never "none".
pub(super) fn none_will_run(expected: Result<Option<String>, ForgeError>) -> Option<Ready> {
    match expected {
        Ok(None) => None,
        Ok(Some(sign)) => Some(Ready::Stalled(format!(
            "no check has started on it, and {sign}, so purlis waits for its checks"
        ))),
        Err(why) => Some(Ready::Stalled(format!(
            "no check has started on it, and purlis could not read whether this repository              runs any ({})",
            why.said()
        ))),
    }
}

#[cfg(test)]
mod gate_tests {
    use super::*;

    fn checks(ci: Ci, total: u64) -> Checks {
        Checks {
            total: Some(total),
            ci,
            why: None,
            failed: if ci == Ci::Failed {
                vec![FailedCheck::of(Some("lint"), None)]
            } else {
                Vec::new()
            },
        }
    }

    const SETTLED: Gate = Gate::Checks { settled: true };
    const FRESH: Gate = Gate::Checks { settled: false };

    #[test]
    fn no_check_at_a_head_just_pushed_waits_and_is_never_read_as_none_ran() {
        // The fault (2026-10-13): zero checks straight after the push merged, and CI failed
        // on a request already merged.
        assert_eq!(
            check_gate(&checks(Ci::NotRun, 0), FRESH),
            Gated::Stop(Ready::Waiting(NOT_STARTED_YET.into()))
        );
    }

    #[test]
    fn no_check_at_a_settled_head_asks_whether_any_will_run_before_it_merges() {
        assert_eq!(check_gate(&checks(Ci::NotRun, 0), SETTLED), Gated::NoneRan);
        assert_eq!(none_will_run(Ok(None)), None);
        let waits = none_will_run(Ok(Some("the last request into main had checks".into())));
        assert!(
            matches!(&waits, Some(Ready::Stalled(why)) if why.contains("the last request")),
            "{waits:?}"
        );
        let unread = none_will_run(Err(ForgeError::new("HTTP 502".to_string())));
        assert!(
            matches!(&unread, Some(Ready::Stalled(why)) if why.contains("could not read")),
            "{unread:?}"
        );
    }

    #[test]
    fn a_failed_check_is_red_with_its_name_whether_or_not_the_head_is_settled() {
        for gate in [FRESH, SETTLED] {
            let Gated::Stop(Ready::Red(failed)) = check_gate(&checks(Ci::Failed, 2), gate) else {
                panic!("a failed check let it through");
            };
            assert_eq!(failed[0].name, "lint");
        }
    }

    #[test]
    fn running_and_unreadable_checks_wait_and_passed_ones_go() {
        for gate in [FRESH, SETTLED] {
            assert!(matches!(
                check_gate(&checks(Ci::Running, 1), gate),
                Gated::Stop(Ready::Waiting(_))
            ));
            // Not read is not passed, and nothing says when it will be: the person's to end.
            assert!(matches!(
                check_gate(&checks(Ci::Unknown, 0), gate),
                Gated::Stop(Ready::Stalled(_))
            ));
        }
        assert_eq!(check_gate(&checks(Ci::Passed, 1), SETTLED), Gated::Go);
    }

    #[test]
    fn a_passed_check_at_a_head_just_pushed_waits_for_the_checks_that_have_not_started() {
        // Review N1: a bot's green status two seconds after the push, with the workflow's own
        // runs not registered yet, is not "every check passed".
        assert_eq!(
            check_gate(&checks(Ci::Passed, 1), FRESH),
            Gated::Stop(Ready::Waiting(SOME_NOT_STARTED_YET.into()))
        );
    }

    #[test]
    fn only_the_person_passes_over_the_checks() {
        for ci in [Ci::Failed, Ci::Running, Ci::NotRun, Ci::Unknown, Ci::Passed] {
            assert_eq!(
                check_gate(&checks(ci, 1), Gate::Person),
                Gated::Go,
                "{ci:?}"
            );
        }
    }
}
