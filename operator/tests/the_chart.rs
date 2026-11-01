// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The operator's Helm chart, `charts/purlis-cluster-operator/`, rendered by `helm` itself: the
//! plain manifests in `deploy/` are what it renders, so the two never disagree, and no value it
//! takes but `forge.keySecretNames` widens what the operator may do, or names its image by anything but a digest.
//!
//! Needs `helm` (v4) on PATH. When the chart changes, regenerate the plain manifests with
//! `PURLIS_OPERATOR_BLESS=1 cargo test --test the_chart` and read their diff.

mod common;

use serde_json::{Value, json};
use std::path::PathBuf;
use std::process::Command;

/// The digest the plain manifests name until an install sets the real one: one no registry
/// holds, so a manifest applied without it starts nothing.
macro_rules! placeholder_digest {
    () => {
        "sha256:0000000000000000000000000000000000000000000000000000000000000000"
    };
}
/// A digest the tests install with.
macro_rules! digest {
    () => {
        "sha256:1111111111111111111111111111111111111111111111111111111111111111"
    };
}

const PLACEHOLDER_DIGEST: &str = placeholder_digest!();
/// The image the plain manifests name. `tools/operator-install.mjs` replaces exactly this line,
/// for `e2e/kind.sh` and `.github/workflows/operator-image.yml` alike (#2281).
const PLACEHOLDER: &str = concat!(
    "ghcr.io/purlis/purlis-cluster-operator@",
    placeholder_digest!()
);
const DIGEST: &str = digest!();
const DIGEST_VALUE: &str = concat!("image.digest=", digest!());
/// The oldest Kubernetes purlis supports (spec #2041), which the chart requires.
const KUBERNETES: &str = "1.36.0";

fn operator_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn chart() -> PathBuf {
    operator_dir()
        .join("charts")
        .join("purlis-cluster-operator")
}

fn deploy(file: &str) -> PathBuf {
    operator_dir().join("deploy").join(file)
}

/// `helm` with homes of the test's own, so the person's own Helm settings are never read or
/// written.
fn helm(args: &[&str]) -> std::process::Output {
    let home = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("helm-home");
    Command::new("helm")
        .args(args)
        .env("HELM_CACHE_HOME", home.join("cache"))
        .env("HELM_CONFIG_HOME", home.join("config"))
        .env("HELM_DATA_HOME", home.join("data"))
        .output()
        .unwrap_or_else(|error| {
            panic!("helm (v4) is needed on PATH for the chart's tests: {error}")
        })
}

/// What `helm template` renders for the release `purlis-cluster-operator` in `purlis-system`
/// with these `--set` values, or what helm said when it refused.
fn render(values: &[&str]) -> Result<String, String> {
    let chart = chart();
    let mut args = vec![
        "template",
        "purlis-cluster-operator",
        chart.to_str().expect("a UTF-8 path"),
        "--namespace",
        "purlis-system",
        "--kube-version",
        KUBERNETES,
    ];
    for value in values {
        args.push("--set");
        args.push(value);
    }
    let out = helm(&args);
    if out.status.success() {
        Ok(String::from_utf8(out.stdout).expect("UTF-8"))
    } else {
        Err(String::from_utf8_lossy(&out.stderr).into_owned())
    }
}

fn documents_of(text: &str) -> Vec<Value> {
    common::yaml_text_documents(text)
}

fn rendered(values: &[&str]) -> Vec<Value> {
    documents_of(&render(values).unwrap_or_else(|error| panic!("helm template: {error}")))
}

/// These `--set` values, with the image's digest set.
fn with_digest<'a>(values: &[&'a str]) -> Vec<&'a str> {
    let mut all = vec![DIGEST_VALUE];
    all.extend_from_slice(values);
    all
}

fn is_policy(document: &Value) -> bool {
    matches!(
        document["kind"].as_str(),
        Some("ValidatingAdmissionPolicy" | "ValidatingAdmissionPolicyBinding")
    )
}

const OPERATOR_HEADER: &str = "# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0

# purlis's cluster operator: its namespace, its service account, what it may do, the one pod
# that runs it, its metrics Service and the NetworkPolicy that lets only the scrapers the chart's
# values name reach that pod (none by default). Applied after upstream agent-sandbox
# (vendor/agent-sandbox/), the policy (policy.yaml) and purlis's definitions (crds.yaml);
# docs/cluster-operator.md says how.
#
# Rendered from the chart, charts/purlis-cluster-operator/, with its default values; never
# edited by hand. Regenerate with
#   PURLIS_OPERATOR_BLESS=1 cargo test --test the_chart
#
# Its image is a placeholder digest no registry holds: install it with the image set to the
# operator's digest (docs/cluster-operator.md).
#
# What it may do is the ClusterRole purlis-cluster-operator below and nothing else. It holds no
# exec or attach right to a pod itself and none to read a Secret, and it only reads a runner's
# pod by name, for the link key it vouches for: upstream's controller makes the runners' pods.
# It makes each person's runner namespace and binds purlis-runner-owner, only to that person,
# only in runner namespaces, as the policy purlis-runner-namespaces (policy.yaml) holds at
# admission (#2055). Its own signing key is a Secret the kubelet mounts in its pod, made before
# it is installed (docs/cluster-operator.md). The chart's forge.keySecretNames alone adds a Role
# that reads the GitHub App keys it names (#4204); this default install names none.
# operator/tests/the_install_manifests.rs holds every rule.
---
# The namespace the chart is installed in, which Helm leaves to the person: only pods that meet
# the restricted Pod Security Standard are admitted in it.
apiVersion: v1
kind: Namespace
metadata:
  name: purlis-system
  labels:
    pod-security.kubernetes.io/enforce: restricted
";

const POLICY_HEADER: &str = "# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0

# Who may use a Pool is the cluster's own access control on it: a Claim is admitted only when
# the person making it may `use` the Pool it names, asked of the cluster's authorizer as that
# person. Applied before purlis's definitions (crds.yaml), so no Claim is admitted before it
# holds, and before the operator.
#
# Rendered from the chart, charts/purlis-cluster-operator/; never edited by hand. Regenerate
# with
#   PURLIS_OPERATOR_BLESS=1 cargo test --test the_chart
#
# A cluster admin grants `use` with RBAC, on one pool or on all of them:
#
#   apiGroups: [runners.purlis.dev]
#   resources: [pools]
#   resourceNames: [team]
#   verbs: [use]
#
# A Claim's pool never changes after it is made (crds.yaml), so creating one is the only time
# this is asked. operator/tests/the_install_manifests.rs holds it, and the kind tests check that
# the cluster refuses a Claim from someone who may not use its pool.
#
# Upstream's claims on a purlis pool's warm pool, and warm pools of its template, are made by
# the operator alone, so a runner from a pool is had only through a Claim (#2055).
#
# The second policy, purlis-runner-namespaces (#2055): only the operator makes a runner
# namespace (purlis-runners-*) or changes who it is for, and the operator binds
# purlis-runner-owner, and makes the quota purlis-person, in a runner namespace alone, for the
# one person that namespace is for, who is not the operator.
#
# The third policy, purlis-operator-changes-only-a-sandboxs-mode, narrows the operator's own
# `patch` on upstream's sandboxes (#2058), which RBAC cannot scope to a field or an owner: the
# operator may change only a Sandbox's spec.operatingMode, and only on a Sandbox a SandboxClaim
# controls; and its spec.podTemplate only while it is Suspended in a runner namespace (#3982).
#
# The fourth policy, purlis-forge (#4206): whoever writes a Pool's spec.forge needs `use` on the
# key it names (forgekeys), and only a Claim's requester changes the repos it names. The operator
# mints only for a Claim and a Pool made once its binding has settled.
#
# The fifth policy, purlis-operator-deletes-only-a-retained-claim (#3981): the operator deletes
# only a kept Claim, not stopped, that carries its own RetentionEnded condition.
#
# The sixth policy, purlis-only-the-operator-writes-a-claims-status (#3981): what the operator acts
# on is what a Claim's status says, so no one else writes it.
#
# The seventh policy, purlis-operator-writes-only-a-runners-shape (#3754): the operator writes
# upstream's templates, claims and warm pools in a runner namespace alone, a template's pod only in
# purlis's runner shape.
";

/// The plain manifests the chart's default render makes: `operator.yaml` and `policy.yaml`.
fn plain_manifests() -> (String, String) {
    let text = render(&[&format!("image.digest={PLACEHOLDER_DIGEST}")])
        .unwrap_or_else(|error| panic!("helm template: {error}"));
    let mut operator = OPERATOR_HEADER.to_owned();
    let mut policy = POLICY_HEADER.to_owned();
    for document in text.split("\n---\n") {
        let document = document.trim_start_matches("---\n").trim();
        if document.is_empty() {
            continue;
        }
        let parsed = documents_of(document);
        let target = if parsed.iter().any(is_policy) {
            &mut policy
        } else {
            &mut operator
        };
        target.push_str("---\n");
        target.push_str(document);
        target.push('\n');
    }
    (operator, policy)
}

#[test]
fn the_plain_manifests_are_what_the_chart_renders() {
    let (operator, policy) = plain_manifests();
    if std::env::var_os("PURLIS_OPERATOR_BLESS").is_some() {
        std::fs::write(deploy("operator.yaml"), &operator).expect("written");
        std::fs::write(deploy("policy.yaml"), &policy).expect("written");
    }
    for (file, wanted) in [("operator.yaml", operator), ("policy.yaml", policy)] {
        let held = common::yaml_documents(&deploy(file));
        assert!(
            held == documents_of(&wanted),
            "deploy/{file} is not what the chart renders; regenerate it with \
             PURLIS_OPERATOR_BLESS=1 cargo test --test the_chart"
        );
    }
}

#[test]
fn the_plain_operator_manifest_names_the_placeholder_image_once() {
    let text = std::fs::read_to_string(deploy("operator.yaml")).expect("operator.yaml");
    let line = format!("          image: {PLACEHOLDER}");
    assert_eq!(
        text.lines().filter(|l| *l == line).count(),
        1,
        "tools/operator-install.mjs replaces exactly `{line}`"
    );
}

#[test]
fn the_charts_definitions_are_the_plain_ones() {
    let plain = std::fs::read_to_string(deploy("crds.yaml")).expect("deploy/crds.yaml");
    let path = chart().join("crds").join("crds.yaml");
    if std::env::var_os("PURLIS_OPERATOR_BLESS").is_some() {
        std::fs::write(&path, &plain).expect("written");
    }
    let held = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        held == plain,
        "the chart's crds/crds.yaml is not deploy/crds.yaml; copy it with \
         PURLIS_OPERATOR_BLESS=1 cargo test --test the_chart"
    );
}

#[test]
fn the_definitions_are_left_out_of_the_charts_templates() {
    for document in rendered(&with_digest(&[])) {
        assert_ne!(document["kind"], "CustomResourceDefinition", "{document}");
    }
}

/// The operator's pod, as the chart's Deployment holds it.
fn pod(documents: &[Value]) -> Value {
    let deployment = documents
        .iter()
        .find(|d| d["kind"] == "Deployment")
        .expect("the operator's deployment");
    deployment["spec"]["template"]["spec"].clone()
}

fn container(documents: &[Value]) -> Value {
    pod(documents)["containers"][0].clone()
}

#[test]
fn the_image_is_named_by_its_digest() {
    let documents = rendered(&with_digest(&[]));
    assert_eq!(
        container(&documents)["image"],
        format!("ghcr.io/purlis/purlis-cluster-operator@{DIGEST}")
    );
    let documents = rendered(&with_digest(&["image.repository=localhost:5001/operator"]));
    assert_eq!(
        container(&documents)["image"],
        format!("localhost:5001/operator@{DIGEST}")
    );
}

#[test]
fn an_install_without_a_digest_is_refused() {
    let error = render(&[]).expect_err("no digest, no render");
    assert!(error.contains("image.digest"), "{error}");
}

#[test]
fn a_tag_is_never_taken_for_a_digest() {
    for values in [
        vec!["image.digest=main"],
        vec!["image.digest=sha256:abc"],
        with_digest(&["image.repository=ghcr.io/purlis/purlis-cluster-operator:main"]),
        with_digest(&["image.repository=ghcr.io/purlis/purlis-cluster-operator@sha256:x"]),
    ] {
        render(&values).expect_err(&format!("refused: {values:?}"));
    }
}

fn access(documents: &[Value]) -> Vec<Value> {
    documents
        .iter()
        .filter(|d| {
            matches!(
                d["kind"].as_str(),
                Some("ClusterRole" | "ClusterRoleBinding" | "Role" | "RoleBinding")
            )
        })
        .cloned()
        .collect()
}

#[test]
fn no_value_but_the_key_names_changes_what_the_operator_may_do() {
    let plain = access(&rendered(&with_digest(&[])));
    assert_eq!(
        plain.len(),
        3,
        "the operator's ClusterRole and its binding, and purlis-runner-owner: {plain:?}"
    );
    let every_value = rendered(&with_digest(&[
        "image.repository=localhost:5001/operator",
        "imagePullSecrets[0].name=ghcr",
        "resources.requests.cpu=50m",
        "resources.limits.memory=1Gi",
        "perPerson.quota.claims=50",
        "perPerson.quota.cpu=64",
        "perPerson.quota.memory=null",
    ]));
    assert_eq!(access(&every_value), plain);
}

#[test]
fn a_value_the_chart_does_not_document_is_refused() {
    for value in [
        "rbac.create=false",
        "serviceAccount.name=default",
        "securityContext.runAsUser=0",
        "image.tag=main",
    ] {
        let error = render(&with_digest(&[value])).expect_err(value);
        assert!(error.contains("additional"), "{value}: {error}");
    }
}

#[test]
fn the_values_set_only_what_they_name() {
    let documents = rendered(&with_digest(&[
        "imagePullSecrets[0].name=ghcr",
        "resources.requests.cpu=50m",
        "resources.requests.memory=64Mi",
        "resources.limits.memory=1Gi",
    ]));
    let set = pod(&documents);
    assert_eq!(set["imagePullSecrets"], json!([{ "name": "ghcr" }]));
    assert_eq!(
        set["containers"][0]["resources"],
        json!({ "requests": { "cpu": "50m", "memory": "64Mi" }, "limits": { "memory": "1Gi" } })
    );
    let defaults = pod(&rendered(&with_digest(&[])));
    assert!(defaults.get("imagePullSecrets").is_none(), "{defaults}");
}

/// The per-person quota the operator's pod is handed.
fn quota_handed(documents: &[Value]) -> Value {
    let env = &container(documents)["env"];
    let value = env
        .as_array()
        .and_then(|env| {
            env.iter()
                .find(|var| var["name"] == "PURLIS_PER_PERSON_QUOTA")
        })
        .and_then(|var| var["value"].as_str())
        .unwrap_or_else(|| panic!("PURLIS_PER_PERSON_QUOTA in {env}"));
    serde_json::from_str(value).expect("JSON")
}

#[test]
fn the_per_person_quota_reaches_the_operator_as_set() {
    let defaults = quota_handed(&rendered(&with_digest(&[])));
    assert_eq!(
        defaults,
        json!({ "claims": 5, "cpu": "4", "memory": "8Gi", "storage": "50Gi" })
    );
    let quota = purlis_cluster_operator::person::Quota::from_var(Some(&defaults.to_string()))
        .expect("the operator reads the default");
    assert_eq!(quota, purlis_cluster_operator::person::Quota::default());
    let set = quota_handed(&rendered(&with_digest(&[
        "perPerson.quota.claims=2",
        "perPerson.quota.cpu=1500m",
        "perPerson.quota.storage=null",
    ])));
    assert_eq!(set, json!({ "claims": 2, "cpu": "1500m", "memory": "8Gi" }));
    for value in ["perPerson.quota.disks=2", "perPerson.quota.cpu=lots"] {
        render(&with_digest(&[value])).expect_err(value);
    }
}

#[test]
fn everything_the_chart_makes_lives_in_the_release_namespace_or_the_cluster() {
    for document in rendered(&with_digest(&[])) {
        let namespaced = matches!(
            document["kind"].as_str(),
            Some("ServiceAccount" | "Deployment" | "Service" | "NetworkPolicy")
        );
        if namespaced {
            assert_eq!(
                document["metadata"]["namespace"], "purlis-system",
                "{document}"
            );
        } else {
            assert!(
                document["metadata"].get("namespace").is_none(),
                "{document}"
            );
        }
    }
}

#[test]
fn the_chart_lints_clean_under_strict() {
    let chart = chart();
    let out = helm(&[
        "lint",
        "--strict",
        "--kube-version",
        KUBERNETES,
        chart.to_str().expect("a UTF-8 path"),
        "--set",
        DIGEST_VALUE,
    ]);
    assert!(
        out.status.success(),
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn the_charts_app_version_is_the_operators() {
    let chart: Value = serde_yaml::from_str(
        &std::fs::read_to_string(chart().join("Chart.yaml")).expect("Chart.yaml"),
    )
    .expect("YAML");
    assert_eq!(chart["appVersion"], env!("CARGO_PKG_VERSION"));
    assert_eq!(chart["name"], "purlis-cluster-operator");
}

#[test]
fn the_operators_metrics_are_served_on_its_port_through_a_service_of_their_own() {
    let documents = rendered(&with_digest(&[]));
    let deployment = documents
        .iter()
        .find(|d| d["kind"] == "Deployment")
        .expect("the operator's deployment");
    let port = purlis_cluster_operator::metrics::PORT;
    assert_eq!(
        deployment["spec"]["template"]["spec"]["containers"][0]["ports"],
        json!([
            { "name": "metrics", "containerPort": port, "protocol": "TCP" },
            {
                "name": "mint",
                "containerPort": purlis_cluster_operator::mint::PORT,
                "protocol": "TCP",
            },
        ]),
        "{deployment}"
    );
    let service = documents
        .iter()
        .find(|d| d["kind"] == "Service")
        .expect("the metrics Service (#3737)");
    assert_eq!(
        service["metadata"]["name"],
        "purlis-cluster-operator-metrics"
    );
    assert_eq!(
        service["spec"]["selector"], deployment["spec"]["selector"]["matchLabels"],
        "it selects the operator's pod"
    );
    assert_eq!(
        service["spec"]["ports"],
        json!([{ "name": "metrics", "port": port, "targetPort": "metrics", "protocol": "TCP" }]),
        "{service}"
    );
}

/// The NetworkPolicy in front of the operator's pod.
fn network_policy(documents: &[Value]) -> Value {
    documents
        .iter()
        .find(|d| d["kind"] == "NetworkPolicy")
        .cloned()
        .expect("the operator's NetworkPolicy (#3737)")
}

#[test]
fn nothing_reaches_the_operators_pod_unless_the_values_name_who_scrapes_it() {
    let documents = rendered(&with_digest(&[]));
    let deployment = documents
        .iter()
        .find(|d| d["kind"] == "Deployment")
        .expect("the operator's deployment");
    let policy = network_policy(&documents);
    assert_eq!(policy["metadata"]["name"], "purlis-cluster-operator");
    assert_eq!(
        policy["spec"]["podSelector"]["matchLabels"],
        deployment["spec"]["selector"]["matchLabels"]
    );
    assert_eq!(policy["spec"]["policyTypes"], json!(["Ingress"]));
    // No ingress rule at all: nothing may reach the pod.
    assert!(policy["spec"].get("ingress").is_none(), "{policy}");

    let scraped = network_policy(&rendered(&with_digest(&[
        "metrics.allowFrom[0].namespaceSelector.matchLabels.team=monitoring",
    ])));
    assert_eq!(
        scraped["spec"]["ingress"],
        json!([{
            "from": [{ "namespaceSelector": { "matchLabels": { "team": "monitoring" } } }],
            "ports": [{ "port": "metrics", "protocol": "TCP" }],
        }]),
        "{scraped}"
    );
}

/// The mint port opens only where the chart names a key (#4206), and only to runner namespaces.
#[test]
fn only_a_named_key_opens_the_mint_port_and_only_to_runner_namespaces() {
    let mint_service = |documents: &[Value]| {
        documents
            .iter()
            .find(|d| {
                d["kind"] == "Service" && d["metadata"]["name"] == "purlis-cluster-operator-mint"
            })
            .cloned()
    };
    let plain = rendered(&with_digest(&[]));
    assert!(mint_service(&plain).is_none());
    assert!(network_policy(&plain)["spec"].get("ingress").is_none());

    let named = rendered(&with_digest(&["forge.keySecretNames[0]=org-app"]));
    let service = mint_service(&named).expect("the mint Service");
    assert_eq!(service["spec"]["type"], "ClusterIP");
    assert_eq!(
        service["spec"]["ports"],
        json!([{
            "name": "mint",
            "port": purlis_cluster_operator::mint::PORT,
            "targetPort": "mint",
            "protocol": "TCP",
        }])
    );
    assert_eq!(
        network_policy(&named)["spec"]["ingress"],
        json!([{
            "from": [{ "namespaceSelector": { "matchLabels": {
                "app.kubernetes.io/managed-by": "purlis-cluster-operator",
            } } }],
            "ports": [{ "port": "mint", "protocol": "TCP" }],
        }])
    );
}

/// The Role that reads the admin's App key (#4204, ruling Q4), when the values name any Secret.
fn key_reader(documents: &[Value]) -> Vec<&Value> {
    documents
        .iter()
        .filter(|d| {
            d["metadata"]["name"] == "purlis-forge-key-reader"
                && matches!(d["kind"].as_str(), Some("Role" | "RoleBinding"))
        })
        .collect()
}

/// `resourceNames: []` means every name, so no name must mean no Role at all.
#[test]
fn with_no_key_named_the_operator_may_read_no_secret_at_all() {
    let documents = rendered(&with_digest(&[]));
    assert_eq!(key_reader(&documents), Vec::<&Value>::new());
    assert!(
        !render(&with_digest(&[]))
            .unwrap()
            .contains("resourceNames: []")
    );
    let names_none = rendered(&with_digest(&["forge.keySecretNames=null"]));
    assert_eq!(key_reader(&names_none), Vec::<&Value>::new());
}

#[test]
fn a_named_key_is_read_by_get_alone_by_the_operator_alone_in_its_own_namespace() {
    let documents = rendered(&with_digest(&[
        "forge.keySecretNames[0]=org-app",
        "forge.keySecretNames[1]=other-app",
    ]));
    let found = key_reader(&documents);
    assert_eq!(found.len(), 2, "{found:?}");
    let role = found
        .iter()
        .find(|d| d["kind"] == "Role")
        .expect("the Role");
    assert_eq!(role["metadata"]["namespace"], "purlis-system");
    assert_eq!(
        role["rules"],
        json!([{
            "apiGroups": [""],
            "resources": ["secrets"],
            "resourceNames": ["org-app", "other-app"],
            "verbs": ["get"],
        }, {
            "apiGroups": [""],
            "resources": ["services"],
            "resourceNames": ["purlis-cluster-operator-mint"],
            "verbs": ["get"],
        }])
    );
    let binding = found
        .iter()
        .find(|d| d["kind"] == "RoleBinding")
        .expect("the binding");
    assert_eq!(binding["metadata"]["namespace"], "purlis-system");
    assert_eq!(
        binding["roleRef"],
        json!({
            "apiGroup": "rbac.authorization.k8s.io",
            "kind": "Role",
            "name": "purlis-forge-key-reader",
        })
    );
    assert_eq!(
        binding["subjects"],
        json!([{
            "kind": "ServiceAccount",
            "name": "purlis-cluster-operator",
            "namespace": "purlis-system",
        }])
    );
    // Nothing else it may do moves with the value.
    assert_eq!(
        access(&rendered(&with_digest(&[])))
            .into_iter()
            .filter(|d| d["kind"] != "Role" && d["kind"] != "RoleBinding")
            .collect::<Vec<_>>(),
        access(&documents)
            .into_iter()
            .filter(|d| d["kind"] != "Role" && d["kind"] != "RoleBinding")
            .collect::<Vec<_>>(),
    );
}

#[test]
fn the_values_never_name_the_signing_key_or_a_name_rbac_would_read_as_every_name() {
    for bad in [
        "forge.keySecretNames[0]=purlis-cluster-operator-signing-key",
        "forge.keySecretNames[0]=",
        "forge.keySecretNames[0]=*",
        "forge.keySecretNames[0]=Org_App",
    ] {
        assert!(render(&with_digest(&[bad])).is_err(), "{bad} was taken");
    }
    let twice = with_digest(&["forge.keySecretNames[0]=a", "forge.keySecretNames[1]=a"]);
    assert!(render(&twice).is_err(), "one name twice was taken");
    // A check the schema makes, the template makes too: a schema can be skipped.
    let chart = chart();
    let skipped = |name: &str| {
        let name = format!("forge.keySecretNames[0]={name}");
        helm(&[
            "template",
            "purlis-cluster-operator",
            chart.to_str().expect("a UTF-8 path"),
            "--namespace",
            "purlis-system",
            "--kube-version",
            KUBERNETES,
            "--skip-schema-validation",
            "--set",
            DIGEST_VALUE,
            "--set",
            &name,
        ])
    };
    assert!(
        skipped("org-app").status.success(),
        "the control was refused"
    );
    for name in ["purlis-cluster-operator-signing-key", ""] {
        let out = skipped(name);
        assert!(
            !out.status.success(),
            "{name:?} was taken with the schema skipped"
        );
        let said = String::from_utf8_lossy(&out.stderr);
        assert!(said.contains("may not read"), "{said}");
    }
}
