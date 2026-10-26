// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The operator's Helm chart, `charts/purlis-cluster-operator/`, rendered by `helm` itself: the
//! plain manifests in `deploy/` are what it renders, so the two never disagree, and no value it
//! takes widens what the operator may do or names its image by anything but a digest.
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

# purlis's cluster operator: its namespace, its service account, what it may do, and the one
# pod that runs it. Applied after upstream agent-sandbox (vendor/agent-sandbox/), the policy
# (policy.yaml) and purlis's definitions (crds.yaml); docs/cluster-operator.md says how.
#
# Rendered from the chart, charts/purlis-cluster-operator/, with its default values; never
# edited by hand. Regenerate with
#   PURLIS_OPERATOR_BLESS=1 cargo test --test the_chart
#
# Its image is a placeholder digest no registry holds: install it with the image set to the
# operator's digest (docs/cluster-operator.md).
#
# What it may do is the ClusterRole below and nothing else. It holds no right to exec or attach
# to a pod and none to read a Secret, and it only reads a runner's pod by name, for the link key
# it vouches for: upstream's controller makes the runners' pods. Its own signing key is a
# Secret the kubelet mounts in its pod, made before it is installed (docs/cluster-operator.md).
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
fn no_value_changes_what_the_operator_may_do() {
    let plain = access(&rendered(&with_digest(&[])));
    assert_eq!(plain.len(), 2, "one ClusterRole and its binding: {plain:?}");
    let every_value = rendered(&with_digest(&[
        "image.repository=localhost:5001/operator",
        "imagePullSecrets[0].name=ghcr",
        "resources.requests.cpu=50m",
        "resources.limits.memory=1Gi",
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

#[test]
fn everything_the_chart_makes_lives_in_the_release_namespace_or_the_cluster() {
    for document in rendered(&with_digest(&[])) {
        let namespaced = matches!(
            document["kind"].as_str(),
            Some("ServiceAccount" | "Deployment")
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
