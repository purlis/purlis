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
use purlis_core::policy::levels::Chat;
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
    let mut request = connect(host);
    request.context["port"] = port.into();
    let found = purlis_core::policy::organisation::in_force();
    let root = tempfile::tempdir().expect("a folder");
    let loaded = purlis_core::policy::levels::load(&purlis_core::policy::levels::Sources {
        root: root.path(),
        workspaces: &[],
        personas: &[],
        organisation: found.as_ref().map(|f| f.as_sources()),
        accepted: None,
    });
    let world = purlis_core::policy::World::from_json(serde_json::json!([])).expect("a world");
    [
        decide::decide_locally(&request),
        decide::decide(&loaded, &request, &world),
    ]
}

/// Whether `said` was refused by the Organisation's rule `id`.
fn refused_by(said: &decide::Decision, id: &str) -> bool {
    said.outcome == Outcome::Deny && said.deciding.iter().any(|d| d.id == id)
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
    organisation(r#"{"sandbox": {"presets": ["forge"]}}"#);
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
