// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What the operator asks of the pinned upstream agent-sandbox for a person's Claim on a team's
//! Pool, and what it tells the person back on the Claim. Read through the objects it would apply,
//! as the cluster would receive them.

use k8s_openapi::apimachinery::pkg::apis::meta::v1::{Condition, Time};
use kube::api::ObjectMeta;
use purlis_cluster_operator::{Claim, ClaimSpec, ClaimStatus, Pool, PoolSpec, upstream};
use serde_json::{Value, json};

const IMAGE: &str = "ghcr.io/purlis/purlis-runner@sha256:1111111111111111111111111111111111111111111111111111111111111111";

fn pool() -> Pool {
    let mut pool = Pool::new(
        "team",
        PoolSpec {
            image: IMAGE.into(),
            kept_volume: Default::default(),
        },
    );
    pool.metadata.uid = Some("pool-uid".into());
    pool
}

fn claim() -> Claim {
    Claim {
        metadata: ObjectMeta {
            name: Some("ada-1".into()),
            namespace: Some("ada".into()),
            uid: Some("claim-uid".into()),
            ..ObjectMeta::default()
        },
        spec: ClaimSpec {
            pool: "team".into(),
            requester: "ada@example.com".into(),
            kept: false,
        },
        status: None,
    }
}

fn json_of(value: &impl serde::Serialize) -> Value {
    serde_json::to_value(value).expect("serialises")
}

#[test]
fn a_claims_runner_runs_the_pools_image_and_its_own_command_as_a_user_who_is_not_root() {
    let template = json_of(&upstream::template_for(
        &pool(),
        "ada",
        upstream::owners_with(None, &claim(), &[&claim()]),
    ));
    let pod = &template["spec"]["podTemplate"]["spec"];
    let containers = pod["containers"].as_array().expect("containers");
    assert_eq!(containers.len(), 1, "{pod}");
    let runner = &containers[0];
    assert_eq!(runner["image"], IMAGE);
    // The image's own entrypoint and command: `tini -- purlis serve` (docs/runner-image.md).
    assert!(runner.get("command").is_none(), "{runner}");
    assert!(runner.get("args").is_none(), "{runner}");
    let security = &runner["securityContext"];
    assert_eq!(security["runAsNonRoot"], true);
    assert_eq!(security["allowPrivilegeEscalation"], false);
    assert_eq!(security["capabilities"]["drop"], json!(["ALL"]));
    assert_eq!(security["seccompProfile"]["type"], "RuntimeDefault");
}

#[test]
fn a_runner_gets_no_kubernetes_credential_and_no_service_addresses() {
    let template = json_of(&upstream::template_for(
        &pool(),
        "ada",
        upstream::owners_with(None, &claim(), &[&claim()]),
    ));
    let pod = &template["spec"]["podTemplate"]["spec"];
    assert_eq!(pod["automountServiceAccountToken"], false, "{pod}");
    assert_eq!(pod["enableServiceLinks"], false, "{pod}");
}

fn another_claim() -> Claim {
    let mut claim = claim();
    claim.metadata.name = Some("ada-2".into());
    claim.metadata.uid = Some("another-claim-uid".into());
    claim
}

#[test]
fn a_pools_template_and_warm_pool_live_in_the_claims_namespace_and_belong_to_its_claims_there() {
    // The second claim joins the first, which owns the object as the cluster holds it.
    let first = upstream::template_for(
        &pool(),
        "ada",
        upstream::owners_with(None, &claim(), &[&claim()]),
    );
    let owners = upstream::owners_with(
        Some(&first.metadata),
        &another_claim(),
        &[&claim(), &another_claim()],
    );
    for object in [
        json_of(&upstream::template_for(&pool(), "ada", owners.clone())),
        json_of(&upstream::warm_pool_for(&pool(), "ada", owners.clone())),
    ] {
        assert_eq!(object["metadata"]["namespace"], "ada", "{object}");
        assert_eq!(object["metadata"]["name"], "purlis-pool-team", "{object}");
        let owners = object["metadata"]["ownerReferences"]
            .as_array()
            .expect("owners");
        // Each claim on the pool in the namespace, and none of them its controller: the garbage
        // collector removes the object once every one of them is gone, and not before, whatever
        // becomes of the pool.
        assert_eq!(owners.len(), 2, "{object}");
        for (owner, (name, uid)) in owners
            .iter()
            .zip([("ada-1", "claim-uid"), ("ada-2", "another-claim-uid")])
        {
            assert_eq!(owner["kind"], "Claim");
            assert_eq!(owner["apiVersion"], "runners.purlis.dev/v1alpha1");
            assert_eq!(owner["name"], name);
            assert_eq!(owner["uid"], uid);
            assert!(owner.get("controller").is_none(), "{object}");
        }
    }
}

#[test]
fn a_claim_joins_the_claims_that_own_its_pools_objects_once_and_only_purlis_claims_stay() {
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::OwnerReference;
    let reference = |api_version: &str, kind: &str, name: &str, uid: &str| OwnerReference {
        api_version: api_version.into(),
        kind: kind.into(),
        name: name.into(),
        uid: uid.into(),
        controller: Some(true),
        block_owner_deletion: Some(true),
    };
    let held = ObjectMeta {
        owner_references: Some(vec![
            reference(
                "runners.purlis.dev/v1alpha1",
                "Claim",
                "ada-2",
                "another-claim-uid",
            ),
            // The claim itself already: it is named once.
            reference("runners.purlis.dev/v1alpha1", "Claim", "ada-1", "claim-uid"),
            // The pool, as the operator made them before its claims owned them, and anything
            // else: neither stays.
            reference("runners.purlis.dev/v1alpha1", "Pool", "team", "pool-uid"),
            reference("example.com/v1", "Claim", "ada-3", "foreign-uid"),
        ]),
        ..ObjectMeta::default()
    };
    let owners = upstream::owners_with(Some(&held), &claim(), &[&claim(), &another_claim()]);
    let named: Vec<(&str, &str)> = owners
        .iter()
        .map(|owner| (owner.name.as_str(), owner.uid.as_str()))
        .collect();
    assert_eq!(
        named,
        [("ada-1", "claim-uid"), ("ada-2", "another-claim-uid")]
    );
    for owner in &owners {
        assert_eq!(owner.controller, None, "{owner:?}");
        assert_eq!(owner.block_owner_deletion, None, "{owner:?}");
    }
}

#[test]
fn a_pool_keeps_no_runner_warm() {
    let warm = json_of(&upstream::warm_pool_for(
        &pool(),
        "ada",
        upstream::owners_with(None, &claim(), &[&claim()]),
    ));
    assert_eq!(warm["spec"]["replicas"], 0, "{warm}");
    assert_eq!(
        warm["spec"]["sandboxTemplateRef"]["name"],
        "purlis-pool-team"
    );
}

#[test]
fn a_claim_becomes_an_upstream_claim_on_its_pools_warm_pool_that_goes_with_it() {
    let upstream = json_of(&upstream::sandbox_claim_for(&claim(), &Default::default()));
    assert_eq!(upstream["apiVersion"], "extensions.agents.x-k8s.io/v1beta1");
    assert_eq!(upstream["kind"], "SandboxClaim");
    assert_eq!(upstream["metadata"]["name"], "ada-1");
    assert_eq!(upstream["metadata"]["namespace"], "ada");
    assert_eq!(upstream["spec"]["warmPoolRef"]["name"], "purlis-pool-team");
    let owners = upstream["metadata"]["ownerReferences"]
        .as_array()
        .expect("owners");
    assert_eq!(owners.len(), 1, "{upstream}");
    assert_eq!(owners[0]["kind"], "Claim");
    assert_eq!(owners[0]["uid"], "claim-uid");
    assert_eq!(owners[0]["controller"], true);
}

fn ready(status: &str, reason: &str, message: &str) -> Condition {
    Condition {
        type_: "Ready".into(),
        status: status.into(),
        reason: reason.into(),
        message: message.into(),
        last_transition_time: Time(k8s_openapi::jiff::Timestamp::UNIX_EPOCH),
        observed_generation: None,
    }
}

#[test]
fn a_claim_is_ready_when_its_upstream_claim_is_and_says_why_not_otherwise() {
    let mut given = upstream::sandbox_claim_for(&claim(), &Default::default());
    given.status = Some(upstream::SandboxClaimStatus {
        conditions: vec![ready("True", "SandboxReady", "the sandbox is ready")],
        sandbox: None,
    });
    let status = purlis_cluster_operator::status_of(Some(&given));
    let condition = &status.conditions[0];
    assert_eq!(
        (condition.type_.as_str(), condition.status.as_str()),
        ("Ready", "True")
    );

    given.status = Some(upstream::SandboxClaimStatus {
        conditions: vec![ready("False", "SandboxNotReady", "pulling the image")],
        sandbox: None,
    });
    let status = purlis_cluster_operator::status_of(Some(&given));
    let condition = &status.conditions[0];
    assert_eq!(condition.status, "False");
    assert_eq!(condition.reason, "SandboxNotReady");
    assert_eq!(condition.message, "pulling the image");
}

#[test]
fn a_claim_whose_runner_has_not_been_asked_for_yet_is_starting() {
    let given = upstream::sandbox_claim_for(&claim(), &Default::default());
    let status = purlis_cluster_operator::status_of(Some(&given));
    let condition = &status.conditions[0];
    assert_eq!(condition.status, "False");
    assert_eq!(condition.reason, "Starting");
}

#[test]
fn a_claim_on_a_pool_that_does_not_exist_says_so() {
    let status = purlis_cluster_operator::status_of_a_missing_pool(&claim());
    let condition = &status.conditions[0];
    assert_eq!(condition.type_, "Ready");
    assert_eq!(condition.status, "False");
    assert_eq!(condition.reason, "PoolNotFound");
    assert_eq!(condition.message, "There is no pool named team.");
}

#[test]
fn a_claims_ready_keeps_the_time_it_last_changed_until_it_changes_again() {
    let mut before = purlis_cluster_operator::status_of(None);
    before.conditions[0].last_transition_time = Time(k8s_openapi::jiff::Timestamp::UNIX_EPOCH);

    let again =
        purlis_cluster_operator::settled(Some(&before), purlis_cluster_operator::status_of(None));
    assert_eq!(again, before, "the same word is not written twice");

    let mut given = upstream::sandbox_claim_for(&claim(), &Default::default());
    given.status = Some(upstream::SandboxClaimStatus {
        conditions: vec![ready("True", "SandboxReady", "ready")],
        sandbox: None,
    });
    let changed = purlis_cluster_operator::settled(
        Some(&before),
        purlis_cluster_operator::status_of(Some(&given)),
    );
    assert_ne!(
        changed.conditions[0].last_transition_time,
        before.conditions[0].last_transition_time
    );
}

#[test]
fn an_owner_reference_never_blocks_its_owners_deletion() {
    // Blocking would need update rights on the owner's finalizers wherever the cluster enforces
    // owner-reference permissions; the garbage collector removes the dependents either way.
    for object in [
        json_of(&upstream::template_for(
            &pool(),
            "ada",
            upstream::owners_with(None, &claim(), &[&claim()]),
        )),
        json_of(&upstream::warm_pool_for(
            &pool(),
            "ada",
            upstream::owners_with(None, &claim(), &[&claim()]),
        )),
        json_of(&upstream::sandbox_claim_for(&claim(), &Default::default())),
    ] {
        let owner = &object["metadata"]["ownerReferences"][0];
        assert_eq!(owner["kind"], "Claim", "{object}");
        assert!(owner.get("blockOwnerDeletion").is_none(), "{object}");
    }
}

#[test]
fn a_runners_home_is_on_the_volume_its_pod_mounts_and_its_group_may_write() {
    let mut pool = pool();
    pool.spec.kept_volume.size = "20Gi".into();
    let template = json_of(&upstream::template_for(&pool, "ada", Vec::new()));
    let pod = &template["spec"]["podTemplate"]["spec"];
    let runner = &pod["containers"][0];
    assert_eq!(
        runner["env"],
        json!([{ "name": "HOME", "value": upstream::RUNNER_HOME }])
    );
    assert!(upstream::RUNNER_HOME.starts_with(&format!("{}/", upstream::KEPT_FOLDER)));
    let home = pod["volumes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|volume| volume["name"] == upstream::KEPT_VOLUME)
        .expect("the home's volume");
    // A fresh claim's: a folder of the node's, as big as a kept claim's volume.
    assert_eq!(home["emptyDir"], json!({ "sizeLimit": "20Gi" }));
    assert_eq!(
        pod["securityContext"],
        json!({ "fsGroup": upstream::VOLUME_GROUP, "fsGroupChangePolicy": "OnRootMismatch" })
    );
}

#[test]
fn a_pool_without_a_kept_volume_gives_each_kept_claim_ten_gibibytes() {
    let pool: Pool = serde_json::from_value(json!({
        "apiVersion": "runners.purlis.dev/v1alpha1",
        "kind": "Pool",
        "metadata": { "name": "team" },
        "spec": { "image": IMAGE },
    }))
    .unwrap();
    assert_eq!(
        pool.spec.kept_volume,
        purlis_cluster_operator::KeptVolume::default()
    );
    assert_eq!(pool.spec.kept_volume.size, "10Gi");
    assert_eq!(pool.spec.kept_volume.storage_class_name, None);
    let claim: Claim = serde_json::from_value(json!({
        "apiVersion": "runners.purlis.dev/v1alpha1",
        "kind": "Claim",
        "metadata": { "name": "ada-1", "namespace": "ada" },
        "spec": { "pool": "team", "requester": "ada@example.com" },
    }))
    .unwrap();
    assert!(
        !claim.spec.kept,
        "a claim is fresh unless it says it is kept"
    );
}

#[test]
fn a_warm_pool_keeps_as_many_runners_warm_as_it_holds() {
    assert_eq!(upstream::replicas_held(None), 0);
    let mut held = upstream::warm_pool_for(&pool(), "ada", Vec::new());
    held.spec.replicas = 2;
    assert_eq!(upstream::replicas_held(Some(&held)), 2);
}

mod stages {
    use super::*;
    use k8s_openapi::api::core::v1::{
        ContainerState, ContainerStateRunning, ContainerStateTerminated, ContainerStateWaiting,
        ContainerStatus, Pod, PodCondition, PodStatus,
    };
    use purlis_cluster_operator::stage;

    fn waiting(name: &str, reason: &str, image_id: &str) -> ContainerStatus {
        ContainerStatus {
            name: name.into(),
            image_id: image_id.into(),
            state: Some(ContainerState {
                waiting: Some(ContainerStateWaiting {
                    reason: Some(reason.into()),
                    message: Some(format!("{reason} for {name}")),
                }),
                ..ContainerState::default()
            }),
            ..ContainerStatus::default()
        }
    }

    fn pod(scheduled: Option<&str>, init: Vec<ContainerStatus>, main: Vec<ContainerStatus>) -> Pod {
        Pod {
            status: Some(PodStatus {
                conditions: scheduled.map(|status| {
                    vec![PodCondition {
                        type_: "PodScheduled".into(),
                        status: status.into(),
                        message: Some("0/1 nodes are available: 1 Insufficient memory.".into()),
                        ..PodCondition::default()
                    }]
                }),
                init_container_statuses: Some(init),
                container_statuses: Some(main),
                ..PodStatus::default()
            }),
            ..Pod::default()
        }
    }

    #[test]
    fn a_cold_runner_says_it_waits_for_a_node_then_its_image_then_itself() {
        assert_eq!(stage::stage_of(None).0, stage::SCHEDULING);
        let unscheduled = pod(Some("False"), Vec::new(), Vec::new());
        let (reason, message) = stage::stage_of(Some(&unscheduled));
        assert_eq!(reason, stage::SCHEDULING);
        assert!(message.contains("Insufficient memory"), "{message}");
        let pulling = pod(
            Some("True"),
            vec![waiting(upstream::KEY_STEP, "PodInitializing", "")],
            vec![waiting(upstream::RUNNER_CONTAINER, "PodInitializing", "")],
        );
        assert_eq!(stage::stage_of(Some(&pulling)).0, stage::PULLING);
        let failing = pod(
            Some("True"),
            vec![waiting(upstream::KEY_STEP, "ImagePullBackOff", "")],
            Vec::new(),
        );
        let (reason, message) = stage::stage_of(Some(&failing));
        assert_eq!(reason, stage::PULLING);
        assert!(message.contains("ImagePullBackOff"), "{message}");
        let mut started = waiting(upstream::KEY_STEP, "", "sha256:abc");
        started.state = Some(ContainerState {
            terminated: Some(ContainerStateTerminated::default()),
            ..ContainerState::default()
        });
        let mut runner = waiting(upstream::RUNNER_CONTAINER, "", "sha256:abc");
        runner.state = Some(ContainerState {
            running: Some(ContainerStateRunning::default()),
            ..ContainerState::default()
        });
        let starting = pod(Some("True"), vec![started], vec![runner]);
        assert_eq!(stage::stage_of(Some(&starting)).0, stage::STARTING);
        // Waiting for its volume to attach is not pulling (#2057).
        let mut attaching = pod(
            Some("True"),
            vec![waiting(upstream::KEY_STEP, "PodInitializing", "")],
            Vec::new(),
        );
        let ready_to_start = |status: &str| PodCondition {
            type_: "PodReadyToStartContainers".into(),
            status: status.into(),
            ..PodCondition::default()
        };
        let conditions = attaching
            .status
            .as_mut()
            .unwrap()
            .conditions
            .as_mut()
            .unwrap();
        conditions.push(ready_to_start("False"));
        let (reason, message) = stage::stage_of(Some(&attaching));
        assert_eq!(reason, stage::STARTING);
        assert!(message.contains("volume"), "{message}");
        let conditions = attaching
            .status
            .as_mut()
            .unwrap()
            .conditions
            .as_mut()
            .unwrap();
        conditions.pop();
        conditions.push(ready_to_start("True"));
        assert_eq!(stage::stage_of(Some(&attaching)).0, stage::PULLING);
    }

    #[test]
    fn only_a_runner_on_its_way_is_told_as_a_stage() {
        let with = |status: &str, reason: &str| ClaimStatus {
            conditions: vec![ready(status, reason, "upstream's words")],
            link_key: None,
        };
        let staged = stage::staged(with("False", "SandboxNotReady"), None);
        assert_eq!(staged.conditions[0].reason, stage::SCHEDULING);
        for (status, reason) in [
            ("False", "VolumeClaimTemplatesError"),
            ("False", "SandboxExpired"),
            ("True", "DependenciesReady"),
        ] {
            let staged = stage::staged(with(status, reason), None);
            assert_eq!(staged.conditions[0].reason, reason);
            assert_eq!(staged.conditions[0].message, "upstream's words");
        }
    }
}

#[test]
fn a_kept_claims_upstream_claim_keeps_the_volume_it_was_made_with_when_its_pool_changes() {
    let mut kept = claim();
    kept.spec.kept = true;
    let made = upstream::sandbox_claim_for(&kept, &purlis_cluster_operator::KeptVolume::default());
    let changed = purlis_cluster_operator::KeptVolume {
        size: "50Gi".into(),
        storage_class_name: Some("fast".into()),
    };
    let mut wanted = upstream::sandbox_claim_for(&kept, &changed);
    assert_ne!(wanted.spec, made.spec);
    upstream::keep_volumes_held(&mut wanted, Some(&made));
    assert_eq!(
        wanted.spec.volume_claim_templates, made.spec.volume_claim_templates,
        "upstream refuses a change to them"
    );
    // A claim not made yet takes its pool's as it is now.
    let mut new = upstream::sandbox_claim_for(&kept, &changed);
    upstream::keep_volumes_held(&mut new, None);
    assert_eq!(
        json_of(&new)["spec"]["volumeClaimTemplates"][0]["spec"]["resources"]["requests"]["storage"],
        "50Gi"
    );
}

#[test]
fn a_statement_goes_when_what_its_claim_asks_for_cannot_be_applied() {
    use purlis_cluster_operator::{LinkKey, LinkKeyIs, status_patch, status_withdrawn};
    let mut vouched = purlis_cluster_operator::status_of(None);
    vouched.conditions.push(Condition {
        type_: "KeyPublished".into(),
        ..ready("True", "Vouched", "vouched")
    });
    vouched.link_key = Some(LinkKey {
        statement: "a statement".into(),
        signature: "a signature".into(),
    });
    let withdrawn = status_withdrawn(Some(&vouched), "upstream refused it");
    assert_eq!(withdrawn.link_key, None);
    let key = withdrawn
        .conditions
        .iter()
        .find(|c| c.type_ == "KeyPublished")
        .expect("KeyPublished");
    assert_eq!(
        (key.status.as_str(), key.reason.as_str()),
        ("False", "NotVouched")
    );
    assert!(
        key.message.contains("upstream refused it"),
        "{}",
        key.message
    );
    assert!(withdrawn.conditions.iter().any(|c| c.type_ == "Ready"));
    assert_eq!(withdrawn.conditions.len(), 2);
    let patch = status_patch(Some(&vouched), withdrawn, LinkKeyIs::Said).expect("a change");
    assert_eq!(patch["status"]["linkKey"], Value::Null);
}
