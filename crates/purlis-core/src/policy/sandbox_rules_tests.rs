// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The sandbox compiled from the policy** (M75-22, #7031): the differential against the
//! authorizer on generated trees (fixed seeds, every PR), the subset's refusals at load, the
//! cross-workspace forms, `@ask` on a path, and what the start event records.

use std::collections::BTreeMap;
use std::sync::Arc;

use proptest::prelude::*;
use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};

use super::*;
use crate::policy::resolver::{CloneFolder, Entity, Kind, Layout, normalise, resolve};
use crate::policy::{File, Level, Question, World, load};

const HEADER: &str = "// purlis-policy 1\n";

fn layout() -> Layout {
    Layout {
        root: "/plane".to_owned(),
        workspaces: vec!["qa".into(), "billing".into(), "web".into()],
        clones: vec![CloneFolder {
            workspace: "qa".into(),
            repo: "repo".into(),
            folder: "/plane/workspaces/qa/repo".into(),
        }],
    }
}

fn sets() -> BTreeMap<String, Vec<String>> {
    BTreeMap::from([
        (
            "secrets".to_owned(),
            vec![
                "/plane/workspaces/qa/a/secret".to_owned(),
                "workspaces/billing/vault".to_owned(),
            ],
        ),
        (
            "secret-stores".to_owned(),
            vec!["/plane/other/x".to_owned()],
        ),
    ])
}

fn policy_with(rules: &str) -> Policy {
    let text = format!("{HEADER}{rules}");
    load(&[
        File::shipped_default(),
        File::at("policy.cedar", &text, Level::Project),
    ])
    .unwrap_or_else(|problems| panic!("the policy loads: {problems:?}"))
}

fn qa(sandbox: &'static str) -> Chat<'static> {
    Chat {
        chat: "chat-1",
        workspace: "qa",
        persona: "builder",
        sandbox,
        machine: None,
        depth: 0,
    }
}

fn path(text: &str) -> Resolved {
    resolve(&layout(), &sets(), text).expect("an absolute path")
}

/// What the host's own path answers for `path`: the authorizer over the world the shared
/// resolver describes.
fn authorizer_says(policy: &Policy, chat: &Chat<'_>, action: &str, path: &Resolved) -> Decision {
    let uid = |kind: &str, id: &str| serde_json::json!({"type": kind, "id": id});
    let world = World::from_json(serde_json::json!([
        path.entity_json(),
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
    ]))
    .expect("the world fits the schema");
    let mut context = serde_json::json!({
        "sandbox": chat.sandbox,
        "attended": false,
        "harness_mode": "default",
        "depth": chat.depth,
    });
    if let Some(machine) = chat.machine {
        context["machine"] = machine.into();
    }
    policy.decide(
        &Question {
            chat: chat.chat,
            action,
            resource: ("Path", &path.path),
            context,
        },
        &world,
    )
}

fn compiled(policy: &Policy, chat: &Chat<'_>) -> Compiled {
    compile(policy, chat).unwrap_or_else(|refused| panic!("the policy compiles: {refused:?}"))
}

// ---- the differential ---------------------------------------------------------------------

/// Each a condition over the generated tree's names: inside the subset, or (the negations and the
/// literals not spelt normally) outside it, which the compile must refuse naming the rule.
const SUBJECTS: [&str; 22] = [
    "resource in Path::\"/plane/workspaces/qa/a\"",
    "resource in Path::\"/plane/other\"",
    "resource == Path::\"/plane/other/x\"",
    "resource in Workspace::\"billing\"",
    "resource in Workspace::\"qa\"",
    "resource in Workspace::\"ghost\"",
    "resource in Repo::\"qa/repo\"",
    "resource in Repo::\"billing/repo\"",
    "resource in Branch::\"qa/repo#p1\"",
    "resource in PathSet::\"secrets\"",
    "resource in PathSet::\"missing\"",
    "resource in Workspace::\"web\" || resource in Path::\"/plane/other/y\"",
    "resource has workspace && resource.workspace != principal.workspace",
    "resource has workspace && resource.workspace != Workspace::\"web\"",
    "resource has workspace && resource.workspace == principal.workspace",
    "resource has workspace && resource.workspace == Workspace::\"billing\"",
    "true",
    "false",
    // Not spelt normally: refused, since the hook would never match them.
    "resource in Path::\"/plane/other/../other/z\"",
    "resource in Path::\"/plane/other/\"",
    "resource == Path::\"/plane//other/x\"",
    "resource in Workspace::\"billing\" && resource in Path::\"/plane/other\"",
];

/// How a rule's subject is guarded, or wrapped in an `unless`.
const GUARDS: u8 = 9;

#[derive(Debug, Clone)]
struct RuleSpec {
    permit: bool,
    ask: bool,
    actions: u8,
    subject: usize,
    guard: u8,
}

fn rule_spec() -> impl Strategy<Value = RuleSpec> {
    (
        any::<bool>(),
        any::<bool>(),
        0u8..3,
        0..SUBJECTS.len(),
        0..GUARDS,
    )
        .prop_map(|(permit, ask, actions, subject, guard)| RuleSpec {
            permit,
            ask: ask && !permit,
            actions,
            subject,
            guard,
        })
}

fn text_of(specs: &[RuleSpec]) -> String {
    let mut text = String::new();
    for (n, spec) in specs.iter().enumerate() {
        let subject = format!("({})", SUBJECTS[spec.subject]);
        let body = match spec.guard {
            1 => format!("when {{ context.sandbox == \"full\" && {subject} }}"),
            2 => format!("when {{ {subject} && context.depth < 3 }}"),
            3 => format!("when {{ principal.persona == Persona::\"builder\" && {subject} }}"),
            4 => format!("when {{ principal.persona != Persona::\"builder\" && {subject} }}"),
            5 => {
                format!("when {{ context has machine && context.machine == \"m1\" && {subject} }}")
            }
            // Another device than the one named (an unguarded read of `machine` does not
            // validate, so no loaded rule errs on an unset device; `erred` is built by hand below).
            6 => {
                format!("when {{ context has machine && context.machine != \"m1\" && {subject} }}")
            }
            7 => format!("unless {{ {subject} }}"),
            8 => format!("when {{ context.depth > 3 && {subject} }}"),
            _ => format!("when {{ {subject} }}"),
        };
        let actions = match spec.actions {
            0 => "action == Action::\"read\"",
            1 => "action == Action::\"write\"",
            _ => "action in [Action::\"read\", Action::\"write\"]",
        };
        text.push_str(&format!(
            "@id(\"sandbox-gen-{n}\") @level(\"project\"){} {} (principal, {actions}, resource) \
             {body};\n",
            if spec.ask { " @ask(\"you\")" } else { "" },
            if spec.permit { "permit" } else { "forbid" },
        ));
    }
    text
}

fn generated_path() -> impl Strategy<Value = String> {
    (
        prop::sample::select(vec![
            "workspaces/qa",
            "workspaces/billing",
            "workspaces/web",
            "workspaces/ghost",
            "workspaces",
            "other",
            "elsewhere",
        ]),
        prop::collection::vec(
            prop::sample::select(vec![
                "a",
                "b",
                "repo",
                ".worktrees",
                "p1",
                "p2",
                "secret",
                "x",
                "vault",
                "y",
                "z",
                "f",
            ]),
            0..5,
        ),
    )
        .prop_map(|(root, parts)| format!("/plane/{root}/{}", parts.join("/")))
}

/// A path that may also lie outside the plane's root, where only a rule on the root of the
/// filesystem or on the path itself reaches it.
fn generated_path_anywhere() -> impl Strategy<Value = String> {
    prop_oneof![
        4 => generated_path(),
        1 => prop::collection::vec(prop::sample::select(vec!["elsewhere", "x", "etc"]), 1..4)
            .prop_map(|parts| format!("/{}", parts.join("/"))),
    ]
}

/// A chat to compile for: its workspace, persona and device, and how deep it is.
fn generated_chat() -> impl Strategy<Value = Chat<'static>> {
    (
        prop::sample::select(vec!["qa", "billing", "web"]),
        prop::sample::select(vec!["builder", "auditor"]),
        prop::sample::select(vec!["off", "floor", "full"]),
        prop::sample::select(vec![None, Some("m1"), Some("m2")]),
        prop::sample::select(vec![0i64, 2, 5]),
    )
        .prop_map(|(workspace, persona, sandbox, machine, depth)| Chat {
            chat: "chat-1",
            workspace,
            persona,
            sandbox,
            machine,
            depth,
        })
}

/// The seeded runs of the differential and the profile property: `check` is told what the
/// policy compiled to for the generated chat, and the generated paths.
fn over_generated_policies(
    check: impl Fn(&Policy, &Chat<'_>, &Compiled, &[String], &str) -> Result<(), TestCaseError>,
) {
    over_generated_policies_from(true, check);
}

/// [`over_generated_policies`] with or without the shipped default's catch-all permit under the
/// generated rules: without it the policy permits only what a generated permit does, so
/// `allowed()` is what the profile holds, not the whole tree less the forbids.
fn over_generated_policies_from(
    with_shipped_default: bool,
    check: impl Fn(&Policy, &Chat<'_>, &Compiled, &[String], &str) -> Result<(), TestCaseError>,
) {
    for seed in [1u8, 7, 42] {
        let mut runner = TestRunner::new_with_rng(
            Config {
                cases: 80,
                failure_persistence: None,
                ..Config::default()
            },
            TestRng::from_seed(RngAlgorithm::ChaCha, &[seed; 32]),
        );
        let strategy = (
            prop::collection::vec(rule_spec(), 0..6),
            prop::collection::vec(generated_path_anywhere(), 1..10),
            generated_chat(),
        );
        runner
            .run(&strategy, |(specs, paths, chat)| {
                let text = text_of(&specs);
                // Out of the subset (a negation, an `unless`, a literal not spelt normally, a
                // second condition): refused at load, naming a rule of this policy.
                let rules_text = format!("{HEADER}{text}");
                let mut files = vec![File::at("policy.cedar", &rules_text, Level::Project)];
                if with_shipped_default {
                    files.insert(0, File::shipped_default());
                }
                let policy = match load(&files) {
                    Ok(policy) => policy,
                    Err(problems) => {
                        prop_assert!(!problems.is_empty());
                        for problem in &problems {
                            prop_assert!(
                                problem
                                    .rule
                                    .as_deref()
                                    .is_some_and(|rule| rule.starts_with("sandbox-gen-")),
                                "a load problem names its rule: {problem:?} under\n{text}"
                            );
                        }
                        return Ok(());
                    }
                };
                match compile(&policy, &chat) {
                    Ok(compiled) => check(&policy, &chat, &compiled, &paths, &text),
                    Err(refused) => {
                        // Refused, never compiled wrongly: each refusal names a rule of this
                        // policy, and the same text is refused at the start only, not loaded
                        // past a rule it cannot follow.
                        prop_assert!(!refused.is_empty());
                        for refusal in &refused {
                            prop_assert!(
                                refusal.rule.starts_with("sandbox-gen-"),
                                "a refusal names its rule: {refusal} under\n{text}"
                            );
                        }
                        Ok(())
                    }
                }
            })
            .unwrap_or_else(|e| panic!("seed {seed}: {e}"));
    }
}

#[test]
fn the_compiled_sandbox_equals_the_authorizer_on_generated_trees() {
    over_generated_policies(|policy, chat, compiled, paths, text| {
        for path_text in paths {
            let resolved = path(path_text);
            for (action, rules) in [("read", &compiled.read), ("write", &compiled.write)] {
                prop_assert_eq!(
                    rules.decide(&resolved),
                    authorizer_says(policy, chat, action, &resolved),
                    "{} {} for {:?} under\n{}",
                    action,
                    path_text,
                    chat,
                    text
                );
            }
        }
        Ok(())
    });
}

#[test]
fn the_compiled_sandbox_equals_the_authorizer_without_the_shipped_catch_all() {
    over_generated_policies_from(false, |policy, chat, compiled, paths, text| {
        for path_text in paths {
            let resolved = path(path_text);
            for (action, rules) in [("read", &compiled.read), ("write", &compiled.write)] {
                prop_assert_eq!(
                    rules.decide(&resolved),
                    authorizer_says(policy, chat, action, &resolved),
                    "{} {} for {:?} under\n{}",
                    action,
                    path_text,
                    chat,
                    text
                );
                prop_assert_eq!(
                    rules.profile_allows(&layout(), &sets(), &resolved.path),
                    authorizer_says(policy, chat, action, &resolved).allows(),
                    "{} {} for {:?} under\n{}",
                    action,
                    path_text,
                    chat,
                    text
                );
            }
        }
        Ok(())
    });
}

#[test]
fn the_profile_never_lets_through_what_decide_does_not_and_lets_through_what_it_does() {
    over_generated_policies(|policy, chat, compiled, paths, text| {
        for path_text in paths {
            let resolved = path(path_text);
            for (action, rules) in [("read", &compiled.read), ("write", &compiled.write)] {
                let said = rules.decide(&resolved);
                prop_assert_eq!(
                    rules.profile_allows(&layout(), &sets(), &resolved.path),
                    said.allows(),
                    "{} {} for {:?}: {:?} under\n{}",
                    action,
                    path_text,
                    chat,
                    said,
                    text
                );
                prop_assert_eq!(
                    said.allows(),
                    authorizer_says(policy, chat, action, &resolved).allows()
                );
            }
        }
        Ok(())
    });
}

// ---- the resolver -------------------------------------------------------------------------

#[test]
fn cedar_ancestors_equal_folder_prefixes_plus_owning_workspace_repo_and_branch_folder() {
    let mut runner = TestRunner::new_with_rng(
        Config {
            cases: 200,
            failure_persistence: None,
            ..Config::default()
        },
        TestRng::from_seed(RngAlgorithm::ChaCha, &[11; 32]),
    );
    runner
        .run(&generated_path(), |text| {
            let resolved = path(&text);
            // The folders: every prefix of the path, the path itself and the root, worked out
            // here by splitting the text, not by the resolver's walk.
            let parts: Vec<&str> = text.split('/').filter(|p| !p.is_empty()).collect();
            let mut expected: Vec<Entity> = vec![Entity::new(Kind::Path, "/")];
            for n in 1..=parts.len() {
                expected.push(Entity::new(
                    Kind::Path,
                    format!("/{}", parts[..n].join("/")),
                ));
            }
            // The owning workspace, repo and branch folder, from the plane's layout.
            if let ["plane", "workspaces", ws, rest @ ..] = parts.as_slice() {
                let known = layout().workspaces.iter().any(|name| name == ws);
                if known {
                    expected.push(Entity::new(Kind::Workspace, *ws));
                }
                if let ([".worktrees", repo, piece, ..], true) = (rest, known) {
                    expected.push(Entity::new(Kind::Repo, format!("{ws}/{repo}")));
                    expected.push(Entity::new(Kind::Branch, format!("{ws}/{repo}#{piece}")));
                }
                if *ws == "qa" && rest.first() == Some(&"repo") {
                    expected.push(Entity::new(Kind::Repo, "qa/repo"));
                }
            }
            // The named sets that list the path or a folder above it.
            let listed = |folder: &str| text == folder || text.starts_with(&format!("{folder}/"));
            if listed("/plane/workspaces/qa/a/secret") || listed("/plane/workspaces/billing/vault")
            {
                expected.push(Entity::new(Kind::PathSet, "secrets"));
            }
            if listed("/plane/other/x") {
                expected.push(Entity::new(Kind::PathSet, "secret-stores"));
            }
            expected.sort();
            expected.dedup();
            let found: Vec<Entity> = resolved.members.iter().cloned().collect();
            prop_assert_eq!(found, expected, "{}", text);
            prop_assert_eq!(
                resolved.workspace.as_deref(),
                match parts.as_slice() {
                    ["plane", "workspaces", ws, ..]
                        if layout().workspaces.contains(&ws.to_string()) =>
                        Some(*ws),
                    _ => None,
                }
            );
            Ok(())
        })
        .unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn a_path_is_spelt_one_way_whatever_dots_and_slashes_it_came_with() {
    assert_eq!(
        normalise("/plane//workspaces/qa/./a/../b").as_deref(),
        Some("/plane/workspaces/qa/b")
    );
    assert_eq!(
        normalise("/a/../.."),
        None,
        "a path cannot climb above the root"
    );
    assert_eq!(normalise("relative/path"), None, "a path is absolute");
    assert_eq!(
        path("/plane/workspaces/qa/../billing/ledger")
            .workspace
            .as_deref(),
        Some("billing"),
        "a path that climbs out of one workspace into another is owned by the one it lands in"
    );
}

#[test]
fn the_root_of_the_filesystem_as_a_set_member_holds_every_path() {
    let layout = layout();
    assert_eq!(
        crate::policy::resolver::member_folder(&layout, "/").as_deref(),
        Some("/"),
        "an absolute `/` is the root of the filesystem, not the plane's"
    );
    let sets = BTreeMap::from([("everything".to_owned(), vec!["/".to_owned()])]);
    for text in ["/plane/workspaces/qa/a", "/elsewhere/x", "/"] {
        let resolved = resolve(&layout, &sets, text).expect("an absolute path");
        assert!(
            resolved
                .members
                .contains(&Entity::new(Kind::PathSet, "everything")),
            "{text} is in the set"
        );
    }
}

// ---- the subset, refused at load ----------------------------------------------------------

fn problems_of(rules: &str) -> Vec<Problem> {
    policy_with(rules).sandbox_problems()
}

/// What `load` refuses a project file for, where it does.
fn refused_at_load(rules: &str) -> Vec<Problem> {
    let text = format!("{HEADER}{rules}");
    match load(&[
        File::shipped_default(),
        File::at("policy.cedar", &text, Level::Project),
    ]) {
        Ok(_) => panic!("the policy was loaded"),
        Err(problems) => problems,
    }
}

#[test]
fn a_sandbox_rule_naming_attended_is_refused_at_load() {
    let found = refused_at_load(
        "@id(\"sandbox-needs-someone\") @level(\"project\")\n\
         forbid (principal, action == Action::\"read\", resource) when { !context.attended };\n",
    );
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].rule.as_deref(), Some("sandbox-needs-someone"));
    assert!(found[0].what.contains("attended"), "{}", found[0].what);
}

#[test]
fn a_sandbox_rule_naming_harness_mode_is_refused_at_load() {
    let found = refused_at_load(
        "@id(\"sandbox-by-mode\") @level(\"project\")\n\
         forbid (principal, action in [Action::\"write\"], resource) \
         when { context.harness_mode == \"plan\" };\n",
    );
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].rule.as_deref(), Some("sandbox-by-mode"));
    assert!(found[0].what.contains("harness_mode"), "{}", found[0].what);
}

#[test]
fn a_sandbox_rule_using_like_is_refused_at_load() {
    let found = refused_at_load(
        "@id(\"sandbox-by-pattern\") @level(\"project\")\n\
         forbid (principal, action == Action::\"read\", resource) \
         when { context.sandbox like \"fl*\" };\n",
    );
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].rule.as_deref(), Some("sandbox-by-pattern"));
    assert!(found[0].what.contains("like"), "{}", found[0].what);
}

#[test]
fn a_rule_on_another_family_may_name_attended_without_being_refused() {
    let found = problems_of(
        "@id(\"guards-merge-unattended\") @level(\"project\")\n\
         forbid (principal, action == Action::\"merge\", resource) when { !context.attended };\n",
    );
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn a_residual_outside_the_subset_is_refused_naming_the_rule() {
    // Valid Cedar that the validator accepts, and that no profile can follow: two conditions on
    // the path joined by `&&`.
    let found = refused_at_load(
        "@id(\"sandbox-only-billing\") @level(\"project\")\n\
         forbid (principal, action == Action::\"read\", resource) \
         when { resource in Workspace::\"billing\" && resource in Path::\"/plane/x\" };\n",
    );
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].rule.as_deref(), Some("sandbox-only-billing"));
    assert_eq!(found[0].line, 2, "it names the rule's line in its file");
    assert!(found[0].what.contains("&&"), "{}", found[0].what);
}

#[test]
fn an_own_workspace_permit_written_with_an_equality_compiles_to_the_workspace() {
    let policy = policy_with(
        "@id(\"sandbox-own-workspace-files\") @level(\"project\")\n\
         permit (principal, action in [Action::\"read\", Action::\"write\"], resource)\n\
         when { resource has workspace && resource.workspace == principal.workspace };\n",
    );
    assert!(policy.sandbox_problems().is_empty());
    let compiled = compiled(&policy, &qa("full"));
    let permit = compiled
        .read
        .permits
        .iter()
        .find(|rule| rule.id == "sandbox-own-workspace-files")
        .expect("the permit compiles");
    assert_eq!(
        permit.terms,
        vec![Term::In(Entity::new(Kind::Workspace, "qa"))]
    );
}

#[test]
fn the_shipped_default_and_a_plain_policy_have_no_sandbox_problems() {
    assert!(problems_of("").is_empty());
}

#[test]
fn a_rule_outside_the_subset_for_the_workspace_it_names_is_refused_at_load() {
    // For any other workspace the first condition folds the rule away: only a chat in `qa`
    // would meet the `&&`, so the load check probes the workspaces the rules name.
    let found = refused_at_load(
        "@id(\"sandbox-qa-only\") @level(\"project\")\n\
         forbid (principal, action == Action::\"read\", resource) when { \
         principal.workspace == Workspace::\"qa\" && resource in Workspace::\"billing\" \
         && resource in Path::\"/plane/x\" };\n",
    );
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].rule.as_deref(), Some("sandbox-qa-only"));
}

#[test]
fn a_path_literal_not_spelt_normally_is_refused_naming_the_rule() {
    for literal in [
        "/plane/other/../x",
        "/plane/other/",
        "/plane//x",
        "/plane/./x",
    ] {
        let found = refused_at_load(&format!(
            "@id(\"sandbox-dotted\") @level(\"project\")\n\
             forbid (principal, action == Action::\"read\", resource) \
             when {{ resource in Path::\"{literal}\" }};\n"
        ));
        assert_eq!(found.len(), 1, "{literal}: {found:?}");
        assert_eq!(found[0].rule.as_deref(), Some("sandbox-dotted"));
        assert!(
            found[0].what.contains("not spelt normally"),
            "{}",
            found[0].what
        );
    }
}

#[test]
fn a_rule_that_erred_at_the_start_leaves_the_profile_allowing_nothing() {
    let rules = ActionRules {
        permits: vec![RuleTerms {
            id: "permit-all".into(),
            ask: false,
            terms: vec![Term::Always],
        }],
        erred: vec!["sandbox-needs-a-device".into()],
        ..ActionRules::default()
    };
    assert_eq!(
        rules.decide(&path("/plane/x")),
        Decision::Erred {
            rules: vec!["sandbox-needs-a-device".into()]
        }
    );
    assert!(rules.allowed(&layout(), &sets()).is_empty());
    assert_eq!(rules.forbidden(&layout(), &sets()), vec![Reach::Everything]);
    assert!(!rules.profile_allows(&layout(), &sets(), "/plane/x"));
}

#[test]
fn a_rule_on_the_device_follows_the_start_and_folds_away_where_none_is_confirmed() {
    let policy = policy_with(
        "@id(\"sandbox-by-device\") @level(\"project\")\n\
         forbid (principal, action == Action::\"read\", resource)\n\
         when { context has machine && context.machine == \"m1\" && \
         resource in Path::\"/plane/elsewhere\" };\n",
    );
    let allows = |compiled: &Compiled| {
        compiled
            .read
            .profile_allows(&layout(), &sets(), "/plane/elsewhere/x")
    };
    assert!(allows(&compiled(&policy, &qa("full"))));
    let mut chat = qa("full");
    chat.machine = Some("m1");
    assert!(!allows(&compiled(&policy, &chat)));
    chat.machine = Some("m2");
    assert!(allows(&compiled(&policy, &chat)));
}

#[test]
fn a_workspace_the_plane_does_not_have_holds_nothing_and_a_repos_worktrees_folder_is_not_in_it() {
    let policy = policy_with(
        "@id(\"sandbox-ghost-or-repo\") @level(\"project\")\n\
         permit (principal, action == Action::\"read\", resource)\n\
         when { resource in Workspace::\"ghost\" || resource in Repo::\"billing/repo\" };\n",
    );
    // Only this rule: the shipped default's catch-all permit would allow every path.
    let mut rules = compiled(&policy, &qa("full")).read;
    rules
        .permits
        .retain(|rule| rule.id == "sandbox-ghost-or-repo");
    rules.forbids.clear();
    let allowed = |text: &str| rules.profile_allows(&layout(), &sets(), text);
    let decides = |text: &str| rules.decide(&path(text)).allows();
    for text in [
        "/plane/workspaces/ghost/a",
        "/plane/workspaces/billing/.worktrees/repo",
        "/plane/workspaces/billing/.worktrees/other/p1/f",
    ] {
        assert!(!allowed(text) && !decides(text), "{text}");
    }
    let nested = "/plane/workspaces/billing/.worktrees/repo/p1/f";
    assert!(
        allowed(nested) && decides(nested),
        "a branch folder of the repo is in it"
    );
}

#[test]
fn a_path_spelt_in_another_case_fails_closed() {
    for spelt in [
        "/plane/Workspaces/billing/ledger",
        "/plane/workspaces/Billing/ledger",
        "/plane/workspaces/qa/.Worktrees/repo/p1",
        "/plane/workspaces/qa/Repo/x",
        "/plane/WORKSPACES/qa/a/secret/k",
    ] {
        assert!(
            resolve(&layout(), &sets(), spelt).is_none(),
            "{spelt} names the plane's folders in another case, so it reads as no path"
        );
    }
    assert!(resolve(&layout(), &sets(), "/plane/workspaces/qa/Notes.md").is_some());
}

// ---- the cross-workspace forms ------------------------------------------------------------

#[test]
fn the_cross_workspace_residual_is_admitted_and_compiled() {
    use pst::{BinaryOp, Expr, Var};
    let attribute = |var| {
        Arc::new(Expr::GetAttr {
            expr: Arc::new(Expr::Var(var)),
            attr: "workspace".into(),
        })
    };
    let bare = Expr::BinaryOp {
        op: BinaryOp::NotEq,
        left: attribute(Var::Resource),
        right: attribute(Var::Principal),
    };
    assert_eq!(
        terms_of(&bare, "qa"),
        Ok(vec![Term::NotWorkspace {
            workspace: "qa".into(),
            guarded: false
        }])
    );
    // And what a bare one does to a path with no owning workspace: it errs, which denies.
    let rules = ActionRules {
        forbids: vec![RuleTerms {
            id: "sandbox-bare".into(),
            ask: false,
            terms: terms_of(&bare, "qa").expect("admitted"),
        }],
        ..ActionRules::default()
    };
    assert_eq!(
        rules.decide(&path("/etc/hosts")),
        Decision::Erred {
            rules: vec!["sandbox-bare".into()]
        }
    );
    assert_eq!(
        rules.decide(&path("/plane/workspaces/billing/ledger")),
        Decision::Deny {
            forbids: vec!["sandbox-bare".into()]
        },
        "a path of another workspace is held"
    );
}

#[test]
fn the_guarded_cross_workspace_form_is_admitted_and_compiled() {
    let policy = policy_with("");
    let compiled = compiled(&policy, &qa("floor"));
    let held = compiled
        .read
        .forbids
        .iter()
        .find(|rule| rule.id == "sandbox-other-workspace-read")
        .expect("the shipped default's cross-workspace read forbid compiles");
    assert_eq!(
        held.terms,
        vec![Term::NotWorkspace {
            workspace: "qa".into(),
            guarded: true
        }]
    );
    assert!(
        held.ask,
        "the shipped default asks before another workspace is read"
    );
    assert!(
        compiled
            .write
            .forbids
            .iter()
            .all(|rule| rule.id != "sandbox-other-workspace-read"),
        "it is a rule about reads"
    );
}

#[test]
fn another_workspaces_file_is_denied_by_decide_and_held_by_forbidden_when_the_policy_forbids_it() {
    let policy = policy_with(
        "@id(\"sandbox-no-cross-workspace-read\") @level(\"project\")\n\
         forbid (principal, action == Action::\"read\", resource)\n\
         when { resource has workspace && resource.workspace != principal.workspace };\n",
    );
    let compiled = compiled(&policy, &qa("full"));
    let billing = path("/plane/workspaces/billing/ledger/2026.csv");
    let own = path("/plane/workspaces/qa/notes.md");
    assert!(
        matches!(
            compiled.read.decide(&billing),
            Decision::Deny { ref forbids } if forbids == &vec!["sandbox-no-cross-workspace-read".to_owned()]
        ),
        "{:?}",
        compiled.read.decide(&billing)
    );
    assert!(compiled.read.decide(&own).allows());
    // The compiled form a harness compiler will read (none does yet): the folders held are every
    // other workspace's, never qa's own.
    let held = compiled.read.forbidden(&layout(), &sets());
    assert!(held.contains(&Reach::Folder("/plane/workspaces/billing".into())));
    assert!(held.contains(&Reach::Folder("/plane/workspaces/web".into())));
    assert!(!held.contains(&Reach::Folder("/plane/workspaces/qa".into())));
    assert!(
        !held.contains(&Reach::Everything),
        "the chat's own files and the rest of the plane stay readable"
    );
}

// ---- `@ask` on a path ---------------------------------------------------------------------

#[test]
fn an_ask_on_a_path_compiles_to_a_block_with_allow() {
    let policy = policy_with(
        "@id(\"sandbox-ask-ledger\") @level(\"project\") @ask(\"you\")\n\
         forbid (principal, action == Action::\"read\", resource)\n\
         when { resource in Path::\"/plane/workspaces/qa/ledger\" };\n",
    );
    let compiled = compiled(&policy, &qa("full"));
    assert!(compiled.read.blocks().contains(&"sandbox-ask-ledger"));
    assert!(
        compiled
            .read
            .forbidden(&layout(), &sets())
            .contains(&Reach::Folder("/plane/workspaces/qa/ledger".into())),
        "the profile denies the path until the person's Allow lifts its Block"
    );
    assert_eq!(
        compiled
            .read
            .decide(&path("/plane/workspaces/qa/ledger/2026.csv")),
        Decision::Ask {
            asks: vec!["sandbox-ask-ledger".into()]
        }
    );
    assert!(
        compiled.write.blocks().is_empty(),
        "an ask on a read is not an ask on a write"
    );
}

// ---- the start's context folds ------------------------------------------------------------

#[test]
fn the_start_context_folds_into_the_compiled_rules() {
    let policy = policy_with(
        "@id(\"sandbox-full-only\") @level(\"project\")\n\
         forbid (principal, action == Action::\"write\", resource)\n\
         when { context.sandbox == \"full\" && resource in Workspace::\"web\" };\n",
    );
    let holds = |sandbox| {
        compiled(&policy, &qa(sandbox))
            .write
            .forbids
            .iter()
            .any(|rule| rule.id == "sandbox-full-only")
    };
    assert!(holds("full"));
    assert!(
        !holds("floor"),
        "a level the rule does not name folds it away"
    );
    assert!(!holds("off"));
}

// ---- the start event ----------------------------------------------------------------------

#[test]
fn the_start_event_records_the_compiled_rule_set() {
    let policy = policy_with(
        "@id(\"sandbox-ask-ledger\") @level(\"project\") @ask(\"you\")\n\
         forbid (principal, action == Action::\"read\", resource)\n\
         when { resource in Path::\"/plane/workspaces/qa/ledger\" };\n",
    );
    let record = compiled(&policy, &qa("full")).record();
    let json = serde_json::to_value(&record).expect("a record is JSON");
    let text = json.to_string();
    let forbid = json["read"]["forbids"]
        .as_array()
        .and_then(|rules| rules.iter().find(|r| r["id"] == "sandbox-ask-ledger"))
        .unwrap_or_else(|| panic!("the record names the rule: {text}"));
    assert_eq!(forbid["ask"], true);
    assert_eq!(forbid["terms"], serde_json::json!(["in path"]));
    assert!(
        json["read"]["forbids"]
            .as_array()
            .expect("a list")
            .iter()
            .any(|r| r["id"] == "sandbox-other-workspace-read"
                && r["terms"] == serde_json::json!(["not in workspace qa"])),
        "{text}"
    );
    assert!(
        json["read"]["permits"]
            .as_array()
            .expect("a list")
            .iter()
            .any(|r| r["id"] == "policy-everything-else"
                && r["terms"] == serde_json::json!(["always"])),
        "{text}"
    );
    assert!(
        !text.contains("/plane") && !text.contains("workspaces/"),
        "the record carries no path: {text}"
    );
    let again: Record = serde_json::from_value(json).expect("a start event reads back");
    assert_eq!(again, record);
}
