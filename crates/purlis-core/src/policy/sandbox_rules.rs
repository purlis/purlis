// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Compiling the sandbox from the policy** (M75-22, #7031; spec §11): one function turns a
//! loaded policy's `read` and `write` rules into the set of paths a chat's sandbox holds, at the
//! chat's start, with Cedar's typed partial evaluation (`PolicySet::tpe`).
//!
//! - **What is known and what is not** (§11.1, §11.3). The chat, its workspace and persona and
//!   the start's context (`sandbox`, `machine`, `depth`) are known; the resource is an unknown
//!   `Path`. Each rule then folds to `true`, `false`, an error or a **residual** over the path.
//!   An unset `machine` folds as an error, so the rule it guards is an error and the action it
//!   guards is denied (§6.3, Q320d.1).
//! - **The subset** (§11.2). A residual is `true`, `false`, `resource in E`, `resource == E`, a
//!   disjunction of those (`E` a Path, Workspace, Repo, Branch or a named `PathSet`), the
//!   cross-workspace form `resource.workspace != principal.workspace` and its guarded form
//!   `resource has workspace && resource.workspace != principal.workspace` (S2, S2-SF7). Anything
//!   else (`like`, another attribute, any other `&&`) is refused, **naming the rule**
//!   ([`Refusal`]). `attended` and `harness_mode` are refused at load, before any chat starts
//!   ([`Policy::sandbox_problems`]): a fixed profile cannot follow either, and a hook fallback
//!   would leave shell commands unguarded.
//! - **The compiled form** ([`Compiled`]) is, per action, the permits and the forbids, each with
//!   its `@id` and the terms it was compiled from, so a decision made from it names the rule
//!   ([`ActionRules::decide`], the same `Decision` the authorizer's path gives). A profile is
//!   the permits minus the forbids ([`ActionRules::allowed`], [`ActionRules::forbidden`]).
//! - **`@ask` on a Path** compiles to a deny in the profile plus a Block with its Allow
//!   ([`ActionRules::blocks`]): the forbid holds the path, and the window's ask is what lifts it.
//! - **Reads** are an allow-list under deny-by-default on Seatbelt and Landlock. Claude Code's
//!   sandbox takes deny paths only, so its reads are "everything but the forbids": a declared
//!   gap (slice 37), never a refusal.
//! - **The start event records the compiled rule set** ([`Compiled::record`]): the rules' ids and
//!   the shape of each term, never a path.
//!
//! The path a term meets is read by [`super::resolver`], the one resolver the hook shares.

use std::collections::BTreeMap;

use cedar_policy::{
    Context, EntityUid, PartialEntities, PartialEntityUid, PartialRequest, PolicyId, TpeResponse,
    pst,
};
use serde::{Deserialize, Serialize};

use super::resolver::{self, Entity, Kind, Layout, Resolved};
use super::{Decision, Policy, Problem, Rule};

/// The two actions a sandbox rule is about.
pub const ACTIONS: [&str; 2] = ["read", "write"];

/// The chat a sandbox is compiled for, and the start it is compiled at.
#[derive(Debug, Clone, Copy)]
pub struct Chat<'a> {
    /// The chat's id.
    pub chat: &'a str,
    /// Its workspace.
    pub workspace: &'a str,
    /// Its persona.
    pub persona: &'a str,
    /// The sandbox level the start resolved: `off`, `floor` or `full`. Until slice 11 lands,
    /// `context.sandbox` is the level `Template::sandbox_level` resolves.
    pub sandbox: &'a str,
    /// The device id, where one is confirmed.
    pub machine: Option<&'a str>,
    /// How deep in a dispatch chain the chat is.
    pub depth: i64,
}

/// One disjunct of a residual.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Term {
    /// `true`: every path.
    Always,
    /// `resource in E`.
    In(Entity),
    /// `resource == E`.
    Is(Entity),
    /// `resource.workspace != W`, `W` the chat's own workspace where the rule wrote
    /// `principal.workspace`. `guarded` says the rule wrote `resource has workspace &&` first.
    /// Unguarded, a path with no owning workspace is an evaluation error.
    NotWorkspace { workspace: String, guarded: bool },
}

impl Term {
    /// Whether the term holds of `path`, or `Err` where evaluating it errs.
    fn holds_of(&self, path: &Resolved) -> Result<bool, ()> {
        match self {
            Term::Always => Ok(true),
            Term::In(entity) => Ok(path.is_in(entity)),
            Term::Is(entity) => Ok(entity.kind == Kind::Path && entity.id == path.path),
            Term::NotWorkspace { workspace, guarded } => match &path.workspace {
                Some(owner) => Ok(owner != workspace),
                None if *guarded => Ok(false),
                None => Err(()),
            },
        }
    }

    /// The term's shape for the start event: its kind and, for a name that is not a path, the
    /// name. A path itself is never recorded.
    fn shape(&self) -> String {
        let named = |entity: &Entity| match entity.kind {
            Kind::Path => "path".to_owned(),
            kind => format!("{} {}", kind.type_name().to_lowercase(), entity.id),
        };
        match self {
            Term::Always => "always".to_owned(),
            Term::In(entity) => format!("in {}", named(entity)),
            Term::Is(entity) => format!("is {}", named(entity)),
            Term::NotWorkspace { workspace, .. } => format!("not in workspace {workspace}"),
        }
    }
}

/// One rule, compiled: its `@id`, whether it asks, and the terms of which it is the disjunction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleTerms {
    pub id: String,
    /// A forbid that asks the person (`@ask`).
    pub ask: bool,
    pub terms: Vec<Term>,
}

/// What `read` or `write` compiled to.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ActionRules {
    pub permits: Vec<RuleTerms>,
    pub forbids: Vec<RuleTerms>,
    /// The rules that erred at the start: any of them makes every answer an error, a deny
    /// naming it (§6.3), as the authorizer's path does.
    pub erred: Vec<String>,
}

/// A place a profile holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reach {
    /// Every path.
    Everything,
    /// A folder and everything in it.
    Folder(String),
    /// One path alone (`resource == Path::"…"`).
    Entry(String),
    /// What is inside a folder, the folder itself not: a repo's branch folders, which are the
    /// folders below `.worktrees/<repo>`.
    Inside(String),
    /// Every path outside all of these folders: what no workspace owns.
    Unowned(Vec<String>),
}

impl Reach {
    /// Whether the place holds `path`, normalised as [`resolver::resolve`] gives it.
    pub fn covers(&self, path: &str) -> bool {
        match self {
            Reach::Everything => true,
            Reach::Folder(folder) => resolver::within(path, folder),
            Reach::Entry(entry) => path == entry,
            Reach::Inside(folder) => path != folder && resolver::within(path, folder),
            Reach::Unowned(folders) => !folders.iter().any(|folder| resolver::within(path, folder)),
        }
    }
}

impl ActionRules {
    /// **The decision for `path`** from the compiled terms: what [`Policy::decide`] answers for
    /// the same question (the differential test holds them equal), the rule `@id`s included.
    pub fn decide(&self, path: &Resolved) -> Decision {
        let mut erred: Vec<String> = self.erred.clone();
        let mut holding = |rules: &[RuleTerms]| -> Vec<(String, bool)> {
            let mut held = Vec::new();
            for rule in rules {
                let mut any = false;
                for term in &rule.terms {
                    match term.holds_of(path) {
                        Ok(true) => any = true,
                        Ok(false) => {}
                        Err(()) => erred.push(rule.id.clone()),
                    }
                }
                if any {
                    held.push((rule.id.clone(), rule.ask));
                }
            }
            held.sort();
            held
        };
        let forbids = holding(&self.forbids);
        let permits: Vec<String> = holding(&self.permits)
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        if !erred.is_empty() {
            erred.sort();
            erred.dedup();
            return Decision::Erred { rules: erred };
        }
        if forbids.is_empty() {
            return if permits.is_empty() {
                Decision::NoPermit
            } else {
                Decision::Allow { permits }
            };
        }
        let denying: Vec<String> = forbids
            .iter()
            .filter(|(_, ask)| !ask)
            .map(|(id, _)| id.clone())
            .collect();
        if !denying.is_empty() {
            Decision::Deny { forbids: denying }
        } else if permits.is_empty() {
            Decision::NoPermit
        } else {
            Decision::Ask {
                asks: forbids.into_iter().map(|(id, _)| id).collect(),
            }
        }
    }

    /// The `@id`s of the forbids that ask: each is a deny in the profile and a Block with its
    /// Allow, which the person's answer lifts.
    pub fn blocks(&self) -> Vec<&str> {
        self.forbids
            .iter()
            .filter(|rule| rule.ask)
            .map(|rule| rule.id.as_str())
            .collect()
    }

    /// What the permits let through, as places (the allow-list of a deny-by-default profile).
    /// **Empty when a rule erred at the start**: every answer is then a deny
    /// ([`Self::decide`]), so the profile allows nothing.
    pub fn allowed(&self, layout: &Layout, sets: &BTreeMap<String, Vec<String>>) -> Vec<Reach> {
        if self.erred.is_empty() {
            reach_of(&self.permits, layout, sets)
        } else {
            Vec::new()
        }
    }

    /// What the forbids hold, asks included, as places: denied in every profile. **Everything
    /// when a rule erred at the start**, as [`Self::allowed`] is nothing then.
    pub fn forbidden(&self, layout: &Layout, sets: &BTreeMap<String, Vec<String>>) -> Vec<Reach> {
        if self.erred.is_empty() {
            reach_of(&self.forbids, layout, sets)
        } else {
            vec![Reach::Everything]
        }
    }

    /// Whether the profile (the allowed places less the forbidden ones) lets `path` through: never
    /// more than [`Self::decide`] allows, and exactly what it allows for a path the resolver
    /// reads (the property test holds both). An `@ask` forbid holds the path here: the person's
    /// Allow lifts its Block, which is not the profile's.
    pub fn profile_allows(
        &self,
        layout: &Layout,
        sets: &BTreeMap<String, Vec<String>>,
        path: &str,
    ) -> bool {
        let covered = |places: Vec<Reach>| places.iter().any(|place| place.covers(path));
        covered(self.allowed(layout, sets)) && !covered(self.forbidden(layout, sets))
    }
}

/// Whether `name` is one component of a path, as a repo's folder or a piece is.
fn is_component(name: &str) -> bool {
    !name.is_empty() && !name.contains('/') && name != "." && name != ".."
}

/// The places `rules` name, folders as the plane has them: **what the resolver would put in an
/// entity, found from the other side** (a path is in the entity exactly when a place covers it).
fn reach_of(
    rules: &[RuleTerms],
    layout: &Layout,
    sets: &BTreeMap<String, Vec<String>>,
) -> Vec<Reach> {
    let known = |workspace: &str| layout.workspaces.iter().any(|name| name == workspace);
    let workspace_folder =
        |workspace: &str| resolver::normalise(&resolver::workspace_folder(layout, workspace));
    let mut reach = Vec::new();
    for term in rules.iter().flat_map(|rule| &rule.terms) {
        match term {
            Term::Always => reach.push(Reach::Everything),
            Term::In(entity) | Term::Is(entity) => {
                let whole = matches!(term, Term::In(_));
                match entity.kind {
                    // A literal that is not spelt as the resolver spells a path holds none.
                    Kind::Path => {
                        if resolver::normalise(&entity.id).as_deref() == Some(entity.id.as_str()) {
                            reach.push(if whole {
                                Reach::Folder(entity.id.clone())
                            } else {
                                Reach::Entry(entity.id.clone())
                            });
                        }
                    }
                    // A workspace the plane does not have holds nothing.
                    Kind::Workspace if whole && known(&entity.id) => {
                        reach.extend(workspace_folder(&entity.id).map(Reach::Folder));
                    }
                    Kind::Repo if whole => {
                        for clone in &layout.clones {
                            if format!("{}/{}", clone.workspace, clone.repo) == entity.id {
                                reach.extend(resolver::normalise(&clone.folder).map(Reach::Folder));
                            }
                        }
                        // A repo's branch folders: what is inside `.worktrees/<repo>`.
                        if let Some((workspace, repo)) = entity.id.split_once('/')
                            && known(workspace)
                            && is_component(repo)
                        {
                            reach.extend(workspace_folder(workspace).map(|folder| {
                                Reach::Inside(format!("{folder}/.worktrees/{repo}"))
                            }));
                        }
                    }
                    Kind::Branch if whole => {
                        let folder = entity.id.split_once('#').and_then(|(repo, piece)| {
                            let (workspace, repo) = repo.split_once('/')?;
                            (known(workspace)
                                && is_component(repo)
                                && !repo.contains('#')
                                && is_component(piece))
                            .then(|| {
                                workspace_folder(workspace)
                                    .map(|folder| format!("{folder}/.worktrees/{repo}/{piece}"))
                            })
                            .flatten()
                        });
                        reach.extend(folder.map(Reach::Folder));
                    }
                    Kind::PathSet if whole => {
                        for member in sets.get(&entity.id).into_iter().flatten() {
                            reach
                                .extend(resolver::member_folder(layout, member).map(Reach::Folder));
                        }
                    }
                    // A path is never equal to a workspace, repo, branch or set.
                    _ => {}
                }
            }
            Term::NotWorkspace { workspace, guarded } => {
                for other in layout.workspaces.iter().filter(|name| *name != workspace) {
                    reach.extend(workspace_folder(other).map(Reach::Folder));
                }
                // Unguarded, a path no workspace owns is an evaluation error: a deny.
                if !guarded {
                    reach.push(Reach::Unowned(
                        layout
                            .workspaces
                            .iter()
                            .filter_map(|name| workspace_folder(name))
                            .collect(),
                    ));
                }
            }
        }
    }
    reach.sort_by_key(|place| format!("{place:?}"));
    reach.dedup();
    reach
}

/// What `read` and `write` compiled to for one chat, at its start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Compiled {
    pub read: ActionRules,
    pub write: ActionRules,
}

impl Compiled {
    /// **What the start event records** of the compiled set: each rule's `@id` and the shape of
    /// its terms. Never a path.
    pub fn record(&self) -> Record {
        let of = |rules: &ActionRules| ActionRecord {
            permits: rules.permits.iter().map(RuleRecord::of).collect(),
            forbids: rules.forbids.iter().map(RuleRecord::of).collect(),
        };
        Record {
            read: of(&self.read),
            write: of(&self.write),
        }
    }
}

/// The compiled rule set as a start event carries it (`protection.sandbox_rules`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    pub read: ActionRecord,
    pub write: ActionRecord,
}

/// One action's rules in the record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionRecord {
    pub permits: Vec<RuleRecord>,
    pub forbids: Vec<RuleRecord>,
}

/// One rule in the record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleRecord {
    /// Its `@id`.
    pub id: String,
    /// Whether it asks.
    #[serde(default, skip_serializing_if = "is_false")]
    pub ask: bool,
    /// The shape of each term: `always`, `in workspace billing`, `in path`.
    pub terms: Vec<String>,
}

fn is_false(value: &bool) -> bool {
    !value
}

impl RuleRecord {
    fn of(rule: &RuleTerms) -> Self {
        RuleRecord {
            id: rule.id.clone(),
            ask: rule.ask,
            terms: rule.terms.iter().map(Term::shape).collect(),
        }
    }
}

/// A rule the compile refuses, and why: it names the rule, as a load problem does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    /// The rule's `@id`, or empty where the evaluation as a whole failed.
    pub rule: String,
    /// The action it was compiled for.
    pub action: String,
    pub what: String,
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.rule.is_empty() {
            write!(f, "{}: {}", self.action, self.what)
        } else {
            write!(f, "{}: {} ({})", self.rule, self.what, self.action)
        }
    }
}

/// **Compiles `read` and `write` for `chat`** from `policy`, or refuses, naming every rule
/// outside the subset.
pub fn compile(policy: &Policy, chat: &Chat<'_>) -> Result<Compiled, Vec<Refusal>> {
    let mut refused = Vec::new();
    let mut done = Vec::new();
    for action in ACTIONS {
        match compile_action(policy, chat, action) {
            Ok(rules) => done.push(rules),
            Err(found) => refused.extend(found),
        }
    }
    if !refused.is_empty() {
        return Err(refused);
    }
    let write = done.pop().unwrap_or_default();
    let read = done.pop().unwrap_or_default();
    Ok(Compiled { read, write })
}

fn compile_action(
    policy: &Policy,
    chat: &Chat<'_>,
    action: &str,
) -> Result<ActionRules, Vec<Refusal>> {
    let whole = |what: String| {
        vec![Refusal {
            rule: String::new(),
            action: action.to_owned(),
            what,
        }]
    };
    let schema = super::schema();
    let names = super::names();
    let action_uid: &EntityUid = names
        .actions
        .get(action)
        .ok_or_else(|| whole("the schema has no such action".into()))?;
    let mut context = serde_json::json!({
        "sandbox": chat.sandbox,
        // A rule naming either is refused at load ([`Policy::sandbox_problems`]), so no rule
        // here reads these two.
        "attended": false,
        "harness_mode": "default",
        "depth": chat.depth,
    });
    if let Some(machine) = chat.machine {
        context["machine"] = machine.into();
    }
    let context = Context::from_json_value(context, Some((schema, action_uid)))
        .map_err(|e| whole(format!("the start's context does not fit: {e}")))?;
    let principal = EntityUid::from_type_name_and_id(
        names.types["Chat"].clone(),
        cedar_policy::EntityId::new(chat.chat),
    );
    let resource = PartialEntityUid::new(names.types["Path"].clone(), None);
    let request = PartialRequest::new(
        PartialEntityUid::from_concrete(principal),
        action_uid.clone(),
        resource,
        Some(context),
        schema,
    )
    .map_err(|e| whole(format!("the partial request does not fit: {e}")))?;
    let uid = |kind: &str, id: &str| serde_json::json!({"type": kind, "id": id});
    let entities = PartialEntities::from_json_value(
        serde_json::json!([
            {
                "uid": uid("Chat", chat.chat),
                "attrs": {
                    "workspace": {"__entity": uid("Workspace", chat.workspace)},
                    "persona": {"__entity": uid("Persona", chat.persona)},
                },
                "parents": [
                    uid("Workspace", chat.workspace),
                    uid("Persona", chat.persona),
                    uid("Plane", "plane"),
                ],
            },
            {"uid": uid("Workspace", chat.workspace), "attrs": {}, "parents": [uid("Plane", "plane")]},
            {"uid": uid("Persona", chat.persona), "attrs": {}, "parents": []},
            {"uid": uid("Plane", "plane"), "attrs": {}, "parents": []},
        ]),
        schema,
    )
    .map_err(|e| whole(format!("the chat's entities do not fit: {e}")))?;
    let response = policy
        .all
        .tpe(&request, &entities, schema)
        .map_err(|e| whole(format!("the partial evaluation failed: {e}")))?;

    let ask_of = |id: &PolicyId| policy.rule(id.as_ref()).is_some_and(|rule| rule.ask);
    let mut out = ActionRules::default();
    let mut refusals = Vec::new();
    let always = |id: &PolicyId, ask: bool| RuleTerms {
        id: id.to_string(),
        ask,
        terms: vec![Term::Always],
    };
    out.permits
        .extend(response.true_permits().map(|id| always(id, false)));
    out.forbids
        .extend(response.true_forbids().map(|id| always(id, ask_of(id))));
    out.erred.extend(
        response
            .error_permits()
            .chain(response.error_forbids())
            .map(|id| id.to_string()),
    );
    let mut residual = |id: &PolicyId, ask: bool, into: &mut Vec<RuleTerms>| match residual_terms(
        &response,
        id,
        chat.workspace,
    ) {
        Ok(terms) => into.push(RuleTerms {
            id: id.to_string(),
            ask,
            terms,
        }),
        Err(what) => refusals.push(Refusal {
            rule: id.to_string(),
            action: action.to_owned(),
            what,
        }),
    };
    for id in response.residual_permits() {
        residual(id, false, &mut out.permits);
    }
    for id in response.residual_forbids() {
        residual(id, ask_of(id), &mut out.forbids);
    }
    if !refusals.is_empty() {
        refusals.sort_by(|a, b| a.rule.cmp(&b.rule));
        return Err(refusals);
    }
    for rules in [&mut out.permits, &mut out.forbids] {
        rules.sort_by(|a, b| a.id.cmp(&b.id));
    }
    out.erred.sort();
    out.erred.dedup();
    Ok(out)
}

/// The terms of one residual policy: its single `when` clause, read by [`terms_of`].
fn residual_terms(
    response: &TpeResponse<'_>,
    id: &PolicyId,
    workspace: &str,
) -> Result<Vec<Term>, String> {
    let policy = response
        .get_policy(id)
        .ok_or_else(|| "its residual cannot be read".to_owned())?;
    let policy = policy.to_pst().map_err(|e| e.to_string())?;
    match policy.body().clauses().as_slice() {
        [pst::Clause::When(expr)] => terms_of(expr, workspace),
        _ => Err("its residual is not one condition".to_owned()),
    }
}

/// **The terms a residual expression is, or why it is outside the subset** (§11.2).
/// `workspace` is the chat's, which `principal.workspace` folds to.
pub fn terms_of(expr: &pst::Expr, workspace: &str) -> Result<Vec<Term>, String> {
    use pst::{BinaryOp as Op, Expr, Literal, Var};
    let resource = |e: &Expr| matches!(e, Expr::Var(Var::Resource));
    match expr {
        Expr::Literal(Literal::Bool(true)) => Ok(vec![Term::Always]),
        Expr::Literal(Literal::Bool(false)) => Ok(Vec::new()),
        Expr::BinaryOp {
            op: Op::Or,
            left,
            right,
        } => {
            let mut terms = terms_of(left, workspace)?;
            terms.extend(terms_of(right, workspace)?);
            Ok(terms)
        }
        Expr::BinaryOp { op, left, right } if resource(left) && matches!(op, Op::In | Op::Eq) => {
            let entity = entity_of(right)?;
            Ok(vec![if matches!(op, Op::In) {
                Term::In(entity)
            } else {
                Term::Is(entity)
            }])
        }
        Expr::BinaryOp {
            op: Op::NotEq,
            left,
            right,
        } => workspace_comparison(left, right, workspace, false, true).map(|term| vec![term]),
        // Partial evaluation writes `a != b` as `!(a == b)`.
        Expr::UnaryOp {
            op: pst::UnaryOp::Not,
            expr: inner,
        } if matches!(&**inner, Expr::BinaryOp { op: Op::Eq, .. }) => match &**inner {
            Expr::BinaryOp { left, right, .. } => {
                workspace_comparison(left, right, workspace, false, true).map(|term| vec![term])
            }
            _ => unreachable!("matched above"),
        },
        Expr::BinaryOp {
            op: Op::And,
            left,
            right,
        } => {
            let guard = matches!(&**left, Expr::HasAttr { expr, attrs }
                if resource(expr) && attrs.iter().map(|a| a.as_str()).eq(["workspace"]));
            // `(negated, left, right)` of the comparison the guard stands before.
            let comparison = match &**right {
                Expr::BinaryOp {
                    op: Op::NotEq,
                    left,
                    right,
                } => Some((true, left, right)),
                Expr::BinaryOp {
                    op: Op::Eq,
                    left,
                    right,
                } => Some((false, left, right)),
                Expr::UnaryOp {
                    op: pst::UnaryOp::Not,
                    expr,
                } => match &**expr {
                    Expr::BinaryOp {
                        op: Op::Eq,
                        left,
                        right,
                    } => Some((true, left, right)),
                    _ => None,
                },
                _ => None,
            };
            match comparison {
                Some((negated, left, right)) if guard => {
                    workspace_comparison(left, right, workspace, true, negated)
                        .map(|term| vec![term])
                }
                _ => Err("it joins two conditions with `&&`, which only \
                          `resource has workspace && resource.workspace != principal.workspace` \
                          (or `==`) may do"
                    .to_owned()),
            }
        }
        Expr::Like { .. } => Err("it uses `like`, which a profile cannot follow".to_owned()),
        other => Err(format!(
            "`{other}` is outside what a sandbox rule may say: `resource in E`, `resource == E`, \
             and the cross-workspace form"
        )),
    }
}

/// `resource.workspace != principal.workspace` (or a workspace named outright), or, `negated`
/// false and guarded, the `==` of it. **`==` is beyond §11.2's list, and proposed here**: a
/// project's permit for its own workspace's files is written that way (the `regulated` sample
/// project), and a profile follows it as `resource in Workspace::"…"`, which the path resolver
/// makes the same thing for a `Path` (it sets the attribute and the ancestor together).
fn workspace_comparison(
    left: &pst::Expr,
    right: &pst::Expr,
    workspace: &str,
    guarded: bool,
    negated: bool,
) -> Result<Term, String> {
    use pst::{Expr, Literal, Var};
    let attribute_of = |e: &Expr, var: Var| matches!(e, Expr::GetAttr { expr, attr } if **expr == Expr::Var(var) && attr.as_str() == "workspace");
    if !attribute_of(left, Var::Resource) {
        return Err("it compares something but the path's workspace".to_owned());
    }
    let named = match right {
        _ if attribute_of(right, Var::Principal) => workspace.to_owned(),
        Expr::Literal(Literal::EntityUID(uid)) if uid.ty.to_string() == "Workspace" => {
            uid.eid.to_string()
        }
        _ => {
            return Err(
                "it compares the path's workspace with something but a workspace".to_owned(),
            );
        }
    };
    if negated {
        Ok(Term::NotWorkspace {
            workspace: named,
            guarded,
        })
    } else if guarded {
        Ok(Term::In(Entity::new(Kind::Workspace, named)))
    } else {
        Err("it compares the path's workspace with `==` without checking it has one".to_owned())
    }
}

/// The entity a residual names on the right of `in` or `==`.
fn entity_of(expr: &pst::Expr) -> Result<Entity, String> {
    match expr {
        pst::Expr::Literal(pst::Literal::EntityUID(uid)) => {
            let ty = uid.ty.to_string();
            let kind = Kind::of_type(&ty).ok_or_else(|| {
                format!(
                    "it names a {ty}, and a sandbox rule names a Path, Workspace, Repo, \
                     Branch or PathSet"
                )
            })?;
            let id = uid.eid.to_string();
            // The hook's paths are normalised, so a literal spelt another way would never match
            // there and would be held in the profile: refused, so the two cannot differ.
            if kind == Kind::Path && resolver::normalise(&id).as_deref() != Some(id.as_str()) {
                return Err(format!(
                    "it names the path `{id}`, which is not spelt normally (absolute, with no \
                     `.`, `..`, doubled or trailing slash)"
                ));
            }
            Ok(Entity::new(kind, id))
        }
        other => Err(format!(
            "`{other}` is not an entity a sandbox rule may name"
        )),
    }
}

/// Whether `expr` has a node `found` says yes to.
fn any_node(expr: &pst::Expr, found: &dyn Fn(&pst::Expr) -> bool) -> bool {
    use pst::Expr;
    if found(expr) {
        return true;
    }
    let below = |e: &std::sync::Arc<Expr>| any_node(e, found);
    match expr {
        Expr::UnaryOp { expr, .. }
        | Expr::GetAttr { expr, .. }
        | Expr::HasAttr { expr, .. }
        | Expr::Like { expr, .. } => below(expr),
        Expr::BinaryOp { left, right, .. } => below(left) || below(right),
        Expr::Is { expr, in_expr, .. } => below(expr) || in_expr.as_ref().is_some_and(below),
        Expr::IfThenElse {
            cond,
            then_expr,
            else_expr,
        } => below(cond) || below(then_expr) || below(else_expr),
        Expr::Set(items) => items.iter().any(below),
        Expr::Record(fields) => fields.values().any(below),
        _ => false,
    }
}

impl Rule {
    /// Whether the rule applies to `read` or `write`: it names no action, or one of the two, or
    /// the `sandbox` family they are in.
    fn applies_to_the_sandbox(&self) -> bool {
        let actions = self.actions();
        actions.is_empty()
            || actions
                .iter()
                .any(|action| ACTIONS.contains(&action.as_str()) || action == "sandbox")
    }
}

impl Policy {
    /// **The problems a policy has as a sandbox's rules** (§11.2, §11.3):
    /// [`Self::sandbox_names_problems`] and, by compiling for each level a chat starts at, with
    /// the device id known and not, at a shallow and a deep chain, and for the workspaces and
    /// personas the rules themselves name, any rule outside the subset. Each names its rule. A
    /// chat's start refuses the same rules ([`compile`]); this finds them before one starts.
    ///
    /// **Its limit:** a residual's shape depends on the chat only through the constants the
    /// rules compare it with, so those are the values probed (and one the rules do not name); a
    /// rule that branches on another constant, a device id or a depth threshold, is not folded
    /// every way, and the start's own refusal stands behind it.
    pub fn sandbox_problems(&self) -> Vec<Problem> {
        let mut problems = self.sandbox_names_problems();
        if !problems.is_empty() {
            return problems;
        }
        let (workspaces, personas) = self.named_workspaces_and_personas();
        let mut chats: Vec<(String, String)> = vec![("load-check".into(), "load-check".into())];
        chats.extend(
            workspaces
                .into_iter()
                .map(|workspace| (workspace, "load-check".to_owned())),
        );
        chats.extend(
            personas
                .into_iter()
                .map(|persona| ("load-check".to_owned(), persona)),
        );
        for (workspace, persona) in &chats {
            for sandbox in ["off", "floor", "full"] {
                for machine in [None, Some("device")] {
                    for depth in [0, 1_000] {
                        let chat = Chat {
                            chat: "load-check",
                            workspace,
                            persona,
                            sandbox,
                            machine,
                            depth,
                        };
                        let Err(refused) = compile(self, &chat) else {
                            continue;
                        };
                        for refusal in refused {
                            let Some(rule) = self.rule(&refusal.rule) else {
                                continue;
                            };
                            let problem = Problem {
                                file: rule.file.clone(),
                                line: rule.line,
                                rule: Some(rule.id.clone()),
                                what: refusal.what.clone(),
                            };
                            if !problems.contains(&problem) {
                                problems.push(problem);
                            }
                        }
                    }
                }
            }
        }
        problems
    }

    /// The workspaces and personas the sandbox rules name outright, sorted and each once.
    fn named_workspaces_and_personas(&self) -> (Vec<String>, Vec<String>) {
        let found = std::cell::RefCell::new((
            std::collections::BTreeSet::new(),
            std::collections::BTreeSet::new(),
        ));
        for rule in self
            .rules
            .iter()
            .filter(|rule| rule.applies_to_the_sandbox())
        {
            let Ok(policy) = rule.policy.to_pst() else {
                continue;
            };
            for clause in policy.body().clauses() {
                let (pst::Clause::When(expr) | pst::Clause::Unless(expr)) = clause;
                any_node(expr, &|e| {
                    if let pst::Expr::Literal(pst::Literal::EntityUID(uid)) = e {
                        let (workspaces, personas) = &mut *found.borrow_mut();
                        match uid.ty.to_string().as_str() {
                            "Workspace" => workspaces.insert(uid.eid.to_string()),
                            "Persona" => personas.insert(uid.eid.to_string()),
                            _ => false,
                        };
                    }
                    false
                });
            }
        }
        let (workspaces, personas) = found.into_inner();
        (
            workspaces.into_iter().collect(),
            personas.into_iter().collect(),
        )
    }

    /// **What load refuses of a sandbox action's rules** (§11.3): one that names `attended` or
    /// `harness_mode`, or uses `like`. Read from the rule's text alone, so it is cheap enough for
    /// every load; the rest of the subset is [`Self::sandbox_problems`]'s and the start's.
    pub(super) fn sandbox_names_problems(&self) -> Vec<Problem> {
        let mut problems = Vec::new();
        let at = |rule: &Rule, what: String| Problem {
            file: rule.file.clone(),
            line: rule.line,
            rule: Some(rule.id.clone()),
            what,
        };
        for rule in self
            .rules
            .iter()
            .filter(|rule| rule.applies_to_the_sandbox())
        {
            let Ok(policy) = rule.policy.to_pst() else {
                continue;
            };
            let names = |word: &'static str| {
                move |e: &pst::Expr| match e {
                    pst::Expr::GetAttr { expr, attr } => {
                        **expr == pst::Expr::Var(pst::Var::Context) && attr.as_str() == word
                    }
                    pst::Expr::HasAttr { expr, attrs } => {
                        **expr == pst::Expr::Var(pst::Var::Context)
                            && attrs.iter().any(|attr| attr.as_str() == word)
                    }
                    _ => false,
                }
            };
            for clause in policy.body().clauses() {
                let (pst::Clause::When(expr) | pst::Clause::Unless(expr)) = clause;
                for word in ["attended", "harness_mode"] {
                    if any_node(expr, &names(word)) {
                        problems.push(at(
                            rule,
                            format!(
                                "a rule on a sandbox action cannot name `{word}`: a fixed profile \
                                 cannot follow it"
                            ),
                        ));
                    }
                }
                if any_node(expr, &|e| matches!(e, pst::Expr::Like { .. })) {
                    problems.push(at(
                        rule,
                        "a rule on a sandbox action cannot use `like`: a profile cannot follow it"
                            .to_owned(),
                    ));
                }
            }
        }
        problems
    }
}

#[cfg(test)]
#[path = "sandbox_rules_tests.rs"]
mod tests;
