// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The operator acts only on Claims admitted under its policy, however it was installed: a
//! Claim made more than `SETTLE` (30 s) after the policy's binding is acted on; one made before
//! it, or within that time, is refused for good; and with no binding in force, or a policy that is
//! not the one the operator ships, no Claim is acted on at all.

use k8s_openapi::api::admissionregistration::v1::{
    MatchCondition, MatchResources, NamedRuleWithOperations, ParamRef, ValidatingAdmissionPolicy,
    ValidatingAdmissionPolicyBinding, ValidatingAdmissionPolicyBindingSpec,
    ValidatingAdmissionPolicySpec,
};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{LabelSelector, Time};
use k8s_openapi::jiff::Timestamp;
use kube::api::ObjectMeta;
use purlis_cluster_operator::{
    Admission, Claim, ClaimSpec, FORGE_POLICY, POLICY, RUNNER_NAMESPACES_POLICY, SETTLE, admission,
    in_force_since, shipped_policy,
};

/// The namespace the operator runs in, as the plain install names it.
const NAMESPACE: &str = "purlis-system";

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

/// The policy on Claims as the operator ships it, made at `time`.
fn policy_made(time: &str) -> ValidatingAdmissionPolicy {
    let mut policy = shipped_policy(POLICY, NAMESPACE).expect("the shipped policy");
    policy.metadata.creation_timestamp = Some(at(time));
    policy
}

/// When the policy is in force from under `binding`, the policy made long before it.
fn since(binding: &ValidatingAdmissionPolicyBinding) -> Option<Timestamp> {
    in_force_since(
        Some(&policy_made("2026-01-01T00:00:00Z")),
        Some(binding),
        POLICY,
        NAMESPACE,
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

/// The bindings purlis installs give the policy no parameters: one that does is no binding.
#[test]
fn a_binding_that_gives_the_policy_parameters_is_no_binding() {
    let claim = claim_made(Some("2026-10-17T10:00:31Z"));
    let mut with_params = in_force("2026-10-17T10:00:00Z");
    with_params.spec.param_ref = Some(ParamRef {
        name: Some("params".into()),
        ..ParamRef::default()
    });
    assert_eq!(
        admission(&claim, since(&with_params)),
        Admission::PolicyNotInForce
    );
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
        in_force_since(Some(&older), Some(&binding), POLICY, NAMESPACE),
        Some(settled("2026-10-17T10:00:00Z"))
    );
    // Made again under a binding that stayed: what was made before it was not admitted under it.
    let again = policy_made("2026-10-17T11:00:00Z");
    assert_eq!(
        in_force_since(Some(&again), Some(&binding), POLICY, NAMESPACE),
        Some(settled("2026-10-17T11:00:00Z"))
    );
    assert_eq!(
        admission(
            &claim_made(Some("2026-10-17T10:30:00Z")),
            in_force_since(Some(&again), Some(&binding), POLICY, NAMESPACE)
        ),
        Admission::MadeBeforePolicy
    );
}

#[test]
fn a_binding_with_no_policy_holds_nothing() {
    let binding = in_force("2026-10-17T10:00:00Z");
    assert_eq!(
        in_force_since(None, Some(&binding), POLICY, NAMESPACE),
        None
    );
}

#[test]
fn a_binding_changed_since_it_was_made_is_not_in_force() {
    let policy = policy_made("2026-10-17T09:00:00Z");
    for generation in [Some(2), None] {
        let mut changed = in_force("2026-10-17T10:00:00Z");
        changed.metadata.generation = generation;
        assert_eq!(
            in_force_since(Some(&policy), Some(&changed), POLICY, NAMESPACE),
            None,
            "{generation:?}"
        );
    }
}

/// A binding that names what it matches the way the API server would leave it unnamed.
fn matching_everything() -> MatchResources {
    MatchResources {
        namespace_selector: Some(LabelSelector::default()),
        object_selector: Some(LabelSelector::default()),
        match_policy: Some("Equivalent".into()),
        ..MatchResources::default()
    }
}

#[test]
fn a_binding_that_narrows_what_it_matches_is_not_in_force() {
    let narrowed = |change: fn(&mut MatchResources)| {
        let mut resources = matching_everything();
        change(&mut resources);
        let mut binding = in_force("2026-10-17T10:00:00Z");
        binding.spec.match_resources = Some(resources);
        binding
    };
    let some_namespaces = LabelSelector {
        match_labels: Some([("team".to_owned(), "a".to_owned())].into()),
        ..LabelSelector::default()
    };
    let one_rule = vec![NamedRuleWithOperations {
        resources: Some(vec!["claims".into()]),
        ..NamedRuleWithOperations::default()
    }];
    let bindings = [
        ("a namespace selector", {
            let mut b = narrowed(|_| {});
            b.spec.match_resources.as_mut().unwrap().namespace_selector =
                Some(some_namespaces.clone());
            b
        }),
        ("an object selector", {
            let mut b = narrowed(|_| {});
            b.spec.match_resources.as_mut().unwrap().object_selector =
                Some(some_namespaces.clone());
            b
        }),
        ("resource rules", {
            let mut b = narrowed(|_| {});
            b.spec.match_resources.as_mut().unwrap().resource_rules = Some(one_rule.clone());
            b
        }),
        ("excluded resource rules", {
            let mut b = narrowed(|_| {});
            b.spec
                .match_resources
                .as_mut()
                .unwrap()
                .exclude_resource_rules = Some(one_rule.clone());
            b
        }),
        (
            "an exact match policy",
            narrowed(|r| r.match_policy = Some("Exact".into())),
        ),
    ];
    for (what, binding) in bindings {
        assert_eq!(
            since(&binding),
            None,
            "a binding narrowed by {what} is in force"
        );
    }
}

#[test]
fn a_binding_that_names_what_it_matches_as_everything_is_in_force() {
    for resources in [MatchResources::default(), matching_everything()] {
        let mut binding = in_force("2026-10-17T10:00:00Z");
        binding.spec.match_resources = Some(resources.clone());
        assert_eq!(
            since(&binding),
            Some(at("2026-10-17T10:00:00Z").0.checked_add(SETTLE).unwrap()),
            "{resources:?}"
        );
    }
}

#[test]
fn a_policy_that_lets_through_what_it_cannot_check_is_not_in_force() {
    let binding = in_force("2026-10-17T10:00:00Z");
    let with = |failure: Option<&str>| {
        let mut policy = policy_made("2026-10-17T09:00:00Z");
        policy.spec.as_mut().expect("a spec").failure_policy = failure.map(str::to_owned);
        policy
    };
    assert_eq!(
        in_force_since(
            Some(&with(Some("Ignore"))),
            Some(&binding),
            POLICY,
            NAMESPACE
        ),
        None
    );
    // `Fail` is what the API server takes when none is named.
    for failure in [Some("Fail"), None] {
        assert!(
            in_force_since(Some(&with(failure)), Some(&binding), POLICY, NAMESPACE).is_some(),
            "{failure:?}"
        );
    }
}

/// **The operator acts only while the policy is the one it ships**: a policy whose rules say
/// anything else, changed in place or made again, holds nothing the operator counts on.
#[test]
fn a_policy_that_is_not_the_one_the_operator_ships_is_not_in_force() {
    let binding = in_force("2026-10-17T10:00:00Z");
    let changed = |change: fn(&mut ValidatingAdmissionPolicySpec)| {
        let mut policy = policy_made("2026-10-17T09:00:00Z");
        change(policy.spec.as_mut().expect("a spec"));
        policy
    };
    let policies = [
        (
            "a rule taken out",
            changed(|spec| {
                spec.validations.as_mut().expect("its rules").remove(0);
            }),
        ),
        (
            "a rule that admits everything",
            changed(|spec| {
                spec.validations.as_mut().expect("its rules")[0].expression = "true".into();
            }),
        ),
        ("no rules at all", changed(|spec| spec.validations = None)),
        (
            "a rule that matches less",
            changed(|spec| {
                let rules = spec
                    .match_constraints
                    .as_mut()
                    .and_then(|constraints| constraints.resource_rules.as_mut())
                    .expect("its resource rules");
                rules[0].operations = Some(vec!["UPDATE".into()]);
            }),
        ),
        (
            "a condition on what it matches",
            changed(|spec| {
                spec.match_conditions = Some(vec![MatchCondition {
                    name: "never".into(),
                    expression: "false".into(),
                }]);
            }),
        ),
        (
            "an object selector",
            changed(|spec| {
                spec.match_constraints
                    .as_mut()
                    .expect("its constraints")
                    .object_selector = Some(LabelSelector {
                    match_labels: Some([("team".to_owned(), "a".to_owned())].into()),
                    ..LabelSelector::default()
                });
            }),
        ),
        ("no spec", {
            let mut policy = policy_made("2026-10-17T09:00:00Z");
            policy.spec = None;
            policy
        }),
    ];
    for (what, policy) in policies {
        assert_eq!(
            in_force_since(Some(&policy), Some(&binding), POLICY, NAMESPACE),
            None,
            "a policy with {what} is in force"
        );
    }
}

/// The shipped policy names the operator's own service account, in the namespace the operator
/// runs in: the same rules naming another namespace's are not the operator's.
#[test]
fn the_policy_is_the_operators_only_for_the_namespace_it_runs_in() {
    let binding = in_force("2026-10-17T10:00:00Z");
    let mut elsewhere = shipped_policy(POLICY, "elsewhere").expect("the shipped policy");
    elsewhere.metadata.creation_timestamp = Some(at("2026-10-17T09:00:00Z"));
    assert!(in_force_since(Some(&elsewhere), Some(&binding), POLICY, "elsewhere").is_some());
    assert_eq!(
        in_force_since(Some(&elsewhere), Some(&binding), POLICY, NAMESPACE),
        None
    );
}

/// The API server fills in what a policy leaves unnamed: a policy read back with those defaults
/// is still the one the operator ships.
#[test]
fn the_shipped_policy_read_back_with_the_servers_defaults_is_in_force() {
    let binding = in_force("2026-10-17T10:00:00Z");
    let mut policy = policy_made("2026-10-17T09:00:00Z");
    let spec = policy.spec.as_mut().expect("a spec");
    spec.failure_policy = Some("Fail".into());
    let constraints = spec.match_constraints.as_mut().expect("its constraints");
    constraints.match_policy = Some("Equivalent".into());
    constraints.namespace_selector = Some(LabelSelector::default());
    constraints.object_selector = Some(LabelSelector::default());
    for rule in constraints.resource_rules.iter_mut().flatten() {
        rule.scope = Some("*".into());
    }
    assert!(in_force_since(Some(&policy), Some(&binding), POLICY, NAMESPACE).is_some());
}

#[test]
fn the_operator_ships_each_policy_it_reads() {
    for name in [POLICY, RUNNER_NAMESPACES_POLICY, FORGE_POLICY] {
        let policy = shipped_policy(name, NAMESPACE).unwrap_or_else(|| panic!("{name}"));
        assert_eq!(policy.metadata.name.as_deref(), Some(name));
        assert!(
            policy
                .spec
                .as_ref()
                .and_then(|spec| spec.validations.as_ref())
                .is_some_and(|rules| !rules.is_empty()),
            "{name}"
        );
    }
    assert!(shipped_policy("no-such-policy", NAMESPACE).is_none());
}
