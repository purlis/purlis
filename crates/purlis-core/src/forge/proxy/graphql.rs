// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A chat's GraphQL document, read as a syntax tree and decided by what it would do.
//!
//! GraphQL has one URL, so the path says nothing about the repo: the document does. This reads
//! every operation in it and refuses the whole request if any one of them is not allowed:
//!
//! - **a subscription** is refused;
//! - **a mutation** must be on [`mutation_allowed`], and **every id in its input must be tied to
//!   an allowed repo**: a `repositoryId` the chat's repo is known by, a `projectPath` that is its
//!   path, or an object id the [`ObjectRepo`] places in it. A mutation naming nothing it can be
//!   tied to is refused. A mutation not on the list is [`Refusal::GraphqlUnknownMutation`], unless
//!   its name says what it is (a merge, a ruleset, a release…), when the refusal says that;
//! - **a query** may start only at the fields in [`root_allowed`], and `repository(owner:, name:)`
//!   (GitHub) or `project(fullPath:)` (GitLab) must name an allowed repo. Fields that lead out of
//!   the repo or into its administration ([`DENIED_FIELDS`]) are refused wherever they appear,
//!   fragments included. A variable's value is read from the request's `variables`; a variable
//!   with no value is refused.
//!
//! A query can still follow a link from an allowed repo to what the person may read, through a
//! field not on [`DENIED_FIELDS`]. That is a read, and the list is the one that someone keeps
//! (ADR 0128 Open item 6). Writes cannot do it: every mutation is tied to an allowed repo.

use graphql_parser::query::{
    Definition, Document, Field, OperationDefinition, Selection, SelectionSet, Value,
    VariableDefinition, parse_query,
};
use serde_json::Value as Json;

use super::commands;
use super::policy::{Class, ObjectRepo, Refusal};
use super::repo::{AllowedRepo, Kind, normalise_repo_path};

/// The largest document read.
const MAX_BODY: usize = 1 << 20;

/// What a value resolved to.
#[derive(Debug, Clone)]
enum Val {
    Str(String),
    List(Vec<Val>),
    Obj(Vec<(String, Val)>),
    Other,
}

fn from_json(json: &Json) -> Val {
    match json {
        Json::String(s) => Val::Str(s.clone()),
        Json::Array(items) => Val::List(items.iter().map(from_json).collect()),
        Json::Object(map) => Val::Obj(map.iter().map(|(k, v)| (k.clone(), from_json(v))).collect()),
        _ => Val::Other,
    }
}

/// The values a document's variables have: the request's, then each definition's default.
struct Vars<'a> {
    given: &'a serde_json::Map<String, Json>,
    defaults: Vec<(String, Value<'a, String>)>,
}

impl Vars<'_> {
    fn resolve(&self, value: &Value<'_, String>) -> Result<Val, ()> {
        Ok(match value {
            Value::Variable(name) => match self.given.get(name) {
                Some(json) => from_json(json),
                None => {
                    let default = self.defaults.iter().find(|(n, _)| n == name).ok_or(())?;
                    self.resolve(&default.1)?
                }
            },
            Value::String(s) => Val::Str(s.clone()),
            Value::List(items) => Val::List(
                items
                    .iter()
                    .map(|v| self.resolve(v))
                    .collect::<Result<_, _>>()?,
            ),
            Value::Object(map) => Val::Obj(
                map.iter()
                    .map(|(k, v)| Ok((k.clone(), self.resolve(v)?)))
                    .collect::<Result<_, ()>>()?,
            ),
            _ => Val::Other,
        })
    }
}

/// The mutations a chat may make, per forge. Everything else is refused.
fn mutation_allowed(kind: Kind, name: &str) -> bool {
    const GITHUB: &[&str] = &[
        "createPullRequest",
        "updatePullRequest",
        "closePullRequest",
        "reopenPullRequest",
        "markPullRequestReadyForReview",
        "convertPullRequestToDraft",
        "updatePullRequestBranch",
        "requestReviews",
        "addComment",
        "updateIssueComment",
        "addPullRequestReviewComment",
        "addPullRequestReviewThread",
        "addPullRequestReviewThreadReply",
        "resolveReviewThread",
        "unresolveReviewThread",
        "createIssue",
        "updateIssue",
        "closeIssue",
        "reopenIssue",
        "addLabelsToLabelable",
        "removeLabelsFromLabelable",
    ];
    const GITLAB: &[&str] = &[
        "mergeRequestCreate",
        "mergeRequestUpdate",
        "mergeRequestSetDraft",
        "mergeRequestSetLabels",
        "mergeRequestSetAssignees",
        "createNote",
        "updateNote",
        "createDiffNote",
        "createDiscussion",
        "discussionToggleResolve",
        "createIssue",
        "updateIssue",
        "issueSetLabels",
    ];
    match kind {
        Kind::GitHub => GITHUB.contains(&name),
        Kind::GitLab => GITLAB.contains(&name),
    }
}

/// What a mutation not on the list is, by its name, so the refusal says so.
fn named_mutation_refusal(name: &str) -> Refusal {
    let n = name.to_ascii_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| n.contains(w));
    if has(&["automerge", "auto_merge"])
        || n == "mergepullrequest"
        || n == "mergebranch"
        || n.starts_with("mergerequestaccept")
    {
        Refusal::Merge
    } else if has(&["approve", "approval"])
        || n == "addpullrequestreview"
        || n == "submitpullrequestreview"
    {
        Refusal::Approval
    } else if has(&[
        "ruleset",
        "protectionrule",
        "protectedbranch",
        "branchprotection",
    ]) {
        Refusal::Ruleset
    } else if has(&["release", "tag"]) && !has(&["stage"]) {
        Refusal::ReleaseOrTag
    } else if has(&[
        "repository",
        "webhook",
        "deploykey",
        "secret",
        "collaborator",
        "invitation",
        "team",
        "environment",
        "variable",
        "accesstoken",
        "pushrule",
        "member",
    ]) {
        Refusal::Admin
    } else {
        Refusal::GraphqlUnknownMutation
    }
}

/// The fields a query may start at.
fn root_allowed(kind: Kind, name: &str) -> bool {
    match kind {
        Kind::GitHub => matches!(name, "repository" | "viewer" | "rateLimit" | "__typename"),
        Kind::GitLab => matches!(name, "project" | "currentUser" | "metadata" | "__typename"),
    }
}

/// What `viewer` and `currentUser` may be asked for: who the person is, and nothing they own.
const WHO: &[&str] = &[
    "login",
    "username",
    "name",
    "id",
    "databaseId",
    "url",
    "webUrl",
    "__typename",
];

/// Fields that lead out of the repo or into its administration, refused wherever they appear.
const DENIED_FIELDS: &[&str] = &[
    "node",
    "nodes",
    "search",
    "organization",
    "user",
    "users",
    "enterprise",
    "topic",
    "resource",
    "parent",
    "forks",
    "templateRepository",
    "repositories",
    "pinnedRepositories",
    "projects",
    "group",
    "groups",
    "namespace",
    "collaborators",
    "invitations",
    "branchProtectionRules",
    "rulesets",
    "deployKeys",
    "webhooks",
    "secrets",
    "variables",
    "ciVariables",
    "members",
    "projectMembers",
    "pushRules",
    "runners",
    "mirrorUrl",
    "headRepository",
    "headRepositoryOwner",
    "ruleset",
    "environment",
    "environments",
    "branchProtectionRule",
    "sourceProject",
    "targetProject",
    "securityVulnerabilities",
    "vulnerabilityAlerts",
    "autoMergeRequest",
    "mergeQueue",
    "mergeQueueEntries",
];

/// What a GraphQL document was decided to be, and the chat's repos it touched.
pub(crate) struct Decided<'a> {
    pub class: Class,
    /// Every repo of the chat the document names (a `repository(...)`, a `repositoryId`, a path, an
    /// object id tied to one), each once. Empty for a document that touches none (`viewer`). The
    /// credential is requested for each of these, never for a repo the document does not name.
    pub touched: Vec<&'a AllowedRepo>,
}

/// Decide a GraphQL request body. `repos` are the chat's repos that this host serves.
pub(crate) fn decide<'a>(
    kind: Kind,
    repos: &[&'a AllowedRepo],
    body: &[u8],
    objects: &dyn ObjectRepo,
) -> Result<Decided<'a>, Refusal> {
    if body.len() > MAX_BODY {
        return Err(Refusal::BodyUnreadable);
    }
    let json: Json = serde_json::from_slice(body).map_err(|_| Refusal::GraphqlParse)?;
    let Json::Object(object) = &json else {
        return Err(Refusal::GraphqlParse);
    };
    let Some(Json::String(query)) = object.get("query") else {
        return Err(Refusal::GraphqlParse);
    };
    let empty = serde_json::Map::new();
    let given = match object.get("variables") {
        None | Some(Json::Null) => &empty,
        Some(Json::Object(map)) => map,
        Some(_) => return Err(Refusal::GraphqlParse),
    };
    let document: Document<'_, String> = parse_query(query).map_err(|_| Refusal::GraphqlParse)?;

    let ctx = Context {
        kind,
        repos,
        objects,
        touched: std::cell::RefCell::new(Vec::new()),
    };
    let mut class = Class::GraphqlQuery;
    for definition in &document.definitions {
        match definition {
            Definition::Fragment(fragment) => {
                let vars = Vars {
                    given,
                    defaults: Vec::new(),
                };
                ctx.check_fields(&fragment.selection_set, &vars)?;
            }
            Definition::Operation(operation) => {
                let (defs, selection_set, is_mutation) = match operation {
                    OperationDefinition::SelectionSet(set) => (&[][..], set, false),
                    OperationDefinition::Query(q) => {
                        (q.variable_definitions.as_slice(), &q.selection_set, false)
                    }
                    OperationDefinition::Mutation(m) => {
                        (m.variable_definitions.as_slice(), &m.selection_set, true)
                    }
                    OperationDefinition::Subscription(_) => return Err(Refusal::NotOnList),
                };
                let vars = Vars {
                    given,
                    defaults: defaults(defs),
                };
                if is_mutation {
                    class = ctx.check_mutation(selection_set, &vars)?;
                } else {
                    ctx.check_query(selection_set, &vars)?;
                }
            }
        }
    }
    let touched = ctx
        .touched
        .into_inner()
        .into_iter()
        .map(|at| repos[at])
        .collect();
    Ok(Decided { class, touched })
}

fn defaults<'a>(defs: &[VariableDefinition<'a, String>]) -> Vec<(String, Value<'a, String>)> {
    defs.iter()
        .filter_map(|d| d.default_value.clone().map(|v| (d.name.clone(), v)))
        .collect()
}

struct Context<'a> {
    kind: Kind,
    repos: &'a [&'a AllowedRepo],
    objects: &'a dyn ObjectRepo,
    /// Indexes into `repos` of each repo the document named, once each, in the order met.
    touched: std::cell::RefCell<Vec<usize>>,
}

impl Context<'_> {
    /// Refuse a denied field anywhere in a selection set. A `repository(owner:, name:)` or `project(fullPath:)` met below the root names an allowed
    /// repo too.
    fn check_fields(&self, set: &SelectionSet<'_, String>, vars: &Vars<'_>) -> Result<(), Refusal> {
        for selection in &set.items {
            match selection {
                Selection::Field(field) => {
                    if DENIED_FIELDS
                        .iter()
                        .any(|d| field.name.eq_ignore_ascii_case(d))
                    {
                        return Err(Refusal::GraphqlOtherRepo);
                    }
                    if !field.arguments.is_empty() {
                        match (self.kind, field.name.as_str()) {
                            (Kind::GitHub, "repository") => {
                                self.check_named(field, vars, "owner", Some("name"))?
                            }
                            (Kind::GitLab, "project") => {
                                self.check_named(field, vars, "fullPath", None)?
                            }
                            _ => {}
                        }
                    }
                    self.check_fields(&field.selection_set, vars)?;
                }
                Selection::InlineFragment(fragment) => {
                    self.check_fields(&fragment.selection_set, vars)?
                }
                Selection::FragmentSpread(_) => {}
            }
        }
        Ok(())
    }

    fn check_query(&self, set: &SelectionSet<'_, String>, vars: &Vars<'_>) -> Result<(), Refusal> {
        for selection in &set.items {
            // A root made of fragments is not read through: refuse it.
            let Selection::Field(field) = selection else {
                return Err(Refusal::GraphqlOtherRepo);
            };
            if !root_allowed(self.kind, &field.name) {
                return Err(Refusal::GraphqlOtherRepo);
            }
            match field.name.as_str() {
                "viewer" | "currentUser" => self.check_who(&field.selection_set)?,
                "repository" => self.check_named(field, vars, "owner", Some("name"))?,
                "project" => self.check_named(field, vars, "fullPath", None)?,
                _ => {}
            }
            self.check_fields(&field.selection_set, vars)?;
        }
        Ok(())
    }

    fn check_who(&self, set: &SelectionSet<'_, String>) -> Result<(), Refusal> {
        for selection in &set.items {
            match selection {
                Selection::Field(f) if WHO.contains(&f.name.as_str()) => {}
                _ => return Err(Refusal::GraphqlOtherRepo),
            }
        }
        Ok(())
    }

    /// `repository(owner:, name:)` or `project(fullPath:)` must name an allowed repo.
    fn check_named(
        &self,
        field: &Field<'_, String>,
        vars: &Vars<'_>,
        first: &str,
        second: Option<&str>,
    ) -> Result<(), Refusal> {
        let arg = |name: &str| {
            field
                .arguments
                .iter()
                .find(|(n, _)| n == name)
                .and_then(|(_, v)| vars.resolve(v).ok())
                .and_then(|v| match v {
                    Val::Str(s) => Some(s),
                    _ => None,
                })
        };
        let path = match second {
            Some(second) => match (arg(first), arg(second)) {
                (Some(owner), Some(name)) => format!("{owner}/{name}"),
                _ => return Err(Refusal::GraphqlOtherRepo),
            },
            None => arg(first).ok_or(Refusal::GraphqlOtherRepo)?,
        };
        self.in_repos(&path)
    }

    fn touch(&self, at: usize) {
        let mut touched = self.touched.borrow_mut();
        if !touched.contains(&at) {
            touched.push(at);
        }
    }

    fn in_repos(&self, path: &str) -> Result<(), Refusal> {
        let path = normalise_repo_path(self.kind, path).map_err(|_| Refusal::GraphqlOtherRepo)?;
        match self.repos.iter().position(|r| r.path == path) {
            Some(at) => {
                self.touch(at);
                Ok(())
            }
            None => Err(Refusal::GraphqlOtherRepo),
        }
    }

    fn check_mutation(
        &self,
        set: &SelectionSet<'_, String>,
        vars: &Vars<'_>,
    ) -> Result<Class, Refusal> {
        let mut class = None;
        for selection in &set.items {
            let Selection::Field(field) = selection else {
                return Err(Refusal::GraphqlUnknownMutation);
            };
            if field.name == "__typename" {
                continue;
            }
            if !mutation_allowed(self.kind, &field.name) {
                return Err(named_mutation_refusal(&field.name));
            }
            let mut ids = Vec::new();
            for (_, value) in &field.arguments {
                let val = vars
                    .resolve(value)
                    .map_err(|()| Refusal::GraphqlOtherRepo)?;
                collect_ids("", &val, &mut ids);
                if strings(&val)
                    .iter()
                    .any(|text| commands::has_merge_command(text))
                {
                    return Err(Refusal::MergeCommand);
                }
            }
            self.check_scoped(&ids)?;
            // What a mutation asks back is read like any query: it may not walk out of the repo.
            self.check_fields(&field.selection_set, vars)?;
            class = Some(Class::GraphqlMutation(field.name.clone()));
        }
        class.ok_or(Refusal::GraphqlUnknownMutation)
    }

    /// Every id the mutation names is tied to an allowed repo, and it names at least one.
    fn check_scoped(&self, ids: &[(String, String)]) -> Result<(), Refusal> {
        if ids.is_empty() {
            return Err(Refusal::GraphqlOtherRepo);
        }
        for (key, value) in ids {
            match key.as_str() {
                "repositoryId" => {
                    let at = self
                        .repos
                        .iter()
                        .position(|r| r.node_ids.iter().any(|id| id == value))
                        .ok_or(Refusal::GraphqlOtherRepo)?;
                    self.touch(at);
                }
                "projectPath" | "fullPath" => self.in_repos(value)?,
                _ => {
                    let at = self
                        .repos
                        .iter()
                        .position(|r| {
                            self.objects
                                .repo_of(r.kind, &r.host, value)
                                .is_some_and(|path| r.path == path)
                        })
                        .ok_or(Refusal::GraphqlOtherRepo)?;
                    self.touch(at);
                }
            }
        }
        Ok(())
    }
}

/// The ids in a mutation's arguments: strings under a key that names an id or a path.
fn collect_ids(key: &str, val: &Val, out: &mut Vec<(String, String)>) {
    match val {
        Val::Str(s) if names_an_id(key) => out.push((key.to_string(), s.clone())),
        Val::List(items) => items.iter().for_each(|v| collect_ids(key, v, out)),
        Val::Obj(fields) => fields.iter().for_each(|(k, v)| collect_ids(k, v, out)),
        _ => {}
    }
}

fn names_an_id(key: &str) -> bool {
    key != "clientMutationId"
        && (key == "id"
            || key.ends_with("Id")
            || key.ends_with("Ids")
            || key == "projectPath"
            || key == "fullPath")
}

/// Every string in a value.
fn strings(val: &Val) -> Vec<String> {
    match val {
        Val::Str(s) => vec![s.clone()],
        Val::List(items) => items.iter().flat_map(strings).collect(),
        Val::Obj(fields) => fields.iter().flat_map(|(_, v)| strings(v)).collect(),
        Val::Other => Vec::new(),
    }
}
