// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! #7494, #7526: update-branch (GitHub) and a rebase (GitLab) make the forge write the pull
//! request's base into its **head**. So they are forwarded only for a head that is a branch of the
//! chat's own repo that this chat put its own tip on, at the tip the forge reports now: the record
//! the push rule reads. Any other head (the default branch, `develop`, `release/*`, another chat's
//! or the person's branch, a fork's, one the forge cannot place, one that moved since the chat
//! pushed it) is refused, whatever the pull request's base says or is changed to. A pull request
//! from the default branch is refused when it is opened, as the wrong way round.

use purlis_core::forge::proxy::{Outcome, Refusal};

use crate::support::forge_proxy::{ObjectsWithHeads, Scene, graphql, is_refused};

const CREATE: &str = "mutation PullRequestCreate($input: CreatePullRequestInput!) { \
    createPullRequest(input: $input) { pullRequest { id url } } }";

const UPDATE_BRANCH: &str = "mutation PullRequestUpdateBranch($input: UpdatePullRequestBranchInput!) { \
    updatePullRequestBranch(input: $input) { pullRequest { id } } }";

/// chat-a's repo is `Acme/App` (default `main`, `release` protected), and the chat made `fix`
/// (tip `1` * 40). Pull requests of it, by node id and number: 7 from `fix`; 8 from `main`; 9 from
/// a fork's `fix`; 10 from `release`; 11 from `Main`; 13 from `develop` and 14 from `m75-policy`
/// (branches the chat did not make); 15 from `fix` after someone moved it; 16 from `release/1.2`.
/// A pull request 12 exists but the forge cannot say its head. On GitLab (`group/proj`, project
/// 42, the chat made `work`): MR 5 from `work`, 6 from `feature`, 7 from `main`.
fn scene() -> Scene {
    let mut objects = ObjectsWithHeads::default()
        .pull("acme/app", "PR_feature", 7, "acme/app", "fix")
        .pull("acme/app", "PR_main", 8, "acme/app", "main")
        .pull("acme/app", "PR_fork", 9, "someone/app", "fix")
        .pull("acme/app", "PR_release", 10, "acme/app", "release")
        .pull("acme/app", "PR_cased", 11, "acme/app", "Main")
        .pull("acme/app", "PR_develop", 13, "acme/app", "develop")
        .pull("acme/app", "PR_policy", 14, "acme/app", "m75-policy")
        .pull_at(
            "acme/app",
            "PR_moved",
            15,
            "acme/app",
            "fix",
            &"2".repeat(40),
        )
        .pull("acme/app", "PR_release_x", 16, "acme/app", "release/1.2")
        .pull("group/proj", "MR_own", 5, "group/proj", "work")
        .pull("group/proj", "MR_feature", 6, "group/proj", "feature")
        .pull("group/proj", "MR_main", 7, "group/proj", "main");
    objects
        .objects
        .0
        .insert("PR_nohead".into(), "acme/app".into());
    Scene::tuned_owning(objects, &[("fix", &"1".repeat(40))], |proxy| proxy)
}

fn gql(s: &Scene, query: &str, variables: serde_json::Value) -> Outcome {
    s.send(
        "POST",
        "api.github.com",
        "/graphql",
        &graphql(query, variables),
    )
}

fn create(s: &Scene, head: &str, base: &str) -> Outcome {
    gql(
        s,
        CREATE,
        serde_json::json!({"input": {
            "repositoryId": "R_acme_app", "baseRefName": base, "headRefName": head,
            "title": "t", "body": "b"}}),
    )
}

fn update(s: &Scene, id: &str) -> Outcome {
    gql(
        s,
        UPDATE_BRANCH,
        serde_json::json!({"input": {"pullRequestId": id, "expectedHeadOid": "a".repeat(40)}}),
    )
}

fn rest_create(s: &Scene, head: &str, base: &str) -> Outcome {
    let body = serde_json::json!({"title": "t", "head": head, "base": base});
    s.send(
        "POST",
        "api.github.com",
        "/repos/Acme/App/pulls",
        body.to_string().as_bytes(),
    )
}

fn rest_update(s: &Scene, number: u64) -> Outcome {
    s.send(
        "PUT",
        "api.github.com",
        &format!("/repos/Acme/App/pulls/{number}/update-branch"),
        b"",
    )
}

fn rebase(s: &Scene, number: u64) -> Outcome {
    s.send(
        "PUT",
        "gitlab.com",
        &format!("/api/v4/projects/42/merge_requests/{number}/rebase"),
        b"",
    )
}

fn forwarded(out: &Outcome) -> bool {
    matches!(out, Outcome::Forward(_))
}

#[test]
fn a_pull_request_from_the_default_branch_is_refused_in_every_spelling_by_graphql_and_rest() {
    purlis_core::unsteered!();
    let s = scene();
    for head in [
        "main",
        "Main",
        "refs/heads/main",
        "acme:main",
        "acme:refs/heads/main",
        "acme/app:main",
        "Acme/App:refs/heads/Main",
    ] {
        let out = create(&s, head, "fix");
        assert!(
            is_refused(&out, Refusal::ProtectedPullHead),
            "graphql {head}: {out:?}"
        );
        let out = rest_create(&s, head, "fix");
        assert!(
            is_refused(&out, Refusal::ProtectedPullHead),
            "rest {head}: {out:?}"
        );
    }
}

#[test]
fn a_pull_request_from_a_release_branch_or_another_branch_is_opened_as_any_other() {
    purlis_core::unsteered!();
    let s = scene();
    // Writing to the head is where ownership is asked (update-branch), not when it is opened.
    for head in [
        "fix",
        "acme:fix",
        "refs/heads/fix",
        "release/1.2",
        "develop",
    ] {
        for base in ["main", "fix"] {
            let out = create(&s, head, base);
            assert!(forwarded(&out), "graphql {head}->{base}: {out:?}");
            let out = rest_create(&s, head, base);
            assert!(forwarded(&out), "rest {head}->{base}: {out:?}");
        }
    }
}

#[test]
fn a_graphql_pull_request_from_main_is_refused_when_it_names_its_head_repository_by_id() {
    purlis_core::unsteered!();
    let s = scene();
    let input = |head: &str, head_repo: &str| {
        serde_json::json!({"input": {
            "repositoryId": "R_acme_app", "headRepositoryId": head_repo,
            "baseRefName": "fix", "headRefName": head, "title": "t"}})
    };
    let out = gql(&s, CREATE, input("main", "R_acme_app"));
    assert!(is_refused(&out, Refusal::ProtectedPullHead), "{out:?}");
    // A head repository named by id is not one the route resolves (as before #7526), the chat's
    // own included, so it is never forwarded. A `createPullRequest` is refused as
    // `GraphqlPullRequestOtherRepo` (#7544), the kind that names the REST route.
    let out = gql(&s, CREATE, input("fix", "R_acme_app"));
    assert!(
        is_refused(&out, Refusal::GraphqlPullRequestOtherRepo),
        "{out:?}"
    );
    let out = gql(&s, CREATE, input("fix", "R_someone_else"));
    assert!(!forwarded(&out), "{out:?}");
}

#[test]
fn a_pull_request_from_a_non_main_default_branch_is_refused_and_one_from_main_is_not() {
    purlis_core::unsteered!();
    // chat-a's GHES repo `acme/app` has `trunk` as its default.
    let s = Scene::with_two_github_hosts(
        purlis_core::forge::proxy::TokenKind::Oauth,
        crate::support::forge_proxy::Objects::default(),
    );
    let on_ghes = |head: &str| {
        let body = serde_json::json!({"title": "t", "head": head, "base": "fix"});
        s.send(
            "POST",
            crate::support::forge_proxy::GHES,
            "/api/v3/repos/acme/app/pulls",
            body.to_string().as_bytes(),
        )
    };
    for head in ["trunk", "refs/heads/trunk", "acme:trunk", "Trunk"] {
        let out = on_ghes(head);
        assert!(
            is_refused(&out, Refusal::ProtectedPullHead),
            "{head}: {out:?}"
        );
    }
    assert!(forwarded(&on_ghes("feature")));
}

#[test]
fn a_graphql_update_branch_on_a_head_the_chat_did_not_make_is_refused() {
    purlis_core::unsteered!();
    let s = scene();
    // The default, a protected and a release branch; an unprotected branch of the person's or of
    // another chat's (A of #7526); a fork's; one the forge cannot place; one that moved.
    for id in [
        "PR_main",
        "PR_release",
        "PR_cased",
        "PR_develop",
        "PR_policy",
        "PR_release_x",
        "PR_fork",
        "PR_nohead",
        "PR_moved",
    ] {
        let out = update(&s, id);
        assert!(
            is_refused(&out, Refusal::ProtectedPullHead),
            "{id}: {out:?}"
        );
    }
    // An alias does not hide it.
    let aliased = "mutation($input: UpdatePullRequestBranchInput!) { \
        a: updatePullRequestBranch(input: $input) { pullRequest { id } } }";
    let out = gql(
        &s,
        aliased,
        serde_json::json!({"input": {"pullRequestId": "PR_develop"}}),
    );
    assert!(is_refused(&out, Refusal::ProtectedPullHead), "{out:?}");
    // Nor does a second, harmless mutation in the same document.
    let two = "mutation($a: UpdatePullRequestBranchInput!, $b: UpdatePullRequestBranchInput!) { \
        ok: updatePullRequestBranch(input: $a) { pullRequest { id } } \
        bad: updatePullRequestBranch(input: $b) { pullRequest { id } } }";
    let out = gql(
        &s,
        two,
        serde_json::json!({"a": {"pullRequestId": "PR_feature"}, "b": {"pullRequestId": "PR_develop"}}),
    );
    assert!(is_refused(&out, Refusal::ProtectedPullHead), "{out:?}");
    // No pull request named at all.
    let out = gql(
        &s,
        UPDATE_BRANCH,
        serde_json::json!({"input": {"repositoryId": "R_acme_app"}}),
    );
    assert!(is_refused(&out, Refusal::ProtectedPullHead), "{out:?}");
}

#[test]
fn a_graphql_rebase_of_a_head_the_chat_did_not_make_is_refused_and_of_its_own_is_not() {
    purlis_core::unsteered!();
    let s = scene();
    let rebase = |id: &str| {
        gql(
            &s,
            UPDATE_BRANCH,
            serde_json::json!({"input": {"pullRequestId": id, "updateMethod": "REBASE"}}),
        )
    };
    let out = rebase("PR_develop");
    assert!(is_refused(&out, Refusal::ProtectedPullHead), "{out:?}");
    let out = rebase("PR_feature");
    assert!(forwarded(&out), "{out:?}");
}

#[test]
fn a_graphql_update_branch_on_a_branch_the_chat_made_is_still_answered() {
    purlis_core::unsteered!();
    let out = update(&scene(), "PR_feature");
    assert!(forwarded(&out), "{:?}", out);
}

#[test]
fn a_rest_update_branch_on_a_head_the_chat_did_not_make_is_refused() {
    purlis_core::unsteered!();
    let s = scene();
    for number in [8, 9, 10, 11, 12, 13, 14, 15, 16] {
        let out = rest_update(&s, number);
        assert!(
            is_refused(&out, Refusal::ProtectedPullHead),
            "#{number}: {out:?}"
        );
    }
    // A resolver that implements no lookup knows no head.
    let out = rest_update(&Scene::new(), 7);
    assert!(is_refused(&out, Refusal::ProtectedPullHead), "{out:?}");
}

#[test]
fn a_rest_update_branch_on_a_branch_the_chat_made_is_still_answered() {
    purlis_core::unsteered!();
    let out = rest_update(&scene(), 7);
    assert!(forwarded(&out), "{out:?}");
}

#[test]
fn changing_a_pull_requests_base_to_the_chats_branch_does_not_open_its_head_to_update_branch() {
    purlis_core::unsteered!();
    // B of #7526: the person's pull request 13 is from `develop` into `main`. The chat may edit
    // it (its base moves onto `fix`), but update-branch writes to `develop`, which it did not make.
    let s = scene();
    let patch = s.send(
        "PATCH",
        "api.github.com",
        "/repos/Acme/App/pulls/13",
        br#"{"base":"fix"}"#,
    );
    assert!(forwarded(&patch), "{patch:?}");
    let out = rest_update(&s, 13);
    assert!(is_refused(&out, Refusal::ProtectedPullHead), "{out:?}");
    let by_graphql = gql(
        &s,
        "mutation($input: UpdatePullRequestInput!) { \
            updatePullRequest(input: $input) { pullRequest { id } } }",
        serde_json::json!({"input": {"pullRequestId": "PR_develop", "baseRefName": "fix"}}),
    );
    // Whatever the edit is answered with, the update that follows it is not.
    let _ = by_graphql;
    let out = update(&s, "PR_develop");
    assert!(is_refused(&out, Refusal::ProtectedPullHead), "{out:?}");
}

#[test]
fn a_gitlab_rebase_is_forwarded_only_for_a_source_branch_the_chat_made() {
    purlis_core::unsteered!();
    let s = scene();
    let out = rebase(&s, 5);
    assert!(forwarded(&out), "{out:?}");
    // A reversed merge request (source: the default branch) and another's branch.
    for number in [6, 7, 99] {
        let out = rebase(&s, number);
        assert!(
            is_refused(&out, Refusal::ProtectedPullHead),
            "!{number}: {out:?}"
        );
    }
    // A forge that cannot say what the source is.
    let out = rebase(&Scene::new(), 5);
    assert!(is_refused(&out, Refusal::ProtectedPullHead), "{out:?}");
}

#[test]
fn a_gitlab_merge_request_from_the_default_branch_is_refused_when_opened() {
    purlis_core::unsteered!();
    let s = scene();
    let open = |source: &str| {
        let body = serde_json::json!({
            "source_branch": source, "target_branch": "work", "title": "t"});
        s.send(
            "POST",
            "gitlab.com",
            "/api/v4/projects/42/merge_requests",
            body.to_string().as_bytes(),
        )
    };
    for source in ["main", "Main", "refs/heads/main"] {
        let out = open(source);
        assert!(
            is_refused(&out, Refusal::ProtectedPullHead),
            "{source}: {out:?}"
        );
    }
    assert!(forwarded(&open("work")));
    assert!(forwarded(&open("release/1.2")));
}

#[test]
fn the_refusal_says_what_to_do_instead() {
    purlis_core::unsteered!();
    let said = Refusal::ProtectedPullHead.advice().expect("advice");
    assert!(
        said.contains("from your feature branch into the default branch"),
        "{said}"
    );
    assert!(said.contains("this chat's own branches"), "{said}");
}

#[test]
fn a_gitlab_graphql_merge_request_from_the_default_branch_is_refused_inline_and_by_variable() {
    purlis_core::unsteered!();
    let s = scene();
    let send = |query: &str, variables: serde_json::Value| {
        s.send(
            "POST",
            "gitlab.com",
            "/api/graphql",
            &graphql(query, variables),
        )
    };
    let inline = |source: &str| {
        send(
            &format!(
                "mutation {{ mergeRequestCreate(input: {{projectPath: \"group/proj\", \
                 sourceBranch: \"{source}\", targetBranch: \"work\", title: \"t\"}}) \
                 {{ errors }} }}"
            ),
            serde_json::json!({}),
        )
    };
    let by_variable = |source: &str| {
        send(
            "mutation($input: MergeRequestCreateInput!) { \
             mergeRequestCreate(input: $input) { errors } }",
            serde_json::json!({"input": {
                "projectPath": "group/proj", "sourceBranch": source,
                "targetBranch": "work", "title": "t"}}),
        )
    };
    for source in ["main", "Main", "refs/heads/main"] {
        for (how, out) in [
            ("inline", inline(source)),
            ("variable", by_variable(source)),
        ] {
            assert!(
                is_refused(&out, Refusal::ProtectedPullHead),
                "{how} {source}: {out:?}"
            );
        }
    }
    for source in ["work", "release/1.2"] {
        assert!(forwarded(&inline(source)), "inline {source}");
        assert!(forwarded(&by_variable(source)), "variable {source}");
    }
}
