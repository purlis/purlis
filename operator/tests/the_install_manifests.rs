// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What an admin applies: `deploy/crds.yaml` (purlis's definitions, their own step) and
//! `deploy/operator.yaml` (the operator, its service account and what it may do).
//!
//! `deploy/crds.yaml` is generated from the Rust types. When a type changes, regenerate it with
//! `PURLIS_OPERATOR_BLESS=1 cargo test --test the_install_manifests` and read its diff.

use kube::CustomResourceExt;
use purlis_cluster_operator::{Claim, Pool};
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::PathBuf;

fn deploy(file: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("deploy")
        .join(file)
}

fn documents(file: &str) -> Vec<Value> {
    let text = std::fs::read_to_string(deploy(file)).expect("the manifest");
    serde_yaml::Deserializer::from_str(&text)
        .map(|document| {
            serde_json::to_value(serde_yaml::Value::deserialize(document).expect("YAML"))
                .expect("as JSON")
        })
        .filter(|document| !document.is_null())
        .collect()
}

const HEADER: &str = "# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0

# purlis's own definitions, Pool and Claim (runners.purlis.dev/v1alpha1), applied before the
# operator. Generated from operator/src/resources.rs; never edited by hand. Regenerate with
#   PURLIS_OPERATOR_BLESS=1 cargo test --test the_install_manifests
";

fn generated() -> String {
    let mut out = HEADER.to_owned();
    for crd in [Pool::crd(), Claim::crd()] {
        out.push_str("---\n");
        out.push_str(&serde_yaml::to_string(&crd).expect("YAML"));
    }
    out
}

#[test]
fn the_definitions_are_the_ones_the_types_generate() {
    let path = deploy("crds.yaml");
    let wanted = generated();
    if std::env::var_os("PURLIS_OPERATOR_BLESS").is_some() {
        std::fs::write(&path, &wanted).expect("written");
    }
    let held = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        held == wanted,
        "deploy/crds.yaml is not what the types generate; regenerate it with \
         PURLIS_OPERATOR_BLESS=1 cargo test --test the_install_manifests"
    );
}

#[test]
fn a_pool_is_the_clusters_and_a_claim_is_a_namespaces() {
    let documents = documents("crds.yaml");
    let scope = |kind: &str| {
        documents
            .iter()
            .find(|d| d["spec"]["names"]["kind"] == kind)
            .map(|d| d["spec"]["scope"].clone())
    };
    assert_eq!(scope("Pool"), Some(json!("Cluster")));
    assert_eq!(scope("Claim"), Some(json!("Namespaced")));
}

fn operator_rules() -> Vec<Value> {
    let documents = documents("operator.yaml");
    let roles: Vec<&Value> = documents
        .iter()
        .filter(|d| matches!(d["kind"].as_str(), Some("ClusterRole" | "Role")))
        .collect();
    assert_eq!(roles.len(), 1, "the operator has one role: {roles:?}");
    roles[0]["rules"].as_array().expect("rules").clone()
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .map(|s| s.as_str().expect("a string").to_owned())
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn the_operator_may_not_exec_or_attach_and_reads_no_secret() {
    for rule in operator_rules() {
        let resources = strings(&rule["resources"]);
        let verbs = strings(&rule["verbs"]);
        let groups = strings(&rule["apiGroups"]);
        for word in resources.iter().chain(&verbs).chain(&groups) {
            assert_ne!(word, "*", "no wildcard: {rule}");
        }
        for resource in &resources {
            assert!(
                !matches!(
                    resource.as_str(),
                    "secrets" | "pods/exec" | "pods/attach" | "pods/portforward" | "nodes/proxy"
                ),
                "{rule}"
            );
        }
        for verb in &verbs {
            assert!(
                !matches!(verb.as_str(), "escalate" | "bind" | "impersonate"),
                "{rule}"
            );
        }
        assert!(rule.get("nonResourceURLs").is_none(), "{rule}");
    }
}

#[test]
fn the_operator_may_do_exactly_what_it_needs() {
    let mut rules: Vec<(String, String, String)> = Vec::new();
    for rule in operator_rules() {
        for group in strings(&rule["apiGroups"]) {
            for resource in strings(&rule["resources"]) {
                let mut verbs = strings(&rule["verbs"]);
                verbs.sort();
                rules.push((group.clone(), resource, verbs.join(",")));
            }
        }
    }
    rules.sort();
    let wanted = [
        (
            "extensions.agents.x-k8s.io",
            "sandboxclaims",
            "create,get,list,patch,watch",
        ),
        (
            "extensions.agents.x-k8s.io",
            "sandboxtemplates",
            "create,patch",
        ),
        (
            "extensions.agents.x-k8s.io",
            "sandboxwarmpools",
            "create,patch",
        ),
        ("runners.purlis.dev", "claims", "get,list,watch"),
        ("runners.purlis.dev", "claims/status", "patch"),
        ("runners.purlis.dev", "pools", "get,list,watch"),
    ]
    .map(|(g, r, v)| (g.to_owned(), r.to_owned(), v.to_owned()));
    assert_eq!(rules, wanted);
}

#[test]
fn the_operator_runs_as_a_user_who_is_not_root_with_nothing_added() {
    let documents = documents("operator.yaml");
    let deployment = documents
        .iter()
        .find(|d| d["kind"] == "Deployment")
        .expect("the operator's deployment");
    let pod = &deployment["spec"]["template"]["spec"];
    let container = &pod["containers"][0];
    let security = &container["securityContext"];
    assert_eq!(security["runAsNonRoot"], true);
    assert_eq!(security["allowPrivilegeEscalation"], false);
    assert_eq!(security["readOnlyRootFilesystem"], true);
    assert_eq!(security["capabilities"]["drop"], json!(["ALL"]));
    assert_eq!(security["seccompProfile"]["type"], "RuntimeDefault");
    assert_eq!(pod["containers"].as_array().map(Vec::len), Some(1));
}
