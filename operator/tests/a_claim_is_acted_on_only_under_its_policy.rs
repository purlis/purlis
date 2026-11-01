// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The operator acts only on Claims admitted under its policy, however it was installed: a
//! Claim made more than `SETTLE` (30 s) after the policy's binding is acted on; one made before
//! it, or within that time, is refused for good; and with no binding in force, no Claim is acted
//! on at all.

use k8s_openapi::api::admissionregistration::v1::{
    ValidatingAdmissionPolicy, ValidatingAdmissionPolicyBinding,
    ValidatingAdmissionPolicyBindingSpec,
};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;
use k8s_openapi::jiff::Timestamp;
use kube::api::ObjectMeta;
use purlis_cluster_operator::{
    Admission, Claim, ClaimSpec, POLICY, SETTLE, admission, in_force_since,
};

fn at(time: &str) -> Time {
    Time(time.parse::<Timestamp>().expect("a timestamp"))
}

fn claim_made(time: Option<&str>) -> Claim {
    Claim {
        metadata: ObjectMeta {
            name: Some("ada-1".into()),
            namespace: Some("ada".into()),
            creation_timestamp: time.map(at),
            ..ObjectMeta::default()
        },
        spec: ClaimSpec {
            pool: "team".into(),
            requester: "ada@example.com".into(),
            kept: false,
            stopped: false,
            forge: None,
        },
        status: None,
    }
}

fn binding(time: &str, policy: &str, actions: &[&str]) -> ValidatingAdmissionPolicyBinding {
    ValidatingAdmissionPolicyBinding {
        metadata: ObjectMeta {
            name: Some(POLICY.into()),
            creation_timestamp: Some(at(time)),
            generation: Some(1),
            ..ObjectMeta::default()
        },
        spec: ValidatingAdmissionPolicyBindingSpec {
            policy_name: policy.into(),
            validation_actions: actions.iter().map(|a| (*a).to_owned()).collect(),
            ..ValidatingAdmissionPolicyBindingSpec::default()
        },
    }
}

fn policy_made(time: &str) -> ValidatingAdmissionPolicy {
    ValidatingAdmissionPolicy {
        metadata: ObjectMeta {
            name: Some(POLICY.into()),
            creation_timestamp: Some(at(time)),
            ..ObjectMeta::default()
        },
        ..ValidatingAdmissionPolicy::default()
    }
}

/// When the policy is in force from under `binding`, the policy made long before it.
fn since(binding: &ValidatingAdmissionPolicyBinding) -> Option<Timestamp> {
    in_force_since(
        Some(&policy_made("2026-01-01T00:00:00Z")),
        Some(binding),
        POLICY,
    )
}

fn in_force(time: &str) -> ValidatingAdmissionPolicyBinding {
    binding(time, POLICY, &["Deny"])
}

#[test]
fn a_claim_made_after_the_binding_has_settled_is_acted_on() {
    let claim = claim_made(Some("2026-10-17T10:00:31Z"));
    let binding = in_force("2026-10-17T10:00:00Z");
    assert_eq!(admission(&claim, since(&binding)), Admission::UnderPolicy);
    assert!(Admission::UnderPolicy.status().is_none());
}

#[test]
fn a_claim_made_before_the_binding_is_refused() {
    let claim = claim_made(Some("2026-10-17T09:59:59Z"));
    let binding = in_force("2026-10-17T10:00:00Z");
    assert_eq!(
        admission(&claim, since(&binding)),
        Admission::MadeBeforePolicy
    );
}

#[test]
fn a_claim_made_while_the_binding_settles_is_refused() {
    let binding = in_force("2026-10-17T10:00:00Z");
    for made in [
        "2026-10-17T10:00:01Z",
        "2026-10-17T10:00:29Z",
        "2026-10-17T10:00:30Z",
    ] {
        assert_eq!(
            admission(&claim_made(Some(made)), since(&binding)),
            Admission::MadeBeforePolicy,
            "{made}"
        );
    }
}

#[test]
fn a_claim_made_in_the_same_second_as_the_binding_is_refused() {
    let claim = claim_made(Some("2026-10-17T10:00:00Z"));
    let binding = in_force("2026-10-17T10:00:00Z");
    assert_eq!(
        admission(&claim, since(&binding)),
        Admission::MadeBeforePolicy
    );
}

#[test]
fn a_claim_with_no_creation_time_is_refused() {
    let binding = in_force("2026-10-17T10:00:00Z");
    assert_eq!(
        admission(&claim_made(None), since(&binding)),
        Admission::MadeBeforePolicy
    );
}

#[test]
fn with_no_binding_no_claim_is_acted_on() {
    let claim = claim_made(Some("2026-10-17T10:00:31Z"));
    assert_eq!(admission(&claim, None), Admission::PolicyNotInForce);
}

#[test]
fn a_binding_that_does_not_deny_with_purlis_policy_is_no_binding() {
    let claim = claim_made(Some("2026-10-17T10:00:31Z"));
    for binding in [
        binding("2026-10-17T10:00:00Z", POLICY, &["Warn"]),
        binding("2026-10-17T10:00:00Z", POLICY, &["Audit"]),
        binding("2026-10-17T10:00:00Z", POLICY, &[]),
        binding("2026-10-17T10:00:00Z", "another-policy", &["Deny"]),
    ] {
        assert_eq!(
            admission(&claim, since(&binding)),
            Admission::PolicyNotInForce,
            "{binding:?}"
        );
    }
}

fn ready(admission: Admission) -> (String, String, String) {
    let status = admission.status().expect("a status");
    let ready = status
        .conditions
        .iter()
        .find(|c| c.type_ == "Ready")
        .expect("Ready");
    (
        ready.status.clone(),
        ready.reason.clone(),
        ready.message.clone(),
    )
}

#[test]
fn a_refused_claim_says_to_delete_it_and_make_it_again() {
    let (status, reason, message) = ready(Admission::MadeBeforePolicy);
    assert_eq!(status, "False");
    assert_eq!(reason, "MadeBeforePolicy");
    assert!(message.contains("Delete it and make it again"), "{message}");
}

#[test]
fn a_claim_waiting_for_the_policy_says_so() {
    let (status, reason, message) = ready(Admission::PolicyNotInForce);
    assert_eq!(status, "False");
    assert_eq!(reason, "PolicyNotInForce");
    assert!(message.contains(POLICY), "{message}");
    assert!(
        message.contains("delete this claim and make it again"),
        "{message}"
    );
}

#[test]
fn a_policy_is_in_force_from_the_later_of_it_and_its_binding() {
    let settled = |time: &str| at(time).0.checked_add(SETTLE).unwrap();
    let binding = in_force("2026-10-17T10:00:00Z");
    let older = policy_made("2026-10-17T09:00:00Z");
    assert_eq!(
        in_force_since(Some(&older), Some(&binding), POLICY),
        Some(settled("2026-10-17T10:00:00Z"))
    );
    // Made again under a binding that stayed: what was made before it was not admitted under it.
    let again = policy_made("2026-10-17T11:00:00Z");
    assert_eq!(
        in_force_since(Some(&again), Some(&binding), POLICY),
        Some(settled("2026-10-17T11:00:00Z"))
    );
    assert_eq!(
        admission(
            &claim_made(Some("2026-10-17T10:30:00Z")),
            in_force_since(Some(&again), Some(&binding), POLICY)
        ),
        Admission::MadeBeforePolicy
    );
}

#[test]
fn a_binding_with_no_policy_holds_nothing() {
    let binding = in_force("2026-10-17T10:00:00Z");
    assert_eq!(in_force_since(None, Some(&binding), POLICY), None);
}

#[test]
fn a_binding_changed_since_it_was_made_is_not_in_force() {
    let policy = policy_made("2026-10-17T09:00:00Z");
    for generation in [Some(2), None] {
        let mut changed = in_force("2026-10-17T10:00:00Z");
        changed.metadata.generation = generation;
        assert_eq!(
            in_force_since(Some(&policy), Some(&changed), POLICY),
            None,
            "{generation:?}"
        );
    }
}
