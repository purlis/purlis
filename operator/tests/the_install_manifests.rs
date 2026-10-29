// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What an admin applies: `deploy/crds.yaml` (purlis's definitions, their own step) and
//! `deploy/operator.yaml` (the operator, its service account and what it may do).
//!
//! `deploy/crds.yaml` is generated from the Rust types. When a type changes, regenerate it with
//! `PURLIS_OPERATOR_BLESS=1 cargo test --test the_install_manifests` and read its diff.

mod common;

use kube::CustomResourceExt;
use purlis_cluster_operator::{Claim, Pool};
use serde_json::{Value, json};
use std::path::PathBuf;

fn deploy(file: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("deploy")
        .join(file)
}

fn documents(file: &str) -> Vec<Value> {
    common::yaml_documents(&deploy(file))
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
    // The first line that differs, so a run that cannot bless shows where.
    let differs = held
        .lines()
        .zip(wanted.lines())
        .enumerate()
        .find(|(_, (held, wanted))| held != wanted)
        .map(|(at, (held, wanted))| format!("line {}: held {held:?}, generated {wanted:?}", at + 1))
        .unwrap_or_else(|| "one ends before the other".to_owned());
    assert!(
        held == wanted,
        "deploy/crds.yaml is not what the types generate ({differs}); regenerate it with \
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

/// The rules of the role `name` in `operator.yaml`, its one of that name.
fn rules_of(name: &str) -> Vec<Value> {
    let documents = documents("operator.yaml");
    let roles: Vec<&Value> = documents
        .iter()
        .filter(|d| matches!(d["kind"].as_str(), Some("ClusterRole" | "Role")))
        .collect();
    let names: Vec<&str> = roles
        .iter()
        .filter_map(|role| role["metadata"]["name"].as_str())
        .collect();
    assert_eq!(
        names,
        ["purlis-cluster-operator", "purlis-runner-owner"],
        "the operator's role, and the one it binds for a person (#2055)"
    );
    let role = roles
        .iter()
        .find(|role| role["metadata"]["name"] == name)
        .unwrap_or_else(|| panic!("the role {name}"));
    role["rules"].as_array().expect("rules").clone()
}

fn operator_rules() -> Vec<Value> {
    rules_of("purlis-cluster-operator")
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
                !matches!(verb.as_str(), "escalate" | "impersonate"),
                "{rule}"
            );
            // It binds one role alone, a person's own runners (#2055).
            if verb == "bind" {
                assert_eq!(resources, ["clusterroles"], "{rule}");
                assert_eq!(
                    strings(&rule["resourceNames"]),
                    ["purlis-runner-owner"],
                    "{rule}"
                );
            }
        }
        assert!(rule.get("nonResourceURLs").is_none(), "{rule}");
    }
}

#[test]
fn the_operator_may_do_exactly_what_it_needs() {
    assert_eq!(
        flat(&operator_rules()),
        [
            ("", "namespaces", "", "create,get"),
            ("", "pods", "", "get"),
            ("", "resourcequotas", "", "create"),
            ("", "resourcequotas", "purlis-person", "get,patch"),
            (
                "admissionregistration.k8s.io",
                "validatingadmissionpolicybindings",
                "purlis-claims-use-their-pool",
                "get,list,watch",
            ),
            ("agents.x-k8s.io", "sandboxes", "", "get"),
            (
                "extensions.agents.x-k8s.io",
                "sandboxclaims",
                "",
                "create,delete,get,list,patch,watch",
            ),
            (
                "extensions.agents.x-k8s.io",
                "sandboxtemplates",
                "",
                "create,delete,get,list,patch,watch",
            ),
            (
                "extensions.agents.x-k8s.io",
                "sandboxwarmpools",
                "",
                "create,delete,get,list,patch,watch",
            ),
            (
                "rbac.authorization.k8s.io",
                "clusterroles",
                "purlis-runner-owner",
                "bind"
            ),
            ("rbac.authorization.k8s.io", "rolebindings", "", "create"),
            (
                "rbac.authorization.k8s.io",
                "rolebindings",
                "purlis-runner-owner",
                "get,patch"
            ),
            ("runners.purlis.dev", "claims", "", "list,patch,watch"),
            ("runners.purlis.dev", "claims/status", "", "patch"),
            ("runners.purlis.dev", "pools", "", "get,list,watch"),
            ("runners.purlis.dev", "pools/status", "", "patch"),
        ]
        .map(|(g, r, n, v)| (g.to_owned(), r.to_owned(), n.to_owned(), v.to_owned()))
    );
}

/// `rules` as one (group, resource, names, verbs) line each, sorted.
fn flat(rules: &[Value]) -> Vec<(String, String, String, String)> {
    let mut flat = Vec::new();
    for rule in rules {
        for group in strings(&rule["apiGroups"]) {
            for resource in strings(&rule["resources"]) {
                let mut verbs = strings(&rule["verbs"]);
                verbs.sort();
                flat.push((
                    group.clone(),
                    resource,
                    strings(&rule["resourceNames"]).join(","),
                    verbs.join(","),
                ));
            }
        }
    }
    flat.sort();
    flat
}

#[test]
fn a_person_may_see_exec_into_and_read_the_logs_of_their_runners_and_nothing_else() {
    assert_eq!(
        flat(&rules_of(purlis_cluster_operator::person::OWNER_ROLE)),
        [
            ("", "pods", "", "get,list,watch"),
            ("", "pods/exec", "", "create,get"),
            ("", "pods/log", "", "get"),
        ]
        .map(|(g, r, n, v)| (g.to_owned(), r.to_owned(), n.to_owned(), v.to_owned()))
    );
}

#[test]
fn the_operator_reads_only_its_own_policys_binding() {
    let rules: Vec<Value> = operator_rules()
        .into_iter()
        .filter(|rule| strings(&rule["apiGroups"]) == ["admissionregistration.k8s.io"])
        .collect();
    assert_eq!(rules.len(), 1, "{rules:?}");
    assert_eq!(
        strings(&rules[0]["resources"]),
        ["validatingadmissionpolicybindings"]
    );
    assert_eq!(
        strings(&rules[0]["resourceNames"]),
        [purlis_cluster_operator::POLICY]
    );
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

/// Its signing key (#2056) is a Secret the kubelet mounts in its pod, read-only, for its user's
/// group alone: the operator holds no right to read a Secret.
#[test]
fn the_operators_signing_key_is_mounted_from_its_secret_by_the_kubelet() {
    let documents = documents("operator.yaml");
    let deployment = documents
        .iter()
        .find(|d| d["kind"] == "Deployment")
        .expect("the operator's deployment");
    let pod = &deployment["spec"]["template"]["spec"];
    let volumes = pod["volumes"].as_array().expect("volumes");
    assert_eq!(volumes.len(), 1, "{volumes:?}");
    let secret = &volumes[0]["secret"];
    assert_eq!(secret["secretName"], "purlis-cluster-operator-signing-key");
    assert_eq!(secret["defaultMode"], 0o440);
    assert_eq!(pod["securityContext"]["fsGroup"], 65532);
    let mounts = pod["containers"][0]["volumeMounts"]
        .as_array()
        .expect("mounts");
    assert_eq!(mounts.len(), 1);
    assert_eq!(mounts[0]["name"], volumes[0]["name"]);
    assert_eq!(mounts[0]["readOnly"], true);
    let path = format!(
        "{}/{}",
        mounts[0]["mountPath"].as_str().unwrap(),
        secret["items"][0]["path"].as_str().unwrap()
    );
    assert_eq!(path, purlis_cluster_operator::vouch::SIGNING_KEY_PATH);
}

#[test]
fn a_claim_is_admitted_only_for_someone_the_cluster_lets_use_its_pool() {
    let documents = documents("policy.yaml");
    let policy = documents
        .iter()
        .find(|d| d["kind"] == "ValidatingAdmissionPolicy")
        .expect("the policy");
    let spec = &policy["spec"];
    assert_eq!(spec["failurePolicy"], "Fail");
    let rules = spec["matchConstraints"]["resourceRules"]
        .as_array()
        .expect("rules");
    assert_eq!(rules.len(), 2, "{spec}");
    assert_eq!(strings(&rules[0]["apiGroups"]), ["runners.purlis.dev"]);
    assert_eq!(strings(&rules[0]["resources"]), ["claims"]);
    assert_eq!(strings(&rules[0]["operations"]), ["CREATE"]);
    let validations = spec["validations"].as_array().expect("validations");
    assert_eq!(validations.len(), 4, "{spec}");
    let expression = |at: usize| {
        validations[at]["expression"]
            .as_str()
            .expect("expression")
            .split_whitespace()
            .collect::<String>()
    };
    assert_eq!(
        expression(0),
        "request.resource.group!='runners.purlis.dev'||authorizer.group('runners.purlis.dev').resource('pools').name(object.spec.pool).check('use').allowed()"
    );
    // Who asked is stamped as the cluster signed them in (#2056).
    assert_eq!(
        expression(1),
        "request.resource.group!='runners.purlis.dev'||object.spec.requester==request.userInfo.username"
    );

    let binding = documents
        .iter()
        .find(|d| d["kind"] == "ValidatingAdmissionPolicyBinding")
        .expect("its binding");
    assert_eq!(binding["spec"]["policyName"], policy["metadata"]["name"]);
    assert_eq!(strings(&binding["spec"]["validationActions"]), ["Deny"]);
}

#[test]
fn an_upstream_claim_or_warm_pool_on_a_purlis_pools_objects_is_admitted_from_the_operator_alone() {
    let documents = documents("policy.yaml");
    let policy = documents
        .iter()
        .find(|d| d["kind"] == "ValidatingAdmissionPolicy")
        .expect("the policy");
    let spec = &policy["spec"];
    let rules = spec["matchConstraints"]["resourceRules"]
        .as_array()
        .expect("rules");
    assert_eq!(
        strings(&rules[1]["apiGroups"]),
        ["extensions.agents.x-k8s.io"]
    );
    assert_eq!(
        strings(&rules[1]["resources"]),
        ["sandboxclaims", "sandboxwarmpools"]
    );
    assert_eq!(strings(&rules[1]["operations"]), ["CREATE", "UPDATE"]);
    let validations = spec["validations"].as_array().expect("validations");
    let expression = |at: usize| {
        validations[at]["expression"]
            .as_str()
            .expect("expression")
            .split_whitespace()
            .collect::<String>()
    };
    // The prefix of every name the operator gives a pool's template and warm pool. On an update,
    // a reference may neither move onto one of them nor off one.
    let ours = purlis_cluster_operator::upstream::name_for("");
    let operator = "system:serviceaccount:purlis-system:purlis-cluster-operator";
    for (at, resource, reference) in [
        (2, "sandboxclaims", "warmPoolRef"),
        (3, "sandboxwarmpools", "sandboxTemplateRef"),
    ] {
        assert_eq!(
            expression(at),
            format!(
                "request.resource.resource!='{resource}'||\
                 request.userInfo.username=='{operator}'||\
                 (request.operation=='UPDATE'?\
                 oldObject.spec.{reference}.name==object.spec.{reference}.name||\
                 !oldObject.spec.{reference}.name.startsWith('{ours}')&&\
                 !object.spec.{reference}.name.startsWith('{ours}'):\
                 !object.spec.{reference}.name.startsWith('{ours}'))"
            )
        );
        assert_eq!(validations[at]["reason"], "Forbidden");
    }
}
