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
                "validatingadmissionpolicies",
                "purlis-claims-use-their-pool,purlis-runner-namespaces,purlis-forge",
                "get",
            ),
            (
                "admissionregistration.k8s.io",
                "validatingadmissionpolicybindings",
                "purlis-claims-use-their-pool,purlis-runner-namespaces,purlis-forge",
                "get,list,watch",
            ),
            ("agents.x-k8s.io", "sandboxes", "", "get,patch"),
            ("authorization.k8s.io", "subjectaccessreviews", "", "create"),
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
            ("networking.k8s.io", "networkpolicies", "", "create"),
            (
                "networking.k8s.io",
                "networkpolicies",
                "purlis-forge-mint",
                "get,patch"
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
            (
                "runners.purlis.dev",
                "claims",
                "",
                "delete,get,list,patch,watch"
            ),
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
    assert_eq!(rules.len(), 2, "{rules:?}");
    assert_eq!(
        strings(&rules[0]["resources"]),
        ["validatingadmissionpolicybindings"]
    );
    assert_eq!(
        strings(&rules[0]["resourceNames"]),
        [
            purlis_cluster_operator::POLICY,
            purlis_cluster_operator::RUNNER_NAMESPACES_POLICY,
            purlis_cluster_operator::FORGE_POLICY,
        ]
    );
    assert_eq!(
        strings(&rules[1]["resources"]),
        ["validatingadmissionpolicies"]
    );
    assert_eq!(strings(&rules[1]["verbs"]), ["get"]);
    assert_eq!(
        strings(&rules[1]["resourceNames"]),
        [
            purlis_cluster_operator::POLICY,
            purlis_cluster_operator::RUNNER_NAMESPACES_POLICY,
            purlis_cluster_operator::FORGE_POLICY,
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
    // CREATE alone: who may change the repos it names is the policy on App keys' (#4206).
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
        "request.resource.group!='runners.purlis.dev'||request.operation!='CREATE'||authorizer.group('runners.purlis.dev').resource('pools').name(object.spec.pool).check('use').allowed()"
    );
    // Who asked is stamped as the cluster signed them in (#2056).
    assert_eq!(
        expression(1),
        "request.resource.group!='runners.purlis.dev'||request.operation!='CREATE'||object.spec.requester==request.userInfo.username"
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
    // Its mode may change; its template only while it is suspended, in a runner namespace.
    assert!(!validations[1].contains("object.spec.operatingMode"));
    assert!(validations[1].contains(
        "request.namespace.startsWith('purlis-runners-')&&has(oldObject.spec.operatingMode)&&oldObject.spec.operatingMode=='Suspended'&&variables.renewed"
    ));
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
            ("networking.k8s.io", "networkpolicies"),
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
    assert_eq!(validations.len(), 7, "{validations:?}");
    // What it makes, it makes under the restricted Pod Security Standard (#3824).
    assert_eq!(
        validations[6],
        "request.resource.resource!='namespaces'||!variables.operator||\
         request.operation!='CREATE'||\
         has(object.metadata.labels)&&'pod-security.kubernetes.io/enforce'inobject.metadata.labels&&\
         object.metadata.labels['pod-security.kubernetes.io/enforce']=='restricted'"
    );
    // Its mark opens the mint port, so no one else marks another namespace (#4206).
    assert_eq!(
        validations[1],
        "request.resource.resource!='namespaces'||variables.operator||variables.runners||\
         variables.marks[0]!='purlis-cluster-operator'"
    );
    let validations: Vec<String> = validations
        .iter()
        .enumerate()
        .filter(|(at, _)| *at != 1)
        .map(|(_, held)| held.clone())
        .collect();
    // Its one NetworkPolicy, to its own mint port alone (#4206).
    for words in [
        "request.resource.resource!='networkpolicies'||!variables.operator||variables.runners&&",
        "object.metadata.name=='purlis-forge-mint'",
        "object.spec.policyTypes==['Egress']&&!has(object.spec.ingress)",
        "object.spec.egress[0].ports[0].port==8443",
    ] {
        assert!(
            validations[4].contains(words),
            "{words}: {}",
            validations[4]
        );
    }
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

/// The policy on App keys (#4206): spending a key takes `use` on it, and only a Claim's requester
/// changes the repos it names. Its own binding, so its time says when these rules came into force.
#[test]
fn spending_an_app_key_takes_use_on_it() {
    let documents = documents("policy.yaml");
    let policy = policy_named(&documents, purlis_cluster_operator::FORGE_POLICY);
    let spec = &policy["spec"];
    assert_eq!(spec["failurePolicy"], "Fail");
    let flat = |value: &Value| {
        value
            .as_str()
            .expect("an expression")
            .split_whitespace()
            .collect::<String>()
    };
    let validations = spec["validations"].as_array().expect("validations");
    assert_eq!(validations.len(), 3, "{spec}");
    assert_eq!(
        flat(&validations[0]["expression"]),
        "request.resource.resource!='pools'||!has(object.spec.forge)||(request.operation=='UPDATE'&&has(oldObject.spec.forge)&&object.spec.forge==oldObject.spec.forge)||authorizer.group('runners.purlis.dev').resource('forgekeys').name(object.spec.forge.appKeySecretRef.name).check('use').allowed()"
    );
    // On a Pool that mints, the image holds the link keys, so it takes `use` on the key too.
    assert_eq!(
        flat(&validations[1]["expression"]),
        "request.resource.resource!='pools'||request.operation!='UPDATE'||object.spec.image==oldObject.spec.image||!has(oldObject.spec.forge)||!oldObject.spec.forge.mintInCluster||authorizer.group('runners.purlis.dev').resource('forgekeys').name(oldObject.spec.forge.appKeySecretRef.name).check('use').allowed()"
    );
    assert_eq!(
        flat(&validations[2]["expression"]),
        "request.resource.resource!='claims'||(has(object.spec.forge)?has(oldObject.spec.forge)&&object.spec.forge==oldObject.spec.forge:!has(oldObject.spec.forge))||(request.userInfo.username==object.spec.requester&&authorizer.group('runners.purlis.dev').resource('pools').name(object.spec.pool).check('use').allowed())"
    );
    let binding = documents
        .iter()
        .find(|d| {
            d["kind"] == "ValidatingAdmissionPolicyBinding"
                && d["spec"]["policyName"] == "purlis-forge"
        })
        .expect("its binding");
    assert_eq!(strings(&binding["spec"]["validationActions"]), ["Deny"]);
}

/// Whether `text` names `name`, and not a longer word that starts with it.
fn names(text: &str, name: &str) -> bool {
    text.match_indices(name)
        .any(|(at, _)| !text[at + name.len()..].starts_with(char::is_alphanumeric))
}

/// The spec schema of the pinned upstream's definition of `kind`.
fn upstream_definition(kind: &str) -> Value {
    let vendored = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("vendor/agent-sandbox")
        .join(purlis_cluster_operator::upstream::VERSION)
        .join("sandbox-with-extensions.yaml");
    let definition = common::yaml_documents(&vendored)
        .into_iter()
        .find(|d| d["kind"] == "CustomResourceDefinition" && d["spec"]["names"]["kind"] == kind)
        .expect("the definition");
    definition["spec"]["versions"][0]["schema"]["openAPIV3Schema"]["properties"]["spec"].clone()
}

fn fields_of(properties: &Value) -> Vec<String> {
    properties
        .as_object()
        .expect("its fields")
        .keys()
        .cloned()
        .collect()
}

/// The fields of a spec in the pinned upstream's definition of `kind`.
fn upstream_spec_fields(kind: &str) -> Vec<String> {
    fields_of(&upstream_definition(kind)["properties"])
}

#[test]
fn what_the_operator_writes_of_upstreams_is_held_to_a_runners_shape() {
    let documents = documents("policy.yaml");
    let name = "purlis-operator-writes-only-a-runners-shape";
    let policy = policy_named(&documents, name);
    let spec = &policy["spec"];
    assert_eq!(spec["failurePolicy"], "Fail");
    assert_eq!(
        policy["metadata"]["annotations"]["helm.sh/resource-policy"],
        "keep"
    );
    let rule = &spec["matchConstraints"]["resourceRules"][0];
    assert_eq!(strings(&rule["operations"]), ["CREATE", "UPDATE"]);
    assert_eq!(
        strings(&rule["resources"]),
        ["sandboxclaims", "sandboxtemplates", "sandboxwarmpools"]
    );
    assert!(
        spec["matchConditions"][0]["expression"]
            .as_str()
            .expect("its condition")
            .contains("system:serviceaccount:purlis-system:purlis-cluster-operator")
    );
    let binding = documents
        .iter()
        .find(|d| d["kind"] == "ValidatingAdmissionPolicyBinding" && d["metadata"]["name"] == name)
        .expect("its binding");
    assert_eq!(binding["spec"]["policyName"], name);
    assert_eq!(strings(&binding["spec"]["validationActions"]), ["Deny"]);
    let validations: Vec<String> = spec["validations"]
        .as_array()
        .expect("validations")
        .iter()
        .map(|v| v["expression"].as_str().expect("an expression").to_owned())
        .collect();
    assert_eq!(validations.len(), 7, "{validations:?}");
    assert_eq!(
        validations[0],
        "request.namespace.startsWith('purlis-runners-')"
    );
    // Every field the pinned upstream gives a claim and a template is named, so a new one fails
    // here and is looked at when the pin moves.
    for field in upstream_spec_fields("SandboxClaim") {
        assert!(
            names(&validations[1], &field),
            "{field}: {}",
            validations[1]
        );
    }
    for field in upstream_spec_fields("SandboxTemplate") {
        assert!(
            field == "podTemplate" || names(&validations[3], &field),
            "{field}: {}",
            validations[3]
        );
    }
    for words in [
        "automountServiceAccountToken == false",
        "serviceAccountName == 'default'",
        "object.spec.updateStrategy.type == 'Recreate'",
        "!has(object.spec.volumeClaimTemplates[0].spec.volumeName)",
        "!has(object.spec.volumeClaimTemplates[0].spec.dataSourceRef)",
    ] {
        assert!(
            validations[1..=4].iter().any(|held| held.contains(words)),
            "{words}"
        );
    }
    // Every field the pinned upstream gives a pod, a container and their security contexts is
    // named, by what the template sets or by being refused, so a new one fails here.
    let template = upstream_definition("SandboxTemplate");
    let pod = &template["properties"]["podTemplate"]["properties"]["spec"]["properties"];
    for field in fields_of(pod)
        .into_iter()
        .filter(|field| field != "initContainers")
    {
        assert!(
            names(&validations[4], &format!("variables.pod.{field}"))
                || names(&validations[5], &format!("variables.pod.{field}"))
                || names(&validations[6], &format!("variables.pod.{field}")),
            "the pod's {field}"
        );
    }
    for field in fields_of(&pod["securityContext"]["properties"]) {
        assert!(
            names(
                &validations[4],
                &format!("variables.pod.securityContext.{field}")
            ),
            "the pod's securityContext.{field}"
        );
    }
    let container = &pod["containers"]["items"]["properties"];
    for field in fields_of(container) {
        let set_by_it = [
            "args",
            "image",
            "name",
            "terminationMessagePath",
            "terminationMessagePolicy",
        ]
        .contains(&field.as_str());
        assert!(
            set_by_it || names(&validations[6], &format!("c.{field}")),
            "a container's {field}"
        );
    }
    for field in fields_of(&container["securityContext"]["properties"]) {
        assert!(
            names(&validations[6], &format!("c.securityContext.{field}")),
            "a container's securityContext.{field}"
        );
    }
    // A new kept volume's fields too: its size, how it is written and its class alone are free.
    let volume = &upstream_definition("SandboxClaim")["properties"]["volumeClaimTemplates"]["items"]
        ["properties"]["spec"]["properties"];
    for field in fields_of(volume) {
        assert!(
            ["accessModes", "resources", "storageClassName"].contains(&field.as_str())
                || names(&validations[1], &format!("spec.{field}")),
            "a kept volume's {field}"
        );
    }
    // A renewal holds every one of them as it was, but a container's image and resources.
    let renewal = policy_named(&documents, "purlis-operator-changes-only-a-sandboxs-mode")["spec"]
        ["variables"][0]["expression"]
        .as_str()
        .expect("its expression")
        .to_owned();
    for field in fields_of(pod) {
        assert!(
            names(
                &renewal,
                &format!("oldObject.spec.podTemplate.spec.{field}")
            ),
            "the pod's {field}"
        );
    }
    for field in fields_of(container) {
        let may_differ = ["image", "name", "resources"].contains(&field.as_str());
        assert!(
            may_differ || names(&renewal, &format!("o.{field}")),
            "a container's {field}"
        );
    }
    assert!(validations[5].contains("['purlis-link', 'purlis-kept'] && has(v.emptyDir)"));
    for words in [
        "runAsNonRoot == true",
        "allowPrivilegeEscalation == false",
        "drop == ['ALL']",
        "!has(c.securityContext.capabilities.add)",
        "type == 'RuntimeDefault'",
        "!has(c.command)",
        "has(e.valueFrom.fieldRef)",
    ] {
        assert!(
            validations[6].contains(words),
            "{words}: {}",
            validations[6]
        );
    }
}
