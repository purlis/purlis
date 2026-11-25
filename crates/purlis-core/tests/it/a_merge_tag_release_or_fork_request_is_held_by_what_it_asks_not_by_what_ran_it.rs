// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The proxy holds an act by the request it reads, never by parsing a harness** (ADR 0128 D8.1,
//! D8.2): what `gh pr merge` and `glab mr merge` send, and the other ways to tag, release and fork,
//! each become one held request, or a refusal, with the one sentence the chat is told.

use purlis_core::forge::Kind;
use purlis_core::heldact::{Call, Recognised, Requested, Why, recognise, said};
use purlis_core::mergehold::Request;
use serde_json::{Value, json};

use crate::support::held_acts::{Rec, repo, tag};

fn run(kind: Kind, method: &str, path: &str, query: &[(&str, &str)], body: Value) -> Recognised {
    let segments: Vec<String> = path
        .trim_start_matches('/')
        .split('/')
        .map(str::to_owned)
        .collect();
    let query: Vec<(String, String)> = query
        .iter()
        .map(|(k, v)| ((*k).into(), (*v).into()))
        .collect();
    let call = Call {
        method,
        segments: &segments,
        query: &query,
        body: &body,
    };
    recognise(kind, &call, &Rec::acme())
}

fn merge_of(number: u64) -> Recognised {
    Recognised::Hold(Requested::Merge(Request {
        repo: Some(repo("acme/app")),
        number,
    }))
}

#[test]
fn glab_mr_merge_sends_a_request_the_proxy_holds_as_a_merge_of_that_request() {
    purlis_core::unsteered!();
    // glab: PUT /projects/:id/merge_requests/:iid/merge, whatever flags it carries.
    for body in [json!({}), json!({"squash": true, "sha": "abc"})] {
        assert_eq!(
            run(
                Kind::GitLab,
                "PUT",
                "/api/v4/projects/acme%2Fapp/merge_requests/12/merge",
                &[],
                body.clone()
            ),
            Recognised::Refuse(Why::NotRecorded),
            "a path segment is already decoded once by the proxy: `acme/app` is one segment"
        );
        assert_eq!(
            run(
                Kind::GitLab,
                "PUT",
                "/api/v4/projects/acme/app/merge_requests/12/merge",
                &[],
                body
            ),
            Recognised::Other,
            "an undecoded project path is not a project id"
        );
    }
    // The proxy's reading hands over the decoded id as one segment.
    let segments: Vec<String> = [
        "api",
        "v4",
        "projects",
        "acme/app",
        "merge_requests",
        "12",
        "merge",
    ]
    .map(str::to_owned)
    .to_vec();
    let body = json!({"squash": true});
    let call = Call {
        method: "PUT",
        segments: &segments,
        query: &[],
        body: &body,
    };
    assert_eq!(recognise(Kind::GitLab, &call, &Rec::acme()), merge_of(12));
    // A numeric project id is read through the recorded repos.
    let segments: Vec<String> = [
        "api",
        "v4",
        "projects",
        "42",
        "merge_requests",
        "12",
        "merge",
    ]
    .map(str::to_owned)
    .to_vec();
    let call = Call {
        method: "PUT",
        segments: &segments,
        query: &[],
        body: &body,
    };
    assert_eq!(recognise(Kind::GitLab, &call, &Rec::acme()), merge_of(12));
    // …and an id nobody recorded is no merge of a repo purlis acts on.
    let segments: Vec<String> = [
        "api",
        "v4",
        "projects",
        "43",
        "merge_requests",
        "12",
        "merge",
    ]
    .map(str::to_owned)
    .to_vec();
    let call = Call {
        method: "PUT",
        segments: &segments,
        query: &[],
        body: &body,
    };
    assert_eq!(
        recognise(Kind::GitLab, &call, &Rec::acme()),
        Recognised::Refuse(Why::NotRecorded)
    );
}

#[test]
fn a_github_rest_merge_and_the_graphql_readings_merge_are_the_same_held_request() {
    purlis_core::unsteered!();
    assert_eq!(
        run(
            Kind::GitHub,
            "PUT",
            "/repos/acme/app/pulls/12/merge",
            &[],
            json!({"merge_method": "squash"})
        ),
        merge_of(12)
    );
    // GHES serves the same under `/api/v3`.
    assert_eq!(
        run(
            Kind::GitHub,
            "PUT",
            "/api/v3/repos/acme/app/pulls/12/merge",
            &[],
            json!({})
        ),
        merge_of(12)
    );
    // `gh pr merge` sends GraphQL's `mergePullRequest`: the proxy's reading resolves its node id
    // to a recorded repo and a number, and hands over this.
    assert_eq!(
        Recognised::Hold(Requested::merge_of(repo("acme/app"), 12)),
        merge_of(12)
    );
    // A merge request number that is not a number is refused, not guessed.
    assert!(matches!(
        run(
            Kind::GitHub,
            "PUT",
            "/repos/acme/app/pulls/1e1/merge",
            &[],
            json!({})
        ),
        Recognised::Refuse(Why::Unreadable(_))
    ));
}

#[test]
fn the_sentence_the_chat_is_told_for_a_held_merge_is_the_one_the_record_gives() {
    purlis_core::unsteered!();
    assert_eq!(
        said::asked(&Requested::merge_of(repo("acme/app"), 12)),
        "purlis holds merges. It has asked the person; the merge happens when they approve it in Asks."
    );
}

#[test]
fn a_tag_is_held_by_its_ref_on_github_and_its_name_on_gitlab() {
    purlis_core::unsteered!();
    let sha = "a".repeat(40);
    let want = Recognised::Hold(Requested::Tag {
        repo: repo("acme/app"),
        name: tag("v1"),
        rev: sha.clone(),
    });
    assert_eq!(
        run(
            Kind::GitHub,
            "POST",
            "/repos/acme/app/git/refs",
            &[],
            json!({"ref": "refs/tags/v1", "sha": sha})
        ),
        want
    );
    let segments: Vec<String> = ["api", "v4", "projects", "acme/app", "repository", "tags"]
        .map(str::to_owned)
        .to_vec();
    let body = json!({"tag_name": "v1", "ref": sha});
    let call = Call {
        method: "POST",
        segments: &segments,
        query: &[],
        body: &body,
    };
    assert_eq!(recognise(Kind::GitLab, &call, &Rec::acme()), want);
    // A branch is no tag: the ordinary rules, and the CI diff, decide it.
    assert_eq!(
        run(
            Kind::GitHub,
            "POST",
            "/repos/acme/app/git/refs",
            &[],
            json!({"ref": "refs/heads/x", "sha": sha})
        ),
        Recognised::Other
    );
}

#[test]
fn a_tag_is_never_moved_or_deleted_by_a_chat() {
    purlis_core::unsteered!();
    assert_eq!(
        run(
            Kind::GitHub,
            "PATCH",
            "/repos/acme/app/git/refs/tags/v1",
            &[],
            json!({"sha": "a".repeat(40), "force": true})
        ),
        Recognised::Refuse(Why::TagMoved)
    );
    assert_eq!(
        run(
            Kind::GitHub,
            "DELETE",
            "/repos/acme/app/git/refs/tags/v1",
            &[],
            json!({})
        ),
        Recognised::Refuse(Why::TagDeleted)
    );
    let segments: Vec<String> = [
        "api",
        "v4",
        "projects",
        "acme/app",
        "repository",
        "tags",
        "v1",
    ]
    .map(str::to_owned)
    .to_vec();
    let body = json!({});
    let call = Call {
        method: "DELETE",
        segments: &segments,
        query: &[],
        body: &body,
    };
    assert_eq!(
        recognise(Kind::GitLab, &call, &Rec::acme()),
        Recognised::Refuse(Why::TagDeleted)
    );
}

#[test]
fn a_release_names_its_tag_and_where_the_tag_is_made() {
    purlis_core::unsteered!();
    let want = Recognised::Hold(Requested::Release {
        repo: repo("acme/app"),
        tag: tag("v2"),
        rev: Some("main".into()),
        title: "Two".into(),
        notes: "notes".into(),
        draft: false,
        prerelease: true,
    });
    assert_eq!(
        run(
            Kind::GitHub,
            "POST",
            "/repos/acme/app/releases",
            &[],
            json!({"tag_name": "v2", "target_commitish": "main", "name": "Two", "body": "notes", "prerelease": true})
        ),
        want
    );
    let segments: Vec<String> = ["api", "v4", "projects", "acme/app", "releases"]
        .map(str::to_owned)
        .to_vec();
    let body = json!({"tag_name": "v2", "ref": "main", "name": "Two", "description": "notes", "prerelease": "true"});
    let call = Call {
        method: "POST",
        segments: &segments,
        query: &[],
        body: &body,
    };
    assert_eq!(recognise(Kind::GitLab, &call, &Rec::acme()), want);
}

#[test]
fn the_chat_is_told_nothing_happened_and_what_happens_next_for_every_kind() {
    purlis_core::unsteered!();
    let sha = "a".repeat(40);
    for (requested, noun) in [
        (
            Requested::Tag {
                repo: repo("acme/app"),
                name: tag("v1"),
                rev: sha.clone(),
            },
            "tags",
        ),
        (
            Requested::Fork {
                source: repo("acme/app"),
            },
            "forks",
        ),
        (Requested::merge_of(repo("acme/app"), 3), "merges"),
    ] {
        let said = said::asked(&requested);
        assert!(said.starts_with(&format!("purlis holds {noun}.")), "{said}");
        assert!(said.contains("approve it in Asks"), "{said}");
    }
}

#[test]
fn a_repo_the_person_did_not_record_is_refused_for_every_kind() {
    purlis_core::unsteered!();
    for (method, path, body) in [
        ("PUT", "/repos/evil/other/pulls/1/merge", json!({})),
        ("POST", "/repos/evil/other/forks", json!({})),
        (
            "POST",
            "/repos/evil/other/releases",
            json!({"tag_name": "v1"}),
        ),
        (
            "POST",
            "/repos/evil/other/git/refs",
            json!({"ref": "refs/tags/v1", "sha": "a".repeat(40)}),
        ),
    ] {
        assert_eq!(
            run(Kind::GitHub, method, path, &[], body),
            Recognised::Refuse(Why::NotRecorded),
            "{method} {path}"
        );
    }
}

#[test]
fn a_read_or_an_ordinary_write_is_not_a_held_act() {
    purlis_core::unsteered!();
    assert_eq!(
        run(
            Kind::GitHub,
            "GET",
            "/repos/acme/app/pulls/12",
            &[],
            Value::Null
        ),
        Recognised::Other
    );
    assert_eq!(
        run(
            Kind::GitHub,
            "GET",
            "/repos/acme/app/releases",
            &[],
            Value::Null
        ),
        Recognised::Other
    );
    assert_eq!(
        run(
            Kind::GitHub,
            "POST",
            "/repos/acme/app/issues/3/comments",
            &[],
            json!({"body": "hi"})
        ),
        Recognised::Other
    );
}

#[test]
fn a_path_the_proxy_decoded_into_one_segment_is_read_as_the_path_it_is() {
    purlis_core::unsteered!();
    // `git/refs/tags%2Fv1` reaches the recogniser as the segments `git`, `refs`, `tags/v1`.
    let segments: Vec<String> = ["repos", "acme", "app", "git", "refs", "tags/v1"]
        .map(str::to_owned)
        .to_vec();
    let body = json!({"sha": "a".repeat(40), "force": true});
    let call = Call {
        method: "PATCH",
        segments: &segments,
        query: &[],
        body: &body,
    };
    assert_eq!(
        recognise(Kind::GitHub, &call, &Rec::acme()),
        Recognised::Refuse(Why::TagMoved)
    );
    let call = Call {
        method: "DELETE",
        segments: &segments,
        query: &[],
        body: &body,
    };
    assert_eq!(
        recognise(Kind::GitHub, &call, &Rec::acme()),
        Recognised::Refuse(Why::TagDeleted)
    );
    // And a merge spelled with an encoded slash in its path.
    let segments: Vec<String> = ["repos", "acme/app", "pulls", "12", "merge"]
        .map(str::to_owned)
        .to_vec();
    let call = Call {
        method: "PUT",
        segments: &segments,
        query: &[],
        body: &json!({}),
    };
    assert_eq!(recognise(Kind::GitHub, &call, &Rec::acme()), merge_of(12));
}

#[test]
fn the_other_ways_to_merge_and_to_change_a_release_are_refused_or_held_never_left_to_chance() {
    purlis_core::unsteered!();
    assert_eq!(
        run(
            Kind::GitHub,
            "POST",
            "/repos/acme/app/merges",
            &[],
            json!({"base": "main", "head": "x"})
        ),
        Recognised::Refuse(Why::MergesWithoutRequest)
    );
    for (method, path) in [
        ("PATCH", "/repos/acme/app/releases/7"),
        ("DELETE", "/repos/acme/app/releases/7"),
        ("POST", "/repos/acme/app/releases/7/assets"),
        ("PATCH", "/repos/acme/app/releases/assets/9"),
        ("DELETE", "/repos/acme/app/releases/assets/9"),
    ] {
        assert_eq!(
            run(Kind::GitHub, method, path, &[], json!({})),
            Recognised::Refuse(Why::ReleaseEdit),
            "{method} {path}"
        );
    }
    let id = "acme/app".to_owned();
    let on_gitlab = |method: &str, rest: &[&str], body: Value| {
        let mut segments: Vec<String> = ["api", "v4", "projects"].map(str::to_owned).to_vec();
        segments.push(id.clone());
        segments.extend(rest.iter().map(|part| (*part).to_owned()));
        let call = Call {
            method,
            segments: &segments,
            query: &[],
            body: &body,
        };
        recognise(Kind::GitLab, &call, &Rec::acme())
    };
    assert_eq!(
        on_gitlab("POST", &["merge_trains", "merge_requests", "12"], json!({})),
        merge_of(12)
    );
    assert_eq!(
        on_gitlab("PUT", &["releases", "v1"], json!({})),
        Recognised::Refuse(Why::ReleaseEdit)
    );
    assert_eq!(
        on_gitlab("DELETE", &["releases", "v1"], json!({})),
        Recognised::Refuse(Why::ReleaseEdit)
    );
    assert_eq!(
        on_gitlab("POST", &["releases", "v1", "assets", "links"], json!({})),
        Recognised::Refuse(Why::ReleaseEdit)
    );
}
