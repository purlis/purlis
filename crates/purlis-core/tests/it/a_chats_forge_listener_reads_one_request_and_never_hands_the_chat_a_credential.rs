// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! One login, slice A2 (ADR 0128 D3a to D3c, D3g): the listener reads one request, asks the core,
//! sends what it allows to the forge with the real credential, and reads the answer before the chat
//! does. Run on socket pairs, so no port is bound.

use purlis_core::forge::proxy::{AllowedRepo, Kind};

use crate::support::forge_listener::{
    Answer, REAL, World, basic, gh_get, parse_reply, pkt, plain, plain_raw,
};

/// `scheme://user:secret@rest`, joined here so that no line of the file is shaped like one.
fn credentialed(scheme: &str, user: &str, secret: &str, rest: &str) -> String {
    format!("{scheme}://{user}{}{secret}{}{rest}", ':', '@')
}

/// `scheme://user@rest`: a user and no secret.
fn credentialed_bare(scheme: &str, user: &str, rest: &str) -> String {
    format!("{scheme}://{user}{}{rest}", '@')
}

fn sentinel(w: &World) -> String {
    w.sentinel.expose().to_string()
}

#[test]
fn gh_on_its_socket_reaches_the_forge_as_the_person_and_the_chat_never_sees_the_token() {
    purlis_core::unsteered!();
    let w = World::new();
    w.fake.answer(Answer::ok(r#"{"login":"octocat"}"#));
    let reply = plain(&w, &gh_get(&w, "/user"));
    assert_eq!(reply.status, 200, "{}", reply.text());
    assert_eq!(reply.text(), r#"{"login":"octocat"}"#);

    let seen = w.fake.requests();
    assert_eq!(seen.len(), 1);
    // The address is the core's, built from the recorded host: never the chat's `Host`.
    assert_eq!(seen[0].url, "https://api.github.com/user");
    assert_eq!(seen[0].method, "GET");
    let auth: Vec<&str> = seen[0]
        .headers
        .iter()
        .filter(|(n, _)| n.eq_ignore_ascii_case("authorization"))
        .map(|(_, v)| v.as_str())
        .collect();
    assert_eq!(auth, [format!("Bearer {REAL}")]);
    let everything = format!("{:?}", seen[0]);
    assert!(
        !everything.contains(&sentinel(&w)),
        "the sentinel went on: {everything}"
    );
    assert!(
        seen[0]
            .headers
            .iter()
            .all(|(n, _)| !n.eq_ignore_ascii_case("host")),
        "{:?}",
        seen[0].headers
    );
}

#[test]
fn a_refused_request_is_answered_403_and_nothing_is_sent_to_the_forge() {
    purlis_core::unsteered!();
    let w = World::new();
    let merge = format!(
        "PUT /repos/acme/app/pulls/7/merge HTTP/1.1\r\nHost: api.github.localhost\r\nAuthorization: token {}\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{{}}",
        sentinel(&w)
    );
    let reply = plain(&w, merge.as_bytes());
    assert_eq!(reply.status, 403, "{}", reply.text());
    assert!(reply.text().contains("Merge"), "{}", reply.text());
    // A wrong sentinel, another repo, an unknown host.
    for request in [
        "GET /user HTTP/1.1\r\nHost: api.github.localhost\r\nAuthorization: token not-the-sentinel\r\n\r\n".to_string(),
        "GET /repos/acme/other/pulls HTTP/1.1\r\nHost: api.github.localhost\r\n\r\n".to_string(),
        "GET /user HTTP/1.1\r\nHost: evil.example\r\n\r\n".to_string(),
        "GET /user HTTP/1.1\r\n\r\n".to_string(),
    ] {
        let reply = plain(&w, request.as_bytes());
        assert_eq!(reply.status, 403, "{request:?}: {}", reply.text());
    }
    assert!(w.fake.requests().is_empty(), "{:?}", w.fake.requests());
}

#[test]
fn with_two_github_hosts_github_localhost_fails_closed() {
    purlis_core::unsteered!();
    let w = World::with_repos(vec![
        AllowedRepo::new(Kind::GitHub, "github.com", 443, "acme/app", "main").unwrap(),
        AllowedRepo::new(Kind::GitHub, "ghe.corp.example", 443, "acme/app", "trunk").unwrap(),
    ]);
    let reply = plain(&w, &gh_get(&w, "/user"));
    assert_eq!(reply.status, 403, "{}", reply.text());
    assert!(reply.text().contains("AmbiguousHost"), "{}", reply.text());
    assert!(w.fake.requests().is_empty());
    // And the refusal is in the chat's audit, as the core would have written it.
    assert!(!w.proxy.audit(&w.chat).is_empty());
}

#[test]
fn a_ghes_only_chat_has_github_coms_paths_mapped_to_the_enterprise_api() {
    purlis_core::unsteered!();
    let w = World::with_repos(vec![
        AllowedRepo::new(Kind::GitHub, "ghe.corp.example", 443, "acme/app", "trunk").unwrap(),
    ]);
    for (sent, wanted) in [
        ("/user", "https://ghe.corp.example/api/v3/user"),
        (
            "/repos/acme/app/pulls?state=open",
            "https://ghe.corp.example/api/v3/repos/acme/app/pulls?state=open",
        ),
    ] {
        w.fake.answer(Answer::ok("{}"));
        let reply = plain(&w, &gh_get(&w, sent));
        assert_eq!(reply.status, 200, "{sent}: {}", reply.text());
        assert_eq!(w.fake.requests().last().unwrap().url, wanted);
    }
}

#[test]
fn an_answer_that_holds_the_real_token_is_never_sent_to_the_chat() {
    purlis_core::unsteered!();
    let w = World::new();
    for answer in [
        // In the body, as it is, percent-encoded and as Base64; and in a header.
        Answer::ok(&format!(r#"{{"token":"{REAL}"}}"#)),
        Answer::ok(&format!(
            r#"{{"x":"{}"}}"#,
            data_encoding::BASE64.encode(REAL.as_bytes())
        )),
        Answer::ok("{}").with_header("X-Echo", REAL),
        Answer::ok("{}").with_header("Location", &format!("https://api.github.com/x?q={REAL}")),
    ] {
        w.fake.answer(answer);
        let raw = plain_raw(&w, &gh_get(&w, "/user"));
        let text = String::from_utf8_lossy(&raw);
        assert!(!text.contains(REAL), "the token reached the chat: {text}");
        assert!(
            !text.contains(&data_encoding::BASE64.encode(REAL.as_bytes())),
            "{text}"
        );
        assert_eq!(parse_reply(&raw).status, 502, "{text}");
    }
}

#[test]
fn an_encoded_answer_is_never_sent_because_nothing_could_look_inside_it() {
    purlis_core::unsteered!();
    let w = World::new();
    for encoding in ["gzip", "br", "deflate", "zstd", "GZIP"] {
        // The bytes are not a real encoding of anything: the answer is refused for its header,
        // not read, so what it holds does not matter, and none of it reaches the chat.
        w.fake
            .answer(Answer::ok(&format!("body {REAL}")).with_header("Content-Encoding", encoding));
        let raw = plain_raw(&w, &gh_get(&w, "/user"));
        let text = String::from_utf8_lossy(&raw);
        assert_eq!(parse_reply(&raw).status, 502, "{encoding}: {text}");
        assert!(!text.contains(REAL), "{encoding}: {text}");
        assert!(!text.contains("body "), "{encoding}: {text}");
    }
    // The identity coding is no coding at all.
    w.fake
        .answer(Answer::ok("{}").with_header("Content-Encoding", "identity"));
    assert_eq!(
        parse_reply(&plain_raw(&w, &gh_get(&w, "/user"))).status,
        200
    );
}

#[test]
fn a_token_split_across_two_reads_is_found_and_none_of_it_is_sent() {
    purlis_core::unsteered!();
    let w = World::new();
    // Far more than one read, with the token across a boundary of any size.
    for before in [
        0usize,
        1,
        32 * 1024 - 7,
        32 * 1024 - 1,
        32 * 1024 + 3,
        70_000,
    ] {
        let body = format!("{}{REAL}{}", "a".repeat(before), "b".repeat(100));
        w.fake.answer(Answer::ok(&body));
        let raw = plain_raw(&w, &gh_get(&w, "/user"));
        let text = String::from_utf8_lossy(&raw);
        assert!(!text.contains(REAL), "{before}: the token reached the chat");
        assert!(
            !text.contains(&REAL[..REAL.len() / 2]),
            "{before}: half of it reached the chat"
        );
    }
}

#[test]
fn a_credentialed_clone_url_in_a_text_answer_is_cut_down_to_the_url() {
    purlis_core::unsteered!();
    let w = World::new();
    // Synthetic test URLs, put together here: a bare user is not a credential.
    let ssh = credentialed_bare("ssh", "git", "github.com/acme/app.git");
    let scp = format!("git{}github.com:acme/app.git", '@');
    let body = format!(
        r#"{{"clone_url":"{}","ssh_url":"{ssh}","note":"{scp}"}}"#,
        credentialed(
            "https",
            "x-access-token",
            "ghs_abcdef",
            "github.com/acme/app.git"
        )
    );
    w.fake.answer(Answer::ok(&body));
    let reply = plain(&w, &gh_get(&w, "/repos/acme/app"));
    assert_eq!(reply.status, 200);
    let text = reply.text();
    assert!(
        !text.contains("ghs_abcdef") && !text.contains("x-access-token"),
        "{text}"
    );
    assert!(text.contains("https://github.com/acme/app.git"), "{text}");
    // A bare user is not a credential.
    assert!(text.contains(&ssh), "{text}");
    assert!(text.contains(&scp), "{text}");
}

#[test]
fn a_url_cut_across_two_reads_is_cut_down_whole() {
    purlis_core::unsteered!();
    let w = World::new();
    for before in [0usize, 5, 32 * 1024 - 20, 32 * 1024 - 3, 32 * 1024 + 1] {
        let body = format!(
            r#"{}"{}" and {}"#,
            "a".repeat(before),
            credentialed(
                "https",
                "oauth2",
                "glpat-SECRETSECRET",
                "gitlab.com/g/p.git"
            ),
            "b".repeat(40_000)
        );
        w.fake.answer(Answer::ok(&body));
        let reply = plain(&w, &gh_get(&w, "/repos/acme/app"));
        let text = reply.text();
        assert!(
            !text.contains("SECRETSECRET") && !text.contains("glpat"),
            "{before}: leaked"
        );
        assert!(
            text.contains("https://gitlab.com/g/p.git"),
            "{before}: {}",
            &text[before..before + 60]
        );
        assert_eq!(
            text.len(),
            body.len() - "oauth2:glpat-SECRETSECRET@".len(),
            "{before}"
        );
    }
}

#[test]
fn link_and_location_are_written_again_for_the_chat() {
    purlis_core::unsteered!();
    let w = World::new();
    let link = [
        "<https://api.github.com/repositories/1/pulls?page=2>; rel=\"next\"".to_string(),
        "<https://evil.example/steal>; rel=\"last\"".to_string(),
        format!(
            "<{}>; rel=\"prev\"",
            credentialed("https", "user", "pw", "api.github.com/x")
        ),
        "<https://api.github.com/repositories/1/pulls?access_token=abc>; rel=\"first\"".to_string(),
    ]
    .join(", ");
    w.fake.answer(Answer::ok("[]").with_header("Link", &link));
    let reply = plain(&w, &gh_get(&w, "/repos/acme/app/pulls"));
    assert_eq!(
        reply.header("link"),
        Some("<http://api.github.localhost/repositories/1/pulls?page=2>; rel=\"next\"")
    );
    // Nothing the chat is sent sets a cookie or keeps a connection.
    w.fake.answer(
        Answer::ok("[]")
            .with_header("Set-Cookie", "a=b")
            .with_header("Alt-Svc", "h3=\":443\""),
    );
    let reply = plain(&w, &gh_get(&w, "/repos/acme/app/pulls"));
    assert!(reply.header("set-cookie").is_none() && reply.header("alt-svc").is_none());
    assert_eq!(reply.header("connection"), Some("close"));
}

#[test]
fn a_redirect_to_a_download_host_is_followed_once_with_no_credential() {
    purlis_core::unsteered!();
    let w = World::new();
    let signed = "https://release-assets.githubusercontent.com/github-production-release-asset/1?sig=abc&se=2026";
    w.fake
        .answer(Answer::ok("").status(302).with_header("Location", signed));
    w.fake.answer(Answer {
        status: 200,
        headers: vec![("Content-Type".into(), "application/octet-stream".into())],
        body: b"BINARYASSET".to_vec(),
    });
    let reply = plain(&w, &gh_get(&w, "/repos/acme/app/releases/assets/9"));
    assert_eq!(reply.status, 200, "{}", reply.text());
    assert_eq!(reply.body, b"BINARYASSET");
    let seen = w.fake.requests();
    assert_eq!(seen.len(), 2);
    assert_eq!(seen[1].url, signed);
    assert!(
        seen[1]
            .headers
            .iter()
            .all(|(n, _)| !n.eq_ignore_ascii_case("authorization")),
        "the credential followed the redirect: {:?}",
        seen[1].headers
    );
    assert!(!format!("{:?}", seen[1]).contains(REAL));
}

#[test]
fn a_redirect_anywhere_else_or_a_second_one_is_not_followed() {
    purlis_core::unsteered!();
    let w = World::new();
    for location in [
        "https://evil.example/x".to_string(),
        "http://api.github.com/x".to_string(),
        credentialed("https", "user", "pw", "codeload.github.com/x"),
        "https://codeload.github.com:8443/x".to_string(),
        "https://api.github.com.evil.example/x".to_string(),
    ] {
        w.fake.answer(
            Answer::ok("")
                .status(302)
                .with_header("Location", &location),
        );
        let reply = plain(&w, &gh_get(&w, "/repos/acme/app/releases/assets/9"));
        assert_eq!(reply.status, 502, "{location}: {}", reply.text());
        assert_eq!(w.fake.requests().len(), 1, "{location}");
        w.fake.seen.lock().unwrap().clear();
    }
    // The storage host answers with another redirect: not followed again.
    w.fake.answer(
        Answer::ok("")
            .status(302)
            .with_header("Location", "https://codeload.github.com/a"),
    );
    w.fake.answer(
        Answer::ok("")
            .status(302)
            .with_header("Location", "https://codeload.github.com/b"),
    );
    let reply = plain(&w, &gh_get(&w, "/repos/acme/app/releases/assets/9"));
    assert!(reply.status >= 300, "{reply:?}");
    assert_eq!(w.fake.requests().len(), 2);
    assert!(
        reply
            .header("location")
            .is_none_or(|l| !l.contains("codeload")),
        "{reply:?}"
    );
    // A redirect inside the recorded host is written again for the chat, which makes its own request.
    w.fake.seen.lock().unwrap().clear();
    w.fake.answer(
        Answer::ok("")
            .status(301)
            .with_header("Location", "https://api.github.com/repositories/9"),
    );
    let reply = plain(&w, &gh_get(&w, "/repos/acme/app"));
    assert_eq!(reply.status, 301);
    assert_eq!(
        reply.header("location"),
        Some("http://api.github.localhost/repositories/9")
    );
    assert_eq!(w.fake.requests().len(), 1);
}

#[test]
fn framing_that_a_forge_might_read_another_way_is_refused_and_a_second_request_is_never_served() {
    purlis_core::unsteered!();
    let w = World::new();
    let s = sentinel(&w);
    let both = format!(
        "POST /repos/acme/app/issues HTTP/1.1\r\nHost: api.github.localhost\r\nAuthorization: token {s}\r\nContent-Type: application/json\r\nContent-Length: 2\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n"
    );
    let underscore = format!(
        "GET /user HTTP/1.1\r\nHost: api.github.localhost\r\nAuthorization: token {s}\r\nContent_Type: x\r\n\r\n"
    );
    for request in [both, underscore] {
        let reply = plain(&w, request.as_bytes());
        assert_eq!(reply.status, 403, "{request:?}: {}", reply.text());
    }
    for raw in [
        "GET /user HTTP/1.0\r\nHost: api.github.localhost\r\n\r\n",
        "GET /user HTTP/1.1\nHost: api.github.localhost\n\n",
        "GET /user HTTP/1.1\r\nHost: api.github.localhost\r\nX: a\nb\r\n\r\n",
        "GET /user HTTP/1.1\r\nHost: api.github.localhost\r\nX: a\0b\r\n\r\n",
        "GET http://api.github.com/user HTTP/1.1\r\nHost: api.github.localhost\r\n\r\n",
        "NOT HTTP\r\n\r\n",
    ] {
        let reply = plain(&w, raw.as_bytes());
        assert!(
            reply.status == 400 || reply.status == 403,
            "{raw:?}: {reply:?}"
        );
    }
    assert!(w.fake.requests().is_empty(), "{:?}", w.fake.requests());
    // Two requests in one write: the first is answered, the second is not read as one.
    w.fake.answer(Answer::ok(r#"{"login":"a"}"#));
    w.fake.answer(Answer::ok(r#"{"login":"b"}"#));
    let one = String::from_utf8(gh_get(&w, "/user")).unwrap();
    let raw = plain_raw(&w, format!("{one}{one}").as_bytes());
    let text = String::from_utf8_lossy(&raw);
    assert_eq!(text.matches("HTTP/1.1 ").count(), 1, "{text}");
    assert_eq!(w.fake.requests().len(), 1);
}

#[test]
fn a_chunked_body_is_read_whole_and_sent_with_the_length_it_had_and_one_content_type() {
    purlis_core::unsteered!();
    let w = World::new();
    w.fake.answer(Answer::ok("{}").status(201));
    let request = format!(
        "POST /repos/acme/app/issues/7/comments HTTP/1.1\r\nHost: api.github.localhost\r\nAuthorization: token {}\r\nContent-Type: application/json; charset=utf-8\r\nTransfer-Encoding: chunked\r\nExpect: 100-continue\r\n\r\n8\r\n{{\"body\":\r\n5\r\n\"ok\"}}\r\n0\r\n\r\n",
        sentinel(&w)
    );
    let raw = plain_raw(&w, request.as_bytes());
    let text = String::from_utf8_lossy(&raw);
    assert!(text.starts_with("HTTP/1.1 100 Continue"), "{text}");
    let reply = parse_reply(&raw[raw.windows(4).position(|x| x == b"\r\n\r\n").unwrap() + 4..]);
    assert_eq!(reply.status, 201);
    let seen = w.fake.requests();
    assert_eq!(seen[0].body, br#"{"body":"ok"}"#);
    let types: Vec<&str> = seen[0]
        .headers
        .iter()
        .filter(|(n, _)| n.eq_ignore_ascii_case("content-type"))
        .map(|(_, v)| v.as_str())
        .collect();
    assert_eq!(types, ["application/json"]);
    assert!(
        seen[0]
            .headers
            .iter()
            .all(|(n, _)| !n.eq_ignore_ascii_case("transfer-encoding")
                && !n.eq_ignore_ascii_case("expect")),
        "{:?}",
        seen[0].headers
    );
}

#[test]
fn a_body_over_the_limit_is_refused_before_it_is_read() {
    purlis_core::unsteered!();
    let w = World::new();
    let big = format!(
        "POST /repos/acme/app/issues HTTP/1.1\r\nHost: api.github.localhost\r\nAuthorization: token {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
        sentinel(&w),
        (1 << 20) + 1
    );
    let reply = plain(&w, big.as_bytes());
    assert_eq!(reply.status, 413, "{}", reply.text());
    assert!(w.fake.requests().is_empty());
}

#[test]
fn a_push_is_decided_on_its_commands_and_its_branch_is_owned_only_when_the_forge_took_all_of_it() {
    purlis_core::unsteered!();
    let w = World::new();
    let zero = "0".repeat(40);
    let new = "1".repeat(40);
    let command = |name: &str, old: &str| {
        let line =
            format!("{old} {new} refs/heads/{name}\0 report-status side-band-64k agent=git/2");
        format!("{}0000PACK....", pkt(&line))
    };
    let push = |body: &str| {
        let head = format!(
            "POST /acme/app.git/git-receive-pack HTTP/1.1\r\nHost: github.com\r\nAuthorization: {}\r\nContent-Type: application/x-git-receive-pack-request\r\nContent-Length: {}\r\n\r\n",
            basic("x-access-token", w.sentinel.expose()),
            body.len()
        );
        format!("{head}{body}")
    };
    // The proxy sees a push through the terminating door: use it on a socket pair as `git` does.
    let ca = purlis_core::forge::proxy::listener::ChatCa::new(&w.proxy.terminable_hosts(&w.chat))
        .unwrap();
    let send = |request: &str| {
        crate::support::forge_listener::tls(
            &w,
            &ca,
            "github.com",
            443,
            "github.com",
            request.as_bytes(),
        )
        .expect("a handshake")
    };

    // The forge refuses it: the branch is not the chat's.
    w.fake.answer(Answer::ok(&format!(
        "{}{}0000",
        pkt("unpack ok\n"),
        pkt("ng refs/heads/mine rejected\n")
    )));
    let first = send(&push(&command("mine", &zero)));
    assert_eq!(first.status, 200, "{}", first.text());
    let moved = send(&push(&command("mine", &"2".repeat(40))));
    assert_eq!(
        moved.status,
        403,
        "a branch the forge refused is not owned: {}",
        moved.text()
    );
    assert!(moved.text().contains("ProtectedRef"), "{}", moved.text());

    // The forge takes it: now it is.
    w.fake.answer(Answer::ok(&format!(
        "{}{}0000",
        pkt("unpack ok\n"),
        pkt("ok refs/heads/mine\n")
    )));
    assert_eq!(send(&push(&command("mine", &zero))).status, 200);
    w.fake.answer(Answer::ok(&format!(
        "{}{}0000",
        pkt("unpack ok\n"),
        pkt("ok refs/heads/mine\n")
    )));
    assert_eq!(send(&push(&command("mine", &"2".repeat(40)))).status, 200);

    // The same report inside side-band, as GitHub sends it.
    let inner = format!(
        "{}{}0000",
        pkt("unpack ok\n"),
        pkt("ok refs/heads/banded\n")
    );
    let banded = format!("{}0000", pkt(&format!("\u{1}{inner}")));
    w.fake.answer(Answer::ok(&banded));
    assert_eq!(send(&push(&command("banded", &zero))).status, 200);
    w.fake.answer(Answer::ok("{}"));
    assert_eq!(
        send(&push(&command("banded", &"2".repeat(40)))).status,
        200,
        "owned after a banded report"
    );

    // Refused: the default branch, a tag, a delete; never sent.
    let seen_before = w.fake.requests().len();
    for body in [
        command("main", &zero),
        command("release", &zero),
        command("release-1", &zero),
    ] {
        assert_eq!(send(&push(&body)).status, 403, "{body}");
    }
    assert_eq!(w.fake.requests().len(), seen_before);
}

#[test]
fn a_push_advertisement_is_read_so_a_branch_that_exists_is_not_made() {
    purlis_core::unsteered!();
    let w = World::new();
    let ca = purlis_core::forge::proxy::listener::ChatCa::new(&w.proxy.terminable_hosts(&w.chat))
        .unwrap();
    let advert = format!(
        "{}0000{}{}0000",
        pkt("# service=git-receive-pack\n"),
        pkt(&format!(
            "{} refs/heads/taken\0 report-status\n",
            "3".repeat(40)
        )),
        pkt(&format!("{} refs/heads/main\n", "4".repeat(40))),
    );
    w.fake.answer(Answer::ok(&advert).with_header(
        "Content-Type",
        "application/x-git-receive-pack-advertisement",
    ));
    let request = format!(
        "GET /acme/app.git/info/refs?service=git-receive-pack HTTP/1.1\r\nHost: github.com\r\nAuthorization: {}\r\n\r\n",
        basic("x-access-token", w.sentinel.expose())
    );
    let reply = crate::support::forge_listener::tls(
        &w,
        &ca,
        "github.com",
        443,
        "github.com",
        request.as_bytes(),
    )
    .unwrap();
    assert_eq!(reply.status, 200);
    assert_eq!(
        reply.body,
        advert.as_bytes(),
        "the advertisement is passed on as it is"
    );
    let zero = "0".repeat(40);
    let line = format!("{zero} {} refs/heads/taken\0 report-status", "1".repeat(40));
    let body = format!("{}0000PACK", pkt(&line));
    let push = format!(
        "POST /acme/app.git/git-receive-pack HTTP/1.1\r\nHost: github.com\r\nAuthorization: {}\r\nContent-Type: application/x-git-receive-pack-request\r\nContent-Length: {}\r\n\r\n{body}",
        basic("x-access-token", w.sentinel.expose()),
        body.len()
    );
    let reply = crate::support::forge_listener::tls(
        &w,
        &ca,
        "github.com",
        443,
        "github.com",
        push.as_bytes(),
    )
    .unwrap();
    assert_eq!(reply.status, 403, "{}", reply.text());
}
