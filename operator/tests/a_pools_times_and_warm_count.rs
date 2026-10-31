// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A Pool's times and warm count** (#3737, the person's ruling of 2026-10-29): `idleAfter`,
//! `deleteSuspendedAfter`, `noticeBeforeDelete`, `maxAge` and `warmReplicas`, each with the
//! default the person ruled, spelled as `suspendAfter` is, and refused by the API server where
//! purlis takes no such value. The warm count is the operator's own: a warm pool keeps
//! `warmReplicas` runners warm, none while its Pool is stopped.

use purlis_cluster_operator::upstream;
use purlis_cluster_operator::{Claim, ClaimSpec, Pool, PoolSpec, warm_wanted};
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;

mod common;

const IMAGE: &str = "ghcr.io/purlis/purlis-runner@sha256:1111111111111111111111111111111111111111111111111111111111111111";

fn pool_from(spec: Value) -> Pool {
    serde_json::from_value(json!({
        "apiVersion": "runners.purlis.dev/v1alpha1",
        "kind": "Pool",
        "metadata": { "name": "team" },
        "spec": spec,
    }))
    .expect("a pool")
}

#[test]
fn a_pool_that_names_none_of_them_takes_the_ruled_defaults() {
    let pool = pool_from(json!({ "image": IMAGE }));
    let spec = &pool.spec;
    assert_eq!(spec.idle_after, "30m");
    assert_eq!(spec.delete_suspended_after, "720h");
    assert_eq!(spec.notice_before_delete, "168h");
    assert_eq!(spec.max_age, "336h");
    assert_eq!(spec.warm_replicas, 0);
}

#[test]
fn a_pool_reads_the_times_it_names() {
    let pool = pool_from(json!({
        "image": IMAGE,
        "idleAfter": "90s",
        "deleteSuspendedAfter": "48h",
        "noticeBeforeDelete": "12h",
        "maxAge": "7200m",
    }));
    let spec = &pool.spec;
    assert_eq!(spec.idle_after(), Duration::from_secs(90));
    assert_eq!(
        spec.delete_suspended_after(),
        Duration::from_secs(48 * 3600)
    );
    assert_eq!(spec.notice_before_delete(), Duration::from_secs(12 * 3600));
    assert_eq!(spec.max_age(), Duration::from_secs(7200 * 60));
}

#[test]
fn a_warm_pool_keeps_its_pools_warm_count_for_someone_who_wants_one() {
    let mut pool = pool_from(json!({ "image": IMAGE, "warmReplicas": 2 }));
    assert_eq!(upstream::replicas_kept(&pool, true), 2);
    assert_eq!(upstream::replicas_kept(&pool, false), 0);
    pool.spec.stopped = true;
    assert_eq!(upstream::replicas_kept(&pool, true), 0);
    let none = pool_from(json!({ "image": IMAGE }));
    assert_eq!(upstream::replicas_kept(&none, true), 0);
}

fn claim_of(name: &str, requester: &str, kept: bool, stopped: bool) -> Arc<Claim> {
    let mut claim = Claim::new(
        name,
        ClaimSpec {
            pool: "team".into(),
            requester: requester.into(),
            kept,
            stopped,
            forge: None,
        },
    );
    claim.metadata.namespace = Some("ada".into());
    claim.metadata.uid = Some(format!("uid-{name}"));
    Arc::new(claim)
}

#[test]
fn runners_are_kept_warm_only_for_a_requester_with_a_fresh_running_claim() {
    let pool = pool_from(json!({ "image": IMAGE, "warmReplicas": 1 }));
    let fresh = claim_of("fresh", "ada@example.com", false, false);
    let kept = claim_of("kept", "ada@example.com", true, false);
    let stopped = claim_of("stopped", "ada@example.com", false, true);
    let bobs = claim_of("bobs", "bob@example.com", false, false);
    assert!(warm_wanted(&fresh, &pool, []));
    assert!(!warm_wanted(&kept, &pool, []), "a kept claim starts cold");
    assert!(
        !warm_wanted(&stopped, &pool, []),
        "a stopped claim gets no runner"
    );
    assert!(
        !warm_wanted(&kept, &pool, [stopped.clone(), bobs.clone()]),
        "only the requester's own fresh claims count"
    );
    assert!(warm_wanted(&kept, &pool, [fresh.clone()]));
    let mut going = (*fresh).clone();
    going.metadata.deletion_timestamp = Some(k8s_openapi::apimachinery::pkg::apis::meta::v1::Time(
        k8s_openapi::jiff::Timestamp::UNIX_EPOCH,
    ));
    assert!(!warm_wanted(&kept, &pool, [Arc::new(going)]));
    let mut stopped_pool = pool.clone();
    stopped_pool.spec.stopped = true;
    assert!(!warm_wanted(&fresh, &stopped_pool, []));
}

/// The Pool's spec schema in deploy/crds.yaml.
fn pool_spec_schema() -> Value {
    let documents = common::yaml_documents(
        &std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("deploy/crds.yaml"),
    );
    let pool = documents
        .iter()
        .find(|d| d["spec"]["names"]["kind"] == "Pool")
        .expect("the Pool's definition");
    pool["spec"]["versions"][0]["schema"]["openAPIV3Schema"]["properties"]["spec"].clone()
}

#[test]
fn the_api_server_fills_in_each_default_and_takes_only_whole_durations_and_a_count() {
    let spec = pool_spec_schema();
    let properties = &spec["properties"];
    for (key, default) in [
        ("idleAfter", "30m"),
        ("deleteSuspendedAfter", "720h"),
        ("noticeBeforeDelete", "168h"),
        ("maxAge", "336h"),
    ] {
        assert_eq!(properties[key]["default"], default, "{key}");
        assert_eq!(properties[key]["pattern"], "^[1-9][0-9]*[smh]$", "{key}");
        assert_eq!(properties[key]["maxLength"], 12, "{key}");
    }
    assert_eq!(properties["warmReplicas"]["default"], 0);
    assert_eq!(properties["warmReplicas"]["minimum"].as_f64(), Some(0.0));
    assert_eq!(properties["warmReplicas"]["maximum"].as_f64(), Some(20.0));
    let rules: Vec<&str> = spec["x-kubernetes-validations"]
        .as_array()
        .expect("the Pool's own rules")
        .iter()
        .filter_map(|rule| rule["rule"].as_str())
        .collect();
    assert!(
        rules.contains(&"duration(self.noticeBeforeDelete) < duration(self.deleteSuspendedAfter)"),
        "{rules:?}"
    );
    assert!(
        rules.contains(&"duration(self.idleAfter) >= duration('1m')"),
        "{rules:?}"
    );
    assert!(
        rules.contains(&"duration(self.maxAge) > duration('0s')"),
        "every time is read as a duration, so one too long for it is refused: {rules:?}"
    );
}

#[test]
fn a_pool_spec_written_in_full_still_reads_back() {
    // `PoolSpec` keeps no field of its own out of the CRD: a spec read and written again is the
    // same spec.
    let pool = pool_from(json!({ "image": IMAGE, "warmReplicas": 1, "idleAfter": "1h" }));
    let again: PoolSpec =
        serde_json::from_value(serde_json::to_value(&pool.spec).expect("written")).expect("read");
    assert_eq!(again, pool.spec);
}
