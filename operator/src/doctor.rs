// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis-cluster-operator doctor` (#2236): what a cluster lacks before purlis's cluster
//! operator is installed, one line per finding, each naming the next step.
//!
//! It asks the cluster's API server with the credentials of whoever runs it (their kubeconfig's
//! current context) and changes nothing: every request is a read, plus `SelfSubjectAccessReview`,
//! the question `kubectl auth can-i` asks, which the API server answers and keeps nothing of.
//!
//! The checks run against a [`Cluster`]: [`Kube`] is the real one, and tests hand in their own.

use crate::upstream::VERSION;
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::authorization::v1::{
    ResourceAttributes, SelfSubjectAccessReview, SelfSubjectAccessReviewSpec,
};
use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use kube::api::{Api, PostParams};
use std::fmt;
use std::future::Future;

/// The oldest Kubernetes purlis supports (spec #2041): pod user namespaces.
pub const MINIMUM: (u32, u32) = (1, 36);

/// The definitions the pinned upstream release installs, each with the version purlis uses.
pub const UPSTREAM_DEFINITIONS: &[(&str, &str)] = &[
    ("sandboxes.agents.x-k8s.io", "v1beta1"),
    ("sandboxtemplates.extensions.agents.x-k8s.io", "v1beta1"),
    ("sandboxwarmpools.extensions.agents.x-k8s.io", "v1beta1"),
    ("sandboxclaims.extensions.agents.x-k8s.io", "v1beta1"),
];

/// Where upstream's controller runs, as its vendored manifest installs it.
pub const UPSTREAM_CONTROLLER: (&str, &str) = ("agent-sandbox-system", "agent-sandbox-controller");

/// Every cluster-scoped kind the install applies, as (API group, resource): the admin needs to
/// create and patch each (`kubectl apply --server-side`).
pub const CLUSTER_SCOPED: &[(&str, &str)] = &[
    ("", "namespaces"),
    ("apiextensions.k8s.io", "customresourcedefinitions"),
    ("rbac.authorization.k8s.io", "clusterroles"),
    ("rbac.authorization.k8s.io", "clusterrolebindings"),
    (
        "admissionregistration.k8s.io",
        "validatingadmissionpolicies",
    ),
    (
        "admissionregistration.k8s.io",
        "validatingadmissionpolicybindings",
    ),
];

/// The verbs `kubectl apply --server-side` needs on an object it makes or changes.
const APPLY: [&str; 2] = ["create", "patch"];

/// The Kubernetes version the API server reports.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Version {
    pub major: String,
    pub minor: String,
    pub git_version: String,
}

/// A question the cluster did not answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unanswered {
    /// The person running doctor may not read it.
    Forbidden,
    /// Anything else: the API server unreachable, a credential refused, a body not understood.
    Failed(String),
}

/// What the checks ask of a cluster. Every question only reads.
pub trait Cluster {
    /// The API server's Kubernetes version.
    fn version(&self) -> impl Future<Output = Result<Version, Unanswered>> + Send;
    /// The versions a custom resource definition serves, or `None` when there is no such
    /// definition.
    fn definition(
        &self,
        name: &str,
    ) -> impl Future<Output = Result<Option<Vec<String>>, Unanswered>> + Send;
    /// The images of upstream's controller ([`UPSTREAM_CONTROLLER`]), or `None` when it is not
    /// installed.
    fn controller_images(
        &self,
    ) -> impl Future<Output = Result<Option<Vec<String>>, Unanswered>> + Send;
    /// Whether the person running doctor may `verb` a cluster-scoped `resource` of `group`.
    fn may(
        &self,
        verb: &str,
        group: &str,
        resource: &str,
    ) -> impl Future<Output = Result<bool, Unanswered>> + Send;
}

/// One thing the cluster lacks, and the next step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finding {
    pub lacks: String,
    pub next: String,
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "missing: {}. Next: {}.", self.lacks, self.next)
    }
}

/// What doctor found: what holds, and what the cluster lacks.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub holds: Vec<String>,
    pub findings: Vec<Finding>,
}

impl Report {
    /// Whether the cluster lacks anything, which is doctor's exit status.
    pub fn lacks_anything(&self) -> bool {
        !self.findings.is_empty()
    }

    fn lacks(&mut self, lacks: impl Into<String>, next: impl Into<String>) {
        self.findings.push(Finding {
            lacks: lacks.into(),
            next: next.into(),
        });
    }
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for line in &self.holds {
            writeln!(f, "ok: {line}")?;
        }
        for finding in &self.findings {
            writeln!(f, "{finding}")?;
        }
        if self.findings.is_empty() {
            write!(
                f,
                "nothing missing: install the operator as docs/cluster-operator.md says"
            )
        } else {
            write!(
                f,
                "{} missing: install the operator once doctor finds nothing missing",
                self.findings.len()
            )
        }
    }
}

fn manifest() -> String {
    format!("operator/vendor/agent-sandbox/{VERSION}/sandbox-with-extensions.yaml")
}

fn apply_upstream() -> String {
    format!(
        "apply it, step 1 of the install: kubectl apply --server-side -f {}",
        manifest()
    )
}

fn ask_again(error: &str) -> String {
    format!(
        "the cluster answered \"{error}\"; check that kubectl version answers for this context \
         (kubectl config current-context), then run doctor again"
    )
}

/// Every check, in order: Kubernetes, upstream agent-sandbox, the rights to apply the install.
pub async fn check(cluster: &impl Cluster) -> Report {
    let mut report = Report::default();
    match cluster.version().await {
        Ok(version) => kubernetes(&mut report, &version),
        Err(Unanswered::Forbidden) => report.lacks(
            "permission to read the cluster's Kubernetes version",
            "ask a cluster admin to run doctor, or for the system:discovery role",
        ),
        // Nothing else would be answered either: one line, not a guess per check.
        Err(Unanswered::Failed(error)) => {
            report.lacks("an answer from the cluster's API server", ask_again(&error));
            return report;
        }
    }
    upstream(&mut report, cluster).await;
    rights(&mut report, cluster).await;
    report
}

fn leading_number(text: &str) -> Option<u32> {
    let digits: String = text.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

fn kubernetes(report: &mut Report, version: &Version) {
    let (major, minor) = MINIMUM;
    let wanted = format!("Kubernetes {major}.{minor} or later");
    let shown = if version.git_version.is_empty() {
        format!("{}.{}", version.major, version.minor)
    } else {
        version.git_version.clone()
    };
    match (
        leading_number(&version.major),
        leading_number(&version.minor),
    ) {
        (Some(has_major), Some(has_minor)) if (has_major, has_minor) >= MINIMUM => {
            report.holds.push(format!("Kubernetes {shown}"));
        }
        (Some(_), Some(_)) => report.lacks(
            format!("{wanted}: this cluster runs {shown}"),
            format!(
                "upgrade the cluster to {wanted}; its pod user namespaces are what runners need"
            ),
        ),
        _ => report.lacks(
            format!("{wanted}: the API server reports a version doctor cannot read, \"{shown}\""),
            "check the version with kubectl version, and upgrade to 1.36 or later if it is older",
        ),
    }
}

async fn upstream(report: &mut Report, cluster: &impl Cluster) {
    let wanted = format!("upstream agent-sandbox {VERSION}");
    let mut present = 0;
    let mut absent = Vec::new();
    for &(name, version) in UPSTREAM_DEFINITIONS {
        match cluster.definition(name).await {
            Ok(Some(served)) => {
                present += 1;
                if !served.iter().any(|v| v == version) {
                    absent.push(format!("{name} serving {version}"));
                }
            }
            Ok(None) => absent.push(name.to_owned()),
            Err(Unanswered::Forbidden) => {
                report.lacks(
                    format!("permission to read customresourcedefinitions, to see whether {wanted} is installed"),
                    "run doctor as a cluster admin, or ask one for get on customresourcedefinitions.apiextensions.k8s.io",
                );
                return;
            }
            Err(Unanswered::Failed(error)) => {
                report.lacks(format!("an answer about {name}"), ask_again(&error));
                return;
            }
        }
    }
    let (namespace, name) = UPSTREAM_CONTROLLER;
    let images = match cluster.controller_images().await {
        Ok(images) => images,
        Err(Unanswered::Forbidden) => {
            report.lacks(
                format!("permission to read deployments in {namespace}, to see which release of upstream agent-sandbox runs"),
                format!("run doctor as a cluster admin, or ask one for get on deployments.apps in {namespace}"),
            );
            return;
        }
        Err(Unanswered::Failed(error)) => {
            report.lacks(
                format!("an answer about deployment {namespace}/{name}"),
                ask_again(&error),
            );
            return;
        }
    };
    let Some(images) = images else {
        if present == 0 {
            report.lacks(format!("{wanted}: not installed"), apply_upstream());
        } else {
            absent.push(format!("its controller, deployment {namespace}/{name}"));
            report.lacks(
                format!("{wanted}: installed in part, without {}", absent.join(", ")),
                apply_upstream(),
            );
        }
        return;
    };
    if !absent.is_empty() {
        report.lacks(
            format!("{wanted}: installed in part, without {}", absent.join(", ")),
            apply_upstream(),
        );
        return;
    }
    let image = images
        .iter()
        .find(|image| image.contains(name))
        .or(images.first())
        .cloned()
        .unwrap_or_default();
    match tag_of(&image) {
        Some(tag) if tag == VERSION => report.holds.push(wanted),
        Some(tag) => report.lacks(
            format!("{wanted}: the cluster runs {tag}, and purlis is tested against {VERSION} only"),
            format!(
                "check with whoever installed {tag} that nothing else needs it, then apply the pinned release over it: \
                 kubectl apply --server-side -f {}",
                manifest()
            ),
        ),
        None => report.lacks(
            format!("{wanted}: doctor cannot tell which release runs, since the controller's image \"{image}\" names no tag"),
            format!(
                "apply the pinned release, whose controller image is tagged {VERSION}: kubectl apply --server-side -f {}",
                manifest()
            ),
        ),
    }
}

/// The tag of an image reference, `registry/repo:tag` or `registry/repo:tag@sha256:…`.
fn tag_of(image: &str) -> Option<&str> {
    let named = image.split('@').next().unwrap_or(image);
    let (_, tag) = named.rsplit_once(':')?;
    (!tag.contains('/')).then_some(tag)
}

async fn rights(report: &mut Report, cluster: &impl Cluster) {
    let mut denied = Vec::new();
    for &(group, resource) in CLUSTER_SCOPED {
        let what = if group.is_empty() {
            resource.to_owned()
        } else {
            format!("{resource}.{group}")
        };
        for verb in APPLY {
            match cluster.may(verb, group, resource).await {
                Ok(true) => {}
                Ok(false) | Err(Unanswered::Forbidden) => denied.push(format!("{verb} {what}")),
                Err(Unanswered::Failed(error)) => {
                    report.lacks(
                        format!("an answer to whether you may {verb} {what}"),
                        ask_again(&error),
                    );
                    return;
                }
            }
        }
    }
    match denied.first() {
        None => report
            .holds
            .push("you may apply the install's cluster-scoped definitions".to_owned()),
        Some(first) => report.lacks(
            format!(
                "permission to apply the install's cluster-scoped definitions: you may not {}",
                denied.join(", ")
            ),
            format!(
                "run doctor as a cluster admin, or ask one for these rights; check each with kubectl auth can-i {first}"
            ),
        ),
    }
}

/// The real cluster, through a kube client with the credentials of whoever runs doctor.
pub struct Kube(pub kube::Client);

fn unanswered(error: kube::Error) -> Unanswered {
    match &error {
        kube::Error::Api(status) if status.code == 403 => Unanswered::Forbidden,
        _ => Unanswered::Failed(error.to_string()),
    }
}

impl Cluster for Kube {
    async fn version(&self) -> Result<Version, Unanswered> {
        let info = self.0.apiserver_version().await.map_err(unanswered)?;
        Ok(Version {
            major: info.major,
            minor: info.minor,
            git_version: info.git_version,
        })
    }

    async fn definition(&self, name: &str) -> Result<Option<Vec<String>>, Unanswered> {
        let definitions = Api::<CustomResourceDefinition>::all(self.0.clone());
        let found = definitions.get_opt(name).await.map_err(unanswered)?;
        Ok(found.map(|definition| {
            definition
                .spec
                .versions
                .into_iter()
                .filter(|version| version.served)
                .map(|version| version.name)
                .collect()
        }))
    }

    async fn controller_images(&self) -> Result<Option<Vec<String>>, Unanswered> {
        let (namespace, name) = UPSTREAM_CONTROLLER;
        let deployments = Api::<Deployment>::namespaced(self.0.clone(), namespace);
        let found = deployments.get_opt(name).await.map_err(unanswered)?;
        Ok(found.map(|deployment| {
            deployment
                .spec
                .and_then(|spec| spec.template.spec)
                .map(|pod| pod.containers.into_iter().filter_map(|c| c.image).collect())
                .unwrap_or_default()
        }))
    }

    async fn may(&self, verb: &str, group: &str, resource: &str) -> Result<bool, Unanswered> {
        let review = SelfSubjectAccessReview {
            spec: SelfSubjectAccessReviewSpec {
                resource_attributes: Some(ResourceAttributes {
                    verb: Some(verb.to_owned()),
                    group: Some(group.to_owned()),
                    resource: Some(resource.to_owned()),
                    ..ResourceAttributes::default()
                }),
                non_resource_attributes: None,
            },
            ..SelfSubjectAccessReview::default()
        };
        let reviews = Api::<SelfSubjectAccessReview>::all(self.0.clone());
        let answered = reviews
            .create(&PostParams::default(), &review)
            .await
            .map_err(unanswered)?;
        Ok(answered.status.is_some_and(|status| status.allowed))
    }
}
