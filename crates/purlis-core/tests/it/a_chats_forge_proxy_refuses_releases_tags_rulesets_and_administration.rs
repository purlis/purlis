// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! One login, slice A: what protects a repo, and what administers it, is not a chat's to touch,
//! and the writes a chat may make are an allowlist, not a list of bad ones (ADR 0128 D2, Q331).

use purlis_core::forge::proxy::{Outcome, Refusal};

use crate::support::forge_proxy::{Scene, graphql, is_refused, push_body};

#[test]
fn release_and_tag_writes_are_refused() {
    purlis_core::unsteered!();
    let s = Scene::single();
    for (method, host, target) in [
        ("POST", "api.github.com", "/repos/acme/app/releases"),
        ("PATCH", "api.github.com", "/repos/acme/app/releases/5"),
        ("DELETE", "api.github.com", "/repos/acme/app/releases/5"),
        ("POST", "api.github.com", "/repos/acme/app/git/tags"),
        ("POST", "api.github.com", "/repos/acme/app/git/refs/tags/v1"),
        ("POST", "gitlab.com", "/api/v4/projects/42/repository/tags"),
        ("POST", "gitlab.com", "/api/v4/projects/42/releases"),
        (
            "DELETE",
            "gitlab.com",
            "/api/v4/projects/42/repository/tags/v1",
        ),
    ] {
        let out = s.send(method, host, target, b"{}");
        assert!(
            is_refused(&out, Refusal::ReleaseOrTag),
            "{method} {host}{target}: {out:?}"
        );
    }
    let q = "mutation{ createRelease(input:{repositoryId:\"R_acme_app\", tagName:\"v1\"}){ clientMutationId } }";
    let out = s.send(
        "POST",
        "api.github.com",
        "/graphql",
        &graphql(q, serde_json::json!({})),
    );
    assert!(is_refused(&out, Refusal::ReleaseOrTag), "{out:?}");
}

#[test]
fn rulesets_and_branch_protection_are_refused_to_read_and_to_write() {
    purlis_core::unsteered!();
    let s = Scene::single();
    for (method, host, target) in [
        ("POST", "api.github.com", "/repos/acme/app/rulesets"),
        ("GET", "api.github.com", "/repos/acme/app/rulesets"),
        (
            "PUT",
            "api.github.com",
            "/repos/acme/app/branches/main/protection",
        ),
        (
            "DELETE",
            "api.github.com",
            "/repos/acme/app/branches/main/protection",
        ),
        (
            "POST",
            "gitlab.com",
            "/api/v4/projects/42/protected_branches",
        ),
        ("PUT", "gitlab.com", "/api/v4/projects/42/approval_settings"),
        ("POST", "gitlab.com", "/api/v4/projects/42/push_rule"),
    ] {
        let out = s.send(method, host, target, b"{}");
        assert!(
            is_refused(&out, Refusal::Ruleset),
            "{method} {host}{target}: {out:?}"
        );
    }
    let q = "mutation{ createRepositoryRuleset(input:{sourceId:\"R_acme_app\", name:\"x\"}){ clientMutationId } }";
    let out = s.send(
        "POST",
        "api.github.com",
        "/graphql",
        &graphql(q, serde_json::json!({})),
    );
    assert!(is_refused(&out, Refusal::Ruleset), "{out:?}");
}

#[test]
fn repository_administration_is_refused_to_read_and_to_write() {
    purlis_core::unsteered!();
    let s = Scene::single();
    for (method, host, target) in [
        ("PATCH", "api.github.com", "/repos/acme/app"),
        ("DELETE", "api.github.com", "/repos/acme/app"),
        ("POST", "api.github.com", "/repos/acme/app/hooks"),
        ("GET", "api.github.com", "/repos/acme/app/hooks"),
        ("POST", "api.github.com", "/repos/acme/app/keys"),
        ("GET", "api.github.com", "/repos/acme/app/actions/secrets"),
        (
            "PUT",
            "api.github.com",
            "/repos/acme/app/actions/secrets/TOKEN",
        ),
        (
            "PUT",
            "api.github.com",
            "/repos/acme/app/collaborators/someone",
        ),
        ("POST", "api.github.com", "/repos/acme/app/transfer"),
        (
            "POST",
            "api.github.com",
            "/repos/acme/app/actions/workflows/ci.yml/dispatches",
        ),
        ("POST", "gitlab.com", "/api/v4/projects/42/hooks"),
        ("POST", "gitlab.com", "/api/v4/projects/42/deploy_keys"),
        ("POST", "gitlab.com", "/api/v4/projects/42/variables"),
        ("GET", "gitlab.com", "/api/v4/projects/42/variables"),
        ("POST", "gitlab.com", "/api/v4/projects/42/members"),
        ("PUT", "gitlab.com", "/api/v4/projects/42/settings"),
    ] {
        let out = s.send(method, host, target, b"{}");
        assert!(
            is_refused(&out, Refusal::Admin),
            "{method} {host}{target}: {out:?}"
        );
    }
    let q = "mutation{ updateRepository(input:{repositoryId:\"R_acme_app\", name:\"x\"}){ clientMutationId } }";
    let out = s.send(
        "POST",
        "api.github.com",
        "/graphql",
        &graphql(q, serde_json::json!({})),
    );
    assert!(is_refused(&out, Refusal::Admin), "{out:?}");
}

#[test]
fn a_write_route_that_is_not_on_the_list_is_refused_whatever_it_is() {
    purlis_core::unsteered!();
    let s = Scene::new();
    for (method, host, target) in [
        (
            "PUT",
            "api.github.com",
            "/repos/acme/app/contents/README.md",
        ),
        ("POST", "api.github.com", "/repos/acme/app/git/refs"),
        (
            "PATCH",
            "api.github.com",
            "/repos/acme/app/git/refs/heads/main",
        ),
        ("POST", "api.github.com", "/repos/acme/app/forks"),
        ("POST", "api.github.com", "/repos/acme/app/statuses/abc"),
        (
            "POST",
            "api.github.com",
            "/repos/acme/app/actions/runs/9/rerun",
        ),
        ("POST", "gitlab.com", "/api/v4/projects/42/pipeline"),
        (
            "POST",
            "gitlab.com",
            "/api/v4/projects/42/pipelines/9/retry",
        ),
        (
            "POST",
            "gitlab.com",
            "/api/v4/projects/42/repository/commits",
        ),
        ("GET", "api.github.com", "/user/repos"),
        ("GET", "gitlab.com", "/api/v4/users"),
        ("TRACE", "api.github.com", "/repos/acme/app/pulls"),
    ] {
        let out = s.send(method, host, target, b"{}");
        assert!(
            is_refused(&out, Refusal::NotOnList),
            "{method} {host}{target}: {out:?}"
        );
    }
}

#[test]
fn a_push_may_not_move_the_default_branch_a_protected_one_a_tag_or_another_kind_of_ref() {
    purlis_core::unsteered!();
    let s = Scene::new();
    let push = |host: &str, repo: &str, cmds: &[(&str, &str)]| {
        s.send(
            "POST",
            host,
            &format!("{repo}.git/git-receive-pack"),
            &push_body(cmds),
        )
    };
    for (cmds, why) in [
        (vec![("update", "refs/heads/main")], Refusal::ProtectedRef),
        (
            vec![("create", "refs/heads/gh-pages")],
            Refusal::ProtectedRef,
        ),
        (
            vec![("update", "refs/heads/someone-elses")],
            Refusal::ProtectedRef,
        ),
        (vec![("update", "refs/heads/MAIN")], Refusal::ProtectedRef),
        (vec![("delete", "refs/heads/main")], Refusal::ProtectedRef),
        (
            vec![("create", "refs/heads/release/1.0")],
            Refusal::ProtectedRef,
        ),
        (
            vec![("create", "refs/heads/deploy/prod")],
            Refusal::ProtectedRef,
        ),
        (vec![("create", "refs/heads/prod/x")], Refusal::ProtectedRef),
        (vec![("create", "refs/heads/master")], Refusal::ProtectedRef),
        (
            vec![("create", "refs/heads/production")],
            Refusal::ProtectedRef,
        ),
        (
            vec![("update", "refs/heads/release")],
            Refusal::ProtectedRef,
        ),
        (
            vec![("update", "refs/notes/commits")],
            Refusal::ProtectedRef,
        ),
        (vec![("update", "refs/pull/7/head")], Refusal::ProtectedRef),
        (vec![("create", "refs/tags/v1")], Refusal::ReleaseOrTag),
        (
            vec![("create", "refs/heads/ok"), ("create", "refs/tags/v1")],
            Refusal::ReleaseOrTag,
        ),
        (
            vec![("create", "refs/heads/ok"), ("update", "refs/heads/main")],
            Refusal::ProtectedRef,
        ),
    ] {
        let out = push("github.com", "/acme/app", &cmds);
        assert!(is_refused(&out, why), "{cmds:?}: {out:?}");
    }
    // The GHES repo's default branch is `trunk`, and `main` is then a branch like any other.
    assert!(is_refused(
        &push(
            "ghe.corp.example",
            "/acme/app",
            &[("update", "refs/heads/trunk")]
        ),
        Refusal::ProtectedRef
    ));
    assert!(matches!(
        push(
            "ghe.corp.example",
            "/acme/app",
            &[("create", "refs/heads/feature-x")]
        ),
        Outcome::Forward(_)
    ));
}

#[test]
fn a_push_whose_commands_cannot_be_read_is_refused() {
    purlis_core::unsteered!();
    let s = Scene::new();
    let url = "/acme/app.git/git-receive-pack";
    let mut cert = Vec::new();
    cert.extend(format!("{:04x}push-cert\0 report-status\n", 4 + 24).as_bytes());
    cert.extend(b"0000");
    let gzip = [0x1f, 0x8b, 8, 0, 0, 0, 0, 0];
    for (name, body) in [
        ("no body", Vec::new()),
        ("a flush alone", b"0000".to_vec()),
        ("gzip", gzip.to_vec()),
        ("not a command", b"0008abcd0000".to_vec()),
        ("a length past the body", b"00ffxx".to_vec()),
        ("a signed push", cert),
        (
            "no flush",
            push_body(&[("update", "refs/heads/x")])[..60].to_vec(),
        ),
    ] {
        let out = s.send("POST", "github.com", url, &body);
        assert!(is_refused(&out, Refusal::BodyUnreadable), "{name}: {out:?}");
    }
}
