// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! One login, slice A: what a chat does on its own repo goes through, on every host (ADR 0128 D2).
//!
//! Fetch, push of a branch, opening a pull or merge request, commenting and reading CI are
//! allowed on github.com, a GitHub Enterprise Server, gitlab.com and a self-hosted GitLab, and the
//! forge is sent the person's credential in the form that surface reads, with none of the chat's.

use purlis_core::forge::proxy::{Class, Outcome, TokenKind};

use crate::support::forge_proxy::{
    Auth, GHES, GITLAB_SELF, Objects, REAL, Scene, graphql, headers, push_body,
};

fn class(outcome: Outcome) -> Class {
    match outcome {
        Outcome::Forward(f) => f.class,
        Outcome::Refused(r) => panic!("refused: {r:?}"),
    }
}

#[test]
fn a_fetch_and_a_clone_of_the_chats_repo_are_answered_on_every_host() {
    let s = Scene::new();
    for (host, port, repo) in [
        ("github.com", 443, "/Acme/App.git"),
        (GHES, 443, "/acme/app.git"),
        ("gitlab.com", 443, "/group/proj.git"),
        (GITLAB_SELF, 8443, "/grp/sub/proj.git"),
    ] {
        let refs = format!("{repo}/info/refs?service=git-upload-pack");
        assert_eq!(
            class(s.send_port("GET", host, port, &refs)),
            Class::GitRefs,
            "{host}"
        );
        let pack = format!("{repo}/git-upload-pack");
        assert_eq!(
            class(s.send_port("POST", host, port, &pack)),
            Class::GitFetch,
            "{host}"
        );
    }
}

#[test]
fn a_push_of_a_feature_branch_to_the_recorded_address_is_answered() {
    let s = Scene::new();
    let advert = s.send(
        "GET",
        "github.com",
        "/acme/app.git/info/refs?service=git-receive-pack",
        b"",
    );
    assert_eq!(class(advert), Class::GitRefs);
    let body = push_body(&[("create", "refs/heads/feature/one-login")]);
    assert_eq!(
        class(s.send(
            "POST",
            "github.com",
            "/acme/app.git/git-receive-pack",
            &body
        )),
        Class::GitPush
    );
    let body = push_body(&[("update", "refs/heads/work")]); // seeded as the chat's own
    assert_eq!(
        class(s.send(
            "POST",
            "gitlab.com",
            "/group/proj.git/git-receive-pack",
            &body
        )),
        Class::GitPush
    );
}

#[test]
fn opening_a_pull_request_commenting_and_reading_ci_are_answered_on_github() {
    let s = Scene::new();
    for (method, target, want) in [
        ("POST", "/repos/acme/app/pulls", Class::PullRequestWrite),
        ("PATCH", "/repos/acme/app/pulls/7", Class::PullRequestWrite),
        (
            "POST",
            "/repos/acme/app/issues/7/comments",
            Class::CommentWrite,
        ),
        (
            "POST",
            "/repos/acme/app/pulls/7/comments",
            Class::CommentWrite,
        ),
        ("POST", "/repos/acme/app/issues/7/labels", Class::LabelWrite),
        ("POST", "/repos/acme/app/issues", Class::IssueWrite),
        ("GET", "/repos/acme/app/pulls/7", Class::ApiRead),
        (
            "GET",
            "/repos/acme/app/actions/runs?branch=x",
            Class::ApiRead,
        ),
        (
            "GET",
            "/repos/acme/app/commits/abc123/check-runs",
            Class::ApiRead,
        ),
        ("GET", "/repos/acme/app/actions/runs/9/logs", Class::ApiRead),
        (
            "GET",
            "/repos/acme/app/contents/docs/keys/readme.md",
            Class::ApiRead,
        ),
    ] {
        assert_eq!(
            class(s.send(method, "api.github.com", target, b"{}")),
            want,
            "{method} {target}"
        );
    }
}

#[test]
fn the_same_work_is_answered_on_a_github_enterprise_servers_api() {
    let s = Scene::new();
    assert_eq!(
        class(s.send("POST", GHES, "/api/v3/repos/acme/app/pulls", b"{}")),
        Class::PullRequestWrite
    );
    assert_eq!(
        class(s.send("GET", GHES, "/api/v3/repos/acme/app/actions/runs", b"")),
        Class::ApiRead
    );
}

#[test]
fn opening_a_merge_request_commenting_and_reading_ci_are_answered_on_gitlab() {
    let s = Scene::new();
    for (host, port, base) in [
        ("gitlab.com", 443, "/api/v4/projects/group%2Fproj"),
        ("gitlab.com", 443, "/api/v4/projects/42"),
        (GITLAB_SELF, 8443, "/api/v4/projects/grp%2Fsub%2Fproj"),
    ] {
        for (method, tail, want) in [
            ("POST", "/merge_requests", Class::PullRequestWrite),
            ("PUT", "/merge_requests/3", Class::PullRequestWrite),
            ("POST", "/merge_requests/3/notes", Class::CommentWrite),
            (
                "POST",
                "/merge_requests/3/discussions/abc/notes",
                Class::CommentWrite,
            ),
            ("GET", "/pipelines", Class::ApiRead),
            ("GET", "/pipelines/12/jobs", Class::ApiRead),
            (
                "GET",
                "/repository/files/docs%2Fkeys.md?ref=main",
                Class::ApiRead,
            ),
        ] {
            let target = format!("{base}{tail}");
            assert_eq!(
                class(s.send_port(method, host, port, &target)),
                want,
                "{host} {method} {target}"
            );
        }
    }
}

#[test]
fn a_graphql_query_on_the_chats_repo_and_a_pull_request_it_creates_are_answered() {
    let s = Scene::new();
    let query = graphql(
        "query($o:String!,$n:String!){ viewer{login} repository(owner:$o,name:$n){ pullRequest(number:7){ title statusCheckRollup: commits(last:1){ totalCount } } } }",
        serde_json::json!({"o": "ACME", "n": "App"}),
    );
    assert_eq!(
        class(s.send("POST", "api.github.com", "/graphql", &query)),
        Class::GraphqlQuery
    );
    let create = graphql(
        "mutation($i:CreatePullRequestInput!){ createPullRequest(input:$i){ pullRequest{ url } } }",
        serde_json::json!({"i": {"repositoryId": "R_acme_app", "baseRefName": "main", "headRefName": "x", "title": "t"}}),
    );
    assert_eq!(
        class(s.send("POST", "api.github.com", "/graphql", &create)),
        Class::GraphqlMutation("createPullRequest".into())
    );
    let gl = graphql(
        "query{ project(fullPath:\"group/proj\"){ mergeRequests{ count } } }",
        serde_json::json!({}),
    );
    assert_eq!(
        class(s.send("POST", "gitlab.com", "/api/graphql", &gl)),
        Class::GraphqlQuery
    );
}

#[test]
fn a_mutation_on_an_object_is_answered_only_when_the_object_is_known_to_be_in_the_chats_repo() {
    let mut known = Objects::default();
    known.0.insert("PR_kwDO_1".into(), "acme/app".into());
    known.0.insert("PR_kwDO_other".into(), "acme/other".into());
    let s = Scene::with(TokenKind::Oauth, known);
    let comment = |id: &str| {
        graphql(
            "mutation($i:AddCommentInput!){ addComment(input:$i){ clientMutationId } }",
            serde_json::json!({"i": {"subjectId": id, "body": "hi"}}),
        )
    };
    assert!(matches!(
        s.send("POST", "api.github.com", "/graphql", &comment("PR_kwDO_1")),
        Outcome::Forward(_)
    ));
    assert!(matches!(
        s.send(
            "POST",
            "api.github.com",
            "/graphql",
            &comment("PR_kwDO_other")
        ),
        Outcome::Refused(_)
    ));
    assert!(matches!(
        s.send(
            "POST",
            "api.github.com",
            "/graphql",
            &comment("PR_kwDO_unknown")
        ),
        Outcome::Refused(_)
    ));
}

#[test]
fn the_forge_is_sent_the_real_credential_and_none_of_the_chats() {
    let s = Scene::new();
    let sentinel = s.a_sentinel.expose().to_string();
    let mut sent = headers(Auth::Bearer(&sentinel));
    sent.push(("Cookie".into(), "session=abc".into()));
    sent.push(("Proxy-Authorization".into(), format!("Bearer {sentinel}")));
    let head = purlis_core::forge::proxy::RequestHead::new(
        "GET",
        "api.github.com",
        443,
        "/repos/acme/app/pulls",
    );
    let out = s.proxy.handle(
        &s.a,
        &purlis_core::forge::proxy::Inbound {
            head: &head,
            headers: &sent,
            body: b"",
        },
    );
    let Outcome::Forward(f) = out else {
        panic!("refused")
    };
    let all: String = f
        .headers
        .iter()
        .map(|(n, v)| format!("{n}: {v}\n"))
        .collect();
    assert!(!all.contains(&sentinel), "the sentinel went to the forge");
    assert!(!all.to_lowercase().contains("cookie"));
    assert!(!all.to_lowercase().contains("proxy-authorization"));
    assert!(
        all.contains("Accept: application/json"),
        "other headers are kept"
    );
    let auth: Vec<_> = f
        .headers
        .iter()
        .filter(|(n, _)| n.eq_ignore_ascii_case("authorization"))
        .collect();
    assert_eq!(auth.len(), 1);
    assert_eq!(auth[0].1, format!("Bearer {REAL}"));
}

#[test]
fn the_credential_takes_the_form_each_surface_reads() {
    let s = Scene::new();
    let find = |o: Outcome, name: &str| match o {
        Outcome::Forward(f) => f
            .headers
            .into_iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v),
        Outcome::Refused(r) => panic!("refused: {r:?}"),
    };
    let basic = |user: &str| {
        format!(
            "Basic {}",
            data_encoding::BASE64.encode(format!("{user}:{REAL}").as_bytes())
        )
    };
    assert_eq!(
        find(
            s.send(
                "GET",
                "github.com",
                "/acme/app.git/info/refs?service=git-upload-pack",
                b""
            ),
            "authorization"
        ),
        Some(basic("x-access-token"))
    );
    assert_eq!(
        find(
            s.send(
                "GET",
                "gitlab.com",
                "/group/proj.git/info/refs?service=git-upload-pack",
                b""
            ),
            "authorization"
        ),
        Some(basic("oauth2"))
    );
    assert_eq!(
        find(
            s.send("GET", "gitlab.com", "/api/v4/projects/42/pipelines", b""),
            "authorization"
        ),
        Some(format!("Bearer {REAL}"))
    );
    let pat = Scene::with(TokenKind::Pat, Objects::default());
    assert_eq!(
        find(
            pat.send("GET", "gitlab.com", "/api/v4/projects/42/pipelines", b""),
            "private-token"
        ),
        Some(REAL.to_string())
    );
}
