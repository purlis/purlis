// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The operator's metrics** (#3737, the person's ruling of 2026-10-29), in Prometheus's text
//! format on its own port: its Claims by pool, namespace and state, the runners each pool keeps
//! warm in each runner namespace, and how many runners it suspended and Claims it saw deleted,
//! by pool and cause.

use k8s_openapi::apimachinery::pkg::apis::meta::v1::{Condition, Time};
use kube::api::ObjectMeta;
use purlis_cluster_operator::metrics::{self, Cause, Metrics};
use purlis_cluster_operator::upstream::{
    Reference, SandboxWarmPool, SandboxWarmPoolSpec, SandboxWarmPoolStatus,
};
use purlis_cluster_operator::{Claim, ClaimSpec, ClaimStatus};
use std::sync::Arc;

fn claim(namespace: &str, name: &str, pool: &str, ready: Option<(&str, &str)>) -> Arc<Claim> {
    let mut claim = Claim::new(
        name,
        ClaimSpec {
            pool: pool.into(),
            requester: "ada@example.com".into(),
            kept: false,
            stopped: false,
        },
    );
    claim.metadata.namespace = Some(namespace.into());
    claim.status = ready.map(|(status, reason)| ClaimStatus {
        conditions: vec![Condition {
            type_: "Ready".into(),
            status: status.into(),
            reason: reason.into(),
            message: String::new(),
            last_transition_time: Time(k8s_openapi::jiff::Timestamp::UNIX_EPOCH),
            observed_generation: None,
        }],
        link_key: None,
    });
    Arc::new(claim)
}

fn warm_pool(namespace: &str, name: &str, ready: Option<i32>) -> Arc<SandboxWarmPool> {
    Arc::new(SandboxWarmPool {
        metadata: ObjectMeta {
            name: Some(name.into()),
            namespace: Some(namespace.into()),
            ..ObjectMeta::default()
        },
        spec: SandboxWarmPoolSpec {
            replicas: 2,
            sandbox_template_ref: Reference { name: name.into() },
            update_strategy: None,
        },
        status: Some(SandboxWarmPoolStatus {
            ready_replicas: ready,
        }),
    })
}

fn lines(text: &str) -> Vec<&str> {
    text.lines().filter(|line| !line.starts_with('#')).collect()
}

#[test]
fn claims_are_counted_by_pool_namespace_and_state() {
    let claims = [
        claim("ada", "a", "team", Some(("True", "Ready"))),
        claim("ada", "b", "team", Some(("True", "Ready"))),
        claim("ada", "c", "team", Some(("False", "Stopped"))),
        claim("bob", "d", "team", None),
        claim("bob", "e", "gpu", Some(("False", "PoolNotFound"))),
    ];
    let text = Metrics::default().render(&claims, &[]);
    let said = lines(&text);
    for line in [
        r#"purlis_cluster_operator_claims{pool="gpu",namespace="bob",state="PoolNotFound"} 1"#,
        r#"purlis_cluster_operator_claims{pool="team",namespace="ada",state="Ready"} 2"#,
        r#"purlis_cluster_operator_claims{pool="team",namespace="ada",state="Stopped"} 1"#,
        r#"purlis_cluster_operator_claims{pool="team",namespace="bob",state="Unknown"} 1"#,
    ] {
        assert!(said.contains(&line), "{line}\n{text}");
    }
    assert!(
        text.contains("# TYPE purlis_cluster_operator_claims gauge"),
        "{text}"
    );
}

#[test]
fn warm_runners_are_counted_by_pool_and_runner_namespace() {
    let warm = [
        warm_pool("purlis-runners-0123456789ab", "purlis-pool-team", Some(2)),
        warm_pool("purlis-runners-ba9876543210", "purlis-pool-team", None),
        // Not named for a pool, or not in a runner namespace: not the operator's.
        warm_pool("purlis-runners-0123456789ab", "other", Some(5)),
        warm_pool("default", "purlis-pool-team", Some(7)),
    ];
    let text = Metrics::default().render(&[], &warm);
    assert_eq!(
        lines(&text),
        [
            r#"purlis_cluster_operator_warm_runners{pool="team",namespace="purlis-runners-0123456789ab"} 2"#,
            r#"purlis_cluster_operator_warm_runners{pool="team",namespace="purlis-runners-ba9876543210"} 0"#,
        ],
        "{text}"
    );
    assert!(
        text.contains("# TYPE purlis_cluster_operator_warm_runners gauge"),
        "{text}"
    );
}

#[test]
fn suspensions_and_deletions_count_up_by_pool_and_cause() {
    let metrics = Metrics::default();
    metrics.suspended("team", Cause::Stopped);
    metrics.suspended("team", Cause::Stopped);
    metrics.deleted("team", Cause::Requested);
    metrics.deleted("gpu", Cause::Requested);
    let text = metrics.render(&[], &[]);
    let said = lines(&text);
    for line in [
        r#"purlis_cluster_operator_suspensions_total{pool="team",cause="stopped"} 2"#,
        r#"purlis_cluster_operator_claims_deleted_total{pool="gpu",cause="requested"} 1"#,
        r#"purlis_cluster_operator_claims_deleted_total{pool="team",cause="requested"} 1"#,
    ] {
        assert!(said.contains(&line), "{line}\n{text}");
    }
    for counter in ["suspensions_total", "claims_deleted_total"] {
        assert!(
            text.contains(&format!("# TYPE purlis_cluster_operator_{counter} counter")),
            "{text}"
        );
    }
}

#[test]
fn a_label_value_is_escaped() {
    let claims = [claim(
        "ada",
        "a",
        "team",
        Some(("False", "Odd\"Reason\\\n")),
    )];
    let text = Metrics::default().render(&claims, &[]);
    assert!(text.contains(r#"state="Odd\"Reason\\\n"} 1"#), "{text}");
}

#[test]
fn the_port_answers_a_scrape_with_the_metrics_and_anything_else_with_404() {
    let scrape = metrics::answer(
        "GET /metrics HTTP/1.1\r\nHost: operator:8080\r\nAccept: text/plain\r\n\r\n",
        || "purlis_cluster_operator_claims 0\n".to_owned(),
    );
    assert!(scrape.starts_with("HTTP/1.1 200 OK\r\n"), "{scrape}");
    assert!(
        scrape.contains("content-type: text/plain; version=0.0.4; charset=utf-8\r\n"),
        "{scrape}"
    );
    assert!(scrape.contains("content-length: 33\r\n"), "{scrape}");
    assert!(
        scrape.ends_with("\r\n\r\npurlis_cluster_operator_claims 0\n"),
        "{scrape}"
    );
    assert!(
        metrics::answer("GET /metrics?x=1 HTTP/1.1\r\n\r\n", String::new)
            .starts_with("HTTP/1.1 200"),
        "a query is ignored"
    );
    for head in [
        "GET / HTTP/1.1\r\n\r\n",
        "POST /metrics HTTP/1.1\r\n\r\n",
        "",
        "garbage",
    ] {
        let answered = metrics::answer(head, || panic!("rendered for {head:?}"));
        assert!(
            answered.starts_with("HTTP/1.1 404 Not Found\r\n"),
            "{head:?}: {answered}"
        );
    }
}
