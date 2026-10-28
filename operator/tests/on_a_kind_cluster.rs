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

/// Who the tests' own kubeconfig signs in as: kind's cluster admin.
const ADMIN: &str = "kubernetes-admin";

/// A claim on `pool`, made by the cluster admin the tests sign in as.
fn claim(name: &str, pool: &str) -> Claim {
    claim_by(name, pool, ADMIN)
}

/// A claim on `pool`, naming `requester` as who makes it.
fn claim_by(name: &str, pool: &str, requester: &str) -> Claim {
    Claim::new(
        name,
        ClaimSpec {
            pool: pool.to_owned(),
            requester: requester.to_owned(),
            kept: false,
            stopped: false,
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
            kept_volume: Default::default(),
            stopped: false,
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

/// The bytes of 64 or 128 lower-case hex digits.
fn unhex(hex: &str) -> Vec<u8> {
    (0..hex.len() / 2)
        .map(|at| u8::from_str_radix(&hex[at * 2..at * 2 + 2], 16).expect("hex"))
        .collect()
}

#[tokio::test]
#[ignore = "needs a kind cluster: operator/e2e/kind.sh"]
async fn a_runners_link_key_is_vouched_for_with_the_key_its_pool_names() {
    let client = Client::try_default().await.expect("the cluster");
    let scratch = Scratch::new(&client, "e2e-vouch").await;
    let pools = Api::<Pool>::all(client.clone());
    let pool = Pool::new(
        "e2e-vouch",
        PoolSpec {
            image: runner_image(),
            kept_volume: Default::default(),
            stopped: false,
        },
    );
    pools
        .patch(
            "e2e-vouch",
            &PatchParams::apply("e2e").force(),
            &Patch::Apply(&pool),
        )
        .await
        .expect("the pool");
    let signing_key = eventually(
        "the pool names the operator's key",
        Duration::from_secs(60),
        || async { pools.get("e2e-vouch").await.ok()?.status?.signing_key },
    )
    .await;
    let public = unhex(
        signing_key
            .strip_prefix("ed25519:")
            .expect("an Ed25519 key"),
    );
    let claims = claims_in(&client, &scratch.name);
    let made = claims
        .create(&PostParams::default(), &claim("ada-1", "e2e-vouch"))
        .await
        .expect("the claim");

    // What the claim says of its key, named in the failure if it is never vouched for.
    let said = std::sync::Mutex::new(String::new());
    let link_key = eventually(
        "the operator vouches for the key",
        Duration::from_secs(300),
        || async {
            let status = claims.get("ada-1").await.ok()?.status?;
            if let Some(word) = status.conditions.iter().find(|c| c.type_ == "KeyPublished") {
                let word = format!("{}: {}", word.reason, word.message);
                let mut kept = said.lock().unwrap();
                if *kept != word {
                    eprintln!("KeyPublished {word}");
                    *kept = word;
                }
            }
            status.link_key
        },
    )
    .await;
    ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, &public)
        .verify(link_key.statement.as_bytes(), &unhex(&link_key.signature))
        .expect("signed with the key the pool names");

    let pods = Api::<Pod>::namespaced(client.clone(), &scratch.name);
    let list = pods.list(&ListParams::default()).await.expect("pods");
    assert_eq!(list.items.len(), 1);
    let pod = &list.items[0];
    let published = pod
        .status
        .as_ref()
        .and_then(|status| status.init_container_statuses.as_ref())
        .and_then(|statuses| statuses.iter().find(|s| s.name == "link-key"))
        .and_then(|status| status.state.as_ref()?.terminated.as_ref()?.message.clone())
        .expect("the pod's first step published its key");
    let expected =
        purlis_cluster_operator::vouch::statement(&purlis_cluster_operator::vouch::About {
            namespace: &scratch.name,
            claim: "ada-1",
            claim_uid: &made.uid().expect("its uid"),
            pod: &pod.name_any(),
            key: &published,
            requester: ADMIN,
            until: purlis_cluster_operator::vouch::until_of(&link_key.statement)
                .expect("the statement says until when it holds"),
        })
        .expect("a statement");
    assert_eq!(link_key.statement, expected);
    let claim = claims.get("ada-1").await.expect("the claim");
    let vouched = claim
        .status
        .expect("a status")
        .conditions
        .into_iter()
        .find(|c| c.type_ == "KeyPublished")
        .expect("KeyPublished");
    assert_eq!(
        (vouched.status.as_str(), vouched.reason.as_str()),
        ("True", "Vouched")
    );

    let _ = claims.delete("ada-1", &DeleteParams::foreground()).await;
    let _ = pools.delete("e2e-vouch", &DeleteParams::default()).await;
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
            kept_volume: Default::default(),
            stopped: false,
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

#[tokio::test]
#[ignore = "needs a kind cluster: operator/e2e/kind.sh"]
async fn an_upstream_object_the_operator_did_not_make_is_left_as_it_is_until_it_is_removed() {
    use purlis_cluster_operator::upstream::{self, SandboxTemplate};
    let client = Client::try_default().await.expect("the cluster");
    let scratch = Scratch::new(&client, "e2e-in-the-way").await;
    let pools = Api::<Pool>::all(client.clone());
    let pool = Pool::new(
        "e2e-in-the-way",
        PoolSpec {
            image: runner_image(),
            kept_volume: Default::default(),
            stopped: false,
        },
    );
    pools
        .patch(
            "e2e-in-the-way",
            &PatchParams::apply("e2e").force(),
            &Patch::Apply(&pool),
        )
        .await
        .expect("the pool");
    // A template under the name the operator would use, made by hand: no owner.
    let templates = Api::<SandboxTemplate>::namespaced(client.clone(), &scratch.name);
    let mut by_hand = upstream::template_for(&pool, &scratch.name, vec![]);
    by_hand.metadata.owner_references = None;
    let containers = &mut by_hand
        .spec
        .pod_template
        .spec
        .as_mut()
        .expect("a pod")
        .containers;
    containers[0].name = "by-hand".into();
    let made = templates
        .create(&PostParams::default(), &by_hand)
        .await
        .expect("the template made by hand");
    let claims = Api::<Claim>::namespaced(client.clone(), &scratch.name);
    claims
        .create(&PostParams::default(), &claim("ada-1", "e2e-in-the-way"))
        .await
        .expect("the claim");

    let message = eventually(
        "the claim names the template in its way",
        Duration::from_secs(60),
        || async {
            let claim = claims.get("ada-1").await.ok()?;
            let ready = claim
                .status?
                .conditions
                .into_iter()
                .find(|c| c.type_ == "Ready")?;
            (ready.reason == "UpstreamObjectInTheWay").then_some(ready.message)
        },
    )
    .await;
    assert!(
        message.contains(&format!("SandboxTemplate {}", made.name_any())),
        "{message}"
    );
    // A while later: still as it was made, and nothing asked of upstream for the claim.
    tokio::time::sleep(Duration::from_secs(20)).await;
    let held = templates.get(&made.name_any()).await.expect("the template");
    // Upstream's own controller labels every template; the operator writes nothing to it.
    assert_eq!(held.spec, made.spec);
    assert!(held.owner_references().is_empty(), "{held:?}");
    assert!(
        held.managed_fields()
            .iter()
            .all(|entry| entry.manager.as_deref() != Some(purlis_cluster_operator::FIELD_MANAGER)),
        "{held:?}"
    );
    assert_eq!(upstream_claims_in(&client, &scratch.name).await, 0);

    templates
        .delete(&made.name_any(), &DeleteParams::default())
        .await
        .expect("the template removed");
    eventually(
        "with it gone, the claim goes on",
        Duration::from_secs(300),
        || async {
            let claim = claims.get("ada-1").await.ok()?;
            (ready_of(&claim)?.0 == "True").then_some(())
        },
    )
    .await;
    let ours = templates
        .get(&made.name_any())
        .await
        .expect("the operator's template");
    assert_eq!(
        ours.owner_references().first().map(|o| o.kind.as_str()),
        Some("Claim")
    );

    let _ = pools
        .delete("e2e-in-the-way", &DeleteParams::default())
        .await;
    scratch.end().await;
}

#[tokio::test]
#[ignore = "needs a kind cluster: operator/e2e/kind.sh"]
async fn a_claims_runner_keeps_running_when_its_pool_is_deleted_and_the_claim_says_so() {
    use k8s_openapi::api::networking::v1::NetworkPolicy;
    use purlis_cluster_operator::upstream::{SandboxTemplate, name_for};
    let client = Client::try_default().await.expect("the cluster");
    let scratch = Scratch::new(&client, "e2e-pool-gone").await;
    let pools = Api::<Pool>::all(client.clone());
    let pool = Pool::new(
        "e2e-pool-gone",
        PoolSpec {
            image: runner_image(),
            kept_volume: Default::default(),
            stopped: false,
        },
    );
    pools
        .patch(
            "e2e-pool-gone",
            &PatchParams::apply("e2e").force(),
            &Patch::Apply(&pool),
        )
        .await
        .expect("the pool");
    let claims = Api::<Claim>::namespaced(client.clone(), &scratch.name);
    claims
        .create(&PostParams::default(), &claim("ada-1", "e2e-pool-gone"))
        .await
        .expect("the claim");
    eventually("the claim is ready", Duration::from_secs(300), || async {
        let claim = claims.get("ada-1").await.ok()?;
        (ready_of(&claim)?.0 == "True").then_some(())
    })
    .await;
    let pods = Api::<Pod>::namespaced(client.clone(), &scratch.name);
    let running = pods.list(&ListParams::default()).await.expect("pods");
    assert_eq!(running.items.len(), 1);
    let pod = running.items[0].clone();

    pools
        .delete("e2e-pool-gone", &DeleteParams::foreground())
        .await
        .expect("the pool deleted");
    eventually("the pool is gone", Duration::from_secs(60), || async {
        pools
            .get_opt("e2e-pool-gone")
            .await
            .ok()?
            .is_none()
            .then_some(())
    })
    .await;
    eventually(
        "the claim says its pool is gone and its runner is ready",
        Duration::from_secs(60),
        || async {
            let claim = claims.get("ada-1").await.ok()?;
            (ready_of(&claim)? == ("True".into(), "PoolDeleted".into())).then_some(())
        },
    )
    .await;
    // What the claim says is what the cluster shows: the same pod, still running, with the
    // template it was made from and the network policy upstream makes of that template.
    tokio::time::sleep(Duration::from_secs(10)).await;
    let still = pods.get(&pod.name_any()).await.expect("the runner's pod");
    assert_eq!(still.uid(), pod.uid());
    assert_eq!(
        still.status.and_then(|status| status.phase).as_deref(),
        Some("Running")
    );
    let template = name_for("e2e-pool-gone");
    Api::<SandboxTemplate>::namespaced(client.clone(), &scratch.name)
        .get(&template)
        .await
        .expect("the runner's template");
    Api::<NetworkPolicy>::namespaced(client.clone(), &scratch.name)
        .get(&format!("{template}-network-policy"))
        .await
        .expect("the runner's network policy");
    let claim = claims.get("ada-1").await.expect("the claim");
    assert_eq!(
        ready_of(&claim),
        Some(("True".into(), "PoolDeleted".into()))
    );

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
    scratch.end().await;
}

#[tokio::test]
#[ignore = "needs a kind cluster: operator/e2e/kind.sh"]
async fn a_pools_template_and_warm_pool_go_with_its_last_claim_in_a_namespace() {
    use purlis_cluster_operator::upstream::{SandboxTemplate, SandboxWarmPool, name_for};
    let client = Client::try_default().await.expect("the cluster");
    let scratch = Scratch::new(&client, "e2e-last-claim").await;
    let pools = Api::<Pool>::all(client.clone());
    let pool = Pool::new(
        "e2e-last-claim",
        PoolSpec {
            image: runner_image(),
            kept_volume: Default::default(),
            stopped: false,
        },
    );
    pools
        .patch(
            "e2e-last-claim",
            &PatchParams::apply("e2e").force(),
            &Patch::Apply(&pool),
        )
        .await
        .expect("the pool");
    let claims = Api::<Claim>::namespaced(client.clone(), &scratch.name);
    let templates = Api::<SandboxTemplate>::namespaced(client.clone(), &scratch.name);
    let warm_pools = Api::<SandboxWarmPool>::namespaced(client.clone(), &scratch.name);
    let name = name_for("e2e-last-claim");
    let ready = |claim: &'static str| {
        let claims = claims.clone();
        async move {
            eventually(
                &format!("{claim} is ready"),
                Duration::from_secs(300),
                || async {
                    let claim = claims.get(claim).await.ok()?;
                    (ready_of(&claim)?.0 == "True").then_some(())
                },
            )
            .await
        }
    };

    claims
        .create(&PostParams::default(), &claim("ada-1", "e2e-last-claim"))
        .await
        .expect("the first claim");
    ready("ada-1").await;
    // The last claim goes, and another is made at the same moment: it is served.
    claims
        .delete("ada-1", &DeleteParams::default())
        .await
        .expect("the first claim deleted");
    claims
        .create(&PostParams::default(), &claim("ada-2", "e2e-last-claim"))
        .await
        .expect("a claim made as the last one goes");
    ready("ada-2").await;
    let template = templates.get(&name).await.expect("the template");
    assert!(
        template
            .owner_references()
            .iter()
            .any(|owner| owner.kind == "Claim" && owner.name == "ada-2"),
        "{template:?}"
    );

    claims
        .delete("ada-2", &DeleteParams::default())
        .await
        .expect("the last claim deleted");
    eventually(
        "the template and warm pool go with the last claim",
        Duration::from_secs(120),
        || async {
            let template = templates.get_opt(&name).await.ok()?;
            let warm = warm_pools.get_opt(&name).await.ok()?;
            (template.is_none() && warm.is_none()).then_some(())
        },
    )
    .await;
    // The pool is still there: a claim made now is served from it.
    claims
        .create(&PostParams::default(), &claim("ada-3", "e2e-last-claim"))
        .await
        .expect("a claim made after");
    ready("ada-3").await;

    let _ = pools
        .delete("e2e-last-claim", &DeleteParams::default())
        .await;
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

fn claims_in(client: &Client, namespace: &str) -> Api<Claim> {
    Api::<Claim>::namespaced(client.clone(), namespace)
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
            kept_volume: Default::default(),
            stopped: false,
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
        .create(
            &PostParams::default(),
            &claim_by("ada-1", "e2e-use", "e2e-ada"),
        )
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
    // Who asks is stamped as the cluster signs them in (#2056): bea cannot claim as ada.
    match as_bea
        .create(
            &PostParams::default(),
            &claim_by("bea-0", "e2e-use", "e2e-ada"),
        )
        .await
    {
        Err(kube::Error::Api(status)) => {
            assert_eq!(status.code, 403, "{status:?}");
            assert!(
                status.message.contains("set spec.requester to e2e-bea"),
                "{status:?}"
            );
        }
        other => panic!("bea's claim as ada was not refused: {other:?}"),
    }
    as_bea
        .create(
            &PostParams::default(),
            &claim_by("bea-1", "e2e-use", "e2e-bea"),
        )
        .await
        .expect("bea may use the pool");
    // And the stamp never changes.
    let changed = claims_in(&client, &scratch.name)
        .patch(
            "bea-1",
            &PatchParams::default(),
            &Patch::Merge(serde_json::json!({ "spec": { "requester": "e2e-ada" } })),
        )
        .await;
    assert!(
        matches!(&changed, Err(kube::Error::Api(status)) if status.code == 422
            && status.message.contains("requester cannot change")),
        "{changed:?}"
    );

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

/// Whether the operator has asked upstream for a runner for any claim in `namespace`.
async fn upstream_claims_in(client: &Client, namespace: &str) -> usize {
    Api::<purlis_cluster_operator::upstream::SandboxClaim>::namespaced(client.clone(), namespace)
        .list(&ListParams::default())
        .await
        .expect("upstream claims")
        .items
        .len()
}

/// Long enough after a binding is made for a claim to be acted on (`SETTLE`, and two seconds).
fn settled() -> Duration {
    Duration::from_secs(purlis_cluster_operator::SETTLE.as_secs().unsigned_abs() + 2)
}

/// The installed binding made again as a new object, unless it is there: from then on the
/// policy is in force.
async fn restore_binding(
    bindings: &Api<k8s_openapi::api::admissionregistration::v1::ValidatingAdmissionPolicyBinding>,
    installed: &k8s_openapi::api::admissionregistration::v1::ValidatingAdmissionPolicyBinding,
) {
    let name = installed.name_any();
    if bindings.get_opt(&name).await.ok().flatten().is_some() {
        return;
    }
    let mut again = installed.clone();
    again.metadata = ObjectMeta {
        name: Some(name),
        labels: installed.metadata.labels.clone(),
        annotations: installed.metadata.annotations.clone(),
        ..ObjectMeta::default()
    };
    bindings
        .create(&PostParams::default(), &again)
        .await
        .expect("the binding made again");
}

#[tokio::test]
#[ignore = "needs a kind cluster: operator/e2e/kind.sh"]
async fn only_a_claim_made_while_the_policy_is_in_force_is_acted_on() {
    use futures::FutureExt;
    use k8s_openapi::api::admissionregistration::v1::ValidatingAdmissionPolicyBinding;
    use purlis_cluster_operator::POLICY;

    let client = Client::try_default().await.expect("the cluster");
    let bindings = Api::<ValidatingAdmissionPolicyBinding>::all(client.clone());
    let installed = bindings.get(POLICY).await.expect("the installed binding");
    bindings
        .delete(POLICY, &DeleteParams::default())
        .await
        .expect("the binding deleted");
    eventually("the binding is gone", Duration::from_secs(30), || async {
        bindings.get_opt(POLICY).await.ok()?.is_none().then_some(())
    })
    .await;

    let outcome = std::panic::AssertUnwindSafe(async {
        let scratch = Scratch::new(&client, "e2e-policy").await;
        let pools = Api::<Pool>::all(client.clone());
        let pool = Pool::new(
            "e2e-policy",
            PoolSpec {
                image: runner_image(),
                kept_volume: Default::default(),
                stopped: false,
            },
        );
        pools
            .patch(
                "e2e-policy",
                &PatchParams::apply("e2e").force(),
                &Patch::Apply(&pool),
            )
            .await
            .expect("the pool");
        let claims = Api::<Claim>::namespaced(client.clone(), &scratch.name);
        claims
            .create(&PostParams::default(), &claim("ada-1", "e2e-policy"))
            .await
            .expect("the claim, made with no policy in force");

        eventually(
            "with no binding, the claim says the policy is not in force",
            Duration::from_secs(60),
            || async {
                let claim = claims.get("ada-1").await.ok()?;
                (ready_of(&claim)? == ("False".into(), "PolicyNotInForce".into())).then_some(())
            },
        )
        .await;
        assert_eq!(upstream_claims_in(&client, &scratch.name).await, 0);

        restore_binding(&bindings, &installed).await;
        eventually(
            "the claim made before the binding is refused",
            Duration::from_secs(60),
            || async {
                let claim = claims.get("ada-1").await.ok()?;
                (ready_of(&claim)? == ("False".into(), "MadeBeforePolicy".into())).then_some(())
            },
        )
        .await;
        assert_eq!(upstream_claims_in(&client, &scratch.name).await, 0);

        // A claim made while the binding settles is refused too: wait past it.
        tokio::time::sleep(settled()).await;
        claims
            .create(&PostParams::default(), &claim("ada-2", "e2e-policy"))
            .await
            .expect("a claim made under the policy");
        eventually(
            "the claim made after the binding is acted on",
            Duration::from_secs(60),
            || async { (upstream_claims_in(&client, &scratch.name).await == 1).then_some(()) },
        )
        .await;
        let refused = claims.get("ada-1").await.expect("ada-1");
        assert_eq!(
            ready_of(&refused),
            Some(("False".into(), "MadeBeforePolicy".into()))
        );

        let _ = pools.delete("e2e-policy", &DeleteParams::default()).await;
        scratch.end().await;
    })
    .catch_unwind()
    .await;
    restore_binding(&bindings, &installed).await;
    // The tests after this one make claims at once: past the binding's settling time.
    tokio::time::sleep(settled()).await;
    if let Err(panic) = outcome {
        std::panic::resume_unwind(panic);
    }
}

/// After the policy's binding is made again, a Claim made before it is refused but its runner
/// runs on. Its pool's template, and the network policy upstream makes of it, stay as long as
/// that Claim does: a Claim made under the new binding never takes them over or away.
#[tokio::test]
#[ignore = "needs a kind cluster: operator/e2e/kind.sh"]
async fn a_refused_claims_running_runner_keeps_its_template_and_network_policy() {
    use futures::FutureExt;
    use k8s_openapi::api::admissionregistration::v1::ValidatingAdmissionPolicyBinding;
    use k8s_openapi::api::networking::v1::NetworkPolicy;
    use purlis_cluster_operator::POLICY;
    use purlis_cluster_operator::upstream::{SandboxTemplate, name_for};

    let client = Client::try_default().await.expect("the cluster");
    let bindings = Api::<ValidatingAdmissionPolicyBinding>::all(client.clone());
    let installed = bindings.get(POLICY).await.expect("the installed binding");
    let scratch = Scratch::new(&client, "e2e-refused-runs").await;
    let pools = Api::<Pool>::all(client.clone());

    let outcome = std::panic::AssertUnwindSafe(async {
        let pool = Pool::new(
            "e2e-refused-runs",
            PoolSpec {
                image: runner_image(),
                kept_volume: Default::default(),
                stopped: false,
            },
        );
        pools
            .patch(
                "e2e-refused-runs",
                &PatchParams::apply("e2e").force(),
                &Patch::Apply(&pool),
            )
            .await
            .expect("the pool");
        let claims = Api::<Claim>::namespaced(client.clone(), &scratch.name);
        claims
            .create(&PostParams::default(), &claim("ada-1", "e2e-refused-runs"))
            .await
            .expect("the first claim");
        eventually(
            "the first claim is ready",
            Duration::from_secs(300),
            || async {
                let claim = claims.get("ada-1").await.ok()?;
                (ready_of(&claim)?.0 == "True").then_some(())
            },
        )
        .await;
        let pods = Api::<Pod>::namespaced(client.clone(), &scratch.name);
        let running = pods.list(&ListParams::default()).await.expect("pods");
        assert_eq!(running.items.len(), 1);
        let runner = running.items[0].clone();

        // The binding made again: the first claim was made before it.
        bindings
            .delete(POLICY, &DeleteParams::default())
            .await
            .expect("the binding deleted");
        eventually("the binding is gone", Duration::from_secs(30), || async {
            bindings.get_opt(POLICY).await.ok()?.is_none().then_some(())
        })
        .await;
        restore_binding(&bindings, &installed).await;
        eventually(
            "the first claim is refused",
            Duration::from_secs(60),
            || async {
                let claim = claims.get("ada-1").await.ok()?;
                (ready_of(&claim)? == ("False".into(), "MadeBeforePolicy".into())).then_some(())
            },
        )
        .await;
        tokio::time::sleep(settled()).await;

        // A claim made under the new binding: the template is owned only by a claim the
        // operator refused, so it is not taken for the operator's, nothing is written, and the
        // claim says whose it is.
        claims
            .create(&PostParams::default(), &claim("ada-2", "e2e-refused-runs"))
            .await
            .expect("a claim made under the new binding");
        eventually(
            "the new claim leaves the refused claim's template as it is",
            Duration::from_secs(60),
            || async {
                let claim = claims.get("ada-2").await.ok()?;
                let ready = claim
                    .status?
                    .conditions
                    .into_iter()
                    .find(|c| c.type_ == "Ready")?;
                (ready.reason == "HeldByClaimsMadeBeforePolicy" && ready.message.contains("ada-1"))
                    .then_some(())
            },
        )
        .await;
        claims
            .delete("ada-2", &DeleteParams::foreground())
            .await
            .expect("the new claim deleted");
        eventually("the new claim is gone", Duration::from_secs(60), || async {
            claims.get_opt("ada-2").await.ok()?.is_none().then_some(())
        })
        .await;
        tokio::time::sleep(Duration::from_secs(10)).await;

        let template = name_for("e2e-refused-runs");
        let held = Api::<SandboxTemplate>::namespaced(client.clone(), &scratch.name)
            .get(&template)
            .await
            .expect("the refused claim's runner keeps its template");
        assert!(
            held.owner_references()
                .iter()
                .any(|owner| owner.kind == "Claim" && owner.name == "ada-1"),
            "{held:?}"
        );
        Api::<NetworkPolicy>::namespaced(client.clone(), &scratch.name)
            .get(&format!("{template}-network-policy"))
            .await
            .expect("and the network policy upstream makes of it");
        let still = pods.get(&runner.name_any()).await.expect("the runner");
        assert_eq!(still.uid(), runner.uid());

        let _ = pools
            .delete("e2e-refused-runs", &DeleteParams::default())
            .await;
    })
    .catch_unwind()
    .await;
    scratch.end().await;
    restore_binding(&bindings, &installed).await;
    // The tests after this one make claims at once: past the binding's settling time.
    tokio::time::sleep(settled()).await;
    if let Err(panic) = outcome {
        std::panic::resume_unwind(panic);
    }
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

/// The claim `name`'s statement's `pod:` line, once the operator vouches for its runner.
async fn vouched_pod(claims: &Api<Claim>, name: &str) -> (String, String) {
    eventually(
        &format!("the operator vouches for {name}'s runner"),
        Duration::from_secs(300),
        || async {
            let statement = claims.get(name).await.ok()?.status?.link_key?.statement;
            let pod = statement
                .lines()
                .find_map(|line| line.strip_prefix("pod: "))?
                .to_owned();
            Some((pod, statement))
        },
    )
    .await
}

#[tokio::test]
#[ignore = "needs a kind cluster: operator/e2e/kind.sh"]
async fn a_fresh_claim_takes_a_warm_runner_and_a_kept_one_starts_cold_saying_its_stages() {
    use purlis_cluster_operator::upstream::{SandboxWarmPool, name_for};
    let client = Client::try_default().await.expect("the cluster");
    let scratch = Scratch::new(&client, "e2e-warm").await;
    let pools = Api::<Pool>::all(client.clone());
    let pool = Pool::new(
        "e2e-warm",
        PoolSpec {
            image: runner_image(),
            kept_volume: purlis_cluster_operator::KeptVolume {
                size: "1Gi".into(),
                storage_class_name: None,
            },
            stopped: false,
        },
    );
    pools
        .patch(
            "e2e-warm",
            &PatchParams::apply("e2e").force(),
            &Patch::Apply(&pool),
        )
        .await
        .expect("the pool");
    let claims = claims_in(&client, &scratch.name);
    // The pool's warm pool is made with its first claim, keeping no runner warm.
    claims
        .create(&PostParams::default(), &claim("ada-first", "e2e-warm"))
        .await
        .expect("the first claim");
    vouched_pod(&claims, "ada-first").await;
    let warm_pools = Api::<SandboxWarmPool>::namespaced(client.clone(), &scratch.name);
    let name = name_for("e2e-warm");
    assert_eq!(
        warm_pools
            .get(&name)
            .await
            .expect("the warm pool")
            .spec
            .replicas,
        0,
        "no runner kept warm by default"
    );
    // A team admin keeps one runner warm; the operator keeps the number it finds.
    warm_pools
        .patch(
            &name,
            &PatchParams::default(),
            &Patch::Merge(serde_json::json!({ "spec": { "replicas": 1 } })),
        )
        .await
        .expect("one runner kept warm");
    let pods = Api::<Pod>::namespaced(client.clone(), &scratch.name);
    let warm = eventually("a warm runner, ready", Duration::from_secs(300), || async {
        let list = pods.list(&ListParams::default()).await.ok()?;
        list.items.into_iter().find_map(|pod| {
            let warm = pod
                .labels()
                .contains_key("agents.x-k8s.io/warm-pool-sandbox");
            let ready = pod
                .status
                .as_ref()?
                .conditions
                .as_ref()?
                .iter()
                .any(|c| c.type_ == "Ready" && c.status == "True");
            (warm && ready).then(|| pod.name_any())
        })
    })
    .await;

    // A fresh claim takes it.
    claims
        .create(&PostParams::default(), &claim("ada-fresh", "e2e-warm"))
        .await
        .expect("a fresh claim");
    let (pod, _) = vouched_pod(&claims, "ada-fresh").await;
    assert_eq!(pod, warm, "the fresh claim took the warm runner");
    assert_eq!(
        warm_pools
            .get(&name)
            .await
            .expect("the warm pool")
            .spec
            .replicas,
        1,
        "the operator kept the number the admin set"
    );

    // A kept claim starts a runner of its own, cold, saying its stages as it goes.
    let mut kept = claim("ada-kept", "e2e-warm");
    kept.spec.kept = true;
    claims
        .create(&PostParams::default(), &kept)
        .await
        .expect("a kept claim");
    let mut said = Vec::<String>::new();
    let start = Instant::now();
    loop {
        if let Some((status, reason)) = claims
            .get("ada-kept")
            .await
            .ok()
            .as_ref()
            .and_then(ready_of)
        {
            if status == "True" {
                break;
            }
            if said.last() != Some(&reason) {
                eprintln!("ada-kept Ready=False {reason}");
                said.push(reason);
            }
        }
        assert!(
            start.elapsed() < Duration::from_secs(300),
            "ada-kept is not ready within 300s; it said {said:?}"
        );
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    assert!(
        !said.is_empty(),
        "a cold claim says its stage while it starts"
    );
    for reason in &said {
        assert!(
            ["Scheduling", "Pulling", "Starting"].contains(&reason.as_str()),
            "{said:?}"
        );
    }
    let (pod, _) = vouched_pod(&claims, "ada-kept").await;
    assert_eq!(pod, "ada-kept", "a kept claim's runner is its own");
    let volumes = Api::<k8s_openapi::api::core::v1::PersistentVolumeClaim>::namespaced(
        client.clone(),
        &scratch.name,
    );
    let volume = volumes
        .get("purlis-kept-ada-kept")
        .await
        .expect("the kept claim's own volume");
    assert_eq!(
        volume
            .spec
            .as_ref()
            .and_then(|spec| spec.resources.as_ref())
            .and_then(|resources| resources.requests.as_ref())
            .and_then(|requests| requests.get("storage"))
            .map(|size| size.0.as_str()),
        Some("1Gi")
    );

    // The pool's keptVolume edited: the kept claim keeps the volume it was made with, and the
    // operator goes on looking at it (#2057).
    pools
        .patch(
            "e2e-warm",
            &PatchParams::default(),
            &Patch::Merge(serde_json::json!({ "spec": { "keptVolume": { "size": "2Gi" } } })),
        )
        .await
        .expect("the pool's kept volume edited");
    // Its pod lost, it is made again on that volume. (That it keeps its pairing there is the
    // desktop's side to show: crates/purlis-core/tests/a_chat_runs_on_a_cluster_runner_through_kubectl_exec.rs.)
    let first = pods.get("ada-kept").await.expect("its pod").uid();
    pods.delete("ada-kept", &DeleteParams::default())
        .await
        .expect("its pod deleted");
    eventually(
        "its pod made again and ready",
        Duration::from_secs(300),
        || async {
            let pod = pods.get_opt("ada-kept").await.ok()??;
            let ready = pod
                .status
                .as_ref()?
                .conditions
                .as_ref()?
                .iter()
                .any(|c| c.type_ == "Ready" && c.status == "True");
            (pod.uid() != first && ready).then_some(())
        },
    )
    .await;
    // Never paired, it made a new key: the operator vouches for that one, not the old.
    let published = eventually(
        "the new pod's first step published its key",
        Duration::from_secs(60),
        || async {
            let pod = pods.get("ada-kept").await.ok()?;
            pod.status?
                .init_container_statuses?
                .into_iter()
                .find(|s| s.name == "link-key")?
                .state?
                .terminated?
                .message
        },
    )
    .await;
    eventually(
        "the operator vouches for the new pod's key",
        Duration::from_secs(180),
        || async {
            let statement = claims
                .get("ada-kept")
                .await
                .ok()?
                .status?
                .link_key?
                .statement;
            statement
                .contains(&format!("key: {published}\n"))
                .then_some(())
        },
    )
    .await;
    assert_eq!(
        volumes
            .get("purlis-kept-ada-kept")
            .await
            .expect("its volume")
            .uid(),
        volume.uid(),
        "on the same volume"
    );
    // A kept claim made after the edit has the new size.
    let mut later = claim("ada-later", "e2e-warm");
    later.spec.kept = true;
    claims
        .create(&PostParams::default(), &later)
        .await
        .expect("a kept claim made after the edit");
    let size = eventually("its volume", Duration::from_secs(120), || async {
        let volume = volumes.get_opt("purlis-kept-ada-later").await.ok()??;
        volume
            .spec?
            .resources?
            .requests?
            .get("storage")
            .map(|size| size.0.clone())
    })
    .await;
    assert_eq!(size, "2Gi");
    // A fresh claim cannot be made kept, nor a kept one fresh.
    let refused = claims
        .patch(
            "ada-fresh",
            &PatchParams::default(),
            &Patch::Merge(serde_json::json!({ "spec": { "kept": true } })),
        )
        .await;
    assert!(refused.is_err(), "{refused:?}");
    // And a kept claim stays kept.
    let refused = claims
        .patch(
            "ada-kept",
            &PatchParams::default(),
            &Patch::Merge(serde_json::json!({ "spec": { "kept": false } })),
        )
        .await;
    assert!(refused.is_err(), "{refused:?}");

    for name in ["ada-first", "ada-fresh", "ada-kept", "ada-later"] {
        let _ = claims.delete(name, &DeleteParams::foreground()).await;
    }
    // A kept claim's volume, which holds its runner's key, goes with it.
    eventually(
        "the kept claims' volumes go with them",
        Duration::from_secs(180),
        || async {
            let kept = volumes.get_opt("purlis-kept-ada-kept").await.ok()?;
            let later = volumes.get_opt("purlis-kept-ada-later").await.ok()?;
            (kept.is_none() && later.is_none()).then_some(())
        },
    )
    .await;
    let _ = pools.delete("e2e-warm", &DeleteParams::default()).await;
    scratch.end().await;
}

/// A pool `name` of the runner image, its kept volume 1Gi, applied by the tests.
async fn small_pool(client: &Client, name: &str) -> Api<Pool> {
    let pools = Api::<Pool>::all(client.clone());
    let pool = Pool::new(
        name,
        PoolSpec {
            image: runner_image(),
            kept_volume: purlis_cluster_operator::KeptVolume {
                size: "1Gi".into(),
                storage_class_name: None,
            },
            stopped: false,
        },
    );
    pools
        .patch(
            name,
            &PatchParams::apply("e2e").force(),
            &Patch::Apply(&pool),
        )
        .await
        .expect("the pool");
    pools
}

/// The sandboxes in `namespace`, by name.
async fn sandboxes_in(client: &Client, namespace: &str) -> Vec<String> {
    use purlis_cluster_operator::upstream::Sandbox;
    let mut names: Vec<String> = Api::<Sandbox>::namespaced(client.clone(), namespace)
        .list(&ListParams::default())
        .await
        .expect("the sandboxes")
        .items
        .iter()
        .map(|sandbox| sandbox.name_any())
        .collect();
    names.sort();
    names
}

/// Whether `pod` is ready.
fn pod_ready(pod: &Pod) -> bool {
    pod.status
        .as_ref()
        .and_then(|status| status.conditions.as_ref())
        .is_some_and(|conditions| {
            conditions
                .iter()
                .any(|c| c.type_ == "Ready" && c.status == "True")
        })
}

#[tokio::test]
#[ignore = "needs a kind cluster: operator/e2e/kind.sh"]
async fn a_pool_scales_from_zero_for_a_claim_and_back_to_zero() {
    use purlis_cluster_operator::upstream::{SandboxWarmPool, name_for};
    let client = Client::try_default().await.expect("the cluster");
    let scratch = Scratch::new(&client, "e2e-zero").await;
    let pools = small_pool(&client, "e2e-zero").await;
    let pods = Api::<Pod>::namespaced(client.clone(), &scratch.name);
    // Nothing runs for a pool no one claims.
    assert!(sandboxes_in(&client, &scratch.name).await.is_empty());
    let claims = claims_in(&client, &scratch.name);
    claims
        .create(&PostParams::default(), &claim("ada-zero", "e2e-zero"))
        .await
        .expect("the claim");
    vouched_pod(&claims, "ada-zero").await;
    // One runner, the claim's, and none kept warm.
    assert_eq!(sandboxes_in(&client, &scratch.name).await, ["ada-zero"]);
    let warm_pools = Api::<SandboxWarmPool>::namespaced(client.clone(), &scratch.name);
    assert_eq!(
        warm_pools
            .get(&name_for("e2e-zero"))
            .await
            .expect("the warm pool")
            .spec
            .replicas,
        0
    );
    claims
        .delete("ada-zero", &DeleteParams::foreground())
        .await
        .expect("the claim deleted");
    eventually(
        "back to no runner at all",
        Duration::from_secs(180),
        || async {
            let none = sandboxes_in(&client, &scratch.name).await.is_empty();
            let pods = pods.list(&ListParams::default()).await.ok()?;
            (none && pods.items.is_empty()).then_some(())
        },
    )
    .await;
    let _ = pools.delete("e2e-zero", &DeleteParams::default()).await;
    scratch.end().await;
}

/// The reason of `name`'s `Ready`, once its statement is gone and it says `Stopped`.
async fn says_stopped(claims: &Api<Claim>, name: &str) -> String {
    eventually(
        &format!("{name} says it is stopped"),
        Duration::from_secs(120),
        || async {
            let claim = claims.get(name).await.ok()?;
            let ready = claim
                .status
                .as_ref()?
                .conditions
                .iter()
                .find(|c| c.type_ == "Ready")?
                .clone();
            let gone = claim.status.as_ref()?.link_key.is_none();
            (gone && ready.status == "False" && ready.reason == "Stopped").then_some(ready.message)
        },
    )
    .await
}

#[tokio::test]
#[ignore = "needs a kind cluster: operator/e2e/kind.sh"]
async fn stopped_on_a_pool_or_a_claim_suspends_its_runners_and_never_destroys_them() {
    use purlis_cluster_operator::upstream::{SUSPENDED, Sandbox};
    let client = Client::try_default().await.expect("the cluster");
    let scratch = Scratch::new(&client, "e2e-stop").await;
    let pools = small_pool(&client, "e2e-stop").await;
    let claims = claims_in(&client, &scratch.name);
    let pods = Api::<Pod>::namespaced(client.clone(), &scratch.name);
    let sandboxes = Api::<Sandbox>::namespaced(client.clone(), &scratch.name);
    let volumes = Api::<k8s_openapi::api::core::v1::PersistentVolumeClaim>::namespaced(
        client.clone(),
        &scratch.name,
    );
    let mut kept = claim("ada-kept", "e2e-stop");
    kept.spec.kept = true;
    claims
        .create(&PostParams::default(), &kept)
        .await
        .expect("a kept claim");
    claims
        .create(&PostParams::default(), &claim("ada-fresh", "e2e-stop"))
        .await
        .expect("a fresh claim");
    let (_, statement) = vouched_pod(&claims, "ada-kept").await;
    let kept_key = key_of(&statement);
    let (fresh_pod, _) = vouched_pod(&claims, "ada-fresh").await;
    let volume = volumes
        .get("purlis-kept-ada-kept")
        .await
        .expect("the kept claim's volume")
        .uid();

    // The pool stopped from kubectl: every runner of it suspended.
    let stop = |stopped: bool| serde_json::json!({ "spec": { "stopped": stopped } });
    pools
        .patch(
            "e2e-stop",
            &PatchParams::default(),
            &Patch::Merge(stop(true)),
        )
        .await
        .expect("the pool stopped");
    for name in ["ada-kept", "ada-fresh"] {
        let message = says_stopped(&claims, name).await;
        assert!(
            message.contains("The pool e2e-stop is stopped"),
            "{message}"
        );
    }
    eventually("their pods end", Duration::from_secs(180), || async {
        let kept = pods.get_opt("ada-kept").await.ok()?;
        let fresh = pods.get_opt(&fresh_pod).await.ok()?;
        (kept.is_none() && fresh.is_none()).then_some(())
    })
    .await;
    for name in ["ada-kept", &fresh_pod] {
        let sandbox = sandboxes.get(name).await.expect("the sandbox is kept");
        assert_eq!(sandbox.spec.operating_mode.as_deref(), Some(SUSPENDED));
    }
    assert_eq!(
        volumes
            .get("purlis-kept-ada-kept")
            .await
            .expect("the kept claim's volume is kept")
            .uid(),
        volume
    );
    // A claim made while its pool is stopped gets no runner yet.
    claims
        .create(&PostParams::default(), &claim("ada-later", "e2e-stop"))
        .await
        .expect("a claim on a stopped pool");
    says_stopped(&claims, "ada-later").await;
    assert_eq!(upstream_claims_in(&client, &scratch.name).await, 2);

    // Run again: the kept runner comes back on its own volume, and the later claim is served.
    pools
        .patch(
            "e2e-stop",
            &PatchParams::default(),
            &Patch::Merge(stop(false)),
        )
        .await
        .expect("the pool running again");
    for name in ["ada-fresh", "ada-later"] {
        vouched_pod(&claims, name).await;
    }
    // The kept runner is the same runner, paired as it was: the same key vouched for (#2057).
    let (_, statement) = vouched_pod(&claims, "ada-kept").await;
    assert_eq!(key_of(&statement), kept_key);
    assert_eq!(
        volumes
            .get("purlis-kept-ada-kept")
            .await
            .expect("the kept claim's volume")
            .uid(),
        volume
    );

    // One claim stopped alone: only its runner is suspended.
    claims
        .patch(
            "ada-kept",
            &PatchParams::default(),
            &Patch::Merge(stop(true)),
        )
        .await
        .expect("the claim stopped");
    let message = says_stopped(&claims, "ada-kept").await;
    assert!(message.contains("This claim is stopped"), "{message}");
    eventually("its pod ends", Duration::from_secs(180), || async {
        pods.get_opt("ada-kept").await.ok()?.is_none().then_some(())
    })
    .await;
    let others = claims.get("ada-later").await.expect("the other claim");
    assert_eq!(ready_of(&others).map(|r| r.0).as_deref(), Some("True"));
    claims
        .patch(
            "ada-kept",
            &PatchParams::default(),
            &Patch::Merge(stop(false)),
        )
        .await
        .expect("the claim running again");
    let (_, statement) = vouched_pod(&claims, "ada-kept").await;
    assert_eq!(key_of(&statement), kept_key);
    eventually("its pod is back", Duration::from_secs(180), || async {
        pods.get_opt("ada-kept")
            .await
            .ok()?
            .filter(pod_ready)
            .map(|_| ())
    })
    .await;

    // Its pool deleted, a runner it served is still stopped by its claim.
    let (later_pod, _) = vouched_pod(&claims, "ada-later").await;
    pools
        .delete("e2e-stop", &DeleteParams::default())
        .await
        .expect("the pool deleted");
    eventually(
        "the claim says its pool is deleted",
        Duration::from_secs(120),
        || async {
            let claim = claims.get("ada-later").await.ok()?;
            (ready_of(&claim)?.1 == "PoolDeleted").then_some(())
        },
    )
    .await;
    claims
        .patch(
            "ada-later",
            &PatchParams::default(),
            &Patch::Merge(stop(true)),
        )
        .await
        .expect("the claim stopped");
    let message = says_stopped(&claims, "ada-later").await;
    assert!(message.contains("This claim is stopped"), "{message}");
    eventually("its pod ends", Duration::from_secs(180), || async {
        pods.get_opt(&later_pod).await.ok()?.is_none().then_some(())
    })
    .await;

    for name in ["ada-kept", "ada-fresh", "ada-later"] {
        let _ = claims.delete(name, &DeleteParams::foreground()).await;
    }
    let _ = pools.delete("e2e-stop", &DeleteParams::default()).await;
    scratch.end().await;
}

#[tokio::test]
#[ignore = "needs a kind cluster: operator/e2e/kind.sh"]
async fn a_warm_runner_of_an_earlier_template_is_replaced_and_a_fresh_claim_takes_a_current_one() {
    use purlis_cluster_operator::upstream::{Sandbox, SandboxWarmPool, name_for};
    use purlis_cluster_operator::vouch::pod_name_of;
    let client = Client::try_default().await.expect("the cluster");
    let scratch = Scratch::new(&client, "e2e-stale").await;
    let pools = small_pool(&client, "e2e-stale").await;
    let claims = claims_in(&client, &scratch.name);
    let pods = Api::<Pod>::namespaced(client.clone(), &scratch.name);
    // The first claim makes the pool's warm pool; a team admin keeps one runner warm.
    claims
        .create(&PostParams::default(), &claim("ada-first", "e2e-stale"))
        .await
        .expect("the first claim");
    let (first, _) = vouched_pod(&claims, "ada-first").await;
    Api::<SandboxWarmPool>::namespaced(client.clone(), &scratch.name)
        .patch(
            &name_for("e2e-stale"),
            &PatchParams::default(),
            &Patch::Merge(serde_json::json!({ "spec": { "replicas": 1 } })),
        )
        .await
        .expect("one runner kept warm");
    let warm_ready = |not: Option<String>| {
        let pods = pods.clone();
        let first = first.clone();
        async move {
            let list = pods.list(&ListParams::default()).await.ok()?;
            list.items.into_iter().find_map(|pod| {
                let warm = pod
                    .labels()
                    .contains_key("agents.x-k8s.io/warm-pool-sandbox")
                    && pod.name_any() != first
                    && Some(pod.name_any()) != not;
                (warm && pod_ready(&pod) && pod.metadata.deletion_timestamp.is_none())
                    .then(|| pod.name_any())
            })
        }
    };
    let stale = eventually("a warm runner, ready", Duration::from_secs(300), || {
        warm_ready(None)
    })
    .await;

    // The pool changes: the warm runner, of the earlier template, is replaced.
    pools
        .patch(
            "e2e-stale",
            &PatchParams::default(),
            &Patch::Merge(serde_json::json!({ "spec": { "keptVolume": { "size": "2Gi" } } })),
        )
        .await
        .expect("the pool changed");
    eventually(
        "the stale warm runner goes",
        Duration::from_secs(180),
        || async { pods.get_opt(&stale).await.ok()?.is_none().then_some(()) },
    )
    .await;
    let current = eventually(
        "a warm runner of the current template, ready",
        Duration::from_secs(300),
        || warm_ready(Some(stale.clone())),
    )
    .await;

    // A fresh claim takes it, and is vouched for: never stuck on the stale one.
    claims
        .create(&PostParams::default(), &claim("ada-fresh", "e2e-stale"))
        .await
        .expect("a fresh claim");
    let (pod, _) = vouched_pod(&claims, "ada-fresh").await;
    assert_ne!(pod, stale);
    assert_eq!(pod, current, "the fresh claim took the current warm runner");
    // The pod vouched for is the one its sandbox records, as upstream records an adopted pod.
    let upstream_claim = Api::<purlis_cluster_operator::upstream::SandboxClaim>::namespaced(
        client.clone(),
        &scratch.name,
    )
    .get("ada-fresh")
    .await
    .expect("its upstream claim");
    let sandbox_name = upstream_claim
        .status
        .and_then(|status| status.sandbox)
        .expect("its sandbox")
        .name;
    let sandbox = Api::<Sandbox>::namespaced(client.clone(), &scratch.name)
        .get(&sandbox_name)
        .await
        .expect("the sandbox");
    assert_eq!(pod_name_of(&sandbox.metadata), pod);

    for name in ["ada-first", "ada-fresh"] {
        let _ = claims.delete(name, &DeleteParams::foreground()).await;
    }
    let _ = pools.delete("e2e-stale", &DeleteParams::default()).await;
    scratch.end().await;
}

/// The `key:` line of a statement.
fn key_of(statement: &str) -> String {
    statement
        .lines()
        .find_map(|line| line.strip_prefix("key: "))
        .expect("a key line")
        .to_owned()
}

/// Whether `result` is the cluster's refusal by an admission policy.
fn refused_by_policy<T: std::fmt::Debug>(result: &Result<T, kube::Error>) -> bool {
    matches!(result, Err(kube::Error::Api(status)) if status.code == 403 || status.code == 422)
}

#[tokio::test]
#[ignore = "needs a kind cluster: operator/e2e/kind.sh"]
async fn the_cluster_lets_the_operator_change_only_the_mode_of_a_claims_sandbox() {
    use kube::api::{ApiResource, DynamicObject};
    use purlis_cluster_operator::upstream::{Sandbox, sandbox_in_mode};
    let client = Client::try_default().await.expect("the cluster");
    let scratch = Scratch::new(&client, "e2e-sandbox-right").await;
    let pools = small_pool(&client, "e2e-sandbox-right").await;
    let claims = claims_in(&client, &scratch.name);
    claims
        .create(&PostParams::default(), &claim("ada-1", "e2e-sandbox-right"))
        .await
        .expect("the claim");
    let (pod, _) = vouched_pod(&claims, "ada-1").await;
    let as_operator = acting_as(SERVICE_ACCOUNT).await;
    let sandboxes = Api::<Sandbox>::namespaced(as_operator, &scratch.name);
    // Dry runs: admission is asked, and nothing changes.
    let dry = PatchParams {
        dry_run: true,
        ..PatchParams::default()
    };
    // Its claim's sandbox, in its mode: let through.
    sandboxes
        .patch(
            &pod,
            &dry,
            &Patch::Merge(serde_json::json!({ "spec": { "operatingMode": "Suspended" } })),
        )
        .await
        .expect("the operator may change a claim's sandbox's mode");
    // Anything else of it: refused by the cluster.
    for (what, body) in [
        (
            "another spec field",
            serde_json::json!({ "spec": { "shutdownPolicy": "Delete" } }),
        ),
        (
            "a label",
            serde_json::json!({ "metadata": { "labels": { "e2e": "x" } } }),
        ),
        (
            "an annotation",
            serde_json::json!({ "metadata": { "annotations": { "e2e": "x" } } }),
        ),
    ] {
        let refused = sandboxes.patch(&pod, &dry, &Patch::Merge(body)).await;
        assert!(refused_by_policy(&refused), "{what}: {refused:?}");
    }
    // Applied to the sandbox it read, by uid: one of another uid under the name is refused.
    let mut other = sandboxes.get(&pod).await.expect("the sandbox");
    other.metadata.uid = Some("00000000-0000-4000-8000-000000000000".into());
    let mut apply = PatchParams::apply("purlis-cluster-operator").force();
    apply.dry_run = true;
    let refused = sandboxes
        .patch(
            &pod,
            &apply,
            &Patch::Apply(&sandbox_in_mode(&other, "Suspended")),
        )
        .await;
    assert!(
        matches!(&refused, Err(kube::Error::Api(status)) if status.code == 409),
        "{refused:?}"
    );

    // A sandbox no SandboxClaim controls, made by someone else: not even its mode.
    let resource = ApiResource::erase::<Sandbox>(&());
    let bare: DynamicObject = serde_json::from_value(serde_json::json!({
        "apiVersion": "agents.x-k8s.io/v1beta1",
        "kind": "Sandbox",
        "metadata": { "name": "someone-elses", "namespace": scratch.name },
        "spec": {
            "operatingMode": "Suspended",
            "podTemplate": { "spec": { "containers": [{ "name": "x", "image": runner_image() }] } },
        },
    }))
    .expect("a sandbox");
    Api::<DynamicObject>::namespaced_with(client.clone(), &scratch.name, &resource)
        .create(&PostParams::default(), &bare)
        .await
        .expect("a sandbox of someone else's");
    let refused = sandboxes
        .patch(
            "someone-elses",
            &dry,
            &Patch::Merge(serde_json::json!({ "spec": { "operatingMode": "Running" } })),
        )
        .await;
    assert!(refused_by_policy(&refused), "{refused:?}");

    let _ = claims.delete("ada-1", &DeleteParams::foreground()).await;
    let _ = pools
        .delete("e2e-sandbox-right", &DeleteParams::default())
        .await;
    scratch.end().await;
}
