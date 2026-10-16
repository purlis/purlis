// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The operator in a real cluster: a kind cluster with the pinned upstream agent-sandbox,
//! purlis's definitions and the operator installed by `operator/e2e/kind.sh`, which then runs
//! these tests (ignored anywhere else) with the cluster's admin credentials. Each test checks only
//! what the cluster shows, and ends what it started.
//!
//! `PURLIS_RUNNER_IMAGE` names the runner image by digest, as a Pool does.

use k8s_openapi::api::authorization::v1::{
    ResourceAttributes, SubjectAccessReview, SubjectAccessReviewSpec,
};
use k8s_openapi::api::core::v1::{Namespace, Pod};
use kube::api::{
    Api, DeleteParams, ListParams, LogParams, ObjectMeta, Patch, PatchParams, PostParams,
};
use kube::{Client, ResourceExt};
use purlis_cluster_operator::{Claim, ClaimSpec, Pool, PoolSpec};
use std::time::{Duration, Instant};

const SERVICE_ACCOUNT: &str = "system:serviceaccount:purlis-system:purlis-cluster-operator";

fn runner_image() -> String {
    std::env::var("PURLIS_RUNNER_IMAGE").expect("PURLIS_RUNNER_IMAGE names the runner image")
}

async fn eventually<T, F, Fut>(what: &str, within: Duration, mut check: F) -> T
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Option<T>>,
{
    let start = Instant::now();
    loop {
        if let Some(found) = check().await {
            return found;
        }
        assert!(start.elapsed() < within, "{what}: not within {within:?}");
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

/// A namespace of the test's own, removed with everything in it when the test ends.
struct Scratch {
    client: Client,
    name: String,
}

impl Scratch {
    async fn new(client: &Client, name: &str) -> Self {
        let namespaces = Api::<Namespace>::all(client.clone());
        let namespace = Namespace {
            metadata: ObjectMeta {
                name: Some(name.to_owned()),
                ..ObjectMeta::default()
            },
            ..Namespace::default()
        };
        namespaces
            .create(&PostParams::default(), &namespace)
            .await
            .expect("a scratch namespace");
        Scratch {
            client: client.clone(),
            name: name.to_owned(),
        }
    }

    async fn end(self) {
        let _ = Api::<Namespace>::all(self.client)
            .delete(&self.name, &DeleteParams::default())
            .await;
    }
}

fn claim(name: &str, pool: &str) -> Claim {
    Claim::new(
        name,
        ClaimSpec {
            pool: pool.to_owned(),
        },
    )
}

fn ready_of(claim: &Claim) -> Option<(String, String)> {
    claim
        .status
        .as_ref()?
        .conditions
        .iter()
        .find(|c| c.type_ == "Ready")
        .map(|c| (c.status.clone(), c.reason.clone()))
}

#[tokio::test]
#[ignore = "needs a kind cluster: operator/e2e/kind.sh"]
async fn a_pool_and_a_claim_yield_a_running_session_host_that_goes_with_the_claim() {
    let client = Client::try_default().await.expect("the cluster");
    let scratch = Scratch::new(&client, "e2e-runs").await;
    let pools = Api::<Pool>::all(client.clone());
    let pool = Pool::new(
        "e2e-runs",
        PoolSpec {
            image: runner_image(),
        },
    );
    pools
        .patch(
            "e2e-runs",
            &PatchParams::apply("e2e").force(),
            &Patch::Apply(&pool),
        )
        .await
        .expect("the pool");
    let claims = Api::<Claim>::namespaced(client.clone(), &scratch.name);
    claims
        .create(&PostParams::default(), &claim("ada-1", "e2e-runs"))
        .await
        .expect("the claim");

    eventually("the claim is ready", Duration::from_secs(300), || async {
        let claim = claims.get("ada-1").await.ok()?;
        (ready_of(&claim)?.0 == "True").then_some(())
    })
    .await;

    let pods = Api::<Pod>::namespaced(client.clone(), &scratch.name);
    let pod = eventually("one runner pod", Duration::from_secs(30), || async {
        let list = pods.list(&ListParams::default()).await.ok()?;
        (list.items.len() == 1).then(|| list.items[0].name_any())
    })
    .await;
    eventually(
        "the session host says it is ready",
        Duration::from_secs(60),
        || async {
            let log = pods.logs(&pod, &LogParams::default()).await.ok()?;
            log.lines().any(|line| line.trim() == "ready").then_some(())
        },
    )
    .await;

    claims
        .delete("ada-1", &DeleteParams::foreground())
        .await
        .expect("the claim deleted");
    eventually(
        "the runner goes with its claim",
        Duration::from_secs(120),
        || async {
            let list = pods.list(&ListParams::default()).await.ok()?;
            list.items.is_empty().then_some(())
        },
    )
    .await;

    let _ = pools.delete("e2e-runs", &DeleteParams::default()).await;
    scratch.end().await;
}

#[tokio::test]
#[ignore = "needs a kind cluster: operator/e2e/kind.sh"]
async fn a_claim_on_a_pool_that_does_not_exist_says_so() {
    let client = Client::try_default().await.expect("the cluster");
    let scratch = Scratch::new(&client, "e2e-missing").await;
    let claims = Api::<Claim>::namespaced(client.clone(), &scratch.name);
    claims
        .create(&PostParams::default(), &claim("ada-1", "no-such-pool"))
        .await
        .expect("the claim");
    eventually(
        "the claim names the missing pool",
        Duration::from_secs(60),
        || async {
            let claim = claims.get("ada-1").await.ok()?;
            (ready_of(&claim)? == ("False".into(), "PoolNotFound".into())).then_some(())
        },
    )
    .await;
    scratch.end().await;
}

#[tokio::test]
#[ignore = "needs a kind cluster: operator/e2e/kind.sh"]
async fn a_pool_names_its_image_by_digest_and_a_claim_keeps_its_pool() {
    let client = Client::try_default().await.expect("the cluster");
    let pools = Api::<Pool>::all(client.clone());
    let by_tag = Pool::new(
        "e2e-tag",
        PoolSpec {
            image: "ghcr.io/purlis/purlis-runner:main".into(),
        },
    );
    let refused = pools.create(&PostParams::default(), &by_tag).await;
    assert!(
        matches!(&refused, Err(kube::Error::Api(status)) if status.code == 422),
        "{refused:?}"
    );

    let scratch = Scratch::new(&client, "e2e-keeps").await;
    let claims = Api::<Claim>::namespaced(client.clone(), &scratch.name);
    claims
        .create(&PostParams::default(), &claim("ada-1", "first"))
        .await
        .expect("the claim");
    let moved = claims
        .patch(
            "ada-1",
            &PatchParams::default(),
            &Patch::Merge(serde_json::json!({ "spec": { "pool": "second" } })),
        )
        .await;
    assert!(
        matches!(&moved, Err(kube::Error::Api(status)) if status.code == 422),
        "{moved:?}"
    );
    scratch.end().await;
}

async fn may(
    client: &Client,
    group: &str,
    verb: &str,
    resource: &str,
    subresource: Option<&str>,
) -> bool {
    let review = SubjectAccessReview {
        spec: SubjectAccessReviewSpec {
            user: Some(SERVICE_ACCOUNT.to_owned()),
            resource_attributes: Some(ResourceAttributes {
                group: Some(group.to_owned()),
                verb: Some(verb.to_owned()),
                resource: Some(resource.to_owned()),
                subresource: subresource.map(str::to_owned),
                namespace: Some("default".to_owned()),
                ..ResourceAttributes::default()
            }),
            ..SubjectAccessReviewSpec::default()
        },
        ..SubjectAccessReview::default()
    };
    let answer = Api::<SubjectAccessReview>::all(client.clone())
        .create(&PostParams::default(), &review)
        .await
        .expect("the cluster answers an access review");
    answer.status.expect("a status").allowed
}

#[tokio::test]
#[ignore = "needs a kind cluster: operator/e2e/kind.sh"]
async fn the_cluster_refuses_the_operator_exec_attach_and_secrets() {
    let client = Client::try_default().await.expect("the cluster");
    for (verb, resource, subresource) in [
        ("create", "pods", Some("exec")),
        ("get", "pods", Some("exec")),
        ("create", "pods", Some("attach")),
        ("create", "pods", Some("portforward")),
        ("get", "secrets", None),
        ("list", "secrets", None),
        ("watch", "secrets", None),
        ("create", "pods", None),
    ] {
        assert!(
            !may(&client, "", verb, resource, subresource).await,
            "the operator may {verb} {resource} {subresource:?}"
        );
    }
    // The check is the cluster's answer, not an empty one: what the operator needs, it may.
    assert!(
        may(
            &client,
            "runners.purlis.dev",
            "patch",
            "claims",
            Some("status")
        )
        .await
    );
}

/// A client acting as `user`, through the admin's right to impersonate.
async fn acting_as(user: &str) -> Client {
    let mut config = kube::Config::infer().await.expect("the cluster");
    config.auth_info.impersonate = Some(user.to_owned());
    Client::try_from(config).expect("a client")
}

fn rbac(value: serde_json::Value) -> kube::api::DynamicObject {
    serde_json::from_value(value).expect("an RBAC object")
}

async fn apply_rbac(client: &Client, kind: &str, namespace: Option<&str>, body: serde_json::Value) {
    let resource = kube::api::ApiResource {
        group: "rbac.authorization.k8s.io".into(),
        version: "v1".into(),
        api_version: "rbac.authorization.k8s.io/v1".into(),
        kind: kind.into(),
        plural: format!("{}s", kind.to_lowercase()),
    };
    let api: Api<kube::api::DynamicObject> = match namespace {
        Some(namespace) => Api::namespaced_with(client.clone(), namespace, &resource),
        None => Api::all_with(client.clone(), &resource),
    };
    let object = rbac(body);
    api.patch(
        &object.name_any(),
        &PatchParams::apply("e2e").force(),
        &Patch::Apply(&object),
    )
    .await
    .unwrap_or_else(|error| panic!("{kind}: {error}"));
}

#[tokio::test]
#[ignore = "needs a kind cluster: operator/e2e/kind.sh"]
async fn a_claim_is_refused_to_someone_the_cluster_does_not_let_use_its_pool() {
    let client = Client::try_default().await.expect("the cluster");
    let scratch = Scratch::new(&client, "e2e-use").await;
    let pools = Api::<Pool>::all(client.clone());
    let pool = Pool::new(
        "e2e-use",
        PoolSpec {
            image: runner_image(),
        },
    );
    pools
        .patch(
            "e2e-use",
            &PatchParams::apply("e2e").force(),
            &Patch::Apply(&pool),
        )
        .await
        .expect("the pool");
    // Both may make claims in the namespace; only bea may use the pool.
    apply_rbac(
        &client,
        "Role",
        Some(&scratch.name),
        serde_json::json!({
            "apiVersion": "rbac.authorization.k8s.io/v1", "kind": "Role",
            "metadata": { "name": "e2e-claims", "namespace": scratch.name },
            "rules": [{ "apiGroups": ["runners.purlis.dev"], "resources": ["claims"],
                        "verbs": ["create", "get"] }]
        }),
    )
    .await;
    apply_rbac(
        &client,
        "RoleBinding",
        Some(&scratch.name),
        serde_json::json!({
            "apiVersion": "rbac.authorization.k8s.io/v1", "kind": "RoleBinding",
            "metadata": { "name": "e2e-claims", "namespace": scratch.name },
            "roleRef": { "apiGroup": "rbac.authorization.k8s.io", "kind": "Role", "name": "e2e-claims" },
            "subjects": [
                { "apiGroup": "rbac.authorization.k8s.io", "kind": "User", "name": "e2e-ada" },
                { "apiGroup": "rbac.authorization.k8s.io", "kind": "User", "name": "e2e-bea" }
            ]
        }),
    )
    .await;
    apply_rbac(
        &client,
        "ClusterRole",
        None,
        serde_json::json!({
            "apiVersion": "rbac.authorization.k8s.io/v1", "kind": "ClusterRole",
            "metadata": { "name": "e2e-use-pool" },
            "rules": [{ "apiGroups": ["runners.purlis.dev"], "resources": ["pools"],
                        "resourceNames": ["e2e-use"], "verbs": ["use"] }]
        }),
    )
    .await;
    apply_rbac(
        &client,
        "ClusterRoleBinding",
        None,
        serde_json::json!({
            "apiVersion": "rbac.authorization.k8s.io/v1", "kind": "ClusterRoleBinding",
            "metadata": { "name": "e2e-use-pool" },
            "roleRef": { "apiGroup": "rbac.authorization.k8s.io", "kind": "ClusterRole", "name": "e2e-use-pool" },
            "subjects": [{ "apiGroup": "rbac.authorization.k8s.io", "kind": "User", "name": "e2e-bea" }]
        }),
    )
    .await;

    let as_ada = Api::<Claim>::namespaced(acting_as("e2e-ada").await, &scratch.name);
    let refused = as_ada
        .create(&PostParams::default(), &claim("ada-1", "e2e-use"))
        .await;
    match &refused {
        Err(kube::Error::Api(status)) => {
            assert_eq!(status.code, 403, "{status:?}");
            // The policy's own words, not an ordinary refusal of the request.
            assert!(
                status.message.contains("may not use the pool e2e-use"),
                "{status:?}"
            );
        }
        other => panic!("ada's claim was not refused: {other:?}"),
    }

    let as_bea = Api::<Claim>::namespaced(acting_as("e2e-bea").await, &scratch.name);
    as_bea
        .create(&PostParams::default(), &claim("bea-1", "e2e-use"))
        .await
        .expect("bea may use the pool");

    let delete = DeleteParams::default();
    let roles = kube::api::ApiResource {
        group: "rbac.authorization.k8s.io".into(),
        version: "v1".into(),
        api_version: "rbac.authorization.k8s.io/v1".into(),
        kind: "ClusterRole".into(),
        plural: "clusterroles".into(),
    };
    let bindings = kube::api::ApiResource {
        kind: "ClusterRoleBinding".into(),
        plural: "clusterrolebindings".into(),
        ..roles.clone()
    };
    let _ = Api::<kube::api::DynamicObject>::all_with(client.clone(), &bindings)
        .delete("e2e-use-pool", &delete)
        .await;
    let _ = Api::<kube::api::DynamicObject>::all_with(client.clone(), &roles)
        .delete("e2e-use-pool", &delete)
        .await;
    let _ = pools.delete("e2e-use", &delete).await;
    scratch.end().await;
}

/// Doctor (#2236) run by someone who may only read what it reads: it still answers every check,
/// so it needs no right to change anything, and it says they may not apply the install.
#[tokio::test]
#[ignore = "needs a kind cluster: operator/e2e/kind.sh"]
async fn doctor_needs_only_reads_and_says_what_a_reader_may_not_apply() {
    use purlis_cluster_operator::doctor;
    let client = Client::try_default().await.expect("the cluster");
    apply_rbac(
        &client,
        "ClusterRole",
        None,
        serde_json::json!({
            "apiVersion": "rbac.authorization.k8s.io/v1", "kind": "ClusterRole",
            "metadata": { "name": "e2e-doctor-reads" },
            "rules": [
                { "apiGroups": ["apiextensions.k8s.io"], "resources": ["customresourcedefinitions"], "verbs": ["get"] },
                { "apiGroups": ["apps"], "resources": ["deployments"], "verbs": ["get"] }
            ]
        }),
    )
    .await;
    apply_rbac(
        &client,
        "ClusterRoleBinding",
        None,
        serde_json::json!({
            "apiVersion": "rbac.authorization.k8s.io/v1", "kind": "ClusterRoleBinding",
            "metadata": { "name": "e2e-doctor-reads" },
            "roleRef": { "apiGroup": "rbac.authorization.k8s.io", "kind": "ClusterRole", "name": "e2e-doctor-reads" },
            "subjects": [{ "apiGroup": "rbac.authorization.k8s.io", "kind": "User", "name": "e2e-doctor" }]
        }),
    )
    .await;

    let as_reader = doctor::check(&doctor::Kube::new(
        acting_as("e2e-doctor").await,
        "kind, as e2e-doctor",
    ))
    .await;
    let as_admin = doctor::check(&doctor::Kube::new(client.clone(), "kind")).await;

    let delete = DeleteParams::default();
    for (kind, plural) in [
        ("ClusterRoleBinding", "clusterrolebindings"),
        ("ClusterRole", "clusterroles"),
    ] {
        let resource = kube::api::ApiResource {
            group: "rbac.authorization.k8s.io".into(),
            version: "v1".into(),
            api_version: "rbac.authorization.k8s.io/v1".into(),
            kind: kind.into(),
            plural: plural.into(),
        };
        let _ = Api::<kube::api::DynamicObject>::all_with(client.clone(), &resource)
            .delete("e2e-doctor-reads", &delete)
            .await;
    }

    assert!(!as_admin.lacks_anything(), "{as_admin}");
    assert_eq!(as_reader.findings.len(), 1, "{as_reader}");
    let line = as_reader.findings[0].to_string();
    assert!(
        line.contains("permission to apply the install's cluster-scoped definitions"),
        "{line}"
    );
    assert!(
        as_reader
            .holds
            .iter()
            .any(|held| held == "upstream agent-sandbox v1.0.6"),
        "{as_reader}"
    );
}
