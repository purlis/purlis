// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! One login follow-up to #7371 (ADR 0128 amended 2026-11-22): `glab` reaches the listener inside
//! TLS, where it cannot follow a redirect to object storage itself. gitlab.com's answer for a job's
//! artifacts is followed **once, by a GET, with no credential**, to its one artifacts bucket and to
//! nothing else. A self-hosted GitLab's redirect, another bucket, another host that is spelled like
//! it, a second redirect and a redirect after a write are answered `502`, never followed.
//! Run on socket pairs, so no port is bound.

use purlis_core::forge::proxy::listener::ChatCa;
use purlis_core::forge::proxy::{AllowedRepo, Kind};

use crate::support::forge_listener::{Answer, REAL, Reply, World, basic, tls};

const SIGNED: &str = "https://storage.googleapis.com/gitlab-gprd-artifacts/ab/cd/9/artifacts.zip?X-Goog-Signature=abc";

fn bearer(w: &World) -> String {
    format!("Bearer {}", w.sentinel.expose())
}

fn gitlab(w: &World, ca: &ChatCa, host: &str, method: &str, target: &str) -> Reply {
    let request = format!(
        "{method} {target} HTTP/1.1\r\nHost: {host}\r\nAuthorization: {}\r\nContent-Length: 0\r\n\r\n",
        bearer(w)
    );
    tls(w, ca, host, 443, host, request.as_bytes()).expect("a handshake")
}

fn world() -> (World, ChatCa) {
    let w = World::new();
    let ca = ChatCa::new(&w.proxy.terminable_hosts(&w.chat)).unwrap();
    (w, ca)
}

const ARTIFACTS: &str = "/api/v4/projects/group%2Fproj/jobs/5/artifacts";

#[test]
fn a_job_log_is_read_through_the_tunnel() {
    purlis_core::unsteered!();
    let (w, ca) = world();
    w.fake.answer(Answer::ok("step 1\nstep 2\n"));
    let reply = gitlab(
        &w,
        &ca,
        "gitlab.com",
        "GET",
        "/api/v4/projects/group%2Fproj/jobs/5/trace",
    );
    assert_eq!(reply.status, 200, "{}", reply.text());
    assert_eq!(reply.text(), "step 1\nstep 2\n");
    assert_eq!(
        w.fake.requests()[0].url,
        "https://gitlab.com/api/v4/projects/group%2Fproj/jobs/5/trace"
    );
}

#[test]
fn a_redirect_to_gitlab_coms_artifact_bucket_is_followed_once_with_no_credential() {
    purlis_core::unsteered!();
    for target in [
        ARTIFACTS,
        "/api/v4/projects/42/jobs/5/artifacts",
        "/api/v4/projects/group%2Fproj/jobs/artifacts/main/download?job=build",
        "/api/v4/projects/group%2Fproj/jobs/5/trace",
    ] {
        let (w, ca) = world();
        w.fake
            .answer(Answer::ok("").status(302).with_header("Location", SIGNED));
        w.fake.answer(Answer::ok("ZIPBYTES"));
        let reply = gitlab(&w, &ca, "gitlab.com", "GET", target);
        assert_eq!(reply.status, 200, "{target}: {}", reply.text());
        assert_eq!(reply.body, b"ZIPBYTES", "{target}");
        let seen = w.fake.requests();
        assert_eq!(seen.len(), 2, "{target}");
        assert_eq!(seen[1].url, SIGNED);
        assert_eq!(seen[1].method, "GET");
        assert!(
            seen[1]
                .headers
                .iter()
                .all(|(n, _)| !n.eq_ignore_ascii_case("authorization")
                    && !n.eq_ignore_ascii_case("private-token")),
            "{target}: the credential followed the redirect: {:?}",
            seen[1].headers
        );
        assert!(!format!("{:?}", seen[1]).contains(REAL));
    }
}

#[test]
fn a_redirect_to_any_other_host_or_bucket_is_not_followed() {
    purlis_core::unsteered!();
    let (w, ca) = world();
    for location in [
        // Anyone's bucket on the same host, and a name that begins like the right one.
        "https://storage.googleapis.com/attacker-bucket/x?X-Goog-Signature=abc",
        "https://storage.googleapis.com/gitlab-gprd-artifacts-evil/x",
        "https://storage.googleapis.com/gitlab-gprd-artifacts",
        "https://storage.googleapis.com/other/gitlab-gprd-artifacts/x",
        // A `..` is resolved before the path is read.
        "https://storage.googleapis.com/gitlab-gprd-artifacts/../attacker-bucket/x",
        "https://storage.googleapis.com/gitlab-gprd-artifacts/%2e%2e/attacker-bucket/x",
        // The host spelled like it, and the bucket as a subdomain.
        "https://gitlab-gprd-artifacts.storage.googleapis.com/x",
        "https://storage.googleapis.com.evil.example/gitlab-gprd-artifacts/x",
        "https://evilstorage.googleapis.com/gitlab-gprd-artifacts/x",
        "https://storage.googleapis.com./gitlab-gprd-artifacts/x",
        // Not https, another port, a login in the address.
        "http://storage.googleapis.com/gitlab-gprd-artifacts/x",
        "https://storage.googleapis.com:8443/gitlab-gprd-artifacts/x",
        "https://user@storage.googleapis.com/gitlab-gprd-artifacts/x",
        // GitHub's storage is not GitLab's, and nothing else is carried.
        "https://pipelines.actions.githubusercontent.com/x?sig=abc",
        "https://productionresultssa7.blob.core.windows.net/x?sig=abc",
        "https://evil.example/x",
        "http://169.254.169.254/latest/meta-data/",
        "https://localhost/x",
    ] {
        w.fake
            .answer(Answer::ok("").status(302).with_header("Location", location));
        let reply = gitlab(&w, &ca, "gitlab.com", "GET", ARTIFACTS);
        assert_eq!(reply.status, 502, "{location}: {}", reply.text());
        assert_eq!(w.fake.requests().len(), 1, "{location}");
        w.fake.seen.lock().unwrap().clear();
    }
}

#[test]
fn a_second_redirect_and_one_after_a_write_are_not_followed() {
    purlis_core::unsteered!();
    let (w, ca) = world();
    w.fake
        .answer(Answer::ok("").status(302).with_header("Location", SIGNED));
    w.fake.answer(
        Answer::ok("")
            .status(302)
            .with_header("Location", SIGNED.replace("/9/", "/8/").as_str()),
    );
    let reply = gitlab(&w, &ca, "gitlab.com", "GET", ARTIFACTS);
    assert!(reply.status >= 300, "{reply:?}");
    assert_eq!(w.fake.requests().len(), 2);
    assert!(
        reply
            .header("location")
            .is_none_or(|l| !l.contains("storage.googleapis.com")),
        "{reply:?}"
    );

    // A retry (a write) answered with a redirect to the same place is never followed.
    let (w, ca) = world();
    w.fake
        .answer(Answer::ok("").status(302).with_header("Location", SIGNED));
    let reply = gitlab(
        &w,
        &ca,
        "gitlab.com",
        "POST",
        "/api/v4/projects/group%2Fproj/jobs/5/retry",
    );
    assert_eq!(reply.status, 502, "{}", reply.text());
    assert_eq!(w.fake.requests().len(), 1);
}

#[test]
fn a_self_hosted_gitlab_or_a_github_repo_is_not_followed_to_gitlab_coms_bucket() {
    purlis_core::unsteered!();
    // A self-hosted GitLab serves artifacts itself; its object storage is the operator's choice
    // of host, and is not carried, not even the bucket gitlab.com uses.
    let w = World::with_repos(vec![
        AllowedRepo::new(Kind::GitLab, "git.corp.example", 443, "grp/proj", "main").unwrap(),
    ]);
    let ca = ChatCa::new(&w.proxy.terminable_hosts(&w.chat)).unwrap();
    w.fake
        .answer(Answer::ok("").status(302).with_header("Location", SIGNED));
    let reply = gitlab(
        &w,
        &ca,
        "git.corp.example",
        "GET",
        "/api/v4/projects/grp%2Fproj/jobs/5/artifacts",
    );
    assert_eq!(reply.status, 502, "{}", reply.text());
    assert_eq!(w.fake.requests().len(), 1);

    // A GitHub chat is not given GitLab's rule: a redirect to that bucket is refused.
    let (w, ca) = world();
    w.fake
        .answer(Answer::ok("").status(302).with_header("Location", SIGNED));
    let request = format!(
        "GET /repos/acme/app/actions/jobs/5/logs HTTP/1.1\r\nHost: api.github.com\r\nAuthorization: {}\r\n\r\n",
        basic("x-access-token", w.sentinel.expose())
    );
    let reply = tls(
        &w,
        &ca,
        "api.github.com",
        443,
        "api.github.com",
        request.as_bytes(),
    )
    .expect("a handshake");
    assert_eq!(reply.status, 502, "{}", reply.text());
    assert_eq!(w.fake.requests().len(), 1);
}
