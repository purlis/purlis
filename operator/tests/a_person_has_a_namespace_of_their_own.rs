// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Each person's runners run in a namespace of their own (#2055, ruled 2026-10-29): made by the
//! operator for one requester, with a role binding that gives that person alone
//! `purlis-runner-owner` and a quota from the chart's `perPerson.quota`. A runner that would go
//! over it is not asked for, and its Claim says `OverQuota`.

use k8s_openapi::api::core::v1::{ResourceQuota, ResourceQuotaStatus};
use k8s_openapi::apimachinery::pkg::api::resource::Quantity;
use purlis_cluster_operator::person::{
    self, CLAIMS_KEY, CPU_KEY, EPHEMERAL_STORAGE_KEY, MEMORY_KEY, MEMORY_LIMIT_KEY, Quota,
    STORAGE_KEY, milli,
};
use purlis_cluster_operator::{Pool, PoolSpec, upstream};
use serde_json::json;

const IMAGE: &str = "ghcr.io/purlis/purlis-runner@sha256:1111111111111111111111111111111111111111111111111111111111111111";

#[test]
fn a_requesters_runner_namespace_is_named_by_the_hash_of_who_they_are() {
    let ada = person::runner_namespace("ada@example.com");
    assert_eq!(ada, "purlis-runners-b5fc85e55755f9e0");
    assert_eq!(
        person::runner_namespace("kubernetes-admin"),
        "purlis-runners-b2f220d99d68735b"
    );
    assert_ne!(person::runner_namespace("bea@example.com"), ada);
    // A DNS-1123 label, whatever the requester holds.
    for requester in [
        "ada@example.com",
        "system:serviceaccount:team:ci",
        "Ada Lovelace",
    ] {
        let name = person::runner_namespace(requester);
        assert!(purlis_cluster_operator::vouch::is_label(&name), "{name}");
        assert_eq!(name.len(), "purlis-runners-".len() + person::HASH_DIGITS);
    }
}

#[test]
fn the_operator_uses_a_runner_namespace_only_when_it_made_it_for_that_person() {
    let mut made = person::namespace_for("ada@example.com");
    assert_eq!(
        serde_json::to_value(&made).unwrap()["metadata"],
        json!({
            "name": "purlis-runners-b5fc85e55755f9e0",
            "labels": {
                "app.kubernetes.io/managed-by": "purlis-cluster-operator",
                "pod-security.kubernetes.io/enforce": "restricted",
            },
            "annotations": { "runners.purlis.dev/requester": "ada@example.com" },
        })
    );
    // Not yet made: no uid for what it makes there to be controlled by.
    assert!(!person::is_theirs(&made, "ada@example.com"));
    made.metadata.uid = Some("runner-uid".into());
    assert!(person::is_theirs(&made, "ada@example.com"));
    // Another person's of the same name, one without the operator's label, or one made by
    // hand: in the way.
    assert!(!person::is_theirs(&made, "bea@example.com"));
    let mut unlabelled = made.clone();
    unlabelled.metadata.labels = None;
    assert!(!person::is_theirs(&unlabelled, "ada@example.com"));
    let mut by_hand = made.clone();
    by_hand.metadata.annotations = None;
    assert!(!person::is_theirs(&by_hand, "ada@example.com"));
    let mut misnamed = made;
    misnamed.metadata.name = Some("ada".into());
    assert!(!person::is_theirs(&misnamed, "ada@example.com"));
}

#[test]
fn the_person_alone_is_bound_to_the_runner_owner_role_as_the_cluster_signs_them_in() {
    let binding = person::owner_binding("purlis-runners-b5fc85e55755f9e0", "ada@example.com");
    assert_eq!(
        serde_json::to_value(&binding).unwrap(),
        json!({
            "apiVersion": "rbac.authorization.k8s.io/v1",
            "kind": "RoleBinding",
            "metadata": {
                "name": "purlis-runner-owner",
                "namespace": "purlis-runners-b5fc85e55755f9e0",
            },
            "roleRef": {
                "apiGroup": "rbac.authorization.k8s.io",
                "kind": "ClusterRole",
                "name": "purlis-runner-owner",
            },
            "subjects": [{
                "apiGroup": "rbac.authorization.k8s.io",
                "kind": "User",
                "name": "ada@example.com",
            }],
        })
    );
    let robot = person::owner_binding("ns", "system:serviceaccount:team:ci");
    assert_eq!(
        serde_json::to_value(&robot).unwrap()["subjects"],
        json!([{ "kind": "ServiceAccount", "name": "ci", "namespace": "team" }])
    );
    // A username that only looks like one is a user.
    for user in [
        "system:serviceaccount:team",
        "system:serviceaccount::ci",
        "system:serviceaccount:a:b:c",
    ] {
        assert_eq!(person::subject_of(user).kind, "User", "{user}");
    }
}

#[test]
fn a_binding_says_what_it_should_only_with_that_role_for_that_person_alone() {
    let wanted = person::owner_binding("ns", "system:serviceaccount:team:ci");
    let mut held = wanted.clone();
    // As the cluster writes a service account's subject back.
    held.subjects.as_mut().unwrap()[0].api_group = Some(String::new());
    assert!(person::binding_says(&held, &wanted));
    let mut more = wanted.clone();
    more.subjects
        .as_mut()
        .unwrap()
        .push(person::subject_of("mallory@example.com"));
    assert!(!person::binding_says(&more, &wanted));
    let mut other = wanted.clone();
    other.role_ref.name = "admin".into();
    assert!(!person::binding_says(&other, &wanted));
}

#[test]
fn the_default_quota_is_five_claims_and_what_five_runners_ask_for() {
    let quota = Quota::from_var(None).unwrap();
    assert_eq!(quota, Quota::default());
    assert_eq!(
        serde_json::to_value(person::quota_for("ns", &quota)).unwrap(),
        json!({
            "apiVersion": "v1",
            "kind": "ResourceQuota",
            "metadata": { "name": "purlis-person", "namespace": "ns" },
            "spec": { "hard": {
                "count/sandboxclaims.extensions.agents.x-k8s.io": "5",
                "requests.cpu": "4",
                "requests.memory": "8Gi",
                "limits.memory": "64Gi",
                "requests.ephemeral-storage": "16Gi",
                "requests.storage": "50Gi",
            } },
        })
    );
    // Five runners of the template, and five kept volumes of a pool's default size, fit.
    let pod = upstream::template_for(&pool(), "ns", Vec::new())
        .spec
        .pod_template
        .spec
        .unwrap();
    let asks = person::asks_of(&pod, Some("10Gi"));
    for (key, asked) in &asks {
        let allowed = &quota.hard()[*key];
        assert!(
            5 * milli(asked).unwrap() <= milli(&allowed.0).unwrap(),
            "{key}: five of {asked} within {}",
            allowed.0
        );
    }
}

#[test]
fn the_charts_quota_is_read_with_each_key_it_leaves_out_unlimited() {
    let quota = Quota::from_var(Some(r#"{"claims":2,"cpu":1.5,"memory":"2Gi"}"#)).unwrap();
    assert_eq!(
        quota,
        Quota {
            claims: Some(2),
            cpu: Some("1.5".into()),
            memory: Some("2Gi".into()),
            memory_limit: None,
            ephemeral_storage: None,
            storage: None,
        }
    );
    assert_eq!(
        quota.hard().keys().collect::<Vec<_>>(),
        [CLAIMS_KEY, CPU_KEY, MEMORY_KEY]
    );
    // Anything else stops the operator rather than run it without the quota asked for.
    for bad in [
        "",
        "null",
        r#"{"disks":2}"#,
        r#"{"claims":-1}"#,
        r#"{"cpu":"lots"}"#,
        r#"{"memory":true}"#,
    ] {
        assert!(Quota::from_var(Some(bad)).is_err(), "{bad}");
    }
}

#[test]
fn a_quantity_is_read_as_kubernetes_reads_it() {
    for (quantity, thousandths) in [
        ("250m", 250),
        ("4", 4000),
        ("0.5", 500),
        (".5", 500),
        ("1.5", 1500),
        ("1k", 1_000_000),
        ("8Gi", 8 * 1000 * (1 << 30)),
        ("512Mi", 512 * 1000 * (1 << 20)),
        ("1G", 1_000_000_000_000),
    ] {
        assert_eq!(milli(quantity), Some(thousandths), "{quantity}");
    }
    for unread in ["", "lots", "1e3", "1.2.3", "-1", "4 Gi", "."] {
        assert_eq!(milli(unread), None, "{unread}");
    }
}

#[test]
fn a_quota_the_cluster_writes_in_other_words_says_the_same() {
    let wanted = person::quota_for(
        "ns",
        &Quota {
            cpu: Some("0.5".into()),
            ..Quota::default()
        },
    );
    let mut held = wanted.clone();
    held.spec
        .as_mut()
        .unwrap()
        .hard
        .as_mut()
        .unwrap()
        .insert(CPU_KEY.into(), Quantity("500m".into()));
    assert!(person::quota_says(&held, &wanted));
    held.spec
        .as_mut()
        .unwrap()
        .hard
        .as_mut()
        .unwrap()
        .insert(CPU_KEY.into(), Quantity("8".into()));
    assert!(!person::quota_says(&held, &wanted));
}

fn pool() -> Pool {
    Pool::new("team", PoolSpec::new(IMAGE))
}

/// A quota as the cluster holds it, counting `used` against `hard`.
fn counted(hard: &[(&str, &str)], used: &[(&str, &str)]) -> ResourceQuota {
    let map = |pairs: &[(&str, &str)]| {
        Some(
            pairs
                .iter()
                .map(|(key, value)| ((*key).to_owned(), Quantity((*value).to_owned())))
                .collect(),
        )
    };
    ResourceQuota {
        status: Some(ResourceQuotaStatus {
            hard: map(hard),
            used: map(used),
        }),
        ..ResourceQuota::default()
    }
}

#[test]
fn one_more_runner_asks_for_a_claim_its_pods_requests_and_a_kept_ones_volume() {
    let pod = upstream::template_for(&pool(), "ns", Vec::new())
        .spec
        .pod_template
        .spec
        .unwrap();
    // The larger of the containers' requests together and the first step's.
    assert_eq!(
        person::asks_of(&pod, None),
        [
            (CLAIMS_KEY, "1".to_owned()),
            (CPU_KEY, "250m".to_owned()),
            (MEMORY_KEY, format!("{}m", 512u128 * 1000 * (1 << 20))),
            (EPHEMERAL_STORAGE_KEY, format!("{}m", 1000u128 * (1 << 30))),
            (MEMORY_LIMIT_KEY, format!("{}m", 4000u128 * (1 << 30))),
        ]
    );
    assert_eq!(
        person::asks_of(&pod, Some("10Gi")).last(),
        Some(&(STORAGE_KEY, "10Gi".to_owned()))
    );
}

#[test]
fn a_runner_over_any_of_its_persons_quota_is_not_asked_for_and_says_what_is_over() {
    let asks = [(CLAIMS_KEY, "1".to_owned()), (CPU_KEY, "250m".to_owned())];
    let hard = [(CLAIMS_KEY, "5"), (CPU_KEY, "4")];
    // Room for one more.
    assert_eq!(
        person::over_quota(&counted(&hard, &[(CLAIMS_KEY, "4"), (CPU_KEY, "1")]), &asks),
        None
    );
    // Nothing counted yet, or nothing in use: room.
    assert_eq!(person::over_quota(&ResourceQuota::default(), &asks), None);
    assert_eq!(person::over_quota(&counted(&hard, &[]), &asks), None);
    // Five claims already.
    let over = person::over_quota(&counted(&hard, &[(CLAIMS_KEY, "5"), (CPU_KEY, "1")]), &asks)
        .expect("over");
    assert!(over.contains(CLAIMS_KEY), "{over}");
    assert!(!over.contains(CPU_KEY), "{over}");
    // The CPU, in the cluster's own words.
    let over = person::over_quota(
        &counted(&hard, &[(CLAIMS_KEY, "1"), (CPU_KEY, "3900m")]),
        &asks,
    )
    .expect("over");
    assert!(over.contains("requests.cpu: 3900m in use of 4"), "{over}");
    // A key the quota does not limit is never over.
    assert_eq!(
        person::over_quota(
            &counted(&[(CLAIMS_KEY, "5")], &[(CLAIMS_KEY, "1"), (CPU_KEY, "99")]),
            &asks
        ),
        None
    );
}

#[test]
fn a_claim_over_its_quota_says_so_and_what_frees_it() {
    let status = person::status_over_quota("purlis-runners-b5fc85e55755f9e0", "claims: 5 of 5");
    let ready = &status.conditions[0];
    assert_eq!(
        (
            ready.type_.as_str(),
            ready.status.as_str(),
            ready.reason.as_str()
        ),
        ("Ready", "False", "OverQuota")
    );
    for words in [
        "purlis-person in purlis-runners-b5fc85e55755f9e0",
        "claims: 5 of 5",
        "perPerson.quota",
    ] {
        assert!(ready.message.contains(words), "{}", ready.message);
    }
    assert_eq!(status.link_key, None);
}

#[test]
fn a_claim_whose_runner_namespace_is_someone_elses_says_so() {
    let status = person::status_of_a_namespace_in_the_way("purlis-runners-b5fc85e55755f9e0", false);
    let ready = &status.conditions[0];
    assert_eq!(ready.reason, "NamespaceInTheWay");
    assert!(
        ready.message.contains("purlis-runners-b5fc85e55755f9e0"),
        "{}",
        ready.message
    );
}

#[test]
fn only_a_refusal_for_a_quota_reads_as_over_quota() {
    let refused = |code: u16, message: &str| {
        kube::Error::Api(
            kube::core::Status::failure(message, "Forbidden")
                .with_code(code)
                .boxed(),
        )
    };
    assert!(person::is_over_quota(&refused(
        403,
        "sandboxclaims.extensions.agents.x-k8s.io \"ada-6\" is forbidden: exceeded quota: purlis-person, requested: count/sandboxclaims.extensions.agents.x-k8s.io=1, used: count/sandboxclaims.extensions.agents.x-k8s.io=5, limited: count/sandboxclaims.extensions.agents.x-k8s.io=5"
    )));
    assert!(!person::is_over_quota(&refused(
        403,
        "forbidden: the operator may not"
    )));
    assert!(!person::is_over_quota(&refused(409, "exceeded quota")));
}

fn at(time: &str) -> k8s_openapi::apimachinery::pkg::apis::meta::v1::Time {
    k8s_openapi::apimachinery::pkg::apis::meta::v1::Time(time.parse().expect("a timestamp"))
}

/// Ada's runner namespace as the operator made it, at `made`.
fn made_at(made: &str) -> k8s_openapi::api::core::v1::Namespace {
    let mut runner = person::namespace_for("ada@example.com");
    runner.metadata.uid = Some("runner-uid".into());
    runner.metadata.creation_timestamp = Some(at(made));
    runner
}

#[test]
fn a_runner_namespace_is_acted_in_only_when_made_for_its_person_under_the_policy() {
    use person::RunnerNamespace;
    let since = Some(at("2026-10-29T10:00:30Z").0);
    let now = at("2026-10-29T12:00:00Z").0;
    let ada = "ada@example.com";
    let made = made_at("2026-10-29T11:00:00Z");
    assert_eq!(
        person::runner_namespace_of(Some(made.clone()), ada, since, now),
        RunnerNamespace::Theirs(Box::new(made.clone()))
    );
    assert_eq!(
        person::runner_namespace_of(None, ada, since, now),
        RunnerNamespace::Missing
    );
    // Made before the policy was in force, or within its settling time: whoever made it.
    for before in ["2026-10-29T09:00:00Z", "2026-10-29T10:00:30Z"] {
        assert_eq!(
            person::runner_namespace_of(Some(made_at(before)), ada, since, now),
            RunnerNamespace::InTheWay {
                made_before_policy: true
            },
            "{before}"
        );
    }
    // Someone else's, or one not marked as the operator's.
    assert_eq!(
        person::runner_namespace_of(Some(made.clone()), "bea@example.com", since, now),
        RunnerNamespace::InTheWay {
            made_before_policy: false
        }
    );
    // No binding in force: nothing is made or used, not even their own.
    assert_eq!(
        person::runner_namespace_of(Some(made), ada, None, now),
        RunnerNamespace::PolicyNotInForce
    );
    assert_eq!(
        person::runner_namespace_of(None, ada, None, now),
        RunnerNamespace::PolicyNotInForce
    );
}

#[test]
fn a_deleted_claim_cleans_only_in_a_namespace_marked_for_its_person_and_goes_either_way() {
    let made = made_at("2026-10-29T11:00:00Z");
    assert_eq!(
        person::runner_to_clean(Some(made.clone()), "ada@example.com"),
        Some(made.clone())
    );
    // Someone else's namespace under the name, or none: nothing deleted, and the Claim goes.
    assert_eq!(person::runner_to_clean(Some(made), "bea@example.com"), None);
    let mut unmarked = made_at("2026-10-29T11:00:00Z");
    unmarked.metadata.annotations = None;
    assert_eq!(
        person::runner_to_clean(Some(unmarked), "ada@example.com"),
        None
    );
    assert_eq!(person::runner_to_clean(None, "ada@example.com"), None);
}

#[test]
fn a_key_left_out_of_the_charts_quota_is_a_difference_to_apply() {
    let wanted = person::quota_for(
        "ns",
        &Quota {
            storage: None,
            ..Quota::default()
        },
    );
    // What the cluster holds from before, with storage still limited.
    let held = person::quota_for("ns", &Quota::default());
    assert!(!person::quota_says(&held, &wanted));
}

#[test]
fn a_quota_that_has_not_counted_yet_is_not_an_over_quota() {
    let refused = kube::Error::Api(
        kube::core::Status::failure(
            "sandboxclaims.extensions.agents.x-k8s.io \"ada-1\" is forbidden: status unknown for quota: purlis-person, resources: count/sandboxclaims.extensions.agents.x-k8s.io",
            "Forbidden",
        )
        .with_code(403)
        .boxed(),
    );
    assert!(person::is_quota_not_counted_yet(&refused));
    assert!(!person::is_over_quota(&refused));
}

#[test]
fn a_namespace_in_the_way_says_why() {
    let made_before = person::status_of_a_namespace_in_the_way("purlis-runners-x", true);
    assert!(
        made_before.conditions[0]
            .message
            .contains("before the policy on runner namespaces"),
        "{made_before:?}"
    );
    let not_ours = person::status_of_runner_namespaces_not_in_force();
    assert_eq!(not_ours.conditions[0].reason, "PolicyNotInForce");
    assert!(
        not_ours.conditions[0]
            .message
            .contains(purlis_cluster_operator::RUNNER_NAMESPACES_POLICY)
    );
}

#[test]
fn no_runner_namespace_is_made_while_the_policys_binding_settles() {
    use person::RunnerNamespace;
    let since = at("2026-10-29T10:00:30Z").0;
    let until = at("2026-10-29T10:00:35Z").0;
    // As on an upgrade, the operator meets a Claim within seconds of the binding being made: it
    // waits, and makes nothing, until the binding is in force and a little after.
    for now in [
        "2026-10-29T10:00:01Z",
        "2026-10-29T10:00:30Z",
        "2026-10-29T10:00:34Z",
    ] {
        assert_eq!(
            person::runner_namespace_of(None, "ada@example.com", Some(since), at(now).0),
            RunnerNamespace::Settling { until },
            "{now}"
        );
    }
    assert_eq!(
        person::runner_namespace_of(None, "ada@example.com", Some(since), until),
        RunnerNamespace::Missing
    );
    // One made since, read back, is theirs.
    let made = made_at("2026-10-29T10:00:36Z");
    assert_eq!(
        person::runner_namespace_of(Some(made.clone()), "ada@example.com", Some(since), until),
        RunnerNamespace::Theirs(Box::new(made))
    );
}
