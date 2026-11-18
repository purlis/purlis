// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The Organisation's folder holds outside every plane** (M75 slice 06, #7014; spec §6, review
//! round 4, SEC4-SF1, and §14.1).
//!
//! A process with no plane and no host to ask is judged by the shipped default under `strict`,
//! where it runs. The Organisation's files are layered on that, so an admin's forbid and lock hold
//! there too. In a test build the Organisation comes from the `set_for_this_test` seam alone.

use std::path::Path;

use purlis_core::policy::decide::{self, Outcome, Request};
use purlis_core::policy::levels::{Accepted, Chat};
use purlis_core::sandbox::policy::{Locks, set_for_this_test};

fn connect(host: &'static str) -> Request<'static> {
    Request {
        chat: "3",
        of: Chat {
            workspace: None,
            persona: None,
        },
        action: "connect",
        resource: ("Host", host),
        context: serde_json::json!({
            "sandbox": "floor", "attended": true, "harness_mode": "default", "depth": 0,
            "port": 443, "via": "hook"
        }),
    }
}

fn organisation(json: &str) {
    set_for_this_test(Locks::parse(json, Path::new("/etc/purlis/policy.json")));
}

#[test]
fn a_plane_less_process_runs_local_strict_with_the_organisations_files() {
    purlis_core::unsteered!();
    let evil = connect("evil.example");
    // With no Organisation, nothing names the host.
    let alone = decide::decide_locally(&evil);
    assert!(
        !alone
            .deciding
            .iter()
            .any(|d| d.id.starts_with("network-organisation")),
        "{alone:?}"
    );
    // An admin's forbid refuses it outside every plane, naming the Organisation's rule.
    organisation(r#"{"owner": "Platform team", "sandbox": {"never-hosts": ["evil.example"]}}"#);
    let held = decide::decide_locally(&evil);
    assert_eq!(held.outcome, Outcome::Deny, "{held:?}");
    assert!(
        held.deciding
            .iter()
            .any(|d| d.id == "network-organisation-never-0" && d.owner == "Platform team"),
        "{held:?}"
    );
    // A host it does not name is not refused by it.
    let other = decide::decide_locally(&connect("fine.example"));
    assert!(
        !other
            .deciding
            .iter()
            .any(|d| d.id.starts_with("network-organisation")),
        "{other:?}"
    );
    // And the shipped default still holds beside it.
    let merge = decide::decide_locally(&Request {
        action: "merge",
        resource: ("Branch", "acme/web#main"),
        context: serde_json::json!({
            "sandbox": "floor", "attended": true, "harness_mode": "default", "depth": 0,
            "via": "hook"
        }),
        ..connect("x")
    });
    assert_eq!(merge.outcome, Outcome::Ask, "{merge:?}");
}

#[test]
fn a_plane_less_process_under_a_refused_organisation_reads_every_lock() {
    purlis_core::unsteered!();
    // A policy.json that says anything unknown is refused: strict plus every lock.
    organisation(r#"{"sandbox": {"made-up": true}}"#);
    let held = decide::decide_locally(&connect("fine.example"));
    assert!(held.lock.is_some(), "a family is locked: {held:?}");
    // And so is every other family, not only the one asked: a lock on each.
    for (action, kind, id) in [
        ("connect", "Host", "fine.example"),
        ("dispatch-to", "Persona", "reviewer"),
        ("use", "Secret", "vault/key"),
        ("run", "Tool", "lint"),
        ("install", "Skill", "web/x"),
        ("start", "Harness", "claude-code"),
        ("merge", "Branch", "acme/web#main"),
    ] {
        let said = decide::decide_locally(&Request {
            action,
            resource: (kind, id),
            context: serde_json::json!({}),
            ..connect("x")
        });
        assert!(said.lock.is_some(), "{action} is locked: {said:?}");
    }
}

/// `host` at `port`, asked of the Organisation as a process outside every plane would, and as a
/// chat in a plane would, with the world each has (none known).
fn connect_both_ways(host: &'static str, port: i64) -> [decide::Decision; 2] {
    connect_in_project(host, port, None)
}

/// As [`connect_both_ways`], in a plane whose project file is `project` (the copy last accepted).
fn connect_in_project(
    host: &'static str,
    port: i64,
    project: Option<&str>,
) -> [decide::Decision; 2] {
    let mut request = connect(host);
    request.of = Chat {
        workspace: Some("web"),
        persona: Some("builder"),
    };
    request.context["port"] = port.into();
    let found = purlis_core::policy::organisation::in_force();
    let root = tempfile::tempdir().expect("a folder");
    let loaded = purlis_core::policy::levels::load(&purlis_core::policy::levels::Sources {
        root: root.path(),
        workspaces: &["web"],
        personas: &["builder"],
        organisation: found.as_ref().map(|f| f.as_sources()),
        accepted: project.map(|settings| Accepted {
            settings: Some(settings),
            rules: None,
        }),
    });
    let world = purlis_core::policy::World::from_json(serde_json::json!([])).expect("a world");
    [
        decide::decide_locally(&request),
        decide::decide(&loaded, &request, &world),
    ]
}

/// Whether `said` was refused by the Organisation's rule `id`.
fn refused_by(said: &decide::Decision, id: &str) -> bool {
    said.outcome == Outcome::Deny
        && said.after_mode == Outcome::Deny
        && said.deciding.iter().any(|d| d.id == id)
}

#[test]
fn a_wildcard_never_host_refuses_every_name_under_it_and_only_those() {
    purlis_core::unsteered!();
    organisation(r#"{"sandbox": {"never-hosts": ["*.evil.example"]}}"#);
    let never = "network-organisation-never-0";
    for host in ["a.evil.example", "b.a.evil.example", "A.Evil.Example"] {
        for said in connect_both_ways(host, 443) {
            assert!(refused_by(&said, never), "{host}: {said:?}");
        }
    }
    // The domain itself, a name that merely ends in it, and a sibling are not under it.
    for host in [
        "evil.example",
        "notevil.example",
        "a.evil.example.org",
        "fine.example",
    ] {
        for said in connect_both_ways(host, 443) {
            assert!(!refused_by(&said, never), "{host}: {said:?}");
        }
    }
}

#[test]
fn a_ported_never_host_refuses_that_port_alone() {
    purlis_core::unsteered!();
    organisation(r#"{"sandbox": {"never-hosts": ["evil.example:8443"]}}"#);
    let never = "network-organisation-never-0";
    for said in connect_both_ways("evil.example", 8443) {
        assert!(refused_by(&said, never), "{said:?}");
    }
    for said in connect_both_ways("evil.example", 443) {
        assert!(!refused_by(&said, never), "{said:?}");
    }
    for said in connect_both_ways("other.example", 8443) {
        assert!(!refused_by(&said, never), "{said:?}");
    }
}

#[test]
fn a_wildcard_ported_never_host_refuses_names_under_it_at_that_port_alone() {
    purlis_core::unsteered!();
    organisation(r#"{"sandbox": {"never-hosts": ["*.evil.example:8443"]}}"#);
    let never = "network-organisation-never-0";
    for said in connect_both_ways("a.evil.example", 8443) {
        assert!(refused_by(&said, never), "{said:?}");
    }
    for (host, port) in [
        ("a.evil.example", 443),
        ("evil.example", 8443),
        ("x.example", 8443),
    ] {
        for said in connect_both_ways(host, port) {
            assert!(!refused_by(&said, never), "{host}:{port}: {said:?}");
        }
    }
}

#[test]
fn a_fixed_host_list_with_a_wildcard_or_a_port_forbids_only_what_it_does_not_cover() {
    purlis_core::unsteered!();
    let fixed = "network-organisation-fixed-hosts";
    organisation(r#"{"sandbox": {"hosts": ["*.corp.example", "api.example.com:8443"]}}"#);
    for (host, port, forbidden) in [
        ("a.corp.example", 443, false),
        ("b.a.corp.example", 443, false),
        ("corp.example", 443, true),
        ("api.example.com", 8443, false),
        ("api.example.com", 443, true),
        ("other.example", 443, true),
    ] {
        for said in connect_both_ways(host, port) {
            assert_eq!(
                refused_by(&said, fixed),
                forbidden,
                "{host}:{port}: {said:?}"
            );
        }
    }
}

#[test]
fn a_preset_that_lists_a_wildcard_host_does_not_forbid_the_names_under_it() {
    purlis_core::unsteered!();
    // The forge preset lists `*.githubusercontent.com`.
    organisation(r#"{"sandbox": {"presets": ["forge"], "hosts": ["extra.example"]}}"#);
    for said in connect_both_ways("raw.githubusercontent.com", 443) {
        assert!(
            !refused_by(&said, "network-organisation-fixed-hosts"),
            "{said:?}"
        );
    }
    for (n, said) in connect_both_ways("evil.example", 443)
        .into_iter()
        .enumerate()
    {
        assert!(
            refused_by(&said, "network-organisation-fixed-hosts"),
            "{n}: {said:?}"
        );
    }
}

#[test]
fn a_false_in_policy_json_is_a_hard_forbid_never_an_ask() {
    purlis_core::unsteered!();
    let context = |extra: serde_json::Value| {
        let mut base = serde_json::json!({
            "sandbox": "floor", "attended": true, "harness_mode": "default", "depth": 0
        });
        base.as_object_mut()
            .expect("an object")
            .extend(extra.as_object().expect("an object").clone());
        base
    };
    let chat = |action, resource, context| Request {
        chat: "web/builder",
        of: Chat {
            workspace: Some("web"),
            persona: Some("builder"),
        },
        action,
        resource,
        context,
    };
    let world = purlis_core::policy::World::from_json(
        serde_json::from_str(
            &std::fs::read_to_string(
                Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/policy/world.json"),
            )
            .expect("the world fixture"),
        )
        .expect("JSON"),
    )
    .expect("the world loads");
    // key, the request it forbids, and the rule that says so.
    let cases = [
        (
            r#"{"dispatch": {"allow": false}}"#,
            chat(
                "dispatch-to",
                ("Persona", "reviewer"),
                context(serde_json::json!({
                    "kind": "dispatch", "profile": "default", "subagent": false
                })),
            ),
            "dispatch-organisation-forbidden",
        ),
        (
            r#"{"sandbox": {"opt-out": false}}"#,
            chat(
                "start",
                ("Harness", "claude-code"),
                context(serde_json::json!({
                    "sandbox": "off", "profile": "default", "gaps": []
                })),
            ),
            "trust-organisation-no-opt-out",
        ),
        (
            r#"{"sandbox": {"persona-tools": false}}"#,
            chat("run", ("Tool", "lint"), context(serde_json::json!({}))),
            "tools-organisation-no-persona-tools",
        ),
        (
            r#"{"sandbox": {"vault-grants": false}}"#,
            chat(
                "use",
                ("Secret", "ops/key"),
                context(serde_json::json!({ "program": "x", "declared": true })),
            ),
            "vaults-organisation-no-vault-grants",
        ),
        (
            r#"{"sandbox": {"live-asks": false}}"#,
            chat(
                "connect",
                ("Host", "unlisted.example"),
                context(serde_json::json!({ "port": 443, "via": "hook" })),
            ),
            "network-organisation-no-live-asks",
        ),
    ];
    for (json, request, id) in cases {
        organisation(json);
        let found = purlis_core::policy::organisation::in_force().expect("an Organisation");
        let loaded =
            purlis_core::policy::levels::Loaded::shipped_strict_under(Some(&found.as_sources()));
        let rule = loaded
            .policy
            .rule(id)
            .unwrap_or_else(|| panic!("{json}: no {id}"));
        assert!(!rule.ask, "{id} asks");
        let said = decide::decide(&loaded, &request, &world);
        assert!(refused_by(&said, id), "{json}: {said:?}");
        assert!(
            said.lock.is_some(),
            "{json}: its family is locked: {said:?}"
        );
    }
}

#[test]
fn a_locked_dispatch_pair_refuses_that_pair_alone() {
    purlis_core::unsteered!();
    organisation(r#"{"dispatch": {"locked": [{"from": "builder", "to": "reviewer"}]}}"#);
    let found = purlis_core::policy::organisation::in_force().expect("an Organisation");
    let loaded =
        purlis_core::policy::levels::Loaded::shipped_strict_under(Some(&found.as_sources()));
    let world = purlis_core::policy::World::from_json(
        serde_json::from_str(
            &std::fs::read_to_string(
                Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/policy/world.json"),
            )
            .expect("the world fixture"),
        )
        .expect("JSON"),
    )
    .expect("the world loads");
    let dispatch = |chat, to| {
        decide::decide(
            &loaded,
            &Request {
                chat,
                of: Chat {
                    workspace: None,
                    persona: None,
                },
                action: "dispatch-to",
                resource: ("Persona", to),
                context: serde_json::json!({
                    "sandbox": "floor", "attended": true, "harness_mode": "default",
                    "depth": 0, "kind": "dispatch", "profile": "default", "subagent": false
                }),
            },
            &world,
        )
    };
    let pair = "dispatch-organisation-pair-0";
    assert!(refused_by(&dispatch("web/builder", "reviewer"), pair));
    // Another target, or another persona asking, is not that pair.
    assert!(!refused_by(&dispatch("web/builder", "deployer"), pair));
    assert!(!refused_by(&dispatch("infra/deployer", "reviewer"), pair));
}

/// The project files that would loosen the `network` family or one rule: no policy at all, the
/// family off, the shipped `watch` template, and a rule turned off by its id.
const LOOSE_PROJECTS: [Option<&str>; 6] = [
    None,
    Some("[policy]\nnetwork = \"off\"\n"),
    Some("[policy]\ntemplate = \"watch\"\n"),
    Some("[policy.rules]\n\"network-organisation-never-0\" = \"off\"\n"),
    Some("[policy.workspace.web]\nnetwork = \"off\"\n"),
    Some("[policy.persona.builder]\nnetwork = \"off\"\n"),
];

#[test]
fn an_organisation_never_host_is_enforced_whatever_the_projects_mode() {
    purlis_core::unsteered!();
    let never = "network-organisation-never-0";
    for json in [
        r#"{"sandbox": {"never-hosts": ["*.evil.example"]}}"#,
        r#"{"sandbox": {"never-hosts": ["*.evil.example"], "hosts": ["fine.example"]}}"#,
    ] {
        organisation(json);
        for project in LOOSE_PROJECTS {
            for said in connect_in_project("a.evil.example", 443, project) {
                assert!(
                    refused_by(&said, never),
                    "{json} under {project:?}: {said:?}"
                );
                assert_eq!(said.after_mode, Outcome::Deny, "{said:?}");
            }
        }
    }
}

#[test]
fn an_organisation_fixed_host_list_is_enforced_whatever_the_projects_mode() {
    purlis_core::unsteered!();
    organisation(r#"{"sandbox": {"hosts": ["fine.example"]}}"#);
    for project in LOOSE_PROJECTS {
        for said in connect_in_project("evil.example", 443, project) {
            assert!(
                refused_by(&said, "network-organisation-fixed-hosts"),
                "under {project:?}: {said:?}"
            );
        }
    }
}

#[test]
fn an_exact_never_host_meets_the_host_however_it_is_spelt() {
    purlis_core::unsteered!();
    organisation(r#"{"sandbox": {"never-hosts": ["evil.example"]}}"#);
    let never = "network-organisation-never-0";
    for host in [
        "evil.example",
        "Evil.Example",
        "evil.example.",
        "EVIL.EXAMPLE.",
    ] {
        for said in connect_both_ways(host, 443) {
            assert!(refused_by(&said, never), "{host}: {said:?}");
        }
    }
}

#[test]
fn a_path_shaped_or_ported_host_id_is_unasked_and_never_in_a_wildcards_set() {
    purlis_core::unsteered!();
    organisation(r#"{"sandbox": {"hosts": ["*.corp.example"]}}"#);
    for host in [
        "evil.com/x.corp.example",
        "evil.com?x.corp.example",
        "a.corp.example:443",
        "*.corp.example",
        "",
    ] {
        for said in connect_both_ways(host, 443) {
            assert_eq!(said.outcome, Outcome::Deny, "{host}: {said:?}");
            assert_eq!(said.after_mode, Outcome::Deny, "{host}: {said:?}");
            assert!(said.unasked.is_some(), "{host}: {said:?}");
        }
    }
}

#[test]
fn an_ipv6_never_host_meets_the_address_however_it_is_spelt() {
    purlis_core::unsteered!();
    organisation(r#"{"sandbox": {"never-hosts": ["[fd00::1]"]}}"#);
    let never = "network-organisation-never-0";
    for host in [
        "[fd00::1]",
        "[FD00::1]",
        "[fd00:0:0:0:0:0:0:1]",
        "[fd00:0::1]",
    ] {
        for said in connect_both_ways(host, 443) {
            assert!(refused_by(&said, never), "{host}: {said:?}");
        }
    }
    for said in connect_both_ways("[fd00::2]", 443) {
        assert!(!refused_by(&said, never), "{said:?}");
    }
}

#[test]
fn an_ipv6_never_host_with_a_port_refuses_that_port_alone() {
    purlis_core::unsteered!();
    organisation(r#"{"sandbox": {"never-hosts": ["[fd00::1]:8443", "trailing.example.:8443"]}}"#);
    for said in connect_both_ways("[fd00::1]", 8443) {
        assert!(
            refused_by(&said, "network-organisation-never-0"),
            "{said:?}"
        );
    }
    for said in connect_both_ways("[fd00::1]", 443) {
        assert!(
            !refused_by(&said, "network-organisation-never-0"),
            "{said:?}"
        );
    }
    // A trailing dot is dropped as it is read, and a request's host is read as the policy's is.
    for host in ["trailing.example", "trailing.example."] {
        for said in connect_both_ways(host, 8443) {
            assert!(
                refused_by(&said, "network-organisation-never-1"),
                "{host}: {said:?}"
            );
        }
    }
}

#[test]
fn an_empty_host_list_forbids_every_host_and_an_empty_preset_list_none() {
    purlis_core::unsteered!();
    let fixed = "network-organisation-fixed-hosts";
    organisation(r#"{"sandbox": {"hosts": []}}"#);
    for said in connect_both_ways("fine.example", 443) {
        assert!(refused_by(&said, fixed), "{said:?}");
    }
    // `presets` alone restricts no host, as the legacy reader and spec §13.6 read it.
    for json in [
        r#"{"sandbox": {"presets": []}}"#,
        r#"{"sandbox": {"presets": ["forge"]}}"#,
    ] {
        organisation(json);
        for said in connect_both_ways("fine.example", 443) {
            assert!(
                !said
                    .deciding
                    .iter()
                    .any(|d| d.id.starts_with("network-organisation")),
                "{json}: {said:?}"
            );
        }
    }
}

#[test]
fn a_project_cannot_soften_a_legacy_false_key() {
    purlis_core::unsteered!();
    organisation(r#"{"sandbox": {"opt-out": false}}"#);
    let id = "trust-organisation-no-opt-out";
    let found = purlis_core::policy::organisation::in_force().expect("an Organisation");
    let world = purlis_core::policy::World::from_json(serde_json::json!([])).expect("a world");
    let start = Request {
        action: "start",
        resource: ("Harness", "claude-code"),
        context: serde_json::json!({
            "sandbox": "off", "attended": true, "harness_mode": "default", "depth": 0,
            "profile": "default", "gaps": []
        }),
        ..connect("x")
    };
    let projects = [
        ("[policy]\nsandbox = \"off\"\ntrust = \"off\"\n", None),
        (
            "[policy.rules]\n\"trust-organisation-no-opt-out\" = \"off\"\n",
            None,
        ),
        (
            "[policy]\nsandbox = \"off\"\n",
            Some(
                "// purlis-policy 1\n@id(\"trust-organisation-no-opt-out\")\n\
                 permit (principal, action == Action::\"start\", resource);\n",
            ),
        ),
    ];
    for (settings, rules) in projects {
        let root = tempfile::tempdir().expect("a folder");
        let loaded = purlis_core::policy::levels::load(&purlis_core::policy::levels::Sources {
            root: root.path(),
            workspaces: &[],
            personas: &[],
            organisation: Some(found.as_sources()),
            accepted: Some(Accepted {
                settings: Some(settings),
                rules,
            }),
        });
        let said = decide::decide(&loaded, &start, &world);
        assert!(refused_by(&said, id), "{settings}: {said:?}");
    }
}

#[test]
fn a_project_cannot_name_a_set_like_a_wildcard_host() {
    purlis_core::unsteered!();
    organisation(r#"{"sandbox": {"hosts": ["*.corp.example"]}}"#);
    let found = purlis_core::policy::organisation::in_force().expect("an Organisation");
    let root = tempfile::tempdir().expect("a folder");
    let loaded = purlis_core::policy::levels::load(&purlis_core::policy::levels::Sources {
        root: root.path(),
        workspaces: &[],
        personas: &[],
        organisation: Some(found.as_sources()),
        accepted: Some(Accepted {
            settings: Some("[policy.sets]\n\"*.corp.example\" = [\"evil.com\"]\n"),
            rules: None,
        }),
    });
    assert!(
        loaded
            .notices
            .iter()
            .any(|n| n.what.contains("named like a wildcard host")),
        "{:?}",
        loaded.notices
    );
    assert!(!loaded.sets.contains_key("*.corp.example"));
}

#[test]
fn an_organisations_limits_are_kept_for_slice_12() {
    purlis_core::unsteered!();
    organisation(r#"{"dispatch": {"depth": 2}}"#);
    let found = purlis_core::policy::organisation::in_force().expect("an Organisation");
    let root = tempfile::tempdir().expect("a folder");
    let loaded = purlis_core::policy::levels::load(&purlis_core::policy::levels::Sources {
        root: root.path(),
        workspaces: &[],
        personas: &[],
        organisation: Some(found.as_sources()),
        accepted: None,
    });
    assert_eq!(loaded.organisation_limits.get("dispatch.depth"), Some(&2));
}

/// A plane's decision on `request` where no family lock applies: the Organisation's locks are
/// cleared, so what holds the forbid is its mode alone.
fn decide_with_no_lock(
    organisation_rules: Option<&str>,
    json: &str,
    project: Option<&str>,
    request: &Request<'_>,
) -> decide::Decision {
    organisation(json);
    let found = purlis_core::policy::organisation::in_force().expect("an Organisation");
    let mut sources = found.as_sources();
    if let Some(rules) = organisation_rules {
        sources.rules = Some(rules);
    }
    let root = tempfile::tempdir().expect("a folder");
    let mut loaded = purlis_core::policy::levels::load(&purlis_core::policy::levels::Sources {
        root: root.path(),
        workspaces: &["web"],
        personas: &["builder"],
        organisation: Some(sources),
        accepted: project.map(|settings| Accepted {
            settings: Some(settings),
            rules: None,
        }),
    });
    assert!(
        !loaded.notices.iter().any(|n| n.what.contains("is broken")),
        "{:?}",
        loaded.notices
    );
    loaded.organisation_locks = purlis_core::policy::levels::Locks::default();
    loaded.project_locks = purlis_core::policy::levels::Locks::default();
    let world = purlis_core::policy::World::from_json(serde_json::json!([])).expect("a world");
    // Asked as the chat of workspace `web` and persona `builder`, so a table the project writes
    // for either applies (round 5: the cases that wrote them in the plural tested a strict project).
    let asked = Request {
        of: Chat {
            workspace: Some("web"),
            persona: Some("builder"),
        },
        ..request.clone()
    };
    // The project is the loose one it says: no accepted-copy notice, and the family is off.
    if let Some(text) = project {
        assert!(
            !loaded
                .notices
                .iter()
                .any(|n| n.file.contains("the copy you last accepted")),
            "{text}: the project must read, not fall back to strict: {:?}",
            loaded.notices
        );
        if text.contains("network = \"off\"") {
            assert_eq!(
                loaded.mode_for("network", asked.of),
                purlis_core::enforcement::Mode::Off,
                "{text}: the project is not strict"
            );
        }
    }
    decide::decide(&loaded, &asked, &world)
}

const NATIVE: &str = "// purlis-policy 1\n@id(\"network-corp-no-evil\") @level(\"organisation\")\n\
                      forbid (principal, action == Action::\"connect\", resource == Host::\"evil.example\");\n";

#[test]
fn a_native_organisation_forbid_is_enforced_whatever_the_projects_mode() {
    purlis_core::unsteered!();
    for project in LOOSE_PROJECTS.into_iter().chain([
        // A project's `[policy.rules]` naming the forbid's own id has no effect (round 5).
        Some("[policy.rules]\n\"network-corp-no-evil\" = \"off\"\n"),
        Some("[policy.rules]\n\"network-corp-no-evil\" = \"log\"\n"),
    ]) {
        let said = decide_with_no_lock(Some(NATIVE), "{}", project, &connect("evil.example"));
        assert!(
            refused_by(&said, "network-corp-no-evil"),
            "under {project:?}: {said:?}"
        );
    }
}

#[test]
fn the_organisations_own_rules_table_can_lower_its_forbid() {
    purlis_core::unsteered!();
    organisation("{}");
    let world = purlis_core::policy::World::from_json(serde_json::json!([])).expect("a world");
    let ask = connect("evil.example");
    let decide_under = |settings: Option<&str>| {
        let organisation = purlis_core::policy::levels::Organisation {
            folder: "/etc/purlis",
            owner: "Platform team",
            rules: Some(NATIVE),
            settings,
            json: None,
            failed: None,
        };
        let root = tempfile::tempdir().expect("a folder");
        let loaded = purlis_core::policy::levels::load(&purlis_core::policy::levels::Sources {
            root: root.path(),
            workspaces: &[],
            personas: &[],
            organisation: Some(organisation),
            accepted: None,
        });
        assert!(
            !loaded.notices.iter().any(|n| n.what.contains("is broken")),
            "{:?}",
            loaded.notices
        );
        decide::decide(&loaded, &ask, &world)
    };
    // Untouched, the forbid is enforced; the Organisation's own table lowers it, and only that.
    assert!(refused_by(&decide_under(None), "network-corp-no-evil"));
    let off = decide_under(Some("[policy.rules]\n\"network-corp-no-evil\" = \"off\"\n"));
    assert!(
        !off.deciding.iter().any(|d| d.id == "network-corp-no-evil"),
        "{off:?}"
    );
    let log = decide_under(Some("[policy.rules]\n\"network-corp-no-evil\" = \"log\"\n"));
    assert_ne!(log.after_mode, Outcome::Deny, "{log:?}");
}

#[test]
fn a_translated_forbid_holds_with_no_lock_at_all() {
    purlis_core::unsteered!();
    // Without the family lock, the forbid's own mode is all that holds it.
    for (json, host, id) in [
        (
            r#"{"sandbox": {"never-hosts": ["evil.example"]}}"#,
            "evil.example",
            "network-organisation-never-0",
        ),
        (
            r#"{"sandbox": {"hosts": ["fine.example"]}}"#,
            "evil.example",
            "network-organisation-fixed-hosts",
        ),
    ] {
        for project in LOOSE_PROJECTS {
            let said = decide_with_no_lock(None, json, project, &connect(host));
            assert!(refused_by(&said, id), "{json} under {project:?}: {said:?}");
        }
    }
}

#[test]
fn presets_only_never_hosts_only_and_locked_pairs_lock_their_families() {
    purlis_core::unsteered!();
    for (json, family) in [
        (r#"{"sandbox": {"presets": ["forge"]}}"#, "network"),
        (
            r#"{"sandbox": {"never-hosts": ["evil.example"]}}"#,
            "network",
        ),
        (
            r#"{"dispatch": {"locked": [{"from": "builder", "to": "reviewer"}]}}"#,
            "dispatch",
        ),
    ] {
        organisation(json);
        let found = purlis_core::policy::organisation::in_force().expect("an Organisation");
        let root = tempfile::tempdir().expect("a folder");
        let loaded = purlis_core::policy::levels::load(&purlis_core::policy::levels::Sources {
            root: root.path(),
            workspaces: &[],
            personas: &[],
            organisation: Some(found.as_sources()),
            accepted: Some(Accepted {
                settings: Some("[policy]\ntemplate = \"open\"\n"),
                rules: None,
            }),
        });
        assert!(
            loaded.organisation_locks.families.contains(family),
            "{json} locks {family}"
        );
        assert_eq!(
            loaded.mode_for(
                family,
                Chat {
                    workspace: None,
                    persona: None
                }
            ),
            purlis_core::enforcement::Mode::Enforce,
            "{json}"
        );
    }
}

#[test]
fn a_locked_dispatch_pair_holds_in_a_plane_under_a_loose_project() {
    purlis_core::unsteered!();
    organisation(r#"{"dispatch": {"locked": [{"from": "builder", "to": "reviewer"}]}}"#);
    let found = purlis_core::policy::organisation::in_force().expect("an Organisation");
    let world = purlis_core::policy::World::from_json(
        serde_json::from_str(
            &std::fs::read_to_string(
                Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/policy/world.json"),
            )
            .expect("the world fixture"),
        )
        .expect("JSON"),
    )
    .expect("the world loads");
    let ask = Request {
        chat: "web/builder",
        of: Chat {
            workspace: Some("web"),
            persona: Some("builder"),
        },
        action: "dispatch-to",
        resource: ("Persona", "reviewer"),
        context: serde_json::json!({
            "sandbox": "floor", "attended": true, "harness_mode": "default", "depth": 0,
            "kind": "dispatch", "profile": "default", "subagent": false
        }),
    };
    for (project, expected) in [
        (None, None),
        (
            Some("[policy]\ndispatch = \"off\"\n"),
            Some(purlis_core::enforcement::Mode::Off),
        ),
        (
            Some("[policy.workspace.web]\ndispatch = \"off\"\n"),
            Some(purlis_core::enforcement::Mode::Off),
        ),
        (
            Some("[policy.persona.builder]\ndispatch = \"log\"\n"),
            Some(purlis_core::enforcement::Mode::Log),
        ),
        (
            Some("[policy.rules]\n\"dispatch-organisation-pair-0\" = \"off\"\n"),
            None,
        ),
    ] {
        let root = tempfile::tempdir().expect("a folder");
        let mut loaded = purlis_core::policy::levels::load(&purlis_core::policy::levels::Sources {
            root: root.path(),
            workspaces: &["web"],
            personas: &["builder"],
            organisation: Some(found.as_sources()),
            accepted: project.map(|settings| Accepted {
                settings: Some(settings),
                rules: None,
            }),
        });
        // The project reads as written (no accepted-copy fallback to strict), and the family
        // runs as it says, so the case is a loose project and not a strict one (round 5).
        assert!(
            !loaded
                .notices
                .iter()
                .any(|n| n.file.contains("the copy you last accepted")),
            "under {project:?}: {:?}",
            loaded.notices
        );
        if let Some(mode) = expected {
            // The Organisation's family lock is lifted for the question, as in the other cases.
            let held = std::mem::take(&mut loaded.organisation_locks);
            assert_eq!(
                loaded.mode_for("dispatch", ask.of),
                mode,
                "under {project:?}: the project is not strict"
            );
            loaded.organisation_locks = held;
        }
        let said = decide::decide(&loaded, &ask, &world);
        assert!(
            refused_by(&said, "dispatch-organisation-pair-0"),
            "under {project:?}: {said:?}"
        );
    }
}

#[test]
fn a_host_set_member_that_is_no_bare_host_breaks_the_file() {
    purlis_core::unsteered!();
    organisation("{}");
    let found = purlis_core::policy::organisation::in_force().expect("an Organisation");
    for set in ["known-hosts", "production-hosts"] {
        for member in [
            "evil.example:8443",
            "*.evil.example",
            "not a host",
            "evil.com/x",
        ] {
            let text = format!("[policy.sets]\n{set} = [\"{member}\"]\n");
            let root = tempfile::tempdir().expect("a folder");
            let sources = |organisation, accepted| purlis_core::policy::levels::Sources {
                root: root.path(),
                workspaces: &[],
                personas: &[],
                organisation,
                accepted,
            };
            // The project's copy: the file reads broken, and the Notice names the set.
            let loaded = purlis_core::policy::levels::load(&sources(
                Some(found.as_sources()),
                Some(Accepted {
                    settings: Some(&text),
                    rules: None,
                }),
            ));
            assert!(
                loaded.notices.iter().any(|n| n.what.contains(set)),
                "{set} {member}: {:?}",
                loaded.notices
            );
            // The Organisation's own policy.toml: it reads strict plus every lock.
            let organisation = purlis_core::policy::levels::Organisation {
                folder: "/etc/purlis",
                owner: "Platform team",
                rules: None,
                settings: Some(&text),
                json: None,
                failed: None,
            };
            let loaded = purlis_core::policy::levels::load(&sources(Some(organisation), None));
            assert!(
                loaded
                    .notices
                    .iter()
                    .any(|n| n.what.contains(set)
                        && n.what.contains("reads as strict with every lock")),
                "{set} {member}: {:?}",
                loaded.notices
            );
            for family in purlis_core::policy::FAMILIES {
                assert!(
                    loaded.organisation_locks.families.contains(family),
                    "{set} {member}: {family} is locked"
                );
            }
            assert_eq!(
                loaded.sandbox_at_least,
                Some(purlis_core::enforcement::SandboxLevel::Full)
            );
        }
    }
}
