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
        ("secret-stores".to_owned(), vec!["/plane/other/x".to_owned()]),
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

/// Each a condition inside the subset, over the generated tree's names.
const SUBJECTS: [&str; 13] = [
    "resource in Path::\"/plane/workspaces/qa/a\"",
    "resource in Path::\"/plane/other\"",
    "resource == Path::\"/plane/other/x\"",
    "resource in Workspace::\"billing\"",
    "resource in Workspace::\"qa\"",
    "resource in Repo::\"qa/repo\"",
    "resource in Branch::\"qa/repo#p1\"",
    "resource in PathSet::\"secrets\"",
    "resource in Workspace::\"web\" || resource in Path::\"/plane/other/y\"",
    "resource has workspace && resource.workspace != principal.workspace",
    "resource has workspace && resource.workspace != Workspace::\"web\"",
    "true",
    "false",
];

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
        0u8..3,
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
        let condition = match spec.guard {
            1 => format!("context.sandbox == \"full\" && {subject}"),
            2 => format!("{subject} && context.depth < 3"),
            _ => subject,
        };
        let actions = match spec.actions {
            0 => "action == Action::\"read\"",
            1 => "action == Action::\"write\"",
            _ => "action in [Action::\"read\", Action::\"write\"]",
        };
        text.push_str(&format!(
            "@id(\"sandbox-gen-{n}\") @level(\"project\"){} {} (principal, {actions}, resource) \
             when {{ {condition} }};\n",
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
            "other",
            "elsewhere",
        ]),
        prop::collection::vec(
            prop::sample::select(vec![
                "a", "b", "repo", ".worktrees", "p1", "p2", "secret", "x", "vault", "y", "f",
            ]),
            0..5,
        ),
    )
        .prop_map(|(root, parts)| format!("/plane/{root}/{}", parts.join("/")))
}

#[test]
fn the_compiled_sandbox_equals_the_authorizer_on_generated_trees() {
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
            prop::collection::vec(generated_path(), 1..10),
            prop::sample::select(vec!["off", "floor", "full"]),
        );
        runner
            .run(&strategy, |(specs, paths, sandbox)| {
                let policy = policy_with(&text_of(&specs));
                let chat = qa(sandbox);
                let compiled = compiled(&policy, &chat);
                for text in &paths {
                    let resolved = path(text);
                    for (action, rules) in [("read", &compiled.read), ("write", &compiled.write)] {
                        prop_assert_eq!(
                            rules.decide(&resolved),
                            authorizer_says(&policy, &chat, action, &resolved),
                            "{} {} under\n{}",
                            action,
                            text,
                            text_of(&specs)
                        );
                    }
                }
                Ok(())
            })
            .unwrap_or_else(|e| panic!("seed {seed}: {e}"));
    }
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
                expected.push(Entity::new(Kind::Path, format!("/{}", parts[..n].join("/"))));
            }
            // The owning workspace, repo and branch folder, from the plane's layout.
            if let ["plane", "workspaces", ws, rest @ ..] = parts.as_slice() {
                expected.push(Entity::new(Kind::Workspace, *ws));
                if let [".worktrees", repo, piece, ..] = rest {
                    expected.push(Entity::new(Kind::Repo, format!("{ws}/{repo}")));
                    expected.push(Entity::new(Kind::Branch, format!("{ws}/{repo}#{piece}")));
                }
                if *ws == "qa" && rest.first() == Some(&"repo") {
                    expected.push(Entity::new(Kind::Repo, "qa/repo"));
                }
            }
            // The named sets that list the path or a folder above it.
            let listed = |folder: &str| {
                text == folder || text.starts_with(&format!("{folder}/"))
            };
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
                    ["plane", "workspaces", ws, ..] if layout().workspaces.contains(&ws.to_string()) =>
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
    assert_eq!(normalise("/a/../.."), None, "a path cannot climb above the root");
    assert_eq!(normalise("relative/path"), None, "a path is absolute");
    assert_eq!(
        path("/plane/workspaces/qa/../billing/ledger").workspace.as_deref(),
        Some("billing"),
        "a path that climbs out of one workspace into another is owned by the one it lands in"
    );
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
    let found = problems_of(
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
    assert_eq!(permit.terms, vec![Term::In(Entity::new(Kind::Workspace, "qa"))]);
}

#[test]
fn the_shipped_default_and_a_plain_policy_have_no_sandbox_problems() {
    assert!(problems_of("").is_empty());
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
    assert!(held.ask, "the shipped default asks before another workspace is read");
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
fn another_workspaces_file_is_unreadable_when_the_policy_forbids_it_in_every_harness() {
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
    // The one compiled form every harness's compiler reads: the folders held are every other
    // workspace's, never qa's own. Seatbelt and Landlock deny them under a read allow-list;
    // Claude Code's sandbox takes them as deny paths.
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
    assert!(!holds("floor"), "a level the rule does not name folds it away");
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
            .any(|r| r["id"] == "policy-everything-else" && r["terms"] == serde_json::json!(["always"])),
        "{text}"
    );
    assert!(
        !text.contains("/plane") && !text.contains("workspaces/"),
        "the record carries no path: {text}"
    );
    let again: Record = serde_json::from_value(json).expect("a start event reads back");
    assert_eq!(again, record);
}
