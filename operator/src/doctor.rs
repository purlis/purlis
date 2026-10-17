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

use crate::upstream::{CONTROLLER_DIGEST, VERSION};
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::authorization::v1::{
    ResourceAttributes, SelfSubjectAccessReview, SelfSubjectAccessReviewSpec,
};
use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use kube::api::{Api, PostParams};
use kube::config::Kubeconfig;
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
/// create and patch each (`kubectl apply --server-side`), and [`ROLE_RIGHTS`] besides.
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

/// What Kubernetes asks besides, of someone applying cluster roles and their bindings: `escalate`
/// to write a cluster role granting rights they do not hold, `bind` to bind one. Without them the
/// install's cluster roles are refused unless the admin already holds every right they grant.
pub const ROLE_RIGHTS: &[(&str, &str, &str)] = &[
    ("escalate", "rbac.authorization.k8s.io", "clusterroles"),
    ("bind", "rbac.authorization.k8s.io", "clusterroles"),
];

/// The Kubernetes version the API server reports.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Version {
    pub major: String,
    pub minor: String,
    pub git_version: String,
}

/// Upstream's controller ([`UPSTREAM_CONTROLLER`]), as its deployment shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Controller {
    /// The images of its pod's containers.
    pub images: Vec<String>,
    /// How many of its replicas are ready.
    pub ready: i32,
}

/// A question the cluster did not answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unanswered {
    /// The person running doctor may not ask it.
    Forbidden,
    /// Anything else: the API server unreachable, a credential refused, a body not understood.
    Failed(String),
}

/// What the checks ask of a cluster. Every question only reads.
pub trait Cluster {
    /// Which cluster is asked, as the person knows it: its context and API server.
    fn asked(&self) -> String;
    /// The API server's Kubernetes version.
    fn version(&self) -> impl Future<Output = Result<Version, Unanswered>> + Send;
    /// The versions a custom resource definition serves, or `None` when there is no such
    /// definition.
    fn definition(
        &self,
        name: &str,
    ) -> impl Future<Output = Result<Option<Vec<String>>, Unanswered>> + Send;
    /// Upstream's controller, or `None` when it is not installed.
    fn controller(&self) -> impl Future<Output = Result<Option<Controller>, Unanswered>> + Send;
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

impl Finding {
    pub fn new(lacks: impl Into<String>, next: impl Into<String>) -> Self {
        Finding {
            lacks: lacks.into(),
            next: next.into(),
        }
    }
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
        self.findings.push(Finding::new(lacks, next));
    }

    fn holds(&mut self, line: impl Into<String>) {
        self.holds.push(line.into());
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
                "nothing missing: install the cluster operator as docs/cluster-operator.md says"
            )
        } else {
            write!(
                f,
                "{} missing: install the cluster operator once doctor finds nothing missing",
                self.findings.len()
            )
        }
    }
}

/// The one command that applies the pinned upstream release.
/// Step 1 of the install: the pinned release, its controller named by digest.
fn apply_upstream() -> String {
    format!("kubectl apply --server-side -k operator/vendor/agent-sandbox/{VERSION}")
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
    report.holds(format!("asked {}", cluster.asked()));
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
            report.holds(format!("Kubernetes {shown}"));
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
    let controller = match cluster.controller().await {
        Ok(controller) => controller,
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
    let has_controller = controller.is_some();
    let Some(controller) = controller.filter(|_| absent.is_empty()) else {
        let apply = format!("apply it, step 1 of the install: {}", apply_upstream());
        if present == 0 && !has_controller {
            report.lacks(format!("{wanted}: not installed"), apply);
        } else {
            if !has_controller {
                absent.push(format!("its controller, deployment {namespace}/{name}"));
            }
            report.lacks(
                format!("{wanted}: installed in part, without {}", absent.join(", ")),
                apply,
            );
        }
        return;
    };
    let image = controller
        .images
        .iter()
        .find(|image| image.contains(name))
        .or(controller.images.first());
    let Some(image) = image else {
        report.lacks(
            format!("{wanted}: doctor cannot tell which release runs, since deployment {namespace}/{name} names no image"),
            format!("apply the pinned release, which names its controller by digest: {}", apply_upstream()),
        );
        return;
    };
    let pinned = digest_of(image) == Some(CONTROLLER_DIGEST);
    if !pinned {
        let next = format!(
            "apply the pinned release, which names its controller by digest: {}",
            apply_upstream()
        );
        match (tag_of(image), digest_of(image)) {
            (Some(VERSION), None) => report.lacks(
                format!("{wanted}: its controller runs {VERSION} by its tag, not by the pinned release's digest"),
                next,
            ),
            (Some(VERSION), Some(digest)) => report.lacks(
                format!("{wanted}: its controller's image names {VERSION} at {digest}, which is not the pinned release's {CONTROLLER_DIGEST}"),
                next,
            ),
            (Some(release), _) => report.lacks(
                format!("{wanted}: the cluster runs {release}, and purlis is tested against {VERSION} only"),
                format!(
                    "check with whoever installed {release} that nothing else needs it, then apply the pinned release over it: {}",
                    apply_upstream()
                ),
            ),
            (None, _) => report.lacks(
                format!("{wanted}: doctor cannot tell which release runs, since the controller's image \"{image}\" names no tag and not the pinned digest"),
                next,
            ),
        }
        return;
    }
    if controller.ready < 1 {
        report.lacks(
            format!("{wanted}: its controller, deployment {namespace}/{name}, has no ready replica"),
            format!("see why with kubectl -n {namespace} describe deployment {name}, and run doctor again once it is ready"),
        );
    } else {
        report.holds(wanted);
    }
}

/// The digest of an image reference, `registry/repo@sha256:…` or `registry/repo:tag@sha256:…`.
fn digest_of(image: &str) -> Option<&str> {
    image.split_once('@').map(|(_, digest)| digest)
}

/// The tag of an image reference, `registry/repo:tag` or `registry/repo:tag@sha256:…`.
fn tag_of(image: &str) -> Option<&str> {
    let named = image.split('@').next().unwrap_or(image);
    let (_, tag) = named.rsplit_once(':')?;
    (!tag.contains('/')).then_some(tag)
}

async fn rights(report: &mut Report, cluster: &impl Cluster) {
    let applies = CLUSTER_SCOPED
        .iter()
        .flat_map(|&(group, resource)| APPLY.map(|verb| (verb, group, resource)));
    let mut denied = Vec::new();
    let mut roles_denied = Vec::new();
    let mut unasked = Vec::new();
    for (verb, group, resource) in applies.chain(ROLE_RIGHTS.iter().copied()) {
        let what = if group.is_empty() {
            format!("{verb} {resource}")
        } else {
            format!("{verb} {resource}.{group}")
        };
        match cluster.may(verb, group, resource).await {
            Ok(true) => {}
            Ok(false) if ROLE_RIGHTS.contains(&(verb, group, resource)) => roles_denied.push(what),
            Ok(false) => denied.push(what),
            Err(Unanswered::Forbidden) => unasked.push(what),
            Err(Unanswered::Failed(error)) => {
                report.lacks(
                    format!("an answer to whether you may {what}"),
                    ask_again(&error),
                );
                return;
            }
        }
    }
    if !unasked.is_empty() {
        report.lacks(
            format!(
                "permission to ask the cluster what you may do: you may not ask whether you may {}",
                unasked.join(", ")
            ),
            "run doctor as a cluster admin, or ask one for create on selfsubjectaccessreviews.authorization.k8s.io (the system:basic-user role)",
        );
    }
    if denied.is_empty() && roles_denied.is_empty() {
        if unasked.is_empty() {
            report.holds("you may apply the install's cluster-scoped definitions");
        }
        return;
    }
    let mut lacks =
        "permission to apply the install's cluster-scoped definitions: you may not".to_owned();
    if !denied.is_empty() {
        lacks.push(' ');
        lacks.push_str(&denied.join(", "));
    }
    if !roles_denied.is_empty() {
        if !denied.is_empty() {
            lacks.push(',');
        }
        lacks.push(' ');
        lacks.push_str(&roles_denied.join(", "));
        lacks.push_str(", unless you already hold every right the install's cluster roles grant");
    }
    let first = denied
        .first()
        .or(roles_denied.first())
        .cloned()
        .unwrap_or_default();
    report.lacks(
        lacks,
        format!(
            "run doctor as a cluster admin, or ask one for these rights; check each with kubectl auth can-i {first}"
        ),
    );
}

/// The real cluster, through a kube client with the credentials of whoever runs doctor.
pub struct Kube {
    client: kube::Client,
    asked: String,
}

impl Kube {
    /// `client`, which asks the cluster the person knows as `asked`.
    pub fn new(client: kube::Client, asked: impl Into<String>) -> Self {
        Kube {
            client,
            asked: asked.into(),
        }
    }

    /// The cluster of the person's kubeconfig (or the pod's own, in a cluster), as kubectl
    /// picks it.
    pub async fn from_environment() -> Result<Self, String> {
        let config = kube::Config::infer()
            .await
            .map_err(|error| error.to_string())?;
        let server = config.cluster_url.to_string();
        let asked = match Kubeconfig::read().ok().and_then(|k| k.current_context) {
            Some(context) => format!("context {context}, API server {server}"),
            None => format!("API server {server}"),
        };
        let client = kube::Client::try_from(config).map_err(|error| error.to_string())?;
        Ok(Kube::new(client, asked))
    }
}

fn unanswered(error: kube::Error) -> Unanswered {
    match &error {
        kube::Error::Api(status) if status.code == 403 => Unanswered::Forbidden,
        _ => Unanswered::Failed(error.to_string()),
    }
}

impl Cluster for Kube {
    fn asked(&self) -> String {
        self.asked.clone()
    }

    async fn version(&self) -> Result<Version, Unanswered> {
        let info = self.client.apiserver_version().await.map_err(unanswered)?;
        Ok(Version {
            major: info.major,
            minor: info.minor,
            git_version: info.git_version,
        })
    }

    async fn definition(&self, name: &str) -> Result<Option<Vec<String>>, Unanswered> {
        let definitions = Api::<CustomResourceDefinition>::all(self.client.clone());
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

    async fn controller(&self) -> Result<Option<Controller>, Unanswered> {
        let (namespace, name) = UPSTREAM_CONTROLLER;
        let deployments = Api::<Deployment>::namespaced(self.client.clone(), namespace);
        let found = deployments.get_opt(name).await.map_err(unanswered)?;
        Ok(found.map(|deployment| Controller {
            ready: deployment
                .status
                .and_then(|status| status.ready_replicas)
                .unwrap_or(0),
            images: deployment
                .spec
                .and_then(|spec| spec.template.spec)
                .map(|pod| pod.containers.into_iter().filter_map(|c| c.image).collect())
                .unwrap_or_default(),
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
        let reviews = Api::<SelfSubjectAccessReview>::all(self.client.clone());
        let answered = reviews
            .create(&PostParams::default(), &review)
            .await
            .map_err(unanswered)?;
        Ok(answered.status.is_some_and(|status| status.allowed))
    }
}
