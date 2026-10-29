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

    /// Its claims deleted first, and waited for: each one's finalizer removes its runner from
    /// the runner namespace the tests' claims share (#2055), so the next test's claims of the
    /// same names find nothing of these.
    async fn end(self) {
        let claims = Api::<Claim>::namespaced(self.client.clone(), &self.name);
        let _ = claims
            .delete_collection(&DeleteParams::default(), &ListParams::default())
            .await;
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(180) {
            if claims
                .list(&ListParams::default())
                .await
                .is_ok_and(|list| list.items.is_empty())
            {
                break;
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
        let _ = Api::<Namespace>::all(self.client)
            .delete(&self.name, &DeleteParams::default())
            .await;
    }
}

/// Who the tests' own kubeconfig signs in as: kind's cluster admin.
const ADMIN: &str = "kubernetes-admin";

/// The runner namespace of the cluster admin the tests sign in as, where the runners of the
/// tests' claims run (#2055).
fn runners() -> String {
    purlis_cluster_operator::person::runner_namespace(ADMIN)
}

/// The label selector of the pods of the claim `name`'s runner, once its upstream claim is
/// made in the runner namespace: upstream names the upstream claim on each by uid.
async fn runner_selector(client: &Client, name: &str) -> String {
    let upstream = Api::<purlis_cluster_operator::upstream::SandboxClaim>::namespaced(
        client.clone(),
        &runners(),
    );
    eventually(
        &format!("{name}'s upstream claim in the runner namespace"),
        Duration::from_secs(120),
        || async {
            let uid = upstream.get_opt(name).await.ok()??.uid()?;
            Some(format!("agents.x-k8s.io/claim-uid={uid}"))
        },
    )
    .await
}

/// The runner namespace's pods `selector` picks ([`runner_selector`]).
async fn pods_selected(client: &Client, selector: &str) -> Vec<Pod> {
    Api::<Pod>::namespaced(client.clone(), &runners())
        .list(&ListParams::default().labels(selector))
        .await
        .map(|list| list.items)
        .unwrap_or_default()
}

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

    // Its runner is in the runner namespace, and nothing of it in the claim's (#2055).
    let selector = runner_selector(&client, "ada-1").await;
    let pod = eventually("one runner pod", Duration::from_secs(30), || async {
        let list = pods_selected(&client, &selector).await;
        (list.len() == 1).then(|| list[0].name_any())
    })
    .await;
    let here = Api::<Pod>::namespaced(client.clone(), &scratch.name)
        .list(&ListParams::default())
        .await
        .expect("the claim's namespace's pods");
    assert!(here.items.is_empty(), "{:?}", here.items);
    let pods = Api::<Pod>::namespaced(client.clone(), &runners());
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
            pods_selected(&client, &selector)
                .await
                .is_empty()
                .then_some(())
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

    let list = pods_selected(&client, &runner_selector(&client, "ada-1").await).await;
    assert_eq!(list.len(), 1);
    let pod = &list[0];
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
            runner_namespace: &runners(),
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
    // The runner namespace the operator makes, made by a claim on another pool if it is not
    // there yet; then a template in it under the name the operator would use, made by hand:
    // no owner.
    let claims = Api::<Claim>::namespaced(client.clone(), &scratch.name);
    let namespaces = Api::<Namespace>::all(client.clone());
    if namespaces
        .get_opt(&runners())
        .await
        .ok()
        .flatten()
        .is_none()
    {
        let seeds = Api::<Pool>::all(client.clone());
        seeds
            .patch(
                "e2e-in-the-way-seed",
                &PatchParams::apply("e2e").force(),
                &Patch::Apply(&Pool::new(
                    "e2e-in-the-way-seed",
                    PoolSpec {
                        image: runner_image(),
                        kept_volume: Default::default(),
                    },
                )),
            )
            .await
            .expect("the seed pool");
        claims
            .create(
                &PostParams::default(),
                &claim("ada-seed", "e2e-in-the-way-seed"),
            )
            .await
            .expect("a claim that makes the runner namespace");
        eventually(
            "the runner namespace is made",
            Duration::from_secs(60),
            || async { namespaces.get_opt(&runners()).await.ok()?.map(|_| ()) },
        )
        .await;
        let _ = claims.delete("ada-seed", &DeleteParams::default()).await;
        let _ = seeds
            .delete("e2e-in-the-way-seed", &DeleteParams::default())
            .await;
    }
    let templates = Api::<SandboxTemplate>::namespaced(client.clone(), &runners());
    let mut by_hand = upstream::template_for(&pool, &runners(), vec![]);
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
    assert!(message.contains(&runners()), "{message}");
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
    assert!(!upstream_claim_made(&client, "ada-1").await);

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
    // Controlled by the runner namespace, by its uid (#2055).
    let runner_uid = namespaces
        .get(&runners())
        .await
        .expect("the runner namespace")
        .uid();
    assert!(
        ours.owner_references()
            .iter()
            .any(|o| o.kind == "Namespace" && Some(&o.uid) == runner_uid.as_ref()),
        "{ours:?}"
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
    let pods = Api::<Pod>::namespaced(client.clone(), &runners());
    let selector = runner_selector(&client, "ada-1").await;
    let running = pods_selected(&client, &selector).await;
    assert_eq!(running.len(), 1);
    let pod = running[0].clone();

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
    Api::<SandboxTemplate>::namespaced(client.clone(), &runners())
        .get(&template)
        .await
        .expect("the runner's template");
    Api::<NetworkPolicy>::namespaced(client.clone(), &runners())
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
            pods_selected(&client, &selector)
                .await
                .is_empty()
                .then_some(())
        },
    )
    .await;
    scratch.end().await;
}

#[tokio::test]
#[ignore = "needs a kind cluster: operator/e2e/kind.sh"]
async fn a_pools_template_and_warm_pool_go_with_its_persons_last_claim_on_it() {
    use purlis_cluster_operator::upstream::{SandboxTemplate, SandboxWarmPool, name_for};
    let client = Client::try_default().await.expect("the cluster");
    let scratch = Scratch::new(&client, "e2e-last-claim").await;
    let pools = Api::<Pool>::all(client.clone());
    let pool = Pool::new(
        "e2e-last-claim",
        PoolSpec {
            image: runner_image(),
            kept_volume: Default::default(),
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
    let templates = Api::<SandboxTemplate>::namespaced(client.clone(), &runners());
    let warm_pools = Api::<SandboxWarmPool>::namespaced(client.clone(), &runners());
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
    templates.get(&name).await.expect("the template");

    claims
        .delete("ada-2", &DeleteParams::default())
        .await
        .expect("the last claim deleted");
    eventually(
        "the template and warm pool go with the last claim, by its finalizer",
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
    may_as(
        client,
        SERVICE_ACCOUNT,
        Asked {
            group,
            verb,
            resource,
            subresource,
            namespace: Some("default"),
            name: None,
        },
    )
    .await
}

/// What an access review asks of the cluster.
struct Asked<'a> {
    group: &'a str,
    verb: &'a str,
    resource: &'a str,
    subresource: Option<&'a str>,
    namespace: Option<&'a str>,
    name: Option<&'a str>,
}

/// Whether the cluster lets `user` do what `asked` says.
async fn may_as(client: &Client, user: &str, asked: Asked<'_>) -> bool {
    let review = SubjectAccessReview {
        spec: SubjectAccessReviewSpec {
            user: Some(user.to_owned()),
            resource_attributes: Some(ResourceAttributes {
                group: Some(asked.group.to_owned()),
                verb: Some(asked.verb.to_owned()),
                resource: Some(asked.resource.to_owned()),
                subresource: asked.subresource.map(str::to_owned),
                namespace: asked.namespace.map(str::to_owned),
                name: asked.name.map(str::to_owned),
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
    // It deletes no namespace, and binds no role but a person's own runners' (#2055).
    assert!(!may(&client, "", "delete", "namespaces", None).await);
    fn bind(name: &str) -> Asked<'_> {
        Asked {
            group: "rbac.authorization.k8s.io",
            verb: "bind",
            resource: "clusterroles",
            subresource: None,
            namespace: None,
            name: Some(name),
        }
    }
    assert!(!may_as(&client, SERVICE_ACCOUNT, bind("cluster-admin")).await);
    assert!(!may_as(&client, SERVICE_ACCOUNT, bind("admin")).await);
    // The check is the cluster's answer, not an empty one: what the operator needs, it may.
    assert!(may_as(&client, SERVICE_ACCOUNT, bind("purlis-runner-owner")).await);
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

#[tokio::test]
#[ignore = "needs a kind cluster: operator/e2e/kind.sh"]
async fn a_runner_namespace_made_again_as_its_policys_binding_settles_waits_for_it() {
    use k8s_openapi::api::admissionregistration::v1::ValidatingAdmissionPolicyBinding;
    use purlis_cluster_operator::RUNNER_NAMESPACES_POLICY;
    let client = Client::try_default().await.expect("the cluster");
    let scratch = Scratch::new(&client, "e2e-settling").await;
    let pools = Api::<Pool>::all(client.clone());
    pools
        .patch(
            "e2e-settling",
            &PatchParams::apply("e2e").force(),
            &Patch::Apply(&Pool::new(
                "e2e-settling",
                PoolSpec {
                    image: runner_image(),
                    kept_volume: Default::default(),
                },
            )),
        )
        .await
        .expect("the pool");
    let claims = claims_in(&client, &scratch.name);
    claims
        .create(
            &PostParams::default(),
            &claim("ada-settling", "e2e-settling"),
        )
        .await
        .expect("the claim");
    let ready = || async {
        eventually("the claim is ready", Duration::from_secs(400), || async {
            let claim = claims.get("ada-settling").await.ok()?;
            (ready_of(&claim)?.0 == "True").then_some(())
        })
        .await
    };
    ready().await;

    // As on an upgrade that brings the policy: its binding is made while the operator already
    // has a claim to serve, and the claim's runner namespace is not there.
    let bindings = Api::<ValidatingAdmissionPolicyBinding>::all(client.clone());
    let installed = bindings
        .get(RUNNER_NAMESPACES_POLICY)
        .await
        .expect("the installed binding");
    bindings
        .delete(RUNNER_NAMESPACES_POLICY, &DeleteParams::default())
        .await
        .expect("the binding deleted");
    eventually("the binding is gone", Duration::from_secs(30), || async {
        bindings
            .get_opt(RUNNER_NAMESPACES_POLICY)
            .await
            .ok()?
            .is_none()
            .then_some(())
    })
    .await;
    restore_binding(&bindings, &installed).await;
    let namespaces = Api::<Namespace>::all(client.clone());
    let _ = namespaces
        .delete(&runners(), &DeleteParams::default())
        .await;

    // The operator makes it again only once the binding is in force, and serves the claim there.
    let bound = bindings
        .get(RUNNER_NAMESPACES_POLICY)
        .await
        .expect("the binding")
        .creation_timestamp()
        .expect("its creation time");
    let in_force = bound
        .0
        .checked_add(purlis_cluster_operator::SETTLE)
        .expect("a time");
    eventually(
        "the runner namespace made again, after the binding is in force",
        Duration::from_secs(300),
        || async {
            let held = namespaces.get_opt(&runners()).await.ok()??;
            let made = held.creation_timestamp()?;
            (held.metadata.deletion_timestamp.is_none() && made.0 > in_force).then_some(())
        },
    )
    .await;
    eventually(
        "the claim's runner asked for again there",
        Duration::from_secs(120),
        || async {
            upstream_claim_made(&client, "ada-settling")
                .await
                .then_some(())
        },
    )
    .await;
    ready().await;
    let _ = pools.delete("e2e-settling", &DeleteParams::default()).await;
    scratch.end().await;
}

/// Whether `made` was refused at admission, in words that hold `words`.
fn refused_saying<T: std::fmt::Debug>(made: &Result<T, kube::Error>, words: &str) -> bool {
    matches!(made, Err(kube::Error::Api(status)) if status.code == 403 && status.message.contains(words))
}

#[tokio::test]
#[ignore = "needs a kind cluster: operator/e2e/kind.sh"]
async fn a_runner_namespace_and_what_the_operator_binds_are_guarded_at_admission() {
    use k8s_openapi::api::core::v1::ResourceQuota;
    use k8s_openapi::api::rbac::v1::RoleBinding;
    use purlis_cluster_operator::person;
    let client = Client::try_default().await.expect("the cluster");
    let scratch = Scratch::new(&client, "e2e-guarded").await;
    let namespaces = Api::<Namespace>::all(client.clone());

    // A runner namespace made by anyone but the operator, the cluster admin included, marked as
    // the operator's for someone or not: refused.
    for marked in [true, false] {
        let mut forged = person::namespace_for("e2e-forged");
        if !marked {
            forged.metadata.labels = None;
            forged.metadata.annotations = None;
        }
        let made = namespaces.create(&PostParams::default(), &forged).await;
        assert!(
            refused_saying(
                &made,
                "Only the purlis cluster operator makes a runner namespace"
            ),
            "{made:?}"
        );
    }

    // Who a runner namespace the operator made is for is changed by no one else; another label
    // may be.
    let pools = Api::<Pool>::all(client.clone());
    pools
        .patch(
            "e2e-guarded",
            &PatchParams::apply("e2e").force(),
            &Patch::Apply(&Pool::new(
                "e2e-guarded",
                PoolSpec {
                    image: runner_image(),
                    kept_volume: Default::default(),
                },
            )),
        )
        .await
        .expect("the pool");
    let claims = claims_in(&client, &scratch.name);
    claims
        .create(&PostParams::default(), &claim("ada-guarded", "e2e-guarded"))
        .await
        .expect("a claim that makes the runner namespace");
    eventually(
        "the runner namespace is made",
        Duration::from_secs(120),
        || async { namespaces.get_opt(&runners()).await.ok()?.map(|_| ()) },
    )
    .await;
    let retargeted = namespaces
        .patch(
            &runners(),
            &PatchParams::default(),
            &Patch::Merge(serde_json::json!({
                "metadata": { "annotations": { person::REQUESTER_ANNOTATION: "e2e-someone-else" } }
            })),
        )
        .await;
    assert!(
        refused_saying(&retargeted, "changes who it is for"),
        "{retargeted:?}"
    );
    let labelled = namespaces
        .patch(
            &runners(),
            &PatchParams::default(),
            &Patch::Merge(
                serde_json::json!({ "metadata": { "labels": { "e2e/touched": "yes" } } }),
            ),
        )
        .await;
    assert!(labelled.is_ok(), "{labelled:?}");
    let unmarked = namespaces
        .patch(
            &runners(),
            &PatchParams::default(),
            &Patch::Merge(serde_json::json!({
                "metadata": { "labels": { person::MANAGED_BY.0: null } }
            })),
        )
        .await;
    assert!(
        refused_saying(&unmarked, "changes who it is for"),
        "{unmarked:?}"
    );

    // The operator binds purlis-runner-owner in a runner namespace alone, to one person who is
    // not itself, and caps a person there alone: asked as its service account.
    let as_operator = acting_as(SERVICE_ACCOUNT).await;
    let binding = |namespace: &str, subject: serde_json::Value| -> RoleBinding {
        serde_json::from_value(serde_json::json!({
            "apiVersion": "rbac.authorization.k8s.io/v1", "kind": "RoleBinding",
            "metadata": { "name": person::OWNER_ROLE, "namespace": namespace },
            "roleRef": { "apiGroup": "rbac.authorization.k8s.io", "kind": "ClusterRole", "name": person::OWNER_ROLE },
            "subjects": [subject]
        }))
        .expect("a binding")
    };
    let someone = serde_json::json!({ "apiGroup": "rbac.authorization.k8s.io", "kind": "User", "name": "e2e-someone" });
    let itself = serde_json::json!({ "kind": "ServiceAccount", "name": "purlis-cluster-operator", "namespace": "purlis-system" });
    let words = "binds purlis-runner-owner alone, in a runner namespace alone";
    let outside = Api::<RoleBinding>::namespaced(as_operator.clone(), &scratch.name)
        .create(
            &PostParams::default(),
            &binding(&scratch.name, someone.clone()),
        )
        .await;
    assert!(refused_saying(&outside, words), "{outside:?}");
    let to_itself = Api::<RoleBinding>::namespaced(as_operator.clone(), &runners())
        .patch(
            person::OWNER_ROLE,
            &PatchParams::apply("e2e").force(),
            &Patch::Apply(&binding(&runners(), itself)),
        )
        .await;
    assert!(refused_saying(&to_itself, words), "{to_itself:?}");
    // Nor to anyone but the person the runner namespace is for, nor a group, nor to a Role of
    // the role's name: each refused before the cluster looks for the binding already there.
    let in_runners = Api::<RoleBinding>::namespaced(as_operator.clone(), &runners());
    let mut role = binding(
        &runners(),
        serde_json::json!({ "apiGroup": "rbac.authorization.k8s.io", "kind": "User", "name": ADMIN }),
    );
    role.role_ref.kind = "Role".into();
    for (what, wrong) in [
        ("another person", binding(&runners(), someone.clone())),
        (
            "a group",
            binding(
                &runners(),
                serde_json::json!({ "apiGroup": "rbac.authorization.k8s.io", "kind": "Group", "name": "system:authenticated" }),
            ),
        ),
        (
            "a system user",
            binding(
                &runners(),
                serde_json::json!({ "apiGroup": "rbac.authorization.k8s.io", "kind": "User", "name": "system:kube-scheduler" }),
            ),
        ),
    ] {
        let made = in_runners.create(&PostParams::default(), &wrong).await;
        assert!(refused_saying(&made, words), "{what}: {made:?}");
    }
    // A Role of the role's name: refused, by RBAC itself where there is no such Role for the
    // operator to bind, and by the policy where there is one.
    let made = in_runners.create(&PostParams::default(), &role).await;
    assert!(
        matches!(&made, Err(kube::Error::Api(status)) if matches!(status.code, 403 | 404)),
        "a Role: {made:?}"
    );
    let quota: ResourceQuota = serde_json::from_value(serde_json::json!({
        "apiVersion": "v1", "kind": "ResourceQuota",
        "metadata": { "name": person::QUOTA, "namespace": scratch.name },
        "spec": { "hard": { "pods": "0" } }
    }))
    .expect("a quota");
    let capped = Api::<ResourceQuota>::namespaced(as_operator.clone(), &scratch.name)
        .create(&PostParams::default(), &quota)
        .await;
    assert!(
        refused_saying(&capped, "in a runner namespace alone"),
        "{capped:?}"
    );
    let elsewhere = Namespace {
        metadata: ObjectMeta {
            name: Some("e2e-not-a-runner-namespace".into()),
            ..ObjectMeta::default()
        },
        ..Namespace::default()
    };
    let made = Api::<Namespace>::all(as_operator)
        .create(&PostParams::default(), &elsewhere)
        .await;
    assert!(
        refused_saying(&made, "makes runner namespaces (purlis-runners-*) alone"),
        "{made:?}"
    );

    // What the operator made itself stands: the claim is served.
    eventually("the claim is ready", Duration::from_secs(300), || async {
        let claim = claims.get("ada-guarded").await.ok()?;
        (ready_of(&claim)?.0 == "True").then_some(())
    })
    .await;
    let _ = pools.delete("e2e-guarded", &DeleteParams::default()).await;
    scratch.end().await;
}

#[tokio::test]
#[ignore = "needs a kind cluster: operator/e2e/kind.sh"]
async fn a_persons_runners_run_in_their_own_namespace_within_their_quota_and_go_with_their_claims()
{
    use k8s_openapi::api::core::v1::ResourceQuota;
    use k8s_openapi::api::rbac::v1::RoleBinding;
    use purlis_cluster_operator::person;
    use purlis_cluster_operator::upstream::{
        SandboxClaim, SandboxTemplate, SandboxWarmPool, name_for,
    };
    const WHO: &str = "e2e-quota";
    let client = Client::try_default().await.expect("the cluster");
    let scratch = Scratch::new(&client, "e2e-person").await;
    let pools = Api::<Pool>::all(client.clone());
    let pool = Pool::new(
        "e2e-person",
        PoolSpec {
            image: runner_image(),
            kept_volume: Default::default(),
        },
    );
    pools
        .patch(
            "e2e-person",
            &PatchParams::apply("e2e").force(),
            &Patch::Apply(&pool),
        )
        .await
        .expect("the pool");
    // They may make, read and delete claims in the namespace, and use the pool.
    apply_rbac(
        &client,
        "Role",
        Some(&scratch.name),
        serde_json::json!({
            "apiVersion": "rbac.authorization.k8s.io/v1", "kind": "Role",
            "metadata": { "name": "e2e-person", "namespace": scratch.name },
            "rules": [{ "apiGroups": ["runners.purlis.dev"], "resources": ["claims"],
                        "verbs": ["create", "get", "list", "delete"] }]
        }),
    )
    .await;
    apply_rbac(
        &client,
        "RoleBinding",
        Some(&scratch.name),
        serde_json::json!({
            "apiVersion": "rbac.authorization.k8s.io/v1", "kind": "RoleBinding",
            "metadata": { "name": "e2e-person", "namespace": scratch.name },
            "roleRef": { "apiGroup": "rbac.authorization.k8s.io", "kind": "Role", "name": "e2e-person" },
            "subjects": [{ "apiGroup": "rbac.authorization.k8s.io", "kind": "User", "name": WHO }]
        }),
    )
    .await;
    apply_rbac(
        &client,
        "ClusterRole",
        None,
        serde_json::json!({
            "apiVersion": "rbac.authorization.k8s.io/v1", "kind": "ClusterRole",
            "metadata": { "name": "e2e-person-pool" },
            "rules": [{ "apiGroups": ["runners.purlis.dev"], "resources": ["pools"],
                        "resourceNames": ["e2e-person"], "verbs": ["use"] }]
        }),
    )
    .await;
    apply_rbac(
        &client,
        "ClusterRoleBinding",
        None,
        serde_json::json!({
            "apiVersion": "rbac.authorization.k8s.io/v1", "kind": "ClusterRoleBinding",
            "metadata": { "name": "e2e-person-pool" },
            "roleRef": { "apiGroup": "rbac.authorization.k8s.io", "kind": "ClusterRole", "name": "e2e-person-pool" },
            "subjects": [{ "apiGroup": "rbac.authorization.k8s.io", "kind": "User", "name": WHO }]
        }),
    )
    .await;
    let theirs = Api::<Claim>::namespaced(acting_as(WHO).await, &scratch.name);
    let runner = person::runner_namespace(WHO);
    let upstream = Api::<SandboxClaim>::namespaced(client.clone(), &runner);
    let made = |name: &'static str| {
        let upstream = upstream.clone();
        async move { upstream.get_opt(name).await.ok().flatten().is_some() }
    };
    const FIVE: [&str; 5] = ["quota-1", "quota-2", "quota-3", "quota-4", "quota-5"];

    // The default quota's five claims, each asked of upstream in their runner namespace.
    for name in FIVE {
        theirs
            .create(&PostParams::default(), &claim_by(name, "e2e-person", WHO))
            .await
            .unwrap_or_else(|error| panic!("{name}: {error}"));
    }
    eventually(
        "five upstream claims in their runner namespace",
        Duration::from_secs(120),
        || async {
            for name in FIVE {
                if !made(name).await {
                    return None;
                }
            }
            Some(())
        },
    )
    .await;
    assert_eq!(upstream_claims_in(&client, &scratch.name).await, 0);

    // The namespace is the operator's, for them; they alone are bound there; the quota holds.
    let namespace = Api::<Namespace>::all(client.clone())
        .get(&runner)
        .await
        .expect("their runner namespace");
    assert_eq!(
        namespace
            .labels()
            .get("app.kubernetes.io/managed-by")
            .map(String::as_str),
        Some("purlis-cluster-operator")
    );
    assert_eq!(
        namespace
            .annotations()
            .get("runners.purlis.dev/requester")
            .map(String::as_str),
        Some(WHO)
    );
    let binding = Api::<RoleBinding>::namespaced(client.clone(), &runner)
        .get("purlis-runner-owner")
        .await
        .expect("their binding");
    assert_eq!(binding.role_ref.name, "purlis-runner-owner");
    let bound: Vec<String> = binding
        .subjects
        .unwrap_or_default()
        .into_iter()
        .map(|subject| format!("{}:{}", subject.kind, subject.name))
        .collect();
    assert_eq!(bound, [format!("User:{WHO}")]);
    let quota = Api::<ResourceQuota>::namespaced(client.clone(), &runner)
        .get("purlis-person")
        .await
        .expect("their quota");
    let hard = quota.spec.and_then(|spec| spec.hard).unwrap_or_default();
    assert_eq!(
        hard.get(person::CLAIMS_KEY).map(|q| q.0.as_str()),
        Some("5"),
        "{hard:?}"
    );
    fn pod_right<'a>(verb: &'a str, subresource: Option<&'a str>, namespace: &'a str) -> Asked<'a> {
        Asked {
            group: "",
            verb,
            resource: "pods",
            subresource,
            namespace: Some(namespace),
            name: None,
        }
    }
    // They may reach their runners: see them, exec into them, read their logs.
    for (verb, subresource) in [
        ("get", None),
        ("list", None),
        ("create", Some("exec")),
        ("get", Some("log")),
    ] {
        assert!(
            may_as(&client, WHO, pod_right(verb, subresource, &runner)).await,
            "{verb} {subresource:?}"
        );
    }
    // Nothing more there, nothing in another person's, and no one else in theirs.
    for (verb, subresource) in [
        ("create", None),
        ("delete", None),
        ("create", Some("attach")),
    ] {
        assert!(
            !may_as(&client, WHO, pod_right(verb, subresource, &runner)).await,
            "{verb} {subresource:?}"
        );
    }
    assert!(!may_as(&client, WHO, pod_right("create", Some("exec"), &runners())).await);
    assert!(
        !may_as(
            &client,
            "e2e-someone-else",
            pod_right("create", Some("exec"), &runner)
        )
        .await
    );

    // A sixth is over their quota: it says so, and nothing is asked of upstream for it.
    theirs
        .create(
            &PostParams::default(),
            &claim_by("quota-6", "e2e-person", WHO),
        )
        .await
        .expect("a sixth claim");
    eventually(
        "the sixth claim says it is over the quota",
        Duration::from_secs(60),
        || async {
            let claim = theirs.get("quota-6").await.ok()?;
            (ready_of(&claim)? == ("False".into(), "OverQuota".into())).then_some(())
        },
    )
    .await;
    assert!(!made("quota-6").await);

    // One goes: its finalizer removes its runner first, and the sixth goes on.
    let first = theirs.get("quota-1").await.expect("quota-1");
    assert!(
        first.finalizers().iter().any(|f| f == person::FINALIZER),
        "{first:?}"
    );
    theirs
        .delete("quota-1", &DeleteParams::default())
        .await
        .expect("quota-1 deleted");
    eventually(
        "quota-1 and its upstream claim are gone",
        Duration::from_secs(120),
        || async {
            let gone = theirs.get_opt("quota-1").await.ok()?.is_none();
            (gone && !made("quota-1").await).then_some(())
        },
    )
    .await;
    eventually(
        "the sixth claim is asked of upstream",
        Duration::from_secs(120),
        || async { made("quota-6").await.then_some(()) },
    )
    .await;

    // All go: the pool's template and warm pool with the last of them. Their namespace, its
    // binding and its quota stay, empty, for their next claim.
    for name in ["quota-2", "quota-3", "quota-4", "quota-5", "quota-6"] {
        let _ = theirs.delete(name, &DeleteParams::default()).await;
    }
    let object = name_for("e2e-person");
    eventually(
        "every runner of theirs goes, and the pool's objects with the last",
        Duration::from_secs(180),
        || async {
            let left = upstream.list(&ListParams::default()).await.ok()?.items;
            let template = Api::<SandboxTemplate>::namespaced(client.clone(), &runner)
                .get_opt(&object)
                .await
                .ok()?;
            let warm = Api::<SandboxWarmPool>::namespaced(client.clone(), &runner)
                .get_opt(&object)
                .await
                .ok()?;
            (left.is_empty() && template.is_none() && warm.is_none()).then_some(())
        },
    )
    .await;
    Api::<Namespace>::all(client.clone())
        .get(&runner)
        .await
        .expect("their runner namespace stays");
    Api::<RoleBinding>::namespaced(client.clone(), &runner)
        .get("purlis-runner-owner")
        .await
        .expect("and their binding");

    let _ = pools.delete("e2e-person", &DeleteParams::default()).await;
    scratch.end().await;
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

/// The policy's refusal of `made`, in its own words, or a panic naming `what`.
fn refused_in_words<T: std::fmt::Debug>(what: &str, made: Result<T, kube::Error>, words: &str) {
    match &made {
        Err(kube::Error::Api(status)) => {
            assert_eq!(status.code, 403, "{what}: {status:?}");
            assert!(status.message.contains(words), "{what}: {status:?}");
        }
        other => panic!("{what} was not refused: {other:?}"),
    }
}

#[tokio::test]
#[ignore = "needs a kind cluster: operator/e2e/kind.sh"]
async fn an_upstream_claim_on_a_purlis_pool_made_directly_is_refused() {
    use purlis_cluster_operator::upstream::{
        Reference, SandboxClaim, SandboxClaimSpec, SandboxWarmPool, SandboxWarmPoolSpec, name_for,
    };
    let client = Client::try_default().await.expect("the cluster");
    let scratch = Scratch::new(&client, "e2e-direct").await;
    // Even the cluster admin: a runner from a purlis pool is had only through a Claim.
    let upstream_claims = Api::<SandboxClaim>::namespaced(client.clone(), &scratch.name);
    let warm_pools = Api::<SandboxWarmPool>::namespaced(client.clone(), &scratch.name);
    let claim_on = |name: &str, warm_pool: &str| {
        SandboxClaim::new(
            name,
            SandboxClaimSpec {
                warm_pool_ref: Reference {
                    name: warm_pool.to_owned(),
                },
                volume_claim_templates: Default::default(),
            },
        )
    };
    let warm_pool_of = |name: &str, template: &str| {
        SandboxWarmPool::new(
            name,
            SandboxWarmPoolSpec {
                replicas: 0,
                sandbox_template_ref: Reference {
                    name: template.to_owned(),
                },
            },
        )
    };
    let ours = name_for("e2e-direct");
    refused_in_words(
        "an upstream claim on a purlis warm pool",
        upstream_claims
            .create(&PostParams::default(), &claim_on("direct", &ours))
            .await,
        "are claimed with a Claim",
    );
    refused_in_words(
        "a warm pool of a purlis template",
        warm_pools
            .create(&PostParams::default(), &warm_pool_of("direct", &ours))
            .await,
        "are claimed with a Claim",
    );
    // Upstream's own objects that are not purlis's are left to upstream.
    let theirs_claim = upstream_claims
        .create(&PostParams::default(), &claim_on("theirs", "a-team-pool"))
        .await
        .expect("an upstream claim on another warm pool is admitted");
    let theirs = warm_pools
        .create(
            &PostParams::default(),
            &warm_pool_of("theirs", "a-team-template"),
        )
        .await
        .expect("a warm pool of another template is admitted");
    // Nor can one be moved onto a purlis pool's objects once made.
    let moved = |reference: &str, name: &str| {
        Patch::Merge(serde_json::json!({ "spec": { reference: { "name": name } } }))
    };
    refused_in_words(
        "an upstream claim moved onto a purlis warm pool",
        upstream_claims
            .patch(
                &theirs_claim.name_any(),
                &PatchParams::default(),
                &moved("warmPoolRef", &ours),
            )
            .await,
        "are claimed with a Claim",
    );
    refused_in_words(
        "a warm pool moved onto a purlis template",
        warm_pools
            .patch(
                &theirs.name_any(),
                &PatchParams::default(),
                &moved("sandboxTemplateRef", &ours),
            )
            .await,
        "are claimed with a Claim",
    );

    // And what the operator made for a Claim cannot be moved off a purlis pool's objects.
    let pools = Api::<Pool>::all(client.clone());
    let pool = Pool::new(
        "e2e-direct",
        PoolSpec {
            image: runner_image(),
            kept_volume: Default::default(),
        },
    );
    pools
        .patch(
            "e2e-direct",
            &PatchParams::apply("e2e").force(),
            &Patch::Apply(&pool),
        )
        .await
        .expect("the pool");
    Api::<Claim>::namespaced(client.clone(), &scratch.name)
        .create(&PostParams::default(), &claim("ada-1", "e2e-direct"))
        .await
        .expect("the claim");
    // In its requester's runner namespace (#2055).
    let upstream_claims = Api::<SandboxClaim>::namespaced(client.clone(), &runners());
    let warm_pools = Api::<SandboxWarmPool>::namespaced(client.clone(), &runners());
    eventually(
        "the operator asks upstream for the claim's runner",
        Duration::from_secs(120),
        || async {
            let claim = upstream_claims.get("ada-1").await.ok()?;
            let warm_pool = warm_pools.get(&ours).await.ok()?;
            (claim.spec.warm_pool_ref.name == ours
                && warm_pool.spec.sandbox_template_ref.name == ours)
                .then_some(())
        },
    )
    .await;
    refused_in_words(
        "the operator's upstream claim moved off its warm pool",
        upstream_claims
            .patch(
                "ada-1",
                &PatchParams::default(),
                &moved("warmPoolRef", "a-team-pool"),
            )
            .await,
        "are claimed with a Claim",
    );
    refused_in_words(
        "the operator's warm pool moved off its template",
        warm_pools
            .patch(
                &ours,
                &PatchParams::default(),
                &moved("sandboxTemplateRef", "a-team-template"),
            )
            .await,
        "are claimed with a Claim",
    );
    let _ = pools.delete("e2e-direct", &DeleteParams::default()).await;
    scratch.end().await;
}

/// Whether the claim `name`'s upstream claim is made, in the runner namespace (#2055).
async fn upstream_claim_made(client: &Client, name: &str) -> bool {
    Api::<purlis_cluster_operator::upstream::SandboxClaim>::namespaced(client.clone(), &runners())
        .get_opt(name)
        .await
        .expect("upstream claims")
        .is_some()
}

/// How many upstream claims there are in `namespace`.
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
        assert!(!upstream_claim_made(&client, "ada-1").await);

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
        assert!(!upstream_claim_made(&client, "ada-1").await);

        // A claim made while the binding settles is refused too: wait past it.
        tokio::time::sleep(settled()).await;
        claims
            .create(&PostParams::default(), &claim("ada-2", "e2e-policy"))
            .await
            .expect("a claim made under the policy");
        eventually(
            "the claim made after the binding is acted on",
            Duration::from_secs(60),
            || async { upstream_claim_made(&client, "ada-2").await.then_some(()) },
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
        let pods = Api::<Pod>::namespaced(client.clone(), &runners());
        let running = pods_selected(&client, &runner_selector(&client, "ada-1").await).await;
        assert_eq!(running.len(), 1);
        let runner = running[0].clone();

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

        // A claim made under the new binding, of the same person on the same pool: served from
        // the pool's template in their runner namespace, which the operator made (#2055).
        claims
            .create(&PostParams::default(), &claim("ada-2", "e2e-refused-runs"))
            .await
            .expect("a claim made under the new binding");
        eventually(
            "the new claim is served",
            Duration::from_secs(300),
            || async {
                let claim = claims.get("ada-2").await.ok()?;
                (ready_of(&claim)?.0 == "True").then_some(())
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
        // The new claim's finalizer leaves them: the refused claim on the pool is still there.
        Api::<SandboxTemplate>::namespaced(client.clone(), &runners())
            .get(&template)
            .await
            .expect("the refused claim's runner keeps its template");
        Api::<NetworkPolicy>::namespaced(client.clone(), &runners())
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
    let warm_pools = Api::<SandboxWarmPool>::namespaced(client.clone(), &runners());
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
    let pods = Api::<Pod>::namespaced(client.clone(), &runners());
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
        &runners(),
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
    // desktop's side to show: crates/purlis-core/tests/it/a_chat_runs_on_a_cluster_runner_through_kubectl_exec.rs.)
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
