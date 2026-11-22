// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! One login follow-up (#7372; ADR 0128 amended 2026-11-22): the CI and landing work a chat does
//! on its own GitLab project is answered, as #7358 did for GitHub: protected branches, approval
//! rules and a merge request's approval state, merge trains, pipelines and jobs (their logs and
//! artifacts too), and a retry or a cancel of a pipeline or a job. Another project (by path or by
//! numeric id), a write to a rule, a merge-train join, `play` and the trigger routes stay refused.

use purlis_core::forge::proxy::{Class, Outcome, Refusal};

use crate::support::forge_proxy::{GITLAB_SELF, Scene, is_refused};

/// The project as a path and as the numeric id the person's confirmation recorded (42).
const BY_PATH: &str = "/api/v4/projects/group%2Fproj";
const BY_ID: &str = "/api/v4/projects/42";

fn class(out: Outcome) -> Class {
    match out {
        Outcome::Forward(allowed) => allowed.class,
        other => panic!("not forwarded: {other:?}"),
    }
}

#[test]
fn the_rules_pipelines_jobs_and_trains_of_the_chats_project_are_read() {
    purlis_core::unsteered!();
    let s = Scene::new();
    for project in [BY_PATH, BY_ID] {
        for rest in [
            "/protected_branches",
            "/protected_branches/main",
            "/protected_branches?per_page=100&page=2",
            "/approval_rules",
            "/merge_requests/3/approval_state",
            "/merge_trains",
            "/merge_trains?scope=active&per_page=50",
            "/merge_trains/merge_requests/3",
            "/pipelines",
            "/pipelines?ref=main&status=failed&per_page=20",
            "/pipelines/latest?ref=main",
            "/pipelines/9",
            "/pipelines/9/jobs",
            "/jobs/5",
            "/jobs/5/trace",
            "/jobs/5/artifacts",
            "/jobs/artifacts/main/download?job=build",
        ] {
            let target = format!("{project}{rest}");
            let out = s.send("GET", "gitlab.com", &target, b"");
            assert_eq!(class(out), Class::ApiRead, "{target}");
        }
    }
}

#[test]
fn a_self_hosted_projects_ci_reads_and_retries_are_answered_by_its_path() {
    purlis_core::unsteered!();
    let s = Scene::new();
    let project = "/api/v4/projects/grp%2Fsub%2Fproj";
    for rest in ["/pipelines/9/jobs", "/jobs/5/trace", "/protected_branches"] {
        let out = s.send_port("GET", GITLAB_SELF, 8443, &format!("{project}{rest}"));
        assert_eq!(class(out), Class::ApiRead, "{rest}");
    }
    let out = s.send_port(
        "POST",
        GITLAB_SELF,
        8443,
        &format!("{project}/jobs/5/retry"),
    );
    assert_eq!(class(out), Class::CiWrite);
}

#[test]
fn another_project_by_path_or_by_id_is_refused_for_every_ci_and_land_read() {
    purlis_core::unsteered!();
    let s = Scene::new();
    // `group/other` is not recorded; 43 is not the recorded id; the self-hosted project has no
    // id on record, so no number is taken for it.
    for project in [
        "/api/v4/projects/group%2Fother",
        "/api/v4/projects/group%2Fproj-x",
        "/api/v4/projects/group%2Fproj.git",
        "/api/v4/projects/43",
        "/api/v4/projects/4",
        "/api/v4/projects/420",
    ] {
        for rest in [
            "/protected_branches",
            "/approval_rules",
            "/merge_requests/3/approval_state",
            "/merge_trains",
            "/pipelines/9/jobs",
            "/jobs/5/trace",
            "/jobs/5/artifacts",
            "/jobs/artifacts/main/download?job=build",
        ] {
            let target = format!("{project}{rest}");
            let out = s.send("GET", "gitlab.com", &target, b"");
            assert!(is_refused(&out, Refusal::OtherRepo), "{target}: {out:?}");
        }
    }
    let out = s.send_port("GET", GITLAB_SELF, 8443, "/api/v4/projects/42/jobs/5/trace");
    assert!(is_refused(&out, Refusal::OtherRepo), "{out:?}");
}

#[test]
fn a_write_to_a_rule_and_the_rules_nearby_reads_stay_refused() {
    purlis_core::unsteered!();
    let s = Scene::new();
    for project in [BY_PATH, BY_ID] {
        for (method, rest) in [
            ("POST", "/protected_branches"),
            ("PUT", "/protected_branches/main"),
            ("PATCH", "/protected_branches/main"),
            ("DELETE", "/protected_branches/main"),
            ("POST", "/approval_rules"),
            ("PUT", "/approval_rules/4"),
            ("DELETE", "/approval_rules/4"),
            // What stays refused to read as well: the settings, the push rules, the tags.
            ("GET", "/approval_settings"),
            ("GET", "/push_rule"),
            ("GET", "/protected_tags"),
            ("GET", "/approval_rules/4/extra"),
            ("GET", "/protected_branches/main/extra"),
        ] {
            let target = format!("{project}{rest}");
            let out = s.send(method, "gitlab.com", &target, b"{}");
            assert!(
                is_refused(&out, Refusal::Ruleset),
                "{method} {target}: {out:?}"
            );
        }
    }
}

#[test]
fn a_retry_or_a_cancel_of_a_pipeline_or_a_job_of_the_chats_project_is_answered() {
    purlis_core::unsteered!();
    let s = Scene::new();
    for project in [BY_PATH, BY_ID] {
        for rest in [
            "/pipelines/9/retry",
            "/pipelines/9/cancel",
            "/jobs/5/retry",
            "/jobs/5/cancel",
        ] {
            for body in [&b""[..], b"{}"] {
                let target = format!("{project}{rest}");
                let out = s.send("POST", "gitlab.com", &target, body);
                assert_eq!(class(out), Class::CiWrite, "{target}");
            }
        }
    }
}

#[test]
fn a_retry_that_carries_a_body_or_a_query_is_refused() {
    purlis_core::unsteered!();
    let s = Scene::new();
    for body in [
        &br#"{"x":1}"#[..],
        br#"{"job_variables_attributes":[{"key":"A","value":"b"}]}"#,
        b"key=value",
    ] {
        for rest in ["/pipelines/9/retry", "/jobs/5/retry", "/jobs/5/cancel"] {
            let target = format!("{BY_PATH}{rest}");
            let out = s.send("POST", "gitlab.com", &target, body);
            assert!(
                is_refused(&out, Refusal::MalformedRequest),
                "{rest} {}: {out:?}",
                String::from_utf8_lossy(body)
            );
        }
    }
    let out = s.send(
        "POST",
        "gitlab.com",
        &format!("{BY_PATH}/jobs/5/retry?x=1"),
        b"",
    );
    assert!(is_refused(&out, Refusal::MalformedRequest), "{out:?}");
}

#[test]
fn a_retry_or_a_cancel_of_another_project_is_refused() {
    purlis_core::unsteered!();
    let s = Scene::new();
    for project in [
        "/api/v4/projects/group%2Fother",
        "/api/v4/projects/43",
        "/api/v4/projects/42x",
    ] {
        for rest in ["/pipelines/9/retry", "/jobs/5/retry", "/jobs/5/cancel"] {
            let target = format!("{project}{rest}");
            let out = s.send("POST", "gitlab.com", &target, b"");
            assert!(is_refused(&out, Refusal::OtherRepo), "{target}: {out:?}");
        }
    }
}

#[test]
fn play_is_held_for_the_person_whatever_it_carries() {
    purlis_core::unsteered!();
    let s = Scene::new();
    for project in [BY_PATH, BY_ID] {
        for body in [
            &b""[..],
            b"{}",
            br#"{"job_variables_attributes":[{"key":"DEPLOY","value":"prod"}]}"#,
        ] {
            let target = format!("{project}/jobs/5/play");
            let out = s.send("POST", "gitlab.com", &target, body);
            assert!(is_refused(&out, Refusal::Approval), "{target}: {out:?}");
        }
    }
}

#[test]
fn the_trigger_routes_and_every_other_ci_write_stay_refused() {
    purlis_core::unsteered!();
    let s = Scene::new();
    for (method, rest) in [
        ("POST", "/trigger/pipeline"),
        ("POST", "/ref/main/trigger/pipeline"),
        ("POST", "/pipeline"),
        ("POST", "/pipelines"),
        ("DELETE", "/pipelines/9"),
        ("POST", "/jobs/5/erase"),
        ("DELETE", "/jobs/5/artifacts"),
        ("POST", "/jobs/5/artifacts/keep"),
        ("POST", "/pipelines/x/retry"),
        ("POST", "/pipelines/9/retry/now"),
        ("PUT", "/pipelines/9/retry"),
        ("GET", "/triggers"),
    ] {
        let target = format!("{BY_PATH}{rest}");
        let out = s.send(method, "gitlab.com", &target, b"{}");
        assert!(
            matches!(out, Outcome::Refused(_)),
            "{method} {target}: {out:?}"
        );
    }
    // What a pipeline reads of other projects, and its variables, are not carried.
    for rest in ["/pipelines/9/bridges", "/pipelines/9/variables"] {
        let target = format!("{BY_PATH}{rest}");
        let out = s.send("GET", "gitlab.com", &target, b"");
        assert!(matches!(out, Outcome::Refused(_)), "{target}: {out:?}");
    }
}

#[test]
fn a_merge_train_is_read_but_joining_one_is_still_a_merge() {
    purlis_core::unsteered!();
    let s = Scene::new();
    for (method, rest, why) in [
        // The train's own routes: a write is not on the list.
        ("POST", "/merge_trains/merge_requests/3", Refusal::NotOnList),
        (
            "DELETE",
            "/merge_trains/merge_requests/3",
            Refusal::NotOnList,
        ),
        // Every form of merge, and of approval, is what it was.
        ("PUT", "/merge_requests/3/merge", Refusal::Merge),
        (
            "POST",
            "/merge_requests/3/cancel_merge_when_pipeline_succeeds",
            Refusal::Merge,
        ),
        ("POST", "/merge_requests/3/approve", Refusal::Approval),
        ("POST", "/merge_requests/3/unapprove", Refusal::Approval),
        // Tags, releases, administration.
        ("POST", "/repository/tags", Refusal::ReleaseOrTag),
        ("POST", "/releases", Refusal::ReleaseOrTag),
        ("GET", "/variables", Refusal::Admin),
        ("GET", "/pipelines/9/variables", Refusal::Admin),
        ("POST", "/runners", Refusal::Admin),
    ] {
        let target = format!("{BY_PATH}{rest}");
        let out = s.send(method, "gitlab.com", &target, b"{}");
        assert!(is_refused(&out, why), "{method} {target}: {out:?}");
    }
}
