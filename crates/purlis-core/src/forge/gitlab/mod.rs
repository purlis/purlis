// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The GitLab backend: every operation of [`super::backend`]'s areas, as GitLab's REST API
//! (v4, checked against the GitLab 19.4 documentation) and its GraphQL API spell it. Each
//! request is built once, as a [`Call`], and sent by whichever transport the backend was built
//! over: `glab api`, or the native transport ([`super::http`]) with charter's own sign-in.

use serde_json::Value;

use super::backend::{
    About, Asker, Caller, Capabilities, Capability, ForgeRef, Issues, NewWorkItem, Owner,
    PushProtection, Reach, Reason, RepoRecord, Repos, Requests, Support, Tier, Unavailable,
    UnknownWhy, Visibility, WorkItems,
};
use super::checks::{self, Checks};
use super::pr::{
    AutoMerge, GITLAB_ACTIVE, GITLAB_NOT_MERGEABLE, MergeAs, MergedAt, Opened, Pr, Ready, Request,
    State, check_gate, commit_named, is_ours, not_queued_when, own_mr, pr_of, unknown_state,
};
use super::transport::{Call, Field, Method, Reply};
use super::{
    Failure, ForgeError, Kind, LIST_TIMEOUT, Raised, falsy, first_field, listed_branch,
    listed_description, listed_id, listed_str, listed_topics, mapped, quote, truthy, word_of,
};

mod read;
// `read` calls the boards listing; the rest of the work items wait for their first caller
// (#1202), and until then only their tests reach them, so the compiler would call them dead.
#[allow(dead_code)]
pub(crate) mod work;

/// The GitLab backend.
pub(super) struct GitLab(pub(super) Asker);

/// Whether a merge request record says GitLab will merge it later, by either spelling.
fn set_to_merge_later(record: &Value) -> bool {
    truthy(&record["merge_when_pipeline_succeeds"]) || truthy(&record["auto_merge_enabled"])
}

/// The merge trains API's refusals read as "this request does not join the train now": 400,
/// GitLab would not add it, and 404, a GitLab older than the API. `doc/api/merge_trains.md`
/// documents only the 201 and 202 answers, so these two are purlis's reading, to be confirmed
/// by the live nightly (FG-4). Any other refusal, a 401, 403 or 409, is an error.
const GITLAB_TRAIN_REFUSED: [&str; 2] = ["(HTTP 400)", "(HTTP 404)"];

/// GitLab's pipeline `status` → charter's neutral vocabulary. Python's `gitlab._CI_MAP`, plus
/// the two statuses GitLab added after it (GitLab 19.4 `doc/api/pipelines.md`, the `status`
/// filter): `waiting_for_callback` waits as `pending` does, and `canceling` is on its way to
/// `canceled`.
const CI: [(&str, &str); 13] = [
    ("success", "success"),
    ("failed", "failed"),
    ("running", "running"),
    ("canceled", "canceled"),
    ("canceling", "canceled"),
    ("skipped", "skipped"),
    ("manual", "manual"),
    ("pending", "pending"),
    ("created", "pending"),
    ("preparing", "pending"),
    ("waiting_for_resource", "pending"),
    ("waiting_for_callback", "pending"),
    ("scheduled", "pending"),
];

impl GitLab {
    /// Every page of a GitLab listing of `whose` repos, `path_of` naming each page's path. A
    /// first page `missing` recognises is `None`: there is no such owner to list.
    fn paged(
        &self,
        caller: &Caller,
        whose: &str,
        missing: impl Fn(&Reply) -> bool,
        path_of: impl Fn(usize) -> String,
    ) -> Result<Option<Vec<Value>>, ForgeError> {
        let mut out = Vec::new();
        let mut page = 1;
        loop {
            let path = path_of(page);
            let found = self
                .0
                .found(caller, &path, "GitLab API call", |answer| {
                    page == 1 && missing(answer)
                })
                .map_err(|e| {
                    let said = format!("listing repos for {whose} failed: {e}");
                    e.reworded(said)
                })?;
            let Some(batch) = found else {
                return Ok(None);
            };
            let items = batch.as_array().cloned().unwrap_or_default();
            if items.is_empty() {
                break;
            }
            let short = items.len() < 100;
            out.extend(items);
            if short {
                break;
            }
            page += 1;
        }
        Ok(Some(out))
    }
}

/// Whether GitLab answered that no group has the name asked: a `404`, by its status where the
/// transport saw one, else by `glab`'s words or GitLab's own message.
fn no_such_group(answer: &Reply) -> bool {
    let both = answer.both();
    answer.failure() == Failure::NotFound
        || both.contains("HTTP 404")
        || both.contains("404 Group Not Found")
}

/// One GitLab repo as a neutral record (Python's `_normalize`). Its name is its `path`, the
/// name in its URL, else its display `name`.
fn normalize(raw: &Value) -> RepoRecord {
    let name = Some(listed_str(raw, "path"))
        .filter(|p| !p.is_empty())
        .unwrap_or_else(|| listed_str(raw, "name"));
    RepoRecord {
        id: listed_id(raw),
        name,
        path_with_namespace: listed_str(raw, "path_with_namespace"),
        default_branch: listed_branch(raw),
        description: listed_description(raw),
        web_url: listed_str(raw, "web_url"),
        ssh_url: listed_str(raw, "ssh_url_to_repo"),
        topics: listed_topics(raw),
        forge: Kind::GitLab,
    }
}

/// The [`State`] a GitLab merge request record says. A merged MR names its squash commit, else
/// its merge commit; a fast-forward merge names neither.
fn state_of(record: &Value, doing: &str) -> Result<State, ForgeError> {
    match record["state"].as_str().unwrap_or("") {
        "opened" | "locked" => Ok(State::Open),
        "closed" => Ok(State::Closed),
        "merged" => Ok(State::Merged {
            commit: commit_named(record, "squash_commit_sha")
                .or_else(|| commit_named(record, "merge_commit_sha")),
        }),
        _ => Err(unknown_state(record, doing)),
    }
}

impl Repos for GitLab {
    fn owned(&self, caller: &Caller, owner: &Owner) -> Result<Vec<RepoRecord>, ForgeError> {
        let owner = owner.as_str();
        let enc = quote(owner);
        let group = format!("GitLab group '{owner}'");
        // `with_shared=false`: a project another namespace shared into the group is not the
        // group's, and GitLab lists it unless asked not to (`doc/api/groups.md`).
        let listed = self.paged(caller, &group, no_such_group, |page| {
            format!(
                "groups/{enc}/projects?per_page=100&page={page}&include_subgroups=true\
                 &archived=false&with_shared=false"
            )
        })?;
        let raw = match listed {
            Some(raw) => raw,
            // A user namespace is no group, and its own listing has the same record shape, as
            // a GitHub personal account's has. Only a not-found answer falls back.
            None => {
                let user = format!("GitLab user '{owner}' (no group has that name)");
                self.paged(
                    caller,
                    &user,
                    |_| false,
                    |page| format!("users/{enc}/projects?per_page=100&page={page}&archived=false"),
                )?
                .unwrap_or_default()
            }
        };
        Ok(raw.iter().map(normalize).collect())
    }

    fn reachable(&self, caller: &Caller, owner: &Owner) -> Result<Vec<RepoRecord>, ForgeError> {
        let group = format!("GitLab group '{}'", owner.as_str());
        let raw = self
            .paged(
                caller,
                &group,
                |_| false,
                |page| format!("projects?membership=true&archived=false&per_page=100&page={page}"),
            )?
            .unwrap_or_default();
        Ok(raw
            .iter()
            .map(normalize)
            .filter(|r| owner.holds(r))
            .collect())
    }

    fn top_level(
        &self,
        caller: &Caller,
        repo: &RepoRecord,
        git_ref: Option<&str>,
    ) -> Result<Vec<String>, ForgeError> {
        let Some(ForgeRef(id)) = &repo.id else {
            return Err(ForgeError::new(format!(
                "GitLab gave no id for {}, so its tree cannot be listed",
                repo.path_with_namespace
            )));
        };
        let rid = quote(id);
        let ref_q = git_ref
            .filter(|r| !r.is_empty())
            .map(|r| format!("&ref={}", quote(r)))
            .unwrap_or_default();
        let mut out = Vec::new();
        let mut page = 1;
        loop {
            let path = format!("projects/{rid}/repository/tree?per_page=100&page={page}{ref_q}");
            let batch = self.0.strict(caller, &path, "GitLab API call")?;
            let items = batch.as_array().cloned().unwrap_or_default();
            if items.is_empty() {
                break;
            }
            let short = items.len() < 100;
            out.extend(items.iter().map(|e| {
                e.get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string()
            }));
            if short {
                break;
            }
            page += 1;
        }
        Ok(out)
    }

    fn about(&self, caller: &Caller, repo: &RepoRecord) -> Result<About, ForgeError> {
        let path = repo.path_with_namespace.as_str();
        let api = format!("projects/{}", quote(path));
        let doing = format!("reading {path}");
        let answer = self.0.ask(caller, &Call::get(&api, LIST_TIMEOUT), &doing)?;
        let visibility = answer["visibility"]
            .as_str()
            .and_then(Visibility::parse)
            .ok_or_else(|| ForgeError::new(format!("{doing}: GitLab named no visibility")))?;
        // `issues_access_level` is `disabled`, `private` (members only) or `enabled`; the older
        // `issues_enabled` says only whether they are on. A member of the repo has an access
        // level in `permissions`, given on the repo itself (`project_access`, GitLab's word) or
        // through its group.
        let member = ["project_access", "group_access"].iter().any(|via| {
            answer["permissions"][via]["access_level"]
                .as_u64()
                .is_some()
        });
        let issues = match answer["issues_access_level"].as_str() {
            _ if answer["archived"].as_bool() == Some(true) => Issues::Archived,
            Some("disabled") => Issues::Off,
            _ if answer["issues_enabled"].as_bool() == Some(false) => Issues::Off,
            Some("private") if !member => Issues::NoRight,
            _ => Issues::Open,
        };
        // Named `pre_receive_secret_detection_enabled` before GitLab 18.0, and in the answer
        // only for an account that may change it.
        let push_protection = PushProtection::of(
            answer["secret_push_protection_enabled"]
                .as_bool()
                .or_else(|| answer["pre_receive_secret_detection_enabled"].as_bool()),
        );
        Ok(About {
            visibility,
            issues,
            push_protection,
        })
    }
}

impl WorkItems for GitLab {
    fn create(
        &self,
        caller: &Caller,
        path: &str,
        new: &NewWorkItem,
    ) -> Result<crate::work::WorkItem, ForgeError> {
        let mut fields = vec![
            Field::text("title", &new.title),
            Field::text("description", &new.body),
        ];
        if let Some(ws) = &new.workspace_label {
            fields.push(Field::text(
                "labels",
                &super::backend::workspace_label(Kind::GitLab, ws),
            ));
        }
        let doing = format!("opening an issue in {path}");
        let call = Call::write(
            Method::Post,
            format!("projects/{}/issues", quote(path)),
            fields,
        );
        let issue = self.0.ask(caller, &call, &doing)?;
        item_of(&issue, Some(&new.title), &doing)
    }

    fn read(
        &self,
        caller: &Caller,
        path: &str,
        number: u64,
    ) -> Result<crate::work::WorkItem, ForgeError> {
        self.read_item(caller, path, number)
    }
}

/// GitLab's answer for an issue, in the neutral model: its key, ids, title, state, labels and
/// milestone. A title it leaves out is `sent`, the one charter sent; with none sent, an answer
/// with no title is malformed, as GitHub's is.
fn item_of(
    issue: &serde_json::Value,
    sent: Option<&str>,
    doing: &str,
) -> Result<crate::work::WorkItem, ForgeError> {
    let url = issue["web_url"].as_str().unwrap_or_default().to_string();
    let iid = issue["iid"].as_u64();
    // The path as GitLab spells it, from its own full reference: `group/sub/repo#12`.
    let full = issue["references"]["full"].as_str().unwrap_or_default();
    let place = full.rsplit_once('#').map(|(place, _)| place);
    let host = crate::work::key::page_of(&url).map(|(host, _)| host);
    let (Some(iid), Some(place), Some(host)) = (iid, place, host) else {
        return Err(ForgeError::new(format!(
            "{doing}: GitLab's answer names no page and reference purlis can key the issue by"
        )));
    };
    let key = crate::work::TrackerKey::gitlab_issue(&host, place, iid)
        .map_err(|why| ForgeError::new(format!("{doing}: {why}")))?;
    // GitHub's answer without a title is refused as malformed. GitLab's is read field by field:
    // a title it leaves out of a create's answer is the one charter sent, and a read's is refused.
    let Some(title) = issue["title"].as_str().or(sent) else {
        return Err(ForgeError::new(format!(
            "{doing}: GitLab's answer names no title"
        )));
    };
    let mut item = crate::work::WorkItem::new(key, crate::work::Kind::Issue, title);
    item.forge_ref = issue["id"].as_u64().map(|id| ForgeRef(id.to_string()));
    item.url = url;
    item.state = crate::work::State::of_forge(issue["state"].as_str().unwrap_or_default());
    item.labels = issue["labels"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|l| l.as_str().map(str::to_string))
        .collect();
    let milestone = &issue["milestone"];
    item.milestone = milestone["title"].as_str().map(|title| {
        crate::work::Milestone::of_forge(
            milestone["id"].as_u64().map(|id| ForgeRef(id.to_string())),
            title,
            milestone["due_date"].as_str(),
        )
    });
    Ok(item)
}

impl Requests for GitLab {
    fn open_or_update(
        &self,
        caller: &Caller,
        path: &str,
        head: &str,
        base: &str,
        title: &str,
        body: &str,
    ) -> Result<Opened, ForgeError> {
        let mrs = format!("projects/{}/merge_requests", quote(path));
        let lookup = format!(
            "{mrs}?state=opened&source_branch={}&target_branch={}&per_page=100",
            quote(head),
            quote(base)
        );
        let doing = format!("looking for an open merge request from {head} into {base}");
        let found = self
            .0
            .ask(caller, &Call::get(&lookup, LIST_TIMEOUT), &doing)?;
        let call = match own_mr(&found) {
            Some(open) => {
                let mr = pr_of(open, "iid", "web_url", &doing)?;
                if !is_ours(head, open.get("description").and_then(Value::as_str)) {
                    return Ok(Opened {
                        pr: mr,
                        ours: false,
                    });
                }
                Call::write(
                    Method::Put,
                    format!("{mrs}/{}", mr.number),
                    vec![
                        Field::text("title", title),
                        Field::text("description", body),
                    ],
                )
            }
            None => Call::write(
                Method::Post,
                mrs,
                vec![
                    Field::text("source_branch", head),
                    Field::text("target_branch", base),
                    Field::text("title", title),
                    Field::text("description", body),
                ],
            ),
        };
        let doing = format!("opening a merge request from {head} into {base}");
        let pr = pr_of(
            &self.0.ask(caller, &call, &doing)?,
            "iid",
            "web_url",
            &doing,
        )?;
        Ok(Opened { pr, ours: true })
    }

    fn state(&self, caller: &Caller, path: &str, pr: &Pr) -> Result<State, ForgeError> {
        let api = format!("projects/{}/merge_requests/{}", quote(path), pr.number);
        let doing = format!("reading merge request !{} of {path}", pr.number);
        let record = self.0.ask(caller, &Call::get(&api, LIST_TIMEOUT), &doing)?;
        state_of(&record, &doing)
    }

    fn body(&self, caller: &Caller, path: &str, pr: &Pr) -> Result<String, ForgeError> {
        let api = format!("projects/{}/merge_requests/{}", quote(path), pr.number);
        let doing = format!(
            "reading merge request !{}'s description in {path}",
            pr.number
        );
        let record = self.0.ask(caller, &Call::get(&api, LIST_TIMEOUT), &doing)?;
        // GitLab answers `"description": null` for a merge request opened with none.
        Ok(record["description"]
            .as_str()
            .unwrap_or_default()
            .to_string())
    }

    fn set_body(&self, caller: &Caller, path: &str, pr: &Pr, body: &str) -> Result<(), ForgeError> {
        let api = format!("projects/{}/merge_requests/{}", quote(path), pr.number);
        let doing = format!(
            "writing merge request !{}'s description in {path}",
            pr.number
        );
        let call = Call::write(Method::Put, api, vec![Field::text("description", body)]);
        self.0.ask(caller, &call, &doing).map(|_| ())
    }

    fn by_head(
        &self,
        caller: &Caller,
        path: &str,
        branch: &str,
    ) -> Result<Option<Request>, ForgeError> {
        let api = format!(
            "projects/{}/merge_requests?source_branch={}&state=all&per_page=100",
            quote(path),
            quote(branch)
        );
        let doing = format!("finding the merge request from {branch} in {path}");
        let listing = self.0.ask(caller, &Call::get(&api, LIST_TIMEOUT), &doing)?;
        // GitLab matches `source_branch` by name across forks too, so only an MR from the
        // project itself is this member's.
        let found = own_mr(&listing);
        // A full page with none of the project's own is a page charter could not see past,
        // which is not "no merge request".
        let full = listing.as_array().is_some_and(|all| all.len() >= 100);
        if found.is_none() && full {
            return Err(ForgeError::new(format!(
                "{doing}: a hundred merge requests from forks share this branch name, and \
                 purlis reads no further"
            )));
        }
        let Some(record) = found else {
            return Ok(None);
        };
        let pr = pr_of(record, "iid", "web_url", &doing)?;
        let head = record["sha"]
            .as_str()
            .filter(|sha| !sha.is_empty())
            .ok_or_else(|| format!("{doing}: the forge named no head commit"))?;
        Ok(Some(Request {
            number: pr.number,
            url: pr.url,
            state: state_of(record, &doing)?,
            head: head.to_string(),
        }))
    }

    /// Merge when the pipeline succeeds, with `sha`. GitLab merges *at once* when told that
    /// while no pipeline is running, so the MR's pipeline is read first and a finished or
    /// missing one is `NotQueued`. A GitLab project has one merge method, so the only choice
    /// is squash, asked for only when the project always squashes.
    ///
    /// `auto_merge` where the instance says it is GitLab 17.11 or later, which brought it in
    /// (#805); `merge_when_pipeline_succeeds`, deprecated in 17.11 and still accepted in 19.4
    /// (`doc/api/merge_requests.md`, "Merge a merge request"), everywhere else, an instance that
    /// did not say included: a GitLab older than `auto_merge` ignores a parameter it does not
    /// know, so sending that one would merge at once rather than wait.
    ///
    /// **Merge trains** (#805). Since 19.1, `auto_merge` on a repo with merge trains adds the
    /// merge request to its train (`doc/api/merge_requests.md`, "Merge a merge request": the
    /// `fix_merge_api_train_bypass` change), and the answer is
    /// [`AutoMerge::JoinedTrain`]. Before 19.1 the same call merged past the train, so on a repo
    /// whose `merge_trains_enabled` is true and an instance that does not say it is 19.1 or
    /// later, the request is added to the train through the merge trains API instead
    /// ([`Self::join_train`]), and never through the merge call. A repo whose settings do not
    /// name `merge_trains_enabled` is asked as before.
    fn request_auto_merge(
        &self,
        caller: &Caller,
        path: &str,
        pr: &Pr,
        head_sha: &str,
    ) -> Result<AutoMerge, ForgeError> {
        let project = format!("projects/{}", quote(path));
        let doing = format!("reading how {path} merges");
        let settings = self
            .0
            .ask(caller, &Call::get(&project, LIST_TIMEOUT), &doing)?;
        let train = settings["merge_trains_enabled"].as_bool() == Some(true);
        let squash = if settings["squash_option"] == "always" {
            "true"
        } else {
            "false"
        };
        let mr = format!("{project}/merge_requests/{}", pr.number);
        let doing = format!("reading !{} of {path}", pr.number);
        let record = self.0.ask(caller, &Call::get(&mr, LIST_TIMEOUT), &doing)?;
        let pipeline = record["head_pipeline"]["status"].as_str().unwrap_or("");
        if !GITLAB_ACTIVE.contains(&pipeline) {
            return Ok(AutoMerge::NotQueued(format!(
                "!{} has no pipeline running, so there is nothing for GitLab to wait for",
                pr.number
            )));
        }
        let instance = self.instance(caller);
        let at_least = |major, minor| {
            instance
                .as_ref()
                .is_ok_and(|instance| instance.at_least(major, minor))
        };
        let has_auto_merge = at_least(17, 11);
        if train && !at_least(19, 1) {
            return self.join_train(caller, path, pr, head_sha, squash, has_auto_merge);
        }
        let flag = if has_auto_merge {
            "auto_merge"
        } else {
            "merge_when_pipeline_succeeds"
        };
        let call = Call::write(
            Method::Put,
            format!("{mr}/merge"),
            vec![
                Field::typed(flag, "true"),
                Field::typed("squash", squash),
                Field::text("sha", head_sha),
            ],
        );
        let asked = not_queued_when(
            self.0.said(caller, &call),
            &GITLAB_NOT_MERGEABLE,
            &format!("asking {path} to merge !{} when it passes", pr.number),
        )?;
        Ok(match asked {
            AutoMerge::Queued if train => AutoMerge::JoinedTrain,
            asked => asked,
        })
    }

    /// The repo's squash preference and the merge request's state, head, draft flag and
    /// `detailed_merge_status`; the pipelines at the head, read as `checks_at` reads them; then
    /// the merge call with `sha` and **no** `auto_merge`, by the repo's own merge method, squashed
    /// when the repo squashes by default. Only `mergeable` merges: GitLab's own word that no
    /// approval, discussion, pipeline or other rule the project sets blocks it. A repo with a
    /// merge train is never merged here, only through its train.
    fn merge_when_ready(
        &self,
        caller: &Caller,
        path: &str,
        pr: &Pr,
        head_sha: &str,
    ) -> Result<Ready, ForgeError> {
        let project = format!("projects/{}", quote(path));
        let settings = self.0.ask(
            caller,
            &Call::get(&project, LIST_TIMEOUT),
            &format!("reading how {path} merges"),
        )?;
        let squash = matches!(
            settings["squash_option"].as_str(),
            Some("always" | "default_on")
        );
        // A train repo lands only through its train (`join_train`): a direct merge would skip
        // it. A repo whose settings do not name `merge_trains_enabled` is read as
        // `request_auto_merge` reads it, as one without a train.
        if settings["merge_trains_enabled"].as_bool() == Some(true) {
            return Ok(Ready::Refused(format!(
                "{path} lands through a merge train, and purlis never merges !{} past it",
                pr.number
            )));
        }
        let mr = format!("{project}/merge_requests/{}", pr.number);
        let number = pr.number;
        let record = self.0.ask(
            caller,
            &Call::get(&mr, LIST_TIMEOUT),
            &format!("reading !{number} of {path}"),
        )?;
        match record["state"].as_str() {
            Some("opened") => {}
            Some("merged") => return Ok(Ready::Merged),
            _ => {
                return Ok(Ready::Refused(format!(
                    "!{number} is closed without merging"
                )));
            }
        }
        if truthy(&record["draft"]) {
            return Ok(Ready::Refused(format!("!{number} is a draft")));
        }
        if record["sha"].as_str() != Some(head_sha) {
            return Ok(Ready::Refused(format!(
                "!{number} is no longer at the commit this save pushed, {head_sha}"
            )));
        }
        if let Some(gated) = check_gate(&self.checks_at(caller, path, head_sha, number)) {
            return Ok(gated);
        }
        let status = record["detailed_merge_status"].as_str().unwrap_or("");
        match status {
            "mergeable" => {}
            "checking" | "unchecked" | "preparing" | "approvals_syncing" | "ci_still_running" => {
                return Ok(Ready::Waiting(format!(
                    "GitLab is still working out whether !{number} can merge ({status})"
                )));
            }
            "conflict" => {
                return Ok(Ready::Refused(format!(
                    "!{number} conflicts with its target branch"
                )));
            }
            "need_rebase" => {
                return Ok(Ready::Refused(format!(
                    "!{number} must be rebased onto its target branch first"
                )));
            }
            "not_approved" | "requested_changes" => {
                return Ok(Ready::Refused(format!(
                    "!{number} needs an approval first ({status})"
                )));
            }
            other => {
                return Ok(Ready::Refused(format!(
                    "GitLab says !{number} cannot merge ({})",
                    if other.is_empty() {
                        "no merge status"
                    } else {
                        other
                    }
                )));
            }
        }
        let call = Call::write(
            Method::Put,
            format!("{mr}/merge"),
            vec![
                Field::typed("squash", if squash { "true" } else { "false" }),
                Field::text("sha", head_sha),
            ],
        );
        let doing = format!("merging !{number} of {path}");
        match self.0.answered(caller, &call, &doing)? {
            Ok(answer) if answer["state"] == "merged" => Ok(Ready::Merged),
            // Set to merge later after all: GitLab merges it when its pipeline passes, as the
            // auto-merge this stands in for would have.
            Ok(answer) if set_to_merge_later(&answer) => Ok(Ready::Waiting(format!(
                "GitLab merges !{number} when its pipeline passes"
            ))),
            Ok(answer) => Ok(Ready::Refused(format!(
                "GitLab did not merge !{number} (state {:?})",
                answer["state"].as_str().unwrap_or("unknown")
            ))),
            Err(why) => Ok(Ready::Refused(why)),
        }
    }

    /// The repo's `merge_trains_enabled`. **A repo that does not name it fails closed**: the
    /// field is missing when the token cannot read the repo's settings or the GitLab tier does
    /// not report it, and merging directly where a train may exist would skip the train.
    fn lands_through_queue(
        &self,
        caller: &Caller,
        path: &str,
        _pr: &Pr,
    ) -> Result<bool, ForgeError> {
        let repo = format!("projects/{}", quote(path));
        let doing = format!("reading whether {path} lands through a merge train");
        let settings = self
            .0
            .ask(caller, &Call::get(&repo, LIST_TIMEOUT), &doing)?;
        settings["merge_trains_enabled"].as_bool().ok_or_else(|| {
            ForgeError::new(format!(
                "{doing}: GitLab did not say. Its answer has no merge_trains_enabled, which \
                 GitLab leaves out for a token that cannot read the repo's settings and on a \
                 tier that does not report merge trains. purlis will not merge directly where a \
                 train may exist, so this member is landed by a person"
            ))
        })
    }

    /// The merge call with `sha`, and **no** `merge_when_pipeline_succeeds` or `auto_merge`:
    /// either would have GitLab merge later, at whatever head the branch then has, and a
    /// later push does not undo it. GitLab answers 409 and merges nothing when the head has
    /// moved past `sha`. Its merge method is the repo's own, so the only choice is squash.
    ///
    /// Should GitLab answer that it set the merge request to merge later anyway, charter
    /// cancels that at once and says so: a merge charter did not see happen is one it cannot
    /// vouch for.
    fn merge_at(
        &self,
        caller: &Caller,
        path: &str,
        pr: &Pr,
        head_sha: &str,
        how: &MergeAs,
    ) -> Result<MergedAt, ForgeError> {
        let mr = format!("projects/{}/merge_requests/{}", quote(path), pr.number);
        let message = format!("{}\n\n{}", how.title, how.message);
        let mut fields = vec![
            Field::typed("squash", if how.squash { "true" } else { "false" }),
            Field::text("sha", head_sha),
            Field::text("merge_commit_message", &message),
        ];
        if how.squash {
            fields.push(Field::text("squash_commit_message", &message));
        }
        let doing = format!("merging merge request !{} of {path}", pr.number);
        let answer = self.0.ask(
            caller,
            &Call::write(Method::Put, format!("{mr}/merge"), fields),
            &doing,
        )?;
        if answer["state"] == "merged" {
            return Ok(MergedAt::Now);
        }
        if !set_to_merge_later(&answer) {
            return Err(ForgeError::new(format!(
                "{doing}: GitLab did not confirm a merge (state {:?})",
                answer["state"].as_str().unwrap_or("unknown")
            )));
        }
        let cancel = Call::write(
            Method::Post,
            format!("{mr}/cancel_merge_when_pipeline_succeeds"),
            Vec::new(),
        );
        if let Err(why) = match self.0.said(caller, &cancel) {
            Ok(said) => said,
            Err(no) => Err(no.said().to_string()),
        } {
            return Ok(MergedAt::Later(format!(
                "GitLab set !{} to merge later, and purlis could not cancel that ({why})",
                pr.number
            )));
        }
        // Confirmed by reading it again, not by the cancel's answer alone.
        let read = format!(
            "reading !{} of {path} after cancelling its merge",
            pr.number
        );
        match self.0.ask(caller, &Call::get(&mr, LIST_TIMEOUT), &read) {
            Ok(after) if !set_to_merge_later(&after) => Err(ForgeError::new(format!(
                "{doing}: GitLab set it to merge later instead of merging it, and purlis \
                 cancelled that"
            ))),
            Ok(_) => Ok(MergedAt::Later(format!(
                "GitLab set !{} to merge later, and it is still set after purlis cancelled it",
                pr.number
            ))),
            Err(why) => Ok(MergedAt::Later(format!(
                "GitLab set !{} to merge later, and purlis could not confirm the cancel ({})",
                pr.number,
                why.said()
            ))),
        }
    }

    /// Added to the merge train with `sha` and no `auto_merge`, so GitLab adds it now or
    /// refuses; the train then runs its own pipeline and merges it.
    fn enqueue_at(
        &self,
        caller: &Caller,
        path: &str,
        pr: &Pr,
        head_sha: &str,
        how: &MergeAs,
    ) -> Result<(), ForgeError> {
        let api = format!(
            "projects/{}/merge_trains/merge_requests/{}",
            quote(path),
            pr.number
        );
        let doing = format!(
            "adding merge request !{} of {path} to its merge train at {head_sha}",
            pr.number
        );
        let answer = self.0.ask(
            caller,
            &Call::write(
                Method::Post,
                api,
                vec![
                    Field::typed("squash", if how.squash { "true" } else { "false" }),
                    Field::text("sha", head_sha),
                ],
            ),
            &doing,
        )?;
        let on_it = answer.as_array().is_some_and(|cars| {
            cars.iter()
                .any(|car| car["merge_request"]["iid"].as_u64() == Some(pr.number))
        });
        if !on_it {
            return Err(ForgeError::new(format!(
                "{doing}: GitLab's answer names no car for !{}",
                pr.number
            )));
        }
        Ok(())
    }

    fn checks_at(&self, caller: &Caller, path: &str, sha: &str, request: u64) -> Checks {
        checks::guarded(sha, || {
            let answer = self.0.ask(
                caller,
                &Call::get(
                    format!(
                        "projects/{}/merge_requests/{request}/pipelines?per_page=100",
                        quote(path)
                    ),
                    LIST_TIMEOUT,
                ),
                &format!("reading the pipelines of merge request !{request} of {path}"),
            )?;
            Ok(checks::gitlab(&answer, sha, request)?)
        })
    }

    fn open_on_branch(
        &self,
        caller: &Caller,
        path: &str,
        branch: &str,
    ) -> Result<Option<Value>, Raised> {
        // GitLab matches `source_branch` by name in forks too, so only the project's own merge
        // request is the branch's, as `by_head` reads it. The first page of a hundred is read;
        // past it, the status line shows none rather than a stranger's.
        let asked = format!(
            "projects/{}/merge_requests?state=opened&source_branch={}&per_page=100",
            quote(path),
            quote(branch)
        );
        let Some(answer) = self.0.best_effort(caller, &asked) else {
            return Ok(None);
        };
        if falsy(&answer) {
            return Ok(None);
        }
        if !answer.is_array() {
            return Err(Raised);
        }
        Ok(own_mr(&answer).and_then(|mr| mr.get("iid").cloned()))
    }

    fn ci_word(&self, caller: &Caller, path: &str, branch: &str) -> Result<Option<String>, Raised> {
        let asked = format!(
            "projects/{}/pipelines?ref={}&per_page=1",
            quote(path),
            quote(branch)
        );
        let status = first_field(self.0.best_effort(caller, &asked), "status")?;
        Ok(mapped(&CI, word_of(status.as_ref())))
    }
}

/// What a GitLab instance says of itself to any signed-in account: `GET metadata` (GitLab
/// 15.2; `doc/api/metadata.md`), its version and whether it is the Enterprise Edition.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Instance {
    /// As GitLab spells it: `18.1.2-ee`, `19.5.0-pre`.
    version: String,
    /// Its major and minor release.
    release: (u32, u32),
    /// `Some(true)` for the Enterprise Edition, `Some(false)` for the Community Edition, `None`
    /// when the answer does not say.
    enterprise: Option<bool>,
}

impl Instance {
    /// The instance `answer` describes, or `None` when it names no release purlis can read.
    fn of(answer: &Value) -> Option<Instance> {
        let version = answer["version"].as_str()?;
        let mut numbers = version
            .split(|c: char| !c.is_ascii_digit())
            .map(str::parse::<u32>);
        let release = (numbers.next()?.ok()?, numbers.next()?.ok()?);
        let enterprise = answer["enterprise"]
            .as_bool()
            .or_else(|| version.ends_with("-ee").then_some(true));
        Some(Instance {
            version: version.to_string(),
            release,
            enterprise,
        })
    }

    fn at_least(&self, major: u32, minor: u32) -> bool {
        self.release >= (major, minor)
    }
}

/// Whether a plan or licence holds GitLab's Premium features.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tiered {
    Premium,
    BelowPremium,
}

/// A plan or licence word: Premium and Ultimate hold Premium's features; Free (a namespace's)
/// and Starter (an old licence's) do not. Any other word is not read as either.
fn tiered(plan: &str) -> Option<Tiered> {
    match plan {
        "premium" | "ultimate" => Some(Tiered::Premium),
        "free" | "starter" => Some(Tiered::BelowPremium),
        _ => None,
    }
}

/// The capabilities a GitLab namespace or licence has only on Premium: epics, iterations,
/// blocking links, and a work item's own status (GitLab 19.4 docs, each feature's tier).
fn premium_only(what: Capability) -> bool {
    matches!(
        what,
        Capability::Epics
            | Capability::Iterations
            | Capability::Dependencies
            | Capability::ItemStatus
    )
}

/// The namespace a reach is about: a group (or user) by its full path, or a repo's namespace.
fn namespace_of(at: &Reach) -> Option<&str> {
    match at {
        Reach::Instance => None,
        Reach::Owner(owner) => Some(owner),
        Reach::Repo(path) => path.rsplit_once('/').map(|(namespace, _)| namespace),
    }
}

impl GitLab {
    /// A train repo's request added to its train through the merge trains API
    /// (`doc/api/merge_trains.md`, "Add a merge request to a merge train", since 15.11), set to
    /// join once its pipeline succeeds: `auto_merge` where the instance says it is 17.11 or
    /// later, `when_pipeline_succeeds` (deprecated in 17.11, which every GitLab with this API
    /// knows) anywhere else. This route only ever adds to the train, so no answer merges past
    /// it. Where it does not answer — a GitLab without it (404), or one that will not add this
    /// request (400) — nothing is merged and the request stays open for a person.
    fn join_train(
        &self,
        caller: &Caller,
        path: &str,
        pr: &Pr,
        head_sha: &str,
        squash: &str,
        has_auto_merge: bool,
    ) -> Result<AutoMerge, ForgeError> {
        let flag = if has_auto_merge {
            "auto_merge"
        } else {
            "when_pipeline_succeeds"
        };
        let call = Call::write(
            Method::Post,
            format!(
                "projects/{}/merge_trains/merge_requests/{}",
                quote(path),
                pr.number
            ),
            vec![
                Field::typed(flag, "true"),
                Field::typed("squash", squash),
                Field::text("sha", head_sha),
            ],
        );
        let asked = not_queued_when(
            self.0.said(caller, &call),
            &GITLAB_TRAIN_REFUSED,
            &format!("adding !{} of {path} to its merge train", pr.number),
        )?;
        Ok(match asked {
            AutoMerge::Queued => AutoMerge::JoinedTrain,
            AutoMerge::NotQueued(why) => AutoMerge::NotQueued(format!(
                "{path} lands through a merge train, and GitLab would not add !{} to it: {why}",
                pr.number
            )),
            joined => joined,
        })
    }

    /// FG-2's version probe (#802): what the instance says it is. A refusal, a timeout or an
    /// answer naming no release is [`UnknownWhy::Unanswered`], and a capability that turns on
    /// it stays `Unknown`.
    fn instance(&self, caller: &Caller) -> Result<Instance, UnknownWhy> {
        Instance::of(&self.0.probe(caller, "metadata")?).ok_or(UnknownWhy::Unanswered)
    }

    /// FG-2's licence probe on a self-managed GitLab: `GET license` (`doc/api/license.md`),
    /// which GitLab answers to an administrator only. Anyone else is refused, and the licence
    /// stays unanswered. An expired licence holds no Premium feature.
    fn licence(&self, caller: &Caller) -> Result<Tiered, UnknownWhy> {
        let answer = self.0.probe(caller, "license")?;
        let tier = answer["plan"]
            .as_str()
            .and_then(tiered)
            .ok_or(UnknownWhy::Unanswered)?;
        match answer["expired"].as_bool() {
            Some(false) => Ok(tier),
            Some(true) => Ok(Tiered::BelowPremium),
            None => Err(UnknownWhy::Unanswered),
        }
    }

    /// FG-2's plan probe on gitlab.com: `GET namespaces/:id` (`doc/api/namespaces.md`), whose
    /// `plan` GitLab shows to a namespace's owners. A plan shown to no one is unanswered. A
    /// personal namespace's Premium is not taken for a group's epics and iterations, which
    /// purlis has not asked of it. Free is below Premium whatever the namespace.
    fn plan(&self, caller: &Caller, namespace: &str) -> Result<Tiered, UnknownWhy> {
        let answer = self
            .0
            .probe(caller, &format!("namespaces/{}", quote(namespace)))?;
        let tier = answer["plan"]
            .as_str()
            .and_then(tiered)
            .ok_or(UnknownWhy::Unanswered)?;
        match tier {
            Tiered::Premium if answer["kind"] == "group" => Ok(Tiered::Premium),
            Tiered::Premium => Err(UnknownWhy::NotProbed),
            Tiered::BelowPremium => Ok(Tiered::BelowPremium),
        }
    }

    /// A self-managed instance's answer for a Premium capability: its version (a work item's
    /// status needs 18.1), its edition, then its licence, each asked only when the one before
    /// has not settled it.
    fn self_managed(&self, caller: &Caller, what: Capability) -> Support {
        let instance = match self.instance(caller) {
            Ok(instance) => instance,
            Err(why) => return Support::Unknown(why),
        };
        if what == Capability::ItemStatus && !instance.at_least(18, 1) {
            return Support::Unavailable(Unavailable::because(
                what,
                Reason::HostTooOld(instance.version),
            ));
        }
        match instance.enterprise {
            Some(false) => return below_premium(what),
            Some(true) => {}
            // Answered, with no edition named.
            None => return Support::Unknown(UnknownWhy::Unanswered),
        }
        match self.licence(caller) {
            Ok(Tiered::Premium) => Support::Available,
            Ok(Tiered::BelowPremium) => below_premium(what),
            Err(why) => Support::Unknown(why),
        }
    }
}

fn below_premium(what: Capability) -> Support {
    Support::Unavailable(Unavailable::because(
        what,
        Reason::NeedsTier(Tier::GitLabPremium),
    ))
}

impl Capabilities for GitLab {
    /// What GitLab is known to have without asking, and what FG-2's probes answer (#802).
    ///
    /// gitlab.com has epics, iterations, boards, child items, blocking links, work item types,
    /// an item's own status and close reasons, said of the instance. Of one group or repo,
    /// epics, iterations, blocking links and work item status (whose category says why a
    /// closed item was closed) need a Premium namespace, which the plan probe asks; a Free
    /// namespace has none of them, though an item marked a duplicate is still read as one, so
    /// its close reasons stay unknown. The rest of a narrower reach turns on its settings,
    /// which are not asked: `Unknown`, which takes its fallback.
    ///
    /// A self-managed GitLab shows its version and edition to any signed-in account, and its
    /// licence to an administrator. Its Premium capabilities, said of the instance, are
    /// answered from those (a work item's status also needs 18.1); everything else there, and
    /// what nobody showed, is `Unknown`.
    fn support(&self, caller: &Caller, at: &Reach, what: Capability) -> Support {
        let dotcom = self.0.forge.host.eq_ignore_ascii_case("gitlab.com");
        match what {
            Capability::Epics
            | Capability::Iterations
            | Capability::Boards
            | Capability::SubIssues
            | Capability::Dependencies
            | Capability::IssueTypes
            | Capability::CloseReasons
            | Capability::ItemStatus
                if dotcom && *at == Reach::Instance =>
            {
                Support::Available
            }
            _ if dotcom && (premium_only(what) || what == Capability::CloseReasons) => {
                let plan = namespace_of(at)
                    .ok_or(UnknownWhy::NotProbed)
                    .and_then(|namespace| self.plan(caller, namespace));
                match plan {
                    Ok(Tiered::Premium) => Support::Available,
                    Ok(Tiered::BelowPremium) if premium_only(what) => below_premium(what),
                    // A Free namespace still reads a duplicate marking: its close reasons are
                    // not denied, and not asked either.
                    Ok(Tiered::BelowPremium) => Support::Unknown(UnknownWhy::NotProbed),
                    Err(why) => Support::Unknown(why),
                }
            }
            _ if !dotcom && *at == Reach::Instance && premium_only(what) => {
                self.self_managed(caller, what)
            }
            _ => Support::Unknown(UnknownWhy::NotProbed),
        }
    }
}

#[cfg(test)]
mod capability_tests {
    use std::sync::Arc;

    use super::*;
    use crate::forge::Forge;
    use crate::forge::backend::{Fixed, Taken};
    use crate::forge::recorded::Recorded;

    fn on(host: &str) -> GitLab {
        let recorded = Recorded::parse(r#"{"source": "none", "exchanges": []}"#).unwrap();
        let mut forge = Forge::default_of(Kind::GitLab);
        forge.host = host.to_string();
        GitLab(Asker {
            forge,
            transports: Arc::new(Fixed(Arc::new(recorded))),
        })
    }

    #[test]
    fn gitlab_com_has_epics_said_of_the_instance_and_unknown_of_a_group_or_repo() {
        let gitlab = on("gitlab.com");
        let me = Caller::window();
        for what in [
            Capability::Epics,
            Capability::Iterations,
            Capability::Boards,
            Capability::IssueTypes,
            Capability::CloseReasons,
            Capability::ItemStatus,
        ] {
            assert_eq!(
                gitlab.support(&me, &Reach::Instance, what),
                Support::Available
            );
            // A question that turns on the namespace's plan asks it, and nothing here
            // answers: unanswered (#802). Every other one asks nothing.
            let why = if premium_only(what) || what == Capability::CloseReasons {
                UnknownWhy::Unanswered
            } else {
                UnknownWhy::NotProbed
            };
            for narrower in [Reach::Owner("acme".into()), Reach::Repo("acme/api".into())] {
                let said = gitlab.support(&me, &narrower, what);
                assert_eq!(said, Support::Unknown(why.clone()), "{what:?} {narrower:?}");
                assert_eq!(said.taken(what), Taken::Fallback(what.fallback()));
            }
        }
    }

    /// A GitLab on `host` answering each `(path, reply)` once, as recorded.
    fn answering(host: &str, exchanges: &[(&str, serde_json::Value)]) -> (GitLab, Arc<Recorded>) {
        let exchanges: Vec<serde_json::Value> = exchanges
            .iter()
            .map(|(path, reply)| {
                serde_json::json!({"call": {"endpoint": {"rest": {"method": null, "path": path}},
                                            "fields": []},
                                   "reply": reply})
            })
            .collect();
        let text = serde_json::json!({
            "source": "GitLab 19.4 REST API docs: metadata.md, license.md, namespaces.md",
            "exchanges": exchanges});
        let recorded = Arc::new(Recorded::parse(&text.to_string()).unwrap());
        let mut forge = Forge::default_of(Kind::GitLab);
        forge.host = host.to_string();
        let gitlab = GitLab(Asker {
            forge,
            transports: Arc::new(Fixed(recorded.clone())),
        });
        (gitlab, recorded)
    }

    fn ok(out: serde_json::Value) -> serde_json::Value {
        serde_json::json!({"code": 0, "out": out.to_string()})
    }

    fn refused(status: u16) -> serde_json::Value {
        let said = format!("{status} Forbidden");
        serde_json::json!({"code": 1, "out": format!("{{\"message\":\"{said}\"}}"),
                           "err": format!("glab: {said} (HTTP {status})"), "status": status})
    }

    fn metadata(version: &str, enterprise: bool) -> serde_json::Value {
        ok(
            serde_json::json!({"version": version, "revision": "c401a659d0c",
                              "kas": {"enabled": false, "externalUrl": null, "version": null},
                              "enterprise": enterprise}),
        )
    }

    fn licence(plan: &str, expired: bool) -> serde_json::Value {
        ok(serde_json::json!({"id": 2, "plan": plan, "expired": expired}))
    }

    const SELF_MANAGED: &str = "git.example.com";

    #[test]
    fn a_self_managed_gitlab_on_18_1_with_a_premium_licence_has_an_items_own_status() {
        let (gitlab, recorded) = answering(
            SELF_MANAGED,
            &[
                ("metadata", metadata("18.1.2-ee", true)),
                ("license", licence("premium", false)),
            ],
        );
        let said = gitlab.support(&Caller::window(), &Reach::Instance, Capability::ItemStatus);
        assert_eq!(said, Support::Available);
        assert!(recorded.unspent().is_empty());
        for what in [
            Capability::Epics,
            Capability::Iterations,
            Capability::Dependencies,
        ] {
            let (gitlab, _) = answering(
                SELF_MANAGED,
                &[
                    ("metadata", metadata("17.0.0-ee", true)),
                    ("license", licence("ultimate", false)),
                ],
            );
            assert_eq!(
                gitlab.support(&Caller::window(), &Reach::Instance, what),
                Support::Available,
                "{what:?}"
            );
        }
    }

    #[test]
    fn a_self_managed_gitlab_older_than_18_1_has_no_item_status_and_its_licence_is_not_asked() {
        let (gitlab, recorded) =
            answering(SELF_MANAGED, &[("metadata", metadata("18.0.3-ee", true))]);
        let said = gitlab.support(&Caller::window(), &Reach::Instance, Capability::ItemStatus);
        assert_eq!(
            said,
            Support::Unavailable(Unavailable::because(
                Capability::ItemStatus,
                Reason::HostTooOld("18.0.3-ee".into())
            ))
        );
        assert_eq!(
            said.taken(Capability::ItemStatus),
            Taken::Fallback(crate::forge::backend::Fallback::BoardStatus)
        );
        assert!(recorded.unspent().is_empty());
    }

    #[test]
    fn a_community_edition_has_no_premium_feature_and_its_licence_is_not_asked() {
        for what in [
            Capability::Epics,
            Capability::Iterations,
            Capability::Dependencies,
            Capability::ItemStatus,
        ] {
            let (gitlab, recorded) =
                answering(SELF_MANAGED, &[("metadata", metadata("19.4.0", false))]);
            assert_eq!(
                gitlab.support(&Caller::window(), &Reach::Instance, what),
                Support::Unavailable(Unavailable::because(
                    what,
                    Reason::NeedsTier(Tier::GitLabPremium)
                )),
                "{what:?}"
            );
            assert!(recorded.unspent().is_empty());
        }
    }

    #[test]
    fn a_licence_below_premium_or_expired_says_so_and_one_shown_to_no_one_stays_unknown() {
        let unavailable = Support::Unavailable(Unavailable::because(
            Capability::Epics,
            Reason::NeedsTier(Tier::GitLabPremium),
        ));
        for (answer, expected) in [
            (licence("starter", false), unavailable.clone()),
            (licence("premium", true), unavailable),
            // Asked, and refused (GitLab shows its licence to an administrator only), or
            // answered in a plan word purlis does not read: unanswered, not unasked (#802).
            (refused(403), Support::Unknown(UnknownWhy::Unanswered)),
            (
                licence("platinum", false),
                Support::Unknown(UnknownWhy::Unanswered),
            ),
        ] {
            let (gitlab, recorded) = answering(
                SELF_MANAGED,
                &[
                    ("metadata", metadata("19.4.0-ee", true)),
                    ("license", answer),
                ],
            );
            assert_eq!(
                gitlab.support(&Caller::window(), &Reach::Instance, Capability::Epics),
                expected
            );
            assert!(recorded.unspent().is_empty());
        }
    }

    #[test]
    fn a_self_managed_gitlab_that_does_not_say_what_it_is_stays_unknown() {
        for answer in [
            refused(404),
            ok(serde_json::json!({"revision": "c401a659d0c"})),
        ] {
            let (gitlab, recorded) = answering(SELF_MANAGED, &[("metadata", answer)]);
            assert_eq!(
                gitlab.support(&Caller::window(), &Reach::Instance, Capability::ItemStatus),
                Support::Unknown(UnknownWhy::Unanswered)
            );
            assert!(recorded.unspent().is_empty());
        }
        // An edition it does not name is not taken for either.
        let (gitlab, _) = answering(
            SELF_MANAGED,
            &[("metadata", ok(serde_json::json!({"version": "19.4.0"})))],
        );
        assert_eq!(
            gitlab.support(&Caller::window(), &Reach::Instance, Capability::Epics),
            Support::Unknown(UnknownWhy::Unanswered)
        );
    }

    fn namespace(kind: &str, plan: Option<&str>) -> serde_json::Value {
        let mut out = serde_json::json!({"id": 9, "name": "Acme", "path": "acme", "kind": kind,
                                         "full_path": "acme"});
        if let Some(plan) = plan {
            out["plan"] = plan.into();
        }
        ok(out)
    }

    #[test]
    fn a_premium_group_on_gitlab_com_has_epics_for_its_repos_and_a_free_one_does_not() {
        for at in [Reach::Repo("acme/api".into()), Reach::Owner("acme".into())] {
            let (gitlab, recorded) = answering(
                "gitlab.com",
                &[("namespaces/acme", namespace("group", Some("premium")))],
            );
            assert_eq!(
                gitlab.support(&Caller::window(), &at, Capability::Epics),
                Support::Available,
                "{at:?}"
            );
            assert!(recorded.unspent().is_empty());
            let (gitlab, _) = answering(
                "gitlab.com",
                &[("namespaces/acme", namespace("group", Some("free")))],
            );
            assert_eq!(
                gitlab.support(&Caller::window(), &at, Capability::ItemStatus),
                Support::Unavailable(Unavailable::because(
                    Capability::ItemStatus,
                    Reason::NeedsTier(Tier::GitLabPremium)
                ))
            );
        }
        // A subgroup's repo asks its group, the path encoded as one segment.
        let (gitlab, recorded) = answering(
            "gitlab.com",
            &[(
                "namespaces/acme%2Fplatform",
                namespace("group", Some("ultimate")),
            )],
        );
        let at = Reach::Repo("acme/platform/api".into());
        assert_eq!(
            gitlab.support(&Caller::window(), &at, Capability::Iterations),
            Support::Available
        );
        assert!(recorded.unspent().is_empty());
    }

    #[test]
    fn a_plan_shown_to_no_one_or_a_personal_namespace_stays_unknown_on_gitlab_com() {
        let at = Reach::Repo("acme/api".into());
        // Asked and not answered: a plan shown to no one, a word purlis does not read, a
        // refusal. A personal namespace's Premium is an answer, but whether it gives a group's
        // epics is a question purlis has not asked.
        for (answer, why) in [
            (namespace("group", None), UnknownWhy::Unanswered),
            (namespace("user", Some("premium")), UnknownWhy::NotProbed),
            (namespace("group", Some("default")), UnknownWhy::Unanswered),
            (refused(404), UnknownWhy::Unanswered),
        ] {
            let (gitlab, recorded) = answering("gitlab.com", &[("namespaces/acme", answer)]);
            assert_eq!(
                gitlab.support(&Caller::window(), &at, Capability::Epics),
                Support::Unknown(why)
            );
            assert!(recorded.unspent().is_empty());
        }
        // A free namespace still reads a duplicate marking, so its close reasons are not denied.
        let (gitlab, _) = answering(
            "gitlab.com",
            &[("namespaces/acme", namespace("group", Some("free")))],
        );
        assert_eq!(
            gitlab.support(&Caller::window(), &at, Capability::CloseReasons),
            Support::Unknown(UnknownWhy::NotProbed)
        );
    }

    #[test]
    fn a_question_no_probe_answers_asks_nothing() {
        let (gitlab, recorded) = answering(
            "gitlab.com",
            &[("namespaces/acme", namespace("group", Some("premium")))],
        );
        let me = Caller::window();
        for what in [
            Capability::Boards,
            Capability::SubIssues,
            Capability::AutoMerge,
        ] {
            let _ = gitlab.support(&me, &Reach::Repo("acme/api".into()), what);
        }
        for what in Capability::ALL {
            let _ = gitlab.support(&me, &Reach::Instance, what);
        }
        assert_eq!(recorded.unspent().len(), 1);
        // A self-managed GitLab is asked about itself only for the instance.
        let (gitlab, recorded) =
            answering(SELF_MANAGED, &[("metadata", metadata("19.4.0-ee", true))]);
        let _ = gitlab.support(&me, &Reach::Repo("acme/api".into()), Capability::Epics);
        let _ = gitlab.support(&me, &Reach::Instance, Capability::Boards);
        assert_eq!(recorded.unspent().len(), 1);
    }

    /// As [`answering`], over a backend that keeps its probe answers for `kept_for`.
    fn keeping(
        host: &str,
        exchanges: &[(&str, serde_json::Value)],
        kept_for: std::time::Duration,
    ) -> (GitLab, Arc<Recorded>) {
        let (GitLab(asker), recorded) = answering(host, exchanges);
        let transports = Arc::new(crate::forge::backend::Kept::over(
            asker.transports,
            kept_for,
        ));
        let gitlab = GitLab(Asker {
            forge: asker.forge,
            transports,
        });
        (gitlab, recorded)
    }

    fn an_account(login: &str) -> crate::forge::backend::Account {
        crate::forge::backend::Account {
            kind: Kind::GitLab,
            host: SELF_MANAGED.into(),
            login: login.into(),
        }
    }

    #[test]
    fn a_probes_answer_is_kept_for_the_caller_that_asked_it() {
        // One answer each: a second question that asked again would find nothing recorded.
        let (gitlab, recorded) = keeping(
            SELF_MANAGED,
            &[
                ("metadata", metadata("19.4.0-ee", true)),
                ("license", licence("premium", false)),
            ],
            crate::forge::backend::PROBE_KEPT_FOR,
        );
        let me = Caller::window();
        for what in [
            Capability::Epics,
            Capability::ItemStatus,
            Capability::Iterations,
        ] {
            assert_eq!(
                gitlab.support(&me, &Reach::Instance, what),
                Support::Available,
                "{what:?}"
            );
        }
        assert!(recorded.unspent().is_empty());
    }

    #[test]
    fn a_kept_answer_is_never_another_accounts_or_a_chats() {
        let answers = [
            ("metadata", metadata("19.4.0-ee", true)),
            ("license", licence("premium", false)),
        ];
        let callers = [
            Caller::window().as_account(an_account("ada")),
            Caller::window().as_account(an_account("grace")),
            Caller::window(),
            Caller::chat("c1"),
            Caller::chat("c2"),
            Caller::mcp("c1"),
        ];
        let every: Vec<_> = callers.iter().flat_map(|_| answers.clone()).collect();
        let (gitlab, recorded) =
            keeping(SELF_MANAGED, &every, crate::forge::backend::PROBE_KEPT_FOR);
        for caller in &callers {
            assert_eq!(
                gitlab.support(caller, &Reach::Instance, Capability::Epics),
                Support::Available,
                "{caller:?}"
            );
        }
        assert!(
            recorded.unspent().is_empty(),
            "each caller asked for itself"
        );
    }

    #[test]
    fn an_expired_answer_or_a_refusal_is_asked_again() {
        let (gitlab, recorded) = keeping(
            SELF_MANAGED,
            &[
                ("metadata", metadata("19.4.0", false)),
                ("metadata", metadata("19.4.0", false)),
            ],
            std::time::Duration::ZERO,
        );
        for _ in 0..2 {
            let _ = gitlab.support(&Caller::window(), &Reach::Instance, Capability::Epics);
        }
        assert!(
            recorded.unspent().is_empty(),
            "an expired answer is asked again"
        );

        let (gitlab, recorded) = keeping(
            SELF_MANAGED,
            &[
                ("metadata", metadata("19.4.0-ee", true)),
                ("license", refused(403)),
                ("license", refused(403)),
            ],
            crate::forge::backend::PROBE_KEPT_FOR,
        );
        for _ in 0..2 {
            assert_eq!(
                gitlab.support(&Caller::window(), &Reach::Instance, Capability::Epics),
                Support::Unknown(UnknownWhy::Unanswered)
            );
        }
        assert!(
            recorded.unspent().is_empty(),
            "the version is kept, the refusal asked again"
        );
    }

    #[test]
    fn a_self_managed_gitlab_that_gives_no_answer_is_unanswered_and_the_rest_unasked() {
        // Nothing is recorded, so the instance probe gets no answer at all: a Premium question
        // was asked and is unanswered; every other one asks nothing and stays unasked.
        let gitlab = on("git.example.com");
        for what in Capability::ALL {
            let why = if premium_only(what) {
                UnknownWhy::Unanswered
            } else {
                UnknownWhy::NotProbed
            };
            assert_eq!(
                gitlab.support(&Caller::window(), &Reach::Instance, what),
                Support::Unknown(why),
                "{what:?}"
            );
        }
    }
}

#[cfg(test)]
mod create_mapping_tests {
    use std::sync::Arc;

    use serde_json::json;

    use crate::forge::Forge;
    use crate::forge::Kind;
    use crate::forge::backend::{Caller, ForgeRef, NewWorkItem};
    use crate::forge::recorded::Recorded;
    use crate::work::Milestone;

    fn created(answer: serde_json::Value) -> crate::work::WorkItem {
        let call = json!({"endpoint": {"rest": {"method": "POST", "path": "projects/acme%2Fapi/issues"}},
                          "fields": [{"text": ["title", "Port the picker"]},
                                     {"text": ["description", "Why."]}]});
        let text = json!({"source": "GitLab REST API v4, issues: New issue",
                          "exchanges": [{"call": call, "reply": {"code": 0, "out": answer.to_string()}}]});
        let recorded = Arc::new(Recorded::parse(&text.to_string()).unwrap());
        let backend = Forge::default_of(Kind::GitLab).backend_over(recorded);
        let new = NewWorkItem {
            title: "Port the picker".into(),
            body: "Why.".into(),
            workspace_label: None,
        };
        backend
            .create(&Caller::command(), "acme/api", &new)
            .unwrap()
    }

    fn issue() -> serde_json::Value {
        json!({"id": 84012, "iid": 12, "project_id": 3, "title": "Port the picker",
               "state": "opened", "labels": [], "web_url": "https://gitlab.com/acme/api/-/issues/12",
               "references": {"full": "acme/api#12"}})
    }

    #[test]
    fn a_milestone_in_gitlabs_answer_is_mapped_with_its_id_and_due_day() {
        let mut answer = issue();
        answer["milestone"] = json!({"id": 501, "iid": 1, "title": "v1", "state": "active",
                                     "due_date": "2026-10-31"});
        assert_eq!(
            created(answer).milestone,
            Some(Milestone {
                forge_ref: Some(ForgeRef("501".into())),
                title: "v1".into(),
                due: chrono::NaiveDate::from_ymd_opt(2026, 10, 31),
            })
        );
    }

    #[test]
    fn no_milestone_or_no_due_date_in_gitlabs_answer_maps_to_none() {
        assert_eq!(created(issue()).milestone, None);
        let mut answer = issue();
        answer["milestone"] = json!({"id": 501, "iid": 1, "title": "v1", "state": "active",
                                     "due_date": null});
        assert_eq!(created(answer).milestone.unwrap().due, None);
    }
}
