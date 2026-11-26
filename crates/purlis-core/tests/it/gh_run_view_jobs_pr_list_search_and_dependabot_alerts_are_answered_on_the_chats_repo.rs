// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! #7679, #7680, #7681: three reads a chat makes of its own recorded repo through the forge route.
//!
//! - `gh run view <id> --json jobs` follows the run's `jobs_url`, an absolute
//!   `https://api.github.com/...` address, so it arrives on the TLS route with gh's own headers
//!   (`Host: api.github.com`, `Content-Type` on a `GET`, `Accept-Encoding: gzip`). The headers
//!   below are what gh 2.69 sent, captured with `GH_DEBUG=api`.
//! - `gh pr list --author ...` lists through GraphQL `search(query: $q, type: ISSUE)`, with the
//!   query in a variable: answered when every `repo:` qualifier is one of the chat's repos.
//! - A repo's Dependabot alerts are read, never written.

use purlis_core::forge::proxy::{Outcome, Refusal, TokenKind};

use crate::support::forge_listener::{Answer, World, plain};
use crate::support::forge_proxy::{Objects, Scene, graphql, is_refused};

fn scene() -> Scene {
    Scene::with(TokenKind::Oauth, Objects::default())
}

/// What gh 2.69 wrote to the chat's socket for the jobs of `gh run view <id> --json jobs`: the
/// run's `jobs_url` is `https://api.github.com/...`, which gh's socket transport sends as plain
/// HTTP with that `Host` and no port. Go's transport adds `Accept-Encoding: gzip`. `extra` is
/// put in before the blank line.
fn gh_run_view_jobs(world: &World, target: &str, host: &str, extra: &str) -> Vec<u8> {
    format!(
        "GET {target} HTTP/1.1\r\n\
         Host: {host}\r\n\
         User-Agent: GitHub CLI 2.69.0\r\n\
         Accept: application/vnd.github.merge-info-preview+json, application/vnd.github.nebula-preview\r\n\
         Authorization: token {}\r\n\
         Content-Type: application/json; charset=utf-8\r\n\
         Time-Zone: Asia/Yerevan\r\n\
         Accept-Encoding: gzip\r\n\
         {extra}\r\n",
        world.sentinel.expose()
    )
    .into_bytes()
}

const JOBS: &str = "/repos/acme/app/actions/runs/45770612164/jobs?per_page=100";

#[test]
fn gh_run_view_json_jobs_on_the_chats_socket_is_answered() {
    purlis_core::unsteered!();
    let w = World::new();
    for (target, upstream) in [
        (
            JOBS,
            "https://api.github.com/repos/acme/app/actions/runs/45770612164/jobs?per_page=100",
        ),
        ("/repos/acme/app", "https://api.github.com/repos/acme/app"),
    ] {
        w.fake.answer(Answer::ok(r#"{"jobs":[]}"#));
        let reply = plain(&w, &gh_run_view_jobs(&w, target, "api.github.com", ""));
        assert_eq!(reply.status, 200, "{target}: {}", reply.text());
        assert_eq!(w.fake.requests().last().unwrap().url, upstream);
    }
    // `:443` said out loud is the same place.
    w.fake.answer(Answer::ok(r#"{"jobs":[]}"#));
    let reply = plain(&w, &gh_run_view_jobs(&w, JOBS, "api.github.com:443", ""));
    assert_eq!(reply.status, 200, "{}", reply.text());
}

#[test]
fn a_portless_host_on_the_socket_names_only_the_https_port_and_only_the_chats_hosts() {
    purlis_core::unsteered!();
    let w = World::new();
    for host in [
        "api.github.com:80",
        "api.github.com:8443",
        "evil.example",
        "api.github.com.evil.example",
    ] {
        let reply = plain(&w, &gh_run_view_jobs(&w, JOBS, host, ""));
        assert_eq!(reply.status, 403, "{host}: {}", reply.text());
    }
    // Another repo on the right host is still another repo.
    let reply = plain(
        &w,
        &gh_run_view_jobs(
            &w,
            "/repos/acme/other/actions/runs/9/jobs",
            "api.github.com",
            "",
        ),
    );
    assert_eq!(reply.status, 403, "{}", reply.text());
    assert!(reply.text().contains("OtherRepo"), "{}", reply.text());
    assert!(w.fake.requests().is_empty(), "{:?}", w.fake.requests());
}

#[test]
fn a_request_that_is_still_malformed_is_refused_and_told_which_check_failed() {
    purlis_core::unsteered!();
    let w = World::new();
    for (extra, host, check) in [
        (
            "Content-Length: 0\r\nTransfer-Encoding: chunked\r\n",
            "api.github.com",
            "Content-Length",
        ),
        ("X_Forwarded: x\r\n", "api.github.com", "underscore"),
        (
            "content-type: text/plain\r\n",
            "api.github.com",
            "Content-Type",
        ),
        ("Host: evil.example\r\n", "api.github.com", "Host"),
        (
            "Content-Encoding: gzip\r\n",
            "api.github.com",
            "Content-Encoding",
        ),
    ] {
        let reply = plain(&w, &gh_run_view_jobs(&w, JOBS, host, extra));
        assert_eq!(reply.status, 403, "{check}: {}", reply.text());
        let said = reply.text();
        assert!(said.contains("MalformedRequest"), "{check}: {said}");
        assert!(said.contains(check), "{check}: {said}");
    }
    assert!(w.fake.requests().is_empty(), "{:?}", w.fake.requests());
}

/// What `gh pr list -R OWNER/REPO --author app/dependabot --state open --json ...` sends (gh
/// 2.69's `PullRequestSearch`, captured with `GH_DEBUG=api`): no `repository(...)` field, only the
/// search, with its query in `$q`.
const PR_SEARCH: &str = "fragment pr on PullRequest{number,title,headRefName}
    query PullRequestSearch(
      $q: String!,
      $limit: Int!,
      $endCursor: String,
    ) {
      search(query: $q, type: ISSUE, first: $limit, after: $endCursor) {
        issueCount
        nodes {
          ...pr
        }
        pageInfo {
          hasNextPage
          endCursor
        }
      }
    }";

fn pr_search(s: &Scene, q: &str) -> Outcome {
    s.send(
        "POST",
        "api.github.com",
        "/graphql",
        &graphql(PR_SEARCH, serde_json::json!({"limit": 30, "q": q})),
    )
}

#[test]
fn gh_pr_list_author_on_the_chats_repo_is_answered_as_a_read() {
    purlis_core::unsteered!();
    let s = scene();
    for q in [
        "author:app/dependabot repo:acme/app state:open type:pr",
        // `-R` and a `--search` that names the repo again: the same repo twice.
        "author:app/dependabot repo:acme/app repo:Acme/App state:open type:pr",
        // A value gh quotes because it holds a space.
        "label:\"needs review\" repo:acme/app type:pr",
        "repo:acme/app label:\"a b\" -label:\"c d\" \"some words\"",
    ] {
        let out = pr_search(&s, q);
        let Outcome::Forward(f) = out else {
            panic!("{q}: {out:?}");
        };
        assert!(f.repo.eq_ignore_ascii_case("acme/app"), "{q}: {}", f.repo);
    }
}

#[test]
fn a_pr_list_search_of_another_repo_none_an_org_or_a_user_is_refused() {
    purlis_core::unsteered!();
    let s = scene();
    for q in [
        "author:app/dependabot state:open type:pr",
        "author:app/dependabot repo:acme/other state:open type:pr",
        "repo:acme/app repo:acme/other type:pr",
        "repo:acme/app org:acme type:pr",
        "repo:acme/app user:acme type:pr",
        "repo:acme/app owner:acme type:pr",
        // A quoted repo, which the forge might read as text, leaving the search unscoped.
        "repo:\"acme/app\" type:pr",
        "label:\"a b\" repo:\"acme/app\"",
        // A quoted phrase is text to the forge: it scopes nothing.
        "\"repo:acme/app\" type:pr",
        "\"repo:acme/app org:acme\" type:pr",
        // A quote left open, and a quote inside a qualifier's name.
        "repo:acme/app label:\"open type:pr",
        "re\"po\":acme/app type:pr",
        "repo:\"acme/app\"x type:pr",
        "repo:\"acme/app\" \"\" org:acme",
        "repo:acme/app OR is:public",
    ] {
        let out = pr_search(&s, q);
        assert!(is_refused(&out, Refusal::GraphqlOtherRepo), "{q}: {out:?}");
    }
}

#[test]
fn the_chats_repos_dependabot_alerts_are_read() {
    purlis_core::unsteered!();
    let s = scene();
    for target in [
        "/repos/acme/app/dependabot/alerts",
        "/repos/Acme/App/dependabot/alerts?state=open&per_page=100",
        "/repos/acme/app/dependabot/alerts/12",
    ] {
        for method in ["GET", "HEAD"] {
            let out = s.send(method, "api.github.com", target, b"");
            assert!(
                matches!(out, Outcome::Forward(_)),
                "{method} {target}: {out:?}"
            );
        }
    }
}

#[test]
fn a_write_to_an_alert_and_another_repos_alerts_are_refused() {
    purlis_core::unsteered!();
    let s = scene();
    for (method, target, why) in [
        (
            "PATCH",
            "/repos/acme/app/dependabot/alerts/12",
            Refusal::NotOnList,
        ),
        (
            "POST",
            "/repos/acme/app/dependabot/alerts",
            Refusal::NotOnList,
        ),
        (
            "GET",
            "/repos/acme/other/dependabot/alerts",
            Refusal::OtherRepo,
        ),
        (
            "GET",
            "/repos/acme/other/dependabot/alerts/12",
            Refusal::OtherRepo,
        ),
    ] {
        let out = s.send(method, "api.github.com", target, b"{}");
        assert!(is_refused(&out, why), "{method} {target}: {out:?}");
    }
    // Neither a word for an alert's number nor Dependabot's secrets, which are administration.
    for target in [
        "/repos/acme/app/dependabot/alerts/x",
        "/repos/acme/app/dependabot/alerts/12/x",
        "/repos/acme/app/dependabot/secrets",
    ] {
        let out = s.send("GET", "api.github.com", target, b"");
        assert!(!matches!(out, Outcome::Forward(_)), "{target}: {out:?}");
    }
}
