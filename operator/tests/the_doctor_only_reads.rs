// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The doctor's real cluster, [`doctor::Kube`], against an API server of the test's own making
//! (#2236): it asks only reads and `SelfSubjectAccessReview`, which the API server keeps nothing
//! of, and it reads the answers a Kubernetes 1.36 API server gives. `e2e/kind.sh` runs the same
//! doctor against a real cluster.

use http::{Method, Request, Response, StatusCode};
use kube::client::Body;
use purlis_cluster_operator::doctor;
use serde_json::{Value, json};
use std::convert::Infallible;
use std::sync::{Arc, Mutex};

/// A cluster with upstream installed at `tag`, where the person may do anything but what
/// `denied` names (`verb resource`).
#[derive(Clone)]
struct Server {
    upstream: Option<&'static str>,
    denied: Vec<&'static str>,
    forbidden_reads: bool,
    /// A definition that has `v1beta1` but does not serve it.
    unserved: Option<&'static str>,
    asked: Arc<Mutex<Vec<(Method, String)>>>,
}

/// The controller's image at `tag`, as the install names it: the pinned release by its digest.
fn installed(tag: &str) -> String {
    use purlis_cluster_operator::upstream::{CONTROLLER_DIGEST, CONTROLLER_IMAGE, VERSION};
    if tag == VERSION {
        format!("{CONTROLLER_IMAGE}@{CONTROLLER_DIGEST}")
    } else {
        format!("{CONTROLLER_IMAGE}:{tag}")
    }
}

fn status(code: u16, reason: &str) -> (u16, Value) {
    (
        code,
        json!({"kind": "Status", "apiVersion": "v1", "status": "Failure",
               "reason": reason, "message": reason, "code": code}),
    )
}

impl Server {
    fn new(upstream: Option<&'static str>) -> Self {
        Server {
            upstream,
            denied: Vec::new(),
            forbidden_reads: false,
            unserved: None,
            asked: Arc::default(),
        }
    }

    fn answer(&self, method: &Method, path: &str, body: &[u8]) -> (u16, Value) {
        const CRDS: &str = "/apis/apiextensions.k8s.io/v1/customresourcedefinitions/";
        const CONTROLLER: &str =
            "/apis/apps/v1/namespaces/agent-sandbox-system/deployments/agent-sandbox-controller";
        const REVIEWS: &str = "/apis/authorization.k8s.io/v1/selfsubjectaccessreviews";
        match (method, path) {
            (&Method::GET, "/version") => (
                200,
                json!({"major": "1", "minor": "36", "gitVersion": "v1.36.4",
                       "gitCommit": "0", "gitTreeState": "clean", "buildDate": "2026-09-01T00:00:00Z",
                       "goVersion": "go1.26", "compiler": "gc", "platform": "linux/amd64"}),
            ),
            (&Method::GET, _) if self.forbidden_reads => status(403, "Forbidden"),
            (&Method::GET, p) if p.starts_with(CRDS) => match self.upstream {
                None => status(404, "NotFound"),
                Some(_) => (
                    200,
                    json!({"apiVersion": "apiextensions.k8s.io/v1", "kind": "CustomResourceDefinition",
                           "metadata": {"name": &p[CRDS.len()..]},
                           "spec": {"group": "agents.x-k8s.io", "scope": "Namespaced",
                                    "names": {"plural": "x", "kind": "X"},
                                    "versions": [{"name": "v1beta1", "storage": true,
                                              "served": self.unserved != Some(&p[CRDS.len()..])}]}}),
                ),
            },
            (&Method::GET, CONTROLLER) => match self.upstream {
                None => status(404, "NotFound"),
                Some(tag) => (
                    200,
                    json!({"apiVersion": "apps/v1", "kind": "Deployment",
                           "metadata": {"name": "agent-sandbox-controller", "namespace": "agent-sandbox-system"},
                           "spec": {"selector": {}, "template": {"spec": {"containers": [
                               {"name": "agent-sandbox-controller",
                                "image": installed(tag)}]}}},
                           "status": {"readyReplicas": 1}}),
                ),
            },
            (&Method::POST, REVIEWS) => {
                let mut review: Value = serde_json::from_slice(body).expect("a review");
                let attributes = &review["spec"]["resourceAttributes"];
                let asked = format!(
                    "{} {}",
                    attributes["verb"].as_str().unwrap_or_default(),
                    attributes["resource"].as_str().unwrap_or_default()
                );
                review["status"] = json!({"allowed": !self.denied.contains(&asked.as_str())});
                (201, review)
            }
            _ => status(405, "MethodNotAllowed"),
        }
    }

    fn client(&self) -> kube::Client {
        let server = self.clone();
        let service = tower::service_fn(move |request: Request<Body>| {
            let server = server.clone();
            async move {
                let (parts, body) = request.into_parts();
                let body = http_body_util::BodyExt::collect(body)
                    .await
                    .expect("a body")
                    .to_bytes();
                let path = parts.uri.path().to_owned();
                server
                    .asked
                    .lock()
                    .expect("not poisoned")
                    .push((parts.method.clone(), path.clone()));
                let (code, answer) = server.answer(&parts.method, &path, &body);
                Ok::<_, Infallible>(
                    Response::builder()
                        .status(StatusCode::from_u16(code).expect("a status"))
                        .header("content-type", "application/json")
                        .body(Body::from(serde_json::to_vec(&answer).expect("JSON")))
                        .expect("a response"),
                )
            }
        });
        kube::Client::new(service, "default")
    }

    fn doctor(&self) -> doctor::Report {
        // The client's buffer starts on the runtime it is made in.
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("a runtime")
            .block_on(async {
                doctor::check(&doctor::Kube::new(self.client(), "the test's API server")).await
            })
    }
}

#[test]
fn it_asks_only_reads_and_access_reviews() {
    let server = Server::new(None);
    let report = server.doctor();
    assert!(report.lacks_anything(), "{report}");
    let asked = server.asked.lock().expect("not poisoned").clone();
    assert!(!asked.is_empty());
    for (method, path) in &asked {
        let read = method == Method::GET;
        let review = method == Method::POST
            && path == "/apis/authorization.k8s.io/v1/selfsubjectaccessreviews";
        assert!(read || review, "doctor asked {method} {path}");
    }
}

#[test]
fn a_kubernetes_1_36_cluster_with_the_pinned_upstream_lacks_nothing() {
    let report = Server::new(Some("v1.0.6")).doctor();
    assert!(report.findings.is_empty(), "{report}");
}

#[test]
fn a_cluster_without_upstream_names_it() {
    let report = Server::new(None).doctor();
    assert_eq!(report.findings.len(), 1, "{report}");
    assert!(
        report.findings[0]
            .to_string()
            .contains("agent-sandbox v1.0.6: not installed"),
        "{report}"
    );
}

#[test]
fn another_upstream_release_is_read_from_the_controllers_image() {
    let report = Server::new(Some("v1.1.0")).doctor();
    assert_eq!(report.findings.len(), 1, "{report}");
    assert!(
        report.findings[0].to_string().contains("v1.1.0"),
        "{report}"
    );
}

#[test]
fn a_refused_read_and_a_denied_review_are_findings() {
    let mut server = Server::new(Some("v1.0.6"));
    server.forbidden_reads = true;
    server.denied = vec!["patch clusterrolebindings"];
    let report = server.doctor();
    let lines = report.to_string();
    assert_eq!(report.findings.len(), 2, "{lines}");
    assert!(
        lines.contains("permission to read customresourcedefinitions"),
        "{lines}"
    );
    assert!(
        lines.contains("patch clusterrolebindings.rbac.authorization.k8s.io"),
        "{lines}"
    );
}

#[test]
fn a_definition_that_has_the_version_but_does_not_serve_it_lacks_it() {
    let mut server = Server::new(Some("v1.0.6"));
    server.unserved = Some("sandboxclaims.extensions.agents.x-k8s.io");
    let report = server.doctor();
    assert_eq!(report.findings.len(), 1, "{report}");
    assert!(
        report.findings[0]
            .to_string()
            .contains("sandboxclaims.extensions.agents.x-k8s.io serving v1beta1"),
        "{report}"
    );
}
