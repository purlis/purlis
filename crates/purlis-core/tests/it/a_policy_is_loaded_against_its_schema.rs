// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A project policy is loaded against the schema purlis embeds, and one decision stays within
//! its budget** (M75 slice 01, #7009; spec §3 and §6).
//!
//! The schema is spec §3's vocabulary: every family a Cedar action group, every action in
//! exactly one of them, and every attribute a rule can read declaring where purlis takes it from
//! (sealed, committed or host-supplied). A policy file names its version on its first line, is
//! validated strictly, and passes purlis's own checks of its annotations. Whatever fails is
//! reported with its file, its line and the rule's `@id`.
//!
//! The prototype's three sample projects (`work/who-judges/prototypes/policy-writing/projects/`
//! on the plane) are copied in under `tests/fixtures/policy/` as they were, with only what the
//! spec renamed changed: each rule's `@id` carries its family's prefix and the catch-all is
//! `policy-everything-else` (Q326).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::str::FromStr as _;
use std::time::{Duration, Instant};

use purlis_core::policy::{self, Decision, File, Level, Problem, Question, World};

/// The fixtures copied in from the prototype.
fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/policy")
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The shipped default with one project file on top.
fn with_project(text: &str) -> Result<policy::Policy, Vec<Problem>> {
    policy::load(&[
        File::shipped_default(),
        File::at("policy.cedar", text, Level::Project),
    ])
}

/// The problems a load reports, or a panic naming the load that passed.
fn refused(loaded: Result<policy::Policy, Vec<Problem>>) -> Vec<Problem> {
    match loaded {
        Ok(_) => panic!("the load passed; it should have been refused"),
        Err(problems) => problems,
    }
}

/// The embedded schema in Cedar's JSON form, where its annotations can be read.
fn schema_json() -> serde_json::Value {
    let (fragment, _) = cedar_policy::SchemaFragment::from_cedarschema_str(policy::SCHEMA)
        .expect("the schema parses");
    let json = fragment
        .to_json_value()
        .expect("the schema has a JSON form");
    json.get("").cloned().expect("the schema has no namespace")
}

/// Whether an action of the schema's JSON form applies to anything: a group doesn't.
fn applies(action: &serde_json::Value) -> bool {
    action["appliesTo"]["principalTypes"]
        .as_array()
        .is_some_and(|types| !types.is_empty())
}

#[test]
fn a_policy_that_fails_strict_validation_is_reported_with_its_file_and_line() {
    let text = "// purlis-policy 1\n\
                \n\
                @id(\"guards-git-quiet-push\")\n\
                forbid (principal, action == Action::\"push\", resource)\n\
                \x20 when { context.quietly };\n";
    let problems = refused(with_project(text));
    assert_eq!(problems.len(), 1, "{problems:#?}");
    let problem = &problems[0];
    assert_eq!(problem.file, "policy.cedar");
    assert_eq!(
        problem.line, 5,
        "the line the unknown attribute is on: {problem:#?}"
    );
    assert_eq!(problem.rule.as_deref(), Some("guards-git-quiet-push"));
    assert!(
        problem.to_string().starts_with("policy.cedar:5: "),
        "{problem}"
    );
}

#[test]
fn a_rule_without_an_id_is_refused_at_load() {
    let text = "// purlis-policy 1\n\
                \n\
                forbid (principal, action == Action::\"merge\", resource);\n";
    let problems = refused(with_project(text));
    assert_eq!(problems.len(), 1, "{problems:#?}");
    assert_eq!(
        (problems[0].file.as_str(), problems[0].line),
        ("policy.cedar", 3)
    );
    assert_eq!(
        problems[0].rule, None,
        "the line names it, since its @id is what failed"
    );
    assert!(problems[0].what.contains("@id"), "{}", problems[0]);
}

#[test]
fn a_rule_whose_id_lacks_its_family_prefix_or_takes_the_reserved_one_is_refused_at_load() {
    for id in [
        "merge-hold-again",
        "policy-everything-else",
        "Guards-Merge-Loud",
    ] {
        let text = format!(
            "// purlis-policy 1\n@id(\"{id}\")\nforbid (principal, action == Action::\"merge\", resource);\n"
        );
        let problems = refused(with_project(&text));
        assert_eq!(problems.len(), 1, "{id}: {problems:#?}");
        assert_eq!(
            (problems[0].line, problems[0].rule.as_deref()),
            (2, Some(id))
        );
    }
    let spans_two = "// purlis-policy 1\n\
                     @id(\"vaults-or-secrets-never\")\n\
                     forbid (principal, action in [Action::\"use\", Action::\"reveal\"], resource);\n";
    assert!(
        with_project(spans_two).is_ok(),
        "a rule over two families takes either's prefix"
    );
}

#[test]
fn two_rules_with_one_id_at_different_levels_are_refused_at_load() {
    let workspace = "// purlis-policy 1\n\
                     @id(\"guards-merge-hold\") @level(\"workspace\")\n\
                     permit (principal in Workspace::\"web\", action == Action::\"merge\", resource);\n";
    let problems = refused(policy::load(&[
        File::shipped_default(),
        File::at("web/policy.cedar", workspace, Level::Workspace),
    ]));
    assert_eq!(problems.len(), 1, "{problems:#?}");
    assert_eq!(problems[0].file, "web/policy.cedar");
    assert_eq!(
        (problems[0].line, problems[0].rule.as_deref()),
        (2, Some("guards-merge-hold"))
    );
    assert!(
        problems[0].what.contains("project"),
        "names the other level: {}",
        problems[0]
    );

    let same_level = "// purlis-policy 1\n\
                      @id(\"guards-merge-hold\")\n\
                      forbid (principal, action == Action::\"merge\", resource);\n";
    let replaced = with_project(same_level).expect("the same @id at the same level replaces it");
    let ids = replaced.rules();
    assert_eq!(
        ids.iter().filter(|id| *id == "guards-merge-hold").count(),
        1,
        "{ids:?}"
    );
}

#[test]
fn two_files_at_one_level_with_one_id_are_refused_naming_both() {
    // Two workspaces' files, each at the Workspace level: neither may overwrite the other.
    let web = "// purlis-policy 1\n\
               @id(\"guards-merge-workspace\") @level(\"workspace\")\n\
               forbid (principal in Workspace::\"web\", action == Action::\"merge\", resource);\n";
    let api = "// purlis-policy 1\n\
               \n\
               @id(\"guards-merge-workspace\") @level(\"workspace\")\n\
               permit (principal in Workspace::\"api\", action == Action::\"merge\", resource);\n";
    let problems = refused(policy::load(&[
        File::shipped_default(),
        File::at("web/policy.cedar", web, Level::Workspace),
        File::at("api/policy.cedar", api, Level::Workspace),
    ]));
    assert_eq!(problems.len(), 1, "{problems:#?}");
    let problem = &problems[0];
    assert_eq!(
        (problem.file.as_str(), problem.line, problem.rule.as_deref()),
        ("api/policy.cedar", 3, Some("guards-merge-workspace"))
    );
    assert!(
        problem.what.contains("web/policy.cedar:2"),
        "names the other file and line: {problem}"
    );
}

#[test]
fn a_level_above_its_file_or_you_in_a_committed_file_is_refused_at_load() {
    for level in ["organisation", "you", "nobody"] {
        let text = format!(
            "// purlis-policy 1\n@id(\"guards-merge-held\") @level(\"{level}\")\nforbid (principal, action == Action::\"merge\", resource);\n"
        );
        let problems = refused(with_project(&text));
        assert_eq!(problems.len(), 1, "{level}: {problems:#?}");
        assert_eq!(problems[0].line, 2, "{level}");
    }
}

#[test]
fn an_ask_naming_anyone_but_you_fails_validation() {
    let text = "// purlis-policy 1\n\
                @id(\"guards-merge-held\") @ask(\"admin\")\n\
                forbid (principal, action == Action::\"merge\", resource);\n";
    let problems = refused(with_project(text));
    assert_eq!(problems.len(), 1, "{problems:#?}");
    assert_eq!(
        (problems[0].line, problems[0].rule.as_deref()),
        (2, Some("guards-merge-held"))
    );
    assert!(problems[0].what.contains("@ask"), "{}", problems[0]);

    for (annotation, value) in [("keep", "forever"), ("timed", "2h")] {
        let text = format!(
            "// purlis-policy 1\n@id(\"guards-merge-held\") @ask(\"you\") @{annotation}(\"{value}\")\nforbid (principal, action == Action::\"merge\", resource);\n"
        );
        let problems = refused(with_project(&text));
        assert_eq!(problems.len(), 1, "@{annotation}: {problems:#?}");
    }
}

#[test]
fn an_action_named_like_its_family_fails_validation() {
    assert_eq!(
        policy::check_schema(policy::SCHEMA),
        Vec::<String>::new(),
        "the embedded schema"
    );

    let action_named_skills = policy::SCHEMA
        .replace(
            "\"mode\", \"tools\", \"trust\", \"runners\", \"consents\", \"limits\", \"audit\";",
            "\"mode\", \"tools\", \"trust\", \"runners\", \"consents\", \"limits\", \"audit\";\naction \"skills\" appliesTo { principal: Chat, resource: Skill, context: Ctx };",
        )
        .replacen("\"browser\", \"skills\",", "\"browser\",", 1);
    assert_ne!(
        action_named_skills,
        policy::SCHEMA,
        "the test's edit applied"
    );
    let problems = policy::check_schema(&action_named_skills);
    assert!(
        problems
            .iter()
            .any(|p| p.contains("\"skills\"") && p.contains("family")),
        "{problems:#?}"
    );
}

#[test]
fn every_action_belongs_to_exactly_one_family_group() {
    let families: BTreeSet<&str> = policy::FAMILIES.into_iter().collect();
    let mut expected: BTreeSet<&str> = purlis_core::enforcement::Family::ALL
        .iter()
        .map(|f| f.key())
        .collect();
    expected.insert("audit");
    assert_eq!(
        families, expected,
        "ADR 0125's 24 families and audit (spec §3.3)"
    );

    let schema = schema_json();
    let actions = schema["actions"].as_object().expect("actions");
    let mut verbs = 0;
    for (name, action) in actions {
        let groups: Vec<&str> = action
            .get("memberOf")
            .and_then(|m| m.as_array())
            .map(|m| m.iter().filter_map(|g| g["id"].as_str()).collect())
            .unwrap_or_default();
        if families.contains(name.as_str()) {
            assert!(
                !applies(action),
                "the family {name} is a group, not an action"
            );
            assert!(groups.is_empty(), "the family {name} sits in no group");
        } else {
            verbs += 1;
            assert_eq!(
                groups.len(),
                1,
                "{name} is in exactly one group: {groups:?}"
            );
            assert!(
                families.contains(groups[0]),
                "{name}'s group {} is a family",
                groups[0]
            );
        }
    }
    for family in &families {
        assert!(
            actions.contains_key(*family),
            "the family {family} is declared"
        );
    }
    assert_eq!(verbs, 51, "spec §3.3's 51 actions");
}

#[test]
fn every_schema_attribute_declares_its_source() {
    let schema = schema_json();
    let common = schema["commonTypes"]
        .as_object()
        .cloned()
        .unwrap_or_default();
    let mut sources = std::collections::BTreeMap::new();
    let mut missing = Vec::new();
    let mut note = |at: String, annotations: &serde_json::Value| match annotations
        .get("source")
        .and_then(|s| s.as_str())
    {
        Some(source) if source.split('|').all(|s| policy::SOURCES.contains(&s)) => {
            sources.insert(at, source.to_owned());
        }
        _ => missing.push(at),
    };
    let none = serde_json::Value::Null;
    for (name, entity) in schema["entityTypes"].as_object().expect("entity types") {
        let shape = &entity["shape"];
        let attributes = shape
            .get("attributes")
            .and_then(|a| a.as_object())
            .or_else(|| {
                shape
                    .get("name")
                    .and_then(|n| n.as_str())
                    .and_then(|n| common[n]["attributes"].as_object())
            });
        for (attribute, declared) in attributes.into_iter().flatten() {
            note(
                format!("{name}.{attribute}"),
                declared.get("annotations").unwrap_or(&none),
            );
        }
        if entity.get("tags").is_some() {
            note(
                format!("{name} tags"),
                entity.get("annotations").unwrap_or(&none),
            );
        }
    }
    let mut contexts = BTreeSet::new();
    for (action, declared) in schema["actions"].as_object().expect("actions") {
        let Some(context) = declared.get("appliesTo").and_then(|a| a.get("context")) else {
            continue;
        };
        let (name, record) = match context.get("type").and_then(|n| n.as_str()) {
            Some("Record") | None => (format!("{action}'s context"), context),
            Some(name) => (name.to_owned(), &common[name]),
        };
        if !contexts.insert(name.clone()) {
            continue;
        }
        for (field, declared) in record["attributes"]
            .as_object()
            .expect("a context is a record")
        {
            note(
                format!("{name}.{field}"),
                declared.get("annotations").unwrap_or(&none),
            );
        }
    }
    assert_eq!(missing, Vec::<String>::new(), "attributes with no source");
    assert!(
        sources.len() > 20,
        "the walk found the attributes: {sources:#?}"
    );

    // Spec §3.2's table, row by row.
    let source = |at: &str| sources.get(at).map(String::as_str);
    assert_eq!(source("Ctx.machine"), Some("sealed"));
    assert_eq!(
        source("Ctx.harness_mode"),
        Some("host-supplied"),
        "Q320e R6-1"
    );
    for field in ["sandbox", "attended", "depth"] {
        assert_eq!(
            source(&format!("Ctx.{field}")),
            Some("host-supplied"),
            "{field}"
        );
    }
    assert_eq!(source("StartCtx.gaps"), Some("host-supplied"));
    assert_eq!(source("GitCtx.force"), Some("host-supplied"));
    assert_eq!(source("Chat.workspace"), Some("host-supplied"));
    assert_eq!(source("Chat.persona"), Some("host-supplied"));
    assert_eq!(source("Path.workspace"), Some("committed"));
    assert_eq!(source("Vault tags"), Some("committed|sealed"));
    assert_eq!(source("Tool.declaration"), Some("committed|sealed"));
}

#[test]
fn the_shipped_default_ends_in_the_everything_else_permit() {
    assert!(policy::DEFAULT.starts_with("// purlis-policy 1\n"));
    let loaded = policy::load(&[File::shipped_default()]).expect("the shipped default loads");
    assert_eq!(
        loaded.rules().last().map(String::as_str),
        Some(policy::EVERYTHING_ELSE)
    );

    let last = policy::DEFAULT
        .trim_end()
        .rsplit(";\n")
        .next()
        .expect("a last rule");
    assert!(
        last.ends_with("@id(\"policy-everything-else\") @level(\"project\")\npermit (principal, action, resource);"),
        "an unconditional permit over everything: {last}"
    );

    let world =
        World::from_json(serde_json::from_str(&read(&fixtures().join("world.json"))).unwrap())
            .expect("the world loads");
    let unnamed = Question {
        chat: "web/builder",
        action: "install",
        resource: ("Skill", "tdd"),
        context: base_context(),
    };
    assert_eq!(
        loaded.decide(&unnamed, &world),
        Decision::Allow {
            permits: vec![policy::EVERYTHING_ELSE.to_owned()]
        }
    );
}

#[test]
fn a_newer_schema_version_than_this_build_reads_is_reported_as_broken() {
    let text = "// purlis-policy 2\n\
                @id(\"guards-merge-held\")\n\
                forbid (principal, action == Action::\"merge\", resource);\n";
    let problems = refused(with_project(text));
    assert_eq!(problems.len(), 1, "{problems:#?}");
    assert_eq!(
        (problems[0].file.as_str(), problems[0].line),
        ("policy.cedar", 1)
    );
    assert!(problems[0].what.contains("version 2"), "{}", problems[0]);
    assert!(problems[0].what.contains("broken"), "{}", problems[0]);

    let unversioned =
        "@id(\"guards-merge-held\")\nforbid (principal, action == Action::\"merge\", resource);\n";
    assert_eq!(refused(with_project(unversioned))[0].line, 1);
}

#[test]
fn the_prototype_sample_projects_validate_against_the_schema() {
    for project in ["solo", "team", "regulated"] {
        let text = read(
            &fixtures()
                .join("projects")
                .join(project)
                .join("policy.cedar"),
        );
        if let Err(problems) = with_project(&text) {
            panic!(
                "{project}: {}",
                problems
                    .iter()
                    .map(|p| format!("\n  {p}"))
                    .collect::<String>()
            );
        }
    }
}

#[test]
fn an_ask_with_no_permit_under_it_is_no_permit_and_an_error_is_a_deny() {
    let deny_by_default = policy::DEFAULT.replace(
        "@id(\"policy-everything-else\") @level(\"project\")\npermit (principal, action, resource);",
        "",
    );
    let world =
        World::from_json(serde_json::from_str(&read(&fixtures().join("world.json"))).unwrap())
            .unwrap();
    let merge = Question {
        chat: "web/builder",
        action: "merge",
        resource: ("Branch", "acme/web#main"),
        context: with(base_context(), "via", "".into()),
    };
    let shipped = policy::load(&[File::shipped_default()]).unwrap();
    assert_eq!(
        shipped.decide(&merge, &world),
        Decision::Ask {
            asks: vec!["guards-merge-hold".into()]
        }
    );
    let bare = policy::load(&[File {
        name: "default",
        text: &deny_by_default,
        place: Level::Project,
        shipped: true,
    }])
    .unwrap();
    assert_eq!(bare.decide(&merge, &world), Decision::NoPermit);

    let missing_field = Question {
        context: base_context(),
        ..merge
    };
    assert!(matches!(
        shipped.decide(&missing_field, &world),
        Decision::Unasked { .. }
    ));

    // A chat the world doesn't hold has no persona to read: the rule that reads it errs, and an
    // error is a deny naming that rule, never a skipped forbid (spec §6.3).
    let stranger_uses_a_token = Question {
        chat: "nobody/here",
        action: "use",
        resource: ("Secret", "builder/npm-token"),
        context: with(
            with(base_context(), "program", "npm".into()),
            "declared",
            true.into(),
        ),
    };
    assert_eq!(
        shipped.decide(&stranger_uses_a_token, &world),
        Decision::Erred {
            rules: vec!["vaults-other-persona".into()]
        }
    );
}

/// The context every action has, as the session host supplies it.
fn base_context() -> serde_json::Value {
    serde_json::json!({ "sandbox": "floor", "attended": true, "harness_mode": "default", "depth": 0 })
}

fn with(
    mut context: serde_json::Value,
    field: &str,
    value: serde_json::Value,
) -> serde_json::Value {
    context[field] = value;
    context
}

/// The median of `samples` runs of `f`, after a warm-up.
fn median(samples: usize, mut f: impl FnMut()) -> Duration {
    for _ in 0..samples / 10 {
        f();
    }
    let mut times: Vec<Duration> = (0..samples)
        .map(|_| {
            let at = Instant::now();
            f();
            at.elapsed()
        })
        .collect();
    times.sort();
    times[samples / 2]
}

/// **The bench** (spec §6's time budget; review round 1's method). One decision costs no more
/// than 0.25 ms p50 where one plain Cedar evaluation costs Q311's 94 µs: so its median is held
/// to `250 / 94` times the median of a baseline measured in the same run, on the same machine,
/// never to an absolute number a shared runner can't keep. The baseline is one evaluation by
/// Cedar's own authorizer over the same rules and world, of the same request built once, with
/// nothing of purlis's around it: what Q311's 94 µs measured. The decision is timed whole, the
/// request built from purlis's question included. The question is an ask, purlis's dearest path
/// (it asks Cedar twice).
#[test]
fn one_decision_within_budget_relative_to_baseline() {
    const BUDGET_PER_BASELINE: f64 = 250.0 / 94.0;
    const SAMPLES: usize = 400;

    let world_json: serde_json::Value =
        serde_json::from_str(&read(&fixtures().join("world.json"))).unwrap();
    let world = World::from_json(world_json.clone()).unwrap();
    let loaded = policy::load(&[File::shipped_default()]).unwrap();
    let push = Question {
        chat: "web/builder",
        action: "push",
        resource: ("Branch", "acme/web#feature-x"),
        context: serde_json::json!({
            "sandbox": "floor", "attended": true, "harness_mode": "default", "depth": 0,
            "force": true, "skip-hooks": false, "brokered": false, "amend": false,
        }),
    };
    assert_eq!(
        loaded.decide(&push, &world),
        Decision::Ask {
            asks: vec!["guards-git-force-push".into()]
        }
    );

    let schema = policy::schema();
    let rules = cedar_policy::PolicySet::from_str(policy::DEFAULT).unwrap();
    let entities = cedar_policy::Entities::from_json_value(world_json, Some(schema)).unwrap();
    let action = cedar_policy::EntityUid::from_str("Action::\"push\"").unwrap();
    let authorizer = cedar_policy::Authorizer::new();
    let request = cedar_policy::Request::new(
        cedar_policy::EntityUid::from_str("Chat::\"web/builder\"").unwrap(),
        action.clone(),
        cedar_policy::EntityUid::from_str("Branch::\"acme/web#feature-x\"").unwrap(),
        cedar_policy::Context::from_json_value(push.context.clone(), Some((schema, &action)))
            .unwrap(),
        Some(schema),
    )
    .unwrap();
    let baseline = median(SAMPLES, || {
        std::hint::black_box(authorizer.is_authorized(&request, &rules, &entities));
    });
    let decision = median(SAMPLES, || {
        std::hint::black_box(loaded.decide(&push, &world));
    });
    let ratio = decision.as_secs_f64() / baseline.as_secs_f64();
    println!(
        "one decision: {decision:?} p50; Cedar alone: {baseline:?} p50; {ratio:.2}× (budget {BUDGET_PER_BASELINE:.2}×)"
    );
    assert!(
        ratio <= BUDGET_PER_BASELINE,
        "one decision took {decision:?} p50, {ratio:.2}× Cedar's own {baseline:?}: over the budget of {BUDGET_PER_BASELINE:.2}×"
    );
}
