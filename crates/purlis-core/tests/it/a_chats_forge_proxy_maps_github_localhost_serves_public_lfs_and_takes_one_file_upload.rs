// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! One login, slice A2 (ADR 0128 D3c, D3f, D3i): `gh`'s `GH_HOST=github.localhost` means the one
//! GitHub host a chat's repos are on, and no other; Git LFS is its download batch call, for a
//! recorded repo and, anonymously, for a public one; a multipart body is carried only as one file
//! to the one route that takes an upload, and refused anywhere else.

use purlis_core::forge::proxy::{
    AllowedRepo, ChatId, Class, ForgeProxy, Inbound, Kind, NoCredentials, NoObjects, Outcome,
    Refusal, RequestHead,
};

use crate::support::forge_proxy::{Objects, Scene, is_refused};

fn alias(s: &Scene, host: &str) -> Option<Result<purlis_core::forge::proxy::GithubAlias, Refusal>> {
    s.proxy.github_alias(&s.a, host)
}

#[test]
fn github_localhost_is_the_one_github_host_the_chat_has_and_fails_closed_with_two() {
    purlis_core::unsteered!();
    let one = Scene::single();
    for name in [
        "github.localhost",
        "api.github.localhost",
        "API.GitHub.Localhost",
    ] {
        let a = alias(&one, name).expect("an alias").expect("one host");
        assert_eq!(
            (a.host.as_str(), a.port, a.enterprise),
            ("api.github.com", 443, false),
            "{name}"
        );
    }
    // Any other name is not an alias, so the policy reads it as it reads any host.
    for name in [
        "github.com",
        "api.github.com",
        "github.localhost.evil.example",
        "localhost",
    ] {
        assert!(alias(&one, name).is_none(), "{name}");
    }
    // github.com and a GHES: which is meant is not the chat's to say.
    let two = Scene::with_two_github_hosts(
        purlis_core::forge::proxy::TokenKind::Oauth,
        Objects::default(),
    );
    for name in ["github.localhost", "api.github.localhost"] {
        assert_eq!(
            alias(&two, name).expect("an alias").unwrap_err(),
            Refusal::AmbiguousHost,
            "{name}"
        );
    }
    // A chat that ended, or never was, has no host to map to.
    assert_eq!(
        one.proxy
            .github_alias(&ChatId::new("nobody"), "github.localhost")
            .expect("an alias")
            .unwrap_err(),
        Refusal::UnknownChat
    );
}

#[test]
fn the_alias_of_a_ghes_host_maps_github_coms_paths_to_the_enterprise_api() {
    purlis_core::unsteered!();
    let proxy = ForgeProxy::new(Box::new(NoCredentials), Box::new(NoObjects));
    let chat = ChatId::new("ghes-only");
    proxy.enrol(
        &chat,
        vec![AllowedRepo::new(Kind::GitHub, "ghe.corp.example", 443, "acme/app", "trunk").unwrap()],
    );
    let a = proxy
        .github_alias(&chat, "api.github.localhost")
        .unwrap()
        .unwrap();
    assert_eq!(a.host, "ghe.corp.example");
    assert!(a.enterprise);
    for (sent, wanted) in [
        ("/user", "/api/v3/user"),
        ("/graphql", "/api/graphql"),
        (
            "/repos/acme/app/pulls?state=open",
            "/api/v3/repos/acme/app/pulls?state=open",
        ),
    ] {
        assert_eq!(a.target(sent), wanted);
    }
    let github = alias(&Scene::single(), "github.localhost")
        .unwrap()
        .unwrap();
    assert_eq!(github.target("/repos/acme/app?x=1"), "/repos/acme/app?x=1");
}

fn lfs(s: &Scene, host: &str, target: &str, ct: &str, body: &str, sentinel: bool) -> Outcome {
    let head = RequestHead::new("POST", host, 443, target);
    let mut headers = vec![("Content-Type".to_string(), ct.to_string())];
    if sentinel {
        headers.push((
            "Authorization".to_string(),
            format!("Bearer {}", s.a_sentinel.expose()),
        ));
    }
    s.proxy.handle(
        &s.a,
        &Inbound {
            head: &head,
            headers: &headers,
            body: body.as_bytes(),
        },
    )
}

const LFS_TYPE: &str = "application/vnd.git-lfs+json";
const DOWNLOAD: &str =
    r#"{"operation":"download","transfers":["basic"],"objects":[{"oid":"abc","size":1}]}"#;

#[test]
fn an_lfs_download_batch_is_answered_for_the_chats_repo_and_anonymously_for_a_public_one() {
    purlis_core::unsteered!();
    let s = Scene::new();
    let Outcome::Forward(f) = lfs(
        &s,
        "github.com",
        "/acme/app.git/info/lfs/objects/batch",
        LFS_TYPE,
        DOWNLOAD,
        true,
    ) else {
        panic!("the chat's own repo is answered");
    };
    assert_eq!(f.class, Class::LfsBatch);
    assert!(f.credentialed);
    assert_eq!(
        f.upstream,
        "https://github.com/acme/app.git/info/lfs/objects/batch"
    );
    let types: Vec<&str> = f
        .headers
        .iter()
        .filter(|(n, _)| n.eq_ignore_ascii_case("content-type"))
        .map(|(_, v)| v.as_str())
        .collect();
    assert_eq!(types, [LFS_TYPE]);

    // A repo that is not the chat's: no credential, whatever the chat held.
    for sentinel in [true, false] {
        let Outcome::Forward(f) = lfs(
            &s,
            "github.com",
            "/someone/public.git/info/lfs/objects/batch",
            LFS_TYPE,
            DOWNLOAD,
            sentinel,
        ) else {
            panic!("a public repo is answered");
        };
        assert!(!f.credentialed);
        assert!(
            f.headers
                .iter()
                .all(|(n, _)| !n.eq_ignore_ascii_case("authorization")),
            "{:?}",
            f.headers
        );
        assert!(f.body.is_some(), "the batch is sent as it was read");
    }
}

#[test]
fn everything_else_of_lfs_is_refused_with_or_without_a_sentinel() {
    purlis_core::unsteered!();
    let s = Scene::new();
    let batch = "/acme/app.git/info/lfs/objects/batch";
    let upload = r#"{"operation":"upload","objects":[]}"#;
    for sentinel in [true, false] {
        for (target, ct, body) in [
            (batch, LFS_TYPE, upload),
            (batch, LFS_TYPE, "{}"),
            (batch, LFS_TYPE, r#"{"operation":["download"]}"#),
            (
                batch,
                LFS_TYPE,
                r#"{"operation":"download","operation":"upload"}"#,
            ),
            ("/acme/app.git/info/lfs/locks", LFS_TYPE, DOWNLOAD),
            ("/acme/app.git/info/lfs/locks/verify", LFS_TYPE, DOWNLOAD),
            ("/acme/app.git/info/lfs/objects/abc", LFS_TYPE, DOWNLOAD),
            ("/other/public.git/info/lfs/objects/batch", LFS_TYPE, upload),
        ] {
            let out = lfs(&s, "github.com", target, ct, body, sentinel);
            assert!(
                is_refused(&out, Refusal::Lfs) || is_refused(&out, Refusal::BodyUnreadable),
                "{target} {body}: {out:?}"
            );
        }
        for ct in [
            "text/plain",
            "application/x-www-form-urlencoded",
            "multipart/form-data; boundary=x",
        ] {
            let out = lfs(&s, "github.com", batch, ct, DOWNLOAD, sentinel);
            assert!(
                is_refused(&out, Refusal::BodyUnreadable) || is_refused(&out, Refusal::Multipart),
                "{ct}: {out:?}"
            );
        }
    }
    // A GET of the batch address is not the batch call.
    let out = s.send("GET", "github.com", batch, b"");
    assert!(is_refused(&out, Refusal::Lfs), "{out:?}");
}

const UPLOAD: &str = "/api/v4/projects/42/uploads";

fn multipart(boundary: &str, parts: &str) -> (String, Vec<u8>) {
    (
        format!("multipart/form-data; boundary={boundary}"),
        parts.as_bytes().to_vec(),
    )
}

fn send_upload(s: &Scene, target: &str, ct: &str, body: &[u8]) -> Outcome {
    let head = RequestHead::new("POST", "gitlab.com", 443, target);
    let headers = vec![
        ("Content-Type".to_string(), ct.to_string()),
        (
            "PRIVATE-TOKEN".to_string(),
            s.a_sentinel.expose().to_string(),
        ),
    ];
    s.proxy.handle(
        &s.a,
        &Inbound {
            head: &head,
            headers: &headers,
            body,
        },
    )
}

#[test]
fn one_file_to_gitlabs_upload_route_is_carried_as_it_was_sent_and_nothing_else_multipart_is() {
    purlis_core::unsteered!();
    let s = Scene::new();
    let ok = "--XyZ\r\nContent-Disposition: form-data; name=\"file\"; filename=\"shot.png\"\r\nContent-Type: image/png\r\n\r\nPNG\r\n\r\ndata\r\n--XyZ--\r\n";
    let (ct, body) = multipart("XyZ", ok);
    let Outcome::Forward(f) = send_upload(&s, UPLOAD, &ct, &body) else {
        panic!("one file is carried");
    };
    assert_eq!(f.class, Class::UploadWrite);
    assert_eq!(f.body.as_deref(), Some(&body[..]));
    let types: Vec<&str> = f
        .headers
        .iter()
        .filter(|(n, _)| n.eq_ignore_ascii_case("content-type"))
        .map(|(_, v)| v.as_str())
        .collect();
    assert_eq!(types, ["multipart/form-data; boundary=XyZ"]);

    let two = "--XyZ\r\nContent-Disposition: form-data; name=\"file\"; filename=\"a\"\r\n\r\nx\r\n--XyZ\r\nContent-Disposition: form-data; name=\"note\"\r\n\r\n/merge\r\n--XyZ--";
    let other_name = "--XyZ\r\nContent-Disposition: form-data; name=\"note\"; filename=\"a\"\r\n\r\nx\r\n--XyZ--";
    let path_name = "--XyZ\r\nContent-Disposition: form-data; name=\"file\"; filename=\"../../a\"\r\n\r\nx\r\n--XyZ--";
    let extra_header = "--XyZ\r\nContent-Disposition: form-data; name=\"file\"; filename=\"a\"\r\nX-Other: 1\r\n\r\nx\r\n--XyZ--";
    let trailing = "--XyZ\r\nContent-Disposition: form-data; name=\"file\"; filename=\"a\"\r\n\r\nx\r\n--XyZ--\r\nmore";
    let preamble = "junk\r\n--XyZ\r\nContent-Disposition: form-data; name=\"file\"; filename=\"a\"\r\n\r\nx\r\n--XyZ--";
    for (what, text) in [
        ("two parts", two),
        ("another name", other_name),
        ("a path as the name", path_name),
        ("another header", extra_header),
        ("bytes after the end", trailing),
        ("bytes before the start", preamble),
    ] {
        let (ct, body) = multipart("XyZ", text);
        let out = send_upload(&s, UPLOAD, &ct, &body);
        assert!(is_refused(&out, Refusal::Multipart), "{what}: {out:?}");
    }
    for ct in [
        "multipart/form-data",
        "multipart/form-data; boundary=\"\"",
        "multipart/form-data; boundary=XyZ; charset=x",
        "multipart/mixed; boundary=XyZ",
        "multipart/form-data; boundary=a b",
    ] {
        let out = send_upload(&s, UPLOAD, ct, ok.as_bytes());
        assert!(is_refused(&out, Refusal::Multipart), "{ct}: {out:?}");
    }
    // Every other write takes JSON or a form and says multipart is not what it is.
    for target in [
        "/api/v4/projects/42/issues/1/notes",
        "/api/v4/projects/42/merge_requests",
        "/api/v4/projects/42/uploads/extra",
    ] {
        let out = send_upload(&s, target, &ct, &body);
        assert!(
            is_refused(&out, Refusal::Multipart) || is_refused(&out, Refusal::NotOnList),
            "{target}: {out:?}"
        );
    }
    // And GitHub has no such route.
    let head = RequestHead::new("POST", "api.github.com", 443, "/repos/acme/app/uploads");
    let headers = vec![
        ("Content-Type".to_string(), ct.clone()),
        (
            "Authorization".to_string(),
            format!("Bearer {}", s.a_sentinel.expose()),
        ),
    ];
    let out = s.proxy.handle(
        &s.a,
        &Inbound {
            head: &head,
            headers: &headers,
            body: &body,
        },
    );
    assert!(matches!(out, Outcome::Refused(_)), "{out:?}");
}
