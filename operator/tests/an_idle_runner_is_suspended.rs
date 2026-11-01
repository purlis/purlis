// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A runner idle for its pool's `idleAfter` is suspended** (#3980): it says so with a known
//! termination message, its Claim says it, and a word from its desktop for this suspension runs
//! it again.

use k8s_openapi::api::core::v1::{
    ContainerState, ContainerStateTerminated, ContainerStatus, Pod, PodStatus,
};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;
use k8s_openapi::jiff::{SignedDuration, Timestamp};
use purlis_cluster_operator::lifecycle::{
    IDLE, IDLE_MESSAGE, IDLE_REASON, IDLE_WORD_HOLDS, RUN_AGAIN_ANNOTATION, asked_to_run_again,
    idle_condition, keeping_idle, said_idle, status_of_an_idle_runner, status_woken,
};
use purlis_cluster_operator::upstream::{
    IDLE_AFTER_VAR, RUNNER_CONTAINER, SUSPENDED, Sandbox, SandboxSpec, Suspension,
    operating_mode_while_idle,
};
use purlis_cluster_operator::{Claim, ClaimSpec};

fn ended(message: &str, at: Timestamp) -> ContainerState {
    ContainerState {
        terminated: Some(ContainerStateTerminated {
            message: Some(message.to_owned()),
            finished_at: Some(Time(at)),
            ..ContainerStateTerminated::default()
        }),
        ..ContainerState::default()
    }
}

fn pod(container: &str, last: Option<ContainerState>) -> Pod {
    Pod {
        status: Some(PodStatus {
            container_statuses: Some(vec![ContainerStatus {
                name: container.to_owned(),
                last_state: last,
                ..ContainerStatus::default()
            }]),
            ..PodStatus::default()
        }),
        ..Pod::default()
    }
}

#[test]
fn only_the_runners_own_recent_word_says_it_is_idle() {
    let now = Timestamp::now();
    let said = |pod: Pod| said_idle(&pod, now);
    assert!(said(pod(RUNNER_CONTAINER, Some(ended(IDLE_MESSAGE, now)))));
    assert!(!said(pod(RUNNER_CONTAINER, None)), "it has not ended");
    assert!(
        !said(pod(RUNNER_CONTAINER, Some(ended("out of memory", now)))),
        "another end says nothing"
    );
    assert!(
        !said(pod("another", Some(ended(IDLE_MESSAGE, now)))),
        "another container's word is not the runner's"
    );
    let old = now - IDLE_WORD_HOLDS - SignedDuration::from_secs(1);
    assert!(
        !said(pod(RUNNER_CONTAINER, Some(ended(IDLE_MESSAGE, old)))),
        "a word from before its host served again is not taken"
    );
}

fn claim() -> Claim {
    Claim::new(
        "ada-1",
        ClaimSpec {
            pool: "team".into(),
            requester: "ada@example.com".into(),
            kept: true,
            stopped: false,
            forge: None,
        },
    )
}

#[test]
fn a_claim_says_its_runner_is_idle_and_vouches_for_no_key() {
    let status = status_of_an_idle_runner(Suspension::Suspended);
    let of = |kind: &str| status.conditions.iter().find(|c| c.type_ == kind).unwrap();
    assert_eq!(of("Ready").status, "False");
    assert_eq!(of("Ready").reason, IDLE_REASON);
    assert_eq!(of(IDLE).status, "True");
    assert_eq!(of("KeyPublished").status, "False");
    assert!(status.link_key.is_none());
    let mut idle = claim();
    idle.status = Some(status);
    assert!(idle_condition(&idle).is_some());
}

#[test]
fn only_a_word_for_this_suspension_runs_it_again() {
    let mut idle = claim();
    idle.status = Some(status_of_an_idle_runner(Suspension::Suspended));
    assert!(!asked_to_run_again(&idle), "nobody asked");
    let since = idle_condition(&idle).unwrap().last_transition_time.0;
    let word = |at: String| Some([(RUN_AGAIN_ANNOTATION.to_owned(), at)].into());
    idle.metadata.annotations = word((since - SignedDuration::from_secs(60)).to_string());
    assert!(!asked_to_run_again(&idle), "a word for an earlier one");
    idle.metadata.annotations = word("whenever".to_owned());
    assert!(!asked_to_run_again(&idle), "no time at all");
    idle.metadata.annotations = word(since.to_string());
    assert!(asked_to_run_again(&idle));
    assert!(!asked_to_run_again(&claim()), "a claim that is not idle");
}

#[test]
fn an_idle_runner_is_suspended_once() {
    let sandbox = |mode: Option<&str>| {
        Sandbox::new(
            "ada-1",
            SandboxSpec {
                operating_mode: mode.map(str::to_owned),
            },
        )
    };
    assert_eq!(operating_mode_while_idle(&sandbox(None)), Some(SUSPENDED));
    assert_eq!(
        operating_mode_while_idle(&sandbox(Some("Running"))),
        Some(SUSPENDED)
    );
    assert_eq!(operating_mode_while_idle(&sandbox(Some(SUSPENDED))), None);
}

#[test]
fn only_a_word_from_its_desktop_ends_a_claims_idle() {
    let idle = status_of_an_idle_runner(Suspension::Suspended);
    let other = purlis_cluster_operator::status_of_a_missing_pool(&claim());
    let says_idle = |status: &purlis_cluster_operator::ClaimStatus| {
        status.conditions.iter().any(|c| c.type_ == IDLE)
    };
    assert!(
        says_idle(&keeping_idle(Some(&idle), other.clone())),
        "a status about something else keeps it"
    );
    assert!(!says_idle(&keeping_idle(None, other)), "nothing to keep");
    assert!(!says_idle(&status_woken()));
}

#[test]
fn a_change_of_idle_time_leaves_a_runner_its_templates() {
    use k8s_openapi::api::core::v1::{Container, EnvVar, PodSpec, PodTemplateSpec};
    use purlis_cluster_operator::vouch::template_is_ours;
    let with = |env: Vec<(&str, &str)>| PodTemplateSpec {
        metadata: None,
        spec: Some(PodSpec {
            containers: vec![Container {
                name: RUNNER_CONTAINER.to_owned(),
                env: Some(
                    env.into_iter()
                        .map(|(name, value)| EnvVar {
                            name: name.to_owned(),
                            value: Some(value.to_owned()),
                            ..EnvVar::default()
                        })
                        .collect(),
                ),
                ..Container::default()
            }],
            ..PodSpec::default()
        }),
    };
    let wanted = with(vec![("HOME", "/h"), (IDLE_AFTER_VAR, "30m")]);
    assert!(template_is_ours(&with(vec![("HOME", "/h")]), &wanted));
    assert!(template_is_ours(
        &with(vec![("HOME", "/h"), (IDLE_AFTER_VAR, "2h")]),
        &wanted
    ));
    assert!(
        !template_is_ours(&with(vec![("HOME", "/other")]), &wanted),
        "any other variable still counts"
    );
}
