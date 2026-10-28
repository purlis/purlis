// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The operator writes upstream agent-sandbox objects of exactly one version, whose definitions
//! are vendored verbatim. Every field it writes must be one those definitions declare, so moving
//! the pin to a version that renames or drops one turns this red before a cluster does.

mod common;

use kube::api::ObjectMeta;
use purlis_cluster_operator::{Claim, ClaimSpec, Pool, PoolSpec, upstream};
use serde_json::Value;
use std::path::PathBuf;

fn vendored() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("vendor/agent-sandbox")
        .join(upstream::VERSION)
        .join("sandbox-with-extensions.yaml")
}

fn documents() -> Vec<Value> {
    common::yaml_documents(&vendored())
}

/// The schema upstream declares for `kind` at `apiVersion`.
fn schema_of(documents: &[Value], api_version: &str, kind: &str) -> Value {
    let (group, version) = api_version.split_once('/').expect("group/version");
    let definition = documents
        .iter()
        .find(|d| {
            d["kind"] == "CustomResourceDefinition"
                && d["spec"]["group"] == group
                && d["spec"]["names"]["kind"] == kind
        })
        .unwrap_or_else(|| panic!("upstream {} defines no {kind}", upstream::VERSION));
    let version = definition["spec"]["versions"]
        .as_array()
        .expect("versions")
        .iter()
        .find(|v| v["name"] == version && v["served"] == true)
        .unwrap_or_else(|| panic!("upstream serves no {api_version} {kind}"));
    version["schema"]["openAPIV3Schema"].clone()
}

/// Every path in `value` the schema does not declare.
fn undeclared(value: &Value, schema: &Value, path: &str, out: &mut Vec<String>) {
    match value {
        Value::Object(fields) => {
            for (key, field) in fields {
                let here = format!("{path}.{key}");
                if path.is_empty() && matches!(key.as_str(), "apiVersion" | "kind" | "metadata") {
                    continue;
                }
                match schema["properties"].get(key) {
                    Some(inner) => undeclared(field, inner, &here, out),
                    None if schema["additionalProperties"].is_object() => {
                        undeclared(field, &schema["additionalProperties"], &here, out)
                    }
                    None if schema["x-kubernetes-preserve-unknown-fields"] == true => {}
                    None => out.push(here),
                }
            }
        }
        Value::Array(items) => {
            for (i, item) in items.iter().enumerate() {
                undeclared(item, &schema["items"], &format!("{path}[{i}]"), out);
            }
        }
        _ => {}
    }
}

fn written() -> Vec<Value> {
    let image = format!("registry.example/runner@sha256:{}", "2".repeat(64));
    let mut pool = Pool::new(
        "team",
        PoolSpec {
            image,
            kept_volume: purlis_cluster_operator::KeptVolume {
                size: "10Gi".into(),
                storage_class_name: Some("standard".into()),
            },
            stopped: false,
        },
    );
    pool.metadata.uid = Some("u".into());
    let claim = Claim {
        metadata: ObjectMeta {
            name: Some("ada-1".into()),
            namespace: Some("ada".into()),
            uid: Some("v".into()),
            ..ObjectMeta::default()
        },
        spec: ClaimSpec {
            pool: "team".into(),
            requester: "ada@example.com".into(),
            // Kept, so its volume's fields are checked too (#2057).
            kept: true,
            stopped: false,
        },
        status: None,
    };
    [
        serde_json::to_value(upstream::template_for(
            &pool,
            "ada",
            upstream::owners_with(None, &claim, &[&claim]),
        )),
        serde_json::to_value(upstream::warm_pool_for(
            &pool,
            "ada",
            upstream::owners_with(None, &claim, &[&claim]),
        )),
        serde_json::to_value(upstream::sandbox_claim_for(&claim, &Default::default())),
        // A runner suspended or run again when its Pool or Claim is stopped (#2058).
        serde_json::to_value(upstream::sandbox_in_mode(
            &upstream::Sandbox {
                metadata: ObjectMeta {
                    name: Some("ada-1".into()),
                    namespace: Some("ada".into()),
                    uid: Some("w".into()),
                    ..ObjectMeta::default()
                },
                spec: Default::default(),
                status: None,
            },
            "Suspended",
        )),
    ]
    .into_iter()
    .map(|v| v.expect("serialises"))
    .collect()
}

#[test]
fn every_field_the_operator_writes_is_one_the_pinned_upstream_declares() {
    let documents = documents();
    for object in written() {
        let api_version = object["apiVersion"].as_str().expect("apiVersion");
        let kind = object["kind"].as_str().expect("kind");
        let schema = schema_of(&documents, api_version, kind);
        let mut missing = Vec::new();
        undeclared(&object, &schema, "", &mut missing);
        assert!(
            missing.is_empty(),
            "upstream {} does not declare {missing:?} on {kind}",
            upstream::VERSION
        );
    }
}

#[test]
fn the_vendored_definitions_and_controller_are_of_the_pinned_version() {
    let documents = documents();
    let controller = documents
        .iter()
        .find(|d| d["kind"] == "Deployment" && d["metadata"]["name"] == "agent-sandbox-controller")
        .expect("upstream's controller");
    let image = controller["spec"]["template"]["spec"]["containers"][0]["image"]
        .as_str()
        .expect("its image");
    assert_eq!(
        image,
        format!(
            "registry.k8s.io/agent-sandbox/agent-sandbox-controller:{}",
            upstream::VERSION
        )
    );
}

#[test]
fn the_check_finds_a_field_upstream_does_not_declare() {
    let documents = documents();
    let mut object = written().remove(1);
    object["spec"]["notAField"] = Value::Bool(true);
    let schema = schema_of(
        &documents,
        "extensions.agents.x-k8s.io/v1beta1",
        "SandboxWarmPool",
    );
    let mut missing = Vec::new();
    undeclared(&object, &schema, "", &mut missing);
    assert_eq!(missing, vec![".spec.notAField".to_owned()]);
}
