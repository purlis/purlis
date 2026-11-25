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
//!   (GitHub) or `project(fullPath:)` (GitLab) must name an allowed repo. **Every selected field
//!   on every type is checked against that type's table** ([`super::schema`]): a field that is not
//!   on it is refused, however deep, through fragments too (each spread is read where it is used,
//!   as the type it is spread on). `node(id:)` and `nodes(ids:)` are allowed, and only for ids
//!   the proxy can tie to one of the chat's repos, and then only as the types the tables name. A
//!   variable's value is read from the request's `variables`; a variable with no value is
//!   refused.
//!
//! Where the tables stop is where the list is kept (ADR 0128 Open item 6): a field GitHub adds
//! tomorrow is refused until it is read and added. Writes cannot walk out of the repo at all:
//! every mutation is tied to an allowed repo, and what it asks back is read like any query.

use std::collections::HashMap;

use graphql_parser::query::{
    Definition, Document, Field, FragmentDefinition, OperationDefinition, Selection, SelectionSet,
    TypeCondition, Value, VariableDefinition,
};
use serde_json::Value as Json;

use super::commands;
use super::policy::{Class, ObjectRepo, OwnsHead, PullRef, Refusal};
use super::repo::{AllowedRepo, Kind, normalise_repo_path};
use super::rest;
use super::schema::{self, Ty};

/// How deep a selection (fragments included) may go. A document deeper than a person writes by
/// hand, or one whose fragments spread each other, is refused.
const MAX_DEPTH: usize = 16;

/// The largest document read.
const MAX_BODY: usize = 1 << 20;

/// Whether `graphql-parser` 0.4.1 could panic reading `query` (#7589, #7596).
///
/// Its `unquote_block_string` (`common.rs`) takes `str::lines()` of a block string, sets the
/// common indent to the least byte length of leading whitespace (`trim_start`, so
/// `char::is_whitespace`) over every non-blank line after the first, and then cuts every line
/// after the first, blank ones too, at `&line[common_indent..]`. That cut panics when it lands
/// inside a character. If every line's leading whitespace is ASCII, `common_indent` is a count of
/// ASCII bytes that each non-blank line starts with (it is the least of them) and a blank line
/// too short for it is not cut, so the cut is always on a boundary. A panic therefore needs a
/// line whose leading whitespace holds a non-ASCII character. The block string's lines are lines
/// of the query (`lines()` and `split('\n')` cut alike; a lone `\r` is whitespace and a
/// trailing `\r` is dropped by `lines()`, neither of which changes a line's leading whitespace),
/// so this reads every line of the whole query: it also refuses a non-ASCII indent outside any
/// block string, which no chat writes, and the first line of a block string, which the parser
/// never cuts, in exchange for needing no tokenizer of its own.
fn could_panic_the_parser(query: &str) -> bool {
    query.contains("\"\"\"")
        && query.split('\n').any(|line| {
            line.chars()
                .take_while(|c| c.is_whitespace())
                .any(|c| !c.is_ascii())
        })
}

/// The document `query` is, or none when it is not one the parser reads or could panic on. A
/// shipped build aborts on any panic before it unwinds ([`crate::panicend::fatal`]), so the
/// panic is never raised: such a query is refused before the parser sees it. The `catch_unwind`
/// is a second line for debug and test builds only, which unwind.
fn parse(query: &str) -> Option<Document<'_, String>> {
    if could_panic_the_parser(query) {
        return None;
    }
    std::panic::catch_unwind(|| graphql_parser::query::parse_query::<String>(query))
        .ok()?
        .ok()
}

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
    // The mutations that take only label ids (`addLabelsToLabelable`, `mergeRequestSetLabels`,
    // `issueSetLabels`…) are not on the list: the proxy cannot read a label's name from its id.
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
    ];
    const GITLAB: &[&str] = &[
        "mergeRequestCreate",
        "mergeRequestUpdate",
        "mergeRequestSetDraft",
        "mergeRequestSetAssignees",
        "createNote",
        "updateNote",
        "createDiffNote",
        "createDiscussion",
        "discussionToggleResolve",
        "createIssue",
        "updateIssue",
    ];
    match kind {
        Kind::GitHub => GITHUB.contains(&name),
        Kind::GitLab => GITLAB.contains(&name),
    }
}

/// The mutations that merge, which `guards.merge` = `log` or `off` puts on the list: a merge, auto
/// merge on or off, and the merge queue on or off. `mergeBranch` merges with no pull request and
/// stays refused.
fn merge_mutation(kind: Kind, name: &str) -> bool {
    match kind {
        Kind::GitHub => matches!(
            name,
            "mergePullRequest"
                | "enablePullRequestAutoMerge"
                | "disablePullRequestAutoMerge"
                | "enqueuePullRequest"
                | "dequeuePullRequest"
        ),
        Kind::GitLab => name == "mergeRequestAccept",
    }
}

/// **The act a merge mutation asks for**, which the person's press performs (#7543): a merge
/// now or a place in the merge queue. Each is held as the act it names and never as another: the
/// press of an enqueue enqueues and never merges. Auto-merge is not held
/// ([`unheld_merge_said`]), and undoing one (`disablePullRequestAutoMerge`,
/// `dequeuePullRequest`) is never held.
pub(super) fn merge_act(kind: Kind, name: &str) -> Option<crate::mergehold::LandAs> {
    use crate::mergehold::LandAs;
    match (kind, name) {
        (Kind::GitHub, "mergePullRequest") | (Kind::GitLab, "mergeRequestAccept") => {
            Some(LandAs::Merge)
        }
        (Kind::GitHub, "enqueuePullRequest") => Some(LandAs::Enqueue),
        _ => None,
    }
}

/// What a chat is told when it asks for auto-merge, which is refused and never held: it pins the
/// head only when it is turned on, so the forge would merge any head pushed later, unseen.
pub(super) fn unheld_merge_said(kind: Kind, name: &str) -> Option<&'static str> {
    match (kind, name) {
        (Kind::GitHub, "enablePullRequestAutoMerge") => Some(
            "auto-merge would merge any head pushed later, unseen, so purlis does not hold it. \
             Ask the person to merge it, or ask to enqueue it",
        ),
        _ => None,
    }
}

/// What a chat is told when a GitLab accept sets a merge strategy: `strategy` and
/// `autoMergeStrategy` ask GitLab to merge when the pipeline succeeds or through a train, which
/// the person's press does not do ([`Merged::Now`](crate::mergehold::Merged)).
const STRATEGY_SAID: &str = "this merge sets a merge strategy (merge when the pipeline succeeds, \
                             or a merge train), which purlis does not hold: its press merges at \
                             once. Ask the person to merge it, or merge now without a strategy";

/// Whether a GitLab `mergeRequestAccept` document sets `strategy` or `autoMergeStrategy`, to a
/// value or a variable: the document asks for more than a merge now, so it is refused with
/// [`STRATEGY_SAID`] where `body` is one. `None` where it is not one or cannot be read.
pub(super) fn strategy_said(kind: Kind, body: &[u8]) -> Option<&'static str> {
    if kind != Kind::GitLab {
        return None;
    }
    let (name, arguments) = mutation_arguments(body)?;
    (name == "mergeRequestAccept" && has_key(&arguments, &["strategy", "autoMergeStrategy"]))
        .then_some(STRATEGY_SAID)
}

/// Whether `val` holds any of `keys` as a field, at any depth.
fn has_key(val: &Val, keys: &[&str]) -> bool {
    match val {
        Val::Obj(fields) => fields
            .iter()
            .any(|(k, v)| keys.contains(&k.as_str()) || has_key(v, keys)),
        Val::List(items) => items.iter().any(|v| has_key(v, keys)),
        _ => false,
    }
}

/// The first string `key` holds in `val`, at any depth.
fn find_str<'v>(val: &'v Val, key: &str) -> Option<&'v str> {
    match val {
        Val::Obj(fields) => fields.iter().find_map(|(k, v)| match v {
            Val::Str(s) if k == key => Some(s.as_str()),
            _ => find_str(v, key),
        }),
        Val::List(items) => items.iter().find_map(|v| find_str(v, key)),
        _ => None,
    }
}

/// **The one mutation a document is**, as its field's name and its arguments, variables
/// resolved: `None` where the document is not exactly one mutation with one field, or an
/// argument cannot be resolved.
fn mutation_arguments(body: &[u8]) -> Option<(String, Val)> {
    let json: Json = serde_json::from_slice(body).ok()?;
    let Json::Object(object) = &json else {
        return None;
    };
    let Some(Json::String(query)) = object.get("query") else {
        return None;
    };
    let empty = serde_json::Map::new();
    let given = match object.get("variables") {
        None | Some(Json::Null) => &empty,
        Some(Json::Object(map)) => map,
        Some(_) => return None,
    };
    let parsed: Document<'_, String> = parse_query(query).ok()?;
    let mut operations = parsed.definitions.iter().filter_map(|d| match d {
        Definition::Operation(operation) => Some(operation),
        Definition::Fragment(_) => None,
    });
    let Some(OperationDefinition::Mutation(mutation)) = operations.next() else {
        return None;
    };
    if operations.next().is_some() {
        return None;
    }
    let [Selection::Field(field)] = mutation.selection_set.items.as_slice() else {
        return None;
    };
    let vars = Vars {
        given,
        defaults: defaults(&mutation.variable_definitions),
    };
    let mut arguments = Vec::new();
    for (name, value) in &field.arguments {
        arguments.push((name.clone(), vars.resolve(value).ok()?));
    }
    let arguments = Val::Obj(arguments);
    Some((field.name.clone(), arguments))
}

/// **The pull request a merge document is about**: the path of the one recorded repo of `repos`
/// it is in and its number, for a document that is exactly one holdable merge mutation
/// ([`merge_act`]), with that act. On GitHub the node id is resolved by `objects`; on GitLab the
/// project path and `iid` are in the document. `None` where any of it is missing or not a plain
/// number.
/// Never decides whether the request is allowed: [`decide`] does that.
pub(super) fn merge_target(
    kind: Kind,
    repos: &[&AllowedRepo],
    body: &[u8],
    objects: &dyn ObjectRepo,
) -> Option<(String, u64, crate::mergehold::LandAs)> {
    let (name, arguments) = mutation_arguments(body)?;
    let act = merge_act(kind, &name)?;
    match kind {
        Kind::GitHub => {
            let id = find_str(&arguments, "pullRequestId")?;
            repos.iter().filter(|r| r.kind == kind).find_map(|r| {
                objects
                    .pull_of(r.kind, &r.host, id)
                    .filter(|(path, number)| *path == r.path && *number > 0)
                    .map(|(path, number)| (path, number, act))
            })
        }
        Kind::GitLab => {
            let project = find_str(&arguments, "projectPath")?;
            let iid = find_str(&arguments, "iid")?;
            let number = iid
                .parse::<u64>()
                .ok()
                .filter(|n| *n > 0 && iid.bytes().all(|b| b.is_ascii_digit()))?;
            let path = normalise_repo_path(kind, project).ok()?;
            repos
                .iter()
                .find(|r| r.kind == kind && r.path == path)
                .map(|r| (r.path.clone(), number, act))
        }
    }
}

/// What a mutation not on the list is, by its name, so the refusal says so.
fn named_mutation_refusal(name: &str) -> Refusal {
    let n = name.to_ascii_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| n.contains(w));
    if has(&["automerge", "auto_merge"])
        || n == "mergepullrequest"
        || n == "enqueuepullrequest"
        || n == "dequeuepullrequest"
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
        Kind::GitHub => matches!(
            name,
            "repository"
                | "viewer"
                | "rateLimit"
                | "node"
                | "nodes"
                | "search"
                | "__type"
                | "__schema"
                | "__typename"
        ),
        Kind::GitLab => matches!(
            name,
            "project" | "currentUser" | "metadata" | "__type" | "__schema" | "__typename"
        ),
    }
}

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
    owns: OwnsHead<'_>,
    merging: bool,
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
    let document = parse(query).ok_or(Refusal::GraphqlParse)?;

    let mut fragments = HashMap::new();
    for definition in &document.definitions {
        if let Definition::Fragment(fragment) = definition {
            // Two fragments of a name is a document a server refuses and a reader may pick
            // either of: refused here.
            if fragments.insert(fragment.name.as_str(), fragment).is_some() {
                return Err(Refusal::GraphqlParse);
            }
        }
    }
    let ctx = Context {
        kind,
        repos,
        objects,
        owns,
        merging,
        fragments,
        touched: std::cell::RefCell::new(Vec::new()),
    };
    let mut class = Class::GraphqlQuery;
    for definition in &document.definitions {
        match definition {
            // A fragment is read where it is spread, as the type it is spread on.
            Definition::Fragment(_) => {}
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
                    class = ctx
                        .check_mutation(selection_set, &vars)
                        .map_err(|refusal| {
                            // Only these two have a REST route to be told of (`Refusal::advice`).
                            let names_one = selection_set.items.iter().any(|s| {
                                matches!(s, Selection::Field(f) if matches!(
                                    f.name.as_str(),
                                    "createPullRequest" | "updatePullRequestBranch"
                                ))
                            });
                            if refusal == Refusal::GraphqlOtherRepo && names_one {
                                Refusal::GraphqlPullRequestOtherRepo
                            } else {
                                refusal
                            }
                        })?;
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

/// The index in `repos` of the one repo a search query names, or `None` when it is not a search
/// of exactly one of them (GraphQL's `search(query:)` and REST's `GET /search/issues?q=`).
pub(crate) fn search_repo(query: &str, repos: &[&AllowedRepo]) -> Option<usize> {
    // A zero-width space or a control character is a separator or nothing to the forge and a
    // letter of a word to this reading: only printable ASCII and the space are read.
    if !query.bytes().all(|b| b == b' ' || b.is_ascii_graphic()) {
        return None;
    }
    // A quote hides a qualifier from this reading and not from the forge's.
    if query.contains(['"', '\'', '\\', '`']) {
        return None;
    }
    // Without a boolean operator or a group, every other term only narrows the one repo's
    // results; with one, `repo:a OR is:issue` widens them to the whole forge.
    if query.contains(['(', ')']) {
        return None;
    }
    let mut paths = Vec::new();
    for token in query.split_whitespace() {
        if ["or", "and", "not"]
            .iter()
            .any(|op| token.eq_ignore_ascii_case(op))
        {
            return None;
        }
        // `|`, `||`, `&` and `&&` are not the forge's operators as far as is known, but a token
        // of nothing else is refused rather than guessed at.
        if token.chars().all(|c| c == '|' || c == '&') {
            return None;
        }
        let lower = token
            .trim_start_matches(['(', '-', '+', '!'])
            .to_ascii_lowercase();
        if let Some(path) = lower.strip_prefix("repo:") {
            // The qualifier is read only as the forge surely reads it: the token starts with
            // `repo:` itself. A prefix (`+repo:`, `-repo:`, `!repo:`) may make it plain text to
            // the forge, which would leave the search unscoped.
            if !token.to_ascii_lowercase().starts_with("repo:")
                || path.contains(['(', ')', ',', '*'])
            {
                return None;
            }
            paths.push(path.to_string());
        } else if ["org:", "user:", "owner:", "team:", "enterprise:"]
            .iter()
            .any(|q| lower.starts_with(q))
        {
            return None;
        }
    }
    let [path] = paths.as_slice() else {
        return None;
    };
    // The recorded path as it is, case aside: not what `normalise_repo_path` makes of a
    // prefix or a `.git` suffix, which the forge's search does not read as the repo.
    repos.iter().position(|r| r.path.eq_ignore_ascii_case(path))
}

fn defaults<'a>(defs: &[VariableDefinition<'a, String>]) -> Vec<(String, Value<'a, String>)> {
    defs.iter()
        .filter_map(|d| d.default_value.clone().map(|v| (d.name.clone(), v)))
        .collect()
}

struct Context<'a, 'd> {
    kind: Kind,
    repos: &'a [&'a AllowedRepo],
    objects: &'a dyn ObjectRepo,
    /// Whether the chat may write to a pull request's head (update-branch).
    owns: OwnsHead<'a>,
    /// `guards.merge` lets a merge mutation ([`merge_mutation`]) through.
    merging: bool,
    fragments: HashMap<&'d str, &'d FragmentDefinition<'d, String>>,
    /// Indexes into `repos` of each repo the document named, once each, in the order met.
    touched: std::cell::RefCell<Vec<usize>>,
}

impl<'d> Context<'_, 'd> {
    /// Walk a selection set of type `parent`: every field must be on its type's table, and what
    /// it selects is walked as the type it returns.
    fn walk(
        &self,
        parent: &Ty,
        set: &SelectionSet<'d, String>,
        depth: usize,
    ) -> Result<(), Refusal> {
        if depth > MAX_DEPTH {
            return Err(Refusal::GraphqlOtherRepo);
        }
        for selection in &set.items {
            match selection {
                Selection::Field(field) => {
                    let ty = schema::field(self.kind, parent, &field.name)
                        .ok_or(Refusal::GraphqlOtherRepo)?;
                    let selects = !field.selection_set.items.is_empty();
                    match (ty, selects) {
                        (Ty::Leaf, false) => {}
                        (Ty::Leaf, true) | (_, false) => return Err(Refusal::GraphqlOtherRepo),
                        (ty, true) => self.walk(&ty, &field.selection_set, depth + 1)?,
                    }
                }
                Selection::InlineFragment(fragment) => {
                    let ty = match &fragment.type_condition {
                        Some(TypeCondition::On(name)) => schema::narrow(self.kind, parent, name)
                            .ok_or(Refusal::GraphqlOtherRepo)?,
                        None => *parent,
                    };
                    self.walk(&ty, &fragment.selection_set, depth + 1)?;
                }
                Selection::FragmentSpread(spread) => {
                    let fragment = self
                        .fragments
                        .get(spread.fragment_name.as_str())
                        .ok_or(Refusal::GraphqlOtherRepo)?;
                    let TypeCondition::On(name) = &fragment.type_condition;
                    let ty =
                        schema::narrow(self.kind, parent, name).ok_or(Refusal::GraphqlOtherRepo)?;
                    self.walk(&ty, &fragment.selection_set, depth + 1)?;
                }
            }
        }
        Ok(())
    }

    fn check_query(&self, set: &SelectionSet<'d, String>, vars: &Vars<'_>) -> Result<(), Refusal> {
        for selection in &set.items {
            // A root made of fragments is not read through: refuse it.
            let Selection::Field(field) = selection else {
                return Err(Refusal::GraphqlOtherRepo);
            };
            if !root_allowed(self.kind, &field.name) {
                return Err(Refusal::GraphqlOtherRepo);
            }
            let ty = match field.name.as_str() {
                "viewer" => Ty::Obj("User"),
                "currentUser" => Ty::Obj("UserCore"),
                "repository" => {
                    self.check_named(field, vars, "owner", Some("name"))?;
                    Ty::Obj("Repository")
                }
                "project" => {
                    self.check_named(field, vars, "fullPath", None)?;
                    Ty::Obj("Project")
                }
                "rateLimit" => Ty::Obj("RateLimit"),
                "__type" => Ty::Obj("__Type"),
                "__schema" => Ty::Obj("__Schema"),
                "search" => {
                    self.check_search(field, vars)?;
                    Ty::Conn("SearchResultItem")
                }
                "metadata" => Ty::Obj("Metadata"),
                "node" => {
                    self.check_node_ids(field, vars, "id", false)?;
                    Ty::Obj("Node")
                }
                "nodes" => {
                    self.check_node_ids(field, vars, "ids", true)?;
                    Ty::Obj("Node")
                }
                // `__typename` of the root: a leaf.
                _ => {
                    if field.selection_set.items.is_empty() {
                        continue;
                    }
                    return Err(Refusal::GraphqlOtherRepo);
                }
            };
            if field.selection_set.items.is_empty() {
                return Err(Refusal::GraphqlOtherRepo);
            }
            self.walk(&ty, &field.selection_set, 0)?;
        }
        Ok(())
    }

    /// `search(query:, type: ISSUE)`, which `gh pr status` and a filtered `gh pr list` use: only
    /// issues and pull requests, and only of the chat's repos. The query must name **exactly one**
    /// `repo:` and it must be one of the chat's, and it may not name an organisation, a user, an
    /// owner, a team or an enterprise, or negate any of them: search says which repos by those
    /// qualifiers alone, so a query with none of them but the one is a search of that repo. No
    /// boolean operator (`OR`, `AND`, `NOT`, any case) and no parenthesis is read: `gh` sends
    /// neither for `pr status` or a filtered `pr list`, which join their terms by spaces.
    fn check_search(&self, field: &Field<'_, String>, vars: &Vars<'_>) -> Result<(), Refusal> {
        let refuse = Refusal::GraphqlOtherRepo;
        // An argument given twice is read by the server by its own rule, which this reading does
        // not know.
        for name in ["query", "type"] {
            if field.arguments.iter().filter(|(n, _)| n == name).count() > 1 {
                return Err(refuse);
            }
        }
        // `gh issue list --search` sends the type as a variable (`$type: SearchType!`, given
        // `"ISSUE"`), which is read by its value like the query's.
        let is_issue = field.arguments.iter().any(|(n, v)| {
            n == "type"
                && match v {
                    Value::Enum(e) => e == "ISSUE",
                    Value::Variable(_) => {
                        matches!(vars.resolve(v), Ok(Val::Str(s)) if s == "ISSUE")
                    }
                    _ => false,
                }
        });
        if !is_issue {
            return Err(refuse);
        }
        let query = field
            .arguments
            .iter()
            .find(|(n, _)| n == "query")
            .and_then(|(_, v)| vars.resolve(v).ok())
            .and_then(|v| match v {
                Val::Str(s) => Some(s),
                _ => None,
            })
            .ok_or(refuse)?;
        match search_repo(&query, self.repos) {
            Some(at) => {
                self.touch(at);
                Ok(())
            }
            None => Err(refuse),
        }
    }

    /// `node(id:)` and `nodes(ids:)`: every id must be one of the chat's repos' own, or resolve
    /// to an object in one (the forge alone knows, and the proxy asks). One that does not is
    /// another repo's, or nobody's we can say, and the whole document is refused.
    fn check_node_ids(
        &self,
        field: &Field<'_, String>,
        vars: &Vars<'_>,
        argument: &str,
        list: bool,
    ) -> Result<(), Refusal> {
        let value = field
            .arguments
            .iter()
            .find(|(n, _)| n == argument)
            .and_then(|(_, v)| vars.resolve(v).ok())
            .ok_or(Refusal::GraphqlOtherRepo)?;
        let ids: Vec<String> = match (value, list) {
            (Val::Str(id), false) => vec![id],
            (Val::List(items), true) => items
                .into_iter()
                .map(|item| match item {
                    Val::Str(id) => Ok(id),
                    _ => Err(Refusal::GraphqlOtherRepo),
                })
                .collect::<Result<_, _>>()?,
            _ => return Err(Refusal::GraphqlOtherRepo),
        };
        if ids.is_empty() || ids.len() > 100 || !ids.iter().all(|id| self.id_in_repos(id)) {
            return Err(Refusal::GraphqlOtherRepo);
        }
        Ok(())
    }

    fn id_in_repos(&self, id: &str) -> bool {
        let at = self.repos.iter().position(|r| {
            r.node_ids.iter().any(|own| own == id)
                || self
                    .objects
                    .repo_of(r.kind, &r.host, id)
                    .is_some_and(|path| r.path == path)
        });
        at.inspect(|at| self.touch(*at)).is_some()
    }

    fn touch(&self, at: usize) {
        let mut touched = self.touched.borrow_mut();
        if !touched.contains(&at) {
            touched.push(at);
        }
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
        set: &SelectionSet<'d, String>,
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
            if !mutation_allowed(self.kind, &field.name)
                && !(self.merging && merge_mutation(self.kind, &field.name))
            {
                return Err(named_mutation_refusal(&field.name));
            }
            let mut ids = Vec::new();
            for (name, value) in &field.arguments {
                let val = vars
                    .resolve(value)
                    .map_err(|()| Refusal::GraphqlOtherRepo)?;
                if commands::is_label_id_key(name) || has_label_id_key(&val) {
                    return Err(Refusal::LabelById);
                }
                collect_ids("", &val, &mut ids);
                // A label named in a document is read as a REST body's is: split at commas, and
                // refused when it is one a bot merges on or holds a control character.
                let mut labels = Vec::new();
                collect_label_names(name, &val, &mut labels);
                if labels.iter().any(|label| {
                    commands::is_merge_label(label) || commands::label_has_control(label)
                }) {
                    return Err(Refusal::MergeCommand);
                }
                // `updatePullRequestBranch`'s `updateMethod` is an enum (`MERGE` or `REBASE`) that
                // says how the base is brought into the branch, not a command a bot reads: gh
                // sends it for `gh pr update-branch`, and `MERGE` would read as a merge label.
                let texts = if field.name == "updatePullRequestBranch" {
                    strings(&without_update_method(&val))
                } else {
                    strings(&val)
                };
                if texts.iter().any(|text| commands::has_merge_command(text)) {
                    return Err(Refusal::MergeCommand);
                }
                if self.kind == Kind::GitLab
                    && strings(&val)
                        .iter()
                        .any(|text| commands::has_quick_action(text))
                {
                    return Err(Refusal::QuickAction);
                }
                // A pull request's head read from a fork reaches that fork's objects.
                if field.name == "createPullRequest" {
                    let mut heads = Vec::new();
                    collect_named("headRefName", &val, &mut heads);
                    if heads
                        .iter()
                        .any(|head| !self.repos.iter().any(|r| rest::side_is_own(head, r)))
                    {
                        return Err(Refusal::GraphqlOtherRepo);
                    }
                    // A head that is the default branch makes update-branch merge the base into
                    // it on the forge's side (#7494).
                    if heads
                        .iter()
                        .any(|head| self.repos.iter().any(|r| rest::head_is_default(head, r)))
                    {
                        return Err(Refusal::ProtectedPullHead);
                    }
                }
                // A merge request from the default branch is the wrong way round, as REST's is
                // (#7585). `projectPath` is already tied to the chat's repos by the scope check,
                // so the source is a branch of that project, as in REST.
                if field.name == "mergeRequestCreate" {
                    let mut sources = Vec::new();
                    collect_named("sourceBranch", &val, &mut sources);
                    if sources
                        .iter()
                        .any(|src| self.repos.iter().any(|r| rest::head_is_default(src, r)))
                    {
                        return Err(Refusal::ProtectedPullHead);
                    }
                }
            }
            self.check_scoped(&ids)?;
            if field.name == "updatePullRequestBranch" {
                self.check_update_branch_head(&field.arguments, vars)?;
            }
            // What a mutation asks back is read like any query: it may not walk out of the repo.
            self.walk(&Ty::Obj("Payload"), &field.selection_set, 0)?;
            class = Some(Class::GraphqlMutation(field.name.clone()));
        }
        class.ok_or(Refusal::GraphqlUnknownMutation)
    }

    /// `updatePullRequestBranch` merges (or, with `updateMethod: REBASE`, rebases) the base into
    /// the pull request's **head**: that head must be a branch of the repo the pull request is
    /// in that this chat put its own tip on, at the tip the forge reports now ([`OwnsHead`], the
    /// push rule's record; #7526). A head the forge cannot place is refused. Every
    /// `pullRequestId` of the mutation is asked.
    fn check_update_branch_head(
        &self,
        arguments: &[(String, Value<'d, String>)],
        vars: &Vars<'_>,
    ) -> Result<(), Refusal> {
        let mut pulls = Vec::new();
        for (_, value) in arguments {
            let val = vars
                .resolve(value)
                .map_err(|()| Refusal::ProtectedPullHead)?;
            collect_named("pullRequestId", &val, &mut pulls);
        }
        if pulls.is_empty() {
            return Err(Refusal::ProtectedPullHead);
        }
        for id in &pulls {
            let ours = self.repos.iter().find(|r| {
                self.objects
                    .repo_of(r.kind, &r.host, id)
                    .is_some_and(|path| r.path == path)
            });
            let head = ours.and_then(|r| {
                self.objects
                    .pull_head(r.kind, &r.host, PullRef::Node(id))
                    .map(|head| (r, head))
            });
            match head {
                Some((r, head)) if (self.owns)(r, &head) => {}
                _ => return Err(Refusal::ProtectedPullHead),
            }
        }
        Ok(())
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

/// **The one allowed repo a refused mutation document is about**, read from the ids its fields
/// name, so a person asked to allow it is asked about a repo and no other. `None` when the
/// document is not a readable mutation, names no id, an id is tied to none of `repos` (a
/// `repositoryId` the repo is known by, a `projectPath` that is its path, or an object id
/// `objects` places in it), or the ids are in more than one repo. Never decides whether the
/// request is allowed: [`decide`] does that; this only says where a held act would land.
pub fn mutation_repo(
    repos: &[&AllowedRepo],
    content_type: Option<&str>,
    body: &[u8],
    objects: &dyn ObjectRepo,
) -> Option<String> {
    let document = commands::parse(body, content_type).ok()??;
    let json = document.json?;
    let Json::Object(object) = &json else {
        return None;
    };
    let Some(Json::String(query)) = object.get("query") else {
        return None;
    };
    let empty = serde_json::Map::new();
    let given = match object.get("variables") {
        None | Some(Json::Null) => &empty,
        Some(Json::Object(map)) => map,
        Some(_) => return None,
    };
    let parsed = parse(query)?;
    let mut ids = Vec::new();
    for definition in &parsed.definitions {
        let Definition::Operation(OperationDefinition::Mutation(mutation)) = definition else {
            continue;
        };
        let vars = Vars {
            given,
            defaults: defaults(&mutation.variable_definitions),
        };
        for selection in &mutation.selection_set.items {
            let Selection::Field(field) = selection else {
                return None;
            };
            for (_, value) in &field.arguments {
                collect_ids("", &vars.resolve(value).ok()?, &mut ids);
            }
        }
    }
    let mut found: Option<String> = None;
    for (key, value) in &ids {
        let path = repo_of_id(repos, key, value, objects)?;
        match &found {
            Some(first) if *first != path => return None,
            _ => found = Some(path),
        }
    }
    found
}

/// The path of the repo among `repos` that the id `value`, named by `key`, is in.
fn repo_of_id(
    repos: &[&AllowedRepo],
    key: &str,
    value: &str,
    objects: &dyn ObjectRepo,
) -> Option<String> {
    match key {
        "repositoryId" => repos
            .iter()
            .find(|r| r.node_ids.iter().any(|id| id == value))
            .map(|r| r.path.clone()),
        "projectPath" | "fullPath" => repos.iter().find_map(|r| {
            normalise_repo_path(r.kind, value)
                .ok()
                .filter(|path| *path == r.path)
        }),
        _ => repos.iter().find_map(|r| {
            objects
                .repo_of(r.kind, &r.host, value)
                .filter(|path| *path == r.path)
        }),
    }
}

/// The label names a value holds: every string under a key that names labels (`labels`,
/// `addLabels`, `removeLabels`, `labelNames`, `label`), at any depth, each split at commas
/// ([`commands::push_labels`]). `key` is the key `val` is the value of.
fn collect_label_names(key: &str, val: &Val, out: &mut Vec<String>) {
    fn names_labels(key: &str) -> bool {
        let key = key.to_ascii_lowercase().replace('_', "");
        matches!(
            key.as_str(),
            "labels" | "addlabels" | "removelabels" | "labelnames" | "label"
        )
    }
    fn every_string(val: &Val, out: &mut Vec<String>) {
        match val {
            Val::Str(s) => commands::push_labels(s, out),
            Val::List(items) => items.iter().for_each(|v| every_string(v, out)),
            Val::Obj(fields) => fields.iter().for_each(|(_, v)| every_string(v, out)),
            _ => {}
        }
    }
    if names_labels(key) {
        every_string(val, out);
        return;
    }
    match val {
        Val::List(items) => items.iter().for_each(|v| collect_label_names(key, v, out)),
        Val::Obj(fields) => fields
            .iter()
            .for_each(|(k, v)| collect_label_names(k, v, out)),
        _ => {}
    }
}

/// Every string under a key called `name`, at any depth.
fn collect_named(name: &str, val: &Val, out: &mut Vec<String>) {
    match val {
        Val::List(items) => items.iter().for_each(|v| collect_named(name, v, out)),
        Val::Obj(fields) => fields.iter().for_each(|(k, v)| match v {
            Val::Str(s) if k == name => out.push(s.clone()),
            _ => collect_named(name, v, out),
        }),
        _ => {}
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

/// Whether any key of a value, at any depth, names labels by id (`labelIds`, `addLabelIds`).
fn has_label_id_key(val: &Val) -> bool {
    match val {
        Val::List(items) => items.iter().any(has_label_id_key),
        Val::Obj(fields) => fields
            .iter()
            .any(|(k, v)| commands::is_label_id_key(k) || has_label_id_key(v)),
        _ => false,
    }
}

/// `val` without an `updateMethod` that is one of the two enum values the forge takes. Any other
/// value under that key stays, and is read as text like the rest.
fn without_update_method(val: &Val) -> Val {
    match val {
        Val::Obj(fields) => Val::Obj(
            fields
                .iter()
                .filter(|(k, v)| {
                    !(k == "updateMethod"
                        && matches!(v, Val::Str(s) if s == "MERGE" || s == "REBASE"))
                })
                .map(|(k, v)| (k.clone(), without_update_method(v)))
                .collect(),
        ),
        other => other.clone(),
    }
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
