// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis-cluster-operator doctor` (#2236): what a cluster lacks before the operator is
//! installed, one line per finding, each naming the next step. These tests hand the checks a
//! cluster of their own making, so no cluster is needed; `on_a_kind_cluster.rs` and
//! `e2e/kind.sh` run the same checks against a real one.

mod common;

use purlis_cluster_operator::doctor::{self, Cluster, Unanswered, Version};
use std::collections::{BTreeMap, BTreeSet};

/// A cluster as the doctor's checks see it: what its API server would answer.
#[derive(Clone)]
struct Fake {
    version: Result<Version, Unanswered>,
    /// Each definition there is, by name, with the versions it serves.
    definitions: BTreeMap<String, Vec<String>>,
    definitions_readable: bool,
    /// The images of upstream's controller, when it is installed.
    controller: Option<Vec<String>>,
    controller_readable: bool,
    /// `verb resource.group` the person may not do.
    denied: BTreeSet<String>,
}

fn version(major: &str, minor: &str) -> Version {
    Version {
        major: major.into(),
        minor: minor.into(),
        git_version: format!("v{major}.{minor}.4"),
    }
}

/// Kubernetes 1.36 with nothing of upstream's installed, and an admin who may do anything.
fn bare() -> Fake {
    Fake {
        version: Ok(version("1", "36")),
        definitions: BTreeMap::new(),
        definitions_readable: true,
        controller: None,
        controller_readable: true,
        denied: BTreeSet::new(),
    }
}

const UPSTREAM_DEFINITIONS: [&str; 4] = [
    "sandboxes.agents.x-k8s.io",
    "sandboxtemplates.extensions.agents.x-k8s.io",
    "sandboxwarmpools.extensions.agents.x-k8s.io",
    "sandboxclaims.extensions.agents.x-k8s.io",
];

/// The pinned upstream release applied, as its vendored manifest makes it.
fn with_upstream(mut fake: Fake, tag: &str) -> Fake {
    for name in UPSTREAM_DEFINITIONS {
        fake.definitions.insert(name.into(), vec!["v1beta1".into()]);
    }
    fake.controller = Some(vec![format!(
        "registry.k8s.io/agent-sandbox/agent-sandbox-controller:{tag}"
    )]);
    fake
}

fn ready() -> Fake {
    with_upstream(bare(), "v1.0.6")
}

impl Cluster for Fake {
    async fn version(&self) -> Result<Version, Unanswered> {
        self.version.clone()
    }

    async fn definition(&self, name: &str) -> Result<Option<Vec<String>>, Unanswered> {
        if !self.definitions_readable {
            return Err(Unanswered::Forbidden);
        }
        Ok(self.definitions.get(name).cloned())
    }

    async fn controller_images(&self) -> Result<Option<Vec<String>>, Unanswered> {
        if !self.controller_readable {
            return Err(Unanswered::Forbidden);
        }
        Ok(self.controller.clone())
    }

    async fn may(&self, verb: &str, group: &str, resource: &str) -> Result<bool, Unanswered> {
        let what = if group.is_empty() {
            format!("{verb} {resource}")
        } else {
            format!("{verb} {resource}.{group}")
        };
        Ok(!self.denied.contains(&what))
    }
}

fn run(cluster: &Fake) -> doctor::Report {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("a runtime")
        .block_on(doctor::check(cluster))
}

/// The one finding whose line mentions `about`.
fn finding<'a>(report: &'a doctor::Report, about: &str) -> &'a doctor::Finding {
    let found: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.to_string().contains(about))
        .collect();
    assert_eq!(found.len(), 1, "one finding about {about}: {report}");
    found[0]
}

#[test]
fn a_cluster_with_the_pinned_upstream_lacks_nothing() {
    let report = run(&ready());
    assert!(report.findings.is_empty(), "{report}");
    assert!(!report.lacks_anything());
    assert!(report.to_string().contains("nothing missing"), "{report}");
}

#[test]
fn a_cluster_older_than_1_36_is_named_with_its_version() {
    let mut cluster = ready();
    cluster.version = Ok(version("1", "35"));
    let report = run(&cluster);
    assert!(report.lacks_anything());
    let line = finding(&report, "Kubernetes 1.36").to_string();
    assert!(line.contains("v1.35.4"), "{line}");
    assert!(line.contains("upgrade"), "{line}");
}

#[test]
fn a_provider_minor_version_with_a_plus_is_read_as_its_number() {
    let mut cluster = ready();
    cluster.version = Ok(version("1", "37+"));
    assert!(run(&cluster).findings.is_empty());
}

#[test]
fn a_version_it_cannot_read_is_a_finding_not_a_pass() {
    let mut cluster = ready();
    cluster.version = Ok(version("", ""));
    let report = run(&cluster);
    finding(&report, "Kubernetes 1.36");
}

#[test]
fn a_cluster_without_upstream_names_it_and_the_manifest_to_apply() {
    let report = run(&bare());
    assert_eq!(report.findings.len(), 1, "{report}");
    let line = finding(&report, "agent-sandbox v1.0.6").to_string();
    assert!(line.contains("not installed"), "{line}");
    assert!(
        line.contains(
            "kubectl apply --server-side -f operator/vendor/agent-sandbox/v1.0.6/sandbox-with-extensions.yaml"
        ),
        "{line}"
    );
}

#[test]
fn another_upstream_release_is_named_with_the_one_purlis_is_tested_against() {
    let report = run(&with_upstream(bare(), "v1.1.0"));
    let line = finding(&report, "agent-sandbox v1.0.6").to_string();
    assert!(line.contains("v1.1.0"), "{line}");
    assert!(line.contains("sandbox-with-extensions.yaml"), "{line}");
}

#[test]
fn an_upstream_install_missing_a_definition_names_what_is_missing() {
    let mut cluster = ready();
    cluster
        .definitions
        .remove("sandboxwarmpools.extensions.agents.x-k8s.io");
    let report = run(&cluster);
    let line = finding(&report, "agent-sandbox v1.0.6").to_string();
    assert!(
        line.contains("sandboxwarmpools.extensions.agents.x-k8s.io"),
        "{line}"
    );
}

#[test]
fn a_definition_that_does_not_serve_the_version_the_operator_uses_is_missing() {
    let mut cluster = ready();
    cluster.definitions.insert(
        "sandboxclaims.extensions.agents.x-k8s.io".into(),
        vec!["v1alpha1".into()],
    );
    let report = run(&cluster);
    let line = finding(&report, "agent-sandbox v1.0.6").to_string();
    assert!(
        line.contains("sandboxclaims.extensions.agents.x-k8s.io"),
        "{line}"
    );
}

#[test]
fn upstream_definitions_without_its_controller_name_the_controller() {
    let mut cluster = ready();
    cluster.controller = None;
    let report = run(&cluster);
    let line = finding(&report, "agent-sandbox v1.0.6").to_string();
    assert!(line.contains("agent-sandbox-controller"), "{line}");
}

#[test]
fn a_controller_named_by_digest_alone_is_one_it_cannot_tell_the_release_of() {
    let mut cluster = ready();
    cluster.controller = Some(vec![
        "registry.k8s.io/agent-sandbox/agent-sandbox-controller@sha256:0123".into(),
    ]);
    let report = run(&cluster);
    let line = finding(&report, "agent-sandbox v1.0.6").to_string();
    assert!(line.contains("cannot tell"), "{line}");

    // A mirror's port is not a tag.
    cluster.controller = Some(vec![
        "mirror.example:5000/agent-sandbox-controller@sha256:0123".into(),
    ]);
    let report = run(&cluster);
    let line = finding(&report, "agent-sandbox v1.0.6").to_string();
    assert!(line.contains("cannot tell"), "{line}");
}

#[test]
fn a_controller_tagged_and_pinned_by_digest_is_read_by_its_tag() {
    let mut cluster = ready();
    cluster.controller = Some(vec![
        "registry.k8s.io/agent-sandbox/agent-sandbox-controller:v1.0.6@sha256:0123".into(),
    ]);
    assert!(run(&cluster).findings.is_empty());
}

#[test]
fn what_the_admin_may_not_read_is_a_finding_with_who_to_ask() {
    let mut cluster = ready();
    cluster.definitions_readable = false;
    let report = run(&cluster);
    let line = finding(&report, "customresourcedefinitions").to_string();
    assert!(line.contains("read"), "{line}");
    assert!(line.contains("cluster admin"), "{line}");

    let mut cluster = ready();
    cluster.controller_readable = false;
    let report = run(&cluster);
    let line = finding(&report, "deployments").to_string();
    assert!(line.contains("agent-sandbox-system"), "{line}");
}

#[test]
fn an_admin_who_may_not_apply_cluster_scoped_definitions_is_told_which() {
    let mut cluster = ready();
    cluster
        .denied
        .insert("create customresourcedefinitions.apiextensions.k8s.io".into());
    cluster
        .denied
        .insert("patch clusterroles.rbac.authorization.k8s.io".into());
    let report = run(&cluster);
    assert_eq!(report.findings.len(), 1, "{report}");
    let line = finding(&report, "cluster-scoped").to_string();
    assert!(
        line.contains("create customresourcedefinitions.apiextensions.k8s.io"),
        "{line}"
    );
    assert!(
        line.contains("patch clusterroles.rbac.authorization.k8s.io"),
        "{line}"
    );
    assert!(line.contains("kubectl auth can-i"), "{line}");
}

#[test]
fn an_unreachable_cluster_is_one_finding_and_no_guesses() {
    let mut cluster = ready();
    cluster.version = Err(Unanswered::Failed("connection refused".into()));
    let report = run(&cluster);
    assert_eq!(report.findings.len(), 1, "{report}");
    let line = report.findings[0].to_string();
    assert!(line.contains("connection refused"), "{line}");
    assert!(line.contains("kubectl"), "{line}");
}

#[test]
fn every_finding_is_one_line_naming_the_next_step() {
    let mut cluster = with_upstream(bare(), "v1.1.0");
    cluster.version = Ok(version("1", "30"));
    cluster.denied.insert("create namespaces".into());
    let report = run(&cluster);
    assert_eq!(report.findings.len(), 3, "{report}");
    for finding in &report.findings {
        let line = finding.to_string();
        assert!(!line.contains('\n'), "{line:?}");
        assert!(line.starts_with("missing: "), "{line}");
        assert!(line.contains(" Next: "), "{line}");
    }
}

/// The cluster-scoped kinds the install applies are the ones whose rights the doctor asks for:
/// read from the manifests themselves, so a new kind in them is a kind the doctor checks.
#[test]
fn it_asks_for_the_rights_to_every_cluster_scoped_kind_the_install_applies() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let manifests = [
        root.join("vendor/agent-sandbox/v1.0.6/sandbox-with-extensions.yaml"),
        root.join("deploy/crds.yaml"),
        root.join("deploy/policy.yaml"),
        root.join("deploy/operator.yaml"),
    ];
    // Which kinds Kubernetes keeps cluster-wide, from its API reference.
    let cluster_scoped: BTreeMap<&str, (&str, &str)> = [
        ("Namespace", ("", "namespaces")),
        (
            "CustomResourceDefinition",
            ("apiextensions.k8s.io", "customresourcedefinitions"),
        ),
        ("ClusterRole", ("rbac.authorization.k8s.io", "clusterroles")),
        (
            "ClusterRoleBinding",
            ("rbac.authorization.k8s.io", "clusterrolebindings"),
        ),
        (
            "ValidatingAdmissionPolicy",
            (
                "admissionregistration.k8s.io",
                "validatingadmissionpolicies",
            ),
        ),
        (
            "ValidatingAdmissionPolicyBinding",
            (
                "admissionregistration.k8s.io",
                "validatingadmissionpolicybindings",
            ),
        ),
    ]
    .into();
    let namespaced = [
        "ServiceAccount",
        "Deployment",
        "Service",
        "ConfigMap",
        "Role",
        "RoleBinding",
    ];
    let mut wanted = BTreeSet::new();
    for path in manifests {
        for document in common::yaml_documents(&path) {
            let kind = document["kind"].as_str().expect("a kind");
            match cluster_scoped.get(kind) {
                Some(&(group, resource)) => {
                    wanted.insert((group.to_owned(), resource.to_owned()));
                }
                None => assert!(
                    namespaced.contains(&kind),
                    "{kind} in {}: is it cluster-scoped?",
                    path.display()
                ),
            }
        }
    }
    let asked: BTreeSet<_> = doctor::CLUSTER_SCOPED
        .iter()
        .map(|&(group, resource)| (group.to_owned(), resource.to_owned()))
        .collect();
    assert_eq!(asked, wanted);
}
