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
}
