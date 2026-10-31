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
                "purlis-claims-use-their-pool,purlis-runner-namespaces",
                "get,list,watch",
            ),
            ("agents.x-k8s.io", "sandboxes", "", "get,patch"),
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
        [
            purlis_cluster_operator::POLICY,
            purlis_cluster_operator::RUNNER_NAMESPACES_POLICY
        ]
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
    // The whole Secret, so next.pem comes and goes with a roll, refreshed by the kubelet: a
    // subPath mount or `items` naming a key would not be (#3305).
    assert!(
        secret.get("items").is_none(),
        "the signing-key volume names items of the Secret"
    );
    assert!(mounts[0].get("subPath").is_none());
    assert_eq!(
        mounts[0]["mountPath"],
        purlis_cluster_operator::vouch::SIGNING_KEY_DIR
    );
    assert_eq!(
        format!(
            "{}/key.pem",
            purlis_cluster_operator::vouch::SIGNING_KEY_DIR
        ),
        purlis_cluster_operator::vouch::SIGNING_KEY_PATH
    );
    assert_eq!(
        format!(
            "{}/next.pem",
            purlis_cluster_operator::vouch::SIGNING_KEY_DIR
        ),
        purlis_cluster_operator::vouch::NEXT_SIGNING_KEY_PATH
    );
}

#[test]
fn a_claim_is_admitted_only_for_someone_the_cluster_lets_use_its_pool() {
    let documents = documents("policy.yaml");
    let policy = policy_named(&documents, purlis_cluster_operator::POLICY);
    let spec = &policy["spec"];
    assert_eq!(spec["failurePolicy"], "Fail");
    let rules = spec["matchConstraints"]["resourceRules"]
        .as_array()
        .expect("rules");
    assert_eq!(rules.len(), 2, "{spec}");
    assert_eq!(strings(&rules[0]["apiGroups"]), ["runners.purlis.dev"]);
    assert_eq!(strings(&rules[0]["resources"]), ["claims"]);
    // CREATE for who may make one, UPDATE for who may change the repos it names (#4204).
    assert_eq!(strings(&rules[0]["operations"]), ["CREATE", "UPDATE"]);
    let validations = spec["validations"].as_array().expect("validations");
    assert_eq!(validations.len(), 5, "{spec}");
    let expression = |at: usize| {
        validations[at]["expression"]
            .as_str()
            .expect("expression")
            .split_whitespace()
            .collect::<String>()
    };
    assert_eq!(
        expression(0),
        "request.resource.group!='runners.purlis.dev'||request.operation!='CREATE'||authorizer.group('runners.purlis.dev').resource('pools').name(object.spec.pool).check('use').allowed()"
    );
    // Who asked is stamped as the cluster signed them in (#2056).
    assert_eq!(
        expression(1),
        "request.resource.group!='runners.purlis.dev'||request.operation!='CREATE'||object.spec.requester==request.userInfo.username"
    );
    // Only who asked changes the repos a claim names (#4204).
    assert_eq!(
        expression(4),
        "request.resource.group!='runners.purlis.dev'||request.operation!='UPDATE'||(has(object.spec.forge)?has(oldObject.spec.forge)&&object.spec.forge==oldObject.spec.forge:!has(oldObject.spec.forge))||(request.userInfo.username==object.spec.requester&&authorizer.group('runners.purlis.dev').resource('pools').name(object.spec.pool).check('use').allowed())"
    );

    let binding = documents
        .iter()
        .find(|d| {
            d["kind"] == "ValidatingAdmissionPolicyBinding"
                && d["spec"]["policyName"] == "purlis-claims-use-their-pool"
        })
        .expect("its binding");
    assert_eq!(binding["spec"]["policyName"], policy["metadata"]["name"]);
    assert_eq!(strings(&binding["spec"]["validationActions"]), ["Deny"]);
}

/// The spec fields the pinned upstream's Sandbox declares.
fn sandbox_spec_fields() -> Vec<String> {
    let vendored = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("vendor/agent-sandbox")
        .join(purlis_cluster_operator::upstream::VERSION)
        .join("sandbox-with-extensions.yaml");
    let definition = common::yaml_documents(&vendored)
        .into_iter()
        .find(|d| {
            d["kind"] == "CustomResourceDefinition"
                && d["metadata"]["name"] == "sandboxes.agents.x-k8s.io"
        })
        .expect("upstream's Sandbox");
    let mut fields: Vec<String> = definition["spec"]["versions"][0]["schema"]["openAPIV3Schema"]
        ["properties"]["spec"]["properties"]
        .as_object()
        .expect("its spec")
        .keys()
        .cloned()
        .collect();
    fields.sort();
    fields
}

/// **The operator's `patch` on sandboxes is narrowed by the cluster** (#2058): RBAC cannot name
/// a field or an owner, so a policy refuses the operator's service account any change to a
/// Sandbox but its operating mode, and any change to one no SandboxClaim controls.
#[test]
fn the_operators_right_on_sandboxes_is_narrowed_to_the_mode_of_a_claims_sandbox() {
    let documents = documents("policy.yaml");
    let name = "purlis-operator-changes-only-a-sandboxs-mode";
    let policy = documents
        .iter()
        .find(|d| d["kind"] == "ValidatingAdmissionPolicy" && d["metadata"]["name"] == name)
        .expect("the policy on sandboxes");
    let spec = &policy["spec"];
    assert_eq!(spec["failurePolicy"], "Fail");
    let rules = spec["matchConstraints"]["resourceRules"]
        .as_array()
        .expect("rules");
    assert_eq!(rules.len(), 1, "{spec}");
    assert_eq!(strings(&rules[0]["apiGroups"]), ["agents.x-k8s.io"]);
    assert_eq!(strings(&rules[0]["resources"]), ["sandboxes"]);
    assert_eq!(strings(&rules[0]["operations"]), ["UPDATE"]);
    let squeezed = |value: &Value| {
        value
            .as_str()
            .expect("an expression")
            .split_whitespace()
            .collect::<String>()
    };
    let conditions = spec["matchConditions"].as_array().expect("conditions");
    assert_eq!(conditions.len(), 1);
    assert_eq!(
        squeezed(&conditions[0]["expression"]),
        "request.userInfo.username=='system:serviceaccount:purlis-system:purlis-cluster-operator'"
    );
    let validations: Vec<String> = spec["validations"]
        .as_array()
        .expect("validations")
        .iter()
        .map(|v| squeezed(&v["expression"]))
        .collect();
    assert_eq!(validations.len(), 3, "{validations:?}");
    assert!(
        validations[0].contains("r.controller&&r.kind=='SandboxClaim'"),
        "{}",
        validations[0]
    );
    // Every spec field upstream declares is held as it was, but operatingMode.
    let held: Vec<String> = sandbox_spec_fields()
        .into_iter()
        .filter(|field| field != "operatingMode")
        .collect();
    assert!(!held.is_empty());
    for field in &held {
        assert!(
            validations[1].contains(&format!("object.spec.{field}==oldObject.spec.{field}")),
            "spec.{field} is not held: {}",
            validations[1]
        );
    }
    assert!(!validations[1].contains("operatingMode"));
    for field in ["labels", "annotations", "ownerReferences", "finalizers"] {
        assert!(
            validations[2].contains(&format!(
                "object.metadata.{field}==oldObject.metadata.{field}"
            )),
            "metadata.{field} is not held"
        );
    }
    let binding = documents
        .iter()
        .find(|d| {
            d["kind"] == "ValidatingAdmissionPolicyBinding" && d["spec"]["policyName"] == name
        })
        .expect("its binding");
    assert_eq!(strings(&binding["spec"]["validationActions"]), ["Deny"]);
}

#[test]
fn an_upstream_claim_or_warm_pool_on_a_purlis_pools_objects_is_admitted_from_the_operator_alone() {
    let documents = documents("policy.yaml");
    let policy = policy_named(&documents, purlis_cluster_operator::POLICY);
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

/// The ValidatingAdmissionPolicy `name` among `documents`, and its binding, which denies.
fn policy_named<'a>(documents: &'a [Value], name: &str) -> &'a Value {
    let named = |kind: &str| {
        documents
            .iter()
            .find(|d| d["kind"] == kind && d["metadata"]["name"] == name)
            .unwrap_or_else(|| panic!("the {kind} {name}"))
    };
    let binding = named("ValidatingAdmissionPolicyBinding");
    assert_eq!(binding["spec"]["policyName"], name);
    assert_eq!(binding["spec"]["validationActions"], json!(["Deny"]));
    named("ValidatingAdmissionPolicy")
}

#[test]
fn a_runner_namespace_and_what_the_operator_binds_and_caps_there_are_guarded_at_admission() {
    let documents = documents("policy.yaml");
    let policy = policy_named(
        &documents,
        purlis_cluster_operator::RUNNER_NAMESPACES_POLICY,
    );
    let spec = &policy["spec"];
    assert_eq!(spec["failurePolicy"], "Fail");
    assert_eq!(
        policy["metadata"]["annotations"]["helm.sh/resource-policy"],
        "keep"
    );
    let mut matched: Vec<(String, String)> = Vec::new();
    for rule in spec["matchConstraints"]["resourceRules"]
        .as_array()
        .expect("rules")
    {
        assert_eq!(strings(&rule["operations"]), ["CREATE", "UPDATE"], "{rule}");
        for group in strings(&rule["apiGroups"]) {
            for resource in strings(&rule["resources"]) {
                matched.push((group.clone(), resource));
            }
        }
    }
    matched.sort();
    assert_eq!(
        matched,
        [
            ("", "namespaces"),
            ("", "namespaces/finalize"),
            ("", "namespaces/status"),
            ("", "resourcequotas"),
            ("rbac.authorization.k8s.io", "rolebindings"),
        ]
        .map(|(g, r)| (g.to_owned(), r.to_owned()))
    );
    let flat = |value: &Value| {
        value
            .as_str()
            .expect("an expression")
            .split_whitespace()
            .collect::<String>()
    };
    let variables: Vec<(String, String)> = spec["variables"]
        .as_array()
        .expect("variables")
        .iter()
        .map(|v| {
            (
                v["name"].as_str().unwrap().to_owned(),
                flat(&v["expression"]),
            )
        })
        .collect();
    let operator = "system:serviceaccount:purlis-system:purlis-cluster-operator";
    assert_eq!(variables[0].0, "operator");
    assert_eq!(
        variables[0].1,
        format!("request.userInfo.username=='{operator}'")
    );
    let prefix = purlis_cluster_operator::person::NAMESPACE_PREFIX;
    assert_eq!(variables[1].0, "runners");
    assert!(
        variables[1].1.contains(&format!("startsWith('{prefix}')")),
        "{variables:?}"
    );
    // The marks the operator trusts a runner namespace by are the ones it puts on it.
    let (label, _) = purlis_cluster_operator::person::MANAGED_BY;
    let annotation = purlis_cluster_operator::person::REQUESTER_ANNOTATION;
    for at in [2, 3] {
        assert!(
            variables[at].1.contains(&format!("'{label}'")),
            "{variables:?}"
        );
        assert!(
            variables[at].1.contains(&format!("'{annotation}'")),
            "{variables:?}"
        );
    }
    // The one subject a binding may name is the person its runner namespace is for.
    assert_eq!(variables[4].0, "requester");
    assert!(
        variables[4].1.contains(&format!(
            "namespaceObject.metadata.annotations['{annotation}']"
        )),
        "{variables:?}"
    );
    let validations: Vec<String> = spec["validations"]
        .as_array()
        .expect("validations")
        .iter()
        .map(|v| flat(&v["expression"]))
        .collect();
    assert_eq!(validations.len(), 4, "{validations:?}");
    // Made by the operator alone; who it is for changed by no one else.
    assert_eq!(
        validations[0],
        "request.resource.resource!='namespaces'||variables.operator||!variables.runners||\
         request.operation=='UPDATE'&&variables.marks==variables.oldMarks"
    );
    assert_eq!(
        validations[1],
        "request.resource.resource!='namespaces'||!variables.operator||variables.runners"
    );
    // One role, one person who is not the operator, in a runner namespace alone.
    let owner = purlis_cluster_operator::person::OWNER_ROLE;
    for words in [
        "request.resource.resource!='rolebindings'||!variables.operator||variables.runners&&".to_owned(),
        format!("object.metadata.name=='{owner}'"),
        format!("object.roleRef.kind=='ClusterRole'&&object.roleRef.name=='{owner}'"),
        "has(object.subjects)&&size(object.subjects)==1".to_owned(),
        "object.subjects[0].kind=='User'&&!object.subjects[0].name.startsWith('system:')&&object.subjects[0].name==variables.requester".to_owned(),
        "'system:serviceaccount:'+object.subjects[0].namespace+':'+object.subjects[0].name==variables.requester".to_owned(),
        format!("variables.requester!='{operator}'"),
    ] {
        assert!(validations[2].contains(&words), "{words}: {}", validations[2]);
    }
    assert_eq!(
        validations[3],
        format!(
            "request.resource.resource!='resourcequotas'||!variables.operator||\
             variables.runners&&object.metadata.name=='{}'",
            purlis_cluster_operator::person::QUOTA
        )
    );
}

/// A plain install, made without the chart's values, names no Secret: no Role reads one (#4204).
#[test]
fn the_plain_install_holds_no_role_on_a_secret() {
    for document in documents("operator.yaml") {
        assert_ne!(document["kind"], "Role", "{document}");
    }
}
